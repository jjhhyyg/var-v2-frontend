<script setup lang="ts">
import type { PageResult, Task, TaskSortDirection, TaskSortField, TaskStatus } from '~/composables/useTaskApi'

interface SelectedVideoItem {
  filePath: string
  originalFilename: string
  defaultTaskName: string
  taskName: string
}

const { importVideoTasks, listTasks, startAnalysis, reanalyzeTask, dequeueTask, deleteTask, getTaskStatus } = useTaskApi()
const { connect, disconnect, subscribeToTaskUpdates } = useTauriEvents()
const { pickVideoFiles, listenDragDrop } = useDesktopBridge()
const { queueRecoveryState } = useDesktopState()
const toast = useToast()

const uploading = ref(false)
const loading = ref(false)
const tasks = ref<Task[]>([])
const totalTasks = ref(0)
const totalPages = ref(0)
const currentPage = ref(0)
const pageSize = ref(20)
const selectedStatus = ref<string>()
const sortBy = ref<TaskSortField>()
const sortDirection = ref<TaskSortDirection>('desc')
const selectedVideoItems = ref<SelectedVideoItem[]>([])
let unsubscribeUpdates: (() => void) | null = null
let unsubscribeDragDrop: (() => void) | null = null
let reloadTimer: ReturnType<typeof setTimeout> | null = null
const lastKnownTaskStatus: Record<string, string> = {}

const taskStatusMap = ref<Record<string, TaskStatus>>({})
const dragDropActive = ref(false)

const uploadForm = reactive({
  timeoutRatioNumerator: 1,
  timeoutRatioDenominator: 4,
  enablePreprocessing: false,
  preprocessingStrength: 'moderate',
  preprocessingEnhancePool: false,
  enableTrackingMerge: false,
  trackingMergeStrategy: 'auto'
})

const statusOptions = [
  { label: '全部', value: undefined },
  { label: '待启动', value: 'PENDING' },
  { label: '排队中', value: 'QUEUED' },
  { label: '预处理中', value: 'PREPROCESSING' },
  { label: '分析中', value: 'ANALYZING' },
  { label: '已完成', value: 'COMPLETED' },
  { label: '已完成(超时)', value: 'COMPLETED_TIMEOUT' },
  { label: '失败', value: 'FAILED' }
]

const pageSizeOptions = [
  { label: '10 条/页', value: 10 },
  { label: '20 条/页', value: 20 },
  { label: '50 条/页', value: 50 },
  { label: '100 条/页', value: 100 }
]

const taskColumns = [
  {
    accessorKey: 'name',
    header: '任务',
    enableSorting: false,
    meta: {
      class: {
        th: 'min-w-[220px]',
        td: 'align-top'
      }
    }
  },
  {
    id: 'config',
    header: '配置',
    enableSorting: false,
    meta: {
      class: {
        th: 'min-w-[220px]',
        td: 'align-top'
      }
    }
  },
  {
    accessorKey: 'createdAt',
    header: '创建时间',
    meta: {
      class: {
        th: 'min-w-[180px]',
        td: 'align-top'
      }
    }
  },
  {
    accessorKey: 'status',
    header: '状态 / 进度',
    meta: {
      class: {
        th: 'min-w-[220px]',
        td: 'align-top'
      }
    }
  },
  {
    accessorKey: 'completedAt',
    header: '完成时间',
    meta: {
      class: {
        th: 'min-w-[180px]',
        td: 'align-top'
      }
    }
  },
  {
    id: 'actions',
    header: '操作',
    enableSorting: false,
    meta: {
      class: {
        th: 'w-[160px] text-right',
        td: 'align-top'
      }
    }
  }
]

const hasPendingQueueRecovery = computed(() => queueRecoveryState.value.hasPendingRecovery)

const paginationSummary = computed(() => {
  if (totalTasks.value === 0 || tasks.value.length === 0) {
    return '共 0 条任务'
  }

  const start = currentPage.value * pageSize.value + 1
  const end = start + tasks.value.length - 1
  return `第 ${start}-${end} 条，共 ${totalTasks.value} 条任务`
})

const getDefaultSortDirection = (field: TaskSortField): TaskSortDirection => {
  return field === 'status' ? 'asc' : 'desc'
}

const toggleTaskSort = (field: TaskSortField) => {
  const defaultDirection = getDefaultSortDirection(field)
  const oppositeDirection: TaskSortDirection = defaultDirection === 'asc' ? 'desc' : 'asc'

  if (sortBy.value !== field) {
    sortBy.value = field
    sortDirection.value = defaultDirection
    return
  }

  if (sortDirection.value === defaultDirection) {
    sortDirection.value = oppositeDirection
    return
  }

  sortBy.value = undefined
  sortDirection.value = defaultDirection
}

