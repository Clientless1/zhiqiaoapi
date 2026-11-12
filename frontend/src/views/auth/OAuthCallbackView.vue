<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { exchangeOAuthTicket, safeAuthRedirect } from '../../api/auth'
import BrandLogo from '../../components/common/BrandLogo.vue'
import { useSiteBrand } from '../../composables/useSiteBrand'
import { useStudio } from '../../composables/useStudio'
import { useAuthStore } from '../../stores/auth'
import { ApiError } from '../../utils/errors'
import '../../styles/auth-gateway.css'

const { s } = useStudio(),
  { siteName } = useSiteBrand(),
  auth = useAuthStore(),
  router = useRouter()
const error = ref(''),
  processing = ref(true)
function message(code: string) {
  const messages: Record<string, [string, string]> = {
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
      '该账户需要先修改初始密码，请使用原登录方式完成。',
      'This account must change its initial password using its original sign in method.'
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
    failure = params.get('error'),
    linked = params.get('linked') === 'true'
  // Remove the short lived ticket before network requests or further navigation.
  window.history.replaceState(window.history.state, '', window.location.pathname)
  let redirect: string | null = null
  try {
    redirect = sessionStorage.getItem('zhiqiao_oauth_redirect')
    sessionStorage.removeItem('zhiqiao_oauth_redirect')
  } catch {
    /* Sign in can finish without a stored redirect. */
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
                : s('登录未完成', 'Sign in incomplete')
            }}
          </h1>
        </div>
        <p role="status">
          {{
            processing
              ? s('请稍候，即将进入工作台。', 'Please wait while we open your workspace.')
              : error
          }}
        </p>
        <RouterLink v-if="!processing" to="/login"
          >{{ s('返回登录', 'Back to sign in') }} →</RouterLink
        >
      </div>
    </section>
  </main>
</template>
