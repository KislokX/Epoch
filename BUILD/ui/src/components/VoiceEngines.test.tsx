/**
 * What the voice deck must say, in the three states it has.
 *
 * The interesting one is **not here**: a row that only greys out teaches nobody what to do, and
 * this is the one program on the deck Epoch downloads itself — so its size and its licence have
 * to be on screen *before* the press rather than discovered afterwards.
 */

import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const engines = vi.fn();

const ears = vi.fn();

const forge = vi.fn();

vi.mock("../ipc/launcher", () => ({
  voiceEngines: () => engines(),
  voiceEars: () => ears(),
  voiceForge: () => forge(),
  installedTimbres: vi.fn(async () => []),
  installVoiceEngine: vi.fn(),
  installEar: vi.fn(),
  prepareVoiceForge: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));

import { VoiceEngines } from "./VoiceEngines";

const PIPER = {
  id: "piper",
  name: "Piper",
  installed: false,
  at: null,
  ours: false,
  archive: {
    url: "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_windows_amd64.zip",
    name: "piper_windows_amd64.zip",
    bytes: 22_500_000,
  },
  installing: "Epoch downloads Piper's own release archive.",
  licence: "GPL-3.0 · the voices it reads are separately licensed",
};

describe("the voice deck", () => {
  beforeEach(() => {
    engines.mockReset();
    ears.mockReset();
    forge.mockReset();
    // `null` is *unasked*, which is what a machine nobody has measured honestly is -- and the
    // row must say so rather than reading as a cold forge.
    forge.mockResolvedValue(null);
    // The ear is a separate list; these tests are about the mouth unless they say otherwise.
    ears.mockResolvedValue([]);
  });

  it("offers the ear and its models on one row, each saying what it costs", async () => {
    engines.mockResolvedValue([]);
    ears.mockResolvedValue([
      {
        id: "whisper",
        name: "whisper.cpp",
        installed: false,
        at: null,
        ours: false,
        models: [
          { id: "base", file: "ggml-base.bin", bytes: 147_951_465,
            about: "About a second for anything said in one breath.", here: false },
          { id: "small", file: "ggml-small.bin", bytes: 487_601_967,
            about: "About three seconds.", here: true },
        ],
        archive: { url: "https://example/x.zip", name: "x.zip", bytes: 8_361_840 },
        installing: "Epoch downloads whisper.cpp.",
        licence: "MIT",
      },
    ]);
    render(<VoiceEngines />);

    expect(await screen.findByText(/whisper.cpp — listening/)).toBeTruthy();
    expect(screen.getByText(/INSTALL WHISPER.CPP/)).toBeTruthy();
    expect(screen.getByText(/About 8 MB, under MIT/)).toBeTruthy();
    // A model already here is not offered again, and one that is not says what it costs.
    expect(screen.getByText(/148 MB/)).toBeTruthy();
    expect(screen.getByText(/GET IT/)).toBeTruthy();
    expect(screen.getByText(/— installed\./)).toBeTruthy();
  });

  it("says what it would download, how big it is and under what licence", async () => {
    engines.mockResolvedValue([PIPER]);
    render(<VoiceEngines />);

    expect(await screen.findByText("NOT HERE")).toBeTruthy();
    expect(screen.getByText(/INSTALL PIPER/)).toBeTruthy();
    // The three things a person needs before pressing: what it does, what it costs, what it is.
    expect(screen.getByText(/23 MB/)).toBeTruthy();
    expect(screen.getByText(/GPL-3/)).toBeTruthy();
  });

  it("explains the missing SERVING lamp rather than leaving a hole where one should be", async () => {
    engines.mockResolvedValue([PIPER]);
    render(<VoiceEngines />);

    // Somebody reading the two decks beside this one looks for the third state. An unexplained
    // absence reads as a broken instrument.
    expect(
      await screen.findByText(/no server to start or stop here/),
    ).toBeTruthy();
    expect(screen.queryByText("SERVING")).toBeNull();
  });

  it("tells an install Epoch made from one the user already had", async () => {
    engines.mockResolvedValue([
      { ...PIPER, installed: true, ours: true, at: "C:/…/tools/piper/piper.exe" },
    ]);
    const ours = render(<VoiceEngines />);
    expect(await screen.findByText(/fetched by Epoch/)).toBeTruthy();
    ours.unmount();

    // Not decoration: Epoch may delete what it fetched and may not delete somebody else's.
    engines.mockResolvedValue([
      { ...PIPER, installed: true, ours: false, at: "C:/tools/piper.exe" },
    ]);
    render(<VoiceEngines />);
    expect(await screen.findByText(/Epoch will not touch/)).toBeTruthy();
  });

  it("does not offer a download it has not measured for this platform", async () => {
    engines.mockResolvedValue([{ ...PIPER, archive: null }]);
    render(<VoiceEngines />);

    await waitFor(() => expect(screen.getByText("NOT HERE")).toBeTruthy());
    expect(screen.queryByText(/INSTALL PIPER/)).toBeNull();
    // The frame stays and names what is missing, which is what a cold instrument owes.
    expect(screen.getByText(/no measured download/)).toBeTruthy();
  });

  it("says it is asking rather than drawing an empty deck", async () => {
    engines.mockReturnValue(new Promise(() => {}));
    render(<VoiceEngines />);

    // Unasked is not empty. A folder walk takes a moment and nothing meanwhile reads as *none*.
    expect(await screen.findByText(/Asking this machine/)).toBeTruthy();
  });
});

/**
 * The RVC row, in the three states it has.
 *
 * Its own block because the interesting rule is not *does it render* but **one next step, never
 * a list** — a row showing four unticked boxes would be asking somebody to work out an order
 * that is already fixed.
 */
describe("the RVC row", () => {
  const cold = {
    python: null,
    howToGetPython: "winget install Python.Python.3.12",
    environment: false,
    torch: false,
    definitions: false,
    encoder: false,
    runtime: false,
    cost: "About 870 MB, once.",
    canSpeak: false,
    nextStep: "This machine has no Python that Epoch can build its own environment from.",
  };

  beforeEach(() => {
    engines.mockResolvedValue([]);
    ears.mockResolvedValue([]);
  });

  it("says the one thing in the way, and never the whole list", async () => {
    forge.mockResolvedValue({ ...cold, environment: true, torch: true, definitions: true,
      nextStep: "The encoder that listens to a voice is not converted yet." });
    render(<VoiceEngines />);

    expect(await screen.findByText(/encoder that listens/)).toBeTruthy();
    // The steps already taken are not listed back: they are done, and reading them is work.
    expect(screen.queryByText(/model definitions/)).toBeNull();
  });

  it("shows the command instead of a button when Python is the missing piece", async () => {
    forge.mockResolvedValue(cold);
    render(<VoiceEngines />);

    // Shown, never run. Installing a language for the whole machine is the user's decision and
    // the user's password.
    expect(await screen.findByText(/winget install Python/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: /PREPARE IT/ })).toBeNull();
  });

  it("a machine it could not ask claims nothing either way", async () => {
    // `null` is unasked. Reading it as a cold forge would tell somebody to install Python they
    // already have -- an invented reading wearing a verdict.
    forge.mockResolvedValue(null);
    render(<VoiceEngines />);

    expect(await screen.findByText(/could not be asked about RVC/)).toBeTruthy();
    expect(screen.queryByText(/winget install Python/)).toBeNull();
  });
});
