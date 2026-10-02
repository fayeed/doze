"use client";

import Image from "next/image";
import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { choosePlatform, platformEvent, platformNames, readPlatform, type Platform } from "@/lib/platform";

function subscribe(callback: () => void) {
  window.addEventListener(platformEvent, callback);
  return () => window.removeEventListener(platformEvent, callback);
}

/** The platform being shown, or null while rendering on the server. */
export function usePlatform(): Platform | null {
  return useSyncExternalStore(subscribe, readPlatform, () => null);
}

/** Switches every platform-specific asset on the page. Styling follows <html data-platform>. */
export function PlatformToggle() {
  const current = usePlatform();
  return (
    <div className="toggle" role="radiogroup" aria-label="Show Doze on">
      {(["macos", "windows"] as const).map((platform) => (
        <button
          key={platform}
          type="button"
          role="radio"
          data-value={platform}
          aria-checked={current === platform}
          onClick={() => choosePlatform(platform)}
        >
          {platformNames[platform]}
        </button>
      ))}
    </div>
  );
}

/**
 * A looping clip recorded separately for each platform. Posters render on the server; the
 * video for the shown platform plays only while on screen, and never with reduced motion.
 */
export function PlatformVideo({ name, label, priority = false }: { name: string; label: string; priority?: boolean }) {
  const platform = usePlatform();
  const frame = useRef<HTMLDivElement>(null);
  const video = useRef<HTMLVideoElement>(null);
  const [onScreen, setOnScreen] = useState(false);
  const [reducedMotion, setReducedMotion] = useState(false);

  useEffect(() => {
    const query = window.matchMedia("(prefers-reduced-motion: reduce)");
    const update = () => setReducedMotion(query.matches);
    update();
    query.addEventListener("change", update);
    const observer = new IntersectionObserver(([entry]) => setOnScreen(entry.isIntersecting), {
      rootMargin: "200px 0px",
    });
    if (frame.current) observer.observe(frame.current);
    return () => {
      query.removeEventListener("change", update);
      observer.disconnect();
    };
  }, []);

  useEffect(() => {
    const element = video.current;
    if (!element) return;
    if (onScreen && !reducedMotion) element.play().catch(() => {});
    else element.pause();
  }, [onScreen, reducedMotion, platform]);

  return (
    <div className="media" ref={frame}>
      {(["macos", "windows"] as const).map((poster) => (
        <Image
          key={poster}
          className={`media-poster only-${poster === "macos" ? "mac" : "win"}`}
          src={`/media/${poster}/${name}.webp`}
          alt={`${label} on ${platformNames[poster]}`}
          fill
          sizes="(max-width: 1100px) 100vw, 1080px"
          priority={priority}
        />
      ))}
      {platform && (onScreen || priority) ? (
        <video
          key={platform}
          ref={video}
          muted
          loop
          playsInline
          preload="metadata"
          controls={reducedMotion}
          poster={`/media/${platform}/${name}.webp`}
          aria-label={`${label} on ${platformNames[platform]}`}
        >
          <source src={`/media/${platform}/${name}.mp4`} type="video/mp4" />
        </video>
      ) : null}
    </div>
  );
}
