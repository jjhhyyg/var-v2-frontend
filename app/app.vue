<script setup lang="ts">
useHead({
  meta: [{ name: 'viewport', content: 'width=device-width, initial-scale=1' }],
  link: [{ rel: 'icon', href: '/favicon.ico' }],
  htmlAttrs: {
    lang: 'zh-CN'
  }
})

const title = 'VAR熔池视频分析系统'
const description = '基于AI的VAR熔池视频自动分析系统，支持异常事件检测和动态参数计算'

useSeoMeta({
  title,
  description,
  ogTitle: title,
  ogDescription: description
})

const toast = useToast()
const settingsOpen = ref(false)
const libraryActionLoading = ref(false)
const runtimeImportLoading = ref(false)
const schedulerSaving = ref(false)
const recoveryActionLoading = ref(false)
const closeConfirmOpen = ref(false)
const closingApp = ref(false)
let unlistenWindowClose: (() => void) | null = null

const schedulerForm = reactive({
  maxConcurrency: 3,
  macCpuLimitPercent: 85,
  macMinAvailableMemoryPercent: 20,
  windowsGpuLimitPercent: 60,
  windowsMinAvailableGpuMemoryPercent: 15
})

const { pickDirectory, pickRuntimeZip, listenWindowCloseRequested, requestAppExit } = useDesktopBridge()
const {
  appState,
  resourceState,
  loading,
  initializationProgress,
  migrationProgress,
  runtimeImportProgress,
  queueRecoveryState,
  refreshAppState,
  refreshQueueRecoveryState,
  initializeMediaLibrary,
  selectExistingMediaLibrary,
  migrateMediaLibrary,
  importRuntimeZip,
  updateSchedulerSettings,
  resolveQueueRecovery
} = useDesktopState()

const needsRuntimeSetup = computed(() => {
  if (!appState.value) {
    return false
  }

  return appState.value.runtimeRequired && !appState.value.runtimeReady
})

const needsLibrarySetup = computed(() => {
  if (!appState.value) {
    return true
  }

  if (needsRuntimeSetup.value) {
    return false
  }

  return !appState.value.initialized || !appState.value.mediaLibraryAvailable
})

const isWindowsPlatform = computed(() => appState.value?.platform === 'windows')

const resourceMetricRows = computed(() => {
  const rows = [
    {
      key: 'cpu',
      label: 'CPU',
      icon: 'i-lucide-cpu',
      percent: resourceState.value?.cpuPercent ?? null
    },
    {
      key: 'memory',
      label: '内存',
      icon: 'i-lucide-memory-stick',
      percent: resourceState.value?.memoryUsedPercent ?? null
    }
  ]

  if (isWindowsPlatform.value) {
    rows.push(
      {
        key: 'gpu',
        label: 'GPU',
        icon: 'i-lucide-gauge',
        percent: resourceState.value?.gpuPercent ?? null
      },
      {
        key: 'vram',
        label: '显存',
        icon: 'i-lucide-hard-drive',
        percent: resourceState.value?.gpuMemoryUsedPercent ?? null
      }
    )
  }

  return rows
})

const formatResourcePercent = (percent: number | null) => {
  if (percent === null || Number.isNaN(percent)) {
    return '未采样'
  }

  return `${Math.round(percent)}%`
}

const resourceProgressValue = (percent: number | null) => {
  if (percent === null || Number.isNaN(percent)) {
    return 0
  }

  return Math.min(100, Math.max(0, Math.round(percent)))
}

const progressPercent = (progress?: number) => {
  return Math.round(Math.min(1, Math.max(0, progress || 0)) * 100)
}

const syncSchedulerForm = () => {
  if (!appState.value) {
    return
  }

  schedulerForm.maxConcurrency = appState.value.maxConcurrency
  schedulerForm.macCpuLimitPercent = appState.value.macCpuLimitPercent
  schedulerForm.macMinAvailableMemoryPercent = Math.round(appState.value.macMinAvailableMemoryRatio * 100)
  schedulerForm.windowsGpuLimitPercent = appState.value.windowsGpuLimitPercent
  schedulerForm.windowsMinAvailableGpuMemoryPercent = Math.round(appState.value.windowsMinAvailableGpuMemoryRatio * 100)
}

watch(appState, () => {
  syncSchedulerForm()
}, { deep: true })

