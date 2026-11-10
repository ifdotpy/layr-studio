void import("./mount.tsx").catch((error) => {
  const container = document.getElementById("root");
  if (container)
    container.textContent = `Studio could not recover its settings: ${String(error)}`;
});
