<script>
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { markdownToTasks, tasksToMarkdown } from "../utils/parser.js";

  // State variables (Svelte 5 Runes)
  let filePath = $state("");
  let tasks = $state([]);
  let lastModified = $state(0);
  let focusedTaskId = $state("");
  let editingPath = $state(false);
  let pathInputVal = $state("");
  let statusMessage = $state("");

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

  // Action helpers
  function toggleTask(id) {
    const task = tasks.find(t => t.id === id);
    if (task) {
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
    const temp = tasks[index];
    tasks[index] = tasks[index - 1];
    tasks[index - 1] = temp;
    saveFile();
  }

  function moveTaskDown(index) {
    if (index >= tasks.length - 1) return;
    const temp = tasks[index];
    tasks[index] = tasks[index + 1];
    tasks[index + 1] = temp;
    saveFile();
  }

  function deleteTask(index) {
    tasks.splice(index, 1);
    saveFile();
  }

  function indentTask(id) {
    const task = tasks.find(t => t.id === id);
    if (task) {
      task.indent += 1;
      saveFile();
    }
  }

  function outdentTask(id) {
    const task = tasks.find(t => t.id === id);
    if (task && task.indent > 0) {
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
    
    if (index === -1) {
      tasks.push(newTask);
    } else {
      tasks.splice(index + 1, 0, newTask);
    }
    
    focusedTaskId = newId;
    saveFile();
    
    // Auto focus the input element
    await tick();
    if (inputElements[newId]) {
      inputElements[newId].focus();
    }
  }

  function clearCompleted() {
    tasks = tasks.filter(t => !t.isTask || !t.checked);
    saveFile();
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
</script>

<main class="app-container">
  <!-- Title / Drag Header -->
  <header class="drag-header" data-tauri-drag-region>
    <span class="title-text" data-tauri-drag-region>
      TO-DO {statusMessage ? `[${statusMessage}]` : ""}
    </span>
    <div class="header-controls">
      <button class="ascii-btn" onclick={() => editingPath = !editingPath} title="Settings">
        [S]
      </button>
      <button class="ascii-btn close" onclick={closeApp} title="Close">
        [X]
      </button>
    </div>
  </header>

  <!-- Content Workspace -->
  <div class="content-area">
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
        <div class="settings-actions">
          <button class="action-btn" onclick={savePath}>[Save]</button>
          <button class="action-btn" onclick={cancelPathEdit}>[Cancel]</button>
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
              <input 
                type="checkbox" 
                class="task-check" 
                checked={task.checked} 
                onchange={() => toggleTask(task.id)}
              />
              <input 
                type="text" 
                class="task-text {task.checked ? 'completed' : ''}" 
                value={task.text}
                bind:this={inputElements[task.id]}
                onfocus={() => focusedTaskId = task.id}
                onblur={() => {
                  if (focusedTaskId === task.id) {
                    focusedTaskId = "";
                  }
                }}
                oninput={(e) => updateText(task.id, e.target.value)}
                onkeydown={(e) => handleKeyDown(e, index, task)}
                placeholder="New Task..."
              />
              <div class="row-actions">
                <button class="row-btn" onclick={() => moveTaskUp(index)} title="Move Up">[^]</button>
                <button class="row-btn" onclick={() => moveTaskDown(index)} title="Move Down">[v]</button>
                <button class="row-btn del" onclick={() => deleteTask(index)} title="Delete">[x]</button>
              </div>
            </div>
          {:else}
            <!-- Render non-task text/headings as plain read-only line -->
            {#if task.raw.trim().length > 0}
              <div class="raw-row">
                <span class="raw-text">{task.raw}</span>
                <div class="row-actions">
                  <button class="row-btn del" onclick={() => deleteTask(index)} title="Delete">[x]</button>
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
    <button class="action-btn" onclick={() => addTask(-1, 0)}>[+] Add</button>
    <div class="footer-right">
      <button class="action-btn" onclick={loadFile}>[Reload]</button>
      <button class="action-btn" onclick={clearCompleted}>[Clear]</button>
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
  :global(body) {
    margin: 0;
    padding: 0;
    font-family: "Segoe UI", system-ui, -apple-system, sans-serif;
    background-color: transparent !important;
    color: var(--text-color);
    overflow: hidden;
    user-select: none;
  }

  .app-container {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    background-color: rgba(26, 26, 26, 0.95);
    border: 1px solid var(--border-color);
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
    overflow-y: auto;
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
  }

  .task-row {
    display: flex;
    align-items: center;
    padding: 2px 4px;
    border-radius: 2px;
  }

  .task-row:hover {
    background-color: var(--bg-header);
  }

  .task-check {
    width: 14px;
    height: 14px;
    margin-right: 8px;
    cursor: pointer;
    accent-color: var(--accent);
  }

  .task-text {
    flex: 1;
    background: transparent;
    border: none;
    color: var(--text-color);
    font-size: 13px;
    outline: none;
    padding: 2px 0;
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
    display: none;
    gap: 2px;
  }

  .task-row:hover .row-actions,
  .raw-row:hover .row-actions {
    display: flex;
  }

  /* Buttons */
  .ascii-btn {
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-family: monospace;
    font-size: 12px;
    cursor: pointer;
    padding: 2px 4px;
  }

  .ascii-btn:hover {
    color: var(--text-color);
  }

  .ascii-btn.close:hover {
    color: #ff5555;
  }

  .row-btn {
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-family: monospace;
    font-size: 11px;
    cursor: pointer;
    padding: 0 4px;
  }

  .row-btn:hover {
    color: var(--text-color);
  }

  .row-btn.del:hover {
    color: #ff5555;
  }

  .action-btn {
    background: transparent;
    border: 1px solid var(--border-color);
    color: var(--text-color);
    padding: 4px 8px;
    font-size: 12px;
    cursor: pointer;
  }

  .action-btn:hover {
    background-color: var(--bg-header);
    border-color: var(--text-muted);
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
  }

  .footer-right {
    display: flex;
    gap: 4px;
  }
</style>
