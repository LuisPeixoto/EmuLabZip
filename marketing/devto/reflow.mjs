// How article text is prepared for dev.to.

/**
 * dev.to turns every single line break inside a paragraph into <br>, so text wrapped at 120 columns looks ragged. This joins the
 * lines of each paragraph and list item into one line. Code blocks, headings, tables, quotes and blank lines are left alone.
 */
export function reflow(body) {
  const out = [];
  let fence = false;
  let joinable = false; // the last output line is text that a following line may continue
  for (const line of body.split("\n")) {
    if (/^\s*(```|~~~)/.test(line)) {
      fence = !fence;
      out.push(line);
      joinable = false;
      continue;
    }
    if (fence || line.trim() === "") {
      out.push(line);
      joinable = false;
      continue;
    }
    const blockStart = /^(#{1,6}\s|\||>|\s*([-*+]|\d+[.)])\s|---+\s*$|<)/.test(line);
    if (joinable && !blockStart) out[out.length - 1] += " " + line.trim();
    else {
      out.push(line);
      joinable = !/^(#{1,6}\s|\||---+\s*$)/.test(line);
    }
  }
  return out.join("\n");
}

