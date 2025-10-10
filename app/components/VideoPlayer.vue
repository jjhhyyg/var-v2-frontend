<template>
  <UCard>
    <!-- 视频播放器 -->
    <div class="video-wrapper">
      <!-- 视频加载失败占位符 -->
      <div
        v-if="videoError"
        class="video-error-placeholder"
      >
        <div class="error-content">
          <UIcon
            name="i-lucide-video-off"
            class="error-icon"
          />
          <h3 class="error-title">
            视频加载失败
          </h3>
          <p class="error-message">
            {{ videoErrorMessage }}
          </p>
          <div class="error-actions">
            <UButton
              icon="i-lucide-refresh-cw"
              @click="retryLoadVideo"
            >
              重新加载
            </UButton>
          </div>
        </div>
      </div>

      <!-- 视频加载中占位符 -->
      <div
        v-else-if="videoLoading"
        class="video-loading-placeholder"
      >
        <div class="loading-content">
          <UIcon
            name="i-lucide-loader-2"
            class="loading-icon animate-spin"
          />
          <p class="loading-text">
            正在加载视频...
          </p>
        </div>
      </div>

      <!-- 视频元素 -->
      <video
        v-show="!videoError && !videoLoading"
        ref="videoPlayer"
        class="video-element"
        controls
        @timeupdate="onTimeUpdate"
        @loadedmetadata="onLoadedMetadata"
        @play="isPlaying = true"
        @pause="isPlaying = false"
        @error="onVideoError"
        @loadstart="onVideoLoadStart"
        @canplay="onVideoCanPlay"
      >
        <source
          :src="currentVideoUrl"
          type="video/mp4"
        >
        您的浏览器不支持视频播放
      </video>
    </div>

    <!-- 控制栏 -->
    <div class="controls-bar">
      <!-- 视频切换 -->
      <UFieldGroup>
        <UButton
          :variant="videoType === 'original' ? 'solid' : 'outline'"
          color="primary"
          @click="switchVideo('original')"
        >
          原始视频
        </UButton>
        <UButton
          v-if="hasPreprocessedVideo"
          :variant="videoType === 'preprocessed' ? 'solid' : 'outline'"
          color="primary"
          @click="switchVideo('preprocessed')"
        >
          预处理视频
        </UButton>
        <UButton
          v-if="hasResultVideo"
          :variant="videoType === 'result' ? 'solid' : 'outline'"
          color="primary"
          @click="switchVideo('result')"
        >
          结果视频
        </UButton>
      </UFieldGroup>

      <span
        v-if="!hasResultVideo"
        class="no-result-hint text-muted text-sm"
      >
        结果视频生成中...
      </span>

      <!-- 当前时间显示 -->
      <div class="time-display font-mono text-sm text-muted">
        {{ formatTime(currentTime) }} / {{ formatTime(duration) }}
      </div>
    </div>

    <!-- 时间轴和事件标记 -->
    <UCard class="mt-6">
      <template #header>
        <h3 class="font-semibold">
          事件时间轴
        </h3>
      </template>

      <div
        ref="timeline"
        class="timeline"
        @click="onTimelineClick"
      >
        <!-- 进度条 -->
        <div
          class="timeline-progress"
          :style="{ width: progressPercentage + '%' }"
        />

        <!-- 事件标记 -->
        <div
          v-for="event in events"
          :key="event.eventId"
          class="event-marker"
          :style="{
            left: getEventPosition(frameToTimestamp(event.startFrame)) + '%'
          }"
          :title="getEventTooltip(event)"
          @click.stop="seekToEvent(event)"
        >
          <div
            class="event-dot"
            :class="getEventClass(event.eventType)"
          />
        </div>

        <!-- 物体出现时间段 -->
        <div
          v-for="obj in trackingObjects"
          :key="obj.trackingId"
          class="object-range"
          :style="{
            left: getObjectRangeStart(obj) + '%',
            width: getObjectRangeWidth(obj) + '%'
          }"
          :class="getObjectClass(obj.category)"
          :title="getObjectTooltip(obj)"
        />
      </div>

      <template #footer>
        <!-- 颜色图例 -->
        <div class="timeline-legend">
          <div class="legend-item">
            <div class="legend-color event-pool" />
            <span class="legend-text">熔池未到边</span>
          </div>
          <div class="legend-item">
            <div class="legend-color event-adhesion" />
            <span class="legend-text">电极粘连物</span>
          </div>
          <div class="legend-item">
            <div class="legend-color event-crown" />
            <span class="legend-text">锭冠</span>
          </div>
          <div class="legend-item">
            <div class="legend-color event-glow" />
            <span class="legend-text">辉光</span>
          </div>
          <div class="legend-item">
            <div class="legend-color event-side-arc" />
            <span class="legend-text">边弧（侧弧）</span>
          </div>
          <div class="legend-item">
            <div class="legend-color event-creeping-arc" />
            <span class="legend-text">爬弧</span>
          </div>
        </div>
      </template>
    </UCard>

    <!-- 事件列表 -->
    <UCard class="mt-6">
      <template #header>
        <div class="flex justify-between items-center">
          <h3 class="font-semibold">
            异常事件列表
          </h3>
          <span class="text-sm text-muted">共 {{ events.length }} 个事件</span>
        </div>
      </template>

      <div class="events-body">
        <div
          v-for="event in sortedEvents"
          :key="event.eventId"
          class="event-item"
          @click="seekToEvent(event)"
        >
          <div
            class="event-icon"
            :class="getEventClass(event.eventType)"
          />
          <div class="event-info">
            <div class="event-type">
              {{ getEventTypeLabel(event.eventType) }}
            </div>
            <div class="event-time">
              {{ formatTime(frameToTimestamp(event.startFrame)) }}
            </div>
          </div>
          <UButton
            size="xs"
            variant="outline"
            color="primary"
          >
            跳转
          </UButton>
        </div>
        <div
          v-if="events.length === 0"
          class="no-events text-center py-8 text-muted"
        >
          暂无异常事件
        </div>
      </div>
    </UCard>
  </UCard>
