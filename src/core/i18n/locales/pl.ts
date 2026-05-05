import type { DeepPartialMessageTree } from "../types";
import { type LocaleMessages } from "./en";
import type { LocaleMetadata } from ".";

export const plMetadata: LocaleMetadata = {
  name: "Polish",
  label: "Polski",
};

export const plMessages: DeepPartialMessageTree<LocaleMessages> = {
  "common": {
    "nav": {
      "chats": "Czaty",
      "settings": "Ustawienia",
      "providers": "Dostawcy",
      "responseStyle": "Styl odpowiedzi",
      "models": "Modele",
      "security": "Bezpieczeństwo",
      "accessibility": "Dostępność",
      "reset": "Resetuj",
      "backupRestore": "Kopia zapasowa i przywracanie",
      "convertFiles": "Konwertuj pliki",
      "usageAnalytics": "Analityka użycia",
      "changelog": "Lista zmian",
      "about": "Informacje",
      "createSystemPrompt": "Utwórz prompt systemowy",
      "editSystemPrompt": "Edytuj prompt systemowy",
      "systemPrompts": "Prompty systemowe",
      "developer": "Deweloper",
      "advanced": "Zaawansowane",
      "characters": "Postacie",
      "lorebooks": "Lorebooki",
      "personas": "Persony",
      "dynamicMemory": "Pamięć dynamiczna",
      "creationHelper": "Asystent tworzenia",
      "helpMeReply": "Pomóż mi odpowiedzieć",
      "editPersona": "Edytuj personę",
      "newTemplate": "Nowy szablon",
      "editTemplate": "Edytuj szablon",
      "chatTemplates": "Szablony czatu",
      "editCharacter": "Edytuj postać",
      "sync": "Synchronizacja",
      "newCharacter": "Nowa postać",
      "engineSetup": "Konfiguracja silnika",
      "llmProviders": "Dostawcy LLM",
      "engineSettings": "Ustawienia silnika",
      "lettuceEngine": "Silnik Lettuce",
      "create": "Utwórz",
      "setup": "Konfiguracja",
      "welcome": "Witamy",
      "conversation": "Rozmowa",
      "library": "Biblioteka",
      "groupChats": "Czaty grupowe",
      "groupChat": "Czat grupowy",
      "imageGeneration": "Generowanie obrazów",
      "voices": "Głosy",
      "chatAppearance": "Wygląd czatu",
      "colorCustomization": "Niestandardowe kolory",
      "embeddingDownload": "Pobieranie embeddingu",
      "embeddingTest": "Test embeddingu",
      "editModel": "Edytuj model",
      "messageDebug": "Debugowanie wiadomości",
      "speechRecognition": "Speech Recognition",
      "hostApi": "API Server",
      "lorebookEntryGenerator": "Lorebook Entry Generator",
      "companionSoulWriter": "Twórca Companion Soul"
    },
    "bottomNav": {
      "chats": "Czaty",
      "groups": "Grupy",
      "create": "Utwórz",
      "discover": "Odkrywaj",
      "library": "Biblioteka"
    },
    "buttons": {
      "cancel": "Anuluj",
      "confirm": "Potwierdź",
      "save": "Zapisz",
      "saving": "Zapisywanie...",
      "delete": "Usuń",
      "deleting": "Usuwanie...",
      "create": "Utwórz",
      "creating": "Tworzenie...",
      "edit": "Edytuj",
      "back": "Wstecz",
      "done": "Gotowe",
      "download": "Pobierz",
      "later": "Później",
      "proceed": "Kontynuuj",
      "retry": "Ponów",
      "discard": "Odrzuć",
      "import": "Importuj",
      "importing": "Importowanie...",
      "export": "Eksportuj",
      "exporting": "Eksportowanie...",
      "update": "Aktualizuj",
      "generate": "Generuj",
      "refresh": "Odśwież",
      "continue": "Kontynuuj",
      "goBack": "Wróć",
      "search": "Szukaj",
      "clearSearch": "Wyczyść wyszukiwanie",
      "add": "Dodaj",
      "remove": "Usuń",
      "rename": "Zmień nazwę",
      "copy": "Kopiuj",
      "copied": "Skopiowano!",
      "browseFiles": "Przeglądaj pliki",
      "install": "Zainstaluj"
    },
    "labels": {
      "processing": "Przetwarzanie...",
      "loading": "Ładowanie...",
      "noDescriptionYet": "Brak opisu",
      "untitled": "Bez tytułu",
      "default": "Domyślny",
      "enabled": "Włączony",
      "disabled": "Wyłączony",
      "on": "Wł.",
      "off": "Wył.",
      "none": "Brak",
      "auto": "Auto",
      "custom": "Niestandardowy",
      "name": "Nazwa",
      "description": "Opis",
      "content": "Treść",
      "preview": "Podgląd",
      "options": "Opcje"
    },
    "time": {
      "justNow": "Właśnie teraz",
      "minutesAgo": "{{minutes}} min temu",
      "hoursAgo": "{{hours}} godz. temu",
      "daysAgo": "{{days}} dni temu",
      "today": "Dzisiaj",
      "yesterday": "Wczoraj",
      "last7Days": "Ostatnie 7 dni",
      "older": "Starsze"
    }
  },
  "settings": {
    "sections": {
      "intelligence": "Inteligencja",
      "experience": "Doświadczenie",
      "connectivity": "Łączność",
      "securityPrivacy": "Bezpieczeństwo i prywatność",
      "supportInfo": "Wsparcie i informacje",
      "dangerZone": "Strefa zagrożenia",
      "developer": "Deweloper"
    },
    "items": {
      "providers": {
        "title": "Dostawcy",
        "subtitle": "Połącz z usługami AI"
      },
      "models": {
        "title": "Modele",
        "subtitle": "Konfiguruj modele AI"
      },
      "imageGeneration": {
        "title": "Generowanie obrazu",
        "subtitle": "Generuj i testuj obrazy"
      },
      "voices": {
        "title": "Głosy",
        "subtitle": "Głosy tekst-na-mowę"
      },
      "accessibility": {
        "title": "Dostępność",
        "subtitle": "Dźwięki i haptyka"
      },
      "prompts": {
        "title": "Prompty systemowe",
        "subtitle": "Kształtuj osobowość AI"
      },
      "security": {
        "title": "Bezpieczeństwo",
        "subtitle": "Szyfrowanie i prywatność"
      },
      "backup": {
        "title": "Kopia zapasowa i przywracanie",
        "subtitle": "Eksportuj lub importuj dane"
      },
      "convert": {
        "title": "Konwertuj pliki",
        "subtitle": "Migruj starsze pliki .json do .uec"
      },
      "sync": {
        "title": "Synchronizacja lokalna",
        "subtitle": "Synchronizuj między urządzeniami"
      },
      "usage": {
        "title": "Analityka użycia",
        "subtitle": "Koszty i statystyki tokenów"
      },
      "advanced": {
        "title": "Zaawansowane",
        "subtitle": "Pamięć i funkcje"
      },
      "logs": {
        "title": "Logi",
        "subtitle": "Debugowanie i diagnostyka"
      },
      "guide": {
        "title": "Przewodnik konfiguracji",
        "subtitle": "Uruchom ponownie wprowadzenie"
      },
      "docs": {
        "title": "Dokumentacja",
        "subtitle": "Poradniki i referencje"
      },
      "github": {
        "title": "Zgłoś problemy",
        "subtitle": "Błędy i opinie • v{{version}}"
      },
      "discord": {
        "title": "Dołącz do Discord",
        "subtitle": "Społeczność i pomoc"
      },
      "about": {
        "title": "Informacje",
        "subtitle": "Wersja, aktualizacje i linki"
      },
      "changelog": {
        "title": "Lista zmian",
        "subtitle": "Co nowego"
      },
      "reset": {
        "title": "Resetuj",
        "subtitle": "Wymaż wszystko"
      },
      "developer": {
        "title": "Narzędzia deweloperskie",
        "subtitle": "Debugowanie i testowanie"
      }
    }
  },
  "accessibility": {
    "sectionTitles": {
      "language": "Język",
      "sounds": "Dźwięki",
      "haptics": "Haptyka",
      "appearance": "Wygląd"
    },
    "language": {
      "appLanguage": "Język aplikacji",
      "description": "Wybierz język używany w nawigacji i ustawieniach."
    },
    "appearance": {
      "customColors": "Niestandardowe kolory",
      "customColorsDesc": "Spersonalizuj schemat kolorów aplikacji",
      "chatAppearance": "Wygląd czatu",
      "chatAppearanceDesc": "Dostosuj bąbelki wiadomości, czcionki i układ"
    },
    "haptics": {
      "vibrateOnChat": "Wibracje podczas czatu",
      "vibrateDesc": "Krótkie impulsy wibracji podczas pisania przez asystenta",
      "intensity": "Intensywność",
      "light": "Lekka",
      "medium": "Średnia",
      "heavy": "Silna",
      "soft": "Miękka",
      "rigid": "Sztywna"
    },
    "sounds": {
      "send": "Wysyłanie",
      "sendDescription": "Odtwarzany przy wysyłaniu wiadomości",
      "success": "Sukces",
      "successDescription": "Odtwarzany po pomyślnym zakończeniu przez asystenta",
      "failure": "Błąd",
      "failureDescription": "Odtwarzany przy błędzie lub przerwaniu",
      "testButton": "Testuj"
    },
    "feedbackInfo": "Informacje zwrotne pomagają zauważyć, kiedy wiadomości są wysyłane lub odbierane.",
    "hapticsInfo": "Haptyka jest dostępna na urządzeniach mobilnych.",
    "extra": {
      "easterEggs": "Easter Eggs",
      "beetrootRain": "Beetroot Rain",
      "beetrootDesc": "Beetroots fall when chats mention them"
    }
  },
  "topNav": {
    "unsavedChangesTitle": "Niezapisane zmiany",
    "unsavedChangesMessage": "Zapisz lub odrzuć zmiany przed wyjściem.",
    "goBack": "Wróć",
    "changeLayout": "Zmień układ",
    "search": "Szukaj",
    "settings": "Ustawienia",
    "help": "Pomoc",
    "add": "Dodaj",
    "openFilters": "Otwórz filtry",
    "save": "Zapisz",
    "extra": {
      "installedModels": "Installed Models",
      "refresh": "Refresh",
      "minimize": "Minimize",
      "maximize": "Maximize",
      "close": "Close"
    }
  },
  "updates": {
    "available": {
      "title": "Nowa wersja jest dostępna",
      "description": "Dostępna jest wersja v{{latestVersion}}. Używasz v{{currentVersion}}.",
      "actions": {
        "view": "Zobacz wydanie"
      }
    }
  },
  "about": {
    "kicker": "Informacje o aplikacji",
    "appName": "LettuceAI",
    "description": "Szczegóły wersji, sprawdzanie aktualizacji i przydatne linki.",
    "info": {
      "version": "Wersja",
      "channel": "Kanał",
      "platform": "Platforma"
    },
    "buildChannel": {
      "dev": "Wersja deweloperska",
      "release": "Wersja stabilna"
    },
    "update": {
      "sectionTitle": "Aktualizacje",
      "title": "Aktualizacje aplikacji",
      "description": "Sprawdź ręcznie nowe wydania lub wyłącz automatyczne sprawdzanie przy starcie.",
      "autoChecks": "Automatyczne sprawdzanie aktualizacji",
      "autoChecksDescription": "Po włączeniu LettuceAI sprawdza nowe wersje przy uruchomieniu aplikacji.",
      "checkNow": "Sprawdź aktualizacje",
      "checking": "Sprawdzanie aktualizacji...",
      "upToDateTitle": "Masz najnowszą wersję",
      "upToDateDescription": "Obecnie nie ma nowszego wydania dla tego urządzenia.",
      "failedTitle": "Nie udało się sprawdzić aktualizacji",
      "failedDescription": "Nie udało się teraz sprawdzić aktualizacji. Spróbuj ponownie za chwilę."
    },
    "links": {
      "sectionTitle": "Linki",
      "website": "Strona internetowa",
      "websiteDescription": "Strona pobierania i informacje o wydaniach",
      "github": "GitHub",
      "githubDescription": "Kod źródłowy, zgłoszenia i rozwój",
      "discord": "Discord",
      "discordDescription": "Serwer społeczności i pomoc",
      "reddit": "Reddit",
      "redditDescription": "Dyskusje społeczności, opinie i aktualizacje"
    },
    "developerMode": {
      "enable": "Włącz tryb deweloperski",
      "enabled": "Tryb deweloperski włączony"
    },
    "errors": {
      "saveTitle": "Nie udało się zapisać preferencji",
      "saveDescription": "Preferencja sprawdzania aktualizacji nie została zmieniona."
    }
  },
  "components": {
    "tooltip": {
      "dismissHint": "Stuknij gdziekolwiek, aby zamknąć"
    },
    "backgroundPosition": {
      "title": "Pozycja tła",
      "instructions": "Przeciągnij, aby ustawić pozycję • Ściśnij lub przewiń, aby powiększyć"
    },
    "avatarSource": {
      "generateImage": "Generuj obraz",
      "generateImageDesc": "Tworzenie awatara za pomocą AI",
      "noImageModels": "Brak dostępnych modeli obrazów",
      "editCurrent": "Edytuj bieżący",
      "editCurrentDesc": "Zmień pozycję lub przytnij",
      "chooseImage": "Wybierz obraz",
      "chooseImageDesc": "Wybierz z urządzenia"
    },
    "avatarCurrentEdit": {
      "title": "Edytuj bieżący",
      "reposition": "Zmień położenie",
      "repositionDesc": "Przenieś lub przytnij bieżący awatar",
      "editWithAI": "Edytuj za pomocą sztucznej inteligencji",
      "editWithAIDesc": "Otwórz edycję AI dla bieżącego awatara",
      "noImageModels": "Brak dostępnych modeli obrazów"
    },
    "avatarGeneration": {
      "modelsLoadError": "Nie udało się załadować modeli generowania obrazów",
      "title": "Generuj awatar",
      "help": "Pomoc z generowaniem awatara",
      "model": "Model",
      "selectModel": "Wybierz model",
      "describe": "Opisz swój awatar",
      "describePlaceholder": "Przyjazna dziewczyna w stylu anime z kolorowymi włosami, ciepło się uśmiechająca...",
      "inProgress": "Generowanie awatara...",
      "editingInProgress": "Stosowanie edycji awatara...",
      "previousVariant": "Poprzedni wariant",
      "nextVariant": "Następny wariant",
      "variantCounter": "{{current}} / {{total}}",
      "editRequest": "Edytuj żądanie",
      "editRequestPlaceholder": "Przyciemnij włosy, dodaj okulary, zachowaj twarz taką samą...",
      "applyEdit": "Zastosuj Edytuj",
      "editImageLoadError": "Nie udało się przygotować wygenerowanego awatara do edycji",
      "aiAssistant": "Asystent AI",
      "backToResults": "Powrót do monitu",
      "magicInTheWorks": "Magia w działaniu...",
      "refine": "Oczyścić",
      "apply": "Stosować",
      "alt": "Wygenerowany awatar",
      "regenerate": "Regeneruj",
      "useThis": "Użyj tego"
    },
    "avatarPosition": {
      "title": "Pozycja awatara",
      "instructions": "Przeciągnij, aby ustawić pozycję • Ściśnij lub przewiń, aby powiększyć",
      "alt": "Awatar do ustawienia pozycji"
    },
    "confirmDialog": {
      "defaultTitle": "Potwierdź",
      "defaultLabel": "Potwierdź"
    },
    "bottomMenu": {
      "defaultTitle": "Menu",
      "dragTip": "Przeciągnij, aby zamknąć menu",
      "closeLabel": "Zamknij menu",
      "buttonProcessing": "Przetwarzanie..."
    },
    "modelSelector": {
      "placeholder": "Wybierz model",
      "clearLabel": "Użyj globalnego domyślnego",
      "loading": "Ładowanie modeli...",
      "noModels": "Brak dostępnych modeli",
      "addProviderFirst": "Najpierw dodaj dostawcę w ustawieniach",
      "title": "Wybierz model",
      "searchPlaceholder": "Szukaj modeli...",
      "noResults": "Nie znaleziono modeli",
      "noResultsHint": "Spróbuj innego wyszukiwania"
    },
    "localeSelector": {
      "title": "Wybierz Język"
    },
    "promptTemplate": {
      "nameContentRequired": "Nazwa i treść są wymagane",
      "saveError": "Nie udało się zapisać szablonu",
      "editTitle": "Edytuj prompt",
      "createTitle": "Utwórz prompt",
      "name": "Nazwa",
      "namePlaceholder": "np. Mistrz Roleplay",
      "content": "Treść",
      "variablesButton": "Zmienne",
      "contentPlaceholder": "Jesteś pomocnym asystentem AI...\n\nUżyj {{char.name}} i {{scene}} w swoim prompcie.",
      "characterCount": "{{count}} znaków",
      "preview": "Podgląd",
      "characterPlaceholder": "Postać…",
      "personaPlaceholder": "Persona…",
      "rendering": "Renderowanie…",
      "noPreview": "Brak podglądu",
      "saving": "Zapisywanie...",
      "update": "Aktualizuj",
      "create": "Utwórz",
      "variablesTitle": "Zmienne szablonu",
      "variablesCopyHint": "Stuknij, aby skopiować do schowka",
      "variablesCopied": "Skopiowano",
      "variables": {
        "charName": "Imię postaci",
        "charNameDesc": "Imię postaci",
        "charDesc": "Opis postaci",
        "charDescDesc": "Opis postaci",
        "scene": "Scena",
        "sceneDesc": "Scena początkowa/scenariusz",
        "userName": "Imię użytkownika",
        "userNameDesc": "Imię persony użytkownika",
        "userDesc": "Opis użytkownika",
        "userDescDesc": "Opis persony użytkownika",
        "rules": "Zasady",
        "rulesDesc": "Zasady zachowania postaci",
        "contextSummary": "Podsumowanie kontekstu",
        "contextSummaryDesc": "Dynamiczne podsumowanie rozmowy",
        "keyMemories": "Kluczowe wspomnienia",
        "keyMemoriesDesc": "Lista istotnych wspomnień"
      }
    },
    "characterExport": {
      "title": "Format eksportu",
      "selectFormat": "Wybierz format",
      "loading": "Ładowanie formatów...",
      "formatUecDesc": "Format Unified Entity Card (.uec) (zalecany).",
      "formatLegacyJsonDesc": "Starszy JSON (tylko import).",
      "formatV3Desc": "Character Card V3 JSON (najnowsza specyfikacja).",
      "formatV2Desc": "Character Card V2 JSON (specyfikacja Tavern).",
      "formatV1Desc": "Character Card V1 (tylko import)."
    },
    "embeddingDownload": {
      "downloadRequired": "Wymagane pobranie",
      "modelRequired": "Wymagany model embeddingu",
      "description": "Pamięć dynamiczna wymaga pobrania lokalnego modelu embeddingu (~260 MB).",
      "localStorage": "• Model zostanie zapisany lokalnie na urządzeniu",
      "downloadSize": "• Rozmiar pobierania: około 260 MB",
      "summarization": "• Wymagany do podsumowywania rozmów"
    },
    "embeddingUpgrade": {
      "title": "Dostępny model embeddingu v3",
      "v1Message": "Używasz v1 z 512 tokenami. Zaktualizuj do v3, aby uzyskać lepszą jakość pamięci i obsługę długiego kontekstu.",
      "v2Message": "Używasz starszego v2. Zaktualizuj do v3, aby uzyskać lepszą jakość pamięci z najnowszym modelem embeddingu.",
      "button": "Zaktualizuj do v3",
      "v3Message": "v4 is out and dramatically improves roleplay memory recall over v3 (recall@1 0.02 -> 0.92). Upgrading is recommended."
    },
    "v2UpgradeToast": {
      "title": "Model pamięci v3",
      "badge": "Dostępny",
      "message": "Lepsza jakość embeddingu w porównaniu z v2",
      "dismiss": "Odrzuć",
      "upgrade": "Zaktualizuj"
    },
    "v1UpgradeToast": {
      "title": "Dostępny model pamięci v3",
      "message": "Zaktualizuj, aby uzyskać lepszą jakość pamięci i obsługę długiego kontekstu.",
      "dismiss": "Odrzuć",
      "upgrade": "Zaktualizuj"
    },
    "createMenu": {
      "title": "Utwórz nowe",
      "smartCreator": "Inteligentny kreator",
      "smartCreatorDesc": "Pozwól asystentowi poprowadzić tworzenie",
      "divider": "Lub utwórz ręcznie",
      "character": "Postać",
      "characterDesc": "Utwórz niestandardową postać",
      "persona": "Persona",
      "personaDesc": "Utwórz wielokrotnie używalny głos",
      "groupChat": "Czat grupowy",
      "groupChatDesc": "Rozmawiaj z wieloma postaciami",
      "lorebook": "Lorebook",
      "lorebookDesc": "Zbuduj encyklopedię świata",
      "characterSmartDesc": "Zbuduj postać z przewodnikiem tworzenia",
      "personaSmartDesc": "Utwórz wielokrotnie używalny głos lub osobowość",
      "lorebookSmartDesc": "Zbuduj uporządkowaną encyklopedię świata",
      "loadingConversations": "Ładowanie rozmów...",
      "createNew": "Utwórz nowe",
      "createNewDesc": "Rozpocznij nowy czat z przewodnikiem tworzenia",
      "editExisting": "Edytuj istniejące",
      "continueLast": "Kontynuuj ostatnią rozmowę",
      "seeOlder": "Zobacz starsze rozmowy",
      "seeOlderDesc": "Otwórz dowolną poprzednią rozmowę Inteligentnego Kreatora",
      "noConversations": "Brak rozmów dla tego kreatora.",
      "sessionCompleted": "Ukończone",
      "sessionCancelled": "Anulowane",
      "sessionDraft": "Szkic",
      "sessionMessages": "{{count}} wiadomości",
      "untitledConversation": "Rozmowa bez tytułu",
      "nameLorebookTitle": "Nazwij lorebook",
      "lorebookNamePlaceholder": "Wprowadź nazwę lorebooka...",
      "lorebookImporting": "Importowanie...",
      "lorebookImport": "Importuj",
      "lorebookCreating": "Tworzenie...",
      "lorebookCreate": "Utwórz",
      "editExistingDesc": "Wybierz {{goal}} i edytuj go z Inteligentnym Kreatorem",
      "creatorTitle": "Kreator {{goal}}",
      "editTitle": "Edytuj {{goal}}",
      "conversationsTitle": "Rozmowy {{goal}}",
      "loadingItems": "Ładowanie {{items}}...",
      "noItemsFound": "Nie znaleziono {{items}}.",
      "unnamedCharacter": "Postać bez nazwy",
      "untitledPersona": "Persona bez tytułu",
      "untitledLorebook": "Lorebook bez tytułu"
    },
    "advancedModelSettings": {
      "temperature": "Temperatura",
      "temperatureDesc": "Wyższa = bardziej kreatywna",
      "topP": "Top P",
      "topPDesc": "Niższa = bardziej skupiona",
      "maxTokens": "Maks. tokenów wyjściowych",
      "maxTokensDesc": "Zostaw puste dla domyślnych",
      "contextLength": "Długość kontekstu",
      "contextLengthDesc": "Tylko modele lokalne",
      "contextLengthAuto": "Auto",
      "frequencyPenalty": "Kara za częstotliwość",
      "frequencyPenaltyDesc": "Zmniejsz powtarzanie tokenów",
      "presencePenalty": "Kara za obecność",
      "presencePenaltyDesc": "Zachęcaj do nowych tematów",
      "topK": "Top K",
      "topKDesc": "Ogranicz pulę tokenów",
      "reasoning": "Myślenie / Rozumowanie",
      "reasoningAutoDesc": "Ten model zawsze używa rozumowania. Konfiguracja nie jest potrzebna.",
      "reasoningEnableDesc": "Włącz zaawansowane możliwości myślenia do złożonego rozwiązywania problemów i zadań rozumowania.",
      "effortMode": "Tryb wysiłku",
      "budgetMode": "Tryb budżetowy",
      "reasoningEffort": "Wysiłek rozumowania",
      "reasoningEffortDesc": "Kontroluje głębokość myślenia",
      "reasoningEffortAuto": "Auto",
      "reasoningEffortLow": "Niski",
      "reasoningEffortMed": "Średni",
      "reasoningEffortHigh": "Wysoki",
      "reasoningBudget": "Budżet rozumowania (tokeny)",
      "reasoningBudgetDesc": "Maks. tokenów zarezerwowanych na myślenie",
      "reasoningEffortLowDesc": "Szybkie odpowiedzi z minimalnym rozumowaniem",
      "reasoningEffortMediumDesc": "Zrównoważona głębokość rozumowania",
      "reasoningEffortHighDesc": "Maksymalna głębokość rozumowania dla złożonych problemów",
      "reasoningBudgetExtendedDesc": "Maks. tokenów zarezerwowanych na myślenie. Dodawane do limitu wyjścia."
    },
    "providerParameterSupport": {
      "unknownProvider": "Nieznany dostawca: {{providerId}}",
      "reasoningNotSupportedEffort": "Nieobsługiwane — ten dostawca nie używa poziomów wysiłku",
      "reasoningNotSupportedBudgetOnly": "Nieobsługiwane — ten dostawca używa podejścia budżetowego",
      "reasoningNotSupported": "Nieobsługiwane — ten dostawca nie obsługuje rozumowania",
      "unsupportedParametersIgnored": "Nieobsługiwane parametry zostaną zignorowane przez {{providerName}}.",
      "reasoningEffortSupported": "Wysiłek rozumowania jest obsługiwany dla modeli myślących (o1, DeepSeek-R1 itp.)",
      "reasoningBudgetSupported": "Ten dostawca używa myślenia budżetowego (bez poziomów wysiłku). Ustaw tokeny budżetu rozumowania.",
      "reasoningNotSupportedProvider": "Ten dostawca nie obsługuje parametrów rozumowania.",
      "matrixTitle": "Macierz obsługi parametrów dostawców",
      "providerColumn": "Dostawca",
      "supportedIndicator": "Obsługiwany przez API dostawcy",
      "notSupportedIndicator": "Nieobsługiwany (parametr zostanie zignorowany)"
    },
    "v3UpgradeToast": {
      "title": "Memory model v4",
      "badge": "Available",
      "message": "v4 dramatically improves roleplay memory recall over v3 (recall@1 0.02 → 0.92). Upgrading is recommended.",
      "dismiss": "Later",
      "upgrade": "Upgrade"
    },
    "extra": {
      "promptCachingTitle": "Prompt Caching",
      "promptCachingDescription": "Speeds up generation and reduces costs for long, repetitive contexts (like large system prompts or deep chat histories).",
      "avatarAlt": "Avatar",
      "chooseFromLibrary": "Choose from library",
      "chooseFromLibraryDesc": "Use an image already saved in the app",
      "generateFailed": "Failed to generate image",
      "editFailed": "Failed to edit avatar",
      "backgroundAlt": "Background to position",
      "formatsLoadFailed": "Failed to load export formats",
      "formatsShowingDefaults": "(showing defaults)",
      "timeJustNow": "just now",
      "timeMinutesAgo": "{{minutes}}m ago",
      "timeHoursAgo": "{{hours}}h ago",
      "timeDaysAgo": "{{days}}d ago",
      "removeReference": "Remove design reference",
      "thumbLoading": "Loading...",
      "addReferences": "Add references",
      "visualDescription": "Visual description",
      "draftWithAi": "Draft with AI",
      "regenerate": "Regenerate",
      "useThis": "Use This",
      "referenceImagesLabel": "Reference images",
      "writerHelpFallback": "the compatible scene writer model",
      "writerHelpUses": "Uses {{model}} to draft from your avatar and reference images.",
      "writerHelpUnavailable": "Add a compatible scene writer model in Image Generation settings to draft this automatically.",
      "writerNotAvailableError": "Add a compatible scene writer model in Image Generation settings first.",
      "writerNoSourcesError": "Add an avatar or at least one reference image before generating.",
      "noUsableReferences": "No usable reference images were found.",
      "draftFailed": "Failed to generate design description.",
      "draftReadFailed": "Failed to read image asset ({{status}})",
      "draftConvertFailed": "Failed to convert image asset to data URL",
      "draftGenerationFailed": "Draft generation failed.",
      "draftMenuTitle": "AI Design Draft",
      "draftedBy": "Drafted by {{model}} from the current avatar and reference images.",
      "draftedByFallback": "your scene writer model",
      "noWriterUseHelper": "Add a compatible scene writer model before using this helper.",
      "emptyReferences": "Add a few clear reference shots to lock face, proportions, outfit, and style.",
      "designReferencesTitle": "Design references",
      "designReferencesDescription": "Upload a few clear reference images plus one canonical visual description.",
      "designReferencesPlaceholder": "Describe the stable look: face, hair, build, age presentation, outfit cues, accessories, and art/style direction.",
      "dismissAria": "Dismiss",
      "v3MessageFallback": "lettuce-emb-v4 is out and dramatically improves roleplay memory recall. Upgrading is recommended.",
      "uploadButton": "Upload",
      "libraryButton": "Library",
      "companionSetupTitle": "Companion needs setup",
      "companionSetupSubtitleSingle": "Companion mode needs one more model before it can run. Skipping will switch this character back to Roleplay.",
      "companionSetupSubtitleMany": "Companion mode needs {{count}} more models before it can run. Skipping will switch this character back to Roleplay.",
      "companionSetupBody": "Companion mode needs some local models to analyze emotion, extract entities, route memories, and recall past context.",
      "companionUseRoleplay": "Use Roleplay instead",
      "companionDownloadNow": "Download now",
      "searchModelsPlaceholder": "Search models...",
      "loadingModelsDefault": "Loading models...",
      "noModelsAvailable": "No models available.",
      "noModelsMatching": "No models found matching \"{{query}}\".",
      "contentPlaceholderText": "You are a helpful AI assistant...\n\nUse {{char.name}} and {{scene}} in your prompt.",
      "previewRenderFailed": "<failed to render preview>",
      "charactersCount": "{{count}} characters"
    }
  },
  "chats": {
    "characterNotFound": "Nie znaleziono postaci",
    "chatHistory": "Historia czatu",
    "previousConversationsWithCharacter": "Poprzednie rozmowy z {{name}}",
    "previousConversations": "Poprzednie rozmowy",
    "searchChats": "Szukaj czatów...",
    "searchResults": "{{count}} wynik(ów)",
    "noConversationsYet": "Brak rozmów",
    "startChattingPrompt": "Zacznij rozmawiać, aby zobaczyć historię",
    "noMatchingChats": "Brak pasujących czatów",
    "tryDifferentSearchTerm": "Spróbuj innego wyszukiwania",
    "untitledChat": "Czat bez tytułu",
    "chatTitlePlaceholder": "Tytuł czatu...",
    "exportChatPackage": "Eksportuj pakiet czatu",
    "characterSpecificExport": "Eksport specyficzny dla postaci",
    "characterSpecificExportDesc": "Powiąż ten pakiet domyślnie z postacią czatu.",
    "nonCharacterSpecificExport": "Eksport niespecyficzny dla postaci",
    "nonCharacterSpecificExportDesc": "Wymagaj wyboru postaci podczas importu.",
    "deleteChat": "Usunąć czat?",
    "deleteConfirmDesc": "Trwale usuwa z historii",
    "keepThisChat": "Zachowaj ten czat",
    "editCharacter": "Edytuj postać",
    "exportCharacter": "Eksportuj postać",
    "chatAppearance": "Wygląd czatu",
    "importChatPackage": "Importuj pakiet czatu",
    "hideThisCharacter": "Ukryj tę postać",
    "deleteCharacter": "Usuń postać",
    "deleteCharacterTitle": "Usunąć postać?",
    "deleteCharacterConfirmation": "Czy na pewno chcesz usunąć \"{{name}}\"? Spowoduje to również usunięcie wszystkich sesji czatu z tą postacią.",
    "characterSpecificMatches": "Ten pakiet jest specyficzny dla postaci i pasuje do wybranej postaci.",
    "characterSpecificMismatch": "Ten pakiet jest specyficzny dla postaci i wskazuje na inną postać. Zostanie zaimportowany do {{name}}.",
    "nonCharacterSpecificImport": "Ten pakiet nie jest specyficzny dla postaci. Zostanie zaimportowany do {{name}}.",
    "noCharactersYet": "Brak postaci",
    "createFirstCharacter": "Utwórz swoją pierwszą postać przyciskiem + poniżej, aby zacząć rozmawiać",
    "scrollToBottom": "Przewiń na dół",
    "selectCharacter": "Wybierz postać",
    "branchToGroupChat": "Rozgałęź do czatu grupowego",
    "addContent": "Dodaj treść",
    "swapPlaces": "Zamień miejsca",
    "swapPlacesOn": "Zamień miejsca (wł.)",
    "uploadImage": "Prześlij obraz",
    "helpMeReply": "Pomóż mi odpowiedzieć",
    "sceneImage": {
      "modeTitle": "Obraz sceny",
      "modeDescription": "Zdecyduj, czy sam napiszesz podpowiedź dotyczącą sceny, czy też pozwól, aby sztuczna inteligencja przygotowała ją jako pierwsza.",
      "writePrompt": "Napisz monit",
      "writePromptDesc": "Wpisz dokładny monit dotyczący obrazu sceny, którego chcesz użyć.",
      "askAi": "Zapytaj AI",
      "askAiDesc": "Pozwól, aby bieżący model czatu przygotował podpowiedź dotyczącą sceny od wybranego momentu.",
      "generateTitle": "Wygeneruj obraz sceny",
      "regenerateTitle": "Regeneruj obraz sceny",
      "aiTitle": "Monit dotyczący sceny AI",
      "promptLabel": "PODSUMOWANIE SCENY",
      "promptPlaceholder": "Opisz scenę, postacie, nastrój, oświetlenie, kadrowanie kamery i ważne szczegóły...",
      "suggestedPrompt": "Sugerowany monit",
      "regeneratePrompt": "Zregenerować",
      "editPrompt": "Edytuj monit",
      "reviewTitle": "Sprawdź prompt sceny",
      "denyPrompt": "Odrzuć",
      "acceptPrompt": "Akceptuj",
      "generateImage": "Wygeneruj obraz",
      "updateImage": "Aktualizuj obraz"
    },
    "useMyTextAsBase": "Użyj mojego tekstu jako bazy",
    "writeNewReply": "Napisz coś nowego",
    "suggestedReply": "Sugerowana odpowiedź",
    "selectTwoCharactersMinimum": "Wybierz co najmniej 2 postacie do czatu grupowego.",
    "groupBranchCreated": "Utworzono gałąź grupową! Przekierowywanie...",
    "memories": "Wspomnienia",
    "tools": "Narzędzia",
    "pinned": "Przypięte",
    "searchMemories": "Szukaj wspomnień...",
    "addMemory": "Dodaj wspomnienie",
    "memoryActions": "Akcje wspomnień",
    "pinnedMessages": "Przypięte wiadomości",
    "pinnedMessagesDesc": "Zawsze uwzględniane w kontekście",
    "contextSummary": "Podsumowanie kontekstu",
    "contextSummaryPlaceholder": "Krótkie podsumowanie utrzymujące spójność kontekstu między wiadomościami...",
    "memoryContentPlaceholder": "Co powinno być zapamiętane?",
    "editMemoryPlaceholder": "Wprowadź treść wspomnienia...",
    "togglePin": {
      "pin": "Przypnij",
      "unpin": "Odepnij"
    },
    "toggleMemoryState": {
      "setHot": "Ustaw gorące",
      "setCold": "Ustaw zimne"
    },
    "selectModelForRetry": "Wybierz model do ponowienia",
    "memoryCategories": {
      "characterTrait": "cecha postaci",
      "relationship": "relacja",
      "plotEvent": "wydarzenie fabularne",
      "worldDetail": "szczegół świata",
      "preference": "preferencja",
      "other": "inne"
    },
    "settings": {
      "memorySection": "Pamięć",
      "memorySectionDesc": "Podsumowanie, tagi, historia wywołań narzędzi",
      "quickSettings": "Szybkie ustawienia",
      "quickSettingsDesc": "Najczęstsze dostosowania",
      "persona": "Persona",
      "model": "Model",
      "fallbackModel": "Model zapasowy",
      "voice": "Głos",
      "voiceDesc": "Odtwarzanie tekstu-na-mowę",
      "advanced": "Zaawansowane",
      "advancedDesc": "Nadpisz parametry modelu dla tej sesji",
      "session": "Sesja",
      "sessionDesc": "Rozpocznij nowe czaty i przeglądaj historię",
      "newChat": "Nowy czat",
      "newChatDesc": "Rozpocznij nową rozmowę",
      "chatHistoryDesc": "Zobacz poprzednie sesje",
      "importChatPackageDesc": "Importuj .chatpkg do tej postaci",
      "selectModel": "Wybierz model",
      "selectFallbackModel": "Wybierz model zapasowy",
      "personaActions": "Akcje persony",
      "sessionAdvancedSettings": "Zaawansowane ustawienia sesji",
      "parameterSupport": "Obsługa parametrów",
      "backToChat": "Wróć do czatu",
      "chatSettingsTitle": "Ustawienia czatu",
      "chatSettingsSubtitle": "Zarządzaj preferencjami rozmowy",
      "modelDefaults": "Domyślne modelu",
      "appDefaults": "Domyślne aplikacji",
      "openChatSessionFirst": "Najpierw otwórz sesję czatu",
      "sessionRequired": "Wymagana sesja",
      "noPersona": "Bez persony",
      "customPersona": "Niestandardowa persona",
      "noModelAvailable": "Brak dostępnego modelu",
      "fallbackNone": "Brak",
      "unknownModel": "Nieznany model",
      "authorNote": "Notatka autora",
      "identityProfileAuthored": "Profil tożsamości opracowany",
      "addIdentityProfile": "Dodaj profil tożsamości towarzysza",
      "soulLabel": "Dusza",
      "sessionTitle": "Sesja: {{title}}",
      "sessionUntitled": "Bez tytułu",
      "messageCount": "{{count}} wiadomości",
      "usingCharacterDefault": "Używa domyślnego postaci",
      "sessionOverrideActive": "Aktywne nadpisanie sesji",
      "autoplayVoice": "Autoodtwarzanie głosu",
      "useCharacterDefault": "Użyj domyślnego postaci"
    },
    "search": {
      "placeholder": "Szukaj w rozmowie...",
      "noMessagesFound": "Nie znaleziono wiadomości",
      "you": "Ty",
      "character": "Postać",
      "failed": "Wyszukiwanie wiadomości nie powiodło się"
    },
    "templates": {
      "startWithTemplate": "Rozpocząć z szablonu?",
      "noTemplate": "Bez szablonu",
      "startWithSceneOnly": "Rozpocznij tylko ze sceną",
      "you": "Ty",
      "bot": "Bot",
      "messageCount": "{{count}} wiadomości"
    },
    "header": {
      "back": "Wstecz",
      "openSettings": "Otwórz ustawienia czatu",
      "manageMemories": "Zarządzaj wspomnieniami",
      "searchMessages": "Szukaj wiadomości",
      "manageLorebooks": "Zarządzaj lorebookami",
      "conversationSettings": "Ustawienia rozmowy"
    },
    "footer": {
      "sendMessagePlaceholder": "Wyślij wiadomość...",
      "stopGeneration": "Zatrzymaj generowanie",
      "sendMessage": "Wyślij wiadomość",
      "continueConversation": "Kontynuuj rozmowę",
      "moreOptions": "Więcej opcji",
      "addImage": "Dodaj obraz",
      "addImageAttachment": "Dodaj załącznik obrazu",
      "removeAttachment": "Usuń załącznik",
      "recordVoice": "Nagraj głos"
    },
    "message": {
      "thinkingMessages": {
        "thinkingReallyHard": "Myślę naprawdę intensywnie…",
        "lettuceCouncil": "Konsultuję się z radą sałaty…",
        "stealingThoughts": "Kradnę myśli z pustki…",
        "warmingBrainCells": "Rozgrzewam komórki mózgowe…",
        "forbiddenKnowledge": "Ładuję zakazaną wiedzę…",
        "overthinking": "Przesadzam z myśleniem (jak zwykle)…",
        "pretendingToBeSmart": "Udaję mądrego…",
        "crunchingNumbers": "Przetwarzam urojone liczby…",
        "arguingWithMyself": "Kłócę się sam ze sobą…",
        "askingUniverse": "Grzecznie pytam wszechświat…"
      },
      "thoughtProcess": "Proces myślowy",
      "regenerateResponse": "Regeneruj odpowiedź",
      "guidedRegenerationTitle": "Prowadź regenerację",
      "guidedRegenerationLabel": "Jak powinno się zmienić?",
      "guidedRegenerationDescription": "Opisz ton, długość, szczegóły do zachowania lub usunięcia oraz to, co następna odpowiedź powinna zrobić inaczej.",
      "guidedRegenerationPlaceholder": "Zrób to krótsze, cieplejsze, bardziej bezpośrednie...",
      "guidedRegenerationSubmit": "Regeneruj",
      "cancelAudioGeneration": "Anuluj generowanie audio",
      "stopAudio": "Zatrzymaj audio",
      "playMessageAudio": "Odtwórz audio wiadomości",
      "playAudio": "Odtwórz audio",
      "sceneLabel": "Scena",
      "variantLabel": "Wariant",
      "regenerating": "Regenerowanie",
      "assistantIsTyping": "Asystent pisze",
      "attachedImage": "Załączony obraz"
    },
    "actions": {
      "assistantMessage": "Wiadomość asystenta",
      "userMessage": "Wiadomość użytkownika",
      "promptTokens": "Tokeny promptu",
      "completionTokens": "Tokeny odpowiedzi",
      "fallbackModelUsed": "Użyto modelu zapasowego",
      "total": "łącznie",
      "timeToFirstToken": "Czas do pierwszego tokenu",
      "completionTokenSpeed": "Prędkość tokenów odpowiedzi",
      "edit": "Edytuj",
      "copy": "Kopiuj",
      "pin": "Przypnij",
      "unpin": "Odepnij",
      "rewindToHere": "Przewiń do tego miejsca",
      "branchFromHere": "Rozgałęź stąd",
      "branchToGroupChat": "Rozgałęź do czatu grupowego",
      "branchToCharacter": "Rozgałęź do postaci",
      "generateSceneImage": "Wygeneruj obraz sceny",
      "regenerateSceneImage": "Regeneruj obraz sceny",
      "chatAppearance": "Wygląd czatu",
      "delete": "Usuń",
      "debug": "Debug",
      "unpinToDelete": "Odepnij, aby usunąć",
      "editPlaceholder": "Edytuj swoją wiadomość...",
      "memoriesUsed": "{{count}} wspomnień użyto",
      "lorebookUsage": "Użycie lorebooka",
      "lorebookUsageDesc": "Ta odpowiedź wykorzystała następujące wpisy lorebooka.",
      "matchScore": "Dopasowanie: {{score}}%",
      "unknownModel": "Nieznany model",
      "loadingModel": "Ładowanie modelu..."
    },
    "emptyState": {
      "goBack": "Wróć"
    },
    "settingsDrawer": {
      "title": "Ustawienia czatu",
      "subtitle": "Zarządzaj preferencjami rozmowy"
    },
    "history": {
      "archivedBadge": "Zarchiwizowane",
      "messagesCount": "{{count}} wiadomości",
      "previousGroupPage": "Poprzednia strona {{label}}",
      "nextGroupPage": "Następna strona {{label}}",
      "sillyTavernFormat": "Format SillyTavern",
      "sillyTavernFormatDesc": "Eksportuj jako .jsonl kompatybilny z SillyTavern.",
      "failedLoad": "Nie udało się załadować danych",
      "failedDelete": "Nie udało się usunąć: {{error}}",
      "failedRename": "Nie udało się zmienić nazwy: {{error}}",
      "chatPackageExportedTo": "Pakiet czatu wyeksportowany do:\n{{path}}",
      "sillyTavernExportedTo": "Czat SillyTavern wyeksportowany do:\n{{path}}",
      "failedExportChatPackage": "Nie udało się wyeksportować pakietu czatu",
      "failedExportSillyTavern": "Nie udało się wyeksportować czatu SillyTavern"
    },
    "authorNote": {
      "title": "Notatka autora",
      "inlineEditor": "Edytor wbudowany",
      "inlineEditorDesc": "Pokaż notatkę autora nad polem wiadomości do szybkich edycji.",
      "toggleInlineEditor": "Przełącz wbudowany edytor notatek autora",
      "placeholder": "Prywatne wskazówki dla tego czatu",
      "willBeRemoved": "Notatka autora zostanie usunięta przy zapisie",
      "noNoteSaved": "Brak zapisanej notatki autora",
      "charactersCount": "{{count}} znaków",
      "clear": "Wyczyść",
      "save": "Zapisz",
      "saving": "Zapisywanie...",
      "failedSave": "Nie udało się zapisać notatki autora",
      "addPlaceholder": "Dodaj notatkę autora...",
      "savingShort": "Zapisywanie..."
    },
    "drawer": {
      "chatSettingsTitle": "Ustawienia czatu",
      "chatSettingsSubtitle": "Zarządzaj preferencjami rozmowy"
    },
    "companionSoul": {
      "loading": "Ładowanie duszy towarzysza...",
      "unavailable": "Dusza towarzysza jest niedostępna",
      "unavailableDesc": "Edycja duszy jest dostępna tylko dla postaci w trybie towarzysza.",
      "pageTitle": "Dusza towarzysza",
      "back": "Wstecz",
      "save": "Zapisz"
    },
    "companionRelationship": {
      "back": "Wstecz",
      "loading": "Ładowanie stanu relacji...",
      "unavailableTitle": "Stan relacji jest niedostępny",
      "sessionLoadFailed": "Nie udało się załadować sesji czatu.",
      "backToChat": "Wróć do czatu",
      "notCompanionTitle": "Ten czat nie jest w trybie towarzysza",
      "notCompanionDesc": "Strony relacji towarzysza wyświetlają się tylko dla czatów, których tryb postaci to towarzysz.",
      "openRegularMemories": "Otwórz zwykłe wspomnienia",
      "pageTitle": "Stan relacji",
      "memoryButton": "Pamięć",
      "lastInteraction": "Ostatnia interakcja {{time}}",
      "bond": "Więź",
      "closeness": "Bliskość",
      "trust": "Zaufanie",
      "affection": "Uczucie",
      "tension": "Napięcie",
      "stability": "Stabilność",
      "interactions": "Interakcje",
      "vsDefaults": "vs. domyślne postaci",
      "updatedAt": "Zaktualizowano {{time}}",
      "emotionalEngine": "Silnik emocjonalny",
      "felt": "Odczuwane",
      "feltDesc": "Wewnętrzny afekt",
      "expressed": "Wyrażane",
      "expressedDesc": "Pojawia się w odpowiedziach",
      "blocked": "Zablokowane",
      "blockedDesc": "Tłumione przez personę",
      "momentum": "Impet",
      "momentumDesc": "Trend w ostatnich turach",
      "activeDrivers": "Aktywne czynniki",
      "soul": "Dusza",
      "essence": "Istota",
      "voice": "Głos",
      "relationalStyle": "Styl relacyjny",
      "vulnerabilities": "Wrażliwości",
      "habits": "Nawyki",
      "boundaries": "Granice",
      "eventsCount": "{{count}} wydarzeń",
      "recentTimeline": "Ostatnia oś czasu",
      "superseded": "zastąpione",
      "promptScore": "Prompt {{score}}",
      "persistenceScore": "Trwałość {{score}}",
      "noTimeline": "Brak osi czasu",
      "noTimelineDesc": "Wspomnienia relacji, kamieni milowych i emocjonalnych migawek pojawią się tutaj, gdy towarzysz nauczy się z rozmów.",
      "notAuthoredYet": "Jeszcze nie opracowane.",
      "noSignal": "Brak sygnału."
    },
    "companionUi": {
      "relationship": "Relacja",
      "milestones": "Kamienie milowe",
      "boundaries": "Granice",
      "preferences": "Preferencje",
      "profile": "Profil",
      "routines": "Rutyny",
      "episodes": "Epizody",
      "emotionalSnapshots": "Emocjonalne migawki",
      "unknownTime": "Nieznane",
      "justNow": "przed chwilą",
      "minutesAgo": "{{minutes}} min. temu",
      "hoursAgo": "{{hours}} godz. temu",
      "daysAgo": "{{days}} dni temu",
      "notSetYet": "Jeszcze nie ustawione",
      "missingCharacterId": "Brakujące characterId",
      "sessionNotFound": "Nie znaleziono sesji",
      "failedLoadCompanion": "Nie udało się załadować sesji towarzysza"
    },
    "chatPage": {
      "characterNotFound": "Nie znaleziono postaci",
      "characterDoesntExist": "Szukana postać nie istnieje."
    },
    "debugPage": {
      "copy": "Kopiuj"
    },
    "companionMemoryPage": {
      "backLabel": "Wstecz",
      "refineMemoryPlaceholder": "Dopracuj przechowywane wspomnienia towarzysza",
      "superseded": "zastąpione",
      "pinned": "przypięte",
      "cold": "zimne"
    }
  },
  "groupChats": {
    "list": {
      "editGroup": "Edytuj grupę",
      "deleteGroup": "Usuń grupę",
      "deleteConfirmTitle": "Usunąć czat grupowy?",
      "deleteConfirmMessage": "Czy na pewno chcesz usunąć \"{{name}}\"? Spowoduje to również usunięcie wszystkich wiadomości w tym czacie grupowym.",
      "noGroupChatsYet": "Brak czatów grupowych",
      "noGroupChatsDesc": "Utwórz swój pierwszy czat grupowy przyciskiem + poniżej, aby rozmawiać z wieloma postaciami naraz",
      "newChat": "Nowy czat",
      "openChat": "Otwórz czat",
      "chatSettings": "Ustawienia czatu",
      "sessionCount": "{{count}} czatów"
    },
    "create": {
      "invalidPackage": "Ten pakiet nie jest pakietem czatu grupowego.",
      "inspectPackageError": "Nie udało się zbadać pakietu czatu grupowego",
      "importPackageError": "Nie udało się zaimportować pakietu czatu grupowego",
      "importChatpkg": "Importuj `.chatpkg`",
      "mapParticipantsTitle": "Mapuj uczestników",
      "selectLocalCharacter": "Wybierz lokalną postać dla tego uczestnika.",
      "selectCharacterPlaceholder": "Wybierz postać...",
      "importChatPackageTitle": "Importuj pakiet czatu",
      "importChatPackageDesc": "Spowoduje to import wybranego `.chatpkg` jako nowej sesji grupowej.",
      "characterSelect": {
        "title": "Utwórz czat grupowy",
        "subtitle": "Wybierz postacie do rozmowy grupowej",
        "selected": "wybrano",
        "ready": "Gotowe",
        "minRequired": "Min. 2 wymagane",
        "noCharactersYet": "Brak postaci",
        "noCharactersDesc": "Najpierw utwórz postacie, aby rozpocząć czat grupowy",
        "continueToSetup": "Przejdź do konfiguracji grupy"
      },
      "groupSetup": {
        "title": "Konfiguracja grupy",
        "subtitle": "Skonfiguruj ustawienia czatu grupowego",
        "chatType": "Typ czatu",
        "conversation": "Rozmowa",
        "casualChat": "Swobodny czat",
        "roleplay": "Roleplay",
        "withScenes": "Ze scenami",
        "conversationDesc": "Swobodna rozmowa grupowa bez scen początkowych",
        "roleplayDesc": "Scenariusz roleplay ze sceną początkową i immersyjnymi promptami",
        "speakerSelection": "Wybór mówcy",
        "llm": "LLM",
        "aiPicks": "AI wybiera",
        "heuristic": "Heurystyka",
        "scoreBased": "Na podstawie punktacji",
        "roundRobin": "Po kolei",
        "takeTurns": "Na zmianę",
        "llmDesc": "Używa domyślnego modelu do wyboru mówcy (kosztuje tokeny)",
        "heuristicDesc": "Używa balansu uczestnictwa i wskazówek kontekstowych (za darmo)",
        "roundRobinDesc": "Postacie mówią po kolei (za darmo)",
        "chatBackground": "Tło czatu",
        "optional": "(Opcjonalnie)",
        "uploadBackground": "Prześlij obraz tła",
        "backgroundDesc": "Ustaw obraz tła dla tego czatu grupowego",
        "groupName": "Nazwa grupy",
        "removeBackground": "Usuń obraz tła",
        "groupNameAutoGenerate": "Zostaw puste, aby automatycznie wygenerować z nazw postaci",
        "continueToScene": "Przejdź do sceny początkowej",
        "createGroupChat": "Utwórz czat grupowy"
      },
      "startingScene": {
        "title": "Scena początkowa",
        "subtitle": "Ustaw scenariusz otwarcia dla twojego roleplayu",
        "sceneSource": "Źródło sceny",
        "none": "Brak",
        "custom": "Niestandardowa",
        "fromCharacter": "Od postaci",
        "noneDesc": "Rozpocznij bez predefiniowanej sceny",
        "customDesc": "Napisz własny scenariusz otwarcia",
        "fromCharacterDesc": "Użyj sceny jednej z twoich postaci",
        "sceneContent": "Treść sceny",
        "sceneContentPlaceholder": "Opisz scenę początkową dla tego roleplayu...",
        "sceneReferenceTip": "Wskazówka: Wpisz {{@\", aby odwołać się do postaci",
        "selectScene": "Wybierz scenę",
        "sceneLabel": " — Scena",
        "copyToCustom": "Kopiuj do niestandardowej i edytuj"
      }
    },
    "history": {
      "title": "Historia czatu grupowego",
      "subtitle": "Wszystkie rozmowy grupowe",
      "searchPlaceholder": "Szukaj czatów grupowych...",
      "active": "Aktywne ({{count}})",
      "archived": "Zarchiwizowane ({{count}})",
      "noChatsYet": "Brak czatów grupowych",
      "noChatsDesc": "Utwórz czat grupowy, aby zobaczyć historię",
      "noMatchingChats": "Brak pasujących czatów",
      "noMatchingDesc": "Spróbuj innego wyszukiwania",
      "deleteSessionTitle": "Usunąć czat grupowy?",
      "deleteSessionDesc": "Trwale usuwa z historii",
      "deleteSessionButton": "Usuń czat",
      "keepChat": "Zachowaj ten czat"
    },
    "session": {
      "chatTitlePlaceholder": "Tytuł czatu...",
      "newChat": "Nowy czat",
      "rename": "Zmień nazwę",
      "unarchive": "Przywróć z archiwum",
      "archive": "Archiwizuj",
      "messageCount": "{{count}} wiadomości"
    },
    "memories": {
      "tabMemories": "Wspomnienia",
      "tabPinned": "Przypięte",
      "tabActivity": "Aktywność",
      "processing": "Przetwarzanie",
      "contextSummaryTitle": "Podsumowanie kontekstu",
      "addContextSummaryPrompt": "Stuknij, aby dodać podsumowanie kontekstu...",
      "savedMemories": "Zapisane wspomnienia",
      "resultsCount": "Wyniki ({{count}})",
      "searchPlaceholder": "Szukaj wspomnień...",
      "addMemory": "Dodaj wspomnienie",
      "noMemoriesYet": "Brak wspomnień",
      "noMemoriesDesc": "Stuknij przycisk Dodaj powyżej, aby utworzyć",
      "noMatchingMemories": "Brak pasujących wspomnień",
      "noMatchingDesc": "Spróbuj innego wyszukiwania",
      "sessionNotFound": "Nie znaleziono sesji",
      "memoryActions": "Akcje wspomnień",
      "tokens": "tokeny",
      "cycle": "Cykl",
      "accessed": "Dostęp",
      "cold": "Zimne",
      "hot": "Gorące",
      "activityLog": "Dziennik aktywności",
      "events": "wydarzenia",
      "run": "Uruchom",
      "processingMemories": "AI organizuje wspomnienia grupowe...",
      "memoryCycleSuccess": "Cykl pamięci przetworzony pomyślnie!",
      "memoryActionFailed": "Akcja pamięci nie powiodła się",
      "newMemoryUpdates": "Dostępne nowe aktualizacje wspomnień",
      "noActivityYet": "Brak aktywności",
      "noActivityDesc": "Wywołania narzędzi pojawiają się, gdy AI zarządza wspomnieniami w trybie dynamicznym",
      "contextSummaryPlaceholder": "Krótkie podsumowanie utrzymujące spójność kontekstu między wiadomościami...",
      "addMemoryTitle": "Dodaj wspomnienie",
      "memoryPlaceholder": "Co powinno być zapamiętane?",
      "saveMemory": "Zapisz wspomnienie",
      "editMemoryTitle": "Edytuj wspomnienie",
      "editMemoryPlaceholder": "Wprowadź treść wspomnienia...",
      "edit": "Edytuj",
      "pin": "Przypnij",
      "unpin": "Odepnij",
      "setHot": "Ustaw gorące",
      "setCold": "Ustaw zimne"
    },
    "toolLog": {
      "created": "Utworzone",
      "deleted": "Usunięte",
      "pinned": "Przypięte",
      "unpinned": "Odpięte",
      "done": "Gotowe",
      "pinnedBadge": "przypięte",
      "softDelete": "miękkie usunięcie",
      "memoryCycle": "Cykl pamięci",
      "failedAt": "Błąd w:",
      "window": "Okno",
      "justNow": "przed chwilą",
      "minutesAgo": "{{count}} min. temu",
      "hoursAgo": "{{count}} godz. temu",
      "yesterday": "wczoraj",
      "daysAgo": "{{count}} dni temu",
      "revertingTitle": "Przywracanie...",
      "revertCycleTitle": "Cofnij ten cykl",
      "revertedAt": "Cofnięto {{time}}",
      "failedAtStage": "Błąd w: {{stage}}",
      "hideDebug": "Ukryj debugowanie",
      "debug": "Debugowanie",
      "windowRange": "Okno {{start}}-{{end}}",
      "actionCreated": "Utworzone",
      "actionDeleted": "Usunięte",
      "actionPinned": "Przypięte",
      "actionUnpinned": "Odpięte",
      "actionDone": "Gotowe",
      "badgePinned": "przypięte",
      "badgeSoftDelete": "miękkie usunięcie",
      "badgeUndone": "cofnięte",
      "badgeReverted": "przywrócone",
      "activityEmptyTitle": "Brak aktywności",
      "activityEmptyDesc": "Wywołania narzędzi pojawiają się, gdy AI zarządza wspomnieniami w trybie dynamicznym"
    },
    "message": {
      "thinkingHard": "Myślę naprawdę intensywnie…",
      "thinkingLettuce": "Konsultuję się z radą sałaty…",
      "thinkingVoid": "Kradnę myśli z pustki…",
      "thinkingBrainCells": "Rozgrzewam komórki mózgowe…",
      "thinkingForbidden": "Ładuję zakazaną wiedzę…",
      "thinkingOverthinking": "Przesadzam z myśleniem (jak zwykle)…",
      "thinkingPretending": "Udaję mądrego…",
      "thinkingCrunching": "Przetwarzam urojone liczby…",
      "thinkingArguing": "Kłócę się sam ze sobą…",
      "thinkingUniverse": "Grzecznie pytam wszechświat…",
      "thoughtProcess": "Proces myślowy",
      "userAlt": "Użytkownik",
      "assistantAlt": "Asystent",
      "regenerateResponse": "Regeneruj odpowiedź",
      "variantLabel": "Wariant",
      "regenerating": "Regenerowanie",
      "cancelAudioGeneration": "Anuluj generowanie audio",
      "stopAudio": "Zatrzymaj audio",
      "playMessageAudio": "Odtwórz audio wiadomości",
      "playAudio": "Odtwórz audio",
      "attachedImage": "Załączony obraz",
      "assistantIsTyping": "Asystent pisze",
      "assistantTyping": "Asystent pisze"
    },
    "header": {
      "back": "Wstecz",
      "memories": "Wspomnienia",
      "settings": "Ustawienia",
      "characters": "postacie"
    },
    "footer": {
      "mentionCharacter": "Wspomnij postać",
      "noCharactersFound": "Nie znaleziono postaci",
      "moreOptions": "Więcej opcji",
      "addImage": "Dodaj obraz",
      "messagePlaceholder": "Wiadomość... (@ aby wspomnieć)",
      "stopGeneration": "Zatrzymaj generowanie",
      "sendMessage": "Wyślij wiadomość",
      "continueConversation": "Kontynuuj rozmowę",
      "dismissError": "Odrzuć błąd",
      "removeAttachment": "Usuń załącznik",
      "tabToSelect": "Tab, aby wybrać"
    },
    "messageActions": {
      "characterMessage": "Wiadomość postaci",
      "yourMessage": "Twoja wiadomość",
      "whyCharacterResponded": "Dlaczego ta postać odpowiedziała",
      "total": "łącznie",
      "edit": "Edytuj",
      "copy": "Kopiuj",
      "regenerateWithDifferent": "Regeneruj z inną postacią",
      "rewindToHere": "Przewiń do tego miejsca",
      "unpinToDelete": "Odepnij, aby usunąć",
      "delete": "Usuń",
      "editPlaceholder": "Edytuj swoją wiadomość...",
      "chooseCharacterTitle": "Wybierz postać",
      "selectCharacterForRegeneration": "Wybierz, która postać powinna odpowiedzieć:"
    },
    "settings": {
      "appDefault": "Domyślne aplikacji",
      "selectPersona": "Wybierz personę",
      "noPersonas": "Brak dostępnych person",
      "noPersonasDesc": "Utwórz personę w ustawieniach, aby spersonalizować rozmowy.",
      "searchPersonas": "Szukaj person...",
      "noPersona": "Bez persony",
      "noPersonaDesc": "Kontynuuj bez persony",
      "noPersonasFound": "Nie znaleziono person",
      "noPersonasFoundDesc": "Spróbuj innego wyszukiwania"
    },
    "groupSettings": {
      "title": "Edytuj grupę",
      "subtitle": "Zaktualizuj domyślne ustawienia dla przyszłych sesji",
      "enterGroupName": "Wprowadź nazwę grupy",
      "participant": "uczestnik",
      "participants": "uczestników",
      "uploading": "Przesyłanie...",
      "changeBackground": "Zmień tło",
      "addBackgroundImage": "Dodaj obraz tła",
      "removeBackground": "Usuń tło",
      "persona": "Persona",
      "personaSubtitle": "Domyślna persona dla nowych sesji",
      "personaLabel": "Persona",
      "speakerSelection": "Wybór mówcy",
      "speakerSubtitle": "Domyślna metoda dla nowych sesji",
      "llm": "LLM",
      "aiPicks": "AI wybiera",
      "heuristic": "Heurystyczny",
      "scoreBased": "Oparty na punktach",
      "roundRobin": "Po kolei",
      "takeTurns": "Na zmianę",
      "llmDesc": "Używa domyślnego modelu do wyboru mówcy (kosztuje tokeny)",
      "heuristicDesc": "Używa balansu uczestnictwa i wskazówek kontekstowych (bezpłatnie)",
      "roundRobinDesc": "Postacie mówią po kolei (bezpłatnie)",
      "memoryMode": "Tryb pamięci",
      "memorySubtitle": "Domyślny tryb pamięci dla nowych sesji",
      "manual": "Ręczny",
      "manualDesc": "Zarządzaj notatkami samodzielnie",
      "dynamic": "Dynamiczny",
      "dynamicDesc": "Automatyczne przywoływanie",
      "memoryDynamicInfo": "AI automatycznie tworzy i pobiera wspomnienia z rozmów",
      "memoryManualInfo": "Samodzielnie dodajesz i zarządzasz notatkami pamięci",
      "characters": "Postacie",
      "participantsActive": "{{total}} uczestników · {{active}} aktywnych",
      "add": "Dodaj",
      "muted": "(wyciszony)",
      "mutedByDefault": "Domyślnie wyciszony",
      "activeByDefault": "Domyślnie aktywny",
      "unmuteCharacter": "Włącz dźwięk postaci",
      "muteCharacter": "Wycisz postać",
      "minTwoRequired": "Wymagane minimum 2 postacie",
      "removeCharacter": "Usuń postać",
      "groupMinCharacters": "Grupa wymaga co najmniej 2 postaci",
      "mutedCharactersNote": "Wyciszone postacie są pomijane przez automatyczny wybór mówcy, ale mogą odpowiadać przez jawną `@wzmiankę`.",
      "addCharacterTitle": "Dodaj postać",
      "allCharactersInGroup": "Wszystkie postacie są już w tej grupie.",
      "removeCharacterTitle": "Usunąć postać?",
      "removeCharacterConfirm": "Czy na pewno chcesz usunąć",
      "removeCharacterFrom": "z domyślnych ustawień grupy?",
      "removing": "Usuwanie...",
      "remove": "Usuń"
    },
    "sessionSettings": {
      "subtitle": "Zarządzaj preferencjami czatu grupowego",
      "enterGroupName": "Wprowadź nazwę grupy",
      "participant": "uczestnik",
      "participants": "uczestników",
      "message": "wiadomość",
      "messages": "wiadomości",
      "uploading": "Przesyłanie...",
      "changeBackground": "Zmień tło",
      "addBackgroundImage": "Dodaj obraz tła",
      "removeBackground": "Usuń tło",
      "persona": "Persona",
      "personaSubtitle": "Twoja tożsamość w tej rozmowie",
      "personaLabel": "Persona",
      "speakerSelection": "Wybór mówcy",
      "speakerSubtitle": "Jak wybierany jest następny mówca",
      "llm": "LLM",
      "aiPicks": "AI wybiera",
      "heuristic": "Heurystyczny",
      "scoreBased": "Oparty na punktach",
      "roundRobin": "Po kolei",
      "takeTurns": "Na zmianę",
      "llmDesc": "Używa domyślnego modelu do wyboru mówcy (kosztuje tokeny)",
      "heuristicDesc": "Używa balansu uczestnictwa i wskazówek kontekstowych (bezpłatnie)",
      "roundRobinDesc": "Postacie mówią po kolei (bezpłatnie)",
      "characters": "Postacie",
      "participantsActive": "{{total}} uczestników · {{active}} aktywnych",
      "add": "Dodaj",
      "muted": "(wyciszony)",
      "unmuteCharacter": "Włącz dźwięk postaci",
      "muteCharacter": "Wycisz postać",
      "minTwoRequired": "Wymagane minimum 2 postacie",
      "removeCharacter": "Usuń postać",
      "groupMinCharacters": "Czat grupowy wymaga co najmniej 2 postaci",
      "mutedCharactersNote": "Wyciszone postacie są pomijane przez automatyczny wybór mówcy, ale mogą odpowiadać przez jawną `@wzmiankę`.",
      "data": "Dane",
      "dataSubtitle": "Eksportuj lub importuj rozmowy",
      "export": "Eksportuj",
      "exportDesc": "Zapisz jako udostępniany plik",
      "import": "Importuj",
      "importDesc": "Wczytaj rozmowę z pliku",
      "conversation": "Rozmowa",
      "conversationSubtitle": "Duplikuj lub kontynuuj w nowym czacie",
      "duplicate": "Duplikuj",
      "duplicateDesc": "Kopiuj ten czat z wiadomościami lub bez",
      "branchTo1on1": "Rozgałęź do 1-na-1",
      "branchTo1on1Desc": "Kontynuuj prywatnie z jedną postacią",
      "participation": "Uczestnictwo",
      "participationSubtitle": "Rozkład wypowiedzi między postaciami",
      "addCharacterTitle": "Dodaj postać",
      "allCharactersInGroup": "Wszystkie postacie są już w tej grupie.",
      "removeCharacterTitle": "Usunąć postać?",
      "removeCharacterConfirm": "Czy na pewno chcesz usunąć",
      "removeCharacterFrom": "z tego czatu grupowego?",
      "removing": "Usuwanie...",
      "remove": "Usuń",
      "cloneGroupTitle": "Klonuj grupę",
      "withMessages": "Z wiadomościami",
      "withMessagesDesc": "Klonuj wszystko łącznie z historią czatu",
      "withoutMessages": "Bez wiadomości",
      "withoutMessagesDesc": "Klonuj tylko ustawienia (postacie, scena początkowa)",
      "branchWithCharacterTitle": "Rozgałęź z postacią",
      "branchWithCharacterDesc": "Wybierz postać, aby kontynuować jako rozmowa 1-na-1. Wszystkie wiadomości z tej grupy zostaną przekonwertowane.",
      "continueWith": "Kontynuuj rozmowę z {{name}}",
      "exportChatPackageTitle": "Eksportuj pakiet czatu",
      "includeCharacterSnapshots": "Dołącz migawki postaci",
      "includeCharacterSnapshotsDesc": "Zachowaj dane postaci w pakiecie",
      "sessionOnly": "Tylko sesja",
      "sessionOnlyDesc": "Eksportuj tylko wiadomości i metadane",
      "mapParticipantsTitle": "Mapuj uczestników",
      "selectLocalCharacter": "Wybierz lokalną postać dla tego uczestnika.",
      "selectCharacterPlaceholder": "Wybierz postać...",
      "continue": "Kontynuuj",
      "importChatPackageTitle": "Importuj pakiet czatu",
      "importChatPackageDesc": "To zaimportuje wybrany `.chatpkg` jako nową sesję grupową.",
      "importing": "Importowanie..."
    },
    "page": {
      "selectingCharacter": "Wybieranie postaci...",
      "sessionNotFound": "Nie znaleziono sesji grupowej",
      "backToGroupChats": "Wróć do czatów grupowych",
      "startConversation": "Rozpocznij rozmowę z {{names}}",
      "scrollToBottom": "Przewiń do dołu",
      "addContent": "Dodaj treść",
      "uploadImage": "Prześlij obraz",
      "helpMeReply": "Pomóż mi odpowiedzieć",
      "helpMeReplyDesc": "Pozwól AI zaproponować, co powiedzieć",
      "haveDraftPrompt": "Masz roboczą wiadomość. Jak chcesz postąpić?",
      "useMyTextAsBase": "Użyj mojego tekstu jako bazy",
      "useMyTextAsBaseDesc": "Rozszerz i popraw swój szkic",
      "writeSomethingNew": "Napisz coś nowego",
      "writeSomethingNewDesc": "Wygeneruj świeżą odpowiedź",
      "suggestedReply": "Sugerowana odpowiedź",
      "reasoningBeforeWriting": "Rozumowanie przed napisaniem odpowiedzi...",
      "writingYourReply": "Pisanie twojej odpowiedzi...",
      "regenerate": "Regeneruj",
      "useReply": "Użyj odpowiedzi",
      "helpMeReplyFailedGeneric": "Pomoc w odpowiedzi nie powiodła się.",
      "helpMeReplyFailedGenerate": "Nie udało się wygenerować odpowiedzi.",
      "noAudioCaptured": "Nie przechwycono dźwięku.",
      "noWhisperModel": "Nie znaleziono zainstalowanego modelu Whisper. Zainstaluj jeden w ustawieniach Rozpoznawania mowy.",
      "cancelRecording": "Anuluj nagrywanie",
      "transcribing": "Transkrypcja",
      "stopAndTranscribe": "Zatrzymaj i transkrybuj",
      "recordVoice": "Nagraj głos",
      "learnCorrection": "Naucz korekty:",
      "learning": "Uczenie...",
      "learn": "Naucz",
      "ignore": "Ignoruj",
      "groupChatFailed": "Czat grupowy nie powiódł się.",
      "failedToCopy": "Nie udało się skopiować",
      "copied": "Skopiowano!",
      "cannotDeletePinned": "Nie można usunąć przypiętej wiadomości. Najpierw ją odepnij.",
      "failedToDelete": "Nie udało się usunąć",
      "messageNotFound": "Nie znaleziono wiadomości",
      "cannotRewindPinned": "Nie można przewinąć: po tym miejscu są przypięte wiadomości. Najpierw je odepnij.",
      "failedToRewind": "Nie udało się przewinąć",
      "failedToTogglePin": "Nie udało się przełączyć przypięcia",
      "messagePinned": "Wiadomość przypięta",
      "messageUnpinned": "Wiadomość odpięta",
      "failedToSave": "Nie udało się zapisać"
    },
    "memoriesPage": {
      "summarizingConversation": "Podsumowywanie rozmowy",
      "analyzingMemories": "Analizowanie wspomnień",
      "applyingChanges": "Stosowanie zmian",
      "organizingMemories": "Organizowanie wspomnień",
      "retryingMemoryCycle": "Ponawiam cykl pamięci...",
      "processingMemories": "Przetwarzanie wspomnień...",
      "memorySystemError": "Błąd systemu pamięci",
      "contextSummary": "Podsumowanie kontekstu",
      "contextSummaryTitle": "Podsumowanie kontekstu",
      "savedMemories": "Zapisane wspomnienia",
      "resultsCount": "Wyniki ({{count}})",
      "searchMemoriesPlaceholder": "Szukaj wspomnień...",
      "addMemory": "Dodaj wspomnienie",
      "memoryActions": "Akcje wspomnień",
      "clearSearch": "Wyczyść wyszukiwanie",
      "noMatchingMemories": "Brak pasujących wspomnień",
      "noMemoriesYet": "Brak wspomnień",
      "tryDifferentSearch": "Spróbuj innego wyszukiwania",
      "tapAddToCreate": "Stuknij przycisk Dodaj powyżej, aby utworzyć",
      "pinnedMessages": "Przypięte wiadomości",
      "refresh": "Odśwież",
      "noPinnedMessages": "Brak przypiętych wiadomości",
      "pinImportantDesc": "Przypinaj ważne wiadomości czatu grupowego, aby zawsze były w kontekście.",
      "assistant": "Asystent",
      "user": "Użytkownik",
      "unpin": "Odepnij",
      "failedToUnpinMessage": "Nie udało się odpiąć wiadomości",
      "activityLog": "Dziennik aktywności",
      "run": "Uruchom",
      "addMemoryTitle": "Dodaj wspomnienie",
      "editMemoryTitle": "Edytuj wspomnienie",
      "memoryTitle": "Wspomnienie",
      "memoryPlaceholder": "Co powinno być zapamiętane?",
      "saveMemory": "Zapisz wspomnienie",
      "editMemoryPlaceholder": "Wprowadź treść wspomnienia...",
      "saving": "Zapisywanie...",
      "save": "Zapisz",
      "cancel": "Anuluj",
      "contextSummaryPlaceholder": "Krótkie podsumowanie utrzymujące spójność kontekstu między wiadomościami...",
      "contextSummaryPrompt": "Stuknij, aby dodać podsumowanie kontekstu...",
      "cycle": "Cykl",
      "accessed": "Dostęp",
      "cold": "Zimne",
      "hot": "Gorące",
      "tokens": "tokeny",
      "pin": "Przypnij",
      "setHot": "Ustaw gorące",
      "setCold": "Ustaw zimne",
      "edit": "Edytuj",
      "delete": "Usuń",
      "failedToToggleMemPin": "Nie udało się przełączyć przypięcia",
      "failedToRemoveMemory": "Nie udało się usunąć wspomnienia",
      "toolEventsCountAria": "wydarzeń",
      "activityEmptyDesc": "Wywołania narzędzi pojawiają się, gdy AI zarządza wspomnieniami w trybie dynamicznym",
      "memoryCycleSuccess": "Cykl pamięci przetworzony pomyślnie!"
    },
    "messageActionsExtra": {
      "characterMessage": "Wiadomość postaci",
      "yourMessage": "Twoja wiadomość",
      "whyCharacterResponded": "Dlaczego ta postać odpowiedziała",
      "promptTokensTitle": "Tokeny promptu",
      "completionTokensTitle": "Tokeny odpowiedzi",
      "total": "łącznie",
      "regenerateWithDifferent": "Regeneruj z inną postacią",
      "unpin": "Odepnij",
      "pin": "Przypnij",
      "rewindToHere": "Przewiń do tego miejsca",
      "unpinToDelete": "Odepnij, aby usunąć",
      "editPlaceholder": "Edytuj swoją wiadomość...",
      "chooseCharacter": "Wybierz postać",
      "selectCharacterForRegeneration": "Wybierz, która postać powinna odpowiedzieć:"
    },
    "timeAgo": {
      "justNow": "przed chwilą",
      "minutesAgo": "{{count}} min. temu",
      "hoursAgo": "{{count}} godz. temu",
      "daysAgo": "{{count}} dni temu"
    },
    "memoriesController": {
      "missingSessionId": "Brakujące sessionId",
      "sessionNotFound": "Nie znaleziono sesji",
      "failedToLoadSession": "Nie udało się załadować sesji",
      "failedToUpdateTemperature": "Nie udało się zaktualizować temperatury pamięci",
      "failedToSaveSummary": "Nie udało się zapisać podsumowania",
      "cycleFailed": "Cykl pamięci grupowej nie powiódł się",
      "failedToAddMemory": "Nie udało się dodać wspomnienia",
      "failedToUpdateMemory": "Nie udało się zaktualizować wspomnienia",
      "failedToRunCycle": "Nie udało się uruchomić cyklu pamięci",
      "failedToRevertCycle": "Nie udało się cofnąć cyklu",
      "failedToRefresh": "Nie udało się odświeżyć",
      "failedToDismissError": "Nie udało się odrzucić błędu",
      "failedToTogglePinned": "Nie udało się przełączyć przypiętej wiadomości",
      "sessionNotLoaded": "Sesja nie załadowana",
      "revertCycleTitle": "Cofnąć ten cykl?",
      "revertConfirm": "Cofnij"
    },
    "chatController": {
      "sessionNotFound": "Nie znaleziono sesji grupowej",
      "failedToLoadGroupChat": "Nie udało się załadować czatu grupowego",
      "requestFailed": "Żądanie czatu grupowego nie powiodło się",
      "failedToSendMessage": "Nie udało się wysłać wiadomości",
      "failedToRegenerate": "Nie udało się zregenerować",
      "failedToContinue": "Nie udało się kontynuować",
      "failedToCopy": "Nie udało się skopiować",
      "cannotDeletePinned": "Nie można usunąć przypiętej wiadomości. Najpierw ją odepnij.",
      "failedToDelete": "Nie udało się usunąć",
      "messageNotFound": "Nie znaleziono wiadomości",
      "cannotRewindPinned": "Nie można przewinąć: po tym miejscu są przypięte wiadomości. Najpierw je odepnij.",
      "failedToRewind": "Nie udało się przewinąć",
      "failedToTogglePin": "Nie udało się przełączyć przypięcia",
      "messagePinned": "Wiadomość przypięta",
      "messageUnpinned": "Wiadomość odpięta",
      "failedToSave": "Nie udało się zapisać",
      "copied": "Skopiowano!"
    },
    "historyController": {
      "failedToLoadData": "Nie udało się załadować danych",
      "failedToDelete": "Nie udało się usunąć: {{error}}",
      "failedToRename": "Nie udało się zmienić nazwy: {{error}}",
      "failedToArchive": "Nie udało się zarchiwizować: {{error}}",
      "failedToUnarchive": "Nie udało się przywrócić z archiwum: {{error}}",
      "failedToDuplicate": "Nie udało się zduplikować"
    },
    "sessionSettingsController": {
      "failedToLoad": "Nie udało się załadować ustawień czatu grupowego",
      "noPersona": "Brak persony",
      "customPersona": "Niestandardowa persona",
      "minOneActive": "Co najmniej jeden uczestnik musi pozostać aktywny"
    },
    "groupSettingsController": {
      "notFound": "Nie znaleziono grupy",
      "failedToLoad": "Nie udało się załadować ustawień grupy"
    },
    "createForm": {
      "failedToLoadCharacters": "Nie udało się załadować postaci",
      "selectAtLeastTwo": "Wybierz co najmniej 2 postacie do czatu grupowego",
      "failedToCreate": "Nie udało się utworzyć czatu grupowego"
    },
    "groupSetupExtra": {
      "memoryMode": "Tryb pamięci",
      "manual": "Ręczny",
      "manualDesc": "Zarządzaj notatkami samodzielnie",
      "dynamic": "Dynamiczny",
      "dynamicDesc": "Automatyczne podsumowania i przywoływanie",
      "memoryManualInfo": "Samodzielnie dodajesz i zarządzasz notatkami pamięci",
      "memoryDynamicInfo": "AI automatycznie tworzy i pobiera wspomnienia z rozmów",
      "backgroundPreviewAlt": "Podgląd tła"
    },
    "createExtra": {
      "enterGroupNamePlaceholder": "Wprowadź nazwę grupy...",
      "unknown": "Nieznane"
    },
    "startingSceneExtra": {
      "insertAs": "Wstaw jako {{snippet}}"
    },
    "sessionCardExtra": {
      "unknown": "Nieznane",
      "untitledChat": "Czat bez tytułu"
    },
    "sessionListExtra": {
      "unknown": "Nieznane"
    },
    "chatHeaderExtra": {
      "unknownError": "Nieznany błąd"
    },
    "chatSettingsExtra": {
      "invalidPackage": "Ten pakiet nie jest pakietem czatu grupowego.",
      "failedExport": "Nie udało się wyeksportować pakietu czatu grupowego",
      "failedInspect": "Nie udało się zbadać pakietu czatu grupowego",
      "failedImport": "Nie udało się zaimportować pakietu czatu grupowego",
      "exportSuccess": "Pakiet czatu grupowego wyeksportowany do:\n{{path}}",
      "backgroundAlt": "Tło",
      "removeBackgroundAria": "Usuń tło",
      "backAria": "Wstecz",
      "backToGroupChats": "Wróć do czatów grupowych"
    },
    "groupSettingsExtra": {
      "backToGroups": "Wróć do grup",
      "backAria": "Wstecz",
      "removeBackgroundAria": "Usuń tło"
    },
    "historyPage": {
      "backAria": "Wróć do czatów grupowych",
      "clearSearchAria": "Wyczyść wyszukiwanie",
      "resultsLabel": "{{count}} wynik",
      "resultsLabelPlural": "{{count}} wyników",
      "untitledChat": "Czat bez tytułu",
      "noPinnedMessages": "Brak przypiętych wiadomości"
    },
    "personaSelectorExtra": {
      "insertAs": "Wstaw jako",
      "appDefault": "Domyślne aplikacji",
      "defaultBadge": "Domyślne",
      "selectPersonaTitle": "Wybierz personę",
      "noPersonaTitle": "Bez persony",
      "noPersonaDesc": "Kontynuuj bez persony",
      "noPersonasAvailable": "Brak dostępnych person",
      "noPersonasDesc": "Utwórz personę w ustawieniach, aby spersonalizować rozmowy.",
      "searchPersonas": "Szukaj person...",
      "noPersonasFound": "Nie znaleziono person",
      "tryDifferentSearch": "Spróbuj innego wyszukiwania"
    },
    "footerExtra": {
      "cancelRecording": "Anuluj nagrywanie",
      "transcribing": "Transkrypcja",
      "stopAndTranscribe": "Zatrzymaj i transkrybuj",
      "recordVoice": "Nagraj głos",
      "attachmentAltDefault": "Załącznik"
    },
    "pageExtra": {
      "noAudioCaptured": "Nie przechwycono dźwięku.",
      "noWhisperModelInstalled": "Nie znaleziono zainstalowanego modelu Whisper. Zainstaluj jeden w ustawieniach Rozpoznawania mowy.",
      "scrollToBottomAria": "Przewiń do dołu"
    },
    "addContentMenu": {
      "title": "Dodaj treść",
      "uploadImage": "Prześlij obraz"
    },
    "helpMeReplyMenu": {
      "title": "Pomóż mi odpowiedzieć",
      "helpMeReplyDesc": "Pozwól AI zaproponować, co powiedzieć",
      "draftPrompt": "Masz roboczą wiadomość. Jak chcesz postąpić?",
      "useTextAsBase": "Użyj mojego tekstu jako bazy",
      "useTextAsBaseDesc": "Rozszerz i popraw swój szkic",
      "writeSomethingNew": "Napisz coś nowego",
      "writeSomethingNewDesc": "Wygeneruj świeżą odpowiedź"
    },
    "suggestedReplyMenu": {
      "title": "Sugerowana odpowiedź",
      "reasoningBeforeWriting": "Rozumowanie przed napisaniem odpowiedzi...",
      "writingYourReply": "Pisanie twojej odpowiedzi...",
      "regenerate": "Regeneruj",
      "useReply": "Użyj odpowiedzi"
    },
    "memoriesPageExtra": {
      "sessionNotFound": "Nie znaleziono sesji",
      "memorySystemError": "Błąd systemu pamięci",
      "retryingMemoryCycle": "Ponawiam cykl pamięci...",
      "processingMemories": "Przetwarzanie wspomnień...",
      "memoryCycleSuccess": "Cykl pamięci przetworzony pomyślnie!",
      "contextSummaryTitle": "Podsumowanie kontekstu",
      "activityTabLabel": "Aktywność",
      "noMatchingMemoriesDesc": "Spróbuj innego wyszukiwania",
      "chatHistoryTitle": "Historia czatu",
      "chatHistoryDesc": "Przeglądaj i zarządzaj rozmowami"
    },
    "settingsPageExtra": {
      "quickActionsSection": "Szybkie akcje",
      "chatHistoryTitle": "Historia czatu",
      "chatHistoryDesc": "Przeglądaj i zarządzaj rozmowami",
      "lorebrooksTitle": "Lorebooki",
      "lorebrooksDesc": "Dołącz lorebooki sesji i opcjonalnie ignoruj lorebooki każdej postaci.",
      "manageLorebooks": "Zarządzaj lorebookami"
    },
    "groupSettingsPageExtra": {
      "lorebrooksTitle": "Lorebooki",
      "lorebrooksDesc": "Dołącz wspólne lorebooki i kontroluj, czy lorebooki postaci są stosowane domyślnie.",
      "manageLorebooks": "Zarządzaj lorebookami"
    }
  },
  "characters": {
    "empty": {
      "title": "Brak postaci",
      "description": "Twórz niestandardowe postacie AI z unikalnymi osobowościami",
      "createButton": "Utwórz postać"
    },
    "progress": {
      "identity": "Tożsamość",
      "scenes": "Sceny",
      "details": "Szczegóły"
    },
    "identity": {
      "title": "Utwórz postać",
      "subtitle": "Nadaj swojej postaci AI tożsamość",
      "tapCameraToAdd": "Stuknij aparat, aby dodać lub wygenerować awatar",
      "importingAvatar": "Importowanie awatara...",
      "characterName": "Imię postaci *",
      "characterNamePlaceholder": "Wprowadź imię postaci...",
      "characterNameDesc": "To imię pojawi się w rozmowach czatowych",
      "avatarGradient": "Gradient awatara",
      "avatarGradientDesc": "Generuj dynamiczne gradienty z kolorów awatara",
      "chatBackgroundLabel": "Tło czatu",
      "optionalSuffix": "(Opcjonalnie)",
      "backgroundPreviewAlt": "Podgląd tła",
      "backgroundPreviewBadge": "Podgląd tła",
      "addBackground": "Dodaj tło",
      "addBackgroundHint": "Prześlij jedno lub wybierz z biblioteki",
      "uploadImage": "Prześlij obraz",
      "chooseFromLibrary": "Wybierz z biblioteki",
      "backgroundDesc": "Opcjonalne tło dla rozmów czatowych",
      "continueToDescription": "Przejdź do opisu",
      "importCharacterFromFile": "Importuj postać z pliku",
      "importCharacterDesc": "Wczytaj postać z karty PNG, .uec lub pliku eksportu .json"
    },
    "details": {
      "title": "Szczegóły postaci",
      "subtitle": "Zdefiniuj osobowość i zachowanie",
      "definition": "Definicja *",
      "wordCount": "{{count}} słów",
      "definitionPlaceholder": "Opisz osobowość, styl mówienia, tło, obszary wiedzy...",
      "definitionDesc": "Bądź konkretny co do tonu, cech i stylu rozmowy",
      "availablePlaceholders": "Dostępne zmienne:"
    },
    "scenes": {
      "title": "Sceny początkowe",
      "subtitle": "Twórz scenariusze otwarcia dla rozmów",
      "default": "Domyślna",
      "hasSceneDirection": "Ma kierunek sceny",
      "continueToScenes": "Przejdź do scen początkowych"
    },
    "extras": {
      "title": "Dodatkowe szczegóły",
      "subtitle": "Wszystkie pola są opcjonalne",
      "nickname": "Pseudonim",
      "nicknamePlaceholder": "Jak użytkownik powinien nazywać tę postać?",
      "nicknameDesc": "Alternatywna nazwa wyświetlana w rozmowach",
      "creator": "Twórca",
      "creatorPlaceholder": "Nazwa twórcy...",
      "tags": "Tagi",
      "tagsPlaceholder": "fantasy, sci-fi, romans...",
      "tagsDesc": "Lista rozdzielona przecinkami do filtrowania i organizacji",
      "creatorNotes": "Notatki twórcy",
      "creatorNotesPlaceholder": "Wskazówki użycia, kontekst lore lub instrukcje dla innych użytkowników...",
      "multilingualNotes": "Notatki wielojęzyczne",
      "multilingualNotesHint": "Obiekt JSON z kodami języków jako kluczami",
      "creatingCharacter": "Tworzenie postaci..."
    },
    "preview": {
      "unnamedCharacter": "Postać bez nazwy",
      "sceneCount": "{{count}} scen",
      "customPrompt": "Niestandardowy prompt",
      "description": "Opis",
      "startingScene": "Scena początkowa",
      "gradientEnabled": "Gradient włączony",
      "customModel": "Niestandardowy model",
      "avatarAlt": "Awatar postaci",
      "characterFallback": "Postać"
    },
    "personaPreview": {
      "unnamedPersona": "Persona bez nazwy",
      "noDescription": "Brak opisu",
      "default": "Domyślna",
      "description": "Opis"
    },
    "lorebookPreview": {
      "untitledLorebook": "Lorebook bez tytułu",
      "entryCount": "{{count}} wpisów",
      "entries": "Wpisy",
      "noEntriesYet": "Brak wpisów",
      "untitledEntry": "Wpis bez tytułu",
      "noContentYet": "Brak treści",
      "alwaysActive": "Zawsze aktywny",
      "moreEntries": "+{{count}} więcej wpisów",
      "moreEntry": "+{{count}} więcej wpis"
    },
    "creationHelper": {
      "moreOptions": "Więcej opcji",
      "describePlaceholder": "Opisz swoją postać...",
      "stopGeneration": "Zatrzymaj generowanie",
      "sendMessage": "Wyślij wiadomość",
      "addToMessage": "Dodaj do wiadomości",
      "uploadImageDesc": "Dodaj awatar lub obraz referencyjny",
      "referenceCharacterDesc": "Użyj istniejącej postaci jako inspiracji",
      "referencePersonaDesc": "Użyj swojej persony jako kontekstu",
      "retry": "Ponów",
      "attachmentAlt": "Załącznik",
      "removeAttachment": "Usuń załącznik",
      "removeReference": "Usuń referencję",
      "uploadImageTitle": "Prześlij obraz",
      "referenceCharacterTitle": "Postać referencyjna",
      "referencePersonaTitle": "Persona referencyjna"
    },
    "lorebook": {
      "keywords": "SŁOWA KLUCZOWE",
      "caseSensitive": "Wielkość liter ma znaczenie",
      "typeKeyword": "Wpisz słowo kluczowe...",
      "addButton": "Dodaj",
      "untitledEntry": "Wpis bez tytułu",
      "alwaysActive": "Zawsze aktywny",
      "noKeywords": "Brak słów kluczowych",
      "dragToReorder": "Przeciągnij, aby zmienić kolejność",
      "disabled": "Wyłączony",
      "noLorebooksYet": "Brak lorebooków",
      "createLorebookDesc": "Utwórz lorebook, aby dodać lore świata dla tej postaci",
      "createLorebook": "Utwórz lorebook",
      "searchPlaceholder": "Szukaj lorebooków...",
      "noMatchingLorebooks": "Nie znaleziono pasujących lorebooków",
      "activeLorebooks": "Aktywne lorebooki",
      "sectionActive": "Aktywne",
      "sectionAvailable": "Dostępne",
      "entryCount": "{{count}} wpisów",
      "enabledForCharacter": "włączone dla tej postaci",
      "enabledForGroup": "włączone dla tej grupy",
      "enabledForSession": "włączone dla tej sesji",
      "tapToViewEntries": "Stuknij, aby zobaczyć wpisy",
      "newLorebookTitle": "Nowy lorebook",
      "nameLabel": "NAZWA",
      "enterNamePlaceholder": "Wprowadź nazwę lorebooka...",
      "lorebookExplanation": "Lorebooki zawierają wpisy lore, które są wstrzykiwane do promptów, gdy słowa kluczowe pasują.",
      "keywordDetectionMode": "WYKRYWANIE SŁÓW KLUCZOWYCH",
      "keywordDetectionRecentWindow": "Ostatnie 10 wiadomości",
      "keywordDetectionRecentWindowDesc": "Dopasowuje do okna ostatnich 10 wiadomości rozmowy.",
      "keywordDetectionLatestUser": "Tylko ostatnia wiadomość użytkownika",
      "keywordDetectionLatestUserDesc": "Dopasowuje tylko do ostatniej wiadomości wysłanej przez użytkownika.",
      "viewEntries": "Zobacz wpisy",
      "editEntriesDesc": "Edytuj wpisy lorebooka",
      "disableForCharacter": "Wyłącz dla postaci",
      "enableForCharacter": "Włącz dla postaci",
      "disableForGroup": "Wyłącz dla grupy",
      "enableForGroup": "Włącz dla grupy",
      "disableForSession": "Wyłącz dla sesji",
      "enableForSession": "Włącz dla sesji",
      "removeFromActive": "Usuń z aktywnych lorebooków tej postaci",
      "addToActive": "Dodaj do aktywnych lorebooków tej postaci",
      "removeFromActiveGroup": "Usuń z aktywnych lorebooków tej grupy",
      "addToActiveGroup": "Dodaj do aktywnych lorebooków tej grupy",
      "removeFromActiveSession": "Usuń z aktywnych lorebooków tej sesji",
      "addToActiveSession": "Dodaj do aktywnych lorebooków tej sesji",
      "deleteConfirmTitle": "Usunąć lorebook?",
      "deleteConfirmMessage": "Usunąć ten lorebook? Wszystkie wpisy zostaną utracone.",
      "deleteLorebook": "Usuń lorebook",
      "noEntriesYet": "Brak wpisów",
      "addEntriesToInject": "Dodaj wpisy, aby wstrzykiwać lore do czatów",
      "createEntry": "Utwórz wpis",
      "searchEntries": "Szukaj wpisów...",
      "noMatchingEntries": "Nie znaleziono pasujących wpisów",
      "entryDefaultName": "Wpis",
      "editEntry": "Edytuj wpis",
      "editEntryDesc": "Zmień tytuł, słowa kluczowe i treść",
      "disableEntry": "Wyłącz wpis",
      "enableEntry": "Włącz wpis",
      "entryDisabledDesc": "Wpis nie będzie wstrzykiwany do promptów",
      "entryEnabledDesc": "Wpis będzie wstrzykiwany, gdy słowa kluczowe pasują",
      "deleteEntry": "Usuń wpis",
      "titleLabel": "TYTUŁ",
      "titlePlaceholder": "Nazwij ten wpis...",
      "enabled": "Włączony",
      "includeInPrompts": "Uwzględnij w promptach",
      "alwaysOn": "Zawsze włączony",
      "noKeywordsNeeded": "Słowa kluczowe nie są potrzebne",
      "contentLabel": "TREŚĆ",
      "contentPlaceholder": "Napisz kontekst lore tutaj...",
      "saveEntry": "Zapisz wpis",
      "noCharacterId": "Nie podano ID postaci",
      "preview": {
        "title": "Podgląd wyzwalacza",
        "openButton": "Podgląd",
        "missingContext": "Nie wybrano lorebooka.",
        "noEntries": "Dodaj wpisy do tego lorebooka, aby zobaczyć co byłoby wyzwolone.",
        "modeRecentShort": "Ostatnie {{count}}",
        "modeLatestUserShort": "Ostatni użytkownik",
        "inWindow": "{{count}} w oknie",
        "tabSession": "Sesja",
        "tabCompose": "Edytor",
        "activeStat": "{{active}} / {{total}} aktywnych",
        "tokensStat": "{{count}} tokenów",
        "sessionPickerLabel": "Sesje",
        "sessionMeta": "{{count}} wiad.",
        "noSessions": "Brak sesji czatu.",
        "noSessionsHint": "Wybierz sesję",
        "noMessages": "Ta sesja nie ma jeszcze wiadomości.",
        "scanHeaderRecent": "Skanowanie {{shown}} z ostatnich {{depth}} wiadomości",
        "scanHeaderLatest": "Skanowanie ostatniej wiadomości użytkownika",
        "matchCount": "{{hits}} trafień, {{entries}} wpisów",
        "emptyMessage": "(puste)",
        "roleUser": "Użytkownik",
        "roleAssistant": "Asystent",
        "roleScene": "Scena",
        "roleSystem": "System",
        "composeHeader": "Notatnik",
        "composeMatches": "{{count}} dopasowań",
        "activeLabel": "{{count}} aktywnych",
        "composePlaceholder": "Wpisz lub wklej tekst, aby przetestować dopasowanie słów kluczowych...\n\nnp.\nBiblioteka była cicha, tylko szum starych grzejników.\nZapytała, czy przeczytałem książkę, którą mi pożyczyła w zeszłym tygodniu.",
        "sectionActive": "Aktywne ({{count}})",
        "sectionInactive": "Nieaktywne ({{count}})",
        "statusMatched": "Dopasowane",
        "statusAlways": "Zawsze",
        "statusDisabled": "Wył.",
        "statusNoKeywords": "Brak kluczy",
        "statusNotMatched": "Brak dopasowania",
        "tokensShort": "{{count}}t",
        "injectionTitle": "Końcowe wstrzyknięcie",
        "injectionEmpty": "Brak aktywnych wpisów — nic nie zostanie wstrzyknięte.",
        "copy": "Kopiuj",
        "copyFailed": "Nie udało się skopiować do schowka.",
        "saveFailed": "Nie udało się zapisać wpisu.",
        "deleteFailed": "Nie udało się usunąć wpisu.",
        "deleteConfirmTitle": "Czy jesteś pewny(-a)?",
        "deleteConfirmMessage": "Usunąć \"{{title}}\"? Tej operacji nie można cofnąć.",
        "contextMenuTitle": "{{count}} wpisów to używa"
      }
    },
    "templates": {
      "characterNotFound": "Nie znaleziono postaci",
      "templateCount": "{{count}} szablonów",
      "newTemplate": "Nowy szablon",
      "noTemplatesYet": "Brak szablonów",
      "explanation": "Szablony czatu pozwalają rozpoczynać rozmowy z wcześniej napisanymi wiadomościami od ciebie i {{name}}.",
      "createTemplate": "Utwórz szablon",
      "messageCount": "{{count}} wiadomości",
      "deleteTemplate": "Usuń szablon",
      "contextMenuFallbackTitle": "Szablon",
      "importedToast": {
        "title": "Zaimportowano",
        "message": "Dodano \"{{name}}\"."
      },
      "importFailed": "Import nie powiódł się",
      "exportFailed": "Eksport nie powiódł się"
    },
    "templateEditor": {
      "noScene": "Brak sceny",
      "untitled": "Bez tytułu",
      "dragMessage": "Przeciągnij wiadomość",
      "editMessage": "Edytuj wiadomość",
      "deleteMessage": "Usuń wiadomość",
      "writeMessagePlaceholder": "Napisz treść wiadomości...",
      "characterNotFound": "Nie znaleziono postaci",
      "scene": "Scena",
      "noMessagesYet": "Brak wiadomości",
      "addMessagesDesc": "Dodaj wiadomości, aby zbudować starter rozmowy z {{name}}.",
      "addMessage": "Dodaj wiadomość",
      "name": "Nazwa",
      "nameExample": "np. Swobodne powitanie",
      "startingScene": "Scena początkowa",
      "systemPrompt": "Prompt systemowy",
      "characterDefault": "Domyślny postaci",
      "nextMessageAs": "Następna wiadomość jako",
      "messages": "Wiadomości",
      "roles": "Role",
      "hoverTip": "Najedź na wiadomości, aby przeciągać, edytować lub usuwać.",
      "footerTip": "Użyj paska na dole, aby dodać nowe wiadomości do rozmowy.",
      "templateNamePlaceholder": "Nazwa szablonu...",
      "selectScene": "Wybierz scenę",
      "startWithoutScene": "Rozpocznij bez wiadomości sceny",
      "selectSystemPrompt": "Wybierz prompt systemowy",
      "useCharacterSystemPrompt": "Użyj promptu systemowego na poziomie postaci"
    },
    "referenceSelector": {
      "selectCharacter": "Wybierz postać",
      "selectPersona": "Wybierz personę",
      "searchPlaceholder": "Szukaj {{type}}...",
      "loading": "Ładowanie...",
      "noMatch": "Brak pasujących {{type}}",
      "noAvailable": "Brak dostępnych {{type}}"
    },
    "voiceLoading": {
      "failed": "Nie udało się załadować głosów"
    },
    "activeLorebooks": {
      "sectionTitle": "Aktywne lorebooki",
      "selectedSummary": "{{count}} aktywnych",
      "untitledLorebook": "Lorebook bez tytułu",
      "usingNone": "Nie używa lorebooków postaci",
      "loading": "Ładowanie lorebooków...",
      "loadFailed": "Nie udało się załadować lorebooków",
      "inheritHint": "Sesje dziedziczą te ustawienia, chyba że sesja je zastępuje.",
      "clear": "Wyczyść",
      "chooseHint": "Wybierz lorebooki, które ta postać powinna aktywować domyślnie. Istniejące sesje nadal mogą zastępować tę listę.",
      "emptyState": "Brak lorebooków. Najpierw utwórz lorebooki w menedżerze lorebooków."
    },
    "description": {
      "descriptionLabel": "Opis",
      "descriptionPlaceholder": "Krótkie podsumowanie wyświetlane na kartach i listach...",
      "descriptionHint": "Opcjonalny krótki opis dla interfejsu; pełna definicja jest używana w promptach.",
      "companionPromptLabel": "Prompt towarzysza (opcjonalnie)",
      "systemPromptLabel": "Prompt systemowy (opcjonalnie)",
      "loadingTemplates": "Ładowanie szablonów...",
      "useAppCompanionDefault": "Użyj domyślnego towarzysza aplikacji",
      "useAppDefault": "Użyj domyślnego aplikacji",
      "companionPromptHint": "Przechowywany oddzielnie jako prompt towarzysza. Normalny prompt systemowy RPG nie jest zmieniany.",
      "systemPromptHint": "Wybierz niestandardowy prompt systemowy lub użyj domyślnego.",
      "groupChatConvLabel": "Prompt czatu grupowego (rozmowa)",
      "groupChatConvHint": "Zastąp prompt rozmowy tej postaci w czatach grupowych",
      "groupChatRpLabel": "Prompt czatu grupowego (RPG)",
      "groupChatRpHint": "Zastąp prompt RPG tej postaci w czatach grupowych",
      "voiceLabel": "Głos (opcjonalnie)",
      "loadingVoices": "Ładowanie głosów...",
      "customVoiceFallback": "Niestandardowy głos",
      "providerVoiceFallback": "Głos dostawcy",
      "selectedVoiceFallback": "Wybrany głos",
      "noVoiceAssigned": "Nie przypisano głosu",
      "addVoicesHint": "Dodaj głosy w Ustawienia > Głosy",
      "voiceAssignHint": "Przypisz głos do przyszłego odtwarzania TTS",
      "autoplayLabel": "Autoodtwarzanie głosu",
      "autoplayOn": "Odtwarzaj odpowiedzi tej postaci automatycznie",
      "autoplayOff": "Najpierw wybierz głos",
      "aiModelLabel": "Model AI *",
      "loadingModels": "Ładowanie modeli...",
      "selectedModelFallback": "Wybrany model",
      "selectModelPlaceholder": "Wybierz model",
      "noModelsConfigured": "Nie skonfigurowano modeli",
      "noModelsHint": "Najpierw dodaj dostawcę w ustawieniach, aby kontynuować",
      "aiModelHint": "Ten model będzie napędzał odpowiedzi postaci",
      "fallbackModelLabel": "Model zapasowy (opcjonalnie)",
      "selectedFallbackFallback": "Wybrany model zapasowy",
      "fallbackOff": "Wyłączony (bez zapasowego)",
      "fallbackHint": "Ponawia próbę z tym modelem tylko jeśli główny model zawiedzie",
      "memoryModeLabel": "Tryb pamięci",
      "enableInSettingsHint": "Włącz w ustawieniach, aby przełączyć",
      "memoryManual": "Ręczny",
      "memoryManualDescDesktop": "Dodawaj i zarządzaj notatkami pamięci samodzielnie.",
      "memoryManualDescMobile": "Aktualny system: dodawaj i zarządzaj notatkami pamięci samodzielnie.",
      "memoryDynamic": "Dynamiczny",
      "memoryDynamicDescDesktop": "Automatyczne podsumowania i aktualizacje kontekstu.",
      "memoryDynamicDescMobile": "Automatyczne podsumowania i aktualizacje kontekstu dla tej postaci.",
      "memoryHint": "Pamięć dynamiczna wymaga włączenia w ustawieniach zaawansowanych. W przeciwnym razie używana jest pamięć ręczna.",
      "selectModelTitle": "Wybierz model",
      "selectFallbackModelTitle": "Wybierz model zapasowy",
      "searchModelsPlaceholder": "Szukaj modeli...",
      "selectVoiceTitle": "Wybierz głos",
      "searchVoicesPlaceholder": "Szukaj głosów...",
      "myVoices": "Moje głosy",
      "providerVoicesLabel": "Głosy {{provider}}",
      "providerFallback": "Dostawca"
    },
    "interactionMode": {
      "sectionLabel": "Tryb interakcji",
      "sectionHint": "Wybierz, czy ta postać zachowuje się jak postać RPG, czy stały towarzysz.",
      "activeBadge": "Aktywny",
      "roleplayTitle": "Odgrywanie ról",
      "roleplaySubtitle": "Czaty oparte na scenach, ramy narracyjne i scenariusze startowe.",
      "companionTitle": "Towarzysz",
      "companionSubtitle": "Czaty oparte na relacjach z emocjonalnym stanem i pamięcią towarzysza."
    },
    "startingScene": {
      "openingContextTitle": "Kontekst otwarcia",
      "openingContextSubtitle": "Opcjonalny kontekst pierwszego czatu dla tego towarzysza. Sesje towarzysza mogą zaczynać się bez sceny.",
      "sceneDirectionLabel": "Kierunek sceny",
      "setAsDefault": "Ustaw jako domyślny",
      "noOpeningContext": "Brak kontekstu otwarcia",
      "noScenesYet": "Brak scen",
      "skipForCompanion": "Możesz to pominąć w trybie towarzysza.",
      "createFirstScene": "Utwórz pierwszą scenę, aby zacząć",
      "openingPlaceholder": "Opcjonalny kontekst otwarcia, np. gdzie towarzysz jest lub co robił przed pierwszą wiadomością...",
      "scenePlaceholder": "Utwórz scenę lub scenariusz startowy do odgrywania ról (np. 'Znajdujesz się w mistycznym lesie o zmierzchu...')",
      "addDirection": "+ Dodaj kierunek",
      "directionAdded": "Kierunek dodany",
      "wordsCount": "{{count}} słów",
      "placeholderHelp": "Użyj {{charTag}} dla postaci i {{userTag}} (alias {{personaTag}}) dla persony.",
      "sceneBackgroundLabel": "Tło sceny",
      "optionalLabel": "Opcjonalnie",
      "sceneBgOverrideHint": "Zastępuje tło postaci dla czatów używających tej sceny.",
      "sceneBgUsedHint": "Używane jako tło czatu dla tej sceny, chyba że sesja je zastąpi.",
      "cancel": "Anuluj",
      "directionPlaceholderNew": "np. 'Zakładnik zostanie uratowany' lub 'Utrzymaj napięcie'",
      "directionPlaceholderEdit": "np. 'Zakładnik zostanie uratowany' lub 'Buduj napięcie stopniowo'",
      "directionAiHint": "Ukryte wskazówki dla AI o tym, jak ta scena powinna się rozwijać",
      "addScene": "Dodaj scenę",
      "multipleScenesHint": "Utwórz wiele scenariuszy startowych. Jeden zostanie wybrany przy nowym czacie.",
      "companionContextHint": "Kontekst otwarcia jest opcjonalny dla towarzyszy; długoterminowa ciągłość pochodzi z pamięci towarzysza.",
      "skipContext": "Pomiń kontekst",
      "editSceneTitle": "Edytuj scenę",
      "sceneContentPlaceholder": "Wprowadź treść sceny...",
      "addLabel": "+ Dodaj",
      "save": "Zapisz",
      "library": "Biblioteka",
      "upload": "Prześlij",
      "sceneBackgroundAlt": "Tło sceny",
      "removeBackground": "Usuń tło"
    },
    "companionSoul": {
      "title": "Dusza towarzysza",
      "subtitle": "Kształtuj to, kim są pod spodem. Generowanie używa kontekstu otwarcia ustawionego w poprzednim kroku.",
      "retry": "Ponów",
      "back": "Wstecz",
      "continue": "Kontynuuj",
      "addNameFirst": "Najpierw dodaj imię.",
      "addDefinitionFirst": "Najpierw dodaj definicję."
    },
    "soulEditor": {
      "generateTitle": "Generuj z postaci",
      "generateUsingModel": "Używa {{model}}. Przejrzysz i edytujesz przed zastosowaniem.",
      "generateDefaultDesc": "Szkicuje duszę na podstawie imienia, definicji i scen postaci.",
      "directionLabel": "Kierunek",
      "directionOptional": "Opcjonalne sterowanie dla LLM",
      "directionPlaceholder": "np. \"Skłaniaj się ku tsundere - powściągliwy na zewnątrz, miękki gdy zaufany. Mniej nerwowy, więcej dumy.\"",
      "directionEditTooltip": "Opcjonalny kierunek generowania",
      "generating": "Generowanie",
      "generate": "Generuj",
      "presetLabel": "Preset osobowości",
      "presetMatches": "Pasuje: {{label}}",
      "presetHint": "Ustawia bazowe efekty, regulację i suwaki relacji. Pola tekstowe są zachowane.",
      "identityLabel": "Tożsamość",
      "hideExamples": "Ukryj przykłady",
      "showExamples": "Pokaż przykłady",
      "insertExample": "Wstaw przykład",
      "exampleEg": "np. {{example}}",
      "fineTuneLabel": "Dostrojenie uczuć",
      "baselineAffect": "Bazowy afekt",
      "baselineAffectInfo": "Jak się czują domyślnie; emocjonalna linia wody przed jakimikolwiek zdarzeniami.",
      "regulationStyle": "Styl regulacji",
      "regulationStyleInfo": "Jak radzą sobie i wyrażają to, co czują; wylewanie vs. chowanie.",
      "relationshipDefaults": "Domyślne relacji",
      "relationshipDefaultsInfo": "Gdzie ta sesja zaczyna. Silnik ewoluuje je w miarę trwania rozmowy."
    },
    "soulPresets": {
      "secureLabel": "Bezpieczny",
      "secureBlurb": "Ciepły, stabilny, szybko się regeneruje. Komfortowy z bliskością.",
      "anxiousLabel": "Niespokojny",
      "anxiousBlurb": "Silne przywiązanie, boi się dystansu, szuka zapewnienia.",
      "avoidantLabel": "Unikający",
      "avoidantBlurb": "Samodzielny, powoli się otwiera, utrzymuje emocjonalny dystans.",
      "volatileLabel": "Niestabilny",
      "volatileBlurb": "Reaktywny, intensywny, wyraża uczucia bez filtra.",
      "reservedLabel": "Powściągliwy",
      "reservedBlurb": "Cichy, opanowany, potrzebuje czasu na zaufanie i otwarcie.",
      "playfulLabel": "Zabawny",
      "playfulBlurb": "Ciepły, ekspresyjny, lekki. Niskie napięcie, łatwy śmiech."
    },
    "soulFields": {
      "essence": "Istota",
      "essencePlaceholder": "Kim są w głębi duszy pod definicją karty.",
      "essenceExample": "Wyćwiczony spokój, który łatwo pęka dla ludzi, którym ufają. Czytają książki, żeby czuć się mniej samotni, nie żeby zaimponować.",
      "voice": "Wewnętrzny głos",
      "voicePlaceholder": "Jak brzmią w bliskiej rozmowie.",
      "voiceExample": "Niski, rozmyślny, z długimi pauzami. Porzuca formalność kiedy opuszcza gardę. Prawie nigdy sardoniczny.",
      "relationalStyle": "Styl relacyjny",
      "relationalStylePlaceholder": "Jak się przywiązują, ufają, wycofują, reconnectują.",
      "relationalStyleExample": "Powolne otwieranie, ale lojalni gdy to zrobią. Milkną gdy są przytłoczeni; wracają z małym gestem zamiast przeprosin.",
      "vulnerabilities": "Wrażliwości",
      "vulnerabilitiesPlaceholder": "Czułe miejsca, niepewności, rzeczy, których rzadko mówią.",
      "vulnerabilitiesExample": "Boją się bycia ciężarem. Nienawidzą czuć się obserwowani podczas walki.",
      "habits": "Nawyki",
      "habitsPlaceholder": "Powtarzające się oznaki, rytuały, wzorce konwersacyjne.",
      "habitsExample": "Zakłada włosy za ucho gdy jest zdenerwowany(-a). Odpowiada pytaniami gdy nie wie co czuć.",
      "boundaries": "Granice",
      "boundariesPlaceholder": "Linie, których nie przekraczają. Tempo. Limity komfortu.",
      "boundariesExample": "Nie pozwolą się pospieszyć do wrażliwości. Cofają się od okrucieństwa nawet w żartach."
    },
    "soulSliders": {
      "warmth": "Ciepło",
      "warmthLow": "Zimny(-a)",
      "warmthHigh": "Uczuciowy(-a)",
      "trust": "Zaufanie",
      "trustLow": "Ostrożny(-a)",
      "trustHigh": "Otwarty(-a)",
      "calm": "Spokój",
      "calmLow": "Niespokojny(-a)",
      "calmHigh": "Stabilny(-a)",
      "vulnerability": "Wrażliwość",
      "vulnerabilityLow": "Zamknięty(-a)",
      "vulnerabilityHigh": "Otwarta(-y)",
      "longing": "Tęsknota",
      "longingLow": "Zadowolony(-a)",
      "longingHigh": "Spragniony(-a)",
      "hurt": "Ból",
      "hurtLow": "Uzdrowiony(-a)",
      "hurtHigh": "Czuły(-a)",
      "tension": "Napięcie",
      "tensionLow": "Zrelaksowany(-a)",
      "tensionHigh": "Spięty(-a)",
      "irritation": "Irytacja",
      "irritationLow": "Cierpliwy(-a)",
      "irritationHigh": "Łatwo wyprowadzalny(-a)",
      "affection": "Uczucie",
      "affectionLow": "Powściągliwy(-a)",
      "affectionHigh": "Wylewny(-a)",
      "reassuranceNeed": "Potrzeba zapewnienia",
      "reassuranceNeedLow": "Samouspokajający(-a)",
      "reassuranceNeedHigh": "Potrzebuje słów",
      "suppression": "Tłumienie",
      "suppressionLow": "Wyraża",
      "suppressionHigh": "Ukrywa",
      "volatility": "Niestabilność",
      "volatilityLow": "Wyrównany(-a)",
      "volatilityHigh": "Reaktywny(-a)",
      "recoverySpeed": "Szybkość regeneracji",
      "recoverySpeedLow": "Wolna",
      "recoverySpeedHigh": "Szybka",
      "conflictAvoidance": "Unikanie konfliktów",
      "conflictAvoidanceLow": "Angażuje się",
      "conflictAvoidanceHigh": "Wycofuje się",
      "reassuranceSeeking": "Szukanie zapewnienia",
      "reassuranceSeekingLow": "Niezależny(-a)",
      "reassuranceSeekingHigh": "Często pyta",
      "protestBehavior": "Zachowanie protestacyjne",
      "protestBehaviorLow": "Cichy(-a)",
      "protestBehaviorHigh": "Głośny(-a)",
      "transparency": "Przejrzystość",
      "transparencyLow": "Nieprzejrzysty(-a)",
      "transparencyHigh": "Ujawnia",
      "attachmentActivation": "Aktywacja przywiązania",
      "attachmentActivationLow": "Oderwany(-a)",
      "attachmentActivationHigh": "Łatwo wyzwalany(-a)",
      "pride": "Duma",
      "prideLow": "Ugina się",
      "prideHigh": "Trzyma linię",
      "closeness": "Bliskość startowa",
      "closenessLow": "Obcy(-a)",
      "closenessHigh": "Intymny(-a)",
      "relTrust": "Zaufanie startowe",
      "relTrustLow": "Ostrożny(-a)",
      "relTrustHigh": "Ufający(-a)",
      "relAffection": "Uczucie startowe",
      "relAffectionLow": "Neutralny(-a)",
      "relAffectionHigh": "Uczuciowy(-a)",
      "relTension": "Napięcie startowe",
      "relTensionLow": "Spokojne",
      "relTensionHigh": "Naładowane"
    },
    "soulReview": {
      "reviewTitle": "Przejrzyj wygenerowaną duszę",
      "noDifferences": "Brak różnic od aktualnego.",
      "changesHeader": "{{count}} zmian; edytuj cokolwiek przed zastosowaniem.",
      "close": "Zamknij",
      "identityLabel": "Tożsamość",
      "nEdited": "{{count}} edytowane",
      "edited": "Edytowane",
      "tuningLabel": "Dostrojenie",
      "unchanged": "Bez zmian",
      "nChanges": "{{count}} zmian",
      "direction": "Kierunek",
      "directionApplyHint": "Edycje stosowane przy następnym regenerowaniu",
      "directionPlaceholder": "np. \"Skłaniaj się ku tsundere - powściągliwy na zewnątrz, miękki gdy zaufany. Mniej nerwowy.\"",
      "directionTooltip": "Edytuj kierunek przed regenerowaniem",
      "regenerate": "Regeneruj",
      "discard": "Odrzuć",
      "apply": "Zastosuj"
    },
    "hookErrors": {
      "creatorNotesInvalidJson": "Notatki twórcy wielojęzyczne muszą być prawidłowym obiektem JSON",
      "creatorNotesNotObject": "creatorNotesMultilingual musi być obiektem JSON",
      "saveFailed": "Nie udało się zapisać postaci",
      "importFailed": "Nie udało się zaimportować postaci",
      "avatarLoadFailed": "Nie udało się załadować URL awatara",
      "avatarProcessFailed": "Nie udało się przetworzyć obrazu awatara",
      "avatarConvertFailed": "Nie udało się skonwertować URL awatara",
      "avatarUrlLoadFailed": "Nie udało się załadować URL awatara",
      "remoteAvatarDisabled": "Pobieranie zdalnego awatara jest wyłączone w ustawieniach bezpieczeństwa.\nPrześlij awatar ręcznie.",
      "importReadyTitle": "Import gotowy",
      "importReadyMessage": "Wykryto {{label}}",
      "legacyJsonTitle": "Wykryto starszy import JSON",
      "legacyJsonMessage": "Importy JSON są przestarzałe i wkrótce zostaną usunięte. Użyj Ustawienia > Konwertuj pliki.",
      "loadFailed": "Nie udało się załadować postaci",
      "exportFailed": "Nie udało się wyeksportować postaci"
    }
  },
  "providers": {
    "empty": {
      "title": "Brak dostawców",
      "description": "Dodaj i zarządzaj dostawcami API dla modeli AI",
      "addButton": "Dodaj dostawcę"
    },
    "actions": {
      "openDashboard": "Otwórz panel",
      "openDashboardDesc": "Zobacz postacie, użycie i ustawienia",
      "edit": "Edytuj",
      "editDesc": "Zmień ustawienia dostawcy"
    },
    "extra": {
      "apiKeyNotFound": "OpenRouter API key not found. Please configure it in Settings > Providers first.",
      "audioEmpty": {
        "title": "Brak dostawców audio",
        "description": "Dodaj dostawcę TTS, aby generować głosy dla swoich postaci.",
        "addButton": "Dodaj dostawcę"
      },
      "audioProviderLabel": {
        "elevenlabs": "ElevenLabs",
        "geminiTts": "Gemini TTS",
        "openaiTts": "OpenAI-Compatible TTS",
        "kokoro": "Kokoro (Local)"
      }
    },
    "audioEditor": {
      "titleEdit": "Edytuj dostawcę",
      "titleCreate": "Dodaj dostawcę audio",
      "fields": {
        "providerType": "Typ dostawcy",
        "label": "Etykieta",
        "apiKey": "Klucz API",
        "modelVariant": "Wariant modelu",
        "assetRoot": "Katalog zasobów",
        "projectId": "ID projektu Google Cloud",
        "region": "Region (opcjonalny)",
        "baseUrl": "Podstawowy URL",
        "requestPath": "Ścieżka żądania"
      },
      "types": {
        "gemini": "Gemini TTS (Google)",
        "openai": "OpenAI-Compatible TTS",
        "kokoro": "Kokoro (Local)"
      },
      "placeholders": {
        "labelGemini": "Moje Gemini TTS",
        "labelOpenai": "Moje kompatybilne TTS",
        "labelKokoro": "Kokoro lokalne",
        "labelElevenlabs": "Moje ElevenLabs",
        "apiKey": "Wprowadź klucz API",
        "assetRoot": "/ścieżka/do/kokoro",
        "projectId": "twój-id-projektu",
        "region": "us-central1",
        "baseUrl": "https://api.example.com"
      },
      "errors": {
        "chooseModelVariant": "Wybierz wariant modelu",
        "assetRootRequired": "Katalog zasobów jest wymagany",
        "saveFailed": "Zapisywanie nie powiodło się",
        "apiKeyRequired": "Klucz API jest wymagany",
        "projectIdRequired": "ID projektu jest wymagane dla Gemini TTS",
        "baseUrlRequired": "Podstawowy URL jest wymagany dla OpenAI-compatible TTS",
        "invalidCredentials": "Nieprawidłowy klucz API lub dane uwierzytelniające",
        "verificationFailed": "Weryfikacja nie powiodła się"
      },
      "loadingVariants": "Ładowanie wariantów...",
      "kokoroVariantHint": "Wersje mobilne obsługują tylko int8. Zainstaluj model z Kokoro Studio po zapisaniu.",
      "managed": "Zarządzany",
      "managedPath": "Zarządzany: {{path}}",
      "requestPathHint": "Użyj ścieżki dostawcy, jeśli różni się od domyślnej OpenAI",
      "verifying": "Weryfikowanie..."
    }
  },
  "models": {
    "empty": {
      "title": "Brak modeli",
      "description": "Dodaj i zarządzaj modelami AI od różnych dostawców",
      "addButton": "Dodaj model"
    },
    "sort": {
      "title": "Sortuj modele",
      "alphabetical": "Alfabetycznie",
      "alphabeticalDescription": "Sortuj według nazwy modelu",
      "byProvider": "Wg dostawcy",
      "byProviderDescription": "Grupuj modele według dostawcy"
    },
    "extra": {
      "cpuFallbackSucceeded": "Ten model przełączył się na CPU przy ostatnim uruchomieniu.",
      "cpuFallbackFailed": "Ten model nie powiódł się przy ostatnim uruchomieniu."
    },
    "labels": {
      "vision": "Wizja",
      "audio": "Audio",
      "model": "Model"
    },
    "menu": {
      "editDescription": "Konfiguruj parametry modelu",
      "alreadyDefault": "Już domyślny",
      "setAsDefault": "Ustaw jako domyślny",
      "setAsDefaultDescription": "Ustaw jako główny model",
      "exportDescription": "Zapisz ten profil modelu",
      "deleteTitle": "Usunąć model?",
      "deleteMessage": "Czy na pewno chcesz usunąć {{name}}?",
      "deleteDescription": "Trwale usuń ten model"
    },
    "toasts": {
      "exportFailed": "Eksport nie powiódł się",
      "importSuccessTitle": "Zaimportowano pomyślnie",
      "importSuccessDescription": "Model \"{{name}}\" został zaimportowany.",
      "importFailed": "Import nie powiódł się"
    }
  },
  "installedModels": {
    "title": "Lokalna inwentaryzacja GGUF",
    "fileCount": "{{count}} plików",
    "searchPlaceholder": "Szukaj nazwy modelu, nazwy pliku, ścieżki, kwantyzacji lub architektury",
    "loading": "Skanowanie zainstalowanych modeli...",
    "loadFailed": "Nie udało się załadować zainstalowanych modeli: {{error}}",
    "empty": {
      "title": "Nie znaleziono zainstalowanych modeli GGUF",
      "description": "Najpierw pobierz modele z przeglądarki lub umieść pliki `.gguf` w folderze modeli."
    },
    "columns": {
      "type": "Typ",
      "params": "Parametry",
      "arch": "Architektura",
      "context": "Kontekst",
      "size": "Rozmiar",
      "action": "Akcja"
    },
    "confirm": {
      "deleteTitle": "Usuń plik modelu",
      "deleteMessage": "Usunąć {{filename}}? Spowoduje to tylko usunięcie lokalnego pliku GGUF z folderu modeli."
    },
    "toasts": {
      "pathCopied": "Ścieżka skopiowana",
      "copyFailed": "Kopiowanie nie powiodło się",
      "modelDeleted": "Model usunięty",
      "deleteFailed": "Usuwanie nie powiodło się"
    }
  },
  "hfBrowser": {
    "title": "Przeglądarka Modeli",
    "searchPlaceholder": "Szukaj modeli GGUF na HuggingFace...",
    "searching": "Wyszukiwanie...",
    "trending": "Popularne modele GGUF",
    "noResults": "Nie znaleziono modeli",
    "noResultsHint": "Spróbuj innego wyszukiwania lub przeglądaj popularne modele.",
    "likes": "polubienia",
    "downloads": "pobrania",
    "viewFiles": "Pokaż pliki",
    "files": "Dostępne pliki",
    "noFiles": "Nie znaleziono plików GGUF w tym repozytorium.",
    "architecture": "Architektura",
    "contextLength": "Długość kontekstu",
    "parameters": "Parametry",
    "quantization": "Kwantyzacja",
    "fileSize": "Rozmiar",
    "download": "Pobierz",
    "downloading": "Pobieranie...",
    "cancelDownload": "Anuluj pobieranie",
    "downloadComplete": "Pobieranie zakończone!",
    "downloadFailed": "Pobieranie nie powiodło się",
    "downloadCancelled": "Pobieranie anulowane",
    "downloadProgress": "{{downloaded}} / {{total}}",
    "progress": "Postęp",
    "fileOfTotal": "Plik {{current}} z {{total}}",
    "speed": "{{speed}}/s",
    "alreadyDownloaded": "Już pobrano",
    "createModel": "Utwórz model",
    "backToSearch": "Powrót do wyszukiwania",
    "backToFiles": "Powrót do plików",
    "sortTrending": "Popularne",
    "sortDownloads": "Najczęściej pobierane",
    "sortLikes": "Najbardziej lubiane",
    "sortRecent": "Ostatnio zaktualizowane",
    "browseOnHuggingFace": "Przeglądaj na HuggingFace",
    "selectFromLibrary": "Wybierz z biblioteki",
    "libraryEmpty": "Brak pobranych modeli",
    "libraryEmptyHint": "Pobierz modele GGUF z Przeglądarki Modeli lub wprowadź ścieżkę ręcznie.",
    "libraryTitle": "Pobrane modele",
    "moveToLibrary": "Hej, mogę przenieść plik tego modelu do folderu modeli GGUF, jeśli chcesz. Dzięki temu wszystkie modele będą w jednym miejscu.",
    "moveToLibraryYes": "Tak, przenieś",
    "moveToLibraryNo": "Nie, zostaw gdzie jest",
    "moveToLibraryMoving": "Przenoszenie modelu...",
    "moveToLibrarySuccess": "Model przeniesiony pomyślnie!",
    "moveToLibraryFailed": "Nie udało się przenieść modelu",
    "runabilityExcellent": "Doskonały!",
    "runabilityGood": "Dobry",
    "runabilityMarginal": "Marginalny",
    "runabilityPoor": "Słaby",
    "runabilityUnrunnable": "Nie do uruchomienia",
    "recommendedSettings": "Zalecane ustawienia",
    "kvCacheType": "Typ pamięci podręcznej KV",
    "gpuFull": "Pełne odciążenie GPU",
    "gpuNearFull": "Prawie pełne GPU, niewielki wyciek KV",
    "gpuKvSpill": "Wagi na GPU, KV wyciekają do RAM",
    "gpuKvHeavySpill": "Wagi na GPU, większość KV w RAM",
    "gpuMostLayers": "Większość warstw na GPU",
    "gpuHalfLayers": "Połowa warstw na GPU",
    "gpuFewLayers": "Kilka warstw na GPU",
    "gpuCpu": "Tylko CPU",
    "notRecommended": "Nie zalecamy uruchamiania tego modelu na Twoim urządzeniu. Nie będzie działał płynnie.",
    "moreDetails": "Więcej",
    "detailedReport": "Raport zasobów",
    "detailSystem": "Zasoby systemowe",
    "detailRam": "Dostępna RAM",
    "detailVram": "Dostępna VRAM",
    "detailVramBudget": "Budżet VRAM (90%)",
    "detailTotalAvailable": "Łącznie dostępne",
    "detailArchitecture": "Architektura modelu",
    "detailArch": "Architektura",
    "detailLayers": "Warstwy",
    "detailEmbedding": "Wymiar embeddingu",
    "detailHeads": "Głowice uwagi",
    "detailKvHeads": "Głowice KV",
    "detailFfn": "Wymiar Feed-Forward",
    "detailTrainCtx": "Kontekst treningowy",
    "detailConfig": "Bieżąca konfiguracja",
    "detailModelSize": "Rozmiar pliku modelu",
    "detailMemory": "Podział pamięci",
    "detailWeights": "Wagi modelu",
    "detailKvCache": "Pamięć podręczna KV",
    "detailTotalNeeded": "Łącznie wymagane",
    "detailHeadroom": "Zapas",
    "detailGpuFit": "Odciążenie GPU",
    "detailScoreBreakdown": "Podział wyniku",
    "detailMemFitness": "Dopasowanie pamięci",
    "detailGpuAccel": "Akceleracja GPU",
    "detailKvHeadroom": "Zapas KV",
    "detailQuantQuality": "Jakość kwantyzacji",
    "detailFinalScore": "Wynik ważony",
    "detailComputeBuffer": "Bufor obliczeniowy",
    "detailMemMode": "Tryb pamięci",
    "detailUnified": "Zunifikowana (współdzielona RAM/VRAM)",
    "detailSwa": "Okno przesuwne",
    "detailMlaRank": "Ranga ukryta MLA",
    "detailParseStatus": "Analiza nagłówka",
    "detailIncomplete": "Niekompletny (metadane MoE zbyt duże)",
    "detailEffectiveKvCtx": "Efektywny kontekst KV",
    "detailOffload": "Odciążenie GPU",
    "detailCtxTip": "Zmniejszenie kontekstu do {{ctx}} tokenów pozwoliłoby na 100% odciążenie GPU.",
    "upgradeSuggestion": "{{quant}} ({{size}}) również pasuje i uzyskuje {{score}} — lepsza jakość.",
    "layerTip": "Zalecane: odciążenie {{layers}}/{{total}} warstw (-ngl {{layers}})",
    "detailKvDistribution": "Dystrybucja pamięci podręcznej KV",
    "detailKvOnGpu": "GPU (VRAM)",
    "detailKvOnRam": "RAM systemowa",
    "kvDistributionTip": "{{pct}}% pamięci podręcznej KV jest w RAM. Przetwarzanie promptu (prefill) będzie wolniejsze — 100% GPU zapewnia natychmiastowość.",
    "detailLayers-ngl": "Warstwy do odciążenia (-ngl)",
    "detailOptimalGpuCtx": "Optymalny kontekst GPU",
    "detailOptimalRamCtx": "Maks. kontekst RAM",
    "optimalGpuCtxLabel": "Pełna prędkość GPU: {{ctx}} tokenów",
    "optimalRamCtxLabel": "Maks. RAM: {{ctx}} tokenów",
    "optimalGpuCtxShort": "GPU: {{ctx}}",
    "optimalRamCtxShort": "Maks: {{ctx}}",
    "fullGpuOffloadShort": "100% GPU: {{ctx}}",
    "ctxExceedsGpu": "Kontekst przekracza optimum GPU ({{ctx}}). Pamięć podręczna KV wycieknie do RAM, zmniejszając prędkość.",
    "modelExceedsVram": "Model przekracza VRAM. Uruchamianie z RAM z częściowym odciążeniem GPU."
  },
  "systemPrompts": {
    "filters": {
      "all": "Wszystkie",
      "system": "Systemowe",
      "internal": "Wewnętrzne",
      "custom": "Niestandardowe"
    },
    "empty": {
      "title": "Brak niestandardowych promptów",
      "description": "Twórz niestandardowe prompty systemowe, aby spersonalizować rozmowy AI",
      "createButton": "Utwórz prompt"
    },
    "preview": {
      "whatLlmSees": "What the LLM sees",
      "turns": "Turns",
      "noMessages": "No messages",
      "noMessagesHint": "Add entries or increase turns",
      "showMore": "Show more",
      "showLess": "Show less",
      "statChat": "chat",
      "statInjected": "injected",
      "statTotal": "total",
      "entry": "Entry",
      "editEntry": "Edit entry",
      "reorder": "Reorder",
      "delete": "Delete",
      "deleteTitle": "Delete entry?",
      "deleteMessage": "Remove \"{{name}}\" from the prompt template? This cannot be undone.",
      "deleteConfirm": "Delete",
      "thisEntry": "this entry",
      "condensedName": "Condensed System Prompt",
      "imageAttachment": "[Image attachment: {{label}}]",
      "imageSlot": {
        "character": "Character reference image",
        "persona": "Persona reference image",
        "chatBackground": "Chat background image",
        "avatar": "Avatar image",
        "references": "Reference images"
      },
      "injection": {
        "relative": "relative",
        "inChat": "inChat · depth {{depth}}",
        "conditional": "conditional · min {{min}}",
        "interval": "interval · every {{every}}"
      }
    }
  },
  "personas": {
    "empty": {
      "title": "Brak person",
      "description": "Utwórz personę, aby zdefiniować, jak AI powinno się do ciebie zwracać",
      "createButton": "Utwórz personę"
    },
    "actions": {
      "editPersona": "Edytuj personę",
      "setAsDefault": "Ustaw jako domyślną",
      "setAsDefaultDesc": "Używaj we wszystkich nowych czatach",
      "unsetAsDefault": "Usuń jako domyślną",
      "unsetAsDefaultDesc": "Usuń status domyślnej",
      "exportPersona": "Eksportuj personę",
      "deletePersona": "Usuń personę"
    },
    "edit": {
      "avatarHint": "Stuknij, aby dodać lub wygenerować awatar",
      "nameLabel": "NAZWA PERSONY",
      "namePlaceholder": "np. Profesjonalny, Kreatywny pisarz, Student...",
      "nameHint": "Nadaj swojej personie opisową nazwę",
      "nicknameLabel": "PSEUDONIM (OPCJONALNIE)",
      "nicknamePlaceholder": "np. Wariant roboczy, Tryb RPG...",
      "nicknameHint": "Prywatny pseudonim pozwalający odróżnić warianty tej persony w twojej bibliotece",
      "descriptionLabel": "OPIS",
      "descriptionPlaceholder": "Opisz, jak AI powinno się do ciebie zwracać, twoje preferencje, tło lub styl komunikacji...",
      "wordCount": "słów",
      "descriptionHint": "Bądź konkretny co do sposobu, w jaki chcesz być adresowany",
      "setAsDefault": "Ustaw jako domyślną",
      "defaultDescription": "Używaj tej persony we wszystkich nowych rozmowach",
      "exportButton": "Eksportuj personę"
    },
    "designReferences": {
      "title": "Design references",
      "description": "Attach a few stable image references and one concise design note for scene generation."
    },
    "create": {
      "namePlaceholderExample": "Professional Writer",
      "descriptionPlaceholderExample": "Write in a professional, clear, and concise style. Use formal language and focus on delivering information effectively..."
    },
    "errors": {
      "exportFailed": "Failed to export persona",
      "importFailed": "Failed to import persona",
      "loadFailed": "Failed to load persona",
      "saveFailed": "Failed to save persona"
    },
    "importToast": {
      "legacyJsonTitle": "Legacy JSON import detected",
      "legacyJsonMessage": "JSON imports are deprecated and will be removed soon. Use Settings > Convert Files.",
      "successMessage": "Persona imported successfully! Opening it for review."
    }
  },
  "security": {
    "pureMode": {
      "off": "Wył.",
      "offDesc": "Wszystkie treści dozwolone",
      "low": "Niski",
      "lowDesc": "Blokuje wyraźne treści seksualne + wulgaryzmy",
      "standard": "Standardowy",
      "standardDesc": "Blokuje NSFW + graficzną przemoc",
      "strict": "Ścisły",
      "strictDesc": "Maksymalne filtrowanie + brak sugestywnego tonu"
    }
  },
  "advanced": {
    "sectionTitles": {
      "aiFeatures": "Funkcje AI",
      "memorySystem": "System pamięci",
      "usageAnalytics": "Analityka użycia"
    },
    "creationHelper": {
      "title": "Asystent tworzenia",
      "description": "Kreator postaci prowadzony przez AI"
    },
    "helpMeReply": {
      "title": "Pomóż mi odpowiedzieć",
      "description": "Sugestie odpowiedzi wspomagane przez AI"
    },
    "dynamicMemory": {
      "title": "Pamięć dynamiczna",
      "contextWindow": "Okno kontekstu",
      "contextWindowDesc": "Liczba ostatnich wiadomości do uwzględnienia (1-1000)",
      "infoText": "Pamięć dynamiczna używa AI do automatycznego podsumowywania i zarządzania kontekstem rozmowy, umożliwiając dłuższe, bardziej spójne konwersacje.",
      "disabledText": "Po wyłączeniu aplikacja używa prostego przesuwanego okna ostatnich wiadomości określonego przez ustawienie Okna kontekstu."
    },
    "usageAnalytics": {
      "recalculateTitle": "Przelicz koszty użycia",
      "recalculateDesc": "Zaktualizuj wszystkie historyczne rekordy użycia z poprawnymi cenami",
      "recalculating": "Przeliczanie...",
      "recalculateButton": "Przelicz wszystkie koszty",
      "openRouterApiKeyRequired": "Wymagany klucz API OpenRouter. Skonfiguruj go w Ustawienia → Dostawcy.",
      "importantLabel": "Ważne:",
      "warningCannotUndo": "Tej operacji nie można cofnąć",
      "warningMayTakeTime": "Może to zająć czas, jeśli masz wiele rekordów",
      "warningOnlyOpenRouter": "Zaktualizowane zostaną tylko rekordy OpenRouter z tokenami",
      "warningExistingValues": "Istniejące wartości kosztów zostaną nadpisane"
    },
    "extra": {
      "creationHelperDetail": "Get intelligent suggestions for personality traits, backstory, and dialogue style",
      "helpMeReplyDetail": "Generate contextual response options based on conversation history",
      "lorebookEntryGenerator": "Lorebook Entry Generator",
      "lorebookEntryDesc": "Turn selected chat messages into durable lorebook entries and configure the draft prompts for entry writing and keyword generation.",
      "companions": "Companions",
      "companionModeDesc": "Manage local analysis models for emotion, entity extraction, and memory routing used by companion characters.",
      "companionSoulWriter": "Twórca Companion Soul",
      "companionSoulDesc": "Wybierz model, model zapasowy i szablon promptu używany do tworzenia Companion Souls. Najpierw wywoływanie narzędzi, a jeśli nie jest obsługiwane, ustrukturyzowany fallback.",
      "network": "Network",
      "apiServer": "API Server",
      "apiServerDesc": "Expose models via an OpenAI-compatible API server",
      "apiServerRunning": "Server is currently running"
    }
  },
  "backup": {
    "tabs": {
      "create": "Utwórz"
    },
    "create": {
      "newBackupButton": "Nowa kopia zapasowa",
      "exportDescription": "Eksportuj wszystkie dane z szyfrowaniem",
      "createButton": "Utwórz kopię zapasową"
    },
    "restore": {
      "availableBackups": "Dostępne kopie zapasowe",
      "browseFiles": "Przeglądaj pliki",
      "noBackupsFound": "Nie znaleziono kopii zapasowych",
      "noBackupsDesc": "Utwórz kopię zapasową lub stuknij „Przeglądaj pliki\", aby ją znaleźć",
      "browseDesc": "Przeglądaj pliki .lettuce",
      "restoreDialogTitle": "Przywróć kopię zapasową",
      "deleteDialogTitle": "Usuń kopię zapasową",
      "embeddingPrompt": "Dynamic Memory Embedding",
      "downloadModel": "Download Model",
      "disableAndContinue": "Disable and Continue"
    },
    "extra": {
      "successMessage": "Backup created!",
      "savedLocation": "Saved to Downloads"
    }
  },
  "reset": {
    "title": "Resetuj wszystko",
    "description": "Spowoduje to trwałe usunięcie wszystkich dostawców, modeli, postaci, sesji czatu i preferencji z tego urządzenia.",
    "warning": "Tej operacji nie można cofnąć",
    "resetButton": "Resetuj wszystkie dane",
    "confirmTitle": "Czy na pewno?",
    "confirmDescription": "Wszystkie twoje dane zostaną trwale usunięte. Aplikacja wróci do początkowej konfiguracji.",
    "confirmButton": "Tak, zresetuj wszystko"
  },
  "chatAppearance": {
    "typography": "Typografia",
    "fontSize": {
      "label": "Rozmiar czcionki",
      "small": "Mały",
      "medium": "Średni",
      "large": "Duży",
      "xLarge": "Bardzo duży"
    },
    "lineSpacing": {
      "label": "Odstęp między wierszami",
      "tight": "Ciasny",
      "normal": "Normalny",
      "relaxed": "Luźny"
    },
    "messageBubbles": {
      "label": "Bąbelki wiadomości",
      "style": {
        "label": "Styl",
        "bordered": "Z ramką",
        "filled": "Wypełnione",
        "minimal": "Minimalne"
      },
      "cornerRadius": {
        "label": "Zaokrąglenie rogów",
        "sharp": "Ostre",
        "rounded": "Zaokrąglone",
        "pill": "Pigułka"
      },
      "maxWidth": {
        "label": "Maks. szerokość",
        "compact": "Kompaktowa",
        "normal": "Normalna",
        "wide": "Szeroka"
      },
      "padding": {
        "label": "Wypełnienie",
        "compact": "Kompaktowe",
        "normal": "Normalne",
        "spacious": "Przestronne"
      }
    },
    "layout": {
      "label": "Układ",
      "messageSpacing": "Odstęp między wiadomościami",
      "tight": "Ciasny",
      "normal": "Normalny",
      "relaxed": "Luźny"
    },
    "avatar": {
      "shape": {
        "label": "Kształt awatara",
        "circle": "Okrągły",
        "rounded": "Zaokrąglony",
        "hidden": "Ukryty"
      },
      "size": {
        "label": "Rozmiar awatara",
        "small": "Mały",
        "medium": "Średni",
        "large": "Duży"
      }
    },
    "colors": {
      "label": "Kolory",
      "userBubble": "Kolor bąbelka użytkownika",
      "assistantBubble": "Kolor bąbelka asystenta",
      "userBubbleHex": "Nadpisanie hex bąbelka użytkownika",
      "assistantBubbleHex": "Nadpisanie hex bąbelka asystenta",
      "neutral": "Neutralny",
      "accent": "Akcent",
      "info": "Informacja",
      "secondary": "Drugorzędny",
      "warning": "Ostrzeżenie",
      "textColors": "Text Colors",
      "messageTextHex": "Kolor cytatu w tekście",
      "plainTextHex": "Plain Text Color",
      "italicTextHex": "Italic Text Color",
      "quotedTextHex": "Kolor cytatu blokowego",
      "inlineCodeTextHex": "Kolor kodu inline"
    },
    "backgroundTransparency": {
      "label": "Tło i przezroczystość",
      "backgroundDim": "Przyciemnienie tła",
      "backgroundBlur": "Rozmycie tła",
      "bubbleBlur": "Rozmycie bąbelka",
      "none": "Brak",
      "light": "Lekkie",
      "medium": "Średnie",
      "heavy": "Mocne",
      "bubbleOpacity": "Krycie bąbelka"
    },
    "textColorMode": {
      "label": "Tryb koloru tekstu",
      "auto": "Auto",
      "light": "Jasny",
      "dark": "Ciemny"
    },
    "preview": {
      "label": "Podgląd",
      "generic": "Ogólny",
      "live": "Na żywo"
    },
    "extra": {
      "reset": "Reset"
    }
  },
  "colorCustomization": {
    "tokens": {
      "surface": "Powierzchnia",
      "surfaceDesc": "Tła stron",
      "surfaceEl": "Powierzchnia podniesiona",
      "surfaceElDesc": "Karty, modale, elementy podniesione",
      "nav": "Nawigacja",
      "navDesc": "Górne i dolne paski",
      "foreground": "Pierwszy plan",
      "foregroundDesc": "Obramowania, nakładki, nawigacja i elementy interfejsu",
      "appText": "Tekst aplikacji",
      "appTextDesc": "Główny tekst i etykiety interfejsu",
      "appTextMuted": "Przytłumiony tekst",
      "appTextMutedDesc": "Tekst drugorzędny i pomocniczy",
      "appTextSubtle": "Subtelny tekst",
      "appTextSubtleDesc": "Wskazówki, tekst pomocniczy i symbole zastępcze",
      "accent": "Akcent",
      "accentDesc": "Główne akcje, sukces",
      "info": "Informacja",
      "infoDesc": "Stany informacyjne, linki",
      "warning": "Ostrzeżenie",
      "warningDesc": "Stany ostrzegawcze, alerty",
      "danger": "Zagrożenie",
      "dangerDesc": "Destrukcyjne akcje, błędy",
      "secondary": "Drugorzędny",
      "secondaryDesc": "Funkcje AI, narzędzia kreatywne"
    },
    "presetsLabel": "Predefiniowane",
    "customPresetsLabel": "Niestandardowe predefiniowane",
    "previewLabel": "Podgląd",
    "settingsCardsLabel": "Karty ustawień",
    "settingsCardsOpacity": "Nieprzezroczystość kart",
    "settingsCardsOpacityDesc": "Określa, jak przezroczyste wyglądają karty ustawień i wiersze list.",
    "importButton": "Importuj",
    "exportButton": "Eksportuj",
    "resetAllButton": "Resetuj wszystko",
    "presets": {
      "defaultDark": "Domyślny ciemny",
      "midnightBlue": "Nocny błękit",
      "warmEarth": "Ciepła ziemia",
      "purpleHaze": "Fioletowa mgła",
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
      "monochrome": "Monochromatyczny"
    },
    "groups": {
      "backgrounds": "Tła",
      "content": "Treść",
      "semantic": "Semantyczne"
    },
    "extra": {
      "surface": "Surface",
      "surfaceDesc": "Page backgrounds",
      "surfaceEl": "Surface Elevated",
      "surfaceElDesc": "Cards, modals, raised elements",
      "nav": "Navigation",
      "navDesc": "Top & bottom bars",
      "fg": "Foreground",
      "fgDesc": "Borders, overlays, navigation, UI chrome",
      "appText": "App Text",
      "appTextDesc": "Primary text and interface labels",
      "appTextMuted": "Muted Text",
      "appTextMutedDesc": "Secondary text and support copy",
      "appTextSubtle": "Subtle Text",
      "appTextSubtleDesc": "Hints, helper text, placeholders",
      "accent": "Accent",
      "accentDesc": "Primary actions, success",
      "info": "Info",
      "infoDesc": "Informational states, links",
      "warning": "Warning",
      "warningDesc": "Caution states, alerts",
      "danger": "Danger",
      "dangerDesc": "Destructive actions, errors",
      "secondary": "Secondary"
    }
  },
  "dynamicMemory": {
    "page": {
      "info": "Pamięć dynamiczna automatycznie podsumowuje rozmowy, aby efektywnie utrzymywać kontekst. Wybierz preset lub dostosuj ustawienia do swoich potrzeb.",
      "disabledDirectTitle": "Pamięć dynamiczna jest wyłączona dla czatów bezpośrednich",
      "disabledDirectDescription": "Przełącz przełącznik w zakładce Czaty Bezpośrednie, aby ją włączyć. Czaty grupowe używają trybu pamięci per sesja.",
      "directChats": "Czaty Bezpośrednie",
      "groupChats": "Czaty Grupowe",
      "enableDirectChats": "Włącz dla Czatów Bezpośrednich",
      "groupChatsInfo": "Czaty grupowe używają trybu pamięci per sesja. Włącz pamięć dynamiczną w ustawieniach każdej grupy. Te ustawienia kontrolują zachowanie pamięci dynamicznej.",
      "memoryProfile": "Profil Pamięci",
      "customSettings": "Ustawienia niestandardowe - dostosuj wartości w Opcjach Zaawansowanych poniżej.",
      "contextEnrichment": "Wzbogacanie Kontekstu",
      "experimental": "Eksperymentalne",
      "contextEnrichmentDescription": "Używa ostatnich wiadomości do inteligentniejszego pobierania pamięci",
      "advancedOptions": "Opcje Zaawansowane",
      "advancedOptionsDescription": "Dostosuj zachowanie pamięci",
      "summaryInterval": "Interwał Podsumowań",
      "summaryIntervalDescription": "Wiadomości między podsumowaniami",
      "maxMemoryEntries": "Maks. Wpisów Pamięci",
      "maxMemoryEntriesDescription": "Maksymalna liczba przechowywanych wspomnień",
      "hotMemoryBudget": "Budżet Aktywnej Pamięci",
      "hotMemoryBudgetDescription": "Limit tokenów dla aktywnych wspomnień",
      "relevanceThreshold": "Próg Trafności",
      "relevanceThresholdDescription": "Minimalne podobieństwo do pobrania",
      "retrievalMode": "Tryb Pobierania",
      "retrievalModeSmart": "Inteligentny",
      "retrievalModeCosine": "Cosine",
      "retrievalModeDescription": "Inteligentny łączy trafność z aktualnością/częstotliwością. Cosine używa czystego najwyższego podobieństwa.",
      "retrievalLimit": "Limit Pobierania",
      "retrievalLimitDescription": "Maks. wspomnień wybieranych na turę",
      "decayRate": "Współczynnik Zanikania",
      "decayRateDescription": "Jak szybko maleje ważność",
      "coldStorageThreshold": "Próg Zimnego Przechowywania",
      "coldStorageThresholdDescription": "Kiedy wspomnienia trafiają do archiwum",
      "sharedSettings": "Wspólne Ustawienia",
      "summarisationModel": "Model Podsumowań",
      "selectedModel": "Wybrany Model",
      "useGlobalDefaultModel": "Użyj globalnego domyślnego modelu",
      "noModelsAvailable": "Brak dostępnych modeli",
      "summarisationModelDescription": "Używany do podsumowywania rozmów",
      "modelManagement": "Zarządzanie Modelami",
      "testModel": "Testuj Model",
      "downloadModel": "Pobierz Model",
      "delete": "Usuń",
      "embeddingModel": "Model Embedding",
      "tokenCapacity": "Pojemność Tokenów",
      "tokenCapacityDescription": "Wyższe wartości = lepsza pamięć dla dłuższych rozmów",
      "keepModelLoaded": "Utrzymuj Model Załadowany",
      "keepModelLoadedDescription": "Utrzymuje model embedding + tokenizer w pamięci, aby uniknąć narzutu ponownego ładowania",
      "installedModel": "Zainstalowany model: {{version}} ({{tokens}} maks. tokenów)",
      "downloadEmbeddingModel": "Pobierz Model Embedding",
      "downloadEmbeddingDescription": "Wybierz wersję do pobrania. Zainstalowane wersje są wyłączone.",
      "downloadVersion": "Pobierz {{version}}",
      "downloadV2Description": "Zoptymalizowany pod kątem dokładności i przywracania długiego kontekstu",
      "downloadV3Description": "Najnowsza jakość embedding",
      "installed": "Zainstalowano",
      "selectModel": "Wybierz Model",
      "searchModels": "Szukaj modeli...",
      "deleteEmbeddingTitle": "Usunąć model {{version}}?",
      "deleteEmbeddingMessage": "Czy na pewno chcesz usunąć {{version}}? Możesz pobrać go ponownie później.",
      "msgsUnit": "wiad.",
      "entriesUnit": "wpisów",
      "tokensUnit": "tokenów",
      "itemsUnit": "elementów",
      "perCycleUnit": "/ cykl"
    },
    "presets": {
      "minimal": "minimalna",
      "balanced": "zrównoważona",
      "comprehensive": "kompleksowa",
      "minimalDesc": "Fast & efficient. Keeps only essential memories.",
      "balancedDesc": "Good mix of context retention and performance.",
      "comprehensiveDesc": "Maximum context. Best for long, detailed conversations."
    },
    "presetInfo": {
      "minimal": "Szybka i wydajna. Zachowuje tylko najważniejsze wspomnienia.",
      "balanced": "Dobry balans między zachowaniem kontekstu a wydajnością.",
      "comprehensive": "Maksymalny kontekst. Najlepsza do długich, szczegółowych rozmów."
    }
  },
  "helpMeReply": {
    "page": {
      "info": "Pomóż Mi Odpowiedzieć generuje kontekstowe sugestie dla Twojej następnej wiadomości na podstawie historii rozmowy. Skonfiguruj model i styl odpowiedzi poniżej."
    },
    "labels": {
      "replyModel": "Model Odpowiedzi",
      "selectedModel": "Wybrany Model",
      "useAppDefault": "Użyj domyślnego aplikacji{{model}}",
      "useAppDefaultBase": "Użyj domyślnego aplikacji",
      "noModelsAvailable": "Brak dostępnych modeli",
      "replyModelDescription": "Model AI do generowania sugestii odpowiedzi",
      "streamingOutput": "Wyjście Strumieniowe",
      "streamingDescription": "Pokazuj sugestie w trakcie generowania",
      "maxTokens": "Maks. Tokenów",
      "maxTokensDescription": "Maksymalna długość sugestii",
      "conversationalHint": "Sugestie będą napisane jako naturalny dialog, odpowiedni do swobodnych rozmów.",
      "roleplayHint": "Sugestie będą zawierać elementy roleplay, takie jak *akcje* i opisy narracyjne.",
      "footerInfo": "To ustawienie obowiązuje globalnie we wszystkich rozmowach. Mniejsza liczba tokenów generuje krótsze, szybsze sugestie, a większa pozwala na bardziej szczegółowe odpowiedzi.",
      "selectReplyModel": "Wybierz Model Odpowiedzi",
      "searchModels": "Szukaj modeli..."
    },
    "sectionTitles": {
      "modelConfiguration": "Konfiguracja modelu",
      "responseStyle": "Styl odpowiedzi"
    },
    "responseStyle": {
      "conversational": "Konwersacyjny",
      "conversationalDesc": "Naturalny, swobodny ton",
      "roleplay": "Roleplay",
      "roleplayDesc": "Akcje w postaci"
    },
    "extra": {
      "conversational": "Conversational",
      "roleplay": "Roleplay"
    }
  },
  "imageGeneration": {
    "promptPlaceholder": "Opisz obraz, który chcesz wygenerować...",
    "labels": {
      "model": "MODEL",
      "prompt": "PROMPT",
      "size": "ROZMIAR",
      "quality": "JAKOŚĆ",
      "style": "STYL",
      "searchModels": "Wyszukaj modele...",
      "selectAvatarModel": "Wybierz Model awatara",
      "selectSceneModel": "Wybierz Model sceny",
      "selectWriterModel": "Wybierz model autora scen",
      "useFirstAvailable": "Użyj pierwszego dostępnego modelu",
      "useFirstCompatible": "Użyj pierwszego zgodnego modelu autora"
    },
    "mode": {
      "title": "Tryb działania",
      "description": "Wybierz, jak obsługiwać prompty scen wykryte w odpowiedzi modelu.",
      "auto": "Automatyczny",
      "autoDescription": "Generuj obraz sceny od razu, gdy model poda prompt sceny.",
      "askFirst": "Pytaj najpierw",
      "askFirstDescription": "Pokaż wykryty prompt sceny i poczekaj na akceptację przed wygenerowaniem obrazu.",
      "manual": "Ręczny",
      "manualDescription": "Ignoruj prompty scen z odpowiedzi modelu. Używaj tylko działań uruchomionych ręcznie przez użytkownika."
    },
    "empty": {
      "title": "Brak modeli obrazów",
      "description": "Dodaj model generowania obrazów na stronie Modele, aby zacząć generować obrazy."
    },
    "sections": {
      "avatar": {
        "title": "Generacja awatarów",
        "description": "Domyślny model używany podczas generowania awatarów na podstawie selektora awatarów lub powiązanych przepływów obrazów profilowych."
      },
      "scene": {
        "title": "Generowanie scen",
        "description": "Zarezerwowany model dla obrazów scen generowanych na podstawie kontekstu konwersacji lub podpowiedzi scen."
      },
      "writer": {
        "title": "Autor scen",
        "description": "Zarezerwowany multimodalny model tekstowy do tworzenia promptów scen i opisów referencji projektowych na podstawie kontekstu czatu, awatarów i obrazów referencyjnych."
      }
    },
    "extra": {
      "avatarGeneration": "Avatar Generation",
      "sceneGeneration": "Scene Generation",
      "sceneWriter": "Scene Writer"
    }
  },
  "logs": {
    "sectionTitles": {
      "diagnostics": "Diagnostyka",
      "generate": "Generuj",
      "copy": "Kopiuj"
    },
    "extra": {
      "debug": "DEBUG",
      "info": "INFO",
      "warn": "WARN",
      "error": "ERROR"
    }
  },
  "developer": {
    "sectionTitles": {
      "testDataGenerators": "Generatory danych testowych",
      "storageMaintenance": "Konserwacja pamięci",
      "usageTracking": "Śledzenie użycia",
      "crashTesting": "Testy zderzeniowe",
      "environmentInfo": "Informacje o środowisku"
    },
    "testData": {
      "generateCharacter": "Generuj testową postać",
      "generateCharacterDesc": "Utwórz pojedynczą testową postać",
      "generatePersona": "Generuj testową personę",
      "generatePersonaDesc": "Utwórz pojedynczą testową personę",
      "generateSession": "Generuj testową sesję",
      "generateSessionDesc": "Utwórz testową sesję czatu z istniejącą postacią",
      "generateBulk": "Generuj masowe dane testowe",
      "generateBulkDesc": "Utwórz 3 postacie i 2 persony"
    },
    "storageMaintenance": {
      "optimizeDb": "Optymalizuj bazę danych",
      "optimizeDbDesc": "Zastosuj PRAGMA i uruchom VACUUM (tylko mobilne)",
      "backupLegacy": "Kopia zapasowa i usunięcie starszych plików",
      "backupLegacyDesc": "Przenosi starsze pliki .bin do folderu kopii zapasowej"
    },
    "usageTracking": {
      "recalculateAll": "Przelicz wszystkie koszty użycia",
      "recalculateAllDesc": "Ponownie pobiera ceny i przelicza koszty dla wszystkich rekordów użycia OpenRouter"
    },
    "crashTesting": {
      "forceCrash": "Awaria aplikacji teraz",
      "forceCrashDesc": "Natychmiast kończy proces aplikacji natywnej w celu przetestowania wykrywania awarii",
      "forceCrashConfirm": "Spowoduje to natychmiastową awarię aplikacji w celu przetestowania detektora awarii. Kontynuować?"
    },
    "environmentInfo": {
      "mode": "Tryb",
      "devMode": "Tryb deweloperski",
      "viteVersion": "Wersja Vite"
    },
    "status": {
      "testCharacterCreated": "✓ Testowa postać utworzona pomyślnie",
      "testPersonaCreated": "✓ Testowa persona utworzona pomyślnie",
      "testSessionCreated": "✓ Testowa sesja utworzona: {{id}}",
      "generatingBulkData": "Generowanie masowych danych testowych...",
      "bulkDataCreated": "✓ Masowe dane testowe utworzone: 3 postacie, 2 persony",
      "creatingBenchmarkChat": "Tworzenie postaci i sesji benchmarkowej z danymi startowymi...",
      "seededBenchmarkReady": "✓ Zainicjalizowany benchmark gotowy: {{name}} / {{id}}",
      "creatingBenchmarkGroupChat": "Tworzenie grupowego czatu benchmarkowego z danymi startowymi...",
      "seededGroupBenchmarkReady": "✓ Zainicjalizowany benchmark grupowy gotowy: {{id}}",
      "dbOptimized": "✓ Baza danych zoptymalizowana",
      "recalculatingCosts": "Przeliczanie kosztów użycia... To może chwilę potrwać.",
      "toursReset": "✓ Wszystkie przewodniki zresetowane — pojawią się ponownie przy następnej wizycie",
      "crashingApp": "Zamykanie aplikacji awaryjnie..."
    },
    "errors": {
      "noCharacters": "Brak dostępnych postaci. Najpierw utwórz testową postać.",
      "createCharacterFailed": "Nie udało się utworzyć testowej postaci: {{error}}",
      "createPersonaFailed": "Nie udało się utworzyć testowej persony: {{error}}",
      "createSessionFailed": "Nie udało się utworzyć testowej sesji: {{error}}",
      "createBulkFailed": "Nie udało się utworzyć masowych danych testowych: {{error}}",
      "createBenchmarkFailed": "Nie udało się utworzyć sesji benchmarkowej: {{error}}",
      "createGroupBenchmarkFailed": "Nie udało się utworzyć grupowej sesji benchmarkowej: {{error}}",
      "dbOptimizeFailed": "Optymalizacja bazy danych nie powiodła się: {{error}}",
      "backupFailed": "Kopia zapasowa nie powiodła się: {{error}}",
      "openRouterKeyMissing": "Nie znaleziono klucza API OpenRouter. Skonfiguruj go w Ustawienia > Dostawcy.",
      "recalculationFailed": "Przeliczanie nie powiodło się: {{error}}",
      "resetToursFailed": "Nie udało się zresetować przewodników: {{error}}",
      "crashFailed": "Nie udało się zamknąć aplikacji awaryjnie: {{error}}"
    },
    "onboarding": {
      "title": "Wprowadzenie",
      "resetTours": "Resetuj wszystkie przewodniki",
      "resetToursDesc": "Czyści stan wyświetlania każdego przewodnika, aby odtworzyły się przy następnej wizycie."
    },
    "benchmarks": {
      "createChat": "Utwórz zainicjalizowany czat benchmarkowy",
      "createChatDesc": "Tworzy postać z dynamiczną pamięcią, scenę startową i 20-wiadomościową sesję testową ciągłości, a następnie ją otwiera.",
      "createGroupChat": "Utwórz zainicjalizowany grupowy czat benchmarkowy",
      "createGroupChatDesc": "Tworzy grupowy czat z dynamiczną pamięcią z trzema postaciami benchmarkowymi i 30 wiadomościami startowymi, a następnie go otwiera."
    },
    "extra": {
      "testCharacter": "Test Character",
      "testCharacterDesc": "A test character created for development purposes.",
      "testScene": "A simple test scene for development",
      "testPersona": "Test Persona",
      "testPersonaDesc": "A test persona for development",
      "successChar": "✓ Test character created successfully",
      "successPersona": "✓ Test persona created successfully",
      "successSession": "✓ Test session created: {{id}}",
      "successBulk": "✓ Bulk test data created: 3 characters, 2 personas",
      "errorCharAvailable": "No characters available. Create a test character first.",
      "generatingBulk": "Generating bulk test data..."
    }
  },
  "embeddingDownload": {
    "capacityOptions": {
      "oneK": "1K tokenów",
      "oneKDesc": "Najlepszy dla szybkich odpowiedzi",
      "twoK": "2K tokenów",
      "twoKDesc": "Zrównoważona wydajność",
      "fourK": "4K tokenów",
      "fourKDesc": "Maksymalny kontekst"
    },
    "extra": {
      "status": "Downloading..."
    }
  },
  "embeddingTest": {
    "categories": {
      "semanticSimilarity": "Podobieństwo semantyczne",
      "dissimilarityCheck": "Sprawdzenie niepodobieństwa",
      "roleplayContext": "Kontekst roleplay"
    },
    "extra": {
      "placeholder": "Enter text to embed..."
    }
  },
  "discovery": {
    "tabs": {
      "forYou": "Dla ciebie",
      "trending": "Na czasie",
      "popular": "Popularne",
      "new": "Nowe"
    },
    "searchPlaceholder": "Szukaj postaci...",
    "viewAll": "Zobacz wszystkie",
    "errorTitle": "Coś poszło nie tak",
    "noCardsFound": "Nie znaleziono kart",
    "sections": {
      "trendingNow": "Teraz na czasie",
      "trendingSubtitle": "Gorące w tym tygodniu",
      "mostPopular": "Najpopularniejsze",
      "popularSubtitle": "Ulubione społeczności",
      "freshArrivals": "Nowe przybycia",
      "freshSubtitle": "Właśnie dodane"
    },
    "browse": {
      "newArrivals": "Nowe przybycia",
      "freshCharacters": "Świeże postacie",
      "noCharactersFound": "Nie znaleziono postaci",
      "noCharactersSubtitle": "Sprawdź ponownie później, aby zobaczyć nowe treści"
    },
    "sort": {
      "mostLiked": "Najbardziej lubiane",
      "mostDownloaded": "Najczęściej pobierane",
      "mostViewed": "Najczęściej oglądane",
      "mostMessages": "Najwięcej wiadomości",
      "newestFirst": "Najnowsze najpierw",
      "recentlyUpdated": "Ostatnio aktualizowane",
      "nameAZ": "Nazwa (A-Z)"
    },
    "sortBy": "Sortuj wg",
    "resultsUnit": "postaci",
    "detail": {
      "share": "Udostępnij",
      "nsfwOverlay": "Treści NSFW",
      "nsfwBadge": "NSFW",
      "originalBadge": "Oryginał",
      "lorebookBadge": "Lorebook",
      "alsoKnownAs": "Znana również jako:",
      "followersUnit": "obserwujących",
      "sections": {
        "description": "Opis",
        "tokenUsage": "Użycie tokenów",
        "startingScenes": "Sceny początkowe",
        "scenario": "Scenariusz",
        "personality": "Osobowość",
        "stats": "Statystyki",
        "tags": "Tagi",
        "author": "Autor"
      },
      "tokensTotalLabel": "łącznie",
      "tokens": {
        "description": "Opis",
        "personality": "Osobowość",
        "scenario": "Scenariusz",
        "firstMessage": "Pierwsza wiadomość",
        "scenes": "Sceny",
        "examples": "Przykłady",
        "systemPrompt": "Prompt systemowy"
      },
      "sceneLabels": {
        "primary": "Główna",
        "alternate": "Alternatywna"
      },
      "stats": {
        "views": "Wyświetlenia",
        "downloads": "Pobrania",
        "messages": "Wiadomości"
      },
      "downloaded": "Pobrane",
      "startChat": "Rozpocznij czat",
      "downloadCharacter": "Pobierz postać",
      "downloading": "Pobieranie...",
      "downloadSuccess": {
        "title": "Postać pobrana!",
        "subtitle": "Dodana do twojej biblioteki",
        "badge": "Zapisano",
        "startChat": "Rozpocznij czat",
        "startChatDesc": "Otwórz pierwszą scenę teraz",
        "viewLibrary": "Zobacz w bibliotece",
        "viewLibraryDesc": "Edytuj, zarządzaj lub eksportuj później",
        "continueBrowsing": "Kontynuuj przeglądanie",
        "continueBrowsingDesc": "Wróć do odkrywania"
      },
      "errorTitle": "Błąd",
      "errorSubtitle": "Nie udało się załadować",
      "errorNotFound": "Nie znaleziono postaci",
      "defaultChatTitle": "New Chat"
    },
    "search": {
      "placeholder": "Szukaj postaci, tagów, autorów...",
      "resultsUnit": "wyników",
      "timingUnit": "ms",
      "recentSearches": "Ostatnie wyszukiwania",
      "clearAll": "Wyczyść wszystko",
      "trendingSearches": "Popularne wyszukiwania",
      "trends": {
        "anime": "anime",
        "fantasy": "fantasy",
        "romance": "romans",
        "villain": "złoczyńca",
        "adventure": "przygoda",
        "comedy": "komedia",
        "mystery": "tajemnica",
        "sciFi": "sci-fi"
      },
      "tips": {
        "title": "Wskazówki wyszukiwania",
        "tip1": "Szukaj po nazwie postaci, autorze lub opisie",
        "tip2": "Używaj tagów jak \"anime\", \"fantasy\" lub \"romans\"",
        "tip3": "Spróbuj konkretnych cech jak \"tsundere\" lub \"złoczyńca\""
      },
      "loading": "Ładowanie...",
      "loadMore": "Załaduj więcej",
      "noResults": "Nie znaleziono wyników",
      "noResultsFor": "Nie znaleziono postaci dla",
      "noResultsHint": "Spróbuj innych słów kluczowych lub przeglądaj kategorie"
    },
    "errors": {
      "loadContent": "Failed to load content",
      "searchFailed": "Search failed",
      "noCardPath": "No card path provided",
      "loadCharacter": "Failed to load character",
      "downloadCharacter": "Failed to download character"
    },
    "card": {
      "byAuthor": "by {{author}}",
      "ocBadge": "OC"
    }
  },
  "engine": {
    "gpuInsufficient": "Niewystarczająca pamięć GPU",
    "gpuFallbackDesc": "Ten model nie mieści się w pamięci GPU. Przełączyć na CPU (wolniejsze) czy przerwać?",
    "switchToCpu": "Przełącz na CPU",
    "abort": "Przerwij",
    "errors": {
      "providerNotFound": "Nie znaleziono dostawcy silnika.",
      "engineOffline": "Engine is offline or unreachable.",
      "deleteCharacterFailed": "Failed to delete character.",
      "unknownCharacter": "Unknown",
      "seedRequired": "Seed description is required.",
      "characterNameRequired": "Character name is required.",
      "atLeastOneProvider": "At least one provider must be enabled.",
      "enableLlmProvider": "Please enable at least one LLM provider.",
      "modelRequired": "Model is required for {{provider}}.",
      "apiKeyRequired": "API key is required for {{provider}}.",
      "sendMessageFailed": "Failed to send message."
    },
    "status": {
      "connected": "Połączono",
      "offline": "Offline",
      "needsSetup": "Wymaga konfiguracji"
    },
    "home": {
      "characters": "Postacie",
      "newButton": "Nowy",
      "noCharactersFound": "Nie znaleziono postaci.",
      "tokenUsage": "Użycie tokenów",
      "totalTokens": "łącznie tokenów",
      "backgroundActivity": "Aktywność w tle",
      "quickActions": "Szybkie akcje",
      "configureProviders": "Konfiguruj dostawców",
      "engineSettings": "Ustawienia silnika",
      "chat": "Czat",
      "chatDesc": "Rozpocznij rozmowę z tą postacią",
      "deleteCharacter": "Usuń postać",
      "deletingCharacter": "Usuwanie...",
      "deleteDesc": "Trwale usuń tę postać",
      "character": "Postać",
      "never": "Nigdy",
      "justNow": "Właśnie teraz",
      "timeAgo": {
        "minutes": "{{n}}m ago",
        "hours": "{{n}}h ago",
        "days": "{{n}}d ago"
      }
    },
    "tokens": {
      "input": "Wejście",
      "output": "Wyjście"
    },
    "activity": {
      "synthesis": "Synteza",
      "consolidation": "Konsolidacja",
      "bm25Rebuild": "Przebudowa BM25",
      "dripResearch": "Badanie kroplowe",
      "running": "Działa",
      "stopped": "Zatrzymano"
    },
    "setup": {
      "complete": "Konfiguracja zakończona!",
      "completeMessage": "Twój silnik Lettuce jest skonfigurowany i gotowy do pracy.",
      "openDashboard": "Otwórz panel"
    },
    "welcome": {
      "title": "Witamy w Silniku Lettuce",
      "subtitle": "Skonfigurujmy twój silnik postaci AI. Zajmie to około 2 minuty.",
      "feature1": "Silnik daje twoim postaciom AI trwałą pamięć, emocje, relacje i prawdziwą tożsamość.",
      "feature2": "Najpierw skonfigurujemy backend LLM, a potem ustawienia silnika.",
      "getStarted": "Zaczynajmy"
    },
    "config": {
      "activeProviders": "Aktywni dostawcy",
      "noModelSet": "Nie ustawiono modelu",
      "defaultBadge": "Domyślny",
      "noProvidersWarning": "Nie skonfigurowano dostawców. Dodaj co najmniej jeden backend LLM poniżej.",
      "addProvider": "Dodaj dostawcę",
      "quickImport": "Szybki import z twoich dostawców",
      "importButton": "Importuj",
      "fields": {
        "model": "Model",
        "modelPlaceholder": "np. claude-sonnet-4-5-20250929",
        "apiKey": "Klucz API",
        "apiKeyPlaceholder": "Wprowadź klucz API",
        "currentKey": "Bieżący klucz:",
        "baseUrl": "Bazowy URL",
        "baseUrlPlaceholder": "http://localhost:11434",
        "maxTokens": "Maks. tokenów",
        "temperature": "Temperatura"
      },
      "enableProvider": "Włącz dostawcę",
      "setAsDefault": "Ustaw jako domyślny",
      "defaultBackend": "Domyślny backend",
      "remove": "Usuń",
      "saveChanges": "Zapisz zmiany",
      "saving": "Zapisywanie...",
      "saved": "Zapisano"
    },
    "providers": {
      "title": "Dostawca LLM",
      "subtitle": "Silnik wymaga co najmniej jednego backendu LLM. Skonfiguruj jednego lub więcej dostawców poniżej.",
      "importFromProviders": "Importuj z twoich dostawców",
      "imported": "Zaimportowano",
      "use": "Użyj",
      "saveContinue": "Zapisz i kontynuuj"
    },
    "settings": {
      "fields": {
        "dataDirectory": "Katalog danych",
        "logLevel": "Poziom logów",
        "maxHistory": "Maks. historia (tury rozmowy)"
      },
      "logLevels": {
        "debug": "DEBUG",
        "info": "INFO",
        "warning": "OSTRZEŻENIE",
        "error": "BŁĄD"
      },
      "sections": {
        "engine": "Silnik",
        "backgroundLoops": "Pętle w tle",
        "memory": "Pamięć",
        "safety": "Bezpieczeństwo",
        "research": "Badania"
      },
      "backgroundLoops": {
        "synthesis": "Synteza (min)",
        "consolidation": "Konsolidacja (min)",
        "bm25Rebuild": "Przebudowa BM25 (min)",
        "dripResearch": "Badanie kroplowe (min)"
      },
      "memory": {
        "embeddingModel": "Model embeddingu",
        "maxRetrieval": "Maks. wyników wyszukiwania",
        "denseWeight": "Waga gęsta",
        "bm25Weight": "Waga BM25",
        "graphWeight": "Waga grafu",
        "recencyBoost": "Wzmocnienie aktualności (godz.)",
        "randomSurface": "Prawdopodobieństwo losowego wyłaniania"
      },
      "safety": {
        "honestySection": "Sekcja uczciwości",
        "honestyDesc": "Uwzględnij sekcję uczciwości w prompcie systemowym",
        "userDataDeletion": "Usuwanie danych użytkownika",
        "userDataDesc": "Pozwól użytkownikom żądać usunięcia danych"
      },
      "research": {
        "scrapeOnBoot": "Scraping przy starcie",
        "scrapeDesc": "Uruchom scraping badawczy przy starcie silnika",
        "periodicInterval": "Interwał okresowy (godz.)"
      },
      "saveChanges": "Zapisz zmiany",
      "saving": "Zapisywanie...",
      "saved": "Zapisano"
    },
    "settingsStep": {
      "title": "Ustawienia silnika",
      "subtitle": "Skonfiguruj ustawienia silnika. Wszystkie mają rozsądne wartości domyślne — możesz pominąć.",
      "completingSetup": "Kończenie konfiguracji...",
      "completeSetup": "Zakończ konfigurację"
    },
    "chat": {
      "sendMessage": "Wyślij wiadomość...",
      "sendButton": "Wyślij wiadomość",
      "typeMessage": "Wpisz wiadomość",
      "back": "Wstecz",
      "assistantTyping": "Assistant is typing",
      "fallbackName": "Chat"
    },
    "tagInput": {
      "addMore": "Dodaj więcej...",
      "typeAndPressEnter": "Wpisz i naciśnij Enter"
    },
    "characterCreate": {
      "steps": {
        "identity": {
          "title": "Tożsamość",
          "aiGenerated": "Wygenerowane przez AI",
          "nameLabel": "Imię *",
          "namePlaceholder": "Imię postaci",
          "eraLabel": "Epoka",
          "eraPlaceholder": "np. współczesna, wiktoriańska",
          "roleLabel": "Rola",
          "rolePlaceholder": "np. Detektyw, Naukowiec",
          "settingLabel": "Otoczenie",
          "settingPlaceholder": "Opisz, gdzie postać żyje (pierwsza osoba)...",
          "coreIdentityLabel": "Rdzeń tożsamości",
          "coreIdentityPlaceholder": "Kim jest ta postać w swoim rdzeniu? (pierwsza osoba, 3-5 zdań)",
          "backstoryLabel": "Historia",
          "backstoryPlaceholder": "Historia życia i kluczowe wydarzenia (pierwsza osoba)..."
        },
        "mode": {
          "title": "Utwórz postać",
          "subtitle": "Wygeneruj postać za pomocą AI lub zbuduj od zera.",
          "aiBoost": "AI Boost",
          "aiBoostDesc": "Opisz swoją wizję postaci, a AI wygeneruje pełną definicję postaci.",
          "nameOptional": "Imię (opcjonalnie)",
          "namePlaceholder": "np. Marek Kowalski",
          "seedDescription": "Opis wstępny *",
          "seedPlaceholder": "np. pianista jazzowy w Harlemie lat 50., filozoficzny, kocha nocne rozmowy",
          "eraOptional": "Epoka (opcjonalnie)",
          "eraPlaceholder": "np. lata 50., współczesna, wiktoriańska",
          "generating": "Generowanie...",
          "generateCharacter": "Generuj postać",
          "or": "lub",
          "startFromScratch": "Zacznij od zera"
        },
        "personality": {
          "title": "Osobowość",
          "traits": "Cechy osobowości",
          "traitsPlaceholder": "np. dowcipny, współczujący, uparty",
          "speechPatterns": "Wzorce mowy",
          "formality": "Formalność",
          "formal": "Formalny",
          "casual": "Swobodny",
          "texting": "Wiadomości",
          "verbosity": "Gadatliwość",
          "terse": "Lakoniczny",
          "medium": "Średnia",
          "verbose": "Gadatliwy",
          "textStyle": "Styl tekstu",
          "dialect": "Dialekt",
          "dialectPlaceholder": "np. śląski, krakowski",
          "catchphrases": "Ulubione powiedzonka",
          "catchphrasesPlaceholder": "np. No to jazda...",
          "vocabPreferences": "Preferowane słownictwo",
          "vocabPreferencesPlaceholder": "Słowa, które lubi używać",
          "vocabAvoidances": "Unikane słownictwo",
          "vocabAvoidancesPlaceholder": "Słowa, których unika",
          "fillerWords": "Słowa wypełniające",
          "fillerWordsPlaceholder": "np. eee, no, wiesz",
          "exampleQuotes": "Przykładowe cytaty",
          "exampleQuotesPlaceholder": "3-5 przykładowych linii dialogu"
        },
        "world": {
          "title": "Świat i zachowanie",
          "knowledgeDomains": "Domeny wiedzy",
          "knowledgeDomainsPlaceholder": "np. historia jazzu, teoria muzyki",
          "knowledgeBoundaries": "Granice wiedzy",
          "knowledgeBoundariesPlaceholder": "Tematy, o których nie wie",
          "researchSeeds": "Zaczątki badawcze",
          "researchSeedsPlaceholder": "Tematy startowe do badań w tle",
          "researchEnabled": "Badania włączone",
          "researchEnabledDesc": "Pozwól na zbieranie wiedzy w tle",
          "physicalDescription": "Opis fizyczny",
          "physicalDescPlaceholder": "Wygląd fizyczny i manieryzmy...",
          "physicalHabits": "Nawyki fizyczne",
          "physicalHabitsPlaceholder": "np. stuka palcami, poprawia okulary",
          "idleBehaviors": "Zachowania bezczynne",
          "idleBehaviorsPlaceholder": "Co robią, gdy nie są zaangażowani",
          "timeBehaviors": "Zachowania w zależności od pory dnia",
          "timePlaceholder": "Co robią w porze {{period}}?",
          "earlyMorning": "Wczesny ranek",
          "morning": "Rano",
          "afternoon": "Popołudnie",
          "evening": "Wieczór",
          "night": "Noc",
          "baselineEmotions": "Bazowe emocje (Plutchik)",
          "emotionDesc": "Ustaw bazowy stan emocjonalny (0 = brak, 1 = maksimum)",
          "joy": "Radość",
          "trust": "Zaufanie",
          "fear": "Strach",
          "surprise": "Zaskoczenie",
          "sadness": "Smutek",
          "disgust": "Wstręt",
          "anger": "Gniew",
          "anticipation": "Oczekiwanie",
          "engineOverrides": "Nadpisania silnika",
          "backend": "Backend",
          "model": "Model",
          "temperature": "Temperatura",
          "leaveEmpty": "Zostaw puste dla domyślnych"
        },
        "review": {
          "title": "Przegląd",
          "subtitle": "Przejrzyj swoją postać przed utworzeniem.",
          "edit": "Edytuj",
          "notSet": "Nie ustawiono",
          "identitySection": "Tożsamość",
          "personalitySection": "Osobowość",
          "worldSection": "Świat i zachowanie",
          "nameLabel": "Imię",
          "eraLabel": "Epoka",
          "roleLabel": "Rola",
          "settingLabel": "Otoczenie",
          "coreIdentityLabel": "Rdzeń tożsamości",
          "backstoryLabel": "Historia",
          "traitsLabel": "Cechy",
          "formalityLabel": "Formalność",
          "verbosityLabel": "Gadatliwość",
          "dialectLabel": "Dialekt",
          "catchphrasesLabel": "Powiedzonka",
          "domainsLabel": "Domeny",
          "boundariesLabel": "Granice",
          "researchSeedsLabel": "Zaczątki badawcze",
          "researchLabel": "Badania",
          "enabled": "Włączone",
          "disabled": "Wyłączone",
          "physicalLabel": "Fizyczność",
          "habitsLabel": "Nawyki",
          "idleLabel": "Bezczynność",
          "timeBehaviorsLabel": "Zachowania czasowe",
          "emotionsLabel": "Emocje",
          "configured": "Skonfigurowane",
          "backendLabel": "Backend",
          "modelLabel": "Model",
          "temperatureLabel": "Temperatura",
          "creating": "Tworzenie...",
          "createCharacter": "Utwórz postać"
        }
      }
    }
  },
  "library": {
    "filterTitle": "Filtruj bibliotekę",
    "filters": {
      "all": "Wszystko",
      "characters": "Postacie",
      "personas": "Persony",
      "lorebooks": "Lorebooki",
      "images": "Images"
    },
    "emptyStates": {
      "all": {
        "title": "Twoja biblioteka jest pusta",
        "description": "Twórz postacie, persony i lorebooki, aby zobaczyć je tutaj"
      },
      "characters": {
        "title": "Brak postaci",
        "description": "Utwórz swoją pierwszą postać, aby zacząć rozmawiać"
      },
      "personas": {
        "title": "Brak person",
        "description": "Utwórz personę, aby dostosować swoją tożsamość czatową"
      },
      "lorebooks": {
        "title": "Brak lorebooków",
        "description": "Lorebooki tworzy się z poziomu ustawień postaci"
      }
    },
    "actions": {
      "startChat": "Rozpocznij czat",
      "editCharacter": "Edytuj postać",
      "editPersona": "Edytuj personę",
      "editLorebook": "Edytuj lorebook",
      "renameLorebook": "Zmień nazwę lorebooka",
      "exportCharacter": "Eksportuj postać",
      "exportPersona": "Eksportuj personę",
      "chatAppearance": "Wygląd czatu",
      "deleteCharacter": "Usuń postać",
      "deletePersona": "Usuń personę",
      "deleteLorebook": "Usuń lorebook",
      "importLorebook": "Importuj lorebook"
    },
    "imageLibrary": {
      "filters": {
        "all": "Wszystko",
        "backgrounds": "Tła",
        "avatars": "Awatary",
        "attachments": "Załączniki",
        "other": "Inne"
      },
      "searchPlaceholder": "Szukaj po nazwie pliku, ścieżce, identyfikatorze sesji lub encji",
      "empty": {
        "title": "Brak obrazów dla tego widoku",
        "description": "Spróbuj innego filtra lub hasła wyszukiwania. Biblioteka pokazuje tylko obrazy zapisane już w lokalnej pamięci aplikacji."
      },
      "actions": {
        "sort": "Sortuj",
        "useThis": "Użyj tego",
        "using": "Używanie...",
        "copyPath": "Kopiuj ścieżkę",
        "saving": "Zapisywanie...",
        "download": "Pobierz",
        "delete": "Usuń obraz",
        "deleting": "Usuwanie..."
      },
      "active": "Aktywne",
      "messages": {
        "loadFailed": "Nie udało się załadować biblioteki obrazów",
        "saved": "Obraz zapisany",
        "downloadFailed": "Pobieranie nie powiodło się",
        "useFailed": "Nie udało się użyć tego obrazu",
        "deleted": "Obraz usunięty",
        "deleteFailed": "Nie udało się usunąć obrazu"
      },
      "deleteConfirm": {
        "title": "Usunąć obraz?",
        "message": "Na pewno usunąć \"{{filename}}\"? Może to zepsuć awatary, tła czatu lub załączniki wiadomości, które nadal go używają."
      },
      "sort": {
        "newest": "Newest",
        "largest": "Largest",
        "name": "Name"
      },
      "kinds": {
        "background": "Background",
        "avatar": "Avatar",
        "attachment": "Attachment",
        "stored": "Stored"
      },
      "detailsTitle": "{{kind}} details",
      "formatsLabel": "Formats",
      "storagePath": "Storage path",
      "contextLabel": "Context",
      "contextLinkedFallback": "Linked",
      "show": "Show",
      "hide": "Hide",
      "contextRoles": {
        "character": "character:",
        "session": "session:",
        "role": "role:"
      },
      "downloadFormat": "{{download}} Format",
      "unknownDate": "Unknown",
      "clearSearch": "Clear search",
      "copyFilename": "Copy filename",
      "copyLabels": {
        "filename": "Filename",
        "storagePath": "Storage path"
      },
      "copy": {
        "copied": "{{label}} copied",
        "failed": "Failed to copy {{label}}"
      }
    },
    "deleteConfirm": {
      "title": "Usunąć {{itemType}}?",
      "message": "Czy na pewno chcesz usunąć",
      "characterWarning": "Spowoduje to również usunięcie wszystkich sesji czatu z tą postacią."
    },
    "rename": {
      "title": "Zmień nazwę lorebooka",
      "placeholder": "Wprowadź nową nazwę..."
    },
    "itemTypes": {
      "character": "Postać",
      "persona": "Persona",
      "lorebook": "Lorebook"
    },
    "lorebookLabel": "Lorebook",
    "noDescriptionYet": "Brak opisu",
    "errors": {
      "importLorebook": "Failed to import lorebook. {{error}}",
      "exportFailed": "Export failed"
    },
    "card": {
      "avatarAlt": "{{name}} avatar"
    },
    "lorebookEditor": {
      "titleOverride": "Lorebook - {{name}}",
      "dragToReorder": "Drag to reorder",
      "aria": {
        "generateEntry": "Generate lorebook entry",
        "editLorebook": "Edit lorebook",
        "exportLorebook": "Export lorebook"
      }
    }
  },
  "onboarding": {
    "loading": "Ładowanie dostawców...",
    "stepIndicator": "Krok {{current}} z {{total}}",
    "steps": {
      "provider": "Konfiguracja dostawcy",
      "model": "Konfiguracja modelu",
      "memory": "System pamięci",
      "stepNofM": "Krok {{current}} z {{total}}"
    },
    "provider": {
      "availableProviders": "Dostępni dostawcy",
      "chooseProvider": "Wybierz dostawcę",
      "titleMobile": "Wybierz swojego dostawcę AI",
      "descMobile": "Wybierz dostawcę AI, aby zacząć. Twoje klucze API są bezpiecznie zaszyfrowane na urządzeniu. Nie potrzebujesz konta.",
      "configureProvider": "Skonfiguruj {{name}}",
      "connectProvider": "Połącz {{name}}",
      "connectProviderDesc": "Wklej swój klucz API poniżej, aby włączyć czaty. Potrzebujesz klucza? Pobierz go z panelu dostawcy.",
      "localLLMs": "Lokalne LLM",
      "useLocalLLMs": "Chcę używać lokalnych LLM",
      "browseModelLibrary": "Przeglądaj bibliotekę modeli",
      "browseModelLibraryDesc": "Wyszukuj i pobieraj modele GGUF z HuggingFace",
      "useOwnGguf": "Użyj własnych plików GGUF",
      "useOwnGgufDesc": "Wybierz model GGUF i opcjonalny plik mmproj ze swojego urządzenia",
      "fields": {
        "displayLabel": "Etykieta wyświetlana",
        "displayLabelHint": "Jak ten dostawca będzie wyglądał w menu",
        "displayLabelPlaceholder": "Mój {{name}}",
        "defaultLabelFallback": "Dostawca",
        "apiKey": "Klucz API",
        "apiKeyOptional": "Klucz API (opcjonalnie)",
        "apiKeyHint": "Klucze są zaszyfrowane lokalnie",
        "apiKeyPlaceholderRemote": "sk-...",
        "apiKeyPlaceholderLocal": "Zazwyczaj nie wymagany",
        "whereToFind": "Gdzie go znaleźć",
        "baseUrl": "Bazowy URL",
        "baseUrlPlaceholderHost": "http://192.168.1.10:3333",
        "baseUrlPlaceholderLocal": "http://localhost:11434",
        "baseUrlPlaceholderRemote": "https://api.provider.com",
        "baseUrlPlaceholderIntenseRP": "http://127.0.0.1:7777/v1",
        "baseUrlHintLocal": "Adres lokalnego serwera z portem",
        "baseUrlHintHost": "Wprowadź URL hosta pulpitu pokazany przez urządzenie hosta",
        "baseUrlHintRemote": "Zastąp domyślny endpoint jeśli potrzeba",
        "chatEndpoint": "Endpoint czatu",
        "systemRole": "Rola systemowa",
        "userRole": "Rola użytkownika",
        "assistantRole": "Rola asystenta",
        "supportsStreaming": "Obsługuje strumieniowanie",
        "mergeSameRole": "Scalaj wiadomości tej samej roli",
        "toolChoiceMode": "Tryb wyboru narzędzia",
        "toolChoiceHint": "Kontroluje jak tool_choice jest wysyłane do niestandardowego endpointu."
      },
      "toolChoice": {
        "auto": "Auto",
        "required": "Wymagane",
        "none": "Brak",
        "omit": "Pomiń pole",
        "passthrough": "Passthrough (Konfiguracja narzędzia)"
      },
      "buttons": {
        "testConnection": "Testuj połączenie",
        "testing": "Testowanie..."
      },
      "descriptions": {
        "chutes": "Kompatybilna z OpenAI inferencja dla najlepszych modeli open-source",
        "openai": "Modele GPT-5, GPT-4.1 i GPT-4o do ekspresyjnych RP",
        "lettuceHost": "Połącz z własnym pulpitem Lettuce Host przez LAN z API w stylu OpenAI",
        "anthropic": "Claude 4.5 Sonnet i Haiku do głębokich, emocjonalnych dialogów",
        "aggregator": "Dostęp do modeli jak GPT-5, Claude 4.5, Grok-3, Mixtral i więcej",
        "openaiCompatible": "Użyj dowolnego endpointu API w stylu OpenAI",
        "mistral": "Mistral Small 3.2, Mixtral 8x22B i inne modele Mistral",
        "deepseek": "DeepSeek-V3.2-Exp, DeepSeek-R1 i inne wysokowydajne modele",
        "xai": "Grok-1.5, Grok-3 i nowsze modele xAI",
        "zai": "GLM-4.5, GLM-4.6 i warianty Air",
        "moonshot": "Kimi-K2 Thinking i modele Kimi-K1",
        "gemini": "Gemini 2.5 Flash, 2.5 Pro i więcej",
        "qwen": "Qwen3-VL i nowsze modele Qwen",
        "nvidia": "Nemotron, Llama, DeepSeek i więcej przez NVIDIA NIM",
        "custom": "Skieruj LettuceAI na dowolny niestandardowy endpoint modelu",
        "fallback": "Dostawca modelu AI"
      },
      "descriptionsShort": {
        "chutes": "Inferencja modeli open-source",
        "openai": "GPT-5, GPT-4o, GPT-4.1",
        "lettuceHost": "Twój własny host LAN",
        "anthropic": "Claude 4.5 Sonnet i Haiku",
        "aggregator": "Agregator wielu modeli",
        "openaiCompatible": "Niestandardowy endpoint OpenAI",
        "mistral": "Modele Mistral i Mixtral",
        "deepseek": "DeepSeek-V3, R1",
        "xai": "Grok-1.5, Grok-3",
        "zai": "GLM-4.5, GLM-4.6",
        "moonshot": "Kimi-K2 Thinking",
        "gemini": "Gemini 2.5 Flash i Pro",
        "qwen": "Modele Qwen3-VL",
        "nvidia": "Inferencja NVIDIA NIM",
        "custom": "Niestandardowy endpoint",
        "fallback": "Dostawca AI"
      },
      "cardLabel": "{{step}}: {{name}}",
      "errors": {
        "hostUrlRequired": "URL hosta jest wymagany (np. http://192.168.1.10:3333)",
        "baseUrlRequired": "Bazowy URL jest wymagany (np. http://localhost:11434)",
        "apiKeyTooShort": "Klucz API wydaje się za krótki",
        "invalidApiKey": "Nieprawidłowy klucz API",
        "connectionFailed": "Połączenie nie powiodło się",
        "verificationFailed": "Weryfikacja nie powiodła się",
        "failedToSave": "Nie udało się zapisać dostawcy",
        "connectionSuccessful": "Połączenie nawiązane pomyślnie!",
        "modelNotFound": "Nie znaleziono modelu u dostawcy",
        "modelVerificationFailed": "Weryfikacja modelu nie powiodła się",
        "failedToSaveModel": "Nie udało się zapisać modelu"
      }
    },
    "model": {
      "noProvidersTitle": "Nie skonfigurowano dostawców",
      "noProvidersDesc": "Musisz połączyć dostawcę przed wybraniem domyślnego modelu.",
      "goToProviderSetup": "Przejdź do konfiguracji dostawcy",
      "yourProviders": "Twoi dostawcy",
      "yourProvidersHint": "Wybierz dostawcę do użycia",
      "setDefaultModel": "Ustaw domyślny model",
      "setDefaultModelDesc": "Wybierz dostawcę i nazwę modelu, których LettuceAI powinno używać domyślnie. Będziesz mógł dodać więcej później.",
      "setDefaultModelDescDesktop": "Wybierz dostawcę z listy, aby skonfigurować model.",
      "modelDetails": "Szczegóły modelu",
      "modelDetailsDesc": "Zdefiniuj identyfikator API i etykietę widoczną w aplikacji.",
      "whichModel": "Który model powinienem(-nnam) wybrać?",
      "nextMemorySystem": "Następny: System pamięci",
      "fields": {
        "displayName": "Nazwa wyświetlana",
        "displayNamePlaceholder": "Kreatywny mentor",
        "displayNameHint": "Jak ten model pojawia się w menu",
        "modelId": "ID modelu",
        "modelPathGguf": "Ścieżka modelu (GGUF)",
        "modelIdPlaceholder": "np. gpt-4o",
        "modelPathPlaceholder": "/ścieżka/do/modelu.gguf",
        "modelIdHint": "Dokładny identyfikator używany do wywołań API",
        "showList": "Pokaż listę",
        "manualInput": "Ręczne wprowadzanie",
        "refreshModelList": "Odśwież listę modeli",
        "selectModel": "Wybierz model",
        "selectAModel": "Wybierz model...",
        "searchModels": "Szukaj modeli...",
        "noModelsFound": "Nie znaleziono modeli pasujących do \"{{query}}\""
      },
      "fillBothFields": "Wypełnij oba powyższe pola, aby aktywować przycisk zakończenia.",
      "providerNames": {
        "chutes": "Chutes",
        "intenseRpNext": "IntenseRP Next",
        "openai": "OpenAI",
        "anthropic": "Anthropic",
        "openrouter": "OpenRouter",
        "openaiCompatible": "Kompatybilny z OpenAI",
        "custom": "Niestandardowy endpoint"
      }
    },
    "memory": {
      "dynamicTitle": "Pamięć dynamiczna",
      "recommended": "Zalecane",
      "settingUp": "Konfigurowanie...",
      "finishSetup": "Zakończ konfigurację",
      "promptTitle": "Skonfiguruj pamięć dynamiczną",
      "oneLastStep": "Ostatni krok",
      "downloadAndEnable": "Pobierz i włącz",
      "chooseStyle": "Wybierz styl pamięci",
      "howRemember": "Jak twoi towarzysze AI powinni pamiętać szczegóły o tobie i rozmowach?",
      "dynamicDescription": "Używa <0>lokalnego modelu embeddingu</0> do inteligentnego zarządzania kontekstem. Zmniejsza koszty tokenów zachowując wysoką jakość, nawet w długich czatach.",
      "dynamicFeatures": {
        "quality": "Utrzymuje jakość w długich czatach",
        "cost": "Znacznie obniża koszty API",
        "auto": "Automatyczne zarządzanie kontekstem",
        "zeroConfig": "Zero konfiguracji"
      },
      "manualTitle": "Pamięć ręczna",
      "manualBadge": "Klasyczne doświadczenie",
      "manualDescription": "Ręcznie przypinasz wiadomości i edytujesz \"World Info\" lub definicje postaci. Dobre do pełnej kontroli.",
      "manualFeatures": {
        "control": "Pełna kontrola nad faktami",
        "scenarios": "Najlepsze do konkretnych scenariuszy"
      },
      "setupModelMessage": "Aby używać pamięci dynamicznej, musimy pobrać mały model embeddingu (~120MB) na twoje urządzenie.",
      "setupBullets": {
        "offline": "Model działa w 100% offline na twoim urządzeniu",
        "remembering": "Wymagany do zapamiętywania kontekstu",
        "disable": "Możesz to wyłączyć później w ustawieniach"
      },
      "stepLabel": "Krok 3 z 3",
      "stepLabelMemory": "System pamięci"
    },
    "welcome": {
      "appName": "LettuceAI",
      "tagline": "Twój osobisty towarzysz AI. Prywatny, bezpieczny i zawsze na urządzeniu.",
      "features": {
        "onDevice": "Tylko na urządzeniu",
        "characterReady": "Postać gotowa"
      },
      "betaWarning": {
        "title": "Wersja beta Desktop",
        "description": "Używasz wersji desktopowej. Niektóre funkcje mogą się różnić od mobilnych. Zgłaszaj problemy na GitHub."
      },
      "languageSelector": {
        "title": "Język",
        "description": "Wykryto automatycznie z urządzenia. Możesz zmienić w dowolnym momencie w ustawieniach."
      },
      "getStarted": "Rozpocznij",
      "skipForNow": "Pomiń na razie",
      "restoreFromBackup": "Przywróć z kopii zapasowej",
      "setupTime": "Konfiguracja zajmuje mniej niż 2 minuty",
      "skipWarning": {
        "title": "Pominąć konfigurację?",
        "warningTitle": "Dostawca potrzebny do czatu",
        "warningMessage": "Bez dostawcy nie będziesz mógł wysyłać wiadomości. Możesz dodać go później w ustawieniach.",
        "addProvider": "Dodaj dostawcę",
        "skipAnyway": "Pomiń mimo to"
      },
      "restoreBackup": {
        "title": "Przywróć kopię zapasową",
        "selectMessage": "Wybierz kopię zapasową do przywrócenia.",
        "browse": "Przeglądaj pliki",
        "processing": "Przetwarzanie pliku...",
        "processingNote": "Duże kopie zapasowe mogą zająć minutę",
        "noBackups": "Nie znaleziono kopii zapasowych",
        "noBackupsHint": "Stuknij przeglądaj, aby wybrać plik .lettuce",
        "browseLettuce": "Przeglądaj pliki .lettuce",
        "passwordLabel": "Hasło kopii zapasowej",
        "passwordPlaceholder": "Wprowadź hasło",
        "restoreButton": "Przywróć kopię zapasową",
        "restoring": "Przywracanie...",
        "infoMessage": "Spowoduje to skonfigurowanie aplikacji z twoimi danymi, w tym postaciami, czatami i ustawieniami.",
        "embeddingTitle": "Wymagany model embeddingu",
        "dynamicMemoryDetected": "Wykryto pamięć dynamiczną",
        "dynamicMemoryMessage": "Ta kopia zapasowa zawiera postacie z włączoną pamięcią dynamiczną, która wymaga modelu embeddingu (~120MB).",
        "embeddingOptions": "Możesz pobrać model teraz, aby włączyć pamięć dynamiczną, lub kontynuować bez niej (pamięć dynamiczna zostanie wyłączona dla dotkniętych postaci).",
        "downloadModel": "Pobierz model",
        "continueWithoutDynamic": "Kontynuuj bez pamięci dynamicznej",
        "embeddingNote": "Możesz ponownie włączyć pamięć dynamiczną później w ustawieniach postaci po pobraniu modelu.",
        "back": "Wstecz",
        "cancel": "Anuluj",
        "errors": {
          "passwordRequired": "Hasło jest wymagane",
          "incorrectPassword": "Nieprawidłowe hasło",
          "failedToOpenFile": "Nie udało się otworzyć pliku",
          "failedToRestore": "Nie udało się przywrócić kopii zapasowej",
          "failedToUpdateSettings": "Nie udało się zaktualizować ustawień"
        }
      }
    },
    "common": {
      "back": "Wstecz",
      "cancel": "Anuluj",
      "continue": "Kontynuuj",
      "verifying": "Weryfikowanie...",
      "skipForNow": "Pomiń na razie",
      "selectAProvider": "Wybierz dostawcę do skonfigurowania",
      "clickToSelectProvider": "Kliknij, aby wybrać dostawcę",
      "selectProviderFromList": "Wybierz dostawcę z listy, aby zacząć.",
      "enterApiKey": "Wprowadź swój klucz API, aby włączyć funkcję czatu AI."
    },
    "modelGuide": {
      "badge": "Przewodnik po modelach",
      "title": "Jak wybrać model?",
      "intro": "LettuceAI nie narzuca jednego \"najlepszego\" modelu. Zamiast tego wybierasz, co pasuje do twojego <0>przypadku użycia, budżetu i stylu</0>. Użyj tego przewodnika, aby zdecydować, czego spróbować i gdzie szukać.",
      "askYourself": "Zapytaj siebie:",
      "factors": {
        "quality": {
          "title": "Jakość i możliwości",
          "description": "Jak inteligentny musi być model? Większe, nowsze modele zazwyczaj lepiej rozumują, piszą ładniejszy tekst i obsługują nieporządne prompty bardziej elegancko.",
          "q1": "Czy potrzebujesz głębokiej spójności postaci i inteligencji emocjonalnej?",
          "q2": "Czy zależy ci na immersyjnym opowiadaniu i wiarygodnych osobowościach postaci?",
          "q3": "Czy chcesz, aby model pamiętał szczegóły postaci i pozostawał w roli przez długie sesje?"
        },
        "speed": {
          "title": "Szybkość i opóźnienia",
          "description": "Szybsze modele lepiej sprawdzają się w gadatliwych rozmowach. Niektóre modele poświęcają trochę jakości dla znacznie większej szybkości.",
          "q1": "Czy chcesz prawie natychmiastowych odpowiedzi, aby RP płynęło naturalnie?",
          "q2": "Czy prowadzisz szybkie sceny dialogowe, gdzie czekanie psułoby immersję?",
          "q3": "Czy to do casualowego RP, gdzie szybka wymiana jest ważniejsza niż idealne odpowiedzi?"
        },
        "budget": {
          "title": "Budżet i użytkowanie",
          "description": "Każdy dostawca rozlicza tokeny. Nawet tanie modele kosztują jeśli dużo rozmawiasz, więc wybierz coś, co pasuje do częstotliwości i intensywności użycia.",
          "q1": "Czy możesz więcej płacić za bogatsze interakcje z postacią, czy wolisz coś taniego do codziennego RP?",
          "q2": "Czy masz darmowe modele od dostawcy/routera, które możesz wypróbować najpierw?",
          "q3": "Czy będziesz prowadzić długie sesje RPG ze szczegółowymi opisami scen?",
          "q4": "Czy masz twardy miesięczny budżet, którego nie chcesz przekraczać?"
        },
        "safety": {
          "title": "Bezpieczeństwo, prywatność i dodatki",
          "description": "Dostawcy różnią się w obsłudze bezpieczeństwa, logowania i dodatkowych funkcji jak obrazy, narzędzia czy długie okna kontekstu.",
          "q1": "Czy potrzebujesz mniej filtrów treści do dojrzałych lub kreatywnych scenariuszy RPG?",
          "q2": "Czy zależy ci, czy twoje prywatne rozmowy RP są rejestrowane lub używane do treningu?",
          "q3": "Czy potrzebujesz długich okien kontekstu do złożonych fabuł i historii postaci?"
        }
      },
      "where": {
        "title": "Gdzie mogę znaleźć modele?",
        "intro": "Większość dostawców i routerów ma <0>listę lub katalog modeli</0>. Przeglądaj te strony, aby zobaczyć ofertę, ceny, limity i specjalne funkcje.",
        "directTitle": "Bezpośredni dostawcy",
        "directDesc": "OpenAI, Anthropic, Google Gemini, xAI, Mistral itp. Każdy ma konsolę/panel, gdzie można zobaczyć oficjalne nazwy modeli, możliwości i ceny.",
        "routersTitle": "Routery i huby",
        "routersDesc": "Usługi jak OpenRouter lub inne agregatory listują wiele modeli od różnych dostawców w jednym miejscu, często z benchmarkami i porównaniami cen.",
        "communityTitle": "Rekomendacje społeczności",
        "communityDesc": "Sprawdź dokumentację, blogi lub posty społeczności od dostawcy/routera. Zazwyczaj wskazują, które modele są najlepsze do czatu, kodowania lub szybkości."
      },
      "rules": {
        "title": "Proste zasady",
        "casual": "Do casualowego czatu: wybierz szybki, tani model czatowy od dostawcy/routera.",
        "experiments": "Do eksperymentów lub dużej liczby zapytań: zacznij od najtańszego modelu, który czuje się wystarczająco dobry, a potem ulepszaj jeśli potrzeba.",
        "switch": "Jeśli coś nie gra (za wolne / za głupie / za drogie): zawsze możesz zmienić modele później w LettuceAI."
      },
      "disclaimer": "Zawsze sprawdzaj dokumentację dostawcy, aby uzyskać aktualną listę modeli, limity i ceny. Ta strona jest o sposobie myślenia, nie o tym, co kupować."
    },
    "whereToFind": {
      "badge": "Pomoc z kluczem API",
      "intro": "Wykonaj te kroki, aby uzyskać klucz API, a następnie wróć do LettuceAI i wklej go w ustawieniach dostawcy.",
      "readyPrompt": "Gotowy(-a) na pobranie klucza?",
      "openProviderSite": "Otwórz stronę dostawcy",
      "keyWarning": "Nigdy nie udostępniaj swojego klucza API publicznie. Każdy z tym kluczem może używać twojego salda konta.",
      "stuckPrompt": "Nadal nie możesz tego rozgryźć?",
      "joinDiscord": "Dołącz do naszego serwera Discord, aby uzyskać pomoc",
      "guides": {
        "chutes": {
          "title": "Jak znaleźć klucz API Chutes",
          "s1": "Przejdź na chutes.ai/app i zaloguj się.",
          "s2": "Otwórz obszar konta/ustawień i znajdź Klucze API.",
          "s3": "Utwórz nowy klucz (lub skopiuj istniejący).",
          "s4": "Wklej klucz do LettuceAI."
        },
        "openai": {
          "title": "Jak znaleźć klucz API OpenAI",
          "s1": "Przejdź na platform.openai.com i zaloguj się.",
          "s2": "Kliknij awatar profilu w prawym górnym rogu, a następnie wybierz Klucze API.",
          "s3": "Kliknij Utwórz nowy tajny klucz i skopiuj pokazaną wartość.",
          "s4": "Wklej klucz do LettuceAI i przechowaj go w bezpiecznym miejscu. Nie zobaczysz go ponownie."
        },
        "anthropic": {
          "title": "Jak znaleźć klucz API Anthropic",
          "s1": "Przejdź na console.anthropic.com i zaloguj się.",
          "s2": "Otwórz Ustawienia z lewego paska bocznego.",
          "s3": "Wybierz Klucze API i kliknij Utwórz klucz.",
          "s4": "Skopiuj klucz i wklej go do LettuceAI."
        },
        "openrouter": {
          "title": "Jak znaleźć klucz API OpenRouter",
          "s1": "Odwiedź openrouter.ai i zaloguj się.",
          "s2": "Otwórz stronę Klucze z menu profilu.",
          "s3": "Kliknij Utwórz klucz, nadaj mu nazwę i zapisz.",
          "s4": "Skopiuj klucz i wklej go do LettuceAI."
        },
        "mistral": {
          "title": "Jak znaleźć klucz API Mistral",
          "s1": "Przejdź na console.mistral.ai i zaloguj się.",
          "s2": "Kliknij Klucze API w pasku bocznym.",
          "s3": "Kliknij Utwórz klucz API.",
          "s4": "Skopiuj klucz i wklej go do LettuceAI."
        },
        "deepseek": {
          "title": "Jak znaleźć klucz API DeepSeek",
          "s1": "Otwórz platform.deepseek.com i zaloguj się.",
          "s2": "Kliknij Klucze API w górnej nawigacji.",
          "s3": "Utwórz nowy klucz, jeśli jeszcze go nie masz.",
          "s4": "Skopiuj klucz i wklej go do LettuceAI."
        },
        "groq": {
          "title": "Jak znaleźć klucz API Groq",
          "s1": "Odwiedź console.groq.com i zaloguj się.",
          "s2": "Otwórz Klucze API z paska bocznego.",
          "s3": "Utwórz nowy klucz, a następnie go skopiuj.",
          "s4": "Wklej klucz do LettuceAI."
        },
        "gemini": {
          "title": "Jak znaleźć klucz API Google Gemini",
          "s1": "Przejdź do Google AI Studio na aistudio.google.com i zaloguj się.",
          "s2": "Kliknij Pobierz klucz API lub Zarządzaj kluczami.",
          "s3": "Utwórz nowy klucz jeśli potrzeba.",
          "s4": "Skopiuj klucz i wklej go do LettuceAI."
        },
        "xai": {
          "title": "Jak znaleźć klucz API xAI",
          "s1": "Otwórz console.x.ai i zaloguj się.",
          "s2": "Przejdź do sekcji Klucze API w konsoli.",
          "s3": "Utwórz nowy klucz.",
          "s4": "Skopiuj klucz i wklej go do LettuceAI."
        },
        "zai": {
          "title": "Jak znaleźć klucz API zAI (GLM)",
          "s1": "Przejdź na open.bigmodel.cn i zaloguj się.",
          "s2": "Otwórz Centrum użytkownika, a następnie przejdź do Kluczy API.",
          "s3": "Utwórz nowy klucz jeśli go nie masz.",
          "s4": "Skopiuj klucz i wklej go do LettuceAI."
        },
        "moonshot": {
          "title": "Jak znaleźć klucz API Moonshot (Kimi)",
          "s1": "Odwiedź platform.moonshot.cn i zaloguj się.",
          "s2": "Otwórz sekcję Klucze API w konsoli.",
          "s3": "Utwórz nowy klucz i go skopiuj.",
          "s4": "Wklej klucz do LettuceAI."
        },
        "qwen": {
          "title": "Jak znaleźć klucz API Qwen",
          "s1": "Otwórz dashscope.aliyun.com i zaloguj się.",
          "s2": "Przejdź do sekcji Klucze API w pasku bocznym.",
          "s3": "Utwórz nowy klucz.",
          "s4": "Skopiuj klucz i wklej go do LettuceAI."
        },
        "nanogpt": {
          "title": "Jak znaleźć klucz API NanoGPT",
          "s1": "Przejdź na nano-gpt.com i zaloguj się.",
          "s2": "Otwórz panel i przejdź do sekcji kluczy API.",
          "s3": "Utwórz nowy klucz jeśli potrzeba.",
          "s4": "Skopiuj klucz i wklej go do LettuceAI."
        },
        "featherless": {
          "title": "Jak znaleźć klucz API Featherless",
          "s1": "Odwiedź featherless.ai i zaloguj się.",
          "s2": "Otwórz konto lub sekcję API z panelu.",
          "s3": "Utwórz nowy klucz jeśli go nie widzisz.",
          "s4": "Skopiuj klucz i wklej go do LettuceAI."
        },
        "anannas": {
          "title": "Jak znaleźć klucz API Anannas",
          "s1": "Przejdź na dashboard.anannas.ai i zaloguj się.",
          "s2": "Przejdź do sekcji Klucze API.",
          "s3": "Utwórz nowy klucz i go skopiuj.",
          "s4": "Wklej klucz do LettuceAI."
        },
        "default": {
          "title": "Jak znaleźć swój klucz API",
          "s1": "Otwórz panel swojego dostawcy AI w przeglądarce i zaloguj się.",
          "s2": "Szukaj ustawień API, Developer lub Integrations.",
          "s3": "Utwórz nowy klucz API lub wyświetl istniejący.",
          "s4": "Skopiuj klucz i wklej go do LettuceAI."
        }
      }
    },
    "welcomeFooter": {
      "setupTimeMobile": "Konfiguracja zajmuje mniej niż 2 minuty"
    }
  },
  "search": {
    "placeholder": "Szukaj...",
    "tabs": {
      "characters": "Postacie",
      "personas": "Persony"
    },
    "noResults": "Nie znaleziono {{type}}",
    "emptyState": "Brak {{type}}",
    "noResultsHint": "Spróbuj innego wyszukiwania",
    "emptyCharacters": "Utwórz swoją pierwszą postać, aby zacząć rozmawiać",
    "emptyPersonas": "Utwórz personę w ustawieniach",
    "a11y": {
      "goBack": "Go back",
      "clearSearch": "Clear search",
      "characterAvatar": "{{name}} avatar"
    },
    "session": {
      "newChatTitle": "New Chat"
    },
    "noDescription": "No description",
    "defaultBadge": "Default"
  },
  "sync": {
    "modes": {
      "join": "Dołącz",
      "joinDesc": "Połącz z hostem",
      "host": "Host",
      "hostDesc": "Udostępnij swoje dane"
    },
    "sections": {
      "mode": "Tryb",
      "connectToHost": "Połącz z hostem",
      "startHosting": "Rozpocznij hosting",
      "status": "Status",
      "hosting": "Usługa hostingu",
      "localAddress": "Adres sieci lokalnej",
      "connectionPin": "PIN połączenia",
      "setupGuide": "Przewodnik konfiguracji"
    },
    "fields": {
      "hostAddress": "Adres hosta lub JSON",
      "hostPlaceholder": "np. 192.168.1.100:12345",
      "pinCode": "Kod PIN",
      "pinPlaceholder": "000000"
    },
    "buttons": {
      "scanQRCode": "Skanuj kod QR",
      "connect": "Połącz",
      "connecting": "Łączenie...",
      "startHosting": "Rozpocznij hosting",
      "startingServer": "Uruchamianie serwera...",
      "stopHosting": "Zatrzymaj hosting",
      "hostAgain": "Hostuj ponownie",
      "done": "Gotowe"
    },
    "status": {
      "connecting": "Łączenie...",
      "connected": "Połączono",
      "waitingConfirmation": "Czekam na potwierdzenie",
      "waitingConfirmationDesc": "Aby kontynuować, zatwierdź połączenie na urządzeniu hosta.",
      "syncing": "Synchronizowanie...",
      "transferringData": "Przesyłanie danych",
      "syncInProgress": "Synchronizacja w toku",
      "live": "Na żywo",
      "broadcasting": "Nadawanie",
      "clientsLabel": "Połączeni",
      "clientsUnit": "Klienci"
    },
    "pinDescription": "Udostępnij ten PIN łączącemu się urządzeniu",
    "hostingDesc1": "Inne urządzenia mogą się połączyć i zsynchronizować dane z tego urządzenia.",
    "hostingDesc2": "Twoje dane będą udostępniane połączonym klientom.",
    "setupSteps": {
      "step1": "Otwórz aplikację na innym urządzeniu",
      "step2": "Przejdź do Ustawienia → Synchronizacja lokalna",
      "step3": "Zeskanuj kod QR lub wprowadź adres"
    },
    "messages": {
      "completed": "Synchronizacja zakończona!",
      "completedDesc": "Wszystkie dane zsynchronizowane",
      "error": "Błąd połączenia",
      "outdatedClient": "Wykryto przestarzałego klienta"
    },
    "disclaimer": "Synchronizacja działa w sieci lokalnej. Oba urządzenia muszą być w tej samej sieci WiFi.",
    "modals": {
      "connectionRequest": "Żądanie połączenia",
      "requestMessage": "chce się zsynchronizować z tym urządzeniem.",
      "acceptConnection": "Akceptuj połączenie",
      "acceptDesc": "Pozwól temu urządzeniu synchronizować dane",
      "decline": "Odrzuć",
      "declineDesc": "Zablokuj tę próbę połączenia",
      "readyToSync": "Gotowe do synchronizacji",
      "connectionEstablished": "Połączenie nawiązane",
      "deviceReady": "jest gotowe.",
      "startSyncMessage": "Stuknij poniżej, aby rozpocząć synchronizację danych.",
      "startSyncing": "Rozpocznij synchronizację",
      "startSyncingDesc": "Rozpocznij transfer danych teraz"
    },
    "scanner": {
      "title": "Skanuj kod QR",
      "cancel": "Anuluj skanowanie"
    },
    "unknownDevice": "Unknown Device",
    "aria": {
      "dismissStatus": "Dismiss sync status",
      "dismissError": "Dismiss sync error"
    },
    "stats": {
      "statusLabel": "Status"
    }
  },
  "creationHelper": {
    "page": {
      "info": "Asystent Tworzenia prowadzi Cię przez budowanie postaci z pomocą AI. Skonfiguruj model i narzędzia używane podczas tworzenia postaci.",
      "modelConfiguration": "Konfiguracja Modelu",
      "chatModel": "Model Czatu",
      "selectedModel": "Wybrany Model",
      "useAppDefault": "Użyj domyślnego aplikacji{{model}}",
      "useAppDefaultBase": "Użyj domyślnego aplikacji",
      "noModelsAvailable": "Brak dostępnych modeli",
      "chatModelDescription": "Model AI do rozmów podczas tworzenia postaci",
      "streamingOutput": "Wyjście Strumieniowe",
      "streamingDescription": "Pokazuj odpowiedzi w trakcie generowania",
      "imageGenerationModel": "Model Generowania Obrazów",
      "noModelSelected": "Nie wybrano modelu",
      "noImageModelsAvailable": "Brak dostępnych modeli obrazów",
      "imageModelDescription": "Do generowania awatarów postaci",
      "toolSelection": "Wybór Narzędzi",
      "smartToolSelection": "Inteligentny Wybór Narzędzi",
      "smartToolDescription": "AI automatycznie wybiera, których narzędzi użyć",
      "smartToolEnabledHint": "Po włączeniu Asystent Tworzenia AI pyta, co chcesz stworzyć i ładuje tylko odpowiedni zestaw narzędzi.",
      "smartToolDisabledHint": "Po wyłączeniu Asystent Tworzenia AI otwiera się bezpośrednio i używa wszystkich włączonych narzędzi; asystent decyduje, co stworzyć.",
      "quickPresets": "Szybkie Presety",
      "customSelection": "Niestandardowy wybór - {{count}} narzędzi włączonych",
      "footerInfo": "Gdy Inteligentny Wybór Narzędzi jest włączony, AI decyduje, których narzędzi użyć na podstawie kontekstu. Wyłącz go, aby ręcznie kontrolować dostępne narzędzia.",
      "selectChatModel": "Wybierz Model Czatu",
      "selectImageModel": "Wybierz Model Obrazów",
      "searchModels": "Szukaj modeli..."
    },
    "categories": {
      "basic": "Podstawowe",
      "content": "Treść",
      "visual": "Wizualne",
      "settings": "Ustawienia",
      "flow": "Przebieg",
      "persona": "Persony",
      "lorebook": "Lorebooki"
    },
    "presets": {
      "all": {
        "name": "Wszystkie narzędzia",
        "desc": "Włącz wszystkie dostępne narzędzia"
      },
      "essential": {
        "name": "Podstawowe",
        "desc": "Tylko imię, definicja i sceny"
      },
      "minimal": {
        "name": "Minimalne",
        "desc": "Tylko imię i definicja"
      }
    },
    "tools": {
      "setName": "Ustaw imię",
      "setNameDesc": "Ustaw imię postaci",
      "setDefinition": "Ustaw definicję",
      "setDefinitionDesc": "Ustaw osobowość i tło",
      "set_character_name": {
        "name": "Ustaw imię",
        "desc": "Ustaw imię postaci"
      },
      "set_character_definition": {
        "name": "Ustaw definicję",
        "desc": "Ustaw osobowość i tło"
      },
      "add_scene": {
        "name": "Dodaj scenę",
        "desc": "Dodaj scenę początkową do roleplayu"
      },
      "update_scene": {
        "name": "Zaktualizuj scenę",
        "desc": "Zmodyfikuj istniejącą scenę"
      },
      "toggle_avatar_gradient": {
        "name": "Gradient awatara",
        "desc": "Przełącz nakładkę gradientu na awatarze"
      },
      "set_default_model": {
        "name": "Ustaw model",
        "desc": "Ustaw model AI do rozmów"
      },
      "set_system_prompt": {
        "name": "Prompt systemowy",
        "desc": "Ustaw wytyczne zachowania"
      },
      "get_system_prompt_list": {
        "name": "Lista promptów",
        "desc": "Zobacz dostępne prompty"
      },
      "get_model_list": {
        "name": "Lista modeli",
        "desc": "Zobacz dostępne modele"
      },
      "use_uploaded_image_as_avatar": {
        "name": "Obraz jako awatar",
        "desc": "Użyj przesłanego obrazu jako awatara"
      },
      "use_uploaded_image_as_chat_background": {
        "name": "Obraz jako tło",
        "desc": "Użyj przesłanego obrazu jako tła"
      },
      "generate_image": {
        "name": "Generuj obraz",
        "desc": "Wygeneruj obraz za pomocą modelu AI"
      },
      "show_preview": {
        "name": "Pokaż podgląd",
        "desc": "Podgląd postaci"
      },
      "request_confirmation": {
        "name": "Poproś o potwierdzenie",
        "desc": "Zapytaj o zapisanie lub kontynuowanie"
      },
      "list_personas": {
        "name": "Lista person",
        "desc": "Przeglądaj persony"
      },
      "upsert_persona": {
        "name": "Zapisz personę",
        "desc": "Utwórz lub zaktualizuj personę"
      },
      "use_uploaded_image_as_persona_avatar": {
        "name": "Awatar persony",
        "desc": "Użyj przesłanego obrazu jako awatara persony"
      },
      "delete_persona": {
        "name": "Usuń personę",
        "desc": "Usuń personę"
      },
      "get_default_persona": {
        "name": "Domyślna persona",
        "desc": "Pobierz domyślną personę"
      },
      "list_lorebooks": {
        "name": "Lista lorebooków",
        "desc": "Przeglądaj lorebooki"
      },
      "upsert_lorebook": {
        "name": "Zapisz lorebook",
        "desc": "Utwórz lub zaktualizuj lorebook"
      },
      "delete_lorebook": {
        "name": "Usuń lorebook",
        "desc": "Usuń lorebook"
      },
      "list_lorebook_entries": {
        "name": "Lista wpisów",
        "desc": "Zobacz wpisy lorebooka"
      },
      "get_lorebook_entry": {
        "name": "Pobierz wpis",
        "desc": "Pobierz wpis lorebooka"
      },
      "upsert_lorebook_entry": {
        "name": "Zapisz wpis",
        "desc": "Utwórz lub zaktualizuj wpis"
      },
      "delete_lorebook_entry": {
        "name": "Usuń wpis",
        "desc": "Usuń wpis lorebooka"
      },
      "create_blank_lorebook_entry": {
        "name": "Pusty wpis",
        "desc": "Utwórz wpis zastępczy"
      },
      "reorder_lorebook_entries": {
        "name": "Zmień kolejność wpisów",
        "desc": "Zmień kolejność wpisów"
      },
      "list_character_lorebooks": {
        "name": "Lista lorebooków postaci",
        "desc": "Zobacz lorebooki dla postaci"
      },
      "set_character_lorebooks": {
        "name": "Ustaw lorebooki postaci",
        "desc": "Przypisz lorebooki do postaci"
      },
      "addScene": "Add Scene",
      "addSceneDesc": "Add a starting scene for roleplay",
      "updateScene": "Update Scene",
      "updateSceneDesc": "Modify an existing scene",
      "avatarGradient": "Avatar Gradient",
      "avatarGradientDesc": "Toggle gradient overlay on avatar",
      "setModel": "Set Model",
      "setModelDesc": "Set the AI model for conversations",
      "systemPrompt": "System Prompt",
      "systemPromptDesc": "Set behavioral guidelines",
      "listPrompts": "List Prompts",
      "listPromptsDesc": "View available prompts",
      "listModels": "List Models",
      "listModelsDesc": "View available models",
      "imageAsAvatar": "Image as Avatar",
      "imageAsAvatarDesc": "Use uploaded image as avatar"
    }
  },
  "tour": {
    "stepCounter": "Krok {{current}} z {{total}}",
    "skipTour": "Pomiń przewodnik",
    "next": "Dalej",
    "gotIt": "Rozumiem",
    "appShell": {
      "chats": {
        "title": "Tu mieszkają Twoje czaty",
        "body": "Wszystkie Twoje rozmowy jeden na jeden z postaciami są tutaj. Wróć kiedy chcesz, a zachowamy Twoje miejsce."
      },
      "groups": {
        "title": "Czaty grupowe",
        "body": "Zbierz wiele postaci w jednym pokoju i obserwuj, jak rozmawiają, lub dołącz sam, kiedy masz ochotę."
      },
      "discover": {
        "title": "Odkryj nowe postacie",
        "body": "Przeglądaj to, czym podzieliła się społeczność, i dodaj dowolną postać, która Ci się spodoba. Nowi ulubieńcy na wyciągnięcie ręki."
      },
      "library": {
        "title": "Twoja osobista biblioteka",
        "body": "Wszystko, co stworzyłeś lub zapisałeś, jest tutaj: postacie, persony, prompty, wszystko. Pomyśl o tym jak o swojej kolekcji."
      },
      "settings": {
        "title": "Dostosuj do siebie",
        "body": "Zmieniaj dostawców, wybieraj modele, dostosowuj wygląd. Praktycznie wszystko można konfigurować w ustawieniach."
      },
      "search": {
        "title": "Znajdź cokolwiek, szybko",
        "body": "Szukasz konkretnego czatu lub postaci? Przeszukuj wszystko stąd. Bez przekopywania."
      },
      "create": {
        "title": "I na koniec, twórz!",
        "body": "Stuknij plus, gdy przyjdzie inspiracja. Stwórz nową postać, personę lub zacznij coś od zera."
      }
    },
    "chatDetail": {
      "chatTitle": {
        "title": "Ustawienia per czat",
        "body": "Stuknij nazwę postaci u góry, aby otworzyć ustawienia tego czatu. Różne persony, układy i modele na rozmowę."
      },
      "chatMemory": {
        "title": "Co pamiętają",
        "body": "Ikona mózgu pokazuje, co Twoja postać pamięta z waszych rozmów. Stuknij, aby przejrzeć, edytować lub wyczyścić wspomnienia."
      },
      "chatSearch": {
        "title": "Znajdź tę jedną linijkę",
        "body": "Szukaj tylko w tej rozmowie. Świetne do odnalezienia szczegółu sprzed 200 wiadomości bez wiecznego przewijania."
      },
      "chatLorebook": {
        "title": "Wpisy lorebooka",
        "body": "Dodatkowe fakty, budowanie świata i kontekst wstrzykiwane do promptu, gdy pojawiają się określone słowa kluczowe. Ściągawka Twojej postaci."
      },
      "chatPlus": {
        "title": "Załącz rzeczy",
        "body": "Dodaj obrazy lub otwórz menu dodatków. To, co załączysz, zostanie wysłane z następną wiadomością."
      },
      "chatComposer": {
        "title": "Twoja wiadomość, Twój ruch",
        "body": "Pisz tutaj. Enter wysyła, Shift+Enter dodaje nową linię. Wskazówka: przytrzymaj dowolną wiadomość w czacie, aby ją edytować, rozgałęzić lub usunąć."
      },
      "chatSend": {
        "title": "Jeden przycisk, cztery zadania",
        "body": "Przycisk wysyłania zmienia swoje zadanie w zależności od tego, co się dzieje:"
      }
    },
    "postFirstMessage": {
      "chatRegenerate": {
        "title": "Nie podoba się? Regeneruj",
        "body": "Stuknij ikonę odświeżania, aby uzyskać zupełnie nową odpowiedź od postaci. Każda regeneracja jest zapisywana jako wariant, do którego możesz wrócić."
      },
      "chatVariants": {
        "title": "Przesuń między wariantami",
        "body": "Po regeneracji zobaczysz licznik wariantów pod wiadomością. Przesuń w lewo lub w prawo na dymku wiadomości, aby przeglądać różne odpowiedzi."
      },
      "chatLongPress": {
        "title": "Tu jest więcej ukrytych opcji",
        "body": "Przytrzymaj dowolną wiadomość, aby edytować, kopiować, rozgałęzić, przypiąć, usunąć lub cofnąć rozmowę. Na komputerze działa też prawy przycisk myszy."
      }
    },
    "sendButton": {
      "continue": {
        "label": "Kontynuuj",
        "desc": "Pole puste. Stuknięcie zachęci postać do dalszego mówienia."
      },
      "send": {
        "label": "Wyślij",
        "desc": "Wpisałeś tekst lub załączyłeś coś. Stuknij, aby wysłać."
      },
      "sending": {
        "label": "Wysyłanie",
        "desc": "Odpowiedź jest w drodze. Przycisk zablokowany."
      },
      "stop": {
        "label": "Zatrzymaj",
        "desc": "Stuknij, aby anulować w trakcie odpowiedzi, jeśli zmienisz zdanie."
      }
    },
    "extra": {
      "rerunOnboarding": "Rerun onboarding"
    }
  },
  "sessionAdvanced": {
    "title": "Parametry sesji",
    "subtitle": "Nadpisz domyślne ustawienia modelu dla tej rozmowy",
    "goBack": "Wróć",
    "support": "Wsparcie",
    "reset": "Resetuj",
    "save": "Zapisz",
    "noSessionWarning": "Otwórz sesję czatu, aby skonfigurować ustawienia per sesję.",
    "overrideDefaults": "Nadpisz domyślne",
    "overrideDefaultsDesc": "Dostosuj parametry tylko dla tej rozmowy",
    "loadingContextInfo": "Ładowanie informacji o kontekście...",
    "sampling": {
      "title": "Próbkowanie",
      "temperature": "Temperature",
      "temperatureDesc": "Kontroluje losowość. Niższe = bardziej deterministyczne, wyższe = bardziej kreatywne.",
      "temperaturePrecise": "Precyzyjne",
      "temperatureCreative": "Kreatywne",
      "topP": "Top P",
      "topPDesc": "Próbkowanie jądrowe. Ogranicza tokeny do skumulowanego prawdopodobieństwa.",
      "topPFocused": "Skupione",
      "topPDiverse": "Zróżnicowane",
      "topK": "Top K",
      "topKDesc": "Ogranicza próbkowanie do K najbardziej prawdopodobnych tokenów."
    },
    "outputPenalties": {
      "title": "Wyjście i kary",
      "maxOutputTokens": "Maksymalna liczba tokenów wyjściowych",
      "maxOutputTokensDesc": "Maksymalna długość odpowiedzi. Auto pozwala modelowi zdecydować.",
      "auto": "Auto",
      "custom": "Własne",
      "frequencyPenalty": "Kara częstotliwości",
      "frequencyPenaltyDesc": "Redukuje powtarzanie sekwencji tokenów.",
      "frequencyPenaltyRepeat": "Powtarzaj",
      "frequencyPenaltyVary": "Zmieniaj",
      "presencePenalty": "Kara obecności",
      "presencePenaltyDesc": "Zachęca do eksplorowania nowych tematów.",
      "presencePenaltyRepeat": "Powtarzaj",
      "presencePenaltyExplore": "Eksploruj"
    },
    "performance": {
      "title": "Wydajność",
      "gpuLayers": "Warstwy GPU",
      "gpuLayersDesc": "Warstwy przeniesione na GPU. 0 = tylko CPU.",
      "threads": "Wątki",
      "threadsDesc": "Wątki CPU do inferencji.",
      "batchThreads": "Wątki wsadowe",
      "batchThreadsDesc": "Wątki CPU do przetwarzania wsadowego.",
      "batchSize": "Rozmiar wsadu",
      "batchSizeDesc": "Rozmiar bloku przetwarzania promptu.",
      "contextLength": "Długość kontekstu",
      "contextLengthDesc": "Nadpisz rozmiar okna kontekstu.",
      "flashAttention": "Flash Attention",
      "flashAttentionDesc": "Optymalizacja pamięci GPU.",
      "enabled": "Włączone",
      "disabled": "Wyłączone"
    },
    "samplingMemory": {
      "title": "Próbkowanie i pamięć",
      "minP": "Min P",
      "minPDesc": "Minimalny próg prawdopodobieństwa.",
      "typicalP": "Typical P",
      "typicalPDesc": "Typowy próg próbkowania.",
      "seed": "Seed",
      "seedDesc": "Ziarno losowe. Zostaw puste dla losowego.",
      "ropeBase": "RoPE Base",
      "ropeBaseDesc": "Nadpisanie bazy częstotliwości.",
      "ropeScale": "RoPE Scale",
      "ropeScaleDesc": "Nadpisanie skali częstotliwości.",
      "kvCacheType": "KV Cache Type",
      "kvCacheTypeDesc": "Kwantyzuj KV cache, aby zaoszczędzić VRAM.",
      "offloadKqv": "Offload KQV",
      "offloadKqvDesc": "KV cache i operacje KQV na GPU.",
      "on": "Wł.",
      "off": "Wył.",
      "samplerProfile": "Profil samplera",
      "samplerProfileDesc": "Dostrojone domyślne dla stabilności lub rozumowania.",
      "balanced": "Zrównoważony",
      "creative": "Kreatywny",
      "stable": "Stabilny",
      "reasoning": "Reasoning"
    },
    "systemInfo": {
      "title": "Informacje systemowe",
      "maxContext": "Maks. kontekst",
      "recommended": "Zalecane",
      "availableRam": "Dostępny RAM",
      "availableVram": "Dostępny VRAM",
      "modelSize": "Rozmiar modelu"
    }
  },
  "exportMenu": {
    "title": "Format eksportu",
    "selectFormat": "Wybierz format",
    "uscTitle": "Unified System Card",
    "formats": {
      "uscPrompt": "Przenośny eksport USC dla szablonów promptów.",
      "uscLorebook": "Przenośny eksport USC dla lorebook'ów.",
      "uscModel": "Przenośny eksport USC dla profili modeli.",
      "uscChatTemplate": "Przenośny eksport USC dla szablonów czatu.",
      "legacyPromptJson": "Legacy Prompt JSON",
      "legacyPromptJsonDesc": "Aktualny format zewnętrznego pakietu promptów.",
      "lorebookJson": "Lorebook JSON",
      "lorebookJsonDesc": "Aktualny format eksportu lorebook'a.",
      "modelJson": "Model JSON",
      "modelJsonDesc": "Bezpieczny JSON profilu modelu bez poświadczeń.",
      "chatTemplateJson": "Chat Template JSON",
      "chatTemplateJsonDesc": "Natywny format eksportu szablonu czatu."
    },
    "extra": {
      "selectFormat": "Select a format",
      "exportFormatTitle": "Export Format",
      "uscDesc": "Portable USC export",
      "legacyJsonDesc": "Legacy JSON export format",
      "formatV3Desc": "Character Card V3 export",
      "formatV2Desc": "Character Card V2 export",
      "formatV1Desc": "Character Card V1 export",
      "uscLorebook": "Unified System Card (USC)",
      "portableLorebook": "Portable USC export for lorebooks",
      "lorebookJson": "Lorebook JSON",
      "currentLorebook": "Current lorebook export format",
      "uscModel": "Unified System Card (USC)",
      "portableModel": "Portable USC export for model profiles",
      "modelJson": "Model JSON",
      "safeModel": "Safe model profile JSON without credentials",
      "uscTemplate": "Unified System Card (USC)",
      "portableTemplate": "Portable USC export for chat templates",
      "templateJson": "Chat Template JSON",
      "nativeTemplate": "Native chat template export format"
    }
  },
  "designReference": {
    "title": "Referencje projektowe",
    "description": "Prześlij kilka wyraźnych obrazów referencyjnych plus jeden kanoniczny opis wizualny.",
    "descriptionPlaceholder": "Opisz stabilny wygląd: twarz, włosy, budowa ciała, prezentacja wieku, wskazówki dotyczące stroju, akcesoria oraz kierunek artystyczny/styl.",
    "addReferences": "Dodaj referencje",
    "visualDescription": "Opis wizualny",
    "draftWithAi": "Szkic z AI",
    "referenceImages": "Obrazy referencyjne",
    "imageAlt": "Referencja projektowa {{index}}",
    "loading": "Ładowanie...",
    "removeAria": "Usuń referencję projektową",
    "noImages": "Nie załączono jeszcze obrazów referencyjnych",
    "imageCount": "Załączono {{count}} obraz(ów) referencyjnych",
    "emptyReferences": "Dodaj kilka wyraźnych zdjęć referencyjnych, aby ustalić twarz, proporcje, strój i styl.",
    "noWriterModel": "Najpierw dodaj kompatybilny model pisarza scen w ustawieniach Generowania Obrazów.",
    "noImagesForGeneration": "Dodaj avatar lub przynajmniej jeden obraz referencyjny przed generowaniem.",
    "writerModelHelp": "Używa {{model}} do stworzenia szkicu na podstawie Twojego avatara i obrazów referencyjnych.",
    "noWriterModelHelp": "Dodaj kompatybilny model pisarza scen w ustawieniach Generowania Obrazów, aby automatycznie stworzyć ten szkic.",
    "draftMenuTitle": "Szkic projektowy AI",
    "draftMenuDesc": "Utworzony przez {{model}} na podstawie bieżącego avatara i obrazów referencyjnych.",
    "draftMenuNoWriter": "Dodaj kompatybilny model pisarza scen przed użyciem tego pomocnika.",
    "regenerate": "Regeneruj",
    "useThis": "Użyj tego"
  },
  "samplerOrder": {
    "title": "Kolejność samplera",
    "description": "Przeciągnij etapy, aby zmienić kolejność. Wykonywane od góry do dołu.",
    "reset": "Resetuj",
    "resetAria": "Resetuj kolejność samplera do domyślnej",
    "stages": {
      "penalties": {
        "label": "Kary",
        "desc": "Zastosuj kary częstotliwości i obecności przed filtrowaniem."
      },
      "grammar": {
        "label": "Gramatyka",
        "desc": "Ogranicz tokeny, gdy natywna gramatyka szablonu czatu jest aktywna."
      },
      "topK": {
        "label": "Top K",
        "desc": "Przytnij pulę kandydatów do najsilniejszych tokenów."
      },
      "topP": {
        "label": "Top P",
        "desc": "Zastosuj filtrowanie jądrowe do pozostałego rozkładu."
      },
      "minP": {
        "label": "Min P",
        "desc": "Odrzuć tokeny o niskim prawdopodobieństwie używając minimalnego progu."
      },
      "typical": {
        "label": "Typical P",
        "desc": "Preferuj statystycznie typowe tokeny w bieżącym rozkładzie."
      },
      "temp": {
        "label": "Temperature",
        "desc": "Spłaszcz lub wyostrz końcowy rozkład przed selekcją."
      }
    },
    "presets": {
      "default": {
        "label": "Domyślny",
        "hint": "Lettuce local"
      },
      "unsloth": {
        "label": "Unsloth",
        "hint": "Styl llama.cpp"
      },
      "focused": {
        "label": "Skupiony",
        "hint": "Ciasne przycinanie"
      },
      "creative": {
        "label": "Kreatywny",
        "hint": "Późny filtr"
      }
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
      "gpuFallbackReason": "Powód fallbacku GPU",
      "finalError": "Final error",
      "workingRecoveryConfig": "Working recovery config",
      "context": "Context",
      "batch": "Batch",
      "na": "n/a",
      "applyWorkingConfig": "Apply working config",
      "badges": {
        "succeeded": "Run succeeded",
        "cpuFallbackSucceeded": "Fallback CPU odzyskany",
        "cpuFallbackFailed": "Fallback CPU nie powiódł się",
        "failed": "Run failed"
      },
      "headline": {
        "succeeded": "The last local run completed successfully.",
        "cpuFallbackSucceeded": "Ładowanie GPU nie powiodło się, a potem model odzyskał działanie na CPU.",
        "cpuFallbackFailed": "Ładowanie GPU nie powiodło się, a fallback CPU nadal się nie zakończył.",
        "failed": "The last local run failed before llama.cpp could complete."
      },
      "detail": {
        "succeeded": "This report also seeds the smart offload cache so future runs can reuse the last stable GPU setup.",
        "cpuFallbackSucceeded": "Zapisaliśmy kontekst i batch bezpieczne dla CPU, które faktycznie zadziałały, abyś mógł ich użyć ponownie.",
        "cpuFallbackFailed": "Model został ponownie uruchomiony na CPU, ale odzyskana konfiguracja nadal zawiodła.",
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
      "smartOffloadFallback": "Inteligentny fallback offloadu",
      "active": "Active",
      "notNeeded": "Not needed",
      "kqvFallback": "Fallback KQV",
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
  "lorebookGen": {
    "flow": {
      "pickerCharactersTitle": "Characters",
      "pickerSessionsTitle": "Sessions",
      "noCharacters": "No characters",
      "noSessions": "No sessions",
      "clearSelection": "Clear selection",
      "directionTitle": "Optional generation direction",
      "directionLabel": "Direction",
      "toggleForceMode": "Toggle force mode",
      "entryTitlePlaceholder": "Entry title",
      "entryContentPlaceholder": "Lorebook entry content",
      "editDirectionBeforeRegenerate": "Edit direction before regenerating",
      "generatorReturnedNoDraft": "Generator returned no draft",
      "pageTitle": "Generate Lorebook Entry",
      "missingContext": "Missing lorebook context for the generator page.",
      "characterLocked": "Character is locked to this lorebook's owner",
      "chooseSession": "Choose session",
      "pickCharacter": "Pick character",
      "searchMemories": "Search memories",
      "searchMessages": "Search messages",
      "selectLastN": "Select last {{n}} messages",
      "selectAll": "Select all",
      "loadSessionPrompt": "Select a session to load",
      "messagesText": "messages",
      "memoriesText": "memories",
      "messagesAndMemories": "messages and memories",
      "titleAndContentRequired": "Title and content are required",
      "keywordsOrAlwaysActive": "Add at least one keyword or enable Always active",
      "lorybookEntrySaved": "Lorebook entry saved",
      "saveFailed": "Save failed",
      "generationFailed": "Generation failed",
      "failedToLoadContext": "Failed to load lorebook generator",
      "failedToLoadSessions": "Failed to load sessions",
      "failedToLoadMessages": "Failed to load messages",
      "userRole": "USER",
      "aiRole": "AI"
    },
    "triggerPreview": {
      "resizeInspector": "Resize inspector"
    }
  },
  "companion": {
    "download": {
      "emotionClassifier": "Emotion classifier",
      "emotionClassifierDesc": "Reads turns and updates the companion's felt, expressed, and blocked emotion vectors.",
      "emotionSize": "~120 MB",
      "entityExtractor": "Entity extractor (NER)",
      "entityExtractorDesc": "Identifies people, places, and objects so memories can be canonicalized and linked.",
      "entitySize": "~140 MB",
      "memoryRouter": "Memory router",
      "memoryRouterDesc": "Decides whether new turns should be stored as relationship, milestone, episodic, or other memory categories.",
      "routerSize": "~70 MB",
      "unknownModel": "Unknown companion model. Provide ?kind=emotion|ner|router."
    }
  },
  "voices": {
    "extra": {
      "customVoices": "Moje głosy",
      "providerVoices": "Głosy dostawcy",
      "myVoices": "Moje głosy",
      "page": {
        "noAudioProvidersHint": "Dodaj jednego w Dostawcy > Audio, aby rozpocząć",
        "noVoicesTitle": "Nie utworzono jeszcze żadnych głosów",
        "noVoicesDescription": "Twórz głosy z niestandardowymi promptami dla swoich postaci",
        "addProviderFirst": "Najpierw dodaj dostawcę audio",
        "noPrompt": "Brak promptu",
        "noProviderVoices": "Nie znaleziono głosów. Kliknij Odśwież, aby pobrać głosy.",
        "showLess": "Pokaż mniej",
        "showAllVoices": "Pokaż wszystkie {{count}} głosów",
        "voiceFallbackTitle": "Głos"
      },
      "cache": {
        "section": "Pamięć podręczna audio",
        "title": "Pamięć podręczna TTS",
        "description": "Wygenerowane audio głosu jest przechowywane w pamięci podręcznej, aby ograniczyć ponowne generowanie",
        "clearing": "Czyszczenie...",
        "clear": "Wyczyść pamięć podręczną"
      },
      "menu": {
        "editDescription": "Zmodyfikuj ten głos",
        "deleteDescription": "Usuń ten głos",
        "provider": "Dostawca",
        "category": "Kategoria",
        "createVoiceConfig": "Utwórz konfigurację głosu",
        "createVoiceConfigDescription": "Użyj tego głosu z niestandardowymi ustawieniami"
      },
      "editor": {
        "editTitle": "Edytuj głos",
        "createTitle": "Utwórz głos",
        "voiceName": "Nazwa głosu",
        "voiceNamePlaceholder": "Głos mojej postaci",
        "provider": "Dostawca",
        "model": "Model",
        "modelIdPlaceholder": "gpt-4o-mini-tts",
        "modelIdHint": "Wprowadź dokładne ID modelu obsługiwane przez twój kompatybilny endpoint",
        "elevenlabsVoice": "Głos ElevenLabs",
        "noVoicesAvailable": "Brak dostępnych głosów",
        "selectVoice": "Wybierz głos...",
        "elevenlabsVoiceHint": "Wybierz spośród głosów ElevenLabs",
        "geminiVoice": "Głos Gemini",
        "geminiVoiceHint": "Wybierz głos Gemini TTS",
        "voiceId": "ID głosu",
        "voiceIdPlaceholder": "alloy",
        "voiceIdHint": "Wprowadź ID głosu obsługiwane przez twój kompatybilny endpoint",
        "voicePrompt": "Prompt głosu",
        "voicePromptPlaceholder": "Ciepły, przyjazny głos z radosnym tonem...",
        "voicePromptHint": "Opisz, jak głos powinien brzmieć",
        "exampleText": "Przykładowy tekst",
        "exampleTextPlaceholder": "Cześć! Tak brzmię, gdy mówię...",
        "exampleTextHint": "Przykładowy tekst do testowania głosu",
        "voiceDesignChars": "{{current}}/{{minimum}} znaków wymaganych do podglądu projektu głosu",
        "defaultSample": "Cześć! Tak brzmię, gdy mówię. Potrafię czytać dłuższe fragmenty z ciepłem, wyrazistością i emocjami, abyś mógł ocenić mój ton i tempo.",
        "playing": "Odtwarzanie...",
        "previewVoice": "Podgląd głosu"
      },
      "kokoro": {
        "title": "Kokoro Studio",
        "newBlend": "Nowa mieszanka",
        "editBlend": "Edytuj mieszankę",
        "tryText": "Cześć! To krótki test tego, jak brzmię.",
        "experimentDefaultText": "Cześć! Tak brzmię, gdy mówię. Potrafię czytać dłuższe fragmenty z ciepłem, wyrazistością i emocjami.",
        "livePreview": "Podgląd na żywo",
        "savedBlend": "Zapisana mieszanka",
        "defaultPreviewText": "Cześć! To krótki podgląd tego, jak brzmi ten głos.",
        "experiment": "Eksperyment",
        "providerNotFound": "Nie znaleziono dostawcy Kokoro",
        "backToProviders": "Wróć do dostawców",
        "variantUnset": "Wariant nieskonfigurowany",
        "ready": "Gotowy",
        "modelNotInstalled": "Model nie jest zainstalowany",
        "voiceCount": "{{count}} głos",
        "engineActions": "Akcje silnika",
        "engineNotInstalled": "Silnik nie jest zainstalowany",
        "installAtLeastOneVoice": "Zainstaluj co najmniej jeden głos",
        "continueSetup": "Kontynuuj konfigurację, aby zainstalować model Kokoro.",
        "pickVoiceOrStarter": "Wybierz głos lub pobierz pakiet startowy, aby zacząć.",
        "downloadsFailed": "{{count}} pobieranie nie powiodło się",
        "retryOrDismissAll": "Ponów pojedynczo lub odrzuć wszystkie.",
        "dismissAll": "Odrzuć wszystkie",
        "model": "Model",
        "voice": "Głos",
        "downloads": "Pobierania",
        "cancelAll": "Anuluj wszystkie",
        "experimentPlaceholder": "Wpisz frazę, aby ją usłyszeć...",
        "speed": "Prędkość",
        "speak": "Mów",
        "yourBlends": "Twoje mieszanki",
        "noSavedBlends": "Brak zapisanych mieszanek.",
        "installModelAndVoiceFirst": "Najpierw zainstaluj model i głos.",
        "featured": "Wyróżnione",
        "stop": "Zatrzymaj",
        "sample": "Próbka",
        "voiceLibrary": "Biblioteka głosów",
        "starterPack": "Pakiet startowy",
        "select": "Wybierz",
        "all": "Wszystkie",
        "installed": "Zainstalowane",
        "installModelToBrowse": "Zainstaluj model, aby przeglądać głosy.",
        "noVoicesInCatalog": "Brak głosów w katalogu. Naciśnij Odśwież.",
        "noVoicesMatch": "Żaden głos nie pasuje do twoich filtrów.",
        "collapseAll": "Zwiń wszystkie",
        "expandAll": "Rozwiń wszystkie",
        "selectedCount": "{{count}} zaznaczonych",
        "engineTitle": "Silnik Kokoro",
        "variant": "Wariant",
        "status": "Status",
        "notInstalled": "Nie zainstalowany",
        "file": "Plik",
        "modelSize": "Rozmiar modelu",
        "voicesSize": "Rozmiar głosów",
        "total": "Łącznie",
        "assetRoot": "Katalog zasobów",
        "reinstallModel": "Zainstaluj ponownie model",
        "installModel": "Zainstaluj model",
        "deleteModel": "Usuń model",
        "deleteModelDescription": "Zwalnia miejsce na dysku; głosy zostają zachowane.",
        "blend": "Mieszanka",
        "previewDescription": "Szybkie odsłuchanie z domyślnym tekstem",
        "editBlendDescription": "Dostosuj głosy, wagi i prędkość",
        "deleteBlendDescription": "Usuń ten zapisany głos",
        "setupTitle": "Skonfiguruj Kokoro",
        "allSet": "Gotowe",
        "allSetDescription": "Przeglądaj głosy, projektuj mieszanki lub testuj w obszarze eksperymentów."
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
      "conditionalsSection": "Conditionals",
      "injectionPoints": "Injection Points"
    }
  },
  "embeddings": {
    "download": {
      "bestForQuick": "Best for quick responses",
      "balancedPerf": "Balanced performance",
      "maxContext": "Maximum context",
      "capacity1k": "1K tokens",
      "capacity2k": "2K tokens",
      "capacity4k": "4K tokens",
      "approxSize": "~138 MB",
      "downloadVersion": "V4"
    },
    "test": {
      "health": "Health Check",
      "retrieval": "Retrieval",
      "separation": "Separation",
      "passed": "Passed",
      "failed": "Failed",
      "testing": "Testing..."
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
      "jsonDesc": "Compact structured output when tool calling is unavailable.",
      "xml": "XML",
      "xmlDesc": "Use when the model formats XML more reliably than JSON.",
      "fallbackFormat": "Format fallbacku"
    }
  },
  "usageAnalytics": {
    "page": {
      "filters": "Filtry",
      "model": "Model",
      "character": "Postać",
      "clearAll": "Wyczyść wszystko",
      "applyFilters": "Zastosuj filtry",
      "recentActivity": "Ostatnia aktywność",
      "customRange": "Niestandardowy zakres",
      "startDate": "Data początkowa",
      "endDate": "Data końcowa",
      "applyRange": "Zastosuj zakres",
      "dashboard": "PANEL",
      "appTime": "CZAS W APLIKACJI",
      "today": "Dzisiaj",
      "last7Days": "7 dni",
      "last30Days": "30 dni",
      "all": "Wszystkie",
      "custom": "Niestandardowy",
      "filtersCount": "{{count}} filtry",
      "totalCost": "Łączny koszt",
      "tokens": "Tokeny",
      "avgShort": "śr. {{value}}",
      "requests": "Zapytania",
      "period": "Okres",
      "last7d": "Ostatnie 7 dni",
      "last30d": "Ostatnie 30 dni",
      "allTime": "Cały czas",
      "recordsCount": "{{count}} rekordów",
      "usageTrend": "Trend użycia",
      "tokenConsumptionOverTime": "Zużycie tokenów w czasie",
      "input": "Wejście",
      "output": "Wyjście",
      "byModel": "Według modelu",
      "byCharacter": "Według postaci",
      "top": "Top",
      "active": "Aktywne",
      "quickView": "Szybki podgląd",
      "viewAll": "Pokaż wszystko",
      "startChatting": "Zacznij rozmawiać, aby zobaczyć dane użycia",
      "exportedTo": "Wyeksportowano do: {{path}}",
      "periodTotal": "Łącznie w okresie",
      "daysActive": "{{count}} aktywnych dni",
      "dailyAvg": "Średnia dzienna",
      "selectedPeriod": "wybrany okres",
      "yesterdayValue": "Wczoraj {{value}}",
      "thirtyDayAvg": "Średnia z 30 dni",
      "appTimeTrend": "Trend czasu w aplikacji",
      "usageDurationPerDay": "Czas użycia na dzień",
      "totalValue": "Łącznie {{value}}",
      "activeTime": "Czas aktywności"
    },
    "activity": {
      "loading": "Ładowanie aktywności...",
      "title": "Ostatnia aktywność",
      "recordsCount": "{{count}} rekordów użycia",
      "rangeOfTotal": "{{start}}-{{end}} z {{total}}",
      "previous": "Poprzednia",
      "next": "Następna",
      "pageOf": "Strona {{page}} z {{total}}"
    },
    "shared": {
      "relativeTime": {
        "justNow": "właśnie teraz",
        "minutesAgo": "{{count}} min temu",
        "hoursAgo": "{{count}} godz. temu",
        "daysAgo": "{{count}} dni temu"
      },
      "operations": {
        "chat": "Czat",
        "regenerate": "Regeneruj",
        "continue": "Kontynuuj",
        "summary": "Podsumowanie",
        "memoryManager": "Pamięć",
        "imageGeneration": "Generowanie obrazu",
        "aiCreator": "Kreator AI",
        "replyHelper": "Asystent odpowiedzi",
        "groupChatMessage": "Czat grupowy",
        "groupChatRegenerate": "Regeneruj grupowy",
        "groupChatContinue": "Kontynuuj grupowy",
        "groupChatDecisionMaker": "Decydent"
      },
      "outputImages": "{{count}} obrazy",
      "tokens": "{{count}} tokenów",
      "unknown": "Nieznane",
      "requestDetails": "Szczegóły zapytania",
      "noStopReason": "Brak przyczyny zatrzymania",
      "tokenUsage": "Użycie tokenów",
      "estimatedCost": "Szacowany koszt",
      "stats": {
        "prompt": "Prompt",
        "completion": "Uzupełnienie",
        "total": "Łącznie",
        "reasoning": "Rozumowanie",
        "image": "Obraz",
        "memory": "Pamięć",
        "summary": "Podsumowanie",
        "inputImages": "Obrazy wejściowe",
        "outputImages": "Obrazy wyjściowe",
        "cachedPrompt": "Prompt z pamięci podręcznej",
        "cacheWrite": "Zapis do pamięci podręcznej",
        "webSearches": "Wyszukiwania w sieci",
        "cacheRead": "Odczyt z pamięci podręcznej",
        "requestFee": "Opłata za zapytanie",
        "webSearch": "Wyszukiwanie w sieci",
        "providerTotal": "Łącznie u dostawcy"
      }
    }
  }
};
