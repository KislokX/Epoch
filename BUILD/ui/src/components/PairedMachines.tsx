import { useEffect, useState } from "react";

import type { DisclosureRow, PairedMachine } from "../ipc/contracts";
import {
  answerDisclosure,
  fetchBridges,
  fetchDisclosures,
  enrolMachine,
  setBridgeGrants,
  unpairMachine,
} from "../ipc/launcher";

/**
 * The machines this World runs on, and what each of them may be (ADR-0029).
 *
 * ## Named for what it lists, not for where it sits
 *
 * It was `BridgeConsole`, which is also the name of the Ship's Log panel in `Instruments` — two
 * components, one identifier, and the collision only surfaced when both ended up imported into
 * the Launcher at once. A name that describes the contents survives being moved; a name that
 * describes the location does not.
 *
 * ## Two grants, shown as two sentences
 *
 * They are not sizes of the same thing:
 *
 * - **may think for me** — a Bridge. It contributes computation and touches nothing of the World.
 * - **may drive my World** — a person, at that machine, acting through this one. Every capability
 *   still runs here and is still judged by this machine's Trust.
 *
 * Separate, because if one grant bought both then a compromised compute machine would have the
 * user's terminal. A machine with neither is a legitimate state: paired, allowed nothing, still
 * known — which is what taking both back leaves behind.
 *
 * ## The Host reaches out; the other machine shows the code
 *
 * It ran the other way first — this Host showed a code and the remote dialled in, so nobody
 * typed an address. Better, and it needs the remote to be able to *start* a connection here. On
 * a real network that was not true: with a Mac on Wi-Fi and this machine on Ethernet, every
 * connection the Mac started was dropped (445, 135, 3389 and 11501 alike, with the firewall rule
 * present and a listener confirmed up) while every connection this machine started succeeded.
 * Client isolation, and it belongs to a router that is not always somebody's to change.
 *
 * So there is one direction now, and it is the one that works on both kinds of network. The
 * address is the price: a machine that cannot call you cannot tell you where it is.
 *
 * ## The code is an introduction, never a credential
 *
 * Six characters, five minutes, drawn from an alphabet with no `0`, `O`, `1`, `I` or `L` because
 * somebody is reading it off one screen and typing it into another. It is generated and checked
 * on the machine that shows it — now the remote — and it buys a long secret that is then spent.
 *
 * ## Where a turn goes is the user's decision
 *
 * Not a warning banner: a question with an answer that is remembered. Asked once per
 * destination, and taken back here — because *a machine you own* and *somebody else's service*
 * are genuinely different decisions, and a nag that covers both trains people to dismiss it.
 */
