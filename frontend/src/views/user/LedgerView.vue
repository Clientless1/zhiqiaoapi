<script setup lang="ts">
import { getLedger, emptyPage, type LedgerEntry } from '../../api/commerce'
import { useCommercePage } from '../../composables/useCommercePage'
import { useLocale } from '../../composables/useLocale'
import { useBillingCurrency } from '../../composables/useBillingCurrency'
import { formatCompactDateTime } from '../../utils/format'
import CommercePagination from '../../components/common/CommercePagination.vue'
const { locale, t } = useLocale()
const { formatMoney } = useBillingCurrency()
const { data, loading, error, reload, page, pageSize, filter } = useCommercePage(getLedger, {
  ...emptyPage<LedgerEntry>(),
  currency: 'CNY'
})
const types = [
  { value: 'all', key: 'ledgerAll' },
  { value: 'usage', key: 'consoleCost' },
  { value: 'recharge', key: 'ordersRecharge' },
  { value: 'package', key: 'ordersPackage' },
  { value: 'redemption', key: 'userRedeem' },
  { value: 'adjustment', key: 'ledgerAdjustment' },
  { value: 'refund', key: 'ledgerRefund' },
  { value: 'allocation_recover', key: 'ledgerRecovery' }
] as const
function label(type: string) {
  return t(types.find((item) => item.value === type)?.key ?? 'ledgerAdjustment')
}
</script>
<template>
  <section class="commerce-view">
    <div class="commerce-toolbar">
      <p class="commerce-intro">{{ t('ledgerIntro') }}</p>
      <el-button :loading="loading" @click="reload()">{{ t('dashboardRefresh') }}</el-button>
    </div>
    <el-select v-model="filter" style="width: 180px" :aria-label="t('ordersType')"
      ><el-option v-for="item in types" :key="item.value" :value="item.value" :label="t(item.key)"
    /></el-select>
    <el-alert v-if="error" type="error" :title="error" :closable="false" />
    <article class="user-panel commerce-panel">
      <el-table v-loading="loading" :data="data.items" :empty-text="t('consoleNoData')">
        <el-table-column :label="t('createdAt')" min-width="165"
          ><template #default="{ row }">{{
            formatCompactDateTime(row.created_at)
          }}</template></el-table-column
        >
        <el-table-column :label="t('ordersType')" min-width="120"
          ><template #default="{ row }">{{ label(row.type) }}</template></el-table-column
        >
        <el-table-column :label="t('ledgerAccount')" min-width="160" show-overflow-tooltip
          ><template #default="{ row }"
            >{{ row.account_name }}
            <div v-if="row.model" class="commerce-label">{{ row.model }}</div></template
          ></el-table-column
        >
        <el-table-column :label="t('ledgerChange')" min-width="140"
          ><template #default="{ row }"
            ><span :class="row.amount_micros > 0 ? 'ledger-credit' : ''"
              >{{ row.amount_micros > 0 ? '+' : ''
              }}{{ formatMoney(row.amount_micros, locale, 6) }}</span
            ></template
          ></el-table-column
        >
        <el-table-column :label="t('ledgerBalanceAfter')" min-width="145"
          ><template #default="{ row }">{{
            row.balance_after_micros == null
              ? '—'
              : formatMoney(row.balance_after_micros, locale, 6)
          }}</template></el-table-column
        >
        <el-table-column
          prop="transaction_id"
          :label="t('ledgerReference')"
          min-width="160"
          show-overflow-tooltip
        />
      </el-table>
    </article>
    <CommercePagination
      v-model:page="page"
      v-model:page-size="pageSize"
      :total="data.total"
      :loading="loading"
    />
  </section>
</template>
<style scoped>
.ledger-credit {
  color: #16966d;
}
</style>
