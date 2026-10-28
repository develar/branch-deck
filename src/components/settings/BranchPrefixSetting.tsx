import { useEffect, useState } from "react"
import { CircleHelpIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { BranchPrefixHelpDialog } from "@/components/repository/BranchPrefixHelp"
import { InlineCode } from "@/components/shared/InlineCode"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import { commands } from "@/lib/bindings"
import type { ResolvedBranchPrefix } from "@/lib/bindings"
import { appPrefixOf, INVALID_PREFIX_TEXT, isValidBranchPrefix, normalizeBranchPrefix } from "@/lib/branch-prefix"
import { onSettingFieldKey } from "@/lib/setting-field"
import { openSettings } from "@/lib/settings-window"
import { setGlobalUserBranchPrefix, useAppSettings } from "@/stores/settings/appSettings"
import { onWriterChange, snapshotValueOf } from "@/stores/settings/persistent-store"
import { useRecentProjects } from "@/stores/settings/repositorySettings"

/**
 * The branch prefix of every repository (`globalUserBranchPrefix`), saved on Enter and when the field loses the focus,
 * not on every key: the prefix names the branches of the sync and the robots of Patronus. Escape takes the typing back.
 * Empty, the prefix of the global git config (`branchdeck.branchPrefix`) applies, its value the placeholder. The selected
 * repository's own git prefix wins over it, which the field says, with the way to its page: what Rust resolved for the
 * repository (`resolve_branch_prefix`) and its source, shown as they are. A sync resolves the prefix when it starts, so a
 * change during one names the branches of the next. Reads only what every window has: the settings and Rust, not the
 * stores of the main window.
 */
export function BranchPrefixSetting() {
  const saved = useAppSettings(state => state.globalUserBranchPrefix) ?? ""
  const path = useRecentProjects(projects => projects[0]?.path)
  // the answer of this window: its source names the repository's own prefix and the global one
  const resolved = useResolvedBranchPrefixOf(path)
  const repositoryPrefix = resolved?.source === "repository_config" ? resolved.prefix : ""
  // the global git prefix is known while it applies, that is while the app sets none
  const globalPrefix = resolved?.source === "global_config" ? resolved.prefix : ""
  const [draft, setDraft] = useState<string | null>(null)
  const [helpOpen, setHelpOpen] = useState(false)
  const value = draft ?? saved
  const prefix = normalizeBranchPrefix(value)
  const invalid = prefix !== "" && !isValidBranchPrefix(prefix)
  // the names the prefix gives; the repository's own prefix has a line of its own. With one, an empty field names the
  // branches of the other repositories by the global config, which this answer doesn't name: no line
  const named = prefix || (repositoryPrefix === "" ? globalPrefix : "")
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
          onKeyDown={event => onSettingFieldKey(event, save, draft === null ? null : () => setDraft(null))}
          placeholder={globalPrefix || "your-name"}
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
        <p className="text-xs text-destructive">{INVALID_PREFIX_TEXT}</p>
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
 * The prefix Rust resolves for the repository at `path` (`resolve_branch_prefix`), or with the global git config
 * alone without one; asked again when the window gets the focus (the user may have run `git config` meanwhile) and once
 * the writer has another app prefix. Null until it answers, or when the repository cannot be read.
 */
function useResolvedBranchPrefixOf(path: string | undefined): ResolvedBranchPrefix | null {
  const [read, setRead] = useState<{ path: string | undefined; prefix: ResolvedBranchPrefix | null } | null>(null)
  const [askCount, setAskCount] = useState(0)
  useWindowFocus(() => setAskCount(count => count + 1))
  useEffect(() => {
    let appPrefix: unknown = snapshotValueOf("appSettings")
    return onWriterChange("appSettings", value => {
      const prefix = appPrefixOf(value)
      if (prefix !== appPrefixOf(appPrefix)) {
        setAskCount(count => count + 1)
      }
      appPrefix = value
    })
  }, [])
  useEffect(() => {
    let current = true
    void commands.resolveBranchPrefix({ repositoryPath: path ?? "" }).then(result => {
      if (current) {
        setRead({ path, prefix: result.status === "ok" ? result.data : null })
      }
    })
    return () => {
      current = false
    }
  }, [path, askCount])
  return read !== null && read.path === path ? read.prefix : null
}
