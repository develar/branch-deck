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
            icon="i-heroicons-folder-open"
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

          <BranchPrefixHelp :configured="branchPrefix.status == 'ok'" :disabled="isSyncing"/>
        </UButtonGroup>
      </UFormField>

      <!-- Actions -->
      <UButton
        :disabled="isSyncing || !repositoryPath || !mutableBranchPrefix"
        :loading="isSyncing"
        icon="i-heroicons-arrow-path"
        @click="createBranches"
      >
        Sync Virtual Branches
      </UButton>
    </div>
    <!-- Loading State -->
    <div v-if="isSyncing" class="py-8 space-y-4">
      <UProgress animation="carousel"/>
    </div>
  </UCard>

  <!-- Results -->
  <UCard v-if="syncResult && !isSyncing" class="mt-6">
    <UAlert
      v-if="syncResult.status == 'error'"
      :title="`Error: ${syncResult.error}`"
      color="error"
      icon="i-heroicons-x-circle"
      variant="soft"
    />
    <UAlert
      v-else-if="syncResult.data.branches.length === 0"
      color="info"
      title="No branches found"
      variant="soft"
    />
    <div v-else class="space-y-4">
      <!-- Branches Tree -->
      <div v-if="syncResult.data.branches.length > 0" class="space-y-4">
        <UTree :items="branchTreeData" :ui="{linkLabel: 'grid grid-cols-4 justify-items-start place-content-end items-center gap-2 w-full'}">
          <!--suppress VueUnrecognizedSlot -->
          <template #item-label="{ item }">
            <span class="truncate">{{ item.label }}</span>

            <div class="flex items-center gap-4">
              <UButton
                v-if="item.meta.commit_count > 0 && item.meta?.name && !item.meta.error"
                :disabled="isPushing(item.meta.name)"
                :loading="isPushing(item.meta.name)"
                icon="i-heroicons-arrow-up-tray"
                size="sm"
                variant="outline"
                @click.stop="pushBranch(item.meta.name)"
              >
                {{ item.meta.sync_status === "Updated" ? "Force Push" : "Push" }}
              </UButton>

              <UBadge
                v-if="item.meta.sync_status"
                :color="item.meta.error ? 'error' : 'info'"
                class="lowercase"
                variant="soft"
              >
                {{ item.meta.error ?? item.meta.sync_status }}
              </UBadge>
            </div>

            <span v-if="item.meta.commit_count" class="flex items-center gap-2 text-xs text-neutral-500">
              {{ item.meta.commit_count }} commit{{ item.meta.commit_count === 1 ? "" : "s" }}
            </span>
          </template>

          <!--suppress VueUnrecognizedSlot -->
          <!-- @vue-ignore -->
          <template #commit-label="{ item }">
            <span class="truncate flex-1">{{ item.label }}</span>
            <span class="text-xs text-neutral-500 font-mono">
              {{ formatTimestamp((item as BranchChild).commit!!.time) }}
            </span>
            <span class="text-xs text-neutral-500 font-mono">
              {{ (item as BranchChild).commit!!.hash.substring(0, 8) }}
            </span>
          </template>
        </UTree>
      </div>
    </div>
  </UCard>
</template>

<script lang="ts" setup>
import { computed } from "vue"
import { useRecentPath } from "../composables/recentPath"
import { usePush } from "../composables/push"
import { useSyncBranches } from "../composables/syncBranches"
import { useVcsRequest } from "../composables/vcsRequest"
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import { BranchInfo, CommitDetail } from "../bindings.ts"
import { formatTimestamp } from "./time.ts"

const {recentPaths, onRepositoryPathChange, addToRecentPaths, repositoryPath} = useRecentPath()

const {branchPrefix, mutableBranchPrefix, vcsRequestFactory} = useVcsRequest(repositoryPath)

const {createBranches, syncResult, isSyncing} = useSyncBranches(vcsRequestFactory)
const {pushBranch, isPushing} = usePush(vcsRequestFactory)

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

const branchTreeData = computed(() => {
  if (syncResult?.value?.status !== "ok") {
    return []
  }

  return syncResult.value.data.branches.map((branch: BranchInfo, index) => {
    let children: Array<BranchChild>

    if (branch.sync_status === "Error") {
      // add error message as child if there's an error
      children = [
        {
          id: `error-${index}`,
          label: branch.error!,
          icon: "i-heroicons-x-circle",
          iconClass: "error",
        },
      ]
    }
    else if (branch.commit_details && branch.commit_details.length > 0) {
      // add commits as children if they exist and no error
      children = branch.commit_details.map((commit, commitIndex) => ({
        id: `commit-${index}-${commitIndex}`,
        label: commit.message,
        slot: "commit",
        commit: commit,
      }))
    }
    else {
      children = []
    }

    return {
      id: `branch-${index}`,
      label: branch.name,
      meta: branch,
      defaultExpanded: branch.sync_status != "Unchanged",
      children,
    }
  })
})

interface BranchChild {
  id: string
  label: string
  slot?: string
  icon?: string
  iconClass?: string
  commit?: CommitDetail
}
</script>
