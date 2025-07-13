<template>
  <div class="space-y-4">
    <!-- Conflict Overview and Missing Commits Card -->
    <UCard class="overflow-hidden">
      <template #header>
        <div class="flex items-start gap-3">
          <UIcon name="i-lucide-git-merge" class="w-5 h-5 text-error mt-0.5 flex-shrink-0"/>
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-highlighted mb-1">
              Cherry-pick conflict detected
            </p>
            <p class="text-sm text-toned">
              Cannot apply: {{ conflict.commitMessage }}
            </p>
            <CommitInfo :hash="conflict.commitHash" :timestamp="conflict.commitTime" class="mt-1" />
          </div>
        </div>
      </template>

      <!-- Missing Commits Info -->
      <div v-if="conflict.conflictAnalysis" class="bg-elevated rounded-lg p-3 border border-warning">
        <div class="flex items-center gap-2">
          <UIcon name="i-lucide-file-diff" class="w-4 h-4 text-warning"/>
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
            :timestamp="conflict.conflictAnalysis.mergeBaseTime"
          />
          <span class="text-muted">
              ({{ conflict.conflictAnalysis.divergenceSummary.commitsAheadInSource }} commits behind source,
              {{ conflict.conflictAnalysis.divergenceSummary.commitsAheadInTarget }} commits behind target)
            </span>
        </p>
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
    <UCard class="overflow-hidden">
      <template #header>
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-3">
            <h2 class="text-sm font-medium text-highlighted">
              Conflicting Files
            </h2>
            <UBadge color="warning" variant="subtle" size="sm">
              {{ conflict.conflictingFiles.length }} {{ conflict.conflictingFiles.length === 1 ? "file" : "files" }}
            </UBadge>
          </div>
          <UButton
            size="xs"
            variant="ghost"
            icon="i-lucide-external-link"
            @click="openConflictingFilesWindow"
          >
            Open in Window
          </UButton>
        </div>
      </template>

      <ConflictExplanationAlert />

      <ConflictingFilesSection
        ref="conflictingFilesSection"
        :conflicts="conflict.conflictingFiles"
        :conflict-info="conflict"
        :conflict-marker-commits="conflictMarkerCommits"
      />
    </UCard>
  </div>
</template>

<script lang="ts" setup>
import type { MergeConflictInfo } from "~/utils/bindings"
import { ref, computed } from "vue"
import { openSubWindow } from '~/utils/window-management'
import ConflictingFilesSection from './ConflictingFilesSection.vue'

const props = defineProps<{
  conflict: MergeConflictInfo
  branchName?: string
}>()

// Ref to the conflicting files section component
const conflictingFilesSection = ref<InstanceType<typeof ConflictingFilesSection>>()

// Get conflict marker commits from the conflict info
const conflictMarkerCommits = computed(() => {
  if (props.conflict.conflictMarkerCommits) {
    // Convert Partial to Record by filtering out undefined values
    const commits = props.conflict.conflictMarkerCommits
    const result: Record<string, { hash: string; message: string; author: string; timestamp: number }> = {}
    
    for (const [key, value] of Object.entries(commits)) {
      if (value) {
        result[key] = value
      }
    }
    
    return result
  }
  return {}
})

// Open conflicting files window
async function openConflictingFilesWindow() {
  const data = {
    conflict: props.conflict,
    branchName: props.branchName || 'Unknown',
    showConflictsOnly: conflictingFilesSection.value?.showConflictsOnly || true,
    viewMode: conflictingFilesSection.value?.viewMode || 'diff',
    conflictDiffViewMode: conflictingFilesSection.value?.conflictDiffViewMode || 'unified'
  }
  
  await openSubWindow({
    windowId: 'conflicting-files',
    url: '/conflicting-files',
    title: `Conflicting Files - ${props.branchName || 'Unknown Branch'}`,
    data
  })
}
</script>
