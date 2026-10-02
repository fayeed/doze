import type { Metadata, Viewport } from "next";
import { Inter, JetBrains_Mono } from "next/font/google";
import { brand } from "@doze/brand";
import { platformScript } from "@/lib/platform";
import "./globals.css";

const sans = Inter({ subsets: ["latin"], variable: "--font-sans" });
const mono = JetBrains_Mono({ subsets: ["latin"], weight: ["400", "500"], variable: "--font-mono" });

export const metadata: Metadata = {
  title: `${brand.name} · ${brand.tagline}`,
  description:
    "Doze keeps your Mac or PC awake while it matters, then lets it sleep: keep-awake sessions, power timers, sleep after playback, and wake leases for coding agents. Local and private.",
  metadataBase: new URL(`https://${brand.domain}`),
  openGraph: { title: brand.name, description: brand.tagline, type: "website" },
};

export const viewport: Viewport = { themeColor: brand.colors.paperText };

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    // The platform script sets data-platform before hydration.
    <html lang="en" className={`${sans.variable} ${mono.variable}`} suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: platformScript }} />
      </head>
      <body>{children}</body>
    </html>
  );
}
