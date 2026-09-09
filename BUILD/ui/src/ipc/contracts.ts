/**
 * The contracts the engine exposes to the presentation layer.
 *
 * These mirror the engine's *projections* (`epoch-engine::world`), never its internal
 * types. The UI consumes contracts and nothing else — it must never learn whether the
 * data behind them came from the engine, a mock, a replay or a snapshot.
 *
 * Rule: replace implementations, not interfaces.
 */


/**
 * A converted RVC voice, and the pitch a character is spoken at through it.
 *
 * **One object rather than two fields**, because the two are meaningless apart: a pitch with no
 * model changes nothing, and a model at the wrong pitch is the same voice half an octave out —
 * which everybody hears as a bad model rather than as a setting. The pitch belongs to the pair.
 */
export interface Timbre {
  /** The voice's **name**, never a path. */
  readonly voice: string;
  /** Semitones. `0` is the model's own register. */
  readonly semitones: number;
  /**
   * Which voice inside a model that holds several.
   *
   * **Optional, because it is genuinely absent from the wire when it is `0`** — the same struct
   * is written into the character's authored file, and a `speaker = 0` line in everybody's TOML
   * is noise. Absent means the first one; the Engine defaults it back on the way in.
   */
  readonly speaker?: number;
}

/** Where a Place sits, in **world units** — never pixels. */
export interface PlacementView {
  readonly x: number;
  readonly y: number;
  /** Footprint in world units. This *is* the Place's scale; Places are not uniform. */
  readonly footprint: number;
  /**
   * Explicit draw order. Places already arrive in order — this is carried so a renderer
   * with its own scene graph can reproduce it rather than re-deriving it.
   */
  readonly zOrder: number;
}

/** Colour roles a shape layer can take — roles, not colours. */
export type Tone = "wall" | "roof" | "detail" | "accent" | "shadow";

/** One drawn layer of a `shape` Renderable. Geometry is in footprint units. */
export interface ShapeLayerView {
  readonly form: "rect" | "polygon" | "dome" | "ellipse";
  readonly tone: Tone;
  readonly x: number;
  readonly y: number;
  /** Width for `rect`, x-radius for `ellipse`. */
  readonly w: number;
  /** Height for `rect`, y-radius for `ellipse`. */
  readonly h: number;
  /** Corner radius for `rect`, radius for `dome`. */
  readonly r: number;
  readonly points: readonly (readonly [number, number])[];
}

/**
 * One drawn part of a Place: a purpose, and how to draw it (ADR-0021).
 *
 * The renderer knows how to draw a mark; it never knows which Place it belongs to or what
 * that Place means. Appearance belongs to the active World.
 */
export interface MarkView {
  /** Canonical mark role id: `"visual"`, `"shadow"`, `"roof"`, `"decoration"`, … */
  readonly role: string;
  /** Canonical renderer kind id: `"shape"`, `"sprite"`, `"tilemap"`, … */
  readonly renderer: string;
  /** False when this build cannot draw the role or the renderer — show a placeholder. */
  readonly supported: boolean;
  /**
   * A `data:` URI for the World's Asset, when it declared one that could be read.
   *
   * Drawn with `<image>` and **never inlined as markup**: a World is third-party content,
   * and inlining its SVG would let it execute script inside Epoch (ADR-0020).
   */
  readonly asset: string | null;
  /** Size relative to the Place footprint. */
  readonly scale: number;
  /** Asset origin in its own 0..1 space. `[0.5, 1]` is bottom-centre. */
  readonly anchor: readonly [number, number];
  readonly shape: readonly ShapeLayerView[];
  /**
   * How to cut this mark's asset, when the asset is a sheet rather than a picture.
   *
   * Absent for every still mark, which is nearly all of them.
   */
  readonly frames?: FramesView;
}

/** Which way somebody is facing — the second half of `walk.east`. */
export type Direction = "north" | "east" | "south" | "west";

/**
 * How a sheet is cut.
 *
 * **The cut, never the playback.** The Engine says how many cells there are, where they are
 * and how long each is shown — facts about the artwork. Which cell is on screen at this
 * millisecond is per-frame and belongs here, in the renderer: the Engine owns reality, the UI
 * owns animation (ADR-0018).
 */
export interface FramesView {
  /** Cells across. */
  readonly columns: number;
  /** Cells down. */
  readonly rows: number;
  /** How many cells are real, in reading order from the top left. Never implied. */
  readonly count: number;
  /** How long one cell is shown. */
  readonly milliseconds: number;
  /**
   * Which direction each **row** walks, in row order.
   *
   * Empty means the sheet is not directional: one loop, whichever way somebody is facing.
   */
  readonly directions: readonly Direction[];
}

/**
 * A named point or region the Place declares, in footprint units.
 *
 * Where inhabitants stand and where the name sits used to be constants in this renderer.
 * They are World data now, which is why one Place's proportions can no longer decide how
 * every Place behaves.
 */
export interface AnchorView {
  /** Canonical anchor role id: `"spawn"`, `"label"`, `"interaction_bounds"`, … */
  readonly role: string;
  /** False when nothing in this build consumes this role yet. */
  readonly supported: boolean;
  readonly x: number;
  readonly y: number;
  /** Half-width. `0` means this anchor is a point rather than a region. */
  readonly w: number;
  /** Half-height. `0` means a point. */
  readonly h: number;
}

/**
 * One Place in the World: a stable identity, and everything projected onto it.
 *
 * A Place is the visible projection of a capability — not a building, not decoration and
 * not UI. Its appearance, assets, occupants and animation may all change; `id` never does,
 * and everything inside Epoch references it by that.
 */
