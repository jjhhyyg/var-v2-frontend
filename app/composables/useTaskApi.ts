/**
 * 任务相关API
 */

/**
 * 任务配置数据（仅包含配置参数）
 */
export interface TaskConfig {
  timeoutRatio?: string
  modelVersion?: string
  enablePreprocessing?: boolean
  preprocessingStrength?: string
  preprocessingEnhancePool?: boolean
  enableTrackingMerge?: boolean
  trackingMergeStrategy?: string
}

/**
 * 任务响应（对应后端 TaskResponse）
 */
export interface Task {
  taskId: string // 后端 Long 类型，前端使用 string 避免精度丢失
  name: string
  videoDuration: number
  status: string
  timeoutThreshold: number
  isTimeout: boolean
  config?: TaskConfig
  createdAt: string // ISO 8601 格式时间字符串
  startedAt?: string
  preprocessingCompletedAt?: string
  completedAt?: string
  failureReason?: string
  resultVideoPath?: string
  preprocessedVideoPath?: string
}

/**
 * 任务状态响应（对应后端 TaskStatusResponse）
 */
export interface TaskStatus {
  taskId: string // 后端 Long 类型，前端使用 string 避免精度丢失
  status: string
  phase?: string
  progress?: number // 0.0~1.0
  currentFrame?: number
  totalFrames?: number
  preprocessingDuration?: number // 秒
  analyzingElapsedTime?: number // 秒
  isTimeout?: boolean
  timeoutWarning?: boolean
  failureReason?: string
}

/**
 * 动态参数数据（每帧的亮度、面积、周长）
 */
export interface DynamicMetric {
  frameNumber: number
  timestamp: number
  brightness?: number // 熔池亮度值
  poolArea?: number // 熔池面积（像素）
  poolPerimeter?: number // 熔池周长（像素）
}

/**
 * 异常事件数据
 */
export interface AnomalyEvent {
  eventId: string // 后端 Long 类型，前端使用 string 避免精度丢失
  eventType: string
  startFrame: number
  endFrame: number
  objectId?: string // 修改为 string 以保持一致性
  metadata?: Record<string, unknown>
}

/**
 * 轨迹点数据
 */
export interface TrajectoryPoint {
  bbox: [number, number, number, number]
  frame: number
  confidence: number
}

/**
 * 追踪物体数据
 */
export interface TrackingObject {
  trackingId: string // 后端 Long 类型，前端使用 string 避免精度丢失
  objectId: string // 修改为 string 以保持一致性
  category: string
  firstFrame: number
  lastFrame: number
  trajectory?: TrajectoryPoint[] // 明确定义为轨迹点数组
}

/**
 * 全局分析数据
 */
export interface GlobalAnalysisMetric {
  frequency?: number
  trend?: string
  mean?: number
  [key: string]: unknown // 允许其他动态属性
}

/**
 * 任务结果响应（对应后端 TaskResultResponse）
 */
export interface TaskResult {
  taskId: string // 后端 Long 类型,前端使用 string 避免精度丢失
  name: string
  status: string
  isTimeout: boolean
  dynamicMetrics: DynamicMetric[] // 每帧的动态参数（亮度、面积、周长）
  globalAnalysis?: Record<string, GlobalAnalysisMetric> // 全局频率分析结果（闪烁频率、趋势等）
  anomalyEvents: AnomalyEvent[]
  trackingObjects: TrackingObject[]
  eventStatistics: Record<string, number> // 事件类型 -> 数量
  objectStatistics: Record<string, number> // 物体类别 -> 数量
}

export interface PageResult<T> {
  items: T[]
  total: number
  totalPages: number
  page: number
  pageSize: number
  hasNext: boolean
  hasPrevious: boolean
}

export const useTaskApi = () => {
  const { request } = useApi()

  /**
   * 上传视频并创建任务
   */
  const uploadTask = async (file: File, name?: string, config?: TaskConfig): Promise<Task> => {
    const formData = new FormData()
    formData.append('video', file)
    if (name) formData.append('name', name)
    if (config?.timeoutRatio) formData.append('timeoutRatio', config.timeoutRatio)
    if (config?.enablePreprocessing !== undefined) formData.append('enablePreprocessing', String(config.enablePreprocessing))
    if (config?.preprocessingStrength) formData.append('preprocessingStrength', config.preprocessingStrength)
    if (config?.preprocessingEnhancePool !== undefined) formData.append('preprocessingEnhancePool', String(config.preprocessingEnhancePool))
    if (config?.enableTrackingMerge !== undefined) formData.append('enableTrackingMerge', String(config.enableTrackingMerge))
    if (config?.trackingMergeStrategy) formData.append('trackingMergeStrategy', config.trackingMergeStrategy)

    return request<Task>('/api/tasks/upload', {
      method: 'POST',
      body: formData
    })
  }

  /**
   * 获取任务列表
   */
  const listTasks = async (page = 0, size = 20, status?: string): Promise<PageResult<Task>> => {
    const params: Record<string, string | number> = { page, size }
    if (status) params.status = status

    return request<PageResult<Task>>('/api/tasks', {
      method: 'GET',
      params
    })
  }

  /**
   * 获取任务详情
   */
  const getTask = async (taskId: string): Promise<Task> => {
    return request<Task>(`/api/tasks/${taskId}`)
  }

  /**
   * 获取任务状态
   */
  const getTaskStatus = async (taskId: string): Promise<TaskStatus> => {
    return request<TaskStatus>(`/api/tasks/${taskId}/status`)
  }

  /**
   * 获取任务结果
   */
  const getTaskResult = async (taskId: string): Promise<TaskResult> => {
    return request<TaskResult>(`/api/tasks/${taskId}/result`)
  }

  /**
   * 开始任务分析
   */
  const startAnalysis = async (taskId: string): Promise<string> => {
    return request<string>(`/api/tasks/${taskId}/start`, {
      method: 'POST'
    })
  }

  /**
   * 重新分析任务
   */
  const reanalyzeTask = async (taskId: string): Promise<string> => {
    return request<string>(`/api/tasks/${taskId}/reanalyze`, {
      method: 'POST'
    })
  }

  /**
   * 删除任务
   */
  const deleteTask = async (taskId: string): Promise<string> => {
    return request<string>(`/api/tasks/${taskId}`, {
      method: 'DELETE'
    })
  }

  return {
    uploadTask,
    listTasks,
    getTask,
    getTaskStatus,
    getTaskResult,
    startAnalysis,
    reanalyzeTask,
    deleteTask
  }
}
