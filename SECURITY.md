# Security

## Which versions get fixes

Epoch has not reached 1.0. **The current release is the only one that receives security fixes**,
and there is no back-porting to earlier builds — saying otherwise would be a support commitment
nothing behind this repository could keep.

## What Epoch actually is, so a report can be aimed properly

Epoch is a desktop application that runs models, agents and tools **on the machine it is
installed on, at the user's request**. Several things that would be vulnerabilities in a web
service are the product here:

- **`run_command` runs commands.** It hands the line to a shell and can start any program on the
  machine. It is `Risk::High` and `Reversal::Permanent`, so the exact command is shown before it
  runs — unless the user has put that World in `Auto`, which is the user saying *free use of the
  tools this character has*. It offers **no operating-system sandbox**, and the working directory
  being confined to the project does not stop a program writing outside it. That is documented in
  `capabilities/machine.rs` and it is a deliberate capability, not a hole.
- **Epoch does not govern a coding agent's own tools.** Claude Code, Codex and Gemini CLI keep
  their own `Read`, `Write` and `Bash` and their own permission prompts. Epoch governs its own
  capabilities through Trust (ADR-0009) and says so.
- **A World's Project Root is read.** Pointing a World at a codebase and then at a hosted
  provider sends source code to that provider. Epoch asks; it does not decide.
- **Pairing two machines trusts whoever answers, once.** Everything after the first exchange is
  pinned to the certificate that was seen — a swapped certificate is refused, which is the whole
  point of the fingerprint. The **first** exchange has nothing to check against, so somebody
  already sitting between the two machines during those five minutes can be the machine you meant
  to pair with, and the code travels in that same channel. Accepted deliberately for an alpha
  paired across a home network, and written here rather than left to be discovered: it is *not*
  authenticated first contact, and nothing in Epoch should be read as claiming it is. The
  reasoning is in `epoch-wire/src/tls.rs`.

**What is in scope**, and worth reporting:

- anything that gets Epoch to run a command, write a file or reach the network **without the
  approval path that is supposed to gate it** — a capability that skips `decide()`, an argument
  that escapes path confinement, a tool result that a model can forge into an approval
- a way for a **web page, a document, a repository or a model** to make Epoch act on instructions
  it read — prompt injection that reaches a real side effect
- credential exposure: a key that leaves the operating system's secret store, ends up in a log, a
  World Pack, a Character Pack, an export or a crash report
- anything reachable **over the network** from another machine on the LAN: the paired-machine
  bridge, the MCP server Epoch hosts, the local HTTP routes
- the `epoch://` scheme escaping the folder it resolves in, or the webview gaining filesystem
  access it is not meant to have
- a supply-chain problem in the installer: what it fetches, from where, and whether it verifies it

**What is not in scope:** a user approving something destructive and it being destructive; a
model saying something wrong; a coding agent's own permission behaviour; anything that requires
already having code execution as that user on that machine; and **an attacker positioned between
two machines during the five minutes of pairing** — that one is named above, accepted, and a
report of it will be answered with this paragraph rather than a fix. Everything *after* pairing
is in scope: a pinned certificate that can be swapped is a real finding.

## Reporting a vulnerability

Use **GitHub's private vulnerability reporting** — the *Report a vulnerability* button under this
repository's **Security** tab. It opens a private thread with the maintainers; nothing is public
until an advisory is published.

Please do not open a public issue for a security problem, and please do not include real
credentials, API keys or private conversations in the report. A reproduction against a throwaway
key is worth more than a real one, and it is the only kind we can safely read.

There is no bug bounty. This is a personal project published as open source, and there is one
maintainer, so a first response may take a few days rather than a few hours. That is stated
rather than promised away.

**What helps most:** what you ran, what you expected, what happened, and the commit or release
you saw it on. If it needs a specific machine — a particular GPU, a runtime, an operating
system — say which, because most of this codebase is measurements of one machine and a report
that names the machine is a report that can be reproduced.

## Disclosure

We will confirm receipt, tell you whether it is something we can fix, and say when a fix ships.
If you would like credit in the advisory, say so and name how you want to be credited; if you
would rather not be named, that is the default.

Please give us a reasonable window before publishing details — 90 days is the usual ask, less if
a fix is already out.
