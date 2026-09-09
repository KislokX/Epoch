import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { ImageShelf, StudioCheck } from "../ipc/launcher";

const shelf = vi.hoisted(() => ({ value: null as ImageShelf | null }));
const walk = vi.hoisted(() => ({ value: [] as StudioCheck[] }));

vi.mock("../ipc/launcher", () => ({
  imageShelf: vi.fn(async () => shelf.value),
  importWorkflow: vi.fn(async () => null),
  attachWorkflow: vi.fn(async () => null),
  forgetWorkflow: vi.fn(async () => null),
  drawUsually: vi.fn(async () => null),
  drawOn: vi.fn(async () => null),
  imageStudios: vi.fn(async () => []),
  // The Creations deck grew a third list (Phase 15). An empty answer is the honest one
  // here: these tests are about what this World can make, not about what can speak.
  voiceEngines: vi.fn(async () => []),
  voiceEars: vi.fn(async () => []),
  // The Speaking deck asks about RVC too. `null` is *unasked*, which is what a mock with
  // nothing behind it honestly is.
  voiceForge: vi.fn(async () => null),
  installedTimbres: vi.fn(async () => []),
  prepareVoiceForge: vi.fn(async () => ""),
  installEar: vi.fn(),
  installVoiceEngine: vi.fn(),
  installStudio: vi.fn(async () => null),
  startStudio: vi.fn(async () => null),
  testStudio: vi.fn(async () => walk.value),
  buildWorkflow: vi.fn(async () => "Basic - sd_xl_base_1.0"),
  // The two newest blocks on this deck. Mocked to their honest empty state — a library with
  // nothing in it and a machine that answers with nothing — because this file is about what the
  // deck *says*, and each of those has its own test.
  findAssets: vi.fn(async () => ({ assets: [], refused: [] })),
  installAsset: vi.fn(async () => ({ said: "", failed: false })),
  studioPanel: vi.fn(async () => ({
    models: [],
    loras: [],
    shapes: [{ label: "1:1", width: 1024, height: 1024 }],
    whereAt: "at http://127.0.0.1:8188",
    problem: null,
  })),
  drawFromPanel: vi.fn(async () => ({ file: "drawn.png", seconds: 1 })),
  sharedImage: vi.fn(async () => null),
}));

import { CreationsPanel } from "./CreationsPanel";

const only = (over: Partial<ImageShelf> = {}): ImageShelf => ({
  // One Style, because a Style exists when something that can draw it arrives. `General` is the
  // exception: it is the absence of a style rather than one of them.
  styles: [{ name: "General", using: [], lit: false }],
  workflows: [],
  usually: "General",
  // One machine, and it is this one: the shape a World has before anything is paired.
  benches: [
    { id: "", name: "THIS MACHINE", here: true, serving: true, models: [] },
  ],
  drawOn: "",
  // A library with nothing in it — which is what a fresh installation has, and the state the
  // panel must not render as a fault.
  library: {
    root: "C:/Users/somebody/AppData/Roaming/Epoch/library/generative",
    note: "ComfyUI has been told where this is.",
    restart: false,
    studioAt: "C:/ComfyUI",
    shelves: [
      { id: "models", name: "MODELS", held: [] },
      { id: "loras", name: "LORAS", held: [] },
    ],
  },
  installed: true,
  serving: true,
  problem: null,
  ...over,
});

