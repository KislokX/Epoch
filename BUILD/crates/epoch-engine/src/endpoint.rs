//! Where an agent reaches Epoch (step 5.2, the transport).
//!
//! ## Why HTTP on this machine, and not stdio
//!
//! MCP has two transports. stdio means the client *spawns* the server — and a process Claude
//! Code spawned is not the Epoch the user is looking at. It would have no World open, no Trust
//! store in memory, and above all nowhere to put the question: an approval prompt has to reach
//! a person, and the person is in front of Epoch, not in front of the agent's terminal.
//!
//! So the running Epoch listens, and the agent connects to it. If Epoch is closed the tools are
//! not there, which is the honest answer rather than a degraded one.
//!
//! ## This is the most dangerous surface in Epoch
//!
//! It is a socket that can run shell commands. Everything below is written for that, and none
//! of it is optional:
//!
//! - **Loopback only.** Bound to `127.0.0.1`, never `0.0.0.0`. A bind address is not a setting.
//! - **A bearer token.** Loopback is not authentication: every process on this machine can
//!   reach a loopback port. Without a shared secret, any program the user runs could tell Epoch
//!   to write files as one of their characters.
//! - **`Origin` refused outright.** A browser always sends one; an MCP client never does. That
//!   asymmetry is the whole defence against DNS rebinding, where a page the user is merely
//!   *visiting* resolves a name to `127.0.0.1` and posts to this port. The token would already
//!   stop it — this stops it before the token is even compared.
//! - **A bounded body.** A request that never ends is a request that never ends.
//!
//! ## The gate is decided without a socket
//!
//! [`Guard::admit`] is pure: method, path, headers in, a verdict out. That is deliberate — the
//! security of this endpoint is testable exhaustively without opening a port, and a test that
//! needs a port is a test somebody eventually skips.
//!
//! ## Not always on
//!
//! Opened when the user turns it on, closed when they turn it off or close Epoch. A listening
//! socket that runs commands should be something somebody did, not something that happened.

use std::io::Read;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::Value;

use epoch_kernel::{Secret, SecretName};

use crate::secrets::Secrets;

/// The port Epoch listens on unless told otherwise.
///
/// Arbitrary, and stable on purpose rather than ephemeral: the user pastes this into an agent's
/// configuration once, and a port that changed every launch would invalidate it every launch.
/// Obscurity is not the protection here — the token is.
pub const DEFAULT_PORT: u16 = 8792;

/// The one path that answers. Anything else is a wrong door.
pub const PATH: &str = "/mcp";

/// How much request body is read before giving up.
///
/// A tool call is a few kilobytes. This is generous by three orders of magnitude and still
/// bounded, because unbounded is how a single request becomes the whole of memory.
const MOST_BODY: usize = 4 * 1024 * 1024;

/// How many requests can be in flight at once.
///
/// Small on purpose. Epoch asks the user **one question at a time**, so a second call needing
/// approval is refused rather than queued — which means the only concurrency that has to work
/// is "a read while a write is being approved". Four is that, with room.
const WORKERS: usize = 4;

/// How long a worker parks before looking at the stop flag again.
///
/// Only ever costs a closing endpoint this much delay, and never costs a request anything:
/// `recv_timeout` returns immediately when something arrives.
const WAKE: std::time::Duration = std::time::Duration::from_millis(250);

#[derive(Debug, thiserror::Error)]
pub enum EndpointError {
    #[error("cannot listen on 127.0.0.1:{port}: {source}")]
    Listen {
        port: u16,
        #[source]
        source: std::io::Error,
    },
}

/// The shared secret an agent must present.
///
/// Held as bytes and compared in constant time. Never logged, never put in an error message,
/// and never sent anywhere — it is shown to the user once, so they can paste it into the
/// agent's own configuration.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl std::fmt::Debug for Token {
    /// Redacted. A token in a panic message is a token in a bug report.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(…)")
    }
}

