<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Refresh, User as UserIcon } from '@element-plus/icons-vue'
import { getUsers } from '../../../api/users'
import {
  getAdminUsage,
  getAdminUsageStatisticsSummary,
  type UsageStatisticsSummary
} from '../../../api/usage'
import { useBillingCurrency } from '../../../composables/useBillingCurrency'
import { useLatestTask } from '../../../composables/useLatestTask'
import { useLocale } from '../../../composables/useLocale'
import type { UsageRecord, User } from '../../../types/admin'
import { readError } from '../../../utils/errors'
import { formatDateTime, formatDurationMs, formatNumber } from '../../../utils/format'

const props = defineProps<{
  modelValue: boolean
  user: User | null
  showBalance: boolean
}>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()
const { locale, t } = useLocale()
const { formatMoney } = useBillingCurrency()
const text = (zh: string, en: string) => (locale.value === 'zh-CN' ? zh : en)
const visible = computed({
  get: () => props.modelValue,
  set: (value: boolean) => emit('update:modelValue', value)
})
const account = ref<User | null>(null)
const summary = ref<UsageStatisticsSummary | null>(null)
const records = ref<UsageRecord[]>([])
const nextCursor = ref<string | null>(null)
const profileError = ref('')
const summaryError = ref('')
const usageError = ref('')
const usageRetryAppend = ref(false)
const profileLoading = ref(false)
const summaryLoading = ref(false)
const usageLoading = ref(false)
const profileTask = useLatestTask(profileLoading)
const summaryTask = useLatestTask(summaryLoading)
const usageTask = useLatestTask(usageLoading)
const refreshing = computed(
  () => profileLoading.value || summaryLoading.value || usageLoading.value
)
const totals = computed(() => summary.value?.totals)
const statusLabel = computed(() => {
  if (account.value?.status === 'enabled') return t('enabled')
  if (account.value?.status === 'pending') return t('pendingApproval')
  return t('disabled')
})
const successRate = computed(() => {
  const value = totals.value
  if (!value?.request_count) return '—'
  return `${((value.success_count / value.request_count) * 100).toFixed(1)}%`
})

async function loadProfile() {
  const current = props.user
  if (!current) return
  profileError.value = ''
  try {
    await profileTask.run(
      () => getUsers({ email: current.email, limit: 100 }),
      (page) => {
        const updated = page.items.find((item) => item.id === current.id)
        if (updated) account.value = updated
        else
          profileError.value = text(
            '用户已不存在，请刷新列表',
            'This user no longer exists. Refresh the list.'
          )
      }
    )
  } catch (error) {
    profileError.value = readError(error)
  }
}

async function loadSummary() {
  const id = props.user?.id
  if (id == null) return
  summaryError.value = ''
  const end = new Date()
  const start = new Date(end.getTime() - 6 * 24 * 60 * 60 * 1000)
  try {
    await summaryTask.run(
      () =>
        getAdminUsageStatisticsSummary({
          user_id: id,
          start: start.toISOString().slice(0, 10),
          end: end.toISOString().slice(0, 10)
        }),
      (value) => {
        summary.value = value
      }
    )
  } catch (error) {
    summaryError.value = readError(error)
  }
}

async function loadUsage(append = false) {
  const id = props.user?.id
  if (id == null || (append && (!nextCursor.value || usageLoading.value))) return
  const cursor = append ? nextCursor.value! : undefined
  usageRetryAppend.value = append
  usageError.value = ''
  try {
    await usageTask.run(
      () => getAdminUsage({ user_id: id, limit: 20, cursor }),
      (page) => {
        // Cursor pages keep the descending order stable while new calls arrive.
        const seen = new Set(append ? records.value.map((item) => item.id) : [])
        const items = page.items.filter((item) => !seen.has(item.id))
        records.value = append ? [...records.value, ...items] : items
        nextCursor.value = page.has_more ? (page.next_cursor ?? null) : null
      }
    )
  } catch (error) {
    usageError.value = readError(error)
  }
}

function refresh() {
  void loadProfile()
  void loadSummary()
  void loadUsage()
}

function requestStatus(record: UsageRecord) {
  const code = record.status_code
  if (code == null) return { tone: 'unknown', label: text('未返回', 'No response') }
  if (code >= 200 && code < 400) return { tone: 'success', label: text('成功', 'Success') }
  return { tone: 'failed', label: text('失败', 'Failed') }
}

