<script>
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { markdownToTasks, tasksToMarkdown } from "../utils/parser.js";
  import "$lib/styles/app.css";
  import AppHeader from "$lib/components/AppHeader.svelte";
  import LayerMenu from "$lib/components/LayerMenu.svelte";
  import BottomBar from "$lib/components/BottomBar.svelte";
  import TaskList from "$lib/components/TaskList.svelte";
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

  // Get log file path corresponding to the todo file path
  function getLogPath(path) {
    const replaced = path.replace(/\.[^\\/]+$/, ".jsonl");
    return replaced === path ? `${path}.jsonl` : replaced;
  }

  // Load markdown tasks from file
  async function loadFile() {
    try {
      if (!filePath) {
        filePath = await invoke("get_default_path");
        pathInputVal = filePath;
      }
      
      logPath = getLogPath(filePath);
      
      const content = await invoke("read_todo", { path: filePath });
      tasks = markdownToTasks(content);
      
      // Update last modified timestamp
      lastModified = await invoke("get_file_modified_time", { path: filePath });
      showStatus("Loaded");
      return true;
    } catch (err) {
      showStatus("Err: " + err);
      return false;
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

  // Helper to save current config to the backend config.json
  async function saveConfig() {
    try {
      await invoke("write_config", {
        config: {
          file_path: filePath,
          layer_mode: layerMode,
          drag_enabled: dragEnabled,
          autostart_enabled: autostartEnabled
        }
      });
    } catch (err) {
      showStatus("Err Config Save: " + err);
    }
  }

  // Polling folder watcher (lightweight checking)
  onMount(() => {
    let interval;

    (async () => {
      try {
        const config = await invoke("read_config");
        filePath = config.file_path;
        pathInputVal = filePath;
        dragEnabled = config.drag_enabled;

        try {
          autostartEnabled = await isAutostartEnabled();
        } catch {
          autostartEnabled = config.autostart_enabled;
        }
        
        await loadFile();
        await changeLayerMode(config.layer_mode);
      } catch (err) {
        showStatus("Err Config Load: " + err);
        await loadFile();
      }
      
      interval = setInterval(async () => {
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
    })();

    return () => {
      if (interval) clearInterval(interval);
    };
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

  async function savePath() {
    const newPath = pathInputVal.trim();
    if (!newPath) return;

    const oldPath = filePath;
    filePath = newPath;

    const ok = await loadFile();
    if (!ok) {
      filePath = oldPath;
      pathInputVal = oldPath;
      return;
    }

    editingPath = false;
    await saveConfig();
  }

  function cancelPathEdit() {
    pathInputVal = filePath;
    editingPath = false;
  }

  function closeApp() {
    getCurrentWindow().close();
  }

  async function changeLayerMode(mode) {
    if (mode === layerMode) {
      showModeMenu = false;
      return;
    }

    try {
      // If leaving desktop mode, unparent first
      if (layerMode === "desktop" && mode !== "desktop") {
        await invoke("set_desktop_parent", { enable: false });
      }

      // Reset top flag (always on bottom is removed)
      await invoke("set_always_on_top", { onTop: false });

      if (mode === "top") {
        await invoke("set_always_on_top", { onTop: true });
      } else if (mode === "desktop") {
        await invoke("set_desktop_parent", { enable: true });
      }
      
      layerMode = mode;
      await saveConfig();
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

  async function toggleDrag() {
    dragEnabled = !dragEnabled;
    await saveConfig();
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
      await saveConfig();
    } catch (err) {
      showStatus("Err Startup: " + err);
    }
  }


</script>

<main class="app-container">
  <!-- Title / Drag Header -->
  <AppHeader 
    dragEnabled={dragEnabled}
    layerMode={layerMode}
    statusMessage={statusMessage}
    onToggleModeMenu={() => showModeMenu = !showModeMenu}
    onToggleSettings={() => editingPath = !editingPath}
    onCloseApp={closeApp}
  />

  <!-- Content Workspace -->
  <div class="content-area" style="position: relative;">
    {#if showModeMenu}
      <LayerMenu layerMode={layerMode} onSelectMode={changeLayerMode} />
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
        <div class="settings-toggle-row" onclick={toggleDrag}>
          <button 
            type="button"
            class="custom-check-btn {dragEnabled ? 'checked' : ''}" 
            title="Toggle header dragging"
          >
            {#if dragEnabled}
              <Check size={10} strokeWidth={4} />
            {/if}
          </button>
          <span class="settings-label clickable-label">
            Window Dragging
          </span>
        </div>

        <!-- Autostart Toggle -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="settings-toggle-row settings-toggle-row-spaced" onclick={toggleAutostart}>
          <button 
            type="button"
            class="custom-check-btn {autostartEnabled ? 'checked' : ''}" 
            title="Toggle autostart on boot"
          >
            {#if autostartEnabled}
              <Check size={10} strokeWidth={4} />
            {/if}
          </button>
          <span class="settings-label clickable-label">
            Start on Boot
          </span>
        </div>

        <div class="settings-actions">
          <button class="action-btn" onclick={savePath}>Save</button>
          <button class="action-btn" onclick={cancelPathEdit}>Cancel</button>
        </div>
      </div>
    {:else}
      <TaskList 
        tasks={tasks}
        inputElements={inputElements}
        onToggleTask={toggleTask}
        onUpdateText={updateText}
        onMoveTaskUp={moveTaskUp}
        onMoveTaskDown={moveTaskDown}
        onDeleteTask={deleteTask}
        onFocus={(id, text) => {
          focusedTaskId = id;
          originalTexts[id] = text;
        }}
        onBlur={(id, text) => {
          if (focusedTaskId === id) {
            focusedTaskId = "";
          }
          const oldText = originalTexts[id];
          if (oldText !== undefined && oldText !== text) {
            logAction({ type: "edit", id, oldText, newText: text });
          }
          delete originalTexts[id];
        }}
        onKeyDown={handleKeyDown}
      />
    {/if}
  </div>

  <!-- Bottom Action Bar -->
  <BottomBar 
    redoStackLength={redoStack.length}
    onAddTask={() => addTask(-1, 0)}
    onUndo={undo}
    onRedo={redo}
    onReload={loadFile}
    onClearCompleted={clearCompleted}
  />
</main>


