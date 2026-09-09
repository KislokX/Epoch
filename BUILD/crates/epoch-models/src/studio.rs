//! The programs on this machine that **make** pictures, as opposed to reading them.
//!
//! ## Why this is not a `Runtime`
//!
//! `runtimes.rs` answers *what can think here*: every entry there becomes a Provider a character
//! can be assigned to. ComfyUI is not one — a character does not think in pictures, it asks for
//! one (ADR-0030), and a diffusion server appearing in the Brain dropdown would be a brain nobody
//! can hold a conversation with.
//!
//! What the two genuinely share is **how a program is found and offered**: a binary somewhere an
//! installer put it, a port that answers or does not, an install command, and a start that opens
//! the machine's own terminal. Those helpers are reused; the concept is not.
//!
//! ## Three facts, three fixes
//!
//! Installed · serving · neither — the same triple the runtimes panel draws, because they have
//! the same three answers and a person reading both should not have to learn two vocabularies.

use serde::Serialize;

/// A program that turns a prompt into an image.
///
/// One variant today and an enum anyway: ADR-0030 names A1111/SD.Next as the second family, and
/// the difference between them is an endpoint and a body — exactly the shape an enum holds. What
/// it must never become is a list somebody adds a *runtime* to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Studio {
    /// A desktop application with a server inside it, and a workflow API in front of that.
    ComfyUi,
}

impl Studio {
    pub const ALL: [Studio; 1] = [Studio::ComfyUi];

    pub const fn id(self) -> &'static str {
        match self {
            Studio::ComfyUi => "comfyui",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Studio::ComfyUi => "ComfyUI",
        }
    }

    /// Where its server listens when nobody has moved it.
    pub const fn usual_port(self) -> u16 {
        match self {
            Studio::ComfyUi => 8188,
        }
    }

    /// What counts as this being here.
    ///
    /// **Measured after installing it, not guessed.** `winget install Comfy.ComfyUI-Desktop` puts
    /// `Comfy Desktop.exe` in `%LOCALAPPDATA%\Programs\Comfy Desktop\` — the same shape as LM
    /// Studio, and nothing lands on the PATH.
    const fn binaries(self) -> &'static [&'static str] {
        match self {
            Studio::ComfyUi => &["Comfy Desktop", "ComfyUI"],
        }
    }

    /// The folder an installer that behaves puts it in, under `Programs` or `/Applications`.
    const fn program_folder(self) -> Option<&'static str> {
        match self {
            Studio::ComfyUi => Some("Comfy Desktop"),
        }
    }

    /// How somebody installs it on this platform.
    ///
    /// Measured against winget itself: the id is `Comfy.ComfyUI-Desktop`. A plausible id that
    /// does not exist is worse than none — somebody pastes it and blames themselves.
    ///
    /// **The macOS half was the rule proving itself.** It said `--cask comfyui`, which was never
    /// measured and does not exist: asked on 2026-08-22, `formulae.brew.sh` answers 404 for
    /// `comfyui` and the whole cask list holds exactly one match — **`comfy`**, *Comfy Desktop*,
    /// *"Node-based image, video and audio generator"*. Found the day the paired MacBook could
    /// first be asked, which is the only way it could have been found.
    pub const fn install_command(self) -> &'static str {
        match (self, cfg!(windows)) {
            (Studio::ComfyUi, true) => "winget install Comfy.ComfyUI-Desktop",
            (Studio::ComfyUi, false) => "brew install --cask comfy",
        }
    }

    /// What to expect when Epoch starts the server itself.
    ///
    /// **The wizard belongs to the dashboard, not to the server.** This used to name the
    /// first-run questions — where to keep models, accept the licence — which was right while the
    /// only thing Epoch could press was the desktop application. Starting `main.py` directly asks
    /// none of them, so saying it would be describing a screen that will not appear.
    ///
    /// The twelve seconds are measured, twice, on this machine.
    pub const fn starting(self) -> &'static str {
        match self {
            Studio::ComfyUi => {
                "Epoch starts its server directly, in a terminal window you can read. It takes \
                 about twelve seconds to answer; nothing here asks you anything."
            }
        }
    }

    /// What the desktop application asks the first time somebody opens it.
    ///
    /// Kept because it is still true of the front door, and because a licence accepted on
    /// somebody's behalf is not accepted.
    pub const fn first_run(self) -> &'static str {
        match self {
            Studio::ComfyUi => {
                "The first time it opens it asks where to keep its models and to accept its own \
                 licence. Epoch answers neither of those for you."
            }
        }
    }

    /// What to expect when Epoch can only open the program's own front door.
    ///
    /// Measured 2026-08-23: `Comfy Desktop.exe` opens a dashboard — *New Instance*, the
    /// installed one, *Comfy Cloud* — and nothing answers on 8188 until an instance is chosen
    /// and finishes loading. Telling somebody it is starting would be inventing an event.
    pub const fn front_door(self) -> &'static str {
        match self {
            Studio::ComfyUi => {
                "Its own window opens on a list of instances. Choose one, wait for it to finish \
                 loading, then press ASK AGAIN here."
            }
        }
    }
}