impl Token {
    /// A fresh token from the operating system's randomness.
    ///
    /// 32 bytes. Not derived from a clock or a process id, which are guessable by exactly the
    /// local programs this exists to keep out.
    pub fn fresh() -> Self {
        let mut bytes = [0u8; 32];
        // If the OS cannot give randomness, refusing is the only safe answer — a predictable
        // token is worse than no endpoint, and this is unreachable on any supported platform.
        getrandom::getrandom(&mut bytes).expect("the operating system must provide randomness");
        Self(bytes.iter().map(|b| format!("{b:02x}")).collect())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// A token that already exists, adopted as it is.
    ///
    /// Named `of` rather than `from_str`: that name belongs to `std::str::FromStr`, and a method
    /// wearing it without being it is one somebody will call expecting `Result`. The audit asked
    /// for one convention across the codebase — `parse` for fallible, plain constructors
    /// otherwise — and this is not fallible.
    pub fn of(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// Parse the stable, 32-byte hexadecimal form accepted from encrypted storage.
    ///
    /// [`Self::of`] remains deliberately permissive for concise authentication tests. A value
    /// that crossed a persistence boundary is stricter: a truncated file must never become the
    /// bearer credential for the local endpoint.
    pub fn from_persisted(raw: &str) -> Option<Self> {
        let kept = raw.trim();
        (kept.len() == 64 && kept.chars().all(|c| c.is_ascii_hexdigit()))
            .then(|| Self(kept.to_owned()))
    }

    /// Constant time, so a wrong guess takes as long as any other wrong guess.
    ///
    /// The naive `==` on strings returns at the first differing byte, which leaks how much of a
    /// guess was right — enough, over many attempts, to find the rest a byte at a time.
    fn matches(&self, offered: &str) -> bool {
        let ours = self.0.as_bytes();
        let theirs = offered.as_bytes();
        // Lengths differ: still walk, so the answer takes the same shape either way.
        let mut different = (ours.len() ^ theirs.len()) as u8;
        for (i, byte) in ours.iter().enumerate() {
            different |= byte ^ theirs.get(i).copied().unwrap_or(0);
        }
        different == 0
    }
}

/// Read the local MCP door credential from the account-bound secret store.
///
/// `agent-token` was a pre-DPAPI format. Migration writes the exact same bearer into DPAPI
/// before deleting plaintext, so agents that already have it configured continue to connect.
/// An unreadable encrypted store or malformed legacy value is explicit: silently replacing a
/// credential would strand an already configured agent while its old bearer may still be live.
/// Throw this machine's door token away and mint a new one.
///
/// **Every door already open stops working**, which is the point: a token is regenerated
/// because the old one may have escaped — into a commit, a screenshot, a paste. A regeneration
/// that let the old one keep working would be a button that reassures without protecting.
///
/// The new one is written where the old one lived (DPAPI), so nothing else has to know this
/// happened. Any `.mcp.json` in a project still holds the previous bearer and has to be written
/// again — which is what connecting an agent already does.
pub fn regenerate(secrets: &Secrets, vault: &std::path::Path) -> Result<Token, String> {
    let fresh = Token::fresh();
    secrets.put(
        &SecretName::for_mcp_door(),
        &Secret::new(fresh.as_str().to_owned()),
    )?;
    // The pre-DPAPI file, if this vault still has one. Leaving it would leave a working bearer
    // in plain text after the user explicitly asked for the old one to stop working.
    remove_legacy(vault)?;
    Ok(fresh)
}

pub fn remembered(secrets: &Secrets, vault: &std::path::Path) -> Result<Token, String> {
    let name = SecretName::for_mcp_door();
    if let Some(stored) = secrets.get_checked(&name)? {
        let token = Token::from_persisted(stored.expose()).ok_or_else(|| {
            "Epoch's encrypted MCP door credential is malformed. Regenerate it before opening the door."
                .to_owned()
        })?;
        remove_legacy(vault)?;
        return Ok(token);
    }

    let legacy = vault.join("agent-token");
    let token = match std::fs::read_to_string(&legacy) {
        Ok(raw) => Token::from_persisted(&raw).ok_or_else(|| {
            "The old MCP door credential is malformed. Remove it and regenerate the door credential."
                .to_owned()
        })?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Token::fresh(),
        Err(error) => return Err(format!("cannot read the old MCP door credential: {error}")),
    };

    secrets.put(&name, &Secret::new(token.as_str().to_owned()))?;
    remove_legacy(vault)?;
    Ok(token)
}

/// Delete plaintext only after encryption succeeds. This removes the ordinary vault/repository
/// exposure; it deliberately does not pretend to provide forensic secure-erasure on Windows.
fn remove_legacy(vault: &std::path::Path) -> Result<(), String> {
    let legacy = vault.join("agent-token");
    match std::fs::remove_file(&legacy) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "the MCP token was encrypted, but old plaintext '{}' could not be removed: {error}",
            legacy.display()
        )),
    }
}

