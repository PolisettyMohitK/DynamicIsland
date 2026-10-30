// The AI service Choco uses, its keys and models (src-tauri/src/ai.rs), for both settings screens.
import { invoke } from '@tauri-apps/api/core';

export type Provider = 'opencode' | 'openai' | 'gemini';
/** chat answers questions; computer works the desktop. */
export type Job = 'chat' | 'computer';
export type Service = { key: boolean; chat: string; computer: string; defaultChat: string; defaultComputer: string };
export type AiStatus = { provider: Provider; services: Record<Provider, Service> };

export const PROVIDERS: { value: Provider; label: string }[] = [
  { value: 'opencode', label: 'OpenCode' },
  { value: 'openai', label: 'OpenAI' },
  { value: 'gemini', label: 'Gemini' },
];

export const providerName = (p: Provider) => PROVIDERS.find((x) => x.value === p)!.label;

export const aiStatus = () => invoke<AiStatus>('ai_status');
export const setProvider = (provider: Provider) => invoke('ai_set_provider', { provider });
/** An empty key removes it. */
export const setKey = (provider: Provider, key: string) => invoke('ai_set_key', { provider, key });
/** The models the service offers for `job`; empty without a key or when it won't say. */
export const aiModels = (provider: Provider, job: Job) => invoke<string[]>('ai_models', { provider, job });
/** An empty model goes back to the default. */
export const setModel = (provider: Provider, job: Job, model: string) => invoke('ai_set_model', { provider, job, model });
