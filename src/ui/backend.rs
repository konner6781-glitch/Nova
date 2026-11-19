use serde_json::Value as JsonValue;
#[cfg(windows)]
use std::sync::mpsc::{self, Receiver, SyncSender};
#[cfg(windows)]
use std::time::Duration;

#[derive(Debug)]
pub(super) enum HostEvent {
    Ipc(String),
    Closed(u64),
    Failed(String),
}

#[cfg(windows)]
mod platform {
    use super::{Duration, HostEvent, JsonValue, Receiver, SyncSender, mpsc};
    use std::collections::HashMap;
    use std::thread;
    use tao::dpi::{LogicalSize, LogicalUnit, PixelUnit};
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
    use tao::platform::windows::EventLoopBuilderExtWindows;
    use tao::window::{Window, WindowBuilder, WindowId, WindowSizeConstraints};
    use wry::WebView;

    pub(super) enum Command {
        Open {
            id: u64,
            title: String,
            width: u32,
            height: u32,
            min_width: Option<u32>,
            min_height: Option<u32>,
            max_width: Option<u32>,
            max_height: Option<u32>,
            resizable: bool,
            initial_message: JsonValue,
            ack: SyncSender<Result<(), String>>,
        },
        FrontendReady(u64),
        FrontendFailed {
            id: u64,
            message: String,
        },
        Send {
            id: u64,
            message: JsonValue,
        },
        Close(u64),
        Shutdown,
    }

    pub struct Host {
        proxy: EventLoopProxy<Command>,
        events: Receiver<HostEvent>,
    }

    struct OpenWindow {
        id: u64,
        _window: Window,
        webview: WebView,
        initial_message: JsonValue,
        ack: Option<SyncSender<Result<(), String>>>,
    }

