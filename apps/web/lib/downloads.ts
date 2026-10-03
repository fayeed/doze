import { brand } from "@doze/brand";

const latest = `${brand.repository}/releases/latest/download`;

export const downloads = {
  windows: `${latest}/Doze-windows-x64-setup.exe`,
  macos: `${latest}/Doze-macos-universal.dmg`,
};
