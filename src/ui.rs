//! Nova's platform-neutral UI model and the versioned webview bridge.

use serde_json::{Map, Value as JsonValue, json};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::Read;
use std::path::Path;

use crate::value::{NativeFunction, Value, normalize_dictionary};

mod backend;

const MAX_IMAGE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UiCallback {
    pub name: String,
    pub arguments: Vec<Value>,
}

#[derive(Debug, Clone)]
struct CallbackBinding {
    function: String,
    bound_arguments: Vec<Value>,
    event_argument: bool,
}

type ComponentSettings = (Map<String, JsonValue>, HashMap<String, CallbackBinding>);

#[derive(Debug, Clone)]
struct WindowModel {
    title: String,
    width: u32,
    height: u32,
    min_width: Option<u32>,
    min_height: Option<u32>,
    max_width: Option<u32>,
    max_height: Option<u32>,
    resizable: bool,
    background: Option<String>,
    theme: String,
    roots: Vec<u64>,
    close_callback: Option<CallbackBinding>,
    shown: bool,
    closed: bool,
}

#[derive(Debug, Clone)]
struct ComponentModel {
    id: u64,
    window_id: u64,
    parent_id: Option<u64>,
    kind: String,
    properties: Map<String, JsonValue>,
    options: Vec<Value>,
    current_value: Value,
    callbacks: HashMap<String, CallbackBinding>,
    children: Vec<u64>,
    removed: bool,
}

pub struct UiRuntime {
    next_id: u64,
    windows: BTreeMap<u64, WindowModel>,
    components: BTreeMap<u64, ComponentModel>,
    host: Option<backend::Host>,
    pending_callbacks: VecDeque<UiCallback>,
}

