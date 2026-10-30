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
// The words of the review plan of a review window (spec/branch/review-plan.spec.md, Status: Experimental): the states
// of the commits since the user's review, the API the reviewed commits remove or change with the uses left outside
// the change, the dependency changes, the repeated edits, the new API, and the badges of the files. Rust pairs the
// commits, ranks the API and the files and decides each fact (`review_plan::review_plan_core`); the card and the tree
// only word them in the order Rust sent.
import type { ReviewPlace } from "@/lib/review-place"
import { fileTop } from "@/lib/review-place"

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

const KIND_WORDS: Readonly<Record<ApiChangeKind, string>> = { removed: "Removed", changed: "Changed", added: "Added" }

export const FORM_WORDS: Readonly<Record<FormChange, string>> = {
  header: "header",
  parameters: "parameters",
  reach: "visibility",
  open: "open",
  constructors: "constructors",
  jvm: "Java view",
}

/** The tone of an entry: a removal may break a use, a change may need one changed */
export type ApiTone = "destructive" | "warning"

function formatUses(count: number): string {
  return count === 1 ? "1 use" : `${count} uses`
}

/**
 * "3 uses left outside", "At least 2 uses left outside" of an incomplete count, "No use found outside, the count is
 * incomplete", "The uses are unknown" without a count, "Finding the uses…" while Rust asks the index (`pending`)
 */
export function formatOutsideUses(usages: Usages | null, pending: boolean): string {
  if (usages === null) {
    return pending ? "Finding the uses…" : "The uses are unknown"
  }
  if (usages.complete) {
    return `${formatUses(usages.outside)} left outside`
  }
  return usages.outside === 0 ? "No use found outside, the count is incomplete" : `At least ${formatUses(usages.outside)} left outside`
}

/** An API the commits remove or change, as the card words it */
export interface BreakageEntry {
  readonly key: string
  /** The file that declares it */
  readonly path: string
  /** Its declaration, before the commits for a removal: a click shows it */
  readonly place: ReviewPlace
  readonly name: string
  /** "Removed", "Changed" */
  readonly kind: string
  readonly tone: ApiTone
  /** "src/Store.kt:34" */
  readonly location: string
  /** "parameters, visibility": the parts of the form that changed, of a change */
  readonly changed: string | null
  /** The forms before and after, each on its own line */
  readonly before: string | null
  readonly after: string | null
  /** "3 uses left outside" (`formatOutsideUses`) */
  readonly outside: string
  /** "2 uses updated in this change"; null for none or an unknown count */
  readonly inside: string | null
  /** The first uses left outside, "src/Base.kt:1", each at its place at the head, then "+4 more" when Rust found more */
  readonly sites: readonly { readonly label: string; readonly place: ReviewPlace }[]
  readonly moreSites: string | null
}

/** "Possible breakage" of the card: the API with uses left outside or unknown, and the API whose every use is updated */
export interface Breakage {
  /** In the order Rust ranked them: the most uses left outside first */
  readonly entries: readonly BreakageEntry[]
  /** "12 API changes, every use updated in this change", and these entries; null for none */
  readonly updated: { readonly label: string; readonly entries: readonly BreakageEntry[] } | null
}

function breakageEntry(change: ApiChange, pending: boolean): BreakageEntry {
  const usages = change.usages
  const more = usages === null ? 0 : usages.outside - usages.outsideSites.length
  return {
    key: `${change.kind}:${change.key}:${change.path}:${change.line}`,
    path: change.path,
    place: { path: change.path, side: change.kind === "removed" ? "old" : "new", line: change.line, name: change.name },
    name: change.name,
    kind: KIND_WORDS[change.kind],
    tone: change.kind === "removed" ? "destructive" : "warning",
    location: `${change.path}:${change.line}`,
    changed: change.changes.length === 0 ? null : change.changes.map(it => FORM_WORDS[it]).join(", "),
    before: change.before,
    after: change.kind === "removed" ? null : change.after,
    outside: formatOutsideUses(usages, pending),
    inside: usages === null || usages.inside === 0 ? null : `${formatUses(usages.inside)} updated in this change`,
    sites: usages?.outsideSites.map(site => ({ label: `${site.path}:${site.line}`, place: { path: site.path, side: "new", line: site.line, name: change.name } })) ?? [],
    moreSites: more > 0 ? `+${more} more` : null,
  }
}

