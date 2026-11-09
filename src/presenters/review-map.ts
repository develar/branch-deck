import type { ModuleShape, Registered, Registration, ReviewPlan, UiText, UiTextChange } from "@/lib/bindings"
import type { ReviewPlace } from "@/lib/review-place"
import { fileTop } from "@/lib/review-place"
import { joinNames } from "@/lib/text"
import type { BreakageEntry } from "@/presenters/review-plan"
import { presentBreakage } from "@/presenters/review-plan"

/**
 * The map of a review (openspec/specs/branch/review-tour/spec.md, "The Map"): the shape of the change on one screen before any
 * of its words, which modules it adds, changes or deletes, how large each part is and what each newly uses
 * (`ReviewPlan.modules`, Rust's, in its order), the contract it changes in one line, the texts of the UI it adds,
 * removes or changes (`ReviewPlan.texts`), and the API it opens wider. Rust
 * decides the modules, their order and their facts; this words them and keeps the map inside its budget: a list longer
 * than it ends with "and N more", never cut silently
 */
export interface MapView {
  /** "141 files +10407 −186": every changed file and line of the review */
  readonly total: { readonly files: number; readonly additions: number; readonly deletions: number }
  /** The rows of the shape: the modules with code, tests or resources, else the folders of the one module; empty for a small change */
  readonly rows: readonly MapRow[]
  /** The rows the budget leaves out: their count and their labels for the hover */
  readonly more: { readonly count: number; readonly labels: string } | null
  /** "New spec Transcript Media Playback (14 requirements) · 3 specs and 3 docs changed"; null without prose */
  readonly contract: MapContract | null
  /** "4 new: “Video: {0}”, “Sound: {0}” and 2 more"; null when no bundle changes */
  readonly texts: MapTexts | null
  /** The declarations the change opens wider, and whether a use left outside can break */
  readonly api: MapApi | null
}

/** A module or a folder of the shape */
export interface MapRow {
  readonly key: string
  readonly label: string
  /** The root of the module, its hover */
  readonly root: string
  /** "New module", "Deleted module"; null for a changed one */
  readonly badge: string | null
  readonly additions: number
  readonly deletions: number
  readonly files: number
  /** Its changed lines against the largest row's, 0..1, for its bar */
  readonly share: number
  /** Of a new module what it uses and where it plugs in ("uses view.gui and shared/code · content of com.intellij.air"), of another its folders ("ui/embeds 29 · ui/markdown 11") */
  readonly detail: string | null
  /** What it adds beside its code: a dependency on a module outside the review, a declaration opened wider */
  readonly extra: string | null
  /** Where a click goes: its first file in the story */
  readonly place: ReviewPlace | null
  /** What its descriptors newly register, change or remove (`ReviewPlan.registrations`), within the budget of the map */
  readonly registrations: readonly MapRegistration[]
  /** Its registrations past the budget */
  readonly moreRegistrations: number
}

/** A registration of a module on the map: "+ applicationService TranscriptEmbedPlayers → TranscriptEmbedPlayersImpl" */
export interface MapRegistration {
  readonly key: string
  /** "+" added, "−" removed, "~" changed */
  readonly sign: "+" | "−" | "~"
  /** The tag of the descriptor that registers it: "applicationService", "registryKey", "action" */
  readonly kind: string
  /** What it registers, by simple names: "TranscriptEmbedPlayers → TranscriptEmbedPlayersImpl", "air.chat.media.inline.playback.enabled = NIGHTLY_ENABLED" */
  readonly name: string
  /** Its hover: the qualified names, the description of a key, the descriptor */
  readonly title: string
  /** The declaration of the class it registers, else its descriptor */
  readonly place: ReviewPlace
}

export interface MapContract {
  readonly newSpecs: readonly { readonly key: string; readonly name: string; readonly rules: number; readonly place: ReviewPlace }[]
  /** "3 specs and 3 docs changed"; null when only new specs */
  readonly changed: string | null
}

