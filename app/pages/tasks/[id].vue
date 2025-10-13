<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import type { Task, TaskResult, TaskStatus, TrackingObject } from '~/composables/useTaskApi'

// 轨迹点接口
interface TrajectoryPoint {
  bbox: [number, number, number, number]
  frame: number
  confidence: number
}

// 边界框接口
interface Bounds {
  minX: number
  minY: number
  maxX: number
  maxY: number
}

const route = useRoute()
const { getTask, getTaskStatus, getTaskResult, reanalyzeTask } = useTaskApi()
const { connect, disconnect, subscribeToTask, subscribeToTaskDetailUpdate, isConnected } = useWebSocket()
const toast = useToast()

const taskId = route.params.id as string
const task = ref<Task>()
const status = ref<TaskStatus>()
const result = ref<TaskResult>()
const loading = ref(true)
const reanalyzing = ref(false)
let unsubscribe: (() => void) | null = null
let unsubscribeDetail: (() => void) | null = null

// 追踪物体分类和分页
const activeObjectTab = ref(0)
const objectTabItems = [
  { label: '已追踪物体', value: 0 },
  { label: '未追踪物体', value: 1 }
]
const objectPageSize = 10
const trackedPage = ref(1)
const untrackedPage = ref(1)

// 分类物体：ID >= 0 为已追踪，ID < 0 为未追踪
const trackedObjects = computed(() => {
  if (!result.value) return []
  return result.value.trackingObjects.filter((obj) => {
    const id = typeof obj.objectId === 'string' ? parseInt(obj.objectId) : obj.objectId
    return id >= 0
  })
})

const untrackedObjects = computed(() => {
  if (!result.value) return []
  return result.value.trackingObjects.filter((obj) => {
    const id = typeof obj.objectId === 'string' ? parseInt(obj.objectId) : obj.objectId
    return id < 0
  })
})

// 分页后的数据
const paginatedTrackedObjects = computed(() => {
  const start = (trackedPage.value - 1) * objectPageSize
  const end = start + objectPageSize
  return trackedObjects.value.slice(start, end)
})

const paginatedUntrackedObjects = computed(() => {
  const start = (untrackedPage.value - 1) * objectPageSize
  const end = start + objectPageSize
  return untrackedObjects.value.slice(start, end)
})

// 轨迹查看相关
const trajectoryModalOpen = ref(false)
const selectedObject = ref<TrackingObject | null>(null)
const trajectoryCurrentIndex = ref(0)
const trajectoryPlaying = ref(false)
const trajectoryCanvas = ref<HTMLCanvasElement | null>(null)
let trajectoryAnimationFrame: number | null = null

// 缩放和平移相关
const trajectoryScale = ref(10)
const trajectoryOffsetX = ref(0)
const trajectoryOffsetY = ref(0)
const isDragging = ref(false)
const dragStartX = ref(0)
const dragStartY = ref(0)

const currentTrajectoryFrame = computed(() => {
  if (!selectedObject.value?.trajectory?.[trajectoryCurrentIndex.value]) {
    return 0
  }
  const point = selectedObject.value.trajectory[trajectoryCurrentIndex.value]
  return point?.frame ?? 0
})

const currentTrajectoryPoint = computed(() => {
  if (!selectedObject.value?.trajectory?.[trajectoryCurrentIndex.value]) {
    return null
  }
  return selectedObject.value.trajectory[trajectoryCurrentIndex.value]
})

// 计算轨迹边界框
const getTrajectoryBounds = (trajectory: TrajectoryPoint[]): Bounds => {
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity

  for (const point of trajectory) {
    if (!point || !point.bbox) continue
    const [x1, y1, x2, y2] = point.bbox
    minX = Math.min(minX, x1, x2)
    minY = Math.min(minY, y1, y2)
    maxX = Math.max(maxX, x1, x2)
    maxY = Math.max(maxY, y1, y2)
  }

  return { minX, minY, maxX, maxY }
}

// 显示轨迹
const showTrajectory = (obj: TrackingObject) => {
  if (!obj.trajectory || obj.trajectory.length === 0) {
    toast.add({
      title: '无轨迹数据',
      description: '该物体没有轨迹数据',
      color: 'warning'
    })
    return
  }

  // 调试：打印轨迹数据
  console.log('轨迹数据:', {
    objectId: obj.objectId,
    firstFrame: obj.firstFrame,
    lastFrame: obj.lastFrame,
    totalFrames: obj.lastFrame - obj.firstFrame,
    trajectoryLength: obj.trajectory.length,
    trajectory: obj.trajectory
  })

  selectedObject.value = obj
  trajectoryCurrentIndex.value = 0
  trajectoryPlaying.value = false

  // 重置缩放和平移
  trajectoryScale.value = 10
  trajectoryOffsetX.value = 0
  trajectoryOffsetY.value = 0

  trajectoryModalOpen.value = true

  // 等待 canvas 渲染后绘制
  nextTick(() => {
    updateTrajectoryCanvas()
  })
}

