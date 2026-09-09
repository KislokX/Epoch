/**
 * The IPC boundary for the World.
 *
 * This is the **only** place in the presentation layer that knows Tauri exists. Swapping
 * the engine for a mock, a replay or a snapshot means replacing this file's implementation
 * — never the contract, and never anything that consumes it.
 */

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { PresenceView, WorldSnapshot, WorldView } from "./contracts";

/** The World with nothing in it. Used only so the World can render before data arrives. */
const EMPTY_WORLD: WorldView = {
  packName: null,
  map: null,
  places: [],
  characters: [],
};

/** The engine publishes here when presence actually changes — never per frame. */
const WORLD_CHANGED = "world:changed";

/**
 * Somebody moving, and nothing else.
 *
 * `world:changed` carries every Place, every mark and every face — the right payload when the
 * World's *shape* changes, and far too much to repeat ten times while somebody walks across it.
 * A walk arrives here instead: who, where, and how far along.
 */
const WORLD_MOVED = "world:moved";

/** A character's last measured context for the active Quest, never an estimate. */
export interface ContextReading {
  readonly characterId: string;
  readonly used: number;
  readonly budget: number;
}

/**
 * Read all active-Quest context readings together.
 *
 * The Engine keys them by Quest, and this one request keeps a Crew card from mixing a reading
 * from the conversation just left with one from the conversation just opened.
 */
export async function fetchRememberedContexts(): Promise<
  readonly ContextReading[]
> {
  try {
    return await invoke<ContextReading[]>("remembered_contexts");
  } catch {
    return [];
  }
}

/**
 * Ask the engine for the current World.
 *
 * Never throws. The World must always be renderable (Build From Life, rule 1), so an
 * unreachable engine returns an honest "unavailable" snapshot instead of an exception.
 */
export async function fetchWorld(): Promise<WorldSnapshot> {
  try {
    const view = await invoke<WorldView>("get_world");
    return { status: "live", view };
  } catch (cause) {
    return {
      status: "unavailable",
      view: EMPTY_WORLD,
      note: cause instanceof Error ? cause.message : String(cause),
    };
  }
}

/** The outcome of trying to follow the World. */
export type Watch =
  | { readonly following: true; readonly stop: () => void }
  | { readonly following: false; readonly reason: string };

/**
 * Watch the World for change.
 *
 * The engine is authoritative and pushes whole authoritative states; the UI renders and
 * interpolates between them, and never invents anything in between (ADR-0018).
 *
 * **Failure is reported, never swallowed.** A UI that quietly stops following the world
 * while looking live is a UI that lies — the caller must be able to say so.
 */
export async function watchWorld(
  onChange: (view: WorldView) => void,
  onMove: (moved: readonly PresenceView[]) => void,
): Promise<Watch> {
  try {
    const stopChanges = await listen<WorldView>(WORLD_CHANGED, (event) =>
      onChange(event.payload),
    );
    const stopMoves = await listen<PresenceView[]>(WORLD_MOVED, (event) =>
      onMove(event.payload),
    );
    const stop = () => {
      stopChanges();
      stopMoves();
    };
    return { following: true, stop };
  } catch (cause) {
    return {
      following: false,
      reason: cause instanceof Error ? cause.message : String(cause),
    };
  }
}

/** One mode a World can be in, exactly as the Engine describes it. */
export interface ModeView {
  readonly id: string;
  readonly label: string;
  readonly describe: string;
}

/**
 * What this World currently allows, and what it could.
 *
 * The list comes from the Engine rather than living here, for the same reason the archetypes
 * do: a hardcoded copy drifts the day a mode is added, and drift in the permission system
 * would look like a bug in the one place nobody should be guessing.
 */
export interface AutonomyView {
  readonly current: string;
  readonly modes: readonly ModeView[];
}

export async function fetchAutonomy(): Promise<AutonomyView | null> {
  try {
    return (await invoke<AutonomyView | null>("get_autonomy")) ?? null;
  } catch {
    return null;
  }
}

