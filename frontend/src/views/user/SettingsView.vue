<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import { Key, Lock, Select } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import { updateUserPassword } from '../../api/userPassword'
import {
  getLinkedOAuthProviders,
  linkOAuthProvider,
  type LinkedOAuthProvider,
  type OAuthProvider
} from '../../api/auth'
import { useStudio } from '../../composables/useStudio'
import { useLocale } from '../../composables/useLocale'
import { withLoading } from '../../composables/useLoadingTask'
import { useAuthStore } from '../../stores/auth'
import {
  readPasswordChangeError,
  readPasswordChangeValidationError,
  resetPasswordChangeForm
} from '../../utils/password'

const { t } = useLocale()
const { s } = useStudio()
const auth = useAuthStore()
const router = useRouter()
const saving = ref(false)
const identities = ref<LinkedOAuthProvider[]>([])
const linking = ref<OAuthProvider | ''>('')
const identityError = ref(false)

async function loadIdentities() {
  try {
    identities.value = await getLinkedOAuthProviders()
    identityError.value = false
  } catch {
    identityError.value = true
  }
}
onMounted(() => {
  if (router.currentRoute.value.query.linked === 'true') {
    ElMessage.success(
      s('第三方账号已绑定，下次可以直接登录。', 'Account connected. You can now use it to sign in.')
    )
    void router.replace({ path: '/home/settings' })
  }
  void loadIdentities()
})
async function connectProvider(provider: OAuthProvider) {
  if (linking.value) return
  linking.value = provider
  try {
    const { url } = await linkOAuthProvider(provider)
    const target = new URL(url)
    const expected = provider === 'github' ? 'https://github.com' : 'https://accounts.google.com'
    if (target.origin !== expected) throw new Error('Invalid authorization destination')
    sessionStorage.setItem('zhiqiao_oauth_redirect', '/home/settings')
    window.location.assign(target.href)
  } catch {
    ElMessage.error(
      s('暂时无法开始绑定，请稍后重试。', 'Could not start account linking. Please try again.')
    )
    linking.value = ''
  }
}

const form = reactive({
  currentPassword: '',
  newPassword: '',
  confirmPassword: ''
})

function resetForm() {
  resetPasswordChangeForm(form)
}

function validateForm() {
  const error = readPasswordChangeValidationError(form, t)
  if (error) {
    ElMessage.error(error)
    return false
  }
  return true
}

function passwordError(err: unknown) {
  return readPasswordChangeError(err, t, {
    sameAsCurrentKey: 'passwordSameAsCurrent',
    fallback: 'readError'
  })
}

async function save() {
  if (!validateForm()) return

  await withLoading(saving, async () => {
    try {
      await updateUserPassword({
        current_password: form.currentPassword,
        new_password: form.newPassword
      })
      auth.clearToken()
      resetForm()
      ElMessage.success(t('passwordChangeRelogin'))
      await router.replace({ name: 'login', query: { redirect: '/home/settings' } })
    } catch (err) {
      ElMessage.error(passwordError(err))
    }
  })
}
</script>

