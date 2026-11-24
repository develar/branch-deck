// @spec openspec/specs/repository/unattended-mode/spec.md

import { describe, expect, it } from "vite-plus/test"
import type { UnattendedDenial, UnattendedMode } from "@/lib/bindings"
import { deniedAgent, deniedCount, deniedPlace, offersDeniedRequests, unattendedModeLabel, unattendedToast } from "@/presenters/unattended"

function denial(rest: Partial<UnattendedDenial> = {}): UnattendedDenial {
  return { time: 1_795_500_000, session_id: "4f1c", cwd: "/Users/me/.branch-deck/pool/app-1/3/app", tool_name: "Bash", summary: "cat /etc/hosts", ...rest }
}

function mode(rest: Partial<UnattendedMode> = {}): UnattendedMode {
  return { on: true, changed: true, warning: null, denials: [], ...rest }
}

describe("unattended", () => {
  it("names the switch by what a choice of it does", () => {
    expect([unattendedModeLabel(false), unattendedModeLabel(true)]).toEqual(["Turn On Unattended Mode", "Turn Off Unattended Mode"])
  })

  it("says nothing of a switch on, nor of a switch that changed nothing: the mark of the title bar says it", () => {
    expect(unattendedToast(mode())).toBeNull()
    expect(unattendedToast(mode({ on: false, changed: false }))).toBeNull()
  })

  it("warns on a switch off whose hook stayed in the settings of Claude Code", () => {
    expect(unattendedToast(mode({ on: false, warning: "Cannot remove the hook of Unattended Mode from /Users/me/.claude/settings.json: it is no JSON object" }))).toEqual({
      type: "warning",
      title: "Turned off Unattended Mode",
      description:
        "No permission request was denied while it was on. Cannot remove the hook of Unattended Mode from /Users/me/.claude/settings.json: it is no JSON object; it denies nothing while the mode is off",
    })
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
})
