/**
 * 桌面版任务 API
 */

export interface TaskConfig {
  timeoutRatio?: string
  modelVersion?: string
  frameRate?: number
  enablePreprocessing?: boolean
  preprocessingStrength?: string
  preprocessingEnhancePool?: boolean
  enableTrackingMerge?: boolean
  trackingMergeStrategy?: string
}

export interface Task {
  taskId: string
  name: string
  originalFilename?: string
  videoDuration: number
  status: string
  timeoutThreshold: number
  isTimeout: boolean
  config?: TaskConfig
  createdAt: string
  startedAt?: string
  preprocessingCompletedAt?: string
  completedAt?: string
  failureReason?: string
  resultVideoPath?: string
  preprocessedVideoPath?: string
  queuePosition?: number
}

export interface TaskStatus {
  taskId: string
  status: string
  phase?: string
  progress?: number
  currentFrame?: number
  totalFrames?: number
  preprocessingDuration?: number
  analyzingElapsedTime?: number
  isTimeout?: boolean
  timeoutWarning?: boolean
  failureReason?: string
  queuePosition?: number
}

export interface BatchImportItem {
  filePath: string
  name?: string
}

export interface BatchImportFailure {
  filePath: string
  fileName: string
  reason: string
}

export interface BatchImportResult {
  createdTasks: Task[]
  failedFiles: BatchImportFailure[]
  queuedTaskIds: string[]
}

export interface DynamicMetric {
  frameNumber: number
  timestamp: number
  brightness?: number
  poolArea?: number
  poolPerimeter?: number
}

export interface AnomalyEvent {
  eventId: string
  eventType: string
  startFrame: number
  endFrame: number
  objectId?: string
  metadata?: Record<string, unknown>
}

export interface TrajectoryPoint {
  bbox: [number, number, number, number]
  frame: number
  confidence: number
}

export interface TrackingObject {
  trackingId: string
  objectId: string
  category: string
  firstFrame: number
  lastFrame: number
  trajectory?: TrajectoryPoint[]
}

export interface GlobalAnalysisMetric {
  frequency?: number
  trend?: string
  mean?: number
  [key: string]: unknown
}

export interface TaskResult {
  taskId: string
  name: string
  status: string
  isTimeout: boolean
  dynamicMetrics: DynamicMetric[]
  globalAnalysis?: Record<string, GlobalAnalysisMetric>
  anomalyEvents: AnomalyEvent[]
  trackingObjects: TrackingObject[]
  eventStatistics: Record<string, number>
  objectStatistics: Record<string, number>
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

export type TaskSortField = 'createdAt' | 'status' | 'completedAt'
export type TaskSortDirection = 'asc' | 'desc'

export const useTaskApi = () => {
  const { invokeCommand } = useDesktopBridge()

  const uploadTask = async (filePath: string, name?: string, config?: TaskConfig): Promise<Task> => {
    return invokeCommand<Task>('import_video_task', {
      request: {
        filePath,
        name,
        config
      }
    })
  }

  const importVideoTasks = async (
    items: BatchImportItem[],
    config?: TaskConfig,
    autoStart = false
  ): Promise<BatchImportResult> => {
    return invokeCommand<BatchImportResult>('import_video_tasks', {
      request: {
        items,
        config,
        autoStart
      }
    })
  }

  const listTasks = async (
    page = 0,
    size = 20,
    status?: string,
    sortBy?: TaskSortField,
    sortDirection?: TaskSortDirection
  ): Promise<PageResult<Task>> => {
    return invokeCommand<PageResult<Task>>('list_tasks', {
      request: {
        page,
        size,
        status,
        sortBy,
        sortDirection
      }
    })
  }

  const getTask = async (taskId: string): Promise<Task> => {
    return invokeCommand<Task>('get_task', { taskId })
  }

  const getTaskStatus = async (taskId: string): Promise<TaskStatus> => {
    return invokeCommand<TaskStatus>('get_task_status', { taskId })
  }

  const getTaskResult = async (taskId: string): Promise<TaskResult> => {
    return invokeCommand<TaskResult>('get_task_result', { taskId })
  }

  const startAnalysis = async (taskId: string): Promise<string> => {
    return invokeCommand<string>('start_task', { taskId })
  }

  const reanalyzeTask = async (taskId: string): Promise<string> => {
    return invokeCommand<string>('reanalyze_task', { taskId })
  }

  const dequeueTask = async (taskId: string): Promise<string> => {
    return invokeCommand<string>('dequeue_task', { taskId })
  }

  const deleteTask = async (taskId: string): Promise<string> => {
    return invokeCommand<string>('delete_task', { taskId })
  }

  const getVideoStreamUrl = async (taskId: string, videoType: 'original' | 'preprocessed' | 'result'): Promise<string> => {
    return invokeCommand<string>('get_video_stream_url', {
      taskId,
      videoType
    })
  }

  const exportReportFile = async (path: string, options: { textContent?: string, base64Content?: string }): Promise<string> => {
    return invokeCommand<string>('export_report_file', {
      request: {
        path,
        textContent: options.textContent,
        base64Content: options.base64Content
      }
    })
  }

  return {
    uploadTask,
    importVideoTasks,
    listTasks,
    getTask,
    getTaskStatus,
    getTaskResult,
    startAnalysis,
    reanalyzeTask,
    dequeueTask,
    deleteTask,
    getVideoStreamUrl,
    exportReportFile
  }
}
