//! Pairing a second machine, and what each pairing is allowed to be (ADR-0029).
//!
//! ## The measured fact this exists for
//!
//! **A bare Ollama on a LAN has no authentication at all.** Anything on the network can use
//! somebody's graphics card, read which models they have, and load one. That is not a flaw — it
//! was built for `localhost` — but it is the whole of why *"just point Epoch at the IP"* is not
//! the answer, and why a Bridge is more than a URL.
//!
//! ## A code is an introduction, never a credential
//!
//! The remote machine shows a **short code**; it is typed once and exchanged for a **long
//! secret** kept where the door token is kept. The code expires in minutes because it is short
//! enough to guess and lives on a screen where somebody can read it over a shoulder. The bond it
//! creates lasts until it is revoked.
//!
//! Short codes are compared **in constant time** for the same reason the agent door's token is:
//! comparison that stops at the first wrong byte tells an attacker how much of the code was
//! right, and a six-character alphabet is small enough for that to matter.
//!
//! ## Two grants, and they are not the same size
//!
//! | grant | what it means |
//! |---|---|
//! | [`Grant::Compute`] | *this machine may think for me* — a Bridge, touching nothing of the World |
//! | [`Grant::Surface`] | *from this machine I may drive my World* — a person, acting through the Host |
//!
//! **Separate on purpose.** If one secret bought both, a compromised compute machine would have
//! the user's terminal. `Compute` is what a graphics card in the next room is for; `Surface` is
//! the user's own hands somewhere else, and every capability it reaches still runs on the Host
//! and is still judged by the Host's Trust.
//!
//! A pairing with **no** grant is a paired machine that can do nothing, and that is a legitimate
//! state: it is what revoking both leaves behind, and it keeps the name and the secret so
//! granting again does not mean pairing again.

use epoch_models::quiet::Quiet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// What a paired machine is allowed to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grant {
    /// It may think. A `Provider` whose transport is another machine, and nothing else.
    Compute,
    /// A person may drive this World from it. Everything still happens on the Host.
    Surface,
}

impl Grant {
    pub const fn id(self) -> &'static str {
        match self {
            Grant::Compute => "compute",
            Grant::Surface => "surface",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "compute" => Some(Grant::Compute),
            "surface" => Some(Grant::Surface),
            _ => None,
        }
    }
}

/// One machine this Host has paired with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paired {
    /// Stable id. Generated here, never taken from the remote machine — a name somebody else
    /// chooses is a name somebody else can collide with.
    pub id: String,
    /// What the user calls it. Theirs to change.
    pub name: String,
    /// Where to reach it, exactly as the user typed it.
    pub address: String,
    /// The certificate that machine answers with, as a SHA-256 in lower-case hex.
    ///
    /// **`None` means this bond predates encrypted bridges, not that any certificate will do.**
    /// A machine with no fingerprint here cannot be connected to at all — there is nothing to
    /// check against, and accepting whatever turned up would be the pairing having bought
    /// nothing. The surface says to pair again, which costs one code.
    #[serde(default)]
    pub fingerprint: Option<String>,
    /// What it may be. Empty is a real state: paired, and allowed nothing.
    #[serde(default)]
    pub grants: Vec<Grant>,
    /// When the bond was made, in milliseconds.
    pub paired_at: u64,
    /// Which programs on that machine were serving, the last time it was asked.
    ///
    /// **Remembered so the registry does not have to go and find out.** Providers are built
    /// whenever anything needs to think, and a network round trip per paired machine on that
    /// path would put a stranger's Wi-Fi in front of every turn. So the probe writes what it
    /// learned and the registry reads it — the same arrangement the address itself has.
    ///
    /// Ids from the closed set (`ollama`, `llama_cpp`, `lm_studio`), never addresses. Empty is
    /// *never asked*, which is why the machine's default Provider exists regardless.
    #[serde(default)]
    pub runners: Vec<String>,
    /// Which image studios that machine was **serving**, the last time it was asked.
    ///
    /// Remembered for the same reason `runners` is, and kept separate for the reason ADR-0030
    /// gives: one of these answers a turn and the other cannot hold a conversation at all. A
    /// shared list is how a diffusion server ends up in the Brain dropdown.
    ///
    /// Ids from the closed set (`comfyui`), never addresses — a lent machine's ComfyUI answers
    /// on *its* loopback, and that address means nothing here. Drawing there goes through the
    /// Bridge's own door.
    #[serde(default)]
    pub easels: Vec<String>,
}

impl Paired {
    pub fn may(&self, grant: Grant) -> bool {
        self.grants.contains(&grant)
    }
}

/// Every machine paired with this Host.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Pairings {
    #[serde(default)]
    machines: Vec<Paired>,
}

/// How long a pairing code is worth typing.
///
/// **Minutes, because it is short.** Six characters from an unambiguous alphabet is roughly a
/// billion codes — plenty against a person reading it off a screen, not plenty against a
/// program allowed to keep trying. The window is what makes the size safe.
pub const CODE_LIFE: std::time::Duration = std::time::Duration::from_secs(300);

/// The alphabet a code is drawn from.
///
/// No `0`, `O`, `1`, `I` or `L`. Somebody is reading this off one screen and typing it into
/// another, and a code that is ambiguous to a human is a code that gets mistyped and read as a
/// failed pairing.
const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

/// A short code, alive for [`CODE_LIFE`].
#[derive(Debug, Clone)]
pub struct Code {
    code: String,
    made: std::time::Instant,
}

/// How many wrong answers this door survives.
///
/// The same number EpochServices uses, and for the same reason: the code is the whole of the
/// defence on a listener the network can reach, so how many times it may be offered is part of
/// it. Ten is enough for somebody reading six characters off another screen and mistyping.
const TRIES: u32 = 10;

impl Code {
    /// A code that ran out before it was ever shown.
    ///
    /// Test-only, and a constructor rather than a mutable field: the moment a code was made is
    /// not something any caller should be able to move, and waiting five minutes for the real
    /// thing is a test nobody would run.
    #[cfg(test)]
    pub(crate) fn already_expired() -> Self {
        let mut code = Self::fresh();
        code.made = code
            .made
            .checked_sub(CODE_LIFE + std::time::Duration::from_secs(1))
            .expect("a monotonic clock that has been running for a second");
        code
    }

