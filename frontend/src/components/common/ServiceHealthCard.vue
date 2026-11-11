<script setup lang="ts">
import { computed } from 'vue'
import { useStudio } from '../../composables/useStudio'
import { useServiceHealth } from '../../composables/useServiceHealth'
const { s } = useStudio()
const { health, latency, checkedAt, loading, error, refresh } = useServiceHealth()
const rows = computed(() => [
  { name: s('API 网关', 'API gateway'), ok: health.value?.ok },
  { name: s('数据库', 'Database'), ok: health.value?.database },
  { name: s('计费队列', 'Billing queue'), ok: health.value?.billing_outbox?.ok }
])
</script>
<template>
  <article class="studio-panel health-card">
    <div class="panel-heading">
      <h3>{{ s('服务状态', 'Service status') }}</h3>
      <button class="text-button" :disabled="loading" @click="refresh">
        {{ s('刷新', 'Refresh') }} ↻
      </button>
    </div>
    <div v-for="row in rows" :key="row.name" class="health-row">
      <span>{{ row.name }}</span
      ><span
        :class="['health-value', row.ok === true ? 'good' : row.ok === false ? 'bad' : 'muted']"
        ><i></i
        >{{
          row.ok === true
            ? s('正常', 'Operational')
            : row.ok === false
              ? s('异常', 'Degraded')
              : s('待确认', 'Unknown')
        }}</span
      >
    </div>
    <div class="health-foot">
      <span>{{
        error
          ? s('连接检查失败', 'Connection check failed')
          : latency !== null
            ? `${latency} ms`
            : s('正在检查', 'Checking…')
      }}</span>
    </div>
    <span v-if="checkedAt" class="sr-only">{{ checkedAt.toISOString() }}</span>
  </article>
</template>
