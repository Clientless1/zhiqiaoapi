<script setup lang="ts">
import { computed, ref, onBeforeUnmount, watch, nextTick } from 'vue'
import { useRoute } from 'vue-router'
import {
  Promotion,
  CopyDocument,
  VideoPause,
  Delete,
  Setting,
  ChatDotRound
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus/es/components/message/index'
import PublicHeader from '../../components/common/PublicHeader.vue'
import { getModelCatalog } from '../../api/catalog'
import { isChatModel } from '../../utils/catalog'
import { useAsyncData } from '../../composables/useAsyncData'
import { useStudio } from '../../composables/useStudio'
import { createEventStreamParser } from '../../utils/eventStream'
import {
  buildPlaygroundPayload,
  preferredPlaygroundProtocol,
  readPlaygroundEvent,
  type PlaygroundProtocol
} from '../../utils/playground'
const route = useRoute(),
  { s } = useStudio()
const isPublic = computed(() => route.path === '/playground')
const { data: catalog, error: catalogError } = useAsyncData(
  () => getModelCatalog(route.path.startsWith('/home/')),
  null
)
const models = computed(() => [
  ...new Set(
    (catalog.value?.models ?? [])
      .filter(
        (m) => m.protocols.includes('openai') && isChatModel(m.model, catalog.value?.models ?? [])
      )
      .map((m) => m.model)
  )
])
const apiKey = ref(''),
  model = ref(String(route.query.model ?? '')),
  system = ref(''),
  prompt = ref(''),
  temperature = ref(0.7),
  maxTokens = ref(2048)
const temperatureEnabled = ref(false),
  chatHost = ref<HTMLElement>()
const protocol = ref<'auto' | PlaygroundProtocol>('auto')
const activeProtocol = computed(() =>
  protocol.value === 'auto' ? preferredPlaygroundProtocol(model.value) : protocol.value
)
const requestPath = computed(() =>
  activeProtocol.value === 'responses' ? '/v1/responses' : '/v1/chat/completions'
)
const messages = ref<{ role: 'user' | 'assistant'; content: string }[]>([]),
  running = ref(false),
  error = ref(''),
  codeOpen = ref(false)
watch(
  () => messages.value.map((m) => m.content).join(''),
  async () => {
    await nextTick()
    if (chatHost.value) chatHost.value.scrollTop = chatHost.value.scrollHeight
  }
)
const elapsed = ref(0),
  firstToken = ref<number | null>(null),
  usage = ref<{ prompt_tokens?: number; completion_tokens?: number; total_tokens?: number } | null>(
    null
  )
let controller: AbortController | undefined,
  disposed = false
const code = computed(
  () => `curl '${window.location.origin}${requestPath.value}' \\
  -H 'Authorization: Bearer YOUR_API_KEY' \\
  -H 'Content-Type: application/json' \\
  --data '${JSON.stringify(payload(false), null, 2).replace(/'/g, "'\\''")}'`
)
function payload(withHistory = true) {
  const history = withHistory
    ? messages.value.filter((m) => m.content).map((m) => ({ role: m.role, content: m.content }))
    : [
        {
          role: 'user' as const,
          content: prompt.value || s('你好，请介绍一下自己。', 'Hello, introduce yourself.')
        }
      ]
  return buildPlaygroundPayload({
    protocol: activeProtocol.value,
    model: model.value,
    messages: history,
    system: system.value,
    temperature: temperatureEnabled.value ? temperature.value : undefined,
    maxTokens: maxTokens.value
  })
}
async function copyCode() {
  try {
    await navigator.clipboard.writeText(code.value)
    ElMessage.success(s('请求代码已复制', 'Request copied'))
  } catch {
    ElMessage.error(s('复制失败，请手动复制', 'Please copy manually'))
  }
}
async function send() {
  if (running.value) return
  if (!apiKey.value.trim() || !model.value.trim() || !prompt.value.trim()) {
    error.value = s('请填写 API Key、模型和消息。', 'Enter an API key, model and message.')
    return
  }
  error.value = ''
  usage.value = null
  elapsed.value = 0
  firstToken.value = null
  messages.value.push({ role: 'user', content: prompt.value.trim() })
  prompt.value = ''
  const body = payload()
  const assistant = { role: 'assistant' as const, content: '' }
  messages.value.push(assistant)
  const index = messages.value.length - 1
  running.value = true
  controller = new AbortController()
  const start = performance.now()
  let done = false,
    finished = false
  try {
    const response = await fetch(requestPath.value, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${apiKey.value.trim()}`
      },
      body: JSON.stringify(body),
      signal: controller.signal
    })
    if (!response.ok) {
      const text = await response.text()
      let message = `HTTP ${response.status}`
      try {
        const json = JSON.parse(text)
        message = json.error?.message || json.message || message
      } catch {
        /* Keep HTTP error when response is not JSON. */
      }
      throw new Error(message)
    }
    function consume(raw: string) {
      if (done) return
      if (raw === '[DONE]') {
        done = true
        return
      }
      const chunk = JSON.parse(raw)
      const update = readPlaygroundEvent(chunk)
      if (update.error) throw new Error(update.error)
      if (update.usage) usage.value = update.usage
      if (update.done) finished = true
      if (chunk.type === 'response.completed' || chunk.type === 'response.incomplete') done = true
      const content = update.delta
      if (typeof content === 'string' && content) {
        if (firstToken.value === null) firstToken.value = Math.round(performance.now() - start)
        if (messages.value[index]) messages.value[index].content += content
      }
      if (update.fullText && messages.value[index] && !messages.value[index].content) {
        if (firstToken.value === null) firstToken.value = Math.round(performance.now() - start)
        messages.value[index].content = update.fullText
      }
      if (update.incomplete)
        error.value =
          update.incomplete === 'max_output_tokens'
            ? s(
                '已达到最大输出 tokens，可调高后重试。',
                'The output token limit was reached. Increase it and try again.'
              )
            : s(`响应未完成：${update.incomplete}`, `Response incomplete: ${update.incomplete}`)
    }
    if (response.headers.get('content-type')?.includes('text/event-stream')) {
      if (!response.body) throw new Error(s('响应流为空', 'No response stream'))
      const reader = response.body.getReader(),
        decoder = new TextDecoder(),
        parser = createEventStreamParser(consume)
      try {
        while (!done) {
          const part = await reader.read()
          if (part.done) break
          parser.push(decoder.decode(part.value, { stream: true }))
        }
        parser.push(decoder.decode())
        parser.finish()
      } finally {
        await reader.cancel().catch(() => {})
        reader.releaseLock()
      }
      if (!done && !finished)
        throw new Error(
          s(
            '响应流提前中断，可检查上游连接后重试。',
            'The response stream ended early. Check the upstream connection and retry.'
          )
        )
    } else {
      const json = await response.json()
      consume(JSON.stringify(json))
    }
    if (!messages.value[index]?.content)
      throw new Error(
        s(
          '模型未返回文本，请检查模型能力或请求参数。',
          'The model returned no text. Check model capabilities and parameters.'
        )
      )
  } catch (err) {
    if (!disposed)
      error.value =
        err instanceof Error && err.name === 'AbortError'
          ? s('已停止生成。', 'Generation stopped.')
          : err instanceof Error
            ? err.message
            : s('请求失败', 'Request failed')
  } finally {
    if (!disposed) {
      elapsed.value = Math.round(performance.now() - start)
      running.value = false
    }
    controller = undefined
  }
}
function clear() {
  messages.value = []
  error.value = ''
  usage.value = null
  elapsed.value = 0
  firstToken.value = null
}
onBeforeUnmount(() => {
  disposed = true
  controller?.abort()
  apiKey.value = ''
})
</script>
<template>
  <div :class="{ 'studio-public': isPublic }">
    <PublicHeader v-if="isPublic" />
    <main class="playground-page" :class="{ 'studio-public-main': isPublic }">
      <header v-if="isPublic" class="studio-section-title">
        <div>
          <span class="kicker">API PLAYGROUND</span>
          <h1>{{ s('灵感，在这里开始运行', 'From idea to first response.') }}</h1>
          <p>
            {{
              s('连接真实模型，查看真实响应。', 'Connect to real models. Inspect real responses.')
            }}
          </p>
        </div>
        <RouterLink to="/docs">{{ s('阅读文档', 'Read the docs') }} ↗</RouterLink>
      </header>
      <div class="playground-layout">
        <aside class="studio-panel playground-config">
          <div class="panel-heading">
            <h3>
              <el-icon><Setting /></el-icon> {{ s('请求配置', 'Configuration') }}
            </h3>
            <span class="studio-pill">{{
              activeProtocol === 'responses' ? 'Responses' : 'Chat Completions'
            }}</span>
          </div>
          <label for="pg-key">API Key</label
          ><el-input
            id="pg-key"
            v-model="apiKey"
            type="password"
            show-password
            placeholder="sk-…"
            autocomplete="off"
            :disabled="running"
          />
          <p class="field-note">
            {{
              s(
                '使用平台 API Key，仅保留在当前页面内存。真实请求将按账户规则计费。',
                'Use a platform API key. It stays in page memory. Real requests are billed to your account.'
              )
            }}
          </p>
          <label>{{ s('接口模式', 'API mode') }}</label>
          <el-select v-model="protocol" :disabled="running" :aria-label="s('接口模式', 'API mode')">
            <el-option value="auto" :label="s('自动选择（推荐）', 'Automatic (recommended)')" />
            <el-option value="responses" label="Responses / Codex" />
            <el-option value="chat" label="Chat Completions" />
          </el-select>
          <p class="field-note">
            {{
              s(
                'Codex 上游请使用 Responses；其他接口可手动切换。',
                'Use Responses for Codex upstreams. Switch manually for other endpoints.'
              )
            }}
          </p>
          <label>{{ s('模型', 'Model') }}</label
          ><el-select
            v-model="model"
            filterable
            allow-create
            default-first-option
            :placeholder="s('选择或输入模型 ID', 'Choose or enter a model ID')"
            :disabled="running"
            ><el-option v-for="m in models" :key="m" :label="m" :value="m"
          /></el-select>
          <p v-if="catalogError" class="field-note">
            {{
              s(
                '目录加载失败，可手动输入模型 ID。',
                'Catalog unavailable; enter a model ID manually.'
              )
            }}
          </p>
          <label>{{ s('系统提示词（选填）', 'System prompt (optional)') }}</label
          ><el-input
            v-model="system"
            type="textarea"
            :rows="4"
            :disabled="running"
            :placeholder="s('为模型设置角色或回答方式…', 'Set a role or response style…')"
          />
          <label
            ><el-checkbox v-model="temperatureEnabled" :disabled="running">Temperature</el-checkbox
            ><span>{{ temperatureEnabled ? temperature : s('默认', 'Default') }}</span></label
          ><el-slider
            v-model="temperature"
            :min="0"
            :max="2"
            :step="0.1"
            :disabled="running || !temperatureEnabled"
          />
          <label>{{ s('最大输出 tokens', 'Maximum output tokens') }}</label
          ><el-input-number
            v-model="maxTokens"
            :min="activeProtocol === 'responses' ? 16 : 1"
            :max="32768"
            :step="256"
            :disabled="running"
          />
          <p class="field-note">
            {{
              s(
                'Temperature 默认不发送；最大输出参数会按接口模式自动匹配。',
                'Temperature is omitted by default. The output limit matches the selected API mode.'
              )
            }}
          </p>
          <el-button :icon="CopyDocument" @click="codeOpen = true">{{
            s('查看请求代码', 'View request code')
          }}</el-button
          ><RouterLink class="text-button" to="/models"
            >{{ s('查找与对比模型', 'Explore & compare models') }} ↗</RouterLink
          >
        </aside>
        <section class="studio-panel playground-chat">
          <div class="panel-heading">
            <div>
              <h3>{{ s('对话预览', 'Conversation') }}</h3>
              <span class="muted">{{ model || s('尚未选择模型', 'Select a model') }}</span>
            </div>
            <el-button text :icon="Delete" :disabled="running || !messages.length" @click="clear">{{
              s('清空', 'Clear')
            }}</el-button>
          </div>
          <div ref="chatHost" class="chat-messages" aria-live="polite" :aria-busy="running">
            <div v-if="!messages.length" class="chat-empty">
              <span class="chat-empty-icon"
                ><el-icon><ChatDotRound /></el-icon
              ></span>
              <h2>{{ s('从一个问题开始', 'Start with a question.') }}</h2>
              <p>
                {{
                  s(
                    '选择模型、填入 Key，然后发送第一条消息。',
                    'Choose a model, enter your key, and send a message.'
                  )
                }}
              </p>
              <div class="prompt-suggestions">
                <button
                  v-for="p in [
                    s('用三句话介绍你自己', 'Introduce yourself in three sentences'),
                    s('写一个 Python 快速排序函数', 'Write a Python quicksort function')
                  ]"
                  :key="p"
                  @click="prompt = p"
                >
                  {{ p }} ↗
                </button>
              </div>
            </div>
            <article
              v-for="(message, index) in messages"
              :key="index"
              :class="['chat-message', message.role]"
            >
              <span class="message-avatar">{{ message.role === 'user' ? 'U' : 'AI' }}</span>
              <div>
                <strong>{{ message.role === 'user' ? s('你', 'You') : model }}</strong>
                <p>
                  {{
                    message.content ||
                    (running ? s('正在思考…', 'Thinking…') : s('未返回文本', 'No text returned'))
                  }}
                </p>
              </div>
            </article>
          </div>
          <el-alert v-if="error" :title="error" type="warning" :closable="false" show-icon />
          <div class="response-metadata">
            <span>STREAM</span
            ><span v-if="firstToken !== null"
              >{{ s('首字', 'First token') }} {{ firstToken }} ms</span
            ><span v-if="elapsed"
              >{{ s('总耗时', 'Total') }} {{ (elapsed / 1000).toFixed(2) }} s</span
            ><span v-if="usage?.total_tokens != null">{{ usage.total_tokens }} tokens</span
            ><span v-else>{{
              s('Token 用量以接口返回为准', 'Token usage reported when provided')
            }}</span>
          </div>
          <form class="chat-composer" @submit.prevent="send">
            <el-input
              v-model="prompt"
              type="textarea"
              :autosize="{ minRows: 2, maxRows: 6 }"
              :placeholder="
                s(
                  '输入消息…（Ctrl / ⌘ + Enter 发送）',
                  'Type a message… (Ctrl / ⌘ + Enter to send)'
                )
              "
              :disabled="running"
              :aria-label="s('消息', 'Message')"
              @keydown.ctrl.enter.prevent="send"
              @keydown.meta.enter.prevent="send"
            />
            <div>
              <span>{{
                s('纯文本预览 · 不执行模型返回代码', 'Plain text · returned code is never executed')
              }}</span
              ><el-button v-if="running" :icon="VideoPause" @click="controller?.abort()">{{
                s('停止生成', 'Stop')
              }}</el-button
              ><el-button v-else type="primary" native-type="submit" :icon="Promotion">{{
                s('发送消息', 'Send')
              }}</el-button>
            </div>
          </form>
        </section>
      </div>
      <el-dialog v-model="codeOpen" :title="s('请求代码', 'Request code')" width="min(740px,94vw)"
        ><p class="studio-note">
          {{
            s(
              'YOUR_API_KEY 为占位符，请在自己的终端中替换。',
              'Replace YOUR_API_KEY in your own terminal.'
            )
          }}
        </p>
        <pre class="request-code">{{ code }}</pre>
        <template #footer
          ><el-button type="primary" :icon="CopyDocument" @click="copyCode">{{
            s('复制代码', 'Copy code')
          }}</el-button></template
        ></el-dialog
      >
    </main>
  </div>
</template>
