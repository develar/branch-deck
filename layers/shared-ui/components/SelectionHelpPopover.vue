<template>
  <UPopover mode="hover">
    <UIcon name="i-lucide-info" class="size-3 text-muted hover:text-highlighted transition-colors cursor-help" />
    <template #content>
      <div class="p-3 space-y-2 text-xs">
        <div class="font-medium text-highlighted mb-2">Selection shortcuts</div>
        <div class="space-y-1.5">
          <div>Click to select a commit</div>
          <div class="flex items-center gap-1">
            <UKbd size="sm">{{ isMac ? '⌘' : 'Ctrl' }}</UKbd>
            <span>+ Click for multiple</span>
          </div>
          <div class="flex items-center gap-1">
            <UKbd size="sm">Shift</UKbd>
            <span>+ Click for range</span>
          </div>
          <div class="flex items-center gap-1">
            <UKbd size="sm">{{ isMac ? '⌘' : 'Ctrl' }}</UKbd>
            <UKbd size="sm">A</UKbd>
            <span>Select all</span>
          </div>
          <div class="flex items-center gap-1">
            <UKbd size="sm">Esc</UKbd>
            <span>Clear selection</span>
          </div>
        </div>
      </div>
    </template>
  </UPopover>
</template>

<script lang="ts" setup>
// Platform detection
const isMac = computed(() => navigator.userAgent.toLowerCase().includes("mac"))
</script>