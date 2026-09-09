import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { readFile } from "@tauri-apps/plugin-fs";
import { useToasts } from "../../../store/toasts";
import * as imageLib from "../../../lib/image";
import NoteEditor from "../NoteEditor";

/** Minimal PNG magic-byte prefix, enough for sniffImageMimeType to recognize it. */
function pngBytes(payloadSize = 40): Uint8Array {
  const bytes = new Uint8Array(8 + payloadSize);
  bytes.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  return bytes;
}

/** Stand-in for HTMLImageElement: jsdom doesn't decode images, so tests control dimensions directly. */
class SmallFakeImage {
  onload: (() => void) | null = null;
  onerror: (() => void) | null = null;
  naturalWidth = 200;
  naturalHeight = 150;
  private _src = "";

  set src(value: string) {
    this._src = value;
    queueMicrotask(() => this.onload?.());
  }

  get src() {
    return this._src;
  }
}

function makeImageFile(name = "photo.png") {
  return new File([new Uint8Array(50)], name, { type: "image/png" });
}

describe("NoteEditor paste handling", () => {
  it("parses plain-text clipboard content as markdown", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();

    const { container } = render(
      <NoteEditor noteId="note-1" content="" onChange={onChange} disabled={false} />
    );

    const editable = container.querySelector(".ProseMirror") as HTMLElement;
    editable.focus();
    await user.paste("**bold** and _italic_");

    await waitFor(() => {
      expect(editable.querySelector("strong")?.textContent).toBe("bold");
    });
    expect(editable.querySelector("em")?.textContent).toBe("italic");
    expect(editable.textContent).not.toContain("**");
  });

  it("leaves rich HTML clipboard content to the default paste handling", async () => {
    const onChange = vi.fn();

    const { container } = render(
      <NoteEditor noteId="note-2" content="" onChange={onChange} disabled={false} />
    );

    await waitFor(() => {
      expect(container.querySelector(".ProseMirror")).toBeTruthy();
    });
    const editable = container.querySelector(".ProseMirror") as HTMLElement;
    editable.focus();
    // jsdom has no DataTransfer constructor, stand in a minimal clipboardData for the paste event.
    const pasteEvent = new Event("paste", { bubbles: true, cancelable: true });
    Object.defineProperty(pasteEvent, "clipboardData", {
      value: {
        getData: (type: string) => {
          if (type === "text/html") return "<strong>already bold</strong>";
          if (type === "text/plain") return "already bold";
          return "";
        },
      },
    });
    act(() => {
      editable.dispatchEvent(pasteEvent);
    });

    await waitFor(() => {
      expect(editable.querySelector("strong")?.textContent).toBe("already bold");
    });
  });
});

