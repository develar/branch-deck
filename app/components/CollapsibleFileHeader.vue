<template>
  <div
    class="w-full px-3 py-2 hover:bg-muted/50 transition-colors flex items-center justify-between cursor-pointer group"
    @click="$emit('toggle')"
  >
    <span class="font-mono text-xs text-toned select-text flex-1">{{ fileName }}</span>
    <div class="flex items-center gap-1">
      <slot name="actions" />
      <UIcon
        :name="expanded ? 'i-lucide-chevron-up' : 'i-lucide-chevron-down'"
        class="w-4 h-4 text-muted"
      />
    </div>
  </div>
</template>

<script lang="ts" setup>
defineProps<{
  fileName: string
  expanded: boolean
}>()

defineEmits<{
  toggle: []
}>()
</script>
