import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ConnectedModels } from "./ConnectedModels";

describe("connected model readings", () => {
  it("keeps the instrument honest when no provider was reachable", () => {
    render(<ConnectedModels providers={[]} agents={[]} />);

    expect(screen.getByText("Nothing can be reached from here yet.")).toBeInTheDocument();
    expect(screen.getByText("Models").closest(".hud__card--dormant")).toBeInTheDocument();
  });

  it("distinguishes an offline provider from a reachable provider with no installed model", () => {
    render(
      <ConnectedModels
        agents={[]}
        providers={[
          {
            id: "ollama",
            name: "Ollama",
            machine: null,
            endpoint: "http://localhost:11434",
            local: true,
            online: false,
            models: [],
            note: "not reachable",
          },
          {
            id: "local",
            name: "Local",
            machine: null,
            endpoint: "http://localhost:11435",
            local: true,
            online: true,
            models: [],
            note: null,
          },
        ]}
      />,
    );

    expect(screen.getByText("Ollama").parentElement).toHaveTextContent("OFFLINE");
    expect(screen.getByText("Local").parentElement).toHaveTextContent("NO MODELS");
  });

  it("shows only the model names the reachable provider actually returned", () => {
    render(
      <ConnectedModels
        agents={[]}
        providers={[
          {
            id: "ollama",
            name: "Ollama",
            machine: null,
            endpoint: "http://localhost:11434",
            local: true,
            online: true,
            models: ["qwen3:14b", "gemma4:12b"],
            note: null,
          },
        ]}
      />,
    );

    expect(screen.getByText("qwen3:14b").parentElement).toHaveTextContent("READY");
    expect(screen.getByText("gemma4:12b").parentElement).toHaveTextContent("READY");
  });

  it("keeps account agents visible beside provider models", () => {
    render(
      <ConnectedModels
        providers={[]}
        agents={[
          { id: "claude-code", kind: "claude-code", name: "Claude Code", installed: true, version: "2", lookedIn: null, note: null, signedIn: true, account: null },
          { id: "codex", kind: "codex", name: "Codex", installed: true, version: "1", lookedIn: null, note: null, signedIn: false, account: null },
        ]}
      />,
    );

    expect(screen.getByText("Models")).toBeInTheDocument();
    expect(screen.getByText("Claude Code").parentElement).toHaveTextContent("READY");
    expect(screen.getByText("Codex").parentElement).toHaveTextContent("SIGN IN");
  });
});
