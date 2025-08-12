<template>
  <div class="space-y-4">
    <!-- Conflict Overview and Missing Commits Card -->
    <UCard class="overflow-hidden" :ui="{ body: 'p-0 sm:p-0' }">
      <template #header>
        <div class="flex items-start gap-3">
          <UIcon name="i-lucide-git-merge" class="size-5 text-error mt-0.5 flex-shrink-0" />
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-highlighted mb-1">
              Cherry-pick conflict detected
            </p>
            <p class="text-sm text-toned">
              Cannot apply: {{ conflict.commitMessage }}
            </p>
            <CommitInfo
              :hash="conflict.commitHash"
              :author-time="conflict.commitAuthorTime"
              :committer-time="conflict.commitCommitterTime"
              class="mt-1"
            />
          </div>
        </div>
      </template>

      <!-- Missing Commits Info -->
      <div class="bd-padding-card">
        <div v-if="conflict.conflictAnalysis" class="bg-elevated rounded-lg bd-padding-card border border-warning">
          <div class="flex items-center gap-2">
            <UIcon name="i-lucide-file-diff" class="size-4 text-warning" />
            <span class="text-sm font-medium text-highlighted">Insight: Missing commits modified the conflicting files</span>
          </div>

          <p class="text-sm text-toned mt-2">
            {{ conflict.conflictAnalysis.missingCommits.length }}
            {{ conflict.conflictAnalysis.missingCommits.length === 1 ? "commit is" : "commits are" }}
            missing that altered the files in conflict.
          </p>

          <p class="text-sm text-toned mt-1">
            Common ancestor:
            <CommitHashPopover
              :hash="conflict.conflictAnalysis.mergeBaseHash"
              :message="conflict.conflictAnalysis.mergeBaseMessage"
              :author="conflict.conflictAnalysis.mergeBaseAuthor"
              :author-time="conflict.conflictAnalysis.mergeBaseTime"
            />
            <span class="text-muted">
              ({{ conflict.conflictAnalysis.divergenceSummary.commitsAheadInSource }} commits behind source,
              {{ conflict.conflictAnalysis.divergenceSummary.commitsAheadInTarget }} commits behind target)
            </span>
          </p>
        </div>
      </div>
    </UCard>

    <!-- Detailed Missing Commits Section -->
    <MissingCommitsDetails
      v-if="conflict.conflictAnalysis?.missingCommits.length > 0"
      :missing-commits="conflict.conflictAnalysis.missingCommits"
      :conflict="conflict"
      :branch-name="branchName"
    />

    <!-- Conflicting Files List -->
    <ConflictingFileListCard
      :conflicts="conflict.conflictingFiles"
      :conflict-info="conflict"
      :conflict-marker-commits="conflictMarkerCommits"
      :branch-name="branchName"
    />
  </div>
</template>

<script lang="ts" setup>
import type { MergeConflictInfo } from "~/utils/bindings"
import { computed } from "vue"

const props = defineProps<{
  conflict: MergeConflictInfo
  branchName?: string
}>()

// Get conflict marker commits from the conflict info
const conflictMarkerCommits = computed(() => {
  if (props.conflict.conflictMarkerCommits) {
    // Convert Partial to Record by filtering out undefined values
    const commits = props.conflict.conflictMarkerCommits
    const result: Record<string, { hash: string, message: string, author: string, authorTime: number, committerTime: number }> = {}

    for (const [key, value] of Object.entries(commits)) {
      if (value) {
        result[key] = value
      }
    }

    return result
  }
  return {}
})
</script>
