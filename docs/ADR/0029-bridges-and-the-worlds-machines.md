# ADR-0029: Bridges — a World spans machines, and one of them owns reality

- Status: Accepted
- Date: 2026-08-19
- Depends on: [[0003-engine-presentation-separation]], [[0007-provider-abstraction]], [[0009-trust-engine]], [[0012-context-composer]], [[0026-characters-are-portable-assets]], [[0027-brains-models-and-agents]]
- Amends: nothing. A Bridge is a `Provider` whose transport happens to be another machine.
- Related architecture: [[../../PRODUCT_ARCHITECTURE]], [[../../CLAUDE]]
- Horizon: the Host/Bridge split, pairing, the stateless rule, the two grants and the disclosure question **IMPLEMENT NOW**; the Bridge Console and a remote surface **DESIGN NOW**; discovery on a LAN and scheduling work across machines **VISION**, and see *Refused* below

## Context — what is already true

More of this exists than a phase plan implies, and saying so first keeps the ADR from inventing
a problem.

A Provider is already an HTTP endpoint (ADR-0007), and Ollama already listens on one. Pointing
Epoch at `http://192.168.1.45:11434` is a backend entry **today** — no new protocol, no new
concept. Phase 8 added an OpenAI-compatible backend, which covers llama.cpp, LM Studio, vLLM and
most of what a second machine actually runs.

So a Bridge must not be a protocol for something that is already a URL. What it earns its
existence for is the part a bare network port does not have:

> **identity, authentication, what it can do, and what happens when it is gone.**

And one measured fact decides the security half: **a bare Ollama on a LAN has no authentication
at all.** Anything on the network can use somebody's graphics card, read which models they have,
and load one. That is not a flaw in Ollama — it was built for `localhost` — but it is the whole
of why "just point Epoch at the IP" is not the answer.

## Decision

### 1. The Host owns reality. The Bridge owns computation.

> **Only one Runtime exists per World.** Every Quest, Chronicle, Knowledge object, Trust decision,
> MCP interaction and Project Root belongs to it. A Bridge contributes computation and never owns
> World state.

"Computation" rather than "inference" on purpose: a Bridge may end up doing embeddings, speech or
diffusion, and none of those are inference. What it never owns is any part of the World.

This is what keeps the whole thing cheap. A second machine adds capacity and **learns nothing** —
so there is no synchronisation, no conflict resolution, no split brain, and unplugging it loses
exactly the turn that was in flight.

### 2. A Bridge is a `Provider`, and that is the entire protocol

`Request → Answer` already is the boundary: model, conversation, parameters and tool declarations
going out; text and tool calls coming back, with `Chunk` for streaming. The Composer builds the
whole conversation every turn (ADR-0012), which is what makes the next rule free rather than
aspirational:

**The Bridge is stateless.** A tool result arrives as another message in the next `Request`, not
as a continuation of a session the Bridge is holding. Nothing to resume, nothing to lose.

A `take_turn()` that took *a turn* would have to understand tools, capabilities and Quests — and a
machine that understands those has started owning World state. The narrow interface is the
security boundary, not a simplification of one.

### 3. The Bridge never executes a tool

It **asks**; the Host judges through Trust (ADR-0009) and runs it. A Bridge running `write_file`
on another machine is precisely the hole ADR-0027 closed, reopened across a network — and it
would run as *that machine's* user, in *that machine's* filesystem, under a Trust decision the
user made about somewhere else.

### 4. Pairing, not an API key

A short code shown on the remote machine, exchanged **once** for a long secret kept in DPAPI on
both sides. The code expires in minutes; the bond lasts.

A code is guessable and lives on a screen, so it is the introduction and never the credential.
This is the same rule the agent door already follows and states in `endpoint.rs`: **loopback is
not authentication, and neither is a LAN.**

### 5. The remote owns its own models

The Host never configures them. It asks what is there, the way it asks every other backend — and
the Workshop's rule applies unchanged: measured, or visibly unknown.

### 6. The disclosure question, asked once and honestly

