---
name: Scratchnote
description: A scratchpad for thoughts mid-task, read back as a quiet journal page.
colors:
  page: "oklch(0.145 0 0)"
  surface: "oklch(0.205 0 0)"
  surface-raised: "oklch(0.269 0 0)"
  hairline: "oklch(0.269 0 0)"
  hairline-strong: "oklch(0.371 0 0)"
  meta: "oklch(0.632 0 0)"
  part-label: "oklch(0.708 0 0)"
  time-lit: "oklch(0.87 0 0)"
  body-text: "oklch(0.922 0 0)"
  strong-text: "oklch(0.97 0 0)"
  foreground: "oklch(0.985 0 0)"
  accent: "oklch(0.922 0 0)"
  accent-foreground: "oklch(0.205 0 0)"
  destructive: "oklch(0.704 0.191 22.216)"
typography:
  headline:
    fontFamily: "'Inter Variable', ui-sans-serif, system-ui, sans-serif"
    fontSize: "1.5rem"
    fontWeight: 600
    lineHeight: "2rem"
    letterSpacing: "-0.025em"
  masthead:
    fontFamily: "'Inter Variable', ui-sans-serif, system-ui, sans-serif"
    fontSize: "1.5rem"
    fontWeight: 500
    lineHeight: "2rem"
  title:
    fontFamily: "'Inter Variable', ui-sans-serif, system-ui, sans-serif"
    fontSize: "1rem"
    fontWeight: 500
    lineHeight: "1.5rem"
  body:
    fontFamily: "'Inter Variable', ui-sans-serif, system-ui, sans-serif"
    fontSize: "1rem"
    fontWeight: 400
    lineHeight: "1.75rem"
  secondary:
    fontFamily: "'Inter Variable', ui-sans-serif, system-ui, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 400
    lineHeight: "1.5rem"
  time:
    fontFamily: "'Inter Variable', ui-sans-serif, system-ui, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 400
    lineHeight: "1.25rem"
    fontFeature: "tnum"
  part:
    fontFamily: "'Inter Variable', ui-sans-serif, system-ui, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 500
    lineHeight: "1.25rem"
  mono:
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace"
    fontSize: "0.875em"
    fontWeight: 400
rounded:
  fixed-sm: "0.25rem"
  sm: "0.375rem"
  md: "0.5rem"
  lg: "0.625rem"
  xl: "0.875rem"
  full: "9999px"
spacing:
  item: "0.75rem"
  gutter: "1.5rem"
  margin-column: "4.5rem"
  part-room: "2rem"
  gap-room-max: "4rem"
  spread-gap: "2rem"
  reading-measure: "70ch"
  view-width: "48rem"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.accent-foreground}"
    rounded: "{rounded.lg}"
    height: "2rem"
    padding: "0 0.625rem"
  button-ghost:
    textColor: "{colors.meta}"
    rounded: "{rounded.lg}"
    height: "2rem"
    padding: "0 0.625rem"
  button-ghost-hover:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.foreground}"
  quiet-action:
    textColor: "{colors.part-label}"
    rounded: "{rounded.fixed-sm}"
    padding: "0.125rem 0.375rem"
    typography: "{typography.time}"
  quiet-action-hover:
    backgroundColor: "{colors.surface-raised}"
    textColor: "{colors.body-text}"
  input:
    textColor: "{colors.foreground}"
    rounded: "{rounded.lg}"
    height: "2rem"
    padding: "0.25rem 0.625rem"
  note-editor:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.strong-text}"
    rounded: "{rounded.fixed-sm}"
    padding: "0.25rem 0.5rem"
    typography: "{typography.body}"
  timeline-item:
    textColor: "{colors.body-text}"
    padding: "0.75rem"
    typography: "{typography.body}"
  timeline-time:
    textColor: "{colors.meta}"
    typography: "{typography.time}"
    width: "4.5rem"
  timeline-time-hover:
    textColor: "{colors.time-lit}"
  page-card:
    rounded: "{rounded.lg}"
    padding: "0.75rem 1rem"
    textColor: "{colors.strong-text}"
  page-card-hover:
    backgroundColor: "{colors.surface}"
---

# Design System: Scratchnote

## Overview

**Creative North Star: "The Breathing Page"**

