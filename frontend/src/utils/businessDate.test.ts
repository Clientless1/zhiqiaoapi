import { describe, expect, it } from 'vitest'
import { businessDays } from './businessDate'

describe('UTC business date axis', () => {
  it('ends at the API business date across month and leap year boundaries', () => {
    expect(businessDays('2024-03-01', 3)).toEqual(['2024-02-28', '2024-02-29', '2024-03-01'])
    expect(businessDays('2026-01-01', 2)).toEqual(['2025-12-31', '2026-01-01'])
  })
  it('waits for a valid business date instead of using the browser local date', () => {
    expect(businessDays('')).toEqual([])
  })
})
