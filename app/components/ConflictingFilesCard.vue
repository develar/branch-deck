<template>
  <UCard class="overflow-hidden">
    <template #header>
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <h2 class="text-sm font-medium text-highlighted">
            Conflicting Files
          </h2>
          <UBadge color="warning" variant="subtle" size="sm">
            {{ conflicts.length }} {{ conflicts.length === 1 ? 'file' : 'files' }}
          </UBadge>
        </div>
        <UButton
          v-if="!isInWindow"
          size="xs"
          variant="ghost"
          icon="i-lucide-external-link"
          @click="openConflictingFilesWindow"
        >
          Open in Window
        </UButton>
      </div>
    </template>

    <ConflictingFilesSection
      :conflicts="conflicts"
      :conflict-info="conflictInfo"
      :conflict-marker-commits="conflictMarkerCommits"
    />

    <template #footer>
      <ConflictExplanationAlert />
    </template>
  </UCard>
</template>

<script lang="ts" setup>
import type { ConflictDetail, MergeConflictInfo, ConflictMarkerCommitInfo } from '~/utils/bindings'
import { openSubWindow } from '~/utils/window-management'
import ConflictingFilesSection from './ConflictingFilesSection.vue'
import ConflictExplanationAlert from './ConflictExplanationAlert.vue'

const props = defineProps<{
  conflicts: ConflictDetail[]
  conflictInfo: MergeConflictInfo
  conflictMarkerCommits: Record<string, ConflictMarkerCommitInfo>
  branchName?: string
  isInWindow?: boolean
}>()

async function openConflictingFilesWindow() {
  // Get current settings from ConflictingFilesSection if needed
  const data = {
    conflict: props.conflictInfo,
    branchName: props.branchName || 'Unknown',
    showConflictsOnly: false,
    viewMode: 'diff',
    conflictDiffViewMode: 'unified' as const
  }
  
  await openSubWindow({
    windowId: 'conflicting-files',
    url: '/conflicting-files',
    title: `Conflicting Files - ${props.branchName || 'Unknown Branch'}`,
    data
  })
}
</script>