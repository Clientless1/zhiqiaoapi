<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import claudeMask from '../../assets/ai-particles/claude.svg'
import openaiMask from '../../assets/ai-particles/openai.svg'
import geminiMask from '../../assets/ai-particles/gemini.svg'
import deepseekMask from '../../assets/ai-particles/deepseek.svg'
import kimiMask from '../../assets/ai-particles/kimi.svg'
import qwenMask from '../../assets/ai-particles/qwen.svg'

type Point = { x: number; y: number }
type Shape = { name: string; points: Point[] }
const canvas = ref<HTMLCanvasElement>()
const activeName = ref('Claude')
const phase = ref('forming')
let dispose = () => {}
const particleCount = 9000
const holdDuration = 3.4
const morphDuration = 2.1
const cycleDuration = holdDuration + morphDuration
const logoSources = [
  { name: 'Claude', url: claudeMask },
  { name: 'ChatGPT', url: openaiMask },
  { name: 'Gemini', url: geminiMask },
  { name: 'DeepSeek', url: deepseekMask },
  { name: 'Kimi', url: kimiMask },
  { name: 'Qwen', url: qwenMask }
]

onMounted(async () => {
  const element = canvas.value
  const context = element?.getContext('2d', { alpha: true })
  if (!element || !context) return
  let disposed = false
  dispose = () => {
    disposed = true
  }
  let seed = 731
  const random = () => {
    seed = (seed * 16807) % 2147483647
    return (seed - 1) / 2147483646
  }
  const smooth = (value: number) => {
    const t = Math.max(0, Math.min(1, value))
    return t * t * (3 - 2 * t)
  }
  // Spatial ordering keeps nearby particles together as one silhouette becomes another.
  function spatialKey(point: Point) {
    const x = Math.round((point.x + 1) * 511)
    const y = Math.round((point.y + 1) * 511)
    let key = 0
    for (let bit = 0; bit < 10; bit++) {
      key |= ((x >> bit) & 1) << (bit * 2)
      key |= ((y >> bit) & 1) << (bit * 2 + 1)
    }
    return key
  }
  async function sampleShape(source: { name: string; url: string }): Promise<Shape> {
    const image = new Image()
    image.src = source.url
    await image.decode()
    const mask = document.createElement('canvas')
    mask.width = mask.height = 384
    const ink = mask.getContext('2d', { willReadFrequently: true })!
    ink.drawImage(image, 12, 12, 360, 360)
    const pixels = ink.getImageData(0, 0, 384, 384).data
    const candidates: Point[] = []
    let minX = 384,
      maxX = 0,
      minY = 384,
      maxY = 0
    for (let y = 0; y < 384; y += 2) {
      for (let x = 0; x < 384; x += 2) {
        if (pixels[(y * 384 + x) * 4 + 3]! < 150) continue
        candidates.push({ x, y })
        minX = Math.min(minX, x)
        maxX = Math.max(maxX, x)
        minY = Math.min(minY, y)
        maxY = Math.max(maxY, y)
      }
    }
    if (!candidates.length) throw new Error('Empty particle mask')
    const centerX = (minX + maxX) / 2
    const centerY = (minY + maxY) / 2
    const radius = Math.max(maxX - minX, maxY - minY) / 2
    const points = Array.from({ length: particleCount }, () => {
      const point = candidates[Math.floor(random() * candidates.length)]!
      const position = {
        x: (point.x + random() * 2 - centerX) / radius,
        y: (point.y + random() * 2 - centerY) / radius
      }
      return { ...position, key: spatialKey(position) }
    }).sort((a, b) => a.key - b.key)
    return { name: source.name, points }
  }
  const loaded = await Promise.allSettled(logoSources.map(sampleShape))
  if (disposed) return
  const shapes = loaded.flatMap((result) => (result.status === 'fulfilled' ? [result.value] : []))
  if (!shapes.length) return
  const particles = Array.from({ length: particleCount }, () => {
    const direction = random() * Math.PI * 2
    return {
      dx: Math.cos(direction) * (0.15 + random() * 0.4),
      dy: Math.sin(direction) * (0.15 + random() * 0.4),
      delay: random() * 0.14,
      wave: random() * Math.PI * 2,
      size: 0.65 + random() * 0.65,
      alpha: 0.38 + random() * 0.43
    }
  })
  const motion = window.matchMedia('(prefers-reduced-motion: reduce)')
  let width = 0,
    height = 0,
    frame = 0,
    time = 0,
    previous = 0
  let pointerX = 0,
    pointerY = 0,
    offsetX = 0,
    offsetY = 0
  let visible = !document.hidden

  function draw(timestamp: number) {
    frame = 0
    if (!width || !height || !visible) return
    // Thirty frames per second is enough for this slow motion; inputs stay independent.
    if (!motion.matches && previous && timestamp - previous < 30) {
      frame = requestAnimationFrame(draw)
      return
    }
    if (previous && !motion.matches) time += Math.min((timestamp - previous) / 1000, 0.08)
    previous = timestamp
    const displayTime = motion.matches ? 0 : time
    const index = Math.floor(displayTime / cycleDuration) % shapes.length
    const current = shapes[index]!
    const next = shapes[(index + 1) % shapes.length]!
    const localTime = displayTime % cycleDuration
    const progress = Math.max(0, (localTime - holdDuration) / morphDuration)
    const entering = motion.matches ? 1 : smooth(displayTime / 1.1)
    activeName.value = progress > 0.6 ? next.name : current.name
    phase.value = entering < 1 ? 'forming' : progress > 0 ? 'morphing' : 'holding'
    offsetX += (pointerX - offsetX) * 0.06
    offsetY += (pointerY - offsetY) * 0.06
    context!.clearRect(0, 0, width, height)
    context!.fillStyle = '#6482af'
    const scale = Math.min(width * 0.225, height * 0.35)
    const breathe = 1 + Math.sin(displayTime * 0.7) * 0.009
    const skew = Math.sin(displayTime * 0.33) * 0.015
    for (let i = 0; i < particleCount; i++) {
      const particle = particles[i]!
      const a = current.points[i]!
      const b = next.points[i]!
      const blend = smooth((progress - particle.delay) / (1 - particle.delay))
      const scatter = Math.sin(Math.PI * blend)
      let x = a.x + (b.x - a.x) * blend + particle.dx * scatter
      let y = a.y + (b.y - a.y) * blend + particle.dy * scatter
      x += particle.dx * (1 - entering) * 1.2
      y += particle.dy * (1 - entering) * 1.2
      const drift = motion.matches ? 0 : Math.sin(displayTime * 0.85 + particle.wave) * 0.003
      const px = width / 2 + (x + y * skew + drift) * scale * breathe + offsetX * 5
      const py = height / 2 - 5 + (y + drift) * scale * breathe + offsetY * 4
      context!.globalAlpha = particle.alpha * (0.6 + entering * 0.4) * (1 - scatter * 0.16)
      context!.fillRect(px, py, particle.size, particle.size)
    }
    context!.globalAlpha = 1
    if (!motion.matches) frame = requestAnimationFrame(draw)
  }
  function refresh() {
    cancelAnimationFrame(frame)
    previous = 0
    if (visible && width && height) frame = requestAnimationFrame(draw)
  }
  function resize() {
    const bounds = element!.getBoundingClientRect()
    width = bounds.width
    height = bounds.height
    const ratio = Math.min(window.devicePixelRatio || 1, 2)
    element!.width = Math.round(width * ratio)
    element!.height = Math.round(height * ratio)
    context!.setTransform(ratio, 0, 0, ratio, 0, 0)
    refresh()
  }
  function pointer(event: PointerEvent) {
    if (motion.matches || event.pointerType === 'touch') return
    pointerX = (event.clientX / window.innerWidth - 0.5) * 2
    pointerY = (event.clientY / window.innerHeight - 0.5) * 2
  }
  function visibility() {
    visible = !document.hidden
    refresh()
  }
  const observer = new ResizeObserver(resize)
  observer.observe(element)
  document.addEventListener('visibilitychange', visibility)
  window.addEventListener('pointermove', pointer, { passive: true })
  motion.addEventListener('change', refresh)
  dispose = () => {
    disposed = true
    cancelAnimationFrame(frame)
    observer.disconnect()
    document.removeEventListener('visibilitychange', visibility)
    window.removeEventListener('pointermove', pointer)
    motion.removeEventListener('change', refresh)
  }
})
onBeforeUnmount(() => dispose())
</script>

<template>
  <div
    class="ai-particle-display"
    :data-particle-shape="activeName"
    :data-particle-phase="phase"
    aria-hidden="true"
  >
    <canvas ref="canvas" class="bridge-particles" />
    <span class="ai-particle-name">{{ activeName }}</span>
  </div>
</template>

<style scoped>
.ai-particle-display {
  position: relative;
  width: 100%;
  height: 100%;
}
.bridge-particles {
  display: block;
  width: 100%;
  height: 100%;
}
.ai-particle-name {
  position: absolute;
  inset: auto 0 18px;
  color: #8995a6;
  text-align: center;
  font-size: 11px;
  font-weight: 400;
  letter-spacing: 1.5px;
}
</style>
