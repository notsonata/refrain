import eslint from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  { ignores: ['dist/**', 'node_modules/**', 'src-tauri/target/**'] },
  eslint.configs.recommended,
  ...tseslint.configs.recommended,
  ...svelte.configs['flat/recommended'],
  {
    languageOptions: {
      globals: {
        console: 'readonly',
        document: 'readonly',
        window: 'readonly',
      },
    },
  },
  {
    files: ['**/*.svelte'],
    languageOptions: {
      parserOptions: { parser: tseslint.parser },
    },
  },
);