/// What one image studio is on this machine, right now.
///
/// Deliberately the same shape as `runtimes::Available` without being the same type: they are
/// drawn alike because they answer alike, and sharing the struct would be the first step to
/// sharing the *list*, which is how a diffusion server ends up in the Brain dropdown.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Easel {
    pub id: &'static str,
    pub name: &'static str,
    /// A binary was found. `false` is *not here*, and the fix is `install`.
    pub installed: bool,
    /// Where it was found, so "not installed" is a fact somebody can go and check.
    pub found_at: Option<String>,
    /// Its API answered. The fact that actually matters.
    pub serving: bool,
    /// The address that answered, or the one that was tried.
    pub endpoint: String,
    /// Checkpoints it reported holding. Empty is honest — a fresh install has none, and that is
    /// the difference between *running* and *able to make a picture*.
    pub models: Vec<String>,
    pub install: &'static str,
    /// The command that would start it, when the program is on this machine.
    pub start: Option<String>,
    /// Whether that command starts the **server** or only opens the program's own front door.
    ///
    /// Measured, and it is why this field exists: Comfy Desktop's executable opens a dashboard
    /// listing instances, and nothing is serving until somebody picks one and waits. A panel that
    /// said *starting it* about that would be describing something that did not happen — so when
    /// Epoch can start the server itself it says so, and when it cannot it says what to do next.
    pub starts_server: bool,
    /// What it will ask the person the first time.
    pub first_run: &'static str,
}

/// Ask this machine about its image studios.
///
/// One loopback request with a short timeout and a directory walk. Nothing is started and nothing
/// is downloaded.
pub fn survey() -> Vec<Easel> {
    Studio::ALL.into_iter().map(look_for).collect()
}

/// One studio, measured.
/// How long to wait for a server on this machine to pick up the phone.
///
/// **Not a request timeout — the one before it.** A server that is running answers loopback in
/// microseconds; one that is not should say so at once and, measured on this machine, takes 21 s
/// to say anything at all. Everything here is local, so anything past a few hundred milliseconds
/// is a port with nothing behind it.
const CONNECT: std::time::Duration = std::time::Duration::from_millis(400);

pub fn look_for(studio: Studio) -> Easel {
    let endpoint = format!("http://127.0.0.1:{}", studio.usual_port());
    let found = studio
        .binaries()
        .iter()
        .find_map(|name| crate::runtimes::found_in(name, None, studio.program_folder()));
    let models = checkpoints_at(&endpoint);

    // The server itself when Epoch can reach it, and the program's own front door otherwise.
    // Never both, and never the front door presented as the server.
    let server = server_command(studio);
    let starts_server = server.is_some();

    Easel {
        id: studio.id(),
        name: studio.name(),
        installed: found.is_some() || starts_server,
        found_at: found.as_ref().map(|path| path.display().to_string()),
        serving: models.is_some(),
        endpoint,
        models: models.unwrap_or_default(),
        install: studio.install_command(),
        start: server.or_else(|| found.map(|path| format!("\"{}\"", path.display()))),
        starts_server,
        first_run: if starts_server {
            studio.starting()
        } else {
            studio.front_door()
        },
    }
}