// 绘制轨迹到 canvas
const updateTrajectoryCanvas = () => {
  if (!trajectoryCanvas.value || !selectedObject.value?.trajectory) return

  const canvas = trajectoryCanvas.value
  const ctx = canvas.getContext('2d')
  if (!ctx) return

  const trajectory = selectedObject.value.trajectory
  const currentIndex = trajectoryCurrentIndex.value

  // 确保索引有效
  if (currentIndex >= trajectory.length) return

  // 计算轨迹边界
  const bounds = getTrajectoryBounds(trajectory)
  const padding = 100 // 边距
  const dataWidth = bounds.maxX - bounds.minX + padding * 2
  const dataHeight = bounds.maxY - bounds.minY + padding * 2

  // Canvas 实际尺寸
  const canvasWidth = canvas.width
  const canvasHeight = canvas.height

  // 清空画布
  ctx.fillStyle = '#1a1a1a'
  ctx.fillRect(0, 0, canvasWidth, canvasHeight)

  // 保存上下文
  ctx.save()

  // 应用缩放和平移
  const scale = trajectoryScale.value
  const offsetX = trajectoryOffsetX.value
  const offsetY = trajectoryOffsetY.value

  // 计算居中偏移
  const centerOffsetX = (canvasWidth - dataWidth) / 2 - bounds.minX + padding
  const centerOffsetY = (canvasHeight - dataHeight) / 2 - bounds.minY + padding

  ctx.translate(canvasWidth / 2, canvasHeight / 2)
  ctx.scale(scale, scale)
  ctx.translate(-canvasWidth / 2 + offsetX, -canvasHeight / 2 + offsetY)
  ctx.translate(centerOffsetX, centerOffsetY)

  // 绘制网格和坐标轴
  drawGrid(ctx, bounds, scale)

  // 1. 绘制所有轨迹线（已过实线 + 未过虚线）
  for (let i = 0; i < trajectory.length - 1; i++) {
    const point1 = trajectory[i]
    const point2 = trajectory[i + 1]
    if (!point1 || !point2) continue

    const [x1, y1, x2, y2] = point1.bbox
    const [x3, y3, x4, y4] = point2.bbox
    const center1X = (x1 + x2) / 2
    const center1Y = (y1 + y2) / 2
    const center2X = (x3 + x4) / 2
    const center2Y = (y3 + y4) / 2

    ctx.beginPath()
    ctx.moveTo(center1X, center1Y)
    ctx.lineTo(center2X, center2Y)

    if (i < currentIndex) {
      // 已经过的线：实线，蓝色
      ctx.strokeStyle = 'rgba(100, 200, 255, 0.8)'
      ctx.lineWidth = 2 / scale
      ctx.setLineDash([])
    } else {
      // 未经过的线：虚线，淡蓝色
      ctx.strokeStyle = 'rgba(100, 200, 255, 0.3)'
      ctx.lineWidth = 2 / scale
      ctx.setLineDash([8 / scale, 4 / scale])
    }
    ctx.stroke()
  }

  // 重置虚线设置
  ctx.setLineDash([])

  // 2. 绘制所有轨迹点
  for (let i = 0; i < trajectory.length; i++) {
    const point = trajectory[i]
    if (!point) continue

    const [x1, y1, x2, y2] = point.bbox
    const centerX = (x1 + x2) / 2
    const centerY = (y1 + y2) / 2

    // 绘制圆点
    ctx.beginPath()
    ctx.arc(centerX, centerY, 6 / scale, 0, Math.PI * 2)

    if (i <= currentIndex) {
      // 已经过的点和当前点：实心点
      ctx.fillStyle =
        i === currentIndex
          ? 'rgba(255, 100, 100, 1)' // 当前点：红色
          : 'rgba(100, 200, 255, 0.9)' // 已过点：蓝色
      ctx.fill()
    } else {
      // 未经过的点：空心点
      ctx.strokeStyle = 'rgba(100, 200, 255, 0.5)'
      ctx.lineWidth = 2 / scale
      ctx.stroke()
    }
  }

  // 3. 绘制当前边界框（高亮）
  const currentPoint = trajectory[currentIndex]
  if (currentPoint) {
    const [x1, y1, x2, y2] = currentPoint.bbox

    ctx.strokeStyle = 'rgba(255, 100, 100, 1)'
    ctx.lineWidth = 3 / scale
    ctx.strokeRect(x1, y1, x2 - x1, y2 - y1)

    // 4. 绘制当前帧号和置信度标签
    ctx.fillStyle = 'rgba(0, 0, 0, 0.8)'
    const labelWidth = 280 / scale
    const labelHeight = 25 / scale
    ctx.fillRect(x1, y1 - 30 / scale, labelWidth, labelHeight)

    ctx.fillStyle = 'rgba(255, 255, 255, 1)'
    ctx.font = `${14 / scale}px monospace`
    ctx.fillText(`Frame: ${currentPoint.frame} | Conf: ${currentPoint.confidence.toFixed(4)}`, x1 + 5 / scale, y1 - 10 / scale)
  }

  // 恢复上下文
  ctx.restore()
}

