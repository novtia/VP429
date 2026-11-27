import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { FullStateDto } from './types';

export const isTauri = () => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export const api = {
  getState: () => invoke<FullStateDto>('get_state'),
  startProxy: () => invoke<void>('start_proxy'),
  stopProxy: () => invoke<void>('stop_proxy'),
  selectNode: (name: string) => invoke<string | null>('select_node', { name }),
  rotateNextNode: () => invoke<string | null>('rotate_next_node'),
  pingAllNodes: () => invoke<void>('ping_all_nodes'),
  checkExitIp: () => invoke<void>('check_exit_ip'),
  setOutboundMode: (mode: string) => invoke<void>('set_outbound_mode', { mode }),
  toggleSystemProxy: (enabled: boolean) => invoke<void>('toggle_system_proxy', { enabled }),
  toggleTun: (enabled: boolean) => invoke<void>('toggle_tun', { enabled }),
  toggleChainProxy: (enabled: boolean) => invoke<void>('toggle_chain_proxy', { enabled }),
  setChainExitNode: (name: string | null) => invoke<void>('set_chain_exit_node', { name }),
  setChainExitPool: (profileId: string | null) =>
    invoke<void>('set_chain_exit_pool', { profileId }),
  setRotationMode: (mode: string) => invoke<void>('set_rotation_mode', { mode }),
  addProfile: (name: string, url: string) => invoke<number>('add_profile', { name, url }),
  addProxyPool: (name: string, lines: string, protocol: string, useCurrentRelay: boolean) =>
    invoke<number>('add_proxy_pool', { name, lines, protocol, useCurrentRelay }),
  activateProfile: (id: string) => invoke<number>('activate_profile', { id }),
  refreshProfile: (id: string) => invoke<number>('refresh_profile', { id }),
  deleteProfile: (id: string) => invoke<void>('delete_profile', { id }),
  resetProfileTraffic: (id: string) => invoke<void>('reset_profile_traffic', { id }),
  setProfileQuota: (id: string, quotaGb: number) =>
    invoke<void>('set_profile_quota', { id, quotaGb }),
  saveStrategy: (payload: {
    cooldownSeconds: number;
    maxRetries: number;
    autoRetryOn429: boolean;
    watchDomains: string[];
    chainBackoffMs: number;
  }) => invoke<void>('save_strategy', payload),
  applyCustomChainExit: (raw: string, backoffMs: number) =>
    invoke<void>('apply_custom_chain_exit', { raw, backoffMs }),
  installCa: () => invoke<void>('install_ca'),
  downloadCore: () => invoke<void>('download_core'),
  saveSystemSettings: (payload: {
    localProxyPort: number;
    allowLan: boolean;
    mihomoMixedPort: number;
    tunMitmPort: number;
    mihomoBypassPort: number;
    mihomoApiPort: number;
    maxBodyMb: number;
    mihomoPath: string | null;
  }) => invoke<void>('save_system_settings', payload),
  setSidebarCollapsed: (collapsed: boolean) =>
    invoke<void>('set_sidebar_collapsed', { collapsed }),
  triggerElevate: () => invoke<void>('trigger_elevate'),
  hideToTray: () => invoke<void>('hide_to_tray'),
  minimize: () => getCurrentWindow().minimize(),
  toggleMaximize: () => getCurrentWindow().toggleMaximize(),
};
