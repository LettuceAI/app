import { useEffect } from "react";
import type { ProviderCapabilitiesCamel } from "../../../../core/providers/capabilities";
import type { TestResult } from "../hooks/onboardingReducer";
import { ProviderCard } from "../components/ProviderCard";
import { ProviderConfigForm } from "../components/ConfigForm";
import { getPlatform } from "../../../../core/utils/platform";

interface ProviderStepProps {
  capabilities: ProviderCapabilitiesCamel[];
  selectedProviderId: string;
  label: string;
  apiKey: string;
  baseUrl: string;
  config?: Record<string, any>;
  testResult: TestResult;
  isTesting: boolean;
  isSubmitting: boolean;
  canTest: boolean;
  canSave: boolean;
  onSelectProvider: (provider: { id: string; name: string; defaultBaseUrl?: string }) => void;
  onLabelChange: (value: string) => void;
  onApiKeyChange: (value: string) => void;
  onBaseUrlChange: (value: string) => void;
  onConfigChange?: (config: Record<string, any>) => void;
  onTestConnection: () => void;
  onSave: () => void;
}

export function ProviderStep({
  capabilities,
  selectedProviderId,
  label,
  apiKey,
  baseUrl,
  config,
  testResult,
  isTesting,
  isSubmitting,
  canTest,
  canSave,
  onSelectProvider,
  onLabelChange,
  onApiKeyChange,
  onBaseUrlChange,
  onConfigChange,
  onTestConnection,
  onSave,
}: ProviderStepProps) {
  const platform = getPlatform();
  const isDesktop = platform.type === "desktop";
  const visibleCapabilities = isDesktop
    ? capabilities
    : capabilities.filter((provider) => provider.id !== "llamacpp");

  const selectedProvider = visibleCapabilities.find((p) => p.id === selectedProviderId);
  const showForm = Boolean(selectedProvider);

  // Scroll to form on first show (mobile only) - using getElementById like original
  useEffect(() => {
    if (isDesktop || !showForm) return;

    const timeout = window.setTimeout(() => {
      const formElement = document.getElementById("provider-config-form");
      if (formElement) {
        formElement.scrollIntoView({
          behavior: "smooth",
          block: "end",
          inline: "nearest",
        });
      }
    }, 350);

    return () => window.clearTimeout(timeout);
  }, [showForm, isDesktop]);

  // Desktop Layout
  if (isDesktop) {
    return (
      <div className="flex flex-1 min-h-0">
        {/* Left Panel - Provider Grid */}
        <div className="flex-1 flex flex-col border-r border-white/10">
          <div className="p-6 pb-3">
            <h2 className="text-sm font-medium text-white/70">Available Providers</h2>
            <p className="text-xs text-gray-500 mt-0.5">Click to select a provider</p>
          </div>
          <div className="flex-1 overflow-y-auto px-6">
            <div className="grid grid-cols-2 xl:grid-cols-3 gap-2">
              {visibleCapabilities.map((provider) => (
                <ProviderCard
                  key={provider.id}
                  provider={provider}
                  isActive={selectedProviderId === provider.id}
                  onClick={() =>
                    onSelectProvider({
                      id: provider.id,
                      name: provider.name,
                      defaultBaseUrl: provider.defaultBaseUrl,
                    })
                  }
                  variant="compact"
                />
              ))}
            </div>
          </div>
        </div>

        {/* Right Panel - Config */}
        <div className="w-100 shrink-0 p-8 overflow-y-auto">
          <div className="space-y-1 mb-6">
            <h1 className="text-xl font-bold text-white">
              {selectedProvider ? `Configure ${selectedProvider.name}` : "Choose a provider"}
            </h1>
            <p className="text-sm text-gray-400 leading-relaxed">
              {selectedProvider
                ? "Enter your API key to enable AI chat functionality."
                : "Select a provider from the list to get started."}
            </p>
          </div>

          {showForm && selectedProvider ? (
            <ProviderConfigForm
              selectedProviderId={selectedProviderId}
              selectedProviderName={selectedProvider.name}
              label={label}
              apiKey={apiKey}
              baseUrl={baseUrl}
              config={config}
              testResult={testResult}
              isTesting={isTesting}
              isSubmitting={isSubmitting}
              canTest={canTest}
              canSave={canSave}
              onLabelChange={onLabelChange}
              onApiKeyChange={onApiKeyChange}
              onBaseUrlChange={onBaseUrlChange}
              onConfigChange={onConfigChange}
              onTestConnection={onTestConnection}
              onSave={onSave}
            />
          ) : (
            <div className="rounded-xl border border-dashed border-white/20 bg-white/5 p-6 text-center">
              <p className="text-sm text-gray-500">Select a provider to configure</p>
            </div>
          )}
        </div>
      </div>
    );
  }

  // Mobile Layout
  return (
    <div className="flex flex-col items-center pb-8">
      {/* Title */}
      <div className="text-center space-y-2 mb-8">
        <h1 className="text-2xl font-bold text-white">Choose your AI provider</h1>
        <p className="text-sm text-gray-400 max-w-sm leading-relaxed">
          Select an AI provider to get started. Your API keys are securely encrypted on your device.
          No account signup needed.
        </p>
      </div>

      {/* Provider Selection */}
      <div className="w-full max-w-2xl mb-8">
        <div className="grid grid-cols-2 gap-3">
          {visibleCapabilities.map((provider) => (
            <ProviderCard
              key={provider.id}
              provider={provider}
              isActive={selectedProviderId === provider.id}
              onClick={() =>
                onSelectProvider({
                  id: provider.id,
                  name: provider.name,
                  defaultBaseUrl: provider.defaultBaseUrl,
                })
              }
              variant="standard"
            />
          ))}
        </div>
      </div>

      {/* Configuration Form */}
      <div
        id="provider-config-form"
        className={`config-form-section w-full max-w-sm transition-all duration-300 ${showForm ? "opacity-100 max-h-500" : "opacity-0 max-h-0 overflow-hidden pointer-events-none"}`}
      >
        <div className="text-center space-y-2 mb-6">
          <h2 className="text-lg font-semibold text-white">Connect {selectedProvider?.name}</h2>
          <p className="text-xs text-gray-400 leading-relaxed">
            Paste your API key below to enable chats. Need a key? Get one from the provider
            dashboard.
          </p>
        </div>

        {selectedProvider && (
          <ProviderConfigForm
            selectedProviderId={selectedProviderId}
            selectedProviderName={selectedProvider.name}
            label={label}
            apiKey={apiKey}
            baseUrl={baseUrl}
            config={config}
            testResult={testResult}
            isTesting={isTesting}
            isSubmitting={isSubmitting}
            canTest={canTest}
            canSave={canSave}
            onLabelChange={onLabelChange}
            onApiKeyChange={onApiKeyChange}
            onBaseUrlChange={onBaseUrlChange}
            onConfigChange={onConfigChange}
            onTestConnection={onTestConnection}
            onSave={onSave}
          />
        )}
      </div>
    </div>
  );
}
