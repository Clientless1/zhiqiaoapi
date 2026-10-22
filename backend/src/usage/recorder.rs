use std::{
    collections::{BTreeSet, HashMap, HashSet},
    time::Duration,
};

use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use tokio::{
    sync::mpsc::{self, error::TrySendError},
    time,
};

use crate::{
    billing::{account, hold, receivable, BillingCharge, DebitPart},
    error::AppResult,
    id::DbId,
};

use super::{ActivityRecorder, UsageDailyRecorder};
use super::{KeyFailure, UsageInsert};

pub(super) mod record {
    use crate::billing::{BillingMeter, TokenUsage};

    use super::UsageInsert;

    #[derive(Clone, Copy)]
    pub(in crate::usage) struct UsageRecordFields<'a> {
        pub(in crate::usage) input_tokens: Option<i64>,
        pub(in crate::usage) output_tokens: Option<i64>,
        pub(in crate::usage) total_tokens: Option<i64>,
        pub(in crate::usage) billing_meter: BillingMeter,
        pub(in crate::usage) billable_units: i64,
        pub(in crate::usage) cost_micros: Option<i64>,
        pub(in crate::usage) billing_status: &'a str,
    }

    pub(in crate::usage) fn usage_fields(item: &UsageInsert) -> UsageRecordFields<'_> {
        let billing = item.billing.as_ref();
        UsageRecordFields {
            input_tokens: input_tokens(item),
            output_tokens: output_tokens(item),
            total_tokens: total_tokens(item),
            billing_meter: billing.map_or(item.billing_meter, |billing| billing.billing_meter),
            billable_units: billing
                .map_or(item.billable_units, |billing| billing.billable_units)
                .max(0),
            cost_micros: billing.map(|billing| billing.cost_micros),
            billing_status: billing.map_or("not_billed", |billing| billing.status.as_str()),
        }
    }

    pub(in crate::usage) fn input_tokens(item: &UsageInsert) -> Option<i64> {
        item.token_usage
            .map(|usage| usage.input_tokens)
            .or_else(|| {
                item.billing
                    .as_ref()
                    .and_then(|billing| billing.input_tokens)
            })
    }

    pub(in crate::usage) fn output_tokens(item: &UsageInsert) -> Option<i64> {
        item.token_usage
            .map(|usage| usage.output_tokens)
            .or_else(|| {
                item.billing
                    .as_ref()
                    .and_then(|billing| billing.output_tokens)
            })
    }

    pub(in crate::usage) fn total_tokens(item: &UsageInsert) -> Option<i64> {
        item.token_usage.map(TokenUsage::total_tokens).or_else(|| {
            item.billing
                .as_ref()
                .and_then(|billing| billing.total_tokens)
        })
    }
}

use record::usage_fields;

const USAGE_BATCH_SIZE: usize = 100;
const KEY_BATCH_SIZE: usize = 100;

#[derive(Clone)]
pub struct UsageRecorder {
    sender: Option<mpsc::Sender<UsageItem>>,
}

struct UsageItem {
    /// None 表示 failure-only 入队项（不写 usage 行），仅用于持久化 key failure。
    usage: Option<UsageInsert>,
    failure: Option<KeyFailure>,
}

impl UsageRecorder {
    pub fn spawn(
        pool: PgPool,
        flush_interval: Duration,
        queue_size: usize,
        activity: ActivityRecorder,
        daily: UsageDailyRecorder,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(queue_size.max(1));
        tokio::spawn(run_worker(pool, receiver, flush_interval, activity, daily));
        Self {
            sender: Some(sender),
        }
    }

    pub fn disabled() -> Self {
        Self { sender: None }
    }

    pub async fn enqueue(&self, usage: UsageInsert, failure: Option<KeyFailure>) -> AppResult<()> {
        let Some(sender) = &self.sender else {
            return Ok(());
        };

        let item = UsageItem {
            usage: Some(usage),
            failure,
        };
        match sender.try_send(item) {
            Ok(()) => {}
            Err(TrySendError::Full(_item)) => {
                tracing::warn!("relay usage queue is full; dropping non-billing usage record");
            }
            Err(TrySendError::Closed(_)) => {
                tracing::warn!("relay usage worker is closed; dropping usage record");
            }
        }
        Ok(())
    }

