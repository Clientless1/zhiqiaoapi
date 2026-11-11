import type { ModelOffer } from '../api/catalog'
import { isChatModel } from './catalog'
import { preferredPlaygroundProtocol } from './playground'

export type CCSwitchApp = 'claude' | 'codex' | 'opencode'

export type CCSwitchProvider = {
  app: CCSwitchApp
  name: string
  origin: string
  key: string
  model: string
  enabled?: boolean
}

export function getCCSwitchModels(app: CCSwitchApp, allowedModels: string[], offers: ModelOffer[]) {
  return [...new Set(allowedModels)].filter((model) => {
    if (!isChatModel(model, offers)) return false
    const name = model.toLowerCase().split('/').pop() ?? ''
    if (app === 'claude') return /^claude/.test(name)
    if (app === 'codex') return /^(gpt-|o[1-9]|chatgpt|codex)/.test(name)
    return true
  })
}

// Official protocol: farion1231/cc-switch, docs/guides/deeplink-site-integration-guide-zh.md.
// Keep credentials in the local protocol link; never send them to an external generator.
export function buildCCSwitchLink(provider: CCSwitchProvider) {
  const origin = provider.origin.replace(/\/+$/, '')
  const endpoint = `${origin}${provider.app === 'claude' ? '' : '/v1'}`
  const params = new URLSearchParams({
    resource: 'provider',
    app: provider.app,
    name: provider.name,
    endpoint,
    apiKey: provider.key,
    apiFormat:
      provider.app === 'claude'
        ? 'anthropic'
        : provider.app === 'codex'
          ? 'openai_responses'
          : preferredPlaygroundProtocol(provider.model) === 'responses'
            ? 'openai_responses'
            : 'openai_chat',
    model: provider.model,
    homepage: origin,
    iconUrl: `${origin}/icons/zhiqiao-favicon-f745f88f.png`,
    enabled: String(provider.enabled ?? true)
  })
  if (provider.app === 'claude') {
    // Every Claude Code fallback must stay inside the key's selected model scope.
    for (const alias of ['haikuModel', 'sonnetModel', 'opusModel'])
      params.set(alias, provider.model)
  }
  if (provider.app === 'codex') {
    const bytes = new TextEncoder().encode(buildClientConfig(provider))
    params.set('configFormat', 'json')
    params.set('config', btoa(String.fromCharCode(...bytes)))
  }
  return `ccswitch://v1/import?${params}`
}

export function buildClientConfig(provider: CCSwitchProvider) {
  const origin = provider.origin.replace(/\/+$/, '')
  if (provider.app === 'claude') {
    return JSON.stringify(
      {
        env: {
          ANTHROPIC_BASE_URL: origin,
          ANTHROPIC_AUTH_TOKEN: provider.key,
          ANTHROPIC_MODEL: provider.model,
          ANTHROPIC_DEFAULT_HAIKU_MODEL: provider.model,
          ANTHROPIC_DEFAULT_SONNET_MODEL: provider.model,
          ANTHROPIC_DEFAULT_OPUS_MODEL: provider.model,
          ANTHROPIC_REASONING_MODEL: provider.model,
          ANTHROPIC_CUSTOM_MODEL_OPTION: provider.model
        }
      },
      null,
      2
    )
  }
  if (provider.app === 'codex') {
    // Bind the relay key to this provider so a shared Codex login cache cannot
    // send another provider's credentials to the relay after switching.
    const toml = `model_provider = "zhiqiao"\nmodel = ${JSON.stringify(provider.model)}\n\n[model_providers.zhiqiao]\nname = ${JSON.stringify(provider.name)}\nbase_url = ${JSON.stringify(`${origin}/v1`)}\nwire_api = "responses"\nrequires_openai_auth = false\nexperimental_bearer_token = ${JSON.stringify(provider.key)}\n`
    return JSON.stringify({ auth: { OPENAI_API_KEY: provider.key }, config: toml }, null, 2)
  }
  return JSON.stringify(
    {
      $schema: 'https://opencode.ai/config.json',
      provider: {
        zhiqiao: {
          npm:
            preferredPlaygroundProtocol(provider.model) === 'responses'
              ? '@ai-sdk/openai'
              : '@ai-sdk/openai-compatible',
          name: provider.name,
          options: { baseURL: `${origin}/v1`, apiKey: provider.key },
          models: { [provider.model]: { name: provider.model } }
        }
      },
      model: `zhiqiao/${provider.model}`
    },
    null,
    2
  )
}
