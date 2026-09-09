import userEvent from "@testing-library/user-event";
import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { ImageStudio } from "../ipc/launcher";

const one = (over: Partial<ImageStudio> = {}): ImageStudio => ({
  id: "comfyui",
  name: "ComfyUI",
  installed: false,
  foundAt: null,
  serving: false,
  endpoint: "http://127.0.0.1:8188",
  models: [],
  install: "winget install Comfy.ComfyUI-Desktop",
  start: null,
  startsServer: false,
  firstRun: "It asks where to keep its models and to accept its own licence.",
  ...over,
});

const studios = vi.hoisted(() => ({ value: [] as ImageStudio[] }));

vi.mock("../ipc/launcher", () => ({
  imageStudios: vi.fn(async () => studios.value),
  installStudio: vi.fn(async () => null),
  startStudio: vi.fn(async () => null),
  stopStudio: vi.fn(async () => "Stopped. Its memory is back."),
}));

import { ImageStudios } from "./ImageStudios";

describe("what the picture makers panel says about each state", () => {
  it("offers to install one that is not here, and names the command", async () => {
    studios.value = [one()];
    render(<ImageStudios />);
    await waitFor(() =>
      expect(screen.getByText("NOT INSTALLED")).toBeInTheDocument(),
    );
    expect(screen.getByText("INSTALL COMFYUI")).toBeInTheDocument();
    expect(
      screen.getByText("winget install Comfy.ComfyUI-Desktop"),
    ).toBeInTheDocument();
  });

  it("does not tell somebody to install what they already have", async () => {
    // The failure keeping *installed* and *serving* apart exists to prevent. An application
    // that is closed is not an application that is missing.
    studios.value = [one({ installed: true, foundAt: "C:/Programs/Comfy" })];
    render(<ImageStudios />);
    await waitFor(() =>
      expect(screen.getByText("INSTALLED · NOT RUNNING")).toBeInTheDocument(),
    );
    expect(screen.queryByText("INSTALL COMFYUI")).not.toBeInTheDocument();
    expect(screen.getByText("START COMFYUI")).toBeInTheDocument();
  });

  it("warns about the wizard before the press, not after it", async () => {
    // Epoch does not answer a licence prompt on somebody's behalf, and a START that appears to
    // do nothing is what happens when nobody said the wizard exists.
    studios.value = [one({ installed: true })];
    render(<ImageStudios />);
    await waitFor(() =>
      expect(
        screen.getByText(/asks where to keep its models/),
      ).toBeInTheDocument(),
    );
  });

  it("separates running from able to draw", async () => {
    // A fresh ComfyUI answers and holds nothing. Reporting that as healthy would leave somebody
    // diagnosing a broken server; reporting it as offline would send them to start what is
    // already started.
    studios.value = [one({ installed: true, serving: true })];
    render(<ImageStudios />);
    await waitFor(() =>
      expect(screen.getByText("SERVING")).toBeInTheDocument(),
    );
    expect(
      screen.getByText(/running and cannot make a picture/),
    ).toBeInTheDocument();
  });

  it("lists what it can draw with when it has something", async () => {
    studios.value = [
      one({ installed: true, serving: true, models: ["sd15.safetensors"] }),
    ];
    render(<ImageStudios />);
    await waitFor(() =>
      expect(screen.getByText(/sd15.safetensors/)).toBeInTheDocument(),
    );
  });

  it("says it is asking rather than showing nothing", () => {
    studios.value = [];
    render(<ImageStudios />);
    expect(screen.getByText("Asking this machine…")).toBeInTheDocument();
  });

  it("offers to stop one that is answering, and says what that costs", async () => {
    // Epoch knew how to start one and not how to stop it, which made "it is running and holding
    // memory" something to go and fix in Task Manager. Measured idle: 2.6 GB of system RAM, and
    // asking it to free memory returns none of it.
    studios.value = [one({ installed: true, serving: true })];
    render(<ImageStudios />);
    await waitFor(() =>
      expect(screen.getByText("STOP COMFYUI")).toBeInTheDocument(),
    );
    expect(screen.getByText(/2.6 GB back/)).toBeInTheDocument();

    await userEvent.click(screen.getByText("STOP COMFYUI"));
    const { stopStudio } = await import("../ipc/launcher");
    expect(stopStudio).toHaveBeenCalledWith("comfyui");
    await waitFor(() =>
      expect(screen.getByText(/Its memory is back/)).toBeInTheDocument(),
    );
  });
});
