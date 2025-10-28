<template>
  <div class="space-y-6" data-testid="branch-creator-root">
    <ConfigurationHeader />

    <!-- Welcome Card for new users or when branch prefix is missing -->
    <LazyWelcomeCard
      v-if="shouldShowWelcomeCard"
      :has-repository="hasRepository"
      :has-branch-prefix="!!repository.effectiveBranchPrefix.value"
    />

    <!-- Error Alert (inline, no card) -->
    <UAlert
      v-if="(syncError || pathValidation.error) && !isSyncing"
      :title="syncError || pathValidation.error || undefined"
      :description="pathValidation.errorDetails || undefined"
      color="error"
      icon="i-lucide-x-circle"
      variant="soft"
      data-testid="error-alert"
    />
    <!-- Empty State -->
    <UAlert
      v-else-if="hasCompletedSync && branches.length === 0 && unassignedCommits.length === 0"
      icon="i-lucide-git-branch"
      color="neutral"
      title="No active branches"
      variant="subtle"
      data-testid="empty-state-alert"
    >
      <template #description>
        <div class="space-y-3">
          <p>
            Commit to your main branch using prefixes like:
          </p>
          <ul class="list-disc list-inside font-mono">
            <li>(auth) Add login form</li>
            <li>YT-123 Fix bug</li>
          </ul>
          <p>
            BranchDeck will automatically group your commits into virtual branches.
          </p>
          <p v-if="archivedBranches.archivedBranches.value.length > 0" class="text-muted text-sm">
            Your archived branches are shown below.
          </p>
        </div>
      </template>
    </UAlert>

    <!-- Unassigned Commits Card -->
    <LazyUnassignedCommitListCard
      v-if="unassignedCommits.length > 0"
      :commits="unassignedCommits"
    />

    <!-- Branches Table Card -->
    <BranchTableCard />

    <!-- Archived Branches Card -->
    <ArchivedBranchTableCard />
  </div>
</template>

<script lang="ts" setup>
import { getCurrentWindow } from "@tauri-apps/api/window"
import pDebounce from "p-debounce"
import { scopedCustomListen } from "~/utils/listen"

// Provide model state for AI features
provideModelState()

// Provide repository state synchronously
const repository = provideRepository()

const branchSync = createBranchSyncState(repository)
provide(BranchSyncKey, branchSync)

const { syncError, isSyncing, branches, unassignedCommits, hasCompletedSync, archivedBranches, syncBranches } = branchSync
const { pathValidation, isLoadingBranchPrefix } = repository

// Auto-sync on window focus
const appSettings = useAppSettingsStore()

// Sync menu checkbox with settings
useMenuSync()

const debouncedSync = pDebounce(async () => {
  // Only sync if the setting is enabled and we're not already syncing
  if (appSettings.autoSyncOnFocus && !isSyncing.value) {
    await syncBranches()
  }
}, 100)

scopedCustomListen("appWindow.onFocusChanged", () => {
  const appWindow = getCurrentWindow()
  return appWindow.onFocusChanged(({ payload: focused }) => {
    if (focused) {
      void debouncedSync()
    }
  })
})

// Basic repository flags
// Consider repository selected as soon as a project is chosen
const hasRepository = computed(() => !!repository.selectedProject.value)
const hasBranchPrefix = computed(() => !!repository.effectiveBranchPrefix.value)

// Determine if we should show the welcome card
const shouldShowWelcomeCard = computed(() => {
  // Show if:
  // 1. No repository is selected AND no recent projects exist (first-run)
  // 2. A repository is selected BUT branch prefix is not configured (guide user to Step 2)
  // Don't show if there's a path validation error (let error alert handle it)
  const hasNoRecentProjects = repository.recentProjects.value.length === 0

  if (!hasRepository.value) {
    return hasNoRecentProjects
  }

  // Don't show welcome card if repository path validation is still in progress
  if (isLoadingBranchPrefix.value) {
    return false
  }

  // Don't show welcome card if there's an error that will be displayed by the error alert
  // This prevents both welcome card and error alert from showing simultaneously
  if (pathValidation.value.error && !isSyncing.value) {
    return false
  }

  return !hasBranchPrefix.value
})
</script>
