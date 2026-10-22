import { ApiError } from './errors'

export type PurchaseIntent = {
  requestId: string
  packageId: number
  couponId?: number
  packageName?: string
  priceMicros?: number
  payableMicros?: number
}

export async function confirmPurchaseIntent(
  intent: PurchaseIntent,
  lookup: (requestId: string) => Promise<unknown>,
  submit: (intent: PurchaseIntent) => Promise<unknown>
) {
  try {
    return await lookup(intent.requestId)
  } catch (error) {
    if (!(error instanceof ApiError && error.status === 404)) throw error
  }
  return submit(intent)
}

type IntentStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>
const storageKey = (userId: number) => `zhiqiao:purchase-intent:user:${userId}`

export function loadPurchaseIntent(
  userId: number,
  storage: IntentStorage = localStorage
): PurchaseIntent | null {
  if (!Number.isSafeInteger(userId) || userId <= 0) throw new Error('Purchase account is not ready')
  const saved = storage.getItem(storageKey(userId))
  if (!saved) return null
  const value: unknown = JSON.parse(saved)
  if (!value || typeof value !== 'object') throw new Error('Invalid saved purchase')
  const item = value as Partial<PurchaseIntent>
  if (
    typeof item.requestId !== 'string' ||
    !item.requestId ||
    !Number.isSafeInteger(item.packageId) ||
    Number(item.packageId) <= 0 ||
    (item.couponId != null && !Number.isSafeInteger(item.couponId))
  )
    throw new Error('Invalid saved purchase')
  return item as PurchaseIntent
}

export function savePurchaseIntent(
  userId: number,
  intent: PurchaseIntent | null,
  storage: IntentStorage = localStorage
) {
  if (!Number.isSafeInteger(userId) || userId <= 0) throw new Error('Purchase account is not ready')
  if (intent) storage.setItem(storageKey(userId), JSON.stringify(intent))
  else storage.removeItem(storageKey(userId))
}

export function purchaseIntent(
  current: PurchaseIntent | null,
  packageId: number,
  couponId?: number,
  createId: () => string = () => crypto.randomUUID()
): PurchaseIntent {
  // Retrying the same checkout must reuse the original receipt identifier.
  if (current?.packageId === packageId && current.couponId === couponId) return current
  return { requestId: createId(), packageId, couponId }
}
