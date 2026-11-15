import { describe, expect, it } from "vite-plus/test"
import { MachineSampler, parseProcVmStat, parseVmStat, type MachineSample } from "./machine-sampler"
import { describeStall, stallContext, SWAP_INS_PER_SECOND, type TimedAction } from "./stall-context"

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
    // 30 pages in 6.4 s is the trickle of a loaded Mac, no swap that stalls it
    expect(context.verdict).toBe("the machine: 3 slow actions of other workers stood beside it")
    expect(describeStall(stall, context, seconds)).not.toContain("did not answer")
  })

  it("calls a stall the machine by its swap from SWAP_INS_PER_SECOND pages a second of it, alone too, a trickle below it not", () => {
    const stall = action(3, 10_000, 4_000)
    const swapping = stallContext(stall, [stall], [sample(10_000, 20, 1_000), sample(14_000, 20, 1_000 + SWAP_INS_PER_SECOND * 4)], 1_000, 1_000, { longTasks: [], answered: true })
    expect(swapping.verdict).toBe(
      `the machine: 0 slow actions of other workers stood beside it, the machine swapped in ${SWAP_INS_PER_SECOND * 4} pages, ${SWAP_INS_PER_SECOND} a second`,
    )
    const trickle = stallContext(stall, [stall], [sample(10_000, 20, 1_000), sample(14_000, 20, 1_000 + SWAP_INS_PER_SECOND * 4 - 1)], 1_000, 1_000, {
      longTasks: [],
      answered: true,
    })
    expect(trickle.verdict).toMatch(/^neither: /)
  })

  it("takes the swap between the samples of the counters around the stall, which come every fifth second, per second of that span", () => {
    // the load every second, the counters every fifth, from `rate` pages a second since 0
    const sampled = (rate: number) => Array.from({ length: 31 }, (_, second) => (second % 5 === 0 ? sample(second * 1000, 30, rate * second) : sample(second * 1000, 30)))
    const machine = (stall: TimedAction, rate: number) => stallContext(stall, [stall], sampled(rate), 1_000, 1_000, { longTasks: [], answered: true })
    // a stall of 2 s between two samples of the counters, at 20 times the threshold
    const short = machine(action(3, 11_000, 2_000), 1_000)
    expect(short.swapins).toBe(5_000)
    expect(short.verdict).toBe("the machine: 0 slow actions of other workers stood beside it, the machine swapped in 5000 pages, 1000 a second")
    // a span of the counters longer than the stall, at 60 a second: the rate of the span, not the count over the stall
    expect(machine(action(3, 9_000, 6_400), 60).verdict).toMatch(/^the machine: .* 60 a second$/)
    expect(machine(action(3, 9_000, 6_400), 40).verdict).toMatch(/^neither: /)
    // a sample of the counters after the stall comes from the stop of the sampler at the latest (below); without one, no
    // swap is told
    expect(machine(action(3, 26_000, 6_000), 1_000).swapins).toBeNull()
  })

  it("takes a swap of a stall at the end of the run from the sample of the counters that the sampler takes as it stops", () => {
    let swapins = 0
    const sampler = new MachineSampler({ now: () => ({ swapins }), later: done => done({ swapins }) })
    const now = Date.now()
    // the last sample of the counters of the run, 4 s before its end
    sampler.samples.push({ at: now - 4_000, load: 30, freeBytes: 0, swapins: 0 })
    // a stall in the last seconds, at 1250 pages a second, which no sample of the counters followed until the stop
    const stall = action(3, now - 3_000, 2_500)
    swapins = 5_000
    sampler.stop()
    const context = stallContext(stall, [stall], sampler.samples, 1_000, 1_000, { longTasks: [], answered: true })
    expect(context.swapins).toBe(5_000)
    expect(context.verdict).toMatch(/^the machine: .* swapped in 5000 pages/)
  })

  it("says the page was not asked when it calls the machine without an answer of the page or a long task of it", () => {
    // a passing test asks its pages nothing, and WebKit tells no long task
    const stall = action(3, 10_000, 5_300, 'Press "Enter"')
    const beside = [action(1, 11_000, 2_500), action(2, 12_000, 3_000)]
    const unasked = stallContext(stall, [stall, ...beside], [], 1_000, 1_000, undefined)
    expect(unasked.verdict).toBe("the machine (the page was not asked): 2 slow actions of other workers stood beside it")
    const asked = stallContext(stall, [stall, ...beside], [], 1_000, 1_000, { longTasks: [], answered: true })
    expect(asked.verdict).toBe("the machine: 2 slow actions of other workers stood beside it")
    // a long task of the page in the stall, too short to be it, says the page ran: no suffix
    const ran = stallContext(stall, [stall, ...beside], [], 1_000, 1_000, { longTasks: [[12_000, 300]], answered: null })
    expect(ran.verdict).toBe("the machine: 2 slow actions of other workers stood beside it")
    // neither, without a word of the page, says it too
    const alone = stallContext(stall, [stall], [], 1_000, 1_000, undefined)
    expect(alone.verdict).toMatch(/^neither \(the page was not asked\): /)
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
