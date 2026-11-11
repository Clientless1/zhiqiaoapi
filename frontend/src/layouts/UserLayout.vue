<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, provide } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { getUserServicePolicy } from '../api/policy'
import { getNavigationSetting, isUserPathVisible, navigationSetting } from '../api/navigation'
import ConsoleFrame from '../components/common/ConsoleFrame.vue'
import { useAsyncData } from '../composables/useAsyncData'
import type { MessageKey } from '../i18n'
import '../styles/user.css'
import '../styles/commerce.css'
const { data: servicePolicy } = useAsyncData(() => getUserServicePolicy(), null)
const route = useRoute(),
  router = useRouter()
provide('servicePolicy', servicePolicy)
async function refreshNavigation() {
  if (document.visibilityState === 'hidden') return
  const setting = await getNavigationSetting(true).catch(() => null)
  if (setting && !isUserPathVisible(route.path, setting)) void router.replace('/home/overview')
}
onMounted(() => {
  void getNavigationSetting().catch(() => null)
  window.addEventListener('focus', refreshNavigation)
})
onBeforeUnmount(() => window.removeEventListener('focus', refreshNavigation))
provide('navigationSetting', navigationSetting)
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

const allGroups = computed(() => [
  {
    key: 'userOverviewGroup' as const,
    items: [{ path: '/home/overview', key: 'userDashboard' as const }]
  },
  {
    key: 'userDevelopmentGroup' as const,
    items: [
      { path: '/home/models', key: 'modelPlaza' as const },
      { path: '/home/playground', key: 'studioPlayground' as const },
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
const groups = computed(() =>
  allGroups.value
    .map((group) => ({
      ...group,
      items: group.items.filter((item) => isUserPathVisible(item.path))
    }))
    .filter((group) => group.items.length > 0)
)
</script>
<template><ConsoleFrame :groups="groups" /></template>
