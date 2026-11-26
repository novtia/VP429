export type NavPage = 'dashboard' | 'nodes' | 'profiles' | 'strategy' | 'logs' | 'settings';

export type OutboundMode = 'rule' | 'global' | 'direct';
export type RotationKey = 'on429' | 'req' | 'int';

export interface NodeDto {
  name: string;
  proxy_type: string;
  server: string;
  port: number;
  latency_ms: number | null;
  cooldown_remaining_secs: number;
  hit_429_count: number;
  success_count: number;
  is_active: boolean;
}

export interface LogDto {
  timestamp: string;
  level: 'INFO' | '429' | 'OK' | 'ERR';
  message: string;
}

export interface TelemetryDto {
  up_bps: number;
  down_bps: number;
  total_up: number;
  total_down: number;
  speed_history: [number, number][];
  uptime_secs: number;
  local_ip: string;
  exit_ip: string | null;
  exit_country_code: string | null;
  checking_ip: boolean;
  outbound_mode: string;
}

export interface SubscriptionProfile {
  id: string;
  name: string;
  url: string;
  node_count: number;
  updated_at: string;
  updated_unix: number;
  used_bytes: number;
  total_bytes: number;
  expire_unix: number;
}

export type ChainExitSource =
  | { kind: 'subscription_node'; name: string }
  | { kind: 'custom_spec'; raw: string };

export interface AppConfigDto {
  subscription_url: string;
  profiles: SubscriptionProfile[];
  active_profile_id: string | null;
  mihomo_path: string | null;
  local_proxy_port: number;
  allow_lan: boolean;
  mihomo_mixed_port: number;
  tun_mitm_port: number;
  mihomo_bypass_port: number;
  mihomo_api_port: number;
  enable_system_proxy: boolean;
  enable_tun: boolean;
  enable_chain_proxy: boolean;
  chain_exit_source: ChainExitSource | null;
  chain_429_backoff_ms: number;
  rotation_mode: 'On429Only' | 'RoundRobinPerRequest' | { RoundRobinInterval: { seconds: number } };
  cooldown_seconds: number;
  auto_retry_on_429: boolean;
  max_retries: number;
  max_retry_body_bytes: number;
  watch_domains: string[];
  intercept_status_codes: number[];
  selected_node: string | null;
  outbound_mode: string;
  sidebar_collapsed: boolean;
}

export interface FullStateDto {
  running: boolean;
  testing_latency: boolean;
  ca_installed: boolean;
  elevated: boolean;
  active_node: string | null;
  nodes: NodeDto[];
  total_429: number;
  total_retries_ok: number;
  ready_count: number;
  cooldown_count: number;
  telemetry: TelemetryDto;
  config: AppConfigDto;
  logs: LogDto[];
}

/** Strip Unicode regional-indicator flag emojis so Windows WebView2 doesn't render duplicate "KR"/"HK" letters */
export function cleanNodeName(name: string): string {
  return (name || '')
    .replace(/[\u{1F1E6}-\u{1F1FF}]{2}/gu, '')
    .replace(/\s{2,}/g, ' ')
    .trim();
}

/** Extract ISO 3166-1 alpha-2 code directly from any Unicode flag emoji pair (covers all ~250 countries) */
function extractEmojiCountryCode(text: string): string | null {
  const m = text.match(/[\u{1F1E6}-\u{1F1FF}]{2}/u);
  if (!m) return null;
  const pts = Array.from(m[0]).map((ch) => (ch.codePointAt(0) ?? 0) - 0x1f1e6 + 65);
  if (pts.length === 2 && pts.every((c) => c >= 65 && c <= 90)) {
    return String.fromCharCode(pts[0], pts[1]);
  }
  return null;
}

