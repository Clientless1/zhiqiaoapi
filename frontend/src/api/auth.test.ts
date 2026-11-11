import { beforeEach, describe, expect, it, vi } from 'vitest'
const { publicRequest, adminRequest } = vi.hoisted(() => ({
  publicRequest: vi.fn(),
  adminRequest: vi.fn()
}))
vi.mock('./request', () => ({ publicRequest, adminRequest }))
import {
  emailLogin,
  exchangeOAuthTicket,
  login,
  register,
  requestLoginVerificationCode,
  safeAuthRedirect
} from './auth'

describe('independent authentication methods', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    publicRequest.mockResolvedValue({ token: 'session', role: 'user' })
  })
  it('sends only the password credential to the password endpoint', async () => {
    await login('person@example.com', 'correct-password')
    const [path, init] = publicRequest.mock.calls[0]!
    expect(path).toBe('/api/login')
    expect(JSON.parse(init.body)).toEqual({
      username: 'person@example.com',
      password: 'correct-password'
    })
    expect(JSON.parse(init.body)).not.toHaveProperty('verification_code')
  })
  it('email login is password independent and registration uses a distinct endpoint', async () => {
    await emailLogin('person@example.com', '123456')
    expect(publicRequest.mock.calls[0]![0]).toBe('/api/email-login')
    expect(JSON.parse(publicRequest.mock.calls[0]![1].body)).not.toHaveProperty('password')
    await register('new@example.com', 'new-password', '654321', true)
    expect(publicRequest.mock.calls[1]![0]).toBe('/api/register')
    expect(JSON.parse(publicRequest.mock.calls[1]![1].body)).toMatchObject({
      accept_terms: true,
      verification_code: '654321'
    })
  })
  it('binds requested verification codes to the intended flow', async () => {
    await requestLoginVerificationCode('person@example.com', 'zh-CN')
    await requestLoginVerificationCode('new@example.com', 'zh-CN', 'register')
    expect(JSON.parse(publicRequest.mock.calls[0]![1].body).purpose).toBe('login')
    expect(JSON.parse(publicRequest.mock.calls[1]![1].body).purpose).toBe('register')
  })
  it('exchanges the single use OAuth ticket in a body with same origin cookies', async () => {
    await exchangeOAuthTicket('a'.repeat(64))
    expect(publicRequest).toHaveBeenCalledWith(
      '/api/auth/oauth/exchange',
      expect.objectContaining({ method: 'POST', credentials: 'same-origin' })
    )
    expect(JSON.parse(publicRequest.mock.calls[0]![1].body).ticket).toBe('a'.repeat(64))
  })
})

describe('authentication redirects', () => {
  it('rejects foreign, malformed and cross role destinations', () => {
    for (const target of [
      'https://evil.example',
      '//evil.example',
      '/admin/users',
      '/homeevil',
      '/home/../admin/users',
      '/home/\\evil',
      '/home/\nredirect'
    ]) {
      expect(safeAuthRedirect('user', target)).toBe('/home/overview')
    }
    expect(safeAuthRedirect('admin', '/home/overview')).toBe('/admin/overview')
  })
  it('preserves safe in-console destinations and enforces required password changes', () => {
    expect(safeAuthRedirect('user', '/home/apikeys?q=example')).toBe('/home/apikeys?q=example')
    expect(safeAuthRedirect('admin', '/admin/settings/auth')).toBe('/admin/settings/auth')
    expect(safeAuthRedirect('user', '/home/apikeys', true)).toBe(
      '/change-password?redirect=%2Fhome%2Fapikeys'
    )
  })
})
