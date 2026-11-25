# Data Hub Upgrade: Match Analysis Pipeline and Visual Suite (Rust)

Target: reproduce, for any match, the full analysis package shown in "Arsenal 1–1 Borussia Dortmund: Siege Without Clinical Finishing" (Mostafa_Tactical, 25 Nov 2026): every metric, every narrative sentence and every match visual, rendered in the same visual language.

---

## 0. Repo conventions come first

This document says what to build. It does not say how the repo should be organised. Before writing any code:

1. Survey the existing repo: workspace and crate layout, module naming, error type (thiserror, anyhow or custom), async runtime, HTTP client, storage layer, how the existing visuals are drawn, theme tokens, fonts, CLI entry points, test and fixture layout, and the snapshot-test approach.
2. Write `docs/hub-mapping.md`. List every component named here (section 2 and later) and give the existing module that will host it, or "new" with the path that follows the repo's naming.
3. Reuse the existing renderer, theme constants, pitch primitives, header and footer, and colour tokens. The style notes in 5.1 describe the target look. If they conflict with tokens already in the repo, the repo wins.
4. New sources follow the pattern of the existing WhoScored, FotMob, SofaScore and Transfermarkt fetchers: same module shape, cache, rate limiter, error type and fixtures.
5. Only use the crate names in section 2 where the repo has no equivalent.

---

## 1. Data sources

### 1.1 Existing sources and what each owns

| Source | Role in the hub | Key payload |
|---|---|---|
| WhoScored | Primary event stream (Opta) | Every on-ball event with x/y, end x/y, qualifiers (pass type, body part, cross, through ball, key pass, big chance, set-piece origin), outcome, minute/second, player and team IDs, formations and formation changes |
| FotMob | Shot-model layer and match metadata | Shot-level xG and xGOT, shot coordinates, lineups, formations, managers, substitutions, cards |
| SofaScore | Cross-check and fallback | Shot xG, attack momentum, player heatmaps, lineups, ratings, incidents |
| Transfermarkt | Squad and player master data | Canonical names, DOB (for average age), primary position (for percentile peer groups), current club (for lineup sanity check), market value |

### 1.2 New sources to add

| Source | Why | Notes |
|---|---|---|
| StatsBomb Open Data (github.com/statsbomb/open-data) | Training data for in-house xG, xT and VAEP models | Non-commercial licence, attribution required. Coverage is patchy by competition and season |
| Understat | Second xG source for top-5 leagues, model calibration check | Per-shot xG with coordinates |
| ClubElo (clubelo.com CSV API) | Opponent strength for multi-match context boards | Daily Elo per club |
| football-data.co.uk | Historical results and closing odds | Calibration of simulated win probabilities and expected points |
| DFL / Sportec open games (CC-BY 4.0, via the databallpy loader) and Metrica sample games | Synchronised tracking and event data | Phase 2 only: off-ball and pitch-control work |
| Opta Analyst | Manual spot-check of headline numbers | No ingestion |

Do not build on FBref. It lost its Opta advanced stats in January 2026 and now carries basic stats only.

### 1.3 Reference implementations to port (Python to Rust)

- socceraction: SPADL action schema, VAEP features and labels, xT grid.
- kloppy: provider coordinate systems and event-type mappings.
- mplsoccer: pitch dimensions and Opta and StatsBomb coordinate conventions, used to check the geometry.

### 1.4 Legal

WhoScored, FotMob, SofaScore and Transfermarkt terms generally prohibit automated scraping and redistribution. Keep scrapers behind a `personal` feature flag, rate-limit and cache aggressively, and plan for a licensed feed (Opta, StatsBomb, Wyscout or a commercial API) before any public or commercial release. Show attribution on every visual footer.

---

## 2. Architecture (fallback only; map onto the existing repo per section 0)

```
crates/
  hub-ingest/     reqwest + tokio, scraper (HTML), serde_json; one module per source; disk cache keyed by URL+date
  hub-model/      canonical domain types, coordinate normalisation, ID resolution across sources
  hub-store/      DuckDB (duckdb-rs) for analytics, Parquet snapshots via polars
  hub-derive/     possessions, sequences, game state, all metrics in section 4
  hub-ml/         xG, xT, VAEP training and inference (linfa logistic regression baseline; gradient boosting via lightgbm bindings optional)
  hub-sim/        Monte Carlo win probability, expected points, live win-prob curve
  hub-render/     hand-built SVG renderer + resvg/tiny-skia rasteriser to PNG; shared theme and pitch primitives
  hub-narrative/  templated match report text (minijinja)
  hub-cli/        clap CLI: ingest, build, render, report
```

Use a custom SVG renderer, not plotters. The visual style (dark theme, condensed headline type, crest header, pitch overlays, KDE shading) needs pixel-level control.

### 2.1 Canonical model (summary)

```rust
pub struct Match { id: MatchId, competition: String, date: NaiveDate, home: TeamId, away: TeamId,
                   managers: (String, String), formations: Vec<FormationSpell>, periods: Vec<Period>,
                   source_ids: SourceIds }

pub struct Event { id: EventId, match_id: MatchId, period: u8, t: f32 /* seconds from kick-off incl. added time */,
                   team: TeamId, player: Option<PlayerId>, kind: EventKind, outcome: bool,
                   start: Pt, end: Option<Pt>, body_part: Option<BodyPart>, play_pattern: PlayPattern,
                   qualifiers: Qualifiers, possession_id: u32, sequence_id: u32 }

pub struct Pt { x: f32, y: f32 } // metres on a 105 x 68 pitch; team in possession attacks +x
```

