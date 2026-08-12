module.exports = {
  root: true,
  parser: "@typescript-eslint/parser",
  plugins: ["@typescript-eslint", "react", "react-hooks"],
  extends: [
    "eslint:recommended",
    "plugin:@typescript-eslint/recommended",
    "plugin:react/recommended",
    "plugin:react-hooks/recommended",
    "prettier",
  ],
  settings: { react: { version: "detect" } },
  env: { browser: true, es2021: true, node: false },
  parserOptions: { ecmaVersion: "latest", sourceType: "module" },
  rules: {
    "react/react-in-jsx-scope": "off",
  },
  ignorePatterns: ["dist", "src-tauri/target"],
};

