import { Button, Loader } from "@mantine/core";
import { Check, Copy, Download } from "lucide-react";
import { makeAutoObservable, onBecomeObserved, onBecomeUnobserved } from "mobx";
import { useState } from "react";
import { Owned } from "../../ui/Owned/Owned.tsx";
import { SourceToggle } from "../SourceToggle/SourceToggle.tsx";
import { CopyFeedback } from "../../../stores/CopyFeedback.ts";

// A sandbox without permissions has an opaque origin. Remove navigation elements
// as well: sandbox alone still permits a link to navigate its own frame.
export function sanitizedDocument(source: string) {
  // This document has no browsing context. Parsing preserves html/body attributes
  // without executing scripts or loading resources before removal.
  const doc = document.implementation.createHTMLDocument("");
  doc.open();
  doc.write(source);
  doc.close();
  doc
    .querySelectorAll(
      "script, iframe, frame, frameset, object, embed, base, meta, link, animate, set, animateMotion, animateTransform",
    )
    .forEach((node) => node.remove());
  doc.querySelectorAll("*").forEach((node) => {
    for (const attr of Array.from(node.attributes)) {
      const allowedImage = /^data:image\/(png|jpeg|gif|webp);base64,/i.test(
        attr.value,
      );
      const fragment =
        /^(href|xlink:href)$/.test(attr.name) &&
        attr.value.startsWith("#") &&
        node.namespaceURI === "http://www.w3.org/2000/svg" &&
        /^(use|linearGradient|radialGradient|pattern|textPath|filter)$/.test(
          node.localName,
        );
      if (
        (/^(src|srcset|poster|background)$/i.test(attr.name) &&
          !allowedImage) ||
        (/^on|^(href|xlink:href|action|formaction|target|srcdoc|ping|autofocus)$/i.test(
          attr.name,
        ) &&
          !fragment)
      )
        node.removeAttribute(attr.name);
    }
  });
  return doc;
}

export function isolatedDocument(
  source: string,
  options: { css?: string; dark?: boolean } = {},
) {
  const doc = sanitizedDocument(source);
  const policy = doc.createElement("meta");
  policy.httpEquiv = "Content-Security-Policy";
  policy.content =
    "default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; img-src data:; font-src data:; connect-src 'none'; frame-src 'none'; form-action 'none'; base-uri 'none'";
  doc.head.prepend(policy);
  const style = doc.createElement("style");
  style.textContent = `html{color-scheme:${options.dark ? "dark" : "light"};background:${options.dark ? "#000" : "#fff"};color:${options.dark ? "#ededed" : "#171717"};font-family:system-ui,sans-serif}body{margin:16px;overflow-wrap:anywhere}img,svg{max-width:100%}*{box-sizing:border-box}`;
  policy.after(style);
  if (options.css) {
    const customStyle = doc.createElement("style");
    // HTML serialization does not escape raw style text. A CSS escape prevents
    // a closing style tag from inserting markup when srcdoc parses it again.
    customStyle.textContent = options.css.replace(/</g, "\\3c ");
    doc.head.append(customStyle);
  }
  return "<!doctype html>" + doc.documentElement.outerHTML;
}

let mermaidPromise: Promise<(typeof import("mermaid"))["default"]> | undefined;
let serial = 0;
function mermaidApi() {
  return (mermaidPromise ??= import("mermaid").then(({ default: mermaid }) => {
    mermaid.initialize({
      startOnLoad: false,
      securityLevel: "strict",
      suppressErrorRendering: true,
      theme: "neutral",
      maxTextSize: 50000,
      maxEdges: 500,
    });
    return mermaid;
  }));
}

export function createRichPreview(
  kind: "mermaid" | "html",
  source: string,
  release = () => {},
) {
  let observed = false;
  let loading = false;
  let started = false;
  const state = makeAutoObservable(
    {
      copy: new CopyFeedback(),
      svg: "",
      error: "",
      get framedDocument() {
        return isolatedDocument(kind === "html" ? source : this.svg);
      },
      get diagramHeight() {
        if (!this.svg) return 160;
        // Mermaid HTML labels need HTML parsing rather than XML parsing.
        const root = new DOMParser()
          .parseFromString(this.svg, "text/html")
          .querySelector("svg");
        const height = Number(root?.getAttribute("viewBox")?.split(/\s+/)[3]);
        return Number.isFinite(height)
          ? Math.max(120, Math.min(560, height + 40))
          : 310;
      },
      *load() {
        if (kind !== "mermaid" || started) return;
        started = loading = true;
        const id = `chat-diagram-${++serial}`;
        try {
          const api = yield* toGenerator(mermaidApi());
          const result = yield* toGenerator(api.render(id, source));
          this.svg = result.svg;
        } catch (e) {
          this.error = e instanceof Error ? e.message : String(e);
        } finally {
          document.getElementById(`d${id}`)?.remove();
          loading = false;
          if (!observed) release();
        }
      },
    },
    {},
    { autoBind: true },
  );
  onBecomeObserved(state, "svg", () => {
    observed = true;
    void state.load();
  });
  onBecomeUnobserved(state, "svg", () => {
    observed = false;
    if (!loading) release();
  });
  return state;
}

