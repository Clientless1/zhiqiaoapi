import { describe, expect, it } from 'vitest'
import type { ModelOffer } from '../api/catalog'
import { groupModelOffers } from './catalog'
import { textModelRequest } from './modelExample'

const offer: ModelOffer = {
  model: 'gpt-5',
  display_name: 'GPT-5',
  provider: 'openai',
  provider_name: 'OpenAI',
  billing_meter: 'token',
  input_price_micros: 1000000,
  output_price_micros: 4000000,
  video_price_tiers: [],
  capabilities: {},
  protocols: ['openai']
}

describe('text model request examples', () => {
  it('uses Responses input for an OAuth-only offer', () => {
    expect(textModelRequest({ ...offer, protocols: ['openai_oauth'] })).toEqual({
      path: '/v1/responses',
      headers: { Authorization: 'Bearer YOUR_API_KEY' },
      body: { model: 'gpt-5', input: 'Hello!' }
    })
  })

  it('keeps the Messages payload and required headers for Anthropic-only offers', () => {
    expect(
      textModelRequest({ ...offer, model: 'claude-sonnet-4-6', protocols: ['anthropic'] })
    ).toEqual({
      path: '/v1/messages',
      headers: {
        'x-api-key': 'YOUR_API_KEY',
        'anthropic-version': '2023-06-01'
      },
      body: {
        model: 'claude-sonnet-4-6',
        max_tokens: 1024,
        messages: [{ role: 'user', content: 'Hello!' }]
      }
    })
  })

  it('uses a supported protocol when a single offer has mixed protocols', () => {
    expect(textModelRequest({ ...offer, protocols: ['openai_oauth', 'openai'] })).toMatchObject({
      path: '/v1/chat/completions',
      body: { messages: [{ role: 'user', content: 'Hello!' }] }
    })
    expect(textModelRequest({ ...offer, protocols: ['openai_oauth', 'anthropic'] })).toMatchObject({
      path: '/v1/messages',
      headers: { 'x-api-key': 'YOUR_API_KEY' }
    })
  })

  it('uses each selected offer instead of a grouped card protocol union', () => {
    const [card] = groupModelOffers([
      { ...offer, protocols: ['openai_oauth'] },
      { ...offer, protocols: ['anthropic'] }
    ])
    expect(card!.protocols).toEqual(['anthropic', 'openai_oauth'])
    expect(textModelRequest(card!.offers[0]!).path).toBe('/v1/responses')
    expect(textModelRequest(card!.offers[1]!).path).toBe('/v1/messages')
  })

  it('uses provider fallback for older offers with no protocol list', () => {
    expect(textModelRequest({ ...offer, provider: 'openai_oauth', protocols: [] }).path).toBe(
      '/v1/responses'
    )
    expect(textModelRequest({ ...offer, provider: 'anthropic', protocols: [] }).path).toBe(
      '/v1/messages'
    )
    expect(textModelRequest({ ...offer, protocols: [] }).path).toBe('/v1/chat/completions')
  })
})
