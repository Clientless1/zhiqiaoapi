<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { getSetupStatus, type ServicePolicy } from '../../api/policy'
import PublicHeader from '../../components/common/PublicHeader.vue'
import ProviderIcon from '../../components/common/ProviderIcon.vue'
import HomeIcon from '../../components/home/HomeIcon.vue'
import { useAsyncData } from '../../composables/useAsyncData'
import { useLocale } from '../../composables/useLocale'
import { useSiteBrand } from '../../composables/useSiteBrand'
import { useAuthStore } from '../../stores/auth'
import { copyTextWithMessage } from '../../utils/clipboard'

const { locale, t } = useLocale()
const { siteName } = useSiteBrand()
const auth = useAuthStore()
const router = useRouter()
const route = useRoute()
const { data: servicePolicy } = useAsyncData<ServicePolicy | null>(() => getSetupStatus(), null)
const brandName = computed(() =>
  siteName.value === '智桥API' ? t('landingBrand') : siteName.value
)
function startUsing() {
  void router.push(auth.isAdmin ? '/admin' : '/home/overview')
}

function startRegistration() {
  void router.push(auth.isAuthed ? (auth.isAdmin ? '/admin' : '/home/overview') : '/register')
}

watch(
  () => route.query.register,
  (register) => {
    if (register === '1') void router.replace('/register')
  },
  { immediate: true }
)

