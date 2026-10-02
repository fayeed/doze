"use client";

import type { Platform } from "@/lib/platform";
import {
  Countdown,
  Cursor,
  Desktop,
  Menu,
  Notice,
  Tooltip,
  Window,
  itemCenter,
  menuHeight,
  submenuTop,
  trayIcon,
  type MenuItem,
} from "./desktop";
import { eased, path, pressed, progress, track, useTime, visible, within } from "./timeline";

type Scene = (props: { platform: Platform }) => React.ReactNode;

/* Menus, mirroring apps/desktop/src-tauri/src/tray.rs. */

function rootMenu(status: string, timer: string, audio = false): MenuItem[] {
  return [
    { label: status },
    { label: timer },
    "-",
    { label: "Keep Awake", icon: "awake", sub: true },
    { label: "Keep awake while audio plays", check: audio },
    "-",
    { label: "Power Timer", icon: "timer", sub: true },
    { label: "Sleep after playback stops" },
    { label: "Countdown", icon: "timer", sub: true },
    "-",
    { label: "Agents", sub: true },
    { label: "Quick Settings", icon: "quick", sub: true },
    { label: "Settings…", icon: "settings" },
    { label: "Help & About", icon: "help", sub: true },
    "-",
    { label: "Quit Doze", icon: "quit" },
  ];
}

const keepAwakeMenu: MenuItem[] = [
  { label: "Default (30 minutes)" },
  { label: "15 minutes" },
  { label: "30 minutes" },
  { label: "1 hour" },
  { label: "2 hours" },
  { label: "Indefinitely" },
  { label: "Custom duration…" },
  { label: "Until a specific time…" },
  "-",
  { label: "No timed session to extend", icon: "add", disabled: true },
  { label: "No awake session to stop", icon: "stop", disabled: true },
];

const ROOT_WIDTH = { macos: 262, windows: 292 };

/** Root menu placement: under the status item on macOS, above the tray on Windows. */
function rootPosition(platform: Platform, items: MenuItem[]) {
  const icon = trayIcon(platform);
  if (platform === "macos") return { x: Math.min(icon.x - 16, 960 - ROOT_WIDTH.macos - 6), y: 26 };
  return { x: icon.x - ROOT_WIDTH.windows, y: 552 - 8 - menuHeight(platform, items) };
}

/** Menus fade quickly on Windows and appear almost at once on macOS. */
const menuOpacity = (platform: Platform, t: number, open: number, close: number) =>
  visible(t, open, close, platform === "macos" ? 0.06 : 0.12);

/* Hero: start a two-hour Keep Awake session from the menu, then check the status. */
const Hero: Scene = ({ platform }) => {
  const t = useTime();
  const icon = trayIcon(platform);
  const awake = within(t, 3.35, 7.75);
  const root = rootMenu(awake ? "Keeping awake · 2h 0m left" : "Normal sleep allowed", "No power action scheduled");
  const r = rootPosition(platform, root);
  const keepAwakeY = r.y + itemCenter(platform, root, "Keep Awake");
  const subX = r.x - 232 + 4;
  const subY = submenuTop(platform, root, "Keep Awake", r.y);
  const twoHoursY = subY + itemCenter(platform, keepAwakeMenu, "2 hours");
  const statusY = r.y + itemCenter(platform, root, "Keeping awake · 2h 0m left");
  const cursor = path(t, [
    [0, 480, 330],
    [0.5, 480, 330],
    [1.2, icon.x, icon.y],
    [1.6, icon.x, icon.y],
    [2.1, r.x + 120, keepAwakeY],
    [2.6, subX + 150, keepAwakeY],
    [3.0, subX + 110, twoHoursY],
    [3.4, subX + 110, twoHoursY],
    [3.9, 520, 360],
    [4.2, 520, 360],
    [4.6, icon.x, icon.y],
    [5.0, icon.x, icon.y],
    [5.4, r.x + 140, statusY],
    [6.8, r.x + 140, statusY],
    [7.6, 480, 330],
  ]);
  const firstMenu = menuOpacity(platform, t, 1.38, 3.36);
  const subOpacity = menuOpacity(platform, t, 2.25, 3.36);
  const secondMenu = menuOpacity(platform, t, 4.78, 6.9);
  const rootActive = within(t, 2.0, 3.36) ? "Keep Awake" : within(t, 5.3, 6.9) ? "Keeping awake · 2h 0m left" : null;
  return (
    <Desktop platform={platform} trayState={awake ? 1 : 0} trayTitle={awake ? "2h 0m" : null}>
      <Menu
        platform={platform}
        items={root}
        x={r.x}
        y={r.y}
        width={ROOT_WIDTH[platform]}
        active={rootActive}
        opacity={Math.max(firstMenu, secondMenu)}
      />
      <Menu
        platform={platform}
        items={keepAwakeMenu}
        x={subX}
        y={subY}
        width={232}
        active={within(t, 2.95, 3.36) ? "2 hours" : null}
        opacity={subOpacity}
      />
      <Cursor platform={platform} x={cursor.x} y={cursor.y} down={pressed(t, [1.3, 3.25, 4.7])} />
    </Desktop>
  );
};

