<script setup lang="ts">
import type { PageResult, Task, TaskStatus } from '~/composables/useTaskApi'

const { uploadTask, listTasks, startAnalysis, deleteTask } = useTaskApi()
const { connect, disconnect, subscribeToTaskUpdates, subscribeToTaskDetailUpdate, subscribeToTask } = useWebSocket()
const toast = useToast()

// 页面状态
const uploading = ref(false)
const loading = ref(false)
const tasks = ref<Task[]>([])
const totalPages = ref(0)
const currentPage = ref(0)
const selectedStatus = ref<string>()
let unsubscribeUpdates: (() => void) | null = null
const taskDetailUnsubscribers = new Map<string, () => void>()
const taskStatusUnsubscribers = new Map<string, () => void>()

// 存储每个任务的实时状态（包括进度）
const taskStatusMap = ref<Record<string, TaskStatus>>({})

// 上传表单
const uploadForm = ref({
  file: null as File | null,
  name: '',
  timeoutRatio: '1:4',
  enablePreprocessing: false,
  preprocessingStrength: 'moderate',
  preprocessingEnhancePool: true
})

// 文件选择
const fileInput = ref<HTMLInputElement>()
const selectedFileName = ref('')

const selectFile = () => {
  fileInput.value?.click()
}

const onFileChange = (event: Event) => {
  const target = event.target as HTMLInputElement
  if (target.files && target.files[0]) {
    uploadForm.value.file = target.files[0]
    selectedFileName.value = target.files[0].name
    uploadForm.value.name = target.files[0].name
  }
}

// 上传视频
const handleUpload = async () => {
  if (!uploadForm.value.file) {
    toast.add({ title: '请选择视频文件', color: 'error' })
    return
  }

  uploading.value = true
  try {
    await uploadTask(uploadForm.value.file, uploadForm.value.name, {
      timeoutRatio: uploadForm.value.timeoutRatio,
      enablePreprocessing: uploadForm.value.enablePreprocessing,
      preprocessingStrength: uploadForm.value.preprocessingStrength,
      preprocessingEnhancePool: uploadForm.value.preprocessingEnhancePool
    })

    toast.add({ title: '任务创建成功', color: 'success' })

    // 重置表单
    uploadForm.value.file = null
    uploadForm.value.name = ''
    selectedFileName.value = ''
    if (fileInput.value) fileInput.value.value = ''

    // 刷新任务列表
    await loadTasks()
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : '未知错误'
    toast.add({ title: '上传失败', description: errorMessage, color: 'error' })
  } finally {
    uploading.value = false
  }
}

// 加载任务列表
const loadTasks = async () => {
  loading.value = true
  try {
    const result: PageResult<Task> = await listTasks(currentPage.value, 20, selectedStatus.value)
    tasks.value = result.items
    totalPages.value = result.totalPages

    // 订阅每个任务的详情更新
    subscribeToTaskDetails()
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : '加载失败'
    toast.add({ title: '加载失败', description: errorMessage, color: 'error' })
  } finally {
    loading.value = false
  }
}

// 订阅所有任务的详情更新
const subscribeToTaskDetails = () => {
  // 先取消所有现有订阅
  taskDetailUnsubscribers.forEach(unsubscribe => unsubscribe())
  taskDetailUnsubscribers.clear()
  taskStatusUnsubscribers.forEach(unsubscribe => unsubscribe())
  taskStatusUnsubscribers.clear()

  // 为每个任务订阅详情更新
  tasks.value.forEach((task) => {
    // 订阅任务详情更新（结果视频路径、模型版本等）
    const unsubscribeDetail = subscribeToTaskDetailUpdate(task.taskId, (updatedTask) => {
      console.log('收到任务详情更新:', updatedTask)
      // 更新列表中的任务数据
      const taskIndex = tasks.value.findIndex(t => t.taskId === updatedTask.taskId)
      if (taskIndex !== -1) {
        tasks.value[taskIndex] = updatedTask
      }
    })
    taskDetailUnsubscribers.set(task.taskId, unsubscribeDetail)

    // 订阅任务状态更新（进度、阶段等）
    const unsubscribeStatus = subscribeToTask(task.taskId, (status) => {
      console.log('收到任务状态更新:', status)
      // 更新状态映射
      taskStatusMap.value[task.taskId] = status

      // 同时更新任务列表中的状态
      const taskIndex = tasks.value.findIndex(t => t.taskId === task.taskId)
      if (taskIndex !== -1 && tasks.value[taskIndex]) {
        tasks.value[taskIndex].status = status.status
      }
    })
    taskStatusUnsubscribers.set(task.taskId, unsubscribeStatus)
  })
}

