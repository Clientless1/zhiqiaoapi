import { ref } from 'vue'
import { adminRequest, publicRequest } from './request'

export const optionalNavigationItems = [
  'models',
  'playground',
  'apikeys',
  'usage',
  'packages',
  'coupons',
  'redeem',
  'orders',
  'ledger',
  'recharge',
  'status',
  'notifications'
] as const
export type OptionalNavigationItem = (typeof optionalNavigationItems)[number]
export type NavigationSetting = { hidden_items: OptionalNavigationItem[] }

export const navigationSetting = ref<NavigationSetting>({ hidden_items: [] })
let loadedAt = 0
let pending: Promise<NavigationSetting> | null = null

export function isUserPathVisible(path: string, setting = navigationSetting.value) {
  const match = /^\/home\/([^/?#]+)(?:[/?#]|$)/.exec(path)
  return !match || !setting.hidden_items.includes(match[1] as OptionalNavigationItem)
}

function apply(setting: NavigationSetting) {
  navigationSetting.value = { hidden_items: [...setting.hidden_items] }
  loadedAt = Date.now()
  return { hidden_items: [...setting.hidden_items] }
}

export function getNavigationSetting(force = false): Promise<NavigationSetting> {
  if (!force && loadedAt && Date.now() - loadedAt < 5000) {
    return Promise.resolve({ hidden_items: [...navigationSetting.value.hidden_items] })
  }
  if (!force && pending) return pending
  const task = publicRequest<NavigationSetting>('/api/public/navigation').then(apply)
  if (force) return task
  pending = task.finally(() => {
    pending = null
  })
  return pending
}

export function getAdminNavigationSetting() {
  return adminRequest<NavigationSetting>('/api/admin/settings/navigation').then(apply)
}

export function saveNavigationSetting(input: NavigationSetting) {
  return adminRequest<NavigationSetting>('/api/admin/settings/navigation', {
    method: 'POST',
    body: JSON.stringify(input)
  }).then(apply)
}
