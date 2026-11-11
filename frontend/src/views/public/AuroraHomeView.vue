<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import { ArrowRight, Check, CopyDocument, Right } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import PublicHeader from '../../components/common/PublicHeader.vue'
import BrandLogo from '../../components/common/BrandLogo.vue'
import ProviderIcon from '../../components/common/ProviderIcon.vue'
import { getModelCatalog } from '../../api/catalog'
import { useAsyncData } from '../../composables/useAsyncData'
import { useStudio } from '../../composables/useStudio'
import { useLocale } from '../../composables/useLocale'
import { useSiteBrand } from '../../composables/useSiteBrand'
import { useAuthStore } from '../../stores/auth'
import { groupModelOffers, isChatModel, priceRange } from '../../utils/catalog'
import { copyTextToClipboard } from '../../utils/clipboard'
import { formatPricePerMillion, type BillingCurrency } from '../../utils/format'

const { s } = useStudio()
const { locale } = useLocale()
const { siteName } = useSiteBrand()
const auth = useAuthStore()
const { data, loading, error, reload } = useAsyncData(() => getModelCatalog(), null)
const models = computed(() => groupModelOffers(data.value?.models ?? []))
const chatModels = computed(() =>
  models.value.filter(
    (model) => model.billing_meter === 'token' && isChatModel(model.model, model.offers)
  )
)
const featured = computed(() => {
  const result: typeof chatModels.value = []
  const providers = new Set<string>()
  for (const model of chatModels.value) {
    const provider =
      model.provider === 'claude'
        ? 'anthropic'
        : model.provider === 'gemini'
          ? 'google'
          : model.provider
    if (!providers.has(provider)) {
      result.push(model)
      providers.add(provider)
    }
    if (result.length === 4) break
  }
  for (const model of chatModels.value) {
    if (result.length === 4) break
    if (!result.some((item) => item.key === model.key)) result.push(model)
  }
  return result
})
const currency = computed<BillingCurrency>(() => (data.value?.currency === 'USD' ? 'USD' : 'CNY'))
const endpoint = window.location.origin + '/v1'
const consolePath = computed(() =>
  auth.isAdmin ? '/admin/overview' : auth.isUser ? '/home/overview' : '/register'
)
const selectedModel = ref('')
const activeModel = computed(
  () => chatModels.value.find((model) => model.model === selectedModel.value) ?? featured.value[0]
)
const exampleModel = computed(() => activeModel.value?.model ?? 'YOUR_MODEL_ID')
const languages = ['Python', 'Node.js', 'cURL'] as const
const language = ref<(typeof languages)[number]>('Python')
const copied = ref<'endpoint' | 'code' | null>(null)
let copyTimer: ReturnType<typeof setTimeout> | undefined
onBeforeUnmount(() => clearTimeout(copyTimer))

