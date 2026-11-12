import { useEffect, useId, useState } from "react"
import type { ReactNode } from "react"
import { openUrl } from "@tauri-apps/plugin-opener"
import { cn } from "cn"
import { CircleArrowUpIcon, DownloadIcon, LogInIcon, XIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { ReadFailure, SettingsSection } from "@/components/settings/SettingRow"
import { CliToolRunningLine } from "@/components/shared/CliToolRunningLine"
import { CommandLine } from "@/components/shared/CommandLine"
import { CopyButton } from "@/components/shared/CopyButton"
import { InstallAgentCliButton } from "@/components/shared/InstallAgentCliButton"
import { LoadingLine } from "@/components/shared/LoadingLine"
import { useAgentCliStatus } from "@/hooks/useAgentCliStatus"
import { setIdeAppInstalled } from "@/hooks/useInstallAgentCli"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import type { AgentCliStatus, CliToolAction, CliToolServer, CliToolSkill, CliToolStatus, IdeApp } from "@/lib/bindings"
import {
  CLI_TOOL_PURPOSE,
  cliToolActionLabel,
  cliToolCommandHint,
  cliToolSkillLabel,
  describeCliToolServer,
  describeCliToolSkill,
  describeCliToolVersion,
  hostOf,
  isSkillAction,
} from "@/lib/cli-tools"
import { notifyError } from "@/lib/errors"
import { resolveHomeDir, tildePath } from "@/lib/home-path"
import {
  cancelCliToolAction,
  refreshCliTools,
  retryCliTools,
  runCliToolAction,
  setInstallWithSkill,
  signInYoutrackWithApp,
  useCliToolStatuses,
  useCliToolsReadError,
  useCliToolsRunning,
  useInstallWithSkill,
  useRunningCliToolAction,
} from "@/stores/cli-tools"
import type { RunningCliToolAction } from "@/stores/cli-tools"

/**
 * Settings → Tools (openspec/specs/repository/cli-tools/spec.md): one list, `brd` first, whose skill tells coding agents how
 * to work here, then IntelliJ IDEA and the `teamcity` and `youtrack` CLIs, each with its binary, version and sign-ins,
 * and the one action Rust picks for it; read again on each open and focus, the last answer shown meanwhile; a failed read
 * says why, with Retry, above them. An action that runs shows on its row in every window, also one another window
 * started (`useCliToolsRunning`). `server` is the TeamCity server an open names (the server of a robot's build), for the
 * sign-in; `onShowAgents` shows the page of the rules of the agents. A host that installs nothing (`installable`: the
 * page in the IDE, whose desktop app installs) shows neither `brd` nor IntelliJ IDEA.
 */
export function ToolsPage({ openCount, server, onShowAgents }: { openCount: number; server: string | undefined; onShowAgents: () => void }) {
  const statuses = useCliToolStatuses()
  const readError = useCliToolsReadError()
  const [homeDir, setHomeDir] = useState("")
  useEffect(() => {
    void refreshCliTools()
  }, [openCount])
  useEffect(() => {
    void resolveHomeDir().then(setHomeDir)
  }, [])
  useWindowFocus(() => void refreshCliTools())
  useCliToolsRunning()

  const agentCli = useAgentCliStatus(openCount)

  return (
    <SettingsSection title="Command-line tools" description="Agents and Branch Deck run them as you would; each keeps its own sign-in, its skill teaches the agents to use it">
      {readError !== null && <ReadFailure title="Cannot read the tools" message={readError} testId="cli-tools-error" onRetry={retryCliTools} />}
      <div className="flex flex-col divide-y divide-border">
        {agentCli?.installable !== false && <AgentCliRow status={agentCli} homeDir={homeDir} onShowAgents={onShowAgents} />}
        {agentCli?.installable === true && <IdeAppRow ideApp={agentCli.ide_app} homeDir={homeDir} />}
        {statuses === null
          ? readError === null && <LoadingLine className="py-3">Reading the tools…</LoadingLine>
          : statuses.map(status => <CliToolRow key={status.tool} status={status} homeDir={homeDir} server={server} />)}
      </div>
    </SettingsSection>
  )
}

/**
 * A row of the list: the name (`mono` for a command), what of it is there (`meta`: version, path) and the one action at
 * the end of its first line, what it is for under it, then its facts
 */
function ToolRow({
  name,
  mono,
  meta,
  end,
  purpose,
  testId,
  focusKey,
  children,
}: {
  name: string
  mono: boolean
  meta: ReactNode
  end: ReactNode
  purpose: ReactNode
  testId: string
  focusKey?: string
  children?: ReactNode
}) {
  const nameId = useId()
  return (
    <div
      role="group"
      aria-labelledby={nameId}
      className="flex flex-col gap-1.5 py-3 first:pt-0 last:pb-0"
      data-testid={testId}
      data-settings-focus={focusKey}
      tabIndex={focusKey === undefined ? undefined : -1}
    >
      <div className="flex min-h-6 items-center gap-2">
        <span id={nameId} className={cn("shrink-0 text-sm font-medium", mono && "font-mono")}>
          {name}
        </span>
        {meta}
        <span className="flex-1" />
        {end}
      </div>
      <p className="text-xs text-muted-foreground">{purpose}</p>
      {children}
    </div>
  )
}

/** The meta of a row: a version, a path, "Not installed"; cut at the end, whole in its tooltip */
function RowMeta({ children, title }: { children: string; title?: string }) {
  return (
    <span className="min-w-0 truncate text-xs text-muted-foreground" title={title}>
      {children}
    </span>
  )
}

/**
 * `brd`, the agent CLI of Branch Deck (openspec/specs/repository/settings/spec.md#tools): whether its skill is installed
 * (`agent_cli_status`, asked by the page on each open and focus) with the path of brd to copy and the skill directories of
 * the agents, else Install Agent CLI, the install of Help → Install Agent CLI…, whose toast has no action here. The rules
 * agents follow are on Agents, which its link shows
 */
function AgentCliRow({ status, homeDir, onShowAgents }: { status: AgentCliStatus | null; homeDir: string; onShowAgents: () => void }) {
  const brd = status?.bd ?? null
  return (
    <ToolRow
      name="brd"
      mono
      testId="agent-skill"
      focusKey="cli-brd"
      meta={
        brd !== null && (
          <>
            <RowMeta title={brd}>{tildePath(brd, homeDir)}</RowMeta>
            <CopyButton text={() => brd} tooltip="Copy the path of brd" alwaysVisible />
          </>
        )
      }
      end={status !== null && brd === null && <InstallAgentCliButton openAgentSettings={false} />}
      purpose={
        <>
          {"Coding agents ask it how to work in your repositories, by the rules of "}
          <Button variant="link" size="xs" className="h-auto p-0" onClick={onShowAgents}>
            Agents
          </Button>
        </>
      }
    >
      {status !== null && (
        <p className="min-w-0 truncate text-xs" data-testid="agent-skill-files" title={status.skills.join("\n")}>
          {brd === null ? "The agent skill is not installed" : `Agent skill branch-deck · ${status.skill_dirs.map(it => tildePath(it, homeDir)).join(", ")}`}
        </p>
      )}
    </ToolRow>
  )
}

/**
 * Branch Deck in IntelliJ IDEA (openspec/specs/repository/ide-embedding/spec.md#install-settings-tools-installs-branch-deck-in-intellij-idea):
 * the app file of an IDE with Air (`agent_cli_status`), with Remove, else "Not installed" and Install; a file that
 * another Branch Deck wrote says so, and Install writes it for this one
 */
function IdeAppRow({ ideApp, homeDir }: { ideApp: IdeApp | null; homeDir: string }) {
  const [running, setRunning] = useState(false)
  const run = (install: boolean) => {
    setRunning(true)
    void setIdeAppInstalled(install).finally(() => setRunning(false))
  }
  return (
    <ToolRow
      name="IntelliJ IDEA"
      mono={false}
      testId="ide-app"
      meta={ideApp === null ? <RowMeta>Not installed</RowMeta> : <RowMeta title={ideApp.file}>{tildePath(ideApp.file, homeDir)}</RowMeta>}
      end={
        <>
          {(ideApp === null || !ideApp.current) && (
            <Button size="xs" variant="outline" disabled={running} aria-busy={running || undefined} onClick={() => run(true)}>
              <DownloadIcon data-icon="inline-start" />
              Install
            </Button>
          )}
          {ideApp !== null && (
            <Button
              variant="link"
              size="xs"
              className="px-1 text-muted-foreground"
              disabled={running}
              aria-label="Remove Branch Deck from IntelliJ IDEA"
              onClick={() => run(false)}
            >
              Remove
            </Button>
          )}
        </>
      }
      purpose="IntelliJ IDEA with Air shows Branch Deck in a tool window of each Git project"
    >
      {ideApp !== null && !ideApp.current && <p className="text-xs text-warning">Another Branch Deck wrote the app file: Install makes IntelliJ IDEA run this one</p>}
    </ToolRow>
  )
}

/** A URL without its trailing `/`, as the CLI names its servers */
function serverKey(url: string): string {
  return url.trim().replace(/\/+$/, "")
}

function CliToolRow({ status, homeDir, server }: { status: CliToolStatus; homeDir: string; server: string | undefined }) {
  const running = useRunningCliToolAction(status.tool)
  const [otherServer, setOtherServer] = useState(false)
  const label = cliToolActionLabel(status)
  const action = status.action
  // the app starts the browser sign-in of teamcity itself: it needs the server
  const signsIn = status.tool === "teamcity" && action === "signIn" && status.command === null
  // the server to sign in to, the one the open names, else the first the CLI knows: one it has a line of signs in from
  // the head of the row, the form only for a server it doesn't know
  const wanted = serverKey(server ?? status.servers[0]?.url ?? "")
  const known = signsIn ? status.servers.find(it => it.url === wanted) : undefined
  // a skill action runs on the line of the skill, everything else on the row
  const runningHere = running !== undefined && !isSkillAction(running.action) ? running : undefined
  const busy = running === undefined ? undefined : `${status.tool} is busy`
  return (
    <ToolRow
      name={status.tool}
      mono
      testId={`cli-tool-${status.tool}`}
      focusKey={`cli-${status.tool}`}
      meta={
        <RowMeta title={status.binary ?? undefined}>
          {status.binary === null ? describeCliToolVersion(status) : `${describeCliToolVersion(status)} · ${tildePath(status.binary, homeDir)}`}
        </RowMeta>
      }
      end={
        runningHere !== undefined ? (
          <Button size="xs" variant="ghost" onClick={() => cancelCliToolAction(status.tool)}>
            <XIcon data-icon="inline-start" />
            Cancel
          </Button>
        ) : known !== undefined ? (
          <>
            {!otherServer && (
              <Button variant="link" size="xs" className="px-1 text-muted-foreground" disabled={running !== undefined} onClick={() => setOtherServer(true)}>
                Other Server…
              </Button>
            )}
            <Button size="xs" variant="outline" disabled={running !== undefined} title={busy} onClick={() => void runCliToolAction(status.tool, "signIn", null, known.url)}>
              <LogInIcon data-icon="inline-start" />
              Sign In
            </Button>
          </>
        ) : (
          label !== null &&
          action !== null && (
            <Button size="xs" variant="outline" disabled={running !== undefined} title={busy} onClick={() => runRowAction(status, action)}>
              {actionIcon(action)}
              {label}
            </Button>
          )
        )
      }
      purpose={CLI_TOOL_PURPOSE[status.tool]}
    >
      {status.servers.map(it => (
        <ServerLine key={it.url} status={status} server={it} again={it.url !== known?.url} disabled={running !== undefined} />
      ))}
      {status.skill !== null && <SkillLine status={status} skill={status.skill} running={running} />}
      {runningHere !== undefined && <CliToolRunningLine running={runningHere} />}
      {running === undefined && signsIn && (known === undefined || otherServer) && <SignInForm status={status} initialServer={known === undefined ? (server ?? "") : ""} />}
      {running === undefined && status.command !== null && (
        <div className="flex flex-col gap-1">
          <p className="text-xs text-muted-foreground">{cliToolCommandHint(action)}</p>
          <CommandLine>{status.command}</CommandLine>
        </div>
      )}
      {status.problem !== null && <p className="text-xs text-warning">{status.problem}</p>}
    </ToolRow>
  )
}

/** The main action of a row: the sign-in of youtrack with the app's YouTrack, else the action of the CLI */
function runRowAction(status: CliToolStatus, action: CliToolAction): void {
  if (action === "signIn" && status.signInWith !== null) {
    void signInYoutrackWithApp(status.signInWith)
    return
  }
  void runCliToolAction(status.tool, action, status.latest)
}

function actionIcon(action: CliToolAction) {
  switch (action) {
    case "update":
      return <CircleArrowUpIcon data-icon="inline-start" />
    case "signIn":
      return <LogInIcon data-icon="inline-start" />
    default:
      return <DownloadIcon data-icon="inline-start" />
  }
}

/**
 * The agent skill of the CLI: for a missing CLI the checkbox of its install, else the version the agents have with
 * Install, Update and Remove; a skill action shows its progress here
 */
function SkillLine({ status, skill, running }: { status: CliToolStatus; skill: CliToolSkill; running: RunningCliToolAction | undefined }) {
  const withSkill = useInstallWithSkill(status.tool)
  if (status.binary === null) {
    return (
      <label className="flex items-center gap-2 text-xs" data-testid="cli-tool-skill">
        <Checkbox checked={withSkill} disabled={running !== undefined} onCheckedChange={on => setInstallWithSkill(status.tool, on)} />
        {`Also install the agent skill ${skill.name}`}
      </label>
    )
  }
  if (running !== undefined && isSkillAction(running.action)) {
    return (
      <div className="flex min-w-0 items-center gap-1" data-testid="cli-tool-skill">
        <CliToolRunningLine running={running} />
        <Button variant="link" size="xs" className="h-auto px-1 py-0" onClick={() => cancelCliToolAction(status.tool)}>
          Cancel
        </Button>
      </div>
    )
  }
  const actions = [...(skill.action === null ? [] : [skill.action]), ...(skill.installed === null ? [] : ["removeSkill" as const])].filter(
    (it): it is "installSkill" | "updateSkill" | "removeSkill" => isSkillAction(it),
  )
  return (
    <p className="flex min-w-0 items-center gap-1 text-xs" data-testid="cli-tool-skill" title="~/.claude/skills, ~/.agents/skills">
      <span className="min-w-0 truncate">{describeCliToolSkill(status.tool, skill)}</span>
      {actions.map(skillAction => (
        <Button
          key={skillAction}
          variant="link"
          size="xs"
          className={cn("h-auto px-1 py-0", skillAction === "removeSkill" && "text-muted-foreground")}
          disabled={running !== undefined}
          aria-label={skillAction === "removeSkill" ? `Remove the ${status.tool} agent skill` : undefined}
          onClick={() => void runCliToolAction(status.tool, skillAction, null)}
        >
          {cliToolSkillLabel(skillAction)}
        </Button>
      ))}
    </p>
  )
}

/**
 * A server of the CLI: its user and expiry, or why its sign-in failed in the CLI's words, in `text-warning`, the page they
 * name a link. `teamcity` signs in to it again from here (an expiring or refused token), unless the head of the row
 * does (`again`)
 */
function ServerLine({ status, server, again, disabled }: { status: CliToolStatus; server: CliToolServer; again: boolean; disabled: boolean }) {
  const signsInAgain = again && status.tool === "teamcity" && status.command === null && server.state !== "guest"
  return (
    <p className="flex min-w-0 items-center gap-1 text-xs" data-testid="cli-tool-server">
      {server.state === "failed" ? (
        <span className="min-w-0 truncate">
          {`${hostOf(server.url)} · `}
          <FailedReason server={server} />
        </span>
      ) : (
        <span className="min-w-0 truncate">{describeCliToolServer(server)}</span>
      )}
      {signsInAgain && (
        <Button variant="link" size="xs" className="h-auto px-1 py-0" disabled={disabled} onClick={() => void runCliToolAction(status.tool, "signIn", null, server.url)}>
          Sign In Again
        </Button>
      )}
    </p>
  )
}

/** Why the sign-in of a server failed, its page a link (`error_url`, which Rust found in the words) */
function FailedReason({ server }: { server: CliToolServer }) {
  const reason = server.error ?? "not signed in"
  const url = server.errorUrl
  const at = url === null ? -1 : reason.indexOf(url)
  if (url === null || at < 0) {
    return <span className="text-warning">{reason}</span>
  }
  return (
    <span className="text-warning">
      {reason.slice(0, at)}
      <Button
        variant="link"
        size="xs"
        className="h-auto p-0 text-warning underline"
        onClick={() => void openUrl(url).catch((error: unknown) => notifyError("Cannot open the page", error))}
      >
        {url}
      </Button>
      {reason.slice(at + url.length)}
    </span>
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
