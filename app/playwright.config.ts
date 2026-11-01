import { defineConfig } from "@playwright/test";

// Window UI tests: the pages run in Chromium against the Vite dev server, with the app's commands
// mocked (tests/mock.ts). The real app and OS are covered by tools/overlay-harness.
export default defineConfig({
  testDir: "tests",
  use: { baseURL: "http://localhost:1420", viewport: { width: 780, height: 640 } },
  webServer: { command: "npx vite --port 1420 --strictPort", url: "http://localhost:1420/ui.html", reuseExistingServer: false },
});
