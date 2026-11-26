# TapTap

TapTap replaces the Windows pointer with an animated hand. This glossary names the user-visible concepts the app and its documentation use.

## Language

**Overlay State**:
The current combination of Active, Presentation Mode, Hand Style, Hand Size, Click Sound and Volume that governs how the Overlay behaves.
_Avoid_: AppState, runtime state, settings snapshot

**Overlay**:
A transparent, click-through layer that shows the Hand above other applications.
_Avoid_: Window, canvas, layer

**Hand**:
The cartoon glove that stands in for the System Cursor.
_Avoid_: Glove, pointer, avatar

**System Cursor**:
The pointer provided by Windows that the Overlay replaces.
_Avoid_: Native cursor, OS cursor, real cursor

**Active**:
The state where the Overlay shows the Hand and the System Cursor is hidden.
_Avoid_: Enabled, on, running

**Presentation Mode**:
The state where mouse clicks are suppressed while the Hand keeps animating and playing sounds, so you can point at things without activating them. Applies only while Active.
_Avoid_: Click-blocking mode, pointing mode

**Poke**:
The Hand's animated strike in response to a click.
_Avoid_: Click, strike, hit

**Combo Heat**:
The buildup from rapid Pokes that makes the Hand's animation and sound more intense.
_Avoid_: Heat, combo meter, streak

**Impact**:
The comic-book rays and arcs drawn around the fingertip during a Poke.
_Avoid_: Burst, effect, ray

**Click Sound**:
The sound played when the Hand Pokes.
_Avoid_: Audio preset, sound effect

**Hand Style**:
The appearance variant of the Hand — Classic or Gingerbread.
_Avoid_: Skin, theme, costume

**Gingerbread Hand**:
A cookie-coloured Hand with white icing piping, icing-dot knuckles, and an icing-squiggle cuff.
_Avoid_: Cookie hand, holiday hand

**Hand Size**:
How large the Hand appears, expressed as a percentage of its default size.
_Avoid_: Scale, zoom

**Jingle Bell**:
A bright, two-strike bell sound, available as a Click Sound.
_Avoid_: Chime, sleigh bell, ring

**Volume**:
How loud a Click Sound plays, from silent to full.
_Avoid_: Level, gain