describe("NoteEditor image insertion", () => {
  const originalImage = globalThis.Image;

  /** Routes the shared `invoke` mock by command name, since a single test can trigger both
   * `read_dropped_image` and `insert_image` (and the paste/toolbar paths only the latter). */
  function stubImageCommands(uuid = "fake-image-uuid") {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "insert_image") return uuid;
      if (cmd === "read_dropped_image") return Array.from(pngBytes());
      if (cmd === "list_note_images") return [];
      throw new Error(`unexpected invoke: ${cmd}`);
    });
  }

  afterEach(() => {
    globalThis.Image = originalImage;
    vi.restoreAllMocks();
    vi.mocked(openFileDialog).mockReset();
    vi.mocked(readFile).mockReset();
    vi.mocked(invoke).mockReset();
  });

  it("inserts an image via the toolbar button (desktop path) and syncs it back as markdown", async () => {
    // @ts-expect-error test double for HTMLImageElement
    globalThis.Image = SmallFakeImage;
    const user = userEvent.setup();
    const onChange = vi.fn();
    stubImageCommands("uuid-desktop");
    vi.mocked(openFileDialog).mockResolvedValue("/home/clement/Pictures/photo.png");
    vi.mocked(readFile).mockResolvedValue(pngBytes());

    const { container } = render(
      <NoteEditor noteId="note-1" content="" onChange={onChange} disabled={false} />
    );

    await user.click(screen.getByTitle("Insert image"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("insert_image", {
        note_id: "note-1",
        bytes: expect.any(Array),
      });
    });

    await waitFor(() => {
      const img = container.querySelector(".ProseMirror img") as HTMLImageElement | null;
      expect(img?.getAttribute("src")).toMatch(/^blob:/);
    });
    const img = container.querySelector(".ProseMirror img") as HTMLImageElement;
    expect(img.getAttribute("alt")).toBe("photo.png");

    await waitFor(
      () => {
        expect(onChange).toHaveBeenCalled();
      },
      { timeout: 1000 }
    );
    const lastMarkdown = onChange.mock.calls[onChange.mock.calls.length - 1][0] as string;
    expect(lastMarkdown).toMatch(/!\[photo\.png\]\(nooto-image:uuid-desktop\)/);
  });

  it("inserts an image via the toolbar button when the dialog returns an Android content:// URI", async () => {
    // @ts-expect-error test double for HTMLImageElement
    globalThis.Image = SmallFakeImage;
    const user = userEvent.setup();
    stubImageCommands();
    vi.mocked(openFileDialog).mockResolvedValue(
      "content://com.android.providers.media.documents/document/image%3A12345"
    );
    vi.mocked(readFile).mockResolvedValue(pngBytes());

    const { container } = render(
      <NoteEditor noteId="note-android" content="" onChange={vi.fn()} disabled={false} />
    );

    await user.click(screen.getByTitle("Insert image"));

    await waitFor(() => {
      // No filename in the URI to key off, so mime type comes from sniffing the bytes.
      const img = container.querySelector(".ProseMirror img") as HTMLImageElement | null;
      expect(img?.getAttribute("src")).toMatch(/^blob:/);
    });
    expect(invoke).toHaveBeenCalledWith("insert_image", {
      note_id: "note-android",
      bytes: expect.any(Array),
    });
  });

  it("does nothing when the file dialog is cancelled", async () => {
    const user = userEvent.setup();
    vi.mocked(openFileDialog).mockResolvedValue(null);

    const { container } = render(
      <NoteEditor noteId="note-cancel" content="" onChange={vi.fn()} disabled={false} />
    );

    await user.click(screen.getByTitle("Insert image"));

    expect(readFile).not.toHaveBeenCalled();
    expect(container.querySelector(".ProseMirror img")).toBeNull();
  });

  it("inserts an image at the cursor position on paste", async () => {
    // @ts-expect-error test double for HTMLImageElement
    globalThis.Image = SmallFakeImage;
    stubImageCommands();

    const onChange = vi.fn();
    const { container } = render(
      <NoteEditor noteId="note-2" content="hello" onChange={onChange} disabled={false} />
    );

    await waitFor(() => {
      expect(container.querySelector(".ProseMirror")).toBeTruthy();
    });
    const pm = container.querySelector(".ProseMirror") as HTMLElement;

    const file = makeImageFile("pasted.png");
    const clipboardData = {
      items: [{ type: "image/png", getAsFile: () => file }],
      files: [file],
      types: ["Files"],
      getData: () => "",
    };
    const pasteEvent = new Event("paste", { bubbles: true, cancelable: true });
    Object.defineProperty(pasteEvent, "clipboardData", { value: clipboardData });
    pm.dispatchEvent(pasteEvent);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("insert_image", {
        note_id: "note-2",
        bytes: expect.any(Array),
      });
    });
    await waitFor(() => {
      expect(container.querySelector(".ProseMirror img")).toBeTruthy();
    });
    const img = container.querySelector(".ProseMirror img") as HTMLImageElement;
    expect(img.getAttribute("alt")).toBe("pasted.png");
  });

  it("disables the insert-image button when the editor is disabled", async () => {
    render(<NoteEditor noteId="note-3" content="" onChange={vi.fn()} disabled={true} />);
    await waitFor(() => {
      expect(screen.getByTitle("Insert image")).toBeDisabled();
    });
  });

  it("renders resize handles on an inserted image", async () => {
    // @ts-expect-error test double for HTMLImageElement
    globalThis.Image = SmallFakeImage;
    const user = userEvent.setup();
    stubImageCommands();
    vi.mocked(openFileDialog).mockResolvedValue("/home/clement/Pictures/photo.png");
    vi.mocked(readFile).mockResolvedValue(pngBytes());

    const { container } = render(
      <NoteEditor noteId="note-4" content="" onChange={vi.fn()} disabled={false} />
    );

    await user.click(screen.getByTitle("Insert image"));

    await waitFor(() => {
      expect(container.querySelectorAll("[data-resize-handle]").length).toBeGreaterThan(0);
    });
  });

  it("commits an in-progress resize on touchend, since tiptap only listens for mouseup", async () => {
    const { container } = render(
      <NoteEditor noteId="note-touch-resize" content="" onChange={vi.fn()} disabled={false} />
    );
    await waitFor(() => {
      expect(container.querySelector(".ProseMirror")).toBeTruthy();
    });

    const marker = document.createElement("div");
    marker.setAttribute("data-resize-state", "true");
    document.body.appendChild(marker);

    const mouseupSpy = vi.fn();
    document.addEventListener("mouseup", mouseupSpy);
    document.dispatchEvent(new Event("touchend", { bubbles: true }));
    document.removeEventListener("mouseup", mouseupSpy);
    marker.remove();

    expect(mouseupSpy).toHaveBeenCalledTimes(1);
  });

  it("does not synthesize a mouseup on touchend when no resize is in progress", async () => {
    const { container } = render(
      <NoteEditor noteId="note-no-resize" content="" onChange={vi.fn()} disabled={false} />
    );
    await waitFor(() => {
      expect(container.querySelector(".ProseMirror")).toBeTruthy();
    });

    const mouseupSpy = vi.fn();
    document.addEventListener("mouseup", mouseupSpy);
    document.dispatchEvent(new Event("touchend", { bubbles: true }));
    document.removeEventListener("mouseup", mouseupSpy);

    expect(mouseupSpy).not.toHaveBeenCalled();
  });

  it("shows an error and does not insert when prepareImageForInsert rejects", async () => {
    // @ts-expect-error test double for HTMLImageElement
    globalThis.Image = SmallFakeImage;
    const user = userEvent.setup();
    vi.spyOn(imageLib, "prepareImageForInsert").mockRejectedValue(
      new imageLib.ImageInputError("Image is too large even after compression.")
    );
    vi.mocked(openFileDialog).mockResolvedValue("/home/clement/Pictures/a.png");
    vi.mocked(readFile).mockResolvedValue(pngBytes());

    useToasts.setState({ toasts: [] });
    const { container } = render(
      <NoteEditor noteId="note-too-large" content="" onChange={vi.fn()} disabled={false} />
    );

    await user.click(screen.getByTitle("Insert image"));

    await waitFor(() => {
      expect(useToasts.getState().toasts.some((t) => /too large/.test(t.message))).toBe(true);
    });
    expect(invoke).not.toHaveBeenCalledWith("insert_image", expect.anything());
    expect(container.querySelector(".ProseMirror img")).toBeNull();
  });

  it("shows an error and does not insert an image node when insert_image fails", async () => {
    // @ts-expect-error test double for HTMLImageElement
    globalThis.Image = SmallFakeImage;
    const user = userEvent.setup();
    vi.mocked(invoke).mockRejectedValue(new Error("No workspace is loaded"));
    vi.mocked(openFileDialog).mockResolvedValue("/home/clement/Pictures/a.png");
    vi.mocked(readFile).mockResolvedValue(pngBytes());

    useToasts.setState({ toasts: [] });
    const { container } = render(
      <NoteEditor noteId="note-insert-fails" content="" onChange={vi.fn()} disabled={false} />
    );

    await user.click(screen.getByTitle("Insert image"));

    await waitFor(() => {
      expect(useToasts.getState().toasts.some((t) => /Failed to insert image/.test(t.message))).toBe(
        true
      );
    });
    expect(container.querySelector(".ProseMirror img")).toBeNull();
  });

  it("reads the file from disk when a drop only carries a file:// path (WebKitGTK)", async () => {
    // @ts-expect-error test double for HTMLImageElement
    globalThis.Image = SmallFakeImage;
    stubImageCommands();
    document.elementFromPoint = vi.fn(() => null);

    const { container } = render(
      <NoteEditor noteId="note-5" content="" onChange={vi.fn()} disabled={false} />
    );

    await waitFor(() => {
      expect(container.querySelector(".ProseMirror")).toBeTruthy();
    });
    const pm = container.querySelector(".ProseMirror") as HTMLElement;

    const dataTransfer = {
      files: [],
      getData: (type: string) =>
        type === "text/uri-list" ? "file:///home/clement/Downloads/3.png" : "",
    };
    const dropEvent = new Event("drop", { bubbles: true, cancelable: true });
    Object.defineProperty(dropEvent, "dataTransfer", { value: dataTransfer });
    Object.defineProperty(dropEvent, "clientX", { value: 0 });
    Object.defineProperty(dropEvent, "clientY", { value: 0 });
    pm.dispatchEvent(dropEvent);

    await waitFor(() => {
      expect(container.querySelector(".ProseMirror img")).toBeTruthy();
    });

    expect(invoke).toHaveBeenCalledWith("read_dropped_image", {
      path: "/home/clement/Downloads/3.png",
    });
    expect(invoke).toHaveBeenCalledWith("insert_image", {
      note_id: "note-5",
      bytes: expect.any(Array),
    });
    const img = container.querySelector(".ProseMirror img") as HTMLImageElement;
    expect(img.getAttribute("alt")).toBe("3.png");
  });
});

