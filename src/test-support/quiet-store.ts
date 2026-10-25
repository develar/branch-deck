/** A store of a module mock that never changes, with `state`: what reads it subscribes, and is told nothing */
export function quietStoreOf<T extends object>(state: T): { subscribe: () => () => void; getState: () => T } {
  return { subscribe: () => () => {}, getState: () => state }
}

/** `quietStoreOf` of a store nothing reads the state of */
export const quietStore = quietStoreOf({})