/// The command that starts this studio's **server**, skipping any dashboard it ships with.
///
/// ## Why this is not the executable
///
/// Comfy Desktop's `.exe` opens a window listing instances; the server starts when somebody picks
/// one. So *press START, then press ASK AGAIN and hope* was the whole interaction, and neither
/// press did what its label said.
///
/// Epoch already knows exactly where the install is — the desktop app writes
/// `installations.json` and names its `installPath`, which is ADR-0032's rule running forwards:
/// ask the program, never walk the disk. Inside it are `main.py` and a virtual environment with
/// torch in it, which is all a server needs.
///
/// ## The one thing this must not lose
///
/// **Measured, and it would have been a silent regression:** running `main.py` on its own listed
/// *only* Epoch's own library and none of the user's checkpoints. Comfy Desktop keeps its model
/// paths in a generated `instance-model-paths/<id>.yaml` and passes it as an argument — so Epoch
/// passes it too. A ComfyUI that had forgotten a 6.9 GB checkpoint looks exactly like a broken
/// install.
/// Whether this studio has anything queued or running right now.
///
/// Asked of the server rather than remembered by Epoch: a picture can be queued from its own web
/// page, from a character's turn and from the panel, and a count kept here would be one of three
/// answers. `/queue` reports both lists, and an unreachable server has nothing running by
/// definition.
///
/// It exists so that closing a panel cannot stop a render that is halfway through — the one way
/// an automatic stop could take something away from somebody.
pub fn is_busy(at: &str) -> bool {
    let Ok(said) = ureq::builder()
        .timeout_connect(CONNECT)
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .get(&format!("{}/queue", at.trim_end_matches('/')))
        .call()
    else {
        return false;
    };
    let Ok(queue) = said.into_json::<serde_json::Value>() else {
        return false;
    };
    let counted = |name: &str| queue[name].as_array().map(Vec::len).unwrap_or_default();
    counted("queue_running") + counted("queue_pending") > 0
}

/// Stop the server this studio runs, and say what happened.
///
/// ## Why a process and not a request
///
/// Measured 2026-08-26 against a live ComfyUI: `POST /shutdown`, `/api/shutdown`, `/exit` and
/// `/quit` all answer **405**, which is what its static handler says to a route it does not have.
/// There is no way to ask it to stop, so the only honest option left is to stop the process —
/// and that puts the burden on identifying it exactly.
///
/// ## Identified by the install it is running from, never by name
///
/// `python.exe` is the most shared name on a machine. What is terminated is only a process whose
/// **command line contains this studio's own base directory** — the folder Epoch already located
/// to build the start command. Somebody else's Python, including another ComfyUI in another
/// folder, does not match and is not touched.
///
/// ## What it is for
///
/// Measured on the owner's machine, idle: ComfyUI holds **2.6 GB of system RAM** with 0.03 GB
/// reserved on the card, and `POST /free` returns none of it — 2609 MB before and after. What
/// stays is the interpreter, torch's CUDA context and the node modules, and none of that goes
/// while the process lives.
///
/// Measured through the button: 2610 MB before, **0 MB after**, and answering again 30 s after
/// START — a fresh one holds 959 MB, which is what it weighs before it has loaded anything.
pub fn stop_server(studio: Studio) -> Result<String, String> {
    let base = match studio {
        Studio::ComfyUi => crate::generative::comfyui_base()
            .ok_or_else(|| "Epoch cannot find that studio's folder on this machine".to_owned())?,
    };
    let here = base.display().to_string();

    #[cfg(windows)]
    let stopped = {
        // The same tool the machine reading already uses, asked for processes whose command line
        // names this install. `Stop-Process -Force` because a server with no shutdown route has
        // no polite ending to wait for.
        let script = format!(
            "$p = Get-CimInstance Win32_Process -Filter \"Name='python.exe'\" | \
             Where-Object {{ $_.CommandLine -like '*{}*' }}; \
             if ($p) {{ $p | ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force }}; \
             ($p | Measure-Object).Count }} else {{ 0 }}",
            here.replace('\'', "''")
        );
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
            .map_err(|why| format!("that could not be asked of this machine: {why}"))?
    };

    #[cfg(not(windows))]
    let stopped = {
        // `pkill -f` matches the whole command line, which is the same identification by install
        // the Windows branch makes. It prints nothing; the count comes from the exit status,
        // where 0 means something matched and 1 means nothing did.
        std::process::Command::new("pkill")
            .args(["-f", &here])
            .output()
            .map_err(|why| format!("that could not be asked of this machine: {why}"))?
    };

    // And the console it was running in, when Epoch is the one that opened it. `cmd /k` keeps a
    // window open so a failure can be read; a server Epoch shut down on purpose leaves nothing to
    // read, and an empty prompt in a window the user never opened is an immersion leak over a
    // World that is meant to be a place.
    crate::runtimes::close_our_terminals(&here);

    let said = String::from_utf8_lossy(&stopped.stdout);
    let count: usize = said
        .trim()
        .parse()
        .unwrap_or(usize::from(stopped.status.success()));
    if count == 0 {
        return Ok("Nothing was running from that folder.".to_owned());
    }
    Ok("Stopped. Its memory is back; starting it again takes about half a minute.".to_owned())
}