const getSortIcon = (field: TaskSortField) => {
  if (sortBy.value !== field) {
    return 'i-lucide-arrow-up-down'
  }

  return sortDirection.value === 'asc' ? 'i-lucide-arrow-up' : 'i-lucide-arrow-down'
}

const isProcessingStatus = (status: string) => {
  return status === 'PREPROCESSING' || status === 'ANALYZING'
}

const isFinalStatus = (status: string) => {
  return status === 'COMPLETED' || status === 'COMPLETED_TIMEOUT' || status === 'FAILED'
}

const getProcessingStatusText = (status: string) => {
  return status === 'PREPROCESSING' ? '预处理中' : '分析中'
}

const scheduleTaskReload = (delay = 350) => {
  if (reloadTimer) {
    clearTimeout(reloadTimer)
  }

  reloadTimer = setTimeout(() => {
    reloadTimer = null
    loadTasks()
  }, delay)
}

watch(
  () => uploadForm.enablePreprocessing,
  (enabled) => {
    if (!enabled) {
      uploadForm.preprocessingEnhancePool = false
    }
  }
)

const defaultTaskNameFromPath = (filePath: string) => {
  const filename = filePath.split(/[\\/]/).pop() || filePath
  const lastDotIndex = filename.lastIndexOf('.')
  return lastDotIndex > 0 ? filename.slice(0, lastDotIndex) : filename
}

const isSupportedVideoPath = (filePath: string) => {
  const extension = filePath.split('.').pop()?.toLowerCase() || ''
  return ['mp4', 'mov', 'avi', 'mkv'].includes(extension)
}

const appendSelectedVideos = (filePaths: string[]) => {
  const existing = new Set(selectedVideoItems.value.map(item => item.filePath))
  const newItems = filePaths
    .filter(filePath => !existing.has(filePath))
    .map<SelectedVideoItem>((filePath) => {
      const originalFilename = filePath.split(/[\\/]/).pop() || filePath
      const defaultTaskName = defaultTaskNameFromPath(filePath)
      return {
        filePath,
        originalFilename,
        defaultTaskName,
        taskName: defaultTaskName
      }
    })

  selectedVideoItems.value = [...selectedVideoItems.value, ...newItems]
  return newItems.length
}

const handlePickVideoFiles = async () => {
  const selected = await pickVideoFiles()
  if (selected.length === 0) {
    return
  }

  const addedCount = appendSelectedVideos(selected)
  if (addedCount === 0) {
    toast.add({
      title: '没有新增文件',
      description: '所选视频已全部存在于待创建列表中。',
      color: 'warning'
    })
  }
}

const handleDroppedVideoPaths = (filePaths: string[]) => {
  const supportedPaths = filePaths.filter(isSupportedVideoPath)
  const unsupportedCount = filePaths.length - supportedPaths.length
  const addedCount = appendSelectedVideos(supportedPaths)

  if (addedCount > 0) {
    toast.add({
      title: '已接收拖拽文件',
      description: `新增 ${addedCount} 个视频到待创建列表`,
      color: 'success'
    })
  }

  if (supportedPaths.length > 0 && addedCount === 0) {
    toast.add({
      title: '没有新增文件',
      description: '拖入的视频已全部存在于待创建列表中。',
      color: 'warning'
    })
  }

  if (unsupportedCount > 0) {
    toast.add({
      title: '存在未导入文件',
      description: `已忽略 ${unsupportedCount} 个非视频文件或不支持的路径`,
      color: 'warning'
    })
  }
}

const removeSelectedVideo = (filePath: string) => {
  selectedVideoItems.value = selectedVideoItems.value.filter(item => item.filePath !== filePath)
}

const buildTimeoutRatio = () => {
  const numerator = Number(uploadForm.timeoutRatioNumerator)
  const denominator = Number(uploadForm.timeoutRatioDenominator)

  if (!Number.isInteger(numerator) || numerator <= 0 || !Number.isInteger(denominator) || denominator <= 0) {
    return null
  }

  return `${numerator}:${denominator}`
}

