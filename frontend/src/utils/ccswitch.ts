export type CCSwitchApp = 'claude' | 'codex' | 'opencode'

export type CCSwitchProvider = {
  app: CCSwitchApp
  name: string
  origin: string
  key: string
  model: string
}

// Official protocol: farion1231/cc-switch, docs/guides/deeplink-site-integration-guide-zh.md.
// Keep credentials in the local protocol link; never send them to an external generator.
export function buildCCSwitchLink(provider: CCSwitchProvider) {
  const endpoint = `${provider.origin.replace(/\/$/, '')}${provider.app === 'claude' ? '' : '/v1'}`
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
          : 'openai_chat',
    model: provider.model,
    homepage: provider.origin
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
  if (provider.app === 'claude') {
    return JSON.stringify(
      {
        env: {
          ANTHROPIC_BASE_URL: provider.origin,
          ANTHROPIC_AUTH_TOKEN: provider.key,
          ANTHROPIC_MODEL: provider.model,
          ANTHROPIC_DEFAULT_HAIKU_MODEL: provider.model,
          ANTHROPIC_DEFAULT_SONNET_MODEL: provider.model,
          ANTHROPIC_DEFAULT_OPUS_MODEL: provider.model
        }
      },
      null,
      2
    )
  }
  if (provider.app === 'codex') {
    // Bind the relay key to this provider so a shared Codex login cache cannot
    // send another provider's credentials to the relay after switching.
    const toml = `model_provider = "zhiqiao"\nmodel = ${JSON.stringify(provider.model)}\n\n[model_providers.zhiqiao]\nname = ${JSON.stringify(provider.name)}\nbase_url = ${JSON.stringify(`${provider.origin.replace(/\/$/, '')}/v1`)}\nwire_api = "responses"\nrequires_openai_auth = false\nexperimental_bearer_token = ${JSON.stringify(provider.key)}\n`
    return JSON.stringify({ auth: { OPENAI_API_KEY: provider.key }, config: toml }, null, 2)
  }
  return JSON.stringify(
    {
      $schema: 'https://opencode.ai/config.json',
      provider: {
        zhiqiao: {
          npm: '@ai-sdk/openai-compatible',
          name: provider.name,
          options: { baseURL: `${provider.origin}/v1`, apiKey: provider.key },
          models: { [provider.model]: { name: provider.model } }
        }
      },
      model: `zhiqiao/${provider.model}`
    },
    null,
    2
  )
}
