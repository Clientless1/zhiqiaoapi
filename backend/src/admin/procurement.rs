use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::{
    auth::AdminAuth,
    error::{AppError, AppResult},
    id::DbId,
    AppState,
};

#[derive(Debug, Deserialize, Serialize)]
pub struct ProcurementInput {
    channel_id: DbId,
    model: String,
    input_cost_micros: Option<i64>,
    output_cost_micros: Option<i64>,
    cache_read_cost_micros: Option<i64>,
    cache_write_cost_micros: Option<i64>,
    unit_cost_micros: Option<i64>,
    monthly_budget_micros: Option<i64>,
    #[serde(default)]
    supplier: String,
    #[serde(default)]
    notes: String,
}

#[derive(Serialize)]
struct ProcurementRecord {
    #[serde(flatten)]
    quote: ProcurementInput,
    updated_at: DateTime<Utc>,
}

pub(crate) fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/admin/procurement", get(list).post(save))
}

async fn list(
    State(state): State<Arc<AppState>>,
    _admin: AdminAuth,
) -> AppResult<Json<Vec<ProcurementRecord>>> {
    let rows = sqlx::query("SELECT * FROM channel_procurement ORDER BY channel_id, model")
        .fetch_all(&state.db.pool)
        .await?;
    let records = rows
        .iter()
        .map(|row| -> Result<_, sqlx::Error> {
            Ok(ProcurementRecord {
                quote: ProcurementInput {
                    channel_id: row.try_get("channel_id")?,
                    model: row.try_get("model")?,
                    input_cost_micros: row.try_get("input_cost_micros")?,
                    output_cost_micros: row.try_get("output_cost_micros")?,
                    cache_read_cost_micros: row.try_get("cache_read_cost_micros")?,
                    cache_write_cost_micros: row.try_get("cache_write_cost_micros")?,
                    unit_cost_micros: row.try_get("unit_cost_micros")?,
                    monthly_budget_micros: row.try_get("monthly_budget_micros")?,
                    supplier: row.try_get("supplier")?,
                    notes: row.try_get("notes")?,
                },
                updated_at: row.try_get("updated_at")?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(records))
}

fn validate(req: &ProcurementInput) -> AppResult<()> {
    if req.model.trim().is_empty()
        || req.model.len() > 256
        || req.supplier.chars().count() > 120
        || req.notes.chars().count() > 2000
    {
        return Err(AppError::BadRequest(
            "invalid model, supplier or notes length".to_string(),
        ));
    }
    for value in [
        req.input_cost_micros,
        req.output_cost_micros,
        req.cache_read_cost_micros,
        req.cache_write_cost_micros,
        req.unit_cost_micros,
        req.monthly_budget_micros,
    ]
    .into_iter()
    .flatten()
    {
        if !(0..=1_000_000_000_000).contains(&value) {
            return Err(AppError::BadRequest(
                "procurement amounts must be between 0 and 1000000000000 micros".to_string(),
            ));
        }
    }
    Ok(())
}

async fn save(
    State(state): State<Arc<AppState>>,
    _admin: AdminAuth,
    Json(req): Json<ProcurementInput>,
) -> AppResult<Json<ProcurementInput>> {
    validate(&req)?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM channel_price WHERE channel_id = $1 AND model = $2)",
    )
    .bind(req.channel_id)
    .bind(&req.model)
    .fetch_one(&state.db.pool)
    .await?;
    if !exists {
        return Err(AppError::NotFound);
    }
    sqlx::query(r#"INSERT INTO channel_procurement
        (channel_id, model, input_cost_micros, output_cost_micros, cache_read_cost_micros,
         cache_write_cost_micros, unit_cost_micros, monthly_budget_micros, supplier, notes)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
        ON CONFLICT (channel_id, model) DO UPDATE SET
        input_cost_micros = EXCLUDED.input_cost_micros, output_cost_micros = EXCLUDED.output_cost_micros,
        cache_read_cost_micros = EXCLUDED.cache_read_cost_micros, cache_write_cost_micros = EXCLUDED.cache_write_cost_micros,
        unit_cost_micros = EXCLUDED.unit_cost_micros, monthly_budget_micros = EXCLUDED.monthly_budget_micros,
        supplier = EXCLUDED.supplier, notes = EXCLUDED.notes, updated_at = now()"#)
        .bind(req.channel_id).bind(&req.model).bind(req.input_cost_micros).bind(req.output_cost_micros)
        .bind(req.cache_read_cost_micros).bind(req.cache_write_cost_micros).bind(req.unit_cost_micros)
        .bind(req.monthly_budget_micros).bind(&req.supplier).bind(&req.notes)
        .execute(&state.db.pool).await?;
    Ok(Json(req))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_negative_or_unbounded_quotes_but_accepts_free_and_unknown() {
        let mut req: ProcurementInput = serde_json::from_value(
            serde_json::json!({"channel_id":1,"model":"test","input_cost_micros":0}),
        )
        .unwrap();
        assert!(validate(&req).is_ok());
        req.unit_cost_micros = Some(-1);
        assert!(validate(&req).is_err());
        req.unit_cost_micros = Some(1_000_000_000_001);
        assert!(validate(&req).is_err());
    }

    #[tokio::test]
    async fn procurement_quotes_require_admin_authentication() {
        use axum::{
            body::Body,
            http::{Request, StatusCode},
        };
        use tower::ServiceExt;
        for method in ["GET", "POST"] {
            let app = router().with_state(crate::app::tests::test_state());
            let response = app
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri("/api/admin/procurement")
                        .header("content-type", "application/json")
                        .body(Body::from("{}"))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn procurement_persists_quotes_without_changing_selling_prices(pool: sqlx::PgPool) {
        sqlx::raw_sql(
            r#"
            INSERT INTO provider_model (provider, model) VALUES ('openai', 'procurement-test');
            INSERT INTO channel (id, provider, name) VALUES (92001, 'openai', 'test');
            INSERT INTO channel_model (channel_id, model) VALUES (92001, 'procurement-test');
            INSERT INTO channel_price (channel_id, model, input_price_micros, output_price_micros)
              VALUES (92001, 'procurement-test', 2000000, 8000000);
        "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        let mut state = crate::app::tests::test_state();
        Arc::get_mut(&mut state).unwrap().db.pool = pool.clone();
        let req: ProcurementInput = serde_json::from_value(serde_json::json!({
            "channel_id": 92001, "model": "procurement-test", "input_cost_micros": 0,
            "output_cost_micros": 3000000, "supplier": "Supplier A", "monthly_budget_micros": 9000000
        })).unwrap();
        let _ = save(State(state.clone()), AdminAuth { admin_id: 1 }, Json(req))
            .await
            .unwrap();
        let Json(records) = list(State(state.clone()), AdminAuth { admin_id: 1 })
            .await
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].quote.input_cost_micros, Some(0));
        assert_eq!(records[0].quote.unit_cost_micros, None);
        let req: ProcurementInput = serde_json::from_value(serde_json::json!({
            "channel_id": 92001, "model": "procurement-test", "supplier": "Supplier B"
        }))
        .unwrap();
        let _ = save(State(state.clone()), AdminAuth { admin_id: 1 }, Json(req))
            .await
            .unwrap();
        let Json(records) = list(State(state), AdminAuth { admin_id: 1 }).await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].quote.input_cost_micros, None);
        assert_eq!(records[0].quote.supplier, "Supplier B");
        let selling: i64 = sqlx::query_scalar(
            "SELECT input_price_micros FROM channel_price WHERE channel_id = 92001",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(selling, 2000000);
        sqlx::query("DELETE FROM channel_price WHERE channel_id = 92001")
            .execute(&pool)
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM channel_procurement")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}
