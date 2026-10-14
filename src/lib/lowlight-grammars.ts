// @git-diff-view/lowlight runs `createLowlight(all)`, which bundles all ~190 highlight.js grammars (about 1 MB).
// The `lowlight-grammars` plugin in vite.config.ts hands it this module instead: lowlight's common set plus
// the build and config languages of JVM repositories. Other extensions fall back to auto-detection.
import { common, createLowlight } from "lowlight"
import dockerfile from "highlight.js/lib/languages/dockerfile"
import gradle from "highlight.js/lib/languages/gradle"
import groovy from "highlight.js/lib/languages/groovy"
import properties from "highlight.js/lib/languages/properties"
import protobuf from "highlight.js/lib/languages/protobuf"
import scala from "highlight.js/lib/languages/scala"

export { createLowlight }

export const all = { ...common, dockerfile, gradle, groovy, properties, protobuf, scala }
