import { publicRequest } from './request'

export type ModelHealthStatus = 'operational' | 'degraded' | 'outage' | 'unknown'

export type ModelHealthBucket = {
  start_at: string
  status: ModelHealthStatus
  sample_count: number
  success_count: number
  latency_ms: number | null
}

export type ModelHealth = {
  provider: string
  model: string
  status: ModelHealthStatus
  availability_percent: number | null
  /** Mean completed successful probe duration; not time to first token. */
  latency_ms: number | null
  /** Null until the probe records actual generated tokens. */
  tokens_per_second: number | null
  sample_count: number
  last_checked_at: string | null
  history: ModelHealthBucket[]
}

export type ModelHealthSnapshot = {
  window_hours: number
  bucket_minutes: number
  generated_at: string
  models: ModelHealth[]
}

export function getModelHealth() {
  return publicRequest<ModelHealthSnapshot>('/api/public/model-health')
}
