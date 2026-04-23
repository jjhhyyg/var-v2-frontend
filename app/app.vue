<script setup lang="ts">
import type { AppState } from '~/composables/useDesktopState'

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
const schedulerSaving = ref(false)
const recoveryActionLoading = ref(false)
const closeConfirmOpen = ref(false)
const closingApp = ref(false)
const allowWindowClose = ref(false)
let unlistenWindowClose: (() => void) | null = null

const schedulerForm = reactive({
  maxConcurrency: 3,
  macCpuLimitPercent: 85,
  macMinAvailableMemoryPercent: 20
})

const { pickDirectory, invokeCommand, listenWindowCloseRequested, closeCurrentWindow } = useDesktopBridge()
const {
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
} = useDesktopState()

const needsLibrarySetup = computed(() => {
  if (!appState.value) {
    return true
  }

  return !appState.value.initialized || !appState.value.mediaLibraryAvailable
})

const syncSchedulerForm = () => {
  if (!appState.value) {
    return
  }

  schedulerForm.maxConcurrency = appState.value.maxConcurrency
  schedulerForm.macCpuLimitPercent = appState.value.macCpuLimitPercent
  schedulerForm.macMinAvailableMemoryPercent = Math.round(appState.value.macMinAvailableMemoryRatio * 100)
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

const handleSaveSchedulerSettings = async () => {
  schedulerSaving.value = true
  try {
    await updateSchedulerSettings({
      maxConcurrency: Number(schedulerForm.maxConcurrency),
      macCpuLimitPercent: Number(schedulerForm.macCpuLimitPercent),
      macMinAvailableMemoryRatio: Number(schedulerForm.macMinAvailableMemoryPercent) / 100
    })
    toast.add({
      title: '调度设置已保存',
      description: `最大并发 ${schedulerForm.maxConcurrency}，CPU 阈值 ${schedulerForm.macCpuLimitPercent}%，剩余内存保底 ${schedulerForm.macMinAvailableMemoryPercent}%`,
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

const getLatestAppStateForCloseCheck = async () => {
  try {
    const latestState = await invokeCommand<AppState>('get_app_state')
    appState.value = latestState
    return latestState
  } catch (error) {
    console.error('关闭前获取应用状态失败:', error)
    return appState.value
  }
}

const handleWindowCloseRequested = async (event: { preventDefault: () => void }) => {
  if (allowWindowClose.value) {
    allowWindowClose.value = false
    return
  }

  const latestState = await getLatestAppStateForCloseCheck()
  if ((latestState?.activeTaskCount ?? 0) === 0) {
    return
  }

  event.preventDefault()
  closeConfirmOpen.value = true
}

const handleConfirmClose = async () => {
  closingApp.value = true
  closeConfirmOpen.value = false
  allowWindowClose.value = true

  try {
    await closeCurrentWindow()
  } catch (error) {
    allowWindowClose.value = false
    closeConfirmOpen.value = true
    toast.add({
      title: '退出失败',
      description: error instanceof Error ? error.message : '无法关闭当前窗口',
      color: 'error'
    })
  } finally {
    closingApp.value = false
  }
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
      v-if="loading || needsLibrarySetup"
      class="fixed inset-0 z-50 flex items-center justify-center bg-default/90 p-6 backdrop-blur-sm"
    >
      <UCard class="w-full max-w-2xl">
        <template #header>
          <div class="space-y-2">
            <h2 class="text-2xl font-bold">
              {{ loading ? '正在初始化桌面环境' : '初始化媒体库' }}
            </h2>
            <p class="text-sm text-muted">
              {{ loading
                ? '请稍候，正在读取应用配置与本地数据库。'
                : '桌面端会将控制数据保存在应用目录中，大体积视频和分析产物保存在独立媒体库中，以避免更新软件时丢失历史数据。' }}
            </p>
          </div>
        </template>

        <div
          v-if="loading"
          class="py-8 text-center"
        >
          <UIcon
            name="i-lucide-loader-2"
            class="mx-auto mb-4 h-10 w-10 animate-spin"
          />
          <p class="text-muted">
            正在准备桌面运行环境...
          </p>
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
              <span>{{ Math.round((migrationProgress.progress || 0) * 100) }}%</span>
            </div>
            <UProgress
              :model-value="Math.round((migrationProgress.progress || 0) * 100)"
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
                <span>{{ Math.round((migrationProgress.progress || 0) * 100) }}%</span>
              </div>
              <UProgress
                :model-value="Math.round((migrationProgress.progress || 0) * 100)"
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
                当 CPU 或剩余内存低于阈值时，系统不会继续追加并发任务。
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
      :open="queueRecoveryState.hasPendingRecovery"
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
