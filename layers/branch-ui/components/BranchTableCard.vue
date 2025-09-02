<template>
  <UCard v-if="branches.length > 0" :ui="{ body: 'p-0 sm:p-0 overflow-x-auto' }">
    <table class="w-full">
      <thead class="bg-muted/50 border-b border-default">
        <tr>
          <TableHeader>Branch Name</TableHeader>
          <TableHeader class="hidden sm:table-cell">Commits</TableHeader>
          <TableHeader class="hidden sm:table-cell">Status</TableHeader>
          <TableHeader>Actions</TableHeader>
        </tr>
      </thead>
      <tbody class="divide-y divide-default">
        <template v-for="branch in branches" :key="branch.name">
          <!-- Branch row -->
          <UContextMenu :items="getContextMenuItems(branch)">
            <tr
              data-testid="branch-row"
              :data-branch-name="branch.name"
              :data-state="isExpanded(branch) ? 'open' : 'closed'"
              :class="[
                'branch-row hover:bg-muted transition-all cursor-pointer group',
                isExpanded(branch) && 'data-[state=open]:bg-elevated',
                pulseClass(branch.name),
              ]"
              @click="toggleExpanded(branch)"
            >
              <BranchNameCell
                :name="branch.name"
                :summary="branch.summary"
                :expanded="isExpanded(branch)"
                :can-expand="branch.commitCount > 0 || branch.hasError"
                @toggle-expanded="toggleExpanded(branch)"
              />
              <td class="px-6 py-4 hidden sm:table-cell">
                <span class="text-sm text-muted">
                  {{ branch.commitCount === 1 ? '1 commit' : `${branch.commitCount} commits` }}
                </span>
              </td>
              <td class="px-6 py-4 hidden sm:table-cell">
                <BranchStatusBadge :branch="branch" />
              </td>
              <td class="px-6 py-4">
                <BranchActions :branch="branch" />
              </td>
            </tr>
          </UContextMenu>

          <!-- Inline issue reference input row -->
          <tr v-if="activeInline?.branchName === branch.name && activeInline?.type === 'issue-reference'" :key="`${branch.name}-inline`">
            <td colspan="4" class="p-0">
              <!-- Portal target for dialog content -->
              <div :id="portalTargetIdFor(branch.name)" />
            </td>
          </tr>

          <!-- Expanded row content -->
          <tr v-if="isExpanded(branch)" :key="`${branch.name}-expanded`" class="!border-t-0">
            <td colspan="4" class="px-0 py-4">
              <ActiveBranchExpanded :branch="branch" />
            </td>
          </tr>
        </template>
      </tbody>
    </table>

    <!-- Inline issue reference input (renders via portal) -->
    <LazyActionsInlineIssueReferenceInput
      v-if="activeInline?.type === 'issue-reference' && activeInline?.branchName"
      :branch-name="activeInline?.branchName || ''"
      :commit-count="getActiveBranchCommitCount()"
      :dialog-title="activeInline?.branchName ? `Add Issue Reference to ${activeInline.branchName}` : ''"
      :dialog-description="activeInline?.branchName ? `Add issue reference form for ${activeInline.branchName} branch` : ''"
      :portal-target="activeInline?.branchName ? portalTargetIdFor(activeInline.branchName) : undefined"
      :is-active="!!activeInline"
      @submit="(issueReference: string) => handleInlineSubmit(issueReference, getActiveBranch()!)"
      @cancel="hideInlineInput"
    />
  </UCard>
</template>

<script lang="ts" setup>
import { useGenericTableExpansion } from "~/composables/useGenericTableExpansion"
import BranchNameCell from "./cells/BranchNameCell.vue"
import BranchStatusBadge from "./cells/BranchStatusBadge.vue"
import BranchActions from "./cells/BranchActions.vue"
import ActiveBranchExpanded from "./ActiveBranchExpanded.vue"
import TableHeader from "~/components/shared/TableHeader.vue"

const { syncBranches, branches } = useBranchSync()

// Use generic table expansion composable
const { isExpanded, toggleExpanded } = useGenericTableExpansion((branch: ReactiveBranch) => branch.name)

// Context actions composable
const {
  activeInline,
  getContextMenuItems,
  hideInlineInput,
  handleInlineSubmit,
  portalTargetIdFor,
  pulseClass,
} = useBranchContextActions()

// Get active branch data
const getActiveBranch = () => {
  if (activeInline.value?.branchName) {
    return branches.value.find(b => b.name === activeInline.value!.branchName)
  }
  else {
    return null
  }
}

const getActiveBranchCommitCount = () => {
  return getActiveBranch()?.commitCount || 0
}

// Listen for sync-branches event from menu
scopedListen("sync-branches", () => {
  syncBranches()
})
</script>