import { brand } from "@doze/brand";
import { Countdown } from "@/components/countdown";

const releases = `${brand.repository}/releases/latest`;

const features = [
  {
    title: "Keep Awake",
    body: "Fifteen minutes, two hours, until 11:30, or indefinitely. Extend or stop from the menu bar or tray; the time left sits beside the icon.",
    tag: "Sessions",
  },
  {
    title: "Power Timer",
    body: "Sleep, shut down, lock or turn the display off on a timer or at a set time. Your computer stays awake until the timer finishes.",
    tag: "Timers",
  },
  {
    title: "After Playback",
    body: "Falling asleep to a film? Once playback stops and you've stopped using the computer, Doze starts the final warning. Pause or touch the trackpad and it waits again.",
    tag: "Audio",
  },
  {
    title: "A final warning, always",
    body: "Nothing happens without a floating countdown first. Cancel it, snooze it for 15 minutes, or choose Stay Awake.",
    tag: "Safety",
  },
  {
    title: "Wake leases for agents",
    body: "Codex, Claude Code and other MCP clients can ask Doze to stay awake while they finish, then return to normal or sleep, only with your approval.",
    tag: "Agents",
  },
  {
    title: "Native on both systems",
    body: "SwiftUI with Liquid Glass on macOS, WinUI 3 with Mica on Windows, and a small Rust engine that owns every power request.",
    tag: "Craft",
  },
];

const faqs = [
  {
    q: "Does Doze listen to my audio?",
    a: "No. On Windows it reads the system's output level meters; on macOS it asks Core Audio whether an output device is running and not muted. Nothing is recorded, captured or inspected, and no recording permission is requested.",
  },
  {
    q: "What happens if I'm still using the computer?",
    a: "After Playback needs both silence and inactivity before it warns you, and using the computer during the countdown cancels it. Timers always show the final warning with Cancel, Snooze and Stay Awake.",
  },
  {
    q: "What if a coding agent crashes halfway through?",
    a: "Agents renew a lease while they work. If one goes quiet, Doze keeps the computer awake for up to 30 minutes, then releases it without running any completion action. Silence is never treated as success.",
  },
  {
    q: "Which systems are supported?",
    a: "Windows, and macOS 13 Ventura or later. Liquid Glass appears on macOS 26 and later; earlier releases use native materials.",
  },
  {
    q: "Does it phone home?",
    a: "No account, no cloud, no telemetry, no ads. Preferences and optional diagnostics stay on your computer.",
  },
];

