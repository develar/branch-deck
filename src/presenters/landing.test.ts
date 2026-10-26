// @spec spec/branch/patronus.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { LandingRefusal, StackCommit } from "@/lib/bindings"
import { describeLandingRefusals, formatLandingFix } from "@/presenters/landing"

const USES: StackCommit = { id: "a25430833a277000", subject: "IJAI-4382 lane-run shows each scenario", branch: "lane-run" }
const DECLARES: StackCommit = { id: "ecebf00037b41000", subject: "IJAI-4382 the bundle manifest names lossy stills", branch: "stills-codec" }

const MISSING: LandingRefusal = {
  kind: "missingDeclaration",
  branch: "lane-run",
  identifier: "Stills",
  file: "avl-guest/src/lane/build_log/tests.rs",
  line: 7,
  usedBy: USES,
  declaredBy: DECLARES,
  fix: { kind: "restack", branch: "lane-run", onto: "stills-codec" },
}

describe("describeLandingRefusals", () => {
  it("refuses the robot of 2026-10-21 with the name, both commits and the restack that lands the branch that adds it", () => {
    const decision = describeLandingRefusals([MISSING], "ui-perf-nightly")
    expect(decision).toEqual({
      reason:
        "“IJAI-4382 lane-run shows each scenario” of lane-run uses Stills (avl-guest/src/lane/build_log/tests.rs:7), which “IJAI-4382 the bundle manifest names lossy stills” of stills-codec adds: git merges it, but the build of ui-perf-nightly fails. Stack lane-run on stills-codec to land it too.",
      fix: MISSING.fix,
      anyway: { allowDangling: false, allowMissing: true },
    })
    expect(formatLandingFix(MISSING.fix)).toBe("Stack lane-run on stills-codec")
  })

  it("counts the other refusals, names an unassigned commit by its hash, and offers no action for a move", () => {
    const unassigned: LandingRefusal = {
      ...MISSING,
      identifier: "StillEncoding",
      declaredBy: { ...DECLARES, branch: "" },
      fix: { kind: "moveCommit", commit: DECLARES, into: "lane-run" },
    }
    const dangling: LandingRefusal = {
      kind: "danglingReference",
      branch: "fake-agent",
      reference: "removed",
      identifier: "CONTROL_URL_ENV",
      file: "AirFlowLaneHost.kt",
      line: 7,
      removedBy: { id: "5e618635aaaa", subject: "Move the control constants", branch: "fake-agent" },
      fixedBy: { id: "6d7aee25bbbb", subject: "Use the control protocol", branch: "lane-document" },
      fix: { kind: "restack", branch: "fake-agent", onto: "lane-document" },
    }
    const decision = describeLandingRefusals([unassigned, dangling, MISSING], "the whole air stack", { allowDangling: true })
    expect(decision).toEqual({
      reason:
        "“IJAI-4382 lane-run shows each scenario” of lane-run uses StillEncoding (avl-guest/src/lane/build_log/tests.rs:7), which ecebf000 (“IJAI-4382 the bundle manifest names lossy stills”, unassigned) adds: git merges it, but the build of the whole air stack fails. Move ecebf000 into lane-run. 2 more names break it too: bd check lists them.",
      fix: unassigned.fix,
      anyway: { allowDangling: true, allowMissing: true },
    })
    expect(formatLandingFix(unassigned.fix)).toBeNull()
  })

  it("says that the branch the start needs is landing in a running Safe Push, which tests it without this start", () => {
    const landing: LandingRefusal = { ...MISSING, landingIn: { branch: "stills-codec", robotId: "9b82be49", wholeStack: false } }
    const decision = describeLandingRefusals([landing], "ui-perf-nightly")
    expect(decision?.reason).toBe(
      "“IJAI-4382 lane-run shows each scenario” of lane-run uses Stills (avl-guest/src/lane/build_log/tests.rs:7), which “IJAI-4382 the bundle manifest names lossy stills” of stills-codec adds: git merges it, but the build of ui-perf-nightly fails. Stack lane-run on stills-codec to land it too. stills-codec is landing in its Safe Push, which tests it without this start: a start beside it fails too.",
    )
    const ofChild = describeLandingRefusals([{ ...landing, landingIn: { branch: "codec-ui", robotId: "1", wholeStack: false } }], "lane-run")
    expect(ofChild?.reason).toContain("stills-codec is landing in the Safe Push of codec-ui, which tests it without this start")
  })

  it("starts when nothing refuses it", () => {
    expect(describeLandingRefusals([], "auth")).toBeNull()
  })
})
