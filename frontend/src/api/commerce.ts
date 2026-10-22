import { publicRequest, userRequest } from './request'

export type ModelPackage = {
  id: number
  name: string
  description: string
  family: string
  price_micros: number
  duration_days: number
  models: string[]
  active: boolean
  enabled: boolean
  service_enabled: boolean
  expires_at?: string | null
}

export type UserCoupon = {
  id: number
  name: string
  amount_micros: number
  min_spend_micros: number
  enabled: boolean
  expires_at?: string | null
  used_at?: string | null
}

export type Redemption = {
  id: number
  code_hint: string
  amount_micros: number
  package_name?: string | null
  created_at: string
}

export type UserOrder = {
  id: string | number
  type: 'package' | 'recharge'
  name: string
  status: string
  amount_micros: number
  created_at: string
  checkout_url?: string | null
  expires_at?: string | null
}

export type UserNotification = {
  id: number
  title: string
  content: string
  created_at: string
  read_at?: string | null
}

export type CommunityInfo = {
  about_title: string
  about_content: string
  qq_group_number?: string | null
  qq_group_url?: string | null
}

export const getPackages = () => userRequest<ModelPackage[]>('/api/user/packages')
export const getCoupons = () => userRequest<UserCoupon[]>('/api/user/coupons')
export type PageQuery = { page: number; page_size: number; type?: string }
export type CommercePage<T> = { items: T[]; total: number; page: number; page_size: number }
export const emptyPage = <T>(): CommercePage<T> => ({ items: [], total: 0, page: 1, page_size: 20 })
export function pageQuery(query: PageQuery) {
  return new URLSearchParams(Object.entries(query).map(([key, value]) => [key, String(value)]))
}
export const getRedemptions = (query: PageQuery) =>
  userRequest<CommercePage<Redemption>>(`/api/user/redemptions?${pageQuery(query)}`)
export const getOrders = (query: PageQuery) =>
  userRequest<CommercePage<UserOrder>>(`/api/user/orders?${pageQuery(query)}`)
export const getNotifications = (query: PageQuery) =>
  userRequest<CommercePage<UserNotification> & { unread_count: number }>(
    `/api/user/notifications?${pageQuery(query)}`
  )
export type LedgerEntry = {
  id: number
  account_name: string
  account_type: string
  model: string | null
  type: string
  reason: string
  amount_micros: number
  balance_after_micros: number | null
  transaction_id: string
  created_at: string
}
export const getLedger = (query: PageQuery) =>
  userRequest<CommercePage<LedgerEntry> & { currency: string }>(
    `/api/user/ledger?${pageQuery(query)}`
  )
export type PurchaseReceipt = {
  ok: boolean
  order_id: string
  request_id: string
  package_id: number
  package_name: string
  coupon_id: number | null
  amount_micros: number
  created_at: string
}
export const getPurchaseReceipt = (requestId: string) =>
  userRequest<PurchaseReceipt>(
    `/api/user/package-orders/by-request/${encodeURIComponent(requestId)}`
  )
export const getCommunity = () => publicRequest<CommunityInfo>('/api/public/community')

export function purchasePackage(id: number, requestId: string, couponId?: number) {
  return userRequest<{ ok: boolean; order_id: string; amount_micros: number }>(
    `/api/user/packages/${id}/purchase`,
    { method: 'POST', body: JSON.stringify({ request_id: requestId, coupon_id: couponId }) }
  )
}

export function claimCoupon(code: string) {
  return userRequest<{ ok: boolean }>('/api/user/coupons/claim', {
    method: 'POST',
    body: JSON.stringify({ code })
  })
}

export function redeemCode(code: string) {
  return userRequest<{ ok: boolean; amount_micros: number; package_name?: string }>(
    '/api/user/redemptions',
    { method: 'POST', body: JSON.stringify({ code }) }
  )
}

export function markNotificationRead(id: number) {
  return userRequest<{ ok: boolean }>(`/api/user/notifications/${id}/read`, {
    method: 'POST',
    body: '{}'
  })
}

export function markAllNotificationsRead() {
  return userRequest<{ ok: boolean }>('/api/user/notifications/read-all', {
    method: 'POST',
    body: '{}'
  })
}
