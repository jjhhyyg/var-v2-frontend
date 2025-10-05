/**
 * 任务相关API
 */

export interface TaskConfig {
  timeoutRatio?: string;
  confidenceThreshold?: number;
  iouThreshold?: number;
  modelVersion?: string;
}

export interface Task {
  taskId: string;
  name: string;
  videoDuration: number;
  status: string;
  timeoutThreshold: number;
  isTimeout: boolean;
  config?: TaskConfig;
  createdAt: string;
  startedAt?: string;
  preprocessingCompletedAt?: string;
  completedAt?: string;
  failureReason?: string;
  resultVideoPath?: string;
}

export interface TaskStatus {
  taskId: string;
  status: string;
  phase?: string;
  progress?: number;
  currentFrame?: number;
  totalFrames?: number;
  preprocessingDuration?: number;
  analyzingElapsedTime?: number;
  isTimeout?: boolean;
  timeoutWarning?: boolean;
  failureReason?: string;
}

export interface DynamicMetric {
  frameNumber: number;
  timestamp: number;
  brightness?: number;  // 熔池亮度值
  poolArea?: number;    // 熔池面积
  poolPerimeter?: number;  // 熔池周长
}

export interface AnomalyEvent {
  eventId: string;
  eventType: string;
  startFrame: number;
  endFrame: number;
  objectId?: string;
  metadata?: Record<string, unknown>;
}

export interface TrajectoryPoint {
  bbox: [number, number, number, number];
  frame: number;
  confidence: number;
}

export interface TrackingObject {
  trackingId: string;
  objectId: string;
  category: string;
  firstFrame: number;
  lastFrame: number;
  trajectory?: TrajectoryPoint[];
}

export interface TaskResult {
  taskId: string;
  name: string;
  status: string;
  isTimeout: boolean;
  dynamicMetrics: DynamicMetric[];  // 每帧的动态参数（亮度、面积、周长）
  globalAnalysis?: Record<string, any>;  // 全局频率分析结果（闪烁频率、趋势等）
  anomalyEvents: AnomalyEvent[];
  trackingObjects: TrackingObject[];
  eventStatistics: Record<string, number>;
  objectStatistics: Record<string, number>;
}

export interface PageResult<T> {
  items: T[];
  total: number;
  totalPages: number;
  page: number;
  pageSize: number;
  hasNext: boolean;
  hasPrevious: boolean;
}

export const useTaskApi = () => {
  const { request } = useApi();

  /**
   * 上传视频并创建任务
   */
  const uploadTask = async (
    file: File,
    name?: string,
    config?: TaskConfig
  ): Promise<Task> => {
    const formData = new FormData();
    formData.append("video", file);
    if (name) formData.append("name", name);
    if (config?.timeoutRatio)
      formData.append("timeoutRatio", config.timeoutRatio);
    if (config?.confidenceThreshold)
      formData.append(
        "confidenceThreshold",
        config.confidenceThreshold.toString()
      );
    if (config?.iouThreshold)
      formData.append("iouThreshold", config.iouThreshold.toString());

    return request<Task>("/api/tasks/upload", {
      method: "POST",
      body: formData,
    });
  };

  /**
   * 获取任务列表
   */
  const listTasks = async (
    page = 0,
    size = 20,
    status?: string
  ): Promise<PageResult<Task>> => {
    const params: Record<string, string | number> = { page, size };
    if (status) params.status = status;

    return request<PageResult<Task>>("/api/tasks", {
      method: "GET",
      params,
    });
  };

  /**
   * 获取任务详情
   */
  const getTask = async (taskId: string): Promise<Task> => {
    return request<Task>(`/api/tasks/${taskId}`);
  };

  /**
   * 获取任务状态
   */
  const getTaskStatus = async (taskId: string): Promise<TaskStatus> => {
    return request<TaskStatus>(`/api/tasks/${taskId}/status`);
  };

  /**
   * 获取任务结果
   */
  const getTaskResult = async (taskId: string): Promise<TaskResult> => {
    return request<TaskResult>(`/api/tasks/${taskId}/result`);
  };

  /**
   * 开始任务分析
   */
  const startAnalysis = async (taskId: string): Promise<string> => {
    return request<string>(`/api/tasks/${taskId}/start`, {
      method: "POST",
    });
  };

  /**
   * 重新分析任务
   */
  const reanalyzeTask = async (taskId: string): Promise<string> => {
    return request<string>(`/api/tasks/${taskId}/reanalyze`, {
      method: "POST",
    });
  };

  /**
   * 删除任务
   */
  const deleteTask = async (taskId: string): Promise<string> => {
    return request<string>(`/api/tasks/${taskId}`, {
      method: "DELETE",
    });
  };

  return {
    uploadTask,
    listTasks,
    getTask,
    getTaskStatus,
    getTaskResult,
    startAnalysis,
    reanalyzeTask,
    deleteTask,
  };
};
