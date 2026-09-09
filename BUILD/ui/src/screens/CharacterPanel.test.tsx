/**
 * What a character may be asked for, and how a connected source is offered.
 *
 * The defect these assertions exist to prevent: a connected MCP server was offered one
 * tick-box per tool. Playwright alone put two dozen of them on the screen, and ticking them
 * wrote that day's twenty-four ids into the character's file — frozen, so a tool the server
 * added later was missing and nothing said so.
 *
 * A server is now one box, and what it grants is answered live.
 */

import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";

import { CharacterPanel } from "./CharacterPanel";
import type {
  CapabilityGroup,
  CharacterSummary,
  Inherited,
  LauncherView,
} from "../ipc/contracts";

const PLAYWRIGHT: CapabilityGroup = {
  id: "mcp:playwright",
  label: "Playwright",
  tools: [
    "playwright_browser_click",
    "playwright_browser_close",
    "playwright_navigate",
  ],
  answering: true,
};

/** The same server, configured but not answering — a broken argument is enough. */
const SILENT: CapabilityGroup = {
  id: "mcp:spotify",
  label: "Spotify",
  tools: [],
  answering: false,
};

function mage(requested: readonly string[] | null): CharacterSummary {
  return {
    id: "mage",
    name: "Mage",
    archetype: "researcher",
    role: "Turns goals into designs",
    prompt: "You explore before committing.",
    skills: [],
  speaksWith: null,
    soundsLike: null,
    provider: "ollama",
    agent: null,
    model: "qwen3",
    brain: "qwen3",
    parameters: {
      temperature: null,
      topP: null,
      contextTokens: null,
      contextPolicy: null,
      reasoning: null,
    },
    tuning: {},
    dormantTuning: [],
    requestedCapabilities: requested ?? null,
    worlds: ["default"],
    routine: [{ activity: "reading", seconds: 12 }],
    sprite: null,
    icon: null,
    portrait: null,
    spriteMark: null,
    iconMark: null,
    actionMarks: {},
    file: "mage.toml",
  };
}

function view(
  character: CharacterSummary,
  groups: readonly CapabilityGroup[],
): LauncherView {
  return {
    orchestrator: {
      name: "KISLOK",
      isUnnamed: false,
      portrait: null,
      problems: [],
    },
    worlds: [],
    characters: [character],
    vocabulary: {
      archetypes: ["researcher"],
      places: [],
      reasoning: ["low", "medium", "high"],
      capabilities: [
        "read_file",
        "write_file",
        ...groups.map((g) => g.id),
        "vision",
      ],
      built: ["read_file", "write_file", ...groups.map((g) => g.id)],
      groups,
    },
    sessionSeconds: 0,
    problems: [],
    skills: [],
    definitionProblems: [],
  };
}

/**
 * The panel asks the Engine two things on open. Neither is what is under test here.
 *
 * The surface arm read `provider_surface` and the command is `get_surface`, so it had never
 * matched anything — the call fell through to the promise that never settles, which looks
 * exactly like a panel waiting politely. A mock naming a command that does not exist is a mock
 * that cannot fail, and it went unnoticed for as long as nothing depended on the answer.
 */
function quiet(
  surface: unknown = { window: null, controls: [], can: null },
  inherited: unknown = NOTHING_APPLIED,
) {
  vi.mocked(invoke).mockImplementation((command: string) => {
    if (command === "get_surface") return Promise.resolve(surface);
    if (command === "inherited_runtime") return Promise.resolve(inherited);
    if (command === "agents") return Promise.resolve([]);
    return new Promise<never>(() => {});
  });
}

/** A Brain nobody has configured in MODELS. The honest default for these tests. */
const NOTHING_APPLIED: Inherited = {
  model: "qwen3",
  window: null,
  source: "nothing",
  profile: null,
  generation: null,
  stable: false,
};

async function openEditor(
  v: LauncherView,
  onConfigureInModels?: (model: string) => void,
) {
  render(
    <CharacterPanel
      view={v}
      providers={[]}
      onChanged={() => {}}
      onConfigureInModels={onConfigureInModels}
    />,
  );
  await userEvent.click(screen.getByRole("button", { name: "Edit" }));
}