export interface PlaceView {
  /** Stable identity. Safe to key on, safe to send back in a command. */
  readonly id: string;
  /**
   * Canonical concept id (e.g. `"research_lab"`) — what capability this projects.
   *
   * `null` for a Place the user invented (ADR-0028). It exists, it can be visited, and no
   * subsystem lights it up. Never render this: it is what a Place *does*, not what it is
   * called — `title` is the name.
   */
  readonly concept: string | null;
  /** What this Place is called in the active World. */
  readonly title: string;
  /** What it is for, in the World's own words. */
  readonly subtitle: string | null;
  /**
   * True when no contributor named it and the title is a stand-in.
   * The World must show it as visibly unresolved rather than pretending otherwise.
   */
  readonly isPlaceholder: boolean;
  /** Where it is. `null` means known but not yet positioned — still a valid World. */
  readonly placement: PlacementView | null;
  /** Everything drawn, in draw order. Empty means no World described its appearance. */
  readonly marks: readonly MarkView[];
  /** Named points and regions, for things other than drawing. */
  readonly anchors: readonly AnchorView[];
}

/** A kind of terrain the engine recognises. */
export type TerrainKind =
  "grass" | "forest" | "water" | "mountain" | "stone" | "sand";

/** One area of terrain, in draw order. */
export interface TerrainView {
  readonly kind: TerrainKind;
  /** Polygon outline as `[x, y]` pairs, in world units. */
  readonly points: readonly (readonly [number, number])[];
}

/** How travelled a road is, as the world's own history. */
export type Prominence = "major" | "minor";

/** A road between two Places, already resolved to world-unit points. */
export interface RouteView {
  readonly points: readonly (readonly [number, number])[];
  readonly prominence: Prominence;
}

/**
 * The world's geography, authored by the active World.
 *
 * The engine carries this without interpreting it — it does no geometry. Deliberately much
 * larger than the viewport: the camera moves *through a world*, not over a canvas.
 */
export interface MapView {
  readonly width: number;
  readonly height: number;
  readonly terrain: readonly TerrainView[];
  readonly routes: readonly RouteView[];
}

/**
 * Whether what a character is doing is their own routine, or real work.
 *
 * The World must render these differently: routine behaviour may never look like work that
 * is not happening.
 */
export type ActivityClass = "idle" | "work" | "waiting";

/**
 * **Which** action to draw, in a closed vocabulary a World Pack resolves (`walk.east`).
 *
 * Beside `class`, never instead of it. `class` is the honesty rule — routine may never look
 * like work that is not happening — and this is the animation. A renderer that read one for
 * the other would be deciding a question the Engine already answered.
 *
 * `think` is the turn started with no tool run yet; it becomes `work` the moment the first
 * tool, command or file change begins.
 *
 * `settle` and `talk` are **beats**, not states: they land on arrival and then become whatever
 * the journey was for. `talk` means the person the work came from is standing here — it is the
 * arrival, never a conversation, and no surface may pretend words are passing.
 *
 * There is no `sleep`: nothing causes it.
 */
export type Action =
  | "idle"
  | "walk"
  | "settle"
  | "talk"
  | "think"
  | "work";

/** One inhabitant of the World, carrying their own identity into it. */
export interface CharacterView {
  /** **Who** this is. Stable identity; safe to key on and to send back in a command. */
  readonly id: string;
  /** What they are called. Theirs, not the World's (ADR-0023). */
  readonly name: string;
  /**
   * Canonical archetype id (e.g. "researcher") — what kind of worker they are.
   * Several characters may share one, so this classifies and never identifies.
   */
  readonly archetype: string;
  /**
   * **Which Place is theirs** — where they return to, not where they are.
   *
   * Separate from `place` on purpose: assigning a residence changes where somebody belongs,
   * never where they are standing, and somebody who is out is genuinely elsewhere.
   */
  readonly home: string;
  /** **Which Place** they are currently in — an identity, not a concept (ADR-0028). */
  readonly place: string;
  /** What they are visibly doing, in their world's terms. The human sentence. */
  readonly activity: string;
  /** The same thing in the closed vocabulary — what to draw rather than what to read. */
  readonly action: Action;
  readonly class: ActivityClass;
  /**
   * How they look. `null` means nobody gave them a face, and the World shows a visible
   * stand-in rather than inventing one.
   *
   * The same `MarkView` a Place uses — a character's appearance is not a second pipeline.
   */
  readonly mark: MarkView | null;
  /**
   * The face that identifies them, used where a portrait is what matters rather than a body:
   * the conversation, a roster row. Falls back to the sprite when they authored no icon.
   */
  readonly icon: MarkView | null;
  /**
   * What they look like doing each thing they have been drawn doing, keyed by action id.
   *
   * Beside `mark`, never instead of it: `mark` is the still picture and it is what to draw for
   * every action **not** in here — which is every action for a character with one drawing, and
   * that character is complete.
   *
   * Always an object, empty when nobody has drawn them moving. A field that were sometimes
   * missing would turn one question into two.
   */
  readonly actions: Readonly<Partial<Record<Action, MarkView>>>;
  /**
   * Where they are going, when they are going somewhere (ADR-0018).
   *
   * Absent for anybody standing still, which is nearly everybody nearly always. `place` above
   * stays the Place they **left** for the whole walk: a character has departed and not arrived,
   * and a Library that counted somebody still walking to it would be lit up for a person who is
   * not there.
   */
  readonly journey?: JourneyView;
  /**
   * What they do when nothing is asked, and where each step happens **in this World**.
   *
   * Two halves with two owners (ADR-0028): the activity and its length are the character's and
   * travel with them; the building is this World's and is authored here. `place: null` means it
   * happens at home, which is every step until somebody decides otherwise.
   */
  /**
   * Which voice they speak with — the name they authored. `null` is silence, the ordinary case.
   *
   * Here rather than fetched from the Launcher because the Chronicle is where an answer arrives,
   * and asking a second surface for a character's file on every finished sentence would be a
   * second reader of one fact.
   */
  readonly speaksWith: string | null;
  /**
   * Which converted RVC voice colours it, and at what pitch. `null` is the plain voice, which
   * is the ordinary case even for somebody who speaks.
   */
  readonly soundsLike: Timbre | null;
  readonly routine: readonly RoutineStepView[];
}

/**
 * Somebody's presence on its own, without the World around it.
 *
 * What arrives on the movement channel. Deliberately a subset of `CharacterView`: a walk needs
 * no artwork, no name and no archetype, because whoever is drawing already has all three from
 * the last full projection.
 */
