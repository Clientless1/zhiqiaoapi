import type { Channel, ChannelProvider, EndpointProtocol, ProviderRecord } from '../types/admin'

export function inferModelProvider(models: string[]): string | null {
  const rules: [string, RegExp][] = [
    ['anthropic', /^(?:claude[-.]|kmt-opus-)/],
    ['openai', /^(?:gpt-|chatgpt-|o[134](?:-|$)|dall-e-|sora-|whisper-)/],
    ['google', /^(?:gemini-|gemma-|imagen-|veo-)/],
    ['deepseek', /^deepseek[-_]/],
    ['qwen', /^(?:qwen|qwq)[-\d]/],
    ['moonshot', /^(?:kimi-|moonshot-)/],
    ['zhipu', /^(?:glm-|cogview-|cogvideo)/],
    ['doubao', /^(?:doubao-|seedream-|seedance-|dreamina-)/],
    ['minimax', /^(?:minimax-|abab|hailuo-)/]
  ]
  const providers = models.map((model) => {
    const normalized = model.trim().toLowerCase().split('/').pop() ?? ''
    return rules.find(([, pattern]) => pattern.test(normalized))?.[0] ?? null
  })
  return providers.length > 0 && providers.every((provider) => provider === providers[0])
    ? (providers[0] ?? null)
    : null
}

export function channelDisplayProvider(channel: Channel) {
  if (channel.provider !== 'openai' || channel.use_credentials) return channel.provider
  const models = channel.endpoints.flatMap((endpoint) => endpoint.models)
  return inferModelProvider(models) ?? channel.provider
}

export function channelDisplayName(channel: Channel, options: ChannelProviderOption[]) {
  const provider = channelDisplayProvider(channel)
  if (
    provider !== channel.provider &&
    /^(openai(?:\s*兼容)?|chatgpt)$/i.test(channel.name.trim())
  ) {
    return options.find((option) => option.value === provider)?.label ?? channel.name
  }
  return channel.name
}

export type ChannelProviderOption = {
  value: ChannelProvider
  label: string
  defaultName: string
  defaultModels?: string[]
  defaultEndpoints: Record<
    EndpointProtocol,
    {
      baseUrl: string
    }
  >
}

export const comfyUIOption: ChannelProviderOption = {
  value: 'comfyui',
  label: 'ComfyUI',
  defaultName: 'ComfyUI',
  defaultEndpoints: emptyDefaultEndpoints()
}

export function providerToOption(provider: ProviderRecord): ChannelProviderOption {
  if (isComfyUIProvider(provider.code)) {
    return comfyUIOption
  }

  const defaultEndpoints = emptyDefaultEndpoints()
  for (const endpoint of provider.default_endpoints) {
    defaultEndpoints[endpoint.protocol] = {
      baseUrl: endpoint.base_url
    }
  }

  return {
    value: provider.code,
    label: provider.display_name,
    defaultName: provider.display_name,
    defaultModels: provider.default_models ?? [],
    defaultEndpoints
  }
}

export function sortProviderOptionsForDisplay(providers: ChannelProviderOption[]) {
  return moveComfyUiLast(providers, (provider) => provider.value)
}

export function sortProvidersForDisplay(providers: ProviderRecord[]) {
  return moveComfyUiLast(providers, (provider) => provider.code)
}

export function findProviderOption(provider: ChannelProvider, providers: ChannelProviderOption[]) {
  return providers.find((option) => option.value === provider)
}

export function formatModels(models: string[], allModelsLabel = '全部模型') {
  return models.length > 0 ? models.join(', ') : allModelsLabel
}

export function endpointLabel(protocol: EndpointProtocol) {
  if (protocol === 'openai_oauth') return 'OpenAI 令牌'
  return protocol === 'openai' ? 'OpenAI' : 'Anthropic'
}

function emptyDefaultEndpoints(): ChannelProviderOption['defaultEndpoints'] {
  return {
    openai: {
      baseUrl: ''
    },
    openai_oauth: {
      baseUrl: ''
    },
    anthropic: {
      baseUrl: ''
    }
  }
}

function moveComfyUiLast<T>(providers: T[], providerCode: (provider: T) => ChannelProvider) {
  return [
    ...providers.filter((provider) => !isComfyUIProvider(providerCode(provider))),
    ...providers.filter((provider) => isComfyUIProvider(providerCode(provider)))
  ]
}

function isComfyUIProvider(provider: ChannelProvider) {
  return provider.toLowerCase() === comfyUIOption.value
}

export function splitCommaList(value: string) {
  return Array.from(
    new Set(
      value
        .split(/[,，、;；\s]+/)
        .map((item) => item.trim())
        .filter(Boolean)
    )
  )
}
