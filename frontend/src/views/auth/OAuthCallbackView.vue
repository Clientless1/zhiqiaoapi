<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import {
  confirmOAuthLink,
  exchangeOAuthTicket,
  requestLoginVerificationCode,
  safeAuthRedirect
} from '../../api/auth'
import BrandLogo from '../../components/common/BrandLogo.vue'
import { useSiteBrand } from '../../composables/useSiteBrand'
import { useStudio } from '../../composables/useStudio'
import { useAuthStore } from '../../stores/auth'
import { useLocale } from '../../composables/useLocale'
import { ApiError } from '../../utils/errors'
import '../../styles/auth-gateway.css'

const { s } = useStudio(),
  { siteName } = useSiteBrand(),
  auth = useAuthStore(),
  router = useRouter()
const error = ref(''),
  processing = ref(true)
const connectTicket = ref(''),
  email = ref(''),
  password = ref(''),
  code = ref('')
const method = ref<'password' | 'email'>('password'),
  busy = ref(false),
  sending = ref(false),
  countdown = ref(0)
const { locale } = useLocale()
let redirect: string | null = null
let timer: ReturnType<typeof setInterval> | undefined
function selectMethod(value: 'password' | 'email') {
  method.value = value
  error.value = ''
}
onBeforeUnmount(() => {
  if (timer) clearInterval(timer)
})
async function sendCode() {
  if (sending.value || countdown.value) return
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email.value.trim())) {
    error.value = s('请输入已注册的邮箱。', 'Enter your registered email.')
    return
  }
  sending.value = true
  error.value = ''
  try {
    await requestLoginVerificationCode(email.value.trim(), locale.value)
    countdown.value = 60
    timer = setInterval(() => {
      if (--countdown.value <= 0 && timer) clearInterval(timer)
    }, 1000)
  } catch {
    error.value = s(
      '邮件暂未发送成功，请稍后重试或使用密码。',
      'Email delivery failed. Try again later or use your password.'
    )
  } finally {
    sending.value = false
  }
}
async function connectAccount() {
  if (busy.value) return
  busy.value = true
  error.value = ''
  try {
    const response = await confirmOAuthLink({
      ticket: connectTicket.value,
      email: email.value.trim(),
      ...(method.value === 'password'
        ? { password: password.value }
        : { verification_code: code.value.trim() })
    })
    auth.setToken(response.token, response.role, response.requires_password_change === true)
    connectTicket.value = ''
    await router.replace(safeAuthRedirect(response.role, redirect, auth.requiresPasswordChange))
  } catch (err) {
    error.value =
      err instanceof ApiError &&
      ['oauth_identity_already_linked', 'password_change_required'].includes(err.code || '')
        ? message(err.code || '')
        : s(
            '验证未通过或授权已过期，请检查邮箱和凭据，或重新发起登录。',
            'Verification failed or authorization expired. Check your email and credentials, or restart sign in.'
          )
  } finally {
    busy.value = false
  }
}
function message(code: string) {
  const messages: Record<string, [string, string]> = {
    oauth_account_exists: [
      '该邮箱已有账户，请先用密码或邮箱验证码登录，再到「个人设置 → 登录方式」绑定。',
      'Sign in with your password or email code, then connect this provider in Settings → Sign in methods.'
    ],
    account_not_registered: [
      '尚未注册，请先创建账户后再登录。',
      'Create an account before signing in.'
    ],
    oauth_cancelled: [
      '你已取消授权，可以重新登录。',
      'Authorization was cancelled. You can try signing in again.'
    ],
    oauth_provider_unavailable: [
      '暂时无法连接第三方登录服务，请重新尝试。',
      'The sign in provider is temporarily unavailable. Please try again.'
    ],
    password_change_required: [
      '请先用原账户登录并修改初始密码，再绑定第三方账号。',
      'Sign in to your existing account and change its initial password before connecting this provider.'
    ],
    oauth_verified_email_required: [
      '请先在第三方平台验证你的邮箱，再重试。',
      'Verify your email with the provider and try again.'
    ],
    account_pending_approval: [
      '账户已提交，等待管理员审核。',
      'Your account is awaiting approval.'
    ],
    registration_closed: ['本站暂未开放注册。', 'Registration is currently closed.'],
    oauth_not_configured: ['该登录方式尚未启用。', 'This sign in provider is not enabled.'],
    oauth_identity_already_linked: [
      '该第三方账号已被绑定，或当前账户已绑定其他同平台账号。',
      'This identity or sign in provider is already connected to an account.'
    ]
  }
  const text = messages[code]
  return text
    ? s(text[0], text[1])
    : s('登录未完成或已过期，请重新登录。', 'Sign in failed or expired. Please try again.')
}
onMounted(async () => {
  const params = new URLSearchParams(window.location.hash.replace(/^#/, ''))
  const ticket = params.get('ticket'),
    connect = params.get('connect'),
    failure = params.get('error'),
    linked = params.get('linked') === 'true'
  // Remove the short lived ticket before network requests or further navigation.
  window.history.replaceState(window.history.state, '', window.location.pathname)
  try {
    redirect = sessionStorage.getItem('zhiqiao_oauth_redirect')
    sessionStorage.removeItem('zhiqiao_oauth_redirect')
  } catch {
    /* Sign in can finish without a stored redirect. */
  }
  if (connect && /^[A-Za-z0-9]{64}$/.test(connect) && !failure) {
    connectTicket.value = connect
    processing.value = false
    return
  }
  if (failure || !ticket || !/^[A-Za-z0-9]{64}$/.test(ticket)) {
    error.value = message(failure || 'oauth_failed')
    processing.value = false
    return
  }
  try {
    const response = await exchangeOAuthTicket(ticket)
    auth.setToken(response.token, response.role, response.requires_password_change === true)
    await router.replace(
      safeAuthRedirect(
        response.role,
        linked ? '/home/settings?linked=true' : redirect,
        auth.requiresPasswordChange
      )
    )
  } catch (err) {
    error.value = message(err instanceof ApiError ? err.code || '' : '')
    processing.value = false
  }
})
</script>
<template>
  <main class="auth-gateway">
    <header class="auth-gateway-nav">
      <RouterLink to="/" class="auth-gateway-brand" :aria-label="siteName"
        ><BrandLogo /><span>{{ siteName }}</span></RouterLink
      >
    </header>
    <section class="auth-gateway-main">
      <div class="auth-gateway-form auth-callback">
        <span v-if="processing" class="auth-progress" aria-hidden="true"></span>
        <div class="auth-gateway-heading">
          <h1>
            {{
              processing
                ? s('正在完成登录', 'Completing sign in')
                : connectTicket
                  ? s('连接已有账户', 'Connect your account')
                  : s('登录未完成', 'Sign in incomplete')
            }}
          </h1>
        </div>
        <p v-if="!connectTicket" role="status">
          {{
            processing
              ? s('请稍候，即将进入工作台。', 'Please wait while we open your workspace.')
              : error
          }}
        </p>
        <template v-if="connectTicket">
          <p>
            {{
              s(
                '第三方授权已完成。验证一次原账户，即可绑定并登录。',
                'Authorization is complete. Verify your existing account once to connect and sign in.'
              )
            }}
          </p>
          <div
            class="auth-entry-tabs"
            role="tablist"
            :aria-label="s('验证方式', 'Verification method')"
          >
            <button
              type="button"
              role="tab"
              :aria-selected="method === 'password'"
              :class="{ active: method === 'password' }"
              @click="selectMethod('password')"
            >
              {{ s('账户密码', 'Password') }}
            </button>
            <button
              type="button"
              role="tab"
              :aria-selected="method === 'email'"
              :class="{ active: method === 'email' }"
              @click="selectMethod('email')"
            >
              {{ s('邮箱验证码', 'Email code') }}
            </button>
          </div>
          <form @submit.prevent="connectAccount">
            <label class="auth-gateway-field"
              ><span>{{ s('已注册邮箱', 'Registered email') }}</span
              ><input
                v-model="email"
                name="email"
                type="email"
                :placeholder="
                  s('与第三方账户相同的已注册邮箱', 'Registered email matching your provider')
                "
                autocomplete="username"
                required
                :disabled="busy"
            /></label>
            <label v-if="method === 'password'" class="auth-gateway-field"
              ><span>{{ s('账户密码', 'Account password') }}</span
              ><input
                v-model="password"
                type="password"
                autocomplete="current-password"
                required
                :disabled="busy"
            /></label>
            <label v-else class="auth-gateway-field"
              ><span>{{ s('邮箱验证码', 'Email code') }}</span
              ><span class="auth-code-input"
                ><input
                  v-model="code"
                  inputmode="numeric"
                  pattern="[0-9]{6}"
                  maxlength="6"
                  autocomplete="one-time-code"
                  required
                  :disabled="busy"
                /><button
                  type="button"
                  :disabled="sending || countdown > 0 || busy"
                  @click="sendCode"
                >
                  {{
                    sending
                      ? s('发送中…', 'Sending…')
                      : countdown
                        ? `${countdown}s`
                        : s('获取验证码', 'Send code')
                  }}
                </button></span
              ></label
            >
            <p v-if="error" class="auth-gateway-feedback" role="alert">{{ error }}</p>
            <button type="submit" class="auth-submit" :disabled="busy">
              {{ busy ? s('验证中…', 'Verifying…') : s('验证并绑定登录', 'Connect and sign in') }}
            </button>
          </form>
        </template>
        <RouterLink v-if="!processing" to="/login"
          >{{ s('返回登录', 'Back to sign in') }} →</RouterLink
        >
      </div>
    </section>
  </main>
</template>

<style scoped>
.auth-entry-tabs {
  display: flex;
  gap: 4px;
  padding: 4px;
  margin: 24px 0;
  background: #f3f5f9;
  border-radius: 10px;
}
.auth-entry-tabs button {
  flex: 1;
  padding: 9px 12px;
  border: 0;
  border-radius: 7px;
  background: transparent;
  color: #64748b;
  cursor: pointer;
}
.auth-entry-tabs button.active {
  background: white;
  color: #243b7a;
  box-shadow: 0 1px 4px #14234b14;
}
</style>
