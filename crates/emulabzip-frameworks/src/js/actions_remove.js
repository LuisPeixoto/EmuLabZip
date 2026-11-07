(function () {
  var restored = 0;
  reduxStores().forEach(function (r) {
    if (r.store.__emulabzipOriginal) {
      r.store.dispatch = r.store.__emulabzipOriginal;
      delete r.store.__emulabzipOriginal;
      restored++;
    }
  });
  delete globalThis.__EMULABZIP_ACTIONS__;
  delete globalThis.__EMULABZIP_SEQ__;
  return JSON.stringify({ restored: restored });
})()
