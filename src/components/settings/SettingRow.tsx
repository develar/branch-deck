import { useId } from "react"
import type { ReactNode } from "react"
import { Switch } from "@/components/ui/switch"
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
  description?: string
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
