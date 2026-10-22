import { adminRequest } from './request'
import type { ModelPackage, CommunityInfo } from './commerce'

export type AdminPackage = ModelPackage & { enabled: boolean; configured_models?: string[] }
export type CouponCampaign = {
  id: number
  code: string
  name: string
  amount_micros: number
  min_spend_micros: number
  max_claims: number
  claimed_count: number
  expires_at: string | null
  enabled: boolean
}
export type AdminRedemption = {
  id: number
  code: string
  amount_micros: number
  package_id: number | null
  expires_at: string | null
  redeemed_at: string | null
}
export type AdminNotification = {
  id: number
  title: string
  content: string
  user_id: number | null
  published: boolean
  created_at: string
}
export type PackageInput = {
  name: string
  description: string
  family: string
  models: string[]
  price_micros: number
  duration_days: number
  enabled: boolean
  service_enabled: boolean
}
export const listAdminPackages = () => adminRequest<AdminPackage[]>('/api/admin/packages')
export const listAdminCoupons = () => adminRequest<CouponCampaign[]>('/api/admin/coupons')
export const listAdminRedemptions = () =>
  adminRequest<AdminRedemption[]>('/api/admin/redemption-codes')
export const listAdminNotifications = () =>
  adminRequest<AdminNotification[]>('/api/admin/notifications')
export const getAdminCommunity = () => adminRequest<CommunityInfo>('/api/admin/community')
export const saveAdminCommunity = (data: CommunityInfo) =>
  adminRequest<CommunityInfo>('/api/admin/community', { method: 'PUT', body: JSON.stringify(data) })
export function saveAdminPackage(data: PackageInput, id?: number) {
  return adminRequest<{ ok: boolean; id: number }>(
    id ? `/api/admin/packages/${id}` : '/api/admin/packages',
    { method: id ? 'PUT' : 'POST', body: JSON.stringify(data) }
  )
}
export function createAdminCoupon(data: Omit<CouponCampaign, 'id' | 'claimed_count' | 'enabled'>) {
  return adminRequest<{ ok: boolean; id: number }>('/api/admin/coupons', {
    method: 'POST',
    body: JSON.stringify(data)
  })
}
export function createAdminRedemption(data: Omit<AdminRedemption, 'id' | 'redeemed_at'>) {
  return adminRequest<{ ok: boolean; id: number }>('/api/admin/redemption-codes', {
    method: 'POST',
    body: JSON.stringify(data)
  })
}
export function publishAdminNotification(data: {
  title: string
  content: string
  user_id: number | null
}) {
  return adminRequest<{ ok: boolean; id: number }>('/api/admin/notifications', {
    method: 'POST',
    body: JSON.stringify(data)
  })
}
export function deleteAdminNotification(id: number) {
  return adminRequest<{ ok: boolean }>(`/api/admin/notifications/${id}`, { method: 'DELETE' })
}
