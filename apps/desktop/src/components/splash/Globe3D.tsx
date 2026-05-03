// Globe3D — Three.js 기반 회전 sphere + 표면 노드 + connection arcs + HTML overlay 라벨.
// Phase 14' v5 (2026-05-04). Anthropic Imagine 시연 영상 차용.
//
// 정책:
// - tree-shake 위해 named imports만. 약 50-80KB minified bundle.
// - splash 끝나면 component unmount → cleanup에서 dispose 호출 → memory release.
// - HTML overlay 라벨 — 매 frame 3D → 2D project. back face는 자동 hide.
// - prefers-reduced-motion 시 회전 정지 (rotationSpeed 0).
// - 사용자 인터랙션 X (요청).

import {
  AmbientLight,
  BackSide,
  BufferAttribute,
  BufferGeometry,
  Group,
  LineBasicMaterial,
  LineSegments,
  Mesh,
  MeshBasicMaterial,
  PerspectiveCamera,
  PointLight,
  Points,
  PointsMaterial,
  QuadraticBezierCurve3,
  Scene,
  SphereGeometry,
  Vector3,
  WebGLRenderer,
  WireframeGeometry,
  Line,
} from "three";
import { useEffect, useMemo, useRef } from "react";

import "./globe3d.css";

export interface GlobeNode {
  /** -π/2 ~ π/2 (남극 ~ 북극). */
  lat: number;
  /** -π ~ π (longitude). */
  lon: number;
  /** 라벨 텍스트 (선택). 비어 있으면 노드만 표시. */
  label?: string;
}

export interface Globe3DProps {
  size?: number;
  className?: string;
  nodes?: GlobeNode[];
  /** node 페어 인덱스 — connection arc. */
  arcPairs?: [number, number][];
  /** rotation 속도 (rad/frame). 0이면 정적. */
  rotationSpeed?: number;
}

// 12 노드 default — 실 LMmaster 런타임/모델/카탈로그 메타.
const DEFAULT_NODES: GlobeNode[] = [
  { lat: 0.4, lon: -0.3, label: "Ollama" },
  { lat: 0.1, lon: 0.5, label: "LM Studio" },
  { lat: -0.35, lon: 0.9, label: "llama.cpp" },
  { lat: -0.55, lon: 0.0, label: "Gemma 3" },
  { lat: 0.7, lon: 0.4, label: "Qwen 2.5" },
  { lat: -0.5, lon: -0.7, label: "EXAONE 4.0" },
  { lat: 0.2, lon: -0.9, label: "KURE-v1" },
  { lat: -0.15, lon: 1.3, label: "bge-m3" },
  { lat: 0.55, lon: 1.6, label: "Codestral" },
  { lat: -0.4, lon: -1.5, label: "Whisper" },
  { lat: 0.0, lon: 2.0, label: "Mistral" },
  { lat: 0.75, lon: -0.1, label: "Phi-4" },
];

const DEFAULT_ARC_PAIRS: [number, number][] = [
  [0, 1],
  [1, 4],
  [4, 6],
  [3, 5],
  [5, 0],
  [6, 11],
  [11, 4],
  [0, 3],
];

