import { useEffect, useState } from "react";

import {
  closeStudio,
  drawFromPanel,
  wakeTheStudio,
  handReferenceOver,
  studioPanel,
  type PanelLoraRow,
  type PanelSteerRow,
  type PanelView,
} from "../ipc/launcher";

import { ImageDrop } from "./ImageDrop";

/**
 * What joins the chosen encoders into one dependency for `useEffect`.
 *
 * A character nobody types and no filename contains, so two files can never be joined into
 * a string that looks like a third.
 *
 * Written as an escape rather than as the character itself. It was a literal NUL in the
 * source — which works, and makes the whole file read as binary to every tool that looks at
 * it, including `grep`, which then declines to show a match inside it.
 */
/**
 * What each medium is called in a sentence, so a heading reads like English.
 *
 * One place, because the same word appears on a group heading, on a row and in a refusal, and
 * three spellings of one fact agree by luck until one of them is edited.
 */
const WHAT_IT_MAKES: Record<string, string> = {
  picture: "a picture",
  video: "a video",
  sound: "a sound",
  model: "a model",
};

const SEPARATOR = "\u0000";

/**
 * The Studio Panel (ADR-0033): Epoch opens it, the person fills it in.
 *
 * ## Why a form and not a sentence
 *
 * A picture is a dozen decisions — which model, which LoRAs and how strongly, the shape, the
 * steps, the seed. A character guessing all twelve from one line will guess some of them wrong,
 * and the only correction available was another sentence. Here the guessing stops.
 *
 * It is a **form, not a node graph**. Nothing exposes an edge, a socket or a wire, so the
 * prohibition in `CLAUDE.md` is untouched — and the graph itself is composed by the Engine
 * against the schema of whichever machine will actually draw.
 *
 * ## Incompatible is greyed and explained, never hidden
 *
 * A LoRA for another family keeps its row, loses its light and says why in the words a person
 * uses. `USE IT ANYWAY` is always there: Epoch measured, the file is theirs, and the answer is
 * theirs. `unknown` is not an incompatibility — it is the absence of a measurement.
 *
 * ## Advanced stays folded
 *
 * ADR-0026's rule: the default is that the user touches nothing.
 */
