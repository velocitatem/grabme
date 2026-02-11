import { useEffect, useMemo, useState } from "react";

export type TimelineSegment = {
  id: string;
  label: string;
  start: number;
  end: number;
  color: string;
};

type DragAction =
  | {
      kind: "move";
      segmentId: string;
      anchorX: number;
      initialStart: number;
      initialEnd: number;
    }
  | {
      kind: "resize";
      segmentId: string;
      anchorX: number;
      initialStart: number;
      initialEnd: number;
    };

type TimelineTrackProps = {
  durationSecs: number;
  pixelsPerSecond: number;
  segments: TimelineSegment[];
  onSegmentChange: (segmentId: string, start: number, end: number) => void;
};

const MIN_SEGMENT_DURATION = 0.2;

export function TimelineTrack(props: TimelineTrackProps): JSX.Element {
  const { durationSecs, pixelsPerSecond, segments, onSegmentChange } = props;
  const [dragAction, setDragAction] = useState<DragAction | null>(null);

  const totalWidth = useMemo(
    () => Math.max(680, durationSecs * pixelsPerSecond),
    [durationSecs, pixelsPerSecond]
  );

  useEffect(() => {
    if (!dragAction) {
      return;
    }

    const onMouseMove = (event: MouseEvent) => {
      const deltaSecs = (event.clientX - dragAction.anchorX) / pixelsPerSecond;
      if (dragAction.kind === "move") {
        const width = dragAction.initialEnd - dragAction.initialStart;
        const start = clamp(dragAction.initialStart + deltaSecs, 0, Math.max(0, durationSecs - width));
        const end = start + width;
        onSegmentChange(dragAction.segmentId, start, end);
        return;
      }

      const end = clamp(
        dragAction.initialEnd + deltaSecs,
        dragAction.initialStart + MIN_SEGMENT_DURATION,
        durationSecs
      );
      onSegmentChange(dragAction.segmentId, dragAction.initialStart, end);
    };

    const onMouseUp = () => setDragAction(null);

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp, { once: true });

    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [dragAction, durationSecs, onSegmentChange, pixelsPerSecond]);

  return (
    <section className="timeline-shell">
      <header className="timeline-head">
        <span>Timeline</span>
        <small>{durationSecs.toFixed(1)}s</small>
      </header>

      <div className="timeline-scroll">
        <div className="timeline-ruler" style={{ width: `${totalWidth}px` }}>
          {buildRulerTicks(durationSecs).map((tick) => (
            <span key={tick} style={{ left: `${tick * pixelsPerSecond}px` }}>
              {tick}s
            </span>
          ))}
        </div>

        <div className="timeline-track" style={{ width: `${totalWidth}px` }}>
          {segments.map((segment) => {
            const left = segment.start * pixelsPerSecond;
            const width = Math.max(14, (segment.end - segment.start) * pixelsPerSecond);
            return (
              <article
                key={segment.id}
                className="timeline-segment"
                style={{ left: `${left}px`, width: `${width}px`, background: segment.color }}
                onMouseDown={(event) => {
                  event.preventDefault();
                  setDragAction({
                    kind: "move",
                    segmentId: segment.id,
                    anchorX: event.clientX,
                    initialStart: segment.start,
                    initialEnd: segment.end,
                  });
                }}
              >
                <span>{segment.label}</span>
                <button
                  type="button"
                  className="resize-handle"
                  onMouseDown={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    setDragAction({
                      kind: "resize",
                      segmentId: segment.id,
                      anchorX: event.clientX,
                      initialStart: segment.start,
                      initialEnd: segment.end,
                    });
                  }}
                />
              </article>
            );
          })}
        </div>
      </div>
    </section>
  );
}

function buildRulerTicks(durationSecs: number): number[] {
  const ticks: number[] = [];
  const maxTick = Math.ceil(durationSecs);
  for (let second = 0; second <= maxTick; second += 1) {
    ticks.push(second);
  }
  return ticks;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
