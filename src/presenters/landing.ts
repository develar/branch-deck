// The page's words of the refusals of the landing gate (`trial_check::landing_gate`, the rule of `bd safe-push`): the
// landing needs of a row and the plan of a start (`Reason::LandingRefused`), the refusal `execute` answers of a whole
// stack (`ErrorDetails::Landing`). The reason of the first, the count of the others, its fix and the overrides of Start
// Anyway; it decides nothing.
import type { LandingAllowances, LandingFix, LandingRefusal, RobotOf, StackCommit } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"

/** Why the build of what a start lands fails, the fix of the first refusal, and the overrides that start it anyway */
export interface LandingRefused {
  readonly reason: string
  readonly fix: LandingFix | null
  readonly anyway: LandingAllowances
}

/**
 * The refusals of a start of `subject` ("auth", "the whole auth stack") in the page's words, the overrides `allow` the
 * user gave already kept in its Start Anyway; null when nothing refuses it
 */
export function describeLandingRefusals(refusals: readonly LandingRefusal[], subject: string, allow: LandingAllowances = {}): LandingRefused | null {
  const [first] = refusals
  if (first === undefined) {
    return null
  }
  const more = refusals.length - 1
  const rest = more === 0 ? "" : ` ${more === 1 ? "One more name breaks" : `${more} more names break`} it too: bd check lists them.`
  const anyway: LandingAllowances = {
    allowDangling: allow.allowDangling === true || refusals.some(refusal => refusal.kind === "danglingReference"),
    allowMissing: allow.allowMissing === true || refusals.some(refusal => refusal.kind === "missingDeclaration"),
  }
  return { reason: `${describeLandingRefusal(first, subject)}${rest}`, fix: first.fix, anyway }
}

/**
 * One refusal in the words of `bd safe-push`: "“Add the lane run” of lane-run uses StillEncoding (tests.rs:7), which
 * “Name lossy stills” of stills-codec adds: git merges it, but the build of ui-perf-nightly fails. Stack lane-run on
 * stills-codec to land it too."
 */
export function describeLandingRefusal(refusal: LandingRefusal, subject: string): string {
  const place = `${refusal.file}:${refusal.line}`
  switch (refusal.kind) {
    case "missingDeclaration": {
      const landing = refusal.landingIn == null ? "" : ` ${describeLanding(refusal.declaredBy.branch, refusal.landingIn)}`
      return `${describeCommit(refusal.usedBy)} uses ${refusal.identifier} (${place}), which ${describeCommit(refusal.declaredBy)} adds: git merges it, but the build of ${subject} fails. ${describeFix(refusal.fix)}${landing}`
    }
    case "danglingReference": {
      const change = refusal.reference === "removed" ? "removes" : refusal.reference === "changed" ? "changes" : "declares again"
      return `${describeCommit(refusal.removedBy)} ${change} ${refusal.identifier}, which ${place} still uses, and ${describeCommit(refusal.fixedBy)} changes that use: the build of ${subject} fails. Move that commit into ${refusal.branch}, or stack ${refusal.branch} on its branch.`
    }
  }
}

/**
 * "stills-codec is landing in its Safe Push, which tests it without this start: a start beside it fails too.": the
 * running Safe Push of the branch the start needs (`landingIn`), its own or of a branch built on it or of its stack
 */
function describeLanding(branch: string, robot: RobotOf): string {
  const of = robot.wholeStack ? `the Safe Push of the whole ${robot.branch} stack` : robot.branch === branch ? "its Safe Push" : `the Safe Push of ${robot.branch}`
  return `${branch} is landing in ${of}, which tests it without this start: a start beside it fails too.`
}

/** The commit outside what lands that the start needs: the one that adds the name, or that changes the dangling use */
export function neededCommit(refusal: LandingRefusal): StackCommit {
  return refusal.kind === "missingDeclaration" ? refusal.declaredBy : refusal.fixedBy
}

/** The label of the action that applies `fix` ("Stack lane-run on stills-codec"); null for a fix the toast can't run */
export function formatLandingFix(fix: LandingFix | null): string | null {
  return fix?.kind === "restack" ? `Stack ${fix.branch} on ${fix.onto}` : null
}

function describeFix(fix: LandingFix): string {
  switch (fix.kind) {
    case "restack":
      return `Stack ${fix.branch} on ${fix.onto} to land it too.`
    case "moveCommit":
      return `Move ${formatShortHash(fix.commit.id)} into ${fix.into}.`
  }
}

/** "“Name lossy stills” of stills-codec", an unassigned commit by its hash */
function describeCommit(commit: StackCommit): string {
  return commit.branch === "" ? `${formatShortHash(commit.id)} (“${commit.subject}”, unassigned)` : `“${commit.subject}” of ${commit.branch}`
}
