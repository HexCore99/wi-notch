# Build your own Nusa-style Windows companion

Written on 12 November 2026. This document can be copied into a new project and given to a new coding harness. It describes the product, the intended scope, and a learning path. You do not need the old conversation to use it.

## 1. Project context for a new harness

I want to implement this application myself, in Rust, with guidance. Help me understand the code and Windows behavior as I build it. Work as a tutor, reviewer, and verifier. Give me one small checkpoint at a time, explain why it matters, and review my attempt before introducing the next feature. Do not generate the whole application or copy the existing implementation into my new project unless I explicitly ask.

**Nusa** is an existing Windows desktop companion built around a small notch at the top center of the screen. The notch exposes useful information and controls without opening a normal application window. Hovering expands it; leaving lets it collapse. Music, lyrics, timers, and small utilities live inside this shared surface.

**My project** is a fresh, independently written application inspired by that interaction. The working name is **Nusa Mini**; a new name can be chosen later. The first substantial goal is a reliable music notch. The next goal is the six-page utility app described below. The larger Nusa feature catalog is a later roadmap, not a requirement for the first release.

The existing reference project is at:

```text
C:\Users\HExCR\Documents\Prog\Nusa-Clone
```

It already has a Rust/GPUI implementation. Use it to explain behavior or compare results when requested. The new project should start separately so that I own the implementation and understand each step. Paths and links to the reference checkout will need adjusting after copying this document elsewhere.

The reference is not Nusa's recovered source code. Its `analysis/` directory contains bounded investigation notes about an installed Nusa 1.2.1.0 executable, plus records about our own clone. Static evidence, publisher descriptions, implemented code, and interactive verification are different kinds of evidence. Keep those distinctions when explaining them.

## 2. What the app should feel like

At launch, a small black clock notch sits against the top edge of the primary display. It has inward-curving top corners, rounded bottom corners, and a soft shadow. Hovering the visible shape expands it smoothly into a panel. Leaving the shape collapses it after a short delay. The shadow should not behave like an invisible hover target.

Inside the expanded notch, arrows and page dots switch between utilities. A pin keeps it expanded. Normal interaction with the overlay should leave the user's current application focused. Text entry can use a separate window that deliberately takes focus. A tray icon provides a dependable way to hide or exit the app.

Use these reference dimensions as a starting point, in **logical pixels**:

- Compact clock: **204 × 35**.
- Expanded Music: **540 × 162**.
- Expanded utility pages, including Lyrics: **540 × 236**.
- Shadow allowance: **24** to each side and below the shape in the current implementation.
- Collapse delay: about **280 ms** in the current implementation.

These are choices from our reference clone, not proven constants from the original Nusa. At a different display scale, logical dimensions must be converted consistently to native window coordinates.

Visual reference: [expanded music notch](screenshots/notch-expanded.jpg). Copy that image into the new project if visual comparison will be needed. The other screenshots include historical layouts; use the expanded notch image and [redesign record](analysis/gpui-ui-rewrite.md) for the current design.

## 3. Complete feature inventory for the six-page rebuild

The features in this section exist in the reference source. That does not mean every integration has passed a real interaction test. Build and verify them in stages.

### Window, interaction, and navigation

- Borderless, transparent, topmost overlay centered at the top of the primary display.
- Compact local clock in 12-hour format; local date shown in the expanded interface.
- Curved notch shape, rounded bottom corners, shadow, muted text, and simple icons.
- Hover expansion, delayed collapse, and pin/unpin behavior.
- Smooth size transitions between compact, Music, and utility dimensions.
- One shared shape definition for painting, native clipping, and pointer detection.
- Six pages: Music, Lyrics, Focus, Shelf, Tasks, and Water.
- Page arrows and clickable dots, with navigation wrapping through the pages.
- Mouse wheel changes pages except in Shelf and Tasks, where it scrolls their lists.
- Keep the panel open while an owned input dialog or outgoing drag needs it.
- Tray actions: show/hide, pin/unpin, open the data folder, and exit.
- Single-instance guard so launching twice does not create two notches.
- Foreground-window handling intended to hide the notch over a full-screen app.
- `--preview` for an expanded, pinned preview below the screen edge.
- `--quit` as a convenience command, with actual process exit verified during testing.

### Music