pub fn server_command(studio: Studio) -> Option<String> {
    match studio {
        Studio::ComfyUi => {
            let base = crate::generative::comfyui_base()?;
            // Joined a component at a time so the command does not mix separators. It runs
            // either way; a path a person has to read should not be half one thing.
            let python = [
                [".venv", "Scripts", "python.exe"],
                [".venv", "bin", "python"],
                [".venv", "bin", "python3"],
            ]
            .into_iter()
            .map(|parts| parts.iter().fold(base.clone(), |at, part| at.join(part)))
            .find(|python| python.is_file())?;

            // **The shell this line is handed to is not the same shell everywhere.**
            //
            // `cd /d` is `cmd.exe`'s, and it is *needed* there: without `/d`, `cd "D:\…"` from a
            // C: prompt changes the directory on D: and leaves the shell on C:, so `main.py` is
            // not found. On the owner's MacBook the identical line goes to `zsh` through
            // Terminal.app, which answered `zsh:cd:1: string not in pwd: /d` and never reached
            // the `&&` — measured 2026-08-25, and it is the whole of why ComfyUI would not start
            // from EpochServices on a Mac. The same command without `/d` had it serving in 25s.
            //
            // Written from `cfg!` rather than from a guess about the shell: the command is built
            // on the machine that will run it.
            let mut command = format!(
                "cd {}\"{}\" && \"{}\" main.py --port {}",
                if cfg!(windows) { "/d " } else { "" },
                base.display(),
                python.display(),
                studio.usual_port()
            );
            if let Some(paths) = desktop_model_paths() {
                command.push_str(&format!(
                    " --extra-model-paths-config \"{}\"",
                    paths.display()
                ));
            }
            Some(command)
        }
    }
}

/// The model-path config Comfy Desktop generated for its instance.
///
/// Read rather than written: the file says *do not edit manually*, and Epoch has its own
/// (`generative::offer_library_to_comfyui`). Handing this one back is what keeps the checkpoints
/// the user already had visible when Epoch starts the server itself.
fn desktop_model_paths() -> Option<std::path::PathBuf> {
    let dir = crate::generative::comfy_desktop_dir()?.join("instance-model-paths");
    // Newest first: an install that was replaced leaves its old file behind, and the instance in
    // use is the one most recently written.
    let mut kept: Vec<_> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|entry| entry.path().extension().and_then(|it| it.to_str()) == Some("yaml"))
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    kept.sort_by_key(|one| std::cmp::Reverse(one.0));
    kept.into_iter().next().map(|(_, path)| path)
}

/// What a ComfyUI says it can load.
///
/// `None` means it did not answer at all, which is a different fact from answering with nothing:
/// a ComfyUI with no checkpoint downloaded is running perfectly well and cannot make a picture,
/// and reporting that as offline would send somebody to start what is already started.
fn checkpoints_at(endpoint: &str) -> Option<Vec<String>> {
    let answer: serde_json::Value = ureq::builder()
        .timeout_connect(std::time::Duration::from_millis(400))
        .timeout(std::time::Duration::from_millis(1500))
        .build()
        .get(&format!("{endpoint}/object_info/CheckpointLoaderSimple"))
        .call()
        .ok()?
        .into_json()
        .ok()?;

    Some(read_checkpoints(&answer))
}

