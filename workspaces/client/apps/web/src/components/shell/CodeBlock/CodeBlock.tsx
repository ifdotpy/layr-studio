import { useState } from "react";
import { Button } from "@mantine/core";

import { copyText } from "../../../clipboard/clipboard.ts";

export default function CodeBlock({
  source,
  language = "text",
}: {
  source: string;
  language?: string | undefined;
}) {
  const [expanded, setExpanded] = useState(false);
  const [result, setResult] = useState<{
    source: string;
    error: string;
  } | null>(null);
  const copied = result?.source === source && !result.error;
  const error = result?.source === source ? result.error : "";
  return (
    <section className="code-block" aria-label={`${language} code`}>
      <div className="rich-preview-toolbar">
        <span className="rich-preview-label">{language}</span>
        <Button
          size="compact-xs"
          variant="subtle"
          onClick={async () => {
            try {
              await copyText(source);
              setResult({ source, error: "" });
            } catch {
              setResult({
                source,
                error: "Cannot copy. Select the code to copy it.",
              });
            }
          }}
        >
          {copied ? "Copied" : "Copy code"}
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
      {error && (
        <p className="rich-preview-error" role="alert">
          {error}
        </p>
      )}
      <pre className="code-block-content" data-expanded={expanded}>
        <code>{source}</code>
      </pre>
    </section>
  );
}