/**
 * The removed and changed API of the card. An entry with a use left outside, or with an unknown or incomplete count,
 * is listed; the entries whose every use is updated in this change fold into one line. An addition breaks no use.
 * `pending`: Rust asks the index, and the plan with the usages follows
 */
export function presentBreakage(api: readonly ApiChange[], pending: boolean): Breakage {
  const entries: BreakageEntry[] = []
  const updated: BreakageEntry[] = []
  for (const change of api) {
    if (change.kind === "added") {
      continue
    }
    // Rust says every use is updated in this change; an unknown or incomplete count never is
    if (change.usages?.updated === true) {
      updated.push(breakageEntry(change, pending))
    } else {
      entries.push(breakageEntry(change, pending))
    }
  }
  const count = updated.length === 1 ? "1 API change" : `${updated.length} API changes`
  return { entries, updated: updated.length === 0 ? null : { label: `${count}, every use updated in this change`, entries: updated } }
}

/** A dependency change of a module of the card */
export interface DependencyLine {
  readonly key: string
  readonly change: DependencyChangeKind
  /** "Changed crate serde 1.0.1 → 1.0.2" (`formatDependencyChange`) */
  readonly text: string
}

/** The dependency changes of one build file */
export interface ModuleDependencies {
  /** The build file: a click selects it */
  readonly manifest: string
  /** "intellij.platform.x", "(root)" */
  readonly module: string
  readonly lines: readonly DependencyLine[]
}

/** A link the commits added between the files of two modules */
export interface CrossModuleLink {
  readonly key: string
  readonly from: string
  readonly to: string
  /** The first use, in the file that uses: a click shows it */
  readonly place: ReviewPlace
  /** "Reader.kt → Cache.kt" */
  readonly label: string
  /** "Cache, get · Reader.kt:3" */
  readonly symbols: string
}

/** "Dependencies" of the card: the changes of each build file, in Rust's order, and the new links between modules */
export interface DependencySection {
  readonly modules: readonly ModuleDependencies[]
  readonly links: readonly CrossModuleLink[]
}

/** The dependency changes by build file, and the links the commits added across modules */
export function presentDependencyChanges(plan: Pick<ReviewPlan, "dependencies" | "links">): DependencySection {
  const modules = new Map<string, { manifest: string; module: string; lines: DependencyLine[] }>()
  plan.dependencies.forEach((change, index) => {
    let entry = modules.get(change.manifest)
    if (entry === undefined) {
      entry = { manifest: change.manifest, module: change.module, lines: [] }
      modules.set(change.manifest, entry)
    }
    entry.lines.push({ key: `${index}`, change: change.change, text: formatDependencyChange(change) })
  })
  const links = plan.links
    .filter(link => link.newBetweenModules)
    .map(link => {
      const { key, symbols } = linkEntry(link, link.to)
      return { key, from: link.from, to: link.to, place: firstUse(link), label: `${fileName(link.from)} → ${fileName(link.to)}`, symbols }
    })
  return { modules: [...modules.values()], links }
}

/** A group of files that get the same edit, as the card words it */
export interface RepeatedEditEntry {
  readonly key: string
  /** "Same edit in 14 files" */
  readonly label: string
  /** "ProjectManager → ProjectManagerEx" (`formatRepeatedEdit`) */
  readonly edit: string
  /** Its files: a click selects one */
  readonly files: readonly string[]
}

/** "Repeated edits" of the card, the most files first as Rust sent them */
export function presentRepeatedEdits(groups: readonly RepeatedEditGroup[]): RepeatedEditEntry[] {
  return groups.map((group, index) => ({ key: `${index}`, label: `Same edit in ${group.files.length} files`, edit: formatRepeatedEdit(group), files: group.files }))
}

/** The public declarations the commits add to one file */
export interface NewApiFile {
  readonly path: string
  /** Its first new declaration: a click shows it */
  readonly place: ReviewPlace
  /** "Cache, get" */
  readonly names: string
}

