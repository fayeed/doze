import { brand } from "@doze/brand";
import config from "../../../release.config.json";

const base = (process.env.DOZE_DOWNLOAD_BASE_URL || config.publicBaseUrl).replace(/\/$/, "");
const fallback = `${brand.repository}/releases/latest`;

// Stable R2 keys: publishing either OS updates only that OS's download.
export const downloads = {
  windows: base ? `${base}/releases/latest/Doze-windows-x64-setup.exe` : fallback,
  macos: base ? `${base}/releases/latest/Doze-macos-universal.dmg` : fallback,
};
