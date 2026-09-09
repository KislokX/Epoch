/**
 * A World, drawn small.
 *
 * The same terrain vocabulary the World itself renders with, at a glance and before entering.
 * Derived from the World's real geography rather than a supplied image, so it can never
 * promise something that is not there — and a World gets one the moment it has a map, with no
 * artwork to commission.
 *
 * A World with no geography shows a plain plate that says so. Never a stock image standing in
 * for a place that does not exist.
 */

import type { WorldPreview as Preview } from "../ipc/contracts";

interface WorldPreviewProps {
  readonly preview: Preview | null;
  /** Used for the accessible label, so the picture is not the only way to read this. */
  readonly name: string;
}

export function WorldPreview({ preview, name }: WorldPreviewProps) {
  if (!preview) {
    return (
      <div className="wp wp--none" role="img" aria-label={`${name} has no geography yet`}>
        <span>no map</span>
      </div>
    );
  }

  return (
    <svg
      className="wp"
      viewBox={`0 0 ${preview.width} ${preview.height}`}
      preserveAspectRatio="xMidYMid slice"
      role="img"
      aria-label={`Map of ${name}`}
    >
      {preview.terrain.map((area, i) => (
        <polygon
          key={`${area.kind}-${i}`}
          className={`terrain terrain--${area.kind}`}
          points={area.points.map(([x, y]) => `${x},${y}`).join(" ")}
        />
      ))}

      {/*
        Roads, drawn wide enough to read when the whole World is a few hundred pixels across.
        Stroke width scales with the World rather than being fixed, so a small World does not
        end up with motorways through it.
      */}
      {preview.routes.map((route, i) => (
        <polyline
          key={i}
          className={`wp__route wp__route--${route.prominence}`}
          points={route.points.map(([x, y]) => `${x},${y}`).join(" ")}
          strokeWidth={preview.width * (route.prominence === "major" ? 0.009 : 0.006)}
        />
      ))}

      {/*
        Places sized by footprint: scale is information here too, not decoration. Drawn as a
        halo and a core so they hold their shape against both grass and water.
      */}
      {preview.places.map(([x, y, footprint], i) => {
        const r = Math.max(preview.width * 0.009, footprint * 0.55);
        return (
          <g key={i}>
            <circle className="wp__halo" cx={x} cy={y} r={r * 1.9} />
            <circle className="wp__place" cx={x} cy={y} r={r} />
          </g>
        );
      })}
    </svg>
  );
}
