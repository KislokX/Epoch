# Projects

> Scope: **Project** (see [[../Architecture/Scope Model]]) · Owner ADR: [[../ADR/0013-scope-model]]

A Project is an "adventure" inside a Workspace. It owns:
- Conversations
- Files
- Snapshots (Character Instances, Automation Runs)
- Project Knowledge (ADRs, PRDs, timelines, execution history, decisions)
- Context Reports
- Timeline
- Project Settings

Projects **reference** Character Definitions from the Workspace (never copy). Custom behavior = explicit override/fork preserving the original.

Full subsystem design pending (box: Project System). This file records scope ownership per ADR-0013.