export interface PresenceView {
  readonly id: string;
  readonly place: string;
  readonly activity: string;
  readonly action: Action;
  readonly class: ActivityClass;
  readonly journey?: JourneyView;
}

/** One step of a routine, and where it happens in this World. */
export interface RoutineStepView {
  readonly activity: string;
  readonly seconds: number;
  readonly place: string | null;
}

/**
 * A walk, as the Engine publishes it.
 *
 * The four numbers that let the World animate without inventing anything. The Engine publishes
 * coarsely — a departure, roughly ten corrections, an arrival — and between those the World
 * interpolates using `speed` and `etaSeconds`, which is *deriving* the answer the Engine would
 * give rather than guessing at one. The next authoritative state always wins.
 */
export interface JourneyView {
  readonly from: string;
  readonly to: string;
  /** How far along, 0..1. */
  readonly progress: number;
  /** World units per second — the same units the map is drawn in. */
  readonly speed: number;
  readonly etaSeconds: number;
  /**
   * Which way they are walking.
   *
   * Measured by the Engine from the World's geography, so picking the row of a directional
   * sheet is reading an answer rather than working one out from coordinates.
   */
  readonly facing: Direction;
}

/**
 * One installed World, as the Launcher shows it.
 *
 * The Launcher is a second Experience Surface over the same Engine — it prepares, the World
 * immerses. Same contracts discipline: this mirrors a projection, never engine internals.
 */
/**
 * A World seen small: its own geography.
 *
 * Derived from the World's real terrain and Place positions, never a supplied image — so it
 * cannot promise something the World does not contain, and costs an author nothing.
 */
export interface WorldPreview {
  readonly width: number;
  readonly height: number;
  readonly terrain: readonly TerrainView[];
  /** The road network. What makes a map recognisable at this size. */
  readonly routes: readonly RouteView[];
  /** Each Place as `[x, y, footprint]`, in world units. */
  readonly places: readonly (readonly [number, number, number])[];
}

export interface WorldSummary {
  /** Stable identity. What entering is addressed by — never the name. */
  readonly id: string;
  readonly name: string;
  readonly version: string;
  /** What kind of World this is, in its author's words. `null` when they did not say. */
  readonly kind: string | null;
  /** What this World is. `null` when unauthored — shown as such, never invented. */
  readonly description: string | null;
  /** Which berth it occupies in the docking bay, from 1. Derived from a sorted list. */
  readonly berth: number;
  /** License type and holder. Mandatory for every World, so never empty. */
  readonly license: string;
  /** Fraction of the Engine's concepts this World supplies, 0..1. */
  readonly coverage: number;
  readonly places: number;
  readonly characters: number;
  /** What is wrong with this World. Shown plainly; a World with problems still opens. */
  readonly problems: readonly string[];
  /** The World drawn small. `null` when it has no geography to draw. */
  readonly preview: WorldPreview | null;
  /**
   * Key art the author supplied, as a `data:` URI.
   *
   * Takes precedence over the derived chart when present — the author decides how much
   * immersion their World earns. The chart stays underneath as the fallback, so artwork adds
   * to something that already works rather than replacing it.
   */
  readonly art: string | null;
  /**
   * The folder this World actually works in (ADR-0025), as the user chose it.
   *
   * `null` is complete, not unfinished: the World still opens and the crew still lives there,
   * there is simply no code to read.
   */
  readonly projectRoot: string | null;
  /** True when a root is set and the folder has gone. Said plainly, never silently ignored. */
  readonly projectMissing: boolean;
  /**
   * The folder of notes this World reads from — an Obsidian vault, or any folder of markdown.
   *
   * A different question from the Project Root rather than a second one: a project is worked
   * in, a library is read. `null` is complete; a World that knows nothing yet still opens.
   */
  readonly library: string | null;
  /** True when a library is set and the folder has gone. */
  readonly libraryMissing: boolean;
}

/**
 * What one Provider can currently do.
 *
 * Always answerable. A Provider that is not reachable is the ordinary state, not an error — so
 * this carries *why* along with where Epoch looked, and nothing may claim `online` without
 * having asked.
 */
/**
 * What exporting one World would carry, before it is carried.
 *
 * The same *say it, then do it* shape as `RemovalView`, for the opposite risk: a removal may
 * take more than you meant, and an export may **send** more than you meant, to somebody you
 * cannot un-send it to.
 */
export interface ExportView {
  /** The World, as a person calls it. */
  readonly what: string;
  /**
   * What the archive will be called.
   *
   * A name rather than a path: **where** it lands is the user's answer, given to a native dialog
   * after they accept the plan.
   */
  readonly into: string;
  /** What travels, by its place inside the exported folder. */
  readonly carries: readonly string[];
  /** How much it weighs, counted from the files rather than estimated. */
  readonly bytes: number;
  /** What stays behind. An export quiet about what it kept reads as one that took everything. */
  readonly leaves: readonly string[];
  /**
   * Of `carries`, the files the declared licence was not written about.
   *
   * Artwork the user imported (ADR-0024). The pack's `[license]` is a statement its author made
   * about the pack; a backdrop dropped in afterwards travels under that sentence without anybody
   * having said so, and Epoch cannot read what a picture is or who made it.
   *
   * **Said, never refused.** The hard rule governs what Epoch distributes; what somebody hands a
   * friend from their own vault is theirs.
   */
  readonly unvouched: readonly string[];
  /** Why it cannot run. Empty means it can. */
  readonly problems: readonly string[];
}