</template>

<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'

interface Event {
  eventId: string
  eventType: string
  startFrame: number
  endFrame: number
  objectId?: string
  metadata?: Record<string, unknown>
}

interface TrackingObject {
  trackingId: string
  objectId: string
  category: string
  firstFrame: number
  lastFrame: number
  trajectory?: Array<Record<string, unknown>>
}

interface Props {
  taskId: string
  videoDuration: number
  resultVideoPath?: string
  preprocessedVideoPath?: string
  events?: Event[]
  trackingObjects?: TrackingObject[]
}

const props = withDefaults(defineProps<Props>(), {
  events: () => [],
  trackingObjects: () => []
})

const videoPlayer = ref<HTMLVideoElement>()
const timeline = ref<HTMLDivElement>()
const videoType = ref<'original' | 'preprocessed' | 'result'>('original')
const currentTime = ref(0)
const duration = ref(0)
const isPlaying = ref(false)
const videoError = ref(false)
const videoErrorMessage = ref('')
const videoLoading = ref(true)

// 获取后端API基础URL
const { baseURL } = useApi()

const hasResultVideo = computed(() => !!props.resultVideoPath)
const hasPreprocessedVideo = computed(() => !!props.preprocessedVideoPath)

const currentVideoUrl = computed(() => {
  return `${baseURL}/api/videos/${props.taskId}/${videoType.value}`
})

const progressPercentage = computed(() => {
  if (duration.value === 0) return 0
  return (currentTime.value / duration.value) * 100
})

// 视频帧率（假设25fps，实际应从视频元数据获取）
const fps = ref(25)

// 帧号转时间戳
const frameToTimestamp = (frame: number): number => {
  return frame / fps.value
}

const sortedEvents = computed(() => {
  return [...props.events].sort((a, b) => a.startFrame - b.startFrame)
})