const COUNTRY_RULES: [string[], string][] = [
  [['香港', 'HONG KONG', 'HONGKONG', 'HKG'], 'HK'],
  [['台湾', '台灣', 'TAIWAN', 'TAIPEI', 'TWN'], 'TW'],
  [['澳门', '澳門', 'MACAU', 'MACAO'], 'MO'],
  [['日本', 'JAPAN', 'TOKYO', 'OSAKA', 'SAITAMA', 'JPN'], 'JP'],
  [['新加坡', '狮城', '獅城', 'SINGAPORE', 'SGP'], 'SG'],
  [['韩国', '韓國', 'KOREA', 'SEOUL', 'CHUNCHEON', 'KOR'], 'KR'],
  [['美国', '美國', 'UNITED STATES', 'USA', 'SAN JOSE', 'LOS ANGELES', 'SEATTLE', 'SILICON', 'DALLAS', 'CHICAGO', 'NEW YORK', 'MIAMI', 'PHOENIX', 'ASHBURN'], 'US'],
  [['英国', '英國', 'UNITED KINGDOM', 'LONDON', 'BRITAIN', 'GBR'], 'GB'],
  [['德国', '德國', 'GERMANY', 'FRANKFURT', 'BERLIN', 'MUNICH', 'DEU'], 'DE'],
  [['法国', '法國', 'FRANCE', 'PARIS', 'MARSEILLE', 'FRA'], 'FR'],
  [['荷兰', '荷蘭', 'NETHERLANDS', 'AMSTERDAM', 'NLD'], 'NL'],
  [['俄罗斯', '俄羅斯', '俄国', 'RUSSIA', 'MOSCOW', 'SIBERIA', 'RUS'], 'RU'],
  [['加拿大', 'CANADA', 'TORONTO', 'VANCOUVER', 'MONTREAL', 'CAN'], 'CA'],
  [['澳大利亚', '澳洲', 'AUSTRALIA', 'SYDNEY', 'MELBOURNE', 'AUS'], 'AU'],
  [['新西兰', '紐西蘭', 'NEW ZEALAND', 'AUCKLAND', 'NZL'], 'NZ'],
  [['印度', 'INDIA', 'MUMBAI', 'DELHI', 'BANGALORE', 'IND'], 'IN'],
  [['马来西亚', '馬來西亞', '马来', 'MALAYSIA', 'KUALA LUMPUR', 'MYS'], 'MY'],
  [['泰国', '泰國', 'THAILAND', 'BANGKOK', 'THA'], 'TH'],
  [['越南', 'VIETNAM', 'HO CHI MINH', 'HANOI', 'VNM'], 'VN'],
  [['菲律宾', '菲律賓', 'PHILIPPINES', 'MANILA', 'PHL'], 'PH'],
  [['印尼', '印度尼西亚', 'INDONESIA', 'JAKARTA', 'IDN'], 'ID'],
  [['土耳其', 'TURKEY', 'TURKIYE', 'ISTANBUL', 'TUR'], 'TR'],
  [['巴西', 'BRAZIL', 'SAO PAULO', 'BRA'], 'BR'],
  [['阿根廷', 'ARGENTINA', 'BUENOS AIRES', 'ARG'], 'AR'],
  [['墨西哥', 'MEXICO', 'MEX'], 'MX'],
  [['智利', 'CHILE', 'SANTIAGO', 'CHL'], 'CL'],
  [['哥伦比亚', 'COLOMBIA', 'BOGOTA', 'COL'], 'CO'],
  [['秘鲁', 'PERU', 'LIMA', 'PER'], 'PE'],
  [['瑞典', 'SWEDEN', 'STOCKHOLM', 'SWE'], 'SE'],
  [['挪威', 'NORWAY', 'OSLO', 'NOR'], 'NO'],
  [['芬兰', '芬蘭', 'FINLAND', 'HELSINKI', 'FIN'], 'FI'],
  [['丹麦', '丹麥', 'DENMARK', 'COPENHAGEN', 'DNK'], 'DK'],
  [['冰岛', '冰島', 'ICELAND', 'REYKJAVIK', 'ISL'], 'IS'],
  [['瑞士', 'SWITZERLAND', 'ZURICH', 'GENEVA', 'CHE'], 'CH'],
  [['奥地利', '奧地利', 'AUSTRIA', 'VIENNA', 'AUT'], 'AT'],
  [['比利时', '比利時', 'BELGIUM', 'BRUSSELS', 'BEL'], 'BE'],
  [['西班牙', 'SPAIN', 'MADRID', 'BARCELONA', 'ESP'], 'ES'],
  [['葡萄牙', 'PORTUGAL', 'LISBON', 'PRT'], 'PT'],
  [['意大利', '義大利', 'ITALY', 'MILAN', 'ROME', 'ITA'], 'IT'],
  [['波兰', '波蘭', 'POLAND', 'WARSAW', 'POL'], 'PL'],
  [['捷克', 'CZECH', 'PRAGUE', 'CZE'], 'CZ'],
  [['爱尔兰', '愛爾蘭', 'IRELAND', 'DUBLIN', 'IRL'], 'IE'],
  [['卢森堡', 'LUXEMBOURG', 'LUX'], 'LU'],
  [['乌克兰', '烏克蘭', 'UKRAINE', 'KYIV', 'UKR'], 'UA'],
  [['罗马尼亚', 'ROMANIA', 'BUCHAREST', 'ROU'], 'RO'],
  [['匈牙利', 'HUNGARY', 'BUDAPEST', 'HUN'], 'HU'],
  [['希腊', '希臘', 'GREECE', 'ATHENS', 'GRC'], 'GR'],
  [['保加利亚', 'BULGARIA', 'SOFIA', 'BGR'], 'BG'],
  [['克罗地亚', 'CROATIA', 'ZAGREB', 'HRV'], 'HR'],
  [['塞尔维亚', 'SERBIA', 'BELGRADE', 'SRB'], 'RS'],
  [['斯洛伐克', 'SLOVAKIA', 'SVK'], 'SK'],
  [['斯洛文尼亚', 'SLOVENIA', 'SVN'], 'SI'],
  [['立陶宛', 'LITHUANIA', 'LTU'], 'LT'],
  [['拉脱维亚', 'LATVIA', 'LVA'], 'LV'],
  [['爱沙尼亚', 'ESTONIA', 'EST'], 'EE'],
  [['以色列', 'ISRAEL', 'TEL AVIV', 'ISR'], 'IL'],
  [['阿联酋', '迪拜', 'DUBAI', 'UAE', 'ARE'], 'AE'],
  [['沙特', 'SAUDI', 'RIYADH', 'SAU'], 'SA'],
  [['卡塔尔', 'QATAR', 'DOHA', 'QAT'], 'QA'],
  [['南非', 'SOUTH AFRICA', 'JOHANNESBURG', 'ZAF'], 'ZA'],
  [['埃及', 'EGYPT', 'CAIRO', 'EGY'], 'EG'],
  [['尼日利亚', 'NIGERIA', 'LAGOS', 'NGA'], 'NG'],
  [['肯尼亚', 'KENYA', 'NAIROBI', 'KEN'], 'KE'],
  [['巴基斯坦', 'PAKISTAN', 'PAK'], 'PK'],
  [['孟加拉', 'BANGLADESH', 'BGD'], 'BD'],
  [['哈萨克', 'KAZAKHSTAN', 'KAZ'], 'KZ'],
  [['蒙古', 'MONGOLIA', 'MNG'], 'MN'],
  [['柬埔寨', 'CAMBODIA', 'KHM'], 'KH'],
  [['缅甸', 'MYANMAR', 'MMR'], 'MM'],
  [['老挝', 'LAOS', 'LAO'], 'LA'],
  [['尼泊尔', 'NEPAL', 'NPL'], 'NP'],
  [['斯里兰卡', 'SRI LANKA', 'LKA'], 'LK'],
  [['中国', '中國', 'CHINA', 'BEIJING', 'SHANGHAI', 'GUANGZHOU', 'SHENZHEN', 'CHN'], 'CN'],
];

