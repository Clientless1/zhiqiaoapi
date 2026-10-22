<script setup lang="ts">
import { computed } from 'vue'
import { ChatDotRound, DocumentCopy } from '@element-plus/icons-vue'
import PublicHeader from '../../components/common/PublicHeader.vue'
import BrandLogo from '../../components/common/BrandLogo.vue'
import { getCommunity } from '../../api/commerce'
import { useAsyncData } from '../../composables/useAsyncData'
import { useLocale } from '../../composables/useLocale'
import { useSiteBrand } from '../../composables/useSiteBrand'
import { copyTextWithMessage } from '../../utils/clipboard'
const { t } = useLocale()
const { siteName } = useSiteBrand()
const { data: community, loading, error, reload } = useAsyncData(getCommunity, null)
const joinUrl = computed(() => {
  try {
    const url = new URL(community.value?.qq_group_url ?? '')
    return url.protocol === 'https:' &&
      (url.hostname === 'qq.com' || url.hostname.endsWith('.qq.com'))
      ? url.href
      : ''
  } catch {
    return ''
  }
})
async function copyGroup() {
  if (community.value?.qq_group_number)
    await copyTextWithMessage(community.value.qq_group_number, t('copiedText'))
}
</script>
<template>
  <div class="about-page">
    <PublicHeader />
    <main v-loading="loading" class="about-content">
      <div class="about-brand">
        <BrandLogo />
      </div>
      <h1>{{ community?.about_title || t('aboutPage') }}</h1>
      <p class="about-tagline">{{ t('aboutIntro') }}</p>
      <el-alert v-if="error" type="error" :title="error" :closable="false"
        ><el-button text type="primary" @click="reload()">{{
          t('consoleRetry')
        }}</el-button></el-alert
      >
      <section class="about-card">
        <p class="about-description">{{ community?.about_content || t('aboutIntro') }}</p>
      </section>
      <section class="about-card about-community">
        <span class="about-community-icon"
          ><el-icon><ChatDotRound /></el-icon
        ></span>
        <h2>{{ t('aboutCommunity') }}</h2>
        <p>{{ t('aboutCommunityHint') }}</p>
        <div v-if="community?.qq_group_number" class="about-qq-number">
          <span>{{ t('aboutQQGroup') }}</span
          ><strong>{{ community.qq_group_number }}</strong
          ><el-button text :icon="DocumentCopy" :aria-label="t('aboutCopyQQ')" @click="copyGroup" />
        </div>
        <a
          v-if="joinUrl"
          class="about-join"
          :href="joinUrl"
          target="_blank"
          rel="noopener noreferrer"
          >{{ t('aboutJoinQQ') }} ↗</a
        >
        <p v-if="!community?.qq_group_number && !joinUrl" class="about-coming">
          {{ t('aboutNoCommunity') }}
        </p>
      </section>
      <footer>© {{ new Date().getFullYear() }} {{ siteName }}</footer>
    </main>
  </div>
</template>
<style scoped>
.about-page {
  min-height: 100vh;
  background: radial-gradient(ellipse at 80% 15%, #f0edff 0, transparent 40%), #f8faff;
  color: #273249;
}
.about-content {
  width: min(680px, calc(100% - 40px));
  margin: auto;
  padding: 70px 0 26px;
  text-align: center;
  display: grid;
  justify-items: stretch;
  gap: 20px;
}
.about-brand img {
  width: 150px;
  height: 66px;
  object-fit: contain;
}
.about-content h1 {
  font-size: 30px;
  letter-spacing: -0.5px;
  margin: 0;
  font-weight: 650;
}
.about-tagline {
  color: #818ba3;
  font-size: 14px;
  line-height: 1.8;
  margin: 0 0 16px;
}
.about-card {
  background: #fff;
  border: 1px solid #e7e7f6;
  border-radius: 16px;
  padding: 30px;
  box-shadow: 0 8px 40px #5b48ef04;
}
.about-description {
  white-space: pre-wrap;
  line-height: 2;
  font-size: 14px;
  color: #68738c;
  text-align: left;
  margin: 0;
  overflow-wrap: anywhere;
}
.about-community {
  display: flex;
  flex-direction: column;
  gap: 16px;
  align-items: center;
}
.about-community-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 48px;
  height: 48px;
  background: #f1efff;
  color: #6251ee;
  border-radius: 14px;
  font-size: 25px;
}
.about-community h2 {
  font-size: 18px;
  margin: 0;
}
.about-community p {
  font-size: 13px;
  line-height: 1.7;
  color: #8992a7;
  margin: 0;
}
.about-qq-number {
  display: flex;
  align-items: center;
  gap: 14px;
  font-size: 13px;
  background: #f8f7ff;
  padding: 8px 14px;
  border-radius: 8px;
  color: #8a83a6;
}
.about-qq-number strong {
  font-size: 18px;
  letter-spacing: 1px;
  color: #4e427c;
  font-weight: 600;
}
.about-join {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(100deg, #5154ff, #5835fa);
  color: #fff;
  font-size: 14px;
  padding: 12px 24px;
  border-radius: 8px;
  text-decoration: none;
}
.about-content footer {
  font-size: 12px;
  color: #a1a7b9;
  margin-top: 26px;
}
@media (max-width: 600px) {
  .about-content {
    padding-top: 38px;
  }
  .about-card {
    padding: 22px;
  }
  .about-content h1 {
    font-size: 26px;
  }
}
</style>
