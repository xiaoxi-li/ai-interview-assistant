import { describe, expect, it } from "vitest";
import { formatBytes } from "../bytes";

describe("formatBytes", () => {
  it("零与非法值返回 0 B", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(-5)).toBe("0 B");
    expect(formatBytes(Number.NaN)).toBe("0 B");
  });

  it("按 1024 进制换算并保留合适精度", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(90 * 1024 * 1024)).toBe("90.0 MB");
    expect(formatBytes(200 * 1024 * 1024)).toBe("200 MB");
  });
});
