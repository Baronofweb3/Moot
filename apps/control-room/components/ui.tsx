"use client";
import { useEffect, useRef, useState } from "react";

export function usePoll<T>(fn: () => Promise<T>, deps: unknown[], ms = 0) {
  const [data, setData] = useState<T | undefined>();
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const fnRef = useRef(fn);
  fnRef.current = fn;
  const [tick, setTick] = useState(0);
  useEffect(() => {
    let dead = false;
    const run = () => fnRef.current().then((d) => { if (!dead) { setData(d); setError(null); setLoading(false); } })
      .catch((e) => { if (!dead) { setError(e?.message || String(e)); setLoading(false); } });
    run();
    const id = ms ? setInterval(run, ms) : undefined;
    return () => { dead = true; if (id) clearInterval(id); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, tick]);
  return { data, error, loading, reload: () => setTick((t) => t + 1) };
}

export function useDebounced<T>(v: T, ms = 350) {
  const [d, setD] = useState(v);
  useEffect(() => { const t = setTimeout(() => setD(v), ms); return () => clearTimeout(t); }, [v, ms]);
  return d;
}

export function Notice({ kind, children }: { kind?: "err" | "warn" | "ok"; children: React.ReactNode }) {
  return <div className={`notice ${kind ?? ""}`}>{children}</div>;
}

export function Stat({ label, value, sub }: { label: string; value: React.ReactNode; sub?: string }) {
  return <div className="card stat"><div className="l">{label}</div><div className="v">{value}</div>{sub && <div className="l">{sub}</div>}</div>;
}

export function Row({ k, v }: { k: string; v: React.ReactNode }) {
  return <div className="row"><span className="mute">{k}</span><b>{v}</b></div>;
}

export function Coin({ symbol }: { symbol: string }) {
  return <div className="coin">{symbol.slice(0, 3)}</div>;
}

export function Phase2({ children }: { children?: React.ReactNode }) {
  return <span className="tag">PHASE 2 · NOT BUILT</span>;
}
