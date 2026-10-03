import React, { useCallback, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import "./style.css";

type Session = { id: string; provider: string; task: string; workspace: string | null; activity: string; workingSeconds: number; startedAt: number; lastActivity: number; status: string; title?: string };
type Snapshot = { awake: boolean; awakeDeadline: number | null; now: number; sessions: Session[] };

function duration(seconds: number) {
  const h = Math.floor(seconds / 3600), m = Math.floor(seconds % 3600 / 60);
  return h ? `${h}h ${m}m` : `${m}m`;
}
function App() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [error, setError] = useState("");
  const refresh = useCallback(async () => {
    try { setSnapshot(await invoke<Snapshot>("panel_snapshot")); setError(""); }
    catch { setError("Doze is starting up. Try again in a moment."); }
  }, []);
  useEffect(() => {
    void refresh();
    let tick = 0;
    const visibility = () => {
      window.clearInterval(tick);
      if (!document.hidden) { void refresh(); tick = window.setInterval(() => void refresh(), 1000); }
    };
    visibility();
    const key = (event: KeyboardEvent) => { if (event.key === "Escape") void invoke("panel_close"); };
    window.addEventListener("keydown", key);
    document.addEventListener("visibilitychange", visibility);
    return () => { clearInterval(tick); document.removeEventListener("visibilitychange", visibility); window.removeEventListener("keydown", key); };
  }, [refresh]);
  const action = async (name: string) => { await invoke("panel_action", { action: name }); await refresh(); };
  const sessions = snapshot?.sessions ?? [];
  const working = sessions.filter(s => s.activity === "Working").length;
  return <main>
    <header><div className="brand"><span className="brand-mark">z</span><div><strong>Doze</strong><small>Power, with a little more peace.</small></div></div><button className="icon-button" aria-label="Close panel" onClick={() => void invoke("panel_close")}>×</button></header>
    <section className="hero"><div className="eyebrow">RIGHT NOW</div><div className="headline"><span className={`pulse ${snapshot?.awake ? "on" : ""}`} />{snapshot?.awake ? "Keeping things awake" : "Normal sleep allowed"}</div><div className="subline">{working} working · {sessions.length} active Doze sessions</div></section>
    <section className="section-head"><h2>Agents</h2><span>{sessions.length} sessions</span></section>
    {error && <p className="empty">{error}</p>}
    {!error && sessions.length === 0 && <div className="empty"><span className="empty-sun">✳</span><strong>All quiet here</strong><span>Connected agent sessions will show up here.</span></div>}
    <div className="sessions">{sessions.map(session => {
      const waiting = session.activity === "Waiting" || session.status === "ConnectionLost" || session.status === "AwaitingAuthorization";
      return <article className="session" key={session.id}>
        <div className={`provider ${session.provider.toLowerCase().includes("claude") ? "claude" : "codex"}`} aria-hidden="true">{session.provider.toLowerCase().includes("claude") ? "✳" : "◈"}</div>
        <div className="session-body"><div className="session-title"><strong>{session.provider}</strong><span className={`state ${waiting ? "waiting" : session.activity === "Working" ? "working" : ""}`}><i />{session.status === "ConnectionLost" ? "Connection lost" : session.status === "AwaitingAuthorization" ? "Approval needed" : session.activity === "Unknown" ? "Session open" : session.activity}</span></div><div className="task">{session.title || session.task}{session.workspace ? ` · ${session.workspace}` : ""}</div><div className="meta">{duration(Math.max(0, (snapshot?.now ?? 0) - session.startedAt))} total · {duration(session.workingSeconds)} working · signal {duration(Math.max(0, (snapshot?.now ?? 0) - session.lastActivity))} ago</div></div>
      </article>;
    })}</div>
    <section className="section-head controls-head"><h2>Keep awake</h2></section>
    <div className="controls"><button onClick={() => void action("awake15")}>15 min</button><button onClick={() => void action("awake30")}>30 min</button><button onClick={() => void action("awake60")}>1 hour</button><button className="quiet" onClick={() => void action(snapshot?.awake ? "stopAwake" : "awakeForever")}>{snapshot?.awake ? "Stop" : "Indefinitely"}</button></div>
    <footer><button onClick={() => void action("timer30")}>＋ Power timer</button><button onClick={() => void action("playback")}>After playback</button><button className="settings" onClick={() => void action("settings")}>Settings ↗</button></footer>
  </main>;
}
createRoot(document.getElementById("root")!).render(<App />);
