<template>
  <div class="video-player-container">
    <!-- 视频播放器 -->
    <div class="video-wrapper">
      <video
        ref="videoPlayer"
        class="video-element"
        controls
        @timeupdate="onTimeUpdate"
        @loadedmetadata="onLoadedMetadata"
        @play="isPlaying = true"
        @pause="isPlaying = false"
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
      <div class="video-switch">
        <button
          :class="['switch-btn', { active: videoType === 'original' }]"
          @click="switchVideo('original')"
        >
          原始视频
        </button>
        <button
          v-if="hasPreprocessedVideo"
          :class="['switch-btn', { active: videoType === 'preprocessed' }]"
          @click="switchVideo('preprocessed')"
        >
          预处理视频
        </button>
        <button
          v-if="hasResultVideo"
          :class="['switch-btn', { active: videoType === 'result' }]"
          @click="switchVideo('result')"
        >
          结果视频
        </button>
        <span
          v-if="!hasResultVideo"
          class="no-result-hint"
        > 结果视频生成中... </span>
      </div>

      <!-- 当前时间显示 -->
      <div class="time-display">
        {{ formatTime(currentTime) }} / {{ formatTime(duration) }}
      </div>
    </div>

    <!-- 时间轴和事件标记 -->
    <div class="timeline-container">
      <div class="timeline-label">
        事件时间轴
      </div>
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
    </div>

    <!-- 事件列表 -->
    <div class="events-list">
      <div class="events-header">
        <h3>异常事件列表</h3>
        <span class="event-count">共 {{ events.length }} 个事件</span>
      </div>
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
          <div class="event-action">
            <button class="jump-btn">
              跳转
            </button>
          </div>
        </div>
        <div
          v-if="events.length === 0"
          class="no-events"
        >
          暂无异常事件
        </div>
      </div>
    </div>
  </div>
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
.video-player-container {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
  padding: 1.5rem;
  background: rgb(var(--color-gray-50));
  border-radius: 8px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.dark .video-player-container {
  background: rgb(var(--color-gray-900));
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
}

.video-wrapper {
  position: relative;
  width: 100%;
  background: #000;
  border-radius: 8px;
  overflow: hidden;
}

.video-element {
  width: 100%;
  height: auto;
  display: block;
}

.controls-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1rem 0;
  gap: 1rem;
}

.video-switch {
  display: flex;
  gap: 0.5rem;
  align-items: center;
}

.switch-btn {
  padding: 0.5rem 1rem;
  border: 1px solid rgb(var(--color-gray-200));
  background: rgb(var(--color-gray-50));
  color: rgb(var(--color-gray-600));
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  font-size: 0.9rem;
}

.dark .switch-btn {
  border-color: rgb(var(--color-gray-700));
  background: rgb(var(--color-gray-800));
  color: rgb(var(--color-gray-300));
}

.switch-btn:hover {
  background: rgb(var(--color-gray-100));
  border-color: #1890ff;
  color: #1890ff;
}

.dark .switch-btn:hover {
  background: rgb(var(--color-gray-700));
}

.switch-btn.active {
  background: #1890ff;
  color: #fff;
  border-color: #1890ff;
}

.no-result-hint {
  font-size: 0.85rem;
  color: rgb(var(--color-gray-400));
  font-style: italic;
}

.time-display {
  font-family: monospace;
  font-size: 0.9rem;
  color: rgb(var(--color-gray-600));
}

.dark .time-display {
  color: rgb(var(--color-gray-300));
}

