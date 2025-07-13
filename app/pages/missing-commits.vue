<template>
  <UPage>
    <UPageBody class="p-6">
      <div v-if="missingCommitsData" class="space-y-4">
        <!-- Conflict Overview -->
        <UCard class="bg-elevated">
          <div class="space-y-3">
            <!-- Header with icon and title -->
            <div class="flex items-center gap-2">
              <UIcon name="i-lucide-git-merge" class="w-4 h-4 text-error" />
              <span class="text-sm font-medium text-highlighted">Conflict Overview</span>
            </div>
            
            <!-- Conflict commit message -->
            <div class="pb-3 border-b border-default">
              <p class="text-sm text-toned">{{ missingCommitsData.conflictCommitMessage }}</p>
              <div class="flex items-center gap-3 mt-1">
                <CommitInfo 
                  :hash="missingCommitsData.conflictCommitHash"
                />
                <span class="text-xs text-muted">•</span>
                <span class="text-xs text-muted">Branch: {{ missingCommitsData.branchName }}</span>
              </div>
            </div>

            <!-- Common ancestor info -->
            <div v-if="missingCommitsData.mergeBase" class="space-y-2">
              <div>
                <p class="text-xs text-muted mb-1">Common ancestor:</p>
                <p class="text-xs text-toned">{{ missingCommitsData.mergeBase.message }}</p>
                <CommitInfo 
                  :hash="missingCommitsData.mergeBase.hash"
                  :author="missingCommitsData.mergeBase.author"
                  :timestamp="missingCommitsData.mergeBase.time"
                />
              </div>
              <p class="text-xs text-muted">
                {{ missingCommitsData.divergenceSummary.commitsAheadInSource }} commits behind source,
                {{ missingCommitsData.divergenceSummary.commitsAheadInTarget }} commits behind target
              </p>
            </div>
          </div>
        </UCard>

      <!-- Missing Commits Details -->
      <MissingCommitsDetails 
        v-if="missingCommitsData.missingCommits.length > 0"
        :missing-commits="missingCommitsData.missingCommits" 
        :is-in-window="true"
      />

      <!-- No missing commits message -->
      <UCard v-else>
        <div class="text-center py-8">
          <UIcon name="i-lucide-check-circle" class="w-12 h-12 text-success mx-auto mb-3" />
          <p class="text-sm text-muted">No missing commits found for this conflict.</p>
        </div>
      </UCard>
    </div>

      <!-- Loading state -->
      <div v-else class="flex items-center justify-center min-h-[400px]">
        <div class="text-center">
          <UIcon name="i-lucide-loader-2" class="w-8 h-8 text-muted animate-spin mx-auto mb-3" />
          <p class="text-sm text-muted">Loading missing commits data...</p>
        </div>
      </div>
    </UPageBody>
  </UPage>
</template>

<script lang="ts" setup>
import { ref, onMounted } from 'vue'
import { listen, emit } from '@tauri-apps/api/event'
import type { MissingCommit } from '~/utils/bindings'

// Disable layout for sub-window
definePageMeta({
  layout: false
})

interface MissingCommitsWindowData {
  conflictCommitHash: string
  conflictCommitMessage: string
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

const missingCommitsData = ref<MissingCommitsWindowData | null>(null)

// Listen for initialization data
onMounted(async () => {
  // Try to restore data from sessionStorage (for hot reload)
  const stored = sessionStorage.getItem('missing-commits-data')
  if (stored) {
    try {
      missingCommitsData.value = JSON.parse(stored)
      return // Don't request new data if we have it
    } catch {
      // Invalid data, continue with normal flow
    }
  }

  // Set up listener and wait for data
  const unlisten = await listen<MissingCommitsWindowData>('init-missing-commits-data', (event) => {
    missingCommitsData.value = event.payload
    // Store data for hot reload
    sessionStorage.setItem('missing-commits-data', JSON.stringify(event.payload))
    // Clean up after receiving data
    unlisten()
  })
  
  // Signal that we're ready to receive data
  await emit('missing-commits-ready', {})
})
</script>