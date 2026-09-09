import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { PanelAsk, PanelView } from "../ipc/launcher";

const view = vi.hoisted(() => ({ value: null as PanelView | null }));
const asked = vi.hoisted(() => ({ calls: [] as PanelAsk[] }));
const spoken = vi.hoisted(() => ({ said: [] as string[] }));

vi.mock("../ipc/launcher", () => ({
  studioPanel: vi.fn(async () => view.value),
  // Closing the panel lets the studio go — the Engine decides whether that means anything.
  closeStudio: vi.fn(async () => null),
  /*
    **And opening it starts the machine.** `true` here is *already serving*, which is what the
    fixtures describe: every one of them lists models and families, so a panel that reported
    itself mid-start would be describing a different situation than the one under test.
  */
  wakeTheStudio: vi.fn(async () => true),
  /*
    **GENERATE draws now, so this is what it calls.** It used to record the settings and hand
    the prompt to the character, and measured 2026-08-25 the character did not draw: handed
    *"a lighthouse at night"* with the panel open, `gemma4:12b` called `open_studio` again.
    A button called GENERATE that asks somebody else to generate is a button that does not work.
  */
  drawFromPanel: vi.fn(async (ask: PanelAsk) => {
    asked.calls.push(ask);
    // Where it also landed: the folder this World's crew works in (ADR-0025).
    return {
      kind: "drew" as const,
      file: "a1b2c3.png",
      at: "C:/their-project/pictures/a1b2c3.png",
      seconds: 4.2,
    };
  }),
}));

import { StudioPanel } from "./StudioPanel";

const panel = (over: Partial<PanelView> = {}): PanelView => ({
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
    {
      file: "v1-5-pruned-emaonly.safetensors",
      family: "sd15",
      saidBase: null,
      bytes: 4_265_146_304,
      kind: "checkpoint",
      needs: [],
      with: [],
      makes: "picture" as const,
      carriesEncoder: true,
    },
  ],
  loras: [
    {
      file: "pixel-art-xl.safetensors",
      family: "sdxl",
      saidBase: null,
      bytes: 170_000_000,
      triggers: ["pixel"],
      fits: true,
      why: null,
    },
    {
      file: "japanese_vhs_flux.safetensors",
      family: "flux",
      saidBase: null,
      bytes: 18_000_000,
      triggers: ["japanese retro aesthetic"],
      fits: false,
      why: "This is for Flux, and the model you chose is SDXL. It will not load.",
    },
  ],
  shapes: [
    { label: "2:3", width: 680, height: 1224, native: true },
    { label: "1:1", width: 1024, height: 1024, native: true },
    { label: "3:2", width: 1224, height: 680, native: true },
  ],
  // A machine with a checkpoint and nothing that arrives in parts, which is the state these
  // tests are about.
  encoders: [],
  vaes: [],
  // Nothing on the upscale shelf, which is the ordinary state of a fresh machine.
  upscalers: [],
  // Empty is the ordinary state of a fresh ComfyUI: the node exists, the shelf does not.
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
  // Nothing measured about how this family is assembled, which is the honest default.
  assembly: null,
  whereAt: "at http://127.0.0.1:8188",
  problem: null,
  // Nobody has drawn with this model here yet, which is the ordinary state.
  remembered: [],
  ...over,
});

