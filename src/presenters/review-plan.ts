// The words of the review plan of a review window (spec/branch/review-plan.spec.md, Status: Experimental): the states
// of the commits since the user's review, the API the reviewed commits change with the usages of each declaration, and
// the risky files. Rust pairs the commits, ranks the API and the files and orders them (`review_plan::review_plan_core`);
// the card only words them in the order Rust sent.
import type {
  ApiChange,
  ApiChangeKind,
  CommitChanges,
  DependencyChange,
  DependencyChangeKind,
  DependencyKind,
  FileFacts,
  FileLink,
  FormChange,
  Interdiff,
  LinkState,
  MechanicalKind,
  PairState,
  RepeatedEditGroup,
  ReviewPlan,
  Usages,
} from "@/lib/bindings"

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

/** "3 usages", "1 usage", "100+ usages" when the index cut its answer, inside and outside the change; null when unknown */
export function formatUsages(usages: Usages | null): string | null {
  if (usages === null) {
    return null
  }
  const count = usages.inside + usages.outside
  if (!usages.complete) {
    return `${count}+ usages`
  }
  return count === 1 ? "1 usage" : `${count} usages`
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
  readonly usages: string | null
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
    usages: formatUsages(change.usages),
  }))
}

/** A file the card raises: the usages Rust counted of the API it removes or changes, or Rust's `untested` */
function isRisky(file: FileFacts): boolean {
  return file.breaks > 0 || file.untested
}

/** A risky file of the card and why: "Used in 12 places · No test changed" */
export interface RiskEntry {
  readonly path: string
  readonly reasons: string
}

/** The files the card raises (an API with usages removed or changed, or `untested`), in the order Rust ranked them */
export function presentRiskyFiles(files: readonly FileFacts[]): RiskEntry[] {
  return files.filter(isRisky).map(file => {
    const reasons: string[] = []
    if (file.breaks > 0) {
      reasons.push(file.breaks === 1 ? "An API used in 1 place" : `An API used in ${file.breaks} places`)
    }
    if (file.untested) {
      reasons.push("No test changed")
    }
    return { path: file.path, reasons: reasons.join(" · ") }
  })
}

/** "2 API changes · 1 risky file", the line of the card's title; "No API changes" */
export function formatPlanSummary(plan: Pick<ReviewPlan, "api" | "files">): string {
  const api = plan.api.length
  const risky = plan.files.filter(isRisky).length
  const parts = [api === 0 ? "No API changes" : api === 1 ? "1 API change" : `${api} API changes`]
  if (risky > 0) {
    parts.push(risky === 1 ? "1 risky file" : `${risky} risky files`)
  }
  return parts.join(" · ")
}

/** What the tree of a review shows of its plan: the facts of each file, the links between the files, the dependency changes */
export type ReviewFacts = Pick<ReviewPlan, "files" | "links" | "dependencies" | "repeatedEdits">

const MECHANICAL_WORDS: Readonly<Record<MechanicalKind, string>> = {
  buildModel: "Build files",
  importsOnly: "Imports only",
  whitespace: "Whitespace only",
  renamed: "Renamed",
  deleted: "Deleted",
}

/** "Mechanical (5)", the group at the end of the tree; "Imports only (2)", a group of one kind in it */
export function formatMechanicalGroup(kind: MechanicalKind | null, count: number): string {
  return `${kind === null ? "Mechanical" : MECHANICAL_WORDS[kind]} (${count})`
}

const LINK_STATE_WORDS: Readonly<Record<LinkState, string>> = { added: "Added", removed: "Removed", kept: "Kept" }

/** At most this many names of the symbols of a link, then "+2" */
const LINK_SYMBOLS = 3

/** A link of a file in its popover: the other file, the names it links on, where the first one is used */
export interface LinkEntry {
  readonly key: string
  /** The other file: a click selects it */
  readonly path: string
  /** Its name, "Cache.kt" */
  readonly name: string
  /** "get, Cache · Reader.kt:6": the names and the line of the first use, in the file that uses */
  readonly symbols: string
  readonly state: LinkState
  /** "Added", "Removed", "Kept" */
  readonly stateWord: string
}

