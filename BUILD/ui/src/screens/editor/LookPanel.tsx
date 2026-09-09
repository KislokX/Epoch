/**
 * Give this World its own windows.
 *
 * ## The last thing in a World Pack that could only be typed
 *
 * The `[[ui]]` pipeline has worked since ADR-0016 and the default pack uses it. What did not
 * exist was any way to reach it: somebody with a drawing of a window had to open `pack.toml`,
 * work out the nine-slice syntax and edit it by hand — which is the program this product exists
 * to avoid opening, one file over.
 *
 * ## The preview is the feature, not decoration
 *
 * An image is not enough. A nine-slice needs the **corner inset**, and it cannot be read from
 * the pixels: the default pack's `[11, 5, 12, 6]` was measured off the artwork by the person who
 * drew it, because a bevel lit from above is not symmetric. So the author has to say — and
 * *asking somebody for numbers they have no way to choose between is how a panel stops being a
 * panel* (the Studio Panel's rule, one subsystem over).
 *
 * Four numbers nobody can pick in the abstract are obvious against a picture. So the frame is
 * drawn at two sizes while they move the sliders, and it is drawn by **`Frame` itself** rather
 * than by a preview renderer of its own: a second implementation of the nine-slice would
 * eventually disagree with the real window, and being trustworthy is the entire value of this.
 */

import { useEffect, useState } from "react";

import { Frame } from "../../components/hud/Frame";
import { ImageDrop } from "../../components/ImageDrop";
import type { Skin } from "../../experience/useSkin";
import { Cold, Field, Panel } from "./Panel";
import { playSfx } from "../../experience/sfx";
import type { SfxName } from "../../experience/sfx";

/**
 * The eight voices a World may replace.
 *
 * Named here because a *list of what exists* is presentation, and `sfx.ts` is where the voices
 * are defined. It is not a catalogue of what is installed — that would be the closed-set mistake
 * ADR-0030 had to undo. These eight are the sounds Epoch actually makes, and a World may take
 * over any of them.
 */
const VOICES: readonly { readonly id: SfxName; readonly when: string }[] = [
  { id: "hover", when: "moving over a button" },
  { id: "click", when: "pressing one" },
  { id: "open", when: "a window opening" },
  { id: "close", when: "a window closing" },
  { id: "select", when: "choosing something" },
  { id: "quest", when: "a Quest beginning" },
  { id: "error", when: "something refused" },
  { id: "type", when: "each keystroke" },
];

/** What a window's artwork may be told to do with its edges when it is stretched. */
const REPEATS = ["stretch", "repeat", "round", "space"] as const;

type Repeat = (typeof REPEATS)[number];

const SIDES = ["top", "right", "bottom", "left"] as const;

export interface LookPanelProps {
  /** The World's current window skin, when it has one. */
  readonly current: Skin | undefined;
  /** Save this artwork and these numbers. Resolves to an error message, or null. */
  readonly onSave: (
    image: string,
    corner: readonly [number, number, number, number],
    repeat: Repeat,
    scale: number,
  ) => Promise<string | null>;
  /** Go back to the window Epoch draws. Resolves to an error message, or null. */
  readonly onClear: () => Promise<string | null>;
  /** Which voices this World supplies, by concept. */
  readonly sounds: Readonly<Record<string, string>>;
  /** Give one voice a sound, or take it back to the synthesised one. */
  readonly onSound: (concept: string, audio: string | null) => Promise<string | null>;
}

