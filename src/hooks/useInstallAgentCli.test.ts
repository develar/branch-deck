import { describe, expect, it } from "vitest"
import { describeInstall } from "@/hooks/useInstallAgentCli"

const claudeSkill = "/Users/ann/.claude/skills/branch-deck/SKILL.md"
const skills = [claudeSkill, "/Users/ann/.agents/skills/branch-deck/SKILL.md"]

describe("describeInstall", () => {
  it("names the skills, and bd as a command when it is on PATH", () => {
    expect(describeInstall({ bd: "/Users/ann/.local/bin/bd", on_path: true, skills })).toBe(
      `Coding agents find bd through the skills in ${skills.join(", ")}. In a terminal, run bd.`,
    )
  })

  it("names the full path and the directory to add to PATH when bd is not on it", () => {
    expect(describeInstall({ bd: "C:\\Program Files\\BranchDeck\\bd.exe", on_path: false, skills: [claudeSkill] })).toBe(
      `Coding agents find bd through the skill in ${claudeSkill}. In a terminal, run C:\\Program Files\\BranchDeck\\bd.exe, or add C:\\Program Files\\BranchDeck to PATH to run bd.`,
    )
  })
})
