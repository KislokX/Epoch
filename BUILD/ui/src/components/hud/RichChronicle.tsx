/**
 * A deliberately small, safe visual vocabulary for Chronicle text.
 *
 * Models still return plain text and the Chronicle keeps that exact text. This component only
 * projects a predictable Markdown subset after the fact; it never accepts HTML, never runs a
 * Mermaid interpreter and never lets a model choose an arbitrary component. That makes the
 * visual vocabulary identical for every provider and keeps the renderer in the presentation
 * layer where it belongs.
 */

import type { ReactNode } from "react";

import { askForTerminal, isShellLanguage, runnableCommand } from "../../experience/askForTerminal";

export interface OpenLink {
  (url: string): void;
}

type Block =
  | { readonly kind: "paragraph"; readonly lines: readonly string[] }
  | { readonly kind: "heading"; readonly depth: number; readonly text: string }
  | { readonly kind: "list"; readonly ordered: boolean; readonly start: number; readonly items: readonly string[] }
  | { readonly kind: "code"; readonly language: string | null; readonly text: string }
  | { readonly kind: "table"; readonly headings: readonly string[]; readonly rows: readonly (readonly string[])[] }
  | { readonly kind: "flow"; readonly rows: readonly FlowRow[] };

interface FlowRow {
  readonly nodes: readonly string[];
  readonly labels: readonly (string | null)[];
}

