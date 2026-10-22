<script setup lang="ts">
import { computed, inject, onBeforeUnmount, onMounted, type Ref } from 'vue'
import { RefreshRight } from '@element-plus/icons-vue'
import { getUserOverview } from '../../api/overview'
import { getUserUsage } from '../../api/usage'
import type { ServicePolicy } from '../../api/policy'
import { useAsyncData } from '../../composables/useAsyncData'
import { useBillingCurrency } from '../../composables/useBillingCurrency'
import { useLocale } from '../../composables/useLocale'
import { formatCompactDateTime } from '../../utils/format'
import { businessDays } from '../../utils/businessDate'

const { locale, t } = useLocale()
const { formatMoney } = useBillingCurrency()
const servicePolicy = inject<Ref<ServicePolicy | null>>('servicePolicy')!
const { data: overview, loading, error, reload } = useAsyncData(getUserOverview, null)
const {
  data: recent,
  loading: recentLoading,
  error: recentError,
  reload: reloadRecent
} = useAsyncData(() => getUserUsage({ limit: 5 }), null)
const showBalance = computed(
  () => servicePolicy.value?.credit_required || servicePolicy.value?.recharge_enabled
)
const cards = computed(() => [
  ...(showBalance.value
    ? [
        {
          label: t('currentBalance'),
          value: formatMoney(overview.value?.available_micros, locale.value, 2),
          balance: true
        }
      ]
    : []),
  { label: t('todayCost'), value: formatMoney(overview.value?.today_cost_micros, locale.value, 2) },
  { label: t('monthCost'), value: formatMoney(overview.value?.month_cost_micros, locale.value, 2) },
  {
    label: t('dashboardRequests'),
    value: (overview.value?.request_count ?? 0).toLocaleString(locale.value)
  }
])
const days = computed(() => {
  const costs = new Map(
    (overview.value?.daily_costs ?? []).map((day) => [day.date, day.cost_micros])
  )
  return businessDays(overview.value?.business_date ?? '').map((date) => ({
    date,
    cost: costs.get(date) ?? 0
  }))
})
const points = computed(() => {
  const max = Math.max(1, ...days.value.map((day) => day.cost))
  return days.value
    .map((day, index) => `${(index * 100) / 29},${88 - (day.cost * 72) / max}`)
    .join(' ')
})
async function refresh() {
  await Promise.allSettled([reload(), reloadRecent()])
}
let timer: number | undefined
onMounted(() => {
  timer = window.setInterval(() => {
    if (document.visibilityState === 'visible') void refresh()
  }, 30000)
})
onBeforeUnmount(() => window.clearInterval(timer))
</script>

<template>
  <section class="dashboard">
    <div class="dashboard-toolbar">
      <RouterLink class="dashboard-start" to="/home/apikeys">{{ t('manageApiKeys') }} ↗</RouterLink>
      <el-button :icon="RefreshRight" :loading="loading || recentLoading" @click="refresh">{{
        t('dashboardRefresh')
      }}</el-button>
    </div>
    <el-alert v-if="error" type="error" :title="error" :closable="false" />
    <div v-loading="loading" class="dashboard-metrics">
      <article
        v-for="card in cards"
        :key="card.label"
        class="user-panel dashboard-metric"
        :class="{ 'is-balance': card.balance }"
      >
        <span>{{ card.label }}</span
        ><strong>{{ card.value }}</strong>
        <RouterLink v-if="card.balance && servicePolicy?.recharge_enabled" to="/home/recharge"
          >{{ t('goRecharge') }} →</RouterLink
        >
      </article>
    </div>
    <article class="user-panel dashboard-chart">
      <div class="dashboard-heading">
        <h3>{{ t('dashboardTrend') }}</h3>
        <span>{{
          formatMoney(
            days.reduce((total, day) => total + day.cost, 0),
            locale,
            4
          )
        }}</span>
      </div>
      <div class="dashboard-chart-canvas">
        <svg
          viewBox="0 0 100 100"
          preserveAspectRatio="none"
          role="img"
          :aria-label="t('dashboardTrend')"
        >
          <line
            v-for="y in [20, 44, 68, 92]"
            :key="y"
            x1="0"
            :y1="y"
            x2="100"
            :y2="y"
            class="chart-grid"
          />
          <polygon :points="`0,92 ${points} 100,92`" class="chart-area" />
          <polyline :points="points" class="chart-line" />
        </svg>
      </div>
      <div class="dashboard-chart-axis">
        <span>{{ days[0]?.date }}</span
        ><span>{{ days[14]?.date }}</span
        ><span>{{ days[29]?.date }}</span>
      </div>
      <p class="commerce-label">{{ t('dashboardUTC') }}</p>
    </article>
    <article class="user-panel dashboard-recent">
      <div class="dashboard-heading">
        <div>
          <h3>{{ t('dashboardRecent') }}</h3>
          <p>{{ t('dashboardRecentHint') }}</p>
        </div>
        <RouterLink to="/home/usage">{{ t('viewUsageDetails') }} →</RouterLink>
      </div>
      <el-alert v-if="recentError" :title="recentError" type="error" :closable="false" />
      <el-table
        v-loading="recentLoading"
        :data="recent?.items ?? []"
        :empty-text="t('dashboardEmpty')"
      >
        <el-table-column :label="t('consoleRequestTime')" min-width="150"
          ><template #default="{ row }">{{
            formatCompactDateTime(row.created_at)
          }}</template></el-table-column
        >
        <el-table-column prop="model" :label="t('model')" min-width="160" show-overflow-tooltip />
        <el-table-column :label="t('consoleRequestStatus')" width="90"
          ><template #default="{ row }"
            ><el-tag
              :type="row.status_code >= 200 && row.status_code < 400 ? 'success' : 'danger'"
              size="small"
              >{{
                row.status_code >= 200 && row.status_code < 400
                  ? t('consoleSuccess')
                  : t('consoleFailure')
              }}</el-tag
            ></template
          ></el-table-column
        >
        <el-table-column :label="t('consoleLatency')" width="100"
          ><template #default="{ row }"
            >{{ (row.latency_ms / 1000).toFixed(2) }} s</template
          ></el-table-column
        >
        <el-table-column :label="t('consoleCost')" width="110"
          ><template #default="{ row }">{{
            formatMoney(row.cost_micros, locale, 4)
          }}</template></el-table-column
        >
      </el-table>
    </article>
  </section>