/** "New API" of the card: the added declarations per file, the files in the order of their first declaration */
export function presentNewApi(api: readonly ApiChange[]): NewApiFile[] {
  const byPath = new Map<string, { line: number; names: string[] }>()
  for (const change of api) {
    if (change.kind === "added") {
      const file = byPath.get(change.path) ?? { line: change.line, names: [] }
      file.names.push(change.name)
      byPath.set(change.path, file)
    }
  }
  return [...byPath].map(([path, file]) => ({ path, place: { path, side: "new", line: file.line, name: file.names[0] ?? null }, names: file.names.join(", ") }))
}

function count(n: number, one: string, many: string): string {
  return n === 1 ? `1 ${one}` : `${n} ${many}`
}

/**
 * "2 possible breaks · 3 dependency changes · 1 repeated edit · 4 new declarations", the line of the card's title;
 * what can break leads, "No possible break" when nothing can, "Finding possible breaks" while Rust asks the index
 * (`pending`)
 */
export function formatPlanSummary(plan: Pick<ReviewPlan, "api" | "dependencies" | "links" | "repeatedEdits">, pending: boolean): string {
  const breaks = presentBreakage(plan.api, pending).entries.length
  const crossLinks = plan.links.filter(link => link.newBetweenModules).length
  const added = plan.api.filter(change => change.kind === "added").length
  const parts = [pending ? "Finding possible breaks" : breaks === 0 ? "No possible break" : count(breaks, "possible break", "possible breaks")]
  if (plan.dependencies.length > 0) {
    parts.push(count(plan.dependencies.length, "dependency change", "dependency changes"))
  }
  if (crossLinks > 0) {
    parts.push(count(crossLinks, "new link between modules", "new links between modules"))
  }
  if (plan.repeatedEdits.length > 0) {
    parts.push(count(plan.repeatedEdits.length, "repeated edit", "repeated edits"))
  }
  if (added > 0) {
    parts.push(count(added, "new declaration", "new declarations"))
  }
  return parts.join(" · ")
}

/** What the tree of a review shows of its plan: the facts of each file, the links between the files, the dependency changes */
/** What the tree reads of the plan: the facts of the files, their links, the dependency changes, the repeated edits */
export type FileFactsOfPlan = Pick<ReviewPlan, "files" | "links" | "dependencies" | "repeatedEdits">

/** What the review window reads of the plan: the facts of the files and the units of the story */
export type ReviewFacts = FileFactsOfPlan & Pick<ReviewPlan, "units" | "api">

export const MECHANICAL_WORDS: Readonly<Record<MechanicalKind, string>> = {
  buildModel: "Build files",
  importsOnly: "Imports only",
  whitespace: "Whitespace only",
  renamed: "Renamed",
  deleted: "Deleted",
}

/** Why the diffs fold a mechanical file, in its header */
export const FOLDED_WORDS: Readonly<Record<MechanicalKind, string>> = {
  buildModel: "Build file",
  importsOnly: "Imports only",
  whitespace: "Whitespace only",
  renamed: "Renamed only",
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
  /** The other file */
  readonly path: string
  /** The other file, at the first use when it is the file that uses: a click shows it */
  readonly place: ReviewPlace
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

/** The first use of a link, in the file that uses; that file at its header without a name */
function firstUse(link: FileLink): ReviewPlace {
  const first = link.symbols[0]
  // a removed link is a use before the commits
  return { path: link.from, side: link.state === "removed" ? "old" : "new", line: first?.line ?? null, name: first?.name ?? null }
}

function linkEntry(link: FileLink, other: string): LinkEntry {
  const names = link.symbols.slice(0, LINK_SYMBOLS).map(symbol => symbol.name)
  const more = link.symbols.length - names.length
  const first = link.symbols[0]
  const site = first === undefined ? "" : ` · ${fileName(link.from)}:${first.line}`
  return {
    key: `${link.from}\n${link.to}`,
    path: other,
    place: other === link.from ? firstUse(link) : fileTop(other),
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

/** The badges of each file row, by its path; a file with none is not listed */
export function presentFileBadges(facts: FileFactsOfPlan): ReadonlyMap<string, FileBadges> {
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
