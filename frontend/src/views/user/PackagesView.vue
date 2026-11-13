<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import {
  getCoupons,
  getPackages,
  getPackageGrants,
  type PackageGrant,
  getPurchaseReceipt,
  purchasePackage,
  type ModelPackage
} from '../../api/commerce'
import ProviderIcon from '../../components/common/ProviderIcon.vue'
import { getUserOverview } from '../../api/overview'
import { useStudio } from '../../composables/useStudio'
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
const { s } = useStudio()
const router = useRouter()
const tab = ref<'quota' | 'metered'>('quota')
const { data: grants, error: grantError, reload: reloadGrants } = useAsyncData(getPackageGrants, [])
const {
  data: overview,
  error: balanceError,
  reload: reloadBalance
} = useAsyncData(getUserOverview, null)
const activeGrants = computed(() => grants.value.filter((g) => g.active))
const historyOpen = ref(false)
const shownGrants = computed(() => (historyOpen.value ? grants.value : activeGrants.value))
function allowance(plan: ModelPackage) {
  return [
    plan.total_quota_micros
      ? `${s('总额度', 'Total')} ${formatMoney(plan.total_quota_micros, locale.value, 2)}`
      : '',
    plan.daily_quota_micros
      ? `${formatMoney(plan.daily_quota_micros, locale.value, 2)} / ${s('天', 'day')}`
      : '',
    plan.weekly_quota_micros
      ? `${formatMoney(plan.weekly_quota_micros, locale.value, 2)} / ${s('周', 'week')}`
      : ''
  ].filter(Boolean)
}
function meters(g: PackageGrant) {
  return [
    {
      label: s('总额度', 'Total'),
      limit: g.total_quota_micros,
      used: g.total_used_micros + g.reserved_micros
    },
    { label: s('今日', 'Today'), limit: g.daily_quota_micros, used: g.daily_used_micros },
    { label: s('本周', 'This week'), limit: g.weekly_quota_micros, used: g.weekly_used_micros }
  ].filter((m): m is { label: string; limit: number; used: number } => m.limit !== null)
}
async function refresh() {
  await Promise.allSettled([reload(), reloadGrants(), reloadBalance(), reloadCoupons()])
}
function usePlan(id: number) {
  return router.push({ path: '/home/apikeys', query: { package: id } })
}
const auth = useAuthStore()
const dialogVisible = ref(false)
const recovering = ref(false)
const { data: packages, loading, error, reload } = useAsyncData(getPackages, [])
const { data: coupons, reload: reloadCoupons } = useAsyncData(getCoupons, [])
const chosen = ref<ModelPackage | null>(null)
const visiblePackages = computed(() =>
  packages.value.filter(
    (p) => p.billing_mode === tab.value && p.models.length > 0 && p.service_enabled && p.enabled
  )
)
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
  await refresh()
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
  <section class="plan-center">
    <header class="plan-header">
      <div>
        <span class="plan-eyebrow">PLANS & BILLING</span>
        <h1>{{ s('套餐中心', 'Package center') }}</h1>
        <p>
          {{
            s('按需选购额度，为每一次调用留足空间。', 'Prepaid allowances for your next project.')
          }}
        </p>
      </div>
      <div class="plan-wallet">
        <span>{{ s('可用余额', 'Available balance') }}</span
        ><strong>{{ overview ? formatMoney(overview.available_micros, locale, 2) : '—' }}</strong
        ><RouterLink to="/home/recharge">{{ s('充值', 'Top up') }} ↗</RouterLink>
      </div>
    </header>
    <el-alert
      v-if="error || grantError || balanceError"
      type="error"
      :title="error || grantError || balanceError || ''"
      :closable="false"
    />
    <el-alert v-if="resultUnknown" type="warning" :closable="false" :title="t('plansPending')"
      ><el-button text type="primary" :loading="recovering" @click="recoverPurchase">{{
        t('plansResume')
      }}</el-button></el-alert
    >
    <section v-if="grants.length" class="plan-owned">
      <div class="plan-section-head">
        <h2>
          {{ s('我的套餐', 'My packages') }}
          <small>{{ activeGrants.length }} {{ s('份生效中', 'active') }}</small>
        </h2>
        <el-checkbox v-model="historyOpen">{{ s('显示历史', 'Show history') }}</el-checkbox>
      </div>
      <div class="grant-grid">
        <article
          v-for="g in shownGrants"
          :key="g.id"
          class="grant-card"
          :class="{ expired: !g.active }"
        >
          <div class="plan-section-head">
            <h3>{{ g.package_name }}</h3>
            <el-tag
              :type="!g.active ? 'info' : g.available_micros > 0 ? 'success' : 'warning'"
              size="small"
              >{{
                !g.active
                  ? s('已到期 / 已停用', 'Expired / disabled')
                  : g.available_micros > 0
                    ? s('生效中', 'Active')
                    : s('额度已用完', 'Allowance used')
              }}</el-tag
            >
          </div>
          <div v-for="m in meters(g)" :key="m.label" class="grant-meter">
            <div>
              <span>{{ m.label }}</span
              ><span
                >{{ formatMoney(Math.max(0, m.limit - m.used), locale, 2) }} /
                {{ formatMoney(m.limit, locale, 2) }}</span
              >
            </div>
            <el-progress
              :percentage="Math.min(100, Math.max(0, (m.used / m.limit) * 100))"
              :show-text="false"
              :stroke-width="5"
            />
          </div>
          <footer>
            <span>{{ formatCompactDateTime(g.expires_at) }} {{ s('到期', 'expires') }}</span
            ><el-button v-if="g.active" text type="primary" @click="usePlan(g.package_id)"
              >{{ s('创建密钥', 'Create key') }} ↗</el-button
            >
          </footer>
        </article>
      </div>
      <p class="plan-footnote">
        {{
          s(
            '显示剩余 / 上限，包含调用中的预占额度。日限额于北京时间 00:00 重置，周限额于周一 00:00 重置。',
            'Remaining / allowance, including in-flight reservations. Daily resets at 00:00 and weekly on Monday, Asia/Shanghai.'
          )
        }}
      </p>
    </section>
    <div class="plan-section-head plan-shop-head">
      <div class="plan-tabs" role="tablist" :aria-label="s('计费方式', 'Billing mode')">
        <button
          role="tab"
          :aria-selected="tab === 'quota'"
          :class="{ selected: tab === 'quota' }"
          @click="tab = 'quota'"
        >
          {{ s('额度套餐', 'Prepaid packages') }}</button
        ><button
          role="tab"
          :aria-selected="tab === 'metered'"
          :class="{ selected: tab === 'metered' }"
          @click="tab = 'metered'"
        >
          {{ s('按量计费', 'Pay as you go') }}
        </button>
      </div>
      <el-button :loading="loading" @click="refresh">{{ t('dashboardRefresh') }}</el-button>
    </div>
    <p class="plan-tab-intro">
      {{
        tab === 'quota'
          ? s(
              '购买后即时生效。重复购买叠加额度，每份独立到期。',
              'Active immediately. Each purchase stacks allowances with its own expiry.'
            )
          : s(
              '调用时按模型价格扣除账户余额。',
              'Usage is charged to your account balance at model rates.'
            )
      }}
    </p>
    <div v-loading="loading" class="plan-shop-grid">
      <article v-for="plan in visiblePackages" :key="plan.id" class="plan-product">
        <div class="plan-product-top">
          <ProviderIcon :provider="plan.family === 'chatgpt' ? 'openai' : plan.family" /><el-tag
            v-if="plan.active_grants"
            size="small"
            type="success"
            >{{ s('已购', 'Owned') }} × {{ plan.active_grants }}</el-tag
          ><span v-else class="plan-family">{{ plan.family }}</span>
        </div>
        <h3>{{ plan.name }}</h3>
        <p class="plan-description">
          {{
            plan.description ||
            (plan.billing_mode === 'quota'
              ? s('一次购买，专属调用额度。', 'Your own prepaid API allowance.')
              : s('灵活使用，按实际消耗付费。', 'Flexible usage, billed as you go.'))
          }}
        </p>
        <div class="plan-price">
          <strong>{{
            plan.price_micros
              ? formatMoney(plan.price_micros, locale, 2)
              : plan.billing_mode === 'quota'
                ? s('免费', 'Free')
                : s('按量付费', 'Pay as you go')
          }}</strong
          ><span v-if="plan.billing_mode === 'quota' || plan.price_micros"
            >/ {{ plan.duration_days }} {{ s('天', 'days') }}</span
          >
        </div>
        <ul class="plan-benefits">
          <li v-for="line in allowance(plan)" :key="line"><span>✓</span>{{ line }}</li>
          <li><span>✓</span>{{ plan.models.length }} {{ s('个可用模型', 'available models') }}</li>
          <li v-if="plan.billing_mode === 'quota'">
            <span>✓</span>{{ s('额度内调用不再扣余额', 'No wallet charge for package usage') }}
          </li>
          <li v-else>
            <span>✓</span>{{ s('按模型定价结算调用费用', 'Usage billed at model rates') }}
          </li>
        </ul>
        <details class="plan-models">
          <summary>{{ s('支持的模型', 'Supported models') }}</summary>
          <div>
            <span v-for="m in plan.models" :key="m">{{ m }}</span>
          </div>
        </details>
        <p v-if="plan.billing_mode === 'metered' && plan.price_micros" class="plan-footnote">
          {{
            s('此为访问资格费用，调用仍按量扣费。', 'Access fee only; usage is billed separately.')
          }}
        </p>
        <div class="plan-product-actions">
          <el-button
            v-if="plan.billing_mode === 'quota' || !plan.active"
            type="primary"
            :disabled="!!resultUnknown"
            @click="choose(plan)"
            >{{
              plan.active_grants ? s('再买一份', 'Buy another') : s('立即购买', 'Buy package')
            }}</el-button
          ><el-button
            v-if="plan.active"
            :type="plan.billing_mode === 'metered' ? 'primary' : 'default'"
            @click="usePlan(plan.id)"
            >{{ s('创建密钥', 'Create key') }}</el-button
          >
        </div>
      </article>
    </div>
    <div v-if="!loading && !error && !visiblePackages.length" class="plan-empty">
      <h3>{{ s('暂无上架的额度套餐', 'No prepaid packages yet') }}</h3>
      <p>
        {{
          s(
            '你仍可选择按量计费，使用账户余额调用模型。',
            'Use pay as you go with your account balance.'
          )
        }}
      </p>
      <el-button @click="tab = 'metered'">{{ s('查看按量计费', 'View pay as you go') }}</el-button>
    </div>
    <div class="plan-guide">
      <div>
        <b>01</b>
        <h3>{{ s('选购套餐', 'Buy a package') }}</h3>
        <p>{{ s('余额支付，可使用优惠券。', 'Pay with balance and apply a coupon.') }}</p>
      </div>
      <div>
        <b>02</b>
        <h3>{{ s('创建专属密钥', 'Create a package key') }}</h3>
        <p>
          {{
            s('选择已购套餐，直接导入 CC Switch。', 'Select your package and import to CC Switch.')
          }}
        </p>
      </div>
      <div>
        <b>03</b>
        <h3>{{ s('随时核对用量', 'Track your usage') }}</h3>
        <p>
          {{
            s(
              '此页查看余量，日志查看逐笔消耗。',
              'Check allowances here and details in request logs.'
            )
          }}
        </p>
      </div>
    </div>
    <el-dialog
      :model-value="dialogVisible"
      :title="t('plansPurchaseTitle')"
      width="min(460px, calc(100vw - 32px))"
      :close-on-click-modal="!buying && !recovering"
      :close-on-press-escape="!buying && !recovering"
      :show-close="!buying && !recovering"
      @update:model-value="updatePurchaseVisibility"
    >
      <div v-if="intent" class="plan-checkout">
        <h3>{{ chosen?.name || intent.packageName }}</h3>
        <ul v-if="chosen" class="plan-benefits">
          <li v-for="line in allowance(chosen)" :key="line">{{ line }}</li>
        </ul>
        <p>
          {{
            chosen?.billing_mode === 'quota'
              ? s(
                  `生效 ${chosen.duration_days} 天。每次购买单独计时，不延长旧套餐。`,
                  `Active for ${chosen.duration_days} days. Existing packages keep their expiry.`
                )
              : t('plansAccessFeeHint')
          }}
        </p>
        <el-form label-position="top"
          ><el-form-item :label="t('plansCoupon')"
            ><el-select
              v-model="couponId"
              :disabled="buying || resultUnknown"
              clearable
              :placeholder="t('plansNoCoupon')"
              style="width: 100%"
              ><el-option
                v-for="c in validCoupons"
                :key="c.id"
                :value="c.id"
                :label="`${c.name} −${formatMoney(c.amount_micros, locale, 2)}`" /></el-select></el-form-item
        ></el-form>
        <div class="plan-checkout-total">
          <span>{{ t('plansBalancePayment') }}</span
          ><strong>{{ formatMoney(payable, locale, 2) }}</strong>
        </div>
        <p v-if="resultUnknown">{{ t('plansRetryHint') }}</p>
        <p v-else-if="overview && overview.available_micros < payable" class="plan-insufficient">
          {{ s('余额不足，请先充值。', 'Insufficient balance. Please top up.') }}
          <RouterLink to="/home/recharge">{{ s('去充值', 'Top up') }} ↗</RouterLink>
        </p>
      </div>
      <template #footer
        ><el-button :disabled="buying || recovering" @click="dialogVisible = false">{{
          t('cancel')
        }}</el-button
        ><el-button
          type="primary"
          :loading="buying || recovering"
          :disabled="!resultUnknown && !!overview && overview.available_micros < payable"
          @click="buy"
          >{{ t('plansConfirmBuy') }}</el-button
        ></template
      >
    </el-dialog>
  </section>
