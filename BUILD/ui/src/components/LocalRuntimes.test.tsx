import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const installRuntime = vi.fn(async () => null);

let engines: unknown[] = [];
/** What this machine holds. Mutable, so the empty case can be a test rather than a hypothetical. */
let weights: unknown[] | null = null;
/** What a runtime keeps in its own cache. llama.cpp's `--cache-list`. */
let cached: string[] = [];

vi.mock("../ipc/launcher", () => ({
  startRuntime: vi.fn(async () => null),
  // The backend read is its own probe and lands a moment after the deck. Empty here is the
  // honest default for these assertions: *not asked yet*, which draws nothing.
  runtimeEngines: vi.fn(async () => engines),
  chooseEngine: vi.fn(async () => "chosen"),
  compressCache: vi.fn(async () => "saved"),
  proveRuntime: vi.fn(async () => null),
  importWeights: vi.fn(async () => null),
  startRouter: vi.fn(async () => "A terminal opened on llama.cpp, offering all 2 models."),
  lendAllToLmStudio: vi.fn(async () => "2 models are on LM Studio's shelf."),
  // Ollama's own models, which llama.cpp can open without a second download — measured: the
  // blob begins `GGUF` and `llama-server -m <blob>` loaded `qwen3:14b` in 2.4 s on the card.
  sharedWeights: vi.fn(async () => weights ?? [
    { name: "qwen3:14b", from: "Ollama", path: "/blobs/sha256-a8cc", bytes: 9_276_184_896 },
    // The other shelf. `SAVE THE FILE` wrote it and nothing was looking at it.
    {
      name: "Qwen3-UD-IQ3_XXS",
      from: "Saved here",
      path: "/vault/models/unsloth-Qwen3-GGUF/Qwen3-UD-IQ3_XXS.gguf",
      bytes: 12_000_000_000,
    },
  ]),
  installRuntime: (...a: unknown[]) =>
    (installRuntime as unknown as (...x: unknown[]) => Promise<null>)(...a),
  localRuntimes: vi.fn(async () => [
    {
      id: "llama_cpp",
      name: "llama.cpp",
      installed: true,
      foundAt: "/opt/homebrew/bin/llama",
      serving: true,
      endpoint: "http://127.0.0.1:8080",
      models: ["Qwen3.8-27B-UD-IQ3_XXS"],
      resident: [],
      devices: [],
      handicap: null,
      cached,
      install: "brew install llama.cpp",
      start: "\"/opt/homebrew/bin/llama-server\"",
    },
    {
      id: "lm_studio",
      name: "LM Studio",
      installed: true,
      foundAt: "/Users/someone/.lmstudio/bin/lms",
      serving: false,
      endpoint: "http://127.0.0.1:1234",
      models: [],
      resident: [],
      devices: [],
      handicap: null,
      cached,
      install: "brew install --cask lm-studio",
      start: "\"/Users/someone/.lmstudio/bin/lms\" server start",
    },
  ]),
}));

import { LocalRuntimes } from "./LocalRuntimes";

describe("what this panel tells the rest of the Launcher", () => {
  it("reports its own reading as soon as it has one", async () => {
    /*
      **The defect, stated as a test.** This panel probes when the deck opens; the Launcher
      probed once when Epoch started and kept that photograph. So a user who started llama.cpp
      read SERVING here and could not find it in CREW LINKS at all -- two measurements of one
      fact, left to drift.

      Measured in the window on 2026-09-08: `ollama serve` answering with one model, and the
      panel still saying *not running* forty-five seconds later.
    */
    const told = vi.fn();
    render(<LocalRuntimes onRead={told} />);
    await waitFor(() => expect(told).toHaveBeenCalledTimes(1));
  });

  it("does not report it twice when the press already asks a wider question", async () => {
    // ASK AGAIN goes through `onChanged`, which re-probes the agents as well. Firing both would
    // make one press measure the backends twice and the agents once, which is not what either
    // callback means.
    const told = vi.fn();
    const wider = vi.fn();
    render(<LocalRuntimes onRead={told} onChanged={wider} />);
    await waitFor(() => expect(told).toHaveBeenCalledTimes(1));

    fireEvent.click(screen.getByRole("button", { name: "ASK AGAIN" }));
    await waitFor(() => expect(wider).toHaveBeenCalledTimes(1));
    expect(told).toHaveBeenCalledTimes(1);
  });
});

