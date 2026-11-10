import { useState } from "react"

/** The value built from `inputs`, or the one built last while they are the same objects */
export type SourceMemo<T> = (inputs: readonly unknown[], build: () => T) => T

/**
 * The memo of the selector of one subscriber of the stores (`useStore`): the value it built last while its inputs (the
 * states of the stores it reads and its own arguments) stay the same objects, so every read between two changes of a
 * store answers one object, as `useSyncExternalStore` needs. One per subscriber, never a cache by a key that two of them
 * share: a cache by branch held one menu for the menu of a row that still faded out and for the command palette opened
 * on the row, whose other inputs differ (their `setExpanded`); each read replaced the other's entry, every read was a
 * new menu, and React rendered both again until it gave up ("Maximum update depth exceeded", the window blank)
 */
export function useSourceMemo<T>(): SourceMemo<T> {
  const [memo] = useState(() => createSourceMemo<T>())
  return memo
}

function createSourceMemo<T>(): SourceMemo<T> {
  let built: { readonly inputs: readonly unknown[]; readonly value: T } | null = null
  return (inputs, build) => {
    if (built !== null && built.inputs.length === inputs.length && built.inputs.every((input, index) => input === inputs[index])) {
      return built.value
    }
    built = { inputs, value: build() }
    return built.value
  }
}
