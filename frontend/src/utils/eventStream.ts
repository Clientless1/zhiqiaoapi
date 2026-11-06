// Incremental SSE framing: network chunks may split JSON, UTF-8 and CRLF pairs.
export function createEventStreamParser(onData: (data: string) => void) {
  let buffer = ''
  let fields: string[] = []
  function line(value: string) {
    if (!value) {
      if (fields.length) onData(fields.join('\n'))
      fields = []
    } else if (value.startsWith('data:')) fields.push(value.slice(5).replace(/^ /, ''))
  }
  return {
    push(chunk: string) {
      buffer += chunk
      while (true) {
        const index = buffer.search(/[\r\n]/)
        if (index < 0) break
        if (buffer[index] === '\r' && index === buffer.length - 1) break
        const ending = buffer[index] === '\r' && buffer[index + 1] === '\n' ? 2 : 1
        line(buffer.slice(0, index))
        buffer = buffer.slice(index + ending)
      }
    },
    finish() {
      if (buffer.endsWith('\r')) buffer = buffer.slice(0, -1)
      if (buffer) line(buffer)
      line('')
      buffer = ''
    }
  }
}
