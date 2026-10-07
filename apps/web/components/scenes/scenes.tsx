"use client";

import type { Platform } from "@/lib/platform";
import { Countdown, Cursor, Desktop, Notice, Tooltip, Window, trayIcon } from "./desktop";
import { MacPanel, WinFlyout, type Agent, type PanelState } from "./panel";
import { SettingsWindow } from "./settings";
import { eased, path, pressed, progress, track, useTime, visible, within } from "./timeline";

type Scene = (props: { platform: Platform }) => React.ReactNode;

/*
 * The panel sits under the status item on macOS (Panel.swift centres it on the icon) and the
 * flyout docks at the right end of the taskbar on Windows (TrayFlyout.cs). Pointer targets
 * below are the centres of the controls they click, measured from the rendered scenes.
 */
const PANEL = { x: trayIcon("macos").x - 184, y: 32 };

/** The panel appears almost at once and fades out like a dismissed menu. */
const panelOpacity = (t: number, open: number, close: number) => visible(t, open, close, 0.12);

/** The flyout slides out from behind the taskbar on a decelerating curve, and back down. */
function flyoutShown(t: number, open: number, close: number) {
  const out = 1 - (1 - progress(t, open, open + 0.28)) ** 3;
  const back = progress(t, close, close + 0.2) ** 2;
  return t < open ? 0 : Math.max(0, out - back);
}

/** The Working dot breathes, as PulsingDot does. */
const pulse = (t: number) => 0.675 + 0.325 * Math.cos((t / 1.6) * Math.PI * 2);

const idle: PanelState = {
  glyph: "normal",
  title: "Normal sleep allowed",
  detail: "No power action scheduled",
  action: "Sleep",
};

/* Hero: start a two-hour Keep Awake session from the panel, then close it. */
const HERO = {
  macos: { chip: [680, 253] },
  windows: { more: [691, 316], twoHours: [680, 323], back: [616, 96] },
};

const Hero: Scene = ({ platform }) => {
  const t = useTime();
  const icon = trayIcon(platform);
  if (platform === "macos") {
    const awake = within(t, 2.55, 7.85);
    const [cx, cy] = HERO.macos.chip;
    const cursor = path(t, [
      [0, 480, 330],
      [0.4, 480, 330],
      [1.2, icon.x, icon.y],
      [1.4, icon.x, icon.y],
      [2.3, cx, cy],
      [3.4, cx, cy],
      [3.9, cx + 30, cy + 40],
      [4.5, cx + 30, cy + 40],
      [5.1, 360, 400],
      [5.6, 360, 400],
      [7.6, 480, 330],
    ]);
    const state: PanelState = awake
      ? { ...idle, glyph: "awake", title: "Keeping awake", detail: "2h 0m left · until 23:40", stop: true }
      : idle;
    return (
      <Desktop platform={platform} trayState={awake ? "awake" : "normal"} trayTitle={awake ? "2:00" : null} trayOpen={within(t, 1.32, 5.25)}>
        <MacPanel
          x={PANEL.x}
          y={PANEL.y}
          opacity={panelOpacity(t, 1.32, 5.37)}
          state={state}
          active={within(t, 2.2, 2.66) ? "awake-2h" : null}
        />
        <Cursor platform={platform} x={cursor.x} y={cursor.y} down={pressed(t, [1.3, 2.5, 5.2])} />
      </Desktop>
    );
  }
  const awake = within(t, 3.05, 7.85);
  const page = within(t, 2.25, 3.85) ? "keep" : "main";
  const { more, twoHours, back } = HERO.windows;
  const cursor = path(t, [
    [0, 480, 300],
    [0.4, 480, 300],
    [1.2, icon.x, icon.y],
    [1.4, icon.x, icon.y],
    [2.1, more[0], more[1]],
    [2.3, more[0], more[1]],
    [2.9, twoHours[0], twoHours[1]],
    [3.2, twoHours[0], twoHours[1]],
    [3.7, back[0], back[1]],
    [3.95, back[0], back[1]],
    [4.4, 700, 330],
    [4.8, 700, 330],
    [5.2, 420, 320],
    [5.5, 420, 320],
    [7.6, 480, 300],
  ]);
  const state: PanelState = awake
    ? { ...idle, glyph: "awake", title: "Keeping awake", detail: "2h 0m left", stop: true, awake: true, awakeChoice: "2 hours", awakeEnds: "Ends 23:40" }
    : idle;
  const active = within(t, 2.05, 2.36)
    ? "keep-more"
    : within(t, 2.85, 3.14)
      ? "2 hours"
      : within(t, 3.65, 3.92)
        ? "back"
        : null;
  return (
    <Desktop platform={platform} trayState={awake ? "awake" : "normal"} trayOpen={within(t, 1.3, 5.3)}>
      <WinFlyout shown={flyoutShown(t, 1.32, 5.32)} page={page} state={state} active={active} />
      <Cursor platform={platform} x={cursor.x} y={cursor.y} down={pressed(t, [1.3, 2.2, 3.0, 3.8, 5.3])} />
    </Desktop>
  );
};

