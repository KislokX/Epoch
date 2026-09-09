/**
 * The offer to bring a colleague in — the flow that had no tests because it lived inside a
 * 1,300-line conversation box.
 *
 * Every case is a promise the product makes: the user reads what would travel before agreeing,
 * nothing is handed over until they say so, and "there is nothing new" is said out loud rather
 * than hidden behind a disabled button.
 */

import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { HandoverOffer } from "./HandoverOffer";
import type { CharacterView } from "../../ipc/contracts";

function person(id: string, name: string): CharacterView {
  return {
    id,
    name,
    archetype: "guardian",
    home: "tower",
    place: "tower",
    activity: "reading",
    action: "idle", actions: {}, class: "idle",
    mark: null,
    icon: null,
    speaksWith: null,
    soundsLike: null,
    routine: [],
  };
}

const crew = [person("mage", "Mage"), person("paladin", "Paladin")];

describe("the offer to hand work on", () => {
  it("names who was mentioned, by name rather than by id", () => {
    render(
      <HandoverOffer
        invited={["paladin"]}
        crew={crew}
        preview={async () => "…"}
        onHandOver={() => {}}
        onDecline={() => {}}
      />,
    );

    expect(screen.getByText(/Paladin was mentioned/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "ASK PALADIN" })).toBeInTheDocument();
  });

  it("shows what would travel, and hands nothing over until the user says so", async () => {
    const onHandOver = vi.fn();
    const preview = vi.fn(async () => "The goal: add a section\n- Mage said: done");

    render(
      <HandoverOffer
        invited={["paladin"]}
        crew={crew}
        preview={preview}
        onHandOver={onHandOver}
        onDecline={() => {}}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: "ASK PALADIN" }));

    // Asked the Engine what this person would be given — not composed here, and not summarised.
    expect(preview).toHaveBeenCalledWith("paladin");
    expect(await screen.findByText(/Mage said: done/)).toBeInTheDocument();
    // And still nothing has happened.
    expect(onHandOver).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole("button", { name: "HAND IT OVER" }));
    expect(onHandOver).toHaveBeenCalledWith("paladin");
  });

  it("lets the user read it and still refuse", async () => {
    const onHandOver = vi.fn();
    const onDecline = vi.fn();
    render(
      <HandoverOffer
        invited={["paladin"]}
        crew={crew}
        preview={async () => "something"}
        onHandOver={onHandOver}
        onDecline={onDecline}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: "ASK PALADIN" }));
    await userEvent.click(await screen.findByRole("button", { name: "NO" }));

    expect(onHandOver).not.toHaveBeenCalled();
    // Back to the offer, not gone: saying no to *this* handover is not saying no to the idea.
    expect(screen.getByRole("button", { name: "ASK PALADIN" })).toBeInTheDocument();
  });

  it("says when there is nothing they have not already seen", async () => {
    // Rather than a silently disabled button. "They have seen it all" is information; a control
    // that does nothing and does not say why is not.
    render(
      <HandoverOffer
        invited={["paladin"]}
        crew={crew}
        preview={async () => null}
        onHandOver={() => {}}
        onDecline={() => {}}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: "ASK PALADIN" }));
    expect(await screen.findByText(/has already seen everything here/)).toBeInTheDocument();
    // Still offered, because bringing somebody in with nothing new is a thing the user may
    // legitimately want — it is their call, not ours.
    expect(screen.getByRole("button", { name: "HAND IT OVER" })).toBeInTheDocument();
  });

  it("draws nothing when nobody was named", () => {
    const { container } = render(
      <HandoverOffer
        invited={[]}
        crew={crew}
        preview={async () => "x"}
        onHandOver={() => {}}
        onDecline={() => {}}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });
});