const HEADING = /^(#{1,6})\s+(.+?)\s*#*\s*$/u;
const UNORDERED = /^\s*[-*+]\s+(.+)$/u;
const ORDERED = /^\s*(\d+)[.)]\s+(.+)$/u;
const FENCE = /^```([^\s`]*)\s*$/u;
const TABLE_DIVIDER = /^:?-{3,}:?$/u;
const FLOW_EDGE = /\s*-->(?:\|([^|]+)\|)?\s*/gu;
const WEB_ADDRESS = /https?:\/\/[^\s<>"'`]+/gu;
/**
 * A link somebody wrote — and deliberately **not** an image.
 *
 * ## Why the leading `!` is excluded
 *
 * `![alt](url)` is Markdown for a picture. Epoch draws pictures in a conversation from
 * `Entry::Produced` — evidence the Engine recorded because something really was made — and a
 * model that writes three characters must never reach the same place. Measured before it was
 * closed: a relative `![it](asuka.png)` already rendered as literal text, but an absolute
 * `![it](https://…)` matched the link half and became a **button** captioned by whatever the
 * model chose. A caption is not consent to open an address.
 *
 * So an image written into a message stays text. Nothing here can promote it.
 */
const MARKDOWN_LINK = /(?<!!)\[([^\]\n]+)\]\((https?:\/\/[^\s)]+)\)/gu;
const BOLD = /\*\*([^*\n]+)\*\*/gu;
const INLINE_CODE = /`([^`\n]+)`/gu;
const TRAILING_PUNCTUATION = /[.,;:]+$/u;

function cells(line: string): string[] {
  const trimmed = line.trim().replace(/^\|/u, "").replace(/\|$/u, "");
  return trimmed.split("|").map((cell) => cell.trim());
}

function isTableDivider(line: string): boolean {
  const row = cells(line);
  return row.length > 0 && row.every((cell) => TABLE_DIVIDER.test(cell));
}

function flowNode(raw: string): string {
  const node = raw.trim().replace(/;$/u, "");
  const named = node.match(/^[\w-]+\s*(?:\[([^\]]*)\]|\{([^}]*)\}|\(([^)]*)\))$/u);
  const label = named?.[1] ?? named?.[2] ?? named?.[3];
  return (label ?? node).replace(/^"|"$/gu, "").trim();
}

function flowRows(source: string): FlowRow[] {
  return source
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith("%%") && !/^(flowchart|graph)\s+/iu.test(line))
    .flatMap((line) => {
      const nodes: string[] = [];
      const labels: (string | null)[] = [];
      let cursor = 0;
      for (const edge of line.matchAll(FLOW_EDGE)) {
        const at = edge.index ?? cursor;
        nodes.push(flowNode(line.slice(cursor, at)));
        labels.push(edge[1]?.trim() || null);
        cursor = at + edge[0].length;
      }
      if (nodes.length === 0) return [];
      nodes.push(flowNode(line.slice(cursor)));
      return nodes.every(Boolean) ? [{ nodes, labels }] : [];
    });
}

function startsBlock(lines: readonly string[], at: number): boolean {
  const line = lines[at] ?? "";
  return (
    !line.trim() ||
    HEADING.test(line) ||
    UNORDERED.test(line) ||
    ORDERED.test(line) ||
    FENCE.test(line) ||
    (line.includes("|") && isTableDivider(lines[at + 1] ?? ""))
  );
}

export function parseChronicle(content: string): readonly Block[] {
  const lines = content.replace(/\r\n?/gu, "\n").split("\n");
  const blocks: Block[] = [];
  let at = 0;

  while (at < lines.length) {
    const line = lines[at] ?? "";
    if (!line.trim()) {
      at += 1;
      continue;
    }

    const fence = line.match(FENCE);
    if (fence) {
      const language = fence[1]?.toLowerCase() || null;
      const body: string[] = [];
      at += 1;
      while (at < lines.length && !FENCE.test(lines[at] ?? "")) {
        body.push(lines[at] ?? "");
        at += 1;
      }
      if (at < lines.length) at += 1;
      const source = body.join("\n");
      const flow = language === "flow" || language === "mermaid" ? flowRows(source) : [];
      blocks.push(flow.length > 0 ? { kind: "flow", rows: flow } : { kind: "code", language, text: source });
      continue;
    }

    const heading = line.match(HEADING);
    if (heading) {
      blocks.push({ kind: "heading", depth: heading[1]?.length ?? 1, text: heading[2] ?? "" });
      at += 1;
      continue;
    }

    if (line.includes("|") && isTableDivider(lines[at + 1] ?? "")) {
      const headings = cells(line);
      const rows: string[][] = [];
      at += 2;
      while (at < lines.length && (lines[at] ?? "").includes("|") && (lines[at] ?? "").trim()) {
        const row = cells(lines[at] ?? "");
        rows.push(headings.map((_, index) => row[index] ?? ""));
        at += 1;
      }
      blocks.push({ kind: "table", headings, rows });
      continue;
    }

    const unordered = line.match(UNORDERED);
    const ordered = line.match(ORDERED);
    if (unordered || ordered) {
      const isOrdered = Boolean(ordered);
      const items: string[] = [];
      const start = Number(ordered?.[1] ?? 1);
      while (at < lines.length) {
        const item = (isOrdered ? (lines[at] ?? "").match(ORDERED) : (lines[at] ?? "").match(UNORDERED));
        if (!item) break;
        items.push(item[isOrdered ? 2 : 1] ?? "");
        at += 1;
      }
      blocks.push({ kind: "list", ordered: isOrdered, start, items });
      continue;
    }

    const paragraph: string[] = [line];
    at += 1;
    while (at < lines.length && !startsBlock(lines, at)) {
      paragraph.push(lines[at] ?? "");
      at += 1;
    }
    blocks.push({ kind: "paragraph", lines: paragraph });
  }

  return blocks;
}

function InlineText({ text, onOpenLink }: { readonly text: string; readonly onOpenLink: OpenLink }) {
  const tokens = [MARKDOWN_LINK, BOLD, INLINE_CODE, WEB_ADDRESS]
    .flatMap((pattern) => [...text.matchAll(pattern)])
    .sort((left, right) => (left.index ?? 0) - (right.index ?? 0));
  const pieces: ReactNode[] = [];
  let cursor = 0;

  for (const token of tokens) {
    const at = token.index ?? cursor;
    const value = token[0];
    if (at < cursor) continue;
    if (at > cursor) pieces.push(text.slice(cursor, at));

    if (value.startsWith("[")) {
      pieces.push(
        <button key={`${at}-${value}`} type="button" className="dlg__inline-link" title={`Open ${token[2]}`} onClick={() => onOpenLink(token[2] ?? "")}>
          {token[1]}
        </button>,
      );
    } else if (value.startsWith("**")) {
      pieces.push(
        <strong key={`${at}-${value}`}>
          <InlineText text={token[1] ?? ""} onOpenLink={onOpenLink} />
        </strong>,
      );
    } else if (value.startsWith("`")) {
      pieces.push(<code key={`${at}-${value}`}>{token[1]}</code>);
    } else {
      const url = value.replace(TRAILING_PUNCTUATION, "");
      const tail = value.slice(url.length);
      pieces.push(
        <button key={`${at}-${url}`} type="button" className="dlg__inline-link" title={`Open ${url}`} onClick={() => onOpenLink(url)}>
          {url}
        </button>,
      );
      if (tail) pieces.push(tail);
    }
    cursor = at + value.length;
  }
  if (cursor < text.length) pieces.push(text.slice(cursor));
  return <>{pieces}</>;
}

