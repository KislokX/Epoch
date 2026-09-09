# ADR-0034: Work that outlasts the turn

- Status: Accepted
- Date: 2026-08-27
- Depends on: [[0005-capability-first-architecture]], [[0009-trust-engine]], [[0015-activity-stream]], [[0018-world-simulation]], [[0025-quests-own-the-work]], [[0030-images-are-made-by-a-capability]]
- Related architecture: [[../../LIVING_WORLD_DESIGN_GUIDE]], [[../../EXPERIENCE_CONSTITUTION]], [[../../CLAUDE]]
- Horizon: the Job, its evidence and its presence **IMPLEMENT NOW**; `make_video` and audio **DESIGN NOW** (they consume this); a queue, priorities and cancellation-by-another-character **refused for now**

## Context — a capability runs inside the call, and the whole product waits

`turn::perform` calls `capability.run_watched(...)` and blocks until it returns. Everything above
it blocks with it: the round, the turn, the model's next word, and the window.

That has been survivable because almost nothing takes long. Measured on this machine, a Flux
render is the longest thing Epoch does at 34.7 s to 116 s, and even that is only tolerable
because the window streams nothing during it. A four-minute video is not tolerable, and neither
is a test suite, a large clone, or a `MEASURE` on a model somebody wants to keep talking through.

`run_watched` was the first answer to this and it is the right one for what it covers: a capability
that streams lines turns a spinner into an account of what is happening. But it still returns
before the turn ends. **It makes waiting legible; it does not make it optional.**

There is a second, larger reason. `LIVING_WORLD_DESIGN_GUIDE` asks for characters who are
genuinely **missable**, and forbids staging it. Until now nothing took long enough to mean
anything, so a character was never really elsewhere. A four-minute render is the first honest
cause — and ADR-0018's causality rule is satisfied by construction, because the character is in
the laboratory *because they are working there*.

## Decision

**A capability may answer `Started` instead of a result. The Engine carries the rest.**

### 1. `Started` is an answer, not a new method

`Outcome` grows a third shape beside `told` and evidence: a call that has begun something which
will finish later. Nothing is added to the `Capability` trait, and the twenty capabilities that
will never be long are untouched.

**Derived, never declared.** Whether a call is long is a fact about *this call's arguments* — a
render of one frame is not a render of two hundred — and the only place that can answer it is the
capability, at the moment it looks. A `fn is_long()` beside `describe()` would be a manifest
entry, and this codebase has already learned what those do (ADR-0030: *a capability typed into a
manifest is one that will eventually lie*).

### 2. A Job belongs to a Quest and a character, from the first instant

A `Job` carries `QuestId` and `CharacterId` taken **when it starts**, never read back from
whatever the surface has selected when it finishes.

This is not a precaution. It is the defect ADR-0025 already recorded: evidence was filed against
`active_id`, so a tool returning while the user had opened another conversation credited the wrong
Quest — *a `spotify_mcp_play` chip inside a Quest called "npm view react version"*. A job outlives
its turn by minutes, which makes that failure the normal case rather than a race.

### 3. What the model is told is bounded and negative

The tool result for a started job says what happened, says nothing exists yet, and does not
invite another attempt:

> `STARTED. Nothing has been made yet and no file exists. You will be told when it is done — do
> not describe the result, and do not call this again for the same thing.`

This is the rule ADR-0030's third amendment arrived at, applied before it can go wrong rather than
after: **what a tool says back is prompt, and it is read as such.** `see_image` invited a retry and
got seven; `draw_image` handed over a filename and got a forged Markdown image. A started job has
both hazards at once — a name that does not exist yet, and a gap a model will fill.

### 4. Evidence lands when the work lands, or History says what happened

When a job finishes it files `Entry::Produced` on **its** Quest and emits an Activity. When it
fails it files the failure. Neither is silent.

**A job that was running when Epoch closed is `Interrupted`, and says so.** It is not resumed and
it is not quietly forgotten: ADR-0025's failure states exist precisely so History can say *this
stopped* rather than keep only the successes. Nothing here is a queue that survives a restart —
that is a promise about durability this build has not earned, and pretending otherwise would be
worse than the gap.

### 5. The character is working because they are working

While a job of theirs runs, the character's `PresenceState` says so, in the place the work
belongs to. When it ends, it stops.

Nothing about this is animation, and nothing is scheduled to *look* busy: the presence traces to a
job that exists, which is ADR-0018's causality rule met by construction. A World with no running
job shows nobody working, and that is the honest frame.

### 6. One job per character, and it is said out loud

A character with a job running takes no new turn until it lands. This is the same single-slot
constraint the approval bridge and `running` already have, named rather than discovered — and the
surface says *Mage is rendering* rather than refusing a message with no reason.

Refused for now: a queue, priorities, and one character cancelling another's work. None has a
motivating case, and `CLAUDE.md`'s Earn Complexity applies — the first real one will say what
shape it needs.

## Why this is best

**It is the smallest thing that makes waiting optional.** The alternative shapes all move the
problem: a longer timeout still blocks, a background thread with no Quest attached files evidence
nowhere, and a durable queue is a database this build does not have a reason for yet.

**It reuses the two invariants that already exist for exactly this failure.** `running` keys work
by character; `Entry::Produced` is how evidence becomes History. A job is those two, held for
minutes instead of seconds.

**It makes an existing promise true instead of adding a feature.** Missable characters were
specified in `LIVING_WORLD_DESIGN_GUIDE` and could not be delivered honestly, because nothing took
long enough to be absent for.

## Alternatives considered

**Let the turn block and show a better spinner.** This is `run_watched`, which exists and is kept.
It makes four minutes legible and still spends four minutes of the user's window.