describe("CharacterPanel — requested capabilities", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    quiet();
  });

  it("offers a connected server once, never once per tool", async () => {
    await openEditor(view(mage([]), [PLAYWRIGHT]));

    // One box, carrying the count so the size of the grant is visible before it is made.
    expect(
      screen.getByRole("checkbox", { name: /Playwright · 3/ }),
    ).toBeInTheDocument();
    for (const tool of PLAYWRIGHT.tools) {
      expect(
        screen.queryByRole("checkbox", { name: tool }),
      ).not.toBeInTheDocument();
    }
  });

  it("shows what the box grants without leaving the screen", async () => {
    // Worth seeing: an outside tool is registered with every effect and no reversal, because
    // MCP declares neither (ADR-0008). "What am I giving them" has to be answerable here.
    await openEditor(view(mage([]), [PLAYWRIGHT]));

    expect(screen.getByText("Playwright offers 3")).toBeInTheDocument();
    for (const tool of PLAYWRIGHT.tools) {
      expect(screen.getByText(tool)).toBeInTheDocument();
    }
  });

  it("ticking a source settles everything inside it", async () => {
    // A mid-turn grant can write a single tool id — the only way a partial grant is
    // expressible. It must not survive the source being ticked, or the file would hold two
    // answers to one question and the next reader would have to guess which won.
    await openEditor(
      view(mage(["read_file", "playwright_browser_click"]), [PLAYWRIGHT]),
    );

    const loose = screen.getByRole("checkbox", {
      name: "Playwright Browser Click",
    });
    expect(loose).toBeChecked();

    await userEvent.click(
      screen.getByRole("checkbox", { name: /Playwright · 3/ }),
    );

    expect(
      screen.queryByRole("checkbox", { name: "Playwright Browser Click" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("checkbox", { name: /Playwright · 3/ }),
    ).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Read File" })).toBeChecked();
  });

  it("does not dim a tool that exists just because it is inside a source", async () => {
    // It reaches this list only through a mid-turn grant, and it is a working tool. Dimming it
    // would say a capability that runs today does not exist yet.
    await openEditor(view(mage(["playwright_navigate"]), [PLAYWRIGHT]));

    const tool = screen
      .getByRole("checkbox", { name: "Playwright Navigate" })
      .closest("label");
    expect(tool).not.toHaveClass("chip--pending");
  });

  it("keeps the box of a server that is configured but not answering", async () => {
    // The reported defect, in order: the NPC tried a Spotify tool, the server was not starting,
    // its box left the screen, and saving anything wrote its absence to the character's file.
    // It looked like the capability had deleted itself.
    await openEditor(
      view(mage(["mcp:spotify", "read_file"]), [PLAYWRIGHT, SILENT]),
    );

    const box = screen.getByRole("checkbox", {
      name: /Spotify · not answering/,
    });
    expect(box).toBeChecked();
    expect(box.closest("label")).toHaveAttribute(
      "title",
      expect.stringContaining("The request is kept"),
    );
  });

  it("does not turn an undecided character into today's list by opening the form", async () => {
    // Undecided means *whatever exists whenever they work*. It used to arrive as a materialised
    // list, which the editor drew as ticked boxes and the next save froze — including whichever
    // servers happened to be answering that minute.
    await openEditor(view(mage(null), [PLAYWRIGHT, SILENT]));

    expect(screen.getByText(/Nobody has chosen yet/)).toBeInTheDocument();
    // Everything reads as ticked, because that is what undecided resolves to.
    expect(screen.getByRole("checkbox", { name: "Read File" })).toBeChecked();
    expect(
      screen.getByRole("checkbox", { name: /Playwright · 3/ }),
    ).toBeChecked();

    // And the first tick is the moment it becomes a decision — starting from everything, so
    // unticking one thing never silently removes the rest.
    await userEvent.click(screen.getByRole("checkbox", { name: "Read File" }));
    expect(screen.queryByText(/Nobody has chosen yet/)).not.toBeInTheDocument();
    expect(
      screen.getByRole("checkbox", { name: "Read File" }),
    ).not.toBeChecked();
    expect(
      screen.getByRole("checkbox", { name: /Playwright · 3/ }),
    ).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Write File" })).toBeChecked();
  });

  it("still says which requests nothing can honour yet", async () => {
    // ADR-0026, and the reason `vision` is on the list at all: a ticked box that does nothing
    // is a promise Epoch did not make, so the surface says which is which.
    await openEditor(view(mage([]), [PLAYWRIGHT]));

    expect(
      screen.getByRole("checkbox", { name: "Vision" }).closest("label"),
    ).toHaveClass("chip--pending");
    expect(
      screen.getByRole("checkbox", { name: /Playwright · 3/ }).closest("label"),
    ).not.toHaveClass("chip--pending");
  });
});

