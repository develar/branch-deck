// @spec openspec/specs/branch/landing-plan/spec.md
import { describe, expect, it } from "vite-plus/test"
import type { PlanLayout } from "@/lib/landing-plan-layout"
import { edgeCurve, layoutPlan } from "@/lib/landing-plan-layout"

/** `unit@column:row` of every cell, then `frame@start-end:row` of every frame */
function format(layout: PlanLayout): string[] {
  const cells = [...layout.cells].map(([unit, cell]) => `${unit}@${cell.column}:${cell.row}`)
  const frames = layout.frames.map(frame => `frame${frame.pair}@${frame.columnStart}-${frame.columnEnd}:${frame.row}`)
  return [...cells, ...frames]
}

describe("layoutPlan", () => {
  it("lays the waves of idea-1 out in columns, each chain on one row past the column between and a pair to land together on one row with its frame below", () => {
    const needs: Record<string, string[]> = {
      "air-tools-rust": ["genui-host-crate"],
      "air-home-watch": ["acp-load-replay"],
      "ci-pack-cache": ["agent-id"],
    }
    const layout = layoutPlan(
      [
        ["genui-host-crate", "acp-load-replay"],
        ["agent-id", "IJAI-7037"],
        ["air-tools-rust", "air-home-watch", "ci-pack-cache"],
      ],
      [{ from: "ci-pack-cache", onto: "agent-id" }],
      unit => needs[unit] ?? [],
    )
    expect(format(layout)).toEqual([
      // a chain on one row: each unit that waits follows what it needs, the cell between kept for its curve
      "genui-host-crate@0:0",
      "air-tools-rust@2:0",
      "acp-load-replay@0:1",
      "air-home-watch@2:1",
      // agent-id takes ci-pack-cache on its row, the price below spans the columns of the frame
      "agent-id@1:2",
      "ci-pack-cache@2:2",
      "IJAI-7037@1:4",
      "frame0@1-2:2",
    ])
    expect(layout.rows).toBe(5)
    expect(layout.columns).toBe(3)
  })

  it("frames the second unit that waits for one needed unit in its own column, its price row kept free", () => {
    const layout = layoutPlan(
      [["util"], ["flow", "ui"]],
      [
        { from: "flow", onto: "util" },
        { from: "ui", onto: "util" },
      ],
      () => ["util"],
    )
    expect(format(layout)).toEqual(["util@0:0", "flow@1:0", "ui@1:2", "frame0@0-1:0", "frame1@1-1:2"])
    expect(layout.rows).toBe(4)
  })
})

describe("edgeCurve", () => {
  it("runs from the right side of the needed card to the left side of the card that waits, its chip halfway", () => {
    const curve = edgeCurve({ left: 0, top: 0, width: 100, height: 40 }, { left: 200, top: 100, width: 100, height: 40 })
    expect(curve.path).toBe("M100 20C150 20 150 120 200 120")
    expect(curve.middle).toEqual({ x: 150, y: 70 })
  })

  it("meets the cards at the middle of their headers, whatever their height", () => {
    const curve = edgeCurve({ left: 0, top: 0, width: 100, height: 40 }, { left: 200, top: 100, width: 100, height: 90 }, 18)
    expect(curve.path).toBe("M100 18C150 18 150 118 200 118")
  })

  it("bows out on the right of a card that waits in the same column", () => {
    const curve = edgeCurve({ left: 0, top: 0, width: 100, height: 40 }, { left: 0, top: 100, width: 100, height: 40 })
    expect(curve.path).toBe("M100 20C140 20 140 120 100 120")
  })
})
