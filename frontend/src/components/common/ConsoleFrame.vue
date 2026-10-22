<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'
import {
  Menu,
  Grid,
  Connection,
  Key,
  User,
  Folder,
  DataLine,
  Tickets,
  Setting,
  CreditCard,
  Bell,
  Present,
  Wallet,
  Discount,
  House,
  Box,
  Guide,
  ArrowRight
} from '@element-plus/icons-vue'
import LocaleToggleButton from './LocaleToggleButton.vue'
import BrandLogo from './BrandLogo.vue'
import AccountMenu from './AccountMenu.vue'
import { useLocale } from '../../composables/useLocale'
import { useSiteBrand } from '../../composables/useSiteBrand'
import { isMessageKey, type MessageKey } from '../../i18n'

export type ConsoleNavItem = { path: string; key: MessageKey }
defineProps<{
  admin?: boolean
  groups: { key: MessageKey; items: ConsoleNavItem[] }[]
  settings?: ConsoleNavItem[]
}>()
const { t } = useLocale()
const { siteName } = useSiteBrand()
const route = useRoute()
const router = useRouter()
const menuOpen = ref(false)
function navigate(path: string) {
  menuOpen.value = false
  void router.push(path)
}
const title = computed(() =>
  t(isMessageKey(route.meta.messageKey) ? route.meta.messageKey : 'console')
)
const subtitle = computed(() =>
  isMessageKey(route.meta.subtitleKey) ? t(route.meta.subtitleKey) : ''
)
const isExplorer = computed(() => /\/(models|supply-chain)$/.test(route.path))
const icons: Record<string, unknown> = {
  modelPlaza: Grid,
  supplyChain: Box,
  upstreamChannels: Connection,
  apps: Box,
  credentialManagement: Key,
  userManagement: User,
  projectManagement: Folder,
  consumptionOverview: DataLine,
  costAttribution: DataLine,
  usage: Tickets,
  overview: House,
  apiKey: Key,
  recharge: CreditCard,
  personalSettings: Setting,
  userPlans: Wallet,
  userCoupons: Discount,
  userRedeem: Present,
  userOrders: Tickets,
  userLedger: Wallet,
  userNotifications: Bell,
  userDashboard: House
}
watch(
  () => route.fullPath,
  () => {
    menuOpen.value = false
  }
)
</script>

<template>
  <el-container
    class="app-shell console-frame"
    :class="admin ? 'admin-shell' : 'user-shell'"
    direction="vertical"
  >
    <el-header class="shell-topbar" height="68px">
      <div class="topbar-left">
        <el-button
          class="mobile-menu-button"
          :icon="Menu"
          :aria-label="t('adminMenu')"
          :aria-expanded="menuOpen"
          @click="menuOpen = !menuOpen"
        />
        <RouterLink class="topbar-brand" to="/" :aria-label="siteName">
          <BrandLogo />
        </RouterLink>
        <span class="topbar-breadcrumb"
          ><span>/</span> {{ t(admin ? 'administrator' : 'developerConsole') }}
          <el-icon><ArrowRight /></el-icon> {{ title }}</span
        >
      </div>
      <div class="topbar-actions">
        <RouterLink class="topbar-docs" to="/about">{{ t('publicAboutUs') }}</RouterLink>
        <RouterLink class="topbar-docs" to="/docs"
          >{{ t('docs') }} <el-icon><Guide /></el-icon
        ></RouterLink>
        <LocaleToggleButton class="topbar-language-button" />
        <AccountMenu />
      </div>
    </el-header>
    <el-container class="shell-body">
      <el-aside :class="{ 'is-open': menuOpen }" width="232px">
        <el-menu
          :key="route.path.startsWith('/admin/settings') ? 'settings' : 'main'"
          :default-active="route.path"
          :default-openeds="route.path.startsWith('/admin/settings') ? ['settings'] : []"
          unique-opened
          @select="navigate"
        >
          <template v-for="group in groups" :key="group.key">
            <li class="admin-nav-group-label">{{ t(group.key) }}</li>
            <el-menu-item v-for="item in group.items" :key="item.path" :index="item.path">
              <el-icon><component :is="icons[item.key] || Grid" /></el-icon
              ><span>{{ t(item.key) }}</span>
            </el-menu-item>
          </template>
          <template v-if="settings?.length">
            <li class="admin-nav-group-label">{{ t('adminNavSystem') }}</li>
            <el-sub-menu index="settings"
              ><template #title
                ><el-icon><Setting /></el-icon><span>{{ t('settings') }}</span></template
              >
              <el-menu-item v-for="item in settings" :key="item.path" :index="item.path">{{
                t(item.key)
              }}</el-menu-item>
            </el-sub-menu>
          </template>
        </el-menu>
      </el-aside>
      <button
        v-if="menuOpen"
        class="console-menu-overlay"
        type="button"
        :aria-label="t('adminMenu')"
        @click="menuOpen = false"
      ></button>
      <el-main class="content">
        <header v-if="!isExplorer" class="page-header">
          <div class="page-title-block">
            <span class="page-eyebrow">{{ t(admin ? 'administrator' : 'developerConsole') }}</span>
            <h2>{{ title }}</h2>
            <span>{{ subtitle }}</span>
          </div>
        </header>
        <RouterView />
      </el-main>
    </el-container>
  </el-container>
</template>
