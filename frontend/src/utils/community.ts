export function qqGroupUrl(value?: string | null) {
  try {
    const url = new URL(value?.trim() ?? '')
    return url.protocol === 'https:' &&
      !url.username &&
      !url.password &&
      (!url.port || url.port === '443') &&
      (url.hostname === 'qq.com' || url.hostname.endsWith('.qq.com'))
      ? url.href
      : ''
  } catch {
    return ''
  }
}
