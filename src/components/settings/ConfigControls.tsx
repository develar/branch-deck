import type { ReactNode } from "react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Switch } from "@/components/ui/switch"
import { FieldGroup } from "@/components/layout/FieldGroup"
import { ReadFailure } from "@/components/settings/SettingRow"
import { LoadingLine } from "@/components/shared/LoadingLine"
import type { ConfigDraft } from "@/hooks/useConfigDraft"
import type { ScopedConfig } from "@/hooks/useScopedConfig"
import { useScopedConfig } from "@/hooks/useScopedConfig"
import { useSettingsScope } from "@/hooks/useSettingsScope"
import type { RepositoryConfig } from "@/lib/bindings"

/**
 * The sections of a page of the git config over one read of the settings of its tab (`useScopedConfig`): "Reading the
 * settings…" until they answer, a failed read with Retry; the rows get null until then, their controls disabled
 */
export function ScopedConfigSections({ children }: { children: (config: RepositoryConfig | null, scoped: ScopedConfig) => ReactNode }) {
  const scoped = useScopedConfig()
  const { name } = useSettingsScope()
  const state = scoped.read.state
  return (
    <>
      {state.status === "loading" && <LoadingLine>Reading the settings…</LoadingLine>}
      {state.status === "error" && (
        <ReadFailure
          title={name === "" ? "Cannot read your global Git config" : `Cannot read the settings of ${name}`}
          message={state.message}
          testId="settings-config-error"
          onRetry={scoped.read.retry}
        />
      )}
      {children(state.status === "loaded" ? state.config : null, scoped)}
    </>
  )
}

/** The switch of a boolean key, named by the label of its row */
export function ConfigSwitch({ value, disabled, labelId, onChange }: { value: boolean | null; disabled: boolean; labelId: string; onChange: (value: boolean) => void }) {
  return <Switch size="sm" checked={value ?? false} disabled={disabled} aria-labelledby={labelId} onCheckedChange={onChange} />
}

/**
 * The buttons of a key of a few values (`aria-pressed`), the value of the tab pressed; a click on the pressed one names
 * it in the config when `namesPressed` (the default of a route pressed while unset)
 */
export function ConfigChoices<T extends string>({
  choices,
  value,
  disabled,
  labelId,
  namesPressed = false,
  onChange,
}: {
  choices: ReadonlyArray<{ value: T; label: string }>
  value: T | null
  disabled: boolean
  labelId: string
  namesPressed?: boolean
  onChange: (value: T) => void
}) {
  return (
    <FieldGroup role="group" aria-labelledby={labelId}>
      {choices.map(choice => {
        const pressed = value === choice.value
        return (
          <Button
            key={choice.value}
            size="xs"
            variant="outline"
            aria-pressed={pressed}
            className={cn(pressed && "border-primary/50 text-primary hover:text-primary")}
            disabled={disabled}
            onClick={() => {
              if (!pressed || namesPressed) {
                onChange(choice.value)
              }
            }}
          >
            {choice.label}
          </Button>
        )
      })}
    </FieldGroup>
  )
}

/** The field of a text key, with the typing of `draft` (`useConfigDraft`) */
export function ConfigTextField({
  draft,
  label,
  placeholder,
  disabled,
  focusKey,
  className,
}: {
  draft: ConfigDraft
  label: string
  placeholder: string
  disabled: boolean
  focusKey?: string
  className?: string
}) {
  return (
    <Input
      value={draft.value}
      onChange={event => draft.onChange(event.target.value)}
      onBlur={draft.onBlur}
      onKeyDown={draft.onKeyDown}
      placeholder={placeholder}
      aria-label={label}
      aria-invalid={draft.error !== null || undefined}
      disabled={disabled}
      data-settings-focus={focusKey}
      className={cn("h-7 w-56", className)}
    />
  )
}
