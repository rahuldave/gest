// gest — live updates
//
// When the store changes, the server sends a `ping` on /events, and the page fetches a fresh fragment for
// #main-content. A swap would throw away what the user is typing, so the page waits while a text field in
// #main-content has focus or holds a draft, and swaps once the field is left clean.
(function () {
  if (!document.body.hasAttribute("data-live-reload")) return;

  var fragmentUrl = (function () {
    var path = window.location.pathname;
    if (path === "/") return "/_dashboard";
    if (path === "/tasks/board") return "/tasks/_board";
    var boardMatch = path.match(/^(\/iterations\/[^/]+)\/board$/);
    if (boardMatch) return boardMatch[1] + "/_board";
    var detailMatch = path.match(/^\/(tasks|artifacts|iterations)\/([^/]+)$/);
    if (detailMatch) return "/" + detailMatch[1] + "/" + detailMatch[2] + "/_detail";
    var listMatch = path.match(/^\/(tasks|artifacts|iterations)$/);
    if (listMatch) return "/" + listMatch[1] + "/_list";
    return null;
  })();

  if (!fragmentUrl) return;

  var search = window.location.search;
  if (search) fragmentUrl += search;

  // Whether a text field in `root` has focus or a value the user changed.
  function hasDraft(root) {
    var fields = root.querySelectorAll("textarea, input[type=text], input[type=search], input:not([type])");
    for (var i = 0; i < fields.length; i++) {
      var field = fields[i];
      if (field === document.activeElement || field.value !== field.defaultValue) return true;
    }
    return false;
  }

  var fetching = false;
  var pending = false;

  function refresh() {
    var main = document.getElementById("main-content");
    if (!main) return;
    if (hasDraft(main)) {
      pending = true;
      return;
    }
    // A ping while a fetch is on its way may name a change that fetch missed; fetch again after it.
    if (fetching) {
      pending = true;
      return;
    }
    pending = false;
    fetching = true;
    fetch(fragmentUrl)
      .then(function (res) { return res.text(); })
      .then(function (html) {
        var current = document.getElementById("main-content");
        // The user may have started typing while the fragment was on its way.
        if (!current || hasDraft(current)) {
          pending = true;
        } else {
          current.innerHTML = html;
          if (window.__renderMermaid) window.__renderMermaid(current);
        }
        done(true);
      })
      .catch(function () {
        // Keep the update for the next ping or focusout; retrying now would loop while the server is down.
        pending = true;
        done(false);
      });
  }

  // End a fetch. After a good one, run a refresh that a ping asked for meanwhile, unless a draft holds it back.
  function done(ok) {
    fetching = false;
    var main = document.getElementById("main-content");
    if (ok && pending && main && !hasDraft(main)) refresh();
  }

  var source = new EventSource("/events");
  source.addEventListener("ping", refresh);

  // Catch up on a skipped update once the user leaves a field with no draft in it.
  document.addEventListener("focusout", function () {
    if (!pending) return;
    setTimeout(function () { if (pending) refresh(); }, 0);
  });

  window.addEventListener("beforeunload", function () {
    source.close();
  });
})();
