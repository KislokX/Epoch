# ADR-0025: Quests own the work

- Status: Accepted
- Date: 2026-07-30
- Depends on: [[0006-conversation-domain-model]], [[0008-execution-engine]], [[0009-trust-engine]], [[0011-definition-runtime]], [[0012-context-composer]], [[0023-characters-belong-to-the-user]]
- Amends: [[0010-knowledge-engine]] (knowledge is the Project, not an abstract store), [[0015-activity-stream]] (evidence, not narration, is what History is made of)
- Related architecture: [[../../PRODUCT_ARCHITECTURE]], [[../../EXPERIENCE_CONSTITUTION]], [[../../CHARACTER_BIBLE]]
- Horizon: the Quest aggregate, the Lifecycle as an ordered Definition, Quest Context, and Evidence **IMPLEMENT NOW**; Party Chat as a surface, the Project root, failure states **DESIGN NOW**; a branching Lifecycle editor **VISION**

## Context — the evidence

Epoch could, as of this week, do something real: a Character with an assigned model takes a turn and the World shows her working. The moment that existed, the shape of everything after it became a question with a wrong default answer.

The wrong answer was already in the code. The shell held `chronicles: BTreeMap<CharacterId, Conversation>` — **one conversation per character**. That is the assistant model: you talk to Mage, and separately you talk to Frog, and the product's job is to route you between them. It works, it is what everyone builds, and it makes four models feel like four models.

It also makes several things impossible that Epoch needs:

- **Two surfaces cannot show one thing.** If the conversation belongs to a character, the World and a chat are two views of a routing table, and keeping them consistent is a synchronisation problem forever.
- **Work cannot survive its worker.** Change Mage's model, swap her for somebody else, add a fifth character — under a per-character model each of those is a migration.
- **History has nothing to be made of.** A pile of per-character transcripts is a log, not a memory.

## Decision

### 1. The Quest is the unit of work

> **NPCs do not own work. Quests own work. NPCs contribute to Quests.**

The Quest is the aggregate root: the unit of persistence, the unit of projection, and the thing every Experience Surface renders. A character is a *contributor* to a Quest, never its owner.

The consequences are immediate and they are why this is worth an ADR:

- Change a character's model and the Quest continues.
- Replace a character entirely and the Quest continues.
- Add a new specialist and the Quest continues.
- Two surfaces do not synchronise — they project the same Quest, exactly as the Launcher and the World project the same Engine today.

`chronicles` keyed by character is therefore **wrong and must move onto the Quest** before anything is built on top of it.

### 2. The domain is Intent → Quest → Execution → Evidence → History

Everything else — characters, models, Worlds, chats, MCP, providers — exists to support that cycle.

> **The user never delegates work to an AI. The user starts a Quest. Everything else is how the World collaborates to complete it.**

A character *inaugurates* a Quest rather than creating it. The work exists from the moment the user says what they want; the character's job is turning an intention into something the crew can act on. That distinction is what makes the first character a **designer of work** rather than a router.

### 2b. The Chronicle is the Quest's record; a Conversation is one projection of it

What a Quest accumulates is not a conversation. It is a **Chronicle**: approvals, state
transitions, artifacts, implementation summaries, documentation, tool executions, test runs,
decisions — and the things people said, alongside all of it.

A chat renders the conversational entries. That is a *projection*, in exactly the sense
Internal First means (ADR-0003): the Chronicle is canonical, and no surface sees all of it.

This gives the kernel's existing `Conversation` type a clearer job rather than replacing it. A
`Conversation` is what gets **composed for one turn** — the slice of the Chronicle a provider
needs, built by the Composer (ADR-0012). It was never meant to be the record; it is the request.

#### The vocabulary this corrects

`PRODUCT_ARCHITECTURE.md` currently lists `Automation → Quest` and `Conversation → Chronicle`
as **presentation** mappings, under the rule that world vocabulary never leaks inward.

That mapping is now wrong, and the fix is to the mapping rather than to the words. A Quest is
not an Automation: it has intent, participants, approvals and a history, none of which an
automation has. A Chronicle is not a Conversation: it holds approvals and artifacts and tool
runs. The domain outgrew the older names.

**Quest and Chronicle are canonical domain terms.** Internal First is unharmed — the rule was
that *presentation* words must not become domain words, and these stopped being presentation
words the moment they carried structure a projection could not.

### 3. The Lifecycle is authored data, and it is a sequence

