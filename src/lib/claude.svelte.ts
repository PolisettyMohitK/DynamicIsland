// Claude Code sessions (claude.rs) for the Control Centre, and a pop-up when one finishes.
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { popup } from './alerts';

export type ClaudeStatus = 'working' | 'waiting' | 'done' | 'stopped';
export type ClaudeSession = {
  id: string;
  title: string;
  cwd: string;
  status: ClaudeStatus;
  updated: number;
  reply: string | null;
  /** The desktop app's id for it, if the desktop app runs it. */
  desktop: string | null;
  /** What it did last, in a few words. */
  step: string | null;
  /** Its task list for what you last asked, if it keeps one. */
  tasks: { text: string; status: 'pending' | 'in_progress' | 'completed' }[];
};

const ACCENT = '#d97757';

class Claude {
  sessions = $state<ClaudeSession[]>([]);

  start() {
    invoke<ClaudeSession[]>('claude_sessions').then((s) => (this.sessions = s));
    const offs = [
      listen<ClaudeSession[]>('claude-sessions', (e) => (this.sessions = e.payload)),
      listen<ClaudeSession>('claude-done', (e) =>
        popup({ icon: 'sparkle', label: e.payload.title, value: 'Done', tone: 'ok', accent: ACCENT, hold: 3000 }),
      ),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  }

  /** Carries on with the session in the desktop app (if it runs it), else in a terminal. */
  open(s: ClaudeSession, inDesktop: boolean) {
    invoke('claude_open', { id: s.id, cwd: s.cwd, desktop: inDesktop ? s.desktop : null }).catch((e) =>
      popup({ icon: 'sparkle', label: 'Claude Code', value: String(e), tone: 'bad', accent: ACCENT, hold: 3000 }),
    );
  }
}

export const claude = new Claude();
