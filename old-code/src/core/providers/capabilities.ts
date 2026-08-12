import { invoke } from "@tauri-apps/api/core";

export type ProviderCapabilities = {
    id: string;
    name: string;
    default_base_url: string;
    api_endpoint_path: string;
    system_role: string;
    supports_stream: boolean;
    requires_api_key: boolean;
    required_auth_headers: string[];
    default_headers: Record<string, string>;
};

export async function getProviderCapabilities(): Promise<ProviderCapabilities[]> {
    return invoke<ProviderCapabilities[]>("get_provider_configs");
}

export type ProviderCapabilitiesCamel = {
    id: string;
    name: string;
    defaultBaseUrl: string;
    apiEndpointPath: string;
    systemRole: string;
    supportsStream: boolean;
    requiresApiKey: boolean;
    requiredAuthHeaders: string[];
    defaultHeaders: Record<string, string>;
};

export function toCamel(c: ProviderCapabilities): ProviderCapabilitiesCamel {
    return {
        id: c.id,
        name: c.name,
        defaultBaseUrl: c.default_base_url,
        apiEndpointPath: c.api_endpoint_path,
        systemRole: c.system_role,
        supportsStream: c.supports_stream,
        requiresApiKey: c.requires_api_key,
        requiredAuthHeaders: c.required_auth_headers,
        defaultHeaders: c.default_headers,
    };
}
