//! The Epoch desktop shell.
//!
//! This binary owns **no business logic** (ADR-0003). It starts the engine, exposes IPC
//! commands, forwards the engine's projections, and nothing else. Everything it can do, the
//! engine can also do headless.

// Keep the console window hidden on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent;
mod microphone;
mod state;

use std::sync::Arc;
use std::time::Duration;

use tauri::{Emitter, Manager};

use state::World;

/// Text selected on the surface and deliberately offered with one message.
///
/// This is an IPC shape, not a filesystem handle. The WebView has already read the file the
/// person chose, and only its name plus text cross into Epoch's durable Chronicle.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttachmentInput {
    name: String,
    content: String,
}

/// An image chosen on the surface and offered with one message.
///
/// The bytes cross once, base64-encoded, and only the Engine writes them (ADR-0024). The
/// `name` is what the user's file was called; it is kept to be shown back to them and never
/// used to find anything — the Engine names the file from what it contains.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageInput {
    name: String,
    data: String,
}

/// What the live desktop engine accepted for one message.
///
/// Returning this explicitly makes a hot-reloaded WebView distinguish an older Rust process
/// (which silently ignores unknown IPC fields) from an engine that really recorded the selected
/// references. The surface must never pretend an attachment landed without this acknowledgement.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SpeakStarted {
    attachments: usize,
    /// Counted for the same reason, and separately: a picture that never reached the Engine
    /// must not be reported as shared just because no text file travelled with it.
    images: usize,
}

const MAX_ATTACHMENTS_PER_MESSAGE: usize = 3;
// This is context, not a file-transfer channel. Keeping the total at roughly four thousand
// tokens leaves room for the user request, the character and recent conversation even on an
// 8k model window; larger files need a future document/artifact path rather than silently
// crowding out the work they were meant to inform.
const MAX_ATTACHMENT_BYTES: usize = 8 * 1024;
const MAX_ATTACHMENT_TOTAL_BYTES: usize = 16 * 1024;

/// The most images one message may carry.
///
/// Same reasoning as the text limit: a message is a message, not an album. Each is also capped
/// at `import::MAX_BYTES` by the Engine that writes it.
const MAX_IMAGES_PER_MESSAGE: usize = 4;

/// Keep every image offered with one message, and hand back what they became.
///
/// Writing happens here and not in `state`, because this is the IPC boundary and the bytes must
/// not travel any further into the Engine than the one function that stores them.
fn keep_images(
    inputs: Option<Vec<ImageInput>>,
) -> Result<Vec<epoch_kernel::ImageAttachment>, String> {
    let inputs = inputs.unwrap_or_default();
    if inputs.len() > MAX_IMAGES_PER_MESSAGE {
        return Err(format!(
            "attach at most {MAX_IMAGES_PER_MESSAGE} images to one message"
        ));
    }
    inputs
        .into_iter()
        .map(|input| {
            let name = input.name.trim();
            // The name never reaches the filesystem, and is still refused when it is a path:
            // it is *shown*, and a surface that displays `../../etc/passwd` as a caption is
            // displaying something the user did not choose.
            if name.is_empty()
                || name.len() > 120
                || name.contains(std::path::is_separator)
                || name.chars().any(char::is_control)
            {
                return Err("an image needs a short file name, not a path".to_owned());
            }
            epoch_engine::import::keep_shared_image(&state::vault_dir(), name, &input.data)
                .map_err(|err| format!("{name}: {err}"))
        })
        .collect()
}

fn validate_attachments(
    inputs: Option<Vec<AttachmentInput>>,
) -> Result<Vec<epoch_kernel::TextAttachment>, String> {
    let inputs = inputs.unwrap_or_default();
    if inputs.len() > MAX_ATTACHMENTS_PER_MESSAGE {
        return Err(format!(
            "attach at most {MAX_ATTACHMENTS_PER_MESSAGE} text files to one message"
        ));
    }
    if inputs.iter().enumerate().any(|(at, input)| {
        inputs[..at]
            .iter()
            .any(|earlier| earlier.name.trim().eq_ignore_ascii_case(input.name.trim()))
    }) {
        return Err("attach each file name only once per message".to_owned());
    }

    let mut total = 0usize;
    inputs
        .into_iter()
        .map(|input| {
            let name = input.name.trim();
            if name.is_empty() || name.len() > 120 || name.contains(['/', '\\', '\r', '\n', '\0']) {
                return Err("an attachment needs a short file name, not a path".to_owned());
            }
            if input.content.contains('\0') {
                return Err(format!("{name} is not a text file"));
            }
            let bytes = input.content.len();
            if bytes > MAX_ATTACHMENT_BYTES {
                return Err(format!("{name} is larger than 8 KB"));
            }
            total += bytes;
            if total > MAX_ATTACHMENT_TOTAL_BYTES {
                return Err("attachments together are larger than 16 KB".to_owned());
            }
            Ok(epoch_kernel::TextAttachment {
                name: name.to_owned(),
                content: input.content,
            })
        })
        .collect()
}

/// What counts as the World being *different*, rather than further along.
///
/// Deliberately not `PresenceState` itself. Presence now carries progress and an ETA, so a
/// character walking across the map is a different value on every tick — comparing the whole
/// thing would send the entire World, artwork included, twice a second for thirty seconds.
///
/// Who, where, what, which class, and where they are bound. Everything a surface would need
/// more than a position for; nothing that merely advances.
type Shape = (
    String,
    String,
    String,
    epoch_kernel::ActivityClass,
    Option<String>,
);

fn shape(presence: &epoch_kernel::PresenceState) -> Shape {
    (
        presence.character.to_string(),
        presence.place.to_string(),
        presence.activity.clone(),
        presence.class,
        presence.journey.as_ref().map(|j| j.to.to_string()),
    )
}

/// How often the simulation is advanced. Presence is *published* only when it actually
/// changes, so this is a sampling rate, not an event rate (ADR-0018: coarse, event-driven).
const TICK: Duration = Duration::from_millis(500);

/// The event the World listens on. The payload is a full `WorldView`.
const WORLD_CHANGED: &str = "world:changed";

/// Somebody moving, and nothing else.
///
/// A separate channel from `world:changed` because the two answer different questions. The World
/// changed when its *shape* did — somebody set out, arrived, started work, a definition was
/// edited — and that payload is every Place, every mark and every face. A walk changes only a
/// number, ten times, and re-sending the World for each of those is the mistake the backdrop
/// already taught us (`CLAUDE.md`: large assets stay out of the per-tick projection).
///
/// The payload is a list of `PresenceView`: who, where, what, and how far along.
const WORLD_MOVED: &str = "world:moved";

/// Return the current World.
///
/// A projection, not engine internals. Always succeeds.
#[tauri::command]
fn get_world(world: tauri::State<'_, Arc<World>>) -> epoch_engine::WorldView {
    world.view()
}

/// Every installed World, for the Launcher.
///
/// A second projection over the same Engine. The Launcher and the World share no
/// presentation and duplicate no logic (`PRODUCT_ARCHITECTURE.md`).
#[tauri::command]
fn list_worlds(world: tauri::State<'_, Arc<World>>) -> epoch_engine::LauncherView {
    world.worlds()
}

/// Enter a World, by identity. Returns false when no installed World has that id.
#[tauri::command]
fn enter_world(world: tauri::State<'_, Arc<World>>, id: String) -> bool {
    world.enter(&id)
}

/// Open the door an agent knocks on, acting for one character (step 5.2).
///
/// Returns the URL and the token to paste into the agent's own configuration. Not opened at
/// startup and not opened for nobody: a socket that can run shell commands should be something
/// somebody did, for somebody named.
#[tauri::command]
fn open_agent_door(
    app: tauri::AppHandle,
    world: tauri::State<'_, Arc<World>>,
    character_id: String,
    port: Option<u16>,
) -> Result<agent::Bridge, String> {
    world.open_agent_door(
        &character_id,
        port.unwrap_or(epoch_engine::endpoint::DEFAULT_PORT),
        app,
    )
}

/// Write the agent's configuration into this World's project.
///
/// Both halves: the server to reach Epoch, and the deny list that takes the agent's own file
/// tools away. The second is the one that matters — without it the agent never knocks, and
/// everything else would have been theatre.
#[tauri::command]
fn adopt_project(
    world: tauri::State<'_, Arc<World>>,
) -> Result<epoch_engine::adopt::Adopted, String> {
    world.adopt_project()
}

/// Close it. Whatever was being asked is withdrawn, which refuses it.
#[tauri::command]
fn close_agent_door(world: tauri::State<'_, Arc<World>>) {
    world.close_agent_door();
}

/// Whether Epoch is listening, for whom, and what is being asked right now.
#[tauri::command]
fn agent_bridge(world: tauri::State<'_, Arc<World>>) -> agent::Bridge {
    world.agent_bridge()
}

/// The configuration to paste into an agent, written out rather than described.
#[tauri::command]
fn agent_configuration(world: tauri::State<'_, Arc<World>>) -> Option<String> {
    let bridge = world.agent_bridge();
    Some(agent::configuration(
        bridge.url.as_deref()?,
        bridge.token.as_deref()?,
    ))
}

/// Answer the question an agent is waiting on.
///
/// `false` means the question is gone rather than that something failed — the deadline can pass
/// between the prompt being drawn and the button being pressed.
#[tauri::command]
fn answer_agent(
    world: tauri::State<'_, Arc<World>>,
    approve: bool,
    always: bool,
) -> Result<bool, String> {
    world.answer_agent(approve, always)
}

/// Which agents this machine has, and which it does not.
///
/// Every one, including the missing: a surface that listed only what was installed could not
/// say *Claude Code is not installed*, which is the sentence somebody needs in order to go and
/// install it.
/// `fresh` runs the programs again instead of answering with what was measured. It is what the
/// panels' own REMEASURE press sends, and the reason the reading can be kept at all: a cached
/// measurement with no way to repeat it would be a claim rather than a reading.
///
/// Off the window thread: `fresh` runs the agents' own programs, and even a cached read is a
/// lock this thread should not be holding while the deck is trying to paint.
#[tauri::command]
async fn list_agents(
    world: tauri::State<'_, Arc<World>>,
    fresh: Option<bool>,
) -> Result<Vec<epoch_engine::agent::AgentStatus>, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.agents(fresh.unwrap_or(false)))
        .await
        .map_err(|err| err.to_string())
}

/// The signed-in account's current plan allowance for an agent that can report one.
///
/// This is separate from `list_agents`: presence is a short probe, while an allowance may require
/// a second local protocol. `None` means unavailable, never exhausted and never an inferred token
/// percentage.
#[tauri::command]
async fn agent_plan_usage(agent: String) -> Option<Vec<epoch_engine::agents::PlanUsage>> {
    // Both supported agents start a short-lived local program to read their allowance. Keeping
    // that blocking work off Tauri's command runtime means the first World paint, Launcher
    // refresh and ordinary IPC reads never wait behind a command-line probe.
    tauri::async_runtime::spawn_blocking(move || {
        // **Per account, resolved here.** An allowance belongs to a sign-in: two Claude Code
        // accounts have two separate windows, and reading one of them twice would be a real
        // reading of the wrong quantity.
        let account = state::agent_accounts()
            .into_iter()
            .find(|it| it.id == agent)?;
        epoch_engine::agents::plan_usage_for(&account)
    })
    .await
    .ok()
    .flatten()
}

/// What an agent last said about its own window, so the gauge survives leaving the World.
///
/// Epoch can recompose a *model's* context and measure it again — it built it. An agent's
/// context is the agent's, so the only honest number is the one it last reported, and `null`
/// means nobody has taken a turn this session rather than "empty".
#[tauri::command]
fn remembered_context(
    world: tauri::State<'_, Arc<World>>,
    character: String,
) -> Option<serde_json::Value> {
    world
        .remembered_context(&character)
        .map(|(used, budget)| serde_json::json!({ "used": used, "budget": budget }))
}

/// The last real context readings for every member who has spoken in the active Quest.
///
/// One projection means a Crew card never assembles readings from N separate IPC calls after the
/// user has changed conversations. Missing members did not report a window; they are absent,
/// never represented as an invented empty percentage.
#[tauri::command]
fn remembered_contexts(world: tauri::State<'_, Arc<World>>) -> Vec<state::ContextWindow> {
    world.remembered_contexts()
}

/// What a mode actually does for this agent.
///
/// Asked of the Engine because the two agents differ: Claude Code asks before its own tools;
/// Codex pauses its native tool request and Epoch asks before allowing that individual action.
/// `null` means Epoch has nothing measured to say, and the surface falls back to the mode's own
/// description.
#[tauri::command]
fn gate_note(agent: String, mode: String) -> Option<String> {
    let autonomy = epoch_kernel::Autonomy::from_id(&mode)?;
    epoch_engine::agents::gate(&agent, autonomy).map(str::to_owned)
}

/// What an agent calls its own models.
///
/// From the Engine, because the answer differs per agent — Claude Code takes aliases like `opus`,
/// Codex takes `gpt-5.6-sol`. A list in a component was correct while there was one agent and
/// became wrong the moment there were two.
///
/// **Asked, not remembered.** The Engine puts the question to the installed program where there
/// is one to put it to, so a model that shipped this morning is in the list this morning. The
/// account is resolved here because *which sign-in* is a settings question and the Engine is
/// handed the answer rather than looking it up — and because an allowance and a model list
/// belong to the same thing: a sign-in, never a program.
///
/// Suggested, never closed: the field stays typeable either way.
#[tauri::command]
async fn agent_models(agent: String) -> Vec<(String, String)> {
    // Off the command runtime, exactly like the allowance beside it: two of the three programs
    // answer this by starting a short-lived process, and the World's first paint must never wait
    // behind one.
    tauri::async_runtime::spawn_blocking(move || {
        let account = state::agent_accounts()
            .into_iter()
            .find(|it| it.id == agent)
            .unwrap_or_else(|| {
                // Not a sign-in this machine knows about, so the program's own default one.
                // Never a refusal: a picker that empties itself because a lookup missed says
                // nothing a person can act on.
                epoch_engine::agents::Account::default_of(epoch_engine::agents::kind_of(&agent))
            });
        epoch_engine::agents::models_for(&account)
    })
    .await
    .unwrap_or_default()
}

/// One character's reasoning ladder, and where they are on it.
///
/// The **brain** declares the rungs (ADR-0026): a model's ladder and Claude Code's are different
/// lengths and do not contain the same levels, so a surface that hardcoded either would be wrong
/// for the other.
#[tauri::command]
fn reasoning_dial(
    world: tauri::State<'_, Arc<World>>,
    character: String,
) -> Result<epoch_engine::deliberation::Dial, String> {
    world.reasoning_dial(&character)
}

/// Move a character along their ladder. `null` means the brain's own default.
#[tauri::command]
fn choose_reasoning(
    world: tauri::State<'_, Arc<World>>,
    character: String,
    rung: Option<String>,
) -> Result<(), String> {
    world.choose_reasoning(&character, rung)
}

/// Open an agent's own sign-in.
///
/// Epoch never asks for the credential itself. This starts the agent's official login in its own
/// window and returns — the user signs in with the agent, and Epoch measures the result
/// afterwards with `list_agents`.
#[tauri::command]
fn sign_in_agent(world: tauri::State<'_, Arc<World>>, agent: String) -> Result<(), String> {
    world.sign_in_agent(&agent)
}

/// Add a second sign-in of one agent program, and start its login.
///
/// Epoch makes the folder and hands the agent its own environment variable pointing at it; the
/// agent runs its own sign-in in its own window. Epoch shows no password field and stores no
/// token — it opens the door and never holds the key.
#[tauri::command]
fn add_agent_account(
    world: tauri::State<'_, Arc<World>>,
    kind: String,
    label: String,
) -> Result<String, String> {
    let id = world.add_agent_account(&kind, &label)?;
    // Straight into the agent's own login. Adding an account and not signing it in would leave a
    // row that reads `not signed in` with no obvious way to fix it.
    world.sign_in_agent(&id)?;
    Ok(id)
}

/// Rename one added account. Its id never changes, because a character's brain names the id.
#[tauri::command]
fn rename_agent_account(
    world: tauri::State<'_, Arc<World>>,
    id: String,
    label: String,
) -> Result<(), String> {
    world.rename_agent_account(&id, &label)
}

/// Forget an added account. Its sign-in is left where it is, and the answer says so.
#[tauri::command]
fn remove_agent_account(world: tauri::State<'_, Arc<World>>, id: String) -> Result<String, String> {
    world.remove_agent_account(&id)
}

/// Every World's Quests and the crew's runs, newest first.
///
/// The Ship's Log and the Bridge Console's readout come from here. Both were cold, waiting on
/// the Activity Recorder (ADR-0015) — which is still unbuilt and was the wrong thing to wait
/// for: Quests already persist, and a run is already recorded as evidence (ADR-0025).
#[tauri::command]
fn ships_log(world: tauri::State<'_, Arc<World>>) -> epoch_engine::ShipsLog {
    world.ships_log()
}

/// Look at a folder before a World is pointed at it, and report what is there.
///
/// Facts, never a verdict. Epoch does not decide whether a folder is a good project.
#[tauri::command]
fn scan_project(
    world: tauri::State<'_, Arc<World>>,
    path: String,
) -> Result<epoch_engine::project::ProjectScan, String> {
    world.scan_project(&path)
}

/// Make a World and enter it, stopped and ready to be authored.
///
/// Returns its identity. The surface goes straight to the World Editor: a World you have just
/// made is empty, and empty is exactly where authoring starts.
///
/// Name, project root and starting crew — and nothing else. Providers, parameters, capabilities,
/// keys and packs are configuration; asking for them here would make creating a World feel like
/// commissioning one.
#[tauri::command]
fn create_world(
    world: tauri::State<'_, Arc<World>>,
    name: String,
    project_root: Option<String>,
    crew: Vec<String>,
) -> Result<String, String> {
    world.create_world(&name, project_root.as_deref(), &crew)
}

/// Rename an installed World. Its identity never changes — only what it is called.
#[tauri::command]
fn rename_world(
    world: tauri::State<'_, Arc<World>>,
    id: String,
    name: String,
) -> Result<(), String> {
    world.rename(&id, &name)
}

/// Leave the World and return to the Launcher.
#[tauri::command]
fn leave_world(world: tauri::State<'_, Arc<World>>) {
    world.leave();
}

/// Save an edit to one member of the crew.
///
/// The shell validates nothing: it hands the edit to the Engine, which owns what a valid
/// character is and has to live with the file afterwards.
#[tauri::command]
fn save_character(
    world: tauri::State<'_, Arc<World>>,
    edit: epoch_engine::CharacterEdit,
) -> Result<(), String> {
    world.save_character(edit)
}