const initializeWithPath = async (path: string) => {
  libraryActionLoading.value = true
  try {
    await initializeMediaLibrary(path)
    toast.add({
      title: '媒体库已初始化',
      description: `已使用目录：${path}`,
      color: 'success'
    })
  } catch (error) {
    toast.add({
      title: '初始化失败',
      description: error instanceof Error ? error.message : '媒体库初始化失败',
      color: 'error'
    })
  } finally {
    libraryActionLoading.value = false
  }
}

const handleInitializeRecommended = async () => {
  if (!appState.value?.recommendedMediaLibraryPath) {
    return
  }
  await initializeWithPath(appState.value.recommendedMediaLibraryPath)
}

const handleInitializeCustom = async () => {
  const selected = await pickDirectory(appState.value?.recommendedMediaLibraryPath)
  if (!selected) {
    return
  }
  await initializeWithPath(selected)
}

const handleSelectExistingLibrary = async () => {
  const selected = await pickDirectory(appState.value?.recommendedMediaLibraryPath)
  if (!selected) {
    return
  }

  libraryActionLoading.value = true
  try {
    await selectExistingMediaLibrary(selected)
    toast.add({
      title: '媒体库已切换',
      description: selected,
      color: 'success'
    })
  } catch (error) {
    toast.add({
      title: '切换失败',
      description: error instanceof Error ? error.message : '媒体库校验失败',
      color: 'error'
    })
  } finally {
    libraryActionLoading.value = false
  }
}

const handleMigrateLibrary = async () => {
  const selected = await pickDirectory(appState.value?.recommendedMediaLibraryPath)
  if (!selected) {
    return
  }

  libraryActionLoading.value = true
  try {
    await migrateMediaLibrary(selected)
    toast.add({
      title: '媒体库迁移完成',
      description: selected,
      color: 'success'
    })
    settingsOpen.value = false
  } catch (error) {
    toast.add({
      title: '迁移失败',
      description: error instanceof Error ? error.message : '媒体库迁移失败',
      color: 'error'
    })
  } finally {
    libraryActionLoading.value = false
  }
}

const handleImportRuntimeZip = async () => {
  if ((appState.value?.activeTaskCount ?? 0) > 0) {
    toast.add({
      title: '暂不能更新算法包',
      description: '请等待当前分析任务结束后再更新算法包。',
      color: 'warning'
    })
    return
  }

  const selected = await pickRuntimeZip()
  if (!selected) {
    return
  }

  runtimeImportLoading.value = true
  const actionLabel = appState.value?.runtimeReady ? '更新' : '导入'
  try {
    await importRuntimeZip(selected)
    toast.add({
      title: `算法包已${actionLabel}`,
      description: '运行时自检已通过，可以继续使用桌面端。',
      color: 'success'
    })
  } catch (error) {
    toast.add({
      title: `算法包${actionLabel}失败`,
      description: getErrorMessage(error, '无法处理算法包'),
      color: 'error'
    })
  } finally {
    runtimeImportLoading.value = false
  }
}

const handleSaveSchedulerSettings = async () => {
  schedulerSaving.value = true
  try {
    const request: {
      maxConcurrency: number
      macCpuLimitPercent: number
      macMinAvailableMemoryRatio: number
      windowsGpuLimitPercent?: number
      windowsMinAvailableGpuMemoryRatio?: number
    } = {
      maxConcurrency: Number(schedulerForm.maxConcurrency),
      macCpuLimitPercent: Number(schedulerForm.macCpuLimitPercent),
      macMinAvailableMemoryRatio: Number(schedulerForm.macMinAvailableMemoryPercent) / 100
    }
    if (isWindowsPlatform.value) {
      request.windowsGpuLimitPercent = Number(schedulerForm.windowsGpuLimitPercent)
      request.windowsMinAvailableGpuMemoryRatio = Number(schedulerForm.windowsMinAvailableGpuMemoryPercent) / 100
    }
    await updateSchedulerSettings(request)
    const description = isWindowsPlatform.value
      ? `最大并发 ${schedulerForm.maxConcurrency}，CPU 阈值 ${schedulerForm.macCpuLimitPercent}%，剩余内存保底 ${schedulerForm.macMinAvailableMemoryPercent}%，GPU 阈值 ${schedulerForm.windowsGpuLimitPercent}%，剩余显存保底 ${schedulerForm.windowsMinAvailableGpuMemoryPercent}%`
      : `最大并发 ${schedulerForm.maxConcurrency}，CPU 阈值 ${schedulerForm.macCpuLimitPercent}%，剩余内存保底 ${schedulerForm.macMinAvailableMemoryPercent}%`
    toast.add({
      title: '调度设置已保存',
      description,
      color: 'success'
    })
  } catch (error) {
    toast.add({
      title: '保存失败',
      description: error instanceof Error ? error.message : '调度设置保存失败',
      color: 'error'
    })
  } finally {
    schedulerSaving.value = false
  }
}

