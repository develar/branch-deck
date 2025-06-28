import pluginVue from "eslint-plugin-vue"

import {defineConfigWithVueTs, vueTsConfigs} from "@vue/eslint-config-typescript"
import stylistic from '@stylistic/eslint-plugin'
import {globalIgnores} from "eslint/config"

export default defineConfigWithVueTs(
  pluginVue.configs["flat/recommended"],
  vueTsConfigs.recommended,
  stylistic.configs.recommended,
  globalIgnores(["src/bindings.ts", "src/vite-env.d.ts"]),
  {
    rules: {
      "vue/max-attributes-per-line": [
        "error",
        {
          singleline: {
            max: 3,
          },
          multiline: {
            max: 1,
          },
        },
      ],
      "@stylistic/quotes": ["error", "double"],
    },
  },
)