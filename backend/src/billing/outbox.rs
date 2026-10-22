use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use tokio::{
    sync::mpsc::{self, error::TryRecvError, Receiver},
    task::JoinHandle,
    time,
};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    id::DbId,
    usage::{self, ActivityRecorder, UsageDailyRecorder, UsageInsert},
};

const BILLING_BATCH_SIZE: i64 = 500;
const BILLING_PROCESS_CHUNK_SIZE: i64 = 500;
const BILLING_MAX_BATCHES_PER_TICK: usize = 40;
const BILLING_PROCESS_WORKERS: usize = 4;
const BILLING_MAX_ATTEMPTS: i32 = 10;
const BILLING_OUTBOX_WRITE_ATTEMPTS: u32 = 7;
const BILLING_OUTBOX_INITIAL_RETRY_DELAY: Duration = Duration::from_millis(50);
const BILLING_OUTBOX_BACKGROUND_RETRY_INITIAL_DELAY: Duration = Duration::from_secs(1);
const BILLING_OUTBOX_BACKGROUND_RETRY_MAX_DELAY: Duration = Duration::from_secs(60);
/// 后台 retry worker 的最大重试次数。超过后将整批记录逐条提交至 billing outbox
/// 表（process worker 负责处理），避免单条损坏记录永久阻塞整个 retry batch。
const BILLING_RETRY_WORKER_MAX_ATTEMPTS: u32 = 20;

#[derive(Clone)]
pub struct BillingOutbox {
    pool: PgPool,
    health: BillingOutboxHealth,
    worker: WorkerSlot,
    retry_worker: WorkerSlot,
    activity: ActivityRecorder,
    daily: UsageDailyRecorder,
}

#[derive(Clone, Default)]
struct BillingOutboxHealth {
    failed_since: Arc<RwLock<Option<DateTime<Utc>>>>,
}

#[derive(Clone, Default)]
struct WorkerSlot {
    sender: Arc<RwLock<Option<mpsc::Sender<UsageInsert>>>>,
    handle: Arc<RwLock<Option<JoinHandle<()>>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BillingOutboxWriteStatus {
    pub healthy: bool,
    pub failed_since: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct BillingOutboxBacklogStatus {
    pub pending_count: i64,
    pub oldest_pending_age_seconds: i64,
    /// Dead-lettered records that exhausted all retry attempts. Reported for
    /// observability only — they are not retried and must NOT gate readiness,
    /// otherwise stale data rows can permanently block service startup.
    pub failed_count: i64,
}

struct PendingBillingRecord {
    id: DbId,
    transaction_id: Uuid,
    payload: serde_json::Value,
}

impl BillingOutbox {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            health: BillingOutboxHealth::default(),
            worker: WorkerSlot::default(),
            retry_worker: WorkerSlot::default(),
            activity: ActivityRecorder::disabled(),
            daily: UsageDailyRecorder::disabled(),
        }
    }

    pub fn spawn(
        pool: PgPool,
        flush_interval: Duration,
        queue_size: usize,
        activity: ActivityRecorder,
        daily: UsageDailyRecorder,
        process_outbox: bool,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(queue_size.max(1));
        let (retry_sender, retry_receiver) = mpsc::channel(queue_size.max(1));
        let outbox = Self {
            pool,
            health: BillingOutboxHealth::default(),
            worker: WorkerSlot::default(),
            retry_worker: WorkerSlot::default(),
            activity,
            daily,
        };
        let worker = outbox.clone();
        outbox.worker.set(
            sender,
            tokio::spawn(async move {
                worker.run_worker(receiver, flush_interval).await;
            }),
        );
        if process_outbox {
            for _ in 0..BILLING_PROCESS_WORKERS {
                let process_worker = outbox.clone();
                // 用监督壳包裹：worker panic 后记录 error 并自动重启，而非静默退出。
                tokio::spawn(async move {
                    loop {
                        let worker = process_worker.clone();
                        let result = tokio::spawn(async move {
                            worker.run_process_worker(flush_interval).await;
                        })
                        .await;
                        match result {
                            Ok(()) => break, // 正常关机（channel 关闭等），不重启
                            Err(err) if err.is_panic() => {
                                tracing::error!(
                                    "billing process worker panicked; restarting after delay: {err:?}"
                                );
                                time::sleep(Duration::from_millis(200)).await;
                            }
                            Err(err) => {
                                tracing::warn!("billing process worker exited: {err:?}");
                                break;
                            }
                        }
                    }
                });
            }
        }
        let retry_worker = outbox.clone();
        outbox.retry_worker.set(
            retry_sender,
            tokio::spawn(async move {
                retry_worker.run_retry_worker(retry_receiver).await;
            }),
        );
        outbox
    }

    pub fn write_status(&self) -> BillingOutboxWriteStatus {
        self.health.status()
    }

    pub async fn enqueue(&self, usage: &UsageInsert) -> AppResult<()> {
        if usage.billing.is_none() {
            return Ok(());
        }

        match persist_billing_usage_with_retry(&self.pool, usage).await {
            Ok(()) => {
                self.health.record_success();
                Ok(())
            }
            Err(err) => {
                self.health.record_failure();
                Err(err)
            }
        }
    }

