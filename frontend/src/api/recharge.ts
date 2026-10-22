import { userRequest } from './request'
import { pageQuery, type PageQuery, type CommercePage } from './commerce'

export type PaymentOrder = {
  id: string
  order_no: number
  user_id: number
  provider: string
  provider_order_id?: string | null
  status: 'pending' | 'paid' | 'failed' | 'canceled' | 'expired'
  currency: string
  amount_micros: number
  payable_amount_minor: number
  checkout_url?: string | null
  return_url?: string | null
  paid_at?: string | null
  created_at: string
  updated_at: string
  expires_at: string
}

export type PayType = 'alipay' | 'wxpay'

export function getRechargeOrders(query: PageQuery & { order_no?: string }, init?: RequestInit) {
  return userRequest<CommercePage<PaymentOrder>>(
    `/api/user/recharge/orders?${pageQuery(query)}`,
    init
  )
}

export const cancelRechargeOrder = (id: string) =>
  userRequest<PaymentOrder>(`/api/user/recharge/orders/${encodeURIComponent(id)}/cancel`, {
    method: 'POST'
  })

export function createRechargeOrder(amountMicros: number, payType: PayType, returnUrl: string) {
  return userRequest<{ order: PaymentOrder; checkout_url?: string | null }>(
    '/api/user/recharge/orders',
    {
      method: 'POST',
      body: JSON.stringify({
        provider: 'zpay',
        amount_micros: amountMicros,
        pay_type: payType,
        return_url: returnUrl
      })
    }
  )
}