const handleCreateTasks = async (autoStart: boolean) => {
  if (selectedVideoItems.value.length === 0) {
    toast.add({ title: '请先选择视频文件', color: 'error' })
    return
  }

  if (autoStart && hasPendingQueueRecovery.value) {
    toast.add({
      title: '无法直接入队',
      description: '当前存在待恢复的排队任务，请先处理恢复弹窗。',
      color: 'warning'
    })
    return
  }

  const timeoutRatio = buildTimeoutRatio()
  if (!timeoutRatio) {
    toast.add({
      title: '超时比例无效',
      description: '超时比例的两个输入框都必须是大于 0 的整数。',
      color: 'error'
    })
    return
  }

  uploading.value = true
  try {
    const result = await importVideoTasks(
      selectedVideoItems.value.map(item => ({
        filePath: item.filePath,
        name: item.taskName.trim() || item.defaultTaskName
      })),
      {
        timeoutRatio,
        enablePreprocessing: uploadForm.enablePreprocessing,
        preprocessingStrength: uploadForm.preprocessingStrength,
        preprocessingEnhancePool: uploadForm.preprocessingEnhancePool,
        enableTrackingMerge: uploadForm.enableTrackingMerge,
        trackingMergeStrategy: uploadForm.trackingMergeStrategy
      },
      autoStart
    )

    if (result.createdTasks.length > 0) {
      toast.add({
        title: autoStart ? '任务已创建并加入队列' : '任务创建成功',
        description: `成功创建 ${result.createdTasks.length} 个任务${autoStart ? `，入队 ${result.queuedTaskIds.length} 个` : ''}`,
        color: 'success'
      })
    }

    if (result.failedFiles.length > 0) {
      toast.add({
        title: '部分文件创建失败',
        description: result.failedFiles.map(item => `${item.fileName}: ${item.reason}`).join('；'),
        color: 'warning'
      })
    }

    const failedPaths = new Set(result.failedFiles.map(item => item.filePath))
    selectedVideoItems.value = selectedVideoItems.value.filter(item => failedPaths.has(item.filePath))
    await loadTasks()
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : '创建任务失败'
    toast.add({ title: autoStart ? '创建并分析失败' : '创建任务失败', description: errorMessage, color: 'error' })
  } finally {
    uploading.value = false
  }
}

const applyTaskPageResult = async (result: PageResult<Task>) => {
  tasks.value = result.items
  totalTasks.value = result.total
  totalPages.value = result.totalPages
  tasks.value.forEach((task) => {
    lastKnownTaskStatus[task.taskId] = task.status
  })

  const nextTaskStatusMap: Record<string, TaskStatus> = {}
  tasks.value.forEach((task) => {
    if (task.status === 'QUEUED') {
      nextTaskStatusMap[task.taskId] = {
        taskId: task.taskId,
        status: task.status,
        queuePosition: task.queuePosition
      }
    }
  })

  const processingTasks = tasks.value.filter(
    task => isProcessingStatus(task.status)
  )

  await Promise.all(
    processingTasks.map(async (task) => {
      try {
        nextTaskStatusMap[task.taskId] = await getTaskStatus(task.taskId)
      } catch (error) {
        console.error(`获取任务 ${task.taskId} 状态失败:`, error)
        nextTaskStatusMap[task.taskId] = {
          taskId: task.taskId,
          status: task.status,
          progress: 0
        }
      }
    })
  )

  taskStatusMap.value = nextTaskStatusMap
}

const loadTasks = async () => {
  loading.value = true
  try {
    let result: PageResult<Task> = await listTasks(
      currentPage.value,
      pageSize.value,
      selectedStatus.value,
      sortBy.value,
      sortBy.value ? sortDirection.value : undefined
    )

    if (result.totalPages > 0 && currentPage.value >= result.totalPages) {
      currentPage.value = result.totalPages - 1
      result = await listTasks(
        currentPage.value,
        pageSize.value,
        selectedStatus.value,
        sortBy.value,
        sortBy.value ? sortDirection.value : undefined
      )
    }

    await applyTaskPageResult(result)
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : '加载失败'
    toast.add({ title: '加载失败', description: errorMessage, color: 'error' })
  } finally {
    loading.value = false
  }
}

const handleStartAnalysis = async (taskId: string) => {
  try {
    await startAnalysis(taskId)
    toast.add({ title: '已加入分析队列', color: 'success' })
    await loadTasks()
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : '加入队列失败'
    toast.add({ title: '加入队列失败', description: errorMessage, color: 'error' })
  }
}

const isDeleteModalOpen = ref(false)
const taskToDelete = ref<string>('')
const isReanalyzeModalOpen = ref(false)
const taskToReanalyze = ref<Task | null>(null)
const reanalyzing = ref(false)

const confirmDelete = (taskId: string) => {
  taskToDelete.value = taskId
  isDeleteModalOpen.value = true
}

