<script setup lang="ts">
import { Lock, User } from '@element-plus/icons-vue'
import { ElMessage, type InputInstance } from 'element-plus'
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { login as loginAccount, requestLoginVerificationCode } from '../../api/auth'
import type { LoginRole } from '../../api/auth'
import LocaleToggleButton from '../../components/common/LocaleToggleButton.vue'
import BrandLogo from '../../components/common/BrandLogo.vue'
import { useLocale } from '../../composables/useLocale'
import { withLoading } from '../../composables/useLoadingTask'
import { useSiteBrand } from '../../composables/useSiteBrand'
import { useAuthStore } from '../../stores/auth'
import { ApiError, isSmtpConfigError, readError } from '../../utils/errors'
import { getSetupStatus } from '../../api/policy'
import { useAsyncData } from '../../composables/useAsyncData'

const auth = useAuthStore()
const route = useRoute()
const router = useRouter()
const { locale, t } = useLocale()
const { siteName } = useSiteBrand()
const { data: policy } = useAsyncData(() => getSetupStatus(), null)
const registrationOpen = computed(() => policy.value?.registration_enabled !== false)
const isRegister = computed(() => route.name === 'register' || route.query.mode === 'register')
const loginTitle = computed(() =>
  t(isRegister.value ? 'registrationTitle' : 'loginTitle', { siteName: siteName.value })
)
const switchAuthLink = computed(() => ({
  path: isRegister.value ? '/login' : '/register',
  query: typeof route.query.redirect === 'string' ? { redirect: route.query.redirect } : {}
}))
const username = ref('')
const password = ref('')
const confirmPassword = ref('')
const verificationCode = ref('')
const error = ref('')
const signingIn = ref(false)
const sendingVerificationCode = ref(false)
const showVerificationCode = ref(false)
const passwordInput = ref<InputInstance>()
const verificationInput = ref<InputInstance>()
const minPasswordLength = 8
const resendSeconds = ref(0)
let resendTimer: ReturnType<typeof setInterval> | undefined

function startResendCountdown() {
  resendSeconds.value = 60
  if (resendTimer) clearInterval(resendTimer)
  resendTimer = setInterval(() => {
    resendSeconds.value -= 1
    if (resendSeconds.value <= 0 && resendTimer) clearInterval(resendTimer)
  }, 1000)
}
onBeforeUnmount(() => {
  if (resendTimer) clearInterval(resendTimer)
})

watch(isRegister, () => {
  error.value = ''
  password.value = ''
  confirmPassword.value = ''
  verificationCode.value = ''
  showVerificationCode.value = false
})

watch(username, () => {
  showVerificationCode.value = false
  verificationCode.value = ''
})

function focusPasswordOnTab(event: KeyboardEvent) {
  if (event.shiftKey) return
  event.preventDefault()
  passwordInput.value?.focus()
}

async function login() {
  error.value = ''
  if (!username.value.trim()) {
    error.value = t('emailRequired')
    return
  }
  if (isRegister.value && !registrationOpen.value) {
    error.value = t('registrationClosed')
    return
  }
  if (isRegister.value && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(username.value.trim())) {
    error.value = t('invalidEmail')
    return
  }
  if (password.value.length < minPasswordLength) {
    error.value = t('passwordMinLength')
    return
  }
  if (isRegister.value && password.value !== confirmPassword.value) {
    error.value = t('registrationPasswordMismatch')
    return
  }
  if (isRegister.value && !/^\d{6}$/.test(verificationCode.value.trim())) {
    error.value = t('registrationCodeRequired')
    return
  }

  await withLoading(signingIn, async () => {
    try {
      const data = await loginAccount(
        username.value.trim(),
        password.value,
        verificationCode.value.trim()
      )
      auth.setToken(data.token, data.role, data.requires_password_change === true)
      await router.replace(readRedirectPath(data.role))
    } catch (err) {
      if (isVerificationRequiredError(err)) {
        showVerificationCode.value = true
        await nextTick()
        verificationInput.value?.focus()
      }
      error.value = readLoginError(err)
    }
  })
}

async function sendVerificationCode() {
  if (resendSeconds.value > 0 || sendingVerificationCode.value) return
  error.value = ''
  if (!username.value.trim()) {
    error.value = t('emailRequired')
    return
  }

  await withLoading(sendingVerificationCode, async () => {
    try {
      await requestLoginVerificationCode(username.value, locale.value)
      showVerificationCode.value = true
      startResendCountdown()
      ElMessage.success(t('loginVerificationCodeSent'))
    } catch (err) {
      ElMessage.error(readLoginVerificationCodeError(err))
    }
  })
}

function isVerificationRequiredError(err: unknown) {
  return err instanceof ApiError && err.code === 'verification_code_required'
}

function readLoginError(err: unknown) {
  if (isVerificationRequiredError(err)) {
    return t('loginVerificationRequired')
  }
  if (err instanceof ApiError && err.code === 'invalid_verification_code') {
    return t('loginVerificationInvalid')
  }
  if (err instanceof ApiError && err.code === 'registration_closed') {
    return t('registrationClosed')
  }
  if (err instanceof ApiError && err.code === 'account_pending_approval') {
    return t('accountPendingApproval')
  }
  if (isSmtpConfigError(err)) {
    return t('smtpEmailUnavailable')
  }
  if (isLoginVerificationRateLimitedError(err)) {
    return t('loginVerificationRateLimited')
  }
  if (err instanceof ApiError && err.code === 'password_min_length') {
    return t('passwordMinLength')
  }
  if (err instanceof ApiError && (err.status === 401 || err.status === 403)) {
    return t('loginFailed')
  }
  return err instanceof Error ? err.message : String(err)
}