Scratchnote reads like a journal page, not a dashboard. A day is one column of notes at reading measure, with a margin to its left where the quiet facts hang: each note's time in small tabular figures, and the name of each part of the day (Morning, Afternoon, Evening, Night) where it begins. A hairline rail runs down the gutter between the margin and the column, with a small dot on it for each note. The space above a note grows with the time since the one before it, so a busy hour reads close and a quiet afternoon reads as room, a long stretch of rail. Every timeline in the app, including plugin timelines, is drawn from the same item, so they all breathe alike.

The look belongs to the user. Theme (light, dark, system), one of nine bundled fonts, an accent preset, the corner radius and the text size are settings, and every surface must hold under every combination. The values in the frontmatter are therefore the defaults (dark theme, Inter, neutral accent, 0.625rem radius, 16px), not brand constants: build with the roles and tokens, never with the literal values. The neutral scale is tinted with the accent hue and mirrored in light mode, so the page is the darkest step in dark and the lightest in light, from one set of token names.

The surface is flat, near-monochrome and quiet. Colour is reserved for the accent (what can be acted on, and the cue that something is under the pointer or focus), for the user's own names and threads (each with its own hue), and for errors. Motion is short and purposeful, with a reduced-motion fallback for every animation.

**Key Characteristics:**
- One reading column (70ch) beside a 4.5rem margin of times and part-of-day labels.
- Space between notes scaled to the time between them, capped at 4rem.
- Tinted neutrals mirrored per theme; the user's font, accent, radius and size.
- Flat surfaces; hairline borders and tonal steps instead of shadows.
- Quiet text in one tuned meta tone (about 5.7:1 on the page in both themes).
- Keyboard-first: every hover cue is also a focus cue.

## Colors

A tinted neutral scale carries almost everything; the user's accent appears only where it means something.

### Primary
- **The User's Accent** (`--primary`, default neutral `{colors.accent}`; presets blue, violet, green, orange, rose): primary buttons, the selected ribbon button and its edge bar, a timeline item's mark on the rail on hover or focus, link underlines (70% mix, full on hover), mention chips (18% mix, 30% on hover), the pulse of a note opened from a link (16% mix), focus rings (`--ring` follows the accent). Each preset has a swatch per theme: deeper in light, with white text on it, and lighter in dark, with dark text on it, so the accent holds at least 4.5:1 as text on the page and under its own label in both.

### Neutral
- **Page** (`neutral-950`, `{colors.page}`): the window and page background in both themes (mirrored to near-white in light mode).
- **Surface** (`neutral-900`, `{colors.surface}`): the note editor's field, a hovered page card, cards and popovers in dark mode.
- **Raised Surface / Hairline** (`neutral-800`, `{colors.surface-raised}`): hover fills of small actions, inline code, borders and dividers.
- **Strong Hairline** (`neutral-700`, `{colors.hairline-strong}`): hovered borders, quote rules, the scrollbar thumb.
- **Meta** (`--color-meta`, a mix of neutral-400 and neutral-500, `{colors.meta}`): times, dates, counts, hints, empty-day lines, the quieter half of a masthead.
- **Part Label** (`neutral-400`, `{colors.part-label}`): the part-of-day names, one step stronger than the times; also resting small actions.
- **Lit Time** (`neutral-300`, `{colors.time-lit}`): a time while its item has the pointer or focus.
- **Body Text** (`neutral-200`, `{colors.body-text}`): note bodies at rest.
- **Strong Text** (`neutral-100`, `{colors.strong-text}`): page card titles, text being edited.

### Status
- **Destructive** (`--destructive`, `text-destructive`): errors and the controls that delete. Deeper than shadcn's default in light mode so an error also reads on the capture window's surface.
- **Warning** (`--warning`, `text-warning`): what needs a second look but has not failed, such as a restart to come. Amber, deep in light mode and bright in dark.
- Never a Tailwind palette colour (`red-400`, `amber-500`) for status text: those hold in one theme only.

### Named Rules
**The Meta Tone Rule.** All quiet text uses `--color-meta`, tuned to about 5.7:1 on the page in both themes; shadcn's `--muted-foreground` points at it, so `text-muted-foreground` and `text-meta` are one tone. Never reach for neutral-500 (4.2:1) or lower for readable text; neutral-600 measured 2.5:1.