describe("the other runtimes on this machine", () => {
  it("keeps installed and serving as separate facts", async () => {
    // Three fixes: install it, start it, or nothing at all. A machine with LM Studio installed
    // and its server switched off is not a machine without LM Studio, and telling somebody to
    // install what they already have is the failure this shape prevents.
    render(<LocalRuntimes />);

    // Anchored, because `/SERVING/` also matches `NOT SERVING` — the two states this test
    // exists to keep apart.
    await waitFor(() =>
      expect(screen.getByText("SERVING · 1 model")).toBeInTheDocument(),
    );
    expect(screen.getByText("INSTALLED · NOT SERVING")).toBeInTheDocument();
    expect(screen.queryByText("NOT INSTALLED")).not.toBeInTheDocument();
  });

  it("says a serving runtime already is a Service, rather than offering to make it one", async () => {
    // **No button, because there is nothing left to decide.** ADD AS A SERVICE wrote a backend
    // entry saying what had already been measured — while a paired machine's LM Studio became a
    // Provider the moment that machine reported it serving. One fact, two rules, and which rule
    // applied depended on which computer the program was installed on.
    render(<LocalRuntimes />);
    await waitFor(() =>
      expect(screen.getByText(/your crew can be assigned to it/)).toBeInTheDocument(),
    );
    expect(screen.queryByRole("button", { name: "ADD AS A SERVICE" })).not.toBeInTheDocument();
    expect(screen.getByText(/127\.0\.0\.1:8080/)).toBeInTheDocument();
  });

  it("says nothing of the sort about one that is not answering", async () => {
    // A Provider is built from what is *serving*. Claiming a switched-off LM Studio is usable
    // would be a Service that looks configured and refuses every turn — and since nobody chose
    // it, nobody would know why.
    render(<LocalRuntimes />);
    await waitFor(() =>
      expect(screen.getByText("INSTALLED · NOT SERVING")).toBeInTheDocument(),
    );
    expect(screen.getAllByText(/your crew can be assigned to it/)).toHaveLength(1);
  });

  it("offers START only for the one that is not answering", async () => {
    // A START beside something already serving is a button whose job is already done. And a
    // server Epoch owned would die when Epoch does, so this opens the machine's own terminal.
    render(<LocalRuntimes />);
    await waitFor(() =>
      expect(screen.getAllByRole("button", { name: "START" })).toHaveLength(1),
    );
    expect(screen.getByText(/lms.*server start/)).toBeInTheDocument();
  });

  it("does not ask llama.cpp which model, because it does not need to be asked", async () => {
    // llama.cpp sat installed and serving nothing, and the reason was not the button — using it
    // appeared to mean downloading a second copy of a model already on the disk. It does not:
    // Ollama stores unmodified GGUF, so this points `llama-server` straight at the blob.
    //
    // Measured 2026-08-21: `llama-server --models-dir` is a router — it reports every model in
    // a directory, loads one only when a request names it, and leaves the rest available. So
    // the dropdown was a question nobody needed to answer.
    //
    // In this fixture llama.cpp is already serving, so even the router button belongs to
    // nobody: one that is up already knows the shelf.
    render(<LocalRuntimes />);
    await waitFor(() => expect(screen.getByText("llama.cpp")).toBeInTheDocument());
    expect(screen.queryByRole("button", { name: "START WITH IT" })).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "START WITH EVERYTHING" }),
    ).not.toBeInTheDocument();
  });

  it("offers it when llama.cpp is here and holding nothing", async () => {
    // The state this exists for: installed, holding nothing, and now startable as a router over
    // everything this machine has rather than over one model somebody picked.
    const { localRuntimes } = await import("../ipc/launcher");
    (localRuntimes as unknown as { mockResolvedValueOnce: (v: unknown) => void })
      .mockResolvedValueOnce([
        {
          id: "llama_cpp",
          name: "llama.cpp",
          installed: true,
          foundAt: "/opt/homebrew/bin/llama-server",
          serving: false,
          endpoint: "http://127.0.0.1:8080",
          models: [],
          resident: [],
          devices: [],
          handicap: null,
          cached: [],
          install: "brew install llama.cpp",
          start: "\"/opt/homebrew/bin/llama-server\"",
        },
      ]);

    render(<LocalRuntimes />);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "START WITH EVERYTHING" })).toBeInTheDocument(),
    );
    // **Both shelves, counted.** Ollama's own blobs and what SAVE THE FILE wrote into the vault
    // — the second of which reached nothing at all before there was a shelf.
    expect(screen.getByText(/2 models, linked rather than copied/)).toBeInTheDocument();
    // **And the plain START is gone for llama.cpp**, which is a removal rather than an omission:
    // starting it bare gives a server with `Available models (0)` — measured — a Service that
    // answers and can never be used.
    expect(screen.queryByRole("button", { name: "START" })).not.toBeInTheDocument();
  });

  it("offers LM Studio every model rather than one, because it loads from its own shelf", async () => {
    // LM Studio loads from its own shelf, so the model is put on it — and never with
    // `lms import`, which defaults to *moving* the file and would take the blob out of Ollama's
    // store. Offered whether or not it is serving: stocking a shelf is not starting a server.
    render(<LocalRuntimes />);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "LEND IT EVERYTHING" })).toBeInTheDocument(),
    );
    // Hard links, so a shelf of five models adds bytes for none of them and Ollama keeps its
    // own. Both runtimes say it in this fixture, which is why the count rather than the
    // presence is asserted.
    expect(
      screen.getByText(/LM Studio's shelf — 2 models, linked rather than copied/),
    ).toBeInTheDocument();
  });

  it("offers Ollama only what it does not already have", async () => {
    // Importing one of its own blobs would hash nine gigabytes to add a manifest — measured:
    // `using existing layer`, 3m30s, and zero disk growth. So the list is the other shelf only.
    const { localRuntimes } = await import("../ipc/launcher");
    (localRuntimes as unknown as { mockResolvedValueOnce: (v: unknown) => void })
      .mockResolvedValueOnce([
        {
          id: "ollama",
          name: "Ollama",
          installed: true,
          foundAt: "/usr/local/bin/ollama",
          serving: true,
          endpoint: "http://127.0.0.1:11434",
          models: ["qwen3:14b"],
          resident: [],
          devices: [],
          handicap: null,
          cached: [],
          install: "brew install --cask ollama",
          start: "\"/usr/local/bin/ollama\" serve",
        },
      ]);

    render(<LocalRuntimes />);
    await waitFor(() =>
      expect(screen.getByText(/give it a model saved by the Workshop/)).toBeInTheDocument(),
    );
    expect(screen.getByText(/Qwen3-UD-IQ3_XXS · 12\.0 GB · Saved here/)).toBeInTheDocument();
    // Its own model is not on the list, because it already has it.
    expect(screen.queryByText(/qwen3:14b · 9\.3 GB/)).not.toBeInTheDocument();
  });

  it("says what a server offers and what it is actually holding, separately", async () => {
    // The reading the deck never took. `models` is a shelf; it was printed as "holding", so a
    // router listing five files on disk reported five models in memory. Somebody watching a
    // 7.5 GB process was being told something nothing had measured.
    const { localRuntimes } = await import("../ipc/launcher");
    (localRuntimes as unknown as { mockResolvedValueOnce: (v: unknown) => void })
      .mockResolvedValueOnce([
        {
          id: "llama_cpp",
          name: "llama.cpp",
          installed: true,
          foundAt: "/usr/local/bin/llama-server",
          serving: true,
          endpoint: "http://127.0.0.1:8080",
          models: ["gemma4-12b", "qwen3-14b"],
          resident: ["gemma4-12b"],
          devices: [],
          handicap: null,
          cached: [],
          install: "brew install llama.cpp",
          start: null,
        },
      ]);

    render(<LocalRuntimes />);
    await waitFor(() =>
      expect(
        screen.getByText(/offers gemma4-12b, qwen3-14b · in memory: gemma4-12b/),
      ).toBeInTheDocument(),
    );
  });

  it("reads zero out loud rather than going blank", async () => {
    // A gauge that disappears when it reads nothing is one nobody can trust when it reads
    // something. Between turns, "in memory: nothing" is the true and useful answer.
    const { localRuntimes } = await import("../ipc/launcher");
    (localRuntimes as unknown as { mockResolvedValueOnce: (v: unknown) => void })
      .mockResolvedValueOnce([
        {
          id: "lm_studio",
          name: "LM Studio",
          installed: true,
          foundAt: "/Applications/LM Studio.app",
          serving: true,
          endpoint: "http://127.0.0.1:1234",
          models: ["gemma4-12b"],
          resident: [],
          devices: [],
          handicap: null,
          cached: [],
          install: "brew install --cask lm-studio",
          start: null,
        },
      ]);

    render(<LocalRuntimes />);
    await waitFor(() =>
      expect(screen.getByText(/in memory: nothing/)).toBeInTheDocument(),
    );
  });

  it("says what llama.cpp will actually run the model on, and what that costs", async () => {
    // The measurement that made this a row rather than a comment: the same GGUF took 36.7s
    // through Epoch's llama.cpp and 11.3s through LM Studio, on the same card and the same
    // engine, because one was Vulkan and the other CUDA. Epoch cannot fix that — winget ships
    // no CUDA package — so the only honest thing it can do is stop hiding it.
    const { localRuntimes } = await import("../ipc/launcher");
    (localRuntimes as unknown as { mockResolvedValueOnce: (v: unknown) => void })
      .mockResolvedValueOnce([
        {
          id: "llama_cpp",
          name: "llama.cpp",
          installed: true,
          foundAt: "/usr/local/bin/llama-server",
          serving: true,
          endpoint: "http://127.0.0.1:8080",
          models: ["gemma4-12b"],
          resident: [],
          devices: ["Vulkan0: NVIDIA GeForce RTX 4070 SUPER (11997 MiB, 3555 MiB free)"],
          handicap: "This build runs your NVIDIA card through Vulkan. A CUDA build is faster.",
          install: "winget install ggml.llamacpp",
          start: null,
        },
      ]);

    render(<LocalRuntimes />);
    await waitFor(() =>
      expect(screen.getByText(/Vulkan0: NVIDIA GeForce RTX 4070 SUPER/)).toBeInTheDocument(),
    );
    expect(screen.getByText(/A CUDA build is faster/)).toBeInTheDocument();
  });

  it("says nothing at all when the program was never asked", async () => {
    // Ollama and LM Studio have no `--list-devices`. An empty list is unasked, and an empty
    // line about somebody's graphics card is exactly the invented gauge this deck deletes.
    const { localRuntimes } = await import("../ipc/launcher");
    (localRuntimes as unknown as { mockResolvedValueOnce: (v: unknown) => void })
      .mockResolvedValueOnce([
        {
          id: "ollama",
          name: "Ollama",
          installed: true,
          foundAt: "/usr/local/bin/ollama",
          serving: true,
          endpoint: "http://127.0.0.1:11434",
          models: [],
          resident: [],
          devices: [],
          handicap: null,
          cached: [],
          install: "brew install --cask ollama",
          start: null,
        },
      ]);

    render(<LocalRuntimes />);
    await waitFor(() => expect(screen.getByText(/in memory: nothing/)).toBeInTheDocument());
    expect(screen.queryByText(/Vulkan/)).not.toBeInTheDocument();
    expect(screen.queryByText(/CUDA/)).not.toBeInTheDocument();
  });

  it("offers no install for something already here", async () => {
    // Both are installed in this fixture, so there is nothing to offer and no button to press.
    render(<LocalRuntimes />);
    await waitFor(() => expect(screen.getByText("llama.cpp")).toBeInTheDocument());
    expect(screen.queryByRole("button", { name: "INSTALL" })).not.toBeInTheDocument();
  });

  it("shows what it will run before it runs it, and claims nothing afterwards", async () => {
    // Epoch opens the door and steps back — the licence, the elevation prompt and the output
    // belong to the person reading them. A terminal opened is all it can honestly claim.
    vi.resetModules();
    const { localRuntimes } = await import("../ipc/launcher");
    (localRuntimes as unknown as { mockResolvedValueOnce: (v: unknown) => void })
      .mockResolvedValueOnce([
        {
          id: "llama_cpp",
          name: "llama.cpp",
          installed: false,
          foundAt: null,
          serving: false,
          endpoint: "http://127.0.0.1:8080",
          models: [],
          resident: [],
          devices: [],
          handicap: null,
          cached: [],
          install: "winget install ggml.llamacpp",
          start: null,
        },
      ]);

    render(<LocalRuntimes />);
    await waitFor(() =>
      expect(screen.getByText("winget install ggml.llamacpp")).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByRole("button", { name: "INSTALL" }));
    await waitFor(() => expect(installRuntime).toHaveBeenCalledWith("llama_cpp"));
    expect(await screen.findByText(/press ASK AGAIN/)).toBeInTheDocument();
  });
});