    /// Atomically persists an async task's billing intent and transitions the
    /// task out of `held`. A crash can therefore leave either both operations
    /// committed or neither operation committed.
    pub async fn enqueue_task_durable(
        &self,
        task_id: DbId,
        usage: &UsageInsert,
    ) -> AppResult<bool> {
        let Some(billing) = &usage.billing else {
            return Ok(false);
        };
        let payload = serde_json::to_value(usage)?;
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            "INSERT INTO billing (transaction_id, payload)
             VALUES ($1, $2)
             ON CONFLICT (transaction_id) DO NOTHING
             RETURNING id",
        )
        .bind(billing.transaction_id)
        .bind(&payload)
        .fetch_optional(&mut *tx)
        .await?;
        if inserted.is_none() {
            let existing_payload: serde_json::Value = sqlx::query_scalar(
                "SELECT payload
                 FROM billing
                 WHERE transaction_id = $1
                 FOR UPDATE",
            )
            .bind(billing.transaction_id)
            .fetch_one(&mut *tx)
            .await?;
            if existing_payload != payload {
                tx.rollback().await?;
                return Err(AppError::Conflict(
                    "billing transaction payload does not match the existing record".to_string(),
                ));
            }
        }
        let updated = sqlx::query(
            "UPDATE task_upstream
             SET billing_status = 'settled', updated_at = now()
             WHERE id = $1 AND billing_status = 'held'
             RETURNING billing_hold",
        )
        .bind(task_id)
        .fetch_optional(&mut *tx)
        .await?;
        super::hold::complete(&mut tx, billing.transaction_id).await?;
        if let Some(row) = updated {
            let hold_value: Option<serde_json::Value> = row.try_get("billing_hold")?;
            let hold = hold_value
                .map(serde_json::from_value::<crate::billing::DebitHold>)
                .transpose()?
                .ok_or_else(|| {
                    AppError::Conflict("async task billing hold is missing".to_string())
                })?;
            if hold.transaction_id != billing.transaction_id {
                tx.rollback().await?;
                return Err(AppError::Conflict(
                    "async task hold transaction does not match billing transaction".to_string(),
                ));
            }
        } else {
            let status: Option<String> = sqlx::query_scalar(
                "SELECT billing_status
                 FROM task_upstream
                 WHERE id = $1
                 FOR UPDATE",
            )
            .bind(task_id)
            .fetch_optional(&mut *tx)
            .await?;
            if status.as_deref() == Some("settled") {
                tx.commit().await?;
                self.health.record_success();
                return Ok(false);
            }
            tx.rollback().await?;
            return Err(match status {
                Some(status) => AppError::Conflict(format!(
                    "async task billing status is {status}; expected held or settled"
                )),
                None => AppError::NotFound,
            });
        }
        tx.commit().await?;
        self.health.record_success();
        Ok(true)
    }

    async fn enqueue_retry(&self, usage: UsageInsert) -> AppResult<()> {
        if usage.billing.is_none() {
            return Ok(());
        }

        let sender = self.retry_worker.sender();
        let Some(sender) = sender else {
            self.health.record_failure();
            return Err(AppError::UpstreamUnavailable(
                "billing outbox retry queue is unavailable".to_string(),
            ));
        };
        sender.send(usage).await.map_err(|_| {
            self.health.record_failure();
            AppError::UpstreamUnavailable("billing outbox retry queue is closed".to_string())
        })
    }

    pub async fn enqueue_or_retry(&self, usage: UsageInsert) -> AppResult<()> {
        if usage.billing.is_none() {
            return Ok(());
        }

        let sender = self.worker.sender();
        let Some(sender) = sender else {
            return self.enqueue_retry(usage).await;
        };
        sender.send(usage).await.map_err(|_| {
            self.health.record_failure();
            AppError::UpstreamUnavailable("billing outbox queue is closed".to_string())
        })
    }

    pub async fn flush_pending(&self, timeout: Duration, process_outbox: bool) {
        let pool = self.pool.clone();
        let activity = self.activity.clone();
        let daily = self.daily.clone();
        let worker = self.worker.clone();
        let retry_worker = self.retry_worker.clone();
        match time::timeout(timeout, async move {
            worker.close_and_wait().await;
            retry_worker.close_and_wait().await;
            if process_outbox {
                drain_billing_outbox(&pool, &activity, &daily).await?;
            }
            Ok::<(), AppError>(())
        })
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(err)) => tracing::warn!("failed to flush billing outbox on shutdown: {err}"),
            Err(_) => tracing::warn!(
                timeout_ms = timeout.as_millis() as i64,
                "timed out flushing billing outbox on shutdown"
            ),
        }
    }

    async fn run_worker(self, mut receiver: Receiver<UsageInsert>, flush_interval: Duration) {
        let mut interval = time::interval(flush_interval.max(Duration::from_millis(1)));
        loop {
            tokio::select! {
                usage = receiver.recv() => {
                    let Some(usage) = usage else {
                        break;
                    };
                    let batch = drain_batch_from_receiver(&mut receiver, usage, BILLING_BATCH_SIZE as usize);

                    match persist_billing_usages_with_retry(&self.pool, &batch).await {
                        Ok(()) => self.health.record_success(),
                        Err(err) => {
                            self.health.record_failure();
                            tracing::error!(
                                count = batch.len(),
                                "failed to persist relay billing usage batch from queue; queueing bounded background retry: {err}"
                            );
                            for usage in batch {
                                if let Err(err) = self.enqueue_retry(usage).await {
                                    tracing::error!("failed to enqueue durable billing retry: {err}");
                                }
                            }
                        }
                    }
                }
                _ = interval.tick() => {}
            }
        }
    }

    async fn run_process_worker(self, flush_interval: Duration) {
        let mut interval = time::interval(flush_interval.max(Duration::from_millis(1)));
        loop {
            interval.tick().await;
            if let Err(err) = super::receivable::reconcile(&self.pool, 32).await {
                tracing::warn!("failed to collect late billing receivables: {err}");
            }
            for _ in 0..BILLING_MAX_BATCHES_PER_TICK {
                match process_billing_outbox_batch(
                    &self.pool,
                    &self.activity,
                    &self.daily,
                    BILLING_BATCH_SIZE,
                )
                .await
                {
                    Ok(processed) if processed >= BILLING_BATCH_SIZE as u64 => {}
                    Ok(_) => break,
                    Err(err) => {
                        tracing::warn!("failed to process durable billing records: {err}");
                        break;
                    }
                }
            }
        }
    }

    async fn run_retry_worker(self, mut receiver: mpsc::Receiver<UsageInsert>) {
        while let Some(usage) = receiver.recv().await {
            let batch =
                drain_batch_from_receiver(&mut receiver, usage, BILLING_BATCH_SIZE as usize);

            let mut delay = BILLING_OUTBOX_BACKGROUND_RETRY_INITIAL_DELAY;
            let mut attempt = 0u32;
            loop {
                match persist_billing_usages(&self.pool, &batch).await {
                    Ok(()) => {
                        self.health.record_success();
                        break;
                    }
                    Err(err) => {
                        self.health.record_failure();
                        attempt += 1;
                        if attempt >= BILLING_RETRY_WORKER_MAX_ATTEMPTS {
                            // 超过最大重试次数后逐条写入 billing outbox 表，由
                            // process worker 再次处理，避免损坏记录永久阻塞整个 batch
                            tracing::error!(
                                count = batch.len(),
                                attempt,
                                "billing retry batch exceeded max attempts; falling back to per-record persist: {err}"
                            );
                            for item in &batch {
                                if let Err(fb_err) =
                                    persist_billing_usage_with_retry(&self.pool, item).await
                                {
                                    tracing::error!(
                                        "failed to fall back per-record billing persist: {fb_err}"
                                    );
                                }
                            }
                            break;
                        }
                        tracing::error!(
                            count = batch.len(),
                            attempt,
                            max_attempts = BILLING_RETRY_WORKER_MAX_ATTEMPTS,
                            "failed to persist durable billing batch in bounded background retry: {err}"
                        );
                        time::sleep(delay).await;
                        delay = (delay * 2).min(BILLING_OUTBOX_BACKGROUND_RETRY_MAX_DELAY);
                    }
                }
            }
        }
    }
}

impl WorkerSlot {
    fn set(&self, sender: mpsc::Sender<UsageInsert>, handle: JoinHandle<()>) {
        *self.sender.write().unwrap_or_else(|e| e.into_inner()) = Some(sender);
        *self.handle.write().unwrap_or_else(|e| e.into_inner()) = Some(handle);
    }

