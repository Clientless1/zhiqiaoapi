import { onBeforeUnmount, onMounted, ref } from 'vue'

type AutoRefreshOptions = {
  intervalMs: number
  canRefresh: () => boolean
}

export function usePageAutoRefresh(refresh: () => Promise<unknown>, options: AutoRefreshOptions) {
  const state = createPageAutoRefresh(refresh, options)
  onMounted(state.start)
  onBeforeUnmount(state.stop)
  return { refreshing: state.refreshing }
}

export function createPageAutoRefresh(
  refresh: () => Promise<unknown>,
  options: AutoRefreshOptions
) {
  const refreshing = ref(false)
  let timer: ReturnType<typeof setTimeout> | undefined
  let active = false
  let lastStarted = -Infinity

  function clearTimer() {
    clearTimeout(timer)
    timer = undefined
  }

  function schedule() {
    clearTimer()
    if (active && document.visibilityState === 'visible') {
      timer = setTimeout(() => void run(), options.intervalMs)
    }
  }

  async function run() {
    if (!active || document.visibilityState !== 'visible' || refreshing.value) return
    if (!options.canRefresh() || Date.now() - lastStarted < 1000) {
      schedule()
      return
    }
    clearTimer()
    lastStarted = Date.now()
    refreshing.value = true
    try {
      await refresh()
    } catch {
      // The data loader reports errors; a failed refresh must not stop future attempts.
    } finally {
      refreshing.value = false
      schedule()
    }
  }

  function handleVisibilityChange() {
    if (document.visibilityState === 'visible') void run()
    else clearTimer()
  }

  function handleFocus() {
    void run()
  }

  function start() {
    if (active) return
    active = true
    document.addEventListener('visibilitychange', handleVisibilityChange)
    window.addEventListener('focus', handleFocus)
    schedule()
  }

  function stop() {
    active = false
    clearTimer()
    document.removeEventListener('visibilitychange', handleVisibilityChange)
    window.removeEventListener('focus', handleFocus)
  }

  return { refreshing, start, stop }
}