A composed turn carries the user's source, their Chronicle and their knowledge. Three
destinations, three different decisions, and only the user can make them:

| where | what it means |
|---|---|
| this machine | nothing leaves |
| another machine they own | it crosses their network |
| a hosted provider | it reaches a third party |

ADR-0025 already named this for hosted providers. This is the same decision with a different
destination, and it must not be buried in *"the World has a second machine now"*.

### 7. A fallback that fires is a fact, not plumbing

**The Runtime never chooses a Brain.** A character owns theirs (ADR-0026 — canonical parameters
are behavioural identity), so when a Bridge is unreachable Epoch says the assigned Service cannot
be reached and offers to retry, to pick another, or to stop. If the user picks another, **the
Chronicle records that they did.**

A silent substitution is a character behaving differently with nobody told. A gauge nobody can
explain is worse than no gauge; a *character* nobody can explain is worse still.

### 8. Two directions, two grants — and they are not the same thing

Asked whether the remote machine could reach **the Host's** terminal — an `ssh` into the World —
the answer is yes, and it is **not the Bridge doing it**. Folding it into the Bridge is what
would break rule 3, so the two are named apart:

| | who acts | what it touches | what it needs |
|---|---|---|---|
| **Bridge** | a model on the remote machine | nothing of the World | *this machine may think for me* |
| **Remote surface** | **a person**, sitting at the remote machine | the Host's World, through the Host | *from this machine I may drive my World* |

A remote surface is a fourth Experience Surface (`PRODUCT_ARCHITECTURE` — the Launcher prepares,
the World immerses, the CLI automates). Nothing about the Runtime changes: `run_command` still
runs **on the Host**, is still judged by the Host's Trust, and is still recorded in the Host's
Chronicle. What travels is the *screen*, not the decision.

**They are separate grants over one pairing, and that is the security-relevant part.** If one
secret bought both, a compromised compute machine would have the user's terminal. *"This machine
may think for me"* and *"from this machine I may drive my World"* are two sentences, and the
second is far larger — so it is granted separately, shown separately, and revocable separately.

### 9. The Bridge is a program, and it is **not** Epoch

Pairing without something on the far end that can check a secret is theatre. A bare Ollama on a
network accepts every request — it has no notion of a bearer — so a roster and a token on the
Host protect nothing: they record who the user *believes* they are talking to.

So the remote machine runs **EpochServices**: a small companion, cross-platform like Epoch, and
the reason it earns its existence is exactly the list in *Context*. It is what turns "an address
on a network" into "a machine I paired with".

**And the security win is the part to say out loud: Ollama stays on `127.0.0.1`.** Nothing is
exposed to the network at all. EpochServices is the only listener, and it is the one that knows
what a secret is. Without it, making a second machine useful means `OLLAMA_HOST=0.0.0.0`, and
then anything on that network can use somebody's graphics card.

**It may not link the Engine, and the compiler holds that.** EpochServices depends on
`epoch-kernel` — canonical types, zero I/O — and on nothing else of Epoch's. Linking
`epoch-engine` would put Quests, Chronicles, Knowledge, Trust and MCP inside a program that this
ADR says must own none of them; the boundary would then be a rule somebody has to remember
rather than one the build enforces. Same argument the Kernel/Engine split already makes one
level down.

The HTTP bits are small and are written again there rather than shared. That is the trade, and
it is the cheap side of it.

### 10. The remote dials in once, and the user never sees an address

The obvious flow asks the user for an IP. Measured against what a person actually has to do, it
asks for four things: expose Ollama, find the address, get past a firewall, and keep it working
after a reboot changes the address.

So the pairing goes the other way:

```text
Host      Services → ADD A DEVICE → shows  K7M2QX
Remote    open EpochServices → paste K7M2QX
          ↳ the remote dials the Host, redeems the code, and in the same
            exchange reports its own address and what it has
Host      "MacBook Pro · qwen3:14b, qwen3:8b · 16 GB"
```

