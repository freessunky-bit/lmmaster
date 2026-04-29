// CommandPalette — ⌘K 데스크톱 명령 팔레트. Phase 1A.4.e §B7~B8.
//
// 구성:
// - Ark UI Combobox (5.36+) 헤드리스 — open/onOpenChange 컨트롤드 + useListCollection 필터.
// - framer-motion AnimatePresence로 backdrop fade + dialog scale.
// - .glass 토큰으로 backdrop-blur + saturate (reduced-transparency 자동 fallback).
// - Esc/click-outside/⌘K로 닫기 (hotkey hook이 처리).

import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Combobox, useListCollection } from "@ark-ui/react/combobox";
import { AnimatePresence, motion } from "framer-motion";
import { createPortal } from "react-dom";

import { useCommandPalette } from "./context";
import { groupCommands, matchesQuery } from "./filter";
import type { Command, CommandGroup } from "./types";
import "./palette.css";

const EASE = [0.16, 1, 0.3, 1] as const;

export function CommandPalette() {
  const { open, setOpen, commands } = useCommandPalette();
  const { t } = useTranslation();
  const [query, setQuery] = useState("");

  // open될 때마다 query 초기화 — 이전 검색 잔재 회피.
  const isOpen = open;

  const filtered = useMemo(
    () => commands.filter((c) => matchesQuery(c, query)),
    [commands, query],
  );
  const groups = useMemo(() => groupCommands(filtered), [filtered]);

  // Ark UI는 collection items의 'value'/'label' 키를 기본으로 사용. 우리 Command은 id/label.
  const collection = useListCollection({
    initialItems: filtered,
    itemToValue: (item: Command) => item.id,
    itemToString: (item: Command) => item.label,
    isItemDisabled: (item: Command) =>
      item.isAvailable ? !item.isAvailable() : false,
  });

  // filtered 변경 시 collection 동기화.
  useMemo(() => {
    collection.collection.setItems(filtered);
  }, [collection.collection, filtered]);

  const handleSelect = async (id: string) => {
    const cmd = commands.find((c) => c.id === id);
    if (!cmd) return;
    if (cmd.isAvailable && !cmd.isAvailable()) return;
    setOpen(false);
    setQuery("");
    try {
      await Promise.resolve(cmd.perform());
    } catch (err) {
      console.error(`[command-palette] perform failed for ${id}:`, err);
    }
  };

  const handleOpenChange = (next: boolean) => {
    setOpen(next);
    if (!next) setQuery("");
  };

  // SSR-safe portal target (Tauri WebView은 항상 document 있음 — 안전장치만).
  if (typeof document === "undefined") return null;

  return createPortal(
    <AnimatePresence>
      {isOpen && (
        <>
          <motion.div
            className="palette-backdrop"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.12, ease: EASE }}
            onClick={() => handleOpenChange(false)}
            aria-hidden
          />
          <motion.div
            className="palette-frame"
            initial={{ opacity: 0, scale: 0.96, y: -8 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.97, y: -4 }}
            transition={{ duration: 0.15, ease: EASE }}
          >
            <Combobox.Root
              collection={collection.collection}
              open={isOpen}
              onOpenChange={(d) => handleOpenChange(d.open)}
              inputValue={query}
              onInputValueChange={(d) => setQuery(d.inputValue)}
              onValueChange={(d) => {
                if (d.value[0]) {
                  void handleSelect(d.value[0]);
                }
              }}
              loopFocus
              openOnClick
              positioning={{ strategy: "fixed" }}
            >
              <div className="palette-dialog glass" role="dialog" aria-modal="true" aria-label={t("palette.aria.dialog") ?? undefined}>
                <Combobox.Control className="palette-control">
                  <Combobox.Input
                    className="palette-input"
                    placeholder={t("palette.placeholder") ?? undefined}
                    autoFocus
                  />
                </Combobox.Control>
                <Combobox.Content className="palette-list">
                  {filtered.length === 0 ? (
                    <div className="palette-empty">{t("palette.empty")}</div>
                  ) : (
                    groups.map(([group, items]) => (
                      <PaletteGroup
                        key={group}
                        group={group}
                        items={items}
                        groupLabel={t(`palette.group.${group}`)}
                      />
                    ))
                  )}
                </Combobox.Content>
              </div>
            </Combobox.Root>
          </motion.div>
        </>
      )}
    </AnimatePresence>,
    document.body,
  );
}

function PaletteGroup({
  group,
  items,
  groupLabel,
}: {
  group: CommandGroup;
  items: Command[];
  groupLabel: string;
}) {
  return (
    <Combobox.ItemGroup id={group} className="palette-group">
      <Combobox.ItemGroupLabel className="palette-group-label">
        {groupLabel}
      </Combobox.ItemGroupLabel>
      {items.map((cmd) => {
        const disabled = cmd.isAvailable ? !cmd.isAvailable() : false;
        return (
          <Combobox.Item
            key={cmd.id}
            item={cmd}
            className="palette-item"
            data-disabled={disabled || undefined}
          >
            <Combobox.ItemText className="palette-item-label">
              {cmd.label}
            </Combobox.ItemText>
            {cmd.shortcut && cmd.shortcut.length > 0 && (
              <span className="palette-item-shortcut" aria-hidden>
                {cmd.shortcut.map((k, i) => (
                  <kbd key={i}>{k}</kbd>
                ))}
              </span>
            )}
          </Combobox.Item>
        );
      })}
    </Combobox.ItemGroup>
  );
}