describe("NoteEditor list indent/outdent buttons", () => {
  it("indent is disabled on the first item of a list, which has no sibling to nest under", async () => {
    const onChange = vi.fn();
    const { getByTitle } = render(
      <NoteEditor noteId="note-3" content={"- first\n- second"} onChange={onChange} disabled={false} />
    );

    await waitFor(() => {
      expect(getByTitle("Indent list item")).toBeDisabled();
    });
  });

  it("outdent lifts a top-level item out of the list into a plain paragraph", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();

    const { container, getByTitle } = render(
      <NoteEditor noteId="note-4" content={"- only item"} onChange={onChange} disabled={false} />
    );

    const editable = container.querySelector(".ProseMirror") as HTMLElement;
    expect(getByTitle("Outdent list item")).not.toBeDisabled();
    await user.click(getByTitle("Outdent list item"));

    await waitFor(() => {
      expect(editable.querySelector("li")).toBeNull();
      expect(editable.textContent).toContain("only item");
    });
  });
});

describe("NoteEditor attachments library", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.mocked(invoke).mockReset();
  });

  /** Routes the shared `invoke` mock by command name. `listByNote` simulates the backend's
   * per-note attachment library (keyed by note_id), independent of the note's text content. */
  function stubImageCommands({
    uuid = "fake-image-uuid",
    listByNote = {},
    deleteImpl,
  }: {
    uuid?: string;
    listByNote?: Record<string, string[]>;
    deleteImpl?: () => Promise<void>;
  } = {}) {
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: any) => {
      if (cmd === "insert_image") return uuid;
      if (cmd === "get_image") return Array.from(pngBytes());
      if (cmd === "delete_image") return deleteImpl ? deleteImpl() : undefined;
      if (cmd === "list_note_images") return listByNote[args?.note_id as string] ?? [];
      throw new Error(`unexpected invoke: ${cmd}`);
    });
  }

  it("shows no attachments bar for a note with no images", async () => {
    stubImageCommands();
    render(<NoteEditor noteId="note-attach-empty" content="just text" onChange={vi.fn()} disabled={false} />);

    await waitFor(() => {
      expect(screen.queryByTitle("Remove attachment")).toBeNull();
    });
  });

  it("shows an attachment already in the note's content on the very first render, without waiting for list_note_images or get_image to resolve", async () => {
    // The bug: a second device opening a note that already references an image (from
    // another device) wouldn't show it in the attachments bar, because the old DB-only
    // list raced with the image's own async fetch-and-cache. Here neither backend call
    // ever resolves, and the attachment must still appear - proving it comes from scanning
    // the content directly, not from either async source.
    vi.mocked(invoke).mockImplementation(() => new Promise(() => {}));

    render(
      <NoteEditor
        noteId="note-race"
        content="![a photo](nooto-image:race-uuid)"
        onChange={vi.fn()}
        disabled={false}
      />
    );

    await waitFor(() => {
      expect(screen.getByText("1 attachment")).toBeTruthy();
    });
  });

  it("loads the note's attachment library from the backend, independent of the text", async () => {
    stubImageCommands({ listByNote: { "note-attach-existing": ["existing-uuid"] } });
    render(
      <NoteEditor
        noteId="note-attach-existing"
        content="plain text, no image markdown in here at all"
        onChange={vi.fn()}
        disabled={false}
      />
    );

    await waitFor(() => {
      expect(screen.getByText("1 attachment")).toBeTruthy();
    });
  });

  it("adds a newly inserted image to the attachments list", async () => {
    // @ts-expect-error test double for HTMLImageElement
    globalThis.Image = SmallFakeImage;
    const user = userEvent.setup();
    stubImageCommands({ uuid: "new-uuid" });
    vi.mocked(openFileDialog).mockResolvedValue("/home/clement/Pictures/photo.png");
    vi.mocked(readFile).mockResolvedValue(pngBytes());

    render(<NoteEditor noteId="note-attach-add" content="" onChange={vi.fn()} disabled={false} />);

    await user.click(screen.getByTitle("Insert image"));

    await waitFor(() => {
      expect(screen.getByText("1 attachment")).toBeTruthy();
    });
  });

  it("updates the attachments list when switching to a different note", async () => {
    stubImageCommands({ listByNote: { "note-a": ["note-a-uuid"], "note-b": [] } });
    const { rerender } = render(
      <NoteEditor noteId="note-a" content="text" onChange={vi.fn()} disabled={false} />
    );

    await waitFor(() => {
      expect(screen.getByText("1 attachment")).toBeTruthy();
    });

    rerender(<NoteEditor noteId="note-b" content="text" onChange={vi.fn()} disabled={false} />);

    await waitFor(() => {
      expect(screen.queryByTitle("Remove attachment")).toBeNull();
    });
  });

  it("refreshes the attachment library when the currently open note's content is updated remotely", async () => {
    // Distinct from switching notes: same noteId throughout, only the content prop changes,
    // as it would when another device's edit is pulled down while this note stays open.
    const images: Record<string, string[]> = { "note-live": ["existing-uuid"] };
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: any) => {
      if (cmd === "list_note_images") return images[args?.note_id as string] ?? [];
      if (cmd === "get_image") return Array.from(pngBytes());
      throw new Error(`unexpected invoke: ${cmd}`);
    });

    const { rerender } = render(
      <NoteEditor noteId="note-live" content="original text" onChange={vi.fn()} disabled={false} />
    );

    await waitFor(() => {
      expect(screen.getByText("1 attachment")).toBeTruthy();
    });

    // Another device added an image to this same note; the backend's library grows and a
    // sync pull updates this device's content prop without switching notes.
    images["note-live"] = ["existing-uuid", "remote-added-uuid"];
    rerender(
      <NoteEditor
        noteId="note-live"
        content="original text, now with more"
        onChange={vi.fn()}
        disabled={false}
      />
    );

    await waitFor(() => {
      expect(screen.getByText("2 attachments")).toBeTruthy();
    });
  });

  it("keeps an attachment listed even after its reference is removed straight from the text", async () => {
    // Removing the image from the note (e.g. selecting and deleting it) shouldn't delete the
    // underlying data - that's what the explicit delete button is for. The library should
    // still offer it for re-insertion.
    stubImageCommands({ listByNote: { "note-attach-keep": ["kept-uuid"] } });
    render(
      <NoteEditor
        noteId="note-attach-keep"
        content="![a](nooto-image:kept-uuid)"
        onChange={vi.fn()}
        disabled={false}
      />
    );

    await waitFor(() => {
      expect(screen.getByText("1 attachment")).toBeTruthy();
    });

    const user = userEvent.setup();
    await user.click(screen.getByTitle("Bold (Ctrl+B)")); // just to focus the editor via toolbar
    await user.keyboard("{Control>}a{/Control}{Backspace}"); // select all, delete

    await waitFor(() => {
      expect(screen.getByText("1 attachment")).toBeTruthy();
    });
  });

  it("inserts an attachment into the note when clicked/tapped", async () => {
    stubImageCommands({ listByNote: { "note-attach-tap": ["tap-uuid"] } });
    const { container } = render(
      <NoteEditor noteId="note-attach-tap" content="" onChange={vi.fn()} disabled={false} />
    );

    const user = userEvent.setup();
    await waitFor(() => {
      expect(screen.getByTitle("Show attachments")).toBeTruthy();
    });
    await user.click(screen.getByTitle("Show attachments"));

    await waitFor(() => {
      expect(screen.getByTitle("Tap to insert into the note, or drag it into place")).toBeTruthy();
    });

    await user.click(screen.getByTitle("Tap to insert into the note, or drag it into place"));

    await waitFor(() => {
      expect(container.querySelector(".ProseMirror img")).toBeTruthy();
    });
  });

  it("inserts an attachment into the note when dragged in", async () => {
    stubImageCommands({ listByNote: { "note-attach-drag": ["drag-uuid"] } });
    const { container } = render(
      <NoteEditor noteId="note-attach-drag" content="" onChange={vi.fn()} disabled={false} />
    );

    await waitFor(() => {
      expect(container.querySelector(".ProseMirror")).toBeTruthy();
    });
    const pm = container.querySelector(".ProseMirror") as HTMLElement;

    const dataTransfer = {
      files: [],
      getData: (type: string) => (type === "application/x-nooto-image-uuid" ? "drag-uuid" : ""),
    };
    const dropEvent = new Event("drop", { bubbles: true, cancelable: true });
    Object.defineProperty(dropEvent, "dataTransfer", { value: dataTransfer });
    Object.defineProperty(dropEvent, "clientX", { value: 0 });
    Object.defineProperty(dropEvent, "clientY", { value: 0 });
    pm.dispatchEvent(dropEvent);

    await waitFor(() => {
      const img = container.querySelector(".ProseMirror img") as HTMLImageElement | null;
      expect(img?.getAttribute("src")).toMatch(/^blob:/);
    });
  });

  it("shows the delete button without needing hover (mobile-friendly)", async () => {
    stubImageCommands({ listByNote: { "note-attach-visible": ["vis-uuid"] } });
    const user = userEvent.setup();
    render(
      <NoteEditor noteId="note-attach-visible" content="" onChange={vi.fn()} disabled={false} />
    );

    await waitFor(() => {
      expect(screen.getByTitle("Show attachments")).toBeTruthy();
    });
    await user.click(screen.getByTitle("Show attachments"));

    const deleteButton = await screen.findByTitle("Remove attachment");
    // The old hover-reveal implementation kept the button transparent until :hover; it should
    // now always be visibly styled.
    expect(deleteButton.className).not.toMatch(/text-transparent/);
  });

  it("collapses and re-expands the attachments panel, hiding/showing thumbnails but not the count", async () => {
    stubImageCommands({ listByNote: { "note-attach-collapse": ["collapse-uuid"] } });
    const user = userEvent.setup();
    render(
      <NoteEditor noteId="note-attach-collapse" content="" onChange={vi.fn()} disabled={false} />
    );

    await waitFor(() => {
      expect(screen.getByTitle("Show attachments")).toBeTruthy();
    });
    // Collapsed by default - thumbnails aren't rendered yet, just the count header.
    expect(screen.queryByTitle("Remove attachment")).toBeNull();
    expect(screen.getByText("1 attachment")).toBeTruthy();

    await user.click(screen.getByTitle("Show attachments"));

    await waitFor(() => {
      expect(screen.getByTitle("Remove attachment")).toBeTruthy();
    });

    await user.click(screen.getByTitle("Hide attachments"));

    expect(screen.queryByTitle("Remove attachment")).toBeNull();
    expect(screen.getByText("1 attachment")).toBeTruthy();
  });

  it("deletes the stored image first, then removes it from the note and the list", async () => {
    // @ts-expect-error test double for HTMLImageElement
    globalThis.Image = SmallFakeImage;
    const user = userEvent.setup();
    const onChange = vi.fn();
    stubImageCommands({ uuid: "to-delete-uuid" });
    vi.mocked(openFileDialog).mockResolvedValue("/home/clement/Pictures/photo.png");
    vi.mocked(readFile).mockResolvedValue(pngBytes());

    const { container } = render(
      <NoteEditor noteId="note-attach-delete" content="" onChange={onChange} disabled={false} />
    );

    await user.click(screen.getByTitle("Insert image"));
    await waitFor(() => {
      expect(container.querySelector(".ProseMirror img")).toBeTruthy();
    });

    await user.click(screen.getByTitle("Show attachments"));
    await user.click(await screen.findByTitle("Remove attachment"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("delete_image", { uuid: "to-delete-uuid" });
    });
    await waitFor(() => {
      expect(container.querySelector(".ProseMirror img")).toBeNull();
      expect(screen.queryByTitle("Remove attachment")).toBeNull();
    });

    await waitFor(
      () => {
        const lastMarkdown = onChange.mock.calls[onChange.mock.calls.length - 1]?.[0] as
          | string
          | undefined;
        expect(lastMarkdown).not.toMatch(/nooto-image:/);
      },
      { timeout: 1000 }
    );
  });

  it("keeps the attachment listed and shows an error when delete_image fails", async () => {
    // @ts-expect-error test double for HTMLImageElement
    globalThis.Image = SmallFakeImage;
    const user = userEvent.setup();
    stubImageCommands({
      uuid: "stubborn-uuid",
      deleteImpl: () => Promise.reject(new Error("Could not reach the server")),
    });
    vi.mocked(openFileDialog).mockResolvedValue("/home/clement/Pictures/photo.png");
    vi.mocked(readFile).mockResolvedValue(pngBytes());

    const { container } = render(
      <NoteEditor noteId="note-attach-delete-fail" content="" onChange={vi.fn()} disabled={false} />
    );

    await user.click(screen.getByTitle("Insert image"));
    await waitFor(() => {
      expect(container.querySelector(".ProseMirror img")).toBeTruthy();
    });

    useToasts.setState({ toasts: [] });
    await user.click(screen.getByTitle("Show attachments"));
    await user.click(await screen.findByTitle("Remove attachment"));

    await waitFor(() => {
      expect(useToasts.getState().toasts.length).toBeGreaterThan(0);
    });
    // The image stays, both in the note and in the list, so the user can retry.
    expect(container.querySelector(".ProseMirror img")).toBeTruthy();
    expect(screen.getByTitle("Remove attachment")).toBeTruthy();
  });
});
