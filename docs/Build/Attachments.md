# Attachments — reference without filesystem authority

## The user-visible contract

`ATTACH` or dropping a supported text file anywhere on the open conversation puts a small
reference beside the next message. The Chronicle records its filename and size next to the user's
words; it does not
turn the document into a pasted transcript every time the conversation is opened.

On Windows, the Tauri window deliberately leaves native drag handling off so Explorer supplies
the same HTML5 file selection that `ATTACH` uses. This is one path with one validation policy:
dropping over the Chronicle and choosing `ATTACH` produce the same candidate reference.

The character receives the exact text on that turn and on later continuation or handover turns.
It is labelled **user-provided reference data**, not instructions. An attached document cannot
override Epoch's rules or the request typed beside it.

The desktop engine acknowledges the number of attachments it accepted before the composer clears
them. This matters during development: a hot-reloaded UI can be newer than the running Rust
process. In that case Epoch keeps the chip and draft and says that the text was sent without a
confirmed attachment, so the person can restart the desktop process and retry rather than lose
the reference silently.

## Deliberate limits

- Text formats only in this first cut: common source, configuration, Markdown, CSV and plain-text
  files.
- At most three files, 8 KB each and 16 KB total per message.
- A message may carry only a reference. Epoch records that no written request was supplied; the
  character reads the reference, acknowledges it, and asks what the person wants to do next.
- No path reaches the Chronicle. Epoch persists only the short filename and the content the user
  explicitly selected.

Those limits are enforced in the desktop shell as well as the UI. The point is not merely to
avoid an upload failure: attachments are context, so they must leave room for the request,
character instructions and recent work even on a small model window.

## What this does not grant

An attachment is not `read_file`, a project root, an artifact, or a new agent permission. The
model and agent receive only the chosen text; neither gets a handle, a path, nor authority to
inspect neighbouring files. Selecting it is the person's one explicit disclosure.

Images, PDFs and spreadsheets need their own provenance-preserving context path. They are not
quietly converted to garbage text in this feature; they remain a later rich-document capability.
Large files (including the requested 1-100 MB range) need that same asset path: reading them into
the WebView and a model context would be slow, fragile and would consume the entire turn budget.
This small-reference channel intentionally refuses them until Epoch can import them durably,
report progress, extract the relevant content by format, and disclose exactly what reaches a brain.

## Architecture

`Entry::Said` owns the reference because it belongs to the user message. The context composer,
agent continuation, fresh agent session and handover all project the same canonical record. The
shell's `SaidView` intentionally exposes only `{ name, bytes }` to the UI, while the full text
remains in the Quest for continuity.

That keeps the record truthful: the Chronicle says *what was shared*, the composer says *what a
brain was told*, and no UI-only attachment cache can drift from either.
