import { describe, it, expect } from 'vitest'
import { createEventStreamParser } from './eventStream'
describe('event stream framing', () => {
  it('handles split CRLF, comments, multiline events and a final unterminated event', () => {
    const events: string[] = []
    const parser = createEventStreamParser((data) => events.push(data))
    for (const chunk of [
      ': ping\r',
      '\ndata: {"text":',
      '"你好"}\r',
      '\ndata: second\r\n\r',
      '\ndata: [DONE]'
    ])
      parser.push(chunk)
    parser.finish()
    expect(events).toEqual(['{"text":"你好"}\nsecond', '[DONE]'])
  })
  it('does not dispatch empty events or unknown fields', () => {
    const events: string[] = []
    const parser = createEventStreamParser((data) => events.push(data))
    parser.push('event: message\nid: 1\n\ndata: ok\n\n\n')
    parser.finish()
    expect(events).toEqual(['ok'])
  })
})
