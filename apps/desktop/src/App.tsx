import { useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";

type SmoothAlgorithm = "ema" | "bezier" | "kalman";

type InputEvent = {
  t: number;
  type: "pointer" | "click" | "scroll" | "key" | "window_focus";
  x?: number;
  y?: number;
  state?: "down" | "up";
};

type PointerSample = { t: number; x: number; y: number };

type LoadedProjectBundle = {
  name: string;
  width: number;
  height: number;
  fps: number;
  screen_path: string | null;
  events: InputEvent[];
};

const CURSOR_SVG =
  "data:image/svg+xml;utf8," +
  encodeURIComponent(
    `<svg width="22" height="30" viewBox="0 0 22 30" xmlns="http://www.w3.org/2000/svg"><path d="M1 1L1 26L7.6 20.2L11.8 28.2L15 26.6L10.7 18.6L20.8 18.6L1 1Z" fill="#F7FAFF" stroke="#0A0A0A" stroke-width="1.2"/></svg>`
  );

export default function App(): JSX.Element {
  const videoRef = useRef<HTMLVideoElement | null>(null);

  const [projectPath, setProjectPath] = useState("./recording");
  const [bundle, setBundle] = useState<LoadedProjectBundle | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [timeSecs, setTimeSecs] = useState(0);
  const [algorithm, setAlgorithm] = useState<SmoothAlgorithm>("ema");
  const [strength, setStrength] = useState(0.35);
  const [scale, setScale] = useState(1);
  const [hudOpen, setHudOpen] = useState(true);

  useEffect(() => {
    let raf = 0;
    const tick = () => {
      if (videoRef.current) {
        setTimeSecs(videoRef.current.currentTime || 0);
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);

  async function loadProject() {
    setError(null);
    try {
      const data = await invoke<LoadedProjectBundle>("load_project_bundle", { projectPath });
      setBundle(data);
    } catch (e) {
      setError(String(e));
    }
  }

  const pointers = useMemo(() => {
    if (!bundle) {
      return [] as PointerSample[];
    }
    return bundle.events
      .filter((event) =>
        (event.type === "pointer" || event.type === "click" || event.type === "scroll") &&
        typeof event.x === "number" &&
        typeof event.y === "number"
      )
      .map((event) => ({ t: event.t / 1e9, x: clamp01(event.x as number), y: clamp01(event.y as number) }));
  }, [bundle]);

  const clicks = useMemo(() => {
    if (!bundle) {
      return [] as PointerSample[];
    }
    return bundle.events
      .filter((event) => event.type === "click" && event.state === "down" && typeof event.x === "number" && typeof event.y === "number")
      .map((event) => ({ t: event.t / 1e9, x: clamp01(event.x as number), y: clamp01(event.y as number) }));
  }, [bundle]);

  const smoothed = useMemo(() => {
    if (algorithm === "ema") {
      return smoothEma(pointers, strength);
    }
    if (algorithm === "bezier") {
      return smoothBezier(pointers, strength);
    }
    return smoothKalman(pointers, strength);
  }, [algorithm, pointers, strength]);

  const pointer = useMemo(() => sampleAtTime(smoothed, timeSecs), [smoothed, timeSecs]);
  const clickPulse = useMemo(() => {
    const click = sampleAtTime(clicks, timeSecs);
    if (!click) {
      return null;
    }
    return clicks.some((c) => Math.abs(c.t - timeSecs) <= 0.2) ? click : null;
  }, [clicks, timeSecs]);

  const videoSrc = useMemo(() => {
    if (!bundle?.screen_path) {
      return undefined;
    }
    return convertFileSrc(bundle.screen_path);
  }, [bundle]);

  return (
    <main className="overlay-root">
      <video ref={videoRef} className="video" controls src={videoSrc} />

      {pointer ? (
        <div
          className="cursor"
          style={{
            left: `${pointer.x * 100}%`,
            top: `${pointer.y * 100}%`,
            transform: `translate(-12%, -8%) scale(${scale})`
          }}
        >
          <img src={CURSOR_SVG} alt="cursor" />
        </div>
      ) : null}

      {clickPulse ? (
        <div className="click" style={{ left: `${clickPulse.x * 100}%`, top: `${clickPulse.y * 100}%` }} />
      ) : null}

      <button className="hud-toggle" onClick={() => setHudOpen((v) => !v)}>
        {hudOpen ? "-" : "+"}
      </button>

      {hudOpen ? (
        <section className="hud">
          <input
            value={projectPath}
            onChange={(e) => setProjectPath(e.target.value)}
            placeholder="./recording"
            aria-label="project path"
          />
          <button onClick={loadProject}>Load</button>
          <select value={algorithm} onChange={(e) => setAlgorithm(e.target.value as SmoothAlgorithm)}>
            <option value="ema">EMA</option>
            <option value="bezier">Bezier</option>
            <option value="kalman">Kalman</option>
          </select>
          <input type="range" min={0.05} max={0.95} step={0.01} value={strength} onChange={(e) => setStrength(Number(e.target.value))} />
          <input type="range" min={0.8} max={2} step={0.05} value={scale} onChange={(e) => setScale(Number(e.target.value))} />
          <small>{bundle?.name ?? "no project"} | t={timeSecs.toFixed(2)}s | n={pointers.length}</small>
          {error ? <small className="error">{error}</small> : null}
        </section>
      ) : null}
    </main>
  );
}

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

function sampleAtTime(points: PointerSample[], timeSecs: number): PointerSample | null {
  if (points.length === 0) {
    return null;
  }
  let low = 0;
  let high = points.length - 1;
  while (low < high) {
    const mid = Math.ceil((low + high) / 2);
    if (points[mid].t <= timeSecs) {
      low = mid;
    } else {
      high = mid - 1;
    }
  }
  return points[low] ?? null;
}

function smoothEma(points: PointerSample[], strength: number): PointerSample[] {
  if (points.length === 0) {
    return [];
  }
  const alpha = clamp01(1 - strength);
  const out: PointerSample[] = [];
  let x = points[0].x;
  let y = points[0].y;
  for (const point of points) {
    x = alpha * point.x + (1 - alpha) * x;
    y = alpha * point.y + (1 - alpha) * y;
    out.push({ ...point, x, y });
  }
  return out;
}

function smoothBezier(points: PointerSample[], strength: number): PointerSample[] {
  if (points.length < 3) {
    return points;
  }
  const pull = clamp01(strength);
  const out: PointerSample[] = [points[0]];
  for (let i = 1; i < points.length - 1; i += 1) {
    const a = points[i - 1];
    const b = points[i];
    const c = points[i + 1];
    const cx = (a.x + c.x) / 2;
    const cy = (a.y + c.y) / 2;
    out.push({
      ...b,
      x: b.x * (1 - pull) + cx * pull,
      y: b.y * (1 - pull) + cy * pull
    });
  }
  out.push(points[points.length - 1]);
  return out;
}

function smoothKalman(points: PointerSample[], strength: number): PointerSample[] {
  if (points.length === 0) {
    return [];
  }
  const q = 0.001 + (1 - strength) * 0.01;
  const r = 0.001 + strength * 0.04;
  const out: PointerSample[] = [];

  let x = points[0].x;
  let y = points[0].y;
  let px = 1;
  let py = 1;

  for (const point of points) {
    px += q;
    py += q;
    const kx = px / (px + r);
    const ky = py / (py + r);
    x = x + kx * (point.x - x);
    y = y + ky * (point.y - y);
    px = (1 - kx) * px;
    py = (1 - ky) * py;
    out.push({ ...point, x, y });
  }

  return out;
}
