// @spec openspec/specs/repository/unattended-mode/spec.md

import { describe, expect, it } from "vite-plus/test"
import type { UnattendedDenial, UnattendedMode } from "@/lib/bindings"
import { deniedAgent, deniedCount, deniedPlace, offersDeniedRequests, offersInstall, unattendedHookLine, unattendedModeLabel, unattendedToast } from "@/presenters/unattended"

function denial(rest: Partial<UnattendedDenial> = {}): UnattendedDenial {
  return { time: 1_795_500_000, session_id: "4f1c", cwd: "/Users/me/.branch-deck/pool/app-1/3/app", tool_name: "Bash", summary: "cat /etc/hosts", ...rest }
}

function mode(rest: Partial<UnattendedMode> = {}): UnattendedMode {
  return { on: true, changed: true, hook: "installed", installable: true, denials: [], ...rest }
}

describe("unattended", () => {
  it("names the switch by what a choice of it does", () => {
    expect([unattendedModeLabel(false), unattendedModeLabel(true)]).toEqual(["Turn On Unattended Mode", "Turn Off Unattended Mode"])
  })

  it("says nothing of a switch on whose hook is installed, warns of one without it and offers its install where the host installs", () => {
    expect(unattendedToast(mode())).toBeNull()
    expect(unattendedToast(mode({ changed: false, hook: "missing" }))).toBeNull()
    expect(unattendedToast(mode({ hook: "missing" }))).toEqual({
      type: "warning",
      title: "Turned on Unattended Mode",
      description: "Claude Code has no hook of Unattended Mode, so its permission requests still wait for you: Install Agent CLI adds it",
    })
    expect(unattendedToast(mode({ hook: "stale" }))?.description).toBe(
      "The hook of Unattended Mode runs another Branch Deck, so it denies nothing: Install Agent CLI puts this one in its place",
    )
    expect(unattendedToast(mode({ hook: "invalid" }))?.description).toBe(
      "The hook of Unattended Mode cannot go into ~/.claude/settings.json, which is no JSON object of Claude Code's settings, so its permission requests still wait for you: fix the file, then Install Agent CLI",
    )
    expect([offersInstall(mode({ hook: "missing" })), offersInstall(mode({ hook: "missing", installable: false })), offersInstall(mode())]).toEqual([true, false, false])
  })

  it("tells how many requests a switch off found and offers their list only when there are some, in the main window", () => {
    const night = mode({ on: false, denials: [denial(), denial({ tool_name: "Read" })] })
    expect(unattendedToast(night)).toEqual({ title: "Turned off Unattended Mode", description: "2 permission requests were denied while it was on" })
    expect(unattendedToast(mode({ on: false, denials: [denial()] }))?.description).toBe("1 permission request was denied while it was on")
    expect(unattendedToast(mode({ on: false }))?.description).toBe("No permission request was denied while it was on")
    expect([offersDeniedRequests(night), offersDeniedRequests(mode({ on: false })), offersDeniedRequests({ ...night, changed: false })]).toEqual([true, false, false])
    // Settings shows no list: its toast tells the count alone
    expect(offersDeniedRequests(night, false)).toBe(false)
    expect(deniedCount(0)).toBe("No permission request was denied")
  })

  it("words who asked and where", () => {
    expect(deniedAgent(denial())).toBe("Main session")
    expect(deniedAgent(denial({ agent_id: "a7", agent_type: "general-purpose" }))).toBe("general-purpose (a7)")
    expect(deniedAgent(denial({ agent_id: "a7", agent_type: null }))).toBe("Subagent a7")
    expect(deniedPlace(denial())).toBe("app")
    expect(deniedPlace(denial({ cwd: "C:\\\\work\\\\app\\\\" }))).toBe("app")
    expect(deniedPlace(denial({ cwd: undefined }))).toBe("")
  })

  it("says in Settings → Tools why the hook denies nothing", () => {
    expect(unattendedHookLine("missing")).toBe("The hook of Unattended Mode is not installed: Install Agent CLI adds it")
    expect(unattendedHookLine("stale")).toBe("The hook of Unattended Mode runs another Branch Deck: Install Agent CLI puts this one in its place")
    expect(unattendedHookLine("invalid")).toBe(
      "The hook of Unattended Mode cannot go into ~/.claude/settings.json, which is no JSON object of Claude Code's settings: fix the file, then Install Agent CLI",
    )
  })
})
