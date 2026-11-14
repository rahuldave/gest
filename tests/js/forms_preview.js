// Loads templates/forms.js against a minimal DOM, then adds a preview button and its textarea *after* load, as
// the live update's _detail fragment swap does, and clicks it. Exits non-zero when the click does nothing.
// Usage: node tests/js/forms_preview.js templates/forms.js
"use strict";
const fs = require("fs");
const vm = require("vm");

const clickHandlers = [];
const elements = {};
function element(id) {
  const classes = new Set();
  return {
    id,
    style: {},
    value: "the draft",
    innerHTML: "",
    classList: { contains: (c) => classes.has(c), add: (c) => classes.add(c), remove: (c) => classes.delete(c) },
  };
}
const document = {
  addEventListener: (type, handler) => type === "click" && clickHandlers.push(handler),
  getElementById: (id) => elements[id] || null,
  querySelector: (selector) => (selector === 'meta[name="csrf-token"]' ? { content: "the-token" } : null),
  querySelectorAll: () => [],
};
let request = null;
const context = {
  document,
  window: {},
  URLSearchParams,
  console,
  fetch: (url, options) => {
    request = { url, options };
    return Promise.resolve({ text: () => Promise.resolve("<p>rendered</p>") });
  },
};
vm.createContext(context);
vm.runInContext(fs.readFileSync(process.argv[2], "utf8"), context);

// The live update swaps in a new fragment after load.
elements["note-body"] = element("note-body");
elements["note-body-preview"] = element("note-body-preview");
const button = { dataset: { target: "note-body" }, textContent: "Preview" };
button.closest = (selector) => (selector === ".md-preview-btn" ? button : null);
const click = (target) => clickHandlers.forEach((handler) => handler({ target, preventDefault() {} }));
const fail = (why) => {
  console.error(why);
  process.exit(1);
};

// A click elsewhere on the page must not preview anything.
click({ closest: () => null });
if (request) fail("a click outside a preview button sent a request");

click(button);

setTimeout(() => {
  if (!request) fail("clicking a preview button added after load sent no request");
  if (request.url !== "/api/render-markdown") fail(`posted to ${request.url}`);
  if (!request.options.body.includes("_csrf=the-token")) fail("the request carried no CSRF token");
  if (elements["note-body-preview"].innerHTML !== "<p>rendered</p>") fail("the preview was not filled");
  if (button.textContent !== "Edit") fail("the button did not toggle to Edit");

  // A second click goes back to the textarea.
  click(button);
  if (button.textContent !== "Preview") fail("the button did not toggle back to Preview");
  if (elements["note-body"].style.display !== "") fail("the textarea did not show again");
  if (elements["note-body-preview"].classList.contains("active")) fail("the preview stayed active");
  console.log("ok");
}, 50);
