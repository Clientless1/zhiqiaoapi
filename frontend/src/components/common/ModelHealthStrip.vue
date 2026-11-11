<script setup lang="ts">
import { computed } from 'vue'
import { Timer, TrendCharts } from '@element-plus/icons-vue'
import type { ModelHealth, ModelHealthStatus } from '../../api/modelHealth'
import { useStudio } from '../../composables/useStudio'
import { useLocale } from '../../composables/useLocale'

const props = defineProps<{ health?: ModelHealth; unavailable?: boolean }>()
const { s } = useStudio()
const { locale } = useLocale()
const status = computed(() => props.health?.status ?? 'unknown')
const label = (value: ModelHealthStatus) =>
  ({
    operational: s('正常', 'Operational'),
    degraded: s('波动', 'Degraded'),
    outage: s('异常', 'Outage'),
    unknown: s('未监测', 'No sample')
  })[value]
const history = computed(() => props.health?.history ?? Array.from({ length: 24 }, () => null))
const latency = computed(() =>
  props.health?.latency_ms == null ? '—' : `${(props.health.latency_ms / 1000).toFixed(2)}s`
)
const speed = computed(() =>
  props.health?.tokens_per_second == null ? '—' : `${props.health.tokens_per_second.toFixed(1)} t/s`
)
const availability = computed(() =>
  props.health?.availability_percent == null
    ? '—'
    : `${props.health.availability_percent.toFixed(1)}%`
)
function date(value: string) {
  return new Date(value).toLocaleString(locale.value, {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false
  })
}
function bucketTitle(index: number) {
  const bucket = history.value[index]
  if (!bucket) return s('暂无探测记录', 'No probe sample')
  return `${date(bucket.start_at)} · ${label(bucket.status)} · ${bucket.success_count}/${bucket.sample_count} ${s('次成功', 'passed')}`
}
</script>
<template>
  <div class="model-health-strip">
    <div class="model-health-top">
      <span class="model-health-state" :class="status"
        ><i></i>{{ unavailable ? s('状态暂不可用', 'Status unavailable') : label(status) }}</span
      >
      <span
        v-if="health?.last_checked_at"
        class="model-health-time"
        :title="s('最近探测', 'Last probe')"
        >{{ date(health.last_checked_at) }}</span
      >
      <span v-else class="model-health-time">{{ s('暂无探测记录', 'No probe samples') }}</span>
    </div>
    <div class="model-health-metrics">
      <span
        :title="
          s(
            '成功探测的平均完成耗时，并非首字延迟',
            'Mean successful probe duration, not time to first token'
          )
        "
        ><Timer />{{ s('耗时', 'Duration') }}<strong>{{ latency }}</strong></span
      >
      <span
        :title="
          s('探测尚未采集生成 Token 数时显示 —', 'Unavailable until generated tokens are recorded')
        "
        ><TrendCharts />{{ s('吞吐', 'Speed') }}<strong>{{ speed }}</strong></span
      >
    </div>
    <div class="model-health-bottom">
      <div
        class="model-health-bars"
        role="img"
        :aria-label="s('最近 24 小时探测历史', '24 hour probe history')"
      >
        <span
          v-for="(bucket, index) in history"
          :key="index"
          :class="bucket?.status ?? 'unknown'"
          :title="bucketTitle(index)"
        ></span>
      </div>
      <strong
        class="model-health-availability"
        :class="status"
        :title="
          s('24 小时探测成功率；灰色表示无样本', '24 hour probe success rate; gray means no sample')
        "
        >{{ availability }}</strong
      >
    </div>
  </div>
</template>
<style scoped>
.model-health-strip {
  border-top: 1px solid #eaf0f5;
  padding: 9px 0 10px;
  font-size: 11px;
  color: #8792a4;
}
.model-health-top,
.model-health-bottom,
.model-health-metrics,
.model-health-metrics > span {
  display: flex;
  align-items: center;
}
.model-health-top {
  justify-content: space-between;
  gap: 8px;
}
.model-health-state {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  color: #8995a6;
}
.model-health-state i {
  width: 6px;
  height: 6px;
  background: currentColor;
  border-radius: 50%;
}
.model-health-time {
  font-size: 10px;
  font-variant-numeric: tabular-nums;
}
.model-health-metrics {
  margin: 7px 0 8px;
  gap: 16px;
  flex-wrap: wrap;
}
.model-health-metrics > span {
  gap: 5px;
}
.model-health-metrics svg {
  width: 12px;
  height: 12px;
  color: #6484b8;
}
.model-health-metrics strong {
  font-weight: 500;
  color: #52627b;
  font-variant-numeric: tabular-nums;
}
.model-health-bottom {
  gap: 12px;
}
.model-health-bars {
  display: flex;
  gap: 3px;
  flex: 1;
  min-width: 0;
}
.model-health-bars span {
  flex: 1;
  height: 12px;
  border-radius: 2px;
  background: #e7ecf2;
  transition: opacity 0.15s;
}
.model-health-bars span:hover {
  opacity: 0.65;
}
.model-health-bars .operational {
  background: #31bd8a;
}
.model-health-bars .degraded {
  background: #eab65b;
}
.model-health-bars .outage {
  background: #eb8080;
}
.model-health-state.operational,
.model-health-availability.operational {
  color: #199a70;
}
.model-health-state.degraded,
.model-health-availability.degraded {
  color: #bd8a28;
}
.model-health-state.outage,
.model-health-availability.outage {
  color: #d06464;
}
.model-health-availability {
  min-width: 47px;
  text-align: right;
  font-size: 12px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
</style>
