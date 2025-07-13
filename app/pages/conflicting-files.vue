<template>
  <div class="w-full px-4 sm:px-6 lg:px-8 py-4">
    <div v-if="conflictData" class="space-y-4">
      <!-- Commit Context -->
      <UCard class="bg-elevated">
        <template #header>
          <ConflictCommitContext
            title="Cannot apply commit"
            :commit-message="conflictData.conflict.commitMessage"
            :commit-hash="conflictData.conflict.commitHash"
            :commit-time="conflictData.conflict.commitTime"
            :branch-name="conflictData.branchName"
          />
        </template>
      </UCard>

      <!-- Conflicting Files Section (main content) -->
      <ConflictingFilesCard
        :conflicts="conflictData.conflict.conflictingFiles"
        :conflict-info="conflictData.conflict"
        :conflict-marker-commits="conflictMarkerCommits"
        :branch-name="conflictData.branchName"
        :is-in-window="true"
      />
    </div>

    <!-- Loading state -->
    <div v-else class="flex items-center justify-center min-h-[400px]">
      <div class="text-center">
        <UIcon name="i-lucide-loader-2" class="w-8 h-8 text-muted animate-spin mx-auto mb-3"/>
        <p class="text-sm text-muted">Loading conflicting files data...</p>
      </div>
    </div>
  </div>
</template>

<script lang="ts" setup>
import { computed } from "vue"
import type { MergeConflictInfo } from "~/utils/bindings"
import { useSubWindowData } from "~/composables/useSubWindowData"

// Disable layout for sub-window
definePageMeta({
  layout: false
})

interface ConflictingFilesData {
  conflict: MergeConflictInfo
  branchName: string
  showConflictsOnly: boolean
  viewMode: string
  conflictDiffViewMode: "unified" | "split"
}

const conflictData = useSubWindowData<ConflictingFilesData>()

// Get conflict marker commits from the conflict info
const conflictMarkerCommits = computed(() => {
  if (!conflictData.value || !conflictData.value.conflict.conflictMarkerCommits) {
    return {}
  }

  const commits = conflictData.value.conflict.conflictMarkerCommits
  const result: Record<string, { hash: string; message: string; author: string; timestamp: number }> = {}

  for (const [key, value] of Object.entries(commits)) {
    if (value) {
      result[key] = value
    }
  }

  return result
})
</script>