export interface ProviderStatus {
  /** Stable id. What a command addresses; never the display name. */
  readonly id: string;
  /**
   * What the **program** is called — `Ollama`, `LM Studio`, or whatever the user typed.
   *
   * Not the computer. A Bridge answered with the machine's name here while a machine was one
   * program, and the crew editor's `Brain` list duly offered `studio-mac.local` as
   * something to think with.
   */
  readonly name: string;
  /**
   * Which computer it runs on, when that is not this one. `null` is this machine.
   *
   * Said by the Engine rather than derived from the endpoint here: a Bridge was told the
   * machine's real name at pairing time, and a name a person chose beats a host parsed out of
   * an IP.
   */
  readonly machine: string | null;
  /** Where Epoch looked. Shown so OFFLINE is a fact the user can go and check. */
  readonly endpoint: string;
  /** True only when the provider actually answered. */
  readonly online: boolean;
  /** True when it runs on the user's own machine — no key, no account, no bill. */
  readonly local: boolean;
  /** Models it reports having, in the order it reported them. */
  readonly models: readonly string[];
  /** Why it is not online, in plain words. `null` when it is. */
  readonly note: string | null;
}

/**
 * What this build knows how to speak to.
 *
 * Mirrors the Engine's closed set (`backends::Kind`). Adding one here without adding it there
 * produces a configuration that can only fail at the moment somebody tries to think with it,
 * which is why the Engine refuses it rather than trusting this list.
 */
/**
 * What this build knows how to speak to.
 *
 * `openai` is not a vendor: it is the OpenAI *API*, which llama.cpp, LM Studio, vLLM, Deepseek,
 * GLM and most of what ships next all expose. One entry covers them, which is what makes adding
 * the next one an address rather than a release.
 */
export type BackendKind = "ollama" | "anthropic" | "openai";

/**
 * One backend, as the user configured it.
 *
 * Infrastructure, not content: an endpoint belongs to a machine, a temperature belongs to a
 * person (ADR-0026). So this list is the same in every World.
 */
export interface Backend {
  /** Which one you mean. What a character's brain names. Never the display name. */
  readonly id: string;
  readonly kind: BackendKind;
  /** Where to look. `http://` or `https://` — the Engine refuses to guess a scheme. */
  readonly endpoint: string;
  /** Off keeps the address and stops the probing. Different from forgetting it. */
  readonly enabled: boolean;
}

/** Everything configured, plus why the file could not be read if it could not. */
export interface BackendsView {
  readonly backends: readonly Backend[];
  /**
   * Ids of the backends that have a credential stored.
   *
   * Ids only, and that is the whole contract: there is no command that reads a key back, and a
   * `Secret` has no serialisation at all — so the interface can be told *that* one exists and
   * never *what* it is (ADR-0026).
   */
  readonly keyed?: readonly string[];
  /**
   * Present only when `providers.toml` could not be parsed. The list above is then the
   * defaults, and the surface must say so — showing them silently would present them as
   * choices the user made.
   */
  readonly problem?: string | null;
}

/**
 * One MCP server, as the user configured it.
 *
 * Its tools become ordinary capabilities (ADR-0008) — same descriptions, same permission gate,
 * same record. Nothing downstream can tell one from a capability Epoch wrote.
 */
export interface McpServer {
  /** Which one you mean. Becomes part of every capability id it contributes. */
  readonly id: string;
  /** The program to run. **No shell** — the arguments are separate for that reason. */
  readonly command: string;
  readonly args: readonly string[];
  /**
   * Environment the server is given, for values that are **not** credentials — a root path, a
   * bucket, a mode. Plainly here because that is what it is: configuration somebody edits.
   */
  readonly env: Readonly<Record<string, string>>;
  /**
   * The **names** of the variables whose values live in the encrypted store.
   *
   * Names only, and there is no shape in which a value comes back: a credential is written by
   * `saveMcpSecret` and read by nothing. That is why this list can be sent back with an edit
   * without a token ever passing through a surface.
   *
   * Removing a name here and saving deletes the stored value with it.
   */
  readonly secrets: readonly string[];
  readonly enabled: boolean;
}

/** Everything configured, plus why the file could not be read if it could not. */
export interface McpView {
  readonly servers: readonly McpServer[];
  readonly problem?: string | null;
}

/** Whose bridge this is. */
export interface Orchestrator {
  /** What to call them. Never empty — falls back to a title when nobody has said. */
  readonly name: string;
  /** True when nobody has introduced themselves, so the bridge can say so. */
  readonly isUnnamed: boolean;
  /** Their portrait as a `data:` URI, or null when they have not chosen one. */
  readonly portrait: string | null;
  /** What went wrong reading the portrait, if anything. */
  readonly problems: readonly string[];
}

/** One step of a character's routine. */
export interface RoutineStep {
  readonly activity: string;
  readonly seconds: number;
}

/**
 * One member of the crew, as the Launcher shows *and edits* them.
 *
 * Characters are not supplied by Worlds (ADR-0023). They are the user's: name, face,
 * personality, role and routine live in the vault and travel into whichever Worlds the
 * character is assigned to.
 */
export interface CharacterSummary {
  /** Stable identity. What every command addresses; never the name. */
  readonly id: string;
  readonly name: string;
  /** Canonical archetype id. Classification, not identity. */
  readonly archetype: string;
  readonly role: string;
  /** Where the personality actually lives. */
  readonly prompt: string;
  /** Provider id that does their thinking. `null` means nobody assigned one. */
  readonly provider: string | null;
  /**
   * Which agent works for them, or `null` when a model thinks for them instead (ADR-0027).
   *
   * Never set alongside `provider`. A *model* thinks and Epoch owns the loop; an *agent*
   * thinks **and works**, owning its own. Which of these two fields is set is the whole
   * distinction, and it is shown rather than inferred.
   */
  readonly agent: string | null;
  /** The model, as whichever of those two names it. `null` when nobody named one. */
  readonly model: string | null;
  /**
   * **What thinks for them**, in one string.
   *
   * The question every surface was actually asking while it read `model`: a model's name, or
   * the agent's when a model was never named. `null` is the only case that really is *cannot
   * think*.
   */
  readonly brain: string | null;
  /** Canonical parameters, exactly as authored. `null` means unset, never zero. */
  readonly parameters: ParametersView;
  /**
   * Provider-native settings currently in force, as `key = value` strings.
   *
   * The values as stored, in the Provider's own vocabulary. What each one *means* — its
   * bounds, its default, whether that bound was measured — is declared by the Provider and
   * fetched separately (ADR-0026), because only it knows.
   */
  readonly tuning: Readonly<Record<string, TuningValue>>;
  /** Providers whose tuning is kept but dormant, because they are not the one assigned. */
  readonly dormantTuning: readonly string[];
  /**
   * What this character asks to be able to use. **Requested, never available.**
   *
   * `null` is **undecided**, which resolves to everything available at the moment it is asked.
   * Deliberately not materialised by the Engine: a surface that could not tell "everything,
   * always" apart from "these, today" saved the second when the user meant the first, and a
   * server that was briefly unreachable was enough to lose a request nobody had withdrawn.
   */
  readonly requestedCapabilities: readonly string[] | null;
  /** Ways of working they have been given, by id. */
  readonly skills: readonly string[];

