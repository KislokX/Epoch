# Contributing to Epoch

Thank you for looking. Before code, three things about how this project decides.

## Measure it

The single rule this codebase runs on: **a claim about behaviour is a measurement,
not a memory.** Somebody else's command-line flag was true the day it was written
down and nothing tells you the day it stopped being. Ask the program.

Every architectural decision here was forced by evidence, and the evidence is in
the commit message and the comment beside the code. A pull request that changes
behaviour should say what it measured, on what, and what the number was.

## A gauge with nothing behind it must read empty

If a panel has no measurement behind it, it keeps its frame, loses its light and
says which subsystem will light it. Never invent a reading, never withhold one that
exists, and never show a real reading of a different quantity. This has cost this
project four separate defects and each one is written down.

The same rule inverted: **a gate with nothing behind it must stay open.** Reading
silence as *no* is the same invention as reading it as *yes*; it merely fails in
the direction that looks responsible.

## Where things go

| | |
|---|---|
| `docs/ADR` | how the system works — read these first |
| `CLAUDE.md` | the constitution, and every defect that produced a rule |
| `PRODUCT_ARCHITECTURE.md` | the map: kernel, engine, surfaces |
| `EXPERIENCE_CONSTITUTION.md` | how the product should feel |
| `CONTENT_PHILOSOPHY.md` | what may and may not ship as content |

`epoch-kernel` has **zero I/O** and depends on nothing. That is enforced by the
crate boundary, not by review. Business logic lives in `epoch-engine`; surfaces sit
over the engine and never touch the kernel.

## Before you open a pull request

```bash
cd BUILD
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check          # licences and advisories

cd ui
npx tsc --noEmit
npx vitest run
```

All of it runs in CI on every push. `cargo deny` is not a formality: Epoch links a
webview, an HTTP server and a JSON-RPC surface, and every dependency is somebody
else's code inside a process holding the user's filesystem capabilities.

## Things that will be sent back

- **A new abstraction with one caller.** Complexity has to justify itself against a
  real problem in the current horizon.
- **A control that does nothing**, or a dropdown whose options are assembled from
  what is conceivable rather than from what was measured.
- **A capability written down in a manifest.** Derive it. A capability somebody
  typed is one that will eventually lie.
- **Copyrighted game assets, of any kind.** Non-negotiable — see
  `CONTENT_PHILOSOPHY.md`.
- **A credential anywhere near a type that gets serialised.** Epoch opens the door
  and never holds the key.

## Reporting something

An issue that says what you did, what you expected and what actually happened is
worth more than a diagnosis. If a number is involved, the number is the report.

## Licence

By contributing you agree that your contribution is licensed under Apache-2.0, the
same licence as the project.
