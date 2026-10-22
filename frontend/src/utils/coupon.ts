import type { UserCoupon } from '../api/commerce'

export function couponState(coupon: UserCoupon, now = Date.now()) {
  if (coupon.used_at) return 'used'
  if (coupon.enabled === false) return 'disabled'
  if (coupon.expires_at && new Date(coupon.expires_at).getTime() <= now) return 'expired'
  return 'available'
}