- Discover media sessions exposed by Windows, rather than building a separate integration for every player.
- Display title, artist, source player, playback state, and supplied album artwork.
- Show an empty state when no session exists and a fallback when artwork is absent.
- Previous, play/pause, next, and a seekable progress display when supported.
- Disable unavailable actions and handle a player rejecting a command.
- Cycle between available players.
- React to session, metadata, playback, and timeline changes.
- Prefer an explicit user selection; otherwise favor an appropriate playing session.
- Toggle mute on the default Windows audio output. This is a system output control, not a per-player mute.

The reference selection order is: explicit selected player if available; previous player if playing; Windows' current player if playing; first playing player; then previous/current/first as fallback. You can reproduce that after single-player support works.

### Lyrics

- Explicitly request synchronized lyrics for the selected title and artist from LRCLIB.
- Show loading, no-result, error, and successful-result states.
- Choose among timed-lyrics results using track duration as a matching signal.
- Parse LRC timestamps, multiple timestamps on one line, and millisecond offset tags.
- Sort timed lines and keep adjusted timestamps at or above zero.
- Highlight and follow the current line using media playback position.
- Click a line to seek when the player permits seeking.
- Retain lyrics in memory for the current track and clear them when the track changes.
- Keep network work away from the UI thread; handle timeout and track changes during a request.

The current clone makes one title/artist search. Metadata cleanup, fallback queries, and a durable disk cache are possible later improvements. A successful HTTP response does not establish that it is the correct recording.

### Focus

- Configurable countdown; default **25 minutes**.
- Start, pause/resume, reset, remaining time, and progress.
- Change duration in five-minute steps, bounded to **1–180 minutes**.
- Use elapsed time from a monotonic clock instead of assuming timer callbacks arrive exactly on schedule.
- On completion, open the Focus page and display a short message; the reference also pins it open.
- Persist the chosen duration. An active countdown does not resume after restarting in the baseline scope.

### Shelf

- Add files through the Windows file picker, including multiple selections.
- List saved file references, scroll the list, and open a file with its associated application.
- Remove a reference without deleting the source file.
- Reject duplicate or missing paths on addition; report missing files when opening them later.
- Initiate a native outgoing file drag offering copy/link effects.
- Persist paths, with a baseline limit of **40 entries**.

The reference does not implement dropping files onto the notch. Incoming drop support belongs to the later roadmap. Shelf storage in this rebuild means references to original paths, not a staging directory that copies or moves files.

### Tasks

- Add tasks through a small native text-entry window.
- Enter submits; Escape cancels; blank input is rejected.
- Mark a task complete or incomplete and delete a task.
- Scroll the task list and persist its contents.
- Baseline limits: **100 tasks**, with titles trimmed and capped at **160 characters**.

Editing, scheduling, reminders, ordering, and calendar views are later additions.

### Water

- Display today's logged amount, daily goal, and progress.
- Add or undo **250 ml**, without going below zero.
- Default daily goal: **2,000 ml**; provide a goal adjustment control.
- Reset the logged amount when the local calendar date changes.
- Persist the amount, date, and goal across restarts.

The reference cycles its goal through a small preset range. A clearer goal editor can be a deliberate later change. Scheduled reminders and drinking-history charts are outside this baseline.

### Persistence and reliability

- Save baseline state to `%LOCALAPPDATA%\NusaMini\state.json`, or use a distinct application folder for the new project.
- Store tasks, shelf paths, water amount/date/goal, and focus duration.
- Handle missing state with defaults and validate loaded values.
- Write a temporary file and replace the previous save; show save failures.
- Keep transient state separate: hover, animation velocity, selected session, pending requests, and active countdown state need not be persisted.
- Release media subscriptions, native hooks, tray resources, and owned handles on shutdown.
- Report startup failures visibly, since a release Windows GUI executable may have no console.

Do not read, overwrite, or migrate the installed original Nusa's private data as part of this learning project.

## 4. Wider Nusa feature roadmap

