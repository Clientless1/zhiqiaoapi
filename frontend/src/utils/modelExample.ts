import type { ModelOffer } from '../api/catalog'

export function textModelRequest(offer: ModelOffer) {
  const protocol =
    ['openai', 'anthropic', 'openai_oauth'].find((item) => offer.protocols.includes(item)) ??
    offer.provider
  if (protocol === 'anthropic') {
    return {
      path: '/v1/messages',
      headers: {
        'x-api-key': 'YOUR_API_KEY',
        'anthropic-version': '2023-06-01'
      },
      body: {
        model: offer.model,
        max_tokens: 1024,
        messages: [{ role: 'user', content: 'Hello!' }]
      }
    }
  }
  return {
    path: protocol === 'openai_oauth' ? '/v1/responses' : '/v1/chat/completions',
    headers: { Authorization: 'Bearer YOUR_API_KEY' },
    body:
      protocol === 'openai_oauth'
        ? { model: offer.model, input: 'Hello!' }
        : { model: offer.model, messages: [{ role: 'user', content: 'Hello!' }] }
  }
}
