/**
 * The offer that appears when a model lands.
 *
 * A freshly downloaded model is loaded the conservative way — the safe half of every trade —
 * until somebody maps its curve. Measured on this card, that is 20.3 tok/s against 35.2 at the
 * same context, and until now nothing said so and the deck that can fix it is one nobody had
 * opened.
 *
 * The rules these assertions hold: a failure gets no button, the cost is on the button, and the
 * offer never measures here — it hands the model to MODELS, where the row, the curve and the
 * progress bar already are.
 */

import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ModelsWorkshop } from "./ModelsWorkshop";

/** The listeners this component registers, so a test can deliver an event to one. */
const heard = new Map<string, (event: { payload: unknown }) => void>();

vi.mock("@tauri-apps/api/event", () => ({
  listen: (name: string, fn: (event: { payload: unknown }) => void) => {
    heard.set(name, fn);
    return Promise.resolve(() => heard.delete(name));
  },
}));

vi.mock("../ipc/launcher", async (real) => ({
  ...(await real<Record<string, unknown>>()),
  fetchFeaturedModels: vi.fn(async () => []),
  fetchInstalledModels: vi.fn(async () => []),
  suitedModels: vi.fn(async () => []),
  fetchHuggingFace: vi.fn(async () => ({ signedIn: false, installed: false, as: null })),
}));

async function landed(model: string, failure: string | null = null) {
  await waitFor(() => expect(heard.has("models:pulled")).toBe(true));
  heard.get("models:pulled")!({ payload: { model, failure } });
}

describe("what happens when a model lands", () => {
  beforeEach(() => heard.clear());

  it("offers to measure it, and says what that costs", async () => {
    const measure = vi.fn();
    render(<ModelsWorkshop onMeasure={measure} />);
    await landed("gemma4:12b");

    const button = await screen.findByRole("button", { name: /MEASURE IT/i });
    // Four minutes is a decision, and a button that hides what it costs is one people press
    // once.
    expect(button).toHaveTextContent(/4 MIN/i);

    await userEvent.click(button);
    // It never measures here: the errand goes to the deck that can show it happening.
    expect(measure).toHaveBeenCalledWith("gemma4:12b");
  });

  it("offers nothing when the download failed", async () => {
    render(<ModelsWorkshop onMeasure={vi.fn()} />);
    await landed("gemma4:12b", "the pull was refused");

    expect(await screen.findByText(/the pull was refused/i)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /MEASURE IT/i })).toBeNull();
  });

  it("says the model arrived even with nowhere to send it", async () => {
    // Rendered without the prop: the sentence is still true and the button simply is not there.
    render(<ModelsWorkshop />);
    await landed("gemma4:12b");

    expect(
      await screen.findByText(/gemma4:12b is on this machine now/i),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /MEASURE IT/i })).toBeNull();
  });
});
