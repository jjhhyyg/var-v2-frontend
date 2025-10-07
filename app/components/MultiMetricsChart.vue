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
  height?: string
}

const props = withDefaults(defineProps<Props>(), {
  height: '600px'
})

interface TooltipParam {
  axisValue: string
  seriesName: string
  value: number
  color: string
  marker: string
}

// 准备图表数据
const chartData = computed(() => {
  // 提取时间
  const xAxisData = props.metrics.map((m) => {
    const seconds = m.timestamp
    const mins = Math.floor(seconds / 60)
    const secs = Math.floor(seconds % 60)
    return `${mins}:${secs.toString().padStart(2, '0')}`
  })

  // 提取各项指标数据
  const brightnessData = props.metrics.map((m) => {
    const value = m.brightness
    return value !== null && value !== undefined ? Number(value.toFixed(1)) : null
  })

  const areaData = props.metrics.map((m) => {
    const value = m.poolArea
    return value !== null && value !== undefined ? Number(value.toFixed(0)) : null
  })

  const perimeterData = props.metrics.map((m) => {
    const value = m.poolPerimeter
    return value !== null && value !== undefined ? Number(value.toFixed(2)) : null
  })

  return {
    xAxisData,
    brightnessData,
    areaData,
    perimeterData
  }
})

// ECharts 配置选项 - 多轴图表
const option = computed<EChartsOption>(() => {
  const { xAxisData, brightnessData, areaData, perimeterData } = chartData.value

  return {
    title: {
      text: '动态参数综合分析',
      left: 'center',
      textStyle: {
        fontSize: 18,
        fontWeight: 'bold'
      }
    },
    tooltip: {
      trigger: 'axis',
      axisPointer: {
        type: 'cross'
      },
      formatter: (params: unknown) => {
        const paramArray = params as TooltipParam[]
        if (!paramArray || paramArray.length === 0) return ''

        let tooltip = `<div style="padding: 8px;"><div style="font-weight: bold; margin-bottom: 8px;">时间: ${paramArray[0]?.axisValue}</div>`

        paramArray.forEach((param) => {
          if (param && param.value !== null && param.value !== undefined) {
            tooltip += `
              <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 4px;">
                ${param.marker}
                <span>${param.seriesName}: ${param.value}</span>
              </div>
            `
          }
        })

        tooltip += '</div>'
        return tooltip
      }
    },
    legend: {
      data: ['熔池亮度 (灰度值)', '熔池面积 (像素)', '熔池周长 (像素)'],
      top: 40,
      selected: {
        '熔池亮度 (灰度值)': true,
        '熔池面积 (像素)': true,
        '熔池周长 (像素)': true
      }
    },
    grid: {
      left: '3%',
      right: '12%',
      bottom: '15%',
      top: '100',
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
          name: '动态参数综合图'
        }
      },
      top: 40,
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
    yAxis: [
      {
        type: 'value',
        name: '亮度 (灰度值)',
        position: 'left',
        axisLabel: {
          formatter: '{value}'
        },
        axisLine: {
          show: true,
          lineStyle: {
            color: '#eab308'
          }
        }
      },
      {
        type: 'value',
        name: '面积 (像素)',
        position: 'right',
        offset: 0,
        axisLabel: {
          formatter: '{value}'
        },
        axisLine: {
          show: true,
          lineStyle: {
            color: '#10b981'
          }
        }
      },
      {
        type: 'value',
        name: '周长 (像素)',
        position: 'right',
        offset: 60,
        axisLabel: {
          formatter: '{value}'
        },
        axisLine: {
          show: true,
          lineStyle: {
            color: '#f59e0b'
          }
        }
      }
    ],
    series: [
      {
        name: '熔池亮度 (灰度值)',
        type: 'line',
        yAxisIndex: 0,
        smooth: true,
        symbol: 'circle',
        symbolSize: 4,
        itemStyle: {
          color: '#eab308'
        },
        lineStyle: {
          width: 2
        },
        data: brightnessData
      },
      {
        name: '熔池面积 (像素)',
        type: 'line',
        yAxisIndex: 1,
        smooth: true,
        symbol: 'circle',
        symbolSize: 4,
        itemStyle: {
          color: '#10b981'
        },
        lineStyle: {
          width: 2
        },
        data: areaData
      },
      {
        name: '熔池周长 (像素)',
        type: 'line',
        yAxisIndex: 2,
        smooth: true,
        symbol: 'circle',
        symbolSize: 4,
        itemStyle: {
          color: '#f59e0b'
        },
        lineStyle: {
          width: 2
        },
        data: perimeterData
      }
    ]
  }
})
</script>

<template>
  <div class="multi-metrics-chart-container">
    <VChart
      :option="option"
      :style="{ height: props.height, width: '100%' }"
      autoresize
    />
  </div>
</template>

<style scoped>
.multi-metrics-chart-container {
  width: 100%;
  min-height: 600px;
}
</style>
