/**
 * The part of a dialogue that reports what has actually happened.
 *
 * A Chronicle contains speech, approvals, produced evidence, work in progress and streamed
 * words. They are deliberately one presentation unit: splitting approvals or work into a
 * different visual record would hide part of the Quest's history (ADR-0025). It owns no turn
 * state and makes no decision about what a line means; all of that arrives from the Engine's
 * projection through `useTurn`.
 */

import { memo, useRef, useState } from "react";

import { PixelIcon } from "./Pixel";
import { ChronicleInline, RichChronicle } from "./RichChronicle";
import type { CharacterView } from "../../ipc/contracts";
import { openPicture, pictureSrc } from "../../ipc/world";
import type { Said, SharedImage } from "../../ipc/world";
import type { Working } from "../../experience/useTurn";

interface ChronicleListProps {
  readonly who: CharacterView;
  /**
   * The name of whoever is answering right now, when it is not the character selected.
   *
   * `null` means it is them, or nobody is. Watched before this existed: a message sent to Mage
   * streamed under PALADIN because the crew card had been clicked in between — and the finished
   * answer then landed under MAGE, correctly. A caret under the wrong name is a character
   * appearing to say something they never said, which is the one thing a Chronicle may not do.
   */
  readonly speaking?: string | null;
  readonly model: string | null;
  readonly crew: readonly CharacterView[];
  readonly said: readonly Said[];
  readonly writing: string;
  readonly thinking: boolean;
  readonly compacting: number | null;
  readonly working: readonly Working[];
  /** Opens an http(s) address through the Engine; it never trusts the WebView to do so. */
  readonly onOpenLink: (url: string) => void;
}

export function ChronicleList({
  who,
  model,
  crew,
  said,
  writing,
  speaking,
  thinking,
  compacting,
  working,
  onOpenLink,
}: ChronicleListProps) {
  return (
    <>
      {said.length === 0 && !writing && !thinking && !compacting && (
        <p className="dlg__empty">
          {model
            ? `Say what you want done. ${who.name} will turn it into a Quest the crew can work on. It is kept with this World.`
            : `${who.name} has no brain assigned, so there is nobody to answer. Pick a model or an agent in Characters.`}
        </p>
      )}

      <Settled who={who} crew={crew} said={said} onOpenLink={onOpenLink} />

      {/* This is a real maintenance turn, but it cannot stream prose without misrepresenting a private brief. */}
      {compacting !== null && (
        <p className="dlg__mark dlg__mark--compacting" role="status">
          <PixelIcon glyph="core" size={10} tone="energy" />
          <span>
            Compacting context: preserving the Chronicle and reducing{" "}
            {compacting} earlier {compacting === 1 ? "record" : "records"}...
          </span>
        </p>
      )}

      {/* What they are writing right now, as it arrives. */}
      {(writing || (thinking && compacting === null)) && (
        <div className="dlg__line dlg__line--assistant">
          <b>{(speaking ?? who.name).toUpperCase()}</b>
          <RichChronicle content={writing} onOpenLink={onOpenLink} />
          <span className="dlg__caret ep-blink" aria-hidden>
            ▌
          </span>
        </div>
      )}

      {/* Work is distinct from talk, and it has no result until the Engine reports one. */}
      {working.map((job, i) => (
        <p
          key={`${job.capability}-${i}`}
          className={`dlg__work${job.running ? " dlg__work--live" : ""}${
            job.ok === false ? " dlg__work--failed" : ""
          }`}
        >
          <PixelIcon
            glyph="core"
            size={10}
            tone={job.ok === false ? "dim" : "energy"}
          />
          <span>{job.what}</span>
          {!job.running && job.detail && <em>{job.detail}</em>}
        </p>
      ))}
    </>
  );
}

/**
 * **What has already been said, and cannot change while somebody is talking.**
 *
 * Split out and memoised, and the number is the reason: with 5,000 lines standing, one streamed
 * token cost **138 ms** of reconciling 20,000 nodes for a prop none of them read
 * (`ChronicleList.cost.test.tsx`). A local model produces about ten tokens a second, so the
 * window fell a second behind for every second of talking and never caught up.
 *
 * The roadmap's plan for this was a virtualised list. Virtualising takes ownership of scrolling,
 * breaks find-in-page and makes *"follow the newest line"* — which this dialogue does on every
 * token — something to implement rather than something the browser already does. It was aimed at
 * the wrong number: the list is not expensive to **have**, it was expensive to **keep**.
 *
 * The lines are still all drawn. At five thousand of them that is 20,000 nodes, which a browser
 * scrolls; if a Chronicle ever gets long enough that having them is the problem rather than
 * keeping them, windowing is still available and the measurement is here to say so.
 */
