<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import {
  getOAuthSettings,
  saveOAuthSettings,
  type OAuthProvider,
  type OAuthSettings,
  type OAuthSettingsInput
} from '../../api/auth'
import { useStudio } from '../../composables/useStudio'
import { readError } from '../../utils/errors'

const { s } = useStudio()
const providers: { id: OAuthProvider; name: string; url: string }[] = [
  { id: 'github', name: 'GitHub', url: 'https://github.com/settings/developers' },
  { id: 'google', name: 'Google', url: 'https://console.cloud.google.com/apis/credentials' }
]
const loading = ref(true),
  saving = ref(false),
  error = ref(''),
  current = ref<OAuthSettings | null>(null)
const form = reactive<OAuthSettingsInput>({
  github: { enabled: false, client_id: '', client_secret: '', clear_client_secret: false },
  google: { enabled: false, client_id: '', client_secret: '', clear_client_secret: false }
})
const dirty = computed(
  () =>
    current.value !== null &&
    providers.some(({ id }) => {
      const saved = current.value![id],
        entry = form[id]
      return (
        saved.enabled !== entry.enabled ||
        saved.client_id !== entry.client_id ||
        !!entry.client_secret?.trim() ||
        entry.clear_client_secret
      )
    })
)
function apply(value: OAuthSettings) {
  current.value = value
  for (const { id } of providers) {
    form[id] = {
      enabled: value[id].enabled,
      client_id: value[id].client_id,
      client_secret: '',
      clear_client_secret: false
    }
  }
}
async function load() {
  loading.value = true
  error.value = ''
  try {
    apply(await getOAuthSettings())
  } catch (err) {
    error.value = readError(err)
  } finally {
    loading.value = false
  }
}
async function save() {
  if (!current.value || loading.value || saving.value || error.value) return
  for (const { id, name } of providers) {
    if (
      form[id].enabled &&
      (!form[id].client_id.trim() ||
        (!form[id].client_secret?.trim() &&
          (!current.value[id].client_secret_set || form[id].clear_client_secret)))
    ) {
      ElMessage.warning(
        s(
          `请补全 ${name} 的 Client ID 和 Client Secret。`,
          `Provide the ${name} client ID and secret.`
        )
      )
      return
    }
    if (form[id].enabled && form[id].clear_client_secret) {
      ElMessage.warning(
        s('清除密钥前，请先关闭对应登录方式。', 'Disable the provider before clearing its secret.')
      )
      return
    }
  }
  saving.value = true
  try {
    apply(await saveOAuthSettings(form))
    ElMessage.success(s('登录设置已保存', 'Authentication settings saved'))
  } catch (err) {
    ElMessage.error(readError(err))
  } finally {
    saving.value = false
  }
}
async function copy(value: string) {
  try {
    await navigator.clipboard.writeText(value)
    ElMessage.success(s('回调地址已复制', 'Callback URL copied'))
  } catch {
    ElMessage.info(s('请选中地址手动复制', 'Select the URL and copy it manually'))
  }
}
onMounted(load)
</script>
<template>
  <section v-loading="loading" class="oauth-settings">
    <header class="oauth-settings-heading">
      <div>
        <h2>{{ s('第三方登录', 'Social sign in') }}</h2>
        <p>
          {{
            s(
              '配置应用凭据后，用户可通过对应平台登录或注册。',
              'Configure application credentials to enable provider sign in and registration.'
            )
          }}
        </p>
      </div>
      <el-button
        type="primary"
        :loading="saving"
        :disabled="!dirty || loading || !!error"
        @click="save"
        >{{ s('保存更改', 'Save changes') }}</el-button
      >
    </header>
    <div v-if="error" class="oauth-settings-error" role="alert">
      <span>{{ error }}</span
      ><el-button @click="load">{{ s('重试', 'Retry') }}</el-button>
    </div>
    <form @submit.prevent="save">
      <section v-for="provider in providers" :key="provider.id" class="oauth-provider-section">
        <header>
          <div>
            <h3>{{ provider.name }}</h3>
            <a :href="provider.url" target="_blank" rel="noopener noreferrer"
              >{{ s('应用管理', 'Manage applications') }} ↗</a
            >
          </div>
          <el-switch
            v-model="form[provider.id].enabled"
            :disabled="loading || saving || !!error"
            :aria-label="s(`启用 ${provider.name} 登录`, `Enable ${provider.name} sign in`)"
          />
        </header>
        <div class="oauth-provider-fields">
          <label
            ><span>Client ID</span
            ><el-input
              v-model="form[provider.id].client_id"
              :disabled="loading || saving || !!error"
              autocomplete="off"
              :maxlength="512"
              placeholder="Client ID" /></label
          ><label
            ><span
              >Client Secret
              <small>{{
                current?.[provider.id].client_secret_set
                  ? s('已保存', 'Saved')
                  : s('未设置', 'Not set')
              }}</small></span
            ><el-input
              v-model="form[provider.id].client_secret"
              type="password"
              show-password
              :disabled="loading || saving || !!error || form[provider.id].clear_client_secret"
              autocomplete="new-password"
              :maxlength="4096"
              :placeholder="
                current?.[provider.id].client_secret_set
                  ? s('留空保留当前密钥', 'Leave blank to keep the saved secret')
                  : 'Client Secret'
              "
          /></label>
        </div>
        <div class="oauth-callback-field">
          <span>{{ s('授权回调地址', 'Authorization callback URL') }}</span>
          <div>
            <code>{{
              current?.[provider.id].callback_url ||
              s('需要配置 PUBLIC_BASE_URL', 'Configure PUBLIC_BASE_URL first')
            }}</code
            ><button
              v-if="current?.[provider.id].callback_url"
              type="button"
              @click="copy(current![provider.id].callback_url)"
            >
              {{ s('复制', 'Copy') }}
            </button>
          </div>
        </div>
        <el-checkbox
          v-if="current?.[provider.id].client_secret_set"
          v-model="form[provider.id].clear_client_secret"
          :disabled="loading || saving || !!error"
          >{{ s('清除已保存的密钥', 'Clear the saved secret') }}</el-checkbox
        >
      </section>
    </form>
    <footer>
      {{
        s(
          '第三方应用的回调地址必须与这里完全一致。Google 需要已验证的域名，公网 IP 不能直接用作其网页 OAuth 回调。已有邮箱账户请继续使用密码或邮箱验证码登录。',
          'Callback URLs must match exactly. Google requires a verified domain and does not allow public IP web OAuth callbacks. Existing email accounts should keep using password or email code sign in.'
        )
      }}
    </footer>
  </section>
