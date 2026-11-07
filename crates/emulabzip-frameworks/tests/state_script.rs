//! Runs the state scripts in Node against a fake React DevTools hook, the way Hermes would run them. Skipped when
//! Node is not installed.

use std::io::Write;
use std::process::{Command, Stdio};

use emulabzip_frameworks::react_native::{
    actions_install_script, actions_read_script, actions_remove_script, parse_actions, parse_state, state_script,
};

const FAKE_APP: &str = r#"
function makeStore(state) {
  const s = { state, dispatch(a) { if (a && a.type === 'add') s.state = { ...s.state, n: s.state.n + 1 }; return a; },
    getState() { return s.state; }, subscribe() { return () => {}; } };
  return s;
}
const store = makeStore({ n: 1, big: Array.from({ length: 500 }, (_, i) => i) });
const cyc = { name: 'loop' }; cyc.self = cyc;
store.state.cyc = cyc;
store.state.fn = function pick() {};
store.state.map = new Map([['a', 1]]);
store.state.set = new Set([1, 2]);
store.state.when = new Date(0);
store.state.deep = { a: { b: { c: { d: { e: { f: { g: { h: { i: 1 } } } } } } } } };
const client = { getQueryCache: () => ({ getAll: () => [{ queryKey: ['time'], state: { status: 'success', fetchStatus: 'idle', dataUpdatedAt: 5, data: { secs: 7 } } }] }) };
const Theme = { displayName: 'ThemeContext' };
const fibers = {
  tag: 0, type: function App() {}, memoizedProps: {}, sibling: null,
  child: { tag: 0, type: function Provider() {}, memoizedProps: { store }, sibling: null,
    child: { tag: 0, type: function QueryClientProvider() {}, memoizedProps: { client }, sibling: null,
      child: { tag: 10, type: { _context: Theme }, memoizedProps: { value: { mode: 'dark' } }, sibling: null,
        child: { tag: 10, type: { _context: { displayName: 'ReduxCtx' } }, memoizedProps: { value: { store } }, sibling: null, child: null } } } } };
globalThis.__REACT_DEVTOOLS_GLOBAL_HOOK__ = { renderers: new Map([[1, {}]]), getFiberRoots: () => new Set([{ current: fibers }]) };
globalThis.__EMULABZIP_STORES__ = { cart: { getState: () => ({ items: 2 }) }, broken: { getState() { throw new Error('nope'); } } };
const run = (src) => (0, eval)(src);
const out = {};
out.state = run(process.env.STATE);
out.before = run(process.env.READ);
out.wrapped = run(process.env.INSTALL);
store.dispatch({ type: 'add', big: 'x'.repeat(10) });
store.dispatch({ type: 'other' });
out.after = run(process.env.READ);
out.leaks = typeof serialize + typeof reduxStores + typeof hook;
out.removed = run(process.env.REMOVE);
out.gone = typeof globalThis.__EMULABZIP_ACTIONS__;
out.restored = store.dispatch === undefined ? 'none' : String(store.__emulabzipOriginal);
console.log(JSON.stringify(out));
"#;

#[test]
fn the_scripts_read_state_and_log_actions() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("node is not installed; skipped");
        return;
    }
    let mut child = Command::new("node")
        .arg("-e")
        .arg(FAKE_APP)
        .env("STATE", state_script())
        .env("READ", actions_read_script(0))
        .env("INSTALL", actions_install_script())
        .env("REMOVE", actions_remove_script())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child.stdin.take().map(|mut s| s.flush());
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();

    let state = parse_state(v["state"].as_str().unwrap()).unwrap();
    let names: Vec<_> = state.stores.iter().map(|s| format!("{}:{}", s.kind, s.name)).collect();
    assert_eq!(
        names,
        ["redux:redux", "query:react-query", "context:ThemeContext", "store:cart", "store:broken"],
        "the Redux context is not listed twice"
    );
    let redux = &state.stores[0];
    assert_eq!(redux.state["n"], 1);
    assert_eq!(redux.state["big"].as_array().unwrap().len(), 200);
    assert_eq!(redux.state["cyc"]["self"]["$"], "circular");
    assert_eq!(redux.state["fn"]["$"], "function");
    assert_eq!(redux.state["map"]["entries"][0][1], 1);
    assert_eq!(redux.state["set"]["items"].as_array().unwrap().len(), 2);
    assert_eq!(redux.state["when"]["value"], "1970-01-01T00:00:00.000Z");
    assert!(redux.cut.iter().any(|c| c.contains("array of 500")) && redux.cut.iter().any(|c| c.contains("depth")));
    assert_eq!(state.stores[1].state[0]["data"]["secs"], 7);
    assert_eq!(state.stores[2].state["mode"], "dark");
    assert_eq!(state.stores[3].state["items"], 2);
    assert_eq!(state.stores[4].error.as_deref(), Some("nope"));

    let (installed, none) = parse_actions(v["before"].as_str().unwrap());
    assert!(!installed && none.is_empty());
    assert_eq!(serde_json::from_str::<serde_json::Value>(v["wrapped"].as_str().unwrap()).unwrap()["wrapped"], 1);
    let (installed, actions) = parse_actions(v["after"].as_str().unwrap());
    assert!(installed);
    let types: Vec<_> = actions.iter().map(|a| a.kind.as_str()).collect();
    assert_eq!(types, ["add", "other"]);
    assert_eq!(actions[1].seq, 2);

    assert_eq!(v["leaks"], "undefinedundefinedundefined", "the helpers stay out of the page's global scope");
    assert_eq!(v["gone"], "undefined");
    assert_eq!(v["restored"], "undefined");
}