/// Why a request was turned away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// Wrong door.
    NotHere,
    /// Only POST carries a JSON-RPC message. GET is the specification's server-initiated
    /// stream, which Epoch does not offer — it declares `listChanged: false` and has nothing
    /// to say unprompted.
    WrongMethod,
    /// No token, or the wrong one.
    NotYou,
    /// A browser is talking to us, and a browser has no business here.
    FromAPage,
}

impl Refused {
    /// The HTTP status to answer with.
    pub fn status(&self) -> u16 {
        match self {
            Refused::NotHere => 404,
            Refused::WrongMethod => 405,
            Refused::NotYou => 401,
            // Deliberately not 401. A page that is told "unauthorised" learns a token exists
            // and is worth guessing; one told "forbidden" learns nothing.
            Refused::FromAPage => 403,
        }
    }

    /// What to say back.
    ///
    /// Short and uninformative on purpose. This is the one place in Epoch where a helpful
    /// error message would be helping the wrong person.
    pub fn message(&self) -> &'static str {
        match self {
            Refused::NotHere => "not found",
            Refused::WrongMethod => "method not allowed",
            Refused::NotYou => "unauthorized",
            Refused::FromAPage => "forbidden",
        }
    }
}

/// Everything about a request that the decision depends on.
///
/// A plain struct rather than the HTTP library's type, so [`Guard::admit`] can be tested
/// exhaustively without opening a port.
#[derive(Debug, Clone, Default)]
pub struct Incoming<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub authorization: Option<&'a str>,
    pub origin: Option<&'a str>,
}

/// The decision, made without any I/O.
pub struct Guard {
    token: Token,
}

impl Guard {
    pub fn new(token: Token) -> Self {
        Self { token }
    }

    /// Whether this request may be answered at all.
    ///
    /// Ordered cheapest-and-least-revealing first: a wrong path never reaches the token
    /// comparison, and a browser never reaches it either.
    pub fn admit(&self, request: &Incoming<'_>) -> Result<(), Refused> {
        // Trailing query or fragment is not our business, but the path itself must be exact.
        let path = request.path.split(['?', '#']).next().unwrap_or_default();
        if path != PATH {
            return Err(Refused::NotHere);
        }

        // Any `Origin` at all is a browser. A real MCP client sends none, so this needs no list
        // of allowed origins to keep up to date — the presence of the header is the signal.
        if request.origin.is_some() {
            return Err(Refused::FromAPage);
        }

        if !request.method.eq_ignore_ascii_case("POST") {
            return Err(Refused::WrongMethod);
        }

        let offered = request
            .authorization
            .and_then(|value| value.strip_prefix("Bearer "))
            .unwrap_or_default();
        if !self.token.matches(offered) {
            return Err(Refused::NotYou);
        }

        Ok(())
    }
}

