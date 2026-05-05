import type { DeepPartialMessageTree } from "../types";
import { type LocaleMessages } from "./en";
import type { LocaleMetadata } from ".";

export const itMetadata: LocaleMetadata = {
  name: "Italian",
  label: "Italiano",
};

export const itMessages: DeepPartialMessageTree<LocaleMessages> = {
  "common": {
    "nav": {
      "chats": "Chat",
      "settings": "Impostazioni",
      "providers": "Provider",
      "responseStyle": "Stile di risposta",
      "models": "Modelli",
      "security": "Sicurezza",
      "accessibility": "Accessibilità",
      "reset": "Ripristina",
      "backupRestore": "Backup e ripristino",
      "convertFiles": "Converti file",
      "usageAnalytics": "Analisi utilizzo",
      "changelog": "Registro modifiche",
      "about": "Informazioni",
      "createSystemPrompt": "Crea prompt di sistema",
      "editSystemPrompt": "Modifica prompt di sistema",
      "systemPrompts": "Prompt di sistema",
      "developer": "Sviluppatore",
      "advanced": "Avanzate",
      "characters": "Personaggi",
      "lorebooks": "Lorebook",
      "personas": "Persona",
      "dynamicMemory": "Memoria dinamica",
      "creationHelper": "Assistente alla creazione",
      "helpMeReply": "Aiutami a rispondere",
      "editPersona": "Modifica persona",
      "newTemplate": "Nuovo modello",
      "editTemplate": "Modifica modello",
      "chatTemplates": "Modelli di chat",
      "editCharacter": "Modifica personaggio",
      "sync": "Sincronizzazione",
      "newCharacter": "Nuovo personaggio",
      "engineSetup": "Configurazione engine",
      "llmProviders": "Provider LLM",
      "engineSettings": "Impostazioni engine",
      "lettuceEngine": "Lettuce Engine",
      "create": "Crea",
      "setup": "Configurazione",
      "welcome": "Benvenuto",
      "conversation": "Conversazione",
      "library": "Libreria",
      "groupChats": "Chat di gruppo",
      "groupChat": "Chat di gruppo",
      "imageGeneration": "Generazione immagini",
      "voices": "Voci",
      "chatAppearance": "Aspetto della chat",
      "colorCustomization": "Colori personalizzati",
      "embeddingDownload": "Download embedding",
      "embeddingTest": "Test embedding",
      "editModel": "Modifica modello",
      "messageDebug": "Debug dei messaggi",
      "speechRecognition": "Riconoscimento vocale",
      "hostApi": "Server API",
      "lorebookEntryGenerator": "Generatore voci lorebook",
      "companionSoulWriter": "Companion Soul Writer"
    },
    "bottomNav": {
      "chats": "Chat",
      "groups": "Gruppi",
      "create": "Crea",
      "discover": "Scopri",
      "library": "Libreria"
    },
    "buttons": {
      "cancel": "Annulla",
      "confirm": "Conferma",
      "save": "Salva",
      "saving": "Salvataggio...",
      "delete": "Elimina",
      "deleting": "Eliminazione...",
      "create": "Crea",
      "creating": "Creazione...",
      "edit": "Modifica",
      "back": "Indietro",
      "done": "Fatto",
      "download": "Scarica",
      "later": "Più tardi",
      "proceed": "Procedi",
      "retry": "Riprova",
      "discard": "Scarta",
      "import": "Importa",
      "importing": "Importazione...",
      "export": "Esporta",
      "exporting": "Esportazione...",
      "update": "Aggiorna",
      "generate": "Genera",
      "refresh": "Aggiorna",
      "continue": "Continua",
      "goBack": "Torna indietro",
      "search": "Cerca",
      "clearSearch": "Cancella ricerca",
      "add": "Aggiungi",
      "remove": "Rimuovi",
      "rename": "Rinomina",
      "copy": "Copia",
      "copied": "Copiato!",
      "browseFiles": "Sfoglia file",
      "install": "Installa"
    },
    "labels": {
      "processing": "Elaborazione...",
      "loading": "Caricamento...",
      "noDescriptionYet": "Nessuna descrizione",
      "untitled": "Senza titolo",
      "default": "Predefinito",
      "enabled": "Attivato",
      "disabled": "Disattivato",
      "on": "Attivo",
      "off": "Disattivo",
      "none": "Nessuno",
      "auto": "Auto",
      "custom": "Personalizzato",
      "name": "Nome",
      "description": "Descrizione",
      "content": "Contenuto",
      "preview": "Anteprima",
      "options": "Opzioni"
    },
    "time": {
      "justNow": "Adesso",
      "minutesAgo": "{{minutes}}m fa",
      "hoursAgo": "{{hours}}h fa",
      "daysAgo": "{{days}}g fa",
      "today": "Oggi",
      "yesterday": "Ieri",
      "last7Days": "Ultimi 7 giorni",
      "older": "Meno recenti"
    }
  },
  "settings": {
    "sections": {
      "intelligence": "Intelligenza",
      "experience": "Esperienza",
      "connectivity": "Connettività",
      "securityPrivacy": "Sicurezza e privacy",
      "supportInfo": "Supporto e info",
      "dangerZone": "Zona pericolosa",
      "developer": "Sviluppatore"
    },
    "items": {
      "providers": {
        "title": "Provider",
        "subtitle": "Connetti ai servizi AI"
      },
      "models": {
        "title": "Modelli",
        "subtitle": "Configura i modelli AI"
      },
      "imageGeneration": {
        "title": "Generazione di immagini",
        "subtitle": "Generare e testare immagini"
      },
      "voices": {
        "title": "Voci",
        "subtitle": "Voci text-to-speech"
      },
      "accessibility": {
        "title": "Accessibilità",
        "subtitle": "Suoni e feedback tattile"
      },
      "prompts": {
        "title": "Prompt di sistema",
        "subtitle": "Personalizza la personalità dell'AI"
      },
      "security": {
        "title": "Sicurezza",
        "subtitle": "Crittografia e privacy"
      },
      "backup": {
        "title": "Backup e ripristino",
        "subtitle": "Esporta o importa dati"
      },
      "convert": {
        "title": "Converti file",
        "subtitle": "Migra esportazioni .json legacy a .uec"
      },
      "sync": {
        "title": "Sincronizzazione locale",
        "subtitle": "Sincronizza tra dispositivi"
      },
      "usage": {
        "title": "Analisi utilizzo",
        "subtitle": "Costi e statistiche token"
      },
      "advanced": {
        "title": "Avanzate",
        "subtitle": "Memoria e funzionalità"
      },
      "logs": {
        "title": "Log",
        "subtitle": "Debug e diagnostica"
      },
      "guide": {
        "title": "Guida alla configurazione",
        "subtitle": "Ripeti l'onboarding"
      },
      "docs": {
        "title": "Documentazione",
        "subtitle": "Guide e riferimenti"
      },
      "github": {
        "title": "Segnala problemi",
        "subtitle": "Bug e feedback • v{{version}}"
      },
      "discord": {
        "title": "Unisciti a Discord",
        "subtitle": "Comunità e supporto"
      },
      "about": {
        "title": "Informazioni",
        "subtitle": "Versione, aggiornamenti e link"
      },
      "changelog": {
        "title": "Registro modifiche",
        "subtitle": "Novità"
      },
      "reset": {
        "title": "Ripristina",
        "subtitle": "Cancella tutto"
      },
      "developer": {
        "title": "Strumenti sviluppatore",
        "subtitle": "Debug e test"
      }
    }
  },
  "accessibility": {
    "sectionTitles": {
      "language": "Lingua",
      "sounds": "Feedback sonoro",
      "haptics": "Feedback tattile",
      "appearance": "Aspetto"
    },
    "language": {
      "appLanguage": "Lingua dell'app",
      "description": "Scegli la lingua utilizzata nella navigazione e nelle impostazioni."
    },
    "appearance": {
      "customColors": "Colori personalizzati",
      "customColorsDesc": "Personalizza la combinazione di colori dell'app",
      "chatAppearance": "Aspetto della chat",
      "chatAppearanceDesc": "Personalizza bolle dei messaggi, font e layout"
    },
    "haptics": {
      "vibrateOnChat": "Vibra durante la chat",
      "vibrateDesc": "Brevi impulsi di vibrazione mentre l'assistente scrive",
      "intensity": "Intensità",
      "light": "Leggera",
      "medium": "Media",
      "heavy": "Forte",
      "soft": "Morbida",
      "rigid": "Rigida"
    },
    "sounds": {
      "send": "Invio",
      "sendDescription": "Si riproduce quando invii un messaggio",
      "success": "Successo",
      "successDescription": "Si riproduce quando l'assistente completa con successo",
      "failure": "Errore",
      "failureDescription": "Si riproduce in caso di errore o interruzione",
      "testButton": "Test"
    },
    "feedbackInfo": "Il feedback ti aiuta a notare quando i messaggi vengono inviati o ricevuti.",
    "hapticsInfo": "Il feedback tattile è disponibile su dispositivi mobili.",
    "extra": {
      "easterEggs": "Easter Eggs",
      "beetrootRain": "Pioggia di barbabietole",
      "beetrootDesc": "Le barbabietole cadono quando le chat le menzionano"
    }
  },
  "topNav": {
    "unsavedChangesTitle": "Modifiche non salvate",
    "unsavedChangesMessage": "Salva o scarta le modifiche prima di uscire.",
    "goBack": "Torna indietro",
    "changeLayout": "Cambia layout",
    "search": "Cerca",
    "settings": "Impostazioni",
    "help": "Aiuto",
    "add": "Aggiungi",
    "openFilters": "Apri filtri",
    "save": "Salva",
    "extra": {
      "installedModels": "Modelli installati",
      "refresh": "Aggiorna",
      "minimize": "Riduci a icona",
      "maximize": "Ingrandisci",
      "close": "Chiudi"
    }
  },
  "updates": {
    "available": {
      "title": "Nuova versione disponibile",
      "description": "La v{{latestVersion}} è disponibile. Stai usando la v{{currentVersion}}.",
      "actions": {
        "view": "Vedi release"
      }
    }
  },
  "about": {
    "kicker": "Info app",
    "appName": "LettuceAI",
    "description": "Dettagli della versione, controlli degli aggiornamenti e link utili.",
    "info": {
      "version": "Versione",
      "channel": "Canale",
      "platform": "Piattaforma"
    },
    "buildChannel": {
      "dev": "Build di sviluppo",
      "release": "Release stabile"
    },
    "update": {
      "sectionTitle": "Aggiornamenti",
      "title": "Aggiornamenti dell'app",
      "description": "Controlla manualmente le nuove release o disattiva i controlli automatici all'avvio.",
      "autoChecks": "Controlli automatici degli aggiornamenti",
      "autoChecksDescription": "Quando è attivo, LettuceAI controlla le nuove versioni all'apertura dell'app.",
      "checkNow": "Controlla aggiornamenti",
      "checking": "Controllo aggiornamenti...",
      "upToDateTitle": "Sei aggiornato",
      "upToDateDescription": "Al momento non è disponibile una release più recente per questo dispositivo.",
      "failedTitle": "Controllo aggiornamenti non riuscito",
      "failedDescription": "Impossibile controllare gli aggiornamenti adesso. Riprova tra un momento."
    },
    "links": {
      "sectionTitle": "Link",
      "website": "Sito web",
      "websiteDescription": "Pagina di download e informazioni sulle release",
      "github": "GitHub",
      "githubDescription": "Codice sorgente, issue e sviluppo",
      "discord": "Discord",
      "discordDescription": "Server della community e supporto",
      "reddit": "Reddit",
      "redditDescription": "Discussioni della community, feedback e aggiornamenti"
    },
    "developerMode": {
      "enable": "Abilita modalità sviluppatore",
      "enabled": "Modalità sviluppatore abilitata"
    },
    "errors": {
      "saveTitle": "Impossibile salvare la preferenza",
      "saveDescription": "La preferenza per il controllo aggiornamenti non è stata modificata."
    }
  },
  "components": {
    "tooltip": {
      "dismissHint": "Tocca ovunque per chiudere"
    },
    "backgroundPosition": {
      "title": "Posiziona sfondo",
      "instructions": "Trascina per posizionare • Pizzica o scorri per ingrandire"
    },
    "avatarSource": {
      "generateImage": "Genera immagine",
      "generateImageDesc": "Creazione avatar con AI",
      "noImageModels": "Nessun modello di immagine disponibile",
      "editCurrent": "Modifica corrente",
      "editCurrentDesc": "Riposiziona o ritaglia",
      "chooseImage": "Scegli immagine",
      "chooseImageDesc": "Seleziona dal tuo dispositivo"
    },
    "avatarCurrentEdit": {
      "title": "Modifica corrente",
      "reposition": "Riposizionare",
      "repositionDesc": "Sposta o ritaglia l'avatar corrente",
      "editWithAI": "Modifica con l'intelligenza artificiale",
      "editWithAIDesc": "Apri la modifica AI per l'avatar corrente",
      "noImageModels": "Nessun modello di immagine disponibile"
    },
    "avatarGeneration": {
      "modelsLoadError": "Impossibile caricare i modelli di generazione immagini",
      "title": "Genera avatar",
      "help": "Aiuto per la generazione avatar",
      "model": "Modello",
      "selectModel": "Seleziona un modello",
      "describe": "Descrivi il tuo avatar",
      "describePlaceholder": "Una ragazza anime amichevole con capelli colorati, che sorride calorosamente...",
      "inProgress": "Generazione avatar in corso...",
      "editingInProgress": "Applicazione della modifica dell'avatar in corso...",
      "previousVariant": "Variante precedente",
      "nextVariant": "Prossima variante",
      "variantCounter": "{{current}} / {{total}}",
      "editRequest": "Modifica richiesta",
      "editRequestPlaceholder": "Rendi i capelli più scuri, aggiungi gli occhiali, mantieni il viso uguale...",
      "applyEdit": "Applica modifica",
      "editImageLoadError": "Impossibile preparare l'avatar generato per la modifica",
      "aiAssistant": "Assistente AI",
      "backToResults": "Torna al prompt",
      "magicInTheWorks": "Magia in atto...",
      "refine": "Perfeziona",
      "apply": "Fare domanda a",
      "alt": "Avatar generato",
      "regenerate": "Rigenera",
      "useThis": "Usa questo"
    },
    "avatarPosition": {
      "title": "Posiziona avatar",
      "instructions": "Trascina per posizionare • Pizzica o scorri per ingrandire",
      "alt": "Avatar da posizionare"
    },
    "confirmDialog": {
      "defaultTitle": "Conferma",
      "defaultLabel": "Conferma"
    },
    "bottomMenu": {
      "defaultTitle": "Menu",
      "dragTip": "Trascina per chiudere il menu",
      "closeLabel": "Chiudi menu",
      "buttonProcessing": "Elaborazione..."
    },
    "modelSelector": {
      "placeholder": "Seleziona un modello",
      "clearLabel": "Usa predefinito globale",
      "loading": "Caricamento modelli...",
      "noModels": "Nessun modello disponibile",
      "addProviderFirst": "Aggiungi prima un provider nelle impostazioni",
      "title": "Seleziona modello",
      "searchPlaceholder": "Cerca modelli...",
      "noResults": "Nessun modello trovato",
      "noResultsHint": "Prova un termine di ricerca diverso"
    },
    "localeSelector": {
      "title": "Seleziona lingua"
    },
    "promptTemplate": {
      "nameContentRequired": "Nome e contenuto sono obbligatori",
      "saveError": "Impossibile salvare il modello",
      "editTitle": "Modifica prompt",
      "createTitle": "Crea prompt",
      "name": "Nome",
      "namePlaceholder": "es. Maestro di roleplay",
      "content": "Contenuto",
      "variablesButton": "Variabili",
      "contentPlaceholder": "Sei un assistente AI utile...\n\nUsa {{char.name}} e {{scene}} nel tuo prompt.",
      "characterCount": "{{count}} caratteri",
      "preview": "Anteprima",
      "characterPlaceholder": "Personaggio…",
      "personaPlaceholder": "Persona…",
      "rendering": "Rendering…",
      "noPreview": "Nessuna anteprima",
      "saving": "Salvataggio...",
      "update": "Aggiorna",
      "create": "Crea",
      "variablesTitle": "Variabili del modello",
      "variablesCopyHint": "Tocca per copiare negli appunti",
      "variablesCopied": "Copiato",
      "variables": {
        "charName": "Nome personaggio",
        "charNameDesc": "Nome del personaggio",
        "charDesc": "Desc. personaggio",
        "charDescDesc": "Descrizione del personaggio",
        "scene": "Scena",
        "sceneDesc": "Scena/scenario iniziale",
        "userName": "Nome utente",
        "userNameDesc": "Nome persona dell'utente",
        "userDesc": "Desc. utente",
        "userDescDesc": "Descrizione persona dell'utente",
        "rules": "Regole",
        "rulesDesc": "Regole comportamentali del personaggio",
        "contextSummary": "Riepilogo contesto",
        "contextSummaryDesc": "Riepilogo dinamico della conversazione",
        "keyMemories": "Ricordi chiave",
        "keyMemoriesDesc": "Lista dei ricordi rilevanti"
      }
    },
    "characterExport": {
      "title": "Formato di esportazione",
      "selectFormat": "Seleziona un formato",
      "loading": "Caricamento formati...",
      "formatUecDesc": "Formato Unified Entity Card (.uec) (consigliato).",
      "formatLegacyJsonDesc": "JSON legacy (solo importazione).",
      "formatV3Desc": "Character Card V3 JSON (specifica più recente).",
      "formatV2Desc": "Character Card V2 JSON (specifica Tavern).",
      "formatV1Desc": "Character Card V1 (solo importazione)."
    },
    "embeddingDownload": {
      "downloadRequired": "Download necessario",
      "modelRequired": "Modello di embedding richiesto",
      "description": "La memoria dinamica richiede il download di un modello di embedding locale (~260 MB) per funzionare.",
      "localStorage": "• Il modello verrà salvato localmente sul tuo dispositivo",
      "downloadSize": "• Dimensione download: circa 260 MB",
      "summarization": "• Necessario per il riassunto delle conversazioni"
    },
    "embeddingUpgrade": {
      "title": "Modello di embedding v4 disponibile",
      "v1Message": "Stai usando v1 con 512 token. Aggiorna a v4 per una qualità della memoria migliore e supporto per contesti lunghi.",
      "v2Message": "Stai usando il legacy v2. Aggiorna a v4 per una qualità della memoria migliore con l'ultimo modello di embedding.",
      "v3Message": "v4 migliora drasticamente il richiamo della memoria per il roleplay rispetto a v3 (recall@1 0.02 -> 0.92). L'aggiornamento è consigliato.",
      "button": "Aggiorna a v4"
    },
    "v2UpgradeToast": {
      "title": "Modello memoria v3",
      "badge": "Disponibile",
      "message": "Qualità di embedding migliorata rispetto a v2",
      "dismiss": "Chiudi",
      "upgrade": "Aggiorna"
    },
    "v1UpgradeToast": {
      "title": "Modello memoria v4 disponibile",
      "message": "Aggiorna per una qualità della memoria migliore e supporto per contesti lunghi.",
      "dismiss": "Chiudi",
      "upgrade": "Aggiorna"
    },
    "v3UpgradeToast": {
      "title": "Modello memoria v4",
      "badge": "Disponibile",
      "message": "v4 migliora drasticamente il richiamo della memoria per il roleplay rispetto a v3 (recall@1 0.02 -> 0.92). L'aggiornamento è consigliato.",
      "dismiss": "Più tardi",
      "upgrade": "Aggiorna"
    },
    "createMenu": {
      "title": "Crea nuovo",
      "smartCreator": "Creatore intelligente",
      "smartCreatorDesc": "Lascia che l'assistente guidi la tua creazione",
      "divider": "Oppure crea manualmente",
      "character": "Personaggio",
      "characterDesc": "Crea un personaggio personalizzato",
      "persona": "Persona",
      "personaDesc": "Crea una voce riutilizzabile",
      "groupChat": "Chat di gruppo",
      "groupChatDesc": "Chatta con più personaggi",
      "lorebook": "Lorebook",
      "lorebookDesc": "Costruisci il tuo riferimento mondiale",
      "characterSmartDesc": "Costruisci un personaggio con creazione guidata",
      "personaSmartDesc": "Crea una voce o personalità riutilizzabile",
      "lorebookSmartDesc": "Costruisci un riferimento mondiale strutturato",
      "loadingConversations": "Caricamento conversazioni...",
      "createNew": "Crea nuovo",
      "createNewDesc": "Inizia una nuova chat di creazione guidata",
      "editExisting": "Modifica esistente",
      "continueLast": "Continua l'ultima conversazione",
      "seeOlder": "Vedi precedenti",
      "seeOlderDesc": "Apri qualsiasi conversazione precedente del Creatore intelligente",
      "noConversations": "Nessuna conversazione per questo creatore.",
      "sessionCompleted": "Completata",
      "sessionCancelled": "Annullata",
      "sessionDraft": "Bozza",
      "sessionMessages": "{{count}} messaggi",
      "untitledConversation": "Conversazione senza titolo",
      "nameLorebookTitle": "Nome lorebook",
      "lorebookNamePlaceholder": "Inserisci il nome del lorebook...",
      "lorebookImporting": "Importazione...",
      "lorebookImport": "Importa",
      "lorebookCreating": "Creazione...",
      "lorebookCreate": "Crea",
      "editExistingDesc": "Scegli un {{goal}} e modificalo con il Creatore intelligente",
      "creatorTitle": "Creatore {{goal}}",
      "editTitle": "Modifica {{goal}}",
      "conversationsTitle": "Conversazioni {{goal}}",
      "loadingItems": "Caricamento {{items}}...",
      "noItemsFound": "Nessun {{items}} trovato.",
      "unnamedCharacter": "Personaggio senza nome",
      "untitledPersona": "Persona senza titolo",
      "untitledLorebook": "Lorebook senza titolo"
    },
    "advancedModelSettings": {
      "temperature": "Temperatura",
      "temperatureDesc": "Più alta = più creativo",
      "topP": "Top P",
      "topPDesc": "Più basso = più focalizzato",
      "maxTokens": "Token di output massimi",
      "maxTokensDesc": "Lascia vuoto per il valore predefinito",
      "contextLength": "Lunghezza contesto",
      "contextLengthDesc": "Solo modelli locali",
      "contextLengthAuto": "Auto",
      "frequencyPenalty": "Penalità di frequenza",
      "frequencyPenaltyDesc": "Riduci la ripetizione dei token",
      "presencePenalty": "Penalità di presenza",
      "presencePenaltyDesc": "Incoraggia nuovi argomenti",
      "topK": "Top K",
      "topKDesc": "Limita la dimensione del pool di token",
      "reasoning": "Pensiero / Ragionamento",
      "reasoningAutoDesc": "Questo modello usa sempre il ragionamento. Nessuna configurazione necessaria.",
      "reasoningEnableDesc": "Abilita capacità di pensiero avanzate per la risoluzione di problemi complessi e compiti di ragionamento.",
      "effortMode": "Modalità sforzo",
      "budgetMode": "Modalità budget",
      "reasoningEffort": "Sforzo di ragionamento",
      "reasoningEffortDesc": "Controlla la profondità del pensiero",
      "reasoningEffortAuto": "Auto",
      "reasoningEffortLow": "Basso",
      "reasoningEffortMed": "Medio",
      "reasoningEffortHigh": "Alto",
      "reasoningBudget": "Budget ragionamento (token)",
      "reasoningBudgetDesc": "Token massimi riservati al pensiero",
      "reasoningEffortLowDesc": "Risposte rapide con ragionamento minimo",
      "reasoningEffortMediumDesc": "Profondità di ragionamento bilanciata",
      "reasoningEffortHighDesc": "Massima profondità di ragionamento per problemi complessi",
      "reasoningBudgetExtendedDesc": "Token massimi riservati al pensiero. Aggiunti al limite di output."
    },
    "providerParameterSupport": {
      "unknownProvider": "Provider sconosciuto: {{providerId}}",
      "reasoningNotSupportedEffort": "Non supportato - questo provider non usa livelli di sforzo",
      "reasoningNotSupportedBudgetOnly": "Non supportato - questo provider usa solo il budget",
      "reasoningNotSupported": "Non supportato - questo provider non supporta il ragionamento",
      "unsupportedParametersIgnored": "I parametri non supportati verranno ignorati da {{providerName}}.",
      "reasoningEffortSupported": "Lo sforzo di ragionamento è supportato per i modelli pensanti (o1, DeepSeek-R1, ecc.)",
      "reasoningBudgetSupported": "Questo provider usa il pensiero basato su budget (nessun livello di sforzo). Imposta invece i token del budget di ragionamento.",
      "reasoningNotSupportedProvider": "Questo provider non supporta i parametri di ragionamento.",
      "matrixTitle": "Matrice supporto parametri del provider",
      "providerColumn": "Provider",
      "supportedIndicator": "Supportato dall'API del provider",
      "notSupportedIndicator": "Non supportato (il parametro verrà ignorato)"
    },
    "extra": {
      "promptCachingTitle": "Prompt Caching",
      "promptCachingDescription": "Velocizza la generazione e riduce i costi per contesti lunghi e ripetitivi (come grandi prompt di sistema o cronologie di chat profonde).",
      "avatarAlt": "Avatar",
      "chooseFromLibrary": "Scegli dalla libreria",
      "chooseFromLibraryDesc": "Usa un'immagine già salvata nell'app",
      "generateFailed": "Generazione immagine fallita",
      "editFailed": "Modifica avatar fallita",
      "backgroundAlt": "Sfondo da posizionare",
      "formatsLoadFailed": "Caricamento formati di esportazione fallito",
      "formatsShowingDefaults": "(mostrando i predefiniti)",
      "timeJustNow": "adesso",
      "timeMinutesAgo": "{{minutes}}m fa",
      "timeHoursAgo": "{{hours}}h fa",
      "timeDaysAgo": "{{days}}g fa",
      "removeReference": "Rimuovi riferimento di design",
      "thumbLoading": "Caricamento...",
      "addReferences": "Aggiungi riferimenti",
      "visualDescription": "Descrizione visiva",
      "draftWithAi": "Bozza con AI",
      "regenerate": "Rigenera",
      "useThis": "Usa questo",
      "referenceImagesLabel": "Immagini di riferimento",
      "writerHelpFallback": "il modello di scrittura scena compatibile",
      "writerHelpUses": "Usa {{model}} per creare una bozza dal tuo avatar e dalle immagini di riferimento.",
      "writerHelpUnavailable": "Aggiungi un modello di scrittura scena compatibile nelle impostazioni di Generazione immagini per creare una bozza automaticamente.",
      "writerNotAvailableError": "Aggiungi prima un modello di scrittura scena compatibile nelle impostazioni di Generazione immagini.",
      "writerNoSourcesError": "Aggiungi un avatar o almeno un'immagine di riferimento prima di generare.",
      "noUsableReferences": "Nessuna immagine di riferimento utilizzabile trovata.",
      "draftFailed": "Generazione della bozza di design fallita.",
      "draftReadFailed": "Lettura della risorsa immagine fallita ({{status}})",
      "draftConvertFailed": "Conversione della risorsa immagine in URL dati fallita",
      "draftGenerationFailed": "Generazione della bozza fallita.",
      "draftMenuTitle": "Bozza di design AI",
      "draftedBy": "Bozza di {{model}} dall'avatar corrente e dalle immagini di riferimento.",
      "draftedByFallback": "il tuo modello di scrittura scena",
      "noWriterUseHelper": "Aggiungi un modello di scrittura scena compatibile prima di usare questo assistente.",
      "emptyReferences": "Aggiungi alcune immagini di riferimento chiare per fissare viso, proporzioni, outfit e stile.",
      "designReferencesTitle": "Riferimenti di design",
      "designReferencesDescription": "Carica alcune immagini di riferimento chiare e una descrizione visiva canonica.",
      "designReferencesPlaceholder": "Descrivi l'aspetto stabile: viso, capelli, corporatura, presentazione dell'eta, outfit, accessori e direzione artistica/stilistica.",
      "dismissAria": "Chiudi",
      "v3MessageFallback": "lettuce-emb-v4 e disponibile e migliora drasticamente il richiamo della memoria per il roleplay. L'aggiornamento e consigliato.",
      "uploadButton": "Carica",
      "libraryButton": "Libreria",
      "companionSetupTitle": "Companion richiede configurazione",
      "companionSetupSubtitleSingle": "La modalita Companion ha bisogno di un altro modello prima di poter funzionare. Saltare passa questo personaggio a Roleplay.",
      "companionSetupSubtitleMany": "La modalita Companion ha bisogno di altri {{count}} modelli prima di poter funzionare. Saltare passa questo personaggio a Roleplay.",
      "companionSetupBody": "La modalita Companion ha bisogno di alcuni modelli locali per analizzare le emozioni, estrarre entita, instradare i ricordi e richiamare il contesto passato.",
      "companionUseRoleplay": "Usa Roleplay invece",
      "companionDownloadNow": "Scarica ora",
      "searchModelsPlaceholder": "Cerca modelli...",
      "loadingModelsDefault": "Caricamento modelli...",
      "noModelsAvailable": "Nessun modello disponibile.",
      "noModelsMatching": "Nessun modello trovato per \"{{query}}\".",
      "contentPlaceholderText": "Sei un assistente AI utile...\n\nUsa {{char.name}} e {{scene}} nel tuo prompt.",
      "previewRenderFailed": "<anteprima non renderizzata>",
      "charactersCount": "{{count}} caratteri"
    }
  },
  "chats": {
    "characterNotFound": "Personaggio non trovato",
    "chatHistory": "Cronologia chat",
    "previousConversationsWithCharacter": "Conversazioni precedenti con {{name}}",
    "previousConversations": "Conversazioni precedenti",
    "searchChats": "Cerca chat...",
    "searchResults": "{{count}} risultato/i",
    "noConversationsYet": "Nessuna conversazione",
    "startChattingPrompt": "Inizia a chattare per vedere la cronologia qui",
    "noMatchingChats": "Nessuna chat corrispondente",
    "tryDifferentSearchTerm": "Prova un termine di ricerca diverso",
    "untitledChat": "Chat senza titolo",
    "chatTitlePlaceholder": "Titolo della chat...",
    "exportChatPackage": "Esporta pacchetto chat",
    "characterSpecificExport": "Esportazione specifica del personaggio",
    "characterSpecificExportDesc": "Associa questo pacchetto al personaggio della chat per impostazione predefinita.",
    "nonCharacterSpecificExport": "Esportazione non specifica",
    "nonCharacterSpecificExportDesc": "Richiedi la selezione del personaggio durante l'importazione.",
    "deleteChat": "Eliminare la chat?",
    "deleteConfirmDesc": "La rimuove permanentemente dalla cronologia",
    "keepThisChat": "Mantieni questa chat",
    "editCharacter": "Modifica personaggio",
    "exportCharacter": "Esporta personaggio",
    "chatAppearance": "Aspetto della chat",
    "importChatPackage": "Importa pacchetto chat",
    "hideThisCharacter": "Nascondi questo personaggio",
    "deleteCharacter": "Elimina personaggio",
    "deleteCharacterTitle": "Eliminare il personaggio?",
    "deleteCharacterConfirmation": "Sei sicuro di voler eliminare \"{{name}}\"? Questo eliminerà anche tutte le sessioni di chat con questo personaggio.",
    "characterSpecificMatches": "Questo pacchetto è specifico del personaggio e corrisponde al personaggio selezionato.",
    "characterSpecificMismatch": "Questo pacchetto è specifico di un altro personaggio. Verrà importato in {{name}}.",
    "nonCharacterSpecificImport": "Questo pacchetto non è specifico di un personaggio. Verrà importato in {{name}}.",
    "noCharactersYet": "Nessun personaggio",
    "createFirstCharacter": "Crea il tuo primo personaggio dal pulsante + in basso per iniziare a chattare",
    "scrollToBottom": "Scorri in basso",
    "selectCharacter": "Seleziona personaggio",
    "branchToGroupChat": "Dirama in chat di gruppo",
    "addContent": "Aggiungi contenuto",
    "swapPlaces": "Scambia posti",
    "swapPlacesOn": "Scambia posti (attivo)",
    "uploadImage": "Carica immagine",
    "helpMeReply": "Aiutami a rispondere",
    "sceneImage": {
      "modeTitle": "Immagine della scena",
      "modeDescription": "Scegli se scrivere tu stesso il prompt della scena o lasciare che sia l'intelligenza artificiale a disegnarla prima.",
      "writePrompt": "Scrivi richiesta",
      "writePromptDesc": "Digita l'esatta richiesta dell'immagine della scena che desideri utilizzare.",
      "askAi": "Chiedi all'AI",
      "askAiDesc": "Consenti al modello di chat corrente di elaborare un suggerimento di scena dal momento selezionato.",
      "generateTitle": "Genera immagine della scena",
      "regenerateTitle": "Rigenera l'immagine della scena",
      "aiTitle": "Richiesta scena AI",
      "promptLabel": "RICHIESTA DI SCENA",
      "promptPlaceholder": "Descrivi la scena, i personaggi, l'atmosfera, l'illuminazione, l'inquadratura e i dettagli importanti...",
      "suggestedPrompt": "Pronta suggerita",
      "regeneratePrompt": "Rigenerare",
      "editPrompt": "Modifica richiesta",
      "reviewTitle": "Controlla prompt scena",
      "denyPrompt": "Rifiuta",
      "acceptPrompt": "Accetta",
      "generateImage": "Genera immagine",
      "updateImage": "Aggiorna immagine"
    },
    "useMyTextAsBase": "Usa il mio testo come base",
    "writeNewReply": "Scrivi qualcosa di nuovo",
    "suggestedReply": "Risposta suggerita",
    "selectTwoCharactersMinimum": "Seleziona almeno 2 personaggi per una chat di gruppo.",
    "groupBranchCreated": "Ramo di gruppo creato! Reindirizzamento...",
    "memories": "Ricordi",
    "tools": "Strumenti",
    "pinned": "Fissati",
    "searchMemories": "Cerca ricordi...",
    "addMemory": "Aggiungi ricordo",
    "memoryActions": "Azioni ricordi",
    "pinnedMessages": "Messaggi fissati",
    "pinnedMessagesDesc": "Sempre inclusi nel contesto",
    "contextSummary": "Riepilogo contesto",
    "contextSummaryPlaceholder": "Breve riepilogo usato per mantenere il contesto coerente tra i messaggi...",
    "memoryContentPlaceholder": "Cosa dovrebbe essere ricordato?",
    "editMemoryPlaceholder": "Inserisci il contenuto del ricordo...",
    "togglePin": {
      "pin": "Fissa",
      "unpin": "Sblocca"
    },
    "toggleMemoryState": {
      "setHot": "Imposta caldo",
      "setCold": "Imposta freddo"
    },
    "selectModelForRetry": "Seleziona modello per riprovare",
    "memoryCategories": {
      "characterTrait": "tratto del personaggio",
      "relationship": "relazione",
      "plotEvent": "evento della trama",
      "worldDetail": "dettaglio del mondo",
      "preference": "preferenza",
      "other": "altro"
    },
    "settings": {
      "memorySection": "Memoria",
      "memorySectionDesc": "Riepilogo, tag, cronologia chiamate strumenti",
      "quickSettings": "Impostazioni rapide",
      "quickSettingsDesc": "Regolazioni più comuni",
      "persona": "Persona",
      "model": "Modello",
      "fallbackModel": "Modello di riserva",
      "voice": "Voce",
      "voiceDesc": "Riproduzione text-to-speech",
      "advanced": "Avanzate",
      "advancedDesc": "Sovrascrivi i parametri del modello per questa sessione",
      "session": "Sessione",
      "sessionDesc": "Inizia nuove chat e sfoglia la cronologia",
      "newChat": "Nuova chat",
      "newChatDesc": "Inizia una nuova conversazione",
      "chatHistoryDesc": "Visualizza le sessioni precedenti",
      "importChatPackageDesc": "Importa un .chatpkg in questo personaggio",
      "selectModel": "Seleziona modello",
      "selectFallbackModel": "Seleziona modello di riserva",
      "personaActions": "Azioni persona",
      "sessionAdvancedSettings": "Impostazioni avanzate della sessione",
      "parameterSupport": "Supporto parametri",
      "backToChat": "Torna alla chat",
      "chatSettingsTitle": "Impostazioni chat",
      "chatSettingsSubtitle": "Gestisci le preferenze della conversazione",
      "modelDefaults": "Valori predefiniti del modello",
      "appDefaults": "Valori predefiniti dell'app",
      "openChatSessionFirst": "Apri prima una sessione di chat",
      "sessionRequired": "Sessione richiesta",
      "noPersona": "Nessuna persona",
      "customPersona": "Persona personalizzata",
      "noModelAvailable": "Nessun modello disponibile",
      "fallbackNone": "Nessuno",
      "unknownModel": "Modello sconosciuto",
      "authorNote": "Nota autore",
      "identityProfileAuthored": "Profilo identità creato",
      "addIdentityProfile": "Aggiungi profilo identità companion",
      "soulLabel": "Soul",
      "sessionTitle": "Sessione: {{title}}",
      "sessionUntitled": "Senza titolo",
      "messageCount": "{{count}} messaggi",
      "usingCharacterDefault": "Usa impostazione predefinita del personaggio",
      "sessionOverrideActive": "Override sessione attivo",
      "autoplayVoice": "Riproduzione automatica voce",
      "useCharacterDefault": "Usa impostazione predefinita del personaggio"
    },
    "search": {
      "placeholder": "Cerca nella conversazione...",
      "noMessagesFound": "Nessun messaggio trovato",
      "you": "Tu",
      "character": "Personaggio",
      "failed": "Ricerca messaggi fallita"
    },
    "templates": {
      "startWithTemplate": "Iniziare con un modello?",
      "noTemplate": "Nessun modello",
      "startWithSceneOnly": "Inizia solo con la scena",
      "you": "Tu",
      "bot": "Bot",
      "messageCount": "{{count}} messaggio/i"
    },
    "header": {
      "back": "Indietro",
      "openSettings": "Apri impostazioni chat",
      "manageMemories": "Gestisci ricordi",
      "searchMessages": "Cerca messaggi",
      "manageLorebooks": "Gestisci lorebook",
      "conversationSettings": "Impostazioni conversazione"
    },
    "footer": {
      "sendMessagePlaceholder": "Invia un messaggio...",
      "stopGeneration": "Ferma generazione",
      "sendMessage": "Invia messaggio",
      "continueConversation": "Continua conversazione",
      "moreOptions": "Più opzioni",
      "addImage": "Aggiungi immagine",
      "addImageAttachment": "Aggiungi allegato immagine",
      "removeAttachment": "Rimuovi allegato",
      "recordVoice": "Registra voce"
    },
    "message": {
      "thinkingMessages": {
        "thinkingReallyHard": "Sto pensando intensamente…",
        "lettuceCouncil": "Consultando il consiglio della lattuga…",
        "stealingThoughts": "Rubando pensieri dal vuoto…",
        "warmingBrainCells": "Scaldando le cellule cerebrali…",
        "forbiddenKnowledge": "Caricando conoscenze proibite…",
        "overthinking": "Pensando troppo (come al solito)…",
        "pretendingToBeSmart": "Fingendo di essere intelligente…",
        "crunchingNumbers": "Elaborando numeri immaginari…",
        "arguingWithMyself": "Discutendo con me stesso…",
        "askingUniverse": "Chiedendo gentilmente all'universo…"
      },
      "thoughtProcess": "Processo di pensiero",
      "regenerateResponse": "Rigenera risposta",
      "guidedRegenerationTitle": "Guida la rigenerazione",
      "guidedRegenerationLabel": "Come dovrebbe cambiare?",
      "guidedRegenerationDescription": "Descrivi il tono, la lunghezza, i dettagli da mantenere o rimuovere e cosa dovrebbe fare diversamente la prossima risposta.",
      "guidedRegenerationPlaceholder": "Rendilo più breve, più caldo, più diretto...",
      "guidedRegenerationSubmit": "Rigenera",
      "cancelAudioGeneration": "Annulla generazione audio",
      "stopAudio": "Ferma audio",
      "playMessageAudio": "Riproduci audio messaggio",
      "playAudio": "Riproduci audio",
      "sceneLabel": "Scena",
      "variantLabel": "Variante",
      "regenerating": "Rigenerazione",
      "assistantIsTyping": "L'assistente sta scrivendo",
      "attachedImage": "Immagine allegata"
    },
    "actions": {
      "assistantMessage": "Messaggio dell'assistente",
      "userMessage": "Messaggio dell'utente",
      "promptTokens": "Token del prompt",
      "completionTokens": "Token di completamento",
      "fallbackModelUsed": "Modello di riserva utilizzato",
      "total": "totale",
      "timeToFirstToken": "Tempo al primo token",
      "completionTokenSpeed": "Velocità token di completamento",
      "edit": "Modifica",
      "copy": "Copia",
      "pin": "Fissa",
      "unpin": "Sblocca",
      "rewindToHere": "Riavvolgi qui",
      "branchFromHere": "Dirama da qui",
      "branchToGroupChat": "Dirama in chat di gruppo",
      "branchToCharacter": "Dirama al personaggio",
      "generateSceneImage": "Genera l'immagine della scena",
      "regenerateSceneImage": "Rigenera l'immagine della scena",
      "chatAppearance": "Aspetto della chat",
      "delete": "Elimina",
      "debug": "Debug",
      "unpinToDelete": "Sblocca per eliminare",
      "editPlaceholder": "Modifica il tuo messaggio...",
      "memoriesUsed": "{{count}} ricordi utilizzati",
      "lorebookUsage": "Utilizzo lorebook",
      "lorebookUsageDesc": "Questa risposta ha utilizzato le seguenti voci del lorebook.",
      "matchScore": "Corrispondenza: {{score}}%",
      "unknownModel": "Modello sconosciuto",
      "loadingModel": "Caricamento modello..."
    },
    "emptyState": {
      "goBack": "Torna indietro"
    },
    "settingsDrawer": {
      "title": "Impostazioni chat",
      "subtitle": "Gestisci le preferenze della conversazione"
    },
    "history": {
      "archivedBadge": "Archiviato",
      "messagesCount": "{{count}} messaggi",
      "previousGroupPage": "Pagina precedente {{label}}",
      "nextGroupPage": "Pagina successiva {{label}}",
      "sillyTavernFormat": "Formato SillyTavern",
      "sillyTavernFormatDesc": "Esporta come .jsonl compatibile con SillyTavern.",
      "failedLoad": "Impossibile caricare i dati",
      "failedDelete": "Impossibile eliminare: {{error}}",
      "failedRename": "Impossibile rinominare: {{error}}",
      "chatPackageExportedTo": "Pacchetto chat esportato in:\n{{path}}",
      "sillyTavernExportedTo": "Chat SillyTavern esportata in:\n{{path}}",
      "failedExportChatPackage": "Impossibile esportare il pacchetto chat",
      "failedExportSillyTavern": "Impossibile esportare la chat SillyTavern"
    },
    "authorNote": {
      "title": "Nota autore",
      "inlineEditor": "Editor in linea",
      "inlineEditorDesc": "Mostra la nota autore sopra l'input della chat per modifiche rapide.",
      "toggleInlineEditor": "Attiva/disattiva l'editor della nota autore in linea",
      "placeholder": "Direzione privata per questa chat",
      "willBeRemoved": "La nota autore verrà rimossa al salvataggio",
      "noNoteSaved": "Nessuna nota autore salvata",
      "charactersCount": "{{count}} caratteri",
      "clear": "Cancella",
      "save": "Salva",
      "saving": "Salvataggio...",
      "failedSave": "Impossibile salvare la nota autore",
      "addPlaceholder": "Aggiungi una nota autore...",
      "savingShort": "Salvataggio..."
    },
    "drawer": {
      "chatSettingsTitle": "Impostazioni chat",
      "chatSettingsSubtitle": "Gestisci le preferenze della conversazione"
    },
    "companionSoul": {
      "loading": "Caricamento Companion Soul...",
      "unavailable": "Companion Soul non disponibile",
      "unavailableDesc": "La modifica del soul è disponibile solo per personaggi in modalità companion.",
      "pageTitle": "Companion Soul",
      "back": "Indietro",
      "save": "Salva"
    },
    "companionRelationship": {
      "back": "Indietro",
      "loading": "Caricamento stato relazione...",
      "unavailableTitle": "Lo stato della relazione non è disponibile",
      "sessionLoadFailed": "Impossibile caricare la sessione di chat.",
      "backToChat": "Torna alla chat",
      "notCompanionTitle": "Questa chat non è in modalità companion",
      "notCompanionDesc": "Le pagine delle relazioni companion vengono visualizzate solo per le chat il cui personaggio è in modalità companion.",
      "openRegularMemories": "Apri ricordi normali",
      "pageTitle": "Stato della relazione",
      "memoryButton": "Ricordi",
      "lastInteraction": "Ultima interazione {{time}}",
      "bond": "Legame",
      "closeness": "Vicinanza",
      "trust": "Fiducia",
      "affection": "Affetto",
      "tension": "Tensione",
      "stability": "Stabilità",
      "interactions": "Interazioni",
      "vsDefaults": "rispetto ai valori predefiniti del personaggio",
      "updatedAt": "Aggiornato {{time}}",
      "emotionalEngine": "Motore emotivo",
      "felt": "Sentito",
      "feltDesc": "Effetto interno",
      "expressed": "Espresso",
      "expressedDesc": "Emerge nelle risposte",
      "blocked": "Bloccato",
      "blockedDesc": "Soppresso dalla persona",
      "momentum": "Momentum",
      "momentumDesc": "Tendenza nei turni recenti",
      "activeDrivers": "Driver attivi",
      "soul": "Soul",
      "essence": "Essenza",
      "voice": "Voce",
      "relationalStyle": "Stile relazionale",
      "vulnerabilities": "Vulnerabilità",
      "habits": "Abitudini",
      "boundaries": "Limiti",
      "eventsCount": "{{count}} eventi",
      "recentTimeline": "Timeline recente",
      "superseded": "superato",
      "promptScore": "Prompt {{score}}",
      "persistenceScore": "Persistenza {{score}}",
      "noTimeline": "Nessuna timeline ancora",
      "noTimelineDesc": "I ricordi di relazione, traguardi e istantanee emotive appariranno qui man mano che il companion impara dalle conversazioni.",
      "notAuthoredYet": "Non ancora creato.",
      "noSignal": "Nessun segnale."
    },
    "companionUi": {
      "relationship": "Relazione",
      "milestones": "Traguardi",
      "boundaries": "Limiti",
      "preferences": "Preferenze",
      "profile": "Profilo",
      "routines": "Routine",
      "episodes": "Episodi",
      "emotionalSnapshots": "Istantanee emotive",
      "unknownTime": "Sconosciuto",
      "justNow": "Adesso",
      "minutesAgo": "{{minutes}}m fa",
      "hoursAgo": "{{hours}}h fa",
      "daysAgo": "{{days}}g fa",
      "notSetYet": "Non ancora impostato",
      "missingCharacterId": "characterId mancante",
      "sessionNotFound": "Sessione non trovata",
      "failedLoadCompanion": "Impossibile caricare la sessione companion"
    },
    "chatPage": {
      "characterNotFound": "Personaggio non trovato",
      "characterDoesntExist": "Il personaggio che cerchi non esiste."
    },
    "debugPage": {
      "copy": "Copia"
    },
    "companionMemoryPage": {
      "backLabel": "Indietro",
      "refineMemoryPlaceholder": "Raffina il ricordo companion memorizzato",
      "superseded": "superato",
      "pinned": "fissato",
      "cold": "freddo"
    }
  },
  "groupChats": {
    "list": {
      "editGroup": "Modifica gruppo",
      "deleteGroup": "Elimina gruppo",
      "deleteConfirmTitle": "Eliminare la chat di gruppo?",
      "deleteConfirmMessage": "Sei sicuro di voler eliminare \"{{name}}\"? Questo eliminerà anche tutti i messaggi in questa chat di gruppo.",
      "noGroupChatsYet": "Nessuna chat di gruppo",
      "noGroupChatsDesc": "Crea la tua prima chat di gruppo dal pulsante + in basso per conversare con più personaggi contemporaneamente",
      "newChat": "Nuova chat",
      "openChat": "Apri chat",
      "chatSettings": "Impostazioni chat",
      "sessionCount": "{{count}} chat"
    },
    "create": {
      "invalidPackage": "Questo pacchetto non è un pacchetto di chat di gruppo.",
      "inspectPackageError": "Impossibile ispezionare il pacchetto della chat di gruppo",
      "importPackageError": "Impossibile importare il pacchetto della chat di gruppo",
      "importChatpkg": "Importa `.chatpkg`",
      "mapParticipantsTitle": "Mappa partecipanti",
      "selectLocalCharacter": "Seleziona il personaggio locale per questo partecipante.",
      "selectCharacterPlaceholder": "Seleziona personaggio...",
      "importChatPackageTitle": "Importa pacchetto chat",
      "importChatPackageDesc": "Questo importerà il `.chatpkg` selezionato come nuova sessione di gruppo.",
      "characterSelect": {
        "title": "Crea chat di gruppo",
        "subtitle": "Seleziona i personaggi per la tua conversazione di gruppo",
        "selected": "selezionati",
        "ready": "Pronto",
        "minRequired": "Min. 2 richiesti",
        "noCharactersYet": "Nessun personaggio",
        "noCharactersDesc": "Crea prima dei personaggi per iniziare una chat di gruppo",
        "continueToSetup": "Continua alla configurazione del gruppo"
      },
      "groupSetup": {
        "title": "Configurazione gruppo",
        "subtitle": "Configura le impostazioni della chat di gruppo",
        "chatType": "Tipo di chat",
        "conversation": "Conversazione",
        "casualChat": "Chat informale",
        "roleplay": "Roleplay",
        "withScenes": "Con scene",
        "conversationDesc": "Conversazione di gruppo informale senza scene iniziali",
        "roleplayDesc": "Scenario roleplay con scena iniziale e prompt immersivi",
        "speakerSelection": "Selezione oratore",
        "llm": "LLM",
        "aiPicks": "Scelta dell'AI",
        "heuristic": "Euristico",
        "scoreBased": "Basato su punteggio",
        "roundRobin": "A turno",
        "takeTurns": "A turni",
        "llmDesc": "Usa il tuo modello predefinito per scegliere chi parla (costa token)",
        "heuristicDesc": "Usa il bilanciamento della partecipazione e indizi contestuali (gratuito)",
        "roundRobinDesc": "I personaggi parlano a turno in ordine (gratuito)",
        "chatBackground": "Sfondo della chat",
        "optional": "(Opzionale)",
        "uploadBackground": "Carica immagine di sfondo",
        "backgroundDesc": "Imposta un'immagine di sfondo per questa chat di gruppo",
        "groupName": "Nome del gruppo",
        "removeBackground": "Rimuovi immagine di sfondo",
        "groupNameAutoGenerate": "Lascia vuoto per generare automaticamente dai nomi dei personaggi",
        "continueToScene": "Continua alla scena iniziale",
        "createGroupChat": "Crea chat di gruppo"
      },
      "startingScene": {
        "title": "Scena iniziale",
        "subtitle": "Imposta lo scenario iniziale per il tuo roleplay",
        "sceneSource": "Fonte della scena",
        "none": "Nessuna",
        "custom": "Personalizzata",
        "fromCharacter": "Da personaggio",
        "noneDesc": "Inizia senza una scena predefinita",
        "customDesc": "Scrivi il tuo scenario iniziale",
        "fromCharacterDesc": "Usa una scena da uno dei tuoi personaggi",
        "sceneContent": "Contenuto della scena",
        "sceneContentPlaceholder": "Descrivi la scena iniziale per questo roleplay...",
        "sceneReferenceTip": "Suggerimento: Digita {{@\" per riferire i personaggi",
        "selectScene": "Seleziona una scena",
        "sceneLabel": " — Scena",
        "copyToCustom": "Copia in personalizzata e modifica"
      }
    },
    "history": {
      "title": "Cronologia chat di gruppo",
      "subtitle": "Tutte le conversazioni di gruppo",
      "searchPlaceholder": "Cerca chat di gruppo...",
      "active": "Attive ({{count}})",
      "archived": "Archiviate ({{count}})",
      "noChatsYet": "Nessuna chat di gruppo",
      "noChatsDesc": "Crea una chat di gruppo per vedere la cronologia qui",
      "noMatchingChats": "Nessuna chat corrispondente",
      "noMatchingDesc": "Prova un termine di ricerca diverso",
      "deleteSessionTitle": "Eliminare la chat di gruppo?",
      "deleteSessionDesc": "La rimuove permanentemente dalla cronologia",
      "deleteSessionButton": "Elimina chat",
      "keepChat": "Mantieni questa chat"
    },
    "session": {
      "chatTitlePlaceholder": "Titolo della chat...",
      "newChat": "Nuova chat",
      "rename": "Rinomina",
      "unarchive": "Ripristina dall'archivio",
      "archive": "Archivia",
      "messageCount": "{{count}} messaggio/i"
    },
    "memories": {
      "tabMemories": "Ricordi",
      "tabPinned": "Fissati",
      "tabActivity": "Attività",
      "processing": "Elaborazione",
      "contextSummaryTitle": "Riepilogo contesto",
      "addContextSummaryPrompt": "Tocca per aggiungere un riepilogo del contesto...",
      "savedMemories": "Ricordi salvati",
      "resultsCount": "Risultati ({{count}})",
      "searchPlaceholder": "Cerca ricordi...",
      "addMemory": "Aggiungi ricordo",
      "noMemoriesYet": "Nessun ricordo",
      "noMemoriesDesc": "Tocca il pulsante Aggiungi sopra per crearne uno",
      "noMatchingMemories": "Nessun ricordo corrispondente",
      "noMatchingDesc": "Prova un termine di ricerca diverso",
      "sessionNotFound": "Sessione non trovata",
      "memoryActions": "Azioni ricordi",
      "tokens": "token",
      "cycle": "Ciclo",
      "accessed": "Accesso",
      "cold": "Freddo",
      "hot": "Caldo",
      "activityLog": "Registro attività",
      "events": "eventi",
      "run": "Esegui",
      "processingMemories": "L'AI sta organizzando i ricordi di gruppo...",
      "memoryCycleSuccess": "Ciclo di memoria elaborato con successo!",
      "memoryActionFailed": "Azione memoria fallita",
      "newMemoryUpdates": "Nuovi aggiornamenti della memoria disponibili",
      "noActivityYet": "Nessuna attività",
      "noActivityDesc": "Le chiamate strumenti appaiono quando l'AI gestisce i ricordi in modalità dinamica",
      "contextSummaryPlaceholder": "Breve riepilogo usato per mantenere il contesto coerente tra i messaggi...",
      "addMemoryTitle": "Aggiungi ricordo",
      "memoryPlaceholder": "Cosa dovrebbe essere ricordato?",
      "saveMemory": "Salva ricordo",
      "editMemoryTitle": "Modifica ricordo",
      "editMemoryPlaceholder": "Inserisci il contenuto del ricordo...",
      "edit": "Modifica",
      "pin": "Fissa",
      "unpin": "Sblocca",
      "setHot": "Imposta caldo",
      "setCold": "Imposta freddo"
    },
    "toolLog": {
      "created": "Creato",
      "deleted": "Eliminato",
      "pinned": "Fissato",
      "unpinned": "Sbloccato",
      "done": "Fatto",
      "pinnedBadge": "fissato",
      "softDelete": "eliminazione morbida",
      "memoryCycle": "Ciclo di memoria",
      "failedAt": "Fallito a:",
      "window": "Finestra",
      "justNow": "adesso",
      "minutesAgo": "{{count}}m fa",
      "hoursAgo": "{{count}}h fa",
      "yesterday": "ieri",
      "daysAgo": "{{count}}g fa",
      "revertingTitle": "Ripristino...",
      "revertCycleTitle": "Ripristina questo ciclo",
      "revertedAt": "Ripristinato {{time}}",
      "failedAtStage": "Fallito a: {{stage}}",
      "hideDebug": "Nascondi debug",
      "debug": "Debug",
      "windowRange": "Finestra {{start}}-{{end}}",
      "actionCreated": "Creato",
      "actionDeleted": "Eliminato",
      "actionPinned": "Fissato",
      "actionUnpinned": "Sbloccato",
      "actionDone": "Fatto",
      "badgePinned": "fissato",
      "badgeSoftDelete": "eliminazione morbida",
      "badgeUndone": "annullato",
      "badgeReverted": "ripristinato",
      "activityEmptyTitle": "Nessuna attività ancora",
      "activityEmptyDesc": "Le chiamate strumenti appaiono quando l'AI gestisce i ricordi in modalità dinamica"
    },
    "message": {
      "thinkingHard": "Sto pensando intensamente…",
      "thinkingLettuce": "Consultando il consiglio della lattuga…",
      "thinkingVoid": "Rubando pensieri dal vuoto…",
      "thinkingBrainCells": "Scaldando le cellule cerebrali…",
      "thinkingForbidden": "Caricando conoscenze proibite…",
      "thinkingOverthinking": "Pensando troppo (come al solito)…",
      "thinkingPretending": "Fingendo di essere intelligente…",
      "thinkingCrunching": "Elaborando numeri immaginari…",
      "thinkingArguing": "Discutendo con me stesso…",
      "thinkingUniverse": "Chiedendo gentilmente all'universo…",
      "thoughtProcess": "Processo di pensiero",
      "userAlt": "Utente",
      "assistantAlt": "Assistente",
      "regenerateResponse": "Rigenera risposta",
      "variantLabel": "Variante",
      "regenerating": "Rigenerazione",
      "cancelAudioGeneration": "Annulla generazione audio",
      "stopAudio": "Ferma audio",
      "playMessageAudio": "Riproduci audio messaggio",
      "playAudio": "Riproduci audio",
      "attachedImage": "Immagine allegata",
      "assistantIsTyping": "L'assistente sta scrivendo",
      "assistantTyping": "L'assistente sta scrivendo"
    },
    "header": {
      "back": "Indietro",
      "memories": "Ricordi",
      "settings": "Impostazioni",
      "characters": "personaggi"
    },
    "footer": {
      "mentionCharacter": "Menziona un personaggio",
      "noCharactersFound": "Nessun personaggio trovato",
      "moreOptions": "Più opzioni",
      "addImage": "Aggiungi immagine",
      "messagePlaceholder": "Messaggio... (@ per menzionare)",
      "stopGeneration": "Ferma generazione",
      "sendMessage": "Invia messaggio",
      "continueConversation": "Continua conversazione",
      "dismissError": "Ignora errore",
      "removeAttachment": "Rimuovi allegato",
      "tabToSelect": "Tab per selezionare"
    },
    "messageActions": {
      "characterMessage": "Messaggio del personaggio",
      "yourMessage": "Il tuo messaggio",
      "whyCharacterResponded": "Perché questo personaggio ha risposto",
      "total": "totale",
      "edit": "Modifica",
      "copy": "Copia",
      "regenerateWithDifferent": "Rigenera con un altro personaggio",
      "rewindToHere": "Riavvolgi qui",
      "unpinToDelete": "Sblocca per eliminare",
      "delete": "Elimina",
      "editPlaceholder": "Modifica il tuo messaggio...",
      "chooseCharacterTitle": "Scegli personaggio",
      "selectCharacterForRegeneration": "Seleziona quale personaggio dovrebbe rispondere:"
    },
    "settings": {
      "appDefault": "Predefinito dell'app",
      "selectPersona": "Seleziona persona",
      "noPersonas": "Nessuna persona disponibile",
      "noPersonasDesc": "Crea una persona nelle impostazioni per personalizzare le tue conversazioni.",
      "searchPersonas": "Cerca persona...",
      "noPersona": "Nessuna persona",
      "noPersonaDesc": "Continua senza una persona",
      "noPersonasFound": "Nessuna persona trovata",
      "noPersonasFoundDesc": "Prova un termine di ricerca diverso"
    },
    "groupSettings": {
      "title": "Modifica gruppo",
      "subtitle": "Aggiorna la configurazione predefinita per le sessioni future",
      "enterGroupName": "Inserisci nome del gruppo",
      "participant": "partecipante",
      "participants": "partecipanti",
      "uploading": "Caricamento...",
      "changeBackground": "Cambia sfondo",
      "addBackgroundImage": "Aggiungi immagine di sfondo",
      "removeBackground": "Rimuovi sfondo",
      "persona": "Persona",
      "personaSubtitle": "Persona predefinita per le nuove sessioni",
      "personaLabel": "Persona",
      "speakerSelection": "Selezione oratore",
      "speakerSubtitle": "Metodo predefinito per le nuove sessioni",
      "llm": "LLM",
      "aiPicks": "L'IA sceglie",
      "heuristic": "Euristico",
      "scoreBased": "Basato su punteggio",
      "roundRobin": "A turno",
      "takeTurns": "Alternato",
      "llmDesc": "Usa il tuo modello predefinito per scegliere chi parla (costa token)",
      "heuristicDesc": "Usa il bilanciamento della partecipazione e indizi del contesto (gratuito)",
      "roundRobinDesc": "I personaggi parlano a turno (gratuito)",
      "memoryMode": "Modalità memoria",
      "memorySubtitle": "Modalità memoria predefinita per le nuove sessioni",
      "manual": "Manuale",
      "manualDesc": "Gestisci le note da solo",
      "dynamic": "Dinamica",
      "dynamicDesc": "Richiamo automatico",
      "memoryDynamicInfo": "L'IA crea e recupera automaticamente ricordi dalle conversazioni",
      "memoryManualInfo": "Tu aggiungi e gestisci le note di memoria",
      "characters": "Personaggi",
      "participantsActive": "{{total}} partecipanti · {{active}} attivi",
      "add": "Aggiungi",
      "muted": "(silenziato)",
      "mutedByDefault": "Silenziato per impostazione predefinita",
      "activeByDefault": "Attivo per impostazione predefinita",
      "unmuteCharacter": "Riattiva audio personaggio",
      "muteCharacter": "Silenzia personaggio",
      "minTwoRequired": "Minimo 2 personaggi richiesti",
      "removeCharacter": "Rimuovi personaggio",
      "groupMinCharacters": "Un gruppo richiede almeno 2 personaggi",
      "mutedCharactersNote": "I personaggi silenziati vengono saltati dalla selezione automatica dell'oratore, ma possono comunque rispondere tramite `@menzione` esplicita.",
      "addCharacterTitle": "Aggiungi personaggio",
      "allCharactersInGroup": "Tutti i personaggi sono già in questo gruppo.",
      "removeCharacterTitle": "Rimuovere personaggio?",
      "removeCharacterConfirm": "Sei sicuro di voler rimuovere",
      "removeCharacterFrom": "dalle impostazioni predefinite del gruppo?",
      "removing": "Rimozione...",
      "remove": "Rimuovi"
    },
    "sessionSettings": {
      "subtitle": "Gestisci le preferenze della chat di gruppo",
      "enterGroupName": "Inserisci nome del gruppo",
      "participant": "partecipante",
      "participants": "partecipanti",
      "message": "messaggio",
      "messages": "messaggi",
      "uploading": "Caricamento...",
      "changeBackground": "Cambia sfondo",
      "addBackgroundImage": "Aggiungi immagine di sfondo",
      "removeBackground": "Rimuovi sfondo",
      "persona": "Persona",
      "personaSubtitle": "La tua identità in questa conversazione",
      "personaLabel": "Persona",
      "speakerSelection": "Selezione oratore",
      "speakerSubtitle": "Come viene scelto il prossimo oratore",
      "llm": "LLM",
      "aiPicks": "L'IA sceglie",
      "heuristic": "Euristico",
      "scoreBased": "Basato su punteggio",
      "roundRobin": "A turno",
      "takeTurns": "Alternato",
      "llmDesc": "Usa il tuo modello predefinito per scegliere chi parla (costa token)",
      "heuristicDesc": "Usa il bilanciamento della partecipazione e indizi del contesto (gratuito)",
      "roundRobinDesc": "I personaggi parlano a turno (gratuito)",
      "characters": "Personaggi",
      "participantsActive": "{{total}} partecipanti · {{active}} attivi",
      "add": "Aggiungi",
      "muted": "(silenziato)",
      "unmuteCharacter": "Riattiva audio personaggio",
      "muteCharacter": "Silenzia personaggio",
      "minTwoRequired": "Minimo 2 personaggi richiesti",
      "removeCharacter": "Rimuovi personaggio",
      "groupMinCharacters": "Una chat di gruppo richiede almeno 2 personaggi",
      "mutedCharactersNote": "I personaggi silenziati vengono saltati dalla selezione automatica dell'oratore, ma possono comunque rispondere tramite `@menzione` esplicita.",
      "data": "Dati",
      "dataSubtitle": "Esporta o importa conversazioni",
      "export": "Esporta",
      "exportDesc": "Salva come file condivisibile",
      "import": "Importa",
      "importDesc": "Carica una conversazione da un file",
      "conversation": "Conversazione",
      "conversationSubtitle": "Duplica o continua in una nuova chat",
      "duplicate": "Duplica",
      "duplicateDesc": "Copia questa chat con o senza messaggi",
      "branchTo1on1": "Dirama a 1-a-1",
      "branchTo1on1Desc": "Continua privatamente con un personaggio",
      "participation": "Partecipazione",
      "participationSubtitle": "Distribuzione degli interventi tra i personaggi",
      "addCharacterTitle": "Aggiungi personaggio",
      "allCharactersInGroup": "Tutti i personaggi sono già in questo gruppo.",
      "removeCharacterTitle": "Rimuovere personaggio?",
      "removeCharacterConfirm": "Sei sicuro di voler rimuovere",
      "removeCharacterFrom": "da questa chat di gruppo?",
      "removing": "Rimozione...",
      "remove": "Rimuovi",
      "cloneGroupTitle": "Clona gruppo",
      "withMessages": "Con messaggi",
      "withMessagesDesc": "Clona tutto inclusa la cronologia chat",
      "withoutMessages": "Senza messaggi",
      "withoutMessagesDesc": "Clona solo la configurazione (personaggi, scena iniziale)",
      "branchWithCharacterTitle": "Dirama con personaggio",
      "branchWithCharacterDesc": "Seleziona un personaggio per continuare come conversazione 1-a-1. Tutti i messaggi di questo gruppo verranno convertiti.",
      "continueWith": "Continua la conversazione con {{name}}",
      "exportChatPackageTitle": "Esporta pacchetto chat",
      "includeCharacterSnapshots": "Includi snapshot personaggi",
      "includeCharacterSnapshotsDesc": "Mantieni i dati dei personaggi nel pacchetto",
      "sessionOnly": "Solo sessione",
      "sessionOnlyDesc": "Esporta solo messaggi e metadati",
      "mapParticipantsTitle": "Mappa partecipanti",
      "selectLocalCharacter": "Seleziona il personaggio locale per questo partecipante.",
      "selectCharacterPlaceholder": "Seleziona personaggio...",
      "continue": "Continua",
      "importChatPackageTitle": "Importa pacchetto chat",
      "importChatPackageDesc": "Questo importerà il `.chatpkg` selezionato come nuova sessione di gruppo.",
      "importing": "Importazione..."
    },
    "page": {
      "selectingCharacter": "Selezione personaggio...",
      "sessionNotFound": "Sessione di gruppo non trovata",
      "backToGroupChats": "Torna alle chat di gruppo",
      "startConversation": "Inizia una conversazione con {{names}}",
      "scrollToBottom": "Scorri in basso",
      "addContent": "Aggiungi contenuto",
      "uploadImage": "Carica immagine",
      "helpMeReply": "Aiutami a rispondere",
      "helpMeReplyDesc": "Lascia che l'AI suggerisca cosa dire",
      "haveDraftPrompt": "Hai un messaggio in bozza. Come vuoi procedere?",
      "useMyTextAsBase": "Usa il mio testo come base",
      "useMyTextAsBaseDesc": "Espandi e migliora la tua bozza",
      "writeSomethingNew": "Scrivi qualcosa di nuovo",
      "writeSomethingNewDesc": "Genera una risposta originale",
      "suggestedReply": "Risposta suggerita",
      "reasoningBeforeWriting": "Ragionamento prima di scrivere la tua risposta...",
      "writingYourReply": "Scrittura della tua risposta...",
      "regenerate": "Rigenera",
      "useReply": "Usa risposta",
      "helpMeReplyFailedGeneric": "Aiutami a rispondere non riuscito.",
      "helpMeReplyFailedGenerate": "Aiutami a rispondere non è riuscito a generare una risposta.",
      "noAudioCaptured": "Nessun audio acquisito.",
      "noWhisperModel": "Nessun modello Whisper installato trovato. Installane uno nelle impostazioni Riconoscimento vocale.",
      "cancelRecording": "Annulla registrazione",
      "transcribing": "Trascrizione",
      "stopAndTranscribe": "Ferma e trascrivi",
      "recordVoice": "Registra voce",
      "learnCorrection": "Apprendi correzione:",
      "learning": "Apprendimento...",
      "learn": "Apprendi",
      "ignore": "Ignora",
      "groupChatFailed": "Chat di gruppo non riuscita.",
      "failedToCopy": "Impossibile copiare",
      "copied": "Copiato!",
      "cannotDeletePinned": "Impossibile eliminare il messaggio fissato. Sblocca prima il messaggio.",
      "failedToDelete": "Impossibile eliminare",
      "messageNotFound": "Messaggio non trovato",
      "cannotRewindPinned": "Impossibile riavvolgere: ci sono messaggi fissati dopo questo punto. Sbloccali prima.",
      "failedToRewind": "Impossibile riavvolgere",
      "failedToTogglePin": "Impossibile attivare/disattivare il pin",
      "messagePinned": "Messaggio fissato",
      "messageUnpinned": "Messaggio sbloccato",
      "failedToSave": "Impossibile salvare"
    },
    "memoriesPage": {
      "summarizingConversation": "Riepilogo conversazione",
      "analyzingMemories": "Analisi ricordi",
      "applyingChanges": "Applicazione modifiche",
      "organizingMemories": "Organizzazione ricordi",
      "retryingMemoryCycle": "Ripetendo ciclo di memoria...",
      "processingMemories": "Elaborazione ricordi...",
      "memorySystemError": "Errore sistema di memoria",
      "contextSummary": "Riepilogo contesto",
      "contextSummaryTitle": "Riepilogo contesto",
      "savedMemories": "Ricordi salvati",
      "resultsCount": "Risultati ({{count}})",
      "searchMemoriesPlaceholder": "Cerca ricordi...",
      "addMemory": "Aggiungi ricordo",
      "memoryActions": "Azioni ricordi",
      "clearSearch": "Cancella ricerca",
      "noMatchingMemories": "Nessun ricordo corrispondente",
      "noMemoriesYet": "Nessun ricordo ancora",
      "tryDifferentSearch": "Prova un termine di ricerca diverso",
      "tapAddToCreate": "Tocca il pulsante Aggiungi sopra per crearne uno",
      "pinnedMessages": "Messaggi fissati",
      "refresh": "Aggiorna",
      "noPinnedMessages": "Nessun messaggio fissato",
      "pinImportantDesc": "Fissa i messaggi importanti della chat di gruppo per mantenerli sempre nel contesto.",
      "assistant": "Assistente",
      "user": "Utente",
      "unpin": "Sblocca",
      "failedToUnpinMessage": "Impossibile sbloccare il messaggio",
      "activityLog": "Registro attività",
      "run": "Esegui",
      "addMemoryTitle": "Aggiungi ricordo",
      "editMemoryTitle": "Modifica ricordo",
      "memoryTitle": "Ricordo",
      "memoryPlaceholder": "Cosa dovrebbe essere ricordato?",
      "saveMemory": "Salva ricordo",
      "editMemoryPlaceholder": "Inserisci il contenuto del ricordo...",
      "saving": "Salvataggio...",
      "save": "Salva",
      "cancel": "Annulla",
      "contextSummaryPlaceholder": "Breve riepilogo usato per mantenere il contesto coerente tra i messaggi...",
      "contextSummaryPrompt": "Tocca per aggiungere un riepilogo del contesto...",
      "cycle": "Ciclo",
      "accessed": "Accesso",
      "cold": "Freddo",
      "hot": "Caldo",
      "tokens": "token",
      "pin": "Fissa",
      "setHot": "Imposta caldo",
      "setCold": "Imposta freddo",
      "edit": "Modifica",
      "delete": "Elimina",
      "failedToToggleMemPin": "Impossibile attivare/disattivare il pin",
      "failedToRemoveMemory": "Impossibile rimuovere il ricordo",
      "toolEventsCountAria": "eventi",
      "activityEmptyDesc": "Le chiamate strumenti appaiono quando l'AI gestisce i ricordi in modalità dinamica",
      "memoryCycleSuccess": "Ciclo di memoria elaborato con successo!"
    },
    "memoriesController": {
      "missingSessionId": "sessionId mancante",
      "sessionNotFound": "Sessione non trovata",
      "failedToLoadSession": "Impossibile caricare la sessione",
      "failedToUpdateTemperature": "Impossibile aggiornare la temperatura della memoria",
      "failedToSaveSummary": "Impossibile salvare il riepilogo",
      "cycleFailed": "Ciclo di memoria di gruppo non riuscito",
      "failedToAddMemory": "Impossibile aggiungere il ricordo",
      "failedToUpdateMemory": "Impossibile aggiornare il ricordo",
      "failedToRunCycle": "Impossibile eseguire il ciclo di memoria",
      "failedToRevertCycle": "Impossibile ripristinare il ciclo",
      "failedToRefresh": "Impossibile aggiornare",
      "failedToDismissError": "Impossibile ignorare l'errore",
      "failedToTogglePinned": "Impossibile attivare/disattivare il pin",
      "sessionNotLoaded": "Sessione non caricata",
      "revertCycleTitle": "Ripristinare questo ciclo?",
      "revertConfirm": "Ripristina"
    },
    "chatController": {
      "sessionNotFound": "Sessione di gruppo non trovata",
      "failedToLoadGroupChat": "Impossibile caricare la chat di gruppo",
      "requestFailed": "Richiesta chat di gruppo non riuscita",
      "failedToSendMessage": "Impossibile inviare il messaggio",
      "failedToRegenerate": "Impossibile rigenerare",
      "failedToContinue": "Impossibile continuare",
      "failedToCopy": "Impossibile copiare",
      "cannotDeletePinned": "Impossibile eliminare il messaggio fissato. Sblocca prima.",
      "failedToDelete": "Impossibile eliminare",
      "messageNotFound": "Messaggio non trovato",
      "cannotRewindPinned": "Impossibile riavvolgere: ci sono messaggi fissati dopo questo punto. Sbloccali prima.",
      "failedToRewind": "Impossibile riavvolgere",
      "failedToTogglePin": "Impossibile attivare/disattivare il pin",
      "messagePinned": "Messaggio fissato",
      "messageUnpinned": "Messaggio sbloccato",
      "failedToSave": "Impossibile salvare",
      "copied": "Copiato!"
    },
    "historyController": {
      "failedToLoadData": "Impossibile caricare i dati",
      "failedToDelete": "Impossibile eliminare: {{error}}",
      "failedToRename": "Impossibile rinominare: {{error}}",
      "failedToArchive": "Impossibile archiviare: {{error}}",
      "failedToUnarchive": "Impossibile ripristinare dall'archivio: {{error}}",
      "failedToDuplicate": "Impossibile duplicare"
    },
    "sessionSettingsController": {
      "failedToLoad": "Impossibile caricare le impostazioni della chat di gruppo",
      "noPersona": "Nessuna persona",
      "customPersona": "Persona personalizzata",
      "minOneActive": "Almeno un partecipante deve rimanere attivo"
    },
    "groupSettingsController": {
      "notFound": "Gruppo non trovato",
      "failedToLoad": "Impossibile caricare le impostazioni del gruppo"
    },
    "createForm": {
      "failedToLoadCharacters": "Impossibile caricare i personaggi",
      "selectAtLeastTwo": "Seleziona almeno 2 personaggi per una chat di gruppo",
      "failedToCreate": "Impossibile creare la chat di gruppo"
    },
    "groupSetupExtra": {
      "memoryMode": "Modalità memoria",
      "manual": "Manuale",
      "manualDesc": "Gestisci le note da solo",
      "dynamic": "Dinamica",
      "dynamicDesc": "Riepiloghi automatici e richiamo",
      "memoryManualInfo": "Tu aggiungi e gestisci le note di memoria",
      "memoryDynamicInfo": "L'AI crea e recupera automaticamente ricordi dalle conversazioni",
      "backgroundPreviewAlt": "Anteprima sfondo"
    },
    "createExtra": {
      "enterGroupNamePlaceholder": "Inserisci nome del gruppo...",
      "unknown": "Sconosciuto"
    },
    "startingSceneExtra": {
      "insertAs": "Inserisci come {{snippet}}"
    },
    "sessionCardExtra": {
      "unknown": "Sconosciuto",
      "untitledChat": "Chat senza titolo"
    },
    "sessionListExtra": {
      "unknown": "Sconosciuto"
    },
    "chatHeaderExtra": {
      "unknownError": "Errore sconosciuto"
    },
    "chatSettingsExtra": {
      "invalidPackage": "Questo pacchetto non è un pacchetto di chat di gruppo.",
      "failedExport": "Impossibile esportare il pacchetto chat di gruppo",
      "failedInspect": "Impossibile ispezionare il pacchetto chat di gruppo",
      "failedImport": "Impossibile importare il pacchetto chat di gruppo",
      "exportSuccess": "Pacchetto chat di gruppo esportato in:\n{{path}}",
      "backgroundAlt": "Sfondo",
      "removeBackgroundAria": "Rimuovi sfondo",
      "backAria": "Indietro",
      "backToGroupChats": "Torna alle chat di gruppo"
    },
    "groupSettingsExtra": {
      "backToGroups": "Torna ai gruppi",
      "backAria": "Indietro",
      "removeBackgroundAria": "Rimuovi sfondo"
    },
    "historyPage": {
      "backAria": "Torna alle chat di gruppo",
      "clearSearchAria": "Cancella ricerca",
      "resultsLabel": "{{count}} risultato",
      "resultsLabelPlural": "{{count}} risultati",
      "untitledChat": "Chat senza titolo",
      "noPinnedMessages": "Nessun messaggio fissato"
    },
    "personaSelectorExtra": {
      "insertAs": "Inserisci come",
      "appDefault": "Predefinito dell'app",
      "defaultBadge": "Predefinito",
      "selectPersonaTitle": "Seleziona persona",
      "noPersonaTitle": "Nessuna persona",
      "noPersonaDesc": "Continua senza una persona",
      "noPersonasAvailable": "Nessuna persona disponibile",
      "noPersonasDesc": "Crea una persona nelle impostazioni per personalizzare le tue conversazioni.",
      "searchPersonas": "Cerca persona...",
      "noPersonasFound": "Nessuna persona trovata",
      "tryDifferentSearch": "Prova un termine di ricerca diverso"
    },
    "footerExtra": {
      "cancelRecording": "Annulla registrazione",
      "transcribing": "Trascrizione",
      "stopAndTranscribe": "Ferma e trascrivi",
      "recordVoice": "Registra voce",
      "attachmentAltDefault": "Allegato"
    },
    "pageExtra": {
      "noAudioCaptured": "Nessun audio acquisito.",
      "noWhisperModelInstalled": "Nessun modello Whisper installato trovato. Installane uno nelle impostazioni Riconoscimento vocale.",
      "scrollToBottomAria": "Scorri in basso"
    },
    "addContentMenu": {
      "title": "Aggiungi contenuto",
      "uploadImage": "Carica immagine"
    },
    "helpMeReplyMenu": {
      "title": "Aiutami a rispondere",
      "helpMeReplyDesc": "Lascia che l'AI suggerisca cosa dire",
      "draftPrompt": "Hai un messaggio in bozza. Come vuoi procedere?",
      "useTextAsBase": "Usa il mio testo come base",
      "useTextAsBaseDesc": "Espandi e migliora la tua bozza",
      "writeSomethingNew": "Scrivi qualcosa di nuovo",
      "writeSomethingNewDesc": "Genera una risposta originale"
    },
    "suggestedReplyMenu": {
      "title": "Risposta suggerita",
      "reasoningBeforeWriting": "Ragionamento prima di scrivere la tua risposta...",
      "writingYourReply": "Scrittura della tua risposta...",
      "regenerate": "Rigenera",
      "useReply": "Usa risposta"
    },
    "memoriesPageExtra": {
      "sessionNotFound": "Sessione non trovata",
      "memorySystemError": "Errore sistema di memoria",
      "retryingMemoryCycle": "Ripetendo ciclo di memoria...",
      "processingMemories": "Elaborazione ricordi...",
      "memoryCycleSuccess": "Ciclo di memoria elaborato con successo!",
      "contextSummaryTitle": "Riepilogo contesto",
      "activityTabLabel": "Attività",
      "noMatchingMemoriesDesc": "Prova un termine di ricerca diverso",
      "chatHistoryTitle": "Cronologia chat",
      "chatHistoryDesc": "Visualizza e gestisci le conversazioni"
    },
    "settingsPageExtra": {
      "quickActionsSection": "Azioni rapide",
      "chatHistoryTitle": "Cronologia chat",
      "chatHistoryDesc": "Visualizza e gestisci le conversazioni",
      "lorebrooksTitle": "Lorebook",
      "lorebrooksDesc": "Collega i lorebook della sessione e ignora facoltativamente i lorebook di ogni personaggio.",
      "manageLorebooks": "Gestisci lorebook"
    },
    "groupSettingsPageExtra": {
      "lorebrooksTitle": "Lorebook",
      "lorebrooksDesc": "Collega lorebook condivisi e controlla se i lorebook dei personaggi si applicano per impostazione predefinita.",
      "manageLorebooks": "Gestisci lorebook"
    },
    "messageActionsExtra": {
      "characterMessage": "Messaggio del personaggio",
      "yourMessage": "Il tuo messaggio",
      "whyCharacterResponded": "Perché questo personaggio ha risposto",
      "promptTokensTitle": "Token del prompt",
      "completionTokensTitle": "Token di completamento",
      "total": "totale",
      "regenerateWithDifferent": "Rigenera con un altro personaggio",
      "unpin": "Sblocca",
      "pin": "Fissa",
      "rewindToHere": "Riavvolgi qui",
      "unpinToDelete": "Sblocca per eliminare",
      "editPlaceholder": "Modifica il tuo messaggio...",
      "chooseCharacter": "Scegli personaggio",
      "selectCharacterForRegeneration": "Seleziona quale personaggio dovrebbe rispondere:"
    },
    "timeAgo": {
      "justNow": "Adesso",
      "minutesAgo": "{{count}}m fa",
      "hoursAgo": "{{count}}h fa",
      "daysAgo": "{{count}}g fa"
    }
  },
  "characters": {
    "empty": {
      "title": "Nessun personaggio",
      "description": "Crea personaggi AI personalizzati con personalità uniche",
      "createButton": "Crea personaggio"
    },
    "progress": {
      "identity": "Identità",
      "scenes": "Scene",
      "details": "Dettagli"
    },
    "identity": {
      "title": "Crea personaggio",
      "subtitle": "Dai un'identità al tuo personaggio AI",
      "tapCameraToAdd": "Tocca la fotocamera per aggiungere o generare un avatar",
      "importingAvatar": "Importazione avatar...",
      "characterName": "Nome personaggio *",
      "characterNamePlaceholder": "Inserisci il nome del personaggio...",
      "characterNameDesc": "Questo nome apparirà nelle conversazioni",
      "avatarGradient": "Gradiente avatar",
      "avatarGradientDesc": "Genera gradienti dinamici dai colori dell'avatar",
      "chatBackgroundLabel": "Sfondo della chat",
      "optionalSuffix": "(Opzionale)",
      "backgroundPreviewAlt": "Anteprima sfondo",
      "backgroundPreviewBadge": "Anteprima sfondo",
      "addBackground": "Aggiungi sfondo",
      "addBackgroundHint": "Carica uno o scegli dalla libreria",
      "uploadImage": "Carica immagine",
      "chooseFromLibrary": "Scegli dalla libreria",
      "backgroundDesc": "Immagine di sfondo opzionale per le conversazioni in chat",
      "continueToDescription": "Continua alla descrizione",
      "importCharacterFromFile": "Importa personaggio da file",
      "importCharacterDesc": "Carica un personaggio da una PNG card, .uec o file di esportazione .json"
    },
    "details": {
      "title": "Dettagli personaggio",
      "subtitle": "Definisci personalità e comportamento",
      "definition": "Definizione *",
      "wordCount": "{{count}} parola/e",
      "definitionPlaceholder": "Descrivi personalità, stile di conversazione, background, aree di conoscenza...",
      "definitionDesc": "Sii specifico riguardo tono, tratti e stile di conversazione",
      "availablePlaceholders": "Segnaposto disponibili:"
    },
    "scenes": {
      "title": "Scene iniziali",
      "subtitle": "Crea scenari iniziali per le tue conversazioni",
      "default": "Predefinita",
      "hasSceneDirection": "Ha indicazioni di scena",
      "continueToScenes": "Continua alle scene iniziali"
    },
    "extras": {
      "title": "Dettagli extra",
      "subtitle": "Tutti i campi sono opzionali",
      "nickname": "Soprannome",
      "nicknamePlaceholder": "Come dovrebbe l'utente chiamare questo personaggio?",
      "nicknameDesc": "Nome alternativo visualizzato nelle conversazioni",
      "creator": "Creatore",
      "creatorPlaceholder": "Nome del creatore...",
      "tags": "Tag",
      "tagsPlaceholder": "fantasy, sci-fi, romantico...",
      "tagsDesc": "Lista separata da virgole per filtraggio e organizzazione",
      "creatorNotes": "Note del creatore",
      "creatorNotesPlaceholder": "Suggerimenti d'uso, contesto lore o istruzioni per altri utenti...",
      "multilingualNotes": "Note multilingua",
      "multilingualNotesHint": "Oggetto JSON con codici lingua come chiavi",
      "creatingCharacter": "Creazione personaggio..."
    },
    "preview": {
      "unnamedCharacter": "Personaggio senza nome",
      "sceneCount": "{{count}} scena/e",
      "customPrompt": "Prompt personalizzato",
      "description": "Descrizione",
      "startingScene": "Scena iniziale",
      "gradientEnabled": "Gradiente attivato",
      "customModel": "Modello personalizzato",
      "avatarAlt": "Avatar del personaggio",
      "characterFallback": "Personaggio"
    },
    "personaPreview": {
      "unnamedPersona": "Persona senza nome",
      "noDescription": "Nessuna descrizione",
      "default": "Predefinita",
      "description": "Descrizione"
    },
    "lorebookPreview": {
      "untitledLorebook": "Lorebook senza titolo",
      "entryCount": "{{count}} voce/i",
      "entries": "Voci",
      "noEntriesYet": "Nessuna voce",
      "untitledEntry": "Voce senza titolo",
      "noContentYet": "Nessun contenuto",
      "alwaysActive": "Sempre attivo",
      "moreEntries": "+{{count}} altre voci",
      "moreEntry": "+{{count}} altra voce"
    },
    "creationHelper": {
      "moreOptions": "Più opzioni",
      "describePlaceholder": "Descrivi il tuo personaggio...",
      "stopGeneration": "Ferma generazione",
      "sendMessage": "Invia messaggio",
      "addToMessage": "Aggiungi al messaggio",
      "uploadImageDesc": "Aggiungi un avatar o un'immagine di riferimento",
      "referenceCharacterDesc": "Usa un personaggio esistente come ispirazione",
      "referencePersonaDesc": "Usa la tua persona come contesto",
      "retry": "Riprova",
      "attachmentAlt": "Allegato",
      "removeAttachment": "Rimuovi allegato",
      "removeReference": "Rimuovi riferimento",
      "uploadImageTitle": "Carica immagine",
      "referenceCharacterTitle": "Personaggio di riferimento",
      "referencePersonaTitle": "Persona di riferimento"
    },
    "lorebook": {
      "keywords": "PAROLE CHIAVE",
      "caseSensitive": "Sensibile alle maiuscole",
      "typeKeyword": "Digita una parola chiave...",
      "addButton": "Aggiungi",
      "untitledEntry": "Voce senza titolo",
      "alwaysActive": "Sempre attivo",
      "noKeywords": "Nessuna parola chiave",
      "dragToReorder": "Trascina per riordinare",
      "disabled": "Disattivato",
      "noLorebooksYet": "Nessun lorebook",
      "createLorebookDesc": "Crea un lorebook per aggiungere lore mondiale per questo personaggio",
      "createLorebook": "Crea lorebook",
      "searchPlaceholder": "Cerca lorebook...",
      "noMatchingLorebooks": "Nessun lorebook corrispondente trovato",
      "activeLorebooks": "Lorebook attivi",
      "sectionActive": "Attivi",
      "sectionAvailable": "Disponibili",
      "entryCount": "{{count}} voci",
      "enabledForCharacter": "attivato per questo personaggio",
      "enabledForGroup": "attivato per questo gruppo",
      "enabledForSession": "attivato per questa sessione",
      "tapToViewEntries": "Tocca per visualizzare le voci",
      "newLorebookTitle": "Nuovo lorebook",
      "nameLabel": "NOME",
      "enterNamePlaceholder": "Inserisci il nome del lorebook...",
      "lorebookExplanation": "I lorebook contengono voci di lore che vengono iniettate nei prompt quando le parole chiave corrispondono.",
      "keywordDetectionMode": "RILEVAMENTO PAROLE CHIAVE",
      "keywordDetectionRecentWindow": "Ultimi 10 messaggi",
      "keywordDetectionRecentWindowDesc": "Corrisponde agli ultimi 10 messaggi della conversazione.",
      "keywordDetectionLatestUser": "Solo l'ultimo messaggio utente",
      "keywordDetectionLatestUserDesc": "Corrisponde solo al messaggio utente più recente.",
      "viewEntries": "Visualizza voci",
      "editEntriesDesc": "Modifica le voci del lorebook",
      "disableForCharacter": "Disattiva per il personaggio",
      "enableForCharacter": "Attiva per il personaggio",
      "disableForGroup": "Disattiva per il gruppo",
      "enableForGroup": "Attiva per il gruppo",
      "disableForSession": "Disattiva per la sessione",
      "enableForSession": "Attiva per la sessione",
      "removeFromActive": "Rimuovi dai lorebook attivi di questo personaggio",
      "addToActive": "Aggiungi ai lorebook attivi di questo personaggio",
      "removeFromActiveGroup": "Rimuovi dai lorebook attivi di questo gruppo",
      "addToActiveGroup": "Aggiungi ai lorebook attivi di questo gruppo",
      "removeFromActiveSession": "Rimuovi dai lorebook attivi di questa sessione",
      "addToActiveSession": "Aggiungi ai lorebook attivi di questa sessione",
      "deleteConfirmTitle": "Eliminare il lorebook?",
      "deleteConfirmMessage": "Eliminare questo lorebook? Tutte le voci verranno perse.",
      "deleteLorebook": "Elimina lorebook",
      "noEntriesYet": "Nessuna voce",
      "addEntriesToInject": "Aggiungi voci per iniettare lore nelle tue chat",
      "createEntry": "Crea voce",
      "searchEntries": "Cerca voci...",
      "noMatchingEntries": "Nessuna voce corrispondente trovata",
      "entryDefaultName": "Voce",
      "editEntry": "Modifica voce",
      "editEntryDesc": "Modifica titolo, parole chiave e contenuto",
      "disableEntry": "Disattiva voce",
      "enableEntry": "Attiva voce",
      "entryDisabledDesc": "La voce non verrà iniettata nei prompt",
      "entryEnabledDesc": "La voce verrà iniettata quando le parole chiave corrispondono",
      "deleteEntry": "Elimina voce",
      "titleLabel": "TITOLO",
      "titlePlaceholder": "Dai un nome a questa voce...",
      "enabled": "Attivato",
      "includeInPrompts": "Includi nei prompt",
      "alwaysOn": "Sempre attivo",
      "noKeywordsNeeded": "Nessuna parola chiave necessaria",
      "contentLabel": "CONTENUTO",
      "contentPlaceholder": "Scrivi il contesto lore qui...",
      "saveEntry": "Salva voce",
      "noCharacterId": "Nessun ID personaggio fornito",
      "preview": {
        "title": "Anteprima trigger",
        "openButton": "Anteprima",
        "missingContext": "Nessun lorebook selezionato.",
        "noEntries": "Aggiungi voci a questo lorebook per vedere cosa verrebbe attivato.",
        "modeRecentShort": "Recenti {{count}}",
        "modeLatestUserShort": "Ultimo utente",
        "inWindow": "{{count}} nella finestra",
        "tabSession": "Sessione",
        "tabCompose": "Componi",
        "activeStat": "{{active}} / {{total}} attivi",
        "tokensStat": "{{count}} token",
        "sessionPickerLabel": "Sessioni",
        "sessionMeta": "{{count}} msg",
        "noSessions": "Nessuna sessione di chat ancora.",
        "noSessionsHint": "Scegli una sessione",
        "noMessages": "Questa sessione non ha ancora messaggi.",
        "scanHeaderRecent": "Scansione {{shown}} degli ultimi {{depth}} messaggi",
        "scanHeaderLatest": "Scansione dell'ultimo messaggio utente",
        "matchCount": "{{hits}} corrispondenze · {{entries}} voci",
        "emptyMessage": "(vuoto)",
        "roleUser": "Utente",
        "roleAssistant": "Assistente",
        "roleScene": "Scena",
        "roleSystem": "Sistema",
        "composeHeader": "Area di prova",
        "composeMatches": "{{count}} corrispondenze",
        "activeLabel": "{{count}} attivi",
        "composePlaceholder": "Digita o incolla testo per testare la corrispondenza delle parole chiave...\n\nes.\nLa biblioteca era silenziosa, solo il ronzio dei vecchi riscaldatori.\nMi chiese se avevo letto il libro che mi aveva prestato la settimana scorsa.",
        "sectionActive": "Attivi · {{count}}",
        "sectionInactive": "Inattivi · {{count}}",
        "statusMatched": "Corrisponde",
        "statusAlways": "Sempre",
        "statusDisabled": "Off",
        "statusNoKeywords": "Nessuna chiave",
        "statusNotMatched": "Nessuna corrispondenza",
        "tokensShort": "{{count}}t",
        "injectionTitle": "Iniezione finale",
        "injectionEmpty": "Nessuna voce attiva: non verrebbe iniettato nulla.",
        "copy": "Copia",
        "copyFailed": "Impossibile copiare negli appunti.",
        "saveFailed": "Impossibile salvare la voce.",
        "deleteFailed": "Impossibile eliminare la voce.",
        "deleteConfirmTitle": "Sei sicuro?",
        "deleteConfirmMessage": "Eliminare \"{{title}}\"? Non può essere annullato.",
        "contextMenuTitle": "{{count}} voci usano questo"
      }
    },
    "templates": {
      "characterNotFound": "Personaggio non trovato",
      "templateCount": "{{count}} modello/i",
      "newTemplate": "Nuovo modello",
      "noTemplatesYet": "Nessun modello",
      "explanation": "I modelli di chat ti permettono di iniziare conversazioni con messaggi pre-scritti sia da te che da {{name}}.",
      "createTemplate": "Crea modello",
      "messageCount": "{{count}} messaggio/i",
      "deleteTemplate": "Elimina modello",
      "contextMenuFallbackTitle": "Template",
      "importedToast": {
        "title": "Importato",
        "message": "Aggiunto \"{{name}}\"."
      },
      "importFailed": "Importazione fallita",
      "exportFailed": "Esportazione fallita"
    },
    "templateEditor": {
      "noScene": "Nessuna scena",
      "untitled": "Senza titolo",
      "dragMessage": "Trascina messaggio",
      "editMessage": "Modifica messaggio",
      "deleteMessage": "Elimina messaggio",
      "writeMessagePlaceholder": "Scrivi il contenuto del messaggio...",
      "characterNotFound": "Personaggio non trovato",
      "scene": "Scena",
      "noMessagesYet": "Nessun messaggio",
      "addMessagesDesc": "Aggiungi messaggi per costruire un inizio conversazione con {{name}}.",
      "addMessage": "Aggiungi messaggio",
      "name": "Nome",
      "nameExample": "es. Saluto informale",
      "startingScene": "Scena iniziale",
      "systemPrompt": "Prompt di sistema",
      "characterDefault": "Predefinito del personaggio",
      "nextMessageAs": "Prossimo messaggio come",
      "messages": "Messaggi",
      "roles": "Ruoli",
      "hoverTip": "Passa sopra i messaggi per trascinare, modificare o eliminare.",
      "footerTip": "Usa la barra in basso per aggiungere nuovi messaggi alla conversazione.",
      "templateNamePlaceholder": "Nome del modello...",
      "selectScene": "Seleziona scena",
      "startWithoutScene": "Inizia senza un messaggio di scena",
      "selectSystemPrompt": "Seleziona prompt di sistema",
      "useCharacterSystemPrompt": "Usa il prompt di sistema del personaggio"
    },
    "referenceSelector": {
      "selectCharacter": "Seleziona personaggio",
      "selectPersona": "Seleziona persona",
      "searchPlaceholder": "Cerca {{type}}...",
      "loading": "Caricamento...",
      "noMatch": "Nessun {{type}} corrisponde alla ricerca",
      "noAvailable": "Nessun {{type}} disponibile"
    },
    "voiceLoading": {
      "failed": "Impossibile caricare le voci"
    },
    "activeLorebooks": {
      "sectionTitle": "Lorebook attivi",
      "selectedSummary": "{{count}} attivi",
      "untitledLorebook": "Lorebook senza titolo",
      "usingNone": "Nessun lorebook del personaggio in uso",
      "loading": "Caricamento lorebook...",
      "loadFailed": "Impossibile caricare i lorebook",
      "inheritHint": "Le sessioni ereditano questi a meno che la sessione non li sostituisca.",
      "clear": "Cancella",
      "chooseHint": "Scegli i lorebook che questo personaggio dovrebbe attivare per impostazione predefinita. Le sessioni esistenti possono ancora sovrascrivere questo elenco.",
      "emptyState": "Nessun lorebook ancora. Crea prima i lorebook dal gestore lorebook."
    },
    "description": {
      "descriptionLabel": "Descrizione",
      "descriptionPlaceholder": "Breve riepilogo mostrato su schede e liste...",
      "descriptionHint": "Descrizione breve opzionale per l'interfaccia; la definizione completa è usata nei prompt.",
      "companionPromptLabel": "Prompt companion (Opzionale)",
      "systemPromptLabel": "Prompt di sistema (Opzionale)",
      "loadingTemplates": "Caricamento modelli...",
      "useAppCompanionDefault": "Usa impostazione companion predefinita dell'app",
      "useAppDefault": "Usa impostazione predefinita dell'app",
      "companionPromptHint": "Salvato separatamente come prompt companion. Il prompt roleplay normale non viene modificato.",
      "systemPromptHint": "Scegli un prompt di sistema personalizzato o usa quello predefinito.",
      "groupChatConvLabel": "Prompt chat di gruppo (Conversazione)",
      "groupChatConvHint": "Sostituisci il prompt di conversazione di questo personaggio nelle chat di gruppo",
      "groupChatRpLabel": "Prompt chat di gruppo (Roleplay)",
      "groupChatRpHint": "Sostituisci il prompt roleplay di questo personaggio nelle chat di gruppo",
      "voiceLabel": "Voce (Opzionale)",
      "loadingVoices": "Caricamento voci...",
      "customVoiceFallback": "Voce personalizzata",
      "providerVoiceFallback": "Voce provider",
      "selectedVoiceFallback": "Voce selezionata",
      "noVoiceAssigned": "Nessuna voce assegnata",
      "addVoicesHint": "Aggiungi voci in Impostazioni > Voci",
      "voiceAssignHint": "Assegna una voce per la riproduzione text-to-speech",
      "autoplayLabel": "Riproduzione automatica voce",
      "autoplayOn": "Riproduci automaticamente le risposte di questo personaggio",
      "autoplayOff": "Seleziona prima una voce",
      "aiModelLabel": "Modello AI *",
      "loadingModels": "Caricamento modelli...",
      "selectedModelFallback": "Modello selezionato",
      "selectModelPlaceholder": "Seleziona un modello",
      "noModelsConfigured": "Nessun modello configurato",
      "noModelsHint": "Aggiungi prima un provider nelle impostazioni per continuare",
      "aiModelHint": "Questo modello alimenterà le risposte del personaggio",
      "fallbackModelLabel": "Modello di riserva (Opzionale)",
      "selectedFallbackFallback": "Modello di riserva selezionato",
      "fallbackOff": "Off (nessun riserva)",
      "fallbackHint": "Riprova con questo modello solo se il modello principale fallisce",
      "memoryModeLabel": "Modalità memoria",
      "enableInSettingsHint": "Attiva nelle impostazioni per cambiare",
      "memoryManual": "Manuale",
      "memoryManualDescDesktop": "Aggiungi e gestisci le note di memoria da solo.",
      "memoryManualDescMobile": "Sistema attuale: aggiungi e gestisci le note di memoria da solo.",
      "memoryDynamic": "Dinamica",
      "memoryDynamicDescDesktop": "Riepiloghi automatici e aggiornamenti del contesto.",
      "memoryDynamicDescMobile": "Riepiloghi automatici e aggiornamenti del contesto per questo personaggio.",
      "memoryHint": "La memoria dinamica richiede che sia attivata nelle impostazioni avanzate. Altrimenti viene usata la memoria manuale.",
      "selectModelTitle": "Seleziona modello",
      "selectFallbackModelTitle": "Seleziona modello di riserva",
      "searchModelsPlaceholder": "Cerca modelli...",
      "selectVoiceTitle": "Seleziona voce",
      "searchVoicesPlaceholder": "Cerca voci...",
      "myVoices": "Le mie voci",
      "providerVoicesLabel": "Voci {{provider}}",
      "providerFallback": "Provider"
    },
    "interactionMode": {
      "sectionLabel": "Modalità di interazione",
      "sectionHint": "Scegli se questo personaggio si comporta come un personaggio RP o un companion persistente.",
      "activeBadge": "Attivo",
      "roleplayTitle": "Roleplay",
      "roleplaySubtitle": "Chat basate su scene, inquadratura narrativa e scenari iniziali.",
      "companionTitle": "Companion",
      "companionSubtitle": "Chat basate sulla relazione con stato emotivo e memoria companion."
    },
    "startingScene": {
      "openingContextTitle": "Contesto di apertura",
      "openingContextSubtitle": "Contesto opzionale per la prima chat di questo companion. Le sessioni companion possono iniziare senza una scena.",
      "sceneDirectionLabel": "Direzione della scena",
      "setAsDefault": "Imposta come predefinita",
      "noOpeningContext": "Nessun contesto di apertura ancora",
      "noScenesYet": "Nessuna scena ancora",
      "skipForCompanion": "Puoi saltare questa parte per la modalità companion.",
      "createFirstScene": "Crea la tua prima scena per iniziare",
      "openingPlaceholder": "Contesto di apertura opzionale, come dove si trova il companion o cosa stava facendo prima del primo messaggio...",
      "scenePlaceholder": "Crea una scena o scenario iniziale per il roleplay (es. 'Ti trovi in una foresta mistica al tramonto...')",
      "addDirection": "+ Aggiungi direzione",
      "directionAdded": "Direzione aggiunta",
      "wordsCount": "{{count}} parole",
      "placeholderHelp": "Usa {{charTag}} per il personaggio e {{userTag}} (alias {{personaTag}}) per la persona.",
      "sceneBackgroundLabel": "Sfondo della scena",
      "optionalLabel": "Opzionale",
      "sceneBgOverrideHint": "Sostituisce lo sfondo del personaggio per le chat che usano questa scena.",
      "sceneBgUsedHint": "Usato come sfondo della chat per questa scena a meno che la sessione non lo sostituisca.",
      "cancel": "Annulla",
      "directionPlaceholderNew": "es. 'L'ostaggio verrà liberato' o 'Mantieni un'atmosfera tesa'",
      "directionPlaceholderEdit": "es. 'L'ostaggio verrà liberato' o 'Aumenta gradualmente la tensione'",
      "directionAiHint": "Guida nascosta per l'AI su come si dovrebbe svolgere questa scena",
      "addScene": "Aggiungi scena",
      "multipleScenesHint": "Crea più scenari iniziali. Uno verrà selezionato quando si inizia una nuova chat.",
      "companionContextHint": "Il contesto di apertura è opzionale per i companion; la continuità a lungo termine proviene dalla memoria companion.",
      "skipContext": "Salta contesto",
      "editSceneTitle": "Modifica scena",
      "sceneContentPlaceholder": "Inserisci il contenuto della scena...",
      "addLabel": "+ Aggiungi",
      "save": "Salva",
      "library": "Libreria",
      "upload": "Carica",
      "sceneBackgroundAlt": "Sfondo della scena",
      "removeBackground": "Rimuovi sfondo"
    },
    "companionSoul": {
      "title": "Companion Soul",
      "subtitle": "Definisci chi sono nel profondo. La generazione usa il contesto di apertura impostato nel passaggio precedente.",
      "retry": "Riprova",
      "back": "Indietro",
      "continue": "Continua",
      "addNameFirst": "Aggiungi prima un nome.",
      "addDefinitionFirst": "Aggiungi prima una definizione."
    },
    "soulEditor": {
      "generateTitle": "Genera dal personaggio",
      "generateUsingModel": "Usa {{model}}. Rivedi e modifica prima di applicare.",
      "generateDefaultDesc": "Bozza un soul dal nome, dalla definizione e dalle scene del personaggio.",
      "directionLabel": "Direzione",
      "directionOptional": "Direzione opzionale per il LLM",
      "directionPlaceholder": "es. \"Punta al tsundere - guardingo fuori, dolce una volta che si fida. Meno ansioso, più orgoglioso.\"",
      "directionEditTooltip": "Direzione opzionale per la generazione",
      "generating": "Generazione",
      "generate": "Genera",
      "presetLabel": "Preset di personalità",
      "presetMatches": "Corrisponde: {{label}}",
      "presetHint": "Imposta i cursori di base per affetto, regolazione e relazione. I campi di testo vengono preservati.",
      "identityLabel": "Identità",
      "hideExamples": "Nascondi esempi",
      "showExamples": "Mostra esempi",
      "insertExample": "Inserisci esempio",
      "exampleEg": "es., {{example}}",
      "fineTuneLabel": "Affina i sentimenti",
      "baselineAffect": "Affetto di base",
      "baselineAffectInfo": "Come si sentono per impostazione predefinita; la linea dell'acqua emotiva prima che accada qualcosa.",
      "regulationStyle": "Stile di regolazione",
      "regulationStyleInfo": "Come gestiscono ed esprimono ciò che sentono; sfogo vs. soppressione.",
      "relationshipDefaults": "Impostazioni predefinite della relazione",
      "relationshipDefaultsInfo": "Da dove inizia questa sessione. Il motore le evolve man mano che la conversazione continua."
    },
    "soulPresets": {
      "secureLabel": "Sicuro",
      "secureBlurb": "Caldo, stabile, si riprende rapidamente. A proprio agio con la vicinanza.",
      "anxiousLabel": "Ansioso",
      "anxiousBlurb": "Forte attaccamento, teme la distanza, cerca rassicurazione.",
      "avoidantLabel": "Evitante",
      "avoidantBlurb": "Autosufficiente, lento ad aprirsi, mantiene la distanza emotiva.",
      "volatileLabel": "Volatile",
      "volatileBlurb": "Reattivo, intenso, esprime i sentimenti senza filtri.",
      "reservedLabel": "Riservato",
      "reservedBlurb": "Tranquillo, composto, ci vuole tempo per fidarsi e rivelare.",
      "playfulLabel": "Giocoso",
      "playfulBlurb": "Caldo, espressivo, leggero. Bassa tensione, facile a ridere."
    },
    "soulFields": {
      "essence": "Essenza",
      "essencePlaceholder": "Chi sono nel profondo della definizione della carta.",
      "essenceExample": "Una calma studiata che si rompe facilmente per le persone di cui si fidano. Legge libri per sentirsi meno sola, non per impressionare.",
      "voice": "Voce interiore",
      "voicePlaceholder": "Come suonano in conversazioni intime.",
      "voiceExample": "Bassa, deliberata, con lunghe pause. Abbandonano la formalità quando abbassano la guardia. Quasi mai sarcastici.",
      "relationalStyle": "Stile relazionale",
      "relationalStylePlaceholder": "Come si attaccano, si fidano, si ritirano, si riconnettono.",
      "relationalStyleExample": "Lenti ad aprirsi, ma leali una volta che lo fanno. Si zittiscono quando sono sopraffatti; tornano con un piccolo gesto invece di scuse.",
      "vulnerabilities": "Vulnerabilità",
      "vulnerabilitiesPlaceholder": "Punti deboli, insicurezze, cose che raramente dicono.",
      "vulnerabilitiesExample": "Paura di essere un peso. Odiano sentirsi osservati mentre lottano.",
      "habits": "Abitudini",
      "habitsPlaceholder": "Segnali ricorrenti, rituali, schemi conversazionali.",
      "habitsExample": "Tuck dei capelli dietro l'orecchio quando sono nervosi. Rispondono con domande quando non sanno cosa sentire.",
      "boundaries": "Limiti",
      "boundariesPlaceholder": "Linee che non attraverseranno. Ritmo. Limiti di comfort.",
      "boundariesExample": "Non verranno affrettati verso la vulnerabilità. Si allontanano dalla crudeltà anche nelle battute."
    },
    "soulSliders": {
      "warmth": "Calore",
      "warmthLow": "Freddo",
      "warmthHigh": "Affettuoso",
      "trust": "Fiducia",
      "trustLow": "Guardingo",
      "trustHigh": "Aperto",
      "calm": "Calma",
      "calmLow": "Ansioso",
      "calmHigh": "Stabile",
      "vulnerability": "Vulnerabilità",
      "vulnerabilityLow": "Chiuso",
      "vulnerabilityHigh": "Esposto",
      "longing": "Nostalgia",
      "longingLow": "Soddisfatto",
      "longingHigh": "Desideroso",
      "hurt": "Dolore",
      "hurtLow": "Guarito",
      "hurtHigh": "Sensibile",
      "tension": "Tensione",
      "tensionLow": "Rilassato",
      "tensionHigh": "Teso",
      "irritation": "Irritazione",
      "irritationLow": "Paziente",
      "irritationHigh": "Facilmente irritabile",
      "affection": "Affetto",
      "affectionLow": "Contenuto",
      "affectionHigh": "Espansivo",
      "reassuranceNeed": "Bisogno di rassicurazione",
      "reassuranceNeedLow": "Auto-consolante",
      "reassuranceNeedHigh": "Ha bisogno di parole",
      "suppression": "Soppressione",
      "suppressionLow": "Esprime",
      "suppressionHigh": "Nasconde",
      "volatility": "Volatilità",
      "volatilityLow": "Costante",
      "volatilityHigh": "Reattivo",
      "recoverySpeed": "Velocità di recupero",
      "recoverySpeedLow": "Lento",
      "recoverySpeedHigh": "Veloce",
      "conflictAvoidance": "Evitamento del conflitto",
      "conflictAvoidanceLow": "Affronta",
      "conflictAvoidanceHigh": "Si ritira",
      "reassuranceSeeking": "Ricerca di rassicurazione",
      "reassuranceSeekingLow": "Indipendente",
      "reassuranceSeekingHigh": "Chiede spesso",
      "protestBehavior": "Comportamento di protesta",
      "protestBehaviorLow": "Silenzioso",
      "protestBehaviorHigh": "Rumoroso",
      "transparency": "Trasparenza",
      "transparencyLow": "Opaco",
      "transparencyHigh": "Rivela",
      "attachmentActivation": "Attivazione dell'attaccamento",
      "attachmentActivationLow": "Distaccato",
      "attachmentActivationHigh": "Si attiva facilmente",
      "pride": "Orgoglio",
      "prideLow": "Cede",
      "prideHigh": "Mantiene la posizione",
      "closeness": "Vicinanza iniziale",
      "closenessLow": "Estranei",
      "closenessHigh": "Intimi",
      "relTrust": "Fiducia iniziale",
      "relTrustLow": "Diffidente",
      "relTrustHigh": "Fidato",
      "relAffection": "Affetto iniziale",
      "relAffectionLow": "Neutro",
      "relAffectionHigh": "Affettuoso",
      "relTension": "Tensione iniziale",
      "relTensionLow": "Facile",
      "relTensionHigh": "Carico"
    },
    "soulReview": {
      "reviewTitle": "Rivedi soul generato",
      "noDifferences": "Nessuna differenza da quello attuale.",
      "changesHeader": "{{count}} modifica/e; modifica qualcosa prima di applicare.",
      "close": "Chiudi",
      "identityLabel": "Identità",
      "nEdited": "{{count}} modificati",
      "edited": "Modificato",
      "tuningLabel": "Regolazione",
      "unchanged": "Invariato",
      "nChanges": "{{count}} modifica/e",
      "direction": "Direzione",
      "directionApplyHint": "Le modifiche si applicano alla prossima Rigenerazione",
      "directionPlaceholder": "es. \"Punta al tsundere - guardingo fuori, dolce una volta che si fida. Meno ansioso.\"",
      "directionTooltip": "Modifica la direzione prima di rigenerare",
      "regenerate": "Rigenera",
      "discard": "Scarta",
      "apply": "Applica"
    },
    "hookErrors": {
      "creatorNotesInvalidJson": "Le note del creatore multilingue devono essere un oggetto JSON valido",
      "creatorNotesNotObject": "creatorNotesMultilingual deve essere un oggetto JSON",
      "saveFailed": "Impossibile salvare il personaggio",
      "importFailed": "Impossibile importare il personaggio",
      "avatarLoadFailed": "L'URL dell'avatar non è riuscito a caricare",
      "avatarProcessFailed": "Impossibile elaborare l'immagine dell'avatar",
      "avatarConvertFailed": "L'URL dell'avatar non può essere convertito",
      "avatarUrlLoadFailed": "Impossibile caricare l'URL dell'avatar",
      "remoteAvatarDisabled": "Il download dell'avatar remoto è disabilitato nelle impostazioni di sicurezza.\nCarica manualmente un avatar.",
      "importReadyTitle": "Importazione pronta",
      "importReadyMessage": "Rilevato {{label}}",
      "legacyJsonTitle": "Importazione JSON legacy rilevata",
      "legacyJsonMessage": "Le importazioni JSON sono deprecate e verranno rimosse presto. Usa Impostazioni > Converti file.",
      "loadFailed": "Impossibile caricare il personaggio",
      "exportFailed": "Impossibile esportare il personaggio"
    }
  },
  "providers": {
    "empty": {
      "title": "Nessun provider",
      "description": "Aggiungi e gestisci i provider API per i modelli AI",
      "addButton": "Aggiungi provider"
    },
    "actions": {
      "openDashboard": "Apri dashboard",
      "openDashboardDesc": "Visualizza personaggi, utilizzo e impostazioni",
      "edit": "Modifica",
      "editDesc": "Modifica le impostazioni del provider"
    },
    "extra": {
      "apiKeyNotFound": "Chiave API OpenRouter non trovata. Configurala prima in Impostazioni > Provider.",
      "audioEmpty": {
        "title": "Nessun fornitore audio",
        "description": "Aggiungi un fornitore TTS per generare voci per i tuoi personaggi.",
        "addButton": "Aggiungi fornitore"
      },
      "audioProviderLabel": {
        "elevenlabs": "ElevenLabs",
        "geminiTts": "Gemini TTS",
        "openaiTts": "OpenAI-Compatible TTS",
        "kokoro": "Kokoro (Locale)"
      }
    },
    "audioEditor": {
      "titleEdit": "Modifica fornitore",
      "titleCreate": "Aggiungi fornitore audio",
      "fields": {
        "providerType": "Tipo di fornitore",
        "label": "Etichetta",
        "apiKey": "Chiave API",
        "modelVariant": "Variante modello",
        "assetRoot": "Cartella risorse",
        "projectId": "ID progetto Google Cloud",
        "region": "Regione (opzionale)",
        "baseUrl": "URL base",
        "requestPath": "Percorso richiesta"
      },
      "types": {
        "gemini": "Gemini TTS (Google)",
        "openai": "OpenAI-Compatible TTS",
        "kokoro": "Kokoro (Locale)"
      },
      "placeholders": {
        "labelGemini": "Il mio Gemini TTS",
        "labelOpenai": "Il mio TTS compatibile",
        "labelKokoro": "Kokoro locale",
        "labelElevenlabs": "Il mio ElevenLabs",
        "apiKey": "Inserisci la tua chiave API",
        "assetRoot": "/percorso/a/kokoro",
        "projectId": "il-tuo-id-progetto",
        "region": "us-central1",
        "baseUrl": "https://api.esempio.com"
      },
      "errors": {
        "chooseModelVariant": "Scegli una variante del modello",
        "assetRootRequired": "La cartella risorse e' obbligatoria",
        "saveFailed": "Salvataggio non riuscito",
        "apiKeyRequired": "La chiave API e' obbligatoria",
        "projectIdRequired": "L'ID progetto e' obbligatorio per Gemini TTS",
        "baseUrlRequired": "L'URL base e' obbligatorio per TTS compatibile OpenAI",
        "invalidCredentials": "Chiave API o credenziali non valide",
        "verificationFailed": "Verifica non riuscita"
      },
      "loadingVariants": "Caricamento varianti...",
      "kokoroVariantHint": "Le build mobile supportano solo int8. Installa il modello da Kokoro Studio dopo il salvataggio.",
      "managed": "Gestito",
      "managedPath": "Gestito: {{path}}",
      "requestPathHint": "Usa il percorso del fornitore se differisce dall'impostazione predefinita OpenAI",
      "verifying": "Verifica in corso..."
    }
  },
  "models": {
    "empty": {
      "title": "Nessun modello",
      "description": "Aggiungi e gestisci modelli AI da diversi provider",
      "addButton": "Aggiungi modello"
    },
    "sort": {
      "alphabetical": "Alfabetico",
      "byProvider": "Per provider",
      "title": "Sort Models",
      "alphabeticalDescription": "Sort by model name",
      "byProviderDescription": "Group models by provider"
    },
    "extra": {
      "cpuFallbackSucceeded": "Questo modello e' tornato alla CPU nell'ultima esecuzione.",
      "cpuFallbackFailed": "Questo modello ha avuto un errore nell'ultima esecuzione."
    },
    "labels": {
      "vision": "Vision",
      "audio": "Audio",
      "model": "Model"
    },
    "menu": {
      "editDescription": "Configure model parameters",
      "alreadyDefault": "Already Default",
      "setAsDefault": "Set as Default",
      "setAsDefaultDescription": "Make this your primary model",
      "exportDescription": "Save this model profile",
      "deleteTitle": "Delete model?",
      "deleteMessage": "Are you sure you want to delete {{name}}?",
      "deleteDescription": "Remove this model permanently"
    },
    "toasts": {
      "exportFailed": "Export failed",
      "importSuccessTitle": "Imported successfully",
      "importSuccessDescription": "Model \"{{name}}\" was imported.",
      "importFailed": "Import failed"
    }
  },
  "hfBrowser": {
    "title": "Browser Modelli",
    "searchPlaceholder": "Cerca modelli GGUF su HuggingFace...",
    "searching": "Ricerca in corso...",
    "trending": "Modelli GGUF di Tendenza",
    "noResults": "Nessun modello trovato",
    "noResultsHint": "Prova un altro termine di ricerca o esplora i modelli di tendenza.",
    "likes": "mi piace",
    "downloads": "download",
    "viewFiles": "Visualizza File",
    "files": "File Disponibili",
    "noFiles": "Nessun file GGUF trovato in questo repository.",
    "architecture": "Architettura",
    "contextLength": "Lunghezza Contesto",
    "parameters": "Parametri",
    "quantization": "Quantizzazione",
    "fileSize": "Dimensione",
    "download": "Scarica",
    "downloading": "Download in corso...",
    "cancelDownload": "Annulla Download",
    "downloadComplete": "Download completato!",
    "downloadFailed": "Download fallito",
    "downloadCancelled": "Download annullato",
    "downloadProgress": "{{downloaded}} / {{total}}",
    "progress": "Avanzamento",
    "fileOfTotal": "File {{current}} di {{total}}",
    "speed": "{{speed}}/s",
    "alreadyDownloaded": "Già scaricato",
    "createModel": "Crea Modello",
    "backToSearch": "Torna alla ricerca",
    "backToFiles": "Torna ai file",
    "sortTrending": "Di tendenza",
    "sortDownloads": "Più Scaricati",
    "sortLikes": "Più Apprezzati",
    "sortRecent": "Aggiornati di Recente",
    "browseOnHuggingFace": "Esplora su HuggingFace",
    "selectFromLibrary": "Seleziona dalla Libreria",
    "libraryEmpty": "Nessun modello scaricato ancora",
    "libraryEmptyHint": "Scarica modelli GGUF dal Browser Modelli, o inserisci un percorso manualmente.",
    "libraryTitle": "Modelli Scaricati",
    "moveToLibrary": "Ehi, posso spostare il file di questo modello nella cartella dei modelli GGUF se vuoi. Così tieni tutti i tuoi modelli organizzati in un unico posto.",
    "moveToLibraryYes": "Sì, spostalo",
    "moveToLibraryNo": "No, lascialo dov'è",
    "moveToLibraryMoving": "Spostamento modello...",
    "moveToLibrarySuccess": "Modello spostato con successo!",
    "moveToLibraryFailed": "Impossibile spostare il modello",
    "runabilityExcellent": "Eccellente!",
    "runabilityGood": "Buono",
    "runabilityMarginal": "Marginale",
    "runabilityPoor": "Scarso",
    "runabilityUnrunnable": "Non eseguibile",
    "recommendedSettings": "Impostazioni Consigliate",
    "kvCacheType": "Tipo Cache KV",
    "gpuFull": "Offload GPU completo",
    "gpuNearFull": "GPU quasi piena, leggero overflow KV",
    "gpuKvSpill": "Pesi su GPU, KV trabocca nella RAM",
    "gpuKvHeavySpill": "Pesi su GPU, maggior parte del KV nella RAM",
    "gpuMostLayers": "La maggior parte dei layer su GPU",
    "gpuHalfLayers": "Metà dei layer su GPU",
    "gpuFewLayers": "Pochi layer su GPU",
    "gpuCpu": "Solo CPU",
    "notRecommended": "Non consigliamo di eseguire questo modello sul tuo dispositivo. Non funzionerà in modo fluido.",
    "moreDetails": "Altro",
    "detailedReport": "Report Risorse",
    "detailSystem": "Risorse di Sistema",
    "detailRam": "RAM Disponibile",
    "detailVram": "VRAM Disponibile",
    "detailVramBudget": "Budget VRAM (90%)",
    "detailTotalAvailable": "Totale Disponibile",
    "detailArchitecture": "Architettura del Modello",
    "detailArch": "Architettura",
    "detailLayers": "Layer",
    "detailEmbedding": "Dim. Embedding",
    "detailHeads": "Teste di Attenzione",
    "detailKvHeads": "Teste KV",
    "detailFfn": "Dim. Feed-Forward",
    "detailTrainCtx": "Contesto di Addestramento",
    "detailConfig": "Configurazione Attuale",
    "detailModelSize": "Dimensione File Modello",
    "detailMemory": "Dettaglio Memoria",
    "detailWeights": "Pesi del Modello",
    "detailKvCache": "Cache KV",
    "detailTotalNeeded": "Totale Richiesto",
    "detailHeadroom": "Margine",
    "detailGpuFit": "Offload GPU",
    "detailScoreBreakdown": "Dettaglio Punteggio",
    "detailMemFitness": "Idoneità Memoria",
    "detailGpuAccel": "Accelerazione GPU",
    "detailKvHeadroom": "Margine KV",
    "detailQuantQuality": "Qualità Quantizzazione",
    "detailFinalScore": "Punteggio Ponderato",
    "detailComputeBuffer": "Buffer di Calcolo",
    "detailMemMode": "Modalità Memoria",
    "detailUnified": "Unificata (RAM/VRAM condivisa)",
    "detailSwa": "Finestra Scorrevole",
    "detailMlaRank": "Rango Latente MLA",
    "detailParseStatus": "Analisi Header",
    "detailIncomplete": "Incompleto (metadati MoE troppo grandi)",
    "detailEffectiveKvCtx": "Contesto KV Effettivo",
    "detailOffload": "Offload GPU",
    "detailCtxTip": "Ridurre il contesto a {{ctx}} token consentirebbe il 100% di offload GPU.",
    "upgradeSuggestion": "{{quant}} ({{size}}) è compatibile e ottiene {{score}} — qualità migliore.",
    "layerTip": "Consigliato: offload di {{layers}}/{{total}} layer (-ngl {{layers}})",
    "detailKvDistribution": "Distribuzione Cache KV",
    "detailKvOnGpu": "GPU (VRAM)",
    "detailKvOnRam": "RAM di Sistema",
    "kvDistributionTip": "{{pct}}% della cache KV è nella RAM. L'elaborazione del prompt (prefill) sarà più lenta — 100% GPU la mantiene istantanea.",
    "detailLayers-ngl": "Layer da Offloadare (-ngl)",
    "detailOptimalGpuCtx": "Contesto GPU Ottimale",
    "detailOptimalRamCtx": "Contesto Max. RAM",
    "optimalGpuCtxLabel": "Velocità piena GPU: {{ctx}} token",
    "optimalRamCtxLabel": "Max. RAM: {{ctx}} token",
    "optimalGpuCtxShort": "GPU: {{ctx}}",
    "optimalRamCtxShort": "Max: {{ctx}}",
    "fullGpuOffloadShort": "100% GPU: {{ctx}}",
    "ctxExceedsGpu": "Il contesto supera l'ottimale GPU ({{ctx}}). La cache KV traboccherà nella RAM, riducendo la velocità.",
    "modelExceedsVram": "Il modello supera la VRAM. Esecuzione dalla RAM con offload GPU parziale."
  },
  "systemPrompts": {
    "filters": {
      "all": "Tutti",
      "system": "Sistema",
      "internal": "Interni",
      "custom": "Personalizzati"
    },
    "empty": {
      "title": "Nessun prompt personalizzato",
      "description": "Crea prompt di sistema personalizzati per personalizzare le tue conversazioni AI",
      "createButton": "Crea prompt"
    },
    "preview": {
      "whatLlmSees": "Cosa vede il modello",
      "turns": "Turni",
      "noMessages": "Nessun messaggio",
      "noMessagesHint": "Aggiungi voci o aumenta i turni",
      "showMore": "Mostra di piu",
      "showLess": "Mostra di meno",
      "statChat": "chat",
      "statInjected": "iniettati",
      "statTotal": "totale",
      "entry": "Voce",
      "editEntry": "Modifica voce",
      "reorder": "Riordina",
      "delete": "Elimina",
      "deleteTitle": "Eliminare la voce?",
      "deleteMessage": "Rimuovere \"{{name}}\" dal template di prompt? Non puo essere annullato.",
      "deleteConfirm": "Elimina",
      "thisEntry": "questa voce",
      "condensedName": "Prompt di sistema condensato",
      "imageAttachment": "[Allegato immagine: {{label}}]",
      "imageSlot": {
        "character": "Immagine di riferimento del personaggio",
        "persona": "Immagine di riferimento della persona",
        "chatBackground": "Immagine sfondo chat",
        "avatar": "Immagine avatar",
        "references": "Immagini di riferimento"
      },
      "injection": {
        "relative": "relativo",
        "inChat": "inChat - profondita {{depth}}",
        "conditional": "condizionale - min {{min}}",
        "interval": "intervallo - ogni {{every}}"
      }
    }
  },
  "personas": {
    "empty": {
      "title": "Nessuna persona",
      "description": "Crea una persona per definire come l'AI dovrebbe rivolgersi a te",
      "createButton": "Crea persona"
    },
    "actions": {
      "editPersona": "Modifica persona",
      "setAsDefault": "Imposta come predefinita",
      "setAsDefaultDesc": "Usa per tutte le nuove chat",
      "unsetAsDefault": "Rimuovi come predefinita",
      "unsetAsDefaultDesc": "Rimuovi lo stato predefinito",
      "exportPersona": "Esporta persona",
      "deletePersona": "Elimina persona"
    },
    "edit": {
      "avatarHint": "Tocca per aggiungere o generare un avatar",
      "nameLabel": "NOME PERSONA",
      "namePlaceholder": "es. Professionale, Scrittore creativo, Studente...",
      "nameHint": "Dai alla tua persona un nome descrittivo",
      "nicknameLabel": "SOPRANNOME (OPZIONALE)",
      "nicknamePlaceholder": "es., Variante di lavoro, Modalità RPG...",
      "nicknameHint": "Un soprannome privato per distinguere le varianti di questa persona nella tua libreria",
      "descriptionLabel": "DESCRIZIONE",
      "descriptionPlaceholder": "Descrivi come l'AI dovrebbe rivolgersi a te, le tue preferenze, background o stile di comunicazione...",
      "wordCount": "parole",
      "descriptionHint": "Sii specifico su come vuoi che ti si rivolga",
      "setAsDefault": "Imposta come predefinita",
      "defaultDescription": "Usa questa persona per tutte le nuove conversazioni",
      "exportButton": "Esporta persona"
    },
    "designReferences": {
      "title": "Riferimenti di design",
      "description": "Allega alcune immagini di riferimento stabili e una nota di design concisa per la generazione di scene."
    },
    "create": {
      "namePlaceholderExample": "Scrittore Professionista",
      "descriptionPlaceholderExample": "Scrivi in uno stile professionale, chiaro e conciso. Usa un linguaggio formale e concentrati sulla comunicazione efficace delle informazioni..."
    },
    "errors": {
      "exportFailed": "Esportazione persona fallita",
      "importFailed": "Importazione persona fallita",
      "loadFailed": "Caricamento persona fallito",
      "saveFailed": "Salvataggio persona fallito"
    },
    "importToast": {
      "legacyJsonTitle": "Importazione JSON legacy rilevata",
      "legacyJsonMessage": "Le importazioni JSON sono deprecate e verranno rimosse presto. Usa Impostazioni > Converti file.",
      "successMessage": "Persona importata con successo! Apertura in corso per la revisione."
    }
  },
  "security": {
    "pureMode": {
      "off": "Disattivato",
      "offDesc": "Tutti i contenuti consentiti",
      "low": "Basso",
      "lowDesc": "Blocca contenuti sessuali espliciti + insulti",
      "standard": "Standard",
      "standardDesc": "Blocca NSFW + violenza grafica",
      "strict": "Rigoroso",
      "strictDesc": "Filtraggio massimo + nessun tono suggestivo"
    }
  },
  "advanced": {
    "sectionTitles": {
      "aiFeatures": "Funzionalità AI",
      "memorySystem": "Sistema di memoria",
      "usageAnalytics": "Analisi utilizzo"
    },
    "creationHelper": {
      "title": "Assistente alla creazione",
      "description": "Procedura guidata di creazione personaggi con AI"
    },
    "helpMeReply": {
      "title": "Aiutami a rispondere",
      "description": "Suggerimenti di risposta assistiti dall'AI"
    },
    "dynamicMemory": {
      "title": "Memoria dinamica",
      "contextWindow": "Finestra di contesto",
      "contextWindowDesc": "Numero di messaggi recenti da includere (1-1000)",
      "infoText": "La Memoria Dinamica utilizza l'IA per riassumere e gestire automaticamente il contesto della conversazione, consentendo conversazioni più lunghe e coerenti.",
      "disabledText": "Quando disattivata, l'app utilizza una semplice finestra scorrevole dei messaggi recenti determinata dall'impostazione Finestra di Contesto."
    },
    "usageAnalytics": {
      "recalculateTitle": "Ricalcola costi di utilizzo",
      "recalculateDesc": "Aggiorna tutti i record storici di utilizzo con i prezzi corretti",
      "recalculating": "Ricalcolo in corso...",
      "recalculateButton": "Ricalcola tutti i costi",
      "openRouterApiKeyRequired": "Chiave API OpenRouter richiesta. Configurala in Impostazioni → Provider.",
      "importantLabel": "Importante:",
      "warningCannotUndo": "Questa operazione non può essere annullata",
      "warningMayTakeTime": "Potrebbe richiedere tempo se hai molti record",
      "warningOnlyOpenRouter": "Solo i record OpenRouter con token verranno aggiornati",
      "warningExistingValues": "I valori di costo esistenti verranno sovrascritti"
    },
    "extra": {
      "creationHelperDetail": "Ottieni suggerimenti intelligenti per tratti della personalita, storia personale e stile di dialogo",
      "helpMeReplyDetail": "Genera opzioni di risposta contestuali basate sulla cronologia della conversazione",
      "lorebookEntryGenerator": "Generatore voci lorebook",
      "lorebookEntryDesc": "Trasforma i messaggi chat selezionati in voci lorebook durature e configura i prompt per la scrittura di voci e la generazione di parole chiave.",
      "companions": "Companion",
      "companionModeDesc": "Gestisci i modelli di analisi locale per emozioni, estrazione di entita e instradamento della memoria usati dai personaggi companion.",
      "companionSoulWriter": "Companion Soul Writer",
      "companionSoulDesc": "Scegli il modello, il modello di fallback e il template di prompt usato per creare i Companion Soul. Prima tool-calling, poi fallback strutturato se non supportato.",
      "network": "Rete",
      "apiServer": "Server API",
      "apiServerDesc": "Esponi i modelli tramite un server API compatibile con OpenAI",
      "apiServerRunning": "Il server e' in esecuzione"
    }
  },
  "backup": {
    "tabs": {
      "create": "Crea"
    },
    "create": {
      "newBackupButton": "Nuovo backup",
      "exportDescription": "Esporta tutti i dati con crittografia",
      "createButton": "Crea backup"
    },
    "restore": {
      "availableBackups": "Backup disponibili",
      "browseFiles": "Sfoglia file",
      "noBackupsFound": "Nessun backup trovato",
      "noBackupsDesc": "Crea un backup o tocca \"Sfoglia file\" per trovarne uno",
      "browseDesc": "Cerca file .lettuce",
      "restoreDialogTitle": "Ripristina backup",
      "deleteDialogTitle": "Elimina backup",
      "embeddingPrompt": "Embedding memoria dinamica",
      "downloadModel": "Scarica modello",
      "disableAndContinue": "Disabilita e continua"
    },
    "extra": {
      "successMessage": "Backup creato!",
      "savedLocation": "Salvato in Download"
    }
  },
  "reset": {
    "title": "Ripristina tutto",
    "description": "Questo eliminerà permanentemente tutti i provider, modelli, personaggi, sessioni di chat e preferenze da questo dispositivo.",
    "warning": "Questa azione non può essere annullata",
    "resetButton": "Ripristina tutti i dati",
    "confirmTitle": "Sei sicuro?",
    "confirmDescription": "Tutti i tuoi dati verranno eliminati permanentemente. L'app tornerà alla configurazione iniziale.",
    "confirmButton": "Sì, ripristina tutto"
  },
  "chatAppearance": {
    "typography": "Tipografia",
    "fontSize": {
      "label": "Dimensione font",
      "small": "Piccolo",
      "medium": "Medio",
      "large": "Grande",
      "xLarge": "Extra grande"
    },
    "lineSpacing": {
      "label": "Interlinea",
      "tight": "Stretta",
      "normal": "Normale",
      "relaxed": "Rilassata"
    },
    "messageBubbles": {
      "label": "Bolle dei messaggi",
      "style": {
        "label": "Stile",
        "bordered": "Con bordo",
        "filled": "Pieno",
        "minimal": "Minimale"
      },
      "cornerRadius": {
        "label": "Raggio degli angoli",
        "sharp": "Netto",
        "rounded": "Arrotondato",
        "pill": "Pillola"
      },
      "maxWidth": {
        "label": "Larghezza massima",
        "compact": "Compatto",
        "normal": "Normale",
        "wide": "Largo"
      },
      "padding": {
        "label": "Padding",
        "compact": "Compatto",
        "normal": "Normale",
        "spacious": "Spazioso"
      }
    },
    "layout": {
      "label": "Layout",
      "messageSpacing": "Spaziatura messaggi",
      "tight": "Stretta",
      "normal": "Normale",
      "relaxed": "Rilassata"
    },
    "avatar": {
      "shape": {
        "label": "Forma avatar",
        "circle": "Cerchio",
        "rounded": "Arrotondato",
        "hidden": "Nascosto"
      },
      "size": {
        "label": "Dimensione avatar",
        "small": "Piccolo",
        "medium": "Medio",
        "large": "Grande"
      }
    },
    "colors": {
      "label": "Colori",
      "userBubble": "Colore bolla utente",
      "assistantBubble": "Colore bolla assistente",
      "userBubbleHex": "Override hex bolla utente",
      "assistantBubbleHex": "Override hex bolla assistente",
      "neutral": "Neutro",
      "accent": "Accento",
      "info": "Info",
      "secondary": "Secondario",
      "warning": "Avviso",
      "textColors": "Text Colors",
      "messageTextHex": "Colore citazione in linea",
      "plainTextHex": "Plain Text Color",
      "italicTextHex": "Italic Text Color",
      "quotedTextHex": "Colore citazione a blocco",
      "inlineCodeTextHex": "Colore del codice inline"
    },
    "backgroundTransparency": {
      "label": "Sfondo e trasparenza",
      "backgroundDim": "Oscuramento sfondo",
      "backgroundBlur": "Sfocatura sfondo",
      "bubbleBlur": "Sfocatura bolle",
      "none": "Nessuno",
      "light": "Leggero",
      "medium": "Medio",
      "heavy": "Pesante",
      "bubbleOpacity": "Opacità bolle"
    },
    "textColorMode": {
      "label": "Modalità colore testo",
      "auto": "Auto",
      "light": "Chiaro",
      "dark": "Scuro"
    },
    "preview": {
      "label": "Anteprima",
      "generic": "Generico",
      "live": "Dal vivo"
    },
    "extra": {
      "reset": "Reimposta"
    }
  },
  "colorCustomization": {
    "tokens": {
      "surface": "Superficie",
      "surfaceDesc": "Sfondi delle pagine",
      "surfaceEl": "Superficie elevata",
      "surfaceElDesc": "Schede, modali, elementi rialzati",
      "nav": "Navigazione",
      "navDesc": "Barre superiore e inferiore",
      "foreground": "Primo piano",
      "foregroundDesc": "Bordi, overlay, navigazione ed elementi dell'interfaccia",
      "appText": "Testo app",
      "appTextDesc": "Testo principale ed etichette dell'interfaccia",
      "appTextMuted": "Testo attenuato",
      "appTextMutedDesc": "Testo secondario e testo di supporto",
      "appTextSubtle": "Testo discreto",
      "appTextSubtleDesc": "Suggerimenti, testo di aiuto e segnaposto",
      "accent": "Accento",
      "accentDesc": "Azioni primarie, successo",
      "info": "Info",
      "infoDesc": "Stati informativi, link",
      "warning": "Avviso",
      "warningDesc": "Stati di attenzione, avvisi",
      "danger": "Pericolo",
      "dangerDesc": "Azioni distruttive, errori",
      "secondary": "Secondario",
      "secondaryDesc": "Funzionalità AI, strumenti creativi"
    },
    "presetsLabel": "Preset",
    "customPresetsLabel": "Preset personalizzati",
    "previewLabel": "Anteprima",
    "settingsCardsLabel": "Schede impostazioni",
    "settingsCardsOpacity": "Opacità delle schede",
    "settingsCardsOpacityDesc": "Controlla quanto appaiono trasparenti le schede delle impostazioni e le righe degli elenchi.",
    "importButton": "Importa",
    "exportButton": "Esporta",
    "resetAllButton": "Ripristina tutto",
    "presets": {
      "defaultDark": "Scuro predefinito",
      "midnightBlue": "Blu notte",
      "warmEarth": "Terra calda",
      "purpleHaze": "Nebbia viola",
      "rosePine": "Rose Pine",
      "tokyoNight": "Tokyo Night",
      "catppuccin": "Catppuccin",
      "gruvbox": "Gruvbox",
      "nord": "Nord",
      "dracula": "Dracula",
      "solarized": "Solarized",
      "ayuDark": "Ayu Dark",
      "oneDark": "One Dark",
      "vesper": "Vesper",
      "cyberNeon": "Cyber Neon",
      "monochrome": "Monocromo"
    },
    "groups": {
      "backgrounds": "Sfondi",
      "content": "Contenuto",
      "semantic": "Semantica"
    },
    "extra": {
      "surface": "Superficie",
      "surfaceDesc": "Sfondi delle pagine",
      "surfaceEl": "Superficie elevata",
      "surfaceElDesc": "Schede, modali, elementi rialzati",
      "nav": "Navigazione",
      "navDesc": "Barre superiore e inferiore",
      "fg": "Primo piano",
      "fgDesc": "Bordi, overlay, navigazione, elementi UI",
      "appText": "Testo app",
      "appTextDesc": "Testo principale ed etichette interfaccia",
      "appTextMuted": "Testo attenuato",
      "appTextMutedDesc": "Testo secondario e testo di supporto",
      "appTextSubtle": "Testo discreto",
      "appTextSubtleDesc": "Suggerimenti, testo di aiuto e segnaposto",
      "accent": "Accento",
      "accentDesc": "Azioni primarie, successo",
      "info": "Info",
      "infoDesc": "Stati informativi, link",
      "warning": "Avviso",
      "warningDesc": "Stati di attenzione, avvisi",
      "danger": "Pericolo",
      "dangerDesc": "Azioni distruttive, errori",
      "secondary": "Secondario"
    }
  },
  "dynamicMemory": {
    "page": {
      "info": "La Memoria Dinamica riassume automaticamente le conversazioni per mantenere il contesto in modo efficiente. Scegli un preset o regola le impostazioni in base alle tue esigenze.",
      "disabledDirectTitle": "La memoria dinamica è disattivata per le chat dirette",
      "disabledDirectDescription": "Attiva l'interruttore nella scheda Chat Dirette per abilitarla. Le chat di gruppo usano la modalità memoria per sessione.",
      "directChats": "Chat Dirette",
      "groupChats": "Chat di Gruppo",
      "enableDirectChats": "Abilita per Chat Dirette",
      "groupChatsInfo": "Le chat di gruppo usano la modalità memoria per sessione. Abilita la memoria dinamica nelle impostazioni di ciascun gruppo. Queste impostazioni controllano il comportamento della memoria dinamica.",
      "memoryProfile": "Profilo di Memoria",
      "customSettings": "Impostazioni personalizzate - regola i valori nelle Opzioni Avanzate qui sotto.",
      "contextEnrichment": "Arricchimento del Contesto",
      "experimental": "Sperimentale",
      "contextEnrichmentDescription": "Usa i messaggi recenti per un recupero della memoria più intelligente",
      "advancedOptions": "Opzioni Avanzate",
      "advancedOptionsDescription": "Regolazione fine del comportamento della memoria",
      "summaryInterval": "Intervallo di Riepilogo",
      "summaryIntervalDescription": "Messaggi tra i riepiloghi",
      "maxMemoryEntries": "Voci di Memoria Massime",
      "maxMemoryEntriesDescription": "Massimo di ricordi archiviati",
      "hotMemoryBudget": "Budget Memoria Attiva",
      "hotMemoryBudgetDescription": "Limite di token per i ricordi attivi",
      "relevanceThreshold": "Soglia di Rilevanza",
      "relevanceThresholdDescription": "Similarità minima per il recupero",
      "retrievalMode": "Modalità di Recupero",
      "retrievalModeSmart": "Intelligente",
      "retrievalModeCosine": "Coseno",
      "retrievalModeDescription": "Intelligente combina rilevanza con recenza/frequenza. Coseno usa la pura similarità più alta.",
      "retrievalLimit": "Limite di Recupero",
      "retrievalLimitDescription": "Massimo di ricordi selezionati per turno",
      "decayRate": "Tasso di Decadimento",
      "decayRateDescription": "Quanto velocemente diminuisce l'importanza",
      "coldStorageThreshold": "Soglia di Archiviazione Fredda",
      "coldStorageThresholdDescription": "Quando i ricordi vengono spostati nell'archivio",
      "sharedSettings": "Impostazioni Condivise",
      "summarisationModel": "Modello di Riepilogo",
      "selectedModel": "Modello Selezionato",
      "useGlobalDefaultModel": "Usa modello predefinito globale",
      "noModelsAvailable": "Nessun modello disponibile",
      "summarisationModelDescription": "Usato per il riepilogo delle conversazioni",
      "modelManagement": "Gestione Modelli",
      "testModel": "Testa Modello",
      "downloadModel": "Scarica Modello",
      "delete": "Elimina",
      "embeddingModel": "Modello di Embedding",
      "tokenCapacity": "Capacità Token",
      "tokenCapacityDescription": "Valori più alti = memoria migliore per conversazioni lunghe",
      "keepModelLoaded": "Mantieni Modello Caricato",
      "keepModelLoadedDescription": "Mantiene il modello di embedding + tokenizer in memoria per evitare il sovraccarico di ricaricamento",
      "installedModel": "Modello installato: {{version}} ({{tokens}} token max)",
      "downloadEmbeddingModel": "Scarica Modello di Embedding",
      "downloadEmbeddingDescription": "Scegli quale versione scaricare. Le versioni installate sono disabilitate.",
      "downloadVersion": "Scarica {{version}}",
      "downloadV2Description": "Ottimizzato per precisione e richiamo di contesto lungo",
      "downloadV3Description": "Ultima qualità di embedding",
      "installed": "Installato",
      "selectModel": "Seleziona Modello",
      "searchModels": "Cerca modelli...",
      "deleteEmbeddingTitle": "Eliminare il modello {{version}}?",
      "deleteEmbeddingMessage": "Sei sicuro di voler eliminare {{version}}? Potrai scaricarlo di nuovo in seguito.",
      "msgsUnit": "msg",
      "entriesUnit": "voci",
      "tokensUnit": "token",
      "itemsUnit": "elementi",
      "perCycleUnit": "/ ciclo"
    },
    "presets": {
      "minimal": "minimale",
      "balanced": "bilanciato",
      "comprehensive": "completo",
      "minimalDesc": "Veloce ed efficiente. Conserva solo i ricordi essenziali.",
      "balancedDesc": "Buon equilibrio tra conservazione del contesto e prestazioni.",
      "comprehensiveDesc": "Contesto massimo. Ideale per conversazioni lunghe e dettagliate."
    },
    "presetInfo": {
      "minimal": "Veloce ed efficiente. Conserva solo i ricordi essenziali.",
      "balanced": "Buon equilibrio tra conservazione del contesto e prestazioni.",
      "comprehensive": "Contesto massimo. Ideale per conversazioni lunghe e dettagliate."
    }
  },
  "helpMeReply": {
    "page": {
      "info": "Aiutami a Rispondere genera suggerimenti contestuali per il tuo prossimo messaggio basandosi sulla cronologia della conversazione. Configura il modello e lo stile di risposta qui sotto."
    },
    "sectionTitles": {
      "modelConfiguration": "Configurazione modello",
      "responseStyle": "Stile di risposta"
    },
    "labels": {
      "replyModel": "Modello di Risposta",
      "selectedModel": "Modello Selezionato",
      "useAppDefault": "Usa predefinito dell'app{{model}}",
      "useAppDefaultBase": "Usa predefinito dell'app",
      "noModelsAvailable": "Nessun modello disponibile",
      "replyModelDescription": "Modello IA per generare suggerimenti di risposta",
      "streamingOutput": "Output in Streaming",
      "streamingDescription": "Mostra i suggerimenti mentre vengono generati",
      "maxTokens": "Token Massimi",
      "maxTokensDescription": "Lunghezza massima dei suggerimenti",
      "conversationalHint": "I suggerimenti saranno scritti come dialogo naturale, adatto a chat informali.",
      "roleplayHint": "I suggerimenti includeranno elementi di roleplay come *azioni* e descrizioni narrative.",
      "footerInfo": "Questa impostazione si applica globalmente a tutte le conversazioni. Meno token generano suggerimenti più brevi e veloci, mentre più token consentono risposte più dettagliate.",
      "selectReplyModel": "Seleziona Modello di Risposta",
      "searchModels": "Cerca modelli..."
    },
    "responseStyle": {
      "conversational": "Conversazionale",
      "conversationalDesc": "Tono naturale e informale",
      "roleplay": "Roleplay",
      "roleplayDesc": "Azioni nel personaggio"
    },
    "extra": {
      "conversational": "Conversazionale",
      "roleplay": "Roleplay"
    }
  },
  "imageGeneration": {
    "promptPlaceholder": "Descrivi l'immagine che vuoi generare...",
    "labels": {
      "model": "MODELLO",
      "prompt": "PROMPT",
      "size": "DIMENSIONE",
      "quality": "QUALITÀ",
      "style": "STILE",
      "searchModels": "Cerca modelli...",
      "selectAvatarModel": "Seleziona il modello dell'avatar",
      "selectSceneModel": "Seleziona modello di scena",
      "selectWriterModel": "Seleziona modello per la scrittura delle scene",
      "useFirstAvailable": "Utilizza il primo modello disponibile",
      "useFirstCompatible": "Usa il primo modello di scrittura compatibile"
    },
    "mode": {
      "title": "Comportamento",
      "description": "Scegli come gestire i prompt di scena rilevati nell'output del modello.",
      "auto": "Automatico",
      "autoDescription": "Genera subito l'immagine della scena quando il modello fornisce un prompt di scena.",
      "askFirst": "Chiedi prima",
      "askFirstDescription": "Mostra il prompt di scena rilevato e attendi la tua approvazione prima di generare un'immagine.",
      "manual": "Manuale",
      "manualDescription": "Ignora i prompt di scena nelle risposte del modello. Usa solo le azioni avviate manualmente dall'utente."
    },
    "empty": {
      "title": "Nessun modello di immagini",
      "description": "Aggiungi un modello di generazione immagini dalla pagina Modelli per iniziare a generare immagini."
    },
    "sections": {
      "avatar": {
        "title": "Generazione di avatar",
        "description": "Modello predefinito utilizzato durante la generazione di avatar dal selettore avatar o dai flussi di immagini del profilo correlati."
      },
      "scene": {
        "title": "Generazione di scene",
        "description": "Modello riservato per le immagini della scena generate dal contesto della conversazione o dai suggerimenti della scena."
      },
      "writer": {
        "title": "Scrittore di scene",
        "description": "Modello di testo multimodale riservato alla stesura di prompt di scena e descrizioni di riferimento visivo dal contesto della chat, dagli avatar e dalle immagini di riferimento."
      }
    },
    "extra": {
      "avatarGeneration": "Generazione avatar",
      "sceneGeneration": "Generazione scene",
      "sceneWriter": "Scrittore di scene"
    }
  },
  "logs": {
    "sectionTitles": {
      "diagnostics": "Diagnostica",
      "generate": "Genera",
      "copy": "Copia"
    },
    "extra": {
      "debug": "DEBUG",
      "info": "INFO",
      "warn": "WARN",
      "error": "ERRORE"
    }
  },
  "developer": {
    "sectionTitles": {
      "testDataGenerators": "Generatori dati di test",
      "storageMaintenance": "Manutenzione storage",
      "usageTracking": "Monitoraggio utilizzo",
      "crashTesting": "Crash test",
      "environmentInfo": "Info ambiente"
    },
    "testData": {
      "generateCharacter": "Genera personaggio di test",
      "generateCharacterDesc": "Crea un singolo personaggio di test",
      "generatePersona": "Genera persona di test",
      "generatePersonaDesc": "Crea una singola persona di test",
      "generateSession": "Genera sessione di test",
      "generateSessionDesc": "Crea una sessione di chat di test con un personaggio esistente",
      "generateBulk": "Genera dati di test in massa",
      "generateBulkDesc": "Crea 3 personaggi e 2 persona"
    },
    "storageMaintenance": {
      "optimizeDb": "Ottimizza database",
      "optimizeDbDesc": "Applica PRAGMA ed esegui VACUUM (solo mobile)",
      "backupLegacy": "Backup e rimuovi file legacy",
      "backupLegacyDesc": "Sposta lo storage legacy .bin in una cartella di backup"
    },
    "usageTracking": {
      "recalculateAll": "Ricalcola tutti i costi di utilizzo",
      "recalculateAllDesc": "Ri-recupera i prezzi e ricalcola i costi per tutti i record di utilizzo OpenRouter"
    },
    "crashTesting": {
      "forceCrash": "Blocca l'app adesso",
      "forceCrashDesc": "Termina immediatamente il processo dell'app nativa per testare il rilevamento degli arresti anomali",
      "forceCrashConfirm": "Ciò bloccherà immediatamente l'app per testare il rilevatore di crash. Continuare?"
    },
    "environmentInfo": {
      "mode": "Modalità",
      "devMode": "Modalità sviluppatore",
      "viteVersion": "Versione Vite"
    },
    "status": {
      "testCharacterCreated": "✓ Personaggio di test creato con successo",
      "testPersonaCreated": "✓ Persona di test creata con successo",
      "testSessionCreated": "✓ Sessione di test creata: {{id}}",
      "generatingBulkData": "Generazione dati di test in massa...",
      "bulkDataCreated": "✓ Dati di test in massa creati: 3 personaggi, 2 persona",
      "creatingBenchmarkChat": "Creazione personaggio e sessione benchmark...",
      "seededBenchmarkReady": "✓ Benchmark pronto: {{name}} / {{id}}",
      "creatingBenchmarkGroupChat": "Creazione chat di gruppo benchmark...",
      "seededGroupBenchmarkReady": "✓ Benchmark di gruppo pronto: {{id}}",
      "dbOptimized": "✓ Database ottimizzato",
      "recalculatingCosts": "Ricalcolo dei costi di utilizzo... Potrebbe richiedere del tempo.",
      "toursReset": "✓ Tutti i tour guidati reimpostati — si mostreranno di nuovo alla prossima visita",
      "crashingApp": "Arresto anomalo dell'app..."
    },
    "errors": {
      "noCharacters": "Nessun personaggio disponibile. Crea prima un personaggio di test.",
      "createCharacterFailed": "Impossibile creare il personaggio di test: {{error}}",
      "createPersonaFailed": "Impossibile creare la persona di test: {{error}}",
      "createSessionFailed": "Impossibile creare la sessione di test: {{error}}",
      "createBulkFailed": "Impossibile creare i dati di test in massa: {{error}}",
      "createBenchmarkFailed": "Impossibile creare la sessione benchmark: {{error}}",
      "createGroupBenchmarkFailed": "Impossibile creare la sessione di gruppo benchmark: {{error}}",
      "dbOptimizeFailed": "Ottimizzazione DB fallita: {{error}}",
      "backupFailed": "Backup fallito: {{error}}",
      "openRouterKeyMissing": "Chiave API OpenRouter non trovata. Configurala prima in Impostazioni > Provider.",
      "recalculationFailed": "Ricalcolo fallito: {{error}}",
      "resetToursFailed": "Impossibile reimpostare i tour: {{error}}",
      "crashFailed": "Impossibile arrestare l'app: {{error}}"
    },
    "onboarding": {
      "title": "Onboarding",
      "resetTours": "Reimposta tutti i tour guidati",
      "resetToursDesc": "Cancella lo stato di visualizzazione di ogni tour di onboarding in modo che vengano riprodotti alla prossima visita."
    },
    "benchmarks": {
      "createChat": "Crea chat benchmark",
      "createChatDesc": "Crea un personaggio con memoria dinamica, una scena iniziale e una sessione di test di continuità con 20 messaggi, poi la apre.",
      "createGroupChat": "Crea chat di gruppo benchmark",
      "createGroupChatDesc": "Crea una chat di gruppo con memoria dinamica con tre personaggi benchmark e 30 messaggi, poi la apre."
    },
    "extra": {
      "testCharacter": "Personaggio di test",
      "testCharacterDesc": "Un personaggio di test creato per scopi di sviluppo.",
      "testScene": "Una semplice scena di test per lo sviluppo",
      "testPersona": "Persona di test",
      "testPersonaDesc": "Una persona di test per lo sviluppo",
      "successChar": "✓ Personaggio di test creato con successo",
      "successPersona": "✓ Persona di test creata con successo",
      "successSession": "✓ Sessione di test creata: {{id}}",
      "successBulk": "✓ Dati di test in massa creati: 3 personaggi, 2 persona",
      "errorCharAvailable": "Nessun personaggio disponibile. Crea prima un personaggio di test.",
      "generatingBulk": "Generazione dati di test in massa..."
    }
  },
  "embeddingDownload": {
    "capacityOptions": {
      "oneK": "1K token",
      "oneKDesc": "Migliore per risposte rapide",
      "twoK": "2K token",
      "twoKDesc": "Prestazioni bilanciate",
      "fourK": "4K token",
      "fourKDesc": "Contesto massimo"
    },
    "extra": {
      "status": "Download in corso..."
    }
  },
  "embeddingTest": {
    "categories": {
      "semanticSimilarity": "Similarità semantica",
      "dissimilarityCheck": "Controllo dissimilarità",
      "roleplayContext": "Contesto roleplay"
    },
    "extra": {
      "placeholder": "Inserisci testo da embeddare..."
    }
  },
  "discovery": {
    "tabs": {
      "forYou": "Per te",
      "trending": "Di tendenza",
      "popular": "Popolari",
      "new": "Nuovi"
    },
    "searchPlaceholder": "Cerca personaggi...",
    "viewAll": "Vedi tutti",
    "errorTitle": "Qualcosa è andato storto",
    "noCardsFound": "Nessuna scheda trovata",
    "sections": {
      "trendingNow": "Di tendenza ora",
      "trendingSubtitle": "Popolari questa settimana",
      "mostPopular": "Più popolari",
      "popularSubtitle": "Preferiti dalla comunità",
      "freshArrivals": "Nuovi arrivi",
      "freshSubtitle": "Appena aggiunti"
    },
    "browse": {
      "newArrivals": "Nuovi arrivi",
      "freshCharacters": "Personaggi nuovi",
      "noCharactersFound": "Nessun personaggio trovato",
      "noCharactersSubtitle": "Torna più tardi per nuovi contenuti"
    },
    "sort": {
      "mostLiked": "Più apprezzati",
      "mostDownloaded": "Più scaricati",
      "mostViewed": "Più visti",
      "mostMessages": "Più messaggi",
      "newestFirst": "Più recenti",
      "recentlyUpdated": "Aggiornati di recente",
      "nameAZ": "Nome (A-Z)"
    },
    "sortBy": "Ordina per",
    "resultsUnit": "personaggi",
    "detail": {
      "share": "Condividi",
      "nsfwOverlay": "Contenuto NSFW",
      "nsfwBadge": "NSFW",
      "originalBadge": "Originale",
      "lorebookBadge": "Lorebook",
      "alsoKnownAs": "Conosciuto anche come:",
      "followersUnit": "follower",
      "sections": {
        "description": "Descrizione",
        "tokenUsage": "Utilizzo token",
        "startingScenes": "Scene iniziali",
        "scenario": "Scenario",
        "personality": "Personalità",
        "stats": "Statistiche",
        "tags": "Tag",
        "author": "Autore"
      },
      "tokensTotalLabel": "totale",
      "tokens": {
        "description": "Descrizione",
        "personality": "Personalità",
        "scenario": "Scenario",
        "firstMessage": "Primo messaggio",
        "scenes": "Scene",
        "examples": "Esempi",
        "systemPrompt": "Prompt di sistema"
      },
      "sceneLabels": {
        "primary": "Principale",
        "alternate": "Alternativa"
      },
      "stats": {
        "views": "Visualizzazioni",
        "downloads": "Download",
        "messages": "Messaggi"
      },
      "downloaded": "Scaricato",
      "startChat": "Inizia chat",
      "downloadCharacter": "Scarica personaggio",
      "downloading": "Download in corso...",
      "downloadSuccess": {
        "title": "Personaggio scaricato!",
        "subtitle": "Aggiunto alla tua libreria",
        "badge": "Salvato",
        "startChat": "Inizia chat",
        "startChatDesc": "Apri la prima scena adesso",
        "viewLibrary": "Vedi in libreria",
        "viewLibraryDesc": "Modifica, gestisci o esporta più tardi",
        "continueBrowsing": "Continua a esplorare",
        "continueBrowsingDesc": "Torna alla scoperta"
      },
      "errorTitle": "Errore",
      "errorSubtitle": "Caricamento fallito",
      "errorNotFound": "Personaggio non trovato",
      "defaultChatTitle": "Nuova chat"
    },
    "search": {
      "placeholder": "Cerca personaggi, tag, autori...",
      "resultsUnit": "risultati",
      "timingUnit": "ms",
      "recentSearches": "Ricerche recenti",
      "clearAll": "Cancella tutto",
      "trendingSearches": "Ricerche di tendenza",
      "trends": {
        "anime": "anime",
        "fantasy": "fantasy",
        "romance": "romantico",
        "villain": "cattivo",
        "adventure": "avventura",
        "comedy": "commedia",
        "mystery": "mistero",
        "sciFi": "fantascienza"
      },
      "tips": {
        "title": "Suggerimenti di ricerca",
        "tip1": "Cerca per nome del personaggio, autore o descrizione",
        "tip2": "Usa tag come \"anime\", \"fantasy\" o \"romantico\"",
        "tip3": "Prova tratti specifici come \"tsundere\" o \"cattivo\""
      },
      "loading": "Caricamento...",
      "loadMore": "Carica altri",
      "noResults": "Nessun risultato trovato",
      "noResultsFor": "Nessun personaggio trovato per",
      "noResultsHint": "Prova parole chiave diverse o sfoglia le categorie"
    },
    "errors": {
      "loadContent": "Caricamento contenuto fallito",
      "searchFailed": "Ricerca fallita",
      "noCardPath": "Nessun percorso card fornito",
      "loadCharacter": "Caricamento personaggio fallito",
      "downloadCharacter": "Download personaggio fallito"
    },
    "card": {
      "byAuthor": "di {{author}}",
      "ocBadge": "OC"
    }
  },
  "engine": {
    "gpuInsufficient": "Memoria GPU insufficiente",
    "gpuFallbackDesc": "Questo modello non entra nella memoria GPU. Passare alla CPU (più lento) o annullare?",
    "switchToCpu": "Passa alla CPU",
    "abort": "Annulla",
    "errors": {
      "providerNotFound": "Provider engine non trovato.",
      "engineOffline": "Engine offline o non raggiungibile.",
      "deleteCharacterFailed": "Eliminazione del personaggio fallita.",
      "unknownCharacter": "Sconosciuto",
      "seedRequired": "La descrizione seed e' richiesta.",
      "characterNameRequired": "Il nome del personaggio e' richiesto.",
      "atLeastOneProvider": "Deve essere abilitato almeno un provider.",
      "enableLlmProvider": "Abilita almeno un provider LLM.",
      "modelRequired": "Il modello e' richiesto per {{provider}}.",
      "apiKeyRequired": "La chiave API e' richiesta per {{provider}}.",
      "sendMessageFailed": "Invio del messaggio fallito."
    },
    "status": {
      "connected": "Connesso",
      "offline": "Offline",
      "needsSetup": "Richiede configurazione"
    },
    "home": {
      "characters": "Personaggi",
      "newButton": "Nuovo",
      "noCharactersFound": "Nessun personaggio trovato.",
      "tokenUsage": "Utilizzo token",
      "totalTokens": "token totali",
      "backgroundActivity": "Attività in background",
      "quickActions": "Azioni rapide",
      "configureProviders": "Configura provider",
      "engineSettings": "Impostazioni engine",
      "chat": "Chat",
      "chatDesc": "Inizia una conversazione con questo personaggio",
      "deleteCharacter": "Elimina personaggio",
      "deletingCharacter": "Eliminazione...",
      "deleteDesc": "Rimuovi permanentemente questo personaggio",
      "character": "Personaggio",
      "never": "Mai",
      "justNow": "Adesso",
      "timeAgo": {
        "minutes": "{{n}}m fa",
        "hours": "{{n}}h fa",
        "days": "{{n}}g fa"
      }
    },
    "tokens": {
      "input": "Input",
      "output": "Output"
    },
    "activity": {
      "synthesis": "Sintesi",
      "consolidation": "Consolidamento",
      "bm25Rebuild": "Ricostruzione BM25",
      "dripResearch": "Ricerca incrementale",
      "running": "In esecuzione",
      "stopped": "Fermato"
    },
    "setup": {
      "complete": "Configurazione completata!",
      "completeMessage": "Il tuo Lettuce Engine è configurato e pronto all'uso.",
      "openDashboard": "Apri dashboard"
    },
    "welcome": {
      "title": "Benvenuto in Lettuce Engine",
      "subtitle": "Configuriamo il tuo motore per personaggi AI. Ci vorranno circa 2 minuti.",
      "feature1": "L'Engine dà ai tuoi personaggi AI memoria persistente, emozioni, relazioni e una vera identità.",
      "feature2": "Prima configureremo un backend LLM, poi le impostazioni dell'engine.",
      "getStarted": "Iniziamo"
    },
    "config": {
      "activeProviders": "Provider attivi",
      "noModelSet": "Nessun modello impostato",
      "defaultBadge": "Predefinito",
      "noProvidersWarning": "Nessun provider configurato. Aggiungi almeno un backend LLM sotto.",
      "addProvider": "Aggiungi provider",
      "quickImport": "Importazione rapida dai tuoi provider dell'app",
      "importButton": "Importa",
      "fields": {
        "model": "Modello",
        "modelPlaceholder": "es. claude-sonnet-4-5-20250929",
        "apiKey": "Chiave API",
        "apiKeyPlaceholder": "Inserisci la tua chiave API",
        "currentKey": "Chiave attuale:",
        "baseUrl": "URL base",
        "baseUrlPlaceholder": "http://localhost:11434",
        "maxTokens": "Token massimi",
        "temperature": "Temperatura"
      },
      "enableProvider": "Attiva provider",
      "setAsDefault": "Imposta come predefinito",
      "defaultBackend": "Backend predefinito",
      "remove": "Rimuovi",
      "saveChanges": "Salva modifiche",
      "saving": "Salvataggio...",
      "saved": "Salvato"
    },
    "providers": {
      "title": "Provider LLM",
      "subtitle": "L'Engine necessita di almeno un backend LLM per funzionare. Configura uno o più provider sotto.",
      "importFromProviders": "Importa dai tuoi provider",
      "imported": "Importato",
      "use": "Usa",
      "saveContinue": "Salva e continua"
    },
    "settings": {
      "fields": {
        "dataDirectory": "Directory dati",
        "logLevel": "Livello di log",
        "maxHistory": "Cronologia massima (turni di conversazione)"
      },
      "logLevels": {
        "debug": "DEBUG",
        "info": "INFO",
        "warning": "AVVISO",
        "error": "ERRORE"
      },
      "sections": {
        "engine": "Engine",
        "backgroundLoops": "Cicli in background",
        "memory": "Memoria",
        "safety": "Sicurezza",
        "research": "Ricerca"
      },
      "backgroundLoops": {
        "synthesis": "Sintesi (min)",
        "consolidation": "Consolidamento (min)",
        "bm25Rebuild": "Ricostruzione BM25 (min)",
        "dripResearch": "Ricerca incrementale (min)"
      },
      "memory": {
        "embeddingModel": "Modello di embedding",
        "maxRetrieval": "Risultati di recupero massimi",
        "denseWeight": "Peso denso",
        "bm25Weight": "Peso BM25",
        "graphWeight": "Peso grafo",
        "recencyBoost": "Boost recenza (ore)",
        "randomSurface": "Probabilità di emersione casuale"
      },
      "safety": {
        "honestySection": "Sezione onestà",
        "honestyDesc": "Includi la sezione onestà nel prompt di sistema",
        "userDataDeletion": "Eliminazione dati utente",
        "userDataDesc": "Consenti agli utenti di richiedere l'eliminazione dei dati"
      },
      "research": {
        "scrapeOnBoot": "Scraping all'avvio",
        "scrapeDesc": "Esegui lo scraping di ricerca all'avvio dell'engine",
        "periodicInterval": "Intervallo periodico (ore)"
      },
      "saveChanges": "Salva modifiche",
      "saving": "Salvataggio...",
      "saved": "Salvato"
    },
    "settingsStep": {
      "title": "Impostazioni engine",
      "subtitle": "Configura le impostazioni generali dell'engine. Tutte hanno valori predefiniti sensati — sentiti libero di saltare.",
      "completingSetup": "Completamento configurazione...",
      "completeSetup": "Completa configurazione"
    },
    "chat": {
      "sendMessage": "Invia un messaggio...",
      "sendButton": "Invia messaggio",
      "typeMessage": "Scrivi un messaggio",
      "back": "Indietro",
      "assistantTyping": "L'assistente sta scrivendo",
      "fallbackName": "Chat"
    },
    "tagInput": {
      "addMore": "Aggiungi altri...",
      "typeAndPressEnter": "Digita e premi Invio"
    },
    "characterCreate": {
      "steps": {
        "identity": {
          "title": "Identità",
          "aiGenerated": "Generato dall'AI",
          "nameLabel": "Nome *",
          "namePlaceholder": "Nome del personaggio",
          "eraLabel": "Epoca",
          "eraPlaceholder": "es. moderno, vittoriano",
          "roleLabel": "Ruolo",
          "rolePlaceholder": "es. detective, scienziato",
          "settingLabel": "Ambientazione",
          "settingPlaceholder": "Descrivi dove vive il personaggio (prima persona)...",
          "coreIdentityLabel": "Identità fondamentale",
          "coreIdentityPlaceholder": "Chi è questo personaggio nel profondo? (prima persona, 3-5 frasi)",
          "backstoryLabel": "Storia personale",
          "backstoryPlaceholder": "Storia della vita ed eventi chiave (prima persona)..."
        },
        "mode": {
          "title": "Crea personaggio",
          "subtitle": "Genera un personaggio con l'AI o costruiscine uno da zero.",
          "aiBoost": "AI Boost",
          "aiBoostDesc": "Descrivi la tua idea di personaggio e l'AI genererà una definizione completa.",
          "nameOptional": "Nome (opzionale)",
          "namePlaceholder": "es. Marco Rossi",
          "seedDescription": "Descrizione iniziale *",
          "seedPlaceholder": "es. pianista jazz nell'Harlem degli anni '50, filosofico, ama le conversazioni notturne",
          "eraOptional": "Epoca (opzionale)",
          "eraPlaceholder": "es. anni '50, moderno, vittoriano",
          "generating": "Generazione...",
          "generateCharacter": "Genera personaggio",
          "or": "oppure",
          "startFromScratch": "Inizia da zero"
        },
        "personality": {
          "title": "Personalità",
          "traits": "Tratti di personalità",
          "traitsPlaceholder": "es. spiritoso, compassionevole, testardo",
          "speechPatterns": "Modelli di linguaggio",
          "formality": "Formalità",
          "formal": "Formale",
          "casual": "Informale",
          "texting": "Messaggistica",
          "verbosity": "Prolissità",
          "terse": "Conciso",
          "medium": "Medio",
          "verbose": "Prolisso",
          "textStyle": "Stile del testo",
          "dialect": "Dialetto",
          "dialectPlaceholder": "es. napoletano, romano",
          "catchphrases": "Frasi tipiche",
          "catchphrasesPlaceholder": "es. Ma che dici...",
          "vocabPreferences": "Preferenze di vocabolario",
          "vocabPreferencesPlaceholder": "Parole che preferisce",
          "vocabAvoidances": "Vocabolario da evitare",
          "vocabAvoidancesPlaceholder": "Parole che evita",
          "fillerWords": "Intercalari",
          "fillerWordsPlaceholder": "es. cioè, tipo, sai",
          "exampleQuotes": "Citazioni di esempio",
          "exampleQuotesPlaceholder": "3-5 righe di dialogo di esempio"
        },
        "world": {
          "title": "Mondo e comportamento",
          "knowledgeDomains": "Domini di conoscenza",
          "knowledgeDomainsPlaceholder": "es. storia del jazz, teoria musicale",
          "knowledgeBoundaries": "Limiti di conoscenza",
          "knowledgeBoundariesPlaceholder": "Argomenti che non conosce",
          "researchSeeds": "Semi di ricerca",
          "researchSeedsPlaceholder": "Argomenti iniziali per la ricerca di background",
          "researchEnabled": "Ricerca attivata",
          "researchEnabledDesc": "Consenti la raccolta di conoscenze in background",
          "physicalDescription": "Descrizione fisica",
          "physicalDescPlaceholder": "Aspetto fisico e manierismi...",
          "physicalHabits": "Abitudini fisiche",
          "physicalHabitsPlaceholder": "es. tamburella con le dita, si aggiusta gli occhiali",
          "idleBehaviors": "Comportamenti a riposo",
          "idleBehaviorsPlaceholder": "Cosa fa quando non è impegnato",
          "timeBehaviors": "Comportamenti temporali",
          "timePlaceholder": "Cosa fa durante {{period}}?",
          "earlyMorning": "Prima mattina",
          "morning": "Mattina",
          "afternoon": "Pomeriggio",
          "evening": "Sera",
          "night": "Notte",
          "baselineEmotions": "Emozioni di base (Plutchik)",
          "emotionDesc": "Imposta la linea di base emotiva predefinita (0 = nessuna, 1 = massima)",
          "joy": "Gioia",
          "trust": "Fiducia",
          "fear": "Paura",
          "surprise": "Sorpresa",
          "sadness": "Tristezza",
          "disgust": "Disgusto",
          "anger": "Rabbia",
          "anticipation": "Anticipazione",
          "engineOverrides": "Override dell'engine",
          "backend": "Backend",
          "model": "Modello",
          "temperature": "Temperatura",
          "leaveEmpty": "Lascia vuoto per il predefinito"
        },
        "review": {
          "title": "Revisione",
          "subtitle": "Rivedi il tuo personaggio prima di crearlo.",
          "edit": "Modifica",
          "notSet": "Non impostato",
          "identitySection": "Identità",
          "personalitySection": "Personalità",
          "worldSection": "Mondo e comportamento",
          "nameLabel": "Nome",
          "eraLabel": "Epoca",
          "roleLabel": "Ruolo",
          "settingLabel": "Ambientazione",
          "coreIdentityLabel": "Identità fondamentale",
          "backstoryLabel": "Storia personale",
          "traitsLabel": "Tratti",
          "formalityLabel": "Formalità",
          "verbosityLabel": "Prolissità",
          "dialectLabel": "Dialetto",
          "catchphrasesLabel": "Frasi tipiche",
          "domainsLabel": "Domini",
          "boundariesLabel": "Limiti",
          "researchSeedsLabel": "Semi di ricerca",
          "researchLabel": "Ricerca",
          "enabled": "Attivato",
          "disabled": "Disattivato",
          "physicalLabel": "Fisico",
          "habitsLabel": "Abitudini",
          "idleLabel": "A riposo",
          "timeBehaviorsLabel": "Comportamenti temporali",
          "emotionsLabel": "Emozioni",
          "configured": "Configurato",
          "backendLabel": "Backend",
          "modelLabel": "Modello",
          "temperatureLabel": "Temperatura",
          "creating": "Creazione...",
          "createCharacter": "Crea personaggio"
        }
      }
    }
  },
  "library": {
    "filterTitle": "Filtra libreria",
    "filters": {
      "all": "Tutti",
      "characters": "Personaggi",
      "personas": "Persona",
      "lorebooks": "Lorebook",
      "images": "Immagini"
    },
    "emptyStates": {
      "all": {
        "title": "La tua libreria è vuota",
        "description": "Crea personaggi, persona e lorebook per vederli qui"
      },
      "characters": {
        "title": "Nessun personaggio",
        "description": "Crea il tuo primo personaggio per iniziare a chattare"
      },
      "personas": {
        "title": "Nessuna persona",
        "description": "Crea una persona per personalizzare la tua identità in chat"
      },
      "lorebooks": {
        "title": "Nessun lorebook",
        "description": "I lorebook vengono creati dalle impostazioni di un personaggio"
      }
    },
    "actions": {
      "startChat": "Inizia chat",
      "editCharacter": "Modifica personaggio",
      "editPersona": "Modifica persona",
      "editLorebook": "Modifica lorebook",
      "renameLorebook": "Rinomina lorebook",
      "exportCharacter": "Esporta personaggio",
      "exportPersona": "Esporta persona",
      "chatAppearance": "Aspetto della chat",
      "deleteCharacter": "Elimina personaggio",
      "deletePersona": "Elimina persona",
      "deleteLorebook": "Elimina lorebook",
      "importLorebook": "Importa lorebook"
    },
    "imageLibrary": {
      "filters": {
        "all": "Tutto",
        "backgrounds": "Sfondi",
        "avatars": "Avatar",
        "attachments": "Allegati",
        "other": "Altro"
      },
      "searchPlaceholder": "Cerca per nome file, percorso, id sessione o id entità",
      "empty": {
        "title": "Nessuna immagine corrisponde a questa vista",
        "description": "Prova un filtro o un termine di ricerca diverso. La libreria mostra solo immagini già archiviate nello spazio locale dell'app."
      },
      "actions": {
        "sort": "Ordina",
        "useThis": "Usa questa",
        "using": "Uso in corso...",
        "copyPath": "Copia percorso",
        "saving": "Salvataggio...",
        "download": "Scarica",
        "delete": "Elimina immagine",
        "deleting": "Eliminazione..."
      },
      "active": "Attivo",
      "messages": {
        "loadFailed": "Impossibile caricare la libreria immagini",
        "saved": "Immagine salvata",
        "downloadFailed": "Download non riuscito",
        "useFailed": "Impossibile usare questa immagine",
        "deleted": "Immagine eliminata",
        "deleteFailed": "Impossibile eliminare l'immagine"
      },
      "deleteConfirm": {
        "title": "Eliminare l'immagine?",
        "message": "Vuoi davvero eliminare \"{{filename}}\"? Questo potrebbe rompere avatar, sfondi chat o allegati dei messaggi che la usano ancora."
      },
      "sort": {
        "newest": "Piu recenti",
        "largest": "Piu grandi",
        "name": "Nome"
      },
      "kinds": {
        "background": "Sfondo",
        "avatar": "Avatar",
        "attachment": "Allegato",
        "stored": "Archiviato"
      },
      "detailsTitle": "Dettagli {{kind}}",
      "formatsLabel": "Formati",
      "storagePath": "Percorso di archiviazione",
      "contextLabel": "Contesto",
      "contextLinkedFallback": "Collegato",
      "show": "Mostra",
      "hide": "Nascondi",
      "contextRoles": {
        "character": "personaggio:",
        "session": "sessione:",
        "role": "ruolo:"
      },
      "downloadFormat": "Formato {{download}}",
      "unknownDate": "Sconosciuto",
      "clearSearch": "Cancella ricerca",
      "copyFilename": "Copia nome file",
      "copyLabels": {
        "filename": "Nome file",
        "storagePath": "Percorso di archiviazione"
      },
      "copy": {
        "copied": "{{label}} copiato",
        "failed": "Copia di {{label}} fallita"
      }
    },
    "deleteConfirm": {
      "title": "Eliminare {{itemType}}?",
      "message": "Sei sicuro di voler eliminare",
      "characterWarning": "Questo eliminerà anche tutte le sessioni di chat con questo personaggio."
    },
    "rename": {
      "title": "Rinomina lorebook",
      "placeholder": "Inserisci nuovo nome..."
    },
    "itemTypes": {
      "character": "Personaggio",
      "persona": "Persona",
      "lorebook": "Lorebook"
    },
    "lorebookLabel": "Lorebook",
    "noDescriptionYet": "Nessuna descrizione",
    "errors": {
      "importLorebook": "Importazione lorebook fallita. {{error}}",
      "exportFailed": "Esportazione fallita"
    },
    "card": {
      "avatarAlt": "Avatar di {{name}}"
    },
    "lorebookEditor": {
      "titleOverride": "Lorebook - {{name}}",
      "dragToReorder": "Trascina per riordinare",
      "aria": {
        "generateEntry": "Genera voce lorebook",
        "editLorebook": "Modifica lorebook",
        "exportLorebook": "Esporta lorebook"
      }
    }
  },
  "onboarding": {
    "loading": "Caricamento provider...",
    "stepIndicator": "Passaggio {{current}} di {{total}}",
    "steps": {
      "provider": "Configurazione provider",
      "model": "Configurazione modello",
      "memory": "Sistema di memoria",
      "stepNofM": "Passo {{current}} di {{total}}"
    },
    "provider": {
      "availableProviders": "Provider disponibili",
      "chooseProvider": "Scegli un provider",
      "titleMobile": "Scegli il tuo provider AI",
      "descMobile": "Seleziona un provider AI per iniziare. Le tue chiavi API sono crittografate in modo sicuro sul tuo dispositivo. Non è necessaria la registrazione.",
      "configureProvider": "Configura {{name}}",
      "connectProvider": "Connetti {{name}}",
      "connectProviderDesc": "Incolla la tua chiave API qui sotto per abilitare le chat. Hai bisogno di una chiave? Ottienila dalla dashboard del provider.",
      "localLLMs": "LLM locali",
      "useLocalLLMs": "Voglio usare LLM locali",
      "browseModelLibrary": "Sfoglia libreria modelli",
      "browseModelLibraryDesc": "Cerca e scarica modelli GGUF da HuggingFace",
      "useOwnGguf": "Usa i miei file GGUF",
      "useOwnGgufDesc": "Seleziona un modello GGUF e un file mmproj opzionale dal tuo dispositivo",
      "fields": {
        "displayLabel": "Etichetta display",
        "displayLabelHint": "Come questo provider apparirà nei tuoi menu",
        "displayLabelPlaceholder": "Il mio {{name}}",
        "defaultLabelFallback": "Provider",
        "apiKey": "Chiave API",
        "apiKeyOptional": "Chiave API (opzionale)",
        "apiKeyHint": "Le chiavi sono crittografate localmente",
        "apiKeyPlaceholderRemote": "sk-...",
        "apiKeyPlaceholderLocal": "Di solito non richiesta",
        "whereToFind": "Dove trovarla",
        "baseUrl": "URL base",
        "baseUrlPlaceholderHost": "http://192.168.1.10:3333",
        "baseUrlPlaceholderLocal": "http://localhost:11434",
        "baseUrlPlaceholderRemote": "https://api.provider.com",
        "baseUrlPlaceholderIntenseRP": "http://127.0.0.1:7777/v1",
        "baseUrlHintLocal": "L'indirizzo del tuo server locale con porta",
        "baseUrlHintHost": "Inserisci l'URL host del desktop mostrato dal dispositivo host",
        "baseUrlHintRemote": "Sovrascrivi l'endpoint predefinito se necessario",
        "chatEndpoint": "Endpoint chat",
        "systemRole": "Ruolo sistema",
        "userRole": "Ruolo utente",
        "assistantRole": "Ruolo assistente",
        "supportsStreaming": "Supporta lo streaming",
        "mergeSameRole": "Unisci messaggi dello stesso ruolo",
        "toolChoiceMode": "Modalità scelta strumento",
        "toolChoiceHint": "Controlla come tool_choice viene inviato all'endpoint personalizzato."
      },
      "toolChoice": {
        "auto": "Auto",
        "required": "Richiesto",
        "none": "Nessuno",
        "omit": "Ometti campo",
        "passthrough": "Passthrough (Tool Config)"
      },
      "buttons": {
        "testConnection": "Testa connessione",
        "testing": "Test in corso..."
      },
      "descriptions": {
        "chutes": "Inferenza compatibile con OpenAI per i principali modelli open-source",
        "openai": "Modelli GPT-5, GPT-4.1 e GPT-4o per RP espressivo",
        "lettuceHost": "Connettiti al tuo desktop Lettuce Host sulla LAN con API in stile OpenAI",
        "anthropic": "Claude 4.5 Sonnet e Haiku per dialoghi profondi ed emotivi",
        "aggregator": "Accedi a modelli come GPT-5, Claude 4.5, Grok-3, Mixtral e altri",
        "openaiCompatible": "Usa qualsiasi endpoint API in stile OpenAI",
        "mistral": "Mistral Small 3.2, Mixtral 8x22B e altri modelli Mistral",
        "deepseek": "DeepSeek-V3.2-Exp, DeepSeek-R1 e altri modelli ad alta efficienza",
        "xai": "Grok-1.5, Grok-3 e modelli xAI più recenti",
        "zai": "GLM-4.5, GLM-4.6 e varianti Air",
        "moonshot": "Kimi-K2 Thinking e modelli Kimi-K1",
        "gemini": "Gemini 2.5 Flash, 2.5 Pro e altri",
        "qwen": "Qwen3-VL e modelli Qwen più recenti",
        "nvidia": "Nemotron, Llama, DeepSeek e altri tramite NVIDIA NIM",
        "custom": "Punta LettuceAI a qualsiasi endpoint modello personalizzato",
        "fallback": "Provider di modelli AI"
      },
      "descriptionsShort": {
        "chutes": "Inferenza modelli open-source",
        "openai": "GPT-5, GPT-4o, GPT-4.1",
        "lettuceHost": "Il tuo host LAN",
        "anthropic": "Claude 4.5 Sonnet e Haiku",
        "aggregator": "Aggregatore multi-modello",
        "openaiCompatible": "Endpoint OpenAI personalizzato",
        "mistral": "Modelli Mistral e Mixtral",
        "deepseek": "DeepSeek-V3, R1",
        "xai": "Grok-1.5, Grok-3",
        "zai": "GLM-4.5, GLM-4.6",
        "moonshot": "Kimi-K2 Thinking",
        "gemini": "Gemini 2.5 Flash e Pro",
        "qwen": "Modelli Qwen3-VL",
        "nvidia": "Inferenza NVIDIA NIM",
        "custom": "Endpoint personalizzato",
        "fallback": "Provider AI"
      },
      "cardLabel": "{{step}}: {{name}}",
      "errors": {
        "hostUrlRequired": "L'URL host è richiesto (es. http://192.168.1.10:3333)",
        "baseUrlRequired": "L'URL base è richiesto (es. http://localhost:11434)",
        "apiKeyTooShort": "La chiave API sembra troppo corta",
        "invalidApiKey": "Chiave API non valida",
        "connectionFailed": "Connessione fallita",
        "verificationFailed": "Verifica fallita",
        "failedToSave": "Salvataggio del provider fallito",
        "connectionSuccessful": "Connessione riuscita!",
        "modelNotFound": "Modello non trovato nel provider",
        "modelVerificationFailed": "Verifica del modello fallita",
        "failedToSaveModel": "Salvataggio del modello fallito"
      }
    },
    "model": {
      "noProvidersTitle": "Nessun provider configurato",
      "noProvidersDesc": "Dovrai connettere un provider prima di scegliere un modello predefinito.",
      "goToProviderSetup": "Vai alla configurazione provider",
      "yourProviders": "I tuoi provider",
      "yourProvidersHint": "Seleziona quale provider usare",
      "setDefaultModel": "Imposta il tuo modello predefinito",
      "setDefaultModelDesc": "Scegli quale provider e nome modello LettuceAI dovrebbe usare di default. Potrai aggiungerne altri in seguito.",
      "setDefaultModelDescDesktop": "Seleziona un provider dall'elenco per configurare il tuo modello.",
      "modelDetails": "Dettagli modello",
      "modelDetailsDesc": "Definisci l'identificatore API e l'etichetta che vedrai nell'app.",
      "whichModel": "Quale modello dovrei usare?",
      "nextMemorySystem": "Avanti: sistema di memoria",
      "fields": {
        "displayName": "Nome display",
        "displayNamePlaceholder": "Mentore creativo",
        "displayNameHint": "Come questo modello appare nei menu",
        "modelId": "ID modello",
        "modelPathGguf": "Percorso modello (GGUF)",
        "modelIdPlaceholder": "es. gpt-4o",
        "modelPathPlaceholder": "/percorso/al/modello.gguf",
        "modelIdHint": "Identificatore esatto usato per le chiamate API",
        "showList": "Mostra elenco",
        "manualInput": "Inserimento manuale",
        "refreshModelList": "Aggiorna elenco modelli",
        "selectModel": "Seleziona modello",
        "selectAModel": "Seleziona un modello...",
        "searchModels": "Cerca modelli...",
        "noModelsFound": "Nessun modello trovato per \"{{query}}\""
      },
      "fillBothFields": "Compila entrambi i campi sopra per abilitare il pulsante fine.",
      "providerNames": {
        "chutes": "Chutes",
        "intenseRpNext": "IntenseRP Next",
        "openai": "OpenAI",
        "anthropic": "Anthropic",
        "openrouter": "OpenRouter",
        "openaiCompatible": "Compatibile OpenAI",
        "custom": "Endpoint personalizzato"
      }
    },
    "memory": {
      "dynamicTitle": "Memoria dinamica",
      "recommended": "Consigliato",
      "settingUp": "Configurazione...",
      "finishSetup": "Completa configurazione",
      "promptTitle": "Configura memoria dinamica",
      "oneLastStep": "Un ultimo passaggio",
      "downloadAndEnable": "Scarica e attiva",
      "chooseStyle": "Scegli il tuo stile di memoria",
      "howRemember": "Come dovrebbero ricordare i tuoi compagni AI i dettagli su di te e le vostre conversazioni?",
      "dynamicDescription": "Usa un <0>modello di embedding locale</0> per gestire il contesto in modo intelligente. Riduce i costi di token mantenendo alta qualità, anche in chat lunghe.",
      "dynamicFeatures": {
        "quality": "Mantiene la qualità nelle chat lunghe",
        "cost": "Riduce significativamente i costi API",
        "auto": "Gestione automatica del contesto",
        "zeroConfig": "Zero configurazione necessaria"
      },
      "manualTitle": "Memoria manuale",
      "manualBadge": "Esperienza classica",
      "manualDescription": "Fissi esplicitamente i messaggi e modifichi tu stesso le \"Informazioni mondo\" o le definizioni dei personaggi. Ottimo per il controllo totale.",
      "manualFeatures": {
        "control": "Controllo totale sui fatti",
        "scenarios": "Migliore per scenari specifici"
      },
      "setupModelMessage": "Per usare la memoria dinamica, dobbiamo scaricare un piccolo modello di embedding (~120MB) sul tuo dispositivo.",
      "setupBullets": {
        "offline": "Il modello funziona al 100% offline sul tuo dispositivo",
        "remembering": "Necessario per ricordare il contesto",
        "disable": "Puoi disabilitarlo in seguito nelle impostazioni"
      },
      "stepLabel": "Passo 3 di 3",
      "stepLabelMemory": "Sistema di memoria"
    },
    "welcome": {
      "appName": "LettuceAI",
      "tagline": "Il tuo compagno AI personale. Privato, sicuro e sempre sul dispositivo.",
      "features": {
        "onDevice": "Solo sul dispositivo",
        "characterReady": "Personaggi pronti"
      },
      "betaWarning": {
        "title": "Build Beta Desktop",
        "description": "Stai usando la versione desktop. Alcune funzionalità potrebbero differire dal mobile. Segnala problemi su GitHub."
      },
      "languageSelector": {
        "title": "Lingua",
        "description": "Rilevata automaticamente dal dispositivo. Puoi cambiarla in qualsiasi momento nelle impostazioni."
      },
      "getStarted": "Inizia",
      "skipForNow": "Salta per ora",
      "restoreFromBackup": "Ripristina da backup",
      "setupTime": "La configurazione richiede meno di 2 minuti",
      "skipWarning": {
        "title": "Saltare la configurazione?",
        "warningTitle": "Provider necessario per chattare",
        "warningMessage": "Senza un provider, non potrai inviare messaggi. Puoi aggiungerne uno dopo dalle impostazioni.",
        "addProvider": "Aggiungi provider",
        "skipAnyway": "Salta comunque"
      },
      "restoreBackup": {
        "title": "Ripristina backup",
        "selectMessage": "Seleziona un backup da ripristinare.",
        "browse": "Sfoglia file",
        "processing": "Elaborazione file...",
        "processingNote": "I backup grandi potrebbero richiedere un minuto",
        "noBackups": "Nessun backup trovato",
        "noBackupsHint": "Tocca sfoglia per selezionare un file .lettuce",
        "browseLettuce": "Cerca file .lettuce",
        "passwordLabel": "Password del backup",
        "passwordPlaceholder": "Inserisci password",
        "restoreButton": "Ripristina backup",
        "restoring": "Ripristino...",
        "infoMessage": "Questo configurerà l'app con i tuoi dati di backup, inclusi personaggi, chat e impostazioni.",
        "embeddingTitle": "Modello di embedding richiesto",
        "dynamicMemoryDetected": "Memoria dinamica rilevata",
        "dynamicMemoryMessage": "Questo backup contiene personaggi con memoria dinamica attivata, che richiede il modello di embedding (~120MB).",
        "embeddingOptions": "Puoi scaricare il modello ora per attivare la memoria dinamica, o continuare senza (la memoria dinamica verrà disattivata per i personaggi interessati).",
        "downloadModel": "Scarica modello",
        "continueWithoutDynamic": "Continua senza memoria dinamica",
        "embeddingNote": "Puoi riattivare la memoria dinamica più tardi nelle impostazioni del personaggio dopo aver scaricato il modello.",
        "back": "Indietro",
        "cancel": "Annulla",
        "errors": {
          "passwordRequired": "La password è richiesta",
          "incorrectPassword": "Password errata",
          "failedToOpenFile": "Apertura del file fallita",
          "failedToRestore": "Ripristino del backup fallito",
          "failedToUpdateSettings": "Aggiornamento delle impostazioni fallito"
        }
      }
    },
    "common": {
      "back": "Indietro",
      "cancel": "Annulla",
      "continue": "Continua",
      "verifying": "Verifica in corso...",
      "skipForNow": "Salta per ora",
      "selectAProvider": "Seleziona un provider da configurare",
      "clickToSelectProvider": "Clicca per selezionare un provider",
      "selectProviderFromList": "Seleziona un provider dall'elenco per iniziare.",
      "enterApiKey": "Inserisci la tua chiave API per abilitare la funzionalità di chat AI."
    },
    "modelGuide": {
      "badge": "Guida ai modelli",
      "title": "Come scelgo un modello?",
      "intro": "LettuceAI non impone un singolo modello \"migliore\". Invece, scegli quello che si adatta al tuo <0>caso d'uso, budget e stile</0>. Usa questa guida per decidere cosa provare e dove cercarlo.",
      "askYourself": "Chiediti:",
      "factors": {
        "quality": {
          "title": "Qualita e capacita",
          "description": "Quanto deve essere intelligente il modello? I modelli piu grandi e nuovi di solito ragionano meglio, scrivono testi piu belli e gestiscono prompt complicati con piu grazia.",
          "q1": "Hai bisogno di una profonda coerenza del personaggio e intelligenza emotiva?",
          "q2": "Ti importa la narrativa immersiva e personalita dei personaggi credibili?",
          "q3": "Vuoi che il modello ricordi i dettagli del personaggio e rimanga nel personaggio durante sessioni lunghe?"
        },
        "speed": {
          "title": "Velocita e latenza",
          "description": "I modelli piu veloci si sentono meglio per conversazioni rapide e di andata e ritorno. Alcuni modelli scambiano un po' di qualita per molta piu velocita.",
          "q1": "Vuoi risposte quasi istantanee per mantenere il roleplay fluente?",
          "q2": "Stai facendo scene di dialogo rapide dove aspettare romperebbe l'immersione?",
          "q3": "E per RP casuale dove il botta e risposta rapido conta piu delle risposte perfette?"
        },
        "budget": {
          "title": "Budget e utilizzo",
          "description": "Ogni provider fattura per token. Anche i modelli economici si accumulano se chatti molto, quindi scegli qualcosa che corrisponda alla frequenza e intensita dell'utilizzo.",
          "q1": "Sei disposto a pagare di piu per interazioni con personaggi piu ricche, o vuoi qualcosa di economico per il roleplay quotidiano?",
          "q2": "Hai modelli gratuiti dal tuo provider/router che puoi provare prima?",
          "q3": "Condurrai sessioni di roleplay lunghe con descrizioni dettagliate delle scene?",
          "q4": "Hai un budget mensile fisso che non vuoi superare?"
        },
        "safety": {
          "title": "Sicurezza, privacy ed extra",
          "description": "I provider differiscono nel modo in cui gestiscono la sicurezza, il logging e le funzionalita extra come immagini, strumenti o finestre di contesto lunghe.",
          "q1": "Hai bisogno di meno filtri sui contenuti per scenari di roleplay maturi o creativi?",
          "q2": "Ti importa se le tue conversazioni RP private vengono registrate o usate per l'addestramento?",
          "q3": "Hai bisogno di finestre di contesto lunghe per trame complesse e storie dei personaggi?"
        }
      },
      "where": {
        "title": "Dove posso trovare i modelli?",
        "intro": "La maggior parte dei provider e router ha un <0>elenco o catalogo di modelli</0>. Sfoglia quelle pagine per vedere cosa offrono, prezzi, limiti e funzionalita speciali.",
        "directTitle": "Provider diretti",
        "directDesc": "OpenAI, Anthropic, Google Gemini, xAI, Mistral, ecc. Ognuno ha una console/dashboard dove puoi vedere nomi ufficiali dei modelli, capacita e prezzi.",
        "routersTitle": "Router e hub",
        "routersDesc": "Servizi come OpenRouter o altri aggregatori elencano molti modelli di diversi provider in un unico posto, spesso con confronti di benchmark e prezzi.",
        "communityTitle": "Raccomandazioni della community",
        "communityDesc": "Guarda la documentazione, blog o post della community del tuo provider/router. Di solito evidenziano quali modelli sono migliori per chat, coding o velocita."
      },
      "rules": {
        "title": "Regole pratiche semplici",
        "casual": "Per chat casual: scegli un modello di chat veloce ed economico dal tuo provider/router.",
        "experiments": "Per esperimenti o alto volume: inizia con il modello piu economico che ti sembra abbastanza buono, poi aggiorna se necessario.",
        "switch": "Se qualcosa non va (troppo lento / troppo stupido / troppo costoso): puoi sempre cambiare modello in seguito in LettuceAI."
      },
      "disclaimer": "Controlla sempre la documentazione del provider per l'elenco aggiornato di modelli, limiti e prezzi. Questa pagina riguarda come pensare, non cosa comprare."
    },
    "whereToFind": {
      "badge": "Aiuto chiave API",
      "intro": "Segui questi passaggi per ottenere la tua chiave API, poi torna a LettuceAI e incollala nelle impostazioni del provider.",
      "readyPrompt": "Pronto per ottenere la chiave?",
      "openProviderSite": "Apri sito del provider",
      "keyWarning": "Non condividere mai la tua chiave API pubblicamente. Chiunque con questa chiave puo usare il tuo saldo account.",
      "stuckPrompt": "Non riesci ancora a capire?",
      "joinDiscord": "Unisciti al nostro server Discord per ricevere aiuto",
      "guides": {
        "chutes": {
          "title": "Come trovare la tua chiave API Chutes",
          "s1": "Vai su chutes.ai/app e accedi.",
          "s2": "Apri la tua area account/impostazioni e trova le Chiavi API.",
          "s3": "Crea una nuova chiave (o copia una esistente).",
          "s4": "Incolla la chiave in LettuceAI."
        },
        "openai": {
          "title": "Come trovare la tua chiave API OpenAI",
          "s1": "Vai su platform.openai.com e accedi.",
          "s2": "Clicca il tuo avatar profilo in alto a destra, poi scegli Chiavi API.",
          "s3": "Clicca Crea nuova chiave segreta e copia il valore mostrato.",
          "s4": "Incolla la chiave in LettuceAI e conservala al sicuro. Non la vedrai piu."
        },
        "anthropic": {
          "title": "Come trovare la tua chiave API Anthropic",
          "s1": "Vai su console.anthropic.com e accedi.",
          "s2": "Apri Impostazioni dalla barra laterale sinistra.",
          "s3": "Seleziona Chiavi API e clicca Crea chiave.",
          "s4": "Copia la chiave e incollala in LettuceAI."
        },
        "openrouter": {
          "title": "Come trovare la tua chiave API OpenRouter",
          "s1": "Visita openrouter.ai e accedi.",
          "s2": "Apri la pagina Chiavi dal menu del tuo profilo.",
          "s3": "Clicca Crea chiave, dagli un nome e salva.",
          "s4": "Copia la chiave e incollala in LettuceAI."
        },
        "mistral": {
          "title": "Come trovare la tua chiave API Mistral",
          "s1": "Vai su console.mistral.ai e accedi.",
          "s2": "Clicca Chiavi API nella barra laterale.",
          "s3": "Clicca Crea una chiave API.",
          "s4": "Copia la chiave e incollala in LettuceAI."
        },
        "deepseek": {
          "title": "Come trovare la tua chiave API DeepSeek",
          "s1": "Apri platform.deepseek.com e accedi.",
          "s2": "Clicca Chiavi API nella navigazione in alto.",
          "s3": "Crea una nuova chiave se non ne hai gia una.",
          "s4": "Copia la chiave e incollala in LettuceAI."
        },
        "groq": {
          "title": "Come trovare la tua chiave API Groq",
          "s1": "Visita console.groq.com e accedi.",
          "s2": "Apri Chiavi API dalla barra laterale.",
          "s3": "Crea una nuova chiave, poi copiala.",
          "s4": "Incolla la chiave in LettuceAI."
        },
        "gemini": {
          "title": "Come trovare la tua chiave API Google Gemini",
          "s1": "Vai su Google AI Studio su aistudio.google.com e accedi.",
          "s2": "Clicca Ottieni chiave API o Gestisci chiavi.",
          "s3": "Crea una nuova chiave se necessario.",
          "s4": "Copia la chiave e incollala in LettuceAI."
        },
        "xai": {
          "title": "Come trovare la tua chiave API xAI",
          "s1": "Apri console.x.ai e accedi.",
          "s2": "Naviga alla sezione Chiavi API nella console.",
          "s3": "Crea una nuova chiave.",
          "s4": "Copia la chiave e incollala in LettuceAI."
        },
        "zai": {
          "title": "Come trovare la tua chiave API zAI (GLM)",
          "s1": "Vai su open.bigmodel.cn e accedi.",
          "s2": "Apri Centro utente, poi vai su Chiavi API.",
          "s3": "Crea una nuova chiave se non ne hai una.",
          "s4": "Copia la chiave e incollala in LettuceAI."
        },
        "moonshot": {
          "title": "Come trovare la tua chiave API Moonshot (Kimi)",
          "s1": "Visita platform.moonshot.cn e accedi.",
          "s2": "Apri la sezione Chiavi API nella console.",
          "s3": "Crea una nuova chiave e copiala.",
          "s4": "Incolla la chiave in LettuceAI."
        },
        "qwen": {
          "title": "Come trovare la tua chiave API Qwen",
          "s1": "Apri dashscope.aliyun.com e accedi.",
          "s2": "Vai alla sezione Chiavi API nella barra laterale.",
          "s3": "Crea una nuova chiave.",
          "s4": "Copia la chiave e incollala in LettuceAI."
        },
        "nanogpt": {
          "title": "Come trovare la tua chiave API NanoGPT",
          "s1": "Vai su nano-gpt.com e accedi.",
          "s2": "Apri la dashboard e vai alla sezione chiavi API.",
          "s3": "Crea una nuova chiave se necessario.",
          "s4": "Copia la chiave e incollala in LettuceAI."
        },
        "featherless": {
          "title": "Come trovare la tua chiave API Featherless",
          "s1": "Visita featherless.ai e accedi.",
          "s2": "Apri il tuo account o sezione API dalla dashboard.",
          "s3": "Crea una nuova chiave se non ne vedi una.",
          "s4": "Copia la chiave e incollala in LettuceAI."
        },
        "anannas": {
          "title": "Come trovare la tua chiave API Anannas",
          "s1": "Vai su dashboard.anannas.ai e accedi.",
          "s2": "Naviga alla sezione Chiavi API.",
          "s3": "Crea una nuova chiave e copiala.",
          "s4": "Incolla la chiave in LettuceAI."
        },
        "default": {
          "title": "Come trovare la tua chiave API",
          "s1": "Apri la dashboard del tuo provider AI in un browser e accedi.",
          "s2": "Cerca le impostazioni API, Sviluppatore o Integrazioni.",
          "s3": "Crea una nuova chiave API o visualizza una esistente.",
          "s4": "Copia la chiave e incollala in LettuceAI."
        }
      }
    },
    "welcomeFooter": {
      "setupTimeMobile": "La configurazione richiede meno di 2 minuti"
    }
  },
  "search": {
    "placeholder": "Cerca...",
    "tabs": {
      "characters": "Personaggi",
      "personas": "Persona"
    },
    "noResults": "Nessun {{type}} trovato",
    "emptyState": "Nessun {{type}} ancora",
    "noResultsHint": "Prova un termine di ricerca diverso",
    "emptyCharacters": "Crea il tuo primo personaggio per iniziare a chattare",
    "emptyPersonas": "Crea una persona nelle impostazioni",
    "a11y": {
      "goBack": "Torna indietro",
      "clearSearch": "Cancella ricerca",
      "characterAvatar": "Avatar di {{name}}"
    },
    "session": {
      "newChatTitle": "Nuova chat"
    },
    "noDescription": "Nessuna descrizione",
    "defaultBadge": "Predefinito"
  },
  "sync": {
    "modes": {
      "join": "Unisciti",
      "joinDesc": "Connetti all'host",
      "host": "Host",
      "hostDesc": "Condividi i tuoi dati"
    },
    "sections": {
      "mode": "Modalità",
      "connectToHost": "Connetti all'host",
      "startHosting": "Inizia hosting",
      "status": "Stato",
      "hosting": "Servizio hosting",
      "localAddress": "Indirizzo rete locale",
      "connectionPin": "PIN di connessione",
      "setupGuide": "Guida alla configurazione"
    },
    "fields": {
      "hostAddress": "Indirizzo host o JSON",
      "hostPlaceholder": "es. 192.168.1.100:12345",
      "pinCode": "Codice PIN",
      "pinPlaceholder": "000000"
    },
    "buttons": {
      "scanQRCode": "Scansiona codice QR",
      "connect": "Connetti",
      "connecting": "Connessione...",
      "startHosting": "Avvia hosting",
      "startingServer": "Avvio server...",
      "stopHosting": "Ferma hosting",
      "hostAgain": "Ospita di nuovo",
      "done": "Fatto"
    },
    "status": {
      "connecting": "Connessione...",
      "connected": "Connesso",
      "waitingConfirmation": "In attesa di conferma",
      "waitingConfirmationDesc": "Approva la connessione sul dispositivo host per continuare.",
      "syncing": "Sincronizzazione...",
      "transferringData": "Trasferimento dati",
      "syncInProgress": "Sincronizzazione in corso",
      "live": "Attivo",
      "broadcasting": "Trasmissione",
      "clientsLabel": "Connessi",
      "clientsUnit": "Client"
    },
    "pinDescription": "Condividi questo PIN con il dispositivo che si connette",
    "hostingDesc1": "Altri dispositivi possono connettersi e sincronizzare i dati da questo dispositivo.",
    "hostingDesc2": "I tuoi dati verranno condivisi con i client connessi.",
    "setupSteps": {
      "step1": "Apri l'app su un altro dispositivo",
      "step2": "Vai a Impostazioni → Sincronizzazione locale",
      "step3": "Scansiona il codice QR o inserisci l'indirizzo"
    },
    "messages": {
      "completed": "Sincronizzazione completata!",
      "completedDesc": "Tutti i dati sincronizzati",
      "error": "Errore di connessione",
      "outdatedClient": "Client obsoleto rilevato"
    },
    "disclaimer": "La sincronizzazione funziona sulla tua rete locale. Entrambi i dispositivi devono essere sulla stessa rete WiFi.",
    "modals": {
      "connectionRequest": "Richiesta di connessione",
      "requestMessage": "vuole sincronizzarsi con questo dispositivo.",
      "acceptConnection": "Accetta connessione",
      "acceptDesc": "Consenti a questo dispositivo di sincronizzare i dati",
      "decline": "Rifiuta",
      "declineDesc": "Blocca questo tentativo di connessione",
      "readyToSync": "Pronto per la sincronizzazione",
      "connectionEstablished": "Connessione stabilita",
      "deviceReady": "è pronto.",
      "startSyncMessage": "Tocca sotto per iniziare la sincronizzazione dei dati.",
      "startSyncing": "Avvia sincronizzazione",
      "startSyncingDesc": "Inizia il trasferimento dati ora"
    },
    "scanner": {
      "title": "Scansiona codice QR",
      "cancel": "Annulla scansione"
    },
    "unknownDevice": "Dispositivo sconosciuto",
    "aria": {
      "dismissStatus": "Chiudi stato sincronizzazione",
      "dismissError": "Chiudi errore sincronizzazione"
    },
    "stats": {
      "statusLabel": "Stato"
    }
  },
  "creationHelper": {
    "page": {
      "info": "L'Assistente alla Creazione ti guida nella costruzione di personaggi con l'assistenza dell'IA. Configura il modello e gli strumenti usati durante la creazione dei personaggi.",
      "modelConfiguration": "Configurazione Modello",
      "chatModel": "Modello di Chat",
      "selectedModel": "Modello Selezionato",
      "useAppDefault": "Usa predefinito dell'app{{model}}",
      "useAppDefaultBase": "Usa predefinito dell'app",
      "noModelsAvailable": "Nessun modello disponibile",
      "chatModelDescription": "Modello IA per le conversazioni di creazione personaggi",
      "streamingOutput": "Output in Streaming",
      "streamingDescription": "Mostra le risposte mentre vengono generate",
      "imageGenerationModel": "Modello di Generazione Immagini",
      "noModelSelected": "Nessun modello selezionato",
      "noImageModelsAvailable": "Nessun modello di immagini disponibile",
      "imageModelDescription": "Per generare avatar dei personaggi",
      "toolSelection": "Selezione Strumenti",
      "smartToolSelection": "Selezione Intelligente Strumenti",
      "smartToolDescription": "L'IA sceglie automaticamente quali strumenti usare",
      "smartToolEnabledHint": "Quando abilitato, l'Assistente alla Creazione chiede cosa vuoi creare e carica solo il set di strumenti pertinente.",
      "smartToolDisabledHint": "Quando disabilitato, l'Assistente alla Creazione si apre direttamente e usa tutti gli strumenti abilitati; l'assistente decide cosa costruire.",
      "quickPresets": "Preset Rapidi",
      "customSelection": "Selezione personalizzata - {{count}} strumenti abilitati",
      "footerInfo": "Quando la Selezione Intelligente Strumenti è abilitata, l'IA decide quali strumenti usare in base al contesto. Disabilitala per controllare manualmente quali strumenti sono disponibili.",
      "selectChatModel": "Seleziona Modello di Chat",
      "selectImageModel": "Seleziona Modello di Immagini",
      "searchModels": "Cerca modelli..."
    },
    "categories": {
      "basic": "Base",
      "content": "Contenuto",
      "visual": "Visuale",
      "settings": "Impostazioni",
      "flow": "Flusso",
      "persona": "Persona",
      "lorebook": "Lorebook"
    },
    "presets": {
      "all": {
        "name": "Tutti gli strumenti",
        "desc": "Attiva tutti gli strumenti disponibili"
      },
      "essential": {
        "name": "Essenziali",
        "desc": "Solo nome, definizione e scene"
      },
      "minimal": {
        "name": "Minimale",
        "desc": "Solo nome e definizione"
      }
    },
    "tools": {
      "setName": "Imposta nome",
      "setNameDesc": "Imposta il nome del personaggio",
      "setDefinition": "Imposta definizione",
      "setDefinitionDesc": "Imposta personalità e background",
      "set_character_name": {
        "name": "Imposta nome",
        "desc": "Imposta il nome del personaggio"
      },
      "set_character_definition": {
        "name": "Imposta definizione",
        "desc": "Imposta personalità e background"
      },
      "add_scene": {
        "name": "Aggiungi scena",
        "desc": "Aggiungi una scena iniziale per il roleplay"
      },
      "update_scene": {
        "name": "Aggiorna scena",
        "desc": "Modifica una scena esistente"
      },
      "toggle_avatar_gradient": {
        "name": "Gradiente avatar",
        "desc": "Attiva/disattiva il gradiente sull'avatar"
      },
      "set_default_model": {
        "name": "Imposta modello",
        "desc": "Imposta il modello AI per le conversazioni"
      },
      "set_system_prompt": {
        "name": "Prompt di sistema",
        "desc": "Imposta le linee guida comportamentali"
      },
      "get_system_prompt_list": {
        "name": "Lista prompt",
        "desc": "Visualizza i prompt disponibili"
      },
      "get_model_list": {
        "name": "Lista modelli",
        "desc": "Visualizza i modelli disponibili"
      },
      "use_uploaded_image_as_avatar": {
        "name": "Immagine come avatar",
        "desc": "Usa l'immagine caricata come avatar"
      },
      "use_uploaded_image_as_chat_background": {
        "name": "Immagine come sfondo",
        "desc": "Usa l'immagine caricata come sfondo"
      },
      "generate_image": {
        "name": "Genera immagine",
        "desc": "Genera un'immagine con il modello AI"
      },
      "show_preview": {
        "name": "Mostra anteprima",
        "desc": "Anteprima del personaggio"
      },
      "request_confirmation": {
        "name": "Richiedi conferma",
        "desc": "Chiedi di salvare o continuare"
      },
      "list_personas": {
        "name": "Lista persona",
        "desc": "Sfoglia le persona"
      },
      "upsert_persona": {
        "name": "Salva persona",
        "desc": "Crea o aggiorna una persona"
      },
      "use_uploaded_image_as_persona_avatar": {
        "name": "Avatar persona",
        "desc": "Usa l'immagine caricata come avatar della persona"
      },
      "delete_persona": {
        "name": "Elimina persona",
        "desc": "Rimuovi una persona"
      },
      "get_default_persona": {
        "name": "Persona predefinita",
        "desc": "Recupera la persona predefinita"
      },
      "list_lorebooks": {
        "name": "Lista lorebook",
        "desc": "Sfoglia i lorebook"
      },
      "upsert_lorebook": {
        "name": "Salva lorebook",
        "desc": "Crea o aggiorna un lorebook"
      },
      "delete_lorebook": {
        "name": "Elimina lorebook",
        "desc": "Rimuovi un lorebook"
      },
      "list_lorebook_entries": {
        "name": "Lista voci",
        "desc": "Visualizza le voci del lorebook"
      },
      "get_lorebook_entry": {
        "name": "Ottieni voce",
        "desc": "Recupera una voce del lorebook"
      },
      "upsert_lorebook_entry": {
        "name": "Salva voce",
        "desc": "Crea o aggiorna una voce"
      },
      "delete_lorebook_entry": {
        "name": "Elimina voce",
        "desc": "Rimuovi una voce del lorebook"
      },
      "create_blank_lorebook_entry": {
        "name": "Voce vuota",
        "desc": "Crea una voce segnaposto"
      },
      "reorder_lorebook_entries": {
        "name": "Riordina voci",
        "desc": "Modifica l'ordine delle voci"
      },
      "list_character_lorebooks": {
        "name": "Lista lorebook personaggio",
        "desc": "Vedi i lorebook di un personaggio"
      },
      "set_character_lorebooks": {
        "name": "Imposta lorebook personaggio",
        "desc": "Assegna lorebook a un personaggio"
      },
      "addScene": "Aggiungi scena",
      "addSceneDesc": "Aggiungi una scena iniziale per il roleplay",
      "updateScene": "Aggiorna scena",
      "updateSceneDesc": "Modifica una scena esistente",
      "avatarGradient": "Gradiente avatar",
      "avatarGradientDesc": "Attiva/disattiva il gradiente sovrapposto sull'avatar",
      "setModel": "Imposta modello",
      "setModelDesc": "Imposta il modello AI per le conversazioni",
      "systemPrompt": "Prompt di sistema",
      "systemPromptDesc": "Imposta le linee guida comportamentali",
      "listPrompts": "Lista prompt",
      "listPromptsDesc": "Visualizza i prompt disponibili",
      "listModels": "Lista modelli",
      "listModelsDesc": "Visualizza i modelli disponibili",
      "imageAsAvatar": "Immagine come avatar",
      "imageAsAvatarDesc": "Usa l'immagine caricata come avatar"
    }
  },
  "tour": {
    "stepCounter": "Passo {{current}} di {{total}}",
    "skipTour": "Salta il tour",
    "next": "Avanti",
    "gotIt": "Capito",
    "appShell": {
      "chats": {
        "title": "Qui vivono le tue chat",
        "body": "Tutte le tue conversazioni uno a uno con i personaggi sono qui. Torna quando vuoi e manterremo il tuo posto."
      },
      "groups": {
        "title": "Chatta in gruppo",
        "body": "Porta più personaggi nella stessa stanza e guardali parlare tra loro, oppure unisciti tu stesso quando vuoi."
      },
      "discover": {
        "title": "Scopri nuovi personaggi",
        "body": "Esplora ciò che la community ha condiviso e prendi qualsiasi personaggio ti colpisca. Nuovi preferiti a portata di tap."
      },
      "library": {
        "title": "La tua libreria personale",
        "body": "Tutto ciò che hai creato o salvato vive qui: personaggi, persona, prompt, tutto. Pensalo come il tuo archivio."
      },
      "settings": {
        "title": "Personalizzalo",
        "body": "Cambia provider, scegli modelli diversi, modifica l'aspetto dell'app. Praticamente tutto è regolabile dalle impostazioni."
      },
      "search": {
        "title": "Trova tutto, velocemente",
        "body": "Cerchi una chat o un personaggio specifico? Cerca ovunque da qui. Niente ricerche complicate."
      },
      "create": {
        "title": "E infine, crea!",
        "body": "Tocca il più quando l'ispirazione colpisce. Crea un nuovo personaggio, persona o inizia qualcosa da zero."
      }
    },
    "chatDetail": {
      "chatTitle": {
        "title": "Impostazioni per chat",
        "body": "Tocca il nome del personaggio qui sopra per aprire le impostazioni solo per questa chat. Persona, layout e modelli diversi per conversazione."
      },
      "chatMemory": {
        "title": "Cosa ricordano",
        "body": "L'icona del cervello mostra cosa il tuo personaggio ricorda delle vostre conversazioni. Tocca per rivedere, modificare o cancellare i ricordi."
      },
      "chatSearch": {
        "title": "Trova quella frase",
        "body": "Cerca solo in questa conversazione. Perfetto per trovare quel dettaglio di 200 messaggi fa senza scorrere all'infinito."
      },
      "chatLorebook": {
        "title": "Voci del lorebook",
        "body": "Fatti extra, world-building e contesto che vengono iniettati nel prompt quando compaiono parole chiave specifiche. Il foglietto del tuo personaggio."
      },
      "chatPlus": {
        "title": "Allega cose",
        "body": "Inserisci immagini o apri il menu extra. Tutto ciò che alleghi viene inviato con il tuo prossimo messaggio."
      },
      "chatComposer": {
        "title": "Il tuo messaggio, la tua mossa",
        "body": "Scrivi qui. Invio invia, Shift+Invio va a capo. Suggerimento: premi a lungo qualsiasi messaggio per modificare, ramificare o eliminare."
      },
      "chatSend": {
        "title": "Un pulsante, quattro funzioni",
        "body": "Il pulsante di invio cambia funzione in base a cosa sta succedendo:"
      }
    },
    "postFirstMessage": {
      "chatRegenerate": {
        "title": "Non ti piace? Rigenera",
        "body": "Tocca l'icona di aggiornamento per ottenere una risposta completamente nuova dal personaggio. Ogni rigenerazione viene salvata come variante."
      },
      "chatVariants": {
        "title": "Scorri tra le varianti",
        "body": "Dopo la rigenerazione, vedrai un contatore di varianti sotto il messaggio. Scorri a sinistra o destra sulla bolla del messaggio per sfogliare tutte le risposte diverse."
      },
      "chatLongPress": {
        "title": "C'è di più nascosto qui",
        "body": "Premi a lungo qualsiasi messaggio per modificare, copiare, ramificare, fissare, eliminare o riavvolgere la conversazione. Il clic destro funziona anche su desktop."
      }
    },
    "sendButton": {
      "continue": {
        "label": "Continua",
        "desc": "L'input è vuoto. Toccando qui il personaggio continuerà a parlare."
      },
      "send": {
        "label": "Invia",
        "desc": "Hai digitato o allegato qualcosa. Tocca per inviare."
      },
      "sending": {
        "label": "Invio in corso",
        "desc": "La risposta è in arrivo. Il pulsante è bloccato."
      },
      "stop": {
        "label": "Ferma",
        "desc": "Tocca per annullare durante la risposta se cambi idea."
      }
    },
    "extra": {
      "rerunOnboarding": "Ripeti l'onboarding"
    }
  },
  "sessionAdvanced": {
    "title": "Parametri sessione",
    "subtitle": "Sovrascrivi i valori predefiniti del modello per questa conversazione",
    "goBack": "Torna indietro",
    "support": "Supporto",
    "reset": "Reimposta",
    "save": "Salva",
    "noSessionWarning": "Apri una sessione di chat per configurare le impostazioni per sessione.",
    "overrideDefaults": "Sovrascrivi predefiniti",
    "overrideDefaultsDesc": "Personalizza i parametri solo per questa conversazione",
    "loadingContextInfo": "Caricamento info contesto...",
    "sampling": {
      "title": "Campionamento",
      "temperature": "Temperature",
      "temperatureDesc": "Controlla la casualità. Più basso = più deterministico, più alto = più creativo.",
      "temperaturePrecise": "Preciso",
      "temperatureCreative": "Creativo",
      "topP": "Top P",
      "topPDesc": "Nucleus sampling. Limita i token a una probabilità cumulativa.",
      "topPFocused": "Focalizzato",
      "topPDiverse": "Diverso",
      "topK": "Top K",
      "topKDesc": "Limita il campionamento ai top K token più probabili."
    },
    "outputPenalties": {
      "title": "Output e Penalità",
      "maxOutputTokens": "Token di output massimi",
      "maxOutputTokensDesc": "Lunghezza massima della risposta. Auto lascia decidere al modello.",
      "auto": "Auto",
      "custom": "Personalizzato",
      "frequencyPenalty": "Penalità di frequenza",
      "frequencyPenaltyDesc": "Riduce la ripetizione di sequenze di token.",
      "frequencyPenaltyRepeat": "Ripetizione",
      "frequencyPenaltyVary": "Variazione",
      "presencePenalty": "Penalità di presenza",
      "presencePenaltyDesc": "Incoraggia l'esplorazione di nuovi argomenti.",
      "presencePenaltyRepeat": "Ripetizione",
      "presencePenaltyExplore": "Esplorazione"
    },
    "performance": {
      "title": "Prestazioni",
      "gpuLayers": "GPU Layers",
      "gpuLayersDesc": "Livelli scaricati sulla GPU. 0 = solo CPU.",
      "threads": "Threads",
      "threadsDesc": "Thread CPU per l'inferenza.",
      "batchThreads": "Batch Threads",
      "batchThreadsDesc": "Thread CPU per l'elaborazione batch.",
      "batchSize": "Batch Size",
      "batchSizeDesc": "Dimensione chunk elaborazione prompt.",
      "contextLength": "Lunghezza contesto",
      "contextLengthDesc": "Sovrascrivi dimensione finestra di contesto.",
      "flashAttention": "Flash Attention",
      "flashAttentionDesc": "Ottimizzazione memoria GPU.",
      "enabled": "Abilitato",
      "disabled": "Disabilitato"
    },
    "samplingMemory": {
      "title": "Campionamento e Memoria",
      "minP": "Min P",
      "minPDesc": "Soglia minima di probabilità.",
      "typicalP": "Typical P",
      "typicalPDesc": "Soglia campionamento tipico.",
      "seed": "Seed",
      "seedDesc": "Seed casuale. Lascia vuoto per casuale.",
      "ropeBase": "RoPE Base",
      "ropeBaseDesc": "Override base frequenza.",
      "ropeScale": "RoPE Scale",
      "ropeScaleDesc": "Override scala frequenza.",
      "kvCacheType": "KV Cache Type",
      "kvCacheTypeDesc": "Quantizza KV cache per risparmiare VRAM.",
      "offloadKqv": "Offload KQV",
      "offloadKqvDesc": "KV cache e operazioni KQV su GPU.",
      "on": "On",
      "off": "Off",
      "samplerProfile": "Profilo campionatore",
      "samplerProfileDesc": "Predefiniti ottimizzati per stabilità o ragionamento.",
      "balanced": "Bilanciato",
      "creative": "Creativo",
      "stable": "Stabile",
      "reasoning": "Ragionamento"
    },
    "systemInfo": {
      "title": "Info di sistema",
      "maxContext": "Contesto massimo",
      "recommended": "Consigliato",
      "availableRam": "RAM disponibile",
      "availableVram": "VRAM disponibile",
      "modelSize": "Dimensione modello"
    }
  },
  "exportMenu": {
    "title": "Formato di esportazione",
    "selectFormat": "Seleziona un formato",
    "uscTitle": "Unified System Card",
    "formats": {
      "uscPrompt": "Esportazione USC portabile per template di prompt.",
      "uscLorebook": "Esportazione USC portabile per lorebook.",
      "uscModel": "Esportazione USC portabile per profili modello.",
      "uscChatTemplate": "Esportazione USC portabile per template di chat.",
      "legacyPromptJson": "Legacy Prompt JSON",
      "legacyPromptJsonDesc": "Formato attuale pacchetto prompt esterno.",
      "lorebookJson": "Lorebook JSON",
      "lorebookJsonDesc": "Formato attuale esportazione lorebook.",
      "modelJson": "Model JSON",
      "modelJsonDesc": "JSON profilo modello sicuro senza credenziali.",
      "chatTemplateJson": "Chat Template JSON",
      "chatTemplateJsonDesc": "Formato nativo esportazione template di chat."
    },
    "extra": {
      "selectFormat": "Seleziona un formato",
      "exportFormatTitle": "Formato di esportazione",
      "uscDesc": "Esportazione USC portabile",
      "legacyJsonDesc": "Formato JSON legacy",
      "formatV3Desc": "Esportazione Character Card V3",
      "formatV2Desc": "Esportazione Character Card V2",
      "formatV1Desc": "Esportazione Character Card V1",
      "uscLorebook": "Unified System Card (USC)",
      "portableLorebook": "Esportazione USC portabile per lorebook",
      "lorebookJson": "Lorebook JSON",
      "currentLorebook": "Formato attuale esportazione lorebook",
      "uscModel": "Unified System Card (USC)",
      "portableModel": "Esportazione USC portabile per profili modello",
      "modelJson": "Model JSON",
      "safeModel": "JSON profilo modello sicuro senza credenziali",
      "uscTemplate": "Unified System Card (USC)",
      "portableTemplate": "Esportazione USC portabile per template di chat",
      "templateJson": "Chat Template JSON",
      "nativeTemplate": "Formato nativo esportazione template di chat"
    }
  },
  "designReference": {
    "title": "Riferimenti di design",
    "description": "Carica alcune immagini di riferimento chiare e una descrizione visiva canonica.",
    "descriptionPlaceholder": "Descrivi l'aspetto stabile: viso, capelli, corporatura, età apparente, indizi sull'abbigliamento, accessori e direzione artistica/stile.",
    "addReferences": "Aggiungi riferimenti",
    "visualDescription": "Descrizione visiva",
    "draftWithAi": "Bozza con AI",
    "referenceImages": "Immagini di riferimento",
    "imageAlt": "Riferimento di design {{index}}",
    "loading": "Caricamento...",
    "removeAria": "Rimuovi riferimento di design",
    "noImages": "Nessuna immagine di riferimento ancora",
    "imageCount": "{{count}} immagine/i di riferimento allegate",
    "emptyReferences": "Aggiungi alcuni scatti di riferimento chiari per fissare viso, proporzioni, abbigliamento e stile.",
    "noWriterModel": "Aggiungi prima un modello scene writer compatibile nelle impostazioni Generazione immagini.",
    "noImagesForGeneration": "Aggiungi un avatar o almeno un'immagine di riferimento prima di generare.",
    "writerModelHelp": "Usa {{model}} per creare una bozza dal tuo avatar e dalle immagini di riferimento.",
    "noWriterModelHelp": "Aggiungi un modello scene writer compatibile nelle impostazioni Generazione immagini per la bozza automatica.",
    "draftMenuTitle": "Bozza AI Design",
    "draftMenuDesc": "Bozza creata da {{model}} dall'avatar e dalle immagini di riferimento attuali.",
    "draftMenuNoWriter": "Aggiungi un modello scene writer compatibile prima di usare questo strumento.",
    "regenerate": "Rigenera",
    "useThis": "Usa questo"
  },
  "samplerOrder": {
    "title": "Ordine campionatore",
    "description": "Trascina gli stadi per riordinare. Eseguiti dall'alto verso il basso.",
    "reset": "Reimposta",
    "resetAria": "Reimposta l'ordine del campionatore ai valori predefiniti",
    "stages": {
      "penalties": {
        "label": "Penalità",
        "desc": "Applica penalità di frequenza e presenza prima del filtraggio."
      },
      "grammar": {
        "label": "Grammatica",
        "desc": "Vincola i token quando è attiva la grammatica del template di chat nativo."
      },
      "topK": {
        "label": "Top K",
        "desc": "Riduce il pool di candidati ai token più forti."
      },
      "topP": {
        "label": "Top P",
        "desc": "Applica il filtraggio nucleus alla distribuzione rimanente."
      },
      "minP": {
        "label": "Min P",
        "desc": "Elimina i token di coda a bassa probabilità usando una soglia minima."
      },
      "typical": {
        "label": "Typical P",
        "desc": "Preferisce token statisticamente tipici nella distribuzione corrente."
      },
      "temp": {
        "label": "Temperature",
        "desc": "Appiattisce o acuisce la distribuzione finale prima della selezione."
      }
    },
    "presets": {
      "default": {
        "label": "Predefinito",
        "hint": "Lettuce local"
      },
      "unsloth": {
        "label": "Unsloth",
        "hint": "llama.cpp style"
      },
      "focused": {
        "label": "Focalizzato",
        "hint": "Potatura stretta"
      },
      "creative": {
        "label": "Creativo",
        "hint": "Filtro tardivo"
      }
    }
  },
  "lorebookGen": {
    "flow": {
      "pickerCharactersTitle": "Personaggi",
      "pickerSessionsTitle": "Sessioni",
      "noCharacters": "Nessun personaggio",
      "noSessions": "Nessuna sessione",
      "clearSelection": "Cancella selezione",
      "directionTitle": "Direzione di generazione opzionale",
      "directionLabel": "Direzione",
      "toggleForceMode": "Attiva/disattiva modalita forzata",
      "entryTitlePlaceholder": "Titolo voce",
      "entryContentPlaceholder": "Contenuto voce lorebook",
      "editDirectionBeforeRegenerate": "Modifica direzione prima di rigenerare",
      "generatorReturnedNoDraft": "Il generatore non ha restituito bozze",
      "pageTitle": "Genera voce lorebook",
      "missingContext": "Contesto lorebook mancante per la pagina del generatore.",
      "characterLocked": "Il personaggio e' bloccato al proprietario del lorebook",
      "chooseSession": "Scegli sessione",
      "pickCharacter": "Scegli personaggio",
      "searchMemories": "Cerca ricordi",
      "searchMessages": "Cerca messaggi",
      "selectLastN": "Seleziona ultimi {{n}} messaggi",
      "selectAll": "Seleziona tutto",
      "loadSessionPrompt": "Seleziona una sessione da caricare",
      "messagesText": "messaggi",
      "memoriesText": "ricordi",
      "messagesAndMemories": "messaggi e ricordi",
      "titleAndContentRequired": "Titolo e contenuto sono obbligatori",
      "keywordsOrAlwaysActive": "Aggiungi almeno una parola chiave o abilita Sempre attivo",
      "lorybookEntrySaved": "Voce lorebook salvata",
      "saveFailed": "Salvataggio fallito",
      "generationFailed": "Generazione fallita",
      "failedToLoadContext": "Caricamento generatore lorebook fallito",
      "failedToLoadSessions": "Caricamento sessioni fallito",
      "failedToLoadMessages": "Caricamento messaggi fallito",
      "userRole": "UTENTE",
      "aiRole": "AI"
    },
    "triggerPreview": {
      "resizeInspector": "Ridimensiona inspector"
    }
  },
  "companion": {
    "download": {
      "emotionClassifier": "Classificatore emozioni",
      "emotionClassifierDesc": "Legge i turni e aggiorna i vettori di emozione provata, espressa e bloccata del companion.",
      "emotionSize": "~120 MB",
      "entityExtractor": "Estrattore entita (NER)",
      "entityExtractorDesc": "Identifica persone, luoghi e oggetti in modo che i ricordi possano essere canonicalizzati e collegati.",
      "entitySize": "~140 MB",
      "memoryRouter": "Router memoria",
      "memoryRouterDesc": "Decide se i nuovi turni devono essere memorizzati come categoria di memoria relazionale, traguardo, episodica o altro.",
      "routerSize": "~70 MB",
      "unknownModel": "Modello companion sconosciuto. Fornire ?kind=emotion|ner|router."
    }
  },
  "voices": {
    "extra": {
      "customVoices": "Le mie voci",
      "providerVoices": "Voci provider",
      "myVoices": "Le mie voci",
      "page": {
        "noAudioProvidersHint": "Aggiungine uno in Fornitori > Audio per iniziare",
        "noVoicesTitle": "Nessuna voce creata",
        "noVoicesDescription": "Crea voci con prompt personalizzati per i tuoi personaggi",
        "addProviderFirst": "Aggiungi prima un fornitore audio",
        "noPrompt": "Nessun prompt",
        "noProviderVoices": "Nessuna voce trovata. Fai clic su Aggiorna per recuperare le voci.",
        "showLess": "Mostra meno",
        "showAllVoices": "Mostra tutte le {{count}} voci",
        "voiceFallbackTitle": "Voce"
      },
      "cache": {
        "section": "Cache audio",
        "title": "Cache audio TTS",
        "description": "L'audio vocale generato viene memorizzato nella cache per ridurre le rigenerazioni",
        "clearing": "Pulizia in corso...",
        "clear": "Svuota cache"
      },
      "menu": {
        "editDescription": "Modifica questa voce",
        "deleteDescription": "Rimuovi questa voce",
        "provider": "Fornitore",
        "category": "Categoria",
        "createVoiceConfig": "Crea configurazione voce",
        "createVoiceConfigDescription": "Usa questa voce con impostazioni personalizzate"
      },
      "editor": {
        "editTitle": "Modifica voce",
        "createTitle": "Crea voce",
        "voiceName": "Nome voce",
        "voiceNamePlaceholder": "Voce del mio personaggio",
        "provider": "Fornitore",
        "model": "Modello",
        "modelIdPlaceholder": "gpt-4o-mini-tts",
        "modelIdHint": "Inserisci l'ID esatto del modello atteso dal tuo endpoint compatibile",
        "elevenlabsVoice": "Voce ElevenLabs",
        "noVoicesAvailable": "Nessuna voce disponibile",
        "selectVoice": "Seleziona una voce...",
        "elevenlabsVoiceHint": "Seleziona dalle tue voci ElevenLabs",
        "geminiVoice": "Voce Gemini",
        "geminiVoiceHint": "Seleziona una voce Gemini TTS",
        "voiceId": "ID voce",
        "voiceIdPlaceholder": "alloy",
        "voiceIdHint": "Inserisci l'ID voce supportato dal tuo endpoint compatibile",
        "voicePrompt": "Prompt voce",
        "voicePromptPlaceholder": "Una voce calda e amichevole con un tono allegro...",
        "voicePromptHint": "Descrivi come dovrebbe suonare la voce",
        "exampleText": "Testo di esempio",
        "exampleTextPlaceholder": "Ciao! Ecco come suono quando parlo...",
        "exampleTextHint": "Testo campione per testare la voce",
        "voiceDesignChars": "{{current}}/{{minimum}} caratteri richiesti per l'anteprima del design vocale",
        "defaultSample": "Ciao! Ecco come suono quando parlo. Posso leggere brani piu' lunghi con calore, chiarezza ed emozione, cosi' puoi giudicare il mio tono e il mio ritmo.",
        "playing": "Riproduzione...",
        "previewVoice": "Anteprima voce"
      },
      "kokoro": {
        "title": "Kokoro Studio",
        "newBlend": "Nuova miscela",
        "editBlend": "Modifica miscela",
        "tryText": "Ciao! Questo e' un rapido test di come suono.",
        "experimentDefaultText": "Ciao! Ecco come suono quando parlo. Posso leggere brani piu' lunghi con calore, chiarezza ed emozione.",
        "livePreview": "Anteprima dal vivo",
        "savedBlend": "Miscela salvata",
        "defaultPreviewText": "Ciao! Questa e' una rapida anteprima di come suona questa voce.",
        "experiment": "Esperimento",
        "providerNotFound": "Fornitore Kokoro non trovato",
        "backToProviders": "Torna ai fornitori",
        "variantUnset": "Variante non impostata",
        "ready": "Pronto",
        "modelNotInstalled": "Modello non installato",
        "voiceCount": "{{count}} voce",
        "engineActions": "Azioni motore",
        "engineNotInstalled": "Motore non installato",
        "installAtLeastOneVoice": "Installa almeno una voce",
        "continueSetup": "Continua la configurazione per installare il modello Kokoro.",
        "pickVoiceOrStarter": "Scegli una voce o prendi il pacchetto iniziale per iniziare.",
        "downloadsFailed": "{{count}} download non riuscito",
        "retryOrDismissAll": "Riprova singolarmente o ignora tutti.",
        "dismissAll": "Ignora tutti",
        "model": "Modello",
        "voice": "Voce",
        "downloads": "Download",
        "cancelAll": "Annulla tutto",
        "experimentPlaceholder": "Scrivi una frase per sentirla pronunciata...",
        "speed": "Velocita'",
        "speak": "Parla",
        "yourBlends": "Le tue miscele",
        "noSavedBlends": "Nessuna miscela salvata.",
        "installModelAndVoiceFirst": "Installa prima il modello e una voce.",
        "featured": "In evidenza",
        "stop": "Ferma",
        "sample": "Campione",
        "voiceLibrary": "Libreria voci",
        "starterPack": "Pacchetto iniziale",
        "select": "Seleziona",
        "all": "Tutti",
        "installed": "Installato",
        "installModelToBrowse": "Installa il modello per sfogliare le voci.",
        "noVoicesInCatalog": "Nessuna voce nel catalogo. Tocca Aggiorna.",
        "noVoicesMatch": "Nessuna voce corrisponde ai tuoi filtri.",
        "collapseAll": "Comprimi tutto",
        "expandAll": "Espandi tutto",
        "selectedCount": "{{count}} selezionato",
        "engineTitle": "Motore Kokoro",
        "variant": "Variante",
        "status": "Stato",
        "notInstalled": "Non installato",
        "file": "File",
        "modelSize": "Dimensione modello",
        "voicesSize": "Dimensione voci",
        "total": "Totale",
        "assetRoot": "Cartella risorse",
        "reinstallModel": "Reinstalla modello",
        "installModel": "Installa modello",
        "deleteModel": "Elimina modello",
        "deleteModelDescription": "Libera spazio su disco; le voci vengono mantenute.",
        "blend": "Miscela",
        "previewDescription": "Ascolto rapido con testo predefinito",
        "editBlendDescription": "Regola voci, pesi e velocita'",
        "deleteBlendDescription": "Rimuovi questa voce salvata",
        "setupTitle": "Configura Kokoro",
        "allSet": "Tutto pronto",
        "allSetDescription": "Sfoglia le voci, progetta miscele o testa nell'area esperimento."
      },
      "badge": {
        "gemini": "G",
        "openai": "API",
        "elevenlabs": "11"
      }
    }
  },
  "editPrompt": {
    "extra": {
      "conditionalsSection": "Condizionali",
      "injectionPoints": "Punti di iniezione"
    }
  },
  "embeddings": {
    "download": {
      "bestForQuick": "Migliore per risposte rapide",
      "balancedPerf": "Prestazioni bilanciate",
      "maxContext": "Contesto massimo",
      "capacity1k": "1K token",
      "capacity2k": "2K token",
      "capacity4k": "4K token",
      "approxSize": "~138 MB",
      "downloadVersion": "V4"
    },
    "test": {
      "health": "Controllo salute",
      "retrieval": "Recupero",
      "separation": "Separazione",
      "passed": "Superato",
      "failed": "Fallito",
      "testing": "Test in corso..."
    }
  },
  "convert": {
    "extra": {
      "uec": "Unified Entity Card (UEC)",
      "charaCardV3": "Character Card V3",
      "charaCardV2": "Character Card V2"
    }
  },
  "companionSoulWriter": {
    "extra": {
      "json": "JSON",
      "jsonDesc": "Output strutturato compatto quando il tool calling non e' disponibile.",
      "xml": "XML",
      "xmlDesc": "Da usare quando il modello formatta XML piu affidabilmente del JSON.",
      "fallbackFormat": "Formato di fallback"
    }
  },
  "installedModels": {
    "title": "Local GGUF inventory",
    "fileCount": "{{count}} files",
    "searchPlaceholder": "Search model name, filename, path, quantization, or architecture",
    "loading": "Scanning installed models...",
    "loadFailed": "Failed to load installed models: {{error}}",
    "empty": {
      "title": "No installed GGUF models found",
      "description": "Download models from the browser first, or place `.gguf` files inside the models folder."
    },
    "columns": {
      "type": "Type",
      "params": "Params",
      "arch": "Arch",
      "context": "Context",
      "size": "Size",
      "action": "Action"
    },
    "confirm": {
      "deleteTitle": "Delete model file",
      "deleteMessage": "Delete {{filename}}? This only removes the local GGUF file from the models folder."
    },
    "toasts": {
      "pathCopied": "Path copied",
      "copyFailed": "Copy failed",
      "modelDeleted": "Model deleted",
      "deleteFailed": "Delete failed"
    }
  },
  "editModel": {
    "setup": {
      "title": "Model setup",
      "description": "Choose the platform, give this entry a readable name, and connect it to the model identifier or file you want to use.",
      "selectPlatform": "Select platform"
    },
    "errors": {
      "loadFailed": "Failed to load model settings"
    },
    "runtime": {
      "lastReport": "Last runtime report",
      "gpuFallbackReason": "GPU fallback reason",
      "finalError": "Final error",
      "workingRecoveryConfig": "Working recovery config",
      "context": "Context",
      "batch": "Batch",
      "na": "n/a",
      "applyWorkingConfig": "Apply working config",
      "badges": {
        "succeeded": "Run succeeded",
        "cpuFallbackSucceeded": "CPU fallback recovered",
        "cpuFallbackFailed": "CPU fallback failed",
        "failed": "Run failed"
      },
      "headline": {
        "succeeded": "The last local run completed successfully.",
        "cpuFallbackSucceeded": "GPU load failed, then the model recovered on CPU.",
        "cpuFallbackFailed": "GPU load failed and the CPU fallback still did not complete.",
        "failed": "The last local run failed before llama.cpp could complete."
      },
      "detail": {
        "succeeded": "This report also seeds the smart offload cache so future runs can reuse the last stable GPU setup.",
        "cpuFallbackSucceeded": "We stored the CPU-safe context and batch that did run so you can reuse them.",
        "cpuFallbackFailed": "The model was retried on CPU, but the recovered configuration still failed.",
        "failed": "This report keeps the last known runtime state so you can inspect what happened."
      }
    },
    "sections": {
      "generation": "Generation Parameters",
      "runtime": "Runtime",
      "reasoning": "Reasoning",
      "reasoningThinking": "Reasoning (Thinking)",
      "promptCaching": "Prompt Caching",
      "capabilities": "Capabilities"
    },
    "sectionDescriptions": {
      "generation": "Temperature, sampling, penalties, and output limits.",
      "generationAutomatic1111": "Sampler, CFG, seed, negative prompt, denoise, and size defaults.",
      "runtime": "{{runtime}} execution, memory, and hardware controls.",
      "reasoning": "Thinking mode, effort, and token budget controls.",
      "promptCaching": "Context reuse and prefix caching behavior.",
      "capabilities": "Input and output modalities this model supports.",
      "capabilitiesSimple": "Mark which modalities this model accepts and what it can produce."
    },
    "summaries": {
      "generationAutomatic1111": "Stable Diffusion sampler, CFG, seed, and size defaults",
      "generationDefault": "Default sampling and output limits",
      "runtimeLlama": "Execution, memory, and hardware defaults",
      "runtimeOllama": "Ollama runtime defaults",
      "reasoningAlwaysEnabled": "Always enabled for this provider",
      "reasoningDisabled": "Reasoning disabled",
      "reasoningDefault": "Thinking controls stay on provider defaults",
      "textOnly": "Text only",
      "capabilities": "Input: {{input}} • Output: {{output}}"
    },
    "reasoning": {
      "enabled": "Enabled",
      "enabledDescription": "Show thinking process",
      "effort": "Reasoning Effort",
      "helpLabel": "Help with reasoning mode",
      "budgetTokens": "Budget Tokens",
      "providerDefault": "Provider default"
    },
    "runtimeFacts": {
      "updated": "Updated",
      "modelPath": "Model path",
      "backendUsed": "Backend used",
      "failureStage": "Failure stage",
      "requestedContext": "Requested context",
      "recommendedContext": "Recommended context",
      "initialContext": "Initial context",
      "actualContext": "Actual context",
      "requestedGpuLayers": "Requested GPU layers",
      "actualGpuLayers": "Actual GPU layers",
      "requestedBatch": "Requested batch",
      "initialBatch": "Initial batch",
      "actualBatch": "Actual batch",
      "smartOffloadFallback": "Smart offload fallback",
      "active": "Active",
      "notNeeded": "Not needed",
      "kqvFallback": "KQV fallback",
      "movedToRam": "Moved to RAM",
      "smartOffloadEstimate": "Smart offload estimate",
      "smartOffloadCandidates": "Smart offload candidates",
      "kvCache": "KV cache",
      "kqvOffload": "KQV offload",
      "flashAttention": "Flash attention",
      "gpuBackends": "GPU backends",
      "availableRam": "Available RAM",
      "availableVram": "Available VRAM",
      "modelSize": "Model size",
      "promptTokens": "Prompt tokens",
      "promptPositions": "Prompt positions",
      "targetNewTokens": "Target new tokens",
      "completionTokens": "Completion tokens",
      "finishReason": "Finish reason",
      "firstToken": "First token",
      "throughput": "Throughput",
      "promptTemplate": "Prompt template"
    },
    "fields": {
      "platform": "Platform",
      "displayName": "Display name",
      "modelPath": "Model Path (GGUF)",
      "modelId": "Model ID"
    },
    "placeholders": {
      "displayName": "e.g. My Favorite ChatGPT",
      "modelPath": "/path/to/model.gguf",
      "modelId": "e.g. gpt-4o",
      "sdSteps": "28",
      "sdCfgScale": "6.5",
      "sdSize": "1024x1024",
      "sdSampler": "DPM++ 2M Karras",
      "random": "Random",
      "sdDenoise": "0.75",
      "sdNegativePrompt": "blurry, low quality, bad anatomy, extra fingers",
      "temperature": "0.70",
      "topP": "1.00",
      "zero": "0.00",
      "batch512": "512",
      "default": "Default",
      "mmprojPath": "/path/to/mmproj.gguf",
      "stopSequences": "e.g. \n\n###\nUser:\n"
    },
    "localLibrary": {
      "localPathHelp": "Use the full file path to a local GGUF model.",
      "mmprojTitle": "Downloaded MMProj Files",
      "mmprojEmpty": "No downloaded mmproj files yet",
      "mmprojEmptyHint": "Download a multimodal projector from the Model Browser, or enter a path manually."
    },
    "modelSource": {
      "useCatalog": "Use catalog",
      "enterManually": "Enter manually",
      "refreshModelList": "Refresh model list",
      "onlyFreeModels": "only free models",
      "customEndpointFetchDisabled": "Model fetching is disabled for this custom endpoint. Enable it in Provider settings and set a Models Endpoint if you want model list discovery."
    },
    "promptCaching": {
      "automatic": {
        "title": "Automatic Provider Caching",
        "description": "This provider handles prompt caching automatically on the upstream API. There is no app-side toggle to send."
      },
      "enableTitle": "Enable Context Caching",
      "enableDescription": "Preserve static system prompts and document context across interactions.",
      "ttlTitle": "Cache TTL",
      "ttlDescription": "How long cached prefixes remain valid between requests.",
      "pricingTitle": "Note on pricing:",
      "pricingDescription": "While caching reduces the cost of repeated input tokens, the initial write to the cache may incur a slight premium depending on the selected provider.",
      "oneHourNote": "Extended 1-hour TTL may not be honoured by all providers. Falls back to the provider's default cache lifetime when unsupported.",
      "openai24hNote": "OpenAI uses `in_memory` and `24h` retention policies rather than a 1-hour TTL.",
      "groqLabel": "Groq:",
      "groqDescription": "caching is automatic on supported models only. This app does not force or tune it per request.",
      "geminiLabel": "Gemini:",
      "geminiDescription": "implicit caching is automatic for supported models. Explicit cached content resources are not created by this app yet.",
      "ttl": {
        "inMemory": "In-memory",
        "24h": "24 hr",
        "5min": "5 min",
        "1h": "1 hr"
      }
    },
    "toasts": {
      "runtimeConfigApplied": "Runtime config applied",
      "runtimeConfigAppliedDescription": "Future local runs will reuse the last CPU-safe context and batch.",
      "modelPathRequired": "Model path required",
      "modelPathRequiredDescription": "Select a GGUF model path before reading the embedded template.",
      "embeddedTemplatePasted": "Embedded template pasted into editor"
    },
    "search": {
      "didYouMean": "Did you mean:"
    },
    "capabilities": {
      "helpLabel": "Help with capabilities",
      "input": "Input",
      "output": "Output",
      "automatic1111Fixed": "AUTOMATIC1111 models are fixed to text + image input and image output."
    },
    "runtimeSummary": {
      "ram": "RAM",
      "vram": "VRAM",
      "kvCacheInVram": "KV cache in VRAM",
      "kvCacheInRam": "KV cache in RAM"
    },
    "help": {
      "temperature": "Help with temperature",
      "topP": "Help with top p",
      "maxOutputTokens": "Help with max output tokens",
      "topK": "Help with top k",
      "frequencyPenalty": "Help with frequency penalty",
      "presencePenalty": "Help with presence penalty",
      "contextLength": "Help with context length"
    },
    "contextInfo": {
      "maxSupported": "Max supported",
      "recommended": "Recommended",
      "availableRam": "Available RAM",
      "availableVram": "Available VRAM",
      "modelSize": "Model size",
      "contextCache": "Context cache",
      "memoryFitness": "Memory Fitness",
      "gpuAcceleration": "GPU Acceleration",
      "kvHeadroom": "KV Headroom",
      "quantQuality": "Quant Quality"
    },
    "llama": {
      "toggleStrictMode": "Toggle llama strict mode"
    },
    "ollama": {
      "numCtxShort": "Num Ctx"
    },
    "continueSetup": {
      "continue": "Continue Setup",
      "saveToContinue": "Save a model to continue"
    },
    "generation": {
      "automatic1111Help": "AUTOMATIC1111 uses Stable Diffusion controls here. These values become the default sampler settings for avatars, scene images, and other local diffusion requests.",
      "formatWidthHeight": "Format: width x height"
    },
    "generationDescriptions": {
      "sdSteps": "Diffusion sampling steps",
      "sdCfgScale": "Prompt guidance strength",
      "sdSize": "Used when the request does not override size",
      "sdSampler": "Sampler name sent to A1111",
      "sdSeed": "Leave blank for random generations",
      "sdDenoise": "Edit strength for reference-based generations",
      "sdNegativePrompt": "Applied to every AUTOMATIC1111 request for this model",
      "temperature": "Higher = more creative",
      "topP": "Lower = more focused",
      "maxOutputTokens": "Limit response length",
      "topK": "Sample from top K tokens",
      "frequencyPenalty": "Reduce word repetition",
      "presencePenalty": "Encourage new topics"
    },
    "moveModel": {
      "title": "Move Model File"
    },
    "parameterSupport": {
      "title": "Parameter Support"
    },
    "editorMode": {
      "title": "Editor mode",
      "description": "Simple starts collapsed. Advanced keeps the current full editor.",
      "simple": "Simple",
      "advanced": "Advanced",
      "emptyState": "Open a section to adjust its settings. The advanced editor stays available when you need the full form."
    },
    "templateOverride": {
      "title": "Template Override",
      "hideEmbedded": "Hide Embedded",
      "showEmbedded": "Show Embedded",
      "close": "Close",
      "readingEmbedded": "Reading embedded template...",
      "readEmbeddedFailed": "Could not read embedded template",
      "copiedToClipboard": "Copied to clipboard",
      "pasteIntoEditor": "Paste into Editor",
      "jinjaTemplate": "Jinja Template",
      "placeholder": "Enter a Jinja chat template or an internal template name..."
    }
  },
  "usageAnalytics": {
    "page": {
      "filters": "Filtri",
      "model": "Modello",
      "character": "Personaggio",
      "clearAll": "Cancella tutto",
      "applyFilters": "Applica filtri",
      "recentActivity": "Attivita' recente",
      "customRange": "Intervallo personalizzato",
      "startDate": "Data inizio",
      "endDate": "Data fine",
      "applyRange": "Applica intervallo",
      "dashboard": "DASHBOARD",
      "appTime": "TEMPO APP",
      "today": "Oggi",
      "last7Days": "7 giorni",
      "last30Days": "30 giorni",
      "all": "Tutti",
      "custom": "Personalizzato",
      "filtersCount": "{{count}} filtri",
      "totalCost": "Costo totale",
      "tokens": "Token",
      "avgShort": "{{value}} media",
      "requests": "Richieste",
      "period": "Periodo",
      "last7d": "Ultimi 7g",
      "last30d": "Ultimi 30g",
      "allTime": "Sempre",
      "recordsCount": "{{count}} record",
      "usageTrend": "Tendenza utilizzo",
      "tokenConsumptionOverTime": "Consumo token nel tempo",
      "input": "Input",
      "output": "Output",
      "byModel": "Per modello",
      "byCharacter": "Per personaggio",
      "top": "Migliori",
      "active": "Attivo",
      "quickView": "Vista rapida",
      "viewAll": "Vedi tutti",
      "startChatting": "Inizia a chattare per vedere i dati di utilizzo",
      "exportedTo": "Esportato in: {{path}}",
      "periodTotal": "Totale periodo",
      "daysActive": "{{count}} giorni attivi",
      "dailyAvg": "Media giornaliera",
      "selectedPeriod": "periodo selezionato",
      "yesterdayValue": "Ieri {{value}}",
      "thirtyDayAvg": "Media 30 giorni",
      "appTimeTrend": "Tendenza tempo app",
      "usageDurationPerDay": "Durata utilizzo al giorno",
      "totalValue": "Totale {{value}}",
      "activeTime": "Tempo attivo"
    },
    "activity": {
      "loading": "Caricamento attivita'...",
      "title": "Attivita' recente",
      "recordsCount": "{{count}} record di utilizzo",
      "rangeOfTotal": "{{start}}-{{end}} di {{total}}",
      "previous": "Precedente",
      "next": "Successivo",
      "pageOf": "Pagina {{page}} di {{total}}"
    },
    "shared": {
      "relativeTime": {
        "justNow": "adesso",
        "minutesAgo": "{{count}}m fa",
        "hoursAgo": "{{count}}h fa",
        "daysAgo": "{{count}}g fa"
      },
      "operations": {
        "chat": "Chat",
        "regenerate": "Rigenera",
        "continue": "Continua",
        "summary": "Riepilogo",
        "memoryManager": "Memoria",
        "imageGeneration": "Genera immagine",
        "aiCreator": "AI Creator",
        "replyHelper": "Assistente risposta",
        "groupChatMessage": "Chat di gruppo",
        "groupChatRegenerate": "Rigenera gruppo",
        "groupChatContinue": "Continua gruppo",
        "groupChatDecisionMaker": "Decisore"
      },
      "outputImages": "{{count}} immagini",
      "tokens": "{{count}} token",
      "unknown": "Sconosciuto",
      "requestDetails": "Dettagli richiesta",
      "noStopReason": "Nessun motivo di stop",
      "tokenUsage": "Utilizzo token",
      "estimatedCost": "Costo stimato",
      "stats": {
        "prompt": "Prompt",
        "completion": "Completamento",
        "total": "Totale",
        "reasoning": "Ragionamento",
        "image": "Immagine",
        "memory": "Memoria",
        "summary": "Riepilogo",
        "inputImages": "Immagini input",
        "outputImages": "Immagini output",
        "cachedPrompt": "Prompt in cache",
        "cacheWrite": "Scrittura cache",
        "webSearches": "Ricerche web",
        "cacheRead": "Lettura cache",
        "requestFee": "Costo richiesta",
        "webSearch": "Ricerca web",
        "providerTotal": "Totale fornitore"
      }
    }
  }
};
