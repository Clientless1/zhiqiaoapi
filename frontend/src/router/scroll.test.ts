import { afterEach, describe, expect, it, vi } from 'vitest'
import { START_LOCATION } from 'vue-router'
import { scrollBehavior } from './scroll'

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('route fragment scrolling', () => {
  it('does not inspect OAuth credentials as section anchors', () => {
    const getElementById = vi.fn()
    vi.stubGlobal('document', { getElementById })
    for (const fragment of ['connect=fixture', 'ticket=fixture', 'error=oauth_cancelled']) {
      expect(
        scrollBehavior(
          { ...START_LOCATION, name: 'oauthCallback', hash: `#${fragment}` },
          START_LOCATION,
          null
        )
      ).toEqual({ top: 0 })
    }
    expect(getElementById).not.toHaveBeenCalled()
  })

  it('finds encoded IDs without interpreting CSS syntax', async () => {
    const element = { id: '接口[1]' }
    const getElementById = vi.fn().mockReturnValue(element)
    vi.stubGlobal('document', { getElementById })
    expect(
      await scrollBehavior(
        { ...START_LOCATION, hash: '#%E6%8E%A5%E5%8F%A3%5B1%5D' },
        START_LOCATION,
        null
      )
    ).toEqual({ el: element, top: 96, behavior: 'smooth' })
    expect(getElementById).toHaveBeenCalledWith('接口[1]')
  })

  it('waits for async sections and falls back for absent or malformed anchors', async () => {
    vi.useFakeTimers()
    vi.stubGlobal('window', { setTimeout })
    const element = { id: 'examples' }
    const getElementById = vi.fn().mockReturnValueOnce(null).mockReturnValue(element)
    vi.stubGlobal('document', { getElementById })
    const pending = scrollBehavior({ ...START_LOCATION, hash: '#examples' }, START_LOCATION, null)
    await vi.runAllTimersAsync()
    expect(await pending).toEqual({ el: element, top: 96, behavior: 'smooth' })
    getElementById.mockReturnValue(null)
    const missing = scrollBehavior({ ...START_LOCATION, hash: '#bad%escape' }, START_LOCATION, null)
    await vi.runAllTimersAsync()
    expect(await missing).toEqual({ top: 0 })
  })

  it('restores history positions', () => {
    const saved = { left: 10, top: 120 }
    expect(scrollBehavior(START_LOCATION, START_LOCATION, saved)).toBe(saved)
  })
})