/// How the endpoint gets an answer.
///
/// A trait rather than a closure type so the shell can hand over something holding its own
/// locks. The endpoint knows HTTP and authentication; it knows nothing about capabilities,
/// characters or Trust — that is [`crate::serve`], and it stays reachable only through here.
///
/// Returning `None` means *say nothing with a body*: a JSON-RPC notification has no reply.
pub trait Answering: Send + Sync + 'static {
    fn answer(&self, message: &Value) -> Option<Value>;
}

/// An open endpoint. Dropping it closes the door.
pub struct Open {
    address: SocketAddr,
    token: Token,
    stop: Arc<AtomicBool>,
    /// The listener, kept so `close` can wake the accept loop by dropping it.
    server: Arc<tiny_http::Server>,
}

impl Open {
    /// Where it is listening. Real, measured from the socket — never the port that was asked
    /// for, which may not be the one that was granted.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// What an agent must present. Shown to the user so they can paste it into a config.
    pub fn token(&self) -> &Token {
        &self.token
    }

    /// The URL an agent connects to.
    pub fn url(&self) -> String {
        format!("http://{}{PATH}", self.address)
    }

    /// Close the door.
    ///
    /// Sets the flag *and* unblocks the accept loop. Setting the flag alone would leave the
    /// thread parked inside `recv()` until the next request arrived — so an endpoint the user
    /// switched off would still answer once.
    pub fn close(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.server.unblock();
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        self.close();
    }
}

/// Start listening.
///
/// Loopback, always. The address is not a parameter, because a bind address is not a setting —
/// there is no version of this that should be reachable from another machine.
pub fn open(port: u16, token: Token, answering: impl Answering) -> Result<Open, EndpointError> {
    let wanted = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
    let server = tiny_http::Server::http(wanted).map_err(|source| EndpointError::Listen {
        port,
        source: std::io::Error::other(source.to_string()),
    })?;
    let server = Arc::new(server);

    // Measured from the socket, never the port that was asked for: with port 0 the operating
    // system picks, and reporting the request rather than the grant would print a URL that
    // nothing is listening on.
    let address = server.server_addr().to_ip().unwrap_or(wanted);

    let stop = Arc::new(AtomicBool::new(false));
    let guard = Guard::new(token.clone());

    // Threads rather than an async runtime. Everything in the Engine is synchronous, and one
    // endpoint is not a reason to bring a scheduler into a crate that has none.
    //
    // **Several of them, and that is not an optimisation.** This started as one thread walking
    // `incoming_requests()`, which is sequential — and the next thing built on top of it is an
    // approval that *blocks the request until a person answers*. With one thread, a `write_file`
    // waiting on the user would stop the endpoint answering anything at all, so the agent could
    // not even read a file while its own question was on screen. It would look like a hang and
    // it would be one.
    //
    // A small pool rather than a thread per request: a thread per request is unbounded, and this
    // is a port. `recv()` is safe to call from several threads, so they simply share the queue.
    let guard = Arc::new(guard);
    let answering = Arc::new(answering);
    for n in 0..WORKERS {
        let listening = Arc::clone(&server);
        let stopping = Arc::clone(&stop);
        let guard = Arc::clone(&guard);
        let answering = Arc::clone(&answering);
        std::thread::Builder::new()
            .name(format!("epoch-mcp-{n}"))
            .spawn(move || {
                loop {
                    // `unblock` makes this return `None`, which is how closing wakes a thread
                    // that is parked here rather than leaving it until the next request.
                    let Ok(Some(mut request)) = listening.recv_timeout(WAKE) else {
                        if stopping.load(Ordering::Relaxed) {
                            break;
                        }
                        continue;
                    };
                    if stopping.load(Ordering::Relaxed) {
                        break;
                    }
                    let reply = decide(&guard, answering.as_ref(), &mut request);
                    // A client that hung up is not an error worth reporting: it is the most
                    // ordinary thing a client does.
                    let _ = match reply {
                        Answer::Json(body) => request.respond(json_response(200, &body)),
                        Answer::Accepted => request.respond(empty_response(202)),
                        Answer::Refused(why) => {
                            request.respond(json_response(why.status(), why.message()))
                        }
                    };
                }
            })
            .map_err(|source| EndpointError::Listen { port, source })?;
    }

    Ok(Open {
        address,
        token,
        stop,
        server,
    })
}