// 开始分析任务
const handleStartAnalysis = async (taskId: string) => {
  try {
    await startAnalysis(taskId)
    toast.add({ title: '任务已启动', color: 'success' })
    await loadTasks()
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : '启动失败'
    toast.add({ title: '启动失败', description: errorMessage, color: 'error' })
  }
}

// 删除任务
const isDeleteModalOpen = ref(false)
const taskToDelete = ref<string>('')

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

// 状态筛选
const statusOptions = ref([
  { label: '全部', value: undefined },
  { label: '等待中', value: 'PENDING' },
  { label: '预处理中', value: 'PREPROCESSING' },
  { label: '分析中', value: 'ANALYZING' },
  { label: '已完成', value: 'COMPLETED' },
  { label: '已完成(超时)', value: 'COMPLETED_TIMEOUT' },
  { label: '失败', value: 'FAILED' }
])

// 状态颜色映射
const getStatusColor = (
  status: string
): 'neutral' | 'primary' | 'secondary' | 'success' | 'info' | 'warning' | 'error' => {
  const colors: Record<string, 'neutral' | 'primary' | 'secondary' | 'success' | 'info' | 'warning' | 'error'> = {
    PENDING: 'neutral',
    PREPROCESSING: 'info',
    ANALYZING: 'primary',
    COMPLETED: 'success',
    COMPLETED_TIMEOUT: 'warning',
    FAILED: 'error'
  }
  return colors[status] || 'neutral'
}

// 状态文本映射
const getStatusText = (status: string) => {
  const texts: Record<string, string> = {
    PENDING: '等待中',
    PREPROCESSING: '预处理中',
    ANALYZING: '分析中',
    COMPLETED: '已完成',
    COMPLETED_TIMEOUT: '已完成(超时)',
    FAILED: '失败'
  }
  return texts[status] || status
}

// 格式化时间
const formatTime = (time?: string) => {
  if (!time) return '-'
  return new Date(time).toLocaleString('zh-CN')
}

// 监听状态变化
watch(selectedStatus, () => {
  currentPage.value = 0
  loadTasks()
})

// WebSocket任务更新回调
const handleTaskUpdate = async (update: { taskId: string, status: string, progress?: number }) => {
  console.log('收到任务更新:', update)

  // 更新列表中对应的任务状态
  const taskIndex = tasks.value.findIndex(t => t.taskId === update.taskId)
  if (taskIndex !== -1 && tasks.value[taskIndex]) {
    tasks.value[taskIndex].status = update.status

    // 如果任务状态变为完成，重新加载任务列表以获取完整的更新信息
    if (update.status === 'COMPLETED' || update.status === 'COMPLETED_TIMEOUT' || update.status === 'FAILED') {
      // 延迟一下再重新加载，确保后端数据已经完全更新
      setTimeout(() => {
        loadTasks()
      }, 500)
    }
  }
}

// 页面加载时获取任务列表
onMounted(async () => {
  await loadTasks()

  // 连接WebSocket并订阅任务列表更新
  try {
    await connect()
    unsubscribeUpdates = subscribeToTaskUpdates(handleTaskUpdate)
    console.log('已订阅任务列表更新')
  } catch (error) {
    console.error('WebSocket连接失败:', error)
    // WebSocket连接失败不影响基本功能，只是无法实时更新
  }
})

// 清理
onUnmounted(() => {
  if (unsubscribeUpdates) {
    unsubscribeUpdates()
  }
  // 取消所有任务详情订阅
  taskDetailUnsubscribers.forEach(unsubscribe => unsubscribe())
  taskDetailUnsubscribers.clear()
  // 取消所有任务状态订阅
  taskStatusUnsubscribers.forEach(unsubscribe => unsubscribe())
  taskStatusUnsubscribers.clear()

  disconnect()
})

// 页码变化
const handlePageChange = (page: number) => {
  currentPage.value = page
  loadTasks()
}
</script>

