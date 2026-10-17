import { describe, it, expect, vi, afterEach } from "vitest";
import {
  isSupportedImageType,
  prepareImageForInsert,
  ImageInputError,
  mimeTypeForFilename,
  fileUriToPath,
  sniffImageMimeType,
} from "../image";

function makeFile(name: string, type: string, sizeBytes = 100): File {
  return new File([new Uint8Array(sizeBytes)], name, { type });
}

/** Stand-in for HTMLImageElement: jsdom doesn't decode images, so tests control dimensions directly. */
class FakeImage {
  onload: (() => void) | null = null;
  onerror: (() => void) | null = null;
  naturalWidth = 0;
  naturalHeight = 0;
  private _src = "";

  set src(value: string) {
    this._src = value;
    queueMicrotask(() => this.onload?.());
  }

  get src() {
    return this._src;
  }
}

describe("isSupportedImageType", () => {
  it("accepts common image formats", () => {
    expect(isSupportedImageType(makeFile("a.png", "image/png"))).toBe(true);
    expect(isSupportedImageType(makeFile("a.jpg", "image/jpeg"))).toBe(true);
    expect(isSupportedImageType(makeFile("a.webp", "image/webp"))).toBe(true);
    expect(isSupportedImageType(makeFile("a.gif", "image/gif"))).toBe(true);
  });

  it("rejects non-image or unsupported formats", () => {
    expect(isSupportedImageType(makeFile("a.pdf", "application/pdf"))).toBe(false);
    expect(isSupportedImageType(makeFile("a.svg", "image/svg+xml"))).toBe(false);
    expect(isSupportedImageType(makeFile("a.txt", ""))).toBe(false);
  });
});

describe("mimeTypeForFilename", () => {
  it("maps known extensions case-insensitively", () => {
    expect(mimeTypeForFilename("photo.PNG")).toBe("image/png");
    expect(mimeTypeForFilename("photo.jpg")).toBe("image/jpeg");
    expect(mimeTypeForFilename("photo.jpeg")).toBe("image/jpeg");
    expect(mimeTypeForFilename("photo.webp")).toBe("image/webp");
    expect(mimeTypeForFilename("photo.gif")).toBe("image/gif");
  });

  it("returns null for unrecognized or missing extensions", () => {
    expect(mimeTypeForFilename("document.pdf")).toBe(null);
    expect(mimeTypeForFilename("noextension")).toBe(null);
  });
});

describe("fileUriToPath", () => {
  it("returns null for non-file URIs", () => {
    expect(fileUriToPath("https://example.com/a.png")).toBe(null);
  });

  it("decodes a plain unix path", () => {
    expect(fileUriToPath("file:///home/clement/Downloads/3.png")).toBe(
      "/home/clement/Downloads/3.png"
    );
  });

  it("decodes percent-encoded characters", () => {
    expect(fileUriToPath("file:///home/clement/My%20Photos/3.png")).toBe(
      "/home/clement/My Photos/3.png"
    );
  });

  it("strips the extra leading slash on Windows drive paths", () => {
    expect(fileUriToPath("file:///C:/Users/clement/3.png")).toBe("C:/Users/clement/3.png");
  });
});

describe("sniffImageMimeType", () => {
  it("identifies formats from magic bytes", () => {
    expect(sniffImageMimeType(new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0, 0, 0, 0]))).toBe(
      "image/png"
    );
    expect(sniffImageMimeType(new Uint8Array([0xff, 0xd8, 0xff, 0, 0]))).toBe("image/jpeg");
    expect(sniffImageMimeType(new Uint8Array([0x47, 0x49, 0x46, 0x38, 0, 0]))).toBe("image/gif");
    expect(
      sniffImageMimeType(
        new Uint8Array([0x52, 0x49, 0x46, 0x46, 0, 0, 0, 0, 0x57, 0x45, 0x42, 0x50])
      )
    ).toBe("image/webp");
  });

  it("returns null for unrecognized bytes, e.g. a filename-less Android content URI read", () => {
    expect(sniffImageMimeType(new Uint8Array([1, 2, 3, 4]))).toBe(null);
  });
});