export function PairedMachines({ onChanged }: { readonly onChanged?: () => void } = {}) {
  const [machines, setMachines] = useState<readonly PairedMachine[]>([]);
  const [going, setGoing] = useState<readonly DisclosureRow[]>([]);
  const [typed, setTyped] = useState("");
  const [address, setAddress] = useState("");
  const [grants, setGrants] = useState<readonly string[]>(["compute"]);
  const [said, setSaid] = useState<string | null>(null);

  const read = () => {
    void fetchBridges().then(setMachines);
    void fetchDisclosures().then(setGoing);
  };

  useEffect(read, []);

  /**
   * Reach out to a machine that is showing a code.
   *
   * This used to be `pairMachine`, which filed a roster entry and minted a secret **here** —
   * so the machine on the other end never learned it and refused every request afterwards. It
   * looked like adding a machine and produced one that could not be used. `enrolMachine`
   * performs the exchange, so both ends end up holding the same bearer.
   */
  const pair = async () => {
    const failure = await enrolMachine(address, typed, grants);
    setSaid(failure);
    if (failure) return;
    setTyped("");
    setAddress("");
    read();
    /*
      **A machine that has just been paired is a machine nobody has asked anything yet.**

      The exchange files the roster entry and the bearer; what that machine is *serving* — its
      Ollama, its LM Studio, its llama.cpp — is a separate question, and until somebody asks it
      the Services list has one entry where there should be three. Making the person press
      PROBE AGAIN to find that out is making them do the computer's job, which is the same
      reasoning that stops them ever typing an IP.
    */
    onChanged?.();
  };

  const toggle = (id: string, machine: PairedMachine, grant: string) => {
    const next = machine.grants.includes(grant)
      ? machine.grants.filter((g) => g !== grant)
      : [...machine.grants, grant];
    void setBridgeGrants(id, next).then((failure) => {
      setSaid(failure);
      read();
      // Taking `compute` away removes that machine's Providers; giving it back adds them. Both
      // are changes to what can think, and neither should wait for a button.
      onChanged?.();
    });
  };

  return (
    <div className="brg">
      <div className="brg__head">
        <span className="rm__label">Machines</span>
        <span className="cc__hint">
          {machines.length === 0
            ? "Only this one."
            : `${machines.length} paired`}
        </span>
      </div>

      {machines.map((machine) => (
        <div key={machine.id} className="brg__row">
          <div>
            <b>{machine.name || machine.id}</b>
            <i>{machine.address}</i>
          </div>
          <div className="brg__grants">
            {(
              [
                ["compute", "may think for me"],
                ["surface", "may drive my World from there"],
              ] as const
            ).map(([grant, says]) => (
              <label key={grant}>
                <input
                  type="checkbox"
                  checked={machine.grants.includes(grant)}
                  onChange={() => toggle(machine.id, machine, grant)}
                />
                <span>{says}</span>
              </label>
            ))}
          </div>
          <button
            type="button"
            className="btn btn--mini"
            onClick={() =>
              void unpairMachine(machine.id).then((failure) => {
                setSaid(failure);
                read();
                // Its Providers are gone from the roster; a list still offering them would be
                // offering a machine Epoch no longer holds a key for.
                onChanged?.();
              })
            }
          >
            UNPAIR
          </button>
        </div>
      ))}

      <div className="brg__pair">
        <span className="rm__label">Pair a machine</span>
        <p className="cc__hint">
          Open <b>EpochServices</b> on the other machine and press{" "}
          <b>SHOW A CODE</b>. Type it here with that machine&apos;s address,
          which is shown right under the code.
        </p>
        <div className="brg__form">
          <input
            type="text"
            value={typed}
            placeholder="code"
            onChange={(e) => setTyped(e.target.value)}
          />
          {/*
            **A shape, not an instance.** The placeholder here was the address of the machine
            this was written on, so every user was shown somebody else's computer as the
            example — the one occurrence of a developer's own network that reached shipped code
            rather than a test.
          */}
          <input
            type="text"
            value={address}
            placeholder="192.168.1.20"
            onChange={(e) => setAddress(e.target.value)}
          />
        </div>
        <div className="brg__grants">
          {(
            [
              ["compute", "may think for me"],
              ["surface", "may drive my World from there"],
            ] as const
          ).map(([grant, says]) => (
            <label key={grant}>
              <input
                type="checkbox"
                checked={grants.includes(grant)}
                onChange={() =>
                  setGrants(
                    grants.includes(grant)
                      ? grants.filter((g) => g !== grant)
                      : [...grants, grant],
                  )
                }
              />
              <span>{says}</span>
            </label>
          ))}
        </div>
        <button
          type="button"
          className="btn btn--mini"
          disabled={!typed.trim() || !address.trim()}
          onClick={() => void pair()}
        >
          PAIR
        </button>
        {said && <p className="notice notice--warn">{said}</p>}
      </div>

      <div className="brg__going">
        <span className="rm__label">Where a turn may go</span>
        {going.map((row) => (
          <div key={row.going} className="brg__ask">
            <p>{row.asks}</p>
            <div className="art__actions">
              <button
                type="button"
                className={`btn btn--mini${row.allowed === true ? " rm__go" : ""}`}
                onClick={() =>
                  void answerDisclosure(row.going, true).then(read)
                }
              >
                ALLOW
              </button>
              <button
                type="button"
                className={`btn btn--mini${row.allowed === false ? " rm__go" : ""}`}
                onClick={() =>
                  void answerDisclosure(row.going, false).then(read)
                }
              >
                REFUSE
              </button>
              {row.allowed !== null && (
                <button
                  type="button"
                  className="btn btn--mini"
                  onClick={() =>
                    void answerDisclosure(row.going, null).then(read)
                  }
                >
                  ASK ME AGAIN
                </button>
              )}
              {/* Unasked is neither, and the row says so rather than looking answered. */}
              <span className="cc__hint">
                {row.allowed === null
                  ? "not answered"
                  : row.allowed
                    ? "allowed"
                    : "refused"}
              </span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
