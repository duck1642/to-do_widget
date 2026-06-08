import { invoke } from "@tauri-apps/api/core";

export async function getDefaultPath() {
  return await invoke("get_default_path");
}

/** @param {string} path */
export async function readTodo(path) {
  return await invoke("read_todo", { path });
}

/**
 * @param {string} path
 * @param {string} content
 */
export async function writeTodo(path, content) {
  return await invoke("write_todo", { path, content });
}

/** @param {string} path */
export async function getFileModifiedTime(path) {
  return await invoke("get_file_modified_time", { path });
}

/**
 * @param {string} path
 * @param {string} entryJson
 */
export async function logDeletedTask(path, entryJson) {
  return await invoke("log_deleted_task", { path, entryJson });
}

/** @param {string} path */
export async function popDeletedTask(path) {
  return await invoke("pop_deleted_task", { path });
}
