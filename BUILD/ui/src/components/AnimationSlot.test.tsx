import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { MarkView } from "../ipc/contracts";
import { AnimationSlot, parse, read } from "./AnimationSlot";

function drawn(): MarkView {
  return {
    role: "visual",
    renderer: "animated_sprite",
    supported: false,
    asset: "data:image/png;base64,iVBORw0KGgo=",
    scale: 1,
    anchor: [0.5, 1],
    shape: [],
    frames: {
      columns: 4,
      rows: 4,
      count: 16,
      milliseconds: 120,
      directions: ["south", "west", "east", "north"],
    },
  };
}

function slot(props: Partial<Parameters<typeof AnimationSlot>[0]> = {}) {
  return (
    <AnimationSlot
      label="Walking"
      hint="On the road."
      mark={null}
      busy={false}
      onChoose={vi.fn().mockResolvedValue(null)}
      onRecut={vi.fn().mockResolvedValue(null)}
      onClear={vi.fn().mockResolvedValue(null)}
      {...props}
    />
  );
}

function type(label: string, value: string) {
  fireEvent.change(screen.getByLabelText(label), { target: { value } });
}

describe("an animation slot", () => {
  it("will not import a sheet until the grid is stated", () => {
    render(slot());
    // The importer is a file input behind its label — there is no dialog to open, because the
    // webview already has one and the frontend never touches a filesystem (ADR-0024).
    const importer = screen.getByLabelText("Import sheet");
    expect(importer).toBeDisabled();

    // A grid with no duration is still not a cut: nothing here may guess how fast somebody
    // drew their animation to run.
    type("Cols", "4");
    type("Rows", "4");
    expect(importer).toBeDisabled();

    type("ms", "120");
    expect(importer).toBeEnabled();
  });

  it("refuses a direction word the World does not know, before anything is uploaded", () => {
    render(slot());
    type("Cols", "4");
    type("Rows", "4");
    type("ms", "120");
    type("Row directions", "south sideways");

    expect(screen.getByLabelText("Import sheet")).toBeDisabled();
  });

  it("offers a re-cut only once the numbers differ from the file", () => {
    const onRecut = vi.fn().mockResolvedValue(null);
    render(slot({ mark: drawn(), onRecut }));

    // The form opens showing what the vault holds, so there is nothing to re-cut yet.
    expect(screen.queryByRole("button", { name: "Re-cut" })).not.toBeInTheDocument();

    type("ms", "150");
    fireEvent.click(screen.getByRole("button", { name: "Re-cut" }));
    expect(onRecut).toHaveBeenCalledWith({
      columns: 4,
      rows: 4,
      count: 16,
      milliseconds: 150,
      directions: ["south", "west", "east", "north"],
    });
  });

  it("will not re-cut to a grid with fewer rows than it has directions", () => {
    // The rows would stop lining up with the words, and somebody would walk south where they
    // should walk east — which the Engine refuses too, one layer down.
    render(slot({ mark: drawn() }));
    type("Rows", "3");
    expect(screen.queryByRole("button", { name: "Re-cut" })).not.toBeInTheDocument();
  });

  it("says the still sprite stands in when nobody has drawn this", () => {
    render(slot());
    expect(
      screen.getByText("Nobody has drawn this. The still sprite stands in."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Clear" })).not.toBeInTheDocument();
  });

  it("says which of the seven things is missing, rather than going quiet", () => {
    // The defect: IMPORT is a <label> over a disabled <input>, so seven different mistakes all
    // looked identical from outside — an ordinary-looking button that does nothing when
    // clicked and explains nothing. A disabled control that cannot explain itself teaches
    // people the feature is broken.
    const empty = { columns: "", rows: "", count: "", milliseconds: "", directions: "" };
    expect(read(empty)).toEqual({ why: "Say how many columns and rows the sheet has." });

    expect(read({ ...empty, columns: "4", rows: "4" })).toEqual({
      why: "Say how long a frame lasts, in milliseconds.",
    });

    expect(
      read({ ...empty, columns: "4", rows: "4", milliseconds: "120", directions: "up" }),
    ).toEqual({ why: '"up" is not a direction — use north, east, south or west.' });

    expect(
      read({
        ...empty,
        columns: "4",
        rows: "2",
        milliseconds: "120",
        directions: "south west east north",
      }),
    ).toEqual({ why: "4 directions for 2 rows — there is one direction per row." });

    expect(
      read({ ...empty, columns: "4", rows: "4", count: "20", milliseconds: "120" }),
    ).toEqual({ why: "20 frames will not fit in a 4×4 grid." });
  });

  it("agrees with itself about which forms are acceptable", () => {
    // `parse` is implemented on `read`, so a validator and a parser cannot drift into
    // disagreeing — which would show up as a button refusing a form nothing objects to.
    const good = {
      columns: "4",
      rows: "4",
      count: "",
      milliseconds: "120",
      directions: "south west east north",
    };
    expect(parse(good)).not.toBeNull();
    expect(read(good)).toHaveProperty("cut");

    const bad = { ...good, rows: "0" };
    expect(parse(bad)).toBeNull();
    expect(read(bad)).toHaveProperty("why");
  });
});
