<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { View, Hide } from '@element-plus/icons-vue'
import {
  login,
  emailLogin,
  register,
  requestLoginVerificationCode,
  getOAuthProviders,
  safeAuthRedirect,
  type OAuthProvider,
  type LoginResponse
} from '../../api/auth'
import { getSetupStatus } from '../../api/policy'
import BrandLogo from '../../components/common/BrandLogo.vue'
import BridgeParticles from '../../components/common/BridgeParticles.vue'
import LocaleToggleButton from '../../components/common/LocaleToggleButton.vue'
import { useAsyncData } from '../../composables/useAsyncData'
import { useLocale } from '../../composables/useLocale'
import { useStudio } from '../../composables/useStudio'
import { useSiteBrand } from '../../composables/useSiteBrand'
import { useAuthStore } from '../../stores/auth'
import { ApiError, isSmtpConfigError, readError } from '../../utils/errors'
import '../../styles/auth-gateway.css'
import '../../styles/auth-entry.css'

const route = useRoute(),
  router = useRouter(),
  auth = useAuthStore()
const { s } = useStudio(),
  { locale } = useLocale(),
  { siteName } = useSiteBrand()
const { data: policy } = useAsyncData(() => getSetupStatus(), null)
const { data: providers, error: providerError } = useAsyncData(() => getOAuthProviders(), null)
const isRegister = computed(() => route.name === 'register')
const registrationOpen = computed(() => policy.value?.registration_enabled !== false)
const mode = ref<'password' | 'email'>('password')
const email = ref(''),
  password = ref(''),
  confirmPassword = ref(''),
  code = ref('')
const accepted = ref(false),
  passwordVisible = ref(false)
const busy = ref(false),
  sending = ref(false),
  error = ref(''),
  countdown = ref(0)
