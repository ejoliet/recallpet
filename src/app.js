(function () {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;

  const statusEl = document.getElementById("status");
  const searchInput = document.getElementById("search");
  const listEl = document.getElementById("item-list");
  const emptyStateEl = document.getElementById("empty-state");
  const storageErrorEl = document.getElementById("storage-error");
  const pauseBtn = document.getElementById("pause-btn");
  const deleteBtn = document.getElementById("delete-btn");
  const openFolderBtn = document.getElementById("open-folder-btn");
  const confirmOverlay = document.getElementById("delete-confirm");
  const confirmYes = document.getElementById("confirm-delete-yes");
  const confirmNo = document.getElementById("confirm-delete-no");

  let paused = false;
  let searchDebounceHandle = null;

  function setAvatarState(state) {
    if (window.RecallPetAvatar) {
      window.RecallPetAvatar.setState(state);
    }
  }

  function showStorageError(show) {
    storageErrorEl.hidden = !show;
  }

  function formatRelativeTime(ms) {
    const diffSeconds = Math.max(0, Math.round((Date.now() - ms) / 1000));
    if (diffSeconds < 5) return "just now";
    if (diffSeconds < 60) return `${diffSeconds}s ago`;
    const minutes = Math.round(diffSeconds / 60);
    if (minutes < 60) return `${minutes} min ago`;
    const hours = Math.round(minutes / 60);
    if (hours < 24) return `${hours} hour${hours === 1 ? "" : "s"} ago`;
    const days = Math.round(hours / 24);
    return `${days} day${days === 1 ? "" : "s"} ago`;
  }

  function copyLabel(count) {
    return count <= 1 ? "Copied once" : `Copied ${count}×`;
  }

  function renderItems(items) {
    listEl.innerHTML = "";
    emptyStateEl.hidden = items.length > 0;

    for (const item of items) {
      const li = document.createElement("li");
      li.className = "item";

      const main = document.createElement("div");
      main.className = "item-main";

      const preview = document.createElement("p");
      preview.className = "item-preview";
      preview.textContent = item.preview;
      main.appendChild(preview);

      const meta = document.createElement("p");
      meta.className = "item-meta";
      meta.textContent = `${copyLabel(item.duplicateCount)} · ${formatRelativeTime(item.lastSeenAt)}`;
      main.appendChild(meta);

      li.appendChild(main);

      const copyBtn = document.createElement("button");
      copyBtn.type = "button";
      copyBtn.className = "item-copy-btn";
      copyBtn.textContent = "Copy";
      copyBtn.setAttribute("aria-label", `Copy: ${item.preview}`);
      copyBtn.addEventListener("click", () => copyItem(item.id));
      li.appendChild(copyBtn);

      listEl.appendChild(li);
    }
  }

  async function loadItems() {
    const query = searchInput.value.trim();
    try {
      const items = query
        ? await invoke("search_clipboard_items", { query, limit: 50 })
        : await invoke("get_recent_clipboard_items", { limit: 50 });
      showStorageError(false);
      renderItems(items);
    } catch (err) {
      console.error("Failed to load clipboard items", err);
      showStorageError(true);
      renderItems([]);
    }
  }

  async function copyItem(id) {
    try {
      await invoke("copy_item_again", { id });
    } catch (err) {
      console.error("Failed to copy item", err);
    }
  }

  function setPausedUi(nextPaused) {
    paused = nextPaused;
    statusEl.textContent = paused ? "Paused" : "Watching clipboard";
    statusEl.classList.toggle("paused", paused);
    pauseBtn.textContent = paused ? "Resume" : "Pause";
    setAvatarState(paused ? "paused" : "idle");
  }

  async function loadInitialStatus() {
    try {
      const status = await invoke("get_collection_status");
      setPausedUi(status.paused);
    } catch (err) {
      console.error("Failed to load collection status", err);
    }
  }

  pauseBtn.addEventListener("click", async () => {
    const next = !paused;
    try {
      await invoke("set_collection_paused", { paused: next });
      setPausedUi(next);
    } catch (err) {
      console.error("Failed to toggle pause", err);
    }
  });

  deleteBtn.addEventListener("click", () => {
    confirmOverlay.hidden = false;
    confirmYes.focus();
  });

  function closeConfirm() {
    confirmOverlay.hidden = true;
    deleteBtn.focus();
  }

  confirmNo.addEventListener("click", closeConfirm);

  confirmOverlay.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      closeConfirm();
    }
  });

  confirmYes.addEventListener("click", async () => {
    confirmOverlay.hidden = true;
    try {
      await invoke("delete_all_clipboard_items");
    } catch (err) {
      console.error("Failed to delete all items", err);
    }
    deleteBtn.focus();
  });

  openFolderBtn.addEventListener("click", async () => {
    try {
      await invoke("open_data_folder");
    } catch (err) {
      console.error("Failed to open data folder", err);
    }
  });

  searchInput.addEventListener("input", () => {
    clearTimeout(searchDebounceHandle);
    searchDebounceHandle = setTimeout(loadItems, 150);
  });

  listen("clipboard:item-created", () => loadItems());
  listen("clipboard:item-updated", () => loadItems());
  listen("storage:cleared", () => renderItems([]));
  listen("collector:paused", () => setPausedUi(true));
  listen("collector:resumed", () => setPausedUi(false));
  listen("avatar:state-changed", (event) => setAvatarState(event.payload));

  loadInitialStatus();
  loadItems();
})();
