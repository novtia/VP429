/// <reference types="vite/client" />
import { geoGraticule, geoMercator, geoPath } from 'd3-geo';
import * as topojson from 'topojson-client';
import type { Topology, GeometryCollection, Polygon, MultiPolygon } from 'topojson-specification';
import worldAtlasRaw from 'world-atlas/countries-110m.json';

export const MAP_W = 978;
export const MAP_H = 612;

export interface RegionMeta {
  code: string;
  name: string;
  isoNum: string | null;
  lonLat: [number, number] | null;
  tagOffset?: { dx: number; dy: number };
}

export const REGION_META: Record<string, RegionMeta> = {
  HK: { code: 'HK', name: '香港', isoNum: '344', lonLat: [114.17, 22.32], tagOffset: { dx: -74, dy: 14 } },
  MO: { code: 'MO', name: '澳门', isoNum: '446', lonLat: [113.54, 22.2], tagOffset: { dx: -74, dy: -12 } },
  TW: { code: 'TW', name: '台湾', isoNum: '158', lonLat: [121.56, 25.03], tagOffset: { dx: 11, dy: 7 } },
  JP: { code: 'JP', name: '日本', isoNum: '392', lonLat: [139.69, 35.68], tagOffset: { dx: 12, dy: -10 } },
  KR: { code: 'KR', name: '韩国', isoNum: '410', lonLat: [126.98, 37.57], tagOffset: { dx: -72, dy: -14 } },
  SG: { code: 'SG', name: '新加坡', isoNum: '702', lonLat: [103.82, 1.35], tagOffset: { dx: -74, dy: 12 } },
  CN: { code: 'CN', name: '中国', isoNum: '156', lonLat: [104.2, 35.86], tagOffset: { dx: -70, dy: -12 } },
  US: { code: 'US', name: '美国', isoNum: '840', lonLat: [-100.4, 39.8], tagOffset: { dx: 12, dy: -8 } },
  CA: { code: 'CA', name: '加拿大', isoNum: '124', lonLat: [-106.35, 56.13], tagOffset: { dx: 12, dy: -8 } },
  GB: { code: 'GB', name: '英国', isoNum: '826', lonLat: [-0.13, 51.51], tagOffset: { dx: 11, dy: -12 } },
  DE: { code: 'DE', name: '德国', isoNum: '276', lonLat: [10.45, 51.16], tagOffset: { dx: 11, dy: 9 } },
  FR: { code: 'FR', name: '法国', isoNum: '250', lonLat: [2.21, 46.23], tagOffset: { dx: -68, dy: 10 } },
  NL: { code: 'NL', name: '荷兰', isoNum: '528', lonLat: [5.29, 52.13], tagOffset: { dx: 11, dy: -12 } },
  RU: { code: 'RU', name: '俄罗斯', isoNum: '643', lonLat: [94.25, 61.52], tagOffset: { dx: 12, dy: -8 } },
  AU: { code: 'AU', name: '澳大利亚', isoNum: '036', lonLat: [133.77, -25.27], tagOffset: { dx: 12, dy: 6 } },
  NZ: { code: 'NZ', name: '新西兰', isoNum: '554', lonLat: [174.88, -40.9], tagOffset: { dx: 12, dy: 6 } },
  IN: { code: 'IN', name: '印度', isoNum: '356', lonLat: [78.96, 20.59] },
  MY: { code: 'MY', name: '马来西亚', isoNum: '458', lonLat: [101.97, 4.21] },
  TH: { code: 'TH', name: '泰国', isoNum: '764', lonLat: [100.99, 15.87] },
  VN: { code: 'VN', name: '越南', isoNum: '704', lonLat: [108.27, 14.05] },
  PH: { code: 'PH', name: '菲律宾', isoNum: '608', lonLat: [121.77, 12.88] },
  ID: { code: 'ID', name: '印尼', isoNum: '360', lonLat: [113.92, -0.79] },
  TR: { code: 'TR', name: '土耳其', isoNum: '792', lonLat: [35.24, 38.96] },
  BR: { code: 'BR', name: '巴西', isoNum: '076', lonLat: [-51.92, -14.23] },
  AR: { code: 'AR', name: '阿根廷', isoNum: '032', lonLat: [-63.61, -38.41] },
  MX: { code: 'MX', name: '墨西哥', isoNum: '484', lonLat: [-102.55, 23.63] },
  CL: { code: 'CL', name: '智利', isoNum: '152', lonLat: [-71.54, -35.67] },
  CO: { code: 'CO', name: '哥伦比亚', isoNum: '170', lonLat: [-74.29, 4.57] },
  PE: { code: 'PE', name: '秘鲁', isoNum: '604', lonLat: [-75.01, -9.19] },
  SE: { code: 'SE', name: '瑞典', isoNum: '752', lonLat: [18.64, 60.12] },
  NO: { code: 'NO', name: '挪威', isoNum: '578', lonLat: [8.46, 60.47] },
  FI: { code: 'FI', name: '芬兰', isoNum: '246', lonLat: [25.74, 61.92] },
  DK: { code: 'DK', name: '丹麦', isoNum: '208', lonLat: [9.5, 56.26] },
  IS: { code: 'IS', name: '冰岛', isoNum: '352', lonLat: [-19.02, 64.96] },
  CH: { code: 'CH', name: '瑞士', isoNum: '756', lonLat: [8.22, 46.81] },
  AT: { code: 'AT', name: '奥地利', isoNum: '040', lonLat: [14.55, 47.51] },
  BE: { code: 'BE', name: '比利时', isoNum: '056', lonLat: [4.47, 50.5] },
  ES: { code: 'ES', name: '西班牙', isoNum: '724', lonLat: [-3.75, 40.46] },
  PT: { code: 'PT', name: '葡萄牙', isoNum: '620', lonLat: [-8.22, 39.39] },
  IT: { code: 'IT', name: '意大利', isoNum: '380', lonLat: [12.56, 41.87] },
  PL: { code: 'PL', name: '波兰', isoNum: '616', lonLat: [19.14, 51.91] },
  CZ: { code: 'CZ', name: '捷克', isoNum: '203', lonLat: [15.47, 49.81] },
  IE: { code: 'IE', name: '爱尔兰', isoNum: '372', lonLat: [-8.24, 53.41] },
  LU: { code: 'LU', name: '卢森堡', isoNum: '442', lonLat: [6.13, 49.81] },
  UA: { code: 'UA', name: '乌克兰', isoNum: '804', lonLat: [31.16, 48.37] },
  RO: { code: 'RO', name: '罗马尼亚', isoNum: '642', lonLat: [24.96, 45.94] },
  HU: { code: 'HU', name: '匈牙利', isoNum: '348', lonLat: [19.5, 47.16] },
  GR: { code: 'GR', name: '希腊', isoNum: '300', lonLat: [21.82, 39.07] },
  BG: { code: 'BG', name: '保加利亚', isoNum: '100', lonLat: [25.48, 42.73] },
  HR: { code: 'HR', name: '克罗地亚', isoNum: '191', lonLat: [15.2, 45.1] },
  RS: { code: 'RS', name: '塞尔维亚', isoNum: '688', lonLat: [21.0, 44.01] },
  SK: { code: 'SK', name: '斯洛伐克', isoNum: '703', lonLat: [19.69, 48.66] },
  SI: { code: 'SI', name: '斯洛文尼亚', isoNum: '705', lonLat: [14.99, 46.15] },
  LT: { code: 'LT', name: '立陶宛', isoNum: '440', lonLat: [23.88, 55.16] },
  LV: { code: 'LV', name: '拉脱维亚', isoNum: '428', lonLat: [24.6, 56.87] },
  EE: { code: 'EE', name: '爱沙尼亚', isoNum: '233', lonLat: [25.01, 58.59] },
  IL: { code: 'IL', name: '以色列', isoNum: '376', lonLat: [34.85, 31.04] },
  AE: { code: 'AE', name: '阿联酋', isoNum: '784', lonLat: [53.84, 23.42] },
  SA: { code: 'SA', name: '沙特', isoNum: '682', lonLat: [45.07, 23.88] },
  QA: { code: 'QA', name: '卡塔尔', isoNum: '634', lonLat: [51.18, 25.35] },
  ZA: { code: 'ZA', name: '南非', isoNum: '710', lonLat: [22.93, -30.55] },
  EG: { code: 'EG', name: '埃及', isoNum: '818', lonLat: [30.8, 26.82] },
  NG: { code: 'NG', name: '尼日利亚', isoNum: '566', lonLat: [8.67, 9.08] },
  KE: { code: 'KE', name: '肯尼亚', isoNum: '404', lonLat: [37.9, -0.02] },
  PK: { code: 'PK', name: '巴基斯坦', isoNum: '586', lonLat: [69.34, 30.37] },
  BD: { code: 'BD', name: '孟加拉', isoNum: '050', lonLat: [90.35, 23.68] },
  KZ: { code: 'KZ', name: '哈萨克', isoNum: '398', lonLat: [66.92, 48.01] },
  MN: { code: 'MN', name: '蒙古', isoNum: '496', lonLat: [103.84, 46.86] },
  KH: { code: 'KH', name: '柬埔寨', isoNum: '116', lonLat: [104.99, 12.56] },
  MM: { code: 'MM', name: '缅甸', isoNum: '104', lonLat: [95.95, 21.91] },
  LA: { code: 'LA', name: '老挝', isoNum: '418', lonLat: [102.49, 19.85] },
  NP: { code: 'NP', name: '尼泊尔', isoNum: '524', lonLat: [84.12, 28.39] },
  LK: { code: 'LK', name: '斯里兰卡', isoNum: '144', lonLat: [80.77, 7.87] },
  GL: { code: 'GL', name: '其他 / 未知', isoNum: null, lonLat: null },
};

