# 多阶段构建 - 前端Dockerfile

# 阶段1：构建阶段
FROM node:22-alpine AS builder

LABEL maintainer="侯阳洋"
LABEL description="VAR熔池视频分析系统 - 前端构建阶段"

WORKDIR /app/frontend

# 接收构建参数（API基础URL配置）
ARG NUXT_PUBLIC_API_BASE=""
ENV NUXT_PUBLIC_API_BASE=${NUXT_PUBLIC_API_BASE}

# 启用corepack并配置npm
RUN corepack enable && corepack prepare npm@11.6.2 --activate

# 复制package.json并安装依赖（利用Docker缓存层）
COPY package.json package-lock.json ./
RUN npm ci

# 复制源代码并构建应用（生成静态文件）
COPY . .
RUN npm run generate

# 阶段2：Nginx 部署阶段
FROM nginx:1.27-alpine

LABEL maintainer="侯阳洋"
LABEL description="VAR熔池视频分析系统 - 前端服务（Nginx）"

# 从构建阶段复制静态文件
COPY --from=builder /app/frontend/.output/public /usr/share/nginx/html

# 复制自定义 Nginx 配置
COPY nginx.conf /etc/nginx/conf.d/default.conf

# 暴露端口
EXPOSE 8848

# 健康检查
HEALTHCHECK --interval=30s --timeout=3s --start-period=15s --retries=3 \
  CMD wget --no-verbose --tries=1 --spider http://localhost:8848/ || exit 1

# 启动 Nginx（前台运行）
CMD ["nginx", "-g", "daemon off;"]
