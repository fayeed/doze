"use client";

import type { Platform } from "@/lib/platform";
import { scenes } from "./scenes";
import { Timeline } from "./timeline";
import "./scenes.css";

/** Renders one scene at stage size, for previewing and for scripts/render-media.mjs. */
export function ScenePlayer({ name, platform }: { name: string; platform: Platform }) {
  const entry = scenes[name];
  if (!entry) return <p>Unknown scene “{name}”. Try: {Object.keys(scenes).join(", ")}.</p>;
  const { Scene, duration } = entry;
  return (
    <Timeline duration={duration}>
      <Scene platform={platform} />
    </Timeline>
  );
}
