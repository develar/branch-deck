import { createContext, use, useId } from "react"
import type { Ref, RefCallback } from "react"
import { registerControl } from "@/lib/pointer-moves"

/** The key of the control a root of `src/components/ui` is, for its trigger (`useControlRef`) */
export const ControlKeyContext = createContext<string | null>(null)

/**
 * The key of the control of a root (spec/branch/branch-row.spec.md, Hover and click): the one its owner names
 * (`controlKey`, the action and its subject, for a control that renders its root anew), else its own, so a trigger
 * that renders as another element in the same root stays the same control for the hover (`registerControl`)
 */
export function useControlKey(controlKey: string | undefined): string {
  const own = useId()
  return controlKey ?? own
}

/** The ref of a trigger: `ref`, and its element named as the control `key`, else as the control of its root */
export function useControlRef<T extends Element>(ref: Ref<T> | undefined, key?: string): RefCallback<T> {
  const inherited = use(ControlKeyContext)
  const control = key ?? inherited
  return (element: T | null) => {
    const unset = assignRef(ref, element)
    const unregister = element === null || control === null ? undefined : registerControl(element, control)
    return () => {
      unregister?.()
      unset()
    }
  }
}

/** Sets `ref` to `element`; returns what clears it again, as React does for a callback that returns no cleanup */
function assignRef<T>(ref: Ref<T> | undefined, element: T | null): () => void {
  if (typeof ref === "function") {
    const cleanup = ref(element)
    return typeof cleanup === "function" ? cleanup : () => ref(null)
  }
  if (ref !== null && ref !== undefined) {
    ref.current = element
    return () => {
      ref.current = null
    }
  }
  return () => {}
}
