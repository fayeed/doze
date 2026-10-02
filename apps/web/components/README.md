# Web components

- `platform.tsx` — the macOS / Windows toggle and `PlatformVideo`, which plays the clip recorded for the platform being shown. `lib/platform.ts` picks the platform before first paint (a `?platform=` link, then the visitor's last choice, then their OS) and stores it on `<html data-platform>`.
- `os.tsx` — `OS` for copy that differs by platform, and the logo mark.
- `scenes/` — recreations of Doze's menus, Settings, timer and countdown windows on each platform. They are the source of every clip in `public/media/{macos,windows}`.

## Re-recording the clips

Scenes are served at `/scenes/<name>?platform=macos|windows` in development only. With the dev server running, record them all (or name a few):

```sh
pnpm dev:web
pnpm --filter @doze/web media            # every scene, both platforms
pnpm --filter @doze/web media hero agents
```

The script needs Chrome (or `CHROME_PATH`) and ffmpeg with libx264 on `PATH`. It steps each scene's timeline frame by frame, so recordings are identical between runs, and writes an H.264 `.mp4` plus a `.webp` poster per scene and platform. Set `SCENES_URL` if the dev server is not on port 3000.