/* Keep Awake: Overview in Settings, extend the session and turn on audio. */
const sidebar = [
  { label: "Overview", color: "#5a45e0", glyph: "\uE80F" },
  { label: "General", color: "#8a8a8e", glyph: "\uE713" },
  { label: "Session defaults", color: "#f08c1a", glyph: "\uE706" },
  { label: "After playback", color: "#e5487b", glyph: "\uE995" },
  { label: "Notifications", color: "#e5484d", glyph: "\uEA8F" },
  { label: "Agents", color: "#14a39a", glyph: "\uE716" },
  { label: "Advanced", color: "#8a8a8e", glyph: "\uE9E9" },
  { label: "Menu guide", color: "#a0703f", glyph: "\uE736" },
  { label: "About Doze", color: "#2f7cf6", glyph: "\uE946" },
];

function Switch({ on }: { on: number }) {
  return (
    <span className="sc-switch" style={{ "--on": on } as React.CSSProperties}>
      <i />
    </span>
  );
}

function SettingsWindow({ platform, left, audio }: { platform: Platform; left: string; audio: number }) {
  const t = useTime();
  const isMac = platform === "macos";
  return (
    <Window
      platform={platform}
      title={isMac ? "" : "Doze Settings"}
      x={110}
      y={isMac ? 50 : 34}
      width={740}
      height={isMac ? 480 : 500}
      className="sc-settings"
    >
      <nav className="sc-sidebar">
        {isMac ? <div className="sc-search-field">Search</div> : null}
        {sidebar.map((item, index) => (
          <div key={item.label}>
            {!isMac && (index === 1 || index === 5 || index === 7) ? <div className="sc-nav-separator" /> : null}
            {isMac && (index === 1 || index === 5 || index === 7) ? <div className="sc-nav-gap" /> : null}
            <div className={`sc-nav${index === 0 ? " selected" : ""}`}>
              {isMac ? (
                <span className="sc-nav-tile" style={{ background: item.color }} />
              ) : (
                <span className="sc-fluent sc-nav-glyph" style={{ color: item.color }}>
                  {item.glyph}
                </span>
              )}
              {item.label}
            </div>
          </div>
        ))}
      </nav>
      <div className="sc-content">
        <h2 className="sc-page-title">Overview</h2>
        <div className="sc-card sc-status-card">
          <span className="sc-status-tile">
            <span className="sc-fluent">{"\uE706"}</span>
          </span>
          <div>
            <strong>Keeping awake · {left} left</strong>
            <span>No power action scheduled</span>
          </div>
        </div>
        <h3 className="sc-section">Keep Awake</h3>
        <div className="sc-group">
          <div className="sc-card">
            <div className="sc-card-text">
              <strong>Keeping awake</strong>
              <span>{left} left</span>
            </div>
            <div className="sc-card-controls">
              <span className={`sc-button${within(t, 1.8, 1.95) ? " active" : ""}`}>Extend 15 minutes</span>
              <span className="sc-button">Stop</span>
            </div>
          </div>
          <div className="sc-card">
            <div className="sc-card-text">
              <strong>Keep awake while audio plays</strong>
              <span>Holds the computer awake while sound is playing.</span>
            </div>
            <div className="sc-card-controls">
              {isMac ? null : <span className="sc-switch-label">{audio > 0.5 ? "On" : "Off"}</span>}
              <Switch on={audio} />
            </div>
          </div>
        </div>
        <h3 className="sc-section">Power Timer</h3>
        <div className="sc-group">
          <div className="sc-card">
            <div className="sc-card-text">
              <strong>Action</strong>
              <span>Used by the timer presets here and in the {isMac ? "menu bar" : "tray menu"}.</span>
            </div>
            <div className="sc-card-controls">
              <span className="sc-select">Sleep</span>
            </div>
          </div>
        </div>
      </div>
    </Window>
  );
}

