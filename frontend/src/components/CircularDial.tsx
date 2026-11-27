import React, { useEffect, useRef, useState, useCallback } from 'react';
import { FlagIcon } from './FlagIcon';
import {
  bpsToNorm,
  cleanNodeName,
  detectCountry,
  formatRateParts,
  formatRateStr,
  formatUptime,
  type FullStateDto,
} from '../types';

interface CircularDialProps {
  data: FullStateDto;
  dialPage: 'power' | 'speed';
  launching: boolean;
  launchStartMs: number;
  stopping: boolean;
  stopStartMs: number;
  onFlipPage: () => void;
  onPowerAction: () => void;
  onSelectNode: (name: string) => void;
  onLaunchComplete: () => void;
  onStopComplete: () => void;
  onLaunchShowSpeed: () => void;
  onStopShowPower: () => void;
}

const LAUNCH_DUR = 1550;
const STOP_DUR = 920;

// ---- Shared precision-instrument rendering helpers (light source: top-left) ----
function lipGrad(
  ctx: CanvasRenderingContext2D,
  cx: number,
  cy: number,
  r: number,
  aTL: number,
  aBR: number
) {
  const g = ctx.createLinearGradient(cx - r * 0.72, cy - r * 0.72, cx + r * 0.72, cy + r * 0.72);
  g.addColorStop(0, `rgba(255, 255, 255, ${aTL})`);
  g.addColorStop(0.5, `rgba(255, 255, 255, ${((aTL + aBR) * 0.25).toFixed(3)})`);
  g.addColorStop(1, `rgba(255, 255, 255, ${aBR})`);
  return g;
}

// Milled recessed channel: dark wall, matte floor, light-catching lips
function drawGroove(
  ctx: CanvasRenderingContext2D,
  cx: number,
  cy: number,
  r: number,
  a0: number,
  a1: number,
  width: number
) {
  ctx.save();
  ctx.lineCap = 'round';
  ctx.beginPath();
  ctx.arc(cx, cy, r, a0, a1);
  ctx.strokeStyle = '#070807';
  ctx.lineWidth = width;
  ctx.stroke();
  ctx.beginPath();
  ctx.arc(cx, cy, r + 0.6, a0, a1);
  ctx.strokeStyle = '#181a17';
  ctx.lineWidth = width - 2.6;
  ctx.stroke();
  // Floor sheen
  ctx.beginPath();
  ctx.arc(cx, cy, r, a0, a1);
  ctx.strokeStyle = lipGrad(ctx, cx, cy, r, 0.0, 0.035);
  ctx.lineWidth = width * 0.35;
  ctx.stroke();
  ctx.lineCap = 'butt';
  // Outer lip (lit at bottom-right) & inner lip (lit at top-left)
  ctx.beginPath();
  ctx.arc(cx, cy, r + width / 2 + 0.6, a0, a1);
  ctx.strokeStyle = lipGrad(ctx, cx, cy, r, 0.015, 0.11);
  ctx.lineWidth = 1;
  ctx.stroke();
  ctx.beginPath();
  ctx.arc(cx, cy, r - width / 2 - 0.6, a0, a1);
  ctx.strokeStyle = lipGrad(ctx, cx, cy, r, 0.1, 0.015);
  ctx.lineWidth = 1;
  ctx.stroke();
  ctx.restore();
}

// Applied index with cast shadow and a polished facet highlight
function drawTick(
  ctx: CanvasRenderingContext2D,
  cx: number,
  cy: number,
  ang: number,
  r1: number,
  r2: number,
  width: number,
  color: string,
  sheen = 0
) {
  const c = Math.cos(ang),
    s = Math.sin(ang);
  ctx.save();
  ctx.lineCap = 'butt';
  ctx.beginPath();
  ctx.moveTo(cx + c * r1 + 0.7, cy + s * r1 + 1.3);
  ctx.lineTo(cx + c * r2 + 0.7, cy + s * r2 + 1.3);
  ctx.strokeStyle = 'rgba(0, 0, 0, 0.7)';
  ctx.lineWidth = width + 0.8;
  ctx.stroke();
  ctx.beginPath();
  ctx.moveTo(cx + c * r1, cy + s * r1);
  ctx.lineTo(cx + c * r2, cy + s * r2);
  ctx.strokeStyle = color;
  ctx.lineWidth = width;
  ctx.stroke();
  if (sheen > 0) {
    let nx = -s,
      ny = c;
    if (nx + ny > 0) {
      nx = -nx;
      ny = -ny;
    } // flip toward top-left light
    const o = width * 0.26;
    ctx.beginPath();
    ctx.moveTo(cx + c * r1 + nx * o, cy + s * r1 + ny * o);
    ctx.lineTo(cx + c * r2 + nx * o, cy + s * r2 + ny * o);
    ctx.strokeStyle = `rgba(255, 255, 255, ${sheen})`;
    ctx.lineWidth = Math.max(0.8, width * 0.38);
    ctx.stroke();
  }
  ctx.restore();
}

// Illuminated light-pipe arc with specular core
function drawLitArc(
  ctx: CanvasRenderingContext2D,
  cx: number,
  cy: number,
  r: number,
  a0: number,
  a1: number,
  width: number,
  color: string | CanvasGradient,
  glow: string | null,
  core = 'rgba(255, 255, 255, 0.34)'
) {
  if (a1 - a0 < 0.002) return;
  ctx.save();
  ctx.lineCap = 'round';
  ctx.beginPath();
  ctx.arc(cx, cy, r, a0, a1);
  ctx.strokeStyle = color;
  ctx.lineWidth = width;
  if (glow) {
    ctx.shadowColor = glow;
    ctx.shadowBlur = 14;
  }
  ctx.stroke();
  ctx.shadowBlur = 0;
  ctx.shadowColor = 'transparent';
  ctx.beginPath();
  ctx.arc(cx, cy, r + width * 0.08, a0, a1);
  ctx.strokeStyle = 'rgba(0, 0, 0, 0.18)';
  ctx.lineWidth = width * 0.5;
  ctx.stroke();
  ctx.beginPath();
  ctx.arc(cx, cy, r - width * 0.2, a0, a1);
  ctx.strokeStyle = core;
  ctx.lineWidth = Math.max(1, width * 0.2);
  ctx.stroke();
  ctx.restore();
}

