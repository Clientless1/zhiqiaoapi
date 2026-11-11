<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import {
  Search,
  Star,
  StarFilled,
  CopyDocument,
  RefreshRight,
  Close,
  ArrowRight
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus/es/components/message/index'
import { getModelCatalog, type ModelOffer } from '../../api/catalog'
import {
  groupModelOffers,
  modelCategory,
  priceRange,
  estimateOffer,
  isChatModel,
  type ModelCard
} from '../../utils/catalog'
import { useAsyncData } from '../../composables/useAsyncData'
import { useStudio } from '../../composables/useStudio'
import { useLocale } from '../../composables/useLocale'
import { formatPricePerMillion, type BillingCurrency } from '../../utils/format'
import PublicHeader from '../../components/common/PublicHeader.vue'
import ProviderIcon from '../../components/common/ProviderIcon.vue'
import '../../styles/model-catalog.css'
import ModelHealthStrip from '../../components/common/ModelHealthStrip.vue'
import { getModelHealth, type ModelHealthSnapshot } from '../../api/modelHealth'
const route = useRoute(),
  router = useRouter()
const { s } = useStudio(),
  { locale } = useLocale()
const isPublic = computed(() => route.path === '/models')
const { data, loading, error, reload } = useAsyncData(
  () => getModelCatalog(route.path.startsWith('/home/')),
  null
)
const healthSnapshot = ref<ModelHealthSnapshot | null>(null)
const healthUnavailable = ref(false)
let healthTimer: ReturnType<typeof setInterval> | undefined
let healthDisposed = false
async function refreshHealth() {
  try {
    const value = await getModelHealth()
    if (!healthDisposed) {
      healthSnapshot.value = value
      healthUnavailable.value = false
    }
  } catch {
    if (!healthDisposed) healthUnavailable.value = true
  }
}
onMounted(() => {
  void refreshHealth()
  healthTimer = setInterval(() => {
    if (document.visibilityState === 'visible') void refreshHealth()
  }, 60000)
})
onBeforeUnmount(() => {
  healthDisposed = true
  clearInterval(healthTimer)
})
const healthByModel = computed(
  () =>
    new Map(
      (healthSnapshot.value?.models ?? []).map((model) => [
        JSON.stringify([model.provider, model.model]),
        model
      ])
    )
)
function modelHealth(model: ModelCard) {
  return healthByModel.value.get(JSON.stringify([model.provider, model.model]))
}
function refreshCatalog() {
  void reload()
  void refreshHealth()
}
const currency = computed<BillingCurrency>(() => (data.value?.currency === 'USD' ? 'USD' : 'CNY'))
const search = ref(String(route.query.q ?? '')),
  provider = ref(''),
  category = ref(''),
  protocol = ref('')
const sort = ref(route.query.sort === 'price' ? 'price' : 'name'),
  favoritesOnly = ref(false),
  limit = ref(24)
function readFavorites(): string[] {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem('zhiqiao_model_favorites') || '[]')
    return Array.isArray(parsed) ? parsed.filter((v): v is string => typeof v === 'string') : []
  } catch {
    return []
  }
}
const favorites = ref(readFavorites()),
  selected = ref<string[]>([]),
  compareOpen = ref(false),
  detail = ref<ModelCard | null>(null)
function hasPrice(m: ModelCard, field: keyof ModelOffer) {
  return m.offers.some((offer) => typeof offer[field] === 'number')
}
const input = ref(10000),
  output = ref(2000),
  units = ref(1)
