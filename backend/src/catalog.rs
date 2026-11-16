use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use serde_json::Value;
use sqlx::{PgPool, Row};

use crate::{
    auth::UserSessionAuth,
    billing::{ImagePriceTier, VideoPriceTier, BILLABLE_PRICE_CONDITION_CP},
    error::AppResult,
    AppState,
};

mod health;

#[derive(Serialize)]
struct Catalog {
    currency: String,
    personalized: bool,
    models: Vec<ModelOffer>,
}

#[derive(Serialize)]
struct ModelOffer {
    model: String,
    display_name: String,
    provider: String,
    provider_name: String,
    billing_meter: String,
    input_price_micros: i64,
    output_price_micros: i64,
    cache_read_price_micros: Option<i64>,
    cache_write_price_micros: Option<i64>,
    unit_price_micros: Option<i64>,
    video_billing_mode: Option<String>,
    video_price_tiers: Vec<VideoPriceTier>,
    image_price_tiers: Vec<ImagePriceTier>,
    capabilities: Value,
    protocols: Vec<String>,
}

pub(crate) fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/public/models", get(public_models))
        .route("/api/user/models", get(user_models))
        .route("/api/public/model-health", get(health::public_model_health))
}

async fn public_models(State(state): State<Arc<AppState>>) -> AppResult<Json<Catalog>> {
    catalog(&state, None).await.map(Json)
}

async fn user_models(
    State(state): State<Arc<AppState>>,
    auth: UserSessionAuth,
) -> AppResult<Json<Catalog>> {
    catalog(&state, Some(auth.user_id)).await.map(Json)
}

fn scaled(value: i64, multiplier: i64) -> i64 {
    ((i128::from(value) * i128::from(multiplier) + 500_000) / 1_000_000)
        .clamp(0, i128::from(i64::MAX)) as i64
}

async fn catalog(state: &AppState, user_id: Option<crate::id::DbId>) -> AppResult<Catalog> {
    let multiplier = if let Some(user_id) = user_id {
        sqlx::query_scalar::<_, i64>(
            r#"SELECT pp.multiplier_micros FROM pricing_policy pp
               JOIN user_group g ON g.code = pp.user_group
               JOIN "user" u ON u.user_group_id = g.id
               WHERE u.id = $1 AND pp.enabled = TRUE
               ORDER BY pp.priority DESC, pp.id DESC LIMIT 1"#,
        )
        .bind(user_id)
        .fetch_optional(&state.db.pool)
        .await?
        .unwrap_or(1_000_000)
    } else {
        1_000_000
    };
    let models = load_offers(&state.db.pool, multiplier).await?;
    Ok(Catalog {
        currency: state.config.billing_currency.as_str().to_string(),
        personalized: user_id.is_some(),
        models,
    })
}

/// Use the exact same publication checks for packages and the model plaza.
pub(crate) async fn available_models(pool: &PgPool) -> AppResult<Vec<(String, String)>> {
    let offers = load_offers(pool, 1_000_000).await?;
    let mut models = std::collections::BTreeSet::new();
    for offer in offers {
        let family =
            model_family(&offer.model, &offer.billing_meter, &offer.capabilities).to_string();
        models.insert((offer.model, family));
    }
    Ok(models.into_iter().collect())
}

pub(crate) fn model_family(model: &str, meter: &str, capabilities: &Value) -> &'static str {
    if model.to_ascii_lowercase().contains("moderation")
        || ["moderation", "moderations"]
            .iter()
            .any(|key| capabilities.get(key).and_then(Value::as_bool) == Some(true))
    {
        "moderation"
    } else if ["embeddings", "embedding"]
        .iter()
        .any(|key| capabilities.get(key).and_then(Value::as_bool) == Some(true))
        || model.to_ascii_lowercase().contains("embedding")
    {
        "embedding"
    } else if meter == "video"
        || capabilities
            .get("video_generation")
            .and_then(Value::as_bool)
            == Some(true)
    {
        "video"
    } else if meter == "audio"
        || ["audio_speech", "audio_transcription"]
            .iter()
            .any(|key| capabilities.get(key).and_then(Value::as_bool) == Some(true))
    {
        "audio"
    } else if meter == "image"
        || capabilities
            .get("image_generation")
            .and_then(Value::as_bool)
            == Some(true)
    {
        "image"
    } else {
        name_family(model)
    }
}

