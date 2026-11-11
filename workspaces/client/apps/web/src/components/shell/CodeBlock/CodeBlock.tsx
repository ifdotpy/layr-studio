import { Owned } from "../../ui/Owned/Owned.tsx";
import { useState } from "react";
import { CopyFeedback } from "../../../stores/CopyFeedback.ts";
import { Button } from "@mantine/core";

export default function CodeBlock({
  source,
  language = "text",
}: {
  source: string;
  language?: string | undefined;
}) {
  const [expanded, setExpanded] = useState(false);
  return (
    <Owned key={source} create={() => new CopyFeedback()}>
      {(copy) => (
        <section className="code-block" aria-label={`${language} code`}>
          <div className="rich-preview-toolbar">
            <span className="rich-preview-label">{language}</span>
            <Button
              size="compact-xs"
              variant="subtle"
              onClick={() => void copy.copy(source, source)}
            >
              {copy.copied(source) ? "Copied" : "Copy code"}
            </Button>
            <Button
              size="compact-xs"
              variant="subtle"
              aria-expanded={expanded}
              onClick={() => setExpanded(!expanded)}
            >
              {expanded ? "Collapse code" : "Expand code"}
            </Button>
          </div>
          {copy.failed && (
            <p className="rich-preview-error" role="alert">
              Cannot copy. Select the code to copy it.
            </p>
          )}
          <pre className="code-block-content" data-expanded={expanded}>
            <code>{source}</code>
          </pre>
        </section>
      )}
    </Owned>
  );
}
