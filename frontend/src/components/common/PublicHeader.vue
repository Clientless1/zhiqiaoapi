<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import LocaleToggleButton from './LocaleToggleButton.vue'
import HomeIcon from '../home/HomeIcon.vue'
import BrandLogo from './BrandLogo.vue'
import AccountMenu from './AccountMenu.vue'
import { useLocale } from '../../composables/useLocale'
import { useSiteBrand } from '../../composables/useSiteBrand'
import { useAuthStore } from '../../stores/auth'
import { getSetupStatus } from '../../api/policy'
import { useAsyncData } from '../../composables/useAsyncData'

defineProps<{ headerClass?: string }>()
const auth = useAuthStore()
const route = useRoute()
const router = useRouter()
const { t } = useLocale()
const { siteName } = useSiteBrand()
const { data: policy } = useAsyncData(() => getSetupStatus(), null)
const search = ref('')
const dashboardLink = computed(() => (auth.isAdmin ? '/admin' : '/home/overview'))
const registrationVisible = computed(
  () => policy.value?.registration_enabled !== false && policy.value?.service_mode !== 'internal'
)
function searchModels() {
  void router.push({
    path: '/models',
    query: search.value.trim() ? { q: search.value.trim() } : {}
  })
}
</script>

<template>
  <header :class="['home-header', headerClass]">
    <div class="public-header-inner">
      <RouterLink class="home-brand" to="/" :aria-label="siteName">
        <BrandLogo />
      </RouterLink>
      <nav class="home-nav" :aria-label="t('home')">
        <RouterLink to="/" :class="{ 'is-active': route.path === '/' }">{{ t('home') }}</RouterLink>
        <RouterLink
          to="/models"
          :class="{ 'is-active': route.path === '/models' && route.query.sort !== 'price' }"
          >{{ t('modelPlaza') }}</RouterLink
        >
        <RouterLink to="/about" :class="{ 'is-active': route.path === '/about' }">{{
          t('publicAboutUs')
        }}</RouterLink>
        <RouterLink
          to="/models?sort=price"
          :class="{ 'is-active': route.path === '/models' && route.query.sort === 'price' }"
          >{{ t('landingPricing') }}</RouterLink
        >
        <RouterLink
          to="/docs"
          :class="{ 'is-active': route.path === '/docs' || route.path.startsWith('/interfaces') }"
          >{{ t('docs') }}</RouterLink
        >
      </nav>
      <form class="home-header-search" role="search" @submit.prevent="searchModels">
        <HomeIcon name="search" />
        <input
          v-model="search"
          type="search"
          :placeholder="t('landingSearch')"
          :aria-label="t('landingSearch')"
        />
      </form>
      <div class="home-header-actions">
        <LocaleToggleButton class="home-language-button" />
        <RouterLink class="home-console-link" :to="dashboardLink">{{ t('console') }}</RouterLink>
        <template v-if="!auth.isAuthed">
          <RouterLink class="home-login-link" to="/login">{{ t('signIn') }}</RouterLink>
          <RouterLink v-if="registrationVisible" class="home-register-button" to="/register">
            {{ t('landingRegister') }}
          </RouterLink>
        </template>
        <AccountMenu v-else />
      </div>
    </div>
  </header>
</template>
