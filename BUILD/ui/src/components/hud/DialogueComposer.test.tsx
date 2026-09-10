import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { invoke } from "@tauri-apps/api/core";

import { converse } from "../../experience/converse";
import { listen } from "../../experience/hearing";
import { hush } from "../../experience/speak";

import { DialogueComposer, aboutTheLanguage } from "./DialogueComposer";

vi.mock("../../experience/converse", () => ({ converse: vi.fn() }));
vi.mock("../../experience/speak", () => ({ hush: vi.fn() }));
vi.mock("../../experience/hearing", async (real) => ({
  ...(await real<Record<string, unknown>>()),
  listen: vi.fn(),
}));

function draw(
  props: Partial<React.ComponentProps<typeof DialogueComposer>> = {},
) {
  const onSpeak = vi.fn();
  const view = render(
    <DialogueComposer
      characterId="mage"
      conversationId="quest-mage"
      name="Mage"
      model="gpt-5.5"
      picturesReachThem={false}
      thinking={false}
      logHeight={480}
      history={[]}
      onSpeak={onSpeak}
      onHalt={() => {}}
      {...props}
    />,
  );
  return { onSpeak, ...view };
}

describe("the dialogue composer", () => {
  it("Enter sends the draft and clears only the draft", async () => {
    const user = userEvent.setup();
    const { onSpeak } = draw();
    const input = screen.getByRole("textbox");

    await user.type(input, "Please inspect this.");
    await user.keyboard("{Enter}");

    expect(onSpeak).toHaveBeenCalledWith("Please inspect this.", [], []);
    expect(input).toHaveValue("");
  });

  it("Shift Enter stays in the draft instead of speaking", async () => {
    const user = userEvent.setup();
    const { onSpeak } = draw();
    const input = screen.getByRole("textbox");

    await user.type(input, "first");
    await user.keyboard("{Shift>}{Enter}{/Shift}second");

    expect(onSpeak).not.toHaveBeenCalled();
    expect(input).toHaveValue("first\nsecond");
  });

  it("sends an explicitly chosen text file as context beside the user's words", async () => {
    const user = userEvent.setup();
    const { onSpeak } = draw();
    const file = new File(["Project facts"], "brief.md", {
      type: "text/markdown",
    });
    Object.defineProperty(file, "text", { value: async () => "Project facts" });

    await user.upload(screen.getByLabelText("Attach text files"), file);
    await user.type(screen.getByRole("textbox"), "Review this reference.");
    await user.click(screen.getByRole("button", { name: "SAY" }));

    expect(onSpeak).toHaveBeenCalledWith(
      "Review this reference.",
      [{ name: "brief.md", content: "Project facts", bytes: file.size }],
      [],
    );
    expect(screen.getByRole("textbox")).toHaveValue("");
  });

  it("sends a chosen text file without inventing a written request", async () => {
    const user = userEvent.setup();
    const { onSpeak } = draw();
    const file = new File(["Project facts"], "brief.md", {
      type: "text/markdown",
    });
    Object.defineProperty(file, "text", { value: async () => "Project facts" });

    await user.upload(screen.getByLabelText("Attach text files"), file);
    expect(screen.getByRole("button", { name: "SAY" })).toBeEnabled();
    await user.click(screen.getByRole("button", { name: "SAY" }));

    expect(onSpeak).toHaveBeenCalledWith(
      "",
      [{ name: "brief.md", content: "Project facts", bytes: file.size }],
      [],
    );
  });

  it("sends an image as bytes, and says what will happen to it", async () => {
    // Two halves of one honesty. The picture travels — that is the whole step — and what
    // reaches the character depends on their brain, so somebody attaching a screenshot to ask
    // "what is wrong here?" learns which one it is *before* spending a turn on it.
    const user = userEvent.setup();
    const { onSpeak } = draw();
    const file = new File([new Uint8Array([1, 2, 3])], "screen.png", {
      type: "image/png",
    });

    await user.upload(screen.getByLabelText("Attach text files"), file);

    expect(await screen.findByText("screen.png")).toBeVisible();
    expect(screen.getByText(/only told a picture was shared/)).toBeVisible();

    await user.click(screen.getByRole("button", { name: "SAY" }));

    expect(onSpeak).toHaveBeenCalledWith(
      "",
      [],
      [expect.objectContaining({ name: "screen.png", bytes: 3 })],
    );
  });

  it("does not claim nobody can see a picture an agent is handed", async () => {
    // The defect this replaced: the note said "nobody can see it yet" while the character on
    // an agent described the screenshot correctly in the next breath. A line that contradicts
    // the turn the user is watching teaches them to stop reading the notes.
    const user = userEvent.setup();
    draw({ picturesReachThem: true });
    const file = new File([new Uint8Array([1, 2, 3])], "screen.png", {
      type: "image/png",
    });

    await user.upload(screen.getByLabelText("Attach text files"), file);

    expect(await screen.findByText(/is handed the picture itself/)).toBeVisible();
    expect(screen.queryByText(/only told a picture was shared/)).toBeNull();
  });

  it("refuses an image format the asset pipeline cannot draw", async () => {
    // GIF, not JPEG: the accepted list is whatever `asset.rs` can deliver, and this test's job
    // is to prove the door is closed to everything else — so it must name a format that is
    // still outside it, or it passes forever without checking anything.
    const user = userEvent.setup();
    draw();
    const file = new File([new Uint8Array([1])], "loop.gif", {
      type: "image/gif",
    });
    // Never read: the format is refused before anything asks for its text. This held a
    // literal NUL byte, which compiled, passed, and turned the whole file binary to git
    // and to every grep -- the same defect this codebase already fixed once in
    // `speak.ts`. Nothing asserts on the bytes of a string only compared with itself.
    Object.defineProperty(file, "text", { value: async () => "" });

    await user.upload(screen.getByLabelText("Attach text files"), file);

    expect(screen.queryByText("loop.gif")).not.toBeInTheDocument();
  });

  it("takes a JPEG, which is a format Epoch can draw", async () => {
    const user = userEvent.setup();
    draw();
    const file = new File([new Uint8Array([0xff, 0xd8, 0xff])], "photo.jpg", {
      type: "image/jpeg",
    });

    await user.upload(screen.getByLabelText("Attach text files"), file);

    expect(await screen.findByText("photo.jpg")).toBeInTheDocument();
  });

  it("accepts a text file dropped over the conversation, not only its attach button", async () => {
    const file = new File(["Project facts"], "brief.md", {
      type: "text/markdown",
    });
    Object.defineProperty(file, "text", { value: async () => "Project facts" });
    draw();
    screen
      .getByRole("textbox")
      .parentElement?.parentElement?.classList.add("dlg");

    fireEvent.drop(screen.getByRole("textbox"), {
      dataTransfer: { types: ["Files"], files: [file] },
    });

    await waitFor(() => expect(screen.getByText("brief.md")).toBeVisible());
    expect(screen.getByRole("button", { name: "SAY" })).toBeEnabled();
  });

  it("keeps an attachment visible when the live desktop engine did not acknowledge it", async () => {
    const user = userEvent.setup();
    const onSpeak = vi.fn().mockResolvedValue(false);
    draw({ onSpeak });
    const file = new File(["Project facts"], "brief.md", {
      type: "text/markdown",
    });
    Object.defineProperty(file, "text", { value: async () => "Project facts" });

    await user.upload(screen.getByLabelText("Attach text files"), file);
    await user.type(screen.getByRole("textbox"), "Review this reference.");
    await user.click(screen.getByRole("button", { name: "SAY" }));

    expect(screen.getByText(/did not confirm the attachment/i)).toBeVisible();
    expect(screen.getByText("brief.md")).toBeVisible();
    expect(screen.getByRole("textbox")).toHaveValue("Review this reference.");
  });

  /** What the Engine answers `get_offered` with, for this character, right now. */
  function offering(answer: unknown) {
    vi.mocked(invoke).mockImplementation((command: string) =>
      command === "get_offered"
        ? Promise.resolve(answer)
        : new Promise<never>(() => {}),
    );
  }

  it("offers what this character can actually reach, read from the Engine", async () => {
    // Not a list this component keeps. `/playwright` has to appear because a server is
    // connected and Trust allows it — which is what makes it disappear again when it does not.
    const user = userEvent.setup();
    const onCompact = vi.fn();
    offering({
      tools: [{ id: "mcp:playwright", summary: "Drive a browser." }],
      withheld: ["write_file"],
      gate: "What this character asked for, filtered by the mode.",
      crew: [{ id: "paladin", name: "Paladin", role: "Builds it" }],
    });
    draw({ mode: "Manual", reasoning: "Medium", onCompact });

    await user.type(screen.getByRole("textbox"), "/");

    expect(await screen.findByText("/mcp:playwright")).toBeVisible();
    expect(screen.getByText("Drive a browser.")).toBeVisible();
    // Asked for and not available is shown rather than left as a gap: an absence cannot say
    // whether Epoch lacks the capability or this character was not given it.
    expect(screen.getByText("write_file")).toBeVisible();
    expect(screen.getByText("Mode: Manual · Reasoning: Medium")).toBeVisible();
    // The Engine's own sentence about what decides the list. It is not the same question for a
    // model and an agent — one is filtered by its request, the other is offered everything so
    // the user can be asked — and the menu used to show one list while describing neither.
    expect(
      screen.getByText(/What this character asked for, filtered by the mode/),
    ).toBeVisible();
  });

  it("typing after the slash narrows it, and Enter takes the highlighted entry", async () => {
    const user = userEvent.setup();
    offering({
      tools: [
        { id: "read_file", summary: "Read a file." },
        { id: "mcp:playwright", summary: "Drive a browser." },
      ],
      withheld: [],
      crew: [],
      gate: "What this character asked for, filtered by the mode.",
    });
    draw();

    await user.type(screen.getByRole("textbox"), "/play");

    expect(await screen.findByText("/mcp:playwright")).toBeVisible();
    expect(screen.queryByText("/read_file")).not.toBeInTheDocument();

    await user.keyboard("{Enter}");

    // Typed into the draft, not sent. The sentence stays the user's.
    expect(screen.getByRole("textbox")).toHaveValue("mcp:playwright ");
  });

  it("opens on a slash anywhere in the sentence, not only at the start", async () => {
    // Reported: "utiliza /" offered nothing, which reads as the menu being broken. A user who
    // types a slash has asked for it, wherever the sentence had got to.
    const user = userEvent.setup();
    offering({
      tools: [{ id: "read_file", summary: "Read a file." }],
      withheld: [],
      crew: [],
      gate: "What this character asked for, filtered by the mode.",
    });
    draw();

    await user.type(screen.getByRole("textbox"), "utiliza /read");

    expect(await screen.findByText("/read_file")).toBeVisible();

    await user.keyboard("{Enter}");

    // Only the `/` word is replaced. Everything already written is the user's and stays.
    expect(screen.getByRole("textbox")).toHaveValue("utiliza read_file ");
  });

  it("arrows walk the menu instead of leaving the field", async () => {
    const user = userEvent.setup();
    offering({
      tools: [
        { id: "read_file", summary: "Read a file." },
        { id: "write_file", summary: "Write a file." },
      ],
      withheld: [],
      crew: [],
      gate: "What this character asked for, filtered by the mode.",
    });
    draw();

    await user.type(screen.getByRole("textbox"), "/");
    await screen.findByText("/read_file");
    await user.keyboard("{ArrowDown}{Enter}");

    expect(screen.getByRole("textbox")).toHaveValue("write_file ");
  });

  it("Enter on a slash draft never sends the half-typed query as a message", async () => {
    // The defect this prevents: `/comp` reaching the Chronicle as something the user said.
    const user = userEvent.setup();
    const onCompact = vi.fn();
    offering({ tools: [], withheld: [], crew: [] });
    const { onSpeak } = draw({ onCompact });

    await user.type(screen.getByRole("textbox"), "/comp");
    await user.keyboard("{Enter}");

    expect(onSpeak).not.toHaveBeenCalled();
    expect(onCompact).toHaveBeenCalledOnce();
  });

  it("Escape closes the menu and keeps what was typed", async () => {
    // A key that discards a draft is a key nobody presses a second time.
    const user = userEvent.setup();
    offering({ tools: [], withheld: [], crew: [] });
    draw();

    await user.type(screen.getByRole("textbox"), "/hola");
    await user.keyboard("{Escape}");

    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(screen.getByRole("textbox")).toHaveValue("hola");
  });

  it("moves the cursor to the person now being spoken to", () => {
    const { rerender } = draw();
    const input = screen.getByRole("textbox");
    input.blur();

    rerender(
      <DialogueComposer
        characterId="paladin"
        conversationId="quest-paladin"
        name="Paladin"
        model="gpt-5.5"
        picturesReachThem={false}
        thinking={false}
        logHeight={480}
        history={[]}
        onSpeak={() => {}}
        onHalt={() => {}}
      />,
    );

    expect(input).toHaveFocus();
  });

  it("keeps STOP in the command slot while a turn is already running", async () => {
    const user = userEvent.setup();
    const onHalt = vi.fn();
    const { onSpeak } = draw({ thinking: true, onHalt });

    await user.type(screen.getByRole("textbox"), "wait");
    await user.click(screen.getByRole("button", { name: "STOP" }));

    expect(onSpeak).not.toHaveBeenCalled();
    expect(onHalt).toHaveBeenCalledOnce();
    expect(screen.getByRole("textbox")).toHaveValue("wait");
  });

  it("recalls only this Quest's own messages, newest first, without storing another history", async () => {
    const user = userEvent.setup();
    draw({ history: ["first request", "second request"] });
    const input = screen.getByRole("textbox");

    await user.click(input);
    await user.keyboard("{ArrowUp}");
    expect(input).toHaveValue("second request");
    await user.keyboard("{ArrowUp}");
    expect(input).toHaveValue("first request");
    await user.keyboard("{ArrowDown}");
    expect(input).toHaveValue("second request");
    await user.keyboard("{ArrowDown}");
    expect(input).toHaveValue("");
  });

  it("does not replace a multiline draft when an arrow is editing it", async () => {
    const user = userEvent.setup();
    draw({ history: ["earlier request"] });
    const input = screen.getByRole("textbox");

    await user.type(input, "first{Shift>}{Enter}{/Shift}second");
    await user.keyboard("{ArrowUp}");

    expect(input).toHaveValue("first\nsecond");
  });

  it("forgets the navigation cursor when a different Quest is opened", async () => {
    const user = userEvent.setup();
    const { rerender } = draw({ history: ["Mage's request"] });
    const input = screen.getByRole("textbox");
    await user.click(input);
    await user.keyboard("{ArrowUp}");
    expect(input).toHaveValue("Mage's request");

    rerender(
      <DialogueComposer
        characterId="mage"
        conversationId="quest-other"
        name="Mage"
        model="gpt-5.5"
        picturesReachThem={false}
        thinking={false}
        logHeight={480}
        history={["Other Quest request"]}
        onSpeak={() => {}}
        onHalt={() => {}}
      />,
    );
    await user.keyboard("{ArrowUp}");
    expect(input).toHaveValue("Other Quest request");
  });
});

