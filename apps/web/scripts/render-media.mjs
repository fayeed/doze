import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import puppeteer from "puppeteer-core";

// Records every scene in components/scenes for macOS and Windows into public/media.
// Start the dev server first (pnpm dev:web), then: pnpm --filter @doze/web media [scene…]
// Needs Chrome (or CHROME_PATH) and ffmpeg with libx264 on PATH.
const base = process.env.SCENES_URL ?? "http://localhost:3000";
const fps = Number(process.env.FPS ?? 30);
const scenes = {
  hero: 4.2,
  "keep-awake": 4.6,
  "power-timer": 5.2,
  "after-playback": 5.6,
  "final-warning": 1.0,
  agents: 3.35,
};
const platforms = ["macos", "windows"];
const only = process.argv.slice(2);
const output = fileURLToPath(new URL("../public/media/", import.meta.url));

const chrome =
  process.env.CHROME_PATH ??
  [
    "C:/Program Files/Google/Chrome/Application/chrome.exe",
    "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/google-chrome",
  ].find((candidate) => existsSync(candidate));
if (!chrome) throw new Error("Chrome not found. Set CHROME_PATH.");

try {
  await fetch(base);
} catch {
  console.error(`No dev server at ${base}. Run pnpm dev:web first, or set SCENES_URL.`);
  process.exit(1);
}

function encoder(file) {
  const ffmpeg = spawn(
    "ffmpeg",
    ["-loglevel", "error", "-y", "-f", "image2pipe", "-framerate", String(fps), "-c:v", "mjpeg", "-i", "-",
      // JPEG frames are full range; convert to the limited-range yuv420p every browser expects.
      "-vf", "scale=in_range=pc:out_range=tv,format=yuv420p", "-color_range", "tv",
      "-c:v", "libx264", "-preset", "slow", "-crf", "23", "-movflags", "+faststart", file],
    { stdio: ["pipe", "inherit", "inherit"] },
  );
  const done = new Promise((resolve, reject) =>
    ffmpeg.on("exit", (code) => (code === 0 ? resolve() : reject(new Error(`ffmpeg exited with ${code}`)))),
  );
  return { stdin: ffmpeg.stdin, done };
}

const browser = await puppeteer.launch({
  executablePath: chrome,
  headless: true,
  args: ["--hide-scrollbars", "--force-color-profile=srgb"],
});
try {
  const page = await browser.newPage();
  await page.setViewport({ width: 960, height: 600, deviceScaleFactor: 2 });
  for (const [name, posterTime] of Object.entries(scenes)) {
    if (only.length && !only.includes(name)) continue;
    for (const platform of platforms) {
      await mkdir(path.join(output, platform), { recursive: true });
      await page.goto(`${base}/scenes/${name}?platform=${platform}&capture`, { waitUntil: "networkidle0" });
      await page.waitForFunction(() => typeof window.__dozeSeek === "function", { timeout: 60000 });
      await page.evaluate(() => document.fonts.ready);
      const duration = await page.evaluate(() => window.__dozeDuration);
      const seek = (t) => page.evaluate((value) => window.__dozeSeek(value), t);

      await seek(posterTime);
      await writeFile(
        path.join(output, platform, `${name}.webp`),
        await page.screenshot({ type: "webp", quality: 82 }),
      );

      const file = path.join(output, platform, `${name}.mp4`);
      const { stdin, done } = encoder(file);
      const frames = Math.round(duration * fps);
      for (let frame = 0; frame < frames; frame++) {
        await seek(frame / fps);
        const jpeg = await page.screenshot({ type: "jpeg", quality: 92, optimizeForSpeed: true });
        if (!stdin.write(jpeg)) await new Promise((resolve) => stdin.once("drain", resolve));
      }
      stdin.end();
      await done;
      console.log(`Rendered ${platform}/${name}.mp4 (${frames} frames)`);
    }
  }
} finally {
  await browser.close();
}
