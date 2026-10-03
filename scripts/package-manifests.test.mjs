import assert from "node:assert/strict";
import test from "node:test";
import { homebrewCask, wingetManifests } from "./package-manifests.mjs";

const hash = "a".repeat(64);
test("Homebrew cask uses immutable release URL and artifact digest", () => {
  const cask = homebrewCask("0.2.0", hash);
  assert.match(cask, /version "0\.2\.0"/);
  assert.match(cask, /releases\/download\/v#\{version\}\/Doze-macos-universal\.dmg/);
  assert.match(cask, new RegExp(`sha256 "${hash}"`));
});
test("WinGet manifests identify the signed x64 NSIS installer and matching checksum", () => {
  const manifests = wingetManifests("0.2.0", hash);
  assert.equal(Object.keys(manifests).length, 3);
  assert.match(manifests["Fayeed.Doze.installer.yaml"], /PackageIdentifier: Fayeed\.Doze/);
  assert.match(manifests["Fayeed.Doze.installer.yaml"], /InstallerType: nullsoft/);
  assert.match(manifests["Fayeed.Doze.installer.yaml"], /Silent: \/S/);
  assert.ok(manifests["Fayeed.Doze.installer.yaml"].includes(hash));
  assert.ok(Object.values(manifests).every(value => value.includes("ManifestVersion: 1.10.0")));
});
