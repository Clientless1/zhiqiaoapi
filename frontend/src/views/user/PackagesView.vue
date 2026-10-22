<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import {
  getCoupons,
  getPackages,
  getPurchaseReceipt,
  purchasePackage,
  type ModelPackage
} from '../../api/commerce'
import { useAsyncData } from '../../composables/useAsyncData'
import { useLocale } from '../../composables/useLocale'
import { useBillingCurrency } from '../../composables/useBillingCurrency'
import { ApiError, readError } from '../../utils/errors'
import { couponState } from '../../utils/coupon'
import {
  purchaseIntent,
  loadPurchaseIntent,
  savePurchaseIntent,
  confirmPurchaseIntent,
  type PurchaseIntent
} from '../../utils/purchaseIntent'
import { useAuthStore } from '../../stores/auth'
import { formatCompactDateTime } from '../../utils/format'
const { locale, t } = useLocale()
const { formatMoney } = useBillingCurrency()
const router = useRouter()
const auth = useAuthStore()
const dialogVisible = ref(false)
const recovering = ref(false)
const { data: packages, loading, error, reload } = useAsyncData(getPackages, [])
const { data: coupons, reload: reloadCoupons } = useAsyncData(getCoupons, [])
const chosen = ref<ModelPackage | null>(null)
const couponId = ref<number | undefined>()
const buying = ref(false)
const intent = ref<PurchaseIntent | null>(null)
const resultUnknown = ref(false)
const validCoupons = computed(() =>
  coupons.value.filter(
    (c) => couponState(c) === 'available' && c.min_spend_micros <= (chosen.value?.price_micros ?? 0)
  )
)
const payable = computed(() =>
  resultUnknown.value && intent.value?.payableMicros != null
    ? intent.value.payableMicros
    : Math.max(
        0,
        (chosen.value?.price_micros ?? intent.value?.priceMicros ?? 0) -
          (validCoupons.value.find((c) => c.id === couponId.value)?.amount_micros ?? 0)
      )
)
function userId() {
  const id = auth.currentUser?.id
  if (!id) throw new Error(t('plansStorageRequired'))
  return id
}
function clearIntent() {
  savePurchaseIntent(userId(), null)
  intent.value = null
  resultUnknown.value = false
}
async function completePurchase() {
  clearIntent()
  chosen.value = null
  dialogVisible.value = false
  ElMessage.success(t('plansBuySuccess'))
  await Promise.allSettled([reload(), reloadCoupons()])
}
async function recoverPurchase() {
  if (recovering.value || buying.value) return
  recovering.value = true
  try {
    const pending = loadPurchaseIntent(userId())
    if (!pending) return
    intent.value = pending
    couponId.value = pending.couponId
    resultUnknown.value = true
    chosen.value = packages.value.find((item) => item.id === pending.packageId) ?? null
    dialogVisible.value = true
    try {
      await getPurchaseReceipt(pending.requestId)
      await completePurchase()
    } catch (error) {
      if (!(error instanceof ApiError && error.status === 404)) throw error
      // The original request may still be committing. Only retry its original ID.
    }
  } catch (error) {
    ElMessage.error(readError(error))
  } finally {
    recovering.value = false
  }
}
async function choose(plan: ModelPackage) {
  try {
    if (loadPurchaseIntent(userId())) {
      await recoverPurchase()
      return
    }
    chosen.value = plan
    couponId.value = undefined
    intent.value = {
      ...purchaseIntent(null, plan.id),
      packageName: plan.name,
      priceMicros: plan.price_micros
    }
    resultUnknown.value = false
    dialogVisible.value = true
  } catch {
    ElMessage.error(t('plansStorageRequired'))
  }
}
watch(couponId, (value) => {
  if (chosen.value && !resultUnknown.value && !buying.value)
    intent.value = {
      ...purchaseIntent(intent.value, chosen.value.id, value),
      packageName: chosen.value.name,
      priceMicros: chosen.value.price_micros
    }
})
watch(packages, (plans) => {
  if (intent.value && resultUnknown.value)
    chosen.value = plans.find((plan) => plan.id === intent.value?.packageId) ?? null
})
function updatePurchaseVisibility(visible: boolean) {
  if (!buying.value && !recovering.value) dialogVisible.value = visible
}
async function buy() {
  if (!intent.value || buying.value || recovering.value) return
  buying.value = true
  try {
    // Persist before sending: closing, reloading, and changing accounts must not lose the ID.
    const previous = loadPurchaseIntent(userId())
    if (previous && previous.requestId !== intent.value.requestId) {
      intent.value = previous
      couponId.value = previous.couponId
      chosen.value = packages.value.find((plan) => plan.id === previous.packageId) ?? null
      resultUnknown.value = true
      ElMessage.warning(t('plansPending'))
      return
    }
    if (!resultUnknown.value) intent.value.payableMicros = payable.value
    savePurchaseIntent(userId(), intent.value)
    resultUnknown.value = true
    await confirmPurchaseIntent(intent.value, getPurchaseReceipt, (pending) =>
      purchasePackage(pending.packageId, pending.requestId, pending.couponId)
    )
    await completePurchase()
  } catch (error) {
    // A definitive validation failure has no charge; transport/server errors stay recoverable.
    if (
      error instanceof ApiError &&
      error.status >= 400 &&
      error.status < 500 &&
      ![408, 409, 429].includes(error.status)
    ) {
      try {
        clearIntent()
        dialogVisible.value = false
      } catch {
        /* Preserve the original error. */
      }
    }
    ElMessage.error(readError(error))
  } finally {
    buying.value = false
  }
}
onMounted(() => void recoverPurchase())
</script>
<template>
  <section class="commerce-view">
    <div class="commerce-toolbar">
      <p class="commerce-intro">{{ t('plansIntro') }}</p>
      <el-button :loading="loading" @click="reload()">{{ t('dashboardRefresh') }}</el-button>
    </div>
    <el-alert v-if="error" type="error" :title="error" :closable="false" />
    <el-alert v-if="resultUnknown" type="warning" :closable="false" :title="t('plansPending')"
      ><el-button text type="primary" :loading="recovering" @click="recoverPurchase">{{
        t('plansResume')
      }}</el-button></el-alert
    >
    <div v-loading="loading" class="commerce-grid">
      <article v-for="plan in packages" :key="plan.id" class="user-panel commerce-card">
        <div class="commerce-card-heading">
          <h3>{{ plan.name }}</h3>
          <el-tag v-if="plan.active && plan.price_micros > 0" type="success" size="small">{{
            t('plansPurchased')
          }}</el-tag
          ><el-tag v-else size="small">{{ plan.family }}</el-tag>
        </div>
        <el-tag v-if="!plan.service_enabled" type="danger" size="small">{{
          t('plansServiceStopped')
        }}</el-tag
        ><el-tag v-else-if="!plan.enabled" type="info" size="small">{{ t('plansOffSale') }}</el-tag>
        <p>{{ plan.description }}</p>
        <strong class="commerce-price">{{
          plan.price_micros === 0 ? t('plansMetered') : formatMoney(plan.price_micros, locale, 2)
        }}</strong>
        <p>
          {{
            plan.price_micros === 0
              ? t('plansMeteredHint')
              : t('plansDays', { days: plan.duration_days })
          }}
        </p>
        <p v-if="plan.price_micros > 0">{{ t('plansAccessFeeHint') }}</p>
        <span class="commerce-label">{{ t('plansModels') }} · {{ plan.models.length }}</span>
        <div class="commerce-tags">
          <span v-for="model in plan.models" :key="model" class="commerce-model">{{ model }}</span>
          <p v-if="!plan.models.length">{{ t('plansNoModels') }}</p>
        </div>
        <p v-if="plan.expires_at">
          {{ t('plansExpires') }} {{ formatCompactDateTime(plan.expires_at) }}
        </p>
        <el-button
          v-if="plan.active"
          type="primary"
          class="card-action"
          :disabled="!plan.models.length || !plan.service_enabled"
          @click="router.push({ path: '/home/apikeys', query: { package: plan.id } })"
          >{{ t('plansUse') }}</el-button
        >
        <el-button
          v-else
          type="primary"
          class="card-action"
          :disabled="!plan.models.length || !plan.service_enabled || !plan.enabled"
          @click="choose(plan)"
          >{{ t('plansBuy') }}</el-button
        >
      </article>
    </div>
    <el-empty v-if="!loading && !error && !packages.length" :description="t('plansEmpty')" />
    <el-dialog
      :model-value="dialogVisible"
      :title="t('plansPurchaseTitle')"
      width="440px"
      :close-on-click-modal="!buying && !recovering"
      :close-on-press-escape="!buying && !recovering"
      :show-close="!buying && !recovering"
      @update:model-value="updatePurchaseVisibility"
    >
      <div v-if="intent" class="commerce-view">
        <h3>{{ chosen?.name || intent.packageName || t('plansPurchaseTitle') }}</h3>
        <p class="commerce-intro">{{ t('plansAccessFeeHint') }}</p>
        <el-form label-position="top"
          ><el-form-item :label="t('plansCoupon')"
            ><el-select
              v-model="couponId"
              :disabled="buying || resultUnknown"
              clearable
              :placeholder="t('plansNoCoupon')"
              style="width: 100%"
              ><el-option
                v-for="coupon in validCoupons"
                :key="coupon.id"
                :value="coupon.id"
                :label="`${coupon.name} −${formatMoney(coupon.amount_micros, locale, 2)}`" /></el-select></el-form-item
        ></el-form>
        <p class="commerce-notice">
          {{ t('plansBalancePayment') }} <strong>{{ formatMoney(payable, locale, 2) }}</strong>
        </p>
        <p v-if="resultUnknown" class="commerce-intro">{{ t('plansRetryHint') }}</p>
      </div>
      <template #footer
        ><el-button :disabled="buying || recovering" @click="dialogVisible = false">{{
          t('cancel')
        }}</el-button
        ><el-button type="primary" :loading="buying || recovering" @click="buy">{{
          t('plansConfirmBuy')
        }}</el-button></template
      >
    </el-dialog>
  </section>
</template>
