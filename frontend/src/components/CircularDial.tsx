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

const LAUNCH_DUR = 1650;
const STOP_DUR = 920;

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

  const smoothDownRef = useRef(0);
  const smoothUpRef = useRef(0);
  const streaksRef = useRef(
    Array.from({ length: 68 }, (_, i) => ({
      angle: (i / 68) * Math.PI * 2 + (Math.random() - 0.5) * 0.25,
      z: 0.12 + Math.random() * 0.88,
      speedMult: 0.75 + Math.random() * 0.7,
      thickness: 1.4 + Math.random() * 2.2,
      color: i % 5 === 0 ? '#ff6a3d' : i % 3 === 0 ? '#5bd68a' : '#e8e6de',
    }))
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
          const p = Math.min(1, (now - cur.launchStartMs) / LAUNCH_DUR);
          const accel = Math.pow(p, 2.3);
          const dzBase = 0.01 + accel * 0.072;
          const masterAlpha = p < 0.12 ? p / 0.12 : p < 0.64 ? 1 : Math.max(0, 1 - (p - 0.64) / 0.36);

          if (powerPageRef.current && p < 0.64 && cur.dialPage === 'power') {
            const shrink = 1 - Math.pow(p / 0.64, 1.8) * 0.28;
            const fadeBtn = Math.max(0, 1 - Math.pow(p / 0.6, 2.0));
            powerPageRef.current.style.transform = `scale(${shrink.toFixed(3)})`;
            powerPageRef.current.style.opacity = fadeBtn.toFixed(3);
          }
          if (p >= 0.62 && cur.dialPage !== 'speed') {
            if (powerPageRef.current) {
              powerPageRef.current.style.transform = '';
              powerPageRef.current.style.opacity = '';
            }
            cur.onLaunchShowSpeed();
          }

          if (masterAlpha > 0.005) {
            wCtx.save();
            wCtx.lineCap = 'round';
            const fov = 72;
            streaksRef.current.forEach((s) => {
              const prevZ = s.z;
              s.z -= dzBase * s.speedMult;
              if (s.z <= 0.04) {
                s.z = 0.88 + Math.random() * 0.14;
                s.angle += (Math.random() - 0.5) * 0.35;
                return;
              }
              const tailZ = Math.min(1.05, prevZ + dzBase * (2.2 + accel * 4.5));
              const rHead = Math.min(315, fov / s.z);
              const rTail = Math.min(310, fov / tailZ);
              if (rHead < 26) return;
              const cos = Math.cos(s.angle),
                sin = Math.sin(s.angle);
              const x0 = cx + cos * rTail,
                y0 = cy + sin * rTail;
              const x1 = cx + cos * rHead,
                y1 = cy + sin * rHead;
              const depthFade = Math.min(1, Math.max(0, (rHead - 26) / 110));
              const grad = wCtx.createLinearGradient(x0, y0, x1, y1);
              grad.addColorStop(0, 'rgba(255,255,255,0)');
              grad.addColorStop(1, s.color);
              wCtx.globalAlpha = masterAlpha * depthFade;
              wCtx.beginPath();
              wCtx.moveTo(x0, y0);
              wCtx.lineTo(x1, y1);
              wCtx.strokeStyle = grad;
              wCtx.lineWidth = s.thickness * (0.45 + depthFade * 1.15);
              wCtx.stroke();
            });
            wCtx.restore();
          }
          if (p >= 1) {
            if (powerPageRef.current) {
              powerPageRef.current.style.transform = '';
              powerPageRef.current.style.opacity = '';
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
        const launchP = cur.launching ? Math.min(1, (now - cur.launchStartMs) / (LAUNCH_DUR * 0.62)) : 0;
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

        pCtx.beginPath();
        pCtx.arc(cx, cy, 280, 0, Math.PI * 2);
        pCtx.strokeStyle = 'rgba(255,255,255,0.06)';
        pCtx.lineWidth = 2;
        pCtx.stroke();

        const startAng = (152 * Math.PI) / 180;
        const endAng = (388 * Math.PI) / 180;
        const sweep = endAng - startAng;

        for (let i = 0; i <= 40; i++) {
          const ang = startAng + (i / 40) * sweep;
          const isMajor = i % 10 === 0;
          const isMid = i % 5 === 0 && !isMajor;
          const r1 = 272;
          const r2 = isMajor ? 260 : isMid ? 263 : 266;
          pCtx.beginPath();
          pCtx.moveTo(cx + Math.cos(ang) * r1, cy + Math.sin(ang) * r1);
          pCtx.lineTo(cx + Math.cos(ang) * r2, cy + Math.sin(ang) * r2);
          pCtx.strokeStyle = isMajor
            ? running && !cur.stopping
              ? 'rgba(91,214,138,0.58)'
              : 'rgba(255,106,61,0.62)'
            : isMid
            ? '#5c5e58'
            : '#343632';
          pCtx.lineWidth = isMajor ? 2.6 : isMid ? 1.6 : 1.2;
          pCtx.stroke();
        }

        pCtx.lineCap = 'round';
        pCtx.beginPath();
        pCtx.arc(cx, cy, 248, startAng, endAng);
        pCtx.strokeStyle = '#232522';
        pCtx.lineWidth = 6.5;
        pCtx.stroke();

        if (cur.stopping) {
          const remain = Math.max(0, 1 - Math.pow(stopP, 0.75));
          if (remain > 0.01) {
            pCtx.save();
            pCtx.beginPath();
            pCtx.arc(cx, cy, 248, startAng, startAng + sweep * remain);
            pCtx.strokeStyle = stopP < 0.45 ? '#5bd68a' : '#ff6a3d';
            pCtx.lineWidth = 6.5;
            pCtx.shadowColor = stopP < 0.45 ? 'rgba(91,214,138,0.7)' : 'rgba(255,106,61,0.75)';
            pCtx.shadowBlur = 12;
            pCtx.stroke();
            pCtx.restore();
          }
        } else if (cur.launching && launchP > 0.01) {
          pCtx.save();
          pCtx.beginPath();
          pCtx.arc(cx, cy, 248, startAng, startAng + sweep * launchP);
          pCtx.strokeStyle = '#ff7b4d';
          pCtx.lineWidth = 6.5;
          pCtx.shadowColor = 'rgba(255,106,61,0.75)';
          pCtx.shadowBlur = 14;
          pCtx.stroke();
          pCtx.restore();
        } else if (running) {
          pCtx.beginPath();
          pCtx.arc(cx, cy, 248, startAng, endAng);
          pCtx.strokeStyle = 'rgba(91,214,138,0.35)';
          pCtx.lineWidth = 3;
          pCtx.stroke();
        }

        pCtx.beginPath();
        pCtx.arc(cx, cy, 198, 0, Math.PI * 2);
        pCtx.strokeStyle = running && !cur.stopping ? 'rgba(91,214,138,0.16)' : 'rgba(255,255,255,0.06)';
        pCtx.lineWidth = 1.5;
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
        sCtx.strokeStyle = 'rgba(255,255,255,0.06)';
        sCtx.lineWidth = 2;
        sCtx.stroke();

        sCtx.lineCap = 'round';
        sCtx.beginPath();
        sCtx.arc(cx, cy, rOuter, startAng, endAng);
        sCtx.strokeStyle = '#2a2c29';
        sCtx.lineWidth = 16;
        sCtx.stroke();

        sCtx.beginPath();
        sCtx.arc(cx, cy, rInner, startAng, endAng);
        sCtx.strokeStyle = '#242623';
        sCtx.lineWidth = 7.5;
        sCtx.stroke();

        for (let i = 0; i <= 40; i++) {
          const ang = startAng + (i / 40) * sweep;
          const isMajor = i % 10 === 0;
          const isMid = i % 5 === 0 && !isMajor;
          const r1 = isMajor ? 270 : isMid ? 268 : 266;
          const r2 = isMajor ? 257 : isMid ? 260 : 262;
          sCtx.beginPath();
          sCtx.moveTo(cx + Math.cos(ang) * r1, cy + Math.sin(ang) * r1);
          sCtx.lineTo(cx + Math.cos(ang) * r2, cy + Math.sin(ang) * r2);
          sCtx.strokeStyle = isMajor ? '#9c9a91' : '#4a4c47';
          sCtx.lineWidth = isMajor ? 3 : 1.5;
          sCtx.stroke();
        }

        const labels = ['0', '100K', '1M', '10M', '100M'];
        sCtx.font = '600 16px ui-monospace, SFMono-Regular, Menlo, monospace';
        sCtx.fillStyle = '#7e7c74';
        sCtx.textAlign = 'center';
        sCtx.textBaseline = 'middle';
        labels.forEach((lbl, idx) => {
          const ang = startAng + (idx / 4) * sweep;
          sCtx.fillText(lbl, cx + Math.cos(ang) * 185, cy + Math.sin(ang) * 185);
        });

        const dn = running ? bpsToNorm(smoothDownRef.current) : 0;
        const un = running ? bpsToNorm(smoothUpRef.current) : 0;

        if (dn > 0.005) {
          const grad = sCtx.createLinearGradient(cx - rOuter, cy, cx + rOuter, cy);
          grad.addColorStop(0, '#1f8a4c');
          grad.addColorStop(0.6, '#5bd68a');
          grad.addColorStop(1, '#8ef0b2');
          sCtx.beginPath();
          sCtx.arc(cx, cy, rOuter, startAng, startAng + dn * sweep);
          sCtx.strokeStyle = grad;
          sCtx.lineWidth = 16;
          sCtx.shadowColor = 'rgba(91,214,138,0.35)';
          sCtx.shadowBlur = 14;
          sCtx.stroke();
          sCtx.shadowBlur = 0;
        }

        if (un > 0.005) {
          sCtx.beginPath();
          sCtx.arc(cx, cy, rInner, startAng, startAng + un * sweep);
          sCtx.strokeStyle = '#ff6a3d';
          sCtx.lineWidth = 7.5;
          sCtx.shadowColor = 'rgba(255,106,61,0.35)';
          sCtx.shadowBlur = 10;
          sCtx.stroke();
          sCtx.shadowBlur = 0;
        }

        const tipAng = startAng + dn * sweep;
        sCtx.beginPath();
        sCtx.moveTo(cx + Math.cos(tipAng) * (rOuter - 14), cy + Math.sin(tipAng) * (rOuter - 14));
        sCtx.lineTo(cx + Math.cos(tipAng) * (rOuter + 16), cy + Math.sin(tipAng) * (rOuter + 16));
        sCtx.strokeStyle = running ? '#e84c1e' : '#555752';
        sCtx.lineWidth = 5;
        sCtx.stroke();

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
          const activeBar = running && tot >= 1;
          const len = activeBar ? minBarLen + norm * (maxBarLen - minBarLen) : minBarLen;
          const rIn = rBase - len;
          const cos = Math.cos(ang),
            sin = Math.sin(ang);
          sCtx.beginPath();
          sCtx.moveTo(cx + cos * rBase, cy + sin * rBase);
          sCtx.lineTo(cx + cos * rIn, cy + sin * rIn);
          sCtx.strokeStyle = !activeBar ? '#363b36' : u > d ? '#ff6a3d' : '#5bd68a';
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
      <div className="w-full h-full rounded-full dial-bezel p-[10px] flex items-center justify-center">
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
