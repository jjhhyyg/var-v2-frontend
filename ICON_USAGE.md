# Nuxt Icon 离线配置使用指南

## ✅ 已配置的离线图标集合

项目已配置为**完全离线模式**，已安装以下图标集合：

- **lucide** - 现代、清晰的图标集（1000+ 图标）
- **simple-icons** - 品牌图标集（2800+ 图标）
- **carbon** - IBM Carbon 设计系统图标
- **heroicons** - Tailwind 团队设计的图标
- **mdi** - Material Design Icons

## 📝 使用方法

### 基础用法

```vue
<template>
  <!-- Lucide 图标 -->
  <Icon name="lucide:home" />
  <Icon name="lucide:settings" />
  <Icon name="lucide:user" />
  
  <!-- Simple Icons (品牌图标) -->
  <Icon name="simple-icons:github" />
  <Icon name="simple-icons:docker" />
  <Icon name="simple-icons:vue-dot-js" />
  
  <!-- Carbon 图标 -->
  <Icon name="carbon:close" />
  <Icon name="carbon:checkmark" />
  
  <!-- Heroicons -->
  <Icon name="heroicons:home" />
  <Icon name="heroicons:user-circle" />
  
  <!-- Material Design Icons -->
  <Icon name="mdi:account" />
  <Icon name="mdi:email" />
</template>
```

### 自定义大小和样式

```vue
<template>
  <!-- 使用 size 属性 -->
  <Icon name="lucide:heart" size="24" />
  <Icon name="lucide:star" size="32px" />
  
  <!-- 使用 CSS 样式 -->
  <Icon name="lucide:sun" style="color: orange; width: 20px; height: 20px;" />
  
  <!-- 使用 Tailwind 类 -->
  <Icon name="lucide:moon" class="w-6 h-6 text-blue-500" />
</template>
```

### 动态图标

```vue
<script setup>
const isDark = ref(false)
</script>

<template>
  <!-- 推荐：使用三元表达式 -->
  <Icon :name="isDark ? 'lucide:moon' : 'lucide:sun'" />
  
  <!-- 避免：动态构建图标名（无法被扫描检测） -->
  <!-- <Icon :name="`lucide:${icon}`" /> -->
</template>
```

## 🔍 查找可用图标

访问以下网站浏览已安装的图标集合：

- **Lucide**: <https://lucide.dev/icons/>
- **Simple Icons**: <https://simpleicons.org/>
- **Carbon**: <https://carbondesignsystem.com/guidelines/icons/library/>
- **Heroicons**: <https://heroicons.com/>
- **MDI**: <https://pictogrammers.com/library/mdi/>

## 🎨 在 app.config.ts 中自定义默认配置

如需要修改全局默认配置，可在 `app/app.config.ts` 中添加：

```typescript
export default defineAppConfig({
  icon: {
    size: '24px',      // 默认图标大小
    class: 'icon',     // 默认 CSS 类
    mode: 'svg',       // 渲染模式 ('svg' 或 'css')
    aliases: {
      'home': 'lucide:home',
      'user': 'lucide:user'
    }
  }
})
```

使用别名：

```vue
<Icon name="home" />  <!-- 等同于 lucide:home -->
```

## 📦 添加新的图标集合

如需要添加更多图标集合：

1. 安装对应的 Iconify JSON 包：

```bash
npm install -D @iconify-json/[collection-name]
```

2. 在 `nuxt.config.ts` 中添加到 `serverBundle.collections` 数组：

```typescript
icon: {
  serverBundle: {
    collections: ['lucide', 'simple-icons', 'carbon', 'heroicons', 'mdi', 'new-collection']
  }
}
```

## ⚡ 性能优化

当前配置已启用：

- ✅ **clientBundle.scan: true** - 自动扫描组件并打包使用到的图标
- ✅ **serverBundle: local** - 从本地 node_modules 加载图标
- ✅ **fallbackToApi: false** - 禁用远程 API 回退（确保完全离线）

这意味着：

- 构建时会自动检测使用的图标
- 常用图标会被打包到客户端 bundle
- 不会产生任何网络请求
- 完全支持离线环境

## 🚨 注意事项

1. **图标名称必须是静态的字符串**才能被扫描检测到
2. 客户端 bundle 大小限制为 256KB，超过会构建失败
3. 只能使用已安装的图标集合中的图标
4. SSR 模式已禁用 (`ssr: false`)，图标将在客户端渲染

## 🔧 故障排查

### 图标不显示？

1. 检查图标名称格式：`collection:icon-name`
2. 确认该图标集合已安装在 `package.json` 中
3. 确认集合名称已添加到 `nuxt.config.ts` 的 `serverBundle.collections` 中
4. 清除缓存重新构建：

```bash
rm -rf .nuxt node_modules/.cache
npm install
npm run dev
```

### 构建时提示 bundle 过大？

增加 `sizeLimitKb` 或减少使用的图标数量：

```typescript
clientBundle: {
  scan: true,
  sizeLimitKb: 512  // 增加限制
}
```