/* Keep Awake: Overview in Settings, extend the session and turn on audio. */
const KEEP_AWAKE = {
  macos: { extend: [698, 225], audio: [803, 273] },
  windows: { extend: [695, 297], audio: [806, 366] },
};

const KeepAwake: Scene = ({ platform }) => {
  const t = useTime();
  const extended = within(t, 1.9, 7.7);
  const left = extended ? "1h 57m" : "1h 42m";
  const audio = t < 3.55 ? 0 : t < 7.7 ? eased(t, 3.55, 3.75) : 1 - eased(t, 7.7, 7.9);
  const { extend, audio: toggle } = KEEP_AWAKE[platform];
  const cursor = path(t, [
    [0, 640, 560],
    [0.6, 640, 560],
    [1.6, extend[0], extend[1]],
    [2.4, extend[0], extend[1]],
    [3.3, toggle[0], toggle[1]],
    [4.4, toggle[0], toggle[1]],
    [5.4, 700, 500],
    [7.2, 700, 500],
    [7.9, 640, 560],
  ]);
  return (
    <Desktop platform={platform} trayState="awake" trayTitle={extended ? "1:57" : "1:42"} frontApp="Doze">
      <SettingsWindow
        platform={platform}
        state={{ left, until: extended ? "23:37" : "23:22", audio, active: within(t, 1.8, 1.95) ? "extend" : null }}
      />
      <Cursor platform={platform} x={cursor.x} y={cursor.y} down={pressed(t, [1.8, 3.5])} />
    </Desktop>
  );
};

/* Power Timer: choose Shut down and an hour from the panel or flyout. */
const MAC_ACTIONS = ["Sleep", "Shut down", "Lock", "Turn display off"];
const POWER_TIMER = {
  macos: { action: [838, 366], shutDown: [790, 390], chip: [646, 404] },
  windows: { more: [691, 415], combo: [861, 137], shutDown: [850, 211], hour: [680, 323], back: [616, 94] },
};

