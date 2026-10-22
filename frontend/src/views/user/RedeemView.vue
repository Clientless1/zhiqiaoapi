<script setup lang="ts">
import { ref } from 'vue'
import { ElMessage } from 'element-plus'
import { getRedemptions, redeemCode, emptyPage, type Redemption } from '../../api/commerce'
import { useCommercePage } from '../../composables/useCommercePage'
import CommercePagination from '../../components/common/CommercePagination.vue'
import { useLocale } from '../../composables/useLocale'
import { useBillingCurrency } from '../../composables/useBillingCurrency'
import { readError } from '../../utils/errors'
import { formatCompactDateTime } from '../../utils/format'
const { locale, t } = useLocale()
const { formatMoney } = useBillingCurrency()
const {
  data: redemptions,
  loading,
  error,
  refresh,
  page,
  pageSize
} = useCommercePage(getRedemptions, emptyPage<Redemption>())
function reward(packageName: string | null | undefined, amount: number) {
  return [packageName, amount > 0 ? `+${formatMoney(amount, locale.value, 2)}` : '']
    .filter(Boolean)
    .join(' + ')
}
const code = ref('')
const redeeming = ref(false)
async function redeem() {
  if (!code.value.trim() || redeeming.value) return
  redeeming.value = true
  try {
    const result = await redeemCode(code.value.trim())
    code.value = ''
    ElMessage.success(
      `${t('redeemSuccess')} · ${reward(result.package_name, result.amount_micros)}`
    )
    await refresh()
  } catch (error) {
    ElMessage.error(readError(error))
  } finally {
    redeeming.value = false
  }
}
</script>
<template>
  <section class="commerce-view">
    <p class="commerce-intro">{{ t('redeemIntro') }}</p>
    <form class="commerce-form" @submit.prevent="redeem">
      <el-input
        v-model="code"
        maxlength="128"
        :placeholder="t('redeemPlaceholder')"
        :aria-label="t('redeemPlaceholder')"
      /><el-button
        native-type="submit"
        type="primary"
        :loading="redeeming"
        :disabled="!code.trim()"
        >{{ t('redeemAction') }}</el-button
      >
    </form>
    <el-alert v-if="error" type="error" :title="error" :closable="false" />
    <article class="user-panel commerce-panel">
      <h3>{{ t('redeemHistory') }}</h3>
      <el-table v-loading="loading" :data="redemptions.items" :empty-text="t('redeemEmpty')"
        ><el-table-column
          prop="code_hint"
          :label="t('userRedeem')"
          min-width="160"
        /><el-table-column :label="t('ordersName')" min-width="160"
          ><template #default="{ row }">{{
            reward(row.package_name, row.amount_micros)
          }}</template></el-table-column
        ><el-table-column :label="t('createdAt')" min-width="160"
          ><template #default="{ row }">{{
            formatCompactDateTime(row.created_at)
          }}</template></el-table-column
        ></el-table
      >
    </article>
    <CommercePagination
      v-model:page="page"
      v-model:page-size="pageSize"
      :total="redemptions.total"
      :loading="loading"
    />
  </section>
</template>
