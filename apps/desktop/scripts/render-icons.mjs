import { existsSync } from "node:fs";
import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import puppeteer from "puppeteer-core";

// Builds src-tauri/icons from the brand artwork in apps/web/public/brand. Small sizes use the
// simplified 16–32 px drawings; Windows gets the freestanding mark, macOS the rounded tile.
// Needs Chrome (or CHROME_PATH): pnpm --filter @doze/desktop icons
const brand = new URL("../../web/public/brand/", import.meta.url);
const icons = new URL("../src-tauri/icons/", import.meta.url);

const chrome =
  process.env.CHROME_PATH ??
  [
    "C:/Program Files/Google/Chrome/Application/chrome.exe",
    "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/google-chrome",
  ].find((candidate) => existsSync(candidate));
if (!chrome) throw new Error("Chrome not found. Set CHROME_PATH.");

const svg = async (name) =>
  (await readFile(new URL(name, brand))).toString("base64");
const art = {
  windows: await svg("doze-icon-windows.svg"),
  windowsSmall: await svg("doze-icon-windows-16-32.svg"),
  macos: await svg("doze-icon-macos.svg"),
  macosSmall: await svg("doze-icon-macos-16-32.svg"),
};

const browser = await puppeteer.launch({
  executablePath: chrome,
  headless: true,
});
const page = await browser.newPage();
async function png(source, size) {
  await page.setViewport({ width: size, height: size, deviceScaleFactor: 1 });
  await page.setContent(
    `<body style="margin:0;background:transparent"><img src="data:image/svg+xml;base64,${source}" width="${size}" height="${size}" style="display:block"></body>`,
  );
  await page.waitForFunction(() => document.images[0].complete);
  return page.screenshot({ type: "png", omitBackground: true });
}

/** ICO with PNG-compressed entries, which Windows Vista and later read natively. */
function ico(entries) {
  const header = Buffer.alloc(6 + entries.length * 16);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(entries.length, 4);
  let offset = header.length;
  entries.forEach(([size, data], index) => {
    const entry = 6 + index * 16;
    header.writeUInt8(size >= 256 ? 0 : size, entry);
    header.writeUInt8(size >= 256 ? 0 : size, entry + 1);
    header.writeUInt16LE(1, entry + 4);
    header.writeUInt16LE(32, entry + 6);
    header.writeUInt32LE(data.length, entry + 8);
    header.writeUInt32LE(offset, entry + 12);
    offset += data.length;
  });
  return Buffer.concat([header, ...entries.map(([, data]) => data)]);
}

/** ICNS with PNG entries keyed by Apple's type codes. */
function icns(entries) {
  const chunks = entries.map(([type, data]) => {
    const head = Buffer.alloc(8);
    head.write(type, 0, "ascii");
    head.writeUInt32BE(data.length + 8, 4);
    return Buffer.concat([head, data]);
  });
  const head = Buffer.alloc(8);
  head.write("icns", 0, "ascii");
  head.writeUInt32BE(
    8 + chunks.reduce((sum, chunk) => sum + chunk.length, 0),
    4,
  );
  return Buffer.concat([head, ...chunks]);
}

try {
  const windows = async (size) =>
    png(size <= 32 ? art.windowsSmall : art.windows, size);
  const macos = async (size, small = size <= 32) =>
    png(small ? art.macosSmall : art.macos, size);

  for (const [file, size] of [
    ["32x32.png", 32],
    ["64x64.png", 64],
    ["128x128.png", 128],
    ["128x128@2x.png", 256],
    ["icon.png", 512],
  ]) {
    await writeFile(new URL(file, icons), await windows(size));
  }

  // One page renders every size, so each waits for the previous one.
  const icoEntries = [];
  for (const size of [16, 20, 24, 32, 40, 48, 64, 256]) {
    icoEntries.push([size, await windows(size)]);
  }
  await writeFile(new URL("icon.ico", icons), ico(icoEntries));

  // 16 pt and 32 pt slots keep the simplified tile at both scales.
  const icnsEntries = [
    ["icp4", 16, true],
    ["ic11", 32, true],
    ["icp5", 32, true],
    ["ic12", 64, true],
    ["icp6", 64, false],
    ["ic07", 128, false],
    ["ic13", 256, false],
    ["ic08", 256, false],
    ["ic14", 512, false],
    ["ic09", 512, false],
    ["ic10", 1024, false],
  ];
  const entries = [];
  for (const [type, size, small] of icnsEntries)
    entries.push([type, await macos(size, small)]);
  await writeFile(new URL("icon.icns", icons), icns(entries));
  console.log(`Wrote icons to ${fileURLToPath(icons)}`);
} finally {
  await browser.close();
}
