import { describe, it, expect, afterEach } from "vitest";
import { Editor } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import { Markdown } from "@tiptap/markdown";
import { ResizableImage } from "../resizableImage";

let editors: Editor[] = [];

function makeEditor(content = "") {
  const editor = new Editor({
    element: document.createElement("div"),
    extensions: [
      StarterKit,
      Markdown,
      ResizableImage.configure({ inline: false, allowBase64: true, resize: { enabled: true } }),
    ],
    content,
    contentType: "markdown",
  });
  editors.push(editor);
  return editor;
}

// Undestroyed editors leave a pending ProseMirror DOMObserver timer behind, which then fires
// after the test's jsdom environment is torn down and throws an uncaught "document is not
// defined" - destroying here avoids that (and the CI flakiness it can cause).
afterEach(() => {
  editors.forEach((editor) => editor.destroy());
  editors = [];
});

describe("ResizableImage markdown round-trip", () => {
  it("uses plain markdown syntax when no width/height is set", () => {
    const editor = makeEditor();
    editor.commands.insertContent({
      type: "image",
      attrs: { src: "data:image/png;base64,AAAA", alt: "plain" },
    });

    expect(editor.getMarkdown().trim()).toBe("![plain](data:image/png;base64,AAAA)");
  });

  it("serializes a resized image as a raw <img> tag with width/height", () => {
    const editor = makeEditor();
    editor.commands.insertContent({
      type: "image",
      attrs: { src: "data:image/png;base64,BBBB", alt: "resized", width: 400, height: 200 },
    });

    expect(editor.getMarkdown().trim()).toBe(
      '<img src="data:image/png;base64,BBBB" alt="resized" width="400" height="200" />'
    );
  });

  it("round-trips a resized image through markdown without losing its size", () => {
    const editor = makeEditor();
    editor.commands.insertContent({
      type: "image",
      attrs: { src: "data:image/png;base64,CCCC", alt: "note-image", width: 320, height: 180 },
    });

    const markdown = editor.getMarkdown();
    editor.commands.setContent(markdown, { contentType: "markdown", emitUpdate: false });

    const imageNode = editor.getJSON().content?.find((n) => n.type === "image");
    expect(imageNode?.attrs).toMatchObject({ width: 320, height: 180 });
    expect(editor.getMarkdown()).toBe(markdown);
  });

  it("reapplies width/height to the rendered element when the node updates externally (e.g. a resize made on another device, applied via setContent)", () => {
    const editor = makeEditor();
    editor.commands.insertContent({
      type: "image",
      attrs: { src: "data:image/png;base64,DDDD", alt: "resized", width: 100, height: 100 },
    });

    const img = editor.view.dom.querySelector("img") as HTMLImageElement;
    expect(img.style.width).toBe("100px");
    expect(img.style.height).toBe("100px");

    // NoteEditor applies remote updates via setContent + emitUpdate: false - same doc shape,
    // new size, no local resize interaction involved.
    const resizedMarkdown = editor
      .getMarkdown()
      .replace('width="100" height="100"', 'width="300" height="450"');
    editor.commands.setContent(resizedMarkdown, { contentType: "markdown", emitUpdate: false });

    const updatedImg = editor.view.dom.querySelector("img") as HTMLImageElement;
    expect(updatedImg.style.width).toBe("300px");
    expect(updatedImg.style.height).toBe("450px");
  });
});
