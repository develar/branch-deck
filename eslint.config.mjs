// @ts-check
import js from "@eslint/js"
import tseslint from "typescript-eslint"
import pluginVue from "eslint-plugin-vue"
import stylistic from "@stylistic/eslint-plugin"

export default tseslint.config(
  {
    ignores: [
      "**/app/utils/bindings.ts",
      "dist/**",
      "target/**",
      "src-tauri/target/**",
      "tests/results/**",
      "playwright-report/**",
      "**/migration-backup/**",
      "**/.tools/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...pluginVue.configs["flat/recommended"],
  stylistic.configs.customize({
    indent: 2,
    quotes: "double",
    semi: false,
    jsx: false,
  }),
  {
    files: ["**/*.vue"],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser,
        extraFileExtensions: [".vue"],
      },
    },
    // same core-rule adjustments typescript-eslint applies to .ts files
    rules: tseslint.configs.eslintRecommended.rules,
  },
  {
    files: ["**/*.{js,mjs,cjs,ts,vue}"],
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
      // Vue template formatting rules that the previous Nuxt preset kept off (enabling them would reformat templates)
      "vue/html-closing-bracket-newline": "off",
      "vue/html-closing-bracket-spacing": "off",
      "vue/html-quotes": "off",
      "vue/multiline-html-element-content-newline": "off",
      "vue/mustache-interpolation-spacing": "off",
      "vue/no-multi-spaces": "off",
      "vue/no-spaces-around-equal-signs-in-attribute": "off",
      "vue/singleline-html-element-content-newline": "off",
      // Components must be imported explicitly
      "vue/no-undef-components": "error",
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
      // Enforce braces for all control statements
      "curly": ["error", "all"],
      // Enforce newline before and after braces
      "@stylistic/brace-style": ["error", "stroustrup", { allowSingleLine: false }],
      // Enforce consistent newlines in control statements
      "@stylistic/nonblock-statement-body-position": ["error", "below"],
    },
  },
)
