<script setup lang="ts">
import { ref } from 'vue'
import { getOrders, emptyPage, type UserOrder } from '../../api/commerce'
import { cancelRechargeOrder } from '../../api/recharge'
import { useCommercePage } from '../../composables/useCommercePage'
import CommercePagination from '../../components/common/CommercePagination.vue'
import { ElMessage } from 'element-plus'
import { readError } from '../../utils/errors'
import { useLocale } from '../../composables/useLocale'
import { useBillingCurrency } from '../../composables/useBillingCurrency'
import { formatCompactDateTime } from '../../utils/format'
const { locale, t } = useLocale()
const { formatMoney } = useBillingCurrency()
const {
  data: orders,
  loading,
  error,
  reload,
  page,
  pageSize,
  filter: orderType
} = useCommercePage(getOrders, emptyPage<UserOrder>())
const canceling = ref('')
async function cancelOrder(id: string) {
  if (canceling.value) return
  canceling.value = id
  try {
    await cancelRechargeOrder(id)
    ElMessage.success(t('ordersCancelSuccess'))
  } catch (error) {
    ElMessage.error(readError(error))
  } finally {
    canceling.value = ''
    await reload()
  }
}
function statusLabel(status: string) {
  return t(
    status === 'paid' || status === 'completed'
      ? 'ordersPaid'
      : status === 'pending'
        ? 'ordersPending'
        : status === 'failed'
          ? 'ordersFailed'
          : status === 'expired'
            ? 'ordersExpired'
            : 'ordersCanceled'
  )
}
function checkoutUrl(value?: string | null) {
  try {
    const url = new URL(value ?? '')
    return ['http:', 'https:'].includes(url.protocol) ? url.href : ''
  } catch {
    return ''
  }
}
</script>
<template>
  <section class="commerce-view">
    <div class="commerce-toolbar">
      <p class="commerce-intro">{{ t('ordersIntro') }}</p>
      <el-button :loading="loading" @click="reload()">{{ t('dashboardRefresh') }}</el-button>
    </div>
    <el-alert v-if="error" type="error" :title="error" :closable="false" />
    <el-radio-group v-model="orderType"
      ><el-radio-button value="all">{{ t('ordersAll') }}</el-radio-button
      ><el-radio-button value="package">{{ t('ordersPackage') }}</el-radio-button
      ><el-radio-button value="recharge">{{ t('ordersRecharge') }}</el-radio-button></el-radio-group
    >
    <article class="user-panel commerce-panel">
      <el-table v-loading="loading" :data="orders.items" :empty-text="t('ordersEmpty')"
        ><el-table-column :label="t('ordersName')" min-width="180"
          ><template #default="{ row }"
            ><strong>{{ row.name }}</strong>
            <div class="commerce-label">{{ row.id }}</div></template
          ></el-table-column
        ><el-table-column :label="t('ordersType')" min-width="110"
          ><template #default="{ row }">{{
            t(row.type === 'package' ? 'ordersPackage' : 'ordersRecharge')
          }}</template></el-table-column
        ><el-table-column :label="t('ordersAmount')" min-width="110"
          ><template #default="{ row }">{{
            formatMoney(row.amount_micros, locale, 2)
          }}</template></el-table-column
        ><el-table-column :label="t('status')" min-width="120"
          ><template #default="{ row }"
            ><el-tag
              :type="
                ['paid', 'completed'].includes(row.status)
                  ? 'success'
                  : row.status === 'pending'
                    ? 'warning'
                    : 'info'
              "
              size="small"
              >{{ statusLabel(row.status) }}</el-tag
            ></template
          ></el-table-column
        ><el-table-column :label="t('createdAt')" min-width="160"
          ><template #default="{ row }">{{
            formatCompactDateTime(row.created_at)
          }}</template></el-table-column
        ><el-table-column :label="t('expiresAt')" min-width="160"
          ><template #default="{ row }">{{
            row.expires_at ? formatCompactDateTime(row.expires_at) : '—'
          }}</template></el-table-column
        ><el-table-column width="170"
          ><template #default="{ row }"
            ><a
              v-if="row.status === 'pending' && checkoutUrl(row.checkout_url)"
              :href="checkoutUrl(row.checkout_url)"
              target="_blank"
              rel="noopener noreferrer"
              >{{ t('ordersPay') }}</a
            ><el-button
              v-if="row.type === 'recharge' && row.status === 'pending'"
              text
              type="danger"
              :loading="canceling === row.id"
              :disabled="!!canceling"
              @click="cancelOrder(String(row.id))"
              >{{ t('ordersCancel') }}</el-button
            ></template
          ></el-table-column
        ></el-table
      >
    </article>
    <p class="commerce-label">{{ t('ordersCancelHint') }}</p>
    <CommercePagination
      v-model:page="page"
      v-model:page-size="pageSize"
      :total="orders.total"
      :loading="loading"
    />
  </section>
</template>
