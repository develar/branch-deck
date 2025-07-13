<template>
  <div class="space-y-4">
    <!-- Help section for 3-way merge view -->
    <UAlert icon="i-lucide-info" color="neutral" variant="subtle">
      <template #description>
        <div class="space-y-2 text-sm">
          <p class="font-medium">Understanding 3-way merge conflicts</p>
          <p>This view shows how both branches evolved from their common ancestor, helping you understand why the conflict occurred:</p>
          <ul class="list-disc list-inside space-y-1 text-muted">
            <li><strong>Base (Common Ancestor)</strong>: The original content before the branches diverged</li>
            <li><strong>Target Branch</strong>: How the file changed in your current branch</li>
            <li><strong>Cherry-pick</strong>: How the file changed in the commit you're trying to apply</li>
          </ul>
          <p class="text-xs text-muted mt-2">The conflict occurs because both branches modified the same parts of the file differently.</p>
        </div>
      </template>
    </UAlert>

    <div v-if="conflicts.length > 0" class="space-y-3">
      <div 
        v-for="(conflict, index) in conflicts" 
        :key="index"
        class="border border-default rounded-lg overflow-hidden"
      >
        <CollapsibleFileHeader
          :file-name="conflict.file"
          :expanded="expandedFiles[index]"
          @toggle="toggleFile(index)"
        >
          <template #actions>
            <UButton
              :icon="copiedFiles.has(conflict.file) ? 'i-lucide-copy-check' : 'i-lucide-copy'"
              size="xs"
              variant="ghost"
              :class="[
                'transition-all',
                copiedFiles.has(conflict.file) ? 'opacity-100 text-success' : 'opacity-0 group-hover:opacity-100'
              ]"
              @click.stop="copyToClipboard(conflict.file)"
            />
          </template>
        </CollapsibleFileHeader>
        
        <div v-if="expandedFiles[index]" class="border-t border-default p-4">
          <SplitterGroup 
            direction="horizontal"
            :auto-save-id="`conflict-${index}`"
            class="h-[600px] w-full"
          >
            <!-- Base Version -->
            <SplitterPanel
              :id="`${index}-base`"
              :collapsible="true"
              :collapsed="collapsedViews[`${index}-base`]"
              :collapsed-size="0"
              :min-size="15"
              :default-size="33"
              @collapse="() => collapsedViews[`${index}-base`] = true"
              @expand="() => collapsedViews[`${index}-base`] = false"
            >
              <div class="h-full flex flex-col">
                <div class="flex items-center justify-between bg-subtle rounded-lg px-3 py-2 mb-2">
                  <div class="flex-1 min-w-0">
                    <h4 class="font-medium text-sm text-highlighted flex items-center gap-2">
                      <UIcon name="i-lucide-git-merge" class="w-4 h-4 flex-shrink-0" />
                      Base (Common Ancestor)
                    </h4>
                    <p class="text-xs text-muted mt-1">
                      File state at 
                      <CommitHashPopover 
                        v-if="mergeBaseInfo"
                        :hash="mergeBaseInfo.hash"
                        :message="mergeBaseInfo.message"
                        :author="mergeBaseInfo.author"
                        :timestamp="mergeBaseInfo.timestamp"
                      />
                      <span v-else>merge base</span>
                    </p>
                  </div>
                </div>
                <div class="flex-1 border border-default rounded-lg overflow-hidden">
                  <DiffView
                    :data="getBaseDiffData(conflict)"
                    :diff-view-mode="DiffModeEnum.Unified"
                    :diff-view-wrap="true"
                    :diff-view-highlight="true"
                    :diff-view-font-size="12"
                    :diff-view-theme="colorMode.preference === 'dark' ? 'dark' : 'light'"
                  />
                </div>
              </div>
            </SplitterPanel>

            <SplitterResizeHandle 
              class="mx-1 w-1 bg-border hover:bg-primary transition-colors cursor-col-resize"
            />

            <!-- Our Version (Target Branch) -->
            <SplitterPanel
              :id="`${index}-target`"
              :min-size="20"
              :default-size="33"
            >
              <div class="h-full flex flex-col">
                <div class="flex items-center justify-between bg-subtle rounded-lg px-3 py-2 mb-2">
                  <div class="flex-1 min-w-0">
                    <h4 class="font-medium text-sm text-highlighted flex items-center gap-2">
                      <UIcon name="i-lucide-git-branch" class="w-4 h-4 flex-shrink-0" />
                      Target Branch (Current)
                    </h4>
                    <p class="text-xs text-muted mt-1">
                      Changes from base to current HEAD
                      <CommitHashPopover 
                        v-if="targetInfo"
                        :hash="targetInfo.hash"
                        :message="targetInfo.message"
                        :author="targetInfo.author"
                        :timestamp="targetInfo.timestamp"
                      />
                    </p>
                  </div>
                </div>
                <div class="flex-1 border border-default rounded-lg overflow-hidden">
                  <DiffView
                    :data="getOursDiffData(conflict)"
                    :diff-view-mode="DiffModeEnum.Unified"
                    :diff-view-wrap="true"
                    :diff-view-highlight="true"
                    :diff-view-font-size="12"
                    :diff-view-theme="colorMode.preference === 'dark' ? 'dark' : 'light'"
                  />
                </div>
              </div>
            </SplitterPanel>

            <SplitterResizeHandle 
              class="mx-1 w-1 bg-border hover:bg-primary transition-colors cursor-col-resize"
            />

            <!-- Their Version (Cherry-pick) -->
            <SplitterPanel
              :id="`${index}-cherry`"
              :min-size="20"
              :default-size="34"
            >
              <div class="h-full flex flex-col">
                <div class="flex items-center justify-between bg-subtle rounded-lg px-3 py-2 mb-2">
                  <div class="flex-1 min-w-0">
                    <h4 class="font-medium text-sm text-highlighted flex items-center gap-2">
                      <UIcon name="i-lucide-git-pull-request" class="w-4 h-4 flex-shrink-0" />
                      Cherry-pick (Incoming)
                    </h4>
                    <p class="text-xs text-muted mt-1">
                      Changes from base to 
                      <CommitHashPopover 
                        v-if="cherryInfo"
                        :hash="cherryInfo.hash"
                        :message="cherryInfo.message"
                        :author="cherryInfo.author"
                        :timestamp="cherryInfo.timestamp"
                      />
                    </p>
                  </div>
                </div>
                <div class="flex-1 border border-default rounded-lg overflow-hidden">
                  <DiffView
                    :data="getTheirsDiffData(conflict)"
                    :diff-view-mode="DiffModeEnum.Unified"
                    :diff-view-wrap="true"
                    :diff-view-highlight="true"
                    :diff-view-font-size="12"
                    :diff-view-theme="colorMode.preference === 'dark' ? 'dark' : 'light'"
                  />
                </div>
              </div>
            </SplitterPanel>
          </SplitterGroup>
        </div>
      </div>
    </div>
    
    <div v-else class="text-center py-8">
      <UIcon name="i-lucide-git-merge" class="w-8 h-8 text-muted mx-auto mb-2" />
      <p class="text-sm text-muted">No conflicts to display</p>
    </div>
  </div>
