# Observability

> Status: Designed (interface) · Owner ADR: [[../ADR/0015-activity-stream]] · Horizon: subscription NOW; Activity Recorder DESIGN NOW; profiler/monitor VISION

Not a "Logger". Observability is the subsystem for understanding what AIOS is doing - metrics, traces, activities, performance, debugging - built as a **consumer of the [[Activity Stream]]**, never coupled into the core.

## Purpose
Give humans and tools insight into the running system without any subsystem depending on it.

## Members
- **Live consumers (NOW):** subscribe to the Activity Stream for live UI/diagnostics; filter by Visibility/Importance.
- **Activity Recorder (DESIGN NOW, not implemented):** a future consumer that persists immutable append-only Activity logs for replay, auditing and diagnostics. Interface designed now so Phase 2 needs no architectural change.
- **Profiler / performance monitor / debug tools (VISION):** live here later.

## NOT responsible for
- Being a source of truth (repositories are - ADR-0014).
- Persisting canonical domain objects (Persistence) or knowledge (Knowledge Engine).
- Emitting Activities (subsystems do; Observability only consumes).

## Communication
Subscribes to the Activity Stream. Reads Activity metadata (Visibility, Importance) to decide what to surface/record. Never on the Command path.

## Why separate from Activity Stream
The Activity Stream only *distributes* events. Observability *interprets/records* them. Keeping them apart stops either from growing beyond its name.

## Open questions
- Activity Recorder log format + retention.
- Metrics/trace model (VISION).