Turns then flow Host → remote, which is rule 2 unchanged. What this removes is the user ever
typing an address: the remote knows its own, and re-reports it whenever EpochServices starts —
so a DHCP lease that changes overnight fixes itself instead of becoming a Bridge that is
mysteriously offline.

### 11. The secret at rest, on three operating systems

Epoch's own `secrets.rs` is **DPAPI, and Windows only** — off Windows it refuses rather than
pretending. EpochServices has to run on all three, so it keeps its bearer in a file inside its
own data directory, `0600` on Unix and in the user's profile on Windows.

That is weaker than DPAPI or Keychain and it is proportionate: the secret grants what a bare
Ollama already grants to the whole network today — the use of a graphics card. Anybody who can
read a `0600` file in the user's home directory can already read far more interesting things.

It is one function, and a platform keystore replaces it without changing anything above.

## Refused, and why

**No automatic failover, load balancing or "fastest available".** These are the obvious features
and they are the ones that break the product: they make the Runtime choose a Brain, which makes
Mage quietly become another Mage. The user assembles their crew deliberately and the Runtime
executes that decision rather than reinterpreting it.

**No heartbeat and no latency reading.** Pinging N machines forever is work nobody watches, and
the number would measure how fast a backend says *"I am here"* while reading as how fast it
thinks. The Launcher probes when it is open; a turn discovers a dead backend by failing honestly.

**No World state on a Bridge.** Not even a cache of it. A cache is a second copy, and a second
copy is a thing that can disagree.

## Consequences

- A second machine is additive: buy it, pair it, assign somebody to it. Nothing is reinstalled and
  no Quest moves.
- Unplugging a Bridge costs the turn in flight and nothing else — which is a claim this design can
  make because of rule 2, not despite it.
- The Host is a single point of failure for the World, deliberately. One source of truth is worth
  more than availability here: this is somebody's desktop, not a datacentre.
- Epoch gains a network listener, which is why Phase 7's security half came first: a strict CSP, a
  DPAPI-held token and no lock panic reachable from IPC are the preconditions for putting anything
  on a LAN at all.

## Assumptions

- The machines are the user's own and on a network they control. Bridging across the internet is
  not refused; it is simply not what this is designed for, and the disclosure question is where
  somebody would notice the difference.
- A remote backend speaks HTTP. Everything measured so far does.

## Risks

- **Pairing is the whole of the security.** If the exchange leaks the long secret, a Bridge is an
  open graphics card on a network. It is written once, over a channel the user can see, and kept
  where the door token is kept.
- **A Bridge that lies about its models** wastes a turn and produces an honest failure. Acceptable:
  the Host asked and reported what it was told, which is what every other probe does.

## Answered while writing this (2026-08-19)

**A Bridge may run capabilities that touch only its own machine** — a terminal on the remote box,
its own files. The owner's reasoning, and it is the right one: *that is what the pairing is for.*

This is not an exception to rule 3, and the difference matters. Rule 3 refuses a Bridge acting on
**the Host's** World — writing the user's project, reaching their MCP servers, deciding on their
behalf. A terminal on the remote machine touches nothing of the World: the Host still owns every
Quest, Chronicle and Trust decision, and still judges whether the capability may run at all.

Two things follow, and both are already true here:

- **Trust is still the Host's.** The Bridge asks, `decide()` answers, and the answer is recorded
  in the Host's Chronicle. What changes is only *where the effect lands*, and that is exactly what
  a capability descriptor already carries (ADR-0008: effects and reversal).
- **The pairing is what makes it safe.** An unpaired machine cannot be asked and cannot ask.
  Nobody gets in between, which is the property a shared secret exchanged once and kept in DPAPI
  buys — and the reason a bare LAN endpoint was never enough.

What it changes for the surface: a capability that runs *there* must say so. "Ran `cargo test`"
and "ran `cargo test` on the Mac mini" are different sentences, and History records what happened.

## Open questions

- Whether discovery on a LAN is worth the surface it adds when adding by address already works.
- What a remote surface may do **while the Host is asleep**. Waking a machine to answer a
  keystroke is a different feature, and pretending the World is there when the Host is not would
  be the World lying.


