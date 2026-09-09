/**
 * The last thing standing when a render throws.
 *
 * ## Why this exists
 *
 * A projection sent `reversal` as `{"kind":"permanent"}` instead of a sentence, a component
 * rendered that object as a child, React unmounted the entire tree, and Epoch became a black
 * rectangle. One field with the wrong shape took the whole World with it.
 *
 * That is not a bug worth fixing once. Every projection is a chance to make the same mistake,
 * and the failure mode — a window with nothing in it and no way to find out why — is the worst
 * one available: it looks identical to a crash, a hang and a blank screen.
 *
 * ## What it does instead
 *
 * Says what broke, in the World's own voice, and offers the way back. `Build From Life` rule 1
 * is that the World always renders; this is the floor that claim stands on.
 *
 * Deliberately not a retry loop: a render error is almost always deterministic, and retrying
 * would flicker rather than recover. Reloading is the honest offer.
 */

import { Component } from "react";
import type { ErrorInfo, ReactNode } from "react";

interface SalvageProps {
  readonly children: ReactNode;
}

interface SalvageState {
  readonly failure: Error | null;
}

export class Salvage extends Component<SalvageProps, SalvageState> {
  state: SalvageState = { failure: null };

  static getDerivedStateFromError(failure: Error): SalvageState {
    return { failure };
  }

  componentDidCatch(failure: Error, info: ErrorInfo) {
    // The stack is the only thing that identifies which projection was wrong. Kept in the
    // console rather than on screen: the user needs the way out, we need the trace.
    console.error("Epoch could not draw this:", failure, info.componentStack);
  }

  render() {
    if (!this.state.failure) return this.props.children;

    return (
      <div className="salvage" role="alert">
        <h1>THE VIEW BROKE</h1>
        <p>
          Something Epoch was handed could not be drawn. The Engine is unaffected — nothing you
          did was lost, and nothing ran that should not have.
        </p>
        <p className="salvage__why">{this.state.failure.message}</p>
        <button type="button" className="btn" onClick={() => window.location.reload()}>
          RELOAD
        </button>
      </div>
    );
  }
}
