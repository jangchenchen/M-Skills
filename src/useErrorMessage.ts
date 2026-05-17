import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import type { ErrorDto } from "./types";

export function useErrorMessage() {
  const { t } = useTranslation("errors");

  return useCallback(
    (err: unknown): string => {
      if (!err) return "";
      if (typeof err === "string") return err;
      const dto = err as ErrorDto;
      if (dto?.code) {
        return t(dto.code, { ...dto.params, defaultValue: dto.code });
      }
      if (err instanceof Error) return err.message;
      return t("unknown");
    },
    [t]
  );
}
