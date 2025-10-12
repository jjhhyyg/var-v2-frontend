/**
 * 报告生成工具
 * 支持将任务分析结果导出为 HTML 和 PDF 格式
 */
import jsPDF from 'jspdf'
import html2canvas from 'html2canvas'
import type { Task, TaskResult, DynamicMetric, AnomalyEvent } from './useTaskApi'

export interface ReportData {
  task: Task
  result: TaskResult
  fps: number // 视频帧率
}

export interface CircularityMetric {
  frameNumber: number
  timestamp: number
  circularity: number
}

export function useReportGenerator() {
  /**
   * 计算圆度 (4π × 面积 / 周长²)
   * 圆度值范围 0-1，1 表示完美的圆形
   */
  const calculateCircularity = (area: number, perimeter: number): number => {
    if (perimeter === 0) return 0
    const circularity = (4 * Math.PI * area) / (perimeter * perimeter)
    return Math.min(circularity, 1) // 确保不超过 1
  }

  /**
   * 计算所有帧的圆度数据
   */
  const calculateCircularityMetrics = (metrics: DynamicMetric[]): CircularityMetric[] => {
    return metrics.map(m => ({
      frameNumber: m.frameNumber,
      timestamp: m.timestamp,
      circularity: calculateCircularity(m.poolArea || 0, m.poolPerimeter || 0)
    }))
  }

  /**
   * 计算平均值
   */
  const calculateAverage = (values: number[]): number => {
    if (values.length === 0) return 0
    const sum = values.reduce((acc, val) => acc + val, 0)
    return sum / values.length
  }

  /**
   * 格式化时间戳为 HH:MM:SS.mmm
   */
  const formatTimestamp = (seconds: number): string => {
    const hours = Math.floor(seconds / 3600)
    const minutes = Math.floor((seconds % 3600) / 60)
    const secs = Math.floor(seconds % 60)
    const ms = Math.floor((seconds % 1) * 1000)

    return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}.${ms.toString().padStart(3, '0')}`
  }

  /**
   * 格式化日期时间
   */
  const formatDateTime = (dateStr?: string): string => {
    if (!dateStr) return '-'
    const date = new Date(dateStr)
    return date.toLocaleString('zh-CN', {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    })
  }

  /**
   * 计算任务耗时（秒）
   */
  const calculateDuration = (startTime?: string, endTime?: string): number => {
    if (!startTime || !endTime) return 0
    const start = new Date(startTime).getTime()
    const end = new Date(endTime).getTime()
    return (end - start) / 1000
  }

  /**
   * 格式化耗时为易读格式
   */
  const formatDuration = (seconds: number): string => {
    if (seconds === 0) return '-'

    const hours = Math.floor(seconds / 3600)
    const minutes = Math.floor((seconds % 3600) / 60)
    const secs = Math.floor(seconds % 60)

    const parts: string[] = []
    if (hours > 0) parts.push(`${hours}小时`)
    if (minutes > 0) parts.push(`${minutes}分钟`)
    if (secs > 0 || parts.length === 0) parts.push(`${secs}秒`)

    return parts.join('')
  }

  /**
   * 获取异常事件的描述信息
   */
  const getEventDescription = (event: AnomalyEvent, fps: number): string => {
    const startTime = formatTimestamp(event.startFrame / fps)
    const endTime = formatTimestamp(event.endFrame / fps)
    const duration = ((event.endFrame - event.startFrame) / fps).toFixed(2)

    let description = `时段: ${startTime} - ${endTime} (${duration}秒)`

    // 特殊处理粘连物掉落事件
    if (event.eventType === 'ADHESION_DROP' && event.metadata) {
      const dropLocation = event.metadata.drop_location as string
      if (dropLocation === 'pool') {
        description += ' | 掉落位置: 熔池中'
      } else if (dropLocation === 'mold') {
        description += ' | 掉落位置: 被结晶器捕获'
      }
    }

    return description
  }

  /**
   * 获取事件类型的中文名称
   */
  const getEventTypeName = (eventType: string): string => {
    const typeMap: Record<string, string> = {
      ADHESION_FORMATION: '粘连物形成',
      ADHESION_DROP: '粘连物掉落',
      INGOT_CROWN_DROP: '锭冠脱落',
      POOL_NOT_REACH_EDGE: '熔池未到边',
      POOL_REACH_EDGE: '熔池到边',
      POOL_OVERFLOW: '熔池溢出'
    }
    return typeMap[eventType] || eventType
  }

  /**
   * 生成 HTML 报告
   */
  const generateHTMLReport = (data: ReportData): string => {
    const { task, result, fps } = data

    // 计算圆度数据
    const circularityMetrics = calculateCircularityMetrics(result.dynamicMetrics)
    const avgCircularity = calculateAverage(circularityMetrics.map(c => c.circularity))

    // 计算各项平均值
    const avgBrightness = calculateAverage(result.dynamicMetrics.map(m => m.brightness || 0))
    const avgArea = calculateAverage(result.dynamicMetrics.map(m => m.poolArea || 0))
    const avgPerimeter = calculateAverage(result.dynamicMetrics.map(m => m.poolPerimeter || 0))

    // 按类型分组异常事件
    const eventsByType = new Map<string, AnomalyEvent[]>()
    result.anomalyEvents.forEach((event) => {
      const typeName = getEventTypeName(event.eventType)
      if (!eventsByType.has(typeName)) {
        eventsByType.set(typeName, [])
      }
      eventsByType.get(typeName)!.push(event)
    })

    const html = `
<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>VAR熔池分析报告 - ${task.name}</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: "Microsoft YaHei", "SimHei", Arial, sans-serif;
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
            box-shadow: 0 2px 8px rgba(0,0,0,0.1);
        }

        .header {
            text-align: center;
            border-bottom: 3px solid #2563eb;
            padding-bottom: 20px;
            margin-bottom: 30px;
        }

        .header h1 {
            font-size: 28px;
            color: #1e40af;
            margin-bottom: 10px;
        }

        .header .subtitle {
            font-size: 14px;
            color: #6b7280;
        }

        .section {
            margin-bottom: 30px;
        }

        .section-title {
            font-size: 20px;
            font-weight: bold;
            color: #1e40af;
            border-left: 4px solid #2563eb;
            padding-left: 10px;
            margin-bottom: 15px;
        }

        .info-grid {
            display: grid;
            grid-template-columns: repeat(2, 1fr);
            gap: 15px;
            margin-bottom: 20px;
        }

        .info-item {
            display: flex;
            padding: 10px;
            background: #f9fafb;
            border-radius: 4px;
        }

        .info-label {
            font-weight: bold;
            color: #4b5563;
            min-width: 120px;
        }

        .info-value {
            color: #1f2937;
        }

        .stats-grid {
            display: grid;
            grid-template-columns: repeat(3, 1fr);
            gap: 15px;
            margin-bottom: 20px;
        }

        .stat-card {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 20px;
            border-radius: 8px;
            text-align: center;
        }

        .stat-card.blue {
            background: linear-gradient(135deg, #2563eb 0%, #1e40af 100%);
        }

        .stat-card.green {
            background: linear-gradient(135deg, #10b981 0%, #059669 100%);
        }

        .stat-card.orange {
            background: linear-gradient(135deg, #f59e0b 0%, #d97706 100%);
        }

        .stat-card.red {
            background: linear-gradient(135deg, #ef4444 0%, #dc2626 100%);
        }

        .stat-label {
            font-size: 14px;
            opacity: 0.9;
            margin-bottom: 8px;
        }

        .stat-value {
            font-size: 32px;
            font-weight: bold;
        }

        .stat-unit {
            font-size: 14px;
            opacity: 0.9;
            margin-left: 5px;
        }

        table {
            width: 100%;
            border-collapse: collapse;
            margin-bottom: 20px;
        }

        table th,
        table td {
            padding: 12px;
            text-align: left;
            border-bottom: 1px solid #e5e7eb;
        }

        table th {
            background: #f3f4f6;
            font-weight: bold;
            color: #374151;
        }

        table tbody tr:hover {
            background: #f9fafb;
        }

        .event-group {
            margin-bottom: 25px;
        }

        .event-type-header {
            background: #dbeafe;
            color: #1e40af;
            padding: 10px 15px;
            font-weight: bold;
            border-radius: 4px;
            margin-bottom: 10px;
        }

        .event-list {
            padding-left: 20px;
        }

        .event-item {
            padding: 8px 0;
            border-bottom: 1px solid #f3f4f6;
        }

        .event-item:last-child {
            border-bottom: none;
        }

        .footer {
            margin-top: 40px;
            padding-top: 20px;
            border-top: 2px solid #e5e7eb;
            text-align: center;
            color: #6b7280;
            font-size: 14px;
        }

        .no-data {
            text-align: center;
            color: #9ca3af;
            padding: 20px;
            font-style: italic;
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
        <!-- 报告头部 -->
        <div class="header">
            <h1>VAR熔池分析报告</h1>
            <div class="subtitle">生成时间: ${new Date().toLocaleString('zh-CN')}</div>
        </div>

        <!-- 基本信息 -->
        <div class="section">
            <div class="section-title">任务基本信息</div>
            <div class="info-grid">
                <div class="info-item">
                    <div class="info-label">任务名称:</div>
                    <div class="info-value">${task.name}</div>
                </div>
                <div class="info-item">
                    <div class="info-label">任务ID:</div>
                    <div class="info-value">${task.taskId}</div>
                </div>
                <div class="info-item">
                    <div class="info-label">创建时间:</div>
                    <div class="info-value">${formatDateTime(task.createdAt)}</div>
                </div>
                <div class="info-item">
                    <div class="info-label">开始时间:</div>
                    <div class="info-value">${formatDateTime(task.startedAt)}</div>
                </div>
                <div class="info-item">
                    <div class="info-label">结束时间:</div>
                    <div class="info-value">${formatDateTime(task.completedAt)}</div>
                </div>
                <div class="info-item">
                    <div class="info-label">任务耗时:</div>
                    <div class="info-value">${formatDuration(calculateDuration(task.startedAt, task.completedAt))}</div>
                </div>
            </div>
        </div>

        <!-- 视频信息 -->
        <div class="section">
            <div class="section-title">视频信息</div>
            <div class="info-grid">
                <div class="info-item">
                    <div class="info-label">视频时长:</div>
                    <div class="info-value">${task.videoDuration.toFixed(2)} 秒</div>
                </div>
                <div class="info-item">
                    <div class="info-label">视频帧率:</div>
                    <div class="info-value">${fps.toFixed(2)} FPS</div>
                </div>
                <div class="info-item">
                    <div class="info-label">总帧数:</div>
                    <div class="info-value">${result.dynamicMetrics.length} 帧</div>
                </div>
                <div class="info-item">
                    <div class="info-label">任务状态:</div>
                    <div class="info-value">${result.isTimeout ? '超时完成' : '正常完成'}</div>
                </div>
            </div>
        </div>

        <!-- 动态参数统计 -->
        <div class="section">
            <div class="section-title">动态参数统计</div>
            <div class="stats-grid">
                <div class="stat-card blue">
                    <div class="stat-label">平均亮度</div>
                    <div class="stat-value">${avgBrightness.toFixed(1)}</div>
                </div>
                <div class="stat-card green">
                    <div class="stat-label">平均面积</div>
                    <div class="stat-value">${avgArea.toFixed(0)}<span class="stat-unit">像素</span></div>
                </div>
                <div class="stat-card orange">
                    <div class="stat-label">平均周长</div>
                    <div class="stat-value">${avgPerimeter.toFixed(1)}<span class="stat-unit">像素</span></div>
                </div>
                <div class="stat-card red">
                    <div class="stat-label">平均圆度</div>
                    <div class="stat-value">${avgCircularity.toFixed(3)}</div>
                </div>
            </div>

            <p style="color: #6b7280; font-size: 14px; margin-top: 10px;">
                注: 圆度 = 4π × 面积 / 周长²，范围为 0-1，值越接近 1 表示形状越接近圆形
            </p>
        </div>

        <!-- 异常事件统计 -->
        <div class="section">
            <div class="section-title">异常事件统计</div>
            ${result.anomalyEvents.length > 0
              ? `
                ${Array.from(eventsByType.entries()).map(([typeName, events]) => `
                    <div class="event-group">
                        <div class="event-type-header">
                            ${typeName} (${events.length} 次)
                        </div>
                        <div class="event-list">
                            ${events.map((event, index) => `
                                <div class="event-item">
                                    ${index + 1}. ${getEventDescription(event, fps)}
                                </div>
                            `).join('')}
                        </div>
                    </div>
                `).join('')}
            `
              : '<div class="no-data">未检测到异常事件</div>'}
        </div>

        <!-- 追踪物体统计 -->
        <div class="section">
            <div class="section-title">追踪物体统计</div>
            ${result.trackingObjects.length > 0
              ? `
                <table>
                    <thead>
                        <tr>
                            <th>物体ID</th>
                            <th>类别</th>
                            <th>首次出现</th>
                            <th>最后出现</th>
                            <th>持续时长</th>
                        </tr>
                    </thead>
                    <tbody>
                        ${result.trackingObjects.map(obj => `
                            <tr>
                                <td>${obj.objectId}</td>
                                <td>${obj.category}</td>
                                <td>第 ${obj.firstFrame} 帧 (${formatTimestamp(obj.firstFrame / fps)})</td>
                                <td>第 ${obj.lastFrame} 帧 (${formatTimestamp(obj.lastFrame / fps)})</td>
                                <td>${((obj.lastFrame - obj.firstFrame) / fps).toFixed(2)} 秒</td>
                            </tr>
                        `).join('')}
                    </tbody>
                </table>
            `
              : '<div class="no-data">未检测到追踪物体</div>'}
        </div>

        <!-- 报告尾部 -->
        <div class="footer">
            <p>VAR熔池智能分析系统 - 自动生成报告</p>
            <p>报告ID: ${task.taskId} | 生成时间: ${new Date().toISOString()}</p>
        </div>
    </div>
</body>
</html>
    `

    return html
  }

  /**
   * 导出 HTML 报告
   */
  const exportToHTML = (data: ReportData) => {
    const html = generateHTMLReport(data)
    const blob = new Blob([html], { type: 'text/html;charset=utf-8' })
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = `VAR分析报告_${data.task.name}_${Date.now()}.html`
    document.body.appendChild(link)
    link.click()
    document.body.removeChild(link)
    URL.revokeObjectURL(url)
  }

  /**
   * 导出 PDF 报告
   */
  const exportToPDF = async (data: ReportData) => {
    // 创建临时容器
    const container = document.createElement('div')
    container.style.position = 'absolute'
    container.style.left = '-9999px'
    container.style.top = '0'
    container.style.width = '1200px'
    container.innerHTML = generateHTMLReport(data)
    document.body.appendChild(container)

    try {
      // 将 HTML 转换为 Canvas
      const canvas = await html2canvas(container, {
        scale: 2,
        useCORS: true,
        logging: false,
        backgroundColor: '#ffffff'
      })

      // 创建 PDF
      const imgWidth = 210 // A4 宽度 (mm)
      const pageHeight = 297 // A4 高度 (mm)
      const imgHeight = (canvas.height * imgWidth) / canvas.width
      let heightLeft = imgHeight
      let position = 0

      const pdf = new jsPDF('p', 'mm', 'a4')
      const imgData = canvas.toDataURL('image/png')

      // 添加第一页
      pdf.addImage(imgData, 'PNG', 0, position, imgWidth, imgHeight)
      heightLeft -= pageHeight

      // 如果内容超过一页，添加额外的页面
      while (heightLeft > 0) {
        position = heightLeft - imgHeight
        pdf.addPage()
        pdf.addImage(imgData, 'PNG', 0, position, imgWidth, imgHeight)
        heightLeft -= pageHeight
      }

      // 保存 PDF
      pdf.save(`VAR分析报告_${data.task.name}_${Date.now()}.pdf`)
    } finally {
      // 清理临时容器
      document.body.removeChild(container)
    }
  }

  return {
    calculateCircularity,
    calculateCircularityMetrics,
    calculateAverage,
    formatTimestamp,
    formatDateTime,
    formatDuration,
    getEventDescription,
    getEventTypeName,
    generateHTMLReport,
    exportToHTML,
    exportToPDF
  }
}