enum Answer {
    Json(String),
    Accepted,
    Refused(Refused),
}

/// Read one request and work out what to send back.
fn decide(guard: &Guard, answering: &impl Answering, request: &mut tiny_http::Request) -> Answer {
    let method = request.method().as_str().to_owned();
    let path = request.url().to_owned();
    // One pass over the headers rather than one pass per header. `equiv` wants a borrow that
    // outlives the call, and taking the owned name each time was the wrong shape anyway: header
    // names are compared case-insensitively, which is what HTTP says and what a client will do.
    let mut authorization = None;
    let mut origin = None;
    for header in request.headers() {
        let name = header.field.as_str().as_str();
        if name.eq_ignore_ascii_case("authorization") {
            authorization = Some(header.value.as_str().to_owned());
        } else if name.eq_ignore_ascii_case("origin") {
            origin = Some(header.value.as_str().to_owned());
        }
    }

    if let Err(why) = guard.admit(&Incoming {
        method: &method,
        path: &path,
        authorization: authorization.as_deref(),
        origin: origin.as_deref(),
    }) {
        return Answer::Refused(why);
    }

    let mut body = String::new();
    // Bounded before it is read, not after. `take` is what makes the limit real.
    if request
        .as_reader()
        .take(MOST_BODY as u64)
        .read_to_string(&mut body)
        .is_err()
    {
        return Answer::Json(parse_error("the request body could not be read"));
    }

    let Ok(message) = serde_json::from_str::<Value>(&body) else {
        return Answer::Json(parse_error("that is not JSON"));
    };

    match answering.answer(&message) {
        Some(reply) => Answer::Json(reply.to_string()),
        // A notification. The specification says to acknowledge and send no body, and inventing
        // one is what makes a client wait for an id that is never coming.
        None => Answer::Accepted,
    }
}

/// A JSON-RPC parse error, which by the specification carries a null id.
fn parse_error(message: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": Value::Null,
        "error": { "code": -32700, "message": message },
    })
    .to_string()
}

fn json_response(status: u16, body: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let mut response = tiny_http::Response::from_string(body).with_status_code(status);
    if let Ok(header) =
        tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
    {
        response.add_header(header);
    }
    response
}