const KeepAwake: Scene = ({ platform }) => {
  const t = useTime();
  const isMac = platform === "macos";
  const extended = within(t, 1.9, 7.7);
  const left = extended ? "1h 57m" : "1h 42m";
  const audio = t < 3.55 ? 0 : t < 7.7 ? eased(t, 3.55, 3.75) : 1 - eased(t, 7.7, 7.9);
  const extend = isMac ? { x: 686, y: 226 } : { x: 672, y: 292 };
  const toggle = isMac ? { x: 794, y: 278 } : { x: 782, y: 362 };
  const cursor = path(t, [
    [0, 640, 520],
    [0.6, 640, 520],
    [1.6, extend.x, extend.y],
    [2.4, extend.x, extend.y],
    [3.3, toggle.x, toggle.y],
    [4.4, toggle.x, toggle.y],
    [5.4, 700, 480],
    [7.2, 700, 480],
    [7.9, 640, 520],
  ]);
  return (
    <Desktop platform={platform} trayState={1} trayTitle={left} frontApp="Doze">
      <SettingsWindow platform={platform} left={left} audio={audio} />
      <Cursor platform={platform} x={cursor.x} y={cursor.y} down={pressed(t, [1.8, 3.5])} />
    </Desktop>
  );
};

/* Power Timer: choose Shut down and an hour, start, then see it in the menu or tooltip. */
const actions = ["Sleep", "Hibernate", "Shut down", "Lock", "Turn display off"];

