<script setup lang="ts">
import { computed, ref, watch, onMounted, onBeforeUnmount } from 'vue'
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
  Search,
  Monitor,
  Promotion
} from '@element-plus/icons-vue'
import LocaleToggleButton from './LocaleToggleButton.vue'
import BrandLogo from './BrandLogo.vue'
import AccountMenu from './AccountMenu.vue'
import { useLocale } from '../../composables/useLocale'
import { useSiteBrand } from '../../composables/useSiteBrand'
import { useStudio } from '../../composables/useStudio'
import { isMessageKey, type MessageKey } from '../../i18n'

export type ConsoleNavItem = { path: string; key: MessageKey }
const props = defineProps<{
  admin?: boolean
  groups: { key: MessageKey; items: ConsoleNavItem[] }[]
  settings?: ConsoleNavItem[]
}>()
const { t } = useLocale()
const { s } = useStudio()
const commandOpen = ref(false),
  commandSearch = ref('')
const commandLinks = computed(() =>
  [...props.groups.flatMap((g) => g.items), ...(props.settings ?? [])].filter((item) =>
    t(item.key).toLowerCase().includes(commandSearch.value.toLowerCase())
  )
)
function commandNavigate(path: string) {
  commandOpen.value = false
  commandSearch.value = ''
  navigate(path)
}
function keyboard(event: KeyboardEvent) {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
    event.preventDefault()
    commandOpen.value = !commandOpen.value
  }
}
onMounted(() => window.addEventListener('keydown', keyboard))
onBeforeUnmount(() => window.removeEventListener('keydown', keyboard))
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
const isExplorer = computed(() => /\/(models|supply-chain|overview)$/.test(route.path))
const icons: Record<string, unknown> = {
  studioOverview: House,
  studioPlayground: Promotion,
  studioStatus: Monitor,
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
    :class="[
      admin ? 'admin-shell' : 'user-shell',
      { 'aurora-console': route.path.endsWith('/overview') }
    ]"
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
        <button class="console-command-trigger" @click="commandOpen = true">
          <el-icon><Search /></el-icon
          ><span>{{ s('搜索页面、功能…', 'Search pages and tools…') }}</span
          ><kbd>⌘ K</kbd>
        </button>
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
      <el-aside class="studio-sidebar" :class="{ 'is-open': menuOpen }" width="232px">
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
        <div class="sidebar-guide">
          <span class="sidebar-guide-symbol">↗</span
          ><strong>{{ s('下一步，开始构建', 'Ready to build?') }}</strong>
          <p>
            {{ s('用调试台验证你的第一个请求。', 'Try your first request in the Playground.') }}
          </p>
          <RouterLink :to="admin ? '/admin/playground' : '/home/playground'"
            >{{ s('打开调试台', 'Open Playground') }} →</RouterLink
          >
        </div>
        <RouterLink class="sidebar-status-link" :to="admin ? '/admin/status' : '/home/status'"
          ><el-icon><Monitor /></el-icon>{{ s('服务状态', 'Service status')
          }}<span>↗</span></RouterLink
        >
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
        <RouterView v-slot="{ Component }"
          ><Transition name="studio-page" mode="out-in"
            ><component :is="Component" :key="route.path" /></Transition
        ></RouterView>
      </el-main>
    </el-container>
    <el-dialog
      v-model="commandOpen"
      :title="s('快速前往', 'Go to page')"
      width="min(580px,94vw)"
      class="command-dialog"
      @opened="($refs.commandInput as { focus: () => void })?.focus()"
      ><el-input
        ref="commandInput"
        v-model="commandSearch"
        :prefix-icon="Search"
        :placeholder="s('输入页面或功能名称…', 'Type a page or tool name…')"
        @keydown.enter.stop.prevent="commandLinks[0] && commandNavigate(commandLinks[0].path)"
      />
      <div class="command-results">
        <button v-for="item in commandLinks" :key="item.path" @click="commandNavigate(item.path)">
          <el-icon><component :is="icons[item.key] || Grid" /></el-icon
          ><span>{{ t(item.key) }}</span
          ><small>↵</small></button
        ><el-empty
          v-if="!commandLinks.length"
          :description="s('未找到页面', 'No matching pages')"
          :image-size="60"
        />
      </div>
      <footer class="command-footer">
        {{ s('Esc 关闭 · Enter 打开第一个结果', 'Esc to close · Enter to open first result') }}
      </footer></el-dialog
    >
  </el-container>
</template>
