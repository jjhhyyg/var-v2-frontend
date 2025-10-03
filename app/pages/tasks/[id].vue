<script setup lang="ts">
import type { Task, TaskResult, TaskStatus } from '~/composables/useTaskApi'

const route = useRoute()
const { getTask, getTaskStatus, getTaskResult } = useTaskApi()
const toast = useToast()

const taskId = route.params.id as string
const task = ref<Task>()
const status = ref<TaskStatus>()
const result = ref<TaskResult>()
const loading = ref(true)
const pollingInterval = ref<ReturnType<typeof setInterval>>()

// progress计算属性，值为status.progress * 100
const progress = computed(() => status.value?.progress * 100 || 0)

// 事件类型映射
const eventTypeMap: Record<string, string> = {
  ADHESION_FORMED: '电极形成粘连物',
  ADHESION_DROPPED: '电极粘连物脱落',
  CROWN_DROPPED: '锭冠脱落',
  GLOW: '辉光',
  SIDE_ARC: '边弧/侧弧',
  CLIMBING_ARC: '爬弧',
  POOL_NOT_EDGE: '熔池未到边'
}

// 物体类别映射
const categoryMap: Record<string, string> = {
  POOL_NOT_EDGE: '熔池未到边',
  ADHESION: '粘连物',
  CROWN: '锭冠',
  GLOW: '辉光',
  SIDE_ARC: '边弧',
  CLIMBING_ARC: '爬弧'
}

// 加载任务详情
const loadTask = async () => {
  try {
    task.value = await getTask(taskId)
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : '加载失败'
    toast.add({ title: '加载失败', description: errorMessage, color: 'error' })
  }
}

// 加载任务状态
const loadStatus = async () => {
  try {
    status.value = await getTaskStatus(taskId)

    // 如果任务已完成，加载结果
    if (status.value.status === 'COMPLETED' || status.value.status === 'COMPLETED_TIMEOUT') {
      stopPolling()
      await loadResult()
    }
  } catch (error: any) {
    console.error('加载状态失败:', error)
  }
}

// 加载任务结果
const loadResult = async () => {
  try {
    result.value = await getTaskResult(taskId)
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : '加载结果失败'
    toast.add({ title: '加载结果失败', description: errorMessage, color: 'error' })
  }
}

// 开始轮询
const startPolling = () => {
  loadStatus()
  pollingInterval.value = setInterval(loadStatus, 2000) // 每2秒轮询一次
}

// 停止轮询
const stopPolling = () => {
  if (pollingInterval.value) {
    clearInterval(pollingInterval.value)
    pollingInterval.value = undefined
  }
}

// 状态颜色
const getStatusColor = (statusStr: string) => {
  const colors: Record<string, string> = {
    PENDING: 'neutral',
    PREPROCESSING: 'info',
    ANALYZING: 'primary',
    COMPLETED: 'success',
    COMPLETED_TIMEOUT: 'warning',
    FAILED: 'error'
  }
  return colors[statusStr] || 'neutral'
}

// 状态文本
const getStatusText = (statusStr: string) => {
  const texts: Record<string, string> = {
    PENDING: '等待中',
    PREPROCESSING: '预处理中',
    ANALYZING: '分析中',
    COMPLETED: '已完成',
    COMPLETED_TIMEOUT: '已完成(超时)',
    FAILED: '失败'
  }
  return texts[statusStr] || statusStr
}

// 格式化时间
const formatTime = (seconds?: number) => {
  if (!seconds) return '-'
  const mins = Math.floor(seconds / 60)
  const secs = seconds % 60
  return `${mins}分${secs}秒`
}

