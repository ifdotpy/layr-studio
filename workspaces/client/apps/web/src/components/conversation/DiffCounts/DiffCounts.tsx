import {
  diffTotals,
  type FileDiff as RecordedDiff,
} from "../support/fileChangeModel.ts";

export function DiffCounts({ files }: { files: RecordedDiff[] }) {
  if (!files.length || files.some((file) => !file.parsed)) return null;
  const { added, removed } = diffTotals(files);
  return (
    <span
      className="file-diff-counts"
      aria-label={`${added} added lines, ${removed} removed lines`}
    >
      (<span className="file-diff-added">+{added}</span>{" "}
      <span className="file-diff-removed">-{removed}</span>)
    </span>
  );
}
