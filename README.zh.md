# VAR 熔池视频分析系统 - 前端应用

简体中文 | [English](README.md)

> 基于 Nuxt 4 构建的 VAR 熔池视频分析系统现代化 Web 界面

## 技术栈

- **框架**: Nuxt 4.1.2 (Vue 3)
- **语言**: TypeScript 5.9
- **UI 库**: Nuxt UI 4.0
- **图表**: ECharts 6.0 + Chart.js 4.5
- **实时通信**: STOMP.js + SockJS
- **导出**: jsPDF + html2canvas
- **包管理器**: pnpm（推荐）/ npm

## 核心功能

- 现代化、响应式 UI 设计
- 通过 WebSocket 实时监控任务进度
- 使用 ECharts 进行交互式数据可视化
- 支持拖拽上传视频文件
- 分析结果展示和导出
- 深色模式支持
- PDF 报告生成

## 环境要求

- Node.js 18+ 或 20+
- pnpm 8+（或 npm 9+）

## 快速开始

### 1. 安装依赖

使用 pnpm（推荐）：

```bash
pnpm install
```

使用 npm：

```bash
npm install
```

### 2. 配置环境变量

在 frontend 目录下创建 `.env` 文件（或使用父项目的环境配置）：

```bash
# API 基础 URL
NUXT_PUBLIC_API_BASE_URL=http://localhost:8080
NUXT_PUBLIC_WS_BASE_URL=ws://localhost:8080
```

### 3. 启动开发服务器

```bash
pnpm dev
```

应用将在 http://localhost:3000 启动

### 4. 生产环境构建

```bash
pnpm build
```

预览生产构建：

```bash
pnpm preview
```

## 开发指南

### 项目结构

```
frontend/
├── app/
│   ├── components/         # Vue 组件
│   ├── composables/        # 组合式函数
│   ├── layouts/            # 布局组件
│   ├── pages/              # 路由页面
│   ├── plugins/            # Nuxt 插件
│   ├── utils/              # 工具函数
│   └── app.vue             # 根组件
├── public/                 # 静态资源
├── .nuxt/                  # 生成的文件（自动生成）
├── nuxt.config.ts          # Nuxt 配置
├── tsconfig.json           # TypeScript 配置
└── package.json
```

### 核心技术

#### Nuxt UI

本项目使用 [Nuxt UI](https://ui.nuxt.com/) 作为组件库，提供：
- 美观、易用的组件
- 内置深色模式
- TypeScript 支持
- Tailwind CSS 集成

#### 实时更新

使用 STOMP.js 和 SockJS 建立 WebSocket 连接，实现任务进度实时更新：

```typescript
// 在 composables 中的使用示例
const { connect, disconnect } = useWebSocket()
connect('/ws/progress', (message) => {
  console.log('进度更新:', message)
})
```

#### 数据可视化

使用 ECharts 实现高级图表功能：
- 时间序列数据的折线图
- 统计数据的柱状图
- 分析结果的自定义可视化

### 代码质量

运行代码检查：

```bash
pnpm lint
```

自动修复代码问题：

```bash
pnpm lint:fix
```

类型检查：

```bash
pnpm typecheck
```

## 配置说明

### Nuxt 配置

`nuxt.config.ts` 中的主要设置：

```typescript
export default defineNuxtConfig({
  modules: ['@nuxt/ui'],

  runtimeConfig: {
    public: {
      apiBaseUrl: process.env.NUXT_PUBLIC_API_BASE_URL || 'http://localhost:8080',
      wsBaseUrl: process.env.NUXT_PUBLIC_WS_BASE_URL || 'ws://localhost:8080'
    }
  }
})
```

### 环境变量

- `NUXT_PUBLIC_API_BASE_URL` - 后端 API 基础 URL
- `NUXT_PUBLIC_WS_BASE_URL` - WebSocket 服务器 URL

## Docker 部署

构建 Docker 镜像：

```bash
docker build -t var-frontend:latest .
```

使用 Docker 运行：

```bash
docker run -p 3000:3000 \
  -e NUXT_PUBLIC_API_BASE_URL=http://api.example.com \
  var-frontend:latest
```

## 故障排查

### 端口被占用

如果 3000 端口已被占用，可以指定其他端口：

```bash
PORT=3001 pnpm dev
```

### API 连接问题

确保后端服务正在运行，且 `NUXT_PUBLIC_API_BASE_URL` 配置正确。

### WebSocket 连接失败

检查：
1. 后端 WebSocket 端点是否可访问
2. `NUXT_PUBLIC_WS_BASE_URL` 配置是否正确
3. 防火墙是否阻止了 WebSocket 连接

## 性能优化

- 使用 `pnpm build` 进行优化的生产构建
- 启用服务端渲染（SSR）以获得更好的 SEO
- 使用 `defineAsyncComponent` 懒加载组件
- 优化 `public/` 目录中的图片

## 浏览器支持

- Chrome/Edge（最新 2 个版本）
- Firefox（最新 2 个版本）
- Safari（最新 2 个版本）

## 许可证

本项目采用 GNU Affero General Public License v3.0 (AGPL-3.0) 许可证 - 详见 [LICENSE](LICENSE) 文件。

**重要提示：** 任何通过网络使用的本软件修改版本必须向用户提供源代码。