const Settled = memo(function Settled({
  who,
  crew,
  said,
  onOpenLink,
}: {
  readonly who: CharacterView;
  readonly crew: readonly CharacterView[];
  readonly said: readonly Said[];
  readonly onOpenLink: (url: string) => void;
}) {
  void who;
  return (
    <>
      {/* Every line names the person who actually said it; a colleague never speaks as `who`. */}
      {said.map((line, i) => {
        // Approval and evidence are Chronicle entries, not words in a character's mouth.
        if (line.kind === "compacted") {
          const covered = line.compaction?.covered;
          return (
            <p key={i} className="dlg__mark dlg__mark--compacted">
              <PixelIcon glyph="core" size={10} tone="gold" />
              <span>
                <b>CONTEXT COMPACTED</b>
                {covered === undefined ? (
                  <ChronicleInline
                    text={line.content}
                    onOpenLink={onOpenLink}
                  />
                ) : (
                  <>
                    Coverage: the first {covered}{" "}
                    {covered === 1
                      ? "record now travels"
                      : "records now travel"}{" "}
                    as a continuity brief in the next fresh agent session. The
                    Chronicle remains intact.
                  </>
                )}
              </span>
            </p>
          );
        }
        if (line.kind === "approved" || line.kind === "produced") {
          return (
            <div key={i}>
              <p className={`dlg__mark dlg__mark--${line.kind}`}>
                <PixelIcon
                  glyph={line.kind === "produced" ? "gem" : "key"}
                  size={10}
                  tone="gold"
                />
                <span>
                  <ChronicleInline
                    text={line.content}
                    onOpenLink={onOpenLink}
                  />
                </span>
              </p>
              {/*
                **A picture the crew made is drawn here.** The same argument that put shared
                pictures in the transcript, from the other side: a mark reading *"a lighthouse
                at dawn"* with no picture under it is a conversation about something the reader
                cannot see — and the person asked for a picture, not for a receipt.

                Same component, same resolution, same folder as one the user pasted. To a
                conversation they are the same kind of thing.
              */}
              {line.images.length > 0 && (
                <div className="dlg__chronicle-images" aria-label="Made here">
                  {line.images.map((image, at) => (
                    <SharedPicture key={`${image.file}-${at}`} image={image} />
                  ))}
                </div>
              )}
            </div>
          );
        }

        const speaker = line.who
          ? crew.find((person) => person.id === line.who)
          : null;
        return (
          <div
            key={i}
            className={`dlg__line dlg__line--${line.who ? "assistant" : "user"}`}
          >
            <b>
              {line.who ? (speaker?.name ?? line.who).toUpperCase() : "YOU"}
            </b>
            {line.attachments.length > 0 && (
              <div
                className="dlg__chronicle-attachments"
                aria-label="User-provided references"
              >
                {line.attachments.map((attachment) => (
                  <span key={attachment.name}>
                    attached {attachment.name} ({attachment.bytes} B)
                  </span>
                ))}
              </div>
            )}
            {/*
              The picture itself, in the conversation.

              A name and a byte count is the right projection for a text attachment — reprinting
              a document into a chat every time it is opened is noise. An image is the opposite:
              it *is* what was shared, and a line reading "attached screenshot.png" is a
              conversation about something the reader cannot see.

              The Engine resolved these to `data:` URIs. A file that has gone resolves to null
              and is said rather than drawn as a broken image, which explains nothing.
            */}
            {line.images.length > 0 && (
              <div className="dlg__chronicle-images" aria-label="Shared images">
                {line.images.map((image, at) => (
                  <SharedPicture key={`${image.file}-${at}`} image={image} />
                ))}
              </div>
            )}
            {!line.who &&
              (line.attachments.length > 0 || line.images.length > 0) &&
              !line.content.trim() && (
                <p className="dlg__attachment-only">
                  {line.images.length > 0 && line.attachments.length === 0
                    ? "Shared an image without a written request."
                    : "Shared reference material without a written request."}
                </p>
              )}
            <RichChronicle content={line.content} onOpenLink={onOpenLink} />
            {/*
              **How fast that was, under the words it describes.**

              Epoch measured tokens per second in five places and none of them was a
              conversation, so *how fast was that reply?* was answered by opening a terminal.
              This is the backend's own figure for its own decode — llama.cpp's
              `timings.predicted_per_second`, Ollama's `eval_count` over `eval_duration` — never
              a stopwatch around the turn, which would be mostly the model loading and the prompt
              being read.

              Absent rather than zero where nobody measured: an agent runs its own loop, and a
              hosted model reports nothing. A dash there would be a reading of a question that
              was never asked.
            */}
            {line.who && typeof line.pace === "number" && line.pace > 0 && (
              <p className="dlg__pace">{line.pace.toFixed(1)} tok/s</p>
            )}
          </div>
        );
      })}
    </>
  );
});

