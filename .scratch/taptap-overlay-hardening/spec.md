Status: ready-for-agent

# TapTap cursor overlay hardening

## Problem Statement

As a TapTap user, I hide my system cursor and get an animated hand at my pointer — but the overlay can strand me. Turning on Presentation Mode intentionally swallows every mouse click across the desktop, which is the behaviour I want; because of that the tray menu cannot be used while it is active, so keyboard hotkeys are the only way back and must remain reliable. If the drawing surface cannot be created at startup, the app panics, restores my cursor but leaves the mouse hook installed and a dead overlay on screen, with no error and no way to recover from the UI. When I quit, the render thread is not joined, so cleanup can be cut off mid-frame, and the hook and resources are only released on the happy path. A panic in the tray setup can skip hook removal entirely. Meanwhile the same commands — toggle active, toggle presentation, change size, quit — are implemented twice, once for hotkeys and once for the menu, so the menu checkmarks and the real state can drift apart. For a maintainer, the hand artwork and paint setup are duplicated, and physics and raster drawing live in the same module, so unrelated changes keep colliding.

## Solution

From the user's perspective: the overlay always leaves a usable escape path — during Presentation Mode mouse input is deliberately suppressed system-wide, and the keyboard hotkeys are the reliable way to leave that state or quit. Any startup or runtime failure is reported and leads to a clean shutdown that restores the system cursor, removes the mouse hook, releases graphics resources, and stops and joins the render thread. Hotkeys and menu items drive one command path, so the UI always reflects real state. From the maintainer's perspective: OS interaction sits behind one seam that can be faked in tests, command handling is defined once, artwork and drawing helpers have a single source of truth, and physics and rendering change independently.

## User Stories

1. As a desktop user, I want the overlay to hide the system cursor and draw an animated hand at my pointer, so that I get the TapTap experience.
2. As a desktop user, I want my clicks to reach the application underneath the overlay during normal operation, so that I can work normally.
3. As a presenter, I want Presentation Mode to suppress mouse clicks across the desktop, so that no click is delivered to applications while I am presenting.
4. As a presenter, I want to leave Presentation Mode using its keyboard hotkey, so that I can recover even though mouse input is suppressed.
5. As a keyboard-only user, I want a hotkey to toggle Presentation Mode, so that I can control it without a mouse.
6. As a user, I want to toggle the overlay Active/Inactive from both the hotkey and the menu, so that I can pause effects without quitting.
7. As a user, I want the system cursor restored whenever the overlay becomes inactive, so that I can see and use my pointer.
8. As a user, I want the system cursor restored when I quit, so that I am never left without a pointer.
9. As a user, I want the cursors restored if the app panics, so that a crash does not leave my desktop unusable.
10. As a user, I want the cursors restored on console Ctrl+C and window close, so that I never have to reboot to get my pointer back.
11. As a user starting a second instance, I want it to exit without touching the first instance's state, so that the single-instance guarantee holds.
12. As a user, I want a failure to create the drawing surface to shut the app down cleanly and restore my cursor, so that I do not get a dead overlay.
13. As a user, I want any render-thread failure to be reported and to trigger a clean shutdown, so that the failure is diagnosable.
14. As a user, I want the mouse hook removed when the app exits for any reason, so that no stale hook keeps observing my input.
15. As a user, I want device contexts, bitmaps, and cursor handles released on every exit path, so that resources are not leaked.
16. As a user, I want the render thread stopped and joined before the app finishes cleanup, so that teardown is never cut short.
17. As a user, I want the tray menu to reflect real state after a hotkey change, so that the UI is not lying to me.
18. As a user, I want scale up/down from the hotkey and from the menu to behave identically, so that my chosen size is consistent.
19. As a user, I want Quit from the hotkey and from the menu to behave identically, so that I always exit cleanly.
20. As a user, I want preset sizes with the correct one checked, so that I can pick a size directly.
21. As a user, I want size steps clamped to a safe range, so that extreme sizes do not break rendering.
22. As a user, I want per-monitor DPI scaling applied, so that the hand looks right on mixed-DPI displays.
23. As a user, I want to mute the audio from the tray, so that I can use the overlay silently.
24. As a user, I want rapid clicks to escalate the animation and pitch, so that repeated pokes feel satisfying.
25. As a user, I want the app to keep working when no audio output device exists, so that the visual overlay is still usable.
26. As a user, I want conflicting hotkey registrations to be surfaced, so that I know why a shortcut does not work.
27. As a user, I want the message loop to handle error results explicitly, so that an error does not turn into a busy loop.
28. As a user, I want the overlay window to stay topmost and out of the taskbar, so that it never obstructs or disappears.
29. As a user, I want the hand hotspot aligned with the real pointer, so that the hand points where I click.
30. As a user, I want no overlay visible when inactive, so that my desktop is unaffected.
31. As a user, I want to quit reliably via the keyboard hotkey while Presentation Mode is on, so that I am never stuck when mouse input is suppressed.
32. As a new user, I want the app to work with no configuration, so that I can run it immediately.
33. As a developer, I want one command dispatch path for hotkeys and menu events, so that behavior is defined exactly once.
34. As a developer, I want a single platform seam, so that I can test behavior without a real desktop.
35. As a tester, I want to inject platform failures, so that I can verify the failure paths.
36. As a tester, I want to assert on observable commands and state, so that tests do not depend on implementation details.
37. As a maintainer, I want the hand artwork defined once, so that the tray icon and the overlay hand cannot drift apart.
38. As a maintainer, I want paint and stroke setup shared, so that adding an effect does not duplicate boilerplate.
39. As a maintainer, I want physics separated from raster drawing, so that each module changes for one reason.
40. As a maintainer, I want internal spring state private, so that callers cannot corrupt it.
41. As a maintainer, I want cursor lifecycle centralized, so that failsafes and normal exit share one restore path.
42. As a maintainer, I want resource ownership explicit, so that panics do not leak OS handles.
43. As a maintainer, I want clear module boundaries, so that I can make changes safely.
44. As a support engineer, I want startup and runtime failures to be diagnosable, so that I can help users.
45. As a contributor, I want the test seam documented, so that I can add behavior tests confidently.

