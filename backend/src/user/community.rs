use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};

use crate::{
    auth::AdminAuth,
    error::{AppError, AppResult},
    AppState,
};

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
struct CommunitySetting {
    about_title: String,
    about_content: String,
    qq_group_number: String,
    qq_group_url: String,
}

pub(crate) fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/public/community", get(public_setting))
        .route("/api/admin/community", get(admin_setting).put(save_setting))
}

async fn load(state: &AppState) -> AppResult<CommunitySetting> {
    let value = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM setting WHERE key='community'",
    )
    .fetch_optional(&state.db.pool)
    .await?;
    let mut setting: CommunitySetting = value
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default();
    if setting.about_title.is_empty() {
        setting.about_title = "关于我们".into();
    }
    if setting.about_content.is_empty() {
        setting.about_content =
            "为开发者提供简单、稳定的多模型 API 服务。欢迎加入社区交流使用体验。".into();
    }
    Ok(setting)
}

async fn public_setting(State(state): State<Arc<AppState>>) -> AppResult<Json<CommunitySetting>> {
    Ok(Json(load(&state).await?))
}
async fn admin_setting(
    State(state): State<Arc<AppState>>,
    _auth: AdminAuth,
) -> AppResult<Json<CommunitySetting>> {
    Ok(Json(load(&state).await?))
}

async fn save_setting(
    State(state): State<Arc<AppState>>,
    auth: AdminAuth,
    Json(mut req): Json<CommunitySetting>,
) -> AppResult<Json<CommunitySetting>> {
    super::commerce::require_admin_write(&state, auth).await?;
    req.about_title = req.about_title.trim().to_string();
    req.about_content = req.about_content.trim().to_string();
    req.qq_group_number = req.qq_group_number.trim().to_string();
    req.qq_group_url = req.qq_group_url.trim().to_string();
    if req.about_title.chars().count() > 100
        || req.about_content.chars().count() > 10000
        || req.qq_group_number.len() > 20
        || !req.qq_group_number.chars().all(|c| c.is_ascii_digit())
    {
        return Err(AppError::BadRequest("关于内容或 QQ 群号格式无效".into()));
    }
    if !req.qq_group_url.is_empty() {
        let url = reqwest::Url::parse(&req.qq_group_url)
            .map_err(|_| AppError::BadRequest("QQ群链接无效".into()))?;
        if url.scheme() != "https"
            || !url
                .host_str()
                .is_some_and(|host| host == "qq.com" || host.ends_with(".qq.com"))
        {
            return Err(AppError::BadRequest(
                "请使用 https:// 开头的 QQ 官方群邀请链接".into(),
            ));
        }
    }
    sqlx::query("INSERT INTO setting (key,value) VALUES ('community',$1) ON CONFLICT (key) DO UPDATE SET value=$1,updated_at=now()")
        .bind(serde_json::to_value(&req)?).execute(&state.db.pool).await?;
    Ok(Json(req))
}