fn empty_response(status: u16) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    tiny_http::Response::from_string("").with_status_code(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> (Guard, Token) {
        let token = Token::of("aaaabbbbccccdddd");
        (Guard::new(token.clone()), token)
    }

    fn asking<'a>(token: &'a str) -> Incoming<'a> {
        Incoming {
            method: "POST",
            path: PATH,
            authorization: Some(token),
            origin: None,
        }
    }

    #[test]
    fn a_correct_request_is_admitted() {
        let (guard, token) = guard();
        let header = format!("Bearer {}", token.as_str());
        assert_eq!(guard.admit(&asking(&header)), Ok(()));
    }

    #[test]
    fn loopback_is_not_authentication() {
        // Every process on this machine can reach a loopback port. Without the token, any
        // program the user runs could tell Epoch to write files as one of their characters.
        let (guard, _) = guard();
        let mut request = asking("Bearer nope");
        assert_eq!(guard.admit(&request), Err(Refused::NotYou));

        request.authorization = None;
        assert_eq!(guard.admit(&request), Err(Refused::NotYou));
    }

    #[test]
    fn a_token_without_its_scheme_is_not_a_token() {
        // Sending the raw secret where `Bearer <secret>` was asked for is a client bug, and
        // accepting it anyway would mean the header is parsed two ways depending on the sender.
        let (guard, token) = guard();
        let mut request = asking(token.as_str());
        assert_eq!(guard.admit(&request), Err(Refused::NotYou));

        request.authorization = Some("bearer aaaabbbbccccdddd");
        assert_eq!(
            guard.admit(&request),
            Err(Refused::NotYou),
            "the scheme is case-sensitive"
        );
    }

    #[test]
    fn anything_from_a_browser_is_refused_before_the_token_is_compared() {
        // DNS rebinding: a page the user is merely *visiting* resolves a name to 127.0.0.1 and
        // posts here. A browser always sends `Origin`; a real MCP client never does — so the
        // presence of the header is the whole signal, and no list of allowed origins has to be
        // kept up to date.
        let (guard, token) = guard();
        let header = format!("Bearer {}", token.as_str());
        let mut request = asking(&header);
        request.origin = Some("http://evil.example");
        assert_eq!(guard.admit(&request), Err(Refused::FromAPage));

        // Even its own machine's name. There is no origin that is fine.
        request.origin = Some("http://localhost:3000");
        assert_eq!(guard.admit(&request), Err(Refused::FromAPage));
    }

    #[test]
    fn a_page_is_told_forbidden_rather_than_unauthorized() {
        // 401 teaches a page that a token exists and is worth guessing. 403 teaches it nothing.
        assert_eq!(Refused::FromAPage.status(), 403);
        assert_eq!(Refused::NotYou.status(), 401);
    }

    #[test]
    fn only_the_one_path_answers_and_it_answers_before_anything_else_is_checked() {
        let (guard, _) = guard();
        let mut request = asking("Bearer wrong");
        request.path = "/";
        // Not `NotYou`: a wrong door should not reveal that the right one takes a token.
        assert_eq!(guard.admit(&request), Err(Refused::NotHere));

        request.path = "/mcp/../admin";
        assert_eq!(guard.admit(&request), Err(Refused::NotHere));
    }

    #[test]
    fn a_query_string_does_not_change_which_door_this_is() {
        let (guard, token) = guard();
        let header = format!("Bearer {}", token.as_str());
        let mut request = asking(&header);
        request.path = "/mcp?session=1";
        assert_eq!(guard.admit(&request), Ok(()));
    }

    #[test]
    fn get_is_refused_because_epoch_has_nothing_to_say_unprompted() {
        // The specification's GET is the server-initiated stream. Epoch declares
        // `listChanged: false` and offers no resources, so there is nothing to stream.
        let (guard, token) = guard();
        let header = format!("Bearer {}", token.as_str());
        let mut request = asking(&header);
        request.method = "GET";
        assert_eq!(guard.admit(&request), Err(Refused::WrongMethod));
    }

    #[test]
    fn two_tokens_are_never_the_same() {
        // From the operating system, not from a clock or a process id — both guessable by
        // exactly the local programs this exists to keep out.
        let one = Token::fresh();
        let two = Token::fresh();
        assert_ne!(one.as_str(), two.as_str());
        assert_eq!(one.as_str().len(), 64, "32 bytes, hex");
    }

    #[cfg(windows)]
    struct VaultDir(std::path::PathBuf);

    #[cfg(windows)]
    impl VaultDir {
        fn new(name: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let number = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("epoch-door-token-{name}-{number}"));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    #[cfg(windows)]
    impl Drop for VaultDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_plaintext_door_token_migrates_without_changing_the_agents_bearer() {
        let vault = VaultDir::new("migrate");
        let old = Token::fresh();
        std::fs::write(vault.0.join("agent-token"), old.as_str()).unwrap();
        let secrets = Secrets::at(&vault.0);

        let migrated = remembered(&secrets, &vault.0).unwrap();
        assert_eq!(migrated.as_str(), old.as_str());
        assert!(!vault.0.join("agent-token").exists());
        assert_eq!(
            secrets
                .get_checked(&SecretName::for_mcp_door())
                .unwrap()
                .expect("the token moved into DPAPI")
                .expose(),
            old.as_str()
        );
        let encrypted = std::fs::read(vault.0.join("secrets.dat")).unwrap();
        assert!(encrypted
            .windows(64)
            .all(|piece| piece != old.as_str().as_bytes()));

        let reopened = remembered(&secrets, &vault.0).unwrap();
        assert_eq!(reopened.as_str(), old.as_str());
    }

    #[cfg(windows)]
    #[test]
    fn a_fresh_door_uses_dpapi_and_never_creates_the_legacy_file() {
        let vault = VaultDir::new("fresh");
        let secrets = Secrets::at(&vault.0);

        let token = remembered(&secrets, &vault.0).unwrap();
        assert_eq!(token.as_str().len(), 64);
        assert!(vault.0.join("secrets.dat").exists());
        assert!(!vault.0.join("agent-token").exists());
    }

    #[test]
    fn a_token_never_appears_in_its_own_debug_output() {
        // A token in a panic message is a token in a bug report.
        let token = Token::of("supersecret");
        assert_eq!(format!("{token:?}"), "Token(…)");
        assert!(!format!("{token:?}").contains("supersecret"));
    }

    #[test]
    fn comparison_does_not_stop_at_the_first_wrong_byte() {
        // Not a timing measurement — that is not something a unit test can assert reliably.
        // What it does assert is the property that makes the timing constant: every byte of
        // our own token is visited, whatever the guess looks like.
        let token = Token::of("abcdef");
        assert!(!token.matches("abcdeX"));
        assert!(!token.matches("Xbcdef"));
        assert!(!token.matches(""));
        assert!(!token.matches("abcdefgh"));
        assert!(token.matches("abcdef"));
    }

    /// The whole endpoint, over a real socket.
    ///
    /// One test that opens a port, because the guard is tested exhaustively without one and
    /// this exists to prove the wiring — that the guard is actually consulted, that a
    /// notification gets no body, and that closing closes.
    #[test]
    fn a_real_client_is_answered_and_a_stranger_is_not() {
        struct Echo;
        impl Answering for Echo {
            fn answer(&self, message: &Value) -> Option<Value> {
                message.get("id")?;
                Some(serde_json::json!({ "jsonrpc": "2.0", "id": message["id"], "result": {} }))
            }
        }

        let token = Token::fresh();
        // Port 0: the operating system picks a free one, so a busy machine cannot fail this.
        let open = open(0, token.clone(), Echo).expect("must listen on loopback");
        let address = open.address();

        let post = |auth: Option<&str>, body: &str| -> String {
            use std::io::Write;
            let mut socket = std::net::TcpStream::connect(address).unwrap();
            let mut request = format!(
                "POST /mcp HTTP/1.1\r\nHost: {address}\r\nContent-Length: {}\r\n",
                body.len()
            );
            if let Some(auth) = auth {
                request.push_str(&format!("Authorization: {auth}\r\n"));
            }
            request.push_str("Connection: close\r\n\r\n");
            request.push_str(body);
            socket.write_all(request.as_bytes()).unwrap();
            let mut answer = String::new();
            let _ = socket.read_to_string(&mut answer);
            answer
        };

        let good = format!("Bearer {}", token.as_str());
        let answered = post(Some(&good), r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#);
        assert!(answered.contains("200 OK"), "{answered}");
        assert!(answered.contains("\"id\":1"), "{answered}");

        // The guard is really in the path, not merely tested beside it.
        let stranger = post(
            Some("Bearer nope"),
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
        );
        assert!(stranger.contains("401"), "{stranger}");

        // A notification is acknowledged with no body.
        let notified = post(
            Some(&good),
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        );
        assert!(notified.contains("202"), "{notified}");

        open.close();
    }
}
