// The words of the review plan of a review window (spec/branch/review-plan.spec.md, Status: Experimental): the states
// of the commits since the user's review, the API the reviewed commits change with the usages of each declaration, and
// the risky files. Rust pairs the commits, ranks the API and the files and orders them (`review_plan::review_plan_core`);
// the card only words them in the order Rust sent.
import type { ApiChange, ApiChangeKind, CommitChanges, FanIn, FileRisk, FormChange, Interdiff, PairState, ReviewPlan } from "@/lib/bindings"

/** What the commit column says of a commit since the review: "New", "Rewritten"; nothing of the others */
export function formatPairState(state: PairState): string | null {
  switch (state) {
    case "added":
      return "New"
    case "rewritten":
      return "Rewritten"
    case "unchanged":
    case "dropped":
      return null
  }
}

/** "2 new · 1 rewritten · 1 dropped": what changed in the branch since its review; null when nothing did */
export function formatSinceReview(interdiff: Interdiff): string | null {
  const count = (state: PairState) => interdiff.commits.filter(commit => commit.state === state).length
  const parts = [
    [count("added"), "new"],
    [count("rewritten"), "rewritten"],
    [count("dropped"), "dropped"],
  ] as const
  const words = parts.filter(([n]) => n > 0).map(([n, word]) => `${n} ${word}`)
  return words.length === 0 ? null : words.join(" · ")
}

/** "src/Store.kt +2 −1" per path: what a rewritten commit changes that its reviewed version did not, and the reverse */
export function formatRewrittenChanges(changes: CommitChanges): string {
  const byPath = new Map<string, { added: number; removed: number }>()
  for (const [side, list] of [
    ["new", changes.newOnly],
    ["reviewed", changes.reviewedOnly],
  ] as const) {
    for (const { path, lines } of list) {
      const entry = byPath.get(path) ?? { added: 0, removed: 0 }
      if (side === "new") {
        entry.added += lines.length
      } else {
        entry.removed += lines.length
      }
      byPath.set(path, entry)
    }
  }
  if (byPath.size === 0) {
    return "Only its message changed"
  }
  return [...byPath].map(([path, { added, removed }]) => `${path} +${added} −${removed}`).join(", ")
}

/** "3 usages", "1 usage", "100+ usages" when the index cut its answer; null without a fan-in */
export function formatFanIn(fanIn: FanIn | null): string | null {
  if (fanIn === null) {
    return null
  }
  if (!fanIn.complete) {
    return `${fanIn.usages}+ usages`
  }
  return fanIn.usages === 1 ? "1 usage" : `${fanIn.usages} usages`
}

const KIND_WORDS: Readonly<Record<ApiChangeKind, string>> = { removed: "Removed", changed: "Changed", added: "Added" }

const FORM_WORDS: Readonly<Record<FormChange, string>> = {
  header: "header",
  parameters: "parameters",
  reach: "visibility",
  open: "open",
  constructors: "constructors",
  jvm: "Java view",
}

/** The tone of an entry: a removal may break a use, a change may need one changed, an addition breaks nothing */
export type ApiTone = "destructive" | "warning" | "info"

/** An entry of the API of the card, as it is worded */
export interface ApiEntry {
  readonly key: string
  readonly name: string
  readonly kind: string
  readonly tone: ApiTone
  /** "src/Store.kt:34" */
  readonly location: string
  /** "parameters, visibility": the parts of the form that changed, of a change */
  readonly changed: string | null
  /** The forms before and after, of a removal or a change */
  readonly before: string | null
  readonly after: string | null
  readonly fanIn: string | null
}

/** The API of the card in the order Rust ranked it (the most usages first) */
export function presentApiChanges(api: readonly ApiChange[]): ApiEntry[] {
  return api.map(change => ({
    key: `${change.kind}:${change.key}:${change.path}:${change.line}`,
    name: change.name,
    kind: KIND_WORDS[change.kind],
    tone: change.kind === "removed" ? "destructive" : change.kind === "changed" ? "warning" : "info",
    location: `${change.path}:${change.line}`,
    changed: change.changes.length === 0 ? null : change.changes.map(it => FORM_WORDS[it]).join(", "),
    // an addition breaks no use: its place says enough, and the form of a new type is only its constructors
    before: change.kind === "added" ? null : change.before,
    after: change.kind === "added" ? null : change.after,
    fanIn: formatFanIn(change.fanIn),
  }))
}

/** A risky file of the card and why: "Used in 12 places · No test changed" */
export interface RiskEntry {
  readonly path: string
  readonly reasons: string
}

/** The files the card raises (`risky`), in the order Rust ranked them */
export function presentRiskyFiles(risk: readonly FileRisk[]): RiskEntry[] {
  return risk
    .filter(file => file.risky)
    .map(file => {
      const reasons: string[] = []
      if (file.maxFanIn !== null && file.maxFanIn > 0) {
        reasons.push(file.maxFanIn === 1 ? "An API used in 1 place" : `An API used in ${file.maxFanIn} places`)
      }
      if (file.untested) {
        reasons.push("No test changed")
      }
      return { path: file.path, reasons: reasons.join(" · ") }
    })
}

/** "2 API changes · 1 risky file", the line of the card's title; "No API changes" */
export function formatPlanSummary(plan: Pick<ReviewPlan, "api" | "risk">): string {
  const api = plan.api.length
  const risky = plan.risk.filter(file => file.risky).length
  const parts = [api === 0 ? "No API changes" : api === 1 ? "1 API change" : `${api} API changes`]
  if (risky > 0) {
    parts.push(risky === 1 ? "1 risky file" : `${risky} risky files`)
  }
  return parts.join(" · ")
}

/** The toast of Mark Reviewed: "Marked auth reviewed", or that it was already */
export function formatMarkedReviewed(branch: string, changed: boolean): string {
  return changed ? `Marked ${branch} reviewed` : `${branch} was already marked reviewed at this commit`
}
