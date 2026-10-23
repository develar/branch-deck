import type { CliTool, CliToolAction, CliToolServer, CliToolStatus } from "@/lib/bindings"

/*
 * The words of Settings → Tools (spec/repository/cli-tools.spec.md). Which action a row offers is Rust's
 * (`CliToolStatus.action`); this only names it.
 */

/** What the tool is for, as its row says */
export const CLI_TOOL_PURPOSE: Record<CliTool, string> = {
  teamcity: "The build logs and tests of the robots, for agents and the prompt of a failed robot",
  youtrack: "The issues of the commits, for agents",
}

/** The label of the button of `action`; null for a sign-in the app doesn't start: the row shows its command */
export function cliToolActionLabel(status: CliToolStatus): string | null {
  switch (status.action) {
    case "install":
      return status.command === null ? "Install" : null
    case "update":
      return status.command === null ? (status.latest === null ? "Update" : `Update to ${status.latest}`) : null
    case "updateSkill":
      return status.skill?.installed === null ? "Install Skill" : "Update Skill"
    case "signIn":
    case null:
      return null
  }
}

/** What the command of a row's action is for, before the command itself */
export function cliToolCommandHint(action: CliToolAction | null): string {
  switch (action) {
    case "install":
      return "Install it in a terminal:"
    case "signIn":
      return "Sign in in a terminal:"
    case "update":
      return "Update it in a terminal:"
    case "updateSkill":
    case null:
      return ""
  }
}

const DATE = new Intl.DateTimeFormat("en-US", { month: "short", day: "numeric" })

/** One line per server: "buildserver.labs.intellij.net · vladimir.krivosheev · expires Nov 21" */
export function describeCliToolServer(server: CliToolServer): string {
  const host = hostOf(server.url)
  switch (server.state) {
    case "authenticated": {
      const parts = [host, server.user, server.expiresAt === null ? null : `expires ${DATE.format(new Date(server.expiresAt * 1000))}`]
      return parts.filter(part => part !== null && part !== "").join(" · ")
    }
    case "guest":
      return `${host} · guest`
    case "failed":
      return `${host} · ${server.error ?? "not signed in"}`
  }
}

/** The version line: "1.6.0", "1.6.0 · 1.7.0 is out", or "Not installed" */
export function describeCliToolVersion(status: CliToolStatus): string {
  if (status.binary === null) {
    return "Not installed"
  }
  const version = status.version ?? "unknown version"
  return status.action === "update" && status.latest !== null ? `${version} · ${status.latest} is out` : version
}

/** The host of a server URL, the URL itself when it is none */
export function hostOf(url: string): string {
  try {
    return new URL(url).host
  } catch {
    return url
  }
}
