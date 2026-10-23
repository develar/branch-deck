import { useEffect, useState } from "react"
import { openUrl } from "@tauri-apps/plugin-opener"
import { DownloadIcon, ExternalLinkIcon, LogInIcon, XIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import { SettingsSection } from "@/components/settings/SettingRow"
import { CommandLine } from "@/components/shared/CommandLine"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import type { CliToolServer, CliToolStatus } from "@/lib/bindings"
import { CLI_TOOL_PURPOSE, cliToolActionLabel, cliToolCommandHint, describeCliToolServer, describeCliToolVersion } from "@/lib/cli-tools"
import { notifyError } from "@/lib/errors"
import { resolveHomeDir, tildePath } from "@/lib/home-path"
import { cancelCliToolAction, refreshCliTools, runCliToolAction, useCliToolStatuses, useRunningCliToolAction } from "@/stores/cli-tools"
import type { RunningCliToolAction } from "@/stores/cli-tools"

/**
 * Settings → Tools (spec/repository/cli-tools.spec.md): the `teamcity` and `youtrack` CLIs, each with its binary,
 * version and sign-ins, and the one action Rust picks for it; read again on each open and focus, the last answer shown
 * meanwhile. `server` is the TeamCity server an open names (the server of a robot's build), for the sign-in.
 */
export function ToolsPage({ openCount, server }: { openCount: number; server: string | undefined }) {
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
        statuses.map(status => <CliToolRow key={status.tool} status={status} homeDir={homeDir} server={server} />)
      )}
    </SettingsSection>
  )
}

function CliToolRow({ status, homeDir, server }: { status: CliToolStatus; homeDir: string; server: string | undefined }) {
  const running = useRunningCliToolAction(status.tool)
  const label = cliToolActionLabel(status)
  const action = status.action
  // the app starts the browser sign-in of the CLI itself: it needs the server
  const signsIn = action === "signIn" && status.command === null
  return (
    <div className="flex flex-col gap-1.5" data-testid={`cli-tool-${status.tool}`} data-settings-focus={status.tool} tabIndex={-1}>
      <div className="flex min-h-6 items-center gap-2">
        <span className="font-mono text-sm">{status.tool}</span>
        <span className="min-w-0 truncate text-xs text-muted-foreground" title={status.binary ?? undefined}>
          {status.binary === null ? describeCliToolVersion(status) : `${describeCliToolVersion(status)} · ${tildePath(status.binary, homeDir)}`}
        </span>
        <span className="flex-1" />
        {running === undefined ? (
          label !== null &&
          action !== null && (
            <Button size="xs" variant="outline" onClick={() => void runCliToolAction(status.tool, action, status.latest)}>
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
      {status.servers.map(known => (
        <ServerLine key={known.url} status={status} server={known} disabled={running !== undefined} />
      ))}
      {running !== undefined && <RunningLine running={running} />}
      {running === undefined && signsIn && <SignInForm status={status} initialServer={server ?? status.servers[0]?.url ?? ""} />}
      {running === undefined && status.command !== null && (
        <div className="flex flex-col gap-1">
          <p className="text-xs text-muted-foreground">{cliToolCommandHint(action)}</p>
          <CommandLine>{status.command}</CommandLine>
        </div>
      )}
      {status.problem !== null && <p className="text-xs text-warning">{status.problem}</p>}
    </div>
  )
}

/** A server of the CLI; `teamcity` signs in to it again from here (an expiring or refused token) */
function ServerLine({ status, server, disabled }: { status: CliToolStatus; server: CliToolServer; disabled: boolean }) {
  const again = status.tool === "teamcity" && status.command === null && server.state !== "guest"
  return (
    <p className="flex min-w-0 items-center gap-1 text-xs" data-testid="cli-tool-server">
      <span className="min-w-0 truncate">{describeCliToolServer(server)}</span>
      {again && (
        <Button variant="link" size="xs" className="px-1" disabled={disabled} onClick={() => void runCliToolAction(status.tool, "signIn", null, server.url)}>
          Sign In Again
        </Button>
      )}
    </p>
  )
}

/** The last line of the running action; a sign-in also offers the page it opened, for a browser that didn't come up */
function RunningLine({ running }: { running: RunningCliToolAction }) {
  const url = running.url
  return (
    <p className="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground" aria-live="polite" data-testid="cli-tool-progress">
      <Spinner className="size-3 shrink-0" />
      <span className="min-w-0 truncate font-mono">{running.line === "" ? "Starting…" : running.line}</span>
      {url !== null && (
        <Button variant="link" size="xs" className="px-1" onClick={() => void openUrl(url).catch((error: unknown) => notifyError("Cannot open the TeamCity sign-in", error))}>
          <ExternalLinkIcon data-icon="inline-start" />
          Open in Browser
        </Button>
      )}
    </p>
  )
}

/** The TeamCity server to sign in to, and Sign In: the CLI's own sign-in in the browser */
function SignInForm({ status, initialServer }: { status: CliToolStatus; initialServer: string }) {
  const [server, setServer] = useState(initialServer)
  const valid = /^https?:\/\/\S+$/.test(server.trim())
  return (
    <form
      className="flex items-center gap-2"
      onSubmit={event => {
        event.preventDefault()
        if (valid) {
          void runCliToolAction(status.tool, "signIn", null, server.trim())
        }
      }}
    >
      <Input
        aria-label="TeamCity server"
        className="h-7 flex-1 text-xs"
        value={server}
        placeholder="https://teamcity.example.com"
        spellCheck={false}
        autoComplete="off"
        onChange={event => setServer(event.target.value)}
      />
      <Button size="xs" variant="outline" type="submit" disabled={!valid}>
        <LogInIcon data-icon="inline-start" />
        Sign In
      </Button>
    </form>
  )
}
