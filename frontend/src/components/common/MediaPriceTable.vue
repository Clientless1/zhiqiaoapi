<script setup lang="ts">
import { computed } from 'vue'
import type { ModelOffer } from '../../api/catalog'
import { mediaQuoteRows } from '../../utils/catalog'
import { useStudio } from '../../composables/useStudio'
const props = withDefaults(
  defineProps<{ offers: ModelOffer[]; currency: string; limit?: number }>(),
  { limit: 1000 }
)
const emit = defineEmits<{ expand: [] }>()
const { s } = useStudio()
const rows = computed(() => mediaQuoteRows(props.offers))
function money(value: number) {
  return `${props.currency === 'USD' ? '$' : '¥'}${Number((value / 1_000_000).toFixed(6))}`
}
function size(value: string) {
  return value === '*'
    ? s('其他规格', 'Other sizes')
    : value === 'auto'
      ? s('默认 / auto', 'Default / auto')
      : value.toUpperCase().replace('X', '×')
}
function quality(value: string) {
  return (
    (
      {
        '*': s('全部画质', 'All qualities'),
        auto: 'auto',
        low: s('低 / low', 'Low'),
        medium: s('中 / medium', 'Medium'),
        high: s('高 / high', 'High'),
        standard: 'standard',
        hd: 'HD'
      } as Record<string, string>
    )[value] ?? value
  )
}
function unit(value: string) {
  return value === 'image'
    ? s('/ 张', '/ image')
    : value === 'second'
      ? s('/ 秒', '/ sec')
      : '/ 1M tokens'
}
</script>
<template>
  <div class="media-quotes">
    <table v-if="rows.length">
      <thead>
        <tr>
          <th>{{ s('规格', 'Specification') }}</th>
          <th>{{ s('价格', 'Price') }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in rows.slice(0, limit)" :key="row.key">
          <td>
            <strong>{{ size(row.spec) }}</strong
            ><small v-if="offers[0]?.billing_meter === 'image'">{{ quality(row.quality) }}</small
            ><small v-if="row.condition">{{
              row.condition === 'base'
                ? s('无视频输入', 'No video input')
                : s('含视频输入', 'With video input')
            }}</small>
          </td>
          <td>
            <span
              >{{ money(row.min)
              }}<template v-if="row.max !== row.min">–{{ money(row.max) }}</template></span
            ><small>{{ unit(row.unit) }}</small>
          </td>
        </tr>
      </tbody>
    </table>
    <p v-else class="media-price-empty">{{ s('暂无规格报价', 'No specification prices') }}</p>
    <button v-if="rows.length > limit" class="media-more" @click="emit('expand')">
      {{ s('查看全部', 'View all') }} {{ rows.length }} {{ s('档价格', 'price tiers') }} ↗
    </button>
  </div>
</template>
<style scoped>
.media-quotes {
  margin: 16px 0;
  min-width: 0;
}
table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}
th {
  color: #8a95a7;
  font-weight: 400;
  text-align: left;
  padding-bottom: 6px;
}
th:last-child,
td:last-child {
  text-align: right;
}
td {
  padding: 8px 0;
  border-top: 1px solid #eef1f6;
  vertical-align: top;
}
td strong {
  font-size: 12px;
  font-weight: 550;
  color: #39475c;
}
td span {
  font-variant-numeric: tabular-nums;
  color: #243854;
  font-weight: 600;
  overflow-wrap: anywhere;
}
small {
  display: block;
  color: #8490a3;
  font-size: 10px;
  line-height: 1.7;
  font-weight: 400;
}
.media-more {
  background: none;
  border: 0;
  padding: 7px 0 0;
  color: #4466c6;
  cursor: pointer;
  font-size: 11px;
}
.media-price-empty {
  color: #8490a3;
  font-size: 12px;
}
</style>
