import { Button } from "@mantine/core";
import { Code, Eye } from "lucide-react";

export function SourceToggle({
  source: selected,
  onChange,
  rich = false,
}: {
  source: boolean;
  onChange: (source: boolean) => void;
  rich?: boolean;
}) {
  return (
    <div
      className={rich ? "rich-preview-modes" : "file-preview-toolbar"}
      role={rich ? "group" : undefined}
      aria-label={rich ? "Preview display" : "File view"}
    >
      {[false, true].map((source) => {
        const label = source ? "Source" : "Preview";
        const props = {
          "aria-pressed": selected === source,
          onClick: () => onChange(source),
        };
        return rich ? (
          <Button
            key={label}
            {...props}
            size="compact-xs"
            variant={props["aria-pressed"] ? "light" : "subtle"}
            leftSection={source ? <Code size={13} /> : <Eye size={13} />}
          >
            {label}
          </Button>
        ) : (
          <button key={label} {...props} type="button">
            {label}
          </button>
        );
      })}
    </div>
  );
}
