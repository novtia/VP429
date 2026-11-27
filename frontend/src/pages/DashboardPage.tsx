import React, { useEffect, useLayoutEffect, useRef } from 'react';
import { CircularDial } from '../components/CircularDial';
import { FlagIcon } from '../components/FlagIcon';
import {
  cleanNodeName,
  detectCountry,
  formatBytes,
  formatRateStr,
  formatUptime,
  type FullStateDto,
  type OutboundMode,
  type RotationKey,
} from '../types';

interface DashboardPageProps {
  data: FullStateDto;
  peakDownBps: number;
  dialPage: 'power' | 'speed';
  launching: boolean;
  launchStartMs: number;
  stopping: boolean;
  stopStartMs: number;
  onFlipPage: () => void;
  onPowerAction: () => void;
  onSelectNode: (name: string) => void;
  onToggleSys: () => void;
  onToggleTun: () => void;
  onToggleChain: () => void;
  onElevate: () => void;
  onSetOutbound: (mode: OutboundMode) => void;
  onSetRotation: (rot: RotationKey) => void;
  onCheckExitIp: () => void;
  onLaunchComplete: () => void;
  onStopComplete: () => void;
  onLaunchShowSpeed: () => void;
  onStopShowPower: () => void;
}

function buildPanelPaths(W: number, H: number) {
  const cx = W * 0.5,
    cy = H * 0.5,
    R = H * 0.34,
    r = 16,
    f = 16;
  const xTopL = cx + 46,
    xTopR = cx + 60;
  const xBotL = cx - 60,
    xBotR = cx - 46;
  const yLeftT = cy - 18,
    yLeftB = cy - 4;
  const yRightT = cy + 18,
    yRightB = cy + 32;
  const n = (v: number) => v.toFixed(2);
  const hFillet = (yLine: number, sideX: number, sideY: number) => {
    const fcy = yLine + sideY * f;
    const dy = fcy - cy;
    const dist = R + f;
    const dx = Math.sqrt(Math.max(1, dist * dist - dy * dy));
    const fcx = cx + sideX * dx;
    const t = R / dist;
    return { lx: fcx, ly: yLine, tx: cx + (fcx - cx) * t, ty: cy + (fcy - cy) * t };
  };
  const vFillet = (xLine: number, sideX: number, sideY: number) => {
    const fcx = xLine + sideX * f;
    const dx = fcx - cx;
    const dist = R + f;
    const dy = Math.sqrt(Math.max(1, dist * dist - dx * dx));
    const fcy = cy + sideY * dy;
    const t = R / dist;
    return { lx: xLine, ly: fcy, tx: cx + (fcx - cx) * t, ty: cy + (fcy - cy) * t };
  };

  const tl = (() => {
    const x0 = 1, y0 = 1, x1 = xTopL, y1 = yLeftT;
    const fv = vFillet(x1, -1, -1);
    const fh = hFillet(y1, -1, -1);
    return [
      `M ${n(x0 + r)} ${n(y0)}`, `L ${n(x1 - r)} ${n(y0)}`, `A ${r} ${r} 0 0 1 ${n(x1)} ${n(y0 + r)}`,
      `L ${n(fv.lx)} ${n(fv.ly)}`, `A ${f} ${f} 0 0 1 ${n(fv.tx)} ${n(fv.ty)}`,
      `A ${n(R)} ${n(R)} 0 0 0 ${n(fh.tx)} ${n(fh.ty)}`, `A ${f} ${f} 0 0 1 ${n(fh.lx)} ${n(fh.ly)}`,
      `L ${n(x0 + r)} ${n(y1)}`, `A ${r} ${r} 0 0 1 ${n(x0)} ${n(y1 - r)}`,
      `L ${n(x0)} ${n(y0 + r)}`, `A ${r} ${r} 0 0 1 ${n(x0 + r)} ${n(y0)} Z`,
    ].join(' ');
  })();

  const tr = (() => {
    const x0 = xTopR, y0 = 1, x1 = W - 1, y1 = yRightT;
    const fh = hFillet(y1, 1, -1);
    const fv = vFillet(x0, 1, -1);
    return [
      `M ${n(x0 + r)} ${n(y0)}`, `L ${n(x1 - r)} ${n(y0)}`, `A ${r} ${r} 0 0 1 ${n(x1)} ${n(y0 + r)}`,
      `L ${n(x1)} ${n(y1 - r)}`, `A ${r} ${r} 0 0 1 ${n(x1 - r)} ${n(y1)}`,
      `L ${n(fh.lx)} ${n(fh.ly)}`, `A ${f} ${f} 0 0 1 ${n(fh.tx)} ${n(fh.ty)}`,
      `A ${n(R)} ${n(R)} 0 0 0 ${n(fv.tx)} ${n(fv.ty)}`, `A ${f} ${f} 0 0 1 ${n(fv.lx)} ${n(fv.ly)}`,
      `L ${n(x0)} ${n(y0 + r)}`, `A ${r} ${r} 0 0 1 ${n(x0 + r)} ${n(y0)} Z`,
    ].join(' ');
  })();

  const bl = (() => {
    const x0 = 1, y0 = yLeftB, x1 = xBotL, y1 = H - 1;
    const fh = hFillet(y0, -1, 1);
    const fv = vFillet(x1, -1, 1);
    return [
      `M ${n(x0 + r)} ${n(y0)}`, `L ${n(fh.lx)} ${n(fh.ly)}`, `A ${f} ${f} 0 0 1 ${n(fh.tx)} ${n(fh.ty)}`,
      `A ${n(R)} ${n(R)} 0 0 0 ${n(fv.tx)} ${n(fv.ty)}`, `A ${f} ${f} 0 0 1 ${n(fv.lx)} ${n(fv.ly)}`,
      `L ${n(x1)} ${n(y1 - r)}`, `A ${r} ${r} 0 0 1 ${n(x1 - r)} ${n(y1)}`,
      `L ${n(x0 + r)} ${n(y1)}`, `A ${r} ${r} 0 0 1 ${n(x0)} ${n(y1 - r)}`,
      `L ${n(x0)} ${n(y0 + r)}`, `A ${r} ${r} 0 0 1 ${n(x0 + r)} ${n(y0)} Z`,
    ].join(' ');
  })();

  const br = (() => {
    const x0 = xBotR, y0 = yRightB, x1 = W - 1, y1 = H - 1;
    const fv = vFillet(x0, 1, 1);
    const fh = hFillet(y0, 1, 1);
    return [
      `M ${n(fh.lx)} ${n(fh.ly)}`, `L ${n(x1 - r)} ${n(y0)}`, `A ${r} ${r} 0 0 1 ${n(x1)} ${n(y0 + r)}`,
      `L ${n(x1)} ${n(y1 - r)}`, `A ${r} ${r} 0 0 1 ${n(x1 - r)} ${n(y1)}`,
      `L ${n(x0 + r)} ${n(y1)}`, `A ${r} ${r} 0 0 1 ${n(x0)} ${n(y1 - r)}`,
      `L ${n(fv.lx)} ${n(fv.ly)}`, `A ${f} ${f} 0 0 1 ${n(fv.tx)} ${n(fv.ty)}`,
      `A ${n(R)} ${n(R)} 0 0 0 ${n(fh.tx)} ${n(fh.ty)}`, `A ${f} ${f} 0 0 1 ${n(fh.lx)} ${n(fh.ly)} Z`,
    ].join(' ');
  })();

  return { tl, tr, bl, br, cx, cy, R, xTopMid: (xTopL + xTopR) * 0.5, xBotMid: (xBotL + xBotR) * 0.5, yLeftMid: (yLeftT + yLeftB) * 0.5, yRightMid: (yRightT + yRightB) * 0.5 };
}

