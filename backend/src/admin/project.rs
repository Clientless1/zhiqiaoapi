use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Row};

use crate::{
    auth::{generate_user_key, key_prefix},
    billing::{account, CreditAccountId, CreditAccountType, DebitPart},
    error::{AppError, AppResult},
    id::DbId,
    input::{bounded_limit, trimmed_non_empty_owned},
    pagination::{created_id_cursor_page, parse_created_id_cursor},
    AppState,
};

#[derive(Debug, Serialize)]
pub struct ProjectRecord {
    pub id: DbId,
    pub name: String,
    pub owner_user_id: DbId,
    pub owner_email: String,
    pub owner_username: Option<String>,
    pub admin_display_names: Vec<String>,
    pub status: String,
    pub is_default: bool,
    pub member_count: i64,
    pub user_key_count: i64,
    pub project_model_count: i64,
    pub balance_micros: i64,
    pub reserved_micros: i64,
    pub available_micros: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ProjectMemberRecord {
    pub id: DbId,
    pub project_id: DbId,
    pub user_id: DbId,
    pub user_email: String,
    pub user_username: Option<String>,
    pub role: String,
    pub user_status: String,
    pub api_key: Option<String>,
    pub api_key_prefix: Option<String>,
    pub last_active_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CreatedProjectMember {
    pub record: ProjectMemberRecord,
    pub key: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreatedProject {
    pub record: ProjectRecord,
    pub key: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProjectPage {
    pub items: Vec<ProjectRecord>,
    pub limit: i64,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
pub struct ListProjectsQuery {
    pub search: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub owner_user_id: DbId,
    #[serde(default = "default_enabled_status")]
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub owner_user_id: Option<DbId>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpsertProjectMemberRequest {
    pub user_id: DbId,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectMemberRequest {
    pub role: String,
}

fn default_enabled_status() -> String {
    "enabled".to_string()
}

pub async fn list_projects(state: &AppState, query: ListProjectsQuery) -> AppResult<ProjectPage> {
    let limit = bounded_limit(query.limit, 50, 200);
    let cursor = parse_created_id_cursor(query.cursor.as_deref(), "invalid cursor")?;
    let search = trimmed_non_empty_owned(query.search.as_deref());
    let status = trimmed_non_empty_owned(query.status.as_deref());
    if let Some(status) = &status {
        validate_project_status(status)?;
    }

    let mut query_builder = QueryBuilder::new(
        r#"WITH page_projects AS (
               SELECT p.id, p.name, p.owner_user_id, p.status, p.is_default,
                      p.created_at, p.updated_at
               FROM project p"#,
    );

    query_builder.push(" WHERE p.deleted_at IS NULL");
    if let Some(search) = search {
        let search_pattern = format!("%{search}%");
        query_builder
            .push(" AND (p.name ILIKE ")
            .push_bind(search_pattern.clone())
            .push(
                r#" OR EXISTS (
                    SELECT 1
                    FROM project_member admin_member
                    JOIN "user" admin_user ON admin_user.id = admin_member.user_id
                    WHERE admin_member.project_id = p.id
                      AND admin_member.role = 'admin'
                      AND (admin_user.email::TEXT ILIKE "#,
            )
            .push_bind(search_pattern.clone())
            .push(" OR admin_user.username ILIKE ")
            .push_bind(search_pattern)
            .push("))")
            .push(")");
    }

    if let Some(status) = status {
        query_builder.push(" AND p.status = ").push_bind(status);
    }

    if let Some((created_at, id)) = cursor {
        query_builder
            .push(" AND (p.created_at, p.id) < (")
            .push_bind(created_at)
            .push(", ")
            .push_bind(id)
            .push(")");
    }

    query_builder
        .push(" ORDER BY p.created_at DESC, p.id DESC LIMIT ")
        .push_bind(limit + 1)
        .push(
            r#"
           )
           SELECT p.id, p.name, p.owner_user_id,
                  owner.email AS owner_email, owner.username AS owner_username,
                  COALESCE(admins.admin_display_names, ARRAY[]::TEXT[]) AS admin_display_names,
                  p.status, p.is_default,
                  COALESCE(pm.member_count, 0) AS member_count,
                  COALESCE(uk.user_key_count, 0) AS user_key_count,
                  COALESCE(pmodels.project_model_count, 0) AS project_model_count,
                  COALESCE(w.balance_micros, 0) AS balance_micros,
                  COALESCE(w.reserved_micros, 0) AS reserved_micros,
                  p.created_at, p.updated_at
           FROM page_projects p
           JOIN "user" owner ON owner.id = p.owner_user_id
           LEFT JOIN credit_account w ON w.owner_type = 'project' AND w.owner_id = p.id
           LEFT JOIN LATERAL (
               SELECT array_agg(
                          COALESCE(NULLIF(admin_user.username, ''), admin_user.email::TEXT)
                          ORDER BY admin_member.created_at ASC, admin_user.id ASC
                      ) AS admin_display_names
               FROM project_member admin_member
               JOIN "user" admin_user ON admin_user.id = admin_member.user_id
               WHERE admin_member.project_id = p.id
                 AND admin_member.role = 'admin'
           ) admins ON TRUE
           LEFT JOIN LATERAL (
               SELECT count(*) AS member_count
               FROM project_member member
               WHERE member.project_id = p.id
           ) pm ON TRUE
           LEFT JOIN LATERAL (
               SELECT count(*) AS user_key_count
               FROM user_key key
               WHERE key.project_id = p.id
           ) uk ON TRUE
           LEFT JOIN LATERAL (
               SELECT count(*) AS project_model_count
               FROM project_model model
               WHERE model.project_id = p.id
           ) pmodels ON TRUE
           ORDER BY p.created_at DESC, p.id DESC"#,
        );

    let rows = query_builder.build().fetch_all(&state.db.pool).await?;
    let (rows, next_cursor, has_more) = created_id_cursor_page(rows, limit)?;

    Ok(ProjectPage {
        items: rows
            .iter()
            .map(project_from_row)
            .collect::<Result<_, _>>()?,
        limit,
        next_cursor,
        has_more,
    })
}

pub async fn create_project(
    state: &AppState,
    req: CreateProjectRequest,
) -> AppResult<CreatedProject> {
    validate_project_status(&req.status)?;
    let name = normalize_project_name(&req.name)?;
    let mut tx = state.db.pool.begin().await?;
    ensure_user_exists_in_tx(&mut tx, req.owner_user_id).await?;
    let row = sqlx::query(
        "INSERT INTO project (name, owner_user_id, status, is_default)
         VALUES ($1, $2, $3, FALSE)
         RETURNING id",
    )
    .bind(name)
    .bind(req.owner_user_id)
    .bind(req.status)
    .fetch_one(&mut *tx)
    .await?;
    let project_id: DbId = row.try_get("id")?;
    sqlx::query(
        "INSERT INTO project_member (project_id, user_id, role)
         VALUES ($1, $2, 'admin')
         ON CONFLICT (project_id, user_id)
         DO UPDATE SET role = 'admin', updated_at = now()",
    )
    .bind(project_id)
    .bind(req.owner_user_id)
    .execute(&mut *tx)
    .await?;
    account::create_credit_account(&mut tx, CreditAccountType::Project, project_id).await?;
    let key =
        ensure_project_member_user_key_in_tx(state, &mut tx, project_id, req.owner_user_id).await?;
    tx.commit().await?;
    Ok(CreatedProject {
        record: get_project(state, project_id).await?,
        key,
    })
}

pub async fn update_project(
    state: &AppState,
    id: DbId,
    req: UpdateProjectRequest,
) -> AppResult<ProjectRecord> {
    if let Some(status) = &req.status {
        validate_project_status(status)?;
    }
    let name = req
        .name
        .as_deref()
        .map(normalize_project_name)
        .transpose()?;
    let mut tx = state.db.pool.begin().await?;
    if let Some(owner_user_id) = req.owner_user_id {
        set_project_admin_in_tx(state, &mut tx, id, owner_user_id).await?;
    }
    let row = sqlx::query(
        "UPDATE project
         SET name = COALESCE($2, name),
             owner_user_id = COALESCE($3, owner_user_id),
             status = COALESCE($4, status),
             updated_at = now()
         WHERE id = $1
         RETURNING id",
    )
    .bind(id)
    .bind(name)
    .bind(req.owner_user_id)
    .bind(req.status.as_deref())
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    let project_id: DbId = row.try_get("id")?;
    tx.commit().await?;
    invalidate_project_keys(state, project_id).await?;
    get_project(state, project_id).await
}

pub async fn delete_project(state: &AppState, id: DbId) -> AppResult<()> {
    let row = sqlx::query(
        "SELECT p.is_default
         FROM project p
         WHERE p.id = $1 AND p.deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    if row.try_get::<bool, _>("is_default")? {
        return Err(AppError::BadRequest(
            "default project cannot be deleted".to_string(),
        ));
    }

    recover_project_hot_credit_accounts(state, id).await?;
    invalidate_project_keys(state, id).await?;

    let mut tx = state.db.pool.begin().await?;
    sqlx::query("DELETE FROM project_member WHERE project_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let result = sqlx::query(
        "UPDATE project
         SET status = 'disabled', deleted_at = now(), updated_at = now()
         WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

pub async fn list_project_members(
    state: &AppState,
    project_id: DbId,
) -> AppResult<Vec<ProjectMemberRecord>> {
    let rows = sqlx::query(
        "SELECT pm.id, pm.project_id, pm.user_id,
                u.email AS user_email, u.username AS user_username,
                pm.role, u.status AS user_status,
                member_key.id AS api_key_id,
                member_key.key_prefix AS api_key_prefix,
                member_key.secret_ciphertext AS api_key_secret_ciphertext,
                pm.last_active_at,
                pm.created_at, pm.updated_at
         FROM project_member pm
         JOIN \"user\" u ON u.id = pm.user_id
         LEFT JOIN LATERAL (
             SELECT uk.id, uk.key_prefix, uk.secret_ciphertext
             FROM user_key uk
             WHERE uk.project_id = pm.project_id
               AND uk.owner_user_id = pm.user_id
             ORDER BY uk.created_at ASC, uk.id ASC
             LIMIT 1
         ) member_key ON TRUE
         WHERE pm.project_id = $1
         ORDER BY
           CASE pm.role
             WHEN 'owner' THEN 1
             WHEN 'admin' THEN 2
             WHEN 'member' THEN 3
             ELSE 4
           END,
           pm.created_at ASC,
           pm.id ASC",
    )
    .bind(project_id)
    .fetch_all(&state.db.pool)
    .await?;
    rows.iter()
        .map(|row| project_member_from_row(state, row))
        .collect::<Result<_, _>>()
}

pub async fn add_project_member(
    state: &AppState,
    project_id: DbId,
    req: UpsertProjectMemberRequest,
) -> AppResult<CreatedProjectMember> {
    validate_editable_project_member_role(&req.role)?;
    let mut tx = state.db.pool.begin().await?;
    ensure_project_exists_in_tx(&mut tx, project_id).await?;
    ensure_user_exists_in_tx(&mut tx, req.user_id).await?;
    let current = sqlx::query(
        "SELECT id, role
         FROM project_member
         WHERE project_id = $1 AND user_id = $2
         FOR UPDATE",
    )
    .bind(project_id)
    .bind(req.user_id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(row) = current {
        let member_id: DbId = row.try_get("id")?;
        let role: String = row.try_get("role")?;
        if role == "owner" {
            return Err(AppError::BadRequest(
                "project owner role cannot be changed".to_string(),
            ));
        }
        if req.role == "member" && (role == "admin" || role == "member") {
            tx.commit().await?;
            return Ok(CreatedProjectMember {
                record: get_project_member(state, project_id, member_id).await?,
                key: None,
            });
        }
    }
    if req.role == "admin" {
        set_project_admin_in_tx(state, &mut tx, project_id, req.user_id).await?;
    }
    let row = sqlx::query(
        "INSERT INTO project_member (project_id, user_id, role)
         VALUES ($1, $2, $3)
         ON CONFLICT (project_id, user_id)
         DO UPDATE SET role = EXCLUDED.role, updated_at = now()
         RETURNING id",
    )
    .bind(project_id)
    .bind(req.user_id)
    .bind(req.role)
    .fetch_one(&mut *tx)
    .await?;
    let member_id: DbId = row.try_get("id")?;
    let key = ensure_project_member_user_key_in_tx(state, &mut tx, project_id, req.user_id).await?;
    tx.commit().await?;
    Ok(CreatedProjectMember {
        record: get_project_member(state, project_id, member_id).await?,
        key,
    })
}

pub async fn update_project_member(
    state: &AppState,
    project_id: DbId,
    member_id: DbId,
    req: UpdateProjectMemberRequest,
) -> AppResult<ProjectMemberRecord> {
    validate_editable_project_member_role(&req.role)?;
    let current = get_project_member(state, project_id, member_id).await?;
    if current.role == "owner" {
        return Err(AppError::BadRequest(
            "project owner role cannot be changed".to_string(),
        ));
    }
    let mut tx = state.db.pool.begin().await?;
    if req.role == "admin" {
        set_project_admin_in_tx(state, &mut tx, project_id, current.user_id).await?;
    }
    let result = sqlx::query(
        "UPDATE project_member
         SET role = $3, updated_at = now()
         WHERE project_id = $1 AND id = $2
           AND role <> 'owner'",
    )
    .bind(project_id)
    .bind(member_id)
    .bind(req.role)
    .execute(&mut *tx)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    tx.commit().await?;
    get_project_member(state, project_id, member_id).await
}

pub async fn delete_project_member(
    state: &AppState,
    project_id: DbId,
    member_id: DbId,
) -> AppResult<()> {
    let current = get_project_member(state, project_id, member_id).await?;
    if current.role == "owner" {
        return Err(AppError::BadRequest(
            "project owner cannot be removed".to_string(),
        ));
    }
    recover_project_member_user_key_hot_credit_accounts(state, project_id, current.user_id).await?;
    invalidate_project_member_user_keys(state, project_id, current.user_id).await?;
    let result = sqlx::query(
        "WITH deleted_keys AS (
             DELETE FROM user_key
             WHERE project_id = $1 AND owner_user_id = $3
         )
         DELETE FROM project_member
         WHERE project_id = $1 AND id = $2
           AND role <> 'owner'",
    )
    .bind(project_id)
    .bind(member_id)
    .bind(current.user_id)
    .execute(&state.db.pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

async fn get_project(state: &AppState, id: DbId) -> AppResult<ProjectRecord> {
    let row = sqlx::query(
        "SELECT p.id, p.name, p.owner_user_id,
                owner.email AS owner_email, owner.username AS owner_username,
                COALESCE(admins.admin_display_names, ARRAY[]::TEXT[]) AS admin_display_names,
                p.status, p.is_default,
                COALESCE(pm.member_count, 0) AS member_count,
                COALESCE(uk.user_key_count, 0) AS user_key_count,
                COALESCE(pmodels.project_model_count, 0) AS project_model_count,
                COALESCE(w.balance_micros, 0) AS balance_micros,
                COALESCE(w.reserved_micros, 0) AS reserved_micros,
                p.created_at, p.updated_at
         FROM project p
         JOIN \"user\" owner ON owner.id = p.owner_user_id
         LEFT JOIN credit_account w ON w.owner_type = 'project' AND w.owner_id = p.id
         LEFT JOIN LATERAL (
             SELECT array_agg(
                        COALESCE(NULLIF(admin_user.username, ''), admin_user.email::TEXT)
                        ORDER BY admin_member.created_at ASC, admin_user.id ASC
                    ) AS admin_display_names
             FROM project_member admin_member
             JOIN \"user\" admin_user ON admin_user.id = admin_member.user_id
             WHERE admin_member.project_id = p.id
               AND admin_member.role = 'admin'
         ) admins ON TRUE
         LEFT JOIN LATERAL (
             SELECT count(*) AS member_count
             FROM project_member member
             WHERE member.project_id = p.id
         ) pm ON TRUE
         LEFT JOIN LATERAL (
             SELECT count(*) AS user_key_count
             FROM user_key key
             WHERE key.project_id = p.id
         ) uk ON TRUE
         LEFT JOIN LATERAL (
             SELECT count(*) AS project_model_count
             FROM project_model model
             WHERE model.project_id = p.id
         ) pmodels ON TRUE
         WHERE p.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    project_from_row(&row)
}

async fn ensure_project_member_user_key_in_tx(
    state: &AppState,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: DbId,
    user_id: DbId,
) -> AppResult<Option<String>> {
    let existing = sqlx::query(
        "SELECT id
         FROM user_key
         WHERE project_id = $1 AND owner_user_id = $2
         LIMIT 1
         FOR KEY SHARE",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;
    if existing.is_some() {
        return Ok(None);
    }

    let key = generate_user_key();
    let secret_ciphertext = state.secrets.encrypt(&key)?;
    let row = sqlx::query(
        "INSERT INTO user_key
            (user_id, project_id, owner_user_id, name, key_prefix, secret_ciphertext, status)
         SELECT $1, $2, $3, p.name, $4, $5, 'enabled'
         FROM project p
         WHERE p.id = $2
         RETURNING id",
    )
    .bind(user_id)
    .bind(project_id)
    .bind(user_id)
    .bind(key_prefix(&key))
    .bind(secret_ciphertext)
    .fetch_one(&mut **tx)
    .await?;
    let user_key_id: DbId = row.try_get("id")?;
    account::create_credit_account(tx, CreditAccountType::UserKey, user_key_id).await?;
    Ok(Some(key))
}

async fn set_project_admin_in_tx(
    state: &AppState,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: DbId,
    user_id: DbId,
) -> AppResult<()> {
    ensure_user_exists_in_tx(tx, user_id).await?;
    ensure_project_exists_in_tx(tx, project_id).await?;
    sqlx::query(
        "UPDATE project_member
         SET role = 'member', updated_at = now()
         WHERE project_id = $1 AND user_id <> $2 AND role = 'admin'",
    )
    .bind(project_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO project_member (project_id, user_id, role)
         VALUES ($1, $2, 'admin')
         ON CONFLICT (project_id, user_id)
         DO UPDATE SET role = 'admin', updated_at = now()",
    )
    .bind(project_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE project
         SET owner_user_id = $2, updated_at = now()
         WHERE id = $1",
    )
    .bind(project_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    ensure_project_member_user_key_in_tx(state, tx, project_id, user_id).await?;
    Ok(())
}

async fn get_project_member(
    state: &AppState,
    project_id: DbId,
    member_id: DbId,
) -> AppResult<ProjectMemberRecord> {
    let row = sqlx::query(
        "SELECT pm.id, pm.project_id, pm.user_id,
                u.email AS user_email, u.username AS user_username,
                pm.role, u.status AS user_status,
                member_key.id AS api_key_id,
                member_key.key_prefix AS api_key_prefix,
                member_key.secret_ciphertext AS api_key_secret_ciphertext,
                pm.last_active_at,
                pm.created_at, pm.updated_at
         FROM project_member pm
         JOIN \"user\" u ON u.id = pm.user_id
         LEFT JOIN LATERAL (
             SELECT uk.id, uk.key_prefix, uk.secret_ciphertext
             FROM user_key uk
             WHERE uk.project_id = pm.project_id
               AND uk.owner_user_id = pm.user_id
             ORDER BY uk.created_at ASC, uk.id ASC
             LIMIT 1
         ) member_key ON TRUE
         WHERE pm.project_id = $1 AND pm.id = $2",
    )
    .bind(project_id)
    .bind(member_id)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    project_member_from_row(state, &row)
}

async fn ensure_project_exists_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: DbId,
) -> AppResult<()> {
    let exists = sqlx::query("SELECT id FROM project WHERE id = $1 FOR KEY SHARE")
        .bind(project_id)
        .fetch_optional(&mut **tx)
        .await?
        .is_some();
    if !exists {
        return Err(AppError::NotFound);
    }
    Ok(())
}

async fn ensure_user_exists_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: DbId,
) -> AppResult<()> {
    let exists = sqlx::query("SELECT id FROM \"user\" WHERE id = $1 FOR KEY SHARE")
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
        .is_some();
    if !exists {
        return Err(AppError::BadRequest(
            "project admin user does not exist".to_string(),
        ));
    }
    Ok(())
}

async fn invalidate_project_keys(state: &AppState, project_id: DbId) -> AppResult<()> {
    let rows = sqlx::query("SELECT id FROM user_key WHERE project_id = $1")
        .bind(project_id)
        .fetch_all(&state.db.pool)
        .await?;
    invalidate_user_key_rows(state, rows).await
}

async fn invalidate_project_member_user_keys(
    state: &AppState,
    project_id: DbId,
    user_id: DbId,
) -> AppResult<()> {
    let rows = sqlx::query("SELECT id FROM user_key WHERE project_id = $1 AND owner_user_id = $2")
        .bind(project_id)
        .bind(user_id)
        .fetch_all(&state.db.pool)
        .await?;
    invalidate_user_key_rows(state, rows).await
}

async fn invalidate_user_key_rows(
    state: &AppState,
    rows: Vec<sqlx::postgres::PgRow>,
) -> AppResult<()> {
    for row in rows {
        let id: DbId = row.try_get("id")?;
        state
            .cache_invalidator
            .invalidate(state, crate::cache::InvalidationEvent::UserKey { id })
            .await;
    }
    Ok(())
}

async fn recover_project_hot_credit_accounts(state: &AppState, project_id: DbId) -> AppResult<()> {
    let rows = sqlx::query(
        "SELECT w.id
         FROM project p
         JOIN credit_account w ON w.owner_type = 'project' AND w.owner_id = p.id
         WHERE p.id = $1
         UNION ALL
         SELECT w.id
         FROM user_key uk
         JOIN credit_account w ON w.owner_type = 'user_key' AND w.owner_id = uk.id
         WHERE uk.project_id = $1
         UNION ALL
         SELECT w.id
         FROM user_key uk
         JOIN user_key_model ukm ON ukm.user_key_id = uk.id
         JOIN credit_account w ON w.owner_type = 'user_key_model' AND w.owner_id = ukm.id
         WHERE uk.project_id = $1",
    )
    .bind(project_id)
    .fetch_all(&state.db.pool)
    .await?;

    recover_hot_credit_account_rows(state, rows).await
}

async fn recover_project_member_user_key_hot_credit_accounts(
    state: &AppState,
    project_id: DbId,
    user_id: DbId,
) -> AppResult<()> {
    let rows = sqlx::query(
        "SELECT w.id
         FROM user_key uk
         JOIN credit_account w ON w.owner_type = 'user_key' AND w.owner_id = uk.id
         WHERE uk.project_id = $1 AND uk.owner_user_id = $2
         UNION ALL
         SELECT w.id
         FROM user_key uk
         JOIN user_key_model ukm ON ukm.user_key_id = uk.id
         JOIN credit_account w ON w.owner_type = 'user_key_model' AND w.owner_id = ukm.id
         WHERE uk.project_id = $1 AND uk.owner_user_id = $2",
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_all(&state.db.pool)
    .await?;

    recover_hot_credit_account_rows(state, rows).await
}

async fn recover_hot_credit_account(
    state: &AppState,
    credit_account: CreditAccountId,
) -> AppResult<()> {
    let mut tx = state.db.pool.begin().await?;
    let recovered = state
        .billing
        .drain_hot_credit_account(&credit_account)
        .await?;
    recover_hot_credit_in_tx(&mut tx, &recovered).await?;
    tx.commit().await?;
    Ok(())
}

async fn recover_hot_credit_account_rows(
    state: &AppState,
    rows: Vec<sqlx::postgres::PgRow>,
) -> AppResult<()> {
    for row in rows {
        recover_hot_credit_account(state, CreditAccountId::new(row.try_get("id")?)).await?;
    }
    Ok(())
}

async fn recover_hot_credit_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    parts: &[DebitPart],
) -> AppResult<()> {
    crate::billing::Billing::release_hold_in_transaction(
        tx,
        &crate::billing::DebitHold::new(0, parts.to_vec(), true),
    )
    .await
}

fn validate_project_status(status: &str) -> AppResult<()> {
    match status {
        "enabled" | "disabled" => Ok(()),
        other => Err(AppError::BadRequest(format!("invalid status: {other}"))),
    }
}

fn validate_editable_project_member_role(role: &str) -> AppResult<()> {
    match role {
        "admin" | "member" => Ok(()),
        other => Err(AppError::BadRequest(format!(
            "invalid editable project member role: {other}"
        ))),
    }
}

fn normalize_project_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("project name is required".to_string()));
    }
    if name.chars().count() > 80 {
        return Err(AppError::BadRequest(
            "project name must be 80 characters or fewer".to_string(),
        ));
    }
    Ok(name.to_string())
}

fn project_from_row(row: &sqlx::postgres::PgRow) -> AppResult<ProjectRecord> {
    let balance_micros = row.try_get("balance_micros")?;
    let reserved_micros = row.try_get("reserved_micros")?;
    Ok(ProjectRecord {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        owner_user_id: row.try_get("owner_user_id")?,
        owner_email: row.try_get("owner_email")?,
        owner_username: row.try_get("owner_username")?,
        admin_display_names: row.try_get("admin_display_names")?,
        status: row.try_get("status")?,
        is_default: row.try_get("is_default")?,
        member_count: row.try_get("member_count")?,
        user_key_count: row.try_get("user_key_count")?,
        project_model_count: row.try_get("project_model_count")?,
        balance_micros,
        reserved_micros,
        available_micros: balance_micros - reserved_micros,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn project_member_from_row(
    state: &AppState,
    row: &sqlx::postgres::PgRow,
) -> AppResult<ProjectMemberRecord> {
    let api_key_id: Option<DbId> = row.try_get("api_key_id")?;
    let api_key_secret_ciphertext: Option<String> = row.try_get("api_key_secret_ciphertext")?;
    let api_key = match (api_key_id, api_key_secret_ciphertext) {
        (Some(id), Some(secret_ciphertext)) => {
            Some(state.secrets.plaintext(id, &secret_ciphertext)?)
        }
        _ => None,
    };
    Ok(ProjectMemberRecord {
        id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        user_id: row.try_get("user_id")?,
        user_email: row.try_get("user_email")?,
        user_username: row.try_get("user_username")?,
        role: row.try_get("role")?,
        user_status: row.try_get("user_status")?,
        api_key,
        api_key_prefix: row.try_get("api_key_prefix")?,
        last_active_at: row.try_get("last_active_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn project_wallet_ownership_migration_backfills_and_tracks_transfers(pool: PgPool) {
        sqlx::raw_sql(
            r#"
            INSERT INTO "user" (id,email,user_group_id)
              SELECT 99301,'project-creator@example.test',id FROM user_group WHERE is_default;
            INSERT INTO "user" (id,email,user_group_id)
              SELECT 99302,'project-new-owner@example.test',id FROM user_group WHERE is_default;
            INSERT INTO project (id,name,owner_user_id) VALUES (99301,'Transferred',99302);
            INSERT INTO user_key (id,user_id,project_id,key_prefix,secret_ciphertext)
              VALUES (99301,99301,99301,'creator','unused');
            INSERT INTO user_key_model (id,user_key_id,model) VALUES (99301,99301,'gpt-4.1');
            INSERT INTO credit_account (owner_type,owner_id,owner_user_id)
              VALUES ('project',99301,99301),('project',99302,99301),
                     ('user_key',99301,99301),('user_key_model',99301,99301);
            UPDATE credit_account SET owner_user_id=99301 WHERE owner_type='project' AND owner_id=99301;
            DROP TRIGGER project_credit_account_owner_sync ON project;
            DROP FUNCTION sync_project_credit_account_owner();
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!(
            "../../migrations/0016_project_credit_account_ownership.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();
        let expected_owners = vec![
            ("project".to_string(), 99301, Some(99302)),
            ("project".to_string(), 99302, Some(99301)),
            ("user_key".to_string(), 99301, Some(99301)),
            ("user_key_model".to_string(), 99301, Some(99301)),
        ];
        let owners: Vec<(String, DbId, Option<DbId>)> = sqlx::query_as(
            "SELECT owner_type,owner_id,owner_user_id FROM credit_account ORDER BY owner_type,owner_id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(owners, expected_owners);

        let mut transfer = pool.begin().await.unwrap();
        sqlx::query("UPDATE project SET owner_user_id=99301 WHERE id=99301")
            .execute(&mut *transfer)
            .await
            .unwrap();
        let transferred_owner: Option<DbId> = sqlx::query_scalar(
            "SELECT owner_user_id FROM credit_account WHERE owner_type='project' AND owner_id=99301",
        )
        .fetch_one(&mut *transfer)
        .await
        .unwrap();
        assert_eq!(transferred_owner, Some(99301));
        let visible_owner: Option<DbId> = sqlx::query_scalar(
            "SELECT owner_user_id FROM credit_account WHERE owner_type='project' AND owner_id=99301",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(visible_owner, Some(99302));
        transfer.rollback().await.unwrap();

        sqlx::raw_sql(
            "UPDATE project SET owner_user_id=99301 WHERE id=99301;
             UPDATE project SET owner_user_id=99302 WHERE id=99301;
             UPDATE project SET status='disabled',deleted_at=now() WHERE id=99301;
             DELETE FROM project WHERE id=99301;",
        )
        .execute(&pool)
        .await
        .unwrap();
        let retained_owners: Vec<(String, DbId, Option<DbId>)> = sqlx::query_as(
            "SELECT owner_type,owner_id,owner_user_id FROM credit_account ORDER BY owner_type,owner_id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(retained_owners, expected_owners);
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn project_credit_recovery_handles_mixed_wallets_and_recovered_allocations(pool: PgPool) {
        let mut parts = Vec::new();
        for (owner_id, amount) in [(99221, 30), (99222, 50)] {
            let account_id: DbId = sqlx::query_scalar(
                "INSERT INTO credit_account (owner_type,owner_id,balance_micros,reserved_micros)
                 VALUES ('project',$1,1000,$2) RETURNING id",
            )
            .bind(owner_id)
            .bind(amount)
            .fetch_one(&pool)
            .await
            .unwrap();
            let allocation_id: DbId = sqlx::query_scalar(
                "INSERT INTO credit_allocation (credit_account_id,amount_micros)
                 VALUES ($1,$2) RETURNING id",
            )
            .bind(account_id)
            .bind(amount)
            .fetch_one(&pool)
            .await
            .unwrap();
            parts.push(DebitPart {
                credit_account: CreditAccountId::new(account_id),
                allocation_id,
                amount_micros: amount,
            });
        }
        let recovered_id: DbId = sqlx::query_scalar(
            "INSERT INTO credit_allocation
             (credit_account_id,amount_micros,returned_micros,status)
             VALUES ($1,70,70,'recovered') RETURNING id",
        )
        .bind(parts[0].credit_account.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        parts.push(DebitPart {
            credit_account: parts[0].credit_account.clone(),
            allocation_id: recovered_id,
            amount_micros: 70,
        });
        parts.reverse();

        let mut tx = pool.begin().await.unwrap();
        recover_hot_credit_in_tx(&mut tx, &parts).await.unwrap();
        tx.commit().await.unwrap();

        let wallets: Vec<(i64, i64)> =
            sqlx::query_as("SELECT balance_micros,reserved_micros FROM credit_account ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(wallets, vec![(1000, 0), (1000, 0)]);
        let allocations: Vec<(i64, String)> =
            sqlx::query_as("SELECT returned_micros,status FROM credit_allocation ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            allocations,
            vec![
                (30, "settled".into()),
                (50, "settled".into()),
                (70, "recovered".into()),
            ]
        );
    }
}