// 帧号转时间戳
const frameToTime = (frame: number, fps = 30) => {
  const seconds = frame / fps
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

// 初始化
onMounted(async () => {
  loading.value = true
  await loadTask()

  // 如果任务未完成，开始轮询
  if (task.value
    && task.value.status !== 'COMPLETED'
    && task.value.status !== 'COMPLETED_TIMEOUT'
    && task.value.status !== 'FAILED') {
    startPolling()
  } else {
    // 已完成，直接加载结果
    await loadStatus()
    if (task.value?.status === 'COMPLETED' || task.value?.status === 'COMPLETED_TIMEOUT') {
      await loadResult()
    }
  }

  loading.value = false
})

// 清理
onUnmounted(() => {
  stopPolling()
})

// 选中的指标
const selectedMetric = ref<'flickerFrequency' | 'poolArea' | 'poolPerimeter'>('poolArea')

// 指标选项
const metricOptions = [
  { label: '熔池面积', value: 'poolArea' },
  { label: '闪烁频率', value: 'flickerFrequency' },
  { label: '熔池周长', value: 'poolPerimeter' }
]

// 图表数据（简化版本，使用ASCII字符）
const chartData = computed(() => {
  if (!result.value?.dynamicMetrics)
    return []

  return result.value.dynamicMetrics.map(m => ({
    time: frameToTime(m.frameNumber),
    value: m[selectedMetric.value] || 0
  }))
})

// 统计卡片数据
const statsCards = computed(() => {
  if (!result.value)
    return []

  return [
    {
      title: '异常事件总数',
      value: result.value.anomalyEvents.length,
      icon: 'i-lucide-alert-triangle',
      color: 'error'
    },
    {
      title: '追踪物体总数',
      value: result.value.trackingObjects.length,
      icon: 'i-lucide-target',
      color: 'info'
    },
    {
      title: '动态参数记录',
      value: result.value.dynamicMetrics.length,
      icon: 'i-lucide-activity',
      color: 'success'
    }
  ]
})
</script>

<template>
  <div class="container mx-auto p-6 max-w-7xl">
    <!-- 返回按钮 -->
    <div class="mb-6">
      <UButton
        to="/"
        icon="i-lucide-arrow-left"
        variant="ghost"
        color="neutral"
      >
        返回任务列表
      </UButton>
    </div>

    <!-- 加载状态 -->
    <div
      v-if="loading"
      class="text-center py-12"
    >
      <UIcon
        name="i-lucide-loader-2"
        class="animate-spin w-12 h-12 mx-auto mb-4"
      />
      <p class="text-muted">
        加载中...
      </p>
    </div>

    <!-- 任务详情 -->
    <div
      v-else-if="task"
      class="space-y-6"
    >
      <!-- 任务信息 -->
      <UCard>
        <template #header>
          <div class="flex items-center justify-between">
            <div>
              <h1 class="text-2xl font-bold">
                {{ task.name }}
              </h1>
              <p class="text-sm text-muted mt-1">
                任务ID: {{ task.taskId }}
              </p>
            </div>
            <div class="flex items-center gap-2">
              <UBadge
                :color="getStatusColor(task.status)"
                size="lg"
              >
                {{ getStatusText(task.status) }}
              </UBadge>
              <UBadge
                v-if="task.isTimeout"
                color="warning"
                size="lg"
              >
                超时
              </UBadge>
            </div>
          </div>
        </template>

        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          <div>
            <p class="text-sm text-muted">
              视频时长
            </p>
            <p class="text-lg font-semibold">
              {{ formatTime(task.videoDuration) }}
            </p>
          </div>
          <div>
            <p class="text-sm text-muted">
              超时阈值
            </p>
            <p class="text-lg font-semibold">
              {{ formatTime(task.timeoutThreshold) }}
            </p>
          </div>
          <div>
            <p class="text-sm text-muted">
              创建时间
            </p>
            <p class="text-lg font-semibold">
              {{ new Date(task.createdAt).toLocaleString('zh-CN') }}
            </p>
          </div>
          <div v-if="task.startedAt">
            <p class="text-sm text-muted">
              开始时间
            </p>
            <p class="text-lg font-semibold">
              {{ new Date(task.startedAt).toLocaleString('zh-CN') }}
            </p>
          </div>
          <div v-if="task.completedAt">
            <p class="text-sm text-muted">
              完成时间
            </p>
            <p class="text-lg font-semibold">
              {{ new Date(task.completedAt).toLocaleString('zh-CN') }}
            </p>
          </div>
          <div v-if="task.config">
            <p class="text-sm text-muted">
              配置
            </p>
            <p class="text-sm">
              超时比例: {{ task.config.timeoutRatio }}<br>
              置信度: {{ task.config.confidenceThreshold }}<br>
              IoU: {{ task.config.iouThreshold }}
            </p>
          </div>
        </div>

        <div
          v-if="task.failureReason"
          class="mt-4 p-4 bg-red-50 dark:bg-red-900/20 rounded-lg"
        >
          <p class="text-sm text-red-600 dark:text-red-400">
            <strong>失败原因:</strong> {{ task.failureReason }}
          </p>
        </div>
      </UCard>

      <!-- 实时进度（处理中） -->
      <UCard v-if="status && (status.status === 'PREPROCESSING' || status.status === 'ANALYZING')">
        <template #header>
          <h2 class="text-xl font-semibold">
            处理进度
          </h2>
        </template>

        <div class="space-y-4">
          <div>
            <div class="flex justify-between mb-2">
              <span class="text-sm font-medium">{{ status.phase || '处理中' }}</span>
              <span class="text-sm font-medium">{{ progress || 0 }}%</span>
            </div>
            <UProgress :value="progress || 0" />
          </div>

          <div class="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
            <div v-if="status.currentFrame">
              <p class="text-muted">
                当前帧
              </p>
              <p class="font-semibold">
                {{ status.currentFrame }} / {{ status.totalFrames }}
              </p>
            </div>
            <div v-if="status.preprocessingDuration">
              <p class="text-muted">
                预处理耗时
              </p>
              <p class="font-semibold">
                {{ formatTime(status.preprocessingDuration) }}
              </p>
            </div>
            <div v-if="status.analyzingElapsedTime">
              <p class="text-muted">
                分析耗时
              </p>
              <p class="font-semibold">
                {{ formatTime(status.analyzingElapsedTime) }}
              </p>
            </div>
            <div v-if="status.timeoutWarning">
              <UBadge color="warning">
                即将超时
              </UBadge>
            </div>
          </div>
        </div>
      </UCard>

      <!-- 统计卡片（已完成） -->
      <div
        v-if="result"
        class="grid grid-cols-1 md:grid-cols-3 gap-4"
      >
        <UCard
          v-for="stat in statsCards"
          :key="stat.title"
        >
          <div class="flex items-center gap-4">
            <div :class="`p-3 rounded-lg bg-${stat.color}-100 dark:bg-${stat.color}-900/20`">
              <UIcon
                :name="stat.icon"
                :class="`w-6 h-6 text-${stat.color}-600 dark:text-${stat.color}-400`"
              />
            </div>
            <div>
              <p class="text-sm text-muted">
                {{ stat.title }}
              </p>
              <p class="text-2xl font-bold">
                {{ stat.value }}
              </p>
            </div>
          </div>
        </UCard>
      </div>

      <!-- 动态参数图表（已完成） -->
      <UCard v-if="result && result.dynamicMetrics.length > 0">
        <template #header>
          <div class="flex items-center justify-between">
            <h2 class="text-xl font-semibold">
              动态参数变化
            </h2>
            <USelect
              v-model="selectedMetric"
              :items="metricOptions"
              value-key="value"
            />
          </div>
        </template>

        <div class="space-y-4">
          <!-- 简化的数据表格 -->
          <div class="overflow-x-auto">
            <table class="w-full text-sm">
              <thead>
                <tr class="border-b">
                  <th class="text-left py-2">
                    时间
                  </th>
                  <th class="text-right py-2">
                    数值
                  </th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="(data, index) in chartData.slice(0, 10)"
                  :key="index"
                  class="border-b last:border-0"
                >
                  <td class="py-2">
                    {{ data.time }}
                  </td>
                  <td class="text-right py-2">
                    {{ data.value }}
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
          <p
            v-if="chartData.length > 10"
            class="text-sm text-muted text-center"
          >
            显示前10条，共{{ chartData.length }}条记录
          </p>
        </div>
      </UCard>

      <!-- 异常事件列表（已完成） -->
      <UCard
        v-if="result && result.anomalyEvents.length > 0"
      >
        <template #header>
          <h2 class="text-xl font-semibold">
            异常事件
          </h2>
        </template>

        <div class="space-y-2">
          <div
            v-for="event in result.anomalyEvents"
            :key="event.eventId"
            class="border rounded-lg p-4 hover:bg-muted/50 transition-colors"
          >
            <div class="flex items-start justify-between">
              <div class="flex-1">
                <div class="flex items-center gap-2 mb-2">
                  <UBadge color="error">
                    {{ eventTypeMap[event.eventType] || event.eventType }}
                  </UBadge>
                  <span class="text-sm text-muted">
                    {{ frameToTime(event.startFrame) }} - {{ frameToTime(event.endFrame) }}
                  </span>
                </div>
                <p class="text-sm">
                  帧范围: {{ event.startFrame }} - {{ event.endFrame }}
                  <span
                    v-if="event.objectId"
                    class="ml-2"
                  >
                    物体ID: {{ event.objectId }}
                  </span>
                </p>
                <div
                  v-if="event.metadata"
                  class="mt-2 text-sm text-muted"
                >
                  <pre class="bg-muted/50 p-2 rounded">{{ JSON.stringify(event.metadata, null, 2) }}</pre>
                </div>
              </div>
            </div>
          </div>
        </div>
      </UCard>

      <!-- 事件统计（已完成） -->
      <UCard v-if="result && result.eventStatistics">
        <template #header>
          <h2 class="text-xl font-semibold">
            事件统计
          </h2>
        </template>

        <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
          <div
            v-for="(count, type) in result.eventStatistics"
            :key="type"
            class="text-center p-4 bg-muted/30 rounded-lg"
          >
            <p class="text-sm text-muted mb-1">
              {{ eventTypeMap[type] || type }}
            </p>
            <p class="text-2xl font-bold">
              {{ count }}
            </p>
          </div>
        </div>
      </UCard>

      <!-- 物体统计（已完成） -->
      <UCard v-if="result && result.objectStatistics">
        <template #header>
          <h2 class="text-xl font-semibold">
            物体统计
          </h2>
        </template>

        <div class="grid grid-cols-2 md:grid-cols-3 gap-4">
          <div
            v-for="(count, category) in result.objectStatistics"
            :key="category"
            class="text-center p-4 bg-muted/30 rounded-lg"
          >
            <p class="text-sm text-muted mb-1">
              {{ categoryMap[category] || category }}
            </p>
            <p class="text-2xl font-bold">
              {{ count }}
            </p>
          </div>
        </div>
      </UCard>

      <!-- 追踪物体列表（已完成） -->
      <UCard v-if="result && result.trackingObjects.length > 0">
        <template #header>
          <h2 class="text-xl font-semibold">
            追踪物体
          </h2>
        </template>

        <div class="overflow-x-auto">
          <table class="w-full text-sm">
            <thead>
              <tr class="border-b">
                <th class="text-left py-2">
                  物体ID
                </th>
                <th class="text-left py-2">
                  类别
                </th>
                <th class="text-left py-2">
                  首帧
                </th>
                <th class="text-left py-2">
                  末帧
                </th>
                <th class="text-left py-2">
                  持续时间
                </th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="obj in result.trackingObjects"
                :key="obj.trackingId"
                class="border-b last:border-0"
              >
                <td class="py-2">
                  {{ obj.objectId }}
                </td>
                <td class="py-2">
                  <UBadge
                    color="info"
                    size="sm"
                  >
                    {{ categoryMap[obj.category] || obj.category }}
                  </UBadge>
                </td>
                <td class="py-2">
                  {{ obj.firstFrame }} ({{ frameToTime(obj.firstFrame) }})
                </td>
                <td class="py-2">
                  {{ obj.lastFrame }} ({{ frameToTime(obj.lastFrame) }})
                </td>
                <td class="py-2">
                  {{ obj.lastFrame - obj.firstFrame }} 帧
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </UCard>
    </div>

    <!-- 错误状态 -->
    <div
      v-else
      class="text-center py-12"
    >
      <UIcon
        name="i-lucide-alert-circle"
        class="w-12 h-12 mx-auto text-red-500 mb-4"
      />
      <p class="text-muted">
        任务不存在
      </p>
    </div>
  </div>
</template>
