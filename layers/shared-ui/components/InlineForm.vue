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
        <div class="bd-padding-content border-b border-default bg-elevated/50" :data-testid="dataTestId">
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
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<script lang="ts" setup>
import { DialogRoot, DialogPortal, DialogContent, DialogTitle, DialogDescription, VisuallyHidden } from "reka-ui"

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

withDefaults(defineProps<Props>(), {
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