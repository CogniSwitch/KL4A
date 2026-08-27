import type { ReactNode } from 'react';

// Small, dependency-free Markdown -> JSX renderer, ported from
// `integration/oss-launch`'s `web/src/lib/markdown.tsx`. Built because
// `AgentEntry.answer` (§4.6) is free-text from an LLM provider, and
// rule-evaluation answers frequently come back as GFM pipe tables — plain
// text rendering collapsed those into an unreadable single line. Covers just
// the subset actually seen in practice (headings, horizontal rules, bullet
// lists, bold, paragraphs, GFM pipe tables). Intentionally NOT a
// general-purpose Markdown engine, and intentionally has zero external
// dependencies — keep it that way.
//
// Styling differs from the oss-launch original: that version emitted bare
// tags styled by a separate stylesheet (`.answer-markdown` CSS). This
// frontend has no such per-component CSS (everything is Tailwind utility
// classes applied at the call site — see `src/index.css`), so the classes
// live on the elements themselves here instead.

// A GFM pipe-table row: starts and ends with "|", at least one "|" between.
const TABLE_ROW = /^\|(.+)\|$/;
// The header-separator row: only "-", ":", "|", and whitespace - e.g.
// "|---|---|" or "| :-- | --: |". Distinguishes a real table's second line
// from a data row that merely happens to look table-ish.
const TABLE_SEPARATOR_ROW = /^\|[\s:-]+(\|[\s:-]+)+\|$/;

function splitTableRow(line: string): string[] {
  const match = TABLE_ROW.exec(line.trim());
  if (!match) return [];
  return match[1].split('|').map((cell) => cell.trim());
}

function renderInline(text: string, keyPrefix: string): ReactNode[] {
  const parts = text.split(/(\*\*[^*]+\*\*)/g);
  return parts
    .filter((part) => part.length > 0)
    .map((part, index) => {
      if (part.startsWith('**') && part.endsWith('**')) {
        return (
          <strong key={`${keyPrefix}-b-${index}`} className="font-semibold text-ink">
            {part.slice(2, -2)}
          </strong>
        );
      }
      return <span key={`${keyPrefix}-t-${index}`}>{part}</span>;
    });
}

export function renderMarkdown(markdown: string): ReactNode[] {
  const lines = markdown.split('\n');
  const blocks: ReactNode[] = [];
  let listBuffer: string[] = [];
  let blockIndex = 0;

  const flushList = () => {
    if (listBuffer.length === 0) return;
    const items = listBuffer;
    listBuffer = [];
    blockIndex += 1;
    blocks.push(
      <ul key={`ul-${blockIndex}`} className="list-disc space-y-0.5 pl-5">
        {items.map((item, i) => (
          <li key={i}>{renderInline(item, `li-${blockIndex}-${i}`)}</li>
        ))}
      </ul>,
    );
  };

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i].trimEnd();
    const trimmed = line.trim();

    if (trimmed === '') {
      flushList();
      continue;
    }
    if (trimmed === '---') {
      flushList();
      blockIndex += 1;
      blocks.push(<hr key={`hr-${blockIndex}`} className="border-line" />);
      continue;
    }
    // A table is a header row immediately followed by a separator row
    // (e.g. "|---|---|") - checking the next line before committing avoids
    // misreading an ordinary line that merely starts and ends with "|".
    if (TABLE_ROW.test(trimmed) && i + 1 < lines.length && TABLE_SEPARATOR_ROW.test(lines[i + 1].trim())) {
      flushList();
      const headerCells = splitTableRow(trimmed);
      const bodyRows: string[][] = [];
      let cursor = i + 2; // skip the header row and the separator row
      while (cursor < lines.length && TABLE_ROW.test(lines[cursor].trim())) {
        bodyRows.push(splitTableRow(lines[cursor].trim()));
        cursor += 1;
      }
      blockIndex += 1;
      const tableKey = blockIndex;
      blocks.push(
        <div className="overflow-x-auto" key={`table-wrap-${tableKey}`}>
          <table className="w-full min-w-max border-collapse text-xs" key={`table-${tableKey}`}>
            <thead>
              <tr>
                {headerCells.map((cell, index) => (
                  <th
                    key={`th-${tableKey}-${index}`}
                    className="border border-line bg-panel-muted px-2 py-1 text-left font-semibold text-ink"
                  >
                    {renderInline(cell, `th-${tableKey}-${index}`)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {bodyRows.map((row, rowIndex) => (
                <tr key={`tr-${tableKey}-${rowIndex}`}>
                  {row.map((cell, cellIndex) => (
                    <td key={`td-${tableKey}-${rowIndex}-${cellIndex}`} className="border border-line px-2 py-1 align-top text-ink/80">
                      {renderInline(cell, `td-${tableKey}-${rowIndex}-${cellIndex}`)}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>,
      );
      i = cursor - 1; // outer loop's i += 1 lands on the first line after the table
      continue;
    }
    if (trimmed.startsWith('#### ')) {
      flushList();
      blockIndex += 1;
      blocks.push(
        <h4 key={`h4-${blockIndex}`} className="text-xs font-semibold text-ink">
          {renderInline(trimmed.slice(5), `h4-${blockIndex}`)}
        </h4>,
      );
      continue;
    }
    if (trimmed.startsWith('### ')) {
      flushList();
      blockIndex += 1;
      blocks.push(
        <h3 key={`h3-${blockIndex}`} className="text-sm font-semibold text-ink">
          {renderInline(trimmed.slice(4), `h3-${blockIndex}`)}
        </h3>,
      );
      continue;
    }
    if (trimmed.startsWith('## ')) {
      flushList();
      blockIndex += 1;
      blocks.push(
        <h2 key={`h2-${blockIndex}`} className="text-sm font-semibold text-ink">
          {renderInline(trimmed.slice(3), `h2-${blockIndex}`)}
        </h2>,
      );
      continue;
    }
    if (trimmed.startsWith('- ')) {
      listBuffer.push(trimmed.slice(2));
      continue;
    }

    flushList();
    blockIndex += 1;
    blocks.push(
      <p key={`p-${blockIndex}`} className="text-ink/80">
        {renderInline(trimmed, `p-${blockIndex}`)}
      </p>,
    );
  }
  flushList();

  return blocks;
}
