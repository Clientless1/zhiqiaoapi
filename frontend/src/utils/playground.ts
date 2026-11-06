export type PlaygroundProtocol = 'responses' | 'chat'
export type PlaygroundMessage = { role: 'user' | 'assistant'; content: string }
export type PlaygroundUsage = {
  prompt_tokens?: number
  completion_tokens?: number
  total_tokens?: number
}

export function preferredPlaygroundProtocol(model: string): PlaygroundProtocol {
  const name = model.toLowerCase().split('/').pop() ?? ''
  return /^(gpt-|o[1-9]|chatgpt|codex)/.test(name) ? 'responses' : 'chat'
}

export function buildPlaygroundPayload(options: {
  protocol: PlaygroundProtocol
  model: string
  messages: PlaygroundMessage[]
  system: string
  temperature?: number
  maxTokens: number
}) {
  const shared = {
    model: options.model,
    ...(options.temperature == null ? {} : { temperature: options.temperature }),
    stream: true
  }
  if (options.protocol === 'responses') {
    return {
      ...shared,
      input: options.messages.map((message) => ({ ...message })),
      ...(options.system ? { instructions: options.system } : {}),
      max_output_tokens: Math.max(16, options.maxTokens),
      store: false
    }
  }
  return {
    ...shared,
    messages: [
      ...(options.system ? [{ role: 'system', content: options.system }] : []),
      ...options.messages.map((message) => ({ ...message }))
    ],
    max_tokens: options.maxTokens
  }
}

type JsonObject = Record<string, unknown>
function record(value: unknown): JsonObject {
  return value && typeof value === 'object' ? (value as JsonObject) : {}
}
function text(value: unknown) {
  return typeof value === 'string' ? value : ''
}
function outputText(value: unknown) {
  const response = record(value)
  if (typeof response.output_text === 'string') return response.output_text
  if (!Array.isArray(response.output)) return ''
  return response.output
    .filter((item) => record(item).type === 'message')
    .flatMap((item) =>
      Array.isArray(record(item).content) ? (record(item).content as unknown[]) : []
    )
    .map((part) => {
      const content = record(part)
      return content.type === 'output_text' ? text(content.text) : text(content.refusal)
    })
    .join('')
}

type PlaygroundUpdate = {
  delta?: string
  fullText?: string
  usage?: PlaygroundUsage
  done?: boolean
  error?: string
  incomplete?: string
}

export function readPlaygroundEvent(value: unknown): PlaygroundUpdate {
  const event = record(value)
  if (event.error)
    return { error: text(record(event.error).message) || text(event.error) || 'Upstream error' }
  if (event.type === 'error') return { error: text(event.message) || 'Upstream stream error' }
  const response = event.response ? record(event.response) : event
  if (event.type === 'response.failed' || response.status === 'failed')
    return { error: text(record(response.error).message) || 'Model response failed' }
  const result: PlaygroundUpdate = {}
  const usage = record(response.usage)
  if (response.usage) {
    const input = usage.input_tokens ?? usage.prompt_tokens
    const output = usage.output_tokens ?? usage.completion_tokens
    result.usage = {
      prompt_tokens: typeof input === 'number' ? input : undefined,
      completion_tokens: typeof output === 'number' ? output : undefined,
      total_tokens: typeof usage.total_tokens === 'number' ? usage.total_tokens : undefined
    }
  }
  if (event.type === 'response.output_text.delta' || event.type === 'response.refusal.delta')
    result.delta = text(event.delta)
  if (
    event.type === 'response.completed' ||
    event.type === 'response.incomplete' ||
    event.object === 'response'
  ) {
    result.fullText = outputText(response)
    result.done = true
    if (response.status === 'incomplete' || event.type === 'response.incomplete')
      result.incomplete = text(record(response.incomplete_details).reason) || 'incomplete'
  }
  if (Array.isArray(event.choices)) {
    const choice = record(event.choices[0])
    result.delta = text(record(choice.delta).content) || text(record(choice.message).content)
    result.done = !!choice.finish_reason
    if (choice.finish_reason === 'length') result.incomplete = 'max_output_tokens'
  }
  return result
}
