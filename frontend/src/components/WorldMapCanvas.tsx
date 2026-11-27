import React, { useMemo, useState } from 'react';
import {
  buildGeodesicArc,
  getFlagUrl,
  MAP_H,
  MAP_W,
  projectRegion,
  REGION_META,
  WORLD_ATLAS,
} from '../data/worldGeo';
import { cleanNodeName, type NodeDto } from '../types';

export type MapZoomMode = 'global' | 'apac' | 'eu' | 'custom';

export interface RegionStats {
  code: string;
  name: string;
  total: number;
  ready: number;
  cooling: number;
  bestLatency: number;
  activeNode: NodeDto | null;
  exitNode: NodeDto | null;
}

interface WorldMapCanvasProps {
  groupedNodes: Record<string, NodeDto[]>;
  activeRegion: string | null;
  exitRegion: string | null;
  chainExitName: string | null;
  selectedRegion: string;
  mapZoom: MapZoomMode;
  showFlags: boolean;
  onSelectRegion: (region: string) => void;
  onOceanClick: () => void;
}

export const WorldMapCanvas: React.FC<WorldMapCanvasProps> = ({
  groupedNodes,
  activeRegion,
  exitRegion,
  chainExitName,
  selectedRegion,
  mapZoom,
  showFlags,
  onSelectRegion,
  onOceanClick,
}) => {
  const [tooltip, setTooltip] = useState<{
    stats: RegionStats;
    x: number;
    y: number;
  } | null>(null);

  const regionStatsMap = useMemo(() => {
    const out: Record<string, RegionStats> = {};
    for (const [code, list] of Object.entries(groupedNodes)) {
      if (!list.length) continue;
      const ready = list.filter((n) => n.cooldown_remaining_secs === 0 && (n.latency_ms ?? 1) > 0).length;
      const cooling = list.filter((n) => n.cooldown_remaining_secs > 0).length;
      const validLats = list.map((n) => n.latency_ms).filter((v): v is number => v != null && v > 0);
      const bestLatency = validLats.length > 0 ? Math.min(...validLats) : 9999;
      const activeNode = list.find((n) => n.is_active) ?? null;
      const exitNode = chainExitName ? list.find((n) => n.name === chainExitName) ?? null : null;
      out[code] = {
        code,
        name: REGION_META[code]?.name ?? code,
        total: list.length,
        ready,
        cooling,
        bestLatency,
        activeNode,
        exitNode,
      };
    }
    return out;
  }, [groupedNodes, chainExitName]);

  // Regions that actually have nodes and valid SVG flag URLs
  const activeFlagCodes = useMemo(() => {
    return Object.keys(regionStatsMap).filter((code) => Boolean(getFlagUrl(code)));
  }, [regionStatsMap]);

  // Camera transform calculation
  const camera = useMemo(() => {
    if (mapZoom === 'apac') return { tx: -355, ty: -210, scale: 2.05 };
    if (mapZoom === 'eu') return { tx: -35, ty: -65, scale: 1.22 };
    if (mapZoom === 'custom' && selectedRegion !== 'ALL') {
      const pt = projectRegion(selectedRegion);
      if (pt) {
        const scale = selectedRegion === 'US' || selectedRegion === 'AU' || selectedRegion === 'RU' || selectedRegion === 'CA' ? 1.95 : 2.75;
        return {
          tx: 324 - pt[0] * scale,
          ty: 306 - pt[1] * scale,
          scale,
        };
      }
    }
    return { tx: 0, ty: 0, scale: 1 };
  }, [mapZoom, selectedRegion]);

  const invScale = 1 / Math.pow(camera.scale, 0.72);

  const geodesicArcD = useMemo(() => {
    if (!activeRegion || !exitRegion || activeRegion === exitRegion) return '';
    return buildGeodesicArc(activeRegion, exitRegion);
  }, [activeRegion, exitRegion]);

  const handleBeaconMove = (stats: RegionStats, e: React.MouseEvent<SVGGElement>) => {
    const stage = e.currentTarget.closest('.map-stage');
    if (!stage) return;
    const rect = stage.getBoundingClientRect();
    let x = e.clientX - rect.left + 16;
    let y = e.clientY - rect.top - 12;
    if (x + 190 > rect.width) x = e.clientX - rect.left - 196;
    if (y + 110 > rect.height) y = rect.height - 116;
    if (y < 48) y = 48;
    setTooltip({ stats, x, y });
  };

  return (
    <div className="relative w-full h-full">
      <svg
        viewBox={`0 0 ${MAP_W} ${MAP_H}`}
        className="w-full h-full block"
        preserveAspectRatio="xMidYMid slice"
      >
        <defs>
          {/* Glazed Ceramic Land Gradients (Lit from Top-Left 315°) */}
          <linearGradient id="ceramic-land-base" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#f7f6f0" />
            <stop offset="55%" stopColor="#efede4" />
            <stop offset="100%" stopColor="#e5e2d6" />
          </linearGradient>
          <linearGradient id="ceramic-land-nodes" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#ffffff" />
            <stop offset="52%" stopColor="#f7f5ee" />
            <stop offset="100%" stopColor="#ece9dd" />
          </linearGradient>
          <linearGradient id="ceramic-land-hover" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#fff8f4" />
            <stop offset="100%" stopColor="#f7e4da" />
          </linearGradient>
          <linearGradient id="ceramic-land-active" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#ffede4" />
            <stop offset="55%" stopColor="#fad9ca" />
            <stop offset="100%" stopColor="#f0c3ae" />
          </linearGradient>
          <linearGradient id="ceramic-land-exit" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#fdf4e3" />
            <stop offset="100%" stopColor="#f1dec0" />
          </linearGradient>

          {/* Subtle Ceramic Glaze Sheen over Flag Inlays */}
          <linearGradient id="ceramic-flag-glaze" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#ffffff" stopOpacity="0.24" />
            <stop offset="50%" stopColor="#f5f3ea" stopOpacity="0.04" />
            <stop offset="100%" stopColor="#28261f" stopOpacity="0.14" />
          </linearGradient>

          {/* Dynamic Country Flag Patterns */}
          {activeFlagCodes.map((code) => {
            const url = getFlagUrl(code);
            if (!url) return null;
            if (code === 'US') {
              return (
                <pattern
                  key={code}
                  id="flag-pat-US"
                  patternUnits="userSpaceOnUse"
                  x="716"
                  y="200"
                  width="170"
                  height="112"
                >
                  <rect width="170" height="112" fill="#f5f4ee" />
                  <image href={url} x="0" y="0" width="170" height="112" preserveAspectRatio="xMidYMid slice" />
                </pattern>
              );
            }
            if (code === 'JP') {
              return (
                <pattern
                  key={code}
                  id="flag-pat-JP"
                  patternUnits="objectBoundingBox"
                  patternContentUnits="objectBoundingBox"
                  width="1"
                  height="1"
                >
                  <rect width="1" height="1" fill="#ffffff" />
                  <image href={url} x="0" y="0.08" width="1" height="0.84" preserveAspectRatio="xMidYMid meet" />
                </pattern>
              );
            }
            return (
              <pattern
                key={code}
                id={`flag-pat-${code}`}
                patternUnits="objectBoundingBox"
                patternContentUnits="objectBoundingBox"
                width="1"
                height="1"
              >
                <image href={url} x="0" y="0" width="1" height="1" preserveAspectRatio="xMidYMid slice" />
              </pattern>
            );
          })}

          {/* 3D Cast Shadow for Raised Ceramic Continents */}
          <filter id="continent-cast-shadow" x="-6%" y="-6%" width="114%" height="116%">
            <feDropShadow dx="2.6" dy="4.2" stdDeviation="2.8" floodColor="#28261f" floodOpacity="0.24" />
            <feDropShadow dx="0.8" dy="1.4" stdDeviation="0.6" floodColor="#28261f" floodOpacity="0.22" />
          </filter>

          {/* Soft Shadow for Markers & Pills */}
          <filter id="keycap-shadow" x="-25%" y="-25%" width="150%" height="160%">
            <feDropShadow dx="0.8" dy="1.8" stdDeviation="1.2" floodColor="#191918" floodOpacity="0.22" />
          </filter>

          {/* Floating Shadow for Geodesic Chain Arc */}
          <filter id="arc-float-shadow" x="-15%" y="-15%" width="130%" height="145%">
            <feDropShadow dx="0" dy="5.5" stdDeviation="2.2" floodColor="#3a190d" floodOpacity="0.22" />
          </filter>
        </defs>

        <g
          id="map-camera"
          style={{
            transform: `translate(${camera.tx.toFixed(1)}px, ${camera.ty.toFixed(1)}px) scale(${camera.scale.toFixed(3)})`,
          }}
        >
          {/* Ocean click target */}
          <rect
            x="-400"
            y="-400"
            width="1800"
            height="1400"
            fill="transparent"
            onClick={onOceanClick}
          />

          {/* Debossed Engraved Ocean Graticule */}
          <g pointerEvents="none">
            <path d={WORLD_ATLAS.graticuleD} fill="none" stroke="rgba(95, 91, 78, 0.22)" strokeWidth="0.65" />
            <path
              d={WORLD_ATLAS.graticuleD}
              fill="none"
              stroke="rgba(255, 255, 255, 0.42)"
              strokeWidth="0.6"
              transform="translate(0.5, 0.6)"
            />
            <path
              d={WORLD_ATLAS.equatorD}
              fill="none"
              stroke="rgba(95, 91, 78, 0.34)"
              strokeWidth="0.85"
              strokeDasharray="4 3"
            />
          </g>

          {/* Continuous 3D Extruded Ceramic Continent Slab */}
          <g pointerEvents="none">
            <path d={WORLD_ATLAS.landD} fill="#b5b0a0" filter="url(#continent-cast-shadow)" />
            <path d={WORLD_ATLAS.landD} fill="#a8a393" transform="translate(1.4, 2.0)" />
            <path d={WORLD_ATLAS.landD} fill="#c1bcb0" transform="translate(0.7, 1.0)" />
            <path d={WORLD_ATLAS.landD} fill="#ffffff" transform="translate(-0.7, -0.7)" opacity="0.92" />
          </g>

          {/* Individual Glazed Ceramic Country Plates + Optional Flag Inlays */}
          <g>
            {WORLD_ATLAS.countries.map((c) => {
              const reg = c.code;
              const hasNodes = Boolean(reg && regionStatsMap[reg]);
              const isSelected = Boolean(reg && selectedRegion === reg);
              const isActiveReg = Boolean(reg && activeRegion === reg);
              const isExitReg = Boolean(reg && exitRegion === reg);

              let fill = 'url(#ceramic-land-base)';
              if (isSelected || isActiveReg) fill = 'url(#ceramic-land-active)';
              else if (isExitReg) fill = 'url(#ceramic-land-exit)';
              else if (hasNodes) fill = 'url(#ceramic-land-nodes)';

              const stroke = isSelected || isActiveReg ? '#e84c1e' : 'none';
              const strokeW = isSelected || isActiveReg ? 0.95 : 0;
              const hasFlagOverlay = showFlags && hasNodes && reg && Boolean(getFlagUrl(reg));
              const flagOpacity = isSelected || isActiveReg ? 0.88 : isExitReg ? 0.8 : 0.68;

              return (
                <React.Fragment key={c.id}>
                  <path
                    d={c.d}
                    fill={fill}
                    stroke={stroke}
                    strokeWidth={strokeW}
                    strokeLinejoin="round"
                    className={`country-plate ${hasNodes ? 'has-nodes cursor-pointer' : ''}`}
                    onClick={(e) => {
                      e.stopPropagation();
                      if (hasNodes && reg) onSelectRegion(reg);
                      else onOceanClick();
                    }}
                  />
                  {hasFlagOverlay && reg && (
                    <>
                      <path
                        d={c.d}
                        fill={`url(#flag-pat-${reg})`}
                        opacity={flagOpacity}
                        stroke={stroke}
                        strokeWidth={strokeW}
                        strokeLinejoin="round"
                        className="country-flag-layer cursor-pointer"
                        onClick={(e) => {
                          e.stopPropagation();
                          onSelectRegion(reg);
                        }}
                      />
                      <path d={c.d} fill="url(#ceramic-flag-glaze)" pointerEvents="none" />
                    </>
                  )}
                </React.Fragment>
              );
            })}

            {/* Crisp CNC-Engraved V-Groove Country Borders */}
            <path
              d={WORLD_ATLAS.bordersD}
              fill="none"
              stroke="rgba(128, 123, 108, 0.45)"
              strokeWidth="0.55"
              strokeLinejoin="round"
              pointerEvents="none"
            />
            <path
              d={WORLD_ATLAS.bordersD}
              fill="none"
              stroke="rgba(255, 255, 255, 0.72)"
              strokeWidth="0.45"
              strokeLinejoin="round"
              transform="translate(0.45, 0.45)"
              pointerEvents="none"
            />
            <path
              d={WORLD_ATLAS.landD}
              fill="none"
              stroke="rgba(130, 125, 110, 0.35)"
              strokeWidth="0.5"
              pointerEvents="none"
            />
          </g>

          {/* Elevated Geodesic Chain Proxy Arc */}
          {geodesicArcD && (
            <g filter="url(#arc-float-shadow)" pointerEvents="none">
              <path
                d={geodesicArcD}
                fill="none"
                stroke="rgba(232,76,30,0.2)"
                strokeWidth="4.2"
                strokeLinecap="round"
              />
              <path
                d={geodesicArcD}
                fill="none"
                stroke="#e84c1e"
                strokeWidth="1.85"
                strokeDasharray="5 4"
                className="chain-arc-flow"
                strokeLinecap="round"
              />
              <circle r="3.2" fill="#ff6a3d" stroke="#ffffff" strokeWidth="0.8">
                <animateMotion dur="2.6s" repeatCount="indefinite" path={geodesicArcD} />
              </circle>
            </g>
          )}

          {/* Minimalist Braun Matte Precision Beacons + Label Pills */}
          <g>
            {Object.values(regionStatsMap).map((stats) => {
              const pt = projectRegion(stats.code);
              if (!pt) return null;
              const isSelected = selectedRegion === stats.code;
              const activeIn = Boolean(stats.activeNode);
              const exitIn = Boolean(stats.exitNode);

              const coreColor = activeIn
                ? '#e84c1e'
                : stats.cooling > 0 && stats.ready === 0
                ? '#d98a1c'
                : '#1f8a4c';
              const haloColor = activeIn
                ? 'rgba(232, 76, 30, 0.16)'
                : stats.cooling > 0 && stats.ready === 0
                ? 'rgba(217, 138, 28, 0.14)'
                : 'rgba(31, 138, 76, 0.13)';

              const tagOffset = REGION_META[stats.code]?.tagOffset ?? { dx: 12, dy: -8 };
              const flagUrl = getFlagUrl(stats.code);
              const hasPillFlag = showFlags && Boolean(flagUrl);
              const tagText = `${exitIn ? '★ ' : ''}${stats.code} · ${stats.total}`;
              const tagW = (exitIn ? 56 : 46) + (hasPillFlag ? 16 : 0);
              const activeTagFill = isSelected
                ? '#1b1c19'
                : activeIn
                ? '#fff7f3'
                : 'rgba(252, 251, 247, 0.94)';
              const activeTagStroke = isSelected ? '#e84c1e' : activeIn ? '#e84c1e' : '#c6c2b6';
              const activeTagText = isSelected ? '#ffffff' : activeIn ? '#c93a10' : '#23221f';

              return (
                <g
                  key={stats.code}
                  className="map-beacon"
                  transform={`translate(${pt[0]}, ${pt[1]})`}
                  onClick={(e) => {
                    e.stopPropagation();
                    onSelectRegion(stats.code);
                  }}
                  onMouseEnter={(e) => handleBeaconMove(stats, e)}
                  onMouseMove={(e) => handleBeaconMove(stats, e)}
                  onMouseLeave={() => setTooltip(null)}
                >
                  <g className="beacon-inner" style={{ transform: `scale(${invScale.toFixed(3)})` }}>
                    {activeIn && (
                      <circle cx="0" cy="0" r="8" fill="none" stroke="#e84c1e" className="beacon-pulse" />
                    )}
                    {isSelected && (
                      <circle
                        cx="0"
                        cy="0"
                        r="11.5"
                        fill="none"
                        stroke="#191918"
                        strokeWidth="1"
                        strokeDasharray="2 2"
                        opacity="0.7"
                      />
                    )}
                    {exitIn && (
                      <circle
                        cx="0"
                        cy="0"
                        r="9.5"
                        fill="none"
                        stroke="#e84c1e"
                        strokeWidth="1.2"
                        strokeDasharray="2.5 2"
                      />
                    )}

                    {/* Minimalist Braun Matte Precision Marker */}
                    <g className="beacon-cap">
                      <circle cx="0" cy="0" r={activeIn ? 8.5 : 7} fill={haloColor} />
                      <circle
                        cx="0"
                        cy="0"
                        r={activeIn ? 5.2 : 4.4}
                        fill="#fcfbf8"
                        stroke={activeIn ? '#e84c1e' : 'rgba(25, 25, 24, 0.22)'}
                        strokeWidth={activeIn ? 1.2 : 0.9}
                        filter="url(#keycap-shadow)"
                      />
                      <circle cx="0" cy="0" r={activeIn ? 2.6 : 2.2} fill={coreColor} />
                    </g>

                    {/* Clean Minimalist Label Pill */}
                    <g
                      className="beacon-tag"
                      transform={`translate(${tagOffset.dx}, ${tagOffset.dy})`}
                      filter="url(#keycap-shadow)"
                    >
                      <rect
                        className="beacon-tag-bg"
                        x="0"
                        y="-8.5"
                        width={tagW}
                        height="17"
                        rx="4.5"
                        fill={activeTagFill}
                        stroke={activeTagStroke}
                        strokeWidth="0.9"
                      />
                      {hasPillFlag && flagUrl && (
                        <image
                          href={flagUrl}
                          x="4.5"
                          y="-4.5"
                          width="12"
                          height="9"
                          preserveAspectRatio="xMidYMid slice"
                          style={{ clipPath: 'inset(0 round 1.5px)' }}
                        />
                      )}
                      <text
                        className="beacon-tag-text"
                        x={hasPillFlag ? 18 + (tagW - 20) / 2 : tagW / 2}
                        y="3.1"
                        textAnchor="middle"
                        fontFamily="ui-monospace, SFMono-Regular, monospace"
                        fontSize="9.5"
                        fontWeight="700"
                        fill={activeTagText}
                      >
                        {tagText}
                      </text>
                    </g>
                  </g>
                </g>
              );
            })}
          </g>
        </g>
      </svg>

      {/* Floating Dark Braun Tooltip */}
      {tooltip && (
        <div
          id="map-hud-tooltip"
          className="absolute z-40 rounded-[7px] px-2.5 py-2 text-[11px] font-mono min-w-[164px] pointer-events-none"
          style={{ left: `${tooltip.x}px`, top: `${tooltip.y}px` }}
        >
          <div className="flex items-center justify-between gap-3 mb-1">
            <span className="font-bold text-white">
              {tooltip.stats.name} ({tooltip.stats.code})
            </span>
            <span className="text-[#5bd68a] tabular-nums">
              {tooltip.stats.bestLatency < 9999 ? `${tooltip.stats.bestLatency} ms` : '--'}
            </span>
          </div>
          <div className="text-[10px] text-[#a8a69e] flex items-center gap-2 tabular-nums">
            <span>可用 {tooltip.stats.ready}/{tooltip.stats.total}</span>
            {tooltip.stats.cooling > 0 && (
              <span className="text-[#ff9c5a]">冷却 {tooltip.stats.cooling}</span>
            )}
          </div>
          {tooltip.stats.activeNode && (
            <div className="mt-1 text-[10px] text-[#ff794e] font-semibold truncate max-w-[190px]">
              ● 当前入口: {cleanNodeName(tooltip.stats.activeNode.name)}
            </div>
          )}
          {tooltip.stats.exitNode && (
            <div className="mt-0.5 text-[10px] text-[#f5b942] font-semibold truncate max-w-[190px]">
              ★ 链式落地: {cleanNodeName(tooltip.stats.exitNode.name)}
            </div>
          )}
        </div>
      )}
    </div>
  );
};
