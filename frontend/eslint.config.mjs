import pluginVue from "eslint-plugin-vue"

import { defineConfigWithVueTs, vueTsConfigs } from "@vue/eslint-config-typescript"

export default defineConfigWithVueTs(pluginVue.configs["flat/recommended"], vueTsConfigs.recommended, {
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
  },
})