---

## Amendment (2026-08-19) — §12: the cable, connected

Everything above described a bond. Building the two ends and running them against each other
changed four things, and every one of them was found by a socket rather than by reading.

### The code and the door are one action

The Host used to mint a code and wait for the *user* to type the remote's name and address into
Epoch. That is backwards: the remote knows its own address and the user does not. So showing a
code now **opens a listener** (`pairing::wait_for_one`, port `11501`) and the remote dials in
with a `Hello` carrying its own address. Cancelling closes it, and so does leaving the panel.

The door exists only while a code is on screen. **A door that outlives the thing it was opened
for is a door nobody is watching** — the same reasoning as spending the code itself.

The manual form stays, as the fallback for a machine not running EpochServices. It is no longer
the path.

### A spent listener is not the same as a closed socket

The first version returned from its thread after minting one bond. The test written to prove
"a second machine cannot ride the same code in" **hung forever** instead of being refused: the
`Server` was still bound, the OS still accepted the connection, and nothing was left reading it.

So a spent door keeps looping and keeps answering `403` until it is dropped. One code still buys
one bond; the difference is that the refusal is now delivered rather than implied.

This is the general shape worth keeping: *stopping to read is not the same as stopping to
listen*, and only a client on the other end can tell you which one you built.

### One enrolment, two ways in

The panel's `pair` and the network door would have been two ways for a bond to come into
existence, which is two sets of rules to keep in agreement. Both now go through one free function
(`enroll`) that files the roster entry and mints the bearer. It takes no `&self` deliberately —
the door runs on its own thread, and reaching for state the turn loop also holds is the mistake
that froze the window once already (`std::sync::Mutex` is not reentrant).

### A Bridge is a Provider, and joins the registry with the rest

`bridge::Bridge` implements `Provider` and nothing else: the Composer builds the same
Conversation, Trust judges the same calls, the Chronicle keeps the same record. Two properties
are asserted rather than assumed:

- **`local` is false.** It costs no money and it is still somebody else's computer, and `local`
  is what the disclosure question keys on. Calling it "this PC" would be the surface lying about
  where a turn went.
- **One chunk, not fake streaming.** The remote answers whole, so the World shows it whole.
  Streaming belongs on the far side first; interpolating tokens here would be the UI inventing
  information the Engine does not have.

Bridges are added inside `Backends::registry` rather than by each caller. The registry answers
*what can currently think*, and a surface that had to ask a second question would eventually
forget to.

A machine without `Grant::Compute`, or whose secret cannot be read, is **not built at all** —
the same reason a disabled backend is not built: a Provider that exists can be probed by
accident.

### Two ports, and the person types neither

The Host's door is `11501`; EpochServices is `11500`. They run on different machines and both
would otherwise be "11500" in the user's head — so EpochServices fills in scheme and port from
whatever was typed, and the Host shows its own address beside the code. What a person reads off
one screen and types into the other is an IP and six characters.

(The port constant is duplicated rather than shared: a port is a fact about a program, not a
domain type, and EpochServices may not link the Engine.)


---

## Amendment (2026-08-19) — §13: the companion, on a machine that is not this one

EpochServices was written on Windows against a `cfg(unix)` branch nobody had run and a `sysctl`
call nobody had made. It was built and exercised on an M2 MacBook Pro, and the result is worth
recording because so little of it was a surprise — which is itself the finding.

**It built with no changes.** One `cargo tauri build`, producing a 3.75 MB `.dmg`. The decision
that made this cheap was the one that felt extravagant at the time: the page is a Rust string
served by the listener the program already had to be, so there is no bundler, no `ui/` folder and
nothing to keep in step with Epoch's frontend. A second frontend build would have been the thing
that broke on a second operating system.

**Two tests ran for the first time.** `#[cfg(unix)]` code is not code until a Unix runs it: the
`0600` assertion on the bond file and the "a machine with no NVIDIA card reports no VRAM rather
than zero" case, which had only ever been checked on a machine that has one. Both passed. The
second matters more than it looks — it is the unified-memory path, and the page reported *17 GB
memory* on a 16 GB machine, which is `hw.memsize` in decimal rather than an invented split.

