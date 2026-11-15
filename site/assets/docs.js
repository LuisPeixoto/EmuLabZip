// Documentation: the menu on small screens, search, and copy buttons on code. The pages work without any of this.
(() => {
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => [...r.querySelectorAll(s)];

  // The menu.
  const menu = $(".docs-menu");
  const nav = $("#docs-nav");
  if (menu && nav) {
    menu.addEventListener("click", () => {
      const open = nav.classList.toggle("open");
      menu.setAttribute("aria-expanded", String(open));
    });
  }

  // Copy buttons on code blocks.
  const label = document.documentElement.lang.startsWith("zh") ? "复制" : document.documentElement.lang.startsWith("pt") ? "Copiar" : document.documentElement.lang.startsWith("es") ? "Copiar" : "Copy";
  $$("pre.code").forEach((pre) => {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "copy-code";
    btn.textContent = label;
    btn.addEventListener("click", async () => {
      try {
        await navigator.clipboard.writeText($("code", pre).textContent);
        btn.textContent = "✓";
        setTimeout(() => (btn.textContent = label), 1500);
      } catch {
        // Copying is a convenience; selecting the text still works.
      }
    });
    pre.appendChild(btn);
  });

  // Search: the index is one small file per language.
  const input = $("#docs-q");
  const list = $("#docs-results");
  const root = $(".docs");
  if (!input || !list || !root) return;
  let index = null;
  let loading = null;
  const load = () => (loading ??= fetch(root.dataset.search).then((r) => r.json()).then((d) => (index = d)).catch(() => (index = [])));
  input.addEventListener("focus", load, { once: true });

  const norm = (s) => s.toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "");
  const score = (page, terms) => {
    let total = 0;
    const title = norm(page.t);
    const heads = norm(page.h.join(" "));
    const text = norm(page.x);
    for (const term of terms) {
      let s = 0;
      if (title.includes(term)) s += 10;
      if (heads.includes(term)) s += 4;
      if (text.includes(term)) s += 1;
      if (!s) return 0;
      total += s;
    }
    return total;
  };
  const snippet = (page, terms) => {
    const at = norm(page.x).indexOf(terms[0]);
    if (at < 0) return page.s;
    const from = Math.max(0, at - 40);
    return (from ? "…" : "") + page.x.slice(from, from + 110) + "…";
  };
  const show = (items, none) => {
    list.replaceChildren();
    if (!items.length) {
      const li = document.createElement("li");
      li.className = "none";
      li.textContent = none;
      list.appendChild(li);
    }
    for (const it of items) {
      const li = document.createElement("li");
      const a = document.createElement("a");
      a.href = it.page.u;
      const b = document.createElement("b");
      b.textContent = it.page.t;
      const sp = document.createElement("span");
      sp.textContent = snippet(it.page, it.terms);
      a.append(b, sp);
      li.appendChild(a);
      list.appendChild(li);
    }
    list.hidden = false;
  };
  const run = async () => {
    const q = norm(input.value.trim());
    if (q.length < 2) {
      list.hidden = true;
      return;
    }
    await load();
    const terms = q.split(/\s+/).filter(Boolean);
    const found = index
      .map((page) => ({ page, terms, s: score(page, terms) }))
      .filter((x) => x.s)
      .sort((a, b) => b.s - a.s)
      .slice(0, 8);
    show(found, input.dataset.none);
  };
  input.addEventListener("input", run);
  input.addEventListener("keydown", (e) => {
    const links = $$("a", list);
    const at = links.indexOf(document.activeElement);
    if (e.key === "ArrowDown" && links.length) {
      e.preventDefault();
      links[Math.min(at + 1, links.length - 1)].focus();
    } else if (e.key === "ArrowUp" && at > 0) {
      e.preventDefault();
      links[at - 1].focus();
    } else if (e.key === "Escape") {
      list.hidden = true;
    } else if (e.key === "Enter" && links[0]) {
      location.href = links[0].href;
    }
  });
  list.addEventListener("keydown", (e) => {
    const links = $$("a", list);
    const at = links.indexOf(document.activeElement);
    if (e.key === "ArrowDown" && at < links.length - 1) {
      e.preventDefault();
      links[at + 1].focus();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      (at > 0 ? links[at - 1] : input).focus();
    } else if (e.key === "Escape") {
      list.hidden = true;
      input.focus();
    }
  });
  document.addEventListener("click", (e) => {
    if (!e.target.closest(".docs-search")) list.hidden = true;
  });
  // "/" focuses the search, as on most documentation sites.
  document.addEventListener("keydown", (e) => {
    if (e.key === "/" && !/input|textarea/i.test(document.activeElement.tagName)) {
      e.preventDefault();
      input.focus();
    }
  });
})();