describe("the panel a character opens", () => {
  it("shows an incompatible LoRA with its reason and still lets you use it", async () => {
    // The rule this deck has everywhere: Epoch measured and said; the file is theirs and the
    // answer is theirs. Hiding it would make the download somebody just paid for disappear.
    view.value = panel();
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);

    await waitFor(() =>
      expect(
        screen.getByText(/This is for Flux, and the model you chose is SDXL/),
      ).toBeTruthy(),
    );
    expect(screen.getByText("USE IT ANYWAY")).toBeTruthy();
    // And a compatible one is simply offered.
    expect(screen.getByText("USE")).toBeTruthy();
  });

  it("says what a LoRA needs in a prompt once it is in use, and offers to add it", async () => {
    /*
      **On the row that is using it, and where it can be acted on.**

      It used to be drawn on every row whether or not it was in use, as a fifth cell in a
      four-column grid and in the tone of prose read afterwards. The owner installed a style
      LoRA, set it to 1.50, wrote a prompt without its trained word and got a picture with none
      of the style in it — the word was measured, written beside the file at install, and never
      reached the person who had to type it.

      Offered and never inserted: Epoch does not write words into somebody's prompt.
    */
    view.value = panel();
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);
    await waitFor(() => expect(screen.getByText("USE")).toBeTruthy());

    // Nothing is said about a LoRA nobody is using.
    expect(screen.queryByText(/needs in the prompt/)).toBeNull();

    await userEvent.click(screen.getByText("USE"));
    await waitFor(() =>
      expect(screen.getByText(/needs in the prompt/)).toBeTruthy(),
    );
    // The words themselves sit beside a <b>, so the row is read whole. `pixel` is what the
    // fixture's compatible LoRA carries.
    expect(
      screen.getByText(/needs in the prompt/).parentElement?.textContent,
    ).toContain("pixel");

    // And the prompt is the user's until they press.
    const box = screen.getByPlaceholderText("what should be in the picture");
    await userEvent.type(box, "asuka");
    expect((box as HTMLTextAreaElement).value).toBe("asuka");

    await userEvent.click(screen.getByText("ADD"));
    expect((box as HTMLTextAreaElement).value).toBe("asuka, pixel");
    // Pressed twice is not said twice.
    expect(screen.getByText("IN THE PROMPT")).toBeTruthy();
  });

  it("sends exactly what was chosen, and nothing it was not asked for", async () => {
    view.value = panel();
    asked.calls = [];
    spoken.said = [];
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);

    await waitFor(() => expect(screen.getByText("USE")).toBeTruthy());
    await userEvent.click(screen.getByText("USE"));
    await userEvent.type(
      screen.getByPlaceholderText("what should be in the picture"),
      "asuka",
    );
    await userEvent.selectOptions(
      screen.getByLabelText("SIZE"),
      screen.getByText(/^2:3/) as HTMLOptionElement,
    );
    await userEvent.click(screen.getByText("GENERATE"));

    await waitFor(() => expect(asked.calls.length).toBe(1));
    const only = asked.calls[0]!;
    expect(only.checkpoint).toBe("sd_xl_base_1.0.safetensors");
    // A checkpoint carries its own encoder and VAE: nothing is sent for parts it does not have.
    expect(only.clip).toEqual([]);
    expect(only.clipType).toBe("");
    expect(only.prompt).toBe("asuka");
    expect(only.loras).toEqual([["pixel-art-xl.safetensors", 0.8]]);
    expect(only.width).toBe(680);
    expect(only.height).toBe(1224);
    // Nothing invented for the negative prompt — Epoch has no opinion about what pictures
    // should not contain.
    expect(only.negative).toBe("");
    // And a seed of zero, which the Engine reads as *a different picture every time*.
    expect(only.seed).toBe(0);

    // **The Engine drew it** (11.26): a button called GENERATE that asks somebody else to
    // generate is a button that does not work. What the person typed goes into the graph, and
    // the picture is filed on the Quest as evidence by the Engine itself.
    //
    // And the conversation is not spoken to. Saying the prompt afterwards made the model read it
    // as a fresh request and draw the same picture twice; nothing asked of a model costs nothing
    // and can misread nothing.
    // **The panel closes on the press**, which is what `onChosen` is — the World holds the
    // wait, not a form nobody is filling in any more. Nothing is *spoken* to the model: saying
    // the prompt afterwards is what once made one draw the same picture twice.
    expect(spoken.said.at(-1)).toBe("asuka");
  });

  it("keeps advanced folded", async () => {
    // ADR-0026's rule: the default is that the user touches nothing.
    view.value = panel();
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);
    await waitFor(() => expect(screen.getByText("ADVANCED ▼")).toBeTruthy());
    expect(screen.queryByText("NEGATIVE")).toBeNull();
    await userEvent.click(screen.getByText("ADVANCED ▼"));
    expect(screen.getByText("NEGATIVE")).toBeTruthy();
  });

  it("cannot be pressed with nothing to draw", async () => {
    view.value = panel();
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);
    const button = await waitFor(() => screen.getByText("GENERATE"));
    expect(button).toBeDisabled();
  });

  it("says why nothing can be drawn rather than showing an empty form", async () => {
    view.value = panel({
      models: [],
      loras: [],
      problem: "It is running and reports no model at all.",
    });
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);
    await waitFor(() =>
      expect(
        screen.getByText("It is running and reports no model at all."),
      ).toBeTruthy(),
    );
  });

  it("names the machine that would draw, not the one it is running on", async () => {
    // The defect this repeats no more: a deck once measured the local ComfyUI while a lent one
    // did the drawing, and every reading was true about something.
    view.value = panel({ whereAt: "on studio-mac.local" });
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);
    await waitFor(() =>
      expect(screen.getByText(/on studio-mac\.local/)).toBeTruthy(),
    );
  });

  it("will not draw a model that arrives in parts until the parts are chosen", async () => {
    // Said by a dead button rather than by a refusal after the fact.
    view.value = panel({
      models: [
        {
          file: "z_image_turbo.safetensors",
          family: "unknown",
          saidBase: null,
          bytes: 6_200_000_000,
          kind: "diffusion",
          needs: [],
          with: [],
      makes: "picture" as const,
      carriesEncoder: true,
        },
      ],
      loras: [],
      encoders: [{ file: "qwen_3_4b_fp8_mixed.safetensors", says: null , medium: null }],
      vaes: [{ file: "z_image_ae.safetensors", says: null , medium: null }],
      clipTypes: ["stable_diffusion", "flux", "qwen_image"],
    });
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);

    await waitFor(() =>
      expect(screen.getByText("THIS MODEL ARRIVES IN PARTS")).toBeTruthy(),
    );
    await userEvent.type(
      screen.getByPlaceholderText("what should be in the picture"),
      "a castle",
    );
    // A prompt is not enough while the parts are missing.
    expect(screen.getByText("GENERATE")).toBeDisabled();

    // The family list is ComfyUI's own, not five names typed into a screen.
    expect(screen.getByText("qwen_image")).toBeTruthy();
  });

  it("offers the family list of the loader the chosen encoders select", async () => {
    // **Measured, and it took a refused picture to find.** The three loaders speak three
    // vocabularies: `CLIPLoader` publishes `stable_diffusion` and no `flux`; `DualCLIPLoader`
    // publishes `flux` and `sdxl` and no `stable_diffusion`; `TripleCLIPLoader` has no family
    // input at all. One list for all three let somebody fill the panel in correctly and get
    // *"'stable_diffusion' not in ['sdxl','sd3','flux', …]"* back from the server.
    view.value = panel({
      models: [
        {
          file: "flux1-dev.safetensors",
          family: "flux",
          saidBase: null,
          bytes: 23_800_000_000,
          kind: "diffusion",
          needs: [],
          with: [],
      makes: "picture" as const,
      carriesEncoder: true,
        },
      ],
      loras: [],
      encoders: [
        { file: "clip_l.safetensors", says: "CLIP-L" , medium: null },
        { file: "t5xxl_fp8.safetensors", says: "T5-XXL" , medium: null },
      ],
      vaes: [{ file: "ae.safetensors", says: null , medium: null }],
      clipTypes: ["stable_diffusion", "qwen_image"],
      clipTypesTwo: ["sdxl", "flux"],
    });
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);
    await waitFor(() =>
      expect(screen.getByText("THIS MODEL ARRIVES IN PARTS")).toBeTruthy(),
    );

    // One encoder: the single loader's words.
    await userEvent.selectOptions(
      screen.getByLabelText("TEXT ENCODER"),
      "clip_l.safetensors",
    );
    expect(screen.getByText("stable_diffusion")).toBeTruthy();
    expect(screen.queryByText("flux")).toBeNull();

    // Two: the double loader's, and `stable_diffusion` is gone because it would be refused.
    await userEvent.selectOptions(
      screen.getByLabelText("SECOND ENCODER"),
      "t5xxl_fp8.safetensors",
    );
    expect(screen.getByText("flux")).toBeTruthy();
    expect(screen.queryByText("stable_diffusion")).toBeNull();
  });

  it("says what the family it measured is loaded with, and marks that family in the list", async () => {
    // **The complaint this answers.** Choosing Flux said only that the model "arrives in parts",
    // and then offered every encoder on the machine, every VAE, and twelve family names. Epoch
    // had measured the model as Flux and said nothing about what that means.
    //
    // And measuring first killed the obvious fix: `understand` reads the *kind* of every part
    // correctly and the *family* of almost none — `clip_l`, `t5xxl` and the Flux VAE all came
    // back `unknown` on this machine — so greying the parts that belong elsewhere would have
    // greyed nothing. The family is what was measured, so the family is what is said.
    view.value = panel({
      models: [
        {
          file: "flux1-dev.safetensors",
          family: "flux",
          saidBase: null,
          bytes: 23_800_000_000,
          kind: "diffusion",
          needs: [],
          with: [],
      makes: "picture" as const,
      carriesEncoder: true,
        },
      ],
      loras: [],
      encoders: [
        { file: "clip_l.safetensors", says: "CLIP-L" , medium: null },
        { file: "t5xxl_fp8.safetensors", says: "T5-XXL" , medium: null },
      ],
      vaes: [{ file: "ae.safetensors", says: null , medium: null }],
      clipTypes: ["stable_diffusion"],
      clipTypesTwo: ["sdxl", "flux"],
      assembly: {
        family: "Flux",
        encoders: 2,
        says: "Flux is loaded with two text encoders (a CLIP-L and a T5-XXL) and its own VAE.",
        clipType: "flux",
        wants: ["CLIP-L", "T5-XXL"],
      },
    });
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);
    await waitFor(() =>
      expect(screen.getByText("THIS MODEL ARRIVES IN PARTS")).toBeTruthy(),
    );

    // `getAllBy`, because the dead GENERATE now says the same number back: the guidance says
    // what the family takes, and the button says how many of them are still to pick.
    expect(screen.getAllByText(/two text encoders/).length).toBeGreaterThan(0);
    expect(screen.getByText(/2 are/)).toBeTruthy();
    /*
      **The second field says the total, not its own share.** It read *"Flux uses one"*, which is
      about this field and was read as *Flux uses one encoder* — so the owner picked one, found
      GENERATE grey, and hit a server refusal from the first thing they tried instead. Saying the
      total is unambiguous and agrees with the guidance above it.
    */
    expect(screen.getByText(/Flux needs 2 in total/)).toBeTruthy();

    await userEvent.selectOptions(
      screen.getByLabelText(/TEXT ENCODER/),
      "clip_l.safetensors",
    );
    await userEvent.selectOptions(
      screen.getByLabelText(/SECOND ENCODER/),
      "t5xxl_fp8.safetensors",
    );

    /*
      **Marked and chosen, and this is a deliberate narrowing of 11.22.**

      That version marked the measured family and refused to select it, on ADR-0033's rule that
      the panel belongs to the user. The rule is about *files* — which checkpoint, which LoRA,
      which encoder — and the encoder family is not a file: it is a property of the model Epoch
      already read out of its tensors, with one right answer. Leaving it blank made somebody
      choose between twenty-eight names for a question they cannot answer better than the
      measurement can, which is guessing moved onto the user rather than out of the product.

      Every other family stays exactly as selectable as before, which is what keeps it a default
      rather than a decision.
    */
    expect(screen.getByText("flux — what this model is")).toBeTruthy();
    expect(screen.getByText("sdxl")).toBeTruthy();
    const family = screen.getByLabelText("FAMILY") as HTMLSelectElement;
    expect(family.value).toBe("flux");
  });

  it("offers the family list of the loader the model will actually use", async () => {
    /*
      **Measured 2026-08-25 against the real server:** `CLIPLoader` publishes no `flux` at all —
      it has `flux2` and nothing else close — while `DualCLIPLoader` publishes `flux`. Keying the
      list on how many encoders the user had picked so far meant a Flux model opened the panel
      showing twenty-eight families, none of which was right, with `flux2` sitting in the middle
      of them as an active trap. The list only became answerable after the step it was meant to
      help with.

      Epoch measured how many encoders this family takes. That is the number to use.
    */
    view.value = panel({
      models: [
        {
          file: "flux1-dev.safetensors",
          family: "flux",
          saidBase: null,
          bytes: 11_900_000_000,
          kind: "diffusion",
          needs: [],
          with: [],
      makes: "picture" as const,
      carriesEncoder: true,
        },
      ],
      clipTypes: ["stable_diffusion", "sd3", "flux2"],
      clipTypesTwo: ["sdxl", "sd3", "flux"],
      assembly: {
        family: "Flux",
        encoders: 2,
        says: "Flux is loaded with two text encoders.",
        clipType: "flux",
        wants: ["CLIP-L", "T5-XXL"],
      },
    });
    render(<StudioPanel onChosen={() => {}} />);

    const family = (await screen.findByLabelText(
      "FAMILY",
    )) as HTMLSelectElement;
    const values = [...family.options].map((it) => it.value);
    expect(values).toContain("flux");
    expect(values).not.toContain("flux2");
    // And chosen, because it is a measurement and not a taste.
    expect(family.value).toBe("flux");
  });

  it("offers a Custom row, and marks only the sizes the model was trained at", async () => {
    /*
      Three ratio buttons were the whole offer, which quietly decided that nobody wants a
      wallpaper. **Which** sizes exist is the Engine's answer — `shapes_for` owns that list and
      its own test holds it — so what this asserts is the two things the surface decides:
      that a size outside the presets is reachable, and that a size the family was not trained
      at does not claim to be one.

      Marked, never fenced off: asking SD 1.5 for 1024 gives two heads and asking Flux for 4K
      gives an out-of-memory, and neither is Epoch's call to make for somebody who owns the card.
    */
    view.value = panel({
      shapes: [
        { label: "1:1", width: 1024, height: 1024, native: true },
        { label: "Full HD", width: 1920, height: 1080, native: false },
      ],
    });
    render(<StudioPanel onChosen={() => {}} />);

    const size = (await screen.findByLabelText("SIZE")) as HTMLSelectElement;
    const labels = [...size.options].map((it) => it.textContent ?? "");
    expect(labels.some((it) => /Full HD . 1920.1080/.test(it))).toBe(true);
    expect(labels.some((it) => /Custom/.test(it))).toBe(true);
    expect(labels.some((it) => /1:1.*trained at/.test(it))).toBe(true);
    expect(labels.some((it) => /Full HD.*trained at/.test(it))).toBe(false);
  });

  it("rounds a custom size to what the sampler will actually use", async () => {
    // Latent space works in multiples of eight and the sampler rounds silently. A panel that
    // showed 1001 and drew 1000 would be reporting a number nobody used.
    view.value = panel();
    asked.calls = [];
    render(<StudioPanel onChosen={() => {}} />);

    const size = (await screen.findByLabelText("SIZE")) as HTMLSelectElement;
    await userEvent.selectOptions(size, "-1");
    const width = screen.getByLabelText("WIDTH");
    await userEvent.clear(width);
    await userEvent.type(width, "1001");
    await userEvent.type(
      screen.getByPlaceholderText("what should be in the picture"),
      "asuka",
    );
    await userEvent.click(screen.getByText("GENERATE"));

    await waitFor(() => expect(asked.calls.length).toBe(1));
    expect(asked.calls[0]!.width).toBe(1000);
  });

  it("says nothing about assembly for a family it has no account of", () => {
    // `null` is the honest answer for an unmeasured model. Inventing a sentence about how an
    // unknown family is put together is the same invention as an invented gauge.
    view.value = panel({
      models: [
        {
          file: "mystery.safetensors",
          family: "unknown",
          saidBase: null,
          bytes: 1,
          kind: "diffusion",
          needs: [],
          with: [],
      makes: "picture" as const,
      carriesEncoder: true,
        },
      ],
      loras: [],
      encoders: [{ file: "clip_l.safetensors", says: "CLIP-L" , medium: null }],
      vaes: [{ file: "ae.safetensors", says: null , medium: null }],
      assembly: null,
    });
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);
    expect(screen.queryByText(/is loaded with/)).toBeNull();
  });

  it("names each part by what it is, since a part has no family to grey it by", async () => {
    // The complaint, generalised: a model in parts offered every encoder on the machine with
    // nothing to tell them apart. A part cannot be greyed by family because it does not have one
    // — `clip_l.safetensors` is the same file beside Flux and beside SDXL. What is in the bytes
    // is what it *is*: 768 wide is a CLIP-L, `shared.weight` at 4096 is a T5-XXL.
    view.value = panel({
      models: [
        {
          file: "flux1-dev.safetensors",
          family: "flux",
          saidBase: null,
          bytes: 1,
          kind: "diffusion",
          needs: [],
          with: [],
      makes: "picture" as const,
      carriesEncoder: true,
        },
      ],
      loras: [],
      encoders: [
        { file: "clip_l.safetensors", says: "CLIP-L" , medium: null },
        { file: "t5xxl_fp8.safetensors", says: "T5-XXL" , medium: null },
        // A file Epoch does not hold. Silence, not a claim that it will not work.
        { file: "somebody_elses.safetensors", says: null , medium: null },
      ],
      vaes: [{ file: "z_image_ae.safetensors", says: "Flux.1-AE" , medium: null }],
      clipTypesTwo: ["flux"],
      assembly: {
        family: "Flux",
        encoders: 2,
        says: "Flux is loaded with two text encoders (a CLIP-L and a T5-XXL) and its own VAE.",
        clipType: "flux",
        wants: ["CLIP-L", "T5-XXL"],
      },
    });
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);
    await waitFor(() =>
      expect(screen.getByText("THIS MODEL ARRIVES IN PARTS")).toBeTruthy(),
    );

    // The guidance names CLIP-L and T5-XXL; the rows now say which file is which, so the two
    // halves meet without Epoch ever claiming a file belongs to a family it cannot see.
    // `getAllByText` with a matcher, because a row that is one of the two the family wants now
    // says so on the row — the guidance and the list meet in the list.
    expect(
      screen.getAllByText(/clip_l\.safetensors · CLIP-L/).length,
    ).toBeGreaterThan(0);
    expect(
      screen.getAllByText(/t5xxl_fp8\.safetensors · T5-XXL/).length,
    ).toBeGreaterThan(0);
    expect(
      screen.getAllByText(/CLIP-L — one this model needs/).length,
    ).toBeGreaterThan(0);
    expect(
      screen.getAllByText("somebody_elses.safetensors").length,
    ).toBeGreaterThan(0);
    /*
      A VAE has no width to read, so the only honest thing to show is what it said about itself —
      and when exactly one on the shelf names the family the model was measured as, saying which
      one costs nothing. One candidate is not a choice, it is an answer.
    */
    expect(
      screen.getByText(/z_image_ae\.safetensors · says it is Flux\.1-AE/),
    ).toBeTruthy();
  });

  it("still helps with a model whose family it could not read", async () => {
    // Z-Image, measured: a diffusion model, and no family. This used to produce a panel headed
    // THIS MODEL ARRIVES IN PARTS above four empty dropdowns with no hint of what went in them.
    //
    // How many encoders it takes is genuinely unknown, so no number is shown. That it needs at
    // least one encoder and a VAE is not a guess — it is what a bare diffusion model *is*.
    view.value = panel({
      models: [
        {
          file: "z_image_turbo.safetensors",
          family: "unknown",
          saidBase: null,
          bytes: 1,
          kind: "diffusion",
          needs: [],
          with: [],
      makes: "picture" as const,
      carriesEncoder: true,
        },
      ],
      loras: [],
      encoders: [{ file: "qwen_3_4b.safetensors", says: "a language model" , medium: null }],
      vaes: [{ file: "z_image_ae.safetensors", says: "Flux.1-AE" , medium: null }],
      clipTypes: ["stable_diffusion"],
      assembly: {
        family: "unknown",
        encoders: null,
        says: "Epoch could not read which family this model is, so it cannot say how many text encoders it takes. Every model that arrives in parts needs at least one encoder and a VAE; each row below says what it is, and the page you got the model from says which it wants.",
        clipType: null,
        wants: ["CLIP-L", "T5-XXL"],
      },
    });
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);
    await waitFor(() =>
      expect(screen.getByText("THIS MODEL ARRIVES IN PARTS")).toBeTruthy(),
    );

    expect(screen.getByText(/at least one encoder and a VAE/)).toBeTruthy();
    // No count, so no claim about a second encoder — and no "Epoch measured this model as
    // unknown", which is a sentence about Epoch's list dressed as a fact about the model.
    expect(screen.queryByText(/uses one/)).toBeNull();
    expect(screen.queryByText(/Epoch measured this model as/)).toBeNull();
    // The row still says what it is, which is the part that was measurable all along.
    expect(
      screen.getAllByText("qwen_3_4b.safetensors · a language model").length,
    ).toBeGreaterThan(0);
  });

  it("offers what already drew with this model, and does not apply it", async () => {
    // Epoch reads a checkpoint's family for six families and will never cover an open set. A
    // graph that ran is a fact about this exact model, filed by hash, and it costs nothing
    // because it already happened.
    view.value = panel({
      models: [
        {
          file: "z_image.safetensors",
          family: "unknown",
          saidBase: null,
          bytes: 6_200_000_000,
          kind: "diffusion",
          needs: [],
          with: [],
      makes: "picture" as const,
      carriesEncoder: true,
        },
      ],
      // What the server offers. A recipe naming a file the server no longer lists would leave
      // the field empty, which is the state the panel already shows for a missing part.
      encoders: [{ file: "qwen_3_4b.safetensors", says: "a language model" , medium: null }],
      vaes: [{ file: "z_image_ae.safetensors", says: null , medium: null }],
      clipTypes: ["stable_diffusion", "qwen_image", "flux2"],
      remembered: [
        {
          clip: ["qwen_3_4b.safetensors"],
          clipType: "qwen_image",
          vae: "z_image_ae.safetensors",
          drew: true,
          said: null,
        },
      ],
    });
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);

    await waitFor(() =>
      expect(screen.getByText(/drew here before/)).toBeInTheDocument(),
    );
    expect(screen.getByText(/drew here before/)).toBeInTheDocument();
    // Offered, never applied: the person presses it, and then presses GENERATE themselves.
    await userEvent.click(screen.getByRole("button", { name: "USE THAT" }));

    // **Including the family**, which is the value this exists to correct. That field is derived
    // from the measured suggestion until somebody chooses, so a version that only set the state
    // left `stable_diffusion` on screen for a model whose recipe says `qwen_image` — the encoder
    // and the VAE filled in and the one thing that had been wrong stayed wrong. Found by
    // pressing it in the real window.
    const chosen = (label: string) =>
      (
        screen
          .getByText(label)
          .closest("label")
          ?.querySelector("select") as HTMLSelectElement
      ).value;
    expect(chosen("TEXT ENCODER")).toBe("qwen_3_4b.safetensors");
    expect(chosen("FAMILY")).toBe("qwen_image");
    expect(chosen("VAE")).toBe("z_image_ae.safetensors");
  });

  it("says what was refused, in the server's own words", async () => {
    view.value = panel({
      models: [
        {
          file: "flux1-dev.safetensors",
          family: "flux",
          saidBase: null,
          bytes: 11_900_000_000,
          kind: "diffusion",
          needs: [],
          with: [],
      makes: "picture" as const,
      carriesEncoder: true,
        },
      ],
      remembered: [
        {
          clip: ["clip_l.safetensors"],
          clipType: "flux",
          vae: "flux-vae.safetensors",
          drew: false,
          said: "'flux' not in (list of length 28)",
        },
      ],
    });
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);

    await waitFor(() =>
      expect(screen.getByText(/Refused here before/)).toBeInTheDocument(),
    );
    expect(screen.getByText(/Refused here before/)).toBeInTheDocument();
    expect(
      screen.getByText(/not in \(list of length 28\)/),
    ).toBeInTheDocument();
    // A refusal is worth showing and is not worth filling a form in with.
    expect(
      screen.queryByRole("button", { name: "USE THAT" }),
    ).not.toBeInTheDocument();
  });

  it("says nothing about recipes when nobody has drawn with it", async () => {
    // And nothing when nobody has measured its hash either, which is the same silence for a
    // different reason — both are honestly "Epoch has nothing to tell you here".
    view.value = panel();
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);
    await waitFor(() =>
      expect(screen.getByText("GENERATE")).toBeInTheDocument(),
    );
    expect(screen.queryByText(/drew here before/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Refused here before/)).not.toBeInTheDocument();
  });

  it("offers an upscaler only when the machine has one, and sends it", async () => {
    // The shelf has existed since ADR-0032 and nothing consumed what landed on it. A control
    // that appears whether or not a file exists would be the opposite mistake.
    view.value = panel({
      upscalers: [{ file: "4x-UltraSharp.pth", says: null , medium: null }],
    });
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);
    await waitFor(() =>
      expect(screen.getByText("ADVANCED \u25bc")).toBeInTheDocument(),
    );

    await userEvent.click(screen.getByText("ADVANCED \u25bc"));
    const field = screen
      .getByText("UPSCALE")
      .closest("label")
      ?.querySelector("select") as HTMLSelectElement;
    // Nothing chosen by default: an upscale is where a card runs out, and it is not Epoch's to
    // spend somebody's memory on unasked.
    expect(field.value).toBe("");

    await userEvent.selectOptions(field, "4x-UltraSharp.pth");
    await userEvent.type(
      screen.getByPlaceholderText(/what should be in the picture/i),
      "a lighthouse",
    );
    await userEvent.click(screen.getByText("GENERATE"));

    await waitFor(() => expect(asked.calls.length).toBeGreaterThan(0));
    expect(asked.calls.at(-1)?.upscale).toBe("4x-UltraSharp.pth");
  });

  it("shows no upscale control on a machine with none", async () => {
    view.value = panel();
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);
    await waitFor(() =>
      expect(screen.getByText("ADVANCED \u25bc")).toBeInTheDocument(),
    );
    await userEvent.click(screen.getByText("ADVANCED \u25bc"));
    expect(screen.queryByText("UPSCALE")).not.toBeInTheDocument();
  });

  it("hands back where the picture landed, and closes", async () => {
    // The order this was asked for: open the panel and the studio starts, press GENERATE and the
    // panel goes, the picture is made, the studio stops. Where it landed travels out with the
    // press, to a notice beside the conversation — never into the Chronicle, which is what a
    // model reads.
    const landed: (string | null)[] = [];
    view.value = panel();
    render(
      <StudioPanel
        onChosen={(prompt) => spoken.said.push(prompt)}
        onDrew={(at) => landed.push(at)}
      />,
    );
    await waitFor(() =>
      expect(screen.getByText("GENERATE")).toBeInTheDocument(),
    );

    await userEvent.type(
      screen.getByPlaceholderText(/what should be in the picture/i),
      "a red buoy",
    );
    await userEvent.click(screen.getByText("GENERATE"));

    await waitFor(() => expect(landed).toHaveLength(1));
    expect(landed[0]).toContain("pictures");
    // And the panel is done: the press is what closes it.
    expect(spoken.said.at(-1)).toBe("a red buoy");
  });

  it("waits for a studio that is still starting, rather than saying it is not there", async () => {
    // Opening the panel starts it, and it takes about thirty seconds to answer. A panel that
    // asked once would sit on "not answering" for that whole time and then stay wrong.
    view.value = panel({
      models: [],
      problem: "ComfyUI is not answering yet.",
    });
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);

    const { studioPanel } = await import("../ipc/launcher");
    await waitFor(() => expect(studioPanel).toHaveBeenCalled());
    const asked = (studioPanel as unknown as { mock: { calls: unknown[] } })
      .mock.calls.length;

    // Something to choose from arrives; the asking stops.
    view.value = panel();
    await waitFor(
      () =>
        expect(
          (studioPanel as unknown as { mock: { calls: unknown[] } }).mock.calls
            .length,
        ).toBeGreaterThan(asked),
      { timeout: 8000 },
    );
  }, 12000);

  it("groups the VAEs by what they decode, and hides none of them", async () => {
    /*
      **The complaint that started this**, in the owner's words: video and audio files showing up
      under IMAGE. Measured on his machine — drawing a picture with Flux, the VAE list offered
      `minimax_h3_audio_vae_fp32` second of four, with nothing on the row saying it decodes sound.

      A VAE has no family to compare: all four on that machine read `Base::Unknown`, and the only
      one claiming a family (`z_image_ae`, which says *Flux.1-AE*) claims the wrong one. What is
      readable is the rank of its convolutions — 4 is a picture, 5 adds time, 3 has no space in
      it at all.

      **And the unplaced one has to stay.** `minimax_h3_fl2va` is real, it is his, and Epoch
      cannot read it; a filter that dropped it would punish him for a failure of Epoch's with
      nothing on screen to explain it.
    */
    view.value = panel({
      // A VAE dropdown only exists for a model that arrives in parts — a checkpoint carries its
      // own, and a control over something the file already has would be a control over nothing.
      models: [
        {
          file: "flux1-dev.safetensors",
          family: "flux",
          saidBase: null,
          bytes: 23_800_000_000,
          kind: "diffusion",
          needs: [],
          with: [],
          makes: "picture" as const,
          carriesEncoder: false,
        },
      ],
      encoders: [{ file: "clip_l.safetensors", says: "CLIP-L", medium: null }],
      clipTypes: ["stable_diffusion"],
      vaes: [
        { file: "flux-vae-bf16.safetensors", says: "decodes a picture", medium: "picture" },
        { file: "minimax_h3_audio_vae.safetensors", says: "decodes a sound", medium: "sound" },
        { file: "minimax_h3_video_vae.safetensors", says: "decodes a video", medium: "video" },
        { file: "mystery_ae.safetensors", says: null, medium: null },
      ],
    });
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);
    await waitFor(() =>
      expect(screen.getByText("GENERATE")).toBeInTheDocument(),
    );

    const options = [...document.querySelectorAll("option")].map((it) =>
      (it.textContent || "").trim(),
    );
    for (const file of [
      "flux-vae-bf16.safetensors",
      "minimax_h3_audio_vae.safetensors",
      "minimax_h3_video_vae.safetensors",
      "mystery_ae.safetensors",
    ]) {
      expect(
        options.some((it) => it.includes(file)),
        `${file} must still be offered`,
      ).toBe(true);
    }

    const headings = [...document.querySelectorAll("optgroup")].map(
      (it) => it.label,
    );
    expect(headings).toContain("Decodes a picture");
    expect(headings).toContain("Epoch could not tell what these decode");
    expect(headings).toContain("Decodes something else");
  });

  it("starts the drawing machine when it opens", async () => {
    /*
      **The half that pairs with letting it go**, and it has been removed once already.

      Starting on open was taken out on 2026-08-28 to keep a form from summoning a process
      nobody asked for, and what it actually removed was the panel's ability to be filled in: a
      model that arrives in parts needs a family, that list is a node's own vocabulary, and no
      shelf can stand in for it — so GENERATE could not be pressed until GENERATE had been
      pressed. The owner reversed it the next day.

      Asserted on the *call*, not on what came back: whether it starts, was already up, or
      refuses because this World draws on somebody else's machine is `wake_studio`'s decision,
      and a second place deciding is how two places come to disagree.
    */
    view.value = panel();
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);
    const { wakeTheStudio } = await import("../ipc/launcher");
    await waitFor(() => expect(wakeTheStudio).toHaveBeenCalled());
  });

  it("lets the studio go when it closes", async () => {
    // Only what Epoch started, and only when nothing is queued — both decided by the Engine,
    // which is the half that knows. Idle it holds 2.6 GB that asking it to free returns none of.
    view.value = panel();
    const { unmount } = render(
      <StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />,
    );
    await waitFor(() =>
      expect(screen.getByText("GENERATE")).toBeInTheDocument(),
    );

    unmount();
    const { closeStudio } = await import("../ipc/launcher");
    expect(closeStudio).toHaveBeenCalled();
  });

  it("has a way out", async () => {
    // GENERATE used to close it. Then it started staying open to say where the picture landed,
    // which left the panel with no way to be closed at all — a surface that can only be opened
    // is not finished.
    view.value = panel();
    render(<StudioPanel onChosen={(prompt) => spoken.said.push(prompt)} />);
    await waitFor(() =>
      expect(screen.getByText("GENERATE")).toBeInTheDocument(),
    );

    await userEvent.click(screen.getByText("CLOSE"));
    expect(spoken.said.at(-1)).toBe("");
  });
  it("names the embeddings this server has, and writes one into the prompt", async () => {
    // **The last shelf that fed nothing, and it never feeds a node.** Measured: nothing in
    // `/object_info` takes an embedding, ComfyUI gives them their own `/embeddings` endpoint,
    // and the only way to use one is to write `embedding:<name>` into the prompt.
    //
    // Which is why the list matters more than it looks. Measured on a real server with the same
    // seed, a name that is *not* installed is not refused — the words are encoded as ordinary
    // text and quietly change the picture. Typing one is guessing.
    view.value = panel({ embeddings: ["epoch-probe", "another"] });
    render(<StudioPanel onChosen={() => {}} />);

    const box = await screen.findByPlaceholderText(/what should be in the picture/i);
    await userEvent.type(box, "a lighthouse");
    await userEvent.click(
      await screen.findByRole("button", { name: "epoch-probe" }),
    );
    expect(box).toHaveValue("a lighthouse embedding:epoch-probe");

    // A second one is appended, not swapped: a prompt may name several.
    await userEvent.click(screen.getByRole("button", { name: "another" }));
    expect(box).toHaveValue(
      "a lighthouse embedding:epoch-probe embedding:another",
    );
  });

  it("makes a picture until somebody presses VIDEO, and then makes a video", async () => {
    // A video checkpoint carries its model and its VAE and not its encoder — measured on
    // `ltxv-2b-0.9.6-distilled`, which answers `CLIP = None`. So the panel asks for one.
    view.value = panel({
      motions: ["LTXV"],
      encoders: [{ file: "t5xxl_fp8_e4m3fn.safetensors", says: "T5-XXL" , medium: null }],
      clipTypes: ["stable_diffusion", "ltxv"],
      // **The two tabs list different models**, because Epoch read each family out of its own
      // tensors: `patchify_proj` beside `adaln_single` is LTXV, and nothing that draws stills
      // has one. The list used to be alphabetical and undivided, so the first thing offered for
      // a picture was a video checkpoint — which answers `clip input is invalid: None` minutes
      // after the form was filled in correctly.
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
        {
          file: "ltxv-2b-0.9.6-distilled.safetensors",
          family: "ltxv",
          saidBase: null,
          bytes: 6_300_000_000,
          kind: "checkpoint",
          needs: [],
          with: [],
          makes: "video" as const,
          carriesEncoder: false,
        },
      ],
    });
    render(<StudioPanel onChosen={() => {}} />);

    const box = await screen.findByPlaceholderText(/what should be in the picture/i);
    await userEvent.type(box, "a red car on a coast road");

    // The seconds and the rate are not on screen until VIDEO is pressed: controls for a medium
    // nobody chose are controls that do nothing. Nor is the encoder, which a picture checkpoint
    // does not need.
    expect(screen.queryByText("SECONDS")).toBeNull();
    expect(screen.queryByText(/NEEDS A TEXT ENCODER/)).toBeNull();

    // The IMAGE tab offers the model that draws stills, and not the one that cannot.
    const models = () =>
      [...screen.getAllByRole("combobox")[0]!.querySelectorAll("option")].map(
        (o) => o.value,
      );
    expect(models()).toEqual(["sd_xl_base_1.0.safetensors"]);

    await userEvent.click(screen.getByRole("button", { name: "VIDEO" }));
    // And the VIDEO tab offers the other one — and only it.
    expect(models()).toEqual(["ltxv-2b-0.9.6-distilled.safetensors"]);
    await screen.findByText("SECONDS");
    await screen.findByText(/NEEDS A TEXT ENCODER/);

    // **And GENERATE waits for them**, rather than sending a graph ComfyUI will refuse — which
    // is what happened before this gate existed, minutes after the form was filled in.
    expect(screen.getByRole("button", { name: "GENERATE" })).toBeDisabled();
    const fields = screen.getAllByRole("combobox");
    await userEvent.selectOptions(
      fields.find((f) => [...f.querySelectorAll("option")].some((o) => o.value === "t5xxl_fp8_e4m3fn.safetensors"))!,
      "t5xxl_fp8_e4m3fn.safetensors",
    );
    await userEvent.selectOptions(
      fields.find((f) => [...f.querySelectorAll("option")].some((o) => o.value === "ltxv"))!,
      "ltxv",
    );

    await userEvent.click(screen.getByRole("button", { name: "GENERATE" }));
    const ask = asked.calls.at(-1);
    // The family travels by name — never a flag, because the Engine must not pick one.
    expect(ask?.motion).toBe("LTXV");
    // The encoder travels beside the checkpoint: the Engine builds the third loading shape from
    // *what was picked*, never from what it decided the file is missing (ADR-0033).
    expect(ask?.clip).toEqual(["t5xxl_fp8_e4m3fn.safetensors"]);
    expect(ask?.clipType).toBe("ltxv");
    // And the frames are the seconds somebody chose at the rate they chose. Two facts on
    // screen, one number on the wire: the default is 2 s at 25 fps.
    expect(ask?.fps).toBe(25);
    expect(ask?.frames).toBe(50);
  });

  it("a model nobody could measure is offered under both, never hidden", async () => {
    // **Unknown is a third answer, not a `false`.** Hiding somebody's file because Epoch failed
    // to read it is the worst outcome available: nothing they can do about it, and nothing
    // saying why. It is offered under both tabs and may simply not work, which is what
    // `USE IT ANYWAY` already says everywhere else on this panel.
    view.value = panel({
      motions: ["LTXV"],
      models: [
        {
          file: "something-new.safetensors",
          family: "unknown",
          saidBase: null,
          bytes: 1_000,
          kind: "checkpoint",
          needs: [],
          with: [],
          makes: null,
          carriesEncoder: false,
        },
      ],
    });
    render(<StudioPanel onChosen={() => {}} />);
    await screen.findByPlaceholderText(/what should be in the picture/i);
    const models = () =>
      [...screen.getAllByRole("combobox")[0]!.querySelectorAll("option")].map(
        (o) => o.value,
      );
    expect(models()).toEqual(["something-new.safetensors"]);
    await userEvent.click(screen.getByRole("button", { name: "VIDEO" }));
    expect(models()).toEqual(["something-new.safetensors"]);
  });

  it("says the machine cannot make one rather than hiding that it could", async () => {
    // A dark control that explains itself, not a missing one: a VIDEO tab that vanishes teaches
    // that Epoch does not do video, which is false. The Launcher's rule, on a tab.
    view.value = panel({ motions: [], sounds: [] });
    render(<StudioPanel onChosen={() => {}} />);
    await screen.findByPlaceholderText(/what should be in the picture/i);
    expect(screen.getByRole("button", { name: "VIDEO" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "AUDIO" })).toBeDisabled();
    expect(screen.getByText(/no video and no audio nodes/i)).toBeTruthy();
  });

  it("LYRICS is offered by a family that sings, and by no other", async () => {
    // **A property of the encoder node, not of the medium.** ACE-Step is told `tags` and
    // `lyrics` as two inputs; Stable Audio is told a description only, so a LYRICS box on its
    // tab would be a control that reaches nothing.
    const song = {
      file: "ace_step_1.5_turbo_aio.safetensors",
      family: "acestep15",
      saidBase: null,
      bytes: 2,
      kind: "checkpoint",
      needs: [],
      with: [],
      makes: "sound" as const,
      carriesEncoder: true,
    };
    const plain = {
      ...song,
      file: "stable-audio-open-1.0.safetensors",
      family: "stableaudio",
      saidBase: null,
      carriesEncoder: true,
    };
    view.value = panel({
      sounds: ["Stable Audio", "ACE-Step 1.5"],
      lyrical: ["ACE-Step 1.5"],
      models: [song, plain],
    });
    render(<StudioPanel onChosen={() => {}} />);
    const box = await screen.findByPlaceholderText(/what should be in the picture/i);
    await userEvent.click(screen.getByRole("button", { name: "AUDIO" }));

    // It opens on the first family, which does not sing.
    expect(screen.queryByText("LYRICS")).toBeNull();

    const family = screen
      .getAllByRole("combobox")
      .find((f) =>
        [...f.querySelectorAll("option")].some(
          (o) => o.value === "ACE-Step 1.5",
        ),
      )!;
    await userEvent.selectOptions(family, "ACE-Step 1.5");
    await screen.findByText("LYRICS");

    await userEvent.type(box, "reggaeton, spanish");
    await userEvent.type(
      screen.getByPlaceholderText(/what is sung/i),
      "yo, me la paso pensando",
    );
    await userEvent.click(screen.getByRole("button", { name: "GENERATE" }));
    // Two fields, and they stay two.
    expect(asked.calls.at(-1)?.lyrics).toBe("yo, me la paso pensando");
    expect(asked.calls.at(-1)?.prompt).toBe("reggaeton, spanish");

    // And back on a family that does not sing, nothing is carried across.
    await userEvent.selectOptions(family, "Stable Audio");
    expect(screen.queryByText("LYRICS")).toBeNull();
  });

  it("AUDIO offers the model that makes sound, and asks for a duration", async () => {
    // Stable Audio's latent takes `seconds` and no pixels at all, so the tab's controls are its
    // own rather than the video tab's with a relabelled field: two seconds is a video and ten is
    // a sound effect, and sharing one number would make choosing a tab change the other answer.
    view.value = panel({
      motions: ["LTXV"],
      sounds: ["Stable Audio"],
      encoders: [{ file: "t5_base.safetensors", says: "T5-Base" , medium: null }],
      clipTypes: ["stable_audio"],
      models: [
        {
          file: "sd_xl_base_1.0.safetensors",
          family: "sdxl",
          saidBase: null,
          bytes: 1,
          kind: "checkpoint",
          needs: [],
          with: [],
          makes: "picture" as const,
          carriesEncoder: true,
        },
        {
          file: "stable-audio-open-1.0.safetensors",
          family: "stableaudio",
          saidBase: null,
          bytes: 2,
          kind: "checkpoint",
          needs: [],
          with: [],
          makes: "sound" as const,
          carriesEncoder: false,
        },
      ],
    });
    render(<StudioPanel onChosen={() => {}} />);
    const box = await screen.findByPlaceholderText(/what should be in the picture/i);
    await userEvent.type(box, "button sound, SNES style");

    const models = () =>
      [...screen.getAllByRole("combobox")[0]!.querySelectorAll("option")].map(
        (o) => o.value,
      );
    expect(models()).toEqual(["sd_xl_base_1.0.safetensors"]);

    await userEvent.click(screen.getByRole("button", { name: "AUDIO" }));
    expect(models()).toEqual(["stable-audio-open-1.0.safetensors"]);
    // A duration, and no frame rate: sound has no frames.
    await screen.findByText("SECONDS");
    expect(screen.queryByText("FPS")).toBeNull();

    const fields = screen.getAllByRole("combobox");
    await userEvent.selectOptions(
      fields.find((f) =>
        [...f.querySelectorAll("option")].some(
          (o) => o.value === "t5_base.safetensors",
        ),
      )!,
      "t5_base.safetensors",
    );
    await userEvent.selectOptions(
      fields.find((f) =>
        [...f.querySelectorAll("option")].some(
          (o) => o.value === "stable_audio",
        ),
      )!,
      "stable_audio",
    );
    await userEvent.click(screen.getByRole("button", { name: "GENERATE" }));

    const ask = asked.calls.at(-1);
    expect(ask?.motion).toBe("Stable Audio");
    expect(ask?.seconds).toBe(10);
    expect(ask?.clipType).toBe("stable_audio");
  });

  it("two families in one medium each offer only their own model", async () => {
    // **The medium filter was not enough the moment a second sound family arrived.** AUDIO
    // offered both families and both checkpoints in any combination, so choosing ACE-Step with
    // Stable Audio's file composed a graph whose encoder node that checkpoint has never heard
    // of — measured in the window, and nothing said why.
    view.value = panel({
      motions: [],
      sounds: ["Stable Audio", "ACE-Step"],
      models: [
        {
          file: "stable-audio-open-1.0.safetensors",
          family: "stableaudio",
          saidBase: null,
          bytes: 1,
          kind: "checkpoint",
          needs: [],
          with: [],
          makes: "sound" as const,
          carriesEncoder: false,
        },
        {
          file: "ace_step_v1_3.5b.safetensors",
          family: "acestep",
          saidBase: null,
          bytes: 2,
          kind: "checkpoint",
          needs: [],
          with: [],
          makes: "sound" as const,
          carriesEncoder: false,
        },
        {
          file: "something-new.safetensors",
          family: "unknown",
          saidBase: null,
          bytes: 3,
          kind: "checkpoint",
          needs: [],
          with: [],
          makes: null,
          carriesEncoder: false,
        },
      ],
    });
    render(<StudioPanel onChosen={() => {}} />);
    await screen.findByPlaceholderText(/what should be in the picture/i);
    await userEvent.click(screen.getByRole("button", { name: "AUDIO" }));

    const family = screen.getAllByRole("combobox")[0]!;
    const models = () =>
      [...screen.getAllByRole("combobox")[1]!.querySelectorAll("option")].map(
        (o) => o.value,
      );
    // The first family is chosen when the tab opens, so its model is what is offered — plus the
    // one nobody could read, which never disappears.
    expect(models()).toEqual([
      "stable-audio-open-1.0.safetensors",
      "something-new.safetensors",
    ]);

    await userEvent.selectOptions(family, "ACE-Step");
    expect(models()).toEqual([
      "ace_step_v1_3.5b.safetensors",
      "something-new.safetensors",
    ]);
  });

  it("3D is dark on a machine with no 3D nodes, and lit on one that has them", async () => {
    // It was dark unconditionally, waiting on something that could display a mesh. Epoch turns
    // one into a picture now (`state/turntable.rs`), so what is left to wait for is the same
    // thing every other tab waits for: a server that can run the nodes.
    view.value = panel({ motions: ["LTXV"], meshes: [] });
    const dark = render(<StudioPanel onChosen={() => {}} />);
    await dark.findByPlaceholderText(/what should be in the picture/i);
    const off = dark.getByRole("button", { name: "3D" });
    expect(off).toBeDisabled();
    expect(off.getAttribute("title")).toMatch(/no nodes for a model/i);
    dark.unmount();

    // **And a dark tab says which absence it is.** With nothing serving, Epoch reads the
    // families off the shelves instead, so *this ComfyUI has no nodes* is a reading of a machine
    // nobody asked. The AUDIO tab said exactly that on a machine holding two audio checkpoints.
    view.value = panel({ motions: [], meshes: [], serving: false });
    const cold = render(<StudioPanel onChosen={() => {}} />);
    await cold.findByPlaceholderText(/what should be in the picture/i);
    const unasked = cold.getByRole("button", { name: "3D" });
    expect(unasked.getAttribute("title")).toMatch(/nothing on the shelves/i);
    expect(unasked.getAttribute("title")).not.toMatch(/ComfyUI/i);
    cold.unmount();

    view.value = panel({ motions: [], meshes: ["Hunyuan3D"] });
    render(<StudioPanel onChosen={() => {}} />);
    await screen.findByPlaceholderText(/what should be in the picture/i);
    const lit = screen.getByRole("button", { name: "3D" });
    expect(lit).not.toBeDisabled();
    expect(lit.getAttribute("title")).toMatch(/Hunyuan3D/);
  });

  it("a picture is still a picture: nothing about motion travels", async () => {
    view.value = panel({ motions: ["LTXV"] });
    render(<StudioPanel onChosen={() => {}} />);
    const box = await screen.findByPlaceholderText(/what should be in the picture/i);
    await userEvent.type(box, "a lighthouse");
    await userEvent.click(screen.getByRole("button", { name: "GENERATE" }));
    expect(asked.calls.at(-1)?.motion).toBe("");
  });
  it("says nothing about embeddings when the server has none", async () => {
    // An empty shelf is a fact about this machine, and a control offering nothing to press is
    // an instrument with no reading behind it.
    view.value = panel({ embeddings: [] });
    render(<StudioPanel onChosen={() => {}} />);
    await screen.findByPlaceholderText(/what should be in the picture/i);
    expect(screen.queryByText(/EMBEDDINGS/)).toBeNull();
  });
});
