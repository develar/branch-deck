import type { CommitFile } from "@/lib/bindings"
import type { BranchReviewGroup } from "@/types/sub-window"

/**
 * What the chips of a review mark: a chip filters nothing, it marks the files of its owner in the whole story, mutes the
 * others, and J/K and N/P move through its files only (spec/branch/branch-review.spec.md, "Code owners")
 */
export type ReviewFilter =
  | { readonly kind: "all" }
  /** A reviewer group the code-owner check asks for */
  | { readonly kind: "group"; readonly name: string }
  /** An owner by the `OWNERSHIP` files */
  | { readonly kind: "owner"; readonly name: string }
  /** The files no `OWNERSHIP` rule owns */
  | { readonly kind: "unowned" }

export interface ReviewFilterChip {
  readonly filter: ReviewFilter
  readonly label: string
  /** The files of the review it marks; null for a group whose files Patronus doesn't tell */
  readonly count: number | null
  /** The code-owner check asks for the approval of the group */
  readonly needsApproval: boolean
  /** A group the user can approve for (`ReviewerGroupView.yours`) */
  readonly yours: boolean
  /** A group that has its approvals (`ReviewerGroupView.approved`) */
  readonly approved: boolean
}

function matchesPath(paths: ReadonlySet<string>, file: CommitFile): boolean {
  return paths.has(file.path) || (file.oldPath !== null && paths.has(file.oldPath))
}

function sameName(a: string, b: string): boolean {
  return a.trim().toLowerCase() === b.trim().toLowerCase()
}

/**
 * The paths a chip marks among the files, null for all of them (All, a branch-wide group) or a group whose files are
 * unknown; empty when the owner has no files in the part
 */
export function ownerPaths(
  filter: ReviewFilter,
  files: readonly CommitFile[],
  groups: readonly BranchReviewGroup[],
  owners: ReadonlyMap<string, string>,
): ReadonlySet<string> | null {
  switch (filter.kind) {
    case "all":
      return null
    case "group": {
      const group = groups.find(it => it.name === filter.name)
      if (group === undefined || group.branchWide || group.paths.length === 0) {
        return null
      }
      const paths = new Set(group.paths)
      return new Set(files.filter(file => matchesPath(paths, file)).map(file => file.path))
    }
    case "owner":
      return new Set(files.filter(file => owners.get(file.path) === filter.name).map(file => file.path))
    case "unowned":
      return new Set(files.filter(file => !owners.has(file.path)).map(file => file.path))
  }
}

/**
 * The chips of a review: all files, the reviewer groups the code-owner check asks for, the user's first (their files,
 * all of them for a branch-wide group, unknown when Patronus doesn't tell), then the other owners by the `OWNERSHIP`
 * files, most files first, and the files without an owner. No owner chips without `OWNERSHIP` files.
 * While a chip is `active` the chips show, its own too when the files have none of its own (0), so it can be turned
 * off. Just "All" when no chip would mark less: one owner (or none) has every file and no group asks for an approval.
 */
export function reviewFilterChips(
  files: readonly CommitFile[],
  groups: readonly BranchReviewGroup[],
  owners: ReadonlyMap<string, string>,
  active: ReviewFilter = { kind: "all" },
): ReviewFilterChip[] {
  const all: ReviewFilterChip = { filter: { kind: "all" }, label: "All", count: files.length, needsApproval: false, yours: false, approved: false }
  const chips: ReviewFilterChip[] = [all]
  // the user's groups first, each part in the order of the check
  for (const group of [...groups.filter(it => it.yours === true), ...groups.filter(it => it.yours !== true)]) {
    const paths = ownerPaths({ kind: "group", name: group.name }, files, groups, owners)
    chips.push({
      filter: { kind: "group", name: group.name },
      label: group.name,
      count: group.branchWide ? files.length : paths === null ? null : paths.size,
      needsApproval: true,
      yours: group.yours === true,
      approved: group.approved === true,
    })
  }
  if (owners.size > 0) {
    const counts = new Map<string, number>()
    let unowned = 0
    for (const file of files) {
      const owner = owners.get(file.path)
      if (owner === undefined) {
        unowned++
      } else {
        counts.set(owner, (counts.get(owner) ?? 0) + 1)
      }
    }
    const others = [...counts].filter(([owner]) => !groups.some(group => sameName(group.name, owner)))
    others.sort(([a, countA], [b, countB]) => countB - countA || a.localeCompare(b))
    for (const [owner, count] of others) {
      chips.push({ filter: { kind: "owner", name: owner }, label: owner, count, needsApproval: false, yours: false, approved: false })
    }
    if (unowned > 0) {
      chips.push({ filter: { kind: "unowned" }, label: "No Owner", count: unowned, needsApproval: false, yours: false, approved: false })
    }
  }
  if (active.kind !== "all") {
    // the chosen chip shows while it is chosen, so it can be turned off
    if (!chips.some(chip => sameFilter(chip.filter, active))) {
      chips.push({ filter: active, label: ownerLabel(active)!, count: 0, needsApproval: false, yours: false, approved: false })
    }
    return chips
  }
  // one chip beside "All" that marks every file marks nothing
  return chips.length === 2 && !chips[1]!.needsApproval && chips[1]!.count === files.length ? [all] : chips
}

