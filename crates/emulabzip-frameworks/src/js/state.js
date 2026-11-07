(function () {
  var stores = [];
  function add(kind, name, build) {
    try {
      var s = serialize(build());
      stores.push({ kind: kind, name: name, state: s.value, cut: s.cut });
    } catch (e) {
      stores.push({ kind: kind, name: name, state: null, cut: [], error: String(e && e.message || e) });
    }
  }
  var hasHook = !!(hook && typeof hook.getFiberRoots === 'function');
  if (hasHook) {
    reduxStores().forEach(function (r) { add('redux', r.name, function () { return r.store.getState(); }); });

    var clients = [], contexts = [], seen = [];
    eachFiber(function (f, parent) {
      var p = f.memoizedProps;
      if (p && typeof p === 'object' && p.client && typeof p.client.getQueryCache === 'function' && clients.indexOf(p.client) < 0) {
        clients.push(p.client);
      }
      if (f.tag === 10) {
        var ctx = f.type && (f.type._context || f.type);
        var value = p && p.value;
        if (value === undefined || value === null) return;
        if (isStore(value.store) || (value && typeof value.getQueryCache === 'function')) return;
        if (seen.indexOf(ctx) >= 0) return;
        seen.push(ctx);
        contexts.push({ name: (ctx && ctx.displayName) || (parent ? parent + ' context' : 'Context'), value: value });
      }
    });
    clients.forEach(function (client, i) {
      add('query', i === 0 ? 'react-query' : 'react-query#' + (i + 1), function () {
        return client.getQueryCache().getAll().map(function (q) {
          var st = q.state || {};
          return {
            key: q.queryKey, status: st.status, fetchStatus: st.fetchStatus, updatedAt: st.dataUpdatedAt,
            error: st.error ? String(st.error.message || st.error) : null, data: st.data
          };
        });
      });
    });
    contexts.forEach(function (c) { add('context', c.name, function () { return c.value; }); });
  }

  var named = globalThis.__EMULABZIP_STORES__;
  if (named && typeof named === 'object') {
    Object.keys(named).forEach(function (name) {
      var s = named[name];
      add('store', name, function () {
        if (s && typeof s.getState === 'function') return s.getState();
        if (s && typeof s.get === 'function') return s.get();
        return s;
      });
    });
  }
  if (!hasHook && !stores.length) return JSON.stringify({ error: 'hook' });
  return JSON.stringify({ stores: stores });
})()
