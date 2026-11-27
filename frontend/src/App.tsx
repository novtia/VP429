import React, { useCallback, useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { api, isTauri } from './api';
import { Titlebar } from './components/Titlebar';
import { Sidebar } from './components/Sidebar';
import { DashboardPage } from './pages/DashboardPage';
import { NodesPage } from './pages/NodesPage';
import { LogsPage, ProfilesPage, SettingsPage, StrategyPage } from './pages/SubPages';
import type { FullStateDto, NavPage, OutboundMode, RotationKey } from './types';

const INITIAL_STATE: FullStateDto = {
  running: false,
  testing_latency: false,
  ca_installed: true,
  elevated: true,
  active_node: '日本-上海专线 BGP',
  nodes: [
    { name: '日本-上海专线 BGP', proxy_type: 'VLESS', server: 'jp.iepl.net', port: 443, latency_ms: 42, cooldown_remaining_secs: 0, hit_429_count: 0, success_count: 12, is_active: true },
    { name: '新加坡 狮城 03 · BGP', proxy_type: 'Trojan', server: 'sg03.bgp.net', port: 443, latency_ms: 58, cooldown_remaining_secs: 0, hit_429_count: 0, success_count: 4, is_active: false },
    { name: '香港 葵涌 02 · IPLC', proxy_type: 'SS', server: 'hk02.iplc.net', port: 8443, latency_ms: 29, cooldown_remaining_secs: 0, hit_429_count: 0, success_count: 8, is_active: false },
    { name: '美国 硅谷 05 · GIA', proxy_type: 'VLESS', server: 'us05.cn2.net', port: 443, latency_ms: 138, cooldown_remaining_secs: 0, hit_429_count: 1, success_count: 2, is_active: false },
    { name: '台湾 台北 01 · HiNet', proxy_type: 'Trojan', server: 'tw01.hinet.net', port: 443, latency_ms: 47, cooldown_remaining_secs: 28, hit_429_count: 2, success_count: 0, is_active: false },
  ],
  total_429: 0,
  total_retries_ok: 0,
  ready_count: 4,
  cooldown_count: 1,
  telemetry: {
    up_bps: 0,
    down_bps: 0,
    total_up: 0,
    total_down: 0,
    speed_history: Array.from({ length: 22 }, () => [0, 0]),
    uptime_secs: 0,
    local_ip: '198.18.0.0',
    exit_ip: null,
    exit_country_code: null,
    checking_ip: false,
    outbound_mode: 'rule',
  },
  config: {
    subscription_url: '',
    profiles: [],
    active_profile_id: null,
    mihomo_path: null,
    local_proxy_port: 7890,
    allow_lan: false,
    mihomo_mixed_port: 17890,
    tun_mitm_port: 17891,
    mihomo_bypass_port: 17892,
    mihomo_api_port: 19090,
    enable_system_proxy: true,
    enable_tun: true,
    enable_chain_proxy: false,
    chain_exit_source: null,
    chain_429_backoff_ms: 1500,
    rotation_mode: 'On429Only',
    cooldown_seconds: 60,
    auto_retry_on_429: true,
    max_retries: 2,
    max_retry_body_bytes: 16 * 1024 * 1024,
    watch_domains: [],
    intercept_status_codes: [429],
    selected_node: null,
    outbound_mode: 'rule',
    sidebar_collapsed: true,
  },
  logs: [{ timestamp: '00:00:00', level: 'INFO', message: 'VP429 React + Tauri v2 前端已就绪' }],
};

export default function App() {
  const [data, setData] = useState<FullStateDto>(INITIAL_STATE);
  const [activePage, setActivePage] = useState<NavPage>('dashboard');
  const [sidebarExpanded, setSidebarExpanded] = useState(false);
  const [dialPage, setDialPage] = useState<'power' | 'speed'>('power');
  const [launching, setLaunching] = useState(false);
  const [launchStartMs, setLaunchStartMs] = useState(0);
  const [stopping, setStopping] = useState(false);
  const [stopStartMs, setStopStartMs] = useState(0);
  const [peakDownBps, setPeakDownBps] = useState(0);

  const syncedOnceRef = useRef(false);
  const pollBusyRef = useRef(false);
  const pollQueuedRef = useRef(false);
  const animGuardRef = useRef({ launching: false, stopping: false });
  animGuardRef.current = { launching, stopping };

  const pollState = useCallback(async () => {
    if (!isTauri()) return;
    if (pollBusyRef.current) {
      pollQueuedRef.current = true;
      return;
    }
    pollBusyRef.current = true;
    try {
      do {
        pollQueuedRef.current = false;
        const dto = await api.getState();
        setData(dto);
        if (dto.running && dto.telemetry.down_bps > 0) {
          setPeakDownBps((prev) => Math.max(prev, dto.telemetry.down_bps));
        }
        if (!syncedOnceRef.current) {
          setSidebarExpanded(!dto.config.sidebar_collapsed);
          setDialPage(dto.running ? 'speed' : 'power');
          syncedOnceRef.current = true;
        } else if (!animGuardRef.current.launching && !animGuardRef.current.stopping && !dto.running) {
          setDialPage('power');
        }
      } while (pollQueuedRef.current);
    } catch (err) {
      console.error('[VP429] get_state failed:', err);
    } finally {
      pollBusyRef.current = false;
    }
  }, []);

  useEffect(() => {
    pollState();
    let unlisten: (() => void) | undefined;
    if (isTauri()) {
      listen('vp429-state-changed', () => {
        pollState();
      })
        .then((fn) => {
          unlisten = fn;
        })
        .catch(() => {});
    }
    const id = setInterval(pollState, 1000);
    return () => {
      clearInterval(id);
      if (unlisten) unlisten();
    };
  }, [pollState]);

  const handlePowerAction = useCallback(() => {
    if (launching || stopping) return;
    if (!data.running) {
      setLaunching(true);
      setLaunchStartMs(performance.now());
      if (isTauri()) {
        api
          .startProxy()
          .then(pollState)
          .catch(() => {
            setLaunching(false);
            pollState();
          });
      } else {
        setData((d) => ({ ...d, running: true }));
      }
    } else {
      setStopping(true);
      setStopStartMs(performance.now());
      if (isTauri()) {
        api.stopProxy().then(pollState).catch(() => {});
      }
    }
  }, [data.running, launching, stopping, pollState]);

  const handleToggleSidebar = useCallback(() => {
    setSidebarExpanded((prev) => {
      const next = !prev;
      if (isTauri()) api.setSidebarCollapsed(!next).catch(() => {});
      return next;
    });
  }, []);

  const handleToggleSys = useCallback(() => {
    const next = !data.config.enable_system_proxy;
    if (isTauri()) {
      api.toggleSystemProxy(next).then(pollState);
    } else {
      setData((d) => ({ ...d, config: { ...d.config, enable_system_proxy: next } }));
    }
  }, [data.config.enable_system_proxy, pollState]);

  const handleToggleTun = useCallback(() => {
    const next = !data.config.enable_tun;
    if (isTauri()) {
      api.toggleTun(next).then(pollState);
    } else {
      setData((d) => ({ ...d, config: { ...d.config, enable_tun: next } }));
    }
  }, [data.config.enable_tun, pollState]);

  const handleToggleChain = useCallback(() => {
    const next = !data.config.enable_chain_proxy;
    if (isTauri()) {
      api.toggleChainProxy(next).then(pollState);
    } else {
      setData((d) => ({ ...d, config: { ...d.config, enable_chain_proxy: next } }));
    }
  }, [data.config.enable_chain_proxy, pollState]);

  const handleElevate = useCallback(() => {
    if (isTauri()) {
      api.triggerElevate().catch(() => {});
    }
  }, []);

  const handleSetOutbound = useCallback(
    (mode: OutboundMode) => {
      if (isTauri()) {
        api.setOutboundMode(mode).then(pollState);
      } else {
        setData((d) => ({
          ...d,
          telemetry: { ...d.telemetry, outbound_mode: mode },
          config: { ...d.config, outbound_mode: mode },
        }));
      }
    },
    [pollState]
  );

  const handleSetRotation = useCallback(
    (rot: RotationKey) => {
      if (isTauri()) {
        api.setRotationMode(rot).then(pollState);
      } else {
        const mode =
          rot === 'req'
            ? 'RoundRobinPerRequest'
            : rot === 'int'
            ? { RoundRobinInterval: { seconds: 60 } }
            : 'On429Only';
        setData((d) => ({ ...d, config: { ...d.config, rotation_mode: mode } }));
      }
    },
    [pollState]
  );

  const handleSelectNode = useCallback(
    (name: string) => {
      if (isTauri()) {
        api.selectNode(name).then(pollState);
      } else {
        setData((d) => ({
          ...d,
          active_node: name,
          nodes: d.nodes.map((n) => ({ ...n, is_active: n.name === name })),
        }));
      }
    },
    [pollState]
  );

  return (
    <div className="braun-chassis w-full h-full flex flex-col select-none overflow-hidden">
      <Titlebar data={data} onToggleSys={handleToggleSys} onToggleTun={handleToggleTun} />

      <div className="flex flex-1 min-h-0">
        <Sidebar
          activePage={activePage}
          expanded={sidebarExpanded}
          running={data.running}
          launching={launching}
          stopping={stopping}
          nodeCount={data.nodes.length}
          onSelectPage={setActivePage}
          onToggleExpand={handleToggleSidebar}
          onPowerAction={handlePowerAction}
        />

        <main
          className={`chassis-backplane flex-1 min-w-0 flex flex-col justify-center relative overflow-hidden ${
            activePage === 'nodes' ? 'p-0' : 'p-4'
          }`}
        >
          {activePage !== 'nodes' && (
            <>
              <span className="chassis-screw absolute top-2 left-2 pointer-events-none" />
              <span className="chassis-screw absolute top-2 right-2 pointer-events-none" />
              <span className="chassis-screw absolute bottom-2 left-2 pointer-events-none" />
              <span className="chassis-screw absolute bottom-2 right-2 pointer-events-none" />
            </>
          )}

          {activePage === 'dashboard' && (
            <DashboardPage
              data={data}
              peakDownBps={peakDownBps}
              dialPage={dialPage}
              launching={launching}
              launchStartMs={launchStartMs}
              stopping={stopping}
              stopStartMs={stopStartMs}
              onFlipPage={() => {
                if (data.running && !launching && !stopping) {
                  setDialPage((p) => (p === 'speed' ? 'power' : 'speed'));
                }
              }}
              onPowerAction={handlePowerAction}
              onSelectNode={handleSelectNode}
              onToggleSys={handleToggleSys}
              onToggleTun={handleToggleTun}
              onToggleChain={handleToggleChain}
              onElevate={handleElevate}
              onSetOutbound={handleSetOutbound}
              onSetRotation={handleSetRotation}
              onCheckExitIp={() => {
                setData((d) => ({ ...d, telemetry: { ...d.telemetry, checking_ip: true } }));
                if (isTauri()) {
                  api.checkExitIp().then(pollState).catch(() => pollState());
                } else {
                  setTimeout(() => {
                    setData((d) => ({
                      ...d,
                      telemetry: {
                        ...d.telemetry,
                        checking_ip: false,
                        exit_ip: d.telemetry.exit_ip || '178.249.213.185',
                        exit_country_code: d.telemetry.exit_country_code || 'JP',
                      },
                    }));
                  }, 540);
                }
              }}
              onLaunchShowSpeed={() => setDialPage('speed')}
              onLaunchComplete={() => {
                setLaunching(false);
                pollState();
              }}
              onStopShowPower={() => setDialPage('power')}
              onStopComplete={() => {
                setStopping(false);
                if (!isTauri()) setData((d) => ({ ...d, running: false }));
                setDialPage('power');
                pollState();
              }}
            />
          )}

          {activePage === 'nodes' && (
            <NodesPage
              data={data}
              onSelectNode={handleSelectNode}
              onPingAll={() => isTauri() && api.pingAllNodes().then(pollState)}
              onRotateNext={() => isTauri() && api.rotateNextNode().then(pollState)}
              onToggleChainExit={(name) => isTauri() && api.setChainExitNode(name).then(pollState)}
            />
          )}

          {activePage === 'profiles' && <ProfilesPage data={data} onRefresh={pollState} />}
          {activePage === 'strategy' && <StrategyPage data={data} onRefresh={pollState} />}
          {activePage === 'logs' && <LogsPage data={data} />}
          {activePage === 'settings' && <SettingsPage data={data} onRefresh={pollState} />}
        </main>
      </div>
    </div>
  );
}