// Families are matched on exact model names at key creation. We never put
// wildcards or provider prefixes into a key's authorization list.
fn name_family(model: &str) -> &'static str {
    let full_model = model.to_ascii_lowercase();
    let model = full_model.rsplit('/').next().unwrap_or(&full_model);
    if [
        "sora", "veo", "kling", "wan", "runway", "seedance", "hailuo", "luma", "gen4-", "gen3-",
    ]
    .iter()
    .any(|p| model.starts_with(p))
        || model.contains("seedance")
        || model.starts_with("cogvideo")
    {
        "video"
    } else if [
        "dall-e",
        "gpt-image",
        "imagen",
        "flux",
        "stable-diffusion",
        "sdxl",
        "midjourney",
        "ideogram",
        "seedream",
        "nano-banana",
        "qwen-image",
        "glm-image",
        "cogview",
        "hunyuan-image",
    ]
    .iter()
    .any(|p| model.starts_with(p))
        || model.contains("seedream")
        || (model.starts_with("gemini") && model.contains("image"))
    {
        "image"
    } else if model.contains("tts")
        || model.contains("whisper")
        || model.contains("speech")
        || model.contains("audio")
        || model.starts_with("cosyvoice")
        || model.starts_with("eleven_")
    {
        "audio"
    } else if model.starts_with("claude") {
        "claude"
    } else if model.starts_with("gemini") || model.starts_with("gemma") {
        "gemini"
    } else if model.starts_with("deepseek") {
        "deepseek"
    } else if model.starts_with("qwen") || model.starts_with("qwq") {
        "qwen"
    } else if model.starts_with("kimi") || model.starts_with("moonshot") {
        "kimi"
    } else if model.starts_with("glm") || model.starts_with("chatglm") {
        "glm"
    } else if model.starts_with("doubao") {
        "doubao"
    } else if model.starts_with("gpt-")
        || model.starts_with("chatgpt-")
        || model.starts_with("o1")
        || model.starts_with("o3")
        || model.starts_with("o4")
        || model.starts_with("text-embedding-")
    {
        "chatgpt"
    } else {
        "custom"
    }
}

