import { describe, expect, it } from "vite-plus/test"
import { codeMenuItems, codeMenuKeyPoint } from "@/lib/diff/code-menu"
import { FROM_MENU } from "@/lib/menu"

const VIEW = { left: 100, top: 50, right: 900, bottom: 650 }

describe("the menu of the code", () => {
  it("offers both items for a name, and disables them with the reason where no name is", () => {
    const calls: string[] = []
    const context = { goToDeclaration: () => calls.push("declaration"), findUsages: () => calls.push("usages") }
    const named = codeMenuItems({ ...context, name: "helper" })
    expect(named.map(item => [item.id, item.disabled, item.description])).toEqual([
      ["go-to-declaration", false, "Go to the declaration of helper"],
      ["find-usages", false, "Find the usages of helper"],
    ])
    for (const item of named) {
      item.onSelect(FROM_MENU)
    }
    expect(calls).toEqual(["declaration", "usages"])
    expect(codeMenuItems({ ...context, name: null }).map(item => [item.id, item.disabled, item.description])).toEqual([
      ["go-to-declaration", true, "No name here"],
      ["find-usages", true, "No name here"],
    ])
    // a view without Find Usages
    expect(codeMenuItems({ ...context, name: "helper", findUsages: null }).map(item => item.id)).toEqual(["go-to-declaration"])
  })

  it("opens for a menu key under the start of the name at the selection", () => {
    expect(codeMenuKeyPoint({ left: 340, bottom: 212 }, VIEW)).toEqual({ x: 340, y: 212 })
  })

  it("opens for a menu key inside the view when a scroll took the selection out of it", () => {
    expect(codeMenuKeyPoint({ left: 40, bottom: 20 }, VIEW)).toEqual({ x: 100, y: 50 })
    expect(codeMenuKeyPoint({ left: 1200, bottom: 2400 }, VIEW)).toEqual({ x: 900, y: 650 })
  })

  it("opens for a menu key at the start of the view where nothing is selected in it", () => {
    expect(codeMenuKeyPoint(undefined, VIEW)).toEqual({ x: 124, y: 74 })
    // a view smaller than the inset
    expect(codeMenuKeyPoint(undefined, { left: 0, top: 0, right: 10, bottom: 10 })).toEqual({ x: 10, y: 10 })
  })
})
