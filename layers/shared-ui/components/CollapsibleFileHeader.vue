<template>
  <AccordionHeader>
    <AccordionTrigger
      class="file-header w-full text-left group px-3 py-2 hover:bg-muted/50 transition-colors flex items-center justify-between cursor-pointer"
    >
      <div class="flex items-center gap-1 flex-1">
        <span class="font-mono text-xs text-toned select-text">{{ fileName }}</span>
        <CopyButton
          :text="fileName"
          tooltip="Copy file name to clipboard"
        />
      </div>
      <div class="flex items-center gap-1">
        <slot name="actions" />
        <UIcon
          name="i-lucide-chevron-down"
          class="w-4 h-4 text-muted transition-transform duration-200 group-data-[state=open]:rotate-180"
        />
      </div>
    </AccordionTrigger>
  </AccordionHeader>
</template>

<script lang="ts" setup>
import { AccordionHeader, AccordionTrigger } from "reka-ui"

defineProps<{
  fileName: string
}>()
</script>

<style scoped>
/* Default state - invisible with no delay when hiding */
.file-header .copy-button {
  transition: opacity 200ms ease-out;
}

/* Hover state - visible with delay when showing */
.file-header:hover .copy-button {
  opacity: 1;
  transition: opacity 200ms ease-out 300ms;
}
</style>
