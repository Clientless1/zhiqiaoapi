<script setup lang="ts">
import { computed, provide } from 'vue'
import { getUserServicePolicy } from '../api/policy'
import ConsoleFrame from '../components/common/ConsoleFrame.vue'
import { useAsyncData } from '../composables/useAsyncData'
import type { MessageKey } from '../i18n'
import '../styles/user.css'
import '../styles/commerce.css'
const { data: servicePolicy } = useAsyncData(() => getUserServicePolicy(), null)
provide('servicePolicy', servicePolicy)
type NavItem = { path: string; key: MessageKey }
const accountItems = computed(() => {
  const items: NavItem[] = [
    { path: '/home/packages', key: 'userPlans' },
    { path: '/home/coupons', key: 'userCoupons' },
    { path: '/home/redeem', key: 'userRedeem' },
    { path: '/home/orders', key: 'userOrders' },
    { path: '/home/ledger', key: 'userLedger' }
  ]
  if (servicePolicy.value?.recharge_enabled) {
    items.push({ path: '/home/recharge', key: 'recharge' })
  }
  return items
})

const groups = computed(() => [
  {
    key: 'userOverviewGroup' as const,
    items: [{ path: '/home/overview', key: 'userDashboard' as const }]
  },
  {
    key: 'userDevelopmentGroup' as const,
    items: [
      { path: '/home/models', key: 'modelPlaza' as const },
      { path: '/home/apikeys', key: 'apiKey' as const },
      { path: '/home/usage', key: 'usage' as const }
    ]
  },
  { key: 'userAccountGroup' as const, items: accountItems.value },
  {
    key: 'userSystemGroup' as const,
    items: [
      { path: '/home/notifications', key: 'userNotifications' as const },
      { path: '/home/settings', key: 'personalSettings' as const }
    ]
  }
])
</script>
<template><ConsoleFrame :groups="groups" /></template>
