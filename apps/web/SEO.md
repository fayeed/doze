# SEO audit — 2 October 2026

Applied the [SEO Audit skill](https://github.com/coreyhaines31/marketingskills/blob/main/skills/seo-audit/SKILL.md) to the Doze landing page. Scope: repository implementation and local production responses. Goal: help people find a free keep-awake and power-timer utility for Mac and Windows. All visible copy and hero markup are preserved.

## Assessment and completed fixes

The page already has one H1, descriptive sections, accessible media labels, responsive images, and server-rendered content. The main gaps were search metadata and crawl discovery.

| Priority | Finding and evidence | Impact | Implemented fix |
| --- | --- | --- | --- |
| High | The original browser title contained only the brand and tagline. | Search results lacked a clear description of the utility. | Added a descriptive keep-awake title and concise description; left the visible headline unchanged. |
| High | No canonical metadata existed. | The preferred homepage URL was not explicit. | Added the HTTPS homepage canonical using the existing brand domain. |
| Medium | No sitemap or robots metadata routes existed. | No explicit sitemap discovery path was available. | Added `/sitemap.xml` with the homepage only and `/robots.txt` referencing it. |
| Medium | Open Graph contained only a name and tagline, with no image or Twitter metadata. | Shared links had incomplete previews. | Added consistent metadata and a generated 1200 × 630 PNG using the brand palette and existing hero headline. |
| Medium | Homepage source had no structured-data declaration. | Software and website identity were not explicitly described. | Added server-rendered WebSite and SoftwareApplication JSON-LD with supported systems, free price and existing download URLs. |
| Low | Development recording scenes had no explicit noindex metadata. | Utility routes lacked an indexing safeguard in development. | Added noindex metadata, excluded scenes from the sitemap and disallowed their crawl path. Production scenes already return 404. |

No fabricated ratings or reviews are included. SoftwareApplication markup describes the app; Google rich-result eligibility has not been confirmed.

## Validation

- Production build passed, including its type checks.
- Source-only ESLint passed. The normal lint command fails on pre-existing generated JavaScript under `apps/web/out`; no lint configuration was changed.
- Local production HTTP checks passed for canonical and social tags, parseable JSON-LD, crawl routes, a production scene 404, and the PNG response and dimensions.
- The generated social image was visually inspected.
- Compared homepage visible markup and copy data with Git HEAD, normalizing line endings: unchanged.
- Headless browser launch failed in this environment, so browser rendering and Google Rich Results validation are unverified.

## Follow-up after deployment

1. Submit `https://getdoze.app/sitemap.xml` in Google Search Console and inspect the homepage URL. No Search Console account was accessed or sitemap submitted during this work.
2. Run Google's Rich Results Test against the deployed URL and measure mobile Core Web Vitals. No live performance, indexing, traffic, or ranking claims were verified in this audit.
3. Use Search Console queries to guide any later content work. Additional feature or comparison pages should contain useful original content and need separate copy approval.

References: [Next.js social-image conventions](https://nextjs.org/docs/app/api-reference/file-conventions/metadata/opengraph-image), [Google software-app structured data](https://developers.google.com/search/docs/appearance/structured-data/software-app).
