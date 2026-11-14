import type { WordMeaning } from "../types/translation";

export function WordMeaningTable({ items, highlight, onHover }: { items: WordMeaning[]; highlight?: number | null; onHover?: (i: number | null) => void }) {
  if (!items.length) return <p className="p-3 text-muted">No word meanings yet.</p>;
  return (
    <table className="w-full border-collapse text-left">
      <thead className="sticky top-0 bg-surface text-[11px] uppercase tracking-wide text-muted">
        <tr>
          <th className="px-3 py-2 font-semibold">Nepali</th>
          <th className="px-3 py-2 font-semibold">Meaning here</th>
          <th className="px-3 py-2 font-semibold">Notes</th>
        </tr>
      </thead>
      <tbody>
        {items.map((w, i) => (
          <tr
            key={`${w.source}-${i}`}
            onMouseEnter={() => onHover?.(i)}
            onMouseLeave={() => onHover?.(null)}
            className={`border-t border-line align-top ${highlight === i ? "bg-accent-soft" : ""}`}
          >
            <td className="px-3 py-2" data-selectable>
              <div className="font-medium">{w.source}</div>
              {w.devanagari && <div className="deva text-[13px] text-muted">{w.devanagari}</div>}
            </td>
            <td className="px-3 py-2" data-selectable>
              {w.meaning}
              {w.literal && w.literal !== w.meaning && <div className="text-[12px] text-muted">Literally: {w.literal}</div>}
            </td>
            <td className="px-3 py-2 text-[12px] text-muted">
              {w.uncertain && <span className="mr-1 rounded bg-warn-bg px-1.5 py-0.5 text-[11px] font-medium text-warn-text">Uncertain</span>}
              {w.origin === "dictionary" && <span className="mr-1 rounded bg-surface-2 px-1.5 py-0.5 text-[11px]">Dictionary</span>}
              {w.context}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
