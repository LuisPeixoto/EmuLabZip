import assert from "node:assert/strict";
import { test } from "node:test";

import { cssVars } from "./apply.ts";
import { BUILTIN, DEFAULT_DARK, DEFAULT_LIGHT, baseColors, builtin } from "./builtin.ts";
import { checks, parseColor, ratio } from "./contrast.ts";
import { MAX_THEME_BYTES, fromAccent, isColor, isFontList, parse, serialize, slug, validate } from "./theme.ts";
import { COLOR_KEYS } from "./tokens.ts";

const minimal = { name: "Mine", base: "dark" };

test("colours are only ever colours", () => {
  for (const ok of ["#fff", "#ffff", "#a1b2c3", "#a1b2c3d4", "rgb(1 2 3)", "rgb(1 2 3 / 0.5)", "rgba(1, 2, 3, 0.5)", "hsl(200 50% 40%)"]) assert.ok(isColor(ok), ok);
  for (const bad of ["red", "#ggg", "#12345", "url(http://x)", "rgb(1 2 3); background: red", "var(--bg)", "rgb(1 2 3) }", "", "calc(1px)", 5, null]) assert.ok(!isColor(bad), String(bad));
});

test("a font is a list of family names and cannot carry CSS", () => {
  assert.ok(isFontList('Inter, "SF Pro Text", sans-serif'));
  assert.ok(isFontList("Menlo"));
  for (const bad of ["url(https://x/f.woff)", "Inter; color: red", "Inter } body { x", "a/b", "", "   ", "x".repeat(201), "@import 'x'", "Inter\\0041"]) assert.ok(!isFontList(bad), bad);
});

test("a minimal theme is filled in from its base", () => {
  const r = validate(minimal);
  assert.ok(r.ok);
  if (!r.ok) return;
  assert.equal(r.theme.id, "mine");
  assert.deepEqual(Object.keys(r.theme.colors).sort(), [...COLOR_KEYS].sort());
  assert.equal(r.theme.colors.bg, baseColors("dark").bg);
  assert.equal(r.theme.shape.radius, 1);
});

test("unknown keys are dropped and known ones are checked", () => {
  const r = validate({ ...minimal, colors: { bg: "#101010", nonsense: "#fff", "--evil": "red" }, extra: { a: 1 } });
  assert.ok(r.ok);
  if (!r.ok) return;
  assert.equal(r.theme.colors.bg, "#101010");
  assert.ok(!("nonsense" in r.theme.colors) && !("--evil" in r.theme.colors) && !("extra" in r.theme));
  for (const bad of [{ bg: "red" }, { bg: "url(x)" }, { fg: "#12" }, { accent: 5 }]) {
    const e = validate({ ...minimal, colors: bad });
    assert.ok(!e.ok, JSON.stringify(bad));
  }
});

test("names, ids, bases, fonts and radius are checked", () => {
  assert.ok(!validate({ base: "dark" }).ok, "needs a name");
  assert.ok(!validate({ name: "x", base: "blue" }).ok);
  assert.ok(!validate({ ...minimal, id: "Has Spaces" }).ok);
  assert.ok(!validate({ ...minimal, id: "../x" }).ok);
  assert.ok(!validate({ ...minimal, schema: 2 }).ok);
  assert.ok(!validate({ ...minimal, fonts: { sans: "url(https://evil/x)" } }).ok);
  assert.ok(!validate({ ...minimal, fonts: { mono: "a; b" } }).ok);
  assert.ok(!validate({ ...minimal, shape: { radius: 3 } }).ok);
  assert.ok(!validate({ ...minimal, shape: { radius: "round" } }).ok);
  assert.ok(!validate([]).ok && !validate(null).ok && !validate("x").ok);
  const ok = validate({ ...minimal, fonts: { sans: "Georgia, serif" }, shape: { radius: 0 } });
  assert.ok(ok.ok && ok.theme.fonts.sans === "Georgia, serif" && ok.theme.shape.radius === 0);
});

test("text fields are trimmed of control characters and length-limited", () => {
  const r = validate({ ...minimal, name: "  My\u0000 theme\n", author: "a".repeat(200), description: "d".repeat(500) });
  assert.ok(r.ok);
  if (!r.ok) return;
  assert.equal(r.theme.name, "My theme");
  assert.equal(r.theme.author?.length, 60);
  assert.equal(r.theme.description?.length, 300);
});

test("a theme file has a size limit and must be JSON", () => {
  assert.ok(!parse("not json").ok);
  assert.ok(!parse(" ".repeat(MAX_THEME_BYTES + 1)).ok);
  assert.ok(parse(JSON.stringify(minimal)).ok);
});

