use std::{collections::BTreeSet, sync::Arc};

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::{
    auth::{AdminAuth, UserSessionAuth},
    billing::{account, CreditAccountId, CreditAccountType},
    error::{AppError, AppResult},
    id::DbId,
    project, AppState,
};

pub(crate) fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/user/packages", get(packages))
        .route("/api/user/packages/{id}/purchase", post(purchase))
        .route(
            "/api/user/package-orders/by-request/{request_id}",
            get(purchase_receipt),
        )
        .route("/api/user/coupons", get(coupons))
        .route("/api/user/coupons/claim", post(claim_coupon))
        .route("/api/user/redemptions", get(redemptions).post(redeem))
        .route("/api/user/orders", get(orders))
        .route("/api/user/ledger", get(user_ledger))
        .route("/api/user/notifications", get(notifications))
        .route(
            "/api/user/notifications/read-all",
            post(read_all_notifications),
        )
        .route("/api/user/notifications/{id}/read", post(read_notification))
        .route(
            "/api/admin/packages",
            get(admin_packages).post(save_package),
        )
        .route(
            "/api/admin/packages/{id}",
            axum::routing::put(update_package),
        )
        .route("/api/admin/coupons", get(admin_coupons).post(create_coupon))
        .route(
            "/api/admin/redemption-codes",
            get(admin_redemptions).post(create_redemption),
        )
        .route(
            "/api/admin/notifications",
            get(admin_notifications).post(create_notification),
        )
        .route(
            "/api/admin/notifications/{id}",
            axum::routing::delete(delete_notification),
        )
}

#[derive(Debug, Serialize)]
pub(crate) struct PackageRecord {
    pub id: DbId,
    pub name: String,
    pub description: String,
    pub family: String,
    pub models: Vec<String>,
    pub configured_models: Vec<String>,
    pub price_micros: i64,
    pub duration_days: i32,
    pub enabled: bool,
    pub service_enabled: bool,
    pub active: bool,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct PageQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub order_no: Option<i64>,
}

impl PageQuery {
    pub(crate) fn bounds(&self) -> AppResult<(i64, i64, i64)> {
        let page = self.page.unwrap_or(1);
        let page_size = self.page_size.unwrap_or(20);
        if page < 1 || !(1..=100).contains(&page_size) {
            return Err(AppError::BadRequest(
                "分页参数无效，page 至少为 1，page_size 为 1–100".into(),
            ));
        }
        let offset = (page - 1)
            .checked_mul(page_size)
            .ok_or_else(|| AppError::BadRequest("分页参数过大".into()))?;
        Ok((page, page_size, offset))
    }

    fn kind<'a>(&'a self, accepted: &[&str]) -> AppResult<&'a str> {
        let kind = self.kind.as_deref().unwrap_or("all");
        if kind != "all" && !accepted.contains(&kind) {
            return Err(AppError::BadRequest("筛选类型无效".into()));
        }
        Ok(kind)
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

