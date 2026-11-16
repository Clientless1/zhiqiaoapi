import { describe, expect, it } from 'vitest'
import { imageTiersPayload, videoTiersPayload, newVideoTier } from './mediaPricing'
import { estimateOffer, mediaQuoteRows } from './catalog'
import type { ModelOffer } from '../api/catalog'
const image: ModelOffer = {
  model: 'flux',
  display_name: 'FLUX',
  provider: 'blackforestlabs',
  provider_name: 'FLUX',
  billing_meter: 'image',
  input_price_micros: 0,
  output_price_micros: 0,
  video_price_tiers: [],
  capabilities: {},
  protocols: ['openai'],
  image_price_tiers: [
    { size: '1k', quality: '*', unit_price_micros: 12345 },
    { size: '2k', quality: 'high', unit_price_micros: 56789 }
  ]
}
describe('media price configuration', () => {
  it('preserves fractional image prices and rejects aliases of the same rule', () => {
    expect(imageTiersPayload([{ size: '2K', quality: 'high', unitPrice: 0.012345 }])).toEqual([
      { size: '2k', quality: 'high', unit_price_micros: 12345 }
    ])
    expect(() =>
      imageTiersPayload([
        { size: '2k', quality: '*', unitPrice: 1 },
        { size: '2048', quality: '*', unitPrice: 2 }
      ])
    ).toThrow()
    for (const value of [0, NaN, Infinity, -1, 0.00000001])
      expect(() => imageTiersPayload([{ size: '*', quality: '*', unitPrice: value }])).toThrow()
  })
  it('serializes custom video tiers without dropping resolutions or pair prices', () => {
    const tier = {
      ...newVideoTier(),
      resolutionsText: '480p, 720p',
      inputWithoutVideoUnit: 0.012345,
      inputWithVideoUnit: 0.034567,
      singlePrice: false
    }
    const payload = videoTiersPayload({ videoBillingMode: 'per_second', videoPriceTiers: [tier] })
    expect(payload[0]).toMatchObject({
      resolutions: ['480p', '720p'],
      input_without_video_unit_micros: 12345,
      input_with_video_unit_micros: 34567
    })
    expect(() =>
      videoTiersPayload({
        videoBillingMode: 'per_second',
        videoPriceTiers: [tier, { ...tier, resolutionsText: '720' }]
      })
    ).toThrow()
    expect(() =>
      videoTiersPayload({
        videoBillingMode: 'per_second',
        videoPriceTiers: [{ ...tier, resolutionsText: '' }]
      })
    ).toThrow()
  })
  it('groups only comparable specification prices and skips misleading estimates', () => {
    const rows = mediaQuoteRows([
      image,
      { ...image, image_price_tiers: [{ size: '1k', quality: '*', unit_price_micros: 23456 }] }
    ])
    expect(rows).toHaveLength(2)
    expect(rows.find((r) => r.spec === '1k')).toMatchObject({
      min: 12345,
      max: 23456,
      unit: 'image'
    })
    expect(rows.find((r) => r.spec === '2k')).toMatchObject({ min: 56789, max: 56789 })
    expect(estimateOffer({ ...image, unit_price_micros: 1 }, 0, 0, 3)).toBeNull()
    const video = {
      ...image,
      billing_meter: 'video' as const,
      video_billing_mode: 'per_second' as const,
      video_price_tiers: [
        {
          resolutions: ['480p'],
          input_without_video_unit_micros: 100,
          input_with_video_unit_micros: 200
        }
      ]
    }
    expect(
      mediaQuoteRows([
        video,
        {
          ...video,
          video_billing_mode: 'official_token',
          video_price_tiers: [
            { resolutions: ['480p'], input_without_video_micros: 300, input_with_video_micros: 300 }
          ]
        }
      ])
    ).toHaveLength(3)
  })
})
