import { openExternalUrl } from "./openExternal";

export const BASE_DOCS_URL = "https://www.lettuceai.app/docs";

interface Docs {
    installation: string;
    quickStart: string;
    apiKeys: string;
    providers: string;
    models: string;
    imagegen: string;
    accessibility: string;
    characters: string;
    personas: string;
    systemPrompts: string;
    memorySystem: string;
    lorebooks: string;
    sync: string;
    helpMeReply: string;
    textToSpeech: string;
    creationHelper: string;
    faq: string;
}

type DocsKey = keyof Docs;

export const DOCS: Docs = {
    installation: `${BASE_DOCS_URL}/installation`,
    quickStart: `${BASE_DOCS_URL}/quickstart`,
    apiKeys: `${BASE_DOCS_URL}/api-keys`,
    providers: `${BASE_DOCS_URL}/providers`,
    models: `${BASE_DOCS_URL}/models`,
    imagegen: `${BASE_DOCS_URL}/images`,
    accessibility: `${BASE_DOCS_URL}/accessibility`,
    characters: `${BASE_DOCS_URL}/characters`,
    personas: `${BASE_DOCS_URL}/personas`,
    systemPrompts: `${BASE_DOCS_URL}/system-prompts`,
    memorySystem: `${BASE_DOCS_URL}/memory`,
    lorebooks: `${BASE_DOCS_URL}/lorebooks`,
    sync: `${BASE_DOCS_URL}/sync`,
    helpMeReply: `${BASE_DOCS_URL}/help-me-reply`,
    textToSpeech: `${BASE_DOCS_URL}/tts`,
    creationHelper: `${BASE_DOCS_URL}/creation-helper`,
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