</template>

<script lang="ts" setup>
import { DiffView, DiffModeEnum } from '@git-diff-view/vue'
import { SplitterGroup, SplitterPanel, SplitterResizeHandle } from 'reka-ui'
import type { ConflictDetail, MergeConflictInfo } from '~/utils/bindings'
import CommitHashPopover from './CommitHashPopover.vue'
import CollapsibleFileHeader from './CollapsibleFileHeader.vue'
import { ref, onMounted } from 'vue'

const props = defineProps<{
  conflicts: ConflictDetail[]
  conflictInfo?: MergeConflictInfo
}>()

// Collapsed state for each diff view
const collapsedViews = ref<Record<string, boolean>>({})

// Collapsed state for each file
const expandedFiles = ref<Record<number, boolean>>({})

// Track copied files
const copiedFiles = ref(new Set<string>())

// Initialize base views as collapsed
onMounted(() => {
  props.conflicts.forEach((_, index) => {
    collapsedViews.value[`${index}-base`] = true
    // By default, expand first file only
    expandedFiles.value[index] = index === 0
  })
})

// Toggle file expansion
function toggleFile(index: number) {
  expandedFiles.value[index] = !expandedFiles.value[index]
}

// Copy file path to clipboard
async function copyToClipboard(fileName: string) {
  try {
    await navigator.clipboard.writeText(fileName)
    copiedFiles.value.add(fileName)
    
    // Reset the copied state after 2 seconds
    setTimeout(() => {
      copiedFiles.value.delete(fileName)
    }, 2000)
  } catch (err) {
    console.error('Failed to copy to clipboard:', err)
  }
}

