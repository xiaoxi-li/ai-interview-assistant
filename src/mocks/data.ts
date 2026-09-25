import type { Group, Note } from "../types";

const DAY = 86_400_000;
const now = Date.now();

export const MOCK_GROUPS: Group[] = [
  { id: "g-frontend", name: "前端工程", parentId: null, order: 0 },
  { id: "g-frontend-react", name: "React 与状态管理", parentId: "g-frontend", order: 0 },
  { id: "g-backend", name: "后端与架构", parentId: null, order: 1 },
  { id: "g-algo", name: "算法与数据结构", parentId: null, order: 2 },
  { id: "g-behavior", name: "行为面试", parentId: null, order: 3 },
];

export const MOCK_NOTES: Note[] = [
  {
    id: "n-react-perf",
    title: "React 性能优化清单",
    groupId: "g-frontend-react",
    tags: ["React", "性能优化"],
    isDeleted: false,
    createdAt: now - 12 * DAY,
    updatedAt: now - 2 * DAY,
    content: `# React 性能优化清单

## 渲染层面
- 使用 \`React.memo\` 包裹纯展示组件，避免父组件重渲染连带更新。
- 拆分状态下沉：把高频变化的状态放到最近的子组件，减少影响范围。
- 列表渲染必须提供稳定 \`key\`，避免使用数组下标。

## 计算与副作用
| 场景 | 手段 | 说明 |
|---|---|---|
| 昂贵计算 | \`useMemo\` | 依赖数组要完整 |
| 函数引用 | \`useCallback\` | 配合 memo 子组件 |
| 大数据列表 | 虚拟滚动 | 只渲染可视区域 |

## 任务优先级
- 使用 \`useTransition\` 把非紧急更新标记为过渡任务。
- 长列表更新可配合 \`useDeferredValue\` 延迟渲染。

\`\`\`tsx
const deferredQuery = useDeferredValue(query);
const results = useMemo(() => search(list, deferredQuery), [list, deferredQuery]);
\`\`\`

## 排查手段
- [x] React DevTools Profiler 火焰图定位重渲染
- [x] \`why-did-you-render\` 捕获无意义渲染
- [ ] 生产环境 Performance 面板采样
`,
  },
  {
    id: "n-event-loop",
    title: "事件循环与异步模型",
    groupId: "g-backend",
    tags: ["JavaScript", "并发"],
    isDeleted: false,
    createdAt: now - 20 * DAY,
    updatedAt: now - 5 * DAY,
    content: `# 事件循环与异步模型

## 核心概念
单线程 + 事件循环，任务分为宏任务与微任务。

1. 执行同步代码，遇到异步 API 交给宿主环境。
2. 当前宏任务结束后，清空**所有微任务**队列。
3. 取下一个宏任务，重复。

> 微任务优先级高于宏任务，这是 \`setTimeout(fn, 0)\` 排在
> \`Promise.then\` 之后的原因。

## 常见陷阱
- 微任务中递归产生新微任务会造成"饿死"宏任务。
- \`await\` 之后的代码等价于 \`Promise.then\` 回调，属于微任务。

## 输出顺序示例
\`\`\`js
console.log("1");
setTimeout(() => console.log("2"));
Promise.resolve().then(() => console.log("3"));
console.log("4");
// 1 4 3 2
\`\`\`
`,
  },
  {
    id: "n-star-story",
    title: "STAR 法则与项目复盘模板",
    groupId: "g-behavior",
    tags: ["行为面试", "表达"],
    isDeleted: false,
    createdAt: now - 30 * DAY,
    updatedAt: now - 8 * DAY,
    content: `# STAR 法则与项目复盘模板

## 结构
- **S**ituation：背景与约束
- **T**ask：我的目标与职责
- **A**ction：我采取的关键动作（突出个人贡献）
- **R**esult：量化结果与反思

## 示例：订单系统重构
- 背景：单体应用日均 300 万订单，高峰期接口 P99 达 2.4s。
- 任务：在不停机前提下将下单链路耗时降到 500ms 以内。
- 行动：拆分读写模型、引入本地缓存、把同步扣减改为异步对账。
- 结果：P99 降至 380ms，故障率下降 76%，方案沉淀为团队规范。

## 复述要点
1. 用数字说明规模与收益。
2. 强调**我**的判断与取舍，而非团队流水账。
3. 主动补充失败与改进，体现反思能力。
`,
  },
];
