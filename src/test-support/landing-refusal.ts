import type { LandingRefusal } from "@/lib/bindings"

/**
 * The robot of 2026-10-21 (`landing_needs`): lane-run uses Stills, which a commit of stills-codec, a branch it is not
 * built on, adds; the fix restacks lane-run on stills-codec
 */
export const MISSING_STILLS: LandingRefusal = {
  kind: "missingDeclaration",
  branch: "lane-run",
  identifier: "Stills",
  file: "lane/src/BuildLog.kt",
  line: 3,
  usedBy: { id: "c1", subject: "Show each scenario", branch: "lane-run" },
  declaredBy: { id: "c2", subject: "Name lossy stills", branch: "stills-codec" },
  fix: { kind: "restack", branch: "lane-run", onto: "stills-codec" },
  landingIn: null,
}