impl Default for UiRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl UiRuntime {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            windows: BTreeMap::new(),
            components: BTreeMap::new(),
            host: None,
            pending_callbacks: VecDeque::new(),
        }
    }

    pub fn call(
        &mut self,
        function: &NativeFunction,
        arguments: &[Value],
        callback_arity: impl Fn(&str) -> Option<usize>,
    ) -> Result<Value, String> {
        if function.module != "UI" {
            return Err(format!(
                "UI runtime cannot call {}.{}.",
                function.module, function.name
            ));
        }
        match function.name.as_str() {
            "window" => self.create_window(arguments, &callback_arity),
            "text" => self.create_text(arguments),
            "button" => self.create_button(arguments, &callback_arity),
            "input" => self.create_input(arguments, &callback_arity),
            "dropdown" => self.create_dropdown(arguments, &callback_arity),
            "checkbox" => self.create_checkbox(arguments, &callback_arity),
            "slider" => self.create_slider(arguments, &callback_arity),
            "image" => self.create_image(arguments),
            "column" | "row" | "container" => self.create_container(&function.name, arguments),
            "show" => self.show(arguments),
            "close" => self.close(arguments),
            "value" => self.read_value(arguments),
            "set_text" | "set_label" => self.update_text(&function.name, arguments),
            "set_value" => self.update_value(arguments),
            "set_visible" => self.update_flag("visible", arguments),
            "set_enabled" => self.update_flag("enabled", arguments),
            "set_style" => self.update_style(arguments),
            "set_options" => self.update_options(arguments),
            "remove" => self.remove_component(arguments),
            "theme" => self.set_theme(arguments),
            name => Err(format!("UI has no function '{}'.", name)),
        }
    }

    pub(crate) fn wait_for_callback(&mut self) -> Result<Option<UiCallback>, String> {
        if let Some(callback) = self.pending_callbacks.pop_front() {
            return Ok(Some(callback));
        }
        if !self
            .windows
            .values()
            .any(|window| window.shown && !window.closed)
        {
            return Ok(None);
        }
        if self.host.is_none() {
            return Err("UI windows are marked open but the UI host is unavailable.".into());
        }
        loop {
            let event = self.host.as_mut().unwrap().recv();
            match event {
                Ok(backend::HostEvent::Ipc(message)) => {
                    if let Ok(Some(callback)) = self.accept_frontend_message(&message) {
                        return Ok(Some(callback));
                    }
                }
                Ok(backend::HostEvent::Closed(window_id)) => {
                    let Some(window) = self.windows.get_mut(&window_id) else {
                        continue;
                    };
                    if window.closed {
                        continue;
                    }
                    window.closed = true;
                    if let Some(binding) = &window.close_callback {
                        return Ok(Some(UiCallback {
                            name: binding.function.clone(),
                            arguments: binding.bound_arguments.clone(),
                        }));
                    }
                }
                Ok(backend::HostEvent::Failed(message)) => return Err(message),
                Err(message) => return Err(message),
            }
            if !self
                .windows
                .values()
                .any(|window| window.shown && !window.closed)
            {
                return Ok(None);
            }
        }
    }

    fn create_window(
        &mut self,
        arguments: &[Value],
        callback_arity: &impl Fn(&str) -> Option<usize>,
    ) -> Result<Value, String> {
        expect_arity("UI.window", arguments, 3, 4)?;
        let title = expect_string("UI.window", &arguments[0])?.to_owned();
        let width = dimension("UI.window width", &arguments[1])?;
        let height = dimension("UI.window height", &arguments[2])?;
        let settings = if arguments.len() == 4 {
            dictionary("UI.window options", &arguments[3])?
        } else {
            Vec::new()
        };
        let mut min_width = None;
        let mut min_height = None;
        let mut max_width = None;
        let mut max_height = None;
        let mut resizable = true;
        let mut background = None;
        let mut theme = "light".to_string();
        let mut close_callback = None;
        for (key, value) in settings {
            match key.as_str() {
                "min_width" => min_width = Some(dimension("UI.window min_width", value)?),
                "min_height" => min_height = Some(dimension("UI.window min_height", value)?),
                "max_width" => max_width = Some(dimension("UI.window max_width", value)?),
                "max_height" => max_height = Some(dimension("UI.window max_height", value)?),
                "resizable" => resizable = expect_bool("UI.window resizable", value)?,
                "background" => {
                    let color = expect_string("UI.window background", value)?.to_string();
                    validate_color(&color)?;
                    background = Some(color);
                }
                "theme" => {
                    theme = expect_theme(value)?;
                }
                "on_close" => {
                    close_callback = Some(make_callback(
                        "UI.window on_close",
                        value,
                        &[],
                        false,
                        callback_arity,
                    )?);
                }
                unknown => return Err(format!("UI.window has unknown option '{}'.", unknown)),
            }
        }
        if min_width.is_some_and(|value| value > width)
            || min_height.is_some_and(|value| value > height)
            || max_width.is_some_and(|value| value < width)
            || max_height.is_some_and(|value| value < height)
        {
            return Err("UI.window min/max dimensions must include the initial size.".into());
        }
        let id = self.allocate_id()?;
        self.windows.insert(
            id,
            WindowModel {
                title,
                width,
                height,
                min_width,
                min_height,
                max_width,
                max_height,
                resizable,
                background,
                theme,
                roots: Vec::new(),
                close_callback,
                shown: false,
                closed: false,
            },
        );
        Ok(Value::UiWindow(id))
    }

    fn create_text(&mut self, arguments: &[Value]) -> Result<Value, String> {
        expect_arity("UI.text", arguments, 2, 3)?;
        let parent = self.parent_from(&arguments[0], "UI.text")?;
        let text = expect_string("UI.text", &arguments[1])?;
        let mut properties = Map::new();
        properties.insert("text".into(), json!(text));
        if let Some(style) = arguments.get(2) {
            merge_style(&mut properties, style)?;
        }
        self.add_component(
            parent,
            "text",
            properties,
            Value::None,
            Vec::new(),
            HashMap::new(),
        )
    }

    fn create_button(
        &mut self,
        arguments: &[Value],
        callback_arity: &impl Fn(&str) -> Option<usize>,
    ) -> Result<Value, String> {
        expect_arity("UI.button", arguments, 2, 3)?;
        let parent = self.parent_from(&arguments[0], "UI.button")?;
        let label = expect_string("UI.button", &arguments[1])?;
        let (mut properties, callbacks) =
            component_settings("UI.button", arguments.get(2), &["on_click"], callback_arity)?;
        properties.insert("text".into(), json!(label));
        self.add_component(
            parent,
            "button",
            properties,
            Value::None,
            Vec::new(),
            callbacks,
        )
    }

    fn create_input(
        &mut self,
        arguments: &[Value],
        callback_arity: &impl Fn(&str) -> Option<usize>,
    ) -> Result<Value, String> {
        expect_arity("UI.input", arguments, 1, 3)?;
        let parent = self.parent_from(&arguments[0], "UI.input")?;
        let initial = arguments
            .get(1)
            .map_or(Ok(""), |value| expect_string("UI.input value", value))?;
        let settings = arguments.get(2);
        let (mut properties, callbacks) =
            component_settings("UI.input", settings, &["on_change"], callback_arity)?;
        properties.insert("value".into(), json!(initial));
        let value = Value::String(initial.to_string());
        self.add_component(parent, "input", properties, value, Vec::new(), callbacks)
    }

    fn create_dropdown(
        &mut self,
        arguments: &[Value],
        callback_arity: &impl Fn(&str) -> Option<usize>,
    ) -> Result<Value, String> {
        expect_arity("UI.dropdown", arguments, 2, 4)?;
        let parent = self.parent_from(&arguments[0], "UI.dropdown")?;
        let options = expect_string_list("UI.dropdown options", &arguments[1])?;
        if options.is_empty() {
            return Err("UI.dropdown requires at least one option.".into());
        }
        let initial = if let Some(value) = arguments.get(2) {
            expect_string("UI.dropdown selection", value)?.to_string()
        } else {
            options[0].clone()
        };
        if !options.iter().any(|option| option == &initial) {
            return Err("UI.dropdown initial selection must be one of its options.".into());
        }
        let (mut properties, callbacks) = component_settings(
            "UI.dropdown",
            arguments.get(3),
            &["on_change"],
            callback_arity,
        )?;
        properties.insert("value".into(), json!(initial));
        properties.insert("options".into(), json!(options));
        self.add_component(
            parent,
            "dropdown",
            properties,
            Value::String(initial),
            options.into_iter().map(Value::String).collect(),
            callbacks,
        )
    }

    fn create_checkbox(
        &mut self,
        arguments: &[Value],
        callback_arity: &impl Fn(&str) -> Option<usize>,
    ) -> Result<Value, String> {
        expect_arity("UI.checkbox", arguments, 2, 4)?;
        let parent = self.parent_from(&arguments[0], "UI.checkbox")?;
        let label = expect_string("UI.checkbox", &arguments[1])?;
        let checked = arguments
            .get(2)
            .map_or(Ok(false), |value| expect_bool("UI.checkbox checked", value))?;
        let (mut properties, callbacks) = component_settings(
            "UI.checkbox",
            arguments.get(3),
            &["on_change"],
            callback_arity,
        )?;
        properties.insert("text".into(), json!(label));
        properties.insert("value".into(), json!(checked));
        self.add_component(
            parent,
            "checkbox",
            properties,
            Value::Bool(checked),
            Vec::new(),
            callbacks,
        )
    }

    fn create_slider(
        &mut self,
        arguments: &[Value],
        callback_arity: &impl Fn(&str) -> Option<usize>,
    ) -> Result<Value, String> {
        expect_arity("UI.slider", arguments, 4, 5)?;
        let parent = self.parent_from(&arguments[0], "UI.slider")?;
        let min = finite_number("UI.slider minimum", &arguments[1])?;
        let max = finite_number("UI.slider maximum", &arguments[2])?;
        let value = finite_number("UI.slider value", &arguments[3])?;
        if min >= max || value < min || value > max {
            return Err("UI.slider requires min < max and min <= value <= max.".into());
        }
        let (mut properties, callbacks) = component_settings(
            "UI.slider",
            arguments.get(4),
            &["on_change"],
            callback_arity,
        )?;
        let step = properties
            .get("step")
            .and_then(JsonValue::as_f64)
            .unwrap_or(1.0);
        if !step.is_finite() || step <= 0.0 {
            return Err("UI.slider step must be a positive finite number.".into());
        }
        properties.insert("min".into(), json!(min));
        properties.insert("max".into(), json!(max));
        properties.insert("value".into(), json!(value));
        self.add_component(
            parent,
            "slider",
            properties,
            number_value(value),
            Vec::new(),
            callbacks,
        )
    }

    fn create_image(&mut self, arguments: &[Value]) -> Result<Value, String> {
        expect_arity("UI.image", arguments, 2, 3)?;
        let parent = self.parent_from(&arguments[0], "UI.image")?;
        let path = expect_string("UI.image path", &arguments[1])?;
        let image = load_image(path)?;
        let mut properties = Map::new();
        properties.insert("src".into(), json!(image));
        properties.insert(
            "alt".into(),
            json!(
                Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Nova image")
            ),
        );
        if let Some(style) = arguments.get(2) {
            merge_style(&mut properties, style)?;
        }
        self.add_component(
            parent,
            "image",
            properties,
            Value::None,
            Vec::new(),
            HashMap::new(),
        )
    }

    fn create_container(&mut self, kind: &str, arguments: &[Value]) -> Result<Value, String> {
        expect_arity(&format!("UI.{}", kind), arguments, 1, 2)?;
        let parent = self.parent_from(&arguments[0], &format!("UI.{}", kind))?;
        let mut properties = Map::new();
        if let Some(style) = arguments.get(1) {
            merge_style(&mut properties, style)?;
        }
        self.add_component(
            parent,
            kind,
            properties,
            Value::None,
            Vec::new(),
            HashMap::new(),
        )
    }

    fn add_component(
        &mut self,
        parent: Parent,
        kind: &str,
        properties: Map<String, JsonValue>,
        current_value: Value,
        options: Vec<Value>,
        callbacks: HashMap<String, CallbackBinding>,
    ) -> Result<Value, String> {
        let id = self.allocate_id()?;
        let (window_id, parent_id) = match parent {
            Parent::Window(id) => (id, None),
            Parent::Component(id) => {
                let component = self.valid_component_mut(id)?;
                (component.window_id, Some(id))
            }
        };
        self.ensure_window_open(window_id)?;
        let component = ComponentModel {
            id,
            window_id,
            parent_id,
            kind: kind.into(),
            properties,
            options,
            current_value,
            callbacks,
            children: Vec::new(),
            removed: false,
        };
        if let Some(parent_id) = parent_id {
            self.components
                .get_mut(&parent_id)
                .unwrap()
                .children
                .push(id);
        } else {
            self.windows.get_mut(&window_id).unwrap().roots.push(id);
        }
        self.components.insert(id, component.clone());
        if self.windows[&window_id].shown {
            if let Err(error) = self.send_component(window_id, id) {
                self.components.remove(&id);
                if let Some(parent_id) = parent_id {
                    self.components
                        .get_mut(&parent_id)
                        .unwrap()
                        .children
                        .retain(|child| *child != id);
                } else {
                    self.windows
                        .get_mut(&window_id)
                        .unwrap()
                        .roots
                        .retain(|root| *root != id);
                }
                return Err(error);
            }
        }
        Ok(Value::UiComponent(id))
    }

    fn show(&mut self, arguments: &[Value]) -> Result<Value, String> {
        expect_arity("UI.show", arguments, 1, 1)?;
        let id = expect_window_handle("UI.show", &arguments[0])?;
        let (
            snapshot,
            title,
            width,
            height,
            min_width,
            min_height,
            max_width,
            max_height,
            resizable,
        ) = {
            let window = self.valid_window(id)?;
            if window.closed {
                return Err("UI.show cannot reopen a closed window.".into());
            }
            if window.shown {
                return Ok(Value::None);
            }
            let roots = window
                .roots
                .iter()
                .filter_map(|id| self.component_json(*id))
                .collect::<Vec<_>>();
            let snapshot = json!({
                "protocol": 1,
                "kind": "init",
                "window_id": id,
                "theme": window.theme,
                "background": window.background,
                "components": roots,
            });
            (
                snapshot,
                window.title.clone(),
                window.width,
                window.height,
                window.min_width,
                window.min_height,
                window.max_width,
                window.max_height,
                window.resizable,
            )
        };
        if self.host.is_none() {
            self.host = Some(backend::Host::start()?);
        }
        self.host.as_mut().unwrap().open(
            id, title, width, height, min_width, min_height, max_width, max_height, resizable,
            snapshot,
        )?;
        self.windows.get_mut(&id).unwrap().shown = true;
        Ok(Value::None)
    }

    fn close(&mut self, arguments: &[Value]) -> Result<Value, String> {
        expect_arity("UI.close", arguments, 1, 1)?;
        let id = expect_window_handle("UI.close", &arguments[0])?;
        let (shown, closed, callback) = {
            let window = self.valid_window(id)?;
            (window.shown, window.closed, window.close_callback.clone())
        };
        if shown && !closed {
            self.host
                .as_mut()
                .ok_or_else(|| "UI host is unavailable.".to_string())?
                .close(id)?;
            self.windows.get_mut(&id).unwrap().closed = true;
            if let Some(binding) = callback {
                self.pending_callbacks.push_back(UiCallback {
                    name: binding.function,
                    arguments: binding.bound_arguments,
                });
            }
        }
        Ok(Value::None)
    }

    fn read_value(&self, arguments: &[Value]) -> Result<Value, String> {
        expect_arity("UI.value", arguments, 1, 1)?;
        let id = expect_component_handle("UI.value", &arguments[0])?;
        let component = self.valid_component(id)?;
        if component.kind == "text"
            || component.kind == "button"
            || component.kind.ends_with("container")
            || component.kind == "row"
            || component.kind == "column"
            || component.kind == "image"
        {
            return Err(format!(
                "UI.value is not available for {} components.",
                component.kind
            ));
        }
        Ok(component.current_value.clone())
    }

    fn update_text(&mut self, name: &str, arguments: &[Value]) -> Result<Value, String> {
        expect_arity(&format!("UI.{}", name), arguments, 2, 2)?;
        let id = expect_component_handle(name, &arguments[0])?;
        let text = expect_string(name, &arguments[1])?;
        self.ensure_component_window_open(id)?;
        let component = self.valid_component(id)?;
        if !matches!(component.kind.as_str(), "text" | "button" | "checkbox") {
            return Err(format!(
                "UI.{} cannot update a {} component.",
                name, component.kind
            ));
        }
        self.send_update(id, "text", json!(text))?;
        self.components
            .get_mut(&id)
            .unwrap()
            .properties
            .insert("text".into(), json!(text));
        Ok(Value::None)
    }

    fn update_value(&mut self, arguments: &[Value]) -> Result<Value, String> {
        expect_arity("UI.set_value", arguments, 2, 2)?;
        let id = expect_component_handle("UI.set_value", &arguments[0])?;
        self.ensure_component_window_open(id)?;
        let component = self.valid_component(id)?;
        match component.kind.as_str() {
            "input" => match &arguments[1] {
                Value::String(_) => {}
                value => {
                    return Err(format!(
                        "UI.set_value for input expects String, received {}.",
                        value.type_name()
                    ));
                }
            },
            "dropdown" => {
                let selection = expect_string("UI.set_value dropdown", &arguments[1])?;
                if !component
                    .options
                    .iter()
                    .any(|option| option == &Value::String(selection.into()))
                {
                    return Err("UI.set_value dropdown selection must exist in its options.".into());
                }
            }
            "checkbox" => {
                if !matches!(arguments[1], Value::Bool(_)) {
                    return Err("UI.set_value for checkbox expects Bool.".into());
                }
            }
            "slider" => {
                let value = finite_number("UI.set_value slider", &arguments[1])?;
                let min = component.properties["min"].as_f64().unwrap();
                let max = component.properties["max"].as_f64().unwrap();
                if value < min || value > max {
                    return Err(
                        "UI.set_value slider value must remain within its minimum and maximum."
                            .into(),
                    );
                }
            }
            kind => return Err(format!("UI.set_value cannot update a {} component.", kind)),
        }
        let value = arguments[1].clone();
        let json_value = value_to_json(&value)?;
        self.send_update(id, "value", json_value.clone())?;
        let component = self.components.get_mut(&id).unwrap();
        component.current_value = value;
        component
            .properties
            .insert("value".into(), json_value.clone());
        Ok(Value::None)
    }

    fn update_flag(&mut self, key: &str, arguments: &[Value]) -> Result<Value, String> {
        let name = if key == "visible" {
            "UI.set_visible"
        } else {
            "UI.set_enabled"
        };
        expect_arity(name, arguments, 2, 2)?;
        let id = expect_component_handle(name, &arguments[0])?;
        let value = expect_bool(name, &arguments[1])?;
        self.ensure_component_window_open(id)?;
        self.send_update(id, key, json!(value))?;
        self.components
            .get_mut(&id)
            .unwrap()
            .properties
            .insert(key.into(), json!(value));
        Ok(Value::None)
    }

    fn update_style(&mut self, arguments: &[Value]) -> Result<Value, String> {
        expect_arity("UI.set_style", arguments, 2, 2)?;
        let id = expect_component_handle("UI.set_style", &arguments[0])?;
        let mut style = Map::new();
        merge_style(&mut style, &arguments[1])?;
        self.ensure_component_window_open(id)?;
        let component = self.valid_component(id)?;
        let window_id = component.window_id;
        let mut merged = component.properties.clone();
        for (key, value) in &style {
            merged.insert(key.clone(), value.clone());
        }
        self.send_message(
            window_id,
            json!({"protocol":1,"kind":"update","component_id":id,"properties":style}),
        )?;
        self.components.get_mut(&id).unwrap().properties = merged;
        Ok(Value::None)
    }

    fn update_options(&mut self, arguments: &[Value]) -> Result<Value, String> {
        expect_arity("UI.set_options", arguments, 2, 2)?;
        let id = expect_component_handle("UI.set_options", &arguments[0])?;
        let options = expect_string_list("UI.set_options", &arguments[1])?;
        if options.is_empty() {
            return Err("UI.set_options requires at least one option.".into());
        }
        self.ensure_component_window_open(id)?;
        let component = self.valid_component(id)?;
        if component.kind != "dropdown" {
            return Err("UI.set_options is only valid for dropdown components.".into());
        }
        let window_id = component.window_id;
        let current = component.current_value.to_string_value();
        let selection = if options.contains(&current) {
            current
        } else {
            options[0].clone()
        };
        self.send_message(
            window_id,
            json!({"protocol":1,"kind":"update","component_id":id,"properties":{"options":options,"value":selection}}),
        )?;
        let component = self.components.get_mut(&id).unwrap();
        component.options = options.iter().cloned().map(Value::String).collect();
        component.current_value = Value::String(selection.clone());
        component
            .properties
            .insert("options".into(), json!(options));
        component
            .properties
            .insert("value".into(), json!(selection));
        Ok(Value::None)
    }

    fn remove_component(&mut self, arguments: &[Value]) -> Result<Value, String> {
        expect_arity("UI.remove", arguments, 1, 1)?;
        let id = expect_component_handle("UI.remove", &arguments[0])?;
        let (window_id, parent_id) = {
            let component = self.valid_component(id)?;
            self.ensure_window_open(component.window_id)?;
            (component.window_id, component.parent_id)
        };
        self.send_message(
            window_id,
            json!({"protocol":1,"kind":"remove","component_id":id}),
        )?;
        self.remove_tree(id);
        if let Some(parent_id) = parent_id {
            self.components
                .get_mut(&parent_id)
                .unwrap()
                .children
                .retain(|child| *child != id);
        } else {
            self.windows
                .get_mut(&window_id)
                .unwrap()
                .roots
                .retain(|root| *root != id);
        }
        Ok(Value::None)
    }

    fn set_theme(&mut self, arguments: &[Value]) -> Result<Value, String> {
        expect_arity("UI.theme", arguments, 2, 2)?;
        let id = expect_window_handle("UI.theme", &arguments[0])?;
        let theme = expect_theme(&arguments[1])?;
        self.ensure_window_open(id)?;
        self.send_message(id, json!({"protocol":1,"kind":"theme","theme":theme}))?;
        self.windows.get_mut(&id).unwrap().theme = theme;
        Ok(Value::None)
    }

    fn accept_frontend_message(&mut self, message: &str) -> Result<Option<UiCallback>, String> {
        let parsed: JsonValue =
            serde_json::from_str(message).map_err(|_| "Malformed UI event message.".to_string())?;
        let object = parsed
            .as_object()
            .ok_or("UI event must be a JSON object.")?;
        if object.get("protocol").and_then(JsonValue::as_u64) != Some(1)
            || object.get("kind").and_then(JsonValue::as_str) != Some("event")
        {
            return Err("Unsupported UI event protocol or message kind.".into());
        }
        let window_id = object
            .get("window_id")
            .and_then(JsonValue::as_u64)
            .ok_or("UI event has no valid window ID.")?;
        let component_id = object
            .get("component_id")
            .and_then(JsonValue::as_u64)
            .ok_or("UI event has no valid component ID.")?;
        let event = object
            .get("event")
            .and_then(JsonValue::as_str)
            .ok_or("UI event has no event name.")?;
        let window = self.valid_window(window_id)?;
        if !window.shown || window.closed {
            return Err("UI event targets a window that is not open.".into());
        }
        let component = self.valid_component_mut(component_id)?;
        if component.window_id != window_id {
            return Err("UI event component does not belong to its window.".into());
        }
        let expected_kind = match event {
            "click" => "button",
            "change" => match component.kind.as_str() {
                "input" | "dropdown" | "checkbox" | "slider" => component.kind.as_str(),
                _ => return Err("Change event is invalid for this component.".into()),
            },
            _ => return Err("Unsupported UI event name.".into()),
        };
        if component.kind != expected_kind || component.removed {
            return Err("UI event targets an invalid component.".into());
        }
        let event_value = if event == "click" {
            Value::None
        } else {
            let value = object.get("value").ok_or("UI change event has no value.")?;
            let converted = json_to_value(value)?;
            validate_event_value(component, &converted)?;
            component.current_value = converted.clone();
            component.properties.insert("value".into(), value.clone());
            converted
        };
        let Some(binding) = component.callbacks.get(event).cloned() else {
            return Ok(None);
        };
        let mut arguments =
            Vec::with_capacity(binding.bound_arguments.len() + usize::from(binding.event_argument));
        if binding.event_argument {
            arguments.push(event_value);
        }
        arguments.extend(binding.bound_arguments);
        Ok(Some(UiCallback {
            name: binding.function,
            arguments,
        }))
    }

    fn parent_from(&self, value: &Value, api: &str) -> Result<Parent, String> {
        match value {
            Value::UiWindow(id) => {
                self.ensure_window_open(*id)?;
                Ok(Parent::Window(*id))
            }
            Value::UiComponent(id) => {
                let component = self.valid_component(*id)?;
                self.ensure_window_open(component.window_id)?;
                if !matches!(component.kind.as_str(), "column" | "row" | "container") {
                    return Err(format!(
                        "{} parent must be a window or layout container.",
                        api
                    ));
                }
                Ok(Parent::Component(*id))
            }
            other => Err(format!(
                "{} expects a Window or Container handle, received {}.",
                api,
                other.type_name()
            )),
        }
    }

    fn valid_window(&self, id: u64) -> Result<&WindowModel, String> {
        self.windows
            .get(&id)
            .ok_or_else(|| format!("Invalid UI window handle {}.", id))
    }

    fn ensure_window_open(&self, id: u64) -> Result<(), String> {
        if self.valid_window(id)?.closed {
            return Err("UI operation cannot target a closed window.".into());
        }
        Ok(())
    }

    fn ensure_component_window_open(&self, id: u64) -> Result<(), String> {
        let component = self.valid_component(id)?;
        self.ensure_window_open(component.window_id)
    }

    fn valid_component(&self, id: u64) -> Result<&ComponentModel, String> {
        self.components
            .get(&id)
            .filter(|component| !component.removed)
            .ok_or_else(|| format!("Invalid or removed UI component handle {}.", id))
    }

    fn valid_component_mut(&mut self, id: u64) -> Result<&mut ComponentModel, String> {
        self.components
            .get_mut(&id)
            .filter(|component| !component.removed)
            .ok_or_else(|| format!("Invalid or removed UI component handle {}.", id))
    }

    fn component_json(&self, id: u64) -> Option<JsonValue> {
        let component = self.components.get(&id)?;
        if component.removed {
            return None;
        }
        let mut properties = component.properties.clone();
        properties.insert("id".into(), json!(component.id));
        properties.insert("kind".into(), json!(component.kind));
        properties.insert("parent_id".into(), json!(component.parent_id));
        properties.insert(
            "children".into(),
            json!(
                component
                    .children
                    .iter()
                    .filter_map(|child| self.component_json(*child))
                    .collect::<Vec<_>>()
            ),
        );
        Some(JsonValue::Object(properties))
    }

    fn send_component(&mut self, window_id: u64, id: u64) -> Result<(), String> {
        let component = self
            .component_json(id)
            .ok_or_else(|| format!("Invalid component {}.", id))?;
        self.send_message(
            window_id,
            json!({"protocol":1,"kind":"create","component":component}),
        )
    }

    fn send_update(&mut self, id: u64, key: &str, value: JsonValue) -> Result<(), String> {
        let window_id = self.valid_component(id)?.window_id;
        self.send_message(
            window_id,
            json!({"protocol":1,"kind":"update","component_id":id,"properties":{key:value}}),
        )
    }

    fn send_message(&mut self, window_id: u64, message: JsonValue) -> Result<(), String> {
        if self
            .windows
            .get(&window_id)
            .is_some_and(|window| window.shown && !window.closed)
        {
            self.host
                .as_mut()
                .ok_or("UI host is unavailable.")?
                .send(window_id, message)?;
        }
        Ok(())
    }

    fn remove_tree(&mut self, id: u64) {
        if let Some(component) = self.components.get_mut(&id) {
            component.removed = true;
            let children = component.children.clone();
            for child in children {
                self.remove_tree(child);
            }
        }
    }

    fn allocate_id(&mut self) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or("UI handle ID space exhausted.")?;
        Ok(id)
    }
}

