<template>
  <PopoverRoot :open="isOpen">
    <PopoverAnchor :reference="targetElement || undefined" />
    <PopoverPortal>
      <PopoverContent
        update-position-strategy="always"
        :side-offset="8"
        side="top"
        align="center"
        :collision-padding="8"
        class="z-50"
        as-child
      >
        <UButton
          size="xs"
          variant="outline"
          color="primary"
          icon="i-lucide-git-branch"
          class="shadow-sm hover:shadow-md transition-shadow"
          data-testid="floating-selection-bar"
          @click="handleCreateBranch"
        >
          Group into Branch
        </UButton>
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>

<script lang="ts" setup>
import { PopoverRoot, PopoverAnchor, PopoverPortal, PopoverContent } from "reka-ui"
import { useTimeoutFn } from "@vueuse/core"

const props = defineProps<{
  selectedCount: number
  targetElement?: HTMLElement | null
  isInlineCreationActive?: boolean
}>()

const emit = defineEmits<{
  "create-branch": []
}>()

// Control popover visibility with delay
const shouldShow = computed(() =>
  props.selectedCount > 0 && !props.isInlineCreationActive && !!props.targetElement,
)

// Add show delay to prevent flickering
const isOpen = ref(false)

// Use timeout for delayed show
const { start: startShowTimer, stop: stopShowTimer } = useTimeoutFn(() => {
  // Double-check conditions before showing
  if (shouldShow.value) {
    isOpen.value = true
  }
}, 150)

// Watch for show with delay, hide immediately
watch(shouldShow, (newValue) => {
  if (newValue) {
    // Delay showing by 150ms
    startShowTimer()
  }
  else {
    // Hide immediately and cancel any pending show
    stopShowTimer()
    isOpen.value = false
  }
}, { immediate: true })

// Handle button click
function handleCreateBranch() {
  emit("create-branch")
}
</script>
