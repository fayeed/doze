import { brand } from "@doze/brand";
import Image from "next/image";
import { Mark, OS, Wordmark } from "@/components/os";
import { PlatformToggle, PlatformVideo } from "@/components/platform";

import { downloads } from "@/lib/downloads";

const chips = [
  "Keep Awake",
  "Power Timer",
  "Sleep after playback",
  "Final warning",
  "Wake leases for agents",
  "Command line",
  "Local first",
  "No telemetry",
  "No simulated input",
];

const features = [
  {
    id: "keep-awake",
    tag: "Keep Awake",
    title: "Awake for exactly as long as you need",
    body: (
      <>
        Fifteen minutes, two hours, until 11:30, or indefinitely. The time left sits beside the{" "}
        <OS mac="menu bar icon" win="tray icon" />, and Overview lets you extend or stop with one click. Normal sleep
        settings apply again the moment it ends.
      </>
    ),
  },
  {
    id: "power-timer",
    tag: "Power Timer",
    title: "Sleep, shut down or lock on a timer",
    body: (
      <>
        Pick an action, a duration or a time of day, and walk away. Your <OS mac="Mac" win="PC" /> stays awake until
        the timer finishes, then Doze sleeps, hibernates, shuts down, locks or turns the display off.
      </>
    ),
  },
  {
    id: "after-playback",
    tag: "After Playback",
    title: "Fall asleep to a film. Doze handles the rest.",
    body: (
      <>
        Turn on <strong>Sleep after playback stops</strong> before you press play. When the credits end and the
        computer goes quiet and untouched, the final warning begins. Pause or touch the{" "}
        <OS mac="trackpad" win="mouse" /> and it waits again. Doze never records audio.
      </>
    ),
  },
  {
    id: "final-warning",
    tag: "Final warning",
    title: "Nothing happens without a warning you can cancel",
    body: (
      <>
        Every power action starts with a floating countdown. Snooze it for 15 minutes, cancel it, or choose Stay
        Awake. It feels at home on both systems: <OS mac="SwiftUI with Liquid Glass" win="WinUI 3 with Mica" />.
      </>
    ),
  },
  {
    id: "agents",
    tag: "For coding agents",
    title: "Finish the refactor, then go to sleep",
    body: (
      <>
        Claude Code, Codex and other MCP clients can ask Doze to stay awake while they work, then sleep when every
        agent is done. You approve each one, and a lost connection releases after 30 minutes without acting.
      </>
    ),
  },
];

const promises = [
  {
    quote: "Doze never takes a power action you didn’t ask for, and never without a warning you can cancel.",
    who: "The one rule",
  },
  {
    quote: "Silence is never treated as success. If an agent goes quiet, Doze waits, then simply lets go.",
    who: "Agents, safely",
  },
];

const audiences = [
  { title: "Developers", body: "Long builds, test suites, migrations and deploys finish before the screen locks." },
  { title: "AI coding agents", body: "Claude Code and Codex keep working overnight, then put the computer to sleep." },
  { title: "Video and 3D", body: "Exports and renders run to the end, followed by Sleep or Shut down." },
  { title: "Big downloads", body: "Games, updates and backups complete without fighting your power settings." },
  { title: "Movie nights", body: "Drift off mid-film; Doze notices the credits and lets the computer sleep." },
  { title: "Presenting", body: "Keep the display on for the length of a talk or a call, then back to normal." },
];

const faqs = [
  {
    q: "Does Doze listen to my audio?",
    a: "No. On Windows it reads the system’s output level meters; on macOS it asks Core Audio whether an output device is running and not muted. Nothing is recorded, captured or inspected, and no recording permission is requested.",
  },
  {
    q: "What happens if I’m still using the computer?",
    a: "After Playback needs both silence and inactivity before it warns you, and using the computer during the countdown cancels it. Timers always show the final warning with Cancel, Snooze and Stay Awake.",
  },
  {
    q: "What if a coding agent crashes halfway through?",
    a: "Agents renew a lease while they work. If one goes quiet, Doze keeps the computer awake for up to 30 minutes, then releases it without running any completion action.",
  },
  {
    q: "Which systems are supported?",
    a: "Windows, and macOS 13 Ventura or later. Liquid Glass appears on macOS 26 and later; earlier releases use native materials.",
  },
  {
    q: "How much does it cost?",
    a: "Nothing. Doze is completely free: no subscription, no ads and no in-app purchases.",
  },
  {
    q: "Does it phone home?",
    a: "No account, no cloud, no telemetry, no ads. Preferences and optional diagnostics stay on your computer.",
  },
];

