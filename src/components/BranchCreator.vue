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

          <BranchPrefixHelp :configured="branchPrefix.status == 'ok'" :disabled="isSyncing" />
        </UButtonGroup>
      </UFormField>

      <!-- Actions -->
      <UButton
        v-if="!showProgress"
        :disabled="isSyncing || !repositoryPath || !mutableBranchPrefix"
        :loading="isSyncing && !showProgress"
        icon="i-heroicons-arrow-path"
        @click="createBranches"
      >
        Sync Virtual Branches
      </UButton>
      <!-- Loading State -->
      <div v-else-if="showProgress" class="flex flex-col items-center justify-center gap-3 py-2">
        <span class="text-sm text-dimmed">
          {{ syncProgress }}
        </span>
        <UProgress />
      </div>
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
        <UTree :items="branchTreeData" :ui="{linkLabel: 'grid grid-cols-4 justify-items-start items-center gap-2 w-full' }">
          <!--suppress VueUnrecognizedSlot -->
          <template #item-label="{ item }">
            <span class="truncate">{{ item.label }}</span>

            <div class="flex items-center gap-4">
              <UButton
                v-if="item.branch.sync_status != 'Error' && item.branch.commit_count > 0"
                :disabled="isPushing(item.branch.name)"
                :loading="isPushing(item.branch.name)"
                icon="i-heroicons-arrow-up-tray"
                size="sm"
                variant="outline"
                @click.stop="pushBranch(item.branch.name)"
              >
                {{ item.branch.sync_status === "Updated" ? "Force Push" : "Push" }}
              </UButton>
              <UBadge
                v-if="item.branch.sync_status"
                :color="item.branch.error ? 'error' : 'info'"
                class="lowercase"
                variant="soft"
              >
                {{ (item.branch.error?.split(":")?.[1] ?? item.branch.error) ?? item.branch.sync_status }}
              </UBadge>
            </div>

            <span v-if="item.branch.sync_status != 'Error'" class="flex items-center gap-2 text-xs text-neutral-500">
              {{ item.branch.commit_count }} commit{{ item.branch.commit_count === 1 ? "" : "s" }}
            </span>
          </template>

          <!--suppress VueUnrecognizedSlot -->
          <!-- @vue-ignore -->
          <template #commit-label="{ item }">
            <span class="truncate col-span-2">{{ item.label }}</span>
            <span class="text-xs text-neutral-500 font-mono">
              {{ formatTimestamp((item as BranchChild).commit.time) }}
            </span>
            <span class="text-xs text-neutral-500 font-mono">
              {{ (item as BranchChild).commit.hash.substring(0, 8) }}
            </span>
          </template>
          <!--suppress VueUnrecognizedSlot -->
          <!-- @vue-ignore -->
          <template #commit-error-label="{ item }">
            <!-- https://github.com/nuxt/ui/issues/4424 -->
            <UAlert
              :description="item.commit.message"
              color="error"
              variant="outline"
              icon="i-lucide-git-pull-request-closed"
              class="col-span-4"
            />
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

const { recentPaths, onRepositoryPathChange, addToRecentPaths, repositoryPath } = useRecentPath()

const { branchPrefix, mutableBranchPrefix, vcsRequestFactory } = useVcsRequest(repositoryPath)

const { createBranches, syncResult, isSyncing, showProgress, syncProgress } = useSyncBranches(vcsRequestFactory)
const { pushBranch, isPushing } = usePush(vcsRequestFactory)

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
  }
  catch (error) {
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
      children = [{
        id: `commit-${index}-0`,
        label: "error",
        slot: "commit-error",
        commit: {
          original_hash: "",
          hash: "",
          is_new: true,
          time: 0,
          message: branch.error || "Unknown error",
        },
      }]
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
      branch,
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
  commit: CommitDetail
}
</script>
