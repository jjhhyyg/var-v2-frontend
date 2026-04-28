<script setup lang="ts">
import type { Task, TaskResult, TimingStage } from '~/composables/useTaskApi'

const { listTasks, getTaskResult } = useTaskApi()

const tasks = ref<Task[]>([])
const results = ref<Record<string, TaskResult>>({})
const selectedTaskId = ref<string>()
const loading = ref(false)
const errorMessage = ref<string>()

const completedStatuses = new Set(['COMPLETED', 'COMPLETED_TIMEOUT'])

const selectedTask = computed(() => tasks.value.find(task => task.taskId === selectedTaskId.value))
const selectedResult = computed(() => selectedTaskId.value ? results.value[selectedTaskId.value] : undefined)
const timingSummary = computed(() => selectedResult.value?.performance.timingSummary ?? null)
const timingStages = computed(() => timingSummary.value?.stages ?? [])
const hasTimingSummary = computed(() => timingStages.value.length > 0)
const preprocessingBenchmark = computed(() => selectedResult.value?.performance.preprocessingBenchmark ?? null)
const preprocessingBenchmarkStages = computed(() => {
  const benchmark = preprocessingBenchmark.value
  if (!benchmark) return []
  return [
    {
      key: 'decode',
      label: '解码',
      totalSeconds: benchmark.decodeDurationSeconds,
      averageMs: benchmark.decodeAverageMs
    },
    {
      key: 'frameProcessing',
      label: '单帧处理',
      totalSeconds: benchmark.frameProcessingDurationSeconds,
      averageMs: benchmark.frameProcessingDurationSeconds > 0 ? benchmark.frameProcessingAverageMs : null
    },
    {
      key: 'encode',
      label: '编码',
      totalSeconds: benchmark.encodeDurationSeconds,
      averageMs: benchmark.encodeAverageMs
    },
    {
      key: 'other',
      label: '其他',
      totalSeconds: benchmark.otherDurationSeconds,
      averageMs: null
    }
  ]
})

const totalStageMs = computed(() => timingStages.value.reduce((total, stage) => total + stage.totalMs, 0))
const dominantStage = computed(() => {
  return [...timingStages.value].sort((left, right) => right.totalMs - left.totalMs)[0]
})

const sortedTasks = computed(() => {
  return [...tasks.value].sort((left, right) => {
    const leftTime = left.completedAt ? Date.parse(left.completedAt) : 0
    const rightTime = right.completedAt ? Date.parse(right.completedAt) : 0
    return rightTime - leftTime
  })
})

const formatNumber = (value?: number | null, digits = 2) => {
  return typeof value === 'number' && Number.isFinite(value) ? value.toFixed(digits) : '-'
}

const formatMs = (value?: number | null) => {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return '-'
  }
  if (value >= 1000) {
    return `${(value / 1000).toFixed(2)} s`
  }
  return `${value.toFixed(2)} ms`
}

const formatSeconds = (value?: number | null) => {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return '-'
  }
  if (value >= 60) {
    const minutes = Math.floor(value / 60)
    const seconds = Math.round(value % 60)
    return `${minutes}m ${seconds}s`
  }
  return `${value}s`
}

const formatDate = (value?: string) => {
  if (!value) {
    return '-'
  }
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString()
}

const stageBarWidth = (stage: TimingStage) => {
  return `${Math.max(0, Math.min(stage.percentOfMeasuredMs, 100))}%`
}

const loadMonitorData = async () => {
  loading.value = true
  errorMessage.value = undefined

  try {
    const [completed, completedTimeout] = await Promise.all([
      listTasks(0, 100, 'COMPLETED', 'completedAt', 'desc'),
      listTasks(0, 100, 'COMPLETED_TIMEOUT', 'completedAt', 'desc')
    ])
    const nextTasks = [...completed.items, ...completedTimeout.items]
      .filter(task => completedStatuses.has(task.status))
      .sort((left, right) => {
        const leftTime = left.completedAt ? Date.parse(left.completedAt) : 0
        const rightTime = right.completedAt ? Date.parse(right.completedAt) : 0
        return rightTime - leftTime
      })

    const resultEntries = await Promise.all(
      nextTasks.map(async (task) => {
        try {
          return [task.taskId, await getTaskResult(task.taskId)] as const
        } catch {
          return [task.taskId, undefined] as const
        }
      })
    )

    const nextResults: Record<string, TaskResult> = {}
    for (const [taskId, result] of resultEntries) {
      if (result) {
        nextResults[taskId] = result
      }
    }

    tasks.value = nextTasks
    results.value = nextResults

    if (!selectedTaskId.value || !nextTasks.some(task => task.taskId === selectedTaskId.value)) {
      selectedTaskId.value = nextTasks[0]?.taskId
    }
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error)
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  loadMonitorData()
})
</script>

