/**
 * The form that makes somebody, and everything it refuses to decide for them.
 *
 * The rule under test is ADR-0023: **characters belong to the user.** A first draft of the
 * install design offered "start with a crew of four" — four pre-made people copied in on
 * request — and that is the same act as generating a face for somebody who has not chosen one,
 * one step earlier. These assertions are what keep the form from drifting back toward it.
 */

import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";

import { Hire } from "./Hire";

const ARCHETYPES = ["researcher", "coordinator", "guardian", "historian"] as const;

/** Answer the two commands this form uses, and nothing else. */
function engine(made: string | Error = "mage") {
  vi.mocked(invoke).mockImplementation((command: string) => {
    if (command === "suggested_prompt") return Promise.resolve("You explore before committing.");
    if (command === "hire_character") {
      return made instanceof Error ? Promise.reject(made) : Promise.resolve(made);
    }
    return new Promise<never>(() => {});
  });
}

describe("Hire", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("asks for a name and nothing else that decides who somebody is", () => {
    engine();
    render(<Hire archetypes={ARCHETYPES} onHired={() => {}} onCancel={() => {}} />);

    expect(screen.getByText("Name")).toBeInTheDocument();
    expect(screen.getByText("Archetype")).toBeInTheDocument();

    // None of these belong here. A model would surprise somebody with a cost they did not
    // choose; a portrait would be a face they did not pick; a home is decided in the World
    // Editor by whoever knows which building is which (ADR-0028).
    expect(screen.queryByText(/^Model$/)).not.toBeInTheDocument();
    expect(screen.queryByText(/^Brain$/)).not.toBeInTheDocument();
    expect(screen.queryByText(/^Sprite$/)).not.toBeInTheDocument();
    expect(screen.queryByText(/^Home$/)).not.toBeInTheDocument();
    expect(screen.queryByText(/capabilit/i)).not.toBeInTheDocument();
  });

  it("will not make somebody with no name", async () => {
    engine();
    render(<Hire archetypes={ARCHETYPES} onHired={() => {}} onCancel={() => {}} />);

    // Every character is somebody. A blank name is the one thing the form itself refuses.
    expect(screen.getByRole("button", { name: "MAKE THEM" })).toBeDisabled();

    // And whitespace is blank. Somebody who typed two spaces has not named anybody.
    await userEvent.type(screen.getByPlaceholderText("Who are they?"), "   ");
    expect(screen.getByRole("button", { name: "MAKE THEM" })).toBeDisabled();

    await userEvent.type(screen.getByPlaceholderText("Who are they?"), "Mage");
    expect(screen.getByRole("button", { name: "MAKE THEM" })).toBeEnabled();
  });

  it("offers the archetype's words rather than writing them behind the user", async () => {
    engine();
    render(<Hire archetypes={ARCHETYPES} onHired={() => {}} onCancel={() => {}} />);

    // Nothing is suggested until asked for: the field starts empty.
    const prompt = screen.getByPlaceholderText(/How do they work/);
    expect(prompt).toHaveValue("");

    await userEvent.click(screen.getByRole("button", { name: /suggested prompt/i }));
    await waitFor(() => expect(prompt).toHaveValue("You explore before committing."));
  });

  it("never overwrites words the user wrote", async () => {
    engine();
    render(<Hire archetypes={ARCHETYPES} onHired={() => {}} onCancel={() => {}} />);

    const prompt = screen.getByPlaceholderText(/How do they work/);
    await userEvent.type(prompt, "Mine.");

    // Changing the archetype offers a suggestion — into an empty field only. Replacing what
    // somebody wrote because they touched a dropdown would be the form deciding it knew better.
    await userEvent.selectOptions(screen.getByRole("combobox"), "guardian");
    await waitFor(() => expect(prompt).toHaveValue("Mine."));
  });

  it("hands back who was made, by identity", async () => {
    engine("mage-the-second");
    const onHired = vi.fn();
    render(<Hire archetypes={ARCHETYPES} onHired={onHired} onCancel={() => {}} />);

    await userEvent.type(screen.getByPlaceholderText("Who are they?"), "Mage the Second");
    await userEvent.click(screen.getByRole("button", { name: "MAKE THEM" }));

    // The id the Engine chose, never the name the user typed: two people may share a name.
    await waitFor(() => expect(onHired).toHaveBeenCalledWith("mage-the-second"));
  });

  it("says why when the Engine refuses, instead of appearing to work", async () => {
    engine(new Error("that name has no letters or digits in it"));
    const onHired = vi.fn();
    render(<Hire archetypes={ARCHETYPES} onHired={onHired} onCancel={() => {}} />);

    await userEvent.type(screen.getByPlaceholderText("Who are they?"), "!!!");
    await userEvent.click(screen.getByRole("button", { name: "MAKE THEM" }));

    await waitFor(() => expect(screen.getByText(/no letters or digits/)).toBeInTheDocument());
    expect(onHired).not.toHaveBeenCalled();
  });
});