/// Events a turn emits, in order. One per character at a time.
/// This is emitted synchronously after a Quest has been prepared, before the worker is spawned.
/// A surface can therefore switch windows without losing the identity that must later receive
/// `turn:ended`.
/// One setting tried, out of the bound a search is drawn against.
///
/// Its own event rather than a field on anything: a search is the only job here that takes
/// minutes, and four minutes of a button reading `MEASURING…` is a job somebody assumes has hung.
const MEASURING: &str = "models:measuring";

/// How far a download has got, as it goes.
///
/// **Six gigabytes with no reading at all** was what a fetch said before this: *this takes as
/// long as it takes*, which is true and is not a number. The same shape as `MEASURING` — the
/// row's own id, what has arrived, and the total when the source stated one. `null` there is
/// **no total**, never zero: a bar guessing at a denominator is the invented gauge.
const FETCHING: &str = "workshop:fetching";

/// One step of First Run, as it happens.
///
/// Every line the program writes rides this too, so `SHOW DETAILS` is a record of what actually
/// happened rather than a summary of it.
const PREPARING: &str = "firstrun:step";

/// What a benchmark is doing now.
///
/// A Standard run is a speed reading and fourteen trials, each of them a request to a model that
/// may take seconds. A job of that length that speaks only at the end is one somebody kills at
/// the halfway mark.
const BENCHING: &str = "models:benching";

const TURN_STARTED: &str = "turn:started";
const TURN_TOKEN: &str = "turn:token";
const TURN_ENDED: &str = "turn:ended";
/// One piece of work, as it happens. Separate from the token stream — see the emit site.
const TURN_STEP: &str = "turn:step";
/// What a character was told this turn. The Context Report of ADR-0012.
const TURN_KNEW: &str = "turn:knew";
/// Which model an agent actually thought with, reported by the agent after the fact.
///
/// Its own event rather than a field on [`TURN_KNEW`], because the two are not the same
/// measurement and most agents report only one of them: Gemini names the model and reports no
/// window; Claude Code reports the window. A payload carrying both would be half-empty in every
/// real run, and a surface reading it could not tell absent from unreported.
const TURN_THOUGHT: &str = "turn:thought";
/// The maintenance task has been prepared and the external agent is now reducing its session.
/// It is separate from the generic turn start so the dialogue can say what is really happening.
const TURN_COMPACTING: &str = "turn:compacting";
/// The external agent session behind the last measurement was deliberately retired.
const TURN_CONTEXT_CLEARED: &str = "turn:context-cleared";

/// How a finished turn reaches the World.
///
/// One shape whether the turn ran normally or resumed from an approval — a surface should not
/// have to know which, and two payloads would eventually disagree about a field.
fn ended_payload(
    character_id: &str,
    quest_id: &str,
    outcome: &Result<epoch_engine::Completed, String>,
    invited: &[String],
) -> serde_json::Value {
    match outcome {
        Ok(completed) => serde_json::json!({
            "characterId": character_id,
            "questId": quest_id,
            "answer": completed.text,
            "error": null,
            "invited": invited,
            "rounds": completed.rounds,
            "evidence": completed.evidence,
            // What was consulted, so the answer can be checked. Distinct from evidence: one is
            // what the Quest produced, the other is what it read (ADR-0025).
            "sources": completed.sources.iter().map(|s| serde_json::json!({
                "title": s.title,
                "url": s.url,
            })).collect::<Vec<_>>(),
            // **How many rounds it took**, so the notice can say the number rather than the word.
            //
            // The owner watched a character say *"Te lo reproduzco ahora:"* and stop, and read
            // it as the character being limited. It was: eight rounds is the whole budget for a
            // turn, and a conversation with an MCP server attached spends them on searches. The
            // Engine has counted them the whole time and the surface said *"the round limit"* —
            // a limit nobody can see the size of is one people assume is arbitrary.
            "rounds": completed.rounds,
            // Why it stopped. `finished` is the ordinary ending; the other two are things the
            // user has to be told rather than left to infer from a short answer.
            "stop": match &completed.stop {
                epoch_engine::Stop::Finished => "finished",
                epoch_engine::Stop::RoundsExhausted => "rounds_exhausted",
                epoch_engine::Stop::Stopped => "stopped",
                epoch_engine::Stop::NeedsApproval(_) => "needs_approval",
                epoch_engine::Stop::NeedsCapability(_) => "needs_capability",
            },
            // Flattened into strings a surface can put on screen, exactly like `step_payload`.
            //
            // Serialising the `Explanation` wholesale sent `reversal` as `{"kind":"permanent"}`,
            // and React threw the moment a component rendered that object as a child — a
            // blank window from one field with the wrong shape. A projection crossing this
            // boundary is presentation data; if a surface has to unpack a domain enum to draw
            // it, the projection was not finished.
            "awaiting": match &completed.stop {
                epoch_engine::Stop::NeedsApproval(paused) => serde_json::json!({
                    "capability": paused.explanation.capability.as_str(),
                    "what": paused.explanation.what,
                    "risk": paused.explanation.risk.as_str(),
                    "reversal": paused.explanation.reversal.describe(),
                    "why": paused.explanation.why,
                    // The change itself, when the capability could show it without making it
                    // (ADR-0009 — Preview Support). A description tells you what will happen;
                    // this lets you notice that the wrong thing is about to.
                    "preview": paused.explanation.preview,
                }),
                _ => serde_json::Value::Null,
            },
            // A capability the character reached for and was never given.
            //
            // Carries what it *is* rather than only its name: somebody deciding whether to hand
            // over `run_command` should be told it runs code on their machine and cannot be
            // undone, not asked to remember.
            "wanted": match &completed.stop {
                epoch_engine::Stop::NeedsCapability(wanted) => serde_json::json!({
                    "capability": wanted.descriptor.id.as_str(),
                    "summary": wanted.descriptor.summary,
                    "risk": wanted.descriptor.risk().as_str(),
                    "reversal": wanted.descriptor.reversal.describe(),
                    "effects": wanted.descriptor
                        .effects
                        .iter()
                        .map(|e| e.describe())
                        .collect::<Vec<_>>(),
                }),
                _ => serde_json::Value::Null,
            },
        }),
        Err(err) => serde_json::json!({
            "characterId": character_id,
            "questId": quest_id,
            "answer": null,
            "error": err,
            "invited": [],
            "rounds": 0,
            "evidence": [],
            "sources": [],
            "stop": "failed",
            "awaiting": null,
            "wanted": null,
        }),
    }
}

/// Project one [`Step`](epoch_engine::Step) for the World.
///
/// Flattened deliberately: a surface receiving this should not have to know the shape of an
/// `Explanation` to render "Robo is reading src/main.rs".
fn step_payload(
    character_id: &str,
    quest_id: &str,
    step: &epoch_engine::Step,
) -> serde_json::Value {
    match step {
        epoch_engine::Step::Using(explanation) => serde_json::json!({
            "characterId": character_id,
            "questId": quest_id,
            "phase": "using",
            "capability": explanation.capability.as_str(),
            "what": explanation.what,
            "risk": explanation.risk.as_str(),
            "reversal": explanation.reversal.describe(),
        }),
        // **It began something that will finish later** (ADR-0034). Its own phase rather than a
        // `used` with a flag: a surface that showed "done" here would be claiming a result that
        // does not exist, which is the failure the tool result itself is worded to prevent.
        epoch_engine::Step::Began {
            capability,
            id,
            what,
            waiting_on,
        } => serde_json::json!({
            "characterId": character_id,
            "questId": quest_id,
            "phase": "began",
            "capability": capability,
            "jobId": id,
            "what": what,
            "waitingOn": waiting_on,
        }),
        // A line printed while it is still going. Deliberately the same event as the other two:
        // a surface following the work wants them interleaved in order, and two channels would
        // make ordering the surface's problem to solve.
        epoch_engine::Step::Said { capability, line } => serde_json::json!({
            "characterId": character_id,
            "questId": quest_id,
            "phase": "said",
            "capability": capability,
            "line": line,
        }),
        epoch_engine::Step::Used {
            capability,
            ok,
            detail,
        } => serde_json::json!({
            "characterId": character_id,
            "questId": quest_id,
            "phase": "used",
            "capability": capability,
            "ok": ok,
            "detail": detail,
        }),
    }
}

/// Say something to a character, and let them answer.
///
/// Returns as soon as the turn has *started*, not when it finishes: a cold local model can take
/// minutes to load, and blocking the IPC thread for that would freeze the window — including the
/// World that is meant to be showing her working.
///
/// The answer arrives as `turn:token` events and one `turn:ended`. Her presence flips to
/// work-class immediately, so the World lights up before the model has said anything — which is
/// the truth: it is loading.
#[tauri::command]
fn speak_to(
    app: tauri::AppHandle,
    world: tauri::State<'_, Arc<World>>,
    character_id: String,
    said: String,
    attachments: Option<Vec<AttachmentInput>>,
    images: Option<Vec<ImageInput>>,
) -> Result<SpeakStarted, String> {
    let said = said.trim();
    let attachments = validate_attachments(attachments)?;
    // Written **before** the turn starts, so a Chronicle entry never references a file that was
    // not saved. The same order the user's words already follow: what was shared is durable
    // before anybody is asked to respond to it.
    let images = keep_images(images)?;
    if said.is_empty() && attachments.is_empty() && images.is_empty() {
        return Err("say something, or attach a file or an image first".into());
    }
    let accepted = attachments.len();
    let pictures = images.len();
    run_turn(
        app,
        world,
        character_id,
        Some(state::UserMessage {
            content: said.to_owned(),
            attachments,
            images,
        }),
    )?;
    Ok(SpeakStarted {
        attachments: accepted,
        images: pictures,
    })
}

/// The durable identity of a live turn. Nothing about a half-written reply belongs in here;
/// the presentation needs only enough to route its transient playback back to the right Quest.
fn started_payload(character_id: &str, quest_id: &str) -> serde_json::Value {
    serde_json::json!({ "characterId": character_id, "questId": quest_id })
}

/// Replace an agent's grown external session with a durable Chronicle brief.
///
/// This is a separate command rather than a message beginning with `/`: a slash typed in the
/// dialogue is still the user's text. Here Epoch can guarantee the summary is read-only,
/// recorded before the session handle is retired, and never appears as a character reply.
#[tauri::command]
fn compact_context(
    app: tauri::AppHandle,
    world: tauri::State<'_, Arc<World>>,
    character_id: String,
) -> Result<(), String> {
    if world.brain_of(&character_id) == Some("agent") {
        let mut prepared = world.prepare_agent_compaction(&character_id)?;
        prepared.attach(world.door_for(&character_id, &app));
        let quest = prepared.quest().to_string();
        let through = prepared
            .compacts_through()
            .expect("a compaction command prepares a compaction turn");
        let world = Arc::clone(&world);

        let _ = app.emit(TURN_STARTED, started_payload(&character_id, &quest));
        let _ = app.emit(
            TURN_COMPACTING,
            serde_json::json!({ "characterId": character_id, "questId": quest, "through": through }),
        );

        std::thread::spawn(move || {
            let outcome = world.run_agent_turn(
                prepared,
                // The brief is a maintenance record, not a spoken answer. It remains auditable in
                // the Chronicle but never streams through the character's dialogue.
                &mut |_| {},
                &mut |_| {},
                &mut |_, _, _| {},
                // A compaction is maintenance. Which model performed it is not a fact about the
                // character's conversation, and reporting it would change the card for a turn
                // the user never asked for.
                &mut |_| {},
                &app,
            );
            if outcome.is_ok() {
                world.forget_context(&quest, &character_id);
                let _ = app.emit(
                    TURN_CONTEXT_CLEARED,
                    serde_json::json!({ "characterId": character_id, "questId": quest }),
                );
            }
            let _ = app.emit(
                TURN_ENDED,
                ended_payload(&character_id, &quest, &outcome, &[]),
            );
            let _ = app.emit(WORLD_CHANGED, world.view());
        });
        return Ok(());
    }

    // Providers do not keep an opaque session that Epoch can reset. They receive a newly
    // composed Chronicle every turn, so their private brief is enough: the next composition
    // sees the durable marker and no longer replays its covered prefix.
    let prepared = world.prepare_turn(&character_id, state::Preparing::Compact)?;
    let quest = prepared.quest().to_string();
    let through = prepared
        .compacts_through()
        .expect("a model compact prepares a compaction turn");
    let world = Arc::clone(&world);
    let _ = app.emit(TURN_STARTED, started_payload(&character_id, &quest));
    let _ = app.emit(
        TURN_COMPACTING,
        serde_json::json!({ "characterId": character_id, "questId": quest, "through": through }),
    );

    std::thread::spawn(move || {
        let outcome = world.run_turn(prepared, &mut |_| {}, &mut |_| {});
        if outcome.is_ok() {
            // Unlike the agent's measured external window, the model's next context is computed
            // locally. Re-read it only after the marker is durable.
            reread_context(&app, &world, &character_id);
        }
        let _ = app.emit(
            TURN_ENDED,
            ended_payload(&character_id, &quest, &outcome, &[]),
        );
        let _ = app.emit(WORLD_CHANGED, world.view());
    });

    Ok(())
}

/// Hand the work to somebody else, and let them answer.
///
/// No new message: the Chronicle already holds whatever was addressed to them, and composing it
/// for them puts it in front of them. This is what makes a crew a crew — Mage says "Paladin,
/// here is the plan", and Paladin *reads it and replies*, rather than Mage writing his line.
///
/// The user's click is the cause. That is the third of the three things allowed to move a Quest
/// (ADR-0025 §7b), and the most explicit of them.
#[tauri::command]
fn hand_over(
    app: tauri::AppHandle,
    world: tauri::State<'_, Arc<World>>,
    character_id: String,
) -> Result<(), String> {
    run_turn(app, world, character_id, None)
}

/// Grant a character a capability they reached for, or refuse.
///
/// Granting writes their own Definition, so it travels with them into every World — what
/// somebody is *for* is not a per-World fact (ADR-0023). Where they may use it is, and that
/// stays the Trust Engine's question: the call is judged afterwards like any other, which is
/// why this may stop again to ask.
#[tauri::command]
fn answer_capability(
    app: tauri::AppHandle,
    world: tauri::State<'_, Arc<World>>,
    character_id: String,
    grant: bool,
) -> Result<(), String> {
    let quest = world.waiting_quest(&character_id)?;
    let world = Arc::clone(&world);
    let _ = app.emit(TURN_STARTED, started_payload(&character_id, &quest));
    std::thread::spawn(move || {
        let outcome = world.answer_capability(
            &character_id,
            grant,
            &mut |token| {
                let _ = app.emit(
                    TURN_TOKEN,
                    serde_json::json!({ "characterId": character_id, "questId": quest, "token": token }),
                );
            },
            &mut |step| {
                let _ = app.emit(TURN_STEP, step_payload(&character_id, &quest, step));
            },
        );
        let _ = app.emit(
            TURN_ENDED,
            ended_payload(&character_id, &quest, &outcome, &[]),
        );
    });
    Ok(())
}

/// Every standing decision, so it can be read back and taken away.
#[tauri::command]
fn list_standing(world: tauri::State<'_, Arc<World>>) -> Vec<state::StandingView> {
    world.standing()
}

/// Take a standing decision back.
#[tauri::command]
fn forget_standing(
    world: tauri::State<'_, Arc<World>>,
    capability: String,
    scope: String,
    who: Option<String>,
) -> Result<(), String> {
    world.forget_standing(&capability, &scope, who.as_deref())
}

/// The most recent change that could be put back, if there is one.
#[tauri::command]
fn get_undoable(world: tauri::State<'_, Arc<World>>) -> Option<state::Undoable> {
    world.undoable()
}

/// Put the last change back. Refuses if the file has moved on since.
#[tauri::command]
fn undo_last(world: tauri::State<'_, Arc<World>>) -> Result<String, String> {
    world.undo_last()
}

/// Where this World works, and what the crew can do here.
#[tauri::command]
fn get_workspace(world: tauri::State<'_, Arc<World>>) -> Option<state::WorkspaceView> {
    world.workspace()
}

/// What this character would be given if they were asked to work right now.
///
/// Read from the same function that composes a turn, so what a surface offers and what the turn
/// allows cannot disagree.
#[tauri::command]
fn get_offered(
    world: tauri::State<'_, Arc<World>>,
    character_id: String,
) -> Option<state::OfferedView> {
    world.offered(&character_id)
}

/// How much rope the open World has, and every mode it could have.
#[tauri::command]
fn get_autonomy(world: tauri::State<'_, Arc<World>>) -> Option<state::AutonomyView> {
    world.autonomy()
}

/// Change how much the crew may do on its own.
#[tauri::command]
fn set_autonomy(world: tauri::State<'_, Arc<World>>, mode: String) -> Result<(), String> {
    world.set_autonomy(&mode)
}

/// Answer a turn that stopped to ask (ADR-0009).
///
/// `always` records a standing decision for this World, so the next one does not stop either.
/// Denying is not a cancellation: the model is told and carries on, which is how it ends up
/// explaining itself rather than leaving half an answer on screen.
#[tauri::command]
fn answer_pending(
    app: tauri::AppHandle,
    world: tauri::State<'_, Arc<World>>,
    character_id: String,
    approve: bool,
    always: bool,
) -> Result<(), String> {
    let quest = world.waiting_quest(&character_id)?;
    let world = Arc::clone(&world);
    let _ = app.emit(TURN_STARTED, started_payload(&character_id, &quest));
    std::thread::spawn(move || {
        let outcome = world.answer_pending(
            &character_id,
            approve,
            always,
            &mut |token| {
                let _ = app.emit(
                    TURN_TOKEN,
                    serde_json::json!({ "characterId": character_id, "questId": quest, "token": token }),
                );
            },
            &mut |step| {
                let _ = app.emit(TURN_STEP, step_payload(&character_id, &quest, step));
            },
        );
        let _ = app.emit(
            TURN_ENDED,
            ended_payload(&character_id, &quest, &outcome, &[]),
        );
    });
    Ok(())
}

/// Send the Context Report to the World.
///
/// Composed by `prepare_turn` rather than by a second function of its own: "what is a character
/// told" already had two authors once, and the fix was to give it one.
///
/// `reserved` rides along because the number the user set and the number the bar shows are not
/// the same number, and the difference is not an error. A character asked for 6,144 and the
/// Composer may spend 5,120 of it; the rest is held back so the reply has somewhere to go.
/// Shown without that, the panel looks like it lost a thousand tokens.
fn emit_knew(
    app: &tauri::AppHandle,
    world: &Arc<World>,
    quest: &str,
    report: &epoch_kernel::Report,
    character_id: &str,
) {
    world.remember_context(
        quest,
        character_id,
        report.used.into(),
        report.budget.into(),
    );
    let _ = app.emit(
        TURN_KNEW,
        serde_json::json!({
            "characterId": character_id,
            "questId": quest,
            "line": report.line(),
            "dropped": report.dropped(),
            "used": report.used,
            "budget": report.budget,
            "reserved": epoch_kernel::Budget::RESERVED_FOR_THE_ANSWER,
        }),
    );
}

