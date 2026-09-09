//! **An interoperability experiment, not a guarantee.**
//!
//! One question about Codex, asked of Codex:
//!
//! > Can an `untrusted` app-server session invoke MCP tools that Codex does not own?
//!
//! It matters because the answer decides whether Epoch's Manual mode can ever offer the crew,
//! the Quest and this World's connected servers to a Codex character — and because three
//! candidate causes on Epoch's side were already eliminated one at a time, leaving a question
//! that is not Epoch's to answer.
//!
//! ## Why this does not go through `Agent::work`
//!
//! Because that path carries a confound that would invalidate the result. Epoch's persona
//! changes with the mode: a Manual turn is also told *use your own tools for files, search and
//! commands rather than an Epoch tool*. A Manual run that never calls an outside tool would then
//! have two possible explanations, and no way to tell them apart.
//!
//! So this speaks the app-server protocol directly, sends the **same instructions in both arms**,
//! and varies exactly one field: `approvalPolicy`. Everything else — the server, the tool, the
//! prompt, the sandbox, the model — is held identical.
//!
//! ## Where it stands: the control does not hold yet, and that is the finding
//!
//! Both arms report **zero** `tools/call`, including `approvalPolicy: never` — which is known to
//! work through `Agent::work`. So this probe is not yet measuring Codex; it is measuring its own
//! client, and no conclusion about `untrusted` may be drawn from it. The control assertion exists
//! to say exactly that rather than let a tidy-looking zero become an answer.
//!
//! What the transcript shows is a specific and useful lead. The session is healthy — the probe
//! server reaches `ready`, the turn starts — and among the messages the app-server sends is:
//!
//! ```text
//! "method":"mcpServer/elicitation/request"
//! ```
//!
//! The app-server relays an MCP server's *elicitation* to whoever is driving it, and this probe
//! never answers. Codex's own MCP client advertises the capability it corresponds to —
//! `"capabilities":{"elicitation":{"form":{},"url":{}}}`, seen arriving at Epoch's door — and
//! neither this probe nor Epoch's app-server client declares or answers it.
//!
//! That is the next thing to establish, and it is a question about the protocol rather than about
//! either arm: **does a tool call wait on an elicitation nobody answers?** If it does, the
//! contract that enables outside MCP tools is elicitation support, not the approval policy — and
//! the Manual question has been aimed at the wrong field the whole time.
//!
//! ## What it can and cannot prove
//!
//! A `tools/call` arriving is proof that it *can* happen. A `tools/call` never arriving is
//! evidence and not proof: a model may simply have chosen not to. That asymmetry is why the
//! result is reported rather than asserted into a passing test, and why the run prints its
//! transcript.
//!
//! ```text
//! cargo test -p epoch-engine --test codex_untrusted_mcp_probe -- --ignored --nocapture
//! ```

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use epoch_engine::endpoint;
use serde_json::{json, Value};

/// How long one arm may take before it is called silent.
const PATIENCE: Duration = Duration::from_secs(90);

/// A minimal MCP server that offers exactly one tool and counts what is asked of it.
///
/// Deliberately **not** Epoch's `Serving`: the question is about Codex's behaviour toward an
/// outside server, so the outside server should be as plain as one can be.
struct Counting {
    calls: Arc<AtomicUsize>,
}

impl endpoint::Answering for Counting {
    fn answer(&self, message: &Value) -> Option<Value> {
        let id = message.get("id").cloned();
        let method = message.get("method").and_then(Value::as_str)?;
        let reply = |result: Value| Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }));

        match method {
            "initialize" => reply(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "probe", "version": "0" },
            })),
            "notifications/initialized" => None,
            "tools/list" => reply(json!({
                "tools": [{
                    "name": "probe_watchword",
                    "description": "Returns the watchword. Takes no arguments.",
                    "inputSchema": { "type": "object", "properties": {} },
                }],
            })),
            "tools/call" => {
                self.calls.fetch_add(1, Ordering::SeqCst);
                reply(json!({
                    "content": [{ "type": "text", "text": "zarpaste" }],
                    "isError": false,
                }))
            }
            _ => reply(json!({})),
        }
    }
}

