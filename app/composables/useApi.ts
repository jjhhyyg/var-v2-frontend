/**
 * API请求封装
 */

type HttpMethod = 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH' | 'HEAD' | 'OPTIONS'

interface FetchOptions {
  method?: HttpMethod
  body?: FormData | Record<string, unknown>
  params?: Record<string, string | number>
  headers?: Record<string, string>
}

export const useApi = () => {
  const config = useRuntimeConfig()
  const baseURL = config.public.apiBase

  /**
   * 通用请求方法
   */
  const request = async <T>(url: string, options?: FetchOptions): Promise<T> => {
    try {
      const response = await $fetch<{ code: number, message: string, data: T }>(url, {
        baseURL,
        ...options
      })

      if (response.code !== 200) {
        throw new Error(response.message || '请求失败')
      }

      return response.data
    } catch (error: unknown) {
      console.error('API请求错误:', error)
      throw error
    }
  }

  return {
    request,
    baseURL
  }
}
