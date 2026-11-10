import type { Locator, Page } from "@playwright/test"
import { expect } from "../fixtures/test-fixtures"

/** The columns of a device row that differ from its background, and their colours */
interface LinePixels {
  /** The columns from the left of the clip, in device pixels */
  readonly columns: readonly number[]
  readonly colours: readonly (readonly [number, number, number])[]
}

/** A pixel is the line's where it differs from the background of its row by more than this, in a channel */
const INK = 24
/** A curve leaving the line keeps its pixels within this of the line's colour, in a channel */
const SAME = 8

/**
 * The line of the lane of `node` (a node of the graph, `lane-node`) where it crosses the `border` of `row`: the device
 * rows just above and just below it, in a clip of 8px around the lane, where a fork curve still runs down its parent's
 * lane. A line that keeps its width and x across the border has the same pixels in both rows, whatever the backgrounds
 * of the rows; an antialiased stroke next to a box does not.
 */
async function linePixelsAcross(page: Page, row: Locator, border: "top" | "bottom", node: Locator): Promise<{ above: LinePixels; below: LinePixels }> {
  // no hover: the pointer draws the path to the root of its branch
  await page.mouse.move(0, 0)
  const lane = await node.boundingBox()
  const box = await row.boundingBox()
  expect(lane).not.toBeNull()
  expect(box).not.toBeNull()
  const y = Math.round(border === "top" ? box!.y : box!.y + box!.height)
  const shot = await page.screenshot({ clip: { x: Math.floor(lane!.x + lane!.width / 2) - 4, y: y - 2, width: 8, height: 4 } })
  return page.evaluate(
    async ({ png, ink }) => {
      const image = new Image()
      image.src = `data:image/png;base64,${png}`
      await image.decode()
      const canvas = document.createElement("canvas")
      canvas.width = image.naturalWidth
      canvas.height = image.naturalHeight
      const context = canvas.getContext("2d")!
      context.drawImage(image, 0, 0)
      const { data, width, height } = context.getImageData(0, 0, canvas.width, canvas.height)
      const pixelsOf = (y: number) => {
        const at = (x: number) => [data[(y * width + x) * 4]!, data[(y * width + x) * 4 + 1]!, data[(y * width + x) * 4 + 2]!] as const
        const background = at(0)
        const columns: number[] = []
        const colours: (readonly [number, number, number])[] = []
        for (let x = 0; x < width; x++) {
          const it = at(x)
          if (it.some((value, channel) => Math.abs(value - background[channel]!) > ink)) {
            columns.push(x)
            colours.push(it)
          }
        }
        return { columns, colours }
      }
      // the clip is 4px from 2px above the border: its last device row above the border and its first below
      const border = Math.floor((2 * height) / 4)
      return { above: pixelsOf(border - 1), below: pixelsOf(border) }
    },
    { png: shot.toString("base64"), ink: INK },
  )
}

/**
 * The lane of `node` crosses the `border` of `row` as one line: the same device pixels in the same colours just above
 * and just below it
 */
export async function expectOneLineAcross(page: Page, row: Locator, border: "top" | "bottom", node: Locator): Promise<void> {
  const { above, below } = await linePixelsAcross(page, row, border, node)
  expect(above.columns.length).toBeGreaterThan(0)
  expect(below.columns).toEqual(above.columns)
  below.colours.forEach((colour, index) => {
    const distance = Math.max(...colour.map((value, channel) => Math.abs(value - above.colours[index]![channel]!)))
    expect(distance, `column ${below.columns[index]}: ${colour.join(",")} below, ${above.colours[index]!.join(",")} above`).toBeLessThanOrEqual(SAME)
  })
}
