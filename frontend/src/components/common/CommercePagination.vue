<script setup lang="ts">
defineProps<{ total: number; loading?: boolean }>()
const page = defineModel<number>('page', { required: true })
const pageSize = defineModel<number>('pageSize', { required: true })
</script>
<template>
  <el-pagination
    v-if="total > 0"
    v-model:current-page="page"
    v-model:page-size="pageSize"
    class="commerce-pagination"
    :total="total"
    :disabled="loading"
    :page-sizes="[20, 50, 100]"
    layout="prev, pager, next, sizes"
    background
  />
</template>
<style scoped>
.commerce-pagination {
  display: flex;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 8px;
}
</style>
