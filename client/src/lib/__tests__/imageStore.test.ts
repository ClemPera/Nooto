import { describe, it, expect, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { IMAGE_URI_PREFIX, registerLocalImage, resolveImageSrc } from "../imageStore";

/** Minimal PNG magic-byte prefix, enough for sniffImageMimeType to recognize it. */
function pngBytes(): number[] {
  return [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3];
}

describe("IMAGE_URI_PREFIX", () => {
  it("is a stable scheme prefix", () => {
    expect(IMAGE_URI_PREFIX).toBe("nooto-image:");
  });
});

describe("registerLocalImage", () => {
  it("makes resolveImageSrc return the registered URL without calling the backend", async () => {
    vi.mocked(invoke).mockReset();
    const uuid = "local-uuid-1";

    registerLocalImage(uuid, "blob:local-preview");

    await expect(resolveImageSrc(uuid)).resolves.toBe("blob:local-preview");
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe("resolveImageSrc", () => {
  it("fetches and decrypts through get_image, returning a blob URL", async () => {
    const uuid = "remote-uuid-1";
    vi.mocked(invoke).mockReset().mockResolvedValue(pngBytes());

    const src = await resolveImageSrc(uuid);

    expect(invoke).toHaveBeenCalledWith("get_image", { uuid });
    expect(src.startsWith("blob:")).toBe(true);
  });

  it("caches the result, only calling the backend once for repeated lookups", async () => {
    const uuid = "remote-uuid-2";
    vi.mocked(invoke).mockReset().mockResolvedValue(pngBytes());

    const first = await resolveImageSrc(uuid);
    const second = await resolveImageSrc(uuid);

    expect(first).toBe(second);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("evicts a failed lookup so a later retry can succeed", async () => {
    const uuid = "remote-uuid-3";
    vi.mocked(invoke).mockReset().mockRejectedValueOnce(new Error("offline"));

    await expect(resolveImageSrc(uuid)).rejects.toThrow("offline");

    vi.mocked(invoke).mockResolvedValue(pngBytes());
    await expect(resolveImageSrc(uuid)).resolves.toMatch(/^blob:/);
  });
});
