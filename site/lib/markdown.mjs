// A Markdown to HTML converter for the project's documentation. No dependencies. It handles what docs/ uses: headings,
// paragraphs, nested lists, block quotes, GFM tables, fenced code (highlighted), links, images, emphasis, inline code, and
// a short allowlist of raw HTML. Anything else is escaped, so a stray "<" can never become markup.

import { highlight } from "./highlight.mjs";

const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** GitHub's heading ids: lower case, punctuation removed, spaces become hyphens. */
export function slugify(text) {
  return text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s_-]/gu, "")
    .trim()
    .replace(/\s/g, "-");
}

const ALLOWED_TAGS = new Set(["kbd", "br", "sup", "sub", "b", "i", "em", "strong", "code", "p", "details", "summary", "img", "a"]);

/** Text of inline markup, as plain text (for tables of contents, titles and search). */
export function plain(md) {
  return md
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/<[^>]+>/g, "")
    .replace(/[`*_~]/g, "")
    .replace(/\\(.)/g, "$1")
    .replace(/\s+/g, " ")
    .trim();
}

/**
 * Converts `source`. Options:
 *   link(href, text)            the href to use for a link (default: unchanged)
 *   image(src, alt)             {src, srcset?, width?, height?} for an image, or null to drop it
 * Returns { html, headings: [{level, id, text}], title, summary, paragraphs } (paragraphs: the plain text of the top-level ones).
 */
export function render(source, options = {}) {
  const ctx = { options, headings: [], ids: new Map(), summary: "", title: "", paragraphs: [] };
  const lines = source.replace(/\r\n?/g, "\n").replace(/\t/g, "    ").split("\n");
  const html = blocks(lines, ctx, true);
  return { html, headings: ctx.headings, title: ctx.title, summary: ctx.summary, paragraphs: ctx.paragraphs };
}

// ---------------------------------------------------------------------------------------------
// Blocks
// ---------------------------------------------------------------------------------------------

const isBlank = (l) => /^\s*$/.test(l);
const FENCE = /^(\s*)(`{3,}|~{3,})\s*([\w+-]*)[^\n]*$/;
const HEADING = /^(#{1,6})\s+(.*?)\s*#*\s*$/;
const HR = /^\s{0,3}([-*_])(?:\s*\1){2,}\s*$/;
const BULLET = /^(\s*)([-*+]|\d+[.)])\s+(.*)$/;
const TABLE_SEP = /^\s*\|?\s*:?-{1,}:?\s*(\|\s*:?-{1,}:?\s*)*\|?\s*$/;
const HTML_BLOCK = /^\s*<(p|details|summary|div|img|table|center)\b/i;

function blocks(lines, ctx, top) {
  const out = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (isBlank(line)) {
      i++;
      continue;
    }

    const fence = FENCE.exec(line);
    if (fence) {
      const [, indent, marker, lang] = fence;
      const body = [];
      i++;
      while (i < lines.length && !new RegExp(`^\\s*${marker[0]}{${marker.length},}\\s*$`).test(lines[i])) {
        body.push(lines[i].startsWith(indent) ? lines[i].slice(indent.length) : lines[i].trimStart());
        i++;
      }
      i++;
      const code = body.join("\n");
      out.push(`<pre class="code"${lang ? ` data-lang="${esc(lang)}"` : ""}><code>${highlight(code, lang)}</code></pre>`);
      continue;
    }

    const heading = HEADING.exec(line);
    if (heading) {
      const level = heading[1].length;
      const raw = heading[2];
      const text = plain(raw);
      let id = slugify(text) || "section";
      const n = ctx.ids.get(id) ?? 0;
      ctx.ids.set(id, n + 1);
      if (n) id = `${id}-${n}`;
      if (level === 1 && !ctx.title) ctx.title = text;
      if (level === 1 && top) {
        // The page's own title is rendered by the template, so it is not repeated here.
        ctx.headings.push({ level, id, text });
        i++;
        continue;
      }
      ctx.headings.push({ level, id, text });
      out.push(`<h${level} id="${id}">${inline(raw, ctx)}<a class="h-anchor" href="#${id}" aria-label="${esc(text)}">#</a></h${level}>`);
      i++;
      continue;
    }

    if (HR.test(line)) {
      out.push("<hr>");
      i++;
      continue;
    }

    if (HTML_BLOCK.test(line)) {
      const body = [];
      while (i < lines.length && !isBlank(lines[i])) body.push(lines[i++]);
      out.push(rawHtml(body.join("\n"), ctx));
      continue;
    }

    if (/^\s{0,3}>/.test(line)) {
      const body = [];
      while (i < lines.length && /^\s{0,3}>/.test(lines[i])) body.push(lines[i++].replace(/^\s{0,3}>\s?/, ""));
      out.push(`<blockquote>${blocks(body, ctx, false)}</blockquote>`);
      continue;
    }

    if (line.includes("|") && i + 1 < lines.length && TABLE_SEP.test(lines[i + 1]) && lines[i + 1].includes("-")) {
      const rows = [];
      while (i < lines.length && !isBlank(lines[i]) && lines[i].includes("|")) rows.push(lines[i++]);
      out.push(table(rows, ctx));
      continue;
    }

    const bullet = BULLET.exec(line);
    if (bullet) {
      const { html, next } = list(lines, i, ctx);
      out.push(html);
      i = next;
      continue;
    }

    // A paragraph runs until a blank line or the start of another block.
    const para = [];
    while (i < lines.length && !isBlank(lines[i]) && (para.length === 0 || !startsBlock(lines, i))) para.push(lines[i++].trim());
    const text = para.join("\n");
    if (top && !/^\s*[|!<]/.test(text)) {
      if (!ctx.summary) ctx.summary = plain(text);
      if (ctx.paragraphs.length < 6) ctx.paragraphs.push(plain(text));
    }
    out.push(`<p>${inline(text, ctx)}</p>`);
  }
  return out.join("\n");
}

