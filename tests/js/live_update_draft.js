// Loads templates/live.js against a minimal DOM on a task detail page, sends live-update pings, and checks that
// a note being typed survives: no swap while a field has focus or a draft, and one swap once the field is left
// clean. Exits non-zero on the first failure.
// Usage: node tests/js/live_update_draft.js templates/live.js
"use strict";
const fs = require("fs");
const vm = require("vm");

const fail = (why) => {
  console.error(why);
  process.exit(1);
};

const textarea = { value: "", defaultValue: "" };
const input = { value: "", defaultValue: "" };
const main = {
  innerHTML: "<old>",
  // live.js asks for text fields with one selector list; answer each part of it.
  querySelectorAll: (selector) => {
    const parts = selector.split(",").map((part) => part.trim());
    const found = [];
    if (parts.includes("textarea")) found.push(textarea);
    if (parts.includes("input[type=text]")) found.push(input);
    return found;
  },
};
const listeners = {};
const document = {
  activeElement: null,
  body: { hasAttribute: (name) => name === "data-live-reload" },
  getElementById: (id) => (id === "main-content" ? main : null),
  addEventListener: (type, handler) => (listeners[type] = handler),
};
let ping = null;
let fetches = 0;
let respond = null; // when set, the next fetch waits for this to be called
let failNext = false;
const context = {
  document,
  window: { location: { pathname: "/tasks/abcd", search: "" }, addEventListener: () => {} },
  EventSource: function () {
    this.addEventListener = (type, handler) => type === "ping" && (ping = handler);
  },
  fetch: (url) => {
    fetches += 1;
    if (url !== "/tasks/abcd/_detail") fail(`fetched ${url}`);
    if (failNext) {
      failNext = false;
      return Promise.reject(new Error("server down"));
    }
    const body = `<new ${fetches}>`;
    if (respond === "hold") {
      return new Promise((resolve) => {
        respond = () => resolve({ text: () => Promise.resolve(body) });
      });
    }
    return Promise.resolve({ text: () => Promise.resolve(body) });
  },
  setTimeout,
};
vm.createContext(context);
vm.runInContext(fs.readFileSync(process.argv[2], "utf8"), context);
if (!ping) fail("live.js did not listen for pings");

const settle = () => new Promise((resolve) => setTimeout(resolve, 10));

(async () => {
  // The user is typing a note: the textarea has focus and a draft.
  document.activeElement = textarea;
  textarea.value = "half a note";
  ping();
  await settle();
  if (fetches !== 0 || main.innerHTML !== "<old>") fail("a ping replaced the page while a note was being typed");

  // The field keeps a draft but loses focus: still no swap.
  document.activeElement = null;
  listeners.focusout();
  await settle();
  if (main.innerHTML !== "<old>") fail("leaving a field that still holds a draft replaced the page");

  // The draft is cleared and the field is left: the skipped update arrives.
  textarea.value = "";
  listeners.focusout();
  await settle();
  if (!main.innerHTML.startsWith("<new")) fail("the skipped update never arrived after the draft was cleared");

  // With no draft, a ping swaps at once.
  main.innerHTML = "<old>";
  ping();
  await settle();
  if (!main.innerHTML.startsWith("<new")) fail("a ping with no draft did not replace the page");

  // A text input with a draft holds the swap back too.
  main.innerHTML = "<old>";
  input.value = "filter";
  ping();
  await settle();
  if (main.innerHTML !== "<old>") fail("a ping replaced the page while a text input held a draft");
  input.value = "";

  // The user starts typing while the fragment is on its way: the fragment is not applied.
  respond = "hold";
  ping();
  await settle();
  textarea.value = "typed meanwhile";
  respond();
  respond = null;
  await settle();
  if (main.innerHTML !== "<old>") fail("a fragment fetched before the user typed replaced the draft");
  textarea.value = "";

  // A ping during a fetch is not lost: a second fetch follows the first.
  respond = "hold";
  ping();
  await settle();
  const before = fetches;
  ping();
  respond();
  respond = null;
  await settle();
  if (fetches !== before + 1) fail(`a ping during a fetch was lost (${fetches - before} more fetches)`);

  // A failed fetch does not retry in a loop, and the next ping fetches again.
  failNext = true;
  const atFailure = fetches;
  ping();
  await settle();
  if (fetches !== atFailure + 1) fail("a failed fetch retried at once");
  main.innerHTML = "<old>";
  ping();
  await settle();
  if (!main.innerHTML.startsWith("<new")) fail("the ping after a failed fetch did not update the page");

  console.log("ok");
})();
