import { describe, expect, it } from 'vitest'
import { qqGroupUrl } from './community'

describe('QQ group links', () => {
  it('preserves official invitation parameters', () => {
    const url = 'https://qm.qq.com/q/Example?authKey=example%2Bkey'
    expect(qqGroupUrl(url)).toBe(url)
    expect(qqGroupUrl('https://qun.qq.com/universal-share/share?ac=1')).toContain('qun.qq.com')
  })

  it('rejects deceptive hosts and non official or executable links', () => {
    for (const value of [
      undefined,
      '',
      'javascript:alert(1)',
      'http://qm.qq.com/a',
      'https://qq.com.evil.test/a',
      'https://evilqq.com/a',
      'https://user@qm.qq.com/a',
      'https://qm.qq.com:8443/a'
    ]) {
      expect(qqGroupUrl(value)).toBe('')
    }
  })
})
