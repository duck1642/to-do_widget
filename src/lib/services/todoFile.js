import { invoke } from "@tauri-apps/api/core";

export async function getDefaultPath() {
  return await invoke("get_default_path");
}

export async function readTodo(path) {
  return await invoke("read_todo", { path });
}

export async function writeTodo(path, content) {
  return await invoke("write_todo", { path, content });
}

export async function getFileModifiedTime(path) {
  return await invoke("get_file_modified_time", { path });
}

export async function logDeletedTask(path, entryJson) {
  return await invoke("log_deleted_task", { path, entryJson });
}

export async function popDeletedTask(path) {
  return await invoke("pop_deleted_task", { path });
}
