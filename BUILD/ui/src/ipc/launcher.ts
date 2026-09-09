/**
 * The Launcher's only door to the Engine.
 *
 * Like `world.ts`, this is the one file that knows Tauri exists. Everything above it consumes
 * contracts and never learns where the data came from.
 */

import { invoke } from "@tauri-apps/api/core";

import { forgetSkin } from "../experience/useSkin";

import type {
  Action,
  Backend,
  BackendsView,
  Direction,
  DisclosureRow,
  ErasableView,
  Facet,
  MachineView,
  ModelOffer,
  PairedMachine,
  ReadinessRow,
  McpServer,
  McpView,
  CharacterEdit,
  LauncherView,
  ProviderStatus,
  ExportView,
  Inherited,
  RemovalView,
  Surface,
  WorkshopListing,
  WorkshopOffer,
  WorkshopShelf,
  Timbre,
} from "./contracts";

/** No Worlds is a valid answer; so is the engine not answering. Never a crash. */
const NONE: LauncherView = {
  orchestrator: {
    name: "ORCHESTRATOR",
    isUnnamed: true,
    portrait: null,
    problems: [],
  },
  worlds: [],
  characters: [],
  vocabulary: {
    archetypes: [],
    places: [],
    reasoning: [],
    capabilities: [],
    built: [],
    groups: [],
  },
  skills: [],
  sessionSeconds: 0,
  problems: [],
  definitionProblems: [],
};

export async function fetchWorlds(): Promise<LauncherView> {
  try {
    return await invoke<LauncherView>("list_worlds");
  } catch (error) {
    return { ...NONE, problems: [String(error)] };
  }
}

/** Enter a World by identity. Resolves false when no installed World has that id. */
export async function enterWorld(id: string): Promise<boolean> {
  try {
    // The windows belong to the World being left behind. Held onto, they would frame the new
    // World's contents in the old World's art (ADR-0016 — the pack owns the skin).
    forgetSkin();
    return await invoke<boolean>("enter_world", { id });
  } catch {
    return false;
  }
}

/**
 * Rename a World. Its identity never changes — only what it is called.
 *
 * Resolves to null on success, or the reason it could not be done. The Launcher shows that
 * reason rather than swallowing it: a rename that quietly does nothing is worse than one
 * that explains itself.
 */
