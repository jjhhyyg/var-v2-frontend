// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  modules: ['@nuxt/eslint', '@nuxt/ui'],

  ssr: false,

  devtools: {
    enabled: false
  },

  css: ['~/assets/css/main.css'],

  runtimeConfig: {
    public: {
      // 使用环境变量配置API基础URL
      // 开发环境: NUXT_PUBLIC_API_BASE=http://localhost:8080
      // 生产环境: NUXT_PUBLIC_API_BASE="" (空字符串，使用nginx反向代理)
      apiBase: process.env.NUXT_PUBLIC_API_BASE !== undefined
        ? process.env.NUXT_PUBLIC_API_BASE
        : 'http://localhost:8080'
    }
  },

  build: {
    transpile: ['echarts', 'vue-echarts']
  },

  // 配置图标为离线模式
  // 由于已经安装了 @iconify-json/lucide 和 @iconify-json/simple-icons
  // Nuxt 会自动从 node_modules 中读取这些图标数据，无需访问 Iconify API
  icon: {
    provider: 'server',
    serverBundle: {
      collections: ['lucide', 'simple-icons']
    }
  },

  // routeRules: {
  //   '/': { ssr: false },
  //   '/tasks/**': { ssr: false }
  // },

  compatibilityDate: '2025-01-15',

  eslint: {
    config: {
      stylistic: {
        commaDangle: 'never',
        braceStyle: '1tbs'
      }
    }
  }
})