describe("CharacterPanel — ways of working", () => {
  /** A Skill that wants something, and a crew member who may or may not have it. */
  function withSkill(
    requested: readonly string[] | null,
    requires: readonly string[],
  ): LauncherView {
    const base = view(mage(requested), []);
    return {
      ...base,
      skills: [
        {
          id: "code-review",
          name: "Code review",
          summary: "How this project reviews a change.",
          requires,
        },
      ],
    };
  }

  it("says what a Skill wants that this character does not have", async () => {
    // The whole reason `requires` reaches a surface. Without it the Skill fails on its third
    // step, in a model's words, far from the screen where somebody assigned it — and the person
    // who assigned it had no way to know.
    await openEditor(withSkill(["read_file"], ["mcp:playwright"]));

    await userEvent.click(
      screen.getByRole("checkbox", { name: /Code review/ }),
    );

    expect(screen.getByText(/wants mcp:playwright/)).toBeInTheDocument();
  });

  it("says nothing when the character already has what the Skill wants", async () => {
    await openEditor(
      withSkill(["read_file", "mcp:playwright"], ["mcp:playwright"]),
    );

    await userEvent.click(
      screen.getByRole("checkbox", { name: /Code review/ }),
    );

    expect(screen.queryByText(/wants /)).not.toBeInTheDocument();
  });

  it("treats an undecided character as having everything available", async () => {
    // Undecided is not "has nothing": it resolves to whatever exists when they work. Warning
    // there would be the surface inventing a problem the Engine does not have.
    await openEditor(withSkill(null, ["read_file"]));

    await userEvent.click(
      screen.getByRole("checkbox", { name: /Code review/ }),
    );

    expect(screen.queryByText(/wants /)).not.toBeInTheDocument();
  });

  it("sits below the capabilities, not among the tuning knobs", async () => {
    // Placement is a decision, and it was made twice: above the capabilities it landed between
    // Reasoning and the tick-boxes and read as one more numeric knob. Asserted rather than left
    // to whoever edits this file next, because nothing else here would notice it moving back.
    await openEditor(withSkill(["read_file"], ["read_file"]));

    const capabilities = screen.getByText("Requested capabilities");
    const skills = screen.getByText("Ways of working");

    expect(
      capabilities.compareDocumentPosition(skills) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });
});

describe("CharacterPanel — what a model declares", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("shows what the model says about itself, beside where it is chosen", async () => {
    // Measured on a real machine: gemma4:12b declares vision *and* audio, and qwen3:14b
    // declares neither. Nobody could have written that from memory — which is the argument for
    // reading it rather than remembering it, and the reason it belongs next to the field where
    // the choice is made.
    quiet({
      window: 131072,
      controls: [],
      can: { sees: true, hears: true, usesTools: true, thinks: true },
    });
    await openEditor(view(mage([]), []));

    for (const said of ["vision", "audio", "tools", "thinking"]) {
      expect(await screen.findByText(said)).toHaveClass("declares--on");
    }
  });

  it("draws what a model does not declare, rather than leaving a gap", async () => {
    // A gap says nothing. "This model does not say it can see" is information, and it is the
    // difference between choosing gemma4 on purpose and choosing it by accident.
    quiet({
      window: 40960,
      controls: [],
      can: { sees: false, hears: false, usesTools: true, thinks: true },
    });
    await openEditor(view(mage([]), []));

    expect(await screen.findByText("vision")).not.toHaveClass("declares--on");
    expect(screen.getByText("tools")).toHaveClass("declares--on");
  });

  it("says why a toolless model will be given no tools", async () => {
    // The one declaration that changes what Epoch does. Without this line the user sees
    // capabilities that do nothing: a model handed tools it cannot call does not fail, it
    // ignores them, and the character answers as though they were never there.
    quiet({
      window: null,
      controls: [],
      can: { sees: false, hears: false, usesTools: false, thinks: false },
    });
    await openEditor(view(mage([]), []));

    expect(
      await screen.findByText(/Epoch will not give it any/),
    ).toBeInTheDocument();
    // And the requests survive: they apply again on a model that can (ADR-0026).
    expect(screen.getByText(/kept and apply again/)).toBeInTheDocument();
  });

  it("does not withhold tools from a runtime that was never asked about them", async () => {
    // **The defect this state exists for.** LM Studio describes a model's modalities and says
    // nothing about tools, so this field used to arrive as `false` — the warning fired, Epoch
    // declared no capabilities, and `gemma4-12b` answered *"no tengo acceso a herramientas en
    // este mundo"*. Honestly, too: it had none. The same model calls tools perfectly well.
    quiet({
      window: 262144,
      controls: [],
      can: { sees: true, hears: false, usesTools: null, thinks: false },
    });
    await openEditor(view(mage([]), []));

    // The badge is still drawn and still dark — *unasked* is worth showing.
    expect(await screen.findByText("tools")).not.toHaveClass("declares--on");
    // But nothing is taken away, and nothing is claimed.
    expect(screen.queryByText(/will not give it any/)).not.toBeInTheDocument();
  });

  it("shows nothing at all when the backend did not answer", async () => {
    // **Unasked is not "cannot".** A row of dark capabilities for a model that simply did not
    // reply would be an invented reading, and this field exists to stop exactly those.
    quiet({ window: null, controls: [], can: null });
    await openEditor(view(mage([]), []));

    expect(screen.queryByText("vision")).not.toBeInTheDocument();
    expect(screen.queryByText(/will not give it any/)).not.toBeInTheDocument();
  });
});

