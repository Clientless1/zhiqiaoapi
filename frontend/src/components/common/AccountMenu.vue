<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ArrowDown, Bell, User } from '@element-plus/icons-vue'
import { useAuthStore } from '../../stores/auth'
import { useLocale } from '../../composables/useLocale'
import { useLogout } from '../../composables/useLogout'
import { userRequest } from '../../api/request'

const auth = useAuthStore()
const { t } = useLocale()
const route = useRoute()
const router = useRouter()
const logout = useLogout(t)
const unreadCount = ref(0)
const displayName = computed(
  () => auth.accountName || t(auth.isAdmin ? 'administrator' : 'accountUser')
)
const initials = computed(() => auth.accountName.slice(0, 1).toUpperCase())

async function refreshNotifications() {
  if (!auth.isAuthed || !auth.isUser) {
    unreadCount.value = 0
    return
  }
  const token = auth.token
  try {
    const data = await userRequest<{ unread_count: number }>('/api/user/notifications')
    if (token === auth.token) unreadCount.value = Math.max(0, data.unread_count || 0)
  } catch {
    unreadCount.value = 0
  }
}

function command(value: string) {
  if (value === 'logout') void logout()
  else void router.push(value)
}

watch(
  () => auth.token,
  async (token) => {
    if (token) await auth.verifySession()
    await refreshNotifications()
  },
  { immediate: true }
)
watch(() => route.fullPath, refreshNotifications)
onMounted(() => window.addEventListener('notifications-updated', refreshNotifications))
onBeforeUnmount(() => window.removeEventListener('notifications-updated', refreshNotifications))
</script>

<template>
  <div v-if="auth.isAuthed" class="account-actions">
    <RouterLink
      v-if="auth.isUser"
      to="/home/notifications"
      class="account-notification"
      :aria-label="t('accountNotifications')"
      :title="t('accountNotifications')"
    >
      <el-badge :value="unreadCount" :hidden="!unreadCount" :max="99">
        <el-icon :size="19"><Bell /></el-icon>
      </el-badge>
    </RouterLink>
    <el-dropdown trigger="click" placement="bottom-end" @command="command">
      <button class="account-menu-trigger" type="button" :aria-label="t('accountMenu')">
        <el-avatar :size="32" :src="auth.currentUser?.avatar_url || undefined">
          <span v-if="initials">{{ initials }}</span>
          <el-icon v-else><User /></el-icon>
        </el-avatar>
        <span class="account-display-name">{{ displayName }}</span>
        <el-icon class="account-menu-chevron"><ArrowDown /></el-icon>
      </button>
      <template #dropdown>
        <el-dropdown-menu>
          <el-dropdown-item disabled>{{ displayName }}</el-dropdown-item>
          <el-dropdown-item :command="auth.isAdmin ? '/admin' : '/home/overview'">
            {{ t('console') }}
          </el-dropdown-item>
          <el-dropdown-item
            :command="auth.isAdmin ? '/admin/settings/admin-password' : '/home/settings'"
          >
            {{ t('personalSettings') }}
          </el-dropdown-item>
          <el-dropdown-item divided command="logout">{{ t('logout') }}</el-dropdown-item>
        </el-dropdown-menu>
      </template>
    </el-dropdown>
  </div>
</template>

<style scoped>
.account-actions {
  display: inline-flex;
  align-items: center;
  gap: 14px;
  min-width: 0;
}
.account-notification {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 34px;
  color: #626b80;
  border-radius: 8px;
}
.account-notification:hover {
  color: #4b42ff;
  background: #f2f3ff;
}
.account-menu-trigger {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  max-width: 200px;
  padding: 0;
  border: 0;
  background: transparent;
  color: #293044;
  font: inherit;
  font-size: 13px;
  cursor: pointer;
}
.account-menu-trigger:focus-visible {
  outline: 2px solid #6c63ff;
  outline-offset: 4px;
  border-radius: 6px;
}
.account-menu-trigger .el-avatar {
  background: #efedff;
  color: #5145e8;
  font-size: 14px;
  font-weight: 600;
}
.account-display-name {
  max-width: 126px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.account-menu-chevron {
  color: #9298a8;
  font-size: 11px;
}
@media (max-width: 520px) {
  .account-actions {
    gap: 6px;
  }
  .account-display-name {
    max-width: 64px;
    font-size: 12px;
  }
  .account-menu-chevron {
    display: none;
  }
}
</style>