/// What one node input offers, asked of a server.
///
/// The general form of `checkpoints_at`, which stays as it is because it carries the *timeouts* a
/// survey needs — this one is asked deliberately, about one thing, and can afford to wait.
///
/// `None` means it did not answer; an empty list means it answered with nothing. Those are two
/// different facts and the panel says two different things about them.
pub fn offered_at(endpoint: &str, node: &str, input: &str) -> Option<Vec<String>> {
    Some(read_offered(&ask_node(endpoint, node)?, node, input))
}

/// One node's description, asked once.
///
/// **Separate from reading it, because a panel needs three answers from one node.** Asking three
/// times cost three round trips and, worse, three timeouts: measured 2026-08-24, the Studio Panel
/// took about thirty seconds to open because `CLIPLoader` was asked twice and `VAELoader` once,
/// each waiting up to ten. One ask, read three times.
pub fn ask_node(endpoint: &str, node: &str) -> Option<serde_json::Value> {
    ureq::builder()
        // **A closed port on this machine is not free, and it is not fast** (measured
        // 2026-08-28). One attempt to `127.0.0.1:8188` with nothing listening took **21 s** —
        // not the instant refusal loopback is supposed to give — and the overall timeout below
        // does not cover it, because it is the connect syscall rather than the request.
        //
        // The panel makes nine of these, so with the studio stopped it took **190 s to open**.
        // Invisible until now only because opening the panel used to start the server first.
        // `checkpoints_at` has carried this for as long as it has existed; this is the same
        // number, in the two places that were missing it.
        .timeout_connect(CONNECT)
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .get(&format!("{endpoint}/object_info/{node}"))
        .call()
        .ok()?
        .into_json()
        .ok()
}

/// The embeddings this server has, by the names it answers with.
///
/// **Its own endpoint, because an embedding is not loaded by a node.** Asked 2026-08-27: nothing
/// in `/object_info` takes one — the matches for the word are tensor-name prefixes on merge
/// nodes — and `GET /embeddings` answers a flat list. That is *why* it has an endpoint: the
/// editor needs the names for autocomplete, because the only way to use one is to write it into
/// the prompt.
///
/// The names come back **without extensions**: a file called `epoch-probe.safetensors` is
/// answered as `epoch-probe`, and that is the spelling the prompt needs.
pub fn embeddings_here(endpoint: &str) -> Vec<String> {
    ureq::builder()
        .timeout_connect(CONNECT)
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .get(&format!("{endpoint}/embeddings"))
        .call()
        .ok()
        .and_then(|answer| answer.into_json::<Vec<String>>().ok())
        .unwrap_or_default()
}

/// One input's choices, out of a node description already fetched.
pub fn read_offered(answer: &serde_json::Value, node: &str, input: &str) -> Vec<String> {
    let field = &answer[node]["input"]["required"][input];
    // **Two shapes, from one server, on the same afternoon.**
    //
    // Measured 2026-08-26 against this ComfyUI, which answers
    //
    //   CLIPLoader.clip_name         [["clip_l.safetensors", "t5xxl_fp8_e4m3fn.safetensors"]]
    //   UpscaleModelLoader.model_name ["COMBO", {"multiselect": false, "options": [...]}]
    //
    // The older form puts the choices first; the newer names the widget and hangs the choices in
    // its options. Reading only the first is why the upscale row silently did not appear the day
    // an upscaler was finally installed — the list came back empty and empty means *none here*.
    //
    // This is the rule this file already learnt one node over, arriving from the other side: a
    // vocabulary read once belongs to **that node**, not to the server. It is true of the shape
    // as well as of the contents.
    let listed = |value: &serde_json::Value| -> Option<Vec<String>> {
        Some(
            value
                .as_array()?
                .iter()
                .filter_map(|name| name.as_str())
                .map(str::to_owned)
                .collect(),
        )
    };
    listed(&field[0])
        .or_else(|| listed(&field[1]["options"]))
        .unwrap_or_default()
}

