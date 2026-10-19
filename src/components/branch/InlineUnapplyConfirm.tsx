import { ArchiveXIcon } from "lucide-react"
import { StackChildrenNote } from "@/components/branch/StackChildrenNote"
import { Callout } from "@/components/shared/Callout"
import { InlineForm } from "@/components/shared/InlineForm"
import { unapplyBranch } from "@/hooks/useUnapplyBranch"
import { getStackChildren, useBranch } from "@/stores/branch-sync"
import { closeInline, useInlineProcessing } from "@/stores/inline-row-action"

export function InlineUnapplyConfirm({ branchName }: { branchName: string }) {
  const branch = useBranch(branchName)
  const processing = useInlineProcessing(branchName) !== null
  const commitCount = branch?.commits.length ?? 0
  const canUnapply = branch !== undefined && commitCount > 0 && !processing
  const children = getStackChildren(branchName)

  return (
    <InlineForm
      title={`Unapply ${branchName}`}
      canSubmit={canUnapply}
      loading={processing}
      submitText="Unapply Branch"
      submitIcon={<ArchiveXIcon data-icon="inline-start" />}
      focusSubmit
      onCancel={closeInline}
      onSubmit={() => {
        if (branch !== undefined) {
          void unapplyBranch(branch)
        }
      }}
    >
      <Callout tone="warning" title={`This will remove ${commitCount} commit${commitCount === 1 ? "" : "s"} from HEAD`}>
        <p>The commits will be preserved in the unapplied branch, but removed from your current working branch. This action can be undone by reapplying the commits later.</p>
        {children.length > 0 && <StackChildrenNote parent={branchName} stacked={children} />}
      </Callout>
    </InlineForm>
  )
}
