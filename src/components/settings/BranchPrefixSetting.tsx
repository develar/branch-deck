import { useEffect, useState } from "react"
import { CircleHelpIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { BranchPrefixHelpDialog } from "@/components/repository/BranchPrefixHelp"
import { InlineCode } from "@/components/shared/InlineCode"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import { commands } from "@/lib/bindings"
import { isValidBranchPrefix, normalizeBranchPrefix } from "@/lib/branch-prefix"
import { openSettings } from "@/lib/settings-window"
import { computeEffectiveBranchPrefix, repositoryOwnBranchPrefix } from "@/stores/repository"
import type { BranchPrefixResult } from "@/stores/repository"
import { setGlobalUserBranchPrefix, useAppSettings } from "@/stores/settings/appSettings"
import { useRecentProjects } from "@/stores/settings/repositorySettings"

/** Until this window's read of git answers */
const NOT_READ: BranchPrefixResult = { status: "error", error: { message: "Not read" } }

/**
 * The branch prefix of every repository (`globalUserBranchPrefix`), saved on Enter and when the field loses the focus,
 * not on every key: the prefix names the branches of the sync and the robots of Patronus. Escape takes the typing back.
 * Empty, the prefix of git (`branchdeck.branchPrefix`) applies, its value the placeholder. The selected repository's own
 * git prefix wins over it (`computeEffectiveBranchPrefix`), which the field says, with the way to its page. A sync reads
 * the prefix when it starts, so a change during one names the branches of the next. Reads only what every window has:
 * the settings and git, not the stores of the main window.
 */
export function BranchPrefixSetting() {
  const saved = useAppSettings(state => state.globalUserBranchPrefix) ?? ""
  const path = useRecentProjects(projects => projects[0]?.path)
  const cachedPrefix = useRecentProjects(projects => projects[0]?.cachedBranchPrefix)
  const gitResult = useGitBranchPrefix(path)
  const gitPrefix = gitResult?.status === "ok" ? gitResult.data.prefix : ""
  // the read of this window, else the prefix the main window cached for the repository
  const repositoryPrefix = path === undefined ? "" : repositoryOwnBranchPrefix(gitResult?.status === "ok" ? gitResult.data : cachedPrefix)
  const [draft, setDraft] = useState<string | null>(null)
  const [helpOpen, setHelpOpen] = useState(false)
  const value = draft ?? saved
  const prefix = normalizeBranchPrefix(value)
  const invalid = prefix !== "" && !isValidBranchPrefix(prefix)
  // the names the prefix gives; the repository's own prefix has a line of its own. With one, an empty field names the
  // branches of the other repositories by the global config, which this read doesn't see: no line
  const named = repositoryPrefix === "" ? computeEffectiveBranchPrefix(prefix, cachedPrefix, gitResult ?? NOT_READ) : prefix
  const showsNames = repositoryPrefix === "" || prefix !== ""

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
          placeholder={(repositoryPrefix === "" && gitPrefix) || "your-name"}
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
        showsNames && (
          <p className="text-xs text-muted-foreground">
            {"Branches are named "}
            <InlineCode>{`${named || "<prefix>"}/virtual/<name>`}</InlineCode>
          </p>
        )
      )}
      {repositoryPrefix !== "" && (
        <p className="flex flex-wrap items-center gap-x-1 text-xs text-muted-foreground" data-testid="setting-branch-prefix-repository">
          <span>
            {"This repository uses its own prefix "}
            <InlineCode>{repositoryPrefix}</InlineCode>
          </span>
          <Button variant="link" size="xs" className="px-1" onClick={() => openSettings({ page: "repository", focus: "branchPrefix" })}>
            Open Repository Settings
          </Button>
        </p>
      )}
      <BranchPrefixHelpDialog open={helpOpen} onOpenChange={setHelpOpen} inSettings />
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
        setRead({ path, result })
      }
    })
    return () => {
      current = false
    }
  }, [path, focusCount])
  return read !== null && read.path === path ? read.result : null
}
