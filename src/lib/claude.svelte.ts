// Claude Code sessions (claude.rs) for the Control Centre, a pop-up when one finishes, and
// prompts sent from the island.
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
};
export type ClaudeReply = { session: string | null; text: string; ok: boolean };

const ACCENT = '#d97757';

class Claude {
  sessions = $state<ClaudeSession[]>([]);
  /** A prompt from the island is running. */
  sending = $state(false);
  /** The answer to the last prompt sent from here. */
  reply = $state<ClaudeReply | null>(null);

  start() {
    invoke<ClaudeSession[]>('claude_sessions').then((s) => (this.sessions = s));
    const offs = [
      listen<ClaudeSession[]>('claude-sessions', (e) => (this.sessions = e.payload)),
      listen<ClaudeSession>('claude-done', (e) =>
        popup({ icon: 'sparkle', label: e.payload.title, value: 'Done', tone: 'ok', accent: ACCENT, hold: 3000 }),
      ),
      listen<ClaudeReply>('claude-reply', (e) => {
        this.sending = false;
        this.reply = e.payload;
        if (!e.payload.ok) popup({ icon: 'sparkle', label: 'Claude Code', value: 'Failed', tone: 'bad', accent: ACCENT, hold: 3000 });
      }),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  }

  /** Runs `prompt` in `cwd`, in `session` if given, else as a new session. */
  send(prompt: string, cwd: string, session?: string) {
    this.sending = true;
    this.reply = null;
    invoke('claude_send', { prompt, cwd, session: session ?? null });
  }
}

export const claude = new Claude();
