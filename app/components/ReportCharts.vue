<script setup lang="ts">
import type { Task, TaskResult } from '~/composables/useTaskApi'
import type { CircularityMetric } from '~/composables/useReportGenerator'
import { use } from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'
import { LineChart } from 'echarts/charts'
import {
  TitleComponent,
  TooltipComponent,
  LegendComponent,
  GridComponent,
  DataZoomComponent
} from 'echarts/components'
import VChart from 'vue-echarts'

// 注册 ECharts 组件
use([
  CanvasRenderer,
  LineChart,
  TitleComponent,
  TooltipComponent,
  LegendComponent,
  GridComponent,
  DataZoomComponent
])

const props = defineProps<{
  task: Task
  result: TaskResult
}>()

const { calculateCircularityMetrics, calculateAverage, formatTimestamp } = useReportGenerator()

// 计算圆度数据
const circularityMetrics = computed<CircularityMetric[]>(() => {
  return calculateCircularityMetrics(props.result.dynamicMetrics)
})

// 计算各项平均值
const avgBrightness = computed(() => {
  return calculateAverage(props.result.dynamicMetrics.map(m => m.brightness || 0))
})

const avgArea = computed(() => {
  return calculateAverage(props.result.dynamicMetrics.map(m => m.poolArea || 0))
})

const avgPerimeter = computed(() => {
  return calculateAverage(props.result.dynamicMetrics.map(m => m.poolPerimeter || 0))
})

const avgCircularity = computed(() => {
  return calculateAverage(circularityMetrics.value.map(c => c.circularity))
})

// 亮度图表配置
const brightnessChartOption = computed(() => {
  const data = props.result.dynamicMetrics.map(m => [m.timestamp, m.brightness || 0])
  const avgLine = Array(data.length).fill(avgBrightness.value)

  return {
    title: {
      text: '熔池亮度变化趋势',
      left: 'center',
      textStyle: { fontSize: 16 }
    },
    tooltip: {
      trigger: 'axis',
      formatter: (params: any[]) => {
        const time = formatTimestamp(params[0].data[0])
        return `时间: ${time}<br/>亮度: ${params[0].data[1].toFixed(1)}<br/>平均值: ${avgBrightness.value.toFixed(1)}`
      }
    },
    legend: {
      data: ['实时亮度', '平均亮度'],
      top: 30
    },
    grid: {
      left: 60,
      right: 60,
      top: 80,
      bottom: 80
    },
    xAxis: {
      type: 'value',
      name: '时间 (秒)',
      nameLocation: 'middle',
      nameGap: 30
    },
    yAxis: {
      type: 'value',
      name: '亮度值'
    },
    series: [
      {
        name: '实时亮度',
        type: 'line',
        data,
        smooth: true,
        lineStyle: { width: 2 },
        itemStyle: { color: '#3b82f6' }
      },
      {
        name: '平均亮度',
        type: 'line',
        data: avgLine.map((val, idx) => [props.result.dynamicMetrics[idx]?.timestamp || 0, val]),
        lineStyle: { type: 'dashed', width: 2 },
        itemStyle: { color: '#ef4444' }
      }
    ],
    dataZoom: [
      {
        type: 'slider',
        start: 0,
        end: 100
      }
    ]
  }
})

// 面积图表配置
const areaChartOption = computed(() => {
  const data = props.result.dynamicMetrics.map(m => [m.timestamp, m.poolArea || 0])
  const avgLine = Array(data.length).fill(avgArea.value)

  return {
    title: {
      text: '熔池面积变化趋势',
      left: 'center',
      textStyle: { fontSize: 16 }
    },
    tooltip: {
      trigger: 'axis',
      formatter: (params: any[]) => {
        const time = formatTimestamp(params[0].data[0])
        return `时间: ${time}<br/>面积: ${params[0].data[1].toFixed(0)} 像素<br/>平均值: ${avgArea.value.toFixed(0)} 像素`
      }
    },
    legend: {
      data: ['实时面积', '平均面积'],
      top: 30
    },
    grid: {
      left: 60,
      right: 60,
      top: 80,
      bottom: 80
    },
    xAxis: {
      type: 'value',
      name: '时间 (秒)',
      nameLocation: 'middle',
      nameGap: 30
    },
    yAxis: {
      type: 'value',
      name: '面积 (像素)'
    },
    series: [
      {
        name: '实时面积',
        type: 'line',
        data,
        smooth: true,
        lineStyle: { width: 2 },
        itemStyle: { color: '#10b981' }
      },
      {
        name: '平均面积',
        type: 'line',
        data: avgLine.map((val, idx) => [props.result.dynamicMetrics[idx]?.timestamp || 0, val]),
        lineStyle: { type: 'dashed', width: 2 },
        itemStyle: { color: '#ef4444' }
      }
    ],
    dataZoom: [
      {
        type: 'slider',
        start: 0,
        end: 100
      }
    ]
  }
})

