import { useEffect, useState } from "react"
import { CircleHelpIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { BranchPrefixHelpDialog } from "@/components/repository/BranchPrefixHelp"
import { InlineCode } from "@/components/shared/InlineCode"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import { commands } from "@/lib/bindings"
import { isValidBranchPrefix, normalizeBranchPrefix } from "@/lib/branch-prefix"
import { computeEffectiveBranchPrefix } from "@/stores/repository"
import type { BranchPrefixResult } from "@/stores/repository"
import { setGlobalUserBranchPrefix, useAppSettings } from "@/stores/settings/appSettings"
import { useRecentProjects } from "@/stores/settings/repositorySettings"

/**
 * The branch prefix of every repository (`globalUserBranchPrefix`), saved on Enter and when the field loses the focus,
 * not on every key: the prefix names the branches of the sync and the robots of Patronus. Escape takes the typing back.
 * Empty, the prefix of git (`branchdeck.branchPrefix`) applies, its value the placeholder. A sync reads the prefix when
 * it starts, so a change during one names the branches of the next. Reads only what every window has: the settings
 * and git, not the stores of the main window.
 */
export function BranchPrefixSetting() {
  const saved = useAppSettings(state => state.globalUserBranchPrefix) ?? ""
  const path = useRecentProjects(projects => projects[0]?.path)
  const cachedPrefix = useRecentProjects(projects => projects[0]?.cachedBranchPrefix)
  const gitResult = useGitBranchPrefix(path)
  const gitPrefix = gitResult?.status === "ok" ? gitResult.data : ""
  const [draft, setDraft] = useState<string | null>(null)
  const [helpOpen, setHelpOpen] = useState(false)
  const value = draft ?? saved
  const prefix = normalizeBranchPrefix(value)
  const invalid = prefix !== "" && !isValidBranchPrefix(prefix)
  const effective = gitResult === null ? prefix : computeEffectiveBranchPrefix(prefix, cachedPrefix, gitResult)

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
              // the window stays open: Escape takes the typing back first
              event.preventDefault()
              event.stopPropagation()
              setDraft(null)
            }
          }}
          placeholder={gitPrefix || "your-name"}
          aria-label="Branch prefix"
          aria-invalid={invalid || undefined}
          data-settings-focus="branchPrefix"
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
          <InlineCode>{`${effective || "<prefix>"}/virtual/<name>`}</InlineCode>
        </p>
      )}
      <BranchPrefixHelpDialog open={helpOpen} onOpenChange={setHelpOpen} />
    </div>
  )
}

/**
 * The prefix of git (`branchdeck.branchPrefix`) for the repository at `path`, or of the global config without one;
 * read again when the window gets the focus (the user may have run `git config` meanwhile). Null until it answers.
 */
function useGitBranchPrefix(path: string | undefined): BranchPrefixResult | null {
  const [read, setRead] = useState<{ path: string | undefined; result: BranchPrefixResult } | null>(null)
  const [focusCount, setFocusCount] = useState(0)
  useWindowFocus(() => setFocusCount(count => count + 1))
  useEffect(() => {
    let current = true
    void commands.getBranchPrefixFromGitConfig({ repositoryPath: path ?? "" }).then(result => {
      if (current) {
        setRead({ path, result: result.status === "ok" ? { status: "ok", data: result.data.prefix } : result })
      }
    })
    return () => {
      current = false
    }
  }, [path, focusCount])
  return read !== null && read.path === path ? read.result : null
}
