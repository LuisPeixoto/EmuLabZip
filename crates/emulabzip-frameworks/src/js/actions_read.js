function (since) {
  var log = globalThis.__EMULABZIP_ACTIONS__ || [];
  return JSON.stringify({ installed: !!globalThis.__EMULABZIP_ACTIONS__, actions: log.filter(function (a) { return a.seq > since; }) });
}
