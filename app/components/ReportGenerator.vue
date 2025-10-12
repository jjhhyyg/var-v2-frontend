<script setup lang="ts">
import type { Task, TaskResult } from '~/composables/useTaskApi'
import type { ReportData } from '~/composables/useReportGenerator'

const props = defineProps<{
  task: Task
  result: TaskResult
}>()

const { exportToHTML, exportToPDF } = useReportGenerator()
const toast = useToast()

// 从后端TaskConfig获取视频帧率，如果没有则使用默认值25
const fps = computed(() => {
  return props.task.config?.frameRate ?? 25
})

// 准备报告数据
const reportData = computed<ReportData>(() => ({
  task: props.task,
  result: props.result,
  fps: fps.value
}))

const exporting = ref(false)
const isPreviewOpen = ref(false)

/**
 * 导出为 HTML
 */
const handleExportHTML = async () => {
  // 如果预览未打开，先打开预览
  if (!isPreviewOpen.value) {
    isPreviewOpen.value = true
    // 等待 DOM 更新
    await nextTick()
    // 再等待一小段时间确保组件完全渲染
    await new Promise(resolve => setTimeout(resolve, 500))
  }

  try {
    await exportToHTML(reportData.value)
    toast.add({
      title: '导出成功',
      description: 'HTML 报告已成功导出',
      color: 'success'
    })
  } catch (error) {
    console.error('导出 HTML 失败:', error)
    toast.add({
      title: '导出失败',
      description: error instanceof Error ? error.message : '导出 HTML 报告时发生错误',
      color: 'error'
    })
  }
}

/**
 * 导出为 PDF
 */
const handleExportPDF = async () => {
  // 如果预览未打开，先打开预览
  if (!isPreviewOpen.value) {
    isPreviewOpen.value = true
    // 等待 DOM 更新
    await nextTick()
    // 再等待一小段时间确保组件完全渲染
    await new Promise(resolve => setTimeout(resolve, 500))
  }

  exporting.value = true
  try {
    await exportToPDF(reportData.value)
    toast.add({
      title: '导出成功',
      description: 'PDF 报告已成功导出',
      color: 'success'
    })
  } catch (error) {
    console.error('导出 PDF 失败:', error)
    toast.add({
      title: '导出失败',
      description: error instanceof Error ? error.message : '导出 PDF 报告时发生错误',
      color: 'error'
    })
  } finally {
    exporting.value = false
  }
}
</script>

<template>
  <div class="space-y-4">
    <!-- 操作按钮 -->
    <div class="flex gap-3">
      <UButton
        icon="i-lucide-eye"
        color="neutral"
        variant="outline"
        @click="isPreviewOpen = true"
      >
        预览报告
      </UButton>

      <UButton
        icon="i-lucide-file-text"
        color="primary"
        variant="outline"
        :disabled="exporting"
        @click="handleExportHTML"
      >
        导出 HTML 报告
      </UButton>

      <UButton
        icon="i-lucide-file-down"
        color="primary"
        :loading="exporting"
        :disabled="exporting"
        @click="handleExportPDF"
      >
        导出 PDF 报告
      </UButton>
    </div>

    <!-- 报告预览模态框 -->
    <UModal
      v-model:open="isPreviewOpen"
      title="报告预览"
      description="导出前预览完整报告内容"
      :fullscreen="true"
    >
      <template #body>
        <div class="p-6 overflow-y-auto max-h-[calc(100vh-200px)] report-preview-container">
          <ReportPreview :task="task" :result="result" />
        </div>
      </template>

      <template #footer>
        <div class="flex justify-between items-center w-full">
          <div class="text-sm text-muted">
            <UIcon name="i-lucide-info" class="w-4 h-4 inline mr-1" />
            滚动查看完整报告内容
          </div>
          <div class="flex gap-3">
            <UButton
              color="neutral"
              variant="outline"
              @click="isPreviewOpen = false"
            >
              关闭
            </UButton>
            <UButton
              icon="i-lucide-file-text"
              color="primary"
              variant="outline"
              @click="handleExportHTML(); isPreviewOpen = false"
            >
              导出 HTML
            </UButton>
            <UButton
              icon="i-lucide-file-down"
              color="primary"
              :loading="exporting"
              :disabled="exporting"
              @click="handleExportPDF(); isPreviewOpen = false"
            >
              导出 PDF
            </UButton>
          </div>
        </div>
      </template>
    </UModal>
  </div>
</template>