## Implementation Decisions

- Add a **platform boundary module** as the single seam: one interface covering window creation/lifecycle, message-loop events, hotkey and tray event delivery, low-level mouse hook installation/removal, system-cursor hide/restore, graphics surface allocation/release, and audio output. The application talks only to this interface; real Win32 is one implementation, tests use a fake.
- Add a **command dispatch module** defining one command set — toggle active, toggle presentation, toggle mute, scale up, scale down, set a scale preset, quit — and one handler that owns every state transition. Both hotkey events and menu events translate into commands and call that same handler.
- Keep a single **command entry point** through which injected events drive the app; tests drive behavior by feeding the fake platform events and reading the commands it records.
- **Presentation Mode** deliberately suppresses mouse button input system-wide by reporting those events as handled from the low-level mouse hook; this is intended behaviour and must be preserved. Because mouse input cannot reach the tray while it is active, the keyboard hotkeys are the supported way to toggle Presentation Mode or quit, and they must keep working in that state. Outside Presentation Mode the overlay remains click-transparent.
- **Startup failures** in the render path propagate to the UI thread as an event instead of panicking across threads; the UI thread records the failure, sets shutdown state, removes the hook, restores cursors, and posts a quit.
- Put OS handles behind **RAII guards** so the hook, device contexts, bitmap, and cursor handles are released on every exit path, including panics and early returns.
- **Centralize cursor restore** into one operation shared by the normal exit path, the panic hook, and the console control handler; the single-instance early-exit path must not disturb an already-running instance.
- **Join the render thread** before releasing resources, so teardown cannot be cut short by process exit.
- Handle the message-loop error result **explicitly** (distinguishing failure from a normal quit) rather than treating it as a successful message.
- Surface **hotkey registration failures**instead of silently ignoring them, so a conflicting shortcut is visible.
- Establish one source of truth for the **hand artwork**, shared between build-time icon generation and runtime rendering, so the two representations cannot diverge.
- Extract shared **paint/stroke helpers** for effect drawing, removing the repeated outline-and-fill setup.
- Split **spring physics** from **raster drawing** into separate responsibilities so each changes for one reason.
- Make spring state and tuning fields **private**; only values genuinely needed by callers remain public.
- No schema, database, or external API changes: this is a local desktop application.
- No new domain glossary or ADRs are introduced; none currently exist and no new domain concept requires one.

## Testing Decisions

- A good test observes only external behavior through the fake platform: it feeds an event (hotkey, menu selection, mouse event, simulated failure) and asserts on the resulting commands and state (cursor hidden/restored, window shown/hidden, scale changed, audio played or muted, hook installed/removed, quit issued). It never asserts on pixel bytes, private spring values, or the internals of the drawing math.
- Modules under test through the single seam: command dispatch and state transitions; cursor lifecycle across normal exit, panic, control-event, and active-toggle paths; hook and resource cleanup; render-thread startup failure handling; scale steps and presets including clamping; Presentation Mode click suppression (suppressed while active, not suppressed when inactive, hotkey-driven exit still works while active); mute and click-heat behavior at the audio sink boundary; single-instance early exit.
- Failure simulation in the fake platform: drawing-surface allocation failure, hotkey registration conflict, message-loop error result, panic during startup, and absent audio device.
- Prior art: none. The repository currently has no automated tests and no existing seam; the platform boundary described above is a new seam introduced specifically to make these behaviors testable.
- Explicitly not tested: exact animation output, waveform shape, pixel-perfect rendering, and the behavior of the real Win32 APIs (those are exercised by manual smoke testing).

## Out of Scope

- New visual features, new gestures, new sound designs, new themes, or changes to the existing hand/impact-effect appearance.
- Changing the global mouse-click suppression semantics of Presentation Mode; that behaviour is intentional.
- New hotkeys or menu items beyond the current set.
- Cross-platform support; the overlay remains Windows-only.
- Performance or binary-size optimisation, including changes to the release profile.
- A full pass on the remaining low-value code smells (primitive wrappers for scale/coordinates, bundling hotspot coordinate clumps), which are judged not worth the churn here.
- Creating a domain glossary or ADRs as part of this work.
- Adding a third-party test framework or CI pipeline; tests use the project's existing toolchain.
- Re-running the Spec axis of the earlier code review (there was no originating issue or spec).

## Further Notes

- Source material: a two-axis review of the diff against the initial commit found 7 Standards findings (all judgement calls from the Fowler smell baseline, since the repo documented no standards) and 7 Correctness & Safety findings. The highest-impact remaining item is a render-thread startup failure that left a dead overlay with hidden cursors. The review's finding that Presentation Mode swallows every mouse click was confirmed by the user to be intended behaviour and is excluded from this spec.
- The seam decision (one seam at the Win32 platform boundary, fake injected in tests) was explicitly confirmed with the user before writing this spec.
- No repository coding standards, glossary, or ADRs existed at the time of writing, so no documented standard or ADR constrains these decisions; after setup, the triage vocabulary is the five default roles and issue state is recorded as a `Status:` line in tracker files.
- Published to the local markdown tracker with the `ready-for-agent` role; no further triage is required.
