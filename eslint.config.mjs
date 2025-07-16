// @ts-check
import withNuxt from "./.nuxt/eslint.config.mjs"
import stylistic from "@stylistic/eslint-plugin"

export default withNuxt()
  .prepend(
    stylistic.configs.customize({
      indent: 2,
      quotes: "double",
      semi: false,
      jsx: false,
    }),
  )
  .append({
    files: ["**/*.{js,mjs,cjs,ts,vue}"],
    rules: {
      "vue/html-indent": ["error", 2, {
        attribute: 1,
        baseIndent: 1,
        closeBracket: 0,
        alignAttributesVertically: true,
        ignores: [],
      }],
      "vue/max-attributes-per-line": ["error", {
        singleline: { max: 3 },
        multiline: { max: 1 },
      }],
      "vue/multi-word-component-names": "off",
      "@stylistic/eol-last": "off",
    },
  })
  .append({
    ignores: [
      "**/app/utils/bindings.ts",
      "**/src-tauri/target/**",
      "**/migration-backup/**",
      "**/.tools/**",
    ],
  })
