import type { ImagePriceTier, VideoPriceTier } from '../types/channel'
import type {
  ChannelImagePriceTierForm,
  ChannelVideoPriceTierForm,
  ChannelPriceForm
} from '../types/channelPricing'

export function normalizeMediaSpec(value: string) {
  const key = value.trim().toLowerCase().replace(/[ _]/g, '').replace(/×/g, 'x')
  return (
    (
      {
        '1024': '1k',
        '1024p': '1k',
        '2048': '2k',
        '2048p': '2k',
        '4096': '4k',
        '4096p': '4k'
      } as Record<string, string>
    )[key] ?? key
  )
}
function positiveMicros(value: number) {
  const micros = Math.round(value * 1_000_000)
  if (!Number.isSafeInteger(micros) || micros <= 0)
    throw new Error(
      '每个价格必须大于零，最多 6 位小数 / Each price must be positive, up to 6 decimals'
    )
  return micros
}
export function imageTiersPayload(tiers: ChannelImagePriceTierForm[]): ImagePriceTier[] {
  if (tiers.length > 100) throw new Error('最多 100 个档位 / Maximum 100 tiers')
  const seen = new Set<string>()
  return tiers.map((tier) => {
    const size = normalizeMediaSpec(tier.size),
      quality = normalizeMediaSpec(tier.quality)
    const key = JSON.stringify([size, quality])
    if (!size || !quality || size.length > 64 || quality.length > 64 || seen.has(key))
      throw new Error(
        '图像规格和画质不能为空或重复 / Image size and quality must be unique and non-empty'
      )
    seen.add(key)
    return { size, quality, unit_price_micros: positiveMicros(tier.unitPrice) }
  })
}
export function newVideoTier(): ChannelVideoPriceTierForm {
  return {
    resolutionsText: '',
    inputWithVideo: 0,
    inputWithoutVideo: 0,
    inputWithVideoUnit: 0,
    inputWithoutVideoUnit: 0,
    estimatedTokensPerSecond: 15000,
    singlePrice: true
  }
}
export function videoTiersPayload(
  form: Pick<ChannelPriceForm, 'videoPriceTiers' | 'videoBillingMode'>
): VideoPriceTier[] {
  if (!form.videoBillingMode || !form.videoPriceTiers.length || form.videoPriceTiers.length > 100)
    throw new Error('请设置视频计费方式和价格档位 / Set video billing mode and tiers')
  const seen = new Set<string>()
  return form.videoPriceTiers.map((tier) => {
    const resolutions = tier.resolutionsText.split(/[,，]/).map((s) => s.trim().toLowerCase())
    for (const resolution of resolutions) {
      const compact = resolution.replace(/[ _]/g, '')
      const key = /4k|2160/.test(compact)
        ? '4k'
        : /1080/.test(compact)
          ? '1080p'
          : /720/.test(compact)
            ? '720p'
            : /480/.test(compact)
              ? '480p'
              : compact
      if (!key || key.length > 64 || seen.has(key))
        throw new Error('视频分辨率不能为空或重复 / Video resolutions must be unique and non-empty')
      seen.add(key)
    }
    const token = form.videoBillingMode === 'official_token'
    const without = positiveMicros(token ? tier.inputWithoutVideo : tier.inputWithoutVideoUnit)
    const withVideo = tier.singlePrice
      ? without
      : positiveMicros(token ? tier.inputWithVideo : tier.inputWithVideoUnit)
    if (
      token &&
      (!Number.isSafeInteger(tier.estimatedTokensPerSecond) || tier.estimatedTokensPerSecond <= 0)
    )
      throw new Error('请设置每秒预估 tokens / Set estimated tokens per second')
    return {
      resolutions,
      input_without_video_micros: token ? without : null,
      input_with_video_micros: token ? withVideo : null,
      estimated_tokens_per_second: token ? tier.estimatedTokensPerSecond : null,
      input_without_video_unit_micros: token ? null : without,
      input_with_video_unit_micros: token ? null : withVideo
    }
  })
}
