use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    routing::post,
    Json, Router,
};
use rand::RngExt;
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Row, Transaction};

mod oauth;
mod password_reset;
mod rate_limit;
mod session;

pub use crate::core::tokens::*;
use crate::{
    email::{smtp_config_error, smtp_test_error, EmailLocale},
    error::{AppError, AppResult},
    id::DbId,
    policy::{registration_policy, ServiceMode},
    project, AppState,
};
pub use rate_limit::AuthRateLimiter;
pub use session::UserSessionAuth;

const PASSWORD_RESET_TTL: std::time::Duration = std::time::Duration::from_secs(30 * 60);
const LOGIN_VERIFICATION_TTL: std::time::Duration = std::time::Duration::from_secs(10 * 60);
const MIN_USER_PASSWORD_LEN: usize = 8;
const SYSTEM_APPS_USER_EMAIL: &str = "system-apps@zhiqiaoapi.local";
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .merge(oauth::router())
        .route("/api/login", post(login))
        .route("/api/email-login", post(email_login))
        .route("/api/register", post(register))
        .route("/api/me", axum::routing::get(session::me))
        .route(
            "/api/login-verification-codes",
            post(request_login_verification_code),
        )
        .route("/api/password-reset-requests", post(request_password_reset))
        .route("/api/password-reset", post(reset_password))
        .route("/api/user/password", post(update_user_password))
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct EmailLoginRequest {
    email: String,
    verification_code: String,
}

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    email: String,
    password: String,
    verification_code: String,
    #[serde(default)]
    accept_terms: bool,
}