/**
 * Which graphics backend a runtime will use.
 *
 * The chooser this replaces was refused once and rightly (11.20): a menu assembled from what is
 * *conceivable* offers every card every option and eventually recommends `Intel Arc: CUDA`.
 * These hold the property that makes this one different — every row is read from the machine —
 * and the three shapes a real answer can take.
 */
describe("which graphics a runtime uses", () => {
  beforeEach(() => {
    engines = [];
  });

  it("says nothing at all when nothing could be read", async () => {
    // **Never "CPU only".** No reading beats an invented one, and a runtime whose layout Epoch
    // does not know is unasked rather than answered for.
    engines = [
      { id: "lm_studio", name: "LM Studio", engines: { kind: "unknown" }, chose: null, compressedCache: false, canCompress: true },
    ];
    render(<LocalRuntimes />);
    await screen.findByText("LM Studio");
    expect(screen.queryByLabelText(/GRAPHICS/i)).toBeNull();
    expect(screen.queryByText(/CPU only/i)).toBeNull();
  });

  it("states one backend rather than offering a menu of one", async () => {
    // llama.cpp carries one per install, so the honest answer is a sentence — and it names the
    // thing that would change it, which is installing another build.
    engines = [
      { id: "llama_cpp", name: "llama.cpp", engines: { kind: "fixed", of: "CUDA" }, chose: null, compressedCache: false, canCompress: false },
    ];
    render(<LocalRuntimes />);
    expect(await screen.findByText(/this build is CUDA/i)).toBeTruthy();
    expect(screen.queryByLabelText(/GRAPHICS/i)).toBeNull();
  });

  it("offers only what is installed, and letting it choose is not one of them", async () => {
    engines = [
      {
        id: "lm_studio",
        name: "LM Studio",
        engines: {
          kind: "choice",
          of: [
            { id: "cuda_v13", name: "CUDA 13", chosen: false },
            { id: "vulkan", name: "Vulkan", chosen: false },
          ],
        },
        chose: null,
        compressedCache: false,
        canCompress: true,
      },
    ];
    render(<LocalRuntimes />);
    const pick = (await screen.findByLabelText(/GRAPHICS/i)) as HTMLSelectElement;
    const values = [...pick.options].map((it) => it.value);
    // Two engines and the empty row, which is the program's own detection: an absence rather
    // than a value, so it must not look like a third backend.
    expect(values).toEqual(["", "cuda_v13", "vulkan"]);
    expect(pick.value).toBe("");
    expect(screen.queryByText(/ROCm|Metal|Intel/i)).toBeNull();
  });

  it("offers the compressed cache only where a runtime can be told about it", async () => {
    // Measured 2026-08-30: LM Studio has no flag and no load route, and llama.cpp's cache is
    // chosen per model by the curve that measured it. A switch on either changes nothing.
    engines = [
      { id: "lm_studio", name: "LM Studio", engines: { kind: "unknown" }, chose: null, compressedCache: false, canCompress: true },
      { id: "llama_cpp", name: "llama.cpp", engines: { kind: "unknown" }, chose: null, compressedCache: false, canCompress: false },
    ];
    render(<LocalRuntimes />);
    const boxes = await screen.findAllByLabelText(/COMPRESS THE ATTENTION CACHE/i);
    expect(boxes).toHaveLength(1);
  });
});

