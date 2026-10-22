import { adminRequest } from './request'

export type ProcurementQuote = {
  channel_id: number
  model: string
  input_cost_micros: number | null
  output_cost_micros: number | null
  cache_read_cost_micros: number | null
  cache_write_cost_micros: number | null
  unit_cost_micros: number | null
  monthly_budget_micros: number | null
  supplier: string
  notes: string
  updated_at?: string
}

export const getProcurementQuotes = () => adminRequest<ProcurementQuote[]>('/api/admin/procurement')

export const saveProcurementQuote = (quote: ProcurementQuote) =>
  adminRequest<ProcurementQuote>('/api/admin/procurement', {
    method: 'POST',
    body: JSON.stringify(quote)
  })
