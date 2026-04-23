# VAR Molten Pool Analysis Desktop Frontend

[简体中文](README.zh.md) | English

> The current frontend targets the macOS Tauri desktop app. It no longer depends on the legacy Spring Boot backend, WebSocket service, Docker, or Nginx deployment path.

## Responsibilities

- Provide the Nuxt 4 + Vue 3 + Nuxt UI desktop interface
- Manage the media library, task database, task queue, and settings through Tauri commands
- Support batch video import, FIFO queueing, limited concurrency, restart recovery, and result display
- Subscribe to Tauri events for task status, queue positions, and detail updates
- Coordinate the macOS worker, ffmpeg/ffprobe, model, and runtime resources

## Commands

```bash
npm install
npm run desktop:dev
npm run typecheck
```

macOS release commands must be run from `frontend/`:

```bash
npm run desktop:macos:ad-hoc
npm run desktop:macos:release-local
npm run desktop:macos:release-public
```

Do not mix `tauri dev`, raw `tauri build`, and the formal macOS release scripts. The trusted release entrypoint is `scripts/macos-release.mjs` and the matching npm scripts.

## Key Files

- `app/pages/index.vue`: batch import, task creation, task table, queue actions
- `app/pages/tasks/[id].vue`: task details, video preview, analysis results
- `app/app.vue`: global header, app settings, restart recovery, close confirmation
- `app/composables/useTaskApi.ts`: Tauri command API wrapper
- `app/composables/useWebSocket.ts`: Tauri event subscription wrapper; the name is retained but it is no longer WebSocket
- `src-tauri/src/lib.rs`: desktop database, queue scheduler, worker management, file management
- `scripts/macos-release.mjs`: macOS release orchestration

## Icon Rule

After adding a Nuxt Icon, verify that it is included in the Nuxt Icon client bundle:

```bash
node ../.codex/skills/nuxt-icon-client-bundle/scripts/verify-nuxt-icon-client-bundle.mjs
```

## Testing

- `npm run typecheck`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- Real macOS desktop UI test with imported video analysis

## Further Reading

- `../docs/macOS桌面端发布指南.md`
- `../docs/桌面端完整功能验证清单.md`
