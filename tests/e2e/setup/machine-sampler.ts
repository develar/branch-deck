import { execFile, execFileSync } from "child_process"
import { readFileSync } from "fs"
import { freemem, loadavg, platform } from "os"

/**
 * The machine the tests run on at a moment, as the timing reporter samples it: the load average of a minute and the
 * free memory every second, no process, and every fifth second the counters of its swap and of the memory compressor
 * since its boot (macOS: `vm_stat`, a process; Linux: `/proc/vmstat`; none on Windows), so a stalled action says whether
 * the machine stalled with it
 */
export interface MachineSample {
  at: number
  load: number
  freeBytes: number
  swapins?: number
  swapouts?: number
  decompressions?: number
}

/** How often the reporter samples the load and the memory */
export const SAMPLE_MS = 1000

/** Every how many samples it reads the counters of the swap as well */
const COUNTERS_EVERY = 5

/** The counters of `vm_stat` (macOS) a stall reads: the pages swapped in and out, and those the compressor gave back */
export function parseVmStat(text: string): Pick<MachineSample, "swapins" | "swapouts" | "decompressions"> {
  const counter = (name: string) => {
    const match = new RegExp(`^${name}:\\s+(\\d+)\\.`, "m").exec(text)
    return match === null ? undefined : Number(match[1])
  }
  return { swapins: counter("Swapins"), swapouts: counter("Swapouts"), decompressions: counter("Decompressions") }
}

/** The swap counters of `/proc/vmstat` (Linux) */
export function parseProcVmStat(text: string): Pick<MachineSample, "swapins" | "swapouts"> {
  const counter = (name: string) => {
    const match = new RegExp(`^${name} (\\d+)$`, "m").exec(text)
    return match === null ? undefined : Number(match[1])
  }
  return { swapins: counter("pswpin"), swapouts: counter("pswpout") }
}

/** The counters of the swap and the compressor that a sample holds */
type Counters = Pick<MachineSample, "swapins" | "swapouts" | "decompressions">

/** How the sampler reads the counters: at once (the last sample) and in the background (the samples of the run) */
export interface CounterReaders {
  now: () => Counters | null
  later: (done: (counters: Counters | null) => void) => void
}

const VM_STAT = "/usr/bin/vm_stat"

/** The counters of this machine: `/proc/vmstat` on Linux, a `vm_stat` on macOS, none elsewhere */
export const SYSTEM_COUNTERS: CounterReaders = {
  now: () => {
    try {
      if (platform() === "linux") {
        return parseProcVmStat(readFileSync("/proc/vmstat", "utf8"))
      }
      return platform() === "darwin" ? parseVmStat(execFileSync(VM_STAT, { encoding: "utf8", timeout: 2000 })) : null
    } catch {
      return null
    }
  },
  later: done => {
    if (platform() === "darwin") {
      execFile(VM_STAT, (error, stdout) => done(error === null ? parseVmStat(stdout) : null))
    } else {
      done(SYSTEM_COUNTERS.now())
    }
  },
}

/**
 * Samples the machine every `SAMPLE_MS` from `start` to `stop`, on a timer that keeps no run alive, and once more as it
 * stops, with the counters read at once (one `vm_stat`): a stall at the end of the run has a sample of the counters
 * after it too
 */
export class MachineSampler {
  readonly samples: MachineSample[] = []
  private timer: NodeJS.Timeout | undefined
  private reading = false
  private count = 0

  constructor(private readonly counters: CounterReaders = SYSTEM_COUNTERS) {}

  start(): void {
    this.sample()
    this.timer = setInterval(() => this.sample(), SAMPLE_MS)
    this.timer.unref()
  }

  stop(): void {
    clearInterval(this.timer)
    this.samples.push({ ...current(), ...this.counters.now() })
  }

  private sample(): void {
    const sample = current()
    if (this.count++ % COUNTERS_EVERY !== 0 || this.reading) {
      // one read of the counters at a time: a machine slow enough to make it outlast a second samples the load alone
      this.samples.push(sample)
      return
    }
    this.reading = true
    this.counters.later(counters => {
      this.reading = false
      this.samples.push({ ...sample, ...counters })
    })
  }
}

/** The load and the free memory now */
function current(): MachineSample {
  return { at: Date.now(), load: loadavg()[0] ?? 0, freeBytes: freemem() }
}