/**
 * Whether this is something that plays rather than something that is looked at.
 *
 * **By extension, and the extension was written by the Engine's own sniffer.** Every file in
 * this folder is named `<hash of its bytes>.<what those bytes are>` — so this reads back a
 * measurement rather than trusting a name, which is the distinction ADR-0024 draws. The Rust
 * side answers the same question the same way (`is_a_picture`), and being wrong here costs a
 * `<video>` that will not play rather than anything reaching the disk.
 */
function moves(file: string): boolean {
  const lower = file.toLowerCase();
  return lower.endsWith(".mp4") || lower.endsWith(".webm");
}

/**
 * Sound, with a transport drawn in the World's own hand.
 *
 * ## Why not the browser's
 *
 * It was the browser's, and the argument for it was the one the file dialog gets: the platform
 * ships a player that already knows about buffering and seeking, and Epoch draws what is
 * Epoch's. Seen in the window, that argument does not survive — a white Chromium control strip
 * sitting in a pixel-art conversation is the scrollbar all over again, and the scrollbar was the
 * first thing the owner pointed at.
 *
 * **But a transport is not a scrollbar.** A scrollbar could be deleted because the content
 * itself says there is more; there is no equivalent for sound. It is the only way to start it,
 * so it is drawn rather than removed — and drawn from the World's own tokens, so a World Pack
 * restyles it with everything else.
 *
 * ## What it deliberately does not have
 *
 * No volume, and no time remaining. The machine has a volume control and the conversation says
 * how long the sound is; a strip carrying every control the platform's does would be the imitation
 * this is trying not to be. Play, a line that fills, and a place to press.
 */
function SoundBar({
  src,
  onGone,
}: {
  readonly src: string;
  readonly onGone: () => void;
}) {
  const sound = useRef<HTMLAudioElement | null>(null);
  const [playing, setPlaying] = useState(false);
  const [at, setAt] = useState(0);
  const [length, setLength] = useState(0);

  const seek = (event: React.MouseEvent<HTMLDivElement>) => {
    const bar = event.currentTarget.getBoundingClientRect();
    const player = sound.current;
    if (!player || !Number.isFinite(length) || length <= 0) return;
    player.currentTime = ((event.clientX - bar.left) / bar.width) * length;
  };

  return (
    <div
      className="dlg__sound"
      // The row is inside the button that opens the file in the machine's own player. Pressing
      // play must not also do that — the frame around this is still the way, from its edges.
      onClick={(event) => event.stopPropagation()}
      role="presentation"
    >
      <audio
        ref={sound}
        src={src}
        preload="metadata"
        onLoadedMetadata={(e) => setLength(e.currentTarget.duration)}
        onTimeUpdate={(e) => setAt(e.currentTarget.currentTime)}
        onPlay={() => setPlaying(true)}
        onPause={() => setPlaying(false)}
        onEnded={() => setPlaying(false)}
        onError={onGone}
      />
      <button
        type="button"
        className="dlg__sound-go"
        aria-label={playing ? "pause" : "play"}
        onClick={() => {
          const player = sound.current;
          if (!player) return;
          // **It does not play on its own.** A conversation that starts making noise when you
          // scroll past it is the World interrupting you, which a muted video is not.
          if (player.paused) void player.play();
          else player.pause();
        }}
      >
        {playing ? "❚❚" : "▶"}
      </button>
      <div className="dlg__sound-line" onClick={seek} role="presentation">
        <i
          className="dlg__sound-far"
          style={{
            width: length > 0 ? `${Math.min(100, (at / length) * 100)}%` : "0%",
          }}
        />
      </div>
      <span className="dlg__sound-len">{clock(at)} / {clock(length)}</span>
    </div>
  );
}

