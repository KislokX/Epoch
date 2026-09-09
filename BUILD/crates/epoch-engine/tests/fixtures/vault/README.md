# A vault for the tests, and only for the tests

These are fixtures. Nobody lives here.

## Why this exists

The Launcher's tests used to read `BUILD/vault` — the vault of whichever machine was running
them. So `the_crew_comes_from_the_vault_and_carries_its_own_identity` asserted that a crew
existed, and what it actually measured was whether the developer happened to have characters that
day. Renaming a character could turn a test red; deleting one certainly would. A test that passes
or fails on the contents of somebody's home folder is not testing the code.

It also stood in the way of the repository meaning what it should. A vault holds a person's crew,
their conversations, their project paths and their standing permissions, and none of that is
something a user installs — so `BUILD/vault` had to leave the tree, and these tests had to stop
depending on it first.

## What belongs here

The smallest authored content that makes a projection answerable: enough of a crew to assert that
a crew is projected with its identity intact, and a Skill to assert that Skills are.

What must never appear here is anything real. No API keys — the type that carries them does not
exist in a Definition (ADR-0026) — no project roots, no Chronicles, no libraries. If a test needs
one of those it should build a scratch vault under `std::env::temp_dir()`, which several already
do, rather than adding it to this one and making every other test read it.

## The names

Archetype-descriptive placeholders, exactly as the default World Pack uses. Epoch's real cast is a
deliberately deferred milestone (`docs/Milestones/Official Epoch Universe.md`) and inventing a
name here would be inventing one there — a fixture is the easiest place for a placeholder to
quietly become canon.