    fn sender(&self) -> Option<mpsc::Sender<UsageInsert>> {
        self.sender
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    async fn close_and_wait(&self) {
        self.sender
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let handle = self
            .handle
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(handle) = handle {
            let _ = handle.await;
        }
    }
}

impl BillingOutboxHealth {
    fn record_success(&self) {
        *self.failed_since.write().unwrap_or_else(|e| e.into_inner()) = None;
    }

    fn record_failure(&self) {
        let mut failed_since = self.failed_since.write().unwrap_or_else(|e| e.into_inner());
        if failed_since.is_none() {
            *failed_since = Some(Utc::now());
        }
    }

    fn status(&self) -> BillingOutboxWriteStatus {
        let failed_since = *self.failed_since.read().unwrap_or_else(|e| e.into_inner());
        BillingOutboxWriteStatus {
            healthy: failed_since.is_none(),
            failed_since,
        }
    }
}

pub async fn backlog_status(
    pool: &PgPool,
    max_pending: i64,
) -> AppResult<BillingOutboxBacklogStatus> {
    let sample_limit = max_pending.saturating_add(1).max(1);
    let row = sqlx::query(
        // Only `pending` rows gate readiness — the drain worker retries them, so
        // an aging pending backlog signals "the drain can't keep up". `failed`
        // rows are dead-lettered (never retried) and are counted separately for
        // observability; they must not block startup on stale data.
        "WITH pending AS (
             SELECT created_at
             FROM billing
             WHERE status = 'pending'
             ORDER BY created_at ASC
             LIMIT $1
         )
         SELECT COUNT(*)::BIGINT AS pending_count,
                COALESCE(EXTRACT(EPOCH FROM (now() - MIN(created_at)))::BIGINT, 0)
                    AS oldest_pending_age_seconds,
                (SELECT COUNT(*) FROM (
                    SELECT 1 FROM billing WHERE status = 'failed' LIMIT $1
                ) f)::BIGINT AS failed_count
         FROM pending",
    )
    .bind(sample_limit)
    .fetch_one(pool)
    .await?;

    Ok(BillingOutboxBacklogStatus {
        pending_count: row.try_get("pending_count")?,
        oldest_pending_age_seconds: row.try_get("oldest_pending_age_seconds")?,
        failed_count: row.try_get("failed_count")?,
    })
}

async fn persist_billing_usage_with_retry(pool: &PgPool, usage: &UsageInsert) -> AppResult<()> {
    let mut delay = BILLING_OUTBOX_INITIAL_RETRY_DELAY;
    for attempt in 1..=BILLING_OUTBOX_WRITE_ATTEMPTS {
        match persist_billing_usage(pool, usage).await {
            Ok(()) => return Ok(()),
            Err(err) if attempt == BILLING_OUTBOX_WRITE_ATTEMPTS => return Err(err),
            Err(err) => {
                tracing::warn!(
                    attempt,
                    "failed to persist durable billing record; retrying: {err}"
                );
                time::sleep(delay).await;
                delay *= 2;
            }
        }
    }
    Ok(())
}

async fn persist_billing_usages_with_retry(pool: &PgPool, usages: &[UsageInsert]) -> AppResult<()> {
    let mut delay = BILLING_OUTBOX_INITIAL_RETRY_DELAY;
    for attempt in 1..=BILLING_OUTBOX_WRITE_ATTEMPTS {
        match persist_billing_usages(pool, usages).await {
            Ok(()) => return Ok(()),
            Err(err) if attempt == BILLING_OUTBOX_WRITE_ATTEMPTS => return Err(err),
            Err(err) => {
                tracing::warn!(
                    attempt,
                    count = usages.len(),
                    "failed to persist durable billing record batch; retrying: {err}"
                );
                time::sleep(delay).await;
                delay *= 2;
            }
        }
    }
    Ok(())
}

/// run_worker / run_retry_worker 共用的批量收集辅助：先放入第一条，再尽量从 channel 排空，
/// 上限 `max_size`。消除两个函数原来的重复 while + try_recv 逻辑。
fn drain_batch_from_receiver<T>(
    receiver: &mut mpsc::Receiver<T>,
    first: T,
    max_size: usize,
) -> Vec<T> {
    let mut batch = vec![first];
    while batch.len() < max_size {
        match receiver.try_recv() {
            Ok(item) => batch.push(item),
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
        }
    }
    batch
}

async fn persist_billing_usage(pool: &PgPool, usage: &UsageInsert) -> AppResult<()> {
    let Some(billing) = &usage.billing else {
        return Ok(());
    };
    let payload = serde_json::to_value(usage)?;
    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO billing
         (transaction_id, payload)
         VALUES ($1, $2)
         ON CONFLICT (transaction_id) DO NOTHING",
    )
    .bind(billing.transaction_id)
    .bind(payload)
    .execute(&mut *tx)
    .await?;
    super::hold::complete(&mut tx, billing.transaction_id).await?;
    tx.commit().await?;
    Ok(())
}

