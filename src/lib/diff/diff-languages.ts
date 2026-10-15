import { setCustomExtension } from "@pierre/diffs"
import type { SupportedLanguages } from "@pierre/diffs"

/** File names and extensions (without the dot) that Shiki has no grammar for, by the grammar that reads them */
export const CUSTOM_EXTENSIONS: Readonly<Record<string, SupportedLanguages>> = {
  "gradle": "groovy",
  "iml": "xml",
  "ipr": "xml",
  "iws": "xml",
  "form": "xml",
  "editorconfig": "ini",
  ".editorconfig": "ini",
  "Cargo.lock": "toml",
}

/** Teaches the highlighter `CUSTOM_EXTENSIONS`; the worker pool receives the map with its render options */
export function registerDiffLanguages(): void {
  for (const [key, lang] of Object.entries(CUSTOM_EXTENSIONS)) {
    setCustomExtension(key, lang)
  }
}
