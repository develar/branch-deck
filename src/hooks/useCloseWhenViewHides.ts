import { createContext, use, useEffectEvent, useLayoutEffect, useRef } from "react"
import type { RefObject } from "react"
import type { MainView } from "@/stores/main-view"
import { mainViewStore } from "@/stores/main-view"

/** The view of the main window a part of the page is kept mounted in (`KeepMounted` of `IndexPage`); null outside them */
export const KeptViewContext = createContext<MainView | null>(null)

/** What a Base UI root lets its owner do (`actionsRef`): every root has these two */
export interface PopupActions {
  close: () => void
  unmount: () => void
}

export interface ClosingWithView<Details> {
  /** The `actionsRef` of the root */
  actionsRef: RefObject<PopupActions | null>
  /** The `onOpenChange` of the root: the owner's, and what the root is now */
  onOpenChange: (open: boolean, details: Details) => void
}

/**
 * Closes a popup of a view of the main window when the view hides (spec/repository/main-views.spec.md): a hidden view
 * keeps its state but runs no effect, so the hover, the dismiss and the focus of an open popup stop listening; it would
 * come back open with the pointer elsewhere, a tooltip or a hover card whose trigger never got its pointer leave, until
 * a click elsewhere. The hide runs the cleanups of the effects of a parent before those of its children, so the root
 * still has its actions then. A closed popup does nothing as its view hides: a table of 141 rows has hundreds of roots.
 *
 * `open`: the open of a controlled root, else what its `onOpenChange` told
 */
export function useCloseWhenViewHides<Details extends { readonly isCanceled: boolean }>(
  open: boolean | undefined,
  onOpenChange: ((open: boolean, details: Details) => void) | undefined,
): ClosingWithView<Details> {
  const view = use(KeptViewContext)
  const actionsRef = useRef<PopupActions | null>(null)
  const told = useRef(false)
  // the actions the root has as its view hides: they go with its effects after this cleanup
  const close = useEffectEvent(() => {
    if (open ?? told.current) {
      actionsRef.current?.close()
    }
  })
  useLayoutEffect(() => {
    if (view === null) {
      return undefined
    }
    return () => {
      // an unmount while the view shows closes nothing: the root goes with it
      if (mainViewStore.getState().view !== view) {
        close()
      }
    }
  }, [view])
  return {
    actionsRef,
    onOpenChange: (next, details) => {
      onOpenChange?.(next, details)
      if (!details.isCanceled) {
        told.current = next
      }
    },
  }
}
