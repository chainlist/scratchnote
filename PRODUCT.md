# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

A Tauri 2 app: the interface is a SvelteKit SPA in the OS webview. On the desktop it is a borderless window that draws its own frame; Windows first, with macOS and Linux built and released but not primary test targets. Since 0.7.0 (2026-10-08) it also runs on Android phones, the same interface in the system WebView: icons and touch targets grow on a touch screen, text follows the phone's text size, Back closes what is open and swipes move between days, but there is no phone-specific layout yet.

## Users

People working at a computer (developers, product people, students, anyone mid-task) who have a thought in the middle of something else and need to put it down without leaving what they are doing, then find it again later. The maintainer is one of them and uses it daily as a journal and a work log. Typical volume is modest: a personal journal is tens of kilobytes of markdown, though the app must stay fast at ten thousand notes.

## Product Purpose

Scratchnote is a scratchpad for the thoughts that come up mid-task. A global shortcut opens a small capture window over any app; the user types, saves, and the window is gone. Every note lands in that day's markdown file. Later, the main window lets the user read a day like one continuous journal page, search by words or by meaning, open the notes closest to one, and follow threads of related notes across days.

Success means capture is fast enough that the user never hesitates to use it, and that a note written weeks ago comes back when it is relevant.

## Positioning

Three mechanisms together, none of which a neighbouring notes app can truthfully claim all at once:

1. **Two-second capture.** Hotkey to saved in under two seconds; saving never waits on the model, the index or anything else.
2. **Plain daily markdown as the source of truth.** One file per day, plus a file per page. The user can open, edit, grep or sync (git, Syncthing, iCloud) them without the app, and the app reads back what they changed.
3. **Meaning, fully local.** Search by meaning, similar notes, recall, threads across days and the map of a space all come from a small embedding model running on the CPU. Nothing about the notes touches the network; the only download for notes is the model itself, once, when asked.

Plugins are a capability, not part of the core claim.

## Operating Context

- Used in short bursts on top of other work: the capture window appears over an editor, a browser or a meeting, and must get out of the way.
- The main window lives in the tray and is opened to read back a day, search, review threads, or write longer pages such as meeting notes.
- Keyboard-first: capture, save, page handoff, space picking and the command center all work from the keyboard (`Ctrl+Enter` save, `Ctrl+Shift+Enter` page, `Esc` dismiss, `Ctrl+1..9` spaces).
- On a phone there is no capture hotkey and no tray: the app opens on the day, a note is written there, and touch, Back and swipes stand in for the keys.
- Spaces keep areas of life apart (work, side projects, personal), each a folder of its own.

## Capabilities and Constraints

- Quick capture window, main window (day view, calendar, search, threads, mentions, map, pages), tray, command center, settings.
- Notes are markdown typed as-is; the editor previews markdown but is not a rich text editor. Attachments are plain links.
- Pages: a titled note with its own markdown file, shown on its day as a card.
- Threads, mentions, pins to the left edge, the day ahead (notes that look forward to a later day).
- Mentions are part of the app itself (SPEC.md 3.10).
- Core plugins (Basics with Tasks and Highlights, Stats, Journal view) and opt-in community plugins from a registry; see SPEC.md 3.9 and PLUGINS.md.
- Appearance belongs to the user: light, dark or system theme; nine bundled font presets (Inter default, Geist, IBM Plex Sans, Atkinson Hyperlegible Next, Literata, Caveat, Shantell Sans, Playpen Sans, system); accent colour presets; corner radius; font size. Every surface must hold up under all of them. Note times on a timeline follow the chosen font with tabular figures (decided 2026-10-07, for a journal rather than a log); other dates and counts stay monospace.
- Interface in six languages (English, French, Spanish, German, Italian, Portuguese), switched live; layouts must survive the longer languages. Note bodies are never translated.
- Built from shadcn-svelte components added via its CLI, on Tailwind CSS 4.
- No accounts, no sync service, no telemetry, no cloud features. No network call on a timer or at launch for notes.
- Performance is a product feature: views that take over 150 ms show a spinner over the view being left; the window must never wait on model work.
- SPEC.md is the detailed product and technical record; PLUGINS.md documents the plugin API.

## Brand Commitments

- Name: Scratchnote. App icon in `src-tauri/icons/`.
- Voice: plain, concrete and short. Copy says what happens in everyday words ("Nothing captured on this day.", "Updating restarts Scratchnote.", "the back arrow returns to the view it came from"), avoids jargon and marketing claims, and speaks of the user's notes, not of features.
- Privacy is stated plainly and must stay true: "Everything stays on this computer."

## Evidence on Hand

- Real UI copy in six languages: `messages/*.json`.
- Changelog of shipped features: `CHANGELOG.md`.
- Onboarding flow with the product's own description of itself: `src/routes/onboarding/`.
- Measured performance figures in SPEC.md (for example, map layout timings on 10,269 notes).
- No testimonials, user counts, reviews, press or pricing exist. Do not invent any.

## Product Principles

1. **Capture never waits.** Nothing (model, index, layout, plugin) may stand between the hotkey and a saved note.
2. **The files are the truth.** Every feature must survive the user editing the markdown by hand; the app reads, it does not own.
3. **Local means local.** No feature may send note content off the machine or phone home on its own.
4. **Get out of the way.** The interface serves a thought in passing and a quiet read later; it should feel like a journal page, not a dashboard.
5. **The user's look wins.** Theme, font, accent, size and radius are theirs; designs must work across every combination rather than depend on one.

## Accessibility & Inclusion

No formal compliance target. Keep it usable: keyboard-first throughout, readable at the user's chosen font and size, honest contrast in light and dark themes and with every accent, and copy that holds in all six languages.
