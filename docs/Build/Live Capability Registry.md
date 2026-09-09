# Live Capability Registry

## Why this starts Phase 5

A character's requested capabilities are intent. The registry is what a World can actually do.
They must never be confused: a check box cannot grant a tool that no live World offers, and a
tool supplied by an MCP server must not be invisible merely because it was not compiled into an
old suggestion list.

## Delivery 1: a World without a project is not a World without tools

Delivered 2026-08-10.

The registry is assembled for every turn from three sources:

1. `fetch_url` and `web_search` are always present. They do not operate in a Project Root.
2. File and command capabilities are added only when the active World has a Project Root.
3. MCP tools are added from the active bridge's current offer, whether or not the World has a
   Project Root.

The same registry supplies provider turns, resumed approvals, agent MCP serving, and the
Workspace reading. Thus a character cannot receive a capability in one path which another path
silently withholds.

The user-visible consequence is simple: before selecting a project folder, the crew can still
use web work and any configured MCP connection; they cannot pretend to read or edit a project
that has not been selected. As soon as a Project Root is selected, the file/tool set joins the
same registry.

## Remaining Phase 5 work

The Launcher character editor currently has an intentionally conservative suggestion list. It
cannot claim that a World-specific MCP tool exists while the Launcher has no active World. The
next slice will project the active World's live registry into the appropriate World-facing
character surface, preserving hand-authored unknown requests rather than deleting them. Skills
and vision follow that contract; they are not prompt strings or a second tool system.

## Evidence

`cargo test -p epoch-engine capabilities::tests` proves the rootless base registry contains
exactly the web capabilities. `cargo test -p epoch-tauri` covers the Tauri composition paths.
