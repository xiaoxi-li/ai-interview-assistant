import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

// 每个用例结束后清理已挂载的组件
afterEach(() => {
  cleanup();
});