const codeSample = computed(() => {
  const model = JSON.stringify(exampleModel.value)
  const base = JSON.stringify(endpoint)
  if (language.value === 'Node.js')
    return [
      'import OpenAI from "openai";',
      '',
      'const client = new OpenAI({',
      '  apiKey: process.env.ZHIQIAO_API_KEY,',
      '  baseURL: ' + base + ',',
      '});',
      '',
      'const response = await client.chat.completions.create({',
      '  model: ' + model + ',',
      '  messages: [{ role: "user", content: "Hello!" }],',
      '});'
    ].join('\n')
  if (language.value === 'cURL')
    return [
      'curl ' + JSON.stringify(endpoint + '/chat/completions') + ' \\',
      '  -H "Authorization: Bearer $ZHIQIAO_API_KEY" \\',
      '  -H "Content-Type: application/json" \\',
      "  -d '{",
      '    "model": ' + model + ',',
      '    "messages": [',
      '      { "role": "user", "content": "Hello!" }',
      '    ]',
      "  }'"
    ].join('\n')
  return [
    'from openai import OpenAI',
    'import os',
    '',
    'client = OpenAI(',
    '    api_key=os.environ["ZHIQIAO_API_KEY"],',
    '    base_url=' + base + ',',
    ')',
    '',
    'response = client.chat.completions.create(',
    '    model=' + model + ',',
    '    messages=[{"role": "user", "content": "Hello!"}],',
    ')'
  ].join('\n')
})
const codeLines = computed(() =>
  codeSample.value.split('\n').map((line) =>
    line
      .split(/("(?:[^"\\]|\\.)*"|'[^']*'|\b(?:from|import|const|new|await)\b)/g)
      .filter(Boolean)
      .map((text) => ({
        text,
        kind: /^["']/.test(text)
          ? 'string'
          : /^(from|import|const|new|await)$/.test(text)
            ? 'keyword'
            : ''
      }))
  )
)
function price(model: (typeof models.value)[number], output = false) {
  const range = priceRange(
    model.offers
      .filter((offer) => offer.billing_meter === 'token')
      .map((offer) => (output ? offer.output_price_micros : offer.input_price_micros))
  )
  return range
    ? formatPricePerMillion(range[0], currency.value, locale.value) +
        (range[0] !== range[1] ? '+' : '')
    : '—'
}
async function copy(kind: 'endpoint' | 'code') {
  try {
    await copyTextToClipboard(kind === 'endpoint' ? endpoint : codeSample.value)
    copied.value = kind
    clearTimeout(copyTimer)
    copyTimer = setTimeout(() => {
      copied.value = null
    }, 2000)
  } catch {
    ElMessage.info(s('请选择内容手动复制', 'Select the content to copy'))
  }
}
</script>

<template>
  <div class="gateway-home">
    <PublicHeader />
    <main>
      <section class="gateway-intro gateway-container">
        <RouterLink class="gateway-kicker" to="/models">
          <span class="gateway-kicker-mark" aria-hidden="true">✳</span>
          {{ s('为每一个好想法', 'For your next great idea') }}
          <ArrowRight />
        </RouterLink>
        <h1>
          {{ s('一个 API，', 'One API.') }}<span>{{ s('更多可能。', 'More possibilities.') }}</span>
        </h1>
        <p>
          {{
            s(
              '一个密钥，连接你需要的 AI 模型。',
              'Claude, GPT, Gemini. Your next idea starts with the right model.'
            )
          }}
        </p>
        <div class="gateway-actions">
          <RouterLink :to="consolePath" class="gateway-primary">
            {{ auth.isAuthed ? s('进入工作台', 'Open console') : s('开始使用', 'Get started')
            }}<ArrowRight />
          </RouterLink>
          <RouterLink to="/models" class="gateway-secondary">
            {{ s('探索模型', 'Explore models') }}<ArrowRight />
          </RouterLink>
        </div>
        <div class="gateway-provider-strip" :aria-label="s('模型提供商', 'Model providers')">
          <span><ProviderIcon provider="anthropic" />Claude</span>
          <span><ProviderIcon provider="openai" />OpenAI</span>
          <span><ProviderIcon provider="google" />Gemini</span>
          <span><ProviderIcon provider="deepseek" />DeepSeek</span>
        </div>
      </section>

      <section
        class="gateway-workspace-section"
        :aria-label="s('API 接入示例', 'API integration example')"
      >
        <div class="gateway-wash" aria-hidden="true"><i></i><i></i><i></i></div>
        <div class="gateway-workspace gateway-container">
          <header class="gateway-workspace-header">
            <div class="gateway-workspace-title">
              <span class="gateway-app-symbol" aria-hidden="true"><span></span><span></span></span>
              <strong>{{ siteName }}</strong
              ><span class="gateway-slash">/</span>
              <span>{{ s('快速接入', 'Quick start') }}</span>
            </div>
            <RouterLink to="/docs" class="gateway-docs-link"
              >{{ s('查看文档', 'Read the docs') }}<ArrowRight
            /></RouterLink>
          </header>
          <div class="gateway-workspace-body">
            <aside class="gateway-model-picker">
              <div class="gateway-picker-label">{{ s('选择模型', 'CHOOSE A MODEL') }}</div>
              <div v-if="loading" class="gateway-picker-loading" aria-live="polite">
                <span v-for="n in 3" :key="n"></span>
              </div>
              <template v-else-if="featured.length">
                <button
                  v-for="model in featured"
                  :key="model.key"
                  :class="{ selected: activeModel?.key === model.key }"
                  :aria-pressed="activeModel?.key === model.key"
                  @click="selectedModel = model.model"
                >
                  <ProviderIcon :provider="model.provider" />
                  <span>{{ model.display_name }}</span>
                  <span
                    v-if="activeModel?.key === model.key"
                    class="gateway-selection-dot"
                    aria-hidden="true"
                  ></span>
                </button>
              </template>
              <p v-else class="gateway-picker-empty">
                {{ s('模型目录暂不可用', 'Catalog unavailable') }}
              </p>
              <RouterLink to="/models" class="gateway-all-models">
                {{ s('全部模型', 'All models') }}<ArrowRight />
              </RouterLink>
              <div class="gateway-compatible">
                <Check />{{ s('兼容 OpenAI SDK', 'OpenAI SDK compatible') }}
              </div>
            </aside>
            <div class="gateway-code-panel">
              <div class="gateway-code-toolbar">
                <div
                  class="gateway-code-tabs"
                  :aria-label="s('示例代码语言', 'Code example language')"
                >
                  <button
                    v-for="item in languages"
                    :key="item"
                    :aria-pressed="language === item"
                    :class="{ selected: language === item }"
                    @click="language = item"
                  >
                    {{ item }}
                  </button>
                </div>
                <button
                  class="gateway-copy-button"
                  @click="copy('code')"
                  :aria-label="s('复制完整代码', 'Copy integration code')"
                >
                  <Check v-if="copied === 'code'" /><CopyDocument v-else />
                  <span>{{
                    copied === 'code' ? s('已复制', 'Copied') : s('复制代码', 'Copy code')
                  }}</span>
                </button>
              </div>
              <div
                class="gateway-code-content"
                tabindex="0"
                :aria-label="s('接入示例代码', 'Integration example')"
              >
                <div v-for="(line, index) in codeLines" :key="index" class="gateway-code-line">
                  <span class="gateway-line-number" aria-hidden="true">{{
                    String(index + 1).padStart(2, '0')
                  }}</span>
                  <code
                    ><span
                      v-for="(token, tokenIndex) in line"
                      :key="tokenIndex"
                      :class="token.kind"
                      >{{ token.text }}</span
                    ></code
                  >
                </div>
              </div>
            </div>
          </div>
          <footer class="gateway-workspace-footer">
            <button
              class="gateway-endpoint"
              @click="copy('endpoint')"
              :aria-label="s('复制 API 接入地址', 'Copy API endpoint')"
            >
              <span>BASE URL</span><code>{{ endpoint }}</code>
              <Check v-if="copied === 'endpoint'" /><CopyDocument v-else />
            </button>
            <RouterLink
              :to="{ path: '/playground', query: activeModel ? { model: exampleModel } : {} }"
              class="gateway-playground-link"
            >
              {{ s('去调试台试试', 'Try in Playground') }}<Right />
            </RouterLink>
          </footer>
        </div>
        <div class="gateway-workspace-note">
          <span>{{ s('一个密钥', 'One API key') }}</span
          ><i></i><span>{{ s('按量计费', 'Pay as you go') }}</span
          ><i></i><span>{{ s('自由切换模型', 'Switch models freely') }}</span>
        </div>
      </section>

      <section class="gateway-catalog gateway-container">
        <header class="gateway-section-heading">
          <div>
            <span class="gateway-section-label">{{ s('模型与价格', 'MODELS & PRICING') }}</span>
            <h2>{{ s('选你所需。', 'Find your fit.') }}</h2>
          </div>
          <RouterLink to="/models"
            >{{ s('浏览全部模型', 'Browse all models') }}<ArrowRight
          /></RouterLink>
        </header>
        <el-alert v-if="error" :title="error" type="error" :closable="false">
          <el-button text @click="reload()">{{ s('重新加载', 'Retry') }}</el-button>
        </el-alert>
        <div v-else v-loading="loading" class="gateway-price-list">
          <div class="gateway-price-head">
            <span>{{ s('模型', 'Model') }}</span
            ><span>{{ s('输入 / 百万 Tokens', 'Input / 1M tokens') }}</span
            ><span>{{ s('输出 / 百万 Tokens', 'Output / 1M tokens') }}</span
            ><span></span>
          </div>
          <RouterLink
            v-for="model in featured"
            :key="model.key"
            :to="{ path: '/models', query: { q: model.model } }"
            class="gateway-price-row"
          >
            <div class="gateway-price-model">
              <ProviderIcon :provider="model.provider" />
              <div>
                <strong>{{ model.display_name }}</strong
                ><small>{{ model.model }}</small>
              </div>
            </div>
            <div class="gateway-price-value">
              <small>{{ s('输入', 'Input') }}</small
              >{{ price(model) }}
            </div>
            <div class="gateway-price-value">
              <small>{{ s('输出', 'Output') }}</small
              >{{ price(model, true) }}
            </div>
            <ArrowRight />
          </RouterLink>
          <p v-if="!loading && !featured.length" class="gateway-empty">
            {{ s('模型目录暂未开放', 'The catalog is not available yet') }}
          </p>
        </div>
        <p v-if="featured.length && !error" class="gateway-price-note">
          {{
            s(
              '价格 / 百万 Tokens · 多组报价显示起价。',
              'Prices per 1M tokens. Starting prices shown.'
            )
          }}
        </p>
      </section>
    </main>
    <footer class="gateway-footer gateway-container">
      <div class="gateway-footer-brand">
        <BrandLogo /><span>© {{ new Date().getFullYear() }} {{ siteName }}</span>
      </div>
      <nav :aria-label="s('页脚导航', 'Footer navigation')">
        <RouterLink to="/docs">{{ s('接入文档', 'Docs') }}</RouterLink>
        <RouterLink to="/terms">{{ s('用户协议', 'Terms') }}</RouterLink>
        <RouterLink to="/privacy">{{ s('隐私政策', 'Privacy') }}</RouterLink>
      </nav>
    </footer>
  </div>
</template>

<style src="../../styles/gateway-home.css"></style>
