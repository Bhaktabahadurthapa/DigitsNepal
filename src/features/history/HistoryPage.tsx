import { useCallback, useEffect, useState } from "react";
import { Button, CopyButton, Notice } from "../../components/ui";
import { api } from "../../providers/api";
import { MODES, type HistoryEntry, type Settings } from "../../types/translation";

export function HistoryPage({ settings, onReuse, onEnable }: { settings: Settings | null; onReuse: (text: string) => void; onEnable: () => void }) {
  const [query, setQuery] = useState("");
  const [favOnly, setFavOnly] = useState(false);
  const [items, setItems] = useState<HistoryEntry[]>([]);
  const [confirmClear, setConfirmClear] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  const load = useCallback(() => {
    api.historySearch(query, favOnly).then(setItems).catch(() => setItems([]));
  }, [query, favOnly]);
  useEffect(() => {
    const h = setTimeout(load, 150);
    return () => clearTimeout(h);
  }, [load]);

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 p-4">
      {!settings?.history_enabled && (
        <Notice tone="info">
          History is off. New translations are not saved (only items you explicitly Save).{" "}
          <button className="underline" onClick={onEnable}>
            Turn on history
          </button>
        </Notice>
      )}
      <div className="flex items-center gap-2">
        <input
          aria-label="Search history"
          className="h-8 flex-1 rounded-md border border-line bg-surface px-3 outline-none"
          placeholder="Search original or translated text…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <label className="flex items-center gap-1.5 text-[12px] text-muted">
          <input type="checkbox" checked={favOnly} onChange={(e) => setFavOnly(e.target.checked)} className="accent-[var(--accent)]" />
          Favorites
        </label>
        <Button
          onClick={async () => {
            try {
              const p = await api.historyExport();
              setMsg(`Exported to ${p}`);
            } catch (e) {
              setMsg((e as { message: string }).message);
            }
          }}
        >
          Export JSON
        </Button>
        {confirmClear ? (
          <>
            <Button
              variant="danger"
              onClick={async () => {
                await api.historyClear();
                setConfirmClear(false);
                load();
              }}
            >
              Delete all permanently
            </Button>
            <Button variant="ghost" onClick={() => setConfirmClear(false)}>
              Cancel
            </Button>
          </>
        ) : (
          <Button variant="danger" onClick={() => setConfirmClear(true)} disabled={!items.length && !query}>
            Clear all…
          </Button>
        )}
      </div>
      {msg && <div className="text-[12px] text-muted">{msg}</div>}

      <ul className="min-h-0 flex-1 space-y-2 overflow-auto">
        {items.length === 0 && <li className="p-6 text-center text-muted">No saved translations.</li>}
        {items.map((e) => (
          <li key={e.id} className="rounded-lg border border-line bg-surface p-3">
            <div className="mb-1 flex items-center gap-2 text-[11px] text-muted">
              <span>{new Date(e.created_at).toLocaleString()}</span>
              <span>· {MODES.find((m) => m.id === e.mode)?.label}</span>
              <span>· {e.provider}</span>
              <div className="flex-1" />
              <button
                aria-label={e.favorite ? "Remove from favorites" : "Add to favorites"}
                aria-pressed={e.favorite}
                className={e.favorite ? "text-crimson" : "hover:text-ink"}
                onClick={async () => {
                  await api.historyFavorite(e.id, !e.favorite);
                  load();
                }}
              >
                {e.favorite ? "★ Favorite" : "☆ Favorite"}
              </button>
            </div>
            <p data-selectable className="text-muted">{e.original_text}</p>
            <p data-selectable className="mt-1 text-[14px]">{e.accurate_english}</p>
            {e.styled_english && <p data-selectable className="mt-1 text-[13px] text-muted">{e.styled_english}</p>}
            <div className="mt-2 flex gap-1">
              <CopyButton text={e.accurate_english} />
              <Button variant="ghost" onClick={() => onReuse(e.original_text)}>
                Reuse
              </Button>
              <Button
                variant="ghost"
                onClick={async () => {
                  await api.historyDelete(e.id);
                  load();
                }}
              >
                Delete
              </Button>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