/// What the *next* turn would carry, now that this one has been recorded.
///
/// The gauge used to freeze the moment a turn started and stay there until the user typed
/// again — so the answer you were reading, which is often the largest thing in the whole
/// Chronicle, was invisible to the one instrument whose job is to say how full the window is.
/// Recomposed after the fact rather than estimated, because an estimate of the answer's size
/// would be a guess wearing a measurement's clothes.
fn reread_context(app: &tauri::AppHandle, world: &Arc<World>, character_id: &str) {
    // `JustLooking`, not the handover path. Preparing a turn is not free of consequences —
    // it also starts the character working — and calling it to read a number left somebody
    // permanently "thinking with" a model that had already finished and gone.
    if let Ok(next) = world.prepare_turn(character_id, state::Preparing::JustLooking) {
        let quest = next.quest().to_string();
        emit_knew(app, world, &quest, &next.report, character_id);
    }
}

/// Start whoever the user agreed to hand the work to.
///
/// **The Runtime moves the Quest, never the character that asked.** A character proposes through
/// the door, the user answers, and this is where the answer becomes a turn — which is also why
/// it runs *after* the proposing turn has ended rather than from inside it: two people working
/// one Quest at once would have the second reading a Chronicle the first has not finished
/// writing, and the user would see the reply before the request.
///
/// Nothing queued is the ordinary case and costs nothing.
fn carry_on(app: &tauri::AppHandle, world: &Arc<World>) {
    let Some(next) = world.take_handover() else {
        return;
    };
    // The same call the button makes. A handover started by a proposal and one started by the
    // user pressing HAND OVER are the same act, and must not be able to behave differently.
    if let Err(why) = begin_turn(app.clone(), Arc::clone(world), next.to_string(), None) {
        eprintln!("[handover] {why}");
    }
}

/// The same turn, for a brain that works instead of answering.
///
/// Deliberately parallel to the model path rather than folded into it: the events a surface
/// receives are identical — tokens as words arrive, steps as work happens, one ending — so the
/// World cannot tell which brain answered, and should not be able to.
fn run_agent_turn(
    app: tauri::AppHandle,
    world: Arc<World>,
    character_id: String,
    // `None` on a handover: the user moved the work and typed nothing, and the Quest is what
    // the new arrival is given.
    message: Option<state::UserMessage>,
) -> Result<(), String> {
    // Opened *for them*, before the turn, so nobody has to set anything up.
    //
    // This is what the manual steps were: paste a URL, paste a token, name a character. Epoch
    // spawns the process, so Epoch is the one that can hand it the address — and it is also what
    // lets `Manual` work at all, because a permission question has to arrive somewhere.
    let door = world.door_for(&character_id, &app);

    let mut prepared = world.prepare_agent_turn(&character_id, message.as_ref())?;
    prepared.attach(door);
    // Anyone the user named while asking. Offered when the turn ends, exactly as the model path
    // offers it — the World must not be able to tell which brain answered (ADR-0027).
    let invited = prepared.invited.clone();
    // Captured now: the gauge belongs to *this* conversation, and the user may well be looking
    // at another one by the time the answer lands.
    let quest_for_windows = prepared.quest().to_string();
    let world_for_windows = Arc::clone(&world);
    let _ = app.emit(
        TURN_STARTED,
        started_payload(&character_id, &quest_for_windows),
    );

    std::thread::spawn(move || {
        let outcome = world.run_agent_turn(
            prepared,
            &mut |token| {
                let _ = app.emit(
                    TURN_TOKEN,
                    serde_json::json!({ "characterId": character_id, "questId": quest_for_windows, "token": token }),
                );
            },
            &mut |step| {
                let _ = app.emit(TURN_STEP, step_payload(&character_id, &quest_for_windows, step));
            },
            // The same gauge, filled from the agent's own accounting. `line` is empty because
            // Epoch did not compose this context and will not invent a breakdown of somebody
            // else's context: the two numbers are measured, and the rest is honestly absent.
            &mut |used, budget, session| {
                world_for_windows.remember_context(&quest_for_windows, &character_id, used, budget);
                let _ = app.emit(
                    TURN_KNEW,
                    serde_json::json!({
                        "characterId": character_id,
                        "questId": quest_for_windows,
                        // The session, because "did NEW really start a new one?" cannot be
                        // answered from the number: a fresh session starts near 26k, so not
                        // returning to zero looks identical to resuming. The id is the fact.
                        "line": match &session {
                            Some(id) => format!(
                                "Composed by the agent, not by Epoch — the numbers are its own. Session {}.",
                                &id[..id.len().min(8)]
                            ),
                            None => "Composed by the agent, not by Epoch — the numbers are its own.".to_string(),
                        },
                        "dropped": 0,
                        "used": used,
                        "budget": budget,
                        "reserved": 0,
                    }),
                );
            },
            // Read, never declared. `-m` is a request to an agent that routes: a measured run
            // asked for nothing, opened with `"model":"auto"`, and thought with
            // `gemini-3-flash-preview`. Showing the requested name would be the card stating
            // something Epoch was never told.
            &mut |model| {
                let _ = app.emit(
                    TURN_THOUGHT,
                    serde_json::json!({
                        "characterId": character_id,
                        "questId": quest_for_windows,
                        "model": model,
                    }),
                );
            },
            // How an approval reaches the window. Everything else about the decision belongs to
            // the Engine (ADR-0003); this is the one part that is genuinely the shell's.
            &app,
        );
        let _ = app.emit(
            TURN_ENDED,
            ended_payload(&character_id, &quest_for_windows, &outcome, &invited),
        );
        // Whoever the user agreed to hand this to, now that the proposing turn is finished.
        carry_on(&app, &world);
    });

    Ok(())
}

fn run_turn(
    app: tauri::AppHandle,
    world: tauri::State<'_, Arc<World>>,
    character_id: String,
    message: Option<state::UserMessage>,
) -> Result<(), String> {
    begin_turn(app, Arc::clone(&world), character_id, message)
}

/// The same, reachable without a command's `State`.
///
/// Split out because the Runtime starts turns too: a handover the user approved becomes a turn
/// when the proposing one ends, and that happens on a worker thread with an `Arc` rather than
/// inside an IPC call. One body, so a turn the user began and a turn the Runtime began cannot
/// drift apart.
fn begin_turn(
    app: tauri::AppHandle,
    world: Arc<World>,
    character_id: String,
    message: Option<state::UserMessage>,
) -> Result<(), String> {
    // Two brains, two turns, dispatched once at the top (ADR-0027).
    //
    // A model is given a composed conversation and hands back tool calls for Epoch to run; an
    // agent is given an intention and comes back finished. One function with a branch inside
    // would mean every caller downstream asking which half it got.
    if world.brain_of(&character_id) == Some("agent") {
        // A handover carries no new words, and no longer needs any: the Quest itself is what
        // the new arrival is given (`epoch_engine::handover`).
        return run_agent_turn(app, world, character_id, message);
    }

    let prepared = world.prepare_turn(
        &character_id,
        match message.as_ref() {
            Some(message) => state::Preparing::Said(message),
            None => state::Preparing::HandingOver,
        },
    )?;
    // Anyone the user named. Carried to the end of the turn and offered there, never acted on:
    // bringing a colleague in loads a second model, and that memory is the user's to spend.
    let invited = prepared.invited.clone();
    // What she was told this turn, and what she was not (ADR-0012). Sent before the turn runs
    // rather than after: an answer that seems to ignore something is explainable only if you
    // can see whether it was there, and by the time the answer arrives you want it already.
    let quest_for_context = prepared.quest().to_string();
    emit_knew(
        &app,
        &world,
        &quest_for_context,
        &prepared.report,
        &character_id,
    );
    let _ = app.emit(
        TURN_STARTED,
        started_payload(&character_id, &quest_for_context),
    );

    /*
        **The same fact, said twice on purpose, and they are not the same channel.**

        `TURN_STARTED` is a Tauri event for one window: a Command's response reaching the
        surface that asked. The Activity is an observation for *whoever is listening* — a live
        readout today, Knowledge and Automation later — and neither may be built out of the
        other. Emitting where the lock is not held, because emission is synchronous and a
        subscriber must never be able to hold what a turn needs.

        Coarse, and it carries ids rather than the message. The Chronicle has what was said.
    */
    let began = epoch_kernel::Activity::new(
        epoch_kernel::ActivityKind::TurnStarted,
        epoch_engine::now_ms(),
        format!("{character_id} began a turn"),
    )
    .about(
        epoch_kernel::Subject::default()
            .with_quest(&quest_for_context)
            .with_character(&character_id),
    );
    let turn_id = began.id.clone();
    world.happened(began);

    std::thread::spawn(move || {
        let outcome = world.run_turn(
            prepared,
            &mut |token| {
                let _ = app.emit(
                    TURN_TOKEN,
                    serde_json::json!({ "characterId": character_id, "questId": quest_for_context, "token": token }),
                );
            },
            // Work, as it happens. A separate channel from the tokens on purpose: tokens are
            // what a character is *saying*, these are what a character is *doing*, and the
            // World renders the two differently (ADR-0018).
            &mut |step| {
                let _ = app.emit(TURN_STEP, step_payload(&character_id, &quest_for_context, step));
            },
        );

        let _ = app.emit(
            TURN_ENDED,
            ended_payload(&character_id, &quest_for_context, &outcome, &invited),
        );
        // Part of the same piece of work, which is what correlation is for: a consumer can group
        // a turn from the stream alone, without asking any repository anything.
        world.happened(
            epoch_kernel::Activity::new(
                epoch_kernel::ActivityKind::ProviderRequestCompleted,
                epoch_engine::now_ms(),
                match &outcome {
                    Ok(_) => format!("{character_id} finished"),
                    Err(why) => format!("{character_id} could not: {why}"),
                },
            )
            .about(
                epoch_kernel::Subject::default()
                    .with_quest(&quest_for_context)
                    .with_character(&character_id),
            )
            .part_of(&turn_id)
            // A turn that failed is the thing somebody is looking for when they open this.
            .at_importance(match &outcome {
                Ok(_) => epoch_kernel::Importance::Normal,
                Err(_) => epoch_kernel::Importance::Critical,
            }),
        );
        // After `TURN_ENDED`, so the answer it is measuring is already in the Chronicle.
        reread_context(&app, &world, &character_id);
        // **Not because a model can propose one today.** The door is opened for agents, and a
        // model's tools go through the capability registry rather than through MCP — so nothing
        // can queue a handover during a model's turn.
        //
        // It is here so that the queue belongs to the Runtime rather than to one of the two
        // paths. The day a model reaches the door, this is already right; and a queue that only
        // one ending drained would strand an approved handover the first time the order of
        // turns was not what somebody assumed.
        carry_on(&app, &world);

        // She has stopped working; publish that without waiting for the next tick.
        let _ = app.emit(WORLD_CHANGED, world.view());
    });

    Ok(())
}

/// The open World's active Quest.
///
/// One Quest at a time, and everyone contributes to it. Talking to a different specialist does
/// not start a different thread — that is the whole point of ADR-0025.
#[tauri::command]
fn active_quest(world: tauri::State<'_, Arc<World>>) -> Option<state::QuestSummary> {
    world.active_quest()
}

/// Make a character. Returns their id, so the surface opens the editor on somebody real.
#[tauri::command]
fn hire_character(
    world: tauri::State<'_, Arc<World>>,
    name: String,
    archetype: String,
    role: String,
    prompt: String,
) -> Result<String, String> {
    world.hire_character(&name, &archetype, &role, &prompt)
}

/// Words an archetype offers somebody who is writing a character.
///
/// Offered, never applied: the difference between help and imposition is who performed the act,
/// so this returns the suggestion and the surface only writes it if the user keeps it.
#[tauri::command]
fn suggested_prompt(archetype: String) -> String {
    epoch_kernel::CharacterArchetype::from_id(&archetype)
        .map(epoch_engine::hiring::suggested_prompt)
        .unwrap_or_default()
        .to_owned()
}

/// Every conversation in this World — open and ended alike.
#[tauri::command]
fn list_conversations(world: tauri::State<'_, Arc<World>>) -> Vec<state::QuestSummary> {
    world.conversations()
}

/// Look at a different conversation. Its Chronicle, and its agent session, come back with it.
#[tauri::command]
fn select_quest(world: tauri::State<'_, Arc<World>>, quest: String) -> Result<(), String> {
    world.select_quest(&quest)
}

/// End one. It stays in History as what it was — closing is not deleting (ADR-0025 §7).
///
/// Answers with where its note was written, when it produced something worth writing down.
/// `None` is the ordinary answer: most conversations are conversations, and a Quest that
/// produced no evidence produced nothing (ADR-0010).
#[tauri::command]
fn close_quest(
    world: tauri::State<'_, Arc<World>>,
    quest: String,
) -> Result<Option<String>, String> {
    world.close_quest(&quest)
}

/// Put the current Quest down, so the next thing said is new work.
#[tauri::command]
fn set_quest_aside(world: tauri::State<'_, Arc<World>>) {
    world.set_quest_aside();
}

/// Rename the active Quest. Its identity never changes — only what it is called.
#[tauri::command]
fn rename_quest(world: tauri::State<'_, Arc<World>>, title: String) -> Result<(), String> {
    world.rename_quest(&title)
}

/// The user's choices about their own machine.
#[tauri::command]
fn get_settings(world: tauri::State<'_, Arc<World>>) -> epoch_engine::Settings {
    world.settings()
}

/// Change **one** setting, named.
///
/// ## Why this is not `save_settings(Settings)` any more
///
/// It was, and the Launcher's checkbox sent `{ concurrentCrew }` — one field of five. Every
/// field on `Settings` carries `#[serde(default)]`, correctly, so that partial deserialised into
/// a complete `Settings` with **no agent accounts and no runtime preferences**, and the write
/// put that on disk. Toggling one checkbox deleted a second Claude Code sign-in and a chosen GPU
/// backend, silently, with the deck showing exactly what the user had just asked for.
///
/// Not hypothetical: `settings.rs` holds the test that spells out what that payload really
/// means. And it is the third time this codebase has paid for a whole-object write from a form
/// that manages part of the object — `draws_in: None`, then two runtime controls in one gesture,
/// now this.
///
/// **A surface can only send what it names.** `change_settings` holds the lock across the read,
/// the change and the write, so two controls pressed together cannot lose one of them either.
#[tauri::command]
fn set_concurrent_crew(world: tauri::State<'_, Arc<World>>, on: bool) -> Result<(), String> {
    world.change_settings(|settings| settings.concurrent_crew = on)
}

/// How many rounds of tools one turn may take. `0` is no limit.
///
/// Named, like every other setting: a surface may only send what it names.
#[tauri::command]
fn set_tool_rounds(world: tauri::State<'_, Arc<World>>, rounds: usize) -> Result<(), String> {
    world.change_settings(|settings| settings.tool_rounds = Some(rounds))
}

/// Remember which language the person at this machine speaks.
///
/// An empty string is *detect each time*, which is a real choice and the default. Named, so a
/// surface can only send what it names — see `set_concurrent_crew`.
#[tauri::command]
fn set_hearing_language(
    world: tauri::State<'_, Arc<World>>,
    language: String,
) -> Result<(), String> {
    let said = language.trim().to_owned();
    world.change_settings(|settings| {
        settings.hearing_language = if said.is_empty() { None } else { Some(said) };
    })
}

/// Remember whether the World may listen through this machine's microphone.
///
/// The answer to a question **Epoch asked**, in its own frame, in its own words — never a guess.
/// `microphone::answer_for` reads this to answer WebView2, and with nothing stored it answers
/// nothing at all.
#[tauri::command]
fn set_microphone(world: tauri::State<'_, Arc<World>>, allowed: bool) -> Result<(), String> {
    world.change_settings(|settings| settings.microphone = Some(allowed))
}

/// What every Provider can currently do.
///
/// Always answers. A Provider that is not reachable is the ordinary state, not an error, and it
/// reports *why* along with where we looked — so "OFFLINE" is a fact the user can go and check.
/*
    **Off the main thread, because this one is measured in seconds.**

    A `#[tauri::command]` declared `fn` runs on the thread that owns the window, so anything slow
    in it is a frozen interface. Measured 2026-08-21, on the path of opening a single deck:

    ```text
    registry.survey (network)   4.18 s     every backend, local and paired
    runtimes::survey              624 ms   installer directories + a TCP probe each
    Backends::load                623 ms   because it surveys
    Machine::measure              269 ms   nvidia-smi
    ```

    Four seconds of somebody else's Wi-Fi, with the window unable to repaint. `async` alone would
    not fix it — the work is blocking, so it would block whichever runtime thread it landed on —
    so it goes to `spawn_blocking`, which is the pool that exists for exactly this.

    The surfaces already say *PROBING…* and *ASKING…* while they wait. They could not draw it.
*/
#[tauri::command]
async fn list_providers(
    world: tauri::State<'_, Arc<World>>,
) -> Result<Vec<epoch_engine::ProviderStatus>, String> {
    let world = Arc::clone(&world);
    // A join failure is the measuring thread itself going wrong, which is rare and worth
    // saying: returning an empty list for it would be an invented reading.
    tauri::async_runtime::spawn_blocking(move || world.providers())
        .await
        .map_err(|why| format!("the measurement did not finish: {why}"))
}

/// Hand a link to the user's browser.
///
/// ## Why this checks the scheme
///
/// The links this opens are **sources**, and a source is a web address chosen by a search
/// engine out of pages written by strangers. Handing an arbitrary string to the operating
/// system's "open this" is handing a stranger the ability to pick which program starts:
/// `file://` opens a file manager, and a registered custom scheme can be anything installed.
///
/// So only `http` and `https` — the two that mean "a web page", which is the only thing a
/// source ever is. Anything else is refused with the address named, rather than quietly
/// ignored, because a link that does nothing when clicked is a bug the user cannot report.
///
/// Same bargain as the folder picker (ADR-0024): no JavaScript API is added to the webview.
/// The frontend cannot open anything; it asks the shell, and the shell checks first.
#[tauri::command]
fn open_link(url: String) -> Result<(), String> {
    let trimmed = url.trim();
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err(format!(
            "only web addresses can be opened, and this is not one: {trimmed}"
        ));
    }
    opener::open(trimmed).map_err(|err| format!("could not open {trimmed}: {err}"))
}

/// Every backend this machine has, configured or defaulted.
///
/// Infrastructure, so one list serves every World (ADR-0026). Separate from `list_providers`,
/// which *probes* — this says what is configured, that says what answered.
///
/// Off the window thread: `Backends::load` reads the file *and* surveys the local runtimes to
/// fill in what is serving here, which was 634 ms cold before it was cached.
#[tauri::command]
async fn list_backends(
    world: tauri::State<'_, Arc<World>>,
) -> Result<epoch_engine::backends::Backends, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.backends())
        .await
        .map_err(|err| err.to_string())
}

