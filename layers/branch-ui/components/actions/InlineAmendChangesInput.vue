<template>
  <DialogRoot :open="isActive">
    <DialogPortal :to="portalTarget ? `#${portalTarget}` : undefined">
      <DialogContent>
        <VisuallyHidden>
          <DialogTitle>{{ dialogTitle }}</DialogTitle>
          <DialogDescription>{{ dialogDescription }}</DialogDescription>
        </VisuallyHidden>
        <div class="bd-padding-content border-b border-default bg-elevated/50">
          <div class="space-y-4">
            <!-- Uncommitted Changes Preview -->
            <UncommittedChangesCard
              :diff-data="diffData"
              :loading="loading"
              :error="error"
            />

            <!-- Actions -->
            <div class="flex items-center justify-end space-x-2 pt-2">
              <UButton
                size="xs"
                variant="ghost"
                color="neutral"
                @click="cancel"
              >
                Cancel
              </UButton>
              <UButton
                size="xs"
                variant="solid"
                :disabled="!canAmend"
                :loading="processing"
                @click="handleSubmit"
              >
                <UIcon name="i-lucide-edit-3" class="size-3" />
                Amend Changes
              </UButton>
            </div>
          </div>
        </div>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<script lang="ts" setup>
import { DialogRoot, DialogPortal, DialogContent, DialogTitle, DialogDescription, VisuallyHidden } from "reka-ui"
import type { UncommittedChangesResult } from "~/utils/bindings"
import UncommittedChangesCard from "./UncommittedChangesCard.vue"

const props = defineProps<{
  branchName: string
  dialogTitle?: string
  dialogDescription?: string
  portalTarget?: string
  isActive?: boolean
  diffData?: UncommittedChangesResult | null
  loading?: boolean
  error?: string | null
  processing?: boolean
}>()

const emit = defineEmits<{
  cancel: []
  submit: []
}>()

// Computed properties
const canAmend = computed(() => {
  return !props.loading && !props.error && props.diffData?.hasChanges && !props.processing
})

// Handle submission
function handleSubmit() {
  if (canAmend.value) {
    emit("submit")
  }
}

// Cancel action
function cancel() {
  emit("cancel")
}
</script>