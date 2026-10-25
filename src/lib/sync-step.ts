import type { SyncStep } from "@/lib/bindings"

/** What a running sync does, as the tooltip of Sync and the loading table say it; without a step yet, only that it runs */
export function describeSyncStep(step: SyncStep | null): string {
  switch (step) {
    case "grouping":
      return "Grouping the commits…"
    case "building":
      return "Building the branches…"
    case "detection":
      return "Checking which branches landed…"
    case null:
      return "Syncing branches…"
  }
}