export function LookPanel({ current, onSave, onClear, sounds, onSound }: LookPanelProps) {
  /** The artwork being considered — imported here, not saved until SAVE. */
  const [image, setImage] = useState<string | null>(null);
  const [corner, setCorner] = useState<[number, number, number, number]>([12, 12, 12, 12]);
  const [repeat, setRepeat] = useState<Repeat>("stretch");
  const [scale, setScale] = useState(1);
  const [size, setSize] = useState<[number, number] | null>(null);
  const [why, setWhy] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Open on what the World already has, so editing an existing window starts from its own
  // numbers rather than from a guess that undoes them.
  useEffect(() => {
    if (!current) return;
    setCorner([...current.corner] as [number, number, number, number]);
    setRepeat(current.repeat);
    setScale(current.scale);
  }, [current]);

  /*
    **The artwork's real size, measured rather than asked for.**

    An inset is a number of the image's own pixels, so *how many there are* is the one fact that
    makes the four sliders meaningful — 12 on a 48px window is a quarter of it and 12 on a 512px
    one is nothing. The browser already knows; nobody should be made to look it up.
  */
  const showing = image ?? current?.image ?? null;
  useEffect(() => {
    if (!showing) {
      setSize(null);
      return;
    }
    const img = new Image();
    img.onload = () => setSize([img.naturalWidth, img.naturalHeight]);
    img.src = showing;
  }, [showing]);

  const candidate: Skin | undefined = showing
    ? {
        image: showing,
        corner,
        repeat,
        scale,
        // The Engine derives a real floor from the artwork. This is a preview at sizes chosen
        // right here, so it imposes none: reimplementing that rule in TypeScript is how the two
        // would come to disagree, and the preview is not what the floor protects.
        minSize: [0, 0],
      }
    : undefined;

  const save = async () => {
    if (!image) return;
    setBusy(true);
    setWhy(await onSave(image, corner, repeat, scale));
    setBusy(false);
    setImage(null);
  };

  return (
    <Panel title="This World's Windows">
      <div className="pxgrid pxgrid--2" style={{ margin: 0 }}>
        <ImageDrop
          label={showing ? "Replace the window" : "Draw the window"}
          className="pxbtn"
          busy={busy}
          onChoose={async (data) => {
            setImage(data);
            setWhy(null);
            return null;
          }}
          onClear={
            current
              ? async () => {
                  setBusy(true);
                  const problem = await onClear();
                  setBusy(false);
                  setImage(null);
                  setWhy(problem);
                  return problem;
                }
              : undefined
          }
          why={why}
        />
      </div>

      {!showing && (
        <Cold>
          With none, Epoch draws its own windows — which is a complete World, not an unfinished
          one. Every World today is in exactly this state.
        </Cold>
      )}

      {showing && (
        <>
          <div className="look__stage" aria-label="How the window will look">
            {/* Two sizes, because a nine-slice is only wrong at one of them. A corner that
                looks right on a small box can eat the face of a wide one. */}
            <Frame className="look__demo look__demo--wide" skin={candidate} corner={12}>
              <p className="look__words">A wide window</p>
            </Frame>
            <Frame className="look__demo look__demo--tall" skin={candidate} corner={12}>
              <p className="look__words">And a small one</p>
            </Frame>
          </div>

          <Cold>
            {size
              ? `Your artwork is ${size[0]}×${size[1]} pixels. The insets below are counted in those.`
              : "Reading the artwork…"}
          </Cold>

          {SIDES.map((side, at) => (
            <Field key={side} label={`${side} inset`}>
              <input
                className="pxinput"
                type="number"
                min={0}
                max={size ? Math.max(size[0], size[1]) : 512}
                value={corner[at]}
                onChange={(e) => {
                  const next = [...corner] as [number, number, number, number];
                  next[at] = Math.max(0, Number(e.target.value) || 0);
                  setCorner(next);
                }}
              />
            </Field>
          ))}

          <Field label="scale">
            {/* Whole numbers only, and the Engine refuses anything below one rather than
                clamping it. Epoch is pixel art; a fractional scale destroys it. */}
            <input
              className="pxinput"
              type="number"
              min={1}
              max={8}
              step={1}
              value={scale}
              onChange={(e) => setScale(Math.max(1, Math.round(Number(e.target.value) || 1)))}
            />
          </Field>

          <Field label="edges">
            <select
              className="pxinput"
              value={repeat}
              onChange={(e) => setRepeat(e.target.value as Repeat)}
            >
              {REPEATS.map((r) => (
                <option key={r} value={r}>
                  {r}
                </option>
              ))}
            </select>
          </Field>

          <Cold>
            <b>stretch</b> pulls the edges to fit, which is right for a soft gradient — repeating
            one bands. A tiled pattern wants <b>repeat</b>.
          </Cold>

          {image && (
            <div className="pxgrid pxgrid--2" style={{ margin: 0 }}>
              <button type="button" className="pxbtn pxbtn--gold" disabled={busy} onClick={save}>
                {busy ? "SAVING…" : "USE THIS WINDOW"}
              </button>
              <button
                type="button"
                className="pxbtn"
                disabled={busy}
                onClick={() => {
                  setImage(null);
                  setWhy(null);
                }}
              >
                CANCEL
              </button>
            </div>
          )}
        </>
      )}

      <Cold>
        A pack owns the <b>skin</b> and never the layout, the behaviour, or whether text can be
        read (ADR-0016). Sky and weather are still authored by hand.
      </Cold>

      {/*
        **The World's own voice, in the same panel.** How a World looks and how it sounds are one
        question about one pack, and a rail of nine tools starts being a menu.
      */}
      <h4 className="look__heading">This World&apos;s Sounds</h4>
      <Cold>
        Epoch synthesises these eight and ships no files, so a World that supplies one is
        <b> replacing</b> a sound that already worked — never filling an empty slot.
      </Cold>

      {VOICES.map((voice) => (
        <Field key={voice.id} label={`${voice.id} · ${voice.when}`}>
          <span className="look__voice">
            <button
              type="button"
              className="pxbtn"
              title="Hear it"
              onClick={() => playSfx(voice.id)}
            >
              ▶
            </button>
            <ImageDrop
              label={sounds[`sfx.${voice.id}`] ? "Replace" : "Record"}
              className="pxbtn"
              accept="audio/*"
              onChoose={(data) => onSound(`sfx.${voice.id}`, data)}
              onClear={
                sounds[`sfx.${voice.id}`]
                  ? () => onSound(`sfx.${voice.id}`, null)
                  : undefined
              }
            />
          </span>
        </Field>
      ))}
    </Panel>
  );
}
