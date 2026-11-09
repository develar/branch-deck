import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"

interface DiskUsageDialogState {
  /** The repository whose config gives the minimum and whose pool is reported first; null while it is closed */
  readonly repositoryPath: string | null
  /** Counts the openings: each reads the disk anew */
  readonly generation: number
}

/** The Disk Usage dialog of the main window (openspec/specs/repository/disk-usage/spec.md): "Show Disk Usage…" of the palette */
export const diskUsageDialogStore = createStore<DiskUsageDialogState>(() => ({ repositoryPath: null, generation: 0 }))

export function useDiskUsageDialog(): DiskUsageDialogState {
  return useStore(diskUsageDialogStore, state => state)
}

export function useDiskUsageDialogOpen(): boolean {
  return useStore(diskUsageDialogStore, state => state.repositoryPath !== null)
}

export function openDiskUsageDialog(repositoryPath: string): void {
  diskUsageDialogStore.setState(state => ({ repositoryPath, generation: state.generation + 1 }))
}

export function closeDiskUsageDialog(): void {
  diskUsageDialogStore.setState({ repositoryPath: null })
}
