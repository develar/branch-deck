<template>
  <UCard>
    <div class="space-y-6">
      <!-- Repository Path -->
      <UFormField label="Repository Path" name="repo-path">
        <UButtonGroup class="flex">
          <USelect
            v-model="repositoryPath"
            :disabled="isSyncing"
            :items="recentPaths"
            class="flex-1"
            creatable
            placeholder="Select or enter repository path..."
            searchable
            @update:model-value="onRepositoryPathChange"
          />
          <UButton
            :disabled="isSyncing"
            icon="i-lucide-folder-search"
            variant="outline"
            @click="browseRepository"
          >
            Browse
          </UButton>
        </UButtonGroup>
      </UFormField>

      <!-- Branch Prefix -->
      <UFormField label="Branch Prefix" name="branch-prefix">
        <UButtonGroup>
          <UInput
            v-model="mutableBranchPrefix"
            :disabled="isSyncing"
            class="flex-1"
            placeholder="Enter branch prefix..."
          />

          <BranchPrefixHelp :configured="gitProvidedBranchPrefix.status == 'ok' && gitProvidedBranchPrefix.data != ''" :disabled="isSyncing"/>
        </UButtonGroup>
      </UFormField>

      <!-- Actions -->
      <UButton
        v-if="!showProgress"
        :disabled="isSyncing || !repositoryPath || !mutableBranchPrefix"
        :loading="isSyncing && !showProgress"
        icon="i-lucide-refresh-cw"
        @click="createBranches"
      >
        Sync Virtual Branches
      </UButton>
      <!-- Loading State -->
      <div v-else-if="showProgress" class="flex flex-col items-center justify-center gap-3 py-2">
        <span class="text-sm text-dimmed">
          {{ syncProgress }}
        </span>
        <UProgress/>
      </div>
    </div>
  </UCard>

  <!-- Results -->
  <UCard v-if="syncError || branches.length > 0" class="mt-6" :ui=" { body: 'p-0 sm:p-0' }">
    <!-- Show error if sync failed -->
    <div v-if="syncError && !isSyncing" class="p-4">
      <UAlert
        :title="`Error: ${syncError}`"
        color="error"
        icon="i-lucide-x-circle"
        variant="soft"
      />
    </div>
    <!-- Show branch data -->
    <div v-else-if="branches.length > 0" class="p-0">
      <!-- Branches Table -->
      <UTable
        ref="tableRef"
        v-model:expanded="expanded"
        :data="branches"
        :columns="incrementalBranchColumns"
        :row-key="'name'"
        @select="onRowSelect"
      >
        <!-- Branch name column -->
        <template #name-cell="{ row }">
          <div class="flex items-center gap-2">
            <UButton
              v-if="row.original.commit_count > 0 || row.original.hasError"
              :icon="row.getIsExpanded() ? 'i-lucide-folder-open' : 'i-lucide-folder-closed'"
              variant="ghost"
              @click.stop="row.toggleExpanded()"
            />
            <span class="font-medium">{{ row.original.name }}</span>
          </div>
        </template>

        <!-- Status column -->
        <template #status-cell="{ row }">
          <div class="flex items-center gap-2">
            <UProgress 
              v-if="row.original.status === 'Syncing'"
              :model-value="row.original.processedCount" 
              :max="row.original.commit_count"
              status
              size="sm" 
              class="w-20"
            />
            <UBadge
              v-else
              :color="getIncrementalStatusColor(row.original.status)"
              variant="soft"
              class="lowercase"
            >
              {{ row.original.statusText }}
            </UBadge>
          </div>
        </template>

        <!-- Commits column -->
        <template #commit_count-cell="{ row }">
          <span class="text-sm text-muted">
            {{ row.original.commit_count }} commit{{ row.original.commit_count === 1 ? "" : "s" }}
          </span>
        </template>

        <!-- Actions column -->
        <template #actions-cell="{ row }">
          <UButton
            v-if="!row.original.hasError && row.original.commit_count > 0"
            :disabled="isPushing(row.original.name) || isSyncing || row.original.status === 'Syncing'"
            :loading="isPushing(row.original.name)"
            icon="i-lucide-upload"
            size="sm"
            variant="outline"
            @click.stop="pushBranch(row.original.name)"
          >
            {{ row.original.status === "Updated" ? "Force Push" : "Push" }}
          </UButton>
        </template>
        <!-- Expanded row content -->
        <template #expanded="{ row }">
          <CommitList v-if="row.original.commits?.size > 0" :commits="row.original.commits" :branch-name="row.original.name"/>
        </template>
      </UTable>
    </div>
    <!-- Show empty state if no branches after sync completes -->
    <div v-else-if="!isSyncing && !syncError && branches.length === 0" class="p-4">
      <UAlert
        color="info"
        title="No branches found"
        variant="soft"
      />
    </div>
  </UCard>
</template>

<script lang="ts" setup>
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import type { Table } from "@tanstack/table-core"
import type { ReactiveBranch } from "~/composables/syncBranches"

const {recentPaths, onRepositoryPathChange, addToRecentPaths, repositoryPath} = useRecentPath()

const {gitProvidedBranchPrefix, mutableBranchPrefix, vcsRequestFactory} = useVcsRequest(repositoryPath)

const tableRef = useTemplateRef("tableRef")

const expandBranch = (branchName: string) => {
  const table = tableRef.value as { tableApi: Table<ReactiveBranch> }
  if (table) {
    const row = table.tableApi.getRowModel().rows.find(row => row.original.name === branchName)
    if (row && !row.getIsExpanded()) {
      row.toggleExpanded()
    }
  }
}

const {createBranches, syncError, isSyncing, showProgress, syncProgress, branches, expanded} = useSyncBranches(vcsRequestFactory, expandBranch)
const {pushBranch, isPushing} = usePush(vcsRequestFactory)

// Helper function for incremental status badge color
const getIncrementalStatusColor = (status: string) => {
  switch (status) {
    case "Error":
    case "error":
      return "error"
    case "MergeConflict":
    case "AnalyzingConflict":
      return "error"
    case "syncing":
      return "info"
    case "Created":
      return "success"
    case "processing…":
    case "Updated":
      return "info"
    default:
      return "neutral"
  }
}

// Table columns
const incrementalBranchColumns = [
  {id: "name", accessorKey: "name", header: "Branch Name"},
  {id: "commit_count", accessorKey: "commit_count", header: "Commits"},
  {id: "status", header: "Status"},
  {id: "actions", header: "Actions"},
]

// Handle row selection to expand/collapse
interface TableRow {
  original: {
    commit_count: number
    hasError?: boolean
  }
  toggleExpanded: () => void
}

const onRowSelect = (row: TableRow) => {
  // Toggle expansion if the row has commits or errors
  if (row.original.commit_count > 0 || row.original.hasError) {
    row.toggleExpanded()
  }
}

const browseRepository = async () => {
  try {
    const path = await openFileDialog({
      title: "Select Project Repository",
      directory: true,
      canCreateDirectories: false,
    })
    if (path) {
      repositoryPath.value = path
      await addToRecentPaths(path)
    }
  } catch (error) {
    console.error("Failed to open directory dialog:", error)
  }
}
</script>