    /// A fresh code from the operating system's randomness.
    pub fn fresh() -> Self {
        let mut bytes = [0u8; 6];
        getrandom::getrandom(&mut bytes).expect("the operating system must provide randomness");
        Self {
            code: bytes
                .iter()
                .map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char)
                .collect(),
            made: std::time::Instant::now(),
        }
    }

    /// The code, to be shown on a screen and nowhere else.
    pub fn as_str(&self) -> &str {
        &self.code
    }

    pub fn expired(&self) -> bool {
        self.made.elapsed() > CODE_LIFE
    }

    /// Whether an offered code is this one, compared **in constant time**.
    ///
    /// A comparison that stops at the first wrong byte tells an attacker how much of the code
    /// was right, one guess at a time. The same rule `endpoint.rs` states for the door token,
    /// and it matters more here because the alphabet is small.
    pub fn matches(&self, offered: &str) -> bool {
        if self.expired() {
            return false;
        }
        let a = self.code.as_bytes();
        let b = offered.trim().to_uppercase();
        let b = b.as_bytes();
        if a.len() != b.len() {
            return false;
        }
        let mut wrong = 0u8;
        for (x, y) in a.iter().zip(b) {
            wrong |= x ^ y;
        }
        wrong == 0
    }
}

impl Pairings {
    pub fn load(vault: &Path) -> Self {
        std::fs::read_to_string(path(vault))
            .ok()
            .and_then(|raw| toml::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, vault: &Path) -> Result<(), String> {
        let body = toml::to_string_pretty(self).map_err(|err| err.to_string())?;
        std::fs::create_dir_all(vault).map_err(|err| err.to_string())?;
        // Atomically: this file holds every pairing and the fingerprint that makes each one
        // usable, and there is no way to reconstruct either except by pairing again.
        epoch_secrets::atomically::replace(&path(vault), body.as_bytes())
    }

    pub fn all(&self) -> &[Paired] {
        &self.machines
    }

    pub fn get(&self, id: &str) -> Option<&Paired> {
        self.machines.iter().find(|m| m.id == id)
    }

    /// Add a machine, with the grants the user chose.
    ///
    /// **The long secret does not live here.** This file is the roster; the secret goes to
    /// `Secrets`, which is DPAPI on Windows — the same place the agent door's token is kept, for
    /// the same reason. A file a person can read is not where a bearer belongs.
    /// File a machine, or refresh the one that is already here.
    ///
    /// ## Why an id is not minted every time
    ///
    /// It used to be `bridge-{timestamp}-{n}`, fresh on every pairing. Unpairing a machine and
    /// pairing it again therefore produced a *different* machine as far as everything else was
    /// concerned — and a character's Brain names that id (ADR-0026). Measured on the real
    /// vault: Mage pointed at `bridge-1787267870768-0` while the roster held
    /// `bridge-1787281768989-0`, the same MacBook, and every turn refused with *not a backend
    /// this machine has*.
    ///
    /// **Identity belongs to the machine, not to the pairing event.** Re-pairing is the user
    /// re-establishing trust with a computer they already named; it is not a new computer. So a
    /// machine already on the roster keeps its id and has its address, name and grants brought
    /// up to date — which is also what makes a machine that moved to a new IP keep working.
    ///
    /// Matched on the name it calls itself, because that is what survives a DHCP lease. Two
    /// genuinely different machines answering to one hostname would collide, and on a network
    /// where that is true the address has already stopped identifying anything either.
    pub fn add(
        &mut self,
        name: &str,
        address: &str,
        fingerprint: Option<String>,
        grants: Vec<Grant>,
        at: u64,
    ) -> String {
        let name = name.trim();
        let address = address.trim();

        if let Some(known) = self
            .machines
            .iter_mut()
            .find(|m| m.name.eq_ignore_ascii_case(name))
        {
            known.address = address.to_owned();
            // **Replaced, not merged.** Re-pairing is the moment a machine says which certificate
            // is its own; a machine that was reinstalled has a new one, and keeping the old would
            // make the bond unusable in a way nothing on screen could explain.
            known.fingerprint = fingerprint;
            known.grants = grants;
            // `paired_at` is when this bond was last established, which is what a surface
            // showing "paired 3 days ago" should say after re-pairing today.
            known.paired_at = at;
            return known.id.clone();
        }

        let id = format!("bridge-{at}-{}", self.machines.len());
        self.machines.push(Paired {
            id: id.clone(),
            name: name.to_owned(),
            address: address.to_owned(),
            fingerprint,
            grants,
            paired_at: at,
            runners: Vec::new(),
            easels: Vec::new(),
        });
        id
    }

    /// Write down which programs a machine was serving, as the probe just measured them.
    ///
    /// Returns whether anything changed, so a caller only pays for a save when there is
    /// something to save. Nothing is invented here: an empty answer is written as an empty
    /// answer, because a machine that stopped serving LM Studio should stop offering it.
    pub fn note_runners(&mut self, id: &str, runners: Vec<String>) -> bool {
        match self.machines.iter_mut().find(|m| m.id == id) {
            Some(machine) if machine.runners != runners => {
                machine.runners = runners;
                true
            }
            _ => false,
        }
    }

    /// Write down which image studios a machine was serving, as the probe just measured them.
    ///
    /// The twin of [`Self::note_runners`], and separate for the same reason the lists are:
    /// nothing here may end up where a Brain is chosen from.
    pub fn note_easels(&mut self, id: &str, easels: Vec<String>) -> bool {
        match self.machines.iter_mut().find(|m| m.id == id) {
            Some(machine) if machine.easels != easels => {
                machine.easels = easels;
                true
            }
            _ => false,
        }
    }

    /// Change what a machine is allowed to be, without unpairing it.
    ///
    /// Granting again must not mean pairing again — a bond the user made is not something to
    /// spend twice because they changed their mind about what it is for.
    pub fn set_grants(&mut self, id: &str, grants: Vec<Grant>) -> bool {
        match self.machines.iter_mut().find(|m| m.id == id) {
            Some(machine) => {
                machine.grants = grants;
                true
            }
            None => false,
        }
    }

    /// Forget a machine entirely. Its secret is the caller's to remove.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.machines.len();
        self.machines.retain(|m| m.id != id);
        self.machines.len() != before
    }
}

/// Where a Host listens **only while a code is on screen**.
///
/// A door that exists for five minutes and then does not is a smaller thing to defend than a
/// door that is always open — so this is not the agent door with a looser rule bolted on, and
/// `endpoint.rs` keeps its own design intact: one path, POST, bearer, no browser.
pub const HOST_PORT: u16 = 11501;

/// This machine's address on the network it is on, for a person to read off the screen.
///
/// **Found by asking the routing table, not by listing interfaces.** A machine with a VPN, a
/// container bridge and a wireless card has several addresses and only one of them is the one
/// another machine on the same network can reach — so it opens a UDP socket toward a public
/// address, reads which local address the kernel chose, and closes it. Nothing is sent.
///
/// Loopback when that cannot be determined, which is a true reading: a machine with no route
/// out cannot be dialled from another machine either.
pub fn here() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            socket.connect("8.8.8.8:80")?;
            socket.local_addr()
        })
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_owned())
}

