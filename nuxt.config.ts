// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  modules: ['@nuxt/eslint', '@nuxt/ui', '@nuxt/icon'],

  ssr: false,

  devtools: {
    enabled: false
  },

  css: ['~/assets/css/main.css'],

  build: {
    transpile: ['echarts', 'vue-echarts']
  },

  vite: {
    optimizeDeps: {
      include: ['jspdf']
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
  },

  // 配置图标为离线模式
  // 由于已经安装了 @iconify-json/* 包
  // Nuxt Icon 会自动从 node_modules 中读取这些图标数据,无需访问 Iconify API
  icon: {
    // 使用 SVG 模式以获得更好的性能和可定制性
    mode: 'svg',

    // 配置服务端 bundle 为 local 模式(默认),从本地 @iconify-json 包加载
    serverBundle: {
      collections: ['lucide', 'simple-icons', 'carbon', 'heroicons', 'mdi']
    },

    // 配置客户端 bundle,将常用图标打包到客户端以避免网络请求
    clientBundle: {
      // 扫描所有组件中使用的图标,自动打包到客户端
      scan: true,

      // 可选:明确指定需要打包的图标(如果扫描未覆盖)
      // icons: ['lucide:home', 'lucide:settings'],

      // 设置客户端 bundle 大小限制 (KB)
      sizeLimitKb: 256
    },

    // 禁用回退到 Iconify API(确保完全离线)
    fallbackToApi: false
  }
})
