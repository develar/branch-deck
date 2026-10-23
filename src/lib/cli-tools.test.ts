// @spec spec/repository/cli-tools.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { CliToolStatus } from "@/lib/bindings"
import { cliToolActionLabel, describeCliToolServer, describeCliToolVersion } from "@/lib/cli-tools"

const INSTALLED: CliToolStatus = {
  tool: "teamcity",
  binary: "/opt/homebrew/bin/teamcity",
  version: "1.6.0",
  servers: [],
  skill: { installed: "1.0.0", bundled: "1.4.0" },
  latest: "1.7.0",
  manager: "homebrew",
  action: null,
  command: null,
  problem: null,
}

describe("cli tools", () => {
  it("names the action the app runs, and none where the row shows a command", () => {
    expect(cliToolActionLabel({ ...INSTALLED, binary: null, action: "install" })).toBe("Install")
    expect(cliToolActionLabel({ ...INSTALLED, binary: null, action: "install", command: "curl -fsSL https://jb.gg/tc/install | bash" })).toBeNull()
    expect(cliToolActionLabel({ ...INSTALLED, action: "update" })).toBe("Update to 1.7.0")
    expect(cliToolActionLabel({ ...INSTALLED, action: "update", command: "youtrack update" })).toBeNull()
    expect(cliToolActionLabel({ ...INSTALLED, action: "updateSkill" })).toBe("Update Skill")
    expect(cliToolActionLabel({ ...INSTALLED, action: "updateSkill", skill: { installed: null, bundled: "1.4.0" } })).toBe("Install Skill")
    expect(cliToolActionLabel({ ...INSTALLED, action: "signIn", command: "teamcity auth login" })).toBeNull()
    expect(cliToolActionLabel(INSTALLED)).toBeNull()
  })

  it("says the version, the newer release and each server in a line", () => {
    expect(describeCliToolVersion({ ...INSTALLED, binary: null, version: null })).toBe("Not installed")
    expect(describeCliToolVersion({ ...INSTALLED, action: "update" })).toBe("1.6.0 · 1.7.0 is out")
    expect(describeCliToolVersion(INSTALLED)).toBe("1.6.0")
    expect(
      describeCliToolServer({ url: "https://buildserver.labs.intellij.net", user: "jane.doe", state: "authenticated", error: null, expiresAt: Date.UTC(2026, 10, 21, 12) / 1000 }),
    ).toBe("buildserver.labs.intellij.net · jane.doe · expires Nov 21")
    expect(describeCliToolServer({ url: "https://ci.example", user: null, state: "failed", error: "invalid token", expiresAt: null })).toBe("ci.example · invalid token")
    expect(describeCliToolServer({ url: "https://ci.example", user: null, state: "guest", error: null, expiresAt: null })).toBe("ci.example · guest")
  })
})
