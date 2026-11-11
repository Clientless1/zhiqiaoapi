<script setup lang="ts">
withDefaults(defineProps<{ compact?: boolean }>(), { compact: false })
</script>
<template>
  <div class="bridge-visual" :class="{ compact }" aria-hidden="true">
    <div class="bridge-orbit orbit-one"></div>
    <div class="bridge-orbit orbit-two"></div>
    <div class="bridge-floor"></div>
    <div class="bridge-core"><i></i><i></i><i></i><i></i></div>
    <span class="bridge-node node-a"><img src="/icons/openai.svg" alt="" /></span>
    <span class="bridge-node node-b"><img src="/icons/gemini.png" alt="" /></span>
    <span class="bridge-node node-c"><img src="/icons/claude.svg" alt="" /></span>
    <b class="bridge-spark spark-a"></b><b class="bridge-spark spark-b"></b>
  </div>
</template>