// 周长图表配置
const perimeterChartOption = computed(() => {
  const data = props.result.dynamicMetrics.map(m => [m.timestamp, m.poolPerimeter || 0])
  const avgLine = Array(data.length).fill(avgPerimeter.value)

  return {
    title: {
      text: '熔池周长变化趋势',
      left: 'center',
      textStyle: { fontSize: 16 }
    },
    tooltip: {
      trigger: 'axis',
      formatter: (params: any[]) => {
        const time = formatTimestamp(params[0].data[0])
        return `时间: ${time}<br/>周长: ${params[0].data[1].toFixed(1)} 像素<br/>平均值: ${avgPerimeter.value.toFixed(1)} 像素`
      }
    },
    legend: {
      data: ['实时周长', '平均周长'],
      top: 30
    },
    grid: {
      left: 60,
      right: 60,
      top: 80,
      bottom: 80
    },
    xAxis: {
      type: 'value',
      name: '时间 (秒)',
      nameLocation: 'middle',
      nameGap: 30
    },
    yAxis: {
      type: 'value',
      name: '周长 (像素)'
    },
    series: [
      {
        name: '实时周长',
        type: 'line',
        data,
        smooth: true,
        lineStyle: { width: 2 },
        itemStyle: { color: '#f59e0b' }
      },
      {
        name: '平均周长',
        type: 'line',
        data: avgLine.map((val, idx) => [props.result.dynamicMetrics[idx]?.timestamp || 0, val]),
        lineStyle: { type: 'dashed', width: 2 },
        itemStyle: { color: '#ef4444' }
      }
    ],
    dataZoom: [
      {
        type: 'slider',
        start: 0,
        end: 100
      }
    ]
  }
})

// 圆度图表配置
const circularityChartOption = computed(() => {
  const data = circularityMetrics.value.map(m => [m.timestamp, m.circularity])
  const avgLine = Array(data.length).fill(avgCircularity.value)

  return {
    title: {
      text: '熔池圆度变化趋势',
      left: 'center',
      textStyle: { fontSize: 16 }
    },
    tooltip: {
      trigger: 'axis',
      formatter: (params: any[]) => {
        const time = formatTimestamp(params[0].data[0])
        return `时间: ${time}<br/>圆度: ${params[0].data[1].toFixed(3)}<br/>平均值: ${avgCircularity.value.toFixed(3)}`
      }
    },
    legend: {
      data: ['实时圆度', '平均圆度'],
      top: 30
    },
    grid: {
      left: 60,
      right: 60,
      top: 80,
      bottom: 80
    },
    xAxis: {
      type: 'value',
      name: '时间 (秒)',
      nameLocation: 'middle',
      nameGap: 30
    },
    yAxis: {
      type: 'value',
      name: '圆度 (0-1)',
      min: 0,
      max: 1
    },
    series: [
      {
        name: '实时圆度',
        type: 'line',
        data,
        smooth: true,
        lineStyle: { width: 2 },
        itemStyle: { color: '#8b5cf6' }
      },
      {
        name: '平均圆度',
        type: 'line',
        data: avgLine.map((val, idx) => [circularityMetrics.value[idx]?.timestamp || 0, val]),
        lineStyle: { type: 'dashed', width: 2 },
        itemStyle: { color: '#ef4444' }
      }
    ],
    dataZoom: [
      {
        type: 'slider',
        start: 0,
        end: 100
      }
    ]
  }
})
</script>

<template>
  <div class="space-y-6">
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <!-- 亮度图表 -->
      <UCard>
        <VChart :option="brightnessChartOption" style="height: 400px" autoresize />
      </UCard>

      <!-- 面积图表 -->
      <UCard>
        <VChart :option="areaChartOption" style="height: 400px" autoresize />
      </UCard>

      <!-- 周长图表 -->
      <UCard>
        <VChart :option="perimeterChartOption" style="height: 400px" autoresize />
      </UCard>

      <!-- 圆度图表 -->
      <UCard>
        <VChart :option="circularityChartOption" style="height: 400px" autoresize />
      </UCard>
    </div>

    <!-- 统计信息 -->
    <UCard>
      <template #header>
        <h3 class="text-lg font-semibold">动态参数统计摘要</h3>
      </template>

      <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
        <div class="text-center p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
          <p class="text-sm text-muted mb-1">平均亮度</p>
          <p class="text-2xl font-bold text-blue-600 dark:text-blue-400">{{ avgBrightness.toFixed(1) }}</p>
        </div>

        <div class="text-center p-4 bg-green-50 dark:bg-green-900/20 rounded-lg">
          <p class="text-sm text-muted mb-1">平均面积</p>
          <p class="text-2xl font-bold text-green-600 dark:text-green-400">{{ avgArea.toFixed(0) }}</p>
          <p class="text-xs text-muted">像素</p>
        </div>

        <div class="text-center p-4 bg-orange-50 dark:bg-orange-900/20 rounded-lg">
          <p class="text-sm text-muted mb-1">平均周长</p>
          <p class="text-2xl font-bold text-orange-600 dark:text-orange-400">{{ avgPerimeter.toFixed(1) }}</p>
          <p class="text-xs text-muted">像素</p>
        </div>

        <div class="text-center p-4 bg-purple-50 dark:bg-purple-900/20 rounded-lg">
          <p class="text-sm text-muted mb-1">平均圆度</p>
          <p class="text-2xl font-bold text-purple-600 dark:text-purple-400">{{ avgCircularity.toFixed(3) }}</p>
        </div>
      </div>

      <div class="mt-4 p-3 bg-gray-50 dark:bg-gray-800 rounded text-sm text-muted">
        <p><strong>说明：</strong></p>
        <ul class="list-disc list-inside mt-2 space-y-1">
          <li>圆度 = 4π × 面积 / 周长²，范围为 0-1</li>
          <li>圆度值越接近 1，表示形状越接近圆形</li>
          <li>虚线表示该参数在整个视频中的平均值</li>
        </ul>
      </div>
    </UCard>
  </div>
</template>
