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
      // 生产环境: NUXT_PUBLIC_API_BASE=https://your-production-domain.com
      apiBase: process.env.NUXT_PUBLIC_API_BASE || 'http://localhost:8080'
    }
  },

  build: {
    transpile: ['echarts', 'vue-echarts']
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