/// Add or change one. Rebuilds what can think, so the next turn uses it.
#[tauri::command]
fn save_backend(
    world: tauri::State<'_, Arc<World>>,
    backend: epoch_engine::backends::Backend,
) -> Result<(), String> {
    world.save_backend(backend)
}

/// Forget one. Characters assigned to it keep their assignment.
#[tauri::command]
fn forget_backend(world: tauri::State<'_, Arc<World>>, id: String) -> Result<(), String> {
    world.forget_backend(&id)
}

/// Ask the running turn to stop at its next seam.
///
/// Never fails and never waits: it sets a flag the turn reads between rounds. Whatever is in
/// flight finishes, and nothing new begins. Saying "stopping" is honest; saying "stopped" would
/// not be, because a model call already under way is still under way.
#[tauri::command]
fn stop_turn(world: tauri::State<'_, Arc<World>>) {
    world.stop();
}

/// Every configured MCP server. Reads the file; starts nothing.
///
/// A projection, not the type — `Configured` omits its empty fields so `mcp.toml` stays readable,
/// and a wire where a missing key means "empty" is a contract nothing can hold.
#[tauri::command]
fn list_mcp(world: tauri::State<'_, Arc<World>>) -> epoch_engine::mcp::View {
    world.mcp().view()
}

/// What the outside tools currently amount to, and what could not be offered.
///
/// Separate from `list_mcp` because this one **starts the servers** to ask them. A surface that
/// listed the configuration and probed it in one call would spawn processes just by being opened.
#[tauri::command]
fn probe_mcp(world: tauri::State<'_, Arc<World>>) -> (Vec<String>, Vec<String>) {
    world.mcp_tools()
}

/// Ask every server again, and write down what they say.
///
/// `probe_mcp` answers from what they said last time, so a restart costs nothing and the tools
/// are there before anybody looks. This is the explicit way to pick up a server whose tools
/// genuinely changed — needed because this build does not yet listen for
/// `notifications/tools/list_changed`.
#[tauri::command]
fn refresh_mcp(world: tauri::State<'_, Arc<World>>) -> (Vec<String>, Vec<String>) {
    world.refresh_mcp()
}

#[tauri::command]
fn save_mcp(
    world: tauri::State<'_, Arc<World>>,
    server: epoch_engine::mcp::Configured,
) -> Result<(), String> {
    world.save_mcp(server)
}

/// Give a configured server a credential. The value goes in; nothing reads it back out.
#[tauri::command]
fn save_mcp_secret(
    world: tauri::State<'_, Arc<World>>,
    server: String,
    name: String,
    value: String,
) -> Result<(), String> {
    world.save_mcp_secret(&server, &name, &value)
}

#[tauri::command]
fn forget_mcp(world: tauri::State<'_, Arc<World>>, id: String) -> Result<(), String> {
    world.forget_mcp(&id)
}

/// Which shells this machine has.
///
/// These commands are the **user's** terminal, and deliberately not a capability: nothing a
/// model can reach opens one, types into one, or reads one. `run_command` exists so that a
/// model's shell access is bounded, explained and approved; a terminal would be the way around
/// it. See `epoch_engine::terminal`.
#[tauri::command]
fn terminal_shells(world: tauri::State<'_, Arc<World>>) -> Vec<epoch_engine::terminal::Shell> {
    world.terminal_shells()
}

#[tauri::command]
fn terminal_open(
    app: tauri::AppHandle,
    world: tauri::State<'_, Arc<World>>,
    id: String,
    shell: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    world.terminal_open(app, id, &shell, cols, rows)
}

/// Keystrokes, verbatim — control characters included, because a prompt expects them.
#[tauri::command]
fn terminal_write(
    world: tauri::State<'_, Arc<World>>,
    id: String,
    keys: String,
) -> Result<(), String> {
    world.terminal_write(&id, &keys)
}

#[tauri::command]
fn terminal_resize(
    world: tauri::State<'_, Arc<World>>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    world.terminal_resize(&id, cols, rows)
}

/// What it printed before this window was looking, so reopening one is not a blank square.
#[tauri::command]
fn terminal_scrollback(world: tauri::State<'_, Arc<World>>, id: String) -> Option<String> {
    world.terminal_scrollback(&id)
}

/// End it, and the program with it. **Minimising is not this.**
#[tauri::command]
fn terminal_close(world: tauri::State<'_, Arc<World>>, id: String) -> Result<(), String> {
    world.terminal_close(&id)
}

/// Which are still running. A surface that remounts rebuilds its windows from this rather than
/// from its own memory of what it opened.
#[tauri::command]
fn terminal_open_ids(world: tauri::State<'_, Arc<World>>) -> Vec<String> {
    world.terminal_open_ids()
}

/// Browse the MCP catalogue.
///
/// Reaches the network, and answers from the cache when it cannot. Never spawns a server: the
/// runners are asked for their version, which is a different thing from starting somebody's MCP
/// server to see what it does.
#[tauri::command]
fn workshop_search(
    world: tauri::State<'_, Arc<World>>,
    query: String,
    cursor: Option<String>,
) -> epoch_engine::workshop::Shelf {
    world.workshop_search(&query, cursor.as_deref())
}

/// Install a catalogue entry.
///
/// The listing and the offer come back from the surface unchanged rather than being re-fetched
/// by name: what the user approved is the command they were shown, and re-fetching would install
/// whatever the catalogue says *now* — which is a different program from the one on screen.
#[tauri::command]
fn workshop_install(
    world: tauri::State<'_, Arc<World>>,
    listing: epoch_engine::workshop::Listing,
    offer: epoch_engine::workshop::Offer,
    answers: epoch_engine::workshop::Answers,
) -> Result<String, String> {
    world.workshop_install(listing, offer, answers)
}

/// Store or clear a backend's credential.
///
/// One-way on purpose: there is no command that reads a key back. The interface can be told
/// *that* one is stored and never *what* it is, which is the same rule as the type — a value
/// that cannot be serialised cannot be returned from a command either.
///
/// An empty string clears it, because that is what clearing the field means.
#[tauri::command]
fn save_backend_key(
    world: tauri::State<'_, Arc<World>>,
    id: String,
    key: String,
) -> Result<(), String> {
    world.save_backend_key(&id, &key)
}

/// What one Provider says about one model: how much it holds, and what it lets you tune.
///
/// The interface draws the knobs without knowing which backend asked for them — a frontend that
/// knew Ollama has `num_ctx` would need editing to add a Provider (ADR-0003, ADR-0026). The
/// window is separate because it is canonical: every backend has one and it means the same
/// thing everywhere, which is what makes it the ceiling on `context_tokens`.
#[tauri::command]
fn get_surface(
    world: tauri::State<'_, Arc<World>>,
    provider: String,
    model: String,
) -> epoch_engine::Surface {
    world.surface(&provider, &model)
}

/// The open World's backdrop, as a `data:` URI. `None` when it has none.
///
/// Deliberately not part of `WorldView`: that projection is re-sent on every presence change,
/// and a full illustration must not ride along with it. Asked for once, when the World mounts.
#[tauri::command]
fn get_backdrop(world: tauri::State<'_, Arc<World>>) -> Option<String> {
    world.backdrop()
}

/// Give a World key art, or clear it back to the chart derived from its geography.
///
/// The image arrives base64-encoded from the webview's own `<input type="file">`. That is
/// deliberate: the frontend never gains filesystem access in order to offer a file, and the
/// Engine chooses the destination name from the image's real format rather than trusting one.
#[tauri::command]
fn set_world_art(
    world: tauri::State<'_, Arc<World>>,
    id: String,
    image: Option<String>,
) -> Result<(), String> {
    world.set_world_art(&id, image.as_deref())
}

/// Give a World its own windows, or take them back to the ones Epoch draws.
///
/// The image arrives base64-encoded from the webview's own `<input type="file">`, like every
/// other import (ADR-0024): the frontend never gains filesystem access, and the Engine names the
/// file from its bytes.
///
/// **The corner inset is the author's, and has to be.** It cannot be read from the pixels — the
/// default pack's `[11, 5, 12, 6]` was measured off the artwork, because a bevel lit from above
/// is not symmetric. The surface draws the result while they choose it, which is what makes a
/// number nobody could pick in the abstract obvious against a picture.
#[tauri::command]
fn set_world_skin(
    world: tauri::State<'_, Arc<World>>,
    concept: String,
    image: Option<String>,
    corner: [u32; 4],
    repeat: String,
    scale: u32,
) -> Result<(), String> {
    world.set_world_skin(&concept, image.as_deref(), corner, &repeat, scale)
}

/// Change what the bridge calls the orchestrator.
#[tauri::command]
fn rename_orchestrator(world: tauri::State<'_, Arc<World>>, name: String) -> Result<(), String> {
    world.rename_orchestrator(&name)
}

/// Give the orchestrator a portrait, or clear it.
#[tauri::command]
fn set_orchestrator_portrait(
    world: tauri::State<'_, Arc<World>>,
    image: Option<String>,
) -> Result<(), String> {
    world.set_orchestrator_portrait(image.as_deref())
}

/// Ask the user to pick a folder, natively.
///
/// Opened from **Rust**, never from the webview. `<input webkitdirectory>` hands over files and
/// no path — browsers strip it deliberately — so a folder picker cannot come from the
/// presentation layer at all. This keeps ADR-0024's rule intact and arguably strengthens it:
/// the frontend still has no filesystem access, it asks the shell to ask the user.
///
/// Returns `None` when the dialog is dismissed. Cancelling is an answer, not an error.
#[tauri::command]
fn choose_folder(start: Option<String>) -> Option<String> {
    let mut dialog = rfd::FileDialog::new().set_title("Choose the folder this World works in");
    // Reopen where they last chose, when that folder is still there.
    if let Some(start) = start.filter(|s| !s.trim().is_empty()) {
        let path = std::path::PathBuf::from(start);
        if path.is_dir() {
            dialog = dialog.set_directory(path);
        }
    }
    dialog.pick_folder().map(|p| p.display().to_string())
}

/// Point a World at the folder it works in, or clear it.
#[tauri::command]
fn set_project_root(
    world: tauri::State<'_, Arc<World>>,
    id: String,
    path: Option<String>,
) -> Result<(), String> {
    world.set_project_root(&id, path.as_deref())
}

/// Choose the folder a World reads its notes from.
///
/// Its own dialog rather than `choose_folder` with a different title: the title is the whole
/// explanation of what the folder is for, and one picker saying "the folder this World works
/// in" for both would be the surface telling the user the wrong thing about their vault.
#[tauri::command]
fn choose_library(start: Option<String>) -> Option<String> {
    let mut dialog = rfd::FileDialog::new().set_title(
        "Choose this World's library of notes — an Obsidian vault, or any folder of markdown",
    );
    if let Some(start) = start.filter(|s| !s.trim().is_empty()) {
        let path = std::path::PathBuf::from(start);
        if path.is_dir() {
            dialog = dialog.set_directory(path);
        }
    }
    dialog.pick_folder().map(|p| p.display().to_string())
}

/// Point a World at the library it reads from, or clear it.
#[tauri::command]
fn set_library(
    world: tauri::State<'_, Arc<World>>,
    id: String,
    path: Option<String>,
) -> Result<(), String> {
    world.set_library(&id, path.as_deref())
}

/// Open a World's library — in Obsidian when it is installed, in the file manager otherwise.
///
/// Returns whether Obsidian took it, so the surface can say which happened instead of leaving
/// the user to wonder why a different program opened.
#[tauri::command]
fn open_library(world: tauri::State<'_, Arc<World>>, id: Option<String>) -> Result<bool, String> {
    world.open_library(id.as_deref())
}

/// Look at a folder before a World reads from it, and report what is there.
#[tauri::command]
fn scan_library(
    world: tauri::State<'_, Arc<World>>,
    path: String,
) -> Result<epoch_engine::library::LibraryScan, String> {
    world.scan_library(&path)
}

/// Give a character one of their drawings, or clear it.
///
/// `slot` is `"sprite"`, `"icon"`, or an action id (`"walk"`, `"idle"`, …). `cut` says how to
/// read a sheet and belongs to action slots only — the still picture and the face are pictures.
#[tauri::command]
fn set_character_art(
    world: tauri::State<'_, Arc<World>>,
    character_id: String,
    slot: String,
    image: Option<String>,
    cut: Option<epoch_kernel::Sheet>,
) -> Result<(), String> {
    world.set_character_art(&character_id, &slot, image.as_deref(), cut)
}

/// Change how an action's sheet is cut, without re-importing it.
#[tauri::command]
fn set_character_cut(
    world: tauri::State<'_, Arc<World>>,
    character_id: String,
    action: String,
    cut: epoch_kernel::Sheet,
) -> Result<(), String> {
    world.set_character_cut(&character_id, &action, cut)
}

/// What removing a character would do — asked **before** anything is deleted.
///
/// Its own command rather than a flag on the delete: the user sees this, accepts it, and only
/// then is anything removed. A confirmation built from a guess would be a confirmation about
/// something else.
#[tauri::command]
fn character_removal(
    world: tauri::State<'_, Arc<World>>,
    character_id: String,
) -> Result<crate::state::RemovalView, String> {
    world.character_removal(&character_id)
}

/// Remove a character, after the user accepted what it does.
///
/// Returns whatever could not be removed. An empty list is a clean removal.
#[tauri::command]
fn remove_character(
    world: tauri::State<'_, Arc<World>>,
    character_id: String,
) -> Result<Vec<String>, String> {
    world.remove_character(&character_id)
}

/// Every machine paired with this Host, and what each may be.
///
/// Off the window thread: one short network probe per paired machine, on somebody else's Wi-Fi.
#[tauri::command]
async fn bridges(world: tauri::State<'_, Arc<World>>) -> Result<Vec<epoch_engine::Paired>, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.bridges())
        .await
        .map_err(|err| err.to_string())
}

/// Reach out to a machine that is showing a code.
///
/// The other direction, for a network that will not let the remote start a connection. The bond
/// is identical; only who opens the socket differs.
#[tauri::command]
fn enrol_machine(
    world: tauri::State<'_, Arc<World>>,
    address: String,
    code: String,
    grants: Vec<String>,
) -> Result<(), String> {
    world.enrol_machine(&address, &code, &grants).map(|_| ())
}

/// Finish a pairing. The code is spent whether or not it matched.
#[tauri::command]
fn pair(
    world: tauri::State<'_, Arc<World>>,
    code: String,
    name: String,
    address: String,
    // What that machine said its certificate is. Absent is refused rather than defaulted.
    fingerprint: Option<String>,
    grants: Vec<String>,
) -> Result<String, String> {
    world.pair(&code, &name, &address, fingerprint, &grants)
}

/// Change what a paired machine may be, without unpairing it.
#[tauri::command]
fn set_bridge_grants(
    world: tauri::State<'_, Arc<World>>,
    id: String,
    grants: Vec<String>,
) -> Result<(), String> {
    world.set_bridge_grants(&id, &grants)
}

/// Unpair a machine — the roster and its secret, together.
#[tauri::command]
fn unpair(world: tauri::State<'_, Arc<World>>, id: String) -> Result<(), String> {
    world.unpair(&id)
}

/// What has been answered about turns leaving this machine, and what has not.
#[tauri::command]
fn disclosures(world: tauri::State<'_, Arc<World>>) -> Vec<serde_json::Value> {
    world
        .disclosures()
        .into_iter()
        .map(|(going, allowed, asks)| {
            serde_json::json!({ "going": going, "allowed": allowed, "asks": asks })
        })
        .collect()
}

/// Answer the disclosure question for one destination, or take the answer back.
#[tauri::command]
fn answer_disclosure(
    world: tauri::State<'_, Arc<World>>,
    going: String,
    allowed: Option<bool>,
) -> Result<(), String> {
    world.answer_disclosure(&going, allowed)
}

/// What this machine has, what it does not, and what to do about each.
///
/// The Engine has measured this since 2026-08-16 and nothing showed it. Look first, and teach
/// only where looking found nothing (`readiness.rs`).
///
/// **Off the window thread, because this one probes.** It calls `providers()`, which is the
/// 4.18 s network survey measured on 2026-08-21 — every backend, local and paired. Moving the
/// four commands the Connections deck opens with was half the fix; this is the other half, and
/// it is the one that was still freezing the deck.
#[tauri::command]
async fn readiness(
    world: tauri::State<'_, Arc<World>>,
) -> Result<Vec<epoch_engine::readiness::Row>, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || {
        let agents = world.agents(false);
        let providers = world.providers();
        let mut rows = epoch_engine::readiness::survey(&agents, &providers);
        rows.extend(epoch_engine::readiness::addable_after_looking(&|name| {
            std::env::var_os(name).is_some()
        }));
        rows
    })
    .await
    .map_err(|err| err.to_string())
}

/// What this machine is — its card, its memory. Measured, or absent.
#[tauri::command]
async fn this_machine(
    world: tauri::State<'_, Arc<World>>,
) -> Result<epoch_engine::Machine, String> {
    let world = Arc::clone(&world);
    // A join failure is the measuring thread itself going wrong, which is rare and worth
    // saying: returning an empty list for it would be an invented reading.
    tauri::async_runtime::spawn_blocking(move || world.machine())
        .await
        .map_err(|why| format!("the measurement did not finish: {why}"))
}

/// What Ollama is featuring, with what this machine already has marked.
#[tauri::command]
fn featured_models(
    world: tauri::State<'_, Arc<World>>,
) -> Result<Vec<epoch_engine::Offer>, String> {
    world.featured_models()
}

/// Search Hugging Face for models this machine could pull.
#[tauri::command]
fn search_models(
    world: tauri::State<'_, Arc<World>>,
    query: String,
    facets: Vec<String>,
    more: Option<String>,
) -> Result<SearchAnswer, String> {
    let found = world.search_models(&query, &facets, more.as_deref())?;
    Ok(SearchAnswer {
        offers: found.offers,
        more: found.more,
    })
}

/// One page of search results, and the cursor for the next.
///
/// A named type rather than a tuple because both halves are shown: the results, and whether a
/// MORE button should exist at all.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchAnswer {
    offers: Vec<epoch_engine::Offer>,
    more: Option<String>,
}

/// Download a model into the local runtime.
///
/// Runs on its own thread and reports through events, because it takes minutes and the IPC
/// thread is what the window draws on. The runtime's own words go out as they arrive — a real
/// account of what is happening beats a spinner, and it is the only account that is true.
#[tauri::command]
fn pull_model(
    app: tauri::AppHandle,
    world: tauri::State<'_, Arc<World>>,
    model: String,
) -> Result<(), String> {
    let world = Arc::clone(&world);
    std::thread::spawn(move || {
        let named = model.clone();
        let outcome = world.pull_model(&model, &mut |progress| {
            let _ = app.emit(
                PULL_PROGRESS,
                serde_json::json!({ "model": named, "progress": progress }),
            );
        });
        let _ = app.emit(
            PULL_DONE,
            serde_json::json!({
                "model": model,
                "failure": outcome.err(),
            }),
        );
    });
    Ok(())
}

