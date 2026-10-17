import { useEffect, useEffectEvent, useId, useRef, useState } from "react"
import type { ReactNode } from "react"
import type { UnlistenFn } from "@tauri-apps/api/event"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { cn } from "cn"
import { RotateCwIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Switch } from "@/components/ui/switch"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { FieldGroup } from "@/components/layout/FieldGroup"
import { Callout } from "@/components/shared/Callout"
import { InlineCode } from "@/components/shared/InlineCode"
import { LoadingLine } from "@/components/shared/LoadingLine"
import { useRepositoryConfig } from "@/hooks/useRepositoryConfig"
import type { ConfigSetting, RepositoryConfig, RepositoryConfigChange } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { isValidBranchPrefix, normalizeBranchPrefix } from "@/lib/branch-prefix"
import { logError, notifyError } from "@/lib/errors"
import {
  CONFIG_KEYS,
  SYMBOL_MODES,
  describeFallback,
  describeSource,
  invalidBranchPrefixReason,
  invalidGitHubRepositoryReason,
  isValidGitHubRepository,
  overriddenReason,
  repositoryName,
  resetLabel,
  stacksOffReason,
  symbolModeLabel,
  writeErrorTitle,
} from "@/lib/repository-config"
import type { RepositoryConfigKey } from "@/lib/repository-config"
import { getSelectedProject, useSelectedProjectPath } from "@/stores/repository"
import { useAppSettings } from "@/stores/settings/appSettings"

/** The settings typed into a field, saved on Enter, when the field is left and when the window closes */
type TextKey = "githubRepository" | "branchPrefix"

const TEXT_KEYS: readonly TextKey[] = ["githubRepository", "branchPrefix"]

function withoutDraft(drafts: Partial<Record<TextKey, string>>, key: TextKey): Partial<Record<TextKey, string>> {
  return key === "branchPrefix" ? { githubRepository: drafts.githubRepository } : { branchPrefix: drafts.branchPrefix }
}

/**
 * Settings → Repository: the `branchdeck.*` git config of the selected repository, which `bd` reads too, written to its
 * `.git/config` only. A switch of the repository shows the other one's settings, the typing of a field dropped.
 */
export function RepositoryPage() {
  const path = useSelectedProjectPath()
  if (path === undefined) {
    return (
      <p className="text-sm text-muted-foreground" data-testid="repository-settings-empty">
        Select a repository to see its settings
      </p>
    )
  }
  // one instance per repository: what was typed for one is never written to another
  return <RepositorySettings key={path} path={path} />
}

function RepositorySettings({ path }: { path: string }) {
  const { state, retry, setConfig } = useRepositoryConfig(path)
  const name = repositoryName(path)
  return (
    <div className="flex flex-col gap-4" data-testid="repository-settings">
      <p className="text-xs text-muted-foreground">
        {"Saved in "}
        <InlineCode>.git/config</InlineCode>
        {` of ${name}, read by bd too`}
      </p>
      {state.status === "loading" && <LoadingLine>Reading the settings…</LoadingLine>}
      {state.status === "error" && (
        <Callout tone="destructive" title={`Cannot read the settings of ${name}`} data-testid="repository-settings-error">
          <p>{state.message}</p>
          <div className="flex">
            <Button size="xs" variant="outline" onClick={retry}>
              <RotateCwIcon data-icon="inline-start" />
              Retry
            </Button>
          </div>
        </Callout>
      )}
      {state.status === "loaded" && <ConfigRows path={path} config={state.config} onConfig={setConfig} />}
    </div>
  )
}

