//! OAuth credentials stay encrypted at rest; authorization codes, access tokens
//! and session tokens never enter frontend URLs. Identity linking is explicit:
//! matching a local email alone never attaches a new provider to that account.
use std::{sync::Arc, time::Duration};

use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::{header, HeaderMap, HeaderValue},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::distr::{Alphanumeric, SampleString};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Row;

use super::{
    consume_login_verification_code, ensure_enabled, hash_key, is_system_apps_email,
    lock_user_login, login_response, normalize_email, verify_user_password, AdminAuth, CodePurpose,
    UserLogin, UserSessionAuth,
};
use crate::{
    error::{AppError, AppResult},
    id::DbId,
    policy::{registration_policy, ServiceMode},
    project,
    secrets::SecretStore,
    AppState,
};

const COOKIE_NAME: &str = "zhiqiao_oauth_flow";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

// This client is used only for fixed GitHub/Google OAuth endpoints. Relay
// traffic keeps its existing client and never inherits the OAuth proxy.
fn oauth_http_client() -> AppResult<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(8))
        .timeout(REQUEST_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none());
    if let Some(proxy) = std::env::var("OAUTH_PROXY_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
    {
        let proxy = reqwest::Proxy::all(proxy.trim())
            .map_err(|_| AppError::BadRequest("OAuth proxy configuration is invalid".into()))?;
        builder = builder.proxy(proxy);
    }
    builder
        .build()
        .map_err(|_| AppError::BadRequest("OAuth HTTP client configuration is invalid".into()))
}

pub(super) fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/auth/providers", get(providers))
        .route("/api/auth/oauth/{provider}/start", get(start))
        .route("/api/auth/oauth/{provider}/callback", get(callback))
        .route("/oauth/{provider}/callback", get(callback))
        .route("/api/auth/oauth/exchange", post(exchange))
        .route("/api/auth/oauth/confirm-link", post(confirm_link))
        .route("/api/user/oauth/identities", get(identities))
        .route("/api/user/oauth/{provider}/link", post(start_link))
        .route(
            "/api/admin/settings/oauth",
            get(settings).post(save_settings),
        )
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Provider {
    Github,
    Google,
}
impl Provider {
    fn name(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Google => "google",
        }
    }
    fn authorize_url(self) -> &'static str {
        match self {
            Self::Github => "https://github.com/login/oauth/authorize",
            Self::Google => "https://accounts.google.com/o/oauth2/v2/auth",
        }
    }
    fn token_url(self) -> &'static str {
        match self {
            Self::Github => "https://github.com/login/oauth/access_token",
            Self::Google => "https://oauth2.googleapis.com/token",
        }
    }
    fn scope(self) -> &'static str {
        match self {
            Self::Github => "read:user user:email",
            Self::Google => "openid email profile",
        }
    }
}

struct ProviderConfig {
    enabled: bool,
    client_id: String,
    ciphertext: Option<String>,
}
impl ProviderConfig {
    fn configured(&self) -> bool {
        self.enabled && !self.client_id.is_empty() && self.ciphertext.is_some()
    }
}

#[derive(Serialize)]
struct ProviderSetting {
    enabled: bool,
    client_id: String,
    client_secret_set: bool,
    callback_url: String,
}
#[derive(Serialize)]
struct Settings {
    github: ProviderSetting,
    google: ProviderSetting,
}
#[derive(Deserialize)]
struct ProviderInput {
    enabled: bool,
    client_id: String,
    client_secret: Option<String>,
    #[serde(default)]
    clear_client_secret: bool,
}
#[derive(Deserialize)]
struct SettingsInput {
    github: ProviderInput,
    google: ProviderInput,
}

async fn config(state: &AppState, provider: Provider) -> AppResult<ProviderConfig> {
    let row = sqlx::query("SELECT enabled,client_id,client_secret_ciphertext FROM auth_oauth_provider WHERE provider=$1")
        .bind(provider.name()).fetch_one(&state.db.pool).await?;
    Ok(ProviderConfig {
        enabled: row.try_get("enabled")?,
        client_id: row.try_get("client_id")?,
        ciphertext: row.try_get("client_secret_ciphertext")?,
    })
}
fn public_base(state: &AppState) -> AppResult<reqwest::Url> {
    let raw = state
        .config
        .public_base_url
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("PUBLIC_BASE_URL is required for OAuth".into()))?;
    let mut url = reqwest::Url::parse(raw)
        .map_err(|_| AppError::BadRequest("invalid PUBLIC_BASE_URL".into()))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(AppError::BadRequest(
            "OAuth requires an HTTPS public base URL".into(),
        ));
    }
    url.set_path("/");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}
