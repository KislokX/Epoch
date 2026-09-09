/** Providers the Engine actually probed, rendered as World readings. */

import type { ProviderStatus } from "../../ipc/contracts";
import { agentLink } from "../../experience/agentLink";
import type { AgentStatus } from "../../ipc/launcher";
import { SidebarCard, SidebarRow } from "./SidebarCard";

interface ConnectedModelsProps {
  readonly providers: readonly ProviderStatus[];
  /** Account agents are separate from providers: they are local signed-in programs, not endpoints. */
  readonly agents: readonly AgentStatus[];
}

/**
 * A configured backend is not necessarily reachable and an online backend need not have a model.
 * Those are three different observations from the Engine, so this component never merges them
 * into a reassuring count.
 */
export function ConnectedModels({ providers, agents }: ConnectedModelsProps) {
  const agentRows = agents.map((agent) => {
    // The decision is shared (`agentLink`); only the words are this card's, because a sidebar
    // row has space for one and it should read as an instruction when there is one to give.
    const link = agentLink(agent);
    const right =
      link.state === "absent"
        ? "ABSENT"
        : link.state === "signedOut"
          ? "SIGN IN"
          : link.state === "unknown"
            ? "CHECK"
            : "READY";
    return (
      <SidebarRow
        key={`agent/${agent.id}`}
        glyph="star"
        label={agent.name}
        right={right}
        dim={right !== "READY"}
      />
    );
  });
  const providerRows = providers.flatMap((provider) =>
    provider.online && provider.models.length > 0
      ? provider.models.map((model) => (
          <SidebarRow key={`${provider.id}/${model}`} glyph="gem" label={model} right="READY" />
        ))
      : [
          <SidebarRow
            key={provider.id}
            glyph="gem"
            label={provider.name}
            right={provider.online ? "NO MODELS" : "OFFLINE"}
            dim
          />,
        ],
  );

  return (
    <SidebarCard
      title="Models"
      dormant={agentRows.length === 0 && providers.every((provider) => !provider.online)}
      scrolls
    >
      {agentRows.length === 0 && providerRows.length === 0 ? (
        <p className="hud__empty">Nothing can be reached from here yet.</p>
      ) : (
        <>{agentRows}{providerRows}</>
      )}
    </SidebarCard>
  );
}
