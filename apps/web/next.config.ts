import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  poweredByHeader: false,
  // Keeps the development badge out of the scenes recorded by scripts/render-media.mjs.
  devIndicators: false,
};

export default nextConfig;
