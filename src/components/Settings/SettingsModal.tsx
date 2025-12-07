import React, { useState, useEffect } from 'react';
import { useChatStore, ApiProvider } from '../../stores/useChatStore';
import { X, Save, CheckCircle, AlertCircle } from 'lucide-react';

interface Props {
    isOpen: boolean;
    onClose: () => void;
}

const SettingsModal: React.FC<Props> = ({ isOpen, onClose }) => {
    const { settings, updateSettings, testConnection } = useChatStore();
    const [apiKey, setApiKey] = useState(settings.api_key);
    const [provider, setProvider] = useState<ApiProvider>(settings.provider);
    const [testStatus, setTestStatus] = useState<'idle' | 'success' | 'error'>('idle');
    const [testMessage, setTestMessage] = useState('');

    useEffect(() => {
        if (isOpen) {
            setApiKey(settings.api_key);
            setProvider(settings.provider);
            setTestStatus('idle');
            setTestMessage('');
        }
    }, [isOpen, settings]);

    if (!isOpen) return null;

    const handleSave = async () => {
        await updateSettings({ api_key: apiKey, provider });
        onClose();
    };

    const handleTest = async () => {
        setTestStatus('idle');
        try {
            const msg = await testConnection(apiKey, provider);
            setTestStatus('success');
            setTestMessage(msg);
        } catch (e: any) {
            setTestStatus('error');
            setTestMessage(e.toString());
        }
    };

    return (
        <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center">
            <div className="bg-white rounded-lg shadow-xl w-full max-w-md p-6">
                <div className="flex justify-between items-center mb-4">
                    <h2 className="text-xl font-semibold">Settings</h2>
                    <button onClick={onClose} className="text-gray-500 hover:text-gray-700">
                        <X size={24} />
                    </button>
                </div>

                <div className="space-y-4">
                    <div>
                        <label className="block text-sm font-medium text-gray-700 mb-1">API Provider</label>
                        <select
                            value={provider}
                            onChange={(e) => setProvider(e.target.value as ApiProvider)}
                            className="w-full border border-gray-300 rounded-md p-2 focus:ring-2 focus:ring-blue-500 outline-none"
                        >
                            <option value="MistralAi">Mistral AI (api.mistral.ai)</option>
                            <option value="Codestral">Codestral (codestral.mistral.ai)</option>
                        </select>
                    </div>

                    <div>
                        <label className="block text-sm font-medium text-gray-700 mb-1">API Key</label>
                        <input
                            type="password"
                            value={apiKey}
                            onChange={(e) => setApiKey(e.target.value)}
                            placeholder="Enter your API Key"
                            className="w-full border border-gray-300 rounded-md p-2 focus:ring-2 focus:ring-blue-500 outline-none"
                        />
                    </div>

                    <div className="flex items-center gap-2">
                        <button
                            onClick={handleTest}
                            className="text-sm text-blue-600 hover:underline"
                        >
                            Test Connection
                        </button>
                        {testStatus === 'success' && <span className="text-green-600 flex items-center gap-1 text-sm"><CheckCircle size={14} /> Connected</span>}
                        {testStatus === 'error' && <span className="text-red-600 flex items-center gap-1 text-sm"><AlertCircle size={14} /> Failed: {testMessage}</span>}
                    </div>
                </div>

                <div className="mt-6 flex justify-end gap-3">
                    <button
                        onClick={onClose}
                        className="px-4 py-2 text-gray-700 hover:bg-gray-100 rounded-md"
                    >
                        Cancel
                    </button>
                    <button
                        onClick={handleSave}
                        className="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 flex items-center gap-2"
                    >
                        <Save size={18} />
                        Save Settings
                    </button>
                </div>
            </div>
        </div>
    );
};

export default SettingsModal;
