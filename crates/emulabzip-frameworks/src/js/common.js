var hook = globalThis.__REACT_DEVTOOLS_GLOBAL_HOOK__;

// Turns any value into something JSON can hold. It copes with cycles, functions, Maps, Sets, Dates and very large
// values, and says what it cut: at most 8 levels deep, 200 keys or items per object, 2000 characters per string and
// about 1 MB in all.
function serialize(root, maxBytes) {
  var ancestors = [], cuts = [], size = 0, limit = maxBytes || 1048576;
  function cut(path, why) { if (cuts.length < 50) cuts.push((path || '/') + ': ' + why); }
  function walk(v, depth, path) {
    var t = typeof v;
    if (v === null || t === 'boolean') { size += 5; return v; }
    if (t === 'undefined') return { $: 'undefined' };
    if (t === 'number') { size += 8; return isFinite(v) ? v : { $: 'number', value: String(v) }; }
    if (t === 'bigint') return { $: 'bigint', value: String(v) };
    if (t === 'symbol') return { $: 'symbol', value: String(v) };
    if (t === 'function') return { $: 'function', name: v.name || '' };
    if (t === 'string') {
      size += v.length + 2;
      if (v.length > 2000) { cut(path, 'string of ' + v.length + ' characters'); return v.slice(0, 2000) + '…'; }
      return v;
    }
    if (size > limit) { cut(path, 'size limit'); return { $: 'cut' }; }
    if (ancestors.indexOf(v) >= 0) return { $: 'circular' };
    if (depth >= 8) { cut(path, 'depth'); return { $: 'cut', kind: Array.isArray(v) ? 'array' : 'object' }; }
    ancestors.push(v);
    try {
      if (v instanceof Date) return { $: 'date', value: isNaN(v.getTime()) ? 'invalid' : v.toISOString() };
      if (v instanceof Error) return { $: 'error', name: v.name, message: String(v.message) };
      if (typeof ArrayBuffer !== 'undefined' && (v instanceof ArrayBuffer || ArrayBuffer.isView(v))) {
        return { $: 'bytes', length: v.byteLength };
      }
      if (typeof Map !== 'undefined' && v instanceof Map) {
        var entries = [], n = 0;
        v.forEach(function (val, key) {
          if (n++ < 200) entries.push([walk(key, depth + 1, path + '/key'), walk(val, depth + 1, path + '/' + String(key))]);
        });
        if (v.size > 200) cut(path, 'Map of ' + v.size + ' entries');
        return { $: 'map', entries: entries };
      }
      if (typeof Set !== 'undefined' && v instanceof Set) {
        var items = [], m = 0;
        v.forEach(function (val) { if (m++ < 200) items.push(walk(val, depth + 1, path + '/' + (m - 1))); });
        if (v.size > 200) cut(path, 'Set of ' + v.size + ' items');
        return { $: 'set', items: items };
      }
      if (Array.isArray(v)) {
        var out = [];
        for (var i = 0; i < v.length && i < 200; i++) out.push(walk(v[i], depth + 1, path + '/' + i));
        if (v.length > 200) cut(path, 'array of ' + v.length + ' items');
        return out;
      }
      var keys;
      try { keys = Object.keys(v); } catch (e) { return { $: 'unreadable' }; }
      var obj = {};
      for (var k = 0; k < keys.length && k < 200; k++) {
        var val;
        try { val = v[keys[k]]; } catch (e) { val = { $: 'unreadable' }; }
        obj[keys[k]] = walk(val, depth + 1, path + '/' + keys[k]);
      }
      if (keys.length > 200) cut(path, 'object with ' + keys.length + ' keys');
      return obj;
    } finally { ancestors.pop(); }
  }
  var value = walk(root, 0, '');
  return { value: value, cut: cuts };
}

function nameOf(f) {
  var t = f.type;
  if (typeof t === 'string') return t;
  if (t && (t.displayName || t.name)) return t.displayName || t.name;
  if (t && t.$$typeof) {
    var inner = t.render || t.type;
    if (inner && (inner.displayName || inner.name)) return inner.displayName || inner.name;
  }
  return null;
}

// Calls visit(fiber, nearestNamedParent) for every fiber of every root. It only reads.
function eachFiber(visit) {
  if (!hook || typeof hook.getFiberRoots !== 'function' || !hook.renderers) return;
  var count = 0;
  function walk(f, parent) {
    for (; f && count < 60000; f = f.sibling) {
      count++;
      visit(f, parent);
      walk(f.child, nameOf(f) || parent);
    }
  }
  hook.renderers.forEach(function (_, id) {
    hook.getFiberRoots(id).forEach(function (r) { walk(r.current, null); });
  });
}

function isStore(s) {
  return !!s && typeof s.getState === 'function' && typeof s.dispatch === 'function' && typeof s.subscribe === 'function';
}

// The Redux stores handed to a Provider (react-redux) as props.store, each once, with a name.
function reduxStores() {
  var found = [];
  eachFiber(function (f) {
    var p = f.memoizedProps;
    if (p && typeof p === 'object' && isStore(p.store) && found.indexOf(p.store) < 0) found.push(p.store);
  });
  return found.map(function (store, i) { return { store: store, name: i === 0 ? 'redux' : 'redux#' + (i + 1) }; });
}
