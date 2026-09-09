//! The Host's door, and it is the only one on the network.
//!
//! ## Why this is not `tiny_http`
//!
//! Everything the Host and this machine say to each other has to be encrypted — the composed
//! turn is the user's Chronicle and their project's context, and pairing carries the secret
//! itself. `tiny_http` can do TLS, and the version of `rustls` it does it with is 0.20, from
//! 2021. Bringing a four-year-old TLS stack into the fix for *"this traffic is not encrypted"*
//! would be answering the audit with a thing to write in the next one.
//!
//! Two other shapes were considered before this one:
//!
//! - **A TLS terminator in front of a loopback `tiny_http`.** It works, and it needs a full
//!   duplex copy in both directions between a `rustls` stream that cannot be split and a socket
//!   that can — two threads and a shutdown ordering problem, to serve seven routes.
//! - **Forwarding into the window's own listener.** Cheapest, and wrong: the forwarded
//!   connection arrives from `127.0.0.1`, so the whole network would be standing where the
//!   person at this machine stands.
//!
//! So the Host's surface gets its own listener, with `rustls` 0.23 — the version already in this
//! tree — and the HTTP it needs to understand, which is a request line, a few headers and a body
//! of a declared length.
//!
//! ## What this deliberately does not implement
//!
//! No keep-alive: one request, one answer, `Connection: close`. No chunked bodies, no
//! `Expect: 100-continue`, no compression, no pipelining. **Every one of those is a refusal
//! rather than a silence** — an unreadable request is answered `400` and the connection ends.
//!
//! That is not a general web server and must never become one. It answers exactly the program
//! on the other side, which is Epoch, and a request it does not recognise is a request that did
//! not come from Epoch.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

/// The most bytes of request line and headers. A Host sends a handful.
const HEAD_LIMIT: usize = 16 * 1024;

/// The most bytes of body **any** route may carry — a composed turn, with base64 images in it.
///
/// A hard ceiling above whatever a door asks for, so a `Ceiling` that returns something enormous
/// cannot widen this one past what [`BODY_BUDGET`] was sized against.
const BODY_LIMIT: usize = 64 * 1024 * 1024;

/// What one route is allowed to carry, decided by whoever owns the door.
///
/// **A turn is megabytes and a pairing code is six characters, and they were sharing a ceiling.**
/// The Host's pairing door answers exactly one route, whose whole content is a short code and a
/// fingerprint — and it would reserve 64 MiB for it before the code was so much as looked at,
/// because whether a caller is who they say is the handler's question and the handler runs
/// after the body is read. Named by the second audit (2026-09-07, finding 3): *small limits on
/// `/enrol`*.
///
/// A function rather than a table in here, because `epoch-wire` does not know Epoch's routes and
/// must not learn them: this crate is the door, and which rooms are behind it belongs to the
/// program that opened it. Two doors use this listener and their routes have nothing in common.
pub type Ceiling = Arc<dyn Fn(&str) -> usize + Send + Sync>;

/// The ceiling for a door that has not thought about it: small.
///
/// **The default is the safe direction.** A door that forgets to raise it refuses a large
/// request, which is visible and fixable; a door that forgets to lower it reserves megabytes for
/// a form, which is invisible until somebody uses it as a way in.
pub const MODEST: usize = 64 * 1024;

/// How long a caller gets to finish its handshake and its request.
///
/// **This is the deadline the loopback listener cannot have.** Here the socket is ours before
/// anything is parsed, so a caller that connects and says nothing costs one thread for a minute
/// rather than forever.
const READING: std::time::Duration = std::time::Duration::from_secs(60);

/// How long an answer has to go back.
///
/// Separate from [`READING`] because they are different questions, and longer because a picture
/// is megabytes and a slow network is not a slow caller.
const WRITING: std::time::Duration = std::time::Duration::from_secs(300);

/// How many connections may be in the building at once.
const IN_FLIGHT: usize = 32;

/// How long one caller gets to deliver its **whole** request — handshake, head and body.
///
/// **[`READING`] is not this, and reading it as this is the mistake.** A socket read timeout
/// bounds one `read` call: a caller that sends a byte every fifty seconds resets it forever and
/// holds a thread for as long as it likes. Named separately because they answer different
/// questions, and the audit found the second one being answered with the first.
const WHOLE_REQUEST: std::time::Duration = std::time::Duration::from_secs(120);

