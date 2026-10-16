import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { RELATIVE_TIME_TICK_MS, subscribeNow } from "@/hooks/useNow"
import { formatRelativeTime } from "@/lib/time"

const START = Date.parse("2026-10-14T12:00:00Z")

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(START)
})

afterEach(() => {
  vi.useRealTimers()
})

describe("subscribeNow", () => {
  it("ticks every listener of an interval length at once, with one interval that stops with the last listener", () => {
    const first: number[] = []
    const second: number[] = []
    const stopFirst = subscribeNow(15_000, now => first.push(now))
    const stopSecond = subscribeNow(15_000, now => second.push(now))
    expect(vi.getTimerCount()).toBe(1)

    vi.advanceTimersByTime(30_000)
    expect(first).toEqual([START + 15_000, START + 30_000])
    expect(second).toEqual(first)

    stopFirst()
    vi.advanceTimersByTime(15_000)
    expect(first).toHaveLength(2)
    expect(second).toEqual([START + 15_000, START + 30_000, START + 45_000])

    stopSecond()
    expect(vi.getTimerCount()).toBe(0)
    // a second unsubscribe changes nothing
    stopSecond()
    expect(vi.getTimerCount()).toBe(0)
  })

  it("has one interval per interval length", () => {
    const ticks: string[] = []
    const stopFast = subscribeNow(1_000, () => ticks.push("fast"))
    const stopSlow = subscribeNow(60_000, () => ticks.push("slow"))
    expect(vi.getTimerCount()).toBe(2)

    vi.advanceTimersByTime(60_000)
    expect(ticks.filter(it => it === "fast")).toHaveLength(60)
    expect(ticks.filter(it => it === "slow")).toHaveLength(1)

    stopFast()
    expect(vi.getTimerCount()).toBe(1)
    stopSlow()
    expect(vi.getTimerCount()).toBe(0)
  })

  it('ages a relative time without anything else changing: "Last synced just now" becomes "1 minute ago"', () => {
    const lastSyncTime = Date.now()
    let text = `Last synced ${formatRelativeTime(lastSyncTime, Date.now())}`
    const stop = subscribeNow(RELATIVE_TIME_TICK_MS, now => {
      text = `Last synced ${formatRelativeTime(lastSyncTime, now)}`
    })
    vi.advanceTimersByTime(45_000)
    expect(text).toBe("Last synced just now")
    // at most one tick late
    vi.advanceTimersByTime(60_000 - 45_000 + RELATIVE_TIME_TICK_MS)
    expect(text).toBe("Last synced 1 minute ago")
    stop()
  })
})
