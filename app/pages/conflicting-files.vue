<template>
  <UPage>
    <UPageBody class="p-6">
      <div v-if="conflictData" class="space-y-4">
        <!-- Conflict Overview -->
        <UCard class="bg-elevated">
          <div class="space-y-3">
            <!-- Header with icon and title -->
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-2">
                <UIcon name="i-lucide-git-merge" class="w-4 h-4 text-error" />
                <span class="text-sm font-medium text-highlighted">Cherry-pick Conflict</span>
              </div>
              <UBadge variant="subtle" size="sm">
                {{ conflictData.conflict.conflictingFiles.length }} {{ conflictData.conflict.conflictingFiles.length === 1 ? 'file' : 'files' }}
              </UBadge>
            </div>
            
            <!-- Conflict commit info -->
            <div class="pb-3 border-b border-default">
              <p class="text-sm text-toned">{{ conflictData.conflict.commitMessage }}</p>
              <div class="flex items-center gap-3 mt-1">
                <CommitInfo 
                  :hash="conflictData.conflict.commitHash"
                  :timestamp="conflictData.conflict.commitTime"
                />
                <span class="text-xs text-muted">•</span>
                <span class="text-xs text-muted">Branch: {{ conflictData.branchName }}</span>
              </div>
            </div>
          </div>
        </UCard>

        <!-- Alert with conflict info -->
        <ConflictExplanationAlert />

        <!-- Conflicting Files Section -->
        <UCard>
          <template #header>
            <div class="flex items-center gap-2">
              <UIcon name="i-lucide-git-merge" class="w-4 h-4 text-error" />
              <span class="text-sm font-medium text-highlighted">Merge Conflicts</span>
            </div>
          </template>

          <ConflictingFilesSection
            v-if="conflictData"
            :conflicts="conflictData.conflict.conflictingFiles"
            :conflict-info="conflictData.conflict"
            :conflict-marker-commits="conflictMarkerCommits"
          />
        </UCard>
      </div>

      <!-- Loading state -->
      <div v-else class="flex items-center justify-center min-h-[400px]">
        <div class="text-center">
          <UIcon name="i-lucide-loader-2" class="w-8 h-8 text-muted animate-spin mx-auto mb-3" />
          <p class="text-sm text-muted">Loading conflicting files data...</p>
        </div>
      </div>
    </UPageBody>
  </UPage>
</template>

<script lang="ts" setup>
import { ref, onMounted, computed } from 'vue'
import { listen, emit } from '@tauri-apps/api/event'
import type { MergeConflictInfo } from '~/utils/bindings'
import ConflictingFilesSection from '~/components/ConflictingFilesSection.vue'

// Disable layout for sub-window
definePageMeta({
  layout: false
})

interface ConflictingFilesData {
  conflict: MergeConflictInfo
  branchName: string
  showConflictsOnly: boolean
  viewMode: string
  conflictDiffViewMode: 'unified' | 'split'
}

const conflictData = ref<ConflictingFilesData | null>(null)


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

// Listen for initialization data
onMounted(async () => {
  // Try to restore data from sessionStorage (for hot reload)
  const stored = sessionStorage.getItem('conflicting-files-data')
  if (stored) {
    try {
      const data = JSON.parse(stored) as ConflictingFilesData
      conflictData.value = data
      return // Don't request new data if we have it
    } catch {
      // Invalid data, continue with normal flow
    }
  }

  // Set up listener and wait for data
  const unlisten = await listen<ConflictingFilesData>('init-conflicting-files-data', (event) => {
    conflictData.value = event.payload
    // Store data for hot reload
    sessionStorage.setItem('conflicting-files-data', JSON.stringify(event.payload))
    // Clean up after receiving data
    unlisten()
  })
  
  // Signal that we're ready to receive data
  await emit('conflicting-files-ready', {})
})
</script>