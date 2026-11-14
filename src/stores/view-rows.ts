import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"

/**
 * The rows of the main window as one session of its repository holds them: `generation` counts the views of another
 * session of the repository the window took in place of the one it held (a slot of a lease pool leased again, another
 * prefix, an actor that died: `installAnother` of `src/model/store.ts`). The branches view mounts anew under each
 * (the expanded rows, the focus, the selection of each commit list), and each store that holds something of a row
 * forgets it (`onRowsForgotten`), as a row of the same name in the view of another session is another branch
 */
const viewRowsStore = createStore<{ readonly generation: number }>(() => ({ generation: 0 }))

/** The generation of the rows the window shows */
export function useRowsGeneration(): number {
  return useStore(viewRowsStore, state => state.generation)
}

/** The rows the window held are of a session it left: the branches view and every store of the rows forget them */
export function forgetRows(): void {
  viewRowsStore.setState(state => ({ generation: state.generation + 1 }))
}

/** Runs `forget` each time the window forgets the rows it held */
export function onRowsForgotten(forget: () => void): void {
  viewRowsStore.subscribe((state, previous) => {
    if (state.generation !== previous.generation) {
      forget()
    }
  })
}