**The door held on the first request.** `GET /have` without a bearer answered `403` on a machine
whose Ollama is not even installed — and the page said so plainly rather than showing an empty
list.

**A signature nobody asked for.** The bundle is `adhoc, linker-signed`, applied by the toolchain
because Apple Silicon requires it. Worth writing down because "unsigned" was about to be
documented, and it was wrong: what is missing is a Developer ID and notarisation, not a
signature. And a `.dmg` built locally carries no `com.apple.quarantine` at all — the first-launch
warning is a property of *where a file came from*, not of the bundle.


---

## Amendment (2026-08-19) — §14: the machine that runs the model is the one that must be asked

Two models were pulled onto a 16 GB MacBook: 17.7 GB and 18.0 GB. Neither fits in that machine's
memory with the operating system switched off. The downloads succeeded and nothing said a word.

Epoch's Models Workshop asks the right question about the wrong machine. **A verdict measured on
the Host is worse than no verdict** — it is confident, and it is about a different computer with
a graphics card the other one does not have.

So EpochServices has a Workshop, and it is *the same* Workshop: `epoch-models` is a crate holding
what was `models.rs` and `machine.rs`, which turned out to be entirely uncoupled from the Engine.
EpochServices may not link `epoch-engine` — that boundary is why this program exists — and the
alternative was a second implementation of weighing, searching and pulling.

**The extraction justified itself within the hour.** `here.rs` already had its own `nvidia-smi`
call and its own `sysctl`, and they were not identical to the Engine's: the shared one read
`/proc/meminfo` on everything that was not Windows, so it reported nothing at all on an Apple
machine while the local copy reported 17 GB. Two answers to one question, and the Workshop used
the wrong one. *"Not Windows" was never a platform.*

**Unified memory is compared against system memory and said to be.** There is no VRAM on that
machine and inventing a video share would describe a division the hardware does not have. The
same tenth is left over as for a card: a model that exactly fills the memory it runs in does not
run.

**A model that does not fit can still be pulled.** The button says PULL ANYWAY and the sentence
says what will happen. Epoch does not decide for somebody who already decided — what it refuses
is for that to happen silently, which is what happened before this existed.

### And the firewall stopped being the user's homework

A listener bound to `0.0.0.0` is not reachable: Windows drops inbound packets silently, so the
remote reports `connection timed out` and reads it as *the Host is unreachable*. The fix was a
`netsh` line in a document — a task handed to somebody who did not sign up for it, to explain a
failure that named the wrong cause.

Epoch asks; **Windows shows its own consent dialog**; the user approves there. Epoch never holds
an administrator token and no password is ever typed into Epoch. It is a **switch** rather than a
button, because a firewall rule outlives Epoch being closed and Epoch being uninstalled — a
control that could only *allow* would make a permanent change with nowhere to undo it. Both
directions re-read the rule afterwards rather than trusting that they asked.


---

## Amendment (2026-08-19) — §15: two directions, because one of them is not always available

§9 had the remote dial the Host, so nobody types an IP. A machine knows where it is; asking a
person to find out is asking them to do the computer's job. That reasoning is still right and it
is still the default.

It also assumed the remote *can* open a connection to the Host, and on a real network it could
not. Measured, with the firewall rule present and a listener confirmed up:

| | |
|---|---|
| Mac → Host, port 11501 | timeout |
| Mac → Host, ports 445 · 135 · 3389 · 5040 | timeout, all of them |
| Mac → Host, IPv6 link-local | timeout |
| **Host → Mac**, IPv4, port 22 | **connects** |
| ARP, Mac → Host | resolves |

Nothing Epoch does was involved: ports with no relation to it failed the same way. That is client
isolation — a wireless client may not start a connection to the wired network — and it is the
router's decision, not always the user's to change.

**So the same exchange runs backwards.** The remote shows the code; the Host reaches out. The
bond is identical — same secret, same bearer, same grants, filed through the same `enroll` —
because all that differs is who opens the socket.

