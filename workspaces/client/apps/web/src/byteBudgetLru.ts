// Weights are adapter-defined bytes; defaults bound retained cache accounting.
export class ByteBudgetLru<T> {
  private entries = new Map<string, { value: T; bytes: number }>();
  private bytes = 0;

  private readonly maxBytes: number;
  private readonly maxEntries: number;

  constructor(maxBytes = 4 * 1024 * 1024, maxEntries = 512) {
    this.maxBytes = maxBytes;
    this.maxEntries = maxEntries;
  }

  get(key: string): T | undefined {
    const entry = this.entries.get(key);
    if (!entry) return undefined;
    this.entries.delete(key);
    this.entries.set(key, entry);
    return entry.value;
  }

  set(key: string, value: T, bytes: number) {
    const previous = this.entries.get(key);
    if (previous) {
      this.bytes -= previous.bytes;
      this.entries.delete(key);
    }
    if (bytes > this.maxBytes || this.maxEntries < 1) return;
    this.entries.set(key, { value, bytes });
    this.bytes += bytes;
    while (this.bytes > this.maxBytes || this.entries.size > this.maxEntries) {
      const oldest = this.entries.keys().next().value!;
      this.bytes -= this.entries.get(oldest)!.bytes;
      this.entries.delete(oldest);
    }
  }
}
