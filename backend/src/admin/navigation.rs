use std::{collections::BTreeSet, sync::Arc};

use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};

use crate::{
    auth::AdminAuth,
    error::{AppError, AppResult},
    AppState,
};

const NAVIGATION_SETTING_KEY: &str = "user_navigation";
const OPTIONAL_ITEMS: &[&str] = &[
    "models",
    "playground",
    "apikeys",
    "usage",
    "packages",
    "coupons",
    "redeem",
    "orders",
    "ledger",
    "recharge",
    "status",
    "notifications",
];

#[derive(Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct NavigationSetting {
    #[serde(default)]
    hidden_items: Vec<String>,
}

impl NavigationSetting {
    fn normalized(self) -> AppResult<Self> {
        if self.hidden_items.len() > OPTIONAL_ITEMS.len()
            || self
                .hidden_items
                .iter()
                .any(|item| !OPTIONAL_ITEMS.contains(&item.as_str()))
        {
            return Err(AppError::BadRequest("无效的用户菜单项目".into()));
        }
        let hidden: BTreeSet<_> = self.hidden_items.into_iter().collect();
        Ok(Self {
            hidden_items: OPTIONAL_ITEMS
                .iter()
                .filter(|item| hidden.contains(**item))
                .map(|item| (*item).to_string())
                .collect(),
        })
    }
}

pub(crate) fn public_router() -> Router<Arc<AppState>> {
    Router::new().route("/api/public/navigation", get(public_setting))
}

pub(crate) fn router() -> Router<Arc<AppState>> {
    Router::new().route(
        "/api/admin/settings/navigation",
        get(admin_setting).post(save_setting),
    )
}

async fn load(state: &AppState) -> AppResult<NavigationSetting> {
    let value =
        sqlx::query_scalar::<_, serde_json::Value>("SELECT value FROM setting WHERE key=$1")
            .bind(NAVIGATION_SETTING_KEY)
            .fetch_optional(&state.db.pool)
            .await?;
    let setting: NavigationSetting = value
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default();
    setting.normalized()
}

async fn public_setting(State(state): State<Arc<AppState>>) -> AppResult<Json<NavigationSetting>> {
    Ok(Json(load(&state).await?))
}

async fn admin_setting(
    State(state): State<Arc<AppState>>,
    _auth: AdminAuth,
) -> AppResult<Json<NavigationSetting>> {
    Ok(Json(load(&state).await?))
}

async fn save_setting(
    State(state): State<Arc<AppState>>,
    auth: AdminAuth,
    Json(req): Json<NavigationSetting>,
) -> AppResult<Json<NavigationSetting>> {
    // Menu settings follow the same owner/operator versus viewer distinction as commerce settings.
    let role: String = sqlx::query_scalar("SELECT role FROM admin WHERE id=$1")
        .bind(auth.admin_id)
        .fetch_one(&state.db.pool)
        .await?;
    if role == "viewer" {
        return Err(AppError::Forbidden("只读管理员无法修改此设置".into()));
    }
    let setting = req.normalized()?;
    sqlx::query("INSERT INTO setting (key,value) VALUES ($1,$2) ON CONFLICT (key) DO UPDATE SET value=EXCLUDED.value,updated_at=now()")
        .bind(NAVIGATION_SETTING_KEY)
        .bind(serde_json::to_value(&setting)?)
        .execute(&state.db.pool).await?;
    Ok(Json(setting))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_preserve_all_pages() {
        assert!(NavigationSetting::default()
            .normalized()
            .unwrap()
            .hidden_items
            .is_empty());
    }

    #[test]
    fn cannot_hide_required_or_unknown_destinations() {
        for item in ["overview", "settings", "admin", "../status", "/home/status"] {
            assert!(NavigationSetting {
                hidden_items: vec![item.into()]
            }
            .normalized()
            .is_err());
        }
    }

    #[test]
    fn deduplicates_and_orders_configuration() {
        assert_eq!(
            NavigationSetting {
                hidden_items: vec!["status".into(), "models".into(), "status".into()]
            }
            .normalized()
            .unwrap()
            .hidden_items,
            vec!["models", "status"]
        );
    }

    #[test]
    fn rejects_unexpected_payload_fields() {
        assert!(serde_json::from_value::<NavigationSetting>(
            serde_json::json!({"hidden_items":[], "admin":true})
        )
        .is_err());
    }
}