**The code moves with the direction.** It is generated and checked on the machine that shows it,
because that is the machine whose listener is now reachable by anything on the network. Its
constant-time comparison stops being a nicety there.

**It is offered before either direction fails.** Somebody who already knows their network
isolates its clients should not have to watch a timeout to learn the other door exists. A
fallback revealed only by a failure is a fallback most people never find.

Measured end to end across the isolating network: a wrong code answers `403`, the right one
returns the machine's name and hardware, the bearer it was given opens `/have`, a wrong bearer
does not, the code is spent on first use — and `gemma4:12b` on the MacBook answered a turn asked
from the Host.

### What this replaced

A manual *"add by hand"* form that filed a roster entry and minted a secret **on the Host**. The
other end never learned it and refused every request afterwards. It looked like adding a machine
and produced one that could not be used — an option that is worse than not having one, because
its failure arrives later and somewhere else.

## Amendment (2026-08-20) — a machine is not one brain

A Bridge assumed a lent machine ran exactly one program, and named it in one line:
`const OLLAMA: &str = "http://127.0.0.1:11434"`. That was true for as long as Ollama was the only
runtime Epoch could reach. It stopped being true the day `llama.cpp` and `LM Studio` became things
a person could install from inside Epoch — and the MacBook now serves all three at once, measured:
Ollama on `11434` holding `Qwen3.8-27B`, LM Studio on `1234` holding `gemma-4-e4b`, `llama-server`
on `8080` holding nothing.

The shape a person means is **Provider · Brain · Model**: the device, the program that runs the
model, the model. Two of the three already existed — a Provider id and a model name. The middle
one did not, so `macbook + LM Studio` was not something Epoch could say.

**A machine is several Providers, not a Provider with a new field.** One Provider per
(machine, program). This is not a workaround: it is what the local side has always looked like —
`ADD AS A SERVICE` on LM Studio here makes a second Backend at a second address on the same
computer, and the character editor already groups Services by machine. A remote machine behaving
differently would have been the odd one out. The default keeps the id it always had
(`bridge:<machine>`), so every character saved before this still resolves; the extra ones are
`bridge:<machine>:<runner>`.

**A turn names *which* program, never *where* it is.** `Ask.runner` carries an id from a closed
set (`ollama`, `llama_cpp`, `lm_studio`); the far machine resolves it to one of its own loopback
ports. A request that could carry a URL would be a request that could point EpochServices at
something else — which is the single property that makes a companion program safer than
`OLLAMA_HOST=0.0.0.0` (§9), and widening the wire would have quietly given it away. An id the far
machine does not recognise is **refused**, not defaulted: measured, `"this machine does not run
'vllm'"`.

**The probe writes down what it measured.** Providers are built on the path to every turn, so a
registry that reached across the network to find out which programs a machine runs would put
somebody's Wi-Fi in front of every message. The `Have` a probe already fetches is noted onto the
roster (`Paired.runners`) and the registry reads a file. Only programs that were **serving** are
remembered — a Provider offered for a switched-off server would be a Service that looks
configured and refuses every turn, and unlike a backend somebody typed in by hand, nobody chose
it, so nobody would know why.

**Two dialects, and the smaller one is honest rather than lazy.** Ollama keeps `/api/chat`;
llama.cpp and LM Studio take the OpenAI-compatible `/v1/chat/completions` Epoch has spoken since
Phase 8. Three things deliberately do not cross to the second: `num_ctx` (those servers take their
window at *startup*, so a request-time value would look applied and not be), `keep_alive` (neither
unloads on a timer) and `think` (llama.cpp's reasoning fields differ by build, and a field guessed
from memory is exactly what the last four defects were). What does cross is everything a character
*is*: the composed prompt, temperature, top_p and the tools.

`POST /release` stays Ollama-only for the same reason, and says so.