watch(
  () => [props.modelValue, props.user?.id] as const,
  ([open]) => {
    // Closing or changing users invalidates every in-flight result, including errors.
    profileTask.invalidate()
    summaryTask.invalidate()
    usageTask.invalidate()
    if (!open || !props.user) return
    account.value = { ...props.user }
    summary.value = null
    records.value = []
    nextCursor.value = null
    profileError.value = ''
    summaryError.value = ''
    usageError.value = ''
    refresh()
  },
  { immediate: true }
)
</script>

<template>
  <el-dialog
    v-model="visible"
    :title="text('用户详情', 'User details')"
    width="1040px"
    top="5vh"
    class="user-detail-dialog"
    append-to-body
    destroy-on-close
  >
    <div v-if="account" class="user-detail-content">
      <div class="detail-identity">
        <span class="detail-avatar"
          ><el-icon><UserIcon /></el-icon
        ></span>
        <div class="detail-identity-text">
          <div class="detail-name-line">
            <strong>{{ account.username || account.email }}</strong>
            <span class="detail-status" :class="`is-${account.status}`"
              ><i></i>{{ statusLabel }}</span
            >
          </div>
          <p>
            {{ account.email }} <span>· ID {{ account.id }}</span>
          </p>
        </div>
        <el-button :icon="Refresh" :loading="refreshing" @click="refresh">
          {{ t('refresh') }}
        </el-button>
      </div>

      <el-alert
        v-if="profileError"
        :title="profileError"
        type="warning"
        :closable="false"
        show-icon
      />

      <dl class="detail-facts">
        <div v-if="showBalance">
          <dt>{{ t('accountBalance') }}</dt>
          <dd>{{ formatMoney(account.balance_micros, locale, 2) }}</dd>
        </div>
        <div v-if="showBalance">
          <dt>{{ t('availableBalance') }}</dt>
          <dd>{{ formatMoney(account.available_micros, locale, 2) }}</dd>
        </div>
        <div>
          <dt>{{ t('userGroup') }}</dt>
          <dd>{{ account.user_group_name || '—' }}</dd>
        </div>
        <div>
          <dt>{{ t('userApiKeyCount') }}</dt>
          <dd>{{ formatNumber(account.user_key_count, locale) }}</dd>
        </div>
        <div>
          <dt>{{ t('createdAt') }}</dt>
          <dd>{{ formatDateTime(account.created_at, locale) }}</dd>
        </div>
        <div>
          <dt>{{ t('lastActiveAt') }}</dt>
          <dd>
            {{
              account.last_active_at
                ? formatDateTime(account.last_active_at, locale)
                : t('neverActive')
            }}
          </dd>
        </div>
      </dl>

      <section class="detail-usage-summary" :aria-label="text('近 7 天用量', 'Last 7 days usage')">
        <div class="detail-section-heading">
          <h3>{{ text('用量概况', 'Usage overview') }}</h3>
          <span>{{ text('近 7 天', 'Last 7 days') }}</span>
        </div>
        <div v-if="summaryError" class="detail-error" role="alert">
          <span
            >{{ text('用量统计加载失败', 'Could not load usage statistics') }}：{{
              summaryError
            }}</span
          >
          <el-button link type="primary" @click="loadSummary">{{
            text('重试', 'Retry')
          }}</el-button>
        </div>
        <div v-else class="detail-metrics" :aria-busy="summaryLoading">
          <div>
            <span>{{ text('调用次数', 'Requests') }}</span
            ><strong>{{ totals ? formatNumber(totals.request_count, locale) : '—' }}</strong>
          </div>
          <div>
            <span>{{ text('成功率', 'Success rate') }}</span
            ><strong>{{ successRate }}</strong
            ><small v-if="totals?.error_count"
              >{{ formatNumber(totals.error_count, locale) }} {{ text('次失败', 'failed') }}</small
            >
          </div>
          <div>
            <span>{{ text('Token 用量', 'Tokens') }}</span
            ><strong>{{ totals ? formatNumber(totals.total_tokens, locale) : '—' }}</strong>
          </div>
          <div>
            <span>{{ text('累计消费', 'Total cost') }}</span
            ><strong>{{ totals ? formatMoney(totals.cost_micros, locale, 4) : '—' }}</strong>
          </div>
        </div>
      </section>

      <section class="detail-recent-usage" :aria-label="text('最近调用记录', 'Recent requests')">
        <div class="detail-section-heading">
          <h3>{{ text('最近调用', 'Recent requests') }}</h3>
          <span>{{ text('最新记录在前', 'Newest first') }}</span>
        </div>
        <div v-if="usageError" class="detail-error" role="alert">
          <span
            >{{ text('调用记录加载失败', 'Could not load recent requests') }}：{{
              usageError
            }}</span
          >
          <el-button link type="primary" @click="loadUsage(usageRetryAppend)">{{
            text('重试', 'Retry')
          }}</el-button>
        </div>
        <el-table
          v-loading="usageLoading"
          :data="records"
          row-key="id"
          max-height="280"
          class="detail-usage-table"
        >
          <el-table-column :label="t('createdAt')" width="166">
            <template #default="{ row }"
              ><span class="detail-time">{{
                formatDateTime(row.created_at, locale)
              }}</span></template
            >
          </el-table-column>
          <el-table-column
            :label="text('模型 / 密钥', 'Model / key')"
            min-width="180"
            show-overflow-tooltip
          >
            <template #default="{ row }">
              <div class="detail-model">{{ row.model || '—' }}</div>
              <small class="detail-key"
                >{{ row.user_key_id ? `Key #${row.user_key_id}` : '—'
                }}<template v-if="row.streamed"> · {{ text('流式', 'Stream') }}</template></small
              >
            </template>
          </el-table-column>
          <el-table-column :label="text('状态', 'Status')" width="112">
            <template #default="{ row }">
              <el-tooltip
                :content="row.error_summary || `HTTP ${row.status_code ?? '—'}`"
                :show-after="300"
                placement="top"
              >
                <span class="detail-request-status" :class="`is-${requestStatus(row).tone}`"
                  >{{ requestStatus(row).label }}<small>{{ row.status_code ?? '—' }}</small></span
                >
              </el-tooltip>
            </template>
          </el-table-column>
          <el-table-column :label="text('用量', 'Usage')" min-width="110" align="right">
            <template #default="{ row }">
              <span>{{
                row.billing_meter === 'token'
                  ? formatNumber(row.total_tokens, locale)
                  : formatNumber(row.billable_units, locale)
              }}</span>
              <small class="detail-key">{{
                row.billing_meter === 'token' ? 'Token' : row.billing_meter
              }}</small>
            </template>
          </el-table-column>
          <el-table-column :label="text('费用', 'Cost')" width="110" align="right">
            <template #default="{ row }">{{ formatMoney(row.cost_micros, locale, 4) }}</template>
          </el-table-column>
          <el-table-column :label="text('耗时', 'Latency')" width="90" align="right">
            <template #default="{ row }">{{ formatDurationMs(row.latency_ms) }}</template>
          </el-table-column>
          <template #empty>
            <div class="detail-empty">
              {{
                usageLoading
                  ? text('正在加载调用记录…', 'Loading requests…')
                  : usageError
                    ? text('加载失败，请重试', 'Loading failed. Please retry.')
                    : text('该用户暂无调用记录', 'This user has no requests yet')
              }}
            </div>
          </template>
        </el-table>
        <div v-if="records.length" class="detail-records-footer">
          <span>{{ text(`已显示 ${records.length} 条`, `${records.length} records shown`) }}</span>
          <el-button
            v-if="nextCursor"
            link
            type="primary"
            :loading="usageLoading"
            :disabled="profileLoading || summaryLoading"
            @click="loadUsage(true)"
            >{{ text('加载更多', 'Load more') }}</el-button
          >
          <span v-else>{{ text('已显示全部', 'All records shown') }}</span>
        </div>
      </section>
    </div>
  </el-dialog>