const handleDelete = async () => {
  try {
    await deleteTask(taskToDelete.value)
    toast.add({ title: '删除成功', color: 'success' })
    await loadTasks()
    isDeleteModalOpen.value = false
    taskToDelete.value = ''
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : '删除失败'
    toast.add({ title: '删除失败', description: errorMessage, color: 'error' })
  }
}

const confirmReanalyze = (task: Task) => {
  taskToReanalyze.value = task
  isReanalyzeModalOpen.value = true
}

const closeReanalyzeModal = () => {
  isReanalyzeModalOpen.value = false
  taskToReanalyze.value = null
}

const handleReanalyze = async () => {
  if (!taskToReanalyze.value) {
    return
  }

  reanalyzing.value = true
  try {
    await reanalyzeTask(taskToReanalyze.value.taskId)
    toast.add({
      title: '任务已重新加入分析队列',
      color: 'success'
    })
    closeReanalyzeModal()
    await loadTasks()
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : '重新分析失败'
    toast.add({ title: '重新分析失败', description: errorMessage, color: 'error' })
  } finally {
    reanalyzing.value = false
  }
}

const handleDequeue = async (taskId: string) => {
  try {
    await dequeueTask(taskId)
    toast.add({ title: '已移出分析队列', color: 'success' })
    await loadTasks()
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : '移出队列失败'
    toast.add({ title: '移出队列失败', description: errorMessage, color: 'error' })
  }
}

const getStatusColor = (
  status: string
): 'neutral' | 'primary' | 'secondary' | 'success' | 'info' | 'warning' | 'error' => {
  const colors: Record<string, 'neutral' | 'primary' | 'secondary' | 'success' | 'info' | 'warning' | 'error'> = {
    PENDING: 'neutral',
    QUEUED: 'warning',
    PREPROCESSING: 'info',
    ANALYZING: 'primary',
    COMPLETED: 'success',
    COMPLETED_TIMEOUT: 'warning',
    FAILED: 'error'
  }
  return colors[status] || 'neutral'
}

const getStatusText = (status: string) => {
  const texts: Record<string, string> = {
    PENDING: '待启动',
    QUEUED: '排队中',
    PREPROCESSING: '预处理中',
    ANALYZING: '分析中',
    COMPLETED: '已完成',
    COMPLETED_TIMEOUT: '已完成(超时)',
    FAILED: '失败'
  }
  return texts[status] || status
}

const formatTime = (time?: string) => {
  if (!time) return '-'
  return new Date(time).toLocaleString('zh-CN')
}

const formatDuration = (seconds: number) => {
  return `${Math.floor(seconds / 60)}分${seconds % 60}秒`
}

const getTaskProgressValue = (taskId: string) => {
  return Math.round((taskStatusMap.value[taskId]?.progress || 0) * 100)
}

watch([selectedStatus, pageSize, sortBy, sortDirection], () => {
  currentPage.value = 0
  loadTasks()
})

const handleTaskUpdate = (update: { taskId: string, status: string, progress?: number, queuePosition?: number }) => {
  const previousStatus = lastKnownTaskStatus[update.taskId]
  const statusChanged = previousStatus !== undefined && previousStatus !== update.status
  lastKnownTaskStatus[update.taskId] = update.status

  const taskIndex = tasks.value.findIndex(task => task.taskId === update.taskId)
  if (taskIndex !== -1 && tasks.value[taskIndex]) {
    tasks.value[taskIndex].status = update.status
    tasks.value[taskIndex].queuePosition = update.queuePosition

    const existingStatus = taskStatusMap.value[update.taskId]
    taskStatusMap.value[update.taskId] = {
      taskId: update.taskId,
      status: update.status,
      progress: isProcessingStatus(update.status) ? update.progress ?? existingStatus?.progress ?? 0 : undefined,
      queuePosition: update.queuePosition
    }
  }

  const affectsCurrentFilter = selectedStatus.value !== undefined
    && statusChanged
    && (previousStatus === selectedStatus.value || update.status === selectedStatus.value)
  const needsReloadForStatusSort = sortBy.value === 'status' && statusChanged
  const needsReloadForCompletedSort = sortBy.value === 'completedAt' && statusChanged && isFinalStatus(update.status)
  const needsReloadForVisibleTerminal = taskIndex !== -1 && isFinalStatus(update.status)

  if (affectsCurrentFilter || needsReloadForStatusSort || needsReloadForCompletedSort || needsReloadForVisibleTerminal) {
    scheduleTaskReload(isFinalStatus(update.status) ? 500 : 350)
  }
}

