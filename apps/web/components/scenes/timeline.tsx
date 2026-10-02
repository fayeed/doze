"use client";

import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { flushSync } from "react-dom";

declare global {
  interface Window {
    /** Set by scenes opened with ?capture: renders the frame at `t` seconds synchronously. */
    __dozeSeek?: (t: number) => void;
    __dozeDuration?: number;
  }
}

const Time = createContext(0);

/** Seconds since the start of the current loop. */
export const useTime = () => useContext(Time);

/** Plays a looping scene in real time, or hands frame control to the media renderer. */
export function Timeline({ duration, children }: { duration: number; children: ReactNode }) {
  const [t, setT] = useState(0);
  useEffect(() => {
    if (new URLSearchParams(location.search).has("capture")) {
      window.__dozeSeek = (value) => flushSync(() => setT(value));
      window.__dozeDuration = duration;
      return;
    }
    let frame = 0;
    const start = performance.now();
    const tick = (now: number) => {
      setT(((now - start) / 1000) % duration);
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [duration]);
  return <Time.Provider value={t}>{children}</Time.Provider>;
}

const smooth = (x: number) => (x < 0.5 ? 4 * x * x * x : 1 - (-2 * x + 2) ** 3 / 2);

export const progress = (t: number, start: number, end: number) =>
  Math.min(1, Math.max(0, (t - start) / (end - start)));

/** 0 → 1 between `start` and `end`, eased in and out. */
export const eased = (t: number, start: number, end: number) => smooth(progress(t, start, end));

/** Fades in over `fade` seconds at `start` and out again at `end`. */
export const visible = (t: number, start: number, end: number, fade = 0.18) =>
  Math.min(eased(t, start, start + fade), 1 - eased(t, end - fade, end));

/** Eased interpolation through [time, value] keys, holding the first and last values. */
export function track(t: number, keys: [number, number][]) {
  if (t <= keys[0][0]) return keys[0][1];
  for (let i = 1; i < keys.length; i++) {
    const [time, value] = keys[i];
    if (t <= time) {
      const [previousTime, previousValue] = keys[i - 1];
      return previousValue + (value - previousValue) * eased(t, previousTime, time);
    }
  }
  return keys[keys.length - 1][1];
}

/** A cursor path through [time, x, y] keys. */
export function path(t: number, keys: [number, number, number][]) {
  return {
    x: track(t, keys.map(([time, x]) => [time, x])),
    y: track(t, keys.map(([time, , y]) => [time, y])),
  };
}

export const within = (t: number, start: number, end: number) => t >= start && t < end;

/** True for a moment after each click, while the button is held. */
export const pressed = (t: number, clicks: number[]) => clicks.some((click) => t >= click && t < click + 0.14);
