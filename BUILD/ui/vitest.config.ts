/**
 * The test harness for the presentation layer.
 *
 * ## Why this exists at all
 *
 * 13,811 lines of TypeScript had **no tests of any kind** — and 407 passing Rust tests did not
 * catch the three defects found by using the product: a refused tool shown as work, an effect
 * keyed on a stable callback that therefore ran once, and an overlay that rendered perfectly and
 * could not be clicked. All three were in this layer.
 *
 * ## Separate from `vite.config.ts`, deliberately
 *
 * The app's build has a Tauri plugin, a fixed port and an asset pipeline that a test run has no
 * use for. One config serving both would mean every test run carrying the app's build concerns,
 * and every build carrying the test environment's.
 */

import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    // The DOM the components expect. Node alone cannot render them.
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    // Only ours. `node_modules` contains tests belonging to other people's packages.
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
