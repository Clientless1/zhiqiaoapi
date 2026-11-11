import { describe, expect, it } from 'vitest'
import {
  buildPlaygroundPayload,
  parsePlaygroundJson,
  preferredPlaygroundProtocol,
  readPlaygroundEvent
} from './playground'
import { createEventStreamParser } from './eventStream'

describe('playground protocol compatibility', () => {
  it('uses Responses for Codex models while preserving a Chat option', () => {
    expect(preferredPlaygroundProtocol('openai/gpt-6.1-sol')).toBe('responses')
    expect(preferredPlaygroundProtocol('deepseek-chat')).toBe('chat')
    const options = {
      model: 'gpt-6.1-sol',
      messages: [{ role: 'user' as const, content: '你好' }],
      system: '简洁回答',
      maxTokens: 128
    }
    expect(buildPlaygroundPayload({ ...options, protocol: 'responses' })).toEqual({
      model: options.model,
      input: options.messages,
      instructions: '简洁回答',
      max_output_tokens: 128,
      stream: true,
      store: false
    })
    expect(buildPlaygroundPayload({ ...options, protocol: 'chat' })).toMatchObject({
      messages: [{ role: 'system', content: '简洁回答' }, ...options.messages],
      max_tokens: 128
    })
  })

  it('reads split Responses SSE deltas and completes without requiring DONE', () => {
    const updates: ReturnType<typeof readPlaygroundEvent>[] = []
    const parser = createEventStreamParser((raw) =>
      updates.push(readPlaygroundEvent(JSON.parse(raw)))
    )
    parser.push('event: response.output_text.delta\ndata: {"type":"response.output_text.del')
    parser.push(
      'ta","delta":"你好"}\n\ndata: {"type":"response.completed","response":{"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"你好"}]}],"usage":{"input_tokens":10,"output_tokens":2,"total_tokens":12}}}\n\n'
    )
    parser.finish()
    expect(updates[0].delta).toBe('你好')
    expect(updates[1]).toMatchObject({
      done: true,
      fullText: '你好',
      usage: { prompt_tokens: 10, completion_tokens: 2, total_tokens: 12 }
    })
  })

  it('reports failed and incomplete response events instead of treating them as success', () => {
    expect(
      readPlaygroundEvent({
        type: 'response.failed',
        response: { error: { message: 'Invalid channel key' } }
      }).error
    ).toBe('Invalid channel key')
    expect(readPlaygroundEvent({ type: 'error', message: 'Stream error' }).error).toBe(
      'Stream error'
    )
    expect(
      readPlaygroundEvent({
        type: 'response.incomplete',
        response: { incomplete_details: { reason: 'max_output_tokens' } }
      })
    ).toMatchObject({ done: true, incomplete: 'max_output_tokens' })
  })

  it('still reads Chat Completions deltas and terminal finish reasons', () => {
    expect(
      readPlaygroundEvent({ choices: [{ delta: { content: 'Hello' }, finish_reason: null }] })
    ).toMatchObject({ delta: 'Hello', done: false })
    expect(readPlaygroundEvent({ choices: [{ delta: {}, finish_reason: 'stop' }] }).done).toBe(true)
    expect(
      readPlaygroundEvent({ choices: [{ message: { content: 'Hello' }, finish_reason: 'length' }] })
    ).toMatchObject({ delta: 'Hello', done: true, incomplete: 'max_output_tokens' })
  })
  it('builds native Claude requests and parses Anthropic streaming usage', () => {
    expect(preferredPlaygroundProtocol('anthropic/claude-sonnet-4-5')).toBe('anthropic')
    expect(
      buildPlaygroundPayload({
        protocol: 'anthropic',
        model: 'claude-test',
        messages: [{ role: 'user', content: 'hi' }],
        system: 'brief',
        maxTokens: 64
      })
    ).toEqual({
      model: 'claude-test',
      messages: [{ role: 'user', content: 'hi' }],
      system: 'brief',
      max_tokens: 64,
      stream: true
    })
    expect(
      readPlaygroundEvent({ type: 'message_start', message: { usage: { input_tokens: 12 } } })
    ).toEqual({ usage: { prompt_tokens: 12 } })
    expect(
      readPlaygroundEvent({
        type: 'content_block_delta',
        delta: { type: 'text_delta', text: '你好' }
      })
    ).toEqual({ delta: '你好' })
    expect(
      readPlaygroundEvent({
        type: 'message_delta',
        delta: { stop_reason: 'max_tokens' },
        usage: { output_tokens: 3 }
      })
    ).toMatchObject({ usage: { completion_tokens: 3 }, incomplete: 'max_output_tokens' })
    expect(readPlaygroundEvent({ type: 'message_stop' })).toEqual({ done: true })
    expect(
      readPlaygroundEvent({
        type: 'error',
        error: { type: 'authentication_error', message: 'Invalid key' }
      })
    ).toEqual({ error: 'Invalid key' })
  })
  it('keeps JSON requests inside the selected model and rejects non-object input', () => {
    expect(parsePlaygroundJson('{"model":"not-authorized","input":"hello"}', 'allowed')).toEqual({
      model: 'allowed',
      input: 'hello'
    })
    for (const body of ['null', '[]', '"string"', '42', '{'])
      expect(() => parsePlaygroundJson(body, 'allowed')).toThrow()
  })
})