/// The runtime's own account of a download, line by line.
const PULL_PROGRESS: &str = "models:pulling";
/// Finished, one way or the other. `failure` is `null` when it worked.
const PULL_DONE: &str = "models:pulled";

/// Give a character a different brain, because the assigned one could not be reached.
///
/// **The user's answer to a question, never a failover.** The Runtime does not choose a Brain
/// (ADR-0026): it says the assigned one is unreachable and offers the choice. This is the
/// choice being taken, and the Chronicle records that a person took it.
#[tauri::command]
fn reassign_brain(
    world: tauri::State<'_, Arc<World>>,
    character_id: String,
    backend: String,
    model: String,
    because: String,
) -> Result<(), String> {
    world.reassign_brain(&character_id, &backend, &model, &because)
}

/// The other runtimes on this machine: llama.cpp and LM Studio.
///
/// Both speak an OpenAI-compatible API, which Epoch has spoken since Phase 8 — so this measures
/// and nothing more. Adding one as a Service is the ordinary backend form.
#[tauri::command]
async fn local_runtimes(
    world: tauri::State<'_, Arc<World>>,
) -> Result<Vec<epoch_engine::models::runtimes::Available>, String> {
    let world = Arc::clone(&world);
    // A join failure is the measuring thread itself going wrong, which is rare and worth
    // saying: returning an empty list for it would be an invented reading.
    tauri::async_runtime::spawn_blocking(move || world.local_runtimes())
        .await
        .map_err(|why| format!("the measurement did not finish: {why}"))
}

/// Every model Ollama already has on disk, as files another runtime can open.
#[tauri::command]
fn shared_weights(
    world: tauri::State<'_, Arc<World>>,
) -> Vec<epoch_engine::models::runtimes::Weights> {
    world.shared_weights()
}

/// What is on the card for one character, and what the user asked for.
///
/// Off the UI thread: it asks a server, and the lamp it feeds is polled while somebody is
/// typing. A local GET is fast and a local GET to a server that is loading a 12 GB model is
/// not, and the window may not be the thing that waits.
#[tauri::command]
async fn warmth(
    world: tauri::State<'_, Arc<World>>,
    character: String,
) -> Result<state::Warmth, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.warmth(&character))
        .await
        .map_err(|why| format!("could not read the card: {why}"))?
}

/// Hold this character's brain on the card, or let it go now.
#[tauri::command]
async fn hold_warm(
    world: tauri::State<'_, Arc<World>>,
    character: String,
    keep: bool,
) -> Result<state::Warmth, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.hold_warm(&character, keep))
        .await
        .map_err(|why| format!("could not change what is held: {why}"))?
}

/// Every model here, with what is known about how it runs. One read, no probing.
///
/// **Off the main thread, because a read is not free.** A synchronous `#[tauri::command]` runs on
/// the thread that pumps the window's messages, so for as long as it takes, Windows paints
/// *(not responding)* over Epoch. Measured on this machine through the real window, three runs:
/// **26.5 s, 26.6 s, 27.2 s** — steady, so not a cold-disk artefact — of which 20.9 s is reading
/// every GGUF header on an 80 GB shelf and 9.1 s is reading them a second time.
///
/// It says *no probing* and that is still true; what it never said is *no work*. `who_can_time`
/// and `who_can_measure` were given `spawn_blocking` because they probe, and the deck's own read
/// was left synchronous because it does not — which reasons about **why** the work happens
/// rather than about **where** it runs. The main thread does not care why.
#[tauri::command]
async fn models_and_loadouts(
    world: tauri::State<'_, Arc<World>>,
) -> Result<Vec<state::ModelHere>, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.models_and_loadouts())
        .await
        .map_err(|why| why.to_string())
}

/// Map what one model can do on this card. Minutes, and it is asked for.
#[tauri::command]
async fn search_loadout(
    app: tauri::AppHandle,
    world: tauri::State<'_, Arc<World>>,
    path: String,
    on: String,
) -> Result<String, String> {
    // Off the UI thread: eight loads is four minutes, and a window that stops answering for four
    // minutes is a window somebody force-quits.
    let world = Arc::clone(&world);
    let watching = path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        world.search_loadout(&path, &on, &|done, total| {
            // Which row, and how far. The path identifies it because the deck is keyed by path:
            // two shelves can hold the same name, and only one of them is being measured.
            let _ = app.emit(
                MEASURING,
                serde_json::json!({ "path": watching, "done": done, "total": total }),
            );
        })
    })
    .await
    .map_err(|why| format!("the search did not finish: {why}"))?
}

/// Take the user's pick out of a measured curve.
#[tauri::command]
fn choose_loadout(
    world: tauri::State<'_, Arc<World>>,
    path: String,
    context: u32,
) -> Result<String, String> {
    world.choose_loadout(&path, context)
}

/// Take one model off this machine, blobs, shelf links and all.
#[tauri::command]
fn remove_model(world: tauri::State<'_, Arc<World>>, path: String) -> Result<String, String> {
    world.remove_model(&path)
}

/// Give Ollama a GGUF it did not download, in the user's own terminal.
#[tauri::command]
fn import_weights(world: tauri::State<'_, Arc<World>>, path: String) -> Result<(), String> {
    world.import_weights(&path)
}

/// What exporting one World would carry, before it is carried.
#[tauri::command]
fn world_export(
    world: tauri::State<'_, Arc<World>>,
    world_id: String,
) -> Result<crate::state::ExportView, String> {
    world.world_export(&world_id)
}

/// Write one World out as an archive, wherever the user says, and report where it landed.
///
/// `None` is a cancelled dialog. Cancelling is an answer, not an error — the same rule the
/// project-root picker follows.
#[tauri::command]
fn export_world(
    world: tauri::State<'_, Arc<World>>,
    world_id: String,
) -> Result<Option<String>, String> {
    world.export_world(&world_id)
}

/// Search Ollama's own shelf — the curated names between its front page and Hugging Face.
#[tauri::command]
fn search_ollama(
    world: tauri::State<'_, Arc<World>>,
    query: String,
) -> Result<Vec<epoch_engine::Offer>, String> {
    world.search_ollama(&query)
}

/// Start llama.cpp knowing about every model on this machine, rather than holding one.
#[tauri::command]
fn start_router(world: tauri::State<'_, Arc<World>>) -> Result<String, String> {
    world.start_router()
}

/// Put every model this machine has on LM Studio's shelf, as links.
#[tauri::command]
fn lend_all_to_lm_studio(world: tauri::State<'_, Arc<World>>) -> Result<String, String> {
    world.lend_all_to_lm_studio()
}

/// Open the user's own terminal on the command that installs one.
///
/// Offered, never imposed. Epoch removes the need to know a terminal was involved and steps
/// back; the licence, the elevation prompt and the output all belong to the person reading them.
#[tauri::command]
fn install_runtime(world: tauri::State<'_, Arc<World>>, id: String) -> Result<(), String> {
    world.install_runtime(&id)
}

/// Start a runtime's server, in the user's own terminal.
#[tauri::command]
fn start_runtime(world: tauri::State<'_, Arc<World>>, id: String) -> Result<(), String> {
    world.start_runtime(&id)
}

/// Everything this World has made.
///
/// Enumerated from the Quests, never a directory walked — the same rule deletion and export
/// follow, and here it is what lets a row say which Quest made a thing and when.
#[tauri::command]
async fn made(world: tauri::State<'_, Arc<World>>) -> Result<Vec<state::MadeView>, String> {
    let world = Arc::clone(&world);
    // Reads every digest in the World. Cheap per Quest and not free across a thousand of them.
    tauri::async_runtime::spawn_blocking(move || world.made())
        .await
        .map_err(|err| err.to_string())
}

/// Open something this World made, with whatever this machine opens that kind of file with.
#[tauri::command]
fn open_made(world: tauri::State<'_, Arc<World>>, reference: String) -> Result<(), String> {
    world.open_made(&reference)
}

/// Serve one picture from the vault, by name, over Epoch's own URI scheme.
///
/// ## Only a name resolves, and that is the whole of the security story
///
/// The path is reduced to its **file-name component** before anything else, exactly as
/// `World::open_picture` and `shared_image` do (ADR-0024). `epoch://picture/../../secrets.dat`
/// and `epoch://picture/C:/Windows/win.ini` both collapse to a name looked for inside the
/// vault's own images folder — they are not refused, they simply cannot address anything else.
/// Everything in that folder arrived through a door that sniffed its bytes, so a name that
/// resolves there is a picture by construction.
///
/// The type is sniffed from the bytes rather than taken from the extension, for the same reason
/// the door does it: a `.png` containing something else is a thing that exists.
/// One catalogue preview, fetched by the Engine and served to the window.
///
/// ## The window asks for a token, and this decides what it means
///
/// The alternative was one line in `tauri.conf.json` — widen `img-src` to the two image CDNs —
/// and it is the wider door: every scroll of a page of results becomes a request from the user's
/// browser context to a third party, and the guarantee that the window talks to nothing but
/// Epoch is spent to save an afternoon. This codebase already chose the narrow door once, for
/// pictures, and measured that it was also the faster one.
///
/// So a row carries sixteen hex digits, the Engine remembers what they stand for, and this
/// fetches it once and keeps it. A token from a search nobody ran resolves to nothing, which is
/// the honest 404.
fn preview_response(token: &str) -> tauri::http::Response<Vec<u8>> {
    let refuse = |code: u16| {
        tauri::http::Response::builder()
            .status(code)
            .body(Vec::new())
            .unwrap_or_default()
    };
    let kept = state::vault_dir().join("previews");
    let Some(bytes) = epoch_engine::models::catalogue::preview_bytes(token, &kept) else {
        return refuse(404);
    };
    let mime = epoch_engine::import::MadeFormat::sniff(&bytes)
        .map_or("application/octet-stream", |format| format.mime());
    tauri::http::Response::builder()
        .status(200)
        .header("Content-Type", mime)
        // The token is a hash of the address, so it cannot come to mean a second picture.
        .header("Cache-Control", "public, max-age=31536000, immutable")
        .body(bytes)
        .unwrap_or_else(|_| refuse(500))
}

/// One spoken line, by the name the Engine gave it.
///
/// **`file_name` and nothing else**, so a name with real separators in it keeps only its last
/// component — the same half of the defence `picture_response` relies on, and it does not depend
/// on the other half holding.
fn spoken_response(name: &str) -> tauri::http::Response<Vec<u8>> {
    let refuse = |code: u16| {
        tauri::http::Response::builder()
            .status(code)
            .body(Vec::new())
            .unwrap_or_default()
    };
    let Some(file) = std::path::Path::new(name.trim()).file_name() else {
        return refuse(404);
    };
    let path = epoch_engine::speech::spoken_dir(&state::vault_dir()).join(file);
    let Ok(bytes) = std::fs::read(&path) else {
        return refuse(404);
    };
    let mime = epoch_engine::import::MadeFormat::sniff(&bytes)
        .map_or("application/octet-stream", |format| format.mime());
    tauri::http::Response::builder()
        .status(200)
        .header("Content-Type", mime)
        // Named by the hash of its own bytes: the same sentence in the same voice is the same
        // file, so it is fetched once however often somebody scrolls past it.
        .header("Cache-Control", "public, max-age=31536000, immutable")
        .body(bytes)
        .unwrap_or_else(|_| refuse(500))
}

/// One catalogue recording, by the token a row carries.
///
/// The audio sibling of `preview_response`, and a sibling rather than a flag: *is this a picture*
/// and *is this a sound* are two checks, and a function taking a boolean would be one place
/// deciding both.
fn sample_response(token: &str) -> tauri::http::Response<Vec<u8>> {
    let refuse = |code: u16| {
        tauri::http::Response::builder()
            .status(code)
            .body(Vec::new())
            .unwrap_or_default()
    };
    let kept = state::vault_dir().join("previews");
    let Some(bytes) = epoch_engine::models::catalogue::sample_bytes(token, &kept) else {
        return refuse(404);
    };
    let mime = epoch_engine::import::MadeFormat::sniff(&bytes)
        .map_or("application/octet-stream", |format| format.mime());
    tauri::http::Response::builder()
        .status(200)
        .header("Content-Type", mime)
        .header("Cache-Control", "public, max-age=31536000, immutable")
        .body(bytes)
        .unwrap_or_else(|_| refuse(500))
}

fn picture_response(path: &str) -> tauri::http::Response<Vec<u8>> {
    let refuse = |code: u16| {
        tauri::http::Response::builder()
            .status(code)
            .body(Vec::new())
            .unwrap_or_default()
    };
    // **The same resolution `open_picture` uses, not a second copy of it.** Two places that each
    // decide what a name may reach are two places to tighten, and the loose one is the one that
    // ends up mattering.
    //
    // **Not percent-decoded, and measured rather than reasoned about.** Every file in this folder
    // is named from the hash of its own bytes — `keep_made` and `keep_shared_image` both do it —
    // so a name is sixteen hex digits and an extension, with nothing in it a URL would escape.
    //
    // Asked through the window with `../../../vault/secrets.dat`, `convertFileSrc` escaped every
    // separator and this saw one long name that is not in the folder: refused in 1 ms. A URL
    // built by hand with real separators is refused by the other half — `file_name` keeps the
    // last component and nothing else — so neither shape depends on the other holding.
    let Ok(file) = state::picture_path(&state::vault_dir(), path.trim_start_matches('/')) else {
        return refuse(404);
    };
    let Ok(bytes) = std::fs::read(&file) else {
        return refuse(404);
    };
    // **What it is, from its bytes** — a picture or a video, both of which a capability can now
    // make (Phase 12). `MadeFormat` rather than `ImageFormat`: the wider list is what a *result*
    // may be, and it is deliberately not what an import may be (ADR-0024).
    let mime = epoch_engine::import::MadeFormat::sniff(&bytes)
        .map_or("application/octet-stream", |format| format.mime());
    tauri::http::Response::builder()
        .status(200)
        .header("Content-Type", mime)
        // Named by the hash of its own bytes, so a name never means two different pictures.
        // The browser may keep it as long as it likes, which is the other half of what this
        // scheme buys: a Chronicle scrolled past and back does not re-read the disk.
        .header("Cache-Control", "public, max-age=31536000, immutable")
        .body(bytes)
        .unwrap_or_else(|_| refuse(500))
}

/// Open a picture from the conversation, by the name the vault knows it under.
///
/// Separate from `open_made` because the Chronicle draws produced pictures and pasted ones with
/// the same component: asking "did this World make it" refuses half of them for a reason the
/// person cannot act on.
#[tauri::command]
fn open_picture(world: tauri::State<'_, Arc<World>>, file: String) -> Result<(), String> {
    world.open_picture(&file)
}

/// Hand a reference picture to the studio, and answer with the name it knows it by.
///
/// Off the UI thread: it uploads bytes to a server. The picture arrives as the `data:` URI the
/// webview's own file input produced — ADR-0024, the frontend never touches a disk.
#[tauri::command]
async fn hand_reference_over(image: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || state::reference_for_studio(&image))
        .await
        .map_err(|why| format!("the picture did not get there: {why}"))?
}

/// What this machine can draw with, and what it is missing (ADR-0030).
///
/// Off the window thread: it asks ComfyUI whether it is answering, which is a loopback request
/// on the path of opening a deck.
#[tauri::command]

async fn image_shelf(world: tauri::State<'_, Arc<World>>) -> Result<state::ImagesView, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.image_shelf())
        .await
        .map_err(|err| err.to_string())
}

/// Take a workflow in, from bytes the window read out of a file input.
#[tauri::command]
async fn import_workflow(
    world: tauri::State<'_, Arc<World>>,
    name: String,
    data: String,
) -> Result<String, String> {
    let world = Arc::clone(&world);
    // Compiling against the server is a network round trip, and a large graph is a real parse.
    tauri::async_runtime::spawn_blocking(move || world.import_workflow(&name, &data))
        .await
        .map_err(|err| err.to_string())?
}

/// Point a Style at a workflow, or take it away.
#[tauri::command]
fn attach_workflow(
    world: tauri::State<'_, Arc<World>>,
    style: String,
    id: String,
    attach: bool,
) -> Result<(), String> {
    world.attach_workflow(&style, &id, attach)
}

/// Forget a workflow, and take it out of every Style that pointed at it.
#[tauri::command]
fn forget_workflow(world: tauri::State<'_, Arc<World>>, id: String) -> Result<(), String> {
    world.forget_workflow(&id)
}

/// Which Style this World draws with when nobody says.
#[tauri::command]
fn draw_usually(world: tauri::State<'_, Arc<World>>, style: String) -> Result<(), String> {
    world.draw_usually(&style)
}

/// Which machine this World draws on. Empty is this one (ADR-0029).
#[tauri::command]
fn draw_on(world: tauri::State<'_, Arc<World>>, machine: String) -> Result<(), String> {
    world.draw_on(&machine)
}

/// What can make a picture on this machine (ADR-0030).
///
/// Off the window thread for the reason every other survey is: a directory walk and a loopback
/// probe, on the path of opening a deck.
#[tauri::command]
async fn image_studios(
    world: tauri::State<'_, Arc<World>>,
) -> Result<Vec<epoch_engine::models::studio::Easel>, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.image_studios())
        .await
        .map_err(|err| err.to_string())
}

/// Build a plain workflow from what this ComfyUI reports, and attach it to General.
///
/// Off the window thread: it asks the server for its whole schema, which is megabytes.
#[tauri::command]
async fn build_workflow(world: tauri::State<'_, Arc<World>>) -> Result<String, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.build_workflow())
        .await
        .map_err(|err| err.to_string())?
}

/// Walk the whole picture chain and report every link.
///
/// **Off the window thread and by some distance the most important one to be**: this draws a
/// real picture, which is seconds of GPU work — on the window thread the World would freeze
/// while it ran, which is exactly the defect that took the Connections deck off it.
#[tauri::command]
async fn test_studio(
    world: tauri::State<'_, Arc<World>>,
) -> Result<Vec<state::StudioCheck>, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.test_studio())
        .await
        .map_err(|err| err.to_string())
}

/// What can speak here.
///
/// Off the window thread for the reason every survey is: it walks a folder and the PATH.
#[tauri::command]
async fn voice_engines(
    world: tauri::State<'_, Arc<World>>,
) -> Result<Vec<epoch_engine::speech::Mouth>, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.voice_engines())
        .await
        .map_err(|err| err.to_string())
}

