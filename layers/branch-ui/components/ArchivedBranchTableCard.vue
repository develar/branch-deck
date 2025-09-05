<template>
  <CollapsibleCard v-if="hasArchivedBranches" data-testid="archived-branches-card">
    <template #header>
      <CardHeader
        title="Archived Branches"
        :count="archivedCount"
        item-singular="branch"
        item-plural="branches"
        badge-color="neutral"
      >
        <template #actions>
          <ArchivedBranchesHelpPopover />
          <UIcon
            name="i-lucide-chevron-down"
            class="w-4 h-4 text-muted transition-transform duration-200 group-data-[state=open]:rotate-180"
          />
        </template>
      </CardHeader>
    </template>
    <table class="w-full">
      <thead class="bg-muted/50 border-b border-default">
        <tr>
          <TableHeader>Branch Name</TableHeader>
          <TableHeader class="hidden sm:table-cell">Status</TableHeader>
          <TableHeader class="hidden sm:table-cell">Integrated</TableHeader>
          <TableHeader>Actions</TableHeader>
        </tr>
      </thead>
      <tbody class="divide-y divide-default">
        <template v-for="branch in archivedBranches.archivedBranches.value" :key="branch.name">
          <UContextMenu :items="getArchivedContextMenuItems(branch)" :disabled="inline.isProcessing(branch.name)">
            <tr
              :data-branch-name="branch.name"
              :data-state="isExpanded(branch) ? 'open' : 'closed'"
              :class="[
                'hover:bg-muted transition-all cursor-pointer',
                isExpanded(branch) && 'bg-elevated',
                branch.isLoadingCommits && 'animate-pulse',
              ]"
              @click="toggleExpanded(branch)"
            >
              <BranchNameCell
                :name="branch.name"
                :summary="branch.summary"
                :expanded="isExpanded(branch)"
                :can-expand="branch.type !== 'placeholder'"
                :simplified="true"
                @toggle-expanded="toggleExpanded(branch)"
              />
              <td class="px-6 py-4 hidden sm:table-cell">
                <ArchivedBranchStatusBadge :branch="branch" />
              </td>
              <td class="px-6 py-4 hidden sm:table-cell">
                <ArchivedDateCell :branch="branch" />
              </td>
              <td class="px-6 py-4">
                <CopyButton
                  :text="() => branch.name"
                  tooltip="Copy full branch name"
                  size="xs"
                  always-visible
                />
              </td>
            </tr>
          </UContextMenu>

          <!-- Single inline form portal row -->
          <tr
            v-if="activeInline?.branchName === branch.name"
            :key="`${branch.name}-inline`">
            <td :id="inline.portalTargetIdFor(branch.name)" colspan="4" class="p-0 relative overflow-hidden">
              <!-- Portal target for any inline form -->
              <!-- Progress bar overlaying the top border -->
              <div
                v-if="activeInline?.processing"
                class="absolute top-0 left-0 w-full h-px bg-accented overflow-hidden">
                <div class="absolute inset-y-0 w-1/2 bg-primary animate-[carousel_2s_ease-in-out_infinite]"/>
              </div>
            </td>
          </tr>

          <!-- Expanded row content -->
          <tr v-if="isExpanded(branch)" :key="`${branch.name}-expanded`" class="!border-t-0">
            <td colspan="4" class="px-0 py-4">
              <ArchivedBranchExpanded :branch="branch" />
            </td>
          </tr>
        </template>
      </tbody>
    </table>

    <!-- Inline delete input (renders via portal) -->
    <LazyActionsInlineDeleteConfirmationInput
      v-if="activeInline?.type === 'delete-archived' && activeInline?.branchName"
      :branch-name="activeInline?.branchName || ''"
      :dialog-title="activeInline?.branchName ? `Delete archived branch ${activeInline.branchName}` : ''"
      :dialog-description="activeInline?.branchName ? `Type the branch name to confirm deletion` : ''"
      :portal-target="activeInline?.branchName ? inline.portalTargetIdFor(activeInline.branchName) : undefined"
      :is-active="!!activeInline"
      @submit="() => confirmDelete(activeInline!.branchName)"
      @cancel="cancelDeleteInline"
    />
  </CollapsibleCard>
</template>

<script lang="ts" setup>
import { useGenericTableExpansion } from "~/composables/useGenericTableExpansion"
import ArchivedBranchesHelpPopover from "./ArchivedBranchesHelpPopover.vue"
import BranchNameCell from "./cells/BranchNameCell.vue"
import ArchivedBranchStatusBadge from "./cells/ArchivedBranchStatusBadge.vue"
import ArchivedDateCell from "./cells/ArchivedDateCell.vue"
import ArchivedBranchExpanded from "./ArchivedBranchExpanded.vue"
import TableHeader from "~/components/shared/TableHeader.vue"

const { archivedBranches } = useBranchSync()
const { getCopyMenuItems } = useBranchCopyActions()
const { selectedProject, effectiveBranchPrefix } = useRepository()
const inline = useInlineRowAction()
const { activeInline } = inline

// Use generic table expansion composable
const { isExpanded, toggleExpanded } = useGenericTableExpansion((branch: ReactiveArchivedBranch) => branch.name)

// Row processing is handled by useInlineRowAction (inline)

function getArchivedContextMenuItems(branch: ReactiveArchivedBranch) {
  const items = [] as Array<Array<{ label: string, icon: string, onSelect: () => void }>>
  // Copy actions
  items.push(getCopyMenuItems(branch.name, true))

  items.push([
    {
      label: "Delete Archived Branch",
      icon: "i-lucide-trash-2",
      onSelect: () => inline.openInline("delete-archived", branch.name),
    },
  ])
  return items
}

function cancelDeleteInline() {
  inline.closeInline()
}

const { syncBranches } = useBranchSync()

async function confirmDelete(branchName: string) {
  if (inline.isProcessing(branchName)) {
    return
  }

  const value = await inline.withRowProcessing(
    branchName,
    async () => {
      const repoPath = selectedProject.value?.path || ""
      const result = await commands.deleteArchivedBranch({
        repositoryPath: repoPath,
        branchName: branchName,
        branchPrefix: effectiveBranchPrefix.value,
      })
      if (result.status !== "ok") {
        throw new Error(result.error)
      }
      return true
    },
    {
      success: () => ({ title: "Archived branch deleted", description: `${getSimpleBranchName(branchName)} was deleted` }),
      error: e => ({ title: "Failed to delete archived branch", description: e instanceof Error ? e.message : String(e) }),
    },
  )

  if (value) {
    // noinspection ES6MissingAwait
    syncBranches({ autoScroll: false, autoExpand: false })
  }
}

// Use the reactive archived branches from the composable
const hasArchivedBranches = computed(() => archivedBranches.archivedBranches.value.length > 0)
const archivedCount = computed(() => archivedBranches.archivedBranches.value.length)

</script>