/**
 * Why llama.cpp's row has no START.
 *
 * Asked by the owner, who went looking for the button. Both reasons were good and neither was on
 * screen: no plain START at all (bare, it serves `Available models (0)`), and its real one is
 * hidden while it is already up. An absence with nothing saying why reads as something missing.
 */
describe("llama.cpp's missing start button", () => {
  beforeEach(() => {
    engines = [];
  });

  it("says why while it is serving, and names what to look for", async () => {
    render(<LocalRuntimes />);
    const said = await screen.findByText(/its start is START WITH EVERYTHING/i);
    // The count is the row's own reading, and the fixture's llama.cpp offers one — so this
    // also holds the singular, which is the half a plural-by-default sentence gets wrong.
    expect(said.textContent).toMatch(/already serving 1 model —/i);
  });

  it("says nothing of the sort about a runtime that is stopped", async () => {
    // Then the button is there, and a sentence explaining its absence would be explaining
    // something that is not absent.
    render(<LocalRuntimes />);
    await screen.findByText("LM Studio");
    const notes = screen.queryAllByText(/its start is START WITH EVERYTHING/i);
    expect(notes).toHaveLength(1);
  });
});

/**
 * A machine with no models at all.
 *
 * Reported from the AMD machine: llama.cpp installed, its card listed on the row above, and no way
 * to start it anywhere on the deck. An absence with nothing saying why reads as something broken —
 * and this one was deliberate, which made it worse rather than better.
 */
