import React, { useMemo, useState } from 'react';
import { Activity, ChevronDown, List, RotateCw, Search } from 'lucide-react';
import { FlagIcon } from '../components/FlagIcon';
import { WorldMapCanvas, type MapZoomMode } from '../components/WorldMapCanvas';
import { REGION_META } from '../data/worldGeo';
import { cleanNodeName, detectCountry, type FullStateDto, type NodeDto } from '../types';

interface NodesPageProps {
  data: FullStateDto;
  onSelectNode: (name: string) => void;
  onPingAll: () => void;
  onRotateNext: () => void;
  onToggleChainExit: (name: string | null) => void;
}

const SignalBars: React.FC<{ ms: number | null }> = ({ ms }) => {
  const val = ms ?? -1;
  const level = val <= 0 ? 0 : val < 60 ? 4 : val < 110 ? 3 : val < 170 ? 2 : 1;
  const activeCol = val === 0 ? '#e84c1e' : val < 110 ? '#1f8a4c' : '#8c8a82';
  const heights = [4, 7, 10, 13];
  return (
    <span className="inline-flex items-end gap-[1.5px] h-[13px]">
      {heights.map((h, i) => (
        <span
          key={h}
          className="w-[2.5px] rounded-[1px]"
          style={{ height: `${h}px`, background: i < level ? activeCol : '#d6d4cb' }}
        />
      ))}
    </span>
  );
};

