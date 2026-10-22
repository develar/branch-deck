import type { LandingAllowances, LandingCheck, LandingFix, LandingRefusal, RobotOf, StackCommit } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { getErrorDetails } from "@/lib/errors"
import { formatShortHash } from "@/lib/hash"
import { patronusRequest } from "@/stores/patronus"

/**
 * What the landing gate means for a start: `refuse` when a refusal holds it back (the build of what lands would fail),
 * with the fix of the first one and the overrides that start anyway; else `start`, with a sentence for the start's toast
 * when the check did not run
 */
export type LandingDecision =
  | { readonly kind: "refuse"; readonly reason: string; readonly fix: LandingFix | null; readonly anyway: LandingAllowances }
  | { readonly kind: "start"; readonly note: string | null }

/**
 * Asks what refuses the start of `branchName`, or of the whole stack of the root `wholeStack` (`check_landing_gate`, the
 * rule of `bd safe-push`): a dangling reference a commit proves, a name a branch outside what lands adds; the commits
 * merged Safe Pushes landed count as in the target, as Rust names them on the robots it reads. `allow` holds what the user
 * started anyway. A failed check holds nothing back: the note says so, and the start command checks again.
 */
export async function checkLandingBeforeStart(branchName: string, wholeStack: string | null, allow: LandingAllowances): Promise<LandingDecision> {
  const subject = wholeStack === null ? branchName : `the whole ${wholeStack} stack`
  try {
    const result = await commands.checkLandingGate({
      ...patronusRequest(),
      branchName: wholeStack ?? branchName,
      ...(wholeStack !== null && { stack: true }),
      allow,
    })
    if (result.status === "error") {
      return { kind: "start", note: notRunNote(subject, result.error.message) }
    }
    return describeLandingCheck(result.data, subject, allow)
  } catch (error) {
    return { kind: "start", note: notRunNote(subject, getErrorDetails(error)) }
  }
}

/** The decision of a check of `subject` ("auth", "the whole auth stack") after the overrides `allow` */
export function describeLandingCheck(check: LandingCheck, subject: string, allow: LandingAllowances): LandingDecision {
  const [first] = check.refusals
  if (first === undefined) {
    return { kind: "start", note: null }
  }
  const more = check.refusals.length - 1
  const rest = more === 0 ? "" : ` ${more === 1 ? "One more name breaks" : `${more} more names break`} it too: bd check lists them.`
  const anyway: LandingAllowances = {
    allowDangling: allow.allowDangling === true || check.refusals.some(refusal => refusal.kind === "danglingReference"),
    allowMissing: allow.allowMissing === true || check.refusals.some(refusal => refusal.kind === "missingDeclaration"),
  }
  return {
    kind: "refuse",
    reason: `${describeLandingRefusal(first, subject)}${rest}`,
    fix: first.fix,
    anyway,
  }
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

function notRunNote(subject: string, error: string): string {
  return `Not checked whether ${subject} uses a name only another branch adds: ${error}`
}
