import { describe, expect, it } from "vite-plus/test"
import { parseProcVmStat, parseVmStat, type MachineSample } from "./machine-sampler"
import { describeStall, stallContext, type TimedAction } from "./stall-context"

const action = (worker: number, start: number, ms: number, name = "Click"): TimedAction => ({ test: `test of ${worker}`, action: name, ms, start, worker })
const sample = (at: number, load: number, swapins?: number): MachineSample => ({ at, load, freeBytes: 0, swapins })
const seconds = (ms: number) => `${(ms / 1000).toFixed(1)} s`

describe("stallContext", () => {
  it("tells the load and the swap-ins of the samples of the stall, with the one before and after it", () => {
    const stall = action(3, 10_000, 5_000, 'Press "h"')
    const samples = [sample(7_000, 10, 100), sample(9_500, 40, 110), sample(12_000, 85, 500), sample(15_500, 60, 900), sample(18_000, 5, 2_000)]
    const context = stallContext(stall, [stall], samples, 1_000, 1_000, undefined)
    expect(context).toMatchObject({ from: 10_000, to: 15_000, worker: 3, load: [40, 85], swapins: 790, beside: [] })
  })

  it("names the slow actions of other workers that stood in the same time, the slowest first, never the stall's own worker or a quick one", () => {
    const stall = action(3, 10_000, 5_000)
    const beside = [action(1, 12_000, 1_500), action(2, 8_000, 4_000), action(4, 9_000, 3_000), action(3, 11_000, 2_000), action(5, 12_000, 200), action(6, 15_000, 3_000)]
    const context = stallContext(stall, [stall, ...beside], [], 1_000, 1_000, undefined)
    expect(context.beside.map(it => it.worker)).toEqual([2, 4, 1])
    expect(context.load).toBeNull()
    expect(context.swapins).toBeNull()
  })

  it("calls a press whose page did not answer at the end of its failed test the page, wherever the machine stood", () => {
    // the press of 22.8 s the test timed out in: the page stuck, on a loaded machine
    const stall = action(3, 10_000, 22_800, 'Press "h"')
    const beside = [action(1, 12_000, 3_900), action(2, 20_000, 4_300)]
    const context = stallContext(stall, [stall, ...beside], [sample(10_000, 85), sample(30_000, 92)], 1_000, 1_000, { longTasks: [], answered: false })
    expect(context.verdict).toMatch(/^the page: it did not answer/)
    const lines = describeStall(stall, context, seconds).split("\n")
    expect(lines[1]).toBe("      → the page: it did not answer before the end of its test, its main thread stuck in a task")
    expect(lines[2]).toContain("a page did not answer")
  })

  it("calls a stall beside slow actions of other workers the machine, never a page that did not answer, when its pages answered and had no long task", () => {
    const stall = action(3, 10_000, 6_400, 'Press "j"')
    const beside = [action(1, 11_000, 2_500), action(2, 12_000, 3_000), action(4, 9_000, 2_000)]
    const context = stallContext(stall, [stall, ...beside], [sample(10_000, 70, 10), sample(16_000, 85, 40)], 1_000, 1_000, { longTasks: [], answered: true })
    expect(context.verdict).toBe("the machine: 3 slow actions of other workers stood beside it, the machine swapped in 30 pages")
    expect(describeStall(stall, context, seconds)).not.toContain("did not answer")
  })

  it("calls a stall over half of which a long task of its own pages stood the page, a shorter one or another time not", () => {
    const stall = action(3, 10_000, 4_000)
    const long = stallContext(stall, [stall], [], 1_000, 1_000, { longTasks: [[13_000, 2_500]], answered: null })
    expect(long.verdict).toBe("the page: a long task of 2.5 s stood in it")
    const elsewhere = stallContext(stall, [stall], [], 1_000, 1_000, {
      longTasks: [
        [30_000, 5_000],
        [12_000, 600],
      ],
      answered: null,
    })
    expect(elsewhere.longTasks).toEqual([[12_000, 600]])
    expect(elsewhere.verdict).toMatch(/^neither: /)
  })

  it("words a stall with its worker, its clock, the load, the swap, the long tasks and what stood beside it", () => {
    const stall = action(3, Date.UTC(2026, 10, 15, 15, 18, 5), 22_800, 'Press "h"')
    const context = stallContext(stall, [stall, action(1, stall.start, 3_900)], [], 1_000, 1_000, { longTasks: [], answered: null })
    const line = describeStall(stall, context, seconds)
    expect(line).toContain('22.8 s Press "h" in test of 3')
    expect(line).toMatch(/worker 3, \d\d:\d\d:\d\d–\d\d:\d\d:\d\d, no long task of its pages, beside it 3\.9 s Click \(worker 1\)/)
  })
})

describe("the counters of the swap", () => {
  it("reads the swap-ins, swap-outs and decompressions of vm_stat (macOS) and the swap of /proc/vmstat (Linux)", () => {
    const vmStat =
      "Mach Virtual Memory Statistics: (page size of 16384 bytes)\nPages free:      38261.\nDecompressions:     117436529.\nPageouts:      21798.\nSwapins:      22080.\nSwapouts:      48608.\n"
    expect(parseVmStat(vmStat)).toEqual({ swapins: 22080, swapouts: 48608, decompressions: 117436529 })
    expect(parseProcVmStat("pgpgin 10\npswpin 7\npswpout 9\n")).toEqual({ swapins: 7, swapouts: 9 })
  })
})
