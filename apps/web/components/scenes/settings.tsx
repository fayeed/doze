"use client";

import type { ReactNode } from "react";
import type { Platform } from "@/lib/platform";
import { AppIcon, Glyph } from "./desktop";
import { Icon, Switch, type IconName } from "./panel";

/*
 * The redesigned Settings window on its Overview page, mirroring
 * apps/desktop/native/macos/Settings.swift and apps/desktop/native/windows/ControlCenter.cs.
 */

const NAV_PATHS = {
  overview: "M12.5 9.8A5 5 0 1 1 6.2 3.5a4 4 0 0 0 6.3 6.3Z",
  home: "M2.5 7.5L8 3l5.5 4.5V13H9.5V9.5h-3V13h-4Z",
  notifications: "M4 11V7.5a4 4 0 0 1 8 0V11l1 1.2H3L4 11Z M6.5 13.6a1.6 1.6 0 0 0 3 0",
  guide: "M2 3.5c2-.8 4-.8 6 .6v9.4c-2-1.4-4-1.4-6-.6Z M14 3.5c-2-.8-4-.8-6 .6v9.4c2-1.4 4-1.4 6-.6Z",
};

type NavItem = { label: string; icon: IconName | keyof typeof NAV_PATHS; mac: string; win: string; group?: "gap" | "bottom" };

const NAV: NavItem[] = [
  { label: "Overview", icon: "overview", mac: "#6C5CE7", win: "#1b1b1b" },
  { label: "General", icon: "gear", mac: "#8E8E93", win: "#1b1b1b" },
  { label: "Session defaults", icon: "sun", mac: "#FF9F0A", win: "#C75100" },
  { label: "After playback", icon: "speaker", mac: "#FF453A", win: "#C2185B" },
  { label: "Notifications", icon: "notifications", mac: "#FF375F", win: "#C42B1C" },
  { label: "Agents", icon: "people", mac: "#30B0C7", win: "#00787A", group: "gap" },
  { label: "Advanced", icon: "sliders", mac: "#636366", win: "#1b1b1b" },
  { label: "Menu guide", icon: "guide", mac: "#FF9F0A", win: "#C75100", group: "bottom" },
  { label: "About Doze", icon: "info", mac: "#0A84FF", win: "#005FB8" },
];

function NavIcon({ item }: { item: NavItem }) {
  if (item.icon in NAV_PATHS) {
    return (
      <svg className="sc-ic" viewBox="0 0 16 16" aria-hidden="true">
        <path d={NAV_PATHS[item.icon as keyof typeof NAV_PATHS]} />
      </svg>
    );
  }
  return <Icon name={item.icon as IconName} />;
}

const PRESETS = ["15m", "30m", "1h", "2h"];

export type OverviewState = {
  left: string;
  until: string;
  audio: number;
  /** Which control the pointer is pressing, if any. */
  active?: string | null;
};

const hot = (id: string, active?: string | null) => (active === id ? " hot" : "");

function MacSettings({ state }: { state: OverviewState }) {
  const row = (title: ReactNode, control: ReactNode, sub?: string) => (
    <div className="sc-s-row">
      <div className="sc-p-grow">
        <span>{title}</span>
        {sub ? <span className="sc-s-sub">{sub}</span> : null}
      </div>
      {control}
    </div>
  );
  return (
    <div className="sc-s-window" style={{ left: 100, top: 46, width: 760, height: 494 }}>
      <nav className="sc-s-side">
        <span className="sc-lights">
          <i />
          <i />
          <i />
        </span>
        <div className="sc-s-search">
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d="M7 2.5a4.5 4.5 0 1 0 0 9a4.5 4.5 0 1 0 0-9Z M10.4 10.4l3.1 3.1" />
          </svg>
          Search
        </div>
        <div className="sc-s-me">
          <AppIcon platform="macos" size={34} />
          <div className="sc-p-grow">
            <strong>Doze</strong>
            <span className="sc-s-sub">Keeping awake</span>
          </div>
        </div>
        {NAV.map((item, index) => (
          <div
            key={item.label}
            className={`sc-s-nav${index === 0 ? " on" : ""}${item.group ? " gap" : ""}`}
          >
            <span className="sc-s-tile" style={{ background: item.mac }}>
              <NavIcon item={item} />
            </span>
            {item.label}
          </div>
        ))}
      </nav>
      <main className="sc-s-main">
        <div className="sc-s-title">Overview</div>
        <div className="sc-s-body">
          <div className="sc-s-grp sc-s-status">
            <span className="sc-p-statustile awake big">
              <Glyph state="awake" />
            </span>
            <div className="sc-p-grow">
              <strong>Keeping awake</strong>
              <span>
                {state.left} left · until {state.until}
              </span>
            </div>
          </div>
          <div className="sc-s-gh">Keep Awake</div>
          <div className="sc-s-grp">
            {row(
              "Keeping awake",
              <span className="sc-s-controls">
                <span className={`sc-s-btn${hot("extend", state.active)}`}>Extend 15 minutes</span>
                <span className="sc-s-btn">Stop</span>
              </span>,
              `${state.left} left`,
            )}
            {row("Keep awake while audio plays", <Switch on={state.audio} />, "Holds your Mac awake while sound is playing.")}
          </div>
          <div className="sc-s-gh">Power Timer</div>
          <div className="sc-s-grp">
            {row(
              "Action",
              <span className="sc-p-acc">
                Sleep
                <i>
                  <svg viewBox="0 0 8 8" aria-hidden="true">
                    <path d="M2 3L4 1.5 6 3M2 5l2 1.5L6 5" />
                  </svg>
                </i>
              </span>,
            )}
            <div className="sc-s-row">
              <span className="sc-s-lab">Start</span>
              <div className="sc-p-chips">
                {PRESETS.map((preset) => (
                  <span key={preset} className="sc-p-chip">
                    {preset}
                  </span>
                ))}
              </div>
            </div>
            {row("Sleep after playback stops", <Switch on={0} />, "Turn on before you start watching; it turns off after it runs.")}
          </div>
        </div>
      </main>
    </div>
  );
}

