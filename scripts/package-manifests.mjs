import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

export function homebrewCask(version, sha256) {
  return `cask "doze" do
  version "${version}"
  sha256 "${sha256}"

  url "https://github.com/fayeed/doze/releases/download/v#{version}/Doze-macos-universal.dmg"
  name "Doze"
  desc "Manage sleep around your work and coding agents"
  homepage "https://getdoze.app"

  depends_on macos: ">= :ventura"

  app "Doze.app"

  zap trash: ["~/Library/Application Support/app.getdoze.desktop"]
end
`;
}

export function wingetManifests(version, sha256) {
  const common = `PackageIdentifier: Fayeed.Doze
PackageVersion: ${version}
`;
  return {
    "Fayeed.Doze.yaml": `# yaml-language-server: $schema=https://aka.ms/winget-manifest.version.1.10.0.schema.json
ManifestType: version
ManifestVersion: 1.10.0
PackageIdentifier: Fayeed.Doze
PackageVersion: ${version}
DefaultLocale: en-US
ManifestFiles:
  - InstallerLocale: en-US
    RelativeFilePath: Fayeed.Doze.locale.en-US.yaml
  - RelativeFilePath: Fayeed.Doze.installer.yaml
`,
    "Fayeed.Doze.locale.en-US.yaml": `# yaml-language-server: $schema=https://aka.ms/winget-manifest.defaultLocale.1.10.0.schema.json
${common}PackageLocale: en-US
Publisher: Fayeed Pawaskar
PublisherUrl: https://getdoze.app
PackageName: Doze
License: MIT
LicenseUrl: https://github.com/fayeed/doze/blob/main/LICENSE.md
ShortDescription: Manage sleep around your work and coding agents
ManifestType: defaultLocale
ManifestVersion: 1.10.0
`,
    "Fayeed.Doze.installer.yaml": `# yaml-language-server: $schema=https://aka.ms/winget-manifest.installer.1.10.0.schema.json
${common}Platform:
  - Windows.Desktop
MinimumOSVersion: 10.0.22000.0
Installers:
  - Architecture: x64
    InstallerType: nullsoft
    InstallerUrl: https://github.com/fayeed/doze/releases/download/v${version}/Doze-windows-x64-setup.exe
    InstallerSha256: ${sha256}
    InstallerSwitches:
      Silent: /S
      SilentWithProgress: /S
    UpgradeBehavior: install
ManifestType: installer
ManifestVersion: 1.10.0
`,
  };
}

async function main() {
  const [version, macSha, windowsSha, output] = process.argv.slice(2);
  if (!/^\d+\.\d+\.\d+$/.test(version ?? "") || !/^[a-f\d]{64}$/i.test(macSha ?? "") || !/^[a-f\d]{64}$/i.test(windowsSha ?? "") || !output) {
    throw new Error("Usage: node scripts/package-manifests.mjs VERSION MAC_SHA256 WINDOWS_SHA256 OUTPUT_DIRECTORY");
  }
  const folder = path.join(output, version);
  await mkdir(folder, { recursive: true });
  await writeFile(path.join(folder, "doze.rb"), homebrewCask(version, macSha));
  const manifests = wingetManifests(version, windowsSha);
  for (const [name, contents] of Object.entries(manifests)) await writeFile(path.join(folder, name), contents);
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  main().catch(error => { console.error(error.message); process.exitCode = 1; });
}