async fn list_packages(pool: &PgPool, user_id: Option<DbId>) -> AppResult<Vec<PackageRecord>> {
    let available = crate::catalog::available_models(pool).await?;
    let rows = sqlx::query(
        "SELECT p.*, s.expires_at, COALESCE(s.requires_payment,FALSE) AS requires_payment FROM user_package p
         LEFT JOIN user_package_subscription s ON s.package_id = p.id AND s.user_id = $1
         WHERE $1::BIGINT IS NULL OR p.enabled OR s.expires_at > now() ORDER BY p.id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(|row| {
            let family: String = row.try_get("family")?;
            let configured: Vec<String> = row.try_get("models")?;
            let models = available
                .iter()
                .filter(|(model, model_family)| {
                    if configured.is_empty() {
                        family != "custom" && *model_family == family
                    } else {
                        configured.contains(model)
                    }
                })
                .map(|(model, _)| model.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let price_micros = row.try_get("price_micros")?;
            let expires_at: Option<DateTime<Utc>> = row.try_get("expires_at")?;
            Ok(PackageRecord {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
                description: row.try_get("description")?,
                family,
                models,
                configured_models: configured,
                price_micros,
                duration_days: row.try_get("duration_days")?,
                enabled: row.try_get("enabled")?,
                service_enabled: row.try_get("service_enabled")?,
                active: row.try_get::<bool, _>("service_enabled")?
                    && ((price_micros == 0 && !row.try_get::<bool, _>("requires_payment")?)
                        || expires_at.is_some_and(|date| date > Utc::now())),
                expires_at,
            })
        })
        .collect()
}

async fn packages(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
) -> AppResult<Json<Vec<PackageRecord>>> {
    Ok(Json(
        list_packages(&state.db.pool, Some(auth.user_id)).await?,
    ))
}

pub(crate) async fn key_package_scope(
    state: &AppState,
    user_id: DbId,
    package_id: DbId,
    requested: Option<Vec<String>>,
) -> AppResult<Vec<String>> {
    let package = list_packages(&state.db.pool, Some(user_id))
        .await?
        .into_iter()
        .find(|p| p.id == package_id)
        .ok_or(AppError::NotFound)?;
    if !package.service_enabled {
        return Err(AppError::Forbidden("此套餐已停止服务".into()));
    }
    if !package.active {
        return Err(AppError::Forbidden("请先购买或兑换此套餐".into()));
    }
    let models = normalized_models(requested.unwrap_or_else(|| package.models.clone()))?;
    if models.is_empty() {
        return Err(AppError::BadRequest(
            "请先配置此套餐的上游模型，再创建密钥".into(),
        ));
    }
    if models.iter().any(|model| !package.models.contains(model)) {
        return Err(AppError::Forbidden("所选模型不属于当前套餐".into()));
    }
    Ok(models)
}

/// Check current membership only within the key's immutable creation snapshot.
/// Relay selection separately validates endpoint health and prices; this keeps
/// revocation immediate without loading the entire public catalog per request.
pub(crate) async fn current_package_models(
    pool: &PgPool,
    package_id: DbId,
    snapshot: &[String],
) -> AppResult<Vec<String>> {
    let row = sqlx::query("SELECT family,models FROM user_package WHERE id=$1 AND service_enabled")
        .bind(package_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    let family: String = row.try_get("family")?;
    let configured: Vec<String> = row.try_get("models")?;
    if !configured.is_empty() {
        return Ok(snapshot
            .iter()
            .filter(|model| configured.contains(model))
            .cloned()
            .collect());
    }
    if family == "custom" {
        return Ok(Vec::new());
    }
    let metadata = sqlx::query(
        "SELECT model,billing_meter,capabilities FROM provider_model WHERE model=ANY($1)",
    )
    .bind(snapshot)
    .fetch_all(pool)
    .await?;
    let mut allowed = BTreeSet::new();
    let mut known = BTreeSet::new();
    for row in metadata {
        let model: String = row.try_get("model")?;
        known.insert(model.clone());
        if crate::catalog::model_family(
            &model,
            &row.try_get::<String, _>("billing_meter")?,
            &row.try_get::<Value, _>("capabilities")?,
        ) == family
        {
            allowed.insert(model);
        }
    }
    for model in snapshot {
        if !known.contains(model)
            && crate::catalog::model_family(model, "token", &json!({})) == family
        {
            allowed.insert(model.clone());
        }
    }
    Ok(snapshot
        .iter()
        .filter(|model| allowed.contains(*model))
        .cloned()
        .collect())
}

fn normalized_models(models: Vec<String>) -> AppResult<Vec<String>> {
    if models.len() > 1000 {
        return Err(AppError::BadRequest("too many models".into()));
    }
    models
        .into_iter()
        .map(|s| text(&s, "model", 200))
        .collect::<AppResult<BTreeSet<_>>>()
        .map(|set| set.into_iter().collect())
}

#[derive(Deserialize)]
struct PurchaseRequest {
    request_id: Uuid,
    coupon_id: Option<DbId>,
}

async fn purchase(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
    Path(id): Path<DbId>,
    Json(req): Json<PurchaseRequest>,
) -> AppResult<Json<Value>> {
    let mut tx = state.db.pool.begin().await?;
    // Serialize retrying requests before any coupon/wallet/subscription change.
    // The unique constraint is a second guard; a hash collision only serializes
    // unrelated requests and cannot authorize access to another user's receipt.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!(
            "package-purchase:{}:{}",
            auth.user_id, req.request_id
        ))
        .execute(&mut *tx)
        .await?;
    if let Some(existing) = sqlx::query("SELECT id,package_id,coupon_id,amount_micros FROM user_package_order WHERE user_id=$1 AND request_id=$2")
        .bind(auth.user_id).bind(req.request_id).fetch_optional(&mut *tx).await? {
        if existing.try_get::<DbId,_>("package_id")? != id || existing.try_get::<Option<DbId>,_>("coupon_id")? != req.coupon_id {
            return Err(AppError::Conflict("购买请求已提交，请刷新订单后重新操作".into()));
        }
        let response=json!({"ok":true,"order_id":existing.try_get::<Uuid,_>("id")?,"amount_micros":existing.try_get::<i64,_>("amount_micros")?});
        tx.commit().await?;
        return Ok(Json(response));
    }
    let row = sqlx::query("SELECT name, price_micros, duration_days FROM user_package WHERE id = $1 AND enabled AND service_enabled FOR SHARE")
        .bind(id).fetch_optional(&mut *tx).await?.ok_or(AppError::NotFound)?;
    let price: i64 = row.try_get("price_micros")?;
    let discount = if let Some(coupon_id) = req.coupon_id {
        let coupon = sqlx::query(
            "SELECT c.amount_micros, c.min_spend_micros, c.expires_at, c.enabled, uc.used_at
             FROM user_coupon uc JOIN user_coupon_campaign c ON c.id = uc.campaign_id
             WHERE uc.id = $1 AND uc.user_id = $2 FOR UPDATE OF uc",
        )
        .bind(coupon_id)
        .bind(auth.user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;
        coupon_discount(
            price,
            coupon.try_get("amount_micros")?,
            coupon.try_get("min_spend_micros")?,
            coupon.try_get::<Option<DateTime<Utc>>, _>("expires_at")?,
            coupon
                .try_get::<Option<DateTime<Utc>>, _>("used_at")?
                .is_some(),
            coupon.try_get("enabled")?,
        )?
    } else {
        0
    };
    let amount = price - discount;
    let credit_account = user_credit_account(&mut tx, auth.user_id).await?;
    let order_id = Uuid::new_v4();
    if amount > 0 {
        let balance = account::adjust_balance(&mut tx, &credit_account, -amount).await?;
        ledger(
            &mut tx,
            &credit_account,
            -amount,
            balance,
            "adjustment",
            order_id,
            json!({"source":"package_purchase", "package_id":id}),
        )
        .await?;
    }
    if let Some(coupon_id) = req.coupon_id {
        sqlx::query("UPDATE user_coupon SET used_at = now() WHERE id = $1")
            .bind(coupon_id)
            .execute(&mut *tx)
            .await?;
    }
    let name: String = row.try_get("name")?;
    sqlx::query("INSERT INTO user_package_order (id,user_id,package_id,package_name,amount_micros,discount_micros,coupon_id,request_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(order_id).bind(auth.user_id).bind(id).bind(&name).bind(amount).bind(discount).bind(req.coupon_id).bind(req.request_id).execute(&mut *tx).await?;
    extend_subscription(&mut tx, auth.user_id, id, row.try_get("duration_days")?).await?;
    protect_paid_subscription(&mut tx, auth.user_id, id, price > 0).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"ok":true,"order_id":order_id,"amount_micros":amount}),
    ))
}

async fn purchase_receipt(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
    Path(request_id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    // Use the same lock as purchase: a confirmation racing its commit waits for
    // the transaction, and never mistakes an in-flight purchase for an absent one.
    let mut tx = state.db.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("package-purchase:{}:{}", auth.user_id, request_id))
        .execute(&mut *tx)
        .await?;
    let receipt = sqlx::query_scalar::<_, Value>(
        "SELECT jsonb_build_object('ok',true,'order_id',id,'request_id',request_id,
            'package_id',package_id,'package_name',package_name,'coupon_id',coupon_id,
            'amount_micros',amount_micros,'discount_micros',discount_micros,
            'created_at',created_at) FROM user_package_order WHERE user_id=$1 AND request_id=$2",
    )
    .bind(auth.user_id)
    .bind(request_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    tx.commit().await?;
    Ok(Json(receipt))
}

async fn protect_paid_subscription(
    tx: &mut Transaction<'_, Postgres>,
    user_id: DbId,
    package_id: DbId,
    protected: bool,
) -> AppResult<()> {
    if protected {
        sqlx::query("UPDATE user_package_subscription SET requires_payment=TRUE WHERE user_id=$1 AND package_id=$2")
            .bind(user_id).bind(package_id).execute(&mut **tx).await?;
    }
    Ok(())
}

fn coupon_discount(
    price: i64,
    amount: i64,
    minimum: i64,
    expires: Option<DateTime<Utc>>,
    used: bool,
    enabled: bool,
) -> AppResult<i64> {
    if used || !enabled || expires.is_some_and(|date| date <= Utc::now()) {
        return Err(AppError::BadRequest("优惠券已使用或已过期".into()));
    }
    if price < minimum || price == 0 {
        return Err(AppError::BadRequest("未达到优惠券使用金额".into()));
    }
    Ok(price.min(amount))
}

async fn extend_subscription(
    tx: &mut Transaction<'_, Postgres>,
    user_id: DbId,
    package_id: DbId,
    days: i32,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO user_package_subscription (user_id,package_id,expires_at) VALUES ($1,$2,now() + make_interval(days => $3))
         ON CONFLICT (user_id,package_id) DO UPDATE SET expires_at = GREATEST(user_package_subscription.expires_at, now()) + make_interval(days => $3)",
    ).bind(user_id).bind(package_id).bind(days).execute(&mut **tx).await?;
    Ok(())
}

