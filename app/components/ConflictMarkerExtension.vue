<template>
  <div v-if="isConflictLine" :class="['border-l-4 bg-muted', conflictBorderClass]">
    <!-- Conflict marker badge with tooltip -->
    <div class="flex items-center gap-2 px-4 py-2">
      <div class="flex items-center gap-2">
        <!-- Icon based on conflict type -->
        <UIcon 
          :name="conflictIcon"
          :class="conflictIconClass"
          class="w-4 h-4"
        />
        
        <!-- Conflict description -->
        <span class="text-sm font-medium" :class="conflictTextClass">
          {{ conflictDescription }}
        </span>
        
        <!-- Commit/branch info if available -->
        <div v-if="data.conflictInfo" class="text-xs text-muted">
          <span v-if="data.conflictInfo.branch && !isCommitHash(data.conflictInfo.branch)">
            {{ data.conflictInfo.branch }}
          </span>
          <CommitHashPopover
            v-else-if="data.conflictInfo.branch && isCommitHash(data.conflictInfo.branch)"
            :hash="data.conflictInfo.branch"
            :message="getCommitInfo(data.conflictInfo.branch)?.message"
            :author="getCommitInfo(data.conflictInfo.branch)?.author"
            :timestamp="getCommitInfo(data.conflictInfo.branch)?.timestamp"
          />
          <CommitHashPopover
            v-else-if="data.conflictInfo.base && isCommitHash(data.conflictInfo.base)"
            :hash="data.conflictInfo.base"
            :message="getCommitInfo(data.conflictInfo.base)?.message"
            :author="getCommitInfo(data.conflictInfo.base)?.author"
            :timestamp="getCommitInfo(data.conflictInfo.base)?.timestamp"
          />
          <span v-else-if="data.conflictInfo.base && !isCommitHash(data.conflictInfo.base)">
            {{ data.conflictInfo.base }}
          </span>
          <CommitHashPopover
            v-else-if="data.conflictInfo.commit && isCommitHash(data.conflictInfo.commit)"
            :hash="data.conflictInfo.commit"
            :message="getCommitInfo(data.conflictInfo.commit)?.message"
            :author="getCommitInfo(data.conflictInfo.commit)?.author"
            :timestamp="getCommitInfo(data.conflictInfo.commit)?.timestamp"
          />
          <span v-else-if="data.conflictInfo.commit && !isCommitHash(data.conflictInfo.commit)">
            {{ data.conflictInfo.commit }}
          </span>
        </div>
      </div>
      
    </div>
  </div>
</template>

<script lang="ts" setup>
import { computed } from 'vue'

// Props from the extend slot
const props = defineProps<{
  lineNumber: number
  side: string | number
  data: {
    tooltip?: string
    conflictInfo?: {
      type: string
      branch?: string
      base?: string
      commit?: string
    }
    conflictMarkerCommits?: Record<string, {
      hash: string
      message: string
      author: string
      timestamp: number
    }>
  }
  diffFile: unknown
}>()


// Check if this line is a conflict marker
const isConflictLine = computed(() => {
  return props.data.conflictInfo && props.data.conflictInfo.type
})

// Get icon based on conflict type
const conflictIcon = computed(() => {
  const type = props.data.conflictInfo?.type
  switch (type) {
    case 'conflict-start':
      return 'i-lucide-git-branch'
    case 'conflict-base':
      return 'i-lucide-git-commit'
    case 'conflict-separator':
      return 'i-lucide-split'
    case 'conflict-end':
      return 'i-lucide-git-merge'
    default:
      return 'i-lucide-alert-circle'
  }
})

// Get icon color class
const conflictIconClass = computed(() => {
  const type = props.data.conflictInfo?.type
  switch (type) {
    case 'conflict-start':
      return 'text-red-500'
    case 'conflict-base':
      return 'text-muted'
    case 'conflict-separator':
      return 'text-yellow-500'
    case 'conflict-end':
      return 'text-green-500'
    default:
      return 'text-muted'
  }
})

// Get text color class
const conflictTextClass = computed(() => {
  const type = props.data.conflictInfo?.type
  switch (type) {
    case 'conflict-start':
      return 'text-red-500'
    case 'conflict-base':
      return 'text-muted'
    case 'conflict-separator':
      return 'text-yellow-500'
    case 'conflict-end':
      return 'text-green-500'
    default:
      return ''
  }
})

// Get border color class
const conflictBorderClass = computed(() => {
  const type = props.data.conflictInfo?.type
  switch (type) {
    case 'conflict-start':
      return 'border-red-500'
    case 'conflict-base':
      return 'border-gray-500'
    case 'conflict-separator':
      return 'border-yellow-500'
    case 'conflict-end':
      return 'border-green-500'
    default:
      return 'border-gray-500'
  }
})

// Get description text
const conflictDescription = computed(() => {
  const type = props.data.conflictInfo?.type
  switch (type) {
    case 'conflict-start':
      return 'Current changes (HEAD)'
    case 'conflict-base':
      return 'Common ancestor'
    case 'conflict-separator':
      return 'Conflict separator'
    case 'conflict-end':
      return 'Incoming changes'
    default:
      return 'Conflict marker'
  }
})

// Check if a string looks like a commit hash (SHA-1)
function isCommitHash(str: string): boolean {
  // Git commit hashes are 40 characters long (full) or commonly abbreviated to 7-8 characters
  // They contain only hexadecimal characters (0-9, a-f)
  return /^[0-9a-f]{7,40}$/i.test(str)
}

// Get commit info from the conflict marker commits map
function getCommitInfo(hash: string) {
  return props.data.conflictMarkerCommits?.[hash]
}
</script>