**Measured end to end on the MacBook**: a turn with `runner: "lm_studio"` reached LM Studio and
came back `"Blue"` in 32.6 s; `runner: "vllm"` was refused before any socket was opened; `runner:
None` still went to Ollama, which is what every Host said before this field existed.

### Named `Runner` in the Kernel, `Brain` where a person reads it

`Brain` is already the Kernel's word for what a character's mind *is* (`mind::Brain`), and
`Runtime` is already its word for what progresses a Quest (ADR-0011, ADR-0025). Presentation
vocabulary does not become domain vocabulary — the same rule `Era`, `Party` and `History` follow.


## Amendment (2026-09-07) — §16: the wire is encrypted, and each end knows which machine the other is

**Everything on this Bridge crossed the network in clear text.** The composed turn is the user's
Chronicle, their system prompt and their project's context; the pairing exchange is the long
secret itself. Anything on that network could read a conversation, and — once — take the
credential that lets it be the Host forever. An audit found it before a user did.

### There is nobody to ask, so the fingerprint replaces the authority

A machine on somebody's home network has no name a certificate authority will vouch for and no
address that survives the night, so *trust this because somebody you already trust says so* has
no one to be. What replaces it is exchanged under the short code that already exists:

- the machine that shows the code puts its **fingerprint** in what it sends (`Hello.fingerprint`);
- the side that dials first **records** the certificate it was shown;
- every connection afterwards accepts **that certificate and no other**.

Trust on first use. Its one weakness is stated rather than papered over: an attacker already
sitting between the two machines *during the five minutes of pairing* can be the machine you
meant to pair with. Afterwards it cannot, and that is the property that did not exist before.

Pinning by fingerprint is also why the certificate's *name* does not matter: an address on a home
network is whatever DHCP said this morning, and what is being verified is the machine, not the
name it answers to. That makes the check stricter than the usual one, not looser — a real
authority would happily vouch for a name an attacker also owns.

### A bond with no fingerprint cannot connect at all

The tempting alternative is to accept any certificate when none was recorded. That would mean the
encryption bought nothing on exactly the bonds nobody has looked at since. A `Paired` with
`fingerprint: None` is refused with a sentence naming the machine and saying it takes one code to
fix — the cold-instrument rule, applied to a connection.

### `epoch-wire`, because two programs are the two ends of one conversation

A handshake written twice is two handshakes, and that is the one place a difference of opinion is
a vulnerability rather than a bug. Not `epoch-secrets` (that is the operating system's credential
store; this is a file beside the vault), not the Kernel (zero I/O), not the Engine — §9 stands.

It does not use `tiny_http`'s TLS: that feature is built on `rustls` 0.20, from 2021, and
answering *"this traffic is not encrypted"* with a four-year-old TLS stack is answering an audit
with a line for the next one. The door speaks `rustls` 0.23 — already in this tree, by way of
`ureq` — and the HTTP the two programs actually exchange: a request line, a few headers, a body
of a declared length. No keep-alive, no chunked bodies, no pipelining, and **every one of those is
a refusal rather than a silence**. It answers Epoch; a request it does not recognise did not come
from Epoch.

### Two doors, because there were always two audiences

EpochServices served the person's pages and the Host's routes on one listener, told apart by
`if mine` on forty match arms. *The pages are not reachable from the network* was therefore true
by repetition, and stayed true only while every arm remembered. The page is now bound to
`127.0.0.1:11499` and the Host's routes live on `11500` under TLS, in a different file. **The
network port did not move** — 11500 is what the other machine knows — so the internal one did.

Ports, restated: EpochServices' Host door `11500` (TLS, every interface) · its window `11499`
(plain, loopback only) · the Host's pairing door `11501` (TLS, and only while a code stands).

### Measured against the running program

Not against a harness that declares its own conditions. With `curl`, on one machine: the door
answers `403` unpaired and `200` paired; the code shown on the page enrols once and is `403`
the second time; a page route on the Host's door is `403` because it is *not there*; an unpair
posted with a foreign `Origin` or a rebound `Host` is refused and the bond survives; the person's
own unpair kills the bearer. The pairing door's own tests now run against a real TLS socket.