    /// billing 路径专用：仅持久化 key 冷却状态，不写 usage 行。
    /// 避免 billing 分支因直接 return 而跳过 `flush_key_failures` 导致
    /// `channel_key.cooldown_until` 只存在内存、routing cache TTL 到期后丢失。
    pub fn enqueue_key_failure(&self, failure: KeyFailure) {
        let Some(sender) = &self.sender else {
            return;
        };
        let item = UsageItem {
            usage: None,
            failure: Some(failure),
        };
        match sender.try_send(item) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                tracing::warn!("relay usage queue is full; dropping key failure record");
            }
            Err(TrySendError::Closed(_)) => {
                tracing::warn!("relay usage worker is closed; dropping key failure record");
            }
        }
    }
}

async fn run_worker(
    pool: PgPool,
    mut receiver: mpsc::Receiver<UsageItem>,
    flush_interval: Duration,
    activity: ActivityRecorder,
    daily: UsageDailyRecorder,
) {
    let mut interval = time::interval(flush_interval.max(Duration::from_millis(1)));
    let mut usages = Vec::with_capacity(USAGE_BATCH_SIZE);
    let mut failures: HashMap<DbId, KeyFailure> = HashMap::new();

    loop {
        tokio::select! {
            item = receiver.recv() => {
                match item {
                    Some(item) => {
                        if let Some(failure) = item.failure {
                            insert_bounded_failure(&mut failures, failure);
                        }
                        if let Some(usage) = item.usage {
                            push_bounded_usage(&mut usages, usage);
                        }
                        if usages.len() >= USAGE_BATCH_SIZE
                            || failures.len() >= KEY_BATCH_SIZE
                        {
                            flush(
                                &pool,
                                &activity,
                                &daily,
                                &mut usages,
                                &mut failures,
                            )
                            .await;
                        }
                    }
                    None => {
                        flush(
                            &pool,
                            &activity,
                            &daily,
                            &mut usages,
                            &mut failures,
                        )
                        .await;
                        break;
                    }
                }
            }
            _ = interval.tick() => {
                flush(
                    &pool,
                    &activity,
                    &daily,
                    &mut usages,
                    &mut failures,
                )
                .await;
            }
        }
    }
}

fn push_bounded_usage(usages: &mut Vec<UsageInsert>, usage: UsageInsert) {
    if usages.len() < USAGE_BATCH_SIZE {
        usages.push(usage);
        return;
    }
    // Buffer 已满。含 billing 字段的记录对应已实际扣款，丢弃会造成"有账单无 usage 行"
    // 的财务数据不一致——优先保留：找一条无 billing 的条目替换。
    if usage.billing.is_some() {
        if let Some(pos) = usages.iter().position(|u| u.billing.is_none()) {
            tracing::warn!(
                "relay usage flush buffer is full; evicting non-billing record to preserve billing usage"
            );
            usages[pos] = usage;
            return;
        }
        // 所有 slot 都是 billing 记录，buffer 已被同等高优先级占满，只能丢弃并 error。
        tracing::error!(
            "relay usage flush buffer is full with billing records; dropping billing usage record"
        );
    } else {
        tracing::warn!("relay usage flush buffer is full; dropping non-billing usage record");
    }
}

fn insert_bounded_failure(failures: &mut HashMap<DbId, KeyFailure>, failure: KeyFailure) {
    if failures.contains_key(&failure.channel_key_id) || failures.len() < KEY_BATCH_SIZE {
        failures.insert(failure.channel_key_id, failure);
        return;
    }
    tracing::warn!("relay key failure flush buffer is full; dropping key failure record");
}

async fn flush(
    pool: &PgPool,
    activity: &ActivityRecorder,
    daily: &UsageDailyRecorder,
    usages: &mut Vec<UsageInsert>,
    failures: &mut HashMap<DbId, KeyFailure>,
) {
    if usages.is_empty() && failures.is_empty() {
        return;
    }

    let Ok(mut tx) = pool.begin().await else {
        tracing::warn!("failed to begin relay usage flush transaction");
        return;
    };
    let result = async {
        if !usages.is_empty() {
            flush_usage(&mut tx, usages).await?;
        }
        if !failures.is_empty() {
            flush_key_failures(&mut tx, failures).await?;
        }
        tx.commit().await?;
        Ok::<(), crate::error::AppError>(())
    }
    .await;

    match result {
        Ok(()) => {
            activity.record(usages);
            daily.record(usages);
            failures.clear();
            usages.clear();
        }
        Err(err) => tracing::warn!("failed to flush relay usage records: {err}"),
    }
}

