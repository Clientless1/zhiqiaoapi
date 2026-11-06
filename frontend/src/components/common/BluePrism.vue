<script setup lang="ts">
import { useId } from 'vue'
const id = useId().replace(/:/g, '')
const paint = (name: string) => `url(#${id}-${name})`
</script>

<template>
  <svg class="blue-prism" viewBox="0 0 620 450" fill="none" aria-hidden="true">
    <defs>
      <linearGradient
        :id="`${id}-top`"
        x1="210"
        y1="95"
        x2="400"
        y2="200"
        gradientUnits="userSpaceOnUse"
      >
        <stop stop-color="#fff" stop-opacity=".96" />
        <stop offset=".5" stop-color="#c6deff" stop-opacity=".76" />
        <stop offset="1" stop-color="#6b9fff" stop-opacity=".91" />
      </linearGradient>
      <linearGradient
        :id="`${id}-left`"
        x1="200"
        y1="160"
        x2="340"
        y2="310"
        gradientUnits="userSpaceOnUse"
      >
        <stop stop-color="#91c7ff" stop-opacity=".85" />
        <stop offset=".5" stop-color="#5593ff" stop-opacity=".78" />
        <stop offset="1" stop-color="#2058ed" />
      </linearGradient>
      <linearGradient
        :id="`${id}-right`"
        x1="450"
        y1="160"
        x2="310"
        y2="340"
        gradientUnits="userSpaceOnUse"
      >
        <stop stop-color="#6295ff" stop-opacity=".9" />
        <stop offset="1" stop-color="#1745e7" />
      </linearGradient>
      <linearGradient
        :id="`${id}-base`"
        x1="150"
        y1="320"
        x2="450"
        y2="390"
        gradientUnits="userSpaceOnUse"
      >
        <stop stop-color="#f3f8ff" />
        <stop offset=".52" stop-color="#abc9ff" />
        <stop offset="1" stop-color="#699bff" />
      </linearGradient>
      <radialGradient :id="`${id}-aura`">
        <stop stop-color="#487cff" stop-opacity=".32" />
        <stop offset="1" stop-color="#80aaff" stop-opacity="0" />
      </radialGradient>
      <filter :id="`${id}-glow`" x="-100%" y="-100%" width="300%" height="300%">
        <feGaussianBlur stdDeviation="8" />
      </filter>
    </defs>
    <ellipse cx="319" cy="305" rx="230" ry="120" :fill="paint('aura')" />
    <g class="prism-orbit">
      <ellipse
        cx="313"
        cy="253"
        rx="272"
        ry="92"
        transform="rotate(-23 313 253)"
        stroke="#82a9f4"
        stroke-opacity=".25"
      />
      <ellipse
        cx="313"
        cy="253"
        rx="248"
        ry="82"
        transform="rotate(-23 313 253)"
        stroke="white"
        stroke-opacity=".9"
      />
      <circle cx="89" cy="343" r="4" fill="#5d8eff" />
      <circle cx="542" cy="163" r="4" fill="#fff" />
    </g>
    <g class="prism-platform">
      <path
        d="M142 320 317 222 493 320 493 343 317 442 142 343Z"
        fill="#81aaff"
        fill-opacity=".12"
      />
      <path d="M149 321 317 225 485 321 317 418Z" fill="white" fill-opacity=".38" stroke="white" />
      <path
        d="M172 305 317 223 462 305 462 326 317 408 172 326Z"
        :fill="paint('base')"
        fill-opacity=".66"
        stroke="#d4e4ff"
      />
      <path
        d="M172 305 317 223 462 305 317 387Z"
        fill="white"
        fill-opacity=".57"
        stroke="white"
        stroke-width="1.5"
      />
      <path d="M197 305 317 237 437 305 317 373Z" stroke="white" stroke-opacity=".75" />
    </g>
    <g class="prism-cube">
      <path
        d="M317 63 440 133 317 204 194 133Z"
        :fill="paint('top')"
        stroke="white"
        stroke-width="1.4"
      />
      <path
        d="M194 133 317 204 317 347 194 276Z"
        :fill="paint('left')"
        stroke="#c7e2ff"
        stroke-width="1.1"
      />
      <path
        d="M317 204 440 133 440 276 317 347Z"
        :fill="paint('right')"
        stroke="#a4caff"
        stroke-width="1.1"
      />
      <path
        d="M256 98 379 169M256 169 379 98M235 110 358 181M276 86 399 157M235 157 358 86M276 181 399 110"
        stroke="white"
        stroke-opacity=".55"
      />
      <path
        d="M235 157V300M276 181V324M358 181V324M399 157V300M194 181 317 252 440 181M194 228 317 299 440 228"
        stroke="white"
        stroke-opacity=".48"
      />
      <path d="M317 111 359 135 317 160 275 135Z" fill="white" :filter="paint('glow')" />
      <path d="M317 117 349 135 317 154 285 135Z" fill="white" fill-opacity=".97" />
      <path d="M276 181 317 204 358 181V228L317 252 276 228Z" fill="#d5edff" fill-opacity=".15" />
      <path d="M317 204V252" stroke="white" stroke-opacity=".75" />
      <path d="M194 133 317 204 440 133" stroke="white" stroke-width="2" />
    </g>
    <g fill="#6c99ef">
      <path d="m114 161 3 10 10 3-10 3-3 10-3-10-10-3 10-3Z" />
      <path d="m479 71 4 13 13 4-13 4-4 13-4-13-13-4 13-4Z" fill="#97b9ff" />
      <path d="m484 337 2 7 7 2-7 2-2 7-2-7-7-2 7-2Z" />
    </g>
    <circle cx="161" cy="88" r="2" fill="white" />
    <circle cx="475" cy="251" r="2" fill="white" />
    <circle cx="250" cy="42" r="2" fill="#7ba8f9" />
  </svg>
</template>

<style scoped>
.blue-prism {
  display: block;
  width: 100%;
  overflow: visible;
}
.prism-cube {
  animation: prism-float 7s ease-in-out infinite;
}
.prism-orbit {
  animation: prism-orbit 10s ease-in-out infinite;
  transform-origin: center;
}
@keyframes prism-float {
  0%,
  100% {
    transform: translateY(0);
  }
  50% {
    transform: translateY(-9px);
  }
}
@keyframes prism-orbit {
  0%,
  100% {
    opacity: 0.65;
  }
  50% {
    opacity: 1;
  }
}
@media (prefers-reduced-motion: reduce) {
  .prism-cube,
  .prism-orbit {
    animation: none;
  }
}
</style>
