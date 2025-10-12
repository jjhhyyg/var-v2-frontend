# 报告生成功能使用指南

## 快速开始

### 1. 查看已完成的任务

访问任务详情页面，确保任务状态为"已完成"或"已完成(超时)"。

### 2. 导出报告

在任务详情页面右上角，你会看到两个导出按钮：

- **导出 HTML 报告** - 生成可在浏览器中查看的网页报告
- **导出 PDF 报告** - 生成 PDF 文件，适合打印和存档

### 3. 查看报告

#### HTML 报告

- 下载后直接用浏览器打开
- 可以打印（Ctrl/Cmd + P）
- 支持在线分享

#### PDF 报告

- 下载后用 PDF 阅读器打开
- 适合打印和长期存档
- 文件大小通常较大（因为包含高质量图片）

## 报告内容说明

### 基本信息部分

包含任务的创建时间、开始时间、结束时间和耗时等信息。

### 视频信息部分

显示视频时长、帧率和总帧数。

### 动态参数统计

用彩色卡片展示四个关键指标的平均值：

- 🔵 平均亮度
- 🟢 平均面积
- 🟠 平均周长
- 🟣 平均圆度

### 异常事件统计

按事件类型分组显示所有异常事件，每个事件包含：

- 发生的具体时段
- 持续时长
- 特殊信息（如粘连物掉落位置）

### 追踪物体统计

列表显示所有被追踪的物体及其出现时间和持续时长。

## 高级使用

### 在代码中使用报告生成功能

如果你需要在其他页面中使用报告生成功能，可以这样做：

```vue
<script setup lang="ts">
import { useReportGenerator } from '~/composables/useReportGenerator'

const { exportToHTML, exportToPDF } = useReportGenerator()

// 准备报告数据
const reportData = {
  task: yourTaskData,
  result: yourResultData,
  fps: calculatedFps
}

// 导出 HTML
const handleExportHTML = () => {
  exportToHTML(reportData)
}

// 导出 PDF
const handleExportPDF = async () => {
  await exportToPDF(reportData)
}
</script>
```

### 添加图表可视化

在任务详情页中添加 `ReportCharts` 组件可以查看动态参数的可视化图表：

```vue
<ReportCharts
  v-if="task && result"
  :task="task"
  :result="result"
/>
```

## 常见问题

### Q: PDF 导出很慢怎么办？

A: PDF 导出需要将 HTML 转换为图片再生成 PDF，数据量大时会比较慢（10-30秒）。这是正常的，请耐心等待。如果只需要在线查看，建议使用 HTML 格式。

### Q: 能否自定义报告样式？

A: 目前报告样式是固定的，如需自定义，可以修改 `useReportGenerator.ts` 中的 `generateHTMLReport` 函数。

### Q: 报告中的圆度是什么意思？

A: 圆度 = 4π × 面积 / 周长²，范围为 0-1。值越接近 1，表示形状越接近完美圆形。这是评估熔池形状规则性的重要指标。

### Q: 粘连物掉落位置是如何判断的？

A: 系统通过连通域分析判断粘连物掉落后的位置，区分"掉落进熔池中"和"被结晶器捕获"两种情况。

### Q: 能否批量导出多个任务的报告？

A: 目前不支持批量导出，需要逐个任务手动导出。这是未来可能添加的功能。

## 技术支持

如有任何问题或建议，请联系开发团队。

---

**提示**: 为了获得最佳体验，建议使用 Chrome 或 Edge 浏览器。
