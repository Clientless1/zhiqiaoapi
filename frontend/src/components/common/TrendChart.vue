<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount } from 'vue'
import { use } from 'echarts/core'
import { LineChart, BarChart } from 'echarts/charts'
import { GridComponent, TooltipComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import { init, type ECharts } from 'echarts/core'
use([LineChart, BarChart, GridComponent, TooltipComponent, CanvasRenderer])
const props = defineProps<{
  days: { date: string; value: number }[]
  label: string
  format: (n: number) => string
  bars?: boolean
}>()
const host = ref<HTMLElement>(),
  chart = ref<ECharts>(),
  observer = ref<ResizeObserver>()
function draw() {
  chart.value?.setOption(
    {
      animation: !window.matchMedia('(prefers-reduced-motion: reduce)').matches,
      animationDuration: 500,
      grid: { top: 20, right: 20, bottom: 25, left: 68 },
      tooltip: {
        trigger: 'axis',
        confine: true,
        valueFormatter: (v: unknown) => props.format(Number(v))
      },
      xAxis: {
        type: 'category',
        boundaryGap: !!props.bars,
        data: props.days.map((d) => d.date.slice(5)),
        axisLine: { show: false },
        axisTick: { show: false },
        axisLabel: { color: '#8792aa', fontSize: 11, margin: 12 }
      },
      yAxis: {
        type: 'value',
        min: 0,
        splitNumber: 3,
        axisLabel: { color: '#8792aa', fontSize: 11, formatter: (v: number) => props.format(v) },
        splitLine: { lineStyle: { color: '#edf1f7', type: 'dashed' } }
      },
      series: [
        {
          name: props.label,
          type: props.bars ? 'bar' : 'line',
          data: props.days.map((d) => d.value),
          smooth: 0.25,
          symbol: 'circle',
          symbolSize: 6,
          showSymbol: props.days.length < 12,
          itemStyle: { color: '#3975f6', borderRadius: [4, 4, 0, 0] },
          lineStyle: { width: 2.5, color: '#3975f6' },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: '#3975f62b' },
                { offset: 1, color: '#3975f601' }
              ]
            }
          }
        }
      ]
    },
    { notMerge: true }
  )
}
onMounted(() => {
  if (host.value) {
    chart.value = init(host.value)
    observer.value = new ResizeObserver(() => chart.value?.resize())
    observer.value.observe(host.value)
    draw()
  }
})
watch(() => [props.days, props.label, props.bars], draw, { deep: true })
onBeforeUnmount(() => {
  observer.value?.disconnect()
  chart.value?.dispose()
})
</script>
<template>
  <div
    ref="host"
    class="studio-trend-chart"
    role="img"
    :aria-label="`${label}: ${days.map((d) => d.date + ' ' + format(d.value)).join('; ')}`"
  ></div>
</template>