async fn load_offers(pool: &PgPool, multiplier: i64) -> AppResult<Vec<ModelOffer>> {
    // Explicit projection: no endpoint URLs, channel names, secrets or procurement quotes.
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        r#"SELECT cp.model, COALESCE(NULLIF(pm.display_name, ''), cp.model) AS display_name,
            c.provider, p.display_name AS provider_name, cp.billing_meter,
            cp.input_price_micros, cp.output_price_micros,
            cp.cache_read_price_micros, cp.cache_write_price_micros,
            cp.unit_price_micros, cp.video_billing_mode, cp.video_price_tiers, cp.image_price_tiers,
            COALESCE(pm.capabilities, '{{}}'::jsonb) AS capabilities,
            ARRAY(SELECT DISTINCT ce.protocol FROM channel_endpoint ce
                WHERE ce.channel_id = c.id AND ce.enabled AND ce.healthy
                AND (ce.cooldown_until IS NULL OR ce.cooldown_until <= now())
                AND (NOT EXISTS (SELECT 1 FROM unnest(ce.models) x WHERE btrim(x) <> '')
                     OR EXISTS (SELECT 1 FROM unnest(ce.models) x WHERE btrim(x) = cp.model))
                ORDER BY ce.protocol) AS protocols
        FROM channel_price cp
        JOIN channel c ON c.id = cp.channel_id AND c.enabled
        JOIN provider p ON p.code = c.provider AND p.enabled
        JOIN channel_model cm ON cm.channel_id = c.id AND cm.model = cp.model
        LEFT JOIN provider_model pm ON pm.provider = c.provider AND pm.model = cp.model
        WHERE cp.enabled AND {BILLABLE_PRICE_CONDITION_CP}
            AND cm.enabled AND cm.status = 'available'
            AND (cm.runtime_status = 'normal' OR
                (cm.runtime_status = 'cooldown' AND cm.cooldown_until <= now()))
            AND EXISTS (SELECT 1 FROM channel_endpoint ce WHERE ce.channel_id = c.id
                AND ce.enabled AND ce.healthy
                AND (ce.cooldown_until IS NULL OR ce.cooldown_until <= now())
                AND (NOT EXISTS (SELECT 1 FROM unnest(ce.models) x WHERE btrim(x) <> '')
                     OR EXISTS (SELECT 1 FROM unnest(ce.models) x WHERE btrim(x) = cp.model)))
            AND ((NOT c.use_credentials AND EXISTS (SELECT 1 FROM channel_key ck
                WHERE ck.channel_id = c.id AND ck.enabled AND ck.healthy
                AND (ck.cooldown_until IS NULL OR ck.cooldown_until <= now())))
                OR (c.use_credentials AND EXISTS (SELECT 1 FROM credential cr
                    WHERE cr.provider = c.provider AND cr.enabled)))
        ORDER BY c.provider, cp.model, cp.id"#
    )))
    .fetch_all(pool)
    .await?;
    let mut models = Vec::with_capacity(rows.len());
    for row in rows {
        let model: String = row.try_get("model")?;
        let meter: String = row.try_get("billing_meter")?;
        let capabilities: Value = row.try_get("capabilities")?;
        let mut protocols: Vec<String> = row.try_get("protocols")?;
        let family = model_family(&model, &meter, &capabilities);
        // Specialized image/video/audio/embedding routes accept OpenAI endpoints
        // only. A healthy Anthropic/OAuth endpoint cannot serve these routes.
        if matches!(
            family,
            "image" | "video" | "audio" | "embedding" | "moderation"
        ) {
            protocols.retain(|protocol| protocol == "openai");
        }
        if protocols.is_empty() {
            continue;
        }
        let tiers: Value = row.try_get("video_price_tiers")?;
        let mut tiers: Vec<VideoPriceTier> = serde_json::from_value(tiers)?;
        for tier in &mut tiers {
            tier.input_with_video_micros =
                tier.input_with_video_micros.map(|v| scaled(v, multiplier));
            tier.input_without_video_micros = tier
                .input_without_video_micros
                .map(|v| scaled(v, multiplier));
            tier.input_with_video_unit_micros = tier
                .input_with_video_unit_micros
                .map(|v| scaled(v, multiplier));
            tier.input_without_video_unit_micros = tier
                .input_without_video_unit_micros
                .map(|v| scaled(v, multiplier));
        }
        models.push(ModelOffer {
            model,
            display_name: row.try_get("display_name")?,
            provider: row.try_get("provider")?,
            provider_name: row.try_get("provider_name")?,
            billing_meter: meter,
            input_price_micros: scaled(row.try_get("input_price_micros")?, multiplier),
            output_price_micros: scaled(row.try_get("output_price_micros")?, multiplier),
            cache_read_price_micros: row
                .try_get::<Option<i64>, _>("cache_read_price_micros")?
                .map(|v| scaled(v, multiplier)),
            cache_write_price_micros: row
                .try_get::<Option<i64>, _>("cache_write_price_micros")?
                .map(|v| scaled(v, multiplier)),
            unit_price_micros: row
                .try_get::<Option<i64>, _>("unit_price_micros")?
                .map(|v| scaled(v, multiplier)),
            video_billing_mode: row.try_get("video_billing_mode")?,
            video_price_tiers: tiers,
            image_price_tiers: serde_json::from_value::<Vec<ImagePriceTier>>(
                row.try_get("image_price_tiers")?,
            )?
            .into_iter()
            .map(|mut tier| {
                tier.unit_price_micros = scaled(tier.unit_price_micros, multiplier);
                tier
            })
            .collect(),
            capabilities,
            protocols,
        });
    }
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL"]
    async fn specialized_models_require_compatible_protocols_in_plaza_and_packages(pool: PgPool) {
        sqlx::raw_sql(r#"
            INSERT INTO provider_model (provider,model,display_name,capabilities)
              VALUES ('openai','gpt-image-catalog','Image','{"image_generation":true}'),
                     ('openai','text-embedding-catalog','Embedding','{"embeddings":true}'),
                     ('openai','omni-moderation-catalog','Moderation','{"moderations":true}');
            INSERT INTO channel (id,provider,name) VALUES (92001,'openai','protocol test');
            INSERT INTO channel_model (channel_id,model,enabled) VALUES (92001,'gpt-image-catalog',true),(92001,'text-embedding-catalog',true),(92001,'omni-moderation-catalog',true);
            INSERT INTO channel_endpoint (channel_id,protocol,base_url) VALUES (92001,'anthropic','https://example.invalid');
            INSERT INTO channel_key (channel_id,name,key_prefix,secret_ciphertext) VALUES (92001,'key','test','encrypted');
            INSERT INTO channel_price (channel_id,model,input_price_micros,output_price_micros)
              VALUES (92001,'gpt-image-catalog',1000000,1000000),(92001,'text-embedding-catalog',1000000,1000000),(92001,'omni-moderation-catalog',1000000,1000000);
        "#).execute(&pool).await.unwrap();
        assert!(load_offers(&pool, 1_000_000).await.unwrap().is_empty());
        assert!(available_models(&pool).await.unwrap().is_empty());
        sqlx::query("UPDATE channel_endpoint SET protocol='openai' WHERE channel_id=92001")
            .execute(&pool)
            .await
            .unwrap();
        let models = available_models(&pool).await.unwrap();
        assert_eq!(
            models,
            vec![
                ("gpt-image-catalog".into(), "image".into()),
                ("omni-moderation-catalog".into(), "moderation".into()),
                ("text-embedding-catalog".into(), "embedding".into())
            ]
        );
        sqlx::query("UPDATE channel_key SET enabled=false WHERE channel_id=92001")
            .execute(&pool)
            .await
            .unwrap();
        assert!(available_models(&pool).await.unwrap().is_empty());
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL"]
    async fn media_tiers_catalog_matches_billing_and_preserves_legacy_prices(pool: PgPool) {
        sqlx::raw_sql(r#"
            INSERT INTO provider_model (provider,model,display_name,capabilities)
              VALUES ('openai','gpt-image-tier-test','Image tiers','{"image_generation":true}');
            INSERT INTO channel (id,provider,name) VALUES (93001,'openai','private tier supplier');
            INSERT INTO channel_model (channel_id,model,enabled) VALUES (93001,'gpt-image-tier-test',true);
            INSERT INTO channel_endpoint (channel_id,protocol,base_url) VALUES (93001,'openai','https://private.invalid');
            INSERT INTO channel_key (channel_id,name,key_prefix,secret_ciphertext) VALUES (93001,'key','secret','encrypted');
            INSERT INTO channel_price (channel_id,model,input_price_micros,output_price_micros,billing_meter,unit_price_micros,image_price_tiers)
              VALUES (93001,'gpt-image-tier-test',0,0,'image',12345,'[{"size":"1k","quality":"*","unit_price_micros":12345},{"size":"2k","quality":"high","unit_price_micros":56789}]');
            INSERT INTO user_group(code,name) VALUES ('tier-test','Tier test');
            INSERT INTO pricing_policy (name,user_group,multiplier_micros,enabled,priority) VALUES ('media-tier-test','tier-test',800000,true,999);
        "#).execute(&pool).await.unwrap();
        let offers = load_offers(&pool, 800000).await.unwrap();
        let offer = offers
            .iter()
            .find(|m| m.model == "gpt-image-tier-test")
            .unwrap();
        assert_eq!(offer.image_price_tiers[0].unit_price_micros, 9876);
        let engine = crate::billing::Billing::new_memory(std::time::Duration::ZERO, 100, 1, 1);
        let mut quote = engine
            .price_for(&pool, 93001, "gpt-image-tier-test", "tier-test")
            .await
            .unwrap();
        crate::billing::image::apply_tier(&mut quote, Some("2048x2048"), Some("high")).unwrap();
        assert_eq!(
            quote.unit_price_micros,
            Some(offer.image_price_tiers[1].unit_price_micros)
        );
        assert_eq!(
            crate::billing::cost_for_billable_usage(
                crate::billing::BillableUsage::image(3),
                &quote
            ),
            136293
        );
        sqlx::query("UPDATE channel_price SET image_price_tiers='[]' WHERE channel_id=93001")
            .execute(&pool)
            .await
            .unwrap();
        let mut legacy = engine
            .price_for(&pool, 93001, "gpt-image-tier-test", "tier-test")
            .await
            .unwrap();
        crate::billing::image::apply_tier(&mut legacy, None, None).unwrap();
        assert_eq!(legacy.unit_price_micros, Some(9876));
    }

    #[test]
    fn catalog_multiplier_matches_billing_rounding_without_overflow() {
        assert_eq!(scaled(3, 500_000), 2);
        assert_eq!(scaled(1_000_000, 800_000), 800_000);
        assert_eq!(scaled(20, 0), 0);
        assert_eq!(scaled(i64::MAX, i64::MAX), i64::MAX);
    }

    #[tokio::test]
    async fn personalized_catalog_requires_a_user_session() {
        use axum::{
            body::Body,
            http::{Request, StatusCode},
        };
        use tower::ServiceExt;
        let app = router().with_state(crate::app::tests::test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/user/models")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL: DATABASE_URL=postgres://localhost/postgres cargo test catalog_publication -- --ignored"]
    async fn catalog_publication(pool: PgPool) {
        sqlx::raw_sql(
            r#"
            INSERT INTO provider_model (provider, model, display_name, enabled)
            VALUES ('openai', 'catalog-test', 'Test model', TRUE);
            INSERT INTO channel (id, provider, name, enabled) VALUES
              (91001, 'openai', 'private supplier A', TRUE),
              (91002, 'openai', 'disabled channel', FALSE),
              (91003, 'openai', 'no valid key', TRUE),
              (91004, 'openai', 'endpoint model mismatch', TRUE);
            INSERT INTO channel_model (channel_id, model, enabled)
              SELECT id, 'catalog-test', TRUE FROM channel WHERE id >= 91001;
            INSERT INTO channel_endpoint (channel_id, protocol, base_url, models)
              SELECT id, 'openai', 'https://private-supplier.invalid',
                CASE WHEN id = 91004 THEN ARRAY['other-model'] ELSE ARRAY[' catalog-test '] END
              FROM channel WHERE id >= 91001;
            INSERT INTO channel_key (channel_id, name, key_prefix, secret_ciphertext, healthy)
              SELECT id, 'private key', 'secret', 'encrypted', id <> 91003
              FROM channel WHERE id >= 91001;
            INSERT INTO channel_price (channel_id, model, input_price_micros, output_price_micros)
              SELECT id, 'catalog-test', 1000000, 3000000 FROM channel WHERE id >= 91001;
            INSERT INTO channel_procurement (channel_id, model, supplier, input_cost_micros)
              VALUES (91001, 'catalog-test', 'private procurement supplier', 123456);
        "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        let models = load_offers(&pool, 800000).await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].input_price_micros, 800000);
        assert_eq!(models[0].output_price_micros, 2400000);
        assert_eq!(models[0].protocols, vec!["openai"]);
        let public = serde_json::to_string(&models).unwrap();
        for private in [
            "private",
            "encrypted",
            "secret",
            "123456",
            "channel_id",
            "cost_micros",
        ] {
            assert!(!public.contains(private), "public catalog leaked {private}");
        }
        sqlx::query("UPDATE channel_price SET enabled = FALSE WHERE channel_id = 91001")
            .execute(&pool)
            .await
            .unwrap();
        assert!(load_offers(&pool, 1000000).await.unwrap().is_empty());
    }
}
