import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { PatronusRobot } from "@/lib/bindings"

interface AirLaunchDialogState {
  /** The robot whose prompt the dialog starts in Air, of `repositoryPath`; null while it is closed */
  readonly robot: PatronusRobot | null
  readonly repositoryPath: string
  /** Counts the openings: each reads the targets and the prompt anew */
  readonly generation: number
}

/** The "Start in Air" dialog of the main window: "Start in Air…" of the actions of a failed robot opens it */
export const airLaunchDialogStore = createStore<AirLaunchDialogState>(() => ({ robot: null, repositoryPath: "", generation: 0 }))

export function useAirLaunchDialog(): AirLaunchDialogState {
  return useStore(airLaunchDialogStore, state => state)
}

export function useAirLaunchDialogOpen(): boolean {
  return useStore(airLaunchDialogStore, state => state.robot !== null)
}

export function openAirLaunchDialog(repositoryPath: string, robot: PatronusRobot): void {
  airLaunchDialogStore.setState(state => ({ robot, repositoryPath, generation: state.generation + 1 }))
}

export function closeAirLaunchDialog(): void {
  airLaunchDialogStore.setState({ robot: null })
}
