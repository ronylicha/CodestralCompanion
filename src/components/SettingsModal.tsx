import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Settings } from "../types";

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSettingsUpdate: () => void;
}

export function SettingsModal({
  isOpen,
  onClose,
  onSettingsUpdate,
}: SettingsModalProps) {
  const [provider, setProvider] = useState<"codestral.mistral.ai" | "api.mistral.ai">("api.mistral.ai");
  const [apiKey, setApiKey] = useState("");
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (isOpen) {
      loadSettings();
    }
  }, [isOpen]);

  const loadSettings = async () => {
    try {
      const currentSettings = await invoke<Settings>("get_app_settings");
      setProvider(currentSettings.api_provider.provider);
      setApiKey(currentSettings.api_provider.api_key || "");
    } catch (error) {
      console.error("Failed to load settings:", error);
    }
  };

  const handleSave = async () => {
    setLoading(true);
    setTestResult(null);
    try {
      await invoke("update_settings", {
        providerType: provider,
        apiKey: apiKey || undefined,
        phoneNumber: undefined,
      });
      onSettingsUpdate();
      onClose();
    } catch (error: any) {
      setTestResult(`Erreur: ${error}`);
    } finally {
      setLoading(false);
    }
  };

  const handleTestConnection = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      // Save settings first for testing
      await invoke("update_settings", {
        providerType: provider,
        apiKey: apiKey || undefined,
        phoneNumber: undefined,
      });

      const result = await invoke<boolean>("test_api_connection");
      setTestResult(result ? "Connexion réussie!" : "Échec de la connexion");
    } catch (error: any) {
      setTestResult(`Erreur: ${error}`);
    } finally {
      setTesting(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2>Paramètres</h2>
          <button onClick={onClose} className="btn-close">×</button>
        </div>

        <div className="modal-body">
          <div className="form-group">
            <label>Fournisseur d'API</label>
            <select
              value={provider}
              onChange={(e) => setProvider(e.target.value as any)}
            >
              <option value="api.mistral.ai">api.mistral.ai (Pay-as-you-go)</option>
              <option value="codestral.mistral.ai">codestral.mistral.ai (Abonnement mensuel)</option>
            </select>
          </div>

          <div className="form-group">
            <label>Clé API</label>
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="Entrez votre clé API Mistral"
            />
            <p className="form-help">
              {provider === "api.mistral.ai" ? (
                <>
                  Obtenez votre clé API sur{" "}
                  <a href="https://console.mistral.ai/" target="_blank" rel="noopener noreferrer">
                    console.mistral.ai
                  </a>
                </>
              ) : (
                <>
                  Pour obtenir une clé API pour codestral.mistral.ai,{" "}
                  <a href="https://codestral.mistral.ai/" target="_blank" rel="noopener noreferrer">
                    inscrivez-vous sur codestral.mistral.ai
                  </a>
                  {" "}avec votre numéro de téléphone pour obtenir votre clé API.
                </>
              )}
            </p>
          </div>

          {testResult && (
            <div className={`test-result ${testResult.includes("Erreur") ? "error" : "success"}`}>
              {testResult}
            </div>
          )}

          <div className="form-actions">
            <button
              onClick={handleTestConnection}
              disabled={testing || !apiKey}
              className="btn-test"
            >
              {testing ? "Test en cours..." : "Tester la connexion"}
            </button>
            <button
              onClick={handleSave}
              disabled={loading || !apiKey}
              className="btn-save"
            >
              {loading ? "Enregistrement..." : "Enregistrer"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