const cards = computed(() => groupModelOffers(data.value?.models ?? []))
const providers = computed(() => [
  ...new Map(cards.value.map((m) => [m.provider, m.provider_name])).entries()
])
const protocols = computed(() => [...new Set(cards.value.flatMap((m) => m.protocols))])
const categories = computed(() => [
  { id: '', name: s('全部模型', 'All models') },
  { id: 'token', name: s('文本对话', 'Text & chat') },
  { id: 'image', name: s('图像生成', 'Image') },
  { id: 'video', name: s('视频生成', 'Video') },
  { id: 'audio', name: s('语音音频', 'Audio') },
  { id: 'embedding', name: s('向量嵌入', 'Embeddings') },
  { id: 'moderation', name: s('内容审核', 'Moderation') }
])
const filtered = computed(() =>
  cards.value
    .filter((m) => {
      const q = search.value.trim().toLowerCase()
      return (
        (!q || `${m.model} ${m.display_name} ${m.provider_name}`.toLowerCase().includes(q)) &&
        (!provider.value || m.provider === provider.value) &&
        (!category.value || modelCategory(m) === category.value) &&
        (!protocol.value || m.protocols.includes(protocol.value)) &&
        (!favoritesOnly.value || favorites.value.includes(m.key))
      )
    })
    .sort((a, b) => {
      if (sort.value === 'price')
        return (
          a.billing_meter.localeCompare(b.billing_meter) ||
          minimumPrice(a) - minimumPrice(b) ||
          a.model.localeCompare(b.model)
        )
      return a.display_name.localeCompare(b.display_name)
    })
)
const compared = computed(() => cards.value.filter((m) => selected.value.includes(m.key)))
function minimumPrice(m: ModelCard) {
  return Math.min(
    ...m.offers.map((o) =>
      o.billing_meter === 'token' ? o.input_price_micros : (o.unit_price_micros ?? Infinity)
    )
  )
}
watch([search, provider, category, protocol, sort, favoritesOnly], () => {
  limit.value = 24
})
watch(
  () => route.query.q,
  (q) => {
    search.value = String(q ?? '')
  }
)
watch(
  () => route.query.sort,
  (q) => {
    sort.value = q === 'price' ? 'price' : 'name'
  }
)
function money(v: number | null | undefined) {
  return formatPricePerMillion(v, currency.value, locale.value)
}
function range(m: ModelCard, field: keyof ModelOffer) {
  const r = priceRange(
    m.offers.map((o) => (typeof o[field] === 'number' ? (o[field] as number) : null))
  )
  return r ? (r[0] === r[1] ? money(r[0]) : `${money(r[0])} – ${money(r[1])}`) : '—'
}
function categoryName(m: ModelOffer) {
  return categories.value.find((c) => c.id === modelCategory(m))?.name ?? m.billing_meter
}
function favorite(m: ModelCard) {
  favorites.value = favorites.value.includes(m.key)
    ? favorites.value.filter((k) => k !== m.key)
    : [...favorites.value, m.key]
  try {
    localStorage.setItem('zhiqiao_model_favorites', JSON.stringify(favorites.value))
  } catch {
    ElMessage.warning(s('浏览器无法保存收藏', 'Browser could not save favorites'))
  }
}
function compare(m: ModelCard) {
  if (selected.value.includes(m.key)) selected.value = selected.value.filter((k) => k !== m.key)
  else if (selected.value.length < 4) selected.value.push(m.key)
  else ElMessage.info(s('最多同时对比 4 个模型', 'Compare up to 4 models'))
}
async function copy(text: string) {
  try {
    await navigator.clipboard.writeText(text)
    ElMessage.success(s('已复制', 'Copied'))
  } catch {
    ElMessage.warning(s('复制失败，请手动复制', 'Please copy manually'))
  }
}
function reset() {
  search.value = ''
  provider.value = ''
  category.value = ''
  protocol.value = ''
  favoritesOnly.value = false
  sort.value = 'name'
}
function estimate(m: ModelCard) {
  const values = m.offers.map((o) => estimateOffer(o, input.value, output.value, units.value))
  const r = priceRange(values)
  return r
    ? r[0] === r[1]
      ? money(r[0])
      : `${money(r[0])} – ${money(r[1])}`
    : s('按具体配置计费', 'Configuration dependent')
}
function tryModel(m: ModelCard) {
  void router.push({
    path: route.path.startsWith('/admin')
      ? '/admin/playground'
      : route.path.startsWith('/home')
        ? '/home/playground'
        : '/playground',
    query: { model: m.model }
  })
}
</script>
<template>
  <div :class="{ 'studio-public': isPublic }">
    <PublicHeader v-if="isPublic" />
    <main class="model-workspace" :class="{ 'studio-public-main': isPublic }">
      <section class="catalog-v2">
        <header class="catalog-heading">
          <div class="catalog-title-row">
            <h1>{{ s('模型广场', 'Model catalog') }}</h1>
            <span class="catalog-total"
              >{{ loading ? '—' : cards.length }} {{ s('个模型', 'models') }}</span
            >
          </div>
          <div class="catalog-heading-actions">
            <el-button
              :icon="RefreshRight"
              :loading="loading"
              :aria-label="s('刷新模型', 'Refresh models')"
              @click="refreshCatalog()"
            />
          </div>
        </header>
        <div class="catalog-toolbar">
          <el-input
            v-model="search"
            clearable
            :prefix-icon="Search"
            :placeholder="s('搜索模型或提供方', 'Search models or providers')"
            :aria-label="s('搜索模型', 'Search models')"
            class="catalog-search"
          />
          <el-select
            v-model="provider"
            :placeholder="s('全部提供方', 'All providers')"
            :aria-label="s('提供方', 'Provider')"
            clearable
          >
            <el-option v-for="[id, name] in providers" :key="id" :value="id" :label="name" />
          </el-select>
          <el-select
            v-model="protocol"
            :placeholder="s('全部协议', 'All protocols')"
            :aria-label="s('接口协议', 'Protocol')"
            clearable
          >
            <el-option v-for="p in protocols" :key="p" :value="p" :label="p" />
          </el-select>
          <el-select v-model="sort" :aria-label="s('排序方式', 'Sort by')">
            <el-option value="name" :label="s('名称排序', 'Name')" />
            <el-option value="price" :label="s('同类价格升序', 'Price by billing type')" />
          </el-select>
          <button
            class="catalog-favorite-filter"
            :class="{ active: favoritesOnly }"
            :aria-pressed="favoritesOnly"
            @click="favoritesOnly = !favoritesOnly"
          >
            <el-icon><StarFilled v-if="favoritesOnly" /><Star v-else /></el-icon
            >{{ s('收藏', 'Saved') }}
          </button>
        </div>
        <nav class="catalog-tabs" :aria-label="s('模型类型', 'Model type')">
          <button
            v-for="c in categories"
            :key="c.id"
            :class="{ active: category === c.id }"
            :aria-pressed="category === c.id"
            @click="category = c.id"
          >
            {{ c.name
            }}<span>{{ cards.filter((m) => !c.id || modelCategory(m) === c.id).length }}</span>
          </button>
        </nav>
        <div class="catalog-meta">
          <span
            >{{ filtered.length }} {{ s('个结果', 'results')
            }}<button
              v-if="search || provider || protocol || category || favoritesOnly || sort !== 'name'"
              @click="reset"
            >
              {{ s('清除筛选', 'Clear filters') }}
            </button></span
          >
          <span
            >{{
              data?.personalized
                ? s('账户价格', 'Account pricing')
                : s('公开价格', 'Public pricing')
            }}
            · {{ data?.currency ?? '—' }}{{ s(' / 百万 tokens', ' / 1M tokens') }}</span
          >
        </div>
        <el-alert v-if="error" :title="error" type="error" :closable="false" show-icon />
        <div v-if="loading && !data" class="catalog-card-grid" aria-busy="true">
          <article v-for="n in 6" :key="n" class="catalog-card catalog-skeleton">
            <el-skeleton :rows="4" animated />
          </article>
        </div>
        <el-empty
          v-else-if="!filtered.length"
          :description="s('没有匹配的模型', 'No matching models')"
        >
          <el-button @click="reset">{{ s('清除筛选', 'Clear filters') }}</el-button>
        </el-empty>
        <div v-else class="catalog-card-grid">
          <article
            v-for="m in filtered.slice(0, limit)"
            :key="m.key"
            class="catalog-card"
            :class="{ 'is-selected': selected.includes(m.key) }"
          >
            <div class="catalog-card-top">
              <span class="catalog-provider-icon"><ProviderIcon :provider="m.provider" /></span>
              <div class="catalog-card-identity">
                <button class="catalog-model-name" :title="m.display_name" @click="detail = m">
                  {{ m.display_name }}
                </button>
                <span class="catalog-provider-name">{{ m.provider_name }}</span>
              </div>
              <div class="catalog-card-actions">
                <button
                  :aria-label="s('复制模型 ID', 'Copy model ID') + ': ' + m.model"
                  :title="s('复制模型 ID', 'Copy model ID')"
                  @click="copy(m.model)"
                >
                  <el-icon><CopyDocument /></el-icon>
                </button>
                <el-checkbox
                  :model-value="selected.includes(m.key)"
                  :aria-label="s('对比模型', 'Compare model') + ': ' + m.display_name"
                  :title="s('加入对比', 'Add to comparison')"
                  @change="compare(m)"
                />
              </div>
            </div>
            <button
              v-if="m.model !== m.display_name"
              class="catalog-model-id"
              :title="s('复制模型 ID', 'Copy model ID') + ': ' + m.model"
              @click="copy(m.model)"
            >
              {{ m.model }}
            </button>
            <dl class="catalog-prices">
              <template v-if="m.billing_meter === 'token'">
                <div>
                  <dt>{{ s('输入', 'Input') }}</dt>
                  <dd>{{ range(m, 'input_price_micros') }}<small>/ 1M</small></dd>
                </div>
                <div>
                  <dt>{{ s('输出', 'Output') }}</dt>
                  <dd>{{ range(m, 'output_price_micros') }}<small>/ 1M</small></dd>
                </div>
                <div v-if="hasPrice(m, 'cache_read_price_micros')">
                  <dt>{{ s('缓存读取', 'Cache read') }}</dt>
                  <dd>{{ range(m, 'cache_read_price_micros') }}<small>/ 1M</small></dd>
                </div>
                <div v-if="hasPrice(m, 'cache_write_price_micros')">
                  <dt>{{ s('缓存写入', 'Cache write') }}</dt>
                  <dd>{{ range(m, 'cache_write_price_micros') }}<small>/ 1M</small></dd>
                </div>
              </template>
              <template v-else>
                <div>
                  <dt>{{ s('单价', 'Unit price') }}</dt>
                  <dd>
                    {{ range(m, 'unit_price_micros') }}<small>/ {{ s('计费单位', 'unit') }}</small>
                  </dd>
                </div>
                <div>
                  <dt>{{ s('类型', 'Type') }}</dt>
                  <dd>{{ categoryName(m) }}</dd>
                </div>
              </template>
            </dl>
            <footer class="catalog-card-footer">
              <div class="catalog-labels">
                <span class="catalog-billing-label">{{
                  m.billing_meter === 'token' ? s('按量计费', 'Usage based') : categoryName(m)
                }}</span
                ><span v-for="p in m.protocols" :key="p">{{ p }}</span
                ><span v-if="m.offers.length > 1"
                  >{{ m.offers.length }} {{ s('组报价', 'offers') }}</span
                >
              </div>
              <button
                class="catalog-save-button"
                :class="{ saved: favorites.includes(m.key) }"
                :aria-label="
                  favorites.includes(m.key)
                    ? s('取消收藏', 'Remove favorite')
                    : s('收藏模型', 'Save model')
                "
                :aria-pressed="favorites.includes(m.key)"
                @click="favorite(m)"
              >
                <el-icon><StarFilled v-if="favorites.includes(m.key)" /><Star v-else /></el-icon>
              </button>
              <button
                class="catalog-detail-button"
                :aria-label="s('查看详情', 'View details') + ': ' + m.display_name"
                @click="detail = m"
              >
                {{ s('详情', 'Details') }}<el-icon><ArrowRight /></el-icon>
              </button>
            </footer>
            <ModelHealthStrip :health="modelHealth(m)" :unavailable="healthUnavailable" />
          </article>
        </div>
        <p class="catalog-monitor-note">
          {{
            s(
              '最近 24 小时探测 · 绿色正常 / 黄色波动 / 红色异常 / 灰色无数据',
              '24 hour probes · Green healthy / Amber degraded / Red failed / Gray no data'
            )
          }}
        </p>
        <p class="catalog-price-note">
          {{
            s(
              '多组报价显示价格区间。图像、音频与视频按各自计费单位计算。',
              'Multiple offers appear as ranges. Image, audio and video use their own billing units.'
            )
          }}<RouterLink to="/interfaces/billing">{{ s('计费说明', 'Billing guide') }} ↗</RouterLink>
        </p>
        <div v-if="filtered.length > limit" class="load-more">
          <el-button @click="limit += 24"
            >{{ s('加载更多', 'Load more') }} · {{ Math.min(limit, filtered.length) }} /
            {{ filtered.length }}</el-button
          >
        </div>
      </section>
      <Transition name="compare-bar"
        ><div v-if="compared.length" class="model-compare-bar">
          <span
            ><strong>{{ compared.length }} / 4</strong>
            {{ s('个模型待对比', 'models selected') }}</span
          >
          <div class="compare-chips">
            <button v-for="m in compared" :key="m.key" @click="compare(m)">
              {{ m.display_name }} ×
            </button>
          </div>
          <el-button type="primary" @click="compareOpen = true"
            >{{ s('开始对比', 'Compare now') }} →</el-button
          ><button
            class="icon-button"
            :aria-label="s('清空对比', 'Clear selection')"
            @click="selected = []"
          >
            <el-icon><Close /></el-icon>
          </button></div
      ></Transition>
      <el-dialog
        v-model="compareOpen"
        :title="s('模型价格对比', 'Compare model pricing')"
        width="min(1080px, 94vw)"
        class="studio-dialog"
        ><div class="estimate-controls">
          <label
            >{{ s('预计输入 tokens', 'Input tokens')
            }}<el-input-number v-model="input" :min="0" :max="100000000" :step="1000" /></label
          ><label
            >{{ s('预计输出 tokens', 'Output tokens')
            }}<el-input-number v-model="output" :min="0" :max="100000000" :step="1000" /></label
          ><label
            >{{ s('图像/音频计费单位', 'Image/audio billing units')
            }}<el-input-number v-model="units" :min="0" :max="100000"
          /></label>
        </div>
        <div class="comparison-scroll">
          <table class="comparison-table">
            <thead>
              <tr>
                <th>{{ s('模型', 'Model') }}</th>
                <th v-for="m in compared" :key="m.key">{{ m.display_name }}</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td>{{ s('提供方', 'Provider') }}</td>
                <td v-for="m in compared" :key="m.key">{{ m.provider_name }}</td>
              </tr>
              <tr>
                <td>{{ s('计费类型', 'Billing type') }}</td>
                <td v-for="m in compared" :key="m.key">{{ m.billing_meter }}</td>
              </tr>
              <tr>
                <td>{{ s('输入 / 1M tokens', 'Input / 1M tokens') }}</td>
                <td v-for="m in compared" :key="m.key">
                  {{ m.billing_meter === 'token' ? range(m, 'input_price_micros') : '—' }}
                </td>
              </tr>
              <tr>
                <td>{{ s('输出 / 1M tokens', 'Output / 1M tokens') }}</td>
                <td v-for="m in compared" :key="m.key">
                  {{ m.billing_meter === 'token' ? range(m, 'output_price_micros') : '—' }}
                </td>
              </tr>
              <tr>
                <td>{{ s('缓存读取 / 1M', 'Cache read / 1M') }}</td>
                <td v-for="m in compared" :key="m.key">
                  {{ range(m, 'cache_read_price_micros') }}
                </td>
              </tr>
              <tr>
                <td>{{ s('接口协议', 'Protocols') }}</td>
                <td v-for="m in compared" :key="m.key">{{ m.protocols.join(', ') }}</td>
              </tr>
              <tr class="estimate-row">
                <td>{{ s('本次用量估算', 'Estimated cost') }}</td>
                <td v-for="m in compared" :key="m.key">{{ estimate(m) }}</td>
              </tr>
            </tbody>
          </table>
        </div>
        <p class="studio-note">
          {{
            s(
              '估算分别使用每组完整报价计算，不包含缓存、阶梯或特殊参数的附加费用；以实际账单为准。视频请查看具体分辨率与计费配置。',
              'Estimates use each complete offer and exclude cache, tiers and parameter-specific charges. Actual billing applies. Check video resolution and billing configuration separately.'
            )
          }}
        </p></el-dialog
      >
      <el-drawer
        :model-value="!!detail"
        :title="detail?.display_name"
        size="min(620px, 96vw)"
        @close="detail = null"
        ><template v-if="detail"
          ><div class="detail-header">
            <ProviderIcon :provider="detail.provider" />
            <div>
              <h2>{{ detail.display_name }}</h2>
              <p>{{ detail.provider_name }} · {{ categoryName(detail) }}</p>
            </div>
          </div>
          <button class="model-code" @click="copy(detail.model)">
            <code>{{ detail.model }}</code
            ><el-icon><CopyDocument /></el-icon></button
          ><el-button
            v-if="isChatModel(detail.model, detail.offers) && detail.protocols.includes('openai')"
            type="primary"
            @click="tryModel(detail)"
            >{{ s('在调试台中打开', 'Open in Playground') }} ↗</el-button
          >
          <h3>{{ s('接口与报价', 'Protocols & offers') }}</h3>
          <article v-for="(offer, index) in detail.offers" :key="index" class="offer-detail">
            <div class="panel-heading">
              <strong>{{ s('报价', 'Offer') }} {{ index + 1 }}</strong
              ><span>{{ offer.protocols.join(' / ') }}</span>
            </div>
            <dl>
              <template v-if="offer.billing_meter === 'token'"
                ><dt>{{ s('输入 / 1M tokens', 'Input / 1M tokens') }}</dt>
                <dd>{{ money(offer.input_price_micros) }}</dd>
                <dt>{{ s('输出 / 1M tokens', 'Output / 1M tokens') }}</dt>
                <dd>{{ money(offer.output_price_micros) }}</dd>
                <dt>{{ s('缓存读取 / 1M tokens', 'Cache read / 1M tokens') }}</dt>
                <dd>{{ money(offer.cache_read_price_micros) }}</dd>
                <dt>{{ s('缓存写入 / 1M tokens', 'Cache write / 1M tokens') }}</dt>
                <dd>{{ money(offer.cache_write_price_micros) }}</dd></template
              ><template v-else
                ><dt>{{ s('计费类型', 'Billing type') }}</dt>
                <dd>{{ offer.billing_meter }}</dd>
                <dt>{{ s('每计费单位', 'Per billing unit') }}</dt>
                <dd>{{ money(offer.unit_price_micros) }}</dd></template
              >
            </dl>
            <template v-if="offer.billing_meter === 'video'"
              ><p>
                {{ s('视频计费配置', 'Video billing configuration') }}:
                {{ offer.video_billing_mode ?? '—' }}
              </p>
              <pre class="detail-json">{{ JSON.stringify(offer.video_price_tiers, null, 2) }}</pre>
            </template>
          </article>
          <details class="capability-details">
            <summary>{{ s('模型能力元数据', 'Capability metadata') }}</summary>
            <pre class="detail-json">{{ JSON.stringify(detail.capabilities, null, 2) }}</pre>
          </details>
          <p class="studio-note">
            {{
              s(
                '目录表示平台当前配置，并非上游实时可用性承诺。',
                'The catalog reflects platform configuration, not an upstream availability guarantee.'
              )
            }}
          </p></template
        ></el-drawer
      >
    </main>
  </div>
</template>