async fn user_credit_account(
    tx: &mut Transaction<'_, Postgres>,
    user_id: DbId,
) -> AppResult<CreditAccountId> {
    let project_id = project::default_project_for_user(tx, user_id).await?;
    account::owner_credit_account_for_update(tx, CreditAccountType::Project, project_id).await
}

async fn ledger(
    tx: &mut Transaction<'_, Postgres>,
    account: &CreditAccountId,
    amount: i64,
    balance: i64,
    reason: &str,
    transaction_id: Uuid,
    metadata: Value,
) -> AppResult<()> {
    sqlx::query("INSERT INTO credit_ledger (credit_account_id,amount_micros,balance_after_micros,reason,transaction_id,metadata) VALUES ($1,$2,$3,$4,$5,$6)")
        .bind(account.id).bind(amount).bind(balance).bind(reason).bind(transaction_id).bind(metadata).execute(&mut **tx).await?;
    Ok(())
}

async fn coupons(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
) -> AppResult<Json<Value>> {
    let value = sqlx::query_scalar::<_, Value>(
        "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY t.created_at DESC), '[]') FROM
         (SELECT uc.id,c.name,c.amount_micros,c.min_spend_micros,c.expires_at,uc.used_at,uc.created_at,c.enabled
          FROM user_coupon uc JOIN user_coupon_campaign c ON c.id=uc.campaign_id WHERE uc.user_id=$1) t",
    ).bind(auth.user_id).fetch_one(&state.db.pool).await?;
    Ok(Json(value))
}

#[derive(Deserialize)]
struct CodeRequest {
    code: String,
}

async fn claim_coupon(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
    Json(req): Json<CodeRequest>,
) -> AppResult<Json<Value>> {
    let code = normalize_code(&req.code)?;
    let mut tx = state.db.pool.begin().await?;
    let row = sqlx::query("SELECT id, enabled, expires_at, claimed_count, max_claims FROM user_coupon_campaign WHERE code=$1 FOR UPDATE")
        .bind(code).fetch_optional(&mut *tx).await?.ok_or_else(|| AppError::BadRequest("优惠码无效".into()))?;
    if !row.try_get::<bool, _>("enabled")?
        || row
            .try_get::<Option<DateTime<Utc>>, _>("expires_at")?
            .is_some_and(|d| d <= Utc::now())
        || row.try_get::<i32, _>("claimed_count")? >= row.try_get::<i32, _>("max_claims")?
    {
        return Err(AppError::BadRequest("优惠码已失效或已领完".into()));
    }
    let id: DbId = row.try_get("id")?;
    let inserted = sqlx::query(
        "INSERT INTO user_coupon (user_id,campaign_id) VALUES ($1,$2) ON CONFLICT DO NOTHING",
    )
    .bind(auth.user_id)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(AppError::Conflict("你已领取过此优惠券".into()));
    }
    sqlx::query("UPDATE user_coupon_campaign SET claimed_count=claimed_count+1 WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true})))
}

async fn redeem(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
    Json(req): Json<CodeRequest>,
) -> AppResult<Json<Value>> {
    let code = normalize_code(&req.code)?;
    let mut tx = state.db.pool.begin().await?;
    let row = sqlx::query("SELECT id, amount_micros, package_id, redeemed_at, enabled, expires_at FROM user_redemption_code WHERE code=$1 FOR UPDATE")
        .bind(&code).fetch_optional(&mut *tx).await?.ok_or_else(|| AppError::BadRequest("兑换码无效".into()))?;
    if !row.try_get::<bool, _>("enabled")?
        || row
            .try_get::<Option<DateTime<Utc>>, _>("redeemed_at")?
            .is_some()
        || row
            .try_get::<Option<DateTime<Utc>>, _>("expires_at")?
            .is_some_and(|d| d <= Utc::now())
    {
        return Err(AppError::BadRequest("兑换码已使用或已过期".into()));
    }
    let amount: i64 = row.try_get("amount_micros")?;
    // Always lock the wallet before the subscription, matching purchases.
    let credit_account = user_credit_account(&mut tx, auth.user_id).await?;
    let mut package_name: Option<String> = None;
    if let Some(package_id) = row.try_get::<Option<DbId>, _>("package_id")? {
        let package = sqlx::query(
            "SELECT name,duration_days,price_micros FROM user_package WHERE id=$1 AND enabled AND service_enabled FOR SHARE",
        )
        .bind(package_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::BadRequest("此套餐已下架".into()))?;
        extend_subscription(
            &mut tx,
            auth.user_id,
            package_id,
            package.try_get("duration_days")?,
        )
        .await?;
        protect_paid_subscription(
            &mut tx,
            auth.user_id,
            package_id,
            package.try_get::<i64, _>("price_micros")? > 0,
        )
        .await?;
        package_name = Some(package.try_get("name")?);
    }
    if amount > 0 {
        let balance = account::adjust_balance(&mut tx, &credit_account, amount).await?;
        ledger(
            &mut tx,
            &credit_account,
            amount,
            balance,
            "gift",
            Uuid::new_v4(),
            json!({"source":"redemption_code","redemption_id":row.try_get::<DbId,_>("id")?}),
        )
        .await?;
    }
    sqlx::query("UPDATE user_redemption_code SET redeemed_by=$2,redeemed_at=now() WHERE id=$1")
        .bind(row.try_get::<DbId, _>("id")?)
        .bind(auth.user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"ok":true,"amount_micros":amount,"package_name":package_name}),
    ))
}

async fn redemptions(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
    Query(query): Query<PageQuery>,
) -> AppResult<Json<Value>> {
    let (page, page_size, offset) = query.bounds()?;
    let data = sqlx::query_scalar::<_, Value>(
        "WITH records AS (
            SELECT r.id, '***' || right(r.code,4) AS code_hint,r.amount_micros,
                   p.name AS package_name,r.redeemed_at AS created_at
            FROM user_redemption_code r LEFT JOIN user_package p ON p.id=r.package_id
            WHERE r.redeemed_by=$1
         ) SELECT jsonb_build_object('items',COALESCE((SELECT jsonb_agg(to_jsonb(t) ORDER BY t.created_at DESC,t.id DESC)
            FROM (SELECT * FROM records ORDER BY created_at DESC,id DESC LIMIT $2 OFFSET $3) t),'[]'),
            'total',(SELECT count(*) FROM records))",
    ).bind(auth.user_id).bind(page_size).bind(offset).fetch_one(&state.db.pool).await?;
    Ok(Json(with_page(data, page, page_size)))
}

fn with_page(mut value: Value, page: i64, page_size: i64) -> Value {
    value["page"] = json!(page);
    value["page_size"] = json!(page_size);
    value
}