export const ISO_NUM_TO_CODE: Record<string, string> = {};
Object.values(REGION_META).forEach((m) => {
  if (m.isoNum) ISO_NUM_TO_CODE[m.isoNum] = m.code;
});

const projection = geoMercator()
  .rotate([-150, 0])
  .scale(158)
  .translate([MAP_W / 2, 362]);

const pathGen = geoPath(projection);
const roundD = (d: string | null | undefined) =>
  d ? d.replace(/-?\d+\.\d+/g, (n) => Number(n).toFixed(1)) : '';

type CountriesTopology = Topology<{ countries: GeometryCollection }>;
const topo = worldAtlasRaw as unknown as CountriesTopology;

const graticuleD = roundD(
  pathGen(
    geoGraticule()
      .step([30, 20])
      .extent([
        [-180, -64],
        [180, 82],
      ])()
  )
);

const equatorD = roundD(
  pathGen({
    type: 'LineString',
    coordinates: [
      [-180, 0],
      [0, 0],
      [180, 0],
    ],
  })
);

const landGeoms = topo.objects.countries.geometries.filter(
  (g): g is Polygon | MultiPolygon =>
    String(g.id) !== '010' && (g.type === 'Polygon' || g.type === 'MultiPolygon')
);
const mergedLand = topojson.merge(topo, landGeoms);
const landD = roundD(pathGen(mergedLand));

