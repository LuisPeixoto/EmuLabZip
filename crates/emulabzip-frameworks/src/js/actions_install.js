(function () {
  var log = globalThis.__EMULABZIP_ACTIONS__;
  if (!log) { log = globalThis.__EMULABZIP_ACTIONS__ = []; globalThis.__EMULABZIP_SEQ__ = 0; }
  var wrapped = 0;
  reduxStores().forEach(function (r) {
    var store = r.store;
    if (store.__emulabzipOriginal) return;
    var original = store.dispatch;
    store.__emulabzipOriginal = original;
    store.dispatch = function (action) {
      var result = original.apply(this, arguments);
      try {
        var seq = ++globalThis.__EMULABZIP_SEQ__;
        var type = action && typeof action === 'object' && action.type !== undefined ? String(action.type) : typeof action === 'function' ? '(thunk)' : String(action);
        log.push({ seq: seq, at: Date.now(), store: r.name, type: type, action: serialize(action, 20000).value });
        if (log.length > 200) log.splice(0, log.length - 200);
      } catch (e) {}
      return result;
    };
    wrapped++;
  });
  return JSON.stringify({ wrapped: wrapped });
})()
