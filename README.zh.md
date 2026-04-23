# VAR 熔池视频分析系统 - 前端应用

简体中文 | [English](README.md)

> 负责视频上传、任务监控、结果展示和报告相关 UI 的 Nuxt 前端。

## 模块职责

前端主要负责：

- 上传视频并创建任务
- 展示任务列表和任务状态
- 展示任务详情、动态指标、异常事件、追踪对象
- 订阅后端 WebSocket 实时更新
- 预览原视频、预处理视频和结果视频

## 技术栈

- Nuxt 4
- Vue 3
- TypeScript
- Nuxt UI
- STOMP.js + SockJS
- ECharts + Chart.js

## 当前真实配置口径

前端当前读取的后端基础地址变量是：

- `NUXT_PUBLIC_API_BASE`

它会在 `nuxt.config.ts` 中映射到 `runtimeConfig.public.apiBase`。

不要再沿用过时的变量名，例如 `NUXT_PUBLIC_API_BASE_URL` 或 `NUXT_PUBLIC_WS_BASE_URL`。

应从主仓库根目录生成 `frontend/.env`：

```bash
./scripts/use-env.sh dev
```

本地典型值：

```bash
NUXT_PUBLIC_API_BASE=http://localhost:8080
```

## 本地开发

安装依赖：

```bash
npm install
```

启动开发服务器：

```bash
npm run dev
```

默认本地地址：

- `http://localhost:3000`

## WebStorm

本地前端开发推荐使用 WebStorm：

1. 打开 `frontend` 目录
2. 配置 Node.js 解释器
3. 执行 `npm install`
4. 确认 `frontend/.env` 已存在
5. 运行 `npm run dev`
6. 结合浏览器开发者工具查看接口请求、状态更新与 WebSocket

WebStorm 特别适合：

- 调整页面和组件
- 观察接口请求
- 定位前端运行时错误
- 观察 WebSocket 是否建立成功

## 常用命令

代码检查：

```bash
npm run lint
```

类型检查：

```bash
npm run typecheck
```

静态生产构建：

```bash
npm run generate
```

macOS 桌面端发布：

```bash
npm run desktop:macos:ad-hoc
npm run desktop:macos:release-local
npm run desktop:macos:release-public
```

`tauri dev`、raw `tauri build` 与仓库发布编排脚本的职责边界，见 `../docs/macOS桌面端发布指南.md`。

## 关键文件

- `app/pages/index.vue`：任务列表与上传入口
- `app/pages/tasks/[id].vue`：任务详情页
- `app/composables/useTaskApi.ts`：REST API 封装
- `app/composables/useWebSocket.ts`：WebSocket 订阅
- `nuxt.config.ts`：运行时配置与构建配置

## 运行时关键事实

- 前端使用同一个后端基础地址处理 HTTP 与 WebSocket
- WebSocket 入口是 `/ws`
- UI 默认订阅 `/topic/tasks/*`
- 当前前端配置为 `ssr: false`

## 测试要求

部署前至少完成这些本地检查：

- `npm run lint`
- `npm run typecheck`
- 首页能正常打开
- 任务列表请求成功
- WebSocket 能正常连接
- 任务详情页能展示来自后端的真实数据

## Docker 说明

生产环境前端镜像由主仓库统一构建，特点是：

- 构建阶段执行静态生成
- 运行阶段由 Nginx 提供静态资源
- 生产环境通常使用 `NUXT_PUBLIC_API_BASE=""`，依赖同域反向代理

前端构建成功不代表整套系统已经可部署，完整联调测试仍然是强制要求。

## 下一步阅读

- 主仓库地址：
  `https://github.com/jjhhyyg/VAR-melting-defect-detection-source-code.git`
- 主仓库中的交接文档：
  `docs/项目接手、开发测试与部署指南.md`