- Normalise every source to 105 x 68 metres, with each team attacking left to right. Store the raw coordinates as well.
- Check the Opta y-axis orientation against known left and right corners before trusting any half-space split.
- ID resolution: fuzzy name match (normalised and diacritics-stripped) plus shirt number plus team, with manual override tables. Merge the FotMob and Understat shot lists onto the WhoScored shot events by (minute, player, distance under 2 m).

---

## 3. Core derivations

### 3.1 Possessions and sequences
- A possession changes when the other team makes a controlled action (completed pass, carry, successful take-on, shot, or a recovery followed by a further action). Duels and failed tackles do not change possession.
- A sequence is an uninterrupted chain within a possession, broken by a stoppage.
- Possession start type: recovery, interception, tackle, throw-in, goal kick, corner, free kick, kick-off, keeper.

### 3.2 Game state
Assign game state (level, leading, trailing) at the start of each possession. Count minutes per half including added time.

### 3.3 Models
- **xG:** logistic regression or gradient boosting trained on StatsBomb open data plus accumulated WhoScored shots. Features: distance, angle, body part, header, first-time, assist type (cross, through ball, cutback), big chance, set-piece origin, fast break. Exclude penalties (npxG); store penalty xG as a constant 0.76. Keep the FotMob xG alongside for comparison.
- **xGOT (post-shot xG):** two layers. Use the FotMob xGOT as the reference value. Also build an in-house "local placement" PSxG: a model on pre-shot xG plus goal-mouth y/z (Opta goal-mouth qualifiers from WhoScored, or the FotMob on-goal coordinates), trained on StatsBomb open data shots, which carry end-location z. Like the source, it has no shot velocity and no goalkeeper position, and every board that uses it must print that caveat. Penalties and own goals are excluded from goals prevented.
- **xT:** fit on a 16 x 12 grid by value iteration on move and shot probabilities. Value only successful moves (passes and carries). For display, aggregate the xT added by where each move began into 8 x 12 squares, using one colour scale for both teams that tops out at the 95th percentile.
- **VAEP:** SPADL conversion, with labels for "scores within the next 10 actions" and "concedes within the next 10 actions". Fit on every stored match. Goalkeeper saves count as defensive value.
- **Win simulation:** 20,000 Monte Carlo replays, one Bernoulli draw per shot at its npxG. At most one goal per possession, so rebound chains are not double-counted. Output P(win), P(draw), P(loss) and xPts = 3·P(win) + P(draw).
- **Live win-probability curve:** a transparent heuristic, not a betting model. At minute t, take the actual score plus Poisson remaining goals for each team over (T − t). The rate is the team's xG per minute to date, shrunk toward a prior (the league average or the ClubElo-implied rate). "Likelier winner for X% of the match" is the share of minutes in which that outcome had the highest probability.

---

## 4. Metric definitions (must match exactly)

| Metric | Definition |
|---|---|
| Possession % | Share of possession time (pass-count share as fallback) |
| Field tilt | Share of the two teams' final-third passes |
| Clear chance | Opta big chance qualifier, or npxG ≥ 0.12 if the qualifier is missing; configurable |
| Build-up success | Share of own-third possession attempts that reach the middle third with a completed action, without a long ball |
| Progressive pass | Moves the ball ≥ 25% closer to the goal centre (from own half), or ends in the box |
| Line-breaking pass | Completed pass that crosses at least one opponent line (approximated from event data by a vertical gain ≥ 10 m through the central lanes); show as completed/attempted |
| Box entry | Completed pass or carry ending inside the penalty area from outside it. Count pass and carry entries separately. Repeated entries in one possession count separately. "Shot followed" means a shot later in the same possession |
| Zone 14 | Central channel outside the box: x 70–88.5 m, y 24.85–43.15 m |
| Half-spaces | Lanes between the box edge and the six-yard-box line, outside the box: y 13.84–24.85 and 43.15–54.16, x ≥ 60 m |
| Cutback | Completed pass from inside the box near the byline (x ≥ 99.5 m) played backwards (end x < start x) |
| Through ball | Opta through-ball qualifier; show attempts |
| High regain | Open-play recovery, interception or tackle won in the attacking 40% (x ≥ 63 m). Outcome is classified as nothing followed, reached the box, or led to a shot |
| High turnover | Ball won at x ≥ 66.7% of pitch length (x ≥ 70 m). Link to a shot by the same team within 12 s |
| Counterpress regain | Regain within 5 s of losing the ball in open play. Show regains/attempts and % |
| Turnover to shot conceded | Open-play loss followed by an opponent shot within 12 s; report count/total losses and xG |
| PPDA | Opponent passes in their own 60% of the pitch divided by defensive actions (tackles, interceptions, challenges, fouls) in that zone. Compute for the full match, per half and in 15-minute bins (0–15, 15–30, 30–45+, 45–60, 60–75, 75–90+). Leave out bins with no defensive actions and clamp values above 30 to 30 |
| PPDA gauge bands | under 8 Elite press, 8–12 Medium block, over 12 Low block; configurable |
| Defensive height | Mean distance from own goal at which the ball was won (tackles won, interceptions, recoveries). Calculate separately for "stopped, not won" (clearances, fouls, blocks) |
| Transition | Open-play regain or turnover where, within 12 s, the ball progresses ≥ 20 m, reaches the final third or the box, or produces a shot. Restarts are excluded. The arrow runs from the start to the most advanced point |
| Set-piece attribution | Trace each shot back to how its possession started: open play, corner, free kick, penalty. Classify by the delivery, not the shot |
| Momentum | xT from successful moves summed over a rolling 8-minute window, weighted toward the middle of the window. Home is plotted up and away down. "Spells" are cut where the share of threat flips; each spell shows the threat split % and the xG for each team |
| Dominating zones | 5 x 7 grid (width x length). Cell value is home touch share minus away touch share in percentage points. Totals panel shows the touch counts and the difference |
| xT by channel | Final-third entries split into left, centre and right; average seconds to reach the final third; touches in the final third |
| Possession funnel | Possessions → reached final third → touched the ball in the box → shot from there → on target |
| Goalkeeping | Saves; post-shot value faced minus goals conceded (goals prevented); where saves went (held or safe, or back to an attacker) |
| Finishing | npxG compared with xGOT compared with goals, per team |
| Goal origin | For each goal: possession start event, ordered touches, passes, seconds, distinct players, start type, assist, body part, shot xG and a label (QUICK ATTACK AFTER A REGAIN, BUILD-UP FROM THE BACK, SET PIECE, COUNTER, LONG BALL) |
| Inferred xA | The xG of a shot, credited to the last successful key pass before it in the same possession, within 15 s |
| xGChain / xGBuildup | xGChain: the possession's shot xG credited to every player involved. xGBuildup: the same, but excluding the shooter and the key-pass provider |
| Second balls won | Loose-ball recoveries within 3 s and 15 m of an aerial duel or a clearance. Report won/total |
| Press trigger | The opponent action immediately before a high regain: loose touch, opponent pass, dispossessed, interception, clearance, dribble attempt, error. Report count and % of high regains |
| Team height and length (5-minute windows) | Height: mean x of defensive actions. Length: the spread (IQR) of the team's touch x. Smooth with a rolling median of three windows. Include the match median line and the halfway line |
| Goal frame zones | Goal-mouth y/z split into a 3 x 2 grid (High Left, High Centre, High Right, Low Left, Mid Centre, Low Right) plus Other |
| Keeper distribution | Keeper passes: count, average length in metres, % launched (≥ 40 m or Opta launch qualifier), % completed |
| Keeper handling | Save destination: held, parried safe, or into danger (back to an attacker), from the Opta save qualifiers. Verify qualifier IDs against the Opta F24 qualifier list |
| Player profile | Percentiles against players in the same role with 30+ minutes in the match pool, when four or more such players exist. Otherwise rank against all players with 30+ minutes and print the fallback note in the caption. A grey ring marks the pool average. Metrics: xG, xA, xGChain, progressive pass share, progression metres, line-break passes, final-third receptions, box entries, PAdj defensive actions, defensive height, aerials won, xA per 100 passes. For goalkeepers: saves, claims, sweeps, progressive pass share. The ring's shortest wedge gets an "differed most" callout |