describe("a machine that has nothing yet", () => {
  beforeEach(() => {
    engines = [];
    weights = null;
    cached = [];
  });

  it("never says there is nothing while it is serving something", async () => {
    /*
      **The contradiction as it was photographed.** The header read `SERVING · 1 model` and the
      line under it read *there are none*, because one came from what the server offers and the
      other from Epoch's own shelf. Two readings of one machine, side by side, disagreeing.
    */
    weights = [];
    render(<LocalRuntimes />);
    const name = await screen.findByText("llama.cpp");
    /*
      **Scoped to the row it is about.** LM Studio in this fixture is installed, not serving and
      has nothing of its own, so it says the sentence correctly — and a global assertion would
      read that as the defect. The first version did.
    */
    const row = name.closest("li") ?? name.parentElement?.parentElement;
    expect(row?.textContent ?? "").not.toMatch(/nothing to serve yet/i);
  });

  it("offers the router when the runtime has models of its own", async () => {
    // `llama download` files a model in llama.cpp's own cache, which is neither Ollama's store
    // nor Epoch's shelf — so an empty shelf was never the same question as an empty machine.
    weights = [];
    cached = ["ggml-org/gemma-3-270m-GGUF:Q8_0"];
    render(<LocalRuntimes />);
    expect(await screen.findByText(/1 model of its own/i)).toBeTruthy();
  });

  it("says why llama.cpp has nothing to serve rather than offering nothing", async () => {
    weights = [];
    render(<LocalRuntimes />);
    // Both runtimes that take the whole shelf say it — llama.cpp and LM Studio — because both
    // are equally empty and hiding one of them would be the same silence one row along.
    const said = await screen.findAllByText(/nothing to serve yet/i);
    expect(said.length).toBeGreaterThan(0);
    for (const one of said) {
      /*
        **The deck a model actually arrives from, and the first version named the wrong one.**

        It said `MODELS`, which is where models are *listed* — so on a machine with none it was a
        pointer at an empty room. WORKSHOP is where one is fetched, and it is worth saying which
        is which because the person reading this sentence has neither.
      */
      expect(one.textContent).toMatch(/WORKSHOP/);
      expect(one.textContent).toMatch(/MODELS only lists/);
    }
  });
});
