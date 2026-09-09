/**
 * What the panel says once a picture is being drawn on top of.
 *
 * Both of these came from a run where the picture never arrived and **nothing on the panel said
 * so**: the row read REPLACE THAT PICTURE, every control looked right, and the only sign was
 * that the result came back 1024×1024 where the photo is 1280×960. That is a fact the window
 * held the whole time and never showed.
 *
 * So: the picture in play is shown, and SIZE stops offering a list it does not decide.
 */

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const handed = vi.fn(async () => "epoch-ref-1122334455667788.png");

vi.mock("../ipc/launcher", async (real) => ({
  ...(await real<Record<string, unknown>>()),
  closeStudio: vi.fn(async () => null),
  drawFromPanel: vi.fn(async () => ({ file: "a.png", at: "C:/a.png", seconds: 1 })),
  handReferenceOver: (...args: unknown[]) => handed(...(args as [])),
  studioPanel: vi.fn(async () => view),
}));

import { StudioPanel } from "./StudioPanel";
import type { PanelView } from "../ipc/launcher";

let view: PanelView;

function base(): PanelView {
  return {
    models: [
      {
        file: "sd_xl_base_1.0.safetensors",
        family: "sdxl",
        saidBase: null,
        bytes: 6_938_040_714,
        kind: "checkpoint",
        needs: [],
        with: [],
        makes: "picture" as const,
        carriesEncoder: true,
      },
    ],
    loras: [],
    shapes: [{ label: "Square", width: 1024, height: 1024, native: true }],
    encoders: [],
    vaes: [],
    upscalers: [],
    controlnets: [],
    preparations: ["Canny"],
    motions: ["LTXV"],
    sounds: [],
    lyrical: [],
    meshes: [],
    serving: true,
    embeddings: [],
    clipTypes: [],
    clipTypesTwo: [],
    assembly: null,
    whereAt: "at http://127.0.0.1:8188",
    problem: null,
    remembered: [],
  } as PanelView;
}

/** The picker for what a render is drawn on top of — never the steering rows' one. */
async function handOver() {
  const inputs = [...document.querySelectorAll<HTMLInputElement>("input[type=file]")];
  const one = inputs.find((it) => !it.closest(".cedit__steer"));
  if (!one) throw new Error("no picker for the base picture");
  await userEvent.upload(one, new File([" "], "us.png", { type: "image/png" }));
}

function sizePicker(): HTMLSelectElement | undefined {
  return screen
    .getAllByRole("combobox")
    .find((it) =>
      [...(it as HTMLSelectElement).options].some((o) => o.text.includes("1024")),
    ) as HTMLSelectElement | undefined;
}

describe("drawing on top of a picture", () => {
  beforeEach(() => {
    handed.mockClear();
    view = base();
  });

  it("shows the picture that is actually in play", async () => {
    render(<StudioPanel onChosen={() => {}} />);
    await screen.findByText(/FROM A PICTURE/i);

    // Nothing handed over yet: nothing to show, and SIZE is a real choice.
    expect(screen.queryByAltText(/drawn on top of/i)).toBeNull();
    expect(sizePicker()).toBeDefined();

    await handOver();

    // The one confirmation that cannot be misread — and it is the picture itself, so a
    // hand-over of the *wrong* file is visible too, not only a hand-over that failed.
    const shot = await screen.findByAltText(/drawn on top of/i);
    expect(shot.getAttribute("src")).toMatch(/^data:image\/png/);
  });

  it("stops offering a size the picture already decides", async () => {
    render(<StudioPanel onChosen={() => {}} />);
    await screen.findByText(/FROM A PICTURE/i);
    await handOver();
    await screen.findByAltText(/drawn on top of/i);

    // `VAEEncode` takes the picture's own dimensions, so every value in that list was ignored.
    expect(sizePicker()).toBeUndefined();
    expect(
      screen.getByText(/The picture you handed over decides this/i),
    ).toBeTruthy();

    // And it comes back the moment there is a size to choose again.
    await userEvent.click(screen.getByText("FROM NOTHING"));
    expect(sizePicker()).toBeDefined();
  });

  it("keeps the picture even when the browser never decodes it", async () => {
    // jsdom never fires `onload` for a data URI, which is exactly the case this asserts: the
    // measurement is a detail of the confirmation and may never hold the picture up. The
    // sentence says less rather than waiting.
    render(<StudioPanel onChosen={() => {}} />);
    await screen.findByText(/FROM A PICTURE/i);
    await handOver();

    await screen.findByAltText(/drawn on top of/i);
    expect(screen.getByText(/decides this\./i)).toBeTruthy();
    expect(screen.queryByText(/decides this:/i)).toBeNull();
  });
});
