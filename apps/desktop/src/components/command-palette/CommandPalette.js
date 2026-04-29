import { jsx as _jsx, jsxs as _jsxs, Fragment as _Fragment } from "react/jsx-runtime";
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
import "./palette.css";
const EASE = [0.16, 1, 0.3, 1];
export function CommandPalette() {
    const { open, setOpen, commands } = useCommandPalette();
    const { t } = useTranslation();
    const [query, setQuery] = useState("");
    // open될 때마다 query 초기화 — 이전 검색 잔재 회피.
    const isOpen = open;
    const filtered = useMemo(() => commands.filter((c) => matchesQuery(c, query)), [commands, query]);
    const groups = useMemo(() => groupCommands(filtered), [filtered]);
    // Ark UI는 collection items의 'value'/'label' 키를 기본으로 사용. 우리 Command은 id/label.
    const collection = useListCollection({
        initialItems: filtered,
        itemToValue: (item) => item.id,
        itemToString: (item) => item.label,
        isItemDisabled: (item) => item.isAvailable ? !item.isAvailable() : false,
    });
    // filtered 변경 시 collection 동기화.
    useMemo(() => {
        collection.collection.setItems(filtered);
    }, [collection.collection, filtered]);
    const handleSelect = async (id) => {
        const cmd = commands.find((c) => c.id === id);
        if (!cmd)
            return;
        if (cmd.isAvailable && !cmd.isAvailable())
            return;
        setOpen(false);
        setQuery("");
        try {
            await Promise.resolve(cmd.perform());
        }
        catch (err) {
            console.error(`[command-palette] perform failed for ${id}:`, err);
        }
    };
    const handleOpenChange = (next) => {
        setOpen(next);
        if (!next)
            setQuery("");
    };
    // SSR-safe portal target (Tauri WebView은 항상 document 있음 — 안전장치만).
    if (typeof document === "undefined")
        return null;
    return createPortal(_jsx(AnimatePresence, { children: isOpen && (_jsxs(_Fragment, { children: [_jsx(motion.div, { className: "palette-backdrop", initial: { opacity: 0 }, animate: { opacity: 1 }, exit: { opacity: 0 }, transition: { duration: 0.12, ease: EASE }, onClick: () => handleOpenChange(false), "aria-hidden": true }), _jsx(motion.div, { className: "palette-frame", initial: { opacity: 0, scale: 0.96, y: -8 }, animate: { opacity: 1, scale: 1, y: 0 }, exit: { opacity: 0, scale: 0.97, y: -4 }, transition: { duration: 0.15, ease: EASE }, children: _jsx(Combobox.Root, { collection: collection.collection, open: isOpen, onOpenChange: (d) => handleOpenChange(d.open), inputValue: query, onInputValueChange: (d) => setQuery(d.inputValue), onValueChange: (d) => {
                            if (d.value[0]) {
                                void handleSelect(d.value[0]);
                            }
                        }, loopFocus: true, openOnClick: true, positioning: { strategy: "fixed" }, children: _jsxs("div", { className: "palette-dialog glass", role: "dialog", "aria-modal": "true", "aria-label": t("palette.aria.dialog") ?? undefined, children: [_jsx(Combobox.Control, { className: "palette-control", children: _jsx(Combobox.Input, { className: "palette-input", placeholder: t("palette.placeholder") ?? undefined, autoFocus: true }) }), _jsx(Combobox.Content, { className: "palette-list", children: filtered.length === 0 ? (_jsx("div", { className: "palette-empty", children: t("palette.empty") })) : (groups.map(([group, items]) => (_jsx(PaletteGroup, { group: group, items: items, groupLabel: t(`palette.group.${group}`) }, group)))) })] }) }) })] })) }), document.body);
}
function PaletteGroup({ group, items, groupLabel, }) {
    return (_jsxs(Combobox.ItemGroup, { id: group, className: "palette-group", children: [_jsx(Combobox.ItemGroupLabel, { className: "palette-group-label", children: groupLabel }), items.map((cmd) => {
                const disabled = cmd.isAvailable ? !cmd.isAvailable() : false;
                return (_jsxs(Combobox.Item, { item: cmd, className: "palette-item", "data-disabled": disabled || undefined, children: [_jsx(Combobox.ItemText, { className: "palette-item-label", children: cmd.label }), cmd.shortcut && cmd.shortcut.length > 0 && (_jsx("span", { className: "palette-item-shortcut", "aria-hidden": true, children: cmd.shortcut.map((k, i) => (_jsx("kbd", { children: k }, i))) }))] }, cmd.id));
            })] }));
}
