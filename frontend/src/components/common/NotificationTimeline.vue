<script setup lang="ts">
import { ref } from 'vue'
import { ElMessage } from 'element-plus'
import {
  getNotifications,
  markAllNotificationsRead,
  markNotificationRead,
  emptyPage,
  type UserNotification
} from '../../api/commerce'
import { useCommercePage } from '../../composables/useCommercePage'
import CommercePagination from './CommercePagination.vue'
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
  <section class="notice-center">
    <div class="notice-tools">
      <el-radio-group v-model="filter" size="small">
        <el-radio-button value="all">{{ t('notificationsAll') }}</el-radio-button>
        <el-radio-button value="unread"
          >{{ t('notificationsNew')
          }}<span v-if="data.unread_count"> · {{ data.unread_count }}</span></el-radio-button
        >
        <el-radio-button value="read">{{ t('notificationsAlreadyRead') }}</el-radio-button>
      </el-radio-group>
      <el-button text :disabled="!data.unread_count" :loading="saving" @click="markRead()">{{
        t('notificationsReadAll')
      }}</el-button>
    </div>
    <el-alert v-if="error" type="error" :title="error" :closable="false" show-icon
      ><el-button text @click="reload()">{{ t('refresh') }}</el-button></el-alert
    >
    <div v-loading="loading" class="notice-scroll" :aria-busy="loading">
      <ol class="notice-timeline">
        <li v-for="notice in data.items" :key="notice.id" :class="{ 'is-unread': !notice.read_at }">
          <time>{{ formatCompactDateTime(notice.created_at) }}</time>
          <div class="notice-title">
            <h3>{{ notice.title }}</h3>
            <span v-if="!notice.read_at" class="notice-new">{{ t('notificationsNew') }}</span>
          </div>
          <p>{{ notice.content }}</p>
          <el-button
            v-if="!notice.read_at"
            text
            type="primary"
            size="small"
            :disabled="saving"
            @click="markRead(notice.id)"
            >{{ t('notificationsRead') }}</el-button
          >
        </li>
      </ol>
      <div v-if="!loading && !error && !data.items.length" class="notice-empty">
        <span>✓</span>
        <p>{{ t('notificationsEmpty') }}</p>
      </div>
    </div>
    <CommercePagination
      v-if="data.total > pageSize || page > 1"
      v-model:page="page"
      v-model:page-size="pageSize"
      :total="data.total"
      :loading="loading"
    />
  </section>
</template>
<style scoped>
.notice-center {
  display: grid;
  gap: 20px;
  min-width: 0;
  color: #252b37;
}
.notice-tools {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  flex-wrap: wrap;
  border-bottom: 1px solid #eaecf0;
  padding-bottom: 16px;
}
.notice-scroll {
  min-height: 170px;
  max-height: 58dvh;
  overflow-y: auto;
  padding: 0 8px 0 3px;
  overscroll-behavior: contain;
}
.notice-timeline {
  list-style: none;
  margin: 0;
  padding: 0 0 0 22px;
}
.notice-timeline li {
  position: relative;
  padding: 0 0 28px 22px;
  border-left: 1px solid #e3e7ed;
  overflow-wrap: anywhere;
}
.notice-timeline li:last-child {
  padding-bottom: 8px;
}
.notice-timeline li:before {
  content: '';
  position: absolute;
  left: -4px;
  top: 5px;
  width: 7px;
  height: 7px;
  background: #bac2cf;
  border-radius: 50%;
  box-shadow: 0 0 0 4px #fff;
}
.notice-timeline li.is-unread:before {
  background: #3668e8;
  box-shadow: 0 0 0 4px #edf3ff;
}
.notice-timeline time {
  font-size: 12px;
  color: #7a8393;
  font-variant-numeric: tabular-nums;
}
.notice-title {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 6px 0 8px;
}
.notice-title h3 {
  font-size: 15px;
  font-weight: 600;
  margin: 0;
  line-height: 1.5;
}
.notice-new {
  color: #3767de;
  background: #edf3ff;
  border-radius: 4px;
  padding: 1px 6px;
  font-size: 11px;
  white-space: nowrap;
}
.notice-timeline p {
  font-size: 13px;
  line-height: 1.8;
  color: #626c7b;
  white-space: pre-wrap;
  margin: 0;
}
.notice-timeline .el-button {
  margin: 8px 0 0 -10px;
}
.notice-empty {
  padding: 34px 16px;
  text-align: center;
  color: #7b8493;
  font-size: 13px;
}
.notice-empty span {
  display: inline-grid;
  place-items: center;
  width: 36px;
  height: 36px;
  border-radius: 50%;
  background: #f2f5f8;
  color: #6a7a8e;
  margin-bottom: 12px;
}
.notice-empty p {
  margin: 0;
}
@media (max-width: 520px) {
  .notice-tools > .el-button {
    padding-inline: 0;
  }
  .notice-tools {
    gap: 10px;
  }
  .notice-timeline {
    padding-left: 9px;
  }
  .notice-timeline li {
    padding-left: 18px;
  }
}
</style>
