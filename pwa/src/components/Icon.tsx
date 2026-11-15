const P: Record<string, string> = {
  gear: "M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Zm7.4-2.1.1-1.4-.1-1.4 2-1.6-2-3.4-2.4 1a7.6 7.6 0 0 0-2.4-1.4L14.2 2h-4l-.4 2.6a7.6 7.6 0 0 0-2.4 1.4l-2.4-1-2 3.4 2 1.6-.1 1.4.1 1.4-2 1.6 2 3.4 2.4-1a7.6 7.6 0 0 0 2.4 1.4l.4 2.6h4l.4-2.6a7.6 7.6 0 0 0 2.4-1.4l2.4 1 2-3.4-2-1.6Z",
  copy: "M8 8V5a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-3M5 8h9a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-9a2 2 0 0 1 2-2Z",
  share: "M12 3v12M7.5 7.5 12 3l4.5 4.5M5 12v7a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-7",
  check: "m5 12.5 4.5 4.5L19 7.5",
  paste: "M9 4h6v3H9zM8 5.5H6a2 2 0 0 0-2 2V19a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5a2 2 0 0 0-2-2h-2",
  x: "M6 6l12 12M18 6 6 18",
  clock: "M12 7v5l3 2M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z",
  lock: "M7 11V8a5 5 0 0 1 10 0v3M6 11h12a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1v-8a1 1 0 0 1 1-1Z",
  alert: "M12 9v4M12 17h.01M10.3 3.9 2.4 18a2 2 0 0 0 1.7 3h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z",
  arrow: "M5 12h14M13 6l6 6-6 6",
  wifi: "M2 2l20 20M8.5 16.5a5 5 0 0 1 7 0M5 13a10 10 0 0 1 5.2-2.8M19 13a10 10 0 0 0-2.6-1.9M2 8.8a15 15 0 0 1 4.2-2.7M22 8.8A15 15 0 0 0 11.2 5M12 20h.01",
};

export function Icon({ name, size = 20 }: { name: keyof typeof P | string; size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={P[name] ?? ""} />
    </svg>
  );
}

export function Pennant({ size = 26 }: { size?: number }) {
  return (
    <svg width={size * 0.78} height={size} viewBox="0 0 100 128" aria-hidden="true" className="pennant">
      <path d="M4 4 L92 62 H38 L92 124 H4 Z" fill="var(--crimson)" stroke="var(--blue)" strokeWidth="7" strokeLinejoin="miter" />
      <path d="M18 46 a12 12 0 0 0 24 0 a12 9 0 0 1 -24 0z" fill="var(--paper)" />
      <circle cx="29" cy="96" r="9" fill="var(--paper)" />
    </svg>
  );
}