/// Fetch the voice engine. **The only program on this deck Epoch downloads itself** — measured:
/// Piper has no winget package and no Homebrew formula or cask.
#[tauri::command]
async fn install_voice_engine(
    app: tauri::AppHandle,
    world: tauri::State<'_, Arc<World>>,
    id: String,
) -> Result<String, String> {
    let world = Arc::clone(&world);
    let watching = id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        world.install_voice_engine(&id, &|done, total| {
            // The same event the asset shelf emits, because it is the same question: how far has
            // this download got. A second channel would be a second answer to draw a bar from.
            let _ = app.emit(
                FETCHING,
                serde_json::json!({ "id": watching, "done": done, "total": total }),
            );
        })
    })
    .await
    .map_err(|err| err.to_string())?
}

/// What can listen on this machine.
#[tauri::command]
async fn voice_ears(
    world: tauri::State<'_, Arc<World>>,
) -> Result<Vec<epoch_engine::hearing::Listening>, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.voice_ears())
        .await
        .map_err(|err| err.to_string())
}

/// Fetch the ear, or one of its models. Progress on the same channel the shelves use.
#[tauri::command]
async fn install_ear(
    app: tauri::AppHandle,
    world: tauri::State<'_, Arc<World>>,
    what: String,
) -> Result<String, String> {
    let world = Arc::clone(&world);
    let watching = what.clone();
    tauri::async_runtime::spawn_blocking(move || {
        world.install_ear(&what, &|done, total| {
            let _ = app.emit(
                FETCHING,
                serde_json::json!({ "id": watching, "done": done, "total": total }),
            );
        })
    })
    .await
    .map_err(|err| err.to_string())?
}

/// Turn a recording into words.
///
/// Off the window thread: it starts a program and waits. About a second for anything said in one
/// breath, measured — and a number measured here is not a promise about another machine.
#[tauri::command]
async fn listen(
    world: tauri::State<'_, Arc<World>>,
    wav: String,
    language: Option<String>,
) -> Result<epoch_engine::hearing::Heard, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.listen(&wav, language.as_deref()))
        .await
        .map_err(|err| err.to_string())?
}

/// Which voices are on the shelf, for the control that picks one.
///
/// **Voices, never files** (ADR-0016). Today a Piper voice is one file and the two coincide; the
/// day an engine holds fifty-four voices in one file, this signature does not change.
#[tauri::command]
async fn installed_voices() -> Result<Vec<epoch_engine::speech::Voice>, String> {
    tauri::async_runtime::spawn_blocking(epoch_engine::speech::voices_here)
        .await
        .map_err(|err| err.to_string())
}

/// Say one line with one voice, and hand back the sound.
///
/// Off the window thread: it starts a program and waits for it. Fast here — 400 ms — and a
/// number measured on this machine is not a promise about anybody else's.
#[tauri::command]
async fn try_voice(
    world: tauri::State<'_, Arc<World>>,
    voice: String,
    say: String,
    sounds_like: Option<epoch_kernel::Timbre>,
) -> Result<state::Spoken, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.try_voice(&voice, &say, sounds_like))
        .await
        .map_err(|err| err.to_string())?
}

/// Every converted RVC voice on this machine.
#[tauri::command]
async fn installed_timbres() -> Result<Vec<epoch_engine::speech::Timbre>, String> {
    tauri::async_runtime::spawn_blocking(epoch_engine::speech::timbres_here)
        .await
        .map_err(|err| err.to_string())
}

/// What this machine can do about RVC right now.
#[tauri::command]
async fn voice_forge() -> Result<serde_json::Value, String> {
    let forge = tauri::async_runtime::spawn_blocking(epoch_engine::rvc::look)
        .await
        .map_err(|err| err.to_string())?;
    // **The two derived answers travel with the facts they are derived from.** A surface that
    // recomputed `can_speak` from the booleans would be a second place deciding what "ready"
    // means, and the two would disagree the first time a piece was added -- which is exactly
    // what just happened when the encoder and the runtime arrived.
    let mut out = serde_json::to_value(&forge).map_err(|why| why.to_string())?;
    if let Some(object) = out.as_object_mut() {
        object.insert("canSpeak".into(), forge.can_speak().into());
        object.insert(
            "nextStep".into(),
            forge
                .next_step()
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null),
        );
    }
    Ok(out)
}

/// Build the forge: Epoch's own Python, the definitions, the encoder and the runtime.
///
/// **Long, and off the window thread.** Roughly 1.3 GB the first time, and each step says what
/// it is doing on the channel the shelves already use -- a progress bar cannot be drawn for
/// `pip`, and a sentence naming the step is more honest than a bar that guesses.
#[tauri::command]
async fn prepare_voice_forge(app: tauri::AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        epoch_engine::rvc::prepare(&|step| {
            let _ = app.emit(FETCHING, serde_json::json!({ "id": "rvc", "step": step }));
        })
    })
    .await
    .map_err(|err| err.to_string())?
}

/// Convert one `.pth` on the Timbres shelf into a voice that can be spoken.
#[tauri::command]
async fn convert_timbre(checkpoint: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shelf = epoch_engine::models::generative::Library::here()
            .shelf(epoch_engine::models::generative::Shelf::Timbres);
        let from = shelf.join(&checkpoint);
        if !from.is_file() {
            return Err(format!("'{checkpoint}' is not on the Timbres shelf."));
        }
        epoch_engine::rvc::convert(&from, &shelf).map(|made| {
            made.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        })
    })
    .await
    .map_err(|err| err.to_string())?
}

/// Open the user's own terminal on the command that installs one.
#[tauri::command]
fn install_studio(world: tauri::State<'_, Arc<World>>, id: String) -> Result<(), String> {
    world.install_studio(&id)
}

/// Start one. It may open its own first-run wizard, which is the user's to answer.
#[tauri::command]
fn start_studio(world: tauri::State<'_, Arc<World>>, id: String) -> Result<(), String> {
    world.start_studio(&id)
}

/// Stop the server a studio runs, and say what happened.
#[tauri::command]
fn stop_studio(world: tauri::State<'_, Arc<World>>, id: String) -> Result<String, String> {
    world.stop_studio(&id)
}

/// What this machine can actually run, most used first.
///
/// A moment's work — one search and one request per repository — so it is a command behind a
/// button rather than something a deck does on open.
#[tauri::command]
async fn suited_models(
    world: tauri::State<'_, Arc<World>>,
) -> Result<Vec<epoch_engine::models::Suited>, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.suited_models())
        .await
        .map_err(|why| why.to_string())?
}

/// Time one model on the runtime serving it, and remember the answer.
///
/// A measurement rather than an estimate: it costs a load and two short answers, and one of them
/// turns every other size on the list into a speed.
#[tauri::command]
async fn time_model(
    world: tauri::State<'_, Arc<World>>,
    model: String,
    on: String,
) -> Result<String, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.time_model(&model, &on))
        .await
        .map_err(|why| why.to_string())?
}

/// Which runtimes are answering, and which models each one could time.
///
/// A probe, deliberately its own command: the deck's own read has none in it and must not grow
/// one.
#[tauri::command]
async fn who_can_time(
    world: tauri::State<'_, Arc<World>>,
) -> Result<Vec<state::TimeableOn>, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.who_can_time())
        .await
        .map_err(|why| why.to_string())
}

/// What this machine already has, and what Epoch could add to it.
///
/// A probe of six programs and a package manager: it runs each one to ask its version, so it is
/// its own command and never part of a deck's read.
#[tauri::command]
async fn first_run_survey() -> Result<Vec<epoch_engine::firstrun::Offer>, String> {
    tauri::async_runtime::spawn_blocking(epoch_engine::firstrun::survey)
        .await
        .map_err(|why| why.to_string())
}

/// Add the chosen programs, one at a time, reporting each step as it happens.
///
/// **Minutes, and it is asked for.** Off the window thread, and every line the package manager
/// writes is emitted as it arrives — which is what makes the details a record rather than a
/// summary.
///
/// A step that fails **does not stop the rest**: somebody who asked for four programs and cannot
/// have one of them should still get the other three, and the one that refused says why in the
/// program's own words.
#[tauri::command]
async fn first_run_install(
    app: tauri::AppHandle,
    wanted: Vec<String>,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let all = epoch_engine::firstrun::survey();
        let steps = epoch_engine::firstrun::plan(&all, &wanted);
        let total = steps.len();
        let mut refused: Vec<String> = Vec::new();
        for (index, step) in steps.iter().enumerate() {
            let say = |state: &str, seconds: Option<f64>, line: Option<&str>| {
                let _ = app.emit(
                    PREPARING,
                    serde_json::json!({
                        "index": index,
                        "total": total,
                        "id": step.id,
                        "name": step.name,
                        "because": step.because,
                        "state": state,
                        "seconds": seconds,
                        "line": line,
                    }),
                );
            };
            // Reported before it starts rather than after it finishes, so the row says what it is
            // doing now. A four-minute step that only speaks on completion is one somebody kills
            // at three.
            say("running", None, Some(&step.command));
            match epoch_engine::firstrun::run(step, &|line| say("running", None, Some(line))) {
                Ok(seconds) => say("done", Some(seconds), None),
                Err(why) => {
                    say("failed", None, Some(&why));
                    refused.push(why);
                }
            }
        }
        refused
    })
    .await
    .map_err(|why| why.to_string())
}

/// Say how one model should run.
///
/// Writes the choice and re-tells llama.cpp, which is a file write and a directory walk.
#[tauri::command]
async fn tune_model(
    world: tauri::State<'_, Arc<World>>,
    model: String,
    tuning: epoch_engine::models::tuning::Tuning,
) -> Result<String, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.tune_model(&model, tuning))
        .await
        .map_err(|why| why.to_string())?
}

/// Put one model through the Standard benchmark.
///
/// Minutes, off the window thread, reporting each trial as it starts.
#[tauri::command]
async fn benchmark(
    app: tauri::AppHandle,
    world: tauri::State<'_, Arc<World>>,
    model: String,
    on: String,
) -> Result<String, String> {
    let world = Arc::clone(&world);
    let watching = model.clone();
    tauri::async_runtime::spawn_blocking(move || {
        world.benchmark(&model, &on, &|what, done, total| {
            let _ = app.emit(
                BENCHING,
                serde_json::json!({
                    "model": watching, "what": what, "done": done, "total": total
                }),
            );
        })
    })
    .await
    .map_err(|why| format!("the benchmark did not finish: {why}"))?
}

/// How a search reports itself while it runs.
const OPTIMIZING: &str = "models:optimizing";

/// What the machine is doing, before somebody commits half an hour to measuring it.
///
/// Read on demand rather than kept: it is two samples over three seconds, and a reading from
/// when the deck opened would describe a machine that has since started compiling something.
#[tauri::command]
async fn before_benchmarking(
    world: tauri::State<'_, Arc<World>>,
) -> Result<crate::state::Preflight, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.before_benchmarking())
        .await
        .map_err(|why| format!("the machine could not be asked: {why}"))
}

/// Throw away a paused benchmark, because somebody pressed CANCEL.
///
/// ## Why this is allowed now and was not before
///
/// The pause used to be load-bearing: while a reference gated a search, an unresolved pause was
/// the record saying *this machine is not where it was, and nothing comparative may run*.
/// Clearing it from a button would have cleared the thing doing the work.
///
/// It gates nothing now — a search measures in whatever state it finds and labels the result —
/// so what is left is a banner, and a banner somebody cannot dismiss is worse than no banner.
/// Reported by the owner: CANCEL removed it from the screen and it came back on the next start,
/// which is a control that does not do what it says.
///
/// **The evidence is not what is thrown away.** Every control that pause was built from is an
/// Observation and stays exactly where it was; what goes is the session record and its banner.
#[tauri::command]
fn dismiss_pause(world: tauri::State<'_, Arc<World>>, artifact: String) -> Result<(), String> {
    world.dismiss_pause(&artifact)
}

/// Exactly what a choice of tuning writes into the preset.
///
/// ## Why a control has to show its flags
///
/// The dropdown reads 'draft-mtp' and nothing on the screen says that becomes two lines in a
/// file. The owner asked what the parameters actually are, which is the right question about a
/// control whose values are named after a technique rather than after what they do.
///
/// **Generated by the function that writes them**, so the label cannot drift from the file. There
/// is no second rendering of these flags anywhere, deliberately: a panel that assembled its own
/// version of the line would eventually show one thing and write another, which is exactly how a
/// benchmark came to report a compressed cache it never ran.
#[tauri::command]
fn tuning_writes(tuning: epoch_engine::models::tuning::Tuning) -> String {
    tuning.lines()
}

/// Lets go of the one-search-at-a-time claim, however the search ends.
struct Released(Arc<std::sync::Mutex<Option<String>>>);

impl Drop for Released {
    fn drop(&mut self) {
        if let Ok(mut held) = self.0.lock() {
            *held = None;
        }
    }
}

/// Run the search for one model: try what this build offers and record what each one did.
///
/// **Twenty-odd model loads.** It reports every step, and it can be stopped — a job of this
/// length with no way out is one somebody kills by closing the window.
#[tauri::command]
async fn optimize_model(
    app: tauri::AppHandle,
    world: tauri::State<'_, Arc<World>>,
    model: String,
    // `standard` holds the window at 32K so two models stay comparable; `profile` may move it.
    // Defaults to the strict one — a caller that says nothing gets the honest benchmark.
    phase: Option<String>,
) -> Result<epoch_engine::profiles::Shown, String> {
    let phase = match phase.as_deref() {
        Some("profile") => epoch_engine::optimize::Phase::Profile,
        _ => epoch_engine::optimize::Phase::Standard,
    };
    let world = Arc::clone(&world);

    /*
        **One search at a time.** Two on one card interleave their router restarts, so each one's
        model load lands inside the other's control — and both produce rows that look measured.

        Claimed before the flag is reset, because the reset is itself part of the damage: a second
        call used to clear the cancellation flag, so STOP pressed for the first search stopped
        nothing.

        A poisoned lock is treated as *busy* rather than unwrapped. Somebody pressing a button
        twice should not meet a panic, and refusing is the safe direction for a guard.
    */
    let claim = world
        .searching_for
        .lock()
        .map_err(|_| "a search is already running.".to_owned())
        .and_then(|mut held| match held.as_deref() {
            Some(other) if other == model => {
                Err(format!("a search for {other} is already running."))
            }
            Some(other) => Err(format!(
                "a search for {other} is running. One at a time: two on this card would \
                 interleave, and each would measure the other's model loading."
            )),
            None => {
                *held = Some(model.clone());
                Ok(())
            }
        });
    claim?;

    let stopping = Arc::clone(&world.stop_optimizing);
    stopping.store(false, std::sync::atomic::Ordering::Relaxed);
    let done = Arc::clone(&world.searching_for);
    let letting_go = Released(done);
    tauri::async_runtime::spawn_blocking(move || {
        // Held for as long as the search runs, and released however it ends — returned, refused
        // or unwound. A guard released only on the happy path locks the button after one failure.
        let _letting_go = letting_go;
        let stop = || stopping.load(std::sync::atomic::Ordering::Relaxed);
        let watching = |progress: &epoch_engine::optimize::Progress| {
            let _ = app.emit(OPTIMIZING, progress);
        };
        let found = world.optimize(&model, phase, &stop, &watching)?;

        /*
            **One button, because it is one question.**

            The tuning search and the context ladder were two presses, and the owner's objection
            to that was not the waiting — it was that two buttons doing one job leave a path where
            the measurement never reaches the machine. Measured 2026-09-02: both Qwen models had
            just been searched at 32,768 and both were still *serving* at 16,384, because the
            second stage was a separate press nobody had made.

            The ladder must run after the search rather than beside it: it carries the winning
            configuration up through the windows, so it has nothing to carry until the search has
            named one. Running it first would measure a curve about a machine nobody runs.

            **A refusal here is not a failure of the search.** The tuning half is saved before
            this line, so a ladder that cannot start — a stop, a model that will not hold a larger
            window — leaves everything that was measured exactly where it is. What comes back is
            the record on disk, which is the one the deck reads.
        */
        if stop() {
            return Ok(found.as_shown());
        }

        /*
            **The search leaves the router restarting, and the ladder needs it answering.**

            `optimize` ends by stopping the router and starting it again, so the very next thing
            asking for a serving endpoint asks a port that has not bound yet. Measured 2026-09-02
            on `gemma-3-270m`: eight rows, session complete, and **zero rungs** — the ladder was
            refused with *llama.cpp is not answering* about a llama.cpp that was seconds from
            answering.

            The same wait `put_into_effect` already does, for the same reason. It was missing here
            only because the ladder used to be a separate press, and by the time somebody made it
            the router had long since come back.
        */
        match world.scale_context(&model, &stop, &watching) {
            Ok(whole) => Ok(whole.as_shown()),
            Err(why) => {
                /*
                    **Written down, not emitted.**

                    This used to send the refusal as a `Progress` event, which arrives after the
                    panel has already switched to its completion state — so a stage that did not
                    run left no trace anywhere, and a model with no curve was indistinguishable
                    from a model whose curve nobody had asked for. That is the cold-instrument
                    rule with the instrument missing entirely.

                    On the record instead, where the deck reads it beside the profiles the search
                    did produce.
                */
                let mut said = found;
                said.ladder = Some(why);
                let library = epoch_engine::models::generative::Library::here();
                let mut every = epoch_engine::profiles::Every::load(library.root());
                every.set(&model, said.clone());
                let _ = every.save(library.root());
                Ok(said.as_shown())
            }
        }
    })
    .await
    .map_err(|why| format!("the search did not finish: {why}"))?
}

/// Ask a running search to stop after the configuration it is on.
///
/// **After, not during.** A half-measured configuration is not a row, and killing a model load
/// mid-flight leaves the router holding something nobody asked for.
#[tauri::command]
fn stop_optimizing(world: tauri::State<'_, Arc<World>>) {
    world
        .stop_optimizing
        .store(true, std::sync::atomic::Ordering::Relaxed);
}

/// A short reading of the configuration a model has right now.
#[tauri::command]
async fn quick_test(
    world: tauri::State<'_, Arc<World>>,
    model: String,
) -> Result<epoch_engine::optimize::Quick, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.quick_test(&model))
        .await
        .map_err(|why| format!("the test did not finish: {why}"))?
}

/// What a character on one Brain inherits from MODELS, read-only.
///
/// A character no longer names a window (ADR-0026 amendment), so its panel shows the one MODELS
/// applied — with the provenance beside it, because `64K` means three different things depending
/// on who decided it. Nothing here is writable from a character.
#[tauri::command]
fn inherited_runtime(
    world: tauri::State<'_, Arc<World>>,
    provider: String,
    model: String,
) -> epoch_engine::inherits::Inherited {
    world.inherited_runtime(&provider, &model)
}

/// What has been measured for one model here, and the profiles it produces.
#[tauri::command]
fn optimization(
    world: tauri::State<'_, Arc<World>>,
    model: String,
) -> Option<epoch_engine::profiles::Shown> {
    world.optimization(&model).map(|it| it.as_shown())
}

