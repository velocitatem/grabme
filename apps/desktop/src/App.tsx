import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TimelineSegment, TimelineTrack } from "./components/TimelineTrack";

type Viewport = {
  x: number;
  y: number;
  w: number;
  h: number;
};

type CameraKeyframe = {
  t: number;
  viewport: Viewport;
  easing: string;
  source: string;
};

type CursorMotionTrailConfig = {
  enabled: boolean;
  ghost_count: number;
  speed_threshold: number;
  frame_spacing: number;
};

type CursorConfig = {
  smoothing: string;
  smoothing_factor: number;
  size_multiplier: number;
  custom_asset: string | null;
  show_click_animation: boolean;
  motion_trail: CursorMotionTrailConfig;
};

type Timeline = {
  version: string;
  keyframes: CameraKeyframe[];
  effects: unknown[];
  cursor_config: CursorConfig;
  cuts: Array<{ start_secs: number; end_secs: number; reason: string }>;
};

type TimelineEditorBundle = {
  name: string;
  fps: number;
  duration_secs: number;
  timeline: Timeline;
};

export default function App(): JSX.Element {
  const [projectPath, setProjectPath] = useState("./recording");
  const [bundle, setBundle] = useState<TimelineEditorBundle | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string>("Load a project to begin");
  const [dirty, setDirty] = useState(false);
  const [zoom, setZoom] = useState(64);

  const keyframes = useMemo(() => {
    if (!bundle) {
      return [];
    }
    return [...bundle.timeline.keyframes].sort((a, b) => a.t - b.t);
  }, [bundle]);

  const segments = useMemo<TimelineSegment[]>(() => {
    if (!bundle || keyframes.length === 0) {
      return [];
    }

    return keyframes.map((frame, index) => {
      const nextTime = keyframes[index + 1]?.t ?? bundle.duration_secs;
      const zoomFactor = 1 / Math.max(frame.viewport.w, frame.viewport.h);
      const tint = Math.min(0.85, 0.35 + zoomFactor * 0.15);
      return {
        id: `segment-${index}`,
        label: `k${index + 1} ${Math.round(zoomFactor * 100)}%`,
        start: frame.t,
        end: Math.max(frame.t + 0.2, nextTime),
        color: `rgba(35, 179, 139, ${tint.toFixed(3)})`
      };
    });
  }, [bundle, keyframes]);

  async function loadTimelineBundle() {
    setError(null);
    setStatus("Loading timeline...");
    try {
      const data = await invoke<TimelineEditorBundle>("load_timeline_bundle", {
        projectPath
      });
      data.timeline.keyframes.sort((a, b) => a.t - b.t);
      setBundle(data);
      setDirty(false);
      setStatus(`Loaded ${data.name}`);
    } catch (loadError) {
      setError(String(loadError));
      setStatus("Failed to load timeline");
    }
  }

  async function saveTimelineBundle() {
    if (!bundle) {
      return;
    }
    setError(null);
    setStatus("Saving timeline...");
    try {
      await invoke("save_timeline_bundle", {
        projectPath,
        payload: { timeline: bundle.timeline }
      });
      setDirty(false);
      setStatus("Timeline saved");
    } catch (saveError) {
      setError(String(saveError));
      setStatus("Failed to save timeline");
    }
  }

  function updateSegment(segmentId: string, start: number, end: number) {
    if (!bundle) {
      return;
    }
    const index = Number(segmentId.replace("segment-", ""));
    if (Number.isNaN(index)) {
      return;
    }

    setBundle((prev) => {
      if (!prev) {
        return prev;
      }

      const timeline = structuredClone(prev.timeline);
      timeline.keyframes.sort((a, b) => a.t - b.t);
      const frames = timeline.keyframes;
      if (!frames[index]) {
        return prev;
      }

      const previousTime = index > 0 ? frames[index - 1].t : 0;
      const nextMax = index + 1 < frames.length ? frames[index + 1].t : prev.duration_secs;

      frames[index].t = clamp(start, previousTime, Math.max(previousTime, nextMax - 0.2));
      if (index + 1 < frames.length) {
        const minEnd = frames[index].t + 0.2;
        frames[index + 1].t = clamp(end, minEnd, prev.duration_secs);
      }

      timeline.keyframes = frames.sort((a, b) => a.t - b.t);
      return { ...prev, timeline };
    });

    setDirty(true);
    setStatus("Unsaved timeline edits");
  }

  function setHideMouseJitter(enabled: boolean) {
    if (!bundle) {
      return;
    }

    setBundle((prev) => {
      if (!prev) {
        return prev;
      }
      const timeline = structuredClone(prev.timeline);
      timeline.cursor_config.smoothing = enabled ? "ema" : "none";
      timeline.cursor_config.smoothing_factor = enabled
        ? Math.max(timeline.cursor_config.smoothing_factor, 0.35)
        : 0;
      return { ...prev, timeline };
    });
    setDirty(true);
  }

  const hideMouseJitter = Boolean(
    bundle &&
      bundle.timeline.cursor_config.smoothing !== "none" &&
      bundle.timeline.cursor_config.smoothing_factor > 0
  );

  return (
    <main className="editor-root">
      <header className="editor-header">
        <div>
          <h1>GrabMe Timeline Prototype</h1>
          <p>Record to analyze to tune keyframes to save</p>
        </div>
        <span className={dirty ? "status-chip dirty" : "status-chip"}>{status}</span>
      </header>

      <section className="control-row">
        <label>
          Project path
          <input
            value={projectPath}
            onChange={(event) => setProjectPath(event.target.value)}
            placeholder="./recording"
          />
        </label>

        <label>
          Zoom
          <input
            type="range"
            min={32}
            max={220}
            value={zoom}
            onChange={(event) => setZoom(Number(event.target.value))}
          />
        </label>

        <div className="button-group">
          <button type="button" onClick={loadTimelineBundle}>
            Load Timeline
          </button>
          <button type="button" onClick={saveTimelineBundle} disabled={!bundle || !dirty}>
            Save Timeline
          </button>
        </div>
      </section>

      {bundle ? (
        <>
          <TimelineTrack
            durationSecs={bundle.duration_secs}
            pixelsPerSecond={zoom}
            segments={segments}
            onSegmentChange={updateSegment}
          />

          <section className="inspector-grid">
            <article className="panel">
              <h2>Session</h2>
              <p>
                {bundle.name} · {bundle.fps}fps · {bundle.duration_secs.toFixed(1)}s
              </p>
            </article>

            <article className="panel">
              <h2>Cursor</h2>
              <label className="toggle-row">
                <input
                  type="checkbox"
                  checked={hideMouseJitter}
                  onChange={(event) => setHideMouseJitter(event.target.checked)}
                />
                Hide Mouse Jitter
              </label>
              <small>
                smoothing={bundle.timeline.cursor_config.smoothing} · factor=
                {bundle.timeline.cursor_config.smoothing_factor.toFixed(2)}
              </small>
            </article>

            <article className="panel">
              <h2>Keyframes</h2>
              <ul>
                {keyframes.map((frame, index) => (
                  <li key={`keyframe-${index}`}>
                    t={frame.t.toFixed(2)}s · vp {frame.viewport.x.toFixed(2)},{" "}
                    {frame.viewport.y.toFixed(2)} {frame.viewport.w.toFixed(2)}x
                    {frame.viewport.h.toFixed(2)}
                  </li>
                ))}
              </ul>
            </article>
          </section>
        </>
      ) : null}

      {error ? <p className="error-text">{error}</p> : null}
    </main>
  );
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
