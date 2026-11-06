import { describe, expect, it } from 'vitest'
import { buildCCSwitchLink, buildClientConfig, getCCSwitchModels } from './ccswitch'

describe('CC Switch provider import', () => {
  it('uses Responses for GPT models in OpenCode instead of the unsupported chat endpoint', () => {
    const provider = {
      app: 'opencode' as const,
      name: '智桥',
      origin: 'https://api.example.com',
      key: 'sk-example',
      model: 'gpt-6.1-sol'
    }
    expect(new URL(buildCCSwitchLink(provider)).searchParams.get('apiFormat')).toBe(
      'openai_responses'
    )
    expect(JSON.parse(buildClientConfig(provider)).provider.zhiqiao.npm).toBe('@ai-sdk/openai')
  })
  it('keeps client choices inside the key scope and excludes non-chat models', () => {
    const allowed = [
      'gpt-5',
      'anthropic/claude-sonnet-4-6',
      'deepseek-chat',
      'gpt-image-1',
      'text-embedding-3-small',
      'gpt-5'
    ]
    expect(getCCSwitchModels('codex', allowed, [])).toEqual(['gpt-5'])
    expect(getCCSwitchModels('claude', allowed, [])).toEqual(['anthropic/claude-sonnet-4-6'])
    expect(getCCSwitchModels('opencode', allowed, [])).toEqual([
      'gpt-5',
      'anthropic/claude-sonnet-4-6',
      'deepseek-chat'
    ])
    expect(getCCSwitchModels('codex', [], [])).toEqual([])
  })

  it('uses the catalog to reject a model explicitly marked as non-chat', () => {
    expect(
      getCCSwitchModels(
        'codex',
        ['gpt-5'],
        [
          {
            model: 'gpt-5',
            display_name: 'GPT',
            provider: 'openai',
            provider_name: 'OpenAI',
            billing_meter: 'token',
            input_price_micros: 0,
            output_price_micros: 0,
            video_price_tiers: [],
            protocols: ['openai'],
            capabilities: { chat: false }
          }
        ]
      )
    ).toEqual([])
  })

  it('normalizes OpenCode endpoints when copying or importing a trailing-slash origin', () => {
    const provider = {
      app: 'opencode' as const,
      name: '智桥',
      origin: 'https://api.example.com/',
      key: 'sk-example',
      model: 'deepseek-chat'
    }
    expect(JSON.parse(buildClientConfig(provider)).provider.zhiqiao.options.baseURL).toBe(
      'https://api.example.com/v1'
    )
    expect(new URL(buildCCSwitchLink(provider)).searchParams.get('endpoint')).toBe(
      'https://api.example.com/v1'
    )
  })

  it('encodes credentials locally and uses the official provider protocol', () => {
    const link = new URL(
      buildCCSwitchLink({
        app: 'codex',
        name: '智桥 API',
        origin: 'https://api.example.com',
        key: 'sk-a+b&c?d',
        model: 'gpt-5'
      })
    )
    expect(link.protocol).toBe('ccswitch:')
    expect(link.hostname).toBe('v1')
    expect(link.pathname).toBe('/import')
    expect(link.searchParams.get('resource')).toBe('provider')
    expect(link.searchParams.get('apiKey')).toBe('sk-a+b&c?d')
    expect(link.searchParams.get('endpoint')).toBe('https://api.example.com/v1')
    expect(link.searchParams.get('apiFormat')).toBe('openai_responses')
  })

  it('pins all Claude Code aliases to the selected allowed model', () => {
    const link = new URL(
      buildCCSwitchLink({
        app: 'claude',
        name: 'Claude',
        origin: 'https://api.example.com/',
        key: 'sk-example',
        model: 'claude-sonnet-4-6'
      })
    )
    expect(link.searchParams.get('endpoint')).toBe('https://api.example.com')
    for (const alias of ['model', 'haikuModel', 'sonnetModel', 'opusModel'])
      expect(link.searchParams.get(alias)).toBe('claude-sonnet-4-6')
  })

  it('binds Codex authentication to the selected provider when importing or copying', () => {
    const provider = {
      app: 'codex' as const,
      name: '智桥 API',
      origin: 'http://127.0.0.1:5175/',
      key: 'sk-a+b&c?d',
      model: 'gpt-6.1-sol'
    }
    const copied = JSON.parse(buildClientConfig(provider))
    const link = new URL(buildCCSwitchLink(provider))
    const encoded = link.searchParams.get('config')!
    const imported = JSON.parse(
      new TextDecoder().decode(Uint8Array.from(atob(encoded), (char) => char.charCodeAt(0)))
    )

    expect(link.searchParams.get('configFormat')).toBe('json')
    expect(imported).toEqual(copied)
    expect(imported.auth.OPENAI_API_KEY).toBe(provider.key)
    expect(imported.config).toContain('name = "智桥 API"')
    expect(imported.config).toContain('base_url = "http://127.0.0.1:5175/v1"')
    expect(imported.config).toContain('requires_openai_auth = false')
    expect(imported.config).toContain(`experimental_bearer_token = "${provider.key}"`)
  })

  it('copies an OpenCode config that uses only the selected model', () => {
    const config = JSON.parse(
      buildClientConfig({
        app: 'opencode',
        name: '智桥',
        origin: 'https://api.example.com',
        key: 'sk-example',
        model: 'deepseek-chat'
      })
    )
    expect(config.model).toBe('zhiqiao/deepseek-chat')
    expect(Object.keys(config.provider.zhiqiao.models)).toEqual(['deepseek-chat'])
  })
})
