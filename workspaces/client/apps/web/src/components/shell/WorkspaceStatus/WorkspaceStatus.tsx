import { Badge } from "@mantine/core";

const statusColors: Record<string, string> = {
  accepted: "teal",
  review: "orange",
  failed: "red",
  blocked: "red",
};

export function WorkspaceStatus({ value }: { value: string }) {
  return (
    <Badge size="xs" variant="light" color={statusColors[value] ?? "gray"}>
      {value.replaceAll("_", " ")}
    </Badge>
  );
}
