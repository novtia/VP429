import React from 'react';
import { Globe } from 'lucide-react';

interface FlagIconProps {
  /** ISO 3166-1 alpha-2 code (uppercase or lowercase), or 'GL' for global fallback */
  code: string;
  size?: 'xs' | 'sm' | 'md';
  className?: string;
}

const SIZE_MAP = {
  xs: 'w-[13px] h-[9.5px]',
  sm: 'w-[16px] h-[12px]',
  md: 'w-[18px] h-[13.5px]',
};

export const FlagIcon: React.FC<FlagIconProps> = ({ code, size = 'sm', className = '' }) => {
  const norm = (code || '').trim().toLowerCase();
  const iso = norm === 'uk' ? 'gb' : norm;

  if (!iso || iso === 'gl' || iso.length !== 2) {
    const px = size === 'xs' ? 11 : size === 'sm' ? 13 : 15;
    return <Globe size={px} className={`text-[#78766e] shrink-0 ${className}`} />;
  }

  return (
    <span
      className={`fi fi-${iso} fis-off inline-block rounded-[2px] shadow-[0_0_0_0.5px_rgba(0,0,0,0.18)] shrink-0 bg-cover bg-center ${SIZE_MAP[size]} ${className}`}
      aria-label={iso.toUpperCase()}
    />
  );
};