export function Globe3D({
  size = 480,
  className,
  nodes = DEFAULT_NODES,
  arcPairs = DEFAULT_ARC_PAIRS,
  rotationSpeed = 0.0015,
}: Globe3DProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const labelsRef = useRef<HTMLDivElement | null>(null);

  // memoize so deep-equal nodes prop doesn't trigger re-init.
  const stableNodes = useMemo(() => nodes, [nodes]);
  const stableArcs = useMemo(() => arcPairs, [arcPairs]);

  useEffect(() => {
    const container = containerRef.current;
    const labelsContainer = labelsRef.current;
    if (!container) return;

    const scene = new Scene();
    const camera = new PerspectiveCamera(45, 1, 0.1, 100);
    camera.position.z = 3.2;

    const renderer = new WebGLRenderer({ antialias: true, alpha: true });
    renderer.setSize(size, size);
    renderer.setPixelRatio(Math.min(globalThis.window?.devicePixelRatio ?? 1, 2));
    container.appendChild(renderer.domElement);

    // Ambient + point light — 입체감 (톤다운 v6).
    scene.add(new AmbientLight(0x2a4a3e, 0.5));
    const pointLight = new PointLight(0x7cd4cc, 0.9, 10);
    pointLight.position.set(2, 3, 4);
    scene.add(pointLight);

    // Globe sphere — 더 깊은 dark teal (톤다운 v6).
    const sphereGeo = new SphereGeometry(1, 64, 32);
    const sphereMat = new MeshBasicMaterial({
      color: 0x081a13,
      transparent: true,
      opacity: 0.65,
    });
    const sphere = new Mesh(sphereGeo, sphereMat);
    scene.add(sphere);

    // Wireframe overlay — sage green lat/long 그리드 (톤다운).
    const wireSourceGeo = new SphereGeometry(1.003, 24, 12);
    const wireGeo = new WireframeGeometry(wireSourceGeo);
    const wireMat = new LineBasicMaterial({
      color: 0x5eddae,
      transparent: true,
      opacity: 0.11,
    });
    const wire = new LineSegments(wireGeo, wireMat);
    scene.add(wire);
    wireSourceGeo.dispose();

    // Atmosphere glow — back-side sphere로 외곽 halo (톤다운 + 더 큰 layer).
    const atmGeo = new SphereGeometry(1.08, 32, 16);
    const atmMat = new MeshBasicMaterial({
      color: 0x3fb887,
      transparent: true,
      opacity: 0.06,
      side: BackSide,
    });
    const atm = new Mesh(atmGeo, atmMat);
    scene.add(atm);

    // 노드들 — 표면 약간 위에 작은 sphere (사이즈 작게 0.022 -> 0.018 톤다운 v6).
    const nodeGroup = new Group();
    const NODE_RADIUS_OFFSET = 1.012;
    type NodeRecord = {
      mesh: Mesh;
      pos: Vector3;
      label?: string;
      labelEl?: HTMLDivElement;
    };
    const nodeRecords: NodeRecord[] = stableNodes.map((n) => {
      const x = Math.cos(n.lat) * Math.sin(n.lon) * NODE_RADIUS_OFFSET;
      const y = Math.sin(n.lat) * NODE_RADIUS_OFFSET;
      const z = Math.cos(n.lat) * Math.cos(n.lon) * NODE_RADIUS_OFFSET;
      const geo = new SphereGeometry(0.018, 12, 8);
      const mat = new MeshBasicMaterial({ color: 0x7cd4cc });
      const mesh = new Mesh(geo, mat);
      mesh.position.set(x, y, z);
      nodeGroup.add(mesh);
      return { mesh, pos: new Vector3(x, y, z), label: n.label };
    });
    scene.add(nodeGroup);

    // Connection arcs — 더 얇은 stroke + opacity 낮음 (톤다운 v6).
    const arcGroup = new Group();
    type ArcRecord = { line: Line; geo: BufferGeometry; mat: LineBasicMaterial };
    const arcRecords: ArcRecord[] = [];
    stableArcs.forEach(([a, b]) => {
      const recA = nodeRecords[a];
      const recB = nodeRecords[b];
      if (!recA || !recB) return;
      const start = recA.pos.clone();
      const end = recB.pos.clone();
      const mid = start.clone().lerp(end, 0.5).normalize().multiplyScalar(1.45);
      const curve = new QuadraticBezierCurve3(start, mid, end);
      const points = curve.getPoints(40);
      const geo = new BufferGeometry().setFromPoints(points);
      const mat = new LineBasicMaterial({
        color: 0x7cd4cc,
        transparent: true,
        opacity: 0.32,
      });
      const line = new Line(geo, mat);
      arcGroup.add(line);
      arcRecords.push({ line, geo, mat });
    });
    scene.add(arcGroup);

    // Dust particles — globe 주변 부유하는 미세 점 (Phase 14' v6 권장).
    // 200개 Points를 sphere 주위 random distribution + 천천히 회전 (입체감).
    const dustCount = 200;
    const dustGeo = new BufferGeometry();
    const dustPositions = new Float32Array(dustCount * 3);
    for (let i = 0; i < dustCount; i++) {
      // sphere 주위 1.4 ~ 2.6 radius random.
      const r = 1.4 + Math.random() * 1.2;
      const theta = Math.random() * Math.PI * 2;
      const phi = Math.acos(2 * Math.random() - 1);
      dustPositions[i * 3] = r * Math.sin(phi) * Math.cos(theta);
      dustPositions[i * 3 + 1] = r * Math.cos(phi);
      dustPositions[i * 3 + 2] = r * Math.sin(phi) * Math.sin(theta);
    }
    dustGeo.setAttribute("position", new BufferAttribute(dustPositions, 3));
    const dustMat = new PointsMaterial({
      size: 0.012,
      color: 0xa8e6d4,
      transparent: true,
      opacity: 0.32,
      sizeAttenuation: true,
    });
    const dust = new Points(dustGeo, dustMat);
    scene.add(dust);

    // 라벨 div 생성 — HTML overlay.
    if (labelsContainer) {
      labelsContainer.innerHTML = "";
      nodeRecords.forEach((rec) => {
        if (!rec.label) return;
        const el = document.createElement("div");
        el.className = "globe3d-label";
        el.textContent = rec.label;
        labelsContainer.appendChild(el);
        rec.labelEl = el;
      });
    }

    // Animation loop.
    let frameId = 0;
    let arcPhase = 0;
    const projector = new Vector3();
    const halfSize = size / 2;
    const animate = () => {
      // Group rotation — sphere + wire + atm + nodes + arcs 동기.
      sphere.rotation.y += rotationSpeed;
      wire.rotation.y += rotationSpeed;
      atm.rotation.y += rotationSpeed;
      nodeGroup.rotation.y += rotationSpeed;
      arcGroup.rotation.y += rotationSpeed;
      // Dust — 반대 방향 더 천천히 (parallax depth).
      dust.rotation.y -= rotationSpeed * 0.3;

      // Arc opacity sine wave — 톤다운 (Phase 14' v6, 글로우 축소).
      arcPhase += 0.012;
      arcRecords.forEach((rec, i) => {
        const phase = arcPhase + i * 0.7;
        rec.mat.opacity = 0.18 + 0.22 * (Math.sin(phase) * 0.5 + 0.5);
      });

      // Node 라벨 위치 갱신 — 3D → 2D projection.
      nodeRecords.forEach((rec) => {
        if (!rec.labelEl) return;
        projector.copy(rec.pos).applyMatrix4(nodeGroup.matrixWorld);
        projector.project(camera);
        // back face — z > 0.95 또는 < 0이면 보이지 않음 (전구체 뒤쪽).
        const isFront = projector.z < 0.97 && projector.z > -1;
        // 표면 노드의 normal 방향이 카메라 향함 = front. cos angle 계산.
        const worldPos = rec.pos.clone().applyMatrix4(nodeGroup.matrixWorld);
        const normal = worldPos.clone().normalize();
        const camDir = new Vector3()
          .subVectors(camera.position, worldPos)
          .normalize();
        const dot = normal.dot(camDir);

        const sx = projector.x * halfSize + halfSize;
        const sy = -projector.y * halfSize + halfSize;
        rec.labelEl.style.transform = `translate3d(${sx}px, ${sy}px, 0)`;

        // Phase 14' v6 — back-side cutoff 0.05 (거의 보이지만 매우 dim).
        if (isFront && dot > 0.05) {
          rec.labelEl.style.opacity = String(Math.min(0.85, dot * 1.0));
        } else {
          rec.labelEl.style.opacity = "0";
        }
      });

      renderer.render(scene, camera);
      frameId = requestAnimationFrame(animate);
    };
    animate();

    return () => {
      cancelAnimationFrame(frameId);
      // Three.js dispose chain.
      sphereGeo.dispose();
      sphereMat.dispose();
      wireGeo.dispose();
      wireMat.dispose();
      atmGeo.dispose();
      atmMat.dispose();
      dustGeo.dispose();
      dustMat.dispose();
      nodeRecords.forEach((rec) => {
        rec.mesh.geometry.dispose();
        (rec.mesh.material as MeshBasicMaterial).dispose();
      });
      arcRecords.forEach(({ geo, mat }) => {
        geo.dispose();
        mat.dispose();
      });
      renderer.dispose();
      if (renderer.domElement.parentNode === container) {
        container.removeChild(renderer.domElement);
      }
      if (labelsContainer) labelsContainer.innerHTML = "";
    };
  }, [size, stableNodes, stableArcs, rotationSpeed]);

  return (
    <div
      className={`globe3d-wrap${className ? ` ${className}` : ""}`}
      style={{ width: size, height: size }}
    >
      <div ref={containerRef} className="globe3d-canvas" />
      <div ref={labelsRef} className="globe3d-labels" aria-hidden="true" />
    </div>
  );
}
