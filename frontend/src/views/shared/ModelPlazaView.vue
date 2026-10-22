<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import {
  Search,
  ArrowRight,
  DocumentCopy,
  RefreshRight,
  Grid,
  ChatLineRound,
  Picture,
  VideoCamera,
  Microphone,
  Connection
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import PublicHeader from '../../components/common/PublicHeader.vue'
import ProviderIcon from '../../components/common/ProviderIcon.vue'
import { getModelCatalog, type ModelOffer } from '../../api/catalog'
import { useAsyncData } from '../../composables/useAsyncData'
import { useLocale } from '../../composables/useLocale'
import { useAuthStore } from '../../stores/auth'
import {
  estimateOffer,
  groupModelOffers,
  isSpeechModel,
  modelCategory,
  priceRange,
  type ModelCard
} from '../../utils/catalog'
import type { MessageKey } from '../../i18n'
import { copyTextToClipboard } from '../../utils/clipboard'
import { textModelRequest } from '../../utils/modelExample'

const { t, locale } = useLocale()
const route = useRoute()
const auth = useAuthStore()
const isPublic = computed(() => route.path === '/models')
const { data, loading, error, reload } = useAsyncData(() => getModelCatalog(auth.isUser), {
  currency: 'CNY',
  personalized: false,
  models: [] as ModelOffer[]
})
const query = ref(typeof route.query.q === 'string' ? route.query.q : '')
const category = ref(
  typeof route.query.type === 'string' &&
    ['token', 'image', 'video', 'audio', 'embedding', 'moderation'].includes(route.query.type)
    ? route.query.type
    : 'all'
)
const provider = ref(typeof route.query.provider === 'string' ? route.query.provider : '')
const sort = ref(route.query.sort === 'price' ? 'price' : 'name')
watch(
  () => route.query,
  (params) => {
    query.value = typeof params.q === 'string' ? params.q : ''
    category.value =
      typeof params.type === 'string' &&
      ['token', 'image', 'video', 'audio', 'embedding', 'moderation'].includes(params.type)
        ? params.type
        : 'all'
    provider.value = typeof params.provider === 'string' ? params.provider : ''
    sort.value = params.sort === 'price' ? 'price' : 'name'
  }
)
const selected = ref<ModelCard | null>(null)
const detailOpen = ref(false)
const offerIndex = ref(0)
const inputTokens = ref(1000000)
const outputTokens = ref(1000000)
const quantity = ref(1)
const categories = [
  { value: 'all', key: 'allModels', icon: Grid },
  { value: 'token', key: 'textModels', icon: ChatLineRound },
  { value: 'image', key: 'imageModels', icon: Picture },
  { value: 'video', key: 'videoModels', icon: VideoCamera },
  { value: 'audio', key: 'audioModels', icon: Microphone },
  { value: 'embedding', key: 'embeddingModels', icon: Connection },
  { value: 'moderation', key: 'moderationModels', icon: Connection }
] as const
const models = computed(() => groupModelOffers(data.value.models))
const providers = computed(() => [
  ...new Map(models.value.map((m) => [m.provider, m.provider_name])).entries()
])
const typeCount = computed(() => new Set(models.value.map(modelCategory)).size)
let refreshTimer: ReturnType<typeof window.setInterval> | undefined
function refreshVisibleCatalog() {
  if (!document.hidden && !loading.value) void reload({ silent: true })
}
onMounted(() => {
  window.addEventListener('focus', refreshVisibleCatalog)
  refreshTimer = window.setInterval(refreshVisibleCatalog, 30000)
})
onBeforeUnmount(() => {
  window.removeEventListener('focus', refreshVisibleCatalog)
  if (refreshTimer) window.clearInterval(refreshTimer)
})
function startingPrice(model: ModelCard) {
  if (model.billing_meter === 'video') return Number.POSITIVE_INFINITY
  return Math.min(
    ...model.offers.map((o) =>
      o.billing_meter === 'token' ? o.input_price_micros : (o.unit_price_micros ?? Infinity)
    )
  )
}
const filtered = computed(() => {
  const search = query.value.trim().toLowerCase()
  return models.value
    .filter(
      (m) =>
        (category.value === 'all' || modelCategory(m) === category.value) &&
        (!provider.value || m.provider === provider.value) &&
        (!search ||
          [m.model, m.display_name, m.provider, m.provider_name].some((v) =>
            v.toLowerCase().includes(search)
          ))
    )
    .sort((a, b) =>
      sort.value === 'price'
        ? startingPrice(a) - startingPrice(b) || a.model.localeCompare(b.model)
        : a.model.localeCompare(b.model)
    )
})
const offer = computed(() => selected.value?.offers[offerIndex.value])
const estimated = computed(() =>
  offer.value
    ? estimateOffer(
        offer.value,
        inputTokens.value,
        ['embedding', 'moderation'].includes(modelCategory(offer.value)) ? 0 : outputTokens.value,
        quantity.value
      )
    : null
)
const currency = computed(() => (data.value.currency === 'USD' ? '$' : '¥'))
function money(value: number | null | undefined) {
  return value == null
    ? '—'
    : `${currency.value}${(value / 1000000).toLocaleString(locale.value, { maximumFractionDigits: 6 })}`
}
function range(
  model: ModelCard,
  field: 'input_price_micros' | 'output_price_micros' | 'unit_price_micros'
) {
  const r = priceRange(model.offers.map((o) => o[field]))
  return r ? (r[0] === r[1] ? money(r[0]) : `${money(r[0])} – ${money(r[1])}`) : '—'
}
function typeLabel(meter: ReturnType<typeof modelCategory>): MessageKey {
  return categories.find((c) => c.value === meter)?.key ?? 'textModels'
}
function openDetails(model: ModelCard) {
  selected.value = model
  offerIndex.value = 0
  detailOpen.value = true
}
function clearFilters() {
  query.value = ''
  category.value = 'all'
  provider.value = ''
}
async function copy(text: string) {
  try {
    await copyTextToClipboard(text)
    ElMessage.success(t('copiedText'))
  } catch {
    ElMessage.error(t('copyFailed'))
  }
}
const example = computed(() => {
  if (!offer.value) return ''
  const o = offer.value
  const category = modelCategory(o)
  const textRequest = category === 'token' ? textModelRequest(o) : null
  const path =
    category === 'embedding'
      ? '/v1/embeddings'
      : category === 'moderation'
        ? '/v1/moderations'
        : category === 'video'
          ? '/v1/videos'
          : category === 'image'
            ? '/v1/images/generations'
            : category === 'audio'
              ? '/v1/audio/transcriptions'
              : textRequest!.path
  const url = `${window.location.origin}${path}`
  if (isSpeechModel(o)) {
    const speechBody = JSON.stringify({
      model: o.model,
      input: 'Hello, welcome to Zhiqiao API.',
      voice: 'alloy',
      response_format: 'mp3'
    }).replace(/'/g, "'\\''")
    return `curl '${window.location.origin}/v1/audio/speech' -H 'Authorization: Bearer YOUR_API_KEY' -H 'Content-Type: application/json' -d '${speechBody}' --output speech.mp3`
  }
  if (category === 'audio')
    return `curl '${url}' \\\n  -H 'Authorization: Bearer YOUR_API_KEY' \\\n  -F 'model=${o.model.replace(/'/g, "'\\''")}' \\\n  -F 'file=@audio.mp3'`
  const body =
    category === 'embedding' || category === 'moderation'
      ? { model: o.model, input: 'Hello!' }
      : textRequest
        ? textRequest.body
        : { model: o.model, prompt: 'A quiet garden at sunrise' }
  const escaped = JSON.stringify(body, null, 2).replace(/'/g, "'\\''")
  const headers = {
    ...(textRequest?.headers ?? { Authorization: 'Bearer YOUR_API_KEY' }),
    'Content-Type': 'application/json'
  }
  const headerFlags = Object.entries(headers)
    .map(([name, value]) => `-H '${name}: ${value}'`)
    .join(' \\\n  ')
  return `curl '${url}' \\\n  ${headerFlags} \\\n  -d '${escaped}'`
})
watch(data, () => {
  if (!selected.value) return
  const next = groupModelOffers(data.value.models).find(
    (model) => model.key === selected.value?.key
  )
  selected.value = next ?? null
  if (!next) detailOpen.value = false
  else offerIndex.value = Math.min(offerIndex.value, next.offers.length - 1)
})
</script>

<template>
  <div :class="{ 'plaza-public': isPublic }">
    <PublicHeader v-if="isPublic" />
    <section class="model-plaza" :class="{ 'plaza-public-content': isPublic }">
      <header class="explorer-hero">
        <div>
          <span class="section-eyebrow"><span></span>{{ t('plazaEyebrow') }}</span>
          <h1>{{ t('plazaTitle') }}</h1>
          <p>{{ t('plazaIntro') }}</p>
          <div class="hero-caption">
            <span class="status-dot"></span
            >{{ t(data.personalized ? 'personalPrices' : 'basePrices')
            }}<span class="caption-separator">/</span>{{ t('currencyLabel') }} {{ data.currency }}
          </div>
        </div>
        <div class="explorer-stat-grid">
          <div>
            <strong>{{ loading ? '—' : models.length }}</strong
            ><span>{{ t('publishedModels') }}</span>
          </div>
          <div>
            <strong>{{ loading ? '—' : providers.length }}</strong
            ><span>{{ t('modelProviders') }}</span>
          </div>
          <div>
            <strong>{{ loading ? '—' : typeCount }}</strong
            ><span>{{ t('modelTypes') }}</span>
          </div>
        </div>
      </header>
      <div class="catalog-tabs" role="tablist" :aria-label="t('modelTypes')">
        <button
          v-for="tab in categories"
          :key="tab.value"
          :class="{ active: category === tab.value }"
          role="tab"
          :aria-selected="category === tab.value"
          @click="category = tab.value"
        >
          <el-icon><component :is="tab.icon" /></el-icon>{{ t(tab.key)
          }}<span>{{
            tab.value === 'all'
              ? models.length
              : models.filter((m) => modelCategory(m) === tab.value).length
          }}</span>
        </button>
      </div>
      <div class="catalog-toolbar">
        <el-input
          v-model="query"
          clearable
          :prefix-icon="Search"
          :placeholder="t('searchModels')"
          :aria-label="t('searchModels')"
        /><el-select
          v-model="provider"
          :aria-label="t('allProviders')"
          :placeholder="t('allProviders')"
          ><el-option :label="t('allProviders')" value="" /><el-option
            v-for="[code, name] in providers"
            :key="code"
            :label="name"
            :value="code" /></el-select
        ><el-select v-model="sort" :aria-label="t('sortModels')"
          ><el-option :label="t('sortName')" value="name" /><el-option
            :label="t('sortPrice')"
            value="price" /></el-select
        ><el-button
          :icon="RefreshRight"
          :aria-label="t('retryLoad')"
          :loading="loading"
          @click="reload()"
        />
      </div>
      <div class="catalog-results-line">
        <span>{{ t('modelsFound', { count: filtered.length }) }}</span
        ><span>{{ t(isPublic && !data.personalized ? 'publicPriceNotice' : 'priceNotice') }}</span>
      </div>
      <div v-if="loading" class="model-card-grid" aria-busy="true">
        <div v-for="n in 6" :key="n" class="model-card"><el-skeleton :rows="5" animated /></div>
      </div>
      <div v-else-if="error" class="catalog-empty">
        <el-icon><Connection /></el-icon>
        <h3>{{ t('catalogLoadError') }}</h3>
        <p>{{ error }}</p>
        <el-button @click="reload()">{{ t('retryLoad') }}</el-button>
      </div>
      <div v-else-if="!filtered.length" class="catalog-empty">
        <el-icon><Search /></el-icon>
        <h3>{{ t(models.length ? 'noModelMatches' : 'emptyCatalog') }}</h3>
        <p>{{ t(models.length ? 'noModelMatchesHint' : 'emptyCatalogHint') }}</p>
        <el-button v-if="models.length" @click="clearFilters">{{ t('clearFilters') }}</el-button>
      </div>
      <div v-else class="model-card-grid">
        <article
          v-for="model in filtered"
          :key="model.key"
          class="model-card"
          :class="`model-card-${model.billing_meter}`"
        >
          <div class="model-card-top">
            <span class="model-provider-logo"><ProviderIcon :provider="model.provider" /></span
            ><span class="model-type-pill" :class="model.billing_meter">{{
              t(typeLabel(modelCategory(model)))
            }}</span>
          </div>
          <div class="model-card-title">
            <h3>{{ model.display_name || model.model }}</h3>
            <span>{{ model.provider_name }}</span>
          </div>
          <button
            class="model-id-copy"
            :aria-label="`${t('copyModel')}: ${model.model}`"
            @click="copy(model.model)"
          >
            <code>{{ model.model }}</code
            ><el-icon><DocumentCopy /></el-icon>
          </button>
          <div class="model-card-prices" v-if="model.billing_meter === 'token'">
            <div>
              <span>{{ t('inputPrice') }}</span
              ><strong
                >{{ range(model, 'input_price_micros')
                }}<small>{{ t('perMillion') }}</small></strong
              >
            </div>
            <div>
              <span>{{ t('outputPrice') }}</span
              ><strong
                >{{ range(model, 'output_price_micros')
                }}<small>{{ t('perMillion') }}</small></strong
              >
            </div>
          </div>
          <div v-else-if="model.billing_meter !== 'video'" class="model-unit-price">
            <strong>{{ range(model, 'unit_price_micros') }}</strong
            ><span>{{ t(model.billing_meter === 'image' ? 'perImage' : 'perSecond') }}</span>
          </div>
          <div v-else class="model-video-price">
            <el-icon><VideoCamera /></el-icon><strong>{{ t('videoTiered') }}</strong
            ><span>{{
              [
                ...new Set(
                  model.offers.flatMap((o) => o.video_price_tiers.flatMap((v) => v.resolutions))
                )
              ].join(' · ')
            }}</span>
          </div>
          <div class="model-card-footer">
            <span>{{
              model.protocols
                .map((p) =>
                  p === 'openai_oauth' ? 'OpenAI' : p === 'anthropic' ? 'Anthropic' : 'OpenAI'
                )
                .filter((v, i, a) => a.indexOf(v) === i)
                .join(' / ')
            }}</span
            ><button @click="openDetails(model)">
              {{ t('viewModel') }}<el-icon><ArrowRight /></el-icon>
            </button>
          </div>
        </article>
      </div>
      <footer class="catalog-bottom-note">
        <el-icon><Connection /></el-icon><span>{{ t('unifiedApi') }}</span
        ><RouterLink to="/interfaces">{{ t('apiDocs') }} ↗</RouterLink>
      </footer>
    </section>
    <el-drawer
      v-model="detailOpen"
      :title="selected?.display_name || selected?.model"
      size="600px"
      class="model-detail-drawer"
    >
      <template v-if="selected && offer">
        <div class="model-detail-identity">
          <span class="model-provider-logo"><ProviderIcon :provider="selected.provider" /></span>
          <div>
            <strong>{{ selected.provider_name }}</strong
            ><code>{{ selected.model }}</code>
          </div>
          <el-button
            :icon="DocumentCopy"
            :aria-label="t('copyModel')"
            @click="copy(selected.model)"
          />
        </div>
        <section class="detail-section">
          <h3>{{ t('priceDetails') }}</h3>
          <p>{{ t('offerHint') }}</p>
          <el-select
            v-if="selected.offers.length > 1"
            v-model="offerIndex"
            :aria-label="t('priceOffer')"
            ><el-option
              v-for="(_item, i) in selected.offers"
              :key="i"
              :label="`${t('priceOffer')} ${i + 1}`"
              :value="i"
          /></el-select>
          <dl class="detail-price-list" v-if="offer.billing_meter === 'token'">
            <div>
              <dt>{{ t('inputPrice') }}</dt>
              <dd>
                {{ money(offer.input_price_micros) }} <small>{{ t('perMillion') }}</small>
              </dd>
            </div>
            <div>
              <dt>{{ t('outputPrice') }}</dt>
              <dd>
                {{ money(offer.output_price_micros) }} <small>{{ t('perMillion') }}</small>
              </dd>
            </div>
            <div>
              <dt>{{ t('cacheRead') }}</dt>
              <dd>
                {{ money(offer.cache_read_price_micros) }} <small>{{ t('perMillion') }}</small>
              </dd>
            </div>
            <div>
              <dt>{{ t('cacheWrite') }}</dt>
              <dd>
                {{ money(offer.cache_write_price_micros) }} <small>{{ t('perMillion') }}</small>
              </dd>
            </div>
          </dl>
          <div v-else-if="offer.billing_meter === 'video'" class="video-tiers">
            <div v-for="(tier, i) in offer.video_price_tiers" :key="i">
              <strong>{{ tier.resolutions.join(' / ') }}</strong
              ><span>{{
                offer.video_billing_mode === 'per_second' ? t('perSecond') : t('perMillion')
              }}</span>
              <dl>
                <dt>{{ t('withoutVideo') }}</dt>
                <dd>
                  {{
                    money(
                      offer.video_billing_mode === 'per_second'
                        ? tier.input_without_video_unit_micros
                        : tier.input_without_video_micros
                    )
                  }}
                </dd>
                <dt>{{ t('withVideo') }}</dt>
                <dd>
                  {{
                    money(
                      offer.video_billing_mode === 'per_second'
                        ? tier.input_with_video_unit_micros
                        : tier.input_with_video_micros
                    )
                  }}
                </dd>
              </dl>
            </div>
          </div>
          <div v-else class="detail-unit-price">
            {{ money(offer.unit_price_micros) }}
            <small>{{ t(offer.billing_meter === 'image' ? 'perImage' : 'perSecond') }}</small>
          </div>
        </section>
        <section class="detail-section">
          <h3>{{ t('estimate') }}</h3>
          <p v-if="offer.billing_meter === 'video'">{{ t('videoEstimateHint') }}</p>
          <template v-else
            ><div class="estimate-fields" v-if="offer.billing_meter === 'token'">
              <label
                >{{ t('inputTokens')
                }}<el-input-number
                  v-model="inputTokens"
                  :min="0"
                  :max="1000000000"
                  :step="1000"
                  :precision="0"
                  controls-position="right" /></label
              ><label v-if="!['embedding', 'moderation'].includes(modelCategory(offer))"
                >{{ t('outputTokens')
                }}<el-input-number
                  v-model="outputTokens"
                  :min="0"
                  :max="1000000000"
                  :step="1000"
                  :precision="0"
                  controls-position="right"
              /></label>
            </div>
            <label class="estimate-quantity" v-else
              >{{ t('quantity')
              }}<el-input-number
                v-model="quantity"
                :min="0"
                :max="1000000000"
                controls-position="right"
            /></label>
            <div class="estimate-total">
              <span>{{ t('estimatedTotal') }}</span
              ><strong>{{ money(estimated) }}</strong>
            </div>
            <p>{{ t('estimateHint') }}</p></template
          >
        </section>
        <section class="detail-section">
          <div class="section-heading">
            <h3>{{ t('apiExample') }}</h3>
            <el-button size="small" :icon="DocumentCopy" @click="copy(example)">{{
              t('copyExample')
            }}</el-button>
          </div>
          <p v-if="modelCategory(offer) === 'token'">{{ t('tokenExampleHint') }}</p>
          <pre class="request-example"><code>{{ example }}</code></pre>
          <RouterLink class="detail-doc-link" to="/interfaces" @click="detailOpen = false"
            >{{ t('apiDocs') }} ↗</RouterLink
          >
        </section>
      </template>
    </el-drawer>
  </div>
</template>
