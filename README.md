# VAR Molten Pool Analysis System - Frontend

[简体中文](README.zh.md) | English

> Nuxt frontend for video upload, task monitoring, result display, and report-related UI.

## Responsibilities

The frontend is responsible for:

- uploading videos and creating tasks
- listing tasks and showing task status
- displaying task details, metrics, events, and tracked objects
- subscribing to backend WebSocket updates
- previewing original, preprocessed, and result videos

## Tech Stack

- Nuxt 4
- Vue 3
- TypeScript
- Nuxt UI
- STOMP.js + SockJS
- ECharts + Chart.js

## Current Configuration

The frontend currently reads the backend base URL from:

- `NUXT_PUBLIC_API_BASE`

This value is exposed through `runtimeConfig.public.apiBase` in `nuxt.config.ts`.

Do not rely on outdated names such as `NUXT_PUBLIC_API_BASE_URL` or `NUXT_PUBLIC_WS_BASE_URL`.

Generate `frontend/.env` from the main repository root:

```bash
./scripts/use-env.sh dev
```

Typical local value:

```bash
NUXT_PUBLIC_API_BASE=http://localhost:8080
```

## Local Development

Install dependencies:

```bash
npm install
```

Run the development server:

```bash
npm run dev
```

Default local URL:

- `http://localhost:3000`

## WebStorm

WebStorm is the recommended IDE for local frontend work:

1. Open the `frontend` directory
2. Configure the Node.js interpreter
3. Run `npm install`
4. Make sure `frontend/.env` already exists
5. Run `npm run dev`
6. Use the browser and developer tools to inspect API requests, state updates, and WebSocket behavior

WebStorm is especially useful for:

- UI changes
- component debugging
- checking API requests
- checking WebSocket connection and client-side errors

## Useful Commands

Lint:

```bash
npm run lint
```

Type check:

```bash
npm run typecheck
```

Static production build:

```bash
npm run generate
```

macOS desktop release:

```bash
npm run desktop:macos:ad-hoc
npm run desktop:macos:release-local
npm run desktop:macos:release-public
```

For the responsibility split between `tauri dev`, raw `tauri build`, and the repository release orchestrator, read `../docs/macOS桌面端发布指南.md`.

## Key Files

- `app/pages/index.vue`: task list and upload page
- `app/pages/tasks/[id].vue`: task detail page
- `app/composables/useTaskApi.ts`: REST API access
- `app/composables/useWebSocket.ts`: WebSocket subscriptions
- `nuxt.config.ts`: runtime config and frontend build config

## Important Runtime Behavior

- The frontend uses the backend base URL for both HTTP and WebSocket access
- The WebSocket endpoint is `/ws`
- The UI expects task status topics under `/topic/tasks/*`
- This project currently runs with `ssr: false`

## Testing Expectations

Minimum local checks before deployment:

- `npm run lint`
- `npm run typecheck`
- verify the homepage loads
- verify task list requests succeed
- verify WebSocket connects
- verify task detail pages render real backend data

## Docker Notes

The production Docker image is built from the main repository and uses:

- static generation during build
- Nginx for serving frontend assets
- `NUXT_PUBLIC_API_BASE=""` in production when frontend and backend are reverse proxied under the same origin

Do not treat a successful frontend build as proof that the whole system is ready. Full local integration testing is still required.

## What to Read Next

- Main repository overview:
  `https://github.com/jjhhyyg/VAR-melting-defect-detection-source-code.git`
- Main handover guide in the root repository:
  `docs/项目接手、开发测试与部署指南.md`
