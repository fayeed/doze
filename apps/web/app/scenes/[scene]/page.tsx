import { notFound } from "next/navigation";
import { ScenePlayer } from "@/components/scenes/player";

// Source scenes for the site's videos. They exist only in development, where
// scripts/render-media.mjs records them; the published site ships the encoded clips.
export default async function ScenePage({
  params,
  searchParams,
}: {
  params: Promise<{ scene: string }>;
  searchParams: Promise<{ platform?: string }>;
}) {
  if (process.env.NODE_ENV === "production") notFound();
  const { scene } = await params;
  const { platform } = await searchParams;
  return <ScenePlayer name={scene} platform={platform === "windows" ? "windows" : "macos"} />;
}