async fn persist_billing_usages(pool: &PgPool, usages: &[UsageInsert]) -> AppResult<()> {
    let mut rows = Vec::with_capacity(usages.len());
    for usage in usages {
        let Some(billing) = &usage.billing else {
            continue;
        };
        rows.push((billing.transaction_id, serde_json::to_value(usage)?));
    }
    if rows.is_empty() {
        return Ok(());
    }
    let transaction_ids = rows.iter().map(|(id, _)| *id).collect::<Vec<_>>();
    let mut tx = pool.begin().await?;

    let mut query_builder = sqlx::QueryBuilder::new(
        "INSERT INTO billing
         (transaction_id, payload) ",
    );
    query_builder.push_values(rows, |mut row, (transaction_id, payload)| {
        row.push_bind(transaction_id).push_bind(payload);
    });
    query_builder.push(" ON CONFLICT (transaction_id) DO NOTHING");
    query_builder.build().execute(&mut *tx).await?;
    sqlx::query("DELETE FROM billing_hold WHERE transaction_id = ANY($1)")
        .bind(transaction_ids)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

async fn process_billing_outbox_batch(
    pool: &PgPool,
    activity: &ActivityRecorder,
    daily: &UsageDailyRecorder,
    limit: i64,
) -> AppResult<u64> {
    let mut processed = 0;
    let mut remaining = limit.max(0);
    // 不再跨 chunk 累积 selected_ids：
    // - 成功处理的记录已被标记为 'processed'，被 WHERE status='pending' 自然过滤；
    // - FOR UPDATE SKIP LOCKED 防止并发 worker 重复获取；
    // - 失败记录的 attempts 递增，ORDER BY attempts ASC 让它们排在后面。
    // 原来的累积方案在大批量时会产生最多 40×500=20000 条的 IN 子句，显著增加 PG 开销。
    while remaining > 0 {
        let chunk_limit = remaining.min(BILLING_PROCESS_CHUNK_SIZE);
        let result = process_billing_outbox_chunk(pool, activity, daily, chunk_limit, &[]).await?;
        processed += result.processed;
        remaining -= result.selected as i64;
        if result.selected < chunk_limit as u64 {
            break;
        }
    }
    Ok(processed)
}

async fn drain_billing_outbox(
    pool: &PgPool,
    activity: &ActivityRecorder,
    daily: &UsageDailyRecorder,
) -> AppResult<u64> {
    let mut total = 0;
    loop {
        let processed =
            process_billing_outbox_batch(pool, activity, daily, BILLING_BATCH_SIZE).await?;
        total += processed;
        if processed < BILLING_BATCH_SIZE as u64 {
            break;
        }
    }
    Ok(total)
}

struct BillingOutboxChunkResult {
    selected: u64,
    processed: u64,
}

async fn process_billing_outbox_chunk(
    pool: &PgPool,
    activity: &ActivityRecorder,
    daily: &UsageDailyRecorder,
    limit: i64,
    excluded_ids: &[DbId],
) -> AppResult<BillingOutboxChunkResult> {
    let mut tx = pool.begin().await?;
    let records = fetch_pending_billing_records(&mut tx, limit, excluded_ids).await?;
    if records.is_empty() {
        tx.commit().await?;
        return Ok(BillingOutboxChunkResult {
            selected: 0,
            processed: 0,
        });
    }

    let selected = records.len() as u64;
    let selected_ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
    let mut processed_usages = Vec::with_capacity(records.len());
    for record in &records {
        match usage_from_billing_payload(record.transaction_id, record.payload.clone()) {
            Ok(usage) => processed_usages.push(usage),
            Err(err) => {
                let failed_id = record.id;
                if let Err(rb_err) = tx.rollback().await {
                    tracing::warn!(billing_id = %failed_id, "failed to rollback billing transaction: {rb_err}");
                }
                tracing::warn!(
                    billing_id = %failed_id,
                    selected,
                    "failed to decode billing chunk; retrying selected records individually: {err}"
                );
                let processed =
                    process_billing_records_individually(pool, activity, daily, records).await?;
                return Ok(BillingOutboxChunkResult {
                    selected,
                    processed,
                });
            }
        }
    }

    if let Err(err) = async {
        usage::flush_usage(&mut tx, &processed_usages).await?;
        mark_billing_records_processed(&mut tx, &selected_ids).await?;
        Ok::<(), AppError>(())
    }
    .await
    {
        if let Err(rb_err) = tx.rollback().await {
            tracing::warn!(selected, "failed to rollback billing transaction: {rb_err}");
        }
        tracing::warn!(
            selected,
            "failed to process billing chunk; retrying selected records individually: {err}"
        );
        let processed =
            process_billing_records_individually(pool, activity, daily, records).await?;
        return Ok(BillingOutboxChunkResult {
            selected,
            processed,
        });
    }

    tx.commit().await?;
    activity.record(&processed_usages);
    daily.record(&processed_usages);
    Ok(BillingOutboxChunkResult {
        selected,
        processed: processed_usages.len() as u64,
    })
}

async fn fetch_pending_billing_records(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    limit: i64,
    excluded_ids: &[DbId],
) -> AppResult<Vec<PendingBillingRecord>> {
    let rows = sqlx::query(
        "SELECT id, transaction_id, payload
         FROM billing
         WHERE status = 'pending'
           AND NOT (id = ANY($2::BIGINT[]))
         ORDER BY attempts ASC, created_at ASC
         LIMIT $1
         FOR UPDATE SKIP LOCKED",
    )
    .bind(limit)
    .bind(excluded_ids)
    .fetch_all(&mut **tx)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(PendingBillingRecord {
                id: row.try_get("id")?,
                transaction_id: row.try_get("transaction_id")?,
                payload: row.try_get("payload")?,
            })
        })
        .collect()
}

async fn process_billing_records_individually(
    pool: &PgPool,
    activity: &ActivityRecorder,
    daily: &UsageDailyRecorder,
    records: Vec<PendingBillingRecord>,
) -> AppResult<u64> {
    let mut processed = 0;
    let mut processed_usages = Vec::new();
    for record in records {
        if let Some(usage) = process_billing_outbox_record(pool, record.id).await? {
            processed += 1;
            processed_usages.push(usage);
        }
    }
    activity.record(&processed_usages);
    daily.record(&processed_usages);
    Ok(processed)
}

async fn process_billing_outbox_record(pool: &PgPool, id: DbId) -> AppResult<Option<UsageInsert>> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        "SELECT id, transaction_id, payload
         FROM billing
         WHERE id = $1 AND status = 'pending'
         FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };
    let record = PendingBillingRecord {
        id: row.try_get("id")?,
        transaction_id: row.try_get("transaction_id")?,
        payload: row.try_get("payload")?,
    };

    match process_billing_payload(&mut tx, record.id, record.transaction_id, record.payload).await {
        Ok(usage) => {
            tx.commit().await?;
            Ok(Some(usage))
        }
        Err(err) => {
            let _ = tx.rollback().await;
            tracing::warn!(billing_id = %record.id, "failed to process billing record: {err}");
            if let Err(record_err) = record_billing_failure(pool, record.id, &err).await {
                tracing::warn!(
                    billing_id = %record.id,
                    "failed to record billing processing failure: {record_err}"
                );
                return Err(record_err);
            }
            Ok(None)
        }
    }
}

fn usage_from_billing_payload(
    transaction_id: Uuid,
    payload: serde_json::Value,
) -> AppResult<UsageInsert> {
    let usage: UsageInsert = serde_json::from_value(payload)?;
    let billing = usage
        .billing
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("billing payload is missing charge".to_string()))?;
    if billing.transaction_id != transaction_id {
        return Err(AppError::BadRequest(
            "billing payload transaction id does not match row transaction id".to_string(),
        ));
    }

    Ok(usage)
}

async fn mark_billing_records_processed(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ids: &[DbId],
) -> AppResult<()> {
    if ids.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "UPDATE billing
         SET status = 'processed',
             processed_at = now(),
             last_error = NULL
         WHERE id = ANY($1) AND status = 'pending'",
    )
    .bind(ids)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn process_billing_payload(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: DbId,
    transaction_id: Uuid,
    payload: serde_json::Value,
) -> AppResult<UsageInsert> {
    let usage = usage_from_billing_payload(transaction_id, payload)?;
    usage::flush_usage(tx, std::slice::from_ref(&usage)).await?;

    sqlx::query(
        "UPDATE billing
         SET status = 'processed',
             processed_at = now(),
             last_error = NULL
         WHERE id = $1 AND status = 'pending'",
    )
    .bind(id)
    .execute(&mut **tx)
    .await?;
    Ok(usage)
}

