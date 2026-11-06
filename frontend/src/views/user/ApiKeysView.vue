<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Plus, RefreshRight, Search, CopyDocument, Key, Download } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus/es/components/message/index'
import { ElMessageBox } from 'element-plus/es/components/message-box/index'
import {
  getOwnUserKeys,
  createOwnUserKey,
  updateOwnUserKeyScope,
  updateOwnUserKeyStatus,
  deleteOwnUserKey
} from '../../api/userKeys'
import { getPackages } from '../../api/commerce'
import { getModelCatalog } from '../../api/catalog'
import type { UserKey } from '../../types/admin'
import { useAsyncData } from '../../composables/useAsyncData'
import { useStudio } from '../../composables/useStudio'
import { useSiteBrand } from '../../composables/useSiteBrand'
import {
  buildCCSwitchLink,
  buildClientConfig,
  getCCSwitchModels,
  type CCSwitchApp
} from '../../utils/ccswitch'
import { formatCompactDateTime, maskApiKey } from '../../utils/format'
import { readError } from '../../utils/errors'
const { s } = useStudio()
const { siteName } = useSiteBrand()
const { data: keys, loading, error, reload } = useAsyncData(getOwnUserKeys, [])
const {
  data: packages,
  error: packageError,
  reload: reloadPackages
} = useAsyncData(getPackages, [])
const search = ref(''),
  filter = ref('all'),
  open = ref(false),
  editing = ref<UserKey | null>(null),
  busy = ref(false),
  mutating = ref<number[]>([])
const name = ref(''),
  packageId = ref<number>(),
  models = ref<string[]>([]),
  createdKey = ref(''),
  createdRecord = ref<UserKey | null>(null),
  createdOpen = ref(false),
  formError = ref('')