</template>
<style scoped>
.oauth-settings {
  max-width: 960px;
  background: #fff;
  border: 1px solid #e5e8ed;
  border-radius: 10px;
  overflow: hidden;
  color: #303a49;
}
.oauth-settings-heading {
  padding: 24px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 24px;
  border-bottom: 1px solid #eceef2;
}
.oauth-settings-heading h2 {
  font-size: 18px;
  font-weight: 600;
  margin: 0 0 6px;
}
.oauth-settings-heading p {
  margin: 0;
  color: #929aa7;
  font-size: 12px;
  line-height: 1.7;
}
.oauth-provider-section {
  padding: 24px;
  border-bottom: 1px solid #eceef2;
}
.oauth-provider-section > header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 22px;
}
.oauth-provider-section h3 {
  font-size: 16px;
  font-weight: 600;
  margin: 0 0 4px;
}
.oauth-provider-section header a {
  font-size: 11px;
  color: #7b8ca9;
}
.oauth-provider-fields {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 20px;
}
.oauth-provider-fields > label {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
}
.oauth-provider-fields label > span,
.oauth-callback-field > span {
  font-size: 12px;
  color: #7c8798;
}
.oauth-provider-fields small {
  font-size: 10px;
  color: #a3acba;
  margin-left: 5px;
}
.oauth-callback-field {
  margin: 20px 0 12px;
}
.oauth-callback-field > div {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  border: 1px solid #edf0f4;
  border-radius: 5px;
  background: #fafbfc;
  padding: 10px 12px;
  margin-top: 8px;
}
.oauth-callback-field code {
  font-size: 11px;
  line-height: 1.8;
  overflow-wrap: anywhere;
  min-width: 0;
  color: #6f7f98;
}
.oauth-callback-field button {
  flex-shrink: 0;
  border: 0;
  background: transparent;
  color: #637da8;
  font-size: 11px;
}
.oauth-settings > footer {
  padding: 20px 24px;
  color: #9aa3b1;
  font-size: 11px;
  line-height: 1.9;
}
.oauth-settings-error {
  padding: 16px 24px;
  background: #fff7f3;
  color: #ad6c50;
  display: flex;
  gap: 20px;
  align-items: center;
  justify-content: space-between;
  font-size: 12px;
}
@media (max-width: 650px) {
  .oauth-settings-heading {
    align-items: flex-start;
    gap: 16px;
  }
  .oauth-settings-heading,
  .oauth-provider-section {
    padding: 20px;
  }
  .oauth-provider-fields {
    grid-template-columns: 1fr;
    gap: 16px;
  }
  .oauth-settings-heading h2 {
    font-size: 16px;
  }
  .oauth-settings-heading .el-button {
    padding: 8px 12px;
    font-size: 12px;
  }
}
</style>