describe("what the Creations deck says about what this World can make", () => {
  it("says nothing ships, and why", async () => {
    // A workflow in the box would quietly become the house style, and nobody chose it.
    shelf.value = only();
    render(<CreationsPanel />);
    await waitFor(() =>
      expect(screen.getByText(/Epoch ships no workflow on purpose/)).toBeInTheDocument(),
    );
  });

  it("an unlit Style says what would light it, and never borrows", async () => {
    shelf.value = only();
    render(<CreationsPanel />);
    await waitFor(() =>
      expect(
        screen.getByText("no workflow yet — BUILD ME ONE, or import one below"),
      ).toBeInTheDocument(),
    );
  });

  it("shows no Style nothing can draw", async () => {
    // Five names shipped dark once. A dark reading is a quantity not yet wired; a dark `Anime`
    // was never a quantity - it was a category somebody chose on a Tuesday.
    shelf.value = only();
    render(<CreationsPanel />);
    await waitFor(() => expect(screen.getByText("General")).toBeInTheDocument());
    expect(screen.queryByText("Pixel Art")).not.toBeInTheDocument();
    expect(screen.queryByText("Anime")).not.toBeInTheDocument();
  });

  it("naming what a workflow draws is what creates the Style", async () => {
    // The real cause the second amendment asks for: a file arrived and somebody said what it
    // draws. Epoch never reads the name off the file.
    shelf.value = only({
      workflows: [
        { id: "wet", name: "Wet Media XL", can: ["words"], needs: [], problem: null },
      ],
    });
    render(<CreationsPanel />);
    await waitFor(() => expect(screen.getByText("Wet Media XL")).toBeInTheDocument());

    await userEvent.click(screen.getByRole("button", { name: "IT DRAWS…" }));
    await userEvent.type(screen.getByPlaceholderText("watercolour"), "watercolour");
    await userEvent.click(screen.getByRole("button", { name: "NAME IT" }));

    const { attachWorkflow } = await import("../ipc/launcher");
    expect(attachWorkflow).toHaveBeenCalledWith("watercolour", "wet", true);
  });

  it("will not name a Style with nothing typed", async () => {
    shelf.value = only({
      workflows: [
        { id: "wet", name: "Wet Media XL", can: ["words"], needs: [], problem: null },
      ],
    });
    render(<CreationsPanel />);
    await waitFor(() => expect(screen.getByText("Wet Media XL")).toBeInTheDocument());

    await userEvent.click(screen.getByRole("button", { name: "IT DRAWS…" }));
    expect(screen.getByRole("button", { name: "NAME IT" })).toBeDisabled();
  });

  it("changes its advice once something is importable", async () => {
    shelf.value = only({
      workflows: [
        { id: "snes", name: "SNES Pixel Art", can: ["words"], needs: [], problem: null },
      ],
    });
    render(<CreationsPanel />);
    await waitFor(() =>
      // One row, because one Style exists. The advice changes with what is importable, not with
      // how many names an array happened to hold.
      expect(screen.getByText("no workflow attached — press ATTACH")).toBeInTheDocument(),
    );
  });

  it("tells what a workflow can do and what it uses, both derived", async () => {
    shelf.value = only({
      workflows: [
        {
          id: "snes",
          name: "SNES Pixel Art",
          can: ["words", "a reference"],
          needs: ["sd_xl_base_1.0.safetensors", "pixel-art-xl.safetensors"],
          problem: null,
        },
      ],
    });
    render(<CreationsPanel />);
    await waitFor(() =>
      expect(
        screen.getByText(/draws from words, a reference · uses sd_xl_base/),
      ).toBeInTheDocument(),
    );
  });

  it("names the node a workflow is missing rather than counting them", async () => {
    // "needs 2 custom nodes" sends somebody hunting; `IPAdapterApply` is searchable.
    shelf.value = only({
      workflows: [
        {
          id: "ref",
          name: "Reference Portrait",
          can: [],
          needs: [],
          problem: "this workflow needs nodes this ComfyUI does not have: IPAdapterApply",
        },
      ],
    });
    render(<CreationsPanel />);
    await waitFor(() =>
      expect(screen.getByText(/IPAdapterApply/)).toBeInTheDocument(),
    );
  });

  it("marks which Style is used when nobody says", async () => {
    shelf.value = only({
      styles: [{ name: "General", using: ["Basic SDXL"], lit: true }],
      workflows: [
        { id: "basic", name: "Basic SDXL", can: ["words"], needs: [], problem: null },
      ],
    });
    render(<CreationsPanel />);
    await waitFor(() => expect(screen.getByText("usually")).toBeInTheDocument());
    // And the one already used usually is not offered the button again.
    expect(screen.queryByText("USE USUALLY")).not.toBeInTheDocument();
  });
});