export interface MapTexts {
  /** "4 new", "3 new, 1 changed and 1 removed" */
  readonly counts: string
  /** Every text in Rust's order (the bundles in tree order, then by line): the first `MAP_TEXTS` quoted on the line, all under it */
  readonly texts: readonly MapText[]
}

/** A text of the UI the change adds, removes or changes */
export interface MapText {
  readonly key: string
  /** Its key in its bundle: "embed.media.download" */
  readonly name: string
  readonly change: UiTextChange
  /** What the user reads after the change; of a removal what they read before it */
  readonly words: string
  /** Of a change, what they read before it; null otherwise */
  readonly before: string | null
  /** Its key and its bundle at its line, for the hover */
  readonly title: string
  /** Its entry in its bundle: after the change, before it for a removal */
  readonly place: ReviewPlace
}

export interface MapApi {
  /** The names the change opens wider that no row of the shape names, each at its declaration */
  readonly widened: readonly { readonly key: string; readonly name: string; readonly place: ReviewPlace }[]
  /**
   * Where Rust says no use left outside breaks and nothing can break, the API none of whose uses breaks folded under
   * that verdict, said once: "No use left outside breaks · 2 API changes"; null otherwise (Possible Breakage lists it)
   */
  readonly verdict: { readonly label: string; readonly entries: readonly BreakageEntry[] } | null
}

/** The texts of the UI quoted on the line of the map before "and N more" */
export const MAP_TEXTS = 2

/** The rows of the shape before "and N more" */
export const MAP_ROWS = 6
/** The registrations a row names before "and N more registrations", and all rows together */
const ROW_REGISTRATIONS = 3
const MAP_REGISTRATIONS = 8
/** The folders a changed module names on its row */
const MAP_FOLDERS = 3
/** A change of one module with more files names its folders */
const MIN_FOLDER_FILES = 12

/** The map of `plan` (`MapView`) */
export function presentMap(plan: Pick<ReviewPlan, "modules" | "files" | "order" | "steps" | "api" | "verdict" | "dependencies" | "brief" | "registrations" | "texts">): MapView {
  const firstFile = new Map<number, string>()
  const moduleOf = new Map(plan.files.map(file => [file.path, file.module]))
  const story = storyOrder(plan)
  for (const path of story) {
    const module = moduleOf.get(path)
    if (module !== undefined && module !== null && !firstFile.has(module)) {
      firstFile.set(module, path)
    }
  }
  const readable = plan.modules.map((module, index) => ({ module, index })).filter(({ module }) => readableFiles(module) > 0)
  const shaped = readable.length >= 2 || readable.some(({ module }) => module.state !== "changed")
  const rows: MapRow[] = shaped ? readable.map(({ module, index }) => moduleRow(module, index, plan, firstFile.get(index) ?? null)) : folderRows(readable[0]?.module, story)
  const max = Math.max(1, ...rows.map(row => row.additions + row.deletions))
  const scaled = rows.map(row => ({ ...row, share: (row.additions + row.deletions) / max }))
  const shown = withRegistrations(scaled.slice(0, MAP_ROWS), plan)
  const left = scaled.slice(MAP_ROWS)
  // the modules whose rows name what they open wider
  const named = new Set(shaped ? readable.slice(0, MAP_ROWS).map(({ index }) => index) : [])
  return {
    total: {
      files: plan.files.length,
      additions: plan.modules.reduce((sum, module) => sum + module.additions, 0),
      deletions: plan.modules.reduce((sum, module) => sum + module.deletions, 0),
    },
    rows: shown,
    more: left.length === 0 ? null : { count: left.length, labels: left.map(row => row.label).join(", ") },
    contract: contractOf(plan),
    texts: textsOf(plan.texts),
    api: apiOf(plan, module => module !== null && named.has(module)),
  }
}

