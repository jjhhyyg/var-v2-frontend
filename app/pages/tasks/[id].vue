<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import type { Task, TaskResult, TaskStatus } from '~/composables/useTaskApi'

const route = useRoute()
const { getTask, getTaskStatus, getTaskResult, reanalyzeTask } = useTaskApi()
const { connect, disconnect, subscribeToTask, subscribeToTaskDetailUpdate } = useTauriEvents()
const toast = useToast()

const taskId = route.params.id as string
const task = ref<Task>()
const status = ref<TaskStatus>()
const result = ref<TaskResult>()
const loading = ref(true)
const reanalyzing = ref(false)
let unsubscribe: (() => void) | null = null
let unsubscribeDetail: (() => void) | null = null

const selectedMetric = ref<'brightness' | 'poolArea' | 'poolPerimeter'>('poolArea')
const chartViewMode = ref<'single' | 'multi'>('multi')

const metricOptions = [
  { label: '熔池面积', value: 'poolArea' },
  { label: '熔池亮度', value: 'brightness' },
  { label: '熔池周长', value: 'poolPerimeter' }
]

const viewModeOptions = [
  { label: '综合对比', value: 'multi' },
  { label: '单项指标', value: 'single' }
]

const eventTypeMap: Record<string, string> = {
  POOL_NOT_REACHED: '熔池未到边',
  ADHESION: '电极粘连物',
  CROWN: '锭冠',
  GLOW: '辉光',
  SIDE_ARC: '边弧（侧弧）',
  CREEPING_ARC: '爬弧'
}

const progress = computed(() => (status.value?.progress ?? 0) * 100)

const sourceFps = computed(() => {
  return result.value?.videoInfo.sourceVideoFps ?? task.value?.config?.frameRate ?? 25
})

const calculatedAnalyzingDuration = computed(() => {
  if (!task.value?.startedAt || !task.value?.completedAt) {
    return null
  }
  const startTime = new Date(task.value.startedAt).getTime()
  const endTime = new Date(task.value.completedAt).getTime()
  return Math.max(0, Math.floor((endTime - startTime) / 1000))
})

const loadTask = async () => {
  try {
    task.value = await getTask(taskId)
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : '加载失败'
    toast.add({ title: '加载失败', description: errorMessage, color: 'error' })
  }
}

const loadStatus = async () => {
  try {
    status.value = await getTaskStatus(taskId)
    if (status.value.status === 'COMPLETED' || status.value.status === 'COMPLETED_TIMEOUT') {
      await loadResult()
    }
  } catch (error) {
    console.error('加载状态失败:', error)
  }
}

const loadResult = async () => {
  try {
    result.value = await getTaskResult(taskId)
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : '加载结果失败'
    toast.add({ title: '加载结果失败', description: errorMessage, color: 'error' })
  }
}

const handleStatusUpdate = async (newStatus: TaskStatus) => {
  status.value = newStatus
  if (task.value) {
    task.value.status = newStatus.status
    task.value.queuePosition = newStatus.queuePosition
  }

  if (newStatus.status === 'COMPLETED' || newStatus.status === 'COMPLETED_TIMEOUT') {
    await loadResult()
    await loadTask()
  }
}

const handleReanalyze = async () => {
  if (!confirm('确定要重新分析此任务吗？这将清除旧的分析结果并重新开始分析。')) {
    return
  }

  reanalyzing.value = true
  try {
    await reanalyzeTask(taskId)
    toast.add({ title: '操作成功', description: '任务已重新加入分析队列', color: 'success' })
    result.value = undefined
    await loadTask()
    await loadStatus()
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : '重新分析失败'
    toast.add({ title: '操作失败', description: errorMessage, color: 'error' })
  } finally {
    reanalyzing.value = false
  }
}

const getStatusColor = (
  statusStr: string
): 'error' | 'info' | 'success' | 'primary' | 'secondary' | 'warning' | 'neutral' => {
  const colors: Record<string, 'error' | 'info' | 'success' | 'primary' | 'secondary' | 'warning' | 'neutral'> = {
    PENDING: 'neutral',
    QUEUED: 'warning',
    PREPROCESSING: 'info',
    ANALYZING: 'primary',
    COMPLETED: 'success',
    COMPLETED_TIMEOUT: 'warning',
    FAILED: 'error'
  }
  return colors[statusStr] || 'neutral'
}

