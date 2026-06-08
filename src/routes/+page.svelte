<script>
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { markdownToTasks, tasksToMarkdown } from "../utils/parser.js";
  import { 
    enable as enableAutostart, 
    disable as disableAutostart, 
    isEnabled as isAutostartEnabled 
  } from "@tauri-apps/plugin-autostart";
  import { 
    Plus, 
    Undo2, 
    Redo2, 
    RotateCw, 
    Trash2, 
    ChevronUp, 
    ChevronDown, 
    Settings, 
    Layers, 
    X,
    Check
  } from "@lucide/svelte";

  // State variables (Svelte 5 Runes)
  let filePath = $state("");
  let tasks = $state([]);
  let lastModified = $state(0);
  let focusedTaskId = $state("");
  let editingPath = $state(false);
  let pathInputVal = $state("");
  let statusMessage = $state("");
  let layerMode = $state("normal");
  let showModeMenu = $state(false);
  let logPath = $state("");
  let redoStack = $state([]);
  let originalTexts = {};
  let dragEnabled = $state(true);
  let autostartEnabled = $state(false);

  // Keep a reference to inputs to set focus programmatically
  let inputElements = {};

  // Status helper
  function showStatus(msg) {
    statusMessage = msg;
    setTimeout(() => {
      if (statusMessage === msg) {
        statusMessage = "";
      }
    }, 2000);
  }

  // Load markdown tasks from file
  async function loadFile() {
    try {
      if (!filePath) {
        filePath = await invoke("get_default_path");
        pathInputVal = filePath;
      }
      
      logPath = filePath.replace(/\.md$/, ".jsonl");
      
      const content = await invoke("read_todo", { path: filePath });
      tasks = markdownToTasks(content);
      
      // Update last modified timestamp
      lastModified = await invoke("get_file_modified_time", { path: filePath });
      showStatus("Loaded");
    } catch (err) {
      showStatus("Err: " + err);
    }
  }

  // Save tasks back to markdown file
  async function saveFile() {
    try {
      const content = tasksToMarkdown(tasks);
      await invoke("write_todo", { path: filePath, content });
      
      // Update local modification timestamp
      lastModified = await invoke("get_file_modified_time", { path: filePath });
      showStatus("Saved");
    } catch (err) {
      showStatus("Err: " + err);
    }
  }

  // Polling folder watcher (lightweight checking)
  onMount(() => {
    loadFile();
    
    // Check if autostart is enabled
    isAutostartEnabled().then(val => {
      autostartEnabled = val;
    }).catch(() => {});

    // Load and apply saved window layering mode
    const savedMode = localStorage.getItem("todo-layer-mode");
    if (savedMode) {
      changeLayerMode(savedMode);
    }

    // Load drag enabled setting
    const savedDrag = localStorage.getItem("todo-drag-enabled");
    if (savedDrag !== null) {
      dragEnabled = savedDrag === "true";
    }
    
    const interval = setInterval(async () => {
      // Avoid reloading while editing to prevent cursor jumps
      if (focusedTaskId || editingPath || !filePath) return;
      
      try {
        const modTime = await invoke("get_file_modified_time", { path: filePath });
        if (modTime !== lastModified) {
          await loadFile();
        }
      } catch (e) {
        // Silently skip transient file locked errors on Windows
      }
    }, 2000);
    
    return () => clearInterval(interval);
  });


  // Action logging helper
  async function logAction(action) {
    try {
      // Clear redo stack on manual actions
      redoStack = [];
      await invoke("log_deleted_task", { path: logPath, entryJson: JSON.stringify(action) });
    } catch (err) {
      showStatus("Err log: " + err);
    }
  }

  // Action helpers
  function toggleTask(id) {
    const task = tasks.find(t => t.id === id);
    if (task) {
      logAction({ type: "toggle", id: task.id, oldChecked: task.checked, newChecked: !task.checked });
      task.checked = !task.checked;
      saveFile();
    }
  }

  function updateText(id, text) {
    const task = tasks.find(t => t.id === id);
    if (task && task.text !== text) {
      task.text = text;
      saveFile();
    }
  }

  function moveTaskUp(index) {
    if (index <= 0) return;
    logAction({ type: "move", fromIndex: index, toIndex: index - 1 });
    const temp = tasks[index];
    tasks[index] = tasks[index - 1];
    tasks[index - 1] = temp;
    saveFile();
  }

  function moveTaskDown(index) {
    if (index >= tasks.length - 1) return;
    logAction({ type: "move", fromIndex: index, toIndex: index + 1 });
    const temp = tasks[index];
    tasks[index] = tasks[index + 1];
    tasks[index + 1] = temp;
    saveFile();
  }

  async function deleteTask(index) {
    const taskToDelete = tasks[index];
    if (taskToDelete) {
      logAction({ type: "delete", index: index, task: taskToDelete });
      tasks.splice(index, 1);
      saveFile();
    }
  }

  function indentTask(id) {
    const task = tasks.find(t => t.id === id);
    if (task) {
      logAction({ type: "indent", id: task.id, oldIndent: task.indent, newIndent: task.indent + 1 });
      task.indent += 1;
      saveFile();
    }
  }

  function outdentTask(id) {
    const task = tasks.find(t => t.id === id);
    if (task && task.indent > 0) {
      logAction({ type: "indent", id: task.id, oldIndent: task.indent, newIndent: task.indent - 1 });
      task.indent -= 1;
      saveFile();
    }
  }

  async function addTask(index, indent = 0) {
    const newId = Math.random().toString(36).substring(2, 9) + Date.now().toString(36);
    const newTask = {
      id: newId,
      isTask: true,
      checked: false,
      text: "",
      indent: indent
    };
    
    const insertIndex = index === -1 ? tasks.length : index + 1;
    logAction({ type: "add", index: insertIndex, task: newTask });
    
    tasks.splice(insertIndex, 0, newTask);
    focusedTaskId = newId;
    saveFile();
    
    // Auto focus the input element
    await tick();
    if (inputElements[newId]) {
      inputElements[newId].focus();
    }
  }

  async function clearCompleted() {
    const completedTasks = tasks
      .map((t, i) => ({ task: t, index: i }))
      .filter(x => x.task.isTask && x.task.checked);
    
    if (completedTasks.length === 0) return;
    
    logAction({ type: "clear_completed", deletedTasks: completedTasks });
    
    tasks = tasks.filter(t => !t.isTask || !t.checked);
    saveFile();
  }

  // Core action application function (handles both applying an action and its inverse)
  function applyAction(action, isInverse) {
    if (action.type === "delete") {
      if (isInverse) {
        tasks.splice(action.index, 0, action.task);
      } else {
        const idx = tasks.findIndex(t => t.id === action.task.id);
        if (idx !== -1) tasks.splice(idx, 1);
      }
    } else if (action.type === "add") {
      if (isInverse) {
        const idx = tasks.findIndex(t => t.id === action.task.id);
        if (idx !== -1) tasks.splice(idx, 1);
      } else {
        tasks.splice(action.index, 0, action.task);
      }
    } else if (action.type === "toggle") {
      const task = tasks.find(t => t.id === action.id);
      if (task) {
        task.checked = isInverse ? action.oldChecked : action.newChecked;
      }
    } else if (action.type === "edit") {
      const task = tasks.find(t => t.id === action.id);
      if (task) {
        task.text = isInverse ? action.oldText : action.newText;
      }
    } else if (action.type === "move") {
      const from = isInverse ? action.toIndex : action.fromIndex;
      const to = isInverse ? action.fromIndex : action.toIndex;
      if (from >= 0 && from < tasks.length && to >= 0 && to < tasks.length) {
        const temp = tasks[from];
        tasks[from] = tasks[to];
        tasks[to] = temp;
      }
    } else if (action.type === "indent") {
      const task = tasks.find(t => t.id === action.id);
      if (task) {
        task.indent = isInverse ? action.oldIndent : action.newIndent;
      }
    } else if (action.type === "clear_completed") {
      if (isInverse) {
        // Restore in ascending order of original index
        const sorted = [...action.deletedTasks].sort((a, b) => a.index - b.index);
        for (const entry of sorted) {
          tasks.splice(entry.index, 0, entry.task);
        }
      } else {
        const idsToDelete = new Set(action.deletedTasks.map(x => x.task.id));
        tasks = tasks.filter(t => !idsToDelete.has(t.id));
      }
    }
  }

  async function undo() {
    try {
      const poppedLine = await invoke("pop_deleted_task", { path: logPath });
      const action = JSON.parse(poppedLine);
      
      applyAction(action, true);
      redoStack.push(action);
      
      saveFile();
      showStatus("Undone");
    } catch (err) {
      if (err.includes("No history") || err.includes("No such file")) {
        showStatus("No Undo");
      } else {
        showStatus("Err Undo: " + err);
      }
    }
  }

  async function redo() {
    if (redoStack.length === 0) return;
    
    const action = redoStack.pop();
    applyAction(action, false);
    
    try {
      await invoke("log_deleted_task", { path: logPath, entryJson: JSON.stringify(action) });
    } catch (err) {
      showStatus("Err log: " + err);
    }
    
    saveFile();
    showStatus("Redone");
  }

  // Keyboard navigation & editing handlers
  async function handleKeyDown(event, index, task) {
    if (event.key === "Tab") {
      event.preventDefault();
      if (event.shiftKey) {
        outdentTask(task.id);
      } else {
        indentTask(task.id);
      }
    } else if (event.key === "Enter") {
      event.preventDefault();
      await addTask(index, task.indent);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      for (let i = index - 1; i >= 0; i--) {
        if (tasks[i].isTask) {
          focusedTaskId = tasks[i].id;
          if (inputElements[tasks[i].id]) {
            inputElements[tasks[i].id].focus();
          }
          break;
        }
      }
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      for (let i = index + 1; i < tasks.length; i++) {
        if (tasks[i].isTask) {
          focusedTaskId = tasks[i].id;
          if (inputElements[tasks[i].id]) {
            inputElements[tasks[i].id].focus();
          }
          break;
        }
      }
    } else if (event.key === "Escape") {
      event.target.blur();
    }
  }

  function savePath() {
    if (pathInputVal.trim()) {
      filePath = pathInputVal.trim();
      editingPath = false;
      loadFile();
    }
  }

  function cancelPathEdit() {
    pathInputVal = filePath;
    editingPath = false;
  }

  function closeApp() {
    getCurrentWindow().close();
  }

  async function changeLayerMode(mode) {
    try {
      // If leaving desktop mode, unparent first
      if (layerMode === "desktop" && mode !== "desktop") {
        await invoke("set_desktop_parent", { enable: false });
      }

      // Reset both flags first
      await invoke("set_always_on_top", { onTop: false });
      await invoke("set_always_on_bottom", { onBottom: false });

      if (mode === "top") {
        await invoke("set_always_on_top", { onTop: true });
      } else if (mode === "desktop") {
        await invoke("set_desktop_parent", { enable: true });
        await invoke("set_always_on_bottom", { onBottom: true });
      }
      
      layerMode = mode;
      localStorage.setItem("todo-layer-mode", mode);
      showStatus("Mode: " + getModeLabel(mode));
      showModeMenu = false;
    } catch (err) {
      showStatus("Err Mode: " + err);
    }
  }

  function getModeLabel(mode) {
    if (mode === "top") return "Top";
    if (mode === "desktop") return "Desk";
    return "Norm";
  }

  function toggleDrag() {
    dragEnabled = !dragEnabled;
    localStorage.setItem("todo-drag-enabled", dragEnabled.toString());
    showStatus(dragEnabled ? "Drag On" : "Drag Off");
  }

  async function toggleAutostart() {
    try {
      if (autostartEnabled) {
        await disableAutostart();
        autostartEnabled = false;
        showStatus("Autostart Off");
      } else {
        await enableAutostart();
        autostartEnabled = true;
        showStatus("Autostart On");
      }
    } catch (err) {
      showStatus("Err Startup: " + err);
    }
  }


