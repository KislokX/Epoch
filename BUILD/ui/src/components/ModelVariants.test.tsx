import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("../ipc/launcher", () => ({
  fetchModelVariants: vi.fn(async () => [
    {
      bits: 1,
      variants: [
        { quant: "UD-IQ1_S", bytes: 6_190_000_000, files: 1, pull: "hf.co/r:UD-IQ1_S", tag: null , pullable: true },
      ],
    },
    {
      bits: 4,
      variants: [
        { quant: "Q4_0", bytes: 1_370_000_000, files: 1, pull: "hf.co/r:Q4_0", tag: "MTP" , pullable: true },
        { quant: "UD-Q4_K_XL", bytes: 17_560_000_000, files: 1, pull: "hf.co/r:UD-Q4_K_XL", tag: null , pullable: true },
      ],
    },
    {
      bits: 16,
      variants: [
        { quant: "BF16", bytes: 55_590_000_000, files: 3, pull: "hf.co/r:BF16", tag: null , pullable: false },
      ],
    },
  ]),
}));

import { ModelVariants } from "./ModelVariants";

const card = {
  gpu: "RTX 4070 SUPER",
  vramTotal: 12_900_000_000,
  vramFree: 12_900_000_000,
  ramTotal: 33_900_000_000, unified: false,
};

describe("a repository's quantisations", () => {
  it("says which of them this machine can run, which is the whole point", async () => {
    // A name and a size are on Hugging Face's own page. The verdict is what makes this list
    // Epoch's — so it rides on each chip rather than waiting for somebody to pick one.
    render(<ModelVariants repo="unsloth/x-GGUF" machine={card} onChoose={vi.fn()} />);

    const small = await screen.findByTitle(/UD-IQ1_S/);
    expect(small.className).toContain("mvar__quant--true");

    const big = screen.getByTitle(/UD-Q4_K_XL/);
    expect(big.className).toContain("mvar__quant--false");
    expect(big.title).toContain("spill into system memory");
  });

  it("does not describe an Apple machine as if it had a card to spill out of", async () => {
    // Measured on the owner's M2 (2026-08-25): 17.2 GB of unified memory, of which 7.1 free —
    // real numbers, and the GPU reads them directly. There is no second pool. Telling somebody a
    // model "will spill into system memory and run slowly" describes a machine this is not, and
    // it is the wrong advice: on one pool the model does not get slower, it does not load.
    const apple = {
      gpu: "Apple M2",
      vramTotal: 17_179_869_184,
      vramFree: 7_100_000_000,
      ramTotal: 17_179_869_184,
      unified: true,
    };
    render(<ModelVariants repo="unsloth/x-GGUF" machine={apple} onChoose={vi.fn()} />);

    const big = await screen.findByTitle(/UD-Q4_K_XL/);
    expect(big.className).toContain("mvar__quant--false");
    expect(big.title).not.toContain("spill into system memory");
    expect(big.title).toContain("unified memory");
  });

  it("never reads unknown as no", async () => {
    // An Apple machine has no VRAM to compare against. "Nobody can say" is a different answer
    // from "nothing fits", and dimming everything would be the second one.
    render(
      <ModelVariants
        repo="unsloth/x-GGUF"
        machine={{ gpu: null, vramTotal: null, vramFree: null, ramTotal: 17_179_869_184, unified: false }}
        onChoose={vi.fn()}
      />,
    );
    const chip = await screen.findByTitle(/UD-IQ1_S/);
    expect(chip.className).toContain("mvar__quant--unknown");
    expect(chip.title).toContain("nobody can say");
  });

  it("marks the module that is not a version of the model", async () => {
    // 1.37 GB beside a 27B. Untagged, it is the cheapest-looking entry in the 4-bit row.
    render(<ModelVariants repo="unsloth/x-GGUF" machine={card} onChoose={vi.fn()} />);
    await waitFor(() => expect(screen.getByText("MTP")).toBeInTheDocument());
    expect(screen.getByTitle(/Q4_0 — the MTP module/)).toBeInTheDocument();
  });

  it("says when a download arrives in parts", async () => {
    // The number that used to be wrong: a shard on its own was reported as the whole variant.
    render(<ModelVariants repo="unsloth/x-GGUF" machine={card} onChoose={vi.fn()} />);
    expect(await screen.findByText(/3 files/)).toBeInTheDocument();
    // And the second half of the same fact, measured against Ollama's own refusal: it will not
    // fetch a shard set through its registry. A download button beside it could not work.
    expect(screen.getByText(/Ollama cannot pull these/)).toBeInTheDocument();
    expect(screen.getByText("55.6 GB")).toBeInTheDocument();
  });

  it("groups by width, in the order a person reads them", async () => {
    render(<ModelVariants repo="unsloth/x-GGUF" machine={card} onChoose={vi.fn()} />);
    await screen.findByText("1-bit");
    const rows = document.querySelectorAll(".mvar__bits");
    expect([...rows].map((r) => r.textContent)).toEqual(["1-bit", "4-bit", "16-bit"]);
  });
});
