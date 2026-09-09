/**
 * A request to open a terminal with a command already typed in it.
 *
 * ## Typed, never run
 *
 * This is the security line and it is deliberate. A character can *propose* a command — that is
 * useful, and the whole reason the feature exists — but a proposal it wrote must not become a
 * shell running on one click of a button it also caused to appear. That would give a model shell
 * access gated by nothing, and it is exactly what `run_command` exists to prevent: a model's
 * shell access is bounded, explained and approved, one call at a time.
 *
 * So the command arrives at a prompt with the cursor after it and **no newline**. The user reads
 * it, and pressing Enter is theirs. What Epoch removes is having to find a terminal, not having
 * to decide.
 *
 * ## Why a channel rather than a prop
 *
 * The conversation and the terminals are siblings under the World, and the request crosses from
 * one to the other through several components that have no business knowing either exists. A
 * module-level subscription is smaller than threading a callback through them, and it stays
 * honest because it carries **intent only** — nothing here decides whether a terminal opens, or
 * which one, or what happens next. The Terminals deck does.
 */

/** What was asked for: a command to prefill, and which shell would suit it. */
export interface TerminalRequest {
  readonly command: string;
  /**
   * The fence's language, as the character wrote it — `bash`, `powershell`, `cmd`, …
   *
   * A hint, never an instruction. The deck maps it to a shell **this machine has**, and falls
   * back to whatever it does have rather than refusing: a command written as `bash` is still
   * worth typing into PowerShell, and the user can see it before it runs.
   */
  readonly language: string | null;
}

type Listener = (request: TerminalRequest) => void;

const listeners = new Set<Listener>();

/** Ask for a terminal. Returns nothing: whether one opens is the deck's decision. */
export function askForTerminal(request: TerminalRequest): void {
  for (const listener of listeners) listener(request);
}

export function onTerminalAsked(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/**
 * Shell languages, and the ones that are deliberately not on the list.
 *
 * A fence tagged `json`, `toml` or `python` is not something to offer to type into a shell —
 * doing so would put a "run this" button under every configuration snippet a character shows,
 * and the button would be wrong every time. Only fences that claim to be shell commands get one.
 */
const SHELL_LANGUAGES = new Set([
  "bash",
  "sh",
  "shell",
  "zsh",
  "console",
  "terminal",
  "powershell",
  "ps",
  "ps1",
  "pwsh",
  "cmd",
  "bat",
  "batch",
]);

export function isShellLanguage(language: string | null): boolean {
  return language !== null && SHELL_LANGUAGES.has(language.trim().toLowerCase());
}

/**
 * The command inside a fenced block, ready to be typed.
 *
 * Two things are stripped, and both are what a person would strip by hand: the `$` or `PS>`
 * prompt a transcript carries, and any line that is a comment. What is left must be a single
 * command — a block of five lines is a script, and prefilling a shell with a script that runs
 * the moment Enter is pressed is not the small, readable thing this is for.
 *
 * `null` when there is nothing safe to offer, and the block simply stays a code block.
 */
export function runnableCommand(text: string): string | null {
  const lines = text
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter((line) => line.length > 0 && !line.startsWith("#"));
  if (lines.length !== 1) return null;

  const command = lines[0]!
    .replace(/^(?:\$|>|PS\s*[^>]*>|C:\\[^>]*>)\s*/u, "")
    .trim();
  if (command.length === 0) return null;

  // A newline would be the user pressing Enter, and pressing Enter is theirs.
  if (/[\r\n]/u.test(command)) return null;
  return command;
}