export const DashboardPage: React.FC<DashboardPageProps> = ({
  data,
  peakDownBps,
  dialPage,
  launching,
  launchStartMs,
  stopping,
  stopStartMs,
  onFlipPage,
  onPowerAction,
  onSelectNode,
  onToggleSys,
  onToggleTun,
  onToggleChain,
  onElevate,
  onSetOutbound,
  onSetRotation,
  onCheckExitIp,
  onLaunchComplete,
  onStopComplete,
  onLaunchShowSpeed,
  onStopShowPower,
}) => {
  const stageRef = useRef<HTMLDivElement | null>(null);
  const svgRef = useRef<SVGSVGElement | null>(null);

  // Synchronous pre-paint 1:1 pixel SVG geometry + pin-to-dial 45° PCB trace router (zero frame lag / zero flicker)
  useLayoutEffect(() => {
    const setAttr = (id: string, attrs: Record<string, string | number>) => {
      const el = document.getElementById(id);
      if (!el) return;
      for (const [k, v] of Object.entries(attrs)) {
        el.setAttribute(k, String(v));
      }
    };

    const layoutGeometry = () => {
      const stage = stageRef.current;
      const svg = svgRef.current;
      if (!stage || !svg) return;
      const sRect = stage.getBoundingClientRect();
      if (sRect.width < 10 || sRect.height < 10) return;

      const W = sRect.width;
      const H = sRect.height;
      svg.setAttribute('viewBox', `0 0 ${W.toFixed(2)} ${H.toFixed(2)}`);

      const p = buildPanelPaths(W, H);
      const { cx, cy, R, xTopMid, xBotMid, yLeftMid, yRightMid } = p;
      const rDock = R + 10;
      const f = (v: number) => v.toFixed(2);

      // Keep side panels & wings mathematically locked outside the dial arc rail (zero overlap at any width)
      const sideW = Math.max(172, Math.round(cx - rDock - 10));
      const xTopL = cx + 46;
      const xTopR = cx + 60;
      const xBotL = cx - 60;
      const xBotR = cx - 46;
      stage.style.setProperty('--side-w', `${sideW}px`);
      stage.style.setProperty('--wing-tl-w', `${Math.max(110, Math.round(xTopL - sideW))}px`);
      stage.style.setProperty('--wing-tr-l', `${Math.round(xTopR)}px`);
      stage.style.setProperty('--wing-tr-w', `${Math.max(76, Math.round(W - sideW - xTopR))}px`);
      stage.style.setProperty('--wing-bl-w', `${Math.max(76, Math.round(xBotL - sideW))}px`);
      stage.style.setProperty('--wing-br-l', `${Math.round(xBotR)}px`);
      stage.style.setProperty('--wing-br-w', `${Math.max(110, Math.round(W - sideW - xBotR))}px`);

      // Panels & clip paths
      setAttr('panel-tl', { d: p.tl });
      setAttr('panel-tr', { d: p.tr });
      setAttr('panel-bl', { d: p.bl });
      setAttr('panel-br', { d: p.br });
      setAttr('clip-tl', { d: p.tl });
      setAttr('clip-tr', { d: p.tr });
      setAttr('clip-bl', { d: p.bl });
      setAttr('clip-br', { d: p.br });
      setAttr('pcb-grid-rect', { width: f(W), height: f(H) });

      // Inter-card bus trenches
      setAttr('tr-top-g', { x1: f(xTopMid), y1: 2, x2: f(xTopMid), y2: f(cy - R + 6) });
      setAttr('tr-top-r1', { x1: f(xTopMid - 2), y1: 4, x2: f(xTopMid - 2), y2: f(cy - R + 4) });
      setAttr('tr-top-r2', { x1: f(xTopMid + 2), y1: 4, x2: f(xTopMid + 2), y2: f(cy - R + 4) });
      setAttr('tr-top-p', { d: `M ${f(xTopMid)} 4 L ${f(xTopMid)} ${f(cy - R + 4)}` });

      setAttr('tr-left-g', { x1: 2, y1: f(yLeftMid), x2: f(cx - R + 6), y2: f(yLeftMid) });
      setAttr('tr-left-r1', { x1: 4, y1: f(yLeftMid - 2), x2: f(cx - R + 4), y2: f(yLeftMid - 2) });
      setAttr('tr-left-r2', { x1: 4, y1: f(yLeftMid + 2), x2: f(cx - R + 4), y2: f(yLeftMid + 2) });
      setAttr('tr-left-p', { d: `M 4 ${f(yLeftMid)} L ${f(cx - R + 4)} ${f(yLeftMid)}` });

      setAttr('tr-right-g', { x1: f(cx + R - 6), y1: f(yRightMid), x2: f(W - 2), y2: f(yRightMid) });
      setAttr('tr-right-r1', { x1: f(cx + R - 4), y1: f(yRightMid - 2), x2: f(W - 4), y2: f(yRightMid - 2) });
      setAttr('tr-right-r2', { x1: f(cx + R - 4), y1: f(yRightMid + 2), x2: f(W - 4), y2: f(yRightMid + 2) });
      setAttr('tr-right-p', { d: `M ${f(W - 4)} ${f(yRightMid)} L ${f(cx + R - 4)} ${f(yRightMid)}` });

      setAttr('tr-bot-g', { x1: f(xBotMid), y1: f(cy + R - 6), x2: f(xBotMid), y2: f(H - 2) });
      setAttr('tr-bot-r1', { x1: f(xBotMid - 2), y1: f(cy + R - 4), x2: f(xBotMid - 2), y2: f(H - 4) });
      setAttr('tr-bot-r2', { x1: f(xBotMid + 2), y1: f(cy + R - 4), x2: f(xBotMid + 2), y2: f(H - 4) });
      setAttr('tr-bot-p', { d: `M ${f(xBotMid)} ${f(H - 4)} L ${f(xBotMid)} ${f(cy + R - 4)}` });

      // Dial moat rings & PCB arc rail
      setAttr('moat-glow-c', { cx: f(cx), cy: f(cy), r: f(R + 8) });
      setAttr('moat-well-c', { cx: f(cx), cy: f(cy), r: f(R + 1.5) });
      setAttr('moat-lip-outer-c', { cx: f(cx), cy: f(cy), r: f(R + 1.5) });
      setAttr('moat-lip-shadow-c', { cx: f(cx), cy: f(cy), r: f(R + 0.4) });
      setAttr('moat-knurl-shadow-c', { cx: f(cx), cy: f(cy), r: f(R - 4.4) });
      setAttr('moat-knurl-crest-c', { cx: f(cx), cy: f(cy), r: f(R - 4.4) });
      setAttr('moat-knurl-outer-c', { cx: f(cx), cy: f(cy), r: f(R - 1.4) });
      setAttr('moat-knurl-inner-c', { cx: f(cx), cy: f(cy), r: f(R - 7.4) });
      setAttr('moat-inner-c', { cx: f(cx), cy: f(cy), r: f(R - 9) });
      setAttr('pcb-arc-c', { cx: f(cx), cy: f(cy), r: f(R + 10) });

      // BL serpentine bus & BR gold-finger pads
      const serpD = `M 22 ${f(H - 88)} L 82 ${f(H - 88)} L 90 ${f(H - 96)} L 98 ${f(H - 88)} L 106 ${f(H - 96)} L 114 ${f(H - 88)} L ${f(cx - R * 0.96)} ${f(H - 88)} L ${f(cx - R * 0.78)} ${f(H - 120)} L ${f(cx - R * 0.6)} ${f(H - 120)}`;
      setAttr('pcb-bl-serp', { d: serpD });
      setAttr('pcb-bl-serp-pulse', { d: serpD });
      setAttr('pad-g0', { x: f(W - 62), y: f(H - 76) });
      setAttr('pad-g1', { x: f(W - 51), y: f(H - 76) });
      setAttr('pad-g2', { x: f(W - 40), y: f(H - 76) });
      setAttr('pad-g3', { x: f(W - 29), y: f(H - 76) });

      // Pin-to-dial 45° PCB trace router (runs after CSS vars flush layout so pins are exact)
      const fmt = (v: number) => v.toFixed(1);
      const pinPt = (id: string) => {
        const el = document.getElementById(id);
        if (!el) return null;
        const r = el.getBoundingClientRect();
        return {
          x: r.left + r.width / 2 - sRect.left,
          y: r.top + r.height / 2 - sRect.top,
        };
      };
      const dockPt = (deg: number) => {
        const rad = (deg * Math.PI) / 180;
        return { x: cx + rDock * Math.cos(rad), y: cy + rDock * Math.sin(rad) };
      };
      const octPath = (pt: { x: number; y: number }, t: { x: number; y: number }, reverse = false) => {
        const dx = t.x - pt.x,
          dy = t.y - pt.y;
        const adx = Math.abs(dx),
          ady = Math.abs(dy);
        const dirX = dx >= 0 ? 1 : -1,
          dirY = dy >= 0 ? 1 : -1;
        let pts =
          adx >= ady + 2
            ? [pt, { x: t.x - dirX * ady, y: pt.y }, t]
            : [
                pt,
                { x: pt.x + dirX * Math.min(8, adx * 0.35), y: pt.y },
                { x: t.x, y: pt.y + dirY * Math.max(0, adx - Math.min(8, adx * 0.35)) },
                t,
              ];
        if (reverse) pts = pts.reverse();
        return pts.map((q, i) => `${i === 0 ? 'M' : 'L'} ${fmt(q.x)} ${fmt(q.y)}`).join(' ');
      };
      const wire = (pinId: string, deg: number, lineId: string, pulseId: string | null, via: string, rev = false) => {
        const pt = pinPt(pinId);
        if (!pt) return;
        const t = dockPt(deg);
        const dLine = octPath(pt, t, false);
        document.getElementById(lineId)?.setAttribute('d', dLine);
        if (pulseId) document.getElementById(pulseId)?.setAttribute('d', rev ? octPath(pt, t, true) : dLine);
        ['o', 'i'].forEach((s) => {
          const c = document.getElementById(`via-${via}-${s}`);
          if (c) {
            c.setAttribute('cx', fmt(t.x));
            c.setAttribute('cy', fmt(t.y));
          }
        });
      };

      wire('pin-sys', 226, 'pcb-line-sys', 'pcb-pulse-sys', 'sys', false);
      wire('pin-tun', 210, 'pcb-line-tun', 'pcb-pulse-tun', 'tun', false);
      wire('pin-chain', 195, 'pcb-line-chain', 'pcb-pulse-chain', 'chain', false);
      wire('pin-rule', -54, 'pcb-line-rule', 'pcb-pulse-rule', 'rule', true);
      wire('pin-global', -47, 'pcb-line-global', 'pcb-pulse-global', 'global', true);
      wire('pin-direct', -40, 'pcb-line-direct', 'pcb-pulse-direct', 'direct', true);
      wire('pin-rot', -5, 'pcb-line-rot', 'pcb-pulse-rot', 'rot', true);
      wire('pin-up', 154, 'pcb-line-up', 'pcb-pulse-up', 'up', false);
      wire('pin-down', 141, 'pcb-line-down', 'pcb-pulse-down', 'down', false);
      wire('pin-egress-1', 37, 'pcb-line-egress-1', 'pcb-pulse-egress', 'eg1', true);
      wire('pin-egress-2', 46, 'pcb-line-egress-2', null, 'eg2', false);
    };

    layoutGeometry();
    const ro = new ResizeObserver(layoutGeometry);
    if (stageRef.current) ro.observe(stageRef.current);
    window.addEventListener('resize', layoutGeometry);
    return () => {
      ro.disconnect();
      window.removeEventListener('resize', layoutGeometry);
    };
  }, []);

  // Re-wire traces when dynamic DOM content changes without resizing
  useEffect(() => {
    const stage = stageRef.current;
    if (stage) {
      window.dispatchEvent(new Event('resize'));
    }
  }, [data.running]);

  const outbound = (data.telemetry.outbound_mode || data.config.outbound_mode || 'rule') as OutboundMode;
  const rotKey: RotationKey =
    data.config.rotation_mode === 'RoundRobinPerRequest'
      ? 'req'
      : typeof data.config.rotation_mode === 'object'
      ? 'int'
      : 'on429';
  const totalBytes = Math.max(1, data.telemetry.total_up + data.telemetry.total_down);
  const upPct = Math.max(6, Math.min(94, (data.telemetry.total_up / totalBytes) * 100));
  const exitCountry = detectCountry(data.telemetry.exit_country_code || data.active_node || '');

  return (
    <div ref={stageRef} className="relative w-full h-full">
      {/* Background SVG: 1:1 Pixel Coordinate Backplane Trenches + 4 Sculpted Panels + PCB Traces */}
      <svg
        ref={svgRef}
        preserveAspectRatio="none"
        className={`absolute inset-0 w-full h-full pointer-events-none overflow-visible ${data.running ? 'pcb-live' : ''}`}
      >
        <defs>
          <pattern id="pcbDotGrid" width="14" height="14" patternUnits="userSpaceOnUse">
            <circle cx="7" cy="7" r="0.65" fill="#d7d5cb" opacity="0.52" />
          </pattern>
          <linearGradient id="panelSurfaceGrad" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#f9f8f4" />
            <stop offset="55%" stopColor="#f4f3ed" />
            <stop offset="100%" stopColor="#eeece5" />
          </linearGradient>
          <radialGradient id="moatGlow" cx="50%" cy="50%" r="50%">
            <stop offset="78%" stopColor="#1f8a4c" stopOpacity="0" />
            <stop offset="90%" stopColor="#1f8a4c" stopOpacity="0.28" />
            <stop offset="96%" stopColor="#e84c1e" stopOpacity="0.16" />
            <stop offset="100%" stopColor="#1f8a4c" stopOpacity="0" />
          </radialGradient>
          {/* Directional lighting for the recessed dial socket & knurled grip ring */}
          <linearGradient id="moatFloorGrad" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#c4c1b5" />
            <stop offset="50%" stopColor="#d3d1c6" />
            <stop offset="100%" stopColor="#e2e0d7" />
          </linearGradient>
          <linearGradient id="moatLipOuter" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#a5a296" />
            <stop offset="55%" stopColor="#d6d4ca" />
            <stop offset="100%" stopColor="#ffffff" />
          </linearGradient>
          <linearGradient id="moatLipInner" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#ffffff" stopOpacity="0.9" />
            <stop offset="50%" stopColor="#dcdad1" stopOpacity="0.6" />
            <stop offset="100%" stopColor="#9f9c90" stopOpacity="0.7" />
          </linearGradient>
          <linearGradient id="knurlGrad" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#fbfaf6" />
            <stop offset="30%" stopColor="#e3e1d8" />
            <stop offset="70%" stopColor="#cbc8bd" />
            <stop offset="100%" stopColor="#eeece5" />
          </linearGradient>
          <clipPath id="clip-panels">
            <path id="clip-tl" />
            <path id="clip-tr" />
            <path id="clip-bl" />
            <path id="clip-br" />
          </clipPath>
        </defs>

        {/* Inter-Card Bus Trenches & Dial Socket Moat (1:1 pixel aligned) */}
        <g>
          <line id="tr-top-g" className="trench-groove" />
          <line id="tr-top-r1" className="trench-rail" />
          <line id="tr-top-r2" className="trench-rail" />
          <path id="tr-top-p" className="pcb-pulse-orange pcb-pulse-slow" />

          <line id="tr-left-g" className="trench-groove" />
          <line id="tr-left-r1" className="trench-rail" />
          <line id="tr-left-r2" className="trench-rail" />
          <path id="tr-left-p" className="pcb-pulse-green" />

          <line id="tr-right-g" className="trench-groove" />
          <line id="tr-right-r1" className="trench-rail" />
          <line id="tr-right-r2" className="trench-rail" />
          <path id="tr-right-p" className="pcb-pulse-orange" />

          <line id="tr-bot-g" className="trench-groove" />
          <line id="tr-bot-r1" className="trench-rail" />
          <line id="tr-bot-r2" className="trench-rail" />
          <path id="tr-bot-p" className="pcb-pulse-green pcb-pulse-slow" />

          <circle id="moat-glow-c" fill="url(#moatGlow)" className="dial-moat-underglow" />
          {/* Socket floor: shadowed toward the light (top-left), lit on the far wall */}
          <circle id="moat-well-c" fill="url(#moatFloorGrad)" />
          {/* Outer socket wall lip (catches light at bottom-right) */}
          <circle id="moat-lip-outer-c" fill="none" stroke="url(#moatLipOuter)" strokeWidth="1.2" />
          <circle id="moat-lip-shadow-c" fill="none" stroke="rgba(25,25,24,0.10)" strokeWidth="1" />
          {/* Fine straight-knurled grip ring: tooth shadows, then lit tooth crests */}
          <circle
            id="moat-knurl-shadow-c"
            fill="none"
            stroke="#8f8c80"
            strokeOpacity="0.55"
            strokeWidth="5.2"
            strokeDasharray="1.15 1.45"
            strokeDashoffset="-0.55"
          />
          <circle
            id="moat-knurl-crest-c"
            fill="none"
            stroke="url(#knurlGrad)"
            strokeWidth="5"
            strokeDasharray="0.95 1.65"
          />
          {/* Knurl band chamfers */}
          <circle id="moat-knurl-outer-c" fill="none" stroke="url(#moatLipInner)" strokeWidth="0.8" />
          <circle id="moat-knurl-inner-c" fill="none" stroke="url(#moatLipOuter)" strokeWidth="0.8" />
          {/* Bezel base contact shadow */}
          <circle id="moat-inner-c" fill="none" stroke="rgba(25,25,24,0.28)" strokeWidth="1.4" />
        </g>

        <g className="panel-group-relief">
          <path id="panel-tl" className="panel-shape" />
          <path id="panel-tr" className="panel-shape" />
          <path id="panel-bl" className="panel-shape" />
          <path id="panel-br" className="panel-shape" />
        </g>

        <g clipPath="url(#clip-panels)">
          <rect id="pcb-grid-rect" x="0" y="0" fill="url(#pcbDotGrid)" />
          <circle id="pcb-arc-c" className="pcb-arc-rail" />

          {/* TL traces */}
          <path id="pcb-line-sys" className={`pcb-trace ${data.config.enable_system_proxy ? 'pcb-trace-active-orange' : 'pcb-trace-off'}`} />
          <path id="pcb-pulse-sys" className={`pcb-pulse-orange ${data.config.enable_system_proxy ? '' : 'pulse-muted'}`} />
          <path id="pcb-line-tun" className={`pcb-trace ${data.config.enable_tun ? 'pcb-trace-active-green' : 'pcb-trace-off'}`} />
          <path id="pcb-pulse-tun" className={`pcb-pulse-green pcb-pulse-slow ${data.config.enable_tun ? '' : 'pulse-muted'}`} />
          <path id="pcb-line-chain" className={`pcb-trace ${data.config.enable_chain_proxy ? 'pcb-trace-active-orange' : 'pcb-trace-off'}`} />
          <path id="pcb-pulse-chain" className={`pcb-pulse-orange pcb-pulse-slow ${data.config.enable_chain_proxy ? '' : 'pulse-muted'}`} />
          <circle id="via-sys-o" cx="310" cy="126" r="3.4" className="pcb-via-outer" />
          <circle id="via-sys-i" cx="310" cy="126" r="1.4" className="pcb-via-accent" />
          <circle id="via-tun-o" cx="263" cy="191" r="3.4" className="pcb-via-outer" />
          <circle id="via-tun-i" cx="263" cy="191" r="1.4" className="pcb-via-green" />
          <circle id="via-chain-o" cx="256" cy="230" r="3.2" className="pcb-via-outer" />
          <circle id="via-chain-i" cx="256" cy="230" r="1.3" className="pcb-via-accent" />

          {/* TR traces */}
          <path id="pcb-line-rule" className={`pcb-trace ${outbound === 'rule' ? 'pcb-trace-active-orange' : ''}`} />
          <path id="pcb-line-global" className={`pcb-trace ${outbound === 'global' ? 'pcb-trace-active-orange' : ''}`} />
          <path id="pcb-line-direct" className={`pcb-trace ${outbound === 'direct' ? 'pcb-trace-active-orange' : ''}`} />
          <path id="pcb-pulse-rule" className={`pcb-pulse-orange pcb-pulse-fast ${outbound === 'rule' ? '' : 'pulse-muted'}`} />
          <path id="pcb-pulse-global" className={`pcb-pulse-orange pcb-pulse-fast ${outbound === 'global' ? '' : 'pulse-muted'}`} />
          <path id="pcb-pulse-direct" className={`pcb-pulse-orange pcb-pulse-fast ${outbound === 'direct' ? '' : 'pulse-muted'}`} />
          <path id="pcb-line-rot" className="pcb-trace" />
          <path id="pcb-pulse-rot" className="pcb-pulse-green pcb-pulse-slow" />
          <circle id="via-rule-o" cx="559" cy="117" r="3.4" className="pcb-via-outer" />
          <circle id="via-rule-i" cx="559" cy="117" r="1.4" className="pcb-via-accent" />
          <circle id="via-global-o" cx="584" cy="140" r="3.0" className="pcb-via-outer" />
          <circle id="via-global-i" cx="584" cy="140" r="1.2" className="pcb-via-inner" />
          <circle id="via-direct-o" cx="605" cy="167" r="3.0" className="pcb-via-outer" />
          <circle id="via-direct-i" cx="605" cy="167" r="1.2" className="pcb-via-inner" />
          <circle id="via-rot-o" cx="634" cy="260" r="3.0" className="pcb-via-outer" />
          <circle id="via-rot-i" cx="634" cy="260" r="1.2" className="pcb-via-green" />

          {/* BL traces */}
          <path id="pcb-line-up" className="pcb-trace" />
          <path id="pcb-pulse-up" className="pcb-pulse-orange" />
          <path id="pcb-line-down" className="pcb-trace" />
          <path id="pcb-pulse-down" className="pcb-pulse-green pcb-pulse-fast" />
          <path id="pcb-bl-serp" className="pcb-trace" />
          <path id="pcb-bl-serp-pulse" className="pcb-pulse-green pcb-pulse-slow" />
          <circle id="via-up-o" cx="263" cy="349" r="3.2" className="pcb-via-outer" />
          <circle id="via-up-i" cx="263" cy="349" r="1.3" className="pcb-via-accent" />
          <circle id="via-down-o" cx="283" cy="384" r="3.4" className="pcb-via-outer" />
          <circle id="via-down-i" cx="283" cy="384" r="1.4" className="pcb-via-green" />

          {/* BR traces */}
          <path id="pcb-line-egress-1" className="pcb-trace" />
          <path id="pcb-line-egress-2" className="pcb-trace-thin" />
          <path id="pcb-pulse-egress" className="pcb-pulse-green" />
          <rect id="pad-g0" x="884" y="504" width="8" height="11" rx="1" className="pcb-pad-gold" />
          <rect id="pad-g1" x="895" y="504" width="8" height="11" rx="1" className="pcb-pad-gold" />
          <rect id="pad-g2" x="906" y="504" width="8" height="11" rx="1" className="pcb-pad-gold" />
          <rect id="pad-g3" x="917" y="504" width="8" height="11" rx="1" className="pcb-pad-gold" />
          <circle id="via-eg1-o" cx="595" cy="387" r="3.4" className="pcb-via-outer" />
          <circle id="via-eg1-i" cx="595" cy="387" r="1.4" className="pcb-via-green" />
          <circle id="via-eg2-o" cx="575" cy="410" r="2.8" className="pcb-via-outer" />
          <circle id="via-eg2-i" cx="575" cy="410" r="1.1" className="pcb-via-inner" />
        </g>
      </svg>

      {/* PANEL 1 (TOP-LEFT): System Proxy & TUN Switches */}
      <div
        style={{ width: 'var(--side-w, 256px)' }}
        className="absolute top-0 left-0 h-[45.5%] p-3.5 flex flex-col justify-between"
      >
        <div className="flex items-center justify-between">
          <span className="text-[11px] font-semibold tracking-wider uppercase text-[#78766e]">接管模式</span>
          <span className="hw-readout-bay px-1.5 py-0.5 rounded text-[10px] font-mono font-semibold text-[#54524c] tabular-nums">
            :{data.config.local_proxy_port}
          </span>
        </div>

        <div className="flex-1 flex flex-col justify-center gap-2 mt-1.5">
          <div onClick={onToggleSys} className="hw-module-card relative flex items-center justify-between gap-1.5 px-2.5 py-2 rounded-md cursor-pointer">
            <div className="min-w-0">
              <div className="text-[12px] font-semibold text-[#191918] leading-tight">系统代理</div>
              <div className="text-[10px] font-mono text-[#78766e] mt-0.5 truncate">HTTP / SOCKS5</div>
            </div>
            <div className={`hw-slide-track shrink-0 ${data.config.enable_system_proxy ? 'pill-on' : ''}`}>
              <div className="hw-slide-thumb" />
            </div>
            <span id="pin-sys" className={`hw-pin absolute -right-[3.5px] top-1/2 -translate-y-1/2 ${data.config.enable_system_proxy ? 'pin-orange' : ''}`} />
          </div>

          <div onClick={onToggleTun} className="hw-module-card relative flex items-center justify-between gap-1.5 px-2.5 py-2 rounded-md cursor-pointer">
            <div className="min-w-0">
              <div className="text-[12px] font-semibold text-[#191918] leading-tight">虚拟网卡 TUN</div>
              <div className="flex items-center gap-1 text-[10px] font-mono text-[#78766e] mt-0.5">
                <span className="truncate">{data.elevated ? '全局接管 · 已提权' : '需管理员'}</span>
                {!data.elevated && (
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      onElevate();
                    }}
                    className="px-1.5 py-[0.5px] rounded-[3px] bg-[#e84c1e] hover:bg-[#d13e13] text-white text-[9.5px] font-sans font-semibold leading-tight shrink-0 cursor-pointer"
                  >
                    提权
                  </button>
                )}
              </div>
            </div>
            <div className={`hw-slide-track shrink-0 ${data.config.enable_tun ? 'pill-on' : ''}`}>
              <div className="hw-slide-thumb" />
            </div>
            <span id="pin-tun" className={`hw-pin absolute -right-[3.5px] top-1/2 -translate-y-1/2 ${data.config.enable_tun ? 'pin-green' : ''}`} />
          </div>

          <div onClick={onToggleChain} className="hw-module-card relative flex items-center justify-between gap-1.5 px-2.5 py-2 rounded-md cursor-pointer">
            <div className="min-w-0">
              <div className="text-[12px] font-semibold text-[#191918] leading-tight">链式代理</div>
              <div
                className={`text-[10px] font-mono mt-0.5 truncate ${
                  data.config.enable_chain_proxy ? 'text-[#e84c1e]' : 'text-[#78766e]'
                }`}
              >
                {data.config.chain_exit_source
                  ? data.config.chain_exit_source.kind === 'subscription_node'
                    ? `落地: ${cleanNodeName(data.config.chain_exit_source.name)}`
                    : data.config.chain_exit_source.kind === 'profile_pool'
                    ? `落地池: ${data.config.chain_exit_source.profile_name}`
                    : '自定义落地节点'
                  : '未配置落地节点'}
              </div>
            </div>
            <div className={`hw-slide-track shrink-0 ${data.config.enable_chain_proxy ? 'pill-on' : ''}`}>
              <div className="hw-slide-thumb" />
            </div>
            <span id="pin-chain" className={`hw-pin absolute -right-[3.5px] top-1/2 -translate-y-1/2 ${data.config.enable_chain_proxy ? 'pin-orange' : ''}`} />
          </div>
        </div>
      </div>

      {/* PANEL 1 UPPER WING */}
      <div
        style={{ left: 'var(--side-w, 256px)', width: 'var(--wing-tl-w, 240px)' }}
        className="absolute top-0 h-[15%] px-3.5 py-2 flex items-center justify-between gap-2"
      >
        <div className="flex flex-col">
          <span className="text-[10px] uppercase tracking-wider text-[#78766e]">内核状态</span>
          <span className="text-xs font-semibold text-[#191918] flex items-center gap-1.5 mt-0.5">
            <span
              className="w-1.5 h-1.5 rounded-full"
              style={{ backgroundColor: data.running ? '#1f8a4c' : '#8c8a82' }}
            />
            <span>{data.running ? 'Mihomo · MITM' : '已停止'}</span>
          </span>
        </div>
        <div className="text-right tabular-nums">
          <div className="text-[10px] text-[#78766e]">峰值速率</div>
          <div className="text-xs font-semibold text-[#e84c1e] mt-0.5">{formatRateStr(peakDownBps)}</div>
        </div>
      </div>

      {/* PANEL 2 UPPER WING */}
      <div
        style={{ left: 'var(--wing-tr-l, 533px)', width: 'var(--wing-tr-w, 130px)' }}
        className="absolute top-0 h-[15%] px-3 py-2 flex flex-col justify-center"
      >
        <span className="text-[10px] uppercase tracking-wider text-[#78766e]">当前策略</span>
        <span className="text-xs font-bold text-[#e84c1e] tracking-tight mt-0.5">{outbound.toUpperCase()}</span>
      </div>

      {/* PANEL 2 (TOP-RIGHT): Outbound + 429 Rotation */}
      <div
        style={{ width: 'var(--side-w, 256px)' }}
        className="absolute top-0 right-0 h-[53.5%] p-3.5 flex flex-col justify-between"
      >
        <div>
          <div className="text-[11px] font-semibold tracking-wider uppercase text-[#78766e] mb-2">出站模式</div>
          <div className="space-y-1.5 text-xs">
            {(
              [
                ['rule', '规则 (Rule)'],
                ['global', '全局 (Global)'],
                ['direct', '直连 (Direct)'],
              ] as [OutboundMode, string][]
            ).map(([m, label]) => {
              const active = outbound === m;
              return (
                <div
                  key={m}
                  onClick={() => onSetOutbound(m)}
                  className={`hw-channel-row relative flex items-center gap-2 px-2.5 py-1.5 rounded cursor-pointer ${
                    active ? 'is-active' : ''
                  }`}
                >
                  <span
                    id={`pin-${m}`}
                    className={`hw-pin absolute -left-[3.5px] top-1/2 -translate-y-1/2 ${active ? 'pin-orange' : ''}`}
                  />
                  <span
                    className="w-3 h-3 rounded-full border flex items-center justify-center"
                    style={{ borderColor: active ? '#e84c1e' : '#b4b2a8' }}
                  >
                    {active && <span className="w-1.5 h-1.5 rounded-full bg-[#e84c1e]" />}
                  </span>
                  <span className={active ? 'font-semibold text-[#191918]' : 'text-[#54524c]'}>{label}</span>
                </div>
              );
            })}
          </div>
        </div>

        <div className="pt-2.5 border-t border-[#e0ded6] relative">
          <span id="pin-rot" className="hw-pin pin-green absolute -left-[3.5px] bottom-[19px]" />
          <div className="text-[11px] text-[#78766e] mb-1.5">429 轮询策略</div>
          <div className="hw-keycap-tray p-1 rounded-md grid grid-cols-3 gap-1 text-[11px]">
            {(
              [
                ['on429', '429切'],
                ['req', '逐请求'],
                ['int', '60s'],
              ] as [RotationKey, string][]
            ).map(([k, lbl]) => (
              <button
                key={k}
                onClick={() => onSetRotation(k)}
                className={`hw-keycap py-1 rounded cursor-pointer ${rotKey === k ? 'is-active font-medium' : ''}`}
              >
                {lbl}
              </button>
            ))}
          </div>
        </div>
      </div>

      {/* PANEL 3 (BOTTOM-LEFT): Traffic Stats */}
      <div
        style={{ width: 'var(--side-w, 256px)' }}
        className="absolute bottom-0 left-0 h-[50.5%] p-3.5 flex flex-col justify-between"
      >
        <div className="space-y-2.5">
          <div className="flex items-center justify-between">
            <span className="text-[11px] font-semibold tracking-wider uppercase text-[#78766e]">流量统计</span>
            <span className="text-[10px] text-[#78766e]">本次会话</span>
          </div>

          <div className="hw-meter-trough relative w-full h-2.5 rounded-[4px] overflow-hidden flex p-[1.5px]">
            <div className="h-full bg-[#e84c1e] rounded-l-[2px] transition-all duration-500" style={{ width: `${upPct}%` }} />
            <div className="h-full flex-1 bg-[#222320] rounded-r-[2px]" />
            <div
              className="absolute inset-0 pointer-events-none"
              style={{ background: 'repeating-linear-gradient(90deg, transparent 0 11px, rgba(245,244,239,0.55) 11px 12.5px)' }}
            />
          </div>

          <div className="space-y-1.5 text-xs tabular-nums">
            <div className="hw-readout-bay relative px-2.5 py-1.5 rounded flex items-center justify-between">
              <span className="text-[#e84c1e] font-semibold text-[11.5px]">↑ 上传总量</span>
              <span className="font-mono font-bold text-[#191918]">{formatBytes(data.telemetry.total_up)}</span>
              <span id="pin-up" className="hw-pin pin-orange absolute -right-[3.5px] top-1/2 -translate-y-1/2" />
            </div>
            <div className="hw-readout-bay relative px-2.5 py-1.5 rounded flex items-center justify-between">
              <span className="text-[#1f8a4c] font-semibold text-[11.5px]">↓ 下载总量</span>
              <span className="font-mono font-bold text-[#191918]">{formatBytes(data.telemetry.total_down)}</span>
              <span id="pin-down" className="hw-pin pin-green absolute -right-[3.5px] top-1/2 -translate-y-1/2" />
            </div>
          </div>
        </div>

        <div className="hw-module-card relative px-2.5 py-1.5 rounded flex items-center justify-between text-[11px]">
          <span className="text-[#54524c] font-medium">429 透明重试</span>
          <span className="flex items-center gap-1 px-1.5 py-0.5 rounded bg-[#e4eee7] border border-[#b9d9c4] text-[#1f8a4c] font-mono text-[10px] font-semibold">
            <span className="w-1.5 h-1.5 rounded-full bg-[#1f8a4c]" />
            {data.config.auto_retry_on_429 ? 'ON' : 'OFF'}
          </span>
        </div>
      </div>

      {/* PANEL 3 LOWER WING */}
      <div
        style={{ left: 'var(--side-w, 256px)', width: 'var(--wing-bl-w, 130px)' }}
        className="absolute bottom-0 h-[15%] px-3 py-2 flex flex-col justify-center gap-1 tabular-nums"
      >
        <div className="flex items-center justify-between text-[11px]">
          <span className="text-[#78766e]">429</span>
          <span className="font-mono font-bold text-[#e84c1e]">{data.total_429}</span>
        </div>
        <div className="flex items-center justify-between text-[11px]">
          <span className="text-[#78766e]">重试</span>
          <span className="font-mono font-bold text-[#1f8a4c]">{data.total_retries_ok}</span>
        </div>
      </div>

      {/* PANEL 4 LOWER WING */}
      <div
        style={{ left: 'var(--wing-br-l, 427px)', width: 'var(--wing-br-w, 240px)' }}
        className="absolute bottom-0 h-[15%] px-3.5 py-2 flex items-center justify-between gap-2 tabular-nums"
      >
        <div>
          <div className="text-[10px] uppercase tracking-wider text-[#78766e]">内网 IP</div>
          <div className="text-xs font-mono font-semibold text-[#191918] mt-0.5">{data.telemetry.local_ip}</div>
        </div>
        <div className="text-right">
          <div className="text-[10px] text-[#78766e]">运行时长</div>
          <div className="text-xs font-mono font-bold text-[#191918] mt-0.5">
            {data.running ? formatUptime(data.telemetry.uptime_secs) : '--:--:--'}
          </div>
        </div>
      </div>

      {/* PANEL 4 (BOTTOM-RIGHT): Exit IP Readout Bay */}
      <div
        onClick={onCheckExitIp}
        style={{ width: 'var(--side-w, 256px)' }}
        className="absolute bottom-0 right-0 h-[43%] p-3.5 flex flex-col justify-between cursor-pointer group"
      >
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-1.5">
            <FlagIcon code={exitCountry.code} size="sm" />
            <span className="text-[11px] font-semibold tracking-wider uppercase text-[#78766e]">出口检测</span>
          </div>
          <span className="hw-micro-btn px-2 py-0.5 rounded text-[10.5px] font-semibold text-[#54524c]">
            {data.telemetry.checking_ip ? '...' : '刷新'}
          </span>
        </div>

        <div className="hw-readout-bay relative px-2.5 py-2 rounded-md my-1">
          <span id="pin-egress-1" className="hw-pin pin-green absolute -left-[3.5px] top-[34%]" />
          <span id="pin-egress-2" className="hw-pin absolute -left-[3.5px] top-[68%]" />
          <div className="text-[15.5px] font-mono font-bold tracking-tight text-[#191918] tabular-nums leading-snug truncate">
            {data.telemetry.checking_ip ? '检测中...' : data.telemetry.exit_ip || '点击探测出口 IP'}
          </div>
          <div className="text-[11px] font-mono text-[#626058] mt-0.5 truncate">
            {data.telemetry.exit_country_code || exitCountry.code} · {data.telemetry.local_ip}
          </div>
        </div>

        <div className="pt-1.5 border-t border-[#e0ded6] flex items-center justify-between text-[11px]">
          <span className="text-[#78766e]">点击重新探测</span>
          <span className={data.running ? 'text-[#1f8a4c] font-medium' : 'text-[#78766e] font-medium'}>
            {data.running ? '● 连通' : '○ 待机'}
          </span>
        </div>
      </div>

      {/* CENTER HERO DIAL */}
      <CircularDial
        data={data}
        dialPage={dialPage}
        launching={launching}
        launchStartMs={launchStartMs}
        stopping={stopping}
        stopStartMs={stopStartMs}
        onFlipPage={onFlipPage}
        onPowerAction={onPowerAction}
        onSelectNode={onSelectNode}
        onLaunchComplete={onLaunchComplete}
        onStopComplete={onStopComplete}
        onLaunchShowSpeed={onLaunchShowSpeed}
        onStopShowPower={onStopShowPower}
      />
    </div>
  );
};
