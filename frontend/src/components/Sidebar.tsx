import React from 'react';
import {
  Gauge,
  Globe,
  FileText,
  Shield,
  Terminal,
  Settings,
  Power,
  PanelLeftOpen,
  PanelLeftClose,
} from 'lucide-react';
import type { NavPage } from '../types';

interface SidebarProps {
  activePage: NavPage;
  expanded: boolean;
  running: boolean;
  nodeCount: number;
  onSelectPage: (page: NavPage) => void;
  onToggleExpand: () => void;
  onPowerAction: () => void;
}

const primaryItems: { id: NavPage; label: string; Icon: React.FC<{ size?: number; className?: string }> }[] = [
  { id: 'dashboard', label: '仪表盘', Icon: Gauge },
  { id: 'nodes', label: '节点列表', Icon: Globe },
  { id: 'profiles', label: '配置文件', Icon: FileText },
  { id: 'strategy', label: '429 策略', Icon: Shield },
];

const utilityItems: { id: NavPage; label: string; Icon: React.FC<{ size?: number; className?: string }> }[] = [
  { id: 'logs', label: '连接日志', Icon: Terminal },
  { id: 'settings', label: '系统设置', Icon: Settings },
];

export const Sidebar: React.FC<SidebarProps> = ({
  activePage,
  expanded,
  running,
  nodeCount,
  onSelectPage,
  onToggleExpand,
  onPowerAction,
}) => {
  const renderBtn = (item: (typeof primaryItems)[number]) => {
    const active = activePage === item.id;
    const { Icon } = item;
    return (
      <button
        key={item.id}
        onClick={() => onSelectPage(item.id)}
        className={`sb-nav-btn w-full flex items-center px-2 gap-2.5 cursor-pointer ${
          active ? 'is-active' : ''
        } ${expanded ? 'justify-start pl-3' : 'justify-center'}`}
      >
        <span className="sb-led-slit" />
        <Icon size={16} className="sb-ico shrink-0" />
        {expanded && (
          <span className={`flex-1 text-left text-[12px] truncate ${active ? 'font-semibold' : 'font-medium'}`}>
            {item.label}
          </span>
        )}
        {expanded && item.id === 'nodes' && (
          <span className="px-1.5 py-[1px] rounded bg-[#dcdad1] text-[9.5px] font-mono font-semibold text-[#54524c] tabular-nums">
            {nodeCount}
          </span>
        )}
        <span className="sb-tooltip">
          {item.id === 'nodes' ? `${item.label} · ${nodeCount}` : item.label}
        </span>
      </button>
    );
  };

  return (
    <aside className={`braun-sidebar shrink-0 px-2 py-2.5 flex flex-col justify-between z-20 ${expanded ? 'is-expanded' : ''}`}>
      <div className="w-full flex flex-col gap-2">
        <div className="w-full flex items-center justify-between px-0.5">
          {expanded && (
            <span className="text-[9.5px] font-mono font-semibold tracking-[0.14em] uppercase text-[#86837a] pl-1.5">
              CONTROL
            </span>
          )}
          <button
            onClick={onToggleExpand}
            className={`hw-micro-btn rounded-[6px] flex items-center justify-center text-[#626058] cursor-pointer shrink-0 ${
              expanded ? 'w-7 h-7' : 'w-9 h-9 mx-auto'
            }`}
            title="展开 / 收起侧边栏"
          >
            {expanded ? <PanelLeftClose size={15} /> : <PanelLeftOpen size={16} />}
          </button>
        </div>

        <nav className="sb-nav-tray w-full p-1 rounded-[9px] flex flex-col gap-1">
          {primaryItems.map(renderBtn)}
          <div className="sb-divider my-0.5 mx-1.5" />
          {utilityItems.map(renderBtn)}
        </nav>
      </div>

      <div className="w-full flex flex-col gap-2">
        <div className="flex items-center justify-center gap-1.5 opacity-60 pointer-events-none">
          <span className="w-1 h-1 rounded-full bg-[#b4b1a6] shadow-[0_1px_0_rgba(255,255,255,0.8)]" />
          <span className="w-1 h-1 rounded-full bg-[#b4b1a6] shadow-[0_1px_0_rgba(255,255,255,0.8)]" />
          <span className="w-1 h-1 rounded-full bg-[#b4b1a6] shadow-[0_1px_0_rgba(255,255,255,0.8)]" />
        </div>

        <div className="sb-power-socket w-full p-1 rounded-[9px]">
          <button
            onClick={onPowerAction}
            className={`sb-power-btn w-full h-9 rounded-[6px] px-2 cursor-pointer flex items-center justify-center gap-2 ${
              running ? 'is-running' : ''
            }`}
            title="启动 / 停止代理"
          >
            <Power size={16} className="shrink-0" />
            {expanded && (
              <span className="text-[11.5px] font-semibold tracking-wide whitespace-nowrap">
                {running ? '运行中 · 停止' : '启动代理'}
              </span>
            )}
          </button>
        </div>
      </div>
    </aside>
  );
};
