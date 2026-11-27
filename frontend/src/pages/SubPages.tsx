import React, { useEffect, useState } from 'react';
import { api } from '../api';
import type { FullStateDto } from '../types';

const formatBytes = (bytes: number): string => {
  if (!bytes || bytes <= 0) return '0 B';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
};

export const ProfilesPage: React.FC<{ data: FullStateDto; onRefresh: () => void }> = ({
  data,
  onRefresh,
}) => {
  const [name, setName] = useState('');
  const [url, setUrl] = useState('');
  const [busy, setBusy] = useState(false);
  const [showPoolForm, setShowPoolForm] = useState(false);
  const [poolName, setPoolName] = useState('');
  const [poolProto, setPoolProto] = useState<'http' | 'socks5'>('http');
  const [poolUseRelay, setPoolUseRelay] = useState(true);
  const [poolLines, setPoolLines] = useState('');
  const [poolError, setPoolError] = useState<string | null>(null);
  const [editingQuotaId, setEditingQuotaId] = useState<string | null>(null);
  const [quotaInput, setQuotaInput] = useState('10');

  const handleAdd = async () => {
    if (!url.trim() || busy) return;
    setBusy(true);
    try {
      await api.addProfile(name.trim(), url.trim());
      setName('');
      setUrl('');
      onRefresh();
    } finally {
      setBusy(false);
    }
  };

  const handleAddPool = async () => {
    if (!poolLines.trim() || busy) return;
    setBusy(true);
    setPoolError(null);
    try {
      await api.addProxyPool(poolName.trim(), poolLines, poolProto, poolUseRelay && !!data.active_node);
      setPoolName('');
      setPoolLines('');
      setShowPoolForm(false);
      onRefresh();
    } catch (e) {
      setPoolError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="w-full h-full flex flex-col gap-3">
      <div className="ceramic-panel rounded-[12px] px-4 py-2.5 flex items-center justify-between gap-3 shrink-0">
        <div className="flex items-baseline gap-2">
          <span className="text-[15px] font-bold text-[#191918]">配置文件</span>
          <span className="text-[9.5px] font-mono font-semibold tracking-[0.14em] uppercase text-[#8a877d]">
            SUBSCRIPTIONS & POOLS
          </span>
        </div>
        <div className="flex items-center gap-2">
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="配置名(可选)"
            className="hw-readout-bay h-[30px] w-[120px] px-2.5 rounded-[6px] text-xs font-mono text-[#191918] outline-none"
          />
          <input
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="粘贴 Clash / Mihomo 订阅 URL..."
            className="hw-readout-bay h-[30px] w-[240px] px-2.5 rounded-[6px] text-xs font-mono text-[#191918] outline-none"
          />
          <button
            onClick={handleAdd}
            disabled={busy}
            className="hw-btn-primary h-[30px] px-3 rounded-[6px] text-xs font-semibold cursor-pointer"
          >
            {busy ? '同步中...' : '+ 添加订阅'}
          </button>
          <button
            onClick={() => {
              setShowPoolForm((v) => !v);
              setPoolError(null);
            }}
            className={`h-[30px] px-3 rounded-[6px] text-xs font-semibold cursor-pointer ${
              showPoolForm ? 'hw-btn-primary' : 'hw-micro-btn text-[#191918]'
            }`}
          >
            {showPoolForm ? '收起流量池' : '+ 导入流量池'}
          </button>
        </div>
      </div>

      {showPoolForm && (
        <div className="ceramic-panel rounded-[12px] p-3.5 flex flex-col gap-2.5 shrink-0 text-xs">
          <div className="flex items-center justify-between gap-2">
            <div className="flex items-center gap-2">
              <span className="font-bold text-[#191918]">导入住宅代理流量池</span>
              <span className="text-[10.5px] text-[#78766e]">
                每行一条，支持 host:port:user:pass 或 http:// / socks5:// 链接，导入后支持 429 自动轮换
              </span>
            </div>
            <div className="flex items-center gap-1.5">
              <input
                value={poolName}
                onChange={(e) => setPoolName(e.target.value)}
                placeholder="流量池名称 (如 Surflare 美国住宅)"
                className="hw-readout-bay h-[28px] w-[210px] px-2.5 rounded-[6px] text-xs font-mono text-[#191918] outline-none"
              />
              <select
                value={poolProto}
                onChange={(e) => setPoolProto(e.target.value as 'http' | 'socks5')}
                className="hw-readout-bay h-[28px] px-2 rounded-[6px] text-xs font-mono text-[#191918] outline-none cursor-pointer"
              >
                <option value="http">HTTP</option>
                <option value="socks5">SOCKS5</option>
              </select>
              <button
                onClick={handleAddPool}
                disabled={busy || !poolLines.trim()}
                className="hw-btn-primary h-[28px] px-3 rounded-[6px] text-xs font-semibold cursor-pointer disabled:opacity-50"
              >
                {busy ? '导入中...' : '导入并激活'}
              </button>
            </div>
          </div>
          <textarea
            rows={5}
            value={poolLines}
            onChange={(e) => setPoolLines(e.target.value)}
            placeholder={
              'gate-hk.surflare.net:1000:username:password-US-session1\ngate-hk.surflare.net:1000:username:password-US-session2'
            }
            className="hw-readout-bay w-full p-2.5 rounded-[6px] font-mono text-[11px] text-[#191918] outline-none resize-none"
          />
          <label className="flex items-center gap-2 text-[11px] text-[#54524c] select-none cursor-pointer">
            <input
              type="checkbox"
              checked={poolUseRelay && !!data.active_node}
              disabled={!data.active_node}
              onChange={(e) => setPoolUseRelay(e.target.checked)}
              className="accent-[#e84c1e]"
            />
            <span>
              通过当前节点中转（国内直连海外流量池必选）
              {data.active_node ? `：${data.active_node}` : '（请先激活一个机场订阅并选中前置节点）'}
            </span>
          </label>
          {poolError && (
            <div className="text-[11px] font-mono text-[#e84c1e]">{poolError}</div>
          )}
        </div>
      )}

      <div className="ceramic-panel flex-1 min-h-0 rounded-[12px] p-3 overflow-y-auto nodes-scroll grid grid-cols-2 gap-2.5 content-start">
        {data.config.profiles.length === 0 ? (
          <div className="col-span-2 py-16 text-center text-xs font-mono text-[#8c8a82]">
            暂无订阅配置，请在上方输入 Clash / Mihomo 订阅链接或导入住宅代理流量池
          </div>
        ) : (
          data.config.profiles.map((p) => {
            const isAct = p.id === data.config.active_profile_id;
            const isPool = p.url.startsWith('pool://');
            const isChainPool =
              data.config.chain_exit_source?.kind === 'profile_pool' &&
              data.config.chain_exit_source.profile_id === p.id;
            return (
              <div
                key={p.id}
                className={`node-grid-card rounded-[9px] p-3 flex flex-col justify-between gap-2 ${
                  isAct ? 'is-active' : ''
                }`}
              >
                <div className="flex items-center justify-between gap-2">
                  <div className="flex items-center gap-1.5 min-w-0">
                    <span className="font-bold text-xs text-[#191918] truncate">{p.name}</span>
                    {isPool && (
                      <span className="px-1.5 py-0.5 rounded bg-[#e84c1e]/12 text-[#e84c1e] text-[9.5px] font-mono font-semibold shrink-0">
                        流量池
                      </span>
                    )}
                    {isChainPool && (
                      <span className="px-1.5 py-0.5 rounded bg-[#1f8a4c]/15 text-[#1f8a4c] text-[9.5px] font-mono font-semibold shrink-0">
                        ★ 链式落地
                      </span>
                    )}
                  </div>
                  <span className="hw-readout-bay px-1.5 py-0.5 rounded text-[10px] font-mono text-[#54524c] shrink-0">
                    {p.node_count} 节点
                  </span>
                </div>
                <div className="text-[10px] font-mono text-[#78766e] truncate">{p.url}</div>
                {(() => {
                  const localUsed = p.local_used_bytes ?? 0;
                  const quota = p.local_quota_bytes || p.total_bytes || 0;
                  const pct = quota > 0 ? Math.min(100, (localUsed / quota) * 100) : 0;
                  const isHigh = pct >= 80;
                  const isEditing = editingQuotaId === p.id;
                  return (
                    <div className="flex flex-col gap-1">
                      <div className="flex items-center justify-between gap-2 text-[10px] font-mono">
                        <span className="text-[#54524c] tabular-nums">
                          本地已用: <strong className="text-[#191918]">{formatBytes(localUsed)}</strong>
                          {quota > 0 ? ` / ${formatBytes(quota)} (${pct.toFixed(1)}%)` : ''}
                        </span>
                        <div className="flex items-center gap-1">
                          {isEditing ? (
                            <>
                              <input
                                type="number"
                                step="0.5"
                                min="0"
                                value={quotaInput}
                                onChange={(e) => setQuotaInput(e.target.value)}
                                onKeyDown={(e) => {
                                  if (e.key === 'Enter') {
                                    api
                                      .setProfileQuota(p.id, parseFloat(quotaInput) || 0)
                                      .then(() => {
                                        setEditingQuotaId(null);
                                        onRefresh();
                                      });
                                  } else if (e.key === 'Escape') {
                                    setEditingQuotaId(null);
                                  }
                                }}
                                placeholder="GB"
                                className="hw-readout-bay w-[52px] h-[18px] px-1 rounded text-[10px] text-[#191918] outline-none"
                                autoFocus
                              />
                              <button
                                onClick={() =>
                                  api
                                    .setProfileQuota(p.id, parseFloat(quotaInput) || 0)
                                    .then(() => {
                                      setEditingQuotaId(null);
                                      onRefresh();
                                    })
                                }
                                className="text-[#1f8a4c] font-semibold cursor-pointer hover:underline"
                              >
                                存
                              </button>
                              <button
                                onClick={() => setEditingQuotaId(null)}
                                className="text-[#8c8a82] cursor-pointer hover:underline"
                              >
                                取消
                              </button>
                            </>
                          ) : (
                            <>
                              <button
                                onClick={() => {
                                  setEditingQuotaId(p.id);
                                  setQuotaInput(
                                    quota > 0
                                      ? String(
                                          Math.round((quota / (1024 * 1024 * 1024)) * 100) / 100
                                        )
                                      : '10'
                                  );
                                }}
                                className="text-[#78766e] hover:text-[#191918] cursor-pointer"
                                title="设置本地流量额度 (GB)，填 0 清除"
                              >
                                设额度
                              </button>
                              <span className="text-[#d0cec5]">·</span>
                              <button
                                onClick={() => api.resetProfileTraffic(p.id).then(onRefresh)}
                                className="text-[#78766e] hover:text-[#e84c1e] cursor-pointer"
                                title="清零该配置的本地累计流量"
                              >
                                清零
                              </button>
                            </>
                          )}
                        </div>
                      </div>
                      {quota > 0 && (
                        <div className="w-full h-[4px] rounded-full bg-[#e2e0d7] overflow-hidden">
                          <div
                            className="h-full rounded-full transition-all"
                            style={{
                              width: `${pct}%`,
                              backgroundColor: isHigh ? '#e84c1e' : '#1f8a4c',
                            }}
                          />
                        </div>
                      )}
                    </div>
                  );
                })()}
                <div className="pt-1.5 border-t border-[#e2e0d7] flex items-center justify-between gap-1.5 text-[10.5px]">
                  <span className="font-mono text-[#8c8a82]">{p.updated_at || '未同步'}</span>
                  <div className="flex items-center gap-1.5">
                    <button
                      onClick={() => api.activateProfile(p.id).then(onRefresh)}
                      className={`${
                        isAct ? 'hw-btn-primary' : 'hw-micro-btn text-[#191918]'
                      } px-2 py-0.5 rounded font-semibold cursor-pointer`}
                    >
                      {isAct ? '当前使用' : '切换'}
                    </button>
                    <button
                      onClick={() =>
                        api.setChainExitPool(isChainPool ? null : p.id).then(onRefresh)
                      }
                      className={`${
                        isChainPool
                          ? 'bg-[#e84c1e] text-white'
                          : 'hw-micro-btn text-[#54524c]'
                      } px-2 py-0.5 rounded font-semibold cursor-pointer`}
                      title="将该配置的全部节点作为跨配置链式代理落地池（429 自动轮换）"
                    >
                      {isChainPool ? '★ 落地池' : '设落地池'}
                    </button>
                    {!isPool && (
                      <button
                        onClick={() => api.refreshProfile(p.id).then(onRefresh)}
                        className="hw-micro-btn px-2 py-0.5 rounded text-[#54524c] cursor-pointer"
                      >
                        更新
                      </button>
                    )}
                    <button
                      onClick={() => api.deleteProfile(p.id).then(onRefresh)}
                      className="hw-micro-btn px-2 py-0.5 rounded text-[#e84c1e] cursor-pointer"
                    >
                      删除
                    </button>
                  </div>
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
};

export const StrategyPage: React.FC<{ data: FullStateDto; onRefresh: () => void }> = ({
  data,
  onRefresh,
}) => {
  const [cooldown, setCooldown] = useState(String(data.config.cooldown_seconds));
  const [retries, setRetries] = useState(String(data.config.max_retries));
  const [backoff, setBackoff] = useState(String(data.config.chain_429_backoff_ms));
  const [domains, setDomains] = useState(data.config.watch_domains.join(', '));
  const [customExit, setCustomExit] = useState(
    data.config.chain_exit_source?.kind === 'custom_spec' ? data.config.chain_exit_source.raw : ''
  );

  useEffect(() => {
    setCooldown(String(data.config.cooldown_seconds));
    setRetries(String(data.config.max_retries));
    setBackoff(String(data.config.chain_429_backoff_ms));
    setDomains(data.config.watch_domains.join(', '));
  }, [data.config.cooldown_seconds, data.config.max_retries, data.config.chain_429_backoff_ms]);

  return (
    <div className="w-full h-full flex flex-col gap-3">
      <div className="ceramic-panel rounded-[12px] px-4 py-2.5 flex items-center justify-between shrink-0">
        <div className="flex items-baseline gap-2">
          <span className="text-[15px] font-bold text-[#191918]">429 策略与 MITM 证书</span>
          <span className="text-[9.5px] font-mono font-semibold tracking-[0.14em] uppercase text-[#8a877d]">
            FAILOVER & CA
          </span>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => api.installCa().then(onRefresh)}
            className="hw-micro-btn h-[30px] px-3 rounded-[6px] text-xs font-semibold text-[#191918] cursor-pointer"
          >
            {data.ca_installed ? '重新安装 CA' : '安装根证书 CA'}
          </button>
          <button
            onClick={() =>
              api
                .saveStrategy({
                  cooldownSeconds: parseInt(cooldown, 10) || 60,
                  maxRetries: parseInt(retries, 10) || 2,
                  autoRetryOn429: data.config.auto_retry_on_429,
                  watchDomains: domains
                    .split(/[,\n;]+/)
                    .map((s) => s.trim())
                    .filter(Boolean),
                  chainBackoffMs: parseInt(backoff, 10) || 1500,
                })
                .then(onRefresh)
            }
            className="hw-btn-primary h-[30px] px-3.5 rounded-[6px] text-xs font-semibold cursor-pointer"
          >
            保存策略
          </button>
        </div>
      </div>

      <div className="ceramic-panel flex-1 min-h-0 rounded-[12px] p-4 flex flex-col gap-3.5 overflow-y-auto nodes-scroll text-xs">
        <div className="grid grid-cols-3 gap-3">
          <div>
            <div className="text-[11px] text-[#78766e] mb-1">429 冷却时长 (秒)</div>
            <input
              type="number"
              value={cooldown}
              onChange={(e) => setCooldown(e.target.value)}
              className="hw-readout-bay w-full h-[32px] px-2.5 rounded-[6px] font-mono text-[#191918] outline-none"
            />
          </div>
          <div>
            <div className="text-[11px] text-[#78766e] mb-1">最大透明重试次数</div>
            <input
              type="number"
              value={retries}
              onChange={(e) => setRetries(e.target.value)}
              className="hw-readout-bay w-full h-[32px] px-2.5 rounded-[6px] font-mono text-[#191918] outline-none"
            />
          </div>
          <div>
            <div className="text-[11px] text-[#78766e] mb-1">链式 429 退避 (ms)</div>
            <input
              type="number"
              value={backoff}
              onChange={(e) => setBackoff(e.target.value)}
              className="hw-readout-bay w-full h-[32px] px-2.5 rounded-[6px] font-mono text-[#191918] outline-none"
            />
          </div>
        </div>

        <div>
          <div className="text-[11px] text-[#78766e] mb-1">MITM 监听域名（逗号或换行分隔）</div>
          <textarea
            rows={4}
            value={domains}
            onChange={(e) => setDomains(e.target.value)}
            className="hw-readout-bay w-full p-2.5 rounded-[6px] font-mono text-[#191918] outline-none resize-none"
          />
        </div>

        <div className="pt-2 border-t border-[#e0ded6] flex items-center gap-2">
          <input
            value={customExit}
            onChange={(e) => setCustomExit(e.target.value)}
            placeholder="自定义落地节点: socks5://user:pass@host:port 或单条 Clash YAML"
            className="hw-readout-bay flex-1 h-[32px] px-2.5 rounded-[6px] font-mono text-[#191918] outline-none"
          />
          <button
            onClick={() =>
              customExit.trim() &&
              api.applyCustomChainExit(customExit.trim(), parseInt(backoff, 10) || 1500).then(onRefresh)
            }
            className="hw-micro-btn h-[32px] px-3 rounded-[6px] font-semibold text-[#191918] cursor-pointer"
          >
            应用落地
          </button>
          <button
            onClick={() => api.setChainExitNode(null).then(onRefresh)}
            className="hw-micro-btn h-[32px] px-3 rounded-[6px] text-[#78766e] cursor-pointer"
          >
            清除落地
          </button>
        </div>
      </div>
    </div>
  );
};

export const LogsPage: React.FC<{ data: FullStateDto }> = ({ data }) => (
  <div className="w-full h-full flex flex-col gap-3">
    <div className="ceramic-panel rounded-[12px] px-4 py-2.5 flex items-center justify-between shrink-0">
      <div className="flex items-baseline gap-2">
        <span className="text-[15px] font-bold text-[#191918]">连接日志</span>
        <span className="text-[9.5px] font-mono font-semibold tracking-[0.14em] uppercase text-[#8a877d]">
          TELEMETRY TAPE
        </span>
      </div>
      <span className="text-[11px] font-mono text-[#78766e] tabular-nums">{data.logs.length} 条记录</span>
    </div>

    <div className="flex-1 min-h-0 rounded-[12px] bg-[#161715] border border-[#2f312d] p-3 overflow-y-auto circular-scroll font-mono text-[11px] space-y-1 text-[#dcdad2]">
      {[...data.logs].reverse().map((l, idx) => {
        const col =
          l.level === 'ERR' || l.level === '429'
            ? 'text-[#ff6a3d]'
            : l.level === 'OK'
            ? 'text-[#5bd68a]'
            : 'text-[#c8c6be]';
        return (
          <div key={idx} className="flex gap-2">
            <span className="text-[#78766e] shrink-0">[{l.timestamp}]</span>
            <span className={`${col} shrink-0 w-9`}>[{l.level}]</span>
            <span className="break-all">{l.message}</span>
          </div>
        );
      })}
    </div>
  </div>
);

export const SettingsPage: React.FC<{ data: FullStateDto; onRefresh: () => void }> = ({
  data,
  onRefresh,
}) => {
  const [localPort, setLocalPort] = useState(String(data.config.local_proxy_port));
  const [mixedPort, setMixedPort] = useState(String(data.config.mihomo_mixed_port));
  const [tunPort, setTunPort] = useState(String(data.config.tun_mitm_port));
  const [bypassPort, setBypassPort] = useState(String(data.config.mihomo_bypass_port));
  const [apiPort, setApiPort] = useState(String(data.config.mihomo_api_port));
  const [maxMb, setMaxMb] = useState(
    String(Math.max(1, Math.round(data.config.max_retry_body_bytes / 1048576)))
  );
  const [mihomoPath, setMihomoPath] = useState(data.config.mihomo_path || '');

  return (
    <div className="w-full h-full flex flex-col gap-3">
      <div className="ceramic-panel rounded-[12px] px-4 py-2.5 flex items-center justify-between shrink-0">
        <div className="flex items-baseline gap-2">
          <span className="text-[15px] font-bold text-[#191918]">系统设置</span>
          <span className="text-[9.5px] font-mono font-semibold tracking-[0.14em] uppercase text-[#8a877d]">
            PORTS & KERNEL
          </span>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => api.downloadCore().then(onRefresh)}
            className="hw-micro-btn h-[30px] px-3 rounded-[6px] text-xs font-semibold text-[#191918] cursor-pointer"
          >
            下载 Mihomo 内核
          </button>
          <button
            onClick={() =>
              api
                .saveSystemSettings({
                  localProxyPort: parseInt(localPort, 10) || 7890,
                  allowLan: data.config.allow_lan,
                  mihomoMixedPort: parseInt(mixedPort, 10) || 17890,
                  tunMitmPort: parseInt(tunPort, 10) || 17891,
                  mihomoBypassPort: parseInt(bypassPort, 10) || 17892,
                  mihomoApiPort: parseInt(apiPort, 10) || 19090,
                  maxBodyMb: parseInt(maxMb, 10) || 16,
                  mihomoPath: mihomoPath.trim() || null,
                })
                .then(onRefresh)
            }
            className="hw-btn-primary h-[30px] px-3.5 rounded-[6px] text-xs font-semibold cursor-pointer"
          >
            保存设置
          </button>
        </div>
      </div>

      <div className="ceramic-panel flex-1 min-h-0 rounded-[12px] p-4 grid grid-cols-3 gap-3 content-start text-xs">
        {[
          ['本地代理端口', localPort, setLocalPort],
          ['Mihomo Mixed 端口', mixedPort, setMixedPort],
          ['TUN MITM 端口', tunPort, setTunPort],
          ['Mihomo Bypass 端口', bypassPort, setBypassPort],
          ['Mihomo API 端口', apiPort, setApiPort],
          ['重试最大 Body (MB)', maxMb, setMaxMb],
        ].map(([label, val, setter]) => (
          <div key={label as string}>
            <div className="text-[11px] text-[#78766e] mb-1">{label as string}</div>
            <input
              type="number"
              value={val as string}
              onChange={(e) => (setter as React.Dispatch<React.SetStateAction<string>>)(e.target.value)}
              className="hw-readout-bay w-full h-[32px] px-2.5 rounded-[6px] font-mono outline-none"
            />
          </div>
        ))}
        <div className="col-span-3">
          <div className="text-[11px] text-[#78766e] mb-1">
            自定义 Mihomo 内核路径（留空使用内置 bin\mihomo.exe）
          </div>
          <input
            value={mihomoPath}
            onChange={(e) => setMihomoPath(e.target.value)}
            placeholder="C:\path\to\mihomo.exe"
            className="hw-readout-bay w-full h-[32px] px-2.5 rounded-[6px] font-mono outline-none"
          />
        </div>
      </div>
    </div>
  );
};
