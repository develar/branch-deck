// The page's words of the landing plan as Rust decides it (`RepositoryView.landingPlan`, `domain_policy::landing_plan`;
// openspec/specs/branch/landing-plan/spec.md): the titles of its waves, the size and the checks of a unit, the state of an
// edge, and the price of Land Together as its risks, one by one. It decides nothing: which edge, wave, offer, price and
// action there is are Rust's.
import type { ChecksPrice, LandingEdge, LandingWave, TogetherPrice } from "@/lib/bindings"
import { formatBranchCount } from "@/lib/branch"
import { formatCommitCount } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"
import { joinNames } from "@/lib/text"

/** "Landing now", "Next", "After acp-load-replay", "After codec and util", "After a, b, c and 2 more": the title of a wave */
export function formatWaveTitle(wave: LandingWave): string {
  switch (wave.kind.kind) {
    case "landing":
      return "Landing now"
    case "next":
      return "Next"
    case "after":
      return `After ${joinNames(wave.kind.after)}`
  }
}

/** "3 commits" of a lone branch, "2 branches · 7 commits" of a stack */
export function formatUnitSize(branches: number, commits: number): string {
  return branches > 1 ? `${formatBranchCount(branches)} · ${formatCommitCount(commits)}` : formatCommitCount(commits)
}

/** "7 checks", "1 check": the required checks of Patronus the changes of a unit start; null while they are unknown */
export function formatCheckCount(checks: readonly string[] | null | undefined): string | null {
  if (checks == null) {
    return null
  }
  return checks.length === 1 ? "1 check" : `${checks.length} checks`
}

/** A risk of a price, as the page shows it: its words, the whole of what they shorten in the hover */
export interface PriceRisk {
  readonly text: string
  readonly title?: string
  /** It holds both back where the other alone would pass: said in `warning` */
  readonly warns?: true
}

/** "same 7 checks", "+ AI Assistant Tests +11", "checks unknown": the checks of two units in one robot */
export function describeChecksPrice(checks: ChecksPrice): PriceRisk {
  switch (checks.kind) {
    case "same":
      return { text: `same ${checks.count === 1 ? "check" : `${checks.count} checks`}` }
    case "added": {
      const [first] = checks.added
      const more = checks.added.length - 1
      return {
        text: `+ ${first ?? ""}${more > 0 ? ` +${more}` : ""}`,
        title: `${checks.count} checks together; only one of them starts ${joinNames(checks.added)}`,
      }
    }
    case "unknown":
      return { text: "checks unknown", title: checks.reason }
  }
}

/**
 * The price of Land Together, a risk at a time in the order the plan names them: "1 robot instead of 2", the checks, "a
 * failure holds both", "approvals count in #12", the code owners the wider diff asks, "#12 and #13 become one GitHub stack"
 */
export function describeTogetherPrice(price: TogetherPrice): PriceRisk[] {
  const robots: PriceRisk = { text: `${price.together} ${price.together === 1 ? "robot" : "robots"} instead of ${price.robots}` }
  const risks: PriceRisk[] = [robots, describeChecksPrice(price.checks)]
  if (price.failureHoldsBoth) {
    risks.push({ text: "a failure holds both", title: "One robot, one verdict: a failed check of either fails the start of both", warns: true })
  }
  if (price.approvalsIn != null) {
    risks.push({ text: `approvals count in #${price.approvalsIn}`, title: `Patronus counts the approvals of pull request #${price.approvalsIn} for the start that lands both` })
  }
  const owners = price.ownersAdded ?? []
  if (owners.length > 0) {
    const [first] = owners
    const more = owners.length - 1
    risks.push({ text: `asks ${first}${more > 0 ? ` +${more}` : ""} too`, title: `The wider diff asks the code owners ${joinNames(owners)} too` })
  }
  const stack = price.githubStack ?? []
  if (stack.length > 0) {
    const numbers = joinNames(stack.map(number => `#${number}`))
    risks.push({ text: stack.length > 1 ? `${numbers} become one GitHub stack` : `${numbers} joins a GitHub stack` })
  }
  return risks
}

/** The price in one line: "1 robot instead of 2 · same 7 checks · a failure holds both" */
export function formatTogetherPrice(price: TogetherPrice): string {
  return describeTogetherPrice(price)
    .map(risk => risk.text)
    .join(" · ")
}

/**
 * The state of an edge in a few words, as its chip in the plan says it beside its evidence: "landing", "robot failed",
 * "declared", "nothing lands it"; of an unassigned commit or a cycle none: their line on the card says it
 */
export function formatEdgeState(edge: LandingEdge): string {
  if (edge.kind !== "build") {
    return edge.state.kind === "waits" ? "declared · landing" : "declared"
  }
  switch (edge.state.kind) {
    case "waits":
      return "landing"
    case "neededFailed":
      return "robot failed"
    case "open":
      return "nothing lands it"
    case "unassigned":
      return "unassigned"
    case "cycle":
      return "built on it"
  }
}

/**
 * The evidence of an edge in a few words, for its chip: "Effect, Input +3", "Codec", "declared"; the sentence of the
 * whole edge is its hover (`describeEdge`)
 */
export function formatEdgeEvidence(edge: LandingEdge): string {
  if (edge.kind !== "build") {
    return "declared"
  }
  const [first, second] = edge.names
  if (first === undefined) {
    return edge.files.length === 0 ? "build" : `${edge.files.length} ${edge.files.length === 1 ? "file" : "files"}`
  }
  if (second === undefined) {
    return first
  }
  return edge.names.length === 2 ? `${first}, ${second}` : `${first}, ${second} +${edge.names.length - 2}`
}

/** What an edge needs, as the card of a unit names one no curve draws: "1a2b3c4d (unassigned)", "docs:guide", "hosted-exit" */
export function formatEdgeNeeded(edge: LandingEdge): string {
  return edge.state.kind === "unassigned" ? `${formatShortHash(edge.needed)} (unassigned)` : edge.needed
}

/** The files of the uses of an edge, each on its line, for its hover: the whole paths */
export function formatEdgeFiles(edge: LandingEdge): string {
  return edge.files.join("\n")
}