function readLoginVerificationCodeError(err: unknown) {
  if (isSmtpConfigError(err)) {
    return t('smtpEmailUnavailable')
  }
  if (isLoginVerificationRateLimitedError(err)) {
    return t('loginVerificationRateLimited')
  }
  return readError(err)
}

function isLoginVerificationRateLimitedError(err: unknown) {
  return err instanceof ApiError && err.code === 'login_verification_rate_limited'
}

function readRedirectPath(role: LoginRole) {
  const redirect = route.query.redirect
  if (role === 'user' && auth.requiresPasswordChange) {
    const target = typeof redirect === 'string' && redirect.startsWith('/home') ? redirect : ''
    return target ? `/change-password?redirect=${encodeURIComponent(target)}` : '/change-password'
  }
  if (role === 'admin') {
    return typeof redirect === 'string' && redirect.startsWith('/admin') ? redirect : '/admin'
  }
  return typeof redirect === 'string' && redirect.startsWith('/home') ? redirect : '/home/overview'
}
</script>

<template>
  <main class="login-shell">
    <LocaleToggleButton class="login-language home-language-button" />
    <RouterLink to="/" class="login-home-link">← {{ t('backHome') }}</RouterLink>
    <section class="login-stage login-stage-redesign">
      <aside class="login-story">
        <RouterLink to="/" class="login-story-brand" :aria-label="siteName"
          ><BrandLogo
        /></RouterLink>
        <span class="section-eyebrow">ONE GATEWAY. MORE POSSIBILITIES.</span>
        <h2>{{ t('loginWelcome') }}</h2>
        <p>{{ t('loginIntro') }}</p>
        <div class="login-story-art" aria-hidden="true">
          <span>Text</span><i>↗</i><span>Image</span><span>Video</span>
        </div>
        <RouterLink to="/models">{{ t('discoverBeforeLogin') }} ↗</RouterLink>
      </aside>
      <el-form class="login-panel" @submit.prevent="login">
        <div class="login-panel-heading">
          <h1>{{ loginTitle }}</h1>
          <p>{{ t(isRegister ? 'registrationHint' : 'loginAccountHint') }}</p>
        </div>
        <el-alert
          v-if="isRegister && !registrationOpen"
          :title="t('registrationClosed')"
          type="warning"
          :closable="false"
        />
        <div class="login-fields">
          <label class="login-field">
            <span>{{ t('email') }}</span>
            <el-input
              v-model="username"
              :prefix-icon="User"
              :placeholder="t('loginAccountPlaceholder')"
              :type="isRegister ? 'email' : 'text'"
              autocomplete="username"
              size="large"
              @keydown.tab="focusPasswordOnTab"
            />
          </label>
          <label class="login-field">
            <span class="login-field-row">
              <span>{{ t('password') }}</span>
              <RouterLink v-if="!isRegister" class="login-text-button" to="/forgot-password">{{
                t('forgotPassword')
              }}</RouterLink>
            </span>
            <el-input
              ref="passwordInput"
              v-model="password"
              :prefix-icon="Lock"
              :placeholder="t('password')"
              :autocomplete="isRegister ? 'new-password' : 'current-password'"
              show-password
              size="large"
            />
          </label>
          <label v-if="isRegister" class="login-field">
            <span>{{ t('registrationConfirmPassword') }}</span>
            <el-input
              v-model="confirmPassword"
              :prefix-icon="Lock"
              :placeholder="t('registrationConfirmPassword')"
              autocomplete="new-password"
              show-password
              size="large"
            />
          </label>
          <label v-if="isRegister || showVerificationCode" class="login-field">
            <span class="login-field-row">
              <span>{{ t('verificationCode') }}</span>
              <span class="login-field-hint">{{ t('loginVerificationHint') }}</span>
            </span>
            <div class="login-code-row">
              <el-input
                ref="verificationInput"
                v-model="verificationCode"
                :placeholder="t('verificationCode')"
                maxlength="6"
                inputmode="numeric"
                autocomplete="one-time-code"
                size="large"
              />
              <el-button
                class="login-code-button"
                native-type="button"
                type="primary"
                :loading="sendingVerificationCode"
                :disabled="resendSeconds > 0 || (isRegister && !registrationOpen)"
                @click="sendVerificationCode"
              >
                {{
                  resendSeconds > 0
                    ? t('registrationResend', { seconds: resendSeconds })
                    : t('sendVerificationCode')
                }}
              </el-button>
            </div>
          </label>
        </div>
        <el-button
          class="login-submit"
          native-type="submit"
          type="primary"
          size="large"
          :loading="signingIn"
          :disabled="isRegister && !registrationOpen"
        >
          {{ t(isRegister ? 'registrationSubmit' : 'signIn') }}
        </el-button>
        <el-alert v-if="error" :title="error" type="error" show-icon :closable="false" />
        <p v-if="isRegister || registrationOpen" class="login-auth-switch">
          {{ t(isRegister ? 'registrationHasAccount' : 'registrationNoAccount') }}
          <RouterLink :to="switchAuthLink">{{
            t(isRegister ? 'signIn' : 'landingRegister')
          }}</RouterLink>
        </p>
      </el-form>
    </section>
  </main>
</template>

<style scoped>
.login-story-brand .site-brand-image {
  width: 104px;
  height: 64px;
}
.login-auth-switch {
  margin: 18px 0 0;
  text-align: center;
  color: #858da1;
  font-size: 13px;
}
.login-auth-switch a {
  margin-left: 6px;
  color: #5145ff;
  text-decoration: none;
  font-weight: 500;
}
</style>
