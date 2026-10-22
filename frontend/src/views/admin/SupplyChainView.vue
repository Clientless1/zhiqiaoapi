<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import {
  Search,
  ArrowRight,
  Connection,
  ChatLineRound,
  Picture,
  VideoCamera,
  EditPen,
  RefreshRight
} from '@element-plus/icons-vue'
import { getChannels } from '../../api/channels'
import { getChannelPrices } from '../../api/prices'
import {
  getProcurementQuotes,
  saveProcurementQuote,
  type ProcurementQuote
} from '../../api/procurement'
import { useAsyncData } from '../../composables/useAsyncData'
import { useBillingCurrency } from '../../composables/useBillingCurrency'
import { useLocale } from '../../composables/useLocale'
import { procurementScenario } from '../../utils/catalog'
import { readError } from '../../utils/errors'
import ProviderIcon from '../../components/common/ProviderIcon.vue'
import type { Channel, ChannelPrice } from '../../types/channel'

const { t, locale } = useLocale()
const { currencySymbol } = useBillingCurrency()
const { data, loading, error, reload } = useAsyncData(
  async () => {
    const [channels, prices, quotes] = await Promise.all([
      getChannels(),
      getChannelPrices(),
      getProcurementQuotes()
    ])
    return { channels, prices, quotes }
  },
  { channels: [] as Channel[], prices: [] as ChannelPrice[], quotes: [] as ProcurementQuote[] }
)
const query = ref('')
const channelFilter = ref<number | ''>('')
const category = ref('')
const compareModel = ref('')
const monthlyInput = ref(10)
const monthlyOutput = ref(2)
const monthlyUnits = ref(1000)
type SupplyRow = ChannelPrice & { channel?: Channel; quote?: ProcurementQuote }
const rows = computed<SupplyRow[]>(() => {
  const channels = new Map(data.value.channels.map((c) => [c.id, c]))
  const quotes = new Map(data.value.quotes.map((q) => [JSON.stringify([q.channel_id, q.model]), q]))
  return data.value.prices.map((p) => ({
    ...p,
    channel: channels.get(p.channel_id),
    quote: quotes.get(JSON.stringify([p.channel_id, p.model]))
  }))
})
const filtered = computed(() => {
  const search = query.value.trim().toLowerCase()
  return rows.value.filter(
    (r) =>
      (!channelFilter.value || r.channel_id === channelFilter.value) &&
      (!category.value || r.billing_meter === category.value) &&
      (!search ||
        [r.model, r.channel?.name, r.quote?.supplier, r.provider].some((v) =>
          v?.toLowerCase().includes(search)
        ))
  )
})
const comparableModels = computed(() =>
  [...new Set(rows.value.filter((r) => r.billing_meter !== 'video').map((r) => r.model))].sort()
)
const comparison = computed(() =>
  rows.value
    .filter((r) => r.model === compareModel.value && r.billing_meter !== 'video')
    .map((r) => ({ ...r, cost: scenarioCost(r), revenue: scenarioRevenue(r) }))
    .sort(
      (a, b) =>
        Number(Boolean(b.enabled && b.channel?.enabled)) -
          Number(Boolean(a.enabled && a.channel?.enabled)) ||
        (a.cost ?? Infinity) - (b.cost ?? Infinity)
    )
)
const covered = computed(() => rows.value.filter(hasCost).length)
function hasCost(r: SupplyRow) {
  return r.billing_meter === 'token'
    ? r.quote?.input_cost_micros != null && r.quote?.output_cost_micros != null
    : r.quote?.unit_cost_micros != null
}
function money(v: number | null | undefined) {
  return v == null
    ? '—'
    : `${currencySymbol.value}${(v / 1000000).toLocaleString(locale.value, { maximumFractionDigits: 6 })}`
}
function unit(r: ChannelPrice) {
  return t(
    r.billing_meter === 'token' ||
      (r.billing_meter === 'video' && r.video_billing_mode === 'official_token')
      ? 'perMillion'
      : r.billing_meter === 'image'
        ? 'perImage'
        : 'perSecond'
  )
}
function scenarioCost(r: SupplyRow) {
  if (r.billing_meter === 'video' || !Number.isFinite(monthlyUnits.value)) return null
  return r.billing_meter === 'token'
    ? procurementScenario(
        r.quote?.input_cost_micros,
        r.quote?.output_cost_micros,
        monthlyInput.value,
        monthlyOutput.value
      )
    : r.quote?.unit_cost_micros == null
      ? null
      : Math.round(r.quote.unit_cost_micros * monthlyUnits.value)
}
function scenarioRevenue(r: SupplyRow) {
  if (![monthlyInput.value, monthlyOutput.value, monthlyUnits.value].every(Number.isFinite))
    return null
  return r.billing_meter === 'token'
    ? Math.round(
        r.input_price_micros * monthlyInput.value + r.output_price_micros * monthlyOutput.value
      )
    : r.unit_price_micros == null
      ? null
      : Math.round(r.unit_price_micros * monthlyUnits.value)
}
function margin(cost: number | null, revenue: number | null) {
  return cost == null || revenue == null || revenue <= 0
    ? '—'
    : `${(((revenue - cost) / revenue) * 100).toFixed(1)}%`
}
const drawerOpen = ref(false)
const saving = ref(false)
const editing = ref<SupplyRow | null>(null)
const form = reactive({
  input: undefined as number | undefined,
  output: undefined as number | undefined,
  cacheRead: undefined as number | undefined,
  cacheWrite: undefined as number | undefined,
  unit: undefined as number | undefined,
  budget: undefined as number | undefined,
  supplier: '',
  notes: ''
})
function major(v?: number | null) {
  return v == null ? undefined : v / 1000000
}
function micros(v?: number | null) {
  return v == null ? null : Math.round(v * 1000000)
}
function edit(row: SupplyRow) {
  editing.value = row
  const q = row.quote
  Object.assign(form, {
    input: major(q?.input_cost_micros),
    output: major(q?.output_cost_micros),
    cacheRead: major(q?.cache_read_cost_micros),
    cacheWrite: major(q?.cache_write_cost_micros),
    unit: major(q?.unit_cost_micros),
    budget: major(q?.monthly_budget_micros),
    supplier: q?.supplier ?? '',
    notes: q?.notes ?? ''
  })
  drawerOpen.value = true
}
async function save() {
  if (!editing.value || saving.value) return
  saving.value = true
  try {
    const quote = await saveProcurementQuote({
      channel_id: editing.value.channel_id,
      model: editing.value.model,
      input_cost_micros: micros(form.input),
      output_cost_micros: micros(form.output),
      cache_read_cost_micros: micros(form.cacheRead),
      cache_write_cost_micros: micros(form.cacheWrite),
      unit_cost_micros: micros(form.unit),
      monthly_budget_micros: micros(form.budget),
      supplier: form.supplier.trim(),
      notes: form.notes.trim()
    })
    data.value.quotes = [
      ...data.value.quotes.filter(
        (q) => !(q.channel_id === quote.channel_id && q.model === quote.model)
      ),
      quote
    ]
    drawerOpen.value = false
    ElMessage.success(t('quoteSaved'))
  } catch (e) {
    ElMessage.error(`${t('quoteSaveError')}: ${readError(e)}`)
  } finally {
    saving.value = false
  }
}
const guides = [
  { title: 'textIntegration', description: 'textIntegrationHint', icon: ChatLineRound },
  { title: 'imageIntegration', description: 'imageIntegrationHint', icon: Picture },
  { title: 'videoIntegration', description: 'videoIntegrationHint', icon: VideoCamera }
] as const
</script>

