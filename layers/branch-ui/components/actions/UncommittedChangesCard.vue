<template>
  <UCard class="overflow-hidden" data-testid="uncommitted-changes-card">
    <template #header>
      <CardHeader
        title="Uncommitted Changes"
        :count="fileCount"
        item-singular="file"
        item-plural="files"
      >
        <template #icon>
          <UIcon name="i-lucide-file-diff" class="size-5 text-primary" />
        </template>
        <template #subtitle>
          <div v-if="!loading && !error && diffData?.hasChanges" class="text-sm text-muted">
            {{ stagedCount }} staged, {{ unstagedCount }} unstaged
          </div>
        </template>
        <template #actions>
          <div class="flex items-center gap-3">
            <!-- Help popover -->
            <UPopover mode="hover">
              <UIcon name="i-lucide-info" class="size-3.5 cursor-pointer text-muted hover:text-highlighted transition-colors" />
              <template #content>
                <div class="p-3 space-y-2 text-xs w-xs">
                  <p>This will amend your uncommitted changes to the tip commit of this virtual branch.</p>
                  <p>The original commit message and author information will be preserved.</p>
                </div>
              </template>
            </UPopover>

            <!-- Unified/Split toggle -->
            <DiffViewToggle v-if="!loading && !error && diffData?.hasChanges" />
          </div>
        </template>
      </CardHeader>
    </template>

    <!-- Loading State -->
    <div v-if="loading" class="flex items-center justify-center py-8">
      <div class="flex items-center space-x-3">
        <UIcon name="i-lucide-loader-2" class="size-4 animate-spin text-muted" />
        <span class="text-sm text-muted">Loading uncommitted changes...</span>
      </div>
    </div>

    <!-- Error State -->
    <div v-else-if="error" class="bd-padding-card">
      <div class="rounded-md border border-error/20 bg-error/10 p-4">
        <div class="flex items-center space-x-2">
          <UIcon name="i-lucide-alert-circle" class="size-4 text-error" />
          <span class="text-sm text-error">{{ error }}</span>
        </div>
      </div>
    </div>

    <!-- No Changes State -->
    <div v-else-if="!diffData?.hasChanges" class="bd-padding-card">
      <div class="rounded-md border border-muted/20 bg-muted/10 p-4">
        <div class="flex items-center space-x-2">
          <UIcon name="i-lucide-info" class="size-4 text-muted" />
          <span class="text-sm text-muted">No uncommitted changes to amend</span>
        </div>
      </div>
    </div>

    <!-- File Diffs -->
    <div v-else-if="diffData?.files.length > 0">
      <OnDemandFileDiffList
        :files="diffData.files"
        key-prefix="uncommitted"
        :diff-view-mode="store.conflictDiffViewMode"
      />
    </div>

  </UCard>
</template>

<script lang="ts" setup>
import type { UncommittedChangesResult } from "~/utils/bindings"

const props = defineProps<{
  diffData?: UncommittedChangesResult | null
  loading?: boolean
  error?: string | null
}>()

// Get conflict viewer settings from store (still needed for FileDiffList)
const store = useConflictViewerStore()

// Computed properties for header display
const fileCount = computed(() => {
  return props.diffData?.files.length || 0
})

const stagedCount = computed(() => {
  return props.diffData?.files.filter(f => f.staged).length || 0
})

const unstagedCount = computed(() => {
  return props.diffData?.files.filter(f => f.unstaged).length || 0
})
</script>