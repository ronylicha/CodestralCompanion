import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

export type ApiProvider = 'Codestral' | 'MistralAi';

export interface Message {
    role: string;
    content: string;
}

export interface Conversation {
    id: string;
    title: string;
    messages: Message[];
    created_at: number;
}

export interface AppSettings {
    api_key: string;
    provider: ApiProvider;
}

interface ChatState {
    conversations: Conversation[];
    currentConversationId: string | null;
    settings: AppSettings;
    isLoading: boolean;
    error: string | null;

    // Actions
    fetchConversations: () => Promise<void>;
    fetchSettings: () => Promise<void>;
    createConversation: (title?: string) => Promise<void>;
    selectConversation: (id: string) => void;
    deleteConversation: (id: string) => Promise<void>;
    renameConversation: (id: string, title: string) => Promise<void>;
    sendMessage: (content: string) => Promise<void>;
    updateSettings: (settings: AppSettings) => Promise<void>;
    clearHistory: () => Promise<void>;
    testConnection: (apiKey: string, provider: ApiProvider) => Promise<string>;
}

export const useChatStore = create<ChatState>((set, get) => ({
    conversations: [],
    currentConversationId: null,
    settings: {
        api_key: '',
        provider: 'MistralAi', // Default
    },
    isLoading: false,
    error: null,

    fetchConversations: async () => {
        try {
            const conversations = await invoke<Conversation[]>('get_conversations');
            set({ conversations });
            if (conversations.length > 0 && !get().currentConversationId) {
                set({ currentConversationId: conversations[0].id });
            }
        } catch (e) {
            console.error('Failed to fetch conversations', e);
        }
    },

    fetchSettings: async () => {
        try {
            const settings = await invoke<AppSettings>('get_app_settings');
            set({ settings });
        } catch (e) {
            console.error('Failed to fetch settings', e);
        }
    },

    createConversation: async (title) => {
        try {
            const newConv = await invoke<Conversation>('create_conversation', { title });
            set((state) => ({
                conversations: [newConv, ...state.conversations],
                currentConversationId: newConv.id,
            }));
        } catch (e) {
            console.error('Failed to create conversation', e);
        }
    },

    selectConversation: (id) => {
        set({ currentConversationId: id });
    },

    deleteConversation: async (id) => {
        try {
            await invoke('delete_conversation', { conversationId: id });
            set((state) => {
                const newConvs = state.conversations.filter((c) => c.id !== id);
                return {
                    conversations: newConvs,
                    currentConversationId: state.currentConversationId === id
                        ? (newConvs.length > 0 ? newConvs[0].id : null)
                        : state.currentConversationId
                };
            });
        } catch (e) {
            console.error('Failed to delete conversation', e);
        }
    },

    renameConversation: async (id, title) => {
        try {
            await invoke('rename_conversation', { conversationId: id, newTitle: title });
            set((state) => ({
                conversations: state.conversations.map(c => c.id === id ? { ...c, title } : c)
            }));
        } catch (e) {
            console.error(e);
        }
    },

    sendMessage: async (content) => {
        const { currentConversationId, settings } = get();
        if (!currentConversationId || !settings.api_key) return;

        set({ isLoading: true, error: null });

        // Optimistic update
        const userMsg = { role: 'user', content };
        set((state) => ({
            conversations: state.conversations.map(c =>
                c.id === currentConversationId
                    ? { ...c, messages: [...c.messages, userMsg] }
                    : c
            )
        }));

        try {
            const response = await invoke<string>('send_message', {
                conversationId: currentConversationId,
                content,
                apiKey: settings.api_key,
                provider: settings.provider,
            });

            const assistantMsg = { role: 'assistant', content: response };
            set((state) => ({
                conversations: state.conversations.map(c =>
                    c.id === currentConversationId
                        ? { ...c, messages: [...c.messages, assistantMsg] }
                        : c
                )
            }));
        } catch (e: any) {
            set({ error: e.toString() });
            // Remove optimistic message if needed, or just show error
        } finally {
            set({ isLoading: false });
        }
    },

    updateSettings: async (settings) => {
        try {
            await invoke('update_settings', { settings });
            set({ settings });
        } catch (e) {
            console.error(e);
        }
    },

    clearHistory: async () => {
        try {
            await invoke('clear_history');
            set({ conversations: [], currentConversationId: null });
        } catch (e) {
            console.error(e);
        }
    },

    testConnection: async (apiKey, provider) => {
        return await invoke('test_api_connection', { apiKey, provider });
    }

}));