// 绘制网格和坐标轴
const drawGrid = (ctx: CanvasRenderingContext2D, bounds: Bounds, scale: number) => {
  const { minX, minY, maxX, maxY } = bounds

  // 根据缩放级别调整网格间距
  let gridSpacing = 100 // 基础网格间距（像素）
  if (scale > 2) gridSpacing = 50
  if (scale > 4) gridSpacing = 20
  if (scale < 0.5) gridSpacing = 200

  // 绘制网格线
  ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)'
  ctx.lineWidth = 1 / scale

  // 垂直网格线
  for (let x = Math.floor(minX / gridSpacing) * gridSpacing; x <= maxX; x += gridSpacing) {
    ctx.beginPath()
    ctx.moveTo(x, minY - 50)
    ctx.lineTo(x, maxY + 50)
    ctx.stroke()
  }

  // 水平网格线
  for (let y = Math.floor(minY / gridSpacing) * gridSpacing; y <= maxY; y += gridSpacing) {
    ctx.beginPath()
    ctx.moveTo(minX - 50, y)
    ctx.lineTo(maxX + 50, y)
    ctx.stroke()
  }

  // 绘制坐标轴
  ctx.strokeStyle = 'rgba(255, 255, 255, 0.5)'
  ctx.lineWidth = 2 / scale

  // X 轴
  ctx.beginPath()
  ctx.moveTo(minX - 50, minY - 50)
  ctx.lineTo(maxX + 50, minY - 50)
  ctx.stroke()

  // Y 轴
  ctx.beginPath()
  ctx.moveTo(minX - 50, minY - 50)
  ctx.lineTo(minX - 50, maxY + 50)
  ctx.stroke()

  // 绘制刻度标签
  ctx.fillStyle = 'rgba(255, 255, 255, 0.8)'
  ctx.font = `${12 / scale}px monospace`
  ctx.textAlign = 'center'
  ctx.textBaseline = 'top'

  // X 轴刻度
  for (let x = Math.floor(minX / gridSpacing) * gridSpacing; x <= maxX; x += gridSpacing) {
    ctx.fillText(`${x}px`, x, minY - 45 / scale)
  }

  // Y 轴刻度
  ctx.textAlign = 'right'
  ctx.textBaseline = 'middle'
  for (let y = Math.floor(minY / gridSpacing) * gridSpacing; y <= maxY; y += gridSpacing) {
    ctx.fillText(`${y}px`, minX - 55 / scale, y)
  }

  // 重置文本对齐
  ctx.textAlign = 'left'
  ctx.textBaseline = 'alphabetic'
}

// 播放/暂停轨迹动画
const toggleTrajectoryPlayback = () => {
  trajectoryPlaying.value = !trajectoryPlaying.value

  if (trajectoryPlaying.value) {
    playTrajectoryAnimation()
  } else {
    if (trajectoryAnimationFrame !== null) {
      cancelAnimationFrame(trajectoryAnimationFrame)
      trajectoryAnimationFrame = null
    }
  }
}

// 播放轨迹动画
const playTrajectoryAnimation = () => {
  if (
    !trajectoryPlaying.value ||
    !selectedObject.value?.trajectory ||
    trajectoryCurrentIndex.value >= selectedObject.value.trajectory.length - 1
  ) {
    trajectoryPlaying.value = false
    return
  }

  trajectoryCurrentIndex.value++
  updateTrajectoryCanvas()

  trajectoryAnimationFrame = requestAnimationFrame(() => {
    setTimeout(playTrajectoryAnimation, 25) // ~30fps
  })
}

// 重置轨迹动画
const resetTrajectory = () => {
  trajectoryPlaying.value = false
  trajectoryCurrentIndex.value = 0
  if (trajectoryAnimationFrame !== null) {
    cancelAnimationFrame(trajectoryAnimationFrame)
    trajectoryAnimationFrame = null
  }
  updateTrajectoryCanvas()
}

// 缩放控制
const zoomIn = () => {
  trajectoryScale.value = Math.min(trajectoryScale.value * 1.2, 10)
  updateTrajectoryCanvas()
}

const zoomOut = () => {
  trajectoryScale.value = Math.max(trajectoryScale.value / 1.2, 0.1)
  updateTrajectoryCanvas()
}

const resetZoom = () => {
  trajectoryScale.value = 10
  trajectoryOffsetX.value = 0
  trajectoryOffsetY.value = 0
  updateTrajectoryCanvas()
}

// 鼠标滚轮缩放
const handleWheel = (e: WheelEvent) => {
  e.preventDefault()
  if (e.deltaY < 0) {
    zoomIn()
  } else {
    zoomOut()
  }
}

// 鼠标拖拽平移
const handleMouseDown = (e: MouseEvent) => {
  isDragging.value = true
  dragStartX.value = e.clientX - trajectoryOffsetX.value
  dragStartY.value = e.clientY - trajectoryOffsetY.value
}

const handleMouseMove = (e: MouseEvent) => {
  if (!isDragging.value) return
  trajectoryOffsetX.value = e.clientX - dragStartX.value
  trajectoryOffsetY.value = e.clientY - dragStartY.value
  updateTrajectoryCanvas()
}

const handleMouseUp = () => {
  isDragging.value = false
}

const handleMouseLeave = () => {
  isDragging.value = false
}

// 监听索引变化，更新画布
watch(trajectoryCurrentIndex, () => {
  updateTrajectoryCanvas()
})

// progress计算属性，值为status.progress * 100
const progress = computed(() => (status.value?.progress ?? 0) * 100)

// 计算分析耗时（基于开始时间和完成时间）
const calculatedAnalyzingDuration = computed(() => {
  if (!task.value?.startedAt || !task.value?.completedAt) {
    return null
  }
  const startTime = new Date(task.value.startedAt).getTime()
  const endTime = new Date(task.value.completedAt).getTime()
  const durationMs = endTime - startTime
  return Math.floor(durationMs / 1000) // 转换为秒
})

