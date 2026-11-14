// A separate client endpoint must never change session or OAuth destinations.
export function resolveApiOrigin(configured: string | null | undefined, websiteOrigin: string) {
  if (!configured?.trim()) return websiteOrigin
  try {
    const url = new URL(configured.trim())
    if (
      url.protocol !== 'https:' ||
      url.username ||
      url.password ||
      url.pathname !== '/' ||
      url.search ||
      url.hash
    )
      return websiteOrigin
    return url.origin
  } catch {
    return websiteOrigin
  }
}