fn callback_url(state: &AppState, provider: Provider) -> AppResult<String> {
    let mut url = public_base(state)?;
    url.set_path(&format!("/oauth/{}/callback", provider.name()));
    Ok(url.to_string())
}
async fn providers(State(state): State<Arc<AppState>>) -> AppResult<Json<serde_json::Value>> {
    let https = public_base(&state).is_ok();
    Ok(Json(
        serde_json::json!({ "github": https && config(&state,Provider::Github).await?.configured(), "google": https && config(&state,Provider::Google).await?.configured() }),
    ))
}
async fn settings(
    _auth: AdminAuth,
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Settings>> {
    let entry = |p: Provider, c: ProviderConfig| ProviderSetting {
        enabled: c.enabled,
        client_id: c.client_id,
        client_secret_set: c.ciphertext.is_some(),
        callback_url: callback_url(&state, p).unwrap_or_default(),
    };
    Ok(Json(Settings {
        github: entry(Provider::Github, config(&state, Provider::Github).await?),
        google: entry(Provider::Google, config(&state, Provider::Google).await?),
    }))
}
async fn save_settings(
    auth: AdminAuth,
    State(state): State<Arc<AppState>>,
    Json(req): Json<SettingsInput>,
) -> AppResult<Json<Settings>> {
    let role: String = sqlx::query_scalar("SELECT role FROM admin WHERE id=$1")
        .bind(auth.admin_id)
        .fetch_one(&state.db.pool)
        .await?;
    if role == "viewer" {
        return Err(AppError::Forbidden("只读管理员无法修改此设置".into()));
    }
    let mut tx = state.db.pool.begin().await?;
    for (provider, input) in [
        (Provider::Github, req.github),
        (Provider::Google, req.google),
    ] {
        let client_id = input.client_id.trim();
        if client_id.len() > 512 || input.client_secret.as_ref().is_some_and(|s| s.len() > 4096) {
            return Err(AppError::BadRequest(
                "OAuth credentials are too long".into(),
            ));
        }
        let old: Option<String> = sqlx::query_scalar(
            "SELECT client_secret_ciphertext FROM auth_oauth_provider WHERE provider=$1 FOR UPDATE",
        )
        .bind(provider.name())
        .fetch_one(&mut *tx)
        .await?;
        let secret = if input.clear_client_secret {
            None
        } else if let Some(s) = input
            .client_secret
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(state.secrets.encrypt(s)?)
        } else {
            old
        };
        if input.enabled && (client_id.is_empty() || secret.is_none()) {
            return Err(AppError::BadRequest(
                "enabled OAuth providers require a client ID and secret".into(),
            ));
        }
        if input.enabled {
            public_base(&state)?;
        }
        sqlx::query("UPDATE auth_oauth_provider SET enabled=$2,client_id=$3,client_secret_ciphertext=$4,updated_at=now() WHERE provider=$1")
            .bind(provider.name()).bind(input.enabled).bind(client_id).bind(secret).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    settings(auth, State(state)).await
}

#[derive(Deserialize)]
struct StartQuery {
    #[serde(default = "default_intent")]
    intent: String,
    #[serde(default)]
    accept_terms: bool,
}
fn default_intent() -> String {
    "login".into()
}
fn random_token() -> String {
    Alphanumeric.sample_string(&mut rand::rng(), 64)
}
fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}
fn cookie(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (key, val) = part.trim().split_once('=')?;
            (key == COOKIE_NAME
                && val.len() == 64
                && val.bytes().all(|b| b.is_ascii_alphanumeric()))
            .then_some(val)
        })
}
fn with_cookie(response: impl IntoResponse, value: &str, max_age: u32) -> Response {
    let mut response = response.into_response();
    let value =
        format!("{COOKIE_NAME}={value}; Path=/; Max-Age={max_age}; HttpOnly; Secure; SameSite=Lax");
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&value).expect("generated cookie is ASCII"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}
async fn start(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<Provider>,
    Query(req): Query<StartQuery>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let key = super::client_rate_key(&headers, Some(addr.ip()), state.config.trust_proxy_headers);
    state
        .auth_rate_limiter
        .check_password_login(&format!("oauth-start:{key}"), &key)
        .await?;
    if !matches!(req.intent.as_str(), "login" | "register") {
        return Err(AppError::BadRequest("invalid OAuth intent".into()));
    }
    if req.intent == "register" {
        if !req.accept_terms {
            return Err(AppError::BadRequestWithCode {
                code: "terms_required",
                message: "accept the user agreement and privacy policy",
            });
        }
        if !registration_policy(&state).await?.1 {
            return Err(AppError::BadRequestWithCode {
                code: "registration_closed",
                message: "registration is closed",
            });
        }
    }
    create_flow(&state, provider, &req.intent, None).await
}

