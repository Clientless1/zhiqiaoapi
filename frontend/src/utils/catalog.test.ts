import { describe, expect, it } from 'vitest'
import type { ModelOffer } from '../api/catalog'
import {
  estimateOffer,
  groupModelOffers,
  isSpeechModel,
  isChatModel,
  modelCategory,
  priceRange,
  procurementScenario
} from './catalog'

const offer: ModelOffer = {
  model: 'model',
  display_name: 'Model',
  provider: 'openai',
  provider_name: 'OpenAI',
  billing_meter: 'token',
  input_price_micros: 1000000,
  output_price_micros: 4000000,
  video_price_tiers: [],
  capabilities: {},
  protocols: ['openai']
}

describe('catalog prices', () => {
  it('separates model capabilities from their billing meter', () => {
    expect(modelCategory({ ...offer, model: 'gpt-image-1' })).toBe('image')
    expect(modelCategory({ ...offer, model: 'vendor/FLUX.1-dev' })).toBe('image')
    expect(modelCategory({ ...offer, model: 'sora-2' })).toBe('video')
    expect(modelCategory({ ...offer, model: 'gpt-4o-mini-tts' })).toBe('audio')
    expect(modelCategory({ ...offer, model: 'qwen-vl-max' })).toBe('token')
    expect(modelCategory({ ...offer, model: 'text-embedding-3-small' })).toBe('embedding')
    expect(modelCategory({ ...offer, model: 'omni-moderation-latest' })).toBe('moderation')
    expect(isSpeechModel({ ...offer, model: 'gpt-4o-mini-tts' })).toBe(true)
    expect(isSpeechModel({ ...offer, model: 'whisper-1' })).toBe(false)
  })
  it('rejects embedding and moderation models from coding clients even with token billing', () => {
    const embeddings = {
      ...offer,
      model: 'text-embedding-3-small',
      capabilities: { embeddings: true }
    }
    const named = { ...offer, model: 'custom-vector', capabilities: { embeddings: true } }
    expect(isChatModel(embeddings.model, [embeddings])).toBe(false)
    expect(isChatModel(named.model, [named])).toBe(false)
    expect(isChatModel('omni-moderation-latest', [offer])).toBe(false)
    expect(isChatModel('gpt-4o', [])).toBe(true)
    expect(isChatModel('claude-sonnet-4-6', [])).toBe(true)
  })
  it('keeps channel offers intact and combines protocol availability', () => {
    const other = {
      ...offer,
      input_price_micros: 3000000,
      output_price_micros: 2000000,
      protocols: ['anthropic']
    }
    const result = groupModelOffers([offer, { ...offer }, other])
    expect(result).toHaveLength(1)
    expect(result[0]!.offers).toHaveLength(2)
    expect(result[0]!.protocols).toEqual(['anthropic', 'openai'])
    expect(estimateOffer(result[0]!.offers[0]!, 1000000, 1000000, 0)).toBe(5000000)
    expect(estimateOffer(result[0]!.offers[1]!, 1000000, 1000000, 0)).toBe(5000000)
  })
  it('separates providers and meters and preserves free prices', () => {
    expect(
      groupModelOffers([
        offer,
        { ...offer, provider: 'other' },
        { ...offer, billing_meter: 'image' }
      ])
    ).toHaveLength(3)
    expect(priceRange([null, undefined, 0, 2000000])).toEqual([0, 2000000])
    expect(priceRange([null, NaN, -1])).toBeNull()
  })
  it('uses actual billing units and rounds each token component up', () => {
    expect(
      estimateOffer({ ...offer, input_price_micros: 1, output_price_micros: 1 }, 1, 1, 0)
    ).toBe(2)
    expect(
      estimateOffer({ ...offer, billing_meter: 'image', unit_price_micros: 40000 }, 0, 0, 3)
    ).toBe(120000)
    expect(estimateOffer({ ...offer, billing_meter: 'video' }, 0, 0, 3)).toBeNull()
    expect(estimateOffer(offer, -1, 0, 0)).toBeNull()
  })
  it('keeps unknown procurement costs distinct from zero', () => {
    expect(procurementScenario(null, 2000000, 10, 2)).toBeNull()
    expect(procurementScenario(0, 2000000, 10, 2)).toBe(4000000)
    expect(procurementScenario(null, 2000000, 0, 2)).toBe(4000000)
  })
})