// 切换视频
const switchVideo = (type: 'original' | 'preprocessed' | 'result') => {
  if (type === 'result' && !hasResultVideo.value) return
  if (type === 'preprocessed' && !hasPreprocessedVideo.value) return

  const currentPlayTime = currentTime.value
  const wasPlaying = isPlaying.value

  videoType.value = type

  // 需要在下一个tick中重新加载视频,因为src需要先更新
  nextTick(() => {
    if (videoPlayer.value) {
      // 重新加载视频源
      videoPlayer.value.load()

      // 监听加载完成事件,恢复播放位置
      videoPlayer.value.addEventListener(
        'loadedmetadata',
        () => {
          if (videoPlayer.value) {
            videoPlayer.value.currentTime = currentPlayTime
            if (wasPlaying) {
              videoPlayer.value.play()
            }
          }
        },
        { once: true }
      )
    }
  })
}

// 时间更新
const onTimeUpdate = () => {
  if (videoPlayer.value) {
    currentTime.value = videoPlayer.value.currentTime
  }
}

// 元数据加载
const onLoadedMetadata = () => {
  if (videoPlayer.value) {
    duration.value = videoPlayer.value.duration || props.videoDuration
  }
}

// 视频开始加载
const onVideoLoadStart = () => {
  videoLoading.value = true
  videoError.value = false
}

// 视频可以播放
const onVideoCanPlay = () => {
  videoLoading.value = false
  videoError.value = false
}

// 视频加载错误
const onVideoError = () => {
  videoLoading.value = false
  videoError.value = true

  const video = videoPlayer.value
  if (video?.error) {
    switch (video.error.code) {
      case MediaError.MEDIA_ERR_ABORTED:
        videoErrorMessage.value = '视频加载被中止，请重试'
        break
      case MediaError.MEDIA_ERR_NETWORK:
        videoErrorMessage.value = '网络错误，无法加载视频'
        break
      case MediaError.MEDIA_ERR_DECODE:
        videoErrorMessage.value = '视频解码失败，文件可能已损坏'
        break
      case MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED:
        videoErrorMessage.value = '不支持的视频格式或视频不存在'
        break
      default:
        videoErrorMessage.value = '视频加载失败，请稍后重试'
    }
  } else {
    videoErrorMessage.value = '视频加载失败，请稍后重试'
  }
}

// 重新加载视频
const retryLoadVideo = () => {
  videoError.value = false
  videoLoading.value = true
  if (videoPlayer.value) {
    videoPlayer.value.load()
  }
}