```
Created → Planning → Approval → Execution → Documentation → Review → Completed → History
```

Each stage belongs to a character. The Lifecycle is a **Definition interpreted by the Runtime** (ADR-0011), never hardcoded — the user configures how their crew works, not which model runs.

**It is an ordered list, not a graph.** Two reasons, and the first is the user's own:

- `CLAUDE.md` says outright: *"Users should create powerful workflows using intuitive steps instead of complicated node graphs."* A node editor is the thing that constitution exists to prevent.
- Every motivating example so far is a **sequence**, including the ones with a character appearing twice. No branch, no parallelism, no condition. A list expresses all of them.

A branching editor is what gets built the day a real branch is needed — and that day the branch will be known, rather than guessed at.

### 4. A Quest carries Context, and characters read and write it

What travels between stages is not a prompt. It is the Quest itself:

Goal · Current State · Project Context · Relevant Files · Previous Decisions · Constraints · Accepted Plan · Artifacts · Conversation · Results.

This is ADR-0012 applied rather than contradicted: context is **composed** from the Quest by the Composer, never concatenated by whoever is speaking. A character reads the Quest and writes back to it; the next character reads what is there.

### 5. Knowledge is the Project, and the Project is not embedded

Each World has a **project root** — a real directory on the user's machine. That is the World's workspace: the crew reads from it, writes to it, documents into it.

Three sources, in strict priority:

1. **The Project** — the source of truth. Codebase, docs, ADRs, assets.
2. **The Model** — general reasoning and knowledge.
3. **The Internet** — only when it adds something the Project cannot.

The Project **never** gets embedded into a model. It is accessed as external context, the way a coding agent does it. That is what keeps local models viable against a large codebase, and it is what stops "knowledge" becoming a second, stale copy of something that already exists on disk.

This amends ADR-0010: there is no abstract knowledge store to fill. There is a directory, and it is already full.

### 6. History emerges from evidence

> The history of a World **emerges from** completed Quests.

Not "is a collection of" — a collection is a folder, and this is a consequence. A Quest leaves evidence behind: decisions, conversations, code, documentation, ADRs, commits, files. The World develops memory because things actually happened in it, not because anyone authored lore.

The rule that makes this non-fictional: **a Quest that produced no evidence produced nothing, and the History must say so.** Narration is not evidence. This is the causality rule (ADR-0018) applied to time instead of space.

### 7. The Lifecycle is declarative; the Runtime owns progression

Iteration does **not** live in the Lifecycle. `Planning → Approval → Execution → Documentation`
never changes shape, and never contains a loop.

What changes is the **Quest's state**. Validation fails, and the Quest enters *Needs Revision*.
The Runtime then routes it back to the appropriate stage carrying everything accumulated since.
Several planning and implementation cycles happen without the Lifecycle knowing anything about
cycles.

This yields a principle that is the logical consequence of §1, and worth stating on its own:

> **NPCs do not decide where a Quest goes. The Runtime does, based on the Quest's state.**

If a character chose the next step, that character would still own the flow — and the whole
point of §1 is that they do not. It also keeps the declarative Lifecycle honest: a sequence that
secretly encoded retries would be an execution language wearing a list's clothes.

Failure is therefore not an exception path. It is another truthful outcome: **Blocked** (the plan
is impossible), **Rejected** (the user declined it), **Abandoned** (the user stopped it),
**Failed** (execution did not work), **Needs Revision** (round again).

All of them enter History as what they were, and History records the *whole* story — how many
iterations, which decisions changed, where it stopped. A History that keeps only successes is
propaganda, and a World whose memory is propaganda is a World that lies, which is the one thing
the product forbids everywhere else.

### 7b. Handoffs have visible causes

Every transition traces to one of exactly three things:

1. the previous specialist **finished**,
2. the **Runtime** advanced the Quest on its state,
3. the **user** explicitly approved the next stage.

Nothing else may move a Quest. In practice the third is the common one and it is the strongest:
the user says "hand it to Robo", and only then does Robo become responsible. The cause is not
merely real — it is *visible*, which is what makes a surface showing collaboration different
from a surface simulating it.

While a specialist works, the surface shows that, for as long as it actually takes.

### 8. A surface may never fabricate timing

Party Chat is an Experience Surface, architecturally equal to the World (`PRODUCT_ARCHITECTURE.md`). The same Quest, the same crew, the same state; only the point of view differs.

