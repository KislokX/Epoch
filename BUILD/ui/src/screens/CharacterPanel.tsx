/**
 * The crew — who exists, and where they live.
 *
 * Characters belong to the user, not to a World (ADR-0023). Their name, face, personality,
 * role and routine live in the vault and travel with them, which is why they are managed here
 * and not inside any particular World.
 *
 * ## Two commands, on purpose
 *
 * Editing somebody and moving them between Worlds are separate: a slip in the form can change
 * what a character is like, but it can never quietly evict them from a World. The roster is
 * only ever touched by the checkbox that says so.
 *
 * ## Nothing is validated here
 *
 * The Engine owns what a valid character is and has to live with the file afterwards. This
 * form collects what the user typed and shows the Engine's answer. A UI that validated
 * separately would eventually disagree with the thing writing the file.
 */

import { useEffect, useState } from "react";

import { Hire } from "./Hire";
import { Mark } from "../components/Mark";
import { CharacterArtSlot } from "../components/CharacterArtSlot";
import { AnimationSlot } from "../components/AnimationSlot";
import { RemoveConfirm } from "../components/RemoveConfirm";
import { BoundedControl } from "../components/ProviderControl";
import {
  agentModels,
  fetchAgents,
  signInAgent,
  fetchSurface,
  fetchInherited,
  installedVoices,
  installedTimbres,
  voiceForge,
  saveCharacter,
  type InstalledVoice,
  type InstalledTimbre,
  type VoiceForge,
  characterRemoval,
  removeCharacter,
  setCharacterArt,
  setCharacterCut,
  setCharacterWorld,
} from "../ipc/launcher";
import type { AgentStatus, ArtSlot, SheetCut } from "../ipc/launcher";
import type {
  Action,
  CharacterEdit,
  RemovalView,
  CharacterSummary,
  Control,
  LauncherView,
  Inherited,
  MarkView,
  ProviderStatus,
  Surface,
  TuningValue,
} from "../ipc/contracts";

/** Archetype ids are canonical; these are the Launcher's plain-language labels. */
const ARCHETYPE_LABEL: Record<string, string> = {
  researcher: "Researcher",
  coordinator: "Coordinator",
  guardian: "Guardian",
  historian: "Historian",
};

function titleise(id: string): string {
  return id.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}

function crewLine(n: number): string {
  if (n === 0) return "Nobody signed on yet.";
  return `${n} ${n === 1 ? "person" : "people"} signed on.`;
}

/** Everything the editor can change, taken from what the Engine last reported. */
function toEdit(c: CharacterSummary): CharacterEdit {
  return {
    id: c.id,
    name: c.name,
    archetype: c.archetype,
    role: c.role,
    prompt: c.prompt,
    provider: c.provider,
    agent: c.agent,
    model: c.model,
    routine: c.routine.map((step) => ({ ...step })),
    parameters: { ...c.parameters },
    // Undecided travels as `null` all the way to the Engine, so a save that never touched a
    // tick-box cannot narrow it to whatever was connected this minute.
    requestedCapabilities: c.requestedCapabilities
      ? [...c.requestedCapabilities]
      : null,
    skills: [...c.skills],
    // Carried through so a save keeps the voice somebody chose. The Engine treats `null` as
    // untouched, but a form that shows the control must send what the control says.
    speaksWith: c.speaksWith,
    soundsLike: c.soundsLike,
    tuning: { ...c.tuning },
  };
}

/**
 * A bounded parameter, shown as the range it actually has.
 *
 * A slider is only honest where the limits are *known*. Temperature and Top P have fixed bounds
 * the Engine enforces, so the track can show them and the user never has to discover a limit by
 * being refused.
 *
 * This paragraph used to end with an exception for `context_tokens`, whose ceiling belonged to
 * the loaded model. There is no such control here any more: the window is MODELS' (ADR-0026
 * amendment) and a character reads it rather than setting it.
 *
 * Unset is a real state, and a thumb sitting somewhere would claim a value nobody chose. So
 * an unset slider is inert and says whose default is in force until it is touched.
 */
/**
 * One knob a Provider declared, drawn without knowing which Provider.
 *
 * This function is the whole of what the interface knows about any backend's settings. Adding
 * a Provider adds nothing here — that is the point of the declaration (ADR-0003, ADR-0026),
 * and it is why the previous version of this section could only *display* what it could not
 * understand.
 *
 * Unset is a state, not a zero. A control nobody touched leaves no key in the file, so the
 * Provider's own default stands and the file honestly says nobody chose.
 */