export default function Home() {
  return (
    <>
      <a className="skip" href="#main">
        Skip to content
      </a>
      <header className="masthead">
        <a className="wordmark" href="#top" aria-label={`${brand.name} home`}>
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5Z" />
          </svg>
          {brand.name}
        </a>
        <nav aria-label="Primary">
          <a href="#features">Features</a>
          <a href="#agents">Agents</a>
          <a href="#faq">Questions</a>
          <a className="nav-cta" href={releases}>
            Download
          </a>
        </nav>
      </header>

      <main id="main">
        <section className="hero" id="top">
          <div className="hero-copy">
            <p className="kicker">A menu bar and tray utility for Mac and PC</p>
            <h1>
              Stay up <em>exactly</em> as late as you mean to.
            </h1>
            <p className="lede">
              {brand.name} keeps your computer awake while it matters: a download, a render, a coding agent
              finishing the job. Then it lets it sleep. {brand.tagline}
            </p>
            <div className="cta-row">
              <a className="button primary" href={releases}>
                Download for macOS
              </a>
              <a className="button" href={releases}>
                Download for Windows
              </a>
            </div>
            <p className="fineprint">Free download from GitHub Releases · No account · No telemetry</p>
          </div>
          <Countdown />
        </section>

        <section className="night" aria-labelledby="timeline-title">
          <h2 id="timeline-title" className="section-label">
            One evening with Doze
          </h2>
          <ol className="timeline">
            <li>
              <time>21:40</time>
              <p>
                You start a large export and choose <strong>Keep Awake · 2 hours</strong>.
              </p>
            </li>
            <li>
              <time>23:05</time>
              <p>
                A film plays in bed. <strong>Sleep after playback stops</strong> is on.
              </p>
            </li>
            <li>
              <time>00:52</time>
              <p>The credits end. Silence, no input. A five-minute warning appears.</p>
            </li>
            <li>
              <time>00:57</time>
              <p>
                <strong>Sleep.</strong> Your computer knows when it&rsquo;s bedtime.
              </p>
            </li>
          </ol>
        </section>

        <section className="features" id="features" aria-labelledby="features-title">
          <div className="section-head">
            <h2 id="features-title">Everything waits for you to say goodnight.</h2>
            <p>
              Six habits, one rule: {brand.name} never takes a power action you didn&rsquo;t ask for, and
              never without a warning you can cancel.
            </p>
          </div>
          <ul className="grid">
            {features.map((feature, index) => (
              <li key={feature.title} className="card" style={{ "--i": index } as React.CSSProperties}>
                <span className="card-tag">{feature.tag}</span>
                <h3>{feature.title}</h3>
                <p>{feature.body}</p>
              </li>
            ))}
          </ul>
        </section>

        <section className="agents" id="agents" aria-labelledby="agents-title">
          <div className="agents-copy">
            <p className="kicker">For coding agents</p>
            <h2 id="agents-title">&ldquo;Finish the refactor, then put the computer to sleep.&rdquo;</h2>
            <p>
              Doze speaks the Model Context Protocol over local stdio. An agent asks for a session, you
              approve it once or grant it in Settings, and the agent renews a lease while it works. When
              every overlapping agent finishes with the same action, the usual final warning begins.
            </p>
            <ul className="steps">
              <li>
                <span className="step">1</span>
                <p>
                  Enable MCP and choose <strong>Set up</strong> for Codex, Claude Code or any client.
                </p>
              </li>
              <li>
                <span className="step">2</span>
                <p>
                  Approve the first request with <strong>Allow Once</strong>.
                </p>
              </li>
              <li>
                <span className="step">3</span>
                <p>Lost connections keep the computer awake for up to 30 minutes, then release without acting.</p>
              </li>
            </ul>
          </div>
          <pre className="terminal" aria-label="Example MCP tool calls">
            <code>
              <span className="dim">{"// agent → doze"}</span>
              {"\n"}doze.start_session({"{"}
              {"\n"}  reason: <span className="str">&quot;Finish the requested refactor&quot;</span>,
              {"\n"}  completion_action: <span className="str">&quot;sleep&quot;</span>
              {"\n"}
              {"}"})
              {"\n"}
              <span className="dim">{"// → awaiting_authorization … active"}</span>
              {"\n\n"}doze.heartbeat(session_id)
              {"   "}
              <span className="dim">{"// every few minutes"}</span>
              {"\n"}doze.finish_session(session_id)
              {"\n"}
              <span className="dim">{"// → final warning, then Sleep"}</span>
            </code>
          </pre>
        </section>

        <section className="privacy" aria-labelledby="privacy-title">
          <h2 id="privacy-title">Quiet by design.</h2>
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
              <dd>No simulated input. Doze uses the system&rsquo;s own power assertions and releases them on quit.</dd>
            </div>
          </dl>
        </section>

        <section className="faq" id="faq" aria-labelledby="faq-title">
          <h2 id="faq-title">Questions before bed</h2>
          {faqs.map((item) => (
            <details key={item.q}>
              <summary>{item.q}</summary>
              <p>{item.a}</p>
            </details>
          ))}
        </section>
      </main>

      <footer className="footer">
        <p className="footer-mark">
          {brand.name}
          <span>{brand.tagline}</span>
        </p>
        <nav aria-label="Footer">
          <a href={releases}>Download</a>
          <a href={brand.repository}>Source</a>
          <a href={`${brand.repository}/blob/main/apps/desktop/README.md#agents-and-mcp`}>MCP guide</a>
        </nav>
      </footer>
    </>
  );
}