async fn record_billing_failure(pool: &PgPool, id: DbId, err: &AppError) -> AppResult<()> {
    sqlx::query(
        "UPDATE billing
         SET attempts = attempts + 1,
             status = CASE
                 WHEN attempts + 1 >= $2 THEN 'failed'
                 ELSE status
             END,
             last_error = $3
         WHERE id = $1 AND status IN ('pending', 'failed')",
    )
    .bind(id)
    .bind(BILLING_MAX_ATTEMPTS)
    .bind(err.to_string().chars().take(500).collect::<String>())
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::billing::{
        BillingCharge, BillingChargeStatus, BillingMeter, CreditAccountId, DebitPart,
    };

    fn usage_with_billing(id: DbId) -> UsageInsert {
        UsageInsert {
            user_id: id,
            project_id: id,
            user_key_id: id,
            channel_id: id,
            channel_key_id: None,
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
            latency_ms: 1,
            first_response_ms: None,
            output_tokens_per_second: None,
            error_summary: None,
            token_usage: None,
            billing_meter: BillingMeter::Token,
            billable_units: 0,
            billing: Some(BillingCharge {
                transaction_id: Uuid::new_v4(),
                input_tokens: None,
                output_tokens: None,
                total_tokens: None,
                billing_meter: BillingMeter::Token,
                billable_units: 0,
                cost_micros: 0,
                status: BillingChargeStatus::Billed,
                parts: Vec::new(),
                returned_parts: Vec::new(),
            }),
        }
    }

    #[tokio::test]
    async fn new_outbox_without_workers_reports_unavailable_queue() {
        let pool =
            PgPool::connect_lazy("postgres://zhiqiaoapi:zhiqiaoapi@localhost/zhiqiaoapi").unwrap();
        let outbox = BillingOutbox::new(pool);
        assert!(outbox
            .enqueue_or_retry(usage_with_billing(1))
            .await
            .is_err());

        assert!(outbox
            .worker
            .sender
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .is_none());
    }

    async fn late_usage_fixture(
        pool: &PgPool,
        balance: i64,
        reserved: i64,
    ) -> (UsageInsert, DbId, DbId) {
        let account_id:DbId=sqlx::query_scalar(
            "INSERT INTO credit_account(owner_type,owner_id,balance_micros,reserved_micros) VALUES ('project',99002,$1,$2) RETURNING id",
        ).bind(balance).bind(reserved).fetch_one(pool).await.unwrap();
        let allocation_id:DbId=sqlx::query_scalar(
            "INSERT INTO credit_allocation(credit_account_id,amount_micros,returned_micros,status) VALUES ($1,100,100,'recovered') RETURNING id",
        ).bind(account_id).fetch_one(pool).await.unwrap();
        let mut usage = usage_with_billing(99002);
        let charge = usage.billing.as_mut().unwrap();
        charge.cost_micros = 35;
        charge.parts.push(DebitPart {
            credit_account: CreditAccountId::new(account_id),
            allocation_id,
            amount_micros: 35,
        });
        charge.returned_parts.push(DebitPart {
            credit_account: CreditAccountId::new(account_id),
            allocation_id,
            amount_micros: 65,
        });
        (usage, account_id, allocation_id)
    }

    async fn persist_and_process(pool: &PgPool, usage: &UsageInsert) -> DbId {
        persist_billing_usage(pool, usage).await.unwrap();
        let id: DbId = sqlx::query_scalar("SELECT id FROM billing WHERE transaction_id=$1")
            .bind(usage.billing.as_ref().unwrap().transaction_id)
            .fetch_one(pool)
            .await
            .unwrap();
        assert!(process_billing_outbox_record(pool, id)
            .await
            .unwrap()
            .is_some());
        id
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn durable_outbox_handoff_replaces_live_hold_without_recovery_gap(pool: PgPool) {
        let (usage, account_id, allocation_id) = late_usage_fixture(&pool, 1000, 100).await;
        sqlx::query("UPDATE credit_allocation SET status='active',returned_micros=0,created_at=now()-interval '1 hour' WHERE id=$1")
            .bind(allocation_id).execute(&pool).await.unwrap();
        let billing = super::super::Billing::new_memory(Duration::from_secs(60), 32, 100, 100);
        let candidates = super::super::fetch_stale_allocations(
            &pool,
            Utc::now() - chrono::Duration::seconds(900),
        )
        .await
        .unwrap();
        assert_eq!(candidates.len(), 1);
        let mut hold = super::super::DebitHold::new(
            100,
            vec![DebitPart {
                credit_account: CreditAccountId::new(account_id),
                allocation_id,
                amount_micros: 100,
            }],
            true,
        );
        hold.transaction_id = usage.billing.as_ref().unwrap().transaction_id;
        assert!(super::super::hold::register(&pool, &mut hold)
            .await
            .unwrap()
            .is_empty());
        persist_billing_usage(&pool, &usage).await.unwrap();
        let hold_count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM billing_hold WHERE transaction_id=$1")
                .bind(hold.transaction_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(hold_count, 0);
        let recovered =
            super::super::recover_allocations_in_db(&pool, &candidates, billing.hot.as_ref())
                .await
                .unwrap();
        assert_eq!(
            recovered.count, 0,
            "a pending durable billing record protects the old allocation"
        );
        let id: DbId = sqlx::query_scalar("SELECT id FROM billing WHERE transaction_id=$1")
            .bind(hold.transaction_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(process_billing_outbox_record(&pool, id)
            .await
            .unwrap()
            .is_some());
        let state: (i64, i64) =
            sqlx::query_as("SELECT balance_micros,reserved_micros FROM credit_account WHERE id=$1")
                .bind(account_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(state, (965, 0));
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn recovered_allocation_late_usage_is_debited_exactly_once(pool: PgPool) {
        let (usage, account_id, allocation_id) = late_usage_fixture(&pool, 1000, 0).await;
        let id = persist_and_process(&pool, &usage).await;
        assert!(process_billing_outbox_record(&pool, id)
            .await
            .unwrap()
            .is_none());
        let state:(i64,i64,i64,i64)=sqlx::query_as(
            "SELECT w.balance_micros,w.reserved_micros,a.consumed_micros,a.returned_micros FROM credit_account w JOIN credit_allocation a ON a.credit_account_id=w.id WHERE a.id=$1",
        ).bind(allocation_id).fetch_one(&pool).await.unwrap();
        assert_eq!(state, (965, 0, 35, 65));
        let ledger:(i64,i64)=sqlx::query_as("SELECT count(*),COALESCE(sum(amount_micros),0)::BIGINT FROM credit_ledger WHERE credit_account_id=$1 AND reason='usage'")
            .bind(account_id).fetch_one(&pool).await.unwrap();
        assert_eq!(ledger, (1, -35));
        let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM billing_receivable")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(pending, 0);
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn late_usage_retains_receivable_and_collects_after_topup_without_double_debit(
        pool: PgPool,
    ) {
        let (usage, account_id, allocation_id) = late_usage_fixture(&pool, 0, 0).await;
        persist_and_process(&pool, &usage).await;
        let status: String =
            sqlx::query_scalar("SELECT billing_status FROM usage WHERE billing_transaction_id=$1")
                .bind(usage.billing.as_ref().unwrap().transaction_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "payment_pending");
        let owed: i64 = sqlx::query_scalar(
            "SELECT amount_micros FROM billing_receivable WHERE settled_at IS NULL",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(owed, 35);
        // A top-up must not spend money reserved by another active request.
        sqlx::query("UPDATE credit_account SET balance_micros=100,reserved_micros=80 WHERE id=$1")
            .bind(account_id)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            super::super::receivable::reconcile(&pool, 10)
                .await
                .unwrap(),
            0
        );
        sqlx::query("UPDATE credit_account SET reserved_micros=0 WHERE id=$1")
            .bind(account_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE billing_receivable SET retry_after=now()")
            .execute(&pool)
            .await
            .unwrap();
        let (a, b) = tokio::join!(
            super::super::receivable::reconcile(&pool, 10),
            super::super::receivable::reconcile(&pool, 10)
        );
        assert_eq!(a.unwrap() + b.unwrap(), 1);
        assert_eq!(
            super::super::receivable::reconcile(&pool, 10)
                .await
                .unwrap(),
            0
        );
        let state:(i64,i64,i64,i64)=sqlx::query_as(
            "SELECT w.balance_micros,w.reserved_micros,a.consumed_micros,a.returned_micros FROM credit_account w JOIN credit_allocation a ON a.credit_account_id=w.id WHERE a.id=$1",
        ).bind(allocation_id).fetch_one(&pool).await.unwrap();
        assert_eq!(state, (65, 0, 35, 65));
        let final_status: String =
            sqlx::query_scalar("SELECT billing_status FROM usage WHERE billing_transaction_id=$1")
                .bind(usage.billing.as_ref().unwrap().transaction_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(final_status, "billed");
        let ledger:(i64,i64)=sqlx::query_as("SELECT count(*),sum(amount_micros)::BIGINT FROM credit_ledger WHERE credit_account_id=$1 AND reason='usage'")
            .bind(account_id).fetch_one(&pool).await.unwrap();
        assert_eq!(ledger, (1, -35));
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn unpaid_payer_blocks_new_holds_and_frees_idle_prefetch_for_collection(pool: PgPool) {
        let (usage, account_id, _) = late_usage_fixture(&pool, 100, 100).await;
        persist_and_process(&pool, &usage).await;
        let idle_allocation:DbId=sqlx::query_scalar(
            "INSERT INTO credit_allocation(credit_account_id,amount_micros) VALUES ($1,100) RETURNING id",
        ).bind(account_id).fetch_one(&pool).await.unwrap();
        let billing = super::super::Billing::new_memory(Duration::from_secs(60), 32, 100, 100);
        let account = CreditAccountId::new(account_id);
        billing
            .hot
            .credit_allocation(account.clone(), idle_allocation, 100)
            .await
            .unwrap();
        let result = billing
            .reserve(
                &pool,
                super::super::BillingAccounts {
                    user_id: 1,
                    project_id: 1,
                    user_key_id: 1,
                    user_key_model_credit_account: None,
                    user_key_credit_account: &account,
                    project_credit_account: &account,
                },
                20,
            )
            .await;
        assert!(matches!(
            result,
            Err(AppError::ConflictWithCode {
                code: "payment_pending",
                ..
            })
        ));
        let reserved: i64 =
            sqlx::query_scalar("SELECT reserved_micros FROM credit_account WHERE id=$1")
                .bind(account_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(reserved, 0);
        assert_eq!(billing.hot.available_micros(&[account]).await.unwrap(), 0);
        assert_eq!(
            super::super::receivable::reconcile(&pool, 10)
                .await
                .unwrap(),
            1
        );
        let balance: i64 =
            sqlx::query_scalar("SELECT balance_micros FROM credit_account WHERE id=$1")
                .bind(account_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(balance, 65);
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn deleted_zero_balance_model_debt_blocks_new_key_and_funded_project(pool: PgPool) {
        sqlx::raw_sql(
            r#"
            INSERT INTO "user" (id,email,user_group_id)
              SELECT 99002,'owner-debt@example.test',id FROM user_group WHERE is_default;
            INSERT INTO project (id,name,owner_user_id,is_default) VALUES (99002,'Owner debt',99002,TRUE);
            INSERT INTO user_key (id,user_id,project_id,key_prefix,secret_ciphertext)
              VALUES (99002,99002,99002,'old','unused');
            INSERT INTO user_key_model (id,user_key_id,model) VALUES (99002,99002,'gpt-4.1');
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        let (usage, indebted_account, _) = late_usage_fixture(&pool, 0, 0).await;
        sqlx::query(
            "UPDATE credit_account SET owner_type='user_key_model',owner_id=99002 WHERE id=$1",
        )
        .bind(indebted_account)
        .execute(&pool)
        .await
        .unwrap();
        persist_and_process(&pool, &usage).await;
        sqlx::query("DELETE FROM user_key WHERE id=99002")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql(
            "INSERT INTO user_key (id,user_id,project_id,key_prefix,secret_ciphertext)
               VALUES (99003,99002,99002,'new','unused');
             INSERT INTO credit_account (owner_type,owner_id,balance_micros)
               VALUES ('user_key',99003,0),('project',99002,1000);",
        )
        .execute(&pool)
        .await
        .unwrap();
        let key = CreditAccountId::new(
            sqlx::query_scalar(
                "SELECT id FROM credit_account WHERE owner_type='user_key' AND owner_id=99003",
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
        );
        let project = CreditAccountId::new(
            sqlx::query_scalar(
                "SELECT id FROM credit_account WHERE owner_type='project' AND owner_id=99002",
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
        );
        let billing = super::super::Billing::new_memory(Duration::from_secs(60), 32, 100, 100);
        let idle_project_allocation: DbId = sqlx::query_scalar(
            "INSERT INTO credit_allocation (credit_account_id,amount_micros) VALUES ($1,1000) RETURNING id",
        )
        .bind(project.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query("UPDATE credit_account SET reserved_micros=1000 WHERE id=$1")
            .bind(project.id)
            .execute(&pool)
            .await
            .unwrap();
        billing
            .hot
            .credit_allocation(project.clone(), idle_project_allocation, 1000)
            .await
            .unwrap();
        let accounts = super::super::BillingAccounts {
            user_id: 99002,
            project_id: 99002,
            user_key_id: 99003,
            user_key_model_credit_account: None,
            user_key_credit_account: &key,
            project_credit_account: &project,
        };
        for estimate in [20, 0] {
            assert!(matches!(
                billing.reserve(&pool, accounts, estimate).await,
                Err(AppError::ConflictWithCode {
                    code: "payment_pending",
                    ..
                })
            ));
        }
        let states: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT balance_micros,reserved_micros FROM credit_account WHERE id=ANY($1) ORDER BY balance_micros",
        ).bind(vec![key.id,project.id]).fetch_all(&pool).await.unwrap();
        assert_eq!(states, vec![(0, 0), (1000, 0)]);
        let allocation_count: i64 = sqlx::query_scalar("SELECT count(*) FROM credit_allocation")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            allocation_count, 2,
            "debt rejection must happen before prefetch"
        );
        assert_eq!(
            billing
                .hot
                .available_micros(std::slice::from_ref(&project))
                .await
                .unwrap(),
            0
        );
        let owner: Option<DbId> =
            sqlx::query_scalar("SELECT owner_user_id FROM credit_account WHERE id=$1")
                .bind(indebted_account)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(owner, Some(99002));
        let mut topup = pool.begin().await.unwrap();
        super::super::account::adjust_balance(&mut topup, &project, 100)
            .await
            .unwrap();
        topup.commit().await.unwrap();
        assert_eq!(
            super::super::receivable::reconcile(&pool, 10)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            super::super::receivable::reconcile(&pool, 10)
                .await
                .unwrap(),
            0
        );
        let repaid: (i64, i64) = sqlx::query_as(
            "SELECT count(*),sum(amount_micros)::BIGINT FROM credit_ledger WHERE reason='usage' AND credit_account_id=$1",
        ).bind(project.id).fetch_one(&pool).await.unwrap();
        assert_eq!(repaid, (1, -35));
        let recovered_status: String =
            sqlx::query_scalar("SELECT billing_status FROM usage WHERE billing_transaction_id=$1")
                .bind(usage.billing.as_ref().unwrap().transaction_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(recovered_status, "billed");
        let hold = billing.reserve(&pool, accounts, 20).await.unwrap();
        assert_eq!(hold.estimated_micros, 20);
        billing.release_hold(&pool, hold).await.unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn model_debt_uses_original_then_own_project_without_spending_reserved_credit(
        pool: PgPool,
    ) {
        sqlx::raw_sql(
            r#"
            INSERT INTO "user" (id,email,user_group_id)
              SELECT 99002,'payer-owner@example.test',id FROM user_group WHERE is_default;
            INSERT INTO "user" (id,email,user_group_id)
              SELECT 99004,'other-owner@example.test',id FROM user_group WHERE is_default;
            INSERT INTO project (id,name,owner_user_id,is_default)
              VALUES (99002,'Owner',99002,TRUE),(99004,'Other',99004,TRUE);
            INSERT INTO user_key (id,user_id,project_id,key_prefix,secret_ciphertext)
              VALUES (99002,99002,99002,'old','unused');
            INSERT INTO user_key_model (id,user_key_id,model) VALUES (99002,99002,'gpt-4.1');
            INSERT INTO credit_account (owner_type,owner_id,balance_micros,reserved_micros)
              VALUES ('project',99004,1000,0);
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        let (usage, original_id, allocation_id) = late_usage_fixture(&pool, 10, 0).await;
        sqlx::query(
            "UPDATE credit_account SET owner_type='user_key_model',owner_id=99002 WHERE id=$1",
        )
        .bind(original_id)
        .execute(&pool)
        .await
        .unwrap();
        let project_id: DbId = sqlx::query_scalar(
            "INSERT INTO credit_account (owner_type,owner_id,balance_micros,reserved_micros)
             VALUES ('project',99002,100,90) RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        persist_and_process(&pool, &usage).await;
        let pending: i64 =
            sqlx::query_scalar("SELECT count(*) FROM billing_receivable WHERE settled_at IS NULL")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            pending, 1,
            "another user's wallet and reserved credit cannot repay this debt"
        );
        let mut topup = pool.begin().await.unwrap();
        super::super::account::adjust_balance(&mut topup, &CreditAccountId::new(project_id), 20)
            .await
            .unwrap();
        topup.commit().await.unwrap();
        assert_eq!(
            super::super::receivable::reconcile(&pool, 10)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            super::super::receivable::reconcile(&pool, 10)
                .await
                .unwrap(),
            0
        );
        let original: i64 =
            sqlx::query_scalar("SELECT balance_micros FROM credit_account WHERE id=$1")
                .bind(original_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(original, 0);
        let project: (i64, i64) =
            sqlx::query_as("SELECT balance_micros,reserved_micros FROM credit_account WHERE id=$1")
                .bind(project_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(project, (95, 90));
        let other: i64 = sqlx::query_scalar("SELECT balance_micros FROM credit_account WHERE owner_type='project' AND owner_id=99004")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(other, 1000);
        let charges: Vec<(DbId,i64)> = sqlx::query_as(
            "SELECT credit_account_id,amount_micros FROM credit_ledger WHERE allocation_id=$1 AND reason='usage' ORDER BY amount_micros DESC",
        ).bind(allocation_id).fetch_all(&pool).await.unwrap();
        assert_eq!(charges, vec![(original_id, -10), (project_id, -25)]);
        let traced: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM credit_ledger WHERE credit_account_id=$1 AND metadata->>'original_credit_account_id'=$2 AND metadata->>'repayment_source'='owner_default_project'",
        ).bind(project_id).bind(original_id.to_string()).fetch_one(&pool).await.unwrap();
        assert_eq!(traced, 1);
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn project_late_debt_does_not_spend_owner_personal_default_project(pool: PgPool) {
        sqlx::raw_sql(
            r#"
            INSERT INTO "user" (id,email,user_group_id)
              SELECT 99002,'project-debt-owner@example.test',id FROM user_group WHERE is_default;
            INSERT INTO project (id,name,owner_user_id,is_default)
              VALUES (99002,'Shared project',99002,FALSE),(99003,'Personal default',99002,TRUE);
            INSERT INTO credit_account (owner_type,owner_id,balance_micros)
              VALUES ('project',99003,1000);
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        let (usage, project_id, _) = late_usage_fixture(&pool, 0, 0).await;
        persist_and_process(&pool, &usage).await;
        let pending: i64 =
            sqlx::query_scalar("SELECT count(*) FROM billing_receivable WHERE settled_at IS NULL")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(pending, 1);
        let personal: i64 = sqlx::query_scalar(
            "SELECT balance_micros FROM credit_account WHERE owner_type='project' AND owner_id=99003",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(personal, 1000);
        assert_eq!(
            super::super::receivable::reconcile(&pool, 10)
                .await
                .unwrap(),
            0
        );
        let mut topup = pool.begin().await.unwrap();
        super::super::account::adjust_balance(&mut topup, &CreditAccountId::new(project_id), 35)
            .await
            .unwrap();
        sqlx::query("UPDATE billing_receivable SET retry_after=now()")
            .execute(&mut *topup)
            .await
            .unwrap();
        topup.commit().await.unwrap();
        assert_eq!(
            super::super::receivable::reconcile(&pool, 10)
                .await
                .unwrap(),
            1
        );
        let charges: Vec<(DbId, i64)> = sqlx::query_as(
            "SELECT credit_account_id,amount_micros FROM credit_ledger WHERE reason='usage'",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(charges, vec![(project_id, -35)]);
        let personal: i64 = sqlx::query_scalar(
            "SELECT balance_micros FROM credit_account WHERE owner_type='project' AND owner_id=99003",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(personal, 1000);
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn deleted_key_during_outbox_recording_preserves_usage_and_single_debit(pool: PgPool) {
        let user_id: DbId = sqlx::query_scalar(
            "INSERT INTO \"user\" (email, user_group_id)
             SELECT 'deleted-key-outbox@example.test', id FROM user_group WHERE is_default
             RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let project_id: DbId = sqlx::query_scalar(
            "INSERT INTO project (name, owner_user_id) VALUES ('Outbox race', $1) RETURNING id",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let user_key_id: DbId = sqlx::query_scalar(
            "INSERT INTO user_key (user_id, project_id, key_prefix, secret_ciphertext)
             VALUES ($1, $2, 'test', 'unused-test-ciphertext') RETURNING id",
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let user_key_model_id: DbId = sqlx::query_scalar(
            "INSERT INTO user_key_model (user_key_id, model) VALUES ($1, 'gpt-4.1') RETURNING id",
        )
        .bind(user_key_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let channel_id: DbId = sqlx::query_scalar(
            "INSERT INTO channel (provider, name) VALUES ('custom', 'Outbox race') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        // The accounting owner is deleted by the key's cascade. Its durable
        // account/allocation must still settle the completed request.
        let credit_account_id: DbId = sqlx::query_scalar(
            "INSERT INTO credit_account (owner_type, owner_id, balance_micros, reserved_micros)
             VALUES ('user_key_model', $1, 1000, 100) RETURNING id",
        )
        .bind(user_key_model_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let allocation_id: DbId = sqlx::query_scalar(
            "INSERT INTO credit_allocation (credit_account_id, amount_micros)
             VALUES ($1, 100) RETURNING id",
        )
        .bind(credit_account_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let mut usage = usage_with_billing(user_id);
        usage.project_id = project_id;
        usage.user_key_id = user_key_id;
        usage.channel_id = channel_id;
        let billing = usage.billing.as_mut().unwrap();
        billing.input_tokens = Some(10);
        billing.output_tokens = Some(20);
        billing.total_tokens = Some(30);
        billing.cost_micros = 100;
        billing.parts.push(DebitPart {
            credit_account: CreditAccountId::new(credit_account_id),
            allocation_id,
            amount_micros: 100,
        });
        let transaction_id = billing.transaction_id;
        persist_billing_usage(&pool, &usage).await.unwrap();
        let billing_id: DbId =
            sqlx::query_scalar("SELECT id FROM billing WHERE transaction_id = $1")
                .bind(transaction_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        let mut delete_tx = pool.begin().await.unwrap();
        let deleting_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *delete_tx)
            .await
            .unwrap();
        sqlx::query("DELETE FROM user_key WHERE id = $1")
            .bind(user_key_id)
            .execute(&mut *delete_tx)
            .await
            .unwrap();
        let processor_pool = pool.clone();
        let processing = tokio::spawn(async move {
            process_billing_outbox_record(&processor_pool, billing_id).await
        });
        // Observe the PostgreSQL wait, rather than relying on task scheduling
        // or assuming a sleep means the recorder has reached the deleted key.
        time::timeout(Duration::from_secs(5), async {
            loop {
                let blocked: bool = sqlx::query_scalar(
                    "SELECT EXISTS (
                         SELECT 1 FROM pg_stat_activity
                         WHERE datname = current_database()
                           AND $1 = ANY(pg_blocking_pids(pid))
                     )",
                )
                .bind(deleting_pid)
                .fetch_one(&pool)
                .await
                .unwrap();
                if blocked {
                    break;
                }
                assert!(
                    !processing.is_finished(),
                    "recorder bypassed the key delete lock"
                );
                time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("recorder should wait for the in-progress key deletion");
        assert!(!processing.is_finished());
        delete_tx.commit().await.unwrap();
        let processed = time::timeout(Duration::from_secs(5), processing)
            .await
            .expect("recording should finish after key deletion commits")
            .unwrap()
            .unwrap()
            .expect("the pending billing record should be processed");
        assert_eq!(processed.user_key_id, user_key_id);

        let recorded = sqlx::query(
            "SELECT id, user_id, project_id, user_key_id, channel_id, cost_micros,
                    input_tokens, output_tokens, total_tokens
             FROM usage WHERE billing_transaction_id = $1",
        )
        .bind(transaction_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let usage_id: DbId = recorded.try_get("id").unwrap();
        assert_eq!(
            recorded.try_get::<Option<DbId>, _>("user_id").unwrap(),
            Some(user_id)
        );
        assert_eq!(
            recorded.try_get::<Option<DbId>, _>("project_id").unwrap(),
            Some(project_id)
        );
        assert_eq!(
            recorded.try_get::<Option<DbId>, _>("user_key_id").unwrap(),
            None
        );
        assert_eq!(
            recorded.try_get::<Option<DbId>, _>("channel_id").unwrap(),
            Some(channel_id)
        );
        assert_eq!(
            recorded.try_get::<Option<i64>, _>("cost_micros").unwrap(),
            Some(100)
        );
        assert_eq!(
            recorded.try_get::<Option<i64>, _>("input_tokens").unwrap(),
            Some(10)
        );
        assert_eq!(
            recorded.try_get::<Option<i64>, _>("output_tokens").unwrap(),
            Some(20)
        );
        assert_eq!(
            recorded.try_get::<Option<i64>, _>("total_tokens").unwrap(),
            Some(30)
        );
        let deleted_model_count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM user_key_model WHERE id = $1")
                .bind(user_key_model_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(deleted_model_count, 0);
        let outbox_state: (String, i32, bool) = sqlx::query_as(
            "SELECT status, attempts, processed_at IS NOT NULL FROM billing WHERE id = $1",
        )
        .bind(billing_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(outbox_state, ("processed".into(), 0, true));

        // Replaying the same durable work must not add usage, consume the
        // allocation again, or produce a second ledger debit.
        assert!(process_billing_outbox_record(&pool, billing_id)
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            process_billing_outbox_batch(
                &pool,
                &ActivityRecorder::disabled(),
                &UsageDailyRecorder::disabled(),
                10,
            )
            .await
            .unwrap(),
            0
        );
        let account_state: (i64, i64) = sqlx::query_as(
            "SELECT balance_micros, reserved_micros FROM credit_account WHERE id = $1",
        )
        .bind(credit_account_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(account_state, (900, 0));
        let allocation_state: (i64, i64, String) = sqlx::query_as(
            "SELECT consumed_micros, returned_micros, status FROM credit_allocation WHERE id = $1",
        )
        .bind(allocation_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(allocation_state, (100, 0, "settled".into()));
        let ledger_state: (i64, i64, i64) = sqlx::query_as(
            "SELECT count(*), COALESCE(sum(amount_micros), 0)::BIGINT,
                    COALESCE(min(balance_after_micros), 0)
             FROM credit_ledger
             WHERE transaction_id = $1 AND usage_id = $2
               AND credit_account_id = $3 AND allocation_id = $4 AND reason = 'usage'",
        )
        .bind(transaction_id)
        .bind(usage_id)
        .bind(credit_account_id)
        .bind(allocation_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(ledger_state, (1, -100, 900));
        let usage_count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM usage WHERE billing_transaction_id = $1")
                .bind(transaction_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(usage_count, 1);
    }
}
