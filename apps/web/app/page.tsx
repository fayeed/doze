import { brand } from "@doze/brand";

export default function Home() {
  return (
    <main className="shell">
      <div className="moon" aria-hidden="true">☾</div>
      <p className="eyebrow">A quieter kind of utility</p>
      <h1>{brand.name}</h1>
      <p className="tagline">{brand.tagline}</p>
    </main>
  );
}