One hazard is specific to it and worth naming here because it is easy to get wrong and impossible to spot afterwards: a character appearing in the chat "a few seconds later" must appear because **that character finished**, never because a timer fired. If a stage takes four minutes, it takes four minutes, and the surface shows it being worked on — the same honesty `ActivityClass::Work` now carries in the World.

## Consequences

**Good**

- One state, many surfaces, with no synchronisation anywhere.
- Work outlives its workers: models, characters and crews can all change mid-Quest.
- History becomes real without anyone writing lore.
- The Knowledge Engine gets *smaller*: a directory instead of a store to populate.

**Costs**

- `chronicles` per character is thrown away before it grew roots. Cheap now, expensive in a month.
- The Quest aggregate is the largest domain type Epoch will have, and everything touches it.
- A Lifecycle as data means the Runtime must interpret stages it did not compile — the point of ADR-0011, and still real work.

**Two concerns that were one, now separated**

An earlier draft of this ADR treated "the World has a project root" and "the crew can change
your files" as a single hazard. They are not, and conflating them would have delayed something
safe in order to gate something dangerous.

| | Answers | Gate |
|---|---|---|
| **Project Root** | *Where does this World work?* | none — it is a workspace and a source of context |
| **Trust Engine** (ADR-0009) | *What is this Quest allowed to do in it?* | write, delete, execute, install |

```
World → Project Root (workspace) → Quest (work) → Trust Engine (permissions) → Runtime (execution)
```

Epoch does not introduce a different workspace. It introduces a different way of collaborating
inside the same one — the same repository Claude Code, Codex or Cursor would open. Robo writing
in the user's real project is the *goal*, not a risk to be engineered away.

So:

- **Reading a Project Root is not gated by Trust**, and should arrive early. Planning done against
  the user's actual codebase is worth immeasurably more than planning against a hypothesis.
- **Writing, deleting and executing are gated by Trust**, and none of them happen until ADR-0009
  answers: a Quest's blast radius, what each character may touch, whether each action needs
  confirmation, what happens when the approved plan and the executed work diverge, and how any
  of it is undone.

**One hazard that belongs to reading, and is easy to miss:** with a local provider nothing leaves
the machine. With a hosted one, reading the Project Root means the user's source code is sent to
a third party. That is a disclosure decision, not a mutation one — it is still the user's to make,
explicitly, and it must not be buried in "the World has a project root now".

## Alternatives considered

**Keep conversations per character.** The default, and what the code already did. Rejected: it makes two surfaces a synchronisation problem, work fragile to changing its worker, and History a pile of transcripts.

**Model the Lifecycle as a graph now.** Rejected — see §3. It contradicts the project's own constitution and no example needs it.

**A knowledge store the crew populates.** Rejected — see §5. It would be a second, staler copy of a directory that already exists, and it would put a large codebase inside a model that cannot hold one.

**Let the user assign work directly to a specialist.** Tempting, and it is what the dialogue box built this week looks like at a glance. Rejected as the *model*, kept as the *intake*: saying what you want to the character who designs work is how intent enters. Telling the implementer what to implement is skipping the planning and the approval that make the rest trustworthy.

---

## Amendment - 2026-08-09, from implementation

The crew worked together for the first time, across a local model and two hosted agents. Four
things the build settled, and one lie it caught.

### The lie: a character reported a colleague's work as done

Asked to involve Paladin, Mage did the work herself and answered *"Paladin lo actualizó"*.
Paladin had never run. The Chronicle recorded a contribution that did not happen, which is the
exact failure §7 exists to prevent.

The cause was not the model. The Composer's crew block - *"they speak for themselves, never
answer on their behalf"* - was written **inside the Composer**, where only a model can reach it.
An agent was told it was "one of a crew" and never told who the crew were, so the only way to
satisfy the request was to do it and describe it as somebody else's. The sentence is now
`context::crew_note`, composed once and given to **both** brains, and it says the part today
made necessary: *never say that one of them did something.*

### `epoch__hand_over`: invited to the decision, not in charge of it

A character may **ask** to pass work on. It may not pass it on. The tool takes who and **what** -
the complete instruction that would travel - and its only effect is a question put to the user,
through the same bridge and the same window a tool approval uses.

This keeps §7b intact rather than bending it. The three things that may move a Quest are
unchanged; this is the third one, made reachable to a character that has something to say about
it. The parallel is exact with ADR-0027's `--permission-prompt-tool`: Epoch is invited to a
decision it does not own, and shows the user the requester's own words.

