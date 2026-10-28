# iPad Split View and Slide Over (#136)

Snapshots from the Apple smoke run 40649794587 (commit 58de1f3), scaled to one pixel per point. Each
renders a live ten-week career on a 13-inch iPad (1032 x 1376 points) at the width and size class iPadOS
gives the app in Split View or Slide Over. The app does not set UIRequiresFullScreen.

| Slice | Window | Size class | Layout |
|---|---|---|---|
| portrait-third | 320 x 1376 | compact | phone tabs |
| portrait-half | 507 x 1376 | compact | phone tabs |
| portrait-two-thirds | 694 x 1376 | compact | phone tabs |
| landscape-third (Slide Over width) | 375 x 1032 | compact | phone tabs |
| landscape-half | 683 x 1032 | regular | sidebar shell |
| landscape-two-thirds | 993 x 1032 | regular | sidebar shell |

Each slice has four files, `split-<slice>-<screen>.png`:

- `map`: the map home with its company strip, layer chips and zoom controls (tabs or sidebar shell).
- `markets`: the markets screen with the stock board (tabs or sidebar shell).
- `company`: the company page sheet. It is captured before its page loads, so it shows the loading state.
- `build`: the build flow's sector picker for the player's home region.

In the regular slices the sidebar narrows to 176 points under 900 points wide and Next week and the speed
control stay pinned in the status bar. Known issue: in the regular slices the markets screen's second
column (the stock board) is squeezed to a sliver and clips.