async fn orders(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
    Query(query): Query<PageQuery>,
) -> AppResult<Json<Value>> {
    let (page, page_size, offset) = query.bounds()?;
    let kind = query.kind(&["package", "recharge"])?;
    crate::payment::expire_user_payment_orders(&state.db.pool, auth.user_id).await?;
    let data = sqlx::query_scalar::<_,Value>(
        "WITH records AS (
            SELECT id,'package' AS type,package_name AS name,'paid' AS status,amount_micros,
                   created_at,NULL::TEXT AS checkout_url,NULL::TIMESTAMPTZ AS expires_at,
                   request_id,package_id,coupon_id,discount_micros
            FROM user_package_order WHERE user_id=$1 AND ($2='all' OR $2='package')
            UNION ALL SELECT id,'recharge','账户充值',status,amount_micros,created_at,
                   CASE WHEN status='pending' THEN checkout_url END,expires_at,
                   NULL::UUID,NULL::BIGINT,NULL::BIGINT,0::BIGINT
            FROM payment WHERE user_id=$1 AND ($2='all' OR $2='recharge')
         ) SELECT jsonb_build_object('items',COALESCE((SELECT jsonb_agg(to_jsonb(t) ORDER BY t.created_at DESC,t.id DESC)
            FROM (SELECT * FROM records ORDER BY created_at DESC,id DESC LIMIT $3 OFFSET $4) t),'[]'),
            'total',(SELECT count(*) FROM records))",
    ).bind(auth.user_id).bind(kind).bind(page_size).bind(offset).fetch_one(&state.db.pool).await?;
    Ok(Json(with_page(data, page, page_size)))
}

async fn user_ledger(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
    Query(query): Query<PageQuery>,
) -> AppResult<Json<Value>> {
    let (page, page_size, offset) = query.bounds()?;
    let kind = query.kind(&[
        "usage",
        "recharge",
        "package",
        "redemption",
        "adjustment",
        "refund",
        "allocation_recover",
    ])?;
    let data = sqlx::query_scalar::<_,Value>(
        "WITH owned_accounts AS (
            SELECT w.id,w.owner_type AS account_type,w.owner_name AS account_name,w.owner_model AS model
            FROM credit_account w WHERE w.owner_user_id=$1
         ), all_records AS (
            SELECT l.id,l.credit_account_id,a.account_type,a.account_name,a.model,
                   CASE WHEN l.metadata->>'source'='package_purchase' THEN 'package'
                        WHEN l.metadata->>'source'='redemption_code' THEN 'redemption'
                        WHEN l.reason='gift' THEN 'adjustment' ELSE l.reason END AS type,
                   l.reason,l.amount_micros,l.balance_after_micros,l.transaction_id,l.usage_id,
                   l.metadata,l.created_at
              FROM credit_ledger l JOIN owned_accounts a ON a.id=l.credit_account_id
         ), records AS (SELECT * FROM all_records WHERE $2='all' OR type=$2)
         SELECT jsonb_build_object('items',COALESCE((SELECT jsonb_agg(to_jsonb(t) ORDER BY t.created_at DESC,t.id DESC)
            FROM (SELECT * FROM records ORDER BY created_at DESC,id DESC LIMIT $3 OFFSET $4) t),'[]'),
            'total',(SELECT count(*) FROM records))",
    ).bind(auth.user_id).bind(kind).bind(page_size).bind(offset).fetch_one(&state.db.pool).await?;
    let mut data = with_page(data, page, page_size);
    data["currency"] = json!(state.config.billing_currency.as_str());
    Ok(Json(data))
}

async fn notifications(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
    Query(query): Query<PageQuery>,
) -> AppResult<Json<Value>> {
    let (page, page_size, offset) = query.bounds()?;
    let kind = query.kind(&["unread", "read"])?;
    let data = sqlx::query_scalar::<_,Value>(
        "WITH visible AS (
            SELECT n.id,n.title,n.content,n.created_at,r.read_at FROM user_notification n
            LEFT JOIN user_notification_read r ON r.notification_id=n.id AND r.user_id=$1
            WHERE n.published AND (n.user_id IS NULL OR n.user_id=$1)
         ), records AS (
            SELECT * FROM visible WHERE $2='all' OR ($2='unread' AND read_at IS NULL) OR ($2='read' AND read_at IS NOT NULL)
         ) SELECT jsonb_build_object('items',COALESCE((SELECT jsonb_agg(to_jsonb(t) ORDER BY t.created_at DESC,t.id DESC)
            FROM (SELECT * FROM records ORDER BY created_at DESC,id DESC LIMIT $3 OFFSET $4) t),'[]'),
            'total',(SELECT count(*) FROM records),'unread_count',(SELECT count(*) FROM visible WHERE read_at IS NULL))",
    ).bind(auth.user_id).bind(kind).bind(page_size).bind(offset).fetch_one(&state.db.pool).await?;
    Ok(Json(with_page(data, page, page_size)))
}

async fn read_notification(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
    Path(id): Path<DbId>,
) -> AppResult<Json<Value>> {
    let visible: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM user_notification WHERE id=$1 AND published AND (user_id IS NULL OR user_id=$2))")
        .bind(id).bind(auth.user_id).fetch_one(&state.db.pool).await?;
    if !visible {
        return Err(AppError::NotFound);
    }
    sqlx::query("INSERT INTO user_notification_read (user_id,notification_id) VALUES ($1,$2) ON CONFLICT DO NOTHING")
        .bind(auth.user_id).bind(id).execute(&state.db.pool).await?;
    Ok(Json(json!({"ok":true})))
}

async fn read_all_notifications(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
) -> AppResult<Json<Value>> {
    sqlx::query("INSERT INTO user_notification_read (user_id,notification_id) SELECT $1,id FROM user_notification WHERE published AND (user_id IS NULL OR user_id=$1) ON CONFLICT DO NOTHING")
        .bind(auth.user_id).execute(&state.db.pool).await?;
    Ok(Json(json!({"ok":true})))
}

async fn admin_packages(
    State(state): State<Arc<AppState>>,
    _auth: AdminAuth,
) -> AppResult<Json<Vec<PackageRecord>>> {
    Ok(Json(list_packages(&state.db.pool, None).await?))
}

#[derive(Deserialize)]
struct SavePackageRequest {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default = "custom_family")]
    family: String,
    #[serde(default)]
    models: Vec<String>,
    #[serde(default)]
    price_micros: i64,
    #[serde(default = "month")]
    duration_days: i32,
    #[serde(default = "yes")]
    enabled: bool,
    #[serde(default = "yes")]
    service_enabled: bool,
}
fn custom_family() -> String {
    "custom".into()
}
fn month() -> i32 {
    30
}
fn yes() -> bool {
    true
}