// 格式化时间
const formatTime = (seconds: number): string => {
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = Math.floor(seconds % 60)

  if (h > 0) {
    return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`
  }
  return `${m}:${s.toString().padStart(2, '0')}`
}

// 获取事件位置（百分比）
const getEventPosition = (timestamp: number): number => {
  if (duration.value === 0) return 0
  return (timestamp / duration.value) * 100
}

// 获取物体出现范围起始位置
const getObjectRangeStart = (obj: TrackingObject): number => {
  if (duration.value === 0) return 0
  const firstTimestamp = frameToTimestamp(obj.firstFrame)
  return (firstTimestamp / duration.value) * 100
}

// 获取物体出现范围宽度
const getObjectRangeWidth = (obj: TrackingObject): number => {
  if (duration.value === 0) return 0
  const firstTimestamp = frameToTimestamp(obj.firstFrame)
  const lastTimestamp = frameToTimestamp(obj.lastFrame)
  const rangeDuration = lastTimestamp - firstTimestamp
  const percentage = (rangeDuration / duration.value) * 100

  // 如果firstFrame和lastFrame相同(单帧物体),设置最小宽度0.5%以确保可见
  return percentage > 0 ? percentage : 0.5
}

// 获取事件样式类
const getEventClass = (eventType: string): string => {
  const typeMap: Record<string, string> = {
    POOL_NOT_REACHED: 'event-pool',
    ADHESION_FORMED: 'event-adhesion',
    ADHESION_DROPPED: 'event-adhesion',
    CROWN_DROPPED: 'event-crown',
    GLOW: 'event-glow',
    SIDE_ARC: 'event-side-arc',
    CREEPING_ARC: 'event-creeping-arc'
  }
  return typeMap[eventType] || 'event-default'
}

// 获取物体样式类
const getObjectClass = (category: string): string => {
  const categoryMap: Record<string, string> = {
    POOL_NOT_REACHED: 'object-pool',
    ADHESION: 'object-adhesion',
    CROWN: 'object-crown',
    GLOW: 'object-glow',
    SIDE_ARC: 'object-side-arc',
    CREEPING_ARC: 'object-creeping-arc'
  }
  return categoryMap[category] || 'object-default'
}

// 获取事件类型标签
const getEventTypeLabel = (eventType: string): string => {
  const labelMap: Record<string, string> = {
    POOL_NOT_REACHED: '熔池未到边',
    ADHESION_FORMED: '电极形成粘连物',
    ADHESION_DROPPED: '电极粘连物脱落',
    CROWN_DROPPED: '锭冠脱落',
    GLOW: '辉光',
    SIDE_ARC: '边弧（侧弧）',
    CREEPING_ARC: '爬弧'
  }
  return labelMap[eventType] || eventType
}

// 获取事件提示信息
const getEventTooltip = (event: Event): string => {
  const timestamp = frameToTimestamp(event.startFrame)
  return `${getEventTypeLabel(event.eventType)} - ${formatTime(timestamp)}`
}

// 获取类别中文名称
const getCategoryLabel = (category: string): string => {
  const categoryMap: Record<string, string> = {
    POOL_NOT_REACHED: '熔池未到边',
    ADHESION: '电极粘连物',
    CROWN: '锭冠',
    GLOW: '辉光',
    SIDE_ARC: '边弧（侧弧）',
    CREEPING_ARC: '爬弧'
  }
  return categoryMap[category] || category
}

// 获取物体提示信息
const getObjectTooltip = (obj: TrackingObject): string => {
  const firstTimestamp = frameToTimestamp(obj.firstFrame)
  const lastTimestamp = frameToTimestamp(obj.lastFrame)
  const categoryLabel = getCategoryLabel(obj.category)
  return `${categoryLabel} (ID: ${obj.objectId}) - ${formatTime(
    firstTimestamp
  )} ~ ${formatTime(lastTimestamp)}`
}

// 跳转到事件时间点
const seekToEvent = (event: Event) => {
  if (videoPlayer.value) {
    videoPlayer.value.currentTime = frameToTimestamp(event.startFrame)
    if (!isPlaying.value) {
      videoPlayer.value.play()
    }
  }
}

// 点击时间轴
const onTimelineClick = (e: MouseEvent) => {
  if (!timeline.value || !videoPlayer.value) return

  const rect = timeline.value.getBoundingClientRect()
  const x = e.clientX - rect.left
  const percentage = x / rect.width
  const newTime = percentage * duration.value

  videoPlayer.value.currentTime = Math.max(0, Math.min(newTime, duration.value))
}
</script>

<style scoped>
.video-wrapper {
  position: relative;
  width: 100%;
  background: #000;
  border-radius: 8px;
  overflow: hidden;
  min-height: 400px;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 1rem;
}

.video-element {
  width: 100%;
  height: auto;
  display: block;
}

/* 视频错误占位符 */
.video-error-placeholder {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgb(17, 24, 39);
  color: rgb(243, 244, 246);
}

:global(html.dark) .video-error-placeholder {
  background: rgb(3, 7, 18);
}

.error-content {
  text-align: center;
  padding: 2rem;
  max-width: 400px;
}

.error-icon {
  width: 4rem;
  height: 4rem;
  margin: 0 auto 1.5rem;
  color: rgb(239, 68, 68);
}

.error-title {
  font-size: 1.25rem;
  font-weight: 600;
  margin-bottom: 0.5rem;
}

.error-message {
  font-size: 0.95rem;
  color: rgb(156, 163, 175);
  margin-bottom: 1.5rem;
  line-height: 1.5;
}

.error-actions {
  display: flex;
  justify-content: center;
  gap: 1rem;
}

/* 视频加载占位符 */
.video-loading-placeholder {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgb(17, 24, 39);
}

:global(html.dark) .video-loading-placeholder {
  background: rgb(3, 7, 18);
}

.loading-content {
  text-align: center;
  color: rgb(209, 213, 219);
}

.loading-icon {
  width: 3rem;
  height: 3rem;
  margin: 0 auto 1rem;
  color: rgb(59, 130, 246);
}

.loading-text {
  font-size: 0.95rem;
  color: rgb(156, 163, 175);
}

.controls-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1rem 0;
  gap: 1rem;
  flex-wrap: wrap;
}

.timeline {
  position: relative;
  height: 60px;
  background: rgb(229, 231, 235);
  border-radius: 4px;
  cursor: pointer;
  overflow: hidden;
}

:global(html.dark) .timeline {
  background: rgb(55, 65, 81);
}

.timeline-progress {
  position: absolute;
  top: 0;
  left: 0;
  height: 100%;
  background: linear-gradient(90deg, #1890ff 0%, #40a9ff 100%);
  opacity: 0.3;
  transition: width 0.1s;
}

.event-marker {
  position: absolute;
  top: 50%;
  transform: translate(-50%, -50%);
  z-index: 10;
  cursor: pointer;
}

.event-dot {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  border: 2px solid rgb(249, 250, 251);
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
}

:global(html.dark) .event-dot {
  border-color: rgb(31, 41, 55);
}

/* 事件标记颜色 */
.event-pool { background: rgb(0, 100, 0); }
.event-adhesion { background: rgb(255, 0, 0); }
.event-crown { background: rgb(0, 0, 255); }
.event-glow { background: rgb(0, 255, 255); }
.event-side-arc { background: rgb(128, 0, 128); }
.event-creeping-arc { background: rgb(255, 165, 0); }
.event-default { background: #666; }

/* 物体出现时间段 */
.object-range {
  position: absolute;
  top: 0;
  height: 100%;
  opacity: 0.3;
  pointer-events: none;
  transition: opacity 0.2s;
}

.object-range:hover { opacity: 0.5; }

.object-pool { background: rgb(0, 100, 0); }
.object-adhesion { background: rgb(255, 0, 0); }
.object-crown { background: rgb(0, 0, 255); }
.object-glow { background: rgb(0, 255, 255); }
.object-side-arc { background: rgb(128, 0, 128); }
.object-creeping-arc { background: rgb(255, 165, 0); }
.object-default { background: #666; }

.timeline-legend {
  display: flex;
  flex-wrap: wrap;
  gap: 1rem;
}

.legend-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.legend-color {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  border: 2px solid rgb(249, 250, 251);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  flex-shrink: 0;
}

:global(html.dark) .legend-color {
  border-color: rgb(31, 41, 55);
}

.legend-text {
  font-size: 0.85rem;
  white-space: nowrap;
}

.events-body {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  max-height: 300px;
  overflow-y: auto;
}

.event-item {
  display: flex;
  align-items: center;
  gap: 1rem;
  padding: 0.75rem;
  background: rgb(243, 244, 246);
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
}

:global(html.dark) .event-item {
  background: rgb(31, 41, 55);
}

.event-item:hover {
  background: rgb(229, 231, 235);
  transform: translateX(4px);
}

:global(html.dark) .event-item:hover {
  background: rgb(55, 65, 81);
}

.event-icon {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex-shrink: 0;
}

.event-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.event-type {
  font-weight: 500;
  font-size: 0.9rem;
}

.event-time {
  font-family: monospace;
  font-size: 0.85rem;
  color: rgb(107, 114, 128);
}

:global(html.dark) .event-time {
  color: rgb(156, 163, 175);
}
</style>
