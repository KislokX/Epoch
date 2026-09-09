/**
 * The words the user is about to send, and nothing else.
 *
 * A draft belongs to the open conversation, not to the World: it disappears with the box and is
 * never confused with a Chronicle entry. Its height is measured from its own content, capped by
 * the size the user chose for this dialogue. A fixed two-row field used to hide the third line
 * behind its border, which made text the user had written look absent.
 *
 * Enter is an intention to speak; Shift+Enter is an intention to keep writing. The component
 * stops both events from reaching the World shortcut layer, so pressing Enter in a conversation
 * cannot also move the camera.
 */

import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import type { ReactNode } from "react";
import { playSfx } from "../../experience/sfx";
import { listen, record, type Recording } from "../../experience/hearing";
import { converse, type Conversation } from "../../experience/converse";
import { hush } from "../../experience/speak";
import { Frame, Plate } from "./Frame";
import { fetchSettings, setMicrophone } from "../../ipc/launcher";
import { fetchOffered } from "../../ipc/world";
import type {
  ImageAttachmentInput,
  Offered,
  TextAttachmentInput,
} from "../../ipc/world";

/**
 * One line of the `/` menu.
 *
 * `run` and `insert` are the two kinds and exactly one is present: an entry either *does*
 * something or *types* something. A single optional callback would have made "what happens when
 * I press Enter" a question about which fields were filled in.
 */
interface SlashEntry {
  readonly key: string;
  readonly label: string;
  readonly detail: string;
  readonly run?: () => void;
  readonly insert?: string;
}

const MAX_ATTACHMENTS = 3;
const MAX_IMAGES = 4;
/**
 * The most one image may weigh.
 *
 * Matches the Engine's own limit (`import::MAX_BYTES`), so the surface refuses a file for the
 * same reason the Engine would rather than sending eight megabytes across IPC to be told no.
 * A courtesy, never a control — the Engine checks again (ADR-0024).
 */
const MAX_IMAGE_BYTES = 8 * 1024 * 1024;
const MAX_ATTACHMENT_BYTES = 8 * 1024;
const MAX_ATTACHMENT_TOTAL_BYTES = 16 * 1024;


/**
 * What to say about a language nobody chose.
 *
 * **The reading that was missing.** whisper guessed Greek on *"hazme una tabla con esos datos"*
 * and wrote it into the box with nothing on screen saying a decision had been made at all — so
 * it read as a bad ear rather than as a wrong language, and there was nothing to act on.
 *
 * `null` when a language was chosen, or when it was confident: an instrument that speaks on
 * every transcription is one people stop reading. The threshold is where a coin toss stops and
 * a decision starts, and it is deliberately generous — on this machine a clean Spanish sentence
 * reads `p = 0.993`.
 */
export function aboutTheLanguage(heard: {
  readonly heardAs?: string | null;
  readonly sure?: number | null;
}): string | null {
  /*
    **Nullish, not `undefined`.** `Option<String>` crosses as `null`, and this checked for
    `undefined` — so every transcription printed `Heard as null (0% sure)` under words that had
    been heard perfectly. The owner reported it as *"está tirando un error pero sí está
    sirviendo bien"*, which is exactly what it was.
  */
  const code = heard.heardAs ?? undefined;
  if (code === undefined || code === "") return null;
  /*
    **A pretty name must not be able to take a transcription down.**

    This read `Intl.DisplayNames(…).of(code)` bare, and the owner met
    `The ear stopped: RangeError: invalid_argument`. Measured in this WebView2: `of("")` and
    `of("auto")` both throw, `of("xx")` quietly answers `"xx"`. So an unknown code is harmless
    and a *non-language* is not — and whisper prints `auto` when it did not decide.

    Refused at the source as well (`hearing.rs`), and caught here, because a display concern has
    no business reaching the caller either way. The same rule as a job's optional last step: the
    thing that was actually asked for had already succeeded when this failed.
  */
  let named = code;
  try {
    if (typeof Intl.DisplayNames === "function") {
      named = new Intl.DisplayNames(undefined, { type: "language" }).of(code) ?? code;
    }
  } catch {
    // The code itself, which is what somebody would search for anyway.
  }
  const sure = heard.sure ?? undefined;
  if (sure !== undefined && sure >= 0.9) return null;
  return `Heard as ${named}${
    sure === undefined ? "" : ` (${Math.round(sure * 100)}% sure)`
  } — Settings → Audio devices is where you say which language you speak.`;
}

/**
 * Whether this is a picture Epoch can both store and draw.
 *
 * Exactly what the asset pipeline delivers, and no more: accepting a format Epoch cannot draw
 * would store something that resolves to nothing later, which is a broken image in a
 * conversation rather than a refusal at the moment of choosing.
 *
 * The Engine sniffs the bytes and has the final word (ADR-0024). This filter reads the type and
 * the name because that is all a browser offers before the file is read — a courtesy, never a
 * control.
 */
function isImageFile(file: File) {
  if (/^image\/(png|jpeg|webp|svg\+xml)$/.test(file.type)) return true;
  return /\.(?:png|jpe?g|webp|svg)$/i.test(file.name);
}

