import { ImageResponse } from "next/og";
import { brand } from "@doze/brand";

export const alt = `${brand.name}: Awake when it matters. Asleep when it doesn’t.`;
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

export default function OpenGraphImage() {
  return new ImageResponse(
    (
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          justifyContent: "center",
          width: "100%",
          height: "100%",
          padding: "80px",
          background: brand.colors.paper,
          color: brand.colors.ink,
          fontFamily: "sans-serif",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 20, marginBottom: 44 }}>
          <div
            style={{
              display: "flex",
              width: 56,
              height: 56,
              borderRadius: "50%",
              background: `linear-gradient(${brand.colors.sunTop}, ${brand.colors.sunBottom})`,
            }}
          />
          <span style={{ fontSize: 42, fontWeight: 700 }}>{brand.name}</span>
        </div>
        <div style={{ display: "flex", flexDirection: "column", fontSize: 64, fontWeight: 700, lineHeight: 1.15 }}>
          <span>Awake when it matters.</span>
          <span style={{ color: brand.colors.sunBottom }}>Asleep when it doesn’t.</span>
        </div>
        <div style={{ display: "flex", marginTop: 42, fontSize: 26 }}>
          Free · No account · No telemetry · Mac and PC
        </div>
        <div style={{ display: "flex", marginTop: 22, fontSize: 22, opacity: 0.6 }}>{brand.domain}</div>
      </div>
    ),
    size,
  );
}