const PowerTimer: Scene = ({ platform }) => {
  const t = useTime();
  const icon = trayIcon(platform);
  if (platform === "macos") {
    const { action, shutDown, chip } = POWER_TIMER.macos;
    const chosen = within(t, 2.6, 7.85) ? "Shut down" : "Sleep";
    const running = within(t, 3.45, 7.85);
    const cursor = path(t, [
      [0, 560, 420],
      [0.2, 560, 420],
      [0.9, icon.x, icon.y],
      [1.1, icon.x, icon.y],
      [1.8, action[0], action[1]],
      [2.0, action[0], action[1]],
      [2.4, shutDown[0], shutDown[1]],
      [2.7, shutDown[0], shutDown[1]],
      [3.3, chip[0], chip[1]],
      [3.6, chip[0], chip[1]],
      [4.2, 700, 80],
      [5.5, 700, 80],
      [6.0, 380, 420],
      [6.4, 380, 420],
      [7.6, 560, 420],
    ]);
    const state: PanelState = running
      ? { ...idle, glyph: "awake", title: "Keeping awake", detail: "Shut down in 1h 0m", action: chosen, timer: "Shut down in 1h 0m" }
      : { ...idle, action: chosen };
    return (
      <Desktop
        platform={platform}
        trayState={running ? "awake" : "normal"}
        trayTitle={running ? "1:00" : null}
        trayOpen={within(t, 1.02, 6.3)}
      >
        <MacPanel
          x={PANEL.x}
          y={PANEL.y}
          opacity={panelOpacity(t, 1.02, 6.42)}
          state={state}
          active={within(t, 1.9, 2.0) ? "action" : within(t, 3.35, 3.55) ? "timer-1h" : null}
          popup={{
            items: MAC_ACTIONS,
            selected: "Sleep",
            hover: within(t, 2.0, 2.3) ? "Sleep" : within(t, 2.3, 2.62) ? "Shut down" : null,
            opacity: visible(t, 1.95, 2.66, 0.06),
          }}
        />
        <Cursor platform={platform} x={cursor.x} y={cursor.y} down={pressed(t, [1.0, 1.9, 2.6, 3.4, 6.3])} />
      </Desktop>
    );
  }
  const { more, combo, shutDown, hour, back } = POWER_TIMER.windows;
  const chosen = within(t, 3.1, 7.85) ? "Shut down" : "Sleep";
  const running = within(t, 3.95, 7.85);
  const page = within(t, 1.85, 4.85) ? "timer" : "main";
  const cursor = path(t, [
    [0, 520, 300],
    [0.2, 520, 300],
    [0.9, icon.x, icon.y],
    [1.1, icon.x, icon.y],
    [1.7, more[0], more[1]],
    [1.9, more[0], more[1]],
    [2.4, combo[0], combo[1]],
    [2.6, combo[0], combo[1]],
    [3.0, shutDown[0], shutDown[1]],
    [3.2, shutDown[0], shutDown[1]],
    [3.8, hour[0], hour[1]],
    [4.0, hour[0], hour[1]],
    [4.7, back[0], back[1]],
    [4.95, back[0], back[1]],
    [5.4, 700, 300],
    [5.9, 700, 300],
    [6.3, 420, 320],
    [6.6, 420, 320],
    [7.6, 520, 300],
  ]);
  const state: PanelState = running
    ? { ...idle, glyph: "awake", title: "Keeping awake", detail: "Shut down in 1h 0m", action: chosen, timer: "1h 0m", timerAt: "At 22:40" }
    : { ...idle, action: chosen };
  const active = within(t, 1.65, 1.95)
    ? "timer-more"
    : within(t, 2.4, 2.56)
      ? "action"
      : within(t, 3.75, 3.95)
        ? "1 hour"
        : within(t, 4.65, 4.92)
          ? "back"
          : null;
  return (
    <Desktop platform={platform} trayState={running ? "awake" : "normal"} trayOpen={within(t, 1.0, 6.4)} frontApp="Doze">
      <WinFlyout
        shown={flyoutShown(t, 1.02, 6.42)}
        page={page}
        state={state}
        active={active}
        combo={{
          hover: within(t, 2.6, 2.85) ? "Sleep" : within(t, 2.85, 3.12) ? "Shut down" : null,
          opacity: visible(t, 2.52, 3.16, 0.08),
        }}
      />
      <Cursor platform={platform} x={cursor.x} y={cursor.y} down={pressed(t, [1.0, 1.8, 2.5, 3.1, 3.9, 4.8, 6.4])} />
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
      trayState={warning ? "countdown" : ended ? "normal" : "awake"}
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
        x={isMac ? 290 : 290}
        y={isMac ? 96 : 90}
        remaining={remaining}
        source="Playback stopped"
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
  // The warning counts down from 0:30 and freezes as it fades on Snooze; the snooze then
  // runs as a 15-minute timer beside the status item.
  const warning = t < 3.0 ? 30 - Math.floor(t) : t >= 7.5 ? 30 : 27;
  const shown = Math.max(1 - eased(t, 3.0, 3.3), eased(t, 7.5, 7.9));
  const snooze = isMac ? { x: 392, y: 368 } : { x: 392, y: 298 };
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
      trayState={snoozed ? "awake" : "countdown"}
      trayTitle={snoozed ? "15m" : `0:${String(warning).padStart(2, "0")}`}
      frontApp="Doze"
    >
      <Countdown
        platform={platform}
        x={isMac ? 290 : 286}
        y={isMac ? 106 : 92}
        remaining={warning}
        source="Power timer finished"
        active={within(t, 2.85, 3.0) ? "Snooze 15 minutes" : null}
        opacity={shown}
        scale={0.96 + 0.04 * shown}
      />
      {isMac ? null : (
        <Tooltip lines={["Doze · Keeping awake · timer running", "Sleep in 15m"]} opacity={visible(t, 4.4, 7.0)} />
      )}
      <Cursor platform={platform} x={cursor.x} y={cursor.y} down={pressed(t, [2.85])} />
    </Desktop>
  );
};

