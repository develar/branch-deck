<template>
  <DialogRoot :open="isActive">
    <DialogPortal :to="portalTarget ? `#${portalTarget}` : undefined">
      <DialogContent
        @open-auto-focus="handleOpenAutoFocus"
        @escape-key-down="handleEscapeKey"
        @close-auto-focus.prevent
      >
        <VisuallyHidden>
          <DialogTitle>{{ dialogTitle }}</DialogTitle>
          <DialogDescription>{{ dialogDescription }}</DialogDescription>
        </VisuallyHidden>
        <div class="bd-padding-content border-b border-default bg-elevated/50" :data-testid="$attrs['data-testid']">
          <div class="space-y-2">
            <!-- Compact single-line input with integrated actions -->
            <div class="flex items-center gap-2">
              <!-- Input with flexible content -->
              <div class="relative flex-1">
                <UInput
                  ref="inputRef"
                  :model-value="modelValue"
                  :placeholder="placeholder"
                  size="md"
                  autofocus
                  :color="validationState?.color || 'primary'"
                  autocapitalize="off"
                  autocorrect="off"
                  spellcheck="false"
                  @update:model-value="$emit('update:modelValue', $event)"
                  @keydown.enter.prevent="$emit('submit')"
                >
                  <template #leading>
                    <slot name="leading-icon" />
                  </template>
                </UInput>
              </div>

              <!-- Extra actions slot (for AI icons, etc.) -->
              <slot name="extra-actions" />

              <!-- Standard action buttons -->
              <div class="flex items-center gap-2">
                <UButton
                  size="xs"
                  variant="ghost"
                  color="neutral"
                  @click="() => emit('cancel')"
                >
                  Cancel
                </UButton>
                <UButton
                  size="xs"
                  variant="solid"
                  :disabled="!isValid"
                  @click="() => emit('submit')"
                >
                  {{ primaryButtonText }}
                </UButton>
              </div>
            </div>

            <!-- Validation message (only if needed) -->
            <div v-if="validationState?.message" :class="['text-xs', validationState.textClass]">
              <slot name="validation-message">
                {{ validationState.message }}
              </slot>
            </div>

            <!-- Help text -->
            <div v-if="$slots['help-text']" class="text-xs text-muted">
              <slot name="help-text" />
            </div>

            <!-- After controls slot for additional content like suggestions -->
            <slot name="after-controls" />
          </div>
        </div>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<script lang="ts" setup>
import { useTemplateRef } from "vue"
import { DialogRoot, DialogPortal, DialogContent, DialogTitle, DialogDescription, VisuallyHidden } from "reka-ui"

interface ValidationState {
  color: "primary" | "success" | "error"
  message?: string
  textClass?: string
}

interface Props {
  modelValue: string
  isActive?: boolean
  placeholder: string
  primaryButtonText: string
  isValid: boolean
  validationState?: ValidationState
  dialogTitle?: string
  dialogDescription?: string
  portalTarget?: string
}

withDefaults(defineProps<Props>(), {
  isActive: true,
  validationState: undefined,
  dialogTitle: "Inline Input",
  dialogDescription: "Enter the required information",
  portalTarget: undefined,
})

const emit = defineEmits<{
  "update:modelValue": [value: string]
  "submit": []
  "cancel": []
}>()

// Template refs
const inputRef = useTemplateRef("inputRef")

function handleOpenAutoFocus(event: Event) {
  event.preventDefault()
  inputRef.value?.inputRef?.focus()
}

function handleEscapeKey(event: Event) {
  event.preventDefault()
  event.stopPropagation() // Prevent ESC from bubbling up to clear selection
  emit("cancel")
}

// Select all text in the input
function selectText() {
  inputRef.value?.inputRef?.select()
}

// Focus the input
function focusInput() {
  inputRef.value?.inputRef?.focus()
}

// Expose methods for parent components
defineExpose({
  selectText,
  focusInput,
})
</script>
