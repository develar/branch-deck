import { useEffect, useState } from "react"
import { DownloadIcon, XIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { SettingsSection } from "@/components/settings/SettingRow"
import { CommandLine } from "@/components/shared/CommandLine"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import type { CliToolStatus } from "@/lib/bindings"
import { CLI_TOOL_PURPOSE, cliToolActionLabel, cliToolCommandHint, describeCliToolServer, describeCliToolVersion } from "@/lib/cli-tools"
import { resolveHomeDir, tildePath } from "@/lib/home-path"
import { cancelCliToolAction, refreshCliTools, runCliToolAction, useCliToolStatuses, useRunningCliToolAction } from "@/stores/cli-tools"

/**
 * Settings → Tools (spec/repository/cli-tools.spec.md): the `teamcity` and `youtrack` CLIs, each with its binary,
 * version and sign-ins, and the one action Rust picks for it; read again on each open and focus, the last answer shown
 * meanwhile
 */
export function ToolsPage({ openCount }: { openCount: number }) {
  const statuses = useCliToolStatuses()
  const [homeDir, setHomeDir] = useState("")
  useEffect(() => {
    void refreshCliTools()
  }, [openCount])
  useEffect(() => {
    void resolveHomeDir().then(setHomeDir)
  }, [])
  useWindowFocus(() => void refreshCliTools())

  return (
    <SettingsSection title="Command-line tools" description="Agents and Branch Deck run them as you would; each keeps its own sign-in">
      {statuses === null ? (
        <Spinner className="size-4" aria-label="Reading the tools" />
      ) : (
        statuses.map(status => <CliToolRow key={status.tool} status={status} homeDir={homeDir} />)
      )}
    </SettingsSection>
  )
}

function CliToolRow({ status, homeDir }: { status: CliToolStatus; homeDir: string }) {
  const running = useRunningCliToolAction(status.tool)
  const label = cliToolActionLabel(status)
  return (
    <div className="flex flex-col gap-1.5" data-testid={`cli-tool-${status.tool}`} data-settings-focus={status.tool} tabIndex={-1}>
      <div className="flex min-h-6 items-center gap-2">
        <span className="font-mono text-sm">{status.tool}</span>
        <span className="min-w-0 truncate text-xs text-muted-foreground" title={status.binary ?? undefined}>
          {status.binary === null ? describeCliToolVersion(status) : `${describeCliToolVersion(status)} · ${tildePath(status.binary, homeDir)}`}
        </span>
        <span className="flex-1" />
        {running === undefined ? (
          label !== null && (
            <Button size="xs" variant="outline" onClick={() => status.action !== null && void runCliToolAction(status.tool, status.action, status.latest)}>
              <DownloadIcon data-icon="inline-start" />
              {label}
            </Button>
          )
        ) : (
          <Button size="xs" variant="ghost" onClick={() => cancelCliToolAction(status.tool)}>
            <XIcon data-icon="inline-start" />
            Cancel
          </Button>
        )}
      </div>
      <p className="text-xs text-muted-foreground">{CLI_TOOL_PURPOSE[status.tool]}</p>
      {status.servers.map(server => (
        <p key={server.url} className="text-xs" data-testid="cli-tool-server">
          {describeCliToolServer(server)}
        </p>
      ))}
      {running !== undefined && (
        <p className="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground" aria-live="polite" data-testid="cli-tool-progress">
          <Spinner className="size-3 shrink-0" />
          <span className="min-w-0 truncate font-mono">{running.line === "" ? "Starting…" : running.line}</span>
        </p>
      )}
      {running === undefined && status.command !== null && (
        <div className="flex flex-col gap-1">
          <p className="text-xs text-muted-foreground">{cliToolCommandHint(status.action)}</p>
          <CommandLine>{status.command}</CommandLine>
        </div>
      )}
      {status.problem !== null && <p className="text-xs text-warning">{status.problem}</p>}
    </div>
  )
}
