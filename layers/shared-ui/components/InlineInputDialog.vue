<template>
  <InlineForm
    :open="open"
    :title="title"
    :description="description"
    :portal-target="portalTarget"
    :data-test-id="dataTestId"
    :content-class="'space-y-2'"
    :show-actions="false"
    :can-submit="canSubmit"
    :loading="loading"
    :submit-text="submitText"
    :cancel-text="cancelText"
    @open-auto-focus="handleOpenAutoFocus"
    @cancel="emit('cancel')"
    @submit="emit('submit')"
  >
    <!-- Main content with inline buttons -->
    <div class="flex items-center gap-2">
      <!-- Input with flexible content -->
      <div class="relative flex-1">
        <slot name="input">
          <!-- Default input if no custom input provided -->
          <!-- @vue-expect-error @keydown event works -->
          <UInput
            ref="inputRef"
            :model-value="modelValue"
            :placeholder="placeholder"
            size="md"
            autofocus
            :color="validationColor"
            autocapitalize="off"
            autocorrect="off"
            spellcheck="false"
            @update:model-value="(value) => emit('update:modelValue', value)"
            @keydown.enter.prevent="handleSubmit"
          >
            <template v-if="$slots['leading-icon']" #leading>
              <slot name="leading-icon" />
            </template>
          </UInput>
        </slot>
      </div>

      <!-- Extra actions slot (for AI icons, etc.) -->
      <slot name="extra-actions" />

      <!-- Inline action buttons -->
      <div class="flex items-center gap-2">
        <UButton
          size="xs"
          variant="ghost"
          color="neutral"
          @click="emit('cancel')"
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
    </div>

    <!-- Validation message -->
    <div v-if="hasInteracted && validationMessage" :class="['text-xs', validationTextClass]">
      <slot name="validation-message">
        {{ validationMessage }}
      </slot>
    </div>

    <!-- Help text -->
    <div v-if="$slots['help-text']" class="text-xs text-muted">
      <slot name="help-text" />
    </div>

    <!-- After controls slot for additional content like suggestions -->
    <slot name="after-controls" />
  </InlineForm>
</template>

<script lang="ts" setup>
import { useTemplateRef } from "vue"

interface Props {
  // Dialog props
  open?: boolean
  title?: string
  description?: string
  portalTarget?: string
  dataTestId?: string
  cancelText?: string
  submitText?: string
  canSubmit?: boolean
  loading?: boolean

  // Input props
  modelValue?: string
  placeholder?: string
  validationMessage?: string
  validationColor?: "primary" | "error"
  validationTextClass?: string
}

const props = withDefaults(defineProps<Props>(), {
  open: true,
  title: "Inline Form",
  description: "Enter the required information",
  portalTarget: undefined,
  dataTestId: undefined,
  cancelText: "Cancel",
  submitText: "Submit",
  canSubmit: true,
  loading: false,
  modelValue: "",
  placeholder: "",
  validationMessage: undefined,
  validationColor: "primary",
  validationTextClass: "",
})

const emit = defineEmits<{
  "update:modelValue": [value: string]
  "cancel": []
  "submit": []
}>()

// Template refs
const inputRef = useTemplateRef("inputRef")

// Interaction tracking for validation UX
const hasInteracted = ref(false)

// Reset hasInteracted when form opens
watch(() => props.open, (newValue, oldValue) => {
  if (newValue && !oldValue) {
    // Form is opening, reset interaction state
    hasInteracted.value = false
  }
})

// Handle submit with validation UX
function handleSubmit() {
  if (!props.canSubmit) {
    // User tried to submit with invalid input - show validation message
    hasInteracted.value = true
    return
  }
  emit("submit")
}

// Handle auto focus for input
function handleOpenAutoFocus(event: Event) {
  event.preventDefault()
  inputRef.value?.inputRef?.focus()
}

// Expose methods for parent components
function selectText() {
  inputRef.value?.inputRef?.select()
}

function focusInput() {
  inputRef.value?.inputRef?.focus()
}

defineExpose({
  selectText,
  focusInput,
})
</script>
