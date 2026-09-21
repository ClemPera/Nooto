import { ResizableNodeView } from "@tiptap/core";
import Image from "@tiptap/extension-image";
import { IMAGE_URI_PREFIX, resolveImageSrc } from "./imageStore";

/** Sets `el.src`, resolving a `nooto-image:<uuid>` reference to a blob URL first if needed. */
function setImageSrc(el: HTMLImageElement, src: string): void {
  if (!src.startsWith(IMAGE_URI_PREFIX)) {
    el.src = src;
    return;
  }

  const uuid = src.slice(IMAGE_URI_PREFIX.length);
  resolveImageSrc(uuid)
    .then((blobUrl) => {
      el.src = blobUrl;
    })
    .catch(() => {
      el.alt = el.alt || "Failed to load image";
    });
}

/**
 * Plain markdown image syntax (`![alt](src)`) has no width/height field, so the stock
 * renderMarkdown drops any resize the user did. Here it falls back to a raw `<img>` tag
 * when width or height is set; parseMarkdown already handles that via generic HTML
 * parsing, so this stays a lossless round-trip.
 */
export const ResizableImage = Image.extend({
  renderMarkdown(node) {
    const { src, alt, title, width, height } = node.attrs as {
      src: string;
      alt: string | null;
      title: string | null;
      width: number | null;
      height: number | null;
    };

    if (!width && !height) {
      return title ? `![${alt ?? ""}](${src} "${title}")` : `![${alt ?? ""}](${src})`;
    }

    const attrs = [
      `src="${src}"`,
      alt ? `alt="${alt}"` : null,
      title ? `title="${title}"` : null,
      width ? `width="${width}"` : null,
      height ? `height="${height}"` : null,
    ]
      .filter(Boolean)
      .join(" ");
    return `<img ${attrs} />`;
  },

  // Images are stored server-side and referenced by a `nooto-image:<uuid>` src rather than
  // embedded inline, so the node view resolves that reference to a real (blob:) URL before
  // display. Otherwise identical to the base extension's resizable node view.
  addNodeView() {
    if (!this.options.resize || !this.options.resize.enabled || typeof document === "undefined") {
      return null;
    }

    const { directions, minWidth, minHeight, alwaysPreserveAspectRatio } = this.options.resize;

    return ({ node, getPos, HTMLAttributes, editor }) => {
      const el = document.createElement("img");

      Object.entries(HTMLAttributes).forEach(([key, value]) => {
        if (value != null && key !== "width" && key !== "height" && key !== "src") {
          el.setAttribute(key, value);
        }
      });

      setImageSrc(el, HTMLAttributes.src);

      const nodeView = new ResizableNodeView({
        element: el,
        editor,
        node,
        getPos,
        onResize: (width, height) => {
          el.style.width = `${width}px`;
          el.style.height = `${height}px`;
        },
        onCommit: (width, height) => {
          const pos = getPos();
          if (pos === undefined) return;

          this.editor
            .chain()
            .setNodeSelection(pos)
            .updateAttributes(this.name, { width, height })
            .run();
        },
        onUpdate: (updatedNode) => {
          if (updatedNode.type !== node.type) return false;
          setImageSrc(el, updatedNode.attrs.src);

          // ResizableNodeView's own update() only re-runs this callback - it does NOT
          // reapply width/height to the element (that only happens once, at construction).
          // Without this, a resize made on another device is saved correctly but never
          // shows up here until the note is closed and reopened (which reconstructs the
          // node view from scratch).
          const width = updatedNode.attrs.width as number | null;
          const height = updatedNode.attrs.height as number | null;
          if (width) el.style.width = `${width}px`;
          if (height) el.style.height = `${height}px`;

          return true;
        },
        options: {
          directions,
          min: { width: minWidth, height: minHeight },
          preserveAspectRatio: alwaysPreserveAspectRatio === true,
        },
      });

      const dom = nodeView.dom as HTMLElement;

      // Hidden until the image loads, so the node view mounts with the correct dimensions.
      dom.style.visibility = "hidden";
      dom.style.pointerEvents = "none";
      el.onload = () => {
        dom.style.visibility = "";
        dom.style.pointerEvents = "";
      };

      return nodeView;
    };
  },
});