/// The most body bytes **every connection together** may be holding.
///
/// [`BODY_LIMIT`] bounds one request; [`IN_FLIGHT`] bounds how many there are; multiplying them
/// gave 2 GiB of buffers this program would reserve on request. One caller may still send a
/// composed turn with pictures in it — several at once is what is refused, and refused with a
/// status rather than by running out of memory.
///
/// A number in bytes rather than a share of what the machine has: this is a lending program that
/// runs beside a graphics card holding a model, and *what is free right now* would make the door
/// stricter exactly when the machine is busiest.
const BODY_BUDGET: usize = 192 * 1024 * 1024;

/// What is reserved for bodies at this moment, across every connection.
static RESERVED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A reservation that gives itself back.
///
/// Held for exactly as long as the buffer it accounts for. A counter incremented and decremented
/// by hand leaks the day somebody adds an early return, and this door has several.
struct Reserved(usize);

impl Reserved {
    /// `None` when the budget cannot take it, which the caller answers `503`.
    fn of(bytes: usize) -> Option<Self> {
        use std::sync::atomic::Ordering::SeqCst;
        // Compare-and-swap rather than fetch_add-then-check: two connections adding first and
        // checking afterwards can both pass a budget only one of them fits in.
        let mut held = RESERVED.load(SeqCst);
        loop {
            let wanted = held.checked_add(bytes)?;
            if wanted > BODY_BUDGET {
                return None;
            }
            match RESERVED.compare_exchange_weak(held, wanted, SeqCst, SeqCst) {
                Ok(_) => return Some(Self(bytes)),
                Err(now) => held = now,
            }
        }
    }
}

impl Drop for Reserved {
    fn drop(&mut self) {
        RESERVED.fetch_sub(self.0, std::sync::atomic::Ordering::SeqCst);
    }
}

/// A reader that stops being one when the clock runs out.
///
/// Wrapping rather than checking between reads: the check has to happen where the blocking
/// happens, and the blocking is inside `read_line` and `read_to_end` rather than between them.
///
/// ## Checking before a read is not a deadline
///
/// The first version of this checked the clock and then called `inner.read()`, which cannot
/// interrupt the call it just started. The audit measured it: a 10 ms deadline, a read that
/// takes 60 ms, `Ok(1)` at 60 ms. In this program the overshoot was bounded — [`READING`] is on
/// the socket, so the worst case was a minute past the deadline rather than forever — and a
/// minute past a two-minute deadline is still not the deadline.
///
/// So the socket is told. Before every read the timeout becomes *what is left*, capped at
/// [`READING`] because a long deadline must not make one silent read cheaper than it is now.
/// The wrapper still refuses outright once the clock has run out, because that is the case no
/// socket option covers: the deadline may pass between two reads that each finish on time.
///
/// **Zero is never passed on.** `setsockopt` reads a zero timeout as *no timeout*, so handing
/// over the remaining time without checking it would remove the bound at the exact moment it was
/// supposed to fire — a mistake that looks like a fix and measures as an unbounded read.
///
/// ## It wraps the socket, not the TLS stream, and that was measured
///
/// The obvious arrangement — wrap the TLS stream and keep a `try_clone()` of the socket to set
/// the timeout on — **does not work on Windows**, and the first version of this fix shipped it.
/// A cloned handle carries its own `SO_RCVTIMEO`:
///
/// ```text
/// set on clone:          Ok(())
/// read back on clone:    Ok(Some(150ms))
/// read back on original: Ok(None)      <- and the read then blocked forever
/// ```
///
/// So this sits *underneath* TLS and owns the socket it bounds. Which is the better place
/// anyway: a fragmented handshake is read through here too, and the arrangement that needed a
/// clone could not have bounded it.
struct Until<R> {
    inner: R,
    by: std::time::Instant,
}

/// A reader whose next block can be bounded from outside.
///
/// Two implementations and no blanket one, deliberately. A default that silently did nothing
/// would let a future reader be wrapped in [`Until`] and quietly get the weaker guarantee — the
/// between-reads check — while the doc above promised the socket-level one. Each type says what
/// it can actually do.
trait Bounded {
    /// Take no longer than this on the next read, when that can be asked of it.
    fn within(&self, this_long: std::time::Duration);
}

