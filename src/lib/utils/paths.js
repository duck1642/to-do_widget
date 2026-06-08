export function getLogPath(path) {
  const replaced = path.replace(/\.[^\\/]+$/, ".jsonl");
  return replaced === path ? `${path}.jsonl` : replaced;
}
