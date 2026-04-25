/**
 * 报告生成工具
 * 支持将任务分析结果导出为 HTML 和 PDF 格式
 * 直接生成静态HTML，使用echarts的getDataURL方法导出图表
 */
import * as echarts from 'echarts/core'
import type { Task, TaskResult } from './useTaskApi'
import { loadChineseFontFromFile, CHINESE_FONT_CONFIG } from '~/utils/chineseFont'

export interface ReportData {
  task: Task
  result: TaskResult
  fps: number // 视频帧率
}

// 事件类型映射（与ReportPreview.vue保持一致）
const eventTypeMap: Record<string, string> = {
  POOL_NOT_REACHED: '熔池未到边',
  ADHESION: '电极粘连物',
  CROWN: '锭冠',
  GLOW: '辉光',
  SIDE_ARC: '边弧（侧弧）',
  CREEPING_ARC: '爬弧'
}

export const calculateReportAverage = (values: number[]): number => {
  if (values.length === 0) return 0
  const sum = values.reduce((acc, val) => acc + val, 0)
  return sum / values.length
}

export function useReportGenerator() {
  const { pickSavePath } = useDesktopBridge()
  const { exportReportFile } = useTaskApi()

  /**
   * 计算趋势（与ReportPreview.vue保持一致）
   */
  const calculateTrend = (values: number[]): string => {
    if (values.length < 2) return '数据不足'

    const firstHalf = values.slice(0, Math.floor(values.length / 2))
    const secondHalf = values.slice(Math.floor(values.length / 2))

    const avgFirst = calculateReportAverage(firstHalf)
    const avgSecond = calculateReportAverage(secondHalf)

    const diff = avgSecond - avgFirst
    const changePercent = (diff / avgFirst) * 100

    if (Math.abs(changePercent) < 5) {
      return '平稳'
    } else if (diff > 0) {
      return `上升 (${changePercent.toFixed(1)}%)`
    } else {
      return `下降 (${Math.abs(changePercent).toFixed(1)}%)`
    }
  }

  /**
   * 格式化帧号为时间（与ReportPreview.vue保持一致）
   */
  const formatFrameToTime = (frame: number, fps: number): string => {
    const seconds = frame / fps
    const mins = Math.floor(seconds / 60)
    const secs = Math.floor(seconds % 60)
    return `${mins}:${secs.toString().padStart(2, '0')}`
  }

  /**
   * 格式化耗时（与ReportPreview.vue保持一致）
   */
  const formatDuration = (seconds: number): string => {
    if (seconds === 0) return '-'
    const hours = Math.floor(seconds / 3600)
    const minutes = Math.floor((seconds % 3600) / 60)
    const secs = Math.floor(seconds % 60)

    if (hours > 0) {
      return `${hours}小时${minutes}分钟${secs}秒`
    }
    if (minutes > 0) {
      return `${minutes}分钟${secs}秒`
    }
    return `${secs}秒`
  }

  /**
   * 等待所有echarts图表渲染完成
   */
  const waitForChartsReady = async (): Promise<void> => {
    await nextTick()
    // 等待echarts完成渲染
    await new Promise(resolve => setTimeout(resolve, 800))
  }

  /**
   * 从DOM中获取所有echarts实例并导出为base64图片
   */
  const getChartImages = async (): Promise<string[]> => {
    const chartContainers = document.querySelectorAll('.metrics-chart-container')
    const images: string[] = []

    for (const container of Array.from(chartContainers)) {
      let chartInstance = null

      // 尝试多种方式查找echarts实例
      // 方式1: 直接在容器上查找
      chartInstance = echarts.getInstanceByDom(container as HTMLElement)

      // 方式2: 在容器的第一个子元素上查找（VChart通常渲染为第一个子div）
      if (!chartInstance && container.firstElementChild) {
        chartInstance = echarts.getInstanceByDom(container.firstElementChild as HTMLElement)
      }

      // 方式3: 查找包含canvas的元素
      if (!chartInstance) {
        const canvas = container.querySelector('canvas')
        if (canvas && canvas.parentElement) {
          chartInstance = echarts.getInstanceByDom(canvas.parentElement)
        }
      }

      if (chartInstance) {
        try {
          const base64 = chartInstance.getDataURL({
            type: 'png',
            pixelRatio: 2,
            backgroundColor: '#ffffff'
          })
          images.push(base64)
        } catch (error) {
          console.error('获取图表图片失败:', error)
        }
      } else {
        console.warn('未找到图表实例:', container)
      }
    }

    return images
  }

  /**
   * 生成图表HTML（直接嵌入base64图片）
   */
  const generateChartHTML = (imageBase64: string, title: string, average: string, trend: string, unit: string): string => {
    return `
      <div>
        <div class="flex justify-between items-center mb-3">
          <h4 class="text-base font-medium">${title}</h4>
          <div class="text-sm text-muted">
            <span class="mr-4"><strong>${average}</strong> ${unit}</span>
            <span>变化趋势: <strong>${trend}</strong></span>
          </div>
        </div>
        <div class="metrics-chart-container">
          <img src="${imageBase64}">
        </div>
        <hr>
      </div>
    `
  }

  /**
   * 导出为 HTML
   * 直接生成静态HTML，数据处理逻辑与ReportPreview.vue完全一致
   */
  const exportToHTML = async (data: ReportData) => {
    try {
      // 等待图表渲染完成
      await waitForChartsReady()

      // 获取所有图表的base64图片
      const chartImages = await getChartImages()

      // 计算任务耗时
      let taskDuration = 0
      if (data.task.startedAt && data.task.completedAt) {
        const start = new Date(data.task.startedAt).getTime()
        const end = new Date(data.task.completedAt).getTime()
        taskDuration = (end - start) / 1000
      }

      // 计算各项平均值和趋势（与ReportPreview.vue保持一致）
      const brightnessValues = data.result.dynamicMetrics.map(m => m.brightness || 0)
      const areaValues = data.result.dynamicMetrics.map(m => m.poolArea || 0)
      const perimeterValues = data.result.dynamicMetrics.map(m => m.poolPerimeter || 0)

      const avgBrightness = calculateReportAverage(brightnessValues)
      const avgArea = calculateReportAverage(areaValues)
      const avgPerimeter = calculateReportAverage(perimeterValues)

      const brightnessTrend = calculateTrend(brightnessValues)
      const areaTrend = calculateTrend(areaValues)
      const perimeterTrend = calculateTrend(perimeterValues)

      // 按时间段分组事件（与ReportPreview.vue保持一致）
      const groupedEvents = data.result.anomalyEvents.map((event) => {
        const startTime = formatFrameToTime(event.startFrame, data.fps)
        const endTime = formatFrameToTime(event.endFrame, data.fps)
        const durationFrames = event.endFrame - event.startFrame + 1
        const durationSeconds = durationFrames / data.fps
        const duration = durationSeconds.toFixed(1) + '秒'

        return {
          eventType: eventTypeMap[event.eventType] || event.eventType,
          startTime,
          endTime,
          duration,
          metadata: event.metadata
        }
      })

      // 生成完整的HTML文档
      const htmlContent = `
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>熔池分析报告 - ${data.task.name}</title>
  <style>
    * {
      margin: 0;
      padding: 0;
      box-sizing: border-box;
    }

    body {
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
      line-height: 1.6;
      color: #333;
      background: #f5f5f5;
      padding: 20px;
    }

    .container {
      max-width: 1200px;
      margin: 0 auto;
      background: white;
      padding: 40px;
      border-radius: 8px;
      box-shadow: 0 2px 8px rgba(0,0,0,0.1);
    }

    h1 {
      text-align: center;
      color: #1a1a1a;
      margin-bottom: 10px;
      font-size: 28px;
    }

    h2 {
      color: #2c3e50;
      margin-top: 30px;
      margin-bottom: 15px;
      padding-bottom: 10px;
      border-bottom: 2px solid #3b82f6;
      font-size: 20px;
    }

    h3 {
      color: #34495e;
      margin-top: 20px;
      margin-bottom: 10px;
      font-size: 18px;
      font-weight: 600;
    }

    h4 {
      color: #475569;
      margin-top: 15px;
      margin-bottom: 8px;
      font-size: 16px;
      font-weight: 500;
    }

    .space-y-6 > * + * {
      margin-top: 1.5rem;
    }

    .space-y-8 > * + * {
      margin-top: 2rem;
    }

    .grid {
      display: grid;
      gap: 1rem;
    }

    .grid-cols-2 {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .grid-cols-4 {
      grid-template-columns: repeat(4, minmax(0, 1fr));
    }

    @media (min-width: 768px) {
      .md\\:grid-cols-4 {
        grid-template-columns: repeat(4, minmax(0, 1fr));
      }
    }

    .card {
      background: white;
      border-radius: 8px;
      box-shadow: 0 1px 3px rgba(0,0,0,0.1);
      padding: 1.5rem;
      margin-bottom: 1.5rem;
      border: 1px solid #e2e8f0;
    }

    .card-header {
      font-size: 18px;
      font-weight: 600;
      margin-bottom: 1rem;
      padding-bottom: 0.5rem;
      border-bottom: 1px solid #e2e8f0;
    }

    .text-muted {
      color: #64748b;
      font-size: 12px;
    }

    .font-medium {
      font-weight: 500;
    }

    .font-semibold {
      font-weight: 600;
    }

    .text-center {
      text-align: center;
    }

    .text-sm {
      font-size: 14px;
    }

    .text-base {
      font-size: 16px;
    }

    .text-lg {
      font-size: 18px;
    }

    .text-2xl {
      font-size: 24px;
    }

    .mb-1 {
      margin-bottom: 0.25rem;
    }

    .mb-2 {
      margin-bottom: 0.5rem;
    }

    .mb-3 {
      margin-bottom: 0.75rem;
    }

    .mb-4 {
      margin-bottom: 1rem;
    }

    .mr-4 {
      margin-right: 1rem;
    }

    .flex {
      display: flex;
    }

    .justify-between {
      justify-content: space-between;
    }

    .items-center {
      align-items: center;
    }

    .overflow-x-auto {
      overflow-x: auto;
    }

    table {
      width: 100%;
      border-collapse: collapse;
      margin: 20px 0;
    }

    th, td {
      padding: 12px 16px;
      text-align: left;
      border-bottom: 1px solid #e2e8f0;
    }

    thead {
      background: #f8fafc;
    }

    th {
      font-weight: 600;
      color: #64748b;
      font-size: 12px;
      text-transform: uppercase;
    }

    td {
      font-size: 14px;
    }

    .text-red-600 {
      color: #dc2626;
    }

    .text-green-600 {
      color: #16a34a;
    }

    .text-yellow-600 {
      color: #ca8a04;
    }

    .text-orange-600 {
      color: #ea580c;
    }

    .metrics-chart-container {
      width: 100%;
      margin: 1rem 0;
    }

    .metrics-chart-container img {
      width: 100%;
      height: auto;
      display: block;
    }

    @media print {
      body {
        background: white;
        padding: 0;
      }
      .container {
        box-shadow: none;
        padding: 20px;
      }
    }
  </style>
</head>
<body>
  <div class="container">
    <div class="space-y-6">
      <!-- 报告标题 -->
      <div class="card">
        <div class="text-center">
          <h1 class="text-2xl font-bold mb-2">熔池分析报告</h1>
          <p class="text-muted">${data.task.name}</p>
        </div>
        <div class="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm" style="margin-top: 1.5rem;">
          <div>
            <p class="text-muted mb-1">任务名称</p>
            <p class="font-medium">${data.task.name}</p>
          </div>
          <div>
            <p class="text-muted mb-1">视频时长</p>
            <p class="font-medium">${data.task.videoDuration.toFixed(1)}秒</p>
          </div>
          <div>
            <p class="text-muted mb-1">视频帧率</p>
            <p class="font-medium">${data.fps} FPS</p>
          </div>
          <div>
            <p class="text-muted mb-1">任务创建时间</p>
            <p class="font-medium">${new Date(data.task.createdAt).toLocaleString('zh-CN')}</p>
          </div>
          <div>
            <p class="text-muted mb-1">任务开始时间</p>
            <p class="font-medium">${data.task.startedAt ? new Date(data.task.startedAt).toLocaleString('zh-CN') : '-'}</p>
          </div>
          <div>
            <p class="text-muted mb-1">任务结束时间</p>
            <p class="font-medium">${data.task.completedAt ? new Date(data.task.completedAt).toLocaleString('zh-CN') : '-'}</p>
          </div>
          <div>
            <p class="text-muted mb-1">任务耗时</p>
            <p class="font-medium">${formatDuration(taskDuration)}</p>
          </div>
        </div>
      </div>

      <!-- 动态参数统计 -->
      <div class="card">
        <h3 class="card-header">动态参数统计</h3>
        <div class="space-y-8">
          <!-- 熔池亮度 -->
          ${chartImages[0]
            ? generateChartHTML(
                chartImages[0],
                '<span class="text-yellow-600">熔池亮度</span>',
                avgBrightness.toFixed(1),
                brightnessTrend,
                '灰度值'
              )
            : ''}

          <!-- 熔池面积 -->
          ${chartImages[1]
            ? generateChartHTML(
                chartImages[1],
                '<span class="text-green-600">熔池面积</span>',
                avgArea.toFixed(0),
                areaTrend,
                '像素'
              )
            : ''}

          <!-- 熔池周长 -->
          ${chartImages[2]
            ? generateChartHTML(
                chartImages[2],
                '<span class="text-orange-600">熔池周长</span>',
                avgPerimeter.toFixed(1),
                perimeterTrend,
                '像素'
              )
            : ''}
        </div>
      </div>

      ${groupedEvents.length > 0
        ? `
      <!-- 异常事件统计 -->
      <div class="card">
        <h3 class="card-header">异常事件统计</h3>
        <div class="overflow-x-auto">
          <table>
            <thead>
              <tr>
                <th>事件类型</th>
                <th>开始时间</th>
                <th>结束时间</th>
                <th>持续时间</th>
                <th>详细信息</th>
              </tr>
            </thead>
            <tbody>
              ${groupedEvents.map(event => `
                <tr>
                  <td class="font-medium">${event.eventType}</td>
                  <td class="text-muted">${event.startTime}</td>
                  <td class="text-muted">${event.endTime}</td>
                  <td class="text-muted">${event.duration}</td>
                  <td class="text-muted">
                    ${event.metadata ? JSON.stringify(event.metadata) : '-'}
                  </td>
                </tr>
              `).join('')}
            </tbody>
          </table>
        </div>
      </div>
      `
        : ''}

      <!-- 报告生成时间 -->
      <div class="card">
        <div class="text-center text-sm text-muted">
          <p>报告生成时间: ${new Date().toLocaleString('zh-CN')}</p>
        </div>
      </div>
    </div>
  </div>
</body>
</html>
      `

      const defaultPath = `熔池分析报告_${data.task.name}_${new Date().getTime()}.html`
      const savePath = await pickSavePath(defaultPath, [
        {
          name: 'HTML 报告',
          extensions: ['html']
        }
      ])

      if (!savePath) {
        return
      }

      await exportReportFile(savePath, {
        textContent: htmlContent
      })
    } catch (error) {
      console.error('生成 HTML 失败:', error)
      throw new Error('生成 HTML 时发生错误')
    }
  }

  /**
   * 加载中文字体到 jsPDF
   */
  const loadChineseFont = async (pdf: import('jspdf').jsPDF): Promise<boolean> => {
    try {
      // 尝试从 public 目录加载字体文件
      const fontBase64 = await loadChineseFontFromFile(CHINESE_FONT_CONFIG.fontPath)

      // 添加字体到 VFS
      pdf.addFileToVFS(CHINESE_FONT_CONFIG.fontFileName, fontBase64)

      // 注册字体
      pdf.addFont(
        CHINESE_FONT_CONFIG.fontFileName,
        CHINESE_FONT_CONFIG.fontName,
        CHINESE_FONT_CONFIG.fontStyle
      )

      console.log('中文字体加载成功')
      return true
    } catch (error) {
      console.warn('中文字体加载失败，将使用默认字体（可能无法显示中文）:', error)
      return false
    }
  }

  /**
   * 获取图片的真实尺寸
   */
  const getImageDimensions = (base64: string): Promise<{ width: number, height: number }> => {
    return new Promise((resolve, reject) => {
      const img = new Image()
      img.onload = () => {
        resolve({ width: img.width, height: img.height })
      }
      img.onerror = reject
      img.src = base64
    })
  }

  /**
   * 导出为 PDF
   * 使用jsPDF将HTML内容转换为PDF
   */
  const exportToPDF = async (data: ReportData) => {
    try {
      const { jsPDF } = await import('jspdf')

      // 等待图表渲染完成
      await waitForChartsReady()

      // 获取所有图表的base64图片
      const chartImages = await getChartImages()

      if (chartImages.length === 0) {
        throw new Error('无法获取图表图片，请确保图表已正确渲染')
      }

      // 计算各项平均值和趋势
      const brightnessValues = data.result.dynamicMetrics.map(m => m.brightness || 0)
      const areaValues = data.result.dynamicMetrics.map(m => m.poolArea || 0)
      const perimeterValues = data.result.dynamicMetrics.map(m => m.poolPerimeter || 0)

      const avgBrightness = calculateReportAverage(brightnessValues)
      const avgArea = calculateReportAverage(areaValues)
      const avgPerimeter = calculateReportAverage(perimeterValues)

      const brightnessTrend = calculateTrend(brightnessValues)
      const areaTrend = calculateTrend(areaValues)
      const perimeterTrend = calculateTrend(perimeterValues)

      // 创建 PDF
      const pdf = new jsPDF({
        orientation: 'portrait',
        unit: 'mm',
        format: 'a4'
      })

      // 加载中文字体
      const fontLoaded = await loadChineseFont(pdf)

      // 设置字体（如果加载成功则使用中文字体，否则使用默认字体）
      if (fontLoaded) {
        pdf.setFont(CHINESE_FONT_CONFIG.fontName, CHINESE_FONT_CONFIG.fontStyle)
      }

      const pageWidth = 210
      const margin = 15
      const contentWidth = pageWidth - 2 * margin
      let yPosition = margin

      // 添加标题
      pdf.setFontSize(20)
      pdf.text('熔池分析报告', pageWidth / 2, yPosition, { align: 'center' })
      yPosition += 10

      pdf.setFontSize(12)
      pdf.text(data.task.name, pageWidth / 2, yPosition, { align: 'center' })
      yPosition += 15

      // 计算任务耗时
      let taskDuration = 0
      if (data.task.startedAt && data.task.completedAt) {
        const start = new Date(data.task.startedAt).getTime()
        const end = new Date(data.task.completedAt).getTime()
        taskDuration = (end - start) / 1000
      }

      // 添加基本信息
      pdf.setFontSize(10)
      const info = [
        `任务名称: ${data.task.name}`,
        `视频时长: ${data.task.videoDuration.toFixed(1)}秒`,
        `视频帧率: ${data.fps} FPS`,
        `任务创建时间: ${new Date(data.task.createdAt).toLocaleString('zh-CN')}`,
        `任务开始时间: ${data.task.startedAt ? new Date(data.task.startedAt).toLocaleString('zh-CN') : '-'}`,
        `任务结束时间: ${data.task.completedAt ? new Date(data.task.completedAt).toLocaleString('zh-CN') : '-'}`,
        `任务耗时: ${formatDuration(taskDuration)}`
      ]

      info.forEach((line) => {
        pdf.text(line, margin, yPosition)
        yPosition += 7
      })

      yPosition += 10

      // 添加图表
      const charts = [
        { title: '熔池亮度', avg: avgBrightness.toFixed(1), trend: brightnessTrend, unit: '灰度值', image: chartImages[0] },
        { title: '熔池面积', avg: avgArea.toFixed(0), trend: areaTrend, unit: '像素', image: chartImages[1] },
        { title: '熔池周长', avg: avgPerimeter.toFixed(1), trend: perimeterTrend, unit: '像素', image: chartImages[2] }
      ]

      for (const chart of charts) {
        if (chart.image) {
          // 获取图片的真实尺寸
          const dimensions = await getImageDimensions(chart.image)

          // 计算保持原始长宽比的图片尺寸
          const imgWidth = contentWidth
          const aspectRatio = dimensions.height / dimensions.width
          const imgHeight = imgWidth * aspectRatio

          // 检查是否需要新页面（考虑图片高度）
          if (yPosition + imgHeight + 15 > 280) {
            pdf.addPage()
            yPosition = margin
          }

          pdf.setFontSize(12)
          pdf.text(chart.title, margin, yPosition)
          yPosition += 7

          pdf.setFontSize(9)
          pdf.text(`${chart.avg} ${chart.unit}  变化趋势: ${chart.trend}`, margin, yPosition)
          yPosition += 5

          // 添加图表图片，使用真实长宽比
          pdf.addImage(chart.image, 'PNG', margin, yPosition, imgWidth, imgHeight)
          yPosition += imgHeight + 10
        }
      }

      // 添加异常事件统计（与ReportPreview.vue保持一致）
      const groupedEvents = data.result.anomalyEvents.map((event) => {
        const startTime = formatFrameToTime(event.startFrame, data.fps)
        const endTime = formatFrameToTime(event.endFrame, data.fps)
        const durationFrames = event.endFrame - event.startFrame + 1
        const durationSeconds = durationFrames / data.fps
        const duration = durationSeconds.toFixed(1) + '秒'

        // 格式化详细信息
        let details = '-'
        if (event.metadata) {
          details = JSON.stringify(event.metadata)
        }

        return {
          eventType: eventTypeMap[event.eventType] || event.eventType,
          startTime,
          endTime,
          duration,
          details
        }
      })

      if (groupedEvents.length > 0) {
        // 检查是否需要新页面
        if (yPosition > 200) {
          pdf.addPage()
          yPosition = margin
        }

        // 添加标题
        pdf.setFontSize(14)
        pdf.text('异常事件统计', margin, yPosition)
        yPosition += 10

        // 添加表头
        pdf.setFontSize(9)
        const colWidths = [35, 25, 25, 25, 70] // 列宽度
        const headers = ['事件类型', '开始时间', '结束时间', '持续时间', '详细信息']
        let xPosition = margin

        headers.forEach((header, index) => {
          pdf.text(header, xPosition, yPosition)
          xPosition += colWidths[index] ?? 0
        })

        yPosition += 6

        // 添加分隔线
        pdf.setLineWidth(0.5)
        pdf.line(margin, yPosition, pageWidth - margin, yPosition)
        yPosition += 5

        // 添加事件数据
        pdf.setFontSize(8)
        for (const event of groupedEvents) {
          // 检查是否需要新页面
          if (yPosition > 270) {
            pdf.addPage()
            yPosition = margin

            // 重新添加表头
            pdf.setFontSize(9)
            xPosition = margin
            headers.forEach((header, index) => {
              pdf.text(header, xPosition, yPosition)
              xPosition += colWidths[index] ?? 0
            })
            yPosition += 6
            pdf.setLineWidth(0.5)
            pdf.line(margin, yPosition, pageWidth - margin, yPosition)
            yPosition += 5
            pdf.setFontSize(8)
          }

          xPosition = margin

          // 事件类型
          pdf.text(event.eventType, xPosition, yPosition)
          xPosition += colWidths[0] ?? 0

          // 开始时间
          pdf.text(event.startTime, xPosition, yPosition)
          xPosition += colWidths[1] ?? 0

          // 结束时间
          pdf.text(event.endTime, xPosition, yPosition)
          xPosition += colWidths[2] ?? 0

          // 持续时间
          pdf.text(event.duration, xPosition, yPosition)
          xPosition += colWidths[3] ?? 0

          // 详细信息（处理长文本）
          const maxWidth = (colWidths[4] ?? 0) - 5
          const detailsLines = pdf.splitTextToSize(event.details, maxWidth)
          pdf.text(detailsLines, xPosition, yPosition)

          yPosition += Math.max(6, detailsLines.length * 4)
        }
      }

      const defaultPath = `熔池分析报告_${data.task.name}_${new Date().getTime()}.pdf`
      const savePath = await pickSavePath(defaultPath, [
        {
          name: 'PDF 报告',
          extensions: ['pdf']
        }
      ])

      if (!savePath) {
        return
      }

      const pdfArrayBuffer = pdf.output('arraybuffer')
      const pdfBytes = new Uint8Array(pdfArrayBuffer)
      let binary = ''
      const chunkSize = 0x8000

      for (let index = 0; index < pdfBytes.length; index += chunkSize) {
        const chunk = pdfBytes.subarray(index, index + chunkSize)
        binary += String.fromCharCode(...chunk)
      }

      await exportReportFile(savePath, {
        base64Content: btoa(binary)
      })
    } catch (error) {
      console.error('生成 PDF 失败:', error)
      throw new Error('生成 PDF 时发生错误')
    }
  }

  return {
    calculateAverage: calculateReportAverage,
    exportToHTML,
    exportToPDF
  }
}
