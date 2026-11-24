import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { UnattendedDenial } from "@/lib/bindings"

interface DeniedRequestsState {
  /** The requests the list shows, oldest first; null while it is closed */
  readonly denials: readonly UnattendedDenial[] | null
}

/**
 * The list of Denied Requests of the main window (openspec/specs/repository/unattended-mode/spec.md): what the hook of
 * Unattended Mode denied, opened by the toast of a switch off with the denials its answer named
 */
const deniedRequestsStore = createStore<DeniedRequestsState>(() => ({ denials: null }))

export function useDeniedRequests(): readonly UnattendedDenial[] | null {
  return useStore(deniedRequestsStore, state => state.denials)
}

export function openDeniedRequests(denials: readonly UnattendedDenial[]): void {
  deniedRequestsStore.setState({ denials })
}

export function closeDeniedRequests(): void {
  deniedRequestsStore.setState({ denials: null })
}