// Extract commit info from the parent component's conflict info
const mergeBaseInfo = computed(() => {
  if (props.conflictInfo?.conflictAnalysis?.mergeBaseHash) {
    return {
      hash: props.conflictInfo.conflictAnalysis.mergeBaseHash,
      message: props.conflictInfo.conflictAnalysis.mergeBaseMessage,
      author: props.conflictInfo.conflictAnalysis.mergeBaseAuthor,
      timestamp: props.conflictInfo.conflictAnalysis.mergeBaseTime
    }
  }
  return null
})

const targetInfo = computed(() => {
  if (props.conflictInfo) {
    return {
      hash: props.conflictInfo.targetBranchHash,
      message: props.conflictInfo.targetBranchMessage,
      author: '', // Not provided in current structure
      timestamp: props.conflictInfo.targetBranchTime
    }
  }
  return null
})

const cherryInfo = computed(() => {
  if (props.conflictInfo) {
    return {
      hash: props.conflictInfo.commitHash,
      message: props.conflictInfo.commitMessage,
      author: '', // Not provided in current structure
      timestamp: props.conflictInfo.commitTime
    }
  }
  return null
})

const colorMode = useColorMode()

// Get actual file content for 3-way merge
function getFileContent(conflict: ConflictDetail) {
  const fileName = conflict.file
  const fileExt = fileName.split('.').pop() || 'txt'
  
  // Use actual file content from backend
  const baseContent = conflict.baseFile?.content || ''
  const oursContent = conflict.targetFile?.content || ''
  const theirsContent = conflict.cherryFile?.content || ''

  return {
    base: baseContent,
    ours: oursContent,
    theirs: theirsContent,
    fileName,
    fileExt
  }
}

// Generate diff data for base version (show as context-only)
function getBaseDiffData(conflict: ConflictDetail) {
  // For base view, show the content as unchanged context
  const fileData = getFileContent(conflict)
  
  const result = {
    oldFile: {
      fileName: fileData.fileName,
      fileLang: fileData.fileExt,
      content: fileData.base
    },
    newFile: {
      fileName: fileData.fileName,
      fileLang: fileData.fileExt,
      content: fileData.base
    },
    hunks: [] as string[]
  }
  
  // Generate a context-only hunk if we have content
  if (fileData.base) {
    const lines = fileData.base.split('\n')
    const lineCount = lines.length
    
    // Create proper diff headers and hunk with all lines as context
    let hunk = `--- a/${fileData.fileName}\n+++ b/${fileData.fileName}\n@@ -1,${lineCount} +1,${lineCount} @@`
    for (const line of lines) {
      hunk += `\n ${line}` // Space prefix = context line
    }
    
    result.hunks = [hunk]
  }
  
  return result
}

// Generate diff data for our version (base -> ours)
function getOursDiffData(conflict: ConflictDetail) {
  // Backend always provides this data
  return conflict.baseToTargetDiff
}

// Generate diff data for their version (base -> theirs)
function getTheirsDiffData(conflict: ConflictDetail) {
  // Backend always provides this data
  return conflict.baseToCherryDiff
}
</script>