#[derive(Debug, Deserialize)]
struct PasswordResetRequest {
    email: String,
    locale: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LoginVerificationCodeRequest {
    email: String,
    locale: Option<String>,
    #[serde(default)]
    purpose: CodePurpose,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum CodePurpose {
    #[default]
    Login,
    Register,
}

impl CodePurpose {
    fn hash_context(self, email: &str) -> String {
        format!(
            "{}:{email}",
            match self {
                Self::Login => "login",
                Self::Register => "register",
            }
        )
    }
}

#[derive(Debug, Deserialize)]
struct ResetPasswordRequest {
    token: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct UpdateUserPasswordRequest {
    current_password: String,
    new_password: String,
}

#[derive(Debug, Serialize)]
struct OkResponse {
    ok: bool,
}

fn login_response(state: &AppState, user: UserLogin) -> session::LoginResponse {
    session::LoginResponse {
        token: issue_user_session_token(
            state.config.admin_session_ttl,
            &state.config.admin_token_secret,
            user.user_id,
            user.auth_version,
        ),
        role: "user".to_string(),
        requires_password_change: user.requires_password_change,
    }
}

async fn login(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<session::LoginResponse>> {
    let client_key = client_rate_key(&headers, Some(addr.ip()), state.config.trust_proxy_headers);
    state
        .auth_rate_limiter
        .check_password_login(&req.username, &client_key)
        .await?;
    if let Some(response) = session::login_admin(&state, &req.username, &req.password).await? {
        return Ok(Json(response));
    }
    let email = normalize_email(&req.username)?;
    let user = password_login(&state, &email, &req.password).await?;
    Ok(Json(login_response(&state, user)))
}

async fn email_login(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<EmailLoginRequest>,
) -> AppResult<Json<session::LoginResponse>> {
    let email = normalize_email(&req.email)?;
    let client_key = client_rate_key(&headers, Some(addr.ip()), state.config.trust_proxy_headers);
    state
        .auth_rate_limiter
        .check_password_login(&email, &client_key)
        .await?;
    state
        .auth_rate_limiter
        .check_login_verification_attempt(&email)
        .await?;
    let mut tx = state.db.pool.begin().await?;
    lock_user_login(&mut tx, &email).await?;
    let row = sqlx::query(
        r#"SELECT id, auth_version, status, (password_changed_at IS NULL) AS requires_password_change
           FROM "user" WHERE email=$1 FOR UPDATE"#,
    ).bind(&email).fetch_optional(&mut *tx).await?.ok_or(AppError::Unauthorized)?;
    ensure_enabled(&row.try_get::<String, _>("status")?)?;
    if is_system_apps_email(&email) {
        return Err(AppError::Unauthorized);
    }
    consume_login_verification_code(
        &mut tx,
        &email,
        &req.verification_code,
        CodePurpose::Login,
        &state,
    )
    .await?;
    let user = UserLogin {
        user_id: row.try_get("id")?,
        auth_version: row.try_get("auth_version")?,
        requires_password_change: row.try_get("requires_password_change")?,
    };
    tx.commit().await?;
    Ok(Json(login_response(&state, user)))
}

async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> AppResult<Json<session::LoginResponse>> {
    if !req.accept_terms {
        return Err(AppError::BadRequestWithCode {
            code: "terms_required",
            message: "accept the user agreement and privacy policy",
        });
    }
    let email = normalize_email(&req.email)?;
    if is_system_apps_email(&email) {
        return Err(AppError::Unauthorized);
    }
    validate_user_password_input(&req.password)?;
    let (service_mode, enabled) = registration_policy(&state).await?;
    if !enabled {
        return Err(AppError::BadRequestWithCode {
            code: "registration_closed",
            message: "registration is closed",
        });
    }
    state
        .auth_rate_limiter
        .check_login_verification_attempt(&email)
        .await?;
    let mut tx = state.db.pool.begin().await?;
    lock_user_login(&mut tx, &email).await?;
    let exists: bool = sqlx::query_scalar(r#"SELECT EXISTS(SELECT 1 FROM "user" WHERE email=$1)"#)
        .bind(&email)
        .fetch_one(&mut *tx)
        .await?;
    if exists {
        return Err(AppError::ConflictWithCode {
            code: "account_already_exists",
            message: "account already exists, please sign in",
        });
    }
    consume_login_verification_code(
        &mut tx,
        &email,
        &req.verification_code,
        CodePurpose::Register,
        &state,
    )
    .await?;
    let user_id: DbId = sqlx::query_scalar(
        r#"INSERT INTO "user" (email,status,user_group_id,password_hash,password_changed_at)
           VALUES ($1,$2,(SELECT id FROM user_group WHERE is_default=TRUE),$3,now()) RETURNING id"#,
    )
    .bind(&email)
    .bind(if service_mode == ServiceMode::Internal {
        "pending"
    } else {
        "enabled"
    })
    .bind(hash_user_password(
        &req.password,
        &state.config.admin_token_secret,
    ))
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO user_agreement_acceptance (user_id,version,method) VALUES ($1,'2026-11-07','email')")
        .bind(user_id).execute(&mut *tx).await?;
    project::ensure_default_project_for_user(&mut tx, user_id).await?;
    tx.commit().await?;
    ensure_enabled(if service_mode == ServiceMode::Internal {
        "pending"
    } else {
        "enabled"
    })?;
    Ok(Json(login_response(
        &state,
        UserLogin {
            user_id,
            auth_version: 0,
            requires_password_change: false,
        },
    )))
}

fn ensure_enabled(status: &str) -> AppResult<()> {
    match status {
        "enabled" => Ok(()),
        "pending" => Err(AppError::BadRequestWithCode {
            code: "account_pending_approval",
            message: "account pending approval",
        }),
        _ => Err(AppError::Unauthorized),
    }
}

async fn request_login_verification_code(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<LoginVerificationCodeRequest>,
) -> AppResult<Json<OkResponse>> {
    let email = normalize_email(&req.email)?;
    let client_key = client_rate_key(&headers, Some(addr.ip()), state.config.trust_proxy_headers);
    state
        .auth_rate_limiter
        .check_login_verification_request(&email, &client_key)
        .await?;
    if is_system_apps_email(&email) {
        return Ok(Json(OkResponse { ok: true }));
    }
    let exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(SELECT 1 FROM "user" WHERE email=$1 AND status='enabled')"#,
    )
    .bind(&email)
    .fetch_one(&state.db.pool)
    .await?;
    let should_send = match req.purpose {
        CodePurpose::Login => exists,
        CodePurpose::Register => {
            let (_, enabled) = registration_policy(&state).await?;
            if !enabled {
                return Err(AppError::BadRequestWithCode {
                    code: "registration_closed",
                    message: "registration is closed",
                });
            }
            let any_exists: bool =
                sqlx::query_scalar(r#"SELECT EXISTS(SELECT 1 FROM "user" WHERE email=$1)"#)
                    .bind(&email)
                    .fetch_one(&state.db.pool)
                    .await?;
            !any_exists
        }
    };
    if should_send {
        let code = generate_login_verification_code();
        let code_hash = hash_email_verification_code(
            &req.purpose.hash_context(&email),
            &code,
            &state.config.admin_token_secret,
        );
        // Only the latest code for this purpose remains valid. Other purposes
        // use a different keyed hash context and cannot authenticate this flow.
        let mut tx = state.db.pool.begin().await?;
        lock_user_login(&mut tx, &email).await?;
        sqlx::query(
            "UPDATE user_code SET consumed_at=now() WHERE email=$1 AND consumed_at IS NULL",
        )
        .bind(&email)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO user_code (email,code_hash,expires_at) VALUES ($1,$2,now()+$3::interval)",
        )
        .bind(&email)
        .bind(&code_hash)
        .bind(format!("{} seconds", LOGIN_VERIFICATION_TTL.as_secs()))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        state
            .email
            .send_login_verification_code(
                &email,
                &code,
                EmailLocale::from_public_locale(req.locale.as_deref()),
            )
            .await
            .map_err(email_error)?;
    }

    Ok(Json(OkResponse { ok: true }))
}

async fn request_password_reset(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<PasswordResetRequest>,
) -> AppResult<Json<OkResponse>> {
    let email = normalize_email(&req.email)?;
    let client_key = client_rate_key(&headers, Some(addr.ip()), state.config.trust_proxy_headers);
    state
        .auth_rate_limiter
        .check_password_reset_request(&email, &client_key)
        .await?;
    if is_system_apps_email(&email) {
        return Ok(Json(OkResponse { ok: true }));
    }
    if let Some(token) = password_reset::issue(&state.db.pool, &email, PASSWORD_RESET_TTL).await? {
        let reset_url = reset_url_from_config(state.config.public_base_url.as_deref(), &token)?;
        state
            .email
            .send_password_reset(
                &email,
                &reset_url,
                EmailLocale::from_public_locale(req.locale.as_deref()),
            )
            .await
            .map_err(email_error)?;
    }

    Ok(Json(OkResponse { ok: true }))
}

async fn reset_password(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<ResetPasswordRequest>,
) -> AppResult<Json<OkResponse>> {
    let client_key = client_rate_key(&headers, Some(addr.ip()), state.config.trust_proxy_headers);
    state
        .auth_rate_limiter
        .check_password_reset_attempt(&req.token, &client_key)
        .await?;
    validate_user_password_input(&req.password)?;

    let password_hash = hash_user_password(&req.password, &state.config.admin_token_secret);
    password_reset::consume(&state.db.pool, &req.token, &password_hash).await?;

    Ok(Json(OkResponse { ok: true }))
}

async fn update_user_password(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<UpdateUserPasswordRequest>,
) -> AppResult<Json<OkResponse>> {
    if req.current_password.is_empty() {
        return Err(AppError::BadRequestWithCode {
            code: "current_password_required",
            message: "current password is required",
        });
    }
    validate_user_password_input(&req.new_password)?;

    let session = session::user_session_from_headers(&state, &headers).await?;
    let row = sqlx::query(
        r#"
        SELECT password_hash
        FROM "user"
        WHERE id = $1 AND status = 'enabled'
        "#,
    )
    .bind(session.user_id)
    .fetch_optional(&state.db.pool)
    .await?;
    let Some(row) = row else {
        return Err(AppError::Unauthorized);
    };
    let password_hash: Option<String> = row.try_get("password_hash")?;
    let Some(current_hash) = password_hash else {
        return Err(AppError::Unauthorized);
    };
    if !verify_user_password(
        &req.current_password,
        &state.config.admin_token_secret,
        &current_hash,
    ) {
        return Err(AppError::BadRequestWithCode {
            code: "current_password_incorrect",
            message: "current password is incorrect",
        });
    }
    if verify_user_password(
        &req.new_password,
        &state.config.admin_token_secret,
        &current_hash,
    ) {
        return Err(AppError::BadRequestWithCode {
            code: "password_same_as_current",
            message: "new password cannot be the same as the current password",
        });
    }

    let next_hash = hash_user_password(&req.new_password, &state.config.admin_token_secret);
    let result = sqlx::query(
        r#"
        UPDATE "user"
        SET password_hash = $2,
            password_changed_at = now(),
            updated_at = now()
        WHERE id = $1 AND auth_version = $3 AND password_hash = $4
        "#,
    )
    .bind(session.user_id)
    .bind(next_hash)
    .bind(session.auth_version)
    .bind(current_hash)
    .execute(&state.db.pool)
    .await?;

    if result.rows_affected() != 1 {
        return Err(AppError::Unauthorized);
    }
    Ok(Json(OkResponse { ok: true }))
}

#[derive(Debug, Clone, Copy)]
struct UserLogin {
    user_id: DbId,
    auth_version: i64,
    requires_password_change: bool,
}

async fn password_login(state: &AppState, email: &str, password: &str) -> AppResult<UserLogin> {
    if is_system_apps_email(email) {
        return Err(AppError::Unauthorized);
    }
    if password.is_empty() || password.len() > 1024 {
        return Err(AppError::Unauthorized);
    }
    let row = sqlx::query(
        r#"SELECT id,status,password_hash,auth_version,(password_changed_at IS NULL) AS requires_password_change
           FROM "user" WHERE email=$1"#,
    ).bind(email).fetch_optional(&state.db.pool).await?.ok_or(AppError::Unauthorized)?;
    // A passwordless account must verify its email or provider identity before
    // setting a password; never adopt an arbitrary first login password.
    let stored_hash: Option<String> = row.try_get("password_hash")?;
    let Some(stored_hash) = stored_hash else {
        return Err(AppError::Unauthorized);
    };
    if !verify_user_password(password, &state.config.admin_token_secret, &stored_hash) {
        return Err(AppError::Unauthorized);
    }
    ensure_enabled(&row.try_get::<String, _>("status")?)?;
    Ok(UserLogin {
        user_id: row.try_get("id")?,
        auth_version: row.try_get("auth_version")?,
        requires_password_change: row.try_get("requires_password_change")?,
    })
}

async fn consume_login_verification_code(
    tx: &mut Transaction<'_, Postgres>,
    email: &str,
    code: &str,
    purpose: CodePurpose,
    state: &AppState,
) -> AppResult<()> {
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(AppError::BadRequestWithCode {
            code: "invalid_verification_code",
            message: "invalid verification code",
        });
    }

    let code_hash = hash_email_verification_code(
        &purpose.hash_context(email),
        code,
        &state.config.admin_token_secret,
    );
    let row = sqlx::query(
        r#"
        SELECT id
        FROM user_code
        WHERE email = $1
            AND code_hash = $2
            AND consumed_at IS NULL
            AND expires_at > now()
        ORDER BY created_at DESC
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(email)
    .bind(&code_hash)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(row) = row else {
        return Err(AppError::BadRequestWithCode {
            code: "invalid_verification_code",
            message: "invalid verification code",
        });
    };
    let code_id: DbId = row.try_get("id")?;

    sqlx::query(
        r#"
        UPDATE user_code
        SET consumed_at = now()
        WHERE id = $1
        "#,
    )
    .bind(code_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn normalize_email(email: &str) -> AppResult<String> {
    let email = email.trim().to_ascii_lowercase();
    let has_single_at = email.matches('@').count() == 1;
    if email.is_empty()
        || email.len() > 254
        || !has_single_at
        || email.starts_with('@')
        || email.ends_with('@')
    {
        return Err(AppError::BadRequestWithCode {
            code: "invalid_email",
            message: "invalid email",
        });
    }
    Ok(email)
}

fn is_system_apps_email(email: &str) -> bool {
    email.eq_ignore_ascii_case(SYSTEM_APPS_USER_EMAIL)
}

async fn lock_user_login(tx: &mut Transaction<'_, Postgres>, email: &str) -> AppResult<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(email)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn generate_login_verification_code() -> String {
    format!("{:06}", rand::rng().random_range(0..1_000_000))
}

pub(crate) fn validate_user_password_input(password: &str) -> AppResult<()> {
    if password.is_empty() {
        return Err(AppError::BadRequestWithCode {
            code: "password_required",
            message: "password is required",
        });
    }
    if password.len() > 1024 {
        return Err(AppError::BadRequestWithCode {
            code: "password_too_long",
            message: "password is too long",
        });
    }
    if password.chars().count() < MIN_USER_PASSWORD_LEN {
        return Err(AppError::BadRequestWithCode {
            code: "password_min_length",
            message: "password must be at least 8 characters",
        });
    }
    Ok(())
}

fn client_rate_key(
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
    trust_proxy_headers: bool,
) -> String {
    if trust_proxy_headers {
        if let Some(ip) = forwarded_client_ip(headers) {
            return ip.to_string();
        }
    }
    peer_ip.map_or_else(|| "unknown".to_string(), |ip| ip.to_string())
}

fn email_error(err: anyhow::Error) -> AppError {
    tracing::warn!(reason = %format!("{err:#}"), "Authentication email delivery failed");
    smtp_config_error(&err).map_or_else(
        || {
            let (code, message) = smtp_test_error(&err);
            AppError::UpstreamUnavailableWithCode { code, message }
        },
        |(code, message)| AppError::BadRequestWithCode { code, message },
    )
}

fn forwarded_client_ip(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse().ok())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .and_then(|value| value.parse().ok())
        })
}

fn reset_url_from_config(public_base_url: Option<&str>, token: &str) -> AppResult<String> {
    let base = public_base_url.ok_or_else(|| {
        AppError::BadRequest("PUBLIC_BASE_URL is required for password reset".to_string())
    })?;
    Ok(format!("{base}/reset-password?token={token}"))
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderMap;

    use super::*;

    fn state_with_pool(pool: sqlx::PgPool) -> Arc<AppState> {
        let mut state = crate::app::tests::test_state();
        Arc::get_mut(&mut state).unwrap().db.pool = pool;
        state
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL"]
    async fn password_login_requires_existing_account_and_never_adopts_a_password(
        pool: sqlx::PgPool,
    ) {
        let state = state_with_pool(pool.clone());
        assert!(password_login(&state, "missing@example.com", "password123")
            .await
            .is_err());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM \"user\"")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        sqlx::query("INSERT INTO \"user\"(email,user_group_id) SELECT 'existing@example.com',id FROM user_group WHERE is_default")
            .execute(&pool).await.unwrap();
        assert!(
            password_login(&state, "existing@example.com", "password123")
                .await
                .is_err()
        );
        let stored: Option<String> = sqlx::query_scalar(
            "SELECT password_hash FROM \"user\" WHERE email='existing@example.com'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(stored.is_none());
        sqlx::query("UPDATE \"user\" SET password_hash=$1,password_changed_at=now() WHERE email='existing@example.com'")
            .bind(hash_user_password("password123", &state.config.admin_token_secret)).execute(&pool).await.unwrap();
        assert!(
            password_login(&state, "existing@example.com", "wrong-password")
                .await
                .is_err()
        );
        assert!(
            password_login(&state, "existing@example.com", "password123")
                .await
                .is_ok()
        );
        sqlx::query("UPDATE \"user\" SET status='disabled' WHERE email='existing@example.com'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            password_login(&state, "existing@example.com", "password123")
                .await
                .is_err()
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL"]
    async fn verification_codes_enforce_purpose_expiry_and_single_use(pool: sqlx::PgPool) {
        let state = state_with_pool(pool.clone());
        let email = "code@example.com";
        let code = "123456";
        let digest = hash_email_verification_code(
            &CodePurpose::Register.hash_context(email),
            code,
            &state.config.admin_token_secret,
        );
        sqlx::query("INSERT INTO user_code(email,code_hash,expires_at) VALUES($1,$2,now()+interval '1 minute')")
            .bind(email).bind(&digest).execute(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        assert!(
            consume_login_verification_code(&mut tx, email, code, CodePurpose::Login, &state)
                .await
                .is_err()
        );
        assert!(consume_login_verification_code(
            &mut tx,
            email,
            "000000",
            CodePurpose::Register,
            &state
        )
        .await
        .is_err());
        consume_login_verification_code(&mut tx, email, code, CodePurpose::Register, &state)
            .await
            .unwrap();
        assert!(consume_login_verification_code(
            &mut tx,
            email,
            code,
            CodePurpose::Register,
            &state
        )
        .await
        .is_err());
        tx.commit().await.unwrap();
        sqlx::query("INSERT INTO user_code(email,code_hash,expires_at) VALUES($1,$2,now()-interval '1 second')")
            .bind(email).bind(&digest).execute(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        assert!(consume_login_verification_code(
            &mut tx,
            email,
            code,
            CodePurpose::Register,
            &state
        )
        .await
        .is_err());
    }

    #[test]
    fn reset_url_uses_configured_public_base_url_only() {
        let url = reset_url_from_config(Some("https://app.example.com"), "reset-token").unwrap();

        assert_eq!(
            url,
            "https://app.example.com/reset-password?token=reset-token"
        );
        assert!(reset_url_from_config(None, "reset-token").is_err());
    }

    #[test]
    fn system_apps_email_is_reserved_case_insensitively() {
        assert!(is_system_apps_email("system-apps@zhiqiaoapi.local"));
        assert!(is_system_apps_email("System-Apps@zhiqiaoapi.Local"));
        assert!(!is_system_apps_email("user@example.com"));
    }

    #[test]
    fn client_rate_key_prefers_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "192.0.2.10".parse().unwrap());
        headers.insert(
            "x-forwarded-for",
            "198.51.100.20, 203.0.113.30".parse().unwrap(),
        );

        assert_eq!(client_rate_key(&headers, None, true), "198.51.100.20");
    }

    #[test]
    fn client_rate_key_uses_real_ip_without_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "192.0.2.10".parse().unwrap());

        assert_eq!(client_rate_key(&headers, None, true), "192.0.2.10");
    }

    #[test]
    fn client_rate_key_uses_unknown_without_proxy_headers() {
        assert_eq!(client_rate_key(&HeaderMap::new(), None, true), "unknown");
    }

    #[test]
    fn client_rate_key_ignores_proxy_headers_when_not_trusted() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "198.51.100.20, 203.0.113.30".parse().unwrap(),
        );

        assert_eq!(
            client_rate_key(&headers, Some("203.0.113.99".parse().unwrap()), false),
            "203.0.113.99"
        );
        assert_eq!(client_rate_key(&headers, None, false), "unknown");
    }

    #[test]
    fn client_rate_key_prefers_peer_ip_when_headers_missing() {
        assert_eq!(
            client_rate_key(
                &HeaderMap::new(),
                Some("203.0.113.99".parse().unwrap()),
                true
            ),
            "203.0.113.99"
        );
    }
}