  /**
   * Which voice they speak with — the **name**, exactly as authored.
   *
   * Not resolved against what is installed: whether a voice is on this machine is a fact about
   * the machine, and whether somebody chose one is a fact about them. The editor shows both and
   * can only tell them apart because this stays the author's word.
   */
  readonly speaksWith: string | null;
  /** Which converted RVC voice colours it, and at what pitch. Also unresolved, for the same
   * reason: what is installed is a fact about the machine. */
  readonly soundsLike: Timbre | null;
  /** Ids of the Worlds they currently live in. Empty means nobody has placed them yet. */
  readonly worlds: readonly string[];
  /** What they do while idle. Never empty — a character is never frozen. */
  readonly routine: readonly RoutineStep[];
  /** The world sprite they declare, exactly as authored. `null` means none yet. */
  readonly sprite: string | null;
  /** The icon file they declare. `null` means the sprite stands in for it. */
  readonly icon: string | null;
  /** Their face — the icon when they have one, otherwise the sprite. */
  readonly portrait: MarkView | null;
  /** The world sprite, resolved, so the editor can show what it is about to replace. */
  readonly spriteMark: MarkView | null;
  /** The icon, resolved — only when one is authored. Null means it falls back. */
  readonly iconMark: MarkView | null;
  /**
   * One resolved sheet per action they have been drawn doing, keyed by action id.
   *
   * Resolved rather than authored on purpose: a cut that does not match the artwork is only
   * ever found by watching it play, and a preview drawn from the numbers in the file would
   * agree with the file and disagree with the World.
   */
  readonly actionMarks: Readonly<Partial<Record<Action, MarkView>>>;
  /** Where this character lives on disk. The file is the source of truth. */
  readonly file: string;
}

/**
 * What a removal would do, before it happens.
 *
 * Three lists rather than one sentence. A confirmation that only asks "are you sure?" is a
 * confirmation about nothing, and the difference between what goes, what changes and what
 * survives is exactly what somebody needs in order to answer.
 */
export interface RemovalView {
  /** What is being removed, as a person reads it — and what they must type to confirm. */
  readonly what: string;
  /** The files that go, by name. A surface shows what is deleted; the full path of somebody's
   *  vault is not a thing to put on a screen. */
  readonly files: readonly string[];
  /** What this changes without deleting. */
  readonly consequences: readonly string[];
  /** What survives. Said out loud, because a deletion that stays quiet about what it kept reads
   *  as one that missed something. */
  readonly survives: readonly string[];
}

/** One thing this machine either has or does not. */
export interface ReadinessRow {
  readonly id: string;
  readonly name: string;
  /** `ready` · `missing` · `needsYou` · `unset` — measured, or nothing to detect. */
  readonly state: string;
  /** One sentence, never invented: what the thing itself said, or what Epoch needs. */
  readonly note: string;
  /** `nothing` · `signIn` · `configure` — what Epoch can do about it, if anything. */
  readonly act: string;
}

/** One machine paired with this Host. */
export interface PairedMachine {
  readonly id: string;
  readonly name: string;
  readonly address: string;
  /** `compute` — it may think · `surface` — a person may drive this World from it. */
  readonly grants: readonly string[];
  readonly pairedAt: number;
}

/** One destination a turn could go, and what has been answered about it. */
export interface DisclosureRow {
  readonly going: string;
  /** `null` means **not asked yet** — which is neither a yes nor a no. */
  readonly allowed: boolean | null;
  /** What the person is answering, saying what actually travels. */
  readonly asks: string;
}

/**
 * What this machine is, as far as it can be asked.
 *
 * Every field is `null` when nothing could measure it — never a zero. On Windows and Linux
 * `nvidia-smi` is NVIDIA's, so an AMD or Intel card leaves the VRAM readings unknown, and
 * *unknown* is a different answer from *nothing fits*. A Mac is asked differently and answers.
 */
export interface MachineView {
  readonly gpu: string | null;
  readonly vramTotal: number | null;
  readonly vramFree: number | null;
  readonly ramTotal: number | null;
  /**
   * **The graphics memory is the system memory.** True on Apple Silicon.
   *
   * The numbers above are real either way; what changes is what they mean. There is no separate
   * pool to spill into on a unified machine, so a model that does not fit does not become slow —
   * it does not load. Saying "video memory" about it would be a true number described wrongly,
   * which is the same failure as an invented one.
   */
  readonly unified: boolean;
}

/**
 * What a model says it can do.
 *
 * Ollama's own words, because Ollama is the source that actually declares them. `image` is the
 * exception — Hugging Face's `text-to-image`, which Ollama has no word for because it hosts none.
 */
export type Facet = "vision" | "tools" | "thinking" | "audio" | "image";

