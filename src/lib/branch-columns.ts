/**
 * The columns of the branch view (docs/design/ui-theme.md#branch-columns): the table and every stack island are tables
 * of their own, so their columns line up only when each column is as wide in every card. A track is one kind of item
 * in a column: the commit count, the parts of Status (the state of the branch, its pull request chip, the Landed chip)
 * and of Actions (the push, the next step, "⋯"). Its width is the widest natural width of its items in any card, which
 * a `ResizeObserver` measures on the content of each item and `ColumnRegistry` writes as a CSS variable on the root of
 * the view; every item of the track is at least that wide. A track that no row fills is 0 wide, its gutter too. The
 * label of a column heading a group of tracks is a minimum of the group.
 */
export const BRANCH_TRACKS = ["commits", "state", "pr", "landed", "push", "step", "more"] as const
export type BranchTrack = (typeof BRANCH_TRACKS)[number]

export const BRANCH_TRACK_GROUPS = ["status", "actions"] as const
export type BranchTrackGroup = (typeof BRANCH_TRACK_GROUPS)[number]

/** The tracks whose gutter is set only while one of their items has content: before the item, or after it */
const GUTTERS: Partial<Record<BranchTrack, true>> = { pr: true, landed: true, push: true, step: true }
const GUTTER = "0.5rem"

/** A track's item: its width and its gutter, literal for Tailwind */
export const TRACK_ITEM_CLASS: Readonly<Record<BranchTrack, string>> = {
  // the counts right-aligned under their head, the digits in a column
  commits: "min-w-(--bd-track-commits) justify-end",
  state: "min-w-(--bd-track-state)",
  pr: "ms-(--bd-gap-pr) min-w-(--bd-track-pr)",
  landed: "ms-(--bd-gap-landed) min-w-(--bd-track-landed)",
  push: "me-(--bd-gap-push) min-w-(--bd-track-push)",
  step: "me-(--bd-gap-step) min-w-(--bd-track-step)",
  more: "min-w-(--bd-track-more)",
}

/**
 * The caps of the tracks whose text truncates: the status text and the robot, what lands the branch, the step; a step
 * with actions beside its slot (a start whose answer was lost, a wait for the lock) is as wide as they are, as buttons
 * don't truncate
 */
export const TRACK_CONTENT_CLASS: Readonly<Record<BranchTrack, string>> = {
  commits: "",
  state: "max-w-40",
  pr: "",
  landed: "max-w-48",
  push: "",
  step: "max-w-40 has-[[data-beside-slot]]:max-w-none",
  more: "",
}

/** The container of a group's items in a row or a header: at least as wide as the label of its column */
export const TRACK_GROUP_CLASS: Readonly<Record<BranchTrackGroup, string>> = {
  status: "min-w-(--bd-group-status)",
  actions: "min-w-(--bd-group-actions)",
}

/**
 * The width of the branch view, in rem, from which a row has the room for its words ("with children" of a push): the
 * registry sets `data-wide` on the root of the view from it, which an item reads as `group-data-wide/branch-view:`. Not
 * a container query: WebKit restyles every row of a query container at each change of its width
 * (docs/design/ui-theme.md#branch-columns, "Room, not a container query")
 */
const WIDE_REM = 56

/** A track, or the label of a group's column */
export type ColumnKey = BranchTrack | `group-${BranchTrackGroup}`

type Key = ColumnKey

/**
 * The widths of the tracks of one branch view, written as CSS variables on its root; while the facts of a start are not
 * all in, at least the widths the view had once they were in the last time (the floors of `setLayout`), so a late
 * answer that fills a track the way it did before moves no column; while a sync runs a track only widens (`held`), as a
 * write restyles every row of the view. The root itself is measured too: `data-wide` while it is at least `WIDE_REM`
 * wide, changed only when its width crosses that, so a resize inside either side restyles nothing
 */
export class ColumnRegistry {
  private readonly items = new Map<Element, { key: Key; width: number }>()
  private readonly written = new Map<string, string>()
  /** The width of each track as the root has it now, what a held track is at least */
  private shown: ReadonlyMap<Key, number> = new Map()
  private root: HTMLElement | null = null
  private scheduled = false
  private floors: Readonly<Partial<Record<Key, number>>> | null = null
  private held = false
  private wide: boolean | null = null
  private readonly observer: ResizeObserver | null =
    typeof ResizeObserver === "undefined"
      ? null
      : new ResizeObserver(entries => {
          let resized = false
          for (const entry of entries) {
            if (entry.target === this.root) {
              this.setWide(entry.contentBoxSize[0]?.inlineSize ?? entry.contentRect.width)
              continue
            }
            const item = this.items.get(entry.target)
            if (item !== undefined) {
              // the width as laid out, a fraction too: a track as wide as its content moves nothing when the report of
              // a wider content comes a frame after the content (rounded up it would, by the fraction)
              item.width = entry.borderBoxSize[0]?.inlineSize ?? entry.contentRect.width
              resized = true
            }
          }
          if (resized) {
            this.write()
          }
        })

