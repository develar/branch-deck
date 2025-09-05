<template>
  <InlineInputDialog
    v-model="issueReference"
    :open="isActive"
    :placeholder="`Issue reference for ${branchName} (${commitCount} ${commitCount === 1 ? 'commit' : 'commits'})`"
    :validation-message="errorMessage"
    :validation-color="errorMessage ? 'error' : 'primary'"
    :validation-text-class="errorMessage ? 'text-error' : ''"
    :can-submit="isValid"
    :title="dialogTitle"
    :description="dialogDescription"
    :portal-target="portalTarget"
    submit-text="Add"
    data-test-id="inline-issue-input"
    @submit="onSubmit"
    @cancel="closeInline"
  >
    <template #leading-icon>
      <UIcon name="i-lucide-tag" class="size-4 text-muted" />
    </template>

    <template #help-text>
      The issue reference will be added after the branch prefix in commit messages.
    </template>
  </InlineInputDialog>
</template>

<script lang="ts" setup>
// InlineInputDialog is auto-imported from shared-ui layer

// Access context and composables
const { branches } = useBranchSync()
const { addIssueReference } = useAddIssueReference()
const { activeInline, portalTargetIdFor, closeInline } = useInlineRowAction()

// State
const issueReference = ref("")

// Computed properties from activeInline
const branchName = computed(() => activeInline.value?.branchName || "")
const isActive = computed(() => activeInline.value?.type === "issue-reference" && !!activeInline.value?.branchName)
const dialogTitle = computed(() => branchName.value ? `Add Issue Reference to ${branchName.value}` : "")
const dialogDescription = computed(() => branchName.value ? `Add issue reference form for ${branchName.value} branch` : "")
const portalTarget = computed(() => branchName.value ? portalTargetIdFor(branchName.value) : undefined)

// Get branch and compute commit count
const activeBranch = computed(() => branches.value.find(b => b.name === branchName.value))
const commitCount = computed(() => activeBranch.value?.commitCount || 0)

// Submit handler: add issue reference to the active branch
const onSubmit = () => {
  if (!activeBranch.value) {
    return
  }
  addIssueReference(issueReference.value, activeBranch.value)
}

// Validation
const errorMessage = computed(() => {
  if (!issueReference.value) {
    return undefined
  }

  // Allow flexible issue formats: ABC-123, ISSUE-456, etc.
  const pattern = /^[A-Z]+-\d+$/
  if (!pattern.test(issueReference.value)) {
    return "Issue reference must be in format like ABC-123"
  }

  return undefined
})

const isValid = computed(() => {
  if (!issueReference.value) {
    return false
  }

  const pattern = /^[A-Z]+-\d+$/
  return pattern.test(issueReference.value)
})

</script>