/** Inline-only projection for markers whose surrounding element is already text-level. */
export function ChronicleInline({ text, onOpenLink }: { readonly text: string; readonly onOpenLink: OpenLink }) {
  return <InlineText text={text} onOpenLink={onOpenLink} />;
}

export function RichChronicle({ content, onOpenLink }: { readonly content: string; readonly onOpenLink: OpenLink }) {
  return (
    <div className="dlg__rich">
      {parseChronicle(content).map((block, index) => {
        if (block.kind === "heading") {
          const Heading = block.depth === 1 ? "h2" : block.depth === 2 ? "h3" : "h4";
          return <Heading key={index} className="dlg__rich-heading"><InlineText text={block.text} onOpenLink={onOpenLink} /></Heading>;
        }
        if (block.kind === "list") {
          const List = block.ordered ? "ol" : "ul";
          return <List key={index} className="dlg__rich-list" start={block.ordered ? block.start : undefined}>{block.items.map((item, itemIndex) => <li key={itemIndex}><InlineText text={item} onOpenLink={onOpenLink} /></li>)}</List>;
        }
        if (block.kind === "code") {
          /*
            A shell command a character wrote gets a way to reach a terminal — **typed, never
            run.** The command is already fully visible above the button, which is the point:
            what Epoch removes is having to go and find a terminal, not having to decide.

            Only fences claiming to be shell commands, and only single commands. A `json` block
            would otherwise carry a "run this" button that is wrong every time, and a five-line
            script prefilled at a prompt is not a small readable thing anybody can check.
          */
          const runnable = isShellLanguage(block.language) ? runnableCommand(block.text) : null;
          return (
            <pre key={index} className="dlg__rich-code" data-language={block.language ?? undefined}>
              <code>{block.text}</code>
              {runnable && (
                <button
                  type="button"
                  className="dlg__rich-run"
                  title="Opens a terminal with this typed in. You press Enter."
                  onClick={() => askForTerminal({ command: runnable, language: block.language })}
                >
                  OPEN IN TERMINAL
                </button>
              )}
            </pre>
          );
        }
        if (block.kind === "table") {
          return <div key={index} className="dlg__rich-table-wrap"><table className="dlg__rich-table"><thead><tr>{block.headings.map((heading, cell) => <th key={cell}><InlineText text={heading} onOpenLink={onOpenLink} /></th>)}</tr></thead><tbody>{block.rows.map((row, rowIndex) => <tr key={rowIndex}>{row.map((cell, cellIndex) => <td key={cellIndex}><InlineText text={cell} onOpenLink={onOpenLink} /></td>)}</tr>)}</tbody></table></div>;
        }
        if (block.kind === "flow") {
          return (
            <div key={index} className="dlg__flow" role="group" aria-label="Flow diagram">
              {block.rows.map((row, rowIndex) => (
                <div key={rowIndex} className="dlg__flow-row">
                  {row.nodes.map((node, nodeIndex) => (
                    <span key={`${nodeIndex}-${node}`} className="dlg__flow-part">
                      <span className="dlg__flow-node">
                        <InlineText text={node} onOpenLink={onOpenLink} />
                      </span>
                      {nodeIndex < row.labels.length && (
                        <span className="dlg__flow-arrow">
                          {row.labels[nodeIndex] && <em>{row.labels[nodeIndex]}</em>}→
                        </span>
                      )}
                    </span>
                  ))}
                </div>
              ))}
            </div>
          );
        }
        return <p key={index}><InlineText text={block.lines.join(" ")} onOpenLink={onOpenLink} /></p>;
      })}
    </div>
  );
}
