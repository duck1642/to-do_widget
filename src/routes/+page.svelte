<script>
  import { onMount, tick } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { markdownToTasks, tasksToMarkdown } from "$lib/utils/parser.js";
  import { getLogPath } from "$lib/utils/paths.js";
  import { createTask } from "$lib/utils/tasks.js";
  import { applyAction } from "$lib/utils/actions.js";
  import * as autostartService from "$lib/services/autostart.js";
  import * as configService from "$lib/services/config.js";
  import * as todoFileService from "$lib/services/todoFile.js";
  import { applyLayerMode } from "$lib/services/windowLayer.js";
  import "$lib/styles/app.css";
  import AppHeader from "$lib/components/AppHeader.svelte";
  import LayerMenu from "$lib/components/LayerMenu.svelte";
  import BottomBar from "$lib/components/BottomBar.svelte";
  import TaskList from "$lib/components/TaskList.svelte";
  import SettingsPanel from "$lib/components/SettingsPanel.svelte";

  // State variables (Svelte 5 Runes)
  let filePath = $state("");
  /** @type {any[]} */
  let tasks = $state([]);
  let lastModified = $state(0);
  let focusedTaskId = $state("");
  let editingPath = $state(false);
  let pathInputVal = $state("");
  let statusMessage = $state("");
  let layerMode = $state("normal");
  let showModeMenu = $state(false);
  let logPath = $state("");
  /** @type {any[]} */
  let redoStack = $state([]);
  /** @type {Record<string, string>} */
  let originalTexts = {};
  let dragEnabled = $state(true);
  let autostartEnabled = $state(false);

  // Keep a reference to inputs to set focus programmatically
  /** @type {Record<string, HTMLInputElement>} */
  let inputElements = {};

  // Status helper
  /** @param {string} msg */
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
        filePath = await todoFileService.getDefaultPath();
        pathInputVal = filePath;
      }
      
      logPath = getLogPath(filePath);
      
      const content = await todoFileService.readTodo(filePath);
      tasks = markdownToTasks(content);
      
      // Update last modified timestamp
      lastModified = await todoFileService.getFileModifiedTime(filePath);
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
      await todoFileService.writeTodo(filePath, content);
      
      // Update local modification timestamp
      lastModified = await todoFileService.getFileModifiedTime(filePath);
      showStatus("Saved");
    } catch (err) {
      showStatus("Err: " + err);
    }
  }

  // Helper to save current config to the backend config.json
  async function saveConfig() {
    try {
      await configService.writeConfig({
        file_path: filePath,
        layer_mode: layerMode,
        drag_enabled: dragEnabled,
        autostart_enabled: autostartEnabled
      });
    } catch (err) {
      showStatus("Err Config Save: " + err);
    }
  }

  // Polling folder watcher (lightweight checking)
  onMount(() => {
    /** @type {any} */
    let interval;

    (async () => {
      try {
        const config = await configService.readConfig();
        filePath = config.file_path;
        pathInputVal = filePath;
        dragEnabled = config.drag_enabled;

        try {
          autostartEnabled = await autostartService.isEnabled();
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
          const modTime = await todoFileService.getFileModifiedTime(filePath);
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
  /** @param {any} action */
  async function logAction(action) {
    try {
      // Clear redo stack on manual actions
      redoStack = [];
      await todoFileService.logDeletedTask(logPath, JSON.stringify(action));
    } catch (err) {
      showStatus("Err log: " + err);
    }
  }

  // Action helpers
  /** @param {string} id */
  function toggleTask(id) {
    const task = tasks.find(t => t.id === id);
    if (task) {
      logAction({ type: "toggle", id: task.id, oldChecked: task.checked, newChecked: !task.checked });
      task.checked = !task.checked;
      saveFile();
    }
  }

  /**
   * @param {string} id
   * @param {string} text
   */
  function updateText(id, text) {
    const task = tasks.find(t => t.id === id);
    if (task && task.text !== text) {
      task.text = text;
      saveFile();
    }
  }

  /** @param {number} index */
  function moveTaskUp(index) {
    if (index <= 0) return;
    logAction({ type: "move", fromIndex: index, toIndex: index - 1 });
    const temp = tasks[index];
    tasks[index] = tasks[index - 1];
    tasks[index - 1] = temp;
    saveFile();
  }

  /** @param {number} index */
  function moveTaskDown(index) {
    if (index >= tasks.length - 1) return;
    logAction({ type: "move", fromIndex: index, toIndex: index + 1 });
    const temp = tasks[index];
    tasks[index] = tasks[index + 1];
    tasks[index + 1] = temp;
    saveFile();
  }

  /** @param {number} index */
  async function deleteTask(index) {
    const taskToDelete = tasks[index];
    if (taskToDelete) {
      logAction({ type: "delete", index: index, task: taskToDelete });
      tasks.splice(index, 1);
      saveFile();
    }
  }

  /** @param {string} id */
  function indentTask(id) {
    const task = tasks.find(t => t.id === id);
    if (task) {
      logAction({ type: "indent", id: task.id, oldIndent: task.indent, newIndent: task.indent + 1 });
      task.indent += 1;
      saveFile();
    }
  }

  /** @param {string} id */
  function outdentTask(id) {
    const task = tasks.find(t => t.id === id);
    if (task && task.indent > 0) {
      logAction({ type: "indent", id: task.id, oldIndent: task.indent, newIndent: task.indent - 1 });
      task.indent -= 1;
      saveFile();
    }
  }

  /**
   * @param {number} index
   * @param {number} [indent]
   */
  async function addTask(index, indent = 0) {
    const newTask = createTask(indent);
    const newId = newTask.id;
    
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

  async function undo() {
    try {
      const poppedLine = await todoFileService.popDeletedTask(logPath);
      const action = JSON.parse(poppedLine);
      
      tasks = applyAction(tasks, action, true);
      redoStack.push(action);
      
      saveFile();
      showStatus("Undone");
    } catch (err) {
      const message = String(err);
      if (message.includes("No history") || message.includes("No such file")) {
        showStatus("No Undo");
      } else {
        showStatus("Err Undo: " + message);
      }
    }
  }

  async function redo() {
    if (redoStack.length === 0) return;
    
    const action = redoStack.pop();
    tasks = applyAction(tasks, action, false);
    
    try {
      await todoFileService.logDeletedTask(logPath, JSON.stringify(action));
    } catch (err) {
      showStatus("Err log: " + err);
    }
    
    saveFile();
    showStatus("Redone");
  }

  // Keyboard navigation & editing handlers
  /**
   * @param {KeyboardEvent} event
   * @param {number} index
   * @param {any} task
   */
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
      if (event.target instanceof HTMLElement) {
        event.target.blur();
      }
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

  function shrinkApp() {
    getCurrentWindow().hide();
  }

  function closeApp() {
    getCurrentWindow().close();
  }

  /** @param {string} mode */
  async function changeLayerMode(mode) {
    try {
      layerMode = await applyLayerMode(layerMode, mode);
      await saveConfig();
      showStatus("Mode: " + getModeLabel(mode));
      showModeMenu = false;
    } catch (err) {
      showStatus("Err Mode: " + err);
    }
  }

  /**
   * @param {string} mode
   * @returns {string}
   */
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
        await autostartService.disable();
        autostartEnabled = false;
        showStatus("Autostart Off");
      } else {
        await autostartService.enable();
        autostartEnabled = true;
        showStatus("Autostart On");
      }
      await saveConfig();
    } catch (err) {
      showStatus("Err Startup: " + err);
    }
  }


</script>

<main class="app-container" class:desktop-mode={layerMode === 'desktop'}>
  <!-- Title / Drag Header -->
  <AppHeader 
    dragEnabled={dragEnabled}
    layerMode={layerMode}
    statusMessage={statusMessage}
    onToggleModeMenu={() => showModeMenu = !showModeMenu}
    onToggleSettings={() => editingPath = !editingPath}
    onShrinkApp={shrinkApp}
    onCloseApp={closeApp}
  />

  <!-- Content Workspace -->
  <div class="content-area" style="position: relative;">
    {#if showModeMenu}
      <LayerMenu layerMode={layerMode} onSelectMode={changeLayerMode} />
    {/if}
    {#if editingPath}
      <SettingsPanel 
        bind:pathInputVal={pathInputVal}
        dragEnabled={dragEnabled}
        autostartEnabled={autostartEnabled}
        onSave={savePath}
        onCancel={cancelPathEdit}
        onToggleDrag={toggleDrag}
        onToggleAutostart={toggleAutostart}
      />
    {:else}
      <TaskList 
        tasks={tasks}
        inputElements={inputElements}
        onToggleTask={toggleTask}
        onUpdateText={updateText}
        onMoveTaskUp={moveTaskUp}
        onMoveTaskDown={moveTaskDown}
        onDeleteTask={deleteTask}
        onFocus={(/** @type {string} */ id, /** @type {string} */ text) => {
          focusedTaskId = id;
          originalTexts[id] = text;
        }}
        onBlur={(/** @type {string} */ id, /** @type {string} */ text) => {
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