impl Bounded for TcpStream {
    fn within(&self, this_long: std::time::Duration) {
        // Ignored on purpose: a socket that will not take a timeout is still readable, and
        // refusing the request over it would turn a lost bound into a lost connection.
        let _ = self.set_read_timeout(Some(this_long));
    }
}

/// Bytes already in memory cannot block, so there is nothing to bound.
impl Bounded for &[u8] {
    fn within(&self, _this_long: std::time::Duration) {}
}

/// How long the next read may block: what is left, never more than [`READING`], never zero.
///
/// Split out because it is the whole of the decision and the only part a test can hold still.
/// `None` means the deadline has passed and nothing should be read at all.
fn allowance(by: std::time::Instant, now: std::time::Instant) -> Option<std::time::Duration> {
    let left = by.checked_duration_since(now)?;
    if left.is_zero() {
        return None;
    }
    Some(left.min(READING))
}

impl<R: Read + Bounded> Read for Until<R> {
    fn read(&mut self, into: &mut [u8]) -> std::io::Result<usize> {
        let Some(left) = allowance(self.by, std::time::Instant::now()) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "the request took too long",
            ));
        };
        self.inner.within(left);
        self.inner.read(into)
    }
}

/// Straight through. TLS needs to write down the same socket it reads, and nothing here has an
/// opinion about writing — [`WRITING`] is set once on the socket and answers a different
/// question (how long an answer has to go back, not how long a request has to arrive).
impl<R: std::io::Write> std::io::Write for Until<R> {
    fn write(&mut self, from: &[u8]) -> std::io::Result<usize> {
        self.inner.write(from)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// What the Host asked for.
///
/// ## It carries its own budget
///
/// The reservation used to be a local in `read_request`, so it was given back the moment that
/// function returned — while the body it accounted for travelled on to the handler. The counter
/// therefore bounded *how much is being read at once* and called itself a bound on how much this
/// program is holding, which is a different and much larger number. The audit reproduced the gap
/// with two tiny bodies: `live_body_bytes=32 reserved_bytes=0`.
///
/// So the reservation lives here, and is given back when this is dropped.
///
/// **What that does and does not cover, stated plainly.** It covers this body for as long as the
/// request exists. It does not cover what a handler copies out of it — a parsed structure, a
/// decoded picture, a string pushed into a queue — because nothing at this door can see those,
/// and a counter that claimed to would be an invented reading. The budget is a bound on what the
/// *door* is holding.
pub struct Asked {
    pub method: String,
    pub path: String,
    /// The bearer, with `Bearer ` already off it. `None` when there was no `Authorization`.
    pub bearer: Option<String>,
    pub body: String,
    /// The budget this body is counted against, given back when this is dropped.
    ///
    /// `None` for a request assembled by hand rather than read off a door: nothing was reserved,
    /// so there is nothing to give back, and saying so is more honest than a zero-sized
    /// reservation that pretends the accounting happened.
    _room: Option<Reserved>,
}

impl Asked {
    /// One assembled by hand rather than read off a door.
    ///
    /// For tests and for callers that already hold the parts. It reserves nothing, because
    /// nothing was read: the budget exists to bound what arrives over a socket, and a request
    /// built in memory was already paid for by whoever built it.
    pub fn made(
        method: impl Into<String>,
        path: impl Into<String>,
        bearer: Option<String>,
        body: impl Into<String>,
    ) -> Self {
        Asked {
            method: method.into(),
            path: path.into(),
            bearer,
            body: body.into(),
            _room: None,
        }
    }
}

/// **Printable without printing the two things nobody may log.** The tests need to read a
/// parsed request back, and a derived `Debug` would put the bearer — and the user's whole
/// conversation — into whatever printed it. Written by hand so that cannot happen by accident.
impl std::fmt::Debug for Asked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Asked")
            .field("method", &self.method)
            .field("path", &self.path)
            .field("bearer", &self.bearer.as_ref().map(|_| "<held>"))
            .field("body", &format_args!("{} bytes", self.body.len()))
            .finish()
    }
}

/// What this machine answers with.
pub struct Answer {
    pub status: u16,
    pub kind: &'static str,
    pub body: String,
}

