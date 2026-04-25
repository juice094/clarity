interface DiffLine {
  tag: "equal" | "delete" | "insert";
  content: string;
}

interface DiffHunk {
  old_start: number;
  new_start: number;
  lines: DiffLine[];
}

function DiffViewer({ hunks }: { hunks: DiffHunk[] }) {
  return (
    <div className="diff-viewer">
      {hunks.map((hunk, hi) => (
        <div key={hi} className="diff-hunk">
          <div className="diff-hunk-header">
            @@ -{hunk.old_start} +{hunk.new_start} @@
          </div>
          {hunk.lines.map((line, li) => (
            <div key={li} className={`diff-line ${line.tag}`}>
              <span className="diff-marker">
                {line.tag === "equal" ? " " : line.tag === "delete" ? "-" : "+"}
              </span>
              <span className="diff-content">{line.content}</span>
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

export default DiffViewer;
export type { DiffHunk, DiffLine };
