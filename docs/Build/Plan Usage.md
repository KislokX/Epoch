# Plan usage instruments

The bridge can show two numbers that sound similar but are not interchangeable:

- **Context** is the size of one conversation window. It is reported with a turn and belongs to
  that character and Quest.
- **Plan usage** is the remaining allowance of the signed-in account. It is not derived from
  tokens, a Chronicle, or an estimate.

## Codex

Codex CLI `0.147.0` exposes its signed-in account's rate-limit snapshot through its local,
experimental `app-server` protocol:

1. start `codex app-server` over standard input/output;
2. send `initialize` declaring the experimental capability;
3. send the read-only `account/rateLimits/read` request;
4. read `rateLimits.primary.usedPercent`, `windowDurationMins`, and `resetsAt`; and
5. stop that short-lived process.

Epoch renders **remaining** percentage (`100 - usedPercent`). A 90% used reading therefore draws
a 10% bar. The protocol may expose a weekly primary window, a shorter secondary window, or both;
the UI names the window the CLI reports rather than claiming a fixed plan shape.

The app-server is explicitly experimental. Its only failure mode in Epoch is absence: malformed
JSON, an unavailable CLI, a timeout, an unsupported method, or a value outside 0--100 produces no
reading. It never changes account state, consumes reset credits, reads credentials, or changes
which agent can work.

## Claude Code

Claude Code 2.1.226 answers its built-in `/usage` command in print mode without starting an
agent turn:

1. run `claude -p /usage --output-format json --no-session-persistence`;
2. read the JSON result returned by the locally signed-in CLI; and
3. accept only `Current session: N% used` (5 hours) and `Current week: N% used` (seven days).

Epoch shows each reported **used** percentage directly in the two Claude HUD bars. The measured
call reports zero API duration, zero tokens and zero cost; it is an account reading, not a prompt.
The human-facing reset wording is intentionally not guessed into a timestamp. If Claude changes
the output, takes too long, is signed out, or reports a percentage outside 0--100, that one
instrument stays cold rather than displaying an estimate.

## Refresh and privacy

The World refreshes a plan reading on entry and at a modest interval. The process talks only to
the locally signed-in CLI. Epoch returns the percentage, reported window duration, reset instant,
and source label to the UI; account ids, access tokens, and credit identifiers stay inside Codex.

## Display contract

Claude's top HUD is an account **usage** instrument. Its striped fill and label show the value
the CLI reported as used: `19% used` fills 19% of the five-hour bar, and `97% used` fills 97% of
the weekly bar. Codex retains its established one-bar **charge** reading, so its fill means the
remaining plan room. Crew **ALLOWANCE** also means remaining room for the account assigned to that
character. The headings and accessible labels name the distinction so the same percentage is never
read in opposite directions.

Allowance probes start after the World has painted and run off the Tauri command runtime. A slow
or changed local CLI therefore leaves a cold reading; it never delays the World, Launcher or a
normal IPC response.