function Declared({
  control,
  value,
  onChange,
}: {
  readonly control: Control;
  readonly value: TuningValue | null;
  readonly onChange: (value: TuningValue | null) => void;
}) {
  const suffix =
    control.kind.kind === "whole" && control.measured ? " · measured" : "";

  if (control.kind.kind === "toggle") {
    return (
      <label className="cedit__field" title={control.help}>
        <span>{control.label}</span>
        <label className="cc__check">
          <input
            type="checkbox"
            checked={value === true}
            onChange={(e) => onChange(e.target.checked ? true : null)}
          />
          <span>{value === null ? "provider default" : String(value)}</span>
        </label>
      </label>
    );
  }

  if (control.kind.kind === "choice") {
    return (
      <label className="cedit__field" title={control.help}>
        <span>{control.label}</span>
        <select
          value={typeof value === "string" ? value : ""}
          onChange={(e) =>
            onChange(e.target.value === "" ? null : e.target.value)
          }
        >
          <option value="">— provider default —</option>
          {control.kind.options.map((option) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
      </label>
    );
  }

  if (control.kind.kind === "free") {
    return (
      <label className="cedit__field" title={control.help}>
        <span>{control.label}</span>
        <input
          value={typeof value === "string" ? value : ""}
          placeholder="provider default"
          onChange={(e) =>
            onChange(e.target.value === "" ? null : e.target.value)
          }
        />
      </label>
    );
  }

  // A number with a known range. Drawn as a slider for the same reason the canonical
  // parameters are: a bounded number typed into a box is a number somebody types wrongly.
  //
  // Except when the range is enormous — a seed spans the whole integer line, and a slider
  // across it cannot express a value anybody meant.
  const { min, max } = control.kind;
  const huge = max - min > 1_000_000;
  if (huge) {
    return (
      <label className="cedit__field" title={control.help}>
        <span>{control.label}</span>
        <input
          type="number"
          value={typeof value === "number" ? value : ""}
          min={min}
          placeholder="provider default"
          onChange={(e) =>
            onChange(e.target.value === "" ? null : Number(e.target.value))
          }
        />
      </label>
    );
  }

  return (
    <BoundedControl
      label={control.label + suffix}
      value={typeof value === "number" ? value : null}
      min={min}
      max={max}
      step={control.kind.kind === "ratio" ? 0.05 : 1}
      onChange={onChange}
    />
  );
}
/** How one choice of mind is carried in a `<select>`. Flattened for the widget, split on read. */
// Escaped, never the literal byte. Written as a raw NUL this file was *binary* to git,
// to diffs and to every editor that decides to tidy control characters away — a
// separator nobody can type, stored as a character nobody can see.
const MIND_SEP = "\u0000";

/** Not a model name. A sentinel the select uses to mean "let me type one". */
const NAME_IT = "\u0000name-it";

function mindValue(provider?: string | null, model?: string | null): string {
  return provider && model ? `${provider}${MIND_SEP}${model}` : "";
}

/**
 * A character's face, drawn small.
 *
 * The *same* `MarkView` the World draws them with, through the same renderer — so what the
 * Launcher shows is what the World will show. A second, prettier portrait pipeline would be
 * free to disagree with the World, and eventually would.
 *
 * The footprint is chosen so the artwork fills the box at whatever scale they authored: the
 * mark is drawn at `footprint × scale`, so a footprint of `100 / scale` always yields 100.
 */
/**
 * The four things a character can be drawn doing, and what each one means.
 *
 * The words are the Engine's — `idle`, `walk`, `think`, `work` — because a label the surface
 * invented would be a fifth vocabulary to keep in step. The sentences explain *when* the World
 * uses each, which is the only thing a person needs to decide what to draw.
 */
const ANIMATIONS: readonly {
  readonly action: Action;
  readonly label: string;
  readonly hint: string;
}[] = [
  { action: "idle", label: "Idle", hint: "Standing about, available." },
  {
    action: "walk",
    label: "Walking",
    // **The one action where the row directions matter**, and the editor never said so. Four
    // directions is four *rows of one sheet*, not four imports — the World picks the row from
    // the facing the Engine measured on the journey. Somebody looking for four Walking slots
    // is looking for something that would be four files describing one thing.
    hint: "On the road between two Places. Four directions go in four rows of one sheet — name them below, top row first.",
  },
  {
    action: "settle",
    label: "Arriving",
    hint: "Just arrived, taking the place in. About a second.",
  },
  {
    action: "talk",
    label: "Meeting",
    hint: "Just arrived, and the person the work came from is here.",
  },
  { action: "think", label: "Thinking", hint: "A turn started; no tool has run yet." },
  { action: "work", label: "Working", hint: "Something is genuinely running." },
];

function Portrait({
  mark,
  name,
}: {
  readonly mark: MarkView | null;
  readonly name: string;
}) {
  if (!mark) {
    // Honest rather than unfinished. Nobody authored a face, and inventing one here would be
    // the same lie as a World that draws a building it does not have.
    return (
      <div
        className="cc__face cc__face--none"
        role="img"
        aria-label={`${name} has no artwork`}
      >
        <span>no face</span>
      </div>
    );
  }

  const footprint = mark.asset && mark.scale > 0 ? 100 / mark.scale : 100;
  return (
    <svg
      className="cc__face"
      viewBox="-50 -100 100 100"
      role="img"
      aria-label={`${name}'s artwork`}
    >
      <Mark mark={mark} footprint={footprint} />
    </svg>
  );
}

/**
 * One of a character's two images: what it is now, and how to change it.
 *
 * A filename in a text box told you nothing about whether the picture was right, and asked you
 * to know the vault's folder layout. The picture answers both.
 *
 * Sprite and icon are separate slots because they answer different questions — the body that
 * walks around the World, and the face that identifies somebody in a conversation. Replacing
 * one must never touch the other, which is why they are separate files under separate stems.
 */
/** `65536` reads as `64K`. Exact multiples only — anything else keeps its own digits. */
function asWindow(tokens: number): string {
  return tokens % 1024 === 0 ? `${tokens / 1024}K` : String(tokens);
}

/**
 * The four policies, in the order somebody reads them.
 *
 * The words are the Engine's (`ContextPolicy`), and the sentences under them are this file's
 * because they are presentation. `Adaptive` is the default and is marked as one — an unset
 * character is `Adaptive`, so showing the group with nothing selected would claim a decision
 * nobody has made and, worse, invite a click that changes nothing.
 *
 * **`Adaptive`, not `Auto`**: MODELS keeps `AUTO` for picking a physical profile, and two layers
 * with one word between them is what this repository paid for at `Manual` (ADR-0027).
 */
const POLICIES: readonly {
  readonly id: string;
  readonly label: string;
  readonly about: string;
}[] = [
  {
    id: "adaptive",
    label: "Adaptive",
    about: "Uses as much as the task needs.",
  },
  {
    id: "compact",
    label: "Compact",
    about: "Works from little. Keeps the turn short.",
  },
  {
    id: "long",
    label: "Long",
    about: "Uses as much of the window as is available.",
  },
  { id: "custom", label: "Custom", about: "Configured by you." },
];

/**
 * What this character's Brain runs as, read from MODELS and never written from here.
 *
 * ## Three readings and a silence, each named
 *
 * `64K` on its own is the gauge that identifies nobody. A window applied by a measured profile,
 * one chosen as a loadout, and one the backend simply reports about its own model are three
 * different facts, and only the first has a speed that belongs beside it — a model's last timing
 * was taken at whatever loadout was current that afternoon.
 *
 * A Brain nothing has configured says exactly that. It does not fall back to a plausible number,
 * and it does not go blank: **Not configured in MODELS** is information, and the button under it
 * is what somebody does about it.
 */
function RuntimeFromModels({
  inherited,
  onConfigure,
}: {
  readonly inherited: Inherited | null;
  readonly onConfigure?: (model: string) => void;
}) {
  /*
    **Not yet asked is not the same as nobody configured**, and until this was separated the two
    drew the same sentence. Found by driving the window: the panel read `Not configured in MODELS`
    for a model with a 64K loadout, and a second later it read `64K · configured` — so the wrong
    answer was the one on screen at the moment somebody opens the section, and it was the one that
    looks like a finished measurement.

    A reading in flight says nothing at all. That is the same rule as `None` meaning *unasked*
    everywhere else in this codebase; it had simply never been given a shape on a surface.
  */
  const asked = inherited !== null;
  const window_ = inherited?.window ?? null;
  const source = inherited?.source ?? "nothing";
  return (
    <div className="cedit__field cedit__field--wide runtime">
      <div className="runtime__head">
        <span>Runtime configuration</span>
        <span className="runtime__from">Inherited from MODELS</span>
      </div>

      <div className="runtime__rows">
        <div className="runtime__row">
          <span className="runtime__what">Context window</span>
          {!asked ? (
            <span className="runtime__cold">…</span>
          ) : window_ === null ? (
            <span className="runtime__cold">Not configured in MODELS</span>
          ) : (
            <span className="runtime__is">
              {asWindow(window_)}
              <span className="runtime__note">
                {" · "}
                {source === "profile"
                  ? `measured${inherited?.profile ? ` · ${inherited.profile}` : ""}`
                  : source === "loadout"
                    ? "configured"
                    : "reported by the model"}
              </span>
            </span>
          )}
        </div>

        <div className="runtime__row">
          <span className="runtime__what">Performance</span>
          {/*
            **Only a profile carries a speed.** Anything else would be a real reading of a
            different configuration, which is the most convincing way a gauge can lie — so a
            window nobody searched for reads as nothing at all.
          */}
          {inherited?.generation == null ? (
            <span className="runtime__cold">
              {!asked || window_ === null ? "—" : "Not measured"}
            </span>
          ) : (
            <span className="runtime__is">
              {inherited.generation.toFixed(1)} tok/s
              <span className="runtime__note">
                {" · "}
                {inherited.stable ? "stable" : "unsteady"}
              </span>
            </span>
          )}
        </div>
      </div>

      {/*
        **The Engine names the row, not this panel.** A character on llama.cpp carries the name
        the router serves it under and the Models deck keys on the shelf's own name — they differ
        for every model with a colon in it, and the deck silently drops an errand naming a model
        it does not hold. So the door is addressed with the name the reading came back with.
      */}
      {inherited && onConfigure && (
        <button
          type="button"
          className="btn btn--mini"
          onClick={() => onConfigure(inherited.model)}
        >
          Configured in MODELS →
        </button>
      )}
    </div>
  );
}

/**
 * How this character wants to fill whatever window it is given.
 *
 * The surviving half of the old `context_tokens`, and the half that is genuinely portable: a
 * Historian who wants everything it can have still wants that on a bigger card. It governs the
 * space **between** the floor and the ceiling — the Required blocks are spent before any of this
 * is asked, so no policy can make a character that does not fit, fit.
 */
function ContextPolicyChoice({
  value,
  onChange,
}: {
  readonly value: string | null;
  readonly onChange: (value: string | null) => void;
}) {
  // Unset *is* Adaptive in the Engine, so the group shows it selected. What it does not do is
  // write the word to the file for somebody who never touched it.
  const chosen = value ?? "adaptive";
  return (
    <div className="cedit__field cedit__field--wide policy">
      <span>Character context policy</span>
      <div className="policy__rows">
        {POLICIES.map((one) => (
          <label
            key={one.id}
            className={`policy__one${chosen === one.id ? " policy__one--on" : ""}`}
          >
            <input
              type="radio"
              name="context-policy"
              value={one.id}
              checked={chosen === one.id}
              onChange={() => onChange(one.id)}
            />
            <span className="policy__label">{one.label}</span>
            <span className="policy__about">{one.about}</span>
          </label>
        ))}
      </div>
    </div>
  );
}

interface CharacterPanelProps {
  readonly view: LauncherView;
  /**
   * Who can currently do the thinking.
   *
   * The model dropdown is built from what the Engine actually probed, never from a list kept
   * here — a hand-written list drifts the moment a model is pulled, and the drift would look
   * like Epoch losing a model the user knows they have.
   */
  readonly providers: readonly ProviderStatus[];
  /** Re-read the vault. Called after every write: the file is the source of truth. */
  readonly onChanged: () => void | Promise<void>;
  /**
   * Take the user to where this Brain is actually configured.
   *
   * The panel shows a window it cannot change, so it has to be able to say **where it can be
   * changed** — a read-only reading with no way through to the thing that decides it is a
   * dead end wearing a label. Absent in a test, where there is no deck to travel to.
   */
  readonly onConfigureInModels?: (model: string) => void;
}

/**
 * Which machine the chosen Service runs on, in one line.
 *
 * **Read from the Provider, never assumed.** `local` is what a Provider says about itself, and a
 * Bridge says `false` — it costs no money and it is still somebody else's computer, which is the
 * distinction the disclosure question keys on. Calling a paired MacBook "this PC" would be the
 * editor lying about where a turn goes.
 */
export function whereItRuns(
  edit: {
    readonly agent?: string | null;
    readonly provider?: string | null;
    readonly model?: string | null;
  },
  providers: readonly ProviderStatus[],
): string {
  // An agent is a program installed on this machine. There is no remote arrangement for one, so
  // saying so is a fact rather than a default.
  if (edit.agent) return "Runs on this PC.";
  if (!edit.provider) return "Pick what thinks for them.";

  const chosen = providers.find((p) => p.id === edit.provider);
  // Chosen but not currently offered — a backend switched off, or a machine unpaired. Saying
  // nothing about the machine beats naming one that is not there.
  if (!chosen) return "That Service is not available right now.";
  /*
    **A cloud model breaks the assumption this line rested on.**

    Ollama runs on the user's own computer, so its Provider is local — and
    `ollama pull gpt-oss:120b-cloud` gives it a model it executes on *Ollama's* servers.
    Measured: that tag resolves in the registry like any other, so the local daemon serves it and
    nothing about the Service says the turn left.

    "Runs on this PC" about that is the sentence this whole field exists to keep true, said about
    the one case where it is false. The Engine draws the same line one layer down
    (`disclosure::destination`), and it has to be drawn here too because this is where somebody
    reads it.
  */
  if (runsInTheCloud(edit.model)) {
    return `Runs on ${chosen.name}'s servers, not on this machine. Your conversation goes to them.`;
  }
  if (chosen.local) return "Runs on this PC.";
  // The machine, because that is the thing the sentence is about — `name` is the program now.
  return `Runs on ${machineOf(chosen)} — ${chosen.endpoint}. Your conversation travels there.`;
}

/**
 * Whether a model runs on somebody else's servers rather than where it was asked from.
 *
 * By tag, because that is what Ollama itself uses and the only thing on the wire that says so.
 * The Engine has the same rule (`ollama_library::runs_in_the_cloud`); it is repeated rather than
 * fetched because it is four characters of string comparison and a round trip per keystroke
 * would be the expensive way to learn a suffix.
 */
export function runsInTheCloud(model: string | null | undefined): boolean {
  const tag = model?.split(":").pop();
  return tag === "cloud" || (tag?.endsWith("-cloud") ?? false);
}

/**
 * Which machine a Service runs on, as a person names it.
 *
 * **From the endpoint, not from the label.** A Bridge's name is already its machine, but an
 * OpenAI-compatible Service is whatever the user called it — `llama.cpp`, `the studio box` —
 * and that says nothing about where it is. The host is the one field that always does.
 *
 * Local is *This PC* rather than `127.0.0.1`, because that is what somebody standing at it
 * calls it, and because agents live in the same group and have no endpoint at all.
 */
export function machineOf(provider: {
  readonly local: boolean;
  readonly endpoint: string;
  readonly machine?: string | null;
}): string {
  // **What the Engine said, when it said anything.** A Bridge was told the machine's real name
  // at pairing time — `studio-mac.local`, or whatever the user renamed it to — and a
  // name a person chose beats a host parsed out of an IP.
  if (provider.machine) return provider.machine;
  if (provider.local) return "This PC";
  // The fallback, for a Provider from a build that did not say. Kept because losing the
  // grouping entirely would be worse than an address as a heading.
  const host = provider.endpoint
    .replace(/^[a-z]+:\/\//i, "")
    .split("/")[0]
    ?.replace(/:\d+$/, "");
  return host && host.length > 0 ? host : "Somewhere else";
}

/**
 * Every machine that offers something, each with what it offers.
 *
 * The order is the order the Services arrived in, with this PC first — the machine somebody is
 * standing at is the one they mean most often, and a list that buried it under a paired one
 * would be sorting by an accident of pairing time.
 */
export function byMachine<T extends { readonly local: boolean; readonly endpoint: string }>(
  providers: readonly T[],
): { machine: string; offers: T[] }[] {
  const groups = new Map<string, T[]>();
  for (const provider of providers) {
    const machine = machineOf(provider);
    const held = groups.get(machine);
    if (held) held.push(provider);
    else groups.set(machine, [provider]);
  }
  return [...groups.entries()]
    .map(([machine, offers]) => ({ machine, offers }))
    .sort((a, b) => Number(b.machine === "This PC") - Number(a.machine === "This PC"));
}

/**
 * Which machine a character is currently assigned to, from what they are assigned to.
 *
 * **Derived, never stored.** The Experience Layer's rule applied to a form: state holds user
 * intent only, and everything else is computed on read. A `machine` field saved beside the
 * provider would be a second copy of a fact the provider already carries, free to disagree with
 * it the moment a Service is renamed or a machine re-paired.
 *
 * `null` is *nothing chosen yet*, which is a real state and a different one from `This PC`.
 */
export function machineFor(
  edit: { readonly agent?: string | null; readonly provider?: string | null },
  providers: readonly ProviderStatus[],
): string | null {
  // An agent is a program installed here. There is no remote arrangement for one.
  if (edit.agent) return "This PC";
  if (!edit.provider) return null;
  const chosen = providers.find((p) => p.id === edit.provider);
  // Assigned to something not offered right now — a backend switched off, a machine unpaired.
  // Saying nothing beats naming a machine that is not there.
  return chosen ? machineOf(chosen) : null;
}

/**
 * What can think on one machine: its Services, and its agents when it is this one.
 *
 * This is the **Brain** level — the middle term of `Provider · Brain · Model`. It is derived
 * from the machine rather than filtered by the user, which is what makes the invalid pairing
 * *unexpressible* instead of merely refused: there is no way to hold a machine and a Service
 * that is not on it, because the second list does not contain one.
 */
export function brainsOn(
  machine: string | null,
  providers: readonly ProviderStatus[],
  agents: readonly { readonly id: string; readonly name: string; readonly installed: boolean }[],
): Brain[] {
  if (!machine) return [];
  const here: Brain[] = providers
    .filter((p) => machineOf(p) === machine)
    .map((p) => ({
      kind: "model" as const,
      id: p.id,
      name: p.name,
      installed: true,
      // **Kept, because it is the answer to a question asked right here.** Ollama switched off
      // on this machine still holds `gemma4`, `gpt-oss` and `qwen3` on disk — measured — and
      // the crew editor showed it with an empty Model list and no explanation. Epoch was not
      // lying; it was silent in the one place somebody was looking.
      online: p.online,
    }));
  if (machine === "This PC") {
    here.push(
      ...agents.map((a) => ({
        kind: "agent" as const,
        id: a.id,
        name: a.name,
        installed: a.installed,
        // An agent that is installed is startable on demand. There is no server to be down.
        online: a.installed,
      })),
    );
  }
  return here;
}

/** One thing that can think, on one machine. */
export type Brain = {
  kind: "model" | "agent";
  id: string;
  name: string;
  installed: boolean;
  online: boolean;
};

/**
 * Every machine that has anything on it, whether or not it has a Service.
 *
 * A computer with Claude Code installed and no backend configured is still a machine somebody
 * can put a character on — it was the group that never formed, and its agents vanished with it.
 */
export function machinesWith(
  providers: readonly ProviderStatus[],
  agents: readonly unknown[],
): string[] {
  const machines = byMachine(providers).map((group) => group.machine);
  if (agents.length > 0 && !machines.includes("This PC")) machines.unshift("This PC");
  return machines;
}

export function CharacterPanel({
  view,
  providers,
  onChanged,
  onConfigureInModels,
}: CharacterPanelProps) {
  /** Which character is open in the editor, and what has been typed. User intent only. */
  const [edit, setEdit] = useState<CharacterEdit | null>(null);
  /**
   * What is on the voices shelf.
   *
   * Read once when this panel opens rather than per character: it is a fact about the machine,
   * and the shelf does not change while somebody is typing a description.
   */
  const [voices, setVoices] = useState<readonly InstalledVoice[]>([]);
  const [timbres, setTimbres] = useState<readonly InstalledTimbre[]>([]);
  const [forge, setForge] = useState<VoiceForge | null>(null);

  useEffect(() => {
    void installedVoices().then(setVoices);
    // Two separate readings, because they are two separate machines' worth of state: a voice is
    // installed, a timbre is *converted*, and the forge is what says whether converting is even
    // possible here. Collapsing them would be one gauge answering three questions.
    void installedTimbres().then(setTimbres);
    void voiceForge().then(setForge);
  }, []);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  /**
   * What the assigned Provider says it has, for the assigned model.
   *
   * Fetched rather than known, and re-fetched when either changes: the interesting bound is a
   * property of the model. Empty until it answers, so the panel opens immediately and the
   * Advanced section fills in — the same rule the World follows on its first frame.
   */
  const [surface, setSurface] = useState<Surface>({
    window: null,
    controls: [],
    can: null,
  });
  /**
   * What MODELS says about this Brain: the window it applied, and how fast it measured.
   *
   * **Fetched and never edited.** ADR-0026's amendment moved the window out of the character, so
   * this panel has a reading rather than a field — and it re-reads whenever the assigned Brain
   * changes, which is what makes a profile applied in MODELS show up here without anybody
   * touching the character file.
   */
  const [inherited, setInherited] = useState<Inherited | null>(null);
  /**
   * Which agents this machine has, measured once when the panel opens.
   *
   * Every one, including the missing — a list that showed only what is installed could not say
   * *Claude Code is not installed*, which is the sentence that tells somebody what to do.
   * Measured rather than remembered: each is a program that runs or does not.
   */
  const [agents, setAgents] = useState<readonly AgentStatus[]>([]);
  /**
   * The machine the user just picked, before they have picked what runs on it.
   *
   * **The only piece of this cascade that is state**, and it is user intent — the other two
   * levels are the character's own fields. It exists because picking a machine with two Brains
   * on it is a real moment where nothing is assigned yet, and a form that could not hold that
   * moment would have to guess which Brain was meant.
   *
   * Cleared whenever a different character opens: it is a step in one edit, not a preference.
   */
  const [pickedMachine, setPickedMachine] = useState<string | null>(null);
  /**
   * Whether the agent's model is being typed by hand rather than picked.
   *
   * Presentation only — the Engine sees a model name either way. It exists so the list can be a
   * real list without becoming a closed one.
   */
  const [namingModel, setNamingModel] = useState(false);
  /**
   * What the chosen agent calls its own models.
   *
   * From the Engine, per agent. It was a constant in this file, which was right while Claude Code
   * was the only agent and became wrong the moment Codex existed: Codex would have been offered
   * `opus`, `sonnet` and `haiku`, none of which it has.
   */
  const [agentModels_, setAgentModels] = useState<readonly [string, string][]>(
    [],
  );

  useEffect(() => {
    const chosen = edit?.agent;
    if (!chosen) {
      setAgentModels([]);
      return;
    }
    let alive = true;
    void agentModels(chosen).then((found) => alive && setAgentModels(found));
    return () => {
      alive = false;
    };
  }, [edit?.agent]);
  /** Whether the "make somebody" form is open. */
  const [hiring, setHiring] = useState(false);
  /** Somebody just made: opened for editing once the roster has been re-read. */
  const [openOn, setOpenOn] = useState<string | null>(null);

  /**
   * Ask again who is installed and who is signed in.
   *
   * Named rather than inlined because signing in happens **outside Epoch** — in the agent's own
   * window — so there is nothing to await. The honest shape is a measurement the user can
   * repeat, not a spinner waiting on somebody else's browser.
   */
  const remeasure = () => {
    // **Fresh, or this button stops meaning anything.** The Engine keeps what it measured
    // because asking costs half a second of process spawning; the press is the way to say
    // *ask again*, and a REMEASURE that returned the same kept answer would be a lie with a
    // label on it.
    void fetchAgents(true).then(setAgents);
  };

  useEffect(() => {
    let alive = true;
    void fetchAgents().then((found) => alive && setAgents(found));
    return () => {
      alive = false;
    };
  }, []);

  /**
   * Open the editor on somebody who was just made.
   *
   * Deferred until the roster has actually come back: the moment they were created, this
   * component was still holding the previous `view`, and opening on an id it did not contain
   * would have shown an empty form for a character who does exist.
   */
  useEffect(() => {
    if (!openOn) return;
    const made = view.characters.find((c) => c.id === openOn);
    if (!made) return;
    setEdit(toEdit(made));
    setOpenOn(null);
  }, [openOn, view.characters]);
  const controls = surface.controls;

  useEffect(() => {
    const provider = edit?.provider;
    const model = edit?.model;
    if (!provider || !model) {
      setSurface({ window: null, controls: [], can: null });
      return;
    }
    let alive = true;
    void fetchSurface(provider, model).then((declared) => {
      if (alive) setSurface(declared);
    });
    return () => {
      alive = false;
    };
  }, [edit?.provider, edit?.model]);

  /**
   * Re-read what MODELS applied, whenever the Brain changes or this panel is reopened.
   *
   * A profile applied in MODELS has to show up here **without editing the character**, which is
   * the whole point of the character no longer holding a window. Re-mounting the deck is the
   * cheapest honest trigger: this is one small read, never a probe.
   */
  useEffect(() => {
    const provider = edit?.provider;
    const model = edit?.model;
    if (!provider || !model) {
      setInherited(null);
      return;
    }
    let alive = true;
    void fetchInherited(provider, model).then((read) => {
      if (alive) setInherited(read);
    });
    return () => {
      alive = false;
    };
  }, [edit?.provider, edit?.model]);

  const open = (character: CharacterSummary) => {
    setError(null);
    setPickedMachine(null);
    setEdit(edit?.id === character.id ? null : toEdit(character));
  };

  const save = async () => {
    if (!edit) return;
    setBusy(true);
    const failure = await saveCharacter(edit);
    setBusy(false);
    if (failure) {
      setError(failure);
      return;
    }
    setError(null);
    setEdit(null);
    await onChanged();
  };

  /**
   * Set or clear one of a character's images.
   *
   * Applied straight away and then re-read, so what the editor shows is what the vault holds.
   * The form never carries artwork, so there is nothing here for a later SAVE to undo.
   */
  const art = async (
    characterId: string,
    slot: ArtSlot,
    image: string | null,
    cut?: SheetCut,
  ): Promise<string | null> => {
    setBusy(true);
    const failure = await setCharacterArt(characterId, slot, image, cut);
    setBusy(false);
    setError(failure);
    await onChanged();
    return failure;
  };

  /**
   * Change how a sheet is read, leaving the sheet alone.
   *
   * A separate act from an import: nothing is uploaded and no file is written, which is what
   * makes fixing a typo in the grid two numbers rather than the same PNG again.
   */
  const recut = async (
    characterId: string,
    action: Action,
    cut: SheetCut,
  ): Promise<string | null> => {
    setBusy(true);
    const failure = await setCharacterCut(characterId, action, cut);
    setBusy(false);
    setError(failure);
    await onChanged();
    return failure;
  };

  /**
   * Whose removal is being read, and what it would do.
   *
   * Two pieces of state rather than one: the id opens the block, and the plan arrives after the
   * Engine has been asked. A block that opened already holding a plan would be a block showing
   * a plan somebody guessed.
   */
  const [removing, setRemoving] = useState<string | null>(null);
  const [plan, setPlan] = useState<RemovalView | null>(null);

  const askRemoval = async (characterId: string) => {
    setRemoving(characterId);
    setPlan(null);
    const answer = await characterRemoval(characterId);
    if (typeof answer === "string") {
      setError(answer);
      setRemoving(null);
      return;
    }
    setPlan(answer);
  };

  const confirmRemoval = async (characterId: string) => {
    setBusy(true);
    const failure = await removeCharacter(characterId);
    setBusy(false);
    setError(failure);
    setRemoving(null);
    setPlan(null);
    await onChanged();
  };

  const move = async (
    characterId: string,
    worldId: string,
    livesThere: boolean,
  ) => {
    setBusy(true);
    const failure = await setCharacterWorld(characterId, worldId, livesThere);
    setBusy(false);
    setError(failure);
    await onChanged();
  };

  /**
   * The machine this edit is on: what they just picked, or what their Brain already implies.
   *
   * Intent wins over derivation while an edit is open — somebody who selected `This PC` and has
   * not yet chosen what runs there means `This PC`, and recomputing from an empty provider would
   * snap the first dropdown back to nothing the instant they used it.
   */
  const machine = pickedMachine ?? (edit ? machineFor(edit, providers) : null);

  return (
    <section className="pnl bay">
      <div className="bay__head">
        <div>
          <div className="bay__heading">
            <span className="lx__gem" aria-hidden />
            <h2>CREW ROSTER</h2>
          </div>
          <p className="bay__sub">
            {crewLine(view.characters.length)} Edit them once — they are the
            same people in every World they live in.
          </p>
        </div>
        {/*
          The one action an empty vault offers.

          Not "start with a crew of four": characters belong to the user (ADR-0023), and handing
          somebody four pre-made people would be the same act as generating a face for somebody
          who has not chosen one.
        */}
        <button
          type="button"
          className="btn btn--ghost"
          onClick={() => setHiring((open) => !open)}
          aria-expanded={hiring}
        >
          {hiring ? "NEVER MIND" : "ADD A CHARACTER"}
        </button>
      </div>
      <div className="bay__rule" />

      {view.definitionProblems.map((problem, i) => (
        <p key={i} className="notice notice--warn">
          {problem}
        </p>
      ))}

      {error && <p className="notice notice--warn">{error}</p>}

      {hiring && (
        <Hire
          archetypes={view.vocabulary.archetypes}
          onCancel={() => setHiring(false)}
          onHired={(id) => {
            setHiring(false);
            // Remembered rather than opened here: the roster has not been re-read yet, and the
            // editor opens on a `CharacterSummary` that does not exist on this render.
            setOpenOn(id);
            void onChanged();
          }}
        />
      )}

      {view.characters.length === 0 ? (
        <p className="notice">
          Nobody in the crew yet. Make the first one — they are yours, and they
          travel with you into every World.
        </p>
      ) : (
        <ul className="cast">
          {view.characters.map((character) => {
            const editing = edit?.id === character.id;
            return (
              <li
                key={character.id}
                className={`cc${editing ? " cc--open" : ""}`}
              >
                <Portrait mark={character.portrait} name={character.name} />

                {/*
                  **The action leaves the heading so the row can be a row** (owner, 2026-09-06).

                  It sat inside `cc__head`, pushed right by `space-between` — which is the right
                  place in a narrow stack and pins it to the middle of a wide one. As a column of
                  the row it is at the row's right edge at every width, and the body underneath is
                  free to become *who they are* beside *where they live*.
                */}
                <div className="cc__acts">
                  <button
                    type="button"
                    className="btn btn--mini"
                    onClick={() => open(character)}
                  >
                    {editing ? "Close" : "Edit"}
                  </button>
                </div>

                <div className="cc__body">
                  <div className="cc__head">
                    <div>
                      <h3 className="cc__name">{character.name}</h3>
                      <p className="cc__role">{character.role}</p>
                    </div>
                  </div>

                  <p className="cc__meta">
                    {ARCHETYPE_LABEL[character.archetype] ??
                      titleise(character.archetype)}{" "}
                    · {character.brain ?? "no brain — cannot think"}
                  </p>

                  {/*
                    The roster. Editing it is its own command, applied immediately, so moving
                    somebody never depends on remembering to save a form.
                  */}
                  <div className="cc__worlds">
                    {view.worlds.length === 0 ? (
                      <span className="cc__hint">
                        No Worlds installed to live in.
                      </span>
                    ) : (
                      view.worlds.map((world) => {
                        const lives = character.worlds.includes(world.id);
                        return (
                          <label
                            key={world.id}
                            className={`chip${lives ? " chip--on" : ""}`}
                            title={`${lives ? "Remove" : "Add"} ${character.name} ${
                              lives ? "from" : "to"
                            } ${world.name}`}
                          >
                            <input
                              type="checkbox"
                              checked={lives}
                              disabled={busy}
                              onChange={(e) =>
                                void move(
                                  character.id,
                                  world.id,
                                  e.target.checked,
                                )
                              }
                            />
                            {world.name}
                          </label>
                        );
                      })
                    )}
                  </div>

                  {character.worlds.length === 0 && (
                    <p className="cc__hint">
                      Lives nowhere yet — pick a World above and they will be
                      there when you enter it.
                    </p>
                  )}

                  {editing && edit && (
                    <form
                      className="cedit"
                      onSubmit={(event) => {
                        event.preventDefault();
                        void save();
                      }}
                    >
                      <label className="cedit__field">
                        <span>Name</span>
                        <input
                          value={edit.name}
                          autoFocus
                          onChange={(e) =>
                            setEdit({ ...edit, name: e.target.value })
                          }
                        />
                      </label>

                      <label className="cedit__field">
                        <span>Archetype</span>
                        {/* Options come from the Engine, never from a list kept here. */}
                        <select
                          value={edit.archetype}
                          onChange={(e) =>
                            setEdit({ ...edit, archetype: e.target.value })
                          }
                        >
                          {view.vocabulary.archetypes.map((id) => (
                            <option key={id} value={id}>
                              {ARCHETYPE_LABEL[id] ?? titleise(id)}
                            </option>
                          ))}
                        </select>
                      </label>

                      <label className="cedit__field cedit__field--wide">
                        {/* `role` in the domain; "Description" is what it means to a person.
                            A presentation word, and it stays one (Internal First). */}
                        <span>Description</span>
                        <input
                          value={edit.role}
                          onChange={(e) =>
                            setEdit({ ...edit, role: e.target.value })
                          }
                        />
                      </label>

                      {/*
                        **How they sound** (Phase 15). Beside the name and the description
                        because it is identity, not machinery: somebody who sounds like this
                        still sounds like this after Piper is replaced (ADR-0026's one test), and
                        it travels with them into every World.

                        The list is **voices, never files**. Today a Piper voice is one file and
                        the two coincide; one Kokoro model holds fifty-four, and a file list
                        would show that as a single row called `kokoro-v1_0.pth`.

                        With nothing installed the control keeps its frame, loses its light and
                        names where voices come from — never an empty dropdown, which reads as
                        *you have none* rather than *nobody has installed one yet*.
                      */}
                      <label className="cedit__field cedit__field--wide">
                        <span>Voice</span>
                        {voices.length === 0 ? (
                          <i className="cedit__hint">
                            No voices installed. WORKSHOP → VOICES WORKSHOP has 175 of them, and
                            CREATIONS is where Piper itself is installed. Until then nobody
                            speaks, and everything else about this character works.
                          </i>
                        ) : (
                          <select
                            value={edit.speaksWith ?? ""}
                            onChange={(e) =>
                              setEdit({ ...edit, speaksWith: e.target.value })
                            }
                          >
                            {/* Silence, said on purpose. It is the ordinary case and it is
                                first, because most characters have no voice and choosing one
                                for somebody is a claim about who they are. */}
                            <option value="">Silent — does not speak</option>
                            {voices.map((voice) => (
                              <option key={voice.name} value={voice.name}>
                                {voice.what
                                  ? `${voice.name} — ${voice.what}`
                                  : voice.name}
                              </option>
                            ))}
                            {/*
                              A voice this character asks for that is not installed here.
                              **Kept and shown**, never silently dropped: it is a preference the
                              machine cannot honour yet, and installing it should make them sound
                              right again rather than making somebody choose twice.
                            */}
                            {edit.speaksWith &&
                              !voices.some((v) => v.name === edit.speaksWith) && (
                                <option value={edit.speaksWith}>
                                  {edit.speaksWith} — not installed here
                                </option>
                              )}
                          </select>
                        )}
                      </label>

                      {/*
                        **The colour on top of that voice** (Phase 15, RVC).

                        Offered only to somebody who already speaks: a timbre with no Piper
                        voice under it has nothing to recolour, and a control that reaches
                        nothing is worse than a control that is absent.

                        The pitch lives *inside* this field rather than beside it, because the
                        two are meaningless apart -- the owner's own measurement is the reason
                        it exists at all: the Pato Donald model was inconclusive at 0 and
                        unmistakable at +12. A model at the wrong pitch is the same voice half
                        an octave out, which everybody hears as a bad model.
                      */}
                      {edit.speaksWith ? (
                        <label className="cedit__field cedit__field--wide">
                          <span>Sounds like</span>
                          {forge && !forge.canSpeak ? (
                            <i className="cedit__hint">
                              {forge.nextStep ??
                                "Epoch cannot speak RVC voices on this machine yet."}{" "}
                              CREATIONS &rarr; SPEAKING is where that is prepared.
                            </i>
                          ) : (
                            <>
                              <select
                                value={edit.soundsLike?.voice ?? ""}
                                onChange={(e) =>
                                  setEdit({
                                    ...edit,
                                    soundsLike: {
                                      voice: e.target.value,
                                      semitones: edit.soundsLike?.semitones ?? 0,
                                      speaker: edit.soundsLike?.speaker ?? 0,
                                    },
                                  })
                                }
                              >
                                {/* The ordinary case, and first. */}
                                <option value="">Their own voice</option>
                                {timbres.map((timbre) => (
                                  <option key={timbre.name} value={timbre.name}>
                                    {timbre.what
                                      ? `${timbre.name} — ${timbre.what}`
                                      : timbre.name}
                                  </option>
                                ))}
                                {/* Kept and shown, never dropped: a preference the machine
                                    cannot honour yet. */}
                                {edit.soundsLike?.voice &&
                                  !timbres.some(
                                    (t) => t.name === edit.soundsLike?.voice,
                                  ) && (
                                    <option value={edit.soundsLike.voice}>
                                      {edit.soundsLike.voice} — not converted here
                                    </option>
                                  )}
                              </select>
                              {edit.soundsLike?.voice ? (
                                <label className="cedit__pitch">
                                  {/* **Both ends written down.** An unlabelled range control
                                      is one somebody has to guess at, and this one was read
                                      backwards once already on the LoRA row. */}
                                  <span>Lower</span>
                                  <input
                                    type="range"
                                    min={-24}
                                    max={24}
                                    step={1}
                                    value={edit.soundsLike.semitones}
                                    onChange={(e) =>
                                      setEdit({
                                        ...edit,
                                        soundsLike: {
                                          voice: edit.soundsLike?.voice ?? "",
                                          semitones: Number(e.target.value),
                                          speaker: edit.soundsLike?.speaker ?? 0,
                                        },
                                      })
                                    }
                                  />
                                  <span>Higher</span>
                                  <b>
                                    {edit.soundsLike.semitones > 0 ? "+" : ""}
                                    {edit.soundsLike.semitones}
                                  </b>
                                </label>
                              ) : null}
                              {timbres.length === 0 ? (
                                <i className="cedit__hint">
                                  No RVC voices converted here yet. A `.pth` on the TIMBRES
                                  shelf becomes one; CREATIONS &rarr; SPEAKING converts it.
                                </i>
                              ) : null}
                            </>
                          )}
                        </label>
                      ) : null}

                      {/*
                        Home used to be picked here, from the five places this build knew.

                        It is gone rather than moved (ADR-0028): where somebody lives is a fact
                        about them *in one World*, and this form edits the character — the same
                        file, carried into every World unchanged. A single Home here would have
                        to mean the same building in all of them, and `building_1` is a
                        different building in each. The World Editor decides it, per World.
                      */}

                      {/*
                        Who thinks for them. The model is infrastructure and the Character is
                        the product — this is precisely the field that can be swapped without
                        Mage becoming somebody else.

                        Deliberately not defaulted: on this machine two locally installed models
                        differed by minutes on a cold start, and choosing for the user would
                        surprise them with that cost.
                      */}
                      {/*
                        **Provider · Brain · Model** — the device, the program that runs the
                        model, the model (owner, 2026-08-20).

                        These are the words on screen. Underneath, `Provider` is a machine name
                        derived from an endpoint and `Brain` is a `ProviderStatus` id — the
                        domain keeps its own vocabulary, and this is the one place the two are
                        mapped. The poetry never leaks inward.

                        ## Why this is three controls now and was one before

                        The single control was right while a machine offered one program: picking
                        `MacBook Pro ▸ Ollama` picked both, and the objection to splitting it was
                        real — a separate machine dropdown lets somebody hold a machine and a
                        Service that is not on it.

                        That objection is answered rather than ignored. **The second list is
                        derived from the first**, so the invalid pair is not refused, it is
                        unsayable. What changed is that a machine now genuinely offers several:
                        the MacBook serves Ollama, LM Studio and llama.cpp at once, measured, and
                        one flat list of `Studio Mac`, `Studio Mac · LM Studio` said nothing
                        about which of those was a computer and which was a program on it.
                      */}
                      <label className="cedit__field">
                        <span>Provider</span>
                        <select
                          value={machine ?? ""}
                          onChange={(e) => {
                            const wanted = e.target.value || null;
                            setPickedMachine(wanted);
                            const onIt = brainsOn(wanted, providers, agents);
                            // **One Brain is not a choice.** A machine offering exactly one
                            // program should not make somebody confirm it — that was the whole
                            // strength of the single control, and it is kept.
                            const only = onIt.length === 1 ? onIt[0] : undefined;
                            setEdit({
                              ...edit,
                              agent: only?.kind === "agent" ? only.id : null,
                              provider: only?.kind === "model" ? only.id : null,
                              // A model belongs to a Brain. Moving machines cannot leave the old
                              // machine's model behind.
                              model: null,
                            });
                          }}
                        >
                          <option value="">&mdash; nowhere yet &mdash;</option>
                          {machinesWith(providers, agents).map((one) => (
                            <option key={one} value={one}>
                              {one}
                            </option>
                          ))}
                          {/*
                            The machine they are already on, when nothing currently offers it —
                            a Service switched off, a machine unpaired. Dropping it would move
                            somebody's character while they were renaming them.
                          */}
                          {machine && !machinesWith(providers, agents).includes(machine) && (
                            <option value={machine}>{machine} &mdash; not reachable</option>
                          )}
                        </select>
                        <i className="cedit__hint">{whereItRuns(edit, providers)}</i>
                      </label>

                      {/*
                        THE CHOICE ITSELF, before the thing it chooses between.

                        `Brain` is `Model` or `Agent` (ADR-0027), and this panel used to draw the
                        word only when a character *already* was one — set by hand in the vault,
                        because nothing here could set it. The decision existed in the domain and
                        nowhere on screen.

                        Which field is present **is** the choice: an agent sends no provider, a
                        model sends no agent. A third `kind` field beside them would be one more
                        thing that could disagree with both.
                      */}
                      <label className="cedit__field">
                        <span>Brain</span>
                        <select
                          disabled={!machine}
                          value={
                            edit.agent
                              ? `agent${MIND_SEP}${edit.agent}`
                              : edit.provider
                                ? `model${MIND_SEP}${edit.provider}`
                                : ""
                          }
                          onChange={(e) => {
                            const [kind, id] = e.target.value.split(MIND_SEP);
                            setEdit({
                              ...edit,
                              // Exclusive, and cleared on the way through rather than left for
                              // the Engine to untangle: a form holding both would be a form
                              // that has to decide which one it meant.
                              agent: kind === "agent" ? id : null,
                              provider: kind === "model" ? id : null,
                              // **A model belongs to a Brain.** Changing which program runs
                              // it cannot leave the old one's model behind — that pairing is
                              // exactly what a cascade exists to make unexpressible.
                              model: null,
                            });
                          }}
                        >
                          <option value="">
                            {machine
                              ? "\u2014 nothing thinks for them \u2014"
                              : "\u2014 pick a machine first \u2014"}
                          </option>
                          {brainsOn(machine, providers, agents).map((brain) => (
                            <option
                              key={`${brain.kind}${brain.id}`}
                              value={`${brain.kind}${MIND_SEP}${brain.id}`}
                              disabled={!brain.installed}
                            >
                              {brain.name}
                              {brain.kind === "agent"
                                ? brain.installed
                                  ? " \u2014 thinks and works"
                                  : " \u2014 not installed"
                                : brain.online
                                  ? " \u2014 thinks"
                                  : " — not running"}
                            </option>
                          ))}
                        </select>
                        {/*
                          **The models are there; the server is not.** Measured on this machine:
                          Ollama switched off, `gemma4`, `gpt-oss` and `qwen3` still on disk, and
                          the Model list below simply empty. Epoch was not lying — it was silent
                          in the one place somebody was looking, which is the same failure as a
                          gauge nobody can explain.

                          It names where the fix is rather than doing it here: starting a runtime
                          is Connections' job and it already has the button.
                        */}
                        {edit.provider &&
                          providers.some(
                            (p) => p.id === edit.provider && !p.online,
                          ) && (
                            <i className="cedit__hint">
                              That one is not running, so it lists no models — anything
                              already downloaded is still there. Start it from
                              CONNECTIONS.
                            </i>
                          )}
                        {/*
                          Cold rather than absent, and it names what to do. A machine without
                          Claude Code should be told so, not simply offered a shorter list.
                        */}
                        {/*
                          Cold rather than absent, and it names what to do. A machine without
                          Claude Code should be told so, not simply offered a shorter list.
                        */}
                        {agents.some((a) => !a.installed) && (
                          <i className="cedit__hint">
                            {agents
                              .filter((a) => !a.installed)
                              .map(
                                (a) =>
                                  `${a.name}: ${a.note ?? "not installed"}`,
                              )
                              .join(" · ")}
                          </i>
                        )}
                        {/*
                          Installed and signed out is a *different* problem from not installed,
                          and it has a different fix. It arrived as a character apparently saying
                          "Not logged in · Please run /login" — a sentence Epoch could neither
                          explain nor act on, because nothing here had ever asked the question.

                          Now it is asked (`auth status`), and the answer comes with the button
                          that fixes it. Epoch opens the agent's own sign-in and stops there: no
                          password field, nothing read back, no token kept. The credential is
                          between the user and the agent — the only thing removed is having to
                          know a terminal was needed.
                        */}
                        {(() => {
                          const chosen = agents.find(
                            (a) => a.id === edit.agent,
                          );
                          if (!chosen?.installed) return null;
                          if (chosen.signedIn === false) {
                            return (
                              <div className="cedit__signin">
                                <b>Not signed in.</b>
                                <button
                                  type="button"
                                  className="epbtn"
                                  onClick={() => {
                                    void signInAgent(chosen.id).catch(
                                      (why: unknown) => setError(String(why)),
                                    );
                                  }}
                                >
                                  Sign in
                                </button>
                                <button
                                  type="button"
                                  className="epbtn"
                                  onClick={remeasure}
                                >
                                  Check again
                                </button>
                                <i>
                                  Opens {chosen.name}&rsquo;s own sign-in. Epoch
                                  never sees the credential.
                                </i>
                              </div>
                            );
                          }
                          if (chosen.signedIn && chosen.account) {
                            return (
                              <i className="cedit__hint">
                                Signed in as {chosen.account}.
                              </i>
                            );
                          }
                          return null;
                        })()}
                      </label>

                      {edit.agent ? (
                        /*
                          An agent's model is **its** vocabulary — `opus`, not `gemma4:12b`. A
                          dropdown here would mean Epoch keeping a list of somebody else's model
                          names, and being wrong the day they add one. Empty means the agent's
                          own choice stands, which is what most people want.
                        */
                        <label className="cedit__field">
                          <span>Model — the agent&rsquo;s own name for it</span>
                          {/*
                            Suggested, not enumerated — but drawn as this surface draws things.

                            This was a `<datalist>`, which is both a list and a free field and is
                            exactly right in the abstract. It renders as the *browser's* popup:
                            system font, system colours, system chrome, in the middle of a panel
                            that is none of those. An immersion leak, and the kind that cannot be
                            styled away — a datalist popup takes no CSS.

                            So: a `<select>` that looks like every other control here, with one
                            option that opens a text field. The list stays open-ended, and the
                            open-endedness costs one click instead of the whole look.

                            **Aliases rather than versions**, verified against the CLI: it takes
                            `opus` or `claude-opus-5`, and an alias always means the latest. A
                            list of pinned versions goes stale; a list of aliases does not.
                          */}
                          {namingModel ||
                          (edit.model &&
                            !agentModels_.some(([id]) => id === edit.model)) ? (
                            <input
                              autoFocus
                              value={edit.model ?? ""}
                              placeholder="whatever they call it"
                              onChange={(e) =>
                                setEdit({
                                  ...edit,
                                  model: e.target.value || null,
                                })
                              }
                              onBlur={() => {
                                if (!edit.model) setNamingModel(false);
                              }}
                            />
                          ) : (
                            <select
                              value={edit.model ?? ""}
                              onChange={(e) => {
                                if (e.target.value === NAME_IT) {
                                  setNamingModel(true);
                                  return;
                                }
                                setNamingModel(false);
                                setEdit({
                                  ...edit,
                                  model: e.target.value || null,
                                });
                              }}
                            >
                              <option value="">Its own choice</option>
                              {agentModels_.map(([id, label]) => (
                                <option key={id} value={id}>
                                  {label}
                                </option>
                              ))}
                              <option value={NAME_IT}>Name it myself…</option>
                            </select>
                          )}
                          <i className="cedit__hint">
                            It owns its loop, so its autonomy is its own — Epoch
                            passes this World&rsquo;s mode through when it
                            starts it, and gates only Epoch&rsquo;s tools.
                          </i>
                        </label>
                      ) : (
                        <label className="cedit__field">
                          <span>Model</span>
                          <select
                            value={mindValue(edit.provider, edit.model)}
                            onChange={(e) => {
                              const [provider, ...rest] =
                                e.target.value.split(MIND_SEP);
                              setEdit({
                                ...edit,
                                provider: provider || null,
                                model: rest.join(MIND_SEP) || null,
                              });
                            }}
                          >
                            <option value="">— pick one —</option>

                            {/*
                              **Only this Service's models.** They used to be one flat list of
                              every provider's models with the provider in brackets, which meant
                              the pairing lived in the option rather than in the choice — and
                              moving somebody to another machine silently kept a model that
                              machine may not have. An impossible pair is now unexpressible
                              rather than merely refused.
                            */}
                            {providers
                              .filter((p) => p.id === edit.provider)
                              .flatMap((p) =>
                                p.models.map((m) => (
                                  <option
                                    key={`${p.id} ${m}`}
                                    value={mindValue(p.id, m)}
                                  >
                                    {m}
                                  </option>
                                )),
                              )}

                            {/*
                            Their current choice, when the provider that offers it is not
                            reachable right now. Dropping it would silently un-assign a model
                            because Ollama happened to be closed while they renamed somebody.
                          */}
                            {edit.provider &&
                              edit.model &&
                              !providers.some(
                                (p) =>
                                  p.id === edit.provider &&
                                  p.models.includes(edit.model!),
                              ) && (
                                <option
                                  value={mindValue(edit.provider, edit.model)}
                                >
                                  {edit.model} ({edit.provider} · not reachable)
                                </option>
                              )}
                          </select>
                          {/*
                            **What this model says about itself**, beside the field where it is
                            chosen — because that is the moment the answer changes a decision.

                            Every backend publishes this and Epoch read none of it until somebody
                            asked whether an agent could see. Measured rather than listed:
                            `gemma4:12b` says vision *and audio*, `gemma4:26b` says vision
                            without audio, `qwen3:14b` says neither. Nobody could have written
                            that from memory, which is exactly why it should not be remembered.

                            Absent when the backend did not answer. **Unasked is not "cannot"** —
                            drawing a row of dark capabilities for a model that simply did not
                            reply would be an invented reading, and this whole field exists to
                            stop those.
                          */}
                          {surface.can && (
                            <span className="cedit__declares">
                              {(
                                [
                                  ["sees", "vision"],
                                  ["hears", "audio"],
                                  ["usesTools", "tools"],
                                  ["thinks", "thinking"],
                                ] as const
                              ).map(([key, label]) => (
                                <i
                                  key={key}
                                  className={`declares${surface.can![key] ? " declares--on" : ""}`}
                                  title={
                                    surface.can![key]
                                      ? `This model declares ${label}.`
                                      : surface.can![key] === null
                                        ? `This runtime does not say whether this model has ${label}, which is not the same as saying it does not.`
                                        : `This model does not declare ${label}.`
                                  }
                                >
                                  {label}
                                </i>
                              ))}
                            </span>
                          )}
                          {/*
                            The one declaration that changes what Epoch does, said where the
                            consequence is chosen. A model handed tools it cannot call does not
                            fail — it ignores them, and the character answers as though they were
                            never there. The user would see capabilities that do nothing.
                          */}
                          {/*
                            **Unasked, said out loud.** A model served by llama.cpp or LM Studio
                            publishes an id and nothing else — measured: `google/gemma-4-e4b`
                            answers `can: None` where `gemma4:12b` on Ollama answers vision,
                            audio, tools and thinking. Drawing nothing there is the same silence
                            that made a lent machine's models look blind.
                          */}
                          {edit.provider && edit.model && !surface.can && (
                            <i className="cedit__hint">
                              This runtime does not describe its models, so Epoch
                              cannot say what this one can do — which is not the
                              same as saying it cannot.
                            </i>
                          )}
                          {/*
                            **Only on a definite no.** This used to fire on `false`, and
                            llama.cpp and LM Studio filled that field with `false` because they
                            publish nothing about tools — so a model that calls them perfectly
                            well was shown this sentence, given no capabilities, and then
                            truthfully told the user it had none. `null` is unasked and says
                            nothing here.
                          */}
                          {surface.can && surface.can.usesTools === false && (
                            <i className="cedit__hint cc__hint--warn">
                              This model does not declare that it can use tools,
                              so Epoch will not give it any. The requests below
                              are kept and apply again on a model that can.
                            </i>
                          )}
                        </label>
                      )}

                      {/*
                        Artwork is applied immediately, not on SAVE — the same rule the roster
                        follows. An import writes a file; pretending it is pending until a
                        button is pressed would mean the vault and the form disagree in the
                        meantime.
                      */}
                      <div className="cedit__field cedit__field--wide art__pair">
                        <CharacterArtSlot
                          label="Sprite"
                          portrait={
                            <Portrait
                              mark={character.spriteMark}
                              name={character.name}
                            />
                          }
                          hasArt={character.spriteMark !== null}
                          hint="What walks around the World."
                          fallback="No sprite. The World draws a visible stand-in."
                          busy={busy}
                          onChoose={(data) => art(character.id, "sprite", data)}
                          onClear={() => art(character.id, "sprite", null)}
                        />
                        <CharacterArtSlot
                          label="Icon"
                          portrait={
                            <Portrait
                              mark={character.iconMark}
                              name={character.name}
                            />
                          }
                          hasArt={character.iconMark !== null}
                          hint="What identifies them in a conversation."
                          fallback={
                            character.spriteMark
                              ? "No icon of their own — the sprite stands in."
                              : "No icon, and no sprite to stand in."
                          }
                          busy={busy}
                          onChoose={(data) => art(character.id, "icon", data)}
                          onClear={() => art(character.id, "icon", null)}
                        />
                      </div>

                      {/*
                        Under the two pictures, because that is what they are under: a still
                        drawing is what the World shows for every action nobody has drawn, and
                        these are what it shows instead when somebody has.

                        Each preview **plays**. A cut is four numbers and four numbers cannot be
                        checked by reading them — a sheet declared 4x4 that is really 4x3 looks
                        perfectly reasonable in a form.
                      */}
                      <details className="cedit__field cedit__field--wide anim__door">
                        <summary>Animations</summary>
                        <div className="anim__grid">
                          {ANIMATIONS.map(({ action, label, hint }) => (
                            <AnimationSlot
                              key={action}
                              label={label}
                              hint={hint}
                              mark={character.actionMarks[action] ?? null}
                              busy={busy}
                              onChoose={(data, cut) =>
                                art(character.id, action, data, cut)
                              }
                              onRecut={(cut) => recut(character.id, action, cut)}
                              onClear={() => art(character.id, action, null)}
                            />
                          ))}
                        </div>
                      </details>

                      <label className="cedit__field cedit__field--wide">
                        {/* `prompt` in the domain. Naming it for what it is beats naming it
                            for what it produces — this is the text the model receives. */}
                        <span>Character prompt</span>
                        <textarea
                          rows={5}
                          value={edit.prompt}
                          onChange={(e) =>
                            setEdit({ ...edit, prompt: e.target.value })
                          }
                        />
                      </label>

                      <div className="cedit__field cedit__field--wide">
                        <span>Routine</span>
                        {/*
                          Behaviour, not animation. The Engine holds the activity; the World
                          decides what it looks like. A character is never frozen, so the
                          Engine refuses an empty routine rather than rendering a statue.
                        */}
                        <ul className="routine">
                          {edit.routine.map((step, i) => (
                            <li key={i}>
                              <input
                                aria-label={`Activity ${i + 1}`}
                                value={step.activity}
                                onChange={(e) =>
                                  setEdit({
                                    ...edit,
                                    routine: edit.routine.map((s, j) =>
                                      j === i
                                        ? { ...s, activity: e.target.value }
                                        : s,
                                    ),
                                  })
                                }
                              />
                              <input
                                type="number"
                                min={1}
                                aria-label={`Seconds for step ${i + 1}`}
                                value={step.seconds}
                                onChange={(e) =>
                                  setEdit({
                                    ...edit,
                                    routine: edit.routine.map((s, j) =>
                                      j === i
                                        ? {
                                            ...s,
                                            seconds:
                                              Number(e.target.value) || 1,
                                          }
                                        : s,
                                    ),
                                  })
                                }
                              />
                              <span className="routine__unit">s</span>
                              <button
                                type="button"
                                className="btn btn--mini"
                                aria-label={`Remove step ${i + 1}`}
                                onClick={() =>
                                  setEdit({
                                    ...edit,
                                    routine: edit.routine.filter(
                                      (_, j) => j !== i,
                                    ),
                                  })
                                }
                              >
                                ✕
                              </button>
                            </li>
                          ))}
                        </ul>
                        <button
                          type="button"
                          className="btn btn--mini"
                          onClick={() =>
                            setEdit({
                              ...edit,
                              routine: [
                                ...edit.routine,
                                { activity: "", seconds: 8 },
                              ],
                            })
                          }
                        >
                          Add step
                        </button>
                      </div>

                      {/*
                        Advanced. Collapsed, because the default is that nobody touches it —
                        great defaults beat endless configuration, and every one of these is
                        unset until somebody chooses.

                        Two kinds of setting, and the split is the whole of ADR-0026. The
                        *canonical* parameters survive changing the engine, so this form is
                        allowed to know them by name. Everything below them is declared by the
                        Provider at runtime and drawn generically — adding a backend adds
                        nothing to this file, which is the only way four of them can arrive
                        without four edits to a surface that should never have heard of any.
                      */}
                      <details className="cedit__field cedit__field--wide cedit__advanced">
                        <summary>Advanced</summary>

                        <div className="cedit__grid">
                          <BoundedControl
                            label="Temperature"
                            value={edit.parameters.temperature ?? null}
                            min={0}
                            max={2}
                            step={0.05}
                            onChange={(temperature) =>
                              setEdit({
                                ...edit,
                                parameters: { ...edit.parameters, temperature },
                              })
                            }
                          />

                          <BoundedControl
                            label="Top P"
                            value={edit.parameters.topP ?? null}
                            min={0}
                            max={1}
                            step={0.05}
                            onChange={(topP) =>
                              setEdit({
                                ...edit,
                                parameters: { ...edit.parameters, topP },
                              })
                            }
                          />

                          <label className="cedit__field">
                            <span>Reasoning</span>
                            {/*
                              Offered for every provider, local included. A local runtime has a
                              think flag and a hosted one has effort levels — qwen3, running on
                              this machine, is a thinking model. Which *models* honour it is a
                              question only the Provider can answer, and it will, once it can
                              declare its own surface.
                            */}
                            <select
                              value={edit.parameters.reasoning ?? ""}
                              onChange={(e) =>
                                setEdit({
                                  ...edit,
                                  parameters: {
                                    ...edit.parameters,
                                    reasoning: e.target.value || null,
                                  },
                                })
                              }
                            >
                              <option value="">— provider default —</option>
                              {view.vocabulary.reasoning.map((id) => (
                                <option key={id} value={id}>
                                  {titleise(id)}
                                </option>
                              ))}
                            </select>
                          </label>
                        </div>

                        {/*
                          **The window is MODELS' and this is the view of it** (ADR-0026's
                          amendment, 2026-08-31). What stood here was an editable `Context
                          tokens` box, and it was unsatisfiable in two directions at once:
                          `llama-server` takes `--ctx-size` when it spawns the child that holds
                          the model, so two characters on one Brain cannot have different
                          windows; and `131072` depends on the card, so it is a number nobody
                          can honour on a smaller one.

                          What is left is a reading and a door. `RuntimeFromModels` never
                          writes — there is no command it could call — and the policy below it
                          is the half of the old field that really is the character's.
                        */}
                        {edit.provider && (
                          <>
                            <RuntimeFromModels
                              inherited={inherited}
                              onConfigure={onConfigureInModels}
                            />

                            <ContextPolicyChoice
                              value={edit.parameters.contextPolicy ?? null}
                              onChange={(contextPolicy) =>
                                setEdit({
                                  ...edit,
                                  parameters: {
                                    ...edit.parameters,
                                    contextPolicy,
                                  },
                                })
                              }
                            />
                          </>
                        )}

                        <div className="cedit__field cedit__field--wide">
                          <span>Requested capabilities</span>
                          {/*
                            Tick-boxes, because nobody can spell from memory a name they have
                            never seen. The list comes from the Engine — a *suggestion* list,
                            not a taxonomy: a request authored by hand that is not on it is
                            kept and shown, so an older build cannot destroy a newer file.
                          */}
                          {/*
                            Undecided is a state, and it is not the same as having ticked
                            everything: it means *whatever exists at the time*, and it survives
                            a server being briefly unreachable. Saying so, because a screen full
                            of ticked boxes looks like a decision somebody made.
                          */}
                          {edit.requestedCapabilities === null && (
                            <p className="cc__hint">
                              Nobody has chosen yet, so they get everything
                              available whenever they work. Ticking anything
                              below decides it.
                            </p>
                          )}
                          <div className="caps">
                            {[
                              ...view.vocabulary.capabilities,
                              ...(edit.requestedCapabilities ?? []).filter(
                                (r) =>
                                  !view.vocabulary.capabilities.includes(r),
                              ),
                            ].map((id) => {
                              // Undecided shows every box ticked, because that is what it
                              // resolves to — but the *file* still says undecided until one is
                              // touched.
                              const wanted =
                                edit.requestedCapabilities === null ||
                                edit.requestedCapabilities.includes(id);
                              const group = view.vocabulary.groups.find(
                                (g) => g.id === id,
                              );
                              /*
                                A tool that belongs to a connected source is built, even though
                                it is not offered as a box of its own. It can still reach this
                                list: granting one mid-turn writes that single id, which is the
                                only way a partial grant is expressible — and dimming it here
                                would say a working tool does not exist.
                              */
                              const built =
                                Boolean(group) ||
                                view.vocabulary.built.includes(id) ||
                                view.vocabulary.groups.some((g) =>
                                  g.tools.includes(id),
                                );
                              return (
                                <label
                                  key={id}
                                  className={`chip${wanted ? " chip--on" : ""}${
                                    built ? "" : " chip--pending"
                                  }`}
                                  title={
                                    group && !group.answering
                                      ? `${group.label} is configured but not answering. The request is kept — it works again when the server does.`
                                      : group
                                        ? `Everything ${group.label} offers — ${group.tools.length} right now, and whatever it offers later.`
                                        : built
                                          ? "Available in any World with a project root."
                                          : "Not built yet. The request is kept and will select it on the day it exists."
                                  }
                                >
                                  <input
                                    type="checkbox"
                                    checked={wanted}
                                    onChange={(e) => {
                                      // The first tick is also the moment somebody decided, so
                                      // an undecided character starts from everything rather
                                      // than from nothing.
                                      const from =
                                        edit.requestedCapabilities ??
                                        view.vocabulary.built;
                                      // Ticking or unticking a source settles everything inside
                                      // it. A source sitting beside three of its own tool ids
                                      // is two answers to one question.
                                      const without = from.filter(
                                        (r) =>
                                          r !== id && !group?.tools.includes(r),
                                      );
                                      setEdit({
                                        ...edit,
                                        requestedCapabilities: e.target.checked
                                          ? [...without, id]
                                          : without,
                                      });
                                    }}
                                  />
                                  {group
                                    ? group.answering
                                      ? `${group.label} · ${group.tools.length}`
                                      : `${group.label} · not answering`
                                    : titleise(id)}
                                </label>
                              );
                            })}
                          </div>
                          {/*
                            What a source's box actually grants. Native <details>, so seeing
                            inside costs no state and works from the keyboard — and it is worth
                            seeing: an outside tool is registered with every effect and no
                            reversal, because MCP declares neither (ADR-0008).
                          */}
                          {view.vocabulary.groups.map((group) => (
                            <details key={group.id} className="caps__inside">
                              <summary>
                                {group.label} offers {group.tools.length}
                              </summary>
                              <ul>
                                {group.tools.map((tool) => (
                                  <li key={tool}>{tool}</li>
                                ))}
                              </ul>
                            </details>
                          ))}
                          {/*
                            **Who these actually govern, said out loud.**

                            For a model they are the turn's tool list: `on_the_table` resolves
                            the request and Trust filters it, so an unticked box is a tool that
                            is not offered.

                            For an agent they are not. Epoch's door deliberately publishes the
                            whole registry — hiding a tool would answer "may they?" early, with
                            less information and permanently, and an agent that never saw
                            `write_file` cannot ask for it, so the user never gets to say yes
                            (`serve.rs`). Trust judges each call instead, with the arguments in
                            hand.

                            That is a good arrangement and it was invisible: the same tick-boxes
                            were drawn for both brains, and for one of them they did not decide
                            anything. It is the same defect `CLAUDE.md` recorded on 2026-08-07
                            about the Manual dropdown — a gauge that lies is worse than none —
                            and it was still here one field over.
                          */}
                          {edit.agent && (
                            <p className="cc__hint cc__hint--warn">
                              {edit.agent} decides its own tools, and
                              Epoch&rsquo;s own tools stay open to it so that
                              you can be asked. These are what they
                              <b> want</b>; what they may do is decided per call
                              by the mode.
                            </p>
                          )}
                          <p className="cc__hint">
                            What they ask to use. Reading works in any World
                            with a project root; the dimmed ones are not built
                            yet — the request is kept and will select them on
                            the day they exist.
                          </p>
                        </div>

                        {/*
                          Ways of working, and what each one will want.

                          Below the capabilities rather than above them, which is the second
                          arrangement tried. Reading a Skill first would explain *why* somebody
                          needs Playwright — but placed there it landed between Reasoning and the
                          tick-boxes, among numeric tuning, and read as one more knob. A way of
                          working is not a knob. Sitting under the capabilities it reads as what
                          it is: what this person does with what they have.

                          The whole reason this shows `requires` at all is the warning. A Skill
                          that names a capability its holder does not have will fail on its third
                          step, in a model's words, far from the screen where it was assigned —
                          so it is said here, at the moment of assigning, and said as what it is:
                          a request the Skill makes, never a grant it carries.
                        */}
                        {view.skills.length > 0 && (
                          <div className="cedit__field cedit__field--wide">
                            <span>Ways of working</span>
                            <div className="caps">
                              {view.skills.map((skill) => {
                                const given = (edit.skills ?? []).includes(
                                  skill.id,
                                );
                                // Undecided resolves to everything available, so a character who
                                // has chosen nothing is not missing anything.
                                const held =
                                  edit.requestedCapabilities ??
                                  view.vocabulary.built;
                                const unmet = skill.requires.filter(
                                  (needed) => !held.includes(needed),
                                );
                                return (
                                  <label
                                    key={skill.id}
                                    className={`chip${given ? " chip--on" : ""}`}
                                    title={
                                      unmet.length > 0
                                        ? `${skill.summary || skill.name}\n\nWants ${unmet.join(", ")}, which this character does not have. The Skill still applies; the steps that need it will not.`
                                        : skill.summary || skill.name
                                    }
                                  >
                                    <input
                                      type="checkbox"
                                      checked={given}
                                      onChange={(e) =>
                                        setEdit({
                                          ...edit,
                                          skills: e.target.checked
                                            ? [...(edit.skills ?? []), skill.id]
                                            : (edit.skills ?? []).filter(
                                                (id) => id !== skill.id,
                                              ),
                                        })
                                      }
                                    />
                                    {skill.name}
                                    {given && unmet.length > 0 && (
                                      <i className="cc__unmet">
                                        wants {unmet.join(", ")}
                                      </i>
                                    )}
                                  </label>
                                );
                              })}
                            </div>
                            <p className="cc__hint">
                              A way of working, not a personality — the same
                              Skill can be given to several people without
                              making them the same person. What a Skill
                              <b> wants</b> is a request: it does not hand
                              anybody a capability.
                            </p>
                          </div>
                        )}

                        {/*
                          Provider-native settings. Read-only, and shown rather than hidden: a
                          file holding settings the user cannot see is a file they cannot
                          explain. Cold instrument — it names what will make it editable.
                        */}
                        <div className="cedit__field cedit__field--wide">
                          <span>
                            {(character.provider ?? "provider").toUpperCase()}{" "}
                            SETTINGS
                          </span>
                          {controls.length === 0 ? (
                            <p className="cc__hint">
                              {edit.provider
                                ? "Nothing of its own to tune — this backend's whole surface is above."
                                : "Assign a provider to see what it lets you tune."}
                            </p>
                          ) : (
                            <div className="cedit__grid">
                              {controls.map((control) => (
                                <Declared
                                  key={control.name}
                                  control={control}
                                  value={edit.tuning[control.name] ?? null}
                                  onChange={(value) => {
                                    const next = { ...edit.tuning };
                                    // Unset means unset: the key leaves rather than being
                                    // written as a zero the user never chose.
                                    if (value === null)
                                      delete next[control.name];
                                    else next[control.name] = value;
                                    setEdit({ ...edit, tuning: next });
                                  }}
                                />
                              ))}
                            </div>
                          )}
                          {character.dormantTuning.length > 0 && (
                            <p className="cc__hint">
                              Kept but dormant:{" "}
                              {character.dormantTuning.join(", ")}. Not applied
                              while {character.provider ?? "nobody"} is assigned
                              — switching back restores it.
                            </p>
                          )}
                        </div>
                      </details>

                      <div className="cedit__actions">
                        <button type="submit" className="btn" disabled={busy}>
                          {busy ? "SAVING…" : "SAVE"}
                        </button>
                        {/* The file IS the source of truth, and saying so keeps the Launcher
                            an editor over the vault rather than a parallel store. */}
                        <span className="cedit__file">{character.file}</span>
                      </div>

                      {/*
                        **Why a refusal is repeated here.**

                        There is already a notice near the top of the roster, and it is a
                        thousand lines above this button — off screen entirely once an editor is
                        open. A save that was refused therefore looked like a save that had
                        worked and then quietly reverted: the form reloaded from disk and the old
                        Service came back with no explanation anywhere the user was looking.

                        Second time this shape has cost an evening (Settings' REGENERATE was the
                        first). A control's answer belongs beside the control.
                      */}
                      {error && (
                        <p className="notice notice--warn cedit__refused">{error}</p>
                      )}

                      {/*
                        Last, and apart. A removal is not one more field of the form: it does not
                        wait for SAVE, it cannot be undone, and putting it beside a button people
                        press by habit is how it gets pressed by habit.
                      */}
                      <div className="cedit__field cedit__field--wide cedit__remove">
                        {removing === character.id ? (
                          <RemoveConfirm
                            plan={plan}
                            busy={busy}
                            onCancel={() => {
                              setRemoving(null);
                              setPlan(null);
                            }}
                            onConfirm={() => void confirmRemoval(character.id)}
                          />
                        ) : (
                          <button
                            type="button"
                            className="btn btn--mini"
                            disabled={busy}
                            onClick={() => void askRemoval(character.id)}
                          >
                            REMOVE FROM THE CREW
                          </button>
                        )}
                      </div>
                    </form>
                  )}
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
