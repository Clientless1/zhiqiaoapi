import { describe, expect, it, vi } from 'vitest'

vi.mock('element-plus/es/components/message/index', () => ({
  ElMessage: { error: vi.fn() }
}))

import { createAsyncData } from './useAsyncData'
import { ElMessage } from 'element-plus/es/components/message/index'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise
  })
  return { promise, resolve }
}

describe('createAsyncData', () => {
  it('keeps the last good data on silent failure and recovers without repeated notifications', async () => {
    const loader = vi
      .fn()
      .mockResolvedValueOnce('first')
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce('updated')
    const state = createAsyncData(loader, '')
    await state.reload()
    vi.mocked(ElMessage.error).mockClear()

    const refresh = state.reload({ silent: true, notifyOnError: false })
    expect(state.loading.value).toBe(false)
    await refresh
    expect(state.data.value).toBe('first')
    expect(state.error.value).toBe('offline')
    expect(ElMessage.error).not.toHaveBeenCalled()

    await state.reload({ silent: true, notifyOnError: false })
    expect(state.data.value).toBe('updated')
    expect(state.error.value).toBe('')
  })

  it('does not overwrite a mutation reload with an older background response', async () => {
    const background = deferred<string>()
    const foreground = deferred<string>()
    const loader = vi
      .fn()
      .mockReturnValueOnce(background.promise)
      .mockReturnValueOnce(foreground.promise)
    const state = createAsyncData(loader, 'before')
    const backgroundReload = state.reload({ silent: true })
    const foregroundReload = state.reload()
    foreground.resolve('after edit')
    await foregroundReload
    background.resolve('before edit')
    await backgroundReload
    expect(state.data.value).toBe('after edit')
    expect(state.loading.value).toBe(false)
  })

  it('closes foreground loading when a newer silent request is running', async () => {
    const foreground = deferred<string>()
    const silent = deferred<string>()
    const loader = vi
      .fn()
      .mockReturnValueOnce(foreground.promise)
      .mockReturnValueOnce(silent.promise)
    const state = createAsyncData(loader, '')

    const foregroundReload = state.reload()
    const silentReload = state.reload({ silent: true })

    foreground.resolve('stale')
    await foregroundReload
    expect(state.loading.value).toBe(false)
    expect(state.data.value).toBe('')

    silent.resolve('latest')
    await silentReload
    expect(state.data.value).toBe('latest')
    expect(state.loaded.value).toBe(true)
  })
})
