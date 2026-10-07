"use client";

import type { CSSProperties, ReactNode } from "react";
import type { Platform } from "@/lib/platform";
import macosWallpaper from "./wallpapers/macos.svg";
import windowsWallpaper from "./wallpapers/windows.svg";

export const STAGE = { width: 960, height: 600 };

const wallpapers: Record<Platform, string> = { macos: macosWallpaper.src, windows: windowsWallpaper.src };

/** Where the Doze status item sits: menu bar on macOS, notification area on Windows. */
export const trayIcon = (platform: Platform) =>
  platform === "macos" ? { x: 704, y: 13 } : { x: 752, y: 576 };

export type GlyphState = "normal" | "awake" | "attention" | "countdown";

/**
 * Doze's status glyph, as drawn by tray::image from the brand glyphs: a ring while normal
 * sleep is allowed, a whole sun while awake, the sun with a dot when an agent waits for
 * approval, and the banded setting sun during the final warning. Drawn in currentColor.
 */
export function Glyph({ state }: { state: GlyphState }) {
  return (
    <svg className="sc-glyph" viewBox="0 0 16 16" aria-hidden="true">
      {state === "normal" ? (
        <circle cx="8" cy="8" r="5.75" fill="none" stroke="currentColor" strokeWidth="1.5" />
      ) : state === "awake" ? (
        <circle cx="8" cy="8" r="6.5" fill="currentColor" />
      ) : state === "attention" ? (
        <>
          <path fill="currentColor" d="M13.45 6.57A6.25 6.25 0 1 1 9.43 2.55A3.6 3.6 0 0 0 13.45 6.57Z" />
          <circle cx="13" cy="3" r="2.1" fill="currentColor" />
        </>
      ) : (
        <path
          fill="currentColor"
          d="M1.68 9.5A6.5 6.5 0 1 1 14.32 9.5Z M2 10.5L14 10.5A6.5 6.5 0 0 1 13.12 12L2.88 12A6.5 6.5 0 0 1 2 10.5Z M12.15 13A6.5 6.5 0 0 1 3.85 13Z"
        />
      )}
    </svg>
  );
}

/** Doze's app icon: the rounded tile on macOS, the freestanding mark on Windows. */
export function AppIcon({ platform, size }: { platform: Platform; size: number }) {
  const file = platform === "macos" ? "doze-icon-macos" : "doze-icon-windows";
  // eslint-disable-next-line @next/next/no-img-element -- recorded scenes, never shipped
  return <img src={`/brand/${file}${size <= 32 ? "-16-32" : ""}.svg`} width={size} height={size} alt="" />;
}

