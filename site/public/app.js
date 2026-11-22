(() => {
  "use strict";
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
  const esc = (s) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

  /* ---------- syntax highlighting (Kotlin, Java, Swift, shell) ---------- */
  const KW = {
    kotlin: "val var fun if else when is as in return import package class object interface try catch finally throw null true false use",
    java: "new try catch finally return import class public static final null true false",
    swift: "import let var func if else switch case return try throw nil true false",
    sh: "cd git mkdir curl rustup cargo claude",
  };
  function highlight(src, lang) {
    const kw = new Set((KW[lang] || "").split(" "));
    const re = lang === "sh"
      ? /(#[^\n]*)|("(?:[^"\\]|\\.)*")|(\s--?[\w-]+)|(\b[\w./~:-]+\b)/g
      : /(\/\/[^\n]*)|("""[\s\S]*?"""|"(?:[^"\\\n]|\\.)*")|(\b\d+(?:\.\d+)?\b)|(\b[A-Za-z_]\w*\b)(\s*(?=\())?/g;
    let out = "", last = 0, m;
    while ((m = re.exec(src))) {
      out += esc(src.slice(last, m.index));
      last = re.lastIndex;
      const t = esc(m[0]);
      if (m[1]) out += `<span class="tk-com">${t}</span>`;
      else if (m[2]) out += `<span class="tk-str">${t}</span>`;
      else if (lang === "sh") {
        if (m[3]) out += `<span class="tk-num">${t}</span>`;
        else out += kw.has(m[0]) ? `<span class="tk-kw">${t}</span>` : t;
      } else if (m[3]) out += `<span class="tk-num">${t}</span>`;
      else {
        const w = m[4];
        if (kw.has(w)) out += `<span class="tk-kw">${esc(w)}</span>`;
        else if (m[5] !== undefined) out += `<span class="tk-fn">${esc(w)}</span>`;
        else if (/^[A-Z]/.test(w)) out += `<span class="tk-ty">${esc(w)}</span>`;
        else out += esc(w);
        if (m[5]) out += m[5];
      }
    }
    return out + esc(src.slice(last));
  }

  /* ---------- copy ---------- */
  async function copyText(btn, text) {
    try {
      await navigator.clipboard.writeText(text);
      btn.textContent = "Copied";
    } catch {
      btn.textContent = "Press ⌘C";
    }
    btn.classList.add("done");
    setTimeout(() => { btn.textContent = "Copy"; btn.classList.remove("done"); }, 1600);
  }

  $$("pre[data-lang]").forEach((pre) => {
    const code = $("code", pre);
    const raw = code.textContent;
    code.innerHTML = highlight(raw, pre.dataset.lang);
    const b = document.createElement("button");
    b.className = "copy";
    b.type = "button";
    b.textContent = "Copy";
    b.setAttribute("aria-label", "Copy code");
    b.addEventListener("click", () => copyText(b, raw));
    pre.appendChild(b);
  });
  $$(".copy[data-copy]").forEach((b) => b.addEventListener("click", () => copyText(b, b.dataset.copy)));

  /* ---------- theme ---------- */
  const root = document.documentElement;
  $(".theme-btn").addEventListener("click", () => {
    const dark = root.dataset.theme
      ? root.dataset.theme === "dark"
      : matchMedia("(prefers-color-scheme: dark)").matches;
    root.dataset.theme = dark ? "light" : "dark";
    try { localStorage.setItem("yalin-theme", root.dataset.theme); } catch {}
  });

  /* ---------- mobile contents ---------- */
  const menuBtn = $(".menu-btn");
  const setMenu = (open) => {
    document.body.classList.toggle("toc-open", open);
    menuBtn.setAttribute("aria-expanded", String(open));
  };
  menuBtn.addEventListener("click", () => setMenu(!document.body.classList.contains("toc-open")));
  $$(".toc a").forEach((a) => a.addEventListener("click", () => setMenu(false)));
  document.addEventListener("click", (e) => {
    if (document.body.classList.contains("toc-open") && !e.target.closest(".toc, .menu-btn")) setMenu(false);
  });
  document.addEventListener("keydown", (e) => { if (e.key === "Escape") setMenu(false); });

  /* ---------- scroll spy ---------- */
  const links = new Map($$(".toc a").map((a) => [a.getAttribute("href").slice(1), a]));
  const sections = $$(".content section[id]");
  let current = null, ticking = false;
  function spy() {
    ticking = false;
    let id = null;
    for (const s of sections) {
      if (s.getBoundingClientRect().top > 140) break;
      id = s.id;
    }
    if (id === current) return;
    if (current) links.get(current)?.classList.remove("on");
    current = id;
    const a = id && links.get(id);
    if (a) {
      a.classList.add("on");
      const toc = $(".toc");
      if (toc.scrollHeight > toc.clientHeight) {
        const r = a.getBoundingClientRect(), t = toc.getBoundingClientRect();
        if (r.top < t.top || r.bottom > t.bottom) toc.scrollTop += r.top - t.top - t.height / 2;
      }
    }
  }
  addEventListener("scroll", () => { if (!ticking) { ticking = true; requestAnimationFrame(spy); } }, { passive: true });
  spy();

  /* ---------- benchmark charts ---------- */
  // Infinix X6833B (Helio G99), Android 13, AndroidX Microbenchmark, release build.
  const DATA = {
    project: { unit: "ms", rows: [["Yalın", 7.29], ["fastjson2", 19.98], ["Jackson", 38.56], ["org.json", 41.78], ["Gson", 44.90], ["kotlinx", 50.20], ["Moshi", 64.52]] },
    tree: { unit: "ms", rows: [["Yalın", 16.81], ["fastjson2", 21.94], ["Jackson", 38.62], ["Gson", 44.91], ["kotlinx", 48.44], ["Moshi", 63.78]] },
    "ios-project": { unit: "ms", rows: [["Yalın", 2.23], ["JSONSerialization", 15.74]] },
    "ios-tree": { unit: "ms", rows: [["Yalın", 9.26], ["JSONSerialization", 15.54]] },
    alloc: { unit: "objects", rows: [["Yalın", 79.3], ["fastjson2", 149.1], ["Jackson", 243.9], ["kotlinx", 304.4], ["Moshi", 304.5], ["Gson", 348.5]] },
  };
  function render(el, key) {
    const d = DATA[key];
    const max = Math.max(...d.rows.map((r) => r[1]));
    const base = d.rows[0][1];
    el.innerHTML = d.rows.map(([name, v], i) => {
      const label = d.unit === "ms" ? `${v.toFixed(2)} ms` : `${v}k`;
      const rel = i === 0 ? "" : `<em>${(v / base).toFixed(1)}×</em>`;
      return `<div class="row${i === 0 ? " me" : ""}"><span class="row-name">${name}</span>` +
        `<span class="track"><span class="fill" data-w="calc((100% - 128px) * ${(v / max).toFixed(4)})"></span><span class="val">${label}${rel}</span></span></div>`;
    }).join("");
    el.setAttribute("role", "img");
    el.setAttribute("aria-label", d.rows.map(([n, v]) => `${n} ${v}${d.unit === "ms" ? " ms" : "k objects"}`).join(", "));
    requestAnimationFrame(() => requestAnimationFrame(() =>
      $$(".fill", el).forEach((f) => { f.style.width = f.dataset.w; })));
  }
  const grow = new IntersectionObserver((entries) => {
    for (const e of entries) if (e.isIntersecting) { render(e.target, e.target.dataset.chart); grow.unobserve(e.target); }
  }, { threshold: 0.25 });
  $$(".chart[data-chart]").forEach((c) => grow.observe(c));

  const bench = $("#bench-chart");
  $$(".tabs button").forEach((t) => t.addEventListener("click", () => {
    $$(".tabs button").forEach((b) => b.setAttribute("aria-selected", String(b === t)));
    bench.dataset.chart = t.dataset.tab;
    render(bench, t.dataset.tab);
  }));

  /* ---------- path playground: a JS model of yalin-core's path rules ---------- */
  function parsePath(p) {
    const segs = [];
    let i = 0;
    const name = () => {
      const s = i;
      while (i < p.length && p[i] !== "." && p[i] !== "[" && p[i] !== "]") i++;
      if (i === s) throw i;
      segs.push({ key: p.slice(s, i) });
    };
    const index = () => {
      i++; // [
      const s = i;
      while (i < p.length && p[i] >= "0" && p[i] <= "9") i++;
      if (i === s || (p[s] === "0" && i - s > 1)) throw s;
      if (p[i] !== "]") throw i;
      segs.push({ idx: Number(p.slice(s, i)) });
      i++;
    };
    if (p === "") return segs;
    if (p[0] === "[") index(); else name();
    while (i < p.length) {
      if (p[i] === ".") { i++; name(); }
      else if (p[i] === "[") index();
      else throw i;
    }
    return segs;
  }
  const kindOf = (v) => v === null ? "NULL" : Array.isArray(v) ? "ARRAY" : typeof v === "object" ? "OBJECT"
    : typeof v === "string" ? "STRING" : typeof v === "number" ? "NUMBER" : "BOOLEAN";
  function lookup(doc, path) {
    let segs;
    try { segs = parsePath(path); } catch (pos) { return { err: `YalinPathException: invalid path at position ${pos}` }; }
    let v = doc;
    for (const s of segs) {
      if ("key" in s) {
        if (kindOf(v) !== "OBJECT") return { err: `YalinTypeException: expected OBJECT, found ${kindOf(v)}` };
        if (!Object.prototype.hasOwnProperty.call(v, s.key)) return { kind: "MISSING" };
        v = v[s.key];
      } else {
        if (kindOf(v) !== "ARRAY") return { err: `YalinTypeException: expected ARRAY, found ${kindOf(v)}` };
        if (s.idx >= v.length) return { kind: "MISSING" };
        v = v[s.idx];
      }
    }
    return { kind: kindOf(v), value: v };
  }
  function getter(r) {
    if (r.kind === "MISSING" || r.kind === "NULL") return "every getter returns null";
    if (r.kind === "STRING") return `getString → ${JSON.stringify(r.value)}`;
    if (r.kind === "BOOLEAN") return `getBoolean → ${r.value}`;
    if (r.kind === "NUMBER") {
      return Number.isInteger(r.value) ? `getLong → ${r.value}, getDouble → ${r.value.toFixed(1)}`
        : `getDouble → ${r.value} (getLong throws ArithmeticException)`;
    }
    const n = r.kind === "ARRAY" ? `${r.value.length} items` : `${Object.keys(r.value).length} keys`;
    return `${r.kind.toLowerCase()} of ${n}; typed getters throw YalinTypeException`;
  }
  const pgJson = $("#pg-json"), pgPaths = $("#pg-paths"), pgRes = $(".pg-res"), pgCall = $(".pg-call");
  function runPg() {
    const paths = pgPaths.value.split("\n").map((s) => s.trim()).filter((s, i, a) => s || a.length === 1);
    pgCall.textContent = `YalinJson.project(bytes, ${paths.map((p) => JSON.stringify(p)).join(", ")})`;
    let doc;
    try { doc = JSON.parse(pgJson.value); } catch (e) {
      pgRes.innerHTML = `<li><span class="pg-bad">YalinParseException — ${esc(e.message)}</span></li>`;
      return;
    }
    pgRes.innerHTML = paths.map((p) => {
      const r = lookup(doc, p);
      const tag = r.err ? `<span class="tag err">THROWS</span>`
        : `<span class="tag ${r.kind === "MISSING" ? "miss" : r.kind === "NULL" ? "null" : ""}">${r.kind}</span>`;
      return `<li><div class="p">${esc(p || '""')}</div><div class="r">${tag}${esc(r.err || getter(r))}</div></li>`;
    }).join("");
  }
  let t;
  [pgJson, pgPaths].forEach((el) => el.addEventListener("input", () => { clearTimeout(t); t = setTimeout(runPg, 120); }));
  runPg();
})();