The following catalog summarizes additional capabilities advertised on the [publisher's website](https://rama2555.github.io/nusa/), checked on 12 November 2026. These are product descriptions, not verified behavior of our clone:

- Mascot eyes with idle, sleep, and greeting animations.
- Minimal line mode; floating presentation; frosted appearance.
- Clock ring reflecting battery or hydration.
- Multi-session Pomodoro, breaks, reminders, and history.
- Custom bottle size, reminder hours, hydration prompts, and charts.
- Incoming file drops and clipboard history with recopy/drag actions.
- Task editing, due times, reminders, manual ordering, and calendar.
- Volume, brightness, Caps Lock, and keyboard-language indicators.
- Battery/charging, Wi-Fi, and headphone connection indicators.
- Local prayer calculations, city selection, sun arc, reminders, and optional playback pause.
- Two-widget page: week, prayer, water, focus, world clock, battery, moon, or countdown.
- Notification previews and a Today view.
- Startup toggle, mascot toggle, appearance options, and About.
- Store distribution and Pro entitlements.

The publisher places notifications/Today in its Store edition and several appearance, widget, and calendar features in Pro. Those commercial boundaries do not need reproducing in a personal learning app.

**Build order:** finish the six-page baseline first. Then choose one roadmap item at a time. Notification access, brightness support, startup registration, and packaging need their own platform investigation. Do not assume an API will work merely because the publisher offers a feature. Billing, accounts, cloud synchronization, and a Store release are not initial deliverables.

## 5. Technology and setup

Use **Rust + GPUI + Windows APIs** to follow the existing reference. GPUI draws the interface; Win32 supplies window behavior, tray actions, dialogs, and Shell integration; WinRT supplies media sessions. This is a native Windows project.

The original application's investigated graphics stack uses native Windows graphics APIs. Using GPUI here is our own implementation choice.

Start on **Windows x64**, on the display and scale you actually use. Install the Rust MSVC toolchain and Visual Studio Build Tools with the C++ toolchain and Windows SDK if they are not available. Verify the environment before adding application code:

```powershell
rustc --version
cargo --version
rustup show active-toolchain
```

Create your new project in a separate directory:

```powershell
cargo new my-notch --bin
cd my-notch
cargo run
```

The reference uses Rust edition **2024** and pins `gpui = "=0.2.2"`. Keep that exact GPUI version if following its examples. Add only the dependencies needed for the current checkpoint. The other reference versions are:

- `windows = "=0.61.3"`, with API features enabled as needed.
- `windows-core = "=0.61.2"` and `windows-future = "=0.2.1"`.
- `raw-window-handle = "=0.6.2"` for access to the GPUI window's native handle.
- `serde` 1 with derive support and `serde_json` 1 for persistence.

This is a known reference combination, not a claim that these are the newest versions. Copying examples from a different GPUI or Windows bindings release can produce mismatched APIs. Consult documentation or cached crate source for the selected version. Do not copy every Windows feature flag before you need it.

If linking reports missing `link.exe` or Windows SDK libraries, resolve the build environment before diagnosing UI code.

## 6. Implementation checkpoints

For each checkpoint: explain the small task, let me implement it, review it, run the relevant check, and record what works. Later checkpoint descriptions provide context; they are not permission to implement all of them at once.

### Checkpoint 0: a project that runs

**Build:** an ordinary Cargo binary in the new directory. Keep the generated greeting or change its text yourself.

**Learn:** Cargo project layout, `Cargo.toml`, `src/main.rs`, build versus run, debug versus release.

**Done when:** `cargo run` and `cargo build --release` succeed. Identify where each executable was produced. No UI, tray, media, or persistence yet.

### Checkpoint 1: one ordinary GPUI window

**Build:** one visible GPUI window containing a short label. Keep it easy to see and close.

**Learn:** application startup, creating a view, rendering, and the UI event loop. Use pinned-version examples.

**Done when:** you can launch it, see the label, close it, and confirm the process exits. A successful compile alone is insufficient.

### Checkpoint 2: a static clock notch

**Build:** a compact black shape with a clock. Then add borderless positioning, transparency, topmost behavior, and a reliable exit path.

**Learn:** logical versus physical pixels, native `HWND`, window styles, no-activation behavior, and updating time without rendering continuously.

**Done when:** the notch is centered at the top, has the intended silhouette, leaves your editor focused, and lets clicks outside its visible area reach another app. Keep a close/debug escape route while developing.

### Checkpoint 3: expansion and page shell

**Build:** explicit compact/expanded state, hover handling, delayed collapse, pinning, then an animated transition. Add arrows and dots using placeholder page content.

**Learn:** state transitions, event handling, hit testing, elapsed frame time, and stopping an animation once it settles.

**Done when:** rapid enter/leave does not flicker or get stuck; hovering the shadow does not expand it; pinning works; all page targets resize correctly. Start with immediate resizing before introducing a spring.

### Checkpoint 4: tray and application lifecycle

**Build:** tray visibility and exit actions, then single-instance handling and optional preview/quit commands.

**Learn:** a hidden native message receiver if needed, UI/native event delivery, handle ownership, and orderly shutdown.

**Done when:** hide/show works, a second launch does not create another overlay, and Exit removes the icon and process. Test `--quit` separately; a successful command return does not prove the existing app closed.

### Checkpoint 5: the first useful music version

**Build:** discover one Windows media session and display its real metadata. Add playback controls, timeline/seek, artwork, and finally multiple-player selection.

**Learn:** WinRT asynchronous work, session capabilities, subscriptions, and sending results safely back to the UI.

**Done when:** no-player, one-player, track-change, player-close, unsupported-control, and two-player cases behave sensibly. Try real playback and seek actions. Show failures without crashing.

This completes the first substantial product milestone. You can stop here and polish the music notch before building utilities.

### Checkpoint 6: Focus and saved state

**Build:** the timer as plain state logic, then connect its controls to the UI. Introduce local JSON persistence for the chosen duration.

**Learn:** `Instant`/elapsed time, pause/resume state, serialization, defaults, replacement saves, and visible errors.

**Done when:** pausing preserves the remaining duration, a delayed update does not slow the timer, completion happens once, and restart restores the chosen duration. Define suspend/resume behavior explicitly and test it on this machine.

### Checkpoint 7: Tasks and Water

**Build:** one list of tasks with input/completion/deletion; then daily water logging and a goal. Extend saved state only as each feature arrives.

**Learn:** owned text input, list updates, input limits, calendar dates versus elapsed time, and separating saved data from view state.

**Done when:** empty tasks are rejected, Enter/Escape work, edits to state survive restart, undo cannot create negative water, and changing the supplied test date resets the daily amount exactly once.

### Checkpoint 8: Shelf

**Build:** the file picker, a persistent path list, open/remove actions, then outgoing Shell drag.

**Learn:** `PathBuf`, native dialogs, missing-file handling, COM/OLE drag objects, and reference-versus-file ownership.

**Done when:** canceling is harmless, duplicates are rejected, removal leaves source files untouched, missing paths show an error, and a real receiving app accepts a drag. Use a disposable fixture file for integration checks.

### Checkpoint 9: Lyrics

**Build:** an independently testable LRC parser, request/result states, a worker request, then synchronized display and line seeking.

**Learn:** URL encoding, HTTP/JSON handling, timestamp parsing, request identity, and playback-position estimates.

**Done when:** multiple tags and offsets parse correctly; offline/no-result cases are clear; changing tracks during a request cannot attach old lyrics to a new song; paused playback stops lyric progress; a supported player accepts line seeking.

Do not block playback commands behind a long lyrics request. Choose cancellation or separate work scheduling deliberately.

### Checkpoint 10: display behavior and release

**Build:** full-screen suppression, display/DPI repositioning, cleanup, and release packaging after the baseline works.

**Learn:** foreground events, monitor bounds, scale changes, release diagnostics, and controlled performance measurement.

**Done when:** tested full-screen apps hide/show the notch correctly, monitor or scale changes preserve placement and hit regions, and a portable release launches and exits outside the IDE. Record which display setups were actually tested.

Measure settled compact, expanded idle, playing media, and active animation separately. Record CPU, memory, and executable size with the build and conditions. Small executable size and low runtime memory are separate goals; neither proves the other.

## 7. Keep the structure small

Start with `main.rs`. Split code when the current feature makes the responsibility clear. A useful eventual arrangement is:

```text
src/
  main.rs       startup and application lifetime
  ui.rs         GPUI views and user actions
  notch.rs      shared shape, dimensions, and animation math
  platform.rs   native window, tray, dialogs, foreground/display events
  media.rs      Windows sessions, commands, and snapshots
  state.rs      page state, timer, tasks, water, saved data
  shelf.rs      native outgoing file drag
  lyrics.rs     optional later split for parsing and fetching
```

This is a possible destination, not a set of empty files to create on day one. In the reference checkout, UI code is named `gpui_ui.rs`, icons are in `icons.rs`, and lyrics parsing/fetching currently lives across `state.rs` and `media.rs`.

Keep a simple flow: user or Windows event → state change → view update. A media worker should deliver a snapshot or event to the UI thread; it should not directly manipulate GPUI view state. Keep HTTP and slow file work outside UI rendering. Manage COM initialization and resource lifetimes on the threads that own them.

Share the notch geometry between rendering, clipping, and pointer tests. Separate copies drift apart and create invisible interactive areas. Native resize messages can arrive synchronously, so avoid resizing in a way that re-enters a borrowed UI update. The reference schedules native resizing outside that update.

Prefer notifications from Windows where available, and schedule only the clock, countdown, progress, or animation updates currently required. The reference still uses periodic UI checks; it is not proof of a fully event-driven or zero-CPU design.

## 8. Verification and known reference limits

Run checks appropriate to the current code:

```powershell
cargo fmt --check
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

Pure logic worth testing includes timer transitions, LRC parsing, water rollover, page wrapping, persistence round trips, shape hit testing, and spring settling at different frame rates. Use temporary files for state tests. No test should modify original Nusa data or delete real shelf files.

Native behavior needs real interaction checks as well: focus, hover, transparent hit regions, tray cleanup, playback commands, dialogs, drag destinations, full-screen suppression, and display scaling. A passing unit test or screenshot does not verify all of these.

The current reference's recorded GPUI validation covered visible layouts for all six pages, hover expansion/collapse, page navigation, pinning, and Focus interactions. Player-specific playback, artwork, seeking, lyrics retrieval, endpoint mute, external drag targets, full-screen behavior, and other displays/scales remained unverified in that redesign record. Incoming file drop is absent. Do not describe these as completed integration checks.

There was also an earlier reported release termination with `0xc0000409`; the investigation did not establish its cause. If it recurs, capture the exact build, launch command, reproduction steps, and diagnostic evidence before claiming a fix.

For orientation, the reference portable executable measured **4,654,592 bytes (4.44 MiB)** on 12 November 2026. Treat this as one build's size, not a requirement for the learner project. The much smaller binary recorded in the older Direct2D validation belongs to a different implementation.

Use these release settings only after behavior and diagnostics work:

```toml
[profile.release]
opt-level = "s"
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

They match the reference's size-oriented release profile. Keep useful diagnostics during development; an aborting release panic may terminate the process without the explanation visible in a development console.

## 9. Context to carry between sessions

At the end of a checkpoint, record:

- Current checkpoint and the next small task.
- What I implemented and can explain.
- Dependency versions and commands that succeeded.
- What was verified in a running app, on which display/scale/player.
- Known failures, untested integrations, and deliberate scope decisions.

For example: “Checkpoint 2: the clock renders and stays topmost at 100% scale. Clicking the transparent corner still blocks the app underneath. Next: fix the native hit region before adding expansion.”

That gives a new harness a concrete place to continue. If only this document was copied, it describes a plan, not evidence that the new project has completed any checkpoint.

## 10. Reference material

Local references are relative to the existing checkout:

- [README](README.md): current clone behavior and omissions.
- [Cargo.toml](Cargo.toml): exact reference dependencies and release settings.
- [GPUI redesign and verification](analysis/gpui-ui-rewrite.md): current geometry, design, and integration limits.
- [Original Nusa investigation](analysis/findings-and-proposal.md) and [follow-up](analysis/followup-findings.md): evidence and uncertainty about the installed original application.
- [Media and lyrics analysis](analysis/media-lyrics-followup.md): original-app selection and parsing evidence.
- [Hover and rendering analysis](analysis/idle-hover-followup.md): original-app transition and scheduling evidence.
- [Keyboard and full-screen analysis](analysis/keyboard-followup.md): original-app input and suppression evidence.
- [Storage investigation](analysis/storage-followup.md): original storage observations; these are not the new app's schema.
- [Publisher site](https://rama2555.github.io/nusa/): advertised product scope; not proof of implementation or performance.
- [Windows media-session manager](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessionmanager): primary platform API reference.
- [Windows extended styles](https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles): native overlay window options.

If the old checkout is unavailable, the product requirements and checkpoints above remain usable. Ask for an available screenshot only when a visual detail cannot be established from the written specification. Use this document as the initial specification and inspect the new project's actual files before giving code-specific advice.

## 11. Starter prompt for a new harness

```text
Read guidance.md in this project.

I am building my own Nusa-style Windows notch companion from scratch in Rust
with GPUI pinned to =0.2.2 and Windows APIs. I want to implement it myself.

Act as my tutor, reviewer, and verifier. Start by inspecting what exists and
identifying my current checkpoint. If this is a new Cargo project, begin at
Checkpoint 0. Give me one small task with its purpose and acceptance checks.
Let me write the implementation, then review my attempt before moving on.
Only implement application code when I explicitly ask you to do so.

The first substantial goal is a music notch with reliable native window
behavior. The six-page rebuild follows. The larger Nusa roadmap comes later.
Distinguish intended behavior, code that exists, and behavior actually tested.
```