/// The names out of ComfyUI's node description.
///
/// Its `/object_info/<node>` reports each input as `[choices, options]`, so a checkpoint list is
/// the first element of the first input — a shape rather than a field, which is why it is parsed
/// here and held still by a test.
fn read_checkpoints(answer: &serde_json::Value) -> Vec<String> {
    answer["CheckpointLoaderSimple"]["input"]["required"]["ckpt_name"]
        .get(0)
        .and_then(|list| list.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|name| name.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod what_a_node_publishes {
    use super::*;

    /// Both shapes, copied from this ComfyUI's own answers rather than invented.
    #[test]
    fn a_list_is_read_whichever_of_the_two_shapes_the_node_uses() {
        let old_shape = serde_json::json!({
            "CLIPLoader": {
                "input": { "required": {
                    "clip_name": [["clip_l.safetensors", "t5xxl_fp8_e4m3fn.safetensors"]]
                }}
            }
        });
        assert_eq!(
            read_offered(&old_shape, "CLIPLoader", "clip_name"),
            vec![
                "clip_l.safetensors".to_owned(),
                "t5xxl_fp8_e4m3fn.safetensors".to_owned()
            ]
        );

        let new_shape = serde_json::json!({
            "UpscaleModelLoader": {
                "input": { "required": {
                    "model_name": ["COMBO", {
                        "multiselect": false,
                        "options": ["4xUltrasharp_4xUltrasharpV10.pt"]
                    }]
                }}
            }
        });
        assert_eq!(
            read_offered(&new_shape, "UpscaleModelLoader", "model_name"),
            vec!["4xUltrasharp_4xUltrasharpV10.pt".to_owned()],
            "the newer shape names the widget first and hangs the choices in its options"
        );
    }

    #[test]
    fn a_node_that_is_not_there_offers_nothing_rather_than_panicking() {
        // Empty is the honest answer for a server without the node, and it is what the surface
        // draws as *no control at all*.
        assert!(
            read_offered(&serde_json::json!({}), "UpscaleModelLoader", "model_name").is_empty()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_checkpoint_list_is_the_first_element_of_the_input() {
        // ComfyUI's own shape: `[[choices], {options}]`. Parsed rather than remembered, because
        // an empty list and a missing field mean different things, and only one of them is a
        // machine with no models.
        let answer = serde_json::json!({
            "CheckpointLoaderSimple": {
                "input": {
                    "required": { "ckpt_name": [["sd15.safetensors", "sdxl.safetensors"], {}] }
                }
            }
        });
        assert_eq!(
            read_checkpoints(&answer),
            vec!["sd15.safetensors", "sdxl.safetensors"]
        );
    }

    #[test]
    fn a_studio_with_no_checkpoints_is_running_and_says_so() {
        // A fresh install: the server answers and the list is empty. That is *serving with
        // nothing* — the distinction LM Studio forced on the runtimes panel — and it must not
        // read as offline.
        let answer = serde_json::json!({
            "CheckpointLoaderSimple": { "input": { "required": { "ckpt_name": [[], {}] } } }
        });
        assert!(read_checkpoints(&answer).is_empty());
    }

    #[test]
    fn an_answer_in_a_shape_we_do_not_know_teaches_nothing() {
        assert!(read_checkpoints(&serde_json::json!({ "something": "else" })).is_empty());
    }

    /// What this machine actually has, printed.
    ///
    /// `#[ignore]` because the answer is a fact about one computer and not about the code —
    /// but it is the only thing that proves the search roots are right, and the roots were
    /// wrong twice for the runtimes before they were measured.
    #[test]
    #[ignore = "prints what this machine has"]
    fn what_is_here() {
        for easel in survey() {
            println!(
                "{} installed={} at={:?} serving={} models={:?}",
                easel.name, easel.installed, easel.found_at, easel.serving, easel.models
            );
            // The distinction this file exists to make. `starts_server=false` means pressing
            // START opens somebody else's window and nothing is serving afterwards.
            println!(
                "  startsServer={} start={:?}",
                easel.starts_server, easel.start
            );
            println!("  says: {}", easel.first_run);
        }
    }

    /// A start that only opens a dashboard must never be described as starting a server.
    ///
    /// The two are told apart by a field rather than by a sentence, because the sentence was the
    /// bug: START claimed to start ComfyUI, ComfyUI showed a list of instances, and the panel had
    /// no way to say so.
    #[test]
    fn what_starts_the_server_and_what_only_opens_a_window_say_different_things() {
        // Held on the strings rather than on this machine, so it is true on a laptop with no
        // ComfyUI at all.
        assert_ne!(Studio::ComfyUi.first_run(), Studio::ComfyUi.front_door());
        assert!(
            Studio::ComfyUi.front_door().contains("ASK AGAIN"),
            "the front door has to say what to do next: {}",
            Studio::ComfyUi.front_door()
        );
    }

    /// The command has to be written in the shell that will read it.
    ///
    /// **The defect this closes, measured on the owner's MacBook 2026-08-25:** the line began
    /// `cd /d "..."` on every platform. That is `cmd.exe`'s switch. Terminal.app hands it to
    /// `zsh`, which answered `zsh:cd:1: string not in pwd: /d` and stopped — so pressing START in
    /// EpochServices opened a terminal, printed one error, and never reached `main.py`. The same
    /// command with `/d` removed had ComfyUI answering on 8188 in twenty-five seconds.
    ///
    /// And `/d` is not decoration on Windows: without it, `cd "D:\..."` from a C: prompt moves
    /// the directory on D: and leaves the shell on C:, and `main.py` is not found. Both are true;
    /// they are simply not true of the same shell.
    ///
    /// Not ignored, and it needs no install: the shape of the line is the thing under test.
    #[test]
    fn the_directory_is_changed_in_the_words_of_this_platform_shell() {
        let Some(command) = server_command(Studio::ComfyUi) else {
            // Nothing installed here. The assertions below are about a line that does not exist.
            return;
        };
        if cfg!(windows) {
            assert!(
                command.starts_with("cd /d \""),
                "cmd.exe needs /d or it changes directory without changing drive: {command}"
            );
        } else {
            assert!(
                !command.contains("cd /d"),
                "a POSIX shell reads /d as a second argument and stops: {command}"
            );
            assert!(command.starts_with("cd \""), "{command}");
        }
    }

    /// The user's own checkpoints must survive Epoch starting the server.
    ///
    /// **Measured, and it is the whole reason `--extra-model-paths-config` is passed**: running
    /// `main.py` alone listed only Epoch's library and none of the two checkpoints already on
    /// this machine. Ignored because it needs a real Comfy Desktop install to say anything.
    #[test]
    #[ignore = "needs a Comfy Desktop install on this machine"]
    fn the_server_command_keeps_the_checkpoints_the_user_already_had() {
        let command = server_command(Studio::ComfyUi).expect("a ComfyUI install");
        println!("{command}");
        assert!(command.contains("main.py"), "{command}");
        assert!(command.contains("--port 8188"), "{command}");
        // And it must not repeat the dashboard's wizard, which starting the server never shows.
        assert_eq!(
            look_for(Studio::ComfyUi).first_run,
            Studio::ComfyUi.starting()
        );
        assert!(
            command.contains("--extra-model-paths-config"),
            "without this, a 6.9 GB checkpoint disappears and it looks like a broken install: \
             {command}"
        );
    }

    #[test]
    fn the_mac_command_names_the_cask_homebrew_actually_has() {
        // `comfyui` was written from memory and is a 404. The real token is `comfy` — one match
        // in 7,701 casks, measured 2026-08-22 against `formulae.brew.sh/api/cask.json`.
        //
        // Asserted on the string rather than on a request: a test that reached the network would
        // fail on a train, and what is being held still is that nobody edits this back to a
        // plausible-looking name.
        if !cfg!(windows) {
            assert_eq!(
                Studio::ComfyUi.install_command(),
                "brew install --cask comfy"
            );
        }
    }

    #[test]
    fn the_install_command_names_the_package_winget_actually_has() {
        // `winget search comfy` on 2026-08-21 returned exactly one desktop package, under this
        // id. The assertion is on the id rather than on a run, because running it installs.
        if cfg!(windows) {
            assert!(Studio::ComfyUi
                .install_command()
                .contains("Comfy.ComfyUI-Desktop"));
        }
    }
}