/** Change how much the crew may do on its own. Returns the reason on failure. */
export async function setAutonomy(mode: string): Promise<string | null> {
  try {
    await invoke("set_autonomy", { mode });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * Where this World works, and what the crew can therefore do here.
 *
 * The Launcher already showed the Project Root, and that was not enough: the crew was silently
 * toolless *inside* a World, and the only place saying so was a screen the user had left. A
 * character with no tools answers "I cannot do that" perfectly truthfully, and it reads exactly
 * like a broken product.
 */
export interface Workspace {
  readonly projectRoot: string | null;
  readonly capabilities: readonly string[];
  /** The folder of notes this World reads from, when it has one. */
  readonly library: string | null;
}

export async function fetchWorkspace(): Promise<Workspace | null> {
  try {
    return (await invoke<Workspace | null>("get_workspace")) ?? null;
  } catch {
    return null;
  }
}

/**
 * What one character would be given if they were asked to work right now.
 *
 * The Engine reads it from the function that composes a turn, so the `/` menu shows the real
 * table rather than a list the frontend keeps. That is what makes `/playwright` appear the day
 * a server is connected without anybody adding it here.
 */
export interface Offered {
  readonly tools: readonly { readonly id: string; readonly summary: string }[];
  /** Asked for and not available. Shown, because a gap explains nothing. */
  readonly withheld: readonly string[];
  readonly crew: readonly {
    readonly id: string;
    readonly name: string;
    readonly role: string;
  }[];
  /**
   * One sentence naming what decides this list.
   *
   * It is not the same question for the two brains: a model is offered exactly what it asked
   * for, filtered by the mode, while an agent is offered Epoch's whole set on purpose so that
   * the user can be asked. The menu showed one list for both, which described only one.
   */
  readonly gate: string;
}

export async function fetchOffered(
  characterId: string,
): Promise<Offered | null> {
  try {
    return (
      (await invoke<Offered | null>("get_offered", { characterId })) ?? null
    );
  } catch {
    return null;
  }
}

/** A standing decision, read back so it can be taken away. */
export interface Standing {
  readonly capability: string;
  readonly scope: "character" | "world" | "everywhere";
  readonly who: string | null;
  readonly allowed: boolean;
  /** One sentence, assembled by the Engine so a surface never has to guess the meaning. */
  readonly describe: string;
}

export async function fetchStanding(): Promise<readonly Standing[]> {
  try {
    return await invoke<Standing[]>("list_standing");
  } catch {
    return [];
  }
}

/** Take a standing decision back. Returns the reason on failure. */
export async function forgetStanding(s: Standing): Promise<string | null> {
  try {
    await invoke("forget_standing", {
      capability: s.capability,
      scope: s.scope,
      who: s.who,
    });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * Grant a character a capability they reached for, or refuse.
 *
 * Granting is not approving: the call is judged afterwards like any other, so this may well
 * stop again to ask. Two decisions, two questions.
 */
export async function answerCapability(
  characterId: string,
  grant: boolean,
): Promise<string | null> {
  try {
    await invoke("answer_capability", { characterId, grant });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** The most recent change that could be put back, or null when there is nothing. */
export interface Undoable {
  /**
   * Which entry this is — the World, and its position in that World's journal.
   *
   * Identity rather than wording, so "I have read this" means *this change*. Creating a file,
   * dismissing the notice and creating the same file again produced the same sentence, and the
   * second change therefore arrived already dismissed: a real undo nobody could see or reach.
   * The World is part of it for the same reason, one level up.
   */
  readonly id: string;
  readonly summary: string;
}

export async function fetchUndoable(): Promise<Undoable | null> {
  try {
    return (await invoke<Undoable | null>("get_undoable")) ?? null;
  } catch {
    return null;
  }
}

/**
 * Put the last change back.
 *
 * Refuses when the file has moved on since — an undo that quietly discarded somebody's later
 * work would be worse than the mistake it fixes. Returns what happened, or the reason it did
 * not.
 */
export async function undoLast(): Promise<{
  done: string | null;
  failed: string | null;
}> {
  try {
    return { done: await invoke<string>("undo_last"), failed: null };
  } catch (error) {
    return { done: null, failed: String(error) };
  }
}

/**
 * Open a source in the user's browser.
 *
 * The Engine checks the scheme before anything starts: these addresses come from search
 * results, which are pages chosen by a ranking out of text written by strangers. A surface
 * check here would be a courtesy; that one is the control.
 */
export async function openLink(url: string): Promise<string | null> {
  try {
    await invoke("open_link", { url });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** One rung of a character's reasoning ladder. */
export interface Rung {
  readonly id: string;
  readonly label: string;
}

/**
 * A character's reasoning ladder and where they sit on it.
 *
 * The **brain** declares the rungs: a local model's ladder and Claude Code's are different
 * lengths and do not hold the same levels — the CLI has `xhigh` and has no "off". A surface that
 * hardcoded either would be wrong for the other, and wrong silently.
 */
export interface Dial {
  /** Weakest first. Empty means this brain has no measured ladder — show no shortcut. */
  readonly rungs: readonly Rung[];
  /** `null` is a real position: the brain's own default, which is not the same as the least. */
  readonly chosen: string | null;
  /** Whose ladder it is, in words, so the popover can say it without knowing about brains. */
  readonly brain: string;
  /** Which agent, when it is one. The id, for asking the Engine what a mode means for them. */
  readonly agentId: string | null;
  /**
   * Whether that brain is an agent.
   *
   * From the Engine rather than guessed from the name: an agent enforces the autonomy mode
   * itself, and `manual` means something different there — it refuses rather than asks.
   */
  readonly agent: boolean;
}

/**
 * What a mode actually does for this agent.
 *
 * From the Engine, because the two agents differ: Claude Code asks before each of its own tools;
 * Manual Codex pauses a native tool request for Epoch's decision. The surface must not swap those
 * two paths or claim that one of them cannot ask.
 *
 * `null` means Epoch has nothing measured to say, and the mode's own description stands.
 */
export async function gateNote(
  agent: string,
  mode: string,
): Promise<string | null> {
  try {
    return await invoke<string | null>("gate_note", { agent, mode });
  } catch {
    return null;
  }
}

export async function fetchReasoning(character: string): Promise<Dial | null> {
  try {
    return await invoke<Dial>("reasoning_dial", { character });
  } catch {
    return null;
  }
}

/**
 * Move a character along their ladder. `null` returns them to the brain's own default.
 *
 * Persisted by the Engine, because how hard somebody thinks is who they are (ADR-0026) — a
 * shortcut that reverted when the World closed would be a control that only appears to work.
 */
export async function chooseReasoning(
  character: string,
  rung: string | null,
): Promise<string | null> {
  try {
    await invoke("choose_reasoning", { character, rung });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * One Quest, as any surface receives it.
 *
 * **One type, because there is one thing.** There were two — one for the Chronicle and one for
 * the list — overlapping in four fields and disagreeing about a fifth: `said` was a list of lines
 * in one and a count in the other. Two meanings for one name is how a component ends up reading
 * `quest.said.length` in one place and `quest.said` in another, and only one of them is right.
 *
 * Open and ended live in the same type too. A closed conversation is not a different kind of
 * thing — it is the same work, finished.
 */
export interface QuestSummary {
  readonly id: string;
  readonly title: string;
  readonly state: string;
  /** Whether it can still be continued. */
  readonly open: boolean;
  /** Whether it is the one on screen. */
  readonly current: boolean;
  /** Who has actually contributed, in the order they first did. */
  readonly participants: readonly string[];
  /** Whether it produced anything real. Talking is not evidence (ADR-0025). */
  readonly producedEvidence: boolean;
  /** How many things were said — always present, so a list never needs the transcript. */
  readonly saidCount: number;
  /** The conversation itself. Absent unless it was asked for. */
  readonly chronicle?: readonly Said[];
}

/** One thing that was said. `who` is `null` for the orchestrator, a character id otherwise. */
export interface Said {
  readonly who: string | null;
  readonly content: string;
  /** Text references deliberately included with this user message; never filesystem paths. */
  readonly attachments: readonly Attachment[];
  /** Pictures shared with this message, as references. The bytes are fetched separately. */
  readonly images: readonly SharedImage[];
  /** Present only for a durable context-compaction marker. */
  readonly compaction?: Compaction;
  /**
   * Tokens per second, as the backend that produced this answer measured its own decoding.
   *
   * `None` for everything else, and for a backend that reports no timing — an agent, a hosted
   * model, a machine across the bridge. Never a zero: a rate invented for a turn nobody timed
   * would be the gauge with nothing behind it.
   */
  readonly pace?: number | null;
  /**
   * What kind of entry this is.
   *
   * The Chronicle is not a conversation (ADR-0025): it also holds what was approved and what was
   * produced. Projecting only the spoken parts meant a permission you gave and a file that now
   * exists both vanished from the only view of the Quest.
   */
  readonly kind: "said" | "answered" | "approved" | "produced" | "compacted";
}

/** The Chronicle prefix that a fresh agent session now receives as one continuity brief. */
export interface Compaction {
  readonly covered: number;
}

export interface Attachment {
  readonly name: string;
  readonly bytes: number;
}

/** Text a person selected to accompany one message. Never a filesystem path or a capability. */
export interface TextAttachmentInput {
  readonly name: string;
  readonly content: string;
  /** Original UTF-8 file size for the surface; the shell recomputes its own limit from content. */
  readonly bytes: number;
}

/**
 * An image a person chose to share with one message.
 *
 * `data` is base64, and it crosses once: the Engine writes the bytes and everything afterwards
 * carries a name (ADR-0024). The frontend never touches the filesystem, and `name` is only ever
 * shown back to the user — the Engine names the stored file from what it contains.
 */
export interface ImageAttachmentInput {
  readonly name: string;
  readonly data: string;
  /** Decoded size, for the surface's own limits. The Engine recomputes its own. */
  readonly bytes: number;
}

/** One picture shared in a conversation. A reference, never the picture itself. */
export interface SharedImage {
  /** What the user called it. Shown; never used to find anything. */
  readonly name: string;
  /** The Engine's name for it in the vault, chosen from its bytes. Fetch with `sharedImage`. */
  readonly file: string;
}

/**
 * The bytes of one shared picture, as a `data:` URI, fetched once and kept.
 *
 * Images used to ride the Chronicle projection, which is re-read on every turn: a conversation
 * holding a few screenshots re-encoded and re-sent megabytes each time, and the window stopped
 * responding. The cache is keyed by the Engine's file name, which is a hash of the bytes — so
 * the same picture shared twice, or in two Quests, is fetched once.
 *
 * `null` means the file is no longer in the vault. That is remembered too: a missing file will
 * not reappear, and asking again on every render would be a request per frame.
 */
const pictures = new Map<string, Promise<string | null>>();

/**
 * Everything this World has made.
 *
 * Enumerated from the Quests rather than read out of a folder — which is what lets each row say
 * which conversation made it and when.
 */
export async function madeHere(): Promise<readonly MadeRow[]> {
  try {
    return await invoke<MadeRow[]>("made");
  } catch {
    return [];
  }
}

export interface MadeRow {
  /** What kind of thing, in the words of whatever made it: `file`, `image`, `commit`. */
  readonly kind: string;
  /** The thing itself — a path, a filename, a hash. */
  readonly reference: string;
  readonly summary: string;
  readonly quest: string;
  readonly questTitle: string;
  /** Milliseconds, or `null` for work whose record was compacted away. */
  readonly at: number | null;
  /** It can be drawn here rather than handed to the machine to open. */
  readonly shown: boolean;
}

/** Open one, with whatever this machine opens that kind of file with. */
export async function openMade(reference: string): Promise<string | null> {
  try {
    await invoke("open_made", { reference });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * Open a picture from the conversation in whatever this machine opens pictures with.
 *
 * Separate from `openMade`: the Chronicle draws what the crew produced and what the user pasted
 * with the same component, and asking the Engine "did this World make it" refuses half of them.
 */
export async function openPicture(file: string): Promise<string | null> {
  try {
    await invoke("open_picture", { file });
    return null;
  } catch (error) {
    return String(error);
  }
}

/**
 * Where a picture from the conversation can be fetched, over Epoch's own URI scheme.
 *
 * **The bytes stop coming through JavaScript.** `sharedImage` reads the file, base64s it and
 * carries it across the IPC as a string; measured on a 182 MB render that is 2135 ms for the
 * Engine and the IPC and 348 ms to undo the base64, before the browser has decoded anything. An
 * `<img src>` on this scheme is fetched, cached and decoded by the browser on its own threads,
 * and the page never holds the bytes at all.
 *
 * **ADR-0024 is unchanged in what it protects.** The rule is that the frontend never touches the
 * filesystem and the Engine names the file — and that still holds exactly: this hands over a
 * *name*, the handler in the Engine decides what it resolves to, and no path and no `fs`
 * capability reach the webview. Only the transport changed.
 *
 * `convertFileSrc` because the URL is not the same on every platform: Windows serves a custom
 * scheme as `http://epoch.localhost/…` and the others as `epoch://localhost/…`. Writing either
 * one by hand would work on the machine it was written on.
 */
export function pictureSrc(file: string): string {
  return convertFileSrc(file, "epoch");
}

/**
 * One catalogue preview, by the token a search handed out.
 *
 * **`convertFileSrc` and not a hand-written `epoch://`.** Measured on Windows: an `<img>` whose
 * src was written by hand loaded nothing at all — twenty-three of them, every one 0×0 — because
 * the scheme is served as `http://epoch.localhost/…` there and only this knows that.
 *
 * **One segment, no slash.** `convertFileSrc` escapes separators into the name, which is exactly
 * what the picture handler leans on to keep a name from reaching out of its folder. So a preview
 * is `preview-<token>` rather than `preview/<token>`, and the same property holds for both.
 */
export function previewSrc(token: string): string {
  return convertFileSrc(`preview-${token}`, "epoch");
}

/**
 * Where a catalogue's recording plays from.
 *
 * The audio sibling of {@link previewSrc}, and it goes through the same door for the same
 * reason: the window asks Epoch for a token and talks to no third party. `media-src` allows
 * `epoch:` and deliberately not `data:`, which is what made an earlier attempt silent.
 */
export function spokenSrc(name: string): string {
  return convertFileSrc(`spoken-${name}`, "epoch");
}

export function sampleSrc(token: string): string {
  return convertFileSrc(`sample-${token}`, "epoch");
}

export function sharedImage(file: string): Promise<string | null> {
  const known = pictures.get(file);
  if (known) return known;
  const asked = invoke<string | null>("shared_image", { file }).catch(
    () => null,
  );
  pictures.set(file, asked);
  return asked;
}

export async function fetchConversations(): Promise<readonly QuestSummary[]> {
  try {
    return await invoke<QuestSummary[]>("list_conversations");
  } catch {
    return [];
  }
}

/** Look at a different one. Its Chronicle — and, for an agent, its session — come with it. */
export async function selectQuest(quest: string): Promise<string | null> {
  try {
    await invoke("select_quest", { quest });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** What ending a Quest produced: a note in the library, a refusal, or neither. */
export interface Closed {
  /** Where the Engine wrote what this Quest left behind. `null` when it left nothing. */
  readonly note: string | null;
  readonly failure: string | null;
}

/**
 * End one. It stays in History as what it was; closing is not deleting (ADR-0025).
 *
 * A Quest that produced evidence is remembered in the World's library on the way out — which
 * is the whole of the Knowledge Engine as far as a surface is concerned (ADR-0010).
 */
export async function closeQuest(quest: string): Promise<Closed> {
  try {
    const note = await invoke<string | null>("close_quest", { quest });
    return { note: note ?? null, failure: null };
  } catch (error) {
    return { note: null, failure: String(error) };
  }
}

/**
 * Say where one of somebody's routine activities happens, in this World.
 *
 * `null` puts it back to happening at home. This one call is the whole of authoring idle
 * movement: a character walks on their routine because somebody decided this and for no other
 * reason (ADR-0018's causality rule).
 */
export async function setRoutinePlace(
  characterId: string,
  activity: string,
  placeId: string | null,
): Promise<string | null> {
  try {
    await invoke("set_routine_place", { characterId, activity, placeId });
    return null;
  } catch (cause) {
    return cause instanceof Error ? cause.message : String(cause);
  }
}

/**
 * What somebody would be given, if the work were handed to them now.
 *
 * Composed by the Engine from the Quest, so it is the same text whichever brain receives it —
 * and asking is not doing: nothing is sent and nothing changes until the user says so.
 */
export async function handoverPreview(
  characterId: string,
): Promise<string | null> {
  try {
    return (
      (await invoke<string | null>("handover_preview", { characterId })) ?? null
    );
  } catch {
    return null;
  }
}

/**
 * What the lamp above one conversation reads.
 *
 * Two facts on purpose. `holding` is what the user asked for; `resident` is what is actually on
 * the graphics card, measured from the server that holds it. A lamp wired to the switch would
 * be a gauge reading itself.
 */
export interface Warmth {
  /** Which model this is about, in the backend's own words. */
  readonly model: string;
  /** Whether this brain is being held after it answers. Already includes the machine default. */
  readonly holding: boolean;
  /** Whether the user chose that, rather than inheriting the machine-wide setting. */
  readonly chosen: boolean;
  /**
   * In memory right now.
   *
   * `null` is **unasked**, never "no": a hosted brain holds nothing of the user's, an agent has
   * no model at all, and a local server that will not answer has said nothing. Draw no lamp.
   */
  readonly resident: boolean | null;
}

/** What is on the card for one character. Reaches a server, so it may take a moment. */
export async function fetchWarmth(characterId: string): Promise<Warmth | null> {
  try {
    return await invoke<Warmth>("warmth", { character: characterId });
  } catch {
    // No brain, no backend, nothing to report. The lamp simply does not appear.
    return null;
  }
}

/**
 * Hold this character's brain on the card, or let it go now.
 *
 * Off releases immediately — the button says unload. On loads nothing: the next turn does that,
 * and it stays. So the lamp only ever goes green because the model is genuinely there.
 */
export async function holdWarm(
  characterId: string,
  keep: boolean,
): Promise<Warmth | null> {
  try {
    return await invoke<Warmth>("hold_warm", {
      character: characterId,
      keep,
    });
  } catch {
    return null;
  }
}
