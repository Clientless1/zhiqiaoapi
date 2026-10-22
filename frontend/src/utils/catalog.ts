import type { ModelOffer } from '../api/catalog'

export type ModelCard = ModelOffer & { key: string; offers: ModelOffer[] }

export function modelCategory(
  model: ModelOffer
): 'token' | 'image' | 'video' | 'audio' | 'embedding' | 'moderation' {
  const capabilities = model.capabilities
  if (capabilities.embeddings === true || /embed|bge-|e5-|gte-/i.test(model.model))
    return 'embedding'
  if (capabilities.moderations === true || /moderation/i.test(model.model)) return 'moderation'
  if (capabilities.image_generation === true) return 'image'
  if (capabilities.video_generation === true) return 'video'
  if (capabilities.audio_speech === true || capabilities.audio_transcription === true)
    return 'audio'
  const name = model.model.toLowerCase().split('/').pop() ?? ''
  if (/^(gpt-image|dall-e|flux|stable-diffusion|sdxl|imagen|seedream)/.test(name)) return 'image'
  if (/^(sora|veo|kling|runway|gen4-|wan|seedance|hailuo)/.test(name)) return 'video'
  if (/tts|whisper|transcri|asr/.test(name) || /^(eleven_|cosyvoice)/.test(name)) return 'audio'
  return model.billing_meter
}

export function isChatModel(model: string, offers: ModelOffer[]): boolean {
  if (/embed|moderation|bge-|e5-|gte-/i.test(model)) return false
  const matches = offers.filter((offer) => offer.model === model)
  if (matches.length)
    return (
      matches.some(
        (offer) => modelCategory(offer) === 'token' && offer.capabilities.chat === true
      ) ||
      matches.some(
        (offer) =>
          modelCategory(offer) === 'token' &&
          offer.capabilities.chat !== false &&
          offer.capabilities.embeddings !== true &&
          offer.capabilities.moderations !== true
      )
    )
  return !/(image|dall-e|flux|sora|veo|tts|whisper|speech|video|imagen|cosyvoice|eleven_|runway|kling|seedance|seedream)/i.test(
    model
  )
}

export function isSpeechModel(model: ModelOffer): boolean {
  return (
    model.capabilities.audio_speech === true || /tts|speech|^eleven_|^cosyvoice/i.test(model.model)
  )
}

// Keep each complete offer intact: do not combine one channel's input price
// with another channel's output price for an estimate.
export function groupModelOffers(offers: ModelOffer[]): ModelCard[] {
  const groups = new Map<string, ModelCard>()
  for (const offer of offers) {
    const key = JSON.stringify([offer.provider, offer.model, offer.billing_meter])
    const existing = groups.get(key)
    if (!existing) groups.set(key, { ...offer, key, offers: [offer] })
    else if (!existing.offers.some((item) => JSON.stringify(item) === JSON.stringify(offer))) {
      existing.offers.push(offer)
      existing.protocols = [...new Set([...existing.protocols, ...offer.protocols])].sort()
    }
  }
  return [...groups.values()]
}

export function priceRange(values: (number | null | undefined)[]): [number, number] | null {
  const valid = values.filter((v): v is number => v != null && Number.isFinite(v) && v >= 0)
  return valid.length ? [Math.min(...valid), Math.max(...valid)] : null
}

export function estimateOffer(
  offer: ModelOffer,
  input: number,
  output: number,
  units: number
): number | null {
  if ([input, output, units].some((v) => !Number.isFinite(v) || v < 0)) return null
  if (offer.billing_meter === 'video') return null
  if (offer.billing_meter === 'token') {
    return (
      Math.ceil((input * offer.input_price_micros) / 1_000_000) +
      Math.ceil((output * offer.output_price_micros) / 1_000_000)
    )
  }
  return offer.unit_price_micros == null ? null : Math.round(units * offer.unit_price_micros)
}

export function procurementScenario(
  inputCost: number | null | undefined,
  outputCost: number | null | undefined,
  inputMillions: number,
  outputMillions: number
): number | null {
  if (![inputMillions, outputMillions].every((v) => Number.isFinite(v) && v >= 0)) return null
  if ((inputMillions > 0 && inputCost == null) || (outputMillions > 0 && outputCost == null))
    return null
  return Math.round((inputCost ?? 0) * inputMillions + (outputCost ?? 0) * outputMillions)
}
