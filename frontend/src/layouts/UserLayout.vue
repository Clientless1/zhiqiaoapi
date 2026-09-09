<script setup lang="ts">
import { computed, provide } from 'vue'
import { RouterView, useRoute } from 'vue-router'
import { getUserServicePolicy } from '../api/policy'
import LocaleToggleButton from '../components/common/LocaleToggleButton.vue'
import { isMessageKey, type MessageKey } from '../i18n'
import { useLocale } from '../composables/useLocale'
import { useLogout } from '../composables/useLogout'
import { useAsyncData } from '../composables/useAsyncData'
import { useSiteBrand } from '../composables/useSiteBrand'
import '../styles/user.css'

const { t } = useLocale()
const { siteName, logoUrl } = useSiteBrand()
const route = useRoute()
const logout = useLogout(t)
const { data: servicePolicy } = useAsyncData(() => getUserServicePolicy(), null)
provide('servicePolicy', servicePolicy)
type NavItem = { path: string; key: MessageKey }

const navItems = computed(() => {
  const items: NavItem[] = [
    { path: '/home/overview', key: 'overview' },
    { path: '/home/apikeys', key: 'apiKey' },
    { path: '/home/usage', key: 'usage' }
  ]
  if (servicePolicy.value?.recharge_enabled) {
    items.push({ path: '/home/recharge', key: 'recharge' })
  }
  items.push({ path: '/home/settings', key: 'personalSettings' })
  return items
})

const activeRoute = computed(() => route.path)
const activeRouteLabel = computed(() => {
  const messageKey = route.meta.messageKey
  return t(isMessageKey(messageKey) ? messageKey : 'apiKey')
})
const activeRouteSubtitle = computed(() => {
  const subtitleKey = route.meta.subtitleKey
  return t(isMessageKey(subtitleKey) ? subtitleKey : 'userConsoleStatus')
})
</script>

<template>
  <el-container class="app-shell user-shell" direction="vertical">
    <el-header class="shell-topbar" height="56px">
      <div class="topbar-left">
        <RouterLink class="topbar-brand" to="/" :aria-label="t('home')">
          <img v-if="logoUrl" class="topbar-brand-logo" :src="logoUrl" :alt="siteName" />
          <span class="topbar-brand-name">{{ siteName }}</span>
        </RouterLink>
      </div>
      <div class="topbar-actions">
        <LocaleToggleButton class="topbar-language-button" />
        <el-button class="topbar-logout-button" @click="logout">{{ t('logout') }}</el-button>
      </div>
    </el-header>

    <el-container class="shell-body">
      <el-aside class="left-rail" width="224px">
        <el-menu :default-active="activeRoute" mode="vertical" router>
          <el-menu-item v-for="item in navItems" :key="item.path" :index="item.path">
            <span>{{ t(item.key) }}</span>
          </el-menu-item>
        </el-menu>
      </el-aside>
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
