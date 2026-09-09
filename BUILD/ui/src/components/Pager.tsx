/**
 * A page bar, and the one rule that makes it worth having.
 *
 * ## Why pages rather than a longer list
 *
 * A card that grows with its contents stops being a card: the Ship's Log ran to four screens and
 * pushed the whole bridge out of shape. Cutting the list instead would be worse — it would mean
 * the panel silently stops mentioning things that happened.
 *
 * So the panel keeps its proportions and the rest is *one click away*, and the position is
 * **addressable**: "it was on page three" is a sentence somebody can act on. An infinite scroll
 * cannot be pointed at.
 *
 * ## Shared, because there are three of these now
 *
 * Missions had one, and then the Ship's Log and the Bridge Console wanted the same thing. A third
 * copy is where three page bars start disagreeing about what happens on the last page when the
 * last item is removed.
 */

interface PagerProps {
  /** How many items there are in total. */
  readonly count: number;
  /** How many fit on one page before the panel stops keeping its shape. */
  readonly perPage: number;
  /** Which page is shown, zero-based. Clamped by the caller through {@link pageOf}. */
  readonly at: number;
  readonly onGo: (page: number) => void;
  /** Distinguishes this bar from the others on the same screen, for a screen reader. */
  readonly label: string;
}

/** How many pages `count` items need. Always at least one, so an empty list still has a page. */
export function pages(count: number, perPage: number): number {
  return Math.max(1, Math.ceil(count / perPage));
}

/**
 * The page actually shown.
 *
 * Clamped on read rather than corrected in state: clearing the last entry on page four should show
 * page three, not an empty page four that state has not caught up with yet.
 */
export function pageOf(wanted: number, count: number, perPage: number): number {
  return Math.min(Math.max(0, wanted), pages(count, perPage) - 1);
}

/** The slice for the current page. */
export function slice<T>(all: readonly T[], at: number, perPage: number): readonly T[] {
  const page = pageOf(at, all.length, perPage);
  return all.slice(page * perPage, page * perPage + perPage);
}

export function Pager({ count, perPage, at, onGo, label }: PagerProps) {
  const total = pages(count, perPage);
  // One page is not a choice, and a bar with a single button is furniture.
  if (total < 2) return null;
  const page = pageOf(at, count, perPage);

  return (
    <nav className="pager" aria-label={label}>
      <button
        type="button"
        className="pager__page"
        disabled={page === 0}
        onClick={() => onGo(page - 1)}
        aria-label="Previous page"
      >
        ‹
      </button>
      {Array.from({ length: total }, (_, i) => (
        <button
          key={i}
          type="button"
          className={`pager__page${i === page ? " pager__page--on" : ""}`}
          onClick={() => onGo(i)}
          aria-current={i === page ? "page" : undefined}
        >
          {i + 1}
        </button>
      ))}
      <button
        type="button"
        className="pager__page"
        disabled={page === total - 1}
        onClick={() => onGo(page + 1)}
        aria-label="Next page"
      >
        ›
      </button>
    </nav>
  );
}
