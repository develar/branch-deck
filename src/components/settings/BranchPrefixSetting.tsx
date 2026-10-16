import { useState } from "react"
import { CircleHelpIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { BranchPrefixHelpDialog } from "@/components/repository/BranchPrefixHelp"
import { InlineCode } from "@/components/shared/InlineCode"
import { isValidBranchPrefix, normalizeBranchPrefix } from "@/lib/branch-prefix"
import { useBranchSync } from "@/stores/branch-sync"
import { useEffectiveBranchPrefix, useRepository } from "@/stores/repository"
import { setGlobalUserBranchPrefix, useAppSettings } from "@/stores/settings/appSettings"

/**
 * The branch prefix of every repository (`globalUserBranchPrefix`), saved on Enter and when the field loses the focus,
 * not on every key: the prefix names the branches of the sync and the robots of Patronus. Escape takes the typing back.
 * Empty, the prefix of git (`branchdeck.branchPrefix`) applies, its value the placeholder.
 */
export function BranchPrefixSetting() {
  const saved = useAppSettings(state => state.globalUserBranchPrefix) ?? ""
  const gitPrefix = useRepository(state => (state.gitProvidedBranchPrefix.status === "ok" ? state.gitProvidedBranchPrefix.data : null))
  const effective = useEffectiveBranchPrefix()
  const isSyncing = useBranchSync(state => state.isSyncing)
  const [draft, setDraft] = useState<string | null>(null)
  const [helpOpen, setHelpOpen] = useState(false)
  const value = draft ?? saved
  const prefix = normalizeBranchPrefix(value)
  const invalid = prefix !== "" && !isValidBranchPrefix(prefix)

  const save = () => {
    if (draft !== null && !invalid) {
      setGlobalUserBranchPrefix(prefix === "" ? undefined : prefix)
      setDraft(null)
    }
  }

  return (
    <div className="flex flex-col gap-1.5" data-testid="setting-branch-prefix">
      <div className="flex items-center gap-2">
        <Input
          value={value}
          onChange={event => setDraft(event.target.value)}
          onBlur={save}
          onKeyDown={event => {
            if (event.key === "Enter") {
              event.preventDefault()
              save()
            } else if (event.key === "Escape" && draft !== null) {
              // the dialog keeps open: Escape takes the typing back first
              event.preventDefault()
              event.stopPropagation()
              setDraft(null)
            }
          }}
          disabled={isSyncing}
          placeholder={gitPrefix || "your-name"}
          aria-label="Branch prefix"
          aria-invalid={invalid || undefined}
          className="h-8 w-48"
        />
        <Button variant="link" size="xs" onClick={() => setHelpOpen(true)}>
          <CircleHelpIcon data-icon="inline-start" />
          View Guide
        </Button>
      </div>
      {invalid ? (
        <p className="text-xs text-destructive">A prefix cannot contain spaces, "..", or any of ~ ^ : ? * [ \</p>
      ) : (
        <p className="text-xs text-muted-foreground">
          {"Branches are named "}
          <InlineCode>{`${prefix || effective || "<prefix>"}/virtual/<name>`}</InlineCode>
          {isSyncing ? " · it can change once the sync finished" : ""}
        </p>
      )}
      <BranchPrefixHelpDialog open={helpOpen} onOpenChange={setHelpOpen} />
    </div>
  )
}
