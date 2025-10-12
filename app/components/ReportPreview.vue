<script setup lang="ts">
import type { Task, TaskResult } from '~/composables/useTaskApi'

const props = defineProps<{
  task: Task
  result: TaskResult
}>()

const { calculateAverage } = useReportGenerator()

// 从任务配置获取视频帧率，默认25
const fps = computed(() => {
  return props.task.config?.frameRate ?? 25
})

// 计算任务耗时（秒）
const taskDuration = computed(() => {
  if (!props.task.startedAt || !props.task.completedAt) return 0
  const start = new Date(props.task.startedAt).getTime()
  const end = new Date(props.task.completedAt).getTime()
  return (end - start) / 1000
})

// 格式化耗时
const formatDuration = (seconds: number): string => {
  if (seconds === 0) return '-'
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  const secs = Math.floor(seconds % 60)

  if (hours > 0) {
    return `${hours}小时${minutes}分钟${secs}秒`
  }
  if (minutes > 0) {
    return `${minutes}分钟${secs}秒`
  }
  return `${secs}秒`
}

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

// 计算趋势
const calculateTrend = (values: number[]): string => {
  if (values.length < 2) return '数据不足'

  const firstHalf = values.slice(0, Math.floor(values.length / 2))
  const secondHalf = values.slice(Math.floor(values.length / 2))

  const avgFirst = calculateAverage(firstHalf)
  const avgSecond = calculateAverage(secondHalf)

  const diff = avgSecond - avgFirst
  const changePercent = (diff / avgFirst) * 100

  if (Math.abs(changePercent) < 5) {
    return '平稳'
  } else if (diff > 0) {
    return `上升 (${changePercent.toFixed(1)}%)`
  } else {
    return `下降 (${Math.abs(changePercent).toFixed(1)}%)`
  }
}

// 各指标的变化趋势
const brightnessTrend = computed(() => {
  return calculateTrend(props.result.dynamicMetrics.map(m => m.brightness || 0))
})

const areaTrend = computed(() => {
  return calculateTrend(props.result.dynamicMetrics.map(m => m.poolArea || 0))
})

const perimeterTrend = computed(() => {
  return calculateTrend(props.result.dynamicMetrics.map(m => m.poolPerimeter || 0))
})

// 事件类型映射（对应后端 EventType 枚举）
const eventTypeMap: Record<string, string> = {
  POOL_NOT_REACHED: '熔池未到边',
  ADHESION_FORMED: '电极形成粘连物',
  ADHESION_DROPPED: '电极粘连物脱落',
  CROWN_DROPPED: '锭冠脱落',
  GLOW: '辉光',
  SIDE_ARC: '边弧',
  CREEPING_ARC: '爬弧'
}