function WinSettings({ state }: { state: OverviewState }) {
  const card = (icon: IconName, color: string, title: string, sub: string, control: ReactNode) => (
    <div className="sc-s-card">
      <span className="sc-s-cardicon" style={{ color }}>
        <Icon name={icon} />
      </span>
      <div className="sc-p-grow">
        <span>{title}</span>
        <span className="sc-s-sub">{sub}</span>
      </div>
      {control}
    </div>
  );
  return (
    <div className="sc-s-window" style={{ left: 90, top: 22, width: 780, height: 514 }}>
      <div className="sc-s-tt">
        <span>
          <svg viewBox="0 0 48 48" aria-hidden="true">
            <defs>
              <linearGradient id="sc-s-sun" x1="0" y1="4" x2="0" y2="44" gradientUnits="userSpaceOnUse">
                <stop offset="0" stopColor="#F6B25E" />
                <stop offset="1" stopColor="#DE5F5A" />
              </linearGradient>
            </defs>
            <path d="M4.64 29A20 20 0 1 1 43.36 29Z M41.32 34A20 20 0 0 1 6.68 34Z" fill="url(#sc-s-sun)" />
          </svg>
          Doze Settings
        </span>
        <span className="sc-s-caps">
          <span>–</span>
          <span>□</span>
          <span>✕</span>
        </span>
      </div>
      <div className="sc-s-cols">
        <nav className="sc-s-nav-col">
          <div className="sc-s-search">
            Find a setting
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <path d="M7 2.5a4.5 4.5 0 1 0 0 9a4.5 4.5 0 1 0 0-9Z M10.4 10.4l3.1 3.1" />
            </svg>
          </div>
          {NAV.map((item, index) => (
            <div
              key={item.label}
              className={`sc-s-nav${index === 0 ? " on" : ""}${index === 1 || item.group === "gap" ? " sep" : ""}${item.group === "bottom" ? " bottom" : ""}`}
              style={{ color: item.win }}
            >
              <NavIcon item={index === 0 ? { ...item, icon: "home" } : item} />
              <span>{item.label}</span>
            </div>
          ))}
        </nav>
        <main className="sc-s-main">
          <h2>Overview</h2>
          <div className="sc-s-card sc-s-status">
            <span className="sc-s-statustile">
              <Glyph state="awake" />
            </span>
            <div className="sc-p-grow">
              <strong>Keeping awake</strong>
              <span className="sc-s-sub">{state.left} left</span>
            </div>
          </div>
          <div className="sc-s-sh">Keep Awake</div>
          {card(
            "sun",
            "#C75100",
            "Keeping awake",
            `${state.left} left · ends ${state.until}`,
            <span className="sc-s-controls">
              <span className={`sc-s-btn${hot("extend", state.active)}`}>Extend 15 minutes</span>
              <span className="sc-s-btn">Stop</span>
            </span>,
          )}
          {card(
            "speaker",
            "#C2185B",
            "Keep awake while audio plays",
            "Holds the computer awake while sound is playing.",
            <span className="sc-s-toggle">
              {state.audio > 0.5 ? "On" : "Off"}
              <Switch on={state.audio} />
            </span>,
          )}
          <div className="sc-s-sh">Power Timer</div>
          {card(
            "power",
            "#1b1b1b",
            "Action",
            "Used by the timer presets here and in the tray menu.",
            <span className="sc-f-combo wide">Sleep</span>,
          )}
        </main>
      </div>
    </div>
  );
}

export function SettingsWindow({ platform, state }: { platform: Platform; state: OverviewState }) {
  return platform === "macos" ? <MacSettings state={state} /> : <WinSettings state={state} />;
}
