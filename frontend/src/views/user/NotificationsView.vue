<script setup lang="ts">
import { ref } from 'vue'
import { ElMessage } from 'element-plus'
import {
  getNotifications,
  markAllNotificationsRead,
  markNotificationRead
} from '../../api/commerce'
import { emptyPage, type UserNotification } from '../../api/commerce'
import { useCommercePage } from '../../composables/useCommercePage'
import CommercePagination from '../../components/common/CommercePagination.vue'
import { useLocale } from '../../composables/useLocale'
import { formatCompactDateTime } from '../../utils/format'
import { readError } from '../../utils/errors'
const { t } = useLocale()
const { data, loading, error, reload, page, pageSize, filter } = useCommercePage(getNotifications, {
  ...emptyPage<UserNotification>(),
  unread_count: 0
})
const saving = ref(false)
async function markRead(id?: number) {
  if (saving.value) return
  saving.value = true
  try {
    if (id != null) await markNotificationRead(id)
    else await markAllNotificationsRead()
    await reload()
    window.dispatchEvent(new Event('notifications-updated'))
  } catch (error) {
    ElMessage.error(readError(error))
  } finally {
    saving.value = false
  }
}
</script>
<template>
  <section class="commerce-view">
    <div class="commerce-toolbar">
      <p class="commerce-intro">
        {{ t('notificationsIntro') }}
        <span v-if="data.unread_count">{{
          t('notificationsUnread', { count: data.unread_count })
        }}</span>
      </p>
      <el-button :disabled="!data.unread_count" :loading="saving" @click="markRead()">{{
        t('notificationsReadAll')
      }}</el-button>
    </div>
    <el-radio-group v-model="filter"
      ><el-radio-button value="all">{{ t('notificationsAll') }}</el-radio-button
      ><el-radio-button value="unread">{{ t('notificationsNew') }}</el-radio-button
      ><el-radio-button value="read">{{
        t('notificationsAlreadyRead')
      }}</el-radio-button></el-radio-group
    >
    <el-alert v-if="error" type="error" :title="error" :closable="false" />
    <div v-loading="loading" class="notification-list">
      <article
        v-for="notice in data.items"
        :key="notice.id"
        class="user-panel commerce-card"
        :class="{ unread: !notice.read_at }"
      >
        <div class="commerce-card-heading">
          <h3>
            {{ notice.title }}
            <el-tag v-if="!notice.read_at" size="small">{{ t('notificationsNew') }}</el-tag>
          </h3>
          <time class="commerce-label">{{ formatCompactDateTime(notice.created_at) }}</time>
        </div>
        <p class="notification-content">{{ notice.content }}</p>
        <el-button
          v-if="!notice.read_at"
          text
          type="primary"
          :disabled="saving"
          class="notification-read"
          @click="markRead(notice.id)"
          >{{ t('notificationsRead') }}</el-button
        >
      </article>
    </div>
    <el-empty
      v-if="!loading && !error && !data.items.length"
      :description="t('notificationsEmpty')"
    />
    <CommercePagination
      v-model:page="page"
      v-model:page-size="pageSize"
      :total="data.total"
      :loading="loading"
    />
  </section>
</template>
<style scoped>
.notification-list {
  display: grid;
  gap: 12px;
}
.unread {
  border-left: 3px solid #6655f5;
}
.notification-content {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.notification-read {
  align-self: flex-start;
  margin-left: -12px;
}
.commerce-card-heading h3 {
  display: flex;
  align-items: center;
  gap: 10px;
}
@media (max-width: 600px) {
  .commerce-card-heading {
    align-items: flex-start;
    flex-direction: column;
  }
}
</style>
