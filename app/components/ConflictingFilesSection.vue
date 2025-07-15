<template>
  <div class="space-y-3">
    <UTabs
      v-model="selectedTab"
      :items="tabItems"
      variant="link"
      size="sm"
      color="neutral"
    >
      <template #diff>
        <div class="space-y-3 mt-4">
          <!-- Controls for diff view -->
          <div class="flex justify-between items-center">
            <USwitch
              v-model="showConflictsOnly"
              size="sm"
              label="Conflicts only"
            />
            <UButtonGroup size="xs">
              <UButton
                icon="i-lucide-align-left"
                :color="conflictDiffViewMode === 'unified' ? 'primary' : 'neutral'"
                variant="outline"
                @click="conflictDiffViewMode = 'unified'"
              >
                Unified
              </UButton>
              <UButton
                icon="i-lucide-columns-2"
                :color="conflictDiffViewMode === 'split' ? 'primary' : 'neutral'"
                variant="outline"
                @click="conflictDiffViewMode = 'split'"
              >
                Split
              </UButton>
            </UButtonGroup>
          </div>

          <!-- Diff content -->
          <FileDiffList
            :file-diffs="showConflictsOnly ? conflictingFileDiffsFiltered : conflictingFileDiffs"
            key-prefix="conflict"
            :conflict-marker-commits="conflictMarkerCommits"
            :hide-controls="true"
            :diff-view-mode="conflictDiffViewMode"
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
import { computed } from "vue"
import type { ConflictDetail, MergeConflictInfo } from "~/utils/bindings"
import GitDiffMergeView from "./GitDiffMergeView.vue"
import { useConflictViewerSettings } from "~/composables/conflictViewerSettings"

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
const { showConflictsOnly, viewMode, conflictDiffViewMode } = useConflictViewerSettings()

// Use viewMode as selectedTab for UTabs
const selectedTab = computed({
  get: () => viewMode.value,
  set: (value) => { viewMode.value = value },
})

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

// Expose current state for parent components
defineExpose({
  showConflictsOnly,
  viewMode,
  conflictDiffViewMode,
})

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
