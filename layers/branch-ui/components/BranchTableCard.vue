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
          <UContextMenu :items="getContextMenuItems(branch)" :disabled="isProcessing(branch.name)">
            <tr
              data-testid="branch-row"
              :data-branch-name="branch.name"
              :data-state="isExpanded(branch) ? 'open' : 'closed'"
              :class="[
                'branch-row hover:bg-muted transition-all cursor-pointer group',
                isExpanded(branch) && 'data-[state=open]:bg-elevated',
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

          <!-- Single inline form portal row (for any active inline form) -->
          <tr
            v-if="activeInline?.branchName === branch.name && (activeInline?.type === 'issue-reference' || activeInline?.type === 'amend-changes' || activeInline?.type === 'unapply')"
            :key="`${branch.name}-inline`">
            <td :id="portalTargetIdFor(branch.name)" colspan="4" class="p-0 relative overflow-hidden">
              <!-- Portal target for any inline form -->
              <!-- Progress bar overlaying the top border -->
              <div
                v-if="activeInline?.processing"
                class="absolute top-0 left-0 w-full h-px bg-accented overflow-hidden">
                <div class="absolute inset-y-0 w-1/2 bg-primary animate-[carousel_2s_ease-in-out_infinite]"/>
              </div>
            </td>
          </tr>

          <!-- Inline amend conflict viewer row -->
          <tr v-if="activeInline?.branchName === branch.name && activeInline?.type === 'amend-conflict'" :key="`${branch.name}-conflict`">
            <td colspan="4" class="p-0">
              <div class="bd-padding-content border-b border-default bg-warning/5">
                <!-- Header with dismiss button -->
                <div class="flex items-center justify-between mb-3">
                  <div class="flex items-center gap-2">
                    <UIcon name="i-lucide-alert-triangle" class="size-5 text-warning" />
                    <span class="font-medium">Cannot amend: conflicts with subsequent commits</span>
                  </div>
                  <UButton
                    size="xs"
                    variant="ghost"
                    icon="i-lucide-x"
                    @click="closeInline"
                  />
                </div>

                <!-- Conflict viewer component -->
                <LazyMergeConflictViewer
                  :conflict="activeInline.conflictInfo!"
                  :branch-name="activeInline.branchName"
                />
              </div>
            </td>
          </tr>

          <!-- Expanded row content -->
          <tr v-if="isExpanded(branch)" :key="`${branch.name}-expanded`" class="!border-t-0">
            <td colspan="4" class="px-0 py-4">
              <ActiveBranchExpanded
                :branch="branch"
                :highlight-tip-commit="activeInline?.branchName === branch.name && activeInline?.type === 'amend-changes'"
              />
            </td>
          </tr>
        </template>
      </tbody>
    </table>

    <!-- Inline issue reference input (renders via portal) -->
    <LazyActionsInlineIssueReferenceInput v-if="activeInline?.type === 'issue-reference'" />

    <!-- Inline amend changes input (renders via portal) -->
    <LazyActionsInlineAmendChangesInput v-if="activeInline?.type === 'amend-changes'" />

    <!-- Inline unapply confirmation (renders via portal) -->
    <LazyActionsInlineUnapplyConfirm v-if="activeInline?.type === 'unapply'" />
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
const { isExpanded, toggleExpanded, setExpanded } = useGenericTableExpansion((branch: ReactiveBranch) => branch.name)

const { activeInline, portalTargetIdFor, isProcessing, closeInline } = useInlineRowAction()
const { getContextMenuItems } = useBranchContextActions({ setExpanded })

// Listen for sync-branches event from menu
scopedListen("sync-branches", () => {
  syncBranches()
})
</script>