    impl Host {
        pub fn start() -> Result<Self, String> {
            let (events_tx, events) = mpsc::channel();
            let (proxy_tx, proxy_rx) = mpsc::sync_channel(1);
            thread::Builder::new()
                .name("nova-ui-host".into())
                .spawn(move || {
                    let mut builder = EventLoopBuilder::<Command>::with_user_event();
                    builder.with_any_thread(true);
                    let event_loop = builder.build();
                    let proxy = event_loop.create_proxy();
                    if proxy_tx.send(proxy.clone()).is_err() {
                        return;
                    }
                    let mut windows: HashMap<WindowId, OpenWindow> = HashMap::new();
                    let mut ids: HashMap<u64, WindowId> = HashMap::new();
                    event_loop.run(move |event, target, control_flow| {
                        *control_flow = ControlFlow::Wait;
                        match event {
                            Event::UserEvent(Command::Open {
                                id,
                                title,
                                width,
                                height,
                                min_width,
                                min_height,
                                max_width,
                                max_height,
                                resizable,
                                initial_message,
                                ack,
                            }) => {
                                let logical = |value: Option<u32>| {
                                    value.map(|value| PixelUnit::Logical(LogicalUnit::new(value as f64)))
                                };
                                let constraints = WindowSizeConstraints::new(
                                    logical(min_width),
                                    logical(min_height),
                                    logical(max_width),
                                    logical(max_height),
                                );
                                let builder = WindowBuilder::new()
                                    .with_title(title)
                                    .with_inner_size(LogicalSize::new(width, height))
                                    .with_resizable(resizable)
                                    .with_inner_size_constraints(constraints);
                                let result = (|| {
                                    let window = builder
                                        .build(target)
                                        .map_err(|error| format!("Could not create Nova window: {}.", error))?;
                                    let window_id = window.id();
                                    let ipc_events = events_tx.clone();
                                    let ipc_proxy = proxy.clone();
                                    let html = frontend_document(id);
                                    let embedded_url = format!(
                                        "data:text/html;charset=utf-8;base64,{}",
                                        crate::ui::base64(html.as_bytes())
                                    );
                                    let webview = wry::WebViewBuilder::new()
                                        .with_html(html)
                                        .with_navigation_handler(move |url| url == embedded_url)
                                        .with_ipc_handler(move |request| {
                                            let body = request.body().clone();
                                            let control = serde_json::from_str::<JsonValue>(&body)
                                                .ok()
                                                .filter(|message| {
                                                    message.get("protocol").and_then(JsonValue::as_u64) == Some(1)
                                                        && message.get("window_id").and_then(JsonValue::as_u64) == Some(id)
                                                });
                                            match control.and_then(|message| {
                                                message.get("kind").and_then(JsonValue::as_str).map(str::to_owned)
                                            }) {
                                                Some(kind) if kind == "ready" => {
                                                    let _ = ipc_proxy.send_event(Command::FrontendReady(id));
                                                }
                                                Some(kind) if kind == "init_error" => {
                                                    let message = serde_json::from_str::<JsonValue>(&body)
                                                        .ok()
                                                        .and_then(|value| value.get("message").and_then(JsonValue::as_str).map(str::to_owned))
                                                        .unwrap_or_else(|| "Nova UI frontend initialization failed.".into());
                                                    let _ = ipc_proxy.send_event(Command::FrontendFailed { id, message });
                                                }
                                                _ => {
                                                    let _ = ipc_events.send(HostEvent::Ipc(body));
                                                }
                                            }
                                        })
                                        .build(&window)
                                        .map_err(|error| format!(
                                            "Could not initialize Nova's WebView2 UI host: {}. Install or repair the Microsoft Edge WebView2 Evergreen Runtime, then try again (https://developer.microsoft.com/microsoft-edge/webview2/).",
                                            error
                                        ))?;
                                    Ok::<_, String>((window, window_id, webview))
                                })();
                                match result {
                                    Ok((window, window_id, webview)) => {
                                        ids.insert(id, window_id);
                                        windows.insert(window_id, OpenWindow {
                                            id,
                                            _window: window,
                                            webview,
                                            initial_message,
                                            ack: Some(ack),
                                        });
                                    }
                                    Err(error) => {
                                        let _ = ack.send(Err(error.clone()));
                                        let _ = events_tx.send(HostEvent::Failed(error));
                                    }
                                }
                            }
                            Event::UserEvent(Command::FrontendReady(id)) => {
                                if let Some(window_id) = ids.get(&id).copied() {
                                    if let Some(open) = windows.get_mut(&window_id) {
                                        if open.ack.is_none() {
                                            return;
                                        }
                                        let result = dispatch_script(&open.initial_message).and_then(|script| {
                                            open.webview.evaluate_script(&script).map_err(|error| {
                                                format!("Could not initialize Nova UI components: {}.", error)
                                            })
                                        });
                                        if let Some(ack) = open.ack.take() {
                                            let _ = ack.send(result.clone());
                                        }
                                        if let Err(error) = result {
                                            ids.remove(&id);
                                            windows.remove(&window_id);
                                            let _ = events_tx.send(HostEvent::Failed(error));
                                        }
                                    }
                                }
                            }
                            Event::UserEvent(Command::FrontendFailed { id, message }) => {
                                if let Some(window_id) = ids.remove(&id) {
                                    if let Some(mut open) = windows.remove(&window_id) {
                                        if let Some(ack) = open.ack.take() {
                                            let _ = ack.send(Err(message.clone()));
                                        }
                                    }
                                }
                                let _ = events_tx.send(HostEvent::Failed(format!(
                                    "Nova UI frontend failed to initialize: {}.", message
                                )));
                            }
                            Event::UserEvent(Command::Send { id, message }) => {
                                if let Some(window_id) = ids.get(&id).copied() {
                                    if let Some(window) = windows.get(&window_id) {
                                        if let Ok(script) = dispatch_script(&message) {
                                            if let Err(error) = window.webview.evaluate_script(&script) {
                                                let _ = events_tx.send(HostEvent::Failed(format!("Could not update Nova UI component: {}.", error)));
                                            }
                                        }
                                    }
                                }
                            }
                            Event::UserEvent(Command::Close(id)) => {
                                if let Some(window_id) = ids.remove(&id) {
                                    if let Some(mut open) = windows.remove(&window_id) {
                                        if let Some(ack) = open.ack.take() {
                                            let _ = ack.send(Err("Nova UI window was closed before initialization finished.".into()));
                                        }
                                    }
                                    let _ = events_tx.send(HostEvent::Closed(id));
                                }
                            }
                            Event::UserEvent(Command::Shutdown) => *control_flow = ControlFlow::Exit,
                            Event::WindowEvent { event: WindowEvent::CloseRequested, window_id, .. } => {
                                if let Some(mut open) = windows.remove(&window_id) {
                                    ids.remove(&open.id);
                                    if let Some(ack) = open.ack.take() {
                                        let _ = ack.send(Err("Nova UI window was closed before initialization finished.".into()));
                                    }
                                    let _ = events_tx.send(HostEvent::Closed(open.id));
                                }
                            }
                            _ => {}
                        }
                    });
                })
                .map_err(|error| format!("Could not start Nova's UI host thread: {}.", error))?;
            let proxy = proxy_rx.recv_timeout(Duration::from_secs(10)).map_err(|_| {
                "Could not initialize Nova's Windows UI event loop. Restart the program and check that a desktop session is available.".to_string()
            })?;
            Ok(Self { proxy, events })
        }

