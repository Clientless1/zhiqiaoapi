import { beforeEach, describe, expect, it, vi } from 'vitest'
const { publicRequest, adminRequest } = vi.hoisted(() => ({
  publicRequest: vi.fn(),
  adminRequest: vi.fn()
}))
vi.mock('./request', () => ({ publicRequest, adminRequest }))
import {
  getNavigationSetting,
  isUserPathVisible,
  navigationSetting,
  saveNavigationSetting
} from './navigation'

describe('user menu configuration', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    navigationSetting.value = { hidden_items: [] }
  })
  it('matches hidden pages with query strings and nested paths without hiding admin pages', () => {
    const setting = { hidden_items: ['status' as const] }
    expect(isUserPathVisible('/home/status', setting)).toBe(false)
    expect(isUserPathVisible('/home/status?refresh=1', setting)).toBe(false)
    expect(isUserPathVisible('/home/status/details', setting)).toBe(false)
    expect(isUserPathVisible('/home/status-other', setting)).toBe(true)
    expect(isUserPathVisible('/admin/status', setting)).toBe(true)
    expect(isUserPathVisible('/home/settings', setting)).toBe(true)
  })
  it('updates reactive navigation after a server-confirmed save', async () => {
    adminRequest.mockResolvedValue({ hidden_items: ['status'] })
    await saveNavigationSetting({ hidden_items: ['status'] })
    expect(navigationSetting.value.hidden_items).toEqual(['status'])
    expect(adminRequest).toHaveBeenCalledWith(
      '/api/admin/settings/navigation',
      expect.objectContaining({ method: 'POST' })
    )
  })
  it('does not change visible pages when saving fails', async () => {
    adminRequest.mockRejectedValue(new Error('forbidden'))
    await expect(saveNavigationSetting({ hidden_items: ['status'] })).rejects.toThrow('forbidden')
    expect(navigationSetting.value.hidden_items).toEqual([])
  })
  it('can refresh persisted settings and does not expose its mutable cache', async () => {
    publicRequest.mockResolvedValue({ hidden_items: ['status'] })
    const loaded = await getNavigationSetting(true)
    loaded.hidden_items.length = 0
    expect(navigationSetting.value.hidden_items).toEqual(['status'])
    expect(publicRequest).toHaveBeenCalledWith('/api/public/navigation')
  })
})
