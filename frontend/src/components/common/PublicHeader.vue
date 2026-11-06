<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { Menu, Close } from '@element-plus/icons-vue'
import BrandLogo from './BrandLogo.vue'
import AccountMenu from './AccountMenu.vue'
import LocaleToggleButton from './LocaleToggleButton.vue'
import { useAuthStore } from '../../stores/auth'
import { useStudio } from '../../composables/useStudio'
import { useSiteBrand } from '../../composables/useSiteBrand'
defineProps<{ headerClass?: string }>()
const { s } = useStudio(),
  { siteName } = useSiteBrand(),
  auth = useAuthStore(),
  route = useRoute(),
  open = ref(false)
const links = computed(() => [
  { to: '/models', text: s('模型广场', 'Models') },
  { to: '/playground', text: s('API 调试台', 'Playground') },
  { to: '/docs', text: s('接入文档', 'Docs') },
  { to: '/status', text: s('服务状态', 'Status') },
  { to: '/about', text: s('关于我们', 'About') }
])
watch(
  () => route.fullPath,
  () => {
    open.value = false
  }
)
</script>
<template>
  <header class="studio-public-header" :class="headerClass">
    <div class="studio-header-inner">
      <RouterLink class="studio-brand" to="/" :aria-label="siteName"
        ><BrandLogo /><span class="brand-divider"></span
        ><span class="brand-caption">MODEL GATEWAY</span></RouterLink
      >
      <nav :class="{ open }" :aria-label="s('主导航', 'Main navigation')">
        <RouterLink
          v-for="link in links"
          :key="link.to"
          :to="link.to"
          :class="{ active: route.path === link.to }"
          >{{ link.text }}</RouterLink
        >
      </nav>
      <div class="studio-header-actions">
        <LocaleToggleButton /><AccountMenu v-if="auth.isAuthed" /><RouterLink
          v-else
          to="/login"
          class="studio-login"
          >{{ s('登录', 'Sign in') }}</RouterLink
        ><RouterLink
          class="studio-primary-link compact-link"
          :to="auth.isAdmin ? '/admin/overview' : auth.isUser ? '/home/overview' : '/register'"
          >{{
            auth.isAuthed ? s('进入控制台', 'Console') : s('开始构建', 'Get started')
          }}
          ↗</RouterLink
        ><button
          class="studio-nav-toggle icon-button"
          :aria-label="s('展开导航', 'Toggle navigation')"
          :aria-expanded="open"
          @click="open = !open"
        >
          <el-icon><Close v-if="open" /><Menu v-else /></el-icon>
        </button>
      </div>
    </div>
  </header>
</template>
