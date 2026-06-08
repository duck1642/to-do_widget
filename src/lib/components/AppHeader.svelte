<script>
  import { Settings, Layers, X } from "@lucide/svelte";

  let { 
    dragEnabled, 
    layerMode, 
    statusMessage, 
    onToggleModeMenu, 
    onToggleSettings, 
    onCloseApp 
  } = $props();

  function getModeLabel(mode) {
    if (mode === "top") return "Top";
    if (mode === "desktop") return "Desk";
    return "Norm";
  }
</script>

<header class="drag-header" class:draggable={dragEnabled} data-tauri-drag-region={dragEnabled ? true : undefined}>
  <span class="title-text" data-tauri-drag-region={dragEnabled ? true : undefined}>
    TO-DO {statusMessage ? `[${statusMessage}]` : ""}
  </span>
  <div class="header-controls">
    <button class="icon-btn-header" onclick={onToggleModeMenu} title="Window Layer Mode">
      <Layers size={13} />
      <span class="btn-text">{getModeLabel(layerMode)}</span>
    </button>
    <button class="icon-btn-header" onclick={onToggleSettings} title="Settings">
      <Settings size={13} />
    </button>
    <button class="icon-btn-header close" onclick={onCloseApp} title="Close">
      <X size={13} />
    </button>
  </div>
</header>
