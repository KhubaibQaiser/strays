import { defineConfig } from "vitest/config";

// unit tests sit next to the code (src/**/*.test.ts); tests/ holds the Playwright suites
export default defineConfig({ test: { include: ["src/**/*.test.ts"] } });
