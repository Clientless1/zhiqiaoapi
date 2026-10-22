<script setup lang="ts">
import { useSiteBrand } from '../../composables/useSiteBrand'

const { siteName, logoUrl } = useSiteBrand()
</script>

<template>
  <img
    class="site-brand-image"
    :src="logoUrl || '/logos/zhiqiao-original.png'"
    :alt="siteName"
    width="866"
    height="530"
  />
</template>

<style scoped>
.site-brand-image {
  display: block;
  width: 84px;
  height: 52px;
  flex: 0 0 auto;
  object-fit: contain;
  mix-blend-mode: multiply;
}
</style>
