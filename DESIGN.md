# Data Hub design system: fixa

Data Hub's visual identity follows the fixa film-lab board: warm paper ("Print"), forest-black darkroom
("Darkroom"), one emerald ("Fixer"), Schibsted Grotesk type and a halftone dot motif.

The app is Rust + egui (NativeActivity), not Jetpack Compose. Material 3 roles below are the naming
contract; the tokens live in `crates/hub-app/src/style.rs`, and screens read only those constants and
helpers (`card`, `cta`, `ghost`, `chip`, `metric_chip`, `tab_row`, `back_button`, `section_title`,
`backdrop`, `halftone`).

## Colour tokens

| Token | Hex | Use |
|---|---|---|
| `FIXER` | `#1C7C54` | Primary: CTA fill, active nav disc, selected chips, chart highlight |
| `FOREST` | `#145A3D` | Emerald 1: primary text accent on Print, active nav label |
| `SAGE` | `#6BA68B` | Emerald 3: links, tab underline, index markers and accents on Darkroom |
| `MINT` | `#C5DFD2` | Emerald 4: pale series / category cards |
| `PRINT` | `#F5F3EE` | Light surface; text on Darkroom and on Fixer |
| `PRINT_CARD` | `#E9EFEA` | Light surfaceContainer |
| `PRINT_CARD_HI` | `#DCE7DF` | Light surfaceContainerHigh (metric chips, disabled CTA) |
| `PRINT_MUTED` | `#4F5F56` | Light onSurfaceVariant |
| `PRINT_LINE` | `#C9D6CD` | Light outlineVariant |
| `INK` | `#0B1510` | Dark surface; text on Print |
| `ROOM_CARD` | `#12211A` | Dark surfaceContainer |
| `ROOM_CARD_HI` | `#1A2E24` | Dark surfaceContainerHigh |
| `ROOM_MUTED` | `#A3B3AA` | Dark onSurfaceVariant |
| `ROOM_LINE` | `#24382D` | Dark outlineVariant |
| `ERROR_PRINT` / `ERROR_ROOM` | `#B3261E` / `#F2B8B5` | Error text and destructive fills |

Team colours on boards come from the match theme (club colours) and are data, not brand.

### Material 3 role mapping

| Role | Print (light) | Darkroom (dark) |
|---|---|---|
| surface | `PRINT` | `INK` |
| surfaceContainer | `PRINT_CARD` | `ROOM_CARD` |
| surfaceContainerHigh | `PRINT_CARD_HI` | `ROOM_CARD_HI` |
| onSurface | `INK` | `PRINT` |
| onSurfaceVariant | `PRINT_MUTED` | `ROOM_MUTED` |
| outlineVariant | `PRINT_LINE` | `ROOM_LINE` |
| primary / onPrimary | `FIXER` / `PRINT` | `FIXER` / `PRINT` |
| primary as text | `FOREST` | `SAGE` |
| error | `ERROR_PRINT` | `ERROR_ROOM` |

### Measured contrast (WCAG 2.x, computed from the hex values)

| Pair | Ratio |
|---|---|
| `INK` on `PRINT` / `PRINT_CARD` / `PRINT_CARD_HI` | 16.8 / 15.9 / 14.7 |
| `PRINT_MUTED` on `PRINT` / `PRINT_CARD_HI` | 6.1 / 5.3 |
| `PRINT` on `INK` / `ROOM_CARD` / `ROOM_CARD_HI` | 16.8 / 15.1 / 13.0 |
| `ROOM_MUTED` on `INK` / `ROOM_CARD` / `ROOM_CARD_HI` | 8.5 / 7.6 / 6.6 |
| `PRINT` on `FIXER` (CTA label) | 4.66 |
| `SAGE` on `INK` / `ROOM_CARD` | 6.6 / 5.9 |
| `FIXER` on `INK` | 3.6: graphics and large shapes only, never body text |
| `ERROR_PRINT` on `PRINT`, `ERROR_ROOM` on `INK` | 5.9 / 10.9 |

## Typography

- App UI: **Schibsted Grotesk** Regular / Medium / SemiBold, bundled in `crates/hub-app/assets/fonts/`
  (OFL). The files are static instances cut from the variable font with **tabular figures frozen
  into the digit glyphs**, because egui cannot switch OpenType features: every number in the app is
  tabular.
- Helpers: `head()` SemiBold display, `bold()` SemiBold, `semi()` Medium, `body()` Regular. All take
  the in-app text size (Settings > Text size, 100–150 %).
- Hero figures use `-0.02em` tracking (`extra_letter_spacing`).
- Boards keep Inter and Barlow Condensed (`board_font`), the faces their layouts are measured in, and
  do not follow the text-size setting (they zoom with pinch instead).
- No serif italic is bundled yet; empty-state taglines use Schibsted Regular.

## Shape and spacing

- Primary button: full pill, `FIXER` fill, `PRINT` label. Secondary: 1dp outlined pill in `INK`
  (Print) or `PRINT` (Darkroom).
- Chips: pill, outlined when idle, `FIXER` fill when selected.
- Cards: 20dp radius, tonal step only (no shadows, no strokes).
- Bottom navigation: pill container in `PRINT_CARD` / `ROOM_CARD`, active item on a `FIXER` disc.
- Touch targets: 48dp minimum, 8dp minimum gap.
- Section headers carry an editorial index marker (`No. 01`) in `SAGE`.

## Motif

- `halftone()`: hexagonal dot matrix whose dots shrink toward the edge, in `FIXER`. Used on the
  Darkroom backdrop (top right). Keep it to one motif per screen.

## Charts and boards

- Boards always render in Darkroom: board panels draw in `ROOM_CARD`, the ground is `INK` to `#0F1C15`.
- Tooltips: `SCRIM` (ink) card, `ROOM_LINE` hairline, `FIXER` indicator dot, `PRINT` text. Tap to
  inspect; hover only adds a preview on desktop.
- Legends double as layer toggles; colour is never the only channel (shapes, dashes, labels).

## Known gaps

- Boards stay Darkroom in Print mode (their generator draws for a dark ground).
- System font scale: NativeActivity does not expose it; the app offers its own text size instead.
- `systemGestureExclusion` is not reachable from egui on NativeActivity; edge swipes go to the system.
- Time-frame pill selectors (7D / 30D / 90D / ALL) do not apply: every board covers one match.

## Polish checks and credit

The UI went through a redline polish in the style of the unslop method (spacing, colour, icons, copy,
type, layout details, motion). The method is by Damir Sokolovsky (@heydamir),
<https://heydamir.design/unslop>, licensed CC-BY-4.0; no text of it is reproduced here, only the checks:

- Spacing on a 4dp grid; section titles 24dp above and 8dp below; nested radii (outer radius minus padding, never below 4dp).
- Colour: chrome stays in the `PRINT` / `INK` neutral ramps with `FIXER` / `SAGE` as the only accent. No gradients or glows on chrome.
- Icons: 2.4dp strokes, drawn in the colour of their text role, in fixed-size slots. No emoji in UI strings.
- Copy: no trailing period on short labels and captions; no exclamation marks.
- Type: sentence case for labels (no tracked all-caps); weights Regular, Medium and SemiBold; digits tabular; no italics.
- Motion: exponential fling decay, eased zoom (`1 - 0.001^dt`), 0.18 s legend fade, one cubic-out entrance stagger; all of it off under reduced motion.
