export interface AppState {
  initialized: boolean
  mediaLibraryAvailable: boolean
  mediaLibraryPath?: string
  recommendedMediaLibraryPath: string
  maxConcurrency: number
  macCpuLimitPercent: number
  macMinAvailableMemoryRatio: number
  activeTaskCount: number
  queuedTaskCount: number
  platform: string
  version: string
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

let migrationSubscribed = false
let schedulerSubscribed = false

export const useDesktopState = () => {
  const { invokeCommand, listenEvent } = useDesktopBridge()
  const appState = useState<AppState | null>('desktop-app-state', () => null)
  const loading = useState<boolean>('desktop-app-state-loading', () => false)
  const migrationProgress = useState<MigrationProgress | null>('desktop-migration-progress', () => null)
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

  const refreshAppState = async () => {
    loading.value = true
    try {
      appState.value = await invokeCommand<AppState>('get_app_state')
      await Promise.all([
        ensureMigrationListener(),
        ensureSchedulerListener()
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
    appState.value = await invokeCommand<AppState>('initialize_media_library', { path })
    return appState.value
  }

  const selectExistingMediaLibrary = async (path: string) => {
    appState.value = await invokeCommand<AppState>('select_existing_media_library', { path })
    return appState.value
  }

  const migrateMediaLibrary = async (path: string) => {
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
    loading,
    migrationProgress,
    queueRecoveryState,
    refreshAppState,
    refreshQueueRecoveryState,
    initializeMediaLibrary,
    selectExistingMediaLibrary,
    migrateMediaLibrary,
    updateSchedulerSettings,
    resolveQueueRecovery
  }
}
