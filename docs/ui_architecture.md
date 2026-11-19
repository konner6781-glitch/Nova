# Nova UI architecture

## Backend choice

Nova's first desktop renderer is Wry over Tao, with WebView2 on Windows. Wry provides the embedded webview and its IPC hook; Tao owns native windows and the platform event loop. HTML, CSS, and JavaScript are embedded in Nova, so an application does not need Node.js or npm. The UI model, JSON protocol, argument checks, and event routing live in `src/ui.rs`; the Windows host is isolated in `src/ui/backend.rs` and can be replaced without changing Nova syntax.

The Windows WebView2 Evergreen Runtime is an external runtime prerequisite. Nova does not download or install it. Window creation reports a runtime or host initialization error with a link to Microsoft's installer instructions. The runtime is included with Windows 11 but can be absent on older or managed installations, so Nova checks host startup rather than assuming it exists.

## Execution and event flow

`get UI` exposes native functions through the existing module registry. A VM-owned `UiRuntime` stores windows, component trees, handles, current component values, and callback registrations. `UI.show(window)` sends a snapshot to the renderer and returns; the Nova program can continue creating windows or making updates. When the program reaches `stop`, the VM waits for events while any shown window remains open.

The Tao loop runs on a dedicated Windows thread. It owns all Tao windows and Wry webviews. The embedded frontend reports that its event handler is ready before the host sends the initial model, avoiding a page-load race. The VM sends typed commands through Tao's event-loop proxy; the webview sends JSON IPC messages to a channel consumed by the VM. It never sends a Nova function name. The VM validates a message's window, component, event kind, and value against its own model, finds the callback registered by Nova code, then schedules that function in the same VM using the normal call-frame and local-variable machinery. No second interpreter or frontend-triggered function lookup is used.

Nova functions do not close over globals. UI callbacks therefore take event data plus any explicitly bound values from the component options. For example, a button can bind a component handle with `on_click_args: [status]`; the callback receives that handle as its parameter and can call `UI.set_text` on it.

## Component tree and bridge

Each window has an independent component tree and stable monotonically increasing IDs. `UI.column`, `UI.row`, and `UI.container` create parent components; ordinary components can be added to a window or another container. The renderer uses flexbox for these layouts. Absolute positioning is opt-in through validated style fields.

Commands and events use JSON objects with a versioned kind, window ID, component ID, and typed properties. Nova strings are serialized with `serde_json`; the frontend assigns text with `textContent`, never `innerHTML`. Incoming frontend messages are treated as untrusted and ignored if malformed or inconsistent with the Rust model. The host allows only the exact embedded-document data URL generated for the window and blocks later navigation. Local images are explicitly read by `UI.image`, bounded to 1 MiB, checked against supported file signatures, and embedded as data URLs; the webview receives no general filesystem API and does not fetch remote content.

Nova values crossing the bridge map as JSON scalars, arrays, or objects. Nova dictionaries with non-string keys use an explicit tagged entries representation. Functions and UI handles are not serialized into the webview. UI handles remain typed Nova values and invalid or removed handles produce a runtime error.

## Current target and limits

The first host target is Windows with a working WebView2 Evergreen Runtime. Other operating systems have no native host in this version. Wry's HTML-string mode has a 2 MiB limit on Windows; Nova's bundled document is kept small. Local images are sent as data URLs in the initial JSON model and limited to 1 MiB each. The public model and event contract are platform-neutral for future hosts.

The implementation is tested at the model, bridge, callback, and VM levels without opening a desktop window. A real Windows window launch remains a separate runtime check because it requires an interactive desktop session and WebView2 runtime.