let timer: ReturnType<typeof setInterval> | undefined
const formDisabled = computed(() => busy.value || (isRegister.value && !registrationOpen.value))
const switchLink = computed(() => ({
  path: isRegister.value ? '/login' : '/register',
  query: typeof route.query.redirect === 'string' ? { redirect: route.query.redirect } : {}
}))
const emailValid = () => /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email.value.trim())
function clearCountdown() {
  if (timer) clearInterval(timer)
  timer = undefined
  countdown.value = 0
}
watch([isRegister, mode], () => {
  error.value = ''
  code.value = ''
  password.value = ''
  confirmPassword.value = ''
  passwordVisible.value = false
  accepted.value = false
})
watch(email, () => {
  code.value = ''
})
onBeforeUnmount(clearCountdown)
function readAuthError(err: unknown) {
  if (isSmtpConfigError(err))
    return s(
      '邮件服务暂不可用，请使用密码登录或联系管理员。',
      'Email is unavailable. Use your password or contact support.'
    )
  if (err instanceof ApiError) {
    const messages: Record<string, [string, string]> = {
      invalid_verification_code: [
        '验证码无效或已过期，请重新获取。',
        'The code is invalid or expired. Request a new one.'
      ],
      account_pending_approval: [
        '账户已提交，等待管理员审核。',
        'Your account is awaiting approval.'
      ],
      registration_closed: ['本站暂未开放注册。', 'Registration is currently closed.'],
      account_already_exists: [
        '该邮箱已注册，请返回登录。',
        'This email is registered. Sign in instead.'
      ],
      terms_required: [
        '请先阅读并同意用户协议和隐私政策。',
        'Please accept the terms and privacy policy.'
      ],
      password_min_length: [
        '密码至少需要 8 个字符。',
        'Use at least 8 characters for your password.'
      ],
      login_verification_rate_limited: [
        '请求过于频繁，请稍后再试。',
        'Too many requests. Please try again later.'
      ],
      password_login_rate_limited: [
        '登录尝试过于频繁，请稍后再试。',
        'Too many sign in attempts. Please try again later.'
      ],
      smtp_connection_timed_out: [
        '邮件连接超时，请稍后重试或使用密码登录。',
        'Email delivery timed out. Try again later or sign in with your password.'
      ],
      smtp_test_email_failed: [
        '验证码邮件暂未发送成功，请稍后重试或使用密码登录。',
        'Email delivery failed. Try again later or sign in with your password.'
      ],
      smtp_authentication_failed: [
        '邮件服务验证失败，请联系管理员或使用密码登录。',
        'Email authentication failed. Contact support or use your password.'
      ]
    }
    const message = err.code ? messages[err.code] : undefined
    if (message) return s(message[0], message[1])
    if (err.status === 429)
      return s('请求过于频繁，请稍后再试。', 'Too many requests. Please try again later.')
    if (err.status === 401 || err.status === 403)
      return mode.value === 'email'
        ? s(
            '邮箱或验证码不正确，请确认已注册并重新获取验证码。',
            'Check your registered email and request a fresh code.'
          )
        : s('账户或密码不正确，请重试。', 'The account or password is incorrect.')
  }
  return readError(err)
}
async function complete(response: LoginResponse) {
  auth.setToken(response.token, response.role, response.requires_password_change === true)
  await router.replace(
    safeAuthRedirect(response.role, route.query.redirect, auth.requiresPasswordChange)
  )
}
async function submit() {
  if (formDisabled.value) return
  error.value = ''
  if (!email.value.trim()) {
    error.value = s('请输入邮箱或账户名。', 'Enter your email or account name.')
    return
  }
  if ((isRegister.value || mode.value === 'email') && !emailValid()) {
    error.value = s('请输入有效邮箱。', 'Enter a valid email address.')
    return
  }
  if (isRegister.value && !accepted.value) {
    error.value = s(
      '请阅读并同意用户协议和隐私政策。',
      'Please accept the terms and privacy policy.'
    )
    return
  }
  if (
    (isRegister.value || mode.value === 'password') &&
    (!password.value || (isRegister.value && password.value.length < 8))
  ) {
    error.value = s(
      '请输入密码，注册密码至少 8 位。',
      'Enter your password. New passwords require at least 8 characters.'
    )
    return
  }
  if (isRegister.value && password.value !== confirmPassword.value) {
    error.value = s('两次输入的密码不一致。', 'The passwords do not match.')
    return
  }
  if ((isRegister.value || mode.value === 'email') && !/^\d{6}$/.test(code.value.trim())) {
    error.value = s('请输入 6 位邮箱验证码。', 'Enter the 6 digit email code.')
    return
  }
  busy.value = true
  try {
    const response = isRegister.value
      ? await register(email.value.trim(), password.value, code.value.trim(), accepted.value)
      : mode.value === 'email'
        ? await emailLogin(email.value.trim(), code.value.trim())
        : await login(email.value.trim(), password.value)
    await complete(response)
  } catch (err) {
    error.value = readAuthError(err)
  } finally {
    busy.value = false
  }
}
async function sendCode() {
  if (sending.value || countdown.value || formDisabled.value) return
  error.value = ''
  if (!emailValid()) {
    error.value = s('请先填写有效邮箱。', 'Enter a valid email address first.')
    return
  }
  if (isRegister.value && !accepted.value) {
    error.value = s(
      '请先阅读并同意用户协议和隐私政策。',
      'Please accept the terms and privacy policy first.'
    )
    return
  }
  sending.value = true
  try {
    await requestLoginVerificationCode(
      email.value.trim(),
      locale.value,
      isRegister.value ? 'register' : 'login'
    )
    countdown.value = 60
    if (timer) clearInterval(timer)
    timer = setInterval(() => {
      countdown.value--
      if (countdown.value <= 0) clearCountdown()
    }, 1000)
    ElMessage.success(
      isRegister.value
        ? s(
            '若邮箱可注册，验证码将发送至该邮箱。',
            'If eligible, a code will arrive in your inbox.'
          )
        : s(
            '若邮箱已注册，验证码将发送至该邮箱。',
            'If registered, a code will arrive in your inbox.'
          )
    )
  } catch (err) {
    error.value = readAuthError(err)
  } finally {
    sending.value = false
  }
}
function startOAuth(provider: OAuthProvider) {
  if (!providers.value?.[provider] || formDisabled.value) return
  if (isRegister.value && !accepted.value) {
    error.value = s(
      '请先阅读并同意用户协议和隐私政策。',
      'Please accept the terms and privacy policy first.'
    )
    return
  }
  try {
    const redirect = route.query.redirect
    if (typeof redirect === 'string') sessionStorage.setItem('zhiqiao_oauth_redirect', redirect)
    else sessionStorage.removeItem('zhiqiao_oauth_redirect')
  } catch {
    /* OAuth still works when browser storage is unavailable. */
  }
  const query = new URLSearchParams({ intent: 'register', accept_terms: 'true' })
  window.location.assign(`/api/auth/oauth/${provider}/start?${query.toString()}`)
}
</script>

