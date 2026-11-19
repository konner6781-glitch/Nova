# Nova desktop UI

Nova's built-in `UI` module creates desktop windows with HTML rendering hidden
behind Nova calls. Import it with Nova's `get` keyword:

```nova
get UI

window = UI.window("Welcome", 760, 500)
UI.text(window, "Hello from Nova")
UI.show(window)
stop
```

`UI.show` displays the window and returns to the program. When the program
reaches `stop`, Nova keeps the VM alive while shown windows remain open. Close
all windows to finish the program. A UI program requires Windows and the
Microsoft Edge WebView2 Evergreen Runtime. If window or webview startup fails,
Nova reports the error and points to Microsoft's runtime download. This first
host does not support other operating systems.

## Windows and themes

| Function | Signature | Behavior |
|---|---|---|
| `UI.window` | `UI.window(title, width, height[, options])` | Creates a resizable window handle. Dimensions are whole numbers from 120 to 7680 pixels. |
| `UI.show` | `UI.show(window)` | Opens a window. Calling it again on an already open window does nothing. |
| `UI.close` | `UI.close(window)` | Closes an open window. |
| `UI.theme` | `UI.theme(window, "light" or "dark")` | Changes the window's current theme. |

Window options are a dictionary with `min_width`, `min_height`, `max_width`,
`max_height`, `resizable`, `background`, `theme`, and `on_close`. Minimum and
maximum dimensions must include the initial size. Each width and height limit
is independent, so a program can set only one dimension. `background` is a
color name from the built-in palette or a 3, 4, 6, or 8 digit hex color. `on_close` names
a top-level Nova function that takes no arguments. Multiple windows are
supported and have independent component trees.

## Components and layout

Every component call returns a stable component handle. Its first argument is
the containing window or a layout container.

| Function | Signature | Behavior |
|---|---|---|
| `UI.text` | `UI.text(parent, text[, style])` | Adds readable text. |
| `UI.button` | `UI.button(parent, label[, options])` | Adds a clickable button. |
| `UI.input` | `UI.input(parent[, initial_value[, options]])` | Adds a text input with optional placeholder. |
| `UI.dropdown` | `UI.dropdown(parent, options[, selected[, settings]])` | Adds a single-selection dropdown. The options list must not be empty. |
| `UI.checkbox` | `UI.checkbox(parent, label[, checked[, options]])` | Adds a checkbox, unchecked by default. |
| `UI.slider` | `UI.slider(parent, min, max, value[, options])` | Adds a numeric slider. Requires `min < max` and an initial value in range. |
| `UI.image` | `UI.image(parent, local_path[, style])` | Adds a local PNG, JPEG, GIF, or WebP image. Files are limited to 1 MiB, checked against their file signature, and embedded into the view. |
| `UI.column` | `UI.column(parent[, style])` | Adds a vertical flex layout. |
| `UI.row` | `UI.row(parent[, style])` | Adds a horizontal, wrapping flex layout. |
| `UI.container` | `UI.container(parent[, style])` | Adds a padded panel that can contain more components. |

Use columns and rows for normal responsive layout instead of calculating
coordinates. Components can be nested in containers. Set `position` to
`"absolute"` with numeric `left` and `top` only for advanced positioning.

The default theme uses system typography, rounded controls, flexible spacing,
hover and focus states, and light or dark colors. Styles are ordinary Nova
dictionaries. Available keys are `background`, `color`, `border_color`,
`width`, `height`, `padding`, `margin`, `gap`, `border_radius`, `font_size`,
`font_weight`, `align`, `text_align`, `opacity`, `position`, `left`, `top`,
`layout`, `visible`, and `enabled`. Numeric dimensions use pixels. Width and
height currently take numbers; opacity is from 0 to 1. Colors are restricted
to named colors or hex values, and unknown style keys are errors.

```nova
get UI

window = UI.window("Profile", 680, 420, {"theme": "dark"})
column = UI.column(window, {"gap": 14})
UI.text(column, "Create your profile", {"font_size": 24, "font_weight": "bold"})
name = UI.input(column, "", {"placeholder": "Your name"})
UI.dropdown(column, ["Personal", "Work"], "Personal")
UI.checkbox(column, "Remember this choice", true)
UI.show(window)
stop
```

## Events and callbacks

Callbacks name a top-level Nova function. Nova validates the callback name and
argument count when the component is created. Browser messages contain only a
window ID, component ID, event kind, and typed value; they cannot select or
invoke arbitrary Nova functions.

Button `on_click` callbacks receive the values in `on_click_args`. Change
callbacks (`on_change`) receive the new control value first, then any values in
`on_change_args`. A close callback takes no arguments. Nova functions have
local scopes and do not capture globals, so import `UI` inside a callback that
uses it and pass handles or other needed state explicitly in its argument
lists.

```nova
get UI

function change_name(value, output):
    get UI
    UI.set_text(output, "Name: " + value)

function clear_name(input, output):
    get UI
    UI.set_value(input, "")
    UI.set_text(output, "Name cleared")

window = UI.window("Live form", 620, 360)
column = UI.column(window, {"gap": 12})
output = UI.text(column, "Waiting for input")
input = UI.input(column, "", {
    "placeholder": "Type here",
    "on_change": "change_name",
    "on_change_args": [output]
})
button = UI.button(column, "Clear", {
    "on_click": "clear_name",
    "on_click_args": [input, output]
})
UI.show(window)
stop
```

Supported events are button clicks and input, dropdown, checkbox, and slider
changes. The VM dispatches callbacks through its normal Nova call frames, on
the VM-owning thread, after the source program reaches `stop`.

## Reading values and live updates

`UI.value(component)` reads the latest state of an input, dropdown, checkbox, or
slider. In a change callback, use the event value parameter for the new value.
Updates target a single component:

| Function | Behavior |
|---|---|
| `UI.set_text(component, text)` / `UI.set_label(component, text)` | Changes the text, button label, or checkbox label. |
| `UI.set_value(component, value)` | Changes an input, dropdown selection, checkbox, or slider value after validating its type and range. |
| `UI.set_visible(component, bool)` | Shows or hides a component. |
| `UI.set_enabled(component, bool)` | Enables or disables a control. |
| `UI.set_style(component, dictionary)` | Applies validated style properties. |
| `UI.set_options(dropdown, list)` | Replaces dropdown choices and retains the selection if still present, otherwise selects the first option. |
| `UI.remove(component)` | Removes a component and its descendants. Its handle is invalid afterward. |

Reading or updating an unsupported component type, using a removed handle,
using a handle whose window has closed, or passing an invalid style or value
raises a Nova runtime error. Image paths are resolved from the process's
current working directory; use an absolute path or run Nova from the intended
project directory.

## Runtime and bridge notes

The Rust VM stores the component model; an embedded HTML/CSS/JavaScript view
renders it. Wry and Tao are used for the Windows native window and WebView2
host. The UI frontend gets no general filesystem or network API. `UI.image`
reads only the explicit local path supplied by Nova and accepts the four
documented raster formats. Strings are sent through structured JSON, rendered
as text nodes, and never inserted as HTML. Malformed or unknown frontend event
messages are ignored. Nova values that cannot safely be represented by the
bridge, including functions and UI handles, cannot be serialized.

The WebView2 runtime is not bundled with Nova. See [Microsoft's WebView2
deployment guidance](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution)
for current install and deployment options.

Runnable programs are in `examples/ui_*.nova`. Run one with `cargo run --
examples/ui_hello.nova` from the repository root.
