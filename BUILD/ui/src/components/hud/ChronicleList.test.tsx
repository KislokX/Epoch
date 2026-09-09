/** The durable and live record in a dialogue, without a World or an Engine. */

import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// The bytes of a shared picture are fetched by reference now, not carried in the projection —
// so the test stands in for the Engine's answer rather than for the whole IPC layer.
const opened = vi.fn(async (_file: string) => null as string | null);

// The bytes no longer come through here: a picture is an `<img src>` on Epoch's own scheme, and
// the Engine serves it. What the test stands in for is the URL that scheme produces.
vi.mock("../../ipc/world", () => ({
  pictureSrc: (file: string) => `epoch://picture/${file}`,
  openPicture: (file: string) => opened(file as never),
}));

import { ChronicleList } from "./ChronicleList";
import type { CharacterView } from "../../ipc/contracts";
import type { Said } from "../../ipc/world";
import type { Working } from "../../experience/useTurn";

function person(id: string, name: string): CharacterView {
  return {
    id,
    name,
    archetype: "guardian",
    home: "tower",
    place: "tower",
    activity: "reading",
    action: "idle",
    actions: {},
    class: "idle",
    mark: null,
    icon: null,
    speaksWith: null,
    soundsLike: null,
    routine: [],
  };
}

const mage = person("mage", "Mage");
const paladin = person("paladin", "Robo");

function draw(
  said: readonly Said[] = [],
  working: readonly Working[] = [],
  compacting: number | null = null,
  onOpenLink: (url: string) => void = () => {},
) {
  return render(
    <ChronicleList
      who={mage}
      model="qwen3:14b"
      crew={[mage, paladin]}
      said={said}
      writing=""
      thinking={false}
      compacting={compacting}
      working={working}
      onOpenLink={onOpenLink}
    />,
  );
}

