<script setup lang="ts">
import { ref } from 'vue'
import { ElMessage } from 'element-plus'
import { claimCoupon, getCoupons, getAvailableCoupons, claimPublicCoupon } from '../../api/commerce'
import { useStudio } from '../../composables/useStudio'
import { couponState as state } from '../../utils/coupon'
import { useAsyncData } from '../../composables/useAsyncData'
import { useLocale } from '../../composables/useLocale'
import { useBillingCurrency } from '../../composables/useBillingCurrency'
import { readError } from '../../utils/errors'
import { formatCompactDateTime } from '../../utils/format'
const { locale, t } = useLocale()
const { s } = useStudio()
const { formatMoney } = useBillingCurrency()
const { data: coupons, loading, error, reload } = useAsyncData(getCoupons, [])
const {
  data: available,
  loading: availableLoading,
  error: availableError,
  reload: reloadAvailable
} = useAsyncData(getAvailableCoupons, [])
const claimingId = ref<number | null>(null)
async function claimAvailable(id: number) {
  if (claimingId.value !== null) return
  claimingId.value = id
  try {
    await claimPublicCoupon(id)
    ElMessage.success(t('couponsClaimed'))
    await Promise.all([reload(), reloadAvailable()])
  } catch (err) {
    ElMessage.error(readError(err))
  } finally {
    claimingId.value = null
  }
}
const code = ref('')
const claiming = ref(false)
async function claim() {
  if (!code.value.trim() || claiming.value) return
  claiming.value = true
  try {
    await claimCoupon(code.value.trim())
    code.value = ''
    ElMessage.success(t('couponsClaimed'))
    await reload()
    await reloadAvailable()
  } catch (error) {
    ElMessage.error(readError(error))
  } finally {
    claiming.value = false
  }
}
</script>
<template>
  <section class="commerce-view">
    <p class="commerce-intro">
      {{
        s(
          '平台发放的优惠券会自动到账，也可以在领券中心直接领取。',
          'Issued coupons arrive automatically. You can also claim public offers below.'
        )
      }}
    </p>
    <h3>{{ s('领券中心', 'Available offers') }}</h3>
    <el-alert v-if="availableError" :title="availableError" type="error" :closable="false" />
    <div v-loading="availableLoading" class="commerce-grid">
      <article v-for="coupon in available" :key="coupon.id" class="user-panel commerce-card">
        <div class="commerce-card-heading">
          <h3>{{ coupon.name }}</h3>
          <el-tag size="small">{{ s('可领取', 'Claim now') }}</el-tag>
        </div>
        <strong class="commerce-price">−{{ formatMoney(coupon.amount_micros, locale, 2) }}</strong>
        <p>
          {{
            coupon.min_spend_micros > 0
              ? t('couponsMinSpend', { amount: formatMoney(coupon.min_spend_micros, locale, 2) })
              : t('couponsNoMinSpend')
          }}
        </p>
        <p>
          {{
            coupon.expires_at
              ? `${t('plansExpires')} ${formatCompactDateTime(coupon.expires_at)}`
              : t('couponsNoExpiry')
          }}
        </p>
        <el-button
          type="primary"
          :loading="claimingId === coupon.id"
          :disabled="claimingId !== null"
          @click="claimAvailable(coupon.id)"
          >{{ s('立即领取', 'Claim coupon') }}</el-button
        >
      </article>
      <p v-if="!availableLoading && !availableError && !available.length" class="commerce-intro">
        {{ s('暂无公开优惠券', 'No public offers right now') }}
      </p>
    </div>
    <h3>{{ s('我的优惠券', 'My coupons') }}</h3>
    <form class="commerce-form" @submit.prevent="claim">
      <el-input
        v-model="code"
        maxlength="128"
        :placeholder="t('couponsCode')"
        :aria-label="t('couponsCode')"
      /><el-button
        native-type="submit"
        type="primary"
        :loading="claiming"
        :disabled="!code.trim()"
        >{{ t('couponsClaim') }}</el-button
      >
    </form>
    <el-alert v-if="error" type="error" :title="error" :closable="false" />
    <div v-loading="loading" class="commerce-grid">
      <article v-for="coupon in coupons" :key="coupon.id" class="user-panel commerce-card">
        <div class="commerce-card-heading">
          <h3>{{ coupon.name }}</h3>
          <el-tag :type="state(coupon) === 'available' ? 'success' : 'info'" size="small">{{
            t(
              state(coupon) === 'used'
                ? 'couponsUsed'
                : state(coupon) === 'expired'
                  ? 'couponsExpired'
                  : state(coupon) === 'disabled'
                    ? 'couponsDisabled'
                    : 'couponsAvailable'
            )
          }}</el-tag>
        </div>
        <strong class="commerce-price">−{{ formatMoney(coupon.amount_micros, locale, 2) }}</strong>
        <p>
          {{
            coupon.min_spend_micros > 0
              ? t('couponsMinSpend', { amount: formatMoney(coupon.min_spend_micros, locale, 2) })
              : t('couponsNoMinSpend')
          }}
        </p>
        <p>
          {{
            coupon.expires_at
              ? `${t('plansExpires')} ${formatCompactDateTime(coupon.expires_at)}`
              : t('couponsNoExpiry')
          }}
        </p>
        <RouterLink v-if="state(coupon) === 'available'" to="/home/packages"
          >{{ t('userPlans') }} →</RouterLink
        >
      </article>
    </div>
    <el-empty v-if="!loading && !error && !coupons.length" :description="t('couponsEmpty')" />
  </section>
</template>