// 格式化帧号为时间
const formatFrameToTime = (frame: number): string => {
  const seconds = frame / fps.value
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

// 按时间段分组事件
const groupedEvents = computed(() => {
  const groups: Array<{
    eventType: string
    startTime: string
    endTime: string
    duration: string
    metadata?: Record<string, unknown>
  }> = []

  props.result.anomalyEvents.forEach((event) => {
    const startTime = formatFrameToTime(event.startFrame)
    const endTime = formatFrameToTime(event.endFrame)
    const durationFrames = event.endFrame - event.startFrame
    const durationSeconds = durationFrames / fps.value
    const duration = durationSeconds.toFixed(1) + '秒'

    groups.push({
      eventType: eventTypeMap[event.eventType] || event.eventType,
      startTime,
      endTime,
      duration,
      metadata: event.metadata
    })
  })

  return groups
})
</script>

<template>
  <div class="space-y-6">
    <!-- 报告标题 -->
    <UCard>
      <template #header>
        <div class="text-center">
          <h1 class="text-2xl font-bold mb-2">熔池分析报告</h1>
          <p class="text-muted">{{ task.name }}</p>
        </div>
      </template>

      <div class="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
        <div>
          <p class="text-muted mb-1">任务名称</p>
          <p class="font-medium">{{ task.name }}</p>
        </div>
        <div>
          <p class="text-muted mb-1">视频时长</p>
          <p class="font-medium">{{ task.videoDuration.toFixed(1) }}秒</p>
        </div>
        <div>
          <p class="text-muted mb-1">视频帧率</p>
          <p class="font-medium">{{ fps }} FPS</p>
        </div>
        <div>
          <p class="text-muted mb-1">任务创建时间</p>
          <p class="font-medium">{{ new Date(task.createdAt).toLocaleString('zh-CN') }}</p>
        </div>
        <div>
          <p class="text-muted mb-1">任务开始时间</p>
          <p class="font-medium">{{ task.startedAt ? new Date(task.startedAt).toLocaleString('zh-CN') : '-' }}</p>
        </div>
        <div>
          <p class="text-muted mb-1">任务结束时间</p>
          <p class="font-medium">{{ task.completedAt ? new Date(task.completedAt).toLocaleString('zh-CN') : '-' }}</p>
        </div>
        <div>
          <p class="text-muted mb-1">任务耗时</p>
          <p class="font-medium">{{ formatDuration(taskDuration) }}</p>
        </div>
      </div>
    </UCard>

    <!-- 动态参数统计 -->
    <UCard>
      <template #header>
        <h3 class="text-lg font-semibold">动态参数统计</h3>
      </template>

      <div class="space-y-8">
        <!-- 熔池亮度 -->
        <div>
          <div class="flex justify-between items-center mb-3">
            <h4 class="text-base font-medium text-yellow-600 dark:text-yellow-400">熔池亮度</h4>
            <div class="text-sm text-muted">
              <span class="mr-4"><strong>平均值：{{ avgBrightness.toFixed(1) }}</strong> 灰度值</span>
              <span>变化趋势: <strong>{{ brightnessTrend }}</strong></span>
            </div>
          </div>
          <ClientOnly>
            <MetricsChart
              :metrics="result.dynamicMetrics"
              selected-metric="brightness"
              height="350px"
            />
            <template #fallback>
              <div class="flex items-center justify-center h-[350px] bg-muted/20 rounded-lg">
                <UIcon name="i-lucide-loader-2" class="animate-spin w-6 h-6" />
              </div>
            </template>
          </ClientOnly>
        </div>

        <!-- 熔池面积 -->
        <div>
          <div class="flex justify-between items-center mb-3">
            <h4 class="text-base font-medium text-green-600 dark:text-green-400">熔池面积</h4>
            <div class="text-sm text-muted">
              <span class="mr-4"><strong>平均值：{{ avgArea.toFixed(0) }}</strong> 像素</span>
              <span>变化趋势: <strong>{{ areaTrend }}</strong></span>
            </div>
          </div>
          <ClientOnly>
            <MetricsChart
              :metrics="result.dynamicMetrics"
              selected-metric="poolArea"
              height="350px"
            />
            <template #fallback>
              <div class="flex items-center justify-center h-[350px] bg-muted/20 rounded-lg">
                <UIcon name="i-lucide-loader-2" class="animate-spin w-6 h-6" />
              </div>
            </template>
          </ClientOnly>
        </div>

        <!-- 熔池周长 -->
        <div>
          <div class="flex justify-between items-center mb-3">
            <h4 class="text-base font-medium text-orange-600 dark:text-orange-400">熔池周长</h4>
            <div class="text-sm text-muted">
              <span class="mr-4"><strong>平均值：{{ avgPerimeter.toFixed(1) }}</strong> 像素</span>
              <span>变化趋势: <strong>{{ perimeterTrend }}</strong></span>
            </div>
          </div>
          <ClientOnly>
            <MetricsChart
              :metrics="result.dynamicMetrics"
              selected-metric="poolPerimeter"
              height="350px"
            />
            <template #fallback>
              <div class="flex items-center justify-center h-[350px] bg-muted/20 rounded-lg">
                <UIcon name="i-lucide-loader-2" class="animate-spin w-6 h-6" />
              </div>
            </template>
          </ClientOnly>
        </div>
      </div>
    </UCard>

    <!-- 异常事件统计 -->
    <UCard v-if="groupedEvents.length > 0">
      <template #header>
        <h3 class="text-lg font-semibold">异常事件统计</h3>
      </template>

      <div class="overflow-x-auto">
        <table class="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
          <thead class="bg-gray-50 dark:bg-gray-800">
            <tr>
              <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                事件类型
              </th>
              <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                开始时间
              </th>
              <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                结束时间
              </th>
              <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                持续时间
              </th>
              <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                详细信息
              </th>
            </tr>
          </thead>
          <tbody class="bg-white dark:bg-gray-900 divide-y divide-gray-200 dark:divide-gray-700">
            <tr v-for="(event, index) in groupedEvents" :key="index">
              <td class="px-4 py-3 whitespace-nowrap text-sm font-medium">
                {{ event.eventType }}
              </td>
              <td class="px-4 py-3 whitespace-nowrap text-sm text-muted">
                {{ event.startTime }}
              </td>
              <td class="px-4 py-3 whitespace-nowrap text-sm text-muted">
                {{ event.endTime }}
              </td>
              <td class="px-4 py-3 whitespace-nowrap text-sm text-muted">
                {{ event.duration }}
              </td>
              <td class="px-4 py-3 text-sm text-muted">
                <span v-if="event.eventType === '电极粘连物脱落' && event.metadata">
                  <span v-if="event.metadata.droppedIntoPool" class="text-red-600 dark:text-red-400">
                    ⚠️ 掉落进熔池
                  </span>
                  <span v-else class="text-green-600 dark:text-green-400">
                    ✓ 被结晶器捕获
                  </span>
                </span>
                <span v-else-if="event.metadata">
                  {{ JSON.stringify(event.metadata) }}
                </span>
                <span v-else>-</span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </UCard>

    <!-- 报告生成时间 -->
    <UCard>
      <div class="text-center text-sm text-muted">
        <p>报告生成时间: {{ new Date().toLocaleString('zh-CN') }}</p>
      </div>
    </UCard>
  </div>
</template>
