// @spec spec/branch/patronus.spec.md
// @spec spec/branch/declared-needs.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { LandingRefusal, NeedState, StackCommit } from "@/lib/bindings"
import { describeLandingRefusals, formatLandingFix, formatNeeded } from "@/presenters/landing"
import { DECLARED_DOCS } from "@/test-support/landing-refusal"

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
      anyway: { allowDangling: false, allowMissing: true, allowUnlanded: false },
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
        "“IJAI-4382 lane-run shows each scenario” of lane-run uses StillEncoding (avl-guest/src/lane/build_log/tests.rs:7), which ecebf000 (“IJAI-4382 the bundle manifest names lossy stills”, unassigned) adds: git merges it, but the build of the whole air stack fails. Move ecebf000 into lane-run. 2 more names break it too: brd check lists them.",
      fix: unassigned.fix,
      anyway: { allowDangling: true, allowMissing: true, allowUnlanded: false },
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

  it("words the order the user declared by the state of the branch it names, and Start Anyway lifts it", () => {
    const decision = describeLandingRefusals([DECLARED_DOCS], "auth")
    expect(decision).toEqual({
      reason: "You declared that docs lands before auth: it is not in master yet, so a start of auth lands them out of order.",
      fix: null,
      anyway: { allowDangling: false, allowMissing: false, allowUnlanded: true },
    })
    const of = (state: NeedState) => describeLandingRefusals([{ ...DECLARED_DOCS, state }], "the whole auth stack")?.reason
    expect(of({ kind: "unapplied" })).toBe("You declared that docs lands before auth, and it is unapplied: it cannot land at all. Reapply it, or remove the need.")
    expect(of({ kind: "missing" })).toBe(
      "You declared that docs lands before auth, and no branch of that name is here: a start of the whole auth stack lands what you ordered last. Remove the need, or sync the branch that carries it.",
    )
    expect(of({ kind: "companionUnreadable", companion: "ci-config", reason: "/src/ci-config is gone" })).toBe(
      "You declared that docs lands before auth, and the companion ci-config is not readable: /src/ci-config is gone. Make it readable again, or remove the need.",
    )
    const landing = describeLandingRefusals([{ ...DECLARED_DOCS, landingIn: { branch: "docs", robotId: "r1", wholeStack: false } }], "auth")
    expect(landing?.reason).toContain("out of order. docs is landing in its Safe Push, which tests it without this start: a start beside it fails too.")
    expect(formatNeeded(DECLARED_DOCS)).toBe("docs")
  })

  it("says why a branch built on the one that declared the need inherits it, and unstacks it to land it alone", () => {
    const inherited: LandingRefusal = { ...DECLARED_DOCS, branch: "ui", inheritedFrom: "auth", fix: { kind: "unstack", branch: "ui", from: "auth" } }
    const decision = describeLandingRefusals([inherited, DECLARED_DOCS, MISSING], "ui")
    expect(decision?.reason).toBe(
      "You declared that docs lands before auth: it is not in master yet, so a start of ui lands them out of order. ui is built on auth, which needs it: they land in one robot. Unstack ui from auth to land it alone. One more name breaks it too: brd check lists them. One more declared need holds it back too.",
    )
    expect(decision?.fix).toEqual({ kind: "unstack", branch: "ui", from: "auth" })
    expect(decision?.anyway).toEqual({ allowDangling: false, allowMissing: true, allowUnlanded: true })
    expect(formatLandingFix(decision!.fix)).toBe("Unstack ui from auth")
  })

  it("starts when nothing refuses it", () => {
    expect(describeLandingRefusals([], "auth")).toBeNull()
  })
})