test("a theme survives being saved and read again", () => {
  const original = builtin("midnight")!;
  const back = parse(serialize(original));
  assert.ok(back.ok);
  if (back.ok) assert.deepEqual(back.theme, original);
});

test("slugs are ids", () => {
  assert.equal(slug("Ocean Breeze!"), "ocean-breeze");
  assert.equal(slug("  --Ça va--  "), "a-va");
  assert.equal(slug("!!!"), "");
});

test("colours are read in every form a theme may use", () => {
  assert.deepEqual(parseColor("#fff"), [255, 255, 255]);
  assert.deepEqual(parseColor("#1e222b"), [30, 34, 43]);
  assert.deepEqual(parseColor("rgb(106 77 255 / 0.1)"), [106, 77, 255]);
  assert.deepEqual(parseColor("hsl(0 100% 50%)"), [255, 0, 0]);
  assert.equal(parseColor("red"), null);
});

test("contrast matches the WCAG examples", () => {
  assert.ok(Math.abs(ratio([0, 0, 0], [255, 255, 255]) - 21) < 0.01);
  assert.ok(Math.abs(ratio([255, 255, 255], [255, 255, 255]) - 1) < 0.001);
  assert.ok(ratio([0x77, 0x77, 0x77], [255, 255, 255]) < 4.5);
  assert.ok(ratio([0x76, 0x76, 0x76], [255, 255, 255]) >= 4.5);
});

test("every built-in theme is valid, readable and complete", () => {
  const ids = new Set<string>();
  for (const t of BUILTIN) {
    assert.ok(!ids.has(t.id), `duplicate ${t.id}`);
    ids.add(t.id);
    const r = validate(JSON.parse(serialize(t)));
    assert.ok(r.ok, `${t.id}: ${r.ok ? "" : r.error}`);
    assert.deepEqual(Object.keys(t.colors).sort(), [...COLOR_KEYS].sort(), t.id);
    for (const c of checks(t)) {
      if (c.what === "Borders on panels") continue;
      assert.ok(c.ok, `${t.id}: ${c.what} is ${c.ratio?.toFixed(2)}, needs ${c.needed}`);
    }
    // Text on the window is comfortably readable, not just passing.
    const text = checks(t).find((c) => c.what === "Text on the window")!;
    assert.ok((text.ratio ?? 0) >= 7, `${t.id}: text on the window is ${text.ratio?.toFixed(2)}`);
  }
  assert.ok(builtin(DEFAULT_DARK)?.base === "dark" && builtin(DEFAULT_LIGHT)?.base === "light");
});

test("the default dark is lighter than the old near-black, and still dark", () => {
  const mid = parseColor(builtin("midnight")!.colors.bg)!;
  const old = parseColor(builtin("classic-dark")!.colors.bg)!;
  assert.ok(mid[0] > old[0] * 1.8, "Midnight's window is clearly lighter than Classic Dark's");
  assert.ok(Math.max(...mid) < 64, "but it is still a dark theme");
  const low = checks(builtin("midnight")!).find((c) => c.what === "Secondary text on panels")!;
  assert.ok((low.ratio ?? 0) >= 6, "secondary text is easy to read");
});

test("a theme made from an accent is valid, tinted and readable", () => {
  for (const base of ["dark", "light"] as const) {
    for (const accent of ["#ff6b35", "#22c55e", "#6a4dff", "#00bcd4"]) {
      const t = fromAccent("From accent", accent, base);
      const r = validate(JSON.parse(serialize(t)));
      assert.ok(r.ok, `${base} ${accent}`);
      assert.equal(t.id, "from-accent");
      const text = checks(t).find((c) => c.what === "Text on panels")!;
      assert.ok(text.ok, `${base} ${accent}: text on panels is ${text.ratio?.toFixed(2)}`);
      const muted = checks(t).find((c) => c.what === "Secondary text on panels")!;
      assert.ok(muted.ok, `${base} ${accent}: secondary text is ${muted.ratio?.toFixed(2)}`);
    }
  }
});

test("the CSS variables of a theme are its colours, fonts, roundness and shadow", () => {
  const t = builtin("midnight")!;
  const vars = cssVars(t);
  assert.equal(vars["--bg"], t.colors.bg);
  assert.equal(vars["--roundness"], "1");
  assert.equal(vars["--ui-font-mono"], t.fonts.mono);
  assert.ok(vars["--shadow"].includes("rgb(0 0 0"));
  const tuned = cssVars(t, { roundness: 0.5, fontSans: "Georgia", fontMono: null });
  assert.equal(tuned["--roundness"], "0.5");
  assert.equal(tuned["--ui-font-sans"], "Georgia");
  assert.equal(tuned["--ui-font-mono"], t.fonts.mono);
  assert.ok(cssVars(builtin("daylight")!)["--shadow"].includes("rgb(20 24 36"));
});