.timeline-container {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.timeline-label {
  font-weight: 600;
  color: rgb(var(--color-gray-700));
  font-size: 0.9rem;
}

.dark .timeline-label {
  color: rgb(var(--color-gray-200));
}

.timeline {
  position: relative;
  height: 60px;
  background: rgb(var(--color-gray-200));
  border-radius: 4px;
  cursor: pointer;
  overflow: hidden;
}

.dark .timeline {
  background: rgb(var(--color-gray-700));
}

.timeline-legend {
  display: flex;
  flex-wrap: wrap;
  gap: 1rem;
  padding: 0.75rem;
  background: rgb(var(--color-gray-100));
  border-radius: 4px;
  border: 1px solid rgb(var(--color-gray-200));
}

.dark .timeline-legend {
  background: rgb(var(--color-gray-800));
  border-color: rgb(var(--color-gray-700));
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
  border: 2px solid rgb(var(--color-gray-50));
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  flex-shrink: 0;
}

.dark .legend-color {
  border-color: rgb(var(--color-gray-800));
}

.legend-text {
  font-size: 0.85rem;
  color: rgb(var(--color-gray-600));
  white-space: nowrap;
}

.dark .legend-text {
  color: rgb(var(--color-gray-300));
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
  border: 2px solid rgb(var(--color-gray-50));
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
}

.dark .event-dot {
  border-color: rgb(var(--color-gray-800));
}

/* 事件标记颜色（根据后端BGR定义转换为RGB） */
.event-pool {
  background: rgb(0, 100, 0); /* 深绿色 - 熔池未到边 */
}

.event-adhesion {
  background: rgb(255, 0, 0); /* 红色 - 粘连物 */
}

.event-crown {
  background: rgb(0, 0, 255); /* 蓝色 - 锭冠 */
}

.event-glow {
  background: rgb(0, 255, 255); /* 青色 - 辉光 */
}

.event-side-arc {
  background: rgb(128, 0, 128); /* 紫色 - 边弧（侧弧） */
}

.event-creeping-arc {
  background: rgb(255, 165, 0); /* 橙色 - 爬弧 */
}

.event-default {
  background: #666;
}

/* 物体出现时间段颜色（与事件保持一致） */
.object-range {
  position: absolute;
  top: 0;
  height: 100%;
  opacity: 0.3;
  pointer-events: none;
  transition: opacity 0.2s;
}

.object-range:hover {
  opacity: 0.5;
}

.object-pool {
  background: rgb(0, 100, 0); /* 深绿色 - 熔池未到边 */
}

.object-adhesion {
  background: rgb(255, 0, 0); /* 红色 - 粘连物 */
}

.object-crown {
  background: rgb(0, 0, 255); /* 蓝色 - 锭冠 */
}

.object-glow {
  background: rgb(0, 255, 255); /* 青色 - 辉光 */
}

.object-side-arc {
  background: rgb(128, 0, 128); /* 紫色 - 边弧（侧弧） */
}

.object-creeping-arc {
  background: rgb(255, 165, 0); /* 橙色 - 爬弧 */
}

.object-default {
  background: #666;
}

.events-list {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.events-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding-bottom: 0.5rem;
  border-bottom: 2px solid rgb(var(--color-gray-200));
}

.dark .events-header {
  border-bottom-color: rgb(var(--color-gray-700));
}

.events-header h3 {
  margin: 0;
  font-size: 1rem;
  font-weight: 600;
  color: rgb(var(--color-gray-700));
}

.dark .events-header h3 {
  color: rgb(var(--color-gray-200));
}

.event-count {
  font-size: 0.85rem;
  color: rgb(var(--color-gray-400));
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
  background: rgb(var(--color-gray-100));
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
}

.dark .event-item {
  background: rgb(var(--color-gray-800));
}

.event-item:hover {
  background: rgb(var(--color-gray-200));
  transform: translateX(4px);
}

.dark .event-item:hover {
  background: rgb(var(--color-gray-700));
}

/* 事件列表图标颜色（复用object-*类的颜色定义） */
.event-icon {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex-shrink: 0;
}

.event-icon.event-pool {
  background: rgb(0, 100, 0);
}

.event-icon.event-adhesion {
  background: rgb(255, 0, 0);
}

.event-icon.event-crown {
  background: rgb(0, 0, 255);
}

.event-icon.event-glow {
  background: rgb(0, 255, 255);
}

.event-icon.event-side-arc {
  background: rgb(128, 0, 128);
}

.event-icon.event-creeping-arc {
  background: rgb(255, 165, 0);
}

.event-icon.event-default {
  background: #666;
}

.event-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.event-type {
  font-weight: 500;
  color: rgb(var(--color-gray-700));
  font-size: 0.9rem;
}

.dark .event-type {
  color: rgb(var(--color-gray-200));
}

.event-time {
  font-family: monospace;
  font-size: 0.85rem;
  color: rgb(var(--color-gray-500));
}

.dark .event-time {
  color: rgb(var(--color-gray-400));
}

.jump-btn {
  padding: 0.25rem 0.75rem;
  border: 1px solid #1890ff;
  background: rgb(var(--color-gray-50));
  color: #1890ff;
  border-radius: 4px;
  cursor: pointer;
  font-size: 0.85rem;
  transition: all 0.2s;
}

.dark .jump-btn {
  background: rgb(var(--color-gray-900));
}

.jump-btn:hover {
  background: #1890ff;
  color: #fff;
}

.no-events {
  text-align: center;
  padding: 2rem;
  color: rgb(var(--color-gray-400));
  font-size: 0.9rem;
}
</style>
