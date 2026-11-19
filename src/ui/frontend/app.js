(() => {
  "use strict";
  const nodes = new Map();
  const appRoot = document.getElementById("app");
  window.__NOVA_WINDOW_ID__ = Number(appRoot.dataset.windowId);
  window.__NOVA_QUEUE__ = [];
  window.__NOVA_HANDLER__ = null;
  window.__NOVA_DISPATCH__ = (message) => {
    if (window.__NOVA_HANDLER__) window.__NOVA_HANDLER__(message);
    else window.__NOVA_QUEUE__.push(message);
  };
  const post = (message) => {
    if (window.ipc && typeof window.ipc.postMessage === "function") window.ipc.postMessage(JSON.stringify(message));
  };
  window.addEventListener("error", (event) => {
    post({ protocol: 1, kind: "init_error", window_id: window.__NOVA_WINDOW_ID__, message: String(event.message || "Frontend script error") });
  });
  const send = (componentId, event, value) => {
    const message = { protocol: 1, kind: "event", window_id: window.__NOVA_WINDOW_ID__, component_id: componentId, event };
    if (value !== undefined) message.value = value;
    post(message);
  };

  function applyProperties(element, component) {
    const props = component.properties || component;
    if (Object.prototype.hasOwnProperty.call(props, "visible")) element.hidden = !props.visible;
    if (Object.prototype.hasOwnProperty.call(props, "enabled")) element.disabled = !props.enabled;
    if (Object.prototype.hasOwnProperty.call(props, "text")) {
      if (component.kind === "checkbox") element.querySelector("span").textContent = props.text;
      else element.textContent = props.text;
    }
    if (Object.prototype.hasOwnProperty.call(props, "placeholder")) element.placeholder = props.placeholder;
    if (Object.prototype.hasOwnProperty.call(props, "value")) {
      if (component.kind === "checkbox") element.querySelector("input").checked = !!props.value;
      else element.value = props.value;
    }
    if (Object.prototype.hasOwnProperty.call(props, "options")) {
      const select = element;
      const selected = props.value;
      select.replaceChildren();
      for (const optionValue of props.options) {
        const option = document.createElement("option");
        option.value = optionValue;
        option.textContent = optionValue;
        select.append(option);
      }
      if (selected !== undefined) select.value = selected;
    }
    if (Object.prototype.hasOwnProperty.call(props, "src")) element.src = props.src;
    if (Object.prototype.hasOwnProperty.call(props, "alt")) element.alt = props.alt;
    for (const [key, value] of Object.entries(props)) {
      const cssKey = ({ background: "backgroundColor", border_color: "borderColor", border_radius: "borderRadius", font_size: "fontSize", font_weight: "fontWeight", text_align: "textAlign" })[key] || key.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
      if (["backgroundColor", "color", "borderColor", "width", "height", "padding", "margin", "gap", "borderRadius", "fontSize", "fontWeight", "textAlign", "opacity", "position", "left", "top", "align", "layout", "minWidth", "minHeight", "maxWidth", "maxHeight"].includes(cssKey)) {
        if (cssKey === "align") element.style.alignItems = value === "start" ? "flex-start" : value === "end" ? "flex-end" : value;
        else if (cssKey === "layout") element.style.flexDirection = value;
        else element.style[cssKey] = typeof value === "number" && !["opacity", "fontWeight"].includes(cssKey) ? `${value}px` : String(value);
      }
    }
  }

  function create(component, parent) {
    const kind = component.kind;
    let element;
    if (kind === "text") { element = document.createElement("p"); element.className = "nova-text"; }
    else if (kind === "button") { element = document.createElement("button"); element.type = "button"; element.className = "nova-button"; element.addEventListener("click", () => send(component.id, "click")); }
    else if (kind === "input") { element = document.createElement("input"); element.type = "text"; element.className = "nova-input"; element.addEventListener("input", () => send(component.id, "change", element.value)); }
    else if (kind === "dropdown") { element = document.createElement("select"); element.className = "nova-dropdown"; element.addEventListener("change", () => send(component.id, "change", element.value)); }
    else if (kind === "checkbox") {
      element = document.createElement("label"); element.className = "nova-checkbox";
      const input = document.createElement("input"); input.type = "checkbox";
      const text = document.createElement("span");
      element.append(input, text);
      input.addEventListener("change", () => send(component.id, "change", input.checked));
    }
    else if (kind === "slider") { element = document.createElement("input"); element.type = "range"; element.className = "nova-slider"; element.min = component.min; element.max = component.max; element.step = component.step ?? 1; element.addEventListener("input", () => send(component.id, "change", Number(element.value))); }
    else if (kind === "image") { element = document.createElement("img"); element.className = "nova-image"; }
    else if (kind === "column" || kind === "row" || kind === "container") { element = document.createElement("div"); element.className = `nova-${kind}`; if (kind === "container" && !component.layout) element.style.flexDirection = "column"; }
    else return;
    element.dataset.novaId = String(component.id);
    applyProperties(element, component);
    parent.append(element);
    nodes.set(component.id, { element, component });
    for (const child of component.children || []) create(child, element);
  }

  function receive(message) {
    if (!message || message.protocol !== 1 || typeof message.kind !== "string") return;
    if (message.kind === "init") {
      document.body.dataset.theme = message.theme === "dark" ? "dark" : "light";
      if (message.background) document.body.style.backgroundColor = message.background;
      const root = document.getElementById("app");
      root.replaceChildren();
      for (const component of message.components || []) create(component, root);
    } else if (message.kind === "theme") {
      document.body.dataset.theme = message.theme === "dark" ? "dark" : "light";
    } else if (message.kind === "create") {
      const component = message.component;
      const parentId = component.parent_id;
      const parent = parentId ? nodes.get(parentId)?.element : document.getElementById("app");
      if (parent) create(component, parent);
    } else if (message.kind === "update") {
      const record = nodes.get(message.component_id);
      if (record) {
        const component = { ...record.component, properties: { ...(record.component.properties || record.component), ...(message.properties || {}) } };
        record.component = component;
        applyProperties(record.element, component);
      }
    } else if (message.kind === "remove") {
      const record = nodes.get(message.component_id);
      if (record) {
        record.element.remove();
        for (const [id, child] of nodes) if (record.element.contains(child.element) || id === message.component_id) nodes.delete(id);
      }
    }
  }

  window.__NOVA_HANDLER__ = receive;
  for (const message of window.__NOVA_QUEUE__ || []) receive(message);
  window.__NOVA_QUEUE__ = [];
  post({ protocol: 1, kind: "ready", window_id: window.__NOVA_WINDOW_ID__ });
})();
