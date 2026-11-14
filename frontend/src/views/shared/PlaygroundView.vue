<script setup lang="ts">
import { computed, ref, onBeforeUnmount, watch, nextTick } from 'vue'
import { useRoute } from 'vue-router'
import {
  Promotion,
  CopyDocument,
  VideoPause,
  Delete,
  Setting,
  ChatDotRound,
  ArrowRight,
  QuestionFilled
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus/es/components/message/index'
import PublicHeader from '../../components/common/PublicHeader.vue'
import { getModelCatalog } from '../../api/catalog'
import { isChatModel, modelCategory } from '../../utils/catalog'
import { copyTextToClipboard } from '../../utils/clipboard'
import { useAsyncData } from '../../composables/useAsyncData'
import { useSiteBrand } from '../../composables/useSiteBrand'
import { useStudio } from '../../composables/useStudio'
import { createEventStreamParser } from '../../utils/eventStream'
import {
  buildPlaygroundPayload,
  parsePlaygroundJson,
  playgroundJsonPaths,
  preferredPlaygroundProtocol,
  readPlaygroundEvent,
  type PlaygroundProtocol
} from '../../utils/playground'
const route = useRoute(),
  { s } = useStudio()
const { apiOrigin } = useSiteBrand()
const isPublic = computed(() => route.path === '/playground')
const { data: catalog, error: catalogError } = useAsyncData(
  () => getModelCatalog(route.path.startsWith('/home/')),
  null
)
const keyModels = ref<string[]>([])
const keyLoading = ref(false)
const keyError = ref('')
const verifiedKey = ref('')
const models = computed(() => keyModels.value)
const requestMode = ref<'conversation' | 'json'>('conversation')
const rawPath = ref('/v1/embeddings')
const rawBody = ref('{\n  "input": "Hello"\n}')
const rawResult = ref('')
const rawStatus = ref<number | null>(null)
const apiPaths: readonly string[] = playgroundJsonPaths
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
  requestMode.value === 'json'
    ? rawPath.value
    : activeProtocol.value === 'anthropic'
      ? '/v1/messages'
      : activeProtocol.value === 'responses'
        ? '/v1/responses'
        : '/v1/chat/completions'
)
const validating = ref(false)
watch(model, (value) => {
  if (!value) return
  const offer = catalog.value?.models.find((item) => item.model === value)
  const category = offer
    ? modelCategory(offer)
    : /embed|bge-|e5-|gte-/i.test(value)
      ? 'embedding'
      : /moderation/i.test(value)
        ? 'moderation'
        : null
  if (isChatModel(value, catalog.value?.models ?? [])) {
    if (requestMode.value === 'json') setChatJsonTemplate()
    return
  }
  requestMode.value = 'json'
  const defaults: Record<string, [string, Record<string, unknown>]> = {
    embedding: ['/v1/embeddings', { input: 'Hello' }],
    moderation: ['/v1/moderations', { input: 'Hello' }],
    image: ['/v1/images/generations', { prompt: 'A quiet mountain lake', n: 1 }],
    video: ['/v1/videos', { prompt: 'A quiet mountain lake' }],
    audio: ['/v1/audio/speech', { input: 'Hello', voice: 'alloy' }]
  }
  const preset = category ? defaults[category] : undefined
  if (preset) {
    rawPath.value = preset[0]
    rawBody.value = JSON.stringify(preset[1], null, 2)
  }
})
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
  () => `curl '${apiOrigin.value}${requestPath.value}' \\
  -H 'Authorization: Bearer YOUR_API_KEY' \\
${(requestMode.value === 'json' ? rawPath.value === '/v1/messages' : activeProtocol.value === 'anthropic') ? "  -H 'anthropic-version: 2023-06-01' \\\n" : ''}  -H 'Content-Type: application/json' \\
  --data '${JSON.stringify(examplePayload(), null, 2).replace(/'/g, "'\\''")}'`
)
function examplePayload() {
  try {
    return payload(false)
  } catch {
    return { model: model.value }
  }
}
function payload(withHistory = true) {
  if (requestMode.value === 'json') return parsePlaygroundJson(rawBody.value, model.value)
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
let modelController: AbortController | undefined
let keyTimer: ReturnType<typeof setTimeout> | undefined
watch(apiKey, () => {
  modelController?.abort()
  verifiedKey.value = ''
  keyModels.value = []
  keyError.value = ''
  keyLoading.value = false
  clearTimeout(keyTimer)
  if (apiKey.value.trim()) keyTimer = setTimeout(loadKeyModels, 600)
})
async function loadKeyModels() {
  clearTimeout(keyTimer)
  modelController?.abort()
  const key = apiKey.value.trim()
  if (!key) return
  const current = new AbortController()
  modelController = current
  keyLoading.value = true
  keyError.value = ''
  try {
    const response = await fetch('/v1/models', {
      headers: { Authorization: 'Bearer ' + key },
      signal: current.signal,
      cache: 'no-store'
    })
    const result = await response.json()
    if (!response.ok)
      throw new Error(result.error?.message || result.message || 'HTTP ' + response.status)
    if (current.signal.aborted || key !== apiKey.value.trim()) return
    const ids: string[] = (result.data ?? [])
      .map((item: { id?: string }) => item.id)
      .filter((id: unknown): id is string => typeof id === 'string')
    keyModels.value = [...new Set(ids)]
    verifiedKey.value = key
    if (!keyModels.value.includes(model.value)) model.value = keyModels.value[0] ?? ''
    if (!keyModels.value.length)
      keyError.value = s(
        '此 Key 暂无可用模型，请检查模型权限或套餐。',
        'No models are available to this key. Check its permissions and plan.'
      )
  } catch (err) {
    if (!current.signal.aborted && key === apiKey.value.trim())
      keyError.value =
        err instanceof Error ? err.message : s('读取模型失败', 'Could not load models')
  } finally {
    if (modelController === current) keyLoading.value = false
  }
}
async function copyCode() {
  try {
    await copyTextToClipboard(code.value)
    ElMessage.success(s('请求代码已复制', 'Request copied'))
  } catch {
    ElMessage.error(s('复制失败，请手动复制', 'Please copy manually'))
  }
}
async function send() {
  if (running.value || keyLoading.value || validating.value) return
  if (
    !apiKey.value.trim() ||
    !model.value.trim() ||
    (requestMode.value === 'conversation' && !prompt.value.trim())
  ) {
    error.value = s('请填写 API Key、模型和消息。', 'Enter an API key, model and message.')
    return
  }
  validating.value = true
  try {
    if (verifiedKey.value !== apiKey.value.trim()) await loadKeyModels()
  } finally {
    validating.value = false
  }
  if (disposed) return
  if (verifiedKey.value !== apiKey.value.trim() || !keyModels.value.includes(model.value)) {
    error.value =
      keyError.value || s('请选择此 Key 有权限的模型。', 'Choose a model available to this key.')
    return
  }
  if (requestMode.value === 'json') {
    await sendJsonRequest()
    return
  }
  if (!isChatModel(model.value, catalog.value?.models ?? [])) {
    error.value = s(
      '此模型不是对话模型，请切换到 JSON 请求。',
      'Switch to JSON request for this model.'
    )
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
        Authorization: `Bearer ${apiKey.value.trim()}`,
        ...(activeProtocol.value === 'anthropic' ? { 'anthropic-version': '2023-06-01' } : {})
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
      if (update.usage)
        usage.value = {
          ...usage.value,
          ...Object.fromEntries(Object.entries(update.usage).filter(([, value]) => value != null))
        }
      if (update.done) finished = true
      if (
        chunk.type === 'response.completed' ||
        chunk.type === 'response.incomplete' ||
        chunk.type === 'message_stop'
      )
        done = true
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
async function sendJsonRequest() {
  if (!apiPaths.includes(rawPath.value)) return
  let body: Record<string, unknown>
  try {
    body = parsePlaygroundJson(rawBody.value, model.value)
  } catch {
    error.value = s('请求体必须是有效的 JSON 对象。', 'Enter a valid JSON object.')
    return
  }
  error.value = ''
  rawResult.value = ''
  rawStatus.value = null
  running.value = true
  controller = new AbortController()
  const start = performance.now()
  try {
    const response = await fetch(rawPath.value, {
      method: 'POST',
      signal: controller.signal,
      headers: {
        'Content-Type': 'application/json',
        Authorization: 'Bearer ' + apiKey.value.trim(),
        ...(rawPath.value === '/v1/messages' ? { 'anthropic-version': '2023-06-01' } : {})
      },
      body: JSON.stringify(body)
    })
    rawStatus.value = response.status
    if (response.headers.get('content-type')?.startsWith('audio/')) {
      await response.body?.cancel()
      rawResult.value = s(
        '音频响应已成功返回。完整请求可复制到终端下载。',
        'Audio returned successfully. Copy the request to download the response in your terminal.'
      )
    } else {
      const reader = response.body?.getReader()
      const decoder = new TextDecoder()
      let text = ''
      if (reader) {
        try {
          while (true) {
            const part = await reader.read()
            if (part.done) break
            text += decoder.decode(part.value, { stream: true })
            if (text.length > 1_000_000) {
              text += '\n[Response truncated at 1 MB]'
              break
            }
          }
          text += decoder.decode()
        } finally {
          await reader.cancel().catch(() => {})
          reader.releaseLock()
        }
      }
      try {
        rawResult.value = JSON.stringify(JSON.parse(text), null, 2)
      } catch {
        rawResult.value = text
      }
    }
    if (!response.ok) error.value = 'HTTP ' + response.status
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Request failed'
  } finally {
    running.value = false
    elapsed.value = Math.round(performance.now() - start)
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
function setChatJsonTemplate() {
  const selected = preferredPlaygroundProtocol(model.value)
  rawPath.value =
    selected === 'anthropic'
      ? '/v1/messages'
      : selected === 'responses'
        ? '/v1/responses'
        : '/v1/chat/completions'
  const body = buildPlaygroundPayload({
    protocol: selected,
    model: model.value,
    messages: [{ role: 'user', content: 'Hello' }],
    system: '',
    maxTokens: 256
  })
  rawBody.value = JSON.stringify({ ...body, stream: false }, null, 2)
}
watch(requestMode, (value) => {
  if (value === 'json' && model.value && isChatModel(model.value, catalog.value?.models ?? []))
    setChatJsonTemplate()
})
watch([apiKey, model], () => {
  clear()
  rawResult.value = ''
  rawStatus.value = null
})
onBeforeUnmount(() => {
  disposed = true
  modelController?.abort()
  clearTimeout(keyTimer)
  controller?.abort()
  apiKey.value = ''
})
</script>
<template>
  <div :class="{ 'studio-public': isPublic }">
    <PublicHeader v-if="isPublic" />
    <main class="playground-page" :class="{ 'studio-public-main': isPublic }">
      <header v-if="isPublic" class="studio-section-title playground-title">
        <h1>{{ s('API 调试台', 'API Playground') }}</h1>
        <RouterLink to="/docs">{{ s('文档', 'Docs') }} ↗</RouterLink>
      </header>
      <div class="playground-layout">
        <aside class="studio-panel playground-config">
          <div class="panel-heading">
            <h3>
              <el-icon><Setting /></el-icon> {{ s('配置', 'Configuration') }}
            </h3>
          </div>
          <div class="playground-field">
            <label>{{ s('调试方式', 'Request type') }}</label>
            <el-select v-model="requestMode" :disabled="running"
              ><el-option value="conversation" :label="s('对话', 'Conversation')" /><el-option
                value="json"
                :label="s('JSON 请求 · 全部模型', 'JSON request · all models')"
            /></el-select>
          </div>
          <div class="playground-field">
            <div class="field-label">
              <label for="pg-key">API Key</label>
              <el-tooltip :content="s('密钥仅在当前页面保存。', 'Your key stays in page memory.')">
                <button
                  type="button"
                  class="field-help"
                  :aria-label="s('密钥说明', 'About your key')"
                >
                  <el-icon><QuestionFilled /></el-icon>
                </button>
              </el-tooltip>
            </div>
            <el-input
              id="pg-key"
              v-model="apiKey"
              type="password"
              show-password
              placeholder="sk-…"
              autocomplete="off"
              :disabled="running"
            />
            <el-button
              link
              :loading="keyLoading"
              :disabled="!apiKey.trim() || running"
              @click="loadKeyModels"
              >{{ s('读取 Key 可用模型', 'Load key models') }}</el-button
            >
            <p v-if="keyError" class="field-note" role="alert">{{ keyError }}</p>
            <p v-else class="field-note">
              {{
                verifiedKey
                  ? s(
                      '已按此 Key 的权限加载模型，调用按量计费。',
                      'Models loaded for this key. Requests are billed by usage.'
                    )
                  : s(
                      '填入 Key 后自动读取可用模型；密钥仅保存在当前页面。',
                      'Enter a key to load its models. Your key stays in page memory.'
                    )
              }}
            </p>
          </div>
          <div class="playground-field">
            <label for="pg-model">{{ s('模型', 'Model') }}</label>
            <el-select
              id="pg-model"
              v-model="model"
              filterable
              default-first-option
              :placeholder="s('先输入 Key，再选择模型', 'Enter a key, then choose a model')"
              :disabled="running"
              :aria-label="s('模型', 'Model')"
            >
              <el-option v-for="m in models" :key="m" :label="m" :value="m" />
            </el-select>
            <p v-if="catalogError" class="field-note">
              {{
                s(
                  '模型能力信息加载失败，不影响按 Key 读取模型。',
                  'Capability labels are unavailable; key model discovery still works.'
                )
              }}
            </p>
          </div>
          <div v-if="requestMode === 'json'" class="playground-field">
            <label>{{ s('请求接口', 'Endpoint') }}</label>
            <el-select v-model="rawPath" :disabled="running"
              ><el-option v-for="path in apiPaths" :key="path" :value="path" :label="path"
            /></el-select>
            <p class="field-note">
              {{
                s(
                  '选择此模型支持的接口。model 自动使用所选模型，调用按量计费。',
                  'Choose an endpoint supported by the selected model. The model field is set automatically. Requests are billed by usage.'
                )
              }}
            </p>
          </div>
          <details v-else class="playground-advanced">
            <summary>
              <span>{{ s('高级设置', 'Advanced settings') }}</span>
              <el-icon class="advanced-chevron"><ArrowRight /></el-icon>
            </summary>
            <div class="advanced-fields">
              <div class="playground-field">
                <label for="pg-protocol">{{ s('接口', 'API mode') }}</label>
                <el-select
                  id="pg-protocol"
                  v-model="protocol"
                  :disabled="running"
                  :aria-label="s('接口', 'API mode')"
                >
                  <el-option value="auto" :label="s('自动', 'Automatic')" />
                  <el-option value="responses" label="Responses / Codex" />
                  <el-option value="anthropic" label="Anthropic Messages / Claude" />
                  <el-option value="chat" label="Chat Completions" />
                </el-select>
              </div>
              <div class="playground-field">
                <label for="pg-system">{{ s('系统提示词', 'System prompt') }}</label>
                <el-input
                  id="pg-system"
                  v-model="system"
                  type="textarea"
                  :rows="3"
                  :disabled="running"
                  :placeholder="s('选填', 'Optional')"
                />
              </div>
              <div class="playground-field temperature-field">
                <div class="field-label">
                  <el-checkbox v-model="temperatureEnabled" :disabled="running"
                    >Temperature</el-checkbox
                  >
                  <span>{{ temperatureEnabled ? temperature : s('默认', 'Default') }}</span>
                </div>
                <el-slider
                  v-model="temperature"
                  :min="0"
                  :max="2"
                  :step="0.1"
                  :disabled="running || !temperatureEnabled"
                  :aria-label="s('温度', 'Temperature')"
                />
              </div>
              <div class="playground-field">
                <label for="pg-max-tokens"
                  >{{ s('输出上限', 'Output limit') }} <span class="field-unit">tokens</span></label
                >
                <el-input-number
                  id="pg-max-tokens"
                  v-model="maxTokens"
                  :min="activeProtocol === 'responses' ? 16 : 1"
                  :max="32768"
                  :step="256"
                  :disabled="running"
                />
              </div>
            </div>
          </details>
          <div class="config-actions">
            <el-button :icon="CopyDocument" @click="codeOpen = true">{{
              s('请求代码', 'Request code')
            }}</el-button>
            <RouterLink class="text-button" to="/models"
              >{{ s('浏览模型', 'Browse models') }} ↗</RouterLink
            >
          </div>
        </aside>
        <section v-if="requestMode === 'json'" class="studio-panel playground-chat">
          <div class="panel-heading">
            <h3>JSON {{ s('请求', 'request') }}</h3>
            <code>{{ rawPath }}</code>
          </div>
          <p class="field-note">
            {{
              s(
                '支持文本、嵌入、审核、图片和视频创建等 JSON 接口。音频上传和图片编辑等文件请求请使用文档中的客户端。',
                'Test JSON APIs for text, embeddings, moderation, images and video creation. Use a client for multipart file uploads.'
              )
            }}
          </p>
          <el-input
            v-model="rawBody"
            type="textarea"
            :rows="12"
            :disabled="running"
            :aria-label="s('JSON 请求体', 'JSON request body')"
          />
          <div class="config-actions">
            <el-button v-if="running" @click="controller?.abort()">{{
              s('停止', 'Stop')
            }}</el-button
            ><el-button v-else type="primary" :loading="validating || keyLoading" @click="send">{{
              s('发送请求', 'Send request')
            }}</el-button
            ><span v-if="rawStatus">HTTP {{ rawStatus }} · {{ elapsed }} ms</span>
          </div>
          <el-alert v-if="error" :title="error" type="warning" :closable="false" />
          <pre v-if="rawResult" class="request-code">{{ rawResult }}</pre>
        </section>
        <section v-else class="studio-panel playground-chat">
          <div class="panel-heading">
            <div class="conversation-title">
              <h3>{{ s('对话', 'Conversation') }}</h3>
              <span v-if="model" class="muted selected-model" :title="model">{{ model }}</span>
            </div>
            <div class="conversation-actions">
              <span v-if="model" class="protocol-label">{{
                activeProtocol === 'anthropic'
                  ? 'Anthropic Messages'
                  : activeProtocol === 'responses'
                    ? 'Responses'
                    : 'Chat Completions'
              }}</span>
              <el-tooltip :content="s('清空对话', 'Clear conversation')">
                <el-button
                  text
                  :icon="Delete"
                  :aria-label="s('清空对话', 'Clear conversation')"
                  :disabled="running || !messages.length"
                  @click="clear"
                />
              </el-tooltip>
            </div>
          </div>
          <div ref="chatHost" class="chat-messages" aria-live="polite" :aria-busy="running">
            <div v-if="!messages.length" class="chat-empty">
              <span class="chat-empty-icon"
                ><el-icon><ChatDotRound /></el-icon
              ></span>
              <h2>{{ s('开始对话', 'Start a conversation') }}</h2>
              <div class="prompt-suggestions">
                <button
                  type="button"
                  @click="prompt = s('用三句话介绍你自己', 'Introduce yourself in three sentences')"
                >
                  {{ s('自我介绍', 'Introduce yourself') }} ↗
                </button>
                <button
                  type="button"
                  @click="
                    prompt = s('写一个 Python 快速排序函数', 'Write a Python quicksort function')
                  "
                >
                  {{ s('写段代码', 'Write some code') }} ↗
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
          <div v-if="running || firstToken !== null || elapsed || usage" class="response-metadata">
            <span v-if="running">{{ s('生成中', 'Generating') }}</span>
            <span v-if="firstToken !== null"
              >{{ s('首字', 'First token') }} {{ firstToken }} ms</span
            >
            <span v-if="elapsed">{{ s('耗时', 'Time') }} {{ (elapsed / 1000).toFixed(2) }} s</span>
            <span v-if="usage?.total_tokens != null">{{ usage.total_tokens }} tokens</span>
          </div>
          <form class="chat-composer" @submit.prevent="send">
            <el-input
              v-model="prompt"
              type="textarea"
              :autosize="{ minRows: 2, maxRows: 6 }"
              :placeholder="s('输入消息…', 'Type a message…')"
              :disabled="running"
              :aria-label="s('消息', 'Message')"
              aria-keyshortcuts="Control+Enter Meta+Enter"
              @keydown.ctrl.enter.prevent="send"
              @keydown.meta.enter.prevent="send"
            />
            <div>
              <span class="shortcut-hint">⌘ / Ctrl + Enter</span>
              <el-button v-if="running" :icon="VideoPause" @click="controller?.abort()">{{
                s('停止', 'Stop')
              }}</el-button>
              <el-button
                v-else
                type="primary"
                native-type="submit"
                :loading="validating || keyLoading"
                :icon="Promotion"
                >{{ s('发送', 'Send') }}</el-button
              >
            </div>
          </form>
        </section>
      </div>
      <el-dialog v-model="codeOpen" :title="s('请求代码', 'Request code')" width="min(740px,94vw)">
        <p class="studio-note">
          {{ s('将 YOUR_API_KEY 替换为你的密钥。', 'Replace YOUR_API_KEY with your key.') }}
        </p>
        <pre class="request-code">{{ code }}</pre>
        <template #footer
          ><el-button type="primary" :icon="CopyDocument" @click="copyCode">{{
            s('复制代码', 'Copy code')
          }}</el-button></template
        >
      </el-dialog>
    </main>
  </div>
</template>

<style scoped>
.playground-title {
  margin-bottom: 26px;
}
.playground-title h1 {
  margin: 0;
  font-size: 26px;
}
.playground-layout {
  min-height: 0;
  align-items: start;
}
.playground-config {
  display: block;
}
.playground-config .panel-heading {
  margin-bottom: 24px;
}
.playground-field {
  margin-bottom: 22px;
  min-width: 0;
}
.playground-field > label,
.field-label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
  font-size: 12px;
  color: #687996;
}
.field-label > label {
  color: inherit;
}
.field-help {
  display: inline-flex;
  border: 0;
  padding: 2px;
  background: transparent;
  color: #94a2b9;
  cursor: help;
}
.field-help:focus-visible {
  outline: 2px solid #3975f6;
  outline-offset: 3px;
  border-radius: 4px;
}
.field-note {
  line-height: 1.6;
  margin-top: 7px;
}
.field-unit {
  color: #91a0b7;
  font-size: 11px;
}
.playground-advanced {
  border-top: 1px solid #edf1f7;
}
.playground-advanced summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 16px 0;
  font-size: 12px;
  color: #687996;
  cursor: pointer;
  list-style: none;
}
.playground-advanced summary::-webkit-details-marker {
  display: none;
}
.advanced-chevron {
  transition: transform 0.15s ease;
}
.playground-advanced[open] .advanced-chevron {
  transform: rotate(90deg);
}
.advanced-fields {
  padding-top: 4px;
}
.temperature-field .field-label {
  margin-bottom: 0;
}
.config-actions {
  display: grid;
  gap: 12px;
  border-top: 1px solid #edf1f7;
  padding-top: 18px;
}
.config-actions .el-button {
  width: 100%;
  font-size: 12px;
}
.config-actions .text-button {
  text-align: center;
  font-size: 12px;
}
.playground-chat {
  height: clamp(500px, calc(100dvh - 220px), 760px);
  min-height: 500px;
  max-height: 850px;
}
.playground-chat > .panel-heading {
  align-items: center;
  min-height: 66px;
  padding: 16px 22px;
  gap: 16px;
}
.conversation-title {
  display: grid;
  min-width: 0;
  gap: 4px;
}
.selected-model {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.conversation-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}
.protocol-label {
  font-size: 10px;
  color: #8b9ab1;
}
.chat-messages {
  min-height: 180px;
  max-height: none;
}
.chat-empty {
  min-height: 180px;
}
.chat-empty-icon {
  width: 48px;
  height: 48px;
  font-size: 24px;
  border-radius: 14px;
  margin-bottom: 16px;
}
.chat-empty h2 {
  font-size: 17px;
  margin-bottom: 18px;
}
.prompt-suggestions {
  gap: 8px;
}
.prompt-suggestions button {
  padding: 7px 11px;
}
.response-metadata > span:first-child {
  color: inherit;
  font-weight: 400;
  letter-spacing: normal;
}
@media (max-width: 700px) {
  .playground-config {
    padding: 18px;
  }
  .playground-config .panel-heading {
    margin-bottom: 18px;
  }
  .playground-field {
    margin-bottom: 18px;
  }
  .playground-chat {
    height: 500px;
    min-height: 460px;
  }
  .playground-chat > .panel-heading {
    padding: 14px 18px;
  }
  .protocol-label {
    display: none;
  }
  .shortcut-hint {
    visibility: hidden;
  }
}
</style>