/**
 * Epoch asks for the microphone, in the World.
 *
 * The leak: `SPEAK` went straight to `getUserMedia` and Edge asked over the World —
 * *"http://tauri.localhost wants to use your microphones"*, a URL in somebody else's frame. It
 * was missed, and the report was *"cuando le doy a speak no pasa nada"* about a control that was
 * working perfectly.
 */
describe("asking for the microphone", () => {
  function answering(microphone: boolean | null, recorded: string[] = []) {
    vi.mocked(invoke).mockImplementation((command: string, args?: unknown) => {
      if (command === "get_settings") {
        return Promise.resolve({ concurrentCrew: false, microphone });
      }
      if (command === "set_microphone") {
        recorded.push(String((args as { allowed: boolean }).allowed));
        return Promise.resolve(null);
      }
      return new Promise<never>(() => {});
    });
  }

  it("asks in its own words before the browser is ever reached", async () => {
    const user = userEvent.setup();
    answering(null);
    draw();

    await user.click(screen.getByRole("button", { name: /SPEAK/ }));

    // Epoch's frame, saying what it is for — which a URL cannot.
    expect(
      await screen.findByRole("dialog", { name: /would like to listen/i }),
    ).toBeVisible();
    expect(screen.getByText(/on this machine/i)).toBeVisible();
    // And it has not started recording: the label has not moved.
    expect(screen.getByRole("button", { name: /SPEAK/ })).toBeVisible();
  });

  it("a refusal is written down, so the question does not come back every press", async () => {
    const user = userEvent.setup();
    const said: string[] = [];
    answering(null, said);
    draw();

    await user.click(screen.getByRole("button", { name: /SPEAK/ }));
    await user.click(await screen.findByRole("button", { name: "NOT NOW" }));

    await waitFor(() => expect(said).toEqual(["false"]));
    expect(screen.getByText(/Left off/)).toBeVisible();
  });

  it("does not ask again once it has been answered", async () => {
    const user = userEvent.setup();
    answering(false);
    draw();

    await user.click(screen.getByRole("button", { name: /SPEAK/ }));

    // A kept refusal says what it is and where to change it, rather than re-opening a question
    // somebody already answered.
    expect(await screen.findByText(/microphone is off for Epoch/)).toBeVisible();
    expect(screen.queryByRole("dialog", { name: /listen/i })).toBeNull();
  });
});

