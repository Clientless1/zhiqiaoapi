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
import BluePrism from '../../components/common/BluePrism.vue'
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
  <section class="aurora-dashboard">
    <div class="zqd-page-heading">
      <div>
        <h1>{{ s('工作台', 'Workspace') }}</h1>
        <span>{{
          admin ? s('全局概览', 'Operations overview') : s('我的概览', 'Your overview')
        }}</span>
      </div>
      <div>
        <span class="zqd-timezone">{{ admin ? 'UTC' : (user?.timezone ?? 'UTC') }}</span
        ><button class="zqd-refresh" :disabled="loading" @click="refresh">
          <RefreshRight :class="{ spinning: loading }" />{{ s('刷新数据', 'Refresh') }}
        </button>
      </div>
    </div>
    <el-alert v-if="error" :title="error" type="error" :closable="false" />
    <div class="zqd-overview">
      <header class="zqd-welcome">
        <div class="zqd-welcome-copy">
          <span class="zqd-welcome-eyebrow">YOUR AI WORKSPACE</span>
          <h2>
            {{ s('你好，', 'Hello, ')
            }}{{ user?.display_name || auth.accountName || s('开发者', 'developer') }}<span>✦</span>
          </h2>
          <p>
            {{
              admin
                ? s(
                    '每一次连接与调用，都在这里一目了然。',
                    'Every connection and request, in one clear view.'
                  )
                : s(
                    '欢迎回来，开始构建你的下一个好想法。',
                    'Welcome back. Let’s build your next great idea.'
                  )
            }}
          </p>
          <RouterLink :to="prefix + '/playground'"
            >{{ s('开始探索', 'Start exploring') }}<ArrowRight
          /></RouterLink>
        </div>
        <div class="zqd-welcome-art" aria-hidden="true">
          <BluePrism /><span class="zqd-art-label">CONNECT TO CREATE</span>
        </div>
      </header>
      <div class="zqd-metrics">
        <article
          v-for="(card, i) in cards"
          :key="card.label"
          class="zqd-metric"
          :class="'zqd-metric-' + i"
        >
          <span class="zqd-metric-icon"><component :is="card.icon" /></span>
          <div>
            <span class="zqd-metric-label">{{ card.label }}</span
            ><strong>{{ card.value }}</strong
            ><small>{{ card.note }}</small>
          </div>
          <div class="zqd-metric-decoration" aria-hidden="true"><i></i><i></i><i></i></div>
        </article>
      </div>
    </div>
    <div class="zqd-main-grid">
      <article class="zqd-panel zqd-chart">
        <div class="zqd-panel-heading">
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
          <div class="zqd-chart-options">
            <select v-if="admin" v-model="metric" :aria-label="s('图表指标', 'Chart metric')">
              <option value="cost">{{ s('费用', 'Spend') }}</option>
              <option value="requests">{{ s('请求数', 'Requests') }}</option>
            </select>
            <div class="zqd-segments">
              <button
                v-for="n in admin ? [7, 30, 90] : [7, 30]"
                :key="n"
                :aria-pressed="range === n"
                :class="{ active: range === n }"
                @click="range = n"
              >
                {{ n }}{{ s('天', 'D') }}
              </button>
            </div>
          </div>
        </div>
        <div class="zqd-chart-frame">
          <TrendChart
            v-if="!error && chartDays.length"
            :days="chartDays"
            :label="metric === 'cost' ? s('费用', 'Spend') : s('请求', 'Requests')"
            :format="
              metric === 'cost' ? (v) => formatMoney(v, locale, 2) : (v) => v.toLocaleString(locale)
            "
          />
          <div v-if="!error && !chartDays.some((d) => d.value > 0)" class="zqd-chart-empty">
            <span><DataLine /></span
            ><strong>{{
              metric === 'cost'
                ? s('所选时段暂无费用', 'No spend in this period')
                : s('所选时段暂无请求', 'No requests in this period')
            }}</strong>
            <p>
              {{
                s(
                  '调用记录更新后，趋势会自动呈现在这里',
                  'Trends will update automatically as usage is recorded'
                )
              }}
            </p>
            <RouterLink :to="prefix + '/playground'"
              >{{ s('去调试台试试', 'Try Playground') }} ↗</RouterLink
            >
          </div>
          <el-empty
            v-if="error"
            :description="s('数据加载失败，请重试', 'Unable to load data. Please retry.')"
            :image-size="60"
          />
        </div>
        <div class="zqd-chart-footer">
          <span
            ><i></i
            >{{ metric === 'cost' ? s('消费金额', 'Spend') : s('请求数量', 'Requests') }}</span
          >
          <div>
            {{ s('区间合计', 'Period total')
            }}<strong>{{
              metric === 'cost'
                ? formatMoney(
                    chartDays.reduce((n, d) => n + d.value, 0),
                    locale,
                    4
                  )
                : chartDays.reduce((n, d) => n + d.value, 0).toLocaleString(locale)
            }}</strong>
          </div>
        </div>
      </article>
      <aside class="zqd-side">
        <ServiceHealthCard />
        <article class="zqd-connection">
          <div class="zqd-connection-title">
            <span><Connection /></span>
            <h3>{{ s('你的统一接入地址', 'Your API endpoint') }}</h3>
          </div>
          <p>OpenAI-compatible API</p>
          <button @click="copy">
            <code>{{ baseUrl }}</code
            ><CopyDocument /></button
          ><RouterLink to="/docs"
            >{{ s('查看接入文档', 'Read the integration guide') }}<ArrowRight
          /></RouterLink>
        </article>
      </aside>
    </div>
    <div class="zqd-lower-grid">
      <article class="zqd-panel zqd-shortcuts">
        <div class="zqd-panel-heading">
          <h3>{{ s('快捷入口', 'Quick access') }}</h3>
          <span>{{ s('下一步，从这里开始', 'Your next step starts here') }}</span>
        </div>
        <div class="zqd-shortcut-grid">
          <RouterLink
            v-for="(item, i) in shortcuts"
            :key="item.to"
            :to="item.to"
            :class="'zqd-shortcut-' + i"
            ><span><component :is="item.icon" /></span><strong>{{ item.label }}</strong
            ><small>{{ item.hint }}</small></RouterLink
          >
        </div>
      </article>
      <article v-if="admin" class="zqd-panel zqd-channels">
        <div class="zqd-panel-heading">
          <h3>{{ s('上游渠道', 'Upstream channels') }}</h3>
          <RouterLink to="/admin/channels">{{ s('管理渠道', 'Manage') }} ↗</RouterLink>
        </div>
        <el-alert v-if="channelError" :title="channelError" type="error" :closable="false" />
        <div v-else-if="!channels.length" class="zqd-inline-empty">
          {{ s('还没有配置渠道', 'No channels configured')
          }}<RouterLink to="/admin/channels"
            >{{ s('前往配置', 'Configure channels') }} ↗</RouterLink
          >
        </div>
        <div v-for="channel in channels.slice(0, 2)" :key="channel.id" class="zqd-channel-row">
          <span class="zqd-channel-symbol"><Connection /></span>
          <div>
            <strong>{{ channel.name }}</strong
            ><small
              >{{ channel.models.filter((m) => m.enabled).length }}
              {{ s('个启用模型', 'enabled models') }}</small
            >
          </div>
          <span class="zqd-channel-pill" :class="{ disabled: !channel.enabled }"
            ><i></i>{{ channel.enabled ? s('已启用', 'Enabled') : s('已停用', 'Disabled') }}</span
          >
        </div>
        <p class="zqd-channel-note">
          {{
            s(
              '渠道启用状态，实时探测请查看渠道详情。',
              'Enabled state. See channel details for live probe results.'
            )
          }}
        </p>
      </article>
      <article v-else class="zqd-panel zqd-first-steps">
        <div class="zqd-panel-heading">
          <h3>{{ s('三步完成接入', 'Your first integration') }}</h3>
          <RouterLink to="/docs">{{ s('查看指南', 'View guide') }} ↗</RouterLink>
        </div>
        <div>
          <RouterLink to="/home/models"
            ><b>01</b><span>{{ s('选择模型', 'Choose model') }}</span
            ><ArrowRight /></RouterLink
          ><RouterLink to="/home/apikeys"
            ><b>02</b><span>{{ s('创建 API Key', 'Create API key') }}</span
            ><ArrowRight /></RouterLink
          ><RouterLink to="/home/playground"
            ><b>03</b><span>{{ s('发送请求', 'Make a request') }}</span
            ><ArrowRight
          /></RouterLink>
        </div>
      </article>
    </div>
    <article class="zqd-panel zqd-recent">
      <div class="zqd-panel-heading">
        <div>
          <h3>{{ s('最近请求', 'Recent requests') }}</h3>
          <p>{{ s('最近 5 条调用记录', 'Your latest five requests') }}</p>
        </div>
        <RouterLink :to="prefix + '/usage'"
          >{{ s('查看全部', 'View all') }}<ArrowRight
        /></RouterLink>
      </div>
      <el-alert v-if="recentError" :title="recentError" type="error" :closable="false" />
      <el-table
        v-loading="recentLoading"
        :data="recent?.items ?? []"
        :empty-text="
          s(
            '暂无调用记录，发送第一条请求后即可在这里查看',
            'No requests yet. Your first request will appear here.'
          )
        "
      >
        <el-table-column :label="s('时间', 'Time')" min-width="145"
          ><template #default="{ row }">{{
            formatCompactDateTime(row.created_at)
          }}</template></el-table-column
        >
        <el-table-column
          prop="model"
          :label="s('模型', 'Model')"
          min-width="170"
          show-overflow-tooltip
        />
        <el-table-column :label="s('状态', 'Status')" width="85"
          ><template #default="{ row }"
            ><span
              :class="[
                'request-status',
                row.status_code >= 200 && row.status_code < 400 ? 'good' : 'bad'
              ]"
              >{{ row.status_code }}</span
            ></template
          ></el-table-column
        >
        <el-table-column :label="s('耗时', 'Latency')" width="100"
          ><template #default="{ row }"
            >{{ (row.latency_ms / 1000).toFixed(2) }} s</template
          ></el-table-column
        >
        <el-table-column :label="s('费用', 'Cost')" width="110"
          ><template #default="{ row }">{{
            formatMoney(row.cost_micros, locale, 4)
          }}</template></el-table-column
        >
      </el-table>
    </article>
    <footer class="zqd-footer">
      {{ s('智桥 API · 让连接更简单', 'Zhiqiao API · A simpler connection')
      }}<RouterLink to="/docs"
        >{{ s('需要帮助？查看文档', 'Need help? Read the docs') }} ↗</RouterLink
      >
    </footer>
  </section>
</template>
<style src="../../styles/aurora-dashboard.css"></style>
