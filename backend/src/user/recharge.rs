use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use uuid::Uuid;

use super::commerce::{Page, PageQuery};

use crate::{
    auth::UserSessionAuth,
    error::AppResult,
    payment::{
        cancel_user_payment_order, create_user_payment_order, list_user_payment_orders,
        CreatePaymentOrderRequest, CreatePaymentOrderResponse, PaymentOrderRecord,
    },
    AppState,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/user/recharge/orders",
            get(list_orders).post(create_order),
        )
        .route("/api/user/recharge/orders/{id}/cancel", post(cancel_order))
}

async fn create_order(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
    Json(req): Json<CreatePaymentOrderRequest>,
) -> AppResult<Json<CreatePaymentOrderResponse>> {
    Ok(Json(create_user_payment_order(&state, auth, req).await?))
}

async fn list_orders(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
    Query(query): Query<PageQuery>,
) -> AppResult<Json<Page<PaymentOrderRecord>>> {
    Ok(Json(list_user_payment_orders(&state, auth, query).await?))
}

async fn cancel_order(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PaymentOrderRecord>> {
    Ok(Json(cancel_user_payment_order(&state, auth, id).await?))
}
