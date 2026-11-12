use anyhow::{Context, Result};
use futures_util::{stream::FuturesUnordered, StreamExt};
use lettre::{
    message::{header::ContentType, Attachment, Mailbox, MultiPart, SinglePart},
    transport::smtp::{
        authentication::{Credentials, Mechanism},
        client::{AsyncSmtpConnection, TlsParameters},
        extension::ClientId,
    },
    Message,
};
use serde::Deserialize;
use sqlx::Row;

use crate::{db::Db, id::DbId, input::trimmed_non_empty, secrets::SecretStore};

pub const SMTP_SETTING_KEY: &str = "smtp";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmailLocale {
    ZhCn,
    EnUs,
}

impl EmailLocale {
    pub fn from_public_locale(locale: Option<&str>) -> Self {
        let Some(locale) = trimmed_non_empty(locale) else {
            return Self::ZhCn;
        };
        let locale = locale.replace('_', "-").to_ascii_lowercase();
        if locale == "en" || locale.starts_with("en-") {
            Self::EnUs
        } else {
            Self::ZhCn
        }
    }

    fn product_name(self) -> &'static str {
        match self {
            Self::ZhCn => "智桥",
            Self::EnUs => "Zhiqiao",
        }
    }
}

#[derive(Clone)]
pub struct EmailService {
    source: EmailConfigSource,
}

#[derive(Clone)]
enum EmailConfigSource {
    Database { db: Db, secrets: Box<SecretStore> },
    Static(EmailConfig),
}

#[derive(Clone, Debug)]
pub struct EmailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_tls: bool,
    pub from_email: String,
    pub from_name: Option<String>,
    pub subject_prefix: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StoredSmtpSetting {
    smtp_host: String,
    smtp_port: u16,
    smtp_username: Option<String>,
    password_ciphertext: Option<String>,
    smtp_tls: bool,
    from_email: String,
    from_name: Option<String>,
    subject_prefix: Option<String>,
}

impl EmailService {
    pub fn new(db: Db, secrets: SecretStore) -> Self {
        Self {
            source: EmailConfigSource::Database {
                db,
                secrets: Box::new(secrets),
            },
        }
    }

    pub fn test() -> Self {
        let config = EmailConfig {
            smtp_host: "localhost".to_string(),
            smtp_port: 2525,
            smtp_username: None,
            smtp_password: None,
            smtp_tls: false,
            from_email: "noreply@example.com".to_string(),
            from_name: None,
            subject_prefix: None,
        };
        Self {
            source: EmailConfigSource::Static(config),
        }
    }

    pub async fn send_api_key(
        &self,
        to_email: &str,
        api_key: &str,
        locale: EmailLocale,
    ) -> Result<()> {
        let config = self.config().await?;
        let message = api_key_message(&config, to_email, api_key, locale)?;
        send_message(&config, message)
            .await
            .context("failed to send API key email")?;

        Ok(())
    }

    pub async fn send_password_reset(
        &self,
        to_email: &str,
        reset_url: &str,
        locale: EmailLocale,
    ) -> Result<()> {
        let config = self.config().await?;
        let message = password_reset_message(&config, to_email, reset_url, locale)?;
        send_message(&config, message)
            .await
            .context("failed to send password reset email")?;

        Ok(())
    }

    pub async fn send_login_verification_code(
        &self,
        to_email: &str,
        code: &str,
        locale: EmailLocale,
    ) -> Result<()> {
        let config = self.config().await?;
        let message = login_verification_code_message(&config, to_email, code, locale)?;
        send_message(&config, message)
            .await
            .context("failed to send login verification code email")?;

        Ok(())
    }

    pub async fn send_test(config: &EmailConfig, to_email: &str) -> Result<()> {
        let locale = EmailLocale::ZhCn;
        let message = smtp_test_message(config, to_email, locale)?;
        send_message(config, message)
            .await
            .context("failed to send SMTP test email")?;

        Ok(())
    }

    async fn config(&self) -> Result<EmailConfig> {
        match &self.source {
            EmailConfigSource::Static(config) => Ok(config.clone()),
            EmailConfigSource::Database { db, secrets } => load_smtp_config(db, secrets).await,
        }
    }

