<template>
  <div :class="['copy-button transition-opacity', !alwaysVisible && 'opacity-0']">
    <UTooltip
      v-model:open="tooltipOpen"
      :text="tooltipText"
    >
      <UButton
        :icon="copiedItems.has(text) ? 'i-lucide-copy-check' : 'i-lucide-copy'"
        :size="size"
        :variant="variant"
        color="neutral"
        :class="{'text-muted': alwaysVisible }"
        @click.stop="copyToClipboard(text)"
      />
    </UTooltip>
  </div>
</template>

<script lang="ts" setup>
// useCopyToClipboard is auto-imported

interface Props {
  text: string
  tooltip?: string
  size?: "xs" | "sm" | "md" | "lg" | "xl"
  variant?: "solid" | "outline" | "soft" | "ghost" | "link"
  alwaysVisible?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  size: "xs",
  variant: "ghost",
  tooltip: "Copy to clipboard",
  alwaysVisible: false,
})

// Use the copy to clipboard composable
const { copiedItems, tooltipOpen, copyToClipboard } = useCopyToClipboard()

// Computed property for dynamic tooltip text
const tooltipText = computed(() => {
  return copiedItems.value.has(props.text) ? "Copied!" : props.tooltip
})
</script>