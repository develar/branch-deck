<template>
  <div class="space-y-2">
    <AccordionRoot
      type="multiple"
      class="space-y-2"
      @update:model-value="onAccordionChange"
    >
      <AccordionItem
        v-for="(file, idx) in files"
        :key="`${keyPrefix}-${idx}`"
        :value="`item-${idx}`"
        class="border border-default rounded-lg overflow-hidden"
      >
        <CollapsibleFileHeader
          :file-name="file.filePath"
        >
          <template #actions>
            <div class="flex items-center gap-1">
              <!--<UBadge-->
              <!--  v-if="file.staged"-->
              <!--  color="success"-->
              <!--  variant="soft"-->
              <!--  size="xs">staged</UBadge>-->
              <!--<UBadge-->
              <!--  v-if="file.unstaged"-->
              <!--  color="neutral"-->
              <!--  variant="soft"-->
              <!--  size="xs">unstaged</UBadge>-->
              <UBadge :color="getStatusColor(file.status)" variant="soft" size="xs">
                {{ file.status }}
              </UBadge>
            </div>
          </template>
        </CollapsibleFileHeader>
        <AccordionContent class="border-t border-default">
          <!-- Loading state when fetching content -->
          <div v-if="loadingFiles.has(file.filePath)" class="flex items-center justify-center py-8">
            <div class="flex items-center space-x-3">
              <UIcon name="i-lucide-loader-2" class="size-4 animate-spin text-muted" />
              <span class="text-sm text-muted">Loading file content...</span>
            </div>
          </div>

          <!-- Error state if content loading failed -->
          <div v-else-if="fileErrors.has(file.filePath)" class="p-4">
            <div class="rounded-md border border-error/20 bg-error/10 p-4">
              <div class="flex items-center space-x-2">
                <UIcon name="i-lucide-alert-circle" class="size-4 text-error" />
                <span class="text-sm text-error">{{ fileErrors.get(file.filePath) }}</span>
              </div>
            </div>
          </div>

          <!-- Diff view when content is loaded -->
          <DiffView
            v-else-if="hasLoadedDiff(file.filePath)"
            :data="loadedDiffs.get(file.filePath)!"
            :diff-view-mode="currentDiffMode === 'unified' ? DiffModeEnum.Unified : DiffModeEnum.Split"
            :diff-view-wrap="true"
            :diff-view-highlight="true"
            :diff-view-font-size="12"
            :diff-view-theme="colorMode.preference === 'dark' ? 'dark' : 'light'"
          />
        </AccordionContent>
      </AccordionItem>
    </AccordionRoot>
  </div>
</template>

<script lang="ts" setup>
import { ref, computed } from "vue"
import { DiffView, DiffModeEnum } from "@git-diff-view/vue"
import "@git-diff-view/vue/styles/diff-view-pure.css"
import { AccordionRoot, AccordionItem, AccordionContent } from "reka-ui"
import type { UncommittedFileChange, FileDiff } from "~/utils/bindings"
import { commands } from "~/utils/bindings"

// Props
const props = defineProps<{
  files: Array<UncommittedFileChange>
  keyPrefix: string
  diffViewMode?: "unified" | "split"
}>()

// Reactive state
const expandedItems = ref<string[]>([])
const loadedDiffs = ref<Map<string, FileDiff>>(new Map())
const loadingFiles = ref<Set<string>>(new Set())
const fileErrors = ref<Map<string, string>>(new Map())

// Diff view mode (unified or split) - use prop value if provided, otherwise use internal state
const internalDiffMode = ref<"unified" | "split">("unified")
const currentDiffMode = computed(() => props.diffViewMode || internalDiffMode.value)

// Color mode for theme
const colorMode = useColorMode()

// Get VCS request factory for repository operations
const { vcsRequestFactory } = useRepository()

// Helper function to check if diff is loaded
const hasLoadedDiff = (filePath: string) => {
  return loadedDiffs.value.has(filePath)
}

// Handle accordion state changes and load content on demand
function onAccordionChange(newExpanded: string | string[] | undefined) {
  if (!newExpanded) {
    return
  }

  const expandedArray = Array.isArray(newExpanded) ? newExpanded : [newExpanded]
  const previousExpanded = expandedItems.value || []
  const newlyExpanded = expandedArray.filter(item => !previousExpanded.includes(item))

  expandedItems.value = expandedArray

  for (const itemValue of newlyExpanded) {
    const itemIndex = parseInt(itemValue.replace("item-", ""))
    const file = props.files[itemIndex]

    if (file && !loadedDiffs.value.has(file.filePath) && !loadingFiles.value.has(file.filePath)) {
      loadFileContent(file.filePath)
    }
  }
}

// Load file content for diff display
async function loadFileContent(filePath: string) {
  loadingFiles.value.add(filePath)
  fileErrors.value.delete(filePath)

  try {
    const vcsRequest = vcsRequestFactory.createRequest()
    const result = await commands.getFileContentForDiff({
      repositoryPath: vcsRequest.repositoryPath,
      filePath: filePath,
    })

    if (result.status === "ok") {
      loadedDiffs.value.set(filePath, result.data)
    }
    else {
      throw new Error(result.error)
    }
  }
  catch (error) {
    fileErrors.value.set(filePath, `Failed to load content: ${error}`)
    console.error("Failed to load file content:", error)
  }
  finally {
    loadingFiles.value.delete(filePath)
  }
}

function getStatusColor(status: string): "success" | "info" | "error" | "neutral" {
  switch (status) {
    case "added": return "success"
    case "modified": return "info"
    case "deleted": return "error"
    case "renamed": return "neutral"
    case "copied": return "neutral"
    default: return "info"
  }
}
</script>