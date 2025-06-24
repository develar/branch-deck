<template>
  <UCard>
    <template #header>
      <h2>Branch Manager</h2>
    </template>
    <div class="space-y-6">
      <!-- Repository Path -->
      <UFormField label="Repository Path" name="repo-path">
        <div class="flex gap-3">
          <USelect
            v-model="repositoryPath"
            :items="recentPaths"
            placeholder="Select or enter repository path..."
            searchable
            creatable
            class="flex-1"
            :disabled="isSyncing"
            @update:model-value="onRepositoryPathChange"
          />
          <UButton
            icon="i-heroicons-folder-open"
            variant="outline"
            :disabled="isSyncing"
            @click="browseRepository"
          >
            Browse
          </UButton>
        </div>
      </UFormField>

      <!-- Branch Prefix -->
      <UFormField label="Branch Prefix" name="branch-prefix">
        <UButtonGroup>
          <UInput
            v-model="branchPrefix"
            placeholder="Enter branch prefix..."
            class="flex-1"
            :disabled="isSyncing"
          />

          <BranchPrefixHelp :disabled="isSyncing" />
        </UButtonGroup>
      </UFormField>

      <!-- Actions -->
      <UButton
        icon="i-heroicons-arrow-path"
        :loading="isSyncing"
        :disabled="isSyncing || !repositoryPath"
        @click="createBranches"
      >
        Sync Virtual Branches
      </UButton>
    </div>
    <!-- Loading State -->
    <div v-if="isSyncing" class="py-8 space-y-4">
      <UProgress animation="carousel" />
    </div>

    <!-- Results -->
    <div v-if="result && !isSyncing" class="mt-6">
      <UAlert
        v-if="result.branches?.length === 0"
        color="info"
        variant="soft"
        :title="`${result.message || 'No branches found'}`"
      />
      <div v-else-if="result.success" class="space-y-4">
        <USeparator />

        <!-- Branches Tree -->
        <div v-if="result.branches?.length > 0" class="space-y-4">
          <UTree :items="branchTreeData" :ui="{linkLabel: 'grid grid-cols-4 justify-items-start place-content-end items-center gap-2 w-full'}">
            <!--suppress VueUnrecognizedSlot -->
            <template #item-label="{ item }">
              <span class="truncate">{{ item.label }}</span>

              <div class="flex items-center gap-4">
                <UButton
                  v-if="item.meta.commitCount > 0 && item.meta?.name && !item.meta.error"
                  size="xs"
                  variant="outline"
                  icon="i-heroicons-arrow-up-tray"
                  :loading="isPushing(item.meta.name)"
                  :disabled="isPushing(item.meta.name)"
                  @click.stop="pushBranch(item.meta.name)"
                >
                  {{ item.meta.syncStatus === backend.BranchSyncStatus.UPDATED ? "Force Push" : "Push" }}
                </UButton>

                <UBadge
                  v-if="item.meta.syncStatus"
                  :color="item.meta.error ? 'error' : 'info'"
                  size="md"
                  variant="soft"
                >
                  {{ item.meta.error ?? syncStatusToString.get(item.meta.syncStatus) }}
                </UBadge>
              </div>

              <span v-if="item.meta.commitCount" class="flex items-center gap-2 text-xs text-neutral-500">
                {{ item.meta.commitCount }} commit{{ item.meta.commitCount === 1 ? "" : "s" }}
              </span>
            </template>

            <!--suppress VueUnrecognizedSlot -->
            <template #commit-label="{ item }">
              <div class="flex items-center gap-3">
                <span class="truncate flex-1">{{ item.label }}</span>
                <span class="text-xs text-neutral-500 font-mono">
                  {{ item.meta.hash.substring(0, 8) }}
                </span>
              </div>
            </template>
          </UTree>
        </div>
      </div>
      <UAlert
        v-else
        icon="i-heroicons-x-circle"
        color="error"
        variant="soft"
        :title="`Error: ${result.message}`"
      />
    </div>
  </UCard>
</template>

<script setup lang="ts">
import {computed} from "vue"
import {OpenDirectoryDialog} from "../../wailsjs/go/main/App"
import {backend} from "../../wailsjs/go/models"
import {useRecentPath} from "../composables/recentPath"
import {usePush} from "../composables/push"
import {syncStatusToString, useSyncBranches} from "../composables/syncBranches"
import {useVcsRequest} from "../composables/vcsRequest"

const {recentPaths, onRepositoryPathChange, addToRecentPaths, repositoryPath} = useRecentPath()

// @ts-expect-error we use Suspense
const {branchPrefix, vcsRequestFactory} = await useVcsRequest(repositoryPath)

const {createBranches, result, isSyncing} = useSyncBranches(vcsRequestFactory)
const {pushBranch, isPushing} = usePush(vcsRequestFactory)

const browseRepository = async () => {
  try {
    const path = await OpenDirectoryDialog()
    if (path) {
      repositoryPath.value = path
      await addToRecentPaths(path)
    }
  } catch (error) {
    console.error("Failed to open directory dialog:", error)
  }
}

const branchTreeData = computed(() => {
  if (!result.value?.branches) {
    return []
  }

  return result.value.branches.map((branch: backend.BranchResult, index) => {
    let children = []

    // add commits as children if they exist and no error
    if (!branch.error && branch.commitDetails && branch.commitDetails.length > 0) {
      children = branch.commitDetails.map((commit, commitIndex) => ({
        id: `commit-${index}-${commitIndex}`,
        label: commit.message,
        slot: "commit",
        meta: {
          hash: commit.hash,
        },
      }))
    }

    // add error message as child if there's an error
    if (branch.error) {
      children = [
        {
          id: `error-${index}`,
          label: branch.error,
          icon: "i-heroicons-x-circle",
          iconClass: "error",
        },
      ]
    }

    return {
      id: `branch-${index}`,
      label: branch.name,
      meta: branch,
      defaultExpanded: branch.syncStatus != backend.BranchSyncStatus.UNCHANGED,
      children: children,
    }
  })
})
</script>
