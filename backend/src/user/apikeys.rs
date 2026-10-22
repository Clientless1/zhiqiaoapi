use std::sync::Arc;

use axum::{
    extract::{Path, State},
    routing::{get, patch},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::{
    auth::{generate_user_key, key_prefix, UserSessionAuth},
    billing::{account, Billing, CreditAccountId, CreditAccountType, DebitHold},
    cache::InvalidationEvent,
    error::{AppError, AppResult},
    id::DbId,
    policy::{service_mode, ServiceMode},
    project, AppState,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/user/apikeys", get(list_apikeys).post(create_apikey))
        .route(
            "/api/user/apikeys/{id}",
            patch(update_apikey).delete(delete_apikey),
        )
}

#[derive(Debug, Serialize)]
struct UserApiKeyRecord {
    id: DbId,
    user_id: DbId,
    name: String,
    key: String,
    key_prefix: String,
    status: String,
    package_id: Option<DbId>,
    package_name: Option<String>,
    model_limits: Option<Vec<String>>,
    last_active_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
    balance_micros: i64,
    reserved_micros: i64,
    available_micros: i64,
    month_cost_micros: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct CreatedUserApiKey {
    record: UserApiKeyRecord,
    key: String,
}

#[derive(Debug, Deserialize)]
struct CreateApiKeyRequest {
    name: String,
    package_id: DbId,
    models: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct UpdateApiKeyRequest {
    status: Option<String>,
    package_id: Option<DbId>,
    models: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct DeleteApiKeyResponse {
    ok: bool,
}

async fn list_apikeys(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
) -> AppResult<Json<Vec<UserApiKeyRecord>>> {
    let rows = sqlx::query(
        "SELECT uk.id, uk.user_id, uk.name, uk.key_prefix, uk.secret_ciphertext,
                uk.status, uk.package_id, p.name AS package_name, uk.model_limits, uk.last_active_at, uk.expires_at,
                w.balance_micros, w.reserved_micros,
                COALESCE(month_usage.month_cost_micros, 0)::BIGINT AS month_cost_micros,
                uk.created_at, uk.updated_at
         FROM user_key uk
         LEFT JOIN user_package p ON p.id = uk.package_id
         JOIN credit_account w ON w.owner_type = 'user_key' AND w.owner_id = uk.id
         LEFT JOIN (
             SELECT user_key_id,
                    COALESCE(SUM(cost_micros), 0)::BIGINT AS month_cost_micros
             FROM usage_daily
             WHERE user_id = $1
               AND day >= date_trunc('month', now())::date
             GROUP BY user_key_id
         ) month_usage ON month_usage.user_key_id = uk.id
         WHERE uk.user_id = $1
         ORDER BY uk.created_at DESC",
    )
    .bind(auth.user_id)
    .fetch_all(&state.db.pool)
    .await?;

    Ok(Json(
        rows.iter()
            .map(|row| apikey_from_row(&state, row))
            .collect::<Result<_, _>>()?,
    ))
}

async fn create_apikey(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
    Json(req): Json<CreateApiKeyRequest>,
) -> AppResult<Json<CreatedUserApiKey>> {
    if service_mode(&state).await? != ServiceMode::Paid {
        return Err(AppError::BadRequest(
            "default project is only available in paid service mode".to_string(),
        ));
    }
    let name = normalize_api_key_name(&req.name)?;
    let models =
        super::commerce::key_package_scope(&state, auth.user_id, req.package_id, req.models)
            .await?;
    let key = generate_user_key();
    let secret_ciphertext = state.secrets.encrypt(&key)?;
    let mut tx = state.db.pool.begin().await?;
    let project_id = project::default_project_for_user(&mut tx, auth.user_id).await?;
    let row = sqlx::query(
        r#"
        INSERT INTO user_key
            (user_id, project_id, owner_user_id, name, key_prefix, secret_ciphertext, status, expires_at, package_id, model_limits)
        VALUES ($1, $2, $1, $3, $4, $5, 'enabled', NULL, $6, $7)
        RETURNING id
        "#,
    )
    .bind(auth.user_id)
    .bind(project_id)
    .bind(name)
    .bind(key_prefix(&key))
    .bind(secret_ciphertext)
    .bind(req.package_id)
    .bind(models)
    .fetch_one(&mut *tx)
    .await?;
    let user_key_id: DbId = row.try_get("id")?;
    account::create_credit_account(&mut tx, CreditAccountType::UserKey, user_key_id).await?;
    tx.commit().await?;

    Ok(Json(CreatedUserApiKey {
        record: get_apikey(&state, auth.user_id, user_key_id).await?,
        key,
    }))
}

async fn update_apikey(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
    Path(id): Path<DbId>,
    Json(req): Json<UpdateApiKeyRequest>,
) -> AppResult<Json<UserApiKeyRecord>> {
    if let Some(status) = req.status.as_deref() {
        validate_status(status)?;
    }
    if req.models.is_some() && req.package_id.is_none() {
        return Err(AppError::BadRequest(
            "package_id is required to change model scope".into(),
        ));
    }
    let models = if let Some(package_id) = req.package_id {
        Some(
            super::commerce::key_package_scope(&state, auth.user_id, package_id, req.models)
                .await?,
        )
    } else {
        None
    };
    if req.status.as_deref() == Some("disabled") {
        let credit_accounts = user_apikey_credit_accounts(&state, auth.user_id, id).await?;
        for credit_account in credit_accounts {
            recover_hot_credit_account(&state, credit_account).await?;
        }
    }

    let result = sqlx::query(
        "UPDATE user_key
         SET status = COALESCE($3, status), package_id = COALESCE($4, package_id),
             model_limits = COALESCE($5, model_limits), updated_at = now()
         WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(auth.user_id)
    .bind(&req.status)
    .bind(req.package_id)
    .bind(models)
    .execute(&state.db.pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    state
        .cache_invalidator
        .invalidate(&state, InvalidationEvent::UserKey { id })
        .await;
    Ok(Json(get_apikey(&state, auth.user_id, id).await?))
}

async fn delete_apikey(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
    Path(id): Path<DbId>,
) -> AppResult<Json<DeleteApiKeyResponse>> {
    let credit_accounts = user_apikey_credit_accounts(&state, auth.user_id, id).await?;
    for credit_account in credit_accounts {
        recover_hot_credit_account(&state, credit_account).await?;
    }

    let result = sqlx::query("DELETE FROM user_key WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(auth.user_id)
        .execute(&state.db.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    state
        .cache_invalidator
        .invalidate(&state, InvalidationEvent::UserKey { id })
        .await;
    Ok(Json(DeleteApiKeyResponse { ok: true }))
}

async fn get_apikey(state: &AppState, user_id: DbId, id: DbId) -> AppResult<UserApiKeyRecord> {
    let row = sqlx::query(
        "SELECT uk.id, uk.user_id, uk.name, uk.key_prefix, uk.secret_ciphertext,
                uk.status, uk.package_id, p.name AS package_name, uk.model_limits, uk.last_active_at, uk.expires_at,
                w.balance_micros, w.reserved_micros,
                COALESCE(month_usage.month_cost_micros, 0)::BIGINT AS month_cost_micros,
                uk.created_at, uk.updated_at
         FROM user_key uk
         LEFT JOIN user_package p ON p.id = uk.package_id
         JOIN credit_account w ON w.owner_type = 'user_key' AND w.owner_id = uk.id
         LEFT JOIN (
             SELECT user_key_id,
                    COALESCE(SUM(cost_micros), 0)::BIGINT AS month_cost_micros
             FROM usage_daily
             WHERE user_id = $2
               AND day >= date_trunc('month', now())::date
             GROUP BY user_key_id
         ) month_usage ON month_usage.user_key_id = uk.id
         WHERE uk.id = $1 AND uk.user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_one(&state.db.pool)
    .await?;

    apikey_from_row(state, &row)
}

async fn user_apikey_credit_accounts(
    state: &AppState,
    user_id: DbId,
    id: DbId,
) -> AppResult<Vec<CreditAccountId>> {
    let rows = sqlx::query(
        "SELECT w.id
         FROM user_key uk
         JOIN credit_account w ON w.owner_type = 'user_key' AND w.owner_id = uk.id
         WHERE uk.id = $1 AND uk.user_id = $2
         UNION ALL
         SELECT w.id
         FROM user_key uk
         JOIN user_key_model ukm ON ukm.user_key_id = uk.id
         JOIN credit_account w ON w.owner_type = 'user_key_model' AND w.owner_id = ukm.id
         WHERE uk.id = $1 AND uk.user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .fetch_all(&state.db.pool)
    .await?;
    if rows.is_empty() {
        return Err(AppError::NotFound);
    }
    rows.iter()
        .map(|row| Ok(CreditAccountId::new(row.try_get("id")?)))
        .collect()
}

async fn recover_hot_credit_account(
    state: &AppState,
    credit_account: CreditAccountId,
) -> AppResult<()> {
    let recovered = state
        .billing
        .drain_hot_credit_account(&credit_account)
        .await?;
    let mut tx = state.db.pool.begin().await?;
    Billing::release_hold_in_transaction(&mut tx, &DebitHold::new(0, recovered, true)).await?;
    tx.commit().await?;
    Ok(())
}

fn apikey_from_row(state: &AppState, row: &sqlx::postgres::PgRow) -> AppResult<UserApiKeyRecord> {
    let id = row.try_get("id")?;
    let secret_ciphertext: String = row.try_get("secret_ciphertext")?;
    Ok(UserApiKeyRecord {
        id,
        user_id: row.try_get("user_id")?,
        name: row.try_get("name")?,
        key: state.secrets.plaintext(id, &secret_ciphertext)?,
        key_prefix: row.try_get("key_prefix")?,
        status: row.try_get("status")?,
        package_id: row.try_get("package_id")?,
        package_name: row.try_get("package_name")?,
        model_limits: row.try_get("model_limits")?,
        last_active_at: row.try_get("last_active_at")?,
        expires_at: row.try_get("expires_at")?,
        balance_micros: row.try_get("balance_micros")?,
        reserved_micros: row.try_get("reserved_micros")?,
        available_micros: row.try_get::<i64, _>("balance_micros")?
            - row.try_get::<i64, _>("reserved_micros")?,
        month_cost_micros: row.try_get("month_cost_micros")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn normalize_api_key_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("api key name is required".to_string()));
    }
    if name.chars().count() > 80 {
        return Err(AppError::BadRequest(
            "api key name must be 80 characters or fewer".to_string(),
        ));
    }
    Ok(name.to_string())
}

fn validate_status(status: &str) -> AppResult<()> {
    match status {
        "enabled" | "disabled" => Ok(()),
        _ => Err(AppError::BadRequest("invalid api key status".to_string())),
    }
}