</script>

<main class="app-container">
  <!-- Title / Drag Header -->
  {#if dragEnabled}
    <header class="drag-header draggable" data-tauri-drag-region>
      <span class="title-text" data-tauri-drag-region>
        TO-DO {statusMessage ? `[${statusMessage}]` : ""}
      </span>
      <div class="header-controls">
        <button class="icon-btn-header" onclick={() => showModeMenu = !showModeMenu} title="Window Layer Mode">
          <Layers size={13} />
          <span class="btn-text">{getModeLabel(layerMode)}</span>
        </button>
        <button class="icon-btn-header" onclick={() => editingPath = !editingPath} title="Settings">
          <Settings size={13} />
        </button>
        <button class="icon-btn-header close" onclick={closeApp} title="Close">
          <X size={13} />
        </button>
      </div>
    </header>
  {:else}
    <header class="drag-header">
      <span class="title-text">
        TO-DO {statusMessage ? `[${statusMessage}]` : ""}
      </span>
      <div class="header-controls">
        <button class="icon-btn-header" onclick={() => showModeMenu = !showModeMenu} title="Window Layer Mode">
          <Layers size={13} />
          <span class="btn-text">{getModeLabel(layerMode)}</span>
        </button>
        <button class="icon-btn-header" onclick={() => editingPath = !editingPath} title="Settings">
          <Settings size={13} />
        </button>
        <button class="icon-btn-header close" onclick={closeApp} title="Close">
          <X size={13} />
        </button>
      </div>
    </header>
  {/if}

  <!-- Content Workspace -->
  <div class="content-area" style="position: relative;">
    {#if showModeMenu}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="dropdown-menu">
        <button class="menu-item {layerMode === 'top' ? 'active' : ''}" onclick={() => changeLayerMode("top")}>
          Always on Top
        </button>
        <button class="menu-item {layerMode === 'normal' ? 'active' : ''}" onclick={() => changeLayerMode("normal")}>
          Normal Window
        </button>
        <button class="menu-item {layerMode === 'desktop' ? 'active' : ''}" onclick={() => changeLayerMode("desktop")}>
          Pin to Desktop
        </button>
      </div>
    {/if}
    {#if editingPath}
      <div class="settings-panel">
        <label for="path-input" class="settings-label">File Path (UTF-8):</label>
        <input 
          id="path-input"
          type="text" 
          class="settings-input" 
          bind:value={pathInputVal} 
          placeholder="C:\Users\...\todo.md"
        />
        
        <!-- Window Dragging Toggle -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="settings-toggle-row" style="display: flex; align-items: center; gap: 6px; cursor: pointer;" onclick={toggleDrag}>
          <button 
            type="button"
            class="custom-check-btn {dragEnabled ? 'checked' : ''}" 
            title="Toggle header dragging"
          >
            {#if dragEnabled}
              <Check size={10} strokeWidth={4} />
            {/if}
          </button>
          <span class="settings-label" style="user-select: none; cursor: pointer;">
            Window Dragging
          </span>
        </div>

        <!-- Autostart Toggle -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="settings-toggle-row" style="display: flex; align-items: center; gap: 6px; cursor: pointer; margin-top: 4px;" onclick={toggleAutostart}>
          <button 
            type="button"
            class="custom-check-btn {autostartEnabled ? 'checked' : ''}" 
            title="Toggle autostart on boot"
          >
            {#if autostartEnabled}
              <Check size={10} strokeWidth={4} />
            {/if}
          </button>
          <span class="settings-label" style="user-select: none; cursor: pointer;">
            Start on Boot
          </span>
        </div>

        <div class="settings-actions">
          <button class="action-btn" onclick={savePath}>Save</button>
          <button class="action-btn" onclick={cancelPathEdit}>Cancel</button>
        </div>
      </div>
    {:else}
      <div class="task-list">
        {#each tasks as task, index (task.id)}
          {#if task.isTask}
            <div 
              class="task-row" 
              style="padding-left: {task.indent * 16}px"
            >
              <button 
                type="button"
                class="custom-check-btn {task.checked ? 'checked' : ''}" 
                onclick={() => toggleTask(task.id)}
                title={task.checked ? "Mark active" : "Mark completed"}
              >
                {#if task.checked}
                  <Check size={10} strokeWidth={4} />
                {/if}
              </button>
              <input 
                type="text" 
                class="task-text {task.checked ? 'completed' : ''}" 
                value={task.text}
                bind:this={inputElements[task.id]}
                onfocus={() => {
                  focusedTaskId = task.id;
                  originalTexts[task.id] = task.text;
                }}
                onblur={() => {
                  if (focusedTaskId === task.id) {
                    focusedTaskId = "";
                  }
                  const oldText = originalTexts[task.id];
                  if (oldText !== undefined && oldText !== task.text) {
                    logAction({ type: "edit", id: task.id, oldText, newText: task.text });
                  }
                  delete originalTexts[task.id];
                }}
                oninput={(e) => updateText(task.id, e.target.value)}
                onkeydown={(e) => handleKeyDown(e, index, task)}
                placeholder="New Task..."
              />
              <div class="row-actions">
                <button class="row-btn" onclick={() => moveTaskUp(index)} title="Move Up">
                  <ChevronUp size={13} />
                </button>
                <button class="row-btn" onclick={() => moveTaskDown(index)} title="Move Down">
                  <ChevronDown size={13} />
                </button>
                <button class="row-btn del" onclick={() => deleteTask(index)} title="Delete">
                  <Trash2 size={13} />
                </button>
              </div>
            </div>
          {:else}
            <!-- Render non-task text/headings as plain read-only line -->
            {#if task.raw.trim().length > 0}
              <div class="raw-row">
                <span class="raw-text">{task.raw}</span>
                <div class="row-actions">
                  <button class="row-btn del" onclick={() => deleteTask(index)} title="Delete">
                    <Trash2 size={13} />
                  </button>
                </div>
              </div>
            {/if}
          {/if}
        {/each}

        {#if tasks.length === 0}
          <div class="empty-state">
            No tasks. Press [+] below to start.
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <!-- Bottom Action Bar -->
  <footer class="bottom-bar">
    <button class="action-btn" onclick={() => addTask(-1, 0)} title="Add Task">
      <Plus size={13} />
    </button>
    <div class="footer-right">
      <button class="action-btn" onclick={undo} title="Undo last action">
        <Undo2 size={13} />
      </button>
      <button class="action-btn" onclick={redo} disabled={redoStack.length === 0} title="Redo last undone action">
        <Redo2 size={13} />
      </button>
      <button class="action-btn" onclick={loadFile} title="Reload file">
        <RotateCw size={13} />
      </button>
      <button class="action-btn" onclick={clearCompleted} title="Clear completed tasks">
        <Trash2 size={13} />
      </button>
    </div>
  </footer>
</main>

<style>
  :root {
    --bg-dark: #121212;
    --bg-panel: #1a1a1a;
    --bg-header: #222222;
    --border-color: #2e2e2e;
    --text-color: #e0e0e0;
    --text-muted: #757575;
    --accent: #00adb5;
  }

  /* Reset and base styles */
  :global(html), :global(body) {
    margin: 0;
    padding: 0;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background-color: var(--bg-panel) !important;
  }

  :global(body) {
    font-family: "Segoe UI", system-ui, -apple-system, sans-serif;
    color: var(--text-color);
    user-select: none;
    position: relative;
  }

  .app-container {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    flex-direction: column;
    background-color: var(--bg-panel);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    box-sizing: border-box;
    overflow: hidden;
  }

  /* Header zone */
  .drag-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 32px;
    background-color: var(--bg-header);
    border-bottom: 1px solid var(--border-color);
    padding: 0 8px;
    cursor: default;
    flex: none;
  }

  .drag-header.draggable {
    cursor: move;
  }

  .title-text {
    font-size: 11px;
    font-weight: bold;
    letter-spacing: 1px;
    color: var(--text-muted);
  }

  .header-controls {
    display: flex;
    gap: 4px;
  }

  /* Content body */
  .content-area {
    flex: 1;
    overflow: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
  }

  /* Settings view */
  .settings-panel {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px;
  }

  .settings-label {
    font-size: 12px;
    color: var(--text-muted);
  }

  .settings-input {
    background-color: var(--bg-dark);
    border: 1px solid var(--border-color);
    color: var(--text-color);
    padding: 6px;
    font-size: 12px;
    outline: none;
  }

  .settings-actions {
    display: flex;
    gap: 8px;
  }

  /* Tasks view */
  .task-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: max-content;
    min-width: 100%;
  }

  .task-row {
    display: flex;
    align-items: center;
    padding: 2px 4px;
    border-radius: 2px;
    min-width: 100%;
    width: max-content;
    box-sizing: border-box;
  }



  .custom-check-btn {
    width: 14px;
    height: 14px;
    margin-right: 8px;
    cursor: pointer;
    background: var(--bg-dark);
    border: 1.5px solid #444444;
    border-radius: 3px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    color: #ffffff;
    flex-shrink: 0;
  }

  .custom-check-btn:hover {
    border-color: #666666;
    background: rgba(255, 255, 255, 0.02);
  }

  .custom-check-btn:active {
    border-color: #888888;
    background: rgba(255, 255, 255, 0.05);
  }

  .custom-check-btn.checked {
    border-color: rgba(255, 255, 255, 0.85);
    background: #2e2e2e;
    color: #ffffff;
  }

  .custom-check-btn.checked:hover {
    border-color: #ffffff;
    background: #383838;
  }

  .custom-check-btn.checked:active {
    border-color: rgba(255, 255, 255, 0.7);
    background: #252525;
  }

  .task-text {
    flex: 1;
    background: transparent;
    border: none;
    color: var(--text-color);
    font-size: 13px;
    outline: none;
    padding: 2px 0;
    min-width: 150px; /* Prevent text input from collapsing on deep indentation */
  }

  .task-text.completed {
    text-decoration: line-through;
    color: var(--text-muted);
    opacity: 0.6;
  }

  /* Raw lines rendering */
  .raw-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 6px;
    background-color: rgba(255, 255, 255, 0.02);
    border-left: 2px solid var(--text-muted);
    margin: 4px 0;
    min-width: 100%;
    width: max-content;
    box-sizing: border-box;
  }

  .raw-text {
    font-size: 12px;
    color: var(--text-muted);
    font-style: italic;
  }

  .empty-state {
    text-align: center;
    font-size: 12px;
    color: var(--text-muted);
    padding-top: 40px;
  }

  /* Controls visibility on hover */
  .row-actions {
    display: flex;
    gap: 2px;
    opacity: 0;
    pointer-events: none;
  }

  .task-row:hover .row-actions,
  .raw-row:hover .row-actions {
    opacity: 1;
    pointer-events: auto;
  }

  /* Buttons */
  .icon-btn-header {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 4px;
    display: flex;
    align-items: center;
    gap: 3px;
    border-radius: 4px;
  }

  .icon-btn-header:hover {
    color: var(--text-color);
    background-color: var(--border-color);
  }

  .icon-btn-header.close:hover {
    color: #ff5555;
    background-color: rgba(255, 85, 85, 0.1);
  }

  .btn-text {
    font-size: 9px;
    font-weight: bold;
    text-transform: uppercase;
  }

  .row-btn {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 2px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 3px;
  }

  .row-btn:hover {
    color: var(--text-color);
    background-color: var(--border-color);
  }

  .row-btn.del:hover {
    color: #ff5555;
    background-color: rgba(255, 85, 85, 0.1);
  }

  .action-btn {
    background: transparent;
    border: 1px solid var(--border-color);
    color: var(--text-color);
    padding: 5px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
  }

  .action-btn:hover:not(:disabled) {
    background-color: var(--bg-header);
    border-color: var(--text-muted);
  }

  .action-btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  /* Bottom status bar */
  .bottom-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 36px;
    background-color: var(--bg-header);
    border-top: 1px solid var(--border-color);
    padding: 0 8px;
    flex: none;
  }

  .footer-right {
    display: flex;
    gap: 4px;
  }

  /* Dropdown Menu Mode Selector */
  .dropdown-menu {
    position: absolute;
    top: 8px;
    right: 8px;
    background-color: var(--bg-header);
    border: 1px solid var(--border-color);
    display: flex;
    flex-direction: column;
    z-index: 1000;
    padding: 4px 0;
    box-shadow: 0 4px 8px rgba(0, 0, 0, 0.5);
    width: 140px;
  }

  .menu-item {
    background: transparent;
    border: none;
    color: var(--text-color);
    padding: 6px 12px;
    font-size: 11px;
    text-align: left;
    cursor: pointer;
    width: 100%;
    outline: none;
  }

  .menu-item:hover {
    background-color: var(--border-color);
  }

  .menu-item.active {
    color: var(--accent);
    font-weight: bold;
  }

  /* Custom dark scrollbar styling for WebKit (Edge WebView2) */
  ::-webkit-scrollbar {
    width: 6px;
    height: 6px;
  }

  ::-webkit-scrollbar-track {
    background: transparent;
  }

  ::-webkit-scrollbar-thumb {
    background: #333333;
    border-radius: 3px;
  }

  ::-webkit-scrollbar-thumb:hover {
    background: #444444;
  }

  ::-webkit-scrollbar-corner {
    background: transparent;
  }

  /* Hide focus outlines globally */
  * {
    outline: none;
  }
  *:focus {
    outline: none;
  }
</style>
