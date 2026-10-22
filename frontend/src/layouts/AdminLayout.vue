<script setup lang="ts">
import { computed } from 'vue'
import { getAdminServicePolicy } from '../api/policy'
import ConsoleFrame from '../components/common/ConsoleFrame.vue'
import { useAsyncData } from '../composables/useAsyncData'
import type { MessageKey } from '../i18n'
import '../styles/admin.css'
const { data: servicePolicy } = useAsyncData(() => getAdminServicePolicy(), null)
type AdminNavItem = { path: string; key: MessageKey }
type SettingNavItem = AdminNavItem
type AdminNavGroup = { key: MessageKey; items: AdminNavItem[] }
const navGroups = computed(() => {
  const serviceMode = servicePolicy.value?.service_mode
  const operationItems: AdminNavItem[] = [
    { path: '/admin/models', key: 'modelPlaza' },
    { path: '/admin/supply-chain', key: 'supplyChain' },
    { path: '/admin/channels', key: 'upstreamChannels' },
    { path: '/admin/user-operations', key: 'userOperations' }
  ]
  if (serviceMode === 'internal') {
    operationItems.push({ path: '/admin/apps', key: 'apps' })
  }
  if (serviceMode === 'paid') {
    operationItems.push({ path: '/admin/credentials', key: 'credentialManagement' })
    operationItems.push({ path: '/admin/keys', key: 'userManagement' })
  }
  const groups: AdminNavGroup[] = [
    {
      key: 'adminNavOperations',
      items: operationItems
    }
  ]
  if (serviceMode === 'internal') {
    groups.push({
      key: 'adminNavAccounts',
      items: [
        { path: '/admin/keys', key: 'userManagement' },
        { path: '/admin/projects', key: 'projectManagement' }
      ]
    })
  }
  const usageItems: AdminNavItem[] = [{ path: '/admin/statistics', key: 'consumptionOverview' }]
  if (serviceMode !== 'paid') {
    usageItems.push({ path: '/admin/cost-attribution', key: 'costAttribution' })
  }
  usageItems.push({ path: '/admin/usage', key: 'usage' })
  groups.push({
    key: 'adminNavUsage',
    items: usageItems
  })
  return groups
})

const settingItems = computed(() => {
  const items: SettingNavItem[] = [
    { path: '/admin/settings/admin-password', key: 'adminPasswordSettings' },
    { path: '/admin/settings/smtp', key: 'smtpSettings' },
    { path: '/admin/settings/site', key: 'siteSettingsPage' }
  ]
  if (servicePolicy.value?.service_mode === 'paid') {
    items.push({ path: '/admin/settings/payment', key: 'paymentSettings' })
  }
  if (servicePolicy.value?.service_mode !== 'internal') {
    items.push({ path: '/admin/settings/pricing-policies', key: 'pricingPolicy' })
  }
  items.push({ path: '/admin/settings/other', key: 'otherSettings' })
  return items
})
</script>
<template><ConsoleFrame admin :groups="navGroups" :settings="settingItems" /></template>
