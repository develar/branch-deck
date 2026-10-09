<template>
  <div :class="['copy-button transition-opacity', !alwaysVisible && 'opacity-0']">
    <!-- Keep the tooltip open on click so its text switches to "Copied!" in place
         (closing and reopening it during the exit animation makes reka-ui close it again).
         Explicit ariaLabel: reka-ui otherwise snapshots the accessible text on mount -->
    <UTooltip
      v-model:open="tooltipOpen"
      :text="tooltipText"
      :content="{ ariaLabel: tooltipText }"
      disable-closing-trigger
    >
      <UButton
        :icon="isCopied ? 'i-lucide-copy-check' : 'i-lucide-copy'"
        :size="size"
        :variant="variant"
        color="neutral"
        :class="{ 'text-muted': alwaysVisible }"
        @click.stop="handleCopy"
      />
    </UTooltip>
  </div>
</template>

<script lang="ts" setup>
// useCopyToClipboard is auto-imported

interface Props {
  text: () => string
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
const { isCopied, tooltipOpen, copyToClipboard } = useCopyToClipboard()

// Computed property for dynamic tooltip text
const tooltipText = computed(() => {
  return isCopied.value ? "Copied!" : props.tooltip
})

// Handle click - only evaluate text when needed
const handleCopy = () => {
  const text = props.text()
  copyToClipboard(text)
}
</script>