#[derive(Debug, Clone, Copy)]
enum Parent {
    Window(u64),
    Component(u64),
}

fn component_settings(
    api: &str,
    value: Option<&Value>,
    callback_events: &[&str],
    callback_arity: &impl Fn(&str) -> Option<usize>,
) -> Result<ComponentSettings, String> {
    let mut properties = Map::new();
    let mut callbacks = HashMap::new();
    let Some(value) = value else {
        return Ok((properties, callbacks));
    };
    let entries = dictionary(api, value)?;
    let mut bound_args: HashMap<String, Vec<Value>> = HashMap::new();
    for (key, value) in &entries {
        if let Some(event_key) = key.strip_suffix("_args") {
            if !matches!(event_key, "on_click" | "on_change")
                || !callback_events.contains(&event_key)
            {
                return Err(format!("{}.{} is not valid for this component.", api, key));
            }
            if !entries.iter().any(|(entry_key, _)| entry_key == event_key) {
                return Err(format!(
                    "{}.{} requires a matching {} callback.",
                    api, key, event_key
                ));
            }
            let Value::List(arguments) = value else {
                return Err(format!("{}.{} must be a List.", api, key));
            };
            bound_args.insert(event_key.to_string(), arguments.clone());
        }
    }
    for (key, value) in entries {
        if callback_events.contains(&key.as_str()) {
            let event_argument = key != "on_click";
            let binding = make_callback(
                api,
                value,
                bound_args.get(&key).map_or(&[], Vec::as_slice),
                event_argument,
                callback_arity,
            )?;
            callbacks.insert(
                if key == "on_click" { "click" } else { "change" }.into(),
                binding,
            );
        } else if key.ends_with("_args") {
            // The matching callback and component event were checked above.
        } else {
            validate_style(&key, value)?;
            let json_value = value_to_json(value)?;
            properties.insert(key, json_value);
        }
    }
    Ok((properties, callbacks))
}