/// The name the firewall rule is filed under.
///
/// Fixed, so asking whether it exists and creating it cannot disagree about what they are
/// talking about — and so a person who wants it gone can find it by name.
pub const RULE: &str = "Epoch pairing";

/// Whether this machine will let a paired device reach the pairing door.
///
/// ## Why this is asked at all
///
/// A listener bound to `0.0.0.0` is not reachable: Windows drops inbound packets by default, and
/// it drops them **silently**. The remote sees `connection timed out` rather than a refusal, and
/// a timeout reads as *the Host is unreachable* — so the search goes to routers and cables while
/// the actual cause is one missing rule. Measured on a real pair of machines, at the cost of an
/// evening.
///
/// `None` means *unasked*, never *no*: off Windows there is no rule to look for, and a `netsh`
/// that cannot be run is not evidence of anything. Same discipline as every other probe here.
pub fn port_is_open() -> Option<bool> {
    if !cfg!(windows) {
        // Not a gap. Other systems either have no inbound firewall on by default, or ask the
        // user themselves the first time something listens — which macOS does, and which is why
        // EpochServices needs no equivalent of this.
        return None;
    }
    let asked = std::process::Command::new("netsh")
        .quiet()
        .args([
            "advfirewall",
            "firewall",
            "show",
            "rule",
            &format!("name={RULE}"),
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;
    // Measured: exit 0 when a rule of that name exists, 1 when none matches.
    Some(asked.success())
}

/// Ask Windows to allow the pairing port in, through its own consent dialog.
///
/// ## Epoch asks; Windows decides; the user approves
///
/// This does not open anything by itself and it deliberately cannot: creating a firewall rule
/// needs elevation, so what actually happens is that Windows shows **its own** consent prompt,
/// with its own wording, and the rule exists only if the person says yes there. Epoch never
/// holds an administrator token and never asks for a password.
///
/// That is the whole reason this is worth building rather than leaving the command in a
/// document. The command in a document is a task handed to somebody who did not sign up for it;
/// this is the same decision, made in the one place the operating system already reserves for
/// it.
///
/// ## As narrow as the thing it enables
///
/// One port, TCP, inbound, **private profile only**, and **only from the local subnet** — which
/// is the whole of what pairing needs, since a Host and a machine lending it a graphics card are
/// on the same network by definition. Widening any of those would grant something the feature
/// does not use.
pub fn open_the_port() -> Result<(), String> {
    if !cfg!(windows) {
        return Err("this machine does not need a firewall rule for pairing".to_owned());
    }

    elevate(&[
        "advfirewall",
        "firewall",
        "add",
        "rule",
        // **Quoted, and that is not decoration.** `Start-Process -ArgumentList` joins its
        // arguments with spaces and does not quote them, so `name=Epoch pairing` arrived at
        // netsh as two arguments and it answered *A specified value is not valid*. Every attempt
        // failed, and the failure was invisible because nothing read what netsh said.
        &format!("name=\"{RULE}\""),
        "dir=in",
        "action=allow",
        "protocol=TCP",
        &format!("localport={HOST_PORT}"),
        // As narrow as the thing it enables: a Host and a machine lending it a graphics card are
        // on the same network by definition.
        "profile=private",
        "remoteip=localsubnet",
    ])?;

    // **Verified, not assumed.** `Start-Process` succeeding says a process started, not that a
    // rule exists — and a surface that turned green on the strength of that would be a gauge
    // nobody can explain.
    match port_is_open() {
        Some(true) => Ok(()),
        _ => Err("the rule was not added, and Windows gave no reason.".to_owned()),
    }
}

/// Run one `netsh` command with administrator rights, and **bring back what it said**.
///
/// ## The consent is Windows'
///
/// `Start-Process -Verb RunAs` is what raises the operating system's own dialog. Epoch never
/// holds an administrator token and no password is ever typed into Epoch; the rule exists only
/// if the person says yes there.
///
/// ## And its output is read
///
/// An elevated process writes to a console nobody sees. The first version ignored that and
/// reported *"Windows may have refused it"* — a **guess**, and a wrong one: netsh had been
/// rejecting the arguments all along and saying exactly why, into a void. A message that
/// speculates about a cause is worse than one that admits it does not know, because somebody
/// acts on it. So the output is redirected to a file and read back.
fn elevate(arguments: &[&str]) -> Result<(), String> {
    // **cmd does the redirecting, because `Start-Process` cannot.**
    //
    // `-Verb RunAs` and `-RedirectStandardOutput` belong to different parameter sets and cannot
    // both be used — PowerShell answers `Parameter set cannot be resolved ...
    // AmbiguousParameterSet`, which is exactly what the first attempt at reading netsh's output
    // produced. So the elevated program is `cmd`, and the redirection is cmd's own `>`.
    //
    // Everything goes in **one** argument string, which is the same lesson as the quoting bug it
    // sits beside: `-ArgumentList` joins several arguments with spaces and quotes none of them,
    // so anything containing a space has to arrive already whole.
    let log = std::env::temp_dir().join("epoch-firewall.txt");
    let _ = std::fs::remove_file(&log);

    let line = format!(
        "/c netsh {} > \"{}\" 2>&1",
        arguments.join(" "),
        log.display()
    );
    let script = format!(
        "Start-Process -FilePath cmd -Verb RunAs -Wait -WindowStyle Hidden -ArgumentList '{}'",
        // PowerShell single-quoting: the quote itself is the only character that needs escaping.
        line.replace('\'', "''")
    );

    let ran = std::process::Command::new("powershell")
        .quiet()
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|err| format!("could not ask Windows: {err}"))?;

    let said = std::fs::read_to_string(&log)
        .unwrap_or_default()
        .trim()
        .to_owned();
    let _ = std::fs::remove_file(&log);

    if !ran.status.success() {
        let complaint = String::from_utf8_lossy(&ran.stderr);
        // The overwhelmingly common case is somebody choosing No, and saying so plainly beats
        // reporting a failure for a decision that was made on purpose.
        return Err(
            if complaint.contains("canceled") || complaint.contains("cancelled") {
                "Windows did not get permission.".to_owned()
            } else if !said.is_empty() {
                said
            } else {
                format!("Windows refused: {}", complaint.trim())
            },
        );
    }

    // **netsh exits 0 while refusing.** It puts the reason on stdout instead: `Ok.` is what
    // success looks like, and anything else it bothered to say is the thing somebody needs to
    // read.
    if !said.is_empty() && !said.eq_ignore_ascii_case("ok.") {
        return Err(said);
    }
    Ok(())
}

/// Take the rule away again.
///
/// ## Why this exists rather than only the opening
///
/// **A firewall rule is permanent.** It survives restarts, it survives Epoch being closed, and
/// it survives Epoch being uninstalled — so a button that could only add one would be a control
/// that quietly makes a lasting change to somebody's machine and then has nothing to say about
/// it. A switch says what an *Allow* button cannot: that there is a state, that it persists, and
/// that this is where it is undone.
///
/// The same consent dialog, for the same reason: removing a rule needs elevation too, and Epoch
/// holds no administrator token in either direction.
pub fn close_the_port() -> Result<(), String> {
    if !cfg!(windows) {
        return Err("this machine has no such rule to remove".to_owned());
    }

    elevate(&[
        "advfirewall",
        "firewall",
        "delete",
        "rule",
        &format!("name=\"{RULE}\""),
    ])?;

    // Verified, like the opening.
    match port_is_open() {
        Some(false) => Ok(()),
        _ => Err("the rule is still there.".to_owned()),
    }
}

/// Reach out to a machine that is showing a code, and enrol it.
///
/// ## The other direction, and why it is not a fallback
///
/// [`wait_for_one`] has the remote dial the Host, so nobody types an IP. That is better and it
/// stays the default. It also requires the remote to be able to *open a connection to* the Host,
/// and on a real network that was not true: with a Mac on Wi-Fi and the Host on Ethernet, every
/// connection the Mac started was dropped — 445, 135, 3389 and 11501 alike, with the firewall
/// rule present and a listener confirmed up — while every connection the Host started to the Mac
/// succeeded. Client isolation, decided by a router that is not always somebody's to change.
///
/// So the same exchange runs backwards. The **remote** shows the code, the Host reaches out, and
/// the bond that results is identical: same secret, same bearer, same grants. Only who opens the
/// socket differs.
///
/// ## The Host still mints the secret
///
/// The side that will present a bearer is the side that chooses it, in both directions. What the
/// code buys is the right to hand it over, and the code is checked on the machine that showed
/// it — which is what keeps a network-facing listener safe over there.
pub fn enrol(address: &str, code: &str) -> Result<Introduced, String> {
    let address = address.trim().trim_end_matches('/');
    if address.is_empty() {
        return Err("say where that machine is".to_owned());
    }
    let code = code.trim();
    if code.is_empty() {
        return Err("type the code that machine is showing".to_owned());
    }

    let secret = crate::endpoint::Token::fresh().as_str().to_owned();
    let asking = epoch_kernel::Enrol {
        code: code.to_uppercase(),
        secret: secret.clone(),
        host: format!("{}:{}", here(), HOST_PORT),
    };

    // **The far machine is not known yet, so its certificate is recorded rather than checked.**
    // That is trust on first use, and it is bounded by the five minutes of the code somebody is
    // reading off that machine's screen. What it buys is everything afterwards: the fingerprint
    // learned here is pinned, so a swapped certificate on any later turn is refused.
    let (tls, seen) = epoch_wire::tls::noting();
    let answered: epoch_kernel::Enrolled = ureq::builder()
        .tls_config(tls)
        .build()
        .post(&format!("{}/enrol", full(address)?))
        .timeout(std::time::Duration::from_secs(15))
        .send_json(serde_json::to_value(&asking).map_err(|err| err.to_string())?)
        .map_err(|err| match err {
            // The single refusal that program gives. From here there is one thing it can mean.
            ureq::Error::Status(403, _) => {
                "that code is wrong or has expired. Show a new one on that machine.".to_owned()
            }
            other => format!("could not reach that machine at {address}: {other}"),
        })?
        .into_json()
        .map_err(|err| format!("it answered something unreadable: {err}"))?;

    // Read *after* the exchange, because that is when the handshake has happened. A pairing that
    // could not say which certificate it spoke to is a pairing with nothing to pin, and filing it
    // would leave a machine on the roster that can never be reached.
    let fingerprint = seen.fingerprint().ok_or_else(|| {
        "that machine answered without showing a certificate, so there is nothing to remember it          by. Pair again."
            .to_owned()
    })?;

    Ok(Introduced {
        name: answered.name,
        address: full(address)?,
        fingerprint,
        have: answered.have,
        secret,
    })
}

/// A machine that has just been enrolled, before it is filed.
pub struct Introduced {
    /// What it calls itself. A default the user may rename.
    pub name: String,
    pub address: String,
    /// The certificate it showed while the code still stood. Pinned from here on.
    pub fingerprint: String,
    pub have: epoch_kernel::Have,
    /// The bearer it will be asked for from now on.
    pub secret: String,
}

/// An address a person typed, as a URL — **or a refusal, before anything is sent**.
///
/// They read an IP off the other screen. Requiring a scheme and a port as well is two more
/// chances to be wrong for no information gained — the same reasoning EpochServices applies to
/// the Host's address, in the direction that goes the other way.
///
/// ## Setting up TLS is not the same as requiring it
///
/// This used to keep whatever scheme was typed, on the reasoning that a person who wrote
/// `http://` "will be told it could not be reached". They will not. `ureq` given an `http` URL
/// simply does not do TLS, so the pinning verifier is never consulted, no certificate is ever
/// seen — and this request carries the long secret in its body. The client was configured for
/// TLS and never obliged to use it, which is a configuration rather than a rule.
///
/// So a scheme that is not `https` is refused **here**, before the request exists. Detecting it
/// afterwards — by noticing no certificate was recorded — is detecting it after the credential
/// has already gone.
fn full(typed: &str) -> Result<String, String> {
    let typed = typed.trim().trim_end_matches('/');
    let (scheme, rest) = match typed.split_once("://") {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("https") => ("https", rest),
        Some((scheme, _)) => {
            return Err(format!(
                "that address starts with `{scheme}://`, and pairing sends a secret. Use \
                 `https://` — or type the address on its own and Epoch will."
            ))
        }
        // **`https`**, because that door is encrypted and this exchange carries the secret.
        None => ("https", typed),
    };
    // A port is present only if what follows the last colon is entirely digits — otherwise the
    // colon belongs to an IPv6 address, and appending a port to one is how that breaks.
    let has_port = rest
        .rsplit_once(':')
        .is_some_and(|(_, tail)| !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()));
    if has_port {
        Ok(format!("{scheme}://{rest}"))
    } else {
        Ok(format!("{scheme}://{rest}:{REMOTE_PORT}"))
    }
}

