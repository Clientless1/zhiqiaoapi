import type { RouterScrollBehavior } from 'vue-router'

export const scrollBehavior: RouterScrollBehavior = (to, _from, savedPosition) => {
  if (savedPosition) return savedPosition
  // OAuth fragments carry short lived credentials, not section anchors.
  if (!to.hash || to.name === 'oauthCallback') return { top: 0 }
  let id = to.hash.slice(1)
  try {
    id = decodeURIComponent(id)
  } catch {
    // A literal percent sign can also be part of an element ID.
  }
  // Async section components may mount after navigation. Treat the fragment
  // as an ID instead of a CSS selector, including punctuation and Unicode.
  return new Promise((resolve) => {
    let attempts = 0
    const tryResolve = () => {
      const element = document.getElementById(id)
      if (element) {
        resolve({ el: element, top: 96, behavior: 'smooth' })
        return
      }
      if (++attempts >= 20) {
        resolve({ top: 0 })
        return
      }
      window.setTimeout(tryResolve, 50)
    }
    tryResolve()
  })
}