</template>

<style scoped>
.dashboard {
  display: grid;
  gap: 20px;
  width: 100%;
  color: #273249;
}
.dashboard-toolbar,
.dashboard-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}
.dashboard-start,
.dashboard-heading a {
  color: #5145ef;
  text-decoration: none;
  font-size: 13px;
}
.dashboard-metrics {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
  gap: 16px;
}
.dashboard-metric {
  padding: 22px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-height: 122px;
  border-radius: 12px;
}
.dashboard-metric > span {
  color: #7a849a;
  font-size: 13px;
}
.dashboard-metric strong {
  font-size: 29px;
  font-weight: 650;
  letter-spacing: -0.6px;
}
.dashboard-metric.is-balance {
  background: linear-gradient(120deg, #5149ef, #7266fa);
  color: #fff;
  border-color: transparent;
}
.dashboard-metric.is-balance > span,
.dashboard-metric.is-balance > a {
  color: #f0edff;
}
.dashboard-metric a {
  font-size: 12px;
  text-decoration: none;
}
.dashboard-chart,
.dashboard-recent {
  padding: 22px;
  border-radius: 12px;
  overflow: hidden;
}
.dashboard-heading {
  margin-bottom: 16px;
}
.dashboard-heading h3 {
  font-size: 15px;
  margin: 0;
  font-weight: 600;
}
.dashboard-heading p {
  font-size: 12px;
  color: #8a93a6;
  margin: 6px 0 0;
}
.dashboard-heading > span {
  color: #727d93;
  font-size: 13px;
}
.dashboard-chart-canvas {
  height: 155px;
}
.dashboard-chart svg {
  width: 100%;
  height: 100%;
  overflow: visible;
}
.chart-grid {
  stroke: #eff0f7;
  stroke-width: 1;
  vector-effect: non-scaling-stroke;
}
.chart-area {
  fill: #eeeaff;
  opacity: 0.6;
}
.chart-line {
  fill: none;
  stroke: #6655f5;
  stroke-width: 2;
  vector-effect: non-scaling-stroke;
  stroke-linecap: round;
  stroke-linejoin: round;
}
.dashboard-chart-axis {
  display: flex;
  justify-content: space-between;
  color: #99a0b1;
  font-size: 11px;
  padding-top: 10px;
}
.dashboard-recent :deep(.el-table) {
  font-size: 12px;
}
.dashboard-recent :deep(.el-table__empty-text) {
  line-height: 1.6;
  padding: 30px 12px;
}
@media (max-width: 600px) {
  .dashboard {
    gap: 14px;
  }
  .dashboard-metrics {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }
  .dashboard-metric {
    padding: 16px;
    min-height: 105px;
  }
  .dashboard-metric strong {
    font-size: 23px;
  }
  .dashboard-chart,
  .dashboard-recent {
    padding: 16px;
  }
  .dashboard-heading {
    align-items: flex-start;
  }
  .dashboard-heading a {
    font-size: 12px;
    text-align: right;
  }
}
</style>
