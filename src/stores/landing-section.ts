import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { mainViewStore, showMainViewNow } from "@/stores/main-view"
import { onRowsForgotten } from "@/stores/view-rows"

/** The sections of the Landing view: the plan of what goes into master next, and how the Safe Pushes went */
export type LandingSection = "plan" | "statistics"

/** The edge of the plan a surface shows (the Status cell of "Lands after codec"): of the start of `branch`, to `needed` */
export interface ShownEdge {
  readonly branch: string
  readonly needed: string
}

interface LandingSectionState {
  readonly section: LandingSection
  /** The edge the plan highlights until the user moves on; null while it highlights none */
  readonly edge: ShownEdge | null
  /** Counts each show of an edge, so the plan scrolls it into view again when the same edge is shown twice */
  readonly shows: number
}

/**
 * The section of the Landing view and the edge its plan highlights (openspec/specs/repository/main-views/spec.md). A state of
 * the window's session, never persisted: every start shows the plan first. Only an action of the user changes it
 */
const landingSectionStore = createStore<LandingSectionState>(() => ({ section: "plan", edge: null, shows: 0 }))

// the rows are of another session of the repository: the edge of its branches goes
onRowsForgotten(() => landingSectionStore.setState({ edge: null }))
// the user moved on: Landing hides, the highlight goes with it
mainViewStore.subscribe(state => {
  if (state.view !== "landing") {
    clearShownEdge()
  }
})

export function useLandingSection(): LandingSection {
  return useStore(landingSectionStore, state => state.section)
}

/** The edge the plan highlights and the count of its shows */
export function useShownEdge(): { readonly edge: ShownEdge | null; readonly shows: number } {
  const edge = useStore(landingSectionStore, state => state.edge)
  const shows = useStore(landingSectionStore, state => state.shows)
  return { edge, shows }
}

/** Shows `section` of the Landing view, the main view where it is */
export function setLandingSection(section: LandingSection): void {
  if (landingSectionStore.getState().section !== section) {
    landingSectionStore.setState({ section })
  }
}

/** Shows the Landing view at `section`, committed at once: only from an event */
export function showLanding(section: LandingSection): void {
  landingSectionStore.setState({ section, edge: null })
  showMainViewNow("landing")
}

/** Shows the plan of the Landing view with `edge` highlighted (the Status cell of a start that lands after another) */
export function showLandingEdge(edge: ShownEdge): void {
  landingSectionStore.setState(state => ({ section: "plan", edge, shows: state.shows + 1 }))
  showMainViewNow("landing")
}

/** The plan highlights no edge any more: the user moved on */
export function clearShownEdge(): void {
  if (landingSectionStore.getState().edge !== null) {
    landingSectionStore.setState({ edge: null })
  }
}