describe("walking the picture chain", () => {
  it("shows every link, and the last one is the one to act on", async () => {
    // The point of the button: each of these fails silently, and only the last line here says
    // what to do about it.
    shelf.value = only();
    walk.value = [
      { step: "ComfyUI is answering", ok: true, said: "at http://127.0.0.1:8188" },
      {
        step: "It has something to load",
        ok: false,
        said: "It reports no checkpoint at all.",
      },
    ];
    render(<CreationsPanel />);
    await userEvent.click(
      await screen.findByRole("button", { name: "TEST THIS STUDIO" }),
    );
    await waitFor(() =>
      expect(screen.getByText(/no checkpoint at all/)).toBeInTheDocument(),
    );
    expect(screen.getByText("at http://127.0.0.1:8188")).toBeInTheDocument();
  });

  it("says it is drawing rather than spinning", async () => {
    // A real picture takes seconds. Somebody who knows a model is loading waits; somebody
    // watching a spinner wonders whether it is stuck.
    shelf.value = only();
    const { testStudio } = await import("../ipc/launcher");
    let release: (rows: StudioCheck[]) => void = () => {};
    vi.mocked(testStudio).mockImplementationOnce(
      () => new Promise((resolve) => (release = resolve)),
    );
    render(<CreationsPanel />);
    await userEvent.click(
      await screen.findByRole("button", { name: "TEST THIS STUDIO" }),
    );
    expect(
      await screen.findByRole("button", { name: "DRAWING…" }),
    ).toBeDisabled();
    release([{ step: "ComfyUI is answering", ok: true, said: "at :8188" }]);
    await waitFor(() =>
      expect(screen.getByText("at :8188")).toBeInTheDocument(),
    );
  });

  it("a command that fails is a broken link, never a passing chain", async () => {
    shelf.value = only();
    const { testStudio } = await import("../ipc/launcher");
    vi.mocked(testStudio).mockRejectedValueOnce(new Error("the window lost it"));
    render(<CreationsPanel />);
    await userEvent.click(
      await screen.findByRole("button", { name: "TEST THIS STUDIO" }),
    );
    await waitFor(() =>
      expect(screen.getByText(/the window lost it/)).toBeInTheDocument(),
    );
  });
});

