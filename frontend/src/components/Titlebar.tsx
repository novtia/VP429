import React from 'react';
import { api, isTauri } from '../api';
import type { FullStateDto } from '../types';

interface TitlebarProps {
  data: FullStateDto;
  onToggleSys: () => void;
  onToggleTun: () => void;
}

export const Titlebar: React.FC<TitlebarProps> = ({ data, onToggleSys, onToggleTun }) => {
  const sysOn = data.config.enable_system_proxy;
  const tunOn = data.config.enable_tun;
  const port = data.config.local_proxy_port;

  return (
    <header
      data-tauri-drag-region
      className="braun-titlebar pl-3.5 pr-2 flex items-center justify-between text-xs z-30 shrink-0"
    >
      {/* Left: Brand Emblem + Title + Engraved Model Silk-Screen */}
      <div data-tauri-drag-region className="flex items-center gap-2.5">
        <div
          data-tauri-drag-region
          className="tb-brand-badge w-[23px] h-[23px] rounded-[6px] flex items-center justify-center"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
            <path
              d="M4 6L12 19L20 6"
              stroke="#191918"
              strokeWidth="2.6"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
            <path d="M12 19L16 12" stroke="#e84c1e" strokeWidth="2.6" strokeLinecap="round" />
          </svg>
        </div>
        <div data-tauri-drag-region className="flex items-baseline gap-2">
          <span data-tauri-drag-region className="font-semibold tracking-tight text-[#191918] text-[12.5px]">
            代理控制中心
          </span>
          <span
            data-tauri-drag-region
            className="hidden sm:inline text-[9.5px] font-mono font-semibold tracking-[0.14em] uppercase text-[#8b887f]"
          >
            VP429 · CORE
          </span>
        </div>
      </div>

      {/* Right: Recessed Telemetry Status Pill Tray + Tactile Window Controls */}
      <div className="flex items-center gap-3">
        <div className="tb-status-tray h-[24px] px-1 rounded-[6px] flex items-center gap-0.5 text-[10.5px] font-mono tabular-nums">
          <button
            onClick={onToggleSys}
            className="tb-status-chip h-[18px] px-2 rounded-[4px] flex items-center gap-1.5 cursor-pointer"
            title="点击快速切换系统代理"
          >
            <span
              className="w-1.5 h-1.5 rounded-full"
              style={{ backgroundColor: sysOn ? '#e84c1e' : '#9c9a92' }}
            />
            <span className={sysOn ? 'text-[#191918] font-semibold' : 'text-[#78766e] font-medium'}>
              {sysOn ? `SYS :${port}` : 'SYS OFF'}
            </span>
          </button>

          <span className="w-[1px] h-3 bg-[#c2bfb4]" />

          <button
            onClick={onToggleTun}
            className="tb-status-chip h-[18px] px-2 rounded-[4px] flex items-center gap-1.5 cursor-pointer"
            title="点击快速切换 TUN 虚拟网卡"
          >
            <span
              className="w-1.5 h-1.5 rounded-full"
              style={{ backgroundColor: tunOn ? '#1f8a4c' : '#9c9a92' }}
            />
            <span className={tunOn ? 'text-[#1f8a4c] font-semibold' : 'text-[#78766e] font-medium'}>
              {tunOn ? 'TUN ON' : 'TUN OFF'}
            </span>
          </button>

          <span className="w-[1px] h-3 bg-[#c2bfb4]" />

          <div className="h-[18px] px-2 rounded-[4px] flex items-center gap-1.5" title="MITM 根证书状态">
            <span
              className="w-1.5 h-1.5 rounded-full"
              style={{ backgroundColor: data.ca_installed ? '#1f8a4c' : '#e84c1e' }}
            />
            <span className={data.ca_installed ? 'text-[#1f8a4c] font-semibold' : 'text-[#e84c1e] font-semibold'}>
              {data.ca_installed ? 'CA OK' : 'CA 未装'}
            </span>
          </div>
        </div>

        <span className="w-[1px] h-4 bg-[#c6c3b8]" />

        <div className="flex items-center gap-1">
          <button
            onClick={() => isTauri() && api.minimize()}
            className="tb-win-btn flex items-center justify-center cursor-pointer"
            title="最小化"
          >
            <svg width="11" height="11" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
              <path d="M2.5 6h7" />
            </svg>
          </button>
          <button
            onClick={() => isTauri() && api.hideToTray()}
            className="tb-win-btn tb-win-close flex items-center justify-center cursor-pointer"
            title="最小化到系统托盘"
          >
            <svg width="10" height="10" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round">
              <path d="M2.5 2.5l7 7M9.5 2.5l-7 7" />
            </svg>
          </button>
        </div>
      </div>
    </header>
  );
};
