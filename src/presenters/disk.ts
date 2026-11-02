import type { MachineDisk, PoolDisk, SlotDisk, TrimSummary } from "@/lib/bindings"

/*
 * The words of the Disk Usage dialog (spec/repository/disk-usage.spec.md): what Rust measured and decided (`disk_usage`,
 * which slots a trim deletes next), worded. Sizes come in MiB and are worded binary, as `bd lease disk` and the minimum
 * of the git config (`200G`) count them.
 */

/** "12.3 GiB", "512 MiB" below a gibibyte, "0 B" for nothing */
export function formatSize(mib: number): string {
  if (mib <= 0) {
    return "0 B"
  }
  if (mib < 1024) {
    return `${mib} MiB`
  }
  return `${(mib / 1024).toFixed(1)} GiB`
}

/** "This Mac", else the alias of the host */
export function machineTitle(machine: MachineDisk): string {
  return machine.alias ?? "This Mac"
}

/** "214.0 GiB free, a get trims below 200.0 GiB" or "… free, no minimum set" */
export function freeSpaceLine(machine: MachineDisk): string {
  if (machine.freeMib === null) {
    return "The free space is unknown"
  }
  const free = `${formatSize(machine.freeMib)} free`
  if (machine.minFreeMib === null) {
    return `${free}, no minimum set (branchdeck.leaseMinFree)`
  }
  return `${free}, a lease trims the pools below ${formatSize(machine.minFreeMib)}`
}

/** The free space is below the minimum: the next lease trims */
export function isBelowMinimum(machine: MachineDisk): boolean {
  return machine.freeMib !== null && machine.minFreeMib !== null && machine.freeMib < machine.minFreeMib
}

/** "12 slots, limit 10, 2 go with a trim" */
export function poolLine(pool: PoolDisk): string {
  const slots = `${pool.slots.length} ${pool.slots.length === 1 ? "slot" : "slots"}, limit ${pool.maxSlots}`
  return pool.pastLimit === 0 ? slots : `${slots}, ${pool.pastLimit} ${pool.pastLimit === 1 ? "goes" : "go"} with a trim`
}

/** "Free", "Leased by lane-3", "Busy" */
export function slotStateLabel(slot: SlotDisk): string {
  switch (slot.state) {
    case "free":
      return "Free"
    case "leased":
      return slot.holder === null ? "Leased" : `Leased by ${slot.holder}`
    case "changing":
      return "Busy"
  }
}

/** What the size of a slot is: "worktree 15.1 GiB, build 41.0 GiB, Bazel 23.5 GiB" */
export function slotSizeDetail(slot: SlotDisk): string {
  const parts = [
    slot.worktreeMib > 0 ? `worktree ${formatSize(slot.worktreeMib)}` : null,
    slot.buildMib > 0 ? `build ${formatSize(slot.buildMib)}` : null,
    slot.basesMib > 0 ? `Bazel ${formatSize(slot.basesMib)}` : null,
  ]
  return parts.filter(part => part !== null).join(", ")
}

/** The slots of a pool, the largest first */
export function slotsBySize(pool: PoolDisk): SlotDisk[] {
  return [...pool.slots].sort((a, b) => b.mib - a.mib)
}

/** Why Trim to Limit of this Mac does nothing now; null when it deletes something */
export function trimDisabledReason(machine: MachineDisk): string | null {
  return machine.pastLimit === 0 && machine.orphans === 0 ? "No free slot is past the limit" : null
}

/** "2 free slots and 3 output bases go" */
export function trimWhat(machine: MachineDisk): string {
  const slots = machine.pastLimit === 0 ? null : `${machine.pastLimit} free ${machine.pastLimit === 1 ? "slot" : "slots"}`
  const orphans = machine.orphans === 0 ? null : `${machine.orphans} output ${machine.orphans === 1 ? "base" : "bases"} of gone slots`
  return [slots, orphans].filter(part => part !== null).join(" and ")
}

/** The toast of a trim: "Trimmed 2 slots", "Freed 70.2 GiB; 1 slot stayed, a process works in it" */
export function trimToast(summary: TrimSummary): { title: string; description: string } {
  const title = summary.removedSlots === 0 && summary.orphans === 0 ? "Nothing to trim" : `Trimmed ${summary.removedSlots} ${summary.removedSlots === 1 ? "slot" : "slots"}`
  const busy = summary.busy === 0 ? "" : `; ${summary.busy} ${summary.busy === 1 ? "slot stayed, a process works in it" : "slots stayed, processes work in them"}`
  return { title, description: `Freed ${formatSize(summary.freedMib)}${busy}` }
}
