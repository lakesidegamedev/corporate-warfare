# Visual specification v1.1

Corporate Warfare reads as an economic strategy game played over a real map, in the
manner of A House Divided's corporations: sector types, operating strategies, plants in
states. It must not read as an operations dashboard or a SaaS settings form. Native
SwiftUI on iPhone and iPad; iPad is a first-class layout, not a stretched phone.

## Tokens

One dark cartographic palette in both system appearances (the app sets dark):
ink #0B1622 (sea, canvas), panel #13212F, raised panel #1A2B3C, stroke #22364A,
play-state land #2B3B4A, neighbouring land #1E2A36, water #163555, interstate #C68A3D,
text #E8EEF5, secondary #93A4B8, gold #E2B04A (money and the primary action),
positive #3FBF8A, negative #E5605E, warning #F2B748, copper #C9804A (inputs).
Positive and negative always carry a sign or word as well as colour.

Each AHD sector type has one colour and one SF Symbol, used identically on map markers,
lists, the build flow and the CEO office (`Sector` in `Design.swift`). Section titles are
uppercase tracked eyebrows; figures use monospaced digits; large values abbreviate with an
accessible full value. Controls at least 44x44pt.

## Navigation and layout

Five tabs: Map (map), CEO Office (building.2), Markets (chart.bar.xaxis), Inbox
(tray.full, badged with mandatory decisions), Portfolio (building.columns).

Map is home (1.2.0: regions). A flat vector map (not satellite imagery) of the eight playable states with
counties, the Great Lakes, major rivers, interstates and hub cities, drawn from public
Census, TIGER and Natural Earth data bundled at build time. Neighbouring states are drawn
dimmer. Thin lines mark the 31 economic regions, thick lines the states. In the Your divisions layer
the player's divisions appear as one gold-ringed sector circle per owned sector type beside
each region's anchor city, with a badge counting its plants; rival plants are not drawn. Layers: presence, room to build, wages, workforce skill, each with a legend.
Pan, pinch zoom, double-tap reset, tap a region to open its board. On iPhone the board is
a sheet with half and full detents; on iPad it is a fixed side panel beside the map.

A state's board leads with one Build action, then room to build (smoothed imports),
conditions (grid, roads, skill, broadband, wages, cost of living, crime, union strength)
and the plants operating there. Build is a flow: sector type (with focus labels) then
strategy cards (output, inputs, capex, build time, staff, reference margin, local import
room) then size against the engine's own preview.

CEO Office is the corporation: headline figures, sector focus with its transition and
cooldown, logistics budget and strength, and plants grouped by sector with retooling.
Markets is a per-state commodity board grouped by sector: last price against reference,
weekly sales, stock, gateway prices, import room. Corporation founding sets headquarters,
primary sector, founding strategy and an optional secondary sector.

Action forms are sheets with visible actor, units, computed preview and one confirm.
Errors stay inline with the reason and a remedy. No Rust, FFI, ID or version text in
normal product screens.

## Motion and assets

Short 150-200ms transitions only; Reduce Motion removes map zoom animation. Haptics only
for a committed material action or urgent alert. App icon: original asset in every
required Apple size; never a placeholder.

## Evidence

The Apple workflow renders snapshots of Map, a state board, the build flow, CEO Office,
Markets and Inbox on iPhone and iPad from a deterministic ten-week career and uploads
them as the `ui-snapshots` artifact. Review them before release.