<template>
  <section class="user-settings-view">
    <section class="user-panel user-settings-form user-identity-panel">
      <header class="user-section-header user-settings-header">
        <div class="user-settings-title">
          <h3>{{ s('登录方式', 'Sign in methods') }}</h3>
        </div>
      </header>
      <p class="user-identity-note">
        {{
          s(
            '绑定后，可使用第三方账号登录当前账户。',
            'Connect an account to sign in with it next time.'
          )
        }}
      </p>
      <div v-if="identityError" class="user-identity-row">
        <span>{{ s('登录方式加载失败', 'Could not load sign in methods') }}</span>
        <el-button text @click="loadIdentities">{{ s('重试', 'Retry') }}</el-button>
      </div>
      <div v-for="identity in identities" :key="identity.provider" class="user-identity-row">
        <strong>{{ identity.provider === 'github' ? 'GitHub' : 'Google' }}</strong>
        <span v-if="identity.linked" class="user-identity-connected">{{
          s('已绑定', 'Connected')
        }}</span>
        <el-button
          v-else
          :disabled="!identity.enabled || !!linking"
          :loading="linking === identity.provider"
          @click="connectProvider(identity.provider)"
        >
          {{ identity.enabled ? s('绑定账号', 'Connect') : s('暂未开放', 'Unavailable') }}
        </el-button>
      </div>
    </section>
    <el-form class="user-panel user-settings-form" label-position="top" @submit.prevent="save">
      <header class="user-section-header user-settings-header">
        <div class="user-settings-title">
          <span class="user-settings-icon" aria-hidden="true">
            <el-icon><Lock /></el-icon>
          </span>
          <h3>{{ t('loginPassword') }}</h3>
        </div>
      </header>

      <div class="user-settings-grid">
        <el-form-item :label="t('currentPassword')">
          <el-input
            v-model="form.currentPassword"
            :prefix-icon="Key"
            autocomplete="current-password"
            show-password
            type="password"
          />
        </el-form-item>

        <el-form-item :label="t('newPassword')">
          <el-input
            v-model="form.newPassword"
            :prefix-icon="Lock"
            autocomplete="new-password"
            show-password
            type="password"
          />
        </el-form-item>

        <el-form-item :label="t('confirmNewPassword')">
          <el-input
            v-model="form.confirmPassword"
            :prefix-icon="Lock"
            autocomplete="new-password"
            show-password
            type="password"
          />
        </el-form-item>
      </div>

      <div class="user-settings-actions">
        <el-button native-type="submit" type="primary" :icon="Select" :loading="saving">
          {{ t('save') }}
        </el-button>
      </div>
    </el-form>
  </section>
</template>

<style scoped>
.user-settings-view {
  display: grid;
  gap: 18px;
  color: #354154;
  font-family:
    Inter,
    ui-sans-serif,
    system-ui,
    -apple-system,
    BlinkMacSystemFont,
    'Segoe UI',
    'PingFang SC',
    'Microsoft YaHei',
    sans-serif;
  max-width: 1120px;
}

.user-identity-note {
  color: #667085;
  font-size: 13px;
  line-height: 1.6;
  margin: 16px 0 6px;
}
.user-identity-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  min-height: 58px;
  border-bottom: 1px solid var(--user-border);
}
.user-identity-row:last-child {
  border-bottom: 0;
}
.user-identity-row strong {
  font-size: 14px;
  font-weight: 500;
}
.user-identity-connected {
  font-size: 13px;
  color: #15803d;
}

.user-settings-form {
  padding: 22px;
  width: min(520px, 100%);
}

.user-settings-header {
  border-bottom: 1px solid var(--user-border);
  padding-bottom: 18px;
}

.user-settings-title {
  align-items: center;
  display: flex;
  gap: 10px;
  min-width: 0;
}

.user-settings-title h3 {
  color: #1f2937;
  font-size: 17px;
  font-weight: 500;
  line-height: 1.25;
}

.user-settings-icon {
  align-items: center;
  background: var(--user-primary-soft);
  border: 1px solid var(--user-primary-border);
  border-radius: 8px;
  color: var(--user-primary);
  display: inline-flex;
  flex: 0 0 auto;
  height: 36px;
  justify-content: center;
  width: 36px;
}

.user-settings-grid {
  display: grid;
  gap: 14px;
  grid-template-columns: minmax(260px, 360px);
  padding-top: 18px;
}

.user-settings-grid :deep(.el-form-item) {
  margin-bottom: 0;
}

.user-settings-grid :deep(.el-form-item__label) {
  color: #667085;
  font-size: 13px;
  font-weight: 400;
  line-height: 1.2;
  margin-bottom: 7px;
}

.user-settings-grid :deep(.el-input__wrapper) {
  border-radius: 8px;
  min-height: 38px;
}

.user-settings-grid :deep(.el-input__inner) {
  color: #354154;
  font-family: inherit;
  font-size: 13px;
  font-weight: 400;
}

.user-settings-grid :deep(.el-input__inner::placeholder) {
  color: #a2acba;
}

.user-settings-grid :deep(.el-input__prefix),
.user-settings-grid :deep(.el-input__suffix) {
  color: #8b96a8;
}

.user-settings-actions {
  display: flex;
  padding-top: 16px;
}

.user-settings-actions :deep(.el-button) {
  border-radius: 7px;
  font-size: 13px;
  font-weight: 500;
  min-height: 36px;
  min-width: 96px;
}

@media (max-width: 640px) {
  .user-settings-form {
    padding: 18px;
  }

  .user-settings-grid {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
