<script setup lang="ts">
import type { Task, TaskResult } from '~/composables/useTaskApi'
import type { ReportData } from '~/composables/useReportGenerator'

const props = defineProps<{
  task: Task
  result: TaskResult
}>()

const { exportToHTML, exportToPDF } = useReportGenerator()
const toast = useToast()

// 计算视频帧率
const fps = computed(() => {
  if (!props.result.dynamicMetrics || props.result.dynamicMetrics.length === 0) {
    return 30 // 默认帧率
  }
  const totalFrames = props.result.dynamicMetrics.length
  const duration = props.task.videoDuration
  return totalFrames / duration
})

// 准备报告数据
const reportData = computed<ReportData>(() => ({
  task: props.task,
  result: props.result,
  fps: fps.value
}))

const exporting = ref(false)

/**
 * 导出为 HTML
 */
const handleExportHTML = () => {
  try {
    exportToHTML(reportData.value)
    toast.add({
      title: '导出成功',
      description: 'HTML 报告已成功导出',
      color: 'success'
    })
  } catch (error) {
    console.error('导出 HTML 失败:', error)
    toast.add({
      title: '导出失败',
      description: '导出 HTML 报告时发生错误',
      color: 'error'
    })
  }
}

/**
 * 导出为 PDF
 */
const handleExportPDF = async () => {
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
      description: '导出 PDF 报告时发生错误',
      color: 'error'
    })
  } finally {
    exporting.value = false
  }
}
</script>

<template>
  <div class="flex gap-3">
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
</template>