const importOpen = ref(false)
const importRecord = ref<UserKey | null>(null)
const importApp = ref<CCSwitchApp>('codex')
const importModel = ref('')
const {
  data: catalog,
  loading: catalogLoading,
  error: catalogError,
  reload: reloadCatalog
} = useAsyncData(() => getModelCatalog(true), {
  currency: 'CNY',
  personalized: true,
  models: []
})
const clientOptions: { value: CCSwitchApp; label: string }[] = [
  { value: 'codex', label: 'Codex' },
  { value: 'claude', label: 'Claude Code' },
  { value: 'opencode', label: 'OpenCode' }
]
const importApps = computed(() =>
  clientOptions.filter(
    (app) =>
      getCCSwitchModels(app.value, importRecord.value?.model_limits ?? [], catalog.value.models)
        .length > 0
  )
)
const importModels = computed(() =>
  getCCSwitchModels(importApp.value, importRecord.value?.model_limits ?? [], catalog.value.models)
)
const canImport = computed(
  () =>
    importRecord.value?.status === 'enabled' &&
    hasFullKey(importRecord.value.key) &&
    !catalogLoading.value &&
    !catalogError.value &&
    importModels.value.includes(importModel.value)
)
const importProvider = computed(() => ({
  app: importApp.value,
  name: `${siteName.value} · ${importRecord.value?.name ?? ''}`,
  origin: window.location.origin,
  key: importRecord.value?.key ?? '',
  model: importModel.value
}))
watch(importApps, (apps) => {
  if (!apps.some((app) => app.value === importApp.value))
    importApp.value = apps[0]?.value ?? 'codex'
})
watch(importModels, (available) => {
  if (!available.includes(importModel.value)) importModel.value = available[0] ?? ''
})
const activePackages = computed(() => packages.value.filter((p) => p.active && p.service_enabled))
const currentPackage = computed(() => packages.value.find((p) => p.id === packageId.value))
const filtered = computed(() =>
  keys.value.filter(
    (k) =>
      (filter.value === 'all' || k.status === filter.value) &&
      `${k.name} ${k.key_prefix} ${k.package_name ?? ''}`
        .toLowerCase()
        .includes(search.value.toLowerCase())
  )
)
watch(packageId, () => {
  models.value = models.value.filter((m) => currentPackage.value?.models.includes(m))
})
function create() {
  editing.value = null
  name.value = ''
  packageId.value = activePackages.value[0]?.id
  models.value = []
  formError.value = ''
  open.value = true
}
function edit(key: UserKey) {
  editing.value = key
  name.value = key.name
  packageId.value = key.package_id ?? undefined
  models.value = [...(key.model_limits ?? [])]
  formError.value = ''
  open.value = true
}
async function save() {
  if (busy.value) return
  if (!name.value.trim() || !packageId.value) {
    formError.value = s('请填写名称并选择有效套餐', 'Enter a name and choose a valid package')
    return
  }
  const scope = models.value.length ? [...models.value] : [...(currentPackage.value?.models ?? [])]
  if (!scope.length) {
    formError.value = s(
      '当前套餐未配置模型，请先联系管理员。',
      'This package has no models. Contact your administrator.'
    )
    return
  }
  busy.value = true
  formError.value = ''
  try {
    if (editing.value) await updateOwnUserKeyScope(editing.value.id, packageId.value, scope)
    else {
      const result = await createOwnUserKey(name.value.trim(), packageId.value, scope)
      createdKey.value = result.key
      createdRecord.value = { ...result.record, key: result.key }
      createdOpen.value = true
    }
    open.value = false
    await reload()
    ElMessage.success(s('已保存', 'Saved'))
  } catch (err) {
    formError.value = readError(err)
  } finally {
    busy.value = false
  }
}
async function status(key: UserKey) {
  if (mutating.value.includes(key.id)) return
  mutating.value.push(key.id)
  try {
    await updateOwnUserKeyStatus(key.id, key.status === 'enabled' ? 'disabled' : 'enabled')
    await reload()
  } catch (err) {
    ElMessage.error(readError(err))
  } finally {
    mutating.value = mutating.value.filter((id) => id !== key.id)
  }
}
async function remove(key: UserKey) {
  try {
    await ElMessageBox.confirm(
      s(
        `删除「${key.name}」后，使用该 Key 的应用将无法继续调用。`,
        `Applications using “${key.name}” will lose access after deletion.`
      ),
      s('删除 API Key', 'Delete API key'),
      {
        type: 'warning',
        confirmButtonText: s('删除', 'Delete'),
        cancelButtonText: s('取消', 'Cancel')
      }
    )
  } catch {
    return
  }
  mutating.value.push(key.id)
  try {
    await deleteOwnUserKey(key.id)
    await reload()
  } catch (err) {
    ElMessage.error(readError(err))
  } finally {
    mutating.value = mutating.value.filter((id) => id !== key.id)
  }
}
async function refreshAll() {
  await Promise.allSettled([reload(), reloadPackages()])
}
async function copy(key: string) {
  try {
    await navigator.clipboard.writeText(key)
    ElMessage.success(s('已复制', 'Copied'))
  } catch {
    ElMessage.error(s('复制失败，请手动复制', 'Please copy manually'))
  }
}
function hasFullKey(value: string) {
  return !!value?.trim() && !/[*•\s]/.test(value)
}
function openImport(key: UserKey) {
  if (key.status !== 'enabled' || !hasFullKey(key.key)) {
    ElMessage.error(
      s('请使用已启用且包含完整凭证的 Key', 'Choose an enabled key with its full credential')
    )
    return
  }
  importRecord.value = key
  importApp.value = importApps.value[0]?.value ?? 'codex'
  importModel.value = importModels.value[0] ?? ''
  importOpen.value = true
  if (catalogError.value) void reloadCatalog()
}
function importCreatedKey() {
  if (!createdRecord.value) return
  openImport(createdRecord.value)
  createdOpen.value = false
}
function clearCreatedKey() {
  createdKey.value = ''
  createdRecord.value = null
}
function clearImport() {
  importRecord.value = null
  importModel.value = ''
}
function importToCCSwitch() {
  if (!canImport.value) return
  window.location.assign(buildCCSwitchLink(importProvider.value))
}
async function copyClientConfig() {
  if (!canImport.value) return
  await copy(buildClientConfig(importProvider.value))
}
</script>
<template>
  <section class="key-workspace">
    <div class="key-intro studio-panel">
      <span class="shortcut-icon"
        ><el-icon><Key /></el-icon
      ></span>
      <div>
        <h3>{{ s('连接应用，从一个 Key 开始', 'One key. Your next integration.') }}</h3>
        <p>
          {{
            s(
              '为不同应用分配独立凭证，按套餐限制模型范围，随时停用与撤销。',
              'Create credentials per application, scope model access by package, and revoke access when needed.'
            )
          }}
        </p>
      </div>
      <el-button type="primary" :icon="Plus" @click="create">{{
        s('创建 API Key', 'Create API key')
      }}</el-button>
    </div>
    <el-alert
      v-if="error || packageError"
      :title="error || packageError"
      type="error"
      :closable="false"
    />
    <div class="key-toolbar">
      <el-input
        v-model="search"
        :prefix-icon="Search"
        clearable
        :placeholder="s('搜索名称、Key 前缀或套餐…', 'Search name, key prefix or package…')"
      /><el-select v-model="filter"
        ><el-option value="all" :label="s('全部状态', 'All statuses')" /><el-option
          value="enabled"
          :label="s('已启用', 'Enabled')" /><el-option
          value="disabled"
          :label="s('已停用', 'Disabled')" /></el-select
      ><el-button :icon="RefreshRight" :loading="loading" @click="refreshAll">{{
        s('刷新', 'Refresh')
      }}</el-button>
    </div>
    <article class="studio-panel key-table">
      <div class="panel-heading">
        <h3>
          API Keys <span class="studio-pill">{{ filtered.length }}</span>
        </h3>
        <RouterLink to="/home/playground">{{ s('前往调试台', 'Open Playground') }} ↗</RouterLink>
      </div>
      <el-table
        v-loading="loading"
        :data="filtered"
        :empty-text="s('暂无 API Key，创建一个开始接入', 'No API keys. Create one to get started.')"
        ><el-table-column :label="s('名称 / 凭证', 'Name / credential')" min-width="220"
          ><template #default="{ row }"
            ><div class="key-name-cell">
              <strong>{{ row.name }}</strong
              ><code>{{ row.key_prefix || maskApiKey(row.key || '') }}••••••</code>
            </div></template
          ></el-table-column
        ><el-table-column :label="s('套餐 / 模型范围', 'Package / model scope')" min-width="210"
          ><template #default="{ row }"
            ><div class="key-name-cell">
              <strong>{{ row.package_name || '—' }}</strong
              ><span>{{
                row.model_limits?.length
                  ? row.model_limits.join(', ')
                  : s('套餐内全部模型', 'All models in package')
              }}</span>
            </div></template
          ></el-table-column
        ><el-table-column :label="s('状态', 'Status')" width="110"
          ><template #default="{ row }"
            ><span :class="['studio-pill', row.status === 'enabled' ? 'good' : 'muted']">{{
              row.status === 'enabled' ? s('已启用', 'Enabled') : s('已停用', 'Disabled')
            }}</span></template
          ></el-table-column
        ><el-table-column :label="s('最近使用', 'Last used')" min-width="150"
          ><template #default="{ row }">{{
            formatCompactDateTime(row.last_active_at)
          }}</template></el-table-column
        ><el-table-column :label="s('操作', 'Actions')" width="360" fixed="right"
          ><template #default="{ row }"
            ><el-button
              link
              type="primary"
              :icon="Download"
              :disabled="row.status !== 'enabled' || mutating.includes(row.id)"
              @click="openImport(row)"
              >{{ s('导入 CC Switch', 'Import to CC Switch') }}</el-button
            ><el-button
              v-if="row.key && !row.key.includes('*')"
              link
              type="primary"
              @click="copy(row.key)"
              >{{ s('复制', 'Copy') }}</el-button
            ><el-button
              link
              type="primary"
              :disabled="mutating.includes(row.id)"
              @click="edit(row)"
              >{{ s('范围', 'Scope') }}</el-button
            ><el-button link :disabled="mutating.includes(row.id)" @click="status(row)">{{
              row.status === 'enabled' ? s('停用', 'Disable') : s('启用', 'Enable')
            }}</el-button
            ><el-button
              link
              type="danger"
              :disabled="mutating.includes(row.id)"
              @click="remove(row)"
              >{{ s('删除', 'Delete') }}</el-button
            ></template
          ></el-table-column
        ></el-table
      >
      <p class="studio-note">
        {{
          s(
            '完整 Key 创建后立即妥善保存。页面默认仅显示前缀；不要将密钥提交到公开代码仓库。',
            'Store the full key after creation. This page shows prefixes by default. Keep secrets out of public repositories.'
          )
        }}
      </p>
    </article>
    <el-dialog
      v-model="open"
      :title="
        editing ? s('修改模型访问范围', 'Edit model access') : s('创建 API Key', 'Create API key')
      "
      width="min(560px,94vw)"
      :close-on-click-modal="!busy"
      :show-close="!busy"
      :close-on-press-escape="!busy"
      ><el-form label-position="top" @submit.prevent="save"
        ><el-form-item :label="s('名称', 'Name')"
          ><el-input
            v-model="name"
            :disabled="!!editing || busy"
            maxlength="100"
            :placeholder="
              s('例如：我的网站 / 开发环境', 'e.g. My website / Development')
            " /></el-form-item
        ><el-form-item :label="s('绑定套餐', 'Package')"
          ><el-select
            v-model="packageId"
            :disabled="busy"
            :placeholder="s('选择有效套餐', 'Choose an active package')"
            ><el-option
              v-for="p in activePackages"
              :key="p.id"
              :label="p.name"
              :value="p.id" /></el-select></el-form-item
        ><el-form-item :label="s('允许的模型', 'Allowed models')"
          ><el-select
            v-model="models"
            multiple
            filterable
            collapse-tags
            :disabled="busy || !currentPackage"
            :placeholder="s('留空允许套餐内全部模型', 'Leave empty for all models in the package')"
            ><el-option
              v-for="m in currentPackage?.models ?? []"
              :key="m"
              :value="m"
              :label="m" /></el-select
        ></el-form-item>
        <p v-if="!activePackages.length" class="studio-note">
          {{
            s(
              '需要先获得有效套餐后才能创建 Key。',
              'An active package is required to create a key.'
            )
          }}
          <RouterLink to="/home/packages">{{ s('查看套餐', 'View packages') }} ↗</RouterLink>
        </p>
        <el-alert v-if="formError" :title="formError" type="error" :closable="false" /></el-form
      ><template #footer
        ><el-button :disabled="busy" @click="open = false">{{ s('取消', 'Cancel') }}</el-button
        ><el-button
          type="primary"
          :loading="busy"
          :disabled="!activePackages.length"
          @click="save"
          >{{ s('保存', 'Save') }}</el-button
        ></template
      ></el-dialog
    >
    <el-dialog
      v-model="createdOpen"
      :title="s('API Key 已创建', 'API key created')"
      width="min(560px,94vw)"
      @closed="clearCreatedKey"
      ><p>{{ s('请立即复制并妥善保存此凭证。', 'Copy and store this credential securely.') }}</p>
      <el-input :model-value="createdKey" readonly type="password" show-password /><template #footer
        ><el-button type="primary" :icon="CopyDocument" @click="copy(createdKey)">{{
          s('复制 API Key', 'Copy API key')
        }}</el-button
        ><el-button :icon="Download" @click="importCreatedKey">{{
          s('导入 CC Switch', 'Import to CC Switch')
        }}</el-button
        ><el-button @click="createdOpen = false">{{ s('已保存', 'Done') }}</el-button></template
      ></el-dialog
    >
    <el-dialog
      v-model="importOpen"
      :title="s('导入 CC Switch', 'Import to CC Switch')"
      width="min(560px,94vw)"
      @closed="clearImport"
    >
      <p class="studio-note">
        {{
          s(
            '将当前 Key、接口地址和默认模型导入本机 CC Switch。浏览器提示时请选择「打开」。',
            'Import this key, endpoint and default model into CC Switch on your computer. Choose Open when your browser asks.'
          )
        }}
      </p>
      <div class="cc-import-summary">
        <strong>{{ importRecord?.name }}</strong>
        <code>{{ importProvider.origin }}{{ importApp === 'claude' ? '' : '/v1' }}</code>
      </div>
      <el-alert
        v-if="catalogError"
        :title="s('模型目录加载失败，请重试', 'Could not load available models. Please retry.')"
        type="error"
        :closable="false"
      >
        <el-button link type="primary" @click="reloadCatalog">{{ s('重试', 'Retry') }}</el-button>
      </el-alert>
      <el-form v-loading="catalogLoading" label-position="top">
        <el-form-item :label="s('客户端', 'Client')">
          <el-select
            v-model="importApp"
            :aria-label="s('CC Switch 客户端', 'CC Switch client')"
            :placeholder="s('选择客户端', 'Choose a client')"
            style="width: 100%"
          >
            <el-option
              v-for="app in importApps"
              :key="app.value"
              :value="app.value"
              :label="app.label"
            />
          </el-select>
        </el-form-item>
        <el-form-item :label="s('默认模型', 'Default model')">
          <el-select
            v-model="importModel"
            :aria-label="s('CC Switch 默认模型', 'CC Switch default model')"
            :placeholder="s('选择此 Key 允许的模型', 'Choose a model allowed by this key')"
            filterable
            style="width: 100%"
          >
            <el-option v-for="model in importModels" :key="model" :value="model" :label="model" />
          </el-select>
        </el-form-item>
      </el-form>
      <el-alert
        v-if="!catalogLoading && !catalogError && !importModels.length"
        :title="
          s(
            '此 Key 暂无可导入的对话模型，请先在「范围」中配置。',
            'This key has no supported chat models. Configure its model scope first.'
          )
        "
        type="warning"
        :closable="false"
      />
      <p class="studio-note">
        {{
          s(
            '需要先安装 CC Switch。没有弹出应用时，可复制配置手动添加。',
            'Install CC Switch first. If the app does not open, copy the configuration and add it manually.'
          )
        }}
        <a
          href="https://github.com/farion1231/cc-switch/releases"
          target="_blank"
          rel="noopener noreferrer"
          >{{ s('下载 / 更新 CC Switch', 'Download / update CC Switch') }} ↗</a
        >
      </p>
      <template #footer>
        <el-button @click="importOpen = false">{{ s('取消', 'Cancel') }}</el-button>
        <el-button :disabled="!canImport" :icon="CopyDocument" @click="copyClientConfig">
          {{ s('复制配置', 'Copy configuration') }}
        </el-button>
        <el-button
          type="primary"
          :disabled="!canImport"
          :icon="Download"
          @click="importToCCSwitch"
          >{{ s('打开 CC Switch 导入', 'Open CC Switch to import') }}</el-button
        >
      </template>
    </el-dialog>
  </section>
</template>

<style scoped>
.cc-import-summary {
  display: grid;
  gap: 6px;
  padding: 14px 16px;
  margin: 16px 0 22px;
  border: 1px solid #e2e9f6;
  border-radius: 12px;
  background: #f6f9ff;
}
.cc-import-summary code {
  overflow-wrap: anywhere;
  font-size: 12px;
  color: #6d7f9d;
}
</style>
