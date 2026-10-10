# Glia

[![CI](https://github.com/draphael89/glia/actions/workflows/ci.yml/badge.svg)](https://github.com/draphael89/glia/actions/workflows/ci.yml)
&nbsp;![Platform](https://img.shields.io/badge/macOS-15%2B-blue)
&nbsp;![License](https://img.shields.io/badge/license-MIT-green)

**A native macOS window into your agent's brain — watch it learn in real time.**

![Glia growth replay](docs/replay.gif)

*Above: 6,300 pages of an agent's knowledge assembling itself over one month — the growth replay, captured live.*

Glia renders a [gbrain](https://github.com/garrytan/gbrain) knowledge base as a living
constellation: every page a lit orb, every typed link a thread, laid out with a
Barnes-Hut force engine and drawn by Metal. New knowledge blooms into a *stable*
map — your brain's geography stays where you learned it.

> Status: early. Built native-first in Swift 6 / SwiftUI / Metal. No web views.

## Why Glia exists: inject *who you are*, not just what's relevant

Seeing your brain is the hook. The thesis is bigger. As context windows grow, the
binding constraint on an agent shifts from *retrieval* (the pages relevant to your
question) to *identity* (who you are — your values, worldview, how you work). Glia
is the surface for **curating** that identity; the [`glia-context` MCP](mcp/) is how
it gets **injected** into any agent at the start of a session.

```
Glia.app  →  ~/.glia/psyche.md  →  glia-context MCP  →  any agent
(see + curate)   (your curated mind)   (injects identity + relevance)
```

We didn't just assert this — we [measured it](experiments/psyche-injection/), then
tried to break our own result. Under judges **blind** to how each answer was made
(49 independent judgments across 4 conditions), the honest finding:

- **Injecting who-you-are makes an agent measurably sharper — as a *complement* to
  what's relevant, not a replacement.** Combining identity + retrieval wins the
  blind ranking and beats retrieval-alone 71%, holding in 6 of 7 tasks.
- **Retrieval buys specificity; identity buys insight.** On neutral technical tasks
  every condition ties — the lift is specific to identity-shaped work, not a global
  "try harder."
- A first, non-blind run overclaimed (psyche *alone* dominates); that didn't survive
  blind judging. The correction is kept in the repo — [read the arc](experiments/psyche-injection/FINDINGS.md).

**Live write-up + results: [glia-sandy.vercel.app](https://glia-sandy.vercel.app)** ·
the full method, the honest arc, and the robustness ledger.

Star the pages that are the truest map of you (⌘D), export the psyche, and prime any
agent with it. See [`mcp/`](mcp/) to install the server (`cd mcp && ./install.sh`).

## Install

**Build from source** (works today — requires Xcode 16+):

```bash
git clone https://github.com/draphael89/glia && cd glia
brew install xcodegen
xcodegen generate
xcodebuild -project Glia.xcodeproj -scheme Glia -configuration Release \
  -derivedDataPath build build
open build/Build/Products/Release/Glia.app   # or just run from Xcode
```

No brain yet? Launch and click **Explore the Demo**.

### Windows community fork

[AGI Brain](https://github.com/caiomioto2/agi-brain) is a community-maintained
Windows 10/11 x64 fork, implemented natively in Rust. It includes a constellation
view, Brazilian Portuguese as the default language, local graph-file watching,
and an optional read-only SSH connection to a hosted GBrain.
See its [Windows downloads](https://github.com/caiomioto2/agi-brain/releases) and
[setup instructions](https://github.com/caiomioto2/agi-brain#readme).
Windows support is maintained in that fork; this repository remains the native
macOS implementation.

**Prebuilt DMG:** signed + notarized releases land on the
[Releases](https://github.com/draphael89/glia/releases) page once code-signing
is set up (tag a `v*` and CI builds it — see `docs/RELEASING.md`). A
Homebrew cask (`packaging/glia.rb`) follows the first notarized release.

## Features

- **The graph** — GPU-rendered (instanced SDF orbs, MSAA, 120Hz), colored by page
  type, sized by connectivity. Pinch to dive, two-finger pan, double-click to fly.
- **Growth replay** — scrub or play the brain's history and watch months of
  learning assemble itself in seconds. New pages bloom in place.
- **⌘K search** — type, arrow, return: the camera flies to the node.
- **Focus mode** — click a node: its neighborhood stays lit, everything else
  recedes. Inspector shows type, dates, connections, and deep-links into Obsidian.
- **Stable spatial memory** — layout settles once and persists. No perpetual
  physics soup; insertions relax locally and the map you know stays the map.
- **Menu-bar presence** — page count and today's activity at a glance.

## Data

First launch with no data shows a friendly empty state: **Explore the Demo**
(a bundled synthetic brain — try every feature immediately) or **Choose Brain
Folder…** (⌘O; sandbox-safe security-scoped bookmark, remembered across
launches). By default Glia reads a JSON export at `~/.gbrain/viz/graph.json`:

```json
{
  "generated_at": "…",
  "nodes": [{ "id": 1, "slug": "…", "type": "…", "source": "…",
              "title": "…", "created": "YYYY-MM-DD", "updated": "ISO8601" }],
  "links": [{ "source": 1, "target": 2, "type": "mentions" }]
}
```

Any exporter that produces this shape works. A gbrain ops-contract client
(`export_graph` / `get_brain_identity` polling) is the planned first-class
backend — no direct database coupling, works with PGLite and Postgres brains.

## Build

Requires Xcode 16+ and [XcodeGen](https://github.com/yonaskolb/XcodeGen).

```bash
xcodegen generate
xcodebuild -project Glia.xcodeproj -scheme Glia -configuration Release build
```

### Snapshot mode (for development)

Deterministic offscreen renders for pixel iteration:

```bash
GLIA_SNAPSHOT=/tmp/glia.png Glia.app/Contents/MacOS/Glia            # overview
GLIA_SNAPSHOT_FOCUS=people/some-slug …                              # focus mode
GLIA_SNAPSHOT_REPLAY=0.3 …                                          # replay state
```

## Performance budget

Measured on Apple Silicon, MSAA 4×, GPU frame time at Retina (2880×1800),
across brain sizes (synthetic stress graphs, hub-and-spoke topology):

| Brain size | GPU / frame | Interaction |
|---|---|---|
| 6.6k nodes (a real brain today) | ~0.6 ms | 120 fps |
| 20k nodes (a year or two out) | ~2.8 ms | 120 fps |
| 100k nodes (design ceiling) | ~20 ms | ~50 fps |

Two things keep this honest:

- **The render loop is on-demand.** When nothing animates, Glia draws **0
  frames** and sits at 0% GPU — so a static view of *any* size is free. The
  numbers above are the cost *during* pan/zoom/settle only.
- **Layout is a one-time background settle.** ~1 s for a few thousand nodes,
  longer for 100k, off the main thread — then positions persist, so you never
  pay it again.

At every realistic size Glia is comfortably within the 120 Hz budget. Edges
dominate the cost at extreme scale (a dense web is ~80% of the fill), so
they draw at a zoom-gated LOD — a representative subset when the whole graph
is a distant haze, ramping to every edge as you dive into a neighborhood
where each connection matters. That keeps even the 100k ceiling interactive.

## Not in v1 (on purpose)

Editing pages (Glia is read-only by design) · backend pluggability beyond the
JSON contract · embedding-space visualization · chat · multi-brain management.

## License

[MIT](LICENSE)