#[derive(Serialize)]
struct LinkedProvider {
    provider: &'static str,
    enabled: bool,
    linked: bool,
}
async fn identities(
    auth: UserSessionAuth,
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<Vec<LinkedProvider>>> {
    let linked: Vec<String> =
        sqlx::query_scalar("SELECT provider FROM user_oauth_identity WHERE user_id=$1")
            .bind(auth.user_id)
            .fetch_all(&state.db.pool)
            .await?;
    let mut result = Vec::new();
    for provider in [Provider::Github, Provider::Google] {
        result.push(LinkedProvider {
            provider: provider.name(),
            enabled: public_base(&state).is_ok() && config(&state, provider).await?.configured(),
            linked: linked.iter().any(|name| name == provider.name()),
        });
    }
    Ok(Json(result))
}
async fn start_link(
    auth: UserSessionAuth,
    State(state): State<Arc<AppState>>,
    Path(provider): Path<Provider>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let session = super::session::user_session_from_headers(&state, &headers).await?;
    state
        .auth_rate_limiter
        .check_password_login(
            &format!("oauth-link:{}", auth.user_id),
            &format!("oauth-link:{}", auth.user_id),
        )
        .await?;
    create_flow(
        &state,
        provider,
        "link",
        Some((session.user_id, session.auth_version)),
    )
    .await
}
async fn create_flow(
    state: &AppState,
    provider: Provider,
    intent: &str,
    link_session: Option<(DbId, i64)>,
) -> AppResult<Response> {
    let c = config(state, provider).await?;
    if !c.configured() {
        return Err(AppError::BadRequestWithCode {
            code: "oauth_not_configured",
            message: "this login provider is not configured",
        });
    }
    let callback = callback_url(&state, provider)?;
    let flow = random_token();
    let browser = random_token();
    let verifier = random_token();
    sqlx::query("DELETE FROM auth_oauth_flow WHERE expires_at < now()")
        .execute(&state.db.pool)
        .await?;
    sqlx::query("DELETE FROM auth_oauth_ticket WHERE expires_at < now()")
        .execute(&state.db.pool)
        .await?;
    sqlx::query("INSERT INTO auth_oauth_flow(state_hash,browser_hash,provider,verifier_ciphertext,intent,link_user_id,link_auth_version,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7,now()+interval '10 minutes')")
        .bind(hash_key(&flow)).bind(hash_key(&browser)).bind(provider.name()).bind(state.secrets.encrypt(&verifier)?).bind(intent).bind(link_session.map(|s|s.0)).bind(link_session.map(|s|s.1)).execute(&state.db.pool).await?;
    let mut url = reqwest::Url::parse(provider.authorize_url()).expect("static HTTPS URL");
    url.query_pairs_mut()
        .append_pair("client_id", &c.client_id)
        .append_pair("redirect_uri", &callback)
        .append_pair("response_type", "code")
        .append_pair("scope", provider.scope())
        .append_pair("state", &flow)
        .append_pair("code_challenge", &pkce_challenge(&verifier))
        .append_pair("code_challenge_method", "S256");
    if intent == "link" {
        Ok(with_cookie(
            Json(serde_json::json!({"url": url.as_str()})),
            &browser,
            600,
        ))
    } else {
        Ok(with_cookie(Redirect::to(url.as_str()), &browser, 600))
    }
}

#[derive(Deserialize)]
struct CallbackQuery {
    state: Option<String>,
    code: Option<String>,
    error: Option<String>,
}
async fn callback(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<Provider>,
    Query(req): Query<CallbackQuery>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let result = finish(&state, provider, req, &headers).await;
    let mut target = public_base(&state)?;
    target.set_path("/oauth/callback");
    match result {
        Ok(FinishedFlow::Session { ticket, linked }) => {
            target.set_fragment(Some(&format!("ticket={ticket}&linked={linked}")));
            Ok(with_cookie(
                Redirect::to(target.as_str()),
                cookie(&headers).unwrap_or(""),
                120,
            ))
        }
        Ok(FinishedFlow::ConfirmLink { ticket }) => {
            target.set_fragment(Some(&format!("connect={ticket}")));
            Ok(with_cookie(
                Redirect::to(target.as_str()),
                cookie(&headers).unwrap_or(""),
                600,
            ))
        }
        Err(error) => {
            // Use fixed public codes only. Never echo a provider response, token,
            // authorization code or internal SQL/HTTP details into a redirect.
            let code = match &error {
                AppError::ConflictWithCode { code, .. }
                | AppError::BadRequestWithCode { code, .. } => *code,
                AppError::Reqwest(error) => {
                    tracing::warn!(provider = provider.name(), timeout = error.is_timeout(), connect = error.is_connect(), status = ?error.status(), "OAuth provider connection failed");
                    "oauth_provider_unavailable"
                }
                _ => "oauth_failed",
            };
            tracing::warn!(
                provider = provider.name(),
                reason = code,
                "OAuth sign in did not complete"
            );
            target.set_fragment(Some(&format!("error={code}")));
            Ok(with_cookie(Redirect::to(target.as_str()), "", 0))
        }
    }
}

enum FinishedFlow {
    Session { ticket: String, linked: bool },
    ConfirmLink { ticket: String },
}

async fn finish(
    state: &AppState,
    provider: Provider,
    req: CallbackQuery,
    headers: &HeaderMap,
) -> AppResult<FinishedFlow> {
    let browser = cookie(headers).ok_or(AppError::Unauthorized)?;
    let flow = req
        .state
        .as_deref()
        .filter(|s| s.len() == 64)
        .ok_or(AppError::Unauthorized)?;
    let row=sqlx::query("DELETE FROM auth_oauth_flow WHERE state_hash=$1 AND browser_hash=$2 AND provider=$3 AND expires_at>now() RETURNING verifier_ciphertext,intent,link_user_id,link_auth_version")
        .bind(hash_key(flow)).bind(hash_key(browser)).bind(provider.name()).fetch_optional(&state.db.pool).await?.ok_or(AppError::Unauthorized)?;
    if req.error.is_some() {
        return Err(AppError::BadRequestWithCode {
            code: "oauth_cancelled",
            message: "authorization was cancelled",
        });
    }
    let code = req
        .code
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or(AppError::Unauthorized)?;
    let c = config(state, provider).await?;
    if !c.configured() {
        return Err(AppError::Unauthorized);
    }
    let store = SecretStore::new(&state.config.upstream_secret_key, 0);
    let secret = store.plaintext(0, c.ciphertext.as_deref().ok_or(AppError::Unauthorized)?)?;
    let verifier = store.plaintext(0, &row.try_get::<String, _>("verifier_ciphertext")?)?;
    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: Option<String>,
    }
    let redirect_uri = callback_url(state, provider)?;
    let token_body = serde_urlencoded::to_string([
        ("client_id", c.client_id.as_str()),
        ("client_secret", secret.as_str()),
        ("code", code.as_str()),
        ("redirect_uri", redirect_uri.as_str()),
        ("grant_type", "authorization_code"),
        ("code_verifier", verifier.as_str()),
    ])
    .map_err(|_| AppError::BadRequest("invalid OAuth request".into()))?;
    let client = oauth_http_client()?;
    let response = client
        .post(provider.token_url())
        .timeout(REQUEST_TIMEOUT)
        .header(header::ACCEPT, "application/json")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(token_body)
        .send()
        .await?
        .error_for_status()?
        .json::<TokenResponse>()
        .await?;
    let access = response
        .access_token
        .filter(|s| !s.is_empty() && s.len() <= 16384)
        .ok_or(AppError::Unauthorized)?;
    let (subject, email) = verified_identity(&client, provider, &access).await?;
    let email = normalize_email(&email)?;
    if is_system_apps_email(&email) {
        return Err(AppError::Unauthorized);
    }
    let intent: String = row.try_get("intent")?;
    let linked = intent == "link";
    let user = if linked {
        link_identity(
            state,
            provider,
            &subject,
            row.try_get("link_user_id")?,
            row.try_get("link_auth_version")?,
        )
        .await?
    } else {
        match identity_user(state, provider, &subject, &email, &intent).await {
            Ok(user) => user,
            Err(AppError::ConflictWithCode {
                code: "oauth_account_exists",
                ..
            }) => {
                let ticket = random_token();
                sqlx::query("DELETE FROM auth_oauth_pending_link WHERE expires_at < now() OR browser_hash=$1")
                    .bind(hash_key(browser)).execute(&state.db.pool).await?;
                sqlx::query("INSERT INTO auth_oauth_pending_link(ticket_hash,browser_hash,provider,subject,email,expires_at) VALUES($1,$2,$3,$4,$5,now()+interval '10 minutes')")
                    .bind(hash_key(&ticket)).bind(hash_key(browser)).bind(provider.name()).bind(&subject).bind(&email).execute(&state.db.pool).await?;
                return Ok(FinishedFlow::ConfirmLink { ticket });
            }
            Err(error) => return Err(error),
        }
    };
    let ticket = random_token();
    sqlx::query("INSERT INTO auth_oauth_ticket(ticket_hash,browser_hash,user_id,auth_version,expires_at) VALUES($1,$2,$3,$4,now()+interval '60 seconds')")
        .bind(hash_key(&ticket)).bind(hash_key(browser)).bind(user.user_id).bind(user.auth_version).execute(&state.db.pool).await?;
    Ok(FinishedFlow::Session { ticket, linked })
}