---

## 5. Visual suite: recreate every chart

### 5.1 Shared design system
- Canvas 1600 x 1130 px (about 1.42:1), exported as SVG and PNG at 2x.
- Background: near-black with a subtle dark-violet radial gradient (#07060c to #120a14). Text white and grey (#b8b8c0). Pitch lines white at 70% opacity on transparent.
- Team colours are never hardcoded. Each match resolves them at build time through the theme pipeline in 5.4. The Arsenal red and Dortmund yellow in these examples are just one resolved output. Use a muted grey for "nothing followed" and incomplete passes.
- **Header:** circular creator logo on the left; a small "MATCH VISUAL" kicker; a large condensed headline (Barlow Condensed or Oswald, bold); a one-line grey subtitle defining the denominator. On the right: home crest, home name in home colour, score "1 — 1", away name in away colour, away crest. Below that, a scope label ("FULL MATCH" or "TEAM VIEW · ARSENAL") and "CREATED BY …". Under the header, a 3 px rule split home colour (left) and away colour (right).
- **Footer:** a "how to read" sentence bottom-left, and "FULL VISUAL REDESIGN · REAL MATCH DATA" bottom-right.
- **Pitches:** vertical with attack up for single-team boards, labelled "ATTACK". Horizontal for VAEP and full-pitch team comparisons. Half-pitch for shot, regain and turnover boards.
- **Data-driven headlines:** each board generates its own title from the data, for example "Arsenal got into the box 26 times · 15 led to a shot", "Arsenal pressed hardest between 75' and 94'", "Madueke added the most to their side's chances".

### 5.2 Boards (each needs a render function, a data query and a snapshot test)

1. **xG flow step chart.** Cumulative npxG step lines for both teams over minutes. Step height is scaled to chance quality. Goal markers, woodwork markers, HT line.
2. **Goal Origins.** One pitch per goal. Numbered touch nodes in team colour, arrows for the passes, a dashed line for the shot, a star at the shot location. Below each pitch: minute (large, team colour), scorer, origin label, "N passes · S s · P players", "Started: <type>", "Assist: <name> · <foot> · <xG> xG".
3. **Win odds and expected points.** Headline "The chances were worth X points to A and Y to B". A strip reading "LIKELIER WINNER FOR A x% · B y% · LEVEL z%". A 100% stacked bar of win/draw/loss with labels. Below it, a minute-by-minute line chart: home win (solid, filled area), away win (solid), draw (dashed grey). Caption: "transparent heuristic on score, time and xG, not a betting model".
4. **Set-Piece Contribution.** Two panels of horizontal bars (Open play, Corners, Free kicks) labelled "shots · xG · goals". Subtitle "N% of xG came from a dead ball". Print "none" for empty categories.
5. **Creation Corridors.** Two attack-up half-pitches with Zone 14 and both half-spaces shaded. Passes coloured white (Z14), blue (left half-space), teal (right half-space), yellow (cutback), orange (through ball). Count labels at the lane bases. A header line per team: "Z14 · Half-spaces (L/R) · Cutbacks · Through-balls".
6. **Shot Anatomy and Pitch Zones.** Two half-pitches with the six-yard box and penalty area shaded. Marker shape: circle (foot), triangle (header), diamond (first-touch). Fill: white (six-yard box), blue (box), grey (outside). Size scales with xG; a gold ring marks a goal. Summary strip: 6-YD BOX, ALL BOX, OUTSIDE, HEADED, 1ST-TOUCH, each as count (xG).
7. **Shot Map per team** ("TEAM VIEW"). Headline: "<Team>: N shots worth X xG, G goal(s)". Full vertical pitch with attack up. Markers: off target (orange X), blocked (grey square), woodwork (purple diamond), saved (teal circle), goal (gold star). Size scales with xG, with legend counts. Clear chances carry an xG label. Side panel "SHOT OUTPUT": shots, xG, xG per shot (3 decimals), on target, and "GOAL FRAME TARGETED" counts per frame zone.
8. **Box Entries per team.** Full vertical pitch. Entry lines are white with an arrow if a shot followed, team colour if not. Side panel: "ENTRY CONTRIBUTORS" (top 5 plus Others), entry-method legend with pass and carry counts, total.
9. **Passing by Third per team.** Three vertical pitches (build-up, progression, final third). Completed passes in team colour, incomplete in grey, key passes in white. Header per pitch: "N passes · % completed · % forward" and "N completed into the next third" (or "into the box · N key passes").
10. **Pass Network**, four boards (each team, each half). Headline: "Pass Network · <Team> · First/Second Half". Subtitle: "N participants · each circle sits at the player's average touch position".
    - Nodes are numbered by shirt number with the name beneath. A filled circle means started and stayed, a ring means went off, a square means came on. A dotted line joins each replaced player to his replacement.
    - Edge width and opacity follow completed passes between the pair in both directions.
    - Right panel "TOP PAIRS": the top 4 pairs in "A ↔ B" form, with a count and a bar in team colour.
    - "LINE-UP" card: "Started the half" lists shirt number and name, with ▼ and minute for players who went off. "Came on" lists ▲, number, name, "for X" and minute. Show "No changes in this half" where there were none, and "N CHANGES" in the card header.
    - Stats strip: completed links, players used, and the hub player ("most passing routes ran through him", the highest weighted degree).

11. **Defensive Activity per team.** Two pitches. "WON THE BALL" shows recoveries (dot), tackles won (triangle) and interceptions (diamond) over a KDE heat layer. "STOPPED, NOT WON" shows clearances (x), fouls (square) and blocked shots (circle). A dashed average-height line on each, labelled "avg N m".
12. **High Regains per team.** Attacking half with the 40% line dashed. Grey dot means nothing followed, team-colour square means reached the box, white star with a minute label means led to a shot. Side panel: high regains, led to a shot (count · xG), reached the box only, nothing followed.
13. **High Turnovers Forced, Errors and xPts.** Two half-pitches with the x = 66.7 line. A turnover that led to a shot is a team-colour dot joined by a dashed line to a triangle at the shot (star for a goal); other turnovers are grey; an X marks an opponent error forced. Summary strip: HIGH TO (n → shots), TO GOALS/xG, OWN ERRORS (shots/goals), xPTS, AVG AGE (from Transfermarkt, minutes-weighted).
14. **PPDA Timeline.** Line chart of 15-minute bins with an inverted y-axis (more pressing at the top), point labels, a note "bins with no defensive actions are left out; values above 30 are drawn at 30". The headline names the peak window.
15. **Match Momentum.** Mirrored area chart (home above zero, away below) of rolling 8-minute xT. Goal markers labelled with scorer and minute, a dashed HT line, substitution triangles with ×n grouping, red-card markers. Spells bar beneath: blocks coloured by the dominant team, labelled "start–end' share/share" and "xG – xG".
16. **Game-State Output.** A time-share bar (level, home ahead, away ahead, in minutes). One card per state showing Shots, xG, Transitions, Box entries and xG/30 min for both teams. Print "NOT PLAYED · <team> never led" for empty states.
17. **VAEP Action Value.** Horizontal pitch. Each player is a disc at their average action location, sized by value added; a ring shows value given away. Team totals sit at the top. Right panel: "ACTIONS THAT MOVED THE MATCH MOST" (minute, player, action, value). Bottom: bars for the top 6 players, solid for attack and faded for defence.
18. **Dominating Zones.** 5 x 7 grid on a vertical pitch with a diverging colour scale (home colour, black, away colour) and the signed value in each cell. Side panel: touches per team with %, and the difference.
19. **Transition Map and Outcomes.** Two pitches. Grey arrows show the average transition per zone that produced no shot; coloured arrows show transitions that created a chance (a goal gets a distinct style). Outcomes panel: transitions, created chance, goals, transition xG, box entries, chance rate %.
20. **Match Statistics.** Two mirrored bar tables. ATTACK and PASSING: Goals, xG, xGOT, Shots, Big chances, Positive pass xT, Passes attempted, Key passes, Progressive passes, Line-breaking passes (c/a), Box entries, Field tilt %. DEFENSIVE ACTIONS: Tackles, Interceptions, Blocks, Clearances, Ball recoveries, Possession regains, High regains, Fouls, Aerial duels (w/t), Take-on duels (w/t). PPDA gauges for each team with band colours and labels. Show the PPDA differential and a three-column "Reading this page" panel.
21. **xT Heatmap per team.** Subtitle: "8 × 12 squares over the pitch · xT added where each successful pass or carry began · one scale for both teams". A vertical pitch with the 8 x 12 grid drawn. The value is printed in squares above a threshold (default 0.05). A vertical colourbar is labelled "xT added per square · scale tops out at the 95th percentile". The top 3 xT passes are drawn as white arrows. Side panel "TOP 10 XT PASSES": rank, player, match clock mm:ss, and value to 3 decimals.
22. **Goalkeeper Goal Frames.** Two panels, one per keeper, showing the goal as seen from behind the shooter. Every shot faced is plotted where it crossed the goal line; off-target shots sit outside the posts and blocked shots are excluded. Colour shows the outcome (off target, woodwork, saved, goal). Shape shows the body part: circle for right foot, square for left foot, triangle for header. Size scales with xG.
    - Subline: "<Team> · N shots faced · N off target · N woodwork · N blocked".
    - Stat tiles: faced (on target), saves, conceded, prevented (signed, 2 decimals), save rate, claims, sweeps.
    - DISTRIBUTION line: passes, average length in metres, % launched, % completed. HANDLING line: held, parried safe, into danger.
    - Footer definition: "Prevented = post-shot xG faced minus goals conceded, penalties and own goals left out".
23. **Defensive Action Height and Touch Spread.** Two stacked line charts over match minutes, in five-minute windows smoothed with a rolling median of three.
    - Top chart: mean x of defensive actions per team, with the area between the lines shaded in whichever colour is higher, a halfway reference line, and the match median line labelled.
    - Bottom chart: team length (IQR of touch x), shaded the same way, with the match median labelled.
    - Raw five-minute samples are faint dots behind the lines. Half-time is a dashed vertical line.
24. **Chance Quality against Finishing.**
    - Left, "From chance to goal": grouped horizontal bars per team for chances created (npxG), reached the frame (pre-shot xG of on-target shots), value once struck (PSxG) and goals.
    - Right, "Every attempt that reached the frame": a dumbbell per on-target shot, labelled "Player minute'" in team colour, with a hollow marker at pre-shot xG, a filled marker at PSxG and a star for a goal. Sort by PSxG descending.
    - Two verdict sentences in team colour, generated from the numbers: "struck them worse/better than the chances were worth".
    - Caveat line about the placement-only model.
25. **Press Triggers and Rest Defence.** Two panels.
    - Subline: "N of M high regains followed an opponent action".
    - Horizontal bars of press triggers sorted descending, labelled "n (pct%)".
    - Table "REST DEFENCE & SECOND BALLS": open-play losses, opponent shot within 12 seconds (n/losses), second balls won (n/total, pct%).
    - Footer: "Pressing-trigger counts are event-based review cues; they cannot confirm coordinated pressing without video."
26. **Progression and Chance Creation scatter.** Eligible players are those with 30+ minutes. X axis: progressive passes plus explicit carries. Y axis: inferred xA. Home players are team-colour circles and away players are team-colour squares. Dotted lines mark the eligible-player medians. Every point is labelled, with a repel step to avoid overlaps. Subtitle: "N observations | players with at least 30 minutes, match totals".
27. **From Possession to a Shot on Target.** Mirrored funnel with the home team's bars growing left and the away team's growing right. Stage labels run down the middle: Possessions, Reached final third, Then box, Then shot, On target. Between stages, show "↓ pct%" for the drop-off. Footer: "Same possession at every stage."
28. **Box-Entry Routes and Their Next Outcome.** Two panels. For Left, Centre and Right, a dumbbell from 0 to all entries (grey circle), with a team-colour square at the number of entries followed by a shot, labelled "N shots · pct%" and "N entries". Footer: "A following shot is descriptive, not proof of cause."
29. **Territory and Chances Across the Competition.** Scatter with one point per team-match: field tilt % against xG. All competition team-matches are grey. This team's other matches are small dots in team colour; this match is a large ringed dot labelled "this match". Both teams are shown. Subtitle: "Comparable metric version, one point per team-match, not the two teams of one fixture". Footer: "input-model provenance is listed in each manifest" (see 6.3).
30. **Pressing Frequency and Danger After Losses.** Same layout as 29. X axis: PPDA (lower means fewer opponent passes per defensive action). Y axis: dangerous counters as a % of advanced losses.
31. **Player Card**, one per player. Header: name, then "Team · Role · Touches N · Shots N".
    - Three headline tiles for the player's strongest ring measures (value, then "k of n in his role"), plus minutes.
    - Left: a percentile ring of eight wedges with raw values labelled. The wedges are progressive pass share, PAdj defensive actions, aerials won (defenders) or progression metres, defensive height or line-break passes, progression metres, xGChain, and others by role. The grey ring is the pool average. Caption: "Ranked against N players in the same role (30+ min)".
    - Right, "Where he had the ball, and where he defended": a horizontal pitch with a touch-density KDE in blue, touch dots, defending markers in pink, a dashed defensive-height line, progressive passes as thin lines, passes into the box as white arrows, and shots as stars. Tactical snapshot text: touches by third, average touch upfield in metres, metres left or right of centre, defensive action count.
    - Three stat columns, labelled by role. Defenders: ON THE BALL / DEFENDING / THREAT. Midfielders: ON THE BALL / CREATION / WITHOUT THE BALL. Forwards use the same pattern. Show N/A for values that are unavailable.
    - Small-print definitions block, then the footer "Role and minutes govern comparisons; these are match observations, not a season ability rating. Nominal pitch 105 × 68 m."

### 5.4 Works for any WhoScored match: per-match theming and generalisation

The whole pipeline must run from a single WhoScored match ID or URL, for any competition, with no team-specific code. Arsenal–Dortmund is only the golden test case.

**Entry point**

```
hub build --whoscored <match_id|url> [--theme-override path.toml] [--boards all|list]
```

1. Fetch the WhoScored match (events, lineups, formations, team IDs, names, date).
2. Resolve the matching FotMob, SofaScore and Transfermarkt IDs automatically: date ±1 day plus fuzzy team names through `id_map`. Cache the mapping. Any unresolved source becomes a recorded fallback, not a failure.
3. Derive the metrics, resolve the theme, render all boards and write the manifests.

**Colour resolution (per match, per team)**

Candidates, in priority order:
1. A manual override in `theme_overrides.toml`, keyed by canonical team ID.
2. FotMob team colours from match details. Prefer the dark-mode variant, since every board has a dark background.
3. SofaScore team colours (primary and secondary).
4. Dominant colours extracted from the crest: k-means (k = 3) on non-transparent pixels, dropping near-white, near-black and grey clusters.
5. A neutral fallback pair (home warm red, away yellow).

Normalisation, worked in OKLCH:
- Each team colour needs contrast ≥ 3:1 against the board background. If it falls short, raise L in steps of 0.03 until it passes, capping C to stay in gamut.
- Near-black or near-grey team colours (C < 0.04) switch to the secondary colour or the crest-derived colour. Grey is reserved for "nothing followed" and incomplete passes, and black is the background.

Clash handling:
- If the home and away colours are too close (CIEDE2000 ΔE < 25, or both read as the same hue family on screen), the away team switches to its secondary colour, then its crest-derived colour, then the first fallback from a high-contrast set (yellow, cyan, white, lime) that clears ΔE ≥ 25 against home. Home keeps its primary. Typical cases: red against red, blue against blue, white against light grey.
- Record the final choice and the reason in the manifest (for example "away switched to secondary: ΔE 11.2 vs home").

Semantic colour collision:
- Some board markers have fixed colours: goal gold, saved teal, woodwork purple, off-target orange, half-space blue and teal, cutback yellow, through-ball orange, touch-density blue, defending pink.
- When a team colour sits within ΔE 20 of one of these on a board where both appear, that board switches the semantic marker to its alternate from a predefined second palette, and the legend updates automatically. A yellow team then never shares yellow with cutbacks or goal stars.

Derived tokens per team, stored in a `MatchTheme` struct:
- `primary`, plus `primary_muted` (for the faded defence bars in VAEP and secondary series).
- `fill_alpha` variants for areas and heat layers.
- `on_primary` text colour, black or white, whichever gives the higher contrast.
- A sequential scale (background to primary) for the xT heatmaps.
- A diverging scale (home to near-black to away) for Dominating Zones and the shaded areas between lines.
- Header rule segments.

Persist the resolved theme in a `match_theme` table and in every manifest, so re-renders are deterministic.

**Crests and names**
- Take crests from FotMob team logos by team ID. If missing, use SofaScore team images, then Transfermarkt, then a generated monogram disc in the primary colour. Cache them locally and rasterise SVG crests with resvg.
- Team display names have priority: short name (FotMob shortName) first, then the full name with fit-to-width. If it still overflows the header slot, use a defined abbreviation map, then a 3-letter code. The source truncates "BORUSSIA"; this must not happen.
- Player names: use the canonical name from `id_map`. Use the short form (surname, or the known-as name) on pitches and the full name in tables. Keep diacritics.

**Structural cases every board must handle**
- **Goal count 0–N.** Goal Origins lays out 1–2 goals per row and paginates beyond 4. At 0–0 it renders an empty state ("No goals · biggest chance: <player> <minute>' <xG>") instead of failing.
- **Own goals** are credited to the beneficiary's scoreline, excluded from the shooter's xG, and marked "OG" in timelines. Penalties are kept out of npxG and shown as a separate marker. Penalty shootouts are excluded from all metrics.
- **Extra time:** extend the minute axes, PPDA bins (90–105, 105–120+) and momentum. Game-state minutes include all periods.
- **Red cards:** show a marker on the momentum and win-prob timelines, and split game-state cards by numerical state when one exists.
- **Substitutions:** any number, including goalkeeper changes and players who come on and go off again. Pass-network nodes and the line-up card adapt; the "N CHANGES" label is computed.
- **Formations:** render every formation spell from WhoScored, not just the starting one.
- **Missing sources:** if FotMob or SofaScore have no xGOT, PSxG boards use `psxg_local` and print the fallback in the footer. If a Transfermarkt player can't be matched, average age shows N/A with the count of players excluded. A peer group of fewer than 4 uses the all-player fallback.
- **States that never happen** (a team never leads, no set-piece shots, no carries) render as an explicit "NOT PLAYED" or "none" state, never as an empty box or a division by zero.
- **Headlines and narrative** are templated with pluralisation ("1 goal" / "2 goals", "1 shot led" / "8 led"). Comparisons are picked from the data ("could not hold on", "never led", "came from behind", "kept a clean sheet").
- **Axes:** limits are data-driven with sensible floors (xG axis at least 0–1, PPDA clamp at 30). Label placement uses collision avoidance on every scatter and pitch.

**Generalisation test corpus**
Keep a fixture set of WhoScored matches chosen to stress the edge cases:
- a 0–0
- a 5+ goal game
- a red-on-red clash (for example Liverpool–Manchester United)
- a blue-on-blue clash (Chelsea–Everton)
- a white-on-white clash
- a match with a red card
- extra time with penalties
- a match with own goals
- a goalkeeper substitution
- a lower league with no FotMob xGOT
- a women's match

CI renders every board for every fixture. It snapshot-tests the SVGs, asserts the colour rules (contrast, clash ΔE and semantic ΔE) programmatically, and runs the quality gates in section 7 on each match.

### 5.5 Narrative generator
Generate the article text from templates using the derived numbers. Sections: How the match was decided; What the chances were worth; Creation corridors and shot anatomy; <Team> with the ball; <Team> without the ball (both teams); Pressing through the match; Changes, game state and the last half-hour; The players who shaped it; What decided it; Complete visual guide (one caption per board, with a hand-off line to the next board). Phrasing must switch on the data: for example "could not hold on" when a lead was lost, and "never led" when a team never led.

---

## 6. Data gaps: what the four sources do not cover, and how to add it

### 6.1 Field-by-field availability

| Needed for | Field | WhoScored | FotMob | SofaScore | Transfermarkt | Gap fix |
|---|---|---|---|---|---|---|
| Shot map, GK frames | Goal-mouth y/z | Opta goal-mouth qualifiers | on-goal x/y | partial | – | Parse the WhoScored qualifiers; fall back to FotMob |
| Shot map | Woodwork | Opta "post" event type | yes | yes | – | Map to the `ShotOutcome::Woodwork` enum |
| GK frames | Claims, sweeps, save handling | Opta claim, sweeper and save qualifiers | – | partial | – | Parse the qualifiers; add a `KeeperAction` enum |
| GK frames, finishing | xGOT / PSxG | – | yes | yes | – | Use FotMob; also train the in-house placement model (6.2 C) |
| xG, VAEP, xT, PSxG | Training set | – | – | – | – | **Missing.** Add StatsBomb open data (6.2 A) |
| Pass networks | Shirt numbers, substitution minute, "for X" | yes | yes | yes | – | Use the WhoScored lineup, checked against FotMob |
| Pass networks | Average touch position | derive | – | heatmap only | – | Derive from events |
| xT heatmap, top xT passes | xT grid | – | – | – | – | **Missing.** Fit from StatsBomb open data plus the stored match pool (6.2 A) |
| Player card | Role / position | yes | yes | yes | yes | Use Transfermarkt as canonical, with the WhoScored match position overriding |
| High-turnover strip | Average age | – | partial | partial | DOB | Use Transfermarkt DOB, minutes-weighted |
| Line-breaking passes | Opponent line positions | – | – | – | – | **Not in event data.** Ship the event-only approximation now; the exact version needs StatsBomb 360 freeze frames or tracking (6.2 D) |
| PSxG caveat | Shot velocity, keeper position | – | – | – | – | **Not available.** Freeze frames give the keeper position for StatsBomb open competitions only; keep the caveat |
| Competition scatters | Every team-match in the competition | per match | per match | per match | – | **Missing at scale.** Add a batch season ingest (6.2 B) |
| Context colouring | Opponent strength | – | – | – | – | **Missing.** Add ClubElo (6.2 E) |
| Win-prob calibration | Closing odds | – | – | – | – | **Missing.** Add football-data.co.uk (6.2 F) |
| Off-ball pressing, pitch control | Tracking | – | – | – | – | Phase 2: DFL/Sportec open games and Metrica (6.2 D) |

### 6.2 How to add each missing source

Every new source follows the same seven steps, shaped to match the repo's existing fetchers (section 0):

1. `sources/<name>/mod.rs` (or the repo's equivalent): a `fetch_*` function using the shared HTTP client, the cache and the rate limiter.
2. Raw payloads go to the repo's raw cache, at `<cache>/<source>/<id>.json|csv`. Never re-fetch anything that is already cached.
3. `parse.rs`: raw payload to canonical structs. Keep raw coordinates and source IDs.
4. A DB migration adding the new tables or columns, using the repo's migration tool.
5. ID mapping rows in the shared `id_map` table (source, source_id, canonical_id, confidence, manual_override).
6. Fixture files under the repo's test fixtures, with parser tests and one end-to-end test.
7. A CLI subcommand that matches the existing ones.

**A. StatsBomb Open Data** (model training)
- Fetch: shallow-clone `github.com/statsbomb/open-data`, or pull `data/competitions.json`, then `matches/<comp>/<season>.json`, `events/<match_id>.json` and `three-sixty/<match_id>.json` over raw.githubusercontent.com. GitHub is reachable from most CI and proxy allowlists.
- Parse to the canonical `Event`. StatsBomb uses a 120 x 80 grid; scale to 105 x 68. Shots carry `end_location` [x, y, z] and `freeze_frame`.
- Tables: `train_events`, `train_shots` (with end z and freeze-frame keeper position when present), `train_freeze_frames`.
- Used by: the xG fit, the PSxG placement fit, VAEP labels and features, the xT transition matrices.
- Licence: non-commercial use with attribution. Add "Model training data: StatsBomb Open Data" to the footers of the xG, PSxG, xT and VAEP boards.

**B. Season batch ingest** (competition scatters, percentile pools)
- Add `ingest season --competition <id> --season <id>`. It walks the existing WhoScored and FotMob fixture lists and fetches every match through the existing fetchers.
- Aggregate into `team_match_metrics` (team_id, match_id, competition, season, metric_version, field_tilt, xg, ppda, dangerous_counter_pct, …) and `player_match_metrics`.
- `metric_version` is a semver string bumped whenever a definition in section 4 changes. Scatter boards only plot rows with the current version.

**C. In-house PSxG placement model**
- Train on StatsBomb shots on target, using pre-shot xG, goal-mouth y, goal-mouth z and body part, with logistic or gradient-boosted regression on goal versus no goal.
- Apply it to WhoScored shots using the Opta goal-mouth qualifiers. Store it as `psxg_local` next to `xgot_fotmob`. Boards use `psxg_local`, and the QA log reports the difference between the two.

**D. Tracking and 360 data** (Phase 2)
- StatsBomb 360 freeze frames come from the same repository as A. The DFL/Sportec open games are CC-BY 4.0; download them from the DFL publication (the databallpy `get_open_game` source list gives the URLs). The Metrica sample games are on GitHub (`metrica-sports/sample-data`).
- Tables: `tracking_frames` (match_id, frame, t, player_id, x, y), `tracking_ball`.
- Unlocks: exact line-breaking passes, keeper position for PSxG, off-ball pressing intensity.

**E. ClubElo**
- `GET http://api.clubelo.com/<YYYY-MM-DD>` returns a CSV of every club's Elo on that date.
- Table `elo_daily` (date, club, elo). Map clubs through `id_map`.
- Used by: the context-scatter colouring, opponent-adjusted percentiles and the win-probability prior.

**F. football-data.co.uk**
- Season CSVs per league (`mmz4281/<season>/<league>.csv`) with results and closing odds.
- Table `odds_closing`. Used only to calibrate the win simulation (Brier score and a reliability plot in the QA report).

**G. Understat** (xG cross-check, top 5 leagues)
- The match page embeds a JSON `shotsData` blob. Parse it into `shots_understat` and map shots by minute, player and location.

### 6.3 Provenance manifest
Every rendered board writes `<board>.manifest.json` next to the PNG/SVG, containing: board id, match id(s), metric_version, model versions (xg, psxg_local, xt, vaep with training-set hash), the sources used, fetch timestamps and any fallbacks applied (for example "peer group < 4, ranked against all 30+ min players"). The competition boards point to this file in their footer.

## 7. Data quality gates
- Totals must reconcile across boards. Shots in the shot map, the funnel, Match Statistics and the game-state cards must sum to the same number. The same applies to xG.
- Define "box entries followed by a shot" once and use it everywhere. The source article prints 15 in one place and 14 in another; that drift must be impossible here.
- Where several high-regain xG variants exist (attacking-40% regains, x ≥ 66.7 turnovers, counterpress regains, transitions), label each board with its exact definition.
- Lineup sanity check: flag any player whose Transfermarkt club at the match date differs from the team they appear for. The source article lists Konsa, Guimarães and Tzolis for Arsenal and Veerman for Dortmund, which this check would catch.
- Compare FotMob xG with the in-house xG per shot and log deltas above 0.15.
- Turnover-to-shot counts must come from one function. The source prints 7/82 in the text and 8/82 on the Press Triggers board for Dortmund.
- Shot outcome partition: off target + blocked + woodwork + saved + goal = shots (Arsenal 14 + 10 + 1 + 3 + 1 = 29).
- Route split: the per-route "shot followed" counts must sum to the headline box-entry figure.

## 8. Acceptance test: golden numbers (Arsenal 1–1 Dortmund, from the article)
Possession 65/35 · field tilt 67.4/32.6 · npxG 2.51/1.30 · xGOT 0.51/1.49 · shots 29/13 · on target 4/4 · blocked 10/3 · clear chances 4/3 · box entries 26/6 · high regains 27/14 · PPDA 5.15/10.94 · win sim 65/19/16 · xPts 2.1/0.7 · corners 8/5 (3/2 shot within 10 s) · Z14 11/13 · half-spaces 36 (18/18) / 16 (7/9) · touches 713/428 · transitions 43/39, transition xG 0.82/0.50 · VAEP team totals +3.80/+2.05.

Additional goldens from the second batch of boards:
- Arsenal xG/shot 0.087; goal frame targeted High Left 1, High Right 1, Mid Centre 1, Other 1.
- Pass network first half: 11 players, 283 completed links, hub Rice, top pair Rice ↔ Konsa 19. Second half: 15 players, 228 completed links, hub Ødegaard, top pair Magalhães ↔ Hincapié 18, 4 changes (64' ×3, 86').
- Top xT pass: Rice 63:15, 0.297.
- Kepa: faced 4, saves 3, prevented +0.49, save rate 75%, claims 1, sweeps 2, distribution 60 passes at 28 m average, 22% launched, 88% completed. Kobel: faced 4, saves 3, −0.49, claims 0, sweeps 1, 35 passes at 61 m average, 66% launched, 57% completed.
- Reached the frame (pre-shot xG): 0.26 / 0.51.
- Press triggers: Arsenal 24 of 27, led by loose touch 8 (33%). Dortmund 11 of 14, led by clearance 3 (27%). Second balls 20/38 and 12/38.
- Funnel: 125 → 69 → 34 → 17 → 2 and 115 → 38 → 18 → 10 → 3.
- Entry routes: Arsenal L 11/6, C 6/3, R 9/5; Dortmund L 0/0, C 1/1, R 5/3.
- Ben White: progressive pass share 25%, defensive height 43, xGChain 0.41, 65 minutes. Nmecha: xA/100 passes 0.18, xGChain 0.87, progressive pass share 14%.

Running the hub on the same event data must reproduce these numbers within ±2% for counts and ±0.05 for xG. Any difference must come from a documented definition choice.

## 9. Milestones
0. Repo survey and `docs/hub-mapping.md` (section 0).
1. Canonical model, coordinate normalisation and ID resolution on top of the existing fetchers; Opta qualifier parsing for goal-mouth, keeper and woodwork data.
2. New sources: StatsBomb open data (A), ClubElo (E), football-data.co.uk (F), Understat (G).
3. Possessions, game state, all section 4 metrics, data quality gates.
4. xG, PSxG placement, xT and VAEP training; win simulation; provenance manifests.
5. `hub build --whoscored` entry point, automatic cross-source ID resolution, MatchTheme pipeline (5.4), then boards 1–20 in the repo's existing style.
6. Boards 21–31 and the season batch ingest (B) for the competition scatters and percentile pools.
7. Narrative generator, full report export, golden-number test passing, generalisation corpus (5.4) green in CI.
8. Phase 2: tracking and 360 data (D), exact line-breaking passes, off-ball pressing.
