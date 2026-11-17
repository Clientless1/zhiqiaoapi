import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPageAutoRefresh } from './usePageAutoRefresh'

describe('page auto refresh', () => {
  let page: EventTarget & { visibilityState: string }
  let browser: EventTarget
  const controllers: ReturnType<typeof createPageAutoRefresh>[] = []

  beforeEach(() => {
    vi.useFakeTimers()
    page = Object.assign(new EventTarget(), { visibilityState: 'visible' })
    browser = new EventTarget()
    vi.stubGlobal('document', page)
    vi.stubGlobal('window', browser)
  })

  afterEach(() => {
    controllers.splice(0).forEach((controller) => controller.stop())
    vi.useRealTimers()
    vi.unstubAllGlobals()
  })

  function setup(refresh = vi.fn(async () => {}), canRefresh = () => true) {
    const controller = createPageAutoRefresh(refresh, { intervalMs: 15000, canRefresh })
    controllers.push(controller)
    controller.start()
    return controller
  }

  it('refreshes periodically without duplicating the initial page request', async () => {
    const refresh = vi.fn(async () => {})
    setup(refresh)
    expect(refresh).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(15000)
    expect(refresh).toHaveBeenCalledTimes(1)
    await vi.advanceTimersByTimeAsync(15000)
    expect(refresh).toHaveBeenCalledTimes(2)
  })

  it('pauses on hidden pages and refreshes once when the user returns', async () => {
    const refresh = vi.fn(async () => {})
    setup(refresh)
    page.visibilityState = 'hidden'
    page.dispatchEvent(new Event('visibilitychange'))
    await vi.advanceTimersByTimeAsync(60000)
    browser.dispatchEvent(new Event('focus'))
    expect(refresh).not.toHaveBeenCalled()

    page.visibilityState = 'visible'
    page.dispatchEvent(new Event('visibilitychange'))
    browser.dispatchEvent(new Event('focus'))
    await vi.advanceTimersByTimeAsync(0)
    expect(refresh).toHaveBeenCalledTimes(1)
    browser.dispatchEvent(new Event('focus'))
    expect(refresh).toHaveBeenCalledTimes(1)
    await vi.advanceTimersByTimeAsync(1000)
    browser.dispatchEvent(new Event('focus'))
    expect(refresh).toHaveBeenCalledTimes(2)
  })

  it('keeps requests serial even when a request takes longer than the interval', async () => {
    let finish!: () => void
    const refresh = vi.fn(() => new Promise<void>((resolve) => (finish = resolve)))
    const state = setup(refresh)
    await vi.advanceTimersByTimeAsync(15000)
    expect(state.refreshing.value).toBe(true)
    await vi.advanceTimersByTimeAsync(60000)
    browser.dispatchEvent(new Event('focus'))
    expect(refresh).toHaveBeenCalledTimes(1)
    finish()
    await vi.advanceTimersByTimeAsync(15000)
    expect(refresh).toHaveBeenCalledTimes(2)
  })

  it('leaves foreground requests and edit dialogs alone, then resumes', async () => {
    let blocked = true
    const refresh = vi.fn(async () => {})
    setup(refresh, () => !blocked)
    await vi.advanceTimersByTimeAsync(30000)
    browser.dispatchEvent(new Event('focus'))
    expect(refresh).not.toHaveBeenCalled()
    blocked = false
    await vi.advanceTimersByTimeAsync(15000)
    expect(refresh).toHaveBeenCalledTimes(1)
  })

  it('continues after a network failure', async () => {
    const refresh = vi.fn(async () => {})
    refresh.mockRejectedValueOnce(new Error('offline'))
    const state = setup(refresh)
    await vi.advanceTimersByTimeAsync(15000)
    expect(state.refreshing.value).toBe(false)
    await vi.advanceTimersByTimeAsync(15000)
    expect(refresh).toHaveBeenCalledTimes(2)
  })

  it('removes listeners and timers when leaving, including an in-flight refresh', async () => {
    let finish!: () => void
    const refresh = vi.fn(() => new Promise<void>((resolve) => (finish = resolve)))
    const state = setup(refresh)
    await vi.advanceTimersByTimeAsync(15000)
    state.stop()
    finish()
    await vi.advanceTimersByTimeAsync(60000)
    browser.dispatchEvent(new Event('focus'))
    page.dispatchEvent(new Event('visibilitychange'))
    expect(refresh).toHaveBeenCalledTimes(1)
    expect(vi.getTimerCount()).toBe(0)
  })
})