describe("the dialogue Chronicle", () => {
  it("a colleagues words are never shown as the person you are talking to", () => {
    // The defect this prevents reported somebody else's contribution as Mage's work.
    draw([
      {
        who: "paladin",
        content: "I checked the patch.",
        attachments: [],
        images: [],
        kind: "answered",
      },
    ]);

    expect(screen.getByText("ROBO")).toBeInTheDocument();
    expect(screen.queryByText("MAGE")).not.toBeInTheDocument();
  });

  it("draws a picture the crew made, rather than naming the file", async () => {
    // The same argument that put shared pictures in the transcript, from the other side: a mark
    // reading "a lighthouse at dawn" with no picture under it is a conversation about something
    // the reader cannot see — and they asked for a picture, not a receipt.
    draw([
      {
        who: null,
        content: "a lighthouse at dawn",
        attachments: [],
        images: [{ name: "a1b2c3.png", file: "a1b2c3.png" }],
        kind: "produced",
      },
    ]);

    expect(await screen.findByLabelText("Made here")).toBeInTheDocument();
    // The picture is inside a control that opens it, so the accessible name belongs to the
    // button — an image labelled `a1b2c3.png` would say what it is and not what pressing does.
    const opens = await screen.findByRole("button", { name: /open a1b2c3\.png/i });
    expect(opens.querySelector("img")).toHaveAttribute(
      "src",
      "epoch://picture/a1b2c3.png",
    );
    // The line is still there: what it is, above what it looks like.
    expect(screen.getByText("a lighthouse at dawn")).toBeInTheDocument();
  });

  it("a produced thing that is not a picture is still just a mark", () => {
    draw([
      {
        who: null,
        content: "Changed config.toml",
        attachments: [],
        images: [],
        kind: "produced",
      },
    ]);
    expect(screen.queryByLabelText("Made here")).not.toBeInTheDocument();
  });

  it("evidence and approval are Chronicle marks rather than speech", () => {
    draw([
      {
        who: "mage",
        content: "Changed config.toml",
        attachments: [],
        images: [],
        kind: "produced",
      },
      {
        who: null,
        content: "You allowed write_file",
        attachments: [],
        images: [],
        kind: "approved",
      },
    ]);

    expect(screen.getByText("Changed config.toml").closest("p")).toHaveClass(
      "dlg__mark--produced",
    );
    expect(screen.getByText("You allowed write_file").closest("p")).toHaveClass(
      "dlg__mark--approved",
    );
  });

  it("names an attached reference without reprinting its contents in the Chronicle", () => {
    draw([
      {
        who: null,
        content: "Compare the options in this file.",
        attachments: [{ name: "laptops.md", bytes: 284 }],
        images: [],
        kind: "said",
      },
    ]);

    expect(screen.getByText("attached laptops.md (284 B)")).toBeInTheDocument();
    expect(
      screen.getByText("Compare the options in this file."),
    ).toBeInTheDocument();
  });

  it("draws a shared image rather than naming it", async () => {
    // The one place where a name is the wrong projection. A text attachment is named because
    // reprinting a document into a chat on every open is noise; an image *is* what was shared,
    // and "attached screen.png" is a conversation about something the reader cannot see.
    draw([
      {
        who: null,
        content: "look at this",
        attachments: [],
        images: [{ name: "screen.png", file: "AAAA" }],
        kind: "said",
      },
    ]);

    const opens = await screen.findByRole("button", { name: /open screen\.png/i });
    expect(opens.querySelector("img")).toHaveAttribute(
      "src",
      "epoch://picture/AAAA",
    );
  });

  it("says an image is gone rather than drawing a broken one", async () => {
    // A file can leave the vault between the Chronicle being written and being read. A
    // broken-image icon explains nothing, and the sentence does.
    //
    // **How that is learned changed with the transport.** The bytes used to be fetched here, so
    // a missing file was a `null` this component could see; now the browser fetches the URL and
    // the Engine answers 404, which arrives as the image's own `error`. Nothing loads in jsdom,
    // so the failure is delivered rather than waited for — the assertion is about what the
    // component does with it, which is the half that is ours.
    draw([
      {
        who: null,
        content: "look at this",
        attachments: [],
        images: [{ name: "screen.png", file: "gone.png" }],
        kind: "said",
      },
    ]);

    const picture = await screen.findByRole("button", {
      name: /open screen\.png/i,
    });
    const img = picture.querySelector("img");
    if (!img) throw new Error("the picture is drawn before it fails");
    fireEvent.error(img);

    expect(
      await screen.findByText(/no longer in the vault/),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /open screen/i })).toBeNull();
  });

  it("makes an attachment-only message legible without inventing user speech", () => {
    draw([
      {
        who: null,
        content: "",
        attachments: [{ name: "brief.md", bytes: 12 }],
        images: [],
        kind: "said",
      },
    ]);

    expect(
      screen.getByText("Shared reference material without a written request."),
    ).toBeInTheDocument();
    expect(screen.getByText("attached brief.md (12 B)")).toBeInTheDocument();
  });

  it("a running capability has no result to claim yet", () => {
    draw(
      [],
      [
        {
          capability: "read_file",
          what: "Reading src/main.rs",
          running: true,
          ok: null,
          detail: "the first line that must wait",
        },
      ],
    );

    expect(screen.getByText("Reading src/main.rs")).toBeInTheDocument();
    expect(
      screen.queryByText("the first line that must wait"),
    ).not.toBeInTheDocument();
  });

  it("reports a completed compaction's coverage without presenting it as character speech", () => {
    draw([
      {
        who: null,
        content: "Context compacted. 4 earlier records now travel as a brief.",
        attachments: [],
        images: [],
        compaction: { covered: 4 },
        kind: "compacted",
      },
    ]);

    expect(screen.getByText("CONTEXT COMPACTED").closest("p")).toHaveClass(
      "dlg__mark--compacted",
    );
    expect(
      screen.getByText(/Coverage: the first 4 records/),
    ).toBeInTheDocument();
    expect(screen.getByText(/Chronicle remains intact/)).toBeInTheDocument();
    expect(screen.queryByText("MAGE")).not.toBeInTheDocument();
  });

  it("shows what is happening while the continuity brief is being prepared", () => {
    draw([], [], 4);

    expect(screen.getByRole("status")).toHaveTextContent("Compacting context");
    expect(screen.getByRole("status")).toHaveTextContent("4 earlier records");
  });

  it("opens a web address in any speaker's text without treating its trailing sentence stop as part of the URL", async () => {
    const user = userEvent.setup();
    const opened: string[] = [];
    draw(
      [
        {
          who: "paladin",
          content: "The source is https://epoch.example/guide.",
          attachments: [],
          images: [],
          kind: "answered",
        },
      ],
      [],
      null,
      (url) => opened.push(url),
    );

    await user.click(
      screen.getByRole("button", { name: "https://epoch.example/guide" }),
    );
    expect(opened).toEqual(["https://epoch.example/guide"]);
    expect(
      screen.getByText("The source is", { exact: false }).closest("p"),
    ).toHaveTextContent("https://epoch.example/guide.");
  });

  it("titles the live block with whoever is actually answering", () => {
    // Watched in the World: a message sent to Mage streamed under the other name because the
    // crew card had been clicked in between, and the finished answer then landed under MAGE —
    // correctly. A caret under the wrong name is a character appearing to say something they
    // never said, which is the one thing a Chronicle may not do.
    render(
      <ChronicleList
        who={paladin}
        model="qwen3:14b"
        crew={[mage, paladin]}
        said={[]}
        writing="Los faros se yerguen"
        speaking="Mage"
        thinking
        compacting={null}
        working={[]}
        onOpenLink={() => {}}
      />,
    );
    expect(screen.getByText("MAGE")).toBeInTheDocument();
    expect(screen.queryByText("ROBO")).not.toBeInTheDocument();
  });

  it("falls back to the selected character when nobody else is speaking", () => {
    render(
      <ChronicleList
        who={paladin}
        model="qwen3:14b"
        crew={[mage, paladin]}
        said={[]}
        writing="thinking aloud"
        thinking
        compacting={null}
        working={[]}
        onOpenLink={() => {}}
      />,
    );
    expect(screen.getByText("ROBO")).toBeInTheDocument();
  });
  it("opens the picture where the machine opens pictures, whoever put it there", async () => {
    // **One door for both kinds.** `open_made` asks whether this World *made* a thing, which is
    // right for the FILES list and refuses half of a conversation: the Chronicle draws what the
    // crew produced and what the person pasted with the same component, because to a
    // conversation they are the same kind of thing.
    //
    // And it opens the file rather than enlarging it here. A viewer inside the World would be a
    // second place a picture can live, with its own zoom and its own idea of what fits, when the
    // person already has an application that does all of it and remembers where the file is.
    opened.mockClear();
    draw([
      {
        who: null,
        content: "look at this",
        attachments: [],
        images: [{ name: "screenshot.png", file: "a1b2c3.png" }],
        kind: "said",
      },
    ]);

    await userEvent.click(await screen.findByRole("button", { name: /open screenshot/i }));
    // By the vault's name, never by what the user's file was called.
    expect(opened).toHaveBeenCalledWith("a1b2c3.png");
  });

  it("says why it could not open, where the click was", async () => {
    // A picture that does nothing when clicked is a bug the user cannot report.
    opened.mockClear();
    opened.mockResolvedValueOnce("a1b2c3.png is in this conversation and is not in the vault");
    draw([
      {
        who: null,
        content: "look at this",
        attachments: [],
        images: [{ name: "screenshot.png", file: "a1b2c3.png" }],
        kind: "said",
      },
    ]);

    await userEvent.click(await screen.findByRole("button", { name: /open screenshot/i }));
    expect(await screen.findByText(/not in the vault/)).toBeInTheDocument();
  });
});