#[derive(Deserialize)]
struct ConfirmLinkRequest {
    ticket: String,
    email: String,
    password: Option<String>,
    verification_code: Option<String>,
}

async fn confirm_link(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<ConfirmLinkRequest>,
) -> AppResult<Response> {
    let email = normalize_email(&req.email)?;
    let client_key =
        super::client_rate_key(&headers, Some(addr.ip()), state.config.trust_proxy_headers);
    state
        .auth_rate_limiter
        .check_password_login(&email, &client_key)
        .await?;
    if req.verification_code.is_some() {
        state
            .auth_rate_limiter
            .check_login_verification_attempt(&email)
            .await?;
    }
    let browser = cookie(&headers).ok_or(AppError::Unauthorized)?;
    let user = confirm_pending_identity(&state, browser, &email, &req).await?;
    Ok(with_cookie(Json(login_response(&state, user)), "", 0))
}

async fn confirm_pending_identity(
    state: &AppState,
    browser: &str,
    email: &str,
    req: &ConfirmLinkRequest,
) -> AppResult<UserLogin> {
    if req.ticket.len() != 64 || is_system_apps_email(email) {
        return Err(AppError::Unauthorized);
    }
    let mut tx = state.db.pool.begin().await?;
    lock_user_login(&mut tx, email).await?;
    let pending = sqlx::query("SELECT provider,subject FROM auth_oauth_pending_link WHERE ticket_hash=$1 AND browser_hash=$2 AND email=$3 AND expires_at>now() FOR UPDATE")
        .bind(hash_key(&req.ticket)).bind(hash_key(browser)).bind(email).fetch_optional(&mut *tx).await?.ok_or(AppError::Unauthorized)?;
    let row = sqlx::query(r#"SELECT id,status,password_hash,auth_version,(password_changed_at IS NULL) AS requires_password_change FROM "user" WHERE email=$1 FOR UPDATE"#)
        .bind(email).fetch_optional(&mut *tx).await?.ok_or(AppError::Unauthorized)?;
    match (&req.password, &req.verification_code) {
        (Some(password), None) if !password.is_empty() && password.len() <= 1024 => {
            let stored: Option<String> = row.try_get("password_hash")?;
            if !stored.is_some_and(|hash| {
                verify_user_password(password, &state.config.admin_token_secret, &hash)
            }) {
                return Err(AppError::Unauthorized);
            }
        }
        (None, Some(code)) => {
            consume_login_verification_code(&mut tx, email, code, CodePurpose::Login, state)
                .await?;
        }
        _ => return Err(AppError::Unauthorized),
    }
    ensure_enabled(&row.try_get::<String, _>("status")?)?;
    let user = UserLogin {
        user_id: row.try_get("id")?,
        auth_version: row.try_get("auth_version")?,
        requires_password_change: row.try_get("requires_password_change")?,
    };
    // Force-change accounts must complete that step before adding credentials.
    if user.requires_password_change {
        return Err(AppError::PasswordChangeRequired);
    }
    let provider: String = pending.try_get("provider")?;
    let subject: String = pending.try_get("subject")?;
    let enabled: bool =
        sqlx::query_scalar("SELECT enabled FROM auth_oauth_provider WHERE provider=$1")
            .bind(&provider)
            .fetch_one(&mut *tx)
            .await?;
    if !enabled {
        return Err(AppError::Unauthorized);
    }
    sqlx::query("INSERT INTO user_oauth_identity(provider,subject,user_id) VALUES($1,$2,$3) ON CONFLICT DO NOTHING")
        .bind(&provider).bind(&subject).bind(user.user_id).execute(&mut *tx).await?;
    let owns: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_oauth_identity WHERE provider=$1 AND subject=$2 AND user_id=$3)")
        .bind(&provider).bind(&subject).bind(user.user_id).fetch_one(&mut *tx).await?;
    if !owns {
        return Err(AppError::ConflictWithCode {
            code: "oauth_identity_already_linked",
            message: "this provider or identity is already connected",
        });
    }
    sqlx::query("DELETE FROM auth_oauth_pending_link WHERE ticket_hash=$1")
        .bind(hash_key(&req.ticket))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(user)
}

