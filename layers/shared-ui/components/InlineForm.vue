<template>
  <DialogRoot :open="open">
    <DialogPortal :to="portalTarget ? `#${portalTarget}` : undefined">
      <DialogContent
        @open-auto-focus="handleOpenAutoFocus"
        @escape-key-down="handleEscapeKey"
        @close-auto-focus.prevent
      >
        <VisuallyHidden>
          <DialogTitle>{{ title }}</DialogTitle>
          <DialogDescription>{{ description }}</DialogDescription>
        </VisuallyHidden>

        <!-- Transition between form and progress states -->
        <Transition name="form-progress" mode="out-in">
          <!-- Progress state - slim progress bar -->
          <div
            v-if="isProcessing"
            key="progress"
            class="bd-padding-content border-b border-default bg-elevated/50 py-3"
            :data-testid="`${dataTestId}-progress`">
            <div class="flex items-center space-x-3">
              <div class="flex-shrink-0">
                <UIcon name="i-lucide-loader-2" class="size-4 animate-spin text-primary" />
              </div>
              <div class="flex-1 min-w-0">
                <p class="text-sm font-medium text-default truncate">
                  {{ processingMessage || 'Processing' }}
                </p>
              </div>
            </div>
          </div>

          <!-- Form state - full form content -->
          <div
            v-else
            key="form"
            class="bd-padding-content border-b border-default bg-elevated/50"
            :data-testid="dataTestId">
            <div :class="contentClass">
              <!-- Main content slot -->
              <slot />

              <!-- Actions - can be overridden via slot -->
              <slot name="actions">
                <div v-if="showActions" class="flex items-center justify-end space-x-2 pt-2">
                  <UButton
                    size="xs"
                    variant="ghost"
                    color="neutral"
                    @click="handleCancel"
                  >
                    {{ cancelText }}
                  </UButton>
                  <UButton
                    size="xs"
                    variant="solid"
                    :disabled="!canSubmit"
                    :loading="loading"
                    @click="handleSubmit"
                  >
                    <slot name="submit-icon" />
                    {{ submitText }}
                  </UButton>
                </div>
              </slot>
            </div>
          </div>
        </Transition>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<script lang="ts" setup>
import { DialogRoot, DialogPortal, DialogContent, DialogTitle, DialogDescription, VisuallyHidden } from "reka-ui"

// Get activeInline directly for processing state
const { activeInline } = useInlineRowAction()

interface Props {
  // Dialog props
  open?: boolean
  title?: string
  description?: string
  portalTarget?: string
  dataTestId?: string
  contentClass?: string

  // Action props
  showActions?: boolean
  cancelText?: string
  submitText?: string
  canSubmit?: boolean
  loading?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  open: true,
  title: "Inline Form",
  description: "Enter the required information",
  portalTarget: undefined,
  dataTestId: undefined,
  contentClass: "space-y-4",
  showActions: true,
  cancelText: "Cancel",
  submitText: "Submit",
  canSubmit: true,
  loading: false,
})

const emit = defineEmits<{
  "cancel": []
  "submit": []
  "open-auto-focus": [event: Event]
}>()

// Compute processing state
const isProcessing = computed(() =>
  props.open && activeInline.value?.processing,
)
const processingMessage = computed(() =>
  isProcessing.value ? activeInline.value?.processingMessage : undefined,
)

// Handle auto focus - allow parent to customize
function handleOpenAutoFocus(event: Event) {
  emit("open-auto-focus", event)
}

// Handle escape key properly to prevent bubbling
function handleEscapeKey(event: Event) {
  event.preventDefault()
  event.stopPropagation() // Prevent ESC from bubbling up to clear selection
  emit("cancel")
}

function handleCancel() {
  emit("cancel")
}

function handleSubmit() {
  emit("submit")
}

</script>

<style scoped>
/* Smooth transition between form and progress states */
.form-progress-enter-active,
.form-progress-leave-active {
  transition: all 0.2s ease-in-out;
}

.form-progress-enter-from {
  opacity: 0;
  transform: translateY(-4px);
}

.form-progress-leave-to {
  opacity: 0;
  transform: translateY(4px);
}
</style>