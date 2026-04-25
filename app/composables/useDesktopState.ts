export interface AppState {
  initialized: boolean
  mediaLibraryAvailable: boolean
  mediaLibraryPath?: string
  recommendedMediaLibraryPath: string
  maxConcurrency: number
  macCpuLimitPercent: number
  macMinAvailableMemoryRatio: number
  windowsGpuLimitPercent: number
  windowsMinAvailableGpuMemoryRatio: number
  activeTaskCount: number
  queuedTaskCount: number
  platform: string
  version: string
  runtimeRequired: boolean
  runtimeReady: boolean
  runtimeBuildId?: string
  requiredRuntimeBuildId: string
  runtimePlatform: string
  runtimeError?: string
}

export interface MigrationProgress {
  stage: string
  progress: number
  message?: string
}

export interface SchedulerSettingsInput {
  maxConcurrency?: number
  macCpuLimitPercent?: number
  macMinAvailableMemoryRatio?: number
  windowsGpuLimitPercent?: number
  windowsMinAvailableGpuMemoryRatio?: number
}

export interface SchedulerState {
  maxConcurrency: number
  activeTaskCount: number
  queuedTaskCount: number
}

export interface QueueRecoveryTask {
  taskId: string
  name: string
  queueOrder: number
}

export interface QueueRecoveryState {
  hasPendingRecovery: boolean
  tasks: QueueRecoveryTask[]
}

export interface ImportRuntimeInput {
  path: string
}

export interface RuntimeStateResponse {
  runtimeRequired: boolean
  runtimeReady: boolean
  runtimeBuildId?: string
  requiredRuntimeBuildId: string
  runtimePlatform: string
  runtimeError?: string
}

export interface ResourceState {
  cpuPercent: number
  memoryUsedPercent: number
  gpuPercent: number | null
  gpuMemoryUsedPercent: number | null
}

let migrationSubscribed = false
let schedulerSubscribed = false
let resourceSubscribed = false
let initializationSubscribed = false
let runtimeImportSubscribed = false

export const useDesktopState = () => {
  const { invokeCommand, listenEvent } = useDesktopBridge()
  const appState = useState<AppState | null>('desktop-app-state', () => null)
  const resourceState = useState<ResourceState | null>('desktop-resource-state', () => null)
  const loading = useState<boolean>('desktop-app-state-loading', () => false)
  const initializationProgress = useState<MigrationProgress | null>('desktop-initialization-progress', () => null)
  const migrationProgress = useState<MigrationProgress | null>('desktop-migration-progress', () => null)
  const runtimeImportProgress = useState<MigrationProgress | null>('desktop-runtime-import-progress', () => null)
  const queueRecoveryState = useState<QueueRecoveryState>('desktop-queue-recovery-state', () => ({
    hasPendingRecovery: false,
    tasks: []
  }))

  const ensureMigrationListener = async () => {
    if (migrationSubscribed) {
      return
    }

    migrationSubscribed = true
    await listenEvent<MigrationProgress>('library-migration-progress', (payload) => {
      migrationProgress.value = payload
    })
  }

  const ensureSchedulerListener = async () => {
    if (schedulerSubscribed) {
      return
    }

    schedulerSubscribed = true
    await listenEvent<SchedulerState>('scheduler-state-update', (payload) => {
      if (!appState.value) {
        return
      }

      appState.value = {
        ...appState.value,
        maxConcurrency: payload.maxConcurrency,
        activeTaskCount: payload.activeTaskCount,
        queuedTaskCount: payload.queuedTaskCount
      }
    })
  }

  const ensureInitializationListener = async () => {
    if (initializationSubscribed) {
      return
    }

    initializationSubscribed = true
    await listenEvent<MigrationProgress>('desktop-initialization-progress', (payload) => {
      initializationProgress.value = payload
    })
  }

  const ensureRuntimeImportListener = async () => {
    if (runtimeImportSubscribed) {
      return
    }

    runtimeImportSubscribed = true
    await listenEvent<MigrationProgress>('runtime-import-progress', (payload) => {
      runtimeImportProgress.value = payload
    })
  }

  const ensureResourceListener = async () => {
    if (resourceSubscribed) {
      return
    }

    resourceSubscribed = true
    await listenEvent<ResourceState>('resource-state-update', (payload) => {
      resourceState.value = payload
    })
  }

  const refreshResourceState = async () => {
    resourceState.value = await invokeCommand<ResourceState>('get_resource_state')
    return resourceState.value
  }

  const refreshAppState = async () => {
    loading.value = true
    initializationProgress.value = {
      stage: 'starting',
      progress: 0,
      message: '开始读取桌面环境...'
    }
    try {
      await Promise.all([
        ensureInitializationListener(),
        ensureMigrationListener(),
        ensureSchedulerListener(),
        ensureResourceListener(),
        ensureRuntimeImportListener()
      ])
      appState.value = await invokeCommand<AppState>('get_app_state')
      await Promise.all([
        refreshResourceState()
      ])
      return appState.value
    } finally {
      loading.value = false
    }
  }

  const refreshQueueRecoveryState = async () => {
    queueRecoveryState.value = await invokeCommand<QueueRecoveryState>('get_queue_recovery_state')
    return queueRecoveryState.value
  }

  const initializeMediaLibrary = async (path: string) => {
    await ensureMigrationListener()
    migrationProgress.value = {
      stage: 'initializing',
      progress: 0,
      message: '准备初始化媒体库...'
    }
    appState.value = await invokeCommand<AppState>('initialize_media_library', { path })
    return appState.value
  }

  const selectExistingMediaLibrary = async (path: string) => {
    await ensureMigrationListener()
    migrationProgress.value = {
      stage: 'validating',
      progress: 0,
      message: '准备校验媒体库...'
    }
    appState.value = await invokeCommand<AppState>('select_existing_media_library', { path })
    return appState.value
  }

  const migrateMediaLibrary = async (path: string) => {
    await ensureMigrationListener()
    migrationProgress.value = {
      stage: 'preparing',
      progress: 0,
      message: '准备迁移媒体库...'
    }
    appState.value = await invokeCommand<AppState>('migrate_media_library', { path })
    return appState.value
  }

  const updateSchedulerSettings = async (request: SchedulerSettingsInput) => {
    appState.value = await invokeCommand<AppState>('update_scheduler_settings', {
      request
    })
    return appState.value
  }

  const importRuntimeZip = async (path: string) => {
    await ensureRuntimeImportListener()
    runtimeImportProgress.value = {
      stage: 'preparing',
      progress: 0,
      message: '准备导入算法包...'
    }
    await invokeCommand<RuntimeStateResponse>('import_runtime_zip', {
      request: {
        path
      }
    })
    return refreshAppState()
  }

  const resolveQueueRecovery = async (continueAnalysis: boolean) => {
    const message = await invokeCommand<string>('resolve_queue_recovery', {
      request: {
        continueAnalysis
      }
    })
    await Promise.all([
      refreshQueueRecoveryState(),
      refreshAppState()
    ])
    return message
  }

  return {
    appState,
    resourceState,
    loading,
    initializationProgress,
    migrationProgress,
    runtimeImportProgress,
    queueRecoveryState,
    refreshAppState,
    refreshResourceState,
    refreshQueueRecoveryState,
    initializeMediaLibrary,
    selectExistingMediaLibrary,
    migrateMediaLibrary,
    importRuntimeZip,
    updateSchedulerSettings,
    resolveQueueRecovery
  }
}
