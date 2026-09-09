# ADR-0001: Desktop shell — Tauri

- Status: Accepted
- Date: 2026-07-22
- Depends on: —
- Related architecture: [[Domain Kernel]], [[../ARCHITECTURE]]

## Context
AIOS is a desktop application for Windows 11 (Mac/Linux later). It needs a native window, filesystem/process/OS access, and a rich UI. Core business logic lives in a compiled engine (ADR-0002, ADR-0003).

## Decision
Use **Tauri** as the desktop shell: Rust backend process + web-tech frontend (React/TypeScript) in the OS webview.

## Why this is best
- Rust-native backend matches the engine language (ADR-0002) — one language for all logic, no FFI seam between shell and engine.
- Small binary, low memory vs. Electron (no bundled Chromium).
- First-class OS access (fs, shell, process spawn) needed for Tools/Integrations.
- IPC command + event model fits the Engine/Presentation split (ADR-0003) exactly.

## Alternatives considered
- **Electron** — mature, huge ecosystem, but heavy (bundled Chromium); Node backend splits engine language away from Rust.
- **Native (egui/iced)** — pure Rust UI, but weak for the animation-rich JRPG UI; slower design iteration.
- **Flutter desktop** — good UI, but Dart backend fights the Rust engine decision.

## Consequences
- Frontend constrained to web tech (acceptable — matches team + UI ambition).
- Webview quirks vary per OS; test the target webview per platform.
- Enables headless engine reuse later (ADR-0003) — Tauri is just one shell over the engine.

## Assumptions
- Team comfortable with React/TypeScript for presentation.
- Target OS webviews capable enough for the intended UI.

## Risks
- Webview inconsistency across OSes. *Mitigation:* zero business logic in frontend; UI stays replaceable.

## Open questions
- Auto-update strategy (Tauri updater vs. custom) — defer to packaging phase.
