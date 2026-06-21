// Flat config (ESLint 9). Type-aware linting of the shell's TypeScript under src/. The browser
// dependencies are declared in package.json; this is dev tooling, not bundled into the build.
import js from "@eslint/js";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  // src/shell/generated/ is emitted by ts-rs (cargo test --features ts-export); not hand-edited.
  { ignores: ["node_modules/**", "**/*.tera", "src/shell/generated/**"] },
  {
    files: ["src/**/*.ts"],
    extends: [js.configs.recommended, ...tseslint.configs.recommendedTypeChecked],
    languageOptions: {
      globals: { ...globals.browser },
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
  },
);