impl Answer {
    pub fn new(status: u16, kind: &'static str, body: String) -> Self {
        Self { status, kind, body }
    }
}

/// Listen on `port`, encrypted, and answer with `handle`.
///
/// Blocks forever, so the caller gives it a thread. A failure to bind is returned rather than
/// printed: a machine that cannot open its door has to say so in a window.
pub fn serve<H>(
    port: u16,
    identity: &crate::tls::Identity,
    ceiling: Ceiling,
    handle: H,
) -> Result<std::convert::Infallible, String>
where
    H: Fn(Asked) -> Answer + Send + Sync + 'static,
{
    let never = Arc::new(std::sync::atomic::AtomicBool::new(false));
    answer_on(open(port)?, identity, never, ceiling, handle)?;
    Err("the door stopped accepting connections".to_owned())
}

/// Take the port, or say why not.
///
/// **Separate from answering, because binding is the part a caller has to know about.** A door
/// opened on a thread reports its failure to that thread: the caller is told *the door is open*
/// and finds out otherwise when somebody knocks. Worse, it is a race rather than a state — on
/// Windows the listener happened to be up before the first connection and on macOS it did not,
/// so two tests that had passed for a day failed on the other machine with `connection refused`.
///
/// So the caller binds, on its own thread, and hands the result to [`answer_on`]. A taken port
/// is then a `Result` at the moment it can still be reported to a person.
pub fn open(port: u16) -> Result<std::net::TcpListener, String> {
    let listener = std::net::TcpListener::bind(("0.0.0.0", port))
        .map_err(|why| format!("port {port} could not be opened: {why}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|why| format!("port {port} could not be watched: {why}"))?;
    Ok(listener)
}

/// Answer on a listener that is already bound, for as long as `stop` says so.
///
/// **The pairing door needs this and the lending door does not**, which is why there are two
/// entry points rather than one with a flag nobody sets. A code buys one bond and lasts five
/// minutes; a door that outlived it would be a door nobody is watching.
///
/// Polled rather than woken: the listener is non-blocking and this looks at `stop` between
/// attempts. The alternative is connecting to yourself to break an `accept`, which is a trick
/// that has to work on three operating systems to be worth the line it saves.
pub fn answer_on<H>(
    listener: std::net::TcpListener,
    identity: &crate::tls::Identity,
    stop: Arc<std::sync::atomic::AtomicBool>,
    ceiling: Ceiling,
    handle: H,
) -> Result<(), String>
where
    H: Fn(Asked) -> Answer + Send + Sync + 'static,
{
    let config = identity.server()?;
    let handle = Arc::new(handle);
    let in_flight = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
        let stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(why) if why.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
            Err(_) => continue,
        };
        // Back to blocking for this conversation: the deadline it answers to is the socket's own
        // read timeout, not the accept loop's poll.
        if stream.set_nonblocking(false).is_err() {
            continue;
        }
        if in_flight.load(std::sync::atomic::Ordering::Relaxed) >= IN_FLIGHT {
            // Nothing is said, because nothing can be: refusing politely would mean completing
            // a TLS handshake first, which is the work being refused.
            continue;
        }
        in_flight.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let config = Arc::clone(&config);
        let handle = Arc::clone(&handle);
        let ceiling = Arc::clone(&ceiling);
        let counted = Arc::clone(&in_flight);
        let started = std::thread::Builder::new().spawn(move || {
            converse(stream, config, ceiling.as_ref(), handle.as_ref());
            counted.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        });
        if started.is_err() {
            in_flight.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
    Ok(())
}

/// One connection: handshake, one request, one answer, close.
fn converse<H>(
    stream: TcpStream,
    config: Arc<rustls::ServerConfig>,
    ceiling: &(dyn Fn(&str) -> usize + Send + Sync),
    handle: &H,
) where
    H: Fn(Asked) -> Answer,
{
    let _ = stream.set_read_timeout(Some(READING));
    let _ = stream.set_write_timeout(Some(WRITING));
    let Ok(connection) = rustls::ServerConnection::new(config) else {
        return;
    };
    // **Under TLS, holding the socket itself.** The handshake is read through this too, which
    // is the half a wrapper above TLS could never have covered.
    let bounded = Until {
        inner: stream,
        by: std::time::Instant::now() + WHOLE_REQUEST,
    };
    let mut tls = rustls::StreamOwned::new(connection, bounded);
    let answer = match read_request(&mut tls, ceiling) {
        Ok(asked) => handle(asked),
        // Deliberately shapeless. Something that cannot speak this program's HTTP is not the
        // Host, and telling it what it got wrong is telling a stranger about the door.
        Err(status) => Answer::new(status, "text/plain", "no".to_owned()),
    };

    let head = format!(
        "HTTP/1.1 {} \r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        answer.status,
        answer.kind,
        answer.body.len()
    );
    // Written straight down the TLS stream. [`Until`] sits under it and bounds how long a caller
    // may take to *ask*; an answer that is slow to go out is a slow network rather than a slow
    // caller — which is what [`WRITING`] is for and why they are two numbers.
    let _ = tls.write_all(head.as_bytes());
    let _ = tls.write_all(answer.body.as_bytes());
    let _ = tls.flush();
    tls.conn.send_close_notify();
    let _ = tls.flush();
}

/// Read one request, or the status to refuse it with.
fn read_request(
    tls: &mut impl Read,
    ceiling: &(dyn Fn(&str) -> usize + Send + Sync),
) -> Result<Asked, u16> {
    let mut reader = BufReader::new(tls);

    let mut head = String::new();
    loop {
        let mut line = String::new();
        let read = reader
            .by_ref()
            .take((HEAD_LIMIT - head.len()) as u64)
            .read_line(&mut line)
            .map_err(|_| 400u16)?;
        if read == 0 {
            return Err(400);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        head.push_str(&line);
        if head.len() >= HEAD_LIMIT {
            return Err(431);
        }
    }

    let mut lines = head.lines();
    let start = lines.next().ok_or(400u16)?;
    let mut parts = start.split_whitespace();
    let method = parts.next().ok_or(400u16)?.to_owned();
    let target = parts.next().ok_or(400u16)?;
    // The query is not this door's business and the path must be exact.
    let path = target.split(['?', '#']).next().unwrap_or("/").to_owned();

    let mut bearer = None;
    let mut length = 0usize;
    let mut chunked = false;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("authorization") {
            bearer = Some(value.trim_start_matches("Bearer ").trim().to_owned());
        } else if name.eq_ignore_ascii_case("content-length") {
            length = value.parse().map_err(|_| 400u16)?;
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            chunked = true;
        }
    }
    // **A length or nothing.** A body whose size is described some other way is a body this
    // program would have to guess the end of, and guessing where a request ends is how two
    // programs come to disagree about where the next one starts.
    if chunked {
        return Err(411);
    }
    // **Asked after the path is known and before a byte of body is read**, which is the only
    // window where it is both answerable and useful. `BODY_LIMIT` stays as the hard ceiling: a
    // door may lower it and may not raise it past what the budget was sized against.
    if length > ceiling(&path).min(BODY_LIMIT) {
        return Err(413);
    }

    // **Reserved before it is read, and given back however this returns.** Without this, thirty
    // -two callers each claiming the ceiling reserve two gigabytes between them — and they can
    // claim it before anything they say has been believed, because whether a bearer is *valid*
    // is the handler's question and the handler runs after this.
    let room = Reserved::of(length).ok_or(503u16)?;

    // Read what arrives rather than trusting the number: `vec![0u8; length]` commits the whole
    // claim to memory before a byte of it exists, so a caller that promises 64 MiB and sends one
    // costs 64 MiB. `read_to_end` on a bounded reader costs what was sent.
    let mut body = Vec::new();
    if length > 0 {
        reader
            .by_ref()
            .take(length as u64)
            .read_to_end(&mut body)
            .map_err(|_| 400u16)?;
        // A short body is not a small request, it is half of one — the same refusal
        // `read_exact` used to give, kept now that the read itself tolerates it.
        if body.len() != length {
            return Err(400);
        }
    }
    Ok(Asked {
        method,
        path,
        bearer,
        // Moved rather than copied when it is already valid UTF-8, which is every real request.
        // `from_utf8_lossy(&body).into_owned()` allocates the whole body a second time, so a
        // 64 MiB request briefly cost 128 MiB against a budget that had been told about 64.
        body: String::from_utf8(body)
            .unwrap_or_else(|bad| String::from_utf8_lossy(bad.as_bytes()).into_owned()),
        _room: Some(room),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The budget is one counter for the whole program, so its tests take turns.**
    ///
    /// Two of these assert what is reserved *after* they finish, and `cargo test` runs
    /// them on several threads: without this they read each other's reservations and fail
    /// for a reason that has nothing to do with the door. Found by running them together
    /// after each passed alone.
    static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// A request read out of bytes, exactly as it arrives from a socket.
    ///
    /// At the hard ceiling, so these keep measuring the door's own limits rather than
    /// one caller's — the per-route cap has its own test below.
    fn read(raw: &str) -> Result<Asked, u16> {
        read_request(&mut raw.as_bytes(), &|_| BODY_LIMIT)
    }

    #[test]
    fn a_turn_arrives_whole() {
        let asked = read(
            "POST /ask HTTP/1.1\r\nHost: 10.0.0.9:11500\r\nAuthorization: Bearer sesame\r\n\
             Content-Type: application/json\r\nContent-Length: 13\r\n\r\n{\"model\":\"x\"}",
        )
        .expect("a well-formed turn");
        assert_eq!(asked.method, "POST");
        assert_eq!(asked.path, "/ask");
        assert_eq!(asked.bearer.as_deref(), Some("sesame"));
        assert_eq!(asked.body, "{\"model\":\"x\"}");
    }

    #[test]
    fn the_query_is_not_part_of_the_path() {
        // Otherwise `/have?x=1` is not `/have`, and a route match becomes a string somebody can
        // walk past.
        let asked = read("GET /have?x=1 HTTP/1.1\r\nContent-Length: 0\r\n\r\n").expect("a read");
        assert_eq!(asked.path, "/have");
        assert_eq!(asked.bearer, None);
    }

    #[test]
    fn a_body_bigger_than_the_ceiling_is_refused_before_it_is_read() {
        let refused = read(&format!(
            "POST /ask HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            BODY_LIMIT + 1
        ))
        .unwrap_err();
        assert_eq!(refused, 413);
    }

    /// **A pairing code and a composed turn stop sharing a ceiling.**
    ///
    /// The Host's pairing door answers one route whose whole content is a short code and a
    /// fingerprint, and it would reserve 64 MiB for it — before the code was looked at, because
    /// whether a caller is who they say is the handler's question and the handler runs after the
    /// body is read. Named by the second audit (2026-09-07, finding 3).
    ///
    /// Asserted as the *door deciding*, on one request that both doors would see: the same bytes
    /// are refused where the ceiling is modest and read where it is not. A test that only checked
    /// the refusal would pass against a door that refused everything.
    #[test]
    fn each_door_says_how_much_its_own_routes_may_carry() {
        let raw = format!(
            "POST /enrol HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            MODEST + 1
        );

        // The pairing door: one route, and nothing large belongs on it.
        assert_eq!(
            read_request(&mut raw.as_bytes(), &|_| MODEST).unwrap_err(),
            413
        );

        // A door that carries turns: the same request is merely a request.
        let permissive = read_request(&mut raw.as_bytes(), &|_| BODY_LIMIT).unwrap_err();
        assert_ne!(permissive, 413, "not refused for its size here");

        // And a door may lower its own ceiling per route without lowering the others.
        let split = |path: &str| if path == "/ask" { BODY_LIMIT } else { MODEST };
        assert_eq!(read_request(&mut raw.as_bytes(), &split).unwrap_err(), 413);
        let allowed = format!(
            "POST /ask HTTP/1.1\r\nContent-Length: {}\r\n\r\n{}",
            MODEST + 1,
            "x".repeat(MODEST + 1)
        );
        assert_eq!(
            read_request(&mut allowed.as_bytes(), &split)
                .expect("a turn is allowed to be large")
                .path,
            "/ask"
        );
    }

    /// A door may not raise its own ceiling past what the shared budget was sized against.
    #[test]
    fn a_generous_door_cannot_widen_the_hard_ceiling() {
        let raw = format!(
            "POST /ask HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            BODY_LIMIT + 1
        );
        assert_eq!(
            read_request(&mut raw.as_bytes(), &|_| usize::MAX).unwrap_err(),
            413
        );
    }

    #[test]
    fn a_body_with_no_length_is_refused_rather_than_guessed_at() {
        let refused =
            read("POST /ask HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello").unwrap_err();
        assert_eq!(refused, 411);
    }

    #[test]
    fn a_head_that_never_ends_is_refused() {
        // Slowloris in its simplest form: headers forever. The socket also has a deadline, and
        // this is the half that does not depend on the caller ever pausing.
        let endless = "GET / HTTP/1.1\r\n".to_owned() + &"X-Pad: pad\r\n".repeat(4000);
        assert_eq!(read(&endless).unwrap_err(), 431);
    }

    /// **A body is reserved against a budget every connection shares, and 503 is the refusal.**
    ///
    /// The audit's arithmetic: thirty-two connections times a 64 MiB ceiling is two gigabytes
    /// of buffers this program would reserve on request, before anything a caller said had been
    /// believed — whether a bearer is *valid* is the handler's question, and the handler runs
    /// after the body is read.
    ///
    /// Asserted by holding a reservation and watching the next request be refused, rather than
    /// by allocating two gigabytes to see what happens.
    #[test]
    fn a_body_nobody_has_room_for_is_refused_rather_than_allocated() {
        let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let held = Reserved::of(BODY_BUDGET).expect("an empty budget takes the whole of it");
        let refused = read(&format!(
            "POST /ask HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            1024
        ))
        .unwrap_err();
        assert_eq!(refused, 503, "no room, and it says so");

        drop(held);
        // And the room comes back when the buffer does: a counter that only goes up is a door
        // that closes once and stays closed.
        let asked = read("POST /ask HTTP/1.1\r\nContent-Length: 2\r\n\r\nhi").expect("room again");
        assert_eq!(asked.body, "hi");
        // Still counted, because the body is still here. This line read `0` when the
        // reservation died with `read_request`, and that zero was the defect rather than the
        // property -- a test asserting the bug is the hardest kind to see.
        assert_eq!(RESERVED.load(std::sync::atomic::Ordering::SeqCst), 2);
        drop(asked);
        assert_eq!(RESERVED.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    /// **What is held is what arrived, not what was claimed.**
    ///
    /// `vec![0u8; length]` commits the whole claim before a byte of it exists, so a caller that
    /// promises 64 MiB and sends one costs 64 MiB. This is the same refusal for a short body —
    /// still `400`, still half a request — with the memory following the bytes.
    #[test]
    fn a_claimed_length_is_not_memory_until_it_arrives() {
        let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let refused = read(&format!(
            "POST /ask HTTP/1.1\r\nContent-Length: {}\r\n\r\nshort",
            BODY_LIMIT
        ))
        .unwrap_err();
        assert_eq!(refused, 400);
        // Nothing stays reserved after a refusal, which is what `Reserved`'s `Drop` is for.
        assert_eq!(RESERVED.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    /// **The budget is held for as long as the body is.**
    ///
    /// The defect, as the audit reproduced it: the reservation was a local in `read_request`, so
    /// it was given back the moment that function returned -- while the body travelled on to the
    /// handler. Two tiny bodies alive, `reserved_bytes=0`. The counter bounded how much was
    /// being *read* at once and was being read as a bound on how much this program was holding.
    ///
    /// Both halves are asserted, because only the pair is the property: still counted while the
    /// request is in hand, back to nothing once it is dropped. A version that never gave it back
    /// would pass the first assertion and wedge the door after a few hundred requests.
    #[test]
    fn a_body_still_in_hand_is_still_reserved() {
        let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(RESERVED.load(std::sync::atomic::Ordering::SeqCst), 0);

        let asked =
            read("POST /ask HTTP/1.1\r\nContent-Length: 5\r\n\r\nhello").expect("a request");
        assert_eq!(asked.body, "hello");
        assert_eq!(RESERVED.load(std::sync::atomic::Ordering::SeqCst), 5);

        drop(asked);
        assert_eq!(RESERVED.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    /// **One built by hand reserves nothing, and says so by holding nothing.**
    ///
    /// A constructor that quietly reserved would make every test and every in-memory caller
    /// spend the door's budget on memory the door never read.
    #[test]
    fn a_request_nobody_read_off_a_socket_costs_the_budget_nothing() {
        let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let made = Asked::made("POST", "/ask", None, "hello");
        assert_eq!(RESERVED.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(made.body, "hello");
    }

    /// **What the next read is allowed to take, which is the whole of the decision.**
    ///
    /// Three cases and each one is a different bug if it is wrong: an expired deadline that
    /// still reads, a long deadline that lets one read block for hours, and a zero handed to
    /// `setsockopt` — which reads zero as *no timeout* and would remove the bound at the moment
    /// it was meant to fire.
    #[test]
    fn a_read_may_take_what_is_left_and_never_zero() {
        let now = std::time::Instant::now();

        // Already over: nothing may be read at all.
        assert_eq!(
            allowance(now - std::time::Duration::from_secs(1), now),
            None
        );
        assert_eq!(allowance(now, now), None);

        // Less than a whole socket timeout left: that is what the read gets.
        let soon = now + std::time::Duration::from_millis(10);
        let left = allowance(soon, now).expect("time left");
        assert!(left <= std::time::Duration::from_millis(10), "{left:?}");
        assert!(!left.is_zero());

        // More left than one read may take anyway: capped, so a long deadline does not make a
        // single silent read cheaper than it is today.
        let far = now + std::time::Duration::from_secs(3_600);
        assert_eq!(allowance(far, now), Some(READING));
    }

    /// **A read already blocking is interrupted, on a real socket.**
    ///
    /// The audit's measurement was a 10 ms deadline against a 60 ms read that returned `Ok(1)`
    /// at 60 ms — the wrapper checked the clock and then started a call it could not stop. This
    /// is that measurement with a socket underneath and nobody at the other end: the peer stays
    /// connected and sends nothing, so the only thing that can end the read is the deadline.
    ///
    /// Timed rather than merely asserted, because the failure this replaces is not *the wrong
    /// error* — it is *the right error, a minute late*. [`READING`] would have answered this
    /// eventually, and a test that only checked the kind would pass against the version that
    /// took sixty seconds.
    #[test]
    fn a_read_in_flight_ends_when_the_clock_does() {
        let door = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
        let at = door.local_addr().expect("an address");
        // Held open for the whole test: dropping it closes the connection, and a closed
        // connection ends the read for a reason that has nothing to do with the deadline.
        let quiet = TcpStream::connect(at).expect("a caller");
        let (side, _) = door.accept().expect("the other half");

        let mut waiting = Until {
            inner: side,
            by: std::time::Instant::now() + std::time::Duration::from_millis(150),
        };

        let began = std::time::Instant::now();
        let mut into = [0u8; 8];
        let answer = waiting.read(&mut into);
        let took = began.elapsed();

        assert!(answer.is_err(), "nothing was sent, so nothing may be read");
        assert!(
            took < std::time::Duration::from_secs(5),
            "the deadline has to end the read that is already blocking, not the socket timeout \
             a minute later: took {took:?}"
        );
        drop(quiet);
    }

    /// **The whole request has a deadline, and a socket read timeout is not one.**
    ///
    /// `set_read_timeout` bounds one `read`: a caller sending a byte at a time resets it forever
    /// and holds a thread for as long as it likes. Measured here by giving the reader a deadline
    /// that has already passed, over a stream that would otherwise be read happily.
    #[test]
    fn a_caller_that_takes_forever_runs_out_of_time() {
        let mut slow = Until {
            inner: "GET / HTTP/1.1\r\nContent-Length: 0\r\n\r\n".as_bytes(),
            by: std::time::Instant::now() - std::time::Duration::from_secs(1),
        };
        assert_eq!(read_request(&mut slow, &|_| BODY_LIMIT).unwrap_err(), 400);

        // The same bytes, in time, are a request.
        let mut ok = Until {
            inner: "GET / HTTP/1.1\r\nContent-Length: 0\r\n\r\n".as_bytes(),
            by: std::time::Instant::now() + std::time::Duration::from_secs(30),
        };
        assert_eq!(
            read_request(&mut ok, &|_| BODY_LIMIT)
                .expect("in time")
                .path,
            "/"
        );
    }

    #[test]
    fn a_truncated_body_is_not_half_a_request() {
        let refused = read("POST /ask HTTP/1.1\r\nContent-Length: 100\r\n\r\nshort").unwrap_err();
        assert_eq!(refused, 400);
    }
}