function startsBlock(lines, i) {
  const l = lines[i];
  return FENCE.test(l) || HEADING.test(l) || HR.test(l) || /^\s{0,3}>/.test(l) || HTML_BLOCK.test(l) || BULLET.test(l) ||
    (l.includes("|") && i + 1 < lines.length && TABLE_SEP.test(lines[i + 1]) && lines[i + 1].includes("-"));
}

function list(lines, start, ctx) {
  const first = BULLET.exec(lines[start]);
  const ordered = /\d/.test(first[2]);
  const baseIndent = first[1].length;
  const items = [];
  let loose = false;
  let i = start;
  while (i < lines.length) {
    const m = BULLET.exec(lines[i]);
    if (!m || m[1].length !== baseIndent || /\d/.test(m[2]) !== ordered) break;
    const contentIndent = baseIndent + m[2].length + 1;
    const body = [m[3]];
    i++;
    for (;;) {
      if (i >= lines.length) break;
      const l = lines[i];
      if (isBlank(l)) {
        // A blank line continues the item only when what follows is indented under it.
        let j = i;
        while (j < lines.length && isBlank(lines[j])) j++;
        if (j < lines.length && indentOf(lines[j]) >= contentIndent) {
          for (; i < j; i++) body.push("");
          continue;
        }
        // Items separated by a blank line make a loose list, still one list.
        const next = j < lines.length ? BULLET.exec(lines[j]) : null;
        if (next && next[1].length === baseIndent && /\d/.test(next[2]) === ordered) {
          loose = true;
          i = j;
        }
        break;
      }
      if (indentOf(l) >= contentIndent) {
        body.push(l.slice(contentIndent));
        i++;
      } else if (BULLET.test(l) && indentOf(l) <= baseIndent) {
        break;
      } else if (startsBlock(lines, i) && indentOf(l) <= baseIndent) {
        break;
      } else {
        // A wrapped line of the item's own paragraph.
        body.push(l.trim());
        i++;
      }
    }
    items.push(body);
  }
  const tag = ordered ? "ol" : "ul";
  const startAttr = ordered && Number.parseInt(first[2], 10) !== 1 ? ` start="${Number.parseInt(first[2], 10)}"` : "";
  const li = items.map((body) => {
    const inner = blocks(body, ctx, false);
    // A tight item shows its text without the paragraph around it.
    const simple = !loose && body.every((l) => !isBlank(l)) ? inner.replace(/^<p>([\s\S]*?)<\/p>/, "$1") : inner;
    return `<li>${simple}</li>`;
  });
  return { html: `<${tag}${startAttr}>${li.join("")}</${tag}>`, next: i };
}

const indentOf = (l) => l.length - l.trimStart().length;

function splitRow(row) {
  let r = row.trim();
  if (r.startsWith("|")) r = r.slice(1);
  if (r.endsWith("|") && !r.endsWith("\\|")) r = r.slice(0, -1);
  const cells = [];
  let cur = "";
  let inCode = false;
  for (let k = 0; k < r.length; k++) {
    const c = r[k];
    if (c === "`") inCode = !inCode;
    if (c === "\\" && r[k + 1] === "|") {
      cur += "|";
      k++;
    } else if (c === "|" && !inCode) {
      cells.push(cur.trim());
      cur = "";
    } else cur += c;
  }
  cells.push(cur.trim());
  return cells;
}