  setRoot(root: HTMLElement | null): void {
    if (this.root !== null) {
      this.observer?.unobserve(this.root)
    }
    this.root = root
    this.written.clear()
    this.shown = new Map()
    this.wide = null
    // its first report comes before the first paint, as the items' do: the first frame has the words where they fit
    // (the content box, as a container query compares)
    if (root !== null) {
      this.observer?.observe(root)
    }
    this.write()
  }

  /**
   * How the tracks follow their content from now on. `floors`: the least width of each track, null for none, the
   * settled layout of the last start (`useSettledLayout`). `held`: a track only widens, never narrower than the root
   * has it, while a sync runs, as the texts of a run ("syncing 3/7…") change the widths of the items, and each width
   * written restyles every row of the view. As the floors go or the hold ends, the view follows its content once, in
   * the commit that renders what ends them: the widths are read from the items as laid out then, not from the observer,
   * whose report of that content comes a frame later, so the drop moves no item for a frame
   */
  setLayout(floors: Readonly<Partial<Record<Key, number>>> | null, held: boolean): void {
    if (this.floors === floors && this.held === held) {
      return
    }
    if ((this.floors !== null && floors === null) || (this.held && !held)) {
      for (const [element, item] of this.items) {
        item.width = element.getBoundingClientRect().width
      }
    }
    this.floors = floors
    this.held = held
    this.write()
  }

  /**
   * The widest item of each track as laid out now, without the floors: what the view settles at. Read from the items
   * themselves, not from the observer, whose report of the frame that changed them comes after a frame callback
   */
  measured(): Partial<Record<Key, number>> {
    const widths: Partial<Record<Key, number>> = {}
    for (const [element, { key }] of this.items) {
      widths[key] = Math.max(widths[key] ?? 0, element.getBoundingClientRect().width)
    }
    return widths
  }

  /** Measures the content of an item of `track` (or the label of `group-<group>`) until the returned cleanup */
  observe(element: Element, key: Key): () => void {
    this.items.set(element, { key, width: 0 })
    this.observer?.observe(element, { box: "border-box" })
    return () => {
      this.observer?.unobserve(element)
      this.items.delete(element)
      // the widest item may be gone: once for the rows a collapse removes together
      if (!this.scheduled) {
        this.scheduled = true
        queueMicrotask(() => {
          this.scheduled = false
          this.write()
        })
      }
    }
  }

  /**
   * `data-wide` on the root while its content box is at least `WIDE_REM` wide. The observer and the font size of the
   * root element both answer in CSS px, which the page zoom of the webview leaves as they are
   */
  private setWide(width: number): void {
    const root = this.root
    if (root === null) {
      return
    }
    const wide = width >= WIDE_REM * Number.parseFloat(getComputedStyle(document.documentElement).fontSize)
    if (this.wide !== wide) {
      this.wide = wide
      root.toggleAttribute("data-wide", wide)
    }
  }

  private write(): void {
    const root = this.root
    if (root === null) {
      return
    }
    const widths = new Map<Key, number>(Object.entries(this.floors ?? {}) as [Key, number][])
    for (const { key, width } of this.items.values()) {
      widths.set(key, Math.max(widths.get(key) ?? 0, width))
    }
    if (this.held) {
      for (const [key, width] of this.shown) {
        widths.set(key, Math.max(widths.get(key) ?? 0, width))
      }
    }
    // nothing measured yet (the items of the first commit, before the observer's first report, which comes before the
    // first paint): a track without its variable is as wide as 0px, and the zeros would restyle every row for none
    if (this.written.size === 0 && ![...widths.values()].some(width => width > 0)) {
      return
    }
    this.shown = widths
    for (const track of BRANCH_TRACKS) {
      const width = widths.get(track) ?? 0
      this.set(root, `--bd-track-${track}`, `${width}px`)
      if (GUTTERS[track] === true) {
        this.set(root, `--bd-gap-${track}`, width > 0 ? GUTTER : "0px")
      }
    }
    for (const group of BRANCH_TRACK_GROUPS) {
      this.set(root, `--bd-group-${group}`, `${widths.get(`group-${group}`) ?? 0}px`)
    }
  }

  private set(root: HTMLElement, name: string, value: string): void {
    if (this.written.get(name) !== value) {
      this.written.set(name, value)
      root.style.setProperty(name, value)
    }
  }
}