fn make_callback(
    api: &str,
    value: &Value,
    bound_arguments: &[Value],
    event_argument: bool,
    callback_arity: &impl Fn(&str) -> Option<usize>,
) -> Result<CallbackBinding, String> {
    let function = expect_string(api, value)?;
    if function.is_empty()
        || !function
            .chars()
            .all(|character| character == '_' || character.is_ascii_alphanumeric())
        || function
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
    {
        return Err(format!("{} callback name '{}' is invalid.", api, function));
    }
    let Some(arity) = callback_arity(function) else {
        return Err(format!(
            "{} callback '{}' is not a Nova function.",
            api, function
        ));
    };
    let required = bound_arguments.len() + usize::from(event_argument);
    if arity != required {
        return Err(format!(
            "{} callback '{}' expects {} argument(s); this binding supplies {}.",
            api, function, arity, required
        ));
    }
    Ok(CallbackBinding {
        function: function.to_string(),
        bound_arguments: bound_arguments.to_vec(),
        event_argument,
    })
}

fn merge_style(properties: &mut Map<String, JsonValue>, style: &Value) -> Result<(), String> {
    for (key, value) in dictionary("UI style", style)? {
        validate_style(&key, value)?;
        properties.insert(key, value_to_json(value)?);
    }
    Ok(())
}

