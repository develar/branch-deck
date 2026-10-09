// @ts-check
import js from "@eslint/js"
import tseslint from "typescript-eslint"
import reactHooks from "eslint-plugin-react-hooks"
import { reactRefresh } from "eslint-plugin-react-refresh"
import stylistic from "@stylistic/eslint-plugin"

export default tseslint.config(
  {
    ignores: [
      "**/src/lib/bindings.ts",
      "dist/**",
      "target/**",
      "src-tauri/target/**",
      "tests/results/**",
      "playwright-report/**",
      "**/migration-backup/**",
      "**/.tools/**",
      // TODO: legacy Vue sources kept on disk as reference during the React rewrite; remove with them
      "app/**",
      "layers/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["src/**/*.{ts,tsx}"],
    ...reactHooks.configs.flat.recommended,
  },
  {
    files: ["src/**/*.tsx"],
    ...reactRefresh.configs.vite(),
  },
  {
    // generated shadcn/ui components export variant helpers (cva) and the toast manager next to components
    files: ["src/components/ui/**/*.tsx"],
    rules: {
      "react-refresh/only-export-components": "off",
    },
  },
  stylistic.configs.customize({
    indent: 2,
    quotes: "double",
    semi: false,
    jsx: true,
  }),
  {
    files: ["**/*.{js,mjs,cjs,ts,tsx}"],
    rules: {
      // TypeScript reports unknown identifiers
      "no-undef": "off",
      "@typescript-eslint/no-unused-vars": ["error", {
        args: "after-used",
        argsIgnorePattern: "^_",
        ignoreRestSiblings: true,
        vars: "all",
        varsIgnorePattern: "^_",
      }],
      "@typescript-eslint/ban-ts-comment": ["error", { minimumDescriptionLength: 10 }],
      "@typescript-eslint/consistent-type-imports": ["error", { disallowTypeAnnotations: false, prefer: "type-imports" }],
      "@typescript-eslint/no-import-type-side-effects": "error",
      "@typescript-eslint/no-dynamic-delete": "error",
      "@typescript-eslint/no-extraneous-class": "error",
      "@typescript-eslint/no-invalid-void-type": "error",
      "@typescript-eslint/no-non-null-asserted-nullish-coalescing": "error",
      "@typescript-eslint/no-useless-constructor": "error",
      "@typescript-eslint/prefer-literal-enum-member": "error",
      "@typescript-eslint/unified-signatures": "error",
      "@stylistic/eol-last": "off",
      // Enforce braces for all control statements
      "curly": ["error", "all"],
      // Enforce newline before and after braces
      "@stylistic/brace-style": ["error", "stroustrup", { allowSingleLine: false }],
      // Enforce consistent newlines in control statements
      "@stylistic/nonblock-statement-body-position": ["error", "below"],
    },
  },
)