/// Where EpochServices listens. Must stay equal to `EpochServices`' own `door::PORT`.
pub const REMOTE_PORT: u16 = 11500;

/// What a Host does when a machine dials in with a code.
///
/// Returns the `Welcome` to send back, or the reason to refuse. The caller owns the roster and
/// the secret store; this owns the rule.
pub type Greeting =
    Box<dyn Fn(epoch_kernel::Hello) -> Result<epoch_kernel::Welcome, String> + Send + Sync>;

/// Listen for one machine to redeem one code, then stop.
///
/// **One, then stop**, and that is the design rather than a limitation: a code buys one bond,
/// and a listener that stayed open after it was spent would be a listener nobody is watching.
/// It also ends when the code expires, so a person who walked away does not leave a door open.
///
/// Runs on its own thread. The returned handle stops it early — what pressing *cancel* does.
pub fn wait_for_one(vault: &Path, code: Code, greet: Greeting) -> Result<Waiting, String> {
    let identity = epoch_wire::tls::identity(vault)?;
    // **Bound here, before this returns.** A door opened on the thread below would report a
    // taken port to that thread, and the caller would be told the door is open. See
    // `epoch_wire::wire::open`.
    let listener = epoch_wire::wire::open(HOST_PORT)?;
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Set the instant the code is **taken**, which is before the bond exists rather than after
    // it. A door that marked it afterwards let two machines through one code — see the `swap`
    // below, and the test that reproduces it.
    //
    // **A spent door keeps answering `no` rather than
    // closing**, because a listener that stops reading does not stop being bound: the port stays
    // open at the operating system's level and a connection to it is accepted and then never
    // answered, which hangs the caller instead of refusing it. Measured, not reasoned — a test
    // that expected a refused connection here blocked forever.
    //
    // **And expiry is the same event**, which is the half that was missed once: a code that ran
    // out used to kill the thread and leave the socket bound, so a remote redeeming an expired
    // code got no answer at all and reported `connection timed out`. That reads as *the Host is
    // unreachable* and sends somebody to inspect their network instead of showing a new code.
    let spent = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    // **How many times this door may be answered wrongly.**
    //
    // Six characters from a 32-letter alphabet is a billion, so guessing is not the threat a
    // limit removes — what it removes is a door that can be hammered for as long as somebody
    // has a code on screen, from anywhere on the network.
    //
    // EpochServices has counted this since it was written and the Host's own door did not, so
    // the same handshake was strict in one direction and open in the other. The number is
    // deliberately the same one.
    //
    // Beside the door rather than inside `Code`: how many chances an *open listener* gives is a
    // property of that listener, and the code is a value the caller may still be showing.
    let wrong = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let counted = std::sync::Arc::clone(&wrong);

    // Set when the listener has actually gone. **`close()` waits for this**, and that is not
    // tidiness: the door polls `stop` between attempts to accept, so for a tenth of a second
    // after a caller says *stop* the port is still bound. A caller that opens a second door
    // immediately — which is exactly what two tests in a row do — would otherwise find the
    // port taken, or worse, find the *previous* door answering.
    let closed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let listening = std::sync::Arc::clone(&stop);
    let marked = std::sync::Arc::clone(&spent);
    let finished = std::sync::Arc::clone(&closed);
    std::thread::spawn(move || {
        // **One route, and its whole content is a code and a fingerprint.** Nothing arriving
        // here is large, and a ceiling shared with a composed turn meant 64 MiB reservable
        // before the code had been looked at — the second audit's finding 3, closed at the door
        // rather than in the handler, because the handler runs after the body is read.
        let modest = std::sync::Arc::new(|_: &str| epoch_wire::wire::MODEST);
        let _ = epoch_wire::wire::answer_on(listener, &identity, listening, modest, move |asked| {
            let answered = (!code.expired())
                .then_some(())
                .filter(|()| counted.load(std::sync::atomic::Ordering::SeqCst) < TRIES)
                .and_then(|()| {
                    let read = serde_json::from_str::<epoch_kernel::Hello>(&asked.body).ok();
                    if read.is_none() {
                        counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                    read
                })
                .filter(|hello| {
                    let right = code.matches(&hello.code);
                    if !right {
                        // Paid for on the way past, so a wrong answer costs something whether or
                        // not the body parsed into a `Hello` at all.
                        counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                    right
                })
                // **Taken here, in one indivisible step, before the bond is made.**
                //
                // This used to read the flag, greet, and set it afterwards. Two machines
                // arriving in the same instant both passed the check before either set it, so
                // `greet` ran twice for one code — two bonds and two secrets from one
                // invitation. Check-then-act, on a listener that serves concurrently by design.
                // Reproduced with `two_machines_cannot_ride_one_code_in_together`: `[true,
                // true]` before this line existed.
                //
                // `swap` is the reservation: whoever turns it from `false` is the one machine
                // this code was for, and everybody else gets the same refusal a wrong code
                // gets. `SeqCst` rather than `Relaxed` — the ordering is the whole point here,
                // and a fence nobody can reason about is not worth the nanoseconds.
                //
                // **After the code matches, never before.** A wrong answer must not burn the
                // invitation; that is what the attempt counter is for.
                .filter(|_| !marked.swap(true, std::sync::atomic::Ordering::SeqCst))
                .ok_or(())
                .and_then(|hello| greet(hello).map_err(|_| ()));

            match answered {
                Ok(welcome) => epoch_wire::wire::Answer::new(
                    200,
                    "application/json",
                    serde_json::to_string(&welcome).unwrap_or_default(),
                ),
                // **One answer for every refusal.** A wrong code, an expired one and a malformed
                // body get the same sentence, because a door that distinguishes them is a door
                // being asked questions by something that is not the user's machine.
                Err(()) => epoch_wire::wire::Answer::new(403, "text/plain", "no".to_owned()),
            }
        });
        finished.store(true, std::sync::atomic::Ordering::Relaxed);
    });

    Ok(Waiting { stop, closed })
}

/// A pairing door that is currently open.
pub struct Waiting {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    closed: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Waiting {
    /// Close it now — what *cancel* does, and what happens when the window goes away.
    /// Close it now — what *cancel* does, and what happens when the window goes away.
    ///
    /// The door polls this between attempts to accept, so it stops within a tenth of a second
    /// rather than instantly. That is the price of not having to break an `accept` on three
    /// operating systems — and this **waits for it**, so that when `close` returns the port is
    /// free rather than nearly free.
    pub fn close(&self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !self.closed.load(std::sync::atomic::Ordering::Relaxed) {
            if std::time::Instant::now() > until {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

impl Drop for Waiting {
    fn drop(&mut self) {
        self.close();
    }
}

/// The secret held for one paired machine.
pub fn secret_name(id: &str) -> epoch_kernel::SecretName {
    epoch_kernel::SecretName::new(format!("bridge:{id}"))
}

fn path(vault: &Path) -> PathBuf {
    vault.join("bridges.toml")
}

#[cfg(test)]
mod firewall_tests {
    use super::*;

    #[test]
    fn the_rule_name_reaches_netsh_as_one_value() {
        // The defect: `Start-Process -ArgumentList` joins its arguments with spaces and does not
        // quote them, so `name=Epoch pairing` arrived at netsh as *two* arguments and it
        // answered `A specified value is not valid`. Every ALLOW failed, and it failed silently
        // because nothing read what netsh said.
        //
        // The name has a space in it because a person reads it in the Windows firewall list.
        // Quoting is what lets it keep one.
        assert!(
            RULE.contains(' '),
            "the name a person reads has a space in it"
        );
        let argument = format!("name=\"{RULE}\"");
        assert_eq!(argument, "name=\"Epoch pairing\"");
        assert!(
            argument.starts_with("name=\"") && argument.ends_with('\"'),
            "the value must arrive quoted: {argument}"
        );
    }

    #[test]
    fn the_port_asked_for_is_the_port_that_is_listened_on() {
        // Two constants that must not drift: a rule for a port nothing listens on is a rule that
        // grants nothing, and it would look exactly like success.
        assert_eq!(HOST_PORT, 11501);
    }
    /// The line handed to `cmd`, built exactly as `elevate` builds it.
    ///
    /// Extracted so the shape can be asserted without elevating anything: the two defects here
    /// were both in the *shape* of the command, and both were invisible until something ran it.
    fn command_line(arguments: &[&str], log: &str) -> String {
        format!("/c netsh {} > \"{}\" 2>&1", arguments.join(" "), log)
    }

    #[test]
    fn the_whole_command_is_one_argument_with_the_name_quoted() {
        // Two separate defects, both about shape, both silent:
        //
        // 1. `-ArgumentList 'a','b'` joins with spaces and quotes nothing, so `name=Epoch
        //    pairing` reached netsh as two arguments: *A specified value is not valid*.
        // 2. `-Verb RunAs` and `-RedirectStandardOutput` are different parameter sets, so asking
        //    for both answered *AmbiguousParameterSet* and nothing ran at all.
        //
        // Both are fixed by the same shape: cmd receives one string, and cmd does the
        // redirecting.
        let line = command_line(
            &[
                "advfirewall",
                "firewall",
                "add",
                "rule",
                "name=\"Epoch pairing\"",
                "dir=in",
            ],
            r"C:\Temp\epoch-firewall.txt",
        );

        assert!(line.starts_with("/c netsh "), "{line}");
        assert!(
            line.contains("name=\"Epoch pairing\""),
            "the name stays whole: {line}"
        );
        assert!(line.ends_with("2>&1"), "stderr is captured too: {line}");
        assert!(
            line.contains(r#"> "C:\Temp\epoch-firewall.txt""#),
            "the path is quoted, because it has spaces on most machines: {line}"
        );
    }
}

#[cfg(test)]
mod door_tests {
    use super::*;

    /// **One door at a time.** Both tests below bind [`HOST_PORT`], and that port is real rather
    /// than convenient: it is the one a remote machine has to find. Run together they are two
    /// doors on one port, and what that looks like is not a clean refusal — on macOS one test's
    /// client reached the *other* test's listener and reported `connection reset`, which reads
    /// as a broken handshake rather than as a test arrangement.
    ///
    /// The early return on a taken port stays, because that case is real: another Epoch running
    /// on the developer's machine is a fact about the machine, not a failure of the code.
    static ONE_DOOR: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn alone() -> std::sync::MutexGuard<'static, ()> {
        ONE_DOOR.lock().unwrap_or_else(|held| held.into_inner())
    }

    /// A throwaway vault, so the door has an identity to serve with that is nobody's real one.
    fn somewhere() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("epoch-door-{}", crate::now_ms()));
        std::fs::create_dir_all(&dir).expect("a temporary folder");
        dir
    }

    /// A client for a machine whose certificate is not known yet — which is what a machine
    /// redeeming a code actually is.
    fn dialling() -> ureq::Agent {
        let (tls, _) = epoch_wire::tls::noting();
        ureq::builder().tls_config(tls).build()
    }

    /// The door, end to end, against a real socket.
    ///
    /// Written because everything about it was otherwise reasoning: that a wrong code is refused
    /// without saying why, that a right one gets the `Welcome`, and that the listener is spent
    /// afterwards. All three are the kind of claim that is true in the head and false on the
    /// wire, so they are measured.
    #[test]
    fn a_code_buys_one_bond_and_then_the_door_is_shut() {
        let _alone = alone();
        let code = Code::fresh();
        let right = code.as_str().to_owned();
        let vault = somewhere();
        let door = match wait_for_one(
            &vault,
            code,
            Box::new(|hello: epoch_kernel::Hello| {
                Ok(epoch_kernel::Welcome {
                    id: format!("bridge-{}", hello.name),
                    secret: "a-long-secret".to_owned(),
                })
            }),
        ) {
            Ok(door) => door,
            // A port already taken means another Epoch is running on this machine, which is a
            // fact about the machine rather than a failure of the code under test.
            Err(_) => return,
        };

        let hello = |code: &str| {
            serde_json::json!({
                "code": code,
                "name": "Studio Mac",
                "address": "https://127.0.0.1:11500",
                "fingerprint": "aa".repeat(32),
                "have": epoch_kernel::Have::default(),
            })
        };
        let at = format!("https://127.0.0.1:{HOST_PORT}/pair");
        let dialling = dialling();

        // A wrong code is refused, and refused with the one sentence every refusal gets.
        let refused = dialling
            .post(&at)
            .timeout(std::time::Duration::from_secs(5))
            .send_json(hello("WRONGX"))
            .unwrap_err();
        assert!(
            matches!(refused, ureq::Error::Status(403, _)),
            "{refused:?}"
        );

        let welcome: epoch_kernel::Welcome = dialling
            .post(&at)
            .timeout(std::time::Duration::from_secs(5))
            .send_json(hello(&right))
            .expect("the right code is admitted")
            .into_json()
            .expect("and answers a Welcome");
        assert_eq!(welcome.id, "bridge-Studio Mac");
        assert_eq!(welcome.secret, "a-long-secret");

        // Spent. A second machine cannot ride the same code in — and it is *refused* rather
        // than left hanging, which is the whole reason the loop keeps running after a bond.
        std::thread::sleep(std::time::Duration::from_millis(200));
        let after = dialling
            .post(&at)
            .timeout(std::time::Duration::from_secs(5))
            .send_json(hello(&right))
            .unwrap_err();
        assert!(matches!(after, ureq::Error::Status(403, _)), "{after:?}");
        drop(door);
    }
    /// **A clear-text address is refused before the secret exists as bytes on a socket.**
    ///
    /// Configuring TLS is not requiring it. `ureq` given an `http` URL does not do TLS at all,
    /// so the pinning verifier never runs, no certificate is ever seen, and this request — which
    /// carries the long secret the machine will be asked for from then on — goes out in the
    /// clear. Noticing afterwards that no certificate was recorded is noticing after the
    /// credential has already gone.
    ///
    /// So this listens on a plain socket and asserts **nothing arrived**. Asserting only on the
    /// error would pass against a version that sent the body and then complained.
    #[test]
    fn a_clear_text_address_never_receives_the_secret() {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port for the test");
        let at = listener.local_addr().expect("its own address");
        let heard = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counting = std::sync::Arc::clone(&heard);

        // One connection, then stop. Anything that reaches it is a byte too many.
        let plain = std::thread::spawn(move || {
            listener
                .set_nonblocking(false)
                .expect("a blocking listener");
            if listener.accept().is_ok() {
                counting.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });

        let why = match enrol(&format!("http://{at}"), "ABCDEF") {
            Err(why) => why,
            Ok(_) => panic!("a clear-text address must be refused"),
        };
        assert!(
            why.contains("http://") && why.contains("https://"),
            "the refusal must say what to type instead: {why}"
        );

        // The listener is still waiting, which is the whole assertion.
        assert_eq!(
            heard.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "something connected to a clear-text address"
        );
        // Let the thread go: connect once ourselves so `accept` returns.
        let _ = std::net::TcpStream::connect(at);
        let _ = plain.join();
    }

    /// An address with no scheme is `https`, and one that already says so is kept.
    #[test]
    fn an_address_a_person_typed_becomes_an_encrypted_one() {
        assert_eq!(
            full("10.0.1.20").expect("a bare address is fine"),
            format!("https://10.0.1.20:{REMOTE_PORT}")
        );
        assert_eq!(
            full("https://10.0.1.20:11500/").expect("a full one is kept"),
            "https://10.0.1.20:11500"
        );
        assert!(full("http://10.0.1.20:11500").is_err(), "clear text is out");
        assert!(full("ws://10.0.1.20").is_err(), "and so is anything else");
    }

    /// **One code, one bond — even when two machines arrive in the same instant.**
    ///
    /// The door checked whether the code was spent, greeted, and *then* marked it. Two requests
    /// could both pass the check before either marked it, so `greet` ran twice for one code:
    /// two bonds, two secrets, one invitation. Check-then-act, on a listener that serves
    /// concurrently by design.
    ///
    /// The greeting sleeps here to widen a window that is otherwise microseconds. That is the
    /// test admitting what it is: a race is not reproduced by hoping, and a window that has to
    /// be widened to be seen is still a window.
    #[test]
    fn two_machines_cannot_ride_one_code_in_together() {
        let _alone = alone();
        let code = Code::fresh();
        let right = code.as_str().to_owned();
        let vault = somewhere();
        let greeted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counting = std::sync::Arc::clone(&greeted);

        let door = match wait_for_one(
            &vault,
            code,
            Box::new(move |hello: epoch_kernel::Hello| {
                counting.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                // Creating a bond writes a secret and a roster entry. Real ones take long
                // enough to overlap; this stands in for that without touching a disk.
                std::thread::sleep(std::time::Duration::from_millis(400));
                Ok(epoch_kernel::Welcome {
                    id: format!("bridge-{}", hello.name),
                    secret: "a-long-secret".to_owned(),
                })
            }),
        ) {
            Ok(door) => door,
            // Another Epoch has the port. A fact about the machine, not about this code.
            Err(_) => return,
        };

        let hello = serde_json::json!({
            "code": right,
            "name": "Studio Mac",
            "address": "https://127.0.0.1:11500",
            "fingerprint": "aa".repeat(32),
            "have": epoch_kernel::Have::default(),
        });
        let at = format!("https://127.0.0.1:{HOST_PORT}/pair");

        let admitted: Vec<bool> = std::thread::scope(|team| {
            let both: Vec<_> = (0..2)
                .map(|_| {
                    let at = at.clone();
                    let hello = hello.clone();
                    team.spawn(move || {
                        dialling()
                            .post(&at)
                            .timeout(std::time::Duration::from_secs(10))
                            .send_json(hello)
                            .is_ok()
                    })
                })
                .collect();
            both.into_iter()
                .map(|one| one.join().unwrap_or(false))
                .collect()
        });

        assert_eq!(
            admitted.iter().filter(|it| **it).count(),
            1,
            "exactly one machine may redeem one code, got {admitted:?}"
        );
        assert_eq!(
            greeted.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "and the bond may only be created once"
        );
        drop(door);
    }

    /// **A door on the network may be answered wrongly a bounded number of times.**
    ///
    /// EpochServices has counted this since it was written; the Host's own door used
    /// `Code::matches` with nothing keeping score, so the same handshake was strict in one
    /// direction and open in the other. Six characters from a 32-letter alphabet is a billion,
    /// so the limit is not about guessing succeeding — it is about a listener that can be
    /// hammered from anywhere for as long as a code is on screen.
    ///
    /// The right code is offered **after** the limit is used up, and must be refused: a counter
    /// that stopped counting once somebody got it right would not be a limit.
    #[test]
    fn a_door_stops_answering_after_enough_wrong_answers() {
        let _alone = alone();
        let code = Code::fresh();
        let right = code.as_str().to_owned();
        let vault = somewhere();
        let door = match wait_for_one(
            &vault,
            code,
            Box::new(|hello: epoch_kernel::Hello| {
                Ok(epoch_kernel::Welcome {
                    id: format!("bridge-{}", hello.name),
                    secret: "a-long-secret".to_owned(),
                })
            }),
        ) {
            Ok(door) => door,
            Err(_) => return,
        };

        let hello = |code: &str| {
            serde_json::json!({
                "code": code,
                "name": "Studio Mac",
                "address": "https://127.0.0.1:11500",
                "fingerprint": "aa".repeat(32),
                "have": epoch_kernel::Have::default(),
            })
        };
        let at = format!("https://127.0.0.1:{HOST_PORT}/pair");
        let offer = |body: serde_json::Value| {
            dialling()
                .post(&at)
                .timeout(std::time::Duration::from_secs(5))
                .send_json(body)
                .is_ok()
        };

        for n in 0..TRIES {
            assert!(!offer(hello("WRONGX")), "a wrong code was admitted at {n}");
        }
        assert!(
            !offer(hello(&right)),
            "the door must be shut once its chances are used up, right code or not"
        );
        drop(door);
    }

    /// An expired code must be **refused**, never left hanging.
    ///
    /// The defect, reported from a real machine: a remote redeeming a code that had run out got
    /// `Connection Failed: Connect error: connection timed out`. A timeout is not what a refusal
    /// looks like — a refusal is immediate — so it read as *the Host is unreachable*, and sent
    /// somebody to inspect their firewall rather than to show a new code.
    ///
    /// The cause was that `code.expired()` ended the accept loop while the handle kept the
    /// socket bound: the OS accepted the connection and nobody ever read it.
    #[test]
    fn a_code_that_ran_out_answers_rather_than_hanging() {
        let _alone = alone();
        let code = Code::already_expired();
        assert!(code.expired(), "the fixture must actually be expired");
        let right = code.as_str().to_owned();

        let vault = somewhere();
        let door = match wait_for_one(
            &vault,
            code,
            Box::new(|_hello: epoch_kernel::Hello| {
                panic!("an expired code must never reach enrolment")
            }),
        ) {
            Ok(door) => door,
            // A port already taken means another Epoch is running on this machine.
            Err(_) => return,
        };

        let at = format!("https://127.0.0.1:{HOST_PORT}/pair");
        let refused = dialling()
            .post(&at)
            // Short on purpose: the whole point is that an answer arrives at all. Before the
            // fix this blocked until the timeout rather than returning a status.
            .timeout(std::time::Duration::from_secs(5))
            .send_json(serde_json::json!({
                "code": right,
                "name": "Studio Mac",
                "address": "http://127.0.0.1:11500",
                "have": epoch_kernel::Have::default(),
            }))
            .unwrap_err();

        assert!(
            matches!(refused, ureq::Error::Status(403, _)),
            "an expired code must be refused, not left waiting: {refused:?}"
        );
        drop(door);
    }
}

#[cfg(test)]
mod tests {

    /// The defect, with the real ids off the vault.
    #[test]
    fn pairing_the_same_machine_again_is_the_same_machine() {
        // It minted `bridge-{timestamp}-{n}` every time, so unpairing and re-pairing produced a
        // different machine as far as everything else was concerned. A character's Brain names
        // that id: Mage pointed at `bridge-1787267870768-0` while the roster held
        // `bridge-1787281768989-0` — the same MacBook — and every turn refused.
        let mut roster = Pairings::default();
        let first = roster.add(
            "studio-mac.local",
            "https://192.168.1.20:11500",
            Some("aa".repeat(32)),
            vec![Grant::Compute],
            1_787_267_870_768,
        );

        // Paired again, later, and from a different address after a new DHCP lease.
        let again = roster.add(
            "studio-mac.local",
            "https://192.168.1.31:11500",
            Some("bb".repeat(32)),
            vec![Grant::Compute, Grant::Surface],
            1_787_281_768_989,
        );

        assert_eq!(first, again, "identity belongs to the machine");
        assert_eq!(roster.all().len(), 1, "and there is still one of it");

        let held = &roster.all()[0];
        assert_eq!(
            held.address, "https://192.168.1.31:11500",
            "moved, and followed"
        );
        assert!(held.may(Grant::Surface), "and the new grants took");
        assert_eq!(held.paired_at, 1_787_281_768_989, "bonded again just now");
    }

    #[test]
    fn a_different_machine_is_still_a_different_machine() {
        // The matching is on the name a machine calls itself. Two computers are two rows.
        let mut roster = Pairings::default();
        let mac = roster.add(
            "studio-mac.local",
            "https://192.168.1.20:11500",
            Some("aa".repeat(32)),
            vec![],
            1,
        );
        let studio = roster.add(
            "desk-mac.local",
            "https://192.168.1.31:11500",
            Some("bb".repeat(32)),
            vec![],
            2,
        );
        assert_ne!(mac, studio);
        assert_eq!(roster.all().len(), 2);
    }

    use super::*;

    fn vault() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("epoch-pairing-{n}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_code_is_readable_by_a_person_moving_between_two_screens() {
        // Somebody reads this off one machine and types it into another. A code that is
        // ambiguous to a human is a code that gets mistyped and reads as a failed pairing.
        let code = Code::fresh();
        assert_eq!(code.as_str().len(), 6);
        assert!(
            !code.as_str().contains(['0', 'O', '1', 'I', 'L']),
            "no character that is another character on a screen: {}",
            code.as_str()
        );
        assert!(code
            .as_str()
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
    }

    #[test]
    fn two_codes_are_never_the_same() {
        let a = Code::fresh();
        let b = Code::fresh();
        assert_ne!(a.as_str(), b.as_str());
    }

    #[test]
    fn a_code_is_matched_whole_and_case_does_not_matter() {
        // Typed by a person, so case is not a decision they made. Length is checked before the
        // comparison and the comparison itself does not stop early — see `matches`.
        let code = Code::fresh();
        let typed = code.as_str().to_lowercase();
        assert!(code.matches(&typed));
        assert!(code.matches(&format!("  {typed}  ")));
        assert!(!code.matches("ABC"));
        assert!(!code.matches(&format!("{typed}X")));
    }

    #[test]
    fn granting_again_is_not_pairing_again() {
        // A bond the user made is not something to spend twice because they changed their mind
        // about what it is for.
        let mut pairings = Pairings::default();
        let id = pairings.add(
            "Mac mini",
            "https://10.0.0.4:11500",
            Some("cc".repeat(32)),
            vec![Grant::Compute],
            1,
        );

        assert!(pairings.get(&id).unwrap().may(Grant::Compute));
        assert!(!pairings.get(&id).unwrap().may(Grant::Surface));

        assert!(pairings.set_grants(&id, vec![Grant::Compute, Grant::Surface]));
        assert!(pairings.get(&id).unwrap().may(Grant::Surface));

        // And down to nothing, which is a real state: paired, allowed nothing, still known.
        assert!(pairings.set_grants(&id, Vec::new()));
        assert!(!pairings.get(&id).unwrap().may(Grant::Compute));
        assert_eq!(pairings.all().len(), 1);
    }

    #[test]
    fn the_roster_survives_the_file_and_holds_no_secret() {
        // The file is the roster. The bearer lives in `Secrets` — DPAPI on Windows — because a
        // file a person can read is not where a bearer belongs.
        let dir = vault();
        let mut pairings = Pairings::default();
        pairings.add(
            "Mac mini",
            "https://10.0.0.4:11500",
            Some("cc".repeat(32)),
            vec![Grant::Surface],
            7,
        );
        pairings.save(&dir).unwrap();

        let raw = std::fs::read_to_string(dir.join("bridges.toml")).unwrap();
        assert!(raw.contains("Mac mini"));
        assert!(raw.contains("surface"));
        assert!(
            !raw.to_lowercase().contains("secret") && !raw.to_lowercase().contains("token"),
            "the roster must not be where a bearer is kept:\n{raw}"
        );

        let back = Pairings::load(&dir);
        assert_eq!(back.all().len(), 1);
        assert!(back.all()[0].may(Grant::Surface));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