fn validate_style(key: &str, value: &Value) -> Result<(), String> {
    match key {
        "background" | "color" | "border_color" => {
            let color = expect_string("UI style color", value)?;
            validate_color(color)?;
        }
        "width" | "height" | "padding" | "margin" | "gap" | "border_radius" | "font_size"
        | "opacity" | "min_width" | "min_height" | "max_width" | "max_height" | "step" | "min"
        | "max" => {
            let number = finite_number("UI style dimension", value)?;
            if number < 0.0 || ((key == "opacity") && number > 1.0) {
                return Err(format!("UI style '{}' is outside its allowed range.", key));
            }
        }
        "font_weight" => match value {
            Value::String(text) if matches!(text.as_str(), "normal" | "bold") => {}
            Value::Int(weight) if (100..=900).contains(weight) && weight % 100 == 0 => {}
            _ => return Err(
                "UI style font_weight must be normal, bold, or a multiple of 100 from 100 to 900."
                    .into(),
            ),
        },
        "align" | "text_align" => match expect_string("UI style alignment", value)? {
            "start" | "center" | "end" | "left" | "right" => {}
            _ => {
                return Err(format!(
                    "UI style '{}' must be start, center, end, left, or right.",
                    key
                ));
            }
        },
        "position" => match expect_string("UI style position", value)? {
            "relative" | "absolute" => {}
            _ => return Err("UI style position must be relative or absolute.".into()),
        },
        "left" | "top" => {
            let _ = finite_number("UI style position", value)?;
        }
        "visible" | "enabled" | "resizable" => {
            let _ = expect_bool("UI style flag", value)?;
        }
        "layout" => match expect_string("UI layout", value)? {
            "column" | "row" => {}
            _ => return Err("UI layout must be column or row.".into()),
        },
        "placeholder" | "alt" => {
            let _ = expect_string("UI style text", value)?;
        }
        unknown => {
            return Err(format!(
                "Unknown UI style or component option '{}'.",
                unknown
            ));
        }
    }
    Ok(())
}

