# VAR 熔池分析桌面端 - 前端与 Tauri 外壳

简体中文 | [English](README.md)

> 当前前端以 macOS Tauri 桌面端为主，运行时通信使用 Tauri command 和 Tauri event。

## 模块职责

- 提供 Nuxt 4 + Vue 3 + Nuxt UI 的桌面端界面
- 通过 Tauri command 管理媒体库、任务数据库、任务队列和设置
- 支持批量导入视频、FIFO 队列、有限并发、重启恢复和结果展示
- 订阅 Tauri 事件更新任务状态、队列位置和详情页结果
- 编排 macOS worker、ffmpeg/ffprobe、模型和运行时资源

## 常用命令

```bash
npm install
npm run desktop:dev
npm run typecheck
```

Windows 本地 CUDA worker 环境建议使用 conda：

```powershell
conda create -n var-env -y python=3.12
conda install -n var-env -y pytorch torchvision pytorch-cuda=12.4 -c pytorch -c nvidia
conda install -n var-env -y ffmpeg
conda run -n var-env python -m pip install -r ../ai-processor/requirements-desktop-windows-cuda.txt "pyinstaller>=6.0.0"
```

模型文件需放在：

```text
../ai-processor/weights/best.pt
```

Windows 打包分两步：

```powershell
npm run desktop:windows:runtime
npm run desktop:windows:build
```

`desktop:windows:runtime` 生成 CUDA 算法包 zip；`desktop:windows:build` 生成不内置 CUDA runtime 的 NSIS 主程序安装包。首次启动或版本不匹配时，Windows App 会要求导入匹配的算法包 zip。

macOS 发布命令必须从 `frontend/` 目录运行：

```bash
npm run desktop:macos:ad-hoc
npm run desktop:macos:release-local
npm run desktop:macos:release-public
```

不要把 `tauri dev`、raw `tauri build` 和正式 macOS 发布脚本混用。可信发布入口是 `scripts/macos-release.mjs` 及对应 npm scripts。

## 关键文件

- `app/pages/index.vue`：批量导入、任务创建、任务表格、队列操作
- `app/pages/tasks/[id].vue`：任务详情、视频预览、分析结果
- `app/app.vue`：全局 Header、应用设置、重启恢复、关闭确认
- `app/composables/useTaskApi.ts`：Tauri command API 封装
- `app/composables/useTauriEvents.ts`：Tauri 任务状态和详情事件订阅封装
- `src-tauri/src/lib.rs`：桌面端数据库、队列调度、worker 管理、文件管理
- `scripts/macos-release.mjs`：macOS 发布编排
- `scripts/build-windows-runtime-package.mjs`：Windows CUDA Runtime zip 生成

## 打包资源规则

`src-tauri/resources/runtime/` 和 `src-tauri/resources/models/` 是 `npm run desktop:build-worker` 生成的打包资源，不提交到 Git。源码仓只保留 `src-tauri/resources/placeholder.txt` 维持目录结构。

macOS 发布脚本会继续把 runtime 资源打进发布产物。Windows NSIS 安装包使用 `src-tauri/tauri.windows.conf.json` 排除完整 CUDA runtime，算法包通过单独 zip 导入。

## 图标约束

新增 Nuxt Icon 后需要确认图标进入 Nuxt Icon client bundle：

```bash
node ../.codex/skills/nuxt-icon-client-bundle/scripts/verify-nuxt-icon-client-bundle.mjs
```

## 测试要求

- `npm run typecheck`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- macOS 桌面端真实界面导入视频并跑完整分析

## 下一步阅读

- `../docs/macOS桌面端发布指南.md`
- `../docs/桌面端完整功能验证清单.md`