/// The Codex to run, preferring an install that is complete — the same rule `agents::codex`
/// follows, and for the same reason it was written: the copy on `PATH` may be missing the helper
/// every command needs.
fn codex() -> Option<PathBuf> {
    let bin = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("OpenAI/Codex/bin");
    let mut best: Option<(bool, std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(bin).ok()?.flatten() {
        let exe = entry.path().join("codex.exe");
        if !exe.is_file() {
            continue;
        }
        let whole = entry
            .path()
            .join("codex-windows-sandbox-setup.exe")
            .is_file();
        let when = entry.metadata().ok()?.modified().ok()?;
        if best
            .as_ref()
            .is_none_or(|(w, t, _)| (whole, when) > (*w, *t))
        {
            best = Some((whole, when, exe));
        }
    }
    best.map(|(_, _, exe)| exe)
}

/// What one arm of the experiment observed.
struct Arm {
    /// How many `tools/call` messages the outside server received.
    calls: usize,
    /// Everything the app-server said, for reading when a number is not enough.
    transcript: String,
}

/// Run one turn at the given approval policy and report what the outside server saw.
///
/// The two arms differ in `approvalPolicy` and in nothing else. `instructions` is sent
/// identically rather than omitted, because absent and present-but-neutral are different
/// conditions and only one of them is a control.
fn run(policy: &str) -> Arm {
    let program = codex().expect("Codex is installed");
    let calls = Arc::new(AtomicUsize::new(0));
    let door = endpoint::open(
        0,
        endpoint::Token::fresh(),
        Counting {
            calls: Arc::clone(&calls),
        },
    )
    .expect("the probe server listens on loopback");

    let mut child = Command::new(&program)
        .arg("app-server")
        .arg("-c")
        .arg("instructions=\"You are a careful assistant.\"")
        .arg("-c")
        .arg(format!("mcp_servers.probe.url=\"{}\"", door.url()))
        .arg("-c")
        .arg("mcp_servers.probe.bearer_token_env_var=\"PROBE_TOKEN\"")
        .env("PROBE_TOKEN", door.token().as_str())
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Codex starts");

    let mut input = child.stdin.take().expect("stdin");

    // Read on its own thread, and take lines through a channel with a deadline.
    //
    // The first version walked `lines()` directly and checked the clock inside the loop, which
    // cannot fire: a blocking read that never returns never reaches the check. The run hung
    // until something outside killed it. Exactly the reason `mcp.rs` bounds its reader the same
    // way — a child that stops answering must end the wait rather than own it.
    let (lines, arriving) = mpsc::channel();
    let stdout = BufReader::new(child.stdout.take().expect("stdout"));
    std::thread::spawn(move || {
        for line in stdout.lines().map_while(Result::ok) {
            if lines.send(line).is_err() {
                return;
            }
        }
    });

    let send = |input: &mut dyn Write, id: Option<u64>, method: &str, params: Value| {
        let mut message = json!({ "method": method, "params": params });
        if let Some(id) = id {
            message["id"] = json!(id);
        }
        let _ = serde_json::to_writer(&mut *input, &message);
        let _ = input.write_all(b"\n");
        let _ = input.flush();
    };

    // `experimentalApi` is not decoration: `thread/start` and `turn/start` live behind it. The
    // first version of this probe omitted it, and the session simply stopped answering after
    // `initialize` — which the control arm caught by measuring nothing, exactly as intended.
    send(
        &mut input,
        Some(1),
        "initialize",
        json!({
            "clientInfo": { "name": "epoch-probe", "version": "0" },
            "capabilities": { "experimentalApi": true },
        }),
    );

    let mut transcript = String::new();
    let started = Instant::now();
    let mut thread: Option<String> = None;
    let mut turn_started = false;

    loop {
        let Some(left) = PATIENCE.checked_sub(started.elapsed()) else {
            transcript.push_str("[probe] gave up waiting\n");
            break;
        };
        let Ok(line) = arriving.recv_timeout(left) else {
            transcript.push_str("[probe] nothing more arrived\n");
            break;
        };
        transcript.push_str(&line);
        transcript.push('\n');
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };

        match message.get("id").and_then(Value::as_u64) {
            Some(1) if thread.is_none() => {
                send(&mut input, None, "initialized", json!({}));
                send(
                    &mut input,
                    Some(2),
                    "thread/start",
                    json!({ "cwd": std::env::temp_dir(), "model": Value::Null }),
                );
            }
            Some(2) if !turn_started => {
                let Some(id) = message
                    .pointer("/result/thread/id")
                    .or_else(|| message.pointer("/result/id"))
                    .and_then(Value::as_str)
                else {
                    break;
                };
                thread = Some(id.to_owned());
                turn_started = true;
                send(
                    &mut input,
                    Some(3),
                    "turn/start",
                    json!({
                        "threadId": id,
                        "input": [{
                            "type": "text",
                            "text": "Call the probe_watchword tool from the probe MCP server and \
                                     reply with only the word it returns. Do not guess it.",
                        }],
                        "cwd": std::env::temp_dir(),
                        "model": Value::Null,
                        // The one field under test.
                        "approvalPolicy": policy,
                        "approvalsReviewer": "user",
                        "sandboxPolicy": {
                            "type": "workspaceWrite",
                            "writableRoots": [std::env::temp_dir()],
                            "networkAccess": true,
                        },
                    }),
                );
            }
            _ => {}
        }

        // **This is the approval for an outside MCP tool call**, and it arrives as an
        // elicitation rather than as one of the `requestApproval` methods:
        //
        //     {"method":"mcpServer/elicitation/request","id":0,"params":{
        //        "serverName":"probe","mode":"form",
        //        "_meta":{"codex_approval_kind":"mcp_tool_call","persist":["session","always"]},
        //        "message":"Allow the probe MCP server to run tool \"probe_watchword\"?"}}
        //
        // Answered here so the arms measure the approval policy rather than this probe's silence.
        if message.get("method").and_then(Value::as_str) == Some("mcpServer/elicitation/request") {
            if let Some(id) = message.get("id").cloned() {
                let _ = serde_json::to_writer(
                    &mut input,
                    &json!({ "id": id, "result": { "action": "accept", "content": {} } }),
                );
                let _ = input.write_all(b"\n");
                let _ = input.flush();
                transcript.push_str("[probe] accepted the elicitation\n");
            }
        }

        if message.get("method").and_then(Value::as_str) == Some("turn/completed")
            || message.pointer("/error").is_some()
        {
            break;
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    Arm {
        calls: calls.load(Ordering::SeqCst),
        transcript,
    }
}

#[test]
#[ignore = "an interoperability experiment: needs a signed-in Codex and spends plan allowance"]
fn does_an_untrusted_session_call_an_outside_mcp_tool() {
    let never = run("never");
    let untrusted = run("untrusted");

    println!(
        "approvalPolicy=never      tools/call received: {}",
        never.calls
    );
    println!(
        "approvalPolicy=untrusted  tools/call received: {}",
        untrusted.calls
    );

    // The control has to hold, or the arms are not comparable and neither number means anything.
    assert!(
        never.calls > 0,
        "the control did not call the tool either, so this experiment measured nothing.\n\
         --- transcript ---\n{}",
        never.transcript
    );

    if untrusted.calls > 0 {
        println!(
            "\nRESULT: an untrusted session DOES call an outside MCP tool. \
             Manual can be given Epoch's door; find the contract that made the earlier runs \
             silent."
        );
    } else {
        println!(
            "\nRESULT: an untrusted session did NOT call an outside MCP tool, with Epoch's \
             persona removed and only approvalPolicy varied. Evidence, not proof — a model may \
             decline. Recorded as a limitation of the Codex app-server integration.\n\
             --- untrusted transcript ---\n{}",
            untrusted.transcript
        );
    }
}
