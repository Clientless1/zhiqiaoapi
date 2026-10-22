<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { Plus, Refresh } from '@element-plus/icons-vue'
import { useLocale } from '../../composables/useLocale'
import { useSiteBrand } from '../../composables/useSiteBrand'
import { readError } from '../../utils/errors'
import { createConfirmAction } from '../../utils/confirm'
import {
  formatDateTime,
  formatMoney,
  majorToMicroAmount,
  microAmountToMajor
} from '../../utils/format'
import type { MessageKey } from '../../i18n'
import type { CommunityInfo } from '../../api/commerce'
import {
  listAdminPackages,
  listAdminCoupons,
  listAdminRedemptions,
  listAdminNotifications,
  getAdminCommunity,
  saveAdminCommunity,
  saveAdminPackage,
  createAdminCoupon,
  createAdminRedemption,
  publishAdminNotification,
  deleteAdminNotification,
  type AdminPackage,
  type CouponCampaign,
  type AdminRedemption,
  type AdminNotification
} from '../../api/adminOperations'

const { t, locale } = useLocale()
const { billingCurrency } = useSiteBrand()
const confirm = createConfirmAction(() => t('cancel'))
const tabs = [
  { value: 'packages', key: 'opsPackages' },
  { value: 'coupons', key: 'opsCoupons' },
  { value: 'redemptions', key: 'opsRedemptions' },
  { value: 'notifications', key: 'opsNotifications' },
  { value: 'community', key: 'opsCommunity' }
] as const
const active = ref<(typeof tabs)[number]['value']>('packages')
const loading = ref(false)
const saving = ref(false)
const dialog = ref(false)
const editingId = ref<number>()
const packages = ref<AdminPackage[]>([])
const coupons = ref<CouponCampaign[]>([])
const redemptions = ref<AdminRedemption[]>([])
const notifications = ref<AdminNotification[]>([])
const community = reactive<CommunityInfo>({
  about_title: '',
  about_content: '',
  qq_group_number: '',
  qq_group_url: ''
})
const form = reactive({
  name: '',
  description: '',
  family: 'custom',
  models: '',
  price: 0,
  duration: 30,
  enabled: true,
  serviceEnabled: true,
  code: '',
  amount: 0,
  minSpend: 0,
  maxClaims: 1,
  expiresAt: '',
  packageId: null as number | null,
  title: '',
  content: '',
  userId: null as number | null
})
const families = [
  'custom',
  'chatgpt',
  'claude',
  'gemini',
  'deepseek',
  'qwen',
  'kimi',
  'glm',
  'doubao',
  'image',
  'video',
  'audio',
  'embedding'
]
const tabTitle = computed(() => t(tabs.find((tab) => tab.value === active.value)!.key))
const rows = computed(() => {
  const data =
    active.value === 'packages'
      ? packages.value
      : active.value === 'coupons'
        ? coupons.value
        : active.value === 'redemptions'
          ? redemptions.value
          : notifications.value
  return data.map((row) => ({ ...row }))
})
const columns = computed<{ prop: string; key: MessageKey; width?: number }[]>(() => {
  if (active.value === 'packages')
    return [
      { prop: 'name', key: 'name' },
      { prop: 'family', key: 'opsFamily', width: 110 },
      { prop: 'models', key: 'opsModels' },
      { prop: 'price_micros', key: 'opsPrice', width: 110 },
      { prop: 'duration_days', key: 'opsDuration', width: 90 },
      { prop: 'enabled', key: 'opsOnSale', width: 90 },
      { prop: 'service_enabled', key: 'opsServiceEnabled', width: 90 }
    ]
  if (active.value === 'coupons')
    return [
      { prop: 'name', key: 'name' },
      { prop: 'code', key: 'opsCode' },
      { prop: 'amount_micros', key: 'amount' },
      { prop: 'min_spend_micros', key: 'opsMinSpend' },
      { prop: 'claimed_count', key: 'opsClaimed' },
      { prop: 'max_claims', key: 'opsQuantity' },
      { prop: 'expires_at', key: 'expiresAt' }
    ]
  if (active.value === 'redemptions')
    return [
      { prop: 'code', key: 'opsCode' },
      { prop: 'amount_micros', key: 'amount' },
      { prop: 'package_id', key: 'opsPackages' },
      { prop: 'redeemed_at', key: 'opsRedeemedAt' },
      { prop: 'expires_at', key: 'expiresAt' }
    ]
  return [
    { prop: 'title', key: 'opsTitle' },
    { prop: 'content', key: 'opsContent' },
    { prop: 'user_id', key: 'opsRecipient', width: 120 },
    { prop: 'created_at', key: 'opsPublishedAt', width: 180 }
  ]
})
function cell(row: Record<string, unknown>, prop: string) {
  const value = row[prop]
  if (prop.endsWith('_micros'))
    return formatMoney(Number(value || 0), billingCurrency.value, locale.value)
  if (prop.endsWith('_at')) return value ? formatDateTime(String(value), locale.value) : '—'
  if (prop === 'enabled') return t(value ? 'opsOnSale' : 'opsOffSale')
  if (prop === 'service_enabled') return t(value ? 'enabled' : 'disabled')
  if (prop === 'models')
    return Array.isArray(value) && value.length ? value.join(', ') : t('opsNoLiveModels')
  if (prop === 'user_id') return value ?? t('opsAllUsers')
  if (prop === 'package_id') return packages.value.find((item) => item.id === value)?.name || '—'
  return value ?? '—'
}
async function load() {
  loading.value = true
  try {
    if (active.value === 'packages') packages.value = await listAdminPackages()
    else if (active.value === 'coupons') coupons.value = await listAdminCoupons()
    else if (active.value === 'redemptions')
      [redemptions.value, packages.value] = await Promise.all([
        listAdminRedemptions(),
        listAdminPackages()
      ])
    else if (active.value === 'notifications') notifications.value = await listAdminNotifications()
    else Object.assign(community, await getAdminCommunity())
  } catch (err) {
    ElMessage.error(readError(err))
  } finally {
    loading.value = false
  }
}
function openForm(id?: number) {
  editingId.value = id
  Object.assign(form, {
    name: '',
    description: '',
    family: 'custom',
    models: '',
    price: 0,
    duration: 30,
    enabled: true,
    serviceEnabled: true,
    code: '',
    amount: 0,
    minSpend: 0,
    maxClaims: 1,
    expiresAt: '',
    packageId: null,
    title: '',
    content: '',
    userId: null
  })
  const item = id ? packages.value.find((item) => item.id === id) : null
  if (item)
    Object.assign(form, {
      name: item.name,
      description: item.description,
      family: item.family,
      models: (item.configured_models || item.models).join('\n'),
      price: microAmountToMajor(item.price_micros),
      duration: item.duration_days,
      enabled: item.enabled,
      serviceEnabled: item.service_enabled
    })
  dialog.value = true
}
async function submit() {
  saving.value = true
  try {
    const expires = form.expiresAt ? new Date(form.expiresAt).toISOString() : null
    if (active.value === 'packages')
      await saveAdminPackage(
        {
          name: form.name.trim(),
          description: form.description.trim(),
          family: form.family,
          models: [...new Set(form.models.split(/[\s,，]+/).filter(Boolean))],
          price_micros: majorToMicroAmount(form.price),
          duration_days: form.duration,
          enabled: form.enabled,
          service_enabled: form.serviceEnabled
        },
        editingId.value
      )
    else if (active.value === 'coupons')
      await createAdminCoupon({
        name: form.name.trim(),
        code: form.code.trim(),
        amount_micros: majorToMicroAmount(form.amount),
        min_spend_micros: majorToMicroAmount(form.minSpend),
        max_claims: form.maxClaims,
        expires_at: expires
      })
    else if (active.value === 'redemptions')
      await createAdminRedemption({
        code: form.code.trim(),
        amount_micros: majorToMicroAmount(form.amount),
        package_id: form.packageId,
        expires_at: expires
      })
    else if (active.value === 'notifications')
      await publishAdminNotification({
        title: form.title.trim(),
        content: form.content.trim(),
        user_id: form.userId
      })
    ElMessage.success(t('opsSaved'))
    dialog.value = false
    await load()
  } catch (err) {
    ElMessage.error(readError(err))
  } finally {
    saving.value = false
  }
}
async function saveCommunity() {
  saving.value = true
  try {
    Object.assign(community, await saveAdminCommunity(community))
    ElMessage.success(t('opsSaved'))
  } catch (err) {
    ElMessage.error(readError(err))
  } finally {
    saving.value = false
  }
}
async function removeNotification(id: number) {
  if (
    !(await confirm(t('opsDeleteNotification'), t('delete'), {
      confirmText: t('delete'),
      danger: true
    }))
  )
    return
  try {
    await deleteAdminNotification(id)
    await load()
  } catch (err) {
    ElMessage.error(readError(err))
  }
}
watch(active, load)
onMounted(load)
</script>

