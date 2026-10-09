import { PenLineIcon } from "lucide-react"
import { InlineForm } from "@/components/shared/InlineForm"
import { UncommittedChangesCard } from "@/components/branch/UncommittedChangesCard"
import { useAmendChanges } from "@/hooks/useAmendChanges"
import { useBranch } from "@/stores/branch-sync"
import { closeInline, useIsRowProcessing } from "@/stores/inline-row-action"

export function InlineAmendChangesInput({ branchName }: { branchName: string }) {
  const branch = useBranch(branchName)
  const processing = useIsRowProcessing(branchName)
  const { diffData, diffLoading, diffError, amendChanges } = useAmendChanges()

  const canAmend = !diffLoading && !diffError && diffData?.hasChanges === true && !processing

  return (
    <InlineForm
      title={`Amend Changes to ${branchName}`}
      description={`Amend uncommitted changes to the tip commit of ${branchName} branch`}
      canSubmit={canAmend}
      loading={processing}
      submitText="Amend Changes"
      submitIcon={<PenLineIcon data-icon="inline-start" />}
      onCancel={closeInline}
      onSubmit={() => {
        if (branch !== undefined) {
          void amendChanges(branch)
        }
      }}
    >
      <UncommittedChangesCard diffData={diffData} loading={diffLoading} error={diffError} />
    </InlineForm>
  )
}