function TimerWindow({ platform, action, minutes, opacity, listOpen, hover, startDown }: {
  platform: Platform;
  action: string;
  minutes: number;
  opacity: number;
  listOpen: number;
  hover: string | null;
  startDown: boolean;
}) {
  const isMac = platform === "macos";
  const ends = minutes === 60 ? "22:40" : "22:10";
  const presets = isMac ? ["15m", "30m", "1h", "2h", "4h", "8h"] : ["15m", "30m", "1h", "2h", "4h", "8h"];
  return (
    <Window
      platform={platform}
      title="Doze"
      x={250}
      y={isMac ? 92 : 64}
      width={460}
      height={isMac ? 312 : 450}
      opacity={opacity}
      scale={0.97 + 0.03 * opacity}
      className="sc-timer"
    >
      <div className="sc-timer-form">
        <div>
          <h2 className="sc-timer-heading">Power Timer</h2>
          <p className="sc-help">
            {isMac
              ? "A final warning lets you cancel or snooze before the action runs."
              : "A final warning lets you cancel or snooze before the action runs."}
          </p>
        </div>
        {isMac ? (
          <>
            <div className="sc-row">
              <span>Action</span>
              <span className="sc-popup">{action}</span>
            </div>
            <div className="sc-row">
              <span>Duration</span>
              <span className="sc-row-controls">
                <span className="sc-field">{minutes}</span>
                <span className="sc-stepper" />
                <span className="sc-muted">minutes</span>
              </span>
            </div>
          </>
        ) : (
          <>
            <label className="sc-header-field">
              <span>Action</span>
              <span className="sc-combo">{action}</span>
            </label>
            <label className="sc-header-field">
              <span>Duration in minutes</span>
              <span className="sc-number">
                {minutes}
                <span className="sc-fluent">{"\uE70E  \uE70D"}</span>
              </span>
            </label>
          </>
        )}
        <div className="sc-presets">
          {presets.map((preset) => (
            <span key={preset} className={`sc-button small${hover === preset ? " active" : ""}`}>
              {preset}
            </span>
          ))}
        </div>
        <p className="sc-muted sc-ends">
          {isMac ? null : <span className="sc-fluent">{"\uE823"} </span>}
          Ends {ends}
        </p>
      </div>
      {isMac ? (
        <div className="sc-timer-footer">
          <span className="sc-button">Cancel</span>
          <span className={`sc-button accent${startDown ? " active" : ""}`}>Start Timer</span>
        </div>
      ) : (
        <div className="sc-timer-footer">
          <span className={`sc-button accent${startDown ? " active" : ""}`}>Start Timer</span>
          <span className="sc-button">Cancel</span>
        </div>
      )}
      {listOpen > 0 ? (
        <div className={`sc-list ${isMac ? "mac" : "win"}`} style={{ opacity: listOpen }}>
          {actions.map((item) => (
            <div key={item} className={`sc-list-item${hover === item ? " active" : ""}`}>
              <span className="sc-check">{item === action ? "✓" : ""}</span>
              {item}
            </div>
          ))}
        </div>
      ) : null}
    </Window>
  );
}

const PowerTimer: Scene = ({ platform }) => {
  const t = useTime();
  const isMac = platform === "macos";
  const icon = trayIcon(platform);
  const action = within(t, 2.2, 8) ? "Shut down" : "Sleep";
  const minutes = within(t, 3.0, 8) ? 60 : 30;
  const scheduled = within(t, 4.4, 7.7);
  const windowOpacity = Math.max(1 - eased(t, 4.4, 4.6), eased(t, 7.7, 8));
  const control = isMac ? { x: 620, y: 200 } : { x: 470, y: 208 };
  const shutDown = isMac ? { x: 600, y: 258 } : { x: 330, y: 344 };
  const preset = isMac ? { x: 386, y: 275 } : { x: 386, y: 328 };
  const start = isMac ? { x: 630, y: 372 } : { x: 376, y: 472 };
  const root = rootMenu("Keeping awake · timer running", "Shut down in 1h 0m");
  const r = rootPosition(platform, root);
  const statusY = r.y + itemCenter(platform, root, "Shut down in 1h 0m");
  const cursor = path(t, [
    [0, 560, 520],
    [0.4, 560, 520],
    [1.2, control.x, control.y],
    [1.5, control.x, control.y],
    [2.0, shutDown.x, shutDown.y],
    [2.3, shutDown.x, shutDown.y],
    [2.8, preset.x, preset.y],
    [3.1, preset.x, preset.y],
    [3.8, start.x, start.y],
    [4.4, start.x, start.y],
    [5.0, icon.x, icon.y],
    ...(isMac
      ? ([
          [5.4, icon.x, icon.y],
          [5.8, r.x + 140, statusY],
          [7.0, r.x + 140, statusY],
        ] as [number, number, number][])
      : ([[7.0, icon.x, icon.y]] as [number, number, number][])),
    [7.7, 560, 520],
  ]);
  return (
    <Desktop
      platform={platform}
      trayState={scheduled ? 1 : 0}
      trayTitle={scheduled ? "1h 0m" : null}
      frontApp="Doze"
    >
      <TimerWindow
        platform={platform}
        action={action}
        minutes={minutes}
        opacity={windowOpacity}
        listOpen={visible(t, 1.45, 2.25, 0.1)}
        hover={within(t, 1.95, 2.25) ? "Shut down" : within(t, 1.45, 1.95) ? "Sleep" : within(t, 2.75, 3.1) ? "1h" : null}
        startDown={pressed(t, [4.3])}
      />
      {isMac ? (
        <Menu
          platform={platform}
          items={root}
          x={r.x}
          y={r.y}
          width={ROOT_WIDTH[platform]}
          active={within(t, 5.7, 7.1) ? "Shut down in 1h 0m" : null}
          opacity={menuOpacity(platform, t, 5.3, 7.1)}
        />
      ) : (
        <Tooltip lines={["Doze · Keeping awake · timer running", "Shut down in 1h 0m"]} opacity={visible(t, 5.5, 7.1)} />
      )}
      <Cursor platform={platform} x={cursor.x} y={cursor.y} down={pressed(t, [1.4, 2.2, 3.0, 4.3, isMac ? 5.2 : -1])} />
    </Desktop>
  );
};