function ConfigRows({ path, config, onConfig }: { path: string; config: RepositoryConfig; onConfig: (config: RepositoryConfig) => void }) {
  const appPrefix = normalizeBranchPrefix(useAppSettings(state => state.globalUserBranchPrefix))
  const [writing, setWriting] = useState<RepositoryConfigKey | null>(null)
  const [drafts, setDrafts] = useState<Partial<Record<TextKey, string>>>({})
  const saving = useRef(new Set<TextKey>())

  const dropDraft = (key: TextKey) => setDrafts(drafts => withoutDraft(drafts, key))

  /** Writes one key; true once the backend answered the settings read again */
  const write = (change: RepositoryConfigChange): Promise<boolean> => {
    const title = writeErrorTitle(change, config.stacks.fallbackValue, path)
    setWriting(change.key)
    return commands
      .setRepositoryConfig({ repositoryPath: path, change })
      .then(
        result => {
          if (result.status === "error") {
            notifyError(title, result.error)
            return false
          }
          onConfig(result.data)
          return true
        },
        (error: unknown) => {
          notifyError(title, error)
          return false
        },
      )
      .finally(() => setWriting(null))
  }

  /** The typed value as the backend takes it: trimmed, the prefix without its trailing slash; empty unsets */
  const typedValue = (key: TextKey, draft: string): string => (key === "branchPrefix" ? normalizeBranchPrefix(draft) : draft.trim())
  const invalidReason = (key: TextKey, value: string): string | null => {
    if (value === "") {
      return null
    }
    if (key === "branchPrefix") {
      return isValidBranchPrefix(value) ? null : invalidBranchPrefixReason(value)
    }
    return isValidGitHubRepository(value) ? null : invalidGitHubRepositoryReason(value)
  }

  /** Saves what was typed into the field, if it is valid and changes something, for the repository the typing began in */
  const save = (key: TextKey): Promise<void> => {
    const draft = drafts[key]
    // another repository was selected meanwhile: its page replaces this one, nothing is written; a blur after Enter
    // while its write runs writes nothing again
    if (draft === undefined || getSelectedProject()?.path !== path || saving.current.has(key)) {
      return Promise.resolve()
    }
    const value = typedValue(key, draft)
    if (invalidReason(key, value) !== null) {
      return Promise.resolve()
    }
    const setting = config[key]
    if (value === (setting.localSet ? (setting.value ?? "") : "")) {
      dropDraft(key)
      return Promise.resolve()
    }
    const change: RepositoryConfigChange = { key, value: value === "" ? null : value }
    saving.current.add(key)
    return write(change).then(written => {
      saving.current.delete(key)
      if (written) {
        dropDraft(key)
      }
    })
  }

  // a window closed while a field holds valid typing (⌘W, the close button, Escape outside the field) saves it first
  const saveOnClose = useEffectEvent(() => Promise.all(TEXT_KEYS.map(key => save(key))))
  useEffect(() => {
    let unlisten: UnlistenFn | null = null
    let cancelled = false
    getCurrentWindow()
      .onCloseRequested(() => saveOnClose().then(() => undefined))
      .then(
        fn => {
          if (cancelled) {
            fn()
          } else {
            unlisten = fn
          }
        },
        (error: unknown) => logError("Cannot save the settings when the window closes", error),
      )
    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [])

  const textField = (key: TextKey, label: string, placeholder: string) => {
    const draft = drafts[key]
    const setting = config[key]
    const reason = draft === undefined ? null : invalidReason(key, typedValue(key, draft))
    return {
      control: (
        <Input
          value={draft ?? setting.value ?? ""}
          onChange={event => {
            const value = event.target.value
            setDrafts(drafts => ({ ...drafts, [key]: value }))
          }}
          onBlur={() => void save(key)}
          onKeyDown={event => {
            if (event.key === "Enter") {
              event.preventDefault()
              void save(key)
            } else if (event.key === "Escape" && draft !== undefined) {
              // the window stays open: Escape takes the typing back first
              event.preventDefault()
              event.stopPropagation()
              dropDraft(key)
            }
          }}
          placeholder={placeholder}
          aria-label={label}
          aria-invalid={reason !== null || undefined}
          disabled={overriddenReason(setting) !== null}
          data-settings-focus={key}
          className="h-7 w-56"
        />
      ),
      error: reason === null ? null : `${writeErrorTitle({ key, value: null }, false, path)}: ${reason}`,
    }
  }

  const detected = config.detectedGithubRepository
  const github = textField("githubRepository", "GitHub repository", detected?.fullName ?? "owner/name")
  const prefix = textField("branchPrefix", "Branch prefix", appPrefix || config.branchPrefix.fallbackValue || "your-name")
  const stacksOff = config.stacks.value ? stacksOffReason(config.stacksOffClashes) : null
  const currentMode = SYMBOL_MODES.find(it => it.mode === config.symbols.value)

  return (
    <>
      <ConfigRow
        configKey="stacks"
        label="Stacked branches"
        setting={config.stacks}
        valueText={String(config.stacks.value)}
        fallbackText={config.stacks.fallbackValue ? "On" : "Off"}
        busy={writing === "stacks"}
        onReset={() => void write({ key: "stacks", value: null })}
        description="A (parent/child) prefix builds the child on its parent"
        reason={stacksOff}
        control={labelId => (
          <Switch
            size="sm"
            checked={config.stacks.value}
            disabled={overriddenReason(config.stacks) !== null || stacksOff !== null || writing === "stacks"}
            onCheckedChange={checked => void write({ key: "stacks", value: checked })}
            aria-labelledby={labelId}
          />
        )}
      />
      <ConfigRow
        configKey="symbols"
        label="Symbol dependencies"
        setting={config.symbols}
        valueText={config.symbols.value}
        fallbackText={symbolModeLabel(config.symbols.fallbackValue)}
        busy={writing === "symbols"}
        onReset={() => void write({ key: "symbols", value: null })}
        description={currentMode?.description ?? ""}
        control={labelId => (
          <FieldGroup role="group" aria-labelledby={labelId}>
            {SYMBOL_MODES.map(({ mode, label }) => {
              const pressed = config.symbols.value === mode
              return (
                <Button
                  key={mode}
                  size="xs"
                  variant="outline"
                  aria-pressed={pressed}
                  className={cn(pressed && "border-primary/50 text-primary hover:text-primary")}
                  disabled={overriddenReason(config.symbols) !== null || writing === "symbols"}
                  onClick={() => {
                    if (!pressed || !config.symbols.localSet) {
                      void write({ key: "symbols", value: mode })
                    }
                  }}
                >
                  {label}
                </Button>
              )
            })}
          </FieldGroup>
        )}
      />
      <ConfigRow
        configKey="githubRepository"
        label="GitHub repository"
        setting={config.githubRepository}
        valueText={config.githubRepository.value ?? ""}
        fallbackText={config.githubRepository.fallbackValue ?? detected?.fullName ?? "none"}
        busy={writing === "githubRepository"}
        onReset={() => void write({ key: "githubRepository", value: null })}
        description={
          detected === null ? "Empty, none: origin is not on GitHub" : `Empty, ${detected.fullName} ${detected.source === "spaceMirror" ? "from the Space mirror" : "from origin"}`
        }
        error={github.error}
        control={() => github.control}
      />
      <ConfigRow
        configKey="branchPrefix"
        label="Branch prefix"
        setting={config.branchPrefix}
        valueText={config.branchPrefix.value ?? ""}
        fallbackText={config.branchPrefix.fallbackValue ?? (appPrefix === "" ? "none" : `${appPrefix}, of Branch Deck`)}
        busy={writing === "branchPrefix"}
        onReset={() => void write({ key: "branchPrefix", value: null })}
        description="Names the branches here, over the prefix of General"
        error={prefix.error}
        control={() => prefix.control}
      />
    </>
  )
}

/**
 * One setting: its label and control, "Use Default" or "Use Global" while the local config sets it, what it does with
 * its git key, where its value comes from (not for the default), why it can't be changed, what is wrong with it
 */
function ConfigRow({
  configKey,
  label,
  setting,
  valueText,
  fallbackText,
  busy,
  onReset,
  description,
  reason = null,
  error = null,
  control,
}: {
  configKey: RepositoryConfigKey
  label: string
  setting: ConfigSetting<unknown>
  /** The effective value as git has it, for the source line */
  valueText: string
  /** The value once the local one is unset, for the tooltip of the reset */
  fallbackText: string
  busy: boolean
  onReset: () => void
  description: string
  /** Why the control is disabled, besides the worktree config or the command line */
  reason?: string | null
  /** The typing the backend would refuse */
  error?: string | null
  control: (labelId: string) => ReactNode
}) {
  const labelId = useId()
  const overridden = overriddenReason(setting)
  // a value the page can't change says where it comes from in its reason
  const source = overridden === null ? describeSource(setting, valueText) : null
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
              <TooltipContent>{`Removes it from .git/config: ${describeFallback(setting, fallbackText)}`}</TooltipContent>
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
