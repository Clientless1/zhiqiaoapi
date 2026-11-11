<script setup lang="ts">
import { computed } from 'vue'
import PublicHeader from '../../components/common/PublicHeader.vue'
import { useRoute } from 'vue-router'
import { useStudio } from '../../composables/useStudio'
import { useServiceHealth } from '../../composables/useServiceHealth'
const { s } = useStudio()
const route = useRoute()
const { health, latency, checkedAt, error, loading, samples, refresh } = useServiceHealth()
const publicPage = computed(() => route.path === '/status')
const rows = computed(() => [
  {
    name: s('API 网关', 'API gateway'),
    desc: s('服务进程的就绪检查', 'Application readiness check'),
    ok: health.value?.ok
  },
  {
    name: s('数据库', 'Database'),
    desc: s('数据库连接与查询检查', 'Database connection and query check'),
    ok: health.value?.database
  },
  {
    name: s('计费队列', 'Billing queue'),
    desc: s('计费写入与待处理任务状态', 'Billing writes and pending tasks'),
    ok: health.value?.billing_outbox?.ok
  },
  ...(health.value?.redis != null
    ? [
        {
          name: 'Redis',
          desc: s('缓存连接检查', 'Cache connectivity check'),
          ok: health.value.redis
        }
      ]
    : [])
])
</script>
<template>
  <div :class="{ 'studio-public': publicPage }">
    <PublicHeader v-if="publicPage" />
    <main class="status-page" :class="{ 'studio-public-main': publicPage }">
      <div class="studio-section-title">
        <div>
          <h1>{{ s('服务状态', 'Service status') }}</h1>
          <p>
            {{ s('每 60 秒自动更新', 'Refreshes every 60 seconds') }}
          </p>
        </div>
        <el-button :loading="loading" @click="refresh"
          >{{ s('立即检查', 'Check now') }} ↻</el-button
        >
      </div>
      <div class="status-banner" :class="{ degraded: !!error || health?.ok === false }">
        <span class="status-beacon"></span>
        <div>
          <h2>
            {{
              loading && !checkedAt
                ? s('正在检查服务', 'Checking services')
                : error
                  ? s('服务连接需要检查', 'Connection needs attention')
                  : health?.ok
                    ? s('核心服务运行正常', 'Core services operational')
                    : s('部分服务异常', 'Some services are degraded')
            }}
          </h2>
          <p>
            {{
              checkedAt
                ? s('最近检查：', 'Last checked: ') + checkedAt.toLocaleString()
                : s('正在获取状态…', 'Fetching status…')
            }}<template v-if="latency !== null"> · {{ latency }} ms</template>
          </p>
        </div>
      </div>
      <article class="studio-panel status-components">
        <div v-for="row in rows" :key="row.name" class="status-component">
          <div>
            <h3>{{ row.name }}</h3>
            <p>{{ row.desc }}</p>
          </div>
          <span
            :class="['health-value', row.ok === true ? 'good' : row.ok === false ? 'bad' : 'muted']"
            ><i></i
            >{{
              row.ok === true
                ? s('正常运行', 'Operational')
                : row.ok === false
                  ? s('需要关注', 'Needs attention')
                  : s('状态未知', 'Unknown')
            }}</span
          >
        </div>
      </article>
      <article class="studio-panel">
        <div class="panel-heading">
          <h3>{{ s('本次访问的检查记录', 'Checks during this visit') }}</h3>
          <span class="muted">{{ samples.length }} / 30</span>
        </div>
        <div class="status-history">
          <span
            v-for="(sample, index) in samples"
            :key="index"
            :class="sample.ok ? 'ok' : 'failed'"
            :title="`${sample.time.toLocaleTimeString()} · ${sample.ok ? 'OK' : 'Failed'} · ${sample.latency} ms`"
          ></span
          ><span v-for="n in 30 - samples.length" :key="'empty' + n" class="unobserved"></span>
        </div>
        <p class="studio-note">
          {{
            s(
              '仅记录本次访问的检查结果。上游模型可用性请在调试台验证。',
              'Only actual checks during this visit are shown. This is not historical uptime. Core readiness does not guarantee every upstream model; test specific models in the Playground.'
            )
          }}
        </p>
      </article>
      <div class="status-links">
        <RouterLink to="/models">{{ s('浏览模型', 'Explore models') }} ↗</RouterLink
        ><RouterLink to="/playground">{{ s('打开调试台', 'Open Playground') }} ↗</RouterLink
        ><RouterLink to="/docs">{{ s('接入文档', 'Documentation') }} ↗</RouterLink>
      </div>
    </main>
  </div>
</template>
