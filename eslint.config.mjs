// @ts-check
import withNuxt from './.nuxt/eslint.config.mjs'

export default withNuxt()
  .append({
    files: ['**/*.{js,mjs,cjs,ts,vue}'],
    rules: {
      'vue/max-attributes-per-line': ['error', {
        singleline: { max: 3 },
        multiline: { max: 1 }
      }],
      'vue/multi-word-component-names': 'off',
    }
  })
  .append({
    ignores: [
      '**/app/utils/bindings.ts',
      '**/src-tauri/target/**',
      '**/migration-backup/**',
      '**/.tools/**'
    ]
  })
