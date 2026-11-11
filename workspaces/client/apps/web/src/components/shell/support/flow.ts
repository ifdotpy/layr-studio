export type ShellFlow<Result = void, Value = unknown> = Generator<
  Promise<unknown>,
  Result,
  Value
>;
