import { describe, expect, it } from 'vitest'
import type { UserCoupon } from '../api/commerce'
import { couponState } from './coupon'

const coupon: UserCoupon = {
  id: 1,
  name: 'Discount',
  amount_micros: 100,
  min_spend_micros: 0,
  enabled: true
}

describe('coupon eligibility', () => {
  it('rejects disabled and already used coupons', () => {
    expect(couponState({ ...coupon, enabled: false })).toBe('disabled')
    expect(couponState({ ...coupon, used_at: '2026-01-01T00:00:00Z' })).toBe('used')
  })

  it('expires a coupon at its exact expiry time', () => {
    const expiry = '2026-10-22T00:00:00Z'
    expect(couponState({ ...coupon, expires_at: expiry }, Date.parse(expiry))).toBe('expired')
    expect(couponState({ ...coupon, expires_at: expiry }, Date.parse(expiry) - 1)).toBe('available')
  })
})
