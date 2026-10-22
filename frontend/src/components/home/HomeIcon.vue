<script setup lang="ts">
defineProps<{ name: string }>()

const paths: Record<string, string[]> = {
  sparkle: ['M12 3l2.3 6.7L21 12l-6.7 2.3L12 21l-2.3-6.7L3 12l6.7-2.3L12 3Z'],
  rocket: [
    'M14 5c2.1-1.6 4.4-2.1 7-2-0.1 2.6-0.6 4.9-2.2 7l-6 6-4.8-4.8L14 5Z',
    'M8 8H5l-3 5h6M16 16v3l-5 3v-6M5 16c-2 0-2 4-2 5 1 0 5 0 5-2',
    'M15 8h.01'
  ],
  document: [
    'M14 3H6a1 1 0 0 0-1 1v16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8l-5-5Z',
    'M14 3v5h5M9 12h6M9 16h6M9 8h1'
  ],
  chat: ['M5 5h15v11H9l-4 4V5Z', 'M5 8H2v11h3M9 9h7M9 12h5'],
  image: [
    'M4 3h16a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1Z',
    'm3 17 5-5 4 4 4-6 5 7M8 7h.01'
  ],
  play: ['M12 3a9 9 0 1 1 0 18 9 9 0 0 1 0-18Z', 'm10 8 6 4-6 4V8Z'],
  audio: ['M3 10v4M7 6v12M12 3v18M17 7v10M21 10v4'],
  code: ['m7 7-5 5 5 5M17 7l5 5-5 5M14 4l-4 16'],
  grid: ['M3 3h7v7H3V3ZM14 3h7v7h-7V3ZM3 14h7v7H3v-7ZM14 14h7v7h-7v-7Z'],
  route: [
    'M12 3a3 3 0 1 1 0 6 3 3 0 0 1 0-6ZM5 16a3 3 0 1 1 0 6 3 3 0 0 1 0-6ZM19 16a3 3 0 1 1 0 6 3 3 0 0 1 0-6Z',
    'm10 9-4 7M14 9l4 7M8 19h8'
  ],
  coins: [
    'M12 3c5 0 8 1.5 8 3s-3 3-8 3-8-1.5-8-3 3-3 8-3ZM4 6v5c0 1.7 3 3 8 3 1.2 0 2.3-.1 3.3-.3M20 6v4M4 11v5c0 1.7 3 3 8 3h1',
    'M19 12c3 0 4 1 4 2s-1 2-4 2-4-1-4-2 1-2 4-2ZM15 14v5c0 1 1 2 4 2s4-1 4-2v-5'
  ],
  shield: ['M12 2c3 2 6 3 9 3v7c0 5-5 8-9 10-4-2-9-5-9-10V5c3 0 6-1 9-3Z', 'm8 12 3 3 5-6'],
  arrow: ['M4 12h16m-6-6 6 6-6 6'],
  cube: ['m12 2 9 5v10l-9 5-9-5V7l9-5Z', 'm3 7 9 5 9-5M12 12v10M7.5 4.5l9 5'],
  user: ['M12 3a4 4 0 1 1 0 8 4 4 0 0 1 0-8ZM3 21v-2c0-4 4-6 9-6s9 2 9 6v2'],
  bolt: ['m14 2-9 12h7l-2 8 9-12h-7l2-8Z'],
  more: ['M5 12h.01M12 12h.01M19 12h.01'],
  search: ['M10.5 3a7.5 7.5 0 1 1 0 15 7.5 7.5 0 0 1 0-15ZM16 16l5 5']
}
</script>

<template>
  <svg
    class="home-icon"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path v-for="(path, i) in paths[name] || paths.code" :key="i" :d="path" />
  </svg>
</template>

<style scoped>
.home-icon {
  display: inline-block;
  flex: 0 0 auto;
  width: 1em;
  height: 1em;
  vertical-align: middle;
}
</style>
