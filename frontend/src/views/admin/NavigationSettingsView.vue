<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import {
  getAdminNavigationSetting,
  saveNavigationSetting,
  type OptionalNavigationItem
} from '../../api/navigation'
import { useStudio } from '../../composables/useStudio'
import { readError } from '../../utils/errors'

const { s } = useStudio()
const loading = ref(true)
const saving = ref(false)
const loadError = ref('')
const hiddenItems = ref<OptionalNavigationItem[]>([])
const original = ref<OptionalNavigationItem[]>([])
const dirty = computed(
  () => [...hiddenItems.value].sort().join(',') !== [...original.value].sort().join(',')
)
const groups: {
  zh: string
  en: string
  items: { id: OptionalNavigationItem; zh: string; en: string }[]
}[] = [
  {
    zh: '开发',
    en: 'Development',
    items: [
      { id: 'models', zh: '模型广场', en: 'Models' },
      { id: 'playground', zh: 'API 调试台', en: 'Playground' },
      { id: 'apikeys', zh: 'API 密钥', en: 'API keys' },
      { id: 'usage', zh: '使用明细', en: 'Usage' }
    ]
  },
  {
    zh: '账户',
    en: 'Account',
    items: [
      { id: 'packages', zh: '套餐', en: 'Plans' },
      { id: 'coupons', zh: '优惠券', en: 'Coupons' },
      { id: 'redeem', zh: '兑换码', en: 'Redeem' },
      { id: 'orders', zh: '订单', en: 'Orders' },
      { id: 'ledger', zh: '资金流水', en: 'Transactions' },
      { id: 'recharge', zh: '充值', en: 'Top up' }
    ]
  },
  {
    zh: '系统',
    en: 'System',
    items: [{ id: 'notifications', zh: '通知公告', en: 'Notifications' }]
  }
]
function setVisible(id: OptionalNavigationItem, visible: boolean) {
  hiddenItems.value = visible
    ? hiddenItems.value.filter((item) => item !== id)
    : [...hiddenItems.value, id]
}
async function load() {
  loading.value = true
  loadError.value = ''
  try {
    const setting = await getAdminNavigationSetting()
    hiddenItems.value = [...setting.hidden_items]
    original.value = [...setting.hidden_items]
  } catch (err) {
    loadError.value = readError(err)
  } finally {
    loading.value = false
  }
}
async function save() {
  if (loadError.value || loading.value) return
  saving.value = true
  try {
    const setting = await saveNavigationSetting({ hidden_items: hiddenItems.value })
    hiddenItems.value = [...setting.hidden_items]
    original.value = [...setting.hidden_items]
    ElMessage.success(s('菜单设置已保存', 'Navigation settings saved'))
  } catch (err) {
    ElMessage.error(readError(err))
  } finally {
    saving.value = false
  }
}
onMounted(load)
</script>

<template>
  <section v-loading="loading" class="navigation-settings">
    <header class="navigation-settings-header">
      <div>
        <h2>{{ s('用户菜单', 'User navigation') }}</h2>
        <p>
          {{ s('选择用户工作台中显示的页面。', 'Choose which pages appear in the user console.') }}
        </p>
      </div>
      <el-button
        type="primary"
        :loading="saving"
        :disabled="!dirty || loading || !!loadError"
        @click="save"
        >{{ s('保存更改', 'Save changes') }}</el-button
      >
    </header>
    <div v-if="loadError" class="navigation-settings-error" role="alert">
      <span>{{ loadError }}</span>
      <el-button @click="load">{{ s('重新加载', 'Retry') }}</el-button>
    </div>
    <section v-for="group in groups" :key="group.en" class="navigation-settings-group">
      <h3>{{ s(group.zh, group.en) }}</h3>
      <div class="navigation-settings-options">
        <label v-for="item in group.items" :key="item.id" class="navigation-settings-option">
          <span>{{ s(item.zh, item.en) }}</span>
          <el-switch
            :model-value="!hiddenItems.includes(item.id)"
            :disabled="loading || saving || !!loadError"
            :aria-label="s(item.zh, item.en)"
            @change="setVisible(item.id, $event === true)"
          />
        </label>
      </div>
    </section>
    <footer>
      {{
        s(
          '仪表盘和个人设置始终保留。隐藏页面不会停用 API 服务或改变账户权限。',
          'Dashboard and account settings remain available. Hiding pages does not disable APIs or change account permissions.'
        )
      }}
    </footer>
  </section>
</template>

<style scoped>
.navigation-settings {
  max-width: 960px;
  width: 100%;
  background: #fff;
  border: 1px solid #e5e7eb;
  border-radius: 12px;
  overflow: hidden;
}
.navigation-settings-error {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 24px;
  background: #fff8f5;
  color: #a94926;
  font-size: 13px;
  gap: 16px;
}
.navigation-settings-header {
  padding: 24px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  border-bottom: 1px solid #eceef2;
}
.navigation-settings-header h2 {
  margin: 0 0 6px;
  font-size: 18px;
  font-weight: 650;
  color: #202938;
}
.navigation-settings-header p {
  margin: 0;
  font-size: 13px;
  color: #6b7280;
}
.navigation-settings-group {
  display: grid;
  grid-template-columns: 130px minmax(0, 1fr);
  gap: 24px;
  padding: 24px;
  border-bottom: 1px solid #eceef2;
}
.navigation-settings-group h3 {
  margin: 10px 0;
  color: #697386;
  font-size: 13px;
  font-weight: 600;
}
.navigation-settings-options {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 2px 36px;
}
.navigation-settings-option {
  display: flex;
  justify-content: space-between;
  align-items: center;
  min-height: 44px;
  gap: 16px;
  font-size: 14px;
  color: #202938;
  cursor: pointer;
}
.navigation-settings footer {
  color: #7a8392;
  padding: 18px 24px;
  font-size: 12px;
  line-height: 1.7;
}
@media (max-width: 640px) {
  .navigation-settings-header {
    align-items: flex-start;
    flex-direction: column;
    padding: 20px;
  }
  .navigation-settings-group {
    grid-template-columns: 1fr;
    gap: 4px;
    padding: 16px 20px;
  }
  .navigation-settings-options {
    grid-template-columns: 1fr;
  }
}
</style>
