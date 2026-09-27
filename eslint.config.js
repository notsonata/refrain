import eslint from '@eslint/js';
import globals from 'globals';
import svelte from 'eslint-plugin-svelte';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  { ignores: ['dist/**', 'node_modules/**', 'src-tauri/target/**'] },
  eslint.configs.recommended,
  ...tseslint.configs.recommended,
  ...svelte.configs['flat/recommended'],
  {
    languageOptions: {
      globals: globals.browser,
    },
  },
  {
    files: ['**/*.svelte'],
    languageOptions: {
      parserOptions: { parser: tseslint.parser },
    },
    rules: {
      // Legacy Svelte reactive effects intentionally persist assignment state
      // across reruns, which the generic ESLint rule cannot observe.
      'no-useless-assignment': 'off',
      // These components replace native Set/Map instances instead of mutating
      // them as reactive state, so Svelte's reactive collection wrappers add no value.
      'svelte/prefer-svelte-reactivity': 'off',
    },
  },
);
