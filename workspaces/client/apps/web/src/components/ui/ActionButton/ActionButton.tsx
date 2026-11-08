import {
  Button,
  useMantineTheme,
  type ButtonProps,
  type ElementProps,
} from "@mantine/core";

/** Preserve native button props and accessible names from each caller. */
export function ActionButton({
  actionRole = "quiet",
  ...props
}: ActionButtonProps) {
  const theme = useMantineTheme();
  const variant =
    actionRole === "quiet"
      ? "subtle"
      : actionRole === "secondary"
        ? "default"
        : "filled";
  const color =
    actionRole === "destructive"
      ? "red"
      : actionRole === "primary"
        ? theme.primaryColor
        : "gray";
  return <Button variant={variant} color={color} {...props} />;
}

export type ActionRole = "primary" | "secondary" | "quiet" | "destructive";

export interface ActionButtonProps
  extends ButtonProps, ElementProps<"button", keyof ButtonProps> {
  actionRole?: ActionRole;
}
