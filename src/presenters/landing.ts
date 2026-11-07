// The page's words of the refusals of the landing gate (`trial_check::landing_gate`, the rule of `brd safe-push`) and
// of the order of landing the user declared (`domain_policy::declared_needs`, `brd need`): the landing needs of a row
// and the plan of a start (`Reason::LandingRefused`), the refusal `execute` answers of a whole stack
// (`ErrorDetails::Landing`). The reason of the first, the count of the others, its fix and the overrides of Start
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
  const anyway: LandingAllowances = {
    allowDangling: allow.allowDangling === true || refusals.some(refusal => refusal.kind === "danglingReference"),
    allowMissing: allow.allowMissing === true || refusals.some(refusal => refusal.kind === "missingDeclaration"),
    allowUnlanded: allow.allowUnlanded === true || refusals.some(refusal => refusal.kind === "unlandedNeed"),
  }
  return { reason: `${describeLandingRefusal(first, subject)}${describeMore(refusals.slice(1))}`, fix: first.fix ?? null, anyway }
}

/** " One more name breaks it too: brd check lists them.", " One more declared need holds it back too.": the refusals past the first */
function describeMore(rest: readonly LandingRefusal[]): string {
  const declared = rest.filter(refusal => refusal.kind === "unlandedNeed").length
  const names = rest.length - declared
  const namesBreak = names === 0 ? "" : ` ${names === 1 ? "One more name breaks" : `${names} more names break`} it too: brd check lists them.`
  const needsHold = declared === 0 ? "" : ` ${declared === 1 ? "One more declared need holds" : `${declared} more declared needs hold`} it back too.`
  return `${namesBreak}${needsHold}`
}

/**
 * One refusal in the words of `brd safe-push`: "“Add the lane run” of lane-run uses StillEncoding (tests.rs:7), which
 * “Name lossy stills” of stills-codec adds: git merges it, but the build of ui-perf-nightly fails. Stack lane-run on
 * stills-codec to land it too."
 */
export function describeLandingRefusal(refusal: LandingRefusal, subject: string): string {
  switch (refusal.kind) {
    case "missingDeclaration": {
      const landing = refusal.landingIn == null ? "" : ` ${describeLanding(refusal.declaredBy.branch, refusal.landingIn)}`
      return `${describeCommit(refusal.usedBy)} uses ${refusal.identifier} (${refusal.file}:${refusal.line}), which ${describeCommit(refusal.declaredBy)} adds: git merges it, but the build of ${subject} fails. ${describeFix(refusal.fix)}${landing}`
    }
    case "danglingReference": {
      const change = refusal.reference === "removed" ? "removes" : refusal.reference === "changed" ? "changes" : "declares again"
      return `${describeCommit(refusal.removedBy)} ${change} ${refusal.identifier}, which ${refusal.file}:${refusal.line} still uses, and ${describeCommit(refusal.fixedBy)} changes that use: the build of ${subject} fails. Move that commit into ${refusal.branch}, or stack ${refusal.branch} on its branch.`
    }
    case "unlandedNeed":
      return describeUnlandedNeed(refusal, subject)
  }
}

/**
 * The order the user declared, by the state of the branch it names: "You declared that docs lands before auth: it is
 * not in master yet, so a start of auth lands them out of order."; of a branch that inherits it, why and the unstack
 * that lands it alone; of a needed branch a robot lands, that robot
 */
function describeUnlandedNeed(refusal: Extract<LandingRefusal, { kind: "unlandedNeed" }>, subject: string): string {
  const { branch, needs, state } = refusal
  const owner = refusal.inheritedFrom ?? branch
  const declared = `You declared that ${needs} lands before ${owner}`
  const inherited = owner === branch ? "" : ` ${branch} is built on ${owner}, which needs it: they land in one robot. Unstack ${branch} from ${owner} to land it alone.`
  switch (state.kind) {
    case "active": {
      const landing = refusal.landingIn == null ? "" : ` ${describeLanding(needs, refusal.landingIn)}`
      return `${declared}: it is not in master yet, so a start of ${subject} lands them out of order.${inherited}${landing}`
    }
    case "unapplied":
      return `${declared}, and it is unapplied: it cannot land at all. Reapply it, or remove the need.${inherited}`
    case "missing":
      return `${declared}, and no branch of that name is here: a start of ${subject} lands what you ordered last. Remove the need, or sync the branch that carries it.${inherited}`
    case "companionUnreadable":
      return `${declared}, and the companion ${state.companion} is not readable: ${state.reason}. Fix it in Settings → Companions, or remove the need.${inherited}`
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

/**
 * What the start needs first, as the row names it ("Needs stills-codec"): the branch of the commit that adds the name or
 * changes the dangling use (an unassigned commit by its hash), or the branch the user declared lands first
 */
export function formatNeeded(refusal: LandingRefusal): string {
  switch (refusal.kind) {
    case "missingDeclaration":
      return branchOf(refusal.declaredBy)
    case "danglingReference":
      return branchOf(refusal.fixedBy)
    case "unlandedNeed":
      return refusal.needs
  }
}

function branchOf(commit: StackCommit): string {
  return commit.branch === "" ? formatShortHash(commit.id) : commit.branch
}

/**
 * The label of the action that applies `fix` ("Stack lane-run on stills-codec", "Unstack ui from auth"); null for a fix
 * the toast can't run
 */
export function formatLandingFix(fix: LandingFix | null): string | null {
  switch (fix?.kind) {
    case "restack":
      return `Stack ${fix.branch} on ${fix.onto}`
    case "unstack":
      return `Unstack ${fix.branch} from ${fix.from}`
    default:
      return null
  }
}

function describeFix(fix: LandingFix): string {
  switch (fix.kind) {
    case "restack":
      return `Stack ${fix.branch} on ${fix.onto} to land it too.`
    case "moveCommit":
      return `Move ${formatShortHash(fix.commit.id)} into ${fix.into}.`
    case "unstack":
      return `Unstack ${fix.branch} from ${fix.from} to land it alone.`
  }
}

/** "“Name lossy stills” of stills-codec", an unassigned commit by its hash */
function describeCommit(commit: StackCommit): string {
  return commit.branch === "" ? `${formatShortHash(commit.id)} (“${commit.subject}”, unassigned)` : `“${commit.subject}” of ${commit.branch}`
}
