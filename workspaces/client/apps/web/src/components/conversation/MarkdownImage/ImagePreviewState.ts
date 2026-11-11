import { makeAutoObservable, observableRef } from "mobx";
import { errorText } from "../../../api.ts";
import type { Net } from "../../../servers/net/net.ts";
import { resourceData } from "../../../store/runtime.ts";
import { openFilePreview } from "../support/PreviewSelection.ts";
import type {
  PreviewTarget,
  FilePreviewSelection,
} from "../FilePreview/FilePreview.tsx";
import { localFileLink } from "../support/fileLinks.ts";
import { relativeToDocument, safeSvg } from "../support/filePreviewFormats.ts";

export type ImageSource = {
  src: string;
  agentId?: string | undefined;
  basePath?: string | undefined;
  localPath?: string | undefined;
  asset?: string;
};
const raster = /^data:image\/(png|jpeg|gif|webp);base64,/i;

/** Local reads, explicit remote consent and media failures for one image source. */
export class ImagePreviewState {
  readonly input: ImageSource;
  selected: { net: Net; file: FilePreviewSelection | null } | null = null;
  remoteAllowed = false;
  failed: Net | null = null;

  constructor(input: ImageSource) {
    this.input = input;
    makeAutoObservable(
      this,
      { input: false, selected: observableRef, failed: observableRef },
      { autoBind: true },
    );
  }

  get target(): PreviewTarget | null {
    const { src, agentId, basePath, localPath, asset } = this.input;
    if (asset) return { asset };
    try {
      const local =
        localPath !== undefined ? { path: localPath } : localFileLink(src);
      return local && agentId
        ? {
            agent: agentId,
            ...(localPath !== undefined
              ? local
              : relativeToDocument(local, basePath)),
          }
        : null;
    } catch {
      return null;
    }
  }

  get remoteHost() {
    try {
      const url = new URL(this.input.src);
      return /^https?:$/.test(url.protocol) && !url.username && !url.password
        ? url.hostname
        : "";
    } catch {
      return "";
    }
  }

  read(net: Net) {
    const target = this.target;
    if (raster.test(this.input.src) || !target) return undefined;
    const { agent, path, asset } = target;
    return net.reads.file({ agent, path, asset });
  }

  loaded(net: Net) {
    const current = this.read(net)?.current,
      file = current && resourceData(current);
    let image = raster.test(this.input.src) ? this.input.src : "";
    let error = current?.status === "error" ? errorText(current.error) : "";
    try {
      if (file) {
        if (
          this.input.asset &&
          !/^image\/(png|jpeg|gif|webp)$/.test(file.mime)
        ) {
          error = "This image type has no preview.";
        } else if (file.mime === "image/svg+xml") {
          const svg = safeSvg(
            new TextDecoder().decode(
              Uint8Array.from(atob(file.base64), (c) => c.charCodeAt(0)),
            ),
          );
          image = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(svg);
        } else {
          if (!/^image\/(png|jpeg|gif|webp|avif|bmp|x-icon)$/.test(file.mime))
            throw new Error("This image type is not supported.");
          image = `data:${file.mime};base64,${file.base64}`;
        }
      } else if (!this.target && !this.remoteHost && !image)
        error = "This image cannot be opened here.";
    } catch (cause) {
      error = errorText(cause);
    }
    return {
      image: image || (this.remoteAllowed ? this.input.src : ""),
      error: this.failed === net ? "Cannot load this image." : error,
    };
  }

  fail(net: Net) {
    this.failed = net;
  }
  allowRemote() {
    this.remoteAllowed = true;
  }
  preview(net: Net) {
    return this.selected?.net === net ? this.selected : null;
  }
  open(net: Net) {
    this.selected = {
      net,
      file: this.target ? openFilePreview(net, this.target) : null,
    };
  }
  close() {
    this.selected = null;
  }
  retry(net: Net) {
    this.failed = null;
    if (this.remoteHost) this.allowRemote();
    else
      void this.read(net)
        ?.read()
        .catch(() => {});
  }
}