<template>
  <section class="supply-chain-page">
    <header class="explorer-hero supply-hero">
      <div>
        <span class="section-eyebrow"><span></span>{{ t('supplyEyebrow') }}</span>
        <h1>{{ t('supplyTitle') }}</h1>
        <p>{{ t('supplyIntro') }}</p>
      </div>
      <RouterLink class="solid-link" to="/admin/channels"
        >{{ t('manageChannels') }}<el-icon><ArrowRight /></el-icon
      ></RouterLink>
    </header>
    <div class="supply-stats">
      <div>
        <span>{{ t('activeChannels') }}</span
        ><strong
          >{{ data.channels.filter((c) => c.enabled).length }}<el-icon><Connection /></el-icon
        ></strong>
      </div>
      <div>
        <span>{{ t('configuredQuotes') }}</span
        ><strong
          >{{ covered }}<small>/ {{ rows.length }}</small></strong
        >
      </div>
      <div>
        <span>{{ t('missingQuotes') }}</span
        ><strong>{{ rows.length - covered }}</strong>
      </div>
      <div>
        <span>{{ t('costCoverage') }}</span
        ><strong
          >{{ rows.length ? Math.round((covered / rows.length) * 100) : 0 }}<small>%</small></strong
        >
      </div>
    </div>
    <div class="supply-guides">
      <article v-for="guide in guides" :key="guide.title">
        <span
          ><el-icon><component :is="guide.icon" /></el-icon
        ></span>
        <div>
          <h3>{{ t(guide.title) }}</h3>
          <p>{{ t(guide.description) }}</p>
        </div>
      </article>
    </div>
    <div class="supply-panel">
      <div class="section-heading">
        <div>
          <h2>{{ t('purchasePrice') }} & {{ t('sellingPrice') }}</h2>
          <p>{{ t('procurementPrivate') }}</p>
        </div>
        <el-button
          :icon="RefreshRight"
          :aria-label="t('retryLoad')"
          :loading="loading"
          @click="reload()"
        />
      </div>
      <div class="catalog-toolbar">
        <el-input
          v-model="query"
          :prefix-icon="Search"
          clearable
          :placeholder="t('searchSupply')"
          :aria-label="t('searchSupply')"
        /><el-select
          v-model="channelFilter"
          :aria-label="t('allChannels')"
          :placeholder="t('allChannels')"
          ><el-option value="" :label="t('allChannels')" /><el-option
            v-for="c in data.channels"
            :key="c.id"
            :value="c.id"
            :label="c.name" /></el-select
        ><el-select v-model="category" :aria-label="t('modelTypes')" :placeholder="t('allModels')"
          ><el-option value="" :label="t('allModels')" /><el-option
            value="token"
            :label="t('textModels')" /><el-option
            value="image"
            :label="t('imageModels')" /><el-option
            value="video"
            :label="t('videoModels')" /><el-option value="audio" :label="t('audioModels')"
        /></el-select>
      </div>
      <el-alert v-if="error" :title="error" type="error" :closable="false" />
      <div class="supply-table-wrap">
        <el-table
          :data="filtered"
          v-loading="loading"
          class="supply-table"
          :empty-text="t('noSupply')"
          row-key="id"
          ><el-table-column label="模型 / Model" min-width="215"
            ><template #default="{ row }"
              ><div class="supply-model-cell">
                <ProviderIcon :provider="row.provider" />
                <div>
                  <strong>{{ row.model }}</strong
                  ><small>{{ row.provider }} · {{ unit(row) }}</small>
                </div>
              </div></template
            ></el-table-column
          ><el-table-column :label="t('channel')" min-width="160"
            ><template #default="{ row }"
              ><div class="supply-stacked">
                <strong>{{ row.channel?.name || '—' }}</strong
                ><small :class="row.channel?.enabled && row.enabled ? 'text-success' : 'text-muted'"
                  >{{ t(row.channel?.enabled && row.enabled ? 'enabledLabel' : 'unavailable') }} ·
                  {{
                    row.channel?.endpoints
                      .filter((e: { enabled: boolean }) => e.enabled)
                      .map((e: { protocol: string }) => e.protocol)
                      .join(' / ')
                  }}</small
                >
              </div></template
            ></el-table-column
          ><el-table-column :label="t('supplier')" min-width="130"
            ><template #default="{ row }">{{
              row.quote?.supplier || '—'
            }}</template></el-table-column
          ><el-table-column :label="t('purchasePrice')" min-width="170"
            ><template #default="{ row }"
              ><div class="supply-stacked" v-if="row.billing_meter === 'token'">
                <span>{{ t('inputPrice') }} {{ money(row.quote?.input_cost_micros) }}</span
                ><span>{{ t('outputPrice') }} {{ money(row.quote?.output_cost_micros) }}</span>
              </div>
              <span v-else>{{ money(row.quote?.unit_cost_micros) }}</span></template
            ></el-table-column
          ><el-table-column :label="t('sellingPrice')" min-width="170"
            ><template #default="{ row }"
              ><div class="supply-stacked" v-if="row.billing_meter === 'token'">
                <span>{{ t('inputPrice') }} {{ money(row.input_price_micros) }}</span
                ><span>{{ t('outputPrice') }} {{ money(row.output_price_micros) }}</span>
              </div>
              <span v-else-if="row.billing_meter === 'video'">{{ t('videoTiered') }}</span
              ><span v-else>{{ money(row.unit_price_micros) }}</span></template
            ></el-table-column
          ><el-table-column :label="t('monthlyBudget')" min-width="135"
            ><template #default="{ row }">{{
              money(row.quote?.monthly_budget_micros)
            }}</template></el-table-column
          ><el-table-column fixed="right" width="125"
            ><template #default="{ row }"
              ><el-button link type="primary" :icon="EditPen" @click="edit(row)">{{
                t(row.quote ? 'editQuote' : 'addQuote')
              }}</el-button></template
            ></el-table-column
          ></el-table
        >
      </div>
      <p v-if="!rows.length && !loading" class="panel-footnote">{{ t('noSupplyHint') }}</p>
    </div>
    <section class="supply-panel scenario-panel">
      <div class="section-heading">
        <div>
          <span class="section-eyebrow">COST PLANNER</span>
          <h2>{{ t('scenario') }}</h2>
          <p>{{ t('scenarioHint') }}</p>
        </div>
      </div>
      <div class="scenario-inputs">
        <label
          >{{ t('sameModel')
          }}<el-select v-model="compareModel" filterable :placeholder="t('sameModel')"
            ><el-option
              v-for="model in comparableModels"
              :key="model"
              :label="model"
              :value="model" /></el-select></label
        ><label
          >{{ t('monthlyInput')
          }}<el-input-number
            v-model="monthlyInput"
            :min="0"
            :max="1000000"
            :precision="3"
            controls-position="right" /></label
        ><label
          >{{ t('monthlyOutput')
          }}<el-input-number
            v-model="monthlyOutput"
            :min="0"
            :max="1000000"
            :precision="3"
            controls-position="right" /></label
        ><label
          >{{ t('monthlyUnits')
          }}<el-input-number
            v-model="monthlyUnits"
            :min="0"
            :max="1000000000"
            :precision="0"
            controls-position="right"
        /></label>
      </div>
      <div v-if="comparison.length" class="comparison-list">
        <article v-for="(r, i) in comparison" :key="r.id">
          <div>
            <strong>{{ r.channel?.name }}</strong
            ><span
              >{{ r.quote?.supplier || r.provider
              }}<small
                v-if="i === 0 && r.cost != null && r.enabled && r.channel?.enabled"
                class="cheapest-badge"
                >{{ t('lowestQuote') }}</small
              ></span
            ><small v-if="!r.enabled || !r.channel?.enabled" class="text-muted">{{
              t('unavailable')
            }}</small>
          </div>
          <div>
            <span>{{ t('estimatedPurchase') }}</span
            ><strong>{{ money(r.cost) }}</strong>
          </div>
          <div>
            <span>{{ t('grossMargin') }}</span
            ><strong>{{ margin(r.cost, r.revenue) }}</strong>
          </div>
          <span
            class="budget-badge"
            :class="{
              'over-budget':
                r.cost != null &&
                r.quote?.monthly_budget_micros != null &&
                r.cost > r.quote.monthly_budget_micros
            }"
            >{{
              t(
                r.quote?.monthly_budget_micros == null
                  ? 'noBudget'
                  : r.cost == null
                    ? 'missingCost'
                    : r.cost > r.quote.monthly_budget_micros
                      ? 'overBudget'
                      : 'withinBudget'
              )
            }}</span
          >
        </article>
      </div>
    </section>
    <el-drawer
      v-model="drawerOpen"
      :title="t('editQuote')"
      size="520px"
      class="procurement-drawer"
      :close-on-click-modal="!saving"
      :close-on-press-escape="!saving"
      :show-close="!saving"
    >
      <template v-if="editing"
        ><div class="quote-model-heading">
          <ProviderIcon :provider="editing.provider" />
          <div>
            <strong>{{ editing.model }}</strong
            ><span>{{ editing.channel?.name }} · {{ unit(editing) }}</span>
          </div>
        </div>
        <p class="panel-footnote">{{ t('quoteHint') }}</p>
        <el-form label-position="top" @submit.prevent="save"
          ><el-form-item :label="t('supplier')"
            ><el-input v-model="form.supplier" maxlength="120"
          /></el-form-item>
          <div class="quote-cost-fields" v-if="editing.billing_meter === 'token'">
            <el-form-item
              v-for="field in [
                { key: 'input', label: 'inputPrice' },
                { key: 'output', label: 'outputPrice' },
                { key: 'cacheRead', label: 'cacheRead' },
                { key: 'cacheWrite', label: 'cacheWrite' }
              ] as const"
              :key="field.key"
              :label="`${t(field.label)} (${currencySymbol} ${t('perMillion')})`"
              ><el-input-number
                v-model="form[field.key]"
                :min="0"
                :max="1000000"
                :precision="6"
                :controls="false"
                :placeholder="t('missingCost')"
            /></el-form-item>
          </div>
          <template v-else
            ><p v-if="editing.billing_meter === 'video'" class="panel-footnote">
              {{ t('videoCostHint') }}
            </p>
            <el-form-item
              :label="`${t(editing.billing_meter === 'video' ? 'videoUnitCost' : 'purchasePrice')} (${currencySymbol} ${unit(editing)})`"
              ><el-input-number
                v-model="form.unit"
                :min="0"
                :max="1000000"
                :precision="6"
                :controls="false"
                :placeholder="t('missingCost')" /></el-form-item></template
          ><el-form-item :label="`${t('monthlyBudget')} (${currencySymbol})`"
            ><el-input-number
              v-model="form.budget"
              :min="0"
              :max="1000000"
              :precision="2"
              :controls="false"
              :placeholder="t('noBudget')"
            />
            <p class="field-hint">{{ t('budgetHint') }}</p></el-form-item
          ><el-form-item :label="t('notesLabel')"
            ><el-input
              v-model="form.notes"
              type="textarea"
              :rows="4"
              maxlength="2000"
              show-word-limit /></el-form-item
          ><el-button
            type="primary"
            native-type="submit"
            :loading="saving"
            class="quote-save-button"
            >{{ t('quoteSave') }}</el-button
          ></el-form
        ></template
      >
    </el-drawer>
  </section>
</template>
