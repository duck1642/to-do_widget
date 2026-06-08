<script>
  import { Check } from "@lucide/svelte";

  let { 
    pathInputVal = $bindable(), 
    dragEnabled, 
    autostartEnabled, 
    onSave, 
    onCancel, 
    onToggleDrag, 
    onToggleAutostart 
  } = $props();
</script>

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
  <div class="settings-toggle-row" onclick={onToggleDrag}>
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
  <div class="settings-toggle-row settings-toggle-row-spaced" onclick={onToggleAutostart}>
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
    <button class="action-btn" onclick={onSave}>Save</button>
    <button class="action-btn" onclick={onCancel}>Cancel</button>
  </div>
</div>
