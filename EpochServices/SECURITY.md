# Security

## Which versions get fixes

EpochServices has not reached 1.0. **The current release is the only one that receives security
fixes.**

## What EpochServices is, so a report can be aimed properly

EpochServices lends one machine's models to an Epoch Host. It is a small program that **listens
on the network** and, unlike Epoch itself, most of its surface is reachable from another
computer. Treat that surface as the sensitive part.

- **Pairing** is a code, typed once and spent the moment it works. There is no account and
  nothing leaves the local network by design.
- **That first exchange trusts whoever answers.** Everything afterwards is pinned to the
  certificate that was seen, and a swapped one is refused — that is what the fingerprint is for.
  The first exchange has nothing to check against, so somebody already sitting between the two
  machines during those five minutes can be the machine you meant to pair with, and the code
  travels in that same channel. Accepted deliberately for an alpha paired across a home network,
  and said here rather than left to be found: it is **not** authenticated first contact, and
  nothing in either program should be read as claiming it is.
- **Two grants are asked separately**, because they are separate questions: whether a turn may
  run here, and whether that machine may drive a World from here.
- **The local pages** — starting a runtime, installing a model, measuring a curve — are the
  machine owner's, pressed on the machine itself.

**What is in scope**, and worth reporting:

- anything that reaches a route **without a valid pairing**, or that lets a revoked pairing keep
  working
- a way to make the service run, install or fetch something on behalf of a request that should
  not have been able to ask
- a page in a browser being able to drive the local routes (cross-site request forgery), or a
  request from an unexpected origin being honoured
- credentials or model paths leaking to an unpaired caller
- a request that can exhaust the machine — unbounded bodies, unbounded concurrency, a read with
  no deadline
- anything in the transport between Host and Services that another machine on the network can
  read or forge

**What is not in scope:** the owner of the machine using their own machine; a model producing a
wrong answer; anything that requires already having code execution on that machine; and **an
attacker positioned between the two machines during the five minutes of pairing** — named above,
accepted, and a report of it will be answered with that paragraph rather than a fix. Everything
*after* pairing is in scope: a pinned certificate that can be swapped is a real finding.

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