**A durable job queue on disk.** Refused for now. It buys resumption across a restart, and
resumption is the one thing a half-finished ComfyUI render cannot honestly offer — the server was
also killed. `Interrupted` is the true state, and writing a queue to claim otherwise would be a
gauge with nothing behind it.

**A `fn is_long()` on the trait.** Refused: a manifest entry about a call's cost, decided before
the arguments are seen. Same failure as a declared capability list.

**Let the model poll.** Refused outright. It spends a turn, a model call and a context window per
check, and the last time a tool result invited another call the model made seven.

## Consequences

- `Outcome` gains a shape, so every `match` on it is a place that must decide what a started job
  means. That is the point: silently treating `Started` as a result is the failure this prevents.
- The turn loop gains one branch and no new thread of its own.
- The World gains a true reason to show somebody working, which Phase 12's remaining items
  (`make_video`, audio) all depend on.
- `asset.rs` still delivers images only. Long work is the gate; delivery is the next one.

## Risks

**A job that never ends.** A ComfyUI that hangs leaves a character working forever. Mitigation: a
job carries what it is waiting on and how long it has been waiting, and the surface shows both —
an instrument, not a timeout guessed at.

**Two surfaces disagreeing about what is running.** The Bridge already relays turns; a job started
on a lent machine is a case this ADR does not cover and must not be assumed to work.

## Open questions

- Where a job's presence *is*: a render belongs in the laboratory, but a test run has no obvious
  place. Answered when the second kind of long work exists, not invented now.
- Whether a job may outlive the World being closed and re-entered, or only the turn.

---

## Amendment — the first job is a video, and it is the panel's (2026-08-28)

> **`make_video` is not a capability. It is the Studio Panel's VIDEO tab**, and pressing GENERATE
> there is what begins a Job.

### What this settles

This ADR was written with `make_video` named as its first consumer and left it as DESIGN NOW. In
the meantime `draw_image` was deleted (ADR-0030's fourth amendment) and **one thing draws, and it
is the panel** — so a capability that made a video would be the mistake that deletion corrected,
arriving in a second medium.

That leaves the Job machinery with exactly the producer it was built for, and none of it changed
to accept one. `Outcome::begun` is still how a *capability* would start work when there is one to
start; the panel calls `Jobs::begin` directly, because a person pressing a button is not a turn.

### What was built, and what was measured

| | |
|---|---|
| **The first frame is a form** | VIDEO sits beside IMAGE and a dark 3D, from ADR-0033's first version |
| **The graph** | the picture graph with three nodes swapped — a latent with a `length`, the family's conditioning, `CreateVideo` into `SaveVideo` |
| **One family** | `LTXV`. 49 frames at 512x320 in **10.1 s** on this card, drawn before a line of this was written |
| **In the window** | GENERATE, the crew card reads `WAITING` within the second, a video plays in the Chronicle **40.3 s** later |

**A picture is still drawn on the calling thread**, and that is a measurement rather than an
oversight: 12 to 35 s on this machine, with the person standing there. Making everything a job to
be consistent would have bought nothing and cost the immediacy of a button.

**Everything refusable is refused before the job starts.** The graph is composed on the calling
thread, so a missing node, an unknown family or an empty prompt is an answer to a button press.
A refusal arriving four minutes later as a message from a character is the defect the deleted
`draw_image` had, where a `refusal()` written to run asynchronously turned *you asked for a style
nobody has* into silence.

### Two sentences, because a job writes two entries

`file_against` records an `Answered` and a `Produced` and the Chronicle prints both. They were
the same string, and the window read:

```text
MAGE  Made a video in 55.2s.
      Made a video in 55.2s.
```

The character says *The video is ready.*; the evidence says what it was and how long it took.
Neither names the file — ADR-0030's third amendment, in a second medium.

### §5, corrected by the owner

This ADR said *the character is working because they are working*. They are not: **ComfyUI is
working and the character is waiting on it.** `Effort::Waiting` is its own state for that reason,
distinct from `Working` and from `Idle`, and the card reads `WAITING`. A character shown as
working while a machine works for them is the World claiming something it cannot see.

### Deferred, and then closed — 2026-08-29

**Audio — done.** Two families, and the second one refutes the first one's limit: Stable Audio
caps at 47 s, ACE-Step writes **90 s of music with lyrics in 8.0 s**. Sound reaches the Chronicle
through the same door pictures do, and `SaveAudio` reports under ComfyUI's `audio` key rather
than `images` — which is why `first_image` now tries `images`, `audio`, `video` and `3d` instead
of the one key the picture chain grew up on.

**3D — done, and the condition was met rather than waived.** The tab said it waited on *something
that can display a mesh*. `polyscope-rs` renders **headless** — no window, no event loop — so the
mesh is turned through 24 frames and kept as an animated GIF named after the mesh's own hash. The
picture path carries it unchanged, and the Chronicle finds a preview with no second field on the
artifact. Driving it in a window would have been a second place things live, which is the
immersion leak this project has a name for.

**Wan and Hunyuan Video** — still no, and still for the reason above: their nodes are on this
server and no model of either is. Writing their recipes would be writing them from memory, which
is the guess this codebase keeps deleting.

### What the media taught the job machinery

**A job must survive its own preview.** The 3D `fine` surface produced a 2 805 800-face mesh and
wgpu panicked over a bind-group limit *inside* the turntable. `preview` promised in its own
comment that every failure is `None` and the caller carries on — and it made that promise with
`?`, which covers returned errors and none of the raised ones. The model sat finished in the
vault while the crew card read `WAITING` for twenty minutes.

> **A job's optional last step must not be able to take the job down.** Not because previews are
> unimportant, but because the thing that was actually asked for was already finished when it
> failed.
