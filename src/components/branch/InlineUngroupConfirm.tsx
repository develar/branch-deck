import { TriangleAlertIcon, UngroupIcon } from "lucide-react"
import { InlineForm } from "@/components/shared/InlineForm"
import { moveCommits } from "@/hooks/useMoveCommits"
import { getStackChildren, useBranch } from "@/stores/branch-sync"
import { closeInline, useIsRowProcessing, withPostSubmit } from "@/stores/inline-row-action"
import { StackChildrenNote } from "@/components/branch/StackChildrenNote"

/** Confirms ungrouping a branch that other branches stack on: they keep naming it as their parent and are blocked */
export function InlineUngroupConfirm({ branchName }: { branchName: string }) {
  const branch = useBranch(branchName)
  const processing = useIsRowProcessing(branchName)
  const children = getStackChildren(branchName)

  return (
    <InlineForm
      title={`Ungroup ${branchName}`}
      description="Remove the prefix of the branch's commits, which makes them unassigned"
      canSubmit={branch !== undefined && !processing}
      loading={processing}
      submitText="Ungroup"
      submitIcon={<UngroupIcon data-icon="inline-start" />}
      onCancel={closeInline}
      onSubmit={() => {
        withPostSubmit(async () => {
          if (branch !== undefined) {
            await moveCommits(branch.commits, null)
          }
        })
      }}
    >
      <div className="flex items-start gap-3 rounded-lg border border-warning/20 bg-warning/10 p-4">
        <TriangleAlertIcon className="mt-0.5 size-5 shrink-0 text-warning" />
        <StackChildrenNote parent={branchName} children={children} />
      </div>
    </InlineForm>
  )
}
