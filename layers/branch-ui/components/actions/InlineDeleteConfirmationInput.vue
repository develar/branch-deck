<template>
  <InlineInputDialog
    v-model="inputText"
    :open="isActive"
    :placeholder="`Type ${simpleName} to confirm deletion`"
    :validation-message="errorMessage"
    :validation-color="errorMessage ? 'error' : 'primary'"
    :validation-text-class="errorMessage ? 'text-error' : ''"
    :can-submit="isValid"
    :title="dialogTitle"
    :description="dialogDescription"
    :portal-target="portalTarget"
    submit-text="Delete"
    data-test-id="inline-delete-input"
    @submit="() => emit('submit')"
    @cancel="emit('cancel')"
  >
    <template #leading-icon>
      <UIcon name="i-lucide-trash-2" class="size-4 text-muted" />
    </template>

    <template #help-text>
      This action will permanently delete the archived branch. This cannot be undone.
    </template>
  </InlineInputDialog>
</template>

<script lang="ts" setup>
import { ref, computed } from "vue"
import UIcon from "@nuxt/ui/components/Icon.vue"
import InlineInputDialog from "@layers/shared-ui/components/InlineInputDialog.vue"
import { getSimpleBranchName } from "~/utils/branch"

const props = defineProps<{
  branchName: string
  dialogTitle?: string
  dialogDescription?: string
  portalTarget?: string
  isActive?: boolean
}>()

const emit = defineEmits<{
  cancel: []
  submit: []
}>()

// State
const inputText = ref("")

const simpleName = computed(() => getSimpleBranchName(props.branchName))

const isValid = computed(() => {
  return inputText.value === simpleName.value
})

const errorMessage = computed(() => {
  if (!isValid.value) {
    return `Type the exact name: ${simpleName.value}`
  }
  return undefined
})
</script>