<template>
  <main class="auth-gateway auth-entry" :class="{ 'is-register': isRegister }">
    <aside class="auth-quiet-panel" aria-label="智桥">
      <div class="auth-particle-art" aria-hidden="true"><BridgeParticles /></div>
      <div class="auth-quiet-caption">
        <h2>
          {{ s('连接想法', 'Ideas, connected.') }}<br /><span>{{
            s('与实现。', 'Possibilities, open.')
          }}</span>
        </h2>
        <p>{{ s('让每一次探索，都向前一步。', 'A little further, with every exploration.') }}</p>
      </div>
    </aside>
    <div class="auth-content-panel">
      <header class="auth-entry-nav">
        <RouterLink to="/" class="auth-gateway-brand auth-mobile-brand" :aria-label="siteName"
          ><BrandLogo /><span>{{ siteName }}</span></RouterLink
        >
        <nav>
          <RouterLink to="/"
            >{{ s('返回首页', 'Back home') }} <span aria-hidden="true">↗</span></RouterLink
          ><LocaleToggleButton />
        </nav>
      </header>
      <section class="auth-gateway-main">
        <div class="auth-form-region">
          <div class="auth-gateway-form">
            <div class="auth-gateway-heading">
              <h1>
                {{
                  isRegister
                    ? s('创建账户', 'Create account')
                    : s('登录账户', 'Sign in to your account')
                }}
              </h1>
            </div>
            <div
              v-if="!isRegister"
              class="auth-method-tabs"
              role="tablist"
              :aria-label="s('登录方式', 'Sign in method')"
            >
              <button
                id="password-tab"
                type="button"
                role="tab"
                :aria-selected="mode === 'password'"
                aria-controls="auth-form"
                :class="{ active: mode === 'password' }"
                @click="mode = 'password'"
              >
                {{ s('密码登录', 'Password') }}
              </button>
              <button
                id="email-tab"
                type="button"
                role="tab"
                :aria-selected="mode === 'email'"
                aria-controls="auth-form"
                :class="{ active: mode === 'email' }"
                @click="mode = 'email'"
              >
                {{ s('邮箱验证码', 'Email code') }}
              </button>
            </div>
            <p v-if="isRegister && !registrationOpen" class="auth-gateway-feedback" role="status">
              {{ s('本站暂未开放注册。', 'Registration is currently closed.') }}
            </p>
            <form
              id="auth-form"
              :aria-labelledby="!isRegister ? mode + '-tab' : undefined"
              @submit.prevent="submit"
            >
              <label class="auth-gateway-field"
                ><span>{{
                  !isRegister && mode === 'password'
                    ? s('邮箱 / 账户名', 'Email / account name')
                    : s('邮箱', 'Email')
                }}</span
                ><input
                  v-model="email"
                  :type="!isRegister && mode === 'password' ? 'text' : 'email'"
                  name="email"
                  :placeholder="
                    !isRegister && mode === 'password'
                      ? s('输入邮箱或账户名', 'Email or account name')
                      : 'you@example.com'
                  "
                  autocomplete="username"
                  :disabled="formDisabled"
                  required
              /></label>
              <template v-if="isRegister || mode === 'password'">
                <label class="auth-gateway-field"
                  ><span class="auth-gateway-field-row"
                    ><span>{{ s('密码', 'Password') }}</span
                    ><RouterLink v-if="!isRegister" to="/forgot-password">{{
                      s('忘记密码？', 'Forgot password?')
                    }}</RouterLink></span
                  ><span class="auth-password-input"
                    ><input
                      v-model="password"
                      :type="passwordVisible ? 'text' : 'password'"
                      name="password"
                      :minlength="isRegister ? 8 : undefined"
                      :placeholder="
                        isRegister
                          ? s('至少 8 个字符', 'At least 8 characters')
                          : s('输入密码', 'Password')
                      "
                      :autocomplete="isRegister ? 'new-password' : 'current-password'"
                      :disabled="formDisabled"
                      required /><button
                      type="button"
                      :aria-label="
                        passwordVisible
                          ? s('隐藏密码', 'Hide password')
                          : s('显示密码', 'Show password')
                      "
                      :aria-pressed="passwordVisible"
                      @click="passwordVisible = !passwordVisible"
                    >
                      <Hide v-if="passwordVisible" /><View v-else /></button></span
                ></label>
                <label v-if="isRegister" class="auth-gateway-field"
                  ><span>{{ s('确认密码', 'Confirm password') }}</span
                  ><input
                    v-model="confirmPassword"
                    :type="passwordVisible ? 'text' : 'password'"
                    name="confirm-password"
                    :placeholder="s('再次输入密码', 'Re-enter password')"
                    autocomplete="new-password"
                    :disabled="formDisabled"
                    required
                /></label>
              </template>
              <label v-if="isRegister || mode === 'email'" class="auth-gateway-field"
                ><span>{{ s('邮箱验证码', 'Email verification code') }}</span
                ><span class="auth-code-input"
                  ><input
                    v-model="code"
                    type="text"
                    name="code"
                    inputmode="numeric"
                    pattern="[0-9]{6}"
                    maxlength="6"
                    autocomplete="one-time-code"
                    :placeholder="s('6 位验证码', '6 digit code')"
                    :disabled="formDisabled"
                    required
                  /><button
                    type="button"
                    :disabled="sending || countdown > 0 || formDisabled"
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
              <p v-if="!isRegister && mode === 'email'" class="auth-gateway-hint">
                {{ s('使用已注册邮箱', 'Use your registered email') }}
              </p>
              <label v-if="isRegister" class="auth-agreement"
                ><input v-model="accepted" type="checkbox" :disabled="formDisabled" /><span
                  >{{ s('我已阅读并同意', 'I agree to the') }}
                  <RouterLink to="/terms" target="_blank">{{ s('用户协议', 'Terms') }}</RouterLink>
                  {{ s('与', 'and') }}
                  <RouterLink to="/privacy" target="_blank">{{
                    s('隐私政策', 'Privacy policy')
                  }}</RouterLink></span
                ></label
              >
              <p v-if="error" class="auth-gateway-feedback" role="alert" aria-live="polite">
                {{ error }}
              </p>
              <button class="auth-submit" type="submit" :disabled="formDisabled">
                {{
                  busy
                    ? s('请稍候…', 'Please wait…')
                    : isRegister
                      ? s('创建账户', 'Create account')
                      : s('登录', 'Sign in')
                }}<span v-if="!busy" aria-hidden="true">→</span>
              </button>
            </form>
            <div v-if="providers?.github || providers?.google" class="auth-gateway-divider">
              <span>{{ s('或', 'or') }}</span>
            </div>
            <div v-if="providers?.github || providers?.google" class="auth-provider-buttons">
              <button
                v-if="providers?.github"
                type="button"
                :disabled="formDisabled"
                @click="startOAuth('github')"
              >
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path
                    fill="currentColor"
                    d="M12 .8a11.2 11.2 0 0 0-3.54 21.82c.56.1.76-.24.76-.54v-2.1c-3.12.68-3.78-1.33-3.78-1.33-.51-1.3-1.25-1.65-1.25-1.65-1.02-.7.08-.69.08-.69 1.13.08 1.72 1.16 1.72 1.16 1 1.72 2.64 1.22 3.28.93.1-.72.39-1.22.71-1.5-2.49-.28-5.11-1.25-5.11-5.54 0-1.22.44-2.22 1.16-3-.12-.28-.5-1.42.11-2.96 0 0 .95-.3 3.08 1.15a10.72 10.72 0 0 1 5.6 0c2.13-1.44 3.08-1.15 3.08-1.15.61 1.54.23 2.68.11 2.97.73.77 1.16 1.77 1.16 2.99 0 4.3-2.62 5.25-5.12 5.53.4.35.76 1.03.76 2.08v3.11c0 .3.2.65.77.54A11.2 11.2 0 0 0 12 .8Z"
                  /></svg
                >GitHub
              </button>
              <button
                v-if="providers?.google"
                type="button"
                :disabled="formDisabled"
                @click="startOAuth('google')"
              >
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path
                    fill="#4285F4"
                    d="M21.6 12.23c0-.71-.06-1.39-.19-2.05H12v3.87h5.38a4.6 4.6 0 0 1-2 3.02v2.51h3.24c1.9-1.75 2.98-4.33 2.98-7.35Z"
                  />
                  <path
                    fill="#34A853"
                    d="M12 22c2.7 0 4.96-.9 6.62-2.42l-3.24-2.51c-.9.6-2.04.97-3.38.97-2.6 0-4.8-1.76-5.6-4.12H3.06v2.6A10 10 0 0 0 12 22Z"
                  />
                  <path
                    fill="#FBBC05"
                    d="M6.4 13.92a6 6 0 0 1 0-3.84V7.48H3.06a10 10 0 0 0 0 9.04l3.34-2.6Z"
                  />
                  <path
                    fill="#EA4335"
                    d="M12 5.96c1.47 0 2.79.5 3.82 1.5l2.87-2.88A9.6 9.6 0 0 0 12 2a10 10 0 0 0-8.94 5.48l3.34 2.6c.8-2.36 3-4.12 5.6-4.12Z"
                  /></svg
                >Google
              </button>
            </div>
            <p
              v-if="!isRegister && (providers?.github || providers?.google)"
              class="auth-gateway-hint"
            >
              {{ s('继续即同意', 'By continuing you agree to the') }}
              <RouterLink to="/terms" target="_blank">{{ s('用户协议', 'Terms') }}</RouterLink>
              {{ s('和', 'and') }}
              <RouterLink to="/privacy" target="_blank">{{
                s('隐私政策', 'Privacy policy')
              }}</RouterLink>
              {{ s('，未注册将自动创建账户。', '. New users get an account automatically.') }}
            </p>
            <p v-if="providerError" class="auth-gateway-hint">
              {{ s('第三方登录状态暂时无法加载。', 'Could not load provider availability.') }}
            </p>
            <p class="auth-switch">
              <span v-if="isRegister || registrationOpen">{{
                isRegister
                  ? s('已经有账户？', 'Already have an account?')
                  : s('还没有账户？', 'New here?')
              }}</span>
              <RouterLink v-if="isRegister || registrationOpen" :to="switchLink">{{
                isRegister ? s('返回登录', 'Back to sign in') : s('注册账户', 'Create account')
              }}</RouterLink
              ><span v-else>{{ s('注册暂未开放', 'Registration is closed') }}</span>
            </p>
          </div>
        </div>
      </section>
      <footer class="auth-gateway-footer">
        <span>© {{ new Date().getFullYear() }} {{ siteName }}</span>
        <nav>
          <RouterLink to="/terms">{{ s('用户协议', 'Terms') }}</RouterLink
          ><RouterLink to="/privacy">{{ s('隐私政策', 'Privacy') }}</RouterLink>
        </nav>
      </footer>
    </div>
  </main>
</template>