export const NodesPage: React.FC<NodesPageProps> = ({
  data,
  onSelectNode,
  onPingAll,
  onRotateNext,
  onToggleChainExit,
}) => {
  const [selectedRegion, setSelectedRegion] = useState<string>('ALL');
  const [mapZoom, setMapZoom] = useState<MapZoomMode>('global');
  const [drawerOpen, setDrawerOpen] = useState<boolean>(false);
  const [showFlags, setShowFlags] = useState<boolean>(true);
  const [search, setSearch] = useState<string>('');
  const [sortLatency, setSortLatency] = useState<boolean>(false);

  const chainExitName =
    data.config.chain_exit_source?.kind === 'subscription_node'
      ? data.config.chain_exit_source.name
      : null;

  const groupedNodes = useMemo(() => {
    const groups: Record<string, NodeDto[]> = {};
    for (const n of data.nodes) {
      const code = detectCountry(n.name).code;
      if (!groups[code]) groups[code] = [];
      groups[code].push(n);
    }
    return groups;
  }, [data.nodes]);

  const activeNode = useMemo(
    () => data.nodes.find((n) => n.is_active) ?? null,
    [data.nodes]
  );

  const exitNode = useMemo(
    () => (chainExitName ? data.nodes.find((n) => n.name === chainExitName) ?? null : null),
    [data.nodes, chainExitName]
  );

  const activeRegion = activeNode ? detectCountry(activeNode.name).code : null;
  const exitRegion = exitNode ? detectCountry(exitNode.name).code : null;
  const glCount = groupedNodes['GL']?.length ?? 0;

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    let list = data.nodes.filter((n) => {
      const c = detectCountry(n.name);
      if (selectedRegion !== 'ALL' && c.code !== selectedRegion) {
        return false;
      }
      if (q) {
        const hay = `${c.code} ${n.name} ${n.proxy_type} ${n.server}`.toLowerCase();
        if (!hay.includes(q)) return false;
      }
      return true;
    });
    if (sortLatency) {
      list = [...list].sort((a, b) => {
        const ma = a.latency_ms && a.latency_ms > 0 ? a.latency_ms : 99999;
        const mb = b.latency_ms && b.latency_ms > 0 ? b.latency_ms : 99999;
        return ma - mb;
      });
    }
    return list;
  }, [data.nodes, selectedRegion, search, sortLatency]);

  const handleSelectRegion = (reg: string) => {
    setSelectedRegion(reg);
    setDrawerOpen(true);
    if (reg === 'ALL' || reg === 'GL') {
      setMapZoom('global');
    } else {
      setMapZoom('custom');
    }
  };

  const handleToggleDrawer = (force?: boolean) => {
    setDrawerOpen((prev) => {
      const next = typeof force === 'boolean' ? force : !prev;
      if (!next && mapZoom === 'custom') {
        setSelectedRegion('ALL');
        setMapZoom('global');
      }
      return next;
    });
  };

  const handleOceanClick = () => {
    setSelectedRegion('ALL');
    setDrawerOpen(false);
    setMapZoom('global');
  };

  const handleSetZoom = (z: MapZoomMode) => {
    setMapZoom(z);
    if (z === 'global') {
      setSelectedRegion('ALL');
    }
  };

  const latestLog = data.logs[data.logs.length - 1];
  const regionTitle =
    selectedRegion === 'ALL'
      ? '全部地区节点'
      : `${REGION_META[selectedRegion]?.name ?? selectedRegion}节点`;

  return (
    <div className="map-stage w-full h-full relative overflow-hidden">
      {/* Full-Bleed Ceramic Relief World Map */}
      <WorldMapCanvas
        groupedNodes={groupedNodes}
        activeRegion={activeRegion}
        exitRegion={exitRegion}
        chainExitName={chainExitName}
        selectedRegion={selectedRegion}
        mapZoom={mapZoom}
        showFlags={showFlags}
        onSelectRegion={handleSelectRegion}
        onOceanClick={handleOceanClick}
      />

      {/* FLOATING HUD 1: Top-Left Status + Zoom & Flag Controls */}
      <div className="absolute top-3.5 left-3.5 z-20 flex items-center gap-2 pointer-events-none">
        <div className="hud-glass rounded-[9px] h-[36px] px-3 flex items-center gap-2.5 pointer-events-auto">
          <div className="flex items-baseline gap-1.5">
            <span className="text-[13px] font-bold tracking-tight text-[#191918]">节点地图</span>
            <span className="text-[9px] font-mono font-semibold tracking-[0.12em] uppercase text-[#8a877d]">
              ATLAS
            </span>
          </div>
          <span className="w-[1px] h-3.5 bg-[#c9c5b8]" />
          <div className="flex items-center gap-2 text-[11px] font-mono tabular-nums">
            <span className="flex items-center gap-1 text-[#1f8a4c] font-semibold">
              <span className="w-1.5 h-1.5 rounded-full bg-[#1f8a4c]" />
              <span>{data.ready_count}</span>
            </span>
            <span className="flex items-center gap-1 text-[#c97a16] font-semibold">
              <span className="w-1.5 h-1.5 rounded-full bg-[#d98a1c]" />
              <span>{data.cooldown_count}</span>
            </span>
            <span className="text-[#78766e]">/ {data.nodes.length}</span>
          </div>
        </div>

        <div className="hud-glass rounded-[9px] h-[36px] p-1 flex items-center gap-0.5 text-[10.5px] font-mono pointer-events-auto">
          <button
            onClick={() => handleSetZoom('global')}
            className={`hw-keycap h-full px-2.5 rounded-[6px] cursor-pointer font-semibold ${
              mapZoom === 'global' ? 'is-active' : ''
            }`}
          >
            全球
          </button>
          <button
            onClick={() => handleSetZoom('apac')}
            className={`hw-keycap h-full px-2.5 rounded-[6px] cursor-pointer font-semibold ${
              mapZoom === 'apac' ? 'is-active' : ''
            }`}
          >
            亚太
          </button>
          <button
            onClick={() => handleSetZoom('eu')}
            className={`hw-keycap h-full px-2.5 rounded-[6px] cursor-pointer font-semibold ${
              mapZoom === 'eu' ? 'is-active' : ''
            }`}
          >
            欧美
          </button>
          <span className="w-[1px] h-3.5 bg-[#c9c5b8] mx-1" />
          <button
            onClick={() => setShowFlags((s) => !s)}
            className={`hw-keycap h-full px-2.5 rounded-[6px] cursor-pointer font-semibold ${
              showFlags ? 'is-active' : ''
            }`}
            title="切换国家国旗覆盖层"
          >
            国旗
          </button>
        </div>
      </div>

      {/* FLOATING HUD 2: Unified Morphing Top-Right Action Card -> Dropdown Node List */}
      <div
        className={`hud-glass morph-dock absolute top-3.5 right-3.5 z-30 flex flex-col overflow-hidden ${
          drawerOpen ? 'is-open' : ''
        }`}
      >
        {/* Persistent Top Action Bar (Morph Anchor) */}
        <div className="h-[36px] p-1 flex items-center gap-1.5 shrink-0">
          <button
            onClick={onPingAll}
            className="hw-micro-btn h-full px-2.5 rounded-[6px] flex items-center gap-1.5 text-[11.5px] font-semibold text-[#191918] cursor-pointer whitespace-nowrap"
            style={data.testing_latency ? { borderColor: '#e84c1e', color: '#e84c1e' } : undefined}
          >
            {data.testing_latency ? (
              <span className="flex items-center gap-[2px]">
                <span className="ping-bar" style={{ animationDelay: '0ms' }} />
                <span className="ping-bar" style={{ animationDelay: '140ms' }} />
                <span className="ping-bar" style={{ animationDelay: '280ms' }} />
              </span>
            ) : (
              <Activity size={13} />
            )}
            <span>{data.testing_latency ? '测速中' : '测速'}</span>
          </button>

          <button
            onClick={onRotateNext}
            className="hw-btn-primary h-full px-3 rounded-[6px] flex items-center gap-1.5 text-[11.5px] font-semibold cursor-pointer whitespace-nowrap"
          >
            <RotateCw size={12.5} />
            <span>轮换</span>
          </button>

          <span className="w-[1px] h-4 bg-[#c9c5b8] mx-0.5 shrink-0" />

          <button
            onClick={() => handleToggleDrawer()}
            className={`hw-keycap h-full ml-auto px-2.5 rounded-[6px] flex items-center gap-1.5 text-[11.5px] font-semibold cursor-pointer whitespace-nowrap ${
              drawerOpen ? 'is-active' : 'text-[#191918]'
            }`}
          >
            <List size={13} />
            <span>{drawerOpen ? '收起列表' : '全部列表'}</span>
            <ChevronDown size={11.5} className="morph-chevron opacity-75" />
          </button>
        </div>

        {/* Expanded Dropdown Body */}
        <div className="morph-body flex-1 min-h-0 flex flex-col border-t border-[#d5d2c8]">
          <div className="px-3.5 py-2.5 bg-[#eceae2]/90 border-b border-[#d5d2c8] flex flex-col gap-2 shrink-0">
            <div className="flex items-center justify-between gap-2">
              <div className="flex items-center gap-2 min-w-0">
                <span
                  className="px-1.5 py-0.5 rounded font-mono text-[10px] font-bold text-[#f7f6f1]"
                  style={{ background: selectedRegion === 'ALL' ? '#1b1c19' : '#e84c1e' }}
                >
                  {selectedRegion}
                </span>
                <span className="text-[13px] font-bold text-[#191918] truncate">{regionTitle}</span>
                <span className="text-[11px] font-mono text-[#78766e] tabular-nums">
                  ({filtered.length})
                </span>
              </div>

              {selectedRegion !== 'ALL' && (
                <button
                  onClick={() => handleSelectRegion('ALL')}
                  className="hw-micro-btn px-2 py-0.5 rounded-[4px] text-[10px] font-mono font-semibold text-[#54524c] cursor-pointer shrink-0"
                >
                  全部
                </button>
              )}
            </div>

            <div className="flex items-center gap-1.5">
              <div className="hw-readout-bay h-[27px] flex-1 px-2 rounded-[6px] flex items-center gap-1.5">
                <Search size={11} className="text-[#78766e] shrink-0" />
                <input
                  type="text"
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  placeholder="筛选节点 / 协议..."
                  className="w-full bg-transparent text-[11px] text-[#191918] placeholder-[#8c8a82] outline-none font-mono"
                />
              </div>
              <button
                onClick={() => setSortLatency((s) => !s)}
                className="hw-micro-btn h-[27px] px-2 rounded-[6px] text-[10.5px] font-mono font-semibold cursor-pointer whitespace-nowrap"
                style={{ color: sortLatency ? '#e84c1e' : '#54524c' }}
              >
                {sortLatency ? '延迟↑ ●' : '延迟↑'}
              </button>
            </div>
          </div>

          <div className="nodes-scroll flex-1 min-h-0 overflow-y-auto">
            {filtered.length === 0 ? (
              <div className="h-full flex items-center justify-center text-xs font-mono text-[#8c8a82]">
                没有匹配的节点
              </div>
            ) : (
              filtered.map((n) => {
                const c = detectCountry(n.name);
                const isExit = chainExitName === n.name;
                const dotColor = n.is_active
                  ? '#e84c1e'
                  : n.cooldown_remaining_secs > 0
                    ? '#d98a1c'
                    : n.latency_ms === 0
                      ? '#e84c1e'
                      : '#1f8a4c';
                const statusText =
                  n.cooldown_remaining_secs > 0
                    ? `冷却 ${n.cooldown_remaining_secs}s`
                    : n.is_active
                      ? '使用中'
                      : `429: ${n.hit_429_count}`;
                const statusColor =
                  n.cooldown_remaining_secs > 0
                    ? 'text-[#e84c1e] font-semibold'
                    : n.is_active
                      ? 'text-[#1f8a4c] font-semibold'
                      : 'text-[#8c8a82]';
                const latText =
                  n.latency_ms == null ? '--' : n.latency_ms === 0 ? '超时' : `${n.latency_ms} ms`;
                const latColor =
                  n.latency_ms === 0
                    ? 'text-[#e84c1e] font-semibold'
                    : n.latency_ms != null && n.latency_ms < 120
                    ? 'text-[#1f8a4c] font-semibold'
                    : 'text-[#54524c]';

                return (
                  <div
                    key={n.name}
                    onClick={() => onSelectNode(n.name)}
                    className={`node-list-row px-3.5 py-2.5 flex items-center justify-between gap-2 cursor-pointer ${
                      n.is_active ? 'is-active' : ''
                    }`}
                  >
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-1.5">
                        <span
                          className="w-[6.5px] h-[6.5px] rounded-full shrink-0"
                          style={{ background: dotColor }}
                        />
                        <FlagIcon code={c.code} size="xs" />
                        <span className="font-mono text-[10px] font-bold text-[#78766e] shrink-0">
                          {c.code}
                        </span>
                        <span
                          className={`text-[12px] truncate ${
                            n.is_active ? 'font-bold text-[#191918]' : 'font-medium text-[#2c2b28]'
                          }`}
                        >
                          {cleanNodeName(n.name)}
                        </span>
                      </div>
                      <div className="flex items-center gap-2 mt-1 pl-3 text-[10px] font-mono">
                        <span className="px-1.5 py-[0.5px] rounded bg-[#e6e4dc] text-[#626058] text-[9px] font-semibold">
                          {n.proxy_type}
                        </span>
                        <span className={statusColor}>{statusText}</span>
                      </div>
                    </div>

                    <div className="flex flex-col items-end gap-1 shrink-0 tabular-nums">
                      <div className="flex items-center gap-1.5">
                        <span className={`font-mono text-[11px] ${latColor}`}>{latText}</span>
                        <SignalBars ms={n.latency_ms} />
                      </div>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          onToggleChainExit(isExit ? null : n.name);
                        }}
                        className={`${
                          isExit ? 'hw-btn-primary' : 'hw-micro-btn text-[#54524c]'
                        } px-1.5 py-[1px] rounded-[4px] text-[9.5px] font-semibold cursor-pointer whitespace-nowrap`}
                      >
                        {isExit ? '★ 落地' : '设落地'}
                      </button>
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </div>
      </div>

      {/* FLOATING HUD 3: Bottom-Left Active Route Card + Legend */}
      <div className="absolute bottom-3.5 left-3.5 z-20 flex items-end gap-2 pointer-events-none">
        <div className="hud-glass rounded-[10px] px-3 py-2 flex flex-col gap-1 pointer-events-auto min-w-[270px] max-w-[380px]">
          <div className="flex items-center justify-between gap-4">
            <div className="flex items-center gap-1.5 text-[11.5px] font-mono font-bold text-[#191918] min-w-0">
              <span className="w-2 h-2 rounded-full bg-[#e84c1e] shadow-[0_0_5px_rgba(232,76,30,0.5)] shrink-0" />
              <span className="truncate">
                {activeNode ? cleanNodeName(activeNode.name) : '未选择节点'}
              </span>
              {exitNode ? (
                <>
                  <span className="text-[#e84c1e] font-normal shrink-0">──▶</span>
                  <span className="text-[#e84c1e] truncate">★ {cleanNodeName(exitNode.name)}</span>
                </>
              ) : (
                <span className="text-[#78766e] font-normal shrink-0">· 直出</span>
              )}
            </div>
            <span className="text-[11px] font-mono font-bold text-[#1f8a4c] tabular-nums shrink-0">
              {activeNode?.latency_ms == null
                ? '--'
                : activeNode.latency_ms === 0
                ? '超时'
                : `${activeNode.latency_ms} ms`}
            </span>
          </div>
          <div className="flex items-center justify-between gap-3 text-[10px] font-mono text-[#78766e]">
            <span className="truncate max-w-[330px]">
              {latestLog ? `[${latestLog.timestamp}] ${latestLog.message}` : '点击任意国家或标点聚焦该地区节点'}
            </span>
          </div>
        </div>

        <div className="hud-glass rounded-[8px] h-[30px] px-2.5 flex items-center gap-2.5 text-[10px] font-mono text-[#54524c] pointer-events-auto">
          <span className="flex items-center gap-1">
            <span className="w-1.5 h-1.5 rounded-full bg-[#e84c1e]" />
            使用中
          </span>
          <span className="flex items-center gap-1">
            <span className="w-1.5 h-1.5 rounded-full bg-[#1f8a4c]" />
            可用
          </span>
          <span className="flex items-center gap-1">
            <span className="w-1.5 h-1.5 rounded-full bg-[#d98a1c]" />
            冷却
          </span>
          {glCount > 0 && (
            <>
              <span className="w-[1px] h-3 bg-[#c9c5b8]" />
              <button
                onClick={() => handleSelectRegion('GL')}
                className="hover:text-[#e84c1e] font-semibold cursor-pointer transition-colors"
              >
                其他·{glCount}
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
};
