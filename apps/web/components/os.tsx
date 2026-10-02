import Image from "next/image";
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

/** The setting-sun mark; the simplified drawing keeps its stripes legible at small sizes. */
export function Mark({ size = 28 }: { size?: number }) {
  const file = size <= 32 ? "doze-icon-windows-16-32.svg" : "doze-icon-windows.svg";
  return <Image src={`/brand/${file}`} alt="" width={size} height={size} unoptimized />;
}

export function Wordmark({ height = 32 }: { height?: number }) {
  return (
    <Image
      src="/brand/doze-wordmark.svg"
      alt="Doze"
      width={(height * 160) / 48}
      height={height}
      style={{ width: "auto", height }}
      unoptimized
      priority
    />
  );
}
