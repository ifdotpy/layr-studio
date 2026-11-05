# Studio redesign brief: dispatch board

Status: experiment on branch `ui/redesign`. Not for `main` until the owner
approves it after a visual review.

## Subject, audience, job

Layr Studio is a dispatch console for a fleet of AI coding agents that work in
parallel across several projects. One operator runs it for long sessions on a
desktop and checks it from a phone.

Primary job: in one glance answer "what is running where, who waits for me,
what broke, what it costs", then steer the agent that needs it.

## Concept

Every chat and every agent is a strip on a dispatch board. A strip shows its
state with a status icon or dot in a fixed leading slot, never with a tinted
fill or a colored rail. Strips are ordered by what needs the operator. The
status marks and the "Needs you" lane are the one bold element. Everything
around them stays quiet: hierarchy comes from type weight and spacing, not
from boxes.

## Palette

The visual language follows the Vercel Geist system
([reference DESIGN.md](https://github.com/VoltAgent/awesome-design-md/blob/main/design-md/vercel/DESIGN.md)):
ink on a neutral gray scale, hairline borders, and stacked soft shadows. State
is the only saturated color. Ink, not a hue, marks the one primary action.

Dark is OLED black. The page, panels, fields, menus, and dialogs are `#000`,
so unlit pixels draw no power. Hairlines and small raised fills carry the
structure; shadows on black are replaced by a hairline ring. The terminal is
an ink panel in both schemes. Modal backdrops do not blur.

| Name     | Light     | Dark      | Meaning                              |
| -------- | --------- | --------- | ------------------------------------ |
| Page     | `#FAFAFA` | `#000000` | Page background                      |
| Surface  | `#FFFFFF` | `#000000` | Panels, composer, dialogs            |
| Raised   | `#F2F2F2` | `#1A1A1A` | Selected rows, badges, user messages |
| Hairline | `#EBEBEB` | `#262626` | Dividers and card borders            |
| Ink      | `#171717` | `#EDEDED` | Text and the primary action fill     |
| Muted    | `#666666` | `#A1A1A1` | Secondary text                       |
| Blue     | `#0761D1` | `#52A8FF` | Links, focus, and `working`          |
| Amber    | `#AB570A` | `#F5A623` | `answer`: the operator must act      |
| Red      | `#C50000` | `#FF6166` | `error`                              |
| Green    | `#107D32` | `#62C073` | `unread`: new result to read         |

Token owners are `src/theme.ts` (Mantine ramps: `inkLight`, `inkDark`, `gray`,
`dark`, `blue`, `red`, `green`, `yellow`, `violet`), `src/studio-theme.css`
(surfaces, hairlines, elevation, radii, font stacks, terminal colors), and
`src/design/state.css` (semantic state colors). `src/design/components.css`
maps the tokens onto shared Mantine parts. The table records reviewed values;
consume the owners, not copied hex values. `src/design/contrast.test.ts`
requires 4.5:1 for text and state colors on page and panel in both schemes.
Soft state backgrounds use `--studio-state-tint`: 5% in light and 14% in dark.
They are for small chips and inline alerts only, never for a whole strip.

Ink is a virtual Mantine color: shade 7 of `inkLight` fills primary actions in
light and shade 4 of `inkDark` in dark. Its foreground is the page color.
A button without a variant is secondary: a surface with a strong hairline.
Toolbar and composer chrome opt into the subtle variant. Blue is for links,
focus rings and the working state only; a request that waits for the
operator takes the answer color.

## Type

Geist for text and Geist Mono for code, terminals, and technical captions.
Both are bundled from `src/assets/fonts` (SIL Open Font License) so the
desktop app renders offline. Weights 400, 500 and 600 only. Scale in rem: meta
0.75, code 0.8125, body 0.875, title 1, section 1.25, display 1.5. Body text
tracks at -0.01 em; titles and sections track tighter. Counters, limits,
costs and times use tabular numerals. Sentence case everywhere: no all-caps
labels or badges.

Radii: 6 px for controls and menu items, 8 px for cards and menus, 12 px for
dialogs and the composer, full for pills.

## Layout

Desktop:

```
┌ Fleet ───────────────┬ Chat ─────────────────────────────┬ Team ───────────┐
│ Needs you        2   │ Title                state  team ▮▮▮│ ▮ Lead  Working │
│ ▮ arb2  Review docs  │───────────────────────────────────│ ▮ W1    Tests   │
│ ▮ attar Approve run  │ transcript                        │ ▮ W2    Done    │
│ Projects             │                                   │ ▮ Review Idle   │
│ af-utils    ▮2 ▮1    │                                   │                 │
│  ▮ Обновление…   3m  │ composer                          │                 │
│ codex-studio         │ Lead Astra 6 High   Workers Luna 6 │                 │
└──────────────────────┴───────────────────────────────────┴─────────────────┘
```

Phone: the fleet board is the home screen. A chat opens as a detail screen
with Back. "Needs you" stays on top.

Alignment: text left aligned; numbers right aligned in their column.

## Principles

1. State is the only saturated color; ink marks the one primary action.
2. Hierarchy by type and spacing. Borders only mark a grouping boundary;
   shadows only on overlays.
3. Order by urgency: `answer`, `error`, `working`, `unread`, `paused`, `none`.
4. Dense and legible: one line per strip; a second line only for live activity.
5. Mantine components and native controls; no custom keyboard handlers.
6. Component CSS uses tokens only. Raw sizes live in the token files.
7. Copy names things by what the operator does, in plain sentence case. Show
   separate facts as separate elements, not as `A · B · C` strings.

## Review against generic defaults

- Near-black with one acid accent: rejected. The black base carries a state
  scale where each color has a meaning; the primary action is ink.
- Identical rounded cards with shadows: rejected for strips with a status
  slot.
- Middle-dot meta strings: replaced with columns and separate elements.
- Monospace body labels: rejected; tabular Geist numerals instead. Geist Mono
  is for code and technical captions only.

## Contract for all workstreams

- Tokens: `src/design/state.css` (state colors, rail size) and the Mantine
  theme in `src/theme.ts`. Only the foundation workstream edits them.
- State: any element with `data-studio-state="<ChatIndicatorKind>"` gets
  `--studio-state` for its own marker (an icon, a dot, or status text). An
  inline marker is `<span class="studio-state-dot" data-studio-state="...">`.
  No JS.
- State colors: `var(--studio-state-<kind>)`; soft backgrounds:
  `var(--studio-state-<kind>-soft)`.

## Shared geometry

`src/design/layout.css` owns the rem geometry, imported by `studio-theme.css`
for the application and Storybook. Current reviewed values:

| Token                             |    rem | Purpose                            |
| --------------------------------- | -----: | ---------------------------------- |
| `--studio-border-width`           | 0.0625 | Group separator                    |
| `--studio-control-size`           |      2 | Desktop icon control               |
| `--studio-touch-target`           |   2.75 | Standalone coarse-pointer control  |
| `--studio-strip-min-height`       |   2.75 | Dense title/activity strip         |
| `--studio-strip-indent`           |  0.375 | One hierarchy level                |
| `--studio-conversation-title-min` |      4 | Phone conversation title minimum   |
| `--studio-team-size`              |     20 | Team dock inline size              |
| `--studio-form-max`               |     40 | Readable form column               |
| `--studio-settings-nav-size`      |     12 | Settings navigation inline size    |
| `--studio-room-list-size`         |     16 | Messages room selector inline size |
| `--studio-meter-size`             | 0.1875 | Context meter thickness            |

`src/studio-preferences.css` alone owns the custom transcript column through
`--studio-content-width-ratio` and its computed `--studio-content-column`.
Full uses the entire available conversation width minus its gutters. Original
layout retains the geometry from `workspace-layout.css`, which also owns
`--studio-sidebar-width` for the desktop fleet column and closed terminal dock. User-selected font
families and sizes still override the Geist theme defaults.

## Workstreams and file ownership

| Stream                | Owns                                                                                                                                  |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Foundation            | `theme.ts`, `studio-theme.css`, `src/design/*`, global rules in `style.css` and `appearance.css`, `realtime-voice.css`                |
| Execution and models  | `agents/ExecutionSettings*`, `AgentSetupPicker`, `models/*`, `UnifiedAgentSettings`, `AccountExclusionsControl`, chat settings dialog |
| Fleet board           | `Sidebar*`, `SidebarRow`, `sidebar/*`, `sidebar-projects.css`                                                                         |
| Conversation chrome   | conversation header, empty chat, composer footer layout, usage footer                                                                 |
| Team and activity     | team panel, `WorkerCard`, `team-navigation.css`, background tasks                                                                     |
| Settings and messages | Studio settings, project settings, Messages drawer                                                                                    |

A stream that needs a change in a file it does not own asks the owner through
the lead.