/** The words of the badges of a file row */
export interface FileBadges {
  /** "uses 2", "used by 1": the links after the commits; null without one */
  readonly uses: string | null
  readonly usedBy: string | null
  /** "+1 new": the links it uses that the commits added */
  readonly newLinks: string | null
  /** The links it uses, then those that use it, a removed one too */
  readonly usesLinks: readonly LinkEntry[]
  readonly usedByLinks: readonly LinkEntry[]
  /**
   * "breaks 3": the uses left outside the change of the API it removes or changes; "breaks 3+" and "breaks ?" where Rust
   * says the count is a lower bound, so a 0 never reads as safe
   */
  readonly breaks: { readonly label: string; readonly title: string; readonly tone: "warning" | "muted" } | null
  /** "no test": a source file changed while none of its tests did */
  readonly untested: string | null
  /** "same edit ×4", the edit, and the other files that get it */
  readonly sameEdit: { readonly label: string; readonly edit: string; readonly files: readonly string[] } | null
  /** "uses 1, +1 new, breaks 3, no test": the badges in words, for the description of the row */
  readonly description: string
}

function fileName(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1)
}

function linkEntry(link: FileLink, other: string): LinkEntry {
  const names = link.symbols.slice(0, LINK_SYMBOLS).map(symbol => symbol.name)
  const more = link.symbols.length - names.length
  const first = link.symbols[0]
  const site = first === undefined ? "" : ` · ${fileName(link.from)}:${first.line}`
  return {
    key: `${link.from}\n${link.to}`,
    path: other,
    name: fileName(other),
    symbols: `${names.join(", ")}${more > 0 ? ` +${more}` : ""}${site}`,
    state: link.state,
    stateWord: LINK_STATE_WORDS[link.state],
  }
}

/** "ProjectManager → ProjectManagerEx": the first line the edit removes and the first it adds */
export function formatRepeatedEdit(group: RepeatedEditGroup): string {
  const removed = group.removed[0]
  const added = group.added[0]
  if (removed !== undefined && added !== undefined) {
    return `${removed} → ${added}`
  }
  return removed === undefined ? `+ ${added ?? ""}` : `− ${removed}`
}

function breaksBadge(file: FileFacts): FileBadges["breaks"] {
  if (file.breaksComplete) {
    return file.breaks === 0 ? null : { label: `breaks ${file.breaks}`, title: `${formatUses(file.breaks)} left outside the change`, tone: "warning" }
  }
  if (file.breaks === 0) {
    return { label: "breaks ?", title: "The uses left outside the change are unknown, or their count is incomplete", tone: "muted" }
  }
  return { label: `breaks ${file.breaks}+`, title: `At least ${formatUses(file.breaks)} left outside the change: the count is incomplete`, tone: "warning" }
}

function formatUses(count: number): string {
  return count === 1 ? "1 use" : `${count} uses`
}

/** The badges of each file row, by its path; a file with none is not listed */
export function presentFileBadges(facts: ReviewFacts): ReadonlyMap<string, FileBadges> {
  const badges = new Map<string, FileBadges>()
  for (const file of facts.files) {
    const usesLinks = facts.links.filter(link => link.from === file.path).map(link => linkEntry(link, link.to))
    const usedByLinks = facts.links.filter(link => link.to === file.path).map(link => linkEntry(link, link.from))
    const group = file.repeatedEdit === null ? undefined : facts.repeatedEdits[file.repeatedEdit]
    const entry: Omit<FileBadges, "description"> = {
      uses: file.uses === 0 ? null : `uses ${file.uses}`,
      usedBy: file.usedBy === 0 ? null : `used by ${file.usedBy}`,
      newLinks: file.newLinks === 0 ? null : `+${file.newLinks} new`,
      usesLinks,
      usedByLinks,
      breaks: breaksBadge(file),
      untested: file.untested ? "no test" : null,
      sameEdit: group === undefined ? null : { label: `same edit ×${group.files.length}`, edit: formatRepeatedEdit(group), files: group.files.filter(path => path !== file.path) },
    }
    if (usesLinks.length > 0 || usedByLinks.length > 0 || entry.breaks !== null || entry.untested !== null || entry.sameEdit !== null) {
      const words = [entry.uses, entry.newLinks, entry.usedBy, entry.breaks?.label, entry.sameEdit?.label, entry.untested]
      badges.set(file.path, { ...entry, description: words.filter(word => word != null).join(", ") })
    }
  }
  return badges
}

