import { useId } from "react"
import type { ReactNode } from "react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { InlineCode } from "@/components/shared/InlineCode"
import type { ConfigSetting } from "@/lib/bindings"
import type { RepositoryConfigKey } from "@/lib/repository-config"
import { CONFIG_KEYS, describeSource, overriddenReason, resetLabel } from "@/lib/repository-config"

/**
 * One setting: its label and control, "Use Default" or "Use Global" while the local config sets it, what it does with
 * its git key, where its value comes from (for the default only where the row says it), why it can't be changed, what
 * is wrong with it
 */
export function ConfigRow({
  configKey,
  label,
  setting,
  fallback,
  busy,
  onReset,
  description,
  unsetSource = null,
  reason = null,
  error = null,
  control,
}: {
  configKey: RepositoryConfigKey
  label: string
  setting: ConfigSetting<unknown>
  /** What unsetting the local value gives (`describeFallback`), for the tooltip of the reset */
  fallback: string
  busy: boolean
  onReset: () => void
  description: string
  /** Where the value comes from while no config sets it: the detected GitHub repository, the prefix of General */
  unsetSource?: string | null
  /** Why the control is disabled, besides the worktree config or the command line */
  reason?: string | null
  /** The typing the backend would refuse */
  error?: string | null
  control: (labelId: string) => ReactNode
}) {
  const labelId = useId()
  const overridden = overriddenReason(setting)
  // a value the page can't change says where it comes from in its reason
  const source = overridden === null ? (describeSource(setting) ?? unsetSource) : null
  const shownReason = overridden ?? reason
  return (
    <div className="flex flex-col gap-1" data-testid={`repository-setting-${configKey}`}>
      <div className="flex min-h-7 items-center gap-3">
        <span id={labelId} className="w-36 shrink-0 text-sm">
          {label}
        </span>
        <div className="flex min-w-0 flex-1 items-center gap-2">
          {control(labelId)}
          {setting.localSet && (
            <Tooltip>
              <TooltipTrigger render={<Button variant="ghost" size="xs" disabled={overridden !== null || busy} onClick={onReset} />}>{resetLabel(setting)}</TooltipTrigger>
              <TooltipContent>{`Removes it from .git/config: back to ${fallback}`}</TooltipContent>
            </Tooltip>
          )}
        </div>
      </div>
      <div className="flex flex-col gap-0.5 text-xs text-muted-foreground">
        <p>
          {`${description} · `}
          <InlineCode>{CONFIG_KEYS[configKey]}</InlineCode>
        </p>
        {source !== null && <p data-testid="repository-setting-source">{source}</p>}
        {setting.invalid !== null && <p className="text-warning">{setting.invalid}</p>}
        {shownReason !== null && (
          <p className="text-warning" data-testid="repository-setting-reason">
            {shownReason}
          </p>
        )}
        {error !== null && (
          <p className="text-destructive" data-testid="repository-setting-error">
            {error}
          </p>
        )}
      </div>
    </div>
  )
}
