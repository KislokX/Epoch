# Chat and MCP regression boundary

This note records the Phase 4 correction after two independent regressions: a live turn could
briefly paint into another Quest, and the MCP door credential still used a pre-DPAPI plaintext
file.

## Conversation contract

A live turn belongs to the pair `(characterId, questId)`, never simply to the character currently
visible. `turn:started`, streamed tokens, tool steps, context measurements and `turn:ended` all
carry that identity. The Experience Surface keeps a short-lived lifecycle record for work that is
off-screen, but it renders it only after that exact Quest becomes visible again.

Consequences the user can rely on:

- Starting **NEW** while Quest A answers never puts A's answer or spinner in Quest B.
- Going back to A restores STOP while it is running, then reloads A's Chronicle after it ends.
- A set-aside Quest remains open and can be returned to; it is not abandoned.
- Auto uses Codex's workspace-write boundary. Manual starts within the same Project Root in
  read-only mode, then pauses on Codex's native app-server permission request before an ordinary
  file change runs. Epoch renders that request with its normal approval modal. No Epoch mode
  selects Codex's unrestricted sandbox.

The UI hook regression test exercises the first two cases. Engine tests cover multiple open
conversations, durable Chronicle reload, and the three Codex autonomy modes.

## Quest-scoped agent controls

`Quest.session` persists the active conversation's optional `autonomy` and `reasoning` overrides.
They are not character settings and they are not a World-wide switch: Quest A can remain Manual
with Low reasoning while Quest B uses Auto and Medium. Old Quest files omit the object and fall
back to the existing World autonomy and character reasoning defaults.

Codex's `app-server` has a bidirectional native approval callback. For Manual, Epoch keeps its
native sandbox read-only and answers its command/file/permission request only after the normal
Epoch approver has obtained a decision. The request opens the question, so the user sees the exact
path/content before the write runs. No Codex app-server turn receives an Epoch MCP server or
bearer: a generic MCP file-tool route cannot synchronously wait for the native approval answer and
was canceling before the prompt became visible. No mode uses `danger-full-access`.

On Windows, an exact `codex-windows-sandbox-setup.exe` bootstrap failure is retried once with the
same root and same sandbox. The retry is deliberately limited to that helper signature, before a
tool event exists; other failures are never retried automatically because they may have performed
real work.

### Manual visual confirmation — 2026-08-11

This path was verified in the running desktop World and is a **regression boundary, not a new
feature to redesign**:

- In Auto, the working character created the requested text file inside the selected Project Root
  and the Terminal/Chronicle recorded that concrete write as evidence.
- Mentioning a colleague produced the handover preview before work moved. Accepting it gave that
  colleague the Quest's scoped brief, including the earlier file evidence and the requested next
  action; it did not replace the Quest with another conversation.
- The receiving character read the handed-over file before proceeding. The Terminal retained the
  ordered `Write` then `Read` evidence, attributed to the character that actually performed each
  operation.

Do not change this Auto, handover, evidence, or per-Quest continuity behavior without repeating
this visual check for both agent-backed characters and local-model characters.

### Manual prompt delivery regression

The Manual permission surface has two sources of truth with deliberately different jobs:

- `agent_bridge` is a recovery snapshot for a World that mounts while a question is already held;
- `agent:asking` / `agent:settled` are the live facts emitted by the Door while Codex is waiting.

The initial implementation issued both reads concurrently. If the empty recovery snapshot returned
after the live `agent:asking` event, it overwrote the visible question. Codex then correctly timed
out with `NobodyAnswered`, but the user had never been shown **ALLOW ONCE**. This was a UI ordering
defect, not an authorization denial and not a Codex sandbox failure.

`useAgentDoor` now registers both event listeners before reading the recovery snapshot and tracks a
monotonic question revision. A snapshot may initialise a World, but it cannot overwrite an event
observed after its request began. The regression suite covers the exact empty-snapshot-after-question
ordering, recovery of an already-held question, and the `answer_agent` continuation command.

The owner later completed this visual proof. Preserve the same harmless-write
test, including **NO** followed by **ALLOW ONCE**, whenever this boundary changes.

### 2026-08-12 native transport correction and visual acceptance

The UI ordering repair above was necessary but not sufficient. Codex Manual was
also given an Epoch MCP configuration. The model could choose generic Epoch file
tools instead of its native app-server request; that non-interactive route
canceled internally before `agent:asking` reached the UI. This is why repeated
UI-only fixes could not make the modal appear.

`BUILD/crates/epoch-engine/src/agents/codex.rs` now gives app-server turns the
persona only: never an Epoch MCP server or bearer token. Manual maps to native
`untrusted` approval, Auto maps to `never`, and both retain the bounded Project
Root sandbox. Command/file/permission requests synchronously use the normal
Epoch approver, a Manual grant is scoped to its current turn, and it never
grants network access. The Manual persona tells Codex to use its native tools
for ordinary filesystem work; handover remains World-owned and separate.

The owner visually confirmed the fixed behavior: **MAGE NEEDS YOUR WORD**
appears before a Manual write; **ALLOW ONCE** produces the file and evidence;
Auto continues to work. This section supersedes the older MCP-description text
above. Preserve the NO/ALLOW ONCE sequence as a regression test whenever this
boundary changes.

