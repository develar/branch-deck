<template>
  <InlineForm
    :open="isActive"
    :title="dialogTitle"
    :description="dialogDescription"
    :portal-target="portalTarget"
    :can-submit="canUnapply"
    :loading="processing"
    submit-text="Unapply Branch"
    @cancel="closeInline"
    @submit="onSubmit"
  >
    <div class="flex items-start gap-3 p-4 bg-warning/10 border border-warning/20 rounded-lg">
      <UIcon name="i-lucide-alert-triangle" class="size-5 text-warning flex-shrink-0 mt-0.5" />
      <div class="space-y-2 text-sm">
        <p class="font-medium text-warning">
          This will remove {{ commitCount }} commit{{ commitCount === 1 ? '' : 's' }} from HEAD
        </p>
        <p class="text-muted">
          The commits will be preserved in the unapplied branch, but removed from your current working branch.
          This action can be undone by reapplying the commits later.
        </p>
      </div>
    </div>

    <template #submit-icon>
      <UIcon name="i-lucide-archive-x" class="size-3" />
    </template>
  </InlineForm>
</template>

<script lang="ts" setup>
// Access context and composables
const { branches } = useBranchSync()
const { unapplyBranch } = useUnapplyBranch()
const inline = useInlineRowAction()
const { activeInline, portalTargetIdFor, closeInline } = inline

// Computed properties from activeInline
const branchName = computed(() => activeInline.value?.branchName || "")
const isActive = computed(() => {
  return activeInline.value?.type === "unapply" && !!activeInline.value?.branchName
})
const dialogTitle = computed(() => branchName.value ? `Unapply ${branchName.value}` : "")
const dialogDescription = computed(() =>
  branchName.value
    ? `Remove virtual branch commits from HEAD and move branch to unapplied state`
    : "",
)
const portalTarget = computed(() => {
  return branchName.value ? portalTargetIdFor(branchName.value) : undefined
})

// Get the branch data
const branch = computed(() => branches.value.find(b => b.name === branchName.value))
const commitCount = computed(() => branch.value?.commits?.length || 0)
const processing = computed(() => branchName.value ? inline.isProcessing(branchName.value) : false)

// Submit handler: unapply the branch
const onSubmit = () => {
  if (!branch.value) {
    return
  }
  unapplyBranch(branch.value)
}

// Computed properties
const canUnapply = computed(() => {
  return !!branch.value && commitCount.value > 0 && !processing.value
})
</script>