export function Cursor({ platform, x, y, down }: { platform: Platform; x: number; y: number; down?: boolean }) {
  const fill = platform === "macos" ? "#000" : "#fff";
  const stroke = platform === "macos" ? "#fff" : "#000";
  return (
    <svg
      className="sc-cursor"
      viewBox="0 0 24 24"
      style={{ left: x - 4, top: y - 2, transform: down ? "scale(0.88)" : undefined }}
      aria-hidden="true"
    >
      <path
        d="M4.5 2.5v17.2l4.4-4.2 2.9 6.6 3-1.3-2.9-6.5h6.1Z"
        fill={fill}
        stroke={stroke}
        strokeWidth="1.4"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function Desktop({
  platform,
  children,
  trayState = "normal",
  trayTitle,
  trayOpen = false,
  frontApp = "Finder",
  dim = 0,
}: {
  platform: Platform;
  children?: ReactNode;
  trayState?: GlyphState;
  trayTitle?: string | null;
  /** Highlights the icon while its panel or flyout is open. */
  trayOpen?: boolean;
  frontApp?: string;
  dim?: number;
}) {
  return (
    <div className={`sc-stage sc-${platform}`} style={{ width: STAGE.width, height: STAGE.height }}>
      <div className="sc-wallpaper" style={{ backgroundImage: `url(${wallpapers[platform]})` }} />
      {dim > 0 ? <div className="sc-dim" style={{ opacity: dim }} /> : null}
      {platform === "macos" ? (
        <div className="sc-menubar">
          <strong>{frontApp}</strong>
          <span>File</span>
          <span>Edit</span>
          <span>View</span>
          <span>Window</span>
          <span>Help</span>
          <div className={`sc-status${trayOpen ? " open" : ""}`} style={{ left: trayIcon("macos").x - 14 }}>
            <Glyph state={trayState} />
            {trayTitle ? <span className="sc-title">{trayTitle}</span> : null}
          </div>
          <svg className="sc-sys wifi" viewBox="0 0 20 16" aria-hidden="true">
            <path d="M2 6a11.5 11.5 0 0 1 16 0M5 9a7.2 7.2 0 0 1 10 0M8 12a3 3 0 0 1 4 0" />
          </svg>
          <svg className="sc-sys battery" viewBox="0 0 28 14" aria-hidden="true">
            <rect x="1" y="1.5" width="23" height="11" rx="3.2" />
            <rect className="fill" x="3" y="3.5" width="15" height="7" rx="1.6" />
            <path d="M26 5.5v3" />
          </svg>
          <span className="sc-clock">Thu 2 Oct 21:40</span>
        </div>
      ) : (
        <div className="sc-taskbar">
          <div className="sc-search">
            <span className="sc-fluent">{""}</span>Search
          </div>
          <div className="sc-apps">
            <span style={{ background: "linear-gradient(135deg,#ffd36b,#f5a524)" }} />
            <span style={{ background: "linear-gradient(135deg,#6fb3ff,#2b74e8)" }} />
            <span style={{ background: "linear-gradient(135deg,#3b3b44,#16161c)" }} />
            {frontApp === "Doze" ? (
              <i className="sc-app-doze">
                <AppIcon platform="windows" size={24} />
              </i>
            ) : null}
          </div>
          <span className="sc-fluent sc-chevron-up">{""}</span>
          <div className={`sc-tray${trayOpen ? " open" : ""}`} style={{ left: trayIcon("windows").x - 16 }}>
            <Glyph state={trayState} />
          </div>
          <div className="sc-tray-sys">
            <span className="sc-fluent">{""}</span>
            <span className="sc-fluent">{""}</span>
            <span className="sc-fluent">{""}</span>
          </div>
          <div className="sc-clock">
            <span>21:40</span>
            <span>02/10/2026</span>
          </div>
        </div>
      )}
      {children}
    </div>
  );
}

/** An app window with the platform's title bar. */
export function Window({
  platform,
  title,
  x,
  y,
  width,
  height,
  opacity = 1,
  scale = 1,
  className = "",
  dark = false,
  children,
}: {
  platform: Platform;
  title: string;
  x: number;
  y: number;
  width: number;
  height: number;
  opacity?: number;
  scale?: number;
  className?: string;
  dark?: boolean;
  children?: ReactNode;
}) {
  if (opacity <= 0) return null;
  const style: CSSProperties = { left: x, top: y, width, height, opacity, transform: `scale(${scale})` };
  return (
    <div className={`sc-window ${dark ? "dark " : ""}${className}`} style={style}>
      {platform === "macos" ? (
        <div className="sc-titlebar">
          <span className="sc-lights">
            <i />
            <i />
            <i />
          </span>
          <span className="sc-window-title">{title}</span>
        </div>
      ) : (
        <div className="sc-titlebar">
          <span className="sc-window-title">{title}</span>
          <span className="sc-captions">
            <span>{""}</span>
            <span>{""}</span>
            <span>{""}</span>
          </span>
        </div>
      )}
      <div className="sc-window-body">{children}</div>
    </div>
  );
}

/** Doze's final warning: action, live clock, hint and the three choices. */
export function Countdown({
  platform,
  x,
  y,
  remaining,
  action = "Sleep",
  source,
  hint = "Cancel the action or snooze for 15 minutes.",
  active,
  opacity = 1,
  scale = 1,
}: {
  platform: Platform;
  x: number;
  y: number;
  remaining: number;
  action?: string;
  /** What started the warning, shown above the action on macOS. */
  source?: string;
  hint?: string;
  active?: string | null;
  opacity?: number;
  scale?: number;
}) {
  if (opacity <= 0) return null;
  const seconds = Math.max(0, remaining);
  const clock = `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
  const button = (label: string, accent = false) => (
    <span className={`sc-button${accent ? " accent" : ""}${active === label ? " active" : ""}`}>{label}</span>
  );
  return (
    <div className="sc-countdown" style={{ left: x, top: y, opacity, transform: `scale(${scale})` }}>
      {platform === "windows" ? <div className="sc-countdown-title">Doze · Countdown</div> : null}
      <div className="sc-countdown-body">
        {platform === "macos" ? (
          <>
            <span className="sc-countdown-icon">
              <Glyph state="countdown" />
            </span>
            {source ? <p className="sc-countdown-source">{source}</p> : null}
          </>
        ) : null}
        <p className="sc-countdown-action">{action} in</p>
        <p className="sc-countdown-clock">{clock}</p>
        <p className="sc-countdown-hint">{hint}</p>
        {platform === "macos" ? (
          <div className="sc-buttons glass">
            {button("Snooze 15 minutes")}
            {button("Cancel")}
            {button("Stay Awake")}
          </div>
        ) : (
          <>
            <div className="sc-buttons split">
              {button("Snooze 15 minutes")}
              {button("Cancel action", true)}
            </div>
            <div className="sc-buttons">{button("Stay Awake")}</div>
          </>
        )}
      </div>
    </div>
  );
}

/** Banner on macOS (top right), toast on Windows (above the clock). */
export function Notice({ platform, title, body, opacity }: { platform: Platform; title: string; body: string; opacity: number }) {
  if (opacity <= 0) return null;
  const offset = (1 - opacity) * 24;
  return (
    <div
      className="sc-notice"
      style={{ opacity, transform: platform === "macos" ? `translateX(${offset}px)` : `translateY(${offset}px)` }}
    >
      <span className="sc-notice-icon">
        <AppIcon platform={platform} size={32} />
      </span>
      <div>
        <strong>{title}</strong>
        <p>{body}</p>
      </div>
    </div>
  );
}

/** The notification-area tooltip Windows shows when hovering Doze. */
export function Tooltip({ lines, opacity }: { lines: string[]; opacity: number }) {
  if (opacity <= 0) return null;
  return (
    <div className="sc-tooltip" style={{ opacity }}>
      {lines.map((line) => (
        <span key={line}>{line}</span>
      ))}
    </div>
  );
}
