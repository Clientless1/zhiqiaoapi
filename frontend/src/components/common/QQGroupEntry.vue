<script setup lang="ts">
import { computed, ref } from 'vue'
import { ChatDotRound, CopyDocument, TopRight } from '@element-plus/icons-vue'
import { getCommunity, type CommunityInfo } from '../../api/commerce'
import { useStudio } from '../../composables/useStudio'
import { useAuthStore } from '../../stores/auth'
import { copyTextWithMessage } from '../../utils/clipboard'
import { qqGroupUrl } from '../../utils/community'
const { s } = useStudio()
const auth = useAuthStore()
const open = ref(false)
const loading = ref(false)
const failed = ref(false)
const community = ref<CommunityInfo | null>(null)
const joinUrl = computed(() => qqGroupUrl(community.value?.qq_group_url))
const groupNumber = computed(() => community.value?.qq_group_number?.trim() || '')

async function showGroup() {
  open.value = true
  loading.value = true
  failed.value = false
  try {
    community.value = await getCommunity()
  } catch {
    failed.value = true
  } finally {
    loading.value = false
  }
}
function copyGroup() {
  if (groupNumber.value)
    void copyTextWithMessage(groupNumber.value, s('群号已复制', 'Group number copied'))
}
</script>

<template>
  <button
    type="button"
    class="qq-group-trigger"
    :aria-label="s('加入 QQ 群', 'QQ community')"
    @click="showGroup"
  >
    <el-icon><ChatDotRound /></el-icon><span>{{ s('加入 QQ 群', 'QQ community') }}</span>
  </button>
  <el-dialog
    v-model="open"
    :title="s('加入 QQ 交流群', 'Join our QQ community')"
    width="min(420px, calc(100vw - 32px))"
    append-to-body
    class="qq-group-dialog"
  >
    <div v-loading="loading" class="qq-group-content" :aria-busy="loading">
      <template v-if="!loading">
        <template v-if="failed">
          <p>
            {{
              s(
                '群信息暂时无法加载，请稍后重试。',
                'Group details could not be loaded. Please try again.'
              )
            }}
          </p>
          <el-button @click="showGroup">{{ s('重新加载', 'Retry') }}</el-button>
        </template>
        <template v-else-if="groupNumber || joinUrl">
          <p>
            {{ s('交流使用经验，获取产品更新。', 'Share tips and catch up on product updates.') }}
          </p>
          <div v-if="groupNumber" class="qq-group-number">
            <div>
              <span>{{ s('QQ群号', 'QQ group number') }}</span
              ><strong>{{ groupNumber }}</strong>
            </div>
            <el-button
              :icon="CopyDocument"
              :aria-label="s('复制群号', 'Copy group number')"
              @click="copyGroup"
            />
          </div>
          <a
            v-if="joinUrl"
            class="qq-group-join"
            :href="joinUrl"
            target="_blank"
            rel="noopener noreferrer"
            >{{ s('一键加群', 'Join group') }}<el-icon><TopRight /></el-icon
          ></a>
          <el-button v-else type="primary" size="large" class="qq-group-copy" @click="copyGroup">{{
            s('复制群号，在 QQ 中查找', 'Copy number to find it in QQ')
          }}</el-button>
          <small>{{
            joinUrl
              ? s(
                  '将在新窗口打开 QQ 官方加群页面。',
                  'Opens the official QQ group page in a new window.'
                )
              : s(
                  '打开 QQ，搜索群号即可申请加入。',
                  'Search for this group number in QQ to request access.'
                )
          }}</small>
        </template>
        <template v-else>
          <p>
            {{
              s(
                '交流群暂未开放，群信息设置后会显示在这里。',
                'The community is not available yet. Group details will appear here once configured.'
              )
            }}
          </p>
          <RouterLink
            v-if="auth.isAdmin"
            class="qq-group-settings"
            to="/admin/user-operations"
            @click="open = false"
            >{{
              s('前往「用户运营 → 社区设置」配置', 'Configure in User operations → Community')
            }}
            ↗</RouterLink
          >
        </template>
      </template>
    </div>
  </el-dialog>
</template>

<style scoped>
.qq-group-trigger {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border: 1px solid #e3e9f1;
  border-radius: 7px;
  background: #fff;
  color: #52627b;
  padding: 7px 10px;
  white-space: nowrap;
  font: inherit;
  font-size: 12px;
  cursor: pointer;
  transition:
    border-color 0.15s,
    color 0.15s;
}
.qq-group-trigger:hover,
.qq-group-trigger:focus-visible {
  border-color: #2563eb;
  color: #2563eb;
}
.qq-group-trigger .el-icon {
  font-size: 15px;
}
.qq-group-content {
  display: grid;
  gap: 18px;
  min-height: 120px;
  padding: 4px 0 8px;
  color: #627086;
}
.qq-group-content p {
  margin: 0;
  font-size: 14px;
  line-height: 1.8;
}
.qq-group-number {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  border: 1px solid #e4eaf2;
  background: #f7f9fc;
  padding: 14px 16px;
  border-radius: 8px;
}
.qq-group-number div {
  display: grid;
  gap: 5px;
}
.qq-group-number span {
  font-size: 12px;
}
.qq-group-number strong {
  font-size: 22px;
  letter-spacing: 1px;
  color: #25334b;
  font-variant-numeric: tabular-nums;
}
.qq-group-join {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  background: #2563eb;
  color: white;
  text-decoration: none;
  border-radius: 7px;
  min-height: 44px;
  font-size: 14px;
  font-weight: 600;
}
.qq-group-join:hover {
  background: #1d4ed8;
}
.qq-group-copy {
  width: 100%;
}
.qq-group-content small {
  font-size: 12px;
  line-height: 1.6;
  color: #8b96a8;
}
.qq-group-settings {
  color: #2563eb;
  font-size: 13px;
  text-decoration: none;
}
@media (max-width: 700px) {
  .qq-group-trigger {
    padding: 7px;
  }
  .qq-group-trigger span {
    display: none;
  }
}
</style>