/// Put one measured configuration into effect, chosen by the user rather than offered by Epoch.
///
/// Addressed by `at` — when the row ran — because the list is reordered by every rule that reads
/// it and an index would apply somebody else's measurement.
#[tauri::command]
async fn use_measured(
    world: tauri::State<'_, Arc<World>>,
    model: String,
    at: u64,
) -> Result<String, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.use_measured(&model, at))
        .await
        .map_err(|why| format!("it was not put into effect: {why}"))?
}

/// Write one profile's whole configuration out.
#[tauri::command]
fn use_profile(
    world: tauri::State<'_, Arc<World>>,
    model: String,
    intent: epoch_engine::profiles::Intent,
) -> Result<String, String> {
    world.use_profile(&model, intent)
}

/// Forget what was measured for one model.
#[tauri::command]
fn forget_optimization(
    world: tauri::State<'_, Arc<World>>,
    model: String,
) -> Result<String, String> {
    world.forget_optimization(&model)
}

/// Whether an interrupted benchmark is waiting for a clean-state check.
///
/// Read when a deck opens, so somebody coming back to the machine is told rather than having to
/// remember. `None` when nothing is waiting, which is the ordinary case.
#[tauri::command]
fn paused_benchmark(world: tauri::State<'_, Arc<World>>) -> Option<epoch_engine::sessions::Paused> {
    world.paused_benchmark()
}

/// Build a reference from the calibrations already taken.
///
/// **A person's decision, not a calibration's.** A run cannot answer whether a fresh process
/// reproduces a state; that is a question about a series of them.
#[tauri::command]
async fn mint_reference(
    world: tauri::State<'_, Arc<World>>,
    model: String,
    wanted: usize,
) -> Result<String, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.mint_reference(&model, wanted))
        .await
        .map_err(|why| format!("minting did not finish: {why}"))?
}

/// Measure this machine's healthy figure, and write down what it is a figure about.
///
/// **Not a check and not a search.** It is what `ReferenceRequired` permits: a control run,
/// recorded with the whole fingerprint, so a later comparison has something to be a comparison
/// *of*. It clears nothing — whether the machine is healthy is the next question and the user's
/// to ask.
#[tauri::command]
async fn calibrate(world: tauri::State<'_, Arc<World>>, model: String) -> Result<String, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.calibrate(&model))
        .await
        .map_err(|why| format!("the calibration did not finish: {why}"))?
}

/// Run only the control and say whether this machine is itself again.
///
/// **It resumes nothing.** A clean answer clears the pause and the next search starts from the
/// beginning — the rows before the break and the rows after it came from two different machines.
#[tauri::command]
async fn clean_state_check(
    world: tauri::State<'_, Arc<World>>,
    model: String,
) -> Result<String, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.clean_state_check(&model))
        .await
        .map_err(|why| format!("the check did not finish: {why}"))?
}

/// Which speculative decodings this build offers, in its own words.
///
/// **One list for the whole deck rather than a copy on every row.** It is a property of the
/// installed `llama-server`, not of a model, and a frontend holding its own copy of eleven
/// strings would be the second place that has to be kept in step — `--spec-type` grew from three
/// values to eleven between releases.
#[tauri::command]
fn speculation_types() -> Vec<String> {
    epoch_engine::models::tuning::SPEC_TYPES
        .iter()
        .map(|it| (*it).to_owned())
        .collect()
}

/// Find the fastest speculative decoding for one model, by measuring every candidate.
///
/// **Far longer than a benchmark** — each configuration restarts llama.cpp and reloads the model —
/// so it reports every step on the same channel, and the label it reports is the configuration
/// being run rather than a percentage nobody can act on.
#[tauri::command]
async fn sweep_speculation(
    app: tauri::AppHandle,
    world: tauri::State<'_, Arc<World>>,
    model: String,
) -> Result<epoch_engine::spec::Sweep, String> {
    let world = Arc::clone(&world);
    let watching = model.clone();
    tauri::async_runtime::spawn_blocking(move || {
        world.sweep_speculation(&model, &|what, done, total| {
            let _ = app.emit(
                BENCHING,
                serde_json::json!({
                    "model": watching, "what": what, "done": done, "total": total
                }),
            );
        })
    })
    .await
    .map_err(|why| format!("the sweep did not finish: {why}"))?
}

/// Every benchmark result this machine holds.
#[tauri::command]
fn benchmarks(world: tauri::State<'_, Arc<World>>) -> Vec<crate::state::Scored> {
    world.benchmarks()
}

/// Forget one model's results.
#[tauri::command]
fn forget_benchmarks(world: tauri::State<'_, Arc<World>>, model: String) -> Result<String, String> {
    world.forget_benchmarks(&model)
}

/// Which runtimes a curve can be mapped on, and how much of it each maps.
///
/// A probe, like `who_can_time`, and its own command for the same reason.
#[tauri::command]
async fn who_can_measure(
    world: tauri::State<'_, Arc<World>>,
) -> Result<Vec<state::MeasurableOn>, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.who_can_measure())
        .await
        .map_err(|why| why.to_string())
}

/// Which GPU backend each runtime has here, and what has been chosen for it.
///
/// A probe — it lists a directory and runs `lms runtime ls` — so it is its own command rather
/// than something the deck's read grows.
#[tauri::command]
async fn runtime_engines(
    world: tauri::State<'_, Arc<World>>,
) -> Result<Vec<state::EnginesView>, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.runtime_engines())
        .await
        .map_err(|why| why.to_string())
}

/// Choose which backend a runtime uses.
#[tauri::command]
async fn choose_engine(
    world: tauri::State<'_, Arc<World>>,
    id: String,
    engine: Option<String>,
) -> Result<String, String> {
    let world = Arc::clone(&world);
    // Runs LM Studio's CLI for one of the three.
    tauri::async_runtime::spawn_blocking(move || world.choose_engine(&id, engine))
        .await
        .map_err(|why| why.to_string())?
}

/// Turn the compressed attention cache on or off for a runtime.
#[tauri::command]
async fn compress_cache(
    world: tauri::State<'_, Arc<World>>,
    id: String,
    on: bool,
) -> Result<String, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.compress_cache(&id, on))
        .await
        .map_err(|why| why.to_string())?
}

/// After a start: wait for the server and prove the compressed cache actually loads a model.
///
/// Minutes, in the worst case — it waits for a server and then loads a model. Off the window
/// thread, and `None` when there was nothing to prove.
#[tauri::command]
async fn prove_runtime(
    world: tauri::State<'_, Arc<World>>,
    id: String,
) -> Result<Option<String>, String> {
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.prove_runtime(&id))
        .await
        .map_err(|why| why.to_string())?
}

/// The creations panel closed: stop the studio if Epoch started it and nothing is being made.
#[tauri::command]
fn close_studio(world: tauri::State<'_, Arc<World>>) -> Option<String> {
    world.close_studio()
}

/// Start a runtime serving one weighed model, in the user's own terminal.
#[tauri::command]
fn serve_model(
    world: tauri::State<'_, Arc<World>>,
    id: String,
    pull: String,
) -> Result<(), String> {
    world.serve_model(&id, &pull)
}

/// Whether this machine has the Hugging Face CLI, and who it is signed in as.
///
/// Two facts with two fixes, kept apart like every agent's: one is solved by installing, the
/// other by signing in.
#[tauri::command]
fn hugging_face(world: tauri::State<'_, Arc<World>>) -> epoch_engine::models::hf::Cli {
    world.hugging_face()
}

/// The Hugging Face CLI on every machine this World can use, asked of each.
///
/// One command rather than one per machine: which machines exist is a question the Engine
/// already answers, and a surface that had to enumerate the roster and then probe each entry
/// would be a second place that knows what a fleet is.
#[tauri::command]
async fn hugging_face_everywhere(
    world: tauri::State<'_, Arc<World>>,
) -> Result<Vec<state::HuggingFaceSomewhere>, String> {
    // One network round trip per paired machine. The panel says *asking each machine* while it
    // waits, which it could not draw while this held the window's thread.
    let world = Arc::clone(&world);
    tauri::async_runtime::spawn_blocking(move || world.hugging_face_everywhere())
        .await
        .map_err(|why| format!("the measurement did not finish: {why}"))
}

/// What a download would fetch, before anything is fetched.
#[tauri::command]
fn plan_download(
    world: tauri::State<'_, Arc<World>>,
    repo: String,
    quant: String,
) -> Result<Vec<epoch_engine::models::hf::Planned>, String> {
    world.plan_download(&repo, &quant)
}

/// Fetch one quantisation onto this machine, for a runtime that takes a file from disk.
///
/// Its own thread, and **no percentage**: measured, `hf download` reports a final path and
/// nothing in between, and none appears on stderr when it is not talking to a terminal. The size
/// is already known from the plan, so the surface can say what it is waiting for without
/// inventing how far along it is.
#[tauri::command]
fn download_file(
    app: tauri::AppHandle,
    world: tauri::State<'_, Arc<World>>,
    repo: String,
    quant: String,
) -> Result<(), String> {
    let world = Arc::clone(&world);
    std::thread::spawn(move || {
        let outcome = world.download_file(&repo, &quant);
        let _ = app.emit(
            FILE_DONE,
            serde_json::json!({
                "repo": repo,
                "quant": quant,
                "at": outcome.as_ref().ok(),
                "failure": outcome.err(),
            }),
        );
    });
    Ok(())
}

/// A file download finished, one way or the other.
const FILE_DONE: &str = "models:downloaded";

/// Every quantisation a repository publishes.
///
/// One request to Hugging Face, and only when somebody opens a result — a search returns sixty
/// repositories and asking each what it contains would be sixty requests for a list nobody may
/// look at.
#[tauri::command]
fn model_variants(
    world: tauri::State<'_, Arc<World>>,
    repo: String,
) -> Result<Vec<epoch_engine::models::Group>, String> {
    world.model_variants(&repo)
}

/// What one model really costs, and whether it fits on this machine.
#[tauri::command]
fn weigh_model(
    world: tauri::State<'_, Arc<World>>,
    name: String,
) -> Result<epoch_engine::Offer, String> {
    world.weigh_model(&name)
}

/// What Epoch is holding that the user might want back, measured now.
#[tauri::command]
fn erasable(world: tauri::State<'_, Arc<World>>) -> Vec<crate::state::ErasableView> {
    world.erasable()
}

/// Erase the things the user ticked, and only those.
///
/// Ids, never paths: a surface sends back which line it ticked, and the Engine decides what
/// that means now.
#[tauri::command]
fn erase(world: tauri::State<'_, Arc<World>>, ids: Vec<String>) -> Result<Vec<String>, String> {
    world.erase(&ids)
}

/// What removing a World would do — asked **before** anything is deleted.
///
/// The plan whose consequences list is longer than its file list, which is the whole reason a
/// World gets a confirmation rather than a button.
#[tauri::command]
fn world_removal(
    world: tauri::State<'_, Arc<World>>,
    world_id: String,
) -> Result<crate::state::RemovalView, String> {
    world.world_removal(&world_id)
}

/// Remove a World, after the user accepted what it does.
#[tauri::command]
fn remove_world(
    world: tauri::State<'_, Arc<World>>,
    world_id: String,
) -> Result<Vec<String>, String> {
    world.remove_world(&world_id)
}

/// Search every asset catalogue at once. Sources that refuse say so beside the results.
///
/// `async` because it reaches two websites: a blocking command would freeze the window for as
/// long as the slower of them takes.
#[tauri::command]
async fn find_assets(
    world: tauri::State<'_, Arc<World>>,
    words: String,
    kind: String,
    adult: bool,
    order: String,
    base: String,
    more: Vec<state::MoreRow>,
) -> Result<state::AssetsFound, String> {
    Ok(world.find_assets(&words, &kind, adult, &order, &base, &more))
}

/// The base models worth offering as a filter, in the sites' own words.
#[tauri::command]
fn asset_bases() -> Vec<String> {
    state::bases_offered()
}

/// Bring one asset into the Generative Library. Gigabytes, and it blocks until it is there.
#[tauri::command]
async fn install_asset(
    app: tauri::AppHandle,
    world: tauri::State<'_, Arc<World>>,
    source: String,
    id: String,
) -> Result<String, String> {
    // **Off the UI thread, and it says how far it has got.** Six gigabytes is minutes, and a
    // window that stops answering for minutes is a window somebody force-quits — the same
    // reasoning that put the loadout search on its own thread with a bar.
    let world = Arc::clone(&world);
    let watching = id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        world.install_asset(&source, &id, &|done, total| {
            // Keyed by the asset's id, because a page of results is a page of rows and only one
            // of them is downloading.
            let _ = app.emit(
                FETCHING,
                serde_json::json!({ "id": watching, "done": done, "total": total }),
            );
        })
    })
    .await
    .map_err(|why| format!("the download did not finish: {why}"))?
}

/// Take a file the person already has into the Generative Library.
///
/// **The shell picks it, and the Engine reads it.** The same bargain as the folder picker
/// (ADR-0024): no filesystem API is added to the webview, and the frontend never learns a path
/// it did not receive from a dialog the user drove.
///
/// `None` when the dialog was dismissed — which is an answer, not a failure.
#[tauri::command]
async fn import_asset(world: tauri::State<'_, Arc<World>>) -> Result<Option<String>, String> {
    let picked = rfd::FileDialog::new()
        .set_title("Choose a model, LoRA, VAE or embedding")
        // What `epoch-assets` can actually read, and nothing else. Offering `.ckpt` would offer
        // a file Epoch refuses to open on purpose.
        .add_filter("What draws", &["safetensors", "sft"])
        .pick_file();
    let Some(path) = picked else {
        return Ok(None);
    };
    world.import_asset(&path).map(Some)
}

/// Remember what the panel was filled in with. Pressing GENERATE does not draw.
#[tauri::command]
async fn choose_recipe(
    world: tauri::State<'_, Arc<World>>,
    ask: state::PanelAsk,
) -> Result<(), String> {
    world.choose_recipe(ask)
}

/// What the Studio Panel offers, for one chosen model (ADR-0033).
#[tauri::command]
async fn studio_panel(
    world: tauri::State<'_, Arc<World>>,
    checkpoint: String,
    clip: Option<Vec<String>>,
) -> Result<state::PanelView, String> {
    // The encoders chosen so far, because what they *are* is what decides how this model is put
    // together — and Epoch can read an encoder for every file on the shelf, while it can read a
    // checkpoint's family for five families and nothing newer.
    Ok(world.studio_panel(&checkpoint, &clip.unwrap_or_default()))
}

/// Start the picture studio because the panel needs something only a running one can answer.
///
/// **Not the Workshop's START, and the difference is who owns it.** That one is somebody
/// deliberately running a server, so Epoch never stops it. This is Epoch starting one on the
/// user's behalf, which is the only thing that licenses stopping it again once the picture is
/// made — the same call GENERATE makes, without the waiting.
#[tauri::command]
async fn wake_the_studio() -> bool {
    tauri::async_runtime::spawn_blocking(|| state::studio::wake_studio(std::time::Duration::ZERO))
        .await
        .unwrap_or(false)
}

/// Make the picture the panel describes.
#[tauri::command]
async fn draw_from_panel(
    world: tauri::State<'_, Arc<World>>,
    ask: state::PanelAsk,
) -> Result<state::PanelMade, String> {
    world.draw_from_panel(ask)
}

/// Which asset catalogues hold a key. Names and a yes/no; never a value.
#[tauri::command]
fn catalogue_keys(world: tauri::State<'_, Arc<World>>) -> Vec<state::CatalogueKeyView> {
    world.catalogue_keys()
}

/// Keep one catalogue's key, encrypted for this Windows account. Write-only.
#[tauri::command]
fn save_catalogue_key(
    world: tauri::State<'_, Arc<World>>,
    source: String,
    value: String,
) -> Result<(), String> {
    world.save_catalogue_key(&source, &value)
}

/// Throw one catalogue's key away.
#[tauri::command]
fn forget_catalogue_key(world: tauri::State<'_, Arc<World>>, source: String) -> Result<(), String> {
    world.forget_catalogue_key(&source)
}

/// Throw this machine's door token away and mint a new one.
///
/// The user's to press. Every open door stops working, which is what makes it worth pressing.
#[tauri::command]
fn regenerate_door(world: tauri::State<'_, Arc<World>>) -> Result<(), String> {
    world.regenerate_door()
}

/// Move a character into or out of a World.
#[tauri::command]
fn set_character_world(
    world: tauri::State<'_, Arc<World>>,
    character_id: String,
    world_id: String,
    lives_there: bool,
) -> Result<(), String> {
    world.set_character_world(&character_id, &world_id, lives_there)
}

/* ------------------------------------------------------------------------- */
/* The World Editor (ADR-0028)                                               */
/* ------------------------------------------------------------------------- */

/// Stop the World so its map can be changed.
///
/// Fails with the sentence the user needs — *"Mage is thinking with gemma4:12b. Finish that
/// before editing."* — rather than a generic refusal. The editor never interrupts work; it
/// waits for it.
#[tauri::command]
fn begin_editing(world: tauri::State<'_, Arc<World>>) -> Result<(), String> {
    world.freeze()
}

/// Let the World run again.
#[tauri::command]
fn end_editing(world: tauri::State<'_, Arc<World>>) {
    world.thaw();
    // Undo is an offer to take back what you just did, not a history of the vault.
    world.forget_edits();
}

/// Put the map back the way it was before the last edit.
///
/// `false` means there was nothing to take back — the surface says so rather than flashing as
/// though something happened.
#[tauri::command]
fn undo_edit(world: tauri::State<'_, Arc<World>>) -> Result<bool, String> {
    world.undo()
}

/// Do again what was just undone.
#[tauri::command]
fn redo_edit(world: tauri::State<'_, Arc<World>>) -> Result<bool, String> {
    world.redo()
}

/// How many steps back and forward are available. Measured, so the buttons can be honestly dim.
#[tauri::command]
fn history_depth(world: tauri::State<'_, Arc<World>>) -> (usize, usize) {
    world.history_depth()
}

/// Say which building somebody lives in, in this World (ADR-0028).
///
/// Written to the character's own file, because the roster lives with the character (ADR-0023).
/// `null` clears it: they live here and it is not settled where.
#[tauri::command]
fn set_character_home(
    world: tauri::State<'_, Arc<World>>,
    character_id: String,
    place_id: Option<String>,
) -> Result<(), String> {
    world.set_character_home(&character_id, place_id.as_deref())
}

/// Say where one of somebody's routine activities happens, in this World (ADR-0028).
///
/// The activity is theirs and travels with them into every World; the building is this World's.
/// `null` puts it back to happening at home.
///
/// **This call is the whole of authoring idle movement.** A character walks on their routine
/// because somebody made this decision and for no other reason — the causality rule, at the
/// surface rather than only inside the Simulation.
#[tauri::command]
fn set_routine_place(
    world: tauri::State<'_, Arc<World>>,
    character_id: String,
    activity: String,
    place_id: Option<String>,
) -> Result<(), String> {
    world.set_routine_place(&character_id, &activity, place_id.as_deref())
}