function table(rows, ctx) {
  const head = splitRow(rows[0]);
  const aligns = splitRow(rows[1]).map((c) => (c.startsWith(":") && c.endsWith(":") ? "c" : c.endsWith(":") ? "r" : ""));
  const cls = (n) => (aligns[n] ? ` class="al-${aligns[n]}"` : "");
  const th = head.map((c, n) => `<th${cls(n)}>${inline(c, ctx)}</th>`).join("");
  const body = rows
    .slice(2)
    .map((r) => {
      const cells = splitRow(r);
      return `<tr>${head.map((_, n) => `<td${cls(n)}>${inline(cells[n] ?? "", ctx)}</td>`).join("")}</tr>`;
    })
    .join("");
  return `<div class="tbl"><table><thead><tr>${th}</tr></thead><tbody>${body}</tbody></table></div>`;
}

// ---------------------------------------------------------------------------------------------
// Raw HTML: only the allowlist survives, with a few attributes
// ---------------------------------------------------------------------------------------------

function rawHtml(src, ctx) {
  return src.replace(/<(\/?)([A-Za-z][\w-]*)([^>]*?)(\/?)>|([^<]+)|</g, (m, close, tag, attrs, self, text) => {
    if (text !== undefined) return esc(text).replace(/&amp;(\w+|#\d+);/g, "&$1;");
    if (!tag) return "&lt;";
    return tagHtml(close, tag.toLowerCase(), attrs, ctx) ?? esc(m);
  });
}

/** The safe form of a tag, or null when it is not allowed. */
function tagHtml(close, tag, attrs, ctx) {
  if (tag === "center" || tag === "div") return close ? "</p>" : '<p class="center">';
  if (!ALLOWED_TAGS.has(tag)) return null;
  if (close) return tag === "img" || tag === "br" ? "" : `</${tag}>`;
  const attr = (name) => new RegExp(`\\b${name}\\s*=\\s*("([^"]*)"|'([^']*)')`, "i").exec(attrs)?.slice(2).find((v) => v !== undefined);
  if (tag === "br") return "<br>";
  if (tag === "img") return imageHtml(attr("src") ?? "", attr("alt") ?? "", ctx);
  if (tag === "p") return /align\s*=\s*["']?center/i.test(attrs) ? '<p class="center">' : "<p>";
  if (tag === "a") return `<a href="${esc(linkHref(attr("href") ?? "", "", ctx))}">`;
  return `<${tag}>`;
}

function imageHtml(src, alt, ctx) {
  const made = ctx.options.image ? ctx.options.image(src, alt) : { src };
  if (!made) return "";
  const srcset = made.srcset ? ` srcset="${esc(made.srcset)}" sizes="(max-width: 900px) 94vw, 820px"` : "";
  const size = made.width && made.height ? ` width="${made.width}" height="${made.height}"` : "";
  return `<img src="${esc(made.src)}"${srcset} alt="${esc(alt)}"${size} loading="lazy" decoding="async">`;
}

function linkHref(href, text, ctx) {
  return ctx.options.link ? ctx.options.link(href, text) : href;
}

// ---------------------------------------------------------------------------------------------
// Inline
// ---------------------------------------------------------------------------------------------

const PUNCT = /[\\`*_{}\[\]()#+\-.!|<>~"'&$%:;,?@/=^]/;

function inline(text, ctx) {
  let out = "";
  let i = 0;
  const n = text.length;
  while (i < n) {
    const c = text[i];

    if (c === "\\" && i + 1 < n && PUNCT.test(text[i + 1])) {
      out += esc(text[i + 1]);
      i += 2;
      continue;
    }

    if (c === "\n") {
      out += "\n";
      i++;
      continue;
    }

    if (c === "`") {
      let run = 1;
      while (text[i + run] === "`") run++;
      const close = new RegExp("(?<!`)`{" + run + "}(?!`)").exec(text.slice(i + run));
      if (close) {
        const code = text.slice(i + run, i + run + close.index).replace(/\n/g, " ");
        out += `<code>${esc(code.length > 1 && code.startsWith(" ") && code.endsWith(" ") ? code.slice(1, -1) : code)}</code>`;
        i += run + close.index + run;
        continue;
      }
      out += "`".repeat(run);
      i += run;
      continue;
    }

    if (c === "!" && text[i + 1] === "[") {
      const parsed = linkAt(text, i + 1);
      if (parsed) {
        out += imageHtml(parsed.href, plain(parsed.label), ctx);
        i = parsed.end;
        continue;
      }
    }

    if (c === "[") {
      const parsed = linkAt(text, i);
      if (parsed) {
        const label = inline(parsed.label, ctx);
        const href = linkHref(parsed.href, plain(parsed.label), ctx);
        out += href === null ? label : `<a href="${esc(href)}"${/^https?:/.test(href) ? ' rel="noopener"' : ""}>${label}</a>`;
        i = parsed.end;
        continue;
      }
    }

    if (c === "<") {
      const auto = /^<(https?:\/\/[^\s>]+)>/.exec(text.slice(i));
      if (auto) {
        out += `<a href="${esc(auto[1])}" rel="noopener">${esc(auto[1])}</a>`;
        i += auto[0].length;
        continue;
      }
      const tag = /^<(\/?)([A-Za-z][\w-]*)([^<>]*?)(\/?)>/.exec(text.slice(i));
      if (tag && ALLOWED_TAGS.has(tag[2].toLowerCase()) && !["p", "details", "summary"].includes(tag[2].toLowerCase())) {
        out += tagHtml(tag[1], tag[2].toLowerCase(), tag[3], ctx) ?? esc(tag[0]);
        i += tag[0].length;
        continue;
      }
      out += "&lt;";
      i++;
      continue;
    }

    if ((c === "*" || c === "_") && !/\s/.test(text[i + 1] ?? " ")) {
      const strong = text.slice(i, i + 2) === c + c;
      const mark = strong ? c + c : c;
      const wordy = c === "_" && /[\p{L}\p{N}]/u.test(text[i - 1] ?? " ");
      if (!wordy) {
        const end = findClose(text, i + mark.length, mark, c === "_");
        if (end > i + mark.length) {
          const tag = strong ? "strong" : "em";
          out += `<${tag}>${inline(text.slice(i + mark.length, end), ctx)}</${tag}>`;
          i = end + mark.length;
          continue;
        }
      }
    }

    if (c === "~" && text[i + 1] === "~") {
      const end = text.indexOf("~~", i + 2);
      if (end > i + 2) {
        out += `<del>${inline(text.slice(i + 2, end), ctx)}</del>`;
        i = end + 2;
        continue;
      }
    }

    if (c === "&") {
      const ent = /^&(?:#\d+|#x[\da-fA-F]+|[A-Za-z][\w]*);/.exec(text.slice(i));
      if (ent) {
        out += ent[0];
        i += ent[0].length;
        continue;
      }
      out += "&amp;";
      i++;
      continue;
    }

    out += c === ">" ? "&gt;" : c === '"' ? "&quot;" : c;
    i++;
  }
  return out;
}

/** The index where `mark` closes, skipping code spans, or -1. */
function findClose(text, from, mark, underscore) {
  let i = from;
  while (i < text.length) {
    if (text[i] === "`") {
      const e = text.indexOf("`", i + 1);
      i = e < 0 ? text.length : e + 1;
      continue;
    }
    if (text[i] === "\\") {
      i += 2;
      continue;
    }
    if (text.startsWith(mark, i) && !/\s/.test(text[i - 1] ?? " ")) {
      const after = text[i + mark.length] ?? " ";
      if (!(underscore && /[\p{L}\p{N}]/u.test(after)) && !(mark.length === 1 && text[i + 1] === mark)) return i;
    }
    i++;
  }
  return -1;
}

/** A `[label](href "title")` at `at`, with the index after it, or null. */
function linkAt(text, at) {
  let depth = 0;
  let i = at;
  for (; i < text.length; i++) {
    const c = text[i];
    if (c === "\\") i++;
    else if (c === "`") {
      const e = text.indexOf("`", i + 1);
      if (e < 0) return null;
      i = e;
    } else if (c === "[") depth++;
    else if (c === "]") {
      depth--;
      if (depth === 0) break;
    }
  }
  if (i >= text.length || text[i + 1] !== "(") return null;
  const label = text.slice(at + 1, i);
  let j = i + 2;
  let parens = 1;
  for (; j < text.length; j++) {
    if (text[j] === "\\") j++;
    else if (text[j] === "(") parens++;
    else if (text[j] === ")" && --parens === 0) break;
  }
  if (j >= text.length) return null;
  const inner = text.slice(i + 2, j).trim();
  const href = inner.replace(/\s+"[^"]*"$/, "").replace(/^<(.*)>$/, "$1");
  return { label, href, end: j + 1 };
}