</template>
<style scoped>
.plan-center {
  max-width: 1200px;
  margin: 0 auto;
  display: grid;
  gap: 24px;
  color: var(--el-text-color-primary);
}
.plan-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 24px;
  padding: 8px 0 12px;
}
.plan-eyebrow {
  font-size: 10px;
  letter-spacing: 0.15em;
  color: var(--el-text-color-secondary);
  font-weight: 600;
}
.plan-header h1 {
  font-size: 28px;
  letter-spacing: -0.8px;
  margin: 10px 0;
}
.plan-header p,
.plan-tab-intro {
  font-size: 13px;
  color: var(--el-text-color-secondary);
  margin: 0;
}
.plan-wallet {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 8px 24px;
  border-left: 1px solid var(--el-border-color-light);
  padding-left: 30px;
  min-width: 160px;
}
.plan-wallet > span {
  font-size: 12px;
  grid-column: 1/-1;
  color: var(--el-text-color-secondary);
}
.plan-wallet strong {
  font-size: 24px;
  font-variant-numeric: tabular-nums;
}
.plan-wallet a {
  align-self: center;
  font-size: 12px;
  color: var(--el-color-primary);
  text-decoration: none;
}
.plan-section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.plan-section-head h2 {
  font-size: 16px;
  margin: 0;
}
.plan-section-head h3 {
  font-size: 14px;
  margin: 0;
}
.plan-section-head small {
  font-size: 11px;
  font-weight: 400;
  color: var(--el-text-color-secondary);
  margin-left: 10px;
}
.plan-owned {
  display: grid;
  gap: 16px;
}
.grant-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: 16px;
}
.grant-card {
  border: 1px solid var(--el-border-color-light);
  padding: 20px;
  border-radius: 14px;
  background: var(--el-bg-color);
}
.grant-card.expired {
  opacity: 0.65;
}
.grant-meter {
  margin-top: 18px;
}
.grant-meter > div:first-child {
  display: flex;
  justify-content: space-between;
  font-size: 11px;
  color: var(--el-text-color-secondary);
  margin-bottom: 8px;
  font-variant-numeric: tabular-nums;
}
.grant-card footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 16px;
  font-size: 11px;
  color: var(--el-text-color-secondary);
}
.plan-footnote {
  font-size: 11px;
  line-height: 1.7;
  color: var(--el-text-color-secondary);
  margin: 0;
}
.plan-shop-head {
  padding-top: 4px;
}
.plan-tabs {
  display: flex;
  padding: 4px;
  border: 1px solid var(--el-border-color-light);
  border-radius: 10px;
  background: var(--el-fill-color-lighter);
  gap: 3px;
}
.plan-tabs button {
  border: 0;
  background: transparent;
  color: var(--el-text-color-secondary);
  padding: 8px 16px;
  border-radius: 7px;
  font: inherit;
  font-size: 13px;
  cursor: pointer;
}
.plan-tabs button.selected {
  color: var(--el-color-primary);
  background: var(--el-bg-color);
  box-shadow: 0 1px 4px #13274812;
}
.plan-tabs button:focus-visible {
  outline: 2px solid var(--el-color-primary);
}
.plan-tab-intro {
  margin-top: -12px;
  line-height: 1.7;
}
.plan-shop-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 20px;
}
.plan-product {
  display: flex;
  flex-direction: column;
  padding: 24px;
  border: 1px solid var(--el-border-color-light);
  border-radius: 16px;
  background: var(--el-bg-color);
  transition:
    border-color 0.18s,
    box-shadow 0.18s;
}
.plan-product:hover {
  border-color: var(--el-color-primary-light-5);
  box-shadow: 0 8px 28px #152a4e08;
}
.plan-product-top {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}
.plan-family {
  font-size: 10px;
  color: var(--el-text-color-secondary);
  text-transform: uppercase;
  letter-spacing: 0.1em;
}
.plan-product h3 {
  font-size: 17px;
  letter-spacing: -0.3px;
  margin: 0 0 10px;
}
.plan-description {
  font-size: 12px;
  line-height: 1.8;
  color: var(--el-text-color-secondary);
  margin: 0 0 20px;
  white-space: pre-line;
  overflow-wrap: anywhere;
}
.plan-price {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin-bottom: 20px;
}
.plan-price strong {
  font-size: 27px;
  letter-spacing: -0.9px;
  font-weight: 650;
}
.plan-price > span {
  font-size: 11px;
  color: var(--el-text-color-secondary);
}
.plan-benefits {
  list-style: none;
  padding: 0;
  margin: 0 0 18px;
  display: grid;
  gap: 12px;
  font-size: 12px;
}
.plan-benefits li {
  display: flex;
  gap: 9px;
  line-height: 1.5;
}
.plan-benefits li > span {
  color: var(--el-color-primary);
}
.plan-models {
  border-top: 1px solid var(--el-border-color-lighter);
  padding: 14px 0;
  margin-bottom: 12px;
  font-size: 11px;
  color: var(--el-text-color-secondary);
}
.plan-models summary {
  cursor: pointer;
}
.plan-models > div {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 12px;
  max-height: 150px;
  overflow: auto;
}
.plan-models > div > span {
  padding: 4px 6px;
  background: var(--el-fill-color-light);
  border-radius: 4px;
  overflow-wrap: anywhere;
  max-width: 100%;
}
.plan-product-actions {
  display: flex;
  gap: 8px;
  margin-top: auto;
  padding-top: 12px;
}
.plan-product-actions .el-button {
  flex: 1;
  margin-left: 0;
}
.plan-empty {
  text-align: center;
  border: 1px dashed var(--el-border-color);
  border-radius: 16px;
  padding: 40px 20px;
}
.plan-empty h3 {
  font-size: 16px;
}
.plan-empty p {
  font-size: 13px;
  color: var(--el-text-color-secondary);
  line-height: 1.8;
}
.plan-guide {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 28px;
  padding: 24px 0;
  border-top: 1px solid var(--el-border-color-light);
}
.plan-guide b {
  font-size: 11px;
  color: var(--el-color-primary);
}
.plan-guide h3 {
  font-size: 13px;
  margin: 10px 0 8px;
}
.plan-guide p {
  font-size: 12px;
  line-height: 1.7;
  color: var(--el-text-color-secondary);
  margin: 0;
}
.plan-checkout {
  display: grid;
  gap: 16px;
}
.plan-checkout h3 {
  margin: 0;
  font-size: 18px;
}
.plan-checkout p {
  margin: 0;
  font-size: 12px;
  line-height: 1.8;
  color: var(--el-text-color-secondary);
}
.plan-checkout-total {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: var(--el-fill-color-light);
  padding: 16px;
  border-radius: 10px;
  font-size: 12px;
}
.plan-checkout-total strong {
  font-size: 22px;
}
.plan-insufficient a {
  color: var(--el-color-primary);
}
@media (max-width: 1050px) {
  .plan-shop-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
@media (max-width: 650px) {
  .plan-header {
    align-items: flex-start;
    flex-direction: column;
  }
  .plan-wallet {
    padding: 14px 0 0;
    border-left: 0;
    border-top: 1px solid var(--el-border-color-light);
    width: 100%;
  }
  .plan-header h1 {
    font-size: 24px;
  }
  .plan-shop-grid,
  .plan-guide,
  .grant-grid {
    grid-template-columns: minmax(0, 1fr);
  }
  .plan-product {
    padding: 20px;
  }
  .plan-guide {
    gap: 20px;
  }
  .plan-section-head small {
    display: block;
    margin: 6px 0 0;
  }
}
@media (prefers-reduced-motion: reduce) {
  .plan-product {
    transition: none;
  }
}
</style>
