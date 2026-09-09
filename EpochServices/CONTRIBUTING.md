# Contributing to EpochServices

## What this program is, and is not

It lends a machine's models to an Epoch Host. It holds no crew, no Quests, no
Chronicles and no knowledge, and it **cannot link `epoch-engine`** — that is a
crate boundary the compiler enforces, not a rule somebody has to remember. A change
that needs domain state belongs in Epoch, not here.

## Measure it

A claim about behaviour is a measurement, not a memory. Every verdict this program
gives is about *the machine it is running on*, and a confident answer about
somewhere else is worse than none. Pull requests that change behaviour should say
what was measured and what the number was.

## The shared crates

`epoch-kernel`, `epoch-models`, `epoch-assets` and `epoch-secrets` come from
[Epoch](https://github.com/KislokX/epoch), pinned by revision in `Cargo.toml`.
There is one copy of that code and it lives there. If your change needs one of them
altered, that is a pull request against Epoch first and a revision bump here after.

## Before you open a pull request

```bash
cd EpochServices
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI builds this on Windows, macOS and Linux. A change that only compiles on one of
them is a change that is not finished.

## Reporting something

Say what you did, what you expected and what happened. If a number is involved, the
number is the report.

## Licence

By contributing you agree that your contribution is licensed under Apache-2.0.
