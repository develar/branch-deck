import { describe, expect, it } from "vite-plus/test"
import type { MachineDisk, PoolDisk, SlotDisk } from "@/lib/bindings"
import {
  formatSize,
  freeSpaceLine,
  isBelowMinimum,
  machineTitle,
  poolLine,
  slotSizeDetail,
  slotsBySize,
  slotStateLabel,
  trimDisabledReason,
  trimToast,
  trimWhat,
} from "@/presenters/disk"

function slot(name: string, mib: number, rest: Partial<SlotDisk> = {}): SlotDisk {
  return { name, path: `/pool/${name}/repo`, state: "free", holder: null, mib, buildMib: mib, basesMib: 0, trimsNext: false, ...rest }
}

function pool(slots: SlotDisk[], rest: Partial<PoolDisk> = {}): PoolDisk {
  return { name: "idea-1", path: "/projects/idea-1", maxSlots: 10, pastLimit: 0, mib: 0, checkoutBasesMib: 0, slots, ...rest }
}

function machine(rest: Partial<MachineDisk> = {}): MachineDisk {
  return { alias: null, error: null, freeMib: 300 * 1024, minFreeMib: 200 * 1024, mib: 0, pools: [], orphans: 0, orphanMib: 0, pastLimit: 0, ...rest }
}

describe("disk", () => {
  it("words a size in binary units, MiB below a GiB", () => {
    expect([formatSize(0), formatSize(512), formatSize(1536), formatSize(200 * 1024)]).toEqual(["0 B", "512 MiB", "1.5 GiB", "200.0 GiB"])
  })

  it("names this Mac and a host, the free space against the minimum", () => {
    expect([machineTitle(machine()), machineTitle(machine({ alias: "studio" }))]).toEqual(["This Mac", "studio"])
    expect(freeSpaceLine(machine())).toBe("300.0 GiB free, a lease trims the pools below 200.0 GiB")
    expect(freeSpaceLine(machine({ minFreeMib: null }))).toBe("300.0 GiB free, no minimum set (branchdeck.leaseMinFree)")
    expect(freeSpaceLine(machine({ freeMib: null }))).toBe("The free space is unknown")
    expect([isBelowMinimum(machine()), isBelowMinimum(machine({ freeMib: 100 * 1024 })), isBelowMinimum(machine({ minFreeMib: null }))]).toEqual([false, true, false])
  })

  it("words a pool, its slots largest first, and what a trim deletes", () => {
    const slots = [slot("1", 100), slot("2", 4096, { basesMib: 2048, buildMib: 2048 }), slot("3", 0, { state: "leased", holder: "lane-3" })]
    expect(poolLine(pool(slots))).toBe("3 slots, limit 10")
    expect(poolLine(pool(slots, { maxSlots: 1, pastLimit: 2 }))).toBe("3 slots, limit 1, 2 go with a trim")
    expect(slotsBySize(pool(slots)).map(it => it.name)).toEqual(["2", "1", "3"])
    expect(slots.map(slotStateLabel)).toEqual(["Free", "Free", "Leased by lane-3"])
    expect(slotStateLabel(slot("4", 0, { state: "changing" }))).toBe("Busy")
    expect(slots.map(slotSizeDetail)).toEqual(["build 100 MiB", "build 2.0 GiB, Bazel 2.0 GiB", ""])
  })

  it("says why Trim to Limit does nothing, else what it deletes", () => {
    expect(trimDisabledReason(machine())).toBe("No free slot is past the limit")
    expect(trimDisabledReason(machine({ orphans: 1 }))).toBeNull()
    expect(trimWhat(machine({ pastLimit: 2, orphans: 1 }))).toBe("2 free slots and 1 output base of gone slots")
    expect(trimWhat(machine({ pastLimit: 1 }))).toBe("1 free slot")
  })

  it("words the toast of a trim", () => {
    expect(trimToast({ removedSlots: 2, orphans: 0, freedMib: 70 * 1024, busy: 1, errors: [] })).toEqual({
      title: "Trimmed 2 slots",
      description: "Freed 70.0 GiB; 1 slot stayed, a process works in it",
    })
    expect(trimToast({ removedSlots: 0, orphans: 0, freedMib: 0, busy: 0, errors: [] }).title).toBe("Nothing to trim")
  })
})