/** Seconds as `m:ss`. `0:00` while the length is still unknown, never `NaN`. */
function clock(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  const whole = Math.floor(seconds);
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, "0")}`;
}

/**
 * Whether this is something that is listened to rather than looked at.
 *
 * Read back from the extension the Engine's own sniffer wrote, exactly as `moves` is. FLAC is
 * what this ComfyUI writes and every Chromium plays; the other three are what a sound could
 * arrive as from a workflow somebody imported.
 */
/**
 * The turn Epoch rendered of a mesh, if this is one.
 *
 * `<stem>.gif` beside `<stem>.glb`, where the stem is the hash of the mesh's own bytes — the
 * preview is *of* that mesh and of nothing else, so sharing the stem is a statement rather than
 * a coincidence.
 */
function turns(file: string): string | null {
  const lower = file.toLowerCase();
  return lower.endsWith(".glb") ? file.slice(0, -4) + ".gif" : null;
}

function sounds(file: string): boolean {
  const lower = file.toLowerCase();
  return [".flac", ".mp3", ".ogg", ".wav"].some((it) => lower.endsWith(it));
}

/**
 * One picture that was shared, fetched by reference.
 *
 * The Chronicle carries the vault's file name, not the bytes — see `sharedImage`. Until they
 * arrive nothing is drawn rather than a spinner: the wait is a disk read of a file this
 * machine already has, and a flash of loading furniture over a conversation is an immersion
 * leak for no information.
 */
function SharedPicture({ image }: { readonly image: SharedImage }) {
  const [gone, setGone] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  // **Asked for by name over Epoch's own scheme, not carried here as a string.** The browser
  // fetches it, caches it by that name — which is the hash of its own bytes, so it never means
  // two pictures — and decodes it off the main thread. Nothing about ADR-0024 changes: this is a
  // name, the Engine decides what it resolves to, and the page still cannot reach a path.
  // **A mesh is shown as its turn.** Nothing in a webview draws a `.glb`, and the Engine leaves
  // an animated GIF beside it named after the same bytes (`state/turntable.rs`) — so this is a
  // rule about names rather than a second field on the artifact.
  //
  // The button still opens the **mesh**: what was made is the model, and the turn is a picture of
  // it. If the preview never landed, `onError` says the file is gone and the row still says a
  // model was made.
  const src = pictureSrc(turns(image.file) ?? image.file);

  // A file can leave the vault between the Chronicle being written and being read. With the
  // bytes fetched here that was a `null`; over a URL it is a failed request, and the sentence
  // has to come from the same place a broken-image icon would have.
  if (gone) {
    // **A mesh with no preview is not a mesh that is gone.** The turn is a *picture of* the
    // model; the model itself is right there, and saying it vanished sends somebody looking for
    // a file that never moved. Measured in the window: the second model of a session rendered
    // no preview, and the row read `bcb184e6fcad6168.glb is no longer in the vault`.
    if (turns(image.file)) {
      return (
        <button
          type="button"
          className="dlg__shared-open dlg__shared-open--plain"
          aria-label={`open ${image.name}`}
          title={said ?? `open ${image.name}`}
          onClick={() => {
            void openPicture(image.file).then(setSaid);
          }}
        >
          <span className="dlg__shared-image--gone">
            A model, with no turn to show of it. Open it to look.
          </span>
          {said && <i className="dlg__shared-said">{said}</i>}
        </button>
      );
    }
    return (
      <span className="dlg__shared-image--gone">
        {image.name} is no longer in the vault
      </span>
    );
  }
  return (
    <button
      type="button"
      className="dlg__shared-open"
      // **Opened by the machine, not enlarged in the conversation.** A viewer inside the World
      // would be a second place a picture can be, with its own zoom, its own close, and its own
      // rules about what fits — and the person already has an application that does all of that
      // and remembers where the file is.
      // The name is on the button, not on the image inside it: a control's name should say what
      // pressing it does, and `screenshot.png` says only what it is.
      aria-label={`open ${image.name}`}
      title={said ?? `open ${image.name}`}
      onClick={() => {
        void openPicture(image.file).then(setSaid);
      }}
    >
      {sounds(image.file) ? (
        <SoundBar src={src} onGone={() => setGone(true)} />
      ) : moves(image.file) ? (
        /*
          **A video plays where a picture would be shown**, in the same button, opened by the
          same click.

          `muted` and `loop` because this sits inside a conversation: a thing that starts
          talking when you scroll past it is the World interrupting you, and a two-second
          render that plays once is a still frame you missed. `playsInline` so it never takes
          the screen for itself. No `controls`: the frame around it is a button, and a control
          strip inside a button is two things fighting for one click — the person who wants to
          scrub it opens it in their own player, which is what pressing this does.
        */
        <video
          className="dlg__shared-image"
          src={src}
          autoPlay
          muted
          loop
          playsInline
          onError={() => setGone(true)}
        />
      ) : (
        <img
          className="dlg__shared-image"
          src={src}
          alt=""
          onError={() => setGone(true)}
        />
      )}
      {/*
        A failure to open is said where the click was, never swallowed: a picture that does
        nothing when clicked is a bug the user cannot report.
      */}
      {said && <i className="dlg__shared-said">{said}</i>}
    </button>
  );
}
