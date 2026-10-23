import type { CliToolsFixture } from "./test-repository"

/** The `teamcity auth status --json` of a sign-in to ci.example as jane until `expiry` (the page's clock starts at 2024-01-02 10:00Z) */
export function teamcitySignedIn(expiry: string): string {
  return JSON.stringify([{ server: "https://ci.example", status: "authenticated", user: { username: "jane" }, token_expiry: expiry }])
}

/** A machine with Homebrew and its teamcity 1.6.0, signed in, while 1.7.0 is out; `files` adds state files of the fakes */
export function teamcityBehindOnHomebrew(files: Record<string, string> = {}): CliToolsFixture {
  return {
    brew: true,
    installed: [{ tool: "teamcity", version: "1.6.0" }],
    latest: { teamcity: "1.7.0" },
    files: { "teamcity.auth": teamcitySignedIn("2026-11-21T16:58:19Z"), ...files },
  }
}
