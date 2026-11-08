import { useId } from "react"
import type { ReactNode } from "react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { CopyButton } from "@/components/shared/CopyButton"
import { useSettingsScope } from "@/hooks/useSettingsScope"
import type { ConfigSetting } from "@/lib/bindings"
import type { RepositoryConfigKey, ScopedRowWords } from "@/lib/repository-config"
import { CONFIG_KEYS, describeScopedSource, everyRepositoryValue, overriddenReason } from "@/lib/repository-config"

/**
 * One `branchdeck.*` key on a page of the git config (spec/repository/settings.spec.md#scope): what it does on the left,
 * its control on the right with the value of the tab shown (`useSettingsScope`): the value of every repository, or the
 * one that holds in the repository. Under it at most one line of where the value comes from (`describeScopedSource`):
 * the repository's own value with Reset and a bar beside the row, the value it overrides with Open <repository>, why the
 * worktree config or the command line keep it; then an invalid value and typing the backend would refuse. The git key
 * is a copy button beside the label. `setting` is null until the settings answer
 */
export function ScopedConfigRow<T>({
  configKey,
  label,
  description,
  setting,
  words,
  busy,
  onReset,
  control,
  error = null,
  notApplied = null,
  focusKey,
}: {
  configKey: RepositoryConfigKey
  label: string
  description: ReactNode
  setting: ConfigSetting<T> | null
  words: ScopedRowWords<T>
  busy: boolean
  onReset: () => void
  /**
   * The control: the value of the tab, whether it can change it (`disabled`; a field only while it can't be written at
   * all, `fieldDisabled`: a write of another row must not take its focus), its label
   */
  control: (props: { value: T | null; disabled: boolean; fieldDisabled: boolean; labelId: string }) => ReactNode
  /** Typing the backend would refuse */
  error?: string | null
  /** Why the setting does not apply in the repository, as Rust says it, with the way to change that */
  notApplied?: ReactNode
  /** The `data-settings-focus` of the row, for a control that is not a field of its own */
  focusKey?: string
}) {
  const { scope, setScope, name } = useSettingsScope()
  const labelId = useId()
  const line = setting === null ? null : describeScopedSource(setting, scope, name, words)
  const shownValue = setting === null ? null : scope === "all" ? everyRepositoryValue(setting).value : setting.value
  const locked = scope === "repository" && setting !== null && overriddenReason(setting) !== null
  // the invalid value is the effective one: shown where it holds
  const invalid = setting?.invalid != null && (scope === "repository" || line?.kind !== "elsewhere") ? setting.invalid : null
  const own = scope === "repository" && setting?.localSet === true
  const key = CONFIG_KEYS[configKey]
  return (
    <div
      className={cn("group @container relative flex flex-col gap-1", own && "before:absolute before:inset-y-0.5 before:-left-3 before:w-0.5 before:rounded-full before:bg-primary")}
      data-testid={`setting-${configKey}`}
      data-own={own || undefined}
      data-settings-focus={focusKey}
      tabIndex={focusKey === undefined ? undefined : -1}
    >
      <div className="flex flex-col gap-2 @md:flex-row @md:items-start @md:justify-between @md:gap-6">
        <div className="flex min-w-0 flex-col gap-0.5">
          <div className="flex min-h-6 items-center gap-1">
            <span id={labelId} className="text-sm">
              {label}
            </span>
            <CopyButton text={() => key} tooltip={`Copy ${key}`} />
          </div>
          <p className="text-xs text-muted-foreground">{description}</p>
        </div>
        <div className="flex shrink-0 items-center gap-2 @md:min-h-6">
          {control({ value: shownValue, disabled: setting === null || busy || locked, fieldDisabled: setting === null || locked, labelId })}
        </div>
      </div>
      {line !== null && (
        <p
          className={cn("flex min-h-6 flex-wrap items-center gap-x-1 text-xs", line.kind === "locked" ? "text-warning" : "text-muted-foreground")}
          data-testid={line.kind === "locked" ? `setting-${configKey}-reason` : `setting-${configKey}-source`}
        >
          {line.text}
          {line.kind === "own" && (
            <Tooltip>
              <TooltipTrigger render={<Button variant="link" size="xs" className="px-1" disabled={busy} onClick={onReset} />}>Reset</TooltipTrigger>
              <TooltipContent>{line.reset}</TooltipContent>
            </Tooltip>
          )}
          {line.kind === "elsewhere" && (
            <Button variant="link" size="xs" className="px-1" onClick={() => setScope("repository")}>
              {`Open ${name}`}
            </Button>
          )}
        </p>
      )}
      {notApplied !== null && (
        <div className="flex flex-wrap items-center gap-x-1 text-xs text-muted-foreground" data-testid={`setting-${configKey}-not-applied`}>
          {notApplied}
        </div>
      )}
      {invalid !== null && <p className="text-xs text-warning">{invalid}</p>}
      {error !== null && (
        <p className="text-xs text-destructive" data-testid={`setting-${configKey}-typing`}>
          {error}
        </p>
      )}
    </div>
  )
}