    #[cfg(test)]
    fn api_key_message(
        &self,
        to_email: &str,
        api_key: &str,
        locale: EmailLocale,
    ) -> Result<Message> {
        let config = match &self.source {
            EmailConfigSource::Static(config) => config,
            EmailConfigSource::Database { .. } => {
                anyhow::bail!("API key email requires resolved SMTP settings")
            }
        };
        api_key_message(config, to_email, api_key, locale)
    }
}

async fn connect_smtp(config: &EmailConfig, port: u16) -> Result<AsyncSmtpConnection> {
    use std::time::Duration;
    let addresses = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::lookup_host((config.smtp_host.as_str(), port)),
    )
    .await
    .context("SMTP DNS lookup timed out")??;
    let mut addresses: Vec<_> = addresses.collect();
    addresses.sort_unstable();
    addresses.dedup();
    let mut attempts = FuturesUnordered::new();
    for address in addresses.into_iter().take(8) {
        attempts.push(async move {
            tokio::time::timeout(Duration::from_secs(8), async {
                let hello = ClientId::default();
                // The socket address selects a healthy endpoint; TLS always
                // validates the configured hostname, never the resolved IP.
                let tls = if config.smtp_tls {
                    Some(TlsParameters::new(config.smtp_host.clone())?)
                } else {
                    None
                };
                let mut connection = AsyncSmtpConnection::connect_tokio1(
                    address,
                    Some(Duration::from_secs(6)),
                    &hello,
                    if port == 465 { tls.clone() } else { None },
                    None,
                )
                .await?;
                if let Some(tls) = tls.filter(|_| port != 465) {
                    connection.starttls(tls, &hello).await?;
                }
                Ok::<_, anyhow::Error>(connection)
            })
            .await
            .context("SMTP connection timed out")?
        });
    }
    let mut last_error = anyhow::anyhow!("SMTP host has no usable addresses");
    while let Some(result) = attempts.next().await {
        match result {
            Ok(connection) => return Ok(connection),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

async fn send_message(config: &EmailConfig, message: Message) -> Result<()> {
    use std::time::Duration;
    let mut connection = match connect_smtp(config, config.smtp_port).await {
        Ok(connection) => connection,
        // QQ exposes both authenticated TLS submission ports. A broken 465
        // endpoint can fall back before AUTH or DATA, never after submission.
        Err(_)
            if config.smtp_tls
                && config.smtp_port == 465
                && config.smtp_host.eq_ignore_ascii_case("smtp.qq.com") =>
        {
            connect_smtp(config, 587).await?
        }
        Err(error) => return Err(error),
    };
    if let (Some(username), Some(password)) = (&config.smtp_username, &config.smtp_password) {
        tokio::time::timeout(
            Duration::from_secs(8),
            connection.auth(
                &[Mechanism::Plain, Mechanism::Login],
                &Credentials::new(username.clone(), password.clone()),
            ),
        )
        .await
        .context("SMTP authentication timed out")??;
    }
    // Do not retry once DATA can have reached the server: an ambiguous result
    // must not produce duplicate password resets, verification codes or keys.
    tokio::time::timeout(
        Duration::from_secs(20),
        connection.send(message.envelope(), &message.formatted()),
    )
    .await
    .context("SMTP submission timed out")??;
    let _ = tokio::time::timeout(Duration::from_secs(1), connection.abort()).await;
    Ok(())
}

async fn load_smtp_config(db: &Db, secrets: &SecretStore) -> Result<EmailConfig> {
    let row = sqlx::query("SELECT id, value FROM setting WHERE key = $1")
        .bind(SMTP_SETTING_KEY)
        .fetch_optional(&db.pool)
        .await?
        .context("SMTP settings are not configured")?;

    let id: DbId = row.try_get("id")?;
    let value: serde_json::Value = row.try_get("value")?;
    let setting: StoredSmtpSetting =
        serde_json::from_value(value).context("SMTP settings are invalid")?;
    let smtp_password = setting
        .password_ciphertext
        .as_deref()
        .map(|ciphertext| secrets.plaintext(id, ciphertext))
        .transpose()?;

    let config = EmailConfig {
        smtp_host: setting.smtp_host,
        smtp_port: setting.smtp_port,
        smtp_username: setting.smtp_username,
        smtp_password,
        smtp_tls: setting.smtp_tls,
        from_email: setting.from_email,
        from_name: setting.from_name,
        subject_prefix: setting.subject_prefix,
    };
    validate_email_config(&config)?;
    Ok(config)
}

fn validate_email_config(config: &EmailConfig) -> Result<()> {
    if config.smtp_host.trim().is_empty() {
        anyhow::bail!("SMTP host is not configured");
    }
    if config.smtp_port == 0 {
        anyhow::bail!("SMTP port is invalid");
    }
    if config.from_email.trim().is_empty() {
        anyhow::bail!("SMTP sender email is not configured");
    }
    Ok(())
}

pub fn smtp_config_error(err: &anyhow::Error) -> Option<(&'static str, &'static str)> {
    err.chain()
        .map(ToString::to_string)
        .find_map(|message| match message.as_str() {
            "SMTP settings are not configured" => Some((
                "smtp_settings_not_configured",
                "SMTP settings are not configured",
            )),
            "SMTP settings are invalid" => {
                Some(("smtp_settings_invalid", "SMTP settings are invalid"))
            }
            "SMTP host is not configured" => {
                Some(("smtp_host_not_configured", "SMTP host is not configured"))
            }
            "SMTP port is invalid" => Some(("smtp_port_invalid", "SMTP port is invalid")),
            "SMTP sender email is not configured" => Some((
                "smtp_sender_email_not_configured",
                "SMTP sender email is not configured",
            )),
            "SMTP sender email is invalid" => {
                Some(("smtp_sender_email_invalid", "SMTP sender email is invalid"))
            }
            _ => None,
        })
}

pub fn smtp_test_error(err: &anyhow::Error) -> (&'static str, &'static str) {
    let message = err
        .chain()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();

    if message.contains("err.login.passerr")
        || message.contains("authentication failed")
        || message.contains("auth failed")
        || message.contains("invalid credentials")
        || message.contains("login failed")
        || contains_smtp_status_code(&message, "535")
    {
        return ("smtp_authentication_failed", "SMTP authentication failed");
    }

    if message.contains("timed out") || message.contains("timeout") {
        return ("smtp_connection_timed_out", "SMTP connection timed out");
    }

    ("smtp_test_email_failed", "SMTP test email failed")
}

fn contains_smtp_status_code(message: &str, code: &str) -> bool {
    message.match_indices(code).any(|(index, _)| {
        let before = index
            .checked_sub(1)
            .and_then(|position| message.as_bytes().get(position))
            .copied();
        let after = message.as_bytes().get(index + code.len()).copied();

        !before.is_some_and(|byte| byte.is_ascii_digit())
            && !after.is_some_and(|byte| byte.is_ascii_digit())
    })
}

fn sender_mailbox(config: &EmailConfig, locale: EmailLocale) -> Result<Mailbox> {
    Ok(Mailbox::new(
        Some(
            config
                .from_name
                .as_deref()
                .and_then(|name| trimmed_non_empty(Some(name)))
                .unwrap_or_else(|| locale.product_name())
                .to_string(),
        ),
        config
            .from_email
            .parse()
            .context("SMTP sender email is invalid")?,
    ))
}

fn branded_body(text: String, html: String) -> MultiPart {
    let logo = r#"<div style="margin:0 0 22px"><img src="cid:zhiqiao-brand" width="40" height="40" alt="智桥" style="vertical-align:middle;border-radius:10px"><span style="vertical-align:middle;margin-left:10px;font-size:18px;font-weight:700;color:#18243b">智桥 API</span></div>"#;
    let html = html.replacen("<h1", &format!("{logo}<h1"), 1);
    MultiPart::alternative()
        .singlepart(SinglePart::plain(text))
        .multipart(
            MultiPart::related()
                .singlepart(SinglePart::html(html))
                .singlepart(Attachment::new_inline("zhiqiao-brand".into()).body(
                    include_bytes!("../../assets/brand.png").to_vec(),
                    ContentType::parse("image/png").expect("valid PNG MIME type"),
                )),
        )
}

fn api_key_message(
    config: &EmailConfig,
    to_email: &str,
    api_key: &str,
    locale: EmailLocale,
) -> Result<Message> {
    let content = api_key_email_content(locale);
    let subject_prefix = config
        .subject_prefix
        .as_deref()
        .unwrap_or_else(|| locale.product_name());
    let subject = api_key_email_subject(subject_prefix, content);
    Message::builder()
        .from(sender_mailbox(config, locale)?)
        .to(to_email
            .parse::<Mailbox>()
            .context("recipient email address is invalid")?)
        .subject(subject)
        .multipart(branded_body(
            api_key_email_text_body(api_key, content),
            api_key_email_html_body(api_key, content),
        ))
        .context("failed to build API key email")
}

fn password_reset_message(
    config: &EmailConfig,
    to_email: &str,
    reset_url: &str,
    locale: EmailLocale,
) -> Result<Message> {
    let content = password_reset_email_content(locale);
    let subject_prefix = config
        .subject_prefix
        .as_deref()
        .unwrap_or_else(|| locale.product_name());
    Message::builder()
        .from(sender_mailbox(config, locale)?)
        .to(to_email
            .parse::<Mailbox>()
            .context("recipient email address is invalid")?)
        .subject(format!("{subject_prefix} {}", content.subject))
        .multipart(branded_body(
            password_reset_email_text_body(reset_url, content),
            password_reset_email_html_body(reset_url, content),
        ))
        .context("failed to build password reset email")
}

fn login_verification_code_message(
    config: &EmailConfig,
    to_email: &str,
    code: &str,
    locale: EmailLocale,
) -> Result<Message> {
    let content = login_verification_code_email_content(locale);
    let subject_prefix = config
        .subject_prefix
        .as_deref()
        .unwrap_or_else(|| locale.product_name());
    Message::builder()
        .from(sender_mailbox(config, locale)?)
        .to(to_email
            .parse::<Mailbox>()
            .context("recipient email address is invalid")?)
        .subject(format!("{subject_prefix} {}", content.subject))
        .multipart(branded_body(
            login_verification_code_email_text_body(code, content),
            login_verification_code_email_html_body(code, content),
        ))
        .context("failed to build login verification code email")
}

fn smtp_test_message(config: &EmailConfig, to_email: &str, locale: EmailLocale) -> Result<Message> {
    let subject_prefix = config
        .subject_prefix
        .as_deref()
        .unwrap_or_else(|| locale.product_name());
    let subject = format!("{subject_prefix} SMTP 测试邮件");
    Message::builder()
        .from(sender_mailbox(config, locale)?)
        .to(to_email
            .parse::<Mailbox>()
            .context("recipient email address is invalid")?)
        .subject(subject)
        .multipart(branded_body(
            smtp_test_email_text_body(),
            smtp_test_email_html_body(),
        ))
        .context("failed to build SMTP test email")
}

#[derive(Clone, Copy)]
struct ApiKeyEmailContent {
    subject: &'static str,
    title: &'static str,
    intro: &'static str,
    api_key_label: &'static str,
    footer: &'static str,
}

fn api_key_email_content(locale: EmailLocale) -> ApiKeyEmailContent {
    match locale {
        EmailLocale::ZhCn => ApiKeyEmailContent {
            subject: "API 密钥",
            title: "你的 智桥 API 密钥已生成",
            intro: "调用智桥 API 时请使用下面的密钥。请妥善保管，完整密钥只会通过这封邮件发送。",
            api_key_label: "API 密钥",
            footer: "如果这不是你本人操作，请在 智桥管理后台轮换该密钥。",
        },
        EmailLocale::EnUs => ApiKeyEmailContent {
            subject: "API Key",
            title: "Your zhiqiaoapi API key is ready",
            intro: "Use this key when calling zhiqiaoapi. Keep it private because the full key is only sent in this email.",
            api_key_label: "API key",
            footer: "If you did not request this key, rotate it from the zhiqiaoapi admin console.",
        },
    }
}

fn api_key_email_subject(prefix: &str, content: ApiKeyEmailContent) -> String {
    format!("{prefix} {}", content.subject)
}

#[derive(Clone, Copy)]
struct PasswordResetEmailContent {
    subject: &'static str,
    title: &'static str,
    intro: &'static str,
    action: &'static str,
    footer: &'static str,
}

#[derive(Clone, Copy)]
struct LoginVerificationCodeEmailContent {
    subject: &'static str,
    title: &'static str,
    intro: &'static str,
    code_label: &'static str,
    footer: &'static str,
}

fn password_reset_email_content(locale: EmailLocale) -> PasswordResetEmailContent {
    match locale {
        EmailLocale::ZhCn => PasswordResetEmailContent {
            subject: "重置密码",
            title: "重置你的 智桥密码",
            intro: "点击下面的按钮设置新的登录密码。链接会在 30 分钟后失效。",
            action: "重置密码",
            footer: "如果这不是你本人操作，可以忽略这封邮件。",
        },
        EmailLocale::EnUs => PasswordResetEmailContent {
            subject: "Reset password",
            title: "Reset your zhiqiaoapi password",
            intro: "Click the button below to set a new sign-in password. This link expires in 30 minutes.",
            action: "Reset password",
            footer: "If you did not request this, you can ignore this email.",
        },
    }
}

fn login_verification_code_email_content(locale: EmailLocale) -> LoginVerificationCodeEmailContent {
    match locale {
        EmailLocale::ZhCn => LoginVerificationCodeEmailContent {
            subject: "登录验证码",
            title: "验证你的 智桥邮箱",
            intro: "请输入下面的验证码完成邮箱验证。验证码会在 10 分钟后失效。",
            code_label: "验证码",
            footer: "如果这不是你本人操作，可以忽略这封邮件。",
        },
        EmailLocale::EnUs => LoginVerificationCodeEmailContent {
            subject: "Sign-in verification code",
            title: "Verify your zhiqiaoapi email",
            intro: "Enter this code to verify your email. It expires in 10 minutes.",
            code_label: "Verification code",
            footer: "If you did not request this, you can ignore this email.",
        },
    }
}

fn api_key_email_text_body(api_key: &str, content: ApiKeyEmailContent) -> String {
    format!(
        "{}\n\n{}:\n{api_key}\n\n{}",
        content.title, content.api_key_label, content.intro
    )
}

fn api_key_email_html_body(api_key: &str, content: ApiKeyEmailContent) -> String {
    let escaped_api_key = escape_html(api_key);
    let escaped_title = escape_html(content.title);
    let escaped_intro = escape_html(content.intro);
    let escaped_footer = escape_html(content.footer);
    format!(
        r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{escaped_title}</title>
  </head>
  <body style="margin:0;background:#f5f7fb;color:#172033;font-family:Arial,Helvetica,sans-serif;">
    <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="background:#f5f7fb;padding:32px 16px;">
      <tr>
        <td align="center">
          <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="max-width:560px;background:#ffffff;border:1px solid #e1e6ef;border-radius:8px;overflow:hidden;">
            <tr>
              <td style="padding:28px 32px 16px;">
                <h1 style="margin:0;color:#111827;font-size:24px;line-height:32px;font-weight:700;">{escaped_title}</h1>
                <p style="margin:12px 0 0;color:#4b5563;font-size:15px;line-height:24px;">{escaped_intro}</p>
              </td>
            </tr>
            <tr>
              <td style="padding:8px 32px 24px;">
                <div style="margin:0;padding:16px;background:#111827;border-radius:8px;color:#f9fafb;font-size:14px;line-height:22px;font-family:Consolas,Menlo,Monaco,monospace;word-break:break-all;">{escaped_api_key}</div>
              </td>
            </tr>
            <tr>
              <td style="padding:0 32px 28px;">
                <p style="margin:0;color:#6b7280;font-size:13px;line-height:20px;">{escaped_footer}</p>
              </td>
            </tr>
          </table>
        </td>
      </tr>
    </table>
  </body>
</html>"#
    )
}

fn login_verification_code_email_text_body(
    code: &str,
    content: LoginVerificationCodeEmailContent,
) -> String {
    format!(
        "{}\n\n{}:\n{code}\n\n{}\n\n{}",
        content.title, content.code_label, content.intro, content.footer
    )
}

fn login_verification_code_email_html_body(
    code: &str,
    content: LoginVerificationCodeEmailContent,
) -> String {
    let escaped_code = escape_html(code);
    let escaped_title = escape_html(content.title);
    let escaped_intro = escape_html(content.intro);
    let escaped_label = escape_html(content.code_label);
    let escaped_footer = escape_html(content.footer);
    format!(
        r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{escaped_title}</title>
  </head>
  <body style="margin:0;background:#f5f7fb;color:#172033;font-family:Arial,Helvetica,sans-serif;">
    <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="background:#f5f7fb;padding:32px 16px;">
      <tr>
        <td align="center">
          <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="max-width:560px;background:#ffffff;border:1px solid #e1e6ef;border-radius:8px;overflow:hidden;">
            <tr>
              <td style="padding:28px 32px 16px;">
                <h1 style="margin:0;color:#111827;font-size:24px;line-height:32px;font-weight:700;">{escaped_title}</h1>
                <p style="margin:12px 0 0;color:#4b5563;font-size:15px;line-height:24px;">{escaped_intro}</p>
              </td>
            </tr>
            <tr>
              <td style="padding:8px 32px 24px;">
                <p style="margin:0 0 8px;color:#6b7280;font-size:13px;line-height:20px;">{escaped_label}</p>
                <div style="display:inline-block;letter-spacing:8px;margin:0;padding:14px 18px;background:#111827;border-radius:8px;color:#f9fafb;font-size:28px;line-height:34px;font-family:Consolas,Menlo,Monaco,monospace;">{escaped_code}</div>
              </td>
            </tr>
            <tr>
              <td style="padding:0 32px 28px;">
                <p style="margin:0;color:#6b7280;font-size:13px;line-height:20px;">{escaped_footer}</p>
              </td>
            </tr>
          </table>
        </td>
      </tr>
    </table>
  </body>
</html>"#
    )
}

fn password_reset_email_text_body(reset_url: &str, content: PasswordResetEmailContent) -> String {
    format!(
        "{}\n\n{}\n{reset_url}\n\n{}",
        content.title, content.intro, content.footer
    )
}

fn password_reset_email_html_body(reset_url: &str, content: PasswordResetEmailContent) -> String {
    let escaped_title = escape_html(content.title);
    let escaped_intro = escape_html(content.intro);
    let escaped_action = escape_html(content.action);
    let escaped_footer = escape_html(content.footer);
    let escaped_url = escape_html(reset_url);
    format!(
        r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{escaped_title}</title>
  </head>
  <body style="margin:0;background:#f5f7fb;color:#172033;font-family:Arial,Helvetica,sans-serif;">
    <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="background:#f5f7fb;padding:32px 16px;">
      <tr>
        <td align="center">
          <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="max-width:560px;background:#ffffff;border:1px solid #e1e6ef;border-radius:8px;overflow:hidden;">
            <tr>
              <td style="padding:28px 32px 16px;">
                <h1 style="margin:0;color:#111827;font-size:24px;line-height:32px;font-weight:700;">{escaped_title}</h1>
                <p style="margin:12px 0 0;color:#4b5563;font-size:15px;line-height:24px;">{escaped_intro}</p>
              </td>
            </tr>
            <tr>
              <td style="padding:8px 32px 24px;">
                <a href="{escaped_url}" style="display:inline-block;background:#3f8cff;color:#ffffff;text-decoration:none;border-radius:8px;padding:12px 18px;font-size:15px;line-height:22px;">{escaped_action}</a>
              </td>
            </tr>
            <tr>
              <td style="padding:0 32px 28px;">
                <p style="margin:0 0 12px;color:#6b7280;font-size:13px;line-height:20px;word-break:break-all;">{escaped_url}</p>
                <p style="margin:0;color:#6b7280;font-size:13px;line-height:20px;">{escaped_footer}</p>
              </td>
            </tr>
          </table>
        </td>
      </tr>
    </table>
  </body>
</html>"#
    )
}

fn smtp_test_email_text_body() -> String {
    "智桥 API SMTP 测试邮件\n\n如果你收到这封邮件，说明 SMTP 配置可以正常发送邮件。".to_string()
}

fn smtp_test_email_html_body() -> String {
    r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>智桥 API SMTP 测试邮件</title>
  </head>
  <body style="margin:0;background:#f5f7fb;color:#172033;font-family:Arial,Helvetica,sans-serif;">
    <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="background:#f5f7fb;padding:32px 16px;">
      <tr>
        <td align="center">
          <table role="presentation" width="100%" cellspacing="0" cellpadding="0" style="max-width:560px;background:#ffffff;border:1px solid #e1e6ef;border-radius:8px;overflow:hidden;">
            <tr>
              <td style="padding:28px 32px;">
                <h1 style="margin:0;color:#111827;font-size:24px;line-height:32px;font-weight:700;">智桥 API SMTP 测试邮件</h1>
                <p style="margin:12px 0 0;color:#4b5563;font-size:15px;line-height:24px;">如果你收到这封邮件，说明 SMTP 配置可以正常发送邮件。</p>
              </td>
            </tr>
          </table>
        </td>
      </tr>
    </table>
  </body>
</html>"#
        .to_string()
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn smtp_submission_is_not_retried_after_data_or_permanent_rejection() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        for reject in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = tokio::spawn(async move {
                let (socket, _) = listener.accept().await.unwrap();
                let mut socket = BufReader::new(socket);
                socket
                    .get_mut()
                    .write_all(b"220 localhost ESMTP\r\n")
                    .await
                    .unwrap();
                let mut deliveries = 0;
                let mut data = false;
                let mut line = String::new();
                loop {
                    line.clear();
                    if socket.read_line(&mut line).await.unwrap_or(0) == 0 {
                        break;
                    }
                    let reply: &[u8] = if data {
                        if line == ".\r\n" {
                            deliveries += 1;
                            data = false;
                            b"250 accepted\r\n"
                        } else {
                            continue;
                        }
                    } else if line.starts_with("EHLO") {
                        b"250 localhost\r\n"
                    } else if line.starts_with("MAIL") && reject {
                        b"550 sender rejected\r\n"
                    } else if line.starts_with("DATA") {
                        data = true;
                        b"354 continue\r\n"
                    } else if line.starts_with("QUIT") {
                        break;
                    } else {
                        b"250 ok\r\n"
                    };
                    socket.get_mut().write_all(reply).await.unwrap();
                    if deliveries > 0 {
                        break;
                    }
                }
                deliveries
            });
            let config = EmailConfig {
                smtp_host: "localhost".into(),
                smtp_port: port,
                smtp_username: None,
                smtp_password: None,
                smtp_tls: false,
                from_email: "test@example.com".into(),
                from_name: None,
                subject_prefix: None,
            };
            let message =
                smtp_test_message(&config, "recipient@example.com", EmailLocale::EnUs).unwrap();
            let result = send_message(&config, message).await;
            assert_eq!(result.is_ok(), !reject);
            assert_eq!(server.await.unwrap(), if reject { 0 } else { 1 });
        }
    }

    #[test]
    fn api_key_email_text_contains_full_key() {
        let body = api_key_email_text_body("sk-test123", api_key_email_content(EmailLocale::EnUs));

        assert!(body.contains("sk-test123"));
        assert!(body.contains("Your zhiqiaoapi API key is ready"));
    }

    #[test]
    fn api_key_email_html_contains_escaped_full_key() {
        let body =
            api_key_email_html_body("sk-test<&123", api_key_email_content(EmailLocale::EnUs));

        assert!(body.contains("sk-test&lt;&amp;123"));
        assert!(body.contains("Your zhiqiaoapi API key is ready"));
    }

    #[test]
    fn api_key_email_content_can_be_chinese() {
        let text = api_key_email_text_body("sk-test123", api_key_email_content(EmailLocale::ZhCn));
        let html = api_key_email_html_body("sk-test123", api_key_email_content(EmailLocale::ZhCn));

        assert!(text.contains("你的 智桥 API 密钥已生成"));
        assert!(text.contains("sk-test123"));
        assert!(html.contains("你的 智桥 API 密钥已生成"));
        assert!(html.contains("sk-test123"));
    }

    #[test]
    fn public_locale_defaults_to_chinese_and_accepts_english() {
        assert_eq!(EmailLocale::from_public_locale(None), EmailLocale::ZhCn);
        assert_eq!(EmailLocale::from_public_locale(Some("")), EmailLocale::ZhCn);
        assert_eq!(
            EmailLocale::from_public_locale(Some("zh-CN")),
            EmailLocale::ZhCn
        );
        assert_eq!(
            EmailLocale::from_public_locale(Some("en-US")),
            EmailLocale::EnUs
        );
        assert_eq!(
            EmailLocale::from_public_locale(Some("en")),
            EmailLocale::EnUs
        );
    }

    #[test]
    fn api_key_message_is_html_with_plain_text_part() {
        let service = EmailService::test();

        let message = service
            .api_key_message("user@example.com", "sk-test123", EmailLocale::ZhCn)
            .unwrap();
        let formatted = String::from_utf8(message.formatted()).unwrap();

        assert!(formatted.contains("Content-Type: multipart/alternative"));
        assert!(formatted.contains("Content-Type: multipart/related"));
        assert!(formatted.contains("Content-ID: <zhiqiao-brand>"));
        assert!(formatted.contains("Content-Type: image/png"));
        assert!(formatted.contains("Content-Type: text/plain"));
        assert!(formatted.contains("Content-Type: text/html"));
        assert!(formatted.contains("sk-test123"));
    }

    #[test]
    fn api_key_email_subject_uses_chinese_product_name() {
        assert_eq!(
            api_key_email_subject("智桥", api_key_email_content(EmailLocale::ZhCn)),
            "智桥 API 密钥"
        );
    }

    #[test]
    fn api_key_email_subject_uses_english_product_name() {
        assert_eq!(
            api_key_email_subject("智桥", api_key_email_content(EmailLocale::EnUs)),
            "智桥 API Key"
        );
    }

    #[test]
    fn password_reset_email_contains_reset_url() {
        let url = "https://example.com/reset-password?token=test";
        let text =
            password_reset_email_text_body(url, password_reset_email_content(EmailLocale::EnUs));
        let html =
            password_reset_email_html_body(url, password_reset_email_content(EmailLocale::EnUs));

        assert!(text.contains(url));
        assert!(text.contains("Reset your zhiqiaoapi password"));
        assert!(html.contains(url));
        assert!(html.contains("Reset your zhiqiaoapi password"));
    }

    #[test]
    fn login_verification_code_email_contains_code() {
        let text = login_verification_code_email_text_body(
            "123456",
            login_verification_code_email_content(EmailLocale::ZhCn),
        );
        let html = login_verification_code_email_html_body(
            "123456",
            login_verification_code_email_content(EmailLocale::ZhCn),
        );

        assert!(text.contains("123456"));
        assert!(text.contains("验证你的 智桥邮箱"));
        assert!(html.contains("123456"));
        assert!(html.contains("验证你的 智桥邮箱"));
    }

    #[test]
    fn product_name_follows_email_locale() {
        assert_eq!(EmailLocale::ZhCn.product_name(), "智桥");
        assert_eq!(EmailLocale::EnUs.product_name(), "Zhiqiao");
    }

    #[test]
    fn smtp_test_error_message_detects_login_failure() {
        let err = anyhow::anyhow!(
            "failed to send SMTP test email: permanent error (535): 5.7.0 ERR.LOGIN.PASSERR"
        );

        assert_eq!(smtp_test_error(&err).0, "smtp_authentication_failed");
    }

    #[test]
    fn smtp_test_error_message_detects_timeout() {
        let err = anyhow::anyhow!("failed to send SMTP test email: operation timed out");

        assert_eq!(smtp_test_error(&err).0, "smtp_connection_timed_out");
    }
}
