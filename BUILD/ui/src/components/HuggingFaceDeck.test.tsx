import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("../ipc/launcher", () => ({
  huggingFaceEverywhere: vi.fn(async () => [
    {
      machine: "This machine",
      local: true,
      reached: true,
      installed: true,
      version: "1.28.0",
      foundAt: "C:\\Users\\someone\\.local\\bin\\hf.exe",
      user: "KislokX",
    },
    {
      machine: "studio-mac.local",
      local: false,
      reached: true,
      installed: true,
      version: "1.28.0",
      foundAt: "/Users/someone/.local/bin/hf",
      user: null,
    },
    {
      machine: "Studio Mac",
      local: false,
      reached: true,
      installed: false,
      version: null,
      foundAt: null,
      user: null,
    },
    {
      machine: "The machine under the desk",
      local: false,
      reached: false,
      installed: false,
      version: null,
      foundAt: null,
      user: null,
    },
  ]),
}));

import { HuggingFaceDeck } from "./HuggingFaceDeck";

describe("the Hugging Face CLI across the fleet", () => {
  it("answers per machine, because one answer would be about the wrong computer", async () => {
    // `hf` here says nothing about the machine lending a graphics card, and it is *that*
    // machine a model would be downloaded onto. A single "installed" line would be true of the
    // Host and read as true of everything.
    render(<HuggingFaceDeck />);

    await waitFor(() =>
      expect(screen.getByText("studio-mac.local")).toBeTruthy(),
    );
    expect(screen.getByText("Studio Mac")).toBeTruthy();
    expect(screen.getByText("The machine under the desk")).toBeTruthy();
  });

  it("keeps reached, installed and signed in as three separate facts", async () => {
    // Three different fixes — wake the machine, install the tool, sign in — so collapsing any
    // two of them would send somebody to do the wrong one.
    render(<HuggingFaceDeck />);

    await waitFor(() => expect(screen.getByText(/KislokX/)).toBeTruthy());
    // Installed and signed out is a real, working state: it downloads anything ungated.
    expect(screen.getByText(/signed out/)).toBeTruthy();
    expect(screen.getByText("NOT INSTALLED")).toBeTruthy();
    // And a machine nobody could reach was never asked. Saying it has none would be inventing
    // an answer on its behalf.
    expect(screen.getByText(/could not be asked/)).toBeTruthy();
  });

  it("says where it was found, so absent is a fact somebody can check", async () => {
    // The reason PATH alone was not enough: its installer writes to `~/.local/bin`, which is
    // not on the PATH of an application started from a desktop.
    render(<HuggingFaceDeck />);
    // Two of them found it, in two different places — which is the point: the path belongs to
    // the machine, not to Epoch.
    await waitFor(() =>
      expect(screen.getAllByText(/\.local[\\/]bin[\\/]hf/)).toHaveLength(2),
    );
  });
});
