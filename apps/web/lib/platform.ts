export type Platform = "macos" | "windows";

export const platformNames: Record<Platform, string> = { macos: "macOS", windows: "Windows" };

const storageKey = "doze-platform";
export const platformEvent = "doze:platform";

/**
 * Runs before first paint and marks <html data-platform>, so platform-specific copy and
 * media never flash. A ?platform= link wins, then the visitor's last choice, then their OS.
 */
export const platformScript = `(function(){var p;try{p=new URLSearchParams(location.search).get("platform")||localStorage.getItem("${storageKey}")}catch(e){}if(p!=="macos"&&p!=="windows"){var n=navigator.userAgentData&&navigator.userAgentData.platform||navigator.platform||navigator.userAgent||"";p=/win/i.test(n)?"windows":"macos"}document.documentElement.dataset.platform=p})()`;

export function readPlatform(): Platform {
  return document.documentElement.dataset.platform === "windows" ? "windows" : "macos";
}

export function choosePlatform(platform: Platform) {
  document.documentElement.dataset.platform = platform;
  try {
    localStorage.setItem(storageKey, platform);
  } catch {
    // Private windows may block storage; the choice still applies to this visit.
  }
  window.dispatchEvent(new Event(platformEvent));
}
