<template>
  <div class="space-y-6">
    <ConfigurationHeader />

    <!-- Welcome Card for new users -->
    <LazyWelcomeCard v-if="shouldShowWelcomeCard" :has-branch-prefix="!!repository.effectiveBranchPrefix.value" />

    <!-- Error Alert (inline, no card) -->
    <UAlert
      v-if="(syncError || pathValidation.error) && !isSyncing"
      :title="syncError || pathValidation.error || undefined"
      :description="pathValidation.errorDetails || undefined"
      color="error"
      icon="i-lucide-x-circle"
      variant="soft"
    />
    <!-- Empty State -->
    <UAlert
      v-else-if="hasCompletedSync && branches.length === 0 && unassignedCommits.length === 0"
      color="info"
      title="No branches found"
      variant="soft"
    />

    <!-- Unassigned Commits Card -->
    <UnassignedCommitListCard
      v-if="unassignedCommits.length > 0"
      :commits="unassignedCommits"
    />

    <!-- Branches Table Card -->
    <BranchTableCard />

  </div>
</template>

<script lang="ts" setup>

// Provide repository state
const repository = provideRepository()

// Initialize model download handler
useModelDownload()

const branchSync = createBranchSyncState(repository)
provide(BranchSyncKey, branchSync)

const { syncError, isSyncing, branches, unassignedCommits, hasCompletedSync } = branchSync
const { pathValidation } = repository

// Determine if we should show the welcome card
const shouldShowWelcomeCard = computed(() => {
  // Show if:
  // 1. No repository is selected
  // 2. No recent projects exist
  const hasNoRepository = !repository.selectedProject.value
  const hasNoRecentProjects = repository.recentProjects.value.length === 0

  return hasNoRepository && hasNoRecentProjects
})
</script>
