/**
 * MISSIONS — every conversation this World has had.
 *
 * ## Why a window rather than a panel
 *
 * The sidebar holds instruments: things you glance at while working. History is not that. You
 * come to it looking for one thing among many, you read, and you leave — so it takes the screen
 * for as long as that takes and then gives it back. The same reason the Chronicle is a window
 * and the crew list is not.
 *
 * ## Why pages rather than one long scroll
 *
 * A Quest per conversation adds up quickly, and a list nobody can find the bottom of is a list
 * nobody reads. Fifteen at a time, numbered — the position is *addressable*, so "it was on page
 * three" is a thing somebody can remember and act on. An infinite scroll cannot be pointed at.
 *
 * ## What it may never do
 *
 * Hide a failure. Ended Quests are kept as what they were — Abandoned is Abandoned, Failed is
 * Failed (ADR-0025 §7) — and this is the surface where that promise is either honoured or
 * quietly broken.
 */

import { useState } from "react";

import { Frame, Plate } from "../components/hud/Frame";
import { Overlay } from "../components/Overlay";
import { Pager, pageOf, slice } from "../components/Pager";
import type { QuestSummary } from "../ipc/world";

/** How many fit on one page before the list stops being readable. */
const PER_PAGE = 15;

interface MissionsProps {
  readonly all: readonly QuestSummary[];
  /** Look at one. The Chronicle — and, for an agent, its session — come back with it. */
  readonly onOpen: (quest: string) => void;
  readonly onClose: () => void;
}

export function Missions({ all, onOpen, onClose }: MissionsProps) {
  const [page, setPage] = useState(0);

  // The shared bar, which this window had a fourth copy of. `Pager`'s own note says three copies
  // is where page bars start disagreeing about the last page; this was the fourth, and it had
  // already drifted — clamping written twice, in two places, for the same rule.
  const at = pageOf(page, all.length, PER_PAGE);
  const shown = slice(all, at, PER_PAGE);

  return (
    // Escape, the scrim, the focus and the pointer-events rule all come from `Overlay` — this
    // window used to own none of them, and the one it forgot made its ✕ unclickable.
    <Overlay label="Missions" onClose={onClose}>
      <Frame className="missions" corner={10} studs fill="var(--ep-window)">
        <div className="missions__head">
          <Plate>MISSIONS</Plate>
          <span className="missions__count">
            {all.length === 1
              ? "1 conversation"
              : `${all.length} conversations`}
          </span>
          <button
            type="button"
            className="dlg__close"
            onClick={onClose}
            aria-label="Close"
            title="Put this away"
          >
            ✕
          </button>
        </div>

        {all.length === 0 ? (
          <p className="hud__empty missions__none">
            Nothing yet. Every conversation this World has will be listed here —
            the ones that worked and the ones that did not.
          </p>
        ) : (
          <ol className="missions__list">
            {shown.map((chat) => (
              <li key={chat.id}>
                <button
                  type="button"
                  className={`missions__row${chat.current ? " missions__row--on" : ""}`}
                  onClick={() => onOpen(chat.id)}
                  title={chat.title}
                >
                  <b>{chat.title}</b>
                  <span className="missions__facts">
                    {/*
                      What it was, said plainly. A closed Quest reads `abandoned`, not "archived"
                      — the record is not softened on the way to the screen.
                    */}
                    <i className={chat.open ? "on" : undefined}>
                      {chat.open
                        ? "OPEN"
                        : chat.state.replace(/_/g, " ").toUpperCase()}
                    </i>
                    <i>{chat.saidCount} said</i>
                    <i>{chat.producedEvidence ? "evidence" : "no evidence"}</i>
                  </span>
                </button>
              </li>
            ))}
          </ol>
        )}

        <Pager
          count={all.length}
          perPage={PER_PAGE}
          at={at}
          onGo={setPage}
          label="Pages"
        />
      </Frame>
    </Overlay>
  );
}
