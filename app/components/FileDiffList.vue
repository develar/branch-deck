<template>
  <div class="space-y-2">
    <!-- View mode toggle -->
    <div v-if="fileDiffs.length > 0 && !hideControls" class="flex justify-end">
      <UButtonGroup size="xs">
        <UButton
          icon="i-lucide-align-left"
          :color="currentDiffMode === 'unified' ? 'primary' : 'neutral'"
          variant="outline"
          @click="internalDiffMode = 'unified'"
        >
          Unified
        </UButton>
        <UButton
          icon="i-lucide-columns-2"
          :color="currentDiffMode === 'split' ? 'primary' : 'neutral'"
          variant="outline"
          @click="internalDiffMode = 'split'"
        >
          Split
        </UButton>
      </UButtonGroup>
    </div>

    <div
      v-for="(diff, idx) in fileDiffs"
      :key="`${keyPrefix}-${idx}`"
      class="border border-default rounded-lg"
    >
      <CollapsibleFileHeader
        :file-name="diff.newFile.fileName"
        :expanded="expandedDiffs[idx]"
        @toggle="toggleDiff(idx)"
      >
        <template #actions>
          <UButton
            :icon="copiedFiles.has(diff.newFile.fileName) ? 'i-lucide-copy-check' : 'i-lucide-copy'"
            size="xs"
            variant="ghost"
            :class="[
              'transition-all',
              copiedFiles.has(diff.newFile.fileName) ? 'opacity-100 text-success' : 'opacity-0 group-hover:opacity-100'
            ]"
            @click.stop="copyToClipboard(diff.newFile.fileName)"
          />
        </template>
      </CollapsibleFileHeader>
      <div v-if="expandedDiffs[idx]" class="border-t border-default">
        <DiffView
          :data="diff"
          :diff-view-mode="currentDiffMode === 'unified' ? DiffModeEnum.Unified : DiffModeEnum.Split"
          :diff-view-wrap="true"
          :diff-view-highlight="true"
          :diff-view-font-size="12"
          :diff-view-theme="colorMode.preference === 'dark' ? 'dark' : 'light'"
          :extend-data="getConflictExtendData(diff)"
          @line-click="handleConflictLineClick"
        >
          <template #extend="{ lineNumber, side, data, diffFile, onUpdate }">
            <ConflictMarkerExtension 
              :line-number="lineNumber"
              :side="side" 
              :data="data"
              :diff-file="diffFile"
              @update="onUpdate"
              @conflict-action="handleConflictAction"
            />
          </template>
        </DiffView>
      </div>
    </div>
  </div>
</template>

<script lang="ts" setup>
import { reactive, ref, computed } from "vue"
import { DiffView, DiffModeEnum } from "@git-diff-view/vue"
import "@git-diff-view/vue/styles/diff-view-pure.css"
import type { FileDiff } from "~/utils/bindings"
import CollapsibleFileHeader from './CollapsibleFileHeader.vue'

// Props
const props = defineProps<{
  fileDiffs: Array<FileDiff>
  keyPrefix: string
  conflictMarkerCommits?: Record<string, {
    hash: string
    message: string
    author: string
    timestamp: number
  }>
  hideControls?: boolean
  diffViewMode?: 'unified' | 'split'
}>()

// State to manage expanded diffs
const expandedDiffs = reactive<Record<number, boolean>>({})

// Track copied file names
const copiedFiles = ref<Set<string>>(new Set())

// Diff view mode (unified or split) - use prop value if provided, otherwise use internal state
const internalDiffMode = ref<'unified' | 'split'>('unified')
const currentDiffMode = computed(() => props.diffViewMode || internalDiffMode.value)


// Color mode for theme
const colorMode = useColorMode()

function toggleDiff(idx: number) {
  expandedDiffs[idx] = !expandedDiffs[idx]
}

async function copyToClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text)
    copiedFiles.value.add(text)
    
    // Remove the file from copied set after 2 seconds
    setTimeout(() => {
      copiedFiles.value.delete(text)
    }, 2000)
  } catch (err) {
    console.error('Failed to copy to clipboard:', err)
  }
}

// Extract commit hashes from conflict markers for tooltips
function extractConflictInfo(line: string) {
  if (line.includes('<<<<<<< ')) {
    const branch = line.replace('<<<<<<< ', '').trim()
    return { type: 'conflict-start', branch, tooltip: `Current branch: ${branch}` }
  }
  if (line.includes('||||||| ')) {
    const base = line.replace('||||||| ', '').trim()
    return { type: 'conflict-base', base, tooltip: `Common ancestor: ${base}` }
  }
  if (line.includes('=======')) {
    return { type: 'conflict-separator', tooltip: 'Conflict separator' }
  }
  if (line.includes('>>>>>>> ')) {
    const commit = line.replace('>>>>>>> ', '').trim()
    return { type: 'conflict-end', commit, tooltip: `Incoming commit: ${commit}` }
  }
  return null
}

// Types for git-diff-view extend data
interface ConflictLineData {
  tooltip: string
  conflictInfo?: ReturnType<typeof extractConflictInfo>
  conflictMarkerCommits?: Record<string, {
    hash: string
    message: string
    author: string
    timestamp: number
  }>
}


// Generate extendData for conflict marker annotations
function getConflictExtendData(diff: FileDiff) {
  const extendData: {
    oldFile: Record<number, { data: ConflictLineData }>
    newFile: Record<number, { data: ConflictLineData }>
  } = {
    oldFile: {},
    newFile: {}
  }

  // Parse hunks to find conflict markers in the diff content
  diff.hunks.forEach((hunk) => {
    const lines = hunk.split('\n')
    let newFileLineNumber = 0
    let inHunk = false
    
    lines.forEach((line) => {
      // Parse hunk header to get starting line numbers
      if (line.startsWith('@@')) {
        const match = line.match(/@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@/)
        if (match && match[1]) {
          newFileLineNumber = parseInt(match[1], 10)
          inHunk = true
        }
        return
      }
      
      if (!inHunk) return
      
      // Look for addition lines that contain conflict markers
      if (line.startsWith('+') && !line.startsWith('+++')) {
        const content = line.substring(1) // Remove the '+' prefix
        const conflictInfo = extractConflictInfo(content)
        
        if (conflictInfo) {
          extendData.newFile[newFileLineNumber] = {
            data: {
              tooltip: conflictInfo.tooltip,
              conflictInfo,
              conflictMarkerCommits: props.conflictMarkerCommits
            }
          }
        }
        newFileLineNumber++
      } else if (line.startsWith('-') && !line.startsWith('---')) {
        // Deletion line, don't increment new file line number
      } else if (!line.startsWith('\\')) {
        // Context line
        newFileLineNumber++
      }
    })
  })

  return extendData
}


// Handle line clicks on conflict markers
function handleConflictLineClick(event: { lineNumber: number; lineContent: string }) {
  const { lineContent } = event
  const conflictInfo = extractConflictInfo(lineContent)
  
  if (conflictInfo) {
    // You could emit an event here to show a resolution dialog
    // emit('conflict-action', { action: 'show-resolution', conflictInfo })
  }
}

// Handle conflict actions from the extension component
function handleConflictAction(payload: { action: string; lineNumber: number; side: string | number }) {
  // Implementation would depend on your conflict resolution workflow
  // This could emit events to parent components or call APIs
  switch (payload.action) {
    case 'accept-current':
      // Accept current version at line
      break
    case 'accept-incoming':
      // Accept incoming version at line
      break
    case 'accept-both':
      // Accept both versions at line
      break
    case 'edit-manually':
      // Open manual editor at line
      break
  }
}
</script>

