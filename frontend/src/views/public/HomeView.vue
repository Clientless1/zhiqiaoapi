<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ArrowRight, CopyDocument, Check, Search, Close } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus/es/components/message/index'
import PublicHeader from '../../components/common/PublicHeader.vue'
import ProviderIcon from '../../components/common/ProviderIcon.vue'
import BrandLogo from '../../components/common/BrandLogo.vue'
import { getModelCatalog } from '../../api/catalog'
import { useAsyncData } from '../../composables/useAsyncData'
import { useStudio } from '../../composables/useStudio'
import { useLocale } from '../../composables/useLocale'
import { useSiteBrand } from '../../composables/useSiteBrand'
import { useAuthStore } from '../../stores/auth'
import { groupModelOffers, isChatModel, priceRange } from '../../utils/catalog'
import { formatMoney, formatPricePerMillion, type BillingCurrency } from '../../utils/format'

const { s } = useStudio()
const { locale } = useLocale()
const { siteName } = useSiteBrand()
const auth = useAuthStore()
const { data, loading, error, reload } = useAsyncData(() => getModelCatalog(), null)
const models = computed(() => groupModelOffers(data.value?.models ?? []))
const chatModels = computed(() =>
  models.value.filter((m) => m.protocols.includes('openai') && isChatModel(m.model, m.offers))
)
const currency = computed<BillingCurrency>(() => (data.value?.currency === 'USD' ? 'USD' : 'CNY'))
const query = ref('')
const visibleModels = computed(() =>
  models.value
    .filter((m) =>
      (m.display_name + ' ' + m.model + ' ' + m.provider_name)
        .toLowerCase()
        .includes(query.value.trim().toLowerCase())
    )
    .slice(0, 6)
)
const selectedModel = ref('')
watch(
  chatModels,
  (list) => {
    if (!list.some((m) => m.model === selectedModel.value))
      selectedModel.value = list[0]?.model ?? ''
  },
  { immediate: true }
)
const exampleModel = computed(() => selectedModel.value || 'YOUR_MODEL_ID')
const language = ref<'python' | 'javascript' | 'curl'>('python')
const languages = [
  { id: 'python', name: 'Python' },
  { id: 'javascript', name: 'JavaScript' },
  { id: 'curl', name: 'cURL' }
] as const
const code = computed(() => {
  const base = window.location.origin + '/v1'
  if (language.value === 'javascript')
    return [
      'import OpenAI from "openai";',
      '',
      'const client = new OpenAI({',
      '  apiKey: "YOUR_API_KEY",',
      '  baseURL: "' + base + '"',
      '});',
      '',
      'const response = await client.chat.completions.create({',
      '  model: "' + exampleModel.value + '",',
      '  messages: [{ role: "user", content: "Hello, world!" }]',
      '});',
      '',
      'console.log(response.choices[0].message.content);'
    ].join('\n')
  if (language.value === 'curl')
    return [
      'curl ' + base + '/chat/completions \\',
      '  -H "Authorization: Bearer YOUR_API_KEY" \\',
      '  -H "Content-Type: application/json" \\',
      "  -d '{",
      '    "model": "' + exampleModel.value + '",',
      '    "messages": [{',
      '      "role": "user",',
      '      "content": "Hello, world!"',
      '    }]',
      "  }'"
    ].join('\n')
  return [
    'from openai import OpenAI',
    '',
    'client = OpenAI(',
    '    api_key="YOUR_API_KEY",',
    '    base_url="' + base + '"',
    ')',
    '',
    'response = client.chat.completions.create(',
    '    model="' + exampleModel.value + '",',
    '    messages=[{"role": "user", "content": "Hello, world!"}]',
    ')',
    '',
    'print(response.choices[0].message.content)'
  ].join('\n')
})
const codeLines = computed(() =>
  code.value
    .split('\n')
    .map((line) =>
      line
        .split(/("(?:[^"\\]|\\.)*"|'[^']*'|\b(?:from|import|const|await|print)\b)/g)
        .map((text) => ({
          text,
          kind: /^["']/.test(text)
            ? 'string'
            : /^(from|import|const|await|print)$/.test(text)
              ? 'keyword'
              : ''
        }))
    )
)
const consolePath = computed(() =>
  auth.isAdmin ? '/admin/overview' : auth.isUser ? '/home/overview' : '/register'
)
const copied = ref(false)
async function copyCode() {
  try {
    await navigator.clipboard.writeText(code.value)
    copied.value = true
    ElMessage.success(s('接入代码已复制', 'Integration code copied'))
  } catch {
    ElMessage.warning(s('请选中代码手动复制', 'Please select and copy the code'))
  }
}
watch([language, selectedModel], () => {
  copied.value = false
})
function price(model: (typeof models.value)[number], direction: 'input' | 'output') {
  if (model.billing_meter === 'video') return s('查看阶梯报价', 'View tiered pricing')
  const range = priceRange(
    model.offers.map((o) =>
      model.billing_meter === 'token'
        ? direction === 'input'
          ? o.input_price_micros
          : o.output_price_micros
        : o.unit_price_micros
    )
  )
  if (!range) return '—'
  const formatted =
    model.billing_meter === 'token'
      ? formatPricePerMillion(range[0], currency.value, locale.value)
      : formatMoney(range[0], currency.value, locale.value, 4)
  return formatted + (range[0] !== range[1] ? '+' : '')
}
const questions = computed(() => [
  {
    q: s('已有 OpenAI SDK，怎么迁移？', 'Can I use my existing OpenAI SDK?'),
    a: s(
      '将 base_url（JavaScript 中为 baseURL）改为本站 /v1 地址，填入平台创建的 API Key，并选择已上架且支持对应协议的模型。可复制上方示例，或查看接入文档了解模型差异。',
      'Set base_url (baseURL in JavaScript) to this site’s /v1 endpoint, use your platform API key, and select a listed model that supports the protocol. Copy the example above or check the integration docs for model-specific differences.'
    )
  },
  {
    q: s('模型费用怎么查看？', 'Where can I see model pricing?'),
    a: s(
      '模型广场提供当前平台报价、协议和计费方式，并支持模型对比与成本估算。实际费用根据套餐、模型、请求用量及平台结算规则计算；首页价格仅作目录预览。',
      'The model catalog shows current offers, protocols and billing methods, with comparison and cost estimation. Actual charges follow your package, model, usage and platform billing rules; homepage pricing is a catalog preview.'
    )
  },
  {
    q: s('可以先测试，再接入应用吗？', 'Can I test a model before integrating?'),
    a: s(
      '可以。打开 API 调试台，选择模型并填写平台 API Key，即可查看流式响应、耗时及接口返回的 Token 用量。调试台发起真实请求，会按账户规则计费。',
      'Yes. Open Playground, choose a model and enter your platform API key to inspect streaming responses, timing and reported token usage. Playground makes real requests billed under your account rules.'
    )
  },
  {
    q: s('在哪里查看运行状态和调用记录？', 'Where can I check service status and requests?'),
    a: s(
      '服务状态页展示网关、数据库与计费队列的当前检查结果。登录工作台后，可以查看账户用量、消费趋势及请求明细。',
      'The status page checks the gateway, database and billing queue. Sign in to your workspace to see account usage, spend trends and request details.'
    )
  }
])
</script>

<template>
  <div id="home-top" class="studio-landing gateway-home">
    <PublicHeader header-class="gateway-header" />
    <main>
      <section class="gh-hero gh-container" aria-labelledby="gh-title">
        <div class="gh-hero-copy">
          <div class="gh-eyebrow">
            <span class="gh-brand-mark" aria-hidden="true"><i></i><i></i><i></i></span
            >{{ s('智桥 API / 为开发者而建', 'ZHIQIAO API / BUILT FOR DEVELOPERS') }}
          </div>
          <h1 id="gh-title">
            {{ s('连接模型，', 'One API.') }}<br /><span>{{
              s('构建更多。', 'More to build.')
            }}</span>
          </h1>
          <p class="gh-hero-description">
            {{
              s(
                '让模型接入变简单。兼容 OpenAI SDK，从模型选择、在线调试到用量管理，把精力留给你的产品。',
                'Less time connecting models. More time building your product. A familiar OpenAI-compatible API, with model discovery, testing and usage in one place.'
              )
            }}
          </p>
          <div class="gh-hero-actions">
            <RouterLink class="gh-button" :to="consolePath"
              >{{ auth.isAuthed ? s('进入工作台', 'Open workspace') : s('开始使用', 'Get started')
              }}<ArrowRight /></RouterLink
            ><a class="gh-text-link" href="#model-catalog"
              >{{ s('查看模型与价格', 'Models & pricing') }}<span>↗</span></a
            >
          </div>
          <div class="gh-hero-notes">
            <span><Check />{{ s('兼容 OpenAI SDK', 'OpenAI SDK compatible') }}</span
            ><span><Check />{{ s('按实际用量计费', 'Usage-based billing') }}</span>
          </div>
        </div>
        <div class="gh-integration">
          <div class="gh-routing" aria-hidden="true">
            <div class="gh-route-app"><span>⌘</span><small>YOUR APP</small></div>
            <div class="gh-route-track"><i></i></div>
            <div class="gh-route-gateway">
              <span class="gh-bridge-glyph">Z<span>↗</span></span
              ><small>{{ siteName }}</small>
            </div>
            <div class="gh-route-track"><i></i></div>
            <div class="gh-route-model"><span>✳</span><small>AI MODELS</small></div>
          </div>
          <div class="gh-editor">
            <div class="gh-editor-toolbar">
              <div
                class="gh-code-tabs"
                role="group"
                :aria-label="s('示例语言', 'Example language')"
              >
                <button
                  v-for="item in languages"
                  :key="item.id"
                  :aria-pressed="language === item.id"
                  :class="{ active: language === item.id }"
                  @click="language = item.id"
                >
                  {{ item.name }}
                </button>
              </div>
              <button
                class="gh-copy"
                :aria-label="s('复制接入代码', 'Copy integration code')"
                @click="copyCode"
              >
                <Check v-if="copied" /><CopyDocument v-else />
              </button>
            </div>
            <pre
              class="gh-source"
              :aria-label="s('接入示例代码', 'Integration example code')"
            ><code><span v-for="(line, index) in codeLines" :key="index" class="gh-code-line"><span class="gh-line-number" aria-hidden="true">{{ String(index + 1).padStart(2, '0') }}</span><span><span v-for="(token, i) in line" :key="i" :class="token.kind ? 'gh-token-' + token.kind : undefined">{{ token.text }}</span></span>{{ '\n' }}</span></code></pre>
            <div class="gh-editor-model">
              <label for="gh-model">{{ s('示例模型', 'Example model') }}</label
              ><select id="gh-model" v-model="selectedModel" :disabled="!chatModels.length">
                <option v-if="!chatModels.length" value="">
                  {{ loading ? s('加载中…', 'Loading…') : 'YOUR_MODEL_ID' }}
                </option>
                <option v-for="model in chatModels" :key="model.key" :value="model.model">
                  {{ model.model }}
                </option>
              </select>
            </div>
          </div>
          <div class="gh-editor-footer">
            <span>{{ s('替换 Key，发送你的第一条请求。', 'Your key. Your first request.') }}</span
            ><RouterLink to="/playground">{{ s('在线调试', 'Try Playground') }} ↗</RouterLink>
          </div>
        </div>
      </section>

      <div class="gh-proof gh-container">
        <span class="gh-proof-label">{{
          s('从接入到管理，一处完成', 'FROM INTEGRATION TO OPERATIONS')
        }}</span>
        <div>
          <b
            >{{ !loading && !error ? models.length : '—' }}<small>{{ s('个', '') }}</small></b
          ><span>{{ s('当前上架模型', 'Listed models') }}</span>
        </div>
        <div>
          <b>1 <small>API</small></b
          ><span>{{ s('统一接入地址', 'Unified endpoint') }}</span>
        </div>
        <div>
          <b>SDK <span class="gh-inline-arrow">↗</span></b
          ><span>{{ s('沿用熟悉的开发方式', 'A familiar way to build') }}</span>
        </div>
        <RouterLink to="/status" class="gh-proof-status"
          >{{ s('查看实时服务状态', 'View live service status') }}<span>↗</span></RouterLink
        >
      </div>

      <section id="model-catalog" class="gh-catalog-section">
        <div class="gh-container">
          <header class="gh-section-header">
            <div>
              <span class="gh-section-label">01 / {{ s('模型目录', 'MODEL CATALOG') }}</span>
              <h2>
                {{ s('先选对模型，', 'Find your model.') }}<br />{{
                  s('再开始创造。', 'Then make it yours.')
                }}
              </h2>
            </div>
            <div class="gh-section-aside">
              <p>
                {{
                  s(
                    '价格、能力、协议，放在一起看。\n为你的下一次请求，找到合适的选择。',
                    'Compare pricing, capabilities and protocols.\nFind the right fit for your next request.'
                  )
                }}
              </p>
              <RouterLink class="gh-text-link" to="/models"
                >{{ s('探索完整模型广场', 'Explore the full catalog') }}<span>↗</span></RouterLink
              >
            </div>
          </header>
          <div class="gh-catalog-toolbar">
            <div class="gh-catalog-caption">
              <span class="gh-square"></span>{{ s('平台当前报价', 'CURRENT PLATFORM OFFERS')
              }}<span>{{ currency }}</span>
            </div>
            <div class="gh-search">
              <Search /><input
                v-model="query"
                type="search"
                :aria-label="s('搜索首页模型', 'Search homepage models')"
                :placeholder="s('搜索模型名称…', 'Search models…')"
              /><button
                v-if="query"
                :aria-label="s('清空模型搜索', 'Clear model search')"
                @click="query = ''"
              >
                <Close />
              </button>
            </div>
          </div>
          <div v-if="error" class="gh-empty" role="alert">
            <p>
              {{ s('模型目录暂时无法加载。', 'The model catalog is temporarily unavailable.') }}
            </p>
            <button class="gh-text-link" @click="reload()">
              {{ s('重新加载', 'Try again') }} ↻
            </button>
          </div>
          <div v-else-if="loading" class="gh-loading" role="status">
            {{ s('正在获取最新模型报价…', 'Loading current model offers…') }}
          </div>
          <div v-else-if="!visibleModels.length" class="gh-empty">
            <p>
              {{
                query
                  ? s('没有找到匹配的模型。', 'No matching models.')
                  : s('模型上架后会显示在这里。', 'Listed models will appear here.')
              }}
            </p>
            <button v-if="query" class="gh-text-link" @click="query = ''">
              {{ s('清空搜索', 'Clear search') }} ↗
            </button>
          </div>
          <div v-else class="gh-model-table">
            <div class="gh-model-table-head" aria-hidden="true">
              <span>{{ s('模型 / 供应商', 'MODEL / PROVIDER') }}</span
              ><span>{{ s('支持协议', 'PROTOCOLS') }}</span
              ><span>{{ s('输入价格', 'INPUT') }}</span
              ><span>{{ s('输出价格', 'OUTPUT') }}</span
              ><span></span>
            </div>
            <RouterLink
              v-for="model in visibleModels"
              :key="model.key"
              class="gh-model-row"
              :to="{ path: '/models', query: { q: model.model } }"
              ><div class="gh-model-name">
                <ProviderIcon :provider="model.provider" />
                <div>
                  <h3>{{ model.display_name }}</h3>
                  <span>{{ model.provider_name }}</span>
                </div>
              </div>
              <div class="gh-model-protocols">
                <span v-for="protocol in model.protocols.slice(0, 2)" :key="protocol">{{
                  protocol
                }}</span>
              </div>
              <div class="gh-model-price">
                <small>{{ s('输入', 'Input') }}</small
                ><b>{{ price(model, 'input') }}</b
                ><span>{{
                  model.billing_meter === 'token'
                    ? '/ 1M tokens'
                    : model.billing_meter === 'video'
                      ? s('按报价详情', 'See offer')
                      : s('/ 计费单位', '/ unit')
                }}</span>
              </div>
              <div class="gh-model-price" :class="{ 'is-unit': model.billing_meter !== 'token' }">
                <small>{{ s('输出', 'Output') }}</small
                ><b>{{ model.billing_meter === 'token' ? price(model, 'output') : '—' }}</b
                ><span v-if="model.billing_meter === 'token'">/ 1M tokens</span>
              </div>
              <span class="gh-row-arrow" aria-hidden="true">↗</span></RouterLink
            >
          </div>
          <div class="gh-catalog-footnote">
            <p>
              {{
                s(
                  '以上为目录报价预览，完整报价及实际计费规则请查看模型详情。',
                  'Catalog preview. See model details for complete offers and billing rules.'
                )
              }}
            </p>
            <RouterLink :to="{ path: '/models', query: query ? { q: query } : {} }"
              >{{ s('查看全部模型', 'View all models') }}<ArrowRight
            /></RouterLink>
          </div>
        </div>
      </section>

      <section class="gh-workflow gh-container">
        <div class="gh-workflow-intro">
          <span class="gh-section-label">02 / {{ s('开发工作流', 'YOUR WORKFLOW') }}</span>
          <h2>
            {{ s('从第一条请求，', 'From a first request') }}<br />{{
              s('到你的下一款产品。', 'to your next product.')
            }}
          </h2>
          <p>
            {{
              s(
                '接入只是开始。把选模型、做验证和查用量连成一条顺手的工作流。',
                'Integration is just the start. Bring discovery, testing and usage into one straightforward workflow.'
              )
            }}
          </p>
          <RouterLink class="gh-button gh-button-light" to="/docs"
            >{{ s('阅读接入文档', 'Read the docs') }}<ArrowRight
          /></RouterLink>
        </div>
        <div class="gh-workflow-steps">
          <RouterLink to="/models"
            ><span class="gh-step-number">01</span>
            <div>
              <h3>{{ s('找到适合任务的模型', 'Choose a model for your task') }}</h3>
              <p>
                {{
                  s(
                    '筛选能力与协议，比较报价，收藏常用模型。',
                    'Filter capabilities and protocols, compare offers and save favorites.'
                  )
                }}
              </p>
              <span class="gh-step-tag"
                >{{ s('模型筛选', 'FILTER') }}<i>/</i>{{ s('价格对比', 'COMPARE') }}</span
              >
            </div>
            <span class="gh-step-arrow">↗</span></RouterLink
          ><RouterLink to="/playground"
            ><span class="gh-step-number">02</span>
            <div>
              <h3>{{ s('让第一条请求跑起来', 'Make your first request') }}</h3>
              <p>
                {{
                  s(
                    '在调试台验证响应，再把熟悉的 SDK 写进应用。',
                    'Test responses in Playground, then integrate with a familiar SDK.'
                  )
                }}
              </p>
              <span class="gh-step-tag">STREAMING<i>/</i>API PLAYGROUND</span>
            </div>
            <span class="gh-step-arrow">↗</span></RouterLink
          ><RouterLink :to="consolePath"
            ><span class="gh-step-number">03</span>
            <div>
              <h3>{{ s('看清每一笔调用与费用', 'Understand usage and spend') }}</h3>
              <p>
                {{
                  s(
                    '查看用量趋势与请求记录，为下一步优化提供依据。',
                    'Inspect usage trends and request logs to guide your next improvement.'
                  )
                }}
              </p>
              <span class="gh-step-tag"
                >{{ s('用量趋势', 'TRENDS') }}<i>/</i>{{ s('请求明细', 'REQUEST LOGS') }}</span
              >
            </div>
            <span class="gh-step-arrow">↗</span></RouterLink
          >
        </div>
      </section>

      <section class="gh-faq gh-container">
        <div>
          <span class="gh-section-label">03 / FAQ</span>
          <h2>
            {{ s('开始之前，', 'Before you start.') }}<br />{{
              s('你可能想知道。', 'A few useful answers.')
            }}
          </h2>
        </div>
        <div class="gh-questions">
          <details v-for="item in questions" :key="item.q">
            <summary>{{ item.q }}<span aria-hidden="true">+</span></summary>
            <p>{{ item.a }}</p>
          </details>
        </div>
      </section>

      <section class="gh-start">
        <div class="gh-container gh-start-inner">
          <div>
            <span class="gh-section-label">YOUR NEXT BUILD STARTS HERE</span>
            <h2>
              {{ s('你的下一步，', 'Your next idea.') }}<br /><span>{{
                s('现在就开始。', 'Ready when you are.')
              }}</span>
            </h2>
          </div>
          <div class="gh-start-actions">
            <RouterLink class="gh-button" :to="consolePath"
              >{{
                auth.isAuthed
                  ? s('打开工作台', 'Open workspace')
                  : s('创建你的账户', 'Create your account')
              }}<ArrowRight /></RouterLink
            ><RouterLink class="gh-text-link" to="/playground"
              >{{ s('先去调试台看看', 'Explore Playground') }} ↗</RouterLink
            >
          </div>
          <span class="gh-start-symbol" aria-hidden="true">↗</span>
        </div>
      </section>
    </main>
    <footer class="gh-footer gh-container">
      <div class="gh-footer-brand">
        <RouterLink to="/" :aria-label="siteName"><BrandLogo /></RouterLink>
        <p>{{ s('让模型接入变简单。', 'Make model integration simple.') }}</p>
      </div>
      <nav :aria-label="s('页脚导航', 'Footer navigation')">
        <RouterLink to="/models">{{ s('模型与价格', 'Models & pricing') }}</RouterLink
        ><RouterLink to="/docs">{{ s('接入文档', 'Documentation') }}</RouterLink
        ><RouterLink to="/status">{{ s('服务状态', 'Service status') }}</RouterLink
        ><RouterLink to="/about">{{ s('关于我们', 'About') }}</RouterLink>
      </nav>
      <div class="gh-footer-bottom">
        <span>© {{ new Date().getFullYear() }} {{ siteName }}</span
        ><span>BUILT FOR YOUR NEXT BUILD.</span
        ><a href="#home-top">{{ s('回到顶部', 'Back to top') }} ↑</a>
      </div>
    </footer>
  </div>
</template>

<style src="../../styles/homepage.css"></style>
