# ADR-0024: Imported artwork, and the door it comes through

- Status: Accepted
- Date: 2026-07-29 (amended 2026-08-27 — §2b, the outbound transport)
- Depends on: [[0016-asset-resolution]], [[0020-asset-pipeline]], [[0023-characters-belong-to-the-user]]
- Related architecture: [[../../CONTENT_PHILOSOPHY]], [[../../PRODUCT_ARCHITECTURE]]
- Horizon: image import, World key art, orchestrator profile **IMPLEMENT NOW**; character sprite import, World export **DESIGN NOW**

## Context — the evidence

Two gaps surfaced from using the Launcher, and they turned out to be the same gap.

**1. The derived chart is honest and cannot be aspirational.** Every World preview is drawn from the World's own terrain, roads and Place positions. That was the right default and it is still the floor: it cannot promise something the World does not contain, and it costs an author nothing. But it can only ever show *what is charted* — never what the author imagines the place to be. Two Worlds of five buildings look alike no matter how differently they are meant to feel.

**2. The bridge has no orchestrator.** The Launcher shows whose Worlds and whose crew, and had no way to show whose *bridge*.

Both wanted the same missing thing: a way for the user to hand Epoch a picture.

## Decision

### 1. Authored artwork overrides derived rendering; derived rendering never goes away

A pack may declare `[pack] preview`. When present the viewport shows it; when absent — or unreadable — the derived chart appears. The chart stays reachable behind a `CHART` toggle whenever both exist.

This is the ADR-0016 fallback chain, applied one level up: **artwork adds to something that already works, rather than filling a hole.** A World with no artwork is complete, not unfinished.

The toggle is momentary state, not a setting. The author's framing is the default for every World, so peeking at one World's chart does not follow you to the next.

**Why the user decides:** how much immersion a World earns, and how detailed it is allowed to look, is a question about *their* World. Epoch supplying the answer — by generating art, or by refusing art — would be Epoch deciding how their work should feel.

### 2. Imports arrive as bytes; the frontend never touches the filesystem

`<input type="file">` and `FileReader` in the webview, base64 across IPC, and the Engine writes the file. No `tauri-plugin-dialog`, no `tauri-plugin-fs`, and — the load-bearing part — **no capability that grants the presentation layer filesystem access.** The surface can offer a file; only the Engine can write one.

The cost is that an image crosses IPC base64-encoded. That is bounded by an 8 MB cap, checked in both places: in the UI so a huge file is refused before being read into memory, and in the Engine because a surface's check is a courtesy and never a control.

### 2b. Amended 2026-08-27 — the rule is *the Engine serves it, by name*, not *base64*

The transport above was read as the decision, and it was only the implementation of it. **The
load-bearing sentence is the one in bold: the presentation layer has no filesystem access, and the
Engine names the file.** How the bytes travel is free to change while that holds.

They change now, in the *outbound* direction only. A picture drawn in the Chronicle or listed in
FILES is fetched by the browser over Epoch's own URI scheme, `epoch://picture/<name>`, served by a
handler in the Engine that resolves the **name** — its file-name component and nothing else — inside
the vault's images folder. The page hands over a name; the Engine decides what it means; no path
and no `fs` capability reach the webview. Every guarantee §2 and §3 make is untouched.

**Measured, on one 182 MB render, opening FILES:**

| | |
| --- | --- |
| Engine reads it, base64s it, IPC carries 243 MB | 2135 ms |
| `atob` in the page | 348 ms |
| the browser decodes 132 megapixels | 918 ms |

3.4 s for one thumbnail, on a page that draws several. The first two rows are what the scheme
removes outright; the third moves off the main thread, where it was the tearing. Against the same
picture at 1 MB the whole thing was 30 ms — **the transport was never the cost until the pictures
got big**, which is why this was not worth doing before ADR-0033 could ask for 4K.

**Inbound is unchanged.** What a person *hands over* still crosses as base64 through
`<input type="file">`, still capped at 8 MB, for the reason §2 gives: those bytes come from the
webview and there is no way for them not to. A scheme serves; it does not receive.

**One resolution, not two.** The handler and `open_picture` call the same function, because two
places that each decide what a name may reach are two places to tighten, and the loose one is the
one that ends up mattering.

