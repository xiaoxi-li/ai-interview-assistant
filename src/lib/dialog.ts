import { open } from "@tauri-apps/plugin-dialog";
import { isTauri } from "./utils";

/** 选择 Markdown 文件（可多选）；非 Tauri 环境返回空数组 */
export async function pickMarkdownFiles(): Promise<string[]> {
  if (!isTauri()) {
    console.warn("非 Tauri 环境，无法打开文件选择器");
    return [];
  }
  const selected = await open({
    multiple: true,
    directory: false,
    filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
  });
  if (!selected) return [];
  return Array.isArray(selected) ? selected : [selected];
}

/** 选择文件夹（批量导入其下全部 Markdown）；非 Tauri 环境返回 null */
export async function pickDirectory(): Promise<string | null> {
  if (!isTauri()) {
    console.warn("非 Tauri 环境，无法打开文件夹选择器");
    return null;
  }
  const selected = await open({ directory: true, multiple: false });
  if (!selected) return null;
  return Array.isArray(selected) ? (selected[0] ?? null) : selected;
}