const SHORT_ISO_SET = new Set([
  'HK', 'TW', 'MO', 'JP', 'SG', 'KR', 'US', 'GB', 'UK', 'DE', 'FR', 'NL', 'RU',
  'CA', 'AU', 'NZ', 'IN', 'MY', 'TH', 'VN', 'PH', 'ID', 'TR', 'BR', 'AR', 'MX',
  'CL', 'CO', 'PE', 'SE', 'NO', 'FI', 'DK', 'IS', 'CH', 'AT', 'BE', 'ES', 'PT',
  'IT', 'PL', 'CZ', 'IE', 'LU', 'UA', 'RO', 'HU', 'GR', 'BG', 'HR', 'RS', 'SK',
  'SI', 'LT', 'LV', 'EE', 'IL', 'AE', 'SA', 'QA', 'ZA', 'EG', 'NG', 'KE', 'PK',
  'BD', 'KZ', 'MN', 'KH', 'MM', 'LA', 'NP', 'LK', 'CN',
]);

export function detectCountry(name: string): { code: string; flag: string } {
  const raw = (name || '').trim();
  if (!raw) return { code: 'GL', flag: '' };

  // 1. Direct Unicode regional-indicator flag emoji (instant support for all ~250 countries)
  const emojiCode = extractEmojiCountryCode(raw);
  if (emojiCode) {
    return { code: emojiCode, flag: emojiCode };
  }

  // 2. Exact 2-letter ISO code input (e.g. exit_country_code "US", "JP", "RU")
  const u = raw.toUpperCase();
  if (/^[A-Z]{2}$/.test(u)) {
    const norm = u === 'UK' ? 'GB' : u;
    return { code: norm, flag: norm };
  }

  // 3. Full keyword match (Chinese, English, major cities, 3-letter ISO)
  for (const [keys, code] of COUNTRY_RULES) {
    if (keys.some((k) => u.includes(k))) return { code, flag: code };
  }

  // 4. Token-boundary 2-letter ISO match (avoids matching "US" inside "PLUS" or "CA" inside "SCALE")
  const tokens = u.split(/[^A-Z0-9]+/).filter(Boolean);
  for (const tok of tokens) {
    if (SHORT_ISO_SET.has(tok)) {
      const norm = tok === 'UK' ? 'GB' : tok;
      return { code: norm, flag: norm };
    }
  }

  return { code: 'GL', flag: '' };
}

export function formatRateParts(bps: number): { val: string; unit: string } {
  if (bps >= 1024 * 1024) {
    const mb = bps / (1024 * 1024);
    return { val: mb >= 10 ? mb.toFixed(1) : mb.toFixed(2), unit: 'MB/s' };
  }
  if (bps >= 1024) return { val: (bps / 1024).toFixed(1), unit: 'KB/s' };
  return { val: String(Math.round(bps)), unit: 'B/s' };
}

export function formatRateStr(bps: number): string {
  const p = formatRateParts(bps);
  return `${p.val} ${p.unit}`;
}

export function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / 1024).toFixed(1)} KB`;
}

export function formatUptime(secs: number): string {
  const h = String(Math.floor(secs / 3600)).padStart(2, '0');
  const m = String(Math.floor((secs % 3600) / 60)).padStart(2, '0');
  const s = String(secs % 60).padStart(2, '0');
  return `${h}:${m}:${s}`;
}

export function bpsToNorm(bps: number): number {
  if (bps <= 1024) return 0;
  return Math.max(0, Math.min(1, (Math.log10(Math.max(10, bps / 1024)) - 1) / 4));
}