// 事件类型映射
const eventTypeMap: Record<string, string> = {
  ADHESION_FORMED: '电极形成粘连物',
  ADHESION_DROPPED: '电极粘连物脱落',
  CROWN_DROPPED: '锭冠脱落',
  GLOW: '辉光',
  SIDE_ARC: '边弧/侧弧',
  CREEPING_ARC: '爬弧',
  POOL_NOT_REACHED: '熔池未到边'
}

// 物体类别映射（对应后端ObjectCategory枚举和YOLO模型类别）
const categoryMap: Record<string, string> = {
  POOL_NOT_REACHED: '熔池未到边',
  ADHESION: '电极粘连物',
  CROWN: '锭冠',
  GLOW: '辉光',
  SIDE_ARC: '边弧（侧弧）',
  CREEPING_ARC: '爬弧'
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
      await loadResult()
    }
  } catch (error: unknown) {
    console.error('加载状态失败:', error)
  }
}

// 加载任务结果
const loadResult = async () => {
  try {
    result.value = await getTaskResult(taskId)
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : '加载结果失败'
    toast.add({
      title: '加载结果失败',
      description: errorMessage,
      color: 'error'
    })
  }
}

// WebSocket状态更新回调
const handleStatusUpdate = async (newStatus: TaskStatus) => {
  console.log('收到任务状态更新:', newStatus)
  status.value = newStatus

  // 同时更新task对象的状态字段，确保UI同步更新
  if (task.value) {
    task.value.status = newStatus.status
  }

  // 如果任务已完成，加载结果并重新加载完整任务信息
  if (newStatus.status === 'COMPLETED' || newStatus.status === 'COMPLETED_TIMEOUT') {
    await loadResult()
    // 重新加载完整任务信息以确保所有字段都是最新的
    await loadTask()
  }
}

