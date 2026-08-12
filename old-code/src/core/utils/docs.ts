import { openExternalUrl } from "./openExternal";

export const BASE_DOCS_URL = "https://www.lettuceai.app/docs";

interface Docs {
    aiBasics: string;
    installation: string;
    quickStart: string;
    apiKeys: string;
    providers: string;
    models: string;
    modelBrowser: string;
    ollama: string;
    hostApi: string;
    imagegen: string;
    accessibility: string;
    characters: string;
    chatTemplates: string;
    personas: string;
    groupChats: string;
    discovery: string;
    systemPrompts: string;
    companionMode: string;
    memorySystem: string;
    lorebooks: string;
    sync: string;
    helpMeReply: string;
    textToSpeech: string;
    speechRecognition: string;
    chatWidgets: string;
    branching: string;
    smartCreator: string;
    security: string;
    usage: string;
    backupRestore: string;
    faq: string;
}

type DocsKey = keyof Docs;

export const DOCS: Docs = {
    aiBasics: `${BASE_DOCS_URL}/ai-basics`,
    installation: `${BASE_DOCS_URL}/installation`,
    quickStart: `${BASE_DOCS_URL}/quickstart`,
    apiKeys: `${BASE_DOCS_URL}/api-keys`,
    providers: `${BASE_DOCS_URL}/providers`,
    models: `${BASE_DOCS_URL}/models`,
    modelBrowser: `${BASE_DOCS_URL}/model-browser`,
    ollama: `${BASE_DOCS_URL}/ollama`,
    hostApi: `${BASE_DOCS_URL}/host-api`,
    imagegen: `${BASE_DOCS_URL}/images`,
    accessibility: `${BASE_DOCS_URL}/accessibility`,
    characters: `${BASE_DOCS_URL}/characters`,
    chatTemplates: `${BASE_DOCS_URL}/chat-templates`,
    personas: `${BASE_DOCS_URL}/personas`,
    groupChats: `${BASE_DOCS_URL}/group-chats`,
    discovery: `${BASE_DOCS_URL}/discovery`,
    systemPrompts: `${BASE_DOCS_URL}/system-prompts`,
    companionMode: `${BASE_DOCS_URL}/companion-mode`,
    memorySystem: `${BASE_DOCS_URL}/memory`,
    lorebooks: `${BASE_DOCS_URL}/lorebooks`,
    sync: `${BASE_DOCS_URL}/sync`,
    helpMeReply: `${BASE_DOCS_URL}/help-me-reply`,
    textToSpeech: `${BASE_DOCS_URL}/tts`,
    speechRecognition: `${BASE_DOCS_URL}/speech-recognition`,
    chatWidgets: `${BASE_DOCS_URL}/chat-widgets`,
    branching: `${BASE_DOCS_URL}/branching`,
    smartCreator: `${BASE_DOCS_URL}/smart-creator`,
    security: `${BASE_DOCS_URL}/security`,
    usage: `${BASE_DOCS_URL}/usage`,
    backupRestore: `${BASE_DOCS_URL}/backup-restore`,
    faq: `https://www.lettuceai.app/faq`,
}

export async function openDocs(key: DocsKey, section?: string) {

    if (!key || !(key in DOCS)) {
        console.error(`Invalid docs key: ${key}`);
        return;
    };

    let url = DOCS[key as DocsKey];
    if (!url) {
        console.error(`No docs found for key: ${key}`);
        return;
    };

    if (section) {
        url += `#${section}`;
    };

    await openExternalUrl(url);
};
