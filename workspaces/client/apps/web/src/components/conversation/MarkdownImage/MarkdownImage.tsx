import { useServerNet } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { Owned } from "../../ui/Owned/Owned.tsx";
import FilePreview from "../FilePreview/FilePreview.tsx";
import { ImagePreviewState, type ImageSource } from "./ImagePreviewState.ts";

type Props = ImageSource & { alt: string };
export default function MarkdownImage(props: Props) {
  const net = useServerNet();
  const { alt } = props;
  return (
    <Owned
      key={JSON.stringify([
        props.src,
        props.agentId,
        props.basePath,
        props.localPath,
        props.asset,
      ])}
      create={() => new ImagePreviewState(props)}
    >
      {(state) => {
        const { image, error: imageError } = state.loaded(net);
        const target = state.target,
          remoteHost = state.remoteHost,
          remote = !!remoteHost;
        const preview = state.preview(net);
        return (
          <span className="markdown-image">
            {image && !imageError ? (
              <button
                type="button"
                className="image-preview-button"
                aria-label={`Preview ${alt || "image"}`}
                onClick={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  state.open(net);
                }}
              >
                <img
                  src={image}
                  alt={alt}
                  loading="lazy"
                  referrerPolicy="no-referrer"
                  onError={() => state.fail(net)}
                />
              </button>
            ) : (
              <span className="markdown-image-placeholder">
                <span>{alt || "Image"}</span>
                {imageError ? (
                  <>
                    <span role="alert">{imageError}</span>
                    <button type="button" onClick={() => state.retry(net)}>
                      Retry image
                    </button>
                  </>
                ) : remote ? (
                  <>
                    <span>
                      External image. Load it to contact {remoteHost}.
                    </span>
                    <button type="button" onClick={state.allowRemote}>
                      Load image
                    </button>
                  </>
                ) : (
                  <span role="status">Load image…</span>
                )}
              </span>
            )}
            <FilePreview
              target={preview?.file ?? null}
              onClose={state.close}
              image={
                preview && !target
                  ? {
                      src: image,
                      alt,
                      onError: () => state.fail(net),
                    }
                  : undefined
              }
            />
          </span>
        );
      }}
    </Owned>
  );
}
