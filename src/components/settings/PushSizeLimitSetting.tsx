import { useState } from "react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ConfigReadError } from "@/components/settings/SettingRow"
import { useConfigWrite } from "@/hooks/useConfigWrite"
import { useRepositoryConfig } from "@/hooks/useRepositoryConfig"
import type { RepositoryConfigChange, WriteScope } from "@/lib/bindings"
import {
  DEFAULT_PUSH_SIZE_LIMIT,
  describeEveryRepositorySource,
  describeFallback,
  invalidPushSizeLimitReason,
  isValidPushSizeLimit,
  overriddenReason,
  repositoryName,
  resetLabel,
  writeErrorTitle,
} from "@/lib/repository-config"
import { onSettingFieldKey } from "@/lib/setting-field"

/**
 * "Push size limit" (`branchdeck.pushSizeLimit`, docs/design/git-operations.md "Push size"): a push that adds more files
 * is refused before it is sent. One value for every repository in the global git config, which bd reads too; a
 * repository whose `.git/config` sets the key keeps its own value: the field changes it there, and "Use Global" or "Use
 * Default" removes it. The worktree config and the command line win over both, so the field is disabled then, with
 * that reason. Saved on Enter and when the field is left, never on a key; Escape takes the typing back, which the
 * backend's rule checks in place (`isValidPushSizeLimit`).
 */
export function PushSizeLimitSetting({ path }: { path: string }) {
  const { state, retry, startWrite } = useRepositoryConfig(path)
  const config = useConfigWrite(path, startWrite)
  const [draft, setDraft] = useState<string | null>(null)
  const setting = state.status === "loaded" ? state.config.pushSizeLimit : null
  const name = repositoryName(path)
  const overridden = setting === null ? null : overriddenReason(setting)
  const typed = draft?.trim() ?? null
  const reason = typed === null || typed === "" || isValidPushSizeLimit(typed) ? null : invalidPushSizeLimitReason(typed)

  const write = (value: string | null, scope: WriteScope): Promise<boolean> => {
    const change: RepositoryConfigChange = { key: "pushSizeLimit", value }
    return config.write(change, scope, scope === "global" ? "Cannot change the push size limit of every repository" : writeErrorTitle(change, false, path))
  }

  /** Writes valid typing that changes the value where it comes from: `.git/config` when it sets the key, else the global config */
  const save = () => {
    if (setting === null || typed === null || reason !== null || config.writing) {
      return
    }
    if (typed === (setting.value ?? "")) {
      setDraft(null)
      return
    }
    void write(typed === "" ? null : typed, setting.localSet ? "local" : "global").then(written => {
      if (written) {
        setDraft(null)
      }
    })
  }

  return (
    <div className="flex flex-col gap-1" data-testid="setting-pushSizeLimit">
      <div className="flex min-h-7 items-center gap-3">
        <span className="w-36 shrink-0 text-sm">Push size limit</span>
        <Input
          value={draft ?? setting?.value ?? ""}
          onChange={event => setDraft(event.target.value)}
          onBlur={save}
          onKeyDown={event => onSettingFieldKey(event, save, draft === null ? null : () => setDraft(null))}
          placeholder={DEFAULT_PUSH_SIZE_LIMIT}
          aria-label="Push size limit"
          aria-invalid={reason !== null || undefined}
          disabled={setting === null || overridden !== null}
          className="h-7 w-32"
        />
      </div>
      <div className="flex flex-col gap-0.5 text-xs text-muted-foreground">
        {state.status === "error" && <ConfigReadError testId="setting-pushSizeLimit-error" message={state.message} onRetry={retry} />}
        {setting !== null && overridden === null && (
          <p className="flex items-center gap-2" data-testid="setting-pushSizeLimit-source">
            {describeEveryRepositorySource(setting, name, DEFAULT_PUSH_SIZE_LIMIT)}
            {setting.localSet && (
              <Tooltip>
                <TooltipTrigger render={<Button variant="ghost" size="xs" disabled={config.writing} onClick={() => void write(null, "local")} />}>
                  {resetLabel(setting)}
                </TooltipTrigger>
                <TooltipContent>{`Removes it from .git/config: back to ${describeFallback(setting, setting.fallbackValue ?? "", `${DEFAULT_PUSH_SIZE_LIMIT}, the default`)}`}</TooltipContent>
              </Tooltip>
            )}
          </p>
        )}
        {overridden !== null && (
          <p className="text-warning" data-testid="setting-pushSizeLimit-reason">
            {overridden}
          </p>
        )}
        {setting?.invalid != null && <p className="text-warning">{setting.invalid}</p>}
        {reason !== null && (
          <p className="text-destructive" data-testid="setting-pushSizeLimit-typing">
            {reason}
          </p>
        )}
      </div>
    </div>
  )
}
