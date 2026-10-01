"use client";

import { useEffect, useState } from "react";

const START = 4 * 60 + 47;

/** A live replica of Doze's final warning. It loops, and holds still for reduced motion. */
export function Countdown() {
  const [remaining, setRemaining] = useState(START);

  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const timer = window.setInterval(
      () => setRemaining((value) => (value <= 1 ? START : value - 1)),
      1000,
    );
    return () => window.clearInterval(timer);
  }, []);

  const minutes = Math.floor(remaining / 60);
  const seconds = String(remaining % 60).padStart(2, "0");

  return (
    <figure className="warning" aria-label="Doze's final warning window, counting down to sleep">
      <div className="warning-chrome" aria-hidden="true">
        <span />
        <span />
        <span />
        <em>Doze · Power countdown</em>
      </div>
      <div className="warning-body">
        <svg className="warning-moon" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5Z" />
        </svg>
        <p className="warning-action">Sleep in</p>
        <p className="warning-time" aria-hidden="true">
          {minutes}:{seconds}
        </p>
        <p className="warning-hint">Cancel the action or snooze for 15 minutes.</p>
        <div className="warning-buttons" aria-hidden="true">
          <span>Snooze 15 minutes</span>
          <span>Cancel</span>
          <span>Stay Awake</span>
        </div>
      </div>
      <figcaption>Every action waits for a final warning you can cancel.</figcaption>
    </figure>
  );
}