const getStatusText = (statusStr: string) => {
  const texts: Record<string, string> = {
    PENDING: '等待中',
    QUEUED: '排队中',
    PREPROCESSING: '预处理中',
    ANALYZING: '分析中',
    COMPLETED: '已完成',
    COMPLETED_TIMEOUT: '已完成(超时)',
    FAILED: '失败'
  }
  return texts[statusStr] || statusStr
}

const formatDuration = (seconds?: number | null) => {
  if (seconds === undefined || seconds === null) return '-'
  const safeSeconds = Math.max(0, Math.floor(seconds))
  const hours = Math.floor(safeSeconds / 3600)
  const minutes = Math.floor((safeSeconds % 3600) / 60)
  const secs = safeSeconds % 60
  if (hours > 0) return `${hours}小时${minutes}分${secs}秒`
  if (minutes > 0) return `${minutes}分${secs}秒`
  return `${secs}秒`
}

const formatDateTime = (time?: string) => {
  return time ? new Date(time).toLocaleString('zh-CN') : '-'
}

const frameToTime = (frame: number) => {
  const seconds = frame / sourceFps.value
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

const eventRows = computed(() => {
  if (!result.value) return []
  return [...result.value.anomalyEvents].sort((a, b) => a.startFrame - b.startFrame)
})

onMounted(async () => {
  loading.value = true
  await loadTask()
  await loadStatus()

  try {
    await connect()
    unsubscribe = subscribeToTask(taskId, handleStatusUpdate)
    unsubscribeDetail = subscribeToTaskDetailUpdate(taskId, async (updatedTask: Task) => {
      task.value = updatedTask
      await loadStatus()
    })
  } catch (error) {
    console.error('Tauri 事件订阅初始化失败:', error)
    toast.add({ title: '实时事件订阅失败', description: '将无法实时更新任务状态', color: 'warning' })
  }

  loading.value = false
})

onUnmounted(() => {
  unsubscribe?.()
  unsubscribeDetail?.()
  disconnect()
})
</script>

<template>
  <div class="container mx-auto max-w-7xl p-6">
    <div class="mb-6">
      <UButton to="/" icon="i-lucide-arrow-left" variant="ghost" color="neutral">
        返回任务列表
      </UButton>
    </div>

    <div v-if="loading" class="py-12 text-center">
      <UIcon name="i-lucide-loader-2" class="mx-auto mb-4 h-12 w-12 animate-spin" />
      <p class="text-muted">加载中...</p>
    </div>

    <div v-else-if="task" class="space-y-6">
      <UCard>
        <template #header>
          <div class="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
            <div>
              <h1 class="text-2xl font-bold">{{ task.name }}</h1>
              <p class="mt-1 text-sm text-muted">任务ID: {{ task.taskId }}</p>
            </div>
            <div class="flex flex-wrap items-center gap-2">
              <UBadge :color="getStatusColor(task.status)" size="lg">
                {{ getStatusText(task.status) }}
              </UBadge>
              <ReportGenerator
                v-if="(task.status === 'COMPLETED' || task.status === 'COMPLETED_TIMEOUT') && result"
                :task="task"
                :result="result"
              />
              <UButton
                v-if="task.status === 'COMPLETED' || task.status === 'COMPLETED_TIMEOUT' || task.status === 'FAILED'"
                icon="i-lucide-refresh-cw"
                color="primary"
                variant="outline"
                :loading="reanalyzing"
                @click="handleReanalyze"
              >
                重新分析
              </UButton>
            </div>
          </div>
        </template>

        <div class="grid grid-cols-1 gap-4 text-sm md:grid-cols-2 lg:grid-cols-4">
          <div>
            <p class="text-muted">视频时长</p>
            <p class="text-lg font-semibold">{{ formatDuration(task.videoDuration) }}</p>
          </div>
          <div>
            <p class="text-muted">超时阈值</p>
            <p class="text-lg font-semibold">{{ formatDuration(task.timeoutThreshold) }}</p>
          </div>
          <div>
            <p class="text-muted">视频帧率</p>
            <p class="text-lg font-semibold">{{ sourceFps }} FPS</p>
          </div>
          <div>
            <p class="text-muted">模型版本</p>
            <p class="text-lg font-semibold">{{ task.config?.modelVersion || '-' }}</p>
          </div>
          <div>
            <p class="text-muted">创建时间</p>
            <p class="font-medium">{{ formatDateTime(task.createdAt) }}</p>
          </div>
          <div>
            <p class="text-muted">开始时间</p>
            <p class="font-medium">{{ formatDateTime(task.startedAt) }}</p>
          </div>
          <div>
            <p class="text-muted">完成时间</p>
            <p class="font-medium">{{ formatDateTime(task.completedAt) }}</p>
          </div>
          <div>
            <p class="text-muted">任务耗时</p>
            <p class="font-medium">{{ formatDuration(calculatedAnalyzingDuration) }}</p>
          </div>
        </div>

        <div v-if="task.config" class="mt-4 flex flex-wrap gap-2">
          <UBadge color="neutral" variant="subtle">
            超时比例 {{ task.config.timeoutRatio }}
          </UBadge>
          <UBadge v-if="task.config.enablePreprocessing" color="primary" variant="subtle">
            预处理 {{ task.config.preprocessingStrength }}
          </UBadge>
          <UBadge v-else color="neutral" variant="subtle">
            原视频直跑
          </UBadge>
          <UBadge
            v-if="task.config.enablePreprocessing && task.config.preprocessingEnhancePool"
            color="success"
            variant="subtle"
          >
            熔池增强
          </UBadge>
        </div>

        <div v-if="task.failureReason" class="mt-4 rounded-lg bg-red-50 p-4 dark:bg-red-900/20">
          <p class="text-sm text-red-600 dark:text-red-400">
            <strong>失败原因:</strong> {{ task.failureReason }}
          </p>
        </div>
      </UCard>

      <UCard
        v-if="
          status &&
            (status.status === 'PREPROCESSING' ||
              status.status === 'ANALYZING' ||
              (status.phase === '生成结果视频' && status.currentFrame !== undefined))
        "
      >
        <template #header>
          <h2 class="text-xl font-semibold">
            {{ status.phase === '生成结果视频' ? '结果视频生成进度' : '处理进度' }}
          </h2>
        </template>

        <div class="space-y-4">
          <div>
            <div class="mb-2 flex justify-between">
              <span class="text-sm font-medium">{{ status.phase || '处理中' }}</span>
              <span class="text-sm font-medium">{{ progress.toFixed(2) }}%</span>
            </div>
            <UProgress :model-value="progress" />
          </div>

          <div class="grid grid-cols-2 gap-4 text-sm md:grid-cols-4">
            <div v-if="status.currentFrame !== undefined">
              <p class="text-muted">当前帧</p>
              <p class="font-semibold">{{ status.currentFrame }} / {{ status.totalFrames }}</p>
            </div>
            <div v-if="status.preprocessingDuration">
              <p class="text-muted">预处理耗时</p>
              <p class="font-semibold">{{ formatDuration(status.preprocessingDuration) }}</p>
            </div>
            <div v-if="status.analyzingElapsedTime">
              <p class="text-muted">分析耗时</p>
              <p class="font-semibold">{{ formatDuration(status.analyzingElapsedTime) }}</p>
            </div>
            <div v-if="status.timeoutWarning && !status.isTimeout">
              <UBadge color="warning">即将超时</UBadge>
            </div>
            <div v-if="status.isTimeout">
              <UBadge color="error">已超时</UBadge>
            </div>
          </div>
        </div>
      </UCard>

      <div v-if="result" class="grid grid-cols-1 gap-4 md:grid-cols-3">
        <UCard>
          <div class="flex items-center gap-4">
            <div class="rounded-lg bg-red-100 p-3 dark:bg-red-900/20">
              <UIcon name="i-lucide-alert-triangle" class="h-6 w-6 text-red-600 dark:text-red-400" />
            </div>
            <div>
              <p class="text-sm text-muted">异常事件总数</p>
              <p class="text-2xl font-bold">{{ result.anomalyEvents.length }}</p>
            </div>
          </div>
        </UCard>
        <UCard>
          <div class="flex items-center gap-4">
            <div class="rounded-lg bg-blue-100 p-3 dark:bg-blue-900/20">
              <UIcon name="i-lucide-video" class="h-6 w-6 text-blue-600 dark:text-blue-400" />
            </div>
            <div>
              <p class="text-sm text-muted">总帧数</p>
              <p class="text-2xl font-bold">{{ result.videoInfo.totalFrames }}</p>
            </div>
          </div>
        </UCard>
        <UCard>
          <div class="flex items-center gap-4">
            <div class="rounded-lg bg-green-100 p-3 dark:bg-green-900/20">
              <UIcon name="i-lucide-activity" class="h-6 w-6 text-green-600 dark:text-green-400" />
            </div>
            <div>
              <p class="text-sm text-muted">动态参数记录</p>
              <p class="text-2xl font-bold">{{ result.dynamicMetrics.length }}</p>
            </div>
          </div>
        </UCard>
      </div>

      <UCard v-if="result">
        <template #header>
          <h2 class="text-xl font-semibold">性能与视频信息</h2>
        </template>

        <div class="grid grid-cols-1 gap-4 text-sm md:grid-cols-2 lg:grid-cols-4">
          <div>
            <p class="text-muted">检测后端</p>
            <p class="font-semibold">{{ result.performance.detectionBackend || '-' }}</p>
          </div>
          <div>
            <p class="text-muted">检测平均 FPS</p>
            <p class="font-semibold">{{ result.performance.defectDetectionAverageFps?.toFixed(2) ?? '-' }}</p>
          </div>
          <div>
            <p class="text-muted">预处理平均 FPS</p>
            <p class="font-semibold">{{ result.performance.preprocessingAverageFps?.toFixed(2) ?? '-' }}</p>
          </div>
          <div>
            <p class="text-muted">分辨率</p>
            <p class="font-semibold">{{ result.videoInfo.width }} x {{ result.videoInfo.height }}</p>
          </div>
          <div>
            <p class="text-muted">预处理耗时</p>
            <p class="font-semibold">{{ formatDuration(result.performance.preprocessingDurationSeconds) }}</p>
          </div>
          <div>
            <p class="text-muted">缺陷检测耗时</p>
            <p class="font-semibold">{{ formatDuration(result.performance.defectDetectionDurationSeconds) }}</p>
          </div>
        </div>
      </UCard>

      <UCard v-if="result">
        <template #header>
          <h2 class="text-xl font-semibold">视频播放</h2>
        </template>

        <VideoPlayer
          :task-id="taskId"
          :video-duration="task.videoDuration"
          :frame-rate="sourceFps"
          :total-frames="result.videoInfo.totalFrames"
          :result-video-path="task.resultVideoPath"
          :preprocessed-video-path="task.preprocessedVideoPath"
          :events="result.anomalyEvents"
        />
      </UCard>

      <UCard v-if="result && result.globalAnalysis">
        <template #header>
          <h2 class="text-xl font-semibold">全局频率分析</h2>
        </template>

        <div class="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-4">
          <div
            v-if="result.globalAnalysis['闪烁']"
            class="rounded-lg border border-yellow-200 bg-yellow-50 p-4 dark:border-yellow-800 dark:bg-yellow-900/20"
          >
            <div class="mb-2 flex items-center gap-2">
              <UIcon name="i-lucide-zap" class="h-5 w-5 text-yellow-600 dark:text-yellow-400" />
              <h3 class="font-semibold">闪烁频率</h3>
            </div>
            <p class="mb-1 text-2xl font-bold">{{ result.globalAnalysis['闪烁'].frequency?.toFixed(3) }} Hz</p>
            <p class="text-sm text-muted">趋势: {{ result.globalAnalysis['闪烁'].trend || '-' }}</p>
          </div>

          <div
            v-if="result.globalAnalysis['面积']"
            class="rounded-lg border border-blue-200 bg-blue-50 p-4 dark:border-blue-800 dark:bg-blue-900/20"
          >
            <div class="mb-2 flex items-center gap-2">
              <UIcon name="i-lucide-square" class="h-5 w-5 text-blue-600 dark:text-blue-400" />
              <h3 class="font-semibold">面积振荡</h3>
            </div>
            <p class="mb-1 text-2xl font-bold">{{ result.globalAnalysis['面积'].frequency?.toFixed(3) }} Hz</p>
            <p class="text-sm text-muted">趋势: {{ result.globalAnalysis['面积'].trend || '-' }}</p>
          </div>

          <div
            v-if="result.globalAnalysis['周长']"
            class="rounded-lg border border-green-200 bg-green-50 p-4 dark:border-green-800 dark:bg-green-900/20"
          >
            <div class="mb-2 flex items-center gap-2">
              <UIcon name="i-lucide-git-commit-horizontal" class="h-5 w-5 text-green-600 dark:text-green-400" />
              <h3 class="font-semibold">周长振荡</h3>
            </div>
            <p class="mb-1 text-2xl font-bold">{{ result.globalAnalysis['周长'].frequency?.toFixed(3) }} Hz</p>
            <p class="text-sm text-muted">趋势: {{ result.globalAnalysis['周长'].trend || '-' }}</p>
          </div>
        </div>
      </UCard>

      <UCard v-if="result && result.dynamicMetrics.length > 0">
        <template #header>
          <div class="flex flex-wrap items-center justify-between gap-4">
            <h2 class="text-xl font-semibold">动态参数变化</h2>
            <div class="flex items-center gap-3">
              <USelect v-model="chartViewMode" :items="viewModeOptions" value-key="value" size="sm" />
              <USelect
                v-if="chartViewMode === 'single'"
                v-model="selectedMetric"
                :items="metricOptions"
                value-key="value"
                size="sm"
              />
            </div>
          </div>
        </template>

        <ClientOnly>
          <MultiMetricsChart v-if="chartViewMode === 'multi'" :metrics="result.dynamicMetrics" height="600px" />
          <MetricsChart v-else :metrics="result.dynamicMetrics" :selected-metric="selectedMetric" height="500px" />
          <template #fallback>
            <div class="flex h-[500px] items-center justify-center rounded-lg bg-muted/20">
              <UIcon name="i-lucide-loader-2" class="h-8 w-8 animate-spin" />
            </div>
          </template>
        </ClientOnly>
      </UCard>

      <UCard v-if="result">
        <template #header>
          <h2 class="text-xl font-semibold">事件统计</h2>
        </template>

        <div class="grid grid-cols-2 gap-4 md:grid-cols-4">
          <div
            v-for="(count, type) in result.eventStatistics"
            :key="type"
            class="rounded-lg bg-muted/30 p-4 text-center"
          >
            <p class="mb-1 text-sm text-muted">{{ eventTypeMap[type] || type }}</p>
            <p class="text-2xl font-bold">{{ count }}</p>
          </div>
        </div>
      </UCard>

      <UCard v-if="result && eventRows.length > 0">
        <template #header>
          <h2 class="text-xl font-semibold">异常事件明细</h2>
        </template>

        <div class="overflow-x-auto">
          <table class="w-full text-sm">
            <thead>
              <tr class="border-b">
                <th class="py-2 text-left">事件类型</th>
                <th class="py-2 text-left">起始帧</th>
                <th class="py-2 text-left">结束帧</th>
                <th class="py-2 text-left">时间段</th>
                <th class="py-2 text-left">证据</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="event in eventRows" :key="event.eventId" class="border-b last:border-0">
                <td class="py-2">
                  <UBadge color="info" size="sm">{{ eventTypeMap[event.eventType] || event.eventType }}</UBadge>
                </td>
                <td class="py-2">{{ event.startFrame }}</td>
                <td class="py-2">{{ event.endFrame }}</td>
                <td class="py-2">{{ frameToTime(event.startFrame) }} - {{ frameToTime(event.endFrame) }}</td>
                <td class="py-2">
                  <span v-if="event.metadata?.maxConfidence">
                    maxConf {{ Number(event.metadata.maxConfidence).toFixed(2) }}
                  </span>
                  <span v-else>-</span>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </UCard>
    </div>

    <div v-else class="py-12 text-center">
      <UIcon name="i-lucide-alert-circle" class="mx-auto mb-4 h-12 w-12 text-red-500" />
      <p class="text-muted">任务不存在</p>
    </div>
  </div>
</template>
