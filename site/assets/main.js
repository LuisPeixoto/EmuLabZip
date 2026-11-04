// Small enhancements. The page works without any of this.
(() => {
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => [...r.querySelectorAll(s)];

  // Download button that names the visitor's system.
  const dl = $("#dl");
  if (dl) {
    const ua = navigator.userAgent || "";
    const platform = /Windows/i.test(ua) ? "win" : /Mac/i.test(ua) && !/iPhone|iPad/i.test(ua) ? "mac" : /Linux/i.test(ua) && !/Android/i.test(ua) ? "linux" : null;
    if (platform) {
      dl.href = dl.dataset[platform];
      $("#dl-text").textContent = dl.dataset["label" + platform[0].toUpperCase() + platform.slice(1)];
    }
  }

  // Copy buttons.
  $$("[data-copy]").forEach((btn) => {
    btn.addEventListener("click", async () => {
      const text = $(btn.dataset.copy).textContent.trim();
      try {
        await navigator.clipboard.writeText(text);
      } catch {
        const r = document.createRange();
        r.selectNodeContents($(btn.dataset.copy));
        const sel = getSelection();
        sel.removeAllRanges();
        sel.addRange(r);
        document.execCommand("copy");
        sel.removeAllRanges();
      }
      const label = $("span", btn);
      const before = label.textContent;
      label.textContent = btn.dataset.done;
      btn.classList.add("done");
      setTimeout(() => {
        label.textContent = before;
        btn.classList.remove("done");
      }, 1800);
    });
  });

  // Tabs for the screenshots (all panels are visible without JavaScript).
  const tabs = $("#tabs");
  if (tabs) {
    const buttons = $$('[role="tab"]', tabs);
    const panels = $$('[role="tabpanel"]', tabs);
    const select = (i, focus) => {
      buttons.forEach((b, j) => {
        b.setAttribute("aria-selected", String(i === j));
        b.tabIndex = i === j ? 0 : -1;
      });
      panels.forEach((p, j) => (p.hidden = i !== j));
      if (focus) buttons[i].focus();
    };
    buttons.forEach((b, i) => {
      b.addEventListener("click", () => select(i));
      b.addEventListener("keydown", (e) => {
        const n = buttons.length;
        if (e.key === "ArrowRight") select((i + 1) % n, true);
        else if (e.key === "ArrowLeft") select((i - 1 + n) % n, true);
        else if (e.key === "Home") select(0, true);
        else if (e.key === "End") select(n - 1, true);
      });
    });
    select(0);
  }

  // Cards follow the pointer with a soft light.
  $$(".card").forEach((c) => {
    c.addEventListener("pointermove", (e) => {
      const r = c.getBoundingClientRect();
      c.style.setProperty("--mx", e.clientX - r.left + "px");
      c.style.setProperty("--my", e.clientY - r.top + "px");
    });
  });

  // Reveal on scroll, and start the terminals when they come into view.
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  if ("IntersectionObserver" in window && !reduce) {
    const terms = $$(".term");
    terms.forEach((t) => t.classList.add("armed"));
    const io = new IntersectionObserver(
      (entries) => {
        entries.forEach((e) => {
          if (!e.isIntersecting) return;
          e.target.classList.add(e.target.classList.contains("term") ? "live" : "in");
          io.unobserve(e.target);
        });
      },
      { threshold: 0.25 },
    );
    $$(".reveal").forEach((el) => io.observe(el));
    terms.forEach((t) => io.observe(t));
  } else {
    $$(".reveal").forEach((el) => el.classList.add("in"));
  }

  // Close the language menu when something else is clicked.
  document.addEventListener("click", (e) => {
    const menu = $(".lang");
    if (menu && menu.open && !menu.contains(e.target)) menu.open = false;
  });

  // A hint when the browser speaks one of the other languages. Nothing is redirected.
  const hint = $("#lang-hint");
  if (hint) {
    let hints = {};
    try {
      hints = JSON.parse(hint.dataset.hints || "{}");
    } catch {}
    const want = (navigator.language || "").toLowerCase().slice(0, 2);
    const current = document.documentElement.lang.toLowerCase().slice(0, 2);
    let dismissed = false;
    try {
      dismissed = sessionStorage.getItem("lang-hint") === "1";
    } catch {}
    const target = hints[want];
    if (target && want !== current && !dismissed) {
      $("#hint-text").textContent = target.text;
      const go = $("#hint-go");
      go.textContent = target.go;
      go.href = target.href;
      go.lang = want;
      hint.hidden = false;
      $("#hint-close").textContent = target.close;
      $("#hint-close").addEventListener("click", () => {
        hint.hidden = true;
        try {
          sessionStorage.setItem("lang-hint", "1");
        } catch {}
      });
    }
  }
})();
