import { ref, onMounted, onBeforeUnmount } from 'vue'
export type Health = {
  ok: boolean
  database: boolean
  redis?: boolean | null
  billing_outbox?: { ok: boolean }
}
export function useServiceHealth() {
  const health = ref<Health | null>(null)
  const latency = ref<number | null>(null)
  const checkedAt = ref<Date | null>(null)
  const error = ref('')
  const loading = ref(false)
  const samples = ref<{ time: Date; ok: boolean; latency: number }[]>([])
  let controller: AbortController | undefined
  let timer: number | undefined
  let disposed = false
  async function refresh() {
    if (loading.value || disposed) return
    loading.value = true
    controller = new AbortController()
    const timeout = window.setTimeout(() => controller?.abort(), 8000)
    const start = performance.now()
    try {
      const response = await fetch('/readyz', { cache: 'no-store', signal: controller.signal })
      const body: unknown = await response.json()
      if (
        !body ||
        typeof body !== 'object' ||
        !('ok' in body) ||
        typeof body.ok !== 'boolean' ||
        !('database' in body) ||
        typeof body.database !== 'boolean'
      )
        throw new Error('Invalid health response')
      if (disposed) return
      health.value = body as Health
      latency.value = Math.round(performance.now() - start)
      error.value = response.ok && health.value.ok ? '' : `HTTP ${response.status}`
    } catch (err) {
      if (disposed) return
      health.value = null
      latency.value = null
      error.value = err instanceof Error ? err.message : 'Connection failed'
    } finally {
      window.clearTimeout(timeout)
      if (!disposed) {
        checkedAt.value = new Date()
        samples.value = [
          ...samples.value,
          {
            time: checkedAt.value,
            ok: health.value?.ok === true && !error.value,
            latency: latency.value ?? 0
          }
        ].slice(-30)
        loading.value = false
      }
    }
  }
  onMounted(() => {
    void refresh()
    timer = window.setInterval(() => {
      if (document.visibilityState === 'visible') void refresh()
    }, 60000)
  })
  onBeforeUnmount(() => {
    disposed = true
    controller?.abort()
    window.clearInterval(timer)
  })
  return { health, latency, checkedAt, error, loading, samples, refresh }
}