onMounted(async () => {
  try {
    await connect()
    unsubscribeUpdates = subscribeToTaskUpdates(handleTaskUpdate)
  } catch (error) {
    console.error('Tauri 事件订阅初始化失败:', error)
  }

  try {
    unsubscribeDragDrop = await listenDragDrop((event) => {
      if (event.type === 'enter' || event.type === 'over') {
        dragDropActive.value = true
        return
      }

      if (event.type === 'leave') {
        dragDropActive.value = false
        return
      }

      if (event.type === 'drop') {
        dragDropActive.value = false
        handleDroppedVideoPaths(event.paths || [])
      }
    })
  } catch (error) {
    console.error('拖拽监听初始化失败:', error)
  }

  await loadTasks()
})

onUnmounted(() => {
  if (unsubscribeUpdates) {
    unsubscribeUpdates()
  }
  if (unsubscribeDragDrop) {
    unsubscribeDragDrop()
  }
  if (reloadTimer) {
    clearTimeout(reloadTimer)
  }
  disconnect()
})

const handlePageChange = (page: number) => {
  currentPage.value = page
  loadTasks()
}
</script>

<template>
  <div class="container mx-auto max-w-7xl p-6">
    <div class="mb-8">
      <h1 class="mb-2 text-3xl font-bold">VAR熔池视频分析系统</h1>
      <p class="text-muted">批量导入视频并统一配置分析参数，支持有限并发和 FIFO 排队。</p>
    </div>

    <UCard class="mb-8">
      <template #header>
        <div class="space-y-1">
          <h2 class="text-xl font-semibold">批量创建任务</h2>
          <p class="text-sm text-muted">导入后的每个视频都会创建一个独立任务，默认任务名为文件名（不带后缀）。</p>
        </div>
      </template>

      <div class="space-y-6">
        <UFileUpload
          multiple
          variant="area"
          :interactive="false"
          :dropzone="false"
          :preview="false"
        >
          <template #default>
            <div
              role="button"
              tabindex="0"
              :class="[
                'flex min-h-40 cursor-pointer flex-col items-center justify-center rounded-lg border border-dashed px-6 py-10 text-center transition',
                dragDropActive
                  ? 'border-primary bg-primary/10 ring-2 ring-primary/30'
                  : 'border-accented hover:border-primary hover:bg-primary/5'
              ]"
              @click="handlePickVideoFiles"
              @keydown.enter.prevent="handlePickVideoFiles"
              @keydown.space.prevent="handlePickVideoFiles"
            >
              <UIcon
                name="i-lucide-files"
                class="mb-4 h-10 w-10 text-primary"
              />
              <p class="text-base font-medium">
                点击此区域选择多个视频文件，或直接拖拽到窗口中
              </p>
              <p class="mt-2 text-sm text-muted">
                当前已选择 {{ selectedVideoItems.length }} 个视频，支持 mp4 / mov / avi / mkv
              </p>
              <p
                v-if="dragDropActive"
                class="mt-3 text-sm font-medium text-primary"
              >
                松开鼠标即可加入待创建列表
              </p>
            </div>
          </template>
        </UFileUpload>

        <div
          v-if="selectedVideoItems.length > 0"
          class="space-y-3"
        >
          <div class="flex items-center justify-between">
            <h3 class="text-sm font-medium">已选视频</h3>
            <span class="text-xs text-muted">可逐项修改任务名</span>
          </div>

          <div class="space-y-3 rounded-lg border p-3">
            <div
              v-for="item in selectedVideoItems"
              :key="item.filePath"
              class="grid gap-3 rounded-lg border p-3 md:grid-cols-[minmax(0,1fr)_260px_auto]"
            >
              <div class="min-w-0 space-y-1">
                <p class="truncate font-medium">
                  {{ item.originalFilename }}
                </p>
                <p class="truncate text-xs text-muted">
                  {{ item.filePath }}
                </p>
              </div>
              <UInput
                v-model="item.taskName"
                placeholder="任务名称"
              />
              <div class="flex justify-end">
                <UButton
                  color="error"
                  variant="ghost"
                  icon="i-lucide-trash-2"
                  @click="removeSelectedVideo(item.filePath)"
                >
                  移除
                </UButton>
              </div>
            </div>
          </div>
        </div>

        <div class="grid gap-5 border-t pt-5 lg:grid-cols-2">
          <section class="space-y-4 rounded-xl border border-accented/70 bg-muted/20 p-4">
            <div class="space-y-1">
              <h3 class="text-sm font-semibold text-highlighted">基础参数</h3>
              <p class="text-xs text-muted">控制超时阈值和追踪后处理策略。</p>
            </div>

            <div class="space-y-4">
              <div class="space-y-2">
                <label class="block text-sm font-medium">超时比例</label>
                <div class="flex items-center gap-3">
                  <UInput
                    v-model.number="uploadForm.timeoutRatioNumerator"
                    type="number"
                    min="1"
                    step="1"
                    inputmode="numeric"
                  />
                  <span class="text-lg font-semibold text-muted">:</span>
                  <UInput
                    v-model.number="uploadForm.timeoutRatioDenominator"
                    type="number"
                    min="1"
                    step="1"
                    inputmode="numeric"
                  />
                </div>
              </div>

              <div class="space-y-3 rounded-lg border border-accented/60 bg-default/80 p-3">
                <div class="flex items-start justify-between gap-3">
                  <div class="space-y-1">
                    <label class="block text-sm font-medium">追踪轨迹合并</label>
                    <p class="text-xs text-muted">统一应用到本批任务，减少物体 ID 断裂。</p>
                  </div>
                  <UCheckbox v-model="uploadForm.enableTrackingMerge" />
                </div>

                <div
                  v-if="uploadForm.enableTrackingMerge"
                  class="space-y-2"
                >
                  <label class="block text-sm font-medium">合并策略</label>
                  <USelect
                    v-model="uploadForm.trackingMergeStrategy"
                    :items="[
                      { label: '自动识别 (推荐)', value: 'auto' },
                      { label: '粘连物专用', value: 'adhesion' },
                      { label: '锭冠专用', value: 'ingot_crown' },
                      { label: '保守模式', value: 'conservative' },
                      { label: '激进模式', value: 'aggressive' }
                    ]"
                    value-key="value"
                  />
                </div>
              </div>
            </div>
          </section>

          <section class="space-y-4 rounded-xl border border-accented/70 bg-muted/20 p-4">
            <div class="space-y-1">
              <h3 class="text-sm font-semibold text-highlighted">视频预处理</h3>
              <p class="text-xs text-muted">关闭时直接使用原视频，开启后才显示预处理相关参数。</p>
            </div>

            <div class="space-y-3 rounded-lg border border-accented/60 bg-default/80 p-3">
              <div class="flex items-start justify-between gap-3">
                <div class="space-y-1">
                  <label
                    class="block cursor-pointer text-sm font-medium"
                    @click="uploadForm.enablePreprocessing = !uploadForm.enablePreprocessing"
                  >
                    启用视频预处理
                  </label>
                  <p class="text-xs text-muted">预处理强度和熔池增强仅在启用后生效。</p>
                </div>
                <UCheckbox v-model="uploadForm.enablePreprocessing" />
              </div>

              <div
                v-if="uploadForm.enablePreprocessing"
                class="space-y-3 pt-2"
              >
                <div class="flex flex-wrap items-center gap-3 sm:flex-nowrap">
                  <label class="w-28 shrink-0 text-sm font-medium">预处理强度</label>
                  <div class="w-full sm:w-56">
                    <USelect
                      v-model="uploadForm.preprocessingStrength"
                      :items="[
                        { label: '轻度 (Mild)', value: 'mild' },
                        { label: '中度 (Moderate)', value: 'moderate' },
                        { label: '强度 (Strong)', value: 'strong' }
                      ]"
                      value-key="value"
                      class="w-full"
                    />
                  </div>
                </div>
                <div class="flex flex-wrap items-center gap-3 sm:flex-nowrap">
                  <label class="w-28 shrink-0 text-sm font-medium">熔池增强</label>
                  <div class="flex h-10 w-full items-center sm:w-56">
                    <UCheckbox v-model="uploadForm.preprocessingEnhancePool" />
                  </div>
                </div>
              </div>

              <p
                v-else
                class="text-xs text-muted"
              >
                当前为原视频直跑模式，不会经过预处理强度和熔池增强。
              </p>
            </div>
          </section>
        </div>

        <div class="flex flex-wrap justify-end gap-3 border-t pt-4">
          <UButton
            icon="i-lucide-file-plus-2"
            color="neutral"
            variant="outline"
            :loading="uploading"
            :disabled="selectedVideoItems.length === 0"
            @click="handleCreateTasks(false)"
          >
            创建分析任务
          </UButton>
          <UButton
            icon="i-lucide-list-start"
            :loading="uploading"
            :disabled="selectedVideoItems.length === 0 || hasPendingQueueRecovery"
            @click="handleCreateTasks(true)"
          >
            创建任务并分析
          </UButton>
        </div>
      </div>
    </UCard>

    <UCard>
      <template #header>
        <div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
          <div class="space-y-1">
            <h2 class="text-xl font-semibold">任务列表</h2>
            <p class="text-sm text-muted">{{ paginationSummary }}</p>
          </div>
          <div class="flex flex-wrap items-center gap-3">
            <USelect
              v-model="pageSize"
              :items="pageSizeOptions"
              value-key="value"
              placeholder="每页条数"
              class="w-32"
            />
            <USelect
              v-model="selectedStatus"
              :items="statusOptions"
              value-key="value"
              placeholder="筛选状态"
              class="w-36"
            />
          </div>
        </div>
      </template>

      <UTable
        :data="tasks"
        :columns="taskColumns"
        :loading="loading"
        sticky="header"
        empty="暂无任务"
        class="overflow-hidden rounded-lg border border-accented/60"
      >
        <template #createdAt-header>
          <UButton
            color="neutral"
            variant="ghost"
            size="xs"
            :trailing-icon="getSortIcon('createdAt')"
            class="-ml-2"
            @click="toggleTaskSort('createdAt')"
          >
            创建时间
          </UButton>
        </template>

        <template #status-header>
          <UButton
            color="neutral"
            variant="ghost"
            size="xs"
            :trailing-icon="getSortIcon('status')"
            class="-ml-2"
            @click="toggleTaskSort('status')"
          >
            状态 / 进度
          </UButton>
        </template>

        <template #completedAt-header>
          <UButton
            color="neutral"
            variant="ghost"
            size="xs"
            :trailing-icon="getSortIcon('completedAt')"
            class="-ml-2"
            @click="toggleTaskSort('completedAt')"
          >
            完成时间
          </UButton>
        </template>

        <template #name-cell="{ row }">
          <div class="space-y-2">
            <div class="space-y-1">
              <p class="font-medium text-highlighted">
                {{ row.original.name }}
              </p>
              <div class="flex flex-wrap items-center gap-2 text-xs text-muted">
                <span>ID {{ row.original.taskId }}</span>
                <span>视频时长 {{ formatDuration(row.original.videoDuration) }}</span>
              </div>
            </div>
            <p
              v-if="row.original.failureReason"
              class="line-clamp-2 text-xs text-error"
            >
              失败原因：{{ row.original.failureReason }}
            </p>
          </div>
        </template>

        <template #config-cell="{ row }">
          <div class="flex flex-wrap gap-2">
            <UBadge
              color="neutral"
              size="xs"
            >
              {{ row.original.config?.timeoutRatio || '-' }}
            </UBadge>
            <UBadge
              v-if="row.original.config?.enablePreprocessing"
              color="primary"
              size="xs"
            >
              预处理:{{ row.original.config.preprocessingStrength === 'mild' ? '轻度' : row.original.config.preprocessingStrength === 'moderate' ? '中度' : '强度' }}
            </UBadge>
            <UBadge
              v-else
              color="neutral"
              variant="subtle"
              size="xs"
            >
              原视频直跑
            </UBadge>
            <UBadge
              v-if="row.original.config?.enablePreprocessing && row.original.config?.preprocessingEnhancePool"
              color="success"
              size="xs"
            >
              熔池增强
            </UBadge>
            <UBadge
              v-if="row.original.config?.enableTrackingMerge"
              color="warning"
              size="xs"
            >
              轨迹合并:{{ row.original.config.trackingMergeStrategy }}
            </UBadge>
          </div>
        </template>

        <template #createdAt-cell="{ row }">
          <div class="text-sm text-muted">
            {{ formatTime(row.original.createdAt) }}
          </div>
        </template>

        <template #status-cell="{ row }">
          <div class="space-y-2">
            <div class="flex flex-wrap items-center gap-2">
              <UBadge :color="getStatusColor(row.original.status)">
                {{ getStatusText(row.original.status) }}
              </UBadge>
              <UBadge
                v-if="row.original.status === 'QUEUED' && row.original.queuePosition"
                color="warning"
                variant="subtle"
                size="xs"
              >
                队列位置 #{{ row.original.queuePosition }}
              </UBadge>
            </div>
            <div
              v-if="(row.original.status === 'PREPROCESSING' || row.original.status === 'ANALYZING') && taskStatusMap[row.original.taskId]"
              class="space-y-1"
            >
              <div class="flex items-center justify-between text-xs text-muted">
                <span>{{ getProcessingStatusText(row.original.status) }}</span>
                <span>{{ getTaskProgressValue(row.original.taskId) }}%</span>
              </div>
              <UProgress
                :model-value="getTaskProgressValue(row.original.taskId)"
                :max="100"
                :color="row.original.status === 'PREPROCESSING' ? 'info' : 'primary'"
                size="sm"
              />
            </div>
          </div>
        </template>

        <template #completedAt-cell="{ row }">
          <div class="text-sm text-muted">
            {{ formatTime(row.original.completedAt) }}
          </div>
        </template>

        <template #actions-cell="{ row }">
          <div class="flex justify-end gap-2">
            <UButton
              v-if="row.original.status === 'PENDING'"
              icon="i-lucide-list-plus"
              color="success"
              variant="ghost"
              size="sm"
              title="加入队列"
              @click="handleStartAnalysis(row.original.taskId)"
            />
            <UButton
              v-if="row.original.status === 'FAILED'"
              icon="i-lucide-refresh-cw"
              color="warning"
              variant="ghost"
              size="sm"
              title="重新分析"
              @click="confirmReanalyze(row.original)"
            />
            <UButton
              v-if="row.original.status === 'QUEUED'"
              icon="i-lucide-rotate-ccw"
              color="warning"
              variant="ghost"
              size="sm"
              title="移出队列"
              @click="handleDequeue(row.original.taskId)"
            />
            <UButton
              v-if="row.original.status === 'COMPLETED' || row.original.status === 'COMPLETED_TIMEOUT'"
              :to="`/tasks/${row.original.taskId}`"
              icon="i-lucide-bar-chart"
              color="primary"
              variant="ghost"
              size="sm"
              title="查看结果"
            />
            <UButton
              v-else
              :to="`/tasks/${row.original.taskId}`"
              icon="i-lucide-eye"
              color="neutral"
              variant="ghost"
              size="sm"
              title="查看详情"
            />
            <UTooltip
              v-if="isProcessingStatus(row.original.status)"
              text="进行中任务不能删除"
            >
              <span class="inline-flex">
                <UButton
                  icon="i-lucide-trash-2"
                  color="error"
                  variant="ghost"
                  size="sm"
                  title="进行中任务不能删除"
                  disabled
                />
              </span>
            </UTooltip>
            <UButton
              v-else
              icon="i-lucide-trash-2"
              color="error"
              variant="ghost"
              size="sm"
              title="删除任务"
              @click="confirmDelete(row.original.taskId)"
            />
          </div>
        </template>

        <template #empty>
          <div class="py-12 text-center">
            <UIcon
              name="i-lucide-inbox"
              class="mx-auto mb-4 h-12 w-12 text-muted"
            />
            <p class="text-muted">暂无任务</p>
          </div>
        </template>
      </UTable>

      <template
        v-if="totalTasks > pageSize"
        #footer
      >
        <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <p class="text-sm text-muted">
            {{ paginationSummary }}
          </p>
          <UPagination
            :page="currentPage + 1"
            :items-per-page="pageSize"
            :total="totalTasks"
            show-edges
            @update:page="handlePageChange($event - 1)"
          />
        </div>
      </template>
    </UCard>

    <UModal v-model:open="isDeleteModalOpen">
      <template #content>
        <div class="p-6">
          <h3 class="mb-4 text-lg font-semibold">确认删除任务</h3>
          <p class="mb-6 text-muted">确定要删除这个任务吗？此操作不可撤销。</p>
          <div class="flex justify-end gap-2">
            <UButton
              color="neutral"
              variant="outline"
              @click="isDeleteModalOpen = false"
            >
              取消
            </UButton>
            <UButton
              color="error"
              @click="handleDelete"
            >
              删除
            </UButton>
          </div>
        </div>
      </template>
    </UModal>

    <UModal v-model:open="isReanalyzeModalOpen">
      <template #content>
        <div class="p-6">
          <h3 class="mb-4 text-lg font-semibold">确认重新分析</h3>
          <p class="mb-6 text-muted">
            确定要重新分析“{{ taskToReanalyze?.name || '该任务' }}”吗？这会清除旧的分析结果并重新进入分析队列。
          </p>
          <div class="flex justify-end gap-2">
            <UButton
              color="neutral"
              variant="outline"
              @click="closeReanalyzeModal"
            >
              取消
            </UButton>
            <UButton
              color="warning"
              :loading="reanalyzing"
              @click="handleReanalyze"
            >
              重新分析
            </UButton>
          </div>
        </div>
      </template>
    </UModal>
  </div>
</template>