const handleQueueRecovery = async (continueAnalysis: boolean) => {
  recoveryActionLoading.value = true
  try {
    const message = await resolveQueueRecovery(continueAnalysis)
    toast.add({
      title: continueAnalysis ? '已恢复排队任务' : '已取消恢复',
      description: message,
      color: 'success'
    })
  } catch (error) {
    toast.add({
      title: '恢复处理失败',
      description: error instanceof Error ? error.message : '无法处理待恢复队列',
      color: 'error'
    })
  } finally {
    recoveryActionLoading.value = false
  }
}

const handleRefreshPage = () => {
  if (typeof window !== 'undefined') {
    window.location.reload()
  }
}

const handleRefreshAppState = async () => {
  await refreshAppState()
}

const getErrorMessage = (error: unknown, fallback: string) => {
  if (error instanceof Error) {
    return error.message
  }

  if (typeof error === 'string') {
    return error
  }

  return fallback
}

const requestExit = async (force: boolean) => {
  closingApp.value = true
  try {
    await requestAppExit(force)
  } catch (error) {
    closingApp.value = false
    if (!force) {
      closeConfirmOpen.value = true
      return
    }

    toast.add({
      title: '退出失败',
      description: getErrorMessage(error, '无法关闭当前窗口'),
      color: 'error'
    })
  }
}

const handleWindowCloseRequested = (event: { preventDefault: () => void }) => {
  event.preventDefault()

  if (closingApp.value || closeConfirmOpen.value) {
    return
  }

  if ((appState.value?.activeTaskCount ?? 0) > 0) {
    closeConfirmOpen.value = true
    return
  }

  void requestExit(false)
}

const handleConfirmClose = async () => {
  closeConfirmOpen.value = false
  await requestExit(true)
}

onMounted(async () => {
  try {
    await refreshAppState()
    await refreshQueueRecoveryState()
    syncSchedulerForm()
  } catch (error) {
    toast.add({
      title: '桌面运行时初始化失败',
      description: error instanceof Error ? error.message : '无法读取应用状态',
      color: 'error'
    })
  }

  try {
    unlistenWindowClose = await listenWindowCloseRequested(handleWindowCloseRequested)
  } catch (error) {
    console.error('窗口关闭监听初始化失败:', error)
  }
})

onUnmounted(() => {
  if (unlistenWindowClose) {
    unlistenWindowClose()
  }
})
</script>

