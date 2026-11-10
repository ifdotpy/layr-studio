import { ConversationView } from "../ConversationView/ConversationView.tsx";
import { observer } from "mobx-react-lite";

const ObservedConversation = observer(ConversationView);

export default ObservedConversation;
