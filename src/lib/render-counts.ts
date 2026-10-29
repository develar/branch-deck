/**
 * The render counter of the page, for the tests that pin what a sync renders (B3 of
 * docs/design/rust-domain-authority.md): `countRender(key)` in the render body of a component adds one to
 * the count of `key` in `window.__bdRenderCounts` when an init script of the test set that map before the page loaded
 * (`recordRenders` of tests/e2e/helpers/render-helpers.ts). It counts the calls of the component's function, a render
 * whose output React keeps included, so it says what a change woke. In the app the map is absent and a render costs
 * one comparison.
 */
const counts: Map<string, number> | undefined = typeof window === "undefined" ? undefined : window.__bdRenderCounts

/** One more render of `key` (`row:<name>`, `status:<name>`, `table`, `island:<root>`), when the page counts them */
export function countRender(key: string): void {
  if (counts !== undefined) {
    counts.set(key, (counts.get(key) ?? 0) + 1)
  }
}
