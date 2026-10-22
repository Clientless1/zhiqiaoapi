use std::sync::Arc;

use axum::{
    extract::{FromRequestParts, State},
    http::{request::Parts, HeaderMap},
    Json,
};
use serde::Serialize;
use sqlx::Row;

use crate::{
    error::{AppError, AppResult},
    id::DbId,
    AppState,
};

const MAX_ADMIN_LOGIN_FAILURES: i32 = 5;
const ADMIN_LOGIN_LOCK_TTL: std::time::Duration = std::time::Duration::from_secs(15 * 60);

#[derive(Debug, Clone)]
pub struct UserSessionAuth {
    pub user_id: DbId,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct UserSessionState {
    pub(super) user_id: DbId,
    pub(super) auth_version: i64,
    requires_password_change: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct LoginResponse {
    pub token: String,
    pub role: String,
    pub requires_password_change: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct MeResponse {
    pub id: DbId,
    pub role: String,
    pub requires_password_change: bool,
    pub username: String,
    pub email: Option<String>,
}

impl FromRequestParts<Arc<AppState>> for UserSessionAuth {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let session = user_session_from_headers(state, &parts.headers).await?;
        if session.requires_password_change {
            return Err(AppError::PasswordChangeRequired);
        }

        super::set_request_auth_log_context(parts, "user", Some(session.user_id));
        Ok(Self {
            user_id: session.user_id,
        })
    }
}

pub(super) async fn me(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> AppResult<Json<MeResponse>> {
    let token = super::bearer(&headers).ok_or(AppError::Unauthorized)?;
    if let Some(claims) =
        super::validate_admin_session_token_claims(token, &state.config.admin_token_secret)
    {
        super::validate_admin_session_claims(&state, claims).await?;
        let username =
            sqlx::query_scalar::<_, String>("SELECT username::TEXT FROM admin WHERE id=$1")
                .bind(claims.admin_id)
                .fetch_one(&state.db.pool)
                .await?;
        return Ok(Json(MeResponse {
            id: claims.admin_id,
            role: "admin".to_string(),
            requires_password_change: false,
            username,
            email: None,
        }));
    }

    let session = user_session_from_token(&state, token).await?;
    let profile = sqlx::query("SELECT username, email::TEXT FROM \"user\" WHERE id=$1")
        .bind(session.user_id)
        .fetch_one(&state.db.pool)
        .await?;
    let email: String = profile.try_get("email")?;

    Ok(Json(MeResponse {
        id: session.user_id,
        role: "user".to_string(),
        requires_password_change: session.requires_password_change,
        username: profile
            .try_get::<Option<String>, _>("username")?
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| email.split('@').next().unwrap_or(&email).to_string()),
        email: Some(email),
    }))
}

pub(super) async fn login_admin(
    state: &AppState,
    username: &str,
    password: &str,
) -> AppResult<Option<LoginResponse>> {
    let row = sqlx::query(
        r#"
        SELECT id, password_hash, status, COALESCE(locked_until > now(), FALSE) AS locked
        FROM admin
        WHERE username = $1
        ORDER BY id ASC
        LIMIT 1
        "#,
    )
    .bind(username)
    .fetch_optional(&state.db.pool)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let id: DbId = row.try_get("id")?;
    let status: String = row.try_get("status")?;
    let locked: bool = row.try_get("locked")?;
    if status != "enabled" || locked {
        return Err(AppError::Unauthorized);
    }

    let password_hash: String = row.try_get("password_hash")?;
    if !super::verify_user_password(password, &state.config.admin_token_secret, &password_hash) {
        record_admin_login_failure(state, id).await?;
        return Err(AppError::Unauthorized);
    }

    record_admin_login_success(state, id).await?;
    Ok(Some(LoginResponse {
        token: super::issue_admin_token(
            state.config.admin_session_ttl,
            &state.config.admin_token_secret,
            id,
        ),
        role: "admin".to_string(),
        requires_password_change: false,
    }))
}

pub(super) async fn user_session_from_headers(
    state: &AppState,
    headers: &HeaderMap,
) -> AppResult<UserSessionState> {
    let token = super::bearer(headers).ok_or(AppError::Unauthorized)?;
    user_session_from_token(state, token).await
}

async fn user_session_from_token(state: &AppState, token: &str) -> AppResult<UserSessionState> {
    let Some(claims) =
        super::validate_user_session_token_claims(token, &state.config.admin_token_secret)
    else {
        return Err(AppError::Unauthorized);
    };

    let row = sqlx::query(
        r#"
        SELECT status, auth_version, (password_changed_at IS NULL) AS requires_password_change
        FROM "user"
        WHERE id = $1
        "#,
    )
    .bind(claims.user_id)
    .fetch_optional(&state.db.pool)
    .await?;
    let Some(row) = row else {
        return Err(AppError::Unauthorized);
    };
    let status: String = row.try_get("status")?;
    if status != "enabled" || row.try_get::<i64, _>("auth_version")? != claims.auth_version {
        return Err(AppError::Unauthorized);
    }
    Ok(UserSessionState {
        user_id: claims.user_id,
        auth_version: claims.auth_version,
        requires_password_change: row.try_get("requires_password_change")?,
    })
}

async fn record_admin_login_failure(state: &AppState, id: DbId) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE admin
        SET failed_login_attempts = failed_login_attempts + 1,
            locked_until = CASE
                WHEN failed_login_attempts + 1 >= $2 THEN now() + $3::interval
                ELSE locked_until
            END,
            updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(MAX_ADMIN_LOGIN_FAILURES)
    .bind(format!("{} seconds", ADMIN_LOGIN_LOCK_TTL.as_secs()))
    .execute(&state.db.pool)
    .await?;
    Ok(())
}

async fn record_admin_login_success(state: &AppState, id: DbId) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE admin
        SET failed_login_attempts = 0,
            locked_until = NULL,
            last_login_at = now(),
            updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(&state.db.pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL"]
    async fn old_sessions_are_revoked_after_password_change_and_reset(pool: sqlx::PgPool) {
        let user_id: DbId = sqlx::query_scalar(
            "INSERT INTO \"user\" (email,password_hash,password_changed_at,user_group_id)
             SELECT 'session@example.com','initial',now(),id FROM user_group WHERE is_default RETURNING id",
        ).fetch_one(&pool).await.unwrap();
        let mut state = crate::app::tests::test_state();
        Arc::get_mut(&mut state).unwrap().db.pool = pool.clone();
        let token = super::super::issue_user_session_token(
            std::time::Duration::from_secs(60),
            &state.config.admin_token_secret,
            user_id,
            0,
        );
        assert!(user_session_from_token(&state, &token).await.is_ok());
        sqlx::query("UPDATE \"user\" SET password_hash='new-password' WHERE id=$1")
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(user_session_from_token(&state, &token).await.is_err());
        let current = super::super::issue_user_session_token(
            std::time::Duration::from_secs(60),
            &state.config.admin_token_secret,
            user_id,
            1,
        );
        assert!(user_session_from_token(&state, &current).await.is_ok());
        let reset = super::super::password_reset::issue(
            &pool,
            "session@example.com",
            std::time::Duration::from_secs(60),
        )
        .await
        .unwrap()
        .unwrap();
        super::super::password_reset::consume(&pool, &reset, "reset-password")
            .await
            .unwrap();
        assert!(user_session_from_token(&state, &current).await.is_err());
        let after_reset = super::super::issue_user_session_token(
            std::time::Duration::from_secs(60),
            &state.config.admin_token_secret,
            user_id,
            2,
        );
        assert!(user_session_from_token(&state, &after_reset).await.is_ok());
    }
}
