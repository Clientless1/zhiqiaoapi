import { computed, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { getCurrentUser, type CurrentUser } from '../api/me'
import type { LoginRole } from '../api/auth'

const tokenStorageKey = 'zhiqiao_token'
const roleStorageKey = 'zhiqiao_role'
const passwordChangeStorageKey = 'zhiqiao_requires_password_change'

export const useAuthStore = defineStore('auth', () => {
  const token = ref(localStorage.getItem(tokenStorageKey) || '')
  const storedRole = localStorage.getItem(roleStorageKey)
  const role = ref<LoginRole | ''>(
    (isLoginRole(storedRole) ? storedRole : null) || roleFromToken(token.value)
  )
  const requiresPasswordChange = ref(
    token.value.length > 0 && localStorage.getItem(passwordChangeStorageKey) === 'true'
  )
  const sessionChecked = ref(token.value.length === 0)
  const verifiedToken = ref('')
  const currentUser = ref<CurrentUser | null>(null)
  const accountName = computed(() => currentUser.value?.username || currentUser.value?.email || '')
  const isAuthed = computed(() => token.value.length > 0)
  const isAdmin = computed(() => role.value === 'admin')
  const isUser = computed(() => role.value === 'user')
  let sessionPromise: Promise<boolean> | null = null

  function setToken(nextToken: string, nextRole: LoginRole, nextRequiresPasswordChange = false) {
    token.value = nextToken
    role.value = nextRole
    requiresPasswordChange.value = nextRole === 'user' && nextRequiresPasswordChange
    currentUser.value = null
    verifiedToken.value = ''
    sessionChecked.value = false
  }

  function clearToken() {
    token.value = ''
    role.value = ''
    requiresPasswordChange.value = false
    verifiedToken.value = ''
    currentUser.value = null
    sessionChecked.value = true
    sessionPromise = null
  }

  function markPasswordChanged() {
    requiresPasswordChange.value = false
    sessionChecked.value = false
    verifiedToken.value = ''
  }

  function markPasswordChangeRequired() {
    requiresPasswordChange.value = role.value === 'user'
  }

  async function verifySession(force = false) {
    if (!token.value) {
      clearToken()
      return false
    }

    if (!force && sessionChecked.value && verifiedToken.value === token.value) {
      return isAuthed.value && isLoginRole(role.value)
    }

    if (sessionPromise) return sessionPromise

    const requestToken = token.value
    sessionPromise = getCurrentUser(requestToken)
      .then((account) => {
        if (token.value !== requestToken) {
          return Boolean(token.value)
        }

        currentUser.value = account
        role.value = account.role
        requiresPasswordChange.value = account.role === 'user' && account.requires_password_change
        verifiedToken.value = requestToken
        sessionChecked.value = true
        return true
      })
      .catch(() => {
        if (token.value === requestToken) {
          clearToken()
        }
        return false
      })
      .finally(() => {
        sessionPromise = null
      })

    return sessionPromise
  }

  watch(
    token,
    (nextToken) => {
      if (nextToken) {
        localStorage.setItem(tokenStorageKey, nextToken)
      } else {
        localStorage.removeItem(tokenStorageKey)
      }
      if (nextToken !== verifiedToken.value) {
        sessionChecked.value = nextToken.length === 0
      }
    },
    { immediate: true }
  )

  watch(
    role,
    (nextRole) => {
      if (nextRole) {
        localStorage.setItem(roleStorageKey, nextRole)
      } else {
        localStorage.removeItem(roleStorageKey)
      }
    },
    { immediate: true }
  )

  watch(
    requiresPasswordChange,
    (nextRequiresPasswordChange) => {
      if (nextRequiresPasswordChange) {
        localStorage.setItem(passwordChangeStorageKey, 'true')
      } else {
        localStorage.removeItem(passwordChangeStorageKey)
      }
    },
    { immediate: true }
  )

  return {
    token,
    role,
    currentUser,
    accountName,
    requiresPasswordChange,
    isAuthed,
    isAdmin,
    isUser,
    sessionChecked,
    setToken,
    clearToken,
    markPasswordChangeRequired,
    markPasswordChanged,
    verifySession
  }
})

function roleFromToken(token: string): LoginRole | '' {
  if (/^neo_admin_v[123]_/.test(token)) return 'admin'
  if (/^neo_user_v[12]_/.test(token)) return 'user'
  return ''
}

function isLoginRole(value: unknown): value is LoginRole {
  return value === 'admin' || value === 'user'
}