function isTextFile(file: File) {
  if (file.type.startsWith("text/")) return true;
  return /\.(?:md|txt|csv|json|ya?ml|toml|xml|html?|css|js|jsx|ts|tsx|rs|py|rb|go|java|c|h|cpp|hpp|sh|ps1|sql|log)$/i.test(
    file.name,
  );
}

interface DialogueComposerProps {
  /** Changing who is spoken to moves the cursor to their conversation. */
  readonly characterId: string;
  /** A Quest change is a different conversation, even when speaking to the same person. */
  readonly conversationId: string | null;
  readonly name: string;
  /** No model means this person genuinely cannot receive a message. */
  readonly model: string | null;
  /** A running turn keeps the draft visible but it is not sent. */
  readonly thinking: boolean;
  /** The dialogue's user-chosen log height caps a growing draft. */
  readonly logHeight: number;
  /** The user's own previous words in this Quest, oldest first, read from the Chronicle. */
  readonly history: readonly string[];
  readonly onSpeak: (
    draft: string,
    attachments: readonly TextAttachmentInput[],
    images: readonly ImageAttachmentInput[],
  ) => void | boolean | Promise<boolean>;
  readonly onHalt: () => void;
  // What this character can reach is deliberately **not** a prop. It used to be the World's
  // whole capability list, passed down and shown to everybody identically — which is a fact
  // about the World answering a question about a person. The menu asks the Engine per
  // character instead, and gets the table that turn would actually be given.
  /** Current Engine-owned controls, shown as information rather than duplicated controls. */
  readonly mode?: string | null;
  readonly reasoning?: string | null;
  /**
   * Whether a picture attached here actually reaches whoever answers.
   *
   * A fact about the turn, not about the model: an agent is handed the bytes, a model is
   * currently told only that a picture was shared. So the note has to be able to say either
   * thing — a character who *did* look at a screenshot, under a line claiming nobody can see
   * it, is the instrument lying about the very turn the user is watching.
   */
  readonly picturesReachThem: boolean;
  /** Present when this character can reduce the active Chronicle into a continuity brief. */
  readonly onCompact?: () => void;
  /** Context and reasoning are instruments beside the command button. */
  readonly children?: ReactNode;
}

