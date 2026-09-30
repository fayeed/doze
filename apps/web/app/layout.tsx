import type { Metadata } from "next";
import { brand } from "@doze/brand";
import "./globals.css";

export const metadata: Metadata = {
  title: brand.name,
  description: brand.tagline,
  metadataBase: new URL(`https://${brand.domain}`),
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