const previews = new Map<string, ReturnType<typeof createRichPreview>>();

/** A source retains one render until its last observer and pending render finish.
 * integration: transcript-open ownership may replace this cache; mounted callers
 * acquire it only from Owned.create, never from a component render body.
 */
export function richPreviewRead(kind: "mermaid" | "html", source: string) {
  const key = JSON.stringify([kind, source]);
  let read = previews.get(key);
  if (!read) {
    read = createRichPreview(kind, source, () => previews.delete(key));
    previews.set(key, read);
  }
  return read;
}

function* toGenerator<T>(promise: Promise<T>): Generator<Promise<T>, T, T> {
  return yield promise;
}

type PreviewProps = {
  kind: "mermaid" | "html";
  source: string;
};
export default function RichPreview({ kind, source }: PreviewProps) {
  const [sourceOpened, setSourceOpened] = useState(false);
  return (
    <Owned
      key={JSON.stringify([kind, source])}
      create={() => richPreviewRead(kind, source)}
    >
      {(state) => {
        const { svg, error, framedDocument, diagramHeight, copy } = state;
        const downloadSvg = kind === "mermaid" && !sourceOpened && !!svg;
        const mime = downloadSvg
          ? "image/svg+xml"
          : kind === "html"
            ? "text/html"
            : "text/plain";
        const url = `data:${mime};charset=utf-8,${encodeURIComponent(downloadSvg ? svg : source)}`;
        return (
          <section
            className="rich-preview"
            data-preview-kind={kind}
            aria-label={kind === "mermaid" ? "Mermaid diagram" : "HTML preview"}
          >
            <div className="rich-preview-toolbar">
              <span className="rich-preview-label">
                {kind === "mermaid" ? "Mermaid" : "HTML"}
              </span>
              <SourceToggle
                source={sourceOpened}
                onChange={setSourceOpened}
                rich
              />
              <Button
                size="compact-xs"
                variant="subtle"
                aria-label="Copy source"
                title="Copy source"
                onClick={() => void copy.copy(source)}
              >
                {copy.copied() ? <Check size={14} /> : <Copy size={14} />}
              </Button>
              <Button
                component="a"
                href={url}
                download={
                  kind === "html"
                    ? "preview.html"
                    : downloadSvg
                      ? "diagram.svg"
                      : "diagram.mmd"
                }
                size="compact-xs"
                variant="subtle"
                aria-label="Download preview"
                title="Download preview"
              >
                <Download size={14} />
              </Button>
            </div>
            {copy.failed && (
              <p role="alert" className="rich-preview-error">
                Cannot copy. Open Source and select the text to copy it.
              </p>
            )}
            {sourceOpened ? (
              <pre className="rich-preview-source">
                <code>{source}</code>
              </pre>
            ) : error ? (
              <div className="rich-preview-error" role="alert">
                <strong>
                  {kind === "mermaid"
                    ? "Cannot render this diagram"
                    : "Preview error"}
                </strong>
                <pre>{error}</pre>
                <p>Open Source to inspect the original text.</p>
              </div>
            ) : kind === "mermaid" && !svg ? (
              <div className="rich-preview-loading">
                <Loader size="sm" />
                Render diagram
              </div>
            ) : (
              <iframe
                title={
                  kind === "mermaid"
                    ? "Mermaid diagram preview"
                    : "HTML content preview"
                }
                sandbox=""
                referrerPolicy="no-referrer"
                srcDoc={framedDocument}
                style={
                  kind === "mermaid" ? { height: diagramHeight } : undefined
                }
                className={`rich-preview-frame ${kind === "mermaid" ? "rich-preview-diagram" : ""}`}
              />
            )}
            <div className="rich-preview-note">
              {kind === "html" ? "Static preview." : "Diagram preview."}
            </div>
          </section>
        );
      }}
    </Owned>
  );
}