**The Earned Accent Rule.** The accent marks what can be clicked or what has the pointer or focus. It never colours the user's own emphasis, headings, or decoration. Two exceptions: it shades counts in heatmaps (the calendar's days, the Stats plugin's weeks, the startup timings), and it marks the onboarding steps, where the app introduces itself.

**The Mirrored Scale Rule.** Use the neutral steps (`neutral-950` page to `neutral-100` text), never light/dark variants: the scale flips with the theme, so one class holds in both. One step is not a straight mirror: in light mode `neutral-900` sits at 0.955 rather than 0.97, so a hover or selection fill shows on the 0.985 page.

**The Own Hue Rule.** A name or thread carries its own hue (`--hue`, oklch): a tint behind its letter or chip (`.name-tint`) and a mark for its activity (`.name-mark`), light in light mode and dark in dark mode, at a fixed lightness so every hue reads alike. A thread takes its name's hue when it was found among a name's notes, else one from its id (`threadHue`), and keeps it everywhere: its arc, its lane, its dots on the map and its pin. A pin wears its target's tint. These are the only per-item colours.

## Typography

**Body Font:** the user's chosen font (default Inter Variable, with ui-sans-serif, system-ui, sans-serif). Presets: Inter, Geist, IBM Plex Sans, Atkinson Hyperlegible Next, Literata, Caveat, Shantell Sans, Playpen Sans, system.
**Label/Mono Font:** the platform monospace stack for code, file paths and error details.

**Character:** one family carries the whole interface, so the user's choice of voice is the voice. Hierarchy comes from size and weight steps, not from mixing families.

### Hierarchy
- **Headline** (600, 1.5rem, tight tracking): a view's title in its header.
- **Masthead** (500, 1.5rem): the day's weekday, followed by its date at weight 400 in the meta tone. A neighbouring day's name in the spread sits at 500, 1.125rem in muted text.
- **Title** (500, 1rem): page card titles.
- **Body** (400, 1rem, 1.75rem line height): note bodies, at most 70ch wide.
- **Secondary** (400, 0.875rem): previews, list rows, the day's "from earlier" lines.
- **Time** (400, 0.75rem, 1.25rem line height, tabular figures): timeline times and dates, right-aligned in the margin, in the user's font.
- **Part** (500, 0.75rem): part-of-day names in the margin, exposed as headings one level under the view's own.
- **Mono** (0.875em): inline code, code blocks, paths and error details.

### Named Rules
**The Tabular Time Rule.** Timeline times take the user's font with tabular figures so they line up down the margin. Other dates and counts that must align use tabular figures or monospace; nothing switches family for decoration.

**The Size Is Theirs Rule.** Everything sizes in rem against the user's root size (14, 16, 18 or 20px). Never set type in px.

## Layout

A view is a single centred column (max 48rem). On the day page, a wider window shows the previous day to the left, and a wide one the next day to the right too, each column up to 48rem with 2rem between; neighbours open by their date and have no new-note field.

Every timeline item is a two-column grid: a 4.5rem margin column and the body column, 1.5rem apart, with 0.75rem of padding pulled back into the gutter by a matching negative margin so text aligns with the view's edge. The body is held to 70ch. Below 24rem of the item's own width (a docked view, a plugin's panel, a thread's day), the grid collapses to one column: the time heads the body, left-aligned, and the rail and its marks are hidden; a page's icon then leads its time.

Room above an item follows the time since the previous item on the same day: none within 20 minutes, then 1rem per hour after that, capped at 4rem. A part-of-day label, which takes 2rem with its own room, counts against that room rather than adding to it. Items spanning days get no gap room and no part labels; their dates separate them. Parts of the day: Morning 05:00 to 12:00, Afternoon 12:00 to 17:00, Evening 17:00 to 22:00, Night otherwise.

### Named Rules
**The Time Is Space Rule.** Vertical rhythm in a timeline comes from time, through `gapRoom`, never from fixed spacers, hour grids, or absolute positions.