/** One model, as the Workshop shows it. */
export interface ModelOffer {
  readonly name: string;
  readonly installed: boolean;
  /** Real bytes from this tag's manifest — what a pull downloads. `null` until asked. */
  readonly bytes: number | null;
  /** Whether it fits in free video memory. `null` when either number is unknown. */
  readonly fits: boolean | null;
  /**
   * Where it came from: `"featured"`, `"ollama"`, `"huggingface"` or `"cloud"`.
   *
   * On screen because the sources are not interchangeable. `ollama pull` takes names from two
   * registries — `qwen3-coder` resolves against Ollama's, `unsloth/Qwen3-GGUF` against Hugging
   * Face — and `"cloud"` is a model Ollama runs on *its own servers*, which makes picking it a
   * disclosure rather than a download.
   */
  readonly source: string;
  /** What it declares, in the source's own words. */
  readonly facets: readonly Facet[];
  /**
   * Whether the source describes this model at all.
   *
   * **The honest half of a filter.** Ollama's featured list annotates nothing, so a surface can
   * say *not described* rather than implying it asked and got a no.
   */
  readonly described: boolean;
  /** What to type to get it — `gemma4:12b`, or `hf.co/user/repo`. */
  readonly pull: string;
  /**
   * Whether Ollama can fetch this one.
   *
   * Measured against its own refusal: a sharded tag answers *"Ollama does not yet support
   * pulling sharded GGUF via the registry"*. The file is still fetchable with `hf`, so one
   * destination closes rather than the model disappearing.
   */
  readonly pullable: boolean;
}

/**
 * One thing Epoch is holding that a person may choose to erase.
 *
 * Three facts per row: what it is, what losing it costs, and how much is held right now. A size
 * nobody measured would be an invented reading, and `0` is how somebody learns there is nothing
 * to clear rather than the row quietly vanishing.
 */
export interface ErasableView {
  readonly id: string;
  readonly what: string;
  readonly cost: string;
  readonly bytes: number;
  /** How many files it is. `0` with a cost written beside it means it is not files at all. */
  readonly files: number;
}

/**
 * The choices the Engine actually allows.
 *
 * Sent so no dropdown carries its own copy of the vocabulary: a UI list would drift the
 * moment an archetype is added, and the drift would look like a bug in the World.
 */
export interface Vocabulary {
  readonly archetypes: readonly string[];
  readonly places: readonly string[];
  /** The canonical reasoning ladder. Sent so the UI never holds its own copy. */
  readonly reasoning: readonly string[];
  /**
   * Capabilities offered as tick-boxes. A suggestion list, not a taxonomy.
   *
   * A connected MCP server appears **once**, as `mcp:playwright` — never once per tool.
   */
  readonly capabilities: readonly string[];
  /** Of those, the ones this build can actually do. The rest are intentions, not promises. */
  readonly built: readonly string[];
  /** Which of the above are sources, and what each is offering at this moment. */
  readonly groups: readonly CapabilityGroup[];
}

/* ------------------------------------------------------------------ workshop */

/** Something a catalogue server must be told before it will run. */
export interface WorkshopInput {
  readonly name: string;
  readonly description: string;
  readonly required: boolean;
  /**
   * A credential. **Decides where the value is stored** — the encrypted store rather than
   * `mcp.toml` — which is why it is not merely an input type.
   */
  readonly secret: boolean;
  readonly default: string | null;
  readonly placeholder: string | null;
  readonly choices: readonly string[];
}

/** One way a catalogue entry could become a working connection. */
export type WorkshopOffer =
  | {
      readonly kind: "start";
      readonly registry: string;
      readonly package: string;
      /** The program Epoch would execute, in this platform's spelling. */
      readonly command: string;
      /** Its arguments, in order. Shown verbatim: this is the whole of what would run. */
      readonly args: readonly string[];
      readonly inputs: readonly WorkshopInput[];
    }
  | { readonly kind: "reach"; readonly transport: string; readonly url: string }
  | { readonly kind: "unusable"; readonly why: string };

/** One server as the catalogue describes it. */
export interface WorkshopListing {
  readonly name: string;
  readonly title: string;
  readonly description: string;
  readonly version: string;
  /** Who published it, as the catalogue's own claim. Provenance belongs next to the button. */
  readonly publisher: string;
  readonly repository: string | null;
  /**
   * Which directory inside that repository this entry is, when it is a monorepo.
   *
   * Where the *useful* README lives: a monorepo's root README is about the monorepo, and what
   * somebody installed is one directory inside it.
   */
  readonly subfolder: string | null;
  readonly active: boolean;
  readonly offers: readonly WorkshopOffer[];
  readonly suggestedId: string;
}

/** One card, with this machine's verdict already applied by the Engine. */
export interface WorkshopShown {
  readonly listing: WorkshopListing;
  /** The offer that would actually run, when one would. */
  readonly ready: WorkshopOffer | null;
  /** Why nothing would, in the user's terms. Never composed here. */
  readonly blocked: string | null;
  /**
   * The ids this entry is already installed under.
   *
   * Matched on the catalogue name each server recorded, never on the id — installing renames on
   * collision, so an id match would call a second copy a different server and offer a third.
   */
  readonly installedAs: readonly string[];
}

/** A page of the catalogue. */
export interface WorkshopShelf {
  readonly shown: readonly WorkshopShown[];
  readonly next: string | null;
  /** When this was fetched, epoch milliseconds. Present when it came from the cache. */
  readonly fetchedMs: number | null;
  readonly problem: string | null;
  /** What this machine can run at all. */
  readonly runtimes: { readonly present: readonly string[] };
}

/** One source offering several capabilities under a single name. */
export interface CapabilityGroup {
  /** What the character's file writes to ask for all of it: `mcp:playwright`. */
  readonly id: string;
  /** What a person calls it. Display only — never parsed, never matched on. */
  readonly label: string;
  /**
   * What it is offering right now. Shown so somebody can see what a tick grants before
   * granting it, and deliberately **not** what gets written to the file: the point of asking
   * for a source is that the answer is re-measured every turn.
   */
  readonly tools: readonly string[];
  /**
   * Whether the source answered when it was last asked.
   *
   * A configured source always has a box, answering or not. One that disappeared when its
   * server stopped starting took a character's request off the screen, and the next save
   * wrote that absence to disk.
   */
  readonly answering: boolean;
}

/**
 * Canonical parameters — the ones that survive changing the engine (ADR-0026).
 *
 * `null` is *unset*: the Provider's own default stands. It must stay distinguishable from a
 * value the user chose that happens to equal a common default.
 */
