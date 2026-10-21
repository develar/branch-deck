import { ArchiveXIcon } from "lucide-react"
import { StackChildrenNote } from "@/components/branch/StackChildrenNote"
import { Callout } from "@/components/shared/Callout"
import { InlineForm } from "@/components/shared/InlineForm"
import { unapplyBranch } from "@/hooks/useUnapplyBranch"
import { formatCommitCount } from "@/lib/commit"
import { getStackChildren, useBranch } from "@/stores/branch-sync"
import { closeInline, inlineSurface, useInlineProcessing, withRowProgress } from "@/stores/inline-row-action"

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
          void withRowProgress(branchName, () => unapplyBranch(branch, { surface: inlineSurface(branchName) }))
        }
      }}
    >
      <Callout tone="warning" title={`This removes ${formatCommitCount(commitCount)} and their changes from HEAD`}>
        <p>The files they changed go back to how they are without them; uncommitted changes to those files stop the unapply. The branch is kept as an unapplied branch.</p>
        {children.length > 0 && <StackChildrenNote parent={branchName} stacked={children} />}
      </Callout>
    </InlineForm>
  )
}