/**
 * What a character shows about a window it no longer owns.
 *
 * ADR-0026's amendment moved the window to MODELS, and the failure it opened is specific: a
 * panel with a number on it and nothing saying whose the number is. Everything below is about
 * attribution, and about the two silences that must not be filled in.
 */
describe("CharacterPanel — the window belongs to MODELS", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("offers nothing to type a context size into", async () => {
    // The control that stood here was unsatisfiable in two directions: `llama-server` takes
    // `--ctx-size` when it spawns the child that holds the model, so two characters on one
    // Brain cannot have different windows; and `131072` is a number a smaller card cannot
    // honour. What replaced it is a reading.
    quiet({ window: 131072, controls: [], can: null });
    await openEditor(view(mage([]), []));

    expect(screen.queryByText(/Context tokens/)).not.toBeInTheDocument();
    expect(await screen.findByText("Runtime configuration")).toBeInTheDocument();
    expect(screen.getByText("Inherited from MODELS")).toBeInTheDocument();
  });

  it("says which of three things a window is a reading of", async () => {
    // `64K` alone is the gauge that identifies nobody: a searched profile, a chosen loadout and
    // a backend's own answer are three different facts.
    quiet({ window: 131072, controls: [], can: null }, {
      model: "qwen3",
      window: 65536,
      source: "profile",
      profile: "BALANCED",
      generation: 41.2,
      stable: true,
    } satisfies Inherited);
    await openEditor(view(mage([]), []));

    expect(await screen.findByText("64K")).toBeInTheDocument();
    expect(screen.getByText(/measured/)).toBeInTheDocument();
    expect(screen.getByText(/BALANCED/)).toBeInTheDocument();
    expect(screen.getByText(/41\.2 tok\/s/)).toBeInTheDocument();
    expect(screen.getByText(/stable/)).toBeInTheDocument();
  });

  it("shows a window with no search behind it, and no speed beside it", async () => {
    /*
      **Only a profile carries a speed.** A model's last timing was taken at whatever loadout
      was current that afternoon, so printing it beside a window chosen afterwards would be a
      real reading of a different configuration — the most convincing way a gauge can lie.
    */
    quiet({ window: 131072, controls: [], can: null }, {
      model: "qwen3",
      window: 32768,
      source: "loadout",
      profile: null,
      generation: null,
      stable: false,
    } satisfies Inherited);
    await openEditor(view(mage([]), []));

    expect(await screen.findByText("32K")).toBeInTheDocument();
    expect(screen.getByText(/configured/)).toBeInTheDocument();
    expect(screen.getByText("Not measured")).toBeInTheDocument();
    expect(screen.queryByText(/tok\/s/)).not.toBeInTheDocument();
  });

  it("says nothing is configured rather than reaching for a plausible number", async () => {
    // The surface reports 131072 for this model and that is **not** what MODELS applied. A
    // panel that borrowed it would be answering a question nobody asked it.
    quiet({ window: 131072, controls: [], can: null });
    await openEditor(view(mage([]), []));

    expect(
      await screen.findByText("Not configured in MODELS"),
    ).toBeInTheDocument();
    expect(screen.queryByText("128K")).not.toBeInTheDocument();
  });

  it("leads to where the window is actually decided", async () => {
    // A read-only reading with no way through to the thing that decides it is a dead end
    // wearing a label. And it reveals — it must never start a search.
    const went = vi.fn();
    quiet({ window: 131072, controls: [], can: null });
    await openEditor(view(mage([]), []), went);

    await userEvent.click(
      await screen.findByRole("button", { name: /Configured in MODELS/ }),
    );
    expect(went).toHaveBeenCalledWith("qwen3");
  });

  it("opens the row by the name MODELS knows, not the one the router serves", async () => {
    /*
      Found by pressing it. A character on llama.cpp carries `gemma4-12b`; the Models deck and
      the profile store both key on `gemma4:12b`. Every name without a colon matches either way,
      which is how it survived — and the deck **drops** an errand naming a model it does not
      hold, so the door would have travelled and opened nothing, on exactly the models whose
      window it had just read correctly.

      The Engine returns the row's own name on the reading. One place decides which row this is.
    */
    const went = vi.fn();
    quiet({ window: 131072, controls: [], can: null }, {
      model: "gemma4:12b",
      window: 65536,
      source: "loadout",
      profile: null,
      generation: null,
      stable: false,
    } satisfies Inherited);
    await openEditor(view(mage([]), []), went);

    await userEvent.click(
      await screen.findByRole("button", { name: /Configured in MODELS/ }),
    );
    expect(went).toHaveBeenCalledWith("gemma4:12b");
    expect(went).not.toHaveBeenCalledWith("qwen3");
  });

  it("does not say a Brain is unconfigured while it is still being asked", async () => {
    /*
      **Found by driving the window, and it is the worst shape this can take.** The reading takes
      a moment — the deck's own list is a shelf read — and until it arrived the panel drew
      `Not configured in MODELS` for a model with a 64K loadout. A second later it corrected
      itself. So the wrong answer was the one on screen at the moment somebody opens the section,
      and it wore the shape of a finished measurement.

      `null` is *not asked* everywhere else in this codebase. It had simply never been given a
      shape on a surface.
    */
    vi.mocked(invoke).mockImplementation((command: string) => {
      if (command === "get_surface")
        return Promise.resolve({ window: 131072, controls: [], can: null });
      if (command === "agents") return Promise.resolve([]);
      // `inherited_runtime` never settles: the panel is caught mid-ask.
      return new Promise<never>(() => {});
    });
    await openEditor(view(mage([]), []));

    expect(await screen.findByText("Runtime configuration")).toBeInTheDocument();
    expect(
      screen.queryByText("Not configured in MODELS"),
    ).not.toBeInTheDocument();
    // And the door waits too: it is addressed by the name the reading comes back with, so
    // offering it early would send the deck a spelling it does not hold.
    expect(
      screen.queryByRole("button", { name: /Configured in MODELS/ }),
    ).not.toBeInTheDocument();
  });

  it("shows the default policy as chosen without writing it to the file", async () => {
    // Unset *is* Adaptive in the Engine. A group with nothing selected would claim a decision
    // nobody made, and invite a click that changes nothing.
    quiet({ window: 131072, controls: [], can: null });
    await openEditor(view(mage([]), []));

    const adaptive = await screen.findByRole("radio", { name: /Adaptive/ });
    expect(adaptive).toBeChecked();
    for (const other of ["Compact", "Long", "Custom"]) {
      expect(
        screen.getByRole("radio", { name: new RegExp(other) }),
      ).not.toBeChecked();
    }
  });
});
