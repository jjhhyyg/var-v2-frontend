import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { getCurrentWindow, type CloseRequestedEvent } from '@tauri-apps/api/window'
import { open, save } from '@tauri-apps/plugin-dialog'

export interface DesktopDragDropEvent {
  type: 'enter' | 'over' | 'drop' | 'leave'
  paths?: string[]
}

export const isTauriApp = () => {
  if (typeof window === 'undefined') {
    return false
  }

  return '__TAURI_INTERNALS__' in window
}

export const useDesktopBridge = () => {
  const ensureTauri = () => {
    if (!isTauriApp()) {
      throw new Error('当前运行环境不是 Tauri 桌面端')
    }
  }

  const invokeCommand = async <T>(command: string, payload?: Record<string, unknown>): Promise<T> => {
    ensureTauri()
    return invoke<T>(command, payload)
  }

  const listenEvent = async <T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> => {
    ensureTauri()
    return listen<T>(event, (evt) => {
      handler(evt.payload)
    })
  }

  const pickVideoFile = async (): Promise<string | null> => {
    ensureTauri()
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [
        {
          name: '视频文件',
          extensions: ['mp4', 'mov', 'avi', 'mkv']
        }
      ]
    })

    if (!selected || Array.isArray(selected)) {
      return null
    }

    return selected
  }

  const pickVideoFiles = async (): Promise<string[]> => {
    ensureTauri()
    const selected = await open({
      multiple: true,
      directory: false,
      filters: [
        {
          name: '视频文件',
          extensions: ['mp4', 'mov', 'avi', 'mkv']
        }
      ]
    })

    if (!selected) {
      return []
    }

    return Array.isArray(selected) ? selected : [selected]
  }

  const pickDirectory = async (defaultPath?: string): Promise<string | null> => {
    ensureTauri()
    const selected = await open({
      multiple: false,
      directory: true,
      defaultPath
    })

    if (!selected || Array.isArray(selected)) {
      return null
    }

    return selected
  }

  const pickRuntimeZip = async (): Promise<string | null> => {
    ensureTauri()
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [
        {
          name: '算法包',
          extensions: ['zip']
        }
      ]
    })

    if (!selected || Array.isArray(selected)) {
      return null
    }

    return selected
  }

  const pickSavePath = async (defaultPath?: string, filters?: Array<{ name: string, extensions: string[] }>): Promise<string | null> => {
    ensureTauri()
    const selected = await save({
      defaultPath,
      filters
    })

    return selected ?? null
  }

  const listenDragDrop = async (handler: (event: DesktopDragDropEvent) => void): Promise<UnlistenFn> => {
    ensureTauri()
    return getCurrentWebview().onDragDropEvent((event) => {
      handler({
        type: event.payload.type,
        paths: 'paths' in event.payload ? event.payload.paths : undefined
      })
    })
  }

  const listenWindowCloseRequested = async (
    handler: (event: CloseRequestedEvent) => void | Promise<void>
  ): Promise<UnlistenFn> => {
    ensureTauri()
    return getCurrentWindow().onCloseRequested(handler)
  }

  const closeCurrentWindow = async (): Promise<void> => {
    ensureTauri()
    await getCurrentWindow().close()
  }

  const requestAppExit = async (force = false): Promise<void> => {
    await invokeCommand<unknown>('request_app_exit', { force })
  }

  return {
    isTauriApp: isTauriApp(),
    invokeCommand,
    listenEvent,
    pickVideoFile,
    pickVideoFiles,
    pickDirectory,
    pickRuntimeZip,
    pickSavePath,
    listenDragDrop,
    listenWindowCloseRequested,
    closeCurrentWindow,
    requestAppExit
  }
}