const countryFeatures = topojson
  .feature(topo, topo.objects.countries)
  .features.filter((f) => String(f.id) !== '010');

export interface CountryPlate {
  id: string;
  code: string | null;
  d: string;
}

const countries: CountryPlate[] = countryFeatures
  .map((f) => {
    const id = String(f.id).padStart(3, '0');
    return {
      id,
      code: ISO_NUM_TO_CODE[id] ?? null,
      d: roundD(pathGen(f)),
    };
  })
  .filter((c) => Boolean(c.d));

const bordersD = roundD(
  pathGen(
    topojson.mesh(
      topo,
      topo.objects.countries,
      (a, b) => a !== b && String(a.id) !== '010' && String(b.id) !== '010'
    )
  )
);

export const WORLD_ATLAS = {
  graticuleD,
  equatorD,
  landD,
  bordersD,
  countries,
};

export function projectRegion(code: string): [number, number] | null {
  const meta = REGION_META[code];
  if (!meta?.lonLat) return null;
  const pt = projection(meta.lonLat);
  if (!pt) return null;
  return [Number(pt[0].toFixed(1)), Number(pt[1].toFixed(1))];
}

export function buildGeodesicArc(fromCode: string, toCode: string): string {
  if (!fromCode || !toCode || fromCode === toCode) return '';
  const a = REGION_META[fromCode]?.lonLat;
  const b = REGION_META[toCode]?.lonLat;
  if (!a || !b) return '';
  return roundD(
    pathGen({
      type: 'LineString',
      coordinates: [a, b],
    })
  );
}

// Vite eagerly resolves 4x3 SVG flag asset URLs for all countries
const flagModules = import.meta.glob<string>('/node_modules/flag-icons/flags/4x3/*.svg', {
  eager: true,
  import: 'default',
});

const FLAG_URL_MAP: Record<string, string> = {};
for (const [filePath, url] of Object.entries(flagModules)) {
  const m = filePath.match(/\/([a-z0-9-]+)\.svg$/i);
  if (m) FLAG_URL_MAP[m[1].toUpperCase()] = url;
}

export function getFlagUrl(code: string): string | null {
  if (!code || code === 'GL') return null;
  const norm = code.toUpperCase() === 'UK' ? 'GB' : code.toUpperCase();
  return FLAG_URL_MAP[norm] ?? null;
}