/**
 * The files as the story shows them: the files of each step in its order, then the files of no step (its mechanical
 * ones) in the dependency order of the plan
 */
function storyOrder(plan: Pick<ReviewPlan, "order" | "steps">): string[] {
  return [...new Set([...plan.steps.steps.flatMap(step => step.paths), ...plan.order])]
}

/** The files of a module a reviewer reads as its work: its code, tests and resources */
function readableFiles(module: ModuleShape): number {
  return module.files.code + module.files.tests + module.files.resources
}

function moduleRow(module: ModuleShape, index: number, plan: Pick<ReviewPlan, "modules" | "api" | "dependencies" | "files">, first: string | null): MapRow {
  const label = (place: number) => plan.modules[place]?.label ?? ""
  const added = module.state === "added"
  const uses = module.uses.map(used => label(used.module))
  const detail = added
    ? [uses.length === 0 ? null : `uses ${joinNames(uses)}`, module.contentOf.length === 0 ? null : `content of ${joinNames(module.contentOf)}`]
        .filter(part => part !== null)
        .join(" · ")
    : foldersOf(module)
  const external = module.newDependencies.flatMap(place => {
    const change = plan.dependencies[place]
    return change === undefined ? [] : [change.name]
  })
  const widened = plan.api.filter(change => change.widened && plan.files.find(file => file.path === change.path)?.module === index).map(change => change.name)
  const extra = [added || external.length === 0 ? null : `+ depends on ${joinNames(external)}`, widened.length === 0 ? null : `Opens wider: ${joinNames(widened)}`]
    .filter(part => part !== null)
    .join(" · ")
  return {
    key: module.root,
    label: module.label,
    root: module.root === "" ? "The root of the repository" : module.root,
    badge: added ? "New module" : module.state === "deleted" ? "Deleted module" : null,
    additions: module.additions,
    deletions: module.deletions,
    files: module.files.code + module.files.tests + module.files.resources + module.files.build + module.files.prose,
    share: 0,
    detail: detail === "" ? null : detail,
    extra: extra === "" ? null : extra,
    place: first === null ? null : fileTop(first),
    registrations: [],
    moreRegistrations: 0,
  }
}

/** The folders of a changed module, the most files first: "ui/embeds 29 · ui/markdown 11 · references 2" */
function foldersOf(module: ModuleShape): string | null {
  const named = module.directories.filter(directory => directory.path !== "")
  if (named.length < 2) {
    return null
  }
  return named
    .slice(0, MAP_FOLDERS)
    .map(directory => `${directory.path} ${directory.files}`)
    .join(" · ")
}

/** Of a change of one module and many files: a row per folder of it, each to its first file in `story`, else none */
function folderRows(module: ModuleShape | undefined, story: readonly string[]): MapRow[] {
  if (module === undefined || readableFiles(module) <= MIN_FOLDER_FILES) {
    return []
  }
  const named = module.directories.filter(directory => directory.path !== "")
  if (named.length < 2) {
    return []
  }
  return named.map(directory => {
    const prefix = module.root === "" ? "" : `${module.root}/`
    const first = story.find(path => path.startsWith(prefix) && path.includes(`/${directory.path}/`))
    return {
      key: `${module.root}:${directory.path}`,
      label: directory.path,
      root: `${prefix}…/${directory.path}`,
      badge: null,
      additions: 0,
      deletions: 0,
      files: directory.files,
      share: 0,
      detail: null,
      extra: null,
      place: first === undefined ? null : fileTop(first),
      registrations: [],
      moreRegistrations: 0,
    }
  })
}