### 2026-08-12 — what the app-server move left behind

The transport change was correct and is visually accepted. It also silently changed three things
that both handovers still describe the old way, and left a large body of code alive only for its
tests. None of this is a defect in the working path; all of it is auditability, and it should be
settled before Phase 5 rather than discovered during it.

**Manual is not a read-only sandbox.** `sandbox_policy()` returns `workspaceWrite` with the
Project Root as the only writable root, for **every** mode, and network access off. `sandbox_of()`
— which returns `read-only` for Manual — is `#[cfg(test)]` and belongs to the retired `exec`
transport.

What protects Manual is now one thing: `approvalPolicy: "untrusted"`, which pauses the ordinary
command or file change *before* it runs and routes it through Epoch's approver. That is a better
guarantee than the old one — the old sandbox refused the write and Epoch inferred the need
afterwards from stderr — but it is **one** layer where the documents claim two. Anyone reading
"Manual starts read-only" would not think to check what happens if that callback ever fails open.
Say what is true: *Manual may write inside the Project Root, and every write stops for the user's
word first.*

**The Windows bootstrap retry is no longer live.** `helper_bootstrap_failed` is `#[cfg(test)]` with
no production caller. The one-shot retry for `codex-windows-sandbox-setup.exe` exists only in the
legacy path's tests.

**`Autonomy::AcceptEdits` is unreachable.** `turn/agent.rs::run` retries a turn only when
`done.blocked` is `Some`, and the only writer of `blocked` was the legacy `work_once`; `claude.rs`
never sets it. The rung is now reachable from tests alone. Either the app-server's per-request
approval makes it genuinely redundant — in which case it should go, and the retry path with it —
or something is meant to still set `blocked`. It should not stay ambiguous.

### The retired transport, and what has to survive it

`codex.rs` keeps the whole `codex exec --json` path behind `#[cfg(test)]`, deliberately —
`work_once` carries `#[allow(dead_code)]` and the note *"preserves the captured legacy parser
fixtures"*. Test-only: `work_once`, `arguments`, `read_event`, `blocked_by_the_sandbox`,
`denies_access`, `as_windows_writes_it`, `flattened`, `sandbox_of`, `approves_workspace_writes`,
`helper_bootstrap_failed`.

The cost was concrete. **Every evidence test in the file was proving the owner's
"we have no way of knowing who did it" fix against a parser production no longer calls**, and
`read_app_item` — the one it does call — had none. That gap is closed: eight tests now drive the
live path, including the case nothing had ever asserted, that a `declined` status (what a Manual
**NO** produces) is not evidence. The shapes were read from the app-server's published schema,
not remembered.

**Retired 2026-08-12.** `codex.rs` went from 2,382 lines to 1,612. Both things its tests uniquely
covered were dealt with first:

1. Token usage. `read_app_notification` now holds the four notification arms that were inline in
   the receive loop, and seven tests cover them — including that a usage report carries what the
   turn **used** with its budget **unknown**, because Codex reports what was used and never what
   is allowed.
2. The `exec` argument order, which is archived here because the transport it described is gone:

   > Every option belonged to `exec`, **before** the `resume` subcommand. Built the other way
   > round it ran perfectly on the first turn and died on the second with
   > `error: unexpected argument '--sandbox' found`, which reached the user as *"Codex stopped:
   > it exited with exit code: 2 and said nothing"* — a first message that works and a second
   > that never does. Reproduced against the real program. The app-server carries the thread id
   > in its JSON-RPC parameters instead, so the ordering cannot recur.

Two guarantees were re-pointed at the live path rather than deleted with it: no Epoch mode turns
the sandbox off (now asserted across every mode against `sandbox_policy` and `approval_policy`,
because a single-case check would not notice a fourth rung arriving), and the session handle is
read from either shape the thread-start reply has used, with a missing handle staying missing
rather than becoming an empty string that looks resumable.

## Ollama visible-answer contract

Ollama can return private reasoning with no visible `message.content`, especially when its output
budget ends. Epoch never promotes that private reasoning into the Chronicle. A completed provider
stream with neither visible text nor a tool call becomes a named provider error instead of a blank
assistant reply. Normal non-thinking Ollama turns continue to stream visible text.

## MCP door credential migration

Older local installs wrote the MCP bearer at `vault/agent-token`. The current door uses the same
Windows-DPAPI store as provider secrets, under the separate secret name `mcp:door`.

On the next opening of the MCP door, Epoch:

1. Reads and validates the existing 64-character bearer, if present.
2. Writes that exact bearer into `vault/secrets.dat` encrypted for the logged-in Windows account.
3. Removes `vault/agent-token` only after the encrypted write succeeds.

If the encrypted store cannot be read or the legacy bearer is malformed, Epoch refuses to open the
door and leaves existing disk state unchanged. It never silently rotates a credential, prints it,
or overwrites an unreadable secret store.

This migration protects the vault. A previously adopted project can still contain an MCP bearer in
its own `.mcp.json`; treating that project configuration as a secret and preventing it from being
committed is a separate release-hardening task.
