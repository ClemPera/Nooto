import { invoke } from "@tauri-apps/api/core";
import { sniffImageMimeType } from "./image";

/** Prefix used for the `src` attribute of an image node backed by a stored image row. */
export const IMAGE_URI_PREFIX = "nooto-image:";

// Blob URLs never expire on their own within a session, so resolving the same UUID twice
// (e.g. re-rendering after a note switch) should hit this cache rather than re-decrypt.
const blobUrlCache = new Map<string, Promise<string>>();

/**
 * Registers a blob URL for an image the caller just inserted locally, so the editor can
 * display it instantly instead of round-tripping through `get_image` right after insert.
 */
export function registerLocalImage(uuid: string, blobUrl: string): void {
  blobUrlCache.set(uuid, Promise.resolve(blobUrl));
}

/**
 * Resolves an image UUID to a displayable blob URL, fetching and decrypting it through the
 * backend on first use (falling back to the server if not yet cached locally) and caching
 * the result for the lifetime of the app.
 */
export function resolveImageSrc(uuid: string): Promise<string> {
  const cached = blobUrlCache.get(uuid);
  if (cached) return cached;

  const promise = invoke<number[]>("get_image", { uuid })
    .then((raw) => {
      const bytes = new Uint8Array(raw);
      const mimeType = sniffImageMimeType(bytes) ?? "application/octet-stream";
      return URL.createObjectURL(new Blob([bytes], { type: mimeType }));
    })
    .catch((err) => {
      blobUrlCache.delete(uuid);
      throw err;
    });

  blobUrlCache.set(uuid, promise);
  return promise;
}
