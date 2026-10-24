import { useId } from "react"
import type { ReactNode } from "react"
import { RotateCwIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Switch } from "@/components/ui/switch"
import { Callout } from "@/components/shared/Callout"
import type { AppSettingsSwitch } from "@/stores/settings/appSettings"
import { setAppSettingsSwitch, useAppSettings } from "@/stores/settings/appSettings"

/** A group of settings on a page of Settings: its title, what the group is about, the settings */
export function SettingsSection({ title, description, children }: { title: string; description: string; children: ReactNode }) {
  const titleId = useId()
  return (
    <section aria-labelledby={titleId} className="flex flex-col gap-3">
      <div className="flex flex-col gap-0.5">
        <h3 id={titleId} className="text-sm font-medium">
          {title}
        </h3>
        <p className="text-xs text-muted-foreground">{description}</p>
      </div>
      {children}
    </section>
  )
}

/** A switch with its label and what it does; the whole row toggles it */
export function SettingRow({
  testId,
  label,
  description,
  checked,
  disabled,
  onCheckedChange,
}: {
  testId: string
  label: string
  description?: ReactNode
  checked: boolean
  disabled?: boolean
  onCheckedChange: (checked: boolean) => void
}) {
  return (
    <label className="flex items-start gap-3 text-sm" data-testid={testId}>
      <Switch size="sm" className="mt-0.5" checked={checked} disabled={disabled} onCheckedChange={onCheckedChange} />
      <span className="flex flex-col gap-0.5">
        {label}
        {description !== undefined && <span className="text-xs text-muted-foreground">{description}</span>}
      </span>
    </label>
  )
}

/** A switch of `appSettings` */
export function SettingSwitch({ name, label, description }: { name: AppSettingsSwitch; label: string; description?: string }) {
  const checked = useAppSettings(state => state[name])
  return <SettingRow testId={`setting-${name}`} label={label} description={description} checked={checked} onCheckedChange={value => setAppSettingsSwitch(name, value)} />
}

/** A failed read of what a page shows (the settings of a repository, a sign-in, the tools), with Retry */
export function ReadFailure({ title, message, testId, onRetry }: { title: string; message: string; testId: string; onRetry: () => void }) {
  return (
    <Callout tone="destructive" title={title} data-testid={testId}>
      <p>{message}</p>
      <div className="flex">
        <Button size="xs" variant="outline" onClick={onRetry}>
          <RotateCwIcon data-icon="inline-start" />
          Retry
        </Button>
      </div>
    </Callout>
  )
}

/** A failed read of the git config under a setting, with Retry */
export function ConfigReadError({ testId, message, onRetry }: { testId: string; message: string; onRetry: () => void }) {
  return (
    <p className="flex items-center gap-2 text-destructive" data-testid={testId}>
      {`Cannot read the git config: ${message}`}
      <Button size="xs" variant="outline" onClick={onRetry}>
        <RotateCwIcon data-icon="inline-start" />
        Retry
      </Button>
    </p>
  )
}
