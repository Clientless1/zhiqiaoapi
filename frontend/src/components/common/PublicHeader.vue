<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import LocaleToggleButton from './LocaleToggleButton.vue'
import { useLocale } from '../../composables/useLocale'
import { useSiteBrand } from '../../composables/useSiteBrand'
import { useAuthStore } from '../../stores/auth'

defineProps<{
  headerClass?: string
}>()

const auth = useAuthStore()
const { t } = useLocale()
const { siteName, logoUrl } = useSiteBrand()
const dashboardLink = computed(() => (auth.isAdmin ? '/admin' : '/home/overview'))
</script>

<template>
  <header :class="['home-header', headerClass]">
    <RouterLink class="home-brand" to="/" :aria-label="siteName">
      <img v-if="logoUrl" class="home-brand-logo" :src="logoUrl" :alt="siteName" />
      <span class="home-brand-name">{{ siteName }}</span>
    </RouterLink>
    <nav class="home-nav" :aria-label="siteName">
      <RouterLink to="/">{{ t('home') }}</RouterLink>
      <RouterLink to="/docs">{{ t('docs') }}</RouterLink>
      <RouterLink to="/interfaces">{{ t('interfaces') }}</RouterLink>
    </nav>
    <div class="home-header-actions">
      <LocaleToggleButton class="home-language-button" />
      <RouterLink
        v-if="auth.isAuthed"
        class="home-login-link home-account-link"
        :to="dashboardLink"
      >
        {{ t('console') }}
      </RouterLink>
      <RouterLink v-else class="home-login-link" to="/login">{{ t('signIn') }}</RouterLink>
    </div>
  </header>
</template>
