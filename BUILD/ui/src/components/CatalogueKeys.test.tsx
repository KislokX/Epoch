import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { CatalogueKeyRow } from "../ipc/launcher";

const rows = vi.hoisted(() => ({ value: [] as CatalogueKeyRow[] }));
const saved = vi.hoisted(() => ({ calls: [] as { source: string; value: string }[] }));

vi.mock("../ipc/launcher", () => ({
  catalogueKeys: vi.fn(async () => rows.value),
  saveCatalogueKey: vi.fn(async (source: string, value: string) => {
    saved.calls.push({ source, value });
    rows.value = rows.value.map((row) =>
      row.id === source ? { ...row, held: true } : row,
    );
    return null;
  }),
  forgetCatalogueKey: vi.fn(async (source: string) => {
    rows.value = rows.value.map((row) =>
      row.id === source ? { ...row, held: false } : row,
    );
    return null;
  }),
}));

import { CatalogueKeys } from "./CatalogueKeys";

const civitai = (held: boolean): CatalogueKeyRow => ({
  id: "civitai",
  name: "Civitai",
  held,
  neededFor: "Downloading. Searching Civitai works without one.",
  foundAt: "civitai.com → your account → API Keys",
  caution:
    "A Civitai key can do anything their API allows on your account. Epoch uses it only to download.",
});

describe("the keys asset catalogues want", () => {
  it("says what the key can do before anybody pastes one", async () => {
    // The caution is on screen from the start, not after the fact. A warning that arrives once
    // the token is already stored is a warning that arrived too late.
    rows.value = [civitai(false)];
    render(<CatalogueKeys />);

    await waitFor(() =>
      expect(
        screen.getByText(/anything their API allows on your account/),
      ).toBeTruthy(),
    );
    expect(
      screen.getByText(/Downloading\. Searching Civitai works without one\./),
    ).toBeTruthy();
  });

  it("keeps a pasted key and never shows one back", async () => {
    rows.value = [civitai(false)];
    saved.calls = [];
    render(<CatalogueKeys />);

    const box = await waitFor(() => screen.getByPlaceholderText("paste your key"));
    // A password field, so a key is not readable over somebody's shoulder or in a screenshot.
    expect(box.getAttribute("type")).toBe("password");

    await userEvent.type(box, "  a-key-with-space  ");
    await userEvent.click(screen.getByText("KEEP IT"));

    await waitFor(() => expect(saved.calls.length).toBe(1));
    const only = saved.calls[0]!;
    expect(only.source).toBe("civitai");
    // Untrimmed here on purpose: the Engine trims, because that is the one place it cannot be
    // forgotten. Every site's copy button takes a newline with it.
    expect(only.value).toContain("a-key-with-space");

    // And afterwards there is no box to read it out of.
    await waitFor(() => expect(screen.getByText("FORGET")).toBeTruthy());
    expect(screen.queryByPlaceholderText("paste your key")).toBeNull();
  });

  it("offers nothing to press for a source that needs no key", async () => {
    // Hugging Face answers search and download without an account (measured), so it must not
    // appear at all — a field for a credential nothing would use is a gauge nobody can explain.
    rows.value = [];
    render(<CatalogueKeys />);

    await waitFor(() =>
      expect(screen.getByText(/Searching never needs/)).toBeTruthy(),
    );
    expect(screen.queryByPlaceholderText("paste your key")).toBeNull();
    expect(screen.queryByText("Hugging Face")).toBeNull();
  });
});