const activeType = ref<'token' | 'image' | 'video' | 'audio'>('token')
const modelTypes = [
  { value: 'token', key: 'landingText', icon: 'chat' },
  { value: 'image', key: 'landingImage', icon: 'image' },
  { value: 'video', key: 'landingVideo', icon: 'play' },
  { value: 'audio', key: 'landingAudio', icon: 'audio' }
] as const
function moveModelTab(event: KeyboardEvent, direction: number) {
  const index = modelTypes.findIndex((type) => type.value === activeType.value)
  activeType.value = modelTypes[(index + direction + modelTypes.length) % modelTypes.length]!.value
  const tabList = (event.currentTarget as HTMLElement).parentElement
  tabList?.querySelector<HTMLButtonElement>(`#landing-tab-${activeType.value}`)?.focus()
}
const providers = computed(() => {
  const all = [
    { id: 'openai', name: 'OpenAI' },
    { id: 'google', name: 'Google Gemini' },
    { id: 'deepseek', name: 'DeepSeek' },
    { id: 'doubao', name: t('landingProviderDoubao') },
    { id: 'anthropic', name: 'Anthropic' },
    { id: 'moonshot', name: 'Kimi' },
    { id: 'qwen', name: t('landingProviderQwen') }
  ]
  if (activeType.value === 'image')
    return all.filter((p) => ['openai', 'google', 'doubao', 'qwen'].includes(p.id))
  if (activeType.value === 'video')
    return all.filter((p) => ['openai', 'google', 'doubao'].includes(p.id))
  if (activeType.value === 'audio')
    return all.filter((p) => ['openai', 'google', 'doubao', 'qwen'].includes(p.id))
  return all
})
const features = [
  { icon: 'grid', title: 'landingUnifiedTitle', hint: 'landingUnifiedHint' },
  { icon: 'route', title: 'landingRoutingTitle', hint: 'landingRoutingHint' },
  { icon: 'coins', title: 'landingPricingTitle', hint: 'landingPricingHint' },
  { icon: 'shield', title: 'landingSecureTitle', hint: 'landingSecureHint' },
  { icon: 'code', title: 'landingOpenTitle', hint: 'landingOpenHint' }
] as const
const apiExample = computed(() => {
  const origin = (servicePolicy.value?.public_base_url || window.location.origin).replace(/\/$/, '')
  const slash = String.fromCharCode(92)
  if (activeType.value === 'audio')
    return [
      `curl -X POST ${origin}/v1/audio/transcriptions ${slash}`,
      `  -H "Authorization: Bearer YOUR_API_KEY" ${slash}`,
      `  -F "model=whisper-1" ${slash}`,
      '  -F "file=@audio.mp3"'
    ].join('\n')
  const path =
    activeType.value === 'image'
      ? '/v1/images/generations'
      : activeType.value === 'video'
        ? '/v1/videos'
        : '/v1/chat/completions'
  const body =
    activeType.value === 'image'
      ? { model: 'dall-e-3', prompt: 'A quiet garden at sunrise' }
      : activeType.value === 'video'
        ? { model: 'sora-2', prompt: 'A quiet garden at sunrise' }
        : { model: 'gpt-4o', messages: [{ role: 'user', content: t('landingHello') }] }
  const payload =
    activeType.value === 'token'
      ? [
          '{',
          '    "model": "gpt-4o",',
          `    "messages": ${JSON.stringify([{ role: 'user', content: t('landingHello') }])}`,
          '  }'
        ].join('\n')
      : JSON.stringify(body, null, 2)
  return [
    `curl -X POST ${origin}${path} ${slash}`,
    `  -H "Authorization: Bearer YOUR_API_KEY" ${slash}`,
    `  -H "Content-Type: application/json" ${slash}`,
    `  -d '${payload}'`
  ].join('\n')
})
const codeLines = computed(() =>
  apiExample.value
    .split('\n')
    .map((line) => line.split(/("[^"\n]*")/g).map((text, i) => ({ text, string: i % 2 === 1 })))
)
async function copyExample() {
  await copyTextWithMessage(apiExample.value, t('copiedText'))
}
</script>

<template>
  <div class="home-page landing-page">
    <PublicHeader />
    <main>
      <section class="landing-hero" aria-labelledby="landing-title">
        <div class="landing-hero-inner">
          <div class="landing-copy">
            <div class="landing-badge"><HomeIcon name="sparkle" />{{ t('landingBadge') }}</div>
            <h1 id="landing-title">
              <span class="landing-title-brand">{{ brandName }}</span
              ><span class="landing-title-line"
                >{{ t('landingTitleBefore') }} <em>{{ t('landingTitleAccent') }}</em
                >{{ locale === 'en-US' ? ' ' : '' }}{{ t('landingTitleAfter') }}</span
              >
            </h1>
            <p class="landing-description">
              {{ t('landingIntro') }}<br />{{ t('landingIntroSecond') }}
            </p>
            <div class="landing-actions">
              <button
                class="landing-button landing-button-primary"
                type="button"
                @click="startUsing"
              >
                <HomeIcon name="rocket" />{{ t('landingStart') }}
              </button>
              <RouterLink class="landing-button landing-button-secondary" to="/docs"
                ><HomeIcon name="document" />{{ t('landingDocs') }}</RouterLink
              >
            </div>
          </div>
          <div class="landing-showcase">
            <div class="showcase-decoration showcase-decoration-one" aria-hidden="true"></div>
            <div class="showcase-decoration showcase-decoration-two" aria-hidden="true"></div>
            <div class="showcase-decoration showcase-decoration-three" aria-hidden="true"></div>
            <div class="showcase-connector" aria-hidden="true"><HomeIcon name="arrow" /></div>
            <section class="landing-model-panel" aria-labelledby="models-panel-title">
              <h2 id="models-panel-title">{{ t('landingProviders') }}</h2>
              <div class="landing-model-tabs" role="tablist" :aria-label="t('modelTypes')">
                <button
                  v-for="type in modelTypes"
                  :id="`landing-tab-${type.value}`"
                  :key="type.value"
                  type="button"
                  role="tab"
                  :aria-selected="activeType === type.value"
                  aria-controls="landing-providers"
                  :tabindex="activeType === type.value ? 0 : -1"
                  :class="{ active: activeType === type.value }"
                  @click="activeType = type.value"
                  @keydown.right.prevent="moveModelTab($event, 1)"
                  @keydown.left.prevent="moveModelTab($event, -1)"
                >
                  <HomeIcon :name="type.icon" />{{ t(type.key) }}
                </button>
              </div>
              <div
                id="landing-providers"
                class="landing-provider-grid"
                role="tabpanel"
                :aria-labelledby="`landing-tab-${activeType}`"
              >
                <RouterLink
                  v-for="provider in providers"
                  :key="provider.id"
                  :class="['landing-provider', `landing-provider-${provider.id}`]"
                  :to="{ path: '/models', query: { provider: provider.id, type: activeType } }"
                  ><ProviderIcon :provider="provider.id" /><span>{{
                    provider.name
                  }}</span></RouterLink
                >
                <RouterLink
                  class="landing-provider"
                  :to="{ path: '/models', query: { type: activeType } }"
                  ><span class="landing-more-icon"><HomeIcon name="more" /></span
                  ><span>{{ t('landingMore') }}</span></RouterLink
                >
              </div>
            </section>
            <section class="landing-api-panel" aria-labelledby="api-panel-title">
              <span class="landing-api-bolt" aria-hidden="true"><HomeIcon name="bolt" /></span>
              <h2 id="api-panel-title"><HomeIcon name="code" />{{ t('landingApi') }}</h2>
              <button
                type="button"
                class="landing-copy-code"
                :aria-label="t('landingCopyCode')"
                :title="t('landingCopyCode')"
                @click="copyExample"
              >
                <HomeIcon name="document" />
              </button>
              <pre
                class="landing-api-code"
              ><code><span v-for="(line, index) in codeLines" :key="index" class="api-code-line"><span v-for="(part, i) in line" :key="i" :class="{ 'api-code-string': part.string }">{{ part.text }}</span>{{ '\n' }}</span></code></pre>
            </section>
          </div>
        </div>
      </section>
      <section class="landing-features" :aria-label="t('unifiedApi')">
        <div class="landing-features-inner">
          <article v-for="feature in features" :key="feature.title" class="landing-feature">
            <span class="landing-feature-icon"><HomeIcon :name="feature.icon" /></span>
            <div>
              <h2>{{ t(feature.title) }}</h2>
              <p>{{ t(feature.hint) }}</p>
            </div>
          </article>
        </div>
      </section>
      <section class="landing-quick-start" aria-labelledby="quick-start-title">
        <header class="landing-quick-header">
          <div>
            <h2 id="quick-start-title"><span>3</span> {{ t('landingQuickTitle') }}</h2>
            <p>{{ t('landingQuickHint') }}</p>
          </div>
          <RouterLink to="/docs"
            >{{ t('landingDetailedDocs') }}<HomeIcon name="arrow"
          /></RouterLink>
        </header>
        <div class="landing-steps">
          <button
            type="button"
            class="landing-step landing-step-register"
            @click="startRegistration"
          >
            <span class="landing-step-number">1</span
            ><span class="landing-step-icon"><HomeIcon name="user" /></span
            ><span class="landing-step-copy"
              ><strong>{{ t('landingStepRegister') }}</strong
              ><span>{{ t('landingStepRegisterHint') }}</span></span
            ><HomeIcon name="arrow" />
          </button>
          <HomeIcon class="landing-step-connector" name="arrow" />
          <RouterLink class="landing-step landing-step-model" to="/models"
            ><span class="landing-step-number">2</span
            ><span class="landing-step-icon"><HomeIcon name="cube" /></span
            ><span class="landing-step-copy"
              ><strong>{{ t('landingStepModel') }}</strong
              ><span>{{ t('landingStepModelHint') }}</span></span
            ><HomeIcon name="arrow"
          /></RouterLink>
          <HomeIcon class="landing-step-connector" name="arrow" />
          <RouterLink class="landing-step landing-step-call" to="/interfaces/openai/text"
            ><span class="landing-step-number">3</span
            ><span class="landing-step-icon"><HomeIcon name="code" /></span
            ><span class="landing-step-copy"
              ><strong>{{ t('landingStepCall') }}</strong
              ><span>{{ t('landingStepCallHint') }}</span></span
            ><HomeIcon name="arrow"
          /></RouterLink>
        </div>
      </section>
    </main>
  </div>
</template>