async fn save_package(
    State(state): State<Arc<AppState>>,
    auth: AdminAuth,
    Json(req): Json<SavePackageRequest>,
) -> AppResult<Json<Value>> {
    require_admin_write(&state, auth).await?;
    write_package(&state, None, req).await.map(Json)
}
async fn update_package(
    State(state): State<Arc<AppState>>,
    auth: AdminAuth,
    Path(id): Path<DbId>,
    Json(req): Json<SavePackageRequest>,
) -> AppResult<Json<Value>> {
    require_admin_write(&state, auth).await?;
    write_package(&state, Some(id), req).await.map(Json)
}
async fn write_package(
    state: &AppState,
    id: Option<DbId>,
    req: SavePackageRequest,
) -> AppResult<Value> {
    let name = text(&req.name, "name", 100)?;
    let family = text(&req.family, "family", 50)?;
    if req.price_micros < 0
        || !(1..=3660).contains(&req.duration_days)
        || req.description.len() > 4000
    {
        return Err(AppError::BadRequest("套餐金额或有效期无效".into()));
    }
    let models = normalized_models(req.models)?;
    if family == "custom" && models.is_empty() {
        return Err(AppError::BadRequest("自定义套餐至少需要一个模型".into()));
    }
    let mut tx = state.db.pool.begin().await?;
    let package_id: DbId = if let Some(id) = id {
        sqlx::query("SELECT id FROM user_package WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
        if !req.service_enabled {
            let protected: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM user_package_subscription s
                 WHERE s.package_id=$1 AND s.expires_at > now() AND s.requires_payment)",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            if protected {
                return Err(AppError::Conflict(
                    "此套餐仍有有效付费订阅，暂不能停止服务。可先停售，已有用户仍可使用至到期。"
                        .into(),
                ));
            }
        }
        sqlx::query_scalar("UPDATE user_package SET name=$2,description=$3,family=$4,models=$5,price_micros=$6,duration_days=$7,enabled=$8,service_enabled=$9,updated_at=now() WHERE id=$1 RETURNING id")
            .bind(id).bind(name).bind(req.description).bind(family).bind(models).bind(req.price_micros).bind(req.duration_days).bind(req.enabled).bind(req.service_enabled).fetch_one(&mut *tx).await?
    } else {
        sqlx::query_scalar("INSERT INTO user_package (name,description,family,models,price_micros,duration_days,enabled,service_enabled) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING id")
            .bind(name).bind(req.description).bind(family).bind(models).bind(req.price_micros).bind(req.duration_days).bind(req.enabled).bind(req.service_enabled).fetch_one(&mut *tx).await?
    };
    tx.commit().await?;
    Ok(json!({"ok":true,"id":package_id}))
}

#[derive(Deserialize)]
struct CreateCouponRequest {
    code: String,
    name: String,
    amount_micros: i64,
    #[serde(default)]
    min_spend_micros: i64,
    #[serde(default = "one")]
    max_claims: i32,
    expires_at: Option<DateTime<Utc>>,
}
fn one() -> i32 {
    1
}
async fn create_coupon(
    State(state): State<Arc<AppState>>,
    auth: AdminAuth,
    Json(req): Json<CreateCouponRequest>,
) -> AppResult<Json<Value>> {
    require_admin_write(&state, auth).await?;
    let code = normalize_code(&req.code)?;
    let name = text(&req.name, "name", 100)?;
    if req.amount_micros <= 0 || req.min_spend_micros < 0 || req.max_claims <= 0 {
        return Err(AppError::BadRequest("优惠券金额或数量无效".into()));
    }
    let id:DbId=sqlx::query_scalar("INSERT INTO user_coupon_campaign (code,name,amount_micros,min_spend_micros,max_claims,expires_at) VALUES ($1,$2,$3,$4,$5,$6) RETURNING id")
        .bind(code).bind(name).bind(req.amount_micros).bind(req.min_spend_micros).bind(req.max_claims).bind(req.expires_at).fetch_one(&state.db.pool).await?;
    Ok(Json(json!({"ok":true,"id":id})))
}
async fn admin_coupons(
    State(state): State<Arc<AppState>>,
    _auth: AdminAuth,
) -> AppResult<Json<Value>> {
    Ok(Json(sqlx::query_scalar::<_,Value>("SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY t.id DESC),'[]') FROM (SELECT * FROM user_coupon_campaign ORDER BY id DESC LIMIT 500) t").fetch_one(&state.db.pool).await?))
}

#[derive(Deserialize)]
struct CreateRedemptionRequest {
    code: String,
    #[serde(default)]
    amount_micros: i64,
    package_id: Option<DbId>,
    expires_at: Option<DateTime<Utc>>,
}
async fn create_redemption(
    State(state): State<Arc<AppState>>,
    auth: AdminAuth,
    Json(req): Json<CreateRedemptionRequest>,
) -> AppResult<Json<Value>> {
    require_admin_write(&state, auth).await?;
    if req.amount_micros < 0 || (req.amount_micros == 0 && req.package_id.is_none()) {
        return Err(AppError::BadRequest("请填写兑换金额或套餐".into()));
    }
    let id:DbId=sqlx::query_scalar("INSERT INTO user_redemption_code (code,amount_micros,package_id,expires_at) VALUES ($1,$2,$3,$4) RETURNING id")
        .bind(normalize_code(&req.code)?).bind(req.amount_micros).bind(req.package_id).bind(req.expires_at).fetch_one(&state.db.pool).await?;
    Ok(Json(json!({"ok":true,"id":id})))
}
async fn admin_redemptions(
    State(state): State<Arc<AppState>>,
    _auth: AdminAuth,
) -> AppResult<Json<Value>> {
    Ok(Json(sqlx::query_scalar::<_,Value>("SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY t.id DESC),'[]') FROM (SELECT * FROM user_redemption_code ORDER BY id DESC LIMIT 500) t").fetch_one(&state.db.pool).await?))
}

#[derive(Deserialize)]
struct NotificationRequest {
    title: String,
    content: String,
    user_id: Option<DbId>,
}
async fn create_notification(
    State(state): State<Arc<AppState>>,
    auth: AdminAuth,
    Json(req): Json<NotificationRequest>,
) -> AppResult<Json<Value>> {
    require_admin_write(&state, auth).await?;
    let title = text(&req.title, "title", 120)?;
    let content = text(&req.content, "content", 10000)?;
    let id: DbId = sqlx::query_scalar(
        "INSERT INTO user_notification (title,content,user_id) VALUES ($1,$2,$3) RETURNING id",
    )
    .bind(title)
    .bind(content)
    .bind(req.user_id)
    .fetch_one(&state.db.pool)
    .await?;
    Ok(Json(json!({"ok":true,"id":id})))
}
async fn admin_notifications(
    State(state): State<Arc<AppState>>,
    _auth: AdminAuth,
) -> AppResult<Json<Value>> {
    Ok(Json(sqlx::query_scalar::<_,Value>("SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY t.id DESC),'[]') FROM (SELECT * FROM user_notification ORDER BY id DESC LIMIT 500) t").fetch_one(&state.db.pool).await?))
}
async fn delete_notification(
    State(state): State<Arc<AppState>>,
    auth: AdminAuth,
    Path(id): Path<DbId>,
) -> AppResult<Json<Value>> {
    require_admin_write(&state, auth).await?;
    sqlx::query("DELETE FROM user_notification WHERE id=$1")
        .bind(id)
        .execute(&state.db.pool)
        .await?;
    Ok(Json(json!({"ok":true})))
}

