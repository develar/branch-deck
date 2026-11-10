import { createContext, useContext, useLayoutEffect, useState } from "react"
import type { RefObject } from "react"

/** The card of the heading of a commit reviewed alone, by the id of that commit: open, and opened by `I` */
interface HeadingCardState {
  readonly commit: string | null
  readonly open: boolean
  readonly byKey: boolean
}

/**
 * The card of the heading of a commit reviewed alone, kept by the view that mounts its overview in one place after
 * another (above the pending headers while the diffs load, then as the prologue of the diffs): a move remounts the
 * heading, which keeps its card and the focus it held
 */
interface HeadingCard {
  readonly card: HeadingCardState
  readonly setCard: (card: HeadingCardState) => void
  /** The commit whose heading has its "⋯" menu open, which a move keeps open too; null for none */
  readonly menu: string | null
  readonly setMenu: (commit: string | null) => void
  /** The heading leaves the document, holding the focus or not */
  readonly leave: (focused: boolean) => void
  /** Whether the heading that left last held the focus, which a heading that mounts takes back once */
  readonly takeFocus: () => boolean
}

const CLOSED: HeadingCardState = { commit: null, open: false, byKey: false }

export const HeadingCardContext = createContext<HeadingCard | null>(null)

/** The value of `HeadingCardContext`, for the view that mounts the overview */
export function useHeadingCardState(): HeadingCard {
  const [card, setCard] = useState<HeadingCardState>(CLOSED)
  const [menu, setMenu] = useState<string | null>(null)
  // stable across renders: a heading's effect depends on them, not on its card
  const [focus] = useState(() => {
    let focused = false
    return {
      leave: (held: boolean) => {
        focused = held
      },
      takeFocus: () => {
        const held = focused
        focused = false
        return held
      },
    }
  })
  return { card, setCard, menu, setMenu, leave: focus.leave, takeFocus: focus.takeFocus }
}

/**
 * The "⋯" menu of the heading of `commit`: open as the view keeps it across a move of the overview
 * (`HeadingCardContext`), so a menu opened while the diffs load stays open as the heading mounts in its new place;
 * uncontrolled outside such a view
 */
export function useHeadingMenu(commit: string): { open?: boolean; onOpenChange?: (open: boolean) => void } {
  const shared = useContext(HeadingCardContext)
  if (shared === null) {
    return {}
  }
  return {
    open: shared.menu === commit,
    onOpenChange: open => shared.setMenu(open ? commit : shared.menu === commit ? null : shared.menu),
  }
}

/**
 * The card of the heading of `commit` (`heading` its element) and how to open or close it: the one the view keeps
 * across a move of the overview (`HeadingCardContext`), else its own; a heading that mounts in the place of one that
 * held the focus takes it
 */
export function useHeadingCard(commit: string, heading: RefObject<HTMLElement | null>): readonly [{ open: boolean; byKey: boolean }, (open: boolean, byKey: boolean) => void] {
  const shared = useContext(HeadingCardContext)
  const [own, setOwn] = useState<HeadingCardState>(CLOSED)
  const state = shared?.card ?? own
  const set = shared?.setCard ?? setOwn
  const leave = shared?.leave
  const takeFocus = shared?.takeFocus
  useLayoutEffect(() => {
    if (takeFocus?.() === true) {
      heading.current?.focus()
    }
    const element = heading.current
    // a cleanup runs before the element leaves the document: the focus is still where it was
    return () => leave?.(element?.contains(document.activeElement) === true)
  }, [leave, takeFocus, heading])
  return [state.commit === commit ? state : CLOSED, (open, byKey) => set({ commit, open, byKey })]
}
