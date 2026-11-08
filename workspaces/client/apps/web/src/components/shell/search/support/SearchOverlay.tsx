export interface SearchSource {
  readonly opened: boolean;
  select(id: string, messageId?: string): void;
  close(): void;
  navigate(section: string, agent?: string): void;
}
