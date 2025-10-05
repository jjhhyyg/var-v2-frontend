<script setup lang="ts">
import { use } from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'
import { LineChart } from 'echarts/charts'
import {
  TitleComponent,
  TooltipComponent,
  LegendComponent,
  GridComponent,
  DataZoomComponent,
  MarkLineComponent,
  ToolboxComponent
} from 'echarts/components'
import VChart from 'vue-echarts'
import type { EChartsOption } from 'echarts'

// 注册必要的 ECharts 组件
use([
  CanvasRenderer,
  LineChart,
  TitleComponent,
  TooltipComponent,
  LegendComponent,
  GridComponent,
  DataZoomComponent,
  MarkLineComponent,
  ToolboxComponent
])

interface DynamicMetric {
  frameNumber: number
  timestamp: number
  brightness?: number
  poolArea?: number
  poolPerimeter?: number
}

interface Props {
  metrics: DynamicMetric[]
  selectedMetric: 'brightness' | 'poolArea' | 'poolPerimeter'
  height?: string
}

const props = withDefaults(defineProps<Props>(), {
  height: '400px'
})

interface TooltipParam {
  axisValue: string
  value: number
}

// 指标配置
const metricConfig = {
  brightness: {
    name: '熔池亮度',
    unit: '灰度值',
    color: '#eab308'
  },
  poolArea: {
    name: '熔池面积',
    unit: '像素',
    color: '#10b981'
  },
  poolPerimeter: {
    name: '熔池周长',
    unit: '像素',
    color: '#f59e0b'
  }
}

// 准备图表数据
const chartData = computed(() => {
  const config = metricConfig[props.selectedMetric]

  // 提取时间和数值
  const xAxisData = props.metrics.map((m) => {
    const seconds = m.timestamp
    const mins = Math.floor(seconds / 60)
    const secs = Math.floor(seconds % 60)
    return `${mins}:${secs.toString().padStart(2, '0')}`
  })

  const seriesData = props.metrics.map((m) => {
    const value = m[props.selectedMetric]
    return value !== null && value !== undefined
      ? Number(value.toFixed(2))
      : null
  })

  // 计算统计信息
  const validData = seriesData.filter(v => v !== null) as number[]
  const average
    = validData.length > 0
      ? validData.reduce((a, b) => a + b, 0) / validData.length
      : 0

  return {
    xAxisData,
    seriesData,
    average: Number(average.toFixed(2)),
    config
  }
})

// ECharts 配置选项
const option = computed<EChartsOption>(() => {
  const { xAxisData, seriesData, average, config } = chartData.value

  return {
    title: {
      text: config.name,
      left: 'center',
      textStyle: {
        fontSize: 16,
        fontWeight: 'bold'
      }
    },
    tooltip: {
      trigger: 'axis',
      formatter: (params: unknown) => {
        const paramArray = params as TooltipParam[]
        if (!paramArray || paramArray.length === 0) return ''
        const param = paramArray[0]
        if (!param) return ''
        return `
          <div style="padding: 8px;">
            <div style="font-weight: bold; margin-bottom: 4px;">时间: ${param.axisValue}</div>
            <div style="display: flex; align-items: center; gap: 8px;">
              <span style="display: inline-block; width: 10px; height: 10px; border-radius: 50%; background: ${config.color};"></span>
              <span>${config.name}: ${param.value} ${config.unit}</span>
            </div>
          </div>
        `
      }
    },
    legend: {
      data: [config.name, '平均值'],
      top: 35
    },
    grid: {
      left: '3%',
      right: '4%',
      bottom: '15%',
      top: '80',
      containLabel: true
    },
    toolbox: {
      feature: {
        dataZoom: {
          yAxisIndex: 'none',
          title: {
            zoom: '区域缩放',
            back: '还原'
          }
        },
        restore: {
          title: '还原'
        },
        saveAsImage: {
          title: '保存为图片',
          name: `${config.name}_图表`
        }
      },
      top: 35,
      right: 20
    },
    dataZoom: [
      {
        type: 'slider',
        show: true,
        start: 0,
        end: 100,
        height: 20,
        bottom: 10
      },
      {
        type: 'inside',
        start: 0,
        end: 100
      }
    ],
    xAxis: {
      type: 'category',
      boundaryGap: false,
      data: xAxisData,
      axisLabel: {
        interval: Math.floor(xAxisData.length / 10) || 0,
        rotate: 45
      }
    },
    yAxis: {
      type: 'value',
      name: config.unit,
      axisLabel: {
        formatter: `{value} ${config.unit}`
      }
    },
    series: [
      {
        name: config.name,
        type: 'line',
        smooth: true,
        symbol: 'circle',
        symbolSize: 4,
        itemStyle: {
          color: config.color
        },
        lineStyle: {
          width: 2
        },
        areaStyle: {
          color: {
            type: 'linear',
            x: 0,
            y: 0,
            x2: 0,
            y2: 1,
            colorStops: [
              {
                offset: 0,
                color: config.color + '40' // 透明度 25%
              },
              {
                offset: 1,
                color: config.color + '10' // 透明度 6%
              }
            ]
          }
        },
        data: seriesData
      },
      {
        name: '平均值',
        type: 'line',
        markLine: {
          silent: true,
          symbol: 'none',
          lineStyle: {
            type: 'dashed',
            color: '#94a3b8',
            width: 2
          },
          label: {
            position: 'end',
            formatter: `平均: ${average} ${config.unit}`
          },
          data: [
            {
              yAxis: average
            }
          ]
        }
      }
    ]
  }
})
</script>

<template>
  <div class="metrics-chart-container">
    <VChart
      :option="option"
      :style="{ height: props.height, width: '100%' }"
      autoresize
    />
  </div>
</template>

<style scoped>
.metrics-chart-container {
  width: 100%;
  min-height: 400px;
}
</style>