// Only a browser flow created from an authenticated account may attach an
// identity. Email equality alone is never an authorization to link accounts.
async fn link_identity(
    state: &AppState,
    provider: Provider,
    subject: &str,
    user_id: DbId,
    auth_version: i64,
) -> AppResult<UserLogin> {
    let mut tx = state.db.pool.begin().await?;
    let row = sqlx::query(r#"SELECT status,auth_version,(password_changed_at IS NULL) AS requires_password_change FROM "user" WHERE id=$1 FOR UPDATE"#)
        .bind(user_id).fetch_optional(&mut *tx).await?.ok_or(AppError::Unauthorized)?;
    ensure_enabled(&row.try_get::<String, _>("status")?)?;
    if row.try_get::<i64, _>("auth_version")? != auth_version
        || row.try_get::<bool, _>("requires_password_change")?
    {
        return Err(AppError::Unauthorized);
    }
    sqlx::query("INSERT INTO user_oauth_identity(provider,subject,user_id) VALUES($1,$2,$3) ON CONFLICT DO NOTHING")
        .bind(provider.name()).bind(subject).bind(user_id).execute(&mut *tx).await?;
    let owns: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_oauth_identity WHERE provider=$1 AND subject=$2 AND user_id=$3)")
        .bind(provider.name()).bind(subject).bind(user_id).fetch_one(&mut *tx).await?;
    if !owns {
        return Err(AppError::ConflictWithCode {
            code: "oauth_identity_already_linked",
            message: "this provider or identity is already linked to an account",
        });
    }
    tx.commit().await?;
    Ok(UserLogin {
        user_id,
        auth_version,
        requires_password_change: false,
    })
}

