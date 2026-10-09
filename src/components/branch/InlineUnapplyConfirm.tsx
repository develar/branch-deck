import { ArchiveXIcon, TriangleAlertIcon } from "lucide-react"
import { InlineForm } from "@/components/shared/InlineForm"
import { unapplyBranch } from "@/hooks/useUnapplyBranch"
import { useBranch } from "@/stores/branch-sync"
import { closeInline, useIsRowProcessing } from "@/stores/inline-row-action"

export function InlineUnapplyConfirm({ branchName }: { branchName: string }) {
  const branch = useBranch(branchName)
  const processing = useIsRowProcessing(branchName)
  const commitCount = branch?.commits.length ?? 0
  const canUnapply = branch !== undefined && commitCount > 0 && !processing

  return (
    <InlineForm
      title={`Unapply ${branchName}`}
      description="Remove virtual branch commits from HEAD and move branch to unapplied state"
      canSubmit={canUnapply}
      loading={processing}
      submitText="Unapply Branch"
      submitIcon={<ArchiveXIcon data-icon="inline-start" />}
      onCancel={closeInline}
      onSubmit={() => {
        if (branch !== undefined) {
          void unapplyBranch(branch)
        }
      }}
    >
      <div className="flex items-start gap-3 rounded-lg border border-warning/20 bg-warning/10 p-4">
        <TriangleAlertIcon className="mt-0.5 size-5 shrink-0 text-warning" />
        <div className="flex flex-col gap-2 text-sm">
          <p className="font-medium text-warning">
            {`This will remove ${commitCount} commit${commitCount === 1 ? "" : "s"} from HEAD`}
          </p>
          <p className="text-muted-foreground">
            The commits will be preserved in the unapplied branch, but removed from your current working branch.
            This action can be undone by reapplying the commits later.
          </p>
        </div>
      </div>
    </InlineForm>
  )
}
