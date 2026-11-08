import type { SearchSource } from "../support/SearchOverlay.tsx";
import { SearchContent } from "../SearchContent/SearchContent.tsx";
import { Modal } from "@mantine/core";
import { observer } from "mobx-react-lite";

export default observer(function SearchOverlay({
  source,
}: {
  source: SearchSource;
}) {
  return (
    <Modal
      opened={source.opened}
      onClose={source.close}
      title="Search messages"
      size="lg"
    >
      <p className="workspace-muted">
        All messages and archived chats. Ctrl+K / ⌘K
      </p>
      {source.opened && <SearchContent source={source} />}
    </Modal>
  );
});