/** The contract of the change in one line: its new specs by name, then how many specs and docs it changes */
function contractOf(plan: Pick<ReviewPlan, "brief">): MapContract | null {
  const entries = plan.brief.contract
  if (entries.length === 0) {
    return null
  }
  const newSpecs = new Map<string, { key: string; name: string; rules: number; place: ReviewPlace }>()
  const changedSpecs = new Set<string>()
  const changedDocs = new Set<string>()
  for (const entry of entries) {
    if (entry.newSpec !== null) {
      newSpecs.set(entry.path, { key: entry.path, name: specName(entry.path), rules: entry.newSpec.rules, place: fileTop(entry.path) })
    } else if (entry.kind === "spec") {
      changedSpecs.add(entry.path)
    } else {
      changedDocs.add(entry.path)
    }
  }
  const counted = [changedSpecs.size === 0 ? null : plural(changedSpecs.size, "spec"), changedDocs.size === 0 ? null : plural(changedDocs.size, "doc")].filter(
    part => part !== null,
  )
  return { newSpecs: [...newSpecs.values()], changed: counted.length === 0 ? null : `${counted.join(" and ")} changed` }
}

/**
 * The name of a spec by its file: `transcript-media-playback.spec.md` is "Transcript Media Playback", and a spec of
 * OpenSpec by its capability, `openspec/specs/branch/stack-parents/spec.md` "Stack Parents"
 */