fn validate_color(color: &str) -> Result<(), String> {
    let named = matches!(
        color,
        "transparent"
            | "white"
            | "black"
            | "red"
            | "green"
            | "blue"
            | "gray"
            | "grey"
            | "navy"
            | "teal"
            | "purple"
            | "orange"
            | "inherit"
            | "currentColor"
    );
    let hex = color.strip_prefix('#').is_some_and(|digits| {
        matches!(digits.len(), 3 | 4 | 6 | 8)
            && digits
                .chars()
                .all(|character| character.is_ascii_hexdigit())
    });
    if named || hex {
        Ok(())
    } else {
        Err(format!(
            "Invalid UI color '{}'; use a named color or a 3, 4, 6, or 8 digit hex color.",
            color
        ))
    }
}

fn validate_event_value(component: &ComponentModel, value: &Value) -> Result<(), String> {
    match (component.kind.as_str(), value) {
        ("input", Value::String(_)) => Ok(()),
        ("dropdown", Value::String(text))
            if component.options.contains(&Value::String(text.clone())) =>
        {
            Ok(())
        }
        ("checkbox", Value::Bool(_)) => Ok(()),
        ("slider", Value::Int(_) | Value::Float(_)) => {
            let number = finite_number("UI slider event", value)?;
            let min = component.properties["min"].as_f64().unwrap_or(f64::MIN);
            let max = component.properties["max"].as_f64().unwrap_or(f64::MAX);
            if number < min || number > max {
                Err("UI slider event value is outside its allowed range.".into())
            } else {
                Ok(())
            }
        }
        ("dropdown", Value::String(_)) => {
            Err("UI dropdown event selected an unknown option.".into())
        }
        _ => Err(format!(
            "UI event has the wrong value type for {}.",
            component.kind
        )),
    }
}

pub fn parse_event_message(message: &str) -> Result<JsonValue, String> {
    let value: JsonValue =
        serde_json::from_str(message).map_err(|_| "Malformed UI event message.".to_string())?;
    let object = value.as_object().ok_or("UI event must be a JSON object.")?;
    if object.get("protocol").and_then(JsonValue::as_u64) != Some(1)
        || object.get("kind").and_then(JsonValue::as_str) != Some("event")
    {
        return Err("Unsupported UI event protocol or message kind.".into());
    }
    Ok(value)
}

fn value_to_json(value: &Value) -> Result<JsonValue, String> {
    match value {
        Value::Int(value) => Ok(json!(value)),
        Value::Float(value) if value.is_finite() => Ok(json!(value)),
        Value::Float(_) => Err("Non-finite Float values cannot cross the UI bridge.".into()),
        Value::String(value) => Ok(json!(value)),
        Value::Bool(value) => Ok(json!(value)),
        Value::None => Ok(JsonValue::Null),
        Value::List(values) => values
            .iter()
            .map(value_to_json)
            .collect::<Result<Vec<_>, _>>()
            .map(JsonValue::Array),
        Value::Dictionary(entries) => {
            let entries = normalize_dictionary(entries);
            if entries
                .iter()
                .all(|(key, _)| matches!(key, Value::String(_)))
            {
                let mut object = Map::new();
                for (key, value) in &entries {
                    let Value::String(key) = key else {
                        unreachable!()
                    };
                    object.insert(key.clone(), value_to_json(value)?);
                }
                Ok(JsonValue::Object(object))
            } else {
                let entries = entries
                    .iter()
                    .map(|(key, value)| Ok(json!([value_to_json(key)?, value_to_json(value)?])))
                    .collect::<Result<Vec<_>, String>>()?;
                Ok(json!({"$nova":"dictionary","entries":entries}))
            }
        }
        Value::NativeFunction(_) => Err("Function values cannot cross the UI bridge.".into()),
        Value::UiWindow(_) | Value::UiComponent(_) => {
            Err("UI handles cannot cross the UI bridge.".into())
        }
    }
}

