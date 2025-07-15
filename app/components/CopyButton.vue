<template>
  <UTooltip :text="tooltipText">
    <UButton
      :icon="copiedItems.has(text) ? 'i-lucide-copy-check' : 'i-lucide-copy'"
      :size="size"
      :variant="variant"
      :class="[
        'transition-all',
        copiedItems.has(text) ? 'opacity-100 text-success' : 'opacity-0 group-hover:opacity-100'
      ]"
      @click.stop="copyToClipboard(text)"
    />
  </UTooltip>
</template>

<script lang="ts" setup>
interface Props {
  text: string
  tooltip?: string
  size?: "xs" | "sm" | "md" | "lg" | "xl"
  variant?: "solid" | "outline" | "soft" | "ghost" | "link"
}

const props = withDefaults(defineProps<Props>(), {
  size: "xs",
  variant: "ghost",
  tooltip: "Copy to clipboard",
})

// Use the copy to clipboard composable
const { copiedItems, copyToClipboard } = useCopyToClipboard()

// Computed property for dynamic tooltip text
const tooltipText = computed(() => {
  return copiedItems.value.has(props.text) ? "Copied!" : props.tooltip
})
</script>