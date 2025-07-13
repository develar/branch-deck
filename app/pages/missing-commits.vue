<template>
  <div class="w-full px-4 sm:px-6 lg:px-8 py-4">
    <div v-if="missingCommitsData" class="space-y-4">
      <!-- Commit Context -->
      <UCard class="bg-elevated">
        <template #header>
          <ConflictCommitContext
            title="Cannot apply commit"
            :commit-message="missingCommitsData.conflictCommitMessage"
            :commit-hash="missingCommitsData.conflictCommitHash"
            :commit-time="missingCommitsData.conflictCommitTime"
            :branch-name="missingCommitsData.branchName"
          />
        </template>

        <!-- Divergence Info (matching main view) -->
        <div v-if="missingCommitsData.mergeBase" class="bg-elevated rounded-lg p-3 border border-warning">
          <div class="flex items-center gap-2 mb-2">
            <UIcon name="i-lucide-file-diff" class="w-4 h-4 text-warning"/>
            <span class="text-sm font-medium text-highlighted">
              Insight: Missing commits modified the conflicting files
            </span>
          </div>
          
          <p class="text-sm text-toned">
            Common ancestor:
            <CommitHashPopover
              :hash="missingCommitsData.mergeBase.hash"
              :message="missingCommitsData.mergeBase.message"
              :author="missingCommitsData.mergeBase.author"
              :timestamp="missingCommitsData.mergeBase.time"
            />
            <span class="text-muted">
              ({{ missingCommitsData.divergenceSummary.commitsAheadInSource }} commits behind source,
              {{ missingCommitsData.divergenceSummary.commitsAheadInTarget }} commits behind target)
            </span>
          </p>
        </div>
      </UCard>

      <!-- Missing Commits Details (main content) -->
      <MissingCommitsDetails
        v-if="missingCommitsData.missingCommits.length > 0"
        :missing-commits="missingCommitsData.missingCommits"
        :is-in-window="true"
      />

      <!-- No missing commits message -->
      <UCard v-else>
        <div class="text-center py-8">
          <UIcon name="i-lucide-check-circle" class="w-12 h-12 text-success mx-auto mb-3"/>
          <p class="text-sm text-muted">No missing commits found for this conflict.</p>
        </div>
      </UCard>
    </div>

    <!-- Loading state -->
    <div v-else class="flex items-center justify-center min-h-[400px]">
      <div class="text-center">
        <UIcon name="i-lucide-loader-2" class="w-8 h-8 text-muted animate-spin mx-auto mb-3"/>
        <p class="text-sm text-muted">Loading missing commits data...</p>
      </div>
    </div>
  </div>
</template>

<script lang="ts" setup>
import type { MissingCommit } from "~/utils/bindings"
import { useSubWindowData } from "~/composables/useSubWindowData"

// Disable layout for sub-window
definePageMeta({
  layout: false
})

interface MissingCommitsWindowData {
  conflictCommitHash: string
  conflictCommitMessage: string
  conflictCommitTime: number
  branchName: string
  missingCommits: MissingCommit[]
  mergeBase?: {
    hash: string
    message: string
    author: string
    time: number
  }
  divergenceSummary: {
    commitsAheadInSource: number
    commitsAheadInTarget: number
  }
}

const missingCommitsData = useSubWindowData<MissingCommitsWindowData>()
</script>