fn json_to_value(value: &JsonValue) -> Result<Value, String> {
    match value {
        JsonValue::Null => Ok(Value::None),
        JsonValue::Bool(value) => Ok(Value::Bool(*value)),
        JsonValue::Number(value) => {
            if let Some(integer) = value.as_i64() {
                Ok(Value::Int(integer))
            } else if let Some(float) = value.as_f64().filter(|value| value.is_finite()) {
                Ok(Value::Float(float))
            } else {
                Err("UI event number is outside Nova's supported range.".into())
            }
        }
        JsonValue::String(value) => Ok(Value::String(value.clone())),
        JsonValue::Array(values) => values
            .iter()
            .map(json_to_value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::List),
        JsonValue::Object(values) => {
            let entries = values
                .iter()
                .map(|(key, value)| Ok((Value::String(key.clone()), json_to_value(value)?)))
                .collect::<Result<Vec<_>, String>>()?;
            Ok(Value::Dictionary(entries))
        }
    }
}

fn expect_arity(
    api: &str,
    arguments: &[Value],
    minimum: usize,
    maximum: usize,
) -> Result<(), String> {
    if arguments.len() < minimum || arguments.len() > maximum {
        return Err(if minimum == maximum {
            format!(
                "{} expects {} argument{} but received {}.",
                api,
                minimum,
                if minimum == 1 { "" } else { "s" },
                arguments.len()
            )
        } else {
            format!(
                "{} expects {} to {} arguments but received {}.",
                api,
                minimum,
                maximum,
                arguments.len()
            )
        });
    }
    Ok(())
}

fn dictionary<'a>(api: &str, value: &'a Value) -> Result<Vec<(String, &'a Value)>, String> {
    let Value::Dictionary(entries) = value else {
        return Err(format!(
            "{} options must be a Dictionary, received {}.",
            api,
            value.type_name()
        ));
    };
    entries
        .iter()
        .map(|(key, value)| match key {
            Value::String(key) => Ok((key.clone(), value)),
            _ => Err(format!("{} option keys must be Strings.", api)),
        })
        .collect()
}

fn expect_string<'a>(api: &str, value: &'a Value) -> Result<&'a str, String> {
    match value {
        Value::String(value) => Ok(value),
        _ => Err(format!(
            "{} expects String, received {}.",
            api,
            value.type_name()
        )),
    }
}

fn expect_bool(api: &str, value: &Value) -> Result<bool, String> {
    match value {
        Value::Bool(value) => Ok(*value),
        _ => Err(format!(
            "{} expects Bool, received {}.",
            api,
            value.type_name()
        )),
    }
}

fn expect_string_list(api: &str, value: &Value) -> Result<Vec<String>, String> {
    let Value::List(values) = value else {
        return Err(format!(
            "{} expects a List, received {}.",
            api,
            value.type_name()
        ));
    };
    values
        .iter()
        .map(|value| Ok(expect_string(api, value)?.to_string()))
        .collect()
}

fn expect_window_handle(api: &str, value: &Value) -> Result<u64, String> {
    match value {
        Value::UiWindow(id) => Ok(*id),
        _ => Err(format!(
            "{} expects a Window handle, received {}.",
            api,
            value.type_name()
        )),
    }
}

fn expect_component_handle(api: &str, value: &Value) -> Result<u64, String> {
    match value {
        Value::UiComponent(id) => Ok(*id),
        _ => Err(format!(
            "{} expects a Component handle, received {}.",
            api,
            value.type_name()
        )),
    }
}

fn finite_number(api: &str, value: &Value) -> Result<f64, String> {
    let number = match value {
        Value::Int(value) => *value as f64,
        Value::Float(value) => *value,
        _ => {
            return Err(format!(
                "{} expects a number, received {}.",
                api,
                value.type_name()
            ));
        }
    };
    if number.is_finite() {
        Ok(number)
    } else {
        Err(format!("{} requires a finite number.", api))
    }
}

fn number_value(value: f64) -> Value {
    if value.fract() == 0.0 && value >= i64::MIN as f64 && value < 9_223_372_036_854_775_808.0 {
        Value::Int(value as i64)
    } else {
        Value::Float(value)
    }
}

fn dimension(api: &str, value: &Value) -> Result<u32, String> {
    let number = finite_number(api, value)?;
    if number.fract() != 0.0 || !(120.0..=7680.0).contains(&number) {
        return Err(format!("{} must be a whole number from 120 to 7680.", api));
    }
    Ok(number as u32)
}

fn expect_theme(value: &Value) -> Result<String, String> {
    match expect_string("UI theme", value)? {
        "light" => Ok("light".into()),
        "dark" => Ok("dark".into()),
        theme => Err(format!(
            "Unknown UI theme '{}'; use 'light' or 'dark'.",
            theme
        )),
    }
}

fn load_image(path: &str) -> Result<String, String> {
    let path = Path::new(path);
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return Err("UI.image supports local PNG, JPEG, GIF, and WebP files.".into()),
    };
    let file = std::fs::File::open(path)
        .map_err(|error| format!("UI.image could not read '{}': {}.", path.display(), error))?;
    if file
        .metadata()
        .map_err(|error| {
            format!(
                "UI.image could not inspect '{}': {}.",
                path.display(),
                error
            )
        })?
        .len()
        > MAX_IMAGE_BYTES as u64
    {
        return Err("UI.image file must be between 1 byte and 1 MiB.".into());
    }
    let mut bytes = Vec::new();
    file.take((MAX_IMAGE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("UI.image could not read '{}': {}.", path.display(), error))?;
    encode_image_data_uri(&bytes, mime)
}

fn encode_image_data_uri(bytes: &[u8], mime: &str) -> Result<String, String> {
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err("UI.image file must be between 1 byte and 1 MiB.".into());
    }
    let matches_signature = match mime {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "image/webp" => bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        _ => false,
    };
    if !matches_signature {
        return Err("UI.image file contents do not match the supported image type.".into());
    }
    Ok(format!("data:{};base64,{}", mime, base64(bytes)))
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = *chunk.get(1).unwrap_or(&0);
        let c = *chunk.get(2).unwrap_or(&0);
        output.push(TABLE[(a >> 2) as usize] as char);
        output.push(TABLE[(((a & 3) << 4) | (b >> 4)) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[(((b & 15) << 2) | (c >> 6)) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(c & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api(name: &str) -> NativeFunction {
        NativeFunction::new("UI", name)
    }
    fn arity(name: &str) -> Option<usize> {
        match name {
            "change" => Some(1),
            "click" => Some(0),
            "bound" => Some(1),
            _ => None,
        }
    }

    #[test]
    fn ui_handles_are_stable_and_typed() {
        let mut ui = UiRuntime::new();
        let window = ui
            .call(
                &api("window"),
                &["Demo".into(), 800_i64.into(), 600_i64.into()],
                arity,
            )
            .unwrap();
        assert_eq!(window, Value::UiWindow(1));
        let text = ui
            .call(&api("text"), &[window, "Hello".into()], arity)
            .unwrap();
        assert_eq!(text, Value::UiComponent(2));
        let next = ui
            .call(&api("button"), &[Value::UiWindow(1), "Go".into()], arity)
            .unwrap();
        assert_eq!(next, Value::UiComponent(3));
    }

    #[test]
    fn window_and_component_options_are_validated() {
        let mut ui = UiRuntime::new();
        assert!(
            ui.call(
                &api("window"),
                &["Demo".into(), 10_i64.into(), 600_i64.into()],
                arity
            )
            .unwrap_err()
            .contains("120 to 7680")
        );
        let window = ui
            .call(
                &api("window"),
                &["Demo".into(), 800_i64.into(), 600_i64.into()],
                arity,
            )
            .unwrap();
        assert!(
            ui.call(
                &api("button"),
                &[
                    window,
                    "Go".into(),
                    Value::Dictionary(vec![(
                        Value::String("background".into()),
                        Value::String("url(evil)".into())
                    )])
                ],
                arity
            )
            .unwrap_err()
            .contains("Invalid UI color")
        );
    }

    #[test]
    fn callbacks_require_existing_functions_with_the_right_arity() {
        let mut ui = UiRuntime::new();
        let window = ui
            .call(
                &api("window"),
                &["Demo".into(), 800_i64.into(), 600_i64.into()],
                arity,
            )
            .unwrap();
        let options = Value::Dictionary(vec![(
            Value::String("on_click".into()),
            Value::String("missing".into()),
        )]);
        assert!(
            ui.call(
                &api("button"),
                &[window.clone(), "Go".into(), options],
                arity
            )
            .unwrap_err()
            .contains("not a Nova function")
        );
        let options = Value::Dictionary(vec![(
            Value::String("on_change".into()),
            Value::String("click".into()),
        )]);
        assert!(
            ui.call(
                &api("input"),
                &[window, Value::String(String::new()), options],
                arity
            )
            .unwrap_err()
            .contains("expects 0 argument")
        );
    }

    #[test]
    fn safe_value_conversion_preserves_non_string_dictionary_keys() {
        let value = Value::Dictionary(vec![(Value::Int(5), Value::String("value".into()))]);
        let json = value_to_json(&value).unwrap();
        assert_eq!(json["$nova"], "dictionary");
        assert_eq!(json["entries"][0][0], 5);
        assert!(value_to_json(&Value::Float(f64::NAN)).is_err());
        assert!(value_to_json(&Value::NativeFunction(NativeFunction::new("Math", "abs"))).is_err());
    }

    #[test]
    fn frontend_event_envelope_is_validated_against_model() {
        let mut ui = UiRuntime::new();
        let window = ui
            .call(
                &api("window"),
                &["Demo".into(), 800_i64.into(), 600_i64.into()],
                arity,
            )
            .unwrap();
        let options = Value::Dictionary(vec![(
            Value::String("on_change".into()),
            Value::String("change".into()),
        )]);
        let input = ui
            .call(
                &api("input"),
                &[window.clone(), Value::String("start".into()), options],
                arity,
            )
            .unwrap();
        let Value::UiComponent(component_id) = input else {
            panic!()
        };
        ui.windows.get_mut(&1).unwrap().shown = true;
        let message = json!({"protocol":1,"kind":"event","window_id":1,"component_id":component_id,"event":"change","value":"updated"}).to_string();
        let callback = ui.accept_frontend_message(&message).unwrap().unwrap();
        assert_eq!(callback.name, "change");
        assert_eq!(callback.arguments, vec![Value::String("updated".into())]);
        assert_eq!(
            ui.components[&component_id].current_value,
            Value::String("updated".into())
        );
        let invalid =
            json!({"protocol":1,"kind":"event","window_id":1,"component_id":999,"event":"click"})
                .to_string();
        assert!(ui.accept_frontend_message(&invalid).is_err());
        assert!(parse_event_message("{broken").is_err());
    }

    #[test]
    fn updates_validate_component_types_and_dropdown_options() {
        let mut ui = UiRuntime::new();
        let window = ui
            .call(
                &api("window"),
                &["Demo".into(), 800_i64.into(), 600_i64.into()],
                arity,
            )
            .unwrap();
        let dropdown = ui
            .call(
                &api("dropdown"),
                &[window, Value::List(vec!["one".into(), "two".into()])],
                arity,
            )
            .unwrap();
        assert!(
            ui.call(
                &api("set_value"),
                &[dropdown.clone(), "three".into()],
                arity
            )
            .unwrap_err()
            .contains("must exist")
        );
        assert_eq!(
            ui.call(&api("value"), &[dropdown], arity).unwrap(),
            Value::String("one".into())
        );
    }

    #[test]
    fn image_base64_is_correct_for_small_payloads() {
        assert_eq!(base64(b"Nova"), "Tm92YQ==");
    }

    #[test]
    fn image_data_uri_checks_size_and_image_signature() {
        assert!(encode_image_data_uri(b"Nova", "image/png").is_err());
        assert!(encode_image_data_uri(&[], "image/png").is_err());
        let too_large = vec![0; MAX_IMAGE_BYTES + 1];
        assert!(encode_image_data_uri(&too_large, "image/png").is_err());
        assert_eq!(
            encode_image_data_uri(b"\x89PNG\r\n\x1a\n", "image/png").unwrap(),
            "data:image/png;base64,iVBORw0KGgo="
        );
        assert!(encode_image_data_uri(b"\x89PNG\r\n\x1a\n", "image/jpeg").is_err());
    }

    #[test]
    fn window_keeps_independent_dimension_constraints() {
        let mut ui = UiRuntime::new();
        ui.call(
            &api("window"),
            &[
                "Demo".into(),
                800_i64.into(),
                600_i64.into(),
                Value::Dictionary(vec![("min_width".into(), 640_i64.into())]),
            ],
            arity,
        )
        .unwrap();
        assert_eq!(ui.windows[&1].min_width, Some(640));
        assert_eq!(ui.windows[&1].min_height, None);
    }

    #[test]
    fn closed_window_rejects_component_updates_and_additions() {
        let mut ui = UiRuntime::new();
        let window = ui
            .call(
                &api("window"),
                &["Demo".into(), 800_i64.into(), 600_i64.into()],
                arity,
            )
            .unwrap();
        let text = ui
            .call(&api("text"), &[window.clone(), "hello".into()], arity)
            .unwrap();
        ui.windows.get_mut(&1).unwrap().closed = true;
        assert!(
            ui.call(&api("set_text"), &[text, "changed".into()], arity)
                .unwrap_err()
                .contains("closed window")
        );
        assert!(
            ui.call(&api("text"), &[window, "too late".into()], arity)
                .unwrap_err()
                .contains("closed window")
        );
    }
}
