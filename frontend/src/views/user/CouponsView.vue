<script setup lang="ts">
import { ref } from 'vue'
import { ElMessage } from 'element-plus'
import { claimCoupon, getCoupons } from '../../api/commerce'
import { couponState as state } from '../../utils/coupon'
import { useAsyncData } from '../../composables/useAsyncData'
import { useLocale } from '../../composables/useLocale'
import { useBillingCurrency } from '../../composables/useBillingCurrency'
import { readError } from '../../utils/errors'
import { formatCompactDateTime } from '../../utils/format'
const { locale, t } = useLocale()
const { formatMoney } = useBillingCurrency()
const { data: coupons, loading, error, reload } = useAsyncData(getCoupons, [])
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
  } catch (error) {
    ElMessage.error(readError(error))
  } finally {
    claiming.value = false
  }
}
</script>
<template>
  <section class="commerce-view">
    <p class="commerce-intro">{{ t('couponsIntro') }}</p>
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
