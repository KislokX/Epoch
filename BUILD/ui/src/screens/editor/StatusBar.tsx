/**
 * What the editor knows about itself, along the bottom.
 *
 * Every cell here is measured: the tool that is actually selected, the thing that is actually
 * selected, the pointer's real world coordinates, the camera's real zoom, the Engine's real
 * undo depth, and warnings computed from the World rather than from a list.
 *
 * The warnings deserve their place. Both are real gaps that only authoring can close — a
 * building nothing leads to, and somebody with nowhere to live — and both are reported *here*
 * rather than enforced at the moment work tries to flow through them. Scenery must never be
 * able to veto collaboration (ADR-0028).
 */

import type { CharacterView, PlaceView } from "../../ipc/contracts";
import type { EditorApi } from "./useEditor";

export function StatusBar({
  editor,
  places,
  crew,
  cursor,
  zoom,
  unreachable,
}: {
  readonly editor: EditorApi;
  readonly places: readonly PlaceView[];
  readonly crew: readonly CharacterView[];
  readonly cursor: { readonly x: number; readonly y: number } | null;
  readonly zoom: number;
  readonly unreachable: readonly string[];
}) {
  const selection = editor.selection;
  const label =
    selection.kind === "place"
      ? `PLACE · ${places.find((p) => p.id === selection.id)?.title ?? "?"}`
      : selection.kind === "character"
        ? `CREW · ${crew.find((c) => c.id === selection.id)?.name ?? "?"}`
        : selection.kind === "road"
          ? `ROAD · ${selection.id}`
          : "nothing";

  const warnings: string[] = [];
  if (unreachable.length) {
    warnings.push(`${unreachable.length} place(s) no road leads to`);
  }
  const nowhere = places.filter((p) => !p.placement).length;
  if (nowhere) warnings.push(`${nowhere} place(s) standing nowhere`);
  const homeless = crew.filter((c) => !places.some((p) => p.id === c.home)).length;
  if (homeless) warnings.push(`${homeless} crew with no home here`);

  return (
    <footer className="pxpanel wed__status">
      <Cell k="Tool" v={editor.mode.toUpperCase()} gold />
      <Cell k="Selection" v={label} />
      <Cell
        k="X / Y"
        v={cursor ? `${Math.round(cursor.x)} , ${Math.round(cursor.y)}` : "—"}
      />
      <Cell k="Zoom" v={`${Math.round(zoom * 100)}%`} />
      <Cell k="History" v={`${editor.depth[0]} back · ${editor.history[0] ?? "—"}`} />
      <div className="wed__warnings">
        <span className="pxlabel">Warnings</span>
        {warnings.length === 0 ? (
          <em>all clear</em>
        ) : (
          warnings.map((w) => <b key={w}>⚠ {w}</b>)
        )}
      </div>
    </footer>
  );
}

function Cell({ k, v, gold }: { readonly k: string; readonly v: string; readonly gold?: boolean }) {
  return (
    <div className={`wed__cell${gold ? " wed__cell--gold" : ""}`}>
      <span className="pxlabel">{k}</span>
      <span>{v}</span>
    </div>
  );
}