async fn verified_identity(
    client: &reqwest::Client,
    provider: Provider,
    token: &str,
) -> AppResult<(String, String)> {
    match provider {
        Provider::Github => {
            #[derive(Deserialize)]
            struct User {
                id: u64,
            }
            #[derive(Deserialize)]
            struct Email {
                email: String,
                verified: bool,
                primary: bool,
            }
            let user = client
                .get("https://api.github.com/user")
                .timeout(REQUEST_TIMEOUT)
                .header(header::USER_AGENT, "zhiqiaoapi-oauth")
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json::<User>()
                .await?;
            let emails = client
                .get("https://api.github.com/user/emails")
                .timeout(REQUEST_TIMEOUT)
                .header(header::USER_AGENT, "zhiqiaoapi-oauth")
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json::<Vec<Email>>()
                .await?;
            let email = emails.into_iter().find(|e| e.primary && e.verified).ok_or(
                AppError::BadRequestWithCode {
                    code: "oauth_verified_email_required",
                    message: "provider requires a verified primary email",
                },
            )?;
            Ok((user.id.to_string(), email.email))
        }
        Provider::Google => {
            #[derive(Deserialize)]
            struct User {
                sub: String,
                email: String,
                email_verified: bool,
            }
            let user = client
                .get("https://openidconnect.googleapis.com/v1/userinfo")
                .timeout(REQUEST_TIMEOUT)
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json::<User>()
                .await?;
            if !user.email_verified || user.sub.is_empty() || user.sub.len() > 255 {
                return Err(AppError::BadRequestWithCode {
                    code: "oauth_verified_email_required",
                    message: "provider requires a verified email",
                });
            }
            Ok((user.sub, user.email))
        }
    }
}
async fn identity_user(
    state: &AppState,
    provider: Provider,
    subject: &str,
    email: &str,
    intent: &str,
) -> AppResult<UserLogin> {
    let mut tx = state.db.pool.begin().await?;
    // Serialize registrations by email, then let the unique provider subject
    // constraint protect concurrent callbacks involving changed email addresses.
    lock_user_login(&mut tx, email).await?;
    if let Some(row)=sqlx::query(r#"SELECT u.id,u.auth_version,u.status,(u.password_changed_at IS NULL) AS requires_password_change FROM user_oauth_identity i JOIN "user" u ON u.id=i.user_id WHERE i.provider=$1 AND i.subject=$2 FOR UPDATE OF u"#)
        .bind(provider.name()).bind(subject).fetch_optional(&mut *tx).await? {
        ensure_enabled(&row.try_get::<String,_>("status")?)?;
        let user=UserLogin{user_id:row.try_get("id")?,auth_version:row.try_get("auth_version")?,requires_password_change:row.try_get("requires_password_change")?};
        tx.commit().await?; return Ok(user);
    }
    let exists: bool = sqlx::query_scalar(r#"SELECT EXISTS(SELECT 1 FROM "user" WHERE email=$1)"#)
        .bind(email)
        .fetch_one(&mut *tx)
        .await?;
    if exists {
        return Err(AppError::ConflictWithCode {
            code: "oauth_account_exists",
            message: "an account with this email exists; use password or email sign in",
        });
    }
    if intent != "register" {
        return Err(AppError::BadRequestWithCode {
            code: "account_not_registered",
            message: "register an account before signing in",
        });
    }
    let (mode, enabled) = registration_policy(state).await?;
    if !enabled {
        return Err(AppError::BadRequestWithCode {
            code: "registration_closed",
            message: "registration is closed",
        });
    }
    let user_id:DbId=sqlx::query_scalar(r#"INSERT INTO "user"(email,status,user_group_id,password_changed_at) VALUES($1,$2,(SELECT id FROM user_group WHERE is_default=TRUE),now()) RETURNING id"#)
        .bind(email).bind(if mode==ServiceMode::Internal{"pending"}else{"enabled"}).fetch_one(&mut *tx).await?;
    sqlx::query("INSERT INTO user_oauth_identity(provider,subject,user_id) VALUES($1,$2,$3)")
        .bind(provider.name())
        .bind(subject)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO user_agreement_acceptance(user_id,version,method) VALUES($1,'2026-11-07',$2)",
    )
    .bind(user_id)
    .bind(provider.name())
    .execute(&mut *tx)
    .await?;
    project::ensure_default_project_for_user(&mut tx, user_id).await?;
    tx.commit().await?;
    ensure_enabled(if mode == ServiceMode::Internal {
        "pending"
    } else {
        "enabled"
    })?;
    Ok(UserLogin {
        user_id,
        auth_version: 0,
        requires_password_change: false,
    })
}

#[derive(Deserialize)]
struct ExchangeRequest {
    ticket: String,
}
async fn exchange(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ExchangeRequest>,
) -> AppResult<Response> {
    let browser = cookie(&headers).ok_or(AppError::Unauthorized)?;
    if req.ticket.len() != 64 {
        return Err(AppError::Unauthorized);
    }
    let row=sqlx::query(r#"DELETE FROM auth_oauth_ticket t USING "user" u WHERE t.ticket_hash=$1 AND t.browser_hash=$2 AND t.expires_at>now() AND t.user_id=u.id AND u.status='enabled' AND u.auth_version=t.auth_version RETURNING u.id,u.auth_version,(u.password_changed_at IS NULL) AS requires_password_change"#)
        .bind(hash_key(&req.ticket)).bind(hash_key(browser)).fetch_optional(&state.db.pool).await?.ok_or(AppError::Unauthorized)?;
    let user = UserLogin {
        user_id: row.try_get("id")?,
        auth_version: row.try_get("auth_version")?,
        requires_password_change: row.try_get("requires_password_change")?,
    };
    Ok(with_cookie(Json(login_response(&state, user)), "", 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn pending_identity(
        state: &AppState,
        email: &str,
        browser: &str,
        subject: &str,
    ) -> ConfirmLinkRequest {
        let ticket = random_token();
        sqlx::query("UPDATE auth_oauth_provider SET enabled=true WHERE provider='github'")
            .execute(&state.db.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO auth_oauth_pending_link(ticket_hash,browser_hash,provider,subject,email,expires_at) VALUES($1,$2,'github',$3,$4,now()+interval '10 minutes')")
            .bind(hash_key(&ticket)).bind(hash_key(browser)).bind(subject).bind(email).execute(&state.db.pool).await.unwrap();
        ConfirmLinkRequest {
            ticket,
            email: email.into(),
            password: Some("password123".into()),
            verification_code: None,
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL"]
    async fn pending_link_requires_browser_and_local_proof_and_is_single_use(pool: sqlx::PgPool) {
        let state = state_with_pool(pool.clone());
        let user_id: DbId = sqlx::query_scalar(r#"INSERT INTO "user"(email,user_group_id,password_hash,password_changed_at) SELECT 'confirm@example.com',id,$1,now() FROM user_group WHERE is_default RETURNING id"#)
            .bind(super::super::hash_user_password("password123", &state.config.admin_token_secret)).fetch_one(&pool).await.unwrap();
        let browser = random_token();
        let mut req =
            pending_identity(&state, "confirm@example.com", &browser, "subject-confirm").await;
        assert!(
            confirm_pending_identity(&state, &random_token(), &req.email, &req)
                .await
                .is_err()
        );
        assert!(
            confirm_pending_identity(&state, &browser, "other@example.com", &req)
                .await
                .is_err()
        );
        req.password = Some("wrong-password".into());
        assert!(confirm_pending_identity(&state, &browser, &req.email, &req)
            .await
            .is_err());
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM user_oauth_identity")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        req.password = Some("password123".into());
        assert_eq!(
            confirm_pending_identity(&state, &browser, &req.email, &req)
                .await
                .unwrap()
                .user_id,
            user_id
        );
        assert!(confirm_pending_identity(&state, &browser, &req.email, &req)
            .await
            .is_err());
        let second =
            pending_identity(&state, "confirm@example.com", &browser, "different-subject").await;
        assert!(matches!(
            confirm_pending_identity(&state, &browser, &second.email, &second).await,
            Err(AppError::ConflictWithCode {
                code: "oauth_identity_already_linked",
                ..
            })
        ));
        sqlx::query("DELETE FROM user_oauth_identity")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE auth_oauth_pending_link SET expires_at=now()-interval '1 minute'")
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            confirm_pending_identity(&state, &browser, &second.email, &second)
                .await
                .is_err()
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL"]
    async fn pending_link_accepts_only_login_code_and_enabled_account(pool: sqlx::PgPool) {
        let state = state_with_pool(pool.clone());
        sqlx::query(r#"INSERT INTO "user"(email,user_group_id,password_changed_at) SELECT 'code-confirm@example.com',id,now() FROM user_group WHERE is_default"#)
            .execute(&pool).await.unwrap();
        let browser = random_token();
        let mut req =
            pending_identity(&state, "code-confirm@example.com", &browser, "subject-code").await;
        req.password = None;
        req.verification_code = Some("123456".into());
        for purpose in [CodePurpose::Register, CodePurpose::Login] {
            let hash = super::super::hash_email_verification_code(
                &purpose.hash_context(&req.email),
                "123456",
                &state.config.admin_token_secret,
            );
            sqlx::query("INSERT INTO user_code(email,code_hash,expires_at) VALUES($1,$2,now()+interval '5 minutes')")
                .bind(&req.email).bind(hash).execute(&pool).await.unwrap();
            if matches!(purpose, CodePurpose::Register) {
                assert!(confirm_pending_identity(&state, &browser, &req.email, &req)
                    .await
                    .is_err());
            }
        }
        sqlx::query(r#"UPDATE "user" SET status='disabled' WHERE email=$1"#)
            .bind(&req.email)
            .execute(&pool)
            .await
            .unwrap();
        assert!(confirm_pending_identity(&state, &browser, &req.email, &req)
            .await
            .is_err());
        sqlx::query(r#"UPDATE "user" SET status='enabled' WHERE email=$1"#)
            .bind(&req.email)
            .execute(&pool)
            .await
            .unwrap();
        assert!(confirm_pending_identity(&state, &browser, &req.email, &req)
            .await
            .is_ok());
        let consumed: bool = sqlx::query_scalar(
            "SELECT consumed_at IS NOT NULL FROM user_code WHERE email=$1 ORDER BY id DESC LIMIT 1",
        )
        .bind(&req.email)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(consumed);
    }

    fn state_with_pool(pool: sqlx::PgPool) -> Arc<AppState> {
        let mut state = crate::app::tests::test_state();
        let inner = Arc::get_mut(&mut state).unwrap();
        inner.db.pool = pool;
        inner.config.public_base_url = Some("https://example.com".into());
        state
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL"]
    async fn identity_login_requires_registered_identity_and_never_links_email_implicitly(
        pool: sqlx::PgPool,
    ) {
        let state = state_with_pool(pool.clone());
        assert!(matches!(
            identity_user(
                &state,
                Provider::Github,
                "123",
                "missing@example.com",
                "login"
            )
            .await,
            Err(AppError::BadRequestWithCode {
                code: "account_not_registered",
                ..
            })
        ));
        let id: DbId = sqlx::query_scalar("INSERT INTO \"user\"(email,user_group_id,password_changed_at) SELECT 'existing@example.com',id,now() FROM user_group WHERE is_default RETURNING id")
            .fetch_one(&pool).await.unwrap();
        assert!(matches!(
            identity_user(
                &state,
                Provider::Github,
                "123",
                "existing@example.com",
                "register"
            )
            .await,
            Err(AppError::ConflictWithCode {
                code: "oauth_account_exists",
                ..
            })
        ));
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM user_oauth_identity")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        sqlx::query(
            "INSERT INTO user_oauth_identity(provider,subject,user_id) VALUES('github','123',$1)",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(
            identity_user(
                &state,
                Provider::Github,
                "123",
                "changed@example.com",
                "login"
            )
            .await
            .unwrap()
            .user_id,
            id
        );
        sqlx::query("UPDATE \"user\" SET status='disabled' WHERE id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(identity_user(
            &state,
            Provider::Github,
            "123",
            "changed@example.com",
            "login"
        )
        .await
        .is_err());
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL"]
    async fn explicit_link_is_idempotent_and_rejects_takeover_or_revoked_sessions(
        pool: sqlx::PgPool,
    ) {
        let state = state_with_pool(pool.clone());
        let ids: Vec<DbId> = sqlx::query_scalar("INSERT INTO \"user\"(email,user_group_id,password_changed_at) SELECT email,id,now() FROM user_group CROSS JOIN (VALUES('link-a@example.com'),('link-b@example.com')) AS e(email) WHERE is_default RETURNING \"user\".id")
            .fetch_all(&pool).await.unwrap();
        let a = ids[0];
        let b = ids[1];
        assert_eq!(
            link_identity(&state, Provider::Google, "subject-a", a, 0)
                .await
                .unwrap()
                .user_id,
            a
        );
        assert!(link_identity(&state, Provider::Google, "subject-a", a, 0)
            .await
            .is_ok());
        assert!(matches!(
            link_identity(&state, Provider::Google, "subject-a", b, 0).await,
            Err(AppError::ConflictWithCode {
                code: "oauth_identity_already_linked",
                ..
            })
        ));
        assert!(matches!(
            link_identity(&state, Provider::Google, "subject-b", a, 0).await,
            Err(AppError::ConflictWithCode {
                code: "oauth_identity_already_linked",
                ..
            })
        ));
        let (first, second) = tokio::join!(
            link_identity(&state, Provider::Github, "concurrent", a, 0),
            link_identity(&state, Provider::Github, "concurrent", b, 0)
        );
        assert_ne!(first.is_ok(), second.is_ok());
        sqlx::query("UPDATE \"user\" SET auth_version=1 WHERE id=$1")
            .bind(b)
            .execute(&pool)
            .await
            .unwrap();
        assert!(matches!(
            link_identity(&state, Provider::Google, "subject-b", b, 0).await,
            Err(AppError::Unauthorized)
        ));
        sqlx::query("UPDATE \"user\" SET status='disabled' WHERE id=$1")
            .bind(a)
            .execute(&pool)
            .await
            .unwrap();
        assert!(link_identity(&state, Provider::Google, "subject-a", a, 0)
            .await
            .is_err());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM user_oauth_identity")
                .fetch_one(&pool)
                .await
                .unwrap(),
            2
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL"]
    async fn exchange_is_browser_bound_single_use_and_revoked_by_account_changes(
        pool: sqlx::PgPool,
    ) {
        let state = state_with_pool(pool.clone());
        let id: DbId = sqlx::query_scalar("INSERT INTO \"user\"(email,user_group_id,password_changed_at) SELECT 'exchange@example.com',id,now() FROM user_group WHERE is_default RETURNING id")
            .fetch_one(&pool).await.unwrap();
        let browser = random_token();
        let ticket = random_token();
        sqlx::query("INSERT INTO auth_oauth_ticket(ticket_hash,browser_hash,user_id,auth_version,expires_at) VALUES($1,$2,$3,0,now()+interval '1 minute')")
            .bind(hash_key(&ticket)).bind(hash_key(&browser)).bind(id).execute(&pool).await.unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{COOKIE_NAME}={}", random_token())).unwrap(),
        );
        assert!(exchange(
            State(state.clone()),
            headers,
            Json(ExchangeRequest {
                ticket: ticket.clone()
            })
        )
        .await
        .is_err());
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{COOKIE_NAME}={browser}")).unwrap(),
        );
        assert!(exchange(
            State(state.clone()),
            headers.clone(),
            Json(ExchangeRequest {
                ticket: ticket.clone()
            })
        )
        .await
        .is_ok());
        assert!(exchange(
            State(state.clone()),
            headers.clone(),
            Json(ExchangeRequest {
                ticket: ticket.clone()
            })
        )
        .await
        .is_err());
        sqlx::query("INSERT INTO auth_oauth_ticket(ticket_hash,browser_hash,user_id,auth_version,expires_at) VALUES($1,$2,$3,0,now()+interval '1 minute')")
            .bind(hash_key(&ticket)).bind(hash_key(&browser)).bind(id).execute(&pool).await.unwrap();
        sqlx::query("UPDATE \"user\" SET auth_version=1 WHERE id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(exchange(
            State(state.clone()),
            headers,
            Json(ExchangeRequest { ticket })
        )
        .await
        .is_err());
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL"]
    async fn viewer_cannot_change_oauth_provider_settings(pool: sqlx::PgPool) {
        let state = state_with_pool(pool.clone());
        let id:DbId=sqlx::query_scalar("INSERT INTO admin(username,password_hash,role) VALUES('viewer-test','dummy','viewer') RETURNING id")
            .fetch_one(&pool).await.unwrap();
        let input = || ProviderInput {
            enabled: false,
            client_id: "must-not-be-saved".into(),
            client_secret: None,
            clear_client_secret: false,
        };
        let result = save_settings(
            AdminAuth { admin_id: id },
            State(state),
            Json(SettingsInput {
                github: input(),
                google: input(),
            }),
        )
        .await;
        assert!(matches!(result, Err(AppError::Forbidden(_))));
        let id: String =
            sqlx::query_scalar("SELECT client_id FROM auth_oauth_provider WHERE provider='github'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(id.is_empty());
    }
    #[test]
    fn pkce_uses_rfc7636_s256() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
    #[test]
    fn cookie_does_not_accept_unbounded_or_injected_values() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("other=x; {COOKIE_NAME}={}", "a".repeat(64))).unwrap(),
        );
        assert_eq!(cookie(&headers), Some("a".repeat(64).as_str()));
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("zhiqiao_oauth_flow=short"),
        );
        assert!(cookie(&headers).is_none());
    }
    #[test]
    fn browser_and_flow_tokens_are_distinct_high_entropy_values() {
        let a = random_token();
        let b = random_token();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }
}