/** The dependency changes of the build files of a directory row */
export interface DependencyBadge {
  /** "+2 deps −1 ~1", "new module" */
  readonly label: string
  /** One line per change: "Added module intellij.platform.x", "Changed crate serde 1.0.1 → 1.0.2" */
  readonly lines: readonly string[]
}

const DEPENDENCY_KIND_WORDS: Readonly<Record<DependencyKind, string>> = {
  newModule: "module",
  module: "module",
  library: "library",
  plugin: "plugin",
  content: "content module",
  crate: "crate",
  package: "package",
}

const CHANGE_WORDS: Readonly<Record<DependencyChangeKind, string>> = { added: "Added", removed: "Removed", changed: "Changed" }

/** "Changed crate serde 1.0.1 → 1.0.2 (dev)": what one change does */
export function formatDependencyChange(change: DependencyChange): string {
  if (change.kind === "newModule") {
    return `${change.change === "removed" ? "Deleted" : "New"} module ${change.name}`
  }
  let version = ""
  if (change.versionBefore !== null && change.versionAfter !== null && change.versionBefore !== change.versionAfter) {
    version = ` ${change.versionBefore} → ${change.versionAfter}`
  } else if ((change.versionAfter ?? change.versionBefore) !== null) {
    version = ` ${change.versionAfter ?? change.versionBefore}`
  }
  let scope = ""
  if (change.scopeBefore !== null && change.scopeBefore !== change.scope) {
    scope = ` (${change.scopeBefore} → ${change.scope})`
  } else if (change.scope !== "main") {
    scope = ` (${change.scope})`
  }
  return `${CHANGE_WORDS[change.change]} ${DEPENDENCY_KIND_WORDS[change.kind]} ${change.name}${version}${scope}`
}

/** The badge of the dependency changes of a module: "+2 deps −1 ~1", "new module"; null for none */
export function presentDependencyBadge(changes: readonly DependencyChange[]): DependencyBadge | null {
  if (changes.length === 0) {
    return null
  }
  const parts: string[] = []
  const modules = changes.filter(change => change.kind === "newModule")
  if (modules.some(change => change.change === "added")) {
    parts.push("new module")
  }
  if (modules.some(change => change.change === "removed")) {
    parts.push("deleted module")
  }
  const entries = changes.filter(change => change.kind !== "newModule")
  const counts = [
    ["+", entries.filter(change => change.change === "added").length],
    ["−", entries.filter(change => change.change === "removed").length],
    ["~", entries.filter(change => change.change === "changed").length],
  ] as const
  const shown = counts.filter(([, count]) => count > 0)
  if (shown.length > 0) {
    const [sign, first] = shown[0]!
    parts.push([`${sign}${first} ${first === 1 ? "dep" : "deps"}`, ...shown.slice(1).map(([mark, count]) => `${mark}${count}`)].join(" "))
  }
  return { label: parts.join(" · "), lines: changes.map(formatDependencyChange) }
}

/** The toast of Mark Reviewed: "Marked auth reviewed", or that it was already */
export function formatMarkedReviewed(branch: string, changed: boolean): string {
  return changed ? `Marked ${branch} reviewed` : `${branch} was already marked reviewed at this commit`
}
