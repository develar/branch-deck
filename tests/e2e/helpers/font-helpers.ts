import type { Page } from "@playwright/test"

/**
 * The text font of the page (`--font-sans` of src/index.css) has loaded: the page drew its first text in the font after
 * it, and its load reflows that text (the intro of the welcome card at 540 px wraps once more and moves the steps 20 px
 * down), a few hundred milliseconds after the first rows under load. A box measured before it is not the box the user
 * sees, so a page is handed to a test once it loaded; the load asks for the font, so it waits also when no text had
 * asked for it yet
 */
export async function fontsLoaded(page: Page): Promise<void> {
  await page.evaluate(() =>
    document.fonts
      .load("1em InterVariable")
      .then(() => document.fonts.ready)
      .then(() => undefined),
  )
}