/**
 * Hands free (Phase 15, step 6).
 *
 * The roadmap named this step's cost as cancelling a turn in flight. Re-measured: `stop_turn`
 * was already built, so what these assert is the half that was actually missing — when a
 * sentence ends, and what happens the moment somebody starts talking over a character.
 */
describe("talking to the World without pressing anything", () => {
  function allowed() {
    vi.mocked(invoke).mockImplementation((command: string) =>
      command === "get_settings"
        ? Promise.resolve({ concurrentCrew: false, microphone: true })
        : new Promise<never>(() => {}),
    );
  }

  /** Hand back the listener the component registered, so a test can be the room. */
  function theRoom() {
    let heard: Parameters<typeof converse>[0] | null = null;
    const stop = vi.fn();
    vi.mocked(converse).mockImplementation(async (to) => {
      heard = to;
      return { stop };
    });
    return { room: () => heard!, stop };
  }

  it("being quiet is not the same as forgetting what was said", async () => {
    const user = userEvent.setup();
    allowed();
    const { room } = theRoom();
    const onHalt = vi.fn();
    draw({ thinking: true, onHalt });

    await user.click(screen.getByRole("button", { name: /HANDS FREE/ }));
    await waitFor(() => expect(room()).not.toBeNull());

    room().onStart();

    /*
      **`hush`, never `stopSpeaking`.** This called the second, which also wipes the memory of
      what has already been said — and the Chronicle is re-read on every render, so every answer
      on screen became something that had not been said yet and was said again. That was the
      loop the owner watched: one paragraph forever, with five entries on disk.

      Asserting on *which* function is not testing an implementation detail here. The two differ
      by exactly the thing that caused the defect.
    */
    expect(vi.mocked(hush)).toHaveBeenCalled();
    expect(onHalt).toHaveBeenCalled();
  });

  it("a finished sentence is sent, because that is what this mode is", async () => {
    const user = userEvent.setup();
    allowed();
    const { room } = theRoom();
    vi.mocked(listen).mockResolvedValue({ text: "¿Qué hay en el proyecto?", millis: 900 });
    const { onSpeak } = draw();

    await user.click(screen.getByRole("button", { name: /HANDS FREE/ }));
    await waitFor(() => expect(room()).not.toBeNull());
    room().onEnd("d2F2");

    // Step 5's rule — *it writes into the draft and never sends* — is not quietly broken here.
    // It is a mode somebody turns on whose entire purpose is that there is nothing to press,
    // and SPEAK still only writes.
    await waitFor(() =>
      expect(onSpeak).toHaveBeenCalledWith("¿Qué hay en el proyecto?", [], []),
    );
  });

  it("says it is holding off while a character talks, rather than going quiet", async () => {
    // The World listening to its own voice is what made the first hands-free session repeat one
    // paragraph forever: the microphone heard the speakers, the answer came back in as a new
    // question, and round it went. The ear holds off now — and **says so**, because a control
    // whose only feedback is a label must not have a state the label cannot show.
    const user = userEvent.setup();
    allowed();
    const { room } = theRoom();
    draw();

    await user.click(screen.getByRole("button", { name: /HANDS FREE/ }));
    await waitFor(() => expect(room()).not.toBeNull());
    room().onRoom(0.01);
    expect(await screen.findByRole("button", { name: /LISTENING/ })).toBeVisible();

    room().onTheirTurn(true);
    expect(await screen.findByRole("button", { name: /THEIR TURN/ })).toBeVisible();

    room().onTheirTurn(false);
    expect(await screen.findByRole("button", { name: /LISTENING/ })).toBeVisible();
  });

  it("sends what was said once, not once per render", async () => {
    // The send used to sit inside a `setDraft` updater, to read the current draft without
    // depending on it. An updater is a function React may call more than once — StrictMode calls
    // every one of them twice on purpose — so that was a turn sent twice, and it would only ever
    // have shown up on somebody else's machine.
    const user = userEvent.setup();
    allowed();
    const { room } = theRoom();
    vi.mocked(listen).mockResolvedValue({ text: "una vez", millis: 100 });
    const { onSpeak } = draw();

    await user.click(screen.getByRole("button", { name: /HANDS FREE/ }));
    await waitFor(() => expect(room()).not.toBeNull());
    room().onEnd("d2F2");

    await waitFor(() => expect(onSpeak).toHaveBeenCalledTimes(1));
    expect(onSpeak).toHaveBeenCalledWith("una vez", [], []);
  });

  it("says which of its five states it is in, and releases the microphone when pressed again", async () => {
    const user = userEvent.setup();
    allowed();
    const { room, stop } = theRoom();
    draw();

    const button = screen.getByRole("button", { name: /HANDS FREE/ });
    await user.click(button);

    // The half-second in which the noise floor is measured is its own state: a control whose
    // only feedback is a label must not have a moment where the label is lying.
    expect(await screen.findByRole("button", { name: /THE ROOM/ })).toBeVisible();
    room().onRoom(0.01);
    expect(await screen.findByRole("button", { name: /LISTENING/ })).toBeVisible();

    await user.click(screen.getByRole("button", { name: /LISTENING/ }));
    expect(stop).toHaveBeenCalled();
    expect(await screen.findByRole("button", { name: /HANDS FREE/ })).toBeVisible();
  });

  it("asks before it listens, exactly as the button beside it does", async () => {
    const user = userEvent.setup();
    vi.mocked(invoke).mockImplementation((command: string) =>
      command === "get_settings"
        ? Promise.resolve({ concurrentCrew: false, microphone: null })
        : new Promise<never>(() => {}),
    );
    theRoom();
    draw();

    await user.click(screen.getByRole("button", { name: /HANDS FREE/ }));

    expect(
      await screen.findByRole("dialog", { name: /would like to listen/i }),
    ).toBeVisible();
    expect(vi.mocked(converse)).not.toHaveBeenCalled();
  });
});