/* After Playback: the credits end, the computer goes quiet, the final warning begins. */
const credits = [
  "Directed by",
  "Mara Lune",
  "",
  "Written by",
  "Theo Hush & Ada Vesper",
  "",
  "Director of Photography",
  "Noor Dusk",
  "",
  "Music by",
  "The Late Hours",
  "",
  "Edited by",
  "Sol Ramírez",
];

const AfterPlayback: Scene = ({ platform }) => {
  const t = useTime();
  const isMac = platform === "macos";
  const played = 0.955 + 0.045 * progress(t, 0, 3.2);
  const total = 6760;
  const current = Math.floor(played * total);
  const clock = (s: number) => `${Math.floor(s / 3600)}:${String(Math.floor(s / 60) % 60).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
  const ended = t >= 3.2;
  const warning = within(t, 4.2, 7.7);
  const remaining = 300 - Math.floor(Math.max(0, t - 4.2));
  const fade = visible(t, 4.2, 7.7, 0.3);
  return (
    <Desktop
      platform={platform}
      trayState={warning ? 2 : ended ? 0 : 1}
      trayTitle={warning ? `${Math.floor(remaining / 60)}:${String(remaining % 60).padStart(2, "0")}` : null}
      frontApp="Player"
      dim={0.22 * fade}
    >
      <Window
        platform={platform}
        title="Night Train.mp4"
        x={70}
        y={isMac ? 44 : 26}
        width={820}
        height={isMac ? 500 : 510}
        dark
        className="sc-player"
      >
        <div className="sc-film" style={{ opacity: 1 - 0.85 * eased(t, 2.8, 3.4) }}>
          <div className="sc-credits" style={{ transform: `translateY(${-track(t, [[0, 0], [3.2, 260]])}px)` }}>
            {credits.map((line, index) => (
              <p key={index} className={index % 3 === 0 ? "role" : ""}>
                {line || "\u00a0"}
              </p>
            ))}
          </div>
        </div>
        <div className="sc-controls">
          <span className="sc-play">{ended ? "▶" : "❚❚"}</span>
          <span className="sc-time">{clock(current)}</span>
          <span className="sc-progress">
            <i style={{ width: `${played * 100}%` }} />
          </span>
          <span className="sc-time">{clock(total)}</span>
          <span className="sc-meter" aria-hidden="true">
            {[0, 1, 2, 3, 4].map((bar) => (
              <i
                key={bar}
                style={{ height: ended ? 2 : 4 + Math.abs(Math.sin(t * 7 + bar * 1.7)) * 12 * (1 - progress(t, 2.6, 3.2)) }}
              />
            ))}
          </span>
        </div>
      </Window>
      <Countdown
        platform={platform}
        x={isMac ? 300 : 290}
        y={isMac ? 110 : 90}
        remaining={remaining}
        opacity={fade}
        scale={0.96 + 0.04 * fade}
      />
      <Notice
        platform={platform}
        title="Doze · Power countdown"
        body={`Sleep in 5m 0s. Open Doze in the ${isMac ? "menu bar" : "tray"} to cancel or snooze 15 minutes.`}
        opacity={visible(t, 4.4, 7.4, 0.3)}
      />
    </Desktop>
  );
};

/* Final warning: snooze it for 15 minutes. */
const FinalWarning: Scene = ({ platform }) => {
  const t = useTime();
  const isMac = platform === "macos";
  const snoozed = within(t, 3.0, 7.5);
  // The warning counts down from 0:30, freezes as it fades on Snooze, and the snoozed
  // countdown then runs from 15:00 beside the status item.
  const warning = t < 3.0 ? 30 - Math.floor(t) : t >= 7.5 ? 30 : 27;
  const title = snoozed ? 900 - Math.floor(t - 3.0) : warning;
  const shown = Math.max(1 - eased(t, 3.0, 3.3), eased(t, 7.5, 7.9));
  const snooze = isMac ? { x: 392, y: 354 } : { x: 392, y: 298 };
  const cursor = path(t, [
    [0, 760, 500],
    [1.2, 760, 500],
    [2.5, snooze.x, snooze.y],
    [3.2, snooze.x, snooze.y],
    [4.2, 700, 470],
    [7.2, 700, 470],
    [7.9, 760, 500],
  ]);
  return (
    <Desktop
      platform={platform}
      trayState={2}
      trayTitle={`${Math.floor(title / 60)}:${String(title % 60).padStart(2, "0")}`}
      frontApp="Doze"
    >
      <Countdown
        platform={platform}
        x={isMac ? 290 : 286}
        y={isMac ? 120 : 92}
        remaining={warning}
        active={within(t, 2.85, 3.0) ? "Snooze 15 minutes" : null}
        opacity={shown}
        scale={0.96 + 0.04 * shown}
      />
      {isMac ? null : (
        <Tooltip lines={["Doze · Keeping awake · countdown running", "Sleep in 15:00 — countdown"]} opacity={visible(t, 4.4, 7.0)} />
      )}
      <Cursor platform={platform} x={cursor.x} y={cursor.y} down={pressed(t, [2.85])} />
    </Desktop>
  );
};

/* Agents: an agent asks to stay awake, you approve it from the menu. */
const AGENT_ROW = "Claude Code · Finish the requested refactor · 0m · Approval needed";

const Agents: Scene = ({ platform }) => {
  const t = useTime();
  const isMac = platform === "macos";
  const icon = trayIcon(platform);
  const approved = within(t, 4.95, 8);
  const root = rootMenu("Normal sleep allowed", "No power action scheduled");
  const agentsMenu: MenuItem[] = [
    { label: "Enable MCP", check: true },
    { label: "Agent settings and connections…" },
    { label: AGENT_ROW, sub: true },
  ];
  const decision: MenuItem[] = [
    { label: "When finished: Sleep", disabled: true },
    { label: "Allow Once" },
    { label: "Deny" },
  ];
  const r = rootPosition(platform, root);
  const agentsY = r.y + itemCenter(platform, root, "Agents");
  const agentsWidth = isMac ? 486 : 452;
  const subX = r.x - agentsWidth + 4;
  const subY = submenuTop(platform, root, "Agents", r.y);
  const rowY = subY + itemCenter(platform, agentsMenu, AGENT_ROW);
  const nestedX = isMac ? subX - 200 + 4 : r.x - 2;
  const nestedY = submenuTop(platform, agentsMenu, AGENT_ROW, subY);
  const allowY = nestedY + itemCenter(platform, decision, "Allow Once");
  const cursor = path(t, [
    [0, 760, 470],
    [2.4, 760, 470],
    [3.0, icon.x, icon.y],
    [3.2, icon.x, icon.y],
    [3.6, r.x + 120, agentsY],
    [3.9, subX + 300, agentsY],
    [4.2, subX + 260, rowY],
    [4.4, isMac ? subX + 60 : nestedX + 80, rowY],
    [4.7, nestedX + 90, allowY],
    [5.0, nestedX + 90, allowY],
    [5.8, 760, 470],
  ]);
  const prompt = isMac ? "~/projects/app %" : "PS C:\\projects\\app>";
  const command = 'claude "Finish the refactor, then put the computer to sleep"';
  const typed = command.slice(0, Math.floor(command.length * progress(t, 0.3, 1.7)));
  const line = (start: number, text: string, className = "") =>
    t >= start ? (
      <p className={className} style={{ opacity: eased(t, start, start + 0.15) }}>
        {text}
      </p>
    ) : null;
  const spinner = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"[Math.floor(t * 12) % 10];
  const menuClose = 4.95;
  return (
    <Desktop platform={platform} trayState={approved ? 1 : 0} frontApp="Terminal">
      <Window
        platform={platform}
        title={isMac ? "app — claude — 90×24" : "PowerShell"}
        x={50}
        y={isMac ? 56 : 36}
        width={640}
        height={isMac ? 400 : 420}
        dark
        className="sc-terminal"
      >
        <div className="sc-term">
          <p>
            <span className="sc-prompt">{prompt}</span> {typed}
            {t < 1.9 && Math.floor(t * 2.5) % 2 === 0 ? <span className="sc-caret" /> : null}
          </p>
          {line(2.0, "⏺ doze · start_session", "sc-tool")}
          {line(2.2, "    reason: Finish the requested refactor")}
          {line(2.35, "    completion_action: sleep")}
          {line(2.6, approved ? "  ⎿ Approved · active" : "  ⎿ Waiting for your approval", approved ? "sc-ok" : "sc-wait")}
          {line(5.3, `⏺ Refactoring 14 files ${t < 7.4 ? spinner : "✓"}`, "sc-tool")}
          {line(6.0, "  ✓ src/session.ts")}
          {line(6.4, "  ✓ src/power.ts")}
          {line(6.8, "  ✓ src/agents.ts")}
          {line(7.2, "⏺ doze · heartbeat", "sc-tool")}
        </div>
      </Window>
      <Menu
        platform={platform}
        items={root}
        x={r.x}
        y={r.y}
        width={ROOT_WIDTH[platform]}
        active={within(t, 3.5, menuClose) ? "Agents" : null}
        opacity={menuOpacity(platform, t, 3.25, menuClose)}
      />
      <Menu
        platform={platform}
        items={agentsMenu}
        x={subX}
        y={subY}
        width={agentsWidth}
        active={within(t, 4.1, menuClose) ? AGENT_ROW : null}
        opacity={menuOpacity(platform, t, 3.75, menuClose)}
      />
      <Menu
        platform={platform}
        items={decision}
        x={nestedX}
        y={nestedY}
        width={200}
        active={within(t, 4.65, menuClose) ? "Allow Once" : null}
        opacity={menuOpacity(platform, t, 4.3, menuClose)}
      />
      <Cursor platform={platform} x={cursor.x} y={cursor.y} down={pressed(t, [3.15, 4.85])} />
    </Desktop>
  );
};

export const scenes: Record<string, { duration: number; Scene: Scene }> = {
  hero: { duration: 8, Scene: Hero },
  "keep-awake": { duration: 8, Scene: KeepAwake },
  "power-timer": { duration: 8, Scene: PowerTimer },
  "after-playback": { duration: 8, Scene: AfterPlayback },
  "final-warning": { duration: 8, Scene: FinalWarning },
  agents: { duration: 8, Scene: Agents },
};
