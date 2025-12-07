import React, { useEffect, useRef } from 'react';
import { useChatStore } from '../../stores/useChatStore';
import { marked } from 'marked';
import { markedHighlight } from "marked-highlight";
import DOMPurify from 'dompurify';
import hljs from 'highlight.js';
import 'highlight.js/styles/github-dark.css';
import MessageInput from './MessageInput';
import { Settings } from 'lucide-react';

// Configure marked with highlight extension
marked.use(
    markedHighlight({
        langPrefix: 'hljs language-',
        highlight(code, lang) {
            const language = hljs.getLanguage(lang) ? lang : 'plaintext';
            return hljs.highlight(code, { language }).value;
        }
    })
);

interface Props {
    onOpenSettings: () => void;
    onToggleSidebar: () => void;
}

const ChatWindow: React.FC<Props> = ({ onOpenSettings, onToggleSidebar }) => {
    const { currentConversationId, conversations, sendMessage, isLoading, error, settings, createConversation } = useChatStore();
    const currentConversation = conversations.find((c) => c.id === currentConversationId);
    const messagesEndRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [currentConversation?.messages, isLoading]);

    const renderContent = (content: string) => {
        // marked.parse returns Promise | string, but practically string with sync options. 
        // Typescript might complain, so cast or handle async.
        // In this setup, it's synchronous.
        const rawMarkup = marked.parse(content, { async: false }) as string;
        const cleanMarkup = DOMPurify.sanitize(rawMarkup);
        return { __html: cleanMarkup };
    };

    if (!currentConversationId && conversations.length === 0) {
        return (
            <div className="flex-1 flex flex-col items-center justify-center p-8 bg-gray-50 text-center">
                <h1 className="text-3xl font-bold text-gray-800 mb-4">Companion Chat</h1>

                {!settings.api_key ? (
                    <>
                        <p className="text-gray-600 mb-8 max-w-md">
                            Welcome! To get started, please configure your API Settings.
                        </p>
                        <button
                            onClick={onOpenSettings}
                            className="px-6 py-3 bg-blue-600 text-white rounded-lg shadow-lg hover:bg-blue-700 transition flex items-center gap-2"
                        >
                            <Settings size={20} />
                            Configure API Key
                        </button>
                    </>
                ) : (
                    <>
                        <p className="text-gray-600 mb-8 max-w-md">
                            You are all set! Start a new conversation to begin chatting.
                        </p>
                        <button
                            onClick={() => createConversation()}
                            className="px-6 py-3 bg-green-600 text-white rounded-lg shadow-lg hover:bg-green-700 transition flex items-center gap-2"
                        >
                            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="12" y1="5" x2="12" y2="19"></line><line x1="5" y1="12" x2="19" y2="12"></line></svg>
                            Start New Chat
                        </button>
                    </>
                )}
            </div>
        )
    }

    return (
        <div className="flex-1 flex flex-col h-screen bg-gray-50 overflow-hidden relative">


            {/* Header */}
            <div className="h-14 border-b bg-white flex items-center px-4 justify-between shrink-0">
                <div className="flex items-center gap-2">
                    <button onClick={onToggleSidebar} className="lg:hidden p-2 hover:bg-gray-100 rounded-md">
                        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="3" y1="12" x2="21" y2="12"></line><line x1="3" y1="6" x2="21" y2="6"></line><line x1="3" y1="18" x2="21" y2="18"></line></svg>
                    </button>
                    <h2 className="font-semibold text-gray-800 truncate max-w-xs sm:max-w-md">
                        {currentConversation?.title || 'New Conversation'}
                    </h2>
                </div>

                <button onClick={onOpenSettings} className="p-2 text-gray-500 hover:text-gray-700 hover:bg-gray-100 rounded-full transition">
                    <Settings size={20} />
                </button>
            </div>

            {/* Messages */}
            <div className="flex-1 overflow-y-auto p-4 space-y-6">
                {currentConversation ? (
                    currentConversation.messages.map((msg, idx) => (
                        <div
                            key={idx}
                            className={`flex w-full ${msg.role === 'user' ? 'justify-end' : 'justify-start'
                                }`}
                        >
                            <div
                                className={`max-w-[85%] rounded-2xl px-5 py-3 shadow-sm ${msg.role === 'user'
                                    ? 'bg-blue-600 text-white'
                                    : 'bg-white text-gray-800 border border-gray-100'
                                    }`}
                            >
                                {msg.role === 'user' ? (
                                    <p className="whitespace-pre-wrap">{msg.content}</p>
                                ) : (
                                    <div
                                        className="prose prose-sm prose-slate max-w-none text-gray-800"
                                        dangerouslySetInnerHTML={renderContent(msg.content)}
                                    />
                                )}
                            </div>
                        </div>
                    ))
                ) : (
                    <div className="flex flex-col items-center justify-center h-full text-gray-400">
                        <p>Select or create a conversation</p>
                    </div>
                )}

                {isLoading && (
                    <div className="flex w-full justify-start">
                        <div className="bg-white text-gray-800 border border-gray-100 rounded-2xl px-5 py-3 shadow-sm flex items-center gap-2">
                            <div className="w-2 h-2 bg-gray-400 rounded-full animate-bounce" style={{ animationDelay: '0ms' }}></div>
                            <div className="w-2 h-2 bg-gray-400 rounded-full animate-bounce" style={{ animationDelay: '150ms' }}></div>
                            <div className="w-2 h-2 bg-gray-400 rounded-full animate-bounce" style={{ animationDelay: '300ms' }}></div>
                        </div>
                    </div>
                )}

                {error && (
                    <div className="w-full flex justify-center">
                        <div className="bg-red-50 text-red-600 px-4 py-2 rounded-lg text-sm border border-red-100">
                            Error: {error}
                        </div>
                    </div>
                )}

                <div ref={messagesEndRef} />
            </div>

            {/* Input */}
            <MessageInput onSend={sendMessage} disabled={isLoading || !currentConversationId} />
        </div>
    );
};

export default ChatWindow;
