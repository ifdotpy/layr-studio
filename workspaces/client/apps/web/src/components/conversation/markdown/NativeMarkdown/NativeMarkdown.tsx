import { createElement, memo, type ReactNode } from "react";
import { observer } from "mobx-react-lite";
import { markdownRead } from "./markdownRead.ts";
import CodeBlock from "../../../shell/CodeBlock/CodeBlock.tsx";
import MarkdownImage from "../../MarkdownImage/MarkdownImage.tsx";
import RichPreview from "../../RichPreview/RichPreview.tsx";

import type {
  MarkdownNode,
  MarkdownTree as Tree,
} from "../../../../generated/rustOwned.ts";
type Context = { agentId?: string | undefined; basePath?: string | undefined };
const textContent = (node: MarkdownNode): string =>
  (node.text || "") + (node.children || []).map(textContent).join("");

// Rust supplies an inert tree with an allowlisted tag/attribute vocabulary.
// Both native and WASM hosts use the same Rust parser and sentence segmenter.
export const NativeMarkdownBlock = memo(function NativeMarkdownBlock({
  block,
  agentId,
  basePath,
}: { block: Tree } & Context) {
  function render(node: MarkdownNode, key: string): ReactNode {
    if (!node.tag) {
      if (!node.sentences?.length) return node.text || "";
      return node.sentences.map((sentence) => sentence.text).join("");
    }
    if (node.tag === "img")
      return (
        <MarkdownImage
          key={key}
          src={node.props?.src || ""}
          alt={node.props?.alt || ""}
          agentId={agentId}
          basePath={basePath}
        />
      );
    const code = node.children?.[0];
    if (node.tag === "pre" && code?.tag === "code")
      return (
        <CodeBlock
          key={key}
          source={textContent(code)}
          language={code.props?.className?.match(/(?:^|\s)language-(\S+)/)?.[1]}
        />
      );
    const children = node.children?.map((child, index) =>
      render(child, `${key}.${index}`),
    );
    return createElement(
      node.tag,
      { ...node.props, key },
      children?.length ? children : undefined,
    );
  }
  return (
    <div className="markdown-block">
      {block.nodes.map((node, index) => render(node, String(index)))}
    </div>
  );
});

export default observer(function NativeMarkdown({
  text,
  streaming,
  agentId,
  basePath,
}: {
  text: string;
  streaming: boolean;
} & Context) {
  const read = streaming ? undefined : markdownRead(text);
  const blocks = read?.blocks;
  const error = read?.error;
  return (
    <>
      {!streaming && error && <p role="alert">{error}</p>}
      {blocks ? (
        blocks.map((block, index) =>
          "kind" in block ? (
            <RichPreview key={index} kind={block.kind} source={block.source} />
          ) : (
            <NativeMarkdownBlock
              key={index}
              block={block}
              agentId={agentId}
              basePath={basePath}
            />
          ),
        )
      ) : (
        <div className="markdown-block" style={{ whiteSpace: "pre-wrap" }}>
          {text}
        </div>
      )}
    </>
  );
});
