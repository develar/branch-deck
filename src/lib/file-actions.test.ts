import { describe, expect, it } from "vite-plus/test"
import { fileMenuItems } from "@/lib/file-actions"

describe("fileMenuItems", () => {
  it("copies the path of a file, with the key only where the key works", () => {
    expect(fileMenuItems({ kind: "file", path: "ui/login.js" }, { copyKey: true }).map(item => [item.id, item.label, item.kbds])).toEqual([
      ["copy-path", "Copy Path", ["mod", "c"]],
    ])
    expect(fileMenuItems({ kind: "file", path: "ui/login.js" }, { copyKey: false })[0]!.kbds).toBeUndefined()
  })

  it("copies the path of a directory, or the changed paths under it", () => {
    expect(fileMenuItems({ kind: "directory", path: "ui", files: ["ui/login.js", "ui/login.css"] }, { copyKey: true }).map(item => item.label)).toEqual([
      "Copy Path",
      "Copy Changed Paths",
    ])
  })
})
