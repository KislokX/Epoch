/**
 * What every test can assume.
 *
 * Two things, and deliberately no more: the extra matchers, and a Tauri that is not there.
 */

import "@testing-library/jest-dom/vitest";
import { afterEach, vi } from "vitest";
import { cleanup } from "@testing-library/react";

/**
 * There is no Tauri in a test run.
 *
 * `invoke` and `listen` reach a shell that does not exist here, and a component that called one
 * would hang rather than fail — which is the worst shape a test failure can take. Mocked at the
 * module boundary so a component under test behaves exactly as it does when the Engine is slow:
 * it asked, and nothing has come back yet.
 *
 * Tests that need an answer say so themselves with `vi.mocked(invoke).mockResolvedValue(...)`.
 * Nothing is answered by default, because a default answer is a fact the test did not state.
 */
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => new Promise(() => {})),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
