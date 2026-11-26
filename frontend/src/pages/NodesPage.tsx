import React, { useMemo, useState } from 'react';
import { Activity, RotateCw, Search, List, LayoutGrid, ArrowUpDown } from 'lucide-react';
import { FlagIcon } from '../components/FlagIcon';
import { cleanNodeName, detectCountry, type FullStateDto } from '../types';

interface NodesPageProps {
  data: FullStateDto;
  onSelectNode: (name: string) => void;
  onPingAll: () => void;
  onRotateNext: () => void;
  onToggleChainExit: (name: string | null) => void;
}

const REGIONS = ['ALL', 'JP', 'SG', 'HK', 'US', 'TW', 'OTHER'] as const;
const MAIN_REGIONS = new Set(['JP', 'SG', 'HK', 'US', 'TW']);

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
  const [region, setRegion] = useState<(typeof REGIONS)[number]>('ALL');
  const [search, setSearch] = useState('');
  const [sortLatency, setSortLatency] = useState(false);
  const [viewMode, setViewMode] = useState<'list' | 'grid'>('list');

  const chainExitName =
    data.config.chain_exit_source?.kind === 'subscription_node'
      ? data.config.chain_exit_source.name
      : null;

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    let list = data.nodes.filter((n) => {
      const c = detectCountry(n.name);
      if (region === 'OTHER') {
        if (MAIN_REGIONS.has(c.code)) return false;
      } else if (region !== 'ALL' && c.code !== region) {
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
  }, [data.nodes, region, search, sortLatency]);

  const latestLog = data.logs[data.logs.length - 1];

  return (
    <div className="w-full h-full flex flex-col gap-3">
      {/* Top Control Deck */}
      <div className="ceramic-panel rounded-[12px] px-4 py-2.5 flex items-center justify-between gap-3 shrink-0">
        <div className="flex items-center gap-3.5">
          <div className="flex items-baseline gap-2">
            <span className="text-[15px] font-bold tracking-tight text-[#191918]">节点列表</span>
            <span className="text-[9.5px] font-mono font-semibold tracking-[0.14em] uppercase text-[#8a877d]">
              NODE MATRIX
            </span>
          </div>

          <div className="tb-status-tray h-[26px] px-2 rounded-[6px] flex items-center gap-2.5 text-[10.5px] font-mono tabular-nums">
            <span className="flex items-center gap-1.5 text-[#1f8a4c] font-semibold">
              <span className="w-1.5 h-1.5 rounded-full bg-[#1f8a4c]" />
              可用 {data.ready_count}
            </span>
            <span className="w-[1px] h-3 bg-[#c2bfb4]" />
            <span className="flex items-center gap-1.5 text-[#c97a16] font-semibold">
              <span className="w-1.5 h-1.5 rounded-full bg-[#d98a1c]" />
              冷却 {data.cooldown_count}
            </span>
            <span className="w-[1px] h-3 bg-[#c2bfb4]" />
            <span className="text-[#54524c] font-semibold">总计 {data.nodes.length}</span>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={onPingAll}
            className="hw-micro-btn h-[30px] px-3 rounded-[6px] flex items-center gap-1.5 text-xs font-semibold text-[#191918] cursor-pointer"
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
            <span>{data.testing_latency ? '测速中...' : '全部测速'}</span>
          </button>

          <button
            onClick={onRotateNext}
            className="hw-btn-primary h-[30px] px-3.5 rounded-[6px] flex items-center gap-1.5 text-xs font-semibold cursor-pointer"
          >
            <RotateCw size={13} />
            <span>轮换下一节点</span>
          </button>
        </div>
      </div>

      {/* Filter, Search, Sort & List/Grid Switcher */}
      <div className="flex items-center justify-between gap-3 shrink-0">
        <div className="hw-keycap-tray p-1 rounded-[7px] flex items-center gap-1 text-[11px] font-mono">
          {REGIONS.map((reg) => (
            <button
              key={reg}
              onClick={() => setRegion(reg)}
              className={`hw-keycap px-2.5 py-1 rounded-[4px] cursor-pointer ${
                region === reg ? 'is-active font-semibold' : ''
              }`}
            >
              {reg === 'ALL' ? '全部' : reg === 'OTHER' ? '其他' : reg}
            </button>
          ))}
        </div>

        <div className="flex items-center gap-2">
          <div className="hw-readout-bay h-[30px] w-[190px] px-2.5 rounded-[6px] flex items-center gap-1.5">
            <Search size={12} className="text-[#78766e] shrink-0" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="筛选节点 / 协议..."
              className="w-full bg-transparent text-[11.5px] text-[#191918] placeholder-[#8c8a82] outline-none font-mono"
            />
          </div>

          <button
            onClick={() => setSortLatency((s) => !s)}
            className="hw-micro-btn h-[30px] px-2.5 rounded-[6px] flex items-center gap-1.5 text-[11px] font-semibold cursor-pointer whitespace-nowrap"
            style={{ color: sortLatency ? '#e84c1e' : '#54524c' }}
          >
            <ArrowUpDown size={12} />
            <span>{sortLatency ? '延迟升序' : '默认顺序'}</span>
          </button>

          <div className="hw-keycap-tray h-[30px] p-[3px] rounded-[6px] flex items-center gap-0.5">
            <button
              onClick={() => setViewMode('list')}
              className={`hw-keycap h-full w-[26px] rounded-[4px] flex items-center justify-center cursor-pointer ${
                viewMode === 'list' ? 'is-active' : ''
              }`}
              title="列表视图"
            >
              <List size={12} />
            </button>
            <button
              onClick={() => setViewMode('grid')}
              className={`hw-keycap h-full w-[26px] rounded-[4px] flex items-center justify-center cursor-pointer ${
                viewMode === 'grid' ? 'is-active' : ''
              }`}
              title="宫格视图"
            >
              <LayoutGrid size={12} />
            </button>
          </div>
        </div>
      </div>

      {/* Molded Ceramic Node List / Grid Container */}
      <div className="ceramic-panel flex-1 min-h-0 rounded-[12px] flex flex-col overflow-hidden">
        {viewMode === 'list' && (
          <div className="px-4 py-2 bg-[#eceae2] border-b border-[#d5d2c8] flex items-center justify-between text-[10.5px] font-mono font-semibold uppercase tracking-wider text-[#78766e] shrink-0">
            <span>地区 / 节点名称 / 协议</span>
            <div className="flex items-center gap-5 pr-1">
              <span>链式落地</span>
              <span className="w-[74px] text-right">节点状态</span>
              <span className="w-[82px] text-right">延迟 / 信号</span>
            </div>
          </div>
        )}

        <div className="nodes-scroll flex-1 min-h-0 overflow-y-auto">
          {filtered.length === 0 ? (
            <div className="h-full flex items-center justify-center text-xs font-mono text-[#8c8a82]">
              没有匹配的节点
            </div>
          ) : viewMode === 'grid' ? (
            <div className="p-3 grid grid-cols-3 gap-2.5">
              {filtered.map((n) => {
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
                    : 'text-[#54524c] font-medium';

                return (
                  <div
                    key={n.name}
                    onClick={() => onSelectNode(n.name)}
                    className={`node-grid-card rounded-[8px] p-2.5 flex flex-col justify-between gap-2 cursor-pointer ${
                      n.is_active ? 'is-active' : ''
                    }`}
                  >
                    <div className="flex items-center justify-between gap-1.5">
                      <div className="flex items-center gap-1.5 min-w-0">
                        <span className="w-[6.5px] h-[6.5px] rounded-full shrink-0" style={{ background: dotColor }} />
                        <FlagIcon code={c.code} size="sm" />
                        <span className="font-mono text-[10px] font-bold text-[#78766e]">{c.code}</span>
                        <span className="hw-readout-bay px-1.5 py-[0.5px] rounded text-[9px] font-mono font-semibold text-[#626058]">
                          {n.proxy_type}
                        </span>
                      </div>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          onToggleChainExit(isExit ? null : n.name);
                        }}
                        className={`${
                          isExit ? 'hw-btn-primary' : 'hw-micro-btn text-[#54524c]'
                        } px-1.5 py-[1px] rounded-[4px] text-[9.5px] font-semibold cursor-pointer whitespace-nowrap shrink-0`}
                      >
                        {isExit ? '★ 落地' : '设落地'}
                      </button>
                    </div>

                    <div className="min-w-0">
                      <div
                        className={`text-[12px] truncate ${
                          n.is_active ? 'font-bold text-[#191918]' : 'font-semibold text-[#2c2b28]'
                        }`}
                      >
                        {cleanNodeName(n.name)}
                      </div>
                      <div className="text-[10px] font-mono text-[#86837a] truncate mt-0.5">
                        {n.server}:{n.port}
                      </div>
                    </div>

                    <div className="pt-1.5 border-t border-[#e2e0d7] flex items-center justify-between gap-2 tabular-nums">
                      <span className={`text-[10px] font-mono ${statusColor}`}>{statusText}</span>
                      <div className="flex items-center gap-1.5">
                        <span className={`font-mono text-[11px] ${latColor}`}>{latText}</span>
                        <SignalBars ms={n.latency_ms} />
                      </div>
                    </div>
                  </div>
                );
              })}
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
                  ? `429 冷却 ${n.cooldown_remaining_secs}s`
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
                  : 'text-[#54524c] font-medium';

              return (
                <div
                  key={n.name}
                  onClick={() => onSelectNode(n.name)}
                  className={`node-list-row px-4 py-2.5 flex items-center justify-between gap-3 cursor-pointer text-xs ${
                    n.is_active ? 'is-active' : ''
                  }`}
                >
                  <div className="flex items-center gap-2.5 min-w-0">
                    <span className="w-[7px] h-[7px] rounded-full shrink-0" style={{ background: dotColor }} />
                    <FlagIcon code={c.code} size="sm" />
                    <span className="w-[22px] font-mono text-[10.5px] font-bold text-[#78766e] shrink-0">
                      {c.code}
                    </span>
                    <span
                      className={`truncate ${
                        n.is_active ? 'font-bold text-[#191918]' : 'font-medium text-[#2c2b28]'
                      }`}
                    >
                      {cleanNodeName(n.name)}
                    </span>
                    <span className="hw-readout-bay px-1.5 py-[1px] rounded text-[9.5px] font-mono font-semibold text-[#626058] shrink-0">
                      {n.proxy_type}
                    </span>
                  </div>

                  <div className="flex items-center gap-5 shrink-0 tabular-nums">
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        onToggleChainExit(isExit ? null : n.name);
                      }}
                      className={`${
                        isExit ? 'hw-btn-primary' : 'hw-micro-btn text-[#54524c]'
                      } px-2 py-[2px] rounded-[4px] text-[10.5px] font-semibold cursor-pointer whitespace-nowrap`}
                    >
                      {isExit ? '★ 落地' : '设为落地'}
                    </button>
                    <span className={`w-[74px] text-right text-[11px] font-mono ${statusColor}`}>{statusText}</span>
                    <div className="w-[82px] flex items-center justify-end gap-2">
                      <span className={`font-mono text-[11.5px] ${latColor}`}>{latText}</span>
                      <SignalBars ms={n.latency_ms} />
                    </div>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>

      {/* Bottom Recessed Ticker Log Bay */}
      <div className="hw-readout-bay rounded-[7px] px-3 py-1.5 flex items-center justify-between gap-3 text-[11px] font-mono shrink-0">
        <div className="flex items-center gap-2 min-w-0">
          <span
            className="w-1.5 h-1.5 rounded-full shrink-0"
            style={{
              backgroundColor:
                latestLog?.level === 'ERR' || latestLog?.level === '429'
                  ? '#e84c1e'
                  : latestLog?.level === 'OK'
                  ? '#1f8a4c'
                  : '#78766e',
            }}
          />
          <span className="text-[#54524c] truncate">
            {latestLog ? `[${latestLog.timestamp}] ${latestLog.message}` : '调度器就绪'}
          </span>
        </div>
        <span
          className="text-[10px] shrink-0"
          style={{ color: chainExitName ? '#e84c1e' : '#78766e' }}
        >
          {chainExitName ? `★ 链式落地：${cleanNodeName(chainExitName)}` : '链式落地：未指定'}
        </span>
      </div>
    </div>
  );
};
