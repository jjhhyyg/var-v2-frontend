/**
 * Tauri 桌面事件订阅封装
 */

import type { Task, TaskStatus } from './useTaskApi'
import type { UnlistenFn } from '@tauri-apps/api/event'

interface TaskListMessage {
  taskId: string
  status: string
  progress?: number
  queuePosition?: number
}

type Unsubscribe = () => void

const activeUnsubscribers = new Set<Unsubscribe>()

export const useTauriEvents = () => {
  const { listenEvent } = useDesktopBridge()
  const isConnected = ref(false)

  const connect = async (): Promise<void> => {
    isConnected.value = true
  }

  const disconnect = async () => {
    activeUnsubscribers.forEach(unsubscribe => unsubscribe())
    activeUnsubscribers.clear()
    isConnected.value = false
  }

  const subscribeToTask = (
    taskId: string,
    callback: (status: TaskStatus) => void
  ): Unsubscribe => {
    let innerUnlisten: UnlistenFn | null = null
    let disposed = false

    listenEvent<TaskStatus>('task-status', (status) => {
      if (status.taskId === taskId) {
        callback(status)
      }
    }).then((unlisten) => {
      if (disposed) {
        unlisten()
        return
      }
      innerUnlisten = unlisten
    })

    const unsubscribe = () => {
      disposed = true
      innerUnlisten?.()
      activeUnsubscribers.delete(unsubscribe)
    }
    activeUnsubscribers.add(unsubscribe)
    return unsubscribe
  }

  const subscribeToTaskUpdates = (
    callback: (message: TaskListMessage) => void
  ): Unsubscribe => {
    let innerUnlisten: UnlistenFn | null = null
    let disposed = false

    listenEvent<TaskListMessage>('task-list-update', callback).then((unlisten) => {
      if (disposed) {
        unlisten()
        return
      }
      innerUnlisten = unlisten
    })

    const unsubscribe = () => {
      disposed = true
      innerUnlisten?.()
      activeUnsubscribers.delete(unsubscribe)
    }
    activeUnsubscribers.add(unsubscribe)
    return unsubscribe
  }

  const subscribeToTaskDetailUpdate = (
    taskId: string,
    callback: (task: Task) => void
  ): Unsubscribe => {
    let innerUnlisten: UnlistenFn | null = null
    let disposed = false

    listenEvent<Task>('task-detail-update', (task) => {
      if (task.taskId === taskId) {
        callback(task)
      }
    }).then((unlisten) => {
      if (disposed) {
        unlisten()
        return
      }
      innerUnlisten = unlisten
    })

    const unsubscribe = () => {
      disposed = true
      innerUnlisten?.()
      activeUnsubscribers.delete(unsubscribe)
    }
    activeUnsubscribers.add(unsubscribe)
    return unsubscribe
  }

  return {
    isConnected: readonly(isConnected),
    connect,
    disconnect,
    subscribeToTask,
    subscribeToTaskUpdates,
    subscribeToTaskDetailUpdate
  }
}