describe("letting Epoch write one", () => {
  it("says what it built and where it went", async () => {
    // The seven-step import across two applications is what this replaces, so the sentence has
    // to leave somebody able to act rather than merely informed.
    shelf.value = only();
    render(<CreationsPanel />);
    await userEvent.click(
      await screen.findByRole("button", { name: "BUILD ME ONE" }),
    );
    await waitFor(() =>
      expect(
        screen.getByText(
          /Built Basic - sd_xl_base_1\.0 and attached it to General/,
        ),
      ).toBeInTheDocument(),
    );
  });

  it("a machine with no checkpoint is told that, not shown a failure", async () => {
    shelf.value = only();
    const { buildWorkflow } = await import("../ipc/launcher");
    vi.mocked(buildWorkflow).mockRejectedValueOnce(
      "ComfyUI is answering and reports no checkpoint, so there is nothing to build a workflow around.",
    );
    render(<CreationsPanel />);
    await userEvent.click(
      await screen.findByRole("button", { name: "BUILD ME ONE" }),
    );
    await waitFor(() =>
      expect(screen.getByText(/reports no checkpoint/)).toBeInTheDocument(),
    );
  });

  it("one machine is not a choice, so nothing is offered", async () => {
    // A World with nothing paired has one place to draw, and a chooser with a single
    // option is furniture. The Launcher's rule about dormant panels is about readings,
    // not about decisions nobody has.
    shelf.value = only();
    render(<CreationsPanel />);
    await screen.findByText("Advanced · Styles");
    expect(screen.queryByText("Where pictures are made")).toBeNull();
  });

  it("a lent machine that is not serving is listed and cannot be chosen", async () => {
    // Listed, because *not serving right now* and *you never paired one* are different
    // facts and only one of them is fixed by pairing. Not choosable, because a place to
    // draw that refuses every picture is worse than no place at all.
    shelf.value = only({
      benches: [
        { id: "", name: "THIS MACHINE", here: true, serving: true, models: [] },
        {
          id: "bridge-1",
          name: "studio-mac.local",
          here: false,
          serving: false,
          models: [],
        },
      ],
    });
    render(<CreationsPanel />);
    await screen.findByText("Where pictures are made");
    expect(
      screen.getByText("studio-mac.local"),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/not serving ComfyUI when it was last asked/),
    ).toBeInTheDocument();

    // No offer at all, rather than one that cannot be taken: the row says why, and a
    // dead button says only that something is wrong with the button.
    expect(screen.queryByRole("button", { name: "DRAW HERE" })).toBeNull();
  });

  it("choosing a machine says so, and asks the Engine", async () => {
    shelf.value = only({
      drawOn: "",
      benches: [
        { id: "", name: "THIS MACHINE", here: true, serving: true, models: [] },
        {
          id: "bridge-1",
          name: "studio-mac.local",
          here: false,
          serving: true,
          models: [],
        },
      ],
    });
    render(<CreationsPanel />);
    // The one already drawing says so instead of offering itself again.
    await screen.findByText("draws here");
    await userEvent.click(
      screen.getByRole("button", { name: "DRAW HERE" }),
    );
    const { drawOn } = await import("../ipc/launcher");
    await waitFor(() => expect(drawOn).toHaveBeenCalledWith("bridge-1"));
  });

  it("says what a file is from its bytes, and never from its name", async () => {
    // The whole point of ADR-0032's rule, on the screen: a file named `sdxl` that is Flux
    // inside reads as Flux, because nothing anywhere looked at the name.
    shelf.value = only({
      library: {
        root: "C:/library",
        note: "ComfyUI has been told where this is.",
        restart: false,
        studioAt: "C:/ComfyUI",
        shelves: [
          {
            id: "loras",
            name: "LORAS",
            held: [
              {
                file: "pixel_art_sdxl_v3.safetensors",
                kind: "a LoRA",
                base: "Flux",
                bytes: 171_969_616,
                unread: null,
                medium: "picture",
              },
            ],
          },
        ],
      },
    });
    render(<CreationsPanel />);

    // One row per file now rather than one joined sentence per shelf, so the text is read off
    // the row rather than out of a single node — and it carries what the file *makes*, which is
    // the question the filter above it answers.
    await waitFor(() => expect(screen.getByText(/pixel_art_sdxl_v3/)).toBeTruthy());
    const row = screen.getByText(/pixel_art_sdxl_v3/).closest("li");
    expect(row?.textContent).toContain("a LoRA, Flux, 172 MB");
    expect(row?.textContent).toContain("makes a picture");
  });

  it("keeps a file Epoch could not place under every medium", async () => {
    /*
      **The rule the whole filter is built around.** A file Epoch failed to read is still the
      user's, still on their disk, and there is nothing they can do about a failure of Epoch's.
      Dropping it from a filtered list takes somebody's own model off the screen with nothing on
      screen to explain it — the worst outcome this control has available, and the same
      inversion as reading silence as *no*.

      `minimax_h3_fl2va` is the real one: it is on the owner's machine and `understand` returns
      `Unknown` for it.
    */
    shelf.value = only({
      library: {
        root: "C:/library",
        note: "ComfyUI has been told where this is.",
        restart: false,
        studioAt: "C:/ComfyUI",
        shelves: [
          {
            id: "diffusion_models",
            name: "DIFFUSION MODELS",
            held: [
              {
                file: "flux1-dev.safetensors",
                kind: "a diffusion model",
                base: "Flux",
                bytes: 11_901_525_888,
                unread: null,
                medium: "picture",
              },
              {
                file: "minimax_h3_fl2va.safetensors",
                kind: "something Epoch could not identify",
                base: "unknown",
                bytes: 6_000_000_000,
                unread: null,
                medium: null,
              },
            ],
          },
        ],
      },
    });
    render(<CreationsPanel />);
    await waitFor(() => expect(screen.getByText(/flux1-dev/)).toBeTruthy());

    // The filter is a dropdown, so this selects rather than clicks.
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: /showing/i }),
      "video",
    );
    // The picture model is not a video model and goes.
    expect(screen.queryByText(/flux1-dev/)).toBeNull();
    // The one nobody could place stays, and says so.
    const row = screen.getByText(/minimax_h3_fl2va/).closest("li");
    expect(row?.textContent).toContain("Epoch could not tell what it makes");
  });

  it("asks for the one restart only when something was actually written", async () => {
    shelf.value = only();
    const { unmount } = render(<CreationsPanel />);
    await waitFor(() =>
      expect(screen.getByText("ComfyUI has been told where this is.")).toBeTruthy(),
    );
    // Nothing was written, so nothing shouts.
    expect(document.querySelector(".notice--warn")).toBeNull();
    unmount();

    shelf.value = only({
      library: {
        root: "C:/library",
        note: "ComfyUI has just been told where this is. Restart it once.",
        restart: true,
        studioAt: "C:/ComfyUI",
        shelves: [],
      },
    });
    render(<CreationsPanel />);
    await waitFor(() => expect(document.querySelector(".notice--warn")).toBeTruthy());
  });

});