/**
 * The chip a review window opens on (spec/repository/review-requests.spec.md, "Owner chips"): the remembered one when it
 * marks a file here, by its name also where the owner is a group of the check and the other way round; else, with
 * `preferYours`, the first group of the user that still needs an approval and has files here; else All. A chip whose
 * files Patronus doesn't tell marks none.
 */
export function initialOwnerFilter(chips: readonly ReviewFilterChip[], remembered: ReviewFilter | undefined, preferYours: boolean): ReviewFilter {
  const marks = (chip: ReviewFilterChip) => chip.count !== null && chip.count > 0
  if (remembered !== undefined && remembered.kind !== "all") {
    const name = ownerLabel(remembered)!
    const chip =
      chips.find(it => sameFilter(it.filter, remembered)) ??
      (remembered.kind === "unowned" ? undefined : chips.find(it => (it.filter.kind === "group" || it.filter.kind === "owner") && sameName(it.label, name)))
    if (chip !== undefined && marks(chip)) {
      return chip.filter
    }
  }
  if (preferYours) {
    const yours = chips.find(chip => chip.yours && !chip.approved && marks(chip))
    if (yours !== undefined) {
      return yours.filter
    }
  }
  return { kind: "all" }
}

/** The owner of every file, when one owner (by the `OWNERSHIP` files) has them all: "18 files · Air Plugin" */
export function soleOwner(files: readonly CommitFile[], owners: ReadonlyMap<string, string>): string | null {
  const first = files.length === 0 ? undefined : owners.get(files[0]!.path)
  return first !== undefined && files.every(file => owners.get(file.path) === first) ? first : null
}

export function sameFilter(a: ReviewFilter, b: ReviewFilter): boolean {
  return a.kind === b.kind && (a.kind === "all" || a.kind === "unowned" || a.name === (b as { name: string }).name)
}

/** The note under the chips when the chosen one marks no file: "No files of Air Plugin in this range"; null for All */
export function ownerEmptyText(filter: ReviewFilter): string | null {
  switch (filter.kind) {
    case "all":
      return null
    case "group":
    case "owner":
      return `No files of ${filter.name} in this range`
    case "unowned":
      return "Every file in this range has an owner"
  }
}

/** What the chips of a review are made of: the files of the shown scope, the reviewer groups, the owners by `OWNERSHIP` */
export interface ReviewOwners {
  readonly files: readonly CommitFile[] | undefined
  readonly groups: readonly BranchReviewGroup[]
  readonly owners: ReadonlyMap<string, string>
}

/**
 * The files the chosen owner chip marks in the story (`OwnerMarks` of the review), by the name of the chip; null for
 * All, a branch-wide group, a group whose files are unknown, or before the files are read
 */
export function reviewOwnerMarks(filter: ReviewFilter, { files, groups, owners }: ReviewOwners): { readonly label: string; readonly paths: ReadonlySet<string> } | null {
  const label = ownerLabel(filter)
  const paths = files === undefined ? null : ownerPaths(filter, files, groups, owners)
  return label === null || paths === null ? null : { label, paths }
}

/** The name of the chosen chip on the rows it marks: "Design", "No Owner"; null for All */
export function ownerLabel(filter: ReviewFilter): string | null {
  return filter.kind === "all" ? null : filter.kind === "unowned" ? "No Owner" : filter.name
}
