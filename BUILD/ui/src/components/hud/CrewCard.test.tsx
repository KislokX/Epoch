import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { CrewCard } from "./CrewCard";

const props = {
  glyph: "potion" as const,
  name: "Mage",
  brain: "gemma4:12b",
  activity: "IDLE" as const,
  active: false,
  onClick: vi.fn(),
};

describe("the crew card", () => {
  it("does not invent an allowance bar for a local model", () => {
    render(<CrewCard {...props} allowance={null} context={null} />);

    expect(screen.queryByText("ALLOWANCE")).not.toBeInTheDocument();
    expect(screen.getByLabelText("CONTEXT: —")).toBeInTheDocument();
  });

  it("reads context the same way the gauge above the composer does", () => {
    render(
      <CrewCard
        {...props}
        allowance={{ remainingPercent: 44, hint: "Measured by Codex." }}
        context={{ used: 250, budget: 1000 }}
      />,
    );

    expect(screen.getByLabelText("ALLOWANCE: 44%")).toBeInTheDocument();
    expect(screen.getByLabelText("CONTEXT: 25%")).toBeInTheDocument();
  });

  it("gives context one look, whether or not the agent reports a ceiling", () => {
    // `.epbar__fill` takes only its first colour from `--bar` and keeps a generic purple second
    // stripe, so a crew member whose agent reports a ceiling drew a different bar from one whose
    // agent does not - same instrument, same conversation, two colour schemes. The tone class is
    // what gives both the cyan pair.
    const measured = render(<CrewCard {...props} allowance={null} context={{ used: 250, budget: 1000 }} />);
    expect(
      measured.container.querySelector(".hud__crew-meter-track--context"),
    ).toBeInTheDocument();
    measured.unmount();

    const open = render(<CrewCard {...props} allowance={null} context={{ used: 98_380, budget: 0 }} />);
    expect(open.container.querySelector(".hud__crew-meter-track--context")).toBeInTheDocument();
  });

  it("shows measured agent context without pretending an unknown ceiling is a percentage", () => {
    const { container } = render(
      <CrewCard
        {...props}
        allowance={{ remainingPercent: 69, hint: "Measured by Codex." }}
        context={{ used: 98_380, budget: 0 }}
      />,
    );

    expect(screen.getByLabelText("ALLOWANCE: 69%")).toBeInTheDocument();
    expect(screen.getByLabelText("CONTEXT: 98,380 used")).toBeInTheDocument();
    expect(container.querySelector(".epbar__fill--indeterminate")).toBeInTheDocument();
  });

  it("names what thinks for them, and says so when nothing does", () => {
    // Two crew cards look identical and may be thinking with entirely different things. The
    // card is also where somebody notices a character cannot work at all — before asking.
    const named = render(
      <CrewCard {...props} allowance={null} context={null} />,
    );
    expect(named.getByText("gemma4:12b")).toBeInTheDocument();
    named.unmount();

    const empty = render(
      <CrewCard {...props} brain={null} allowance={null} context={null} />,
    );
    expect(empty.getByText("no brain")).toBeInTheDocument();
  });

  it("names the model that actually thought, not the one that was asked for", () => {
    // A measured Gemini run opened with `"model":"auto"` and thought with a flash model. The
    // card is where somebody notices that the brain they assigned is not what answered.
    render(
      <CrewCard
        {...props}
        brain="Gemini CLI"
        thoughtWith="gemini-3-flash-preview"
        allowance={null}
        context={null}
      />,
    );

    expect(screen.getByText("thought with gemini-3-flash-preview")).toBeInTheDocument();
  });

  it("says nothing when the agent never reported which model answered", () => {
    // A cold instrument beats an invented reading: most agents report no such thing, and
    // falling back to the requested name would be the card stating something it was not told.
    render(<CrewCard {...props} brain="Claude Code" allowance={null} context={null} />);

    expect(screen.queryByText(/thought with/)).not.toBeInTheDocument();
  });

  it("does not repeat the brain back as an observation", () => {
    // An instrument that never moves is one people stop reading.
    render(
      <CrewCard
        {...props}
        brain="gemma4:12b"
        thoughtWith="gemma4:12b"
        allowance={null}
        context={null}
      />,
    );

    expect(screen.queryByText(/thought with/)).not.toBeInTheDocument();
  });

  it("names which sign-in a character speaks with", () => {
    // Reported by the owner: two characters on two Claude Code accounts drew identical cards,
    // and the only way to find out which was which was to ask the character — whose answer
    // about its own sign-in is a self-report, not a measurement.
    render(
      <CrewCard
        {...props}
        brain="haiku"
        account="Claude Code 2"
        allowance={null}
        context={null}
      />,
    );

    expect(screen.getByText("· Claude Code 2")).toBeInTheDocument();
  });

  it("says nothing about a sign-in when there is only one", () => {
    // Naming the only account on the machine is an instrument that never moves.
    render(
      <CrewCard
        {...props}
        brain="haiku"
        account={null}
        allowance={null}
        context={null}
      />,
    );

    expect(screen.queryByText(/Claude Code/)).not.toBeInTheDocument();
  });
});
