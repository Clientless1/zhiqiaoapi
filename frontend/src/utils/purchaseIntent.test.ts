import { describe, expect, it } from 'vitest'
import {
  purchaseIntent,
  loadPurchaseIntent,
  savePurchaseIntent,
  confirmPurchaseIntent
} from './purchaseIntent'
import { ApiError } from './errors'

function memoryStorage() {
  const values = new Map<string, string>()
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => {
      values.set(key, value)
    },
    removeItem: (key: string) => {
      values.delete(key)
    }
  }
}

describe('package checkout intent', () => {
  it('keeps the same request ID after an unknown or lost response', () => {
    const original = purchaseIntent(null, 5, 8, () => 'first-request')
    const retry = purchaseIntent(original, 5, 8, () => 'must-not-be-used')
    expect(retry).toBe(original)
    expect(retry.requestId).toBe('first-request')
  })

  it('starts a new request when the plan or coupon changes', () => {
    const original = purchaseIntent(null, 5, 8, () => 'original')
    expect(purchaseIntent(original, 5, undefined, () => 'without-coupon').requestId).toBe(
      'without-coupon'
    )
    expect(purchaseIntent(original, 6, 8, () => 'other-plan').requestId).toBe('other-plan')
  })

  it('starts a new purchase after the previous intent is cleared', () => {
    expect(purchaseIntent(null, 5, 8, () => 'new-purchase').requestId).toBe('new-purchase')
  })

  it('recovers a charged order after reload without submitting another charge', async () => {
    const storage = memoryStorage()
    const original = purchaseIntent(null, 5, 8, () => 'receipt-1')
    savePurchaseIntent(1, original, storage)
    let charges = 0
    let receipt: object | null = null
    const lookup = async () => {
      if (!receipt) throw new ApiError('Not found', 404)
      return receipt
    }
    await expect(
      confirmPurchaseIntent(original, lookup, async () => {
        charges += 1
        receipt = { order_id: 'paid' }
        throw new TypeError('Failed to fetch')
      })
    ).rejects.toThrow('Failed to fetch')
    const restored = loadPurchaseIntent(1, storage)!
    expect(restored.requestId).toBe(original.requestId)
    expect(
      await confirmPurchaseIntent(restored, lookup, async () => {
        charges += 1
      })
    ).toEqual(receipt)
    expect(charges).toBe(1)
    expect(loadPurchaseIntent(2, storage)).toBeNull()
    savePurchaseIntent(1, null, storage)
    expect(loadPurchaseIntent(1, storage)).toBeNull()
  })

  it('never creates a purchase when checking the original receipt fails', async () => {
    const original = purchaseIntent(null, 5, undefined, () => 'receipt-2')
    let submitted = false
    await expect(
      confirmPurchaseIntent(
        original,
        async () => {
          throw new ApiError('Unavailable', 503)
        },
        async () => {
          submitted = true
        }
      )
    ).rejects.toThrow('Unavailable')
    expect(submitted).toBe(false)
  })

  it('retries an unfinished purchase with its persisted request ID and coupon', async () => {
    const storage = memoryStorage()
    const original = purchaseIntent(null, 5, 8, () => 'receipt-3')
    savePurchaseIntent(1, original, storage)
    let submitted: unknown
    await confirmPurchaseIntent(
      loadPurchaseIntent(1, storage)!,
      async () => {
        throw new ApiError('Not found', 404)
      },
      async (intent) => {
        submitted = intent
      }
    )
    expect(submitted).toEqual(original)
  })
})
