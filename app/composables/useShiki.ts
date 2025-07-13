import { ref, onMounted } from "vue"
import { createHighlighter  } from "shiki"
import type {Highlighter} from "shiki";

let highlighterInstance: Highlighter | null = null
const isLoading = ref(true)

export function useShiki() {
  // Initialize Shiki lazily when first needed
  const initShiki = async () => {
    if (!highlighterInstance && isLoading.value) {
      try {
        // Load with minimal set of common languages first
        highlighterInstance = await createHighlighter({
          themes: ["github-light", "github-dark"],
          langs: [
            "javascript",
            "typescript",
            "vue",
            "rust",
            "python",
            "java",
            "kotlin",
            "go",
            "cpp",
            "json",
            "yaml",
            "shell",
            "markdown",
          ],
        })
      }
      catch (error) {
        console.error("Failed to initialize Shiki:", error)
      }
      finally {
        isLoading.value = false
      }
    }
  }

  // Initialize on first use
  onMounted(() => {
    initShiki()
  })

  const highlightCode = async (code: string, lang: string, theme: "light" | "dark" = "light"): Promise<string> => {
    await initShiki()
    if (!highlighterInstance) {
      // Return escaped HTML if highlighter is not ready
      return escapeHtml(code)
    }

    try {
      const themeName = theme === "dark" ? "github-dark" : "github-light"
      return highlighterInstance.codeToHtml(code, {
        lang: lang || "text",
        theme: themeName,
      })
    }
    catch (error) {
      console.error("Failed to highlight code:", error)
      return escapeHtml(code)
    }
  }

  const getLanguageFromPath = (filePath: string): string => {
    const extension = filePath.split(".").pop()?.toLowerCase() || ""

    const extensionMap: Record<string, string> = {
      js: "javascript",
      jsx: "javascript",
      ts: "typescript",
      tsx: "typescript",
      vue: "vue",
      rs: "rust",
      py: "python",
      java: "java",
      cpp: "cpp",
      cc: "cpp",
      cxx: "cpp",
      c: "c",
      h: "c",
      hpp: "cpp",
      go: "go",
      rb: "ruby",
      php: "php",
      swift: "swift",
      kt: "kotlin",
      kts: "kotlin",
      scala: "scala",
      sh: "shell",
      bash: "shell",
      zsh: "shell",
      yml: "yaml",
      yaml: "yaml",
      json: "json",
      xml: "xml",
      html: "html",
      htm: "html",
      css: "css",
      scss: "css",
      sass: "css",
      sql: "sql",
      md: "markdown",
      markdown: "markdown",
      toml: "toml",
      ini: "ini",
      conf: "ini",
      dockerfile: "dockerfile",
      Dockerfile: "dockerfile",
      makefile: "makefile",
      Makefile: "makefile",
    }

    // Check special filenames
    const filename = filePath.split("/").pop() || ""
    if (filename === "Dockerfile") return "dockerfile"
    if (filename === "Makefile" || filename === "makefile") return "makefile"
    if (filename === ".gitignore" || filename === ".dockerignore") return "ini"

    return extensionMap[extension] || "text"
  }

  return {
    highlightCode,
    getLanguageFromPath,
    isLoading,
  }
}

function escapeHtml(text: string): string {
  const map: Record<string, string> = {
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    "\"": "&quot;",
    "'": "&#39;",
  }
  return text.replace(/[&<>"']/g, m => map[m])
}
