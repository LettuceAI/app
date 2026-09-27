import { defineConfig, mergeConfig } from "vitest/config";
import viteConfig from "./vite.config.ts";

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      restoreMocks: true,
      projects: [
        {
          extends: true,
          test: {
            name: "ui",
            environment: "jsdom",
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
          },
        },
      ],
    },
  }),
);
