/**
 * A file's size, the way somebody reads one.
 *
 * ## Why this is a module and not a helper at the bottom of a screen
 *
 * It was written twice — once in `FindAssets`, once in `CreationsPanel` — with the same doc
 * comment and **different behaviour at zero**: one answered `size not stated`, the other
 * `1 KB`. Neither was a mistake in isolation, and together they were two answers to one
 * question, which is the state this codebase keeps paying for.
 *
 * ## Decimal, deliberately
 *
 * Every catalogue and every download page says a checkpoint weighs 6.9 GB. Showing 6.5 GiB is
 * the same file and a different number, and the person is comparing against Civitai, not
 * against a disk.
 *
 * ## Two functions, because zero means two things
 *
 * A **measured** zero is a real answer: the file on disk is empty. A **claimed** zero is a
 * source that published no size at all — *unasked*, and printing `0 KB` for it would be the
 * invented reading this codebase keeps deleting. So the difference is in the name of the
 * function rather than in a flag somebody forgets to pass.
 */

function scale(bytes: number): string {
  if (bytes >= 1_000_000_000) return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
  if (bytes >= 1_000_000) return `${Math.round(bytes / 1_000_000)} MB`;
  return `${Math.round(bytes / 1_000)} KB`;
}

/** Bytes Epoch measured. Zero is a real, empty file. */
export function weighMeasured(bytes: number): string {
  return scale(bytes);
}

/** Bytes a catalogue stated. Zero is **no size published**, never an empty file. */
export function weigh(bytes: number): string {
  return bytes === 0 ? "size not stated" : scale(bytes);
}
