import { describe, expect, it } from 'vitest'
import { resolveApiOrigin } from './apiOrigin'
import { buildCCSwitchLink, buildClientConfig } from './ccswitch'

describe('separate API entry point', () => {
  const website = 'https://portal.example.com'
  it('falls back to the website for unset or unsafe destinations', () => {
    for (const value of [
      undefined,
      null,
      '',
      'invalid',
      'http://api.example.com',
      'https://user:pass@api.example.com',
      'https://api.example.com/v1',
      'https://api.example.com/?key=secret',
      'https://api.example.com/#fragment'
    ])
      expect(resolveApiOrigin(value, website)).toBe(website)
  })
  it('exports the HTTPS direct endpoint for all supported clients', () => {
    const origin = resolveApiOrigin('https://192.0.2.1/', website)
    for (const app of ['claude', 'codex', 'opencode'] as const) {
      const provider = { app, name: 'Relay', origin, key: 'sk-example', model: 'test-model' }
      const link = new URL(buildCCSwitchLink(provider))
      expect(link.searchParams.get('endpoint')).toBe(
        `https://192.0.2.1${app === 'claude' ? '' : '/v1'}`
      )
      expect(buildClientConfig(provider)).toContain('https://192.0.2.1')
      expect(buildClientConfig(provider)).not.toContain(website)
    }
  })
})
