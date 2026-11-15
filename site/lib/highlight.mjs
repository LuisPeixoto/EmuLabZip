// A small syntax highlighter for the languages the documentation uses. It returns HTML with <span class="t-…"> markup:
// c comment, s string, k keyword, n number, f flag, v variable, t key or tag, a attribute, p prompt, h heading.

const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const wrap = (cls, text) => `<span class="t-${cls}">${esc(text)}</span>`;

/** Runs `rules` (regex, class or function) over `src`, escaping what no rule matches. */
function scan(src, rules) {
  let out = "";
  let i = 0;
  const sticky = rules.map(([re, cls]) => [new RegExp(re.source, "y" + re.flags.replace(/[gy]/g, "")), cls]);
  outer: while (i < src.length) {
    for (const [re, cls] of sticky) {
      re.lastIndex = i;
      const m = re.exec(src);
      if (m && m[0].length) {
        out += typeof cls === "function" ? cls(m) : wrap(cls, m[0]);
        i += m[0].length;
        continue outer;
      }
    }
    out += esc(src[i]);
    i++;
  }
  return out;
}

const words = (list) => new RegExp(`\\b(?:${list.join("|")})\\b`);
const STRING_DQ = /"(?:[^"\\\n]|\\.)*"?/;
const STRING_SQ = /'(?:[^'\\\n]|\\.)*'?/;
const NUMBER = /\b\d[\d_]*(?:\.\d+)?(?:[eE][+-]?\d+)?\b/;

const languages = {
  bash: (src) =>
    src
      .split("\n")
      .map((line) =>
        scan(line, [
          [/^\s*#.*/, "c"],
          [/\s#.*/, (m) => wrap("c", m[0])],
          [/^\$ /, "p"],
          [/^(?:PS>|>) /, "p"],
          [STRING_DQ, "s"],
          [STRING_SQ, "s"],
          [/\$\{[^}]*\}|\$[A-Za-z_][\w]*|\$\(/, "v"],
          [/(?<=^|\s|=)--?[A-Za-z][\w-]*/, "f"],
          [words(["if", "then", "else", "fi", "for", "do", "done", "in", "case", "esac", "while", "export"]), "k"],
          [NUMBER, "n"],
        ]),
      )
      .join("\n"),

  json: (src) =>
    scan(src, [
      [/"(?:[^"\\\n]|\\.)*"(?=\s*:)/, "t"],
      [STRING_DQ, "s"],
      [/-?\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b/, "n"],
      [/\b(?:true|false|null)\b/, "k"],
    ]),

  toml: (src) =>
    src
      .split("\n")
      .map((line) =>
        scan(line, [
          [/^\s*#.*/, "c"],
          [/\s#.*/, (m) => wrap("c", m[0])],
          [/^\s*\[\[?[^\]\n]+\]\]?/, "h"],
          [/^\s*[A-Za-z_][\w.-]*(?=\s*=)/, "t"],
          [STRING_DQ, "s"],
          [STRING_SQ, "s"],
          [NUMBER, "n"],
          [/\b(?:true|false)\b/, "k"],
        ]),
      )
      .join("\n"),

  yaml: (src) =>
    src
      .split("\n")
      .map((line) =>
        scan(line, [
          [/^\s*#.*/, "c"],
          [/\s#.*/, (m) => wrap("c", m[0])],
          [/^\s*(?:- )?[A-Za-z_][\w.\/-]*(?=:)/, "t"],
          [STRING_DQ, "s"],
          [STRING_SQ, "s"],
          [/\$\{\{[^}]*\}\}/, "v"],
          [NUMBER, "n"],
          [/\b(?:true|false|null)\b/, "k"],
        ]),
      )
      .join("\n"),

  xml: (src) =>
    scan(src, [
      [/<!--[\s\S]*?(?:-->|$)/, "c"],
      [/<\/?[A-Za-z][\w:.-]*/, "t"],
      [/\/?>/, "t"],
      [/[A-Za-z_:][\w:.-]*(?==)/, "a"],
      [STRING_DQ, "s"],
      [STRING_SQ, "s"],
    ]),
};

const cLike = (keywords) => (src) =>
  scan(src, [
    [/\/\/.*/, "c"],
    [/\/\*[\s\S]*?(?:\*\/|$)/, "c"],
    [STRING_DQ, "s"],
    [/'(?:[^'\\\n]|\\.){1,2}'/, "s"],
    [/`(?:[^`\\]|\\.)*`?/, "s"],
    [words(keywords), "k"],
    [/\b0x[\da-fA-F_]+\b|\b\d[\d_]*(?:\.\d+)?\b/, "n"],
  ]);

languages.ts = cLike(["const", "let", "var", "function", "return", "if", "else", "for", "while", "import", "from", "export", "type", "interface", "async", "await", "new", "class", "true", "false", "null", "undefined"]);
languages.rust = cLike(["fn", "let", "mut", "pub", "struct", "enum", "impl", "use", "mod", "match", "if", "else", "for", "while", "loop", "return", "self", "Self", "const", "static", "trait", "where", "async", "await", "true", "false", "as", "in", "move", "ref"]);
languages.kotlin = cLike(["fun", "val", "var", "class", "object", "interface", "if", "else", "for", "while", "return", "import", "package", "when", "is", "in", "null", "true", "false", "override", "private", "public", "suspend", "data", "companion"]);
languages.js = languages.ts;
languages.json5 = languages.json;
languages.sh = languages.bash;
languages.shell = languages.bash;
languages.html = languages.xml;

/** `src` as highlighted HTML for `lang`, or plain escaped text when the language is not known. */
export function highlight(src, lang) {
  const run = languages[(lang || "").toLowerCase()];
  return run ? run(src) : esc(src);
}

export const knownLanguages = Object.keys(languages);
