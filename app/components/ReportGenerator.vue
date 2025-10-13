<script setup lang="ts">
import type { Task, TaskResult } from '~/composables/useTaskApi'
import type { ReportData } from '~/composables/useReportGenerator'

const props = defineProps<{
  task: Task
  result: TaskResult
}>()

const { exportToHTML, exportToPDF } = useReportGenerator()
const toast = useToast()

// 验证 props
if (!props.task || !props.result) {
  console.error('ReportGenerator: 缺少必要的 task 或 result 数据')
}

// 从后端TaskConfig获取视频帧率，如果没有则使用默认值25
const fps = computed(() => {
  return props.task?.config?.frameRate ?? 25
})

// 准备报告数据
const reportData = computed<ReportData>(() => {
  if (!props.task || !props.result) {
    throw new Error('缺少必要的任务或结果数据')
  }
  return {
    task: props.task,
    result: props.result,
    fps: fps.value
  }
})

const exportingHTML = ref(false)
const exportingPDF = ref(false)
const isPreviewOpen = ref(false)

// 计算是否正在导出
const isExporting = computed(() => exportingHTML.value || exportingPDF.value)

/**
 * 等待报告预览组件完全渲染
 */
const waitForReportRender = async () => {
  // 等待 DOM 更新
  await nextTick()

  // 检查预览容器是否存在并包含内容
  let attempts = 0
  const maxAttempts = 20 // 最多等待 2 秒

  while (attempts < maxAttempts) {
    const container = document.querySelector('.report-preview-container')
    if (container && container.children.length > 0) {
      // 找到容器且有内容，再等待一小段时间确保样式应用
      await new Promise(resolve => setTimeout(resolve, 100))
      return
    }
    await new Promise(resolve => setTimeout(resolve, 100))
    attempts++
  }

  // 如果超时，仍然继续，但给出警告
  console.warn('报告预览渲染检测超时，可能导致导出不完整')
}

/**
 * 导出为 HTML
 */
const handleExportHTML = async () => {
  if (exportingHTML.value) {
    return // 防止重复点击
  }

  exportingHTML.value = true
  try {
    // 如果预览未打开，先打开预览
    if (!isPreviewOpen.value) {
      isPreviewOpen.value = true
      await waitForReportRender()
    }

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
  } finally {
    exportingHTML.value = false
  }
}

/**
 * 导出为 PDF
 */
const handleExportPDF = async () => {
  if (exportingPDF.value) {
    return // 防止重复点击
  }

  exportingPDF.value = true
  try {
    // 如果预览未打开，先打开预览
    if (!isPreviewOpen.value) {
      isPreviewOpen.value = true
      await waitForReportRender()
    }

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
    exportingPDF.value = false
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
        :loading="exportingHTML"
        :disabled="isExporting"
        @click="handleExportHTML"
      >
        导出 HTML 报告
      </UButton>

      <UButton
        icon="i-lucide-file-down"
        color="primary"
        :loading="exportingPDF"
        :disabled="isExporting"
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
              :loading="exportingHTML"
              :disabled="isExporting"
              @click="handleExportHTML"
            >
              导出 HTML
            </UButton>
            <UButton
              icon="i-lucide-file-down"
              color="primary"
              :loading="exportingPDF"
              :disabled="isExporting"
              @click="handleExportPDF"
            >
              导出 PDF
            </UButton>
          </div>
        </div>
      </template>
    </UModal>
  </div>
</template>
