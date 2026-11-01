import type { Locator } from "@playwright/test"

/**
 * The point of the map of the files beside the diffs of a review where it shows the start of a section (a
 * `diff-view-file`), a pixel and a half inside it: the document scaled to the track of the map, as `DiffFileMap` scales it
 */
export function mapPointOf(section: Locator): Promise<{ x: number; y: number }> {
  return section.evaluate(element => {
    const view = element.closest<HTMLElement>("[data-testid=diff-view]")!
    const track = view.parentElement!.querySelector("[data-testid=review-file-map-track]")!.getBoundingClientRect()
    const scale = track.height / view.scrollHeight
    return { x: track.left + track.width / 2, y: track.top + (element as HTMLElement).offsetTop * scale + 1.5 }
  })
}
