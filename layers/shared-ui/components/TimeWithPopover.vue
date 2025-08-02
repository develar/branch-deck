<template>
  <UPopover v-if="committerTime && authorTime !== committerTime" mode="hover">
    <span>{{ formatTimestamp(authorTime) }}</span>
    <template #content>
      <div class="p-3">
        <div class="text-xs" style="display: grid; grid-template-columns: auto 1fr; column-gap: 0.5rem; row-gap: 0.25rem;">
          <div class="text-muted">Authored</div>
          <div class="text-highlighted">{{ formatTimestamp(authorTime) }}</div>
          <div class="text-muted">Committed</div>
          <div class="text-highlighted">{{ formatTimestamp(committerTime) }}</div>
        </div>
      </div>
    </template>
  </UPopover>
  <span v-else>{{ formatTimestamp(authorTime) }}</span>
</template>

<script lang="ts" setup>
defineProps<{
  authorTime: number
  committerTime?: number
}>()
</script>