/**
 * A guess has to say it was guessing.
 *
 * The owner said *"hazme una tabla con esos datos"* and read back
 * `Αυτοί, πρέπει να τα βλακουμε τα τάτια.` — Greek, confidently, with nothing on screen saying a
 * decision had been made. It read as a bad ear rather than as a wrong language, and there was
 * nothing to act on.
 */
describe("what is said about a language nobody chose", () => {
  it("says nothing when the language was chosen", () => {
    // No decision was made, so there is no reading. An instrument that never moves is worse
    // than none.
    expect(aboutTheLanguage({})).toBeNull();
    /*
      **And `null` is how it actually arrives.** `Option<String>` without `skip_serializing_if`
      serialises as `null`, and the type said `heardAs?: string` — so this note fired on every
      transcription and read `Heard as null (0% sure)` under words that had been heard
      perfectly. A type describing a wire it has not been checked against is a type that will be
      wrong about exactly one value, and this is that value.
    */
    expect(aboutTheLanguage({ heardAs: null, sure: null })).toBeNull();
    expect(aboutTheLanguage({ heardAs: "", sure: null })).toBeNull();
  });

  it("says nothing when it was sure", () => {
    // A clean sentence on this machine reads `p = 0.993`. Speaking on every transcription is how
    // a note becomes furniture people stop seeing.
    expect(aboutTheLanguage({ heardAs: "es", sure: 0.993 })).toBeNull();
  });

  it("names the language and how sure it was, and where to settle it", () => {
    const said = aboutTheLanguage({ heardAs: "el", sure: 0.41 });
    expect(said).toContain("41%");
    expect(said).toContain("Settings");
    // The language by name rather than by code: `el` is not something anybody reads as Greek.
    expect(said).not.toMatch(/Heard as el\b/);
  });

  it("cannot be taken down by a code Intl refuses", () => {
    /*
      The owner met `The ear stopped: RangeError: invalid_argument`, and the whole transcription
      was lost to it — a *pretty name* for a language failing took down the words it was about.

      Measured in this WebView2: `Intl.DisplayNames.of("")` and `of("auto")` both throw, while
      `of("xx")` quietly answers `"xx"`. So an unknown code was never the hazard; a non-language
      was, and whisper prints `auto` when it did not decide.
    */
    for (const code of ["", "auto", "xx", "not a language at all"]) {
      expect(() => aboutTheLanguage({ heardAs: code, sure: 0.4 })).not.toThrow();
    }
    expect(aboutTheLanguage({ heardAs: "xx", sure: 0.4 })).toContain("xx");
  });

  it("still names it when the build printed no probability", () => {
    // The language is the half that matters; the probability is optional.
    expect(aboutTheLanguage({ heardAs: "fr" })).toContain("Settings");
  });
});
