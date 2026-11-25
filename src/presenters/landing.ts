// The page's words of the refusals of the landing gate (`trial_check::landing_gate`, the rule of `brd safe-push`) and
// of the order of landing the user declared (`domain_policy::declared_needs`, `brd need`): the landing needs of a row
// and the plan of a start (`Reason::LandingRefused`), the refusal `execute` answers of a whole stack
// (`ErrorDetails::Landing`). The reason of the first, the count of the others, its fix and the overrides of Start
// Anyway; the edges of the landing plan Rust merges them into (`domain_policy::landing_edges`), one sentence each; and
// the generated files the words merged that a copy holds (`domain_policy::regenerate`), which its Status
// cell, its push and its start name. It decides nothing.
import type { LandingAllowances, LandingEdge, LandingFix, LandingRefusal, RegenerateFile, RobotOf, StackCommit, Subject } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import { formatGenerated } from "@/presenters/review-plan"

/**
 * Why the build of what a start lands fails, the fix of the first refusal, and the overrides that start it anyway, which
 * lift these refusals only (`Land.anyway`)
 */
export interface LandingRefused {
  readonly reason: string
  readonly fix: LandingFix | null
  readonly anyway: LandingAllowances
  readonly refusals: readonly LandingRefusal[]
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
    allowDangling: allow.allowDangling === true || refusals.some(refusal => refusal.kind === "danglingReference" || refusal.kind === "lostBinding"),
    allowMissing: allow.allowMissing === true || refusals.some(refusal => refusal.kind === "missingDeclaration"),
    allowUnlanded: allow.allowUnlanded === true || refusals.some(refusal => refusal.kind === "unlandedNeed"),
  }
  return { reason: `${describeLandingRefusal(first, subject)}${describeMore(refusals.slice(1))}`, fix: landingFixOf(first), anyway, refusals }
}

/** The fix of `refusal` that the page can name or run; none of a lost binding, which only its own commit fixes */
export function landingFixOf(refusal: LandingRefusal): LandingFix | null {
  return refusal.kind === "lostBinding" ? null : (refusal.fix ?? null)
}

/** " One more name breaks it too.", " One more declared need holds it back too.": the refusals past the first */
function describeMore(rest: readonly LandingRefusal[]): string {
  const declared = rest.filter(refusal => refusal.kind === "unlandedNeed").length
  const names = rest.length - declared
  const namesBreak = names === 0 ? "" : ` ${names === 1 ? "One more name breaks" : `${names} more names break`} it too.`
  const needsHold = declared === 0 ? "" : ` ${declared === 1 ? "One more declared need holds" : `${declared} more declared needs hold`} it back too.`
  return `${namesBreak}${needsHold}`
}

/** A piece of the short detail of a refusal: its text, and the whole of what it shortens in the hover (`title`) */
export interface DetailPart {
  readonly text: string
  readonly title?: string
}

/**
 * The detail of the first of `refusals` under the headline of the popover, short, in one or two lines: commits by their
 * short hash ("1a2b3c4d of flow", the subject in the hover), the file by its name and line ("Flow.kt:3", the path in the
 * hover), then the count of the others; the fix only where no button of the popover runs it (`fixShown`: the restack)
 */
