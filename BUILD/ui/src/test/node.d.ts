/**
 * The one Node API a test in this suite needs, declared rather than installed.
 *
 * `WorldRail.test.tsx` asserts on the text of `hud.css`, because a stylesheet invariant exists
 * nowhere else — jsdom applies no stylesheet, and Vitest replaces every `.css` import with an
 * empty string, `?raw` included (measured 2026-08-25). So the file is read as a file.
 *
 * Not `@types/node`: pulling Node's whole global surface into a presentation layer would make
 * `process`, `Buffer` and `require` type-check everywhere, in a codebase that has no business
 * touching any of them. One function, named.
 */
declare module "node:fs" {
  export function readFileSync(path: string, encoding: "utf8"): string;
}