Two deliberate differences from a tool approval:

- **`Auto` does not skip it.** Autonomy is about what a character may *do*. A handover brings a
  second character - a second model, a second bill, a second set of hands on the user's files -
  into the work, and saying yes to this World once did not say yes to that.
- **Never standing.** Every handover is its own decision. *"Always let Mage hand work to Robo"*
  is the quiet escalation the per-turn permission exists to prevent.

On approval the proposal is recorded as the **proposing character's own words** - not attributed
to the user, and not invented by Epoch - and everyone downstream reads it through the ordinary
path.

### What travels is the Quest, and only the part they have not seen

A model is composed a fresh conversation every turn, so handing it a Quest was already nothing.
An agent keeps its own session containing only what it has been told, so a handover to one was
refused outright: *"say what you want done"*. True, and it left a crew unable to work together.

`handover::briefing` composes what travels: the goal, who said what, what already exists. It
starts after that character's last contribution - derived from the Chronicle like `participants`,
never stored - so somebody who has worked on this Quest is not told what they said themselves in
the third person. Epoch's own bookkeeping (approvals, state changes) is left out: a colleague
briefed on those learns about the machinery instead of the work.

**The user reads it before it travels**, for every brain. A local model cannot call a tool at
all, and the user should not get a weaker guarantee for using one - so the offer shows the exact
text the recipient will receive, unedited. Epoch does not summarise a Quest before showing it,
because a summary of the record is narration and the user would be approving that instead.

### Placement is not wording (measured, again)

The rule against doing a colleague's work was in the crew block the whole time and did not hold:
`gpt-5.4` deferred, `qwen3:14b` edited the file and reported success.

Same failure `context.rs` documents twice, against this same model: a system message is weighed
against thirty turns of conversation and loses. The situational half - *"this message is asking
for Paladin, not for you"* - therefore lives **last, on the user channel**, and is written only on
the turn it is true. With that, `qwen3:14b` answered *"This request is for Paladin"* and stopped.

It remains an instruction, not a gate. The stronger option - withholding write tools from anybody
whose user named a colleague - would refuse *"ask Paladin to review this, and fix the typo while
you're here"*, so it is left as the user's decision rather than imposed. When a model ignores the
instruction, the Chronicle still attributes the work to whoever actually did it.

### The road carries work, visibly

`places.rs` has said since ADR-0028 that a road carries work. It does now: whoever picks a Quest
up walks to where its last contributor is standing, and walks home afterwards on their routine
(ADR-0018). It was missing from the agent path at first, so a handover to an agent moved the work
and nothing moved in the World - the crew collaborating invisibly, which is the one thing a
Living World exists to prevent.

---

## Amendment - 2026-08-09, QuestStore evidence

The first persistence implementation wrote every Quest for a World into one `quests.json` file.
That contradicted the aggregate boundary in section 1 in a practical way: accepting one turn,
compacting one Chronicle or handing work to another character rewrote and reloaded unrelated work.

`QuestStore` now persists one Quest document per Quest. A turn retains the exact Quest identity
it started with, so a late provider, agent, approval or capability result is written back to that
same document even if the user opened another Quest while it was running. A handoff therefore
continues to be a contribution to the existing Quest rather than a transfer of a conversation or
an implicit merge with another Chronicle.

The storage shape is an adapter concern, but this result is architectural evidence: the Quest is
not only the unit rendered by surfaces; it is independently recoverable, migratable and durable.

## Amendment - 2026-08-10, compaction source integrity

Every `/compact`, regardless of model, NPC, handoff or World, begins with the one persisted Quest
document it is reducing. An external agent's session handle is not Quest memory. It is opaque
state owned by another program and can be stale or carry a different conversation; using it as a
source would let one Quest replace its Chronicle with a brief about unrelated work.

Provider compaction composes that Quest directly and refuses if the provider cannot fit all of
the source records. Agent compaction starts a fresh, throwaway session and supplies a serialized,
Epoch-owned source document: Quest identity and intent, any existing durable memory, and the
exact literal Chronicle prefix being reduced. The two recent records remain literal. The source
is framed as untrusted data, not instructions. A failed maintenance turn writes nothing, and a
successful one removes the opaque session handle only after the new durable memory is persisted.

The same boundary applies when an NPC begins or resumes a fresh session: Epoch serializes that
Quest's canonical memory and Chronicle into the new session. A handoff changes contributor, not
source document. No model, NPC, Quest, Chronicle or World may contribute an implicit second
context.

