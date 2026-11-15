import { execFile } from "child_process"
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

/** Samples the machine every `SAMPLE_MS` from `start` to `stop`, on a timer that keeps no run alive */
export class MachineSampler {
  readonly samples: MachineSample[] = []
  private timer: NodeJS.Timeout | undefined
  private reading = false
  private count = 0

  start(): void {
    this.sample()
    this.timer = setInterval(() => this.sample(), SAMPLE_MS)
    this.timer.unref()
  }

  stop(): void {
    clearInterval(this.timer)
  }

  private sample(): void {
    const sample: MachineSample = { at: Date.now(), load: loadavg()[0] ?? 0, freeBytes: freemem() }
    const counters = this.count++ % COUNTERS_EVERY === 0
    if (!counters) {
      this.samples.push(sample)
    } else if (platform() === "linux") {
      try {
        Object.assign(sample, parseProcVmStat(readFileSync("/proc/vmstat", "utf8")))
      } catch {
        // no /proc: the load and the memory only
      }
      this.samples.push(sample)
    } else if (platform() === "darwin" && !this.reading) {
      // one `vm_stat` at a time: a machine slow enough to make it outlast a second samples the load alone meanwhile
      this.reading = true
      execFile("/usr/bin/vm_stat", (error, stdout) => {
        this.reading = false
        this.samples.push(error === null ? { ...sample, ...parseVmStat(stdout) } : sample)
      })
    } else {
      this.samples.push(sample)
    }
  }
}