### 3. The Engine names the file, from the bytes

The destination name is chosen by the Engine from the *sniffed* format — never from what the user's file was called, and never from its claimed extension. A `.png` full of `MZ` is a thing that exists; magic bytes are not.

This removes a class of problem instead of defending against one: there is no path to traverse, no extension to spoof, no name to collide. `preview` and `portrait` are the only stems, and every format under a stem is cleared before writing, so replacing a PNG with an SVG cannot leave the old file behind for the fallback chain to find.

The accepted formats are exactly the ones [`asset.rs`](../../BUILD/crates/epoch-engine/src/asset.rs) can already deliver — PNG, WebP, SVG — and a test asserts the two lists have not drifted. Storing artwork Epoch cannot draw would be a silent trap.

### 4. The orchestrator is not a character

The user's name and portrait live in `vault/orchestrator.toml`, beside the crew but not among them.

A `CharacterDefinition` describes somebody the Engine *runs*: archetype, routine, prompt, home place. None of that is true of the user. Making them a character would mean either giving them a routine they do not have, or making those fields optional for everyone else. It is a separate small file because it is a separate small thing.

It is also deliberately **outside** `vault/definitions/characters`, where every `.toml` is read as a character.

Absent is the ordinary first-run state, and not a fault: an unnamed orchestrator shows the title `ORCHESTRATOR`, and no portrait shows a silhouette. **Never a generated avatar** — inventing a likeness for somebody who has not chosen one is the same lie as a Launcher inventing a crew.

## Consequences

**Good**

- A World can look like what it is meant to be, without Epoch ever generating art (`CLAUDE.md`: "Assets are authored, never generated").
- The chart's honesty is preserved rather than traded away — it is one click from any World that has both.
- The import door is generic. Character sprites and World export use `import::accept_image` unchanged; the only new thing either needs is a stem.
- No new dependency, and no new frontend permission.

**Costs**

- Key art *can* promise what a World does not contain. That is the author's prerogative and the reason the chart stays reachable, but it is a real loss of the guarantee the derived preview alone gave.
- Imported images live inside the pack folder, so a World is no longer purely text-authorable if it wants artwork. It remains fully functional without any.
- Base64 across IPC is wasteful for large images. Bounded, one-time, and not worth a streaming channel yet (Earn Complexity).

**Clarification this forces on CONTENT_PHILOSOPHY**

The hard rule — only original / commissioned / open-source / CC0 / redistributable content — governs **what Epoch distributes**. It has never governed what a user puts in their own vault, and cannot: their machine, their artwork, their World. The rule applies again the moment a World is exported or shared, which is a decision the export milestone has to make explicitly.

## Alternatives considered

**Generate stylised key art from the geography.** Rejected twice over: Epoch does not generate assets, and a prettier chart is still a chart — it cannot express what the author means the place to feel like.

**`tauri-plugin-dialog` for a native file picker.** Rejected. It is a nicer picker and it costs a dependency plus a capability; the webview's own input needs neither. Worth revisiting only if a flow needs a *save* dialog, which the webview cannot provide.

**Trust the uploaded filename.** Rejected — see §3.

**Give the frontend filesystem access** (considered 2026-08-27, when the transport was measured).
Rejected, and the interesting part is that it would have been *worse* than the scheme for the
problem that prompted it: `readFile` hands the bytes to JavaScript, so the page still holds a
copy and still decodes on its own thread — it saves the 33% and the `atob`, and nothing else. The
scheme takes the bytes out of the page entirely. What it would have cost is the guarantee: World
Packs are third-party content rendered in that same webview, and the plan is a platform of
community Worlds.

**Tauri's built-in `asset:` protocol.** Rejected. It is cheaper to switch on and grants the
webview a *directory*, by configuration rather than in code. The custom scheme grants one folder
through one function that can be read.

**A generated thumbnail beside each picture.** Not rejected — deferred, and it is the thing to
build if the remaining 918 ms ever matters. It costs an image-decoding dependency (there is none
in the workspace), a second copy to invalidate, and a downscale that looks worse than the
browser's. None of that is worth 918 ms off the main thread today.

**Put the orchestrator in the Definition registry.** Rejected — see §4.
