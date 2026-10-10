import { event, invoke } from './invoke.js';

/** EmbeddingGemma-300M at Q8_0, which finds notes by meaning. */
export const EMBEDDING_SIZE = '~330 MB';

export interface EmbeddingModelInfo {
	installed: boolean;
	/** Percent done while a download runs, null otherwise. */
	downloading: number | null;
}

export const embeddingModelInfo = () => invoke<EmbeddingModelInfo>('embedding_model_info');

export const downloadEmbeddingModel = () => invoke<void>('download_embedding_model');

/** The chat model a version before 0.5.0 downloaded, which nothing reads any
 *  more: how many bytes it takes, or null when none is left. */
export const oldChatModel = () => invoke<{ bytes: number } | null>('old_chat_model');

export const removeOldChatModel = () => invoke<void>('remove_old_chat_model');

export type EmbeddingStatus =
	{ state: 'downloading'; percent: number } | { state: 'installed' } | { state: 'absent' };

export const onEmbeddingStatus = event<EmbeddingStatus>('embedding-status');

/** What the embedder is doing, for the status bar of a dev build. */
export type EmbedderActivity =
	| { state: 'waiting' | 'noModel' | 'loading' }
	| { state: 'failed'; error: string }
	| { state: 'embedding'; space: string; done: number; total: number }
	| { state: 'threads' | 'map'; space: string }
	| { state: 'idle'; embedded: number; ms: number; vectors: number };

export const embedderActivity = () => invoke<EmbedderActivity>('embedder_activity');

export const onEmbedderActivity = event<EmbedderActivity>('embedder-activity');

/** Fired after notes were embedded, or their vectors first loaded. */
export const onVectorsChanged = event('vectors-changed');
