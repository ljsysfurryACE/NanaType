## App
app-name = NanaType
app-tagline = Typewriter for VTubers
history-count = { $count ->
    [one] { $count } line
   *[other] { $count } lines
}

## Window titles
window-input = Input
window-display = Dialogue
window-history = History
window-settings = Settings

## Tray
tray-show-input = Input
tray-show-display = Dialogue
tray-show-history = History
tray-lock-overlays = Lock overlays
tray-mode = Input mode
tray-skin = Skin
tray-settings = Settings…
tray-obs-help = OBS setup…
tray-quit = Quit NanaType

## Input mode
mode-compose = Compose & send
mode-compose-hint = Write the line, then send it to play with the typewriter
mode-mirror = Live mirror
mode-mirror-hint = The dialogue follows what you type

## Toolbar
tb-bold = Bold
tb-italic = Italic
tb-underline = Underline
tb-strike = Strikethrough
tb-color = Text color
tb-color-reset = Default color
tb-color-recent = Recent
tb-size = Size
tb-size-reset = Default size
tb-stroke = Outline
tb-shadow = Shadow
tb-stroke-none = Remove outline
tb-shadow-none = Remove shadow
tb-stroke-width = Outline width
tb-shadow-blur = Blur
tb-ruby = Ruby
tb-ruby-placeholder = Reading, e.g. かんじ
tb-ruby-apply = Add ruby
tb-ruby-remove = Remove ruby
tb-effects = Effects
tb-effect-shake = Shake
tb-effect-wave = Wave
tb-effect-jump = Jump
tb-effect-rainbow = Rainbow
tb-effect-strength = Strength
tb-effect-speed = Speed
tb-entrance = Entrance
tb-entrance-skin = Skin default
tb-entrance-none = None
tb-entrance-fade = Fade
tb-entrance-pop = Pop
tb-entrance-slide = Slide up
tb-pacing = Pacing
tb-wait = Pause
tb-wait-ms = Pause { $ms } ms
tb-wait-custom = Custom pause…
tb-speed-slow = Slower
tb-speed-fast = Faster
tb-speed-instant = Show instantly
tb-speed-reset = Normal speed
tb-pause = Wait for continue
tb-pause-auto = Pause, then continue
tb-page = New page
tb-insert = Insert
tb-sticker = Sticker
tb-sticker-search = Search stickers
tb-sticker-empty = No sticker folders yet. Add one in Settings › Assets
tb-image = Image…
tb-sound = Sound effect
tb-expression = Expression
tb-expression-placeholder = Expression name, e.g. smile
tb-portrait = Portrait variant
tb-screenshake = Screen shake
tb-phrases = Quick phrases
tb-phrases-empty = No quick phrases yet. Add them in Settings › Assets
tb-source = Markup source
tb-preview-effects = Preview effects
tb-send = Send
tb-send-hint = { $key } to send
tb-clear-format = Clear formatting
tb-more = More
tb-undo = Undo
tb-redo = Redo