export function specName(path: string): string {
  const parts = path.split("/")
  const file = parts.at(-1) ?? ""
  const name = file.toLowerCase() === "spec.md" && parts.length > 1 ? (parts.at(-2) ?? "") : file.replace(/\.spec\.md$|\.md$/i, "")
  return name
    .split(/[-_]+/)
    .filter(word => word !== "")
    .map(word => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ")
}

/** The texts of the UI the change adds, removes or changes, counted by what it does to them, each at its entry */
export function textsOf(texts: readonly UiText[]): MapTexts | null {
  if (texts.length === 0) {
    return null
  }
  const count = (change: UiTextChange, words: string) => {
    const found = texts.filter(text => text.change === change).length
    return found === 0 ? null : `${found} ${words}`
  }
  const counts = [count("added", "new"), count("changed", "changed"), count("removed", "removed")].filter(part => part !== null)
  return {
    counts: joinNames(counts),
    texts: texts.map(text => ({
      key: `${text.path}:${text.key}`,
      name: text.key,
      change: text.change,
      words: (text.change === "removed" ? text.before : text.after) ?? "",
      before: text.change === "changed" ? text.before : null,
      title: `${text.key}\n${text.path}:${text.line}`,
      place: { path: text.path, side: text.change === "removed" ? ("old" as const) : ("new" as const), line: text.line, name: null },
    })),
  }
}

function apiOf(plan: Pick<ReviewPlan, "api" | "verdict" | "files">, onRow: (module: number | null) => boolean): MapApi | null {
  const moduleOf = new Map(plan.files.map(file => [file.path, file.module]))
  const widened = plan.api
    .filter(change => change.widened && !onRow(moduleOf.get(change.path) ?? null))
    .map(change => ({ key: `${change.key}:${change.path}`, name: change.name, place: { path: change.path, side: "new" as const, line: change.line, name: change.name } }))
  // the verdict is of a whole answer, so no use is pending: Possible Breakage lists nothing then
  const breakage = presentBreakage(plan.api, false, plan.verdict)
  const verdict = plan.verdict === "noUseBreaks" && breakage.entries.length === 0 ? breakage.unbroken : null
  return widened.length === 0 && verdict === null ? null : { widened, verdict }
}

function plural(count: number, noun: string): string {
  return count === 1 ? `1 ${noun}` : `${count} ${noun}s`
}

/**
 * The rows with what their descriptors register: a content module none (the new module's line says where it plugs in),
 * at most `ROW_REGISTRATIONS` a row and `MAP_REGISTRATIONS` in all, in the order Rust gives
 */
function withRegistrations(rows: readonly MapRow[], plan: Pick<ReviewPlan, "modules" | "registrations">): MapRow[] {
  let left = MAP_REGISTRATIONS
  return rows.map(row => {
    const module = plan.modules.findIndex(it => it.root === row.key)
    if (module < 0) {
      return { ...row, registrations: [], moreRegistrations: 0 }
    }
    const { registrations, more } = moduleRegistrations(plan, module, Math.min(ROW_REGISTRATIONS, left))
    left -= registrations.length
    return { ...row, registrations, moreRegistrations: more }
  })
}

/** The words of a registration on the map */
/**
 * What the descriptors of the module at `module` register (a content module none: the line of the module says where it
 * plugs in), the first `limit`, and how many more
 */
export function moduleRegistrations(
  plan: Pick<ReviewPlan, "registrations">,
  module: number,
  limit: number,
): { readonly registrations: readonly MapRegistration[]; readonly more: number } {
  const all = plan.registrations.filter(registration => registration.module === module && registration.registered.kind !== "contentModule")
  const shown = all.slice(0, limit)
  return { registrations: shown.map(mapRegistration), more: all.length - shown.length }
}

function mapRegistration(registration: Registration, index: number): MapRegistration {
  const { kind, name, title } = registrationWords(registration.registered)
  // the class it registers: its implementation, else the first the index declares
  const declared = registration.classes.flatMap(it => (it.found?.resolved === "declared" ? [{ role: it.role, ...it.found }] : []))
  const found = declared.find(it => it.role === "implementation") ?? declared[0]
  const changed = registration.change === "changed" && registration.changed.length > 0 ? ` (${registration.changed.join(", ")} changed)` : ""
  return {
    key: `${registration.descriptor}:${index}:${kind}:${name}`,
    sign: registration.change === "added" ? "+" : registration.change === "removed" ? "−" : "~",
    kind,
    name: `${name}${changed}`,
    title: [title, registration.descriptor].filter(part => part !== "").join("\n"),
    place: found === undefined ? fileTop(registration.descriptor) : { path: found.path, side: registration.change === "removed" ? "old" : "new", line: found.line, name: null },
  }
}

/** The tag, the simple names and the hover of what a descriptor registers */
function registrationWords(registered: Registered): { kind: string; name: string; title: string } {
  switch (registered.kind) {
    case "service": {
      const names = [registered.interface, registered.implementation].filter(name => name !== null)
      return { kind: `${AREA_WORDS[registered.area]}Service`, name: names.map(simpleName).join(" → "), title: names.join(" → ") }
    }
    case "extensionPoint":
      return {
        kind: "extensionPoint",
        name: registered.qualifiedName ?? registered.name ?? "",
        title: [registered.interface, registered.beanClass].filter(it => it !== null).join(", "),
      }
    case "listener":
      return { kind: "listener", name: `${simpleName(registered.class)} of ${simpleName(registered.topic)}`, title: `${registered.class} of ${registered.topic}` }
    case "extension": {
      const target = registered.implementation ?? registered.id
      return {
        kind: "extension",
        name: target === null ? registered.point : `${simpleName(registered.point)} → ${simpleName(target)}`,
        title: [registered.point, registered.implementation].filter(it => it !== null).join(" → "),
      }
    }
    case "registryKey":
      return {
        kind: "registryKey",
        name: registered.defaultValue === null ? registered.key : `${registered.key} = ${registered.defaultValue}`,
        title: registered.description ?? "",
      }
    case "action": {
      const name = registered.id ?? (registered.class === null ? "" : simpleName(registered.class))
      return { kind: registered.group ? "group" : "action", name: registered.text === null ? name : `${name} “${registered.text}”`, title: registered.class ?? "" }
    }
    case "contentModule":
      return { kind: "content", name: `${registered.name} (${registered.loading})`, title: "" }
  }
}

const AREA_WORDS = { app: "application", project: "project", module: "module" } as const

/** The last part of a qualified name: `TranscriptEmbedPlayersImpl` of `com.intellij.air.….TranscriptEmbedPlayersImpl` */
export function simpleName(name: string): string {
  return name.slice(Math.max(name.lastIndexOf("."), name.lastIndexOf("$")) + 1)
}
