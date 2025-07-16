<template>
  <div class="space-y-6">
    <UCard>
      <div class="space-y-6">
        <!-- Repository Path -->
        <RepositoryPathField
          :disabled="isSyncing"
        />

        <!-- Branch Prefix -->
        <BranchPrefixField
          :disabled="isSyncing"
        />

        <!-- Actions -->
        <UButton
          v-if="!showProgress"
          :disabled="isSyncing || !store.repositoryPath || !store.branchPrefix || !store.pathValidation.valid"
          :loading="isSyncing && !showProgress"
          icon="i-lucide-refresh-cw"
          @click="syncBranches()"
        >
          Sync Virtual Branches
        </UButton>
        <!-- Loading State -->
        <div v-else-if="showProgress" class="flex flex-col items-center justify-center gap-3 py-2">
          <span class="text-sm text-dimmed">
            {{ syncProgress }}
          </span>
          <UProgress/>
        </div>
      </div>
    </UCard>

    <!-- Error Card -->
    <UCard v-if="syncError && !isSyncing">
      <UAlert
        :title="`Error: ${syncError}`"
        color="error"
        icon="i-lucide-x-circle"
        variant="soft"
      />
    </UCard>

    <!-- Unassigned Commits Card -->
    <UnassignedCommitListCard
      v-if="unassignedCommits.length > 0"
      :commits="unassignedCommits"
      :repository-path="store.repositoryPath"
      :branch-prefix="store.branchPrefix"
      @refresh="syncBranches()"
    />

    <!-- Empty State Card -->
    <UCard v-if="hasCompletedSync && branches.length === 0 && unassignedCommits.length === 0">
      <UAlert
        color="info"
        title="No branches found"
        variant="soft"
      />
    </UCard>

    <!-- Branches Table Card -->
    <BranchTableCard
      ref="branchTableCard"
      :branches="branches"
      :is-syncing="isSyncing"
      @push-branch="pushBranch"
      @refresh="syncBranches"
    />

  </div>
</template>

<script lang="ts" setup>
import { useSyncBranches } from "~/composables/branch/syncBranches"
import { usePush } from "~/composables/git/push"
import { useRepositoryStore } from "~/stores/repository"

// Use the repository store
const store = useRepositoryStore()

// Ref to the branches table card component
const branchTableCard = ref<InstanceType<typeof import("~/components/branchList/BranchTableCard.vue").default>>()

// Function to expand a branch in the table
const expandBranch = (branchName: string, scroll: boolean) => {
  if (branchTableCard.value) {
    branchTableCard.value.expandBranch(branchName, scroll)
  }
}

// Initialize sync and push composables
const { syncBranches, syncError, isSyncing, showProgress, syncProgress, branches, unassignedCommits, hasCompletedSync } = useSyncBranches(store.vcsRequestFactory, expandBranch)
const { pushBranch } = usePush(store.vcsRequestFactory)

</script>
