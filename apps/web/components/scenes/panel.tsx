"use client";

import type { CSSProperties, ReactNode } from "react";
import { Glyph, type GlyphState } from "./desktop";

/*
 * Doze's tray panel, mirroring apps/desktop/native/macos/Panel.swift (the menu bar panel)
 * and apps/desktop/native/windows/TrayFlyout.cs (the Quick Settings-style flyout).
 */

const PATHS = {
  sun: "M8 5a3 3 0 1 0 0 6a3 3 0 1 0 0-6Z M8 1.5v1.6M8 12.9v1.6M1.5 8h1.6M12.9 8h1.6M3.4 3.4l1.1 1.1M11.5 11.5l1.1 1.1M3.4 12.6l1.1-1.1M11.5 4.5l1.1-1.1",
  speaker: "M2.5 6h2.2L8 3.3v9.4L4.7 10H2.5z M10.5 5.6a3.2 3.2 0 0 1 0 4.8M12.3 3.8a5.8 5.8 0 0 1 0 8.4",
  play: "M5.5 3.5l7 4.5-7 4.5z",
  clock: "M8 2a6 6 0 1 0 0 12a6 6 0 1 0 0-12Z M8 4.6V8l2.4 1.5",
  people:
    "M6 3.2a2.3 2.3 0 1 0 0 4.6a2.3 2.3 0 1 0 0-4.6Z M1.8 13c.4-2.4 2.1-3.6 4.2-3.6s3.8 1.2 4.2 3.6 M11.3 4.4a1.8 1.8 0 1 0 0 3.6a1.8 1.8 0 1 0 0-3.6Z M11.6 9.5c1.6.2 2.6 1.3 2.9 3",
  stopwatch: "M8 3.8a5.2 5.2 0 1 0 0 10.4a5.2 5.2 0 1 0 0-10.4Z M8 9V6.2M6.3 1.8h3.4",
  sliders:
    "M2 4.5h7M12 4.5h2M2 11.5h2M7 11.5h7 M10.5 3a1.5 1.5 0 1 0 0 3a1.5 1.5 0 1 0 0-3Z M5.5 10a1.5 1.5 0 1 0 0 3a1.5 1.5 0 1 0 0-3Z",
  info: "M8 2a6 6 0 1 0 0 12a6 6 0 1 0 0-12Z M8 7.2v3.8M8 5v.2",
  power: "M8 1.8v5.4 M4.6 3.9a5.2 5.2 0 1 0 6.8 0",
  gear: "M8 5.5a2.5 2.5 0 1 0 0 5a2.5 2.5 0 1 0 0-5Z M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4",
  back: "M13 8H3.5M7.5 4L3.5 8l4 4",
  plug: "M2.5 5h10a1.5 1.5 0 0 1 1.5 1.5v3A1.5 1.5 0 0 1 12.5 11h-10A1.5 1.5 0 0 1 1 9.5v-3A1.5 1.5 0 0 1 2.5 5Z M15.3 7v2 M7.5 6l-1.5 2h3L7.5 10",
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({ name, className = "sc-ic" }: { name: IconName; className?: string }) {
  return (
    <svg className={className} viewBox="0 0 16 16" aria-hidden="true">
      <path d={PATHS[name]} />
    </svg>
  );
}

const Chevron = () => (
  <svg className="sc-chev" viewBox="0 0 10 10" aria-hidden="true">
    <path d="M3.5 2L6.5 5l-3 3" />
  </svg>
);

const UpDown = () => (
  <i>
    <svg viewBox="0 0 8 8" aria-hidden="true">
      <path d="M2 3L4 1.5 6 3M2 5l2 1.5L6 5" />
    </svg>
  </i>
);

const Down = () => (
  <i>
    <svg viewBox="0 0 8 8" aria-hidden="true">
      <path d="M2 3l2 2 2-2" />
    </svg>
  </i>
);

export function Switch({ on }: { on: number }) {
  return (
    <span className="sc-switch" style={{ "--on": on } as CSSProperties}>
      <i />
    </span>
  );
}

export type Agent = {
  mono: string;
  name: string;
  caption: string;
  state: "working" | "idle" | "done";
  time?: string;
};

export type Approval = { mono: string; name: string; project: string; task: string; then: string };

/** What the panel and flyout show; each field mirrors a piece of the engine's snapshot. */
export type PanelState = {
  glyph: GlyphState;
  title: string;
  detail: string;
  stop?: boolean;
  approval?: Approval | null;
  agents?: Agent[];
  finish?: string;
  audio?: number;
  playback?: number;
  action?: string;
  timer?: string | null;
  awake?: boolean;
  awakeChoice?: string | null;
  awakeEnds?: string | null;
  /** When the running power timer acts, as the Windows timer page shows it. */
  timerAt?: string | null;
};

const hot = (id: string, current?: string | null) => (current === id ? " hot" : "");

function MacSection({ title, trailing, children }: { title: string; trailing?: ReactNode; children: ReactNode }) {
  return (
    <>
      <div className="sc-p-hd">
        <span>{title}</span>
        {trailing}
      </div>
      <div className="sc-p-sec">{children}</div>
    </>
  );
}

function MacRow({ icon, color, children }: { icon: IconName; color: string; children: ReactNode }) {
  return (
    <div className="sc-p-row">
      <span className="sc-p-tile" style={{ background: color }}>
        <Icon name={icon} />
      </span>
      {children}
    </div>
  );
}

const PRESETS = ["15m", "30m", "1h", "2h"];

/** The macOS menu bar panel: Liquid Glass under the status item. */
export function MacPanel({
  x,
  y,
  opacity,
  state,
  active,
  popup,
  pulse = 1,
}: {
  x: number;
  y: number;
  opacity: number;
  state: PanelState;
  active?: string | null;
  /** Opacity of the breathing Working dot. */
  pulse?: number;
  popup?: { items: string[]; selected: string; hover: string | null; opacity: number } | null;
}) {
  if (opacity <= 0) return null;
  const agents = state.agents ?? [];
  const working = agents.filter((agent) => agent.state === "working").length;
  const idle = agents.filter((agent) => agent.state === "idle").length;
  return (
    <div className="sc-panel" style={{ left: x, top: y, opacity, "--pulse": pulse } as CSSProperties}>
      <div className="sc-p-sec sc-p-status">
        <span className={`sc-p-statustile${state.glyph === "normal" ? "" : " awake"}`}>
          <Glyph state={state.glyph} />
        </span>
        <div className="sc-p-grow">
          <strong>{state.title}</strong>
          <span className="sc-p-sub">{state.detail}</span>
        </div>
        {state.stop ? <span className={`sc-p-btn${hot("stop", active)}`}>Stop</span> : null}
      </div>

      {state.approval ? (
        <div className="sc-p-sec sc-p-approval">
          <div className="sc-p-approval-top">
            <span className="sc-mono">{state.approval.mono}</span>
            <div className="sc-p-grow">
              <strong>{state.approval.name} wants to keep your Mac awake</strong>
              <span className="sc-p-sub">
                {state.approval.project} · “{state.approval.task}”
              </span>
              <span className="sc-p-sub">When done: {state.approval.then}</span>
            </div>
          </div>
          <div className="sc-p-actions">
            <span className={`sc-p-btn${hot("deny", active)}`}>Deny</span>
            <span className={`sc-p-btn prominent${hot("allow", active)}`}>Allow</span>
          </div>
        </div>
      ) : null}

      <MacSection
        title="Agents"
        trailing={
          agents.length ? (
            <span className="sc-p-sub sc-p-count">
              {[working ? `${working} working` : null, idle ? `${idle} idle` : null].filter(Boolean).join(" · ")}
            </span>
          ) : (
            <span className="sc-p-link">Set up…</span>
          )
        }
      >
        {agents.length ? (
          <>
            {agents.map((agent) => (
              <div key={agent.name} className="sc-p-row">
                <span className="sc-mono">{agent.mono}</span>
                <div className="sc-p-grow">
                  <span>{agent.name}</span>
                  <span className="sc-p-sub">{agent.caption}</span>
                </div>
                <span className={`sc-p-pill ${agent.state}`}>
                  {agent.state === "working" ? <span className="sc-dot" /> : null}
                  {agent.state === "working" ? `Working · ${agent.time}` : agent.state === "done" ? `Done · ${agent.time}` : "Idle"}
                </span>
              </div>
            ))}
            <MacRow icon="power" color="#636366">
              <div className="sc-p-grow">
                <span>When agents finish</span>
                <span className="sc-p-sub">After the final warning</span>
              </div>
              <span className="sc-p-acc">
                {state.finish ?? "Sleep"}
                <UpDown />
              </span>
            </MacRow>
          </>
        ) : (
          <MacRow icon="people" color="#30B0C7">
            <div className="sc-p-grow">
              <span>No agents running</span>
              <span className="sc-p-sub wrap">Connected agents show up here while they work</span>
            </div>
            <span className="sc-p-btn">Connect…</span>
          </MacRow>
        )}
      </MacSection>

      <MacSection title="Keep Awake">
        <div className="sc-p-row">
          <div className="sc-p-chips">
            {PRESETS.map((preset) => (
              <span key={preset} className={`sc-p-chip${hot(`awake-${preset}`, active)}`}>
                {preset}
              </span>
            ))}
            <span className="sc-p-chip">Indefinitely</span>
          </div>
          <span className="sc-p-acc">
            More
            <Down />
          </span>
        </div>
        <MacRow icon="speaker" color="#FF453A">
          <span className="sc-p-grow">Keep awake while audio plays</span>
          <Switch on={state.audio ?? 0} />
        </MacRow>
      </MacSection>

      <MacSection title="Power Timer">
        <MacRow icon="power" color="#636366">
          <span className="sc-p-grow">Action</span>
          <span className={`sc-p-acc sc-p-popup${hot("action", active)}`}>
            {state.action ?? "Sleep"}
            <UpDown />
            {popup && popup.opacity > 0 ? (
              <span
                className="sc-p-menu"
                style={{ opacity: popup.opacity, top: -6 - 22 * popup.items.indexOf(popup.selected) }}
              >
                {popup.items.map((item) => (
                  <span key={item} className={popup.hover === item ? "hot" : ""}>
                    <b>{item === popup.selected ? "✓" : ""}</b>
                    {item}
                  </span>
                ))}
              </span>
            ) : null}
          </span>
        </MacRow>
        {state.timer ? (
          <div className="sc-p-row">
            <span className="sc-p-grow sc-p-num">{state.timer}</span>
            <span className="sc-p-btn">Stop timer</span>
          </div>
        ) : (
          <div className="sc-p-row">
            <div className="sc-p-chips">
              {PRESETS.map((preset) => (
                <span key={preset} className={`sc-p-chip${hot(`timer-${preset}`, active)}`}>
                  {preset}
                </span>
              ))}
            </div>
            <span className="sc-p-acc">
              More
              <Down />
            </span>
          </div>
        )}
        <MacRow icon="play" color="#5E5CE6">
          <span className="sc-p-grow">Sleep after playback stops</span>
          <Switch on={state.playback ?? 0} />
        </MacRow>
      </MacSection>

      <div className="sc-p-sec sc-p-menus">
        {(
          [
            ["Countdown", "stopwatch", "#FF9F0A"],
            ["Quick Settings", "sliders", "#8E8E93"],
            ["Help & About", "info", "#0A84FF"],
          ] as const
        ).map(([label, icon, color]) => (
          <MacRow key={label} icon={icon} color={color}>
            <span className="sc-p-grow">{label}</span>
            <span className="sc-p-more">›</span>
          </MacRow>
        ))}
      </div>

      <div className="sc-p-foot">
        <span>
          Settings…<span>⌘,</span>
        </span>
        <span>
          Quit Doze<span>⌘Q</span>
        </span>
      </div>
    </div>
  );
}

/* ---------- Windows ---------- */

export const FLYOUT = { right: 12, width: 360 };

function Tile({
  label,
  caption,
  icon,
  on,
  more,
  id,
  active,
}: {
  label: ReactNode;
  caption?: string;
  icon: IconName;
  on?: boolean;
  more?: boolean;
  id: string;
  active?: string | null;
}) {
  return (
    <div className="sc-f-q">
      <span className={`sc-f-qt${on ? " on" : ""}`}>
        <span className={`sc-f-a${hot(id, active)}`}>
          <Icon name={icon} />
        </span>
        {more ? (
          <span className={`sc-f-b${hot(`${id}-more`, active)}`}>
            <Chevron />
          </span>
        ) : null}
      </span>
      <span>
        {label}
        {caption ? <span className="sc-f-cap">{caption}</span> : null}
      </span>
    </div>
  );
}

function Footer({ left, settings }: { left: ReactNode; settings?: boolean }) {
  return (
    <div className="sc-f-ff">
      {left}
      <span className="sc-f-icons">
        {settings ? null : (
          <span className="sc-f-ib">
            <Icon name="sliders" />
          </span>
        )}
        <span className="sc-f-ib">
          <Icon name="gear" />
        </span>
      </span>
    </div>
  );
}

function FlyoutMain({ state, active }: { state: PanelState; active?: string | null }) {
  const agents = state.agents ?? [];
  const working = agents.filter((agent) => agent.state === "working").length;
  const idle = agents.filter((agent) => agent.state === "idle").length;
  return (
    <>
      <div className="sc-f-top">
        <div className="sc-f-status">
          <span className={`sc-f-glyph${state.glyph === "normal" ? "" : " on"}`}>
            <Glyph state={state.glyph} />
          </span>
          <div className="sc-p-grow">
            <strong>{state.title}</strong>
            <span className="sc-f-cap">{state.detail}</span>
          </div>
          {state.stop ? <span className={`sc-f-btn${hot("stop", active)}`}>Stop</span> : null}
        </div>

        {state.approval ? (
          <div className="sc-f-info">
            <div className="sc-p-approval-top">
              <span className="sc-mono">{state.approval.mono}</span>
              <div className="sc-p-grow">
                <strong>{state.approval.name} wants to keep your PC awake</strong>
                <span className="sc-f-cap">
                  {state.approval.project} · “{state.approval.task}” · then {state.approval.then}
                </span>
              </div>
            </div>
            <div className="sc-p-actions">
              <span className={`sc-f-btn accent${hot("allow", active)}`}>Allow</span>
              <span className={`sc-f-btn${hot("deny", active)}`}>Deny</span>
            </div>
          </div>
        ) : null}

        <div className="sc-f-grid">
          <Tile id="keep" label="Keep awake" icon="sun" on={state.awake} more active={active} />
          <Tile id="audio" label={<>Awake while<br />audio plays</>} icon="speaker" on={!!state.audio} active={active} />
          <Tile id="playback" label={<>Sleep after<br />playback</>} icon="play" on={!!state.playback} active={active} />
          <Tile
            id="timer"
            label="Power timer"
            caption={state.timer ? `${state.action} · ${state.timer}` : state.action ?? "Sleep"}
            icon="clock"
            on={!!state.timer}
            more
            active={active}
          />
          <Tile
            id="agents"
            label="Agents"
            caption={state.approval ? "1 asking" : working ? `${working} working` : "None running"}
            icon="people"
            on
            more
            active={active}
          />
          <Tile id="countdown" label="Countdown" caption="5 min" icon="stopwatch" more active={active} />
        </div>

        {agents.length ? (
          <div className="sc-f-agents">
            <div className="sc-f-sh">
              <span>Agents</span>
              <span className="sc-f-cap">
                {[working ? `${working} working` : null, idle ? `${idle} idle` : null].filter(Boolean).join(" · ")}
              </span>
            </div>
            <div className="sc-f-list">
              {agents.map((agent) => (
                <div key={agent.name} className="sc-f-it">
                  <span className="sc-mono">{agent.mono}</span>
                  <div className="sc-p-grow">
                    <span>{agent.name}</span>
                    <span className="sc-f-cap">{agent.caption}</span>
                  </div>
                  <span className={`sc-p-pill ${agent.state}`}>
                    {agent.state === "working" ? <span className="sc-dot" /> : null}
                    {agent.state === "working" ? `Working ${agent.time}` : agent.state === "done" ? "Done" : "Idle"}
                  </span>
                </div>
              ))}
            </div>
            <div className="sc-f-finish">
              <span>When agents finish</span>
              <span className="sc-f-combo">{state.finish ?? "Sleep"}</span>
            </div>
          </div>
        ) : null}
      </div>
      <Footer
        left={
          <span className="sc-f-battery">
            <Icon name="plug" />
            Plugged in
          </span>
        }
      />
    </>
  );
}

type Item = { label: string; caption?: string | null; selected?: boolean } | "-";

function SubPage({
  title,
  items,
  link,
  active,
  header,
}: {
  title: string;
  items: Item[];
  link: string;
  active?: string | null;
  header?: ReactNode;
}) {
  return (
    <>
      <div className="sc-f-hd">
        <span className={`sc-f-ib back${hot("back", active)}`}>
          <Icon name="back" />
        </span>
        <strong>{title}</strong>
      </div>
      <div className="sc-f-sub">
        {header}
        {items.map((item, index) =>
          item === "-" ? (
            <div key={index} className="sc-f-sep" />
          ) : (
            <div key={item.label} className={`sc-f-item${item.selected ? " sel" : ""}${hot(item.label, active)}`}>
              <span>{item.label}</span>
              {item.caption ? <span className="sc-f-cap">{item.caption}</span> : null}
            </div>
          ),
        )}
      </div>
      <Footer left={<span className="sc-f-link">{link}</span>} settings />
    </>
  );
}

export const WIN_ACTIONS = ["Sleep", "Hibernate", "Shut down", "Lock", "Turn display off"];

function KeepAwakePage({ state, active }: { state: PanelState; active?: string | null }) {
  const choice = state.awake ? state.awakeChoice : null;
  const items: Item[] = [
    { label: "Default", caption: "30 minutes" },
    "-",
    ...["15 minutes", "30 minutes", "1 hour", "2 hours"].map((label) => ({
      label,
      caption: choice === label ? state.awakeEnds : null,
      selected: choice === label,
    })),
    { label: "Indefinitely" },
    "-",
    { label: "Custom duration…" },
    { label: "Until a specific time…" },
    ...(state.awake ? (["-", { label: "Stop keeping awake" }] as Item[]) : []),
  ];
  return <SubPage title="Keep awake" items={items} link="More Keep awake settings" active={active} />;
}

function TimerPage({
  state,
  active,
  combo,
}: {
  state: PanelState;
  active?: string | null;
  combo?: { hover: string | null; opacity: number } | null;
}) {
  const action = state.action ?? "Sleep";
  const items: Item[] = [
    ...(state.timer
      ? ([{ label: `${action} in ${state.timer}`, caption: state.timerAt, selected: true }, { label: "Stop timer" }, "-"] as Item[])
      : []),
    { label: "Default", caption: "30 minutes" },
    "-",
    { label: "15 minutes" },
    { label: "30 minutes" },
    { label: "1 hour" },
    { label: "2 hours" },
    "-",
    { label: "Custom duration…" },
    { label: "At a specific time…" },
  ];
  const header = (
    <div className="sc-f-action">
      <span>Action</span>
      <span className={`sc-f-combo wide${hot("action", active)}`}>
        {action}
        {combo && combo.opacity > 0 ? (
          <span className="sc-f-drop" style={{ opacity: combo.opacity, top: -5 - 36 * WIN_ACTIONS.indexOf(action) }}>
            {WIN_ACTIONS.map((item) => (
              <span key={item} className={`${item === action ? "sel" : ""}${combo.hover === item ? " hot" : ""}`}>
                {item}
              </span>
            ))}
          </span>
        ) : null}
      </span>
    </div>
  );
  return <SubPage title="Power timer" items={items} link="More Power timer settings" active={active} header={header} />;
}

/**
 * The Windows tray flyout, docked at the right end of the taskbar. It slides out from behind
 * the taskbar (clipped at its edge) as `shown` goes from 0 to 1.
 */
export function WinFlyout({
  shown,
  page = "main",
  state,
  active,
  combo,
  pulse = 1,
}: {
  shown: number;
  page?: "main" | "keep" | "timer";
  state: PanelState;
  active?: string | null;
  pulse?: number;
  combo?: { hover: string | null; opacity: number } | null;
}) {
  if (shown <= 0) return null;
  return (
    <div className="sc-flyout-clip" style={{ right: FLYOUT.right, width: FLYOUT.width, "--pulse": pulse } as CSSProperties}>
      <div
        className="sc-flyout"
        style={{ transform: `translateY(calc(${(1 - shown) * 100}% + ${(1 - shown) * 12}px))`, opacity: Math.min(1, shown * 2) }}
      >
        {page === "keep" ? (
          <KeepAwakePage state={state} active={active} />
        ) : page === "timer" ? (
          <TimerPage state={state} active={active} combo={combo} />
        ) : (
          <FlyoutMain state={state} active={active} />
        )}
      </div>
    </div>
  );
}