<template>
  <UApp>
    <UHeader>
      <template #left>
        <NuxtLink
          to="/"
          class="flex items-center gap-2"
        >
          <UIcon
            name="i-lucide-video"
            class="h-6 w-6"
          />
          <span class="font-bold">VAR熔池分析</span>
        </NuxtLink>
      </template>

      <template #default>
        <div
          v-if="appState?.initialized"
          class="flex items-center gap-6 text-sm text-muted"
        >
          <span>运行中 {{ appState.activeTaskCount }} / {{ appState.maxConcurrency }}</span>
          <span>排队中 {{ appState.queuedTaskCount }}</span>
        </div>
      </template>

      <template #right>
        <UButton
          v-if="appState?.runtimeRequired"
          icon="i-lucide-package-open"
          color="neutral"
          variant="ghost"
          :loading="runtimeImportLoading"
          aria-label="更新算法包"
          @click="handleImportRuntimeZip"
        >
          更新算法包
        </UButton>
        <UButton
          v-if="appState?.initialized"
          icon="i-lucide-sliders-horizontal"
          color="neutral"
          variant="ghost"
          @click="settingsOpen = true"
        >
          应用设置
        </UButton>
        <UButton
          icon="i-lucide-refresh-cw"
          color="neutral"
          variant="ghost"
          aria-label="刷新页面"
          @click="handleRefreshPage"
        />
        <UColorModeButton />
      </template>
    </UHeader>

    <UMain>
      <NuxtPage />
    </UMain>

    <UFooter>
      <template #left>
        <p class="text-sm text-muted">
          VAR熔池视频分析系统 • © {{ new Date().getFullYear() }}
        </p>
      </template>
    </UFooter>

    <div
      v-if="appState"
      class="fixed right-4 top-28 z-40 w-56 rounded-lg border border-accented/70 bg-default/95 p-3 shadow-lg backdrop-blur"
    >
      <div class="mb-3 flex items-center justify-between gap-2">
        <div class="flex min-w-0 items-center gap-2">
          <UIcon
            name="i-lucide-activity"
            class="h-4 w-4 shrink-0 text-primary"
          />
          <span class="truncate text-sm font-semibold">资源状态</span>
        </div>
        <span class="text-xs text-muted">实时</span>
      </div>

      <div class="space-y-3">
        <div
          v-for="metric in resourceMetricRows"
          :key="metric.key"
          class="space-y-1.5"
        >
          <div class="flex items-center justify-between gap-3 text-xs">
            <div class="flex min-w-0 items-center gap-1.5">
              <UIcon
                :name="metric.icon"
                class="h-3.5 w-3.5 shrink-0 text-muted"
              />
              <span class="truncate text-muted">{{ metric.label }}</span>
            </div>
            <span class="shrink-0 font-medium tabular-nums">{{ formatResourcePercent(metric.percent) }}</span>
          </div>
          <UProgress
            :model-value="resourceProgressValue(metric.percent)"
            :max="100"
            size="xs"
          />
        </div>
      </div>
    </div>

    <div
      v-if="loading || needsRuntimeSetup || needsLibrarySetup"
      class="fixed inset-0 z-50 flex items-center justify-center bg-default/90 p-6 backdrop-blur-sm"
    >
      <UCard class="w-full max-w-2xl">
        <template #header>
          <div class="space-y-2">
            <h2 class="text-2xl font-bold">
              {{ loading ? '正在初始化桌面环境' : needsRuntimeSetup ? '导入 Windows 算法包' : '初始化媒体库' }}
            </h2>
            <p class="text-sm text-muted">
              {{ loading
                ? '请稍候，正在读取应用配置与本地数据库。'
                : needsRuntimeSetup
                  ? '当前 Windows 版本需要导入匹配的 CUDA Runtime 算法包后才能继续使用。'
                : '桌面端会将控制数据保存在应用目录中，大体积视频和分析产物保存在独立媒体库中，以避免更新软件时丢失历史数据。' }}
            </p>
          </div>
        </template>

        <div
          v-if="loading"
          class="space-y-5 py-8 text-center"
        >
          <UIcon
            name="i-lucide-loader-2"
            class="mx-auto mb-4 h-10 w-10 animate-spin"
          />
          <p class="text-muted">
            {{ initializationProgress?.message || '正在准备桌面运行环境...' }}
          </p>
          <div
            v-if="initializationProgress"
            class="mx-auto max-w-md space-y-2 text-left"
          >
            <div class="flex items-center justify-between text-sm">
              <span>阶段：{{ initializationProgress.stage }}</span>
              <span>{{ progressPercent(initializationProgress.progress) }}%</span>
            </div>
            <UProgress
              :model-value="progressPercent(initializationProgress.progress)"
              :max="100"
            />
          </div>
        </div>

        <div
          v-else-if="needsRuntimeSetup"
          class="space-y-5"
        >
          <div class="rounded-lg border bg-muted/30 p-4">
            <p class="mb-2 text-sm font-medium">
              需要的算法包
            </p>
            <div class="space-y-1 break-all text-sm">
              <p>平台：{{ appState?.runtimePlatform }}</p>
              <p>版本：{{ appState?.requiredRuntimeBuildId }}</p>
              <p v-if="appState?.runtimeBuildId">
                已安装版本：{{ appState.runtimeBuildId }}
              </p>
            </div>
          </div>

          <UAlert
            v-if="appState?.runtimeError"
            color="warning"
            variant="soft"
            icon="i-lucide-triangle-alert"
            title="算法包不可用"
            :description="appState.runtimeError"
          />

          <div
            v-if="runtimeImportProgress && runtimeImportProgress.stage !== 'completed'"
            class="space-y-3 rounded-lg border p-4"
          >
            <div class="flex items-center justify-between text-sm">
              <span>导入阶段：{{ runtimeImportProgress.stage }}</span>
              <span>{{ progressPercent(runtimeImportProgress.progress) }}%</span>
            </div>
            <UProgress
              :model-value="progressPercent(runtimeImportProgress.progress)"
              :max="100"
            />
            <p class="break-all text-xs text-muted">
              {{ runtimeImportProgress.message }}
            </p>
          </div>

          <div class="flex flex-wrap gap-3">
            <UButton
              icon="i-lucide-package-open"
              :loading="runtimeImportLoading"
              @click="handleImportRuntimeZip"
            >
              选择算法包 zip
            </UButton>
            <UButton
              color="neutral"
              variant="outline"
              icon="i-lucide-refresh-cw"
              :loading="loading"
              @click="handleRefreshAppState"
            >
              重新检查
            </UButton>
          </div>
        </div>

        <div
          v-else
          class="space-y-5"
        >
          <div class="rounded-lg border bg-muted/30 p-4">
            <p class="mb-2 text-sm font-medium">
              推荐媒体库目录
            </p>
            <p class="break-all text-sm">
              {{ appState?.recommendedMediaLibraryPath }}
            </p>
          </div>

          <div
            v-if="migrationProgress && migrationProgress.stage !== 'completed'"
            class="space-y-3 rounded-lg border p-4"
          >
            <div class="flex items-center justify-between text-sm">
              <span>迁移阶段：{{ migrationProgress.stage }}</span>
              <span>{{ progressPercent(migrationProgress.progress) }}%</span>
            </div>
            <UProgress
              :model-value="progressPercent(migrationProgress.progress)"
              :max="100"
            />
            <p class="break-all text-xs text-muted">
              {{ migrationProgress.message }}
            </p>
          </div>

          <div class="flex flex-wrap gap-3">
            <UButton
              icon="i-lucide-check"
              :loading="libraryActionLoading"
              @click="handleInitializeRecommended"
            >
              使用推荐目录
            </UButton>
            <UButton
              color="neutral"
              variant="outline"
              icon="i-lucide-folder-open"
              :loading="libraryActionLoading"
              @click="handleInitializeCustom"
            >
              选择新媒体库目录
            </UButton>
            <UButton
              color="neutral"
              variant="outline"
              icon="i-lucide-history"
              :loading="libraryActionLoading"
              @click="handleSelectExistingLibrary"
            >
              选择已有媒体库
            </UButton>
          </div>
        </div>
      </UCard>
    </div>

    <UModal
      v-model:open="settingsOpen"
      title="应用设置"
      description="管理媒体库存储位置，并调整桌面端有限并发调度参数。"
    >
      <template #body>
        <div class="space-y-6">
          <div class="space-y-4">
            <div class="rounded-lg border bg-muted/30 p-4">
              <p class="mb-2 text-sm font-medium">
                当前媒体库
              </p>
              <p class="break-all text-sm">
                {{ appState?.mediaLibraryPath || '尚未初始化' }}
              </p>
            </div>

            <div
              v-if="migrationProgress && migrationProgress.stage !== 'completed'"
              class="space-y-3 rounded-lg border p-4"
            >
              <div class="flex items-center justify-between text-sm">
                <span>迁移阶段：{{ migrationProgress.stage }}</span>
                <span>{{ progressPercent(migrationProgress.progress) }}%</span>
              </div>
              <UProgress
                :model-value="progressPercent(migrationProgress.progress)"
                :max="100"
              />
              <p class="break-all text-xs text-muted">
                {{ migrationProgress.message }}
              </p>
            </div>

            <div class="flex flex-wrap gap-3">
              <UButton
                color="neutral"
                variant="outline"
                icon="i-lucide-history"
                :loading="libraryActionLoading"
                @click="handleSelectExistingLibrary"
              >
                切换到已有媒体库
              </UButton>
              <UButton
                icon="i-lucide-folder-sync"
                :loading="libraryActionLoading"
                @click="handleMigrateLibrary"
              >
                迁移媒体库
              </UButton>
            </div>
          </div>

          <div class="space-y-4 border-t pt-6">
            <div class="space-y-1">
              <h3 class="text-base font-semibold">
                任务调度设置
              </h3>
              <p class="text-sm text-muted">
                当前运行中 {{ appState?.activeTaskCount ?? 0 }} / {{ appState?.maxConcurrency ?? 0 }}，排队中 {{ appState?.queuedTaskCount ?? 0 }}。
              </p>
              <p class="text-xs text-muted">
                {{ isWindowsPlatform ? '当 CPU/GPU 达到阈值或剩余内存/显存低于保底时，系统不会继续追加并发任务。' : '当 CPU 达到阈值或剩余内存低于保底时，系统不会继续追加并发任务。' }}
              </p>
            </div>

            <div class="space-y-3 rounded-lg border border-accented/60 bg-muted/20 p-4">
              <div class="flex items-center gap-4">
                <label class="w-[38%] shrink-0 text-sm font-medium">最大并发数</label>
                <UInput
                  v-model.number="schedulerForm.maxConcurrency"
                  type="number"
                  min="1"
                  max="6"
                  class="w-[28%] min-w-28 shrink-0"
                />
              </div>
              <div class="flex items-center gap-4">
                <label class="w-[38%] shrink-0 text-sm font-medium">CPU 阈值 (%)</label>
                <UInput
                  v-model.number="schedulerForm.macCpuLimitPercent"
                  type="number"
                  min="1"
                  max="100"
                  class="w-[28%] min-w-28 shrink-0"
                />
              </div>
              <div class="flex items-center gap-4">
                <label class="w-[38%] shrink-0 text-sm font-medium">最低剩余内存 (%)</label>
                <UInput
                  v-model.number="schedulerForm.macMinAvailableMemoryPercent"
                  type="number"
                  min="0"
                  max="100"
                  step="1"
                  class="w-[28%] min-w-28 shrink-0"
                />
              </div>
              <div
                v-if="isWindowsPlatform"
                class="flex items-center gap-4"
              >
                <label class="w-[38%] shrink-0 text-sm font-medium">GPU 阈值 (%)</label>
                <UInput
                  v-model.number="schedulerForm.windowsGpuLimitPercent"
                  type="number"
                  min="1"
                  max="100"
                  class="w-[28%] min-w-28 shrink-0"
                />
              </div>
              <div
                v-if="isWindowsPlatform"
                class="flex items-center gap-4"
              >
                <label class="w-[38%] shrink-0 text-sm font-medium">最低剩余显存 (%)</label>
                <UInput
                  v-model.number="schedulerForm.windowsMinAvailableGpuMemoryPercent"
                  type="number"
                  min="0"
                  max="100"
                  step="1"
                  class="w-[28%] min-w-28 shrink-0"
                />
              </div>
            </div>

            <div class="flex justify-end">
              <UButton
                icon="i-lucide-save"
                :loading="schedulerSaving"
                @click="handleSaveSchedulerSettings"
              >
                保存调度设置
              </UButton>
            </div>
          </div>
        </div>
      </template>
    </UModal>

    <UModal
      :open="queueRecoveryState.hasPendingRecovery && !needsRuntimeSetup"
      :dismissible="false"
      :close="false"
      title="恢复上次排队任务"
      description="上次关闭应用时仍有排队中的任务。请选择继续恢复队列，或将这些任务改回待启动。"
    >
      <template #body>
        <div class="space-y-4">
          <div class="max-h-72 space-y-2 overflow-y-auto rounded-lg border p-3">
            <div
              v-for="task in queueRecoveryState.tasks"
              :key="task.taskId"
              class="flex items-center justify-between rounded-md border px-3 py-2 text-sm"
            >
              <div class="min-w-0">
                <p class="truncate font-medium">
                  {{ task.name }}
                </p>
                <p class="text-xs text-muted">
                  Task {{ task.taskId }}
                </p>
              </div>
              <span class="text-xs text-muted">queueOrder: {{ task.queueOrder }}</span>
            </div>
          </div>

          <div class="flex justify-end gap-2">
            <UButton
              color="neutral"
              variant="outline"
              :loading="recoveryActionLoading"
              @click="handleQueueRecovery(false)"
            >
              取消恢复并改为待启动
            </UButton>
            <UButton
              :loading="recoveryActionLoading"
              @click="handleQueueRecovery(true)"
            >
              继续排队分析
            </UButton>
          </div>
        </div>
      </template>
    </UModal>

    <UModal
      v-model:open="closeConfirmOpen"
      :dismissible="false"
      :close="false"
      title="确认退出应用"
      description="当前仍有分析中的任务。现在退出会中断分析，下次启动后这些任务会按现有恢复逻辑转为失败或中断结果。"
    >
      <template #body>
        <div class="space-y-4">
          <p class="text-sm text-muted">
            当前运行中 {{ appState?.activeTaskCount ?? 0 }} 个任务。若继续退出，正在预处理或分析的任务不会保留执行现场。
          </p>

          <div class="flex justify-end gap-2">
            <UButton
              color="neutral"
              variant="outline"
              :disabled="closingApp"
              @click="closeConfirmOpen = false"
            >
              取消
            </UButton>
            <UButton
              color="error"
              :loading="closingApp"
              @click="handleConfirmClose"
            >
              仍要退出
            </UButton>
          </div>
        </div>
      </template>
    </UModal>
  </UApp>
</template>
