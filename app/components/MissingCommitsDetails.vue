<template>
  <UCard class="overflow-hidden">
    <template #header>
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <h2 class="text-sm font-medium text-highlighted">
            Missing Commits Details
          </h2>
          <UBadge color="warning" variant="subtle" size="sm">
            {{ missingCommits.length }} {{ missingCommits.length === 1 ? 'commit' : 'commits' }}
          </UBadge>
        </div>
        <div class="flex items-center gap-3">
          <!-- Global diff view toggle -->
          <UButtonGroup v-if="hasAnyFileDiffs" size="xs">
            <UButton
              icon="i-lucide-align-left"
              :color="diffViewMode === 'unified' ? 'primary' : 'neutral'"
              variant="outline"
              @click="diffViewMode = 'unified'"
            >
              Unified
            </UButton>
            <UButton
              icon="i-lucide-columns-2"
              :color="diffViewMode === 'split' ? 'primary' : 'neutral'"
              variant="outline"
              @click="diffViewMode = 'split'"
            >
              Split
            </UButton>
          </UButtonGroup>
          <UButton
            v-if="!isInWindow"
            size="xs"
            variant="ghost"
            icon="i-lucide-external-link"
            @click="openMissingCommitsWindow"
          >
            Open in Window
          </UButton>
        </div>
      </div>
    </template>

    <div class="space-y-4">
      <!-- Iterate through missing commits -->
      <div
        v-for="(commit) in missingCommits"
        :key="commit.hash"
        class="pb-4 border-b border-default last:border-0 last:pb-0"
      >
        <div class="space-y-2">
          <div>
            <p class="text-sm font-medium text-highlighted">{{ commit.message }}</p>
            <CommitInfo 
              :hash="commit.hash"
              :author="commit.author"
              :timestamp="commit.authorTime"
              :committer-timestamp="commit.committerTime"
              :file-count="commit.fileDiffs?.length"
              class="mt-1"
            />
          </div>

          <!-- File diffs -->
          <div v-if="commit.fileDiffs && commit.fileDiffs.length > 0" class="mt-3">
            <FileDiffList 
              :file-diffs="commit.fileDiffs" 
              :key-prefix="commit.hash" 
              :hide-controls="true"
              :diff-view-mode="diffViewMode"
            />
          </div>
        </div>
      </div>
    </div>

    <template #footer>
      <InfoCard
        title="Resolution Steps"
        icon="i-lucide-lightbulb"
        icon-class="text-warning"
      >
        <p class="mb-2">To resolve this conflict, you can:</p>
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
import { ref, computed } from 'vue'
import { openSubWindow } from '~/utils/window-management'

const props = defineProps<{
  missingCommits: MissingCommit[]
  conflict?: MergeConflictInfo
  branchName?: string
  isInWindow?: boolean
}>()

// Global diff view mode
const diffViewMode = ref<'unified' | 'split'>('unified')

// Check if any commits have file diffs
const hasAnyFileDiffs = computed(() => {
  return props.missingCommits.some(commit => commit.fileDiffs && commit.fileDiffs.length > 0)
})

// Open missing commits window directly
async function openMissingCommitsWindow() {
  if (!props.conflict) return
  
  // Prepare the data
  const data = {
    conflictCommitHash: props.conflict.commitHash,
    conflictCommitMessage: props.conflict.commitMessage,
    conflictCommitTime: props.conflict.commitTime,
    branchName: props.branchName || 'Unknown',
    missingCommits: props.conflict.conflictAnalysis?.missingCommits || [],
    mergeBase: props.conflict.conflictAnalysis ? {
      hash: props.conflict.conflictAnalysis.mergeBaseHash,
      message: props.conflict.conflictAnalysis.mergeBaseMessage,
      author: props.conflict.conflictAnalysis.mergeBaseAuthor,
      time: props.conflict.conflictAnalysis.mergeBaseTime,
    } : undefined,
    divergenceSummary: props.conflict.conflictAnalysis?.divergenceSummary || { 
      commitsAheadInSource: 0, 
      commitsAheadInTarget: 0 
    }
  }
  
  await openSubWindow({
    windowId: 'missing-commits',
    url: '/missing-commits',
    title: `Missing Commits Analysis - ${props.branchName || 'Unknown Branch'}`,
    data
  })
}
</script>