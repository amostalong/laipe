// laipe-app Tauri command wrappers for model catalog
//
// 镜像 PlotCraft `src/lib/llm.ts` 中 getModelCatalog / refreshModelCatalog 这两块
// (其他 chat 包装如 start_chat / cancel_chat 走 laipe-vue 的 tauriStream，
// 不在这里重复实现).
//
// Tauri commands:
//   get_model_catalog     -> ResolvedCatalog (mirrors `laipe_streaming::model_catalog::ResolvedCatalog`)
//   refresh_model_catalog -> ResolvedCatalog
//
// 前端通过 `useModelCatalog()` composable 调, 内部 module-level singleton 缓存.
// 错误以 Error 形式抛出, UI 用 try/catch 拿 message 展示.

import { invoke } from "@tauri-apps/api/core";
import type { ModelCatalog } from "../types/catalog";

/** Get the embedded model catalog (1:1 mirror of Rust `get_model_catalog` command) */
export async function getModelCatalog(): Promise<ModelCatalog> {
  return invoke<ModelCatalog>("get_model_catalog");
}

/** Force a fresh remote refresh (1:1 mirror of Rust `refresh_model_catalog` command) */
export async function refreshModelCatalog(): Promise<ModelCatalog> {
  return invoke<ModelCatalog>("refresh_model_catalog");
}