        #[allow(clippy::too_many_arguments)]
        pub fn open(
            &mut self,
            id: u64,
            title: String,
            width: u32,
            height: u32,
            min_width: Option<u32>,
            min_height: Option<u32>,
            max_width: Option<u32>,
            max_height: Option<u32>,
            resizable: bool,
            initial_message: JsonValue,
        ) -> Result<(), String> {
            let (ack, response) = mpsc::sync_channel(1);
            self.proxy
                .send_event(Command::Open {
                    id,
                    title,
                    width,
                    height,
                    min_width,
                    min_height,
                    max_width,
                    max_height,
                    resizable,
                    initial_message,
                    ack,
                })
                .map_err(|_| "Nova's UI event loop has stopped unexpectedly.".to_string())?;
            match response.recv_timeout(Duration::from_secs(30)) {
                Ok(result) => result,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let _ = self.proxy.send_event(Command::Close(id));
                    Err("Timed out while creating a Nova UI window. Check that WebView2 is installed and the desktop session is responsive.".into())
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    Err("Nova's UI window initialization acknowledgement was lost.".into())
                }
            }
        }

        pub fn send(&mut self, id: u64, message: JsonValue) -> Result<(), String> {
            self.proxy
                .send_event(Command::Send { id, message })
                .map_err(|_| "Nova's UI event loop has stopped unexpectedly.".into())
        }

        pub fn close(&mut self, id: u64) -> Result<(), String> {
            self.proxy
                .send_event(Command::Close(id))
                .map_err(|_| "Nova's UI event loop has stopped unexpectedly.".into())
        }

        pub fn recv(&mut self) -> Result<HostEvent, String> {
            self.events
                .recv()
                .map_err(|_| "Nova's UI event channel closed unexpectedly.".into())
        }
    }

    impl Drop for Host {
        fn drop(&mut self) {
            let _ = self.proxy.send_event(Command::Shutdown);
        }
    }

    fn frontend_document(window_id: u64) -> String {
        let html = include_str!("frontend/index.html");
        let css = include_str!("frontend/style.css");
        let js = include_str!("frontend/app.js");
        html.replace("__NOVA_WINDOW_ID__", &window_id.to_string())
            .replace("<!-- NOVA_STYLE -->", &format!("<style>{}</style>", css))
            .replace("<!-- NOVA_SCRIPT -->", &format!("<script>{}</script>", js))
    }

    fn dispatch_script(message: &JsonValue) -> Result<String, String> {
        let encoded_message =
            serde_json::to_string(&message.to_string()).map_err(|error| error.to_string())?;
        Ok(format!(
            "window.__NOVA_DISPATCH__&&window.__NOVA_DISPATCH__(JSON.parse({encoded_message}));"
        ))
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{HostEvent, JsonValue};

    pub struct Host;

    impl Host {
        pub fn start() -> Result<Self, String> {
            Err("Nova's desktop UI backend currently supports Windows only.".into())
        }
        pub fn open(
            &mut self,
            _: u64,
            _: String,
            _: u32,
            _: u32,
            _: Option<u32>,
            _: Option<u32>,
            _: Option<u32>,
            _: Option<u32>,
            _: bool,
            _: JsonValue,
        ) -> Result<(), String> {
            Err("Nova's desktop UI backend currently supports Windows only.".into())
        }
        pub fn send(&mut self, _: u64, _: JsonValue) -> Result<(), String> {
            Err("Nova's desktop UI backend currently supports Windows only.".into())
        }
        pub fn close(&mut self, _: u64) -> Result<(), String> {
            Err("Nova's desktop UI backend currently supports Windows only.".into())
        }
        pub fn recv(&mut self) -> Result<HostEvent, String> {
            Err("Nova's desktop UI backend currently supports Windows only.".into())
        }
    }
}

pub(super) use platform::Host;