// Faceted (two-tone) tapered hand with drop shadow
function drawPointer(
  ctx: CanvasRenderingContext2D,
  cx: number,
  cy: number,
  ang: number,
  r0: number,
  r1: number,
  color: string,
  glow: string | null
) {
  const c = Math.cos(ang),
    s = Math.sin(ang);
  const nx = -s,
    ny = c;
  const w0 = 3.8,
    w1 = 1.3;
  const P = (r: number, w: number): [number, number] => [cx + c * r + nx * w, cy + s * r + ny * w];
  const tip = P(r1 + 2.2, 0);
  ctx.save();
  ctx.beginPath();
  ctx.moveTo(...P(r0, w0));
  ctx.lineTo(...P(r1, w1));
  ctx.lineTo(...tip);
  ctx.lineTo(...P(r1, -w1));
  ctx.lineTo(...P(r0, -w0));
  ctx.closePath();
  ctx.shadowColor = 'rgba(0, 0, 0, 0.8)';
  ctx.shadowBlur = 6;
  ctx.shadowOffsetX = 1.5;
  ctx.shadowOffsetY = 3;
  ctx.fillStyle = color;
  ctx.fill();
  if (glow) {
    ctx.shadowColor = glow;
    ctx.shadowBlur = 12;
    ctx.shadowOffsetX = 0;
    ctx.shadowOffsetY = 0;
    ctx.fill();
  }
  ctx.shadowColor = 'transparent';
  ctx.shadowBlur = 0;
  ctx.shadowOffsetX = 0;
  ctx.shadowOffsetY = 0;
  const sg = nx + ny > 0 ? -1 : 1; // side facing light
  ctx.beginPath();
  ctx.moveTo(...P(r0, 0));
  ctx.lineTo(...P(r0, w0 * sg));
  ctx.lineTo(...P(r1, w1 * sg));
  ctx.lineTo(...tip);
  ctx.closePath();
  ctx.fillStyle = 'rgba(255, 255, 255, 0.30)';
  ctx.fill();
  ctx.beginPath();
  ctx.moveTo(...P(r0, 0));
  ctx.lineTo(...P(r0, -w0 * sg));
  ctx.lineTo(...P(r1, -w1 * sg));
  ctx.lineTo(...tip);
  ctx.closePath();
  ctx.fillStyle = 'rgba(0, 0, 0, 0.22)';
  ctx.fill();
  ctx.restore();
}

