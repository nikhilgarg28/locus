(() => {
  const root = document.body.dataset.root || "";
  const dialog = document.querySelector("#search-dialog");
  const input = document.querySelector("#site-search");
  const results = document.querySelector("#search-results");
  const status = document.querySelector("#search-status");
  const open = () => {
    dialog.showModal();
    input.focus();
  };
  document.querySelector(".open-search")?.addEventListener("click", open);
  document
    .querySelector("#close-search")
    ?.addEventListener("click", () => dialog.close());
  dialog?.addEventListener("click", (event) => {
    if (event.target === dialog) {
      const r = dialog.getBoundingClientRect();
      if (
        event.clientX < r.left ||
        event.clientX > r.right ||
        event.clientY < r.top ||
        event.clientY > r.bottom
      )
        dialog.close();
    }
  });
  document.addEventListener("keydown", (event) => {
    const editing = /INPUT|TEXTAREA|SELECT/.test(
      document.activeElement?.tagName,
    );
    if (
      (event.key.toLowerCase() === "k" && (event.metaKey || event.ctrlKey)) ||
      (event.key === "/" && !editing)
    ) {
      event.preventDefault();
      if (!dialog.open) open();
    }
  });
  input?.addEventListener("input", () => {
    const query = input.value.trim().toLowerCase();
    results.replaceChildren();
    if (!query) {
      status.textContent = "Search chapters, rules, and tasks.";
      return;
    }
    const words = query.split(/\s+/);
    const matches = (window.LOCUS_SEARCH || [])
      .map((item) => {
        const title = item.title.toLowerCase(),
          text = item.text.toLowerCase();
        if (!words.every((word) => title.includes(word) || text.includes(word)))
          return null;
        const score =
          (title.includes(query) ? 20 : 0) +
          words.filter((word) => title.includes(word)).length * 5 +
          (item.kind === "Chapter" ? 2 : 0);
        return { item, score };
      })
      .filter(Boolean)
      .sort((a, b) => b.score - a.score)
      .slice(0, 20);
    status.textContent = matches.length
      ? `${matches.length} results${matches.length === 20 ? " (showing the first 20)" : ""}`
      : "No results. Try a topic, rule number, or LOC identifier.";
    for (const { item } of matches) {
      const li = document.createElement("li"),
        a = document.createElement("a"),
        kind = document.createElement("small"),
        title = document.createElement("strong"),
        p = document.createElement("p");
      a.href = root + item.url;
      kind.textContent = item.kind;
      title.textContent = item.title;
      const at = Math.max(0, item.text.toLowerCase().indexOf(words[0]) - 55);
      p.textContent =
        (at ? "…" : "") +
        item.text.slice(at, at + 180) +
        (item.text.length > at + 180 ? "…" : "");
      a.append(kind, title, p);
      li.append(a);
      results.append(li);
    }
  });
  document.querySelectorAll(".copy-code").forEach((button) =>
    button.addEventListener("click", async () => {
      const value = button.closest("figure").querySelector("code").textContent;
      try {
        await navigator.clipboard.writeText(value);
        button.textContent = "Copied";
        setTimeout(() => (button.textContent = "Copy"), 1800);
      } catch {
        button.textContent = "Select code to copy";
      }
    }),
  );
  // Opening a bookmarked task also opens any enclosing disclosure.
  function revealTarget() {
    let target;
    try {
      target = document.getElementById(
        decodeURIComponent(location.hash.slice(1)),
      );
    } catch {
      return;
    }
    if (!target) return;
    for (let element = target; element; element = element.parentElement)
      if (element.tagName === "DETAILS") element.open = true;
    if (target.classList.contains("task"))
      target.scrollIntoView({ block: "start" });
  }
  window.addEventListener("hashchange", revealTarget);
  revealTarget();
  if (matchMedia("(max-width:760px)").matches)
    document.querySelector(".chapter-menu")?.removeAttribute("open");
  const projectInput = document.querySelector("#roadmap-search");
  let filter = "all";
  const filterProjects = () => {
    const query = (projectInput?.value || "").trim().toLowerCase();
    let visible = 0;
    document.querySelectorAll(".project").forEach((p) => {
      p.hidden = !(
        (filter === "all" || p.dataset.status === filter) &&
        query.split(/\s+/).every((word) => p.dataset.search.includes(word))
      );
      if (!p.hidden) visible++;
    });
    const count = document.querySelector("#roadmap-results");
    if (count)
      count.textContent = `${visible} ${visible === 1 ? "project" : "projects"}`;
    const empty = document.querySelector(".filter-empty");
    if (empty) empty.hidden = visible > 0;
  };
  projectInput?.addEventListener("input", filterProjects);
  document.querySelectorAll("[data-filter]").forEach((button) =>
    button.addEventListener("click", () => {
      filter = button.dataset.filter;
      document
        .querySelectorAll("[data-filter]")
        .forEach((b) => b.setAttribute("aria-pressed", String(b === button)));
      filterProjects();
    }),
  );
  if (projectInput) filterProjects();
})();