</template>

<style scoped>
:global(.user-detail-dialog) {
  max-width: calc(100vw - 32px);
  max-height: 90dvh;
  display: flex;
  flex-direction: column;
  border-radius: 16px;
}
:global(.user-detail-dialog .el-dialog__header) {
  margin: 0;
  padding: 6px 12px 18px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}
:global(.user-detail-dialog .el-dialog__title) {
  font-size: 16px;
  font-weight: 650;
}
:global(.user-detail-dialog .el-dialog__body) {
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 24px 12px 8px;
}
.user-detail-content {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  min-width: 0;
  gap: 20px;
  color: var(--el-text-color-primary);
}
.detail-identity {
  display: flex;
  align-items: center;
  gap: 14px;
}
.detail-avatar {
  display: grid;
  flex: 0 0 48px;
  height: 48px;
  place-items: center;
  border: 1px solid #e1e9f5;
  border-radius: 14px;
  background: #f1f5fc;
  color: #5273b6;
  font-size: 23px;
}
.detail-identity-text {
  flex: 1;
  min-width: 0;
}
.detail-name-line {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}
.detail-name-line strong {
  font-size: 20px;
  font-weight: 650;
  overflow-wrap: anywhere;
}
.detail-identity p {
  margin: 7px 0 0;
  color: var(--el-text-color-regular);
  font-size: 13px;
  overflow-wrap: anywhere;
}
.detail-identity p span {
  color: var(--el-text-color-secondary);
}
.detail-status {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 3px 8px;
  border-radius: 6px;
  font-size: 12px;
  background: #f3f4f6;
  color: #667085;
  white-space: nowrap;
}
.detail-status i {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: currentColor;
}
.detail-status.is-enabled {
  background: #ecf8f1;
  color: #268158;
}
.detail-status.is-pending {
  background: #fff8e9;
  color: #a16b16;
}
.detail-facts {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 22px 24px;
  margin: 0;
  padding-bottom: 24px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}