export const CircularDial: React.FC<CircularDialProps> = ({
  data,
  dialPage,
  launching,
  launchStartMs,
  stopping,
  stopStartMs,
  onFlipPage,
  onPowerAction,
  onSelectNode,
  onLaunchComplete,
  onStopComplete,
  onLaunchShowSpeed,
  onStopShowPower,
}) => {
  const [pickerOpen, setPickerOpen] = useState(false);
  const bayHoveredRef = useRef(false);

  const warpCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const powerCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const speedCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const pickerChromeRef = useRef<HTMLCanvasElement | null>(null);
  const pickerLayerRef = useRef<HTMLDivElement | null>(null);
  const pickerListRef = useRef<HTMLDivElement | null>(null);
  const powerPageRef = useRef<HTMLDivElement | null>(null);
  const speedPageRef = useRef<HTMLDivElement | null>(null);

  const smoothDownRef = useRef(0);
  const smoothUpRef = useRef(0);
  const streaksRef = useRef(
    Array.from({ length: 84 }, (_, i) => {
      const h1 = Math.sin(i * 127.1 + 311.7) * 43758.5453;
      const h2 = Math.sin(i * 269.5 + 183.3) * 43758.5453;
      const h3 = Math.sin(i * 419.2 + 71.9) * 43758.5453;
      const r1 = h1 - Math.floor(h1);
      const r2 = h2 - Math.floor(h2);
      const r3 = h3 - Math.floor(h3);
      return {
        angle: (i / 84) * Math.PI * 2 + (r1 - 0.5) * 0.14,
        z0: r2,
        z: 0.12 + r2 * 0.88,
        speedMult: 0.78 + r3 * 0.56,
        thickness: 1.1 + r1 * 1.75,
        color: i % 4 === 0 ? '#ff6a3d' : i % 3 === 0 ? '#5bd68a' : '#e8e4d8',
      };
    })
  );

  // Latest props ref for 60fps canvas loop
  const propsRef = useRef({
    data,
    dialPage,
    launching,
    launchStartMs,
    stopping,
    stopStartMs,
    onLaunchComplete,
    onStopComplete,
    onLaunchShowSpeed,
    onStopShowPower,
  });
  propsRef.current = {
    data,
    dialPage,
    launching,
    launchStartMs,
    stopping,
    stopStartMs,
    onLaunchComplete,
    onStopComplete,
    onLaunchShowSpeed,
    onStopShowPower,
  };

  // Chord-adaptive circular node picker curve + curved right arc scrollbar
  const updatePickerCurve = useCallback(() => {
    const layer = pickerLayerRef.current;
    const list = pickerListRef.current;
    const cvs = pickerChromeRef.current;
    if (!layer || !list) return;

    const wellH = layer.clientHeight || 313;
    const wellR = wellH * 0.5;
    const rSafe = Math.max(80, wellR - 22);
    const layerRect = layer.getBoundingClientRect();
    const dialCenterY = layerRect.top + wellR;

    const rows = list.querySelectorAll<HTMLElement>('.picker-node-row');
    rows.forEach((row) => {
      const rr = row.getBoundingClientRect();
      const dy = rr.top + rr.height * 0.5 - dialCenterY;
      const absDy = Math.abs(dy);
      const chordW = 2 * Math.sqrt(Math.max(0, rSafe * rSafe - dy * dy));
      const w = Math.max(148, Math.min(wellH * 0.84, chordW));
      row.style.width = `${w.toFixed(1)}px`;

      const fadeStart = wellR * 0.48;
      const fadeEnd = wellR * 0.76;
      const op =
        absDy <= fadeStart
          ? 1
          : Math.max(0.24, 1 - ((absDy - fadeStart) / Math.max(1, fadeEnd - fadeStart)) * 0.76);
      row.style.opacity = op.toFixed(3);
    });

    if (cvs) {
      const ctx = cvs.getContext('2d');
      if (!ctx) return;
      const w = cvs.width,
        h = cvs.height;
      const cx = w / 2,
        cy = h / 2;
      ctx.clearRect(0, 0, w, h);

      ctx.beginPath();
      ctx.arc(cx, cy, 280, 0, Math.PI * 2);
      ctx.strokeStyle = 'rgba(255, 255, 255, 0.06)';
      ctx.lineWidth = 2;
      ctx.stroke();

      ctx.lineCap = 'round';
      ctx.beginPath();
      ctx.arc(cx, cy, 266, (164 * Math.PI) / 180, (196 * Math.PI) / 180);
      ctx.strokeStyle = 'rgba(255, 255, 255, 0.06)';
      ctx.lineWidth = 4;
      ctx.stroke();

      const maxScroll = Math.max(0, list.scrollHeight - list.clientHeight);
      if (maxScroll > 2) {
        const sbStart = (-34 * Math.PI) / 180;
        const sbEnd = (34 * Math.PI) / 180;
        const sbSweep = sbEnd - sbStart;
        const rSb = 266;
        ctx.beginPath();
        ctx.arc(cx, cy, rSb, sbStart, sbEnd);
        ctx.strokeStyle = '#262824';
        ctx.lineWidth = 6;
        ctx.stroke();

        const tf = Math.max(0.18, Math.min(1, list.clientHeight / list.scrollHeight));
        const sf = Math.max(0, Math.min(1, list.scrollTop / maxScroll));
        const thumbSweep = sbSweep * tf;
        const tStart = sbStart + sf * (sbSweep - thumbSweep);
        ctx.beginPath();
        ctx.arc(cx, cy, rSb, tStart, tStart + thumbSweep);
        ctx.strokeStyle = 'rgba(232, 230, 222, 0.78)';
        ctx.lineWidth = 6;
        ctx.stroke();
      }
    }
  }, []);

  useEffect(() => {
    if (pickerOpen) {
      requestAnimationFrame(updatePickerCurve);
    }
  }, [pickerOpen, data.nodes, updatePickerCurve]);

  // Unified 60fps Canvas Loop (Power Dial + Speedometer w/ Sculpted LCD Bay + Warp/Collapse)
  useEffect(() => {
    let rafId = 0;

    const frame = (now: number) => {
      const cur = propsRef.current;
      const running = cur.data.running;
      const targetDown = running ? cur.data.telemetry.down_bps : 0;
      const targetUp = running ? cur.data.telemetry.up_bps : 0;
      smoothDownRef.current += (targetDown - smoothDownRef.current) * 0.12;
      smoothUpRef.current += (targetUp - smoothUpRef.current) * 0.12;

      // 1. Warp / Shutdown Collapse Canvas
      const wCvs = warpCanvasRef.current;
      if (wCvs) {
        const wCtx = wCvs.getContext('2d')!;
        const w = wCvs.width,
          h = wCvs.height;
        const cx = w / 2,
          cy = h / 2;
        wCtx.clearRect(0, 0, w, h);

        if (cur.stopping) {
          const p = Math.min(1, (now - cur.stopStartMs) / STOP_DUR);
          const easeOut = 1 - Math.pow(1 - p, 3);
          const masterAlpha = p < 0.14 ? p / 0.14 : Math.max(0, 1 - Math.pow((p - 0.14) / 0.86, 1.35));
          if (p >= 0.22 && cur.dialPage !== 'power') cur.onStopShowPower();

          if (masterAlpha > 0.005) {
            wCtx.save();
            wCtx.lineCap = 'round';
            const decel = Math.pow(1 - p, 1.6);
            const dzBase = 0.012 + decel * 0.052;
            const fov = 72;
            streaksRef.current.forEach((s, idx) => {
              if (idx % 2 !== 0) return;
              const prevZ = s.z;
              s.z += dzBase * s.speedMult;
              if (s.z >= 1.05) {
                s.z = 0.14 + Math.random() * 0.18;
                return;
              }
              const rOuter = Math.min(290, fov / Math.max(0.08, prevZ));
              const rInner = Math.min(285, fov / Math.max(0.1, s.z + dzBase * (1.8 + decel * 2.5)));
              if (rOuter < 105 || rOuter <= rInner + 2) return;
              const cos = Math.cos(s.angle),
                sin = Math.sin(s.angle);
              const x0 = cx + cos * rOuter,
                y0 = cy + sin * rOuter;
              const x1 = cx + cos * rInner,
                y1 = cy + sin * rInner;
              const ringFade = Math.min(1, Math.max(0, (rOuter - 105) / 85));
              const grad = wCtx.createLinearGradient(x0, y0, x1, y1);
              grad.addColorStop(0, 'rgba(255,255,255,0)');
              grad.addColorStop(1, idx % 4 === 0 ? '#ff6a3d' : '#5bd68a');
              wCtx.globalAlpha = masterAlpha * ringFade * 0.85;
              wCtx.beginPath();
              wCtx.moveTo(x0, y0);
              wCtx.lineTo(x1, y1);
              wCtx.strokeStyle = grad;
              wCtx.lineWidth = s.thickness * (0.55 + decel * 0.7);
              wCtx.stroke();
            });
            const rRing = 260 - easeOut * 164;
            const ringAlpha = Math.sin(p * Math.PI) * 0.55;
            if (ringAlpha > 0.01) {
              wCtx.globalAlpha = 1;
              wCtx.beginPath();
              wCtx.arc(cx, cy, rRing, 0, Math.PI * 2);
              wCtx.strokeStyle = `rgba(255, 106, 61, ${ringAlpha.toFixed(3)})`;
              wCtx.lineWidth = 3.5;
              wCtx.shadowColor = 'rgba(255, 106, 61, 0.75)';
              wCtx.shadowBlur = 14;
              wCtx.stroke();
            }
            wCtx.restore();
          }
          if (p >= 1) cur.onStopComplete();
        } else if (cur.launching) {
          const p = Math.max(0, Math.min(1, (now - cur.launchStartMs) / LAUNCH_DUR));
          const easeOutCubic = (t: number) => 1 - Math.pow(1 - t, 3);
          const easeInCubic = (t: number) => t * t * t;

          const pPower = powerPageRef.current;
          const pSpeed = speedPageRef.current;

          // Phase 1 & 2: Tactile button detent (0..0.16) -> perspective dive (0.16..0.55)
          if (p < 0.56 && cur.dialPage === 'power' && pPower) {
            let sc = 1,
              op = 1;
            if (p < 0.09) {
              sc = 1 - 0.055 * easeOutCubic(p / 0.09);
            } else if (p < 0.16) {
              sc = 0.945 + 0.012 * ((p - 0.09) / 0.07);
            } else {
              const dive = Math.min(1, (p - 0.16) / 0.39);
              sc = 0.957 - 0.36 * easeInCubic(dive);
              op = Math.max(0, 1 - Math.pow(dive, 1.75));
            }
            pPower.style.transition = 'none';
            pPower.style.transform = `scale(${sc.toFixed(4)})`;
            pPower.style.opacity = op.toFixed(4);
          }

          // Phase 3: Synchronized swap to Speedometer behind breakthrough ring (p >= 0.56)
          if (p >= 0.56) {
            if (pPower) {
              pPower.style.transition = 'none';
              pPower.style.opacity = '0';
              pPower.style.transform = 'scale(0.6)';
            }
            if (cur.dialPage !== 'speed') {
              cur.onLaunchShowSpeed();
            }
            if (pSpeed) {
              const em = Math.min(1, (p - 0.56) / 0.14);
              const emEase = easeOutCubic(em);
              pSpeed.style.transition = 'none';
              pSpeed.style.transform = `scale(${(0.92 + 0.08 * emEase).toFixed(4)})`;
              pSpeed.style.opacity = (0.18 + 0.82 * emEase).toFixed(4);
            }
          }

          const tunnelAlpha =
            p < 0.1
              ? easeOutCubic(p / 0.1) * 0.55
              : p < 0.58
              ? 0.55 + 0.45 * ((p - 0.1) / 0.48)
              : Math.max(0, 1 - Math.pow((p - 0.58) / 0.34, 1.35));

          wCtx.save();

          // 1. Phase 1 Ignition Detent Impulse Ring (p: 0.03 -> 0.28)
          if (p >= 0.03 && p <= 0.28) {
            const ip = (p - 0.03) / 0.25;
            const rImp = 194 + easeOutCubic(ip) * 90;
            const aImp = Math.sin(ip * Math.PI) * 0.72;
            wCtx.beginPath();
            wCtx.arc(cx, cy, rImp, 0, Math.PI * 2);
            wCtx.strokeStyle = `rgba(255, 138, 92, ${aImp.toFixed(3)})`;
            wCtx.lineWidth = 3.2 * (1 - ip * 0.55);
            wCtx.shadowColor = 'rgba(255, 106, 61, 0.85)';
            wCtx.shadowBlur = 14;
            wCtx.stroke();
            wCtx.shadowBlur = 0;
          }

          // 2. Optical Core Bloom + 3D Perspective Tunnel Ribs + Radial Light Rails
          if (tunnelAlpha > 0.005) {
            const greenMix = Math.max(0, Math.min(1, (p - 0.34) / 0.24));
            const peakBloom =
              p < 0.58
                ? 0.14 + 0.24 * Math.pow(p / 0.58, 2)
                : 0.38 * Math.max(0, 1 - (p - 0.58) / 0.22);
            const rBloom = 155 + 55 * greenMix;
            const glow = wCtx.createRadialGradient(cx, cy, 4, cx, cy, rBloom);
            if (greenMix > 0.02) {
              glow.addColorStop(0, `rgba(190, 255, 218, ${(peakBloom * 0.65 * greenMix).toFixed(3)})`);
              glow.addColorStop(
                0.32,
                `rgba(91, 214, 138, ${(peakBloom * 0.72 * greenMix + peakBloom * 0.25 * (1 - greenMix)).toFixed(3)})`
              );
              glow.addColorStop(0.68, `rgba(232, 76, 30, ${(peakBloom * 0.28 * (1 - greenMix * 0.6)).toFixed(3)})`);
              glow.addColorStop(1, 'rgba(0, 0, 0, 0)');
            } else {
              glow.addColorStop(0, `rgba(255, 148, 98, ${(peakBloom * 0.65).toFixed(3)})`);
              glow.addColorStop(0.52, `rgba(232, 76, 30, ${(peakBloom * 0.32).toFixed(3)})`);
              glow.addColorStop(1, 'rgba(0, 0, 0, 0)');
            }
            wCtx.fillStyle = glow;
            wCtx.fillRect(0, 0, w, h);

            const travel = p * 1.15 + Math.pow(p, 2.35) * 4.4;
            const fovRing = 42;
            const ringCount = 7;
            const btnMaskR =
              p < 0.22
                ? 182
                : p < 0.44
                ? 182 - easeOutCubic((p - 0.22) / 0.22) * 150
                : p > 0.56
                ? 185 + easeOutCubic(Math.min(1, (p - 0.56) / 0.18)) * 95
                : 32;

            for (let i = 0; i < ringCount; i++) {
              const z = 1.0 - ((travel * 0.72 + i / ringCount) % 1.0);
              if (z < 0.13 || z > 0.96) continue;
              const rr = fovRing / z;
              if (rr < Math.max(28, btnMaskR) || rr > 294) continue;
              const depthFade = Math.min(1, (rr - 28) / 75) * Math.min(1, (294 - rr) / 48);
              const aRing = tunnelAlpha * depthFade * 0.36;
              if (aRing < 0.008) continue;

              const isNear = rr > 155;
              wCtx.beginPath();
              wCtx.arc(cx, cy, rr, 0, Math.PI * 2);
              wCtx.strokeStyle = isNear
                ? `rgba(91, 214, 138, ${(aRing * (0.45 + 0.55 * greenMix)).toFixed(3)})`
                : `rgba(255, 130, 86, ${(aRing * 0.88).toFixed(3)})`;
              wCtx.lineWidth = 0.85 + (rr / 290) * 1.65;
              wCtx.stroke();

              const tickLen = 4 + (rr / 290) * 6;
              wCtx.beginPath();
              for (let q = 0; q < 4; q++) {
                const qa = q * (Math.PI / 2);
                const qc = Math.cos(qa),
                  qs = Math.sin(qa);
                wCtx.moveTo(cx + qc * (rr - tickLen * 0.5), cy + qs * (rr - tickLen * 0.5));
                wCtx.lineTo(cx + qc * (rr + tickLen * 0.5), cy + qs * (rr + tickLen * 0.5));
              }
              wCtx.strokeStyle = `rgba(242, 241, 236, ${(aRing * 0.65).toFixed(3)})`;
              wCtx.lineWidth = 1.1;
              wCtx.stroke();
            }

            wCtx.lineCap = 'round';
            const fov = 56;
            const vel = 0.045 + Math.pow(p, 2.15) * 0.22;
            const innerClear = btnMaskR;

            streaksRef.current.forEach((s) => {
              const z = 1.0 - ((travel * s.speedMult + s.z0) % 1.0);
              if (z <= 0.14 || z >= 0.98) return;
              const tailZ = Math.min(1.02, z + vel * s.speedMult * (1.05 + 0.55 * (1 - z)));
              const rHead = Math.min(302, fov / z);
              const rTail = Math.max(innerClear, Math.min(296, fov / tailZ));
              if (rHead < innerClear + 4 || rHead <= rTail + 3) return;
              const cos = Math.cos(s.angle),
                sin = Math.sin(s.angle);
              const x0 = cx + cos * rTail,
                y0 = cy + sin * rTail;
              const x1 = cx + cos * rHead,
                y1 = cy + sin * rHead;
              const depthFade = Math.min(1, Math.max(0, (rHead - innerClear) / 72));
              const grad = wCtx.createLinearGradient(x0, y0, x1, y1);
              grad.addColorStop(0, 'rgba(255,255,255,0)');
              grad.addColorStop(0.75, s.color);
              grad.addColorStop(1, 'rgba(255, 252, 245, 0.95)');
              wCtx.globalAlpha = tunnelAlpha * depthFade * 0.84;
              wCtx.beginPath();
              wCtx.moveTo(x0, y0);
              wCtx.lineTo(x1, y1);
              wCtx.strokeStyle = grad;
              wCtx.lineWidth = s.thickness * (0.45 + depthFade * 0.9);
              wCtx.stroke();
            });
            wCtx.globalAlpha = 1;
          }

          // 3. Phase 3 Breakthrough Shockwave Ring (p: 0.53 -> 0.75)
          if (p >= 0.53 && p <= 0.75) {
            const bp = (p - 0.53) / 0.22;
            const rWave = 36 + easeOutCubic(bp) * 252;
            const aWave = Math.sin(bp * Math.PI) * 0.85;
            wCtx.beginPath();
            wCtx.arc(cx, cy, rWave, 0, Math.PI * 2);
            wCtx.strokeStyle = `rgba(91, 214, 138, ${(aWave * 0.45).toFixed(3)})`;
            wCtx.lineWidth = 11 * (1 - bp * 0.55);
            wCtx.stroke();
            wCtx.beginPath();
            wCtx.arc(cx, cy, rWave, 0, Math.PI * 2);
            wCtx.strokeStyle = `rgba(225, 255, 238, ${aWave.toFixed(3)})`;
            wCtx.lineWidth = 2.6 * (1 - bp * 0.45);
            wCtx.shadowColor = 'rgba(91, 214, 138, 0.9)';
            wCtx.shadowBlur = 16;
            wCtx.stroke();
            wCtx.shadowBlur = 0;
          }

          wCtx.restore();

          if (p >= 1) {
            if (pPower) {
              pPower.style.transition = '';
              pPower.style.transform = '';
              pPower.style.opacity = '';
            }
            if (pSpeed) {
              pSpeed.style.transition = '';
              pSpeed.style.transform = '';
              pSpeed.style.opacity = '';
            }
            cur.onLaunchComplete();
          }
        }
      }

      // 2. Power Dial Canvas (Page 0)
      const pCvs = powerCanvasRef.current;
      if (pCvs && (cur.dialPage === 'power' || cur.launching || cur.stopping)) {
        const pCtx = pCvs.getContext('2d')!;
        const w = pCvs.width,
          h = pCvs.height;
        const cx = w / 2,
          cy = h / 2;
        pCtx.clearRect(0, 0, w, h);

        const t = now * 0.001;
        const launchP = cur.launching ? Math.min(1, Math.max(0, (now - cur.launchStartMs) / (LAUNCH_DUR * 0.54))) : 0;
        const stopP = cur.stopping ? Math.min(1, (now - cur.stopStartMs) / STOP_DUR) : 0;

        const bgGlow = pCtx.createRadialGradient(cx, cy, 120, cx, cy, 248);
        if (running && !cur.stopping) {
          bgGlow.addColorStop(0, 'rgba(91, 214, 138, 0.11)');
          bgGlow.addColorStop(0.65, 'rgba(91, 214, 138, 0.02)');
          bgGlow.addColorStop(1, 'rgba(0,0,0,0)');
        } else {
          const pulse = 0.08 + 0.045 * Math.sin(t * 2.2) + (cur.launching ? 0.14 * launchP : 0);
          bgGlow.addColorStop(0, `rgba(232, 76, 30, ${pulse.toFixed(3)})`);
          bgGlow.addColorStop(0.62, 'rgba(232, 76, 30, 0.015)');
          bgGlow.addColorStop(1, 'rgba(0,0,0,0)');
        }
        pCtx.fillStyle = bgGlow;
        pCtx.fillRect(0, 0, w, h);

        // Outer chapter-ring hairline (matches speedometer r=280)
        pCtx.beginPath();
        pCtx.arc(cx, cy, 280, 0, Math.PI * 2);
        pCtx.strokeStyle = lipGrad(pCtx, cx, cy, 280, 0.03, 0.09);
        pCtx.lineWidth = 1.5;
        pCtx.stroke();

        const startAng = (152 * Math.PI) / 180;
        const endAng = (388 * Math.PI) / 180;
        const sweep = endAng - startAng;

        // Printed minute-track boundary
        pCtx.beginPath();
        pCtx.arc(cx, cy, 275, startAng, endAng);
        pCtx.strokeStyle = 'rgba(255, 255, 255, 0.07)';
        pCtx.lineWidth = 1;
        pCtx.stroke();

        // Applied chapter indices with cast shadows & polished facets
        const majorCol = running && !cur.stopping ? 'rgba(110, 224, 152, 0.9)' : 'rgba(255, 118, 76, 0.92)';
        for (let i = 0; i <= 40; i++) {
          const ang = startAng + (i / 40) * sweep;
          const isMajor = i % 10 === 0;
          const isMid = i % 5 === 0 && !isMajor;
          if (isMajor) drawTick(pCtx, cx, cy, ang, 273, 257, 4.2, majorCol, 0.42);
          else if (isMid) drawTick(pCtx, cx, cy, ang, 273, 262, 1.9, '#6d6f69', 0.18);
          else drawTick(pCtx, cx, cy, ang, 273, 267, 1.2, '#41433f');
        }

        // Milled recessed outer track (r=248)
        drawGroove(pCtx, cx, cy, 248, startAng, endAng, 8.5);

        if (cur.stopping) {
          const remain = Math.max(0, 1 - Math.pow(stopP, 0.75));
          if (remain > 0.01) {
            const green = stopP < 0.45;
            drawLitArc(
              pCtx,
              cx,
              cy,
              248,
              startAng,
              startAng + sweep * remain,
              6,
              green ? '#5bd68a' : '#ff6a3d',
              green ? 'rgba(91, 214, 138, 0.7)' : 'rgba(255, 106, 61, 0.75)'
            );
          }
        } else if (cur.launching && launchP > 0.01) {
          const easedCharge = launchP < 0.5 ? 2 * launchP * launchP : 1 - Math.pow(-2 * launchP + 2, 2) / 2;
          const g = pCtx.createLinearGradient(cx - 248, cy, cx + 248, cy);
          g.addColorStop(0, '#c9380f');
          g.addColorStop(0.55, '#ff6a3d');
          g.addColorStop(1, '#ffd0b3');
          const headAng = startAng + sweep * easedCharge;
          drawLitArc(pCtx, cx, cy, 248, startAng, headAng, 6, g, 'rgba(255, 106, 61, 0.85)');
          pCtx.save();
          pCtx.beginPath();
          pCtx.arc(cx + Math.cos(headAng) * 248, cy + Math.sin(headAng) * 248, 3.8, 0, Math.PI * 2);
          pCtx.fillStyle = '#fff5ee';
          pCtx.shadowColor = 'rgba(255, 130, 80, 0.95)';
          pCtx.shadowBlur = 12;
          pCtx.fill();
          pCtx.restore();
        } else if (running) {
          drawLitArc(pCtx, cx, cy, 248, startAng, endAng, 3.2, 'rgba(91, 214, 138, 0.42)', null, 'rgba(200, 255, 220, 0.18)');
        }

        // Machined step-down seat around the 194px hero socket
        pCtx.beginPath();
        pCtx.arc(cx, cy, 198, 0, Math.PI * 2);
        pCtx.strokeStyle = 'rgba(0, 0, 0, 0.55)';
        pCtx.lineWidth = 3.5;
        pCtx.stroke();
        pCtx.beginPath();
        pCtx.arc(cx, cy, 200.5, 0, Math.PI * 2);
        pCtx.strokeStyle = running && !cur.stopping ? 'rgba(91, 214, 138, 0.2)' : lipGrad(pCtx, cx, cy, 200, 0.02, 0.12);
        pCtx.lineWidth = 1;
        pCtx.stroke();
      }

      // 3. Speedometer + Sculpted Smoked-Glass Lower LCD Bay Canvas (Page 1)
      const sCvs = speedCanvasRef.current;
      if (sCvs) {
        const sCtx = sCvs.getContext('2d')!;
        const w = sCvs.width,
          h = sCvs.height;
        const cx = w / 2,
          cy = h / 2;
        sCtx.clearRect(0, 0, w, h);

        const startAng = (152 * Math.PI) / 180;
        const endAng = (388 * Math.PI) / 180;
        const sweep = endAng - startAng;
        const rOuter = 248,
          rInner = 214;

        sCtx.beginPath();
        sCtx.arc(cx, cy, 280, 0, Math.PI * 2);
        sCtx.strokeStyle = lipGrad(sCtx, cx, cy, 280, 0.03, 0.09);
        sCtx.lineWidth = 1.5;
        sCtx.stroke();

        sCtx.beginPath();
        sCtx.arc(cx, cy, 274, startAng, endAng);
        sCtx.strokeStyle = 'rgba(255, 255, 255, 0.07)';
        sCtx.lineWidth = 1;
        sCtx.stroke();

        // Milled recessed channels for download (outer) & upload (inner)
        drawGroove(sCtx, cx, cy, rOuter, startAng, endAng, 19);
        drawGroove(sCtx, cx, cy, rInner, startAng, endAng, 10);

        // Applied polished-steel indices
        for (let i = 0; i <= 40; i++) {
          const ang = startAng + (i / 40) * sweep;
          const isMajor = i % 10 === 0;
          const isMid = i % 5 === 0 && !isMajor;
          if (isMajor) drawTick(sCtx, cx, cy, ang, 272, 259, 4, '#d4d2c9', 0.55);
          else if (isMid) drawTick(sCtx, cx, cy, ang, 272, 262, 1.9, '#73756f', 0.18);
          else drawTick(sCtx, cx, cy, ang, 272, 266, 1.2, '#474944');
        }

        const labels = ['0', '100K', '1M', '10M', '100M'];
        sCtx.font = '600 15px ui-monospace, SFMono-Regular, Menlo, monospace';
        sCtx.textAlign = 'center';
        sCtx.textBaseline = 'middle';
        labels.forEach((lbl, idx) => {
          const ang = startAng + (idx / 4) * sweep;
          const lx = cx + Math.cos(ang) * 185,
            ly = cy + Math.sin(ang) * 185;
          sCtx.fillStyle = 'rgba(0, 0, 0, 0.75)';
          sCtx.fillText(lbl, lx + 0.6, ly + 1.2);
          sCtx.fillStyle = '#93918a';
          sCtx.fillText(lbl, lx, ly);
        });

        const liveDn = running ? bpsToNorm(smoothDownRef.current) : 0;
        const liveUn = running ? bpsToNorm(smoothUpRef.current) : 0;

        // Phase 4 Sports-Car Instrument Self-Test Sweep (p: 0.56 -> 1.00)
        let dn = liveDn;
        let un = liveUn;
        let sweepPhase = -1;
        if (cur.launching) {
          const lp = Math.max(0, Math.min(1, (now - cur.launchStartMs) / LAUNCH_DUR));
          if (lp >= 0.56) {
            sweepPhase = (lp - 0.56) / 0.44;
            if (sweepPhase < 0.44) {
              const k = 1 - Math.pow(1 - sweepPhase / 0.44, 3);
              dn = 0.94 * k;
              un = 0.78 * k;
            } else {
              const t = (sweepPhase - 0.44) / 0.56;
              const k = t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
              dn = 0.94 + (liveDn - 0.94) * k;
              un = 0.78 + (liveUn - 0.78) * k;
            }
          }
        }

        if (dn > 0.005) {
          const grad = sCtx.createLinearGradient(cx - rOuter, cy, cx + rOuter, cy);
          grad.addColorStop(0, '#1f8a4c');
          grad.addColorStop(0.6, '#5bd68a');
          grad.addColorStop(1, '#8ef0b2');
          drawLitArc(
            sCtx,
            cx,
            cy,
            rOuter,
            startAng,
            startAng + dn * sweep,
            14,
            grad,
            'rgba(91, 214, 138, 0.38)',
            'rgba(220, 255, 232, 0.38)'
          );
        }

        if (un > 0.005) {
          const ugrad = sCtx.createLinearGradient(cx - rInner, cy, cx + rInner, cy);
          ugrad.addColorStop(0, '#c9380f');
          ugrad.addColorStop(0.6, '#ff6a3d');
          ugrad.addColorStop(1, '#ffa27d');
          drawLitArc(
            sCtx,
            cx,
            cy,
            rInner,
            startAng,
            startAng + un * sweep,
            6.5,
            ugrad,
            'rgba(255, 106, 61, 0.38)',
            'rgba(255, 225, 210, 0.4)'
          );
        }

        // Faceted tapered pointer riding over the download channel
        const tipAng = startAng + dn * sweep;
        drawPointer(
          sCtx,
          cx,
          cy,
          tipAng,
          rOuter - 18,
          rOuter + 20,
          running ? '#ef5425' : '#5e605b',
          running ? 'rgba(232, 76, 30, 0.45)' : null
        );

        // Sculpted Smoked-Glass Lower LCD Bay
        const rBay = 264;
        const yTop = 402;
        const xTL = 148,
          xTR = 452;
        const angR = (36.5 * Math.PI) / 180;
        const angL = (143.5 * Math.PI) / 180;
        const rx = cx + Math.cos(angR) * rBay,
          ry = cy + Math.sin(angR) * rBay;
        const lx = cx + Math.cos(angL) * rBay,
          ly = cy + Math.sin(angL) * rBay;
        const cr = 14;
        const drx = rx - xTR,
          dry = ry - yTop;
        const dLen = Math.hypot(drx, dry);
        const urx = drx / dLen,
          ury = dry / dLen;
        const ulx = -urx,
          uly = ury;
        const blendAng = (3.2 * Math.PI) / 180;
        const arcStartR = angR + blendAng;
        const arcEndL = angL - blendAng;

        sCtx.save();
        sCtx.beginPath();
        sCtx.moveTo(xTL + cr, yTop);
        sCtx.lineTo(xTR - cr, yTop);
        sCtx.quadraticCurveTo(xTR, yTop, xTR + urx * cr, yTop + ury * cr);
        sCtx.lineTo(rx - urx * cr, ry - ury * cr);
        sCtx.quadraticCurveTo(rx, ry, cx + Math.cos(arcStartR) * rBay, cy + Math.sin(arcStartR) * rBay);
        sCtx.arc(cx, cy, rBay, arcStartR, arcEndL, false);
        sCtx.quadraticCurveTo(lx, ly, lx - ulx * cr, ly - uly * cr);
        sCtx.lineTo(xTL + ulx * cr, yTop + uly * cr);
        sCtx.quadraticCurveTo(xTL, yTop, xTL + cr, yTop);
        sCtx.closePath();

        sCtx.save();
        sCtx.strokeStyle = 'rgba(0,0,0,0.78)';
        sCtx.lineWidth = 7;
        sCtx.stroke();
        sCtx.restore();

        const bayGrad = sCtx.createLinearGradient(cx, yTop, cx, cy + rBay);
        bayGrad.addColorStop(0, '#232824');
        bayGrad.addColorStop(0.38, '#1b1f1c');
        bayGrad.addColorStop(1, '#141715');
        sCtx.fillStyle = bayGrad;
        sCtx.fill();

        sCtx.save();
        sCtx.clip();

        const glassSheen = sCtx.createLinearGradient(xTL - 20, yTop, cx + 60, yTop + 120);
        glassSheen.addColorStop(0, 'rgba(255,255,255,0.075)');
        glassSheen.addColorStop(0.42, 'rgba(255,255,255,0.02)');
        glassSheen.addColorStop(0.46, 'rgba(255,255,255,0)');
        sCtx.fillStyle = glassSheen;
        sCtx.fillRect(0, yTop, w, rBay);

        sCtx.fillStyle = 'rgba(8,10,9,0.38)';
        sCtx.fillRect(0, yTop + 48, w, 130);

        sCtx.beginPath();
        sCtx.moveTo(xTL - 14, yTop + 48);
        sCtx.lineTo(xTR + 14, yTop + 48);
        sCtx.strokeStyle = 'rgba(255,255,255,0.11)';
        sCtx.lineWidth = 1.6;
        sCtx.stroke();

        const barStartAng = (131 * Math.PI) / 180;
        const barEndAng = (49 * Math.PI) / 180;
        sCtx.beginPath();
        sCtx.arc(cx, cy, 253, barEndAng, barStartAng, false);
        sCtx.strokeStyle = 'rgba(255,255,255,0.09)';
        sCtx.lineWidth = 1.6;
        sCtx.stroke();

        const rawHist = cur.data.telemetry.speed_history.slice(-22);
        const hist: [number, number][] = [
          ...Array.from({ length: Math.max(0, 22 - rawHist.length) }, () => [0, 0] as [number, number]),
          ...rawHist,
        ];
        const maxVal = Math.max(1024, ...hist.map(([u, d]) => u + d));
        const rBase = 248,
          maxBarLen = 38,
          minBarLen = 5;

        hist.forEach(([u, d], idx) => {
          const frac = idx / (hist.length - 1);
          const ang = barStartAng + frac * (barEndAng - barStartAng);
          const tot = running ? u + d : 0;
          const norm = Math.min(1, tot / (maxVal * 1.1));
          let bootWave = 0;
          if (sweepPhase >= 0.04 && sweepPhase <= 0.88) {
            const waveCenter = (sweepPhase - 0.04) / 0.74;
            const dist = Math.abs(frac - waveCenter);
            if (dist < 0.24) bootWave = Math.cos((dist / 0.24) * (Math.PI / 2));
          }
          const effectiveNorm = Math.max(norm, bootWave * 0.88);
          const isBootLit = bootWave > 0.08 && (!running || tot < 1 || bootWave * 0.88 > norm);
          const activeBar = (running && tot >= 1) || isBootLit;
          const len = activeBar ? minBarLen + effectiveNorm * (maxBarLen - minBarLen) : minBarLen;
          const rIn = rBase - len;
          const cos = Math.cos(ang),
            sin = Math.sin(ang);
          sCtx.beginPath();
          sCtx.moveTo(cx + cos * rBase, cy + sin * rBase);
          sCtx.lineTo(cx + cos * rIn, cy + sin * rIn);
          sCtx.strokeStyle = isBootLit
            ? bootWave > 0.65
              ? '#8ef0b2'
              : '#5bd68a'
            : !activeBar
            ? '#363b36'
            : u > d
            ? '#ff6a3d'
            : '#5bd68a';
          sCtx.lineWidth = activeBar ? 7 : 6.5;
          sCtx.stroke();
        });
        sCtx.restore();

        const hovered = bayHoveredRef.current;
        sCtx.strokeStyle = hovered ? '#e84c1e' : '#4a4f49';
        sCtx.lineWidth = hovered ? 2.8 : 2.2;
        if (hovered) {
          sCtx.shadowColor = 'rgba(232,76,30,0.42)';
          sCtx.shadowBlur = 12;
        }
        sCtx.stroke();

        sCtx.shadowBlur = 0;
        sCtx.beginPath();
        sCtx.moveTo(xTL + cr, yTop + 1.5);
        sCtx.lineTo(xTR - cr, yTop + 1.5);
        sCtx.strokeStyle = hovered ? 'rgba(255,160,122,0.45)' : 'rgba(255,255,255,0.16)';
        sCtx.lineWidth = 1.4;
        sCtx.stroke();
        sCtx.restore();
      }

      rafId = requestAnimationFrame(frame);
    };

    rafId = requestAnimationFrame(frame);
    return () => cancelAnimationFrame(rafId);
  }, []);

  const activeNodeObj = data.nodes.find((n) => n.is_active) ?? data.nodes[0];
  const activeName = activeNodeObj?.name ?? '未加载节点';
  const activeCountry = detectCountry(activeName);
  const activeMs =
    activeNodeObj?.latency_ms == null
      ? '--ms'
      : activeNodeObj.latency_ms === 0
      ? '超时'
      : `${activeNodeObj.latency_ms}ms`;
  const mainRate = formatRateParts(data.running ? data.telemetry.down_bps : 0);

  return (
    <div
      className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 aspect-square flex items-center justify-center z-10"
      style={{ height: '62.5%' }}
    >
      <div className="w-full h-full rounded-full dial-bezel p-[13px] flex items-center justify-center">
        <div className="w-full h-full rounded-full dial-chamfer flex items-center justify-center">
          <div className="dial-well relative w-full h-full rounded-full overflow-hidden flex items-center justify-center">
            <canvas
              ref={warpCanvasRef}
              width={600}
              height={600}
              className="absolute inset-0 w-full h-full pointer-events-none z-20"
            />

            {/* MAIN DIAL LAYER (click blank area to flip between Power & Speedometer) */}
            <div
              onClick={onFlipPage}
              className={`dial-main-layer absolute inset-0 ${pickerOpen ? 'dial-main-slid-down' : ''} ${
                data.running && !launching && !stopping ? 'cursor-pointer' : ''
              }`}
              title="点击表盘空白处切换速度盘 / 启停页"
            >
              {/* PAGE 0: IGNITION / POWER PAGE */}
              <div
                ref={powerPageRef}
                className={`dial-page absolute inset-0 z-10 ${
                  dialPage === 'power' ? 'dial-page-visible' : 'dial-page-hidden'
                }`}
              >
                <canvas
                  ref={powerCanvasRef}
                  width={600}
                  height={600}
                  className="absolute inset-0 w-full h-full pointer-events-none"
                />

                {/* Hero Machined Push-To-Start Ignition Switch */}
                <div
                  onClick={(e) => e.stopPropagation()}
                  className={`ignition-socket absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 w-[194px] h-[194px] rounded-full flex items-center justify-center z-10 ${
                    data.running ? 'is-running' : ''
                  } ${stopping ? 'is-stopping' : ''}`}
                >
                  <div className="ignition-bezel-ring w-[184px] h-[184px] rounded-full p-[3px] flex items-center justify-center">
                    <div className="ignition-light-ring w-full h-full rounded-full p-[3px] flex items-center justify-center bg-[#141514]">
                      <div
                        role="button"
                        tabIndex={0}
                        onClick={(e) => {
                          e.stopPropagation();
                          onPowerAction();
                        }}
                        className={`power-btn-core w-full h-full rounded-full flex flex-col items-center justify-center text-white cursor-pointer pt-1.5 ${
                          data.running ? 'power-btn-stop' : ''
                        }`}
                      >
                        <svg
                          width="44"
                          height="44"
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke={stopping ? '#ff6a3d' : data.running ? '#5bd68a' : 'currentColor'}
                          strokeWidth="2.15"
                          strokeLinecap="round"
                          className="drop-shadow-[0_2px_4px_rgba(0,0,0,0.32)]"
                        >
                          <path d="M12 2.5v9.5" />
                          <path d="M18.36 6.64a9 9 0 1 1-12.73 0" />
                        </svg>

                        {/* Compact Embedded Node Selector Capsule */}
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            setPickerOpen(true);
                          }}
                          onMouseDown={(e) => e.stopPropagation()}
                          className="btn-embedded-node mt-3.5 px-2 py-[2.5px] rounded-full flex items-center gap-1 max-w-[104px] cursor-pointer"
                          title="点击切换出口节点"
                        >
                          <span
                            className={`w-1.5 h-1.5 rounded-full shrink-0 ${data.running ? 'pulse-dot' : ''}`}
                            style={{ backgroundColor: data.running ? '#5bd68a' : '#ffd2bd' }}
                          />
                          <FlagIcon code={activeCountry.code} size="xs" />
                          <span className="text-[9.5px] font-mono font-medium text-white/95 truncate">
                            {cleanNodeName(activeName)}
                          </span>
                          <span className="text-[8px] text-white/70 shrink-0">▾</span>
                        </button>
                      </div>
                    </div>
                  </div>
                </div>

                <div className="absolute top-[84.5%] left-1/2 -translate-x-1/2 whitespace-nowrap text-[10px] font-mono text-[#8e8c84] tracking-wide tabular-nums pointer-events-none z-10">
                  {stopping
                    ? 'STOPPING · 正在断开加密隧道...'
                    : launching
                    ? 'IGNITING · 冲刺建立加密隧道...'
                    : data.running
                    ? `ACTIVE · 已运行 ${formatUptime(data.telemetry.uptime_secs)}`
                    : 'READY · 点击启动内核'}
                </div>
              </div>

              {/* PAGE 1: LIVE SPEEDOMETER PAGE */}
              <div
                ref={speedPageRef}
                className={`dial-page absolute inset-0 z-10 ${
                  dialPage === 'speed' ? 'dial-page-visible' : 'dial-page-hidden'
                }`}
              >
                <canvas
                  ref={speedCanvasRef}
                  width={600}
                  height={600}
                  className="w-full h-full block pointer-events-none"
                />

                <div className="absolute left-1/2 top-[48%] -translate-x-1/2 -translate-y-1/2 flex flex-col items-center pointer-events-none">
                  <div className="flex items-baseline gap-1 tabular-nums">
                    <span className="text-[38px] sm:text-[42px] font-bold leading-none tracking-tight text-[#f2f1ec]">
                      {mainRate.val}
                    </span>
                    <span className="text-xs font-semibold text-[#5bd68a]">{mainRate.unit}</span>
                  </div>
                  <div className="mt-1.5 flex items-center gap-2 text-[10.5px] whitespace-nowrap tabular-nums">
                    <span className="text-[#5bd68a] font-medium">
                      ↓ {formatRateStr(data.running ? data.telemetry.down_bps : 0)}
                    </span>
                    <span className="text-[#4a4c47]">|</span>
                    <span className="text-[#ff6a3d] font-medium">
                      ↑ {formatRateStr(data.running ? data.telemetry.up_bps : 0)}
                    </span>
                  </div>
                </div>

                {/* Sculpted Lower LCD Bay Hit-Target + Upper Shelf Node Selector */}
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    setPickerOpen(true);
                  }}
                  onMouseEnter={() => {
                    bayHoveredRef.current = true;
                  }}
                  onMouseLeave={() => {
                    bayHoveredRef.current = false;
                  }}
                  className="absolute top-[67.5%] left-1/2 -translate-x-1/2 w-[68%] h-[27%] flex flex-col items-center justify-start pt-[6px] cursor-pointer z-10 bg-transparent border-0 outline-none"
                  title="点击下滑选择出口节点"
                >
                  <div className="w-[152px] flex items-center justify-between gap-1.5 min-w-0 leading-none">
                    <div className="flex items-center gap-1 min-w-0">
                      <span className="w-1.5 h-1.5 rounded-full bg-[#5bd68a] pulse-dot shrink-0" />
                      <FlagIcon code={activeCountry.code} size="xs" />
                      <span className="text-[10px] font-mono font-semibold text-[#f2f1ec] truncate">
                        {cleanNodeName(activeName)}
                      </span>
                    </div>
                    <div className="flex items-center gap-1 shrink-0">
                      <span
                        className={`text-[9.5px] font-mono tabular-nums ${
                          activeMs === '超时' ? 'text-[#ff6a3d]' : 'text-[#5bd68a]'
                        }`}
                      >
                        {activeMs}
                      </span>
                      <span className="text-[8.5px] text-[#8c8a82]">▾</span>
                    </div>
                  </div>
                </button>
              </div>
            </div>

            {/* CIRCULAR NODE PICKER LAYER (Chord-Adaptive Pill Rows + Curved Arc Scrollbar) */}
            <div
              ref={pickerLayerRef}
              className={`dial-picker-layer absolute inset-0 z-30 bg-[#151614] overflow-hidden ${
                pickerOpen ? 'dial-picker-open' : ''
              }`}
            >
              <canvas
                ref={pickerChromeRef}
                width={600}
                height={600}
                className="absolute inset-0 w-full h-full pointer-events-none z-20"
              />

              <div className="absolute top-[18px] left-0 right-0 flex flex-col items-center gap-0.5 pointer-events-none z-30">
                <span className="text-[8.5px] tracking-[0.22em] uppercase font-bold text-[#e84c1e]">
                  NODE SELECTOR
                </span>
                <span className="text-[10.5px] font-semibold text-[#e6e4dc] tabular-nums">
                  选择出口节点 · {data.ready_count}/{data.nodes.length}
                </span>
              </div>

              <div
                ref={pickerListRef}
                onScroll={updatePickerCurve}
                className="circular-list-mask circular-scroll absolute inset-x-0 top-[54px] bottom-[46px] overflow-y-auto py-3 flex flex-col items-center gap-[5px] z-10"
              >
                {data.nodes.map((n) => {
                  const c = detectCountry(n.name);
                  const dotColor = n.is_active
                    ? '#e84c1e'
                    : n.cooldown_remaining_secs > 0
                    ? '#c98a2e'
                    : n.latency_ms === 0
                    ? '#e84c1e'
                    : '#5bd68a';
                  const latText =
                    n.latency_ms == null ? '--' : n.latency_ms === 0 ? '超时' : `${n.latency_ms}ms`;
                  const latColor =
                    n.latency_ms === 0
                      ? 'text-[#ff6a3d]'
                      : n.latency_ms != null && n.latency_ms < 120
                      ? 'text-[#5bd68a]'
                      : 'text-[#9c9a91]';
                  return (
                    <div
                      key={n.name}
                      onClick={() => {
                        onSelectNode(n.name);
                        setPickerOpen(false);
                      }}
                      className={`picker-node-row shrink-0 h-[30px] px-3 rounded-full flex items-center justify-between gap-2 text-[11px] cursor-pointer ${
                        n.is_active
                          ? 'bg-[#262824] border border-[#e84c1e]/70 shadow-[0_2px_10px_rgba(232,76,30,0.14)]'
                          : 'bg-white/[0.035] hover:bg-white/[0.08] border border-white/[0.04]'
                      }`}
                    >
                      <div className="flex items-center gap-1.5 min-w-0">
                        <span className="w-1.5 h-1.5 rounded-full shrink-0" style={{ background: dotColor }} />
                        <FlagIcon code={c.code} size="xs" />
                        <span
                          className={`text-[9.5px] font-mono font-bold shrink-0 ${
                            n.is_active ? 'text-[#ff6a3d]' : 'text-[#8c8a82]'
                          }`}
                        >
                          {c.code}
                        </span>
                        <span className={`truncate ${n.is_active ? 'font-semibold text-white' : 'text-[#dcdad2]'}`}>
                          {cleanNodeName(n.name)}
                        </span>
                      </div>
                      <div className="flex items-center gap-1.5 shrink-0 tabular-nums">
                        {n.cooldown_remaining_secs > 0 && (
                          <span className="text-[9.5px] font-mono text-[#ff6a3d]">
                            {n.cooldown_remaining_secs}s
                          </span>
                        )}
                        <span className={`text-[10px] font-mono font-medium ${latColor}`}>{latText}</span>
                      </div>
                    </div>
                  );
                })}
              </div>

              <div className="absolute bottom-[6%] left-1/2 -translate-x-1/2 z-30">
                <button
                  onClick={() => setPickerOpen(false)}
                  className="dial-pager-pill h-[24px] px-3.5 rounded-full whitespace-nowrap flex items-center justify-center text-[10.5px] font-medium text-[#d6d4cc] hover:text-white cursor-pointer"
                >
                  ▲ 返回表盘
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
