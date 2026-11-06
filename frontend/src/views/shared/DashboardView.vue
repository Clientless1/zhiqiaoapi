<script setup lang="ts">
import { computed, ref, watch, onMounted, onBeforeUnmount } from 'vue'
import {
  RefreshRight,
  ArrowRight,
  Key,
  Grid,
  Promotion,
  Tickets,
  Wallet,
  DataLine,
  Connection,
  CopyDocument
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus/es/components/message/index'
import { getUserOverview } from '../../api/overview'
import { getAdminUsageStatisticsSummary, getAdminUsage, getUserUsage } from '../../api/usage'
import { getChannels } from '../../api/channels'
import { useAsyncData } from '../../composables/useAsyncData'
import { useBillingCurrency } from '../../composables/useBillingCurrency'
import { useLocale } from '../../composables/useLocale'
import { useStudio } from '../../composables/useStudio'
import { useAuthStore } from '../../stores/auth'
import { formatCompactDateTime } from '../../utils/format'
import { businessDays } from '../../utils/businessDate'
import BridgeVisual from '../../components/common/BridgeVisual.vue'
import ServiceHealthCard from '../../components/common/ServiceHealthCard.vue'
import TrendChart from '../../components/common/TrendChart.vue'
const props = defineProps<{ admin?: boolean }>()
const auth = useAuthStore(),
  { s } = useStudio(),
  { locale } = useLocale(),
  { formatMoney } = useBillingCurrency()
const range = ref(7),
  metric = ref<'cost' | 'requests'>('cost')
const today = () =>
  new Intl.DateTimeFormat('en-CA', {
    timeZone: 'UTC',
    year: 'numeric',
    month: '2-digit',
    day: '2-digit'
  }).format(new Date())
function query() {
  const days = businessDays(today(), range.value)
  return { start: days[0], end: days[days.length - 1] }
}
const {
  data: user,
  loading: userLoading,
  error: userError,
  reload: reloadUser
} = useAsyncData(() => (props.admin ? Promise.resolve(null) : getUserOverview()), null)
const {
  data: stats,
  loading: statsLoading,
  error: statsError,
  reload: reloadStats
} = useAsyncData(
  () => (props.admin ? getAdminUsageStatisticsSummary(query()) : Promise.resolve(null)),
  null
)
const {
  data: channels,
  error: channelError,
  reload: reloadChannels
} = useAsyncData(() => (props.admin ? getChannels() : Promise.resolve([])), [])
const {
  data: recent,
  loading: recentLoading,
  error: recentError,
  reload: reloadRecent
} = useAsyncData(
  () => (props.admin ? getAdminUsage({ limit: 5 }) : getUserUsage({ limit: 5 })),
  null
)
const loading = computed(() => userLoading.value || statsLoading.value || recentLoading.value)
const error = computed(() => userError.value || statsError.value)
const prefix = computed(() => (props.admin ? '/admin' : '/home'))
const cards = computed(() =>
  props.admin
    ? [
        {
          label: s('请求总数', 'Total requests'),
          value: stats.value?.totals.request_count?.toLocaleString(locale.value) ?? '—',
          note: s('所选时间范围', 'Selected period'),
          icon: Tickets
        },
        {
          label: s('用量费用', 'Usage cost'),
          value: stats.value ? formatMoney(stats.value.totals.cost_micros, locale.value, 2) : '—',
          note: s('所选时间范围', 'Selected period'),
          icon: Wallet
        },
        {
          label: s('总 Token 用量', 'Total tokens'),
          value: stats.value?.totals.total_tokens?.toLocaleString(locale.value) ?? '—',
          note: s('输入与输出合计', 'Input + output'),
          icon: DataLine
        },
        {
          label: s('请求成功率', 'Success rate'),
          value: stats.value?.totals.request_count
            ? `${((stats.value.totals.success_count / stats.value.totals.request_count) * 100).toFixed(1)}%`
            : '—',
          note: s('所选时间范围', 'Selected period'),
          icon: Connection
        }
      ]
    : [
        {
          label: s('账户可用额度', 'Available credit'),
          value: user.value ? formatMoney(user.value.available_micros, locale.value, 2) : '—',
          note: s('已扣除预留额度', 'After reserved credit'),
          icon: Wallet,
          featured: true
        },
        {
          label: s('今日消费', 'Today’s spend'),
          value: user.value ? formatMoney(user.value.today_cost_micros, locale.value, 2) : '—',
          note: user.value?.timezone ?? 'UTC',
          icon: DataLine
        },
        {
          label: s('本月消费', 'Monthly spend'),
          value: user.value ? formatMoney(user.value.month_cost_micros, locale.value, 2) : '—',
          note: s('当前自然月', 'Current calendar month'),
          icon: Tickets
        },
        {
          label: s('累计请求', 'All-time requests'),
          value: user.value?.request_count?.toLocaleString(locale.value) ?? '—',
          note: s('账户请求总数', 'Account total'),
          icon: Connection
        }
      ]
)
const chartDays = computed(() => {
  const dates = businessDays(props.admin ? today() : (user.value?.business_date ?? ''), range.value)
  const values = new Map(
    (props.admin ? (stats.value?.daily ?? []) : (user.value?.daily_costs ?? [])).map((d) => [
      d.date,
      d
    ])
  )
  return dates.map((date) => {
    const d = values.get(date)
    return {
      date,
      value:
        metric.value === 'requests' && d && 'request_count' in d
          ? Number(d.request_count)
          : (d?.cost_micros ?? 0)
    }
  })
})
const shortcuts = computed(() => [
  {
    to: `${prefix.value}/models`,
    label: s('模型广场', 'Model explorer'),
    hint: s('筛选与对比', 'Explore & compare'),
    icon: Grid
  },
  {
    to: `${prefix.value}/playground`,
    label: s('API 调试台', 'Playground'),
    hint: s('即刻验证模型', 'Try a real model'),
    icon: Promotion
  },
  {
    to: props.admin ? '/admin/channels' : '/home/apikeys',
    label: props.admin ? s('渠道管理', 'Channels') : 'API Key',
    hint: props.admin
      ? s('查看上游连接', 'Manage upstreams')
      : s('管理调用凭证', 'Manage credentials'),
    icon: Key
  },
  {
    to: `${prefix.value}/usage`,
    label: s('用量明细', 'Usage logs'),
    hint: s('查看请求记录', 'Inspect requests'),
    icon: Tickets
  }
])
const baseUrl = `${window.location.origin}/v1`
async function copy() {
  try {
    await navigator.clipboard.writeText(baseUrl)
    ElMessage.success(s('接入地址已复制', 'Base URL copied'))
  } catch {
    ElMessage.error(s('复制失败', 'Copy failed'))
  }
}
async function refresh() {
  await Promise.allSettled([reloadUser(), reloadStats(), reloadChannels(), reloadRecent()])
}
watch(range, () => {
  if (props.admin) void reloadStats()
})
let timer: number | undefined
onMounted(() => {
  timer = window.setInterval(() => {
    if (document.visibilityState === 'visible') void refresh()
  }, 60000)
})
onBeforeUnmount(() => window.clearInterval(timer))
</script>
<template>
  <section class="studio-dashboard">
    <header class="workspace-welcome">
      <div>
        <span class="kicker">{{ admin ? 'OPERATIONS WORKSPACE' : 'DEVELOPER WORKSPACE' }}</span>
        <h1>
          {{ s('你好，', 'Hello, ')
          }}{{ user?.display_name || auth.accountName || s('开发者', 'developer')
          }}<span class="welcome-wave">✦</span>
        </h1>
        <p>
          {{
            admin
              ? s(
                  '从一次请求到全局运行，一切尽在掌握。',
                  'A clear view of every request and every connection.'
                )
              : s(
                  '连接更好的模型，构建你的下一个想法。',
                  'Better models. Your next idea. One connection.'
                )
          }}
        </p>
        <RouterLink class="studio-primary-link" :to="prefix + '/playground'"
          >{{ s('开始探索', 'Start exploring') }} <el-icon><ArrowRight /></el-icon
        ></RouterLink>
      </div>
      <BridgeVisual compact />
    </header>
    <el-alert v-if="error" :title="error" type="error" :closable="false" />
    <div class="workspace-section-toolbar">
      <span
        >{{ s('账户概况', 'At a glance') }}
        <span class="muted"
          >/
          {{
            admin ? s('所选时间范围', 'Selected period') : s('实时数据', 'Live account data')
          }}</span
        ></span
      ><el-button text :icon="RefreshRight" :loading="loading" @click="refresh">{{
        s('刷新数据', 'Refresh')
      }}</el-button>
    </div>
    <div class="studio-metrics">
      <article
        v-for="(card, i) in cards"
        :key="card.label"
        class="studio-metric"
        :class="{ featured: 'featured' in card && card.featured }"
        :style="{ '--card-order': i }"
      >
        <div class="metric-heading">
          <span>{{ card.label }}</span
          ><el-icon><component :is="card.icon" /></el-icon>
        </div>
        <strong>{{ card.value }}</strong
        ><span class="metric-note">{{ card.note }}</span>
      </article>
    </div>
    <div class="workspace-main-grid">
      <article class="studio-panel workspace-chart">
        <div class="panel-heading">
          <div>
            <h3>{{ s('用量趋势', 'Usage trends') }}</h3>
            <p>
              {{
                metric === 'cost'
                  ? s('每日消费金额', 'Daily spend')
                  : s('每日请求数量', 'Daily requests')
              }}
              · {{ admin ? 'UTC' : (user?.timezone ?? 'UTC') }}
            </p>
          </div>
          <div class="chart-options">
            <select v-if="admin" v-model="metric" :aria-label="s('图表指标', 'Chart metric')">
              <option value="cost">{{ s('费用', 'Spend') }}</option>
              <option value="requests">{{ s('请求数', 'Requests') }}</option>
            </select>
            <div class="studio-segments">
              <button
                v-for="n in admin ? [7, 30, 90] : [7, 30]"
                :key="n"
                :class="{ active: range === n }"
                @click="range = n"
              >
                {{ n }}{{ s('天', 'D') }}
              </button>
            </div>
          </div>
        </div>
        <TrendChart
          v-if="!error && chartDays.length"
          :days="chartDays"
          :label="metric === 'cost' ? s('费用', 'Spend') : s('请求', 'Requests')"
          :format="
            metric === 'cost' ? (v) => formatMoney(v, locale, 2) : (v) => v.toLocaleString(locale)
          "
        /><el-empty
          v-else
          :description="s('暂无可展示数据', 'No data available')"
          :image-size="64"
        />
        <div class="chart-total">
          <span>{{ s('区间合计', 'Period total') }}</span
          ><strong>{{
            metric === 'cost'
              ? formatMoney(
                  chartDays.reduce((n, d) => n + d.value, 0),
                  locale,
                  4
                )
              : chartDays.reduce((n, d) => n + d.value, 0).toLocaleString(locale)
          }}</strong>
        </div>
      </article>
      <div class="workspace-side">
        <ServiceHealthCard />
        <article class="workspace-connection">
          <span class="kicker">ONE ENDPOINT</span>
          <h3>{{ s('一个地址，连接多种能力', 'One endpoint. More possibilities.') }}</h3>
          <p>OpenAI-compatible API</p>
          <button @click="copy">
            <code>{{ baseUrl }}</code
            ><el-icon><CopyDocument /></el-icon></button
          ><RouterLink to="/docs">{{ s('查看接入文档', 'Integration guide') }} ↗</RouterLink>
        </article>
      </div>
    </div>
    <article class="studio-panel workspace-shortcuts">
      <div class="panel-heading">
        <h3>{{ s('快捷入口', 'Quick access') }}</h3>
        <span class="muted">{{ s('让下一步更简单', 'Your next step, closer') }}</span>
      </div>
      <div class="shortcut-grid">
        <RouterLink v-for="item in shortcuts" :key="item.to" :to="item.to"
          ><span class="shortcut-icon"
            ><el-icon><component :is="item.icon" /></el-icon
          ></span>
          <div>
            <strong>{{ item.label }}</strong
            ><span>{{ item.hint }}</span>
          </div>
          <span class="shortcut-arrow">↗</span></RouterLink
        >
      </div>
    </article>
    <div class="workspace-bottom-grid">
      <article class="studio-panel recent-requests">
        <div class="panel-heading">
          <div>
            <h3>{{ s('最近请求', 'Recent requests') }}</h3>
            <p>{{ s('最近 5 条请求记录', 'Latest five requests') }}</p>
          </div>
          <RouterLink :to="prefix + '/usage'">{{ s('查看全部', 'View all') }} →</RouterLink>
        </div>
        <el-alert v-if="recentError" :title="recentError" type="error" :closable="false" /><el-table
          v-loading="recentLoading"
          :data="recent?.items ?? []"
          :empty-text="
            s(
              '还没有请求记录，去调试台发送第一条消息吧',
              'No requests yet. Send your first message in the Playground.'
            )
          "
          ><el-table-column :label="s('时间', 'Time')" min-width="145"
            ><template #default="{ row }">{{
              formatCompactDateTime(row.created_at)
            }}</template></el-table-column
          ><el-table-column
            prop="model"
            :label="s('模型', 'Model')"
            min-width="170"
            show-overflow-tooltip
          /><el-table-column :label="s('状态', 'Status')" width="85"
            ><template #default="{ row }"
              ><span
                :class="[
                  'request-status',
                  row.status_code >= 200 && row.status_code < 400 ? 'good' : 'bad'
                ]"
                >{{ row.status_code }}</span
              ></template
            ></el-table-column
          ><el-table-column :label="s('耗时', 'Latency')" width="90"
            ><template #default="{ row }"
              >{{ (row.latency_ms / 1000).toFixed(2) }} s</template
            ></el-table-column
          ><el-table-column :label="s('费用', 'Cost')" width="110"
            ><template #default="{ row }">{{
              formatMoney(row.cost_micros, locale, 4)
            }}</template></el-table-column
          ></el-table
        >
      </article>
      <article v-if="admin" class="studio-panel channel-summary">
        <div class="panel-heading">
          <h3>{{ s('上游渠道', 'Upstream channels') }}</h3>
          <RouterLink to="/admin/channels">{{ s('管理', 'Manage') }} ↗</RouterLink>
        </div>
        <el-alert
          v-if="channelError"
          :title="channelError"
          type="error"
          :closable="false"
        /><el-empty
          v-else-if="!channels.length"
          :image-size="55"
          :description="s('暂无渠道', 'No channels')"
        />
        <div v-for="channel in channels.slice(0, 5)" :key="channel.id" class="channel-summary-row">
          <div>
            <strong>{{ channel.name }}</strong
            ><span
              >{{ channel.models.filter((m) => m.enabled).length }}
              {{ s('个启用模型', 'enabled models') }}</span
            >
          </div>
          <span :class="['studio-pill', channel.enabled ? 'good' : 'muted']">{{
            channel.enabled ? s('已启用', 'Enabled') : s('已停用', 'Disabled')
          }}</span>
        </div>
        <p class="studio-note">
          {{
            s(
              '启用状态不代表实时可用性，详细探测结果请查看渠道管理。',
              'Enabled status is not a live availability check. Inspect probes in channel management.'
            )
          }}
        </p>
      </article>
      <article v-else class="studio-panel getting-started">
        <span class="kicker">BUILD SOMETHING GREAT</span>
        <h3>{{ s('三步完成接入', 'Your first integration') }}</h3>
        <RouterLink to="/home/models"
          ><b>01</b>
          <div>
            <strong>{{ s('选择一个模型', 'Choose a model') }}</strong
            ><span>{{ s('按能力和价格找到合适选项', 'Find the right capability and price') }}</span>
          </div>
          ↗</RouterLink
        ><RouterLink to="/home/apikeys"
          ><b>02</b>
          <div>
            <strong>{{ s('创建 API Key', 'Create an API key') }}</strong
            ><span>{{ s('绑定套餐与模型访问范围', 'Set package and model access') }}</span>
          </div>
          ↗</RouterLink
        ><RouterLink to="/home/playground"
          ><b>03</b>
          <div>
            <strong>{{ s('发送第一次请求', 'Make your first request') }}</strong
            ><span>{{ s('在调试台验证响应', 'Inspect the response in Playground') }}</span>
          </div>
          ↗</RouterLink
        >
      </article>
    </div>
  </section>
</template>
