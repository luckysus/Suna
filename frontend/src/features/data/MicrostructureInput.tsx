import type { Gray16Mapping, MicrostructureRasterInput } from '../../bridge/desktop';
import { Input } from '../../components/ui/input';

export type MappingMode = 'unset' | 'full_range' | 'window';
export function gray16Mapping(mode:MappingMode, lower:string, upper:string):Gray16Mapping|undefined {
  if (mode === 'unset') return undefined;
  if (mode === 'full_range') return {mode};
  const lo = Number(lower), hi = Number(upper);
  if (!lower.trim() || !upper.trim() || !Number.isInteger(lo) || !Number.isInteger(hi) || lo < 0 || hi > 65535 || lo >= hi) {
    throw new Error('16 位灰度窗口须为 0–65535 内递增的两个整数');
  }
  return {mode, lower:lo, upper:hi};
}
export function MicrostructureInputOptions({mode,lower,upper,onMode,onLower,onUpper}:{mode:MappingMode;lower:string;upper:string;onMode:(value:MappingMode)=>void;onLower:(value:string)=>void;onUpper:(value:string)=>void}) {
  return <><label>16 位灰度映射<select value={mode} onChange={e=>onMode(e.target.value as MappingMode)}>
    <option value="unset">未设置（仅导入 8 位）</option><option value="full_range">完整量程 0–65535</option><option value="window">指定灰度窗口</option>
  </select></label>{mode==='window'&&<><label>灰度窗口下限<Input type="number" min="0" max="65534" value={lower} onChange={e=>onLower(e.target.value)}/></label><label>灰度窗口上限<Input type="number" min="1" max="65535" value={upper} onChange={e=>onUpper(e.target.value)}/></label></>}
  <p>此选项仅作用于 16 位灰度图。窗口外数值截断，窗口内线性映射到 0–255 并向下取整；原始像素不变。不自动拉伸对比度。</p></>;
}
export function MicrostructureInputSummary({input}:{input?:MicrostructureRasterInput}) {
  if (!input) return <p>历史图像：未记录输入处理版本，沿用原始栅格坐标。</p>;
  const mapping = input.grayscaleMapping;
  return <p>{input.format} · {input.decodedBitDepth} 位 · {input.colorModel==='gray'?'灰度':'RGB'}。
    {mapping?`灰度映射：${mapping.lower}–${mapping.upper} → 0–255，截断并向下取整。`:'8 位灰度量程未拉伸。'}
    原始栅格坐标；忽略 EXIF 方向（标记 {input.sourceExifOrientation}）。未做 ICC 色彩管理。
  </p>;
}
