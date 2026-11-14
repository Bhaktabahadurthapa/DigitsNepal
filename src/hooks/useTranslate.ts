import { useCallback, useRef, useState } from "react";
import { api } from "../providers/api";
import type { AppError, TranslationMode, TranslationResult } from "../types/translation";

/** Runs translations; stale responses (from an older request) are ignored. */
export function useTranslate() {
  const [result, setResult] = useState<TranslationResult | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [loading, setLoading] = useState(false);
  const seq = useRef(0);

  const run = useCallback(async (text: string, mode: TranslationMode, wordMeanings: boolean, isPrivate = false) => {
    const id = ++seq.current;
    setLoading(true);
    setError(null);
    try {
      const r = await api.translate(text, mode, wordMeanings, isPrivate);
      if (id === seq.current) setResult(r.result);
      return r.result;
    } catch (e) {
      if (id === seq.current) setError(e as AppError);
      return null;
    } finally {
      if (id === seq.current) setLoading(false);
    }
  }, []);

  const reset = useCallback(() => {
    seq.current++;
    setResult(null);
    setError(null);
    setLoading(false);
  }, []);

  return { result, error, loading, run, reset, setResult };
}
