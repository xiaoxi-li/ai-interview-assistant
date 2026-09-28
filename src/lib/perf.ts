/**
 * 仅开发期生效的切页耗时打点（NFR-17）。
 *
 * 记录「点击导航 → 目标页首帧」的耗时，用于性能治理前后的对比验证。
 * 生产构建下（`import.meta.env.DEV === false`）全部为空操作：不注册监听、不写标记、
 * 不产生日志，也不改变任何界面行为。
 *
 * 用法：导航点击时 `markNavClick(from, to)`，目标页提交并绘制后 `markPageRendered(to)`。
 */

/** 待结算的一次切页 */
interface PendingSwitch {
  from: string;
  to: string;
  start: number;
}

let pending: PendingSwitch | null = null;

/** 在导航点击的同步时刻记录起点（需早于任何渲染工作） */
export function markNavClick(from: string, to: string): void {
  if (!import.meta.env.DEV) return;
  pending = { from, to, start: performance.now() };
}

/** 目标页首帧后结算耗时并输出；与 {@link markNavClick} 不成对时静默忽略 */
export function markPageRendered(page: string): void {
  if (!import.meta.env.DEV) return;
  const current = pending;
  if (!current || current.to !== page) return;
  pending = null;
  const duration = performance.now() - current.start;
  console.debug(`[perf] 切页 ${current.from} → ${current.to}：${duration.toFixed(1)}ms`);
}