export function DialogueComposer({
  characterId,
  conversationId,
  name,
  model,
  thinking,
  logHeight,
  history,
  onSpeak,
  onHalt,
  mode,
  reasoning,
  picturesReachThem,
  onCompact,
  children,
}: DialogueComposerProps) {
  const [draft, setDraft] = useState("");

  /**
   * Three states, because a person needs to tell them apart.
   *
   * `recording` is the microphone open; `hearing` is whisper working, which is about a second
   * for a breath. A single boolean would make the second one look like the first, and pressing
   * again during it would cancel a transcription that was nearly done.
   */
  const [hearing, setHearing] = useState<"off" | "recording" | "hearing">("off");
  const [misheard, setMisheard] = useState<string | null>(null);
  const recording = useRef<Recording | null>(null);

  /**
   * Whether Epoch is asking for the microphone, in its own frame.
   *
   * **The leak this closes.** Pressing `SPEAK` used to go straight to `getUserMedia`, and Edge
   * put its own prompt over the World — *"http://tauri.localhost wants to use your
   * microphones"*, a URL in somebody else's frame. The owner missed it and reported the button
   * as dead. Nothing was broken; the product simply looked it.
   */
  const [asking, setAsking] = useState(false);

  /**
   * Hands free: the World listens until it is told to stop (Phase 15, step 6).
   *
   * `"room"` is the half-second in which the noise floor is being measured — its own state
   * because a control whose only feedback is a label must not have a moment where the label is
   * lying. `"open"` is listening; `"heard"` is whisper working on a finished sentence; and
   * `"theirs"` is holding off because a character is talking, which is information rather than
   * a gap.
   */
  const [freehand, setFreehand] = useState<
    "off" | "room" | "open" | "heard" | "theirs"
  >("off");
  const conversation = useRef<Conversation | null>(null);

  /**
   * What is in the box right now, readable without depending on it.
   *
   * The hands-free loop is set up once and lives for as long as the microphone is open, so it
   * cannot close over `draft` — and reading it through a state updater, which was the first
   * answer, puts a side effect somewhere React may run twice.
   */
  const draftNow = useRef("");

  /**
   * Say it out loud, and put the words in the draft.
   *
   * **Never sent.** What somebody said aloud is a first version — it may have a name wrong, or a
   * word the room swallowed — and a press that fires a turn is a press nobody can take back.
   */
  const speakIntoTheDraft = async () => {
    setMisheard(null);
    if (hearing === "hearing") return;

    if (hearing === "recording") {
      const open = recording.current;
      recording.current = null;
      setHearing("hearing");
      const wav = await open?.stop();
      if (!wav) {
        setHearing("off");
        setMisheard("Nothing was recorded.");
        return;
      }
      const heard = await listen(wav);
      setHearing("off");
      if (typeof heard === "string") {
        setMisheard(heard);
        return;
      }
      // Appended rather than replacing: somebody may have typed half a sentence and then said
      // the rest, and losing what was already in the box is not something a microphone may do.
      setDraft((already) => (already ? `${already} ${heard.text}` : heard.text));
      // Said beside the words it is about, never as a toast: the draft is where somebody is
      // looking, and a note about the microphone belongs beside the microphone.
      setMisheard(aboutTheLanguage(heard));
      input.current?.focus();
      return;
    }

    // **Epoch asks first, in the World.** Only after somebody has answered does the browser's
    // request arrive, and by then the Engine has an answer to give it (`microphone.rs`).
    // `null` is nobody-has-been-asked; `false` is a refusal, kept, so the question does not come
    // back on every press.
    const said = (await fetchSettings()).microphone;
    if (said === null) {
      setAsking(true);
      return;
    }
    if (said === false) {
      setMisheard(
        "The microphone is off for Epoch. Settings → Audio devices turns it back on.",
      );
      return;
    }

    try {
      recording.current = await record();
      setHearing("recording");
    } catch (why) {
      // Refused, or no microphone. Said here rather than as a silence somebody has to diagnose.
      setMisheard(
        `The microphone could not be opened: ${String(why)}. Settings → Audio devices.`,
      );
    }
  };

  /**
   * Talk to the World without pressing anything.
   *
   * **This is the one place the microphone sends**, and step 5's rule — *it writes into the
   * draft and never sends* — is not being quietly broken: it is a mode somebody turns on, whose
   * whole purpose is that there is nothing to press. Pressing SPEAK still only writes.
   *
   * Everything here composes what already exists. The roadmap named this step's real cost as
   * cancelling a turn in flight; `stop_turn` was already built, so what was actually missing is
   * knowing when a sentence has ended.
   */
  const handsFree = async () => {
    setMisheard(null);
    if (conversation.current) {
      conversation.current.stop();
      conversation.current = null;
      setFreehand("off");
      return;
    }

    const said = (await fetchSettings()).microphone;
    if (said === null) {
      setAsking(true);
      return;
    }
    if (said === false) {
      setMisheard(
        "The microphone is off for Epoch. Settings → Audio devices turns it back on.",
      );
      return;
    }

    try {
      setFreehand("room");
      conversation.current = await converse({
        onRoom: () => setFreehand("open"),
        // Holding off while somebody else talks, said out loud. The World hearing its own
        // voice is what made the first version repeat one paragraph forever.
        onTheirTurn: (holding) =>
          setFreehand((was) =>
            was === "off" || was === "room" || was === "heard"
              ? was
              : holding
                ? "theirs"
                : "open",
          ),
        onStart: () => {
          /*
            **`hush`, never `stopSpeaking`.** This called the second, which also wipes the
            memory of what has already been said — and the Chronicle is re-read on every render,
            so every answer on screen became something that had not been said yet and was said
            again. That was the loop the owner watched: five entries on disk, one paragraph
            forever. *Be quiet now* and *this conversation is over* are two questions.

            The detector no longer runs while a character is speaking, so this can only catch a
            line that is queued rather than playing. Halting the turn is the half that still
            matters and still fires: the model is answering something the user has just changed
            their mind about, and that is true before a word is spoken.
          */
          hush();
          if (thinking) onHalt();
        },
        onEnd: (wav) => {
          /*
            **`.catch` on the end, and it is not defensive programming.**

            This was `void (async () => …)()` with nothing after it, so anything that threw in
            here rejected into nowhere: the label stayed on `HEARING…`, the microphone stayed
            open, and the product said nothing at all. The owner met exactly that and could only
            report *"se queda en listening, después hearing y no pasa nada"* — which is all
            there was to see.

            A mode whose only feedback is a label must not have a way to stop moving it. What
            the failure *was* is still unknown; this is what makes the next one say so.
          */
          void (async () => {
            setFreehand("heard");
            const heard = await listen(wav);
            setFreehand(conversation.current ? "open" : "off");
            if (typeof heard === "string") {
              setMisheard(heard);
              return;
            }
            /*
              Sent, because that is what this mode is. Anything already typed goes with it —
              losing a half-written sentence is not something a microphone may do.

              **The send is not inside the state updater.** It was, to read the current draft
              without a dependency, and an updater is a function React is free to call more than
              once — StrictMode calls every one of them twice on purpose. A side effect in there
              is a turn sent twice, and it would only ever show up on somebody else's machine.
            */
            setMisheard(aboutTheLanguage(heard));
            const already = draftNow.current;
            const whole = already ? `${already} ${heard.text}` : heard.text;
            setDraft("");
            onSpeak(whole, [], []);
          })().catch((why: unknown) => {
            setFreehand(conversation.current ? "open" : "off");
            setMisheard(`The ear stopped: ${String(why)}`);
          });
        },
      });
    } catch (why) {
      setFreehand("off");
      setMisheard(
        `The microphone could not be opened: ${String(why)}. Settings → Audio devices.`,
      );
    }
  };

  // Leaving the conversation releases the microphone — both of them. A recording nobody will
  // read is a light on somebody's camera for no reason, and a hands-free session that outlived
  // the window it was opened in would keep sending turns into a conversation nobody is watching.
  // One line, next to the state it mirrors, so the two cannot drift.
  useEffect(() => {
    draftNow.current = draft;
  }, [draft]);

  useEffect(() => {
    return () => {
      recording.current?.cancel();
      recording.current = null;
      conversation.current?.stop();
      conversation.current = null;
    };
  }, []);
  const [attachments, setAttachments] = useState<
    readonly TextAttachmentInput[]
  >([]);
  const [images, setImages] = useState<readonly ImageAttachmentInput[]>([]);
  const [attachmentError, setAttachmentError] = useState<string | null>(null);
  const [draggingFiles, setDraggingFiles] = useState(false);
  const [historyIndex, setHistoryIndex] = useState<number | null>(null);
  const [highlight, setHighlight] = useState(0);
  const input = useRef<HTMLTextAreaElement>(null);
  const fileInput = useRef<HTMLInputElement>(null);

  useEffect(() => {
    // A draft is an intention for this conversation, never the next Quest's message wearing the
    // old cursor. The Chronicle remains the only durable history; this only resets navigation.
    setDraft("");
    setAttachments([]);
    setAttachmentError(null);
    setHistoryIndex(null);
    input.current?.focus();
  }, [characterId, conversationId]);

  useLayoutEffect(() => {
    const field = input.current;
    if (!field) return;
    field.style.height = "auto";
    field.style.height = `${Math.min(field.scrollHeight, Math.round(logHeight * 0.7))}px`;
  }, [draft, logHeight]);

  const send = async () => {
    if (!model || thinking) return;
    const accepted = await onSpeak(draft, attachments, images);
    // `undefined` keeps shallow component callers compatible; the real turn hook returns a
    // boolean and treats a missing engine receipt as a refused attachment.
    if (accepted === false) {
      setAttachmentError(
        "Your text was sent, but this desktop engine did not confirm the attachment. Restart Epoch, then send it again.",
      );
      return;
    }
    setDraft("");
    setAttachments([]);
    setImages([]);
    setAttachmentError(null);
    setHistoryIndex(null);
  };

  /**
   * Read the pictures out of a selection, and say what could not be taken.
   *
   * Base64 rather than a path, because the frontend has no filesystem and must never be given
   * one (ADR-0024). `FileReader` rather than a hand-rolled loop over the bytes: a megabyte
   * screenshot through `String.fromCharCode` builds a megabyte-long argument list.
   */
  const addImages = useCallback(
    async (chosen: readonly File[]) => {
      if (chosen.length > MAX_IMAGES - images.length) {
        setAttachmentError(`A message can carry at most ${MAX_IMAGES} images.`);
        return;
      }
      if (chosen.some((file) => file.size > MAX_IMAGE_BYTES)) {
        setAttachmentError("Each image must be 8 MB or smaller.");
        return;
      }
      try {
        const read = await Promise.all(
          chosen.map(
            (file) =>
              new Promise<ImageAttachmentInput>((resolve, reject) => {
                const reader = new FileReader();
                reader.onerror = () => reject(new Error(file.name));
                reader.onload = () => {
                  const url = String(reader.result);
                  resolve({
                    name: file.name,
                    // Everything after the comma. The prefix is the reader's, not the data's.
                    data: url.slice(url.indexOf(",") + 1),
                    bytes: file.size,
                  });
                };
                reader.readAsDataURL(file);
              }),
          ),
        );
        setImages((current) => [...current, ...read]);
        setAttachmentError(null);
      } catch {
        setAttachmentError("Epoch could not read that image.");
      }
    },
    [images],
  );

  const addFiles = useCallback(
    async (files: FileList | readonly File[]) => {
      const all = Array.from(files);
      if (all.length === 0) return;
      // **Sorted by what they are, not by what was asked for.** One picker, one drop target,
      // and the user simply shares a thing; making them choose between two buttons would be
      // Epoch's storage arrangement showing through the surface.
      const pictures = all.filter(isImageFile);
      const chosen = all.filter((file) => !isImageFile(file));
      if (pictures.length > 0) await addImages(pictures);
      if (chosen.length === 0) return;
      const available = MAX_ATTACHMENTS - attachments.length;
      if (chosen.length > available) {
        setAttachmentError(
          `A message can carry at most ${MAX_ATTACHMENTS} text files.`,
        );
        return;
      }
      if (
        chosen.some((file, at) =>
          chosen
            .slice(0, at)
            .some(
              (earlier) =>
                earlier.name.toLowerCase() === file.name.toLowerCase(),
            ),
        ) ||
        chosen.some((file) =>
          attachments.some(
            (attachment) =>
              attachment.name.toLowerCase() === file.name.toLowerCase(),
          ),
        )
      ) {
        setAttachmentError("Attach each file name only once per message.");
        return;
      }
      if (chosen.some((file) => !isTextFile(file))) {
        setAttachmentError(
          "Attach a text file, a PNG, a JPEG, a WebP or an SVG. Other documents will arrive with their own context path.",
        );
        return;
      }
      if (chosen.some((file) => file.size > MAX_ATTACHMENT_BYTES)) {
        setAttachmentError("Each attachment must be 8 KB or smaller.");
        return;
      }
      if (
        attachments.reduce((total, attachment) => total + attachment.bytes, 0) +
          chosen.reduce((total, file) => total + file.size, 0) >
        MAX_ATTACHMENT_TOTAL_BYTES
      ) {
        setAttachmentError("Attachments together must be 16 KB or smaller.");
        return;
      }

      try {
        const added = await Promise.all(
          chosen.map(async (file) => ({
            name: file.name,
            content: await file.text(),
            bytes: file.size,
          })),
        );
        if (added.some((attachment) => attachment.content.includes("\0"))) {
          setAttachmentError("One attachment is not text.");
          return;
        }
        setAttachments((current) => [...current, ...added]);
        setAttachmentError(null);
      } catch {
        setAttachmentError("Epoch could not read that text file.");
      }
    },
    [attachments],
  );

  useEffect(() => {
    const isOverThisDialogue = (target: EventTarget | null) => {
      const dialogue = input.current?.closest(".dlg");
      return target instanceof Node && dialogue?.contains(target);
    };
    const hasFiles = (event: DragEvent) =>
      event.dataTransfer?.types.includes("Files") ?? false;
    const canAccept = () => Boolean(model) && !thinking;
    const onDragEnter = (event: DragEvent) => {
      if (!canAccept() || !hasFiles(event) || !isOverThisDialogue(event.target))
        return;
      event.preventDefault();
      setDraggingFiles(true);
    };
    const onDragOver = (event: DragEvent) => {
      if (!canAccept() || !hasFiles(event) || !isOverThisDialogue(event.target))
        return;
      event.preventDefault();
    };
    const onDragLeave = (event: DragEvent) => {
      if (!isOverThisDialogue(event.target)) return;
      const dialogue = input.current?.closest(".dlg");
      if (
        !(event.relatedTarget instanceof Node) ||
        !dialogue?.contains(event.relatedTarget)
      ) {
        setDraggingFiles(false);
      }
    };
    const onDrop = (event: DragEvent) => {
      if (!canAccept() || !hasFiles(event) || !isOverThisDialogue(event.target))
        return;
      event.preventDefault();
      setDraggingFiles(false);
      void addFiles(event.dataTransfer?.files ?? []);
    };

    // A conversation is the drop target, not merely its small text field. Listening at the
    // document lets a file be released over the Chronicle itself while still refusing drops
    // anywhere outside this particular dialogue.
    document.addEventListener("dragenter", onDragEnter);
    document.addEventListener("dragover", onDragOver);
    document.addEventListener("dragleave", onDragLeave);
    document.addEventListener("drop", onDrop);
    return () => {
      document.removeEventListener("dragenter", onDragEnter);
      document.removeEventListener("dragover", onDragOver);
      document.removeEventListener("dragleave", onDragLeave);
      document.removeEventListener("drop", onDrop);
    };
  }, [addFiles, model, thinking]);

  const removeAttachment = (name: string) => {
    setAttachments((current) =>
      current.filter((attachment) => attachment.name !== name),
    );
    setAttachmentError(null);
  };

  const recallPrevious = () => {
    if (history.length === 0) return;
    const next =
      historyIndex === null
        ? history.length - 1
        : Math.max(0, historyIndex - 1);
    const recalled = history[next];
    if (recalled === undefined) return;
    setHistoryIndex(next);
    setDraft(recalled);
  };

  const recallNext = () => {
    if (historyIndex === null) return;
    const next = historyIndex + 1;
    if (next >= history.length) {
      setHistoryIndex(null);
      setDraft("");
      return;
    }
    const recalled = history[next];
    if (recalled === undefined) return;
    setHistoryIndex(next);
    setDraft(recalled);
  };

  const runCompact = useCallback(() => {
    if (!onCompact) return;
    setDraft("");
    setHistoryIndex(null);
    onCompact();
  }, [onCompact]);

  /*
    The `/` menu, and the reason it is short.

    Everything in it is read from the Engine at the moment it opens — the table this character
    would actually be given, and who else lives here. Nothing is a list this file keeps, which
    is why `/playwright` appears the day a server is connected and disappears the day it is
    forgotten, without anybody editing a frontend.

    Two kinds of entry, and the difference is stated rather than implied:

    - an **action** happens (`/compact` reduces the conversation);
    - a **reference** types something for you, and the description says what naming it does.

    A Skill is deliberately absent. Skills are given to a character in the editor and apply to
    every turn; offering `/code-review` here would suggest a per-turn invocation the Engine does
    not have — a control that does nothing, which is the gauge nobody can explain.
  */
  const [offered, setOffered] = useState<Offered | null>(null);
  /*
    The word being typed, when it is a `/` word.

    Anchored to the **end of the draft** rather than to its start. Requiring `/` to be the first
    character meant "utiliza /" offered nothing, which reads as the menu being broken — a user
    who types a slash has asked for it, wherever the sentence had got to.

    Deliberately the last word and not "wherever the caret happens to be". A caret-aware version
    would need the selection on every render and would open the menu when somebody clicked into
    the middle of a finished sentence — a menu that appears without being asked for is worse
    than one that appears slightly less often.
  */
  const typing = /(?:^|\s)\/(\S*)$/.exec(draft);
  const slashOpen = !thinking && typing !== null;
  const query = (typing?.[1] ?? "").toLowerCase();

  useEffect(() => {
    if (!slashOpen) return;
    let current = true;
    void fetchOffered(characterId).then((answer) => {
      if (current) setOffered(answer);
    });
    return () => {
      current = false;
    };
    // Re-read on open rather than once: Trust, the mode and the connected servers all change
    // while a conversation is open, and a menu cached from earlier would offer a stale table.
  }, [slashOpen, characterId]);

  const entries: SlashEntry[] = [];
  if (onCompact) {
    entries.push({
      key: "compact",
      label: "/compact",
      detail:
        "Summarise this conversation; the next turn carries its continuity brief.",
      run: runCompact,
    });
  }
  for (const tool of offered?.tools ?? []) {
    entries.push({
      key: `tool:${tool.id}`,
      label: `/${tool.id}`,
      detail: tool.summary,
      insert: tool.id,
    });
  }
  for (const mate of offered?.crew ?? []) {
    entries.push({
      key: `crew:${mate.id}`,
      label: `/${mate.id}`,
      detail: `${mate.role} — naming them offers to hand this work over.`,
      insert: mate.name,
    });
  }
  const matches = entries.filter((entry) =>
    entry.label.slice(1).toLowerCase().includes(query),
  );
  const chosen =
    matches.length === 0 ? -1 : Math.min(highlight, matches.length - 1);

  const take = (entry: SlashEntry) => {
    if (entry.run) {
      entry.run();
      return;
    }
    // **Only the `/` word is replaced.** Everything already written stays exactly as it was:
    // Epoch finishes a word in the user's sentence, it does not write the sentence. Rewriting
    // the whole draft was survivable while the menu only opened at position zero and would
    // have started deleting people's words the moment it stopped.
    const before = typing
      ? draft.slice(0, typing.index + typing[0].indexOf("/"))
      : "";
    setDraft(`${before}${entry.insert} `);
    setHighlight(0);
    input.current?.focus();
  };

  return (
    <>
      <div className="dlg__compose">
        {draggingFiles && (
          <p className="dlg__drop-hint" role="status">
            Drop text files to attach them to this message.
          </p>
        )}
        {slashOpen && (
          <div
            className="dlg__slash"
            role="listbox"
            aria-label="Commands and what this character can reach"
          >
            <div className="dlg__slash-head">
              <b>{name.toUpperCase()} CAN REACH</b>
              {/* The Engine's own sentence about what decides the list below — different for a
                  model and for an agent, and it used to say neither. */}
              <span>{offered?.gate ?? "Read from the Engine, not a list"}</span>
            </div>
            {matches.map((entry, at) => (
              <button
                key={entry.key}
                type="button"
                role="option"
                aria-selected={at === chosen}
                // Keep the highlighted entry in view. The list scrolls, so walking past its
                // bottom edge moved a highlight nobody could see any more — which looks exactly
                // like the arrows having stopped working, the defect they were just freed from.
                ref={
                  at === chosen
                    ? (element) =>
                        element?.scrollIntoView?.({ block: "nearest" })
                    : undefined
                }
                className={`dlg__slash-action${at === chosen ? " dlg__slash-action--on" : ""}`}
                onMouseEnter={() => setHighlight(at)}
                onClick={() => take(entry)}
              >
                <b>{entry.label}</b>
                <span>{entry.detail}</span>
              </button>
            ))}
            {matches.length === 0 && (
              <p className="dlg__slash-section">
                <span>
                  {offered === null
                    ? "Reading what they can reach…"
                    : `Nothing here matches “${query}”.`}
                </span>
              </p>
            )}
            {/*
              Asked for and not available. Kept visible and deliberately unselectable: the
              difference between "Epoch cannot do that" and "this character was not given it"
              has two different fixes, and an absence says neither.
            */}
            {(offered?.withheld.length ?? 0) > 0 && (
              <div className="dlg__slash-section">
                <b>ASKED FOR, NOT AVAILABLE</b>
                <span>{offered?.withheld.join(" · ")}</span>
              </div>
            )}
            <div className="dlg__slash-section">
              <b>SETTINGS</b>
              <span>
                {mode ? `Mode: ${mode}` : "Mode unavailable"}
                {reasoning ? ` · Reasoning: ${reasoning}` : ""}
              </span>
            </div>
          </div>
        )}
        <textarea
          ref={input}
          rows={2}
          value={draft}
          placeholder={
            model ? `Say something to ${name}…` : "No model assigned"
          }
          disabled={!model}
          onChange={(event) => {
            setDraft(event.target.value);
            playSfx("type");
          }}
          onKeyDown={(event) => {
            event.stopPropagation();
            // The menu owns the arrows while it is open, and only then. Chronicle recall keeps
            // them the rest of the time, so neither has to know the other exists.
            if (slashOpen && matches.length > 0) {
              if (event.key === "ArrowDown") {
                event.preventDefault();
                setHighlight((at) => (at + 1) % matches.length);
                return;
              }
              if (event.key === "ArrowUp") {
                event.preventDefault();
                setHighlight(
                  (at) => (at - 1 + matches.length) % matches.length,
                );
                return;
              }
            }
            if (slashOpen && event.key === "Escape" && typing) {
              // Closing the menu without losing what was typed: a draft is the user's, and a key
              // that discards words is a key nobody presses twice. Only the `/` goes — it is
              // what asked for the menu, so it is what dismissing the menu takes back.
              event.preventDefault();
              const at = typing.index + typing[0].indexOf("/");
              setDraft(draft.slice(0, at) + draft.slice(at + 1));
              return;
            }
            // Arrow keys still edit a multiline draft. Recall begins only from an empty field, and
            // continues while already walking the Chronicle so repeated Up reaches older words.
            if (
              event.key === "ArrowUp" &&
              (historyIndex !== null ||
                (draft.length === 0 &&
                  event.currentTarget.selectionStart === 0))
            ) {
              event.preventDefault();
              recallPrevious();
              return;
            }
            if (event.key === "ArrowDown" && historyIndex !== null) {
              event.preventDefault();
              recallNext();
              return;
            }
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              // A draft that is still a `/` query is not a message. Sending it would put the
              // half-typed name of a tool into the Chronicle as something the user said.
              if (slashOpen && chosen >= 0) {
                const entry = matches[chosen];
                if (entry) take(entry);
                return;
              }
              send();
            }
          }}
        />
        {attachments.length > 0 && (
          <div className="dlg__attachments" aria-label="Attached text files">
            {attachments.map((attachment) => (
              <span className="dlg__attachment" key={attachment.name}>
                <b>{attachment.name}</b>
                <span>{attachment.bytes} B</span>
                <button
                  type="button"
                  onClick={() => removeAttachment(attachment.name)}
                  aria-label={`Remove ${attachment.name}`}
                >
                  ×
                </button>
              </span>
            ))}
          </div>
        )}
        {images.length > 0 && (
          <div className="dlg__images" aria-label="Attached images">
            {images.map((image, at) => (
              <span className="dlg__image" key={`${image.name}-${at}`}>
                <img
                  src={`data:image/*;base64,${image.data}`}
                  alt={image.name}
                />
                <b>{image.name}</b>
                <button
                  type="button"
                  onClick={() =>
                    setImages((current) => current.filter((_, i) => i !== at))
                  }
                  aria-label={`Remove ${image.name}`}
                >
                  ×
                </button>
              </span>
            ))}
            {/*
              **Said before it is sent, not discovered afterwards.**

              An image reaching the Chronicle is not an image reaching whoever answers, and
              which of the two happens depends on the brain. An agent is handed the bytes; a
              model is currently told only that a picture was shared, because sight is a
              capability and `see_image` is not wired into a turn yet.

              So this names the character rather than claiming something about everybody. It
              said "nobody can see it yet" while Mage — on an agent — described the screenshot
              correctly in the next breath. A line that contradicts the turn the user is
              watching teaches them to stop reading the notes.

              The turn says its half too: a model told only that a file was shared will
              describe it from its name, so `user_message` asks it to say plainly that it
              cannot see. Both are needed — this one so the user knows before spending a turn,
              that one so the answer cannot be a guess.
            */}
            <p className="dlg__image-note">
              {picturesReachThem ? (
                <>Kept in the conversation. {name} is handed the picture itself.</>
              ) : (
                <>
                  Kept in the conversation. {name} is only told a picture was
                  shared — describe it or paste the text if you need an answer
                  about what is in it.
                </>
              )}
            </p>
          </div>
        )}
        {attachmentError && (
          <p className="dlg__attachment-error" role="status">
            {attachmentError}
          </p>
        )}
        <input
          ref={fileInput}
          className="dlg__file-input"
          type="file"
          aria-label="Attach text files"
          multiple
          accept="image/png,image/jpeg,image/webp,image/svg+xml,.png,.jpg,.jpeg,.webp,.svg,text/*,.md,.txt,.csv,.json,.yaml,.yml,.toml,.xml,.html,.htm,.css,.js,.jsx,.ts,.tsx,.rs,.py,.rb,.go,.java,.c,.h,.cpp,.hpp,.sh,.ps1,.sql,.log"
          onChange={(event) => {
            void addFiles(event.currentTarget.files ?? []);
            event.currentTarget.value = "";
          }}
        />
      </div>
      {/*
        What the ear had to say when it could not. Never a toast: the draft is where the user is
        looking, and a failure about the microphone belongs beside the microphone.
      */}
      {/*
        **Epoch's own question, in the World's own frame.** A `Frame` like every other window
        here, so a World Pack re-skins it with the rest — and it says what the microphone is
        *for*, which a URL cannot.

        It is not a substitute for the browser's consent: answering yes here is what lets
        `microphone.rs` answer WebView2, and the operating system's own privacy settings are
        untouched either way. Epoch opens the door; it never holds the key.
      */}
      {asking && (
        <Frame
          corner={10}
          fill="var(--ep-window-2)"
          className="dlg__asking-frame"
          role="dialog"
          aria-label="The World would like to listen"
        >
          <div className="dlg__studioBar">
            <Plate>The World would like to listen</Plate>
          </div>
          <p className="dlg__asking">
            Speaking writes into this box and never sends. What you say is turned into words{" "}
            <b>on this machine</b> — nothing is uploaded, and the recording is thrown away as
            soon as it has been read.
          </p>
          <div className="dlg__asking-acts">
            <button
              type="button"
              className="epbtn epbtn--primary"
              onClick={() => {
                void (async () => {
                  setAsking(false);
                  await setMicrophone(true);
                  // Straight on to the thing they pressed for. Asking somebody to press the
                  // same button twice is the shape this was written to remove.
                  await speakIntoTheDraft();
                })();
              }}
            >
              LET IT LISTEN
            </button>
            <button
              type="button"
              className="epbtn"
              onClick={() => {
                void (async () => {
                  setAsking(false);
                  // **Kept, not forgotten.** A refusal nobody writes down is a question that
                  // comes back every time, which teaches people to stop reading it.
                  await setMicrophone(false);
                  setMisheard("Left off. Settings → Audio devices can turn it on later.");
                })();
              }}
            >
              NOT NOW
            </button>
          </div>
        </Frame>
      )}

      {misheard !== null && (
        <p className="dlg__attachment-error" role="status">
          {misheard}
        </p>
      )}
      <div className="dlg__ctrl">
        <button
          type="button"
          className="epbtn dlg__attach"
          disabled={!model || thinking}
          onClick={() => fileInput.current?.click()}
          title="Attach text (8 KB each) or an image (PNG, JPEG, WebP, SVG) to this message"
        >
          ATTACH
        </button>
        {/*
          **The microphone** (Phase 15, step 5). It writes into the draft rather than sending:
          what somebody said out loud is a first version, and a press that fires a turn is a
          press nobody can take back. The words land where they can be read, edited and then
          sent by the same key that sends everything else.

          Its own state on the button rather than a spinner: `LISTENING` while the microphone is
          open, `HEARING…` while whisper works, and back. About a second for a breath, measured —
          which is why there is no bar. A bar for one second is furniture.
        */}
        <button
          type="button"
          className="epbtn dlg__attach"
          disabled={!model || thinking}
          onClick={() => void speakIntoTheDraft()}
          title={
            hearing === "off"
              ? "Say it out loud. The words land in the box; nothing is sent until you send it."
              : "Stop and write down what was said"
          }
          aria-pressed={hearing !== "off"}
        >
          {hearing === "off" ? "SPEAK" : hearing === "recording" ? "LISTENING" : "HEARING…"}
        </button>
        {/*
          **Hands free** (Phase 15, step 6) — its own control beside SPEAK, not a mode SPEAK
          switches into. A button whose meaning depends on hidden state is one this codebase has
          already deleted once (`Manual`, ADR-0027), and *press to talk* and *talk to me* are two
          different things a person wants at two different moments.

          Five labels, because there are five states and a control whose only feedback is a
          label must not have one it cannot show. `THE ROOM` is the half-second in which the
          noise floor is measured — a multiple of what is actually there rather than a number
          that worked in the room this was written in. `THEIR TURN` is the ear holding off while
          a character talks, which is information and not a gap: the World listening to its own
          voice is what made the first version repeat one paragraph forever.
        */}
        <button
          type="button"
          className={`epbtn dlg__attach${freehand !== "off" ? " dlg__attach--live" : ""}`}
          disabled={!model}
          onClick={() => void handsFree()}
          title={
            freehand === "off"
              ? "Talk without pressing anything. Sentences are sent as you finish them, and starting to speak interrupts whoever is talking."
              : "Stop listening"
          }
          aria-pressed={freehand !== "off"}
        >
          {freehand === "off"
            ? "HANDS FREE"
            : freehand === "room"
              ? "THE ROOM…"
              : freehand === "heard"
                ? "HEARING…"
                : freehand === "theirs"
                  ? "THEIR TURN"
                  : "LISTENING"}
        </button>
        {children}
        {thinking ? (
          <button
            type="button"
            className="epbtn dlg__send"
            onClick={onHalt}
            title="Finish what is in flight and start nothing else"
          >
            STOP
          </button>
        ) : (
          <button
            type="button"
            className="epbtn epbtn--primary dlg__send"
            disabled={
              !model ||
              (draft.trim().length === 0 &&
                attachments.length === 0 &&
                images.length === 0)
            }
            onClick={send}
          >
            SAY
          </button>
        )}
      </div>
    </>
  );
}