.detail-facts dt {
  margin-bottom: 7px;
  color: var(--el-text-color-secondary);
  font-size: 12px;
}
.detail-facts dd {
  margin: 0;
  font-size: 14px;
  font-weight: 550;
  overflow-wrap: anywhere;
  font-variant-numeric: tabular-nums;
}
.detail-section-heading {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 14px;
}
.detail-section-heading h3 {
  margin: 0;
  font-size: 14px;
  font-weight: 650;
}
.detail-section-heading > span {
  color: var(--el-text-color-secondary);
  font-size: 12px;
}
.detail-metrics {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  padding: 18px 0;
  background: var(--el-fill-color-light);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 10px;
}
.detail-metrics > div {
  display: flex;
  flex-direction: column;
  gap: 9px;
  padding: 0 20px;
  min-width: 0;
}
.detail-metrics > div + div {
  border-left: 1px solid var(--el-border-color-lighter);
}
.detail-metrics span {
  color: var(--el-text-color-regular);
  font-size: 12px;
}
.detail-metrics strong {
  font-size: 23px;
  font-weight: 650;
  line-height: 1.2;
  letter-spacing: -0.4px;
  font-variant-numeric: tabular-nums;
  overflow-wrap: anywhere;
}
.detail-metrics small {
  color: #b55f4f;
  font-size: 11px;
}
.detail-error {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px;
  margin-bottom: 10px;
  background: var(--el-color-danger-light-9);
  color: var(--el-color-danger);
  border-radius: 8px;
  font-size: 12px;
  overflow-wrap: anywhere;
}
.detail-error .el-button {
  flex-shrink: 0;
}
.detail-usage-table {
  width: 100%;
  --el-table-header-bg-color: var(--el-fill-color-light);
  font-size: 12px;
}
.detail-model {
  font-size: 12px;
  font-weight: 550;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.detail-key {
  display: block;
  margin-top: 2px;
  font-size: 11px;
  color: var(--el-text-color-secondary);
}
.detail-time {
  color: var(--el-text-color-regular);
  font-size: 11px;
  white-space: nowrap;
}
.detail-request-status {
  display: inline-flex;
  gap: 5px;
  align-items: center;
  color: var(--el-text-color-secondary);
  font-size: 12px;
  cursor: help;
}
.detail-request-status.is-success {
  color: #268158;
}
.detail-request-status.is-failed {
  color: #c45050;
}
.detail-request-status small {
  font-size: 10px;
  opacity: 0.8;
  font-variant-numeric: tabular-nums;
}
.detail-empty {
  padding: 28px 0;
  color: var(--el-text-color-secondary);
}
.detail-records-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 40px;
  color: var(--el-text-color-secondary);
  font-size: 12px;
}
@media (max-width: 640px) {
  :global(.user-detail-dialog) {
    max-width: calc(100vw - 16px);
    margin-top: 3dvh;
    max-height: 94dvh;
    padding: 12px;
  }
  :global(.user-detail-dialog .el-dialog__body) {
    padding: 18px 4px 0;
  }
  .detail-identity {
    gap: 10px;
    flex-wrap: wrap;
  }
  .detail-identity > .el-button {
    margin-left: auto;
  }
  .detail-avatar {
    flex-basis: 40px;
    height: 40px;
  }
  .detail-name-line strong {
    font-size: 17px;
  }
  .detail-name-line {
    gap: 7px;
  }
  .detail-facts {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 18px 14px;
  }
  .detail-facts dd {
    font-size: 12px;
  }
  .detail-metrics {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    row-gap: 22px;
  }
  .detail-metrics > div {
    padding: 0 14px;
  }
  .detail-metrics > div:nth-child(3) {
    border-left: 0;
  }
  .detail-metrics strong {
    font-size: 20px;
  }
}
</style>
