<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { RouterView, useRoute } from 'vue-router'
import { Menu } from '@element-plus/icons-vue'
import { getAdminServicePolicy } from '../api/policy'
import LocaleToggleButton from '../components/common/LocaleToggleButton.vue'
import { isMessageKey, type MessageKey } from '../i18n'
import { useLocale } from '../composables/useLocale'
import { useLogout } from '../composables/useLogout'
import { useAsyncData } from '../composables/useAsyncData'
import { useSiteBrand } from '../composables/useSiteBrand'
import '../styles/admin.css'

const { t } = useLocale()
const { siteName, logoUrl } = useSiteBrand()
const route = useRoute()
const logout = useLogout(t)
const { data: servicePolicy } = useAsyncData(() => getAdminServicePolicy(), null)
const adminMenuOpen = ref(false)
type AdminNavItem = { path: string; key: MessageKey }
type SettingNavItem = { path: string; key: MessageKey }
type AdminNavGroup = { key: MessageKey; items: AdminNavItem[] }

const navGroups = computed(() => {
  const serviceMode = servicePolicy.value?.service_mode
  const operationItems: AdminNavItem[] = [
    { path: '/admin/channels', key: 'upstreamChannels' }
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
  const usageItems: AdminNavItem[] = [
    { path: '/admin/statistics', key: 'consumptionOverview' }
  ]
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

const activeRoute = computed(() => route.path)
const settingsOpen = computed(() => route.path.startsWith('/admin/settings'))
const openMenus = computed(() => (settingsOpen.value ? ['settings'] : []))
const activeRouteLabel = computed(() => {
  const messageKey = route.meta.messageKey
  return t(isMessageKey(messageKey) ? messageKey : 'userManagement')
})
const activeRouteSubtitle = computed(() => {
  const subtitleKey = route.meta.subtitleKey
  return t(isMessageKey(subtitleKey) ? subtitleKey : 'adminConsoleSubtitle')
})

watch(
  () => route.fullPath,
  () => {
    adminMenuOpen.value = false
  }
)
</script>

<template>
  <el-container class="app-shell admin-shell" direction="vertical">
    <el-header class="shell-topbar" height="56px">
      <div class="topbar-left">
        <el-button
          class="mobile-menu-button"
          :aria-label="t('adminMenu')"
          :icon="Menu"
          @click="adminMenuOpen = true"
        />
        <a class="topbar-brand" href="/" target="_blank" rel="noopener noreferrer">
          <img v-if="logoUrl" class="topbar-brand-logo" :src="logoUrl" :alt="siteName" />
          <span class="topbar-brand-name">{{ siteName }}</span>
        </a>
      </div>
      <div class="topbar-actions">
        <LocaleToggleButton class="topbar-language-button" />
        <el-button class="topbar-logout-button" @click="logout">{{ t('logout') }}</el-button>
      </div>
    </el-header>

    <el-container class="shell-body">
      <el-aside :class="{ 'is-open': adminMenuOpen }" width="224px">
        <el-menu
          :key="settingsOpen ? 'settings-open' : 'settings-closed'"
          :default-active="activeRoute"
          :default-openeds="openMenus"
          mode="vertical"
          router
          unique-opened
          @select="adminMenuOpen = false"
        >
          <template v-for="group in navGroups" :key="group.key">
            <li class="admin-nav-group-label">{{ t(group.key) }}</li>
            <el-menu-item v-for="item in group.items" :key="item.path" :index="item.path">
              <span>{{ t(item.key) }}</span>
            </el-menu-item>
          </template>
          <li class="admin-nav-group-label">{{ t('adminNavSystem') }}</li>
          <el-sub-menu index="settings">
            <template #title>
              <span>{{ t('settings') }}</span>
            </template>
            <el-menu-item v-for="item in settingItems" :key="item.path" :index="item.path">
              <span>{{ t(item.key) }}</span>
            </el-menu-item>
          </el-sub-menu>
        </el-menu>
      </el-aside>
      <button
        v-if="adminMenuOpen"
        class="admin-menu-overlay"
        type="button"
        :aria-label="t('adminMenu')"
        @click="adminMenuOpen = false"
      ></button>

      <el-main class="content">
        <header class="page-header">
          <div class="page-title-block">
            <h2>{{ activeRouteLabel }}</h2>
            <span>{{ activeRouteSubtitle }}</span>
          </div>
        </header>

        <RouterView />
      </el-main>
    </el-container>
  </el-container>
</template>
