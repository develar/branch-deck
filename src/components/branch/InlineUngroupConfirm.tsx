import { UngroupIcon } from "lucide-react"
import { Callout } from "@/components/shared/Callout"
import { InlineForm } from "@/components/shared/InlineForm"
import { moveCommits } from "@/hooks/useMoveCommits"
import { getStackChildren, useBranch } from "@/stores/branch-sync"
import { closeInline, useIsRowProcessing, withRowProgress } from "@/stores/inline-row-action"
import { StackChildrenNote } from "@/components/branch/StackChildrenNote"

/** Confirms ungrouping a branch that other branches stack on: they keep naming it as their parent and are blocked */
export function InlineUngroupConfirm({ branchName }: { branchName: string }) {
  const branch = useBranch(branchName)
  const processing = useIsRowProcessing(branchName)
  const children = getStackChildren(branchName)

  return (
    <InlineForm
      title={`Ungroup ${branchName}`}
      description="Removes the prefix of its commits: they go back to their issue branch or to the unassigned commits."
      canSubmit={branch !== undefined && !processing}
      loading={processing}
      submitText="Ungroup"
      submitIcon={<UngroupIcon data-icon="inline-start" />}
      focusSubmit
      onCancel={closeInline}
      onSubmit={() => {
        if (branch !== undefined) {
          void withRowProgress(branchName, () => moveCommits(branch.commits, null), { processingMessage: `Ungrouping ${branchName}…`, keepFormOnFailure: true })
        }
      }}
    >
      <Callout tone="warning">
        <StackChildrenNote parent={branchName} children={children} />
      </Callout>
    </InlineForm>
  )
}
