<template>
  <div
    class="w-full px-3 py-2 hover:bg-muted/50 transition-colors flex items-center justify-between cursor-pointer group"
  >
    <div class="flex items-center gap-1 flex-1">
      <span class="font-mono text-xs text-toned select-text">{{ fileName }}</span>
      <CopyButton :text="fileName" tooltip="Copy file name to clipboard" />
    </div>
    <div class="flex items-center gap-1">
      <slot name="actions" />
      <UIcon
        name="i-lucide-chevron-down"
        class="w-4 h-4 text-muted transition-transform duration-200 group-data-[state=open]:rotate-180"
      />
    </div>
  </div>
</template>

<script lang="ts" setup>
// CopyButton is auto-imported by Nuxt

defineProps<{
  fileName: string
}>()
</script>