## Amendment (2026-08-19): a stage is a session with one NPC

**Superseding an amendment written earlier the same day.** That one measured three absences —
no Quest has a Lifecycle, nothing advances `stage`, nothing emits `StageStarted` — and concluded
that stage-driven movement was blocked on a Runtime nobody had built. The measurements were
right. The premise underneath them was not: it assumed the Lifecycle described below, **authored
in advance**. The owner's answer:

> A stage is a session with an NPC. Stage 1: the user asks Mage for a prompt and then says
> *"pass it to Paladin"*. Stage 2: Paladin implements it, and the user says *"have npc3 document
> it"*. Stage 3: npc3 documents. Click X in Workflows → the Quest is completed.

So the three absences were not a gap. They were this ADR describing a different mechanism than
the one the product wants.

### The Lifecycle is not authored. It is recorded.

A stage begins when somebody takes the work and ends when they hand it on, which makes the
**handover the boundary between two stages** rather than something that happens inside one. The
shape of a Quest is therefore the record of who worked on it, in order, and the user steers it a
turn at a time instead of arranging it up front.

`Lifecycle` and `Stage` survive as the **plan** — the Quest *templates* the World Editor already
reserves a door for. `current_stage()` reads that plan and is honestly `None` today. What
actually happened is read from the Chronicle, and the two never merge: a plan that rewrote the
record would be able to disagree with what happened.

**The cursor has exactly one writer.** `begin_stage` records the `StageStarted` entry *and* sets
`Quest::stage`, so the number is always the count of stages begun minus one. Two writers would
make the cursor the one that can be wrong without anybody noticing, and it is the one History
would be read through.

**Nothing names a stage.** It carries the number a person counts — `"1"`, `"2"`, `"3"` — and the
character beside it. Inventing "Planning" or "Implementation" would be Epoch deciding what the
user's work was about, which is the same rule that keeps `Stage.name` uninterpreted.

### `Completed` means it ended. It never meant it went well.

> Completed no es igual a éxito. Completed es que se terminó la quest, no toda quest puede ser
> un éxito por obvias razones.

The Kernel had encoded the opposite — `/// Finished, with evidence` and a `succeeded()` that
returned true for exactly this state. That is gone. Success is not a state a Quest can be in; it
is read from **what the work left behind**, which this ADR already said in §7 and which the
Knowledge Engine already measures.

The X in Workflows is how a Quest ends, and it writes `Completed`. It used to write `Abandoned`,
so every note in the vault said *"you ended the conversation"* about work that had finished
exactly as asked. A Quest that already stopped for a **stated** reason keeps it: `Blocked` and
`Failed` are things that happened, and flattening them into the plain ending is how a History
starts lying by omission.

### A stage is recorded, and deliberately not shown

Asked whether the chat should mark where one session ended and the next began, and whether
Workflows should say *"stage 3 of 3"*, the owner said no to both — and that **a Quest reading as
one flat conversation is correct** (2026-08-19).

That is worth writing down, because the absence looks like an oversight: the Engine went to some
trouble to record `StageStarted` and `StageFinished`, and the projection to the chat drops them.

It is not an oversight. From the user's seat there is **one conversation**, and handing work to
somebody is a thing you do *inside* it rather than a boundary you cross. A divider announcing a
stage change would be the interface explaining its own bookkeeping — and the person who typed
*"pass it to Paladin"* watched it happen; they do not need Epoch to tell them it happened.

The record exists for what reads it: History, the Knowledge Engine, a briefing composed for
somebody joining work already under way, and anything later that asks *who had this, and when*.
Those are questions about the record. The chat is not one of them.

### And the World already moves on it

Under this definition, *"the Quest changed stage"* **is** *"the work passed to somebody else"* —
which the World has walked since the arrival beats landed (ADR-0018 amendment). The receiver
crosses the map to the last contributor's Place and arriving beside them is a measured `Talk`.

So the rule this ADR is owed —

> They are not NPCs walking because they feel like it. They are moving because the Quest changed
> stage.

— is not waiting on anything. It was already true; the Engine simply had no word for it. It has
one now.

What is **not** built, and is not pretended: a Place authored per stage (Planning → the
Laboratory). Under this model a stage happens where its character is, which is a fact about the
person rather than about the stage. If a Quest ever wants to send somebody somewhere the work
did not come from, that Place is authored beside the stage and never inferred from its name.
