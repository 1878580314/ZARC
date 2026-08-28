import { common } from './common';
import { compress } from './compress';
import { decompress } from './decompress';
import { benchmark } from './benchmark';
import { shell } from './shell';
import { ui } from './ui';
import { stores } from './stores';

export type DictTable = Record<string, string>;

export interface DictSlice {
  zh: DictTable;
  en: DictTable;
}

function merge(...slices: DictSlice[]): Record<'zh' | 'en', DictTable> {
  return {
    zh: Object.assign({}, ...slices.map((s) => s.zh)),
    en: Object.assign({}, ...slices.map((s) => s.en))
  };
}

export const DICT = merge(common, compress, decompress, benchmark, shell, ui, stores);