export async function renameWorld(
  id: string,
  name: string,
): Promise<string | null> {
  try {
    await invoke("rename_world", { id, name });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * Save an edit to one member of the crew.
 *
 * Resolves to null on success, or the Engine's reason for refusing. Nothing is validated
 * here: the Engine owns what a valid character is, and a UI that decided separately would
 * eventually disagree with the file on disk.
 */
export async function saveCharacter(
  edit: CharacterEdit,
): Promise<string | null> {
  try {
    await invoke("save_character", { edit });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * What one Provider says about one model.
 *
 * No knobs is a legitimate answer, not a failure: a hosted backend whose whole surface is the
 * canonical parameters has nothing of its own to declare.
 */
export async function fetchSurface(
  provider: string,
  model: string,
): Promise<Surface> {
  try {
    return await invoke<Surface>("get_surface", { provider, model });
  } catch {
    // Reaching a Provider can fail, and a panel that cannot draw its Advanced section is not
    // a reason to fail to open somebody's character. Unknown, not zero.
    return { window: null, controls: [], can: null };
  }
}

/**
 * What a character's Brain inherits from MODELS.
 *
 * Read-only by construction: there is no command that writes any of this from a character
 * panel. Unreachable answers `nothing` rather than a number — a Brain nobody has configured is
 * a real state with a real sentence, and inventing a window here would be worse than saying so.
 */
export async function fetchInherited(
  provider: string,
  model: string,
): Promise<Inherited> {
  const nothing: Inherited = {
    model,
    window: null,
    source: "nothing",
    profile: null,
    generation: null,
    stable: false,
  };
  try {
    return await invoke<Inherited>("inherited_runtime", { provider, model });
  } catch {
    return nothing;
  }
}

/** Move a character into or out of a World. */
export async function setCharacterWorld(
  characterId: string,
  worldId: string,
  livesThere: boolean,
): Promise<string | null> {
  try {
    await invoke("set_character_world", { characterId, worldId, livesThere });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * Give a World key art, or pass `null` to clear it back to its derived chart.
 *
 * The image travels as a `data:` URI. The Engine sniffs its real format, chooses the
 * destination name itself and writes it inside the World — which is why the frontend needs no
 * filesystem permission to offer a picture.
 */
export async function setWorldArt(
  id: string,
  image: string | null,
): Promise<string | null> {
  try {
    await invoke("set_world_art", { id, image });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * Give a character one of their drawings, or clear it.
 *
 * Independent slots: `"sprite"` walks around the World, `"icon"` identifies them in a
 * conversation, and an action id (`"walk"`, `"idle"`, `"think"`, `"work"`) is a sheet of them
 * moving. The Engine names the file from the bytes (ADR-0024).
 *
 * `cut` says how to read a sheet and belongs to action slots only — the Engine refuses the
 * wrong pairing rather than ignoring it, so a sheet can never land on disk with no way to cut
 * it.
 */
/** Which of somebody's drawings is being addressed. */
export type ArtSlot = "sprite" | "icon" | Action;

/**
 * How a sheet is cut, as sent to the Engine.
 *
 * The authored half of `FramesView`: what comes back is the same numbers, validated. Every
 * field is required here because the Engine refuses a sheet it cannot cut, and a surface that
 * omitted one would only learn so from an error.
 */
export interface SheetCut {
  readonly columns: number;
  readonly rows: number;
  /** `0` means every cell — the common case. */
  readonly count: number;
  readonly milliseconds: number;
  readonly directions: readonly Direction[];
}

export async function setCharacterArt(
  characterId: string,
  slot: ArtSlot,
  image: string | null,
  cut?: SheetCut,
): Promise<string | null> {
  try {
    await invoke("set_character_art", { characterId, slot, image, cut });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * What removing a character would do — asked **before** anything is deleted.
 *
 * Its own call rather than a flag on the delete: the user reads this, accepts it, and only then
 * is anything removed.
 */
export async function characterRemoval(
  characterId: string,
): Promise<RemovalView | string> {
  try {
    return await invoke<RemovalView>("character_removal", { characterId });
  } catch (error) {
    return String(error);
  }
}

/**
 * Remove a character, after the user accepted what it does.
 *
 * The Engine rebuilds the plan at delete time rather than trusting the one this surface was
 * shown: a list of paths that came out of a window is a list somebody could change.
 */
export async function removeCharacter(
  characterId: string,
): Promise<string | null> {
  try {
    const problems = await invoke<string[]>("remove_character", {
      characterId,
    });
    return problems.length > 0 ? problems.join("; ") : null;
  } catch (error) {
    return String(error);
  }
}

/**
 * Change how an action's sheet is read, without re-importing it.
 *
 * The editor's real loop: a mis-cut sheet is discovered by watching it, and the fix is two
 * numbers rather than the same file uploaded again.
 */
export async function setCharacterCut(
  characterId: string,
  action: Action,
  cut: SheetCut,
): Promise<string | null> {
  try {
    await invoke("set_character_cut", { characterId, action, cut });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** What this machine has, what it does not, and what to do about each. */
export async function fetchReadiness(): Promise<readonly ReadinessRow[]> {
  try {
    return await invoke<ReadinessRow[]>("readiness");
  } catch {
    return [];
  }
}

/** Every machine paired with this Host. */
export async function fetchBridges(): Promise<readonly PairedMachine[]> {
  try {
    return await invoke<PairedMachine[]>("bridges");
  } catch {
    return [];
  }
}

/**
 * Reach out to a machine that is showing a code, instead of waiting for it to call.
 *
 * For a network whose router will not let the other machine start a connection. The bond is
 * identical either way; only who opens the socket differs.
 */
export async function enrolMachine(
  address: string,
  code: string,
  grants: readonly string[],
): Promise<string | null> {
  try {
    await invoke("enrol_machine", { address, code, grants });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** Finish a pairing. The code is spent whether or not it matched. */
export async function pairMachine(
  code: string,
  name: string,
  address: string,
  grants: readonly string[],
): Promise<string | null> {
  try {
    await invoke("pair", { code, name, address, grants });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** Change what a paired machine may be, without unpairing it. */
export async function setBridgeGrants(
  id: string,
  grants: readonly string[],
): Promise<string | null> {
  try {
    await invoke("set_bridge_grants", { id, grants });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** Unpair a machine — the roster and its secret, together. */
export async function unpairMachine(id: string): Promise<string | null> {
  try {
    await invoke("unpair", { id });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** What has been answered about turns leaving this machine, and what has not. */
export async function fetchDisclosures(): Promise<readonly DisclosureRow[]> {
  try {
    return await invoke<DisclosureRow[]>("disclosures");
  } catch {
    return [];
  }
}

/** Answer for one destination, or pass `null` to be asked again. */
export async function answerDisclosure(
  going: string,
  allowed: boolean | null,
): Promise<string | null> {
  try {
    await invoke("answer_disclosure", { going, allowed });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** What this machine is — its card, its memory. Measured, or absent. */
export async function fetchMachine(): Promise<MachineView | null> {
  try {
    return await invoke<MachineView>("this_machine");
  } catch {
    return null;
  }
}

/** What Ollama is featuring, with what this machine already has marked. */
export async function fetchFeaturedModels(): Promise<
  readonly ModelOffer[] | string
> {
  try {
    return await invoke<ModelOffer[]>("featured_models");
  } catch (error) {
    return String(error);
  }
}

/**
 * Search Hugging Face for models this machine could pull.
 *
 * The other source: Ollama's featured list is nineteen entries, this is the rest of the world,
 * GGUF only because that is what `ollama pull hf.co/…` takes.
 */
/**
 * Search Ollama's own shelf — the curated names between its front page and Hugging Face.
 *
 * Measured before it was built: `ollama.com` has no search API (`/api/search` answers 404) and
 * its registry manifest endpoint genuinely does. Names come from the page and are weighed
 * against the registry that answers, so a page that changes shape yields fewer results and says
 * so, never wrong ones.
 *
 * A failure here is **not** a failed search: Hugging Face is the other half and still answered.
 */
export async function searchOllama(
  query: string,
): Promise<ModelOffer[] | string> {
  try {
    return await invoke<ModelOffer[]>("search_ollama", { query });
  } catch (error) {
    return String(error);
  }
}

export async function searchModels(
  query: string,
  facets: readonly Facet[],
  /**
   * The cursor from a previous answer, when asking for the next page.
   *
   * Hugging Face pages by cursor rather than by number, so there is no "page 7" to jump to —
   * only "after this one". Passed back exactly as it arrived.
   */
  more?: string | null,
): Promise<SearchAnswer | string> {
  try {
    return await invoke<SearchAnswer>("search_models", {
      query,
      facets,
      more: more ?? null,
    });
  } catch (error) {
    return String(error);
  }
}

export interface SearchAnswer {
  readonly offers: readonly ModelOffer[];
  /** Absent when this was the last page. What decides whether MORE exists at all. */
  readonly more: string | null;
}

/**
 * Download a model into the local runtime.
 *
 * Returns as soon as it has started: this takes minutes, and progress arrives on the
 * `models:pulling` and `models:pulled` events instead.
 */
export async function pullModel(model: string): Promise<string | null> {
  try {
    await invoke("pull_model", { model });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** One line of the runtime's own account of a download. */
export interface Fetching {
  readonly status: string;
  readonly total: number | null;
  readonly completed: number | null;
}

/**
 * Give a character a different brain, because the assigned one could not be reached.
 *
 * **The user's answer to a question, never a failover.** Epoch does not choose a Brain: the
 * model is part of who somebody is (ADR-0026), so a silent substitution is a character behaving
 * differently with nobody told. `because` is the question that was being answered, and it goes
 * into the Chronicle beside the change.
 */
export async function reassignBrain(
  characterId: string,
  backend: string,
  model: string,
  because: string,
): Promise<string | null> {
  try {
    await invoke("reassign_brain", { characterId, backend, model, because });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * The other runtimes on this machine: llama.cpp and LM Studio.
 *
 * Both speak an OpenAI-compatible API, which Epoch already speaks — so adding one as a Service
 * is the ordinary backend form rather than new code.
 */
export async function localRuntimes(): Promise<LocalRuntime[]> {
  try {
    return await invoke<LocalRuntime[]>("local_runtimes");
  } catch {
    return [];
  }
}

export interface LocalRuntime {
  readonly id: string;
  readonly name: string;
  /** A binary was found. The fix for `false` is {@link LocalRuntime.install}. */
  readonly installed: boolean;
  /** Where it was found, so "not installed" is checkable. */
  readonly foundAt: string | null;
  /** Its API answered — the fact that actually matters. */
  readonly serving: boolean;
  readonly endpoint: string;
  /**
   * What it **offers**. Empty is honest: LM Studio serves with nothing loaded.
   *
   * A shelf, not a card. This was shown as *"holding …"* and it never meant that.
   */
  readonly models: readonly string[];
  /**
   * What is **in memory right now**, measured from each server's own answer.
   *
   * Empty is the ordinary state between turns, and it is a real reading rather than a blank:
   * the row says so.
   */
  readonly resident: readonly string[];
  /** The command that installs it here, measured against the package manager. */
  readonly install: string;
  /**
   * The command that would start its server, when the program is here.
   *
   * `null` means there is nothing to start — a different sentence from *it is not running*.
   */
  readonly start: string | null;
  /** What it says it can put a model on, in its own words. Empty means it was never asked. */
  readonly devices: readonly string[];
  /**
   * The one thing this machine could be doing faster, when the program's own answer says so.
   *
   * `null` is the ordinary state. Not a warning and not an error — a fact Epoch cannot fix and
   * the user can.
   */
  readonly handicap: string | null;
  /** Models this runtime keeps in its own cache. llama.cpp only; empty for the others. */
  readonly cached: readonly string[];
}

/** Start a runtime's server, in the user's own terminal. */
/**
 * What can make a picture on this machine (ADR-0030).
 *
 * A separate list from {@link localRuntimes} on purpose: everything in that one becomes a brain
 * a character can be assigned to, and a diffusion server is not one.
 */
/**
 * What can speak on this machine.
 *
 * A third list beside {@link localRuntimes} and {@link imageStudios}, and separate for the same
 * reason they are separate from each other: everything in the first becomes a brain a character
 * can be assigned to, and a text-to-speech binary is not one.
 *
 * **No `serving` field, deliberately.** Piper has no server and no port — it is handed a sentence
 * and writes a `.wav`. A lamp that can never move is worse than no lamp.
 */
export interface VoiceEngine {
  readonly id: string;
  readonly name: string;
  readonly installed: boolean;
  /** Exactly which file answered, so two installs can be told apart. `null` when there is none. */
  readonly at: string | null;
  /** Epoch fetched this one, as opposed to finding one the user already had. */
  readonly ours: boolean;
  /** What Epoch would download, and roughly what it weighs. `null` on a platform nobody measured. */
  readonly archive: { readonly url: string; readonly name: string; readonly bytes: number } | null;
  readonly installing: string;
  readonly licence: string;
}

/**
 * What can listen on this machine, and with which models.
 *
 * A fourth list, and separate from {@link voiceEngines} for the reason that one is separate from
 * the runtimes: an ear is not a mouth and neither is a brain. What they share is a shape — no
 * server, no port, a program handed one utterance — and sharing a *shape* is not a reason to
 * share a list.
 */
export interface VoiceEar {
  readonly id: string;
  readonly name: string;
  readonly installed: boolean;
  readonly at: string | null;
  readonly ours: boolean;
  /** The models, offered or present. **Empty `here` is the state that matters**: an ear with no
   * model hears nothing, exactly as a mouth with no voice says nothing. */
  readonly models: readonly {
    readonly id: string;
    readonly file: string;
    readonly bytes: number;
    readonly about: string;
    readonly here: boolean;
  }[];
  readonly archive: { readonly url: string; readonly name: string; readonly bytes: number } | null;
  readonly installing: string;
  readonly licence: string;
}

export async function voiceEars(): Promise<VoiceEar[]> {
  try {
    return await invoke<VoiceEar[]>("voice_ears");
  } catch {
    return [];
  }
}

/** Fetch the ear, or one of its models. Progress arrives on `workshop:fetching`. */
export async function installEar(
  what: string,
): Promise<{ said: string; failed: boolean }> {
  try {
    return { said: await invoke<string>("install_ear", { what }), failed: false };
  } catch (why) {
    return { said: String(why), failed: true };
  }
}

export async function voiceEngines(): Promise<VoiceEngine[]> {
  try {
    return await invoke<VoiceEngine[]>("voice_engines");
  } catch {
    return [];
  }
}

/**
 * Fetch the voice engine.
 *
 * **The only program on the deck Epoch downloads itself**, because it is the only one with no
 * package to ask a package manager for — measured on both platforms. Progress arrives on
 * `workshop:fetching`, the same event the asset shelf uses.
 */
export async function installVoiceEngine(
  id: string,
): Promise<{ said: string; failed: boolean }> {
  try {
    return { said: await invoke<string>("install_voice_engine", { id }), failed: false };
  } catch (why) {
    return { said: String(why), failed: true };
  }
}

/** One voice installed on this machine. */
export interface InstalledVoice {
  /** What a character's `speaksWith` holds — `es_ES-davefx-medium`. */
  readonly name: string;
  /** What it is, from the sidecar: *Español (Spain) · medium · 22050 Hz*. Empty when unread. */
  readonly what: string;
  /** Which language, for grouping a long list into something scannable. `null` when unread. */
  readonly language: string | null;
}

/**
 * Which voices are on the shelf.
 *
 * **Voices, never files** (ADR-0016). A Piper voice happens to be one file today; one Kokoro
 * model holds fifty-four, and this signature does not change on the day that arrives.
 */
/** One converted RVC voice on this machine. */
export interface InstalledTimbre {
  /** What a character's `soundsLike.voice` holds. */
  readonly name: string;
  /** What it is, from the sidecar the conversion wrote: *40 kHz · 109 speakers*. */
  readonly what: string;
  /** How many voices are inside it, so a surface knows whether to offer a choice at all. */
  readonly speakers: number;
}

/**
 * What this machine can do about RVC right now.
 *
 * Every field is its own fact because each has its own fix — and `canSpeak` is deliberately not
 * `ready`: converting a voice needs Python and speaking one does not, so a machine whose PyTorch
 * has been deleted keeps every voice it already made.
 */
export interface VoiceForge {
  readonly python: string | null;
  readonly howToGetPython: string;
  readonly environment: boolean;
  readonly torch: boolean;
  readonly definitions: boolean;
  readonly encoder: boolean;
  readonly runtime: boolean;
  readonly cost: string;
  /** Whether a converted voice could be spoken right now. */
  readonly canSpeak: boolean;
  /** The **single** next step, never a list. `null` is nothing in the way. */
  readonly nextStep: string | null;
}

export async function installedTimbres(): Promise<InstalledTimbre[]> {
  try {
    return await invoke<InstalledTimbre[]>("installed_timbres");
  } catch {
    return [];
  }
}

export async function voiceForge(): Promise<VoiceForge | null> {
  try {
    return await invoke<VoiceForge>("voice_forge");
  } catch {
    // **`null` is unasked, never *unavailable*.** A surface that read a failed call as a cold
    // forge would tell somebody to install Python they already have.
    return null;
  }
}

/** Build the forge. Long — about 1.3 GB the first time — and each step says what it is doing. */
export async function prepareVoiceForge(): Promise<string> {
  return await invoke<string>("prepare_voice_forge");
}

/** Convert one `.pth` on the Timbres shelf into a voice that can be spoken. */
export async function convertTimbre(checkpoint: string): Promise<string> {
  return await invoke<string>("convert_timbre", { checkpoint });
}

export async function installedVoices(): Promise<InstalledVoice[]> {
  try {
    return await invoke<InstalledVoice[]>("installed_voices");
  } catch {
    return [];
  }
}

/** One line, spoken. */
export interface Spoken {
  /**
   * Where to play it from: `epoch://spoken-<name>`.
   *
   * **A name the Engine resolves, never a path and never the bytes.** This was a `data:` URI
   * first and it never made a sound — `media-src` here is `epoch: http://epoch.localhost`, so
   * the browser refused every one of them, silently, because a blocked source and a broken file
   * raise the same error.
   */
  readonly sound: string;
  readonly millis: number;
  readonly bytes: number;
  /**
   * What went wrong with the **optional** half: a timbre that is not converted here, or one
   * that refused. The line was still spoken, in the plain voice.
   *
   * Absent is nothing to say. It exists because falling back silently would leave a character
   * sounding like the wrong person with nothing on screen saying so — the same rule as a gauge
   * that must not read a number nobody can explain.
   */
  readonly but?: string;
}

/**
 * Say one line with one voice, and hand back the sound itself.
 *
 * The deck's own check: a voice that installed perfectly and cannot speak is exactly what a row
 * reading `INSTALLED` would otherwise hide.
 */
export async function tryVoice(
  voice: string,
  say: string,
  soundsLike?: Timbre | null,
): Promise<Spoken | string> {
  try {
    return await invoke<Spoken>("try_voice", { voice, say, soundsLike: soundsLike ?? null });
  } catch (why) {
    return String(why);
  }
}

export async function imageStudios(): Promise<ImageStudio[]> {
  try {
    return await invoke<ImageStudio[]>("image_studios");
  } catch {
    return [];
  }
}

/**
 * Walk the whole picture chain once and report every link.
 *
 * **This draws a real picture**, so it takes as long as drawing one — seconds, sometimes a
 * minute on a cold model. The caller shows that it is working; nothing here pretends otherwise.
 *
 * An empty list means the command itself failed, which is a different fact from a broken link
 * and is reported as one rather than as a passing chain.
 */
export async function testStudio(): Promise<StudioCheck[]> {
  return await invoke<StudioCheck[]>("test_studio");
}

/**
 * Build a plain workflow from what this ComfyUI has, and attach it to General.
 *
 * Nothing ships: the graph is written against the checkpoint the server listed a moment ago and
 * compiled against that server's schema. Answers the name it gave the workflow, or throws with
 * a sentence saying what is missing.
 */
export async function buildWorkflow(): Promise<string> {
  return await invoke<string>("build_workflow");
}

/** One link of the picture chain, walked. */
export interface StudioCheck {
  /** What was tried. */
  readonly step: string;
  readonly ok: boolean;
  /** What happened — on a failure, the fix, or ComfyUI's own refusal. */
  readonly said: string;
}

export interface ImageStudio {
  readonly id: string;
  readonly name: string;
  /** A binary was found. The fix for `false` is {@link ImageStudio.install}. */
  readonly installed: boolean;
  readonly foundAt: string | null;
  /** Its API answered. The fact that actually matters. */
  readonly serving: boolean;
  readonly endpoint: string;
  /** Checkpoints it holds. Empty while serving is *running with nothing to draw with*. */
  readonly models: readonly string[];
  readonly install: string;
  readonly start: string | null;
  /**
   * Whether {@link ImageStudio.start} starts the **server** or only opens the program's own
   * front door.
   *
   * Measured: Comfy Desktop's executable opens a dashboard of instances and nothing is serving
   * until one is chosen. A panel that said *starting it* about that would name an event that
   * did not happen.
   */
  readonly startsServer: boolean;
  /** What to expect after pressing START — which differs between those two paths. */
  readonly firstRun: string;
}

/**
 * Search every asset catalogue at once (ADR-0031).
 *
 * The caller never learns which site answered. A source that refuses contributes its refusal
 * beside the results rather than emptying them — a busy server must never read as *that does
 * not exist*.
 */
export async function findAssets(
  words: string,
  kind: string,
  adult: boolean,
  /** `most_downloaded` or `newest` — the two orders both sites genuinely support. */
  order: string,
  /** A family to narrow to, or empty for all of them. */
  base: string,
  more: readonly MoreRow[] = [],
): Promise<AssetsFound> {
  try {
    return await invoke<AssetsFound>("find_assets", {
      words,
      kind,
      adult,
      order,
      base,
      more,
    });
  } catch (why) {
    return { assets: [], refused: [String(why)], more: [], card: "" };
  }
}

/**
 * How far one source got.
 *
 * Opaque, and handed back exactly as it arrived: Civitai's is a whole next-page URL and Hugging
 * Face's is an offset, and anything that took either apart would be guessing at somebody else's
 * private format.
 */
export interface MoreRow {
  readonly source: string;
  readonly cursor: string;
}

export interface AssetsFound {
  readonly assets: readonly AssetRow[];
  /** Sources that did not answer, in their own words. */
  readonly refused: readonly string[];
  /** Where each source that has more got to. Empty means every one reached its end. */
  readonly more: readonly MoreRow[];
  /** This machine's card, in one line. Empty when there is nothing to report. */
  readonly card: string;
}

export interface AssetRow {
  readonly id: string;
  readonly name: string;
  readonly source: string;
  /**
   * Which reader to ask when this row is installed — **not** `source`.
   *
   * Two shelves read Hugging Face and ask it different things: CREATIONS asks what repositories
   * exist, VOICES asks what is inside one. Both truthfully answer `Hugging Face` to *who
   * published this*, and routing an install on that would send a voice to the reader that
   * cannot find one.
   */
  readonly catalogue: string;
  readonly kind: string;
  /** What the site called the base — `Krea 2`, `Pony`. Verbatim, because a person knows it. */
  readonly saidBase: string;
  /** What Epoch can build with. `unknown` is unmeasured, never incompatible. */
  readonly family: string;
  readonly by: string | null;
  readonly downloads: number;
  readonly adult: boolean;
  readonly bytes: number;
  readonly triggers: readonly string[];
  readonly page: string;
  /**
   * **A token, never an address.** `epoch://preview/<token>` is what the row asks for, and the
   * Engine decides what it resolves to — so the window holds no URL to a third party and
   * `img-src` stays at `'self' data: epoch:`.
   *
   * `null` where the source published no picture, which is most of Hugging Face.
   */
  readonly preview: string | null;
  /**
   * A recording of what this sounds like, as a token: `epoch://sample-<token>`.
   *
   * Its own field and not `preview`, which is a *picture* — putting an mp3 in a field every
   * surface draws as an `<img>` would be a real reading of the wrong quantity. `null` where the
   * source published none.
   */
  readonly sample: string | null;
  /**
   * Which medium the **source** placed it in — `picture`, `video`, `sound`, `model`.
   *
   * A weaker reading than a shelf's: a file on a shelf is read from its own bytes and this has
   * not been downloaded. `null` is unplaced and stays visible under every medium.
   */
  readonly makes: string | null;
  /** The download needs a key nobody has stored. Said before it is pressed. */
  readonly needsKey: boolean;
  /**
   * Every version this was published in, when there is more than one.
   *
   * A LoRA published for two engines is two different files — 170 MB for one, 18 MB for the
   * other — and taking the largest fetched the wrong one for a Flux graph. Empty means there is
   * nothing to choose, and no control is drawn.
   */
  readonly versions: readonly VersionRow[];
  /**
   * Whether it fits in this card's free memory.
   *
   * `null` is *nobody measured*, never *no* — and one that does not fit is still offered, because
   * it is their disk and their decision.
   */
  readonly fits: boolean | null;
}

export interface VersionRow {
  /** The id to install: `civitai:2851202#2142473`. */
  readonly id: string;
  readonly name: string;
  readonly saidBase: string;
  readonly family: string;
  readonly bytes: number;
}

/**
 * Bring one asset into the library. Gigabytes, so this takes as long as it takes.
 *
 * The window passes an **id**, never a URL: the address, the size and the hash are asked of the
 * source at the moment of the download, so nothing crossing this boundary can redirect it.
 */
export async function installAsset(
  source: string,
  id: string,
): Promise<{ said: string; failed: boolean }> {
  try {
    return {
      said: await invoke<string>("install_asset", { source, id }),
      failed: false,
    };
  } catch (why) {
    return { said: String(why), failed: true };
  }
}

/** What the Studio Panel offers for one chosen model (ADR-0033). */
export async function studioPanel(
  checkpoint: string,
  clip: readonly string[] = [],
): Promise<PanelView | null> {
  try {
    // The encoders chosen so far travel with the question, because what they *are* is what
    // decides how this model is put together — and Epoch can read an encoder for every file on
    // the shelf, while it can read a checkpoint's family for five families and nothing newer.
    return await invoke<PanelView>("studio_panel", { checkpoint, clip });
  } catch {
    return null;
  }
}

export interface PanelView {
  readonly models: readonly PanelModelRow[];
  readonly loras: readonly PanelLoraRow[];
  readonly shapes: readonly PanelShapeRow[];
  /** Text encoders the drawing machine can load, for a model that arrives in parts. */
  readonly encoders: readonly PanelPartRow[];
  readonly vaes: readonly PanelPartRow[];
  /**
   * Upscale models the drawing machine offers.
   *
   * Empty is honest and common: nothing installed, or a server whose build has no upscale node.
   * The shelf has existed since ADR-0032 and until now nothing consumed what landed on it.
   */
  readonly upscalers: readonly PanelPartRow[];
  /**
   * The ControlNets this server lists.
   *
   * Empty is the ordinary state of a fresh ComfyUI — measured, `control_net_name` is an empty
   * combo until something is installed — and it is said rather than hidden.
   */
  readonly controlnets: readonly PanelPartRow[];
  /**
   * What this server can turn a picture into, by node class. Empty is a real answer.
   *
   * A named set filtered by what the server has, never a category: `Canny` sits in
   * `image/filters` beside `ImageBlur`, and no category on the machine says *preprocessor*.
   */
  readonly preparations: readonly string[];
  /**
   * The video families this drawing machine can run, in the order they are offered.
   *
   * Empty is the ordinary answer on a ComfyUI without the video nodes, and the VIDEO tab says so
   * rather than offering a family that would be refused after the form was filled in.
   */
  readonly motions: readonly string[];
  /**
   * The families this machine can make **sound** with, in the order they are offered.
   *
   * Its own list rather than a tag on `motions`: two tabs read two lists, and one list filtered
   * in the surface would be a second place deciding which family belongs where.
   */
  readonly sounds: readonly string[];
  /**
   * Which of those are told words that are **sung**, beside words that describe.
   *
   * A subset of `sounds`. ACE-Step's encoder takes `tags` and `lyrics`; Stable Audio's takes a
   * description only, so a LYRICS box on its tab would reach nothing.
   */
  readonly lyrical: readonly string[];
  /** The families this machine can make a **model** with, in the order they are offered. */
  readonly meshes: readonly string[];
  /**
   * Whether the drawing machine is answering right now.
   *
   * Opening the panel does not start it any more — GENERATE does. So this is ordinarily false
   * on a fresh machine, and what it changes is what the panel can *know*: the file lists come
   * off the shelves either way, and a node's own vocabulary does not exist until something is
   * running to be asked.
   */
  readonly serving: boolean;
  /**
   * The embeddings this server has, by the names it answers with — no extensions.
   *
   * Not a control that draws: an embedding is not loaded by a node, and the only way to use one
   * is to write `embedding:<name>` into the prompt. Measured on a real server, same seed: a name
   * that is **not** installed is not refused — the words are encoded as ordinary text and quietly
   * change the picture. So the list is the difference between choosing and guessing.
   */
  readonly embeddings: readonly string[];
  /**
   * The encoder families **ComfyUI itself publishes**.
   *
   * One string is the whole of what separates Flux from Z-Image from Qwen-Image from SD 3, and
   * a family added to ComfyUI tomorrow appears here with no code at all.
   */
  readonly clipTypes: readonly string[];
  /**
   * The same, in `DualCLIPLoader`'s vocabulary — **a different list, and barely an overlapping
   * one.** Measured: the single loader offers `stable_diffusion` and not `flux`; the double
   * offers `flux` and `sdxl` and not `stable_diffusion`. Showing one list for both is how a
   * picture came to be refused by the server after the user had chosen everything correctly.
   */
  readonly clipTypesTwo: readonly string[];
  /**
   * What a model of the chosen family is normally assembled from.
   *
   * `null` when Epoch has nothing true to say — an unmeasured model, or a family whose assembly
   * it has no documented account of. Silence, rather than a guess dressed as help.
   */
  readonly assembly: PanelAssemblyRow | null;
  /**
   * What this exact model has already been drawn with **on this machine**, newest first.
   *
   * Evidence, never a claim about the architecture: it says a graph ran, not that it is the best
   * one, and a second entry is a second thing that worked rather than a contradiction.
   *
   * Empty when nobody has drawn with it — and empty when nobody has measured its hash, which is a
   * different sentence and the honest one. It fills in the first time a picture is made with it.
   */
  readonly remembered: readonly PanelRecipeRow[];
  /** Where the picture would be made, in words. */
  readonly whereAt: string;
  readonly problem: string | null;
}

/**
 * Guidance for a model that arrives in parts — never a choice made on the user's behalf.
 *
 * Measured 2026-08-25 against the files on this machine: Epoch reads the *kind* of every part
 * correctly and the *family* of almost none of them — both text encoders and the Flux VAE came
 * back `unknown`. So greying the parts that belong elsewhere, the way a LoRA row is greyed, would
 * have greyed nothing. What Epoch does know is the model's family and what that family is loaded
 * with, and saying so is what stops somebody choosing between twelve names with nothing to go on.
 */
/**
 * One part a model that arrives in parts can be given: an encoder, or a VAE.
 *
 * A part has no family — `clip_l.safetensors` is the same file beside Flux and beside SDXL — so
 * what the row says is what the part *is*, read from the width of its own embedding table.
 */
export interface PanelPartRow {
  /** Exactly as the drawing server listed it. */
  readonly file: string;
  /** `CLIP-L`, `T5-XXL`, `a language model`. `null` for a file Epoch does not hold or cannot read. */
  readonly says: string | null;
  /**
   * Which medium this part decodes into — `picture`, `video`, `sound`, `model`.
   *
   * Read from the file's own convolutions for a VAE, and `null` everywhere else, including for a
   * VAE whose shape says nothing. **`null` is unplaced and never *not this one*:** such a file is
   * offered under every medium, because hiding it would punish the user for a failure of Epoch's.
   */
  readonly medium: string | null;
}

/** One combination that was run against this model here, and what came of it. */
export interface PanelRecipeRow {
  /** The encoders, as the server names them — what a graph must name. */
  readonly clip: readonly string[];
  readonly clipType: string;
  readonly vae: string;
  /** A picture came out. */
  readonly drew: boolean;
  /** The server's own words, when it refused. Only ever a refusal about the configuration. */
  readonly said: string | null;
}

export interface PanelAssemblyRow {
  /** The family in a person's words — `Flux`, `SD 3`, or `unknown`. */
  readonly family: string;
  /**
   * How many text encoders this family takes.
   *
   * `null` when the family could not be read — which is the one thing that decides the count, so
   * a number here would be a guess at the whole answer.
   */
  readonly encoders: number | null;
  /** What those encoders are, named as what they are rather than as files. */
  readonly says: string;
  /**
   * The family's name in ComfyUI's own vocabulary — present only when the loader that this many
   * encoders selects genuinely publishes it. A family the server has never heard of is left
   * unsaid rather than recommended into a refusal.
   */
  readonly clipType: string | null;
  /**
   * Which encoders this family wants, named the way a part row names a file.
   *
   * So the guidance and the dropdown meet in the dropdown: the sentence said *a CLIP-L and a
   * T5-XXL* and each row said which file was which, and somebody still had to hold one and match
   * it against the other by eye, once per field. Empty where the family is unknown.
   */
  readonly wants: readonly string[];
}

export interface PanelModelRow {
  /**
   * What the site called its base, verbatim — `Pony`, `Illustrious`, `Flux.1 D`.
   *
   * Finer than `family`, which has five names and calls Pony and Illustrious the same thing.
   * Said and never enforced: it is the author's claim, not a measurement.
   */
  readonly saidBase: string | null;
  readonly file: string;
  readonly family: string;
  /** Read from the file when Epoch has it; `null` for one it never saw. */
  readonly bytes: number | null;
  /** `checkpoint` or `diffusion` — which recipe the Engine will write. */
  readonly kind: string;
  /** What a diffusion model still needs before it can draw. Empty means it can. */
  readonly needs: readonly string[];
  /** The encoders and VAE that would be loaded beside it, so the choice is visible. */
  readonly with: readonly string[];
  /**
   * Which medium it makes: `picture`, `video`, `sound`. **`null` is a real answer.**
   *
   * Measured from the model's own tensors. A family Epoch read belongs to one medium and is
   * listed under that tab; a family it could not read is listed under **every** tab, because
   * hiding somebody's file for a failure of Epoch's is worse than offering one that may not fit.
   */
  readonly makes: "picture" | "video" | "sound" | "model" | null;
  /**
   * Whether this checkpoint carries its own text encoder.
   *
   * Measured from its tensors, never inferred from what it makes. The panel asked for an encoder
   * beside every non-picture checkpoint — a rule generalised from two that borrow one — and
   * ACE-Step is all-in-one, so GENERATE sat grey waiting for a file it has no use for.
   */
  readonly carriesEncoder: boolean;
}

export interface PanelLoraRow {
  /**
   * What the site called its base, verbatim — `Pony`, `Illustrious`, `Flux.1 D`.
   *
   * Finer than `family`, which has five names and calls Pony and Illustrious the same thing.
   * Said and never enforced: it is the author's claim, not a measurement.
   */
  readonly saidBase: string | null;
  readonly file: string;
  readonly family: string;
  readonly bytes: number;
  readonly triggers: readonly string[];
  readonly fits: boolean;
  /** Why not, in a person's words. Present for an unmeasured one too. */
  readonly why: string | null;
}

export interface PanelShapeRow {
  readonly label: string;
  readonly width: number;
  readonly height: number;
  /**
   * A size the chosen family was actually trained at.
   *
   * Marked, never enforced: asking SD 1.5 for 1024 gives two heads and asking Flux for 4K gives
   * an out-of-memory, and neither is Epoch's decision to make for somebody who owns the card.
   */
  readonly native: boolean;
}

export interface PanelAsk {
  readonly checkpoint: string;
  /** `checkpoint` or `diffusion`, as the panel row said. */
  readonly kind: string;
  readonly loras: readonly (readonly [string, number])[];
  /** An upscale model to enlarge the finished picture with, as the server lists it. Empty is none. */
  readonly upscale: string;
  /**
   * Each ControlNet, its reference picture and how hard it steers — in order.
   *
   * A list because they chain: each apply node takes a pair of conditionings and answers a
   * pair, so a pose from one picture and a depth from another stack. Empty is the ordinary case.
   */
  readonly controls: readonly PanelSteerRow[];
  /** A picture this one is drawn on top of, by the name the server answered with. Empty is none. */
  readonly from: string;
  /** How much of that picture survives: `0` none of it, `1` all of it. */
  readonly keep: number;
  readonly prompt: string;
  readonly negative: string;
  readonly width: number;
  readonly height: number;
  readonly steps: number;
  readonly cfg: number;
  /** `0` means a different picture every time. */
  readonly seed: number;
  readonly batch: number;
  /** Flux's own dial. Ignored by everything else. */
  readonly guidance: number;
  /** For a model that arrives in parts: its encoders, their family, and its VAE. */
  readonly clip: readonly string[];
  readonly clipType: string;
  readonly vae: string;
  /**
   * Which video family, by the name `PanelView.motions` offered. Empty is a picture.
   *
   * The name rather than a flag: a boolean would say *this moves* and leave the Engine to guess
   * which of several ways, and that guess would be Epoch choosing a family for a file it never
   * measured.
   */
  readonly motion: string;
  /** How many frames, and how fast they play. Read only by a family that makes video. */
  readonly frames: number;
  readonly fps: number;
  /** How long, in seconds. Read only by a family that makes sound, which has no frames. */
  readonly seconds: number;
  /**
   * What is **sung**, which is not what describes the song.
   *
   * Read only by a family whose encoder takes it. Empty is an instrumental.
   */
  readonly lyrics: string;
  /**
   * For a model: the pictures it is built from, one per side — front, left, back, right.
   *
   * **Named views, not a bag.** `Hunyuan3Dv2ConditioningMultiView` takes four optional inputs by
   * name, so a list would have to guess which is which. An empty string is a side nobody gave.
   *
   * All empty means *draw them here*, from `shapePrompts` and `shapeWith`. Both ways are the
   * person's — Epoch never picks the model or the picture.
   */
  readonly shapeFrom: readonly string[];
  /** One prompt per side, when they are drawn here. Same order, same rule about empties. */
  readonly shapePrompts: readonly string[];
  /**
   * How the surface is read out of the voxels: `smooth`, `fine` or `blocky`.
   *
   * One named look rather than two dials: the algorithm and the octree the shape is sampled on
   * are decided together, because a person means *smooth* or *blocky* and not *surface net at
   * 256*. `fine` is the same algorithm at 512 — measured, about six minutes against one, for a
   * base and thin detail that stop showing the grid.
   */
  readonly surface: string;
  /** Which checkpoint draws that picture, when one is drawn here. */
  readonly shapeWith: string;
}

/**
 * The base models worth offering as a filter, in the sites' own words.
 *
 * Read from the Engine rather than typed here: the list was measured across 400 of Civitai's
 * most-downloaded models, and a second copy on a screen would be the second answer that drifts.
 */
export async function assetBases(): Promise<readonly string[]> {
  try {
    return await invoke<string[]>("asset_bases");
  } catch {
    return [];
  }
}

/**
 * Take a file you already have into the library.
 *
 * The shell opens the dialog and the Engine reads the file — the same bargain as the folder
 * picker (ADR-0024): no filesystem API is added to this window, and it never learns a path the
 * user did not choose.
 *
 * `null` means the dialog was dismissed, which is an answer rather than a failure.
 */
export async function importAsset(): Promise<{
  said: string;
  failed: boolean;
} | null> {
  try {
    const said = await invoke<string | null>("import_asset");
    return said === null ? null : { said, failed: false };
  } catch (why) {
    return { said: String(why), failed: true };
  }
}

/**
 * Remember what the panel was filled in with (ADR-0033).
 *
 * **Pressing GENERATE does not draw.** It records the choice; then the person's own words go to
 * the character as an ordinary message, and the picture arrives as the character's reply. That is
 * what keeps the result on the Quest as evidence rather than inside a form — and it is why the
 * character still never names a file: it says *what*, the panel already said *how*.
 */
export async function chooseRecipe(ask: PanelAsk): Promise<string | null> {
  try {
    await invoke("choose_recipe", { ask });
    return null;
  } catch (why) {
    return String(why);
  }
}

/**
 * What pressing GENERATE produced.
 *
 * **Two shapes, because there are two.** A picture is finished when the call returns; a video is
 * not — it becomes a Job, the character shows as waiting in the World, and it arrives in the
 * Chronicle minutes later (ADR-0034). A nullable filename would have made that the surface's
 * guess instead of the Engine's answer.
 */
export type PanelMade =
  | { readonly kind: "drew"; readonly file: string; readonly at: string | null; readonly seconds: number }
  | { readonly kind: "began"; readonly what: string; readonly waitingOn: string };

/**
 * Start the picture studio, because the panel needs something only a running one can answer.
 *
 * Returns at once — starting takes about thirty seconds and the panel re-asks while it does.
 * The same start GENERATE makes, so Epoch owns it and stops it once the picture is made.
 */
export async function wakeTheStudio(): Promise<boolean> {
  try {
    return await invoke<boolean>("wake_the_studio");
  } catch {
    return false;
  }
}

/** Make it. Returns the failure as a string, or what happened. */
export async function drawFromPanel(ask: PanelAsk): Promise<PanelMade | string> {
  try {
    return await invoke<PanelMade>("draw_from_panel", { ask });
  } catch (why) {
    return String(why);
  }
}

/**
 * Which asset catalogues hold a key, and what each one is for.
 *
 * Names and a yes/no. There is deliberately no field a value could arrive in — the panel cannot
 * display a stored token however it is written (ADR-0026's guarantee, on a surface).
 */
/**
 * The bytes of one picture Epoch keeps, as a `data:` URI.
 *
 * Re-exported rather than re-implemented: the cache in `world.ts` is keyed by the Engine's
 * filename, which is a hash of the bytes, so a picture fetched there and here is fetched once.
 */
export { sharedImage } from "./world";

export async function catalogueKeys(): Promise<readonly CatalogueKeyRow[]> {
  try {
    return await invoke<CatalogueKeyRow[]>("catalogue_keys");
  } catch {
    return [];
  }
}

export interface CatalogueKeyRow {
  readonly id: string;
  readonly name: string;
  /** A key is stored. Never what it is. */
  readonly held: boolean;
  /** What stops working without one — measured against the site, not guessed. */
  readonly neededFor: string;
  readonly foundAt: string;
  /** What the key can do beyond what Epoch uses it for. */
  readonly caution: string;
}

/** Keep one catalogue's key, encrypted for this Windows account. Write-only. */
export async function saveCatalogueKey(
  source: string,
  value: string,
): Promise<string | null> {
  try {
    await invoke("save_catalogue_key", { source, value });
    return null;
  } catch (why) {
    return String(why);
  }
}

/** Throw one catalogue's key away. */
export async function forgetCatalogueKey(
  source: string,
): Promise<string | null> {
  try {
    await invoke("forget_catalogue_key", { source });
    return null;
  } catch (why) {
    return String(why);
  }
}

/**
 * What this World can make, and what it is missing (ADR-0030).
 *
 * One call rather than three: Styles without their workflows is six words and no information,
 * and asking ComfyUI whether it is answering costs the same round trip either way.
 */
export async function imageShelf(): Promise<ImageShelf | null> {
  try {
    return await invoke<ImageShelf>("image_shelf");
  } catch {
    return null;
  }
}

export interface ImageShelf {
  readonly styles: readonly StyleRow[];
  readonly workflows: readonly WorkflowRow[];
  /** Which Style is used when nobody says. */
  readonly usually: string;
  readonly installed: boolean;
  /** ComfyUI is answering. The fact that decides whether anything can be drawn right now. */
  readonly serving: boolean;
  /** Every machine that could make a picture, this one first. */
  readonly benches: readonly BenchRow[];
  /** Which of them this World draws on. Empty is this machine. */
  readonly drawOn: string;
  /** This installation's Generative Library, and what ComfyUI was told about it (ADR-0032). */
  readonly library: LibraryRow;
  readonly problem: string | null;
}

/** Where Epoch keeps what it downloaded, and whether ComfyUI has heard about it. */
export interface LibraryRow {
  readonly root: string;
  readonly note: string;
  /** ComfyUI has to be restarted once before it can see the library. Measured, not assumed. */
  readonly restart: boolean;
  readonly studioAt: string | null;
  readonly shelves: readonly ShelfRow[];
}

export interface ShelfRow {
  readonly id: string;
  readonly name: string;
  readonly held: readonly HeldRow[];
}

/** One file, understood from its own bytes — never from its name. */
export interface HeldRow {
  readonly file: string;
  readonly kind: string;
  /** `unknown` means nobody measured it. It is never treated as incompatible. */
  readonly base: string;
  readonly bytes: number;
  /** Why nothing could be derived. Information, not a fault. */
  readonly unread: string | null;
  /**
   * Which medium this file makes — `picture`, `video`, `sound`, `model`.
   *
   * `null` is **unplaced**, never *not this one*: a shelf must keep showing such a file whatever
   * medium the reader is filtering for, because there is nothing they can do about a failure of
   * Epoch's and nothing else would tell them why it vanished.
   */
  readonly medium: string | null;
}

/** A machine that can make a picture — this one, or one that lends its card. */
export interface BenchRow {
  /** Empty for this machine; a paired machine's id otherwise. */
  readonly id: string;
  readonly name: string;
  readonly here: boolean;
  /** Measured there, not here. A dark row is paired and not serving right now. */
  readonly serving: boolean;
  readonly models: readonly string[];
}

export interface StyleRow {
  readonly name: string;
  /** Workflow names attached to it, best first. */
  readonly using: readonly string[];
  /** Something here can draw it. `false` keeps the frame and loses the light. */
  readonly lit: boolean;
}

export interface WorkflowRow {
  readonly id: string;
  readonly name: string;
  /** What it can be asked for — read off the graph, never declared. */
  readonly can: readonly string[];
  /** The model files it names. What the Workshop would have to fetch. */
  readonly needs: readonly string[];
  /** Why it cannot run here, usually a custom node nobody installed — named. */
  readonly problem: string | null;
}

/** Take a workflow in. Base64, because the window never touches the filesystem (ADR-0024). */
export async function importWorkflow(
  name: string,
  data: string,
): Promise<string | null> {
  try {
    await invoke<string>("import_workflow", { name, data });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** Point a Style at a workflow, or take it away. */
export async function attachWorkflow(
  style: string,
  id: string,
  attach: boolean,
): Promise<string | null> {
  try {
    await invoke("attach_workflow", { style, id, attach });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** Forget a workflow, and take it out of every Style that pointed at it. */
export async function forgetWorkflow(id: string): Promise<string | null> {
  try {
    await invoke("forget_workflow", { id });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** Which Style this World draws with when nobody says. */
export async function drawUsually(style: string): Promise<string | null> {
  try {
    await invoke("draw_usually", { style });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** Which machine this World draws on. Empty is this one (ADR-0029). */
export async function drawOn(machine: string): Promise<string | null> {
  try {
    await invoke("draw_on", { machine });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** Open the user's own terminal on the command that installs one. */
export async function installStudio(id: string): Promise<string | null> {
  try {
    await invoke("install_studio", { id });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** Start one. Its own first-run questions are the user's to answer. */
export async function startStudio(id: string): Promise<string | null> {
  try {
    await invoke("start_studio", { id });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * Stop the server a studio runs, and say what happened.
 *
 * Measured on the machine this was built for: idle, ComfyUI holds 2.6 GB of system RAM, and
 * asking it to free memory returns none of it — what stays is the interpreter, torch's CUDA
 * context and the node modules, and none of that goes while the process lives.
 *
 * Returns the Engine's own sentence, or the failure. Both are worth showing: *nothing was
 * running from that folder* is an answer.
 */
export async function stopStudio(id: string): Promise<string> {
  try {
    return await invoke<string>("stop_studio", { id });
  } catch (error) {
    return String(error);
  }
}

/**
 * The creations panel closed.
 *
 * The Engine stops the studio **only if it started it** and only when nothing is queued —
 * somebody who launched ComfyUI themselves is using it, and a panel closing here is not a reason
 * to take it away. Returns what happened, or `null` when there was nothing to do.
 */
export async function closeStudio(): Promise<string | null> {
  try {
    return await invoke<string | null>("close_studio");
  } catch {
    // Nothing to report and nothing to fix: the panel is closing either way.
    return null;
  }
}

/** One model that fits this machine, with the largest quantisation that does. */
export interface SuitedModel {
  /** Its place in the list, from 1. */
  readonly rank: number;
  /** The repository as Hugging Face spells it, or the model's own name when it is already here. */
  readonly repo: string;
  /** Which quantisation fits. Empty for one already installed. */
  readonly quant: string;
  readonly bytes: number;
  /** What to pull to get exactly this one. Empty for one already here. */
  readonly pull: string;
  /** What the repository declared it can do, in its own words. */
  readonly facets: readonly string[];
  /** The runtime it is already installed on. `null` means it would be downloaded. */
  readonly here: string | null;
  /**
   * Tokens per second — **measured** when `measuredOn` says where, an estimate otherwise.
   *
   * `null` on a machine that has timed nothing: without one real answer to divide by, there is
   * no honest way to say how fast a model would run here.
   */
  readonly tokensPerSecond: number | null;
  /** Which runtime the number came from. `null` means it is an estimate. */
  readonly measuredOn: string | null;
}

/** How a model's KV cache is kept. `f16` is llama.cpp's own default. */
export type Cache = "f16" | "q8_0";

/** One way of loading a model. */
export interface Loadout {
  readonly context: number;
  readonly cache: Cache;
}

/** One loadout that was tried, and what it answered at. `null` means it did not load. */
export interface Reading extends Loadout {
  readonly tokensPerSecond: number | null;
}

/**
 * One model on this machine, with everything measured about it.
 *
 * Every field is a reading or an absence. A speed appears only when something timed it; a
 * loadout says whether it was searched for or is the safe default; the curve is empty until
 * somebody pays four minutes for it.
 */
export interface ModelHere {
  readonly name: string;
  /** `Ollama` or `Saved here`. The shelf decides what removing it means. */
  readonly from: string;
  readonly path: string;
  readonly bytes: number;
  /** Whether it has a projector beside it — measured from the headers, never the filename. */
  readonly sees: boolean;
  /** What it was trained to hold, which bounds what any loadout may ask for. */
  readonly trainedContext: number | null;
  /** What it will be loaded with the next time llama.cpp starts. */
  readonly loadout: Loadout;
  /** Whether that came from a search, or is the default nobody has improved on. */
  readonly measured: boolean;
  /** The whole curve, when one has been mapped. */
  readonly readings: readonly Reading[];
  /** Which row Epoch marks out of that curve. */
  readonly recommended: Loadout | null;
  readonly tokensPerSecond: number | null;
  readonly measuredOn: string | null;
  /** Which runtime the curve was mapped on. Separate from `measuredOn`, which is a timing. */
  readonly curveOn: string | null;
  /** What this model is told: flash attention, speculation. A choice, never a measurement. */
  readonly tuning: Tuning;
  /** Every other GGUF here, any of which could be its draft. Epoch does not narrow the list. */
  readonly drafts: readonly string[];
  /**
   * Other rows that are these same weights, stored again.
   *
   * Named rather than folded away: both files are really on the disk, and removing one row must
   * not remove the other. Empty for the ordinary case.
   */
  readonly sameWeightsAs: readonly string[];
}

/** One row of steering: which ControlNet, which picture, how hard. */
export interface PanelSteerRow {
  /** The ControlNet, as the server lists it. */
  readonly file: string;
  /**
   * The reference picture, **by the name ComfyUI answered with**.
   *
   * Never a path from this machine: a lent ComfyUI has its own disk, which is why handing a
   * picture over answers with a name rather than taking one.
   */
  readonly image: string;
  /**
   * What turns the reference into something the ControlNet can read, by node class.
   *
   * Three states, and the empty one is not a default: `""` is **nobody has said yet** and
   * GENERATE waits for it, `"already"` means the picture is already a control map, and anything
   * else names the node that makes one. A canny ControlNet handed a photograph draws noise —
   * measured — and neither Epoch nor the picture itself can say which was meant.
   */
  readonly prepare: string;
  /** How hard it steers. `0` means never set, and the Engine reads that as the server's 1.0. */
  readonly strength: number;
}

/** Every model here, with what is known about how it runs. One read, no probing. */
export async function modelsAndLoadouts(): Promise<readonly ModelHere[]> {
  try {
    return await invoke<ModelHere[]>("models_and_loadouts");
  } catch {
    return [];
  }
}

/**
 * Map what one model can do on this card.
 *
 * Eight loads, about four minutes, on a server of its own — never the router the World is using,
 * which would take everybody's brains away mid-turn.
 */
/** One runtime a curve can be mapped on, and how much of the curve it can be told. */
export interface MeasurableOn {
  readonly id: string;
  readonly name: string;
  /** Which axes it can vary, in words: *context and cache*, or *context only*. */
  readonly maps: string;
  readonly models: readonly string[];
}

export async function whoCanMeasure(): Promise<readonly MeasurableOn[]> {
  try {
    return await invoke<readonly MeasurableOn[]>("who_can_measure");
  } catch {
    return [];
  }
}

/** Speculative decoding, as llama.cpp exposes it. `kind` is one of its own `--spec-type` values. */
export interface Speculation {
  readonly kind: string;
  readonly draft: string | null;
  readonly nMax: number | null;
  readonly nMin: number | null;
  readonly pMin: number | null;
  readonly gpuLayers: number | null;
}

/** What a model is told beyond its context and its cache. */
export interface Tuning {
  /** `null` is llama.cpp's own `auto`, which is a real third state and not an absent answer. */
  readonly flashAttn: boolean | null;
  readonly speculation: Speculation;
  /** Where the model's tensors were put. Absent on a row written before this was recorded. */
  readonly placement?: {
    readonly gpuLayers: number | null;
    readonly overrideTensor: string | null;
  };
}

export const NOTHING_TOLD: Tuning = {
  flashAttn: null,
  speculation: {
    kind: "",
    draft: null,
    nMax: null,
    nMin: null,
    pMin: null,
    gpuLayers: null,
  },
};

/** How one run of one trial went. Mirrors `trials::Outcome`. */
export type Outcome =
  | { readonly kind: "reasoning"; readonly correct: boolean }
  | {
      readonly kind: "coding";
      readonly passed: number;
      readonly total: number;
      readonly note: string | null;
    }
  | { readonly kind: "following"; readonly checks: unknown[] }
  | {
      readonly kind: "tools";
      readonly called: boolean;
      readonly rightTool: boolean;
      readonly rightArguments: boolean;
      readonly validShape: boolean;
      readonly ran: boolean;
      readonly rightAnswer: boolean | null;
      readonly note: string | null;
    }
  | { readonly kind: "refused"; readonly why: string };

export interface Answered {
  readonly trial: string;
  readonly model: string;
  /** Everything it said. This is the half a person judges. */
  readonly said: string;
  readonly outcome: Outcome;
}

/** Where and how a card was measured. A number without these is a fact about nothing. */
export interface Conditions {
  readonly suite: number;
  readonly gpu: string;
  readonly build: string;
  readonly runtime: string;
  readonly context: number;
  readonly tuning: Tuning;
  readonly at: number;
}

export interface BenchRun {
  readonly generation: number | null;
  readonly prompt: number | null;
  readonly firstTokenMs: number | null;
  readonly promptTokens: number | null;
  readonly predictedTokens: number | null;
  readonly cachedTokens: number | null;
  readonly drafted: number | null;
  readonly accepted: number | null;
  readonly vramUsed: number | null;
  readonly seconds: number;
  readonly failed: string | null;
}

/** One model's whole result. */
/**
 * One column of a card, scored by the Engine.
 *
 * `answered` and `asked` travel with the share because the denominator moves: a model that ran
 * out of room on two coding trials of three still scores 100% on the one it answered, and the
 * percentage alone cannot say which card you are reading.
 */
export interface BenchColumn {
  readonly kind: "reasoning" | "coding" | "following" | "tools";
  /**
   * Sum of the per-trial scores — a **sum, not a tally**, because a tool call is judged in five
   * or six separate parts and a trial can be four fifths right.
   */
  readonly correct: number;
  /** How many trials of this kind produced an answer at all. */
  readonly answered: number;
  /** How many were asked. */
  readonly asked: number;
  /** `correct / answered`. `null` where it answered nothing — never zero. */
  readonly correctness: number | null;
  /** `answered / asked`. */
  readonly completion: number | null;
  /** `correct / asked`. The one to rank on. */
  readonly effective: number | null;
}

export interface BenchCard {
  readonly model: string;
  readonly conditions: Conditions;
  readonly speed: { readonly runs: readonly BenchRun[] };
  readonly answers: readonly Answered[];
  /** Scored once, by the Engine. The window renders these rather than recomputing them. */
  readonly columns: readonly BenchColumn[];
}

/** What one speculative-decoding configuration measured. Mirrors `spec::Tried`. */
export interface SpecTried {
  readonly label: string;
  readonly measured: { readonly runs: readonly BenchRun[] };
  /** The same configuration asked one question three times — the best case, labelled as one. */
  readonly repeated: { readonly runs: readonly BenchRun[] };
  readonly load: {
    readonly gpuPercent: MachineReading | null;
    readonly vramUsed: MachineReading | null;
    readonly ramUsed: MachineReading | null;
    readonly cpuPercent: MachineReading | null;
    readonly vramBefore: number | null;
    readonly ramBefore: number | null;
    readonly seconds: number;
  };
  /** On unseen text. `null` where either side was never measured — never 1.0. */
  readonly speedup: number | null;
  /** On text the model has already written. */
  readonly speedupRepeating: number | null;
  /** Whether it wrote the same text as the baseline. `null` is unproven, not wrong. */
  readonly sameAnswers: boolean | null;
  readonly stable: boolean;
  readonly failures: readonly string[];
}

/** A peak and a mean, and how many samples they are made of. */
export interface MachineReading {
  readonly peak: number;
  readonly mean: number;
  readonly samples: number;
}

export interface Sweep {
  readonly model: string;
  /** Every configuration in the order it ran, the baseline first. */
  readonly tried: readonly SpecTried[];
  /** `null` where nothing beat the baseline — a real answer, not a missing winner. */
  readonly best: string | null;
}

/** What somebody wants from a model, before knowing what the machine can give them. */
export type Intent =
  | "auto"
  | "balanced"
  | "fast"
  | "maxQuality"
  | "longContext"
  | "custom";

/**
 * One configuration that was really run on this machine.
 *
 * Everything needed to run it again is here, because a profile the user picks has to be writable
 * back out as flags without anybody re-deriving it. That is the difference between a measurement
 * and a recommendation.
 */
export interface Configuration {
  readonly loadout: { readonly context: number; readonly cache: string };
  readonly tuning: Tuning;
  readonly offload: string | null;
  readonly gpuLayers: number | null;
  readonly generation: number;
  readonly prompt: number | null;
  readonly firstTokenMs: number | null;
  readonly vramUsed: number | null;
  readonly ramUsed: number | null;
  readonly stable: boolean;
  /**
   * What the runs of this configuration amounted to.
   *
   * **This is what a profile is chosen on, not the peak.** Measured: the same configuration
   * answered at 48.3, 44.2 and 25.0 across three searches, because a model instance can be evicted
   * from the card partway through and never recover. A peak is the best moment of whichever
   * instance survived; a median with a spread beside it is the configuration.
   */
  readonly verdict: Verdict;
  readonly around: {
    readonly gpuMean: number | null;
    readonly sharedBefore: number | null;
    readonly sharedAfter: number | null;
    readonly vramBefore: number | null;
    readonly vramPeak: number | null;
  };
  /** `null` is unproven, never wrong. */
  readonly sameAnswers: boolean | null;
  /**
   * The controls on either side of this candidate.
   *
   * **Measurement is not eligibility.** A reading can be perfect and still not be a thing to
   * recommend, because nothing in it says the machine was in the same state afterwards. The
   * Engine has filtered on this since brackets existed; this side had never been given the field,
   * so the panel could mark a contaminated row ★ that `use_profile` would then refuse to write.
   *
   * `null` for a row measured before brackets, which is not eligible either — an absent control
   * is not a control that held.
   */
  readonly bracket: Bracket | null;
  /**
   * How many prompt tokens were actually in the window while this was measured.
   *
   * **The workload this row belongs to, and two of them may never be compared.** `null` is the
   * short workload: a question. A number is the server's own count with the window filled.
   */
  readonly filled: number | null;
  readonly at: number;
}

/** Whether a control reproduced on one side of a candidate. */
export type Held = "reproduced" | "failed" | "missing";

export interface Bracket {
  /** The observation ids, so a row points at the readings rather than repeating them. */
  readonly before: string | null;
  readonly beforeHeld: Held;
  readonly after: string | null;
  readonly afterHeld: Held;
}

/** What a configuration turned out to be. */
export type ConfigState = "stable" | "degraded" | "unstable" | "invalid";

export interface Verdict {
  readonly stateOf: ConfigState | null;
  readonly median: number | null;
  readonly fastest: number | null;
  readonly slowest: number | null;
  readonly spread: number | null;
  readonly kept: number;
  /** Runs thrown away for any reason. */
  readonly discarded: number;
  /** How many of those were collapses rather than plain failures. */
  readonly collapses: number;
}

/** Everything measured for one model here. */
export interface Optimized {
  readonly model: string;
  readonly gpu: string;
  readonly build: string;
  readonly runtime: string;
  /** The unconfigured run — what an improvement is measured against. */
  readonly baseline: Configuration | null;
  readonly tried: readonly Configuration[];
  readonly custom: Configuration | null;
  /**
   * Why this search produced no context curve, where it tried and could not.
   *
   * `null` means it ran, or was never reached. A refusal used to travel as a progress event,
   * which arrives after the panel has moved on — so a stage that silently did not happen looked
   * exactly like one nobody asked for.
   */
  readonly ladder?: string | null;
  /** Which intent the user chose. `null` until they choose one. */
  readonly chosen: Intent | null;
  /** The search this came from, and whether the machine stayed the same throughout. */
  readonly session: Session | null;
  /**
   * Whether these rows may become profiles.
   *
   * **A degraded session produces none.** Its rows are kept because they are the diagnosis — the
   * candidate that broke the environment is named — and nothing measured after the control
   * stopped reproducing may become something somebody runs for hours.
   */
  readonly usable: boolean;
  /**
   * Measured, not offered, and named rather than absent.
   *
   * **The Engine decides this and nothing here recomputes it.** A second place applying the rule
   * would agree with the first only until one of them learned something.
   */
  readonly aside?: readonly WasAside[];
  /** What each intent resolves to. Resolved once, in the Engine. */
  readonly profiles?: readonly Profile[];
}

/** Why one measured configuration is not among the ones Epoch offers by itself. */
export type Aside =
  /** The runs disagreed too much, and the range overlaps something steadier. */
  | "varied"
  /** The instance broke partway through. Named, never offered. */
  | "collapsed"
  /** It changed what the model answered. */
  | "changedAnswers"
  /** No control, or one that did not reproduce. */
  | "unwitnessed"
  /** The whole session ended on a machine that stopped reproducing. */
  | "sessionDegraded";

export interface WasAside {
  readonly configuration: Configuration;
  readonly why: Aside;
}

/**
 * What one intent resolved to, **decided by the Engine**.
 *
 * The panel used to resolve these itself. It stopped the day the Engine learned that a range
 * entirely above another wins whatever it is labelled, and the panel did not — so the deck showed
 * a configuration the Engine had already stopped choosing.
 */
export interface Profile {
  readonly intent: Intent;
  readonly name: string;
  readonly about: string;
  /** Where this resolved to the same configuration as an earlier intent. */
  readonly sameAs: Intent | null;
  readonly configuration: Configuration;
}

export type SessionState =
  | "clean"
  | "running"
  | "degraded"
  /** *I know what healthy looked like and I am not there.* A claim about the machine. */
  | "recoveryRequired"
  /**
   * *I have nothing comparable enough to tell.* A claim about the record.
   *
   * Separate from `recoveryRequired` because telling somebody to restart their machine over
   * this would send them hunting a fault nobody has shown exists. Calibration is permitted
   * here; comparative profiles are not.
   */
  | "referenceRequired"
  | "complete"
  /**
   * Somebody stopped it.
   *
   * **Not a verdict about anything measured.** Every candidate that ran was closed by its own
   * control, and each one's bracket decides whether it may be recommended — so cancelling costs
   * no evidence. What this records is that the search did not reach the end, which `complete`
   * would have quietly denied.
   */
  | "cancelled"
  | "invalid";

/** One reading of the control, and what it sat after. */
export interface Control {
  readonly rate: number | null;
  readonly around: string | null;
  readonly at: number;
}

export interface Session {
  readonly artifact: string;
  readonly gpu: string;
  readonly build: string;
  readonly runtime: string;
  readonly controls: readonly Control[];
  readonly stateOf: SessionState | null;
  /** Which candidate the control stopped reproducing after. */
  readonly trigger: string | null;
  /** What Epoch would say to the person watching. */
  readonly note: string | null;
}

/** How a search is going, while it goes. */
/**
 * Which of the four things a benchmark is doing.
 *
 * Named for what the user sees rather than for the code that runs: `establishingBaseline` covers
 * a calibration series, a clean-state check, or nothing at all when a reference already governs
 * — three quite different amounts of work that answer one question a person actually has.
 */
export type Stage =
  | "checkingHardware"
  | "establishingBaseline"
  | "testingConfigurations"
  | "validatingStability";

/** In the order they happen, so a checklist can tick the earlier ones. */
export const STAGES: readonly { readonly id: Stage; readonly label: string }[] = [
  { id: "checkingHardware", label: "Checking hardware" },
  { id: "establishingBaseline", label: "Establishing baseline" },
  { id: "testingConfigurations", label: "Testing configurations" },
  { id: "validatingStability", label: "Validating stability" },
];

export interface Optimizing {
  readonly stage: Stage;
  readonly model: string;
  readonly machine: string;
  readonly done: number;
  readonly total: number;
  /** What is being tried, in words. */
  readonly about: string;
  /** The flags behind those words, for `show technical details`. */
  readonly technical: string;
  readonly best: Configuration | null;
}

/** A short reading of the configuration a model has right now. */
export interface Quick {
  readonly model: string;
  readonly generation: number | null;
  readonly prompt: number | null;
  readonly firstTokenMs: number | null;
  readonly context: number;
  readonly vramUsed: number | null;
  readonly ramUsed: number | null;
  readonly gpuPercent: number | null;
  readonly cpuPercent: number | null;
  readonly healthy: boolean;
  readonly failures: readonly string[];
}

/**
 * Run the search for one model.
 *
 * **Twenty-odd model loads.** Steps arrive on `models:optimizing`; `stopOptimizing` ends it after
 * the configuration it is on.
 */
export async function optimizeModel(model: string): Promise<Optimized | string> {
  try {
    return await invoke<Optimized>("optimize_model", { model });
  } catch (error) {
    return String(error);
  }
}

/** What the machine is doing, before somebody commits half an hour to measuring it. */
export interface Preflight {
  /** Whether a benchmark would start straight away rather than waiting. */
  readonly quiet: boolean;
  /** One sentence naming what is in the way, or that nothing is. */
  readonly says: string;
  readonly cpu: number | null;
  readonly gpu: number | null;
  readonly vramHeld: number | null;
  /** Other servers holding a model, each named. Empty is a measurement. */
  readonly neighbours: readonly string[];
}

/**
 * Ask what the machine is doing right now.
 *
 * **On demand, never kept.** It is two samples over three seconds, and a reading from when the
 * deck opened would describe a machine that has since started compiling something.
 */
export async function beforeBenchmarking(): Promise<Preflight | null> {
  try {
    return await invoke<Preflight>("before_benchmarking");
  } catch {
    // A machine that could not be asked is not a busy one. The search checks again anyway, and
    // it waits rather than refusing — so nothing here is worth stopping somebody over.
    return null;
  }
}

/**
 * Throw away a paused benchmark, because somebody pressed CANCEL.
 *
 * **The record, not the evidence.** Every control it was built from is an Observation and stays
 * where it is. What goes is the session and its banner — and it may go, because it gates nothing
 * any more.
 */
export async function dismissPause(artifact: string): Promise<void> {
  try {
    await invoke("dismiss_pause", { artifact });
  } catch {
    // A banner that could not be thrown away is not worth an error on top of it.
  }
}

/**
 * Exactly what a choice of tuning writes into llama.cpp's preset.
 *
 * **Generated by the function that writes it**, so the label cannot drift from the file. A panel
 * that assembled its own version of the line would eventually show one thing and write another,
 * which is how a benchmark came to report a compressed cache it never ran.
 */
export async function tuningWrites(tuning: Tuning): Promise<string> {
  try {
    return await invoke<string>("tuning_writes", { tuning });
  } catch {
    return "";
  }
}

export async function stopOptimizing(): Promise<void> {
  try {
    await invoke("stop_optimizing");
  } catch {
    // Asking a search that already ended to stop is not a failure worth reporting.
  }
}

export async function quickTest(model: string): Promise<Quick | string> {
  try {
    return await invoke<Quick>("quick_test", { model });
  } catch (error) {
    return String(error);
  }
}

/** What has been measured for one model here. `null` where nothing has. */
export async function optimization(model: string): Promise<Optimized | null> {
  try {
    return await invoke<Optimized | null>("optimization", { model });
  } catch {
    return null;
  }
}

/**
 * Put one measured configuration into effect, chosen rather than offered.
 *
 * Addressed by `at` — when it ran — because the rows are reordered by every rule that reads them.
 */
export async function useMeasured(model: string, at: number): Promise<string> {
  return invoke<string>("use_measured", { model, at });
}

export async function useProfile(model: string, intent: Intent): Promise<string> {
  try {
    return await invoke<string>("use_profile", { model, intent });
  } catch (error) {
    return String(error);
  }
}

export async function forgetOptimization(model: string): Promise<string> {
  try {
    return await invoke<string>("forget_optimization", { model });
  } catch (error) {
    return String(error);
  }
}

/** A search that stopped and is waiting for a clean GPU state. */
export interface PausedBenchmark {
  readonly session: Session;
  readonly lastControl: number | null;
  /**
   * What this machine produced while it was known healthy.
   *
   * `null` where nothing reliable was ever measured here — and then it is **not shown**. A line
   * reading `known clean reference: —` is worse than no line: it invites somebody to wonder what
   * the reference was.
   */
  readonly cleanReferenceSeen: number | null;
  /**
   * **Which** figure it was measured against, by id.
   *
   * The number beside it is an audit snapshot of what the banner showed that day. Nothing
   * decides anything from it — that was the defect: one fact written in two places, and the
   * gate reading the empty one.
   */
  readonly cleanReferenceId: string | null;
  readonly vramUsed: number | null;
  readonly sharedUsed: number | null;
  readonly at: number;
}

/**
 * Measure this machine's healthy figure, and record what it is a figure about.
 *
 * The only thing `referenceRequired` permits. It clears nothing: whether the machine is healthy
 * is the next question, and it is the user's to ask by running the check.
 */
export async function calibrate(model: string): Promise<string> {
  try {
    return await invoke<string>("calibrate", { model });
  } catch (error) {
    return String(error);
  }
}

/** Whether an interrupted benchmark is waiting. `null` is the ordinary case. */
export async function pausedBenchmark(): Promise<PausedBenchmark | null> {
  try {
    return await invoke<PausedBenchmark | null>("paused_benchmark");
  } catch {
    return null;
  }
}

/**
 * Run only the control and find out whether this machine is itself again.
 *
 * **Resumes nothing.** A clean answer clears the pause and the next search starts from the
 * beginning; the rows before the break and the rows after it came from two different machines.
 */
export async function cleanStateCheck(model: string): Promise<string> {
  try {
    return await invoke<string>("clean_state_check", { model });
  } catch (error) {
    return String(error);
  }
}

/**
 * Which speculative decodings the installed llama.cpp offers, in its own words.
 *
 * **Asked rather than listed here.** `--spec-type` grew from three values to eleven between
 * releases, and a copy in the frontend would be a second place to keep in step.
 */
export async function speculationTypes(): Promise<readonly string[]> {
  try {
    return await invoke<readonly string[]>("speculation_types");
  } catch {
    return [];
  }
}

/**
 * Measure every speculative decoding this build offers, for one model.
 *
 * **Far longer than a benchmark** — each configuration restarts llama.cpp and reloads the
 * model — and it is asked for explicitly for that reason.
 */
export async function sweepSpeculation(model: string): Promise<Sweep | string> {
  try {
    return await invoke<Sweep>("sweep_speculation", { model });
  } catch (error) {
    return String(error);
  }
}

export async function benchmark(model: string, on: string): Promise<string> {
  try {
    return await invoke<string>("benchmark", { model, on });
  } catch (error) {
    return String(error);
  }
}

export async function benchmarks(): Promise<readonly BenchCard[]> {
  try {
    return await invoke<readonly BenchCard[]>("benchmarks");
  } catch {
    return [];
  }
}

export async function forgetBenchmarks(model: string): Promise<string> {
  try {
    return await invoke<string>("forget_benchmarks", { model });
  } catch (error) {
    return String(error);
  }
}

export async function tuneModel(model: string, tuning: Tuning): Promise<string> {
  try {
    return await invoke<string>("tune_model", { model, tuning });
  } catch (error) {
    return String(error);
  }
}

export async function searchLoadout(path: string, on: string): Promise<string> {
  return await invoke<string>("search_loadout", { path, on });
}

/** Take the user's pick out of a measured curve. */
export async function chooseLoadout(
  path: string,
  context: number,
): Promise<string> {
  return await invoke<string>("choose_loadout", { path, context });
}

/**
 * Take one model off this machine.
 *
 * Destructive and not undoable, so the caller confirms first. The Engine matches the path
 * against what it reported holding rather than trusting it.
 */
export async function removeModel(path: string): Promise<string> {
  return await invoke<string>("remove_model", { path });
}

/**
 * Time one model on the runtime serving it, and remember the answer.
 *
 * A measurement rather than an estimate, and one of them turns every other size on the list into
 * a speed. Costs a load and two short answers.
 */
export async function timeModel(model: string, on: string): Promise<string> {
  try {
    return await invoke<string>("time_model", { model, on });
  } catch (error) {
    return String(error);
  }
}

/** One runtime that is answering, and which of this deck's models it could time. */
export interface TimeableOn {
  /** Identifies the sign-in of a runtime. Never taken apart to work out which program it is. */
  readonly id: string;
  readonly name: string;
  /**
   * Which computer it is on. `null` is this machine.
   *
   * Two runtimes of the same program on two machines are two entries with one name, and a
   * picker that showed `llama.cpp, Ollama, llama.cpp` names nothing at all.
   */
  readonly machine: string | null;
  /** The deck's own names, so a row can match itself without knowing what a server calls it. */
  readonly models: readonly string[];
}

/**
 * Which runtimes could time each model.
 *
 * Its own call because it probes, and `modelsAndLoadouts` deliberately does not: opening the
 * models deck must not cost a graphics card.
 */
export async function whoCanTime(): Promise<readonly TimeableOn[]> {
  try {
    return await invoke<TimeableOn[]>("who_can_time");
  } catch {
    return [];
  }
}

/**
 * What this machine can actually run, most used first.
 *
 * Epoch measures the fitting; the **order is Hugging Face's own** `sort=downloads`. There is no
 * measurement of whether a model is good, so nothing here is a ranking of quality — a list
 * ordered by something Epoch invented, with a download button under it, would be the gauge
 * nobody can explain.
 *
 * A moment's work: one search and one request per repository. Hence a button.
 */
export async function suitedModels(): Promise<SuitedModel[] | string> {
  try {
    return await invoke<SuitedModel[]>("suited_models");
  } catch (error) {
    return String(error);
  }
}

/** One GPU backend installed on this machine. */
export interface EngineRow {
  readonly id: string;
  readonly name: string;
  /** Whether the program itself says this is the one in use. LM Studio only. */
  readonly chosen: boolean;
}

/**
 * What a runtime's backends are here.
 *
 * `unknown` is *nobody could read it* and never "there is only CPU" — the same distinction the
 * agent readings keep between signed-out and could-not-ask.
 */
export type Engines =
  | { readonly kind: "choice"; readonly of: readonly EngineRow[] }
  | { readonly kind: "fixed"; readonly of: string }
  | { readonly kind: "unknown" };

export interface RuntimeEngines {
  readonly id: string;
  readonly name: string;
  readonly engines: Engines;
  /** What the user picked. `null` is the program choosing for itself. */
  readonly chose: string | null;
  readonly compressedCache: boolean;
  readonly canCompress: boolean;
}

/** How Epoch would add one program. `null` where it cannot. */
export type How =
  | { readonly kind: "winget"; readonly id: string }
  | { readonly kind: "brew"; readonly id: string }
  | { readonly kind: "npm"; readonly id: string };

/** One thing First Run can find, or add. */
export interface Offer {
  readonly id: string;
  readonly name: string;
  /** One line: what it gives this machine. */
  readonly what: string;
  /** Where it was found. `null` is *not here*, and is what makes it offerable. */
  readonly found: string | null;
  readonly how: How | null;
  /** Why Epoch cannot add it, when it cannot. */
  readonly whyNot: string | null;
  readonly needs: readonly string[];
}

export async function firstRunSurvey(): Promise<readonly Offer[]> {
  try {
    return await invoke<readonly Offer[]>("first_run_survey");
  } catch {
    return [];
  }
}

/** Add the chosen programs. Answers with whatever refused, in each program's own words. */
export async function firstRunInstall(
  wanted: readonly string[],
): Promise<readonly string[]> {
  try {
    return await invoke<readonly string[]>("first_run_install", { wanted });
  } catch (error) {
    return [String(error)];
  }
}

export async function runtimeEngines(): Promise<readonly RuntimeEngines[]> {
  try {
    return await invoke<readonly RuntimeEngines[]>("runtime_engines");
  } catch {
    return [];
  }
}

export async function chooseEngine(
  id: string,
  engine: string | null,
): Promise<string> {
  try {
    return await invoke<string>("choose_engine", { id, engine });
  } catch (error) {
    return String(error);
  }
}

export async function compressCache(id: string, on: boolean): Promise<string> {
  try {
    return await invoke<string>("compress_cache", { id, on });
  } catch (error) {
    return String(error);
  }
}

/** After a start: wait for the server and prove the compressed cache loads. */
export async function proveRuntime(id: string): Promise<string | null> {
  try {
    return await invoke<string | null>("prove_runtime", { id });
  } catch (error) {
    return String(error);
  }
}

export async function startRuntime(id: string): Promise<string | null> {
  try {
    await invoke("start_runtime", { id });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** One model Ollama already holds, as a file another runtime can open. */
export interface SharedWeights {
  /** What a person recognises it by — `qwen3:14b`, or the file's own name. */
  readonly name: string;
  /**
   * Which shelf it is on: `Ollama` or `Saved here`.
   *
   * Two shelves, and they behave differently — which is why it is on screen rather than implied.
   * A model Ollama pulled is usable by Ollama at once; a GGUF the Workshop saved is a file, and
   * until something opens it, it is a file nobody is using.
   */
  readonly from: string;
  /** The file itself. Offered back to `serveWeights`, and checked there against this list. */
  readonly path: string;
  readonly bytes: number;
  /**
   * The vision projector beside it, when the model has one.
   *
   * A vision model is two files, and a model on a shelf without its second half loads happily
   * and then refuses a picture. `null` means it has no eyes, not that nobody looked.
   */
  readonly seesWith: string | null;
}

/**
 * Every model Ollama already has on disk.
 *
 * Ollama stores unmodified GGUF, measured: the blob begins `GGUF` and `llama-server -m <blob>`
 * loaded `qwen3:14b` and answered in 2.4 s on the card. So a second runtime costs no download
 * and no second copy of nine gigabytes.
 */
export async function sharedWeights(): Promise<readonly SharedWeights[]> {
  try {
    return await invoke<SharedWeights[]>("shared_weights");
  } catch {
    return [];
  }
}

/**
 * Give Ollama a GGUF it did not download.
 *
 * Measured: it costs **no disk** — Ollama stores by content hash, so a file it already holds
 * adds a manifest and nothing else — and it costs **about three and a half minutes** for nine
 * gigabytes, because the whole file is hashed to find that out. Hence a terminal rather than a
 * spinner.
 */
export async function importWeights(path: string): Promise<string | null> {
  try {
    await invoke("import_weights", { path });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * Start llama.cpp knowing about **every** model on this machine.
 *
 * Measured 2026-08-21: `llama-server --models-dir` is a router. It reports every model in the
 * directory, loads one only when a request names it, and leaves the rest available — so there is
 * nothing to pick before starting it. Returns what to say, or why it could not.
 */
export async function startRouter(): Promise<string> {
  try {
    return await invoke<string>("start_router");
  } catch (error) {
    return String(error);
  }
}

/**
 * Put every model this machine has on LM Studio's shelf.
 *
 * The same answer in the shape LM Studio takes: it loads from its own directory on its own
 * terms, so what it needs is for everything to be there. Hard links, so it costs no disk and
 * Ollama keeps its own — and never `lms import`, which defaults to *moving* the file.
 */
export async function lendAllToLmStudio(): Promise<string> {
  try {
    return await invoke<string>("lend_all_to_lm_studio");
  } catch (error) {
    return String(error);
  }
}

/** Open the user's own terminal on the command that installs one. */
export async function installRuntime(id: string): Promise<string | null> {
  try {
    await invoke("install_runtime", { id });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** Start a runtime serving one weighed model, in the user's own terminal. */
export async function serveModel(
  id: string,
  pull: string,
): Promise<string | null> {
  try {
    await invoke("serve_model", { id, pull });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * Whether this machine has the Hugging Face CLI, and who it is signed in as.
 *
 * Two facts with two fixes, like every agent's: installing, and signing in.
 */
export async function huggingFaceCli(): Promise<HuggingFaceCli> {
  try {
    return await invoke<HuggingFaceCli>("hugging_face");
  } catch {
    return { installed: false, version: null, foundAt: null, user: null };
  }
}

export interface HuggingFaceCli {
  readonly installed: boolean;
  readonly version: string | null;
  /** Where it was found, so "not installed" is a fact somebody can check. */
  readonly foundAt: string | null;
  readonly user: string | null;
}

/**
 * The Hugging Face CLI on every machine this World can use, measured on each.
 *
 * One call rather than one per machine: which machines exist is a question the Engine already
 * answers, and a surface enumerating the roster itself would be a second place that knows what
 * a fleet is.
 */
export async function huggingFaceEverywhere(): Promise<HuggingFaceRow[]> {
  try {
    return await invoke<HuggingFaceRow[]>("hugging_face_everywhere");
  } catch {
    return [];
  }
}

export interface HuggingFaceRow {
  readonly machine: string;
  /** The machine Epoch runs on. Always first, and always answers. */
  readonly local: boolean;
  /** Whether it could be asked at all — a different fact from whether it has `hf`. */
  readonly reached: boolean;
  readonly installed: boolean;
  readonly version: string | null;
  readonly foundAt: string | null;
  readonly user: string | null;
}

/**
 * What a download would fetch, before anything is fetched.
 *
 * A quantisation filter is a glob, and one that matches nothing downloads nothing while looking
 * like success. Asking first is what turns that into a sentence.
 */
export async function planDownload(
  repo: string,
  quant: string,
): Promise<{ file: string; size: string }[] | string> {
  try {
    return await invoke<{ file: string; size: string }[]>("plan_download", {
      repo,
      quant,
    });
  } catch (error) {
    return String(error);
  }
}

/** Fetch one quantisation onto this machine. Reports on `models:downloaded`. */
export async function downloadFile(
  repo: string,
  quant: string,
): Promise<string | null> {
  try {
    await invoke("download_file", { repo, quant });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * Every quantisation a Hugging Face repository publishes.
 *
 * Asked only when somebody opens a result: a search returns sixty repositories, and asking each
 * what it contains would be sixty requests for a list nobody may look at.
 */
export async function fetchModelVariants(
  repo: string,
): Promise<ModelVariantGroup[] | string> {
  try {
    return await invoke<ModelVariantGroup[]>("model_variants", { repo });
  } catch (error) {
    return String(error);
  }
}

export interface ModelVariantGroup {
  /** `1`, `4`, `16`. Zero when the name says nothing about width. */
  readonly bits: number;
  readonly variants: readonly {
    readonly quant: string;
    /** Bytes, summed across shards — a part on its own is not something anybody can run. */
    readonly bytes: number;
    readonly files: number;
    readonly pull: string;
    /** `MTP` marks a module published beside the model, not a version of it. */
    readonly tag: string | null;
    /**
     * The same quantisation **with the model's own prediction head in it**.
     *
     * Its own field because `tag` already means the opposite: that marks a 1.37 GB module beside
     * a 27B, and this marks a whole model that carries the head — the plain file plus one block,
     * 0.48 GB more on `UD-IQ4_XS`.
     *
     * It matters at download time and only then: the head is weights, so choosing the plain file
     * puts `--spec-type draft-mtp` out of reach of that model for ever.
     */
    readonly ownHead: boolean;
    /**
     * Whether Ollama can fetch this one.
     *
     * Measured against its own refusal: a sharded tag answers *"Ollama does not yet support
     * pulling sharded GGUF via the registry"*. The file is still fetchable with `hf`.
     */
    readonly pullable: boolean;
  }[];
}

/**
 * What one model really costs, and whether it fits here.
 *
 * Any name: the manifest endpoint takes any, so somebody who knows what they want types it
 * rather than hunting for it in a short featured list that may not hold it.
 */
export async function weighModel(name: string): Promise<ModelOffer | string> {
  try {
    return await invoke<ModelOffer>("weigh_model", { name });
  } catch (error) {
    return String(error);
  }
}

/** What Epoch is holding that you might want back, measured now. */
export async function fetchErasable(): Promise<readonly ErasableView[]> {
  try {
    return await invoke<ErasableView[]>("erasable");
  } catch {
    return [];
  }
}

/**
 * Erase the things you ticked, and only those.
 *
 * Ids rather than paths: this sends back which lines were ticked, and the Engine decides what
 * each one means now. A window handing back a list of files to delete would be a window
 * somebody could change.
 */
export async function erase(ids: readonly string[]): Promise<string | null> {
  try {
    const problems = await invoke<string[]>("erase", { ids });
    return problems.length > 0 ? problems.join("; ") : null;
  } catch (error) {
    return String(error);
  }
}

/**
 * What removing a World would do — asked **before** anything is deleted.
 *
 * The plan whose *consequences* list is longer than its file list. A World's id lives in eight
 * places and two of them point at folders the user chose, so this is where those are named as
 * things that survive.
 */
export async function worldRemoval(
  worldId: string,
): Promise<RemovalView | string> {
  try {
    return await invoke<RemovalView>("world_removal", { worldId });
  } catch (error) {
    return String(error);
  }
}

/**
 * What exporting a World would carry — asked **before** anything leaves.
 *
 * The plan whose *leaves* list matters more than its file list. An export is a distribution, so
 * this is where a missing licence stops it and where the crew is named as staying behind.
 */
export async function worldExport(
  worldId: string,
): Promise<ExportView | string> {
  try {
    return await invoke<ExportView>("world_export", { worldId });
  } catch (error) {
    return String(error);
  }
}

/**
 * Write one World out as an archive, wherever the user says.
 *
 * The destination dialog is opened by the Engine, not here: `rfd` has no JavaScript surface, so
 * the presentation layer still cannot open anything or learn a path it was not handed
 * (ADR-0024). It asks the shell to ask the user.
 *
 * `null` means the dialog was dismissed. **Cancelling is an answer**, not an error, and a
 * surface that showed a red line for it would be reporting a fault the user caused on purpose.
 */
export async function exportWorld(
  worldId: string,
): Promise<{ at: string | null } | string> {
  try {
    return { at: await invoke<string | null>("export_world", { worldId }) };
  } catch (error) {
    return String(error);
  }
}

/** Remove a World, after the user accepted what it does. */
export async function removeWorld(worldId: string): Promise<string | null> {
  try {
    const problems = await invoke<string[]>("remove_world", { worldId });
    return problems.length > 0 ? problems.join("; ") : null;
  } catch (error) {
    return String(error);
  }
}

/**
 * Throw this machine's door token away and mint a new one.
 *
 * Every door already open stops working, which is the point: a token is regenerated because the
 * old one may have escaped. A regeneration that let the old one keep working would be a button
 * that reassures without protecting.
 */
export async function regenerateDoor(): Promise<string | null> {
  try {
    await invoke("regenerate_door");
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * Ask the shell to open a native folder picker.
 *
 * The webview cannot do this itself — `<input webkitdirectory>` yields files and no path,
 * which browsers strip deliberately. Rust opens the dialog; the frontend still has no
 * filesystem access of its own (ADR-0024).
 *
 * Returns null when the dialog is dismissed. Cancelling is an answer.
 */
export async function chooseFolder(
  start: string | null,
): Promise<string | null> {
  try {
    return (await invoke<string | null>("choose_folder", { start })) ?? null;
  } catch {
    return null;
  }
}

/** Point a World at the folder it works in, or clear it (ADR-0025). */
export async function setProjectRoot(
  id: string,
  path: string | null,
): Promise<string | null> {
  try {
    await invoke("set_project_root", { id, path });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * Choose the folder a World reads its notes from.
 *
 * Its own command rather than `chooseFolder` with a different label: the dialog's title is the
 * whole explanation of what is being chosen, and one picker saying "the folder this World works
 * in" for both would tell the user the wrong thing about their vault.
 */
export async function chooseLibrary(
  start: string | null,
): Promise<string | null> {
  try {
    return (await invoke<string | null>("choose_library", { start })) ?? null;
  } catch {
    return null;
  }
}

/** Point a World at the library it reads from, or clear it. */
export async function setLibrary(
  id: string,
  path: string | null,
): Promise<string | null> {
  try {
    await invoke("set_library", { id, path });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** What is in a folder somebody is about to hand a World as its library. Facts, not a verdict. */
export interface LibraryScan {
  readonly obsidian: boolean;
  readonly notes: number;
  /** True when counting stopped at a cap, so `notes` reads as "at least". */
  readonly more: boolean;
}

/**
 * Open a World's library where the user actually reads it.
 *
 * Resolves to true when Obsidian took it, false when the folder was opened instead. The caller
 * says which happened: somebody who clicked expecting Obsidian and got a file manager is owed
 * the reason.
 */
export async function openLibrary(
  id: string | null = null,
): Promise<boolean | string> {
  try {
    return await invoke<boolean>("open_library", { id });
  } catch (error) {
    return String(error);
  }
}

export async function scanLibrary(path: string): Promise<LibraryScan | null> {
  try {
    return await invoke<LibraryScan>("scan_library", { path });
  } catch {
    return null;
  }
}

export async function renameOrchestrator(name: string): Promise<string | null> {
  try {
    await invoke("rename_orchestrator", { name });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** Give the orchestrator a portrait, or pass `null` to remove it. */
export async function setOrchestratorPortrait(
  image: string | null,
): Promise<string | null> {
  try {
    await invoke("set_orchestrator_portrait", { image });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * What every Provider can currently do.
 *
 * Its own call rather than part of the Launcher survey: probing reaches the network, and the
 * list of installed Worlds must not wait on it. An empty list means the Engine is not
 * answering at all — which is different from every provider being offline, and reads that way.
 */
export async function fetchProviders(): Promise<readonly ProviderStatus[]> {
  try {
    return await invoke<ProviderStatus[]>("list_providers");
  } catch {
    return [];
  }
}

/**
 * What is configured — which is a different question from what answered.
 *
 * `fetchProviders` probes; this reads the file. A backend can be configured and offline, and
 * the Connections deck has to show both facts at once or "OFFLINE" would look like "missing".
 */
export async function fetchBackends(): Promise<BackendsView> {
  try {
    return await invoke<BackendsView>("list_backends");
  } catch (error) {
    // Not an empty list: nothing configured and the Engine not answering are different states,
    // and only one of them is the user's doing.
    return { backends: [], problem: String(error) };
  }
}

/**
 * Add or change one, by id. Resolves to null on success, or the Engine's reason.
 *
 * Nothing is validated here. The Engine owns what a valid backend is; a surface that decided
 * separately would eventually disagree with the file on disk (ADR-0023).
 */
export async function saveBackend(backend: Backend): Promise<string | null> {
  try {
    await invoke("save_backend", { backend });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * Store a backend's credential, or clear it by passing an empty string.
 *
 * One-way. There is no call that reads a key back, and there is deliberately no place to put
 * one here: the value goes to the operating system's encrypted store and the interface is only
 * ever told *that* a key exists (ADR-0026).
 */
export async function saveBackendKey(
  id: string,
  key: string,
): Promise<string | null> {
  try {
    await invoke("save_backend_key", { id, key });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** Forget one. Characters assigned to it keep their assignment. */
export async function forgetBackend(id: string): Promise<string | null> {
  try {
    await invoke("forget_backend", { id });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * Every configured MCP server. Reads the file; starts nothing.
 *
 * Deliberately separate from {@link probeMcp}, which starts the servers to ask them what they
 * offer. One call that did both would spawn processes just by opening a screen.
 */
export async function fetchMcp(): Promise<McpView> {
  try {
    return await invoke<McpView>("list_mcp");
  } catch (error) {
    return { servers: [], problem: String(error) };
  }
}

/**
 * What the outside tools currently amount to: the capability ids they contribute, and
 * everything that could not be offered and why.
 *
 * **This starts the servers.** A server's tools are whatever the server says they are, and
 * asking is the only honest way to find out.
 */
/** What an agent is, and whether this machine has it. Measured, never assumed. */
export interface AgentStatus {
  readonly id: string;
  /**
   * Which program this is — `claude-code`, `codex`, `gemini`.
   *
   * Separate from `id`, which answers *which account*. Two Claude Code sign-ins are two ids
   * and one kind, and everything that reasons about the program must key on this one.
   */
  readonly kind: string;
  readonly name: string;
  readonly installed: boolean;
  readonly version: string | null;
  readonly lookedIn: string | null;
  readonly note: string | null;
  /**
   * Whether it is signed in. `null` when the question could not be asked.
   *
   * Separate from `installed` because the fixes are different — one is an install, the other a
   * sign-in — and a surface that merged them would send somebody to the wrong one.
   */
  readonly signedIn: boolean | null;
  /**
   * Which sign-in method the user chose, when that is readable and the credential is not.
   *
   * Not `account`, which answers *who* and implies a working session. This answers only *what
   * did they pick* — what remains readable when a CLI has no way to be asked.
   */
  readonly method?: string | null;
  /** Which account, as the agent reports it. Never a credential. */
  readonly account: string | null;
}

/**
 * A signed-in agent's account allowance, as that agent reported it locally.
 *
 * This is not a conversation token count. `null` from {@link fetchAgentPlanUsage} means the
 * installed agent did not expose a safe machine-readable allowance at the time of the request.
 */
export interface AgentPlanUsage {
  readonly remainingPercent: number;
  readonly usedPercent: number;
  readonly windowMinutes: number | null;
  readonly resetsAt: number | null;
  /**
   * What is left to spend once the window is exhausted, when the agent keeps such a balance.
   *
   * **`null` is the ordinary answer, and it is not zero.** Claude Code reports subscription
   * percentages and no balance at all; Codex reports one. An agent with no concept of credit
   * shows nothing rather than an empty purse.
   */
  readonly credits: AgentCredits | null;
}

/** A balance an agent keeps beyond its window. */
export interface AgentCredits {
  /** As the agent counts it. **Not assumed to be money** — it names no currency, so nor do we. */
  readonly balance: number;
  /** The agent said it does not run out. Said as such, never rendered as a large number. */
  readonly unlimited: boolean;
}

/**
 * Open an agent's own sign-in, in its own window.
 *
 * Epoch shows no password field and keeps no token: it starts the agent's official flow and
 * steps back. Resolves when the window is open — whether the sign-in *worked* is what
 * {@link fetchAgents} says afterwards.
 */
export async function signInAgent(agent: string): Promise<void> {
  await invoke("sign_in_agent", { agent });
}

/**
 * Add a second sign-in of one agent program, and start its login. Returns the new account id.
 *
 * The label is the user's own word for it — *work*, *personal*, whatever they type. Epoch does
 * not go looking for a name: Codex cannot be asked which account is signed in at all, and a
 * title guessed from nothing would be a gauge nobody can explain.
 */
export async function addAgentAccount(
  kind: string,
  label: string,
): Promise<string> {
  return await invoke<string>("add_agent_account", { kind, label });
}

/** Rename one added account. The id never changes — a character's brain names the id. */
export async function renameAgentAccount(
  id: string,
  label: string,
): Promise<void> {
  await invoke("rename_agent_account", { id, label });
}

/** Forget an added account. Returns what the Engine says became of its sign-in. */
export async function removeAgentAccount(id: string): Promise<string> {
  return await invoke<string>("remove_agent_account", { id });
}

/**
 * Which agents this machine has, and which it does not.
 *
 * Every one, including the missing: a panel that listed only what was installed could not say
 * *Claude Code is not installed*, which is the sentence that tells somebody what to do next.
 *
 * Asking costs a process per agent and a second for the ones that are there — 537 ms on a
 * machine with two installed, and four surfaces ask independently. So the Engine keeps what it
 * measured, and `fresh` is how a REMEASURE press runs the programs again. A kept reading with no
 * way to repeat it would be a claim rather than a measurement.
 */
export async function fetchAgents(
  fresh = false,
): Promise<readonly AgentStatus[]> {
  try {
    return await invoke<AgentStatus[]>("list_agents", { fresh });
  } catch {
    return [];
  }
}

/** The current signed-in plan allowance for one agent, if its local CLI can report it. */
export async function fetchAgentPlanUsage(
  agent: string,
): Promise<readonly AgentPlanUsage[] | null> {
  try {
    return await invoke<AgentPlanUsage[] | null>("agent_plan_usage", { agent });
  } catch {
    return null;
  }
}

export async function probeMcp(): Promise<
  [readonly string[], readonly string[]]
> {
  try {
    return await invoke<[string[], string[]]>("probe_mcp");
  } catch (error) {
    return [[], [String(error)]];
  }
}

/**
 * Ask every server again, and write down what they say.
 *
 * `probeMcp` answers from what they said last time, so tools are there the moment the panel
 * opens. This is the explicit way to pick up a server whose tools genuinely changed — needed
 * because this build does not listen for `notifications/tools/list_changed`.
 */
export async function refreshMcp(): Promise<
  [readonly string[], readonly string[]]
> {
  try {
    return await invoke<[string[], string[]]>("refresh_mcp");
  } catch (error) {
    return [[], [String(error)]];
  }
}

/**
 * Browse the MCP catalogue.
 *
 * A failure is an empty shelf carrying the reason, never a thrown error: the Workshop is a place
 * you can be standing in when the network drops, and it has to keep being a place.
 */
export async function searchWorkshop(
  query: string,
  cursor?: string,
): Promise<WorkshopShelf> {
  try {
    return await invoke<WorkshopShelf>("workshop_search", {
      query,
      cursor: cursor ?? null,
    });
  } catch (error) {
    return {
      shown: [],
      next: null,
      fetchedMs: null,
      problem: String(error),
      runtimes: { present: [] },
    };
  }
}

/**
 * Install what is on screen.
 *
 * The listing and the offer are sent back unchanged rather than re-fetched by name. What the
 * user approved is the command they were shown; re-fetching would install whatever the catalogue
 * says now, which is a different program.
 */
export async function installFromWorkshop(
  listing: WorkshopListing,
  offer: WorkshopOffer,
  answers: Readonly<Record<string, string>>,
): Promise<{ id: string } | { error: string }> {
  try {
    return {
      id: await invoke<string>("workshop_install", { listing, offer, answers }),
    };
  } catch (error) {
    return { error: String(error) };
  }
}

export async function saveMcp(server: McpServer): Promise<string | null> {
  try {
    await invoke("save_mcp", { server });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * Give a configured server a credential.
 *
 * One-way on purpose: there is no companion that reads one back. The value leaves this process
 * for the encrypted store and the only thing any surface sees afterwards is the variable's name.
 */
export async function saveMcpSecret(
  server: string,
  name: string,
  value: string,
): Promise<string | null> {
  try {
    await invoke("save_mcp_secret", { server, name, value });
    return null;
  } catch (error) {
    return String(error);
  }
}

export async function forgetMcp(id: string): Promise<string | null> {
  try {
    await invoke("forget_mcp", { id });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** The user's choices about their own machine. */
export interface Settings {
  /** Whether several crew members may hold a model in memory at once. */
  readonly concurrentCrew: boolean;
  /**
   * Whether the World may listen through this machine's microphone.
   *
   * **Three states, and `null` is the one that matters**: nobody has been asked. It is not a
   * refusal — a refusal is `false`, and it has to be kept or Epoch asks again on every press.
   */
  readonly microphone: boolean | null;
  /**
   * Which language the person at this machine speaks. `null` is **detect each time**.
   *
   * Not the machine's locale: measured on the owner's, `navigator.language` answers `en-US` and
   * he speaks Spanish. A Windows install language is a fact about the installer.
   */
  readonly hearingLanguage: string | null;
  /**
   * How many rounds of tools one turn may take. `null` is Epoch's own bound; `0` is no limit.
   *
   * The default's reasoning is *look → read → read → answer*, written before a character could
   * have an MCP server's thirty-two tools attached — so the number belongs to the machine now.
   * Each round is a full model call, which is why it is bounded at all.
   */
  readonly toolRounds: number | null;
}

export async function fetchSettings(): Promise<Settings> {
  try {
    return await invoke<Settings>("get_settings");
  } catch {
    // The default protects the machine, so it is also the safe thing to fall back to — and
    // `null` for the microphone, because a failed call means *unasked*, never *refused*.
    return {
      concurrentCrew: false,
      microphone: null,
      hearingLanguage: null,
      toolRounds: null,
    };
  }
}

/**
 * Change one setting, named.
 *
 * **There is deliberately no `saveSettings(Settings)`.** There was, and this surface sent one
 * field of five: the Engine's own defaults filled in the rest and the write put them on disk, so
 * one checkbox deleted a second agent sign-in and a chosen GPU backend. A surface may only send
 * what it names.
 */
export async function setConcurrentCrew(on: boolean): Promise<string | null> {
  try {
    await invoke("set_concurrent_crew", { on });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** Remember which language the user speaks. `""` is *detect each time*. */
export async function setHearingLanguage(language: string): Promise<string | null> {
  try {
    await invoke("set_hearing_language", { language });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** How many rounds of tools one turn may take. `0` is no limit. */
export async function setToolRounds(rounds: number): Promise<string | null> {
  try {
    await invoke("set_tool_rounds", { rounds });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** Remember what the user answered when **Epoch** asked about the microphone. */
export async function setMicrophone(allowed: boolean): Promise<string | null> {
  try {
    await invoke("set_microphone", { allowed });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * The open World's backdrop, as a `data:` URI.
 *
 * Its own call rather than a field on `WorldView`: that projection is re-sent on every presence
 * change, and a full illustration must not ride along with it. Asked for once, on mount.
 */
export async function fetchBackdrop(): Promise<string | null> {
  try {
    return (await invoke<string | null>("get_backdrop")) ?? null;
  } catch {
    // No backdrop is a complete World, so a failure here is simply no backdrop.
    return null;
  }
}

export async function leaveWorld(): Promise<void> {
  try {
    await invoke("leave_world");
  } catch {
    // Leaving is a return to a screen that is already renderable. Nothing to report.
  }
}

/**
 * Make somebody who did not exist.
 *
 * Returns their id, so the caller can open the editor on a character who is now real rather than
 * on a form pretending to be one. A failure comes back as `{ error }` rather than thrown: every
 * reason the Engine refuses is a sentence the user can act on.
 */
export async function hireCharacter(
  name: string,
  archetype: string,
  role: string,
  prompt: string,
): Promise<string | { error: string }> {
  try {
    return await invoke<string>("hire_character", {
      name,
      archetype,
      role,
      prompt,
    });
  } catch (error) {
    return { error: String(error) };
  }
}

/**
 * Words an archetype offers somebody writing a character.
 *
 * From the Engine, because what a Researcher sounds like is a fact about the archetype rather
 * than about this screen — and a copy here would be a second answer free to disagree.
 */
/**
 * What an agent calls its own models.
 *
 * Asked of the Engine rather than held here: Claude Code takes aliases like `opus`, Codex takes
 * `gpt-5.6-sol`, and a list in this layer was correct while there was one agent and wrong the
 * moment there were two — silently, by offering the second agent the first one's models.
 */
export async function agentModels(
  agent: string,
): Promise<readonly [string, string][]> {
  try {
    return await invoke<[string, string][]>("agent_models", { agent });
  } catch {
    return [];
  }
}

export async function suggestedPrompt(archetype: string): Promise<string> {
  try {
    return await invoke<string>("suggested_prompt", { archetype });
  } catch {
    return "";
  }
}

/**
 * Hand a reference picture to the studio, and get back the name it knows it by.
 *
 * The `data:` URI the webview's own file input produced. The frontend never touches a disk
 * (ADR-0024), and what comes back is the server's name rather than anything about this machine.
 */
export async function handReferenceOver(image: string): Promise<string> {
  return await invoke<string>("hand_reference_over", { image });
}
