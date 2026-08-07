// Theme context + persistence.
//
// Drop-in target: alpha-factor-forge/src/theme/ThemeProvider.tsx
//
// Persistence rule (README「不可妥協的邊界」): the selected skin is a
// NON-SENSITIVE UI preference, so localStorage is allowed during the Tauri
// migration. When `app_settings` gets a typed wrapper, swap readSkin/writeSkin
// for db.getSetting('ui.skin') / db.setSetting('ui.skin', id) — nothing else in
// this file changes.

import React, { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { DEFAULT_SKIN, FONT_HREF, getTheme, isSkinId, type SkinId, type Theme } from './theme';
import { applyTheme } from './themeCss';

const STORE_KEY = 'afs.ui.skin';

function readSkin(): SkinId {
  try {
    const value = localStorage.getItem(STORE_KEY);
    if (isSkinId(value)) return value;
    if (value != null) localStorage.removeItem(STORE_KEY);
    return DEFAULT_SKIN;
  } catch {
    return DEFAULT_SKIN;
  }
}

function writeSkin(id: SkinId): void {
  try {
    localStorage.setItem(STORE_KEY, id);
  } catch {
    /* preference only — never fatal */
  }
}

interface ThemeCtx {
  theme: Theme;
  skin: SkinId;
  setSkin: (id: SkinId) => void;
}

const Ctx = createContext<ThemeCtx | null>(null);

/** Load every skin's font families once, up front — switching must not FOUT. */
function useFonts(): void {
  useEffect(() => {
    if (document.getElementById('afs-fonts')) return;
    const link = document.createElement('link');
    link.id = 'afs-fonts';
    link.rel = 'stylesheet';
    link.href = FONT_HREF;
    document.head.appendChild(link);
  }, []);
}

export function ThemeProvider({ children }: { children: React.ReactNode }): React.ReactElement {
  const [skin, setSkinState] = useState<SkinId>(readSkin);
  const theme = useMemo(() => getTheme(skin), [skin]);
  const rootRef = useRef<HTMLDivElement>(null);
  useFonts();
  const workspaceImage = `${theme.workspaceBackground.scrim}, ${theme.workspaceBackground.image}`;

  // Vars go on the app root AND documentElement: the root covers the app tree,
  // documentElement covers the window background behind it (and any portal).
  useEffect(() => {
    if (rootRef.current) applyTheme(rootRef.current, theme);
    applyTheme(document.documentElement, theme);
    document.body.style.backgroundColor = theme.color.bg;
    document.body.style.backgroundImage = workspaceImage;
    document.body.style.backgroundPosition = theme.workspaceBackground.position;
    document.body.style.backgroundSize = theme.workspaceBackground.size;
    document.body.style.backgroundRepeat = theme.workspaceBackground.repeat;
    document.body.style.color = theme.color.ink;
    document.body.style.fontFamily = theme.font.sans;
  }, [theme, workspaceImage]);

  // Keep separately mounted browser/Tauri windows in sync. The writing window
  // updates through setSkin; sibling windows receive the standard storage event.
  useEffect(() => {
    const onStorage = (event: StorageEvent) => {
      if (event.key !== STORE_KEY) return;
      setSkinState(isSkinId(event.newValue) ? event.newValue : DEFAULT_SKIN);
    };
    window.addEventListener('storage', onStorage);
    return () => window.removeEventListener('storage', onStorage);
  }, []);

  const setSkin = useCallback((id: SkinId) => {
    setSkinState(id);
    writeSkin(id);
  }, []);

  const value = useMemo(() => ({ theme, skin, setSkin }), [theme, skin, setSkin]);

  return (
    <Ctx.Provider value={value}>
      <div
        ref={rootRef}
        className="afs-theme-root"
        data-skin={skin}
        style={{
          minHeight: '100vh',
          backgroundColor: theme.color.bg,
          backgroundImage: workspaceImage,
          backgroundPosition: theme.workspaceBackground.position,
          backgroundSize: theme.workspaceBackground.size,
          backgroundRepeat: theme.workspaceBackground.repeat,
          color: theme.color.ink,
        }}
      >
        {children}
      </div>
    </Ctx.Provider>
  );
}

export function useTheme(): Theme {
  const c = useContext(Ctx);
  // Pop-out windows (chart / metrics) mount their own tree; falling back to the
  // stored skin keeps them consistent without forcing a provider in each.
  return c ? c.theme : getTheme(readSkin());
}

export function useSkin(): { skin: SkinId; setSkin: (id: SkinId) => void } {
  const c = useContext(Ctx);
  if (!c) return { skin: readSkin(), setSkin: writeSkin };
  return { skin: c.skin, setSkin: c.setSkin };
}