<template>
  <div class="container mx-auto p-6 max-w-7xl">
    <!-- 页面标题 -->
    <div class="mb-8">
      <h1 class="text-3xl font-bold mb-2">VAR熔池视频分析系统</h1>
      <p class="text-muted">上传视频进行自动分析，检测异常事件和动态参数</p>
    </div>

    <!-- 上传区域 -->
    <UCard class="mb-8">
      <template #header>
        <h2 class="text-xl font-semibold">上传视频</h2>
      </template>

      <div class="space-y-4">
        <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
          <!-- 文件选择 -->
          <div class="flex flex-col">
            <label class="block text-sm font-medium mb-2"> 视频文件 </label>
            <input
              ref="fileInput"
              type="file"
              accept="video/mp4,video/avi,video/mov,video/x-matroska"
              class="hidden"
              @change="onFileChange"
            />
            <UButton icon="i-lucide-upload" color="neutral" variant="outline" block @click="selectFile">
              {{ selectedFileName || '选择视频文件' }}
            </UButton>
          </div>

          <!-- 任务名称 -->
          <div class="flex flex-col">
            <label class="block text-sm font-medium mb-2"> 任务名称 </label>
            <UInput v-model="uploadForm.name" placeholder="留空则使用文件名" />
          </div>

          <!-- 超时比例 -->
          <div class="flex flex-col">
            <label class="block text-sm font-medium mb-2"> 超时比例 </label>
            <UInput v-model="uploadForm.timeoutRatio" placeholder="例如: 1:4" />
          </div>
        </div>

        <!-- 视频预处理选项 -->
        <div class="mt-4 border-t pt-4">
          <div class="flex items-center gap-2 mb-3">
            <UCheckbox v-model="uploadForm.enablePreprocessing" />
            <label class="text-sm font-medium cursor-pointer" @click="uploadForm.enablePreprocessing = !uploadForm.enablePreprocessing">
              启用视频预处理
            </label>
          </div>

          <div v-if="uploadForm.enablePreprocessing" class="grid grid-cols-1 md:grid-cols-2 gap-4 ml-6">
            <!-- 预处理强度 -->
            <div class="flex flex-col">
              <label class="block text-sm font-medium mb-2"> 预处理强度 </label>
              <USelect
                v-model="uploadForm.preprocessingStrength"
                :items="[
                  { label: '轻度 (Mild)', value: 'mild' },
                  { label: '中度 (Moderate)', value: 'moderate' },
                  { label: '强度 (Strong)', value: 'strong' }
                ]"
                value-key="value"
              />
            </div>

            <!-- 熔池增强 -->
            <div class="flex flex-col">
              <label class="block text-sm font-medium mb-2"> 熔池增强 </label>
              <div class="flex items-center h-10">
                <UCheckbox v-model="uploadForm.preprocessingEnhancePool" />
                <label class="ml-2 text-sm">启用熔池特定增强</label>
              </div>
            </div>
          </div>
        </div>
      </div>
      <!-- 分割线 -->
      <hr class="my-4" />
      <div class="mt-4 text-right">
        <UButton icon="i-lucide-send" :loading="uploading" :disabled="!uploadForm.file" @click="handleUpload">
          创建分析任务
        </UButton>
      </div>
    </UCard>

    <!-- 任务列表 -->
    <UCard>
      <template #header>
        <div class="flex items-center justify-between">
          <h2 class="text-xl font-semibold">任务列表</h2>
          <USelect
            v-model="selectedStatus"
            :items="statusOptions"
            value-key="value"
            placeholder="筛选状态"
          />
        </div>
      </template>

      <!-- 加载状态 -->
      <div v-if="loading" class="text-center py-8">
        <UIcon name="i-lucide-loader-2" class="animate-spin w-8 h-8 mx-auto" />
      </div>

      <!-- 任务列表 -->
      <div v-else-if="tasks.length > 0" class="space-y-4">
        <div v-for="task in tasks" :key="task.taskId" class="border rounded-lg p-4 hover:bg-muted/50 transition-colors">
          <div class="flex items-start justify-between">
            <div class="flex-1">
              <div class="flex items-center gap-3 mb-2">
                <h3 class="text-lg font-semibold">
                  {{ task.name }}
                </h3>
                <UBadge :color="getStatusColor(task.status)">
                  {{ getStatusText(task.status) }}
                </UBadge>
              </div>

              <!-- 进度条 - 仅在预处理和分析阶段显示 -->
              <div
                v-if="(task.status === 'PREPROCESSING' || task.status === 'ANALYZING') && taskStatusMap[task.taskId]"
                class="mb-3"
              >
                <div class="flex items-center justify-between text-xs text-muted mb-1">
                  <span>{{ taskStatusMap[task.taskId]?.phase || '处理中' }}</span>
                  <span v-if="taskStatusMap[task.taskId]?.progress !== undefined">
                    {{ Math.round((taskStatusMap[task.taskId]?.progress || 0) * 100) }}%
                  </span>
                </div>
                <UProgress
                  :model-value="Math.round((taskStatusMap[task.taskId]?.progress || 0) * 100)"
                  :max="100"
                  :color="task.status === 'PREPROCESSING' ? 'info' : 'primary'"
                  size="sm"
                />
              </div>

              <div class="text-sm text-muted space-y-1">
                <p>创建时间: {{ formatTime(task.createdAt) }}</p>
                <p v-if="task.completedAt">完成时间: {{ formatTime(task.completedAt) }}</p>
                <p>视频时长: {{ Math.floor(task.videoDuration / 60) }}分{{ task.videoDuration % 60 }}秒</p>
                <div v-if="task.config" class="flex items-center gap-2 flex-wrap mt-2">
                  <span class="text-muted">配置:</span>
                  <UBadge color="neutral" size="xs">{{ task.config.timeoutRatio }}</UBadge>
                  <UBadge color="neutral" size="xs">{{ task.config.modelVersion || '未知' }}</UBadge>
                  <UBadge v-if="task.config.enablePreprocessing" color="primary" size="xs">
                    预处理:{{ task.config.preprocessingStrength === 'mild' ? '轻度' : task.config.preprocessingStrength === 'moderate' ? '中度' : '强度' }}
                  </UBadge>
                  <UBadge v-if="task.config.enablePreprocessing && task.config.preprocessingEnhancePool" color="success" size="xs">
                    熔池增强
                  </UBadge>
                </div>
                <p v-if="task.failureReason" class="text-red-500">失败原因: {{ task.failureReason }}</p>
              </div>
            </div>

            <div class="flex gap-2">
              <UButton
                v-if="task.status === 'PENDING'"
                icon="i-lucide-play"
                color="success"
                size="sm"
                @click="handleStartAnalysis(task.taskId)"
              >
                开始分析
              </UButton>
              <UButton
                v-if="task.status === 'COMPLETED' || task.status === 'COMPLETED_TIMEOUT'"
                :to="`/tasks/${task.taskId}`"
                icon="i-lucide-bar-chart"
                color="primary"
                size="sm"
              >
                查看结果
              </UButton>
              <UButton
                v-else
                :to="`/tasks/${task.taskId}`"
                icon="i-lucide-eye"
                color="neutral"
                variant="outline"
                size="sm"
              >
                查看详情
              </UButton>
              <UButton
                icon="i-lucide-trash-2"
                color="error"
                variant="ghost"
                size="sm"
                @click="confirmDelete(task.taskId)"
              >
                删除
              </UButton>
            </div>
          </div>
        </div>
      </div>

      <!-- 空状态 -->
      <div v-else class="text-center py-12">
        <UIcon name="i-lucide-inbox" class="w-12 h-12 mx-auto text-muted mb-4" />
        <p class="text-muted">暂无任务</p>
      </div>

      <!-- 分页 -->
      <template v-if="totalPages > 1" #footer>
        <div class="flex justify-center">
          <UPagination
            :model-value="currentPage + 1"
            :total="totalPages"
            @update:model-value="handlePageChange($event - 1)"
          />
        </div>
      </template>
    </UCard>

    <!-- 删除确认模态框 -->
    <UModal v-model:open="isDeleteModalOpen">
      <template #content>
        <div class="p-6">
          <h3 class="text-lg font-semibold mb-4">确认删除任务</h3>
          <p class="text-muted mb-6">确定要删除这个任务吗？此操作不可撤销。</p>
          <div class="flex justify-end gap-2">
            <UButton color="neutral" variant="outline" @click="isDeleteModalOpen = false"> 取消 </UButton>
            <UButton color="error" @click="handleDelete"> 删除 </UButton>
          </div>
        </div>
      </template>
    </UModal>
  </div>
</template>