pub(super) async fn require_admin_write(state: &AppState, auth: AdminAuth) -> AppResult<()> {
    let role: String = sqlx::query_scalar("SELECT role FROM admin WHERE id=$1")
        .bind(auth.admin_id)
        .fetch_one(&state.db.pool)
        .await?;
    if role == "viewer" {
        return Err(AppError::Forbidden("只读管理员无法修改此设置".into()));
    }
    Ok(())
}
fn text(input: &str, field: &str, max: usize) -> AppResult<String> {
    let value = input.trim();
    if value.is_empty() || value.chars().count() > max {
        return Err(AppError::BadRequest(format!("invalid {field}")));
    }
    Ok(value.to_string())
}
fn normalize_code(input: &str) -> AppResult<String> {
    let code = text(input, "code", 100)?.to_ascii_uppercase();
    if !code
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(AppError::BadRequest(
            "兑换码只能包含字母、数字、短横线和下划线".into(),
        ));
    }
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn commerce_purchase_retries_do_not_charge_or_extend_twice(pool: PgPool) {
        let user_id = test_user(&pool, "idempotent@example.test").await;
        let state = test_state(pool.clone());
        let package_id:DbId=sqlx::query_scalar("INSERT INTO user_package (name,models,price_micros,duration_days) VALUES ('Paid',ARRAY['gpt-4o'],100000,1) RETURNING id").fetch_one(&pool).await.unwrap();
        sqlx::query("UPDATE credit_account SET balance_micros=1000000 WHERE owner_type='project'")
            .execute(&pool)
            .await
            .unwrap();
        let request_id = Uuid::new_v4();
        let (left, right) = tokio::join!(
            purchase(
                State(state.clone()),
                UserSessionAuth { user_id },
                Path(package_id),
                Json(PurchaseRequest {
                    request_id,
                    coupon_id: None
                })
            ),
            purchase(
                State(state.clone()),
                UserSessionAuth { user_id },
                Path(package_id),
                Json(PurchaseRequest {
                    request_id,
                    coupon_id: None
                })
            ),
        );
        assert_eq!(left.unwrap().0, right.unwrap().0);
        let confirmed = purchase_receipt(
            State(state.clone()),
            UserSessionAuth { user_id },
            Path(request_id),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(confirmed["package_id"], json!(package_id));
        assert_eq!(confirmed["amount_micros"], json!(100000));
        let other = test_user(&pool, "receipt-other@example.test").await;
        assert!(matches!(
            purchase_receipt(
                State(state.clone()),
                UserSessionAuth { user_id: other },
                Path(request_id)
            )
            .await,
            Err(AppError::NotFound)
        ));
        let balance: i64 = sqlx::query_scalar(
            "SELECT balance_micros FROM credit_account WHERE owner_type='project'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(balance, 900000);
        let orders: i64 = sqlx::query_scalar("SELECT count(*) FROM user_package_order")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(orders, 1);
        let seconds:f64=sqlx::query_scalar("SELECT EXTRACT(EPOCH FROM (expires_at-now()))::DOUBLE PRECISION FROM user_package_subscription WHERE user_id=$1").bind(user_id).fetch_one(&pool).await.unwrap();
        assert!((86390.0..=86400.0).contains(&seconds));
        assert!(purchase(
            State(state.clone()),
            UserSessionAuth { user_id },
            Path(package_id + 1),
            Json(PurchaseRequest {
                request_id,
                coupon_id: None
            })
        )
        .await
        .is_err());
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn commerce_purchase_and_package_credit_redemption_do_not_deadlock(pool: PgPool) {
        let user_id = test_user(&pool, "lock-order@example.test").await;
        let state = test_state(pool.clone());
        let package_id:DbId=sqlx::query_scalar("INSERT INTO user_package (name,models,price_micros) VALUES ('Paid',ARRAY['gpt-4o'],100000) RETURNING id").fetch_one(&pool).await.unwrap();
        sqlx::query("UPDATE credit_account SET balance_micros=1000000 WHERE owner_type='project'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO user_redemption_code (code,amount_micros,package_id) VALUES ('CREDIT-AND-PACKAGE',200000,$1)").bind(package_id).execute(&pool).await.unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            tokio::join!(
                purchase(
                    State(state.clone()),
                    UserSessionAuth { user_id },
                    Path(package_id),
                    Json(PurchaseRequest {
                        request_id: Uuid::new_v4(),
                        coupon_id: None
                    })
                ),
                redeem(
                    State(state.clone()),
                    UserSessionAuth { user_id },
                    Json(CodeRequest {
                        code: "CREDIT-AND-PACKAGE".into()
                    })
                ),
            )
        })
        .await
        .unwrap();
        assert!(result.0.is_ok());
        assert!(result.1.is_ok());
        let balance: i64 = sqlx::query_scalar(
            "SELECT balance_micros FROM credit_account WHERE owner_type='project'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(balance, 1100000);
    }

    fn test_state(pool: PgPool) -> Arc<AppState> {
        let mut state = crate::app::tests::test_state();
        Arc::get_mut(&mut state).unwrap().db.pool = pool;
        state
    }

    async fn test_user(pool: &PgPool, email: &str) -> DbId {
        let user_id = sqlx::query_scalar(
            "INSERT INTO \"user\" (email,user_group_id,password_changed_at) SELECT $1,id,now() FROM user_group WHERE is_default RETURNING id",
        ).bind(email).fetch_one(pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        project::ensure_default_project_for_user(&mut tx, user_id)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        user_id
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn commerce_redemptions_and_coupon_claims_are_atomic(pool: PgPool) {
        let a = test_user(&pool, "redemption-a@example.test").await;
        let b = test_user(&pool, "redemption-b@example.test").await;
        let state = test_state(pool.clone());
        sqlx::query(
            "INSERT INTO user_redemption_code (code,amount_micros) VALUES ('ONE-TIME',1234567)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let (left, right) = tokio::join!(
            redeem(
                State(state.clone()),
                UserSessionAuth { user_id: a },
                Json(CodeRequest {
                    code: "one-time".into()
                })
            ),
            redeem(
                State(state.clone()),
                UserSessionAuth { user_id: b },
                Json(CodeRequest {
                    code: "ONE-TIME".into()
                })
            ),
        );
        assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
        let total: i64 =
            sqlx::query_scalar("SELECT SUM(balance_micros)::BIGINT FROM credit_account")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(total, 1234567);
        let entries: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM credit_ledger WHERE metadata->>'source'='redemption_code'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(entries, 1);
        sqlx::query("INSERT INTO user_coupon_campaign (code,name,amount_micros,max_claims) VALUES ('LAST-COUPON','One coupon',100,1)").execute(&pool).await.unwrap();
        let (left, right) = tokio::join!(
            claim_coupon(
                State(state.clone()),
                UserSessionAuth { user_id: a },
                Json(CodeRequest {
                    code: "LAST-COUPON".into()
                })
            ),
            claim_coupon(
                State(state.clone()),
                UserSessionAuth { user_id: b },
                Json(CodeRequest {
                    code: "LAST-COUPON".into()
                })
            ),
        );
        assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM user_coupon")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn commerce_purchase_rolls_back_coupon_when_balance_is_insufficient(pool: PgPool) {
        let user_id = test_user(&pool, "purchase@example.test").await;
        let package_id:DbId=sqlx::query_scalar("INSERT INTO user_package (name,models,price_micros) VALUES ('Paid',ARRAY['gpt-4o'],1000000) RETURNING id").fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO user_coupon_campaign (code,name,amount_micros) VALUES ('SAVE','Save',100000)").execute(&pool).await.unwrap();
        let state = test_state(pool.clone());
        let _ = claim_coupon(
            State(state.clone()),
            UserSessionAuth { user_id },
            Json(CodeRequest {
                code: "SAVE".into(),
            }),
        )
        .await
        .unwrap();
        let coupon_id: DbId = sqlx::query_scalar("SELECT id FROM user_coupon WHERE user_id=$1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(purchase(
            State(state.clone()),
            UserSessionAuth { user_id },
            Path(package_id),
            Json(PurchaseRequest {
                request_id: Uuid::new_v4(),
                coupon_id: Some(coupon_id)
            })
        )
        .await
        .is_err());
        let used: bool =
            sqlx::query_scalar("SELECT used_at IS NOT NULL FROM user_coupon WHERE id=$1")
                .bind(coupon_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(!used);
        let orders: i64 = sqlx::query_scalar("SELECT count(*) FROM user_package_order")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(orders, 0);
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn commerce_key_scope_survives_catalog_expansion_and_rechecks_expiry(pool: PgPool) {
        let user_id = test_user(&pool, "scope@example.test").await;
        let state = test_state(pool.clone());
        sqlx::raw_sql("INSERT INTO channel (id,provider,name) VALUES (99001,'openai','Scope test'); INSERT INTO channel_model (channel_id,model,enabled) VALUES (99001,'gpt-4o',TRUE),(99001,'gpt-4.1',TRUE); INSERT INTO channel_endpoint (channel_id,protocol,base_url) VALUES (99001,'openai','https://scope.invalid'); INSERT INTO channel_key (channel_id,name,key_prefix,secret_ciphertext) VALUES (99001,'Test','sk','encrypted'); INSERT INTO channel_price (channel_id,model,input_price_micros,output_price_micros) VALUES (99001,'gpt-4o',100,200),(99001,'gpt-4.1',100,200);").execute(&pool).await.unwrap();
        let package_id: DbId =
            sqlx::query_scalar("SELECT id FROM user_package WHERE family='chatgpt'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let models = key_package_scope(&state, user_id, package_id, Some(vec!["gpt-4o".into()]))
            .await
            .unwrap();
        assert!(key_package_scope(
            &state,
            user_id,
            package_id,
            Some(vec!["claude-sonnet-4-6".into()])
        )
        .await
        .is_err());
        let encrypted = state.secrets.encrypt("sk-scope-test-key").unwrap();
        let key_id:DbId=sqlx::query_scalar("INSERT INTO user_key (user_id,project_id,name,key_prefix,secret_ciphertext,package_id,model_limits) SELECT $1,id,'Scope','sk-scope',$2,$3,$4 FROM project WHERE owner_user_id=$1 AND is_default RETURNING user_key.id")
            .bind(user_id).bind(encrypted).bind(package_id).bind(models).fetch_one(&pool).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        account::create_credit_account(&mut tx, CreditAccountType::UserKey, key_id)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let auth = crate::auth::user_auth_for_key_id(&state, key_id)
            .await
            .unwrap();
        assert!(auth.ensure_model_scope(&state, "gpt-4o").await.is_ok());
        assert!(auth.ensure_model_scope(&state, "gpt-4.1").await.is_err());
        assert!(auth
            .ensure_model_scope(&state, "claude-sonnet-4-6")
            .await
            .is_err());
        sqlx::query("UPDATE user_package SET price_micros=100 WHERE id=$1")
            .bind(package_id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(auth.ensure_model_scope(&state, "gpt-4o").await.is_err());
        let mut tx = pool.begin().await.unwrap();
        extend_subscription(&mut tx, user_id, package_id, 1)
            .await
            .unwrap();
        protect_paid_subscription(&mut tx, user_id, package_id, true)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert!(auth.ensure_model_scope(&state, "gpt-4o").await.is_ok());
        sqlx::query("UPDATE user_package SET enabled=FALSE WHERE id=$1")
            .bind(package_id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(auth.ensure_model_scope(&state, "gpt-4o").await.is_ok());
        sqlx::query("UPDATE user_package SET models=ARRAY['gpt-4.1'] WHERE id=$1")
            .bind(package_id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(auth.ensure_model_scope(&state, "gpt-4o").await.is_err());
        assert!(auth.ensure_model_scope(&state, "gpt-4.1").await.is_err());
        sqlx::query("UPDATE user_package SET models='{}',price_micros=0 WHERE id=$1")
            .bind(package_id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(auth.ensure_model_scope(&state, "gpt-4o").await.is_ok());
        sqlx::query("UPDATE user_package_subscription SET expires_at=now()-interval '1 second' WHERE user_id=$1").bind(user_id).execute(&pool).await.unwrap();
        assert!(auth.ensure_model_scope(&state, "gpt-4o").await.is_err());
    }
    #[test]
    fn model_families_do_not_mix_text_image_video_or_audio() {
        for (model, family) in [
            ("claude-sonnet-4-6", "claude"),
            ("gpt-4o", "chatgpt"),
            ("gpt-image-1", "image"),
            ("gpt-4o-mini-tts", "audio"),
            ("sora-2", "video"),
            ("gemini-2.5-pro", "gemini"),
            ("vendor/claude-sonnet-4-6", "claude"),
            ("vendor/FLUX.1-dev", "image"),
            ("doubao-seedream-4.0", "image"),
            ("doubao-seedance-1.5-pro", "video"),
            ("gen4-turbo", "video"),
            ("eleven_multilingual_v2", "audio"),
        ] {
            assert_eq!(
                crate::catalog::model_family(model, "token", &json!({})),
                family,
                "{model}"
            );
        }
        assert_eq!(
            crate::catalog::model_family("new-video-name", "video", &json!({})),
            "video"
        );
        assert_eq!(
            crate::catalog::model_family(
                "new-image-name",
                "token",
                &json!({"image_generation":true})
            ),
            "image"
        );
    }
    #[test]
    fn coupons_reject_expiration_reuse_and_minimum_violations() {
        assert!(coupon_discount(100, 20, 0, None, true, true).is_err());
        assert!(coupon_discount(
            100,
            20,
            0,
            Some(Utc::now() - chrono::Duration::seconds(1)),
            false,
            true
        )
        .is_err());
        assert!(coupon_discount(100, 20, 200, None, false, true).is_err());
        assert_eq!(
            coupon_discount(100, 200, 0, None, false, true).unwrap(),
            100
        );
    }
    #[test]
    fn commerce_pagination_rejects_invalid_and_overflowing_bounds() {
        for (page, page_size) in [(0, 20), (1, 0), (1, 101), (i64::MAX, 100)] {
            assert!(PageQuery {
                page: Some(page),
                page_size: Some(page_size),
                ..Default::default()
            }
            .bounds()
            .is_err());
        }
        assert_eq!(PageQuery::default().bounds().unwrap(), (1, 20, 0));
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn commerce_history_paginates_without_truncating_or_crossing_users(pool: PgPool) {
        let user_id = test_user(&pool, "pages@example.test").await;
        let other = test_user(&pool, "pages-other@example.test").await;
        let state = test_state(pool.clone());
        let package_id: DbId =
            sqlx::query_scalar("SELECT id FROM user_package ORDER BY id LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        for n in 0..125 {
            sqlx::query("INSERT INTO user_package_order (id,user_id,package_id,package_name,amount_micros,request_id) VALUES ($1,$2,$3,'History',0,$4)")
                .bind(Uuid::new_v4()).bind(user_id).bind(package_id).bind(Uuid::new_v4()).execute(&pool).await.unwrap();
            sqlx::query(
                "INSERT INTO user_notification (title,content,user_id) VALUES ($1,'Body',$2)",
            )
            .bind(format!("Notice {n}"))
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query("INSERT INTO user_redemption_code (code,amount_micros,redeemed_by,redeemed_at) VALUES ($1,1,$2,now())")
                .bind(format!("PAGE-{n}")).bind(user_id).execute(&pool).await.unwrap();
        }
        sqlx::query(
            "INSERT INTO user_notification (title,content,user_id) VALUES ('Other','Hidden',$1)",
        )
        .bind(other)
        .execute(&pool)
        .await
        .unwrap();
        let page = || {
            Query(PageQuery {
                page: Some(7),
                page_size: Some(20),
                ..Default::default()
            })
        };
        for response in [
            orders(State(state.clone()), UserSessionAuth { user_id }, page())
                .await
                .unwrap()
                .0,
            redemptions(State(state.clone()), UserSessionAuth { user_id }, page())
                .await
                .unwrap()
                .0,
            notifications(State(state.clone()), UserSessionAuth { user_id }, page())
                .await
                .unwrap()
                .0,
        ] {
            assert_eq!(response["total"], json!(125));
            assert_eq!(response["items"].as_array().unwrap().len(), 5);
            assert_eq!(response["page"], json!(7));
        }
        let filtered = orders(
            State(state.clone()),
            UserSessionAuth { user_id },
            Query(PageQuery {
                kind: Some("recharge".into()),
                ..Default::default()
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(filtered["total"], json!(0));
        let _ = read_all_notifications(State(state.clone()), UserSessionAuth { user_id })
            .await
            .unwrap();
        let unread = notifications(
            State(state),
            UserSessionAuth { user_id },
            Query(PageQuery {
                kind: Some("unread".into()),
                ..Default::default()
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(unread["total"], json!(0));
        assert_eq!(unread["unread_count"], json!(0));
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn commerce_delisting_preserves_paid_entitlements_and_blocks_service_stop(pool: PgPool) {
        let user_id = test_user(&pool, "protected@example.test").await;
        let state = test_state(pool.clone());
        let package_id:DbId=sqlx::query_scalar("INSERT INTO user_package (name,models,price_micros) VALUES ('Protected',ARRAY['gpt-4o'],100) RETURNING id").fetch_one(&pool).await.unwrap();
        sqlx::query("UPDATE credit_account SET balance_micros=1000 WHERE owner_type='project'")
            .execute(&pool)
            .await
            .unwrap();
        let _ = purchase(
            State(state.clone()),
            UserSessionAuth { user_id },
            Path(package_id),
            Json(PurchaseRequest {
                request_id: Uuid::new_v4(),
                coupon_id: None,
            }),
        )
        .await
        .unwrap();
        let update = |service_enabled| SavePackageRequest {
            name: "Protected".into(),
            description: String::new(),
            family: "custom".into(),
            models: vec!["gpt-4o".into()],
            price_micros: 0,
            duration_days: 30,
            enabled: false,
            service_enabled,
        };
        write_package(&state, Some(package_id), update(true))
            .await
            .unwrap();
        let package = list_packages(&pool, Some(user_id))
            .await
            .unwrap()
            .into_iter()
            .find(|p| p.id == package_id)
            .unwrap();
        assert!(!package.enabled);
        assert!(package.active);
        assert!(matches!(
            write_package(&state, Some(package_id), update(false)).await,
            Err(AppError::Conflict(_))
        ));
        assert!(purchase(
            State(state.clone()),
            UserSessionAuth { user_id },
            Path(package_id),
            Json(PurchaseRequest {
                request_id: Uuid::new_v4(),
                coupon_id: None
            })
        )
        .await
        .is_err());
        sqlx::query("UPDATE user_package_subscription SET expires_at=now()-interval '1 second' WHERE package_id=$1").bind(package_id).execute(&pool).await.unwrap();
        write_package(&state, Some(package_id), update(false))
            .await
            .unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn commerce_ledger_includes_key_accounts_and_isolates_users(pool: PgPool) {
        let user_id = test_user(&pool, "ledger@example.test").await;
        let other = test_user(&pool, "ledger-other@example.test").await;
        let state = test_state(pool.clone());
        let project_id: DbId =
            sqlx::query_scalar("SELECT id FROM project WHERE owner_user_id=$1 AND is_default")
                .bind(user_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let key_id:DbId=sqlx::query_scalar("INSERT INTO user_key (user_id,project_id,name,key_prefix,secret_ciphertext) VALUES ($1,$2,'My Key','sk','encrypted') RETURNING id").bind(user_id).bind(project_id).fetch_one(&pool).await.unwrap();
        let model_id: DbId = sqlx::query_scalar(
            "INSERT INTO user_key_model (user_key_id,model) VALUES ($1,'gpt-4o') RETURNING id",
        )
        .bind(key_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let mut tx = pool.begin().await.unwrap();
        let key_account =
            account::create_credit_account(&mut tx, CreditAccountType::UserKey, key_id)
                .await
                .unwrap();
        let model_account =
            account::create_credit_account(&mut tx, CreditAccountType::UserKeyModel, model_id)
                .await
                .unwrap();
        let wallet = user_credit_account(&mut tx, user_id).await.unwrap();
        let other_wallet = user_credit_account(&mut tx, other).await.unwrap();
        for (account, amount, reason, metadata) in [
            (&wallet, 100, "recharge", json!({})),
            (&key_account, -10, "usage", json!({})),
            (
                &model_account,
                20,
                "gift",
                json!({"source":"redemption_code"}),
            ),
            (&other_wallet, 999, "gift", json!({})),
        ] {
            ledger(
                &mut tx,
                account,
                amount,
                0,
                reason,
                Uuid::new_v4(),
                metadata,
            )
            .await
            .unwrap();
        }
        tx.commit().await.unwrap();
        sqlx::query("DELETE FROM user_key WHERE id=$1")
            .bind(key_id)
            .execute(&pool)
            .await
            .unwrap();
        let data = user_ledger(
            State(state.clone()),
            UserSessionAuth { user_id },
            Query(PageQuery::default()),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(data["total"], json!(3));
        let data = user_ledger(
            State(state),
            UserSessionAuth { user_id },
            Query(PageQuery {
                kind: Some("redemption".into()),
                ..Default::default()
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(data["total"], json!(1));
        assert_eq!(data["items"][0]["model"], json!("gpt-4o"));
    }
}
