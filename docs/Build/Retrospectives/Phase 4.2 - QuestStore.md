# Phase 4.2 — QuestStore

**Date:** 2026-08-09  
**Outcome:** one durable document per Quest, with a safe one-time migration away from the alpha
World-wide log.

## The problem we actually had

`vault/worlds/<world>/quests.json` had become the persistence boundary by accident. Opening one
conversation deserialised every Chronicle in that World; writing one answer rewrote unrelated
work. It was increasingly visible as context and attachment use made the file larger, and it was
the wrong shape for the architecture: ADR-0025 says a Quest owns its Chronicle.

## What changed

```text
vault/worlds/default/
├─ quests/
│  ├─ q_<uuid-v7>.json
│  └─ ...
└─ quests.legacy-v1.json     # only after an alpha migration; never the live source
```

- The current conversation opens from the newest Quest document, rather than from a master log.
- A turn keeps its Quest id from preparation through agent/provider completion, approval,
  capability continuation and handover. Changing conversations while a turn runs cannot redirect
  the result into whichever Quest happens to be open later.
- Missions and the Bridge enumerate Quest documents only when their history projection is asked
  for; entering a World and taking a turn do not do that scan.
- Migration writes each Quest independently, preserves `quests.legacy-v1.json`, and refuses to
  overwrite a conflicting document.
- Each replacement is recoverable through a validated `.next` file and temporary `.previous`
  sibling. A write problem becomes a World problem rather than a silent claim that work survived.

## Evidence

- `cargo fmt --manifest-path BUILD/Cargo.toml --all -- --check`
- `cargo test --manifest-path BUILD/Cargo.toml --workspace --locked` — 489 passed, 3 external
  integrations intentionally ignored.
- `cargo check --manifest-path BUILD/Cargo.toml --workspace --locked`
- `npm --prefix ui run test` — 114 passed.
- `npm --prefix ui run build`

The focused QuestStore tests additionally exercise the normal migration, a conflicting existing
document, interrupted replacement recovery, and selecting exactly one Quest without reopening
the others.

## What this does not claim yet

This is the storage boundary, not the whole large-history programme. The History projection still
reads full Quest documents when the user opens it. Before the 1,000-Quest claim we will measure
that path, add paging/header caching only if the measurements require it, and virtualise a long
Chronicle in the surface. Physical `/compact` reduction and an attachment blob store are separate
next steps; no original conversation data is discarded by this delivery.
