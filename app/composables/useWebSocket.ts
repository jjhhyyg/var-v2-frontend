/**
 * WebSocket通讯composable
 * 用于实时接收任务状态更新
 */

import { Client } from '@stomp/stompjs'
import SockJS from 'sockjs-client'
import type { Task, TaskStatus } from './useTaskApi'

interface WebSocketMessage {
  taskId: string
  status: string
  progress?: number
}

type StompSubscription = {
  unsubscribe: () => void
}

export const useWebSocket = () => {
  const config = useRuntimeConfig()
  const stompClient = ref<Client | null>(null)
  const isConnected = ref(false)
  const subscriptions = new Map<string, StompSubscription>()

  /**
   * 连接WebSocket
   */
  const connect = (): Promise<void> => {
    return new Promise((resolve, reject) => {
      if (stompClient.value && isConnected.value) {
        resolve()
        return
      }

      const baseUrl = config.public.apiBase ?? 'http://localhost:8080'
      const wsUrl = `${baseUrl}/ws`

      const client = new Client({
        webSocketFactory: () => new SockJS(wsUrl) as WebSocket,
        connectHeaders: {},
        debug: (str: string) => {
          console.log('[WebSocket Debug]', str)
        },
        reconnectDelay: 5000,
        heartbeatIncoming: 4000,
        heartbeatOutgoing: 4000,
        onConnect: () => {
          console.log('WebSocket已连接')
          isConnected.value = true
          resolve()
        },
        onStompError: (frame: { headers: { message?: string } }) => {
          console.error('WebSocket STOMP错误:', frame)
          isConnected.value = false
          reject(new Error(frame.headers.message || 'WebSocket连接失败'))
        },
        onWebSocketError: (event: Event) => {
          console.error('WebSocket错误:', event)
          isConnected.value = false
        },
        onDisconnect: () => {
          console.log('WebSocket已断开')
          isConnected.value = false
        }
      })

      stompClient.value = client
      client.activate()
    })
  }

  /**
   * 断开WebSocket连接
   */
  const disconnect = async () => {
    if (stompClient.value) {
      await stompClient.value.deactivate()
      stompClient.value = null
      isConnected.value = false
      subscriptions.clear()
      console.log('WebSocket连接已关闭')
    }
  }

  /**
   * 订阅特定任务的状态更新
   */
  const subscribeToTask = (
    taskId: string,
    callback: (status: TaskStatus) => void
  ): (() => void) => {
    if (!stompClient.value || !isConnected.value) {
      console.warn('WebSocket未连接，无法订阅')
      return () => {}
    }

    const destination = `/topic/tasks/${taskId}/status`
    const subscription = stompClient.value.subscribe(destination, (message: { body: string }) => {
      try {
        const status = JSON.parse(message.body) as TaskStatus
        callback(status)
      } catch (error) {
        console.error('解析任务状态消息失败:', error)
      }
    })

    subscriptions.set(`task-${taskId}`, subscription)
    console.log(`已订阅任务状态: ${taskId}`)

    // 返回取消订阅函数
    return () => {
      subscription.unsubscribe()
      subscriptions.delete(`task-${taskId}`)
      console.log(`已取消订阅任务状态: ${taskId}`)
    }
  }

  /**
   * 订阅所有任务列表更新
   */
  const subscribeToTaskUpdates = (callback: (message: WebSocketMessage) => void): (() => void) => {
    if (!stompClient.value || !isConnected.value) {
      console.warn('WebSocket未连接，无法订阅')
      return () => {}
    }

    const destination = '/topic/tasks/updates'
    const subscription = stompClient.value.subscribe(destination, (message: { body: string }) => {
      try {
        const update = JSON.parse(message.body) as WebSocketMessage
        callback(update)
      } catch (error) {
        console.error('解析任务列表更新消息失败:', error)
      }
    })

    subscriptions.set('task-updates', subscription)
    console.log('已订阅任务列表更新')

    // 返回取消订阅函数
    return () => {
      subscription.unsubscribe()
      subscriptions.delete('task-updates')
      console.log('已取消订阅任务列表更新')
    }
  }

  /**
   * 订阅特定任务的详情更新（包括resultVideoPath等字段更新）
   */
  const subscribeToTaskDetailUpdate = (
    taskId: string,
    callback: (task: Task) => void
  ): (() => void) => {
    if (!stompClient.value || !isConnected.value) {
      console.warn('WebSocket未连接，无法订阅')
      return () => {}
    }

    const destination = `/topic/tasks/${taskId}/update`
    const subscription = stompClient.value.subscribe(destination, (message: { body: string }) => {
      try {
        const task = JSON.parse(message.body)
        callback(task)
      } catch (error) {
        console.error('解析任务详情更新消息失败:', error)
      }
    })

    subscriptions.set(`task-detail-${taskId}`, subscription)
    console.log(`已订阅任务详情更新: ${taskId}`)

    // 返回取消订阅函数
    return () => {
      subscription.unsubscribe()
      subscriptions.delete(`task-detail-${taskId}`)
      console.log(`已取消订阅任务详情更新: ${taskId}`)
    }
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