async fn flush_key_failures(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    failures: &HashMap<DbId, KeyFailure>,
) -> AppResult<()> {
    let mut query_builder = QueryBuilder::<Postgres>::new(
        "UPDATE channel_key AS ck
         SET cooldown_until = data.cooldown_until,
             last_error = data.error,
             updated_at = now()
         FROM (",
    );
    query_builder.push_values(failures.values(), |mut row, failure| {
        row.push_bind(failure.channel_key_id)
            .push_bind(failure.cooldown_until)
            .push_bind(failure.error.chars().take(500).collect::<String>());
    });
    query_builder.push(") AS data(id, cooldown_until, error) WHERE ck.id = data.id");
    query_builder.build().execute(&mut **tx).await?;
    Ok(())
}

pub(crate) async fn flush_usage(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    usages: &[UsageInsert],
) -> AppResult<()> {
    if usages.is_empty() {
        return Ok(());
    }

    // Async records can outlive their key, project or upstream configuration.
    // Lock surviving references until commit so a concurrent delete cannot race
    // the inserts; missing optional references must not prevent settlement.
    let references = UsageReferences::lock(tx, usages).await?;
    let debit_parts = usages
        .iter()
        .filter_map(|item| item.billing.as_ref())
        .flat_map(|billing| billing.parts.iter().chain(&billing.returned_parts))
        .collect::<Vec<_>>();
    let locked_accounts = account::lock_debit_parts(tx, &debit_parts).await?;
    let mut pending_usage = Vec::with_capacity(usages.len());

    for item in usages {
        if let Some(billing) = &item.billing {
            if billing.parts.is_empty() && billing.returned_parts.is_empty() {
                if item.routing.is_some() {
                    let row = insert_usage(tx, item, &references).await?;
                    let usage_id: DbId = row.try_get("id")?;
                    insert_usage_routing(tx, usage_id, item, &references).await?;
                } else {
                    pending_usage.push(item);
                }
                continue;
            }
            let row = insert_usage(tx, item, &references).await?;
            let usage_id: DbId = row.try_get("id")?;
            insert_usage_routing(tx, usage_id, item, &references).await?;
            let billing_parts = coalesce_debit_parts(&billing.parts);
            for part in &billing_parts {
                flush_billing_part(tx, usage_id, billing, part, &locked_accounts).await?;
            }
            for part in &billing.returned_parts {
                flush_returned_billing_part(tx, part).await?;
            }
            hold::complete(tx, billing.transaction_id).await?;
        } else {
            if item.routing.is_some() {
                let row = insert_usage(tx, item, &references).await?;
                let usage_id: DbId = row.try_get("id")?;
                insert_usage_routing(tx, usage_id, item, &references).await?;
            } else {
                pending_usage.push(item);
            }
        }
    }

    flush_unbilled_usage(tx, &pending_usage, &references).await?;
    Ok(())
}

#[derive(Default)]
struct UsageReferences {
    users: HashSet<DbId>,
    projects: HashSet<DbId>,
    keys: HashSet<DbId>,
    project_models: HashSet<DbId>,
    channels: HashSet<DbId>,
    channel_keys: HashSet<DbId>,
    credentials: HashSet<DbId>,
}

impl UsageReferences {
    async fn lock(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        usages: &[UsageInsert],
    ) -> AppResult<Self> {
        let mut users = BTreeSet::new();
        let mut projects = BTreeSet::new();
        let mut keys = BTreeSet::new();
        let mut project_models = BTreeSet::new();
        let mut channels = BTreeSet::new();
        let mut channel_keys = BTreeSet::new();
        let mut credentials = BTreeSet::new();
        for item in usages {
            users.insert(item.user_id);
            projects.insert(item.project_id);
            keys.insert(item.user_key_id);
            channels.insert(item.channel_id);
            channel_keys.extend(item.channel_key_id);
            credentials.extend(item.credential_id);
            if let Some(routing) = &item.routing {
                projects.insert(routing.project_id);
                project_models.insert(routing.project_model_id);
                channels.extend(routing.selected_channel_id);
            }
        }

        // Use a fixed parent/table and ascending row order for every batch.
        Ok(Self {
            users: lock_reference_ids(tx, "\"user\"", users).await?,
            projects: lock_reference_ids(tx, "project", projects).await?,
            keys: lock_reference_ids(tx, "user_key", keys).await?,
            project_models: lock_reference_ids(tx, "project_model", project_models).await?,
            channels: lock_reference_ids(tx, "channel", channels).await?,
            channel_keys: lock_reference_ids(tx, "channel_key", channel_keys).await?,
            credentials: lock_reference_ids(tx, "credential", credentials).await?,
        })
    }
}

async fn lock_reference_ids(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    table: &'static str,
    ids: BTreeSet<DbId>,
) -> AppResult<HashSet<DbId>> {
    if ids.is_empty() {
        return Ok(HashSet::new());
    }
    let ids = ids.into_iter().collect::<Vec<_>>();
    // Table names come exclusively from the static calls above.
    let mut query = QueryBuilder::<Postgres>::new("SELECT id FROM ");
    query
        .push(table)
        .push(" WHERE id = ANY(")
        .push_bind(ids)
        .push(") ORDER BY id FOR KEY SHARE");
    Ok(query
        .build_query_scalar::<DbId>()
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .collect())
}

fn surviving_reference(ids: &HashSet<DbId>, id: DbId) -> Option<DbId> {
    ids.contains(&id).then_some(id)
}

async fn insert_usage_routing(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    usage_id: DbId,
    item: &UsageInsert,
    references: &UsageReferences,
) -> AppResult<()> {
    let Some(routing) = &item.routing else {
        return Ok(());
    };
    // Routing details are owned by a project (ON DELETE CASCADE). The usage and
    // charge still persist if that project was deleted before this flush.
    if !references.projects.contains(&routing.project_id) {
        return Ok(());
    }
    sqlx::query(
        r#"
        INSERT INTO usage_routing
            (usage_id, project_id, project_model_id, requested_model, selected_model,
             selected_channel_id, decision_source, tier, task_type, confidence, reason_code,
             matched_rule_ids, candidate_summary, fallback_reason, classifier_model, latency_ms)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
        ON CONFLICT (usage_id) DO NOTHING
        "#,
    )
    .bind(usage_id)
    .bind(routing.project_id)
    .bind(surviving_reference(
        &references.project_models,
        routing.project_model_id,
    ))
    .bind(&routing.requested_model)
    .bind(&routing.selected_model)
    .bind(
        routing
            .selected_channel_id
            .and_then(|id| surviving_reference(&references.channels, id)),
    )
    .bind(&routing.decision_source)
    .bind(&routing.tier)
    .bind(&routing.task_type)
    .bind(routing.confidence)
    .bind(&routing.reason_code)
    .bind(sqlx::types::Json(&routing.matched_rule_ids))
    .bind(sqlx::types::Json(&routing.candidate_summary))
    .bind(routing.fallback_reason.as_deref())
    .bind(routing.classifier_model.as_deref())
    .bind(routing.latency_ms)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn flush_returned_billing_part(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    part: &DebitPart,
) -> AppResult<()> {
    // Mirrors `flush_billing_part`: if the allocation was already recovered by
    // the stale-allocation job, the unconsumed remainder was returned and the
    // reserved hold was released at that time. Returning again would exceed the
    // allocation's capacity (and double-release reserved credit), so skip it.
    if account::allocation_is_recovered(tx, part.allocation_id).await? {
        tracing::info!(
            allocation_id = %part.allocation_id,
            amount_micros = part.amount_micros,
            "skipping returned billing part on already-recovered credit allocation; credit was already refunded"
        );
        return Ok(());
    }
    account::mark_allocation_returned(tx, part.allocation_id, part.amount_micros).await?;
    account::decrement_reserved(tx, &part.credit_account, part.amount_micros).await?;
    Ok(())
}

async fn flush_unbilled_usage(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    usages: &[&UsageInsert],
    references: &UsageReferences,
) -> AppResult<()> {
    if usages.is_empty() {
        return Ok(());
    }

    let mut query_builder = QueryBuilder::<Postgres>::new(
        "INSERT INTO usage
         (user_id, project_id, user_key_id, channel_id, channel_key_id, credential_id,
          relay_trace_id, relay_attempt, relay_final,
          model, upstream_model, routing_phase, status_code,
          streamed, latency_ms, first_response_ms, output_tokens_per_second, error_summary,
          input_tokens, output_tokens, total_tokens, cache_in_tokens,
          cache_create_in_tokens, cache_create_5m_in_tokens,
          cache_create_1h_in_tokens, reason_out_tokens, audio_in_tokens,
          audio_out_tokens, billing_meter, billable_units,
          cost_micros, billing_status, billing_transaction_id)
         ",
    );
    query_builder.push_values(usages, |mut row, item| {
        let fields = usage_fields(item);
        row.push_bind(surviving_reference(&references.users, item.user_id))
            .push_bind(surviving_reference(&references.projects, item.project_id))
            .push_bind(surviving_reference(&references.keys, item.user_key_id))
            .push_bind(surviving_reference(&references.channels, item.channel_id))
            .push_bind(
                item.channel_key_id
                    .and_then(|id| surviving_reference(&references.channel_keys, id)),
            )
            .push_bind(
                item.credential_id
                    .and_then(|id| surviving_reference(&references.credentials, id)),
            )
            .push_bind(item.relay_trace_id)
            .push_bind(item.relay_attempt)
            .push_bind(item.relay_final)
            .push_bind(item.model.as_deref())
            .push_bind(item.upstream_model.as_deref())
            .push_bind(&item.routing_phase)
            .push_bind(item.status_code)
            .push_bind(item.streamed)
            .push_bind(item.latency_ms)
            .push_bind(item.first_response_ms)
            .push_bind(item.output_tokens_per_second)
            .push_bind(item.error_summary.as_deref())
            .push_bind(fields.input_tokens)
            .push_bind(fields.output_tokens)
            .push_bind(fields.total_tokens)
            .push_bind(item.token_usage.and_then(|usage| usage.cached_input_tokens))
            .push_bind(
                item.token_usage
                    .and_then(|usage| usage.cache_creation_input_tokens),
            )
            .push_bind(
                item.token_usage
                    .and_then(|usage| usage.cache_creation_input_tokens_5m),
            )
            .push_bind(
                item.token_usage
                    .and_then(|usage| usage.cache_creation_input_tokens_1h),
            )
            .push_bind(
                item.token_usage
                    .and_then(|usage| usage.reasoning_output_tokens),
            )
            .push_bind(item.token_usage.and_then(|usage| usage.audio_input_tokens))
            .push_bind(item.token_usage.and_then(|usage| usage.audio_output_tokens))
            .push_bind(fields.billing_meter.as_str())
            .push_bind(fields.billable_units)
            .push_bind(fields.cost_micros)
            .push_bind(fields.billing_status)
            .push_bind(item.billing.as_ref().map(|billing| billing.transaction_id));
    });
    query_builder.build().execute(&mut **tx).await?;
    Ok(())
}

async fn insert_usage(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    item: &UsageInsert,
    references: &UsageReferences,
) -> AppResult<sqlx::postgres::PgRow> {
    let fields = usage_fields(item);
    let row = sqlx::query(
        "INSERT INTO usage
         (user_id, project_id, user_key_id, channel_id, channel_key_id, credential_id,
          relay_trace_id, relay_attempt, relay_final,
          model, upstream_model, routing_phase, status_code,
          streamed, latency_ms, first_response_ms, output_tokens_per_second, error_summary,
          input_tokens, output_tokens, total_tokens, cache_in_tokens,
          cache_create_in_tokens, cache_create_5m_in_tokens,
          cache_create_1h_in_tokens, reason_out_tokens, audio_in_tokens,
          audio_out_tokens, billing_meter, billable_units,
          cost_micros, billing_status, billing_transaction_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                 $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                 $21, $22, $23, $24, $25, $26, $27, $28, $29, $30,
                 $31, $32, $33)
         RETURNING id",
    )
    .bind(surviving_reference(&references.users, item.user_id))
    .bind(surviving_reference(&references.projects, item.project_id))
    .bind(surviving_reference(&references.keys, item.user_key_id))
    .bind(surviving_reference(&references.channels, item.channel_id))
    .bind(
        item.channel_key_id
            .and_then(|id| surviving_reference(&references.channel_keys, id)),
    )
    .bind(
        item.credential_id
            .and_then(|id| surviving_reference(&references.credentials, id)),
    )
    .bind(item.relay_trace_id)
    .bind(item.relay_attempt)
    .bind(item.relay_final)
    .bind(item.model.as_deref())
    .bind(item.upstream_model.as_deref())
    .bind(&item.routing_phase)
    .bind(item.status_code)
    .bind(item.streamed)
    .bind(item.latency_ms)
    .bind(item.first_response_ms)
    .bind(item.output_tokens_per_second)
    .bind(item.error_summary.as_deref())
    .bind(fields.input_tokens)
    .bind(fields.output_tokens)
    .bind(fields.total_tokens)
    .bind(item.token_usage.and_then(|usage| usage.cached_input_tokens))
    .bind(
        item.token_usage
            .and_then(|usage| usage.cache_creation_input_tokens),
    )
    .bind(
        item.token_usage
            .and_then(|usage| usage.cache_creation_input_tokens_5m),
    )
    .bind(
        item.token_usage
            .and_then(|usage| usage.cache_creation_input_tokens_1h),
    )
    .bind(
        item.token_usage
            .and_then(|usage| usage.reasoning_output_tokens),
    )
    .bind(item.token_usage.and_then(|usage| usage.audio_input_tokens))
    .bind(item.token_usage.and_then(|usage| usage.audio_output_tokens))
    .bind(fields.billing_meter.as_str())
    .bind(fields.billable_units)
    .bind(fields.cost_micros)
    .bind(fields.billing_status)
    .bind(item.billing.as_ref().map(|billing| billing.transaction_id))
    .fetch_one(&mut **tx)
    .await?;
    Ok(row)
}

async fn flush_billing_part(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    usage_id: DbId,
    billing: &BillingCharge,
    part: &DebitPart,
    locked_accounts: &[crate::billing::CreditAccountId],
) -> AppResult<()> {
    // An expired lease or older deployment can still produce a late charge.
    // Recovery released only the reservation: successful usage remains owed.
    if account::allocation_is_recovered(tx, part.allocation_id).await? {
        return receivable::charge_recovered(tx, usage_id, billing, part, locked_accounts).await;
    }
    account::mark_allocation_consumed(tx, part.allocation_id, part.amount_micros).await?;
    let balance_after =
        account::debit_reserved_balance(tx, &part.credit_account, part.amount_micros).await?;

    sqlx::query(
        "INSERT INTO credit_ledger
         (credit_account_id, amount_micros, balance_after_micros, reason,
          usage_id, allocation_id, transaction_id, metadata)
         VALUES ($1, $2, $3, 'usage', $4, $5, $6, $7)",
    )
    .bind(part.credit_account.id)
    .bind(-part.amount_micros)
    .bind(balance_after)
    .bind(usage_id)
    .bind(part.allocation_id)
    .bind(billing.transaction_id)
    .bind(serde_json::json!({
        "billing_status": billing.status,
        "billing_meter": billing.billing_meter,
        "billable_units": billing.billable_units,
        "input_tokens": billing.input_tokens,
        "output_tokens": billing.output_tokens,
        "total_tokens": billing.total_tokens
    }))
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn coalesce_debit_parts(parts: &[DebitPart]) -> Vec<DebitPart> {
    let mut coalesced: Vec<DebitPart> = Vec::new();
    let mut indexes: HashMap<(DbId, DbId), usize> = HashMap::new();

    for part in parts {
        if part.amount_micros <= 0 {
            continue;
        }

        let key = (part.credit_account.id, part.allocation_id);
        if let Some(&index) = indexes.get(&key) {
            coalesced[index].amount_micros += part.amount_micros;
        } else {
            indexes.insert(key, coalesced.len());
            coalesced.push(part.clone());
        }
    }

    coalesced
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::billing::{BillingChargeStatus, BillingMeter, TokenUsage};
    use chrono::Utc;
    use uuid::Uuid;

    pub(in crate::usage) async fn seed_usage_references(pool: &PgPool) {
        sqlx::raw_sql(
            r#"
            INSERT INTO "user" (id, email, user_group_id)
              SELECT 91001, 'usage-reference@example.test', id FROM user_group WHERE is_default;
            INSERT INTO project (id, name, owner_user_id) VALUES (91001, 'Usage test', 91001);
            INSERT INTO user_key (id, user_id, project_id, key_prefix, secret_ciphertext)
              VALUES (91001, 91001, 91001, 'test', 'unused');
            INSERT INTO channel (id, provider, name) VALUES (91001, 'openai', 'Usage test');
            INSERT INTO channel_key (id, channel_id, name, key_prefix, secret_ciphertext)
              VALUES (91001, 91001, 'Test key', 'test', 'unused');
            INSERT INTO credential (id, provider, identity_hash, filename, content_ciphertext, content_sha256)
              VALUES (91001, 'openai', 'test', 'test.json', 'unused', 'test');
            INSERT INTO project_model (id, project_id, model, target_model)
              VALUES (91001, 91001, 'test-route', 'gpt-4.1');
            "#,
        )
        .execute(pool)
        .await
        .unwrap();
    }

    fn routed_usage() -> UsageInsert {
        let mut usage = usage_insert(91001);
        usage.credential_id = Some(91001);
        usage.billing = Some(billing_charge());
        usage.routing = Some(crate::project::models::UsageRoutingSnapshot {
            project_id: 91001,
            project_model_id: 91001,
            requested_model: "test-route".into(),
            selected_model: "gpt-4.1".into(),
            selected_channel_id: Some(91001),
            decision_source: "rules".into(),
            tier: "standard".into(),
            task_type: "unknown".into(),
            confidence: 1.0,
            reason_code: String::new(),
            matched_rule_ids: Vec::new(),
            candidate_summary: Vec::new(),
            fallback_reason: None,
            classifier_model: None,
            latency_ms: 0,
        });
        usage
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn usage_flush_keeps_history_after_references_are_deleted(pool: PgPool) {
        seed_usage_references(&pool).await;
        sqlx::raw_sql(
            "DELETE FROM user_key; DELETE FROM channel; DELETE FROM credential; DELETE FROM project_model;",
        )
        .execute(&pool)
        .await
        .unwrap();
        let mut unbilled = usage_insert(91001);
        unbilled.credential_id = Some(91001);
        let mut tx = pool.begin().await.unwrap();
        flush_usage(&mut tx, &[unbilled.clone(), routed_usage()])
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let rows = sqlx::query("SELECT user_id, project_id, user_key_id, channel_id, channel_key_id, credential_id FROM usage")
            .fetch_all(&pool).await.unwrap();
        assert_eq!(rows.len(), 2);
        for row in rows {
            assert_eq!(row.get::<Option<DbId>, _>("user_id"), Some(91001));
            assert_eq!(row.get::<Option<DbId>, _>("project_id"), Some(91001));
            for field in [
                "user_key_id",
                "channel_id",
                "channel_key_id",
                "credential_id",
            ] {
                assert_eq!(row.get::<Option<DbId>, _>(field), None);
            }
        }
        let routing =
            sqlx::query("SELECT project_model_id, selected_channel_id FROM usage_routing")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(routing.get::<Option<DbId>, _>("project_model_id"), None);
        assert_eq!(routing.get::<Option<DbId>, _>("selected_channel_id"), None);

        sqlx::query("DELETE FROM \"user\"")
            .execute(&pool)
            .await
            .unwrap();
        let mut tx = pool.begin().await.unwrap();
        flush_usage(&mut tx, &[unbilled, routed_usage()])
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let totals: (i64, i64, i64) = sqlx::query_as(
            "SELECT count(*), count(*) FILTER (WHERE user_id IS NULL AND project_id IS NULL), COALESCE(sum(cost_micros),0)::BIGINT FROM usage",
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(totals, (4, 4, 600));
        let routing_count: i64 = sqlx::query_scalar("SELECT count(*) FROM usage_routing")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(routing_count, 0);
    }

    fn billing_charge() -> BillingCharge {
        BillingCharge {
            transaction_id: Uuid::new_v4(),
            input_tokens: Some(10),
            output_tokens: Some(20),
            total_tokens: Some(30),
            billing_meter: BillingMeter::Token,
            billable_units: 30,
            cost_micros: 300,
            status: BillingChargeStatus::Billed,
            parts: Vec::new(),
            returned_parts: Vec::new(),
        }
    }

    pub(in crate::usage) fn usage_insert(id: DbId) -> UsageInsert {
        UsageInsert {
            user_id: id,
            project_id: id,
            user_key_id: id,
            channel_id: id,
            channel_key_id: Some(id),
            credential_id: None,
            relay_trace_id: None,
            relay_attempt: 1,
            relay_final: true,
            model: Some("gpt-4.1".to_string()),
            upstream_model: Some("gpt-4.1".to_string()),
            routing_phase: "relay".to_string(),
            routing: None,
            status_code: Some(200),
            streamed: false,
            latency_ms: 123,
            first_response_ms: None,
            output_tokens_per_second: None,
            error_summary: None,
            token_usage: None,
            billing_meter: BillingMeter::Token,
            billable_units: 0,
            billing: None,
        }
    }

    fn key_failure(id: DbId) -> KeyFailure {
        KeyFailure {
            channel_key_id: id,
            cooldown_until: Utc::now(),
            error: "upstream error".to_string(),
        }
    }

    #[test]
    fn billing_usage_payload_round_trips() {
        let charge = billing_charge();
        let usage = UsageInsert {
            user_id: 1,
            project_id: 1,
            user_key_id: 2,
            channel_id: 3,
            channel_key_id: Some(4),
            credential_id: None,
            relay_trace_id: None,
            relay_attempt: 1,
            relay_final: true,
            model: Some("gpt-4.1".to_string()),
            upstream_model: Some("gpt-4.1".to_string()),
            routing_phase: "relay".to_string(),
            routing: None,
            status_code: Some(200),
            streamed: false,
            latency_ms: 123,
            first_response_ms: None,
            output_tokens_per_second: None,
            error_summary: None,
            token_usage: Some(TokenUsage {
                input_tokens: 10,
                output_tokens: 20,
                cached_input_tokens: Some(5),
                cache_creation_input_tokens: None,
                cache_creation_input_tokens_5m: None,
                cache_creation_input_tokens_1h: None,
                reasoning_output_tokens: Some(3),
                audio_input_tokens: None,
                audio_output_tokens: None,
            }),
            billing_meter: BillingMeter::Token,
            billable_units: 30,
            billing: Some(charge.clone()),
        };

        let payload = serde_json::to_value(&usage).unwrap();
        let decoded: UsageInsert = serde_json::from_value(payload).unwrap();

        assert_eq!(
            decoded.billing.unwrap().transaction_id,
            charge.transaction_id
        );
        assert_eq!(decoded.channel_key_id, Some(4));
    }

    #[test]
    fn usage_flush_buffer_is_bounded() {
        let mut usages = Vec::new();
        for id in 0..USAGE_BATCH_SIZE as DbId {
            push_bounded_usage(&mut usages, usage_insert(id));
        }

        push_bounded_usage(&mut usages, usage_insert(999));

        assert_eq!(usages.len(), USAGE_BATCH_SIZE);
        assert!(!usages.iter().any(|usage| usage.user_id == 999));
    }

    #[test]
    fn key_failure_flush_buffer_is_bounded_but_updates_existing_key() {
        let mut failures = HashMap::new();
        for id in 0..KEY_BATCH_SIZE as DbId {
            insert_bounded_failure(&mut failures, key_failure(id));
        }

        insert_bounded_failure(&mut failures, key_failure(999));
        insert_bounded_failure(
            &mut failures,
            KeyFailure {
                channel_key_id: 1,
                cooldown_until: Utc::now(),
                error: "newer error".to_string(),
            },
        );

        assert_eq!(failures.len(), KEY_BATCH_SIZE);
        assert!(!failures.contains_key(&999));
        assert_eq!(failures.get(&1).unwrap().error, "newer error");
    }

    #[test]
    fn usage_record_fields_prefer_billing_charge_values() {
        let mut usage = usage_insert(1);
        usage.billing_meter = BillingMeter::Image;
        usage.billable_units = -10;
        usage.token_usage = Some(TokenUsage {
            input_tokens: 1,
            output_tokens: 2,
            cached_input_tokens: None,
            cache_creation_input_tokens: None,
            cache_creation_input_tokens_5m: None,
            cache_creation_input_tokens_1h: None,
            reasoning_output_tokens: None,
            audio_input_tokens: None,
            audio_output_tokens: None,
        });
        usage.billing = Some(BillingCharge {
            transaction_id: Uuid::new_v4(),
            input_tokens: Some(10),
            output_tokens: Some(20),
            total_tokens: Some(30),
            billing_meter: BillingMeter::Token,
            billable_units: 30,
            cost_micros: 300,
            status: BillingChargeStatus::Billed,
            parts: Vec::new(),
            returned_parts: Vec::new(),
        });

        let fields = usage_fields(&usage);

        assert_eq!(fields.input_tokens, Some(1));
        assert_eq!(fields.output_tokens, Some(2));
        assert_eq!(fields.total_tokens, Some(3));
        assert_eq!(fields.billing_meter, BillingMeter::Token);
        assert_eq!(fields.billable_units, 30);
        assert_eq!(fields.cost_micros, Some(300));
        assert_eq!(fields.billing_status, "billed");
    }

    #[test]
    fn usage_record_fields_fall_back_to_usage_values_without_billing_charge() {
        let mut usage = usage_insert(1);
        usage.billable_units = -10;

        let fields = usage_fields(&usage);

        assert_eq!(fields.input_tokens, None);
        assert_eq!(fields.output_tokens, None);
        assert_eq!(fields.total_tokens, None);
        assert_eq!(fields.billing_meter, BillingMeter::Token);
        assert_eq!(fields.billable_units, 0);
        assert_eq!(fields.cost_micros, None);
        assert_eq!(fields.billing_status, "not_billed");
    }

    #[test]
    fn coalesces_debit_parts_by_credit_account_and_allocation() {
        let parts = vec![
            serde_json::from_value(serde_json::json!({
                "credit_account": { "id": 7 },
                "allocation_id": 101,
                "amount_micros": 30,
                "generation": 1
            }))
            .unwrap(),
            serde_json::from_value(serde_json::json!({
                "credit_account": { "id": 7 },
                "allocation_id": 101,
                "amount_micros": 20,
                "generation": 2
            }))
            .unwrap(),
            serde_json::from_value(serde_json::json!({
                "credit_account": { "id": 7 },
                "allocation_id": 102,
                "amount_micros": 5,
                "generation": 3
            }))
            .unwrap(),
        ];

        let coalesced = coalesce_debit_parts(&parts);

        assert_eq!(coalesced.len(), 2);
        assert_eq!(coalesced[0].allocation_id, 101);
        assert_eq!(coalesced[0].amount_micros, 50);
        assert_eq!(coalesced[1].allocation_id, 102);
        assert_eq!(coalesced[1].amount_micros, 5);
    }
}
