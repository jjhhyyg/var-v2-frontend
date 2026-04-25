// @ts-check
import withNuxt from './.nuxt/eslint.config.mjs'

export default withNuxt({
  rules: {
    'vue/html-indent': 'off',
    'vue/max-attributes-per-line': 'off',
    'vue/singleline-html-element-content-newline': 'off',
    'vue/multiline-html-element-content-newline': 'off',
    'vue/html-closing-bracket-newline': 'off',
    'vue/html-self-closing': 'off',
    '@stylistic/operator-linebreak': 'off',
    'vue/operator-linebreak': 'off'
  }
}).prepend({
  ignores: [
    '**/node_modules/**',
    '**/.nuxt/**',
    '**/dist/**',
    '**/.output/**',
    '**/coverage/**',
    '**/src-tauri/target/**',
    '**/src-tauri/resources/runtime/**',
    '**/src-tauri/resources/models/**',
    '**/.windows-runtime-package/**',
    '**/.tauri-worker-build/**',
    '**/.desktop-worker-venv/**'
  ]
})
