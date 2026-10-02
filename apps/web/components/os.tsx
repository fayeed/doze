import type { ReactNode } from "react";

/** Copy that differs by platform. Both render; CSS shows the one for <html data-platform>. */
export function OS({ mac, win }: { mac: ReactNode; win: ReactNode }) {
  return (
    <>
      <span className="only-mac">{mac}</span>
      <span className="only-win">{win}</span>
    </>
  );
}

export function Logo({ size = 28 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 1024 1024" aria-hidden="true">
      <defs>
        <linearGradient id="logo-ink" x1="0.15" y1="0.1" x2="0.75" y2="1">
          <stop offset="0" stopColor="#8f7cff" />
          <stop offset="1" stopColor="#4a36d1" />
        </linearGradient>
        <mask id="logo-crescent">
          <circle cx="540" cy="520" r="420" fill="#fff" />
          <circle cx="747" cy="369" r="350" fill="#000" />
        </mask>
      </defs>
      <rect width="1024" height="1024" fill="url(#logo-ink)" mask="url(#logo-crescent)" />
    </svg>
  );
}
