import { defineConfig, mergeConfig } from "vitest/config";
import viteConfig from "./vite.config.ts";

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      restoreMocks: true,
      maxWorkers: 2,
      projects: [
        {
          extends: true,
          test: {
            name: "ui",
            environment: "jsdom",
            testTimeout: 15_000,
            include: ["src/**/*.test.{ts,tsx}"],
            setupFiles: ["src/shared/testing/setup.ts"],
          },
        },
        {
          extends: true,
          test: {
            name: "tooling",
            environment: "node",
            include: ["test/**/*.test.ts"],
            exclude: ["test/lint-fixtures/**"],
          },
        },
      ],
    },
  }),
);
