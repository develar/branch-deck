<template>
  <div class="space-y-3">
    <UTabs
      v-model="store.viewMode"
      :items="tabItems"
      variant="link"
      size="sm"
      color="neutral"
    >
      <template #diff>
        <div class="space-y-3 mt-4">
          <!-- Controls for diff view -->
          <div class="flex justify-between items-center px-1">
            <USwitch
              v-model="store.showConflictsOnly"
              size="sm"
              label="Conflicts only"
            />
            <UFieldGroup size="xs">
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
          </div>

          <!-- Diff content -->
          <FileDiffList
            :file-diffs="store.showConflictsOnly ? conflictingFileDiffsFiltered : conflictingFileDiffs"
            key-prefix="conflict"
            :conflict-marker-commits="conflictMarkerCommits"
            :hide-controls="true"
            :diff-view-mode="store.conflictDiffViewMode"
          />
        </div>
      </template>

      <template #3way>
        <div class="mt-4">
          <GitDiffMergeView :conflicts="conflicts" :conflict-info="conflictInfo" />
        </div>
      </template>
    </UTabs>
  </div>
</template>

<script lang="ts" setup>
import type { ConflictDetail, MergeConflictInfo } from "~/utils/bindings"
import { useConflictViewerStore } from "~/stores/conflictViewer"

const props = defineProps<{
  conflicts: ConflictDetail[]
  conflictInfo?: MergeConflictInfo
  conflictMarkerCommits?: Record<string, {
    hash: string
    message: string
    author: string
    authorTime: number
    committerTime: number
  }>
}>()

// Get conflict viewer settings from store
const store = useConflictViewerStore()

// Tab items for the UTabs component
const tabItems = [
  {
    value: "diff",
    slot: "diff",
    label: "Diff View",
    icon: "i-lucide-align-left",
  },
  {
    value: "3way",
    slot: "3way",
    label: "3-way Merge",
    icon: "i-lucide-columns-3",
  },
]

// Convert conflicting files to FileDiff format for FileDiffList component
const conflictingFileDiffs = computed(() => {
  return props.conflicts.map(file => file.fileDiff)
})

// Filter file diffs to show only hunks with conflicts
const conflictingFileDiffsFiltered = computed(() => {
  return conflictingFileDiffs.value.map((fileDiff) => {
    // Check if any hunk contains conflict markers
    const hasConflictMarkers = (hunk: string) => {
      return hunk.includes("<<<<<<<")
        || hunk.includes("|||||||")
        || hunk.includes("=======")
        || hunk.includes(">>>>>>>")
    }

    // Filter hunks to only include those with conflict markers
    const filteredHunks = fileDiff.hunks.filter(hasConflictMarkers)

    // Return a new FileDiff with only conflict hunks
    return {
      ...fileDiff,
      hunks: filteredHunks,
    }
  }).filter(fileDiff => fileDiff.hunks.length > 0)
})
</script>
