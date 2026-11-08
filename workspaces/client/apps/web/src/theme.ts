import {
  createTheme,
  virtualColor,
  defaultVariantColorsResolver,
  Button,
  ActionIcon,
  TextInput,
  NativeSelect,
  Modal,
  Tooltip,
  Menu,
  Popover,
  Combobox,
  Drawer,
  Badge,
  SegmentedControl,
} from "@mantine/core";

import {
  layoutValues,
  layerZIndex,
} from "./layout/layout-contract.generated.ts";
export { layerZIndex };
export const modalSizes = { settings: layoutValues.modal.settings };

export const theme = createTheme({
  // Ink is the one primary action color: near-black in light, near-white in dark.
  primaryColor: "ink",
  primaryShade: { light: 7, dark: 4 },
  autoContrast: true,
  // Mantine's component resolver chooses text using the light primary shade.
  // The generated contrast variable follows both primaryShade schemes.
  variantColorResolver: (input) => {
    // Uncolored filled actions use ink; an uncolored subtle action stays neutral.
    const colors = defaultVariantColorsResolver(
      input.variant === "subtle" && input.color === input.theme.primaryColor
        ? { ...input, color: "gray" }
        : input,
    );
    return input.variant === "filled" &&
      input.color === input.theme.primaryColor
      ? { ...colors, color: "var(--mantine-primary-color-contrast)" }
      : colors;
  },
  defaultRadius: "sm",
  // Control, card, dialog, and large-card corners, in rem (6, 8, 12, 16 px).
  radius: {
    xs: "0.25rem",
    sm: "0.375rem",
    md: "0.5rem",
    lg: "0.75rem",
    xl: "1rem",
  },
  // Stacked elevation owned by studio-theme.css; dark uses hairline rings.
  shadows: {
    xs: "var(--studio-shadow-xs)",
    sm: "var(--studio-shadow-sm)",
    md: "var(--studio-shadow-md)",
    lg: "var(--studio-shadow-lg)",
    xl: "var(--studio-shadow-lg)",
  },
  fontFamily: "var(--studio-font-sans)",
  fontFamilyMonospace: "var(--studio-font-mono)",
  fontSizes: {
    xs: "var(--studio-text-meta)",
    sm: "var(--studio-text-body)",
    md: "var(--studio-text-body)",
    lg: "var(--studio-text-title)",
    xl: "var(--studio-text-section)",
  },
  headings: { fontFamily: "var(--studio-font-sans)", fontWeight: "600" },
  colors: {
    // Each scheme reads its own ink ramp. Shade 7 (light) and 4 (dark) fill
    // primary actions; the next shade is their hover.
    ink: virtualColor({ name: "ink", light: "inkLight", dark: "inkDark" }),
    inkLight: [
      "#fafafa",
      "#f2f2f2",
      "#ebebeb",
      "#d4d4d4",
      "#a1a1a1",
      "#8f8f8f",
      "#666666",
      "#171717",
      "#383838",
      "#0a0a0a",
    ],
    inkDark: [
      "#ffffff",
      "#fafafa",
      "#f5f5f5",
      "#f2f2f2",
      "#ededed",
      "#cccccc",
      "#a1a1a1",
      "#7d7d7d",
      "#454545",
      "#262626",
    ],
    // Neutral light ramp; Mantine reads gray-0 hovers and gray-4 borders.
    gray: [
      "#fafafa",
      "#f2f2f2",
      "#ebebeb",
      "#e5e5e5",
      "#d4d4d4",
      "#a1a1a1",
      "#8f8f8f",
      "#666666",
      "#4d4d4d",
      "#171717",
    ],
    // OLED dark ramp: dark[7] page and dark[6] fields stay true black or
    // near black so unlit pixels draw no power; dark[4] owns hairlines.
    dark: [
      "#ededed",
      "#c9c9c9",
      "#a1a1a1",
      "#7d7d7d",
      "#262626",
      "#1a1a1a",
      "#0a0a0a",
      "#000000",
      "#000000",
      "#000000",
    ],
    // Semantic ramps. Shade 7 labels small text on light surfaces and shade 4
    // on black; both meet AA (design/contrast.test.ts).
    blue: [
      "#eef6ff",
      "#d3e5ff",
      "#a9cdff",
      "#7cb4ff",
      "#52a8ff",
      "#2b8eff",
      "#0070f3",
      "#0761d1",
      "#0a4fa8",
      "#0b3d80",
    ],
    red: [
      "#fff0f0",
      "#f7d4d6",
      "#ffb3b5",
      "#ff8a8d",
      "#ff6166",
      "#ff3b41",
      "#ee0000",
      "#c50000",
      "#a00000",
      "#7a0000",
    ],
    green: [
      "#effbf2",
      "#d3f5dc",
      "#a8ebba",
      "#7cdc98",
      "#62c073",
      "#3aa556",
      "#1d8a3f",
      "#107d32",
      "#0b6327",
      "#074b1d",
    ],
    yellow: [
      "#fff8e6",
      "#ffefcf",
      "#ffdf9e",
      "#ffcb66",
      "#f5a623",
      "#e8930c",
      "#c97300",
      "#ab570a",
      "#8a4507",
      "#6b3505",
    ],
    orange: [
      "#fff8e6",
      "#ffefcf",
      "#ffdf9e",
      "#ffcb66",
      "#f5a623",
      "#e8930c",
      "#c97300",
      "#ab570a",
      "#8a4507",
      "#6b3505",
    ],
    violet: [
      "#f6f0ff",
      "#e8dcfa",
      "#d8ccf1",
      "#c4aaf0",
      "#b18cf2",
      "#9461e6",
      "#7928ca",
      "#6420ac",
      "#4c2889",
      "#3a1f6b",
    ],
  },
  components: {
    Button: Button.extend({
      styles: {
        label: { textBoxTrim: "none", height: "auto", lineHeight: 1.4 },
        inner: { transform: "none", transition: "opacity 100ms ease" },
      },
      // Actions are secondary buttons by default: a surface with a hairline.
      // Toolbar and composer chrome opts into "subtle".
      defaultProps: { size: "sm", variant: "default", fw: 500 },
    }),
    ActionIcon: ActionIcon.extend({
      defaultProps: {
        size: "var(--studio-action-size)",
        variant: "subtle",
        color: "gray",
      },
    }),
    TextInput: TextInput.extend({ defaultProps: { size: "sm" } }),
    NativeSelect: NativeSelect.extend({ defaultProps: { size: "sm" } }),
    Modal: Modal.extend({
      defaultProps: {
        centered: true,
        radius: "lg",
        padding: "lg",
        shadow: "lg",
        // No backdrop blur: design/components.css owns the backdrop color.
        overlayProps: { backgroundOpacity: 1, blur: 0 },
        closeButtonProps: { "aria-label": "Close" },
      },
    }),
    Drawer: Drawer.extend({
      defaultProps: { overlayProps: { backgroundOpacity: 1, blur: 0 } },
    }),
    Menu: Menu.extend({
      defaultProps: { shadow: "lg", radius: "md" },
      styles: { dropdown: { minWidth: "var(--studio-menu-min)" } },
    }),
    Popover: Popover.extend({ defaultProps: { shadow: "lg", radius: "md" } }),
    Combobox: Combobox.extend({
      defaultProps: { shadow: "lg", radius: "md" },
    }),
    // One compact height for every segmented choice.
    SegmentedControl: SegmentedControl.extend({ defaultProps: { size: "xs" } }),
    Badge: Badge.extend({ defaultProps: { radius: "xl", variant: "light" } }),
    Tooltip: Tooltip.extend({
      defaultProps: { openDelay: 500, withArrow: true, radius: "sm" },
    }),
  },
});