export function StudioPanel({
  onChosen,
  onDrew,
}: {
  onChosen: (prompt: string) => void;
  /**
   * A picture was made, and where it landed when the World has somewhere to put it.
   *
   * Separate from `onChosen` because it carries something the Chronicle must not: a path in the
   * conversation is what a model would need to claim a picture nobody made. The surface shows it
   * beside the conversation instead.
   */
  onDrew?: (at: string | null) => void;
}) {
  const [view, setView] = useState<PanelView | null>(null);
  const [checkpoint, setCheckpoint] = useState("");
  const [chosen, setChosen] = useState<Record<string, number>>({});
  const [prompt, setPrompt] = useState("");
  const [negative, setNegative] = useState("");
  const [shape, setShape] = useState(1);
  /**
   * A size the presets do not offer.
   *
   * `-1` is the Custom row. Held separately from `shape` so switching to a preset and back does
   * not lose what somebody typed.
   */
  const [customW, setCustomW] = useState(1024);
  const [customH, setCustomH] = useState(1024);
  const [steps, setSteps] = useState(20);
  const [cfg, setCfg] = useState(7);
  const [seed, setSeed] = useState(0);
  const [upscale, setUpscale] = useState("");
  /**
   * Which ControlNet steers, the picture it steers by, and how hard.
   *
   * Three pieces of user intent and nothing derived. The reference is held as **the name
   * ComfyUI answered with**, never as anything about this machine: a lent studio has its own
   * disk, which is why handing a picture over returns a name in the first place.
   */
  /**
   * Each ControlNet steering the picture, in the order they apply.
   *
   * A list because they chain — each apply node takes a pair of conditionings and answers a
   * pair — so a pose from one picture and a depth from another stack rather than compete. The
   * reference on each row is **the name ComfyUI answered with**, never anything about this
   * machine: a lent studio has its own disk.
   *
   * No ceiling. ComfyUI has none and the card does, and a limit typed in here would be a number
   * nobody measured.
   */
  const [steers, setSteers] = useState<PanelSteerRow[]>([]);

  /**
   * A picture this one is drawn on top of, by the name the server answered with.
   *
   * **Every picture Epoch made until now began as noise.** There was no way to say *this photo,
   * in another style* — the first thing somebody asks the moment they have a photo. A ControlNet
   * is not the same thing: it carries outlines and reinvents the rest, which is why a face comes
   * back as somebody else's.
   */
  const [from, setFrom] = useState("");
  /**
   * How much of that picture survives, as a fraction.
   *
   * Said this way round because it is the question somebody actually has. ComfyUI counts the
   * other way (`denoise`), and the Engine does that arithmetic once, in one place.
   */
  const [keep, setKeep] = useState(0.55);
  /**
   * The base picture as the browser already had it, and the size it really is.
   *
   * **Two things nobody could see.** A hand-over that failed left the row saying *choose a
   * picture* and nothing else, and the only sign that a picture was not in play was the size of
   * the result — measured, 1024×1024 where the photo is 1280×960. A thumbnail is the one
   * confirmation that cannot be misread.
   *
   * And the size is read here rather than asked of anybody: the file is in this window before it
   * is handed over, `img-src` already allows `data:`, and what the picture measures is what the
   * render will be.
   */
  const [fromShot, setFromShot] = useState<{
    readonly uri: string;
    /** `null` until the browser has decoded it, and forever if it never can. */
    readonly size: { readonly width: number; readonly height: number } | null;
  } | null>(null);
  /**
   * Which medium this panel is making, by the name of the video family — or empty for a picture.
   *
   * **The tab and the family are one piece of state, not two.** A `medium` beside a `family`
   * would be two things that must agree, and the one nobody watches is how a VIDEO tab ends up
   * sending an empty family. Empty *is* the IMAGE tab.
   */
  const [motion, setMotion] = useState("");
  /**
   * How long, in seconds, and how fast.
   *
   * Seconds because that is what a person means. Frames are what the graph takes, and the two
   * are the same fact — so one is stored and the other derived, rather than both stored and
   * left to disagree.
   */
  const [seconds, setSeconds] = useState(2);
  const [fps, setFps] = useState(25);
  /**
   * How long a sound should be.
   *
   * Its own number, not the video's `seconds`. They are both a duration and they are not the
   * same decision: two seconds is a video, ten is a sound effect, and sharing one field would
   * make choosing a tab silently change the other tab's answer.
   */
  const [sound, setSound] = useState(10);
  /** The sides a mesh model can be shown, in the order the node takes them. */
  const SIDES = ["front", "left", "back", "right"] as const;
  /**
   * For a model: which picture it is built from, and which checkpoint draws one when nobody
   * hands one over.
   *
   * **Both ways, because both are real** — the owner asked for both. Hunyuan3D is conditioned on
   * a *picture*, measured: there is no text encoder in its graph at all. So either a file the
   * person already has, or one drawn in the same press by a checkpoint they also picked. Epoch
   * chooses neither.
   */
  /**
   * One entry per side — front, left, back, right — in the order the node takes them.
   *
   * **Four named views, not a list of pictures.** Measured: the multi-view node has no required
   * inputs and four optional ones, each a side. A bag would have to guess which is which.
   */
  const [shapeFrom, setShapeFrom] = useState<string[]>(["", "", "", ""]);
  const [shapePrompts, setShapePrompts] = useState<string[]>(["", "", "", ""]);
  const [shapeWith, setShapeWith] = useState("");
  /**
   * How many sides the person wants to give.
   *
   * **Their choice, because Epoch cannot measure it.** `hunyuan3d-dit-v2_fp16` and
   * `hunyuan3d-dit-v2-mv_fp16` are structurally identical — 1645 tensors, the same prefixes,
   * the same `geo_decoder` — so nothing in the bytes says which is the multi-view one. Only the
   * filename does, and reading filenames is what ADR-0024 forbids.
   */
  const [views, setViews] = useState(1);
  /**
   * How the surface is read out of the voxels: `smooth`, `fine` or `blocky`.
   *
   * **Not a defect to be defaulted away from.** Smooth is what *cow* means, and blocky is what
   * *a voxel model* means — measured, they are the same shape read two ways: 1.65M faces with a
   * face, ears and horns, against 407k and a stepped silhouette.
   *
   * And `fine` is the same algorithm on a grid twice as fine. It was nearly not shipped, on the
   * strength of comparing the *animal* in two turntables and finding the same face, the same
   * ears and the same horns. The owner looked at the **ground**: at 256 the disc under the cow
   * has concentric steps, at 512 it is a clean ellipse. A voxel grid shows itself on flat
   * surfaces and thin detail, which is exactly where nobody was looking.
   *
   * So it is offered **with the minute on it**, because six against one is the whole decision
   * and it is not Epoch's to make.
   */
  const [surface, setSurface] = useState("smooth");
  /**
   * What is **sung**, which is not what describes the song.
   *
   * Its own box because ACE-Step's encoder takes `tags` and `lyrics` as two inputs — measured,
   * and not a rename. The composer passed an empty string here from the day sound worked, with
   * a comment saying the panel had nowhere to type them. This is that nowhere.
   */
  const [lyrics, setLyrics] = useState("");
  const handed = shapeFrom.some((it) => it !== "");
  const [open, setOpen] = useState(false);
  const [drawing, setDrawing] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  const [guidance, setGuidance] = useState(3.5);
  /**
   * The parts a model that arrives in parts needs.
   *
   * Chosen, never inferred. Flux, Z-Image, Qwen-Image and SD 3 differ in one string — the
   * encoder family — and guessing it from a filename is exactly the guessing this panel exists
   * to stop.
   */
  const [clip, setClip] = useState<string[]>([]);
  const [clipType, setClipType] = useState("");
  const [vae, setVae] = useState("");
  /**
   * Whether the user has chosen the encoder family themselves.
   *
   * **Because a fact is not a taste.** 11.22 marked the measured family and refused to select
   * it, on ADR-0033's rule that the panel is the user's. That rule is about *files* — which
   * checkpoint, which LoRA, which encoder — and the encoder family is not one: it is a property
   * of the model Epoch already read out of its tensors. Leaving it blank made somebody choose
   * between twenty-eight names for a question that has one right answer, which is guessing put
   * on the user rather than removed from the product.
   *
   * So a measured family arrives chosen, and changing it clears the flag forever. Every other
   * family stays exactly as selectable as before, which is what keeps this a default rather than
   * a decision.
   */
  const [touched, setTouched] = useState(false);

  /**
   * The families **this** loader accepts, which is decided by how many encoders were chosen.
   *
   * One encoder is a `CLIPLoader` (28 families, `stable_diffusion` among them), two is a
   * `DualCLIPLoader` (12, and `stable_diffusion` is not one of them), three is a
   * `TripleCLIPLoader` — which takes no family at all. Measured by asking the server; the lists
   * barely overlap, and using the wrong one is a picture the server refuses after the panel was
   * filled in perfectly.
   */
  const offered = (() => {
    if (!view) return [];
    /*
      **The count the family needs, before the count the user has reached.**

      Measured 2026-08-25 on this machine: `CLIPLoader` publishes no `flux` at all — it has
      `flux2` and nothing else close — while `DualCLIPLoader` publishes `flux`. So a Flux model
      opened the panel showing twenty-eight families, none of which was the right one, with
      `flux2` sitting in the middle of them as an active trap. The list only became answerable
      after the second encoder was chosen, which is the step the list was supposed to help with.

      Epoch measured how many encoders this family takes. Using that is the same measurement the
      guidance sentence above already uses; using the number of files picked so far was using a
      count of how far somebody had got.
    */
    const takes = view.assembly?.encoders ?? clip.length;
    const count = Math.max(takes, clip.length);
    if (count >= 3) return [];
    if (count === 2) return view.clipTypesTwo;
    return view.clipTypes;
  })();

  /*
    Re-asked whenever the model **or the chosen encoders** change, because compatibility is
    decided in the Engine — one rule in one place beats the same rule written twice and drifting.

    The encoders were added to the question after a Z-Image panel offered twenty-eight families
    and no help: Epoch reads a checkpoint's family for five families and comes back `Unknown` for
    everything newer, while it reads an *encoder* from the width of its embedding table, always.
    And the encoder is what ComfyUI keys on — measured, a Z-Image drew as `stable_diffusion` and
    as `qwen_image`, and failed as `flux2`.
  */
  /**
   * Somebody asked for the drawing machine because the panel needed it.
   *
   * Held here rather than read from the view, because *starting* and *started* are different
   * facts and only the second is measurable: `serving` says whether it answers now, and this
   * says whether anybody is waiting for it to. Without it the panel cannot tell a machine that
   * was never asked from one that is thirty seconds into starting.
   */
  const [waking, setWaking] = useState(false);

  const clipKey = clip.join(SEPARATOR);
  useEffect(() => {
    let alive = true;
    let waiting: number | undefined;

    const ask = () => {
      void studioPanel(
        checkpoint,
        clipKey === "" ? [] : clipKey.split(SEPARATOR),
      ).then((next) => {
        if (!alive) return;
        setView(next);
        if (next && checkpoint === "" && next.models.length > 0) {
          setCheckpoint(next.models[0]!.file);
        }
        // **Opening the panel starts the studio, so the panel waits for it.**
        //
        // Measured: about thirty seconds from START to answering. A panel that asked once would
        // sit on *ComfyUI is not answering* for that whole time and then stay wrong — so it asks
        // again until there is something to choose from, which is also what makes the wait
        // somebody spends rather than watches.
        // **Two reasons to re-ask, and they are different questions.** `waking && !serving` is
        // the studio the panel asked for on open, still on its way — thirty seconds, measured.
        // `models.length === 0` is a machine with nothing to load at all, which is somebody
        // installing one in another window and has no end anybody can predict.
        //
        // The shelves answer the first read either way, so nothing here is a blank screen: what
        // arrives late is the half only a running node can answer.
        if (next && (next.models.length === 0 || (waking && !next.serving))) {
          waiting = window.setTimeout(ask, 3000);
        }
      });
    };
    ask();

    return () => {
      alive = false;
      window.clearTimeout(waiting);
    };
  }, [checkpoint, clipKey, waking]);

  /*
    **Opening the panel starts the drawing machine.**

    `wake_studio` refuses on its own terms — already serving, not installed, or a World drawing
    on somebody else's machine (ADR-0029) — so this asks once and reads the answer rather than
    deciding any of that here. Two places deciding is how they come to disagree.

    `false` means it could not be started, and the panel says so instead of waiting forever for
    something nobody is starting: `waking` is what the header, the button and the re-ask loop
    all read, and it has to end up false when the answer is no.
  */
  useEffect(() => {
    let alive = true;
    setWaking(true);
    void wakeTheStudio().then((up) => {
      if (alive && !up) setWaking(false);
    });
    return () => {
      alive = false;
    };
  }, []);

  // **And closing it lets the studio go.**
  //
  // Only what Epoch started, and only when nothing is queued — both decided by the Engine, which
  // is the half that knows. Idle, ComfyUI holds 2.6 GB of system RAM that asking it to free
  // memory does not return; the only thing that returns it is stopping it.
  useEffect(() => {
    return () => {
      void closeStudio();
    };
  }, []);

  if (view === null) {
    return (
      <div className="rm">
        <span className="rm__label">The panel</span>
        <p className="cc__hint">Asking the machine that would draw…</p>
      </div>
    );
  }

  /**
   * The models that can make what this tab makes.
   *
   * **Three answers, and the third is why this is not a boolean filter.** A family Epoch read
   * off the model's own tensors belongs to one medium and is listed under that tab. A family it
   * could not read belongs to neither, so it is listed under **both** — hiding somebody's file
   * because Epoch failed to measure it is the worst outcome available: nothing they can do, and
   * nothing saying why.
   *
   * It is a filter and not a warning because the failure it prevents is not a bad picture, it is
   * a refusal: an LTXV checkpoint asked for a still answers `clip input is invalid: None`,
   * minutes after the form was filled in correctly. The list was alphabetical, so it was the
   * first thing offered.
   */
  /** Which medium the open tab makes. The family name decides it, never the button. */
  const making: "picture" | "video" | "sound" | "model" =
    motion === ""
      ? "picture"
      : view.sounds.includes(motion)
        ? "sound"
        : view.meshes.includes(motion)
          ? "model"
          : "video";
  /**
   * Why a tab is dark, in the words that are true of *why*.
   *
   * **Two different absences, and they had one sentence.** With ComfyUI running, a dark tab means
   * this server lacks the nodes. With nothing running, Epoch reads the families off the shelves
   * instead — so a dark tab means no model here makes that thing, and saying *this ComfyUI has no
   * audio nodes* is a reading of a machine nobody asked. The cold-instrument rule: no reading
   * beats an invented one.
   */
  const nothingFor = (what: string) =>
    view.serving
      ? `This ComfyUI has no nodes for ${what}, so nothing here can make it.`
      : `Nothing on the shelves makes ${what}. Install a model that does and this lights up.`;
  /**
   * And which *family*, once a tab has more than one.
   *
   * **The medium filter was not enough the moment a second sound family arrived.** AUDIO offered
   * Stable Audio and ACE-Step, and both checkpoints, in any combination — so choosing ACE-Step
   * and leaving Stable Audio's file selected composed a graph whose encoder node the checkpoint
   * has never heard of. Measured in the window: nothing was made and nothing said why.
   *
   * Epoch reads both families out of their own tensors, so this is a comparison rather than a
   * guess — and a model it could not read still passes, for the reason the medium filter has:
   * hiding somebody's file for a failure of Epoch's is the worst outcome available.
   */
  const familyId = motion.toLowerCase().replace(/[^a-z0-9]/g, "");
  const forThisTab = view.models.filter(
    (it) =>
      (it.makes === null || it.makes === making) &&
      (motion === "" || it.family === "unknown" || it.family === familyId),
  );
  /**
   * What can draw the picture a model is built from.
   *
   * The picture half of a model is an ordinary picture graph, so the choice is an ordinary
   * picture checkpoint — and one that arrives in parts is excluded, because that half of the
   * graph loads through `CheckpointLoaderSimple` and has no place to put the pieces.
   */
  const canDraw = view.models.filter(
    (it) => it.kind !== "diffusion" && (it.makes === null || it.makes === "picture"),
  );
  const model =
    forThisTab.find((it) => it.file === checkpoint) ?? forThisTab[0];
  // Two measurements meeting: Epoch read the family out of the model's tensors, and the server
  // published the list this loader accepts. When they agree there is nothing to choose.
  const familyNow =
    touched || !view.assembly?.clipType
      ? clipType
      : offered.includes(view.assembly.clipType)
        ? view.assembly.clipType
        : clipType;
  /*
    **And the VAE is not preselected, because that one is a guess.**

    It was, for a moment: the only VAE whose own metadata named the model's family. Run against
    the files on this machine it chose `z_image_ae.safetensors` for a Flux model, because that
    file says *Flux.1-AE* and `flux-vae-bf16.safetensors` — obviously the right one to a human
    reading the name — says nothing at all. Epoch does not read filenames (ADR-0024) and a
    measurement that only one of two files happens to carry is not a comparison.

    The family above is different in kind: Epoch read it out of the model's own tensors and the
    server published the list. Here there is nothing to read, so nothing is chosen.
  */

  /*
    **How many encoders are still missing, and why that is now enforced.**

    The loader is chosen by how many files were picked — one is a `CLIPLoader`, two is a
    `DualCLIPLoader` — and their family lists barely overlap. Keying the FAMILY list on the count
    the *family* takes fixed a list that could not contain the answer, and opened a worse hole:
    the panel offered `flux` from the dual list while one encoder still built a single loader, and
    ComfyUI answered `'flux' not in (list of length 28)` for every configuration tried.

    So the two now agree by construction. Epoch measured that Flux takes two; saying *pick the
    second one* is information, and a dead GENERATE that explains itself beats a refusal after the
    panel was filled in.
  */
  // The newest combination that actually drew, and the ones the server refused. Split here
  // because they are offered differently: one can be pressed, the other can only be read.
  const drewBefore = view.remembered.find((one) => one.drew) ?? null;
  const refusedBefore = view.remembered.filter((one) => !one.drew);
  const takes = view.assembly?.encoders ?? 0;
  /*
    **Which row to pick, said on the row.**

    The guidance already said *a CLIP-L and a T5-XXL* and every row already said which file was
    which, and the owner still had to hold one sentence and match it against a list by eye —
    once per field, and the second field's own label made it worse by reading as *Flux uses one
    encoder*. Marking is a comparison between two things Epoch measured: what the family wants,
    and what each file is. Nothing is typed and nothing is guessed.

    A want already satisfied by another field drops out, so the second dropdown marks the one
    that is still missing rather than repeating the first.
  */
  const wants = view.assembly?.wants ?? [];
  const saysOf = (file: string) =>
    view.encoders.find((it) => it.file === file)?.says ?? null;
  /** What is still missing, once the *other* fields are accounted for. */
  const stillWanted = (index: number): string[] => {
    const left = [...wants];
    for (const one of clip.filter((_, at) => at !== index).map(saysOf)) {
      const at = one === null ? -1 : left.indexOf(one);
      if (at >= 0) left.splice(at, 1);
    }
    return left;
  };
  // **Any of the remaining ones, not merely the first.** Flux wants a CLIP-L and a T5-XXL and
  // either may go in either field, so marking only the first would quietly say the other one is
  // wrong. The mark is short because the row already names what the file is.
  const markIf = (file: string, index: number) => {
    const says = saysOf(file);
    return says !== null && stillWanted(index).includes(says)
      ? " — one this model needs"
      : "";
  };
  const missingEncoders = Math.max(0, takes - clip.length);

  const preset = view.shapes[shape] ?? view.shapes[1] ?? view.shapes[0];
  // `-1` is Custom. Rounded to eight because latent space works in eights and the sampler
  // silently rounds anything else — better to show the number that will actually be used.
  const size =
    shape === -1
      ? {
          label: "custom",
          width: toEight(customW),
          height: toEight(customH),
          native: false,
        }
      : preset;

  /**
   * **GENERATE draws.**
   *
   * It used to record the settings and hand the prompt to the character, so that the picture
   * came back as their reply and landed on the Quest as evidence. The reasoning was right and
   * the behaviour was not: measured 2026-08-25, `gemma4:12b` handed *"a lighthouse at night"*
   * with the panel already open called `open_studio` again and answered *"El panel está abierto;
   * elige lo que quieras y pulsa Generar."* ComfyUI never received a graph. A button called
   * GENERATE that asks somebody else to generate is a button that does not work.
   *
   * The evidence half is kept and moved rather than dropped: the Engine files the picture on the
   * active Quest in the same `Produced` entry a capability's run makes, so History still sees it
   * (ADR-0025). The character is still told what was asked for, so the conversation reads as one
   * thing that happened rather than a picture appearing out of nowhere.
   */
  const draw = () => {
    if (!model || !size) return;
    setDrawing(true);
    setSaid(null);
    void drawFromPanel({
      checkpoint: model.file,
      kind: model.kind,
      clip,
      clipType: familyNow,
      vae,
      loras: Object.entries(chosen).map(([file, strength]) => [file, strength]),
      upscale,
      controls: steers,
      from,
      keep,
      prompt,
      negative,
      width: size.width,
      height: size.height,
      steps,
      cfg,
      seed,
      batch: 1,
      guidance,
      motion,
      // Frames, from the seconds somebody chose and the rate they chose. The Engine rounds to
      // what the family accepts — LTXV takes `8n+1` — because the Engine is where the family is
      // known, and a number rounded in two places is a number that will be rounded twice.
      frames: Math.round(seconds * fps),
      fps,
      // A sound's own duration. Both travel; the Engine reads whichever the family's medium
      // means, so the surface never has to decide which number was the real one.
      seconds: sound,
      // Only a lyrical family reads it; everything else is handed an empty string it ignores.
      lyrics: view.lyrical.includes(motion) ? lyrics : "",
      // `?` is the moment after somebody chose *pictures I hand over* and before they handed
      // one over. It is not a name, so it never reaches the graph — GENERATE waits.
      shapeFrom: shapeFrom.map((it) => (it === "?" ? "" : it)),
      shapePrompts,
      shapeWith,
      surface,
    }).then((drawn) => {
      setDrawing(false);
      if (typeof drawn === "string") {
        setSaid(drawn);
        return;
      }
      // **It began, so the panel says so and stays out of the way.** Nothing exists yet: the
      // character is waiting on the machine, the World shows it, and the Chronicle gets the
      // video when it lands (ADR-0034). Closing the panel is the same gesture a finished
      // picture gets, and for the same reason — a form nobody is filling in has no business
      // holding the screen while a card works.
      if (drawn.kind === "began") {
        onDrew?.(null);
        onChosen(prompt);
        return;
      }
      // **The panel closes on GENERATE.** That is the order this was asked for: open the
      // panel and the studio starts, press GENERATE and the panel goes, the picture is made, the
      // studio stops. A form nobody is filling in any more has no reason to hold the screen
      // while a card works.
      //
      // Where it landed travels with it, to a notice beside the conversation rather than into
      // the Chronicle — the Chronicle is what a model reads, and a path in it is the one thing a
      // model needs to claim a picture nobody made (ADR-0030's third amendment).
      onDrew?.(drawn.at);
      onChosen(prompt);
    });
  };

  const toggle = (file: string) =>
    setChosen((held) => {
      const next = { ...held };
      if (file in next) delete next[file];
      else next[file] = 0.8;
      return next;
    });

  return (
    <div className="rm">
      <span className="rm__label">The panel</span>
      <p className="cc__hint">
        You choose; nothing here is guessed from a sentence. The graph is
        written against the machine that will draw it — {view.whereAt}.
      </p>

      {/*
        **Opening the panel starts the drawing machine again** — the owner's call, 2026-08-29,
        reversing his own call of the day before.

        The first decision was right about its cost and wrong about what it cost. Starting on
        open was removed because a form should not summon a twenty-gigabyte process and a
        terminal window for somebody who may look and close it again — and the twenty gigabytes
        were measured *after a render*, never at startup: freshly started and holding nothing,
        ComfyUI is 954 MB.

        What the removal actually cost was the panel. A model that arrives in parts needs a
        family, that list is a *node's* vocabulary, and no shelf can stand in for it — so the
        panel opened with a dead GENERATE and a sentence explaining that pressing it once would
        fix it. A form that cannot be completed until you press the button that needs it
        completed is not a form.

        So the flow is one line now: the panel opens, the machine starts behind it, you choose,
        you press GENERATE, the thing lands in the conversation, and closing the panel lets the
        machine go. `wake_studio`'s own comment has said *opening the panel is asking to draw*
        the whole time; only its caller had gone.
      */}
      {!view.serving && (
        <p className="cc__hint cc__hint--read">
          {waking ? (
            <>
              Starting the drawing machine — about half a minute. Everything
              here is read off your shelves meanwhile, and the lists only a
              running machine has fill themselves in when it answers.
            </>
          ) : (
            <>
              The drawing machine is not running and Epoch could not start it
              from here. Everything below is read off your shelves.
              {view.clipTypes.length === 0 &&
                " A model that arrives in parts needs its family, and that list is a node's own vocabulary — no shelf can stand in for it."}
            </>
          )}
        </p>
      )}

      {view.problem && <p className="notice notice--warn">{view.problem}</p>}

      {/*
        A diffusion model cannot draw alone, and saying which three files finish it is worth more
        than refusing. Unfinished is not broken.
      */}
      {model && model.needs.length > 0 && (
        <p className="notice notice--warn">
          {model.file} cannot draw on its own. Still missing:{" "}
          {model.needs.join(", ")}.
        </p>
      )}
      {model && model.with.length > 0 && (
        <p className="cc__hint">Loaded with: {model.with.join(" · ")}</p>
      )}

      {/*
        **The three media, from the first version, with two of them dark** (ADR-0033).

        A missing tab teaches that Epoch does not do video, which was false the day it was
        written and is false now. A dark tab that says what it is waiting for is information —
        the Launcher's rule about a panel with nothing behind it, applied to a control.

        VIDEO lit up in Phase 12. 3D still waits on something that can display a mesh, and the
        World is 2D.
      */}
      <div className="rm__list rm__list--changes">
        <span className="rm__heading">MAKE</span>
        <div className="cc__row">
          <button
            type="button"
            className={`btn btn--mini${making === "picture" ? " btn--on" : ""}`}
            onClick={() => {
              setMotion("");
              // **The chosen model may not belong to the tab being opened.** Cleared rather
              // than carried, so the list picks its own first entry — carrying it is how
              // somebody ends up on the IMAGE tab with a video checkpoint selected and no
              // way to see that from the control.
              setCheckpoint("");
            }}
          >
            IMAGE
          </button>
          <button
            type="button"
            className={`btn btn--mini${making === "video" ? " btn--on" : ""}`}
            disabled={view.motions.length === 0}
            title={
              view.motions.length === 0
                ? nothingFor("something that moves")
                : `Made with ${view.motions.join(" or ")}.`
            }
            onClick={() => {
              setMotion(view.motions[0] ?? "");
              setCheckpoint("");
            }}
          >
            VIDEO
          </button>
          <button
            type="button"
            className={`btn btn--mini${making === "sound" ? " btn--on" : ""}`}
            disabled={view.sounds.length === 0}
            title={
              view.sounds.length === 0
                ? nothingFor("a sound")
                : `Made with ${view.sounds.join(" or ")}.`
            }
            onClick={() => {
              setMotion(view.sounds[0] ?? "");
              setCheckpoint("");
            }}
          >
            AUDIO
          </button>
          <button
            type="button"
            className={`btn btn--mini${making === "model" ? " btn--on" : ""}`}
            disabled={view.meshes.length === 0}
            title={
              view.meshes.length === 0
                ? nothingFor("a model")
                : `Made with ${view.meshes.join(" or ")}. It turns in the conversation; the file is a .glb.`
            }
            onClick={() => {
              setMotion(view.meshes[0] ?? "");
              setCheckpoint("");
            }}
          >
            3D
          </button>
        </div>
        {view.motions.length === 0 && view.sounds.length === 0 && (
          <p className="cc__hint">
            {view.serving
              ? "This drawing machine has no video and no audio nodes. VIDEO and AUDIO light up when it does."
              : "Nothing on the shelves makes a video or a sound. VIDEO and AUDIO light up when a model that does is installed."}
          </p>
        )}
        {making === "sound" && (
          <>
            {view.sounds.length > 1 && (
              <label className="cedit__field cedit__field--wide">
                <span>FAMILY</span>
                <select
                  value={motion}
                  onChange={(e) => setMotion(e.target.value)}
                >
                  {view.sounds.map((it) => (
                    <option key={it} value={it}>
                      {it}
                    </option>
                  ))}
                </select>
              </label>
            )}
            <label className="cedit__field">
              <span>SECONDS</span>
              {/*
                **No ceiling typed here.** The Engine clamps to what the family's own latent node
                declares, and it declares a thousand for both — the limit belongs to ComfyUI
                rather than to the model. What is worth saying is which length the family was
                trained at, and that is a sentence rather than a `max` that would refuse
                something the model would happily do.
              */}
              <input
                type="number"
                min={1}
                step={1}
                value={sound}
                onChange={(e) => setSound(Number(e.target.value) || 10)}
              />
            </label>
            {view.lyrical.includes(motion) && (
              <label className="cedit__field cedit__field--wide">
                <span>LYRICS</span>
                {/*
                  **Only for a family whose encoder takes them.** A music model is told a genre
                  and a vocal separately, which is why this is a second box rather than more of
                  the prompt: splitting one field into two would be Epoch writing words nobody
                  typed. Stable Audio is told a description only, so its tab has no LYRICS at
                  all — a control that reaches nothing is worse than one that is absent.

                  Empty is valid and it means an instrumental.
                */}
                <textarea
                  rows={4}
                  value={lyrics}
                  placeholder="what is sung — leave it empty for an instrumental"
                  onChange={(e) => setLyrics(e.target.value)}
                />
              </label>
            )}
            <p className="cc__hint">
              {sound}s with {motion}
              {view.lyrical.includes(motion) && lyrics.trim() === ""
                ? ", instrumental"
                : ""}
              . It runs on its own while you keep working — the crew card says
              who is waiting, and it arrives in the conversation when it is
              done.
            </p>
          </>
        )}
        {making === "model" && (
          <>
            {view.meshes.length > 1 && (
              <label className="cedit__field cedit__field--wide">
                <span>FAMILY</span>
                <select
                  value={motion}
                  onChange={(e) => setMotion(e.target.value)}
                >
                  {view.meshes.map((it) => (
                    <option key={it} value={it}>
                      {it}
                    </option>
                  ))}
                </select>
              </label>
            )}
            {/*
              **A model is built from a picture, and this is where the person says which.**

              Measured: `Hunyuan3Dv2Conditioning` takes a `CLIP_VISION_OUTPUT` and there is no
              text encoder anywhere in that graph. So text-to-model is really text-to-picture-
              to-model — one press, one graph, and *two* models, both of them chosen here. It is
              not Epoch chaining something behind the person: the second choice is on screen.
            */}
            <label className="cedit__field cedit__field--wide">
              <span>FROM</span>
              <select
                value={handed ? "picture" : "words"}
                onChange={(e) =>
                  setShapeFrom(
                    e.target.value === "words" ? ["", "", "", ""] : ["?", "", "", ""],
                  )
                }
              >
                <option value="words">pictures drawn here, from prompts</option>
                <option value="picture">pictures I hand over</option>
              </select>
            </label>
            <label className="cedit__field cedit__field--wide">
              <span>SURFACE</span>
              <select
                value={surface}
                onChange={(e) => setSurface(e.target.value)}
              >
                <option value="smooth">smooth — about a minute</option>
                <option value="fine">fine — cleaner flat surfaces, about six minutes</option>
                <option value="blocky">blocky — show the voxels</option>
              </select>
            </label>
            <label className="cedit__field cedit__field--wide">
              <span>VIEWS</span>
              <select
                value={views}
                onChange={(e) => {
                  const many = Number(e.target.value);
                  setViews(many);
                  // Sides beyond the count are cleared, so a view nobody can see is never sent.
                  setShapeFrom((was) => was.map((it, at) => (at < many ? it : "")));
                  setShapePrompts((was) => was.map((it, at) => (at < many ? it : "")));
                }}
              >
                <option value={1}>one — the front</option>
                <option value={4}>four — front, left, back, right</option>
              </select>
            </label>
            {views > 1 && (
              <p className="cc__hint cc__hint--read">
                Four views need a <b>multi-view checkpoint</b>. Epoch cannot tell
                yours apart: measured, the single-view and multi-view files have
                the same 1645 tensors under the same names, and only the
                filename differs — which Epoch does not read. If the model is
                the single-view one, the server refuses and says so.
              </p>
            )}
            {!handed && (
              <label className="cedit__field cedit__field--wide">
                <span>DRAWN BY</span>
                <select
                  value={shapeWith}
                  onChange={(e) => setShapeWith(e.target.value)}
                >
                  <option value="">— pick one —</option>
                  {canDraw.map((it) => (
                    <option key={it.file} value={it.file}>
                      {it.file}
                      {it.family === "unknown"
                        ? ""
                        : ` · ${it.family.toUpperCase()}`}
                    </option>
                  ))}
                </select>
              </label>
            )}
            {SIDES.slice(0, views).map((side, at) =>
              handed ? (
                <div className="cedit__field cedit__field--wide" key={side}>
                  <span>{side.toUpperCase()}</span>
                  <ImageDrop
                    label={
                      shapeFrom[at] && shapeFrom[at] !== "?"
                        ? "REPLACE IT"
                        : "CHOOSE A PICTURE"
                    }
                    onChoose={async (dataUri) => {
                      try {
                        const named = await handReferenceOver(dataUri);
                        setShapeFrom((was) =>
                          was.map((it, n) => (n === at ? named : it)),
                        );
                        return null;
                      } catch (why) {
                        // The studio's own words, which name what it did not like.
                        return String(why);
                      }
                    }}
                  />
                </div>
              ) : views === 1 ? null : (
                <label className="cedit__field cedit__field--wide" key={side}>
                  <span>{side.toUpperCase()}</span>
                  <input
                    type="text"
                    placeholder={`what it looks like from the ${side}`}
                    value={at === 0 ? shapePrompts[0] || prompt : shapePrompts[at]}
                    onChange={(e) =>
                      setShapePrompts((was) =>
                        was.map((it, n) => (n === at ? e.target.value : it)),
                      )
                    }
                  />
                </label>
              ),
            )}
            <p className="cc__hint">
              {motion} builds it from a picture — there is no text encoder in
              that graph at all, so words reach it by being drawn first. It runs
              on its own while you keep working, turns in the conversation, and
              the file it leaves is a <b>.glb</b>.
            </p>
            {/*
              **Said, not added.** Measured on this machine: the same model from
              `cow, cartoon style` came back a shapeless blob, and from
              `cow, cartoon style, single object, centered, full body, plain white background`
              came back a cow. The difference is entirely in what the picture looked like.

              Epoch does not append those words. It refused to invent lyrics for a song an hour
              ago for the same reason, and a prompt somebody did not type is a prompt they cannot
              change. What it can do is say what the picture needs to be — the same courtesy the
              sizes get, where the trained one is marked and nothing is chosen.
            */}
            {!handed && (
              <p className="cc__hint cc__hint--read">
                It reads each picture centre-cropped. A prompt that gives it{" "}
                <b>a single subject, whole, centred, on a plain background</b>{" "}
                comes back as a shape; a busy scene comes back as a blob. Epoch
                does not add those words for you — they change the picture, and
                the picture is yours.
              </p>
            )}
          </>
        )}
        {making === "video" && (
          <>
            {view.motions.length > 1 && (
              <label className="cedit__field cedit__field--wide">
                <span>FAMILY</span>
                <select
                  value={motion}
                  onChange={(e) => setMotion(e.target.value)}
                >
                  {view.motions.map((it) => (
                    <option key={it} value={it}>
                      {it}
                    </option>
                  ))}
                </select>
              </label>
            )}
            <div className="cc__row">
              <label className="cedit__field">
                <span>SECONDS</span>
                <input
                  type="number"
                  min={1}
                  max={20}
                  step={1}
                  value={seconds}
                  onChange={(e) => setSeconds(Number(e.target.value) || 1)}
                />
              </label>
              <label className="cedit__field">
                <span>FPS</span>
                <input
                  type="number"
                  min={1}
                  max={60}
                  step={1}
                  value={fps}
                  onChange={(e) => setFps(Number(e.target.value) || 25)}
                />
              </label>
            </div>
            {/*
              **Said, not enforced.** The card decides what actually fits and Epoch cannot know
              that before it tries; what it can do is say what is about to be asked for, so a
              number nobody could have predicted is at least a number somebody chose.
            */}
            <p className="cc__hint">
              {Math.round(seconds * fps)} frames with {motion}. It runs on its
              own while you keep working — the crew card says who is waiting,
              and it arrives in the conversation when it is done.
            </p>
          </>
        )}
      </div>

      {forThisTab.length === 0 && (
        <p className="cc__hint cc__hint--read">
          Nothing on your shelves makes{" "}
          {making === "picture"
            ? "pictures"
            : making === "video"
              ? "video"
              : "sound"}
          .
          Epoch reads a model&rsquo;s family from its own tensors; a model it
          cannot read is offered under both tabs, so an empty list here means
          every model it could read belongs to the other one.
        </p>
      )}

      <label className="cedit__field cedit__field--wide">
        <span>MODEL</span>
        <select
          value={model?.file ?? ""}
          onChange={(e) => setCheckpoint(e.target.value)}
        >
          {forThisTab.map((it) => (
            <option key={it.file} value={it.file}>
              {it.file}
              {it.family === "unknown" ? "" : ` · ${it.family.toUpperCase()}`}
            </option>
          ))}
        </select>
      </label>

      {/*
        **A checkpoint whose text encoder is not in it**, which is how most video models ship.

        Found by drawing one (2026-08-28): `ltxv-2b-0.9.6-distilled` is a checkpoint, loads like
        one, and answers `CLIP = None` — ComfyUI says *"clip input is invalid: None … your
        checkpoint does not contain a valid clip or text encoder model."* So the model and the
        VAE come from the file and the encoder has to come from beside it.

        Only the encoder and its family, because only those are missing. A VAE dropdown here
        would be a control over something the file already has.

        Not shown for a picture: no picture checkpoint on this machine needs one, and a control
        that appears for every model to serve one of them is a control everybody has to ignore.
      */}
      {/*
        **Not for a model.** A mesh loads through `ImageOnlyCheckpointLoader` and is conditioned
        on a picture — measured, there is no text encoder in that graph at all — so asking for
        one is asking for a file the graph has nowhere to put.
      */}
      {making !== "model" &&
        motion !== "" &&
        model?.kind !== "diffusion" &&
        !model?.carriesEncoder && (
        <div className="rm__list rm__list--changes">
          <span className="rm__heading">THIS MODEL NEEDS A TEXT ENCODER</span>
          <p className="cc__hint cc__hint--read">
            A {making} checkpoint carries its model and its VAE and not its text
            encoder — the encoder is a T5 the size of the model itself. Pick the
            one this family was trained with, and the family the loader should
            read it as.
          </p>
          <label className="cedit__field">
            <span>TEXT ENCODER</span>
            <select
              value={clip[0] ?? ""}
              onChange={(e) => setClip(e.target.value ? [e.target.value] : [])}
            >
              <option value="">— pick one —</option>
              {view.encoders.map((it) => (
                <option key={it.file} value={it.file}>
                  {it.file}
                  {it.says ? ` · ${it.says}` : ""}
                </option>
              ))}
            </select>
          </label>
          {/*
            **The one place the panel needs a running machine, with a way out of it.**

            The family list is a *node's* vocabulary, so no shelf can stand in for it — and
            GENERATE waits for a family, which with nothing running is a button that can never
            be pressed. The way out is not to make GENERATE mean two things: it is one control
            that does one thing, here, and then the list arrives and the form is finishable.

            It starts the machine the way GENERATE does, so Epoch owns it and stops it once the
            picture is made.
          */}
          {view.clipTypes.length === 0 && (
            <>
              <p className="cc__hint cc__hint--read">
                Which family this encoder should be read as is a list only a
                running drawing machine has.{" "}
                {waking
                  ? "It is starting — this fills in when it answers."
                  : "Nothing is running, so there is nothing to choose from yet."}
              </p>
              {/*
                **Kept, even though opening the panel now starts it.** This is what is left when
                the automatic start answered *no* — the machine is here and something stopped it
                from starting — and a person pressing it themselves is a different attempt with
                a terminal window they can read. A control that is usually unnecessary is not
                the same as one that is never right.
              */}
              <button
                type="button"
                className="btn btn--mini"
                disabled={waking}
                onClick={() => {
                  setWaking(true);
                  void wakeTheStudio().then((up) => {
                    if (!up) setWaking(false);
                  });
                }}
              >
                {waking ? "STARTING…" : "START THE DRAWING MACHINE"}
              </button>
            </>
          )}
          <label className="cedit__field">
            <span>FAMILY</span>
            <select
              value={clipType}
              onChange={(e) => {
                setTouched(true);
                setClipType(e.target.value);
              }}
            >
              <option value="">— pick one —</option>
              {/*
                `CLIPLoader`'s own list, because one encoder is what a `CLIPLoader` takes — the
                same rule the parts section follows, and the reason a family is never borrowed
                from a loader that will not run.
              */}
              {view.clipTypes.map((it) => (
                <option key={it} value={it}>
                  {it}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}

      {/*
        Only for a model that arrives in parts. A checkpoint carries its own encoder and VAE, and
        three dropdowns over it would be three controls that change nothing.
      */}
      {model?.kind === "diffusion" && (
        <div className="rm__list rm__list--changes">
          <span className="rm__heading">THIS MODEL ARRIVES IN PARTS</span>
          {/*
            **Saying what the family needs, because saying which file is not available.**

            Measured 2026-08-25 against the files on this machine: Epoch reads the *kind* of every
            part correctly and the *family* of almost none — `clip_l`, `t5xxl` and the Flux VAE
            all came back `unknown`. So the obvious help, greying the parts that belong to another
            family the way a LoRA row is greyed, would have greyed nothing and helped nobody.

            What Epoch measured is the model's family. What follows from it is how many encoders
            that family takes and what they are, which is a fact about the family and the same
            kind of statement the shape buttons already make. The user still picks every file.
          */}
          {view.assembly && (
            // Read to use the panel, not read afterwards — so it is legible. See `cc__hint--read`.
            <p className="cc__hint cc__hint--read">
              {view.assembly.says}{" "}
              {view.assembly.encoders != null && (
                <>
                  Epoch measured this model as <b>{view.assembly.family}</b> —
                  the files are yours to choose.
                </>
              )}
            </p>
          )}

          {/*
            **What already drew with this model, here.**

            Epoch reads a checkpoint's family for six families and will never cover an open set —
            but a graph that ran is a fact about this exact model, and it costs nothing because it
            already happened. Filed by hash, so it is about *this file* and not about a name
            somebody could put on any file.

            Offered, never applied: pressing it fills the fields in and the person still presses
            GENERATE. And worded as what it is — a combination that ran, not the right one. A
            second recipe is a second thing that worked.
          */}
          {drewBefore && (
            <p className="cc__hint cc__hint--read">
              This model drew here before, with{" "}
              <b>{drewBefore.clip.join(" + ") || "no separate encoder"}</b>
              {drewBefore.clipType ? (
                <>
                  {" "}
                  as <b>{drewBefore.clipType}</b>
                </>
              ) : null}
              {drewBefore.vae ? (
                <>
                  {" "}
                  and <b>{drewBefore.vae}</b>
                </>
              ) : null}
              .{" "}
              <button
                type="button"
                className="btn btn--mini"
                onClick={() => {
                  setClip([...drewBefore.clip]);
                  setClipType(drewBefore.clipType);
                  setVae(drewBefore.vae);
                  // **And it counts as touching the family.** That field is derived from the
                  // measured suggestion until somebody chooses, so setting the state alone left
                  // the suggestion on screen and put back the one value this recipe exists to
                  // correct — pressed on a Z-Image it filled in the encoder and the VAE and
                  // still said `stable_diffusion`. Found by pressing it.
                  setTouched(true);
                }}
              >
                USE THAT
              </button>
            </p>
          )}

          {/*
            And what was refused. Only ever a refusal **about the configuration** — the server
            said a value is not one it offers, which stays true tomorrow. An out-of-memory is the
            card that day and is never filed, because it would be false the next time the card is
            free.
          */}
          {refusedBefore.map((one) => (
            <p
              key={`${one.clip.join()}${one.clipType}${one.vae}`}
              className="cc__hint"
            >
              Refused here before:{" "}
              {[...one.clip, one.clipType, one.vae].filter(Boolean).join(" · ")}{" "}
              — {one.said}
            </p>
          ))}
          <label className="cedit__field">
            <span>TEXT ENCODER</span>
            <select
              value={clip[0] ?? ""}
              onChange={(e) => setClip(e.target.value ? [e.target.value] : [])}
            >
              <option value="">— pick one —</option>
              {view.encoders.map((it) => (
                <option key={it.file} value={it.file}>
                  {it.file}
                  {it.says ? ` · ${it.says}` : ""}
                  {markIf(it.file, 0)}
                </option>
              ))}
            </select>
          </label>
          <label className="cedit__field">
            <span>
              SECOND ENCODER
              {/*
                **It read as the opposite of what it meant.** *"Flux uses one"* was about this
                field and was read as *Flux uses one encoder* — so the owner picked one, found
                GENERATE grey, and then got a server refusal from the first thing they tried.
                Saying the total is unambiguous, and it is what the guidance above says too.
              */}
              {view.assembly?.encoders != null &&
                view.assembly.encoders >= 2 && (
                  <i className="cc__wants">
                    {" "}
                    · {view.assembly.family} needs {view.assembly.encoders} in
                    total
                  </i>
                )}
            </span>
            <select
              value={clip[1] ?? ""}
              onChange={(e) =>
                setClip((held) =>
                  e.target.value
                    ? [held[0] ?? "", e.target.value]
                    : held.slice(0, 1),
                )
              }
            >
              <option value="">— none —</option>
              {view.encoders.map((it) => (
                <option key={it.file} value={it.file}>
                  {it.file}
                  {it.says ? ` · ${it.says}` : ""}
                  {markIf(it.file, 1)}
                </option>
              ))}
            </select>
          </label>
          {/*
            **The family list belongs to the loader, not to ComfyUI.** Two encoders is a
            `DualCLIPLoader` and it publishes twelve families; one is a `CLIPLoader` and it
            publishes twenty-eight, including `stable_diffusion`, which the double loader has
            never heard of. Offering one list for both let somebody choose a family the server
            then refused — after they had filled the whole panel in correctly.

            Three encoders is a `TripleCLIPLoader`, which has no family input at all, so there is
            nothing to show and nothing to send.
          */}
          {offered.length > 0 && (
            <label className="cedit__field">
              <span>FAMILY</span>
              <select
                value={offered.includes(familyNow) ? familyNow : ""}
                onChange={(e) => {
                  setTouched(true);
                  setClipType(e.target.value);
                }}
              >
                <option value="">— pick one —</option>
                {/*
                  The one the model actually is, named. Two measurements meet here and neither is
                  a guess: Epoch read the family out of the model's tensors, and the server
                  published this list. When they agree, saying so costs nothing and saves somebody
                  choosing between twelve names — which is how a correctly filled panel came to be
                  refused with `'stable_diffusion' not in [...]`.

                  Marked, never selected. The panel is the user's (ADR-0033).
                */}
                {offered.map((it) => (
                  <option key={it} value={it}>
                    {it}
                    {view.assembly?.clipType === it
                      ? ` — what this model is`
                      : ""}
                  </option>
                ))}
              </select>
            </label>
          )}
          <label className="cedit__field">
            <span>VAE</span>
            {/*
              **Sorted by what it decodes, and nothing is taken away.**

              The list was flat and alphabetical, so drawing a picture with Flux offered
              `minimax_h3_audio_vae_fp32` and `minimax_h3_video_vae_fp16` among the four — an
              audio autoencoder, second in the list, with nothing on the row to say so.

              Epoch reads a VAE's *family* as `Unknown` for every file on this machine, and the
              one that does claim a family claims the wrong one. What it can read is the rank of
              its convolutions: rank 4 is a picture, rank 5 adds time, rank 3 has no space in it
              at all. So the row says what the file decodes into, measured.

              **Grouped and never filtered.** A VAE Epoch could not place keeps its place in the
              list under a heading that says so — hiding somebody's file because Epoch failed to
              measure it is the worst thing this control could do, and a group heading costs
              nothing that a filter would have saved.
            */}
            <select value={vae} onChange={(e) => setVae(e.target.value)}>
              <option value="">— pick one —</option>
              {[
                { of: making, say: `Decodes ${WHAT_IT_MAKES[making]}` },
                { of: null, say: "Epoch could not tell what these decode" },
                { of: "other", say: "Decodes something else" },
              ].map((group) => {
                const rows = view.vaes.filter((it) =>
                  group.of === "other"
                    ? it.medium !== null && it.medium !== making
                    : it.medium === group.of,
                );
                if (rows.length === 0) return null;
                return (
                  <optgroup key={group.say} label={group.say}>
                    {rows.map((it) => (
                      <option key={it.file} value={it.file}>
                        {it.file}
                        {it.medium === null && it.says
                          ? ` · says it is ${it.says}`
                          : ""}
                        {it.medium !== null && it.medium !== making && it.says
                          ? ` · ${it.says}`
                          : ""}
                      </option>
                    ))}
                  </optgroup>
                );
              })}
            </select>
          </label>
          <p className="cc__hint">
            The family list is ComfyUI&rsquo;s own, asked of the machine that
            will draw — <b>one list per loader</b>, because they are different
            lists. Two encoders means a dual loader; one means a single one.
          </p>
        </div>
      )}

      {view.loras.length > 0 && (
        <>
          <span className="rm__heading">ADDITIONAL</span>
          <ul className="rdy__list">
            {view.loras.map((lora) => (
              <LoraRow
                key={lora.file}
                lora={lora}
                strength={chosen[lora.file]}
                onToggle={() => toggle(lora.file)}
                onStrength={(value) =>
                  setChosen((held) => ({ ...held, [lora.file]: value }))
                }
                prompt={prompt}
                onTrigger={(words) => {
                  // **Appended, never rewritten.** What somebody typed stays exactly as they
                  // typed it and the words go on the end, which is where a trained word belongs
                  // and the only place Epoch can put one without deciding something.
                  const missing = words.filter((word) => !prompt.includes(word));
                  if (missing.length === 0) return;
                  setPrompt(
                    prompt.trim() === ""
                      ? missing.join(", ")
                      : `${prompt.trim()}, ${missing.join(", ")}`,
                  );
                }}
              />
            ))}
          </ul>
        </>
      )}

      {/*
        **A picture to draw on top of**, on the IMAGE tab and nowhere else: a video, a sound and
        a mesh each already have their own way in, and this is the one the picture path never
        had.

        Offered rather than required — with nothing chosen the graph is exactly what it always
        was, an empty latent and nothing kept.
      */}
      {making === "picture" && (
        <div className="rm">
          <span className="rm__label">From a picture</span>
          <p className="cc__hint cc__hint--read">
            Draw on top of a picture you hand over rather than from nothing. Not a
            ControlNet: that carries the outlines and reinvents everything inside
            them, which is why faces come back as other people. This carries the
            picture itself, and the dial says how much of it is left.
          </p>
          <div className="studio__strength">
            {/*
              **Not the same words as the ControlNet row's picker.** Two buttons reading
              `CHOOSE A PICTURE` on one panel do two different things: one steers by outlines
              and the other is what the picture is drawn on top of. A test caught it by dropping
              into the wrong one, which is exactly what a person would have done.
            */}
            <ImageDrop
              label={
                from === "" ? "CHOOSE THE PICTURE TO DRAW ON" : "REPLACE THAT PICTURE"
              }
              onChoose={async (dataUri) => {
                try {
                  const named = await handReferenceOver(dataUri);
                  // Only once the studio has taken it: a thumbnail of a picture that never
                  // arrived would be the confirmation lying.
                  setFrom(named);
                  setFromShot({ uri: dataUri, size: null });
                  /*
                    **And the size is measured after, never in front.** It is read here rather
                    than asked of anybody — the bytes are already in this window — but a decode
                    that never finishes must not be able to hold up the picture itself, which
                    the studio has already accepted. The thumbnail is the confirmation; the
                    numbers are a detail of it, and the sentence says less rather than waiting.
                  */
                  const probe = new Image();
                  probe.onload = () =>
                    setFromShot({
                      uri: dataUri,
                      size: {
                        width: probe.naturalWidth,
                        height: probe.naturalHeight,
                      },
                    });
                  probe.src = dataUri;
                  return null;
                } catch (why) {
                  return String(why);
                }
              }}
            />
            {from !== "" && (
              <>
                {fromShot !== null && (
                  <img
                    className="studio__base"
                    src={fromShot.uri}
                    alt="the picture this will be drawn on top of"
                  />
                )}
                <button
                  type="button"
                  className="btn btn--mini"
                  onClick={() => {
                    setFrom("");
                    setFromShot(null);
                  }}
                >
                  FROM NOTHING
                </button>
                <input
                  className="studio__slider"
                  type="range"
                  min={0.1}
                  max={0.9}
                  step={0.05}
                  value={keep}
                  onChange={(e) => setKeep(Number(e.target.value))}
                />
                <b>{keep.toFixed(2)}</b>
                <span className="cc__hint">
                  of the picture kept &mdash; 0.20 barely, 0.55 usual, 0.80 almost
                  all of it
                </span>
              </>
            )}
          </div>
        </div>
      )}

      <label className="cedit__field cedit__field--wide">
        <span>PROMPT</span>
        <textarea
          rows={3}
          value={prompt}
          placeholder="what should be in the picture"
          onChange={(e) => setPrompt(e.target.value)}
        />
      </label>

      {/*
        **The last shelf that fed nothing, and it never will feed a node.**

        `Shelf::Embeddings` has been identified from bytes, filed and installed since ADR-0032,
        and no composed graph has ever contained one. Measured rather than assumed: nothing in
        `/object_info` takes an embedding, and ComfyUI gives them their own `/embeddings`
        endpoint — because the only way to use one is to write it into the prompt. There is
        nothing to wire; there was something to *say*.

        And saying it is worth more than it looks. Measured on a real server with the same seed,
        a name that is **not** installed is not refused: `embedding`, `:` and the misspelling are
        encoded as ordinary words and quietly change the picture. Typing one is guessing; this is
        the list.

        It sits under PROMPT and not in ADVANCED because it writes into the prompt, and a
        control belongs next to the thing it changes.
      */}
      {view.embeddings.length > 0 && (
        <div className="cedit__field cedit__field--wide">
          <span>EMBEDDINGS</span>
          <div className="cedit__embeds">
            {view.embeddings.map((name) => (
              <button
                key={name}
                type="button"
                className="btn btn--mini"
                onClick={() =>
                  setPrompt((was) =>
                    was.trim() === ""
                      ? `embedding:${name}`
                      : `${was.trimEnd()} embedding:${name}`,
                  )
                }
              >
                {name}
              </button>
            ))}
          </div>
          <i className="cedit__hint">
            Written into the prompt as <b>embedding:name</b>, which is the only
            way an embedding is used — there is no node for one. A name this
            server does not have is not refused: it becomes words.
          </i>
        </div>
      )}

      {/*
        **A list, because there are now more than three.** Three ratio buttons were the whole
        offer, which quietly decided that nobody wants a wallpaper. The sizes people name out
        loud — HD, Full HD, QHD, 4K — are here, each of them the other way up, and a Custom row
        for anything else.

        The three the family was trained at are **marked**, not fenced off. Asking SD 1.5 for
        1024 gives two heads and asking Flux for 4K gives an out-of-memory; both are true and
        neither is Epoch's call to make for somebody who owns the card. Epoch measured and said
        (ADR-0033), which is the same arrangement a LoRA from another family gets.
      */}
      {/*
        **A picture of its own decides the size, so SIZE stops pretending to.**

        `VAEEncode` takes the handed-over picture's own dimensions, and every value in this list
        was ignored the moment one was chosen — measured, a 1280×960 photo drawing 1280×960 with
        `1:1 · 1024×1024` selected. That is the dead control this codebase already deleted once
        (`Manual`, ADR-0027): a menu whose value changes nothing.

        What replaces it is the reading it was hiding, taken from the picture in this window
        rather than asked of anybody.
      */}
      {from !== "" && (
        <div className="cedit__field cedit__field--wide">
          <span>SIZE</span>
          <p className="cc__hint cc__hint--read">
            {fromShot?.size == null
              ? "The picture you handed over decides this."
              : `The picture you handed over decides this: ${String(fromShot.size.width)}×${String(fromShot.size.height)}.`}{" "}
            Choose FROM NOTHING to pick a size again.
          </p>
        </div>
      )}

      {from === "" && (
      <label className="cedit__field cedit__field--wide">
        <span>SIZE</span>
        <select
          value={String(shape)}
          onChange={(e) => setShape(Number(e.target.value))}
        >
          {view.shapes.map((it, index) => (
            <option key={it.label} value={index}>
              {it.label} · {it.width}×{it.height}
              {it.native ? " — what this model was trained at" : ""}
            </option>
          ))}
          <option value="-1">Custom…</option>
        </select>
      </label>
      )}

      {from === "" && shape === -1 && (
        <div className="rm__list rm__list--changes">
          <label className="cedit__field">
            <span>WIDTH</span>
            <input
              type="number"
              min={64}
              max={4096}
              step={8}
              value={customW}
              onChange={(e) => setCustomW(Number(e.target.value))}
            />
          </label>
          <label className="cedit__field">
            <span>HEIGHT</span>
            <input
              type="number"
              min={64}
              max={4096}
              step={8}
              value={customH}
              onChange={(e) => setCustomH(Number(e.target.value))}
            />
          </label>
          <p className="cc__hint">
            Drawing at {toEight(customW)}×{toEight(customH)}. Latent space works
            in multiples of eight, so anything else is rounded — this is the
            number that will be used.
          </p>
        </div>
      )}

      <button
        type="button"
        className="btn btn--mini"
        onClick={() => setOpen(!open)}
      >
        {open ? "ADVANCED ▲" : "ADVANCED ▼"}
      </button>

      {open && (
        <div className="rm__list rm__list--changes">
          <label className="cedit__field">
            <span>NEGATIVE</span>
            <input
              value={negative}
              placeholder="left empty unless you say otherwise"
              onChange={(e) => setNegative(e.target.value)}
            />
          </label>
          <label className="cedit__field">
            <span>STEPS</span>
            <input
              type="number"
              min={1}
              max={200}
              value={steps}
              onChange={(e) => setSteps(Number(e.target.value))}
            />
          </label>
          <label className="cedit__field">
            <span>CFG</span>
            <input
              type="number"
              min={0}
              max={30}
              step={0.5}
              value={cfg}
              onChange={(e) => setCfg(Number(e.target.value))}
            />
          </label>
          {/*
            **The shelf that filled up and fed nothing.** An upscaler has been identified from its
            bytes, filed and installed since ADR-0032, and no composed graph had ever contained
            one. This is the consumer.

            Marked, never fenced off — the rule the sizes already follow. An upscale is where a
            12 GB card runs out, and saying so beats refusing: the card is theirs.
          */}
          {view.upscalers.length > 0 && (
            <label className="cedit__field">
              <span>UPSCALE</span>
              <select
                value={upscale}
                onChange={(e) => setUpscale(e.target.value)}
              >
                <option value="">— none, save it as drawn —</option>
                {view.upscalers.map((it) => (
                  <option key={it.file} value={it.file}>
                    {it.file}
                    {it.says ? ` · ${it.says}` : ""}
                  </option>
                ))}
              </select>
              <i className="cedit__hint">
                Enlarges the picture that was made, after it is drawn — it does
                not change what is in it. It is also where a card runs out of
                memory.
              </i>
            </label>
          )}
          {/*
            **The other shelf that filed files and fed nothing.** `Shelf::ControlNet` has
            existed since ADR-0032 and no composed graph had ever contained one.

            Shown even when the list is empty, because those are two different sentences: an
            empty list says the shelf is where one would land, and a missing control says Epoch
            cannot do this at all. Only a server with no `ControlNetLoader` hides it, and that is
            a fact about the server.

            The picture and the model are both files, and ADR-0033 settled that files are the
            user's to choose. `draw_image` gains nothing from any of this.
          */}
          {view.controlnets.length > 0 && (
            <div className="cedit__field">
              <span>STEER WITH</span>
              {steers.length === 0 && (
                <i className="cedit__hint">
                  A ControlNet takes the shape out of a picture you give it — a
                  pose, an outline, a depth — and holds the new picture to it.
                  Several stack: a pose from one, a depth from another.
                </i>
              )}
              {steers.map((row, at) => (
                <div className="cedit__steer" key={at}>
                  <select
                    value={row.file}
                    onChange={(e) =>
                      setSteers(
                        steers.map((it, n) =>
                          n === at ? { ...it, file: e.target.value } : it,
                        ),
                      )
                    }
                  >
                    <option value="">— pick one —</option>
                    {view.controlnets.map((it) => (
                      <option key={it.file} value={it.file}>
                        {it.file}
                        {it.says ? ` · ${it.says}` : ""}
                      </option>
                    ))}
                  </select>
                  <ImageDrop
                    label={row.image === "" ? "CHOOSE A PICTURE" : "REPLACE IT"}
                    onChoose={async (dataUri) => {
                      try {
                        const named = await handReferenceOver(dataUri);
                        setSteers(
                          steers.map((it, n) =>
                            n === at ? { ...it, image: named } : it,
                          ),
                        );
                        return null;
                      } catch (why) {
                        // The studio's own words. It names the node and the value it did not
                        // like, and swallowing that in favour of "upload failed" is the thing
                        // this codebase keeps deleting.
                        return String(why);
                      }
                    }}
                  />
                  <input
                    type="range"
                    min={0}
                    max={2}
                    step={0.05}
                    value={row.strength}
                    aria-label="how hard it steers"
                    onChange={(e) =>
                      setSteers(
                        steers.map((it, n) =>
                          n === at
                            ? { ...it, strength: Number(e.target.value) }
                            : it,
                        ),
                      )
                    }
                  />
                  <button
                    type="button"
                    className="btn btn--mini"
                    onClick={() => setSteers(steers.filter((_, n) => n !== at))}
                  >
                    REMOVE
                  </button>
                  <i className="cedit__hint">
                    <b>{row.strength.toFixed(2)}</b> —{" "}
                    {row.image === ""
                      ? "no picture yet, so this one steers by nothing and GENERATE waits for it."
                      : `steering by ${row.image}. 0 ignores it, 1 is the studio's own default, 2 follows it so closely the words stop mattering.`}
                  </i>
                  {/*
                    **What the ControlNet actually reads.** A canny one reads edges, and handed
                    a photograph it draws noise — measured, and it is the reason this control
                    exists rather than a default. Nothing is preselected: which preparation a
                    ControlNet wants is a property of the ControlNet, the only thing naming it
                    is the file name (which ADR-0024 forbids reading), and an edge map and a
                    photograph are both an image, so there is nothing to measure either.
                  */}
                  <select
                    className="cedit__reads"
                    value={row.prepare}
                    aria-label="what to make of the picture"
                    onChange={(e) =>
                      setSteers(
                        steers.map((it, n) =>
                          n === at ? { ...it, prepare: e.target.value } : it,
                        ),
                      )
                    }
                  >
                    <option value="">— what should it read? —</option>
                    <option value="already">
                      it is already a control map
                    </option>
                    {view.preparations.map((it) => (
                      <option key={it} value={it}>
                        find the edges ({it})
                      </option>
                    ))}
                  </select>
                  <i className="cedit__hint cedit__hint--needed">
                    {row.prepare === ""
                      ? "A ControlNet reads a control map — an outline, a depth, a pose — not a photograph. Handed one of those directly it draws noise, so GENERATE waits until you say which this is."
                      : row.prepare === "already"
                        ? "Handed over exactly as it is."
                        : `Run through ${row.prepare} first, at this server's own settings.`}
                  </i>
                </div>
              ))}
              <button
                type="button"
                className="btn btn--mini"
                onClick={() =>
                  setSteers([
                    ...steers,
                    { file: "", image: "", prepare: "", strength: 1 },
                  ])
                }
              >
                {steers.length === 0 ? "STEER BY A PICTURE" : "AND ANOTHER"}
              </button>
            </div>
          )}
          <label className="cedit__field">
            <span>SEED</span>
            <input
              type="number"
              min={0}
              value={seed}
              onChange={(e) => setSeed(Number(e.target.value))}
            />
          </label>
          {model?.family === "flux" && (
            <label className="cedit__field">
              <span>GUIDANCE</span>
              <input
                type="number"
                min={0}
                max={20}
                step={0.5}
                value={guidance}
                onChange={(e) => setGuidance(Number(e.target.value))}
              />
            </label>
          )}
          <p className="cc__hint">
            A seed of <code>0</code> means a different picture every time.
            {model?.family === "flux" &&
              " Flux ignores CFG — guidance is the dial that behaves like one."}
          </p>
        </div>
      )}

      <button
        type="button"
        className="btn"
        disabled={
          drawing ||
          // **Not for a model built from a picture somebody handed over.** There is no text
          // encoder in that graph at all — the prompt reaches nothing — so waiting for one is a
          // button that can never be pressed, over a field that does nothing when it is.
          //
          // Measured in the window: the picture was accepted, the button said REPLACE IT, and
          // GENERATE stayed grey with no reason on screen.
          (prompt.trim() === "" && !(making === "model" && handed)) ||
          !model ||
          // **A ControlNet with no picture steers by nothing.** The hint said GENERATE was
          // waiting for one, and it was not — the Engine drops a half-set steering and draws
          // without it, so the picture would have arrived unsteered with nothing saying why. A
          // sentence the button does not keep is worse than no sentence.
          //
          // Any row: a stack where the third is half-filled would otherwise draw with two and
          // say nothing about the third.
          // A preparation nobody answered is not *already prepared*. Reading silence as an
          // answer is precisely what drew noise, so the row is unfinished until it is said.
          steers.some(
            (row) => row.file === "" || row.image === "" || row.prepare === "",
          ) ||
          // A model that arrives in parts cannot draw until the parts are chosen. Said by a
          // dead button rather than by a refusal after the fact.
          //
          // `offered.length === 0` is three encoders, which is a `TripleCLIPLoader` — it has no
          // family input at all, so requiring one there would block a correctly filled panel.
          (model.kind === "diffusion" &&
            (clip.length === 0 ||
              missingEncoders > 0 ||
              (offered.length > 0 && familyNow === "") ||
              vae === "")) ||
          // A video checkpoint cannot draw until its encoder and family are chosen. Said by a
          // dead button rather than by a server refusal after the form was filled in — the same
          // courtesy the parts section already gets.
          // A checkpoint that borrows an encoder cannot draw until it is chosen — and one that
          // carries its own has nothing to wait for. Measured per file, never assumed from the
          // medium: ACE-Step is all-in-one and this gate held it grey forever.
          (making !== "model" &&
            motion !== "" &&
            model.kind !== "diffusion" &&
            !model.carriesEncoder &&
            (clip.length === 0 || clipType === "")) ||
          // A model needs a picture: either one handed over, or a checkpoint to draw one with.
          // Said by a dead button rather than by a refusal after the form was filled in.
          // A model needs its **front**: a picture for it, or a checkpoint to draw one with.
          // The other three sides are optional in the node itself, so they are optional here.
          (making === "model" &&
            (handed ? shapeFrom[0] === "" || shapeFrom[0] === "?" : shapeWith === ""))
        }
        onClick={draw}
      >
        {drawing ? "SENDING…" : "GENERATE"}
      </button>

      {/*
        **A way out, which this panel had stopped having.**

        GENERATE used to close it. Then it started staying open to say where the picture landed —
        which is right, and left the panel with no way to be closed at all except closing the
        whole conversation. A surface that can only be opened is not finished.

        And it is the gesture that lets the studio go: closing here unmounts the panel, and the
        Engine stops what it started once nothing is queued.
      */}
      <button
        type="button"
        className="btn btn--mini"
        onClick={() => onChosen("")}
      >
        CLOSE
      </button>

      {/*
        **The dead button explains itself.** Epoch measured how many encoders this family takes,
        so *one more to pick* is a fact rather than a rule — and it is the difference between a
        button that looks broken and a button that is waiting.
      */}
      {missingEncoders > 0 && model?.kind === "diffusion" && (
        <p className="cc__hint cc__hint--read">
          {view.assembly?.family ?? "This model"} is loaded with{" "}
          {takes === 2 ? "two" : takes === 3 ? "three" : String(takes)} text
          encoders, and{" "}
          {missingEncoders === 1 ? "one is" : `${missingEncoders} are`} still to
          pick. The family list belongs to the loader that many files select, so
          it cannot be answered until they are chosen.
        </p>
      )}

      {/* A refusal from the server is the one sentence that explains what to change. */}
      {said && <p className="cc__hint cc__hint--read">{said}</p>}
    </div>
  );
}

/** Latent space works in eights; the sampler rounds silently, so the panel rounds visibly. */
function toEight(value: number): number {
  if (!Number.isFinite(value)) return 1024;
  return Math.min(4096, Math.max(64, Math.round(value / 8) * 8));
}

/** One LoRA: whether it is in, how strongly, and why it is not offered when it is not. */
function LoraRow({
  lora,
  strength,
  prompt,
  onToggle,
  onStrength,
  onTrigger,
}: {
  lora: PanelLoraRow;
  strength: number | undefined;
  /** What is in the prompt now, so the row can say whether its words are already there. */
  prompt: string;
  onToggle: () => void;
  onStrength: (value: number) => void;
  onTrigger: (words: readonly string[]) => void;
}) {
  const on = strength !== undefined;
  return (
    <li className={`rdy__row rdy__row--${lora.fits ? "ready" : "needsYou"}`}>
      <span className="rdy__dot" aria-hidden />
      <span className="rdy__name">{lora.file}</span>
      {/*
        **What Epoch measured, and what the site called it.**

        `Base` has five names and calls Pony, Illustrious and NoobAI all `SDXL` — right for what
        it decides (whether a LoRA will load at all) and useless for choosing between two files
        that draw very differently. Measured on the owner's: `DiivesP1` says Pony, `DiivesIXL`
        says Illustrious, and the row said SDXL for both.

        Said and never enforced. It is the author's claim rather than a measurement, and his best
        picture came from the pairing these two words call mismatched.
      */}
      <span className="rdy__note">
        {lora.why ??
          (lora.family === "unknown"
            ? "family unmeasured"
            : lora.family.toUpperCase())}
        {lora.saidBase && lora.why === null && ` · the site called it ${lora.saidBase}`}
      </span>
      {/*
        **What its author says a prompt needs, and a way to put it there.**

        It was a fifth child in a four-column grid, drawn in the tone used for prose read
        afterwards — so on a row that was in use it sat where nobody looked. The owner installed
        a style LoRA, set it to 1.50, wrote a prompt without its trained word and got a picture
        with none of the style in it: `diives` was measured, written beside the file at install,
        and never reached the person who needed to type it.

        **Offered, never inserted.** Epoch does not write words into somebody's prompt — the same
        line that keeps the lyrics box empty rather than filled with a guess. A button is the
        difference between a hint you read and a hint you can act on.
      */}
      {on && lora.triggers.length > 0 && (
        <span className="rdy__trigger">
          <b>needs in the prompt:</b> {lora.triggers.join(", ")}
          <button
            type="button"
            className="btn btn--mini"
            disabled={lora.triggers.every((word) => prompt.includes(word))}
            onClick={() => onTrigger(lora.triggers)}
          >
            {lora.triggers.every((word) => prompt.includes(word))
              ? "IN THE PROMPT"
              : "ADD"}
          </button>
        </span>
      )}
      <button type="button" className="btn btn--mini" onClick={onToggle}>
        {/*
          Never absent for an incompatible one. Epoch measured and said; the file is theirs and
          the answer is theirs — the same arrangement as a model too large for this card.
        */}
        {on ? "REMOVE" : lora.fits ? "USE" : "USE IT ANYWAY"}
      </button>
      {on && (
        /*
          **The number needed saying out loud.** The bar ran 0 to 1.5 with the value beside it
          and nothing else, and the owner read it backwards — reasonably: a slider with no ends
          written on it is a slider you have to guess at. `0` is none of this LoRA and `1.50` is
          all of it and then some; `0.80` is what its authors usually mean by *on*.
        */
        <span className="studio__strength">
          <input
            className="studio__slider"
            type="range"
            min={0}
            max={1.5}
            step={0.05}
            value={strength}
            aria-label={`${lora.file} strength`}
            onChange={(e) => onStrength(Number(e.target.value))}
          />
          <b className="studio__strengthValue">{strength?.toFixed(2)}</b>
          <i className="studio__strengthEnds">
            0 = off · 0.80 = usual · 1.50 = strongest
          </i>
        </span>
      )}
    </li>
  );
}