function DownloadButton({ className = "button primary" }: { className?: string }) {
  return (
    <>
      {(["macos", "windows"] as const).map((platform) => (
        <a key={platform} className={`${className} only-${platform === "macos" ? "mac" : "win"}`} href={downloads[platform]}>
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M12 4v11m0 0-4.5-4.5M12 15l4.5-4.5M5 19h14" />
          </svg>
          Download for {platform === "macos" ? "macOS" : "Windows"}
        </a>
      ))}
    </>
  );
}

function Check() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="m5 12.5 4.5 4.5L19 7.5" />
    </svg>
  );
}

export default function Home() {
  return (
    <>
      <a className="skip" href="#main">
        Skip to content
      </a>
      <header className="nav">
        <div className="nav-inner">
          <a className="brand" href="#top" aria-label={`${brand.name} home`}>
            <Wordmark height={30} />
          </a>
          <PlatformToggle />
          <a className="button small" href="#download">
            Download
          </a>
        </div>
      </header>

      <main id="main">
        <section className="hero" id="top">
          <p className="eyebrow">
            <Mark size={16} />
            <span>Menu bar and tray utility for Mac and PC</span>
          </p>
          <h1>
            Awake when it matters.
            <span>Asleep when it doesn’t.</span>
          </h1>
          <p className="lede">
            {brand.name} keeps your <OS mac="Mac" win="PC" /> awake for a download, a render or a coding agent
            finishing the job, then lets it sleep, always after a final warning you can cancel.
          </p>
          <div className="hero-actions">
            <DownloadButton />
          </div>
          <p className="specs">
            Free · No account · No telemetry · <OS mac="macOS 13+" win="Windows 11" />
          </p>
          <div className="hero-media">
            <PlatformVideo name="hero" label="Starting a two-hour Keep Awake session from Doze’s menu" priority />
          </div>
        </section>

        <section className="intro" aria-labelledby="intro-title">
          <h2 id="intro-title">
            Your computer, on <span>your</span> schedule
          </h2>
          <p>
            One click from the <OS mac="menu bar" win="system tray" />: keep awake, set a timer, or let playback decide
            when it’s bedtime. Everything else stays out of your way.
          </p>
          <ul className="chips">
            {chips.map((chip) => (
              <li key={chip}>{chip}</li>
            ))}
          </ul>
        </section>

        <section className="evening" aria-labelledby="evening-title">
          <div className="evening-heading">
            <h2 id="evening-title">One evening with Doze</h2>
            <p>Friday · 21:40 → 00:57</p>
          </div>
          <ol className="evening-steps">
            <li>
              <div className="evening-time">
                <time dateTime="21:40">21:40</time>
                <span className="evening-dot" aria-hidden="true" />
              </div>
              <p>Start an export. <strong>Keep Awake · 2 hours.</strong></p>
            </li>
            <li>
              <div className="evening-time">
                <time dateTime="23:05">23:05</time>
                <span className="evening-dot" aria-hidden="true" />
              </div>
              <p>A film in bed, with <strong>Sleep after playback stops</strong> on.</p>
            </li>
            <li>
              <div className="evening-time">
                <time dateTime="00:52">00:52</time>
                <Mark size={16} />
              </div>
              <p>Credits end. The <strong>five-minute warning</strong> appears.</p>
            </li>
            <li className="evening-asleep">
              <div className="evening-time">
                <time dateTime="00:57">00:57</time>
                <span className="evening-dot" aria-hidden="true" />
              </div>
              <p><strong>Sleep.</strong> Nobody had to get up.</p>
            </li>
          </ol>
        </section>

        <div id="features">
          {features.map((feature, index) => (
            <div key={feature.id}>
              <section className="feature" id={feature.id === "agents" ? "agents" : undefined}>
                <p className="tag">{feature.tag}</p>
                <h2>{feature.title}</h2>
                <p className="feature-body">{feature.body}</p>
                <PlatformVideo name={feature.id} label={feature.title} />
              </section>
              {index === 1 || index === 3 ? (
                <figure className="promise">
                  <blockquote>{promises[index === 1 ? 0 : 1].quote}</blockquote>
                  <figcaption>
                    <Mark size={22} />
                    {promises[index === 1 ? 0 : 1].who}
                  </figcaption>
                </figure>
              ) : null}
            </div>
          ))}
        </div>

        <section className="cli" aria-labelledby="cli-title">
          <div className="cli-copy">
            <p className="tag">Command line</p>
            <h2 id="cli-title">Jobs without an agent</h2>
            <p>
              Wrap any command and Doze keeps the computer awake until it succeeds, then runs the action after the
              final warning. A failed job or Ctrl-C releases without acting.
            </p>
          </div>
          <pre className="terminal" aria-label="Example doze commands">
            <code>
              <span className="dim"># keep awake until the export succeeds, then sleep</span>
              {"\n"}
              <span className="prompt">$</span> doze run --then sleep -- ffmpeg -i in.mov out.mp4
              {"\n\n"}
              <span className="dim"># follow a process that is already running</span>
              {"\n"}
              <span className="prompt">$</span> doze watch --pid 1234 --then shutdown
            </code>
          </pre>
        </section>

        <section className="audiences" aria-labelledby="audiences-title">
          <h2 id="audiences-title">Built for everything that runs late</h2>
          <p>From overnight builds to the last ten minutes of a film, Doze keeps the right things going.</p>
          <ul>
            {audiences.map((item) => (
              <li key={item.title}>
                <h3>{item.title}</h3>
                <p>{item.body}</p>
              </li>
            ))}
          </ul>
        </section>

        <section className="privacy" id="privacy" aria-labelledby="privacy-title">
          <h2 id="privacy-title">Quiet by design</h2>
          <dl>
            <div>
              <dt>Local</dt>
              <dd>No account, cloud service or network endpoint. The agent bridge listens only on loopback.</dd>
            </div>
            <div>
              <dt>Private</dt>
              <dd>No telemetry and no recorded audio. Logs are opt-in and bounded.</dd>
            </div>
            <div>
              <dt>Honest</dt>
              <dd>No simulated input. Doze uses the system’s own power assertions and releases them on quit.</dd>
            </div>
          </dl>
        </section>

        <section className="download" id="download" aria-labelledby="download-title">
          <h2 id="download-title">Free. For Mac and PC.</h2>
          <p>No price, no subscription, no account. Download it and put your computer on a sensible bedtime.</p>
          <div className="download-cards">
            <article className="download-card mac">
              <h3>macOS</h3>
              <p>macOS 13 Ventura or later</p>
              <ul>
                <li>
                  <Check />
                  Menu bar app with SwiftUI settings
                </li>
                <li>
                  <Check />
                  Liquid Glass on macOS 26
                </li>
              </ul>
              <a className="button" href={downloads.macos}>
                Download for macOS
              </a>
            </article>
            <article className="download-card win">
              <h3>Windows</h3>
              <p>System tray app for Windows</p>
              <ul>
                <li>
                  <Check />
                  WinUI 3 settings with Mica
                </li>
                <li>
                  <Check />
                  Native tray menu and countdown
                </li>
              </ul>
              <a className="button" href={downloads.windows}>
                Download for Windows
              </a>
            </article>
          </div>
        </section>

        <section className="faq" id="faq" aria-labelledby="faq-title">
          <h2 id="faq-title">Questions before bed</h2>
          <div className="faq-list">
            {faqs.map((item) => (
              <details key={item.q}>
                <summary>{item.q}</summary>
                <p>{item.a}</p>
              </details>
            ))}
          </div>
        </section>

        <section className="closing">
          <Mark size={72} />
          <h2>{brand.tagline}</h2>
          <DownloadButton />
        </section>
        <aside className="clypy-promo" aria-labelledby="clypy-title">
          <div className="clypy-card">
            <Image src="/brand/clypy.png" alt="" width={80} height={80} unoptimized />
            <div className="clypy-copy">
              <p className="tag">Also from the maker of Doze</p>
              <h2 id="clypy-title">Meet Clypy.</h2>
              <p>A clipboard manager for Mac, Windows, Linux and phones.</p>
            </div>
            <a className="button primary" href="https://clypy.app">
              Check out Clypy <span aria-hidden="true">↗</span>
            </a>
          </div>
        </aside>
      </main>

      <footer className="footer">
        <div className="footer-inner">
          <Wordmark height={26} />
          <nav aria-label="Footer">
            <a href="#features">Features</a>
            <a href="#agents">Agents</a>
            <a href="#privacy">Privacy</a>
            <a href="#faq">FAQ</a>
            <a href="#download">Download</a>
          </nav>
          <p className="footer-note">Made by Fayeed Pawaskar · No telemetry, ever.</p>
        </div>
      </footer>
    </>
  );
}