export interface ParametersView {
  readonly temperature: number | null;
  readonly topP: number | null;
  /**
   * **Legacy, and no surface writes it** (ADR-0026 amendment, 2026-08-31).
   *
   * The window belongs to MODELS: `llama-server` takes `--ctx-size` when it spawns the child
   * that holds the model, so two characters on one Brain physically cannot have different
   * windows. A character file that already carries this still loads, and it is still honoured
   * as a ceiling a character may *lower*. Nothing new offers it.
   */
  readonly contextTokens: number | null;
  /**
   * How this character wants to use whatever window MODELS gives it.
   *
   * The half of the old `contextTokens` that is genuinely identity: not *how much window*, but
   * *how to fill the one there is*. `null` leaves the Engine's default (`Adaptive`).
   */
  readonly contextPolicy: string | null;
  /** A canonical level id from `Vocabulary.reasoning`, or null. */
  readonly reasoning: string | null;
}

/**
 * What a character's Brain inherits from MODELS. Read-only, always.
 *
 * ## The number has to say whose it is
 *
 * `64K` means three different things depending on who decided it, so `source` travels with it —
 * a correct reading of the right quantity with nothing naming what it is a reading of is the
 * quietest way a gauge lies. `window: null` is *nobody has configured this*, and the panel says
 * that rather than reaching for a plausible constant.
 */
export interface Inherited {
  readonly model: string;
  readonly window: number | null;
  /** `profile` · `loadout` · `reported` · `nothing`. */
  readonly source: "profile" | "loadout" | "reported" | "nothing";
  /** Which profile, where one was applied. `null` for every other source. */
  readonly profile: string | null;
  /**
   * Tokens per second, **and only ever from an applied profile**. A model's last timing was
   * taken at whatever loadout was current that afternoon, so showing it beside a window chosen
   * afterwards would be a real reading of a different configuration.
   */
  readonly generation: number | null;
  readonly stable: boolean;
}

/* -------------------------------------------------------------------------- */
/* The ship's log                                                             */
/* -------------------------------------------------------------------------- */

/** One Quest, as the log lists it. Completed, failed and in-flight alike. */
export interface LoggedQuest {
  readonly world: string;
  readonly worldName: string;
  readonly title: string;
  /** A canonical state id — "completed", "blocked", "working"… */
  readonly state: string;
  /** How much real evidence it left behind. Zero is a real and important answer. */
  readonly evidence: number;
  readonly said: number;
  readonly at: number;
}

/** One thing a character actually ran. */
export interface LoggedRun {
  readonly world: string;
  readonly worldName: string;
  /** Their identity, never their name — a name is theirs to change. */
  readonly character: string;
  readonly summary: string;
  readonly at: number;
}

/**
 * What has happened, across every World.
 *
 * Read from the vault rather than from this session: close Epoch, reopen it, and this is the
 * same. That is the difference between a log and a screen that remembers.
 */
export interface ShipsLogView {
  readonly quests: readonly LoggedQuest[];
  readonly runs: readonly LoggedRun[];
}

/** An edit to one character, as the Launcher submits it. */
export interface CharacterEdit {
  readonly id: string;
  readonly name: string;
  readonly archetype: string;
  readonly role: string;
  readonly prompt: string;
  /** Provider id, or null to leave them unable to think. */
  readonly provider?: string | null;
  /**
   * Agent id, when this character works with one instead (ADR-0027).
   *
   * Never sent alongside `provider`. Which of the two is present **is** the choice — a `kind`
   * field beside them would be a third thing that could disagree with both.
   */
  readonly agent?: string | null;
  /** Model name. Only meaningful together with a provider. */
  readonly model?: string | null;
  readonly routine: readonly RoutineStep[];
  // Artwork is absent on purpose: it has its own command, so this form can never erase a
  // face by carrying a stale filename.
  readonly parameters: ParametersView;
  /**
   * Intent, never a claim. What is actually available is answered by the Provider.
   *
   * `null` means the form never touched the tick-boxes, and undecided stays undecided.
   */
  readonly requestedCapabilities: readonly string[] | null;
  /**
   * Ways of working to give them, or `null` to leave what they have.
   *
   * `null` is **untouched**, for the reason the field above is: a form that does not manage
   * Skills must not be able to take away every Skill somebody had by staying silent.
   */
  readonly skills?: readonly string[] | null;
  /**
   * Which voice to give them, or `null` to leave whatever they have.
   *
   * Three states, like the two fields above: `null` untouched, `""` the deliberate silence a
   * form does say when somebody picks the empty option, and a name for a choice.
   */
  readonly speaksWith?: string | null;
  /**
   * Which timbre to colour it with, or `null` to leave whatever they have.
   *
   * The same three states, and the empty **voice name** is what clears it — a timbre with no
   * model is not a timbre. The pitch travels inside, so it cannot be edited apart from the
   * model it is a pitch for.
   */
  readonly soundsLike?: Timbre | null;
  /**
   * Provider-native values, keyed by the Provider's own names (ADR-0026).
   *
   * Only the assigned Provider's, because those are the only ones it declared. The Engine
   * refuses a name it never offered and a value outside the bounds it stated — a surface's
   * check is a courtesy, never the control.
   */
  readonly tuning: Readonly<Record<string, TuningValue>>;
}

/** A value in a Provider's own vocabulary. Flat scalars only. */
export type TuningValue = boolean | number | string;

/**
 * What a Provider says about one model.
 *
 * The window is separate from the knobs because it is **canonical**: every backend has one and
 * it means the same thing everywhere, which is what makes it the ceiling on `contextTokens`.
 * Deriving that ceiling from a control named `num_ctx` would teach this file a Provider's own
 * parameter name — the mistake ADR-0026 exists to prevent.
 */
export interface Surface {
  /** How much the model holds. `null` is *unknown*, and unknown is never a guess. */
  readonly window: number | null;
  readonly controls: readonly Control[];
  /**
   * What the backend says this model can **do**, when it says anything.
   *
   * `null` is *unasked* — never "cannot". A backend that did not answer has told us nothing,
   * and drawing that as a row of empty capabilities would be an invented reading.
   */
  readonly can: Declared | null;
}

