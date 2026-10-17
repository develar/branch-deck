import React, { useState as useLocalState, useEffect } from "react"
import * as path from "node:path"
import type { Shape } from "./declarations"
import "./side-effect"
import fs = require("fs")
export { Circle } from "./declarations"
export * from "./all"
export * as geometry from "./geometry"

export function run(): void {
  useEffect(() => useLocalState(path.join("a", "b")))
  fs.readFileSync(React.version)
}