**The Margin Holds the Facts Rule.** Times, dates, part names and per-item controls (a selection box, the note's menu cue) live in the margin or beside it, level with the first line; the body column holds only the note.

## Elevation & Depth

The app is flat. No component in the app's own code uses a box shadow; depth comes from tonal steps on the mirrored neutral scale (page, surface, raised surface) and from hairline borders. shadcn-svelte overlays (popovers, dialogs, menus) keep their stock treatment.

### Named Rules
**The Tonal Step Rule.** To lift something, move it one neutral step (page to surface, surface to raised) or give it a hairline. Hover lifts a page card from a hairline to the surface tone over 300ms; it does not cast a shadow.

## Shapes

Corners follow the user's radius setting (0, 0.375, 0.625 or 0.875rem) through `--radius`; the scale derives from it (sm 0.6x, md 0.8x, lg 1x, xl 1.4x), so a "none" setting squares the whole interface. Small inline controls (quiet actions, the note editor's field, inline code, mention chips) use a fixed 0.25rem. Pills and dots (a timeline item's dot, badges, thread-arc dots, the scrollbar thumb) are fully round. Borders are 1px hairlines; a missing page's card uses a dashed one.

## Components

Quiet at rest, legible on approach: controls brighten or fill one tonal step on hover and on focus alike.

### Buttons
- **Shape:** the user's radius (`{rounded.lg}`), 2rem tall by default, 1.75rem for the small icon buttons in headers.
- **Primary:** the accent fill with its own foreground; hover moves it away from the page (`--primary-hover`), toward the text colour for a coloured accent and toward its own label for the neutral one, so the label keeps its contrast.
- **Ghost:** no fill; hover fills with the muted tone. Header navigation (previous and next day, back) uses muted text that turns foreground on hover.
- **Hover / Focus:** a 3px focus ring at 50% of the accent-following ring colour around a 1px solid edge in the full ring colour (shadcn's `border-ring`, or `outline-ring` where a control has no border), so the focus holds 3:1 with every accent; a 1px press nudge. The neutral ring is neutral-500 in light and neutral-400 in dark for the same reason.
- **Quiet actions** (Save, Cancel, a note's menu items in the timeline): 0.75rem text, part-label tone, fixed 0.25rem corners, filling with the raised tone and brightening on hover. Save is the word, not an icon.

### Inputs / Fields
- **Style:** shadcn input, 1px input-tone border, transparent fill, the user's radius.
- **Note editor:** a field on the surface tone with a hairline border and fixed 0.25rem corners, body type at 1.75rem lines, 3 to 16 lines tall; the border steps to neutral-600 while focused.
- **Error:** an inline line in the destructive colour with an icon, details below it in the meta tone and monospace.

### Navigation
- **Ribbon:** a left vertical strip of 1.75rem icon buttons. Resting icons are muted; the selected one takes the accent, with a 0.25rem accent bar on the strip's left edge. A count badge is a full-round accent pill.
- **View header:** the headline title with icon buttons beside it; a compact form at 0.875rem semibold when space is short.

### Cards / Containers
- **Page card:** a page shown on its day. Hairline border, `{rounded.lg}`, 0.75rem by 1rem padding, title at 500 in strong text, a two-line preview at 0.875rem, word count and stats in the meta tone with tabular figures. Hover: border to neutral-700 and fill to the surface tone over 300ms; its actions fade in on hover or focus.
- **Missing page:** the same frame with a dashed border and muted text.

### Timeline Item (signature)
The unit every timeline is built from (`TimelineItem`, exposed to plugins as `Timeline`). The time hangs in the margin, right-aligned, in the meta tone. A 1px neutral-800 rail runs down the middle of the gutter, through the room above each item and the part names, and stops at the first and last items' marks. Each item's mark sits on the rail level with its time's first line: an 8px round dot with a neutral-700 hairline, or for an item with an icon (a page, a plugin's item) the icon in a 1.125rem box with the same hairline. While the pointer or focus is anywhere in the item the time brightens to neutral-300 and the mark takes the accent (dot filled, box edge and icon). That brightening and that mark are the only hover and focus cue: the row itself is never filled. A time can be a button (to the note on its day). An optional date stacks above the time for timelines that span days. The first item of a part of the day carries the part's name above it in the margin. "Double-click to edit" appears only under keyboard focus.

### Thread Arc
A thread's span in time: a hairline axis with one round dot per day that holds notes, sized by count, in the thread's own hue, with the first and last dates in tabular meta text below. Suggested threads draw at half opacity. Dots grow 1.5x on hover and take the focus ring.

### Motion
Reduced motion is on when the user turns on Reduce motion in Appearance or the system asks for it: `applyAppearance` then puts `less-motion` on the root, and every fallback below, the `motion-reduce:` variant and `moveLess()` follow that class.
- **Page in** (350ms, `cubic-bezier(0.22, 1, 0.36, 1)`): a view rises 8px out of a 3px blur. Reduced motion: a 200ms fade.
- **Note pulse** (1.2s): two soft accent pulses on a note opened from a link. Reduced motion: one slow fade.
- **Calendar wave** (500ms, 30ms per row plus column): days rise into place. Reduced motion: a 300ms fade.
- **Search glow** (2.4s loop): text being searched by meaning breathes from the meta tone to the accent and back. Reduced motion: still, in the meta tone.
- **Overlays** (menus, popovers, tooltips, dialogs, 180ms in, 110ms out): shadcn's zoom and slide, on the settle curve whatever duration the component set. Reduced motion: the fade alone.
- **More notes** (200ms): more than three notes coming into a list at once (a new search, Show more) fade in together. A note saved, deleted or moved shows or goes at once: tried with its room opening and closing (2026-10-10), it felt clunky.
- **Suggestion closes** (200ms): a thread suggestion kept or dismissed closes its room, so the ones below move up. Reduced motion: a 150ms fade.
- **Pins** (200ms in, 150ms out, 250ms slide): a pin the user adds grows in from 60%, one taken off shrinks away, and the others slide to their new places, as when one is moved. Pins read at launch, on a space switch or as their threads load just appear. Reduced motion: a fade, no slide.
- **Dock in** (300ms): the dock comes 1rem in from its own edge as it opens or moves sides. Reduced motion: a 200ms fade.
- **Onboarding step** (300ms): each step comes 24px in from the side being gone to; its progress bar fills from the start. Reduced motion: a 150ms fade, the bar at once.
- **Tick** (150ms fill, 250ms draw): a task box ticked on a card fills, then its check is revealed from the left; Saved in the capture window draws its check. Reduced motion: the fill alone, the check already drawn.
- **Day opens** (signature, about 0.9s at most): wherever a list of notes is drawn anew (a day, its neighbours, search, a thread), the rail draws down the margin at 45ms a note, each mark lights in the accent as the line reaches it and cools to its rest, and its note rises 6px out of a 2px blur. Only the first 16 move, the stagger stops at the ninth, and a note written later just shows. The view around it only fades. A day brought in by a swipe does not unfold. Reduced motion: the notes fade in together, the rail is there.
- **Capture sends** (signature, 170ms): on save the text lifts 10px out of the field and blurs away while the send arrow slips right; the window hides once both the save and the motion are done, so the save never waits. Each time the window comes up its edge lights in the accent and cools to its hairline over 700ms. Reduced motion: the text fades; the edge still lights.
- **Gliding marks** (250ms): one mark per group slides to whatever is chosen rather than one going out and another lighting: the ribbon's edge bar (between buttons and pins), a tab list's pill or underline, a segmented picker's fill (`follow` in `#lib/helpers/motion.ts`, `.glide-mark` in layout.css). Reduced motion: the mark moves at once and fades.
- **Day title** (250ms): another day's name comes in 12px from the side it lies on, a later day from the right. Reduced motion: a fade.
- **Look changes** (260ms): a new theme, accent, font, size or radius crossfades the window (a view transition). The first paint is at once.
- **Small feedback**: a pressed button sinks (icon buttons to 90%, others to 97-98%) over 75ms; a timeline mark grows 1.5x (a page's box 1.1x) under the pointer or focus; the day arrows lean 2px toward their day; the Add a note plus turns a quarter; a note's menu cue slides 4px in as it shows; a checkbox's check is revealed from the left and a radio's dot grows; a switch's thumb settles; a link's underline thickens; the threads count grows in when it changes; the error banner drops 6px into place; the new-note editor and the choosing boxes grow in.

#### Named Rules
How to give a new feature or view its motion. Every rule holds under every theme, font, size and language, on a phone as on the desktop.

**The Earned Motion Rule.** Motion only does one of four jobs: it acknowledges an action (a press, a tick), shows where something went or came from (a gliding mark, a day from its side), makes a change of state legible (a suggestion closing its room), or is a surface's one authored moment (the day opening, the capture sending). If taking it away loses no meaning, it is decoration: leave it out. A surface has at most one signature; everything else stays small.

**The Never Wait Rule.** Motion never holds up work. A save, a load, a keystroke or a navigation happens at once and the animation runs beside it; where something must follow both, the slower of the two decides, as the capture window hides once the save and its 170ms are done. Nothing moves while typing, and no motion blocks input for more than 200ms.

**The Timing Scale Rule.** Feedback 75 to 150ms; a change of state 150 to 250ms; overlays, marks and views 250 to 350ms; a signature up to 450ms per element and under a second in all. An exit takes about two thirds of its entrance. Arrivals use the one curve, `settle` (`ease-settle` in Tailwind); exits ease in. No bounce, no elastic, no new curve.

**The Small Distance Rule.** Things travel 2 to 12px, scale between 0.6 and 1.5, and blur at most 3px. Direction carries meaning: a thing comes from where it lies (a later day from the right, the dock from its own edge, a step from the side being gone to), never from an arbitrary corner.

**The One Mark Rule.** When the choice in a group changes, one mark glides to it (`follow`) rather than one going out and another lighting. A new group of choices (tabs, a segmented picker, a rail of buttons) gets its mark this way.

**The Change Not Presence Rule.** Motion marks what the user just did, not what merely is. What arrives at launch, on a space switch, by a sync or as data loads appears at once; only the user's own change moves, as pins only slide after the user's own pin change (`pinsSaved`). A list drawn anew may open, staggered at most 9 steps and moving at most its first 16 items.

**The Still Column Rule.** In the reading column a single note saved, deleted or moved shows or goes at once, with no height or room animation (tried 2026-10-10, clunky). The map's dots never animate, and nothing moves on long lists row by row.

**The Hover Is Focus Rule.** A motion cue on hover also plays on keyboard focus (`group-hover` with `group-focus-within` or `focus-visible`), as every hover cue does.

**The Less Motion Rule.** Every animation has a fallback under `.less-motion` (or `motion-reduce:`; `moveLess()` in script), never `@media (prefers-reduced-motion)`, which misses the app's own Reduce motion setting. The fallback moves nothing and fades in 120 to 200ms, but keeps what confirms an action: a tick still fills, an edge still lights.

**The Reuse Rule.** Build from what exists: `settle`, `rise`, `pop`, `room` and `follow` in `#lib/helpers/motion.ts`, the keyframes in layout.css. Animate transform, opacity, colour and a bounded filter; never width, height or margins frame by frame, except `room` on one short row. A loop (the search glow) runs only while its work does.

**The Written Down Rule.** A new motion gets its line in the list above: its name, duration, what moves and its reduced fallback. Motion tried and rejected keeps its line too, with why, so it is not tried again.

## Do's and Don'ts

### Do:
- **Do** build with roles and tokens (`bg-primary`, `text-meta`, `neutral-*`, `--radius`) so every theme, font, accent, radius and size holds.
- **Do** use `TimelineItem` (or the plugin `Timeline` builder) for anything listed by time, and pass `daySpacing` results for one day's items.
- **Do** put quiet text in `--color-meta`, and give figures that must line up `tabular-nums`.
- **Do** make every hover cue a focus cue too (`group-hover` with `group-focus-within` or `group-focus-visible`).
- **Do** give every animation a reduced-motion fallback under `.less-motion` (or Tailwind's `motion-reduce:`), never `@media (prefers-reduced-motion)`, which misses the app's own setting.
- **Do** leave room for the longest of the six languages: truncate or wrap labels, never fix widths to English.

### Don't:
- **Don't** fill the whole row on hover, or draw a second rail or ruled gutter beside the timeline's own; the mark on the rail is the cue.
- **Don't** use an hour grid or absolute positions for notes; space comes from `gapRoom`.
- **Don't** colour parts of the day with bands or tints; they are named in the margin, not painted.
- **Don't** use neutral-600 or darker for text meant to be read.
- **Don't** use box shadows for depth; step the neutral scale or add a hairline.
- **Don't** hardcode a font family, a px font size, a fixed accent or a fixed radius in app components.
