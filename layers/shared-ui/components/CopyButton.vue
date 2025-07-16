<template>
  <UTooltip
    v-model:open="tooltipOpen"
    :text="tooltipText"
  >
    <UButton
      :icon="copiedItems.has(text) ? 'i-lucide-copy-check' : 'i-lucide-copy'"
      :size="size"
      :variant="variant"
      :class="[
        'transition-all duration-200',
        copiedItems.has(text)
          ? 'opacity-100 text-success'
          : 'opacity-0 group-hover:opacity-100 group-hover:transition-delay-300'
      ]"
      @click.stop="handleCopy"
    />
  </UTooltip>
</template>

<script lang="ts" setup>
// useCopyToClipboard is auto-imported

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

// Control tooltip visibility
const tooltipOpen = ref(false)

// Computed property for dynamic tooltip text
const tooltipText = computed(() => {
  return copiedItems.value.has(props.text) ? "Copied!" : props.tooltip
})

// Handle copy action
async function handleCopy() {
  await copyToClipboard(props.text)
  // Force show tooltip with "Copied!" message
  tooltipOpen.value = true

  // Hide tooltip after 2 seconds
  setTimeout(() => {
    tooltipOpen.value = false
  }, 2000)
}
</script>