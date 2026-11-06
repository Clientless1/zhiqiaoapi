<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  ArrowRight,
  CopyDocument,
  Check,
  Search,
  Close,
  Grid,
  Promotion,
  DataLine,
  Connection,
  Key
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus/es/components/message/index'
import BluePrism from '../../components/common/BluePrism.vue'
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
    .slice(0, 3)
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
  code.value.split('\n').map((line) =>
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
  <div id="home-top" class="aurora-home">
    <PublicHeader header-class="aurora-public-header" />
    <main>
      <section class="ah-hero">
        <div class="ah-hero-waves" aria-hidden="true"><i></i><i></i><i></i></div>
        <div class="ah-container ah-hero-grid">
          <div class="ah-hero-copy">
            <span class="ah-eyebrow"
              ><span></span>{{ s('为想法搭一座桥', 'A bridge for your next idea') }}<ArrowRight
            /></span>
            <h1>
              {{ s('好模型，轻松用。', 'Great models. Made simple.') }}<br /><em>{{
                s('让创意即刻发生。', 'Bring your ideas to life.')
              }}</em>
            </h1>
            <p>
              {{
                s(
                  '从第一行代码，到下一款产品。智桥将模型接入、在线调试与用量管理，放进一个简单好用的工作台。',
                  'From your first line of code to your next product. Discover models, test your API and manage usage in one thoughtful workspace.'
                )
              }}
            </p>
            <div class="ah-actions">
              <RouterLink class="ah-button" :to="consolePath"
                >{{
                  auth.isAuthed ? s('进入工作台', 'Open workspace') : s('立即开始', 'Get started')
                }}<ArrowRight
              /></RouterLink>
              <RouterLink class="ah-button ah-button-white" to="/models"
                >{{ s('探索模型', 'Explore models') }}<Grid
              /></RouterLink>
            </div>
            <div class="ah-hero-note">
              <Check />{{ s('兼容 OpenAI SDK', 'OpenAI SDK compatible') }}<i></i><Check />{{
                s('按实际用量计费', 'Usage-based billing')
              }}
            </div>
          </div>
          <div class="ah-hero-art" aria-hidden="true">
            <div class="ah-art-halo"></div>
            <BluePrism />
            <div class="ah-floating ah-float-one">
              <span class="ah-float-icon"><Connection /></span>
              <div>
                <strong>{{ s('一个 API，多种可能', 'One API. More possibilities.') }}</strong
                ><small>OPENAI COMPATIBLE</small>
              </div>
              <span class="ah-float-dot"></span>
            </div>
            <div class="ah-floating ah-float-two">
              <span class="ah-float-icon violet"><Promotion /></span>
              <div>
                <strong>API Playground</strong
                ><small>{{ s('想法，从一次对话开始', 'Your next idea starts here') }}</small>
              </div>
            </div>
            <span class="ah-art-caption">ZHIQIAO · CONNECT TO CREATE</span>
          </div>
        </div>
        <div class="ah-container ah-foundations">
          <div>
            <span class="ah-foundation-icon"><Connection /></span>
            <p>
              <strong>{{ s('接入更轻松', 'A familiar API') }}</strong
              ><small>{{ s('沿用你熟悉的 SDK', 'Keep the SDK you know') }}</small>
            </p>
          </div>
          <div>
            <span class="ah-foundation-icon violet"><Grid /></span>
            <p>
              <strong>{{ s('选择更清楚', 'Find your model') }}</strong
              ><small>{{ s('能力、协议、价格一起看', 'Compare capabilities and pricing') }}</small>
            </p>
          </div>
          <div>
            <span class="ah-foundation-icon mint"><DataLine /></span>
            <p>
              <strong>{{ s('每次调用都有迹可循', 'Know your usage') }}</strong
              ><small>{{ s('用量趋势与请求明细', 'Trends and request-level details') }}</small>
            </p>
          </div>
          <RouterLink to="/status"
            >{{ s('查看服务状态', 'Service status') }}<ArrowRight
          /></RouterLink>
        </div>
      </section>
      <section class="ah-container ah-model-section">
        <header class="ah-section-heading">
          <div>
            <span class="ah-section-kicker">MODEL EXPLORER</span>
            <h2>{{ s('找到与你的想法合拍的模型', 'The right model for your next idea') }}</h2>
            <p>
              {{
                s(
                  '从平台当前上架的模型开始，比较价格，再放心接入。',
                  'Explore currently listed models. Compare the offers, then start building.'
                )
              }}
            </p>
          </div>
          <RouterLink to="/models" class="ah-more"
            >{{ s('查看全部模型', 'All models') }}<ArrowRight
          /></RouterLink>
        </header>
        <div class="ah-model-toolbar">
          <span class="ah-model-count"
            ><i></i>{{ loading || error ? '—' : models.length }}
            {{ s('个模型已上架', 'models listed') }}<b>{{ currency }}</b></span
          >
          <label class="ah-model-search"
            ><Search /><input
              v-model="query"
              type="search"
              :aria-label="s('搜索首页模型', 'Search homepage models')"
              :placeholder="s('搜索你想用的模型', 'Find a model')" /><button
              v-if="query"
              :aria-label="s('清空模型搜索', 'Clear search')"
              @click="query = ''"
            >
              <Close /></button
          ></label>
        </div>
        <div v-if="loading" class="ah-empty">
          {{ s('正在获取模型报价…', 'Loading current offers…') }}
        </div>
        <div v-else-if="error" class="ah-empty" role="alert">
          {{ s('模型目录暂时无法加载', 'Catalog unavailable')
          }}<button @click="reload()">{{ s('重新加载', 'Retry') }}</button>
        </div>
        <div v-else-if="!visibleModels.length" class="ah-empty">
          {{ s('没有找到匹配的模型', 'No matching models')
          }}<button v-if="query" @click="query = ''">{{ s('清空搜索', 'Clear search') }}</button>
        </div>
        <div v-else class="ah-model-cards">
          <RouterLink
            v-for="model in visibleModels"
            :key="model.key"
            class="ah-model-card"
            :to="{ path: '/models', query: { q: model.model } }"
          >
            <div class="ah-model-card-top">
              <ProviderIcon :provider="model.provider" /><span>{{
                model.protocols.slice(0, 2).join(' · ')
              }}</span
              ><ArrowRight />
            </div>
            <h3>{{ model.display_name }}</h3>
            <p>{{ model.provider_name }}</p>
            <div class="ah-model-prices">
              <div>
                <span>{{ s('输入价格', 'Input') }}</span
                ><strong>{{ price(model, 'input') }}</strong>
              </div>
              <div v-if="model.billing_meter === 'token'">
                <span>{{ s('输出价格', 'Output') }}</span
                ><strong>{{ price(model, 'output') }}</strong>
              </div>
            </div>
            <footer>
              <span>{{
                model.billing_meter === 'token'
                  ? s('每 100 万 Tokens', 'Per 1M tokens')
                  : s('按模型计费单位', 'Per billing unit')
              }}</span
              ><b>{{ s('查看详情', 'View model') }} ↗</b>
            </footer>
          </RouterLink>
        </div>
        <p class="ah-price-note">
          {{
            s(
              '以上为当前目录报价预览，实际费用以模型详情与账户结算规则为准。',
              'Catalog preview. Actual charges follow the model offer and your account billing rules.'
            )
          }}
        </p>
      </section>
      <section class="ah-workflow-section">
        <div class="ah-container">
          <header class="ah-section-heading">
            <div>
              <span class="ah-section-kicker">YOUR AI WORKSPACE</span>
              <h2>
                {{ s('接入、调试、管理，顺手完成', 'One workspace. A better way to build.') }}
              </h2>
              <p>
                {{
                  s(
                    '不在工具之间来回切换，让开发的每一步都连贯。',
                    'Stay in your flow, from choosing a model to understanding every request.'
                  )
                }}
              </p>
            </div>
          </header>
          <div class="ah-workflow-grid">
            <RouterLink :to="consolePath" class="ah-workspace-feature">
              <div>
                <span class="ah-feature-label"
                  ><DataLine />{{ s('用量管理', 'USAGE INSIGHTS') }}</span
                >
                <h3>
                  {{ s('调用了什么，花了多少，', 'Every request.') }}<br />{{
                    s('一眼就清楚。', 'Every detail, in view.')
                  }}
                </h3>
                <p>
                  {{
                    s(
                      '账户概况、消费趋势、请求记录，回到工作台就能看到。',
                      'Credit, spend trends and request logs are always within reach.'
                    )
                  }}
                </p>
                <span class="ah-more">{{ s('打开工作台', 'Open workspace') }}<ArrowRight /></span>
              </div>
              <div class="ah-feature-visual" aria-hidden="true">
                <div class="ah-mini-heading">
                  <span>{{ s('用量洞察', 'USAGE INSIGHTS') }}</span
                  ><span>↗</span>
                </div>
                <div class="ah-mini-values">
                  <span
                    >{{ s('请求明细', 'Requests') }}<b>01 <small>→</small></b></span
                  ><span
                    >{{ s('费用分析', 'Spend') }}<b>02 <small>→</small></b></span
                  >
                </div>
                <div class="ah-mini-bars">
                  <i
                    v-for="(height, index) in [28, 42, 34, 61, 48, 67, 55, 78, 62, 88, 74, 100]"
                    :key="index"
                    :style="{ height: height + '%' }"
                  ></i>
                </div>
                <small class="ah-illustration-label">{{
                  s('功能示意 · 非账户数据', 'Feature illustration · not account data')
                }}</small>
              </div>
            </RouterLink>
            <div class="ah-small-features">
              <RouterLink to="/playground"
                ><span class="ah-feature-symbol violet"><Promotion /></span>
                <div>
                  <h3>{{ s('先试一试，再接入', 'Try it before you build') }}</h3>
                  <p>
                    {{
                      s(
                        '流式响应、耗时和用量，在调试台直接验证。',
                        'Inspect streaming responses, timing and usage in Playground.'
                      )
                    }}
                  </p>
                </div>
                <ArrowRight
              /></RouterLink>
              <RouterLink
                :to="auth.isAdmin ? '/admin/channels' : auth.isUser ? '/home/apikeys' : '/register'"
                ><span class="ah-feature-symbol mint"><Key /></span>
                <div>
                  <h3>{{ s('连接，由你掌控', 'Your connection. Your control.') }}</h3>
                  <p>
                    {{
                      s(
                        '在控制台管理调用凭证与模型接入。',
                        'Manage your credentials and model connections from the console.'
                      )
                    }}
                  </p>
                </div>
                <ArrowRight
              /></RouterLink>
            </div>
          </div>
        </div>
      </section>
      <section class="ah-container ah-integration-section">
        <div class="ah-integration-copy">
          <span class="ah-section-kicker">BUILD WITH WHAT YOU KNOW</span>
          <h2>
            {{ s('熟悉的代码，', 'The code you know.') }}<br /><em>{{
              s('新的可能。', 'New possibilities.')
            }}</em>
          </h2>
          <p>
            {{
              s(
                '替换接入地址和 API Key，选择已上架的模型，即可沿用 OpenAI SDK 开始调用。',
                'Set the endpoint and your API key, pick a listed model, and keep building with the OpenAI SDK.'
              )
            }}
          </p>
          <RouterLink to="/docs" class="ah-more"
            >{{ s('阅读接入文档', 'Read the docs') }}<ArrowRight
          /></RouterLink>
        </div>
        <div class="ah-code-editor">
          <div class="ah-code-toolbar">
            <div class="ah-code-tabs">
              <button
                v-for="item in languages"
                :key="item.id"
                :class="{ active: language === item.id }"
                :aria-pressed="language === item.id"
                @click="language = item.id"
              >
                {{ item.name }}
              </button>
            </div>
            <button
              class="ah-copy-code"
              :aria-label="s('复制接入代码', 'Copy integration code')"
              @click="copyCode"
            >
              <Check v-if="copied" /><CopyDocument v-else />
            </button>
          </div>
          <pre
            class="ah-source"
          ><code><span v-for="(line, index) in codeLines" :key="index"><span v-for="(token, i) in line" :key="i" :class="token.kind ? 'ah-token-' + token.kind : undefined">{{ token.text }}</span>{{ '\n' }}</span></code></pre>
          <label class="ah-example-model"
            >{{ s('示例模型', 'Example model')
            }}<select v-model="selectedModel" :disabled="!chatModels.length">
              <option v-if="!chatModels.length" value="">YOUR_MODEL_ID</option>
              <option v-for="model in chatModels" :key="model.key" :value="model.model">
                {{ model.model }}
              </option>
            </select></label
          >
        </div>
      </section>
      <section class="ah-container ah-faq-section">
        <header>
          <span class="ah-section-kicker">GOOD TO KNOW</span>
          <h2>{{ s('你可能还想了解', 'A few things to know') }}</h2>
        </header>
        <div class="ah-faq-list">
          <details v-for="item in questions" :key="item.q">
            <summary>{{ item.q }}<span>+</span></summary>
            <p>{{ item.a }}</p>
          </details>
        </div>
      </section>
      <section class="ah-start">
        <div class="ah-container">
          <div>
            <span class="ah-section-kicker">YOUR NEXT IDEA</span>
            <h2>{{ s('下一次创造，从智桥开始。', 'Your next idea starts with Zhiqiao.') }}</h2>
            <p>
              {{
                s(
                  '让接入更简单，让想法走得更远。',
                  'A simpler connection. More room for your ideas.'
                )
              }}
            </p>
          </div>
          <RouterLink class="ah-button" :to="consolePath"
            >{{ s('开始构建', 'Start building') }}<ArrowRight
          /></RouterLink>
        </div>
        <span class="ah-start-bridge" aria-hidden="true"></span>
      </section>
    </main>
    <footer class="ah-footer ah-container">
      <div class="ah-footer-brand">
        <BrandLogo /><span>{{
          s('连接模型，也连接可能。', 'Connecting models. Connecting possibilities.')
        }}</span>
      </div>
      <nav>
        <RouterLink to="/models">{{ s('模型广场', 'Models') }}</RouterLink
        ><RouterLink to="/docs">{{ s('接入文档', 'Docs') }}</RouterLink
        ><RouterLink to="/status">{{ s('服务状态', 'Status') }}</RouterLink
        ><RouterLink to="/about">{{ s('关于我们', 'About') }}</RouterLink>
      </nav>
      <div class="ah-footer-bottom">
        <span>© {{ new Date().getFullYear() }} {{ siteName }}</span
        ><span>CONNECT TO CREATE.</span>
      </div>
    </footer>
  </div>
</template>
<style src="../../styles/aurora.css"></style>