/**
 * A model's own account of itself.
 *
 * Every backend publishes one and Epoch read none of them until somebody asked whether an agent
 * could see. Measured: `gemma4:12b` says vision **and audio**, `gemma4:26b` says vision without
 * audio, `qwen3:14b` says neither — a set of differences nobody could have listed from memory.
 */
export interface Declared {
  readonly sees: boolean;
  readonly hears: boolean;
  /**
   * **`null` is unasked, never *no*.** The only one of these that changes what Epoch does, and
   * the only one that needed a third state: llama.cpp and LM Studio describe a model's
   * modalities and say nothing about tools, and reading that silence as a refusal left every
   * character on those backends with no capabilities at all.
   */
  readonly usesTools: boolean | null;
  readonly thinks: boolean;
}

/**
 * One knob a Provider says it has, for one model.
 *
 * Rendered generically. This interface is the *whole* of what a surface knows about any
 * backend's settings — a frontend that knew Ollama has `num_ctx` would need editing to add a
 * Provider (ADR-0003, ADR-0026).
 */
export interface Control {
  /** The Provider's own name, and the key the value is stored under. */
  readonly name: string;
  readonly label: string;
  readonly help: string;
  readonly kind: ControlKind;
  /** What the Provider does when nobody sets it. Shown, never pre-filled. */
  readonly default?: TuningValue | null;
  /**
   * True when the bound was asked for rather than hardcoded.
   *
   * Worth showing: a context limit read from the model is a fact, and the same number written
   * into our source is a guess that goes stale in silence.
   */
  readonly measured: boolean;
}

export type ControlKind =
  | { readonly kind: "toggle" }
  | { readonly kind: "whole"; readonly min: number; readonly max: number }
  | { readonly kind: "ratio"; readonly min: number; readonly max: number }
  | { readonly kind: "choice"; readonly options: readonly string[] }
  | { readonly kind: "free" };

/**
 * One authored way of working, as the Launcher shows it.
 *
 * The `method` is deliberately absent: it is prose for a model, it can be pages long, and this
 * list rides every survey. What somebody choosing a Skill needs is its name, a line about it,
 * and what it will want.
 */
export interface SkillSummary {
  readonly id: string;
  readonly name: string;
  readonly summary: string;
  /**
   * What the method asks for — **a request, never a grant**.
   *
   * Here so a surface can say *this Skill wants Playwright and this character does not have it*
   * at the moment it is assigned, rather than letting it fail on its third step.
   */
  readonly requires: readonly string[];
}

/** Everything the Launcher knows. */
export interface LauncherView {
  /** Whose bridge this is. Never absent — an unnamed orchestrator is still an orchestrator. */
  readonly orchestrator: Orchestrator;
  readonly worlds: readonly WorldSummary[];
  /** Who exists, across every World. */
  readonly characters: readonly CharacterSummary[];
  readonly vocabulary: Vocabulary;
  /** Every way of working this vault holds. Empty is ordinary. */
  readonly skills: readonly SkillSummary[];
  /** How long this session has been running, in seconds. Measured, not dressed. */
  readonly sessionSeconds: number;
  /** Problems finding Worlds at all, as opposed to problems inside one. */
  readonly problems: readonly string[];
  /** Problems with the crew: a file that will not parse, two claiming one identity. */
  readonly definitionProblems: readonly string[];
}

/** The World as the presentation layer receives it. */
/* -------------------------------------------------------------------------- */
/* The World Editor (ADR-0028)                                                */
/* -------------------------------------------------------------------------- */

/** What the user said about one Place. Every field optional — absent leaves the pack's answer. */
export interface PlaceEntry {
  readonly name?: string | null;
  readonly concept?: string | null;
  readonly mark?: string | null;
  readonly spot?: Spot | null;
}

/**
 * Where the user put a Place, in world units.
 *
 * No `zOrder`: draw order already falls out of `y`, and an authored override is the pack
 * author'''s business rather than something a drag gesture can invent.
 */
export interface Spot {
  readonly x: number;
  readonly y: number;
  /** Absent leaves the pack'''s size alone — moving a building must not resize it. */
  readonly footprint?: number | null;
}

/** A way between two Places. Carries handoffs; never blocks them. */
export interface Road {
  readonly from: string;
  readonly to: string;
  /** Corners the World owner traced. The endpoints remain the Places themselves. */
  readonly via?: readonly (readonly [number, number])[];
}

/** One World's map, as the editor works on it. */
export interface WorldMap {
  readonly places: Readonly<Record<string, PlaceEntry>>;
  readonly roads: Readonly<Record<string, Road>>;
  /**
   * The filename of the land its owner painted, if any — never the image itself.
   *
   * A full-map illustration is megabytes and this map is read after every edit. The bytes come
   * from `world_land`, once.
   */
  readonly land?: string | null;
}

export interface WorldView {
  /** Display name of the active World, or null when none is loaded. */
  readonly packName: string | null;
  /** The geography, or null when no World declares one. A world with no map is valid. */
  readonly map: MapView | null;
  /** Every Place, already in draw order. */
  readonly places: readonly PlaceView[];
  readonly characters: readonly CharacterView[];
}

/**
 * How the World is being sourced right now.
 *
 * The UI uses this only to be *honest* — never to change behaviour. The World always
 * renders; it simply tells the truth about what it can see.
 *
 * - `live` — the engine answered, and we are following its changes.
 * - `static` — the engine answered once, but we cannot follow changes. The World the user
 *   is looking at is frozen. This must never be presented as if it were live: a UI that
 *   silently stops following is a UI that lies.
 * - `unavailable` — the engine is not answering at all.
 */
export type WorldStatus = "live" | "static" | "unavailable";

export interface WorldSnapshot {
  readonly status: WorldStatus;
  readonly view: WorldView;
  /** Present only when status is "unavailable". Shown plainly, never as a crash. */
  readonly note?: string;
}
