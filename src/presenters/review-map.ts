import type { ModuleShape, ReviewPlan } from "@/lib/bindings"
import type { ReviewPlace } from "@/lib/review-place"
import { fileTop } from "@/lib/review-place"
import { joinNames } from "@/lib/text"

/**
 * The map of a review (spec/branch/review-tour.spec.md, "The Map"): the shape of the change on one screen before any
 * of its words, which modules it adds, changes or deletes, how large each part is and what each newly uses
 * (`ReviewPlan.modules`, Rust's, in its order), the contract it changes in one line, and the API it opens wider. Rust
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
}

export interface MapContract {
  readonly newSpecs: readonly { readonly key: string; readonly name: string; readonly rules: number; readonly place: ReviewPlace }[]
  /** "3 specs and 3 docs changed"; null when only new specs */
  readonly changed: string | null
}

export interface MapApi {
  /** The names the change opens wider, each at its declaration */
  readonly widened: readonly { readonly key: string; readonly name: string; readonly place: ReviewPlace }[]
  /** "No use left outside breaks" when Rust says so of every removed and changed entry; null otherwise */
  readonly verdict: string | null
}

/** The rows of the shape before "and N more" */
export const MAP_ROWS = 6
/** The folders a changed module names on its row */
const MAP_FOLDERS = 3
/** A change of one module with more files names its folders */
const MIN_FOLDER_FILES = 12

/** The map of `plan` (`MapView`) */
export function presentMap(plan: Pick<ReviewPlan, "modules" | "files" | "order" | "api" | "verdict" | "dependencies" | "brief">): MapView {
  const firstFile = new Map<number, string>()
  const moduleOf = new Map(plan.files.map(file => [file.path, file.module]))
  for (const path of plan.order) {
    const module = moduleOf.get(path)
    if (module !== undefined && module !== null && !firstFile.has(module)) {
      firstFile.set(module, path)
    }
  }
  const readable = plan.modules.map((module, index) => ({ module, index })).filter(({ module }) => readableFiles(module) > 0)
  const shaped = readable.length >= 2 || readable.some(({ module }) => module.state !== "changed")
  const rows: MapRow[] = shaped ? readable.map(({ module, index }) => moduleRow(module, index, plan, firstFile.get(index) ?? null)) : folderRows(readable[0]?.module, plan)
  const max = Math.max(1, ...rows.map(row => row.additions + row.deletions))
  const scaled = rows.map(row => ({ ...row, share: (row.additions + row.deletions) / max }))
  const shown = scaled.slice(0, MAP_ROWS)
  const left = scaled.slice(MAP_ROWS)
  return {
    total: {
      files: plan.files.length,
      additions: plan.modules.reduce((sum, module) => sum + module.additions, 0),
      deletions: plan.modules.reduce((sum, module) => sum + module.deletions, 0),
    },
    rows: shown,
    more: left.length === 0 ? null : { count: left.length, labels: left.map(row => row.label).join(", ") },
    contract: contractOf(plan),
    api: apiOf(plan),
  }
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

/** Of a change of one module and many files: a row per folder of it, else none */
function folderRows(module: ModuleShape | undefined, plan: Pick<ReviewPlan, "files" | "order">): MapRow[] {
  if (module === undefined || readableFiles(module) <= MIN_FOLDER_FILES) {
    return []
  }
  const named = module.directories.filter(directory => directory.path !== "")
  if (named.length < 2) {
    return []
  }
  return named.map(directory => {
    const prefix = module.root === "" ? "" : `${module.root}/`
    const first = plan.order.find(path => path.startsWith(prefix) && path.includes(`/${directory.path}/`))
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

/** The name of a spec by its file: `transcript-media-playback.spec.md` is "Transcript Media Playback" */
export function specName(path: string): string {
  const name = path.slice(path.lastIndexOf("/") + 1).replace(/\.spec\.md$|\.md$/i, "")
  return name
    .split(/[-_]+/)
    .filter(word => word !== "")
    .map(word => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ")
}

function apiOf(plan: Pick<ReviewPlan, "api" | "verdict">): MapApi | null {
  const widened = plan.api
    .filter(change => change.widened)
    .map(change => ({ key: `${change.key}:${change.path}`, name: change.name, place: { path: change.path, side: "new" as const, line: change.line, name: change.name } }))
  const verdict = plan.verdict === "noUseBreaks" && plan.api.some(change => change.kind !== "added" || change.widened) ? "No use left outside breaks" : null
  return widened.length === 0 && verdict === null ? null : { widened, verdict }
}

function plural(count: number, noun: string): string {
  return count === 1 ? `1 ${noun}` : `${count} ${noun}s`
}