/* Agents: Claude Code starts work, asks to keep the computer awake, and you allow it. */
const AGENTS = {
  macos: { allow: [839, 184] },
  windows: { allow: [823, 248] },
};

const Agents: Scene = ({ platform }) => {
  const t = useTime();
  const isMac = platform === "macos";
  const icon = trayIcon(platform);
  const asking = within(t, 2.0, 4.05);
  const approved = within(t, 4.05, 8);
  const allow = AGENTS[platform].allow;
  const cursor = path(t, [
    [0, 560, 470],
    [2.2, 560, 470],
    [2.9, icon.x, icon.y],
    [3.1, icon.x, icon.y],
    [3.8, allow[0], allow[1]],
    [4.15, allow[0], allow[1]],
    [5.0, allow[0] - 60, allow[1] + 160],
    [5.3, allow[0] - 60, allow[1] + 160],
    [5.7, 360, 330],
    [6.0, 360, 330],
    [7.6, 560, 470],
  ]);
  const prompt = isMac ? "~/projects/app %" : "PS C:\\projects\\app>";
  const command = 'claude "Finish the refactor"';
  const typed = command.slice(0, Math.floor(command.length * progress(t, 0.3, 1.4)));
  const line = (start: number, text: string, className = "") =>
    t >= start ? (
      <p className={className} style={{ opacity: eased(t, start, start + 0.15) }}>
        {text}
      </p>
    ) : null;
  const spinner = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"[Math.floor(t * 12) % 10];
  const claude: Agent = { mono: "CC", name: "Claude Code", caption: "app · Finish the refactor", state: "working", time: "0m" };
  const state: PanelState = approved
    ? { ...idle, glyph: "awake", title: "Keeping awake", detail: "For Claude Code · since 21:40", stop: true, agents: [claude], finish: "Sleep" }
    : {
        ...idle,
        approval: asking ? { mono: "CC", name: "Claude Code", project: "app", task: "Finish the refactor", then: "Sleep" } : null,
      };
  const open = 3.02;
  const close = 5.82;
  const allowDown = within(t, 3.9, 4.06) ? "allow" : null;
  return (
    <Desktop
      platform={platform}
      trayState={approved ? "awake" : asking ? "attention" : "normal"}
      trayTitle={approved ? "1" : null}
      trayOpen={within(t, open, close - 0.1)}
      frontApp="Terminal"
    >
      <Window
        platform={platform}
        title={isMac ? "app — claude — 90×24" : "PowerShell"}
        x={50}
        y={isMac ? 56 : 36}
        width={600}
        height={isMac ? 400 : 420}
        dark
        className="sc-terminal"
      >
        <div className="sc-term">
          <p>
            <span className="sc-prompt">{prompt}</span> {typed}
            {t < 1.6 && Math.floor(t * 2.5) % 2 === 0 ? <span className="sc-caret" /> : null}
          </p>
          {line(1.8, "⏺ I'll finish the refactor, starting with the session code.", "sc-tool")}
          {line(2.3, "⏺ Read 14 files", "sc-tool")}
          {line(4.5, "⏺ Update(src/session.ts)", "sc-tool")}
          {line(4.8, "  ⎿  Updated 3 functions", "sc-dim-line")}
          {line(5.4, "⏺ Update(src/power.ts)", "sc-tool")}
          {line(5.7, "  ⎿  Updated 2 functions", "sc-dim-line")}
          {line(6.3, "⏺ Update(src/agents.ts)", "sc-tool")}
          {line(6.9, `⏺ Running the test suite ${spinner}`, "sc-tool")}
        </div>
      </Window>
      <Notice
        platform={platform}
        title={`Claude Code wants to keep your ${isMac ? "Mac" : "PC"} awake`}
        body="app · Allow or deny it in Doze."
        opacity={visible(t, 2.1, open, 0.25)}
      />
      {isMac ? (
        <MacPanel x={PANEL.x} y={PANEL.y} opacity={panelOpacity(t, open, close)} state={state} active={allowDown} pulse={pulse(t)} />
      ) : (
        <WinFlyout shown={flyoutShown(t, open, close)} state={state} active={allowDown} pulse={pulse(t)} />
      )}
      <Cursor platform={platform} x={cursor.x} y={cursor.y} down={pressed(t, [3.0, 4.0, 5.8])} />
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