// 状态颜色
const getStatusColor = (
  statusStr: string
): 'error' | 'info' | 'success' | 'primary' | 'secondary' | 'warning' | 'neutral' => {
  const colors: Record<string, 'error' | 'info' | 'success' | 'primary' | 'secondary' | 'warning' | 'neutral'> = {
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
const frameToTime = (frame: number) => {
  const fps = task.value?.config?.frameRate ?? 25
  const seconds = frame / fps
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

// 重新分析任务
const handleReanalyze = async () => {
  if (!confirm('确定要重新分析此任务吗？这将清除旧的分析结果并重新开始分析。')) {
    return
  }

  reanalyzing.value = true
  try {
    await reanalyzeTask(taskId)
    toast.add({
      title: '操作成功',
      description: '任务已重新开始分析',
      color: 'success'
    })

    // 清空结果并重新加载任务和状态
    result.value = undefined
    await loadTask()
    await loadStatus()
  } catch (error: unknown) {
    const errorMessage = error instanceof Error ? error.message : '重新分析失败'
    toast.add({
      title: '操作失败',
      description: errorMessage,
      color: 'error'
    })
  } finally {
    reanalyzing.value = false
  }
}

// 初始化
onMounted(async () => {
  loading.value = true
  await loadTask()

  // 加载初始状态
  await loadStatus()

  // 注意：loadStatus内部已经会在任务完成时调用loadResult()
  // 所以这里不需要再次调用，避免重复请求

  // 连接WebSocket并订阅任务状态更新
  try {
    await connect()
    unsubscribe = subscribeToTask(taskId, handleStatusUpdate)
    console.log('已订阅任务状态更新')

    // 订阅任务详情更新（如resultVideoPath更新）
    unsubscribeDetail = subscribeToTaskDetailUpdate(taskId, async (updatedTask: Task) => {
      console.log('收到任务详情更新:', updatedTask)
      // 更新task对象
      task.value = updatedTask
      // 重新加载status，以清除"生成结果视频"的进度显示
      await loadStatus()
    })
    console.log('已订阅任务详情更新')
  } catch (error) {
    console.error('WebSocket连接失败:', error)
    toast.add({
      title: 'WebSocket连接失败',
      description: '将无法实时更新任务状态',
      color: 'warning'
    })
  }

  loading.value = false
})

// 清理
onUnmounted(() => {
  if (unsubscribe) {
    unsubscribe()
  }
  if (unsubscribeDetail) {
    unsubscribeDetail()
  }
  disconnect()
})

// 选中的指标
const selectedMetric = ref<'brightness' | 'poolArea' | 'poolPerimeter'>('poolArea')

// 图表视图模式：single 单个指标，multi 综合对比
const chartViewMode = ref<'single' | 'multi'>('multi')

// 指标选项
const metricOptions = ref([
  { label: '熔池面积', value: 'poolArea' },
  { label: '熔池亮度', value: 'brightness' },
  { label: '熔池周长', value: 'poolPerimeter' }
])

// 视图模式选项
const viewModeOptions = ref([
  { label: '综合对比', value: 'multi' },
  { label: '单项指标', value: 'single' }
])

// 统计卡片数据
const statsCards = computed(() => {
  if (!result.value) return []

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
      <UButton to="/" icon="i-lucide-arrow-left" variant="ghost" color="neutral"> 返回任务列表 </UButton>
    </div>

    <!-- 加载状态 -->
    <div v-if="loading" class="text-center py-12">
      <UIcon name="i-lucide-loader-2" class="animate-spin w-12 h-12 mx-auto mb-4" />
      <p class="text-muted">加载中...</p>
    </div>

    <!-- 任务详情 -->
    <div v-else-if="task" class="space-y-6">
      <!-- 任务信息 -->
      <UCard>
        <template #header>
          <div class="flex items-center justify-between">
            <div>
                <div>
                  <h1 class="text-2xl font-bold">
                    {{ task.name }}
                  </h1>
                  <p class="text-sm text-muted mt-1">任务ID: {{ task.taskId }}</p>
                </div>
                <div>
              </div>
            </div>
            <div class="flex items-center gap-2">
              <UBadge :color="getStatusColor(task.status)" size="lg">
                {{ getStatusText(task.status) }}
              </UBadge>
              <!-- 报告生成按钮 -->
              <ReportGenerator
                v-if="(task.status === 'COMPLETED' || task.status === 'COMPLETED_TIMEOUT') && result"
                :task="task"
                :result="result"
              />
              <!-- 重新分析按钮 -->
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

        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          <div>
            <p class="text-sm text-muted">视频时长</p>
            <p class="text-lg font-semibold">
              {{ formatTime(task.videoDuration) }}
            </p>
          </div>
          <div>
            <p class="text-sm text-muted">超时阈值</p>
            <p class="text-lg font-semibold">
              {{ formatTime(task.timeoutThreshold) }}
            </p>
          </div>
          <div>
            <p class="text-sm text-muted">创建时间</p>
            <p class="text-lg font-semibold">
              {{ new Date(task.createdAt).toLocaleString('zh-CN') }}
            </p>
          </div>
          <div v-if="task.startedAt">
            <p class="text-sm text-muted">开始时间</p>
            <p class="text-lg font-semibold">
              {{ new Date(task.startedAt).toLocaleString('zh-CN') }}
            </p>
          </div>
          <div v-if="task.completedAt">
            <p class="text-sm text-muted">完成时间</p>
            <p class="text-lg font-semibold">
              {{ new Date(task.completedAt).toLocaleString('zh-CN') }}
            </p>
          </div>
          <div v-if="calculatedAnalyzingDuration">
            <p class="text-sm text-muted">分析总耗时</p>
            <p class="text-lg font-semibold">
              {{ formatTime(calculatedAnalyzingDuration) }}
            </p>
          </div>
          <div v-if="task.config" class="md:col-span-2 lg:col-span-3">
            <p class="text-sm text-muted mb-2">配置信息</p>
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-2 text-sm">
              <div class="flex items-center gap-2">
                <UIcon name="i-lucide-clock" class="w-4 h-4 text-muted" />
                <span class="text-muted">超时比例:</span>
                <UBadge color="neutral" size="sm">{{ task.config.timeoutRatio }}</UBadge>
              </div>
              <!-- <div v-if="task.config.enablePreprocessing" class="flex items-center gap-2">
                <UIcon name="i-lucide-filter" class="w-4 h-4 text-muted" />
                <span class="text-muted">视频预处理:</span>
                <UBadge color="primary" size="sm">已启用</UBadge>
              </div> -->
              <div v-if="task.config.enablePreprocessing" class="flex items-center gap-2">
                <UIcon name="i-lucide-gauge" class="w-4 h-4 text-muted" />
                <span class="text-muted">预处理强度:</span>
                <UBadge color="info" size="sm">
                  {{ task.config.preprocessingStrength === 'mild' ? '轻度' : task.config.preprocessingStrength === 'moderate' ? '中度' : '强度' }}
                </UBadge>
              </div>
              <div v-if="task.config.enablePreprocessing" class="flex items-center gap-2">
                <UIcon name="i-lucide-sparkles" class="w-4 h-4 text-muted" />
                <span class="text-muted">熔池增强:</span>
                <UBadge :color="task.config.preprocessingEnhancePool ? 'success' : 'neutral'" size="sm">
                  {{ task.config.preprocessingEnhancePool ? '已启用' : '未启用' }}
                </UBadge>
              </div>
              <div class="flex items-center gap-2">
                <UIcon name="i-lucide-git-merge" class="w-4 h-4 text-muted" />
                <span class="text-muted">追踪合并:</span>
                <UBadge :color="task.config.enableTrackingMerge ? 'success' : 'neutral'" size="sm">
                  {{ task.config.enableTrackingMerge ? '已启用' : '未启用' }}
                </UBadge>
              </div>
              <div v-if="task.config.enableTrackingMerge" class="flex items-center gap-2">
                <UIcon name="i-lucide-workflow" class="w-4 h-4 text-muted" />
                <span class="text-muted">合并策略:</span>
                <UBadge color="info" size="sm">
                  {{
                    task.config.trackingMergeStrategy === 'auto' ? '自动识别' :
                    task.config.trackingMergeStrategy === 'adhesion' ? '粘连物专用' :
                    task.config.trackingMergeStrategy === 'ingot_crown' ? '锭冠专用' :
                    task.config.trackingMergeStrategy === 'conservative' ? '保守模式' :
                    task.config.trackingMergeStrategy === 'aggressive' ? '激进模式' : '未知'
                  }}
                </UBadge>
              </div>
            </div>
          </div>
        </div>

        <div v-if="task.failureReason" class="mt-4 p-4 bg-red-50 dark:bg-red-900/20 rounded-lg">
          <p class="text-sm text-red-600 dark:text-red-400"><strong>失败原因:</strong> {{ task.failureReason }}</p>
        </div>
      </UCard>

      <!-- 实时进度（处理中或生成结果视频） -->
      <UCard
        v-if="
          status &&
          (status.status === 'PREPROCESSING' ||
            status.status === 'ANALYZING' ||
            (status.phase === '生成结果视频' && status.currentFrame))
        "
      >
        <template #header>
          <h2 class="text-xl font-semibold">
            {{ status.phase === '生成结果视频' ? '结果视频生成进度' : '处理进度' }}
          </h2>
        </template>

        <div class="space-y-4">
          <div>
            <div class="flex justify-between mb-2">
              <span class="text-sm font-medium">{{ status.phase || '处理中' }}</span>
              <span class="text-sm font-medium">{{ progress.toFixed(2) || 0 }}%</span>
            </div>
            <UProgress :model-value="progress || 0" />
          </div>

          <div class="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
            <div v-if="status.currentFrame">
              <p class="text-muted">当前帧</p>
              <p class="font-semibold">{{ status.currentFrame }} / {{ status.totalFrames }}</p>
            </div>
            <div v-if="status.preprocessingDuration">
              <p class="text-muted">预处理耗时</p>
              <p class="font-semibold">
                {{ formatTime(status.preprocessingDuration) }}
              </p>
            </div>
            <div v-if="status.analyzingElapsedTime">
              <p class="text-muted">分析耗时</p>
              <p class="font-semibold">
                {{ formatTime(status.analyzingElapsedTime) }}
              </p>
            </div>
            <div v-if="status.timeoutWarning && !status.isTimeout">
              <UBadge color="warning"> 即将超时 </UBadge>
            </div>
            <div v-if="status.isTimeout">
              <UBadge color="error"> 已超时 </UBadge>
            </div>
          </div>
        </div>
      </UCard>

      <!-- 统计卡片（已完成） -->
      <div v-if="result" class="grid grid-cols-1 md:grid-cols-3 gap-4">
        <UCard v-for="stat in statsCards" :key="stat.title">
          <div class="flex items-center gap-4">
            <div :class="`p-3 rounded-lg bg-${stat.color}-100 dark:bg-${stat.color}-900/20`">
              <UIcon :name="stat.icon" :class="`w-6 h-6 text-${stat.color}-600 dark:text-${stat.color}-400`" />
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

      <!-- 视频播放器（已完成） -->
      <UCard v-if="result">
        <template #header>
          <h2 class="text-xl font-semibold">视频播放</h2>
        </template>

        <VideoPlayer
          :task-id="taskId"
          :video-duration="task.videoDuration"
          :frame-rate="task.config?.frameRate"
          :result-video-path="task.resultVideoPath"
          :preprocessed-video-path="task.preprocessedVideoPath"
          :events="result.anomalyEvents"
          :tracking-objects="result.trackingObjects as any"
        />
      </UCard>

      <!-- 全局频率分析（已完成） -->
      <UCard v-if="result && result.globalAnalysis">
        <template #header>
          <h2 class="text-xl font-semibold">全局频率分析</h2>
        </template>

        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          <!-- 闪烁频率 -->
          <div
            v-if="result.globalAnalysis['闪烁']"
            class="p-4 bg-gradient-to-br from-yellow-50 to-yellow-100 dark:from-yellow-900/20 dark:to-yellow-800/20 rounded-lg border border-yellow-200 dark:border-yellow-800"
          >
            <div class="flex items-center gap-2 mb-2">
              <UIcon name="i-lucide-zap" class="w-5 h-5 text-yellow-600 dark:text-yellow-400" />
              <h3 class="font-semibold text-yellow-900 dark:text-yellow-100">闪烁频率</h3>
            </div>
            <p class="text-2xl font-bold text-yellow-900 dark:text-yellow-100 mb-1">
              {{ result.globalAnalysis['闪烁'].frequency?.toFixed(3) }} Hz
            </p>
            <p class="text-sm text-yellow-700 dark:text-yellow-300">
              趋势: {{ result.globalAnalysis['闪烁'].trend || '-' }}
            </p>
            <p class="text-xs text-yellow-600 dark:text-yellow-400 mt-1">
              平均亮度:
              {{ result.globalAnalysis['闪烁'].mean?.toFixed(1) || '-' }}
            </p>
          </div>

          <!-- 面积频率 -->
          <div
            v-if="result.globalAnalysis['面积']"
            class="p-4 bg-gradient-to-br from-blue-50 to-blue-100 dark:from-blue-900/20 dark:to-blue-800/20 rounded-lg border border-blue-200 dark:border-blue-800"
          >
            <div class="flex items-center gap-2 mb-2">
              <UIcon name="i-lucide-square" class="w-5 h-5 text-blue-600 dark:text-blue-400" />
              <h3 class="font-semibold text-blue-900 dark:text-blue-100">面积振荡</h3>
            </div>
            <p class="text-2xl font-bold text-blue-900 dark:text-blue-100 mb-1">
              {{ result.globalAnalysis['面积'].frequency?.toFixed(3) }} Hz
            </p>
            <p class="text-sm text-blue-700 dark:text-blue-300">
              趋势: {{ result.globalAnalysis['面积'].trend || '-' }}
            </p>
            <p class="text-xs text-blue-600 dark:text-blue-400 mt-1">
              平均值:
              {{ result.globalAnalysis['面积'].mean?.toFixed(0) || '-' }} px
            </p>
          </div>

          <!-- 周长频率 -->
          <div
            v-if="result.globalAnalysis['周长']"
            class="p-4 bg-gradient-to-br from-green-50 to-green-100 dark:from-green-900/20 dark:to-green-800/20 rounded-lg border border-green-200 dark:border-green-800"
          >
            <div class="flex items-center gap-2 mb-2">
              <UIcon name="i-lucide-git-commit-horizontal" class="w-5 h-5 text-green-600 dark:text-green-400" />
              <h3 class="font-semibold text-green-900 dark:text-green-100">周长振荡</h3>
            </div>
            <p class="text-2xl font-bold text-green-900 dark:text-green-100 mb-1">
              {{ result.globalAnalysis['周长'].frequency?.toFixed(3) }} Hz
            </p>
            <p class="text-sm text-green-700 dark:text-green-300">
              趋势: {{ result.globalAnalysis['周长'].trend || '-' }}
            </p>
            <p class="text-xs text-green-600 dark:text-green-400 mt-1">
              平均值:
              {{ result.globalAnalysis['周长'].mean?.toFixed(1) || '-' }} px
            </p>
          </div>
        </div>
      </UCard>

      <!-- 动态参数图表（已完成） -->
      <UCard v-if="result && result.dynamicMetrics.length > 0">
        <template #header>
          <div class="flex items-center justify-between gap-4 flex-wrap">
            <h2 class="text-xl font-semibold">动态参数变化</h2>
            <div class="flex items-center gap-3">
              <!-- 视图模式切换 -->
              <USelect
                v-model="chartViewMode"
                :items="viewModeOptions"
                value-key="value"
                size="sm"
              />
              <!-- 单项指标选择(仅在单项模式下显示) -->
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

        <div class="space-y-4">
          <!-- ECharts 图表 -->
          <ClientOnly>
            <!-- 综合对比视图 -->
            <MultiMetricsChart v-if="chartViewMode === 'multi'" :metrics="result.dynamicMetrics" height="600px" />
            <!-- 单项指标视图 -->
            <MetricsChart v-else :metrics="result.dynamicMetrics" :selected-metric="selectedMetric" height="500px" />
            <template #fallback>
              <div class="flex items-center justify-center h-[500px] bg-muted/20 rounded-lg">
                <UIcon name="i-lucide-loader-2" class="animate-spin w-8 h-8" />
              </div>
            </template>
          </ClientOnly>
        </div>
      </UCard>

      <!-- 事件统计（已完成） -->
      <UCard v-if="result && result.eventStatistics">
        <template #header>
          <h2 class="text-xl font-semibold">事件统计</h2>
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
          <h2 class="text-xl font-semibold">物体统计</h2>
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
          <h2 class="text-xl font-semibold">追踪物体</h2>
        </template>

        <!-- 分类标签页 -->
        <UTabs v-model="activeObjectTab" :items="objectTabItems" class="mb-4" />

        <!-- 已追踪物体表格 -->
        <div v-show="activeObjectTab === 0" class="space-y-4">
          <div class="overflow-x-auto">
            <table class="w-full text-sm">
              <thead>
                <tr class="border-b">
                  <th class="text-left py-2">物体ID</th>
                  <th class="text-left py-2">类别</th>
                  <th class="text-left py-2">首帧</th>
                  <th class="text-left py-2">末帧</th>
                  <th class="text-left py-2">持续时间</th>
                  <th class="text-center py-2">操作</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="obj in paginatedTrackedObjects" :key="obj.trackingId" class="border-b last:border-0">
                  <td class="py-2">
                    <UBadge color="success" size="sm">
                      {{ obj.objectId }}
                    </UBadge>
                  </td>
                  <td class="py-2">
                    <UBadge color="info" size="sm">
                      {{ categoryMap[obj.category] || obj.category }}
                    </UBadge>
                  </td>
                  <td class="py-2">{{ obj.firstFrame }} ({{ frameToTime(obj.firstFrame) }})</td>
                  <td class="py-2">{{ obj.lastFrame }} ({{ frameToTime(obj.lastFrame) }})</td>
                  <td class="py-2">{{ obj.lastFrame - obj.firstFrame }} 帧</td>
                  <td class="py-2 text-center">
                    <UButton
                      icon="i-lucide-route"
                      size="xs"
                      color="primary"
                      variant="soft"
                      @click="showTrajectory(obj)"
                    >
                      轨迹查看
                    </UButton>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>

          <!-- 分页 -->
          <div class="flex justify-center">
            <UPagination v-model:page="trackedPage" :total="Math.ceil(trackedObjects.length / objectPageSize)" />
          </div>
        </div>

        <!-- 未追踪物体表格 -->
        <div v-show="activeObjectTab === 1" class="space-y-4">
          <div class="overflow-x-auto">
            <table class="w-full text-sm">
              <thead>
                <tr class="border-b">
                  <th class="text-left py-2">物体ID</th>
                  <th class="text-left py-2">类别</th>
                  <th class="text-left py-2">首帧</th>
                  <th class="text-left py-2">末帧</th>
                  <th class="text-left py-2">持续时间</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="obj in paginatedUntrackedObjects" :key="obj.trackingId" class="border-b last:border-0">
                  <td class="py-2">
                    <UBadge color="neutral" size="sm">
                      {{ obj.objectId }}
                    </UBadge>
                  </td>
                  <td class="py-2">
                    <UBadge color="info" size="sm">
                      {{ categoryMap[obj.category] || obj.category }}
                    </UBadge>
                  </td>
                  <td class="py-2">{{ obj.firstFrame }} ({{ frameToTime(obj.firstFrame) }})</td>
                  <td class="py-2">{{ obj.lastFrame }} ({{ frameToTime(obj.lastFrame) }})</td>
                  <td class="py-2">{{ obj.lastFrame - obj.firstFrame }} 帧</td>
                </tr>
              </tbody>
            </table>
          </div>

          <!-- 分页 -->
          <div class="flex justify-center">
            <UPagination v-model:page="untrackedPage" :total="Math.ceil(untrackedObjects.length / objectPageSize)" />
          </div>
        </div>
      </UCard>

      <!-- 轨迹查看弹窗 -->
      <UModal
        v-model:open="trajectoryModalOpen"
        title="物体轨迹查看"
        :description="`物体 ID: ${selectedObject?.objectId || ''} - ${
          selectedObject?.category ? categoryMap[selectedObject.category] || selectedObject.category : ''
        }`"
        class="w-full max-w-7xl max-h-[90vh]"
      >
        <template #content>
          <div v-if="selectedObject" class="space-y-4 p-6">
            <!-- 信息 -->
            <div class="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
              <div>
                <span class="text-muted">类别：</span>
                <UBadge color="info" size="sm">
                  {{ categoryMap[selectedObject.category] || selectedObject.category }}
                </UBadge>
              </div>
              <div>
                <span class="text-muted">帧范围：</span>
                {{ selectedObject.firstFrame }} - {{ selectedObject.lastFrame }} (共
                {{ selectedObject.lastFrame - selectedObject.firstFrame }} 帧)
              </div>
              <div>
                <span class="text-muted">轨迹点数：</span>
                <span class="font-semibold">{{ selectedObject?.trajectory?.length || 0 }}</span>
              </div>
              <div>
                <span class="text-muted">当前：</span>
                帧 {{ currentTrajectoryFrame }} | 置信度
                {{ currentTrajectoryPoint?.confidence.toFixed(4) || '-' }}
              </div>
            </div>

            <!-- 缩放控制 -->
            <div class="flex items-center gap-2 pb-2 border-b">
              <UButton icon="i-lucide-zoom-in" size="sm" variant="soft" title="放大" @click="zoomIn"> 放大 </UButton>
              <UButton icon="i-lucide-zoom-out" size="sm" variant="soft" title="缩小" @click="zoomOut"> 缩小 </UButton>
              <UButton icon="i-lucide-maximize" size="sm" variant="soft" title="重置视图" @click="resetZoom"> 重置视图 </UButton>
              <span class="text-sm text-muted ml-auto">
                缩放: {{ trajectoryScale.toFixed(2) }}x | 提示: 使用滚轮缩放，拖拽移动视图
              </span>
            </div>

            <!-- 轨迹可视化画布 -->
            <div class="relative bg-black rounded-lg overflow-hidden" style="cursor: grab;">
              <canvas
                ref="trajectoryCanvas"
                class="w-full"
                :width="1920"
                :height="1080"
                :style="{ cursor: isDragging ? 'grabbing' : 'grab' }"
                @wheel="handleWheel"
                @mousedown="handleMouseDown"
                @mousemove="handleMouseMove"
                @mouseup="handleMouseUp"
                @mouseleave="handleMouseLeave"
              />
            </div>

            <!-- 播放控制 -->
            <div class="space-y-3">
              <div class="flex items-center gap-2">
                <UButton
                  :icon="trajectoryPlaying ? 'i-lucide-pause' : 'i-lucide-play'"
                  size="sm"
                  @click="toggleTrajectoryPlayback"
                >
                  {{ trajectoryPlaying ? '暂停' : '播放' }}
                </UButton>
                <UButton icon="i-lucide-skip-back" size="sm" variant="soft" @click="resetTrajectory"> 重置 </UButton>
                <span class="text-sm text-muted ml-auto">
                  {{ trajectoryCurrentIndex + 1 }} /
                  {{ selectedObject?.trajectory?.length || 0 }}
                </span>
              </div>

              <!-- 进度条 -->
              <input
                v-model.number="trajectoryCurrentIndex"
                type="range"
                :min="0"
                :max="(selectedObject?.trajectory?.length || 1) - 1"
                class="w-full"
                @input="updateTrajectoryCanvas"
              />
            </div>
          </div>
        </template>
      </UModal>
    </div>

    <!-- 错误状态 -->
    <div v-else class="text-center py-12">
      <UIcon name="i-lucide-alert-circle" class="w-12 h-12 mx-auto text-red-500 mb-4" />
      <p class="text-muted">任务不存在</p>
    </div>
  </div>
</template>
