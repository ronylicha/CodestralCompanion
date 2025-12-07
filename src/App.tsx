import { useEffect, useState } from 'react';
import { useChatStore } from './stores/useChatStore';
import Sidebar from './components/Layout/Sidebar';
import ChatWindow from './components/Chat/ChatWindow';
import SettingsModal from './components/Settings/SettingsModal';
import { listen } from '@tauri-apps/api/event';

function App() {
  const { fetchConversations, fetchSettings } = useChatStore();
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);

  useEffect(() => {
    // Initial fetch
    fetchSettings();
    fetchConversations();

    // Listen for tray events
    const unlistenSettings = listen('open-settings', () => {
      setIsSettingsOpen(true);
    });

    const unlistenClearHistory = listen('request-clear-history', () => {
      // Maybe show confirmation modal? For now just log
      console.log("History clear requested from tray (backend handled store clear, frontend needs refresh)");
      fetchConversations();
    });

    return () => {
      unlistenSettings.then(f => f());
      unlistenClearHistory.then(f => f());
    };
  }, []);

  return (
    <div className="flex w-full h-screen overflow-hidden bg-white">
      <Sidebar
        isOpen={isSidebarOpen}
        onClose={() => setIsSidebarOpen(false)}
      />

      <ChatWindow
        onToggleSidebar={() => setIsSidebarOpen(!isSidebarOpen)}
        onOpenSettings={() => setIsSettingsOpen(true)}
      />

      <SettingsModal
        isOpen={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
      />
    </div>
  );
}

export default App;