/// What somebody would be given, if the work were handed to them now.
///
/// So the user can read it before agreeing to it. Nothing is sent and nothing changes — asking
/// what a handover would carry is not a handover.
#[tauri::command]
fn handover_preview(world: tauri::State<'_, Arc<World>>, character_id: String) -> Option<String> {
    world.handover_preview(&character_id)
}

/// What this World's windows look like (ADR-0016).
///
/// Concepts to images, resolved by the active pack and delivered as `data:` URIs. An empty answer
/// is the ordinary one: Epoch draws its own windows, and a World with no artwork is complete.
#[tauri::command]
fn ui_skin(
    world: tauri::State<'_, Arc<World>>,
) -> std::collections::BTreeMap<String, epoch_engine::pack::Skin> {
    world.ui_skin()
}

/// What this World sounds like. Its own command, like the skin, and for the same reason.
#[tauri::command]
fn world_sounds(world: tauri::State<'_, Arc<World>>) -> std::collections::BTreeMap<String, String> {
    world.world_sounds()
}

/// Give this World a sound of its own, or take it back to the one Epoch synthesises.
///
/// The audio arrives base64-encoded from the webview's own `<input type="file">`, like every
/// other import (ADR-0024), and the Engine names the file from its bytes.
#[tauri::command]
fn set_world_sound(
    world: tauri::State<'_, Arc<World>>,
    concept: String,
    audio: Option<String>,
) -> Result<(), String> {
    world.set_world_sound(&concept, audio.as_deref())
}

/// One picture shared in a conversation, fetched by name and cached by the surface.
///
/// Separate from the Chronicle projection for the reason the skin and the backdrop already
/// are: images are large, and a Quest is re-read on every turn. `None` means the file is no
/// longer in the vault, which the surface says rather than drawing a broken image.
#[tauri::command]
fn shared_image(world: tauri::State<'_, Arc<World>>, file: String) -> Option<String> {
    world.shared_image(&file)
}

/// The map as the editor works on it: what the user named things, and what joins what.
#[tauri::command]
fn get_map(world: tauri::State<'_, Arc<World>>) -> epoch_engine::places::WorldMap {
    world.map()
}

/// Give a Place a different name. Its identity does not move, so roads, homes and History all
/// still point at it.
#[tauri::command]
fn rename_place(
    world: tauri::State<'_, Arc<World>>,
    id: String,
    name: String,
) -> Result<(), String> {
    world.rename_place(&id, &name)
}

/// Build something new, somewhere. Returns its identity.
///
/// The surface supplies the position: it knows how big this World is and what the user was
/// looking at, and the Engine deliberately does not guess.
#[tauri::command]
fn add_place(
    world: tauri::State<'_, Arc<World>>,
    name: String,
    x: Option<f32>,
    y: Option<f32>,
) -> Result<String, String> {
    world.add_place(&name, x.zip(y))
}

/// Put a Place somewhere. Its identity does not move, so every road, home and Quest in History
/// still points at the same building — it is simply standing somewhere else now.
#[tauri::command]
fn move_place(
    world: tauri::State<'_, Arc<World>>,
    id: String,
    x: f32,
    y: f32,
) -> Result<(), String> {
    world.move_place(&id, x, y)
}

/// Change how big a Place is, in world units.
///
/// Size is information — Places are not uniform, and a bigger building reads as a more
/// important one. That is why it is the user's to set rather than something derived.
#[tauri::command]
fn resize_place(
    world: tauri::State<'_, Arc<World>>,
    id: String,
    footprint: f32,
) -> Result<(), String> {
    world.resize_place(&id, footprint)
}

/// Give a building artwork, or take it away.
///
/// `image` is base64 image bytes from the webview's own file input — the frontend never touches
/// the filesystem, and the Engine names the file from the bytes (ADR-0024). `null` clears it and
/// the building goes back to whatever the World Pack drew.
#[tauri::command]
fn set_place_art(
    world: tauri::State<'_, Arc<World>>,
    id: String,
    image: Option<String>,
) -> Result<(), String> {
    world.set_place_art(&id, image.as_deref())
}

/// The land the user painted, as a `data:` URI.
///
/// Its own command because it is megabytes: `world:changed` re-sends the projection on every
/// presence change, and a full-map illustration must not ride along.
#[tauri::command]
fn world_land(world: tauri::State<'_, Arc<World>>) -> Option<String> {
    world.land()
}

/// Paint the land, or strip it back to the World's own geography.
#[tauri::command]
fn set_world_land(
    world: tauri::State<'_, Arc<World>>,
    image: Option<String>,
) -> Result<(), String> {
    world.set_land(image.as_deref())
}

/// Take one down, and every road that led to it.
#[tauri::command]
fn remove_place(world: tauri::State<'_, Arc<World>>, id: String) -> Result<(), String> {
    world.remove_place(&id)
}

/// Join two Places, or part them.
#[tauri::command]
fn connect_places(
    world: tauri::State<'_, Arc<World>>,
    from: String,
    to: String,
    joined: bool,
    via: Option<Vec<[f32; 2]>>,
) -> Result<(), String> {
    world.connect_places(&from, &to, joined, via)
}

/// Places nothing leads to.
///
/// Reported while authoring, where it is help, rather than enforced during a handoff, where it
/// would be scenery vetoing collaboration (ADR-0028).
#[tauri::command]
fn unreachable_places(world: tauri::State<'_, Arc<World>>) -> Vec<String> {
    world.unreachable_places()
}

/// How tall a character stands, relative to a Place. `null` puts them back to the crew's height.
///
/// A scale, never a resize: the imported file is untouched, so this is free to undo.
#[tauri::command]
fn set_character_scale(
    world: tauri::State<'_, Arc<World>>,
    character_id: String,
    scale: Option<f64>,
) -> Result<(), String> {
    world.set_character_scale(&character_id, scale)
}

/// Whether the World is stopped for editing.
#[tauri::command]
fn is_editing(world: tauri::State<'_, Arc<World>>) -> bool {
    world.is_frozen()
}

fn main() {
    // Where the provider writes the last turn it sent, so "was she told?" stops being a
    // guess. Set here rather than read from the environment by the engine, so the location is
    // the vault and not wherever the process happened to start.
    //
    // Safety: single-threaded, before anything else runs.
    unsafe {
        std::env::set_var("EPOCH_TRACE_DIR", state::vault_dir());
    }

    let world = Arc::new(World::load());
    for problem in world.problems() {
        eprintln!("[world] {problem}");
    }

    tauri::Builder::default()
        .manage(Arc::clone(&world))
        // **A picture reaches the window as bytes now, not as a string.**
        //
        // Measured on a 182 MB render, opening FILES: 2135 ms for the Engine to read it and
        // base64 it across the IPC, 348 ms for `atob`, 918 ms to decode 132 megapixels — 3.4 s
        // for one thumbnail, and that page draws several. The first two of those are what this
        // removes entirely: the browser fetches the file itself, caches it, and decodes it on
        // its own threads, so JavaScript never holds the bytes at all.
        //
        // **ADR-0024 is unchanged in what it protects.** The rule was never *base64*; it is that
        // the frontend never touches the filesystem and the Engine names the file. That holds
        // exactly as before: the page asks for a **name**, this handler decides, and no `fs`
        // capability and no path ever reach the webview. What changes is the transport.
        .register_uri_scheme_protocol("epoch", |_ctx, request| {
            // Two things this scheme serves, and the path says which. Everything that is not a
            // preview is a picture from the vault, which is what it has always been.
            // **One segment, and the prefix says which.** `convertFileSrc` escapes every
            // separator into the name, which is what keeps a name from reaching out of its
            // folder — so a preview is `preview-<token>` rather than a second path segment, and
            // that property holds for both things this scheme serves.
            let path = request.uri().path();
            let one = path.trim_start_matches('/');
            // Three things now, and the prefix says which. `convertFileSrc` escapes every
            // separator into the name, so a prefix rather than a second path segment is what
            // keeps a name from reaching out of its folder — the property all three rely on.
            if let Some(token) = one.strip_prefix("preview-") {
                preview_response(token)
            } else if let Some(name) = one.strip_prefix("spoken-") {
                spoken_response(name)
            } else if let Some(token) = one.strip_prefix("sample-") {
                sample_response(token)
            } else {
                picture_response(path)
            }
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            // The one window, for the one capability whose whole effect is on the screen.
            state::remember_window(handle.clone());

            // **Epoch answers the microphone question, having asked it in its own words.**
            // Attached here rather than lazily: the browser raises the request the first time
            // `getUserMedia` runs, and a handler registered after that is a handler that was
            // not there when it mattered. With nothing stored it answers nothing and Edge asks,
            // which is what the product did before this existed.
            if let Some(window) = handle.get_webview_window("main") {
                microphone::answer_for(&window, state::vault_dir());
            }

            // **What is on this machine is configured, without being asked for twice**
            // (owner, 2026-09-07). Somebody who installed llama.cpp — through Epoch's own
            // Setup, or years before Epoch existed — has already said what they want to think
            // with, and making them then type its address into a form is asking the same
            // question again.
            //
            // Once, at startup, and off the thread that draws: it walks directories and opens
            // TCP gates, which is 624 ms measured and not a thing to put in front of the first
            // frame. It only ever adds — a configured entry wins on id, because somebody who
            // typed an address has said something a measurement must not overrule.
            std::thread::spawn(|| {
                let added = epoch_engine::backends::adopt_installed(&state::vault_dir());
                if !added.is_empty() {
                    // Said out loud rather than done silently: this changed the user's own
                    // `providers.toml`, and a file that edits itself with nothing on the record
                    // is the thing this codebase keeps deleting.
                    eprintln!(
                        "epoch: configured what is installed here — {}",
                        added.join(", ")
                    );
                }
            });

            let world = Arc::clone(&world);

            // The Simulation owns time (ADR-0018). A plain thread is enough: this advances
            // presence and reports only real changes, so there is no need for an async
            // runtime here.
            // One probe at a time: the answer takes as long as it takes, and a second question
            // asked on top of the first is two threads waiting for one server.
            let asking_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
            std::thread::spawn(move || {
                use std::sync::atomic::Ordering::Relaxed;
                let asking = Arc::clone(&asking_flag);
                let mut last: Vec<Shape> = world.presences().iter().map(shape).collect();
                loop {
                    std::thread::sleep(TICK);

                    // **Is the card free?** Asked only while something is waiting for it, because
                    // a `/api/ps` twice a second forever is a network call nobody needed. The
                    // render waits on this rather than on the turn ending: measured, a cold model
                    // made one turn take 901 s, and a render that waits for *that* is a render
                    // that gives up and overlaps anyway.
                    //
                    // **On its own thread, and that is not tidiness.** The first version asked
                    // inline and the whole heartbeat stopped with it — presence never published,
                    // finished work never collected, and every turn that drew a picture ran to
                    // the driver's 900 s patience twice in a row, to the tenth of a second. This
                    // loop owns time (ADR-0018); nothing that talks to a network belongs in it.
                    if epoch_engine::jobs::anything_waiting_for_the_card()
                        && !asking.swap(true, Relaxed)
                    {
                        let world = Arc::clone(&world);
                        let asking = Arc::clone(&asking_flag);
                        std::thread::spawn(move || {
                            epoch_engine::jobs::the_card_is_free(world.nothing_is_resident());
                            asking.store(false, Relaxed);
                        });
                    }

                    // **Work that finished while nobody was talking** (ADR-0034). The heartbeat
                    // is the only thread that runs whether or not a turn is in flight, which is
                    // exactly the situation a Job exists for: the render landed minutes after
                    // the turn that asked for it ended, and there is no turn left to notice.
                    for job in world.collect_finished_work() {
                        let _ = handle.emit(
                            "world:finished",
                            serde_json::json!({
                                "characterId": job.character.as_str(),
                                "questId": job.quest.as_str(),
                                "what": job.what,
                                "seconds": job.how_long().as_secs(),
                            }),
                        );
                    }

                    // Definitions are the source of truth on disk; pick up edits live.
                    let reloaded = world.reload_if_changed();
                    if reloaded {
                        for problem in world.problems() {
                            eprintln!("[world] {problem}");
                        }
                    }

                    // **Time passes here, and only here.** Everything about who may move and
                    // why was decided by the Simulation; this thread supplies the heartbeat and
                    // carries the answer to a window.
                    let moved = world.advance();

                    let presences = world.presences();
                    let now: Vec<Shape> = presences.iter().map(shape).collect();
                    let sent = if reloaded || now != last {
                        // The World is a different shape: somebody set out, arrived, started
                        // work, or a file changed on disk. A surface may need more than a
                        // position for any of those, so it gets the World.
                        handle.emit(WORLD_CHANGED, world.view())
                    } else if !moved.is_empty() {
                        // Same shape, further along. Only the travellers, and only what moved.
                        let travelling: Vec<_> = presences
                            .iter()
                            .filter(|p| p.is_travelling())
                            .map(epoch_engine::world::PresenceView::of)
                            .collect();
                        handle.emit(WORLD_MOVED, travelling)
                    } else {
                        Ok(())
                    };

                    // A failed emit means the window is gone; the loop ends with it.
                    if sent.is_err() {
                        break;
                    }
                    last = now;
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_world,
            list_worlds,
            enter_world,
            create_world,
            scan_project,
            ships_log,
            list_agents,
            agent_plan_usage,
            sign_in_agent,
            add_agent_account,
            rename_agent_account,
            remove_agent_account,
            agent_models,
            gate_note,
            reasoning_dial,
            remembered_context,
            remembered_contexts,
            choose_reasoning,
            open_agent_door,
            adopt_project,
            close_agent_door,
            agent_bridge,
            agent_configuration,
            answer_agent,
            rename_world,
            leave_world,
            save_character,
            set_character_world,
            begin_editing,
            end_editing,
            undo_edit,
            redo_edit,
            history_depth,
            set_character_home,
            set_routine_place,
            handover_preview,
            ui_skin,
            shared_image,
            get_map,
            rename_place,
            add_place,
            move_place,
            resize_place,
            remove_place,
            set_place_art,
            world_land,
            set_world_land,
            connect_places,
            unreachable_places,
            is_editing,
            set_character_scale,
            get_backdrop,
            list_providers,
            get_surface,
            open_link,
            list_backends,
            save_backend,
            forget_backend,
            save_backend_key,
            stop_turn,
            list_mcp,
            probe_mcp,
            refresh_mcp,
            workshop_search,
            workshop_install,
            terminal_shells,
            terminal_open,
            terminal_write,
            terminal_resize,
            terminal_scrollback,
            terminal_close,
            terminal_open_ids,
            save_mcp,
            save_mcp_secret,
            forget_mcp,
            speak_to,
            compact_context,
            hand_over,
            active_quest,
            set_quest_aside,
            hire_character,
            suggested_prompt,
            list_conversations,
            select_quest,
            close_quest,
            rename_quest,
            get_settings,
            set_concurrent_crew,
            set_microphone,
            set_hearing_language,
            set_tool_rounds,
            set_world_art,
            set_world_skin,
            world_sounds,
            set_world_sound,
            set_character_art,
            set_character_cut,
            character_removal,
            remove_character,
            find_assets,
            asset_bases,
            install_asset,
            studio_panel,
            choose_recipe,
            import_asset,
            draw_from_panel,
            wake_the_studio,
            catalogue_keys,
            save_catalogue_key,
            forget_catalogue_key,
            regenerate_door,
            world_removal,
            remove_world,
            erasable,
            this_machine,
            bridges,
            enrol_machine,
            pair,
            set_bridge_grants,
            unpair,
            disclosures,
            answer_disclosure,
            readiness,
            featured_models,
            search_models,
            pull_model,
            weigh_model,
            model_variants,
            reassign_brain,
            local_runtimes,
            start_runtime,
            image_studios,
            test_studio,
            build_workflow,
            image_shelf,
            hand_reference_over,
            made,
            open_made,
            open_picture,
            import_workflow,
            attach_workflow,
            forget_workflow,
            draw_usually,
            draw_on,
            install_studio,
            voice_engines,
            install_voice_engine,
            try_voice,
            installed_timbres,
            voice_forge,
            prepare_voice_forge,
            convert_timbre,
            installed_voices,
            voice_ears,
            install_ear,
            listen,
            start_studio,
            stop_studio,
            close_studio,
            suited_models,
            time_model,
            who_can_time,
            first_run_survey,
            first_run_install,
            tune_model,
            tuning_writes,
            benchmark,
            benchmarks,
            sweep_speculation,
            speculation_types,
            paused_benchmark,
            dismiss_pause,
            clean_state_check,
            calibrate,
            mint_reference,
            optimize_model,
            use_measured,
            before_benchmarking,
            stop_optimizing,
            quick_test,
            optimization,
            inherited_runtime,
            use_profile,
            forget_optimization,
            forget_benchmarks,
            who_can_measure,
            runtime_engines,
            choose_engine,
            compress_cache,
            prove_runtime,
            install_runtime,
            shared_weights,
            remove_model,
            models_and_loadouts,
            warmth,
            hold_warm,
            search_loadout,
            choose_loadout,
            import_weights,
            world_export,
            export_world,
            search_ollama,
            start_router,
            lend_all_to_lm_studio,
            serve_model,
            hugging_face,
            hugging_face_everywhere,
            plan_download,
            download_file,
            erase,
            answer_pending,
            get_autonomy,
            get_workspace,
            get_offered,
            get_undoable,
            answer_capability,
            list_standing,
            forget_standing,
            undo_last,
            set_autonomy,
            choose_folder,
            set_project_root,
            choose_library,
            set_library,
            scan_library,
            open_library,
            rename_orchestrator,
            set_orchestrator_portrait
        ])
        // Nothing the user opened outlives the window they opened it from. A terminal is a real
        // process, and a shell left running after Epoch closed is one nobody can see, nobody can
        // type into, and nobody will think to end — the same rule as a credential left encrypted
        // after its server was removed.
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                let world = window.state::<Arc<World>>().inner().clone();
                world.terminals_close_all();
                // **What was still running stopped, and History says so** (ADR-0034). Nothing is
                // resumed — a half-finished render cannot be, because the server it was talking
                // to is going down with us — and nothing is silently dropped, which is what
                // ADR-0025's failure states are for.
                world.record_what_was_interrupted();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to open the World");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_turn_payloads_identify_the_quest_as_well_as_the_character() {
        let started = started_payload("mage", "q-a");
        assert_eq!(started["characterId"], "mage");
        assert_eq!(started["questId"], "q-a");

        let step = step_payload(
            "mage",
            "q-a",
            &epoch_engine::Step::Said {
                capability: "shell".into(),
                line: "created proof.txt".into(),
            },
        );
        assert_eq!(step["characterId"], "mage");
        assert_eq!(step["questId"], "q-a");

        let failed = ended_payload("mage", "q-a", &Err("stopped".into()), &[]);
        assert_eq!(failed["characterId"], "mage");
        assert_eq!(failed["questId"], "q-a");
    }
}
