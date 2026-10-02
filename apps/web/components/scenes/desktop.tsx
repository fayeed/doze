"use client";

import { useId, type CSSProperties, type ReactNode } from "react";
import type { Platform } from "@/lib/platform";

export const STAGE = { width: 960, height: 600 };

/** Where the Doze status item sits: menu bar on macOS, notification area on Windows. */
export const trayIcon = (platform: Platform) =>
  platform === "macos" ? { x: 704, y: 13 } : { x: 752, y: 576 };

/** Doze's status glyph, as drawn by the tray module: a moon, with a badge while awake or counting down. */
export function TrayGlyph({ platform, state }: { platform: Platform; state: 0 | 1 | 2 }) {
  const id = useId();
  const moon = platform === "macos" ? "#1d1d1f" : "#5d5e6b";
  const badge = platform === "macos" ? "#1d1d1f" : state === 2 ? "#eeae4a" : "#9b8bef";
  return (
    <svg className="sc-glyph" viewBox="0 0 32 32" aria-hidden="true">
      <mask id={id}>
        <rect width="32" height="32" fill="#fff" />
        <circle cx="19" cy="10" r="10" fill="#000" />
        {state ? <circle cx="25" cy="25" r="6" fill="#000" /> : null}
      </mask>
      <circle cx="14" cy="15" r="11" fill={moon} mask={`url(#${id})`} />
      {state === 1 ? <circle cx="25" cy="25" r="4" fill={badge} /> : null}
      {state === 2 ? <circle cx="25" cy="25" r="3.1" fill="none" stroke={badge} strokeWidth="1.8" /> : null}
    </svg>
  );
}

const MAC_ICONS: Record<string, ReactNode> = {
  awake: <path d="M8 2.5v1.6M8 11.9v1.6M2.5 8h1.6M11.9 8h1.6M4.1 4.1l1.1 1.1M10.8 10.8l1.1 1.1M4.1 11.9l1.1-1.1M10.8 5.2l1.1-1.1M8 5.4a2.6 2.6 0 1 0 0 5.2 2.6 2.6 0 0 0 0-5.2Z" />,
  timer: <path d="M8 4.2a4.9 4.9 0 1 0 0 9.8 4.9 4.9 0 0 0 0-9.8ZM8 6.6v2.6l1.7 1.1M6.4 2h3.2" />,
  quick: <path d="M3 5h6M12 5h1M3 11h1M7 11h6M10.5 3.5v3M5.5 9.5v3" />,
  settings: <path d="M8 5.8a2.2 2.2 0 1 0 0 4.4 2.2 2.2 0 0 0 0-4.4ZM8 1.8v1.6M8 12.6v1.6M1.8 8h1.6M12.6 8h1.6M3.6 3.6l1.2 1.2M11.2 11.2l1.2 1.2M3.6 12.4l1.2-1.2M11.2 4.8l1.2-1.2" />,
  help: <path d="M8 2.2a5.8 5.8 0 1 0 0 11.6A5.8 5.8 0 0 0 8 2.2ZM6.3 6.4a1.8 1.8 0 1 1 2.6 1.6c-.6.3-.9.7-.9 1.3M8 11.3v.1" />,
  quit: <path d="M8 2v5.5M4.6 4.2a5 5 0 1 0 6.8 0" />,
  stop: <path d="M4.5 4.5h7v7h-7z" />,
  add: <path d="M8 3.5v9M3.5 8h9" />,
};
const WIN_ICONS: Record<string, string> = {
  awake: "",
  timer: "",
  quick: "",
  settings: "",
  help: "",
  quit: "",
  stop: "",
  add: "",
};

export type MenuItem =
  | { label: string; icon?: string; sub?: boolean; check?: boolean; disabled?: boolean }
  | "-";

const METRICS = {
  macos: { pad: 5, item: 22, separator: 11 },
  windows: { pad: 4, item: 28, separator: 9 },
};

export function menuHeight(platform: Platform, items: MenuItem[]) {
  const m = METRICS[platform];
  return items.reduce((sum, item) => sum + (item === "-" ? m.separator : m.item), m.pad * 2);
}

/** Vertical center of an item, measured from the top of its menu. */
export function itemCenter(platform: Platform, items: MenuItem[], label: string) {
  const m = METRICS[platform];
  let y = m.pad;
  for (const item of items) {
    if (item !== "-" && item.label === label) return y + m.item / 2;
    y += item === "-" ? m.separator : m.item;
  }
  return y;
}

/** The top edge a submenu uses so its first item lines up with its parent row. */
export const submenuTop = (platform: Platform, items: MenuItem[], label: string, menuTop: number) =>
  menuTop + itemCenter(platform, items, label) - METRICS[platform].item / 2 - METRICS[platform].pad;

export function Menu({
  platform,
  items,
  x,
  y,
  width,
  active,
  opacity = 1,
}: {
  platform: Platform;
  items: MenuItem[];
  x: number;
  y: number;
  width: number;
  active?: string | null;
  opacity?: number;
}) {
  if (opacity <= 0) return null;
  return (
    <div className="sc-menu" style={{ left: x, top: y, width, opacity }}>
      {items.map((item, index) =>
        item === "-" ? (
          <div key={index} className="sc-separator" />
        ) : (
          <div
            key={item.label}
            className={`sc-item${item.disabled ? " disabled" : ""}${active === item.label ? " active" : ""}`}
          >
            <span className="sc-check">{item.check ? "✓" : ""}</span>
            {item.icon ? (
              platform === "macos" ? (
                <svg className="sc-icon" viewBox="0 0 16 16" aria-hidden="true">
                  {MAC_ICONS[item.icon]}
                </svg>
              ) : (
                <span className="sc-icon">{WIN_ICONS[item.icon]}</span>
              )
            ) : (
              <span className="sc-icon" />
            )}
            <span className="sc-label">{item.label}</span>
            {item.sub ? <span className="sc-chevron">{platform === "macos" ? "›" : ""}</span> : null}
          </div>
        ),
      )}
    </div>
  );
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
  trayState = 0,
  trayTitle,
  frontApp = "Finder",
  dim = 0,
}: {
  platform: Platform;
  children?: ReactNode;
  trayState?: 0 | 1 | 2;
  trayTitle?: string | null;
  frontApp?: string;
  dim?: number;
}) {
  return (
    <div className={`sc-stage sc-${platform}`} style={{ width: STAGE.width, height: STAGE.height }}>
      <div className="sc-wallpaper" />
      {dim > 0 ? <div className="sc-dim" style={{ opacity: dim }} /> : null}
      {platform === "macos" ? (
        <div className="sc-menubar">
          <strong>{frontApp}</strong>
          <span>File</span>
          <span>Edit</span>
          <span>View</span>
          <span>Window</span>
          <span>Help</span>
          <div className="sc-status" style={{ left: trayIcon("macos").x - 11 }}>
            <TrayGlyph platform="macos" state={trayState} />
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
            <span style={{ background: "linear-gradient(135deg,#8f7cff,#4a36d1)" }} />
          </div>
          <span className="sc-fluent sc-chevron-up">{""}</span>
          <div className="sc-tray" style={{ left: trayIcon("windows").x - 14 }}>
            <TrayGlyph platform="windows" state={trayState} />
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
          <svg className="sc-countdown-icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M14.5 15.5A7 7 0 0 1 7.6 5.2 7.5 7.5 0 1 0 17.8 15a7 7 0 0 1-3.3.5Z" />
            <path d="M14 3.5h4l-4 4.5h4M19 9h2.6L19 12h2.6" />
          </svg>
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
        <TrayGlyph platform={platform} state={0} />
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