## Editor
editor-placeholder = Type your line here…
editor-source-placeholder = Edit with [b]markup[/b], e.g. [shake]Whoa![/shake][wait=500]
editor-diagnostic = Character { $pos }: { $message }
marker-wait = wait { $ms }ms
marker-pause = continue
marker-pause-auto = wait { $ms }ms
marker-page = page
marker-sound = sound { $name }
marker-expression = expr { $name }
marker-portrait = portrait { $name }
marker-shake = shake
status-queued = Queued (#{ $position })
status-mirror = Mirroring
status-sent = Sent

## Settings navigation
settings-general = General
settings-playback = Playback
settings-skins = Skins
settings-windows = Windows
settings-history = History
settings-hotkeys = Hotkeys
settings-output = Output / OBS
settings-integrations = Integrations
settings-assets = Assets
settings-about = About

## General
general-language = Language
general-language-system = System default
general-speaker = Character name
general-speaker-hint = Shown on the name plate and in history. Leave empty to hide the plate
general-avatar = Avatar
general-avatar-pick = Choose image…
general-avatar-clear = Remove
general-presence = Input window
general-presence-resident = Always open
general-presence-summon = Summon with hotkey
general-presence-hint = In summon mode the input hides after sending or Esc
general-mode = Input mode
general-send-key = Send key
general-send-enter = Enter sends, Shift+Enter breaks the line
general-send-ctrl-enter = Ctrl+Enter sends, Enter breaks the line
general-autostart = Launch at login
general-updates = Check for updates

## Playback
playback-speed = Typing speed
playback-speed-value = { $value }×
playback-typing = Typewriter effect
playback-typing-hint = When off, each line appears at once
playback-punct = Punctuation pauses
playback-punct-hint = Pause briefly after commas, stops and ellipses
playback-entrance = Character entrance
playback-after = After the line
playback-after-hold = Keep showing
playback-after-fade = Fade out
playback-after-clear = Clear
playback-after-delay = Stay for
playback-after-delay-value = { $seconds } s
playback-auto = Advance automatically
playback-auto-off = Off
playback-auto-value = After { $seconds } s
playback-queue = Sending while a line plays
playback-queue-replace = Replace immediately
playback-queue-finish = Finish the current line first
playback-queue-queue = Queue until continue
playback-reduced = Reduce motion
playback-reduced-hint = Turns off per-character motion and screen shake
playback-stagger = Mirror character delay
playback-sound = Typing sounds
playback-volume = Master volume
playback-volume-typing = Typing volume
playback-volume-effects = Effects volume
playback-device = Output device
playback-device-default = System default

## Skins
skins-active = Current skin
skins-import = Import skin…
skins-export = Export…
skins-duplicate = Duplicate to edit
skins-open-folder = Open folder
skins-delete = Delete
skins-delete-confirm = Delete the skin “{ $name }”? This cannot be undone.
skins-builtin = Built-in
skins-broken = Cannot load
skins-by = by { $author }
skins-version = Version { $version }
skins-overrides = Personalize
skins-override-text = Text color
skins-override-name = Name plate color
skins-override-box = Box color
skins-override-accent = Accent color
skins-override-size = Text size
skins-override-reset = Reset to skin defaults
skins-chrome = Appearance
skins-chrome-skin = Follow skin
skins-chrome-system = Follow system
skins-chrome-light = Light
skins-chrome-dark = Dark
skins-hot-reload = Reload when files change
skins-hot-reload-hint = For skin authors
skins-installed = Installed “{ $name }”
skins-replaced = Updated “{ $name }” from { $previous }
skins-import-failed = The skin could not be imported
skins-fallback = Skin “{ $id }” could not load; using the default skin
skins-preview = Preview
skins-preview-show = Preview this skin
skins-preview-name = Nana
skins-preview-line = Hi everyone! Typing [b]live[/b] again today, with [wave]waves[/wave], [rainbow]rainbows[/rainbow] and a little [shake]shake[/shake].
skins-filter-all = All
skins-tag-light = Light
skins-tag-dark = Dark
skins-tag-classic = Classic
skins-tag-streaming = Streaming
skins-tag-game = Game
skins-tag-retro = Retro
skins-tag-cute = Cute
skins-tag-elegant = Elegant
skins-tag-minimal = Minimal

## Windows
windows-input = Input
windows-display = Dialogue
windows-history = History
windows-visible = Show
windows-lock = Interaction
windows-lock-locked = Locked (click-through)
windows-lock-interactive = Interactive
windows-lock-edit = Edit layout
windows-on-top = Always on top
windows-background = Capture background
windows-background-transparent = Transparent
windows-background-chroma = Chroma key
windows-reset = Reset position
windows-size = Size
windows-size-skin = Follow skin

## History
history-max = Lines kept
history-visible = Lines shown
history-fade = Fade out
history-fade-off = Never
history-fade-value = After { $seconds } s
history-avatar = Show avatar
history-name = Show name
history-time = Show time
history-sync = Appears
history-sync-submit = When sent
history-sync-reveal = Typing along
history-sync-complete = When fully shown
history-clear = Clear history
history-clear-confirm = Clear all history of this session?
history-empty = Lines you send appear here

## Hotkeys
hotkeys-summon = Summon input
hotkeys-continue = Continue
hotkeys-skip = Skip line
hotkeys-clear = Clear dialogue
hotkeys-toggle-mode = Switch input mode
hotkeys-toggle-overlays = Show/hide overlays
hotkeys-record = Record
hotkeys-recording = Press a key combination…
hotkeys-none = Not set
hotkeys-clear-binding = Clear
hotkeys-ok = Registered
hotkeys-conflict-os = Used by another program
hotkeys-conflict-dup = Same as “{ $action }”
hotkeys-unsafe = A single key would block normal typing; add Ctrl, Alt or Shift
hotkeys-local = Input window shortcuts
hotkeys-local-hint = Ctrl+B/I/U for style, Ctrl+↑/↓ to recall sent lines, Esc hides the input

## Output
output-display = Dialogue
output-history = History
output-spout = Spout2 output
output-spout-name = Sender name
output-spout-copy = Copy name
output-spout-size = Output resolution
output-spout-size-window = Match window
output-spout-straight = Straight alpha
output-spout-straight-hint = Turn on if the receiver has no premultiplied alpha mode
output-status-active = Sending
output-status-off = Off
output-status-unavailable = Unavailable: { $reason }
output-spout-unsupported = Spout2 is Windows-only; use window capture
output-sample = Play a sample line
output-sample-line = [b]Hello![/b] This is a NanaType [wave]sample line[/wave].[wait=300][shake]Can you see it?[/shake]
output-obs-title = Set up OBS
output-obs-spout = Install the Spout2 plugin for OBS, add a “Spout2 Capture” source, pick the sender name above, and set the composite mode to “Premultiplied Alpha”.
output-obs-window = You can also use “Window Capture” (method: Windows 10 1903 and up); the dialogue window background is transparent.
output-obs-mac = On macOS use “macOS Screen Capture” and pick the dialogue window.

## Integrations
int-vts = VTube Studio
int-vts-hint = Trigger VTube Studio hotkeys at [expr=name] markers
int-vts-port = Port
int-vts-authorize = Request access
int-vts-authorize-hint = Then allow the NanaType plugin inside VTube Studio
int-vts-map = Expression mapping
int-vts-map-expression = Expression
int-vts-map-hotkey = VTS hotkey
int-vts-map-auto = Match by name
int-vts-refresh = Refresh hotkeys
int-osc = OSC
int-osc-hint = Send OSC messages to VNyan, Warudo, VSeeFace and others
int-osc-host = Host
int-osc-port = Port
int-osc-prefix = Address prefix
int-ws = WebSocket broadcast
int-ws-hint = Broadcast line and expression events (JSON) on a local port
int-ws-port = Port
int-ws-clients = { $count ->
    [one] { $count } client connected
   *[other] { $count } clients connected
}
int-status-off = Off
int-status-connecting = Connecting…
int-status-connected = Connected
int-status-needs-token = Needs access
int-status-error = Error: { $message }

## Assets
assets-stickers = Sticker folders
assets-stickers-hint = Images in these folders become stickers; the folder name is the library name
assets-add-folder = Add folder…
assets-remove = Remove
assets-phrases = Quick phrases
assets-phrase-label = Label
assets-phrase-markup = Content (markup allowed)
assets-phrase-send = Send directly
assets-phrase-add = Add phrase
assets-phrase-hotkey = Hotkey

## About
about-version = Version { $version }
about-build = Build { $build }
about-logs = Open logs folder
about-data = Open data folder
about-diagnostics = Export diagnostics…
about-licenses = Open-source licenses
about-homepage = Project page
about-update-available = Version { $version } is available
about-update-latest = You are up to date

## Onboarding
onboard-welcome = Welcome to NanaType
onboard-welcome-body = A few steps and the typewriter is ready for your stream.
onboard-language = Choose a language
onboard-speaker = Your character name
onboard-skin = Choose a skin
onboard-mode = How you type
onboard-hotkeys = Hotkeys
onboard-hotkeys-body = Press { $summon } to summon the input and { $continue } to continue.
onboard-obs = Connect OBS
onboard-next = Next
onboard-back = Back
onboard-skip = Skip
onboard-done = Get started

## Common
common-ok = OK
common-cancel = Cancel
common-close = Close
common-apply = Apply
common-reset = Reset
common-on = On
common-off = Off
common-browse = Browse…
common-copied = Copied
common-error = Something went wrong
common-ms = { $ms } ms

## Errors and notices
err-settings-quarantined = The settings file was damaged. It was saved to { $path } and defaults are in use
err-settings-newer = These settings come from a newer NanaType; changes made now will not be saved
err-audio = Audio device unavailable: { $message }
err-hotkey = Could not register the hotkey { $hotkey }
err-image = The image could not be read: { $message }
err-spout = Spout output failed: { $message }
