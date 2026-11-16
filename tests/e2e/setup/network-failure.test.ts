import { describe, expect, it } from "vite-plus/test"
import { isLeftBehind } from "./network-failure"

describe("isLeftBehind", () => {
  it("tells a request a leaving page left behind by its first line, also with the stack the log of the page puts after it", () => {
    // the line of moved-lines.spec.ts:292 in WebKit: the log of the page, the stack of the rejection after its words
    expect(
      isLeftBehind(
        "Unhandled rejection of the page: Load failed\narrayBuffer@[native code]\nreadAnswer@web-inspector://bootstrap.js:1455:46\nasync invokeServer@web-inspector://bootstrap.js:1449:26",
      ),
    ).toBe(true)
    // the console of WebKit and its uncaught error, one line each
    expect(isLeftBehind("Uncaught: Unhandled Promise Rejection: TypeError: Load failed")).toBe(true)
    expect(isLeftBehind("Load failed")).toBe(true)
    expect(isLeftBehind("Fetch API cannot load http://localhost:3030/invoke/get_review due to access control checks.")).toBe(true)
  })

  it("leaves every other error an error, a stack that ends in the words of the network too", () => {
    expect(isLeftBehind("Unhandled rejection of the page: Cannot read properties of undefined (reading 'path')\nat readAnswer")).toBe(false)
    expect(isLeftBehind("Cannot read the review of a branch ahead: the branch is gone\nLoad failed")).toBe(false)
    expect(isLeftBehind("Load failed to parse the settings")).toBe(false)
    expect(isLeftBehind("Failed to fetch")).toBe(false)
  })
})
