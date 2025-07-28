<template>
  <UCard class="overflow-hidden">
    <template #header>
      <CardHeader
        title="Conflicting Files"
        :count="conflicts.length"
        item-singular="file"
        item-plural="files"
      >
        <template #actions>
          <UButton
            v-if="!isInWindow"
            size="xs"
            variant="ghost"
            icon="i-lucide-external-link"
            @click="openConflictingFilesWindow"
          >
            Open in Window
          </UButton>
        </template>
      </CardHeader>
    </template>

    <ConflictingFilesSection
      :conflicts="conflicts"
      :conflict-info="conflictInfo"
      :conflict-marker-commits="conflictMarkerCommits"
    />

    <template #footer>
      <InfoCard
        title="Missing commits cause conflicts"
        icon="i-lucide-info"
      >
        <p>
          This commit cannot be copied to the virtual branch because other commits modified the same files first.
        </p>
        <p>
          Apply the missing commits shown above to resolve conflicts automatically.
        </p>
      </InfoCard>
    </template>
  </UCard>
</template>

<script lang="ts" setup>
import type { ConflictDetail, MergeConflictInfo, ConflictMarkerCommitInfo } from "~/utils/bindings"

const props = defineProps<{
  conflicts: ConflictDetail[]
  conflictInfo: MergeConflictInfo
  conflictMarkerCommits: Record<string, ConflictMarkerCommitInfo>
  branchName?: string
  isInWindow?: boolean
}>()

async function openConflictingFilesWindow() {
  const data = {
    conflict: props.conflictInfo,
    branchName: props.branchName || "Unknown",
  }

  await openSubWindow({
    windowId: "conflicting-files",
    url: "/conflicting-files",
    title: `Conflicting Files - ${props.branchName || "Unknown Branch"}`,
    data,
  })
}
</script>
