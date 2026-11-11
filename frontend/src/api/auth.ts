import { adminRequest, publicRequest, userRequest } from './request'

export type LoginRole = 'admin' | 'user'
export type LoginResponse = {
  token: string
  role: LoginRole
  requires_password_change?: boolean
}

export function login(username: string, password: string) {
  return publicRequest<LoginResponse>('/api/login', {
    method: 'POST',
    body: JSON.stringify({ username, password })
  })
}

export function emailLogin(email: string, verificationCode: string) {
  return publicRequest<LoginResponse>('/api/email-login', {
    method: 'POST',
    body: JSON.stringify({ email, verification_code: verificationCode })
  })
}

export function register(
  email: string,
  password: string,
  verificationCode: string,
  acceptTerms: boolean
) {
  return publicRequest<LoginResponse>('/api/register', {
    method: 'POST',
    body: JSON.stringify({
      email,
      password,
      verification_code: verificationCode,
      accept_terms: acceptTerms
    })
  })
}

export function requestLoginVerificationCode(
  email: string,
  locale: string,
  purpose: 'login' | 'register' = 'login'
) {
  return publicRequest<{ ok: boolean }>('/api/login-verification-codes', {
    method: 'POST',
    body: JSON.stringify({ email, locale, purpose })
  })
}

export type OAuthProvider = 'github' | 'google'
export type OAuthProviders = Record<OAuthProvider, boolean>
export type OAuthProviderSetting = {
  enabled: boolean
  client_id: string
  client_secret_set: boolean
  callback_url: string
}
export type OAuthSettings = Record<OAuthProvider, OAuthProviderSetting>
export type OAuthSettingsInput = Record<
  OAuthProvider,
  {
    enabled: boolean
    client_id: string
    client_secret?: string
    clear_client_secret: boolean
  }
>

export function getOAuthProviders() {
  return publicRequest<OAuthProviders>('/api/auth/providers')
}

export type LinkedOAuthProvider = { provider: OAuthProvider; enabled: boolean; linked: boolean }

export function getLinkedOAuthProviders() {
  return userRequest<LinkedOAuthProvider[]>('/api/user/oauth/identities')
}

export function linkOAuthProvider(provider: OAuthProvider) {
  return userRequest<{ url: string }>(`/api/user/oauth/${provider}/link`, {
    method: 'POST',
    credentials: 'same-origin'
  })
}

export function exchangeOAuthTicket(ticket: string) {
  return publicRequest<LoginResponse>('/api/auth/oauth/exchange', {
    method: 'POST',
    credentials: 'same-origin',
    body: JSON.stringify({ ticket })
  })
}

export function getOAuthSettings() {
  return adminRequest<OAuthSettings>('/api/admin/settings/oauth')
}

export function saveOAuthSettings(settings: OAuthSettingsInput) {
  return adminRequest<OAuthSettings>('/api/admin/settings/oauth', {
    method: 'POST',
    body: JSON.stringify(settings)
  })
}

export function safeAuthRedirect(
  role: LoginRole,
  redirect: unknown,
  requiresPasswordChange = false
) {
  const root = role === 'admin' ? '/admin' : '/home'
  let target = role === 'admin' ? '/admin/overview' : '/home/overview'
  if (
    typeof redirect === 'string' &&
    redirect.startsWith('/') &&
    !redirect.startsWith('//') &&
    ![...redirect].some((character) => character.charCodeAt(0) < 32 || character === '\\')
  ) {
    try {
      const parsed = new URL(redirect, 'https://auth.local')
      if (
        parsed.origin === 'https://auth.local' &&
        (parsed.pathname === root || parsed.pathname.startsWith(root + '/'))
      ) {
        target = parsed.pathname + parsed.search + parsed.hash
      }
    } catch {
      /* Invalid redirects fall back to the account's console. */
    }
  }
  return role === 'user' && requiresPasswordChange
    ? `/change-password?redirect=${encodeURIComponent(target)}`
    : target
}

export function requestPasswordReset(email: string, locale: string) {
  return publicRequest<{ ok: boolean }>('/api/password-reset-requests', {
    method: 'POST',
    body: JSON.stringify({ email, locale })
  })
}

export function resetPassword(token: string, password: string) {
  return publicRequest<{ ok: boolean }>('/api/password-reset', {
    method: 'POST',
    body: JSON.stringify({ token, password })
  })
}
