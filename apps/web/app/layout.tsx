import type { Metadata, Viewport } from "next";
import { Inter, JetBrains_Mono } from "next/font/google";
import { Analytics } from "@vercel/analytics/next";
import { brand } from "@doze/brand";
import { platformScript } from "@/lib/platform";
import { seoDescription, seoTitle, siteUrl } from "@/lib/seo";
import "./globals.css";

const sans = Inter({ subsets: ["latin"], variable: "--font-sans" });
const mono = JetBrains_Mono({ subsets: ["latin"], weight: ["400", "500"], variable: "--font-mono" });

export const metadata: Metadata = {
  title: seoTitle,
  description: seoDescription,
  metadataBase: new URL(siteUrl),
  applicationName: brand.name,
  openGraph: {
    title: seoTitle,
    description: seoDescription,
    siteName: brand.name,
    url: siteUrl,
    locale: "en_US",
    type: "website",
    images: [{ url: "/opengraph-image", width: 1200, height: 630, alt: brand.tagline }],
  },
  twitter: {
    card: "summary_large_image",
    title: seoTitle,
    description: seoDescription,
    images: [{ url: "/opengraph-image", alt: brand.tagline }],
  },
};

export const viewport: Viewport = { themeColor: brand.colors.paperText };

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    // The platform script sets data-platform before hydration.
    <html lang="en" className={`${sans.variable} ${mono.variable}`} suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: platformScript }} />
      </head>
      <body>
        {children}
        <Analytics />
      </body>
    </html>
  );
}
