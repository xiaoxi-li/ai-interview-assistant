import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import PrivacySettingsSection from "../PrivacySettingsSection";
import type { PrivacyConfig } from "../../../types";

const config: PrivacyConfig = { uploadAudio: false, encryptLocal: false };

function setup(overrides: Partial<Parameters<typeof PrivacySettingsSection>[0]> = {}) {
  const handlers = { onSave: vi.fn(), onClearData: vi.fn() };
  render(
    <PrivacySettingsSection
      config={config}
      cloudAsrEnabled
      clearing={false}
      {...handlers}
      {...overrides}
    />
  );
  return handlers;
}

describe("PrivacySettingsSection", () => {
  it("云端模式下上传开关可用", () => {
    setup();
    const upload = screen.getByLabelText(/允许上传音频至云端 ASR/) as HTMLInputElement;
    expect(upload.disabled).toBe(false);
    expect(screen.queryByText(/当前为本地 ASR 模式/)).toBeNull();
  });

  it("本地模式下禁用上传开关并给出提示", () => {
    setup({ cloudAsrEnabled: false });
    const upload = screen.getByLabelText(/允许上传音频至云端 ASR/) as HTMLInputElement;
    expect(upload.disabled).toBe(true);
    expect(screen.getByText(/当前为本地 ASR 模式/)).toBeTruthy();
  });

  it("切换开关后保存回传草稿", () => {
    const h = setup();
    fireEvent.click(screen.getByLabelText(/本地敏感数据加密/));
    fireEvent.click(screen.getByText("保存配置"));

    expect(h.onSave).toHaveBeenCalledWith(
      expect.objectContaining({ encryptLocal: true })
    );
  });

  it("清除数据需二次确认", () => {
    const h = setup();

    fireEvent.click(screen.getByText("清除数据…"));
    expect(h.onClearData).not.toHaveBeenCalled();

    fireEvent.click(screen.getByText("确认清除"));
    expect(h.onClearData).toHaveBeenCalledTimes(1);
  });

  it("二次确认可取消", () => {
    const h = setup();
    fireEvent.click(screen.getByText("清除数据…"));
    fireEvent.click(screen.getByText("取消"));

    expect(screen.queryByText("确认清除")).toBeNull();
    expect(h.onClearData).not.toHaveBeenCalled();
  });

  it("清除中禁用确认按钮", () => {
    setup({ clearing: true });
    fireEvent.click(screen.getByText("清除数据…"));

    const confirm = screen.getByText("确认清除").closest("button") as HTMLButtonElement;
    expect(confirm.disabled).toBe(true);
  });
});
