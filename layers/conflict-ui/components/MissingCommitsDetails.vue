<template>
  <UCard class="overflow-hidden" :ui="{ body: 'p-0 sm:p-0' }">
    <template #header>
      <CardHeader
        title="Missing Commits Details"
        :count="missingCommits.length"
        item-singular="commit"
        item-plural="commits"
      >
        <template #actions>
          <!-- Global diff view toggle -->
          <UFieldGroup v-if="hasAnyFileDiffs" size="xs">
            <UButton
              icon="i-lucide-align-left"
              :color="store.conflictDiffViewMode === 'unified' ? 'primary' : 'neutral'"
              variant="outline"
              @click="store.conflictDiffViewMode = 'unified'"
            >
              Unified
            </UButton>
            <UButton
              icon="i-lucide-columns-2"
              :color="store.conflictDiffViewMode === 'split' ? 'primary' : 'neutral'"
              variant="outline"
              @click="store.conflictDiffViewMode = 'split'"
            >
              Split
            </UButton>
          </UFieldGroup>
          <UButton
            v-if="!isInWindow"
            size="xs"
            variant="ghost"
            icon="i-lucide-external-link"
            @click="openMissingCommitsWindow"
          >
            Open in Window
          </UButton>
        </template>
      </CardHeader>
    </template>

    <CommitList
      :commits="missingCommits"
      variant="detailed"
      :show-file-count="true"
    >
      <template #after-commit="{ commit }">
        <!-- File diffs -->
        <div v-if="'fileDiffs' in commit && commit.fileDiffs && commit.fileDiffs.length > 0" class="mt-3 bg-default rounded-lg">
          <FileDiffList
            :file-diffs="commit.fileDiffs"
            :key-prefix="'hash' in commit && commit.hash ? commit.hash : ''"
            :hide-controls="true"
            :diff-view-mode="store.conflictDiffViewMode"
          />
        </div>
      </template>
    </CommitList>

    <template #footer>
      <InfoCard
        title="Resolution Steps"
        icon="i-lucide-lightbulb"
        icon-class="text-warning"
      >
        <p class="mb-2">
          To resolve this conflict, you can:
        </p>
        <ul class="space-y-2 text-default">
          <li class="flex items-start gap-2">
            <span class="text-muted mt-0.5">•</span>
            <span>Include the missing commits in this branch by changing their prefix to match.</span>
          </li>
          <li class="flex items-start gap-2">
            <span class="text-muted mt-0.5">•</span>
            <span>Move the conflicting changes to a different commit or branch.</span>
          </li>
          <li class="flex items-start gap-2">
            <span class="text-muted mt-0.5">•</span>
            <span>Exclude this commit from the branch by changing its prefix.</span>
          </li>
        </ul>
      </InfoCard>
    </template>
  </UCard>
</template>

<script lang="ts" setup>
import type { MissingCommit, MergeConflictInfo } from "~/utils/bindings"
import { useConflictViewerStore } from "~/stores/conflictViewer"

const props = defineProps<{
  missingCommits: MissingCommit[]
  conflict?: MergeConflictInfo
  branchName?: string
  isInWindow?: boolean
}>()

// Get conflict viewer store for diff view mode
const store = useConflictViewerStore()

// Check if any commits have file diffs
const hasAnyFileDiffs = computed(() => {
  return props.missingCommits.some(commit => commit.fileDiffs.length > 0)
})

// Open missing commits window directly
async function openMissingCommitsWindow() {
  if (!props.conflict) {
    return
  }

  // Prepare the data
  const data = {
    conflictCommitHash: props.conflict.commitHash,
    conflictCommitMessage: props.conflict.commitMessage,
    conflictCommitAuthorTime: props.conflict.commitAuthorTime,
    conflictCommitCommitterTime: props.conflict.commitCommitterTime,
    branchName: props.branchName || "Unknown",
    missingCommits: props.conflict.conflictAnalysis?.missingCommits || [],
    mergeBase: props.conflict.conflictAnalysis
      ? {
          hash: props.conflict.conflictAnalysis.mergeBaseHash,
          message: props.conflict.conflictAnalysis.mergeBaseMessage,
          author: props.conflict.conflictAnalysis.mergeBaseAuthor,
          time: props.conflict.conflictAnalysis.mergeBaseTime,
        }
      : undefined,
    divergenceSummary: props.conflict.conflictAnalysis?.divergenceSummary || {
      commitsAheadInSource: 0,
      commitsAheadInTarget: 0,
    },
  }

  await openSubWindow({
    windowId: "missing-commits",
    url: "/missing-commits",
    title: `Missing Commits Analysis - ${props.branchName || "Unknown Branch"}`,
    data,
  })
}
</script>