describe("prepareImageForInsert", () => {
  const originalImage = globalThis.Image;

  afterEach(() => {
    globalThis.Image = originalImage;
    vi.restoreAllMocks();
  });

  it("rejects unsupported file types before reading", async () => {
    await expect(prepareImageForInsert(makeFile("a.pdf", "application/pdf"))).rejects.toThrow(
      ImageInputError
    );
  });

  it("rejects files above the raw size limit", async () => {
    const file = makeFile("a.png", "image/png");
    Object.defineProperty(file, "size", { value: 21 * 1024 * 1024 });
    await expect(prepareImageForInsert(file)).rejects.toThrow(/too large/);
  });

  it("returns the original bytes and mime type unchanged when dimensions are within bounds", async () => {
    class SmallImage extends FakeImage {
      naturalWidth = 400;
      naturalHeight = 300;
    }
    // @ts-expect-error test double for HTMLImageElement
    globalThis.Image = SmallImage;
    const getContextSpy = vi.spyOn(HTMLCanvasElement.prototype, "getContext");

    const result = await prepareImageForInsert(makeFile("small.png", "image/png"));

    expect(result.mimeType).toBe("image/png");
    expect(result.bytes).toBeInstanceOf(Uint8Array);
    expect(getContextSpy).not.toHaveBeenCalled();
  });

  it("downscales through canvas when dimensions exceed the max", async () => {
    class LargeImage extends FakeImage {
      naturalWidth = 4000;
      naturalHeight = 2000;
    }
    // @ts-expect-error test double for HTMLImageElement
    globalThis.Image = LargeImage;

    const drawImage = vi.fn();
    const toDataURL = vi.fn(() => "data:image/jpeg;base64,cmVzaXplZA==");
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
      { drawImage } as unknown as CanvasRenderingContext2D
    );
    vi.spyOn(HTMLCanvasElement.prototype, "toDataURL").mockImplementation(toDataURL);

    const result = await prepareImageForInsert(makeFile("large.jpg", "image/jpeg"));

    expect(drawImage).toHaveBeenCalledWith(expect.anything(), 0, 0, 1280, 640);
    expect(toDataURL).toHaveBeenCalledWith("image/jpeg", 0.75);
    expect(result.mimeType).toBe("image/jpeg");
    // "cmVzaXplZA==" base64-decodes to "resized".
    expect(new TextDecoder().decode(result.bytes)).toBe("resized");
  });

  it("never resizes GIFs, to preserve animation", async () => {
    class LargeImage extends FakeImage {
      naturalWidth = 4000;
      naturalHeight = 4000;
    }
    // @ts-expect-error test double for HTMLImageElement
    globalThis.Image = LargeImage;
    const getContextSpy = vi.spyOn(HTMLCanvasElement.prototype, "getContext");

    const result = await prepareImageForInsert(makeFile("anim.gif", "image/gif"));

    expect(getContextSpy).not.toHaveBeenCalled();
    expect(result.mimeType).toBe("image/gif");
  });

  it("rejects when the compressed result is still too large", async () => {
    class LargeImage extends FakeImage {
      naturalWidth = 4000;
      naturalHeight = 4000;
    }
    // @ts-expect-error test double for HTMLImageElement
    globalThis.Image = LargeImage;

    const hugeBase64 = "A".repeat(12 * 1024 * 1024); // decodes to ~9MB, above the 4MB cap
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
      { drawImage: vi.fn() } as unknown as CanvasRenderingContext2D
    );
    vi.spyOn(HTMLCanvasElement.prototype, "toDataURL").mockReturnValue(
      `data:image/jpeg;base64,${hugeBase64}`
    );

    await expect(prepareImageForInsert(makeFile("large.png", "image/png"))).rejects.toThrow(
      /too large even after compression/
    );
  });
});