<template>
  <div class="mx-auto flex w-full max-w-7xl flex-col gap-5 px-4 py-6">
    <div class="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
      <div>
        <div class="mb-2 flex items-center gap-2 text-sm text-muted">
          <UIcon name="i-lucide-activity" class="h-4 w-4" />
          <span>任务性能监控</span>
        </div>
        <h1 class="text-2xl font-semibold">耗时分段</h1>
      </div>
      <div class="flex items-center gap-2">
        <UButton
          to="/"
          icon="i-lucide-arrow-left"
          color="neutral"
          variant="ghost"
        >
          返回任务
        </UButton>
        <UButton
          icon="i-lucide-refresh-cw"
          color="neutral"
          variant="outline"
          :loading="loading"
          @click="loadMonitorData"
        >
          刷新
        </UButton>
      </div>
    </div>

    <UAlert
      v-if="errorMessage"
      color="error"
      variant="soft"
      title="加载监控数据失败"
      :description="errorMessage"
    />

    <div class="grid min-h-[620px] grid-cols-1 gap-5 lg:grid-cols-[360px_minmax(0,1fr)]">
      <UCard>
        <template #header>
          <div class="flex items-center justify-between gap-3">
            <div>
              <h2 class="text-base font-semibold">已完成任务</h2>
              <p class="text-sm text-muted">{{ sortedTasks.length }} 个任务</p>
            </div>
            <UIcon name="i-lucide-list-filter" class="h-5 w-5 text-muted" />
          </div>
        </template>

        <div v-if="loading && sortedTasks.length === 0" class="space-y-3">
          <USkeleton v-for="index in 6" :key="index" class="h-16 w-full" />
        </div>
        <div v-else-if="sortedTasks.length === 0" class="py-10 text-center text-sm text-muted">
          暂无已完成任务。
        </div>
        <div v-else class="max-h-[560px] space-y-2 overflow-y-auto pr-1">
          <button
            v-for="task in sortedTasks"
            :key="task.taskId"
            type="button"
            class="w-full rounded-md border px-3 py-3 text-left transition hover:border-primary/60 hover:bg-muted/40"
            :class="task.taskId === selectedTaskId ? 'border-primary bg-primary/5' : 'border-muted'"
            @click="selectedTaskId = task.taskId"
          >
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0">
                <p class="truncate text-sm font-medium">{{ task.name }}</p>
                <p class="mt-1 text-xs text-muted">Task {{ task.taskId }} · {{ formatDate(task.completedAt) }}</p>
              </div>
              <UBadge
                size="sm"
                :color="task.status === 'COMPLETED_TIMEOUT' ? 'warning' : 'success'"
                variant="soft"
              >
                {{ task.status === 'COMPLETED_TIMEOUT' ? '超时完成' : '完成' }}
              </UBadge>
            </div>
            <div class="mt-3 grid grid-cols-2 gap-3 text-xs">
              <div>
                <p class="text-muted">后端</p>
                <p class="truncate font-medium">{{ results[task.taskId]?.performance.detectionBackend || '-' }}</p>
              </div>
              <div>
                <p class="text-muted">检测 FPS</p>
                <p class="font-medium">{{ formatNumber(results[task.taskId]?.performance.defectDetectionAverageFps) }}</p>
              </div>
            </div>
          </button>
        </div>
      </UCard>

      <div class="flex min-w-0 flex-col gap-5">
        <UCard v-if="selectedTask && selectedResult">
          <template #header>
            <div class="flex flex-col gap-2 md:flex-row md:items-start md:justify-between">
              <div class="min-w-0">
                <h2 class="truncate text-lg font-semibold">{{ selectedTask.name }}</h2>
                <p class="text-sm text-muted">Task {{ selectedTask.taskId }} · {{ selectedResult.performance.detectionBackend || '-' }}</p>
              </div>
              <UBadge color="neutral" variant="soft">
                {{ timingSummary?.schemaVersion ? `Trace v${timingSummary.schemaVersion}` : '粗粒度数据' }}
              </UBadge>
            </div>
          </template>

          <div class="grid grid-cols-2 gap-4 text-sm lg:grid-cols-6">
            <div>
              <p class="text-muted">检测 FPS</p>
              <p class="text-lg font-semibold">{{ formatNumber(selectedResult.performance.defectDetectionAverageFps) }}</p>
            </div>
            <div>
              <p class="text-muted">预处理 FPS</p>
              <p class="text-lg font-semibold">{{ formatNumber(selectedResult.performance.preprocessingAverageFps) }}</p>
            </div>
            <div>
              <p class="text-muted">检测耗时</p>
              <p class="text-lg font-semibold">{{ formatSeconds(selectedResult.performance.defectDetectionDurationSeconds) }}</p>
            </div>
            <div>
              <p class="text-muted">预处理耗时</p>
              <p class="text-lg font-semibold">{{ formatSeconds(selectedResult.performance.preprocessingDurationSeconds) }}</p>
            </div>
            <div>
              <p class="text-muted">采样帧数</p>
              <p class="text-lg font-semibold">{{ timingSummary?.totalMeasuredFrames ?? '-' }}</p>
            </div>
            <div>
              <p class="text-muted">主要耗时段</p>
              <p class="truncate text-lg font-semibold">{{ dominantStage?.label ?? '-' }}</p>
            </div>
          </div>
        </UCard>

        <UCard v-if="selectedTask && selectedResult && preprocessingBenchmark">
          <template #header>
            <div class="flex flex-col gap-2 md:flex-row md:items-start md:justify-between">
              <div>
                <h2 class="text-base font-semibold">预处理 benchmark</h2>
                <p class="text-sm text-muted">
                  {{ preprocessingBenchmark.backend }} · {{ preprocessingBenchmark.totalFrames }} 帧 · {{ formatNumber(preprocessingBenchmark.totalFps) }} FPS
                </p>
              </div>
              <UBadge color="neutral" variant="soft">
                v{{ preprocessingBenchmark.schemaVersion }}
              </UBadge>
            </div>
          </template>

          <div class="grid grid-cols-1 gap-4 text-sm md:grid-cols-4">
            <div
              v-for="stage in preprocessingBenchmarkStages"
              :key="stage.key"
              class="rounded-md border border-muted p-3"
            >
              <p class="text-muted">{{ stage.label }}</p>
              <p class="mt-1 text-lg font-semibold">{{ formatSeconds(stage.totalSeconds) }}</p>
              <p class="mt-1 text-xs text-muted">
                {{ stage.averageMs === null ? '-' : `${formatNumber(stage.averageMs, 3)} ms/帧` }}
              </p>
            </div>
          </div>
        </UCard>

        <UCard v-if="selectedTask && selectedResult">
          <template #header>
            <div class="flex items-center justify-between gap-3">
              <div>
                <h2 class="text-base font-semibold">阶段耗时</h2>
                <p class="text-sm text-muted">聚合总样本 {{ timingStages.length }} 段 · 样本总耗时 {{ formatMs(totalStageMs) }}</p>
              </div>
              <UIcon name="i-lucide-clock" class="h-5 w-5 text-muted" />
            </div>
          </template>

          <div v-if="!hasTimingSummary" class="py-12 text-center">
            <UIcon name="i-lucide-activity" class="mx-auto mb-3 h-8 w-8 text-muted" />
            <p class="font-medium">该任务没有分段 trace</p>
            <p class="mt-1 text-sm text-muted">旧任务或失败任务仅保留检测 FPS、预处理耗时和检测耗时。</p>
          </div>

          <div v-else class="overflow-x-auto">
            <table class="w-full min-w-[760px] text-sm">
              <thead class="border-b border-muted text-left text-xs text-muted">
                <tr>
                  <th class="pb-2 font-medium">阶段</th>
                  <th class="pb-2 font-medium">占比</th>
                  <th class="pb-2 text-right font-medium">总耗时</th>
                  <th class="pb-2 text-right font-medium">Avg</th>
                  <th class="pb-2 text-right font-medium">P50</th>
                  <th class="pb-2 text-right font-medium">P95</th>
                  <th class="pb-2 text-right font-medium">Max</th>
                  <th class="pb-2 text-right font-medium">样本</th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="stage in timingStages"
                  :key="stage.key"
                  class="border-b border-muted/70"
                >
                  <td class="py-3">
                    <div>
                      <p class="font-medium">{{ stage.label }}</p>
                      <p class="text-xs text-muted">{{ stage.key }}</p>
                    </div>
                  </td>
                  <td class="py-3">
                    <div class="flex min-w-[180px] items-center gap-3">
                      <div class="h-2 flex-1 rounded bg-muted">
                        <div
                          class="h-2 rounded bg-primary"
                          :style="{ width: stageBarWidth(stage) }"
                        />
                      </div>
                      <span class="w-14 text-right tabular-nums">{{ formatNumber(stage.percentOfMeasuredMs, 1) }}%</span>
                    </div>
                  </td>
                  <td class="py-3 text-right tabular-nums">{{ formatMs(stage.totalMs) }}</td>
                  <td class="py-3 text-right tabular-nums">{{ formatMs(stage.avgMs) }}</td>
                  <td class="py-3 text-right tabular-nums">{{ formatMs(stage.p50Ms) }}</td>
                  <td class="py-3 text-right tabular-nums">{{ formatMs(stage.p95Ms) }}</td>
                  <td class="py-3 text-right tabular-nums">{{ formatMs(stage.maxMs) }}</td>
                  <td class="py-3 text-right tabular-nums">{{ stage.samples }}</td>
                </tr>
              </tbody>
            </table>
          </div>
        </UCard>

        <UCard v-if="!selectedTask && !loading">
          <div class="py-16 text-center text-sm text-muted">
            选择一个已完成任务查看耗时分段。
          </div>
        </UCard>
      </div>
    </div>
  </div>
</template>