<template>
  <section class="user-operations-view">
    <el-tabs v-model="active">
      <el-tab-pane v-for="tab in tabs" :key="tab.value" :label="t(tab.key)" :name="tab.value" />
    </el-tabs>
    <div v-loading="loading" class="operations-card">
      <template v-if="active === 'community'">
        <p class="operations-hint">{{ t('opsCommunityHint') }}</p>
        <el-form label-position="top" class="operations-community" @submit.prevent="saveCommunity">
          <el-form-item :label="t('opsTitle')"
            ><el-input v-model="community.about_title" maxlength="120"
          /></el-form-item>
          <el-form-item :label="t('opsContent')"
            ><el-input
              v-model="community.about_content"
              type="textarea"
              :rows="5"
              maxlength="10000"
          /></el-form-item>
          <el-form-item :label="t('opsQQNumber')"
            ><el-input v-model="community.qq_group_number"
          /></el-form-item>
          <el-form-item :label="t('opsQQLink')"
            ><el-input v-model="community.qq_group_url" placeholder="https://qm.qq.com/..."
          /></el-form-item>
          <el-button type="primary" native-type="submit" :loading="saving">{{
            t('save')
          }}</el-button>
          <RouterLink class="operations-preview" to="/about">{{ t('opsPreview') }} ↗</RouterLink>
        </el-form>
      </template>
      <template v-else>
        <div class="operations-toolbar">
          <p>
            {{
              t(
                active === 'packages'
                  ? 'opsPackagesHint'
                  : active === 'coupons'
                    ? 'opsCouponsHint'
                    : active === 'redemptions'
                      ? 'opsRedemptionHint'
                      : 'opsNotificationsHint'
              )
            }}
          </p>
          <div>
            <el-button :icon="Refresh" @click="load">{{ t('refresh') }}</el-button
            ><el-button type="primary" :icon="Plus" @click="openForm()">{{
              t('create')
            }}</el-button>
          </div>
        </div>
        <el-table :data="rows" :empty-text="t('opsEmpty')" style="width: 100%">
          <el-table-column
            v-for="column in columns"
            :key="column.prop"
            :label="t(column.key)"
            :width="column.width"
            :min-width="column.width ? undefined : 140"
            show-overflow-tooltip
          >
            <template #default="scope">{{ cell(scope.row, column.prop) }}</template>
          </el-table-column>
          <el-table-column
            v-if="active === 'packages' || active === 'notifications'"
            :label="t('actions')"
            fixed="right"
            width="90"
          >
            <template #default="scope"
              ><el-button
                link
                :type="active === 'packages' ? 'primary' : 'danger'"
                @click="
                  active === 'packages' ? openForm(scope.row.id) : removeNotification(scope.row.id)
                "
                >{{ t(active === 'packages' ? 'edit' : 'delete') }}</el-button
              ></template
            >
          </el-table-column>
        </el-table>
      </template>
    </div>
    <el-dialog
      v-model="dialog"
      :title="`${t(editingId ? 'edit' : 'create')} · ${tabTitle}`"
      width="min(560px, calc(100vw - 32px))"
      :close-on-click-modal="!saving"
    >
      <el-form label-position="top" @submit.prevent="submit">
        <template v-if="active === 'packages'">
          <el-form-item :label="t('name')" required
            ><el-input v-model="form.name" maxlength="100"
          /></el-form-item>
          <el-form-item :label="t('description')"
            ><el-input v-model="form.description" type="textarea" maxlength="4000"
          /></el-form-item>
          <div class="operations-form-row">
            <el-form-item :label="t('opsFamily')"
              ><el-select v-model="form.family"
                ><el-option
                  v-for="family in families"
                  :key="family"
                  :label="family === 'custom' ? t('opsCustom') : family"
                  :value="family" /></el-select></el-form-item
            ><el-form-item :label="t('opsOnSale')"
              ><el-switch v-model="form.enabled"
            /></el-form-item>
          </div>
          <el-form-item :label="t('opsServiceEnabled')"
            ><el-switch v-model="form.serviceEnabled"
          /></el-form-item>
          <p class="operations-hint">{{ t('opsServiceHint') }}</p>
          <el-form-item :label="t('opsModels')" :required="form.family === 'custom'"
            ><el-input
              v-model="form.models"
              type="textarea"
              :rows="4"
              :placeholder="t('opsModelsHint')"
          /></el-form-item>
          <div class="operations-form-row">
            <el-form-item :label="`${t('opsPrice')} (${billingCurrency})`"
              ><el-input-number v-model="form.price" :min="0" :precision="2" /></el-form-item
            ><el-form-item :label="t('opsDuration')"
              ><el-input-number v-model="form.duration" :min="1" :max="3660" :precision="0"
            /></el-form-item>
          </div>
        </template>
        <template v-else-if="active === 'coupons' || active === 'redemptions'">
          <el-form-item v-if="active === 'coupons'" :label="t('name')" required
            ><el-input v-model="form.name" maxlength="100"
          /></el-form-item>
          <el-form-item :label="t('opsCode')" required
            ><el-input v-model="form.code" maxlength="100"
          /></el-form-item>
          <el-form-item :label="`${t('amount')} (${billingCurrency})`"
            ><el-input-number v-model="form.amount" :min="0" :precision="2"
          /></el-form-item>
          <template v-if="active === 'coupons'"
            ><div class="operations-form-row">
              <el-form-item :label="t('opsMinSpend')"
                ><el-input-number v-model="form.minSpend" :min="0" :precision="2" /></el-form-item
              ><el-form-item :label="t('opsQuantity')"
                ><el-input-number v-model="form.maxClaims" :min="1" :precision="0"
              /></el-form-item></div
          ></template>
          <el-form-item v-else :label="t('opsPackages')"
            ><el-select v-model="form.packageId" clearable @clear="form.packageId = null"
              ><el-option
                v-for="item in packages"
                :key="item.id"
                :label="item.name"
                :value="item.id" /></el-select
          ></el-form-item>
          <el-form-item :label="t('expiresAt')"
            ><el-date-picker
              v-model="form.expiresAt"
              type="datetime"
              value-format="YYYY-MM-DDTHH:mm:ss"
              clearable
          /></el-form-item>
        </template>
        <template v-else-if="active === 'notifications'">
          <el-form-item :label="t('opsTitle')" required
            ><el-input v-model="form.title" maxlength="120"
          /></el-form-item>
          <el-form-item :label="t('opsContent')" required
            ><el-input v-model="form.content" type="textarea" :rows="6" maxlength="10000"
          /></el-form-item>
          <el-form-item :label="t('opsRecipient')"
            ><el-input-number
              v-model="form.userId"
              :min="1"
              :precision="0"
              :placeholder="t('opsAllUsers')"
          /></el-form-item>
          <p class="operations-hint">{{ t('opsRecipientHint') }}</p>
        </template>
        <div class="operations-dialog-actions">
          <el-button :disabled="saving" @click="dialog = false">{{ t('cancel') }}</el-button
          ><el-button type="primary" native-type="submit" :loading="saving">{{
            t('save')
          }}</el-button>
        </div>
      </el-form>
    </el-dialog>
  </section>
</template>

<style scoped>
.operations-card {
  padding: 24px;
  border: 1px solid var(--app-border, #e8ebf2);
  background: #fff;
  border-radius: 12px;
}
.operations-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  margin-bottom: 24px;
}
.operations-toolbar p,
.operations-hint {
  color: #858da1;
  font-size: 13px;
  line-height: 1.8;
  margin: 0 0 16px;
}
.operations-toolbar p {
  margin: 0;
}
.operations-toolbar > div {
  display: flex;
  flex-shrink: 0;
}
.operations-community {
  max-width: 640px;
}
.operations-preview {
  margin-left: 18px;
  color: #5145ff;
  font-size: 13px;
  text-decoration: none;
}
.operations-form-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 20px;
}
.operations-form-row .el-input-number,
.operations-form-row .el-select {
  width: 100%;
}
.operations-dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 24px;
}
@media (max-width: 600px) {
  .operations-card {
    padding: 16px;
  }
  .operations-toolbar {
    align-items: flex-start;
    flex-direction: column;
  }
  .operations-form-row {
    grid-template-columns: 1fr;
    gap: 0;
  }
}
</style>
