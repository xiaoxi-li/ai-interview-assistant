import { useCallback } from "react";
import { translate } from "../lib/i18n";
import { useAppStore } from "../stores/appStore";

/** 翻译函数：`const t = useT(); t("开始转写")` */
export function useT() {
  const locale = useAppStore((s) => s.locale);
  return useCallback(
    (text: string, params?: Record<string, string | number>) =>
      translate(locale, text, params),
    [locale]
  );
}
