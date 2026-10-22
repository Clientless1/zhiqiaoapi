import { publicRequest, userRequest } from './request'
import type { BillingMeter, VideoBillingMode, VideoPriceTier } from '../types/channel'

export type ModelOffer = {
  model: string
  display_name: string
  provider: string
  provider_name: string
  billing_meter: BillingMeter
  input_price_micros: number
  output_price_micros: number
  cache_read_price_micros?: number | null
  cache_write_price_micros?: number | null
  unit_price_micros?: number | null
  video_billing_mode?: VideoBillingMode | null
  video_price_tiers: VideoPriceTier[]
  capabilities: Record<string, unknown>
  protocols: string[]
}

export type ModelCatalog = { currency: string; personalized: boolean; models: ModelOffer[] }

export function getModelCatalog(personalized = false) {
  return personalized
    ? userRequest<ModelCatalog>('/api/user/models')
    : publicRequest<ModelCatalog>('/api/public/models')
}
