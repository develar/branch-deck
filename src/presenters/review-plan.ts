import type {
  ApiChange,
  ApiChangeKind,
  BreakVerdict,
  CommitChanges,
  DependencyChange,
  DependencyChangeKind,
  DependencyKind,
  FileFacts,
  FileLink,
  FormChange,
  GeneratedBy,
  Interdiff,
  LinkState,
  MechanicalKind,
  PairedCommit,
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
import { capitalize, joinNames } from "@/lib/text"
import { GENERATED_NAME_WORDS } from "@/presenters/collapse"

/** What the commit column says of a commit since the review: "New", "Rewritten", "Reworded"; nothing of the others */
export function formatPairState(state: PairState): string | null {
  switch (state) {
    case "added":
      return "New"
    case "rewritten":
      return "Rewritten"
    case "reworded":
      return "Reworded"
    case "unchanged":
    case "dropped":
      return null
  }
}

/** "2 new · 1 rewritten · 1 reworded · 1 dropped": what changed in the branch since its review; null when nothing did */
export function formatSinceReview(interdiff: Interdiff): string | null {
  const count = (state: PairState) => interdiff.commits.filter(commit => commit.state === state).length
  const parts = [
    [count("added"), "new"],
    [count("rewritten"), "rewritten"],
    [count("reworded"), "reworded"],
    [count("dropped"), "dropped"],
  ] as const
  const words = parts.filter(([n]) => n > 0).map(([n, word]) => `${n} ${word}`)
  return words.length === 0 ? null : words.join(" · ")
}

/**
 * What a rewritten or reworded commit changes against its reviewed version: "src/Store.kt +2 −1" per path, the lines it
 * changes that its reviewed version did not and the reverse, then "message changed" when Rust says its message did
 * (`PairedCommit.messageChange`); a rewritten commit whose lines per path are the same moved them ("the same lines in
 * another order")
 */
export function formatRewrittenChanges({ changes, messageChange }: Pick<PairedCommit, "changes" | "messageChange">): string {
  const parts = [...(changes === null ? [] : [formatChangedLines(changes)]), ...(messageChange === null ? [] : ["message changed"])]
  return parts.join(" · ")
}

/** "src/Store.kt +2 −1" per path: what a rewritten commit changes that its reviewed version did not, and the reverse */
function formatChangedLines(changes: CommitChanges): string {
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
  // patch-id saw other lines, the lines of each path are the same: they moved
  if (byPath.size === 0) {
    return "the same lines in another order"
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

/** The tone of an entry: a removal may break a use, a change may need one changed, a widening breaks none */
export type ApiTone = "destructive" | "warning" | "secondary"

function formatUses(count: number): string {
  return count === 1 ? "1 use" : `${count} uses`
}

function formatCalls(count: number): string {
  return count === 1 ? "1 call" : `${count} calls`
}

/**
 * "3 uses left outside", "At least 2 uses left outside" of an incomplete count, "No use left outside found, the search
 * stopped early" of an incomplete count of none (the index answers at most 100 hits a declaration: a lower bound, never
 * safe), "The uses are unknown" without a count, "Finding the uses…" while Rust asks the index (`pending`)
 */
export function formatOutsideUses(usages: Usages | null, pending: boolean): string {
  if (usages === null) {
    return pending ? "Finding the uses…" : "The uses are unknown"
  }
  if (usages.complete) {
    return `${formatUses(usages.outside)} left outside`
  }
  return usages.outside === 0 ? "No use left outside found, the search stopped early" : `At least ${formatUses(usages.outside)} left outside`
}

/** An API the commits remove or change, as the card words it */
export interface BreakageEntry {
  readonly key: string
  /** The file that declares it */
  readonly path: string
  /** Its declaration, before the commits for a removal: a click shows it */
  readonly place: ReviewPlace
  readonly name: string
  /** "Removed", "Changed", "Opens wider" */
  readonly kind: string
  readonly tone: ApiTone
  /** "src/Store.kt:34" */
  readonly location: string
  /** "parameters and visibility changed": the parts of the form that changed, of a change */
  readonly changed: string | null
  /** The forms before and after, each on lines of its own: a type a line per constructor */
  readonly before: string | null
  readonly after: string | null
  /** "3 uses left outside" (`formatOutsideUses`) */
  readonly outside: string
  /** "2 uses updated in this change"; null for none or an unknown count */
  readonly inside: string | null
  /** "4 calls left outside still fit the new form": accio says they fit it, each argument binding what it bound; null for none */
  readonly stillBound: string | null
  /** "3 matches by name only, not checked": hits only the name ties to the declaration, no use; null for none */
  readonly byName: string | null
  /**
   * The first uses left outside, each at its place at the head: "src/Base.kt:1" (`label`), its file name and line
   * ("Base.kt:1") and its directory ("src"), then "+4 more" when Rust found more
   */
  readonly sites: readonly { readonly label: string; readonly name: string; readonly directory: string; readonly place: ReviewPlace }[]
  readonly moreSites: string | null
}

/** "Possible breakage" of the card: the API with uses left outside that can break or are unknown, and the API none of whose uses breaks */
export interface Breakage {
  /** In the order Rust ranked them: the most uses left outside first */
  readonly entries: readonly BreakageEntry[]
  /**
   * The API none of whose uses left outside breaks and its line: "No use left outside breaks · 12 API changes" where
   * that is the verdict of the plan (`ReviewPlan.verdict`), else "12 API changes, no use left outside breaks" under the
   * entries that can break; null for none
   */
  readonly unbroken: { readonly label: string; readonly entries: readonly BreakageEntry[] } | null
}

function breakageEntry(change: ApiChange, pending: boolean): BreakageEntry {
  const usages = change.usages
  const more = usages === null ? 0 : usages.outside - usages.outsideSites.length
  return {
    key: `${change.kind}:${change.key}:${change.path}:${change.line}`,
    path: change.path,
    place: { path: change.path, side: change.kind === "removed" ? "old" : "new", line: change.line, name: change.name },
    name: change.name,
    kind: change.widened ? "Opens wider" : KIND_WORDS[change.kind],
    tone: change.widened ? "secondary" : change.kind === "removed" ? "destructive" : "warning",
    location: `${change.path}:${change.line}`,
    changed: change.changes.length === 0 ? null : `${joinNames(change.changes.map(it => FORM_WORDS[it]))} changed`,
    before: change.before,
    after: change.kind === "removed" ? null : change.after,
    outside: formatOutsideUses(usages, pending),
    inside: usages === null || usages.inside === 0 ? null : `${formatUses(usages.inside)} updated in this change`,
    stillBound: usages === null || usages.stillBound === 0 ? null : `${formatCalls(usages.stillBound)} left outside still ${usages.stillBound === 1 ? "fits" : "fit"} the new form`,
    byName: usages === null || usages.byName === 0 ? null : `${usages.byName === 1 ? "1 match" : `${usages.byName} matches`} by name only, not checked`,
    sites:
      usages?.outsideSites.map(site => {
        const slash = site.path.lastIndexOf("/")
        return {
          label: `${site.path}:${site.line}`,
          name: `${site.path.slice(slash + 1)}:${site.line}`,
          directory: slash < 0 ? "" : site.path.slice(0, slash),
          place: { path: site.path, side: "new" as const, line: site.line, name: change.name },
        }
      }) ?? [],
    moreSites: more > 0 ? `+${more} more` : null,
  }
}

/**
 * The removed and changed API of the card. An entry with a use left outside that can break, or with an unknown or
 * incomplete count, is listed; the entries none of whose uses left outside breaks (each updated in this change, or a
 * call that still binds) and the widenings, which break none, fold into one line, led by the verdict when Rust says no
 * use left outside breaks (`verdict`). Any other addition is not listed: no use of it is left outside. `pending`: Rust
 * asks the index, and the plan with the usages follows
 */
export function presentBreakage(api: readonly ApiChange[], pending: boolean, verdict: BreakVerdict): Breakage {
  const entries: BreakageEntry[] = []
  const unbroken: BreakageEntry[] = []
  for (const change of api) {
    if (change.kind === "added" && !change.widened) {
      continue
    }
    // Rust says no use left outside breaks; an unknown or incomplete count never is
    if (change.widened || change.usages?.unbroken === true) {
      unbroken.push(breakageEntry(change, pending))
    } else {
      entries.push(breakageEntry(change, pending))
    }
  }
  const count = unbroken.length === 1 ? "1 API change" : `${unbroken.length} API changes`
  const label = verdict === "noUseBreaks" ? `No use left outside breaks · ${count}` : `${count}, no use left outside breaks`
  return { entries, unbroken: unbroken.length === 0 ? null : { label, entries: unbroken } }
}

/** What the tree of a review shows of its plan: the facts of each file, the links between the files, the dependency changes */
/** What the tree reads of the plan: the facts of the files, their links, the dependency changes, the repeated edits */
export type FileFactsOfPlan = Pick<ReviewPlan, "files" | "links" | "dependencies" | "repeatedEdits">

/** What the review window reads of the plan: the facts of the files, the units of the story, its concerns, its steps and its brief */
export type ReviewFacts = FileFactsOfPlan & Pick<ReviewPlan, "units" | "api" | "concerns" | "steps" | "brief" | "modules" | "registrations" | "texts">

export const MECHANICAL_WORDS: Readonly<Record<MechanicalKind, string>> = {
  buildModel: "Build files",
  moduleList: "Module lists",
  importsOnly: "Imports only",
  whitespace: "Whitespace only",
  renamed: "Renamed",
  generated: "Generated",
}

/** Why the diffs fold a mechanical file, in its header */
export const FOLDED_WORDS: Readonly<Record<MechanicalKind, string>> = {
  buildModel: "Build file",
  moduleList: "Module list",
  importsOnly: "Imports only",
  whitespace: "Whitespace only",
  renamed: "Renamed only",
  generated: "Generated",
}

/** The longest header line a folded generated file shows, then "…" */
const MAX_HEADER_WORDS = 100

/**
 * Why the diffs fold a generated file, in its header: what its name makes it ("Lock file"), its attributes, or the line
 * its tool wrote ("Generated by tools/render.mjs; edit src/guide.md")
 */
export function formatGenerated(generated: GeneratedBy): string {
  switch (generated.kind) {
    case "name":
      return GENERATED_NAME_WORDS[generated.name]
    case "attribute":
      return "Generated (.gitattributes)"
    case "header":
      return generated.line.length > MAX_HEADER_WORDS ? `${generated.line.slice(0, MAX_HEADER_WORDS - 1).trimEnd()}…` : generated.line
  }
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
  /** "same edit ×4", the edit, and the other files that get it */
  readonly sameEdit: { readonly label: string; readonly edit: string; readonly files: readonly string[] } | null
  /** "uses 1, +1 new, breaks 3": the badges in words, for the description of the row */
  readonly description: string
}

function fileName(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1)
}

/** The first use of a link, in the file that uses; that file at its header without a name */
export function firstUse(link: FileLink): ReviewPlace {
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
    return { label: "breaks ?", title: "The uses left outside the change are unknown, or the search stopped early", tone: "muted" }
  }
  return { label: `breaks ${file.breaks}+`, title: `At least ${formatUses(file.breaks)} left outside the change: the search stopped early`, tone: "warning" }
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
      sameEdit: group === undefined ? null : { label: `same edit ×${group.files.length}`, edit: formatRepeatedEdit(group), files: group.files.filter(path => path !== file.path) },
    }
    if (usesLinks.length > 0 || usedByLinks.length > 0 || entry.breaks !== null || entry.sameEdit !== null) {
      const words = [entry.uses, entry.newLinks, entry.usedBy, entry.breaks?.label, entry.sameEdit?.label]
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

/** A section of the card of a row's mark, under its title ("Uses 1 · +1 new", "Breaks", "Same edit ×3", "Dependencies · new module") */
export type RowFactsSection =
  | { readonly kind: "breaks"; readonly title: string; readonly label: string; readonly why: string; readonly tone: "warning" | "muted" }
  | { readonly kind: "uses" | "usedBy"; readonly title: string; readonly links: readonly LinkEntry[] }
  | { readonly kind: "sameEdit"; readonly title: string; readonly edit: string; readonly files: readonly string[] }
  | { readonly kind: "dependencies"; readonly title: string; readonly lines: readonly string[] }

/** The one mark of a row of the tree and the card its hover opens: the facts of its file and the dependency changes it carries */
export interface RowFacts {
  /** `warning` when the file breaks uses left outside the change (a lower bound above 0 too), else `muted` */
  readonly tone: "warning" | "muted"
  /** "uses 1, +1 new, breaks 3, new module": every fact in words, for the description of the row */
  readonly description: string
  /** What can break first, then the links, the same edit and the dependency changes; only those that apply */
  readonly sections: readonly RowFactsSection[]
}

/** The mark of a row from the badges of its file and the dependency changes it carries; null when it has neither */
export function presentRowFacts(file?: FileBadges, dependency?: DependencyBadge): RowFacts | null {
  if (file === undefined && dependency === undefined) {
    return null
  }
  const sections: RowFactsSection[] = []
  if (file?.breaks != null) {
    sections.push({ kind: "breaks", title: "Breaks", label: file.breaks.label, why: file.breaks.title, tone: file.breaks.tone })
  }
  if (file !== undefined && file.usesLinks.length > 0) {
    const title = [capitalize(file.uses ?? "uses"), file.newLinks].filter(part => part !== null).join(" · ")
    sections.push({ kind: "uses", title, links: file.usesLinks })
  }
  if (file !== undefined && file.usedByLinks.length > 0) {
    sections.push({ kind: "usedBy", title: capitalize(file.usedBy ?? "used by"), links: file.usedByLinks })
  }
  if (file?.sameEdit != null) {
    sections.push({ kind: "sameEdit", title: capitalize(file.sameEdit.label), edit: file.sameEdit.edit, files: file.sameEdit.files })
  }
  if (dependency !== undefined) {
    sections.push({ kind: "dependencies", title: `Dependencies · ${dependency.label}`, lines: dependency.lines })
  }
  return {
    tone: file?.breaks?.tone === "warning" ? "warning" : "muted",
    description: [file?.description, dependency?.label].filter(word => word !== undefined && word !== "").join(", "),
    sections,
  }
}

/** The toast of Mark Reviewed: "Marked auth reviewed", or that it was already */
export function formatMarkedReviewed(branch: string, changed: boolean): string {
  return changed ? `Marked ${branch} reviewed` : `${branch} was already marked reviewed at this commit`
}