export function describeRefusalDetail(refusals: readonly LandingRefusal[], fixShown: boolean): DetailPart[] {
  const [first] = refusals
  if (first === undefined) {
    return []
  }
  const commit = (it: StackCommit): DetailPart => ({
    text: it.branch === "" ? `${formatShortHash(it.id)} (unassigned)` : `${formatShortHash(it.id)} of ${it.branch}`,
    title: it.subject,
  })
  const place = (file: string, line: number): DetailPart => ({ text: `${fileName(file)}:${line}`, title: `${file}:${line}` })
  const text = (value: string): DetailPart => ({ text: value })
  // a declared need words its own fix (the unstack of a branch that inherits it)
  const fix = fixShown || first.kind === "unlandedNeed" ? null : landingFixOf(first)
  const tail = [describeMore(refusals.slice(1)).trim(), fix === null ? "" : describeFix(fix)].filter(part => part !== "").join(" ")
  const after = tail === "" ? [] : [text(` ${tail}`)]
  switch (first.kind) {
    case "missingDeclaration":
      return [commit(first.usedBy), text(` uses ${first.identifier} (`), place(first.file, first.line), text("), which "), commit(first.declaredBy), text(" adds."), ...after]
    case "danglingReference": {
      const change = first.reference === "removed" ? "removes" : first.reference === "changed" ? "changes" : "declares again"
      return [
        commit(first.removedBy),
        text(` ${change} ${first.identifier}, still used at `),
        place(first.file, first.line),
        text("; "),
        commit(first.fixedBy),
        text(" changes that use."),
        ...after,
      ]
    }
    case "lostBinding": {
      const binding = first.binding === "import" ? "the import" : "the declaration"
      return [
        commit(first.removedBy),
        text(` removes ${binding} of ${first.identifier}, still used at `),
        place(first.file, first.line),
        text(". Keep it in that commit."),
        ...after,
      ]
    }
    case "unlandedNeed":
      return [text(describeUnlandedNeed(first, first.branch)), ...after]
  }
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
      const landing = refusal.landingIn == null ? "" : ` ${describeLanding(refusal.fixedBy.branch, refusal.landingIn)}`
      return `${describeCommit(refusal.removedBy)} ${change} ${refusal.identifier}, which ${refusal.file}:${refusal.line} still uses, and ${describeCommit(refusal.fixedBy)} changes that use: the build of ${subject} fails. ${describeFix(refusal.fix)}${landing}`
    }
    case "lostBinding": {
      const binding = refusal.binding === "import" ? "the import" : "the declaration"
      return `${describeCommit(refusal.removedBy)} removes ${binding} of ${refusal.identifier}, which ${refusal.file}:${refusal.line} still uses: git merges it, but the build of ${subject} fails. Keep it in that commit.`
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
      return `${declared}, and the companion ${state.companion} is not readable: ${state.reason}. Fix it in Settings → Landing, or remove the need.${inherited}`
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
 * changes the dangling use (an unassigned commit by its hash), the branch the user declared lands first, or a fix of
 * the commit that removes a binding its file still uses ("Needs fix of 6a4e00db")
 */
export function formatNeeded(refusal: LandingRefusal): string {
  switch (refusal.kind) {
    case "missingDeclaration":
      return branchOf(refusal.declaredBy)
    case "danglingReference":
      return branchOf(refusal.fixedBy)
    // no branch helps: the commit that removes the binding must keep it
    case "lostBinding":
      return `fix of ${formatShortHash(refusal.removedBy.id)}`
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

/**
 * The label of the action of the fix of an edge of row 1′, every fix: "Stack lane-run on stills-codec", "Unstack ui from
 * auth", "Group 1a2b3c4d into lane-run", "Safe Push Whole Stack"; null without a fix
 */
export function formatEdgeFix(fix: LandingFix | null): string | null {
  switch (fix?.kind) {
    case "moveCommit":
      return `Group ${formatShortHash(fix.commit.id)} into ${fix.into}`
    case "start":
      return formatStartOf(fix.subject)
    default:
      return formatLandingFix(fix)
  }
}

/** "Safe Push Whole Stack", "Safe Push ui": the start of a cycle's fix, which lands both */
function formatStartOf(subject: Subject): string {
  return subject.kind === "wholeStack" ? "Safe Push Whole Stack" : `Safe Push ${subject.name}`
}

function describeFix(fix: LandingFix): string {
  switch (fix.kind) {
    case "restack":
      return `Stack ${fix.branch} on ${fix.onto} to land it too.`
    case "moveCommit":
      return `Group ${formatShortHash(fix.commit.id)} into ${fix.into}.`
    case "unstack":
      return `Unstack ${fix.branch} from ${fix.from} to land it alone.`
    case "start":
      return `${fix.subject.kind === "wholeStack" ? `The Safe Push of the whole ${fix.subject.root} stack` : `The Safe Push of ${fix.subject.name}`} lands both.`
  }
}

/** The file of `path` by its name: "acp_load_replay.rs" */
function fileName(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1)
}

/**
 * What the start needs of the branch of `edge`, in one sentence: "Uses 5 names of acp-load-replay in acp_load_replay.rs
 * (Effect, Input +3)", "Uses Codec of codec in Codec.kt", of an unassigned commit by its hash; of a declared need the
 * order the user declared
 */
export function describeEdgeEvidence(edge: LandingEdge): string {
  if (edge.kind !== "build") {
    return `You declared that ${edge.needed} lands before ${edge.from}`
  }
  const [first, second] = edge.names
  const of = edge.state.kind === "unassigned" ? `${formatShortHash(edge.needed)} (unassigned)` : edge.needed
  const names =
    edge.names.length === 1 && first !== undefined
      ? first
      : edge.names.length === 2 && first !== undefined && second !== undefined
        ? `${first} and ${second}`
        : `${edge.names.length} names`
  const listed = edge.names.length > 2 && first !== undefined && second !== undefined ? ` (${first}, ${second} +${edge.names.length - 2})` : ""
  const [file] = edge.files
  const where = file === undefined ? "" : edge.files.length === 1 ? ` in ${fileName(file)}` : ` in ${fileName(file)} and ${edge.files.length - 1} more files`
  return `Uses ${names} of ${of}${where}${listed}`
}

/**
 * The edge in the words of its row: the evidence, then where the branch it needs stands and what clears it: "Uses 5
 * names of acp-load-replay in acp_load_replay.rs (Effect, Input +3): acp-load-replay is landing in its Safe Push, which
 * tests it without this start: a start beside it fails too."
 */
export function describeEdge(edge: LandingEdge): string {
  const evidence = describeEdgeEvidence(edge)
  switch (edge.state.kind) {
    case "waits":
      return `${evidence}: ${describeLanding(edge.needed, edge.state.robot)}`
    case "neededFailed":
      return `${evidence}: the Safe Push of ${edge.needed} failed. Restart it first: a start beside a failing piece fails too.`
    case "cycle":
      return `${evidence}: ${edge.needed} is built on what lands. ${edge.fix === null ? "" : describeFix(edge.fix)}`.trim()
    case "unassigned":
    case "open":
      return edge.fix === null ? `${evidence}: it is not in master yet.` : `${evidence}. ${describeFix(edge.fix)}`
  }
}

/** "Lands after acp-load-replay", "Restart after acp-load-replay": the wait of a start for the landing of what it needs, as a status says it */
export function formatLandsAfter(needed: string, restart: boolean): string {
  return `${restart ? "Restart" : "Lands"} after ${needed}`
}

/** "regenerate Cargo.lock", "regenerate 2 files": the Status cell of a copy that holds generated files the words merged */
export function formatRegenerate(files: readonly RegenerateFile[]): string {
  const [first] = files
  return files.length === 1 && first !== undefined ? `regenerate ${first.path.slice(first.path.lastIndexOf("/") + 1)}` : `regenerate ${files.length} files`
}

/**
 * Why the copy of a branch is no output of the generator of its generated files, and how it gets the commit it lacks:
 * "Cargo.lock (Lock file) was merged by words in the copy of “Add a dependency”, not written by its generator; the copy
 * lacks “Add another dependency” of core. Stack feature on core to land it too." One sentence per file, then the fix
 */
export function describeRegenerate(files: readonly RegenerateFile[], fix: LandingFix | null): string {
  const sentences = files.map(file => {
    const lacks = file.lacks == null ? "" : `; the copy lacks ${describeCommit(file.lacks)}`
    return `${file.path} (${formatGenerated(file.generatedBy)}) was merged by words in the copy of “${file.commit.subject}”, not written by its generator${lacks}.`
  })
  const clears = fix?.kind === "restack" ? ` It clears once ${fix.onto} lands, or a commit of the branch writes the file.` : ""
  return [...sentences, ...(fix === null ? [] : [describeFix(fix)])].join(" ") + clears
}

/** "“Name lossy stills” of stills-codec", an unassigned commit by its hash */
function describeCommit(commit: StackCommit): string {
  return commit.branch === "" ? `${formatShortHash(commit.id)} (“${commit.subject}”, unassigned)` : `“${commit.subject}” of ${commit.branch}`
}
