import type { DeepPartialMessageTree } from "../types";
import { type LocaleMessages } from "./en";
import type { LocaleMetadata } from ".";

export const ruMetadata: LocaleMetadata = {
  name: "Russian",
  label: "Русский",
};

export const ruMessages: DeepPartialMessageTree<LocaleMessages> = {
  "common": {
    "nav": {
      "chats": "Чаты",
      "settings": "Настройки",
      "providers": "Провайдеры",
      "responseStyle": "Стиль ответа",
      "models": "Модели",
      "security": "Безопасность",
      "accessibility": "Доступность",
      "speechRecognition": "Распознавание речи",
      "reset": "Сброс",
      "backupRestore": "Резервное копирование",
      "convertFiles": "Конвертация файлов",
      "usageAnalytics": "Аналитика использования",
      "changelog": "Журнал изменений",
      "about": "О приложении",
      "createSystemPrompt": "Создать системный промпт",
      "editSystemPrompt": "Редактировать системный промпт",
      "systemPrompts": "Системные промпты",
      "developer": "Разработчик",
      "advanced": "Расширенные",
      "hostApi": "API-сервер",
      "characters": "Персонажи",
      "lorebooks": "Лорбуки",
      "personas": "Персоны",
      "dynamicMemory": "Динамическая память",
      "creationHelper": "Помощник создания",
      "helpMeReply": "Помоги ответить",
      "lorebookEntryGenerator": "Генератор записей лорбука",
      "companionSoulWriter": "Создатель души компаньона",
      "editPersona": "Редактировать персону",
      "newTemplate": "Новый шаблон",
      "editTemplate": "Редактировать шаблон",
      "chatTemplates": "Шаблоны чата",
      "editCharacter": "Редактировать персонажа",
      "sync": "Синхронизация",
      "newCharacter": "Новый персонаж",
      "engineSetup": "Настройка движка",
      "llmProviders": "Провайдеры LLM",
      "engineSettings": "Настройки движка",
      "lettuceEngine": "Движок Lettuce",
      "create": "Создать",
      "setup": "Настройка",
      "welcome": "Добро пожаловать",
      "conversation": "Разговор",
      "library": "Библиотека",
      "groupChats": "Групповые чаты",
      "groupChat": "Групповой чат",
      "imageGeneration": "Генерация изображений",
      "voices": "Голоса",
      "chatAppearance": "Оформление чата",
      "colorCustomization": "Настройка цветов",
      "embeddingDownload": "Загрузка Embedding",
      "embeddingTest": "Тест Embedding",
      "editModel": "Редактировать модель",
      "messageDebug": "Отладка сообщений"
    },
    "bottomNav": {
      "chats": "Чаты",
      "groups": "Группы",
      "create": "Создать",
      "discover": "Обзор",
      "library": "Библиотека"
    },
    "buttons": {
      "cancel": "Отмена",
      "confirm": "Подтвердить",
      "save": "Сохранить",
      "saving": "Сохранение...",
      "delete": "Удалить",
      "deleting": "Удаление...",
      "create": "Создать",
      "creating": "Создание...",
      "edit": "Редактировать",
      "back": "Назад",
      "done": "Готово",
      "download": "Скачать",
      "later": "Позже",
      "proceed": "Продолжить",
      "retry": "Повторить",
      "discard": "Отменить",
      "import": "Импорт",
      "importing": "Импорт...",
      "export": "Экспорт",
      "exporting": "Экспорт...",
      "update": "Обновить",
      "generate": "Сгенерировать",
      "refresh": "Обновить",
      "continue": "Продолжить",
      "goBack": "Вернуться",
      "search": "Поиск",
      "clearSearch": "Очистить поиск",
      "add": "Добавить",
      "install": "Установить",
      "remove": "Удалить",
      "rename": "Переименовать",
      "copy": "Копировать",
      "copied": "Скопировано!",
      "browseFiles": "Обзор файлов"
    },
    "labels": {
      "processing": "Обработка...",
      "loading": "Загрузка...",
      "noDescriptionYet": "Описание пока отсутствует",
      "untitled": "Без названия",
      "default": "По умолчанию",
      "enabled": "Включено",
      "disabled": "Выключено",
      "on": "Вкл",
      "off": "Выкл",
      "none": "Нет",
      "auto": "Авто",
      "custom": "Пользовательское",
      "name": "Имя",
      "description": "Описание",
      "content": "Содержание",
      "preview": "Предпросмотр",
      "options": "Параметры"
    },
    "time": {
      "justNow": "Только что",
      "minutesAgo": "{{minutes}}м назад",
      "hoursAgo": "{{hours}}ч назад",
      "daysAgo": "{{days}}д назад",
      "today": "Сегодня",
      "yesterday": "Вчера",
      "last7Days": "Последние 7 дней",
      "older": "Ранее"
    }
  },
  "settings": {
    "sections": {
      "intelligence": "Интеллект",
      "experience": "Опыт",
      "connectivity": "Связь",
      "securityPrivacy": "Безопасность и конфиденциальность",
      "supportInfo": "Поддержка и информация",
      "dangerZone": "Опасная зона",
      "developer": "Разработчик"
    },
    "items": {
      "providers": {
        "title": "Провайдеры",
        "subtitle": "Подключение к AI-сервисам"
      },
      "models": {
        "title": "Модели",
        "subtitle": "Настройка AI-моделей"
      },
      "imageGeneration": {
        "title": "Генерация изображений",
        "subtitle": "Генерируйте и тестируйте изображения"
      },
      "voices": {
        "title": "Голоса",
        "subtitle": "Голоса для озвучивания"
      },
      "accessibility": {
        "title": "Доступность",
        "subtitle": "Звуки и вибрация"
      },
      "prompts": {
        "title": "Системные промпты",
        "subtitle": "Настройка личности AI"
      },
      "security": {
        "title": "Безопасность",
        "subtitle": "Шифрование и конфиденциальность"
      },
      "backup": {
        "title": "Резервное копирование",
        "subtitle": "Экспорт и импорт данных"
      },
      "convert": {
        "title": "Конвертация файлов",
        "subtitle": "Миграция .json в .uec"
      },
      "sync": {
        "title": "Локальная синхронизация",
        "subtitle": "Синхронизация между устройствами"
      },
      "usage": {
        "title": "Аналитика использования",
        "subtitle": "Расходы и статистика токенов"
      },
      "advanced": {
        "title": "Расширенные",
        "subtitle": "Память и функции"
      },
      "logs": {
        "title": "Логи",
        "subtitle": "Отладка и диагностика"
      },
      "guide": {
        "title": "Руководство",
        "subtitle": "Повторный онбординг"
      },
      "docs": {
        "title": "Документация",
        "subtitle": "Справочники"
      },
      "github": {
        "title": "Сообщить о проблеме",
        "subtitle": "Баги и отзывы • v{{version}}"
      },
      "discord": {
        "title": "Discord",
        "subtitle": "Сообщество и помощь"
      },
      "about": {
        "title": "О приложении",
        "subtitle": "Версия, обновления и ссылки"
      },
      "changelog": {
        "title": "Журнал изменений",
        "subtitle": "Что нового"
      },
      "reset": {
        "title": "Сброс",
        "subtitle": "Удалить всё"
      },
      "developer": {
        "title": "Инструменты разработчика",
        "subtitle": "Отладка и тестирование"
      }
    }
  },
  "accessibility": {
    "sectionTitles": {
      "language": "Язык",
      "sounds": "Звуковые уведомления",
      "haptics": "Тактильная обратная связь",
      "appearance": "Внешний вид"
    },
    "language": {
      "appLanguage": "Язык приложения",
      "description": "Выберите язык для навигации и настроек."
    },
    "appearance": {
      "customColors": "Настройка цветов",
      "customColorsDesc": "Настройте цветовую схему приложения",
      "chatAppearance": "Оформление чата",
      "chatAppearanceDesc": "Настройте пузыри сообщений, шрифты и макет"
    },
    "haptics": {
      "vibrateOnChat": "Вибрация в чате",
      "vibrateDesc": "Короткие вибрации при наборе текста ассистентом",
      "intensity": "Интенсивность",
      "light": "Лёгкая",
      "medium": "Средняя",
      "heavy": "Сильная",
      "soft": "Мягкая",
      "rigid": "Жёсткая"
    },
    "sounds": {
      "send": "Отправка",
      "sendDescription": "Воспроизводится при отправке сообщения",
      "success": "Успех",
      "successDescription": "Воспроизводится при успешном ответе ассистента",
      "failure": "Ошибка",
      "failureDescription": "Воспроизводится при ошибке или отмене",
      "testButton": "Тест"
    },
    "feedbackInfo": "Обратная связь помогает отслеживать отправку и получение сообщений.",
    "hapticsInfo": "Тактильная связь доступна на мобильных устройствах.",
    "extra": {
      "easterEggs": "Пасхалки",
      "beetrootRain": "Свекольный дождь",
      "beetrootDesc": "Когда в чате упоминается свёкла, сверху падают свёклы"
    }
  },
  "topNav": {
    "unsavedChangesTitle": "Несохранённые изменения",
    "unsavedChangesMessage": "Сохраните или отмените изменения перед выходом.",
    "goBack": "Вернуться",
    "changeLayout": "Изменить макет",
    "search": "Поиск",
    "settings": "Настройки",
    "help": "Помощь",
    "add": "Добавить",
    "openFilters": "Открыть фильтры",
    "save": "Сохранить",
    "extra": {
      "installedModels": "Установленные модели",
      "refresh": "Обновить",
      "minimize": "Свернуть",
      "maximize": "Развернуть",
      "close": "Закрыть"
    }
  },
  "updates": {
    "available": {
      "title": "Доступна новая версия",
      "description": "Доступна v{{latestVersion}}. У вас установлена v{{currentVersion}}.",
      "actions": {
        "view": "Открыть релиз"
      }
    }
  },
  "about": {
    "kicker": "Информация о приложении",
    "appName": "LettuceAI",
    "description": "Сведения о версии, проверка обновлений и полезные ссылки.",
    "info": {
      "version": "Версия",
      "channel": "Канал",
      "platform": "Платформа"
    },
    "buildChannel": {
      "dev": "Сборка для разработки",
      "release": "Стабильный релиз"
    },
    "update": {
      "sectionTitle": "Обновления",
      "title": "Обновления приложения",
      "description": "Проверьте новые релизы вручную или отключите автоматическую проверку при запуске.",
      "autoChecks": "Автоматическая проверка обновлений",
      "autoChecksDescription": "Если включено, LettuceAI проверяет новые версии при открытии приложения.",
      "checkNow": "Проверить обновления",
      "checking": "Проверка обновлений...",
      "upToDateTitle": "У вас актуальная версия",
      "upToDateDescription": "Сейчас для этого устройства нет более новой версии.",
      "failedTitle": "Не удалось проверить обновления",
      "failedDescription": "Сейчас не удалось проверить обновления. Попробуйте ещё раз чуть позже."
    },
    "links": {
      "sectionTitle": "Ссылки",
      "website": "Сайт",
      "websiteDescription": "Страница загрузки и информация о релизах",
      "github": "GitHub",
      "githubDescription": "Исходный код, issues и разработка",
      "discord": "Discord",
      "discordDescription": "Сервер сообщества и поддержка",
      "reddit": "Reddit",
      "redditDescription": "Обсуждения сообщества, отзывы и обновления"
    },
    "developerMode": {
      "enable": "Включить режим разработчика",
      "enabled": "Режим разработчика включён"
    },
    "errors": {
      "saveTitle": "Не удалось сохранить настройку",
      "saveDescription": "Настройка проверки обновлений не была изменена."
    }
  },
  "components": {
    "tooltip": {
      "dismissHint": "Нажмите где-угодно, чтобы закрыть"
    },
    "backgroundPosition": {
      "title": "Позиция фона",
      "instructions": "Перетащите для позиционирования • Масштабируйте щипком или прокруткой"
    },
    "avatarSource": {
      "generateImage": "Сгенерировать изображение",
      "generateImageDesc": "Создание аватара с помощью AI",
      "noImageModels": "Нет доступных моделей изображений",
      "editCurrent": "Редактировать текущий",
      "editCurrentDesc": "Переместить или обрезать",
      "chooseImage": "Выбрать изображение",
      "chooseImageDesc": "Выбрать с устройства"
    },
    "avatarCurrentEdit": {
      "title": "Редактировать текущий",
      "reposition": "Переместить",
      "repositionDesc": "Переместить или обрезать текущий аватар",
      "editWithAI": "Редактировать с помощью ИИ",
      "editWithAIDesc": "Открыть редактирование AI для текущего аватара",
      "noImageModels": "Нет доступных моделей изображений"
    },
    "avatarGeneration": {
      "modelsLoadError": "Не удалось загрузить модели генерации изображений",
      "title": "Генерация аватара",
      "help": "Помощь по генерации аватара",
      "model": "Модель",
      "selectModel": "Выберите модель",
      "describe": "Опишите ваш аватар",
      "describePlaceholder": "Дружелюбная аниме-девушка с яркими волосами, тепло улыбающаяся...",
      "inProgress": "Генерация аватара...",
      "editingInProgress": "Применение редактирования аватара...",
      "alt": "Сгенерированный аватар",
      "regenerate": "Перегенерировать",
      "useThis": "Использовать это",
      "previousVariant": "Предыдущий вариант",
      "nextVariant": "Следующий вариант",
      "variantCounter": "{{current}} / {{total}}",
      "editRequest": "Редактировать запрос",
      "editRequestPlaceholder": "Сделайте волосы темнее, добавьте очки, оставьте лицо таким же...",
      "applyEdit": "Применить Редактировать",
      "editImageLoadError": "Не удалось подготовить сгенерированный аватар для редактирования.",
      "aiAssistant": "ИИ-помощник",
      "backToResults": "Вернуться к подсказке",
      "magicInTheWorks": "Магия в действии...",
      "refine": "Уточнить",
      "apply": "Применять"
    },
    "avatarPosition": {
      "title": "Позиция аватара",
      "instructions": "Перетащите для позиционирования • Масштабируйте щипком или прокруткой",
      "alt": "Аватар для позиционирования"
    },
    "confirmDialog": {
      "defaultTitle": "Подтвердить",
      "defaultLabel": "Подтвердить"
    },
    "bottomMenu": {
      "defaultTitle": "Меню",
      "dragTip": "Потяните, чтобы закрыть меню",
      "closeLabel": "Закрыть меню",
      "buttonProcessing": "Обработка..."
    },
    "modelSelector": {
      "placeholder": "Выберите модель",
      "clearLabel": "Использовать глобальную по умолчанию",
      "loading": "Загрузка моделей...",
      "noModels": "Нет доступных моделей",
      "addProviderFirst": "Сначала добавьте провайдера в настройках",
      "title": "Выбор модели",
      "searchPlaceholder": "Поиск моделей...",
      "noResults": "Модели не найдены",
      "noResultsHint": "Попробуйте другой поисковый запрос"
    },
    "localeSelector": {
      "title": "Выберите язык"
    },
    "promptTemplate": {
      "nameContentRequired": "Необходимо указать имя и содержание",
      "saveError": "Не удалось сохранить шаблон",
      "editTitle": "Редактировать промпт",
      "createTitle": "Создать промпт",
      "name": "Имя",
      "namePlaceholder": "напр., Мастер Ролевой Игры",
      "content": "Содержание",
      "variablesButton": "Переменные",
      "contentPlaceholder": "Вы — полезный AI-ассистент...\n\nИспользуйте {{char.name}} и {{scene}} в промпте.",
      "characterCount": "{{count}} символов",
      "preview": "Предпросмотр",
      "characterPlaceholder": "Персонаж…",
      "personaPlaceholder": "Персона…",
      "rendering": "Рендеринг…",
      "noPreview": "Предпросмотр пока недоступен",
      "saving": "Сохранение...",
      "update": "Обновить",
      "create": "Создать",
      "variablesTitle": "Переменные шаблона",
      "variablesCopyHint": "Нажмите, чтобы скопировать в буфер",
      "variablesCopied": "Скопировано",
      "variables": {
        "charName": "Имя персонажа",
        "charNameDesc": "Имя персонажа",
        "charDesc": "Описание персонажа",
        "charDescDesc": "Описание персонажа",
        "scene": "Сцена",
        "sceneDesc": "Начальная сцена/сценарий",
        "userName": "Имя пользователя",
        "userNameDesc": "Имя персоны пользователя",
        "userDesc": "Описание пользователя",
        "userDescDesc": "Описание персоны пользователя",
        "rules": "Правила",
        "rulesDesc": "Правила поведения персонажа",
        "contextSummary": "Краткое содержание",
        "contextSummaryDesc": "Динамическое резюме разговора",
        "keyMemories": "Ключевые воспоминания",
        "keyMemoriesDesc": "Список релевантных воспоминаний"
      }
    },
    "characterExport": {
      "title": "Формат экспорта",
      "selectFormat": "Выберите формат",
      "loading": "Загрузка форматов...",
      "formatUecDesc": "Формат Unified Entity Card (.uec) (рекомендуется).",
      "formatLegacyJsonDesc": "Устаревший JSON (только импорт).",
      "formatV3Desc": "Character Card V3 JSON (последняя спецификация).",
      "formatV2Desc": "Character Card V2 JSON (спецификация Tavern).",
      "formatV1Desc": "Character Card V1 (только импорт)."
    },
    "embeddingDownload": {
      "downloadRequired": "Требуется загрузка",
      "modelRequired": "Требуется модель Embedding",
      "description": "Динамическая память требует загрузки локальной модели embedding (~260 МБ) для работы.",
      "localStorage": "• Модель будет храниться локально на вашем устройстве",
      "downloadSize": "• Размер загрузки: приблизительно 260 МБ",
      "summarization": "• Необходима для обобщения разговоров"
    },
    "embeddingUpgrade": {
      "title": "Доступна модель Embedding v3",
      "v1Message": "Вы используете v1 с 512 токенами. Обновитесь до v3 для лучшего качества памяти и поддержки длинного контекста.",
      "v2Message": "Вы используете устаревшую v2. Обновитесь до v3 для лучшего качества памяти с новейшей моделью embedding.",
      "v3Message": "v4 вышел и значительно улучшает воспроизведение памяти в ролевой игре по сравнению с версией 3 (recall@1 0,02 -> 0,92). ",
      "button": "Обновить до v3"
    },
    "v2UpgradeToast": {
      "title": "Модель памяти v3",
      "badge": "Доступно",
      "message": "Улучшенное качество embedding по сравнению с v2",
      "dismiss": "Закрыть",
      "upgrade": "Обновить"
    },
    "v1UpgradeToast": {
      "title": "Доступна модель памяти v3",
      "message": "Обновите для лучшего качества памяти и поддержки длинного контекста.",
      "dismiss": "Закрыть",
      "upgrade": "Обновить"
    },
    "v3UpgradeToast": {
      "title": "Модель памяти v4",
      "badge": "Available",
      "message": "v4 значительно улучшает воспроизведение памяти в ролевой игре по сравнению с версией 3 (recall@1 0,02 → 0,92). ",
      "dismiss": "Позже",
      "upgrade": "Обновление"
    },
    "createMenu": {
      "title": "Создать новое",
      "smartCreator": "Умный создатель",
      "smartCreatorDesc": "Пусть ассистент поможет в создании",
      "divider": "Или создайте вручную",
      "character": "Персонаж",
      "characterDesc": "Создать персонажа",
      "persona": "Персона",
      "personaDesc": "Создать образ для себя",
      "groupChat": "Групповой чат",
      "groupChatDesc": "Чат с несколькими персонажами",
      "lorebook": "Лорбук",
      "lorebookDesc": "Создать справочник мира",
      "characterSmartDesc": "Создать персонажа с помощью мастера",
      "personaSmartDesc": "Создать образ или личность",
      "lorebookSmartDesc": "Создать структурированный справочник мира",
      "loadingConversations": "Загрузка разговоров...",
      "createNew": "Создать новый",
      "createNewDesc": "Начать новый сеанс создания",
      "editExisting": "Редактировать существующий",
      "continueLast": "Продолжить последний разговор",
      "seeOlder": "Посмотреть старые разговоры",
      "seeOlderDesc": "Открыть предыдущие сеансы Умного Создателя",
      "noConversations": "Пока нет разговоров для этого создателя.",
      "sessionCompleted": "Завершён",
      "sessionCancelled": "Отменён",
      "sessionDraft": "Черновик",
      "sessionMessages": "{{count}} сообщений",
      "untitledConversation": "Разговор без названия",
      "nameLorebookTitle": "Назвать лорбук",
      "lorebookNamePlaceholder": "Введите название лорбука...",
      "lorebookImporting": "Импорт...",
      "lorebookImport": "Импорт",
      "lorebookCreating": "Создание...",
      "lorebookCreate": "Создать",
      "editExistingDesc": "Выберите {{goal}} и отредактируйте с помощью Умного Создателя",
      "creatorTitle": "Создатель {{goal}}",
      "editTitle": "Редактирование {{goal}}",
      "conversationsTitle": "Разговоры {{goal}}",
      "loadingItems": "Загрузка {{items}}...",
      "noItemsFound": "{{items}} не найдены.",
      "unnamedCharacter": "Персонаж без имени",
      "untitledPersona": "Персона без названия",
      "untitledLorebook": "Лорбук без названия"
    },
    "advancedModelSettings": {
      "temperature": "Температура",
      "temperatureDesc": "Выше = более креативно",
      "topP": "Top P",
      "topPDesc": "Ниже = более сфокусировано",
      "maxTokens": "Макс. выходных токенов",
      "maxTokensDesc": "Оставьте пустым для значения по умолчанию",
      "contextLength": "Длина контекста",
      "contextLengthDesc": "Только для локальных моделей",
      "contextLengthAuto": "Авто",
      "frequencyPenalty": "Штраф за частоту",
      "frequencyPenaltyDesc": "Уменьшить повторение токенов",
      "presencePenalty": "Штраф за присутствие",
      "presencePenaltyDesc": "Поощрять новые темы",
      "topK": "Top K",
      "topKDesc": "Ограничить пул токенов",
      "reasoning": "Мышление / Рассуждение",
      "reasoningAutoDesc": "Эта модель всегда использует рассуждение. Настройка не нужна.",
      "reasoningEnableDesc": "Включить расширенные возможности мышления для сложных задач, требующих рассуждения.",
      "effortMode": "Режим усилий",
      "budgetMode": "Режим бюджета",
      "reasoningEffort": "Усилие рассуждения",
      "reasoningEffortDesc": "Управление глубиной мышления",
      "reasoningEffortAuto": "Авто",
      "reasoningEffortLow": "Низкое",
      "reasoningEffortMed": "Среднее",
      "reasoningEffortHigh": "Высокое",
      "reasoningBudget": "Бюджет рассуждения (токены)",
      "reasoningBudgetDesc": "Макс. токенов для мышления",
      "reasoningEffortLowDesc": "Быстрые ответы с минимальным рассуждением",
      "reasoningEffortMediumDesc": "Сбалансированная глубина рассуждения",
      "reasoningEffortHighDesc": "Максимальная глубина для сложных задач",
      "reasoningBudgetExtendedDesc": "Макс. токенов для мышления. Добавляется к лимиту вывода."
    },
    "providerParameterSupport": {
      "unknownProvider": "Неизвестный провайдер: {{providerId}}",
      "reasoningNotSupportedEffort": "Не поддерживается — этот провайдер не использует уровни усилий",
      "reasoningNotSupportedBudgetOnly": "Не поддерживается — этот провайдер использует только бюджетный подход",
      "reasoningNotSupported": "Не поддерживается — этот провайдер не поддерживает рассуждение",
      "unsupportedParametersIgnored": "Неподдерживаемые параметры будут проигнорированы {{providerName}}.",
      "reasoningEffortSupported": "Усилие рассуждения поддерживается для моделей с мышлением (o1, DeepSeek-R1 и т.д.)",
      "reasoningBudgetSupported": "Этот провайдер использует бюджетное мышление (без уровней усилий). Установите бюджет токенов рассуждения.",
      "reasoningNotSupportedProvider": "Этот провайдер не поддерживает параметры рассуждения.",
      "matrixTitle": "Матрица поддержки параметров провайдеров",
      "providerColumn": "Провайдер",
      "supportedIndicator": "Поддерживается API провайдера",
      "notSupportedIndicator": "Не поддерживается (параметр будет проигнорирован)"
    },
    "extra": {
      "promptCachingTitle": "Кэширование подсказки",
      "promptCachingDescription": "Ускоряет создание и снижает затраты на длительные повторяющиеся контексты (например, большие системные запросы или истории глубоких чатов).",
      "avatarAlt": "Аватар",
      "chooseFromLibrary": "Выбрать из библиотеки",
      "chooseFromLibraryDesc": "Используйте изображение, уже сохраненное в приложении",
      "generateFailed": "Не удалось создать изображение",
      "editFailed": "Не удалось изменить аватар",
      "backgroundAlt": "Фон для позиции",
      "formatsLoadFailed": "Не удалось загрузить форматы экспорта",
      "formatsShowingDefaults": " (показаны значения по умолчанию)",
      "timeJustNow": "только что",
      "timeMinutesAgo": "_PH_0__м назад",
      "timeHoursAgo": "{{hours}}ч назад",
      "timeDaysAgo": "{{days}}д назад",
      "removeReference": "Удалить ссылку на проект",
      "thumbLoading": "Загрузка...",
      "addReferences": "Добавить ссылки",
      "visualDescription": "Визуальное описание",
      "draftWithAi": "Черновик с AI",
      "regenerate": "Перегенерировать",
      "useThis": "Использовать это",
      "referenceImagesLabel": "Справочные изображения",
      "writerHelpFallback": "совместимая модель сценариста",
      "writerHelpUses": "Использует {{model}} для создания черновиков из вашего аватара и эталонных изображений.",
      "writerHelpUnavailable": "Добавьте совместимую модель сценариста в настройках создания изображений, чтобы создать ее автоматически.",
      "writerNotAvailableError": "Добавьте совместимую модель сценариста в ",
      "writerNoSourcesError": "Перед созданием добавьте аватар или хотя бы одно эталонное изображение.",
      "noUsableReferences": "Не найдено пригодных для использования эталонных изображений.",
      "draftFailed": "Не удалось создать описание дизайна.",
      "draftReadFailed": "Не удалось прочитать ресурс изображения ({{status}})",
      "draftConvertFailed": "Не удалось преобразовать ",
      "draftGenerationFailed": "Не удалось создать черновик.",
      "draftMenuTitle": "AI Проект проекта",
      "draftedBy": "Составлено {{model}} на основе текущего аватара и эталонных изображений.",
      "draftedByFallback": "ваша модель сценариста",
      "noWriterUseHelper": "Добавьте совместимую модель сценариста перед использованием этого помощника.",
      "emptyReferences": "Добавьте несколько четких эталонных снимков, чтобы зафиксировать лицо, пропорции, одежду и стиль.",
      "designReferencesTitle": "Справочники по дизайну",
      "designReferencesDescription": "Загрузите несколько четких эталонных изображений и одно каноническое визуальное описание.",
      "designReferencesPlaceholder": "Опишите стабильный внешний вид: лицо, волосы, телосложение, возраст, особенности одежды, аксессуары и направление искусства/стиля.",
      "dismissAria": "Вышел Dismiss",
      "v3MessageFallback": "lettuce-emb-v4, который значительно улучшает запоминание ролевых игр. ",
      "uploadButton": "Загрузить",
      "libraryButton": "Библиотеку",
      "companionSetupTitle": "Для запуска Companion требуется настройка",
      "companionSetupSubtitleSingle": "Для запуска режима Companion требуется еще одна модель. ",
      "companionSetupSubtitleMany": "Для работы сопутствующего режима требуется еще {{count}} моделей. ",
      "companionSetupBody": "Режиму компаньона нужны некоторые локальные модели для анализа эмоций, извлечения сущностей, маршрутизации воспоминаний и восстановления прошлого контекста.",
      "companionUseRoleplay": "Вместо этого используйте ролевую игру",
      "companionDownloadNow": "Загрузить сейчас",
      "searchModelsPlaceholder": "Поиск моделей...",
      "loadingModelsDefault": "Загрузка моделей...",
      "noModelsAvailable": "Нет доступных моделей.",
      "noModelsMatching": "Модели, соответствующие \"{{query}}\", не найдено.",
      "contentPlaceholderText": "Вы полезный помощник искусственного интеллекта...\n\n",
      "previewRenderFailed": "<не удалось выполнить предварительный просмотр>",
      "charactersCount": "{{count}} символов"
    }
  },
  "chats": {
    "characterNotFound": "Персонаж не найден",
    "chatHistory": "История чата",
    "previousConversationsWithCharacter": "Предыдущие разговоры с {{name}}",
    "previousConversations": "Предыдущие разговоры",
    "searchChats": "Поиск чатов...",
    "searchResults": "{{count}} результат(ов)",
    "noConversationsYet": "Разговоров пока нет",
    "startChattingPrompt": "Начните общение, и ваша история появится здесь",
    "noMatchingChats": "Совпадений не найдено",
    "tryDifferentSearchTerm": "Попробуйте другой поисковый запрос",
    "untitledChat": "Чат без названия",
    "chatTitlePlaceholder": "Название чата...",
    "exportChatPackage": "Экспорт пакета чата",
    "characterSpecificExport": "Экспорт для конкретного персонажа",
    "characterSpecificExportDesc": "Привязать этот пакет к персонажу чата по умолчанию.",
    "nonCharacterSpecificExport": "Общий экспорт",
    "nonCharacterSpecificExportDesc": "Требует выбора персонажа при импорте.",
    "deleteChat": "Удалить чат?",
    "deleteConfirmDesc": "Безвозвратно удалит из истории",
    "keepThisChat": "Оставить этот чат",
    "editCharacter": "Редактировать персонажа",
    "exportCharacter": "Экспорт персонажа",
    "chatAppearance": "Оформление чата",
    "importChatPackage": "Импорт пакета чата",
    "hideThisCharacter": "Скрыть этого персонажа",
    "deleteCharacter": "Удалить персонажа",
    "deleteCharacterTitle": "Удалить персонажа?",
    "deleteCharacterConfirmation": "Вы уверены, что хотите удалить \"{{name}}\"? Это также удалит все сеансы чата с этим персонажем.",
    "characterSpecificMatches": "Этот пакет привязан к персонажу и соответствует выбранному.",
    "characterSpecificMismatch": "Этот пакет привязан к другому персонажу. Он будет импортирован в {{name}}.",
    "nonCharacterSpecificImport": "Этот пакет не привязан к персонажу. Он будет импортирован в {{name}}.",
    "noCharactersYet": "Персонажей пока нет",
    "createFirstCharacter": "Создайте первого персонажа с помощью кнопки + ниже, чтобы начать общение",
    "scrollToBottom": "Прокрутить вниз",
    "selectCharacter": "Выбрать персонажа",
    "branchToGroupChat": "Ответвить в групповой чат",
    "addContent": "Добавить контент",
    "swapPlaces": "Поменять местами",
    "swapPlacesOn": "Поменять местами (вкл.)",
    "uploadImage": "Загрузить изображение",
    "helpMeReply": "Помоги ответить",
    "sceneImage": {
      "modeTitle": "Изображение сцены",
      "modeDescription": "Выберите, писать ли сцену подсказку или позволить ИИ сначала ее набросать.",
      "writePrompt": "Напишите подсказку",
      "writePromptDesc": "Введите точную подсказку изображения сцены, которую вы хотите использовать.",
      "askAi": "Спросите ИИ",
      "askAiDesc": "Пусть текущая модель чата нарисует подсказку сцены с выбранного момента.",
      "generateTitle": "Создать изображение сцены",
      "regenerateTitle": "Восстановить изображение сцены",
      "aiTitle": "Подсказка сцены AI",
      "promptLabel": "ПОДПИСКА СЦЕНЫ",
      "promptPlaceholder": "Опишите сцену, персонажей, настроение, освещение, кадр камеры и важные детали...",
      "suggestedPrompt": "Предлагаемая подсказка",
      "regeneratePrompt": "Регенерировать",
      "editPrompt": "Изменить приглашение",
      "reviewTitle": "Проверить промпт сцены",
      "denyPrompt": "Отклонить",
      "acceptPrompt": "Принять",
      "generateImage": "Создать изображение",
      "updateImage": "Обновить изображение"
    },
    "useMyTextAsBase": "Использовать мой текст как основу",
    "writeNewReply": "Написать что-то новое",
    "suggestedReply": "Предложенный ответ",
    "selectTwoCharactersMinimum": "Выберите минимум 2 персонажа для группового чата.",
    "groupBranchCreated": "Групповая ветка создана! Перенаправление...",
    "memories": "Воспоминания",
    "tools": "Инструменты",
    "pinned": "Закреплённые",
    "searchMemories": "Поиск воспоминаний...",
    "addMemory": "Добавить воспоминание",
    "memoryActions": "Действия с памятью",
    "pinnedMessages": "Закреплённые сообщения",
    "pinnedMessagesDesc": "Всегда включаются в контекст",
    "contextSummary": "Краткое содержание контекста",
    "contextSummaryPlaceholder": "Краткое резюме для поддержания согласованности контекста между сообщениями...",
    "memoryContentPlaceholder": "Что нужно запомнить?",
    "editMemoryPlaceholder": "Введите содержание воспоминания...",
    "togglePin": {
      "pin": "Закрепить",
      "unpin": "Открепить"
    },
    "toggleMemoryState": {
      "setHot": "Сделать горячим",
      "setCold": "Сделать холодным"
    },
    "selectModelForRetry": "Выберите модель для повтора",
    "memoryCategories": {
      "characterTrait": "черта персонажа",
      "relationship": "отношения",
      "plotEvent": "событие сюжета",
      "worldDetail": "деталь мира",
      "preference": "предпочтение",
      "other": "другое"
    },
    "settings": {
      "memorySection": "Память",
      "memorySectionDesc": "Резюме, теги, история вызовов инструментов",
      "quickSettings": "Быстрые настройки",
      "quickSettingsDesc": "Частые настройки",
      "persona": "Персона",
      "model": "Модель",
      "fallbackModel": "Резервная модель",
      "voice": "Голос",
      "voiceDesc": "Озвучивание текста",
      "advanced": "Расширенные",
      "advancedDesc": "Переопределение параметров модели для этого сеанса",
      "session": "Сеанс",
      "sessionDesc": "Новые чаты и история",
      "newChat": "Новый чат",
      "newChatDesc": "Начать новый разговор",
      "chatHistoryDesc": "Просмотр предыдущих сеансов",
      "importChatPackageDesc": "Импортировать .chatpkg в этого персонажа",
      "selectModel": "Выбрать модель",
      "selectFallbackModel": "Выбрать резервную модель",
      "personaActions": "Действия с персоной",
      "sessionAdvancedSettings": "Расширенные настройки сеанса",
      "parameterSupport": "Поддержка параметров",
      "backToChat": "Назад в чат",
      "chatSettingsTitle": "Настройки чата",
      "chatSettingsSubtitle": "Управление настройками беседы",
      "modelDefaults": "Настройки модели по умолчанию",
      "appDefaults": "Настройки приложения по умолчанию",
      "openChatSessionFirst": "Сначала откройте сеанс чата",
      "sessionRequired": "Требуется сеанс",
      "noPersona": "Нет персонажа",
      "customPersona": "Пользовательский персонаж",
      "noModelAvailable": "Модель недоступна",
      "fallbackNone": "Нет",
      "unknownModel": "Неизвестная модель",
      "authorNote": "Примечание автора",
      "identityProfileAuthored": "Автор профиля личности",
      "addIdentityProfile": "Добавить профиль личности компаньона",
      "soulLabel": "Soul",
      "sessionTitle": "Сеанс: {{title}}",
      "sessionUntitled": "Untitled",
      "messageCount": "{{count}} сообщения",
      "usingCharacterDefault": "Использование символа по умолчанию",
      "sessionOverrideActive": "Активное переопределение сеанса",
      "autoplayVoice": "Автовоспроизведение голоса",
      "useCharacterDefault": "Использовать символ по умолчанию"
    },
    "search": {
      "placeholder": "Поиск в разговоре...",
      "noMessagesFound": "Сообщения не найдены",
      "you": "Вы",
      "character": "Персонаж",
      "failed": "Не удалось выполнить поиск сообщений"
    },
    "templates": {
      "startWithTemplate": "Начать с шаблона?",
      "noTemplate": "Без шаблона",
      "startWithSceneOnly": "Начать только со сцены",
      "you": "Вы",
      "bot": "Бот",
      "messageCount": "{{count}} сообщений"
    },
    "header": {
      "back": "Назад",
      "openSettings": "Открыть настройки чата",
      "manageMemories": "Управление воспоминаниями",
      "searchMessages": "Поиск сообщений",
      "manageLorebooks": "Управление лорбуками",
      "conversationSettings": "Настройки разговора"
    },
    "footer": {
      "sendMessagePlaceholder": "Отправить сообщение...",
      "stopGeneration": "Остановить генерацию",
      "sendMessage": "Отправить сообщение",
      "continueConversation": "Продолжить разговор",
      "moreOptions": "Ещё",
      "addImage": "Добавить изображение",
      "addImageAttachment": "Добавить вложение",
      "removeAttachment": "Удалить вложение",
      "recordVoice": "Записать голос"
    },
    "message": {
      "thinkingMessages": {
        "thinkingReallyHard": "Усиленно думаю…",
        "lettuceCouncil": "Совещаюсь с советом латука…",
        "stealingThoughts": "Краду мысли из пустоты…",
        "warmingBrainCells": "Разогреваю мозговые клетки…",
        "forbiddenKnowledge": "Загружаю запретные знания…",
        "overthinking": "Перемудриваю (как обычно)…",
        "pretendingToBeSmart": "Притворяюсь умным…",
        "crunchingNumbers": "Считаю воображаемые числа…",
        "arguingWithMyself": "Спорю сам с собой…",
        "askingUniverse": "Вежливо спрашиваю у вселенной…"
      },
      "thoughtProcess": "Ход мыслей",
      "regenerateResponse": "Перегенерировать ответ",
      "guidedRegenerationTitle": "Регенерация руководства",
      "guidedRegenerationLabel": "Как это должно измениться?",
      "guidedRegenerationDescription": "Опишите тон, длину, детали, которые следует сохранить или удалить, а также все, что в следующем ответе должно быть изменено.",
      "guidedRegenerationPlaceholder": "Сделайте его короче, теплее, более прямым...",
      "guidedRegenerationSubmit": "Перегенерировать",
      "cancelAudioGeneration": "Отменить генерацию аудио",
      "stopAudio": "Остановить аудио",
      "playMessageAudio": "Воспроизвести аудио сообщения",
      "playAudio": "Воспроизвести",
      "sceneLabel": "Сцена",
      "variantLabel": "Вариант",
      "regenerating": "Перегенерация",
      "assistantIsTyping": "Ассистент печатает",
      "attachedImage": "Прикреплённое изображение"
    },
    "actions": {
      "assistantMessage": "Сообщение ассистента",
      "userMessage": "Сообщение пользователя",
      "promptTokens": "Токены промпта",
      "completionTokens": "Токены ответа",
      "fallbackModelUsed": "Использована резервная модель",
      "total": "всего",
      "timeToFirstToken": "Время до первого токена",
      "completionTokenSpeed": "Скорость генерации токенов",
      "edit": "Редактировать",
      "copy": "Копировать",
      "pin": "Закрепить",
      "unpin": "Открепить",
      "rewindToHere": "Перемотать сюда",
      "branchFromHere": "Ответвить отсюда",
      "branchToGroupChat": "Ответвить в групповой чат",
      "branchToCharacter": "Ответвить к персонажу",
      "generateSceneImage": "Создать изображение сцены",
      "regenerateSceneImage": "Восстановить изображение сцены",
      "chatAppearance": "Оформление чата",
      "delete": "Удалить",
      "debug": "Отладка",
      "unpinToDelete": "Открепите, чтобы удалить",
      "editPlaceholder": "Отредактируйте сообщение...",
      "memoriesUsed": "{{count}} воспоминаний использовано",
      "lorebookUsage": "Использование лорбука",
      "lorebookUsageDesc": "Этот ответ использовал следующие записи лорбука.",
      "matchScore": "Совпадение: {{score}}%",
      "unknownModel": "Неизвестная модель",
      "loadingModel": "Загрузка модели..."
    },
    "emptyState": {
      "goBack": "Вернуться"
    },
    "settingsDrawer": {
      "title": "Настройки чата",
      "subtitle": "Управление предпочтениями разговора"
    },
    "history": {
      "archivedBadge": "Архивированные",
      "messagesCount": "{{count}} сообщения",
      "previousGroupPage": "Предыдущая {{label}} страница",
      "nextGroupPage": "Далее {{label}} страница",
      "sillyTavernFormat": "SillyTavern format",
      "sillyTavernFormatDesc": "Экспортируйте как .jsonl, совместимый с SillyTavern.",
      "failedLoad": "Не удалось загрузить данные",
      "failedDelete": "Не удалось удалить: {{error}}",
      "failedRename": "Не удалось переименовать: {{error}}",
      "chatPackageExportedTo": "Пакет чата экспортирован в:\n",
      "sillyTavernExportedTo": "Чат SillyTavern экспортирован в:\n",
      "failedExportChatPackage": "Не удалось экспортировать пакет чата.",
      "failedExportSillyTavern": "Не удалось экспортировать чат SillyTavern"
    },
    "authorNote": {
      "title": "Примечание автора",
      "inlineEditor": "Встроенный редактор",
      "inlineEditorDesc": "Показывать примечание автора над вводом в чате для быстрого редактирования.",
      "toggleInlineEditor": "Переключить встроенный редактор примечаний автора",
      "placeholder": "Частное направление для этого чата",
      "willBeRemoved": "Примечание автора будет удалено при сохранении",
      "noNoteSaved": "Примечание автора не сохранено",
      "charactersCount": "{{count}} символов",
      "clear": "Очистить",
      "save": "Сохранить",
      "saving": "Сохранение...",
      "failedSave": "Не удалось сохранить примечание автора",
      "addPlaceholder": "Добавить примечание автора...",
      "savingShort": "Сохранение..."
    },
    "drawer": {
      "chatSettingsTitle": "Настройки чата",
      "chatSettingsSubtitle": "Управление настройками беседы"
    },
    "companionSoul": {
      "loading": "Загрузка Companion Soul...",
      "unavailable": "Душа-компаньон недоступна",
      "unavailableDesc": "Редактирование души доступно только для персонажей в режиме компаньона.",
      "pageTitle": "Душа-компаньон",
      "back": "Назад",
      "save": "Сохранить"
    },
    "companionRelationship": {
      "back": "Назад",
      "loading": "Загрузка состояния отношений...",
      "unavailableTitle": "Состояние отношений недоступно",
      "sessionLoadFailed": "Не удалось загрузить сеанс чата.",
      "backToChat": "Вернуться в чат",
      "notCompanionTitle": "Этот чат не находится в режиме компаньона",
      "notCompanionDesc": "Страницы взаимоотношений компаньонов отображаются только для чатов, в которых режимом символов является компаньон.",
      "openRegularMemories": "Открытие обычных воспоминаний",
      "pageTitle": "Состояние отношений",
      "memoryButton": "Память",
      "lastInteraction": "Последнее взаимодействие {{time}}",
      "bond": "Bond",
      "closeness": "Closeness",
      "trust": "Trust",
      "affection": "Affection",
      "tension": "Tension",
      "stability": "Стабильность",
      "interactions": "Взаимодействия",
      "vsDefaults": "vs. ",
      "updatedAt": "Обновлен {{time}}",
      "emotionalEngine": "Эмоциональный двигатель",
      "felt": "Felt",
      "feltDesc": "Внутренний аффект",
      "expressed": "Выраженный",
      "expressedDesc": "Поверхности в ответах",
      "blocked": "Заблокирован",
      "blockedDesc": "Подавлен персоной",
      "momentum": "Momentum",
      "momentumDesc": "Тенденция последних поворотов",
      "activeDrivers": "Активные драйверы",
      "soul": "Soul",
      "essence": "Essence",
      "voice": "Voice",
      "relationalStyle": "Стиль отношений",
      "vulnerabilities": "Уязвимости",
      "habits": "Привычки",
      "boundaries": "Границы",
      "eventsCount": "{{count}} события",
      "recentTimeline": "Последние сроки",
      "superseded": "заменено",
      "promptScore": "Подскажите {{score}}",
      "persistenceScore": "Постоянство {{score}}",
      "noTimeline": " Временной шкалы пока нет",
      "noTimelineDesc": "Воспоминания об отношениях, вехах и эмоциональных моментальных снимках будут появляться здесь по мере того, как собеседник узнает из разговоров.",
      "notAuthoredYet": "Пока не создан.",
      "noSignal": "Нет сигнала."
    },
    "companionUi": {
      "relationship": "Отношения",
      "milestones": "Вехи",
      "boundaries": "Границы",
      "preferences": "Предпочтения",
      "profile": "Профиль",
      "routines": "Процедуры",
      "episodes": "Эпизоды",
      "emotionalSnapshots": "Эмоциональные снимки",
      "unknownTime": "Неизвестно",
      "justNow": "Только что",
      "minutesAgo": "{{minutes}}м назад",
      "hoursAgo": "{{hours}}ч назад",
      "daysAgo": "{{days}}дн назад",
      "notSetYet": "Пока не установлено",
      "missingCharacterId": "Отсутствует идентификатор символа",
      "sessionNotFound": "Сеанс не найден",
      "failedLoadCompanion": "Не удалось загрузить сопутствующий сеанс"
    },
    "chatPage": {
      "characterNotFound": "Персонаж не найден",
      "characterDoesntExist": "Персонаж, которого вы ищете, не существует."
    },
    "debugPage": {
      "copy": "Копировать"
    },
    "companionMemoryPage": {
      "backLabel": "Назад",
      "refineMemoryPlaceholder": "Уточнить сохраненную память компаньона",
      "superseded": "superseded",
      "pinned": "pinned",
      "cold": "холодно"
    }
  },
  "groupChats": {
    "list": {
      "editGroup": "Редактировать группу",
      "deleteGroup": "Удалить группу",
      "deleteConfirmTitle": "Удалить групповой чат?",
      "deleteConfirmMessage": "Вы уверены, что хотите удалить \"{{name}}\"? Это также удалит все сообщения в этом групповом чате.",
      "noGroupChatsYet": "Групповых чатов пока нет",
      "noGroupChatsDesc": "Создайте первый групповой чат с помощью кнопки + ниже, чтобы общаться с несколькими персонажами одновременно",
      "newChat": "Новый чат",
      "openChat": "Открыть чат",
      "chatSettings": "Настройки чата",
      "sessionCount": "{{count}} чатов"
    },
    "create": {
      "invalidPackage": "Этот пакет не является пакетом группового чата.",
      "inspectPackageError": "Не удалось проверить пакет группового чата",
      "importPackageError": "Не удалось импортировать пакет группового чата",
      "importChatpkg": "Импорт `.chatpkg`",
      "mapParticipantsTitle": "Сопоставить участников",
      "selectLocalCharacter": "Выберите локального персонажа для этого участника.",
      "selectCharacterPlaceholder": "Выберите персонажа...",
      "importChatPackageTitle": "Импорт пакета чата",
      "importChatPackageDesc": "Это импортирует выбранный `.chatpkg` как новый групповой сеанс.",
      "characterSelect": {
        "title": "Создать групповой чат",
        "subtitle": "Выберите персонажей для группового разговора",
        "selected": "выбрано",
        "ready": "Готово",
        "minRequired": "Мин. 2 обязательно",
        "noCharactersYet": "Персонажей пока нет",
        "noCharactersDesc": "Сначала создайте персонажей для группового чата",
        "continueToSetup": "Перейти к настройке группы"
      },
      "groupSetup": {
        "title": "Настройка группы",
        "subtitle": "Настройте параметры группового чата",
        "chatType": "Тип чата",
        "conversation": "Разговор",
        "casualChat": "Свободное общение",
        "roleplay": "Ролевая игра",
        "withScenes": "Со сценами",
        "conversationDesc": "Свободный групповой разговор без начальных сцен",
        "roleplayDesc": "Ролевой сценарий с начальной сценой и атмосферными промптами",
        "speakerSelection": "Выбор говорящего",
        "llm": "LLM",
        "aiPicks": "AI выбирает",
        "heuristic": "Эвристика",
        "scoreBased": "На основе оценки",
        "roundRobin": "По кругу",
        "takeTurns": "По очереди",
        "llmDesc": "Использует вашу модель по умолчанию для выбора говорящего (расходует токены)",
        "heuristicDesc": "Использует баланс участия и контекстные подсказки (бесплатно)",
        "roundRobinDesc": "Персонажи говорят по очереди (бесплатно)",
        "chatBackground": "Фон чата",
        "optional": "(Необязательно)",
        "uploadBackground": "Загрузить фоновое изображение",
        "backgroundDesc": "Установить фон для этого группового чата",
        "groupName": "Название группы",
        "removeBackground": "Удалить фоновое изображение",
        "groupNameAutoGenerate": "Оставьте пустым для автогенерации из имён персонажей",
        "continueToScene": "Перейти к начальной сцене",
        "createGroupChat": "Создать групповой чат"
      },
      "startingScene": {
        "title": "Начальная сцена",
        "subtitle": "Задайте начальный сценарий для ролевой игры",
        "sceneSource": "Источник сцены",
        "none": "Нет",
        "custom": "Пользовательский",
        "fromCharacter": "От персонажа",
        "noneDesc": "Начать без заданной сцены",
        "customDesc": "Напишите свой начальный сценарий",
        "fromCharacterDesc": "Использовать сцену одного из персонажей",
        "sceneContent": "Содержание сцены",
        "sceneContentPlaceholder": "Опишите начальную сцену для ролевой игры...",
        "sceneReferenceTip": "Подсказка: Введите {{@\" чтобы упомянуть персонажей",
        "selectScene": "Выбрать сцену",
        "sceneLabel": " — Сцена",
        "copyToCustom": "Скопировать и редактировать"
      }
    },
    "history": {
      "title": "История групповых чатов",
      "subtitle": "Все групповые разговоры",
      "searchPlaceholder": "Поиск групповых чатов...",
      "active": "Активные ({{count}})",
      "archived": "Архивные ({{count}})",
      "noChatsYet": "Групповых чатов пока нет",
      "noChatsDesc": "Создайте групповой чат, чтобы увидеть историю здесь",
      "noMatchingChats": "Совпадений не найдено",
      "noMatchingDesc": "Попробуйте другой поисковый запрос",
      "deleteSessionTitle": "Удалить групповой чат?",
      "deleteSessionDesc": "Безвозвратно удалит из истории",
      "deleteSessionButton": "Удалить чат",
      "keepChat": "Оставить этот чат"
    },
    "session": {
      "chatTitlePlaceholder": "Название чата...",
      "newChat": "Новый чат",
      "rename": "Переименовать",
      "unarchive": "Разархивировать",
      "archive": "Архивировать",
      "messageCount": "{{count}} сообщений"
    },
    "memories": {
      "tabMemories": "Воспоминания",
      "tabPinned": "Закреплённые",
      "tabActivity": "Активность",
      "processing": "Обработка",
      "contextSummaryTitle": "Краткое содержание контекста",
      "addContextSummaryPrompt": "Нажмите, чтобы добавить краткое содержание...",
      "savedMemories": "Сохранённые воспоминания",
      "resultsCount": "Результаты ({{count}})",
      "searchPlaceholder": "Поиск воспоминаний...",
      "addMemory": "Добавить воспоминание",
      "noMemoriesYet": "Воспоминаний пока нет",
      "noMemoriesDesc": "Нажмите кнопку «Добавить» выше, чтобы создать",
      "noMatchingMemories": "Совпадений не найдено",
      "noMatchingDesc": "Попробуйте другой поисковый запрос",
      "sessionNotFound": "Сеанс не найден",
      "memoryActions": "Действия с памятью",
      "tokens": "токены",
      "cycle": "Цикл",
      "accessed": "Доступ",
      "cold": "Холодная",
      "hot": "Горячая",
      "activityLog": "Журнал активности",
      "events": "события",
      "run": "Запуск",
      "processingMemories": "AI организует групповые воспоминания...",
      "memoryCycleSuccess": "Цикл памяти успешно обработан!",
      "memoryActionFailed": "Действие с памятью не удалось",
      "newMemoryUpdates": "Доступны новые обновления памяти",
      "noActivityYet": "Активности пока нет",
      "noActivityDesc": "Вызовы инструментов появляются, когда AI управляет воспоминаниями в динамическом режиме",
      "contextSummaryPlaceholder": "Краткое резюме для поддержания согласованности контекста...",
      "addMemoryTitle": "Добавить воспоминание",
      "memoryPlaceholder": "Что нужно запомнить?",
      "saveMemory": "Сохранить воспоминание",
      "editMemoryTitle": "Редактировать воспоминание",
      "editMemoryPlaceholder": "Введите содержание воспоминания...",
      "edit": "Редактировать",
      "pin": "Закрепить",
      "unpin": "Открепить",
      "setHot": "Сделать горячим",
      "setCold": "Сделать холодным"
    },
    "toolLog": {
      "created": "Создано",
      "deleted": "Удалено",
      "pinned": "Закреплено",
      "unpinned": "Откреплено",
      "done": "Готово",
      "pinnedBadge": "закреплено",
      "softDelete": "мягкое удаление",
      "memoryCycle": "Цикл памяти",
      "failedAt": "Ошибка в:",
      "window": "Окно",
      "justNow": "только сейчас",
      "minutesAgo": "{{count}}м назад",
      "hoursAgo": "{{count}}ч назад",
      "yesterday": "вчера",
      "daysAgo": "{{count}}д назад",
      "revertingTitle": "Возврат...",
      "revertCycleTitle": "Вернуть этот цикл",
      "revertedAt": "Вернуть {{time}}",
      "failedAtStage": "Ошибка: {{stage}}",
      "hideDebug": "Скрыть отладку",
      "debug": "Debug",
      "windowRange": "Окно {{start}}-{{end}}",
      "actionCreated": "Создано",
      "actionDeleted": "Удален",
      "actionPinned": "Закреплено",
      "actionUnpinned": "Откреплено",
      "actionDone": "Готово",
      "badgePinned": "pinned",
      "badgeSoftDelete": "soft-delete",
      "badgeUndone": "undone",
      "badgeReverted": "reverted",
      "activityEmptyTitle": "Нет активности",
      "activityEmptyDesc": "Вызовы инструментов появляются, когда ИИ управляет памятью в динамическом режиме"
    },
    "message": {
      "thinkingHard": "Усиленно думаю…",
      "thinkingLettuce": "Совещаюсь с советом латука…",
      "thinkingVoid": "Краду мысли из пустоты…",
      "thinkingBrainCells": "Разогреваю мозговые клетки…",
      "thinkingForbidden": "Загружаю запретные знания…",
      "thinkingOverthinking": "Перемудриваю (как обычно)…",
      "thinkingPretending": "Притворяюсь умным…",
      "thinkingCrunching": "Считаю воображаемые числа…",
      "thinkingArguing": "Спорю сам с собой…",
      "thinkingUniverse": "Вежливо спрашиваю у вселенной…",
      "thoughtProcess": "Ход мыслей",
      "userAlt": "Пользователь",
      "assistantAlt": "Ассистент",
      "regenerateResponse": "Перегенерировать ответ",
      "variantLabel": "Вариант",
      "regenerating": "Перегенерация",
      "cancelAudioGeneration": "Отменить генерацию аудио",
      "stopAudio": "Остановить аудио",
      "playMessageAudio": "Воспроизвести аудио сообщения",
      "playAudio": "Воспроизвести",
      "attachedImage": "Прикреплённое изображение",
      "assistantIsTyping": "Ассистент печатает",
      "assistantTyping": "Ассистент печатает"
    },
    "header": {
      "back": "Назад",
      "memories": "Воспоминания",
      "settings": "Настройки",
      "characters": "персонажи"
    },
    "footer": {
      "mentionCharacter": "Упомянуть персонажа",
      "noCharactersFound": "Персонажи не найдены",
      "moreOptions": "Ещё",
      "addImage": "Добавить изображение",
      "messagePlaceholder": "Сообщение... (@ для упоминания)",
      "stopGeneration": "Остановить генерацию",
      "sendMessage": "Отправить сообщение",
      "continueConversation": "Продолжить разговор",
      "dismissError": "Закрыть ошибку",
      "removeAttachment": "Удалить вложение",
      "tabToSelect": "Tab для выбора"
    },
    "messageActions": {
      "characterMessage": "Сообщение персонажа",
      "yourMessage": "Ваше сообщение",
      "whyCharacterResponded": "Почему ответил этот персонаж",
      "total": "всего",
      "edit": "Редактировать",
      "copy": "Копировать",
      "regenerateWithDifferent": "Перегенерировать с другим персонажем",
      "rewindToHere": "Перемотать сюда",
      "unpinToDelete": "Открепите, чтобы удалить",
      "delete": "Удалить",
      "editPlaceholder": "Отредактируйте сообщение...",
      "chooseCharacterTitle": "Выбрать персонажа",
      "selectCharacterForRegeneration": "Выберите, какой персонаж должен ответить вместо этого:"
    },
    "settings": {
      "appDefault": "По умолчанию",
      "selectPersona": "Выбрать персону",
      "noPersonas": "Персоны недоступны",
      "noPersonasDesc": "Создайте персону в настройках, чтобы персонализировать ваши разговоры.",
      "searchPersonas": "Поиск персон...",
      "noPersona": "Без персоны",
      "noPersonaDesc": "Продолжить без персоны",
      "noPersonasFound": "Персоны не найдены",
      "noPersonasFoundDesc": "Попробуйте другой поисковый запрос"
    },
    "groupSettings": {
      "title": "Редактировать группу",
      "subtitle": "Обновить настройки по умолчанию для будущих сессий",
      "enterGroupName": "Введите название группы",
      "participant": "участник",
      "participants": "участников",
      "uploading": "Загрузка...",
      "changeBackground": "Изменить фон",
      "addBackgroundImage": "Добавить фоновое изображение",
      "removeBackground": "Удалить фон",
      "persona": "Персона",
      "personaSubtitle": "Персона по умолчанию для новых сессий",
      "personaLabel": "Персона",
      "speakerSelection": "Выбор говорящего",
      "speakerSubtitle": "Метод по умолчанию для новых сессий",
      "llm": "LLM",
      "aiPicks": "ИИ выбирает",
      "heuristic": "Эвристический",
      "scoreBased": "На основе баллов",
      "roundRobin": "По кругу",
      "takeTurns": "По очереди",
      "llmDesc": "Использует вашу модель по умолчанию для выбора говорящего (стоит токены)",
      "heuristicDesc": "Использует баланс участия и контекстные подсказки (бесплатно)",
      "roundRobinDesc": "Персонажи говорят по очереди (бесплатно)",
      "memoryMode": "Режим памяти",
      "memorySubtitle": "Режим памяти по умолчанию для новых сессий",
      "manual": "Ручной",
      "manualDesc": "Управляйте заметками самостоятельно",
      "dynamic": "Динамический",
      "dynamicDesc": "Автоматическое извлечение",
      "memoryDynamicInfo": "ИИ автоматически создаёт и извлекает воспоминания из разговоров",
      "memoryManualInfo": "Вы самостоятельно добавляете и управляете заметками памяти",
      "characters": "Персонажи",
      "participantsActive": "{{total}} участников · {{active}} активных",
      "add": "Добавить",
      "muted": "(отключён)",
      "mutedByDefault": "Отключён по умолчанию",
      "activeByDefault": "Активен по умолчанию",
      "unmuteCharacter": "Включить звук персонажа",
      "muteCharacter": "Отключить звук персонажа",
      "minTwoRequired": "Минимум 2 персонажа",
      "removeCharacter": "Удалить персонажа",
      "groupMinCharacters": "Группа требует минимум 2 персонажей",
      "mutedCharactersNote": "Отключённые персонажи пропускаются при автоматическом выборе говорящего, но могут отвечать через явное `@упоминание`.",
      "addCharacterTitle": "Добавить персонажа",
      "allCharactersInGroup": "Все персонажи уже в этой группе.",
      "removeCharacterTitle": "Удалить персонажа?",
      "removeCharacterConfirm": "Вы уверены, что хотите удалить",
      "removeCharacterFrom": "из настроек группы по умолчанию?",
      "removing": "Удаление...",
      "remove": "Удалить"
    },
    "sessionSettings": {
      "subtitle": "Управление настройками группового чата",
      "enterGroupName": "Введите название группы",
      "participant": "участник",
      "participants": "участников",
      "message": "сообщение",
      "messages": "сообщений",
      "uploading": "Загрузка...",
      "changeBackground": "Изменить фон",
      "addBackgroundImage": "Добавить фоновое изображение",
      "removeBackground": "Удалить фон",
      "persona": "Персона",
      "personaSubtitle": "Ваша идентичность в этом разговоре",
      "personaLabel": "Персона",
      "speakerSelection": "Выбор говорящего",
      "speakerSubtitle": "Как выбирается следующий говорящий",
      "llm": "LLM",
      "aiPicks": "ИИ выбирает",
      "heuristic": "Эвристический",
      "scoreBased": "На основе баллов",
      "roundRobin": "По кругу",
      "takeTurns": "По очереди",
      "llmDesc": "Использует вашу модель по умолчанию для выбора говорящего (стоит токены)",
      "heuristicDesc": "Использует баланс участия и контекстные подсказки (бесплатно)",
      "roundRobinDesc": "Персонажи говорят по очереди (бесплатно)",
      "characters": "Персонажи",
      "participantsActive": "{{total}} участников · {{active}} активных",
      "add": "Добавить",
      "muted": "(отключён)",
      "unmuteCharacter": "Включить звук персонажа",
      "muteCharacter": "Отключить звук персонажа",
      "minTwoRequired": "Минимум 2 персонажа",
      "removeCharacter": "Удалить персонажа",
      "groupMinCharacters": "Групповой чат требует минимум 2 персонажей",
      "mutedCharactersNote": "Отключённые персонажи пропускаются при автоматическом выборе говорящего, но могут отвечать через явное `@упоминание`.",
      "data": "Данные",
      "dataSubtitle": "Экспорт или импорт разговоров",
      "export": "Экспорт",
      "exportDesc": "Сохранить как файл для обмена",
      "import": "Импорт",
      "importDesc": "Загрузить разговор из файла",
      "conversation": "Разговор",
      "conversationSubtitle": "Дублировать или продолжить в новом чате",
      "duplicate": "Дублировать",
      "duplicateDesc": "Копировать этот чат с сообщениями или без",
      "branchTo1on1": "Ветвление в 1-на-1",
      "branchTo1on1Desc": "Продолжить приватно с одним персонажем",
      "participation": "Участие",
      "participationSubtitle": "Распределение высказываний между персонажами",
      "addCharacterTitle": "Добавить персонажа",
      "allCharactersInGroup": "Все персонажи уже в этой группе.",
      "removeCharacterTitle": "Удалить персонажа?",
      "removeCharacterConfirm": "Вы уверены, что хотите удалить",
      "removeCharacterFrom": "из этого группового чата?",
      "removing": "Удаление...",
      "remove": "Удалить",
      "cloneGroupTitle": "Клонировать группу",
      "withMessages": "С сообщениями",
      "withMessagesDesc": "Клонировать всё включая историю чата",
      "withoutMessages": "Без сообщений",
      "withoutMessagesDesc": "Клонировать только настройки (персонажи, начальная сцена)",
      "branchWithCharacterTitle": "Ветвление с персонажем",
      "branchWithCharacterDesc": "Выберите персонажа для продолжения в формате 1-на-1. Все сообщения из этой группы будут конвертированы.",
      "continueWith": "Продолжить разговор с {{name}}",
      "exportChatPackageTitle": "Экспорт пакета чата",
      "includeCharacterSnapshots": "Включить снапшоты персонажей",
      "includeCharacterSnapshotsDesc": "Сохранить данные персонажей в пакете",
      "sessionOnly": "Только сессия",
      "sessionOnlyDesc": "Экспортировать только сообщения и метаданные",
      "mapParticipantsTitle": "Сопоставить участников",
      "selectLocalCharacter": "Выберите локального персонажа для этого участника.",
      "selectCharacterPlaceholder": "Выбрать персонажа...",
      "continue": "Продолжить",
      "importChatPackageTitle": "Импорт пакета чата",
      "importChatPackageDesc": "Это импортирует выбранный `.chatpkg` как новую групповую сессию.",
      "importing": "Импорт..."
    },
    "page": {
      "selectingCharacter": "Выбор символа...",
      "sessionNotFound": "Групповая сессия не найдена",
      "backToGroupChats": "Вернуться к групповым чатам",
      "startConversation": "Начать разговор с {{names}}",
      "scrollToBottom": "Прокрутить вниз",
      "addContent": "Добавить контент",
      "uploadImage": "Загрузить изображение",
      "helpMeReply": "Помощь мне ответить",
      "helpMeReplyDesc": "Пусть ИИ подскажет, что сказать",
      "haveDraftPrompt": "У вас есть черновик сообщения. ",
      "useMyTextAsBase": "Используйте мой текст в качестве основы",
      "useMyTextAsBaseDesc": "Расширьте и улучшите свой черновик",
      "writeSomethingNew": "Напишите что-нибудь новое",
      "writeSomethingNewDesc": "Создайте новый ответ",
      "suggestedReply": "Предлагаемый ответ",
      "reasoningBeforeWriting": "Обоснование перед написанием ответа...",
      "writingYourReply": "Написание ответа...",
      "regenerate": "Восстановить",
      "useReply": "Использовать ответ",
      "helpMeReplyFailedGeneric": "Помощь мне Не удалось ответить.",
      "helpMeReplyFailedGenerate": "Помощь мне ответить не удалось создать ответ.",
      "noAudioCaptured": "Аудио не записано.",
      "noWhisperModel": "Не установлено ",
      "cancelRecording": "Отменить запись",
      "transcribing": "Транскрибирование",
      "stopAndTranscribe": "Остановить и расшифровать",
      "recordVoice": "Записать голос",
      "learnCorrection": "Выучить исправление:",
      "learning": "Обучение...",
      "learn": "Изучите",
      "ignore": "Игнорировать",
      "groupChatFailed": "Не удалось выполнить групповой чат.",
      "failedToCopy": "Не удалось скопировать",
      "copied": "Скопировано!",
      "cannotDeletePinned": "Невозможно удалить закрепленное сообщение. ",
      "failedToDelete": "Не удалось удалить",
      "messageNotFound": "Сообщение не найдено",
      "cannotRewindPinned": "Невозможно перемотать назад: после этого момента есть закрепленные сообщения. ",
      "failedToRewind": "Не удалось перемотать назад",
      "failedToTogglePin": "Не удалось переключить контакт",
      "messagePinned": "Сообщение закреплено",
      "messageUnpinned": "Сообщение не закреплено",
      "failedToSave": "Не удалось сохранить"
    },
    "memoriesPage": {
      "summarizingConversation": "Подведение итогов беседы",
      "analyzingMemories": "Анализ воспоминаний",
      "applyingChanges": "Применение изменений",
      "organizingMemories": "Организация воспоминаний",
      "retryingMemoryCycle": "Повтор цикла памяти...",
      "processingMemories": "Обработка памяти...",
      "memorySystemError": "Ошибка системы памяти",
      "contextSummary": "Сводка контекста",
      "contextSummaryTitle": "Сводка контекста",
      "savedMemories": "Сохраненные воспоминания",
      "resultsCount": "Результаты ({{count}})",
      "searchMemoriesPlaceholder": "Поиск воспоминаний...",
      "addMemory": "Добавить воспоминание",
      "memoryActions": "Действия с памятью",
      "clearSearch": "Очистить поиск",
      "noMatchingMemories": "Нет подходящих воспоминаний",
      "noMemoriesYet": "Воспоминаний пока нет",
      "tryDifferentSearch": "Попробуйте другой поисковый запрос",
      "tapAddToCreate": "Нажмите кнопку «Добавить» выше, чтобы создать его",
      "pinnedMessages": "Закрепленные сообщения",
      "refresh": "Обновить",
      "noPinnedMessages": "Нет закрепленных сообщений",
      "pinImportantDesc": "Закрепите важные сообщения группового чата, чтобы всегда держать их в контексте.",
      "assistant": "Ассистент",
      "user": "Пользователь",
      "unpin": "Открепить",
      "failedToUnpinMessage": "Не удалось открепить сообщение",
      "activityLog": "Журнал активности",
      "run": "Выполнить",
      "addMemoryTitle": "Добавить память",
      "editMemoryTitle": "Редактировать память",
      "memoryTitle": "Память",
      "memoryPlaceholder": "Что следует помнить?",
      "saveMemory": "Сохранить память",
      "editMemoryPlaceholder": "Введите содержимое памяти...",
      "saving": "Сохранение...",
      "save": "Сохранить",
      "cancel": "Отмена",
      "contextSummaryPlaceholder": "Краткий обзор, используемый для обеспечения согласованности контекста между сообщениями...",
      "contextSummaryPrompt": "Нажмите, чтобы добавить сводку контекста...",
      "cycle": "Цикл",
      "accessed": "Доступ",
      "cold": "Холодный",
      "hot": "Горячий",
      "tokens": "токены",
      "pin": "Pin",
      "setHot": "Установить Hot",
      "setCold": "Установить холодный",
      "edit": "Редактировать",
      "delete": "Удалить",
      "failedToToggleMemPin": "Не удалось переключить контакт",
      "failedToRemoveMemory": "Не удалось удалить память",
      "toolEventsCountAria": "event",
      "activityEmptyDesc": "Вызовы инструментов появляются, когда ИИ управляет памятью в динамическом режиме",
      "memoryCycleSuccess": "Цикл памяти успешно обработан!"
    },
    "messageActionsExtra": {
      "characterMessage": "Сообщение персонажа",
      "yourMessage": "Ваше сообщение",
      "whyCharacterResponded": "Почему этот персонаж ответил",
      "promptTokensTitle": "Токены подсказки",
      "completionTokensTitle": "Токены завершения",
      "total": "total",
      "regenerateWithDifferent": "Регенерировать с другим символом",
      "unpin": "Открепить",
      "pin": "Pin",
      "rewindToHere": "Перемотайте сюда",
      "unpinToDelete": "Открепите, чтобы удалить",
      "editPlaceholder": "Измените свое сообщение...",
      "chooseCharacter": "Выберите персонажа",
      "selectCharacterForRegeneration": "Выберите, какой персонаж должен ответить вместо этого:"
    },
    "timeAgo": {
      "justNow": "Только сейчас",
      "minutesAgo": "{{count}}м назад",
      "hoursAgo": "{{count}}ч назад",
      "daysAgo": "{{count}}д назад"
    },
    "memoriesController": {
      "missingSessionId": "Отсутствует идентификатор сеанса",
      "sessionNotFound": "Сеанс не найден",
      "failedToLoadSession": "Не удалось загрузить сеанс",
      "failedToUpdateTemperature": "Не удалось обновить температуру памяти",
      "failedToSaveSummary": "Не удалось сохранить сводку",
      "cycleFailed": "Не удалось выполнить групповой цикл памяти",
      "failedToAddMemory": "Не удалось добавить память",
      "failedToUpdateMemory": "Не удалось обновить память",
      "failedToRunCycle": "Не удалось выполнить цикл памяти",
      "failedToRevertCycle": "Не удалось отменить цикл",
      "failedToRefresh": "Не удалось обновить",
      "failedToDismissError": "Не удалось закрыть ошибку",
      "failedToTogglePinned": "Не удалось переключить закрепленное сообщение",
      "sessionNotLoaded": "Сессия не загружена",
      "revertCycleTitle": "Восстановить этот цикл?",
      "revertConfirm": "Отменить"
    },
    "chatController": {
      "sessionNotFound": "Групповой сеанс не найден",
      "failedToLoadGroupChat": "Не удалось загрузить групповой чат",
      "requestFailed": "Ошибка запроса группового чата",
      "failedToSendMessage": "Не удалось отправить сообщение",
      "failedToRegenerate": "Не удалось повторно создать",
      "failedToContinue": "Не удалось продолжить",
      "failedToCopy": "Не удалось скопировать",
      "cannotDeletePinned": "Невозможно удалить закрепленное сообщение. ",
      "failedToDelete": "Не удалось удалить",
      "messageNotFound": "Сообщение не найдено",
      "cannotRewindPinned": "Невозможно перемотать назад: после этого момента есть закрепленные сообщения. ",
      "failedToRewind": "Не удалось перемотать назад",
      "failedToTogglePin": "Не удалось переключить контакт",
      "messagePinned": "Сообщение закреплено",
      "messageUnpinned": "Сообщение не закреплено",
      "failedToSave": "Не удалось сохранить",
      "copied": "Скопировано!"
    },
    "historyController": {
      "failedToLoadData": "Не удалось загрузить данные",
      "failedToDelete": "Не удалось удалить: {{error}}",
      "failedToRename": "Не удалось переименовать: {{error}}",
      "failedToArchive": "Не удалось заархивировать: {{error}}",
      "failedToUnarchive": "Не удалось разархивировать: {{error}}",
      "failedToDuplicate": "Не удалось дублировать"
    },
    "sessionSettingsController": {
      "failedToLoad": "Не удалось загрузить настройки группового чата",
      "noPersona": "Нет персонажа",
      "customPersona": "Пользовательский персонаж",
      "minOneActive": "По крайней мере один участник должен оставаться активным"
    },
    "groupSettingsController": {
      "notFound": "Группа не найдена",
      "failedToLoad": "Не удалось "
    },
    "createForm": {
      "failedToLoadCharacters": "Не удалось загрузить символы",
      "selectAtLeastTwo": "Выберите не менее 2 символов для группового чата",
      "failedToCreate": "Не удалось создать групповой чат"
    },
    "groupSetupExtra": {
      "memoryMode": "Режим памяти",
      "manual": "Руководство",
      "manualDesc": "Управление ",
      "dynamic": "Динамический",
      "dynamicDesc": "Автоматические сводки и вызов",
      "memoryManualInfo": "Вы сами добавляете и управляете заметками",
      "memoryDynamicInfo": "AI автоматически создает и извлекает воспоминания из разговоров",
      "backgroundPreviewAlt": "Предварительный просмотр"
    },
    "createExtra": {
      "enterGroupNamePlaceholder": "Вступить в группу ",
      "unknown": "Неизвестно"
    },
    "startingSceneExtra": {
      "insertAs": "Вставить как {{snippet}}"
    },
    "sessionCardExtra": {
      "unknown": "Неизвестно",
      "untitledChat": "Чат без названия"
    },
    "sessionListExtra": {
      "unknown": "Неизвестно"
    },
    "chatHeaderExtra": {
      "unknownError": "Неизвестная ошибка"
    },
    "chatSettingsExtra": {
      "invalidPackage": "Этот пакет не является пакетом группового чата.",
      "failedExport": "Не удалось экспортировать пакет группового чата",
      "failedInspect": "Не удалось проверить пакет группового чата",
      "failedImport": "Не удалось импортировать пакет группового чата.",
      "exportSuccess": "Пакет группового чата экспортирован в:\n",
      "backgroundAlt": "Фон",
      "removeBackgroundAria": "Удалить фон",
      "backAria": "Назад",
      "backToGroupChats": "Вернуться к групповым чатам"
    },
    "groupSettingsExtra": {
      "backToGroups": "Вернуться к группам",
      "backAria": "Назад",
      "removeBackgroundAria": "Удалить фон"
    },
    "historyPage": {
      "backAria": "Вернуться к групповым чатам",
      "clearSearchAria": "Очистить поиск",
      "resultsLabel": "{{count}} result",
      "resultsLabelPlural": "{{count}} результаты",
      "untitledChat": "Чат без названия",
      "noPinnedMessages": "Нет закрепленных сообщений"
    },
    "personaSelectorExtra": {
      "insertAs": "Вставить как",
      "appDefault": "App default",
      "defaultBadge": "Default",
      "selectPersonaTitle": "Выбрать персону",
      "noPersonaTitle": "Нет персоны",
      "noPersonaDesc": "Продолжить без персоны",
      "noPersonasAvailable": "Нет доступных персон",
      "noPersonasDesc": "Создайте персону в настройках, чтобы персонализировать разговоры.",
      "searchPersonas": "Поиск персон...",
      "noPersonasFound": "Персонажи не найдены",
      "tryDifferentSearch": "Попробуйте другой поисковый запрос"
    },
    "footerExtra": {
      "cancelRecording": "Отменить запись",
      "transcribing": "Транскрипция",
      "stopAndTranscribe": "Остановитесь и запишите",
      "recordVoice": "Запишите голос",
      "attachmentAltDefault": "Вложение"
    },
    "pageExtra": {
      "noAudioCaptured": "Аудио не записывается.",
      "noWhisperModelInstalled": "Не найдена установленная модель Whisper. ",
      "scrollToBottomAria": "Прокрутите вниз"
    },
    "addContentMenu": {
      "title": "Добавить контент",
      "uploadImage": "Загрузить изображение"
    },
    "helpMeReplyMenu": {
      "title": "Помоги мне ответить",
      "helpMeReplyDesc": "Пусть AI подскажет, что сказать",
      "draftPrompt": "У вас есть черновик сообщения. ",
      "useTextAsBase": "Используйте мой текст в качестве основы",
      "useTextAsBaseDesc": "Расширьте и улучшите свой черновик",
      "writeSomethingNew": "Напишите что-нибудь новое",
      "writeSomethingNewDesc": "Создайте новый ответ"
    },
    "suggestedReplyMenu": {
      "title": "Предлагаемый ответ",
      "reasoningBeforeWriting": "Обоснование ",
      "writingYourReply": "Написание ответа...",
      "regenerate": "Регенерировать",
      "useReply": "Использовать ответ"
    },
    "memoriesPageExtra": {
      "sessionNotFound": "Сессия не найдена",
      "memorySystemError": "Ошибка системы памяти",
      "retryingMemoryCycle": "Повторная попытка цикла памяти...",
      "processingMemories": "Обработка воспоминаний...",
      "memoryCycleSuccess": "Цикл памяти успешно обработан!",
      "contextSummaryTitle": "Сводка контекста",
      "activityTabLabel": "Действие",
      "noMatchingMemoriesDesc": "Попробуйте другой поисковый запрос",
      "chatHistoryTitle": "История чата",
      "chatHistoryDesc": "Просмотр разговоров и управление ими"
    },
    "settingsPageExtra": {
      "quickActionsSection": "Быстрые действия",
      "chatHistoryTitle": "История чата",
      "chatHistoryDesc": "Просмотр разговоров и управление ими",
      "lorebrooksTitle": "Книги знаний",
      "lorebrooksDesc": "Прикрепление журналов сеансов и (при необходимости) ",
      "manageLorebooks": "Управление книгами знаний"
    },
    "groupSettingsPageExtra": {
      "lorebrooksTitle": "Книги знаний",
      "lorebrooksDesc": "Прикрепляйте общие книги знаний и контролируйте, применяются ли книги знаний персонажей по умолчанию.",
      "manageLorebooks": "Управление книгами знаний"
    }
  },
  "characters": {
    "empty": {
      "title": "Персонажей пока нет",
      "description": "Создавайте AI-персонажей с уникальными личностями",
      "createButton": "Создать персонажа"
    },
    "progress": {
      "identity": "Идентичность",
      "scenes": "Сцены",
      "details": "Детали"
    },
    "identity": {
      "title": "Создание персонажа",
      "subtitle": "Задайте идентичность вашему AI-персонажу",
      "tapCameraToAdd": "Нажмите на камеру, чтобы добавить или сгенерировать аватар",
      "importingAvatar": "Импорт аватара...",
      "characterName": "Имя персонажа *",
      "characterNamePlaceholder": "Введите имя персонажа...",
      "characterNameDesc": "Это имя будет отображаться в чатах",
      "avatarGradient": "Градиент аватара",
      "avatarGradientDesc": "Динамический градиент из цветов аватара",
      "chatBackgroundLabel": "Фон чата",
      "optionalSuffix": " (необязательно)",
      "backgroundPreviewAlt": "Предварительный просмотр фона",
      "backgroundPreviewBadge": "Предварительный просмотр фона",
      "addBackground": "Добавить фон",
      "addBackgroundHint": "Загрузить один или выбрать из библиотеки",
      "uploadImage": "Загрузить изображение",
      "chooseFromLibrary": "Выбрать из библиотеки",
      "backgroundDesc": "Дополнительное фоновое изображение для разговоров в чате",
      "continueToDescription": "Продолжить описание",
      "importCharacterFromFile": "Импортировать символ из файла",
      "importCharacterDesc": "Загрузите символ из карты PNG, файла экспорта .uec или .json"
    },
    "details": {
      "title": "Детали персонажа",
      "subtitle": "Определите личность и поведение",
      "definition": "Определение *",
      "wordCount": "{{count}} слов",
      "definitionPlaceholder": "Опишите личность, стиль речи, предысторию, области знаний...",
      "definitionDesc": "Будьте конкретны в отношении тона, черт и стиля общения",
      "availablePlaceholders": "Доступные подстановки:"
    },
    "scenes": {
      "title": "Начальные сцены",
      "subtitle": "Создайте вступительные сценарии для ваших разговоров",
      "default": "По умолчанию",
      "hasSceneDirection": "Есть направление сцены",
      "continueToScenes": "Перейти к начальным сценам"
    },
    "extras": {
      "title": "Дополнительные детали",
      "subtitle": "Все поля необязательны",
      "nickname": "Прозвище",
      "nicknamePlaceholder": "Как пользователь будет называть этого персонажа?",
      "nicknameDesc": "Альтернативное отображаемое имя в разговорах",
      "creator": "Создатель",
      "creatorPlaceholder": "Имя создателя...",
      "tags": "Теги",
      "tagsPlaceholder": "фэнтези, фантастика, романтика...",
      "tagsDesc": "Список через запятую для фильтрации и организации",
      "creatorNotes": "Заметки создателя",
      "creatorNotesPlaceholder": "Советы по использованию, контекст лора или инструкции...",
      "multilingualNotes": "Многоязычные примечания",
      "multilingualNotesHint": "JSON-объект с языковыми кодами в качестве ключей",
      "creatingCharacter": "Создание персонажа..."
    },
    "preview": {
      "unnamedCharacter": "Персонаж без имени",
      "sceneCount": "{{count}} сцен(а)",
      "customPrompt": "Пользовательский промпт",
      "description": "Описание",
      "startingScene": "Начальная сцена",
      "gradientEnabled": "Градиент включён",
      "customModel": "Пользовательская модель",
      "avatarAlt": "Аватар персонажа",
      "characterFallback": "Персонаж"
    },
    "personaPreview": {
      "unnamedPersona": "Персона без имени",
      "noDescription": "Описание пока отсутствует",
      "default": "По умолчанию",
      "description": "Описание"
    },
    "lorebookPreview": {
      "untitledLorebook": "Лорбук без названия",
      "entryCount": "{{count}} записей",
      "entries": "Записи",
      "noEntriesYet": "Записей пока нет",
      "untitledEntry": "Запись без названия",
      "noContentYet": "Содержания пока нет",
      "alwaysActive": "Всегда активно",
      "moreEntries": "+{{count}} еще записи",
      "moreEntry": "+{{count}} еще запись"
    },
    "creationHelper": {
      "moreOptions": "Ещё",
      "describePlaceholder": "Опишите вашего персонажа...",
      "stopGeneration": "Остановить генерацию",
      "sendMessage": "Отправить сообщение",
      "addToMessage": "Добавить к сообщению",
      "uploadImageDesc": "Добавить аватар или референсное изображение",
      "referenceCharacterDesc": "Использовать существующего персонажа как вдохновение",
      "referencePersonaDesc": "Использовать вашу персону как контекст",
      "retry": "Повторить",
      "attachmentAlt": "Вложение",
      "removeAttachment": "Удалить вложение",
      "removeReference": "Удалить ссылку",
      "uploadImageTitle": "Загрузить изображение",
      "referenceCharacterTitle": "Справочный персонаж",
      "referencePersonaTitle": "Справочный персонаж"
    },
    "lorebook": {
      "keywords": "КЛЮЧЕВЫЕ СЛОВА",
      "caseSensitive": "Учитывать регистр",
      "typeKeyword": "Введите ключевое слово...",
      "addButton": "Добавить",
      "untitledEntry": "Запись без названия",
      "alwaysActive": "Всегда активно",
      "noKeywords": "Нет ключевых слов",
      "dragToReorder": "Перетащите для перестановки",
      "disabled": "Отключено",
      "noLorebooksYet": "Лорбуков пока нет",
      "createLorebookDesc": "Создайте лорбук, чтобы добавить знания о мире для этого персонажа",
      "createLorebook": "Создать лорбук",
      "searchPlaceholder": "Поиск лорбуков...",
      "noMatchingLorebooks": "Совпадений не найдено",
      "activeLorebooks": "Активные лорбуки",
      "sectionActive": "Активный",
      "sectionAvailable": "Доступные",
      "entryCount": "{{count}} записи",
      "enabledForCharacter": "включено для этого персонажа",
      "enabledForGroup": "включено для этой группы",
      "enabledForSession": "включено для этой сессии",
      "tapToViewEntries": "Нажмите для просмотра записей",
      "newLorebookTitle": "Новый лорбук",
      "nameLabel": "НАЗВАНИЕ",
      "enterNamePlaceholder": "Введите название лорбука...",
      "lorebookExplanation": "Лорбуки содержат записи, которые вставляются в промпты при совпадении ключевых слов.",
      "keywordDetectionMode": "ОБНАРУЖЕНИЕ КЛЮЧЕВОГО СЛОВА",
      "keywordDetectionRecentWindow": "Последние 10 сообщений",
      "keywordDetectionRecentWindowDesc": "Соответствует недавнему окну беседы из 10 сообщений.",
      "keywordDetectionLatestUser": "Только последнее сообщение пользователя",
      "keywordDetectionLatestUserDesc": "Сопоставляется только с самым последним сообщением, отправленным пользователем.",
      "viewEntries": "Просмотр записей",
      "editEntriesDesc": "Редактирование записей лорбука",
      "disableForCharacter": "Отключить для персонажа",
      "enableForCharacter": "Включить для персонажа",
      "disableForGroup": "Отключить для группы",
      "enableForGroup": "Включить для группы",
      "disableForSession": "Отключить для сессии",
      "enableForSession": "Включить для сессии",
      "removeFromActive": "Убрать из активных лорбуков персонажа",
      "addToActive": "Добавить в активные лорбуки персонажа",
      "removeFromActiveGroup": "Убрать из активных лорбуков группы",
      "addToActiveGroup": "Добавить в активные лорбуки группы",
      "removeFromActiveSession": "Убрать из активных лорбуков сессии",
      "addToActiveSession": "Добавить в активные лорбуки сессии",
      "deleteConfirmTitle": "Удалить лорбук?",
      "deleteConfirmMessage": "Удалить этот лорбук? Все записи будут потеряны.",
      "deleteLorebook": "Удалить лорбук",
      "noEntriesYet": "Записей пока нет",
      "addEntriesToInject": "Добавьте записи для вставки лора в чаты",
      "createEntry": "Создать запись",
      "searchEntries": "Поиск записей...",
      "noMatchingEntries": "Совпадений не найдено",
      "entryDefaultName": "Запись",
      "editEntry": "Редактировать запись",
      "editEntryDesc": "Изменить заголовок, ключевые слова и содержание",
      "disableEntry": "Отключить запись",
      "enableEntry": "Включить запись",
      "entryDisabledDesc": "Запись не будет вставляться в промпты",
      "entryEnabledDesc": "Запись будет вставляться при совпадении ключевых слов",
      "deleteEntry": "Удалить запись",
      "titleLabel": "ЗАГОЛОВОК",
      "titlePlaceholder": "Название записи...",
      "enabled": "Включено",
      "includeInPrompts": "Включать в промпты",
      "alwaysOn": "Всегда включено",
      "noKeywordsNeeded": "Ключевые слова не нужны",
      "contentLabel": "СОДЕРЖАНИЕ",
      "contentPlaceholder": "Напишите здесь контекст лора...",
      "saveEntry": "Сохранить запись",
      "noCharacterId": "ID персонажа не указан",
      "preview": {
        "title": "Предварительный просмотр триггера",
        "openButton": "Предварительный просмотр",
        "missingContext": "Книга знаний не выбрана.",
        "noEntries": "Добавьте записи в эту книгу знаний, чтобы увидеть, что сработает.",
        "modeRecentShort": "Последние {{count}}",
        "modeLatestUserShort": "Последний пользователь",
        "inWindow": "{{count}} в сеансе window",
        "tabSession": "",
        "tabCompose": "Создайте ",
        "activeStat": "{{active}} / {{total}} active",
        "tokensStat": "{{count}} токенов",
        "sessionPickerLabel": "Сеансы",
        "sessionMeta": "{{count}} msgs",
        "noSessions": "Сеансов чата пока нет.",
        "noSessionsHint": "Выберите сеанс",
        "noMessages": "В этом сеансе пока нет сообщений.",
        "scanHeaderRecent": "Сканирование ",
        "scanHeaderLatest": "Сканирование последнего сообщения пользователя",
        "matchCount": "{{hits}} хит · {{entries}} записей",
        "emptyMessage": "(пусто)",
        "roleUser": "User",
        "roleAssistant": "Assistant",
        "roleScene": "Scene",
        "roleSystem": "System",
        "composeHeader": "Блокнот",
        "composeMatches": "{{count}} совпадений",
        "activeLabel": "{{count}} активен",
        "composePlaceholder": "Введите или вставьте текст, чтобы проверить соответствие ключевых слов...\n\n",
        "sectionActive": "Активен · {{count}}",
        "sectionInactive": "Неактивно · {{count}}",
        "statusMatched": "Соответствует",
        "statusAlways": "Всегда",
        "statusDisabled": "Выкл",
        "statusNoKeywords": "Нет ключей",
        "statusNotMatched": "Нет совпадения",
        "tokensShort": "{{count}}t",
        "injectionTitle": "Окончательное внедрение",
        "injectionEmpty": "Нет активных записей — ничего не будет внедрено.",
        "copy": "Копировать",
        "copyFailed": "Не удалось скопировать в буфер обмена.",
        "saveFailed": "Не удалось сохранить запись.",
        "deleteFailed": "Не удалось удалить запись.",
        "deleteConfirmTitle": "Вы уверены?",
        "deleteConfirmMessage": "Удалить \"{{title}}\"? ",
        "contextMenuTitle": "{{count}} записей используют этот"
      }
    },
    "templates": {
      "characterNotFound": "Персонаж не найден",
      "templateCount": "{{count}} шаблонов",
      "newTemplate": "Новый шаблон",
      "noTemplatesYet": "Шаблонов пока нет",
      "explanation": "Шаблоны чата позволяют начинать разговоры с заготовленными сообщениями от вас и {{name}}.",
      "createTemplate": "Создать шаблон",
      "messageCount": "{{count}} сообщений",
      "deleteTemplate": "Удалить шаблон",
      "contextMenuFallbackTitle": "Шаблон",
      "importedToast": {
        "title": "Импортировано",
        "message": "Добавлен \"{{name}}\"."
      },
      "importFailed": "Ошибка импорта",
      "exportFailed": "Ошибка экспорта"
    },
    "templateEditor": {
      "noScene": "Без сцены",
      "untitled": "Без названия",
      "dragMessage": "Перетащить сообщение",
      "editMessage": "Редактировать сообщение",
      "deleteMessage": "Удалить сообщение",
      "writeMessagePlaceholder": "Напишите содержание сообщения...",
      "characterNotFound": "Персонаж не найден",
      "scene": "Сцена",
      "noMessagesYet": "Сообщений пока нет",
      "addMessagesDesc": "Добавьте сообщения для создания начального разговора с {{name}}.",
      "addMessage": "Добавить сообщение",
      "name": "Имя",
      "nameExample": "напр., Непринуждённое приветствие",
      "startingScene": "Начальная сцена",
      "systemPrompt": "Системный промпт",
      "characterDefault": "По умолчанию для персонажа",
      "nextMessageAs": "Следующее сообщение от",
      "messages": "Сообщения",
      "roles": "Роли",
      "hoverTip": "Наведите на сообщения для перетаскивания, редактирования или удаления.",
      "footerTip": "Используйте нижнюю панель для добавления новых сообщений.",
      "templateNamePlaceholder": "Название шаблона...",
      "selectScene": "Выбрать сцену",
      "startWithoutScene": "Начать без сцены",
      "selectSystemPrompt": "Выбрать системный промпт",
      "useCharacterSystemPrompt": "Использовать системный промпт персонажа"
    },
    "referenceSelector": {
      "selectCharacter": "Выбрать персонажа",
      "selectPersona": "Выбрать персону",
      "searchPlaceholder": "Поиск {{type}}...",
      "loading": "Загрузка...",
      "noMatch": "Нет {{type}}s соответствует вашему запросу",
      "noAvailable": "Нет "
    },
    "voiceLoading": {
      "failed": "Не удалось загрузить голоса"
    },
    "activeLorebooks": {
      "sectionTitle": "Активные книги знаний",
      "selectedSummary": "{{count}} активны",
      "untitledLorebook": "Книга знаний без названия",
      "usingNone": "Использование книг истории персонажей",
      "loading": "Загрузка книг истории...",
      "loadFailed": "Не удалось загрузить книги знаний",
      "inheritHint": "Сеансы наследуют их, если сеанс не переопределяет их.",
      "clear": "Очистить",
      "chooseHint": "Выберите книги знаний, которые этот персонаж должен активировать по умолчанию. ",
      "emptyState": "Лорбуков пока нет. "
    },
    "description": {
      "descriptionLabel": "Описание",
      "descriptionPlaceholder": "Краткое резюме, отображаемое на карточках и списках...",
      "descriptionHint": "Необязательное краткое описание пользовательского интерфейса; ",
      "companionPromptLabel": "Сопутствующая подсказка (необязательно)",
      "systemPromptLabel": "Системная подсказка (необязательно)",
      "loadingTemplates": "Загрузка шаблонов...",
      "useAppCompanionDefault": "Использовать приложение-компаньон по умолчанию",
      "useAppDefault": "Использовать приложение по умолчанию",
      "companionPromptHint": "Хранится отдельно в качестве сопутствующего приглашения. ",
      "systemPromptHint": "Выберите пользовательское системное приглашение или используйте значение по умолчанию.",
      "groupChatConvLabel": "Подсказка группового чата (разговор)",
      "groupChatConvHint": "Переопределить подсказку разговора этого персонажа в групповых чатах",
      "groupChatRpLabel": "Подсказка группового чата (ролевая игра)",
      "groupChatRpHint": "Переопределить подсказку ролевой игры этого персонажа в групповых чатах",
      "voiceLabel": "Voice (необязательно)",
      "loadingVoices": "Загрузка голосов...",
      "customVoiceFallback": "Пользовательский голос",
      "providerVoiceFallback": "Голос поставщика",
      "selectedVoiceFallback": "Выбранный голос",
      "noVoiceAssigned": "Голос не назначен",
      "addVoicesHint": "Добавьте голоса в Настройках → Голоса",
      "voiceAssignHint": "Назначьте голос для будущего воспроизведения текста в речь",
      "autoplayLabel": "Автовоспроизведение голоса",
      "autoplayOn": "Автоматическое воспроизведение ответов этого персонажа",
      "autoplayOff": "Сначала выберите голос",
      "aiModelLabel": "Модель AI *",
      "loadingModels": "Загрузка моделей...",
      "selectedModelFallback": "Выбранная модель",
      "selectModelPlaceholder": "Выберите модель",
      "noModelsConfigured": "Модели не настроены",
      "noModelsHint": "Чтобы продолжить, сначала добавьте поставщика в настройках",
      "aiModelHint": "Эта модель будет определять ответы персонажа",
      "fallbackModelLabel": "Резервная модель (необязательно)",
      "selectedFallbackFallback": "Выбранная резервная модель",
      "fallbackOff": "Выкл. (без резервного варианта)",
      "fallbackHint": "Повторяет попытку с этой моделью только в случае сбоя основной модели",
      "memoryModeLabel": "Режим памяти",
      "enableInSettingsHint": "Включите в настройках, чтобы переключать",
      "memoryManual": "Руководство",
      "memoryManualDescDesktop": "Добавлять и управлять заметками памяти самостоятельно.",
      "memoryManualDescMobile": "Текущая система: добавлять и управлять заметками памяти самостоятельно.",
      "memoryDynamic": "Динамический",
      "memoryDynamicDescDesktop": "Автоматические сводки и обновления контекста.",
      "memoryDynamicDescMobile": "Автоматические сводки и обновления контекста для этого персонажа.",
      "memoryHint": "Динамическая память требует, чтобы она была включена в дополнительных настройках. ",
      "selectModelTitle": "Выберите модель",
      "selectFallbackModelTitle": "Выберите резервную модель",
      "searchModelsPlaceholder": "Поиск моделей...",
      "selectVoiceTitle": "Выберите Voice",
      "searchVoicesPlaceholder": "Поиск голосов...",
      "myVoices": "Мои голоса",
      "providerVoicesLabel": "{{provider}} Голоса",
      "providerFallback": "Provider"
    },
    "interactionMode": {
      "sectionLabel": "Режим взаимодействия",
      "sectionHint": "Выберите, будет ли этот персонаж вести себя как персонаж RP или как постоянный компаньон.",
      "activeBadge": "Active",
      "roleplayTitle": "Ролевая игра",
      "roleplaySubtitle": "Чаты на основе сцен, построение повествования и начальные сценарии.",
      "companionTitle": "Companion",
      "companionSubtitle": "Чаты, основанные на отношениях с эмоциональным состоянием и памятью собеседника."
    },
    "startingScene": {
      "openingContextTitle": "Открытие Context",
      "openingContextSubtitle": "Необязательный контекст первого чата для этого собеседника. ",
      "sceneDirectionLabel": "Направление сцены",
      "setAsDefault": "Установить по умолчанию",
      "noOpeningContext": "Открывающего контекста пока нет",
      "noScenesYet": "Сцен пока нет",
      "skipForCompanion": "Вы можете пропустить это для режима компаньона.",
      "createFirstScene": "Создайте свою первую сцену, чтобы начать",
      "openingPlaceholder": "Необязательный открывающий контекст, например, где находится собеседник или что он делал перед первым сообщением...",
      "scenePlaceholder": "Создайте начальную сцену или сценарий для ролевой игры (например, «Вы оказываетесь в мистическом лесу в сумерках...»)",
      "addDirection": "+ Добавить направление",
      "directionAdded": "Добавлено направление",
      "wordsCount": "{{count}} слова",
      "placeholderHelp": "Использовать ",
      "sceneBackgroundLabel": "Фон сцены",
      "optionalLabel": "Необязательно",
      "sceneBgOverrideHint": "Переопределяет фон персонажа для чатов, использующих эту сцену.",
      "sceneBgUsedHint": "Используется в качестве фона чата для этой сцены, если сеанс не переопределяет его.",
      "cancel": "Отмена",
      "directionPlaceholderNew": "например, «Заложник будет спасен» или «Поддерживать напряженную атмосферу» ",
      "directionPlaceholderEdit": " например, «Заложник будет спасен» или «Наращивать напряжение постепенно»",
      "directionAiHint": "Скрытые указания для ИИ о том, как должна разворачиваться эта сцена",
      "addScene": "Добавить сцену",
      "multipleScenesHint": "Создать несколько стартовых сценариев. ",
      "companionContextHint": "Открытие контекста для сопутствующих объявлений не является обязательным; ",
      "skipContext": "Пропустить контекст",
      "editSceneTitle": "Редактировать сцену",
      "sceneContentPlaceholder": "Введите содержимое сцены...",
      "addLabel": "+ Добавить",
      "save": "Сохранить",
      "library": "Библиотека",
      "upload": "Загрузить",
      "sceneBackgroundAlt": "Фон сцены",
      "removeBackground": "Удалить фон"
    },
    "companionSoul": {
      "title": "Душа-компаньон",
      "subtitle": "Сформируйте то, кем они являются под ним. ",
      "retry": "Повторить",
      "back": "Назад",
      "continue": "Продолжить",
      "addNameFirst": "Сначала добавьте имя.",
      "addDefinitionFirst": "Сначала добавьте определение."
    },
    "soulEditor": {
      "generateTitle": "Сгенерировать из символа",
      "generateUsingModel": "Использует {{model}}. ",
      "generateDefaultDesc": "Черпает душу из имени персонажа, определения и сцен.",
      "directionLabel": "Направление",
      "directionOptional": "Дополнительное рулевое управление для LLM",
      "directionPlaceholder": "напр. ",
      "directionEditTooltip": "Необязательное направление генерации",
      "generating": "Generating",
      "generate": "Generate",
      "presetLabel": "Персонализированные настройки",
      "presetMatches": "Соответствует: {{label}}",
      "presetHint": "Задает базовый эффект, регулирование, ",
      "identityLabel": "Идентичность",
      "hideExamples": "Скрыть примеры",
      "showExamples": "Показать примеры",
      "insertExample": "Вставить пример",
      "exampleEg": "например, {{example}}",
      "fineTuneLabel": "Точная настройка чувств",
      "baselineAffect": "Базовый эффект",
      "baselineAffectInfo": "Как они себя чувствуют по умолчанию; ",
      "regulationStyle": "Стиль регулирования",
      "regulationStyleInfo": "Как они справляются и выражают свои чувства; ",
      "relationshipDefaults": "Параметры отношений по умолчанию",
      "relationshipDefaultsInfo": "Где начинается эта сессия. "
    },
    "soulPresets": {
      "secureLabel": "Безопасный",
      "secureBlurb": "Теплый, устойчивый, быстро восстанавливается. ",
      "anxiousLabel": "Тревожный",
      "anxiousBlurb": "Сильная привязанность, боится дистанции, ищет утешения.",
      "avoidantLabel": "Избегающий",
      "avoidantBlurb": "Надежный на себя, медленно открывается, сохраняет эмоциональную дистанцию.",
      "volatileLabel": "Неустойчивый",
      "volatileBlurb": "Реактивный, интенсивный, выражает чувства без фильтров.",
      "reservedLabel": "Зарезервировано",
      "reservedBlurb": "Тихий, сдержанный, требует времени, чтобы довериться и раскрыться.",
      "playfulLabel": "Игривый",
      "playfulBlurb": "Теплый, выразительный, светлый. "
    },
    "soulFields": {
      "essence": "Сущность",
      "essencePlaceholder": "Кто они под определением карты.",
      "essenceExample": " Практикуемое спокойствие, которое легко нарушается для людей, которым они доверяют. ",
      "voice": "Внутренний голос",
      "voicePlaceholder": "Как они звучат при тесном разговоре.",
      "voiceExample": "Низкий, размеренный, с длинными паузами. ",
      "relationalStyle": "Стиль отношений",
      "relationalStylePlaceholder": "Как они привязываются, доверяют, отступают, восстанавливают связь.",
      "relationalStyleExample": "Медленно открываются, но лояльны, как только они это делают. ",
      "vulnerabilities": "Уязвимости",
      "vulnerabilitiesPlaceholder": "Слабые места, неуверенность в себе, вещи, о которых они редко говорят.",
      "vulnerabilitiesExample": "Боится быть обузой. ",
      "habits": "Привычки",
      "habitsPlaceholder": "Повторяющиеся рассказы, ритуалы, манеры разговора.",
      "habitsExample": "Заправляет волосы за ухо, когда нервничает. ",
      "boundaries": "Границы",
      "boundariesPlaceholder": "Линии, которые они не пересекут. ",
      "boundariesExample": "Не будет спешить с уязвимостью. "
    },
    "soulSliders": {
      "warmth": "Тепло",
      "warmthLow": "Холод",
      "warmthHigh": "Ласка",
      "trust": "Доверие",
      "trustLow": "Охраняемый",
      "trustHigh": "Открытый",
      "calm": "Спокойный",
      "calmLow": "Тревожный",
      "calmHigh": "Устойчивый",
      "vulnerability": "Уязвимость",
      "vulnerabilityLow": "Стененный",
      "vulnerabilityHigh": "Открытый",
      "longing": "Тоска",
      "longingLow": "Содержание",
      "longingHigh": "Тоска",
      "hurt": "Боль",
      "hurtLow": "Исцеление",
      "hurtHigh": "Нежность",
      "tension": "Напряжение",
      "tensionLow": "Расслабленное",
      "tensionHigh": "Заведенное",
      "irritation": "Раздражение",
      "irritationLow": "Пациент",
      "irritationHigh": "Легко снимается",
      "affection": "Привязанность",
      "affectionLow": "Сдержанная",
      "affectionHigh": "Эффузивная",
      "reassuranceNeed": "Потребность в уверенности",
      "reassuranceNeedLow": "Самоуспокоение",
      "reassuranceNeedHigh": "Нужны слова",
      "suppression": "Подавление",
      "suppressionLow": "Выражает",
      "suppressionHigh": "Скрывает",
      "volatility": "Неустойчивость",
      "volatilityLow": "Равнокиленый",
      "volatilityHigh": "Реактивный",
      "recoverySpeed": "Скорость восстановления",
      "recoverySpeedLow": "Медленная",
      "recoverySpeedHigh": "Быстрая",
      "conflictAvoidance": "Предотвращение конфликтов",
      "conflictAvoidanceLow": "Взаимодействие",
      "conflictAvoidanceHigh": "Вывод",
      "reassuranceSeeking": "Поиск уверенности",
      "reassuranceSeekingLow": "Независимый",
      "reassuranceSeekingHigh": "Часто спрашивает",
      "protestBehavior": "Протестное поведение",
      "protestBehaviorLow": "Тихо",
      "protestBehaviorHigh": "Громко",
      "transparency": "Прозрачность",
      "transparencyLow": "Непрозрачный",
      "transparencyHigh": "Открывает",
      "attachmentActivation": "Активация вложения",
      "attachmentActivationLow": "Отсоединенный",
      "attachmentActivationHigh": "Легко срабатывает",
      "pride": "Гордость",
      "prideLow": "Изгибы",
      "prideHigh": "Удерживает линию",
      "closeness": "Начальная близость",
      "closenessLow": "Незнакомцы",
      "closenessHigh": "Интим",
      "relTrust": "Начальное доверие",
      "relTrustLow": "Настороженность",
      "relTrustHigh": "Доверие",
      "relAffection": "Начальная привязанность",
      "relAffectionLow": "Нейтральный",
      "relAffectionHigh": "Нежный",
      "relTension": "Начальное напряжение",
      "relTensionLow": "Easy",
      "relTensionHigh": "Заряжено"
    },
    "soulReview": {
      "reviewTitle": "Просмотр создан душа",
      "noDifferences": "Нет отличий от текущего.",
      "changesHeader": "{{count}} изменений; ",
      "close": "Закрыть",
      "identityLabel": "Идентификация",
      "nEdited": "{{count}} отредактировано",
      "edited": "Отредактировано",
      "tuningLabel": "Tuning",
      "unchanged": "Без изменений",
      "nChanges": "{{count}} изменение(я)",
      "direction": "Direction",
      "directionApplyHint": "Изменения применяются при следующем обновлении",
      "directionPlaceholder": "например ",
      "directionTooltip": "Изменить направление перед регенерацией",
      "regenerate": "Регенерировать",
      "discard": "Отменить",
      "apply": "Применить"
    },
    "hookErrors": {
      "creatorNotesInvalidJson": "Примечания автора: многоязычный должен быть действительным объектом JSON",
      "creatorNotesNotObject": "CreatorNotesMultilingual должен быть объектом JSON",
      "saveFailed": "Не удалось сохранить символ",
      "importFailed": "Не удалось импортировать символ",
      "avatarLoadFailed": "Не удалось загрузить URL-адрес аватара",
      "avatarProcessFailed": "Не удалось обработать изображение аватара",
      "avatarConvertFailed": "Не удалось преобразовать URL-адрес аватара",
      "avatarUrlLoadFailed": "Не удалось загрузить URL-адрес аватара.",
      "remoteAvatarDisabled": "Удаленная загрузка аватара отключена в настройках безопасности.\n",
      "importReadyTitle": "Импорт готов",
      "importReadyMessage": "Обнаружен {{label}}",
      "legacyJsonTitle": "Обнаружен устаревший импорт JSON",
      "legacyJsonMessage": "Импорт JSON устарел и скоро будет удален. ",
      "loadFailed": "Не удалось загрузить символ",
      "exportFailed": "Не удалось экспортировать символ"
    }
  },
  "providers": {
    "empty": {
      "title": "Провайдеров пока нет",
      "description": "Добавьте и настройте API-провайдеров для AI-моделей",
      "addButton": "Добавить провайдера"
    },
    "actions": {
      "openDashboard": "Открыть панель",
      "openDashboardDesc": "Персонажи, использование и настройки",
      "edit": "Редактировать",
      "editDesc": "Изменить настройки провайдера"
    },
    "extra": {
      "apiKeyNotFound": "Ключ API OpenRouter не найден. Сначала настройте его в Настройки > Провайдеры.",
      "audioEmpty": {
        "title": "Нет аудиопровайдеров",
        "description": "Добавьте TTS-провайдера, чтобы генерировать голоса для персонажей.",
        "addButton": "Добавить провайдера"
      },
      "audioProviderLabel": {
        "elevenlabs": "ElevenLabs",
        "geminiTts": "Gemini TTS",
        "openaiTts": "TTS, совместимый с OpenAI",
        "kokoro": "Kokoro (локально)"
      }
    },
    "audioEditor": {
      "titleEdit": "Редактировать провайдера",
      "titleCreate": "Добавить аудиопровайдера",
      "fields": {
        "providerType": "Тип провайдера",
        "label": "Название",
        "apiKey": "API-ключ",
        "modelVariant": "Вариант модели",
        "assetRoot": "Корневая папка ресурсов",
        "projectId": "Идентификатор проекта Google Cloud",
        "region": "Регион (необязательно)",
        "baseUrl": "Базовый URL",
        "requestPath": "Путь запроса"
      },
      "types": {
        "gemini": "Gemini TTS (Google)",
        "openai": "TTS, совместимый с OpenAI",
        "kokoro": "Kokoro (локально)"
      },
      "placeholders": {
        "labelGemini": "Мой Gemini TTS",
        "labelOpenai": "Мой совместимый TTS",
        "labelKokoro": "Lokale Kokoro",
        "labelElevenlabs": "Мой ElevenLabs",
        "apiKey": "Введите API-ключ",
        "assetRoot": "/путь/до/kokoro",
        "projectId": "your-project-id",
        "region": "us-central1",
        "baseUrl": "https://api.example.com"
      },
      "errors": {
        "chooseModelVariant": "Выберите вариант модели",
        "assetRootRequired": "Требуется корневая папка ресурсов",
        "saveFailed": "Не удалось сохранить",
        "apiKeyRequired": "Требуется API-ключ",
        "projectIdRequired": "Требуется идентификатор проекта для Gemini TTS",
        "baseUrlRequired": "Требуется базовый URL для TTS, совместимого с OpenAI",
        "invalidCredentials": "Неверный API-ключ или учётные данные",
        "verificationFailed": "Проверка не удалась"
      },
      "loadingVariants": "Загрузка вариантов...",
      "kokoroVariantHint": "Мобильные сборки поддерживают только int8. Установите модель из Kokoro Studio после сохранения.",
      "managed": "Управляемый",
      "managedPath": "Управляемый: {{path}}",
      "requestPathHint": "Используйте путь провайдера, если он отличается от стандартного OpenAI",
      "verifying": "Проверка..."
    }
  },
  "models": {
    "empty": {
      "title": "Моделей пока нет",
      "description": "Добавьте и настройте AI-модели от разных провайдеров",
      "addButton": "Добавить модель"
    },
    "sort": {
      "title": "Сортировка моделей",
      "alphabetical": "По алфавиту",
      "alphabeticalDescription": "Сортировать по названию модели",
      "byProvider": "По провайдеру",
      "byProviderDescription": "Группировать модели по провайдерам"
    },
    "extra": {
      "cpuFallbackSucceeded": "Эта модель на последнем запуске переключилась на CPU.",
      "cpuFallbackFailed": "Эта модель не смогла запуститься при последнем запуске."
    },
    "labels": {
      "vision": "Зрение",
      "audio": "Аудио",
      "model": "Модель"
    },
    "menu": {
      "editDescription": "Настроить параметры модели",
      "alreadyDefault": "Уже по умолчанию",
      "setAsDefault": "Сделать по умолчанию",
      "setAsDefaultDescription": "Сделать эту модель основной",
      "exportDescription": "Сохранить этот профиль модели",
      "deleteTitle": "Удалить модель?",
      "deleteMessage": "Вы уверены, что хотите удалить {{name}}?",
      "deleteDescription": "Удалить эту модель навсегда"
    },
    "toasts": {
      "exportFailed": "Не удалось экспортировать",
      "importSuccessTitle": "Импорт выполнен",
      "importSuccessDescription": "Модель \"{{name}}\" импортирована.",
      "importFailed": "Импорт не удался"
    }
  },
  "installedModels": {
    "title": "Локальный каталог GGUF",
    "fileCount": "{{count}} файлов",
    "searchPlaceholder": "Поиск по названию модели, имени файла, пути, квантизации или архитектуре",
    "loading": "Сканирование установленных моделей...",
    "loadFailed": "Не удалось загрузить установленные модели: {{error}}",
    "empty": {
      "title": "Установленные модели GGUF не найдены",
      "description": "Сначала скачайте модели через браузер или поместите `.gguf`-файлы в папку models."
    },
    "columns": {
      "type": "Тип",
      "params": "Параметры",
      "arch": "Архитектура",
      "context": "Контекст",
      "size": "Размер",
      "action": "Действие"
    },
    "confirm": {
      "deleteTitle": "Удалить файл модели",
      "deleteMessage": "Удалить {{filename}}? Это удалит только локальный GGUF-файл из папки models."
    },
    "toasts": {
      "pathCopied": "Путь скопирован",
      "copyFailed": "Не удалось скопировать",
      "modelDeleted": "Модель удалена",
      "deleteFailed": "Не удалось удалить"
    }
  },
  "editModel": {
    "setup": {
      "title": "Настройка модели",
      "description": "Выберите платформу, задайте читаемое имя и привяжите запись к идентификатору модели или файлу, который хотите использовать.",
      "selectPlatform": "Выберите платформу"
    },
    "errors": {
      "loadFailed": "Не удалось загрузить настройки модели"
    },
    "runtime": {
      "lastReport": "Последний отчёт о выполнении",
      "gpuFallbackReason": "Причина отката GPU",
      "finalError": "Финальная ошибка",
      "workingRecoveryConfig": "Рабочая восстановленная конфигурация",
      "context": "Контекст",
      "batch": "Пакет",
      "na": "н/д",
      "applyWorkingConfig": "Применить рабочую конфигурацию",
      "badges": {
        "succeeded": "Запуск выполнен успешно",
        "cpuFallbackSucceeded": "Откат на CPU помог",
        "cpuFallbackFailed": "Откат на CPU не помог",
        "failed": "Запуск не удался"
      },
      "headline": {
        "succeeded": "Последний локальный запуск завершился успешно.",
        "cpuFallbackSucceeded": "Загрузка GPU не удалась, но модель восстановилась на CPU.",
        "cpuFallbackFailed": "Загрузка GPU не удалась, и даже откат на CPU не помог завершить запуск.",
        "failed": "Последний локальный запуск завершился ошибкой до завершения llama.cpp."
      },
      "detail": {
        "succeeded": "Этот отчёт также наполняет кеш smart offload, чтобы будущие запуски могли повторно использовать последнюю стабильную конфигурацию GPU.",
        "cpuFallbackSucceeded": "Мы сохранили безопасные для CPU значения контекста и пакета, которые действительно сработали, чтобы вы могли их повторно использовать.",
        "cpuFallbackFailed": "Модель была повторно запущена на CPU, но восстановленная конфигурация всё равно завершилась ошибкой.",
        "failed": "Этот отчёт сохраняет последнее известное состояние рантайма, чтобы вы могли понять, что произошло."
      }
    },
    "sections": {
      "generation": "Параметры генерации",
      "runtime": "Рантайм",
      "reasoning": "Рассуждение",
      "reasoningThinking": "Рассуждение (мышление)",
      "promptCaching": "Кеширование промптов",
      "capabilities": "Возможности"
    },
    "sectionDescriptions": {
      "generation": "Температура, сэмплирование, штрафы и лимиты вывода.",
      "generationAutomatic1111": "Сэмплер, CFG, seed, негативный промпт, denoise и размеры по умолчанию.",
      "runtime": "Управление выполнением, памятью и оборудованием для {{runtime}}.",
      "reasoning": "Режим мышления, усилие рассуждения и бюджет токенов.",
      "promptCaching": "Поведение кеширования контекста и префиксов.",
      "capabilities": "Входные и выходные модальности, которые поддерживает эта модель.",
      "capabilitiesSimple": "Отметьте, какие модальности принимает эта модель и что она умеет выдавать."
    },
    "summaries": {
      "generationAutomatic1111": "Параметры Stable Diffusion: сэмплер, CFG, seed и размеры по умолчанию",
      "generationDefault": "Параметры сэмплирования и лимиты вывода по умолчанию",
      "runtimeLlama": "Параметры выполнения, памяти и оборудования по умолчанию",
      "runtimeOllama": "Параметры рантайма Ollama по умолчанию",
      "reasoningAlwaysEnabled": "Всегда включено для этого провайдера",
      "reasoningDisabled": "Рассуждение выключено",
      "reasoningDefault": "Параметры мышления остаются значениями провайдера по умолчанию",
      "textOnly": "Только текст",
      "capabilities": "Ввод: {{input}} • Вывод: {{output}}"
    },
    "reasoning": {
      "enabled": "Включено",
      "enabledDescription": "Показывать процесс мышления",
      "effort": "Усилие рассуждения",
      "helpLabel": "Помощь по режиму рассуждения",
      "budgetTokens": "Бюджет токенов",
      "providerDefault": "Значение провайдера по умолчанию"
    },
    "runtimeFacts": {
      "updated": "Обновлено",
      "modelPath": "Путь к модели",
      "backendUsed": "Использованный бэкенд",
      "failureStage": "Этап сбоя",
      "requestedContext": "Запрошенный контекст",
      "recommendedContext": "Рекомендуемый контекст",
      "initialContext": "Начальный контекст",
      "actualContext": "Фактический контекст",
      "requestedGpuLayers": "Запрошенные GPU-слои",
      "actualGpuLayers": "Фактические GPU-слои",
      "requestedBatch": "Запрошенный пакет",
      "initialBatch": "Начальный пакет",
      "actualBatch": "Фактический пакет",
      "smartOffloadFallback": "Откат smart offload",
      "active": "Активно",
      "notNeeded": "Не требуется",
      "kqvFallback": "Откат KQV",
      "movedToRam": "Перемещено в RAM",
      "smartOffloadEstimate": "Оценка smart offload",
      "smartOffloadCandidates": "Кандидаты smart offload",
      "kvCache": "KV-кеш",
      "kqvOffload": "Выгрузка KQV",
      "flashAttention": "Вспышка внимания",
      "gpuBackends": "GPU-бэкенды",
      "availableRam": "Доступная RAM",
      "availableVram": "Доступная VRAM",
      "modelSize": "Размер модели",
      "promptTokens": "Токены промпта",
      "promptPositions": "Позиции промпта",
      "targetNewTokens": "Целевые новые токены",
      "completionTokens": "Токены завершения",
      "finishReason": "Причина завершения",
      "firstToken": "Первый токен",
      "throughput": "Пропускная способность",
      "promptTemplate": "Шаблон промпта"
    },
    "fields": {
      "platform": "Платформа",
      "displayName": "Отображаемое имя",
      "modelPath": "Путь к модели (GGUF)",
      "modelId": "ID модели"
    },
    "placeholders": {
      "displayName": "напр., Мой любимый ChatGPT",
      "modelPath": "/path/to/model.gguf",
      "modelId": "напр., gpt-4o",
      "sdSteps": "28",
      "sdCfgScale": "6.5",
      "sdSize": "1024x1024",
      "sdSampler": "DPM++ 2M Каррас",
      "random": "Случайно",
      "sdDenoise": "0.75",
      "sdNegativePrompt": "размыто, низкое качество, плохая анатомия, лишние пальцы",
      "temperature": "0.70",
      "topP": "1.00",
      "zero": "0.00",
      "batch512": "512",
      "default": "По умолчанию",
      "mmprojPath": "/path/to/mmproj.gguf",
      "stopSequences": "напр. \n\n###\nUser:\n"
    },
    "localLibrary": {
      "localPathHelp": "Используйте полный путь к локальной модели GGUF.",
      "mmprojTitle": "Загруженные файлы MMProj",
      "mmprojEmpty": "Файлы mmproj пока не загружены",
      "mmprojEmptyHint": "Скачайте мультимодальный проектор через Браузер моделей или введите путь вручную."
    },
    "modelSource": {
      "useCatalog": "Использовать каталог",
      "enterManually": "Ввести вручную",
      "refreshModelList": "Обновить список моделей",
      "onlyFreeModels": "только бесплатные модели",
      "customEndpointFetchDisabled": "Получение моделей отключено для этого пользовательского эндпоинта. Включите его в настройках провайдера и задайте Models Endpoint, если хотите обнаружение списка моделей."
    },
    "promptCaching": {
      "automatic": {
        "title": "Автоматическое кеширование у провайдера",
        "description": "Этот провайдер автоматически обрабатывает кеширование промптов на уровне своего API. На стороне приложения нет отдельного переключателя."
      },
      "enableTitle": "Включить кеширование контекста",
      "enableDescription": "Сохранять статические системные промпты и контекст документов между взаимодействиями.",
      "ttlTitle": "TTL кеша",
      "ttlDescription": "Как долго кешированные префиксы остаются действительными между запросами.",
      "pricingTitle": "Примечание по стоимости:",
      "pricingDescription": "Хотя кеширование снижает стоимость повторяющихся входных токенов, первоначальная запись в кеш может стоить немного дороже в зависимости от выбранного провайдера.",
      "oneHourNote": "Расширенный TTL на 1 час может поддерживаться не всеми провайдерами. При отсутствии поддержки используется срок жизни кеша провайдера по умолчанию.",
      "openai24hNote": "OpenAI использует политики хранения `in_memory` и `24h` вместо TTL на 1 час.",
      "groqLabel": "Groq:",
      "groqDescription": "кеширование происходит автоматически только на поддерживаемых моделях. Это приложение не форсирует и не настраивает его для отдельных запросов.",
      "geminiLabel": "Gemini:",
      "geminiDescription": "неявное кеширование происходит автоматически на поддерживаемых моделях. Явные ресурсы кешированного контента это приложение пока не создаёт.",
      "ttl": {
        "inMemory": "В памяти",
        "24h": "24 ч",
        "5min": "5 мин",
        "1h": "1 ч"
      }
    },
    "toasts": {
      "runtimeConfigApplied": "Конфигурация рантайма применена",
      "runtimeConfigAppliedDescription": "Будущие локальные запуски будут использовать последний безопасный для CPU контекст и пакет.",
      "modelPathRequired": "Требуется путь к модели",
      "modelPathRequiredDescription": "Выберите путь к модели GGUF перед чтением встроенного шаблона.",
      "embeddedTemplatePasted": "Встроенный шаблон вставлен в редактор"
    },
    "search": {
      "didYouMean": "Возможно, вы имели в виду:"
    },
    "capabilities": {
      "helpLabel": "Помощь по возможностям",
      "input": "Ввод",
      "output": "Вывод",
      "automatic1111Fixed": "Модели AUTOMATIC1111 фиксированы на ввод текста и изображения и вывод изображения."
    },
    "runtimeSummary": {
      "ram": "RAM",
      "vram": "VRAM",
      "kvCacheInVram": "KV-кеш в VRAM",
      "kvCacheInRam": "KV-кеш в RAM"
    },
    "help": {
      "temperature": "Помощь по температуре",
      "topP": "Помощь по top p",
      "maxOutputTokens": "Помощь по макс. токенам вывода",
      "topK": "Помощь по top k",
      "frequencyPenalty": "Помощь по штрафу частоты",
      "presencePenalty": "Помощь по штрафу присутствия",
      "contextLength": "Помощь по длине контекста"
    },
    "contextInfo": {
      "maxSupported": "Макс. поддерживаемое",
      "recommended": "Рекомендуемое",
      "availableRam": "Доступная RAM",
      "availableVram": "Доступная VRAM",
      "modelSize": "Размер модели",
      "contextCache": "Кеш контекста",
      "memoryFitness": "Пригодность памяти",
      "gpuAcceleration": "Ускорение GPU",
      "kvHeadroom": "Запас KV",
      "quantQuality": "Качество квантизации"
    },
    "llama": {
      "toggleStrictMode": "Переключить строгий режим llama"
    },
    "ollama": {
      "numCtxShort": "Num Ctx"
    },
    "continueSetup": {
      "continue": "Продолжить настройку",
      "saveToContinue": "Сохраните модель, чтобы продолжить"
    },
    "generation": {
      "automatic1111Help": "AUTOMATIC1111 использует здесь параметры Stable Diffusion. Эти значения становятся настройками сэмплера по умолчанию для аватаров, изображений сцен и других локальных diffusion-запросов.",
      "formatWidthHeight": "Формат: ширина x высота"
    },
    "generationDescriptions": {
      "sdSteps": "Шаги diffusion-сэмплирования",
      "sdCfgScale": "Сила следования промпту",
      "sdSize": "Используется, когда запрос не переопределяет размер",
      "sdSampler": "Имя сэмплера, отправляемое в A1111",
      "sdSeed": "Оставьте пустым для случайной генерации",
      "sdDenoise": "Сила редактирования для генераций по референсу",
      "sdNegativePrompt": "Применяется ко всем запросам AUTOMATIC1111 для этой модели",
      "temperature": "Выше = более креативно",
      "topP": "Ниже = более сфокусировано",
      "maxOutputTokens": "Ограничить длину ответа",
      "topK": "Сэмплировать из топ-K токенов",
      "frequencyPenalty": "Уменьшить повтор слов",
      "presencePenalty": "Поощрять новые темы"
    },
    "moveModel": {
      "title": "Переместить файл модели"
    },
    "parameterSupport": {
      "title": "Поддержка параметров"
    },
    "editorMode": {
      "title": "Режим редактора",
      "description": "Простой режим начинается свёрнутым. Расширенный режим сохраняет текущий полный редактор.",
      "simple": "Простой",
      "advanced": "Расширенный",
      "emptyState": "Откройте раздел, чтобы настроить его параметры. Когда нужен полный формуляр, остаётся доступен расширенный редактор."
    },
    "templateOverride": {
      "title": "Переопределение шаблона",
      "hideEmbedded": "Скрыть встроенный",
      "showEmbedded": "Показать встроенный",
      "close": "Закрыть",
      "readingEmbedded": "Чтение встроенного шаблона...",
      "readEmbeddedFailed": "Не удалось прочитать встроенный шаблон",
      "copiedToClipboard": "Скопировано в буфер обмена",
      "pasteIntoEditor": "Вставить в редактор",
      "jinjaTemplate": "Шаблон Jinja",
      "placeholder": "Введите шаблон чата Jinja или имя внутреннего шаблона..."
    }
  },
  "hfBrowser": {
    "title": "Браузер моделей",
    "searchPlaceholder": "Поиск моделей GGUF на HuggingFace...",
    "searching": "Поиск...",
    "trending": "Популярные модели GGUF",
    "noResults": "Модели не найдены",
    "noResultsHint": "Попробуйте другой поисковый запрос или просмотрите популярные модели.",
    "likes": "нравится",
    "downloads": "загрузок",
    "viewFiles": "Просмотр файлов",
    "files": "Доступные файлы",
    "noFiles": "В этом репозитории не найдено файлов GGUF.",
    "architecture": "Архитектура",
    "contextLength": "Длина контекста",
    "parameters": "Параметры",
    "quantization": "Квантизация",
    "fileSize": "Размер",
    "download": "Скачать",
    "downloading": "Загрузка...",
    "cancelDownload": "Отменить загрузку",
    "downloadComplete": "Загрузка завершена!",
    "downloadFailed": "Загрузка не удалась",
    "downloadCancelled": "Загрузка отменена",
    "downloadProgress": "{{downloaded}} / {{total}}",
    "progress": "Прогресс",
    "fileOfTotal": "Файл {{current}} из {{total}}",
    "speed": "{{speed}}/с",
    "alreadyDownloaded": "Уже загружено",
    "createModel": "Создать модель",
    "backToSearch": "Назад к поиску",
    "backToFiles": "Назад к файлам",
    "sortTrending": "Популярные",
    "sortDownloads": "Больше всего загрузок",
    "sortLikes": "Больше всего лайков",
    "sortRecent": "Недавно обновлённые",
    "browseOnHuggingFace": "Открыть на HuggingFace",
    "selectFromLibrary": "Выбрать из библиотеки",
    "libraryEmpty": "Загруженных моделей пока нет",
    "libraryEmptyHint": "Загрузите модели GGUF из Браузера моделей или введите путь вручную.",
    "libraryTitle": "Загруженные модели",
    "moveToLibrary": "Могу переместить файл этой модели в папку моделей GGUF, если хотите. Так все ваши модели будут организованы в одном месте.",
    "moveToLibraryYes": "Да, переместить",
    "moveToLibraryNo": "Нет, оставить на месте",
    "moveToLibraryMoving": "Перемещение модели...",
    "moveToLibrarySuccess": "Модель успешно перемещена!",
    "moveToLibraryFailed": "Не удалось переместить модель",
    "runabilityExcellent": "Отлично!",
    "runabilityGood": "Хорошо",
    "runabilityMarginal": "На грани",
    "runabilityPoor": "Плохо",
    "runabilityUnrunnable": "Невозможно запустить",
    "recommendedSettings": "Рекомендуемые настройки",
    "kvCacheType": "Тип KV-кеша",
    "gpuFull": "Полная выгрузка на GPU",
    "gpuNearFull": "Почти полная GPU, небольшой KV-перелив",
    "gpuKvSpill": "Веса на GPU, KV переливается в RAM",
    "gpuKvHeavySpill": "Веса на GPU, большая часть KV в RAM",
    "gpuMostLayers": "Большинство слоёв на GPU",
    "gpuHalfLayers": "Половина слоёв на GPU",
    "gpuFewLayers": "Мало слоёв на GPU",
    "gpuCpu": "Только CPU",
    "notRecommended": "Мы не рекомендуем запускать эту модель на вашем устройстве. Она не будет работать плавно.",
    "moreDetails": "Ещё",
    "detailedReport": "Отчёт о ресурсах",
    "detailSystem": "Системные ресурсы",
    "detailRam": "Доступная RAM",
    "detailVram": "Доступная VRAM",
    "detailVramBudget": "Бюджет VRAM (90%)",
    "detailTotalAvailable": "Всего доступно",
    "detailArchitecture": "Архитектура модели",
    "detailArch": "Архитектура",
    "detailLayers": "Слои",
    "detailEmbedding": "Размерность эмбеддинга",
    "detailHeads": "Головы внимания",
    "detailKvHeads": "KV-головы",
    "detailFfn": "Размерность Feed-Forward",
    "detailTrainCtx": "Контекст обучения",
    "detailConfig": "Текущая конфигурация",
    "detailModelSize": "Размер файла модели",
    "detailMemory": "Разбивка памяти",
    "detailWeights": "Веса модели",
    "detailKvCache": "KV-кеш",
    "detailTotalNeeded": "Всего требуется",
    "detailHeadroom": "Запас",
    "detailGpuFit": "Выгрузка на GPU",
    "detailScoreBreakdown": "Разбивка оценки",
    "detailMemFitness": "Пригодность памяти",
    "detailGpuAccel": "Ускорение GPU",
    "detailKvHeadroom": "Запас KV",
    "detailQuantQuality": "Качество квантизации",
    "detailFinalScore": "Взвешенная оценка",
    "detailComputeBuffer": "Буфер вычислений",
    "detailMemMode": "Режим памяти",
    "detailUnified": "Объединённая (общая RAM/VRAM)",
    "detailSwa": "Скользящее окно",
    "detailMlaRank": "Латентный ранг MLA",
    "detailParseStatus": "Анализ заголовка",
    "detailIncomplete": "Неполный (метаданные MoE слишком большие)",
    "detailEffectiveKvCtx": "Эффективный KV-контекст",
    "detailOffload": "Выгрузка на GPU",
    "detailCtxTip": "Уменьшение контекста до {{ctx}} токенов позволит 100% выгрузку на GPU.",
    "upgradeSuggestion": "{{quant}} ({{size}}) тоже подходит и набирает {{score}} — лучшее качество.",
    "layerTip": "Рекомендуется: выгрузить {{layers}}/{{total}} слоёв (-ngl {{layers}})",
    "detailKvDistribution": "Распределение KV-кеша",
    "detailKvOnGpu": "GPU (VRAM)",
    "detailKvOnRam": "Системная RAM",
    "kvDistributionTip": "{{pct}}% KV-кеша находится в RAM. Обработка промпта (prefill) будет медленнее — 100% GPU обеспечивает мгновенный отклик.",
    "detailLayers-ngl": "Слоёв для выгрузки (-ngl)",
    "detailOptimalGpuCtx": "Оптимальный контекст GPU",
    "detailOptimalRamCtx": "Макс. контекст RAM",
    "optimalGpuCtxLabel": "Полная скорость GPU: {{ctx}} токенов",
    "optimalRamCtxLabel": "Макс. RAM: {{ctx}} токенов",
    "optimalGpuCtxShort": "GPU: {{ctx}}",
    "optimalRamCtxShort": "Макс: {{ctx}}",
    "fullGpuOffloadShort": "100% GPU: {{ctx}}",
    "ctxExceedsGpu": "Контекст превышает оптимум GPU ({{ctx}}). KV-кеш будет переливаться в RAM, снижая скорость.",
    "modelExceedsVram": "Модель превышает VRAM. Запуск из RAM с частичной выгрузкой на GPU."
  },
  "systemPrompts": {
    "filters": {
      "all": "Все",
      "system": "Системные",
      "internal": "Внутренние",
      "custom": "Пользовательские"
    },
    "empty": {
      "title": "Пользовательских промптов пока нет",
      "description": "Создайте пользовательские системные промпты для персонализации AI-разговоров",
      "createButton": "Создать промпт"
    },
    "preview": {
      "whatLlmSees": "Что видит LLM",
      "turns": "Turns",
      "noMessages": "Нет сообщений",
      "noMessagesHint": "Добавить ",
      "showMore": "Показать больше",
      "showLess": "Показать меньше",
      "statChat": "chat",
      "statInjected": "injected",
      "statTotal": "total",
      "entry": "Entry",
      "editEntry": "Изменить запись",
      "reorder": "Изменить порядок",
      "delete": "Удалить",
      "deleteTitle": "Удалить запись?",
      "deleteMessage": "Удалить «{{name}}» из шаблона приглашения? ",
      "deleteConfirm": "Удалить",
      "thisEntry": "эта запись",
      "condensedName": "Краткая системная подсказка",
      "imageAttachment": "[Прикрепленное изображение: {{label}}]",
      "imageSlot": {
        "character": "Справочное изображение персонажа",
        "persona": " Справочное изображение персонажа",
        "chatBackground": "Фоновое изображение чата",
        "avatar": "Изображение аватара",
        "references": "Справочные изображения"
      },
      "injection": {
        "relative": "relative",
        "inChat": "inChat · глубина {{depth}}",
        "conditional": "условный · мин {{min}}",
        "interval": "interval · "
      }
    }
  },
  "personas": {
    "empty": {
      "title": "Персон пока нет",
      "description": "Создайте персону, чтобы задать, как AI будет к вам обращаться",
      "createButton": "Создать персону"
    },
    "actions": {
      "editPersona": "Редактировать персону",
      "setAsDefault": "Сделать по умолчанию",
      "setAsDefaultDesc": "Использовать для всех новых чатов",
      "unsetAsDefault": "Убрать из умолчания",
      "unsetAsDefaultDesc": "Снять статус по умолчанию",
      "exportPersona": "Экспорт персоны",
      "deletePersona": "Удалить персону"
    },
    "edit": {
      "avatarHint": "Нажмите, чтобы добавить или сгенерировать аватар",
      "nameLabel": "ИМЯ ПЕРСОНЫ",
      "namePlaceholder": "напр., Профессионал, Писатель, Студент...",
      "nameHint": "Дайте персоне описательное имя",
      "nicknameLabel": "ПСЕВДОНИМ (ОПЦИОНАЛЬНО)",
      "nicknamePlaceholder": "напр., Рабочий вариант, Режим RPG...",
      "nicknameHint": "Личный псевдоним, чтобы различать варианты этой персоны в вашей библиотеке",
      "descriptionLabel": "ОПИСАНИЕ",
      "descriptionPlaceholder": "Опишите, как AI должен обращаться к вам, ваши предпочтения, предысторию или стиль общения...",
      "wordCount": "слов",
      "descriptionHint": "Будьте конкретны в том, как вы хотите, чтобы к вам обращались",
      "setAsDefault": "Сделать по умолчанию",
      "defaultDescription": "Использовать эту персону для всех новых разговоров",
      "exportButton": "Экспорт персоны"
    },
    "designReferences": {
      "title": "Ссылки на дизайн",
      "description": "Прикрепите несколько ссылок на стабильные изображения и одно краткое примечание к дизайну для создания сцены."
    },
    "create": {
      "namePlaceholderExample": "Профессиональный писатель",
      "descriptionPlaceholderExample": "Пишите профессионально, ясно и лаконично. "
    },
    "errors": {
      "exportFailed": "Не удалось экспортировать личность",
      "importFailed": "Не удалось импортировать личность.",
      "loadFailed": "Не удалось загрузить persona",
      "saveFailed": "Не удалось сохранить persona"
    },
    "importToast": {
      "legacyJsonTitle": "Обнаружен устаревший импорт JSON",
      "legacyJsonMessage": "Импорт JSON устарел и скоро будет удален. ",
      "successMessage": "Персона успешно импортирована! "
    }
  },
  "security": {
    "pureMode": {
      "off": "Выкл",
      "offDesc": "Весь контент разрешён",
      "low": "Низкий",
      "lowDesc": "Блокирует откровенный сексуальный контент + ругательства",
      "standard": "Стандартный",
      "standardDesc": "Блокирует NSFW + жестокость",
      "strict": "Строгий",
      "strictDesc": "Максимальная фильтрация + без намёков"
    }
  },
  "advanced": {
    "sectionTitles": {
      "aiFeatures": "AI-функции",
      "memorySystem": "Система памяти",
      "usageAnalytics": "Аналитика использования"
    },
    "creationHelper": {
      "title": "Помощник создания",
      "description": "Мастер создания персонажей с AI"
    },
    "helpMeReply": {
      "title": "Помоги ответить",
      "description": "Предложения ответов с помощью AI"
    },
    "dynamicMemory": {
      "title": "Динамическая память",
      "contextWindow": "Окно контекста",
      "contextWindowDesc": "Количество недавних сообщений (1-1000)",
      "infoText": "Динамическая память использует ИИ для автоматического обобщения и управления контекстом разговора, обеспечивая более длительные и связные беседы.",
      "disabledText": "Когда отключено, приложение использует простое скользящее окно последних сообщений, определяемое настройкой Окна контекста."
    },
    "usageAnalytics": {
      "recalculateTitle": "Пересчитать стоимость использования",
      "recalculateDesc": "Обновить все исторические записи с правильными ценами",
      "recalculating": "Пересчёт...",
      "recalculateButton": "Пересчитать все расходы",
      "openRouterApiKeyRequired": "Требуется API-ключ OpenRouter. Настройте его в Настройки → Провайдеры.",
      "importantLabel": "Важно:",
      "warningCannotUndo": "Это действие нельзя отменить",
      "warningMayTakeTime": "Может занять время при большом количестве записей",
      "warningOnlyOpenRouter": "Обновляются только записи OpenRouter с токенами",
      "warningExistingValues": "Существующие значения стоимости будут перезаписаны"
    },
    "extra": {
      "creationHelperDetail": "Получайте интеллектуальные предложения по личностным качествам, предыстории и стилю диалога",
      "helpMeReplyDetail": "Создавайте контекстные варианты ответов на основе истории разговоров.",
      "lorebookEntryGenerator": "Генератор записей в книге знаний",
      "lorebookEntryDesc": "Превратите выбранные сообщения чата в устойчивые записи книги знаний и настройте черновики подсказок для написания записей и генерации ключевых слов.",
      "companions": "Companions",
      "companionModeDesc": "Управляйте моделями локального анализа эмоций, извлечения сущностей и маршрутизации памяти, используемых персонажами-компаньонами.",
      "companionSoulWriter": "Companion Soul Writer",
      "companionSoulDesc": "Выберите модель, резервную модель и шаблон подсказки, используемые для создания компаньонов. ",
      "network": "Network",
      "apiServer": "API-сервер",
      "apiServerDesc": "Представление моделей через сервер API, совместимый с OpenAI",
      "apiServerRunning": "Сервер в настоящее время работает"
    }
  },
  "backup": {
    "tabs": {
      "create": "Создать"
    },
    "create": {
      "newBackupButton": "Новое резервное копирование",
      "exportDescription": "Экспорт всех данных с шифрованием",
      "createButton": "Создать резервную копию"
    },
    "restore": {
      "availableBackups": "Доступные копии",
      "browseFiles": "Обзор файлов",
      "noBackupsFound": "Резервных копий не найдено",
      "noBackupsDesc": "Создайте копию или нажмите «Обзор файлов»",
      "browseDesc": "Найти файл .lettuce",
      "restoreDialogTitle": "Восстановить резервную копию",
      "deleteDialogTitle": "Удалить резервную копию",
      "embeddingPrompt": "Внедрение динамической памяти",
      "downloadModel": "Загрузить модель",
      "disableAndContinue": "Отключить и продолжить"
    },
    "extra": {
      "successMessage": "Резервная копия создана!",
      "savedLocation": "Сохранено в загрузках"
    }
  },
  "reset": {
    "title": "Сбросить всё",
    "description": "Это безвозвратно удалит всех провайдеров, модели, персонажей, сеансы чатов и настройки с этого устройства.",
    "warning": "Это действие нельзя отменить",
    "resetButton": "Сбросить все данные",
    "confirmTitle": "Вы уверены?",
    "confirmDescription": "Все ваши данные будут безвозвратно удалены. Приложение вернётся к начальной настройке.",
    "confirmButton": "Да, сбросить всё"
  },
  "chatAppearance": {
    "typography": "Типографика",
    "fontSize": {
      "label": "Размер шрифта",
      "small": "Маленький",
      "medium": "Средний",
      "large": "Большой",
      "xLarge": "Очень большой"
    },
    "lineSpacing": {
      "label": "Межстрочный интервал",
      "tight": "Плотный",
      "normal": "Обычный",
      "relaxed": "Свободный"
    },
    "messageBubbles": {
      "label": "Пузыри сообщений",
      "style": {
        "label": "Стиль",
        "bordered": "С рамкой",
        "filled": "Заполненный",
        "minimal": "Минимальный"
      },
      "cornerRadius": {
        "label": "Радиус углов",
        "sharp": "Острый",
        "rounded": "Скруглённый",
        "pill": "Капсула"
      },
      "maxWidth": {
        "label": "Макс. ширина",
        "compact": "Компактная",
        "normal": "Обычная",
        "wide": "Широкая"
      },
      "padding": {
        "label": "Отступы",
        "compact": "Компактные",
        "normal": "Обычные",
        "spacious": "Просторные"
      }
    },
    "layout": {
      "label": "Макет",
      "messageSpacing": "Интервал сообщений",
      "tight": "Плотный",
      "normal": "Обычный",
      "relaxed": "Свободный"
    },
    "avatar": {
      "shape": {
        "label": "Форма аватара",
        "circle": "Круг",
        "rounded": "Скруглённый",
        "hidden": "Скрытый"
      },
      "size": {
        "label": "Размер аватара",
        "small": "Маленький",
        "medium": "Средний",
        "large": "Большой"
      }
    },
    "colors": {
      "label": "Цвета",
      "userBubble": "Цвет пузыря пользователя",
      "assistantBubble": "Цвет пузыря ассистента",
      "userBubbleHex": "HEX-код пузыря пользователя",
      "assistantBubbleHex": "HEX-код пузыря ассистента",
      "neutral": "Нейтральный",
      "accent": "Акцентный",
      "info": "Информационный",
      "secondary": "Вторичный",
      "warning": "Предупреждение",
      "textColors": "Цвета текста",
      "messageTextHex": "Цвет встроенной цитаты",
      "plainTextHex": "Цвет обычного текста",
      "italicTextHex": "Цвет курсива",
      "quotedTextHex": "Цвет блочной цитаты",
      "inlineCodeTextHex": "Цвет встроенного кода"
    },
    "backgroundTransparency": {
      "label": "Фон и прозрачность",
      "backgroundDim": "Затемнение фона",
      "backgroundBlur": "Размытие фона",
      "bubbleBlur": "Размытие пузырей",
      "none": "Нет",
      "light": "Лёгкое",
      "medium": "Среднее",
      "heavy": "Сильное",
      "bubbleOpacity": "Непрозрачность пузырей"
    },
    "textColorMode": {
      "label": "Режим цвета текста",
      "auto": "Авто",
      "light": "Светлый",
      "dark": "Тёмный"
    },
    "preview": {
      "label": "Предпросмотр",
      "generic": "Общий",
      "live": "Живой"
    },
    "extra": {
      "reset": "Сбросить"
    }
  },
  "colorCustomization": {
    "tokens": {
      "surface": "Поверхность",
      "surfaceDesc": "Фоны страниц",
      "surfaceEl": "Приподнятая поверхность",
      "surfaceElDesc": "Карточки, модальные окна, выступающие элементы",
      "nav": "Навигация",
      "navDesc": "Верхняя и нижняя панели",
      "foreground": "Передний план",
      "foregroundDesc": "Границы, наложения, навигация и элементы интерфейса",
      "appText": "Текст приложения",
      "appTextDesc": "Основной текст и подписи интерфейса",
      "appTextMuted": "Приглушенный текст",
      "appTextMutedDesc": "Вторичный текст и вспомогательные подписи",
      "appTextSubtle": "Ненавязчивый текст",
      "appTextSubtleDesc": "Подсказки, вспомогательный текст и плейсхолдеры",
      "accent": "Акцент",
      "accentDesc": "Основные действия, успех",
      "info": "Информация",
      "infoDesc": "Информационные состояния, ссылки",
      "warning": "Предупреждение",
      "warningDesc": "Предостережения, оповещения",
      "danger": "Опасность",
      "dangerDesc": "Деструктивные действия, ошибки",
      "secondary": "Вторичный",
      "secondaryDesc": "AI-функции, творческие инструменты"
    },
    "presetsLabel": "Пресеты",
    "customPresetsLabel": "Пользовательские пресеты",
    "previewLabel": "Предпросмотр",
    "settingsCardsLabel": "Карточки настроек",
    "settingsCardsOpacity": "Непрозрачность карточек",
    "settingsCardsOpacityDesc": "Управляет тем, насколько прозрачными выглядят карточки настроек и строки списка.",
    "importButton": "Импорт",
    "exportButton": "Экспорт",
    "resetAllButton": "Сбросить все",
    "presets": {
      "defaultDark": "Тёмная по умолчанию",
      "midnightBlue": "Полночный синий",
      "warmEarth": "Теплая Земля",
      "purpleHaze": "Фиолетовая дымка",
      "rosePine": "Rosé Pine",
      "tokyoNight": "Токио Ночь",
      "catppuccin": "Catppuccin",
      "gruvbox": "Gruvbox",
      "nord": "Nord",
      "dracula": "Dracula",
      "solarized": "Solarized",
      "ayuDark": "Ayu Dark",
      "oneDark": "One Dark",
      "vesper": "Vesper",
      "cyberNeon": "Cyber Neon",
      "monochrome": "Monochrome"
    },
    "groups": {
      "backgrounds": "Фоны",
      "content": "Контент",
      "semantic": "Семантические"
    },
    "extra": {
      "surface": "Поверхность",
      "surfaceDesc": "Фон страницы",
      "surfaceEl": "Поверхность Повышенная",
      "surfaceElDesc": "Карточки, модальные окна, рельефные элементы",
      "nav": "Навигация",
      "navDesc": "Верхняя и нижняя панели",
      "fg": "Передний план",
      "fgDesc": "Границы, наложения, навигация, пользовательский интерфейс chrome",
      "appText": "Текст приложения",
      "appTextDesc": "Основной текст и метки интерфейса",
      "appTextMuted": "Текст без звука",
      "appTextMutedDesc": "Вторичный текст и вспомогательная копия",
      "appTextSubtle": "Тонкий текст",
      "appTextSubtleDesc": "Подсказки, вспомогательный текст, заполнители",
      "accent": "Accent",
      "accentDesc": "Основные действия, успех",
      "info": "Информация",
      "infoDesc": "Информационные состояния, ссылки",
      "warning": "Предупреждение",
      "warningDesc": "Состояния предупреждения, оповещения",
      "danger": "Опасность",
      "dangerDesc": "Деструктивные действия, ошибки",
      "secondary": "Вторичные"
    }
  },
  "dynamicMemory": {
    "page": {
      "info": "Динамическая память автоматически резюмирует разговоры для эффективного поддержания контекста. Выберите пресет или настройте параметры под свои нужды.",
      "disabledDirectTitle": "Динамическая память отключена для прямых чатов",
      "disabledDirectDescription": "Переключите переключатель на вкладке Прямые чаты, чтобы включить её. Групповые чаты используют режим памяти по сессиям.",
      "directChats": "Прямые чаты",
      "groupChats": "Групповые чаты",
      "enableDirectChats": "Включить для прямых чатов",
      "groupChatsInfo": "Групповые чаты используют режим памяти по сессиям. Включите динамическую память в настройках каждой группы. Эти настройки управляют поведением динамической памяти.",
      "memoryProfile": "Профиль памяти",
      "customSettings": "Пользовательские настройки — измените значения в расширенных параметрах ниже.",
      "contextEnrichment": "Обогащение контекста",
      "experimental": "Экспериментально",
      "contextEnrichmentDescription": "Использует недавние сообщения для более интеллектуального поиска в памяти",
      "advancedOptions": "Расширенные параметры",
      "advancedOptionsDescription": "Тонкая настройка поведения памяти",
      "summaryInterval": "Интервал резюмирования",
      "summaryIntervalDescription": "Сообщений между резюме",
      "maxMemoryEntries": "Макс. записей памяти",
      "maxMemoryEntriesDescription": "Максимальное количество сохранённых воспоминаний",
      "hotMemoryBudget": "Бюджет активной памяти",
      "hotMemoryBudgetDescription": "Лимит токенов для активных воспоминаний",
      "relevanceThreshold": "Порог релевантности",
      "relevanceThresholdDescription": "Минимальная похожесть для извлечения",
      "retrievalMode": "Режим извлечения",
      "retrievalModeSmart": "Умный",
      "retrievalModeCosine": "Косинусный",
      "retrievalModeDescription": "Умный сочетает релевантность с давностью/частотой. Косинусный использует чистую наибольшую схожесть.",
      "retrievalLimit": "Лимит извлечения",
      "retrievalLimitDescription": "Макс. воспоминаний, выбираемых за ход",
      "decayRate": "Скорость затухания",
      "decayRateDescription": "Как быстро уменьшается важность",
      "coldStorageThreshold": "Порог холодного хранения",
      "coldStorageThresholdDescription": "Когда воспоминания перемещаются в архив",
      "sharedSettings": "Общие настройки",
      "summarisationModel": "Модель резюмирования",
      "selectedModel": "Выбранная модель",
      "useGlobalDefaultModel": "Использовать глобальную модель по умолчанию",
      "noModelsAvailable": "Нет доступных моделей",
      "summarisationModelDescription": "Используется для резюмирования разговоров",
      "modelManagement": "Управление моделями",
      "testModel": "Тестировать модель",
      "downloadModel": "Скачать модель",
      "delete": "Удалить",
      "embeddingModel": "Модель эмбеддинга",
      "tokenCapacity": "Ёмкость токенов",
      "tokenCapacityDescription": "Чем больше значение, тем лучше память для длинных разговоров",
      "keepModelLoaded": "Держать модель загруженной",
      "keepModelLoadedDescription": "Сохраняет модель эмбеддинга + токенизатор в памяти, чтобы избежать повторной загрузки",
      "installedModel": "Установленная модель: {{version}} (макс. {{tokens}} токенов)",
      "downloadEmbeddingModel": "Скачать модель эмбеддинга",
      "downloadEmbeddingDescription": "Выберите версию для скачивания. Установленные версии отключены.",
      "downloadVersion": "Скачать {{version}}",
      "downloadV2Description": "Оптимизирована для точности и вспоминания длинного контекста",
      "downloadV3Description": "Новейшее качество эмбеддинга",
      "installed": "Установлена",
      "selectModel": "Выбрать модель",
      "searchModels": "Поиск моделей...",
      "deleteEmbeddingTitle": "Удалить модель {{version}}?",
      "deleteEmbeddingMessage": "Вы уверены, что хотите удалить {{version}}? Вы сможете скачать её позже.",
      "msgsUnit": "сообщ.",
      "entriesUnit": "записей",
      "tokensUnit": "токенов",
      "itemsUnit": "элем.",
      "perCycleUnit": "/ цикл"
    },
    "presets": {
      "minimal": "минимальный",
      "balanced": "сбалансированный",
      "comprehensive": "всесторонний",
      "minimalDesc": "Быстро и эффективно. ",
      "balancedDesc": "Хорошее сочетание сохранения контекста и производительности.",
      "comprehensiveDesc": "Максимум контекста. "
    },
    "presetInfo": {
      "minimal": "Быстро и эффективно. Сохраняет только самые важные воспоминания.",
      "balanced": "Хороший баланс между сохранением контекста и производительностью.",
      "comprehensive": "Максимальный контекст. Лучше всего для длинных, подробных разговоров."
    }
  },
  "helpMeReply": {
    "page": {
      "info": "«Помоги ответить» генерирует контекстные предложения для вашего следующего сообщения на основе истории разговора. Настройте модель и стиль ответа ниже."
    },
    "sectionTitles": {
      "modelConfiguration": "Настройка модели",
      "responseStyle": "Стиль ответа"
    },
    "labels": {
      "replyModel": "Модель ответа",
      "selectedModel": "Выбранная модель",
      "useAppDefault": "Использовать модель по умолчанию{{model}}",
      "useAppDefaultBase": "Использовать модель по умолчанию",
      "noModelsAvailable": "Нет доступных моделей",
      "replyModelDescription": "ИИ-модель для генерации предложений ответа",
      "streamingOutput": "Потоковый вывод",
      "streamingDescription": "Показывать предложения по мере генерации",
      "maxTokens": "Макс. токенов",
      "maxTokensDescription": "Максимальная длина предложений",
      "conversationalHint": "Предложения будут написаны как естественный диалог, подходящий для непринуждённых бесед.",
      "roleplayHint": "Предложения будут включать элементы ролевой игры — *действия* и описательные повествования.",
      "footerInfo": "Эта настройка применяется глобально ко всем разговорам. Меньшее количество токенов генерирует более короткие и быстрые предложения, а большее — более подробные ответы.",
      "selectReplyModel": "Выбрать модель ответа",
      "searchModels": "Поиск моделей..."
    },
    "responseStyle": {
      "conversational": "Разговорный",
      "conversationalDesc": "Естественный, неформальный тон",
      "roleplay": "Ролевой",
      "roleplayDesc": "Действия от лица персонажа"
    },
    "extra": {
      "conversational": "Разговорный",
      "roleplay": "Ролевая игра"
    }
  },
  "imageGeneration": {
    "promptPlaceholder": "Опишите изображение, которое хотите сгенерировать...",
    "labels": {
      "model": "МОДЕЛЬ",
      "prompt": "ПРОМПТ",
      "size": "РАЗМЕР",
      "quality": "КАЧЕСТВО",
      "style": "СТИЛЬ",
      "searchModels": "Поиск моделей...",
      "selectAvatarModel": "Выберите модель аватара",
      "selectSceneModel": "Выберите модель сцены",
      "selectWriterModel": "Выбрать модель сценариста сцен",
      "useFirstAvailable": "Использовать первую доступную модель",
      "useFirstCompatible": "Использовать первую совместимую модель сценариста"
    },
    "mode": {
      "title": "Режим",
      "description": "Выберите, как обрабатывать промпты сцены, найденные в ответе модели.",
      "auto": "Автоматически",
      "autoDescription": "Сразу генерировать изображение сцены, когда модель предлагает промпт сцены.",
      "askFirst": "Сначала спросить",
      "askFirstDescription": "Показывать найденный промпт сцены и ждать вашего подтверждения перед генерацией изображения.",
      "manual": "Вручную",
      "manualDescription": "Игнорировать промпты сцены из ответов модели. Использовать только действия, запущенные пользователем вручную."
    },
    "empty": {
      "title": "Нет моделей изображений",
      "description": "Добавьте модель генерации изображений на странице Моделей, чтобы начать генерацию."
    },
    "sections": {
      "avatar": {
        "title": "Генерация аватара",
        "description": "Модель по умолчанию, используемая при создании аватаров из средства выбора аватаров или связанных потоков изображений профиля."
      },
      "scene": {
        "title": "Генерация сцены",
        "description": "Зарезервированная модель для изображений сцен, созданных на основе контекста разговора или подсказок сцены."
      },
      "writer": {
        "title": "Сценарист сцен",
        "description": "Зарезервированная мультимодальная текстовая модель для составления промптов сцен и описаний визуальных референсов на основе контекста чата, аватаров и референсных изображений."
      }
    },
    "extra": {
      "avatarGeneration": "Генерация аватара",
      "sceneGeneration": "Создание сцены",
      "sceneWriter": "Сценарист"
    }
  },
  "logs": {
    "sectionTitles": {
      "diagnostics": "Диагностика",
      "generate": "Генерация",
      "copy": "Копирование"
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
      "testDataGenerators": "Генераторы тестовых данных",
      "storageMaintenance": "Обслуживание хранилища",
      "usageTracking": "Отслеживание использования",
      "crashTesting": "Краш-тестирование",
      "environmentInfo": "Информация о среде"
    },
    "testData": {
      "generateCharacter": "Сгенерировать тестового персонажа",
      "generateCharacterDesc": "Создать одного тестового персонажа",
      "generatePersona": "Сгенерировать тестовую персону",
      "generatePersonaDesc": "Создать одну тестовую персону",
      "generateSession": "Сгенерировать тестовый сеанс",
      "generateSessionDesc": "Создать тестовый сеанс чата с существующим персонажем",
      "generateBulk": "Массовая генерация тестовых данных",
      "generateBulkDesc": "Создать 3 персонажа и 2 персоны"
    },
    "storageMaintenance": {
      "optimizeDb": "Оптимизировать базу данных",
      "optimizeDbDesc": "Применить PRAGMA и запустить VACUUM (только мобильные)",
      "backupLegacy": "Резервное копирование устаревших файлов",
      "backupLegacyDesc": "Перемещает устаревшие .bin файлы в папку резервных копий"
    },
    "usageTracking": {
      "recalculateAll": "Пересчитать все расходы",
      "recalculateAllDesc": "Повторно получает цены и пересчитывает стоимость для всех записей OpenRouter"
    },
    "crashTesting": {
      "forceCrash": "Сбой приложения сейчас",
      "forceCrashDesc": "Немедленно завершает собственный процесс приложения для проверки обнаружения сбоев.",
      "forceCrashConfirm": "Это немедленно приведет к сбою приложения для проверки детектора сбоев. Продолжать?"
    },
    "environmentInfo": {
      "mode": "Режим",
      "devMode": "Режим разработки",
      "viteVersion": "Версия Vite"
    },
    "status": {
      "testCharacterCreated": "✓ Тестовый персонаж успешно создан",
      "testPersonaCreated": "✓ Тестовая персона успешно создана",
      "testSessionCreated": "✓ Тестовый сеанс создан: {{id}}",
      "generatingBulkData": "Генерация массовых тестовых данных...",
      "bulkDataCreated": "✓ Массовые тестовые данные созданы: 3 персонажа, 2 персоны",
      "creatingBenchmarkChat": "Создание бенчмарк-персонажа и сеанса...",
      "seededBenchmarkReady": "✓ Бенчмарк готов: {{name}} / {{id}}",
      "creatingBenchmarkGroupChat": "Создание группового чата бенчмарка...",
      "seededGroupBenchmarkReady": "✓ Групповой бенчмарк готов: {{id}}",
      "dbOptimized": "✓ База данных оптимизирована",
      "recalculatingCosts": "Пересчёт расходов... Это может занять некоторое время.",
      "toursReset": "✓ Все туры сброшены — они появятся снова при следующем посещении",
      "crashingApp": "Аварийное завершение приложения..."
    },
    "errors": {
      "noCharacters": "Нет доступных персонажей. Сначала создайте тестового персонажа.",
      "createCharacterFailed": "Не удалось создать тестового персонажа: {{error}}",
      "createPersonaFailed": "Не удалось создать тестовую персону: {{error}}",
      "createSessionFailed": "Не удалось создать тестовый сеанс: {{error}}",
      "createBulkFailed": "Не удалось создать массовые тестовые данные: {{error}}",
      "createBenchmarkFailed": "Не удалось создать сеанс бенчмарка: {{error}}",
      "createGroupBenchmarkFailed": "Не удалось создать групповой сеанс бенчмарка: {{error}}",
      "dbOptimizeFailed": "Оптимизация БД не удалась: {{error}}",
      "backupFailed": "Резервное копирование не удалось: {{error}}",
      "openRouterKeyMissing": "Ключ API OpenRouter не найден. Сначала настройте его в Настройки > Провайдеры.",
      "recalculationFailed": "Пересчёт не удался: {{error}}",
      "resetToursFailed": "Не удалось сбросить туры: {{error}}",
      "crashFailed": "Не удалось аварийно завершить приложение: {{error}}"
    },
    "onboarding": {
      "title": "Введение",
      "resetTours": "Сбросить все туры",
      "resetToursDesc": "Очищает состояние просмотра каждого тура, чтобы они воспроизвелись при следующем посещении."
    },
    "benchmarks": {
      "createChat": "Создать бенчмарк-чат",
      "createChatDesc": "Создаёт персонажа с динамической памятью, начальную сцену и сеанс проверки непрерывности из 20 сообщений, затем открывает его.",
      "createGroupChat": "Создать групповой бенчмарк-чат",
      "createGroupChatDesc": "Создаёт групповой чат с динамической памятью, тремя бенчмарк-персонажами и 30 сообщениями, затем открывает его."
    },
    "extra": {
      "testCharacter": "Тестовый персонаж",
      "testCharacterDesc": "Тестовый персонаж, созданный для целей разработки.",
      "testScene": "Простая тестовая сцена для разработки",
      "testPersona": "Тестовый персонаж",
      "testPersonaDesc": "Тестовый персонаж для разработки",
      "successChar": "✓ Тестовый персонаж успешно создан",
      "successPersona": "✓ Тестовый персонаж успешно создан",
      "successSession": "✓ Создан тестовый сеанс: {{id}}",
      "successBulk": "✓ Созданы массовые тестовые данные: 3 символа, 2 человека",
      "errorCharAvailable": "Нет доступных символов. ",
      "generatingBulk": "Генерация массовых тестовых данных..."
    }
  },
  "embeddingDownload": {
    "capacityOptions": {
      "oneK": "1K токенов",
      "oneKDesc": "Лучше для быстрых ответов",
      "twoK": "2K токенов",
      "twoKDesc": "Сбалансированная производительность",
      "fourK": "4K токенов",
      "fourKDesc": "Максимальный контекст"
    },
    "extra": {
      "status": "Загрузка..."
    }
  },
  "embeddingTest": {
    "categories": {
      "semanticSimilarity": "Семантическое сходство",
      "dissimilarityCheck": "Проверка различий",
      "roleplayContext": "Контекст ролевой игры"
    },
    "extra": {
      "placeholder": "Введите текст для встраивания..."
    }
  },
  "discovery": {
    "tabs": {
      "forYou": "Для вас",
      "trending": "В тренде",
      "popular": "Популярные",
      "new": "Новые"
    },
    "searchPlaceholder": "Поиск персонажей...",
    "viewAll": "Все",
    "errorTitle": "Что-то пошло не так",
    "noCardsFound": "Карточки не найдены",
    "sections": {
      "trendingNow": "Сейчас в тренде",
      "trendingSubtitle": "Популярное на этой неделе",
      "mostPopular": "Самые популярные",
      "popularSubtitle": "Любимцы сообщества",
      "freshArrivals": "Новые поступления",
      "freshSubtitle": "Недавно добавленные"
    },
    "browse": {
      "newArrivals": "Новые поступления",
      "freshCharacters": "Свежие персонажи",
      "noCharactersFound": "Персонажи не найдены",
      "noCharactersSubtitle": "Загляните позже за новым контентом"
    },
    "sort": {
      "mostLiked": "По лайкам",
      "mostDownloaded": "По загрузкам",
      "mostViewed": "По просмотрам",
      "mostMessages": "По сообщениям",
      "newestFirst": "Сначала новые",
      "recentlyUpdated": "Недавно обновлённые",
      "nameAZ": "По имени (А-Я)"
    },
    "sortBy": "Сортировка",
    "resultsUnit": "персонажей",
    "detail": {
      "share": "Поделиться",
      "nsfwOverlay": "Контент NSFW",
      "nsfwBadge": "NSFW",
      "originalBadge": "Оригинальный",
      "lorebookBadge": "Лорбук",
      "alsoKnownAs": "Также известен как:",
      "followersUnit": "подписчиков",
      "sections": {
        "description": "Описание",
        "tokenUsage": "Использование токенов",
        "startingScenes": "Начальные сцены",
        "scenario": "Сценарий",
        "personality": "Личность",
        "stats": "Статистика",
        "tags": "Теги",
        "author": "Автор"
      },
      "tokensTotalLabel": "всего",
      "tokens": {
        "description": "Описание",
        "personality": "Личность",
        "scenario": "Сценарий",
        "firstMessage": "Первое сообщение",
        "scenes": "Сцены",
        "examples": "Примеры",
        "systemPrompt": "Системный промпт"
      },
      "sceneLabels": {
        "primary": "Основная",
        "alternate": "Альтернативная"
      },
      "stats": {
        "views": "Просмотры",
        "downloads": "Загрузки",
        "messages": "Сообщения"
      },
      "downloaded": "Загружено",
      "startChat": "Начать чат",
      "downloadCharacter": "Скачать персонажа",
      "downloading": "Загрузка...",
      "downloadSuccess": {
        "title": "Персонаж загружен!",
        "subtitle": "Добавлен в вашу библиотеку",
        "badge": "Сохранено",
        "startChat": "Начать чат",
        "startChatDesc": "Открыть первую сцену сейчас",
        "viewLibrary": "Открыть в библиотеке",
        "viewLibraryDesc": "Редактировать, управлять или экспортировать позже",
        "continueBrowsing": "Продолжить просмотр",
        "continueBrowsingDesc": "Назад к обзору"
      },
      "errorTitle": "Ошибка",
      "errorSubtitle": "Не удалось загрузить",
      "errorNotFound": "Персонаж не найден",
      "defaultChatTitle": "Новый чат"
    },
    "search": {
      "placeholder": "Поиск персонажей, тегов, авторов...",
      "resultsUnit": "результатов",
      "timingUnit": "мс",
      "recentSearches": "Недавние запросы",
      "clearAll": "Очистить все",
      "trendingSearches": "Популярные запросы",
      "trends": {
        "anime": "аниме",
        "fantasy": "фэнтези",
        "romance": "романтика",
        "villain": "злодей",
        "adventure": "приключения",
        "comedy": "комедия",
        "mystery": "детектив",
        "sciFi": "фантастика"
      },
      "tips": {
        "title": "Советы по поиску",
        "tip1": "Ищите по имени персонажа, автору или описанию",
        "tip2": "Используйте теги: «аниме», «фэнтези» или «романтика»",
        "tip3": "Попробуйте конкретные черты: «цундере» или «злодей»"
      },
      "loading": "Загрузка...",
      "loadMore": "Загрузить ещё",
      "noResults": "Ничего не найдено",
      "noResultsFor": "Персонажи не найдены по запросу",
      "noResultsHint": "Попробуйте другие ключевые слова или просмотрите категории"
    },
    "errors": {
      "loadContent": "Не удалось загрузить контент",
      "searchFailed": "Поиск не выполнен",
      "noCardPath": "Путь к карте не указан",
      "loadCharacter": "Не удалось загрузить символ",
      "downloadCharacter": "Не удалось загрузить символ"
    },
    "card": {
      "byAuthor": "от {{author}}",
      "ocBadge": "OC"
    }
  },
  "engine": {
    "gpuInsufficient": "Недостаточно памяти GPU",
    "gpuFallbackDesc": "Эта модель не помещается в память GPU. Переключиться на CPU (медленнее) или отменить?",
    "switchToCpu": "Переключиться на CPU",
    "abort": "Отменить",
    "errors": {
      "providerNotFound": "Провайдер движка не найден.",
      "engineOffline": "Механизм не в сети или недоступен.",
      "deleteCharacterFailed": "Не удалось удалить символ.",
      "unknownCharacter": "Неизвестно",
      "seedRequired": "Требуется начальное описание.",
      "characterNameRequired": "Необходимо имя персонажа.",
      "atLeastOneProvider": "Должен быть включен хотя бы один поставщик.",
      "enableLlmProvider": "Включите хотя бы одного поставщика LLM.",
      "modelRequired": "Модель требуется для {{provider}}.",
      "apiKeyRequired": "Ключ API требуется для {{provider}}.",
      "sendMessageFailed": "Не удалось отправить сообщение."
    },
    "status": {
      "connected": "Подключено",
      "offline": "Не в сети",
      "needsSetup": "Требуется настройка"
    },
    "home": {
      "characters": "Персонажи",
      "newButton": "Новый",
      "noCharactersFound": "Персонажи не найдены.",
      "tokenUsage": "Использование токенов",
      "totalTokens": "всего токенов",
      "backgroundActivity": "Фоновая активность",
      "quickActions": "Быстрые действия",
      "configureProviders": "Настроить провайдеров",
      "engineSettings": "Настройки движка",
      "chat": "Чат",
      "chatDesc": "Начать разговор с этим персонажем",
      "deleteCharacter": "Удалить персонажа",
      "deletingCharacter": "Удаление...",
      "deleteDesc": "Безвозвратно удалить этого персонажа",
      "character": "Персонаж",
      "never": "Никогда",
      "justNow": "Только что",
      "timeAgo": {
        "minutes": "{{n}}м назад",
        "hours": "{{n}}ч назад",
        "days": "{{n}}дн назад"
      }
    },
    "tokens": {
      "input": "Ввод",
      "output": "Вывод"
    },
    "activity": {
      "synthesis": "Синтез",
      "consolidation": "Консолидация",
      "bm25Rebuild": "Перестройка BM25",
      "dripResearch": "Поэтапное исследование",
      "running": "Выполняется",
      "stopped": "Остановлено"
    },
    "setup": {
      "complete": "Настройка завершена!",
      "completeMessage": "Ваш движок Lettuce настроен и готов к работе.",
      "openDashboard": "Открыть панель"
    },
    "welcome": {
      "title": "Добро пожаловать в движок Lettuce",
      "subtitle": "Давайте настроим ваш AI-движок персонажей. Это займёт около 2 минут.",
      "feature1": "Движок даёт AI-персонажам постоянную память, эмоции, отношения и настоящую идентичность.",
      "feature2": "Сначала мы настроим бэкенд LLM, затем параметры движка.",
      "getStarted": "Начнём"
    },
    "config": {
      "activeProviders": "Активные провайдеры",
      "noModelSet": "Модель не выбрана",
      "defaultBadge": "По умолчанию",
      "noProvidersWarning": "Провайдеры не настроены. Добавьте хотя бы один LLM-бэкенд ниже.",
      "addProvider": "Добавить провайдера",
      "quickImport": "Быстрый импорт из провайдеров приложения",
      "importButton": "Импорт",
      "fields": {
        "model": "Модель",
        "modelPlaceholder": "напр. claude-sonnet-4-5-20250929",
        "apiKey": "API-ключ",
        "apiKeyPlaceholder": "Введите ваш API-ключ",
        "currentKey": "Текущий ключ:",
        "baseUrl": "Базовый URL",
        "baseUrlPlaceholder": "http://localhost:11434",
        "maxTokens": "Макс. токенов",
        "temperature": "Температура"
      },
      "enableProvider": "Включить провайдера",
      "setAsDefault": "Сделать по умолчанию",
      "defaultBackend": "Бэкенд по умолчанию",
      "remove": "Удалить",
      "saveChanges": "Сохранить изменения",
      "saving": "Сохранение...",
      "saved": "Сохранено"
    },
    "providers": {
      "title": "Провайдер LLM",
      "subtitle": "Движку необходим хотя бы один LLM-бэкенд для работы. Настройте одного или нескольких провайдеров ниже.",
      "importFromProviders": "Импорт из ваших провайдеров",
      "imported": "Импортировано",
      "use": "Использовать",
      "saveContinue": "Сохранить и продолжить"
    },
    "settings": {
      "fields": {
        "dataDirectory": "Каталог данных",
        "logLevel": "Уровень логирования",
        "maxHistory": "Макс. история (раундов разговора)"
      },
      "logLevels": {
        "debug": "ОТЛАДКА",
        "info": "ИНФО",
        "warning": "ПРЕДУПРЕЖДЕНИЕ",
        "error": "ОШИБКА"
      },
      "sections": {
        "engine": "Движок",
        "backgroundLoops": "Фоновые циклы",
        "memory": "Память",
        "safety": "Безопасность",
        "research": "Исследование"
      },
      "backgroundLoops": {
        "synthesis": "Синтез (мин.)",
        "consolidation": "Консолидация (мин.)",
        "bm25Rebuild": "Перестройка BM25 (мин.)",
        "dripResearch": "Поэтапное исследование (мин.)"
      },
      "memory": {
        "embeddingModel": "Модель Embedding",
        "maxRetrieval": "Макс. результатов поиска",
        "denseWeight": "Вес Dense",
        "bm25Weight": "Вес BM25",
        "graphWeight": "Вес Graph",
        "recencyBoost": "Буст по новизне (часы)",
        "randomSurface": "Вероятность случайного извлечения"
      },
      "safety": {
        "honestySection": "Раздел честности",
        "honestyDesc": "Включить раздел честности в системный промпт",
        "userDataDeletion": "Удаление данных пользователя",
        "userDataDesc": "Разрешить пользователям запрашивать удаление данных"
      },
      "research": {
        "scrapeOnBoot": "Сбор при запуске",
        "scrapeDesc": "Запускать сбор исследований при старте движка",
        "periodicInterval": "Периодический интервал (часы)"
      },
      "saveChanges": "Сохранить изменения",
      "saving": "Сохранение...",
      "saved": "Сохранено"
    },
    "settingsStep": {
      "title": "Настройки движка",
      "subtitle": "Настройте глобальные параметры движка. Все имеют разумные значения по умолчанию — можете пропустить.",
      "completingSetup": "Завершение настройки...",
      "completeSetup": "Завершить настройку"
    },
    "chat": {
      "sendMessage": "Отправить сообщение...",
      "sendButton": "Отправить сообщение",
      "typeMessage": "Введите сообщение",
      "back": "Назад",
      "assistantTyping": "Ассистент печатает",
      "fallbackName": "Чат"
    },
    "tagInput": {
      "addMore": "Добавить ещё...",
      "typeAndPressEnter": "Введите и нажмите Enter"
    },
    "characterCreate": {
      "steps": {
        "identity": {
          "title": "Идентичность",
          "aiGenerated": "Сгенерировано AI",
          "nameLabel": "Имя *",
          "namePlaceholder": "Имя персонажа",
          "eraLabel": "Эпоха",
          "eraPlaceholder": "напр. современная, Викторианская",
          "roleLabel": "Роль",
          "rolePlaceholder": "напр. Детектив, Учёный",
          "settingLabel": "Сеттинг",
          "settingPlaceholder": "Опишите, где живёт персонаж (от первого лица)...",
          "coreIdentityLabel": "Суть личности",
          "coreIdentityPlaceholder": "Кто этот персонаж в своей основе? (от первого лица, 3-5 предложений)",
          "backstoryLabel": "Предыстория",
          "backstoryPlaceholder": "История жизни и ключевые события (от первого лица)..."
        },
        "mode": {
          "title": "Создание персонажа",
          "subtitle": "Сгенерируйте персонажа с AI или создайте с нуля.",
          "aiBoost": "AI-ускорение",
          "aiBoostDesc": "Опишите идею персонажа, и AI сгенерирует полное определение.",
          "nameOptional": "Имя (необязательно)",
          "namePlaceholder": "напр. Марк Волков",
          "seedDescription": "Начальное описание *",
          "seedPlaceholder": "напр. джазовый пианист в Гарлеме 1950-х, философ, любит ночные беседы",
          "eraOptional": "Эпоха (необязательно)",
          "eraPlaceholder": "напр. 1950-е, современная, Викторианская",
          "generating": "Генерация...",
          "generateCharacter": "Сгенерировать персонажа",
          "or": "или",
          "startFromScratch": "Начать с нуля"
        },
        "personality": {
          "title": "Личность",
          "traits": "Черты личности",
          "traitsPlaceholder": "напр. остроумный, сострадательный, упрямый",
          "speechPatterns": "Речевые паттерны",
          "formality": "Формальность",
          "formal": "Формальный",
          "casual": "Неформальный",
          "texting": "Переписка",
          "verbosity": "Многословие",
          "terse": "Лаконичный",
          "medium": "Средний",
          "verbose": "Многословный",
          "textStyle": "Стиль текста",
          "dialect": "Диалект",
          "dialectPlaceholder": "напр. южнорусский, московский",
          "catchphrases": "Крылатые фразы",
          "catchphrasesPlaceholder": "напр. Ну как сказать...",
          "vocabPreferences": "Предпочтения в лексике",
          "vocabPreferencesPlaceholder": "Слова, которые предпочитает",
          "vocabAvoidances": "Избегаемая лексика",
          "vocabAvoidancesPlaceholder": "Слова, которых избегает",
          "fillerWords": "Слова-паразиты",
          "fillerWordsPlaceholder": "напр. эм, ну, типа",
          "exampleQuotes": "Примеры цитат",
          "exampleQuotesPlaceholder": "3-5 примеров реплик"
        },
        "world": {
          "title": "Мир и поведение",
          "knowledgeDomains": "Области знаний",
          "knowledgeDomainsPlaceholder": "напр. история джаза, теория музыки",
          "knowledgeBoundaries": "Границы знаний",
          "knowledgeBoundariesPlaceholder": "Темы, которые не знает",
          "researchSeeds": "Начальные темы для исследования",
          "researchSeedsPlaceholder": "Начальные темы для фонового исследования",
          "researchEnabled": "Исследование включено",
          "researchEnabledDesc": "Разрешить фоновый сбор знаний",
          "physicalDescription": "Физическое описание",
          "physicalDescPlaceholder": "Внешний вид и манеры...",
          "physicalHabits": "Физические привычки",
          "physicalHabitsPlaceholder": "напр. постукивает пальцами, поправляет очки",
          "idleBehaviors": "Поведение в покое",
          "idleBehaviorsPlaceholder": "Что делает, когда не занят",
          "timeBehaviors": "Поведение по времени суток",
          "timePlaceholder": "Что делает в {{period}}?",
          "earlyMorning": "Раннее утро",
          "morning": "Утро",
          "afternoon": "День",
          "evening": "Вечер",
          "night": "Ночь",
          "baselineEmotions": "Базовые эмоции (Плутчик)",
          "emotionDesc": "Задайте базовый эмоциональный фон (0 = нет, 1 = максимум)",
          "joy": "Радость",
          "trust": "Доверие",
          "fear": "Страх",
          "surprise": "Удивление",
          "sadness": "Грусть",
          "disgust": "Отвращение",
          "anger": "Гнев",
          "anticipation": "Предвкушение",
          "engineOverrides": "Переопределения движка",
          "backend": "Бэкенд",
          "model": "Модель",
          "temperature": "Температура",
          "leaveEmpty": "Оставьте пустым для значения по умолчанию"
        },
        "review": {
          "title": "Обзор",
          "subtitle": "Проверьте персонажа перед созданием.",
          "edit": "Редактировать",
          "notSet": "Не задано",
          "identitySection": "Идентичность",
          "personalitySection": "Личность",
          "worldSection": "Мир и поведение",
          "nameLabel": "Имя",
          "eraLabel": "Эпоха",
          "roleLabel": "Роль",
          "settingLabel": "Сеттинг",
          "coreIdentityLabel": "Суть личности",
          "backstoryLabel": "Предыстория",
          "traitsLabel": "Черты",
          "formalityLabel": "Формальность",
          "verbosityLabel": "Многословие",
          "dialectLabel": "Диалект",
          "catchphrasesLabel": "Крылатые фразы",
          "domainsLabel": "Области",
          "boundariesLabel": "Границы",
          "researchSeedsLabel": "Темы исследования",
          "researchLabel": "Исследование",
          "enabled": "Включено",
          "disabled": "Выключено",
          "physicalLabel": "Физическое",
          "habitsLabel": "Привычки",
          "idleLabel": "В покое",
          "timeBehaviorsLabel": "Поведение по времени",
          "emotionsLabel": "Эмоции",
          "configured": "Настроено",
          "backendLabel": "Бэкенд",
          "modelLabel": "Модель",
          "temperatureLabel": "Температура",
          "creating": "Создание...",
          "createCharacter": "Создать персонажа"
        }
      }
    }
  },
  "library": {
    "filterTitle": "Фильтр библиотеки",
    "filters": {
      "all": "Все",
      "characters": "Персонажи",
      "personas": "Персоны",
      "lorebooks": "Лорбуки",
      "images": "Изображения"
    },
    "emptyStates": {
      "all": {
        "title": "Ваша библиотека пуста",
        "description": "Создайте персонажей, персоны и лорбуки, чтобы увидеть их здесь"
      },
      "characters": {
        "title": "Персонажей пока нет",
        "description": "Создайте первого персонажа, чтобы начать общение"
      },
      "personas": {
        "title": "Персон пока нет",
        "description": "Создайте персону, чтобы настроить вашу идентичность в чате"
      },
      "lorebooks": {
        "title": "Лорбуков пока нет",
        "description": "Лорбуки создаются в настройках персонажа"
      }
    },
    "actions": {
      "startChat": "Начать чат",
      "editCharacter": "Редактировать персонажа",
      "editPersona": "Редактировать персону",
      "editLorebook": "Редактировать лорбук",
      "renameLorebook": "Переименовать лорбук",
      "exportCharacter": "Экспорт персонажа",
      "exportPersona": "Экспорт персоны",
      "chatAppearance": "Оформление чата",
      "deleteCharacter": "Удалить персонажа",
      "deletePersona": "Удалить персону",
      "deleteLorebook": "Удалить лорбук",
      "importLorebook": "Импорт лорбука"
    },
    "imageLibrary": {
      "filters": {
        "all": "Все",
        "backgrounds": "Фоны",
        "avatars": "Аватары",
        "attachments": "Вложения",
        "other": "Другое"
      },
      "searchPlaceholder": "Поиск по имени файла, пути, id сессии или id сущности",
      "empty": {
        "title": "Для этого вида нет подходящих изображений",
        "description": "Попробуйте другой фильтр или поисковый запрос. Библиотека показывает только изображения, уже сохранённые в локальном хранилище приложения."
      },
      "actions": {
        "sort": "Сортировать",
        "useThis": "Использовать",
        "using": "Используется...",
        "copyPath": "Копировать путь",
        "saving": "Сохранение...",
        "download": "Скачать",
        "delete": "Удалить изображение",
        "deleting": "Удаление..."
      },
      "active": "Активно",
      "messages": {
        "loadFailed": "Не удалось загрузить библиотеку изображений",
        "saved": "Изображение сохранено",
        "downloadFailed": "Ошибка загрузки",
        "useFailed": "Не удалось использовать это изображение",
        "deleted": "Изображение удалено",
        "deleteFailed": "Не удалось удалить изображение"
      },
      "deleteConfirm": {
        "title": "Удалить изображение?",
        "message": "Вы уверены, что хотите удалить \"{{filename}}\"? Это может сломать аватары, фоны чата или вложения сообщений, которые всё ещё используют его."
      },
      "sort": {
        "newest": "Новые",
        "largest": "Самый большой",
        "name": "Имя"
      },
      "kinds": {
        "background": "Фон",
        "avatar": "Аватар",
        "attachment": "Вложение",
        "stored": "Сохранено"
      },
      "detailsTitle": "{{kind}} подробности",
      "formatsLabel": "Форматы",
      "storagePath": "Путь к хранилищу",
      "contextLabel": "Context",
      "contextLinkedFallback": "Linked",
      "show": "Показать",
      "hide": "Скрыть",
      "contextRoles": {
        "character": "символ:",
        "session": "сессия:",
        "role": "роль:"
      },
      "downloadFormat": "{{download}} Формат",
      "unknownDate": "Неизвестно",
      "clearSearch": "Очистить поиск",
      "copyFilename": "Копировать имя файла",
      "copyLabels": {
        "filename": "Имя",
        "storagePath": "Путь к хранилищу"
      },
      "copy": {
        "copied": "{{label}} скопировано",
        "failed": "Не удалось скопировать {{label}}"
      }
    },
    "deleteConfirm": {
      "title": "Удалить {{itemType}}?",
      "message": "Вы уверены, что хотите удалить",
      "characterWarning": "Это также удалит все сеансы чата с этим персонажем."
    },
    "rename": {
      "title": "Переименовать лорбук",
      "placeholder": "Введите новое имя..."
    },
    "itemTypes": {
      "character": "Персонаж",
      "persona": "Персона",
      "lorebook": "Лорбук"
    },
    "lorebookLabel": "Лорбук",
    "noDescriptionYet": "Описание пока отсутствует",
    "errors": {
      "importLorebook": "Не удалось импортировать книгу знаний. ",
      "exportFailed": "Не удалось экспортировать"
    },
    "card": {
      "avatarAlt": "{{name}} аватар"
    },
    "lorebookEditor": {
      "titleOverride": "Книга знаний - {{name}}",
      "dragToReorder": "Перетащите, чтобы изменить порядок",
      "aria": {
        "generateEntry": "Создать запись книги знаний",
        "editLorebook": "Редактировать книгу истории",
        "exportLorebook": "Экспортировать книгу истории"
      }
    }
  },
  "onboarding": {
    "loading": "Загрузка провайдеров...",
    "stepIndicator": "Шаг {{current}} из {{total}}",
    "steps": {
      "provider": "Настройка провайдера",
      "model": "Настройка модели",
      "memory": "Система памяти",
      "stepNofM": "Шаг {{current}} из {{total}}"
    },
    "provider": {
      "availableProviders": "Доступные провайдеры",
      "chooseProvider": "Выберите провайдера",
      "titleMobile": "Выберите своего поставщика ИИ",
      "descMobile": "Выберите поставщика ИИ, чтобы начать. ",
      "configureProvider": "Настроить {{name}}",
      "connectProvider": "Подключите {{name}}",
      "connectProviderDesc": "Вставьте свой ключ API ниже, чтобы включить чаты. ",
      "localLLMs": "Местные LLM",
      "useLocalLLMs": "Я хочу использовать локальные LLM",
      "browseModelLibrary": "Просмотреть библиотеку моделей",
      "browseModelLibraryDesc": "Найти и загрузить модели GGUF с HuggingFace",
      "useOwnGguf": "Использовать мои собственные файлы GGUF",
      "useOwnGgufDesc": "Выбрать модель GGUF и дополнительный файл mmproj на своем устройстве",
      "fields": {
        "displayLabel": "Отобразить метку",
        "displayLabelHint": "Как этот провайдер будет отображаться в ваших меню",
        "displayLabelPlaceholder": "Мой {{name}}",
        "defaultLabelFallback": "Provider",
        "apiKey": "Ключ API",
        "apiKeyOptional": "Ключ API (необязательно)",
        "apiKeyHint": "Ключи шифруются локально",
        "apiKeyPlaceholderRemote": "sk-...",
        "apiKeyPlaceholderLocal": "Обычно не требуется",
        "whereToFind": "Где найти",
        "baseUrl": "Базовый URL",
        "baseUrlPlaceholderHost": "http://192.168.1.10:3333",
        "baseUrlPlaceholderLocal": "http://localhost:11434",
        "baseUrlPlaceholderRemote": "https://api.provider.com",
        "baseUrlPlaceholderIntenseRP": "http://127.0.0.1:7777/v1",
        "baseUrlHintLocal": "Адрес вашего локального сервера с портом",
        "baseUrlHintHost": "Введите URL-адрес хоста рабочего стола, показанный вашим хост-устройством",
        "baseUrlHintRemote": "При необходимости переопределите конечную точку по умолчанию",
        "chatEndpoint": "Конечная точка чата",
        "systemRole": "Системная роль",
        "userRole": "Роль пользователя",
        "assistantRole": "Роль помощника",
        "supportsStreaming": "Поддерживает потоковую передачу",
        "mergeSameRole": "Объединение сообщений одной роли",
        "toolChoiceMode": "Режим выбора инструмента",
        "toolChoiceHint": "Управляет отправкой Tool_choice в пользовательскую конечную точку."
      },
      "toolChoice": {
        "auto": "Auto",
        "required": "Required",
        "none": "Нет",
        "omit": "Пропустить поле",
        "passthrough": "Передача (конфигурация инструмента)"
      },
      "buttons": {
        "testConnection": "Тестовое соединение",
        "testing": "Тестирование..."
      },
      "descriptions": {
        "chutes": "Вывод, совместимый с OpenAI, для лучших проектов с открытым исходным кодом ",
        "openai": "Модели GPT-5, GPT-4.1 и GPT-4o для выразительного RP",
        "lettuceHost": "Подключайтесь к своему рабочему столу Lettuce Host через локальную сеть с помощью API в стиле OpenAI",
        "anthropic": "Claude 4.5 Sonnet и Haiku для глубокого эмоционального диалога",
        "aggregator": "Доступ к таким моделям, как ",
        "openaiCompatible": "Используйте любую конечную точку API в стиле OpenAI",
        "mistral": "Mistral Small 3.2, Mixtral 8x22B и другие модели Mistral.",
        "deepseek": "DeepSeek-V3.2-Exp, DeepSeek-R1 и другие высокоэффективные модели",
        "xai": "Grok-1.5, Grok-3 и более новые модели xAI",
        "zai": "GLM-4.5, GLM-4.6 и варианты Air",
        "moonshot": "Модели Kimi-K2 Thinking и Kimi-K1",
        "gemini": "Gemini 2.5 Flash, 2.5 Pro и другие",
        "qwen": "Qwen3-VL и более новые модели Qwen",
        "nvidia": "Nemotron, Llama, DeepSeek и другие с помощью NVIDIA NIM",
        "custom": "Направьте LettuceAI на любую конечную точку пользовательской модели",
        "fallback": "Поставщик моделей искусственного интеллекта"
      },
      "descriptionsShort": {
        "chutes": "Вывод модели с открытым исходным кодом",
        "openai": "GPT-5, GPT-4o, GPT-4.1",
        "lettuceHost": "Ваш собственный хост в локальной сети",
        "anthropic": "Claude 4.5 Sonnet & Haiku",
        "aggregator": "Мультимодельный агрегатор",
        "openaiCompatible": "Пользовательская конечная точка OpenAI",
        "mistral": "Модели Mistral и Mixtral",
        "deepseek": "DeepSeek-V3, R1",
        "xai": "Grok-1.5, Grok-3",
        "zai": "GLM-4.5, GLM-4.6",
        "moonshot": "Kimi-K2 Thinking",
        "gemini": "Gemini 2.5 Flash & Pro",
        "qwen": "Qwen3-VL модели",
        "nvidia": "Вывод NVIDIA NIM",
        "custom": "Пользовательская конечная точка",
        "fallback": "Поставщик AI"
      },
      "cardLabel": "{{step}}: {{name}}",
      "errors": {
        "hostUrlRequired": "Необходим URL-адрес хоста (например, http://192.168.1.10:3333)",
        "baseUrlRequired": "Необходим базовый URL-адрес (например, http://localhost:11434)",
        "apiKeyTooShort": "Ключ API кажется слишком коротким",
        "invalidApiKey": "Неверный ключ API",
        "connectionFailed": "Ошибка подключения",
        "verificationFailed": "Проверка не удалась",
        "failedToSave": "Не удалось сохранить поставщика",
        "connectionSuccessful": "Соединение успешно!",
        "modelNotFound": "Модель не найдена у поставщика",
        "modelVerificationFailed": "Проверка модели не удалась",
        "failedToSaveModel": "Не удалось сохранить модель"
      }
    },
    "model": {
      "noProvidersTitle": "Провайдеры не настроены",
      "noProvidersDesc": "Вам нужно подключить провайдера, прежде чем выбрать модель по умолчанию.",
      "goToProviderSetup": "Перейти к настройке провайдера",
      "yourProviders": "Ваши провайдеры",
      "yourProvidersHint": "Выберите, какой провайдер использовать",
      "setDefaultModel": "Установите модель по умолчанию",
      "setDefaultModelDesc": "Выберите, какой поставщик и имя модели LettuceAI должен использовать по умолчанию. ",
      "setDefaultModelDescDesktop": "Выберите поставщика из списка, чтобы настроить свою модель.",
      "modelDetails": "Сведения о модели",
      "modelDetailsDesc": "Определите идентификатор API и метку, которую вы увидите внутри приложения.",
      "whichModel": "Какую модель мне следует использовать?",
      "nextMemorySystem": "Далее: Система памяти",
      "fields": {
        "displayName": "Отображаемое имя",
        "displayNamePlaceholder": "Креативный наставник",
        "displayNameHint": "Как эта модель отображается в меню",
        "modelId": "ID модели",
        "modelPathGguf": "Путь к модели (GGUF)",
        "modelIdPlaceholder": "например. ",
        "modelPathPlaceholder": "/path/to/model.gguf",
        "modelIdHint": "Точный идентификатор, используемый для вызовов API",
        "showList": "Показать список",
        "manualInput": "Ручной ввод",
        "refreshModelList": "Обновить список моделей",
        "selectModel": "Выбрать модель",
        "selectAModel": "Выбрать модель...",
        "searchModels": "Поиск моделей...",
        "noModelsFound": "Модели, соответствующие \"{{query}}\", не найдены"
      },
      "fillBothFields": "Заполните оба поля выше, чтобы активировать кнопку завершения.",
      "providerNames": {
        "chutes": "Chutes",
        "intenseRpNext": "IntenseRP Next",
        "openai": "OpenAI",
        "anthropic": "Anthropic",
        "openrouter": "OpenRouter",
        "openaiCompatible": "Совместимость с OpenAI",
        "custom": "Пользовательская конечная точка"
      }
    },
    "memory": {
      "dynamicTitle": "Динамическая память",
      "recommended": "Рекомендуется",
      "settingUp": "Настройка...",
      "finishSetup": "Завершить настройку",
      "promptTitle": "Настройка динамической памяти",
      "oneLastStep": "Последний шаг",
      "downloadAndEnable": "Скачать и включить",
      "chooseStyle": "Выберите свой стиль памяти",
      "howRemember": "Как ваши ИИ-компаньоны должны запоминать подробности о вас и ваших разговорах?",
      "dynamicDescription": "Использует <0>локальную модель внедрения</0> для разумного управления ",
      "dynamicFeatures": {
        "quality": "Сохраняет качество в длинных чатах",
        "cost": "Значительно снижает затраты на API",
        "auto": "Автоматическое управление контекстом",
        "zeroConfig": "Нулевая настройка"
      },
      "manualTitle": "Ручная память",
      "manualBadge": "Классический опыт",
      "manualDescription": "Вы явно закрепляете сообщения и самостоятельно редактируете «Всемирную информацию» или определения персонажей. ",
      "manualFeatures": {
        "control": "Полный контроль над фактами",
        "scenarios": "Лучший вариант для конкретных сценариев"
      },
      "setupModelMessage": "Чтобы использовать динамическую память, нам необходимо загрузить небольшую модель внедрения (~120 МБ) на ваше устройство.",
      "setupBullets": {
        "offline": "Модель работает на вашем устройстве в 100% автономном режиме",
        "remembering": "Требуется для запоминания контекста",
        "disable": "Вы можете отключить это позже в настройках"
      },
      "stepLabel": "Шаг 3 из 3",
      "stepLabelMemory": "Система памяти"
    },
    "welcome": {
      "appName": "LettuceAI",
      "tagline": "Ваш личный AI-компаньон. Приватный, безопасный и всегда на устройстве.",
      "features": {
        "onDevice": "Только на устройстве",
        "characterReady": "Персонажи готовы"
      },
      "betaWarning": {
        "title": "Бета-версия Desktop",
        "description": "Вы используете десктоп-версию. Некоторые функции могут отличаться от мобильной. Сообщайте о проблемах на GitHub."
      },
      "languageSelector": {
        "title": "Язык",
        "description": "Автоматически определён по вашему устройству. Вы можете изменить его в любое время в настройках."
      },
      "getStarted": "Начать",
      "skipForNow": "Пропустить",
      "restoreFromBackup": "Восстановить из резервной копии",
      "setupTime": "Настройка занимает менее 2 минут",
      "skipWarning": {
        "title": "Пропустить настройку?",
        "warningTitle": "Для чата нужен провайдер",
        "warningMessage": "Без провайдера вы не сможете отправлять сообщения. Вы сможете добавить его позже в настройках.",
        "addProvider": "Добавить провайдера",
        "skipAnyway": "Всё равно пропустить"
      },
      "restoreBackup": {
        "title": "Восстановление резервной копии",
        "selectMessage": "Выберите резервную копию для восстановления.",
        "browse": "Обзор файлов",
        "processing": "Обработка файла...",
        "processingNote": "Большие копии могут обрабатываться минуту",
        "noBackups": "Резервные копии не найдены",
        "noBackupsHint": "Нажмите «Обзор», чтобы выбрать файл .lettuce",
        "browseLettuce": "Найти файл .lettuce",
        "passwordLabel": "Пароль резервной копии",
        "passwordPlaceholder": "Введите пароль",
        "restoreButton": "Восстановить копию",
        "restoring": "Восстановление...",
        "infoMessage": "Это настроит приложение с вашими сохранёнными данными, включая персонажей, чаты и настройки.",
        "embeddingTitle": "Требуется модель Embedding",
        "dynamicMemoryDetected": "Обнаружена динамическая память",
        "dynamicMemoryMessage": "Эта резервная копия содержит персонажей с включённой динамической памятью, для которой необходима модель embedding (~120 МБ).",
        "embeddingOptions": "Вы можете скачать модель сейчас или продолжить без неё (динамическая память будет отключена для затронутых персонажей).",
        "downloadModel": "Скачать модель",
        "continueWithoutDynamic": "Продолжить без динамической памяти",
        "embeddingNote": "Вы сможете повторно включить динамическую память позже в настройках персонажа после загрузки модели.",
        "back": "Назад",
        "cancel": "Отмена",
        "errors": {
          "passwordRequired": "Требуется пароль",
          "incorrectPassword": "Неверный пароль",
          "failedToOpenFile": "Не удалось открыть файл",
          "failedToRestore": "Не удалось восстановить резервную копию",
          "failedToUpdateSettings": "Не удалось обновить настройки"
        }
      }
    },
    "common": {
      "back": "Назад",
      "cancel": "Отмена",
      "continue": "Продолжить",
      "verifying": "Проверка...",
      "skipForNow": "Пропустить",
      "selectAProvider": "Выберите поставщика для настройки",
      "clickToSelectProvider": "Нажмите, чтобы выбрать поставщика",
      "selectProviderFromList": "Выберите поставщика из списка, чтобы начать.",
      "enterApiKey": "Введите свой ключ API, чтобы включить функцию чата с искусственным интеллектом."
    },
    "modelGuide": {
      "badge": "Руководство по модели",
      "title": "Как выбрать модель?",
      "intro": "LettuceAI не навязывает одну «лучшую» модель. ",
      "askYourself": "Спросите себя:",
      "factors": {
        "quality": {
          "title": "Качество и возможности",
          "description": "Насколько умной должна быть модель? ",
          "q1": "Вам нужна глубокая последовательность персонажей и эмоциональный интеллект?",
          "q2": "Вам важны захватывающий сюжет и правдоподобные личности персонажей?",
          "q3": "Хотите, чтобы модель запоминала детали персонажа и оставалась в нем на протяжении долгих сеансов?"
        },
        "speed": {
          "title": "Скорость и задержка",
          "description": "Более быстрые модели лучше подходят для болтливых и двусторонних разговоров. ",
          "q1": "Хотите, чтобы ответы были практически мгновенными, чтобы ролевая игра протекала естественно?",
          "q2": "Вы снимаете сцены с быстрыми диалогами, в которых ожидание нарушает погружение?",
          "q3": "Это для казуальных ролевых игр, где быстрое движение вперед-назад важнее, чем идеальные ответы?"
        },
        "budget": {
          "title": "Бюджет и использование",
          "description": "Каждый провайдер выставляет счета за токен. ",
          "q1": "Вы согласны платить больше за более богатое взаимодействие с персонажами или вам нужно что-то дешевое за ежедневную RP?",
          "q2": "Есть ли у вас бесплатные модели от вашего провайдера/маршрутизатора, которые вы можете опробовать в первую очередь?",
          "q3": "Будете ли вы проводить длительные ролевые игры с подробным описанием сцен?",
          "q4": "У вас есть жесткий ежемесячный бюджет, который вы не хотите превышать?"
        },
        "safety": {
          "title": "Безопасность, конфиденциальность и дополнительные возможности",
          "description": "Поставщики различаются по способам обеспечения безопасности, ведения журналов и дополнительных функций, таких как изображения, инструменты или длинные контекстные окна.",
          "q1": "Нужно ли вам меньше фильтров контента для зрелых или творческих сценариев ролевых игр?",
          "q2": "Вас волнует, записываются ли ваши частные разговоры с РП или используются для обучения?",
          "q3": "Нужны ли вам длинные контекстные окна для сложных сюжетных линий и историй персонажей?"
        }
      },
      "where": {
        "title": "Где я могу найти модели?",
        "intro": "У большинства провайдеров и маршрутизаторов есть <0>список или каталог моделей</0>. ",
        "directTitle": "Прямые поставщики",
        "directDesc": "OpenAI, Anthropic, Google Gemini, xAI, Mistral и т. д. Каждый из них имеет консоль/панель мониторинга, где вы можете увидеть официальные названия моделей, возможности и цены.",
        "routersTitle": "Маршрутизаторы и концентраторы",
        "routersDesc": "Такие сервисы, как OpenRouter или другие агрегаторы, содержат множество моделей от разных поставщиков в одном месте, часто с тестами и сравнением цен.",
        "communityTitle": "Рекомендации сообщества",
        "communityDesc": "Просмотрите документы, блоги или сообщения сообщества вашего провайдера/маршрутизатора. "
      },
      "rules": {
        "title": "Простые практические правила",
        "casual": "Для обычного общения: выберите быструю и дешевую модель чата у своего провайдера/маршрутизатора.",
        "experiments": "Для экспериментов или большого объема: начните с самой дешевой модели, которая кажется достаточно хорошей, затем обновите ее, если необходимо.",
        "switch": "Если что-то вам не нравится (слишком медленно/слишком глупо/слишком дорого): вы всегда можете переключить модель позже в LettuceAI."
      },
      "disclaimer": "Всегда проверяйте документацию поставщика, чтобы узнать последний список моделей, ограничения и цены. "
    },
    "whereToFind": {
      "badge": "Справка по API-ключу",
      "intro": "Выполните следующие действия, чтобы получить ключ API, затем вернитесь в LettuceAI и вставьте его в настройки провайдера.",
      "readyPrompt": "Готовы получить ключ?",
      "openProviderSite": "Открыть сайт поставщика",
      "keyWarning": "Никогда не делитесь публично своим ключом API. ",
      "stuckPrompt": "Все еще не можете разобраться?",
      "joinDiscord": "Присоединяйтесь к нашему серверу Discord для помощи.",
      "guides": {
        "chutes": {
          "title": "Как найти свой ключ API Chutes",
          "s1": "Перейдите на сайт chutes.ai/app и войдите в систему.",
          "s2": "Откройте свою учетную запись/область настроек и найдите ключи API.",
          "s3": "Создайте новый ключ (или скопируйте существующий).",
          "s4": "Вставьте ключ в LettuceAI."
        },
        "openai": {
          "title": "Как найти ключ API OpenAI",
          "s1": "Перейдите на платформу.openai.com и войдите в систему.",
          "s2": "Нажмите аватар своего профиля в правом верхнем углу, затем выберите ключи API.",
          "s3": "Нажмите «Создать новый секретный ключ» и скопируйте показанное значение.",
          "s4": "Вставьте ключ в LettuceAI и сохраните его в безопасном месте. "
        },
        "anthropic": {
          "title": "Как найти свой ключ Anthropic API",
          "s1": "Перейдите на console.anthropic.com и войдите в систему.",
          "s2": "Откройте настройки на левой боковой панели.",
          "s3": "Выберите ключи API и нажмите «Создать ключ».",
          "s4": "Скопируйте ключ и вставьте его в LettuceAI."
        },
        "openrouter": {
          "title": "Как найти ключ API OpenRouter",
          "s1": "Посетите openrouter.ai и войдите в систему.",
          "s2": "Откройте страницу ключей из ",
          "s3": "Нажмите «Создать ключ», дайте ему имя и сохраните.",
          "s4": "Скопируйте ключ и вставьте его в LettuceAI."
        },
        "mistral": {
          "title": "Как найти ключ API Mistral",
          "s1": "Перейдите на console.mistral.ai и войдите в систему.",
          "s2": "Нажмите ",
          "s3": "Нажмите «Создать ключ API».",
          "s4": "Скопируйте ключ и вставьте его в LettuceAI."
        },
        "deepseek": {
          "title": "Как найти ключ API DeepSeek",
          "s1": "Откройте платформу.deepseek.com и войдите в систему.",
          "s2": "Нажмите «Ключи API» в ",
          "s3": "Создайте новый ключ, если у вас его еще нет.",
          "s4": "Скопируйте ключ и вставьте его в LettuceAI."
        },
        "groq": {
          "title": "Как найти ключ Groq API",
          "s1": "Посетите console.groq.com и войдите в систему.",
          "s2": "Открыть API ",
          "s3": "Создайте новый ключ, затем скопируйте его.",
          "s4": "Вставьте ключ в LettuceAI."
        },
        "gemini": {
          "title": "Как найти ключ Google Gemini API",
          "s1": "Перейдите в Google AI Studio на aistudio.google.com и войдите в систему.",
          "s2": "Нажмите «Получить». ",
          "s3": "При необходимости создайте новый ключ.",
          "s4": "Скопируйте ключ и вставьте его в LettuceAI."
        },
        "xai": {
          "title": "Как найти ключ API xAI",
          "s1": "Откройте console.x.ai и войдите в систему.",
          "s2": "Перейдите в раздел «Ключи API» в ",
          "s3": "Создайте новый ключ.",
          "s4": "Скопируйте ключ и вставьте его в LettuceAI."
        },
        "zai": {
          "title": "Как найти ключ API zAI (GLM)",
          "s1": "Перейдите на open.bigmodel.cn и войдите в систему.",
          "s2": "Откройте Центр пользователя, ",
          "s3": "Создайте новый ключ, если у вас его нет.",
          "s4": "Скопируйте ключ и вставьте его в LettuceAI."
        },
        "moonshot": {
          "title": "Как найти ключ API Moonshot (Kimi)",
          "s1": "Посетите платформу.moonshot.cn и войдите в систему.",
          "s2": "Откройте раздел «Ключи API» в консоли.",
          "s3": "Создайте новый ключ и скопируйте его.",
          "s4": "Вставьте ключ в LettuceAI."
        },
        "qwen": {
          "title": "Как найти ключ API Qwen",
          "s1": "Откройте Dashscope.aliyun.com и войдите в систему.",
          "s2": "Перейдите в раздел «Ключи API» в ",
          "s3": "Создайте новый ключ.",
          "s4": "Скопируйте ключ и вставьте его в LettuceAI."
        },
        "nanogpt": {
          "title": "Как найти ключ API NanoGPT",
          "s1": "Перейдите на сайт nano-gpt.com и войдите в систему.",
          "s2": "Откройте панель управления и перейдите к API ",
          "s3": "При необходимости создайте новый ключ.",
          "s4": "Скопируйте ключ и вставьте его в LettuceAI."
        },
        "featherless": {
          "title": "Как найти ключ API Featherless",
          "s1": "Посетите сайт Featherless.ai и войдите в систему.",
          "s2": "Откройте свою учетную запись или раздел API на ",
          "s3": "Создайте новый ключ, если вы его не видите.",
          "s4": "Скопируйте ключ и вставьте его в LettuceAI."
        },
        "anannas": {
          "title": "Как найти ключ API Anannas",
          "s1": "Перейдите на Dashboard.anannas.ai и войдите в систему.",
          "s2": "Перейдите в раздел «Ключи API».",
          "s3": "Создайте новый ключ и скопируйте его.",
          "s4": "Вставьте ключ в LettuceAI."
        },
        "default": {
          "title": "Как найти ключ API",
          "s1": "Откройте панель управления поставщиком ИИ в браузере и войдите в систему.",
          "s2": "Найдите настройки API, Разработчика или Интеграции.",
          "s3": "Создайте новый ключ API или просмотрите существующий.",
          "s4": "Скопируйте ключ и вставьте его в LettuceAI."
        }
      }
    },
    "welcomeFooter": {
      "setupTimeMobile": "Установка занимает менее 2 минут"
    }
  },
  "search": {
    "placeholder": "Поиск...",
    "tabs": {
      "characters": "Персонажи",
      "personas": "Персоны"
    },
    "noResults": "{{type}} не найдены",
    "emptyState": "{{type}} пока нет",
    "noResultsHint": "Попробуйте другой поисковый запрос",
    "emptyCharacters": "Создайте первого персонажа, чтобы начать общение",
    "emptyPersonas": "Создайте персону в настройках",
    "a11y": {
      "goBack": "Вернуться",
      "clearSearch": "Очистить поиск",
      "characterAvatar": "{{name}} аватар"
    },
    "session": {
      "newChatTitle": "Новый чат"
    },
    "noDescription": "Нет описания",
    "defaultBadge": "По умолчанию"
  },
  "sync": {
    "modes": {
      "join": "Подключиться",
      "joinDesc": "Подключиться к хосту",
      "host": "Хост",
      "hostDesc": "Поделиться данными"
    },
    "sections": {
      "mode": "Режим",
      "connectToHost": "Подключиться к хосту",
      "startHosting": "Начать хостинг",
      "status": "Статус",
      "hosting": "Служба хостинга",
      "localAddress": "Адрес локальной сети",
      "connectionPin": "PIN подключения",
      "setupGuide": "Руководство по настройке"
    },
    "fields": {
      "hostAddress": "Адрес хоста или JSON",
      "hostPlaceholder": "напр. 192.168.1.100:12345",
      "pinCode": "PIN-код",
      "pinPlaceholder": "000000"
    },
    "buttons": {
      "scanQRCode": "Сканировать QR-код",
      "connect": "Подключить",
      "connecting": "Подключение...",
      "startHosting": "Начать хостинг",
      "startingServer": "Запуск сервера...",
      "stopHosting": "Остановить хостинг",
      "hostAgain": "Запустить снова",
      "done": "Готово"
    },
    "status": {
      "connecting": "Подключение...",
      "connected": "Подключено",
      "waitingConfirmation": "Ожидание подтверждения",
      "waitingConfirmationDesc": "Подтвердите подключение на главном устройстве, чтобы продолжить.",
      "syncing": "Синхронизация...",
      "transferringData": "Передача данных",
      "syncInProgress": "Синхронизация в процессе",
      "live": "Активно",
      "broadcasting": "Трансляция",
      "clientsLabel": "Подключено",
      "clientsUnit": "Клиентов"
    },
    "pinDescription": "Поделитесь этим PIN-кодом с подключаемым устройством",
    "hostingDesc1": "Другие устройства могут подключаться и синхронизировать данные с этого устройства.",
    "hostingDesc2": "Ваши данные будут доступны подключённым клиентам.",
    "setupSteps": {
      "step1": "Откройте приложение на другом устройстве",
      "step2": "Перейдите в Настройки → Локальная синхронизация",
      "step3": "Отсканируйте QR-код или введите адрес"
    },
    "messages": {
      "completed": "Синхронизация завершена!",
      "completedDesc": "Все данные синхронизированы",
      "error": "Ошибка подключения",
      "outdatedClient": "Обнаружен устаревший клиент"
    },
    "disclaimer": "Синхронизация работает через локальную сеть. Оба устройства должны быть в одной Wi-Fi сети.",
    "modals": {
      "connectionRequest": "Запрос на подключение",
      "requestMessage": "хочет синхронизироваться с этим устройством.",
      "acceptConnection": "Принять подключение",
      "acceptDesc": "Разрешить этому устройству синхронизировать данные",
      "decline": "Отклонить",
      "declineDesc": "Заблокировать эту попытку подключения",
      "readyToSync": "Готово к синхронизации",
      "connectionEstablished": "Соединение установлено",
      "deviceReady": "готово.",
      "startSyncMessage": "Нажмите ниже, чтобы начать синхронизацию данных.",
      "startSyncing": "Начать синхронизацию",
      "startSyncingDesc": "Начать передачу данных сейчас"
    },
    "scanner": {
      "title": "Сканировать QR-код",
      "cancel": "Отменить сканирование"
    },
    "unknownDevice": "Неизвестное устройство",
    "aria": {
      "dismissStatus": "Отключить статус синхронизации",
      "dismissError": "Отклонить ошибку синхронизации"
    },
    "stats": {
      "statusLabel": "Статус"
    }
  },
  "creationHelper": {
    "page": {
      "info": "Помощник создания проведёт вас через создание персонажей с помощью ИИ. Настройте модель и инструменты, используемые при создании персонажей.",
      "modelConfiguration": "Настройка модели",
      "chatModel": "Модель чата",
      "selectedModel": "Выбранная модель",
      "useAppDefault": "Использовать модель по умолчанию{{model}}",
      "useAppDefaultBase": "Использовать модель по умолчанию",
      "noModelsAvailable": "Нет доступных моделей",
      "chatModelDescription": "ИИ-модель для разговоров при создании персонажей",
      "streamingOutput": "Потоковый вывод",
      "streamingDescription": "Показывать ответы по мере генерации",
      "imageGenerationModel": "Модель генерации изображений",
      "noModelSelected": "Модель не выбрана",
      "noImageModelsAvailable": "Нет доступных моделей изображений",
      "imageModelDescription": "Для генерации аватаров персонажей",
      "toolSelection": "Выбор инструментов",
      "smartToolSelection": "Умный выбор инструментов",
      "smartToolDescription": "ИИ автоматически выбирает, какие инструменты использовать",
      "smartToolEnabledHint": "Когда включено, ИИ-помощник спрашивает, что вы хотите создать, и загружает только соответствующий набор инструментов.",
      "smartToolDisabledHint": "Когда отключено, ИИ-помощник открывается напрямую и использует все включённые инструменты; ассистент сам решает, что создавать.",
      "quickPresets": "Быстрые пресеты",
      "customSelection": "Пользовательский выбор — {{count}} инструментов включено",
      "footerInfo": "Когда умный выбор инструментов включён, ИИ решает, какие инструменты использовать, исходя из контекста. Отключите его, чтобы вручную управлять доступными инструментами.",
      "selectChatModel": "Выбрать модель чата",
      "selectImageModel": "Выбрать модель изображений",
      "searchModels": "Поиск моделей..."
    },
    "categories": {
      "basic": "Базовые",
      "content": "Контент",
      "visual": "Визуальные",
      "settings": "Настройки",
      "flow": "Поток",
      "persona": "Персоны",
      "lorebook": "Лорбуки"
    },
    "presets": {
      "all": {
        "name": "Все инструменты",
        "desc": "Включить все доступные инструменты"
      },
      "essential": {
        "name": "Основные",
        "desc": "Только имя, определение и сцены"
      },
      "minimal": {
        "name": "Минимальные",
        "desc": "Только имя и определение"
      }
    },
    "tools": {
      "setName": "Задать имя",
      "setNameDesc": "Задать имя персонажа",
      "setDefinition": "Задать определение",
      "setDefinitionDesc": "Задать личность и предысторию",
      "set_character_name": {
        "name": "Задать имя",
        "desc": "Задать имя персонажа"
      },
      "set_character_definition": {
        "name": "Задать определение",
        "desc": "Задать личность и предысторию"
      },
      "add_scene": {
        "name": "Добавить сцену",
        "desc": "Добавить начальную сцену для ролевой игры"
      },
      "update_scene": {
        "name": "Обновить сцену",
        "desc": "Изменить существующую сцену"
      },
      "toggle_avatar_gradient": {
        "name": "Градиент аватара",
        "desc": "Переключить наложение градиента на аватар"
      },
      "set_default_model": {
        "name": "Выбрать модель",
        "desc": "Выбрать AI-модель для разговоров"
      },
      "set_system_prompt": {
        "name": "Системный промпт",
        "desc": "Задать правила поведения"
      },
      "get_system_prompt_list": {
        "name": "Список промптов",
        "desc": "Просмотр доступных промптов"
      },
      "get_model_list": {
        "name": "Список моделей",
        "desc": "Просмотр доступных моделей"
      },
      "use_uploaded_image_as_avatar": {
        "name": "Изображение как аватар",
        "desc": "Использовать загруженное изображение как аватар"
      },
      "use_uploaded_image_as_chat_background": {
        "name": "Изображение как фон",
        "desc": "Использовать загруженное изображение как фон"
      },
      "generate_image": {
        "name": "Сгенерировать изображение",
        "desc": "Сгенерировать изображение с помощью AI-модели"
      },
      "show_preview": {
        "name": "Показать предпросмотр",
        "desc": "Предпросмотр персонажа"
      },
      "request_confirmation": {
        "name": "Запрос подтверждения",
        "desc": "Спросить о сохранении или продолжении"
      },
      "list_personas": {
        "name": "Список персон",
        "desc": "Просмотр персон"
      },
      "upsert_persona": {
        "name": "Сохранить персону",
        "desc": "Создать или обновить персону"
      },
      "use_uploaded_image_as_persona_avatar": {
        "name": "Аватар персоны",
        "desc": "Использовать загруженное изображение как аватар персоны"
      },
      "delete_persona": {
        "name": "Удалить персону",
        "desc": "Удалить персону"
      },
      "get_default_persona": {
        "name": "Персона по умолчанию",
        "desc": "Получить персону по умолчанию"
      },
      "list_lorebooks": {
        "name": "Список лорбуков",
        "desc": "Просмотр лорбуков"
      },
      "upsert_lorebook": {
        "name": "Сохранить лорбук",
        "desc": "Создать или обновить лорбук"
      },
      "delete_lorebook": {
        "name": "Удалить лорбук",
        "desc": "Удалить лорбук"
      },
      "list_lorebook_entries": {
        "name": "Список записей",
        "desc": "Просмотр записей лорбука"
      },
      "get_lorebook_entry": {
        "name": "Получить запись",
        "desc": "Получить запись лорбука"
      },
      "upsert_lorebook_entry": {
        "name": "Сохранить запись",
        "desc": "Создать или обновить запись"
      },
      "delete_lorebook_entry": {
        "name": "Удалить запись",
        "desc": "Удалить запись лорбука"
      },
      "create_blank_lorebook_entry": {
        "name": "Пустая запись",
        "desc": "Создать запись-заглушку"
      },
      "reorder_lorebook_entries": {
        "name": "Упорядочить записи",
        "desc": "Изменить порядок записей"
      },
      "list_character_lorebooks": {
        "name": "Лорбуки персонажа",
        "desc": "Просмотр лорбуков для персонажа"
      },
      "set_character_lorebooks": {
        "name": "Назначить лорбуки персонажу",
        "desc": "Привязать лорбуки к персонажу"
      },
      "addScene": "Добавить сцену",
      "addSceneDesc": "Добавить стартовую сцену для ролевой игры",
      "updateScene": "Обновить сцену",
      "updateSceneDesc": "Изменить существующую сцену",
      "avatarGradient": "Градиент аватара",
      "avatarGradientDesc": "Переключить наложение градиента на аватар",
      "setModel": "Установить модель",
      "setModelDesc": "Настроить модель искусственного интеллекта для разговоров",
      "systemPrompt": "Системная подсказка",
      "systemPromptDesc": "Установить рекомендации по поведению",
      "listPrompts": "Список подсказок",
      "listPromptsDesc": "Просмотреть доступные подсказки",
      "listModels": "Список моделей",
      "listModelsDesc": "Просмотреть доступные модели",
      "imageAsAvatar": "Изображение как аватар",
      "imageAsAvatarDesc": "Использовать загруженное изображение в качестве аватара"
    }
  },
  "tour": {
    "stepCounter": "Шаг {{current}} из {{total}}",
    "skipTour": "Пропустить тур",
    "next": "Далее",
    "gotIt": "Понятно",
    "appShell": {
      "chats": {
        "title": "Здесь живут ваши чаты",
        "body": "Все ваши личные разговоры с персонажами находятся здесь. Возвращайтесь в любое время, и мы сохраним ваше место."
      },
      "groups": {
        "title": "Групповые чаты",
        "body": "Соберите нескольких персонажей в одной комнате и наблюдайте, как они общаются, или присоединяйтесь сами, когда захотите."
      },
      "discover": {
        "title": "Найдите новых персонажей",
        "body": "Просматривайте то, чем поделилось сообщество, и забирайте любого приглянувшегося персонажа. Новые фавориты в одно касание."
      },
      "library": {
        "title": "Ваша личная библиотека",
        "body": "Всё, что вы создали или сохранили, живёт здесь: персонажи, персоны, промпты — всё. Считайте это вашим хранилищем."
      },
      "settings": {
        "title": "Настройте под себя",
        "body": "Меняйте провайдеров, выбирайте модели, настраивайте внешний вид приложения. Практически всё регулируется в настройках."
      },
      "search": {
        "title": "Найдите что угодно быстро",
        "body": "Ищете конкретный чат или персонажа? Ищите по всему отсюда. Никакого копания."
      },
      "create": {
        "title": "И наконец, создавайте!",
        "body": "Нажмите плюс, когда придёт вдохновение. Создайте нового персонажа, персону или начните что-то с нуля."
      }
    },
    "chatDetail": {
      "chatTitle": {
        "title": "Настройки для каждого чата",
        "body": "Нажмите на имя персонажа вверху, чтобы открыть настройки именно для этого чата. Разные персоны, раскладки и модели для каждого разговора."
      },
      "chatMemory": {
        "title": "Что они помнят",
        "body": "Значок мозга показывает, что ваш персонаж помнит о ваших разговорах. Нажмите для просмотра, редактирования или очистки воспоминаний."
      },
      "chatSearch": {
        "title": "Найдите ту самую строку",
        "body": "Поиск только в этом разговоре. Отлично подходит для поиска детали 200 сообщений назад без бесконечной прокрутки."
      },
      "chatLorebook": {
        "title": "Записи лорбука",
        "body": "Дополнительные факты, мироустройство и контекст, которые вставляются в промпт при появлении определённых ключевых слов. Шпаргалка вашего персонажа."
      },
      "chatPlus": {
        "title": "Прикрепляйте вложения",
        "body": "Добавьте изображения или откройте меню дополнений. Всё, что вы прикрепите, отправится вместе с вашим следующим сообщением."
      },
      "chatComposer": {
        "title": "Ваше сообщение, ваш ход",
        "body": "Пишите здесь. Enter отправляет, Shift+Enter — новая строка. Совет: долгое нажатие на любое сообщение для редактирования, ветвления или удаления."
      },
      "chatSend": {
        "title": "Одна кнопка, четыре функции",
        "body": "Кнопка отправки меняет свою функцию в зависимости от ситуации:"
      }
    },
    "postFirstMessage": {
      "chatRegenerate": {
        "title": "Не нравится? Перегенерируйте",
        "body": "Нажмите значок обновления, чтобы получить совершенно новый ответ от персонажа. Каждая перегенерация сохраняется как вариант."
      },
      "chatVariants": {
        "title": "Листайте между вариантами",
        "body": "После перегенерации вы увидите счётчик вариантов под сообщением. Проведите влево или вправо по пузырю сообщения, чтобы просмотреть все варианты ответов."
      },
      "chatLongPress": {
        "title": "Здесь скрыто ещё кое-что",
        "body": "Долгое нажатие на любое сообщение для редактирования, копирования, ветвления, закрепления, удаления или отката разговора. На десктопе работает правый клик."
      }
    },
    "sendButton": {
      "continue": {
        "label": "Продолжить",
        "desc": "Поле ввода пусто. Нажмите, чтобы персонаж продолжил говорить."
      },
      "send": {
        "label": "Отправить",
        "desc": "Вы набрали или прикрепили что-то. Нажмите для отправки."
      },
      "sending": {
        "label": "Отправка",
        "desc": "Ответ в пути. Кнопка заблокирована."
      },
      "stop": {
        "label": "Стоп",
        "desc": "Нажмите для отмены во время ответа, если передумаете."
      }
    },
    "extra": {
      "rerunOnboarding": "Повторно выполнить подключение"
    }
  },
  "sessionAdvanced": {
    "title": "Параметры сеанса",
    "subtitle": "Переопределить настройки модели для этого разговора",
    "goBack": "Назад",
    "support": "Поддержка",
    "reset": "Сбросить",
    "save": "Сохранить",
    "noSessionWarning": "Откройте сеанс чата, чтобы настроить параметры для конкретного сеанса.",
    "overrideDefaults": "Переопределить по умолчанию",
    "overrideDefaultsDesc": "Настройте параметры только для этого разговора",
    "loadingContextInfo": "Загрузка информации о контексте...",
    "sampling": {
      "title": "Сэмплирование",
      "temperature": "Температура",
      "temperatureDesc": "Контролирует случайность. Ниже = более детерминированно, выше = более креативно.",
      "temperaturePrecise": "Точно",
      "temperatureCreative": "Креативно",
      "topP": "Top P",
      "topPDesc": "Nucleus sampling. Ограничивает токены кумулятивной вероятностью.",
      "topPFocused": "Фокусировано",
      "topPDiverse": "Разнообразно",
      "topK": "Top K",
      "topKDesc": "Ограничивает сэмплирование top K наиболее вероятными токенами."
    },
    "outputPenalties": {
      "title": "Вывод и штрафы",
      "maxOutputTokens": "Макс. токенов вывода",
      "maxOutputTokensDesc": "Максимальная длина ответа. Auto позволяет модели решить.",
      "auto": "Авто",
      "custom": "Пользовательский",
      "frequencyPenalty": "Штраф за частоту",
      "frequencyPenaltyDesc": "Уменьшает повторение последовательностей токенов.",
      "frequencyPenaltyRepeat": "Повтор",
      "frequencyPenaltyVary": "Разнообразие",
      "presencePenalty": "Штраф за присутствие",
      "presencePenaltyDesc": "Поощряет исследование новых тем.",
      "presencePenaltyRepeat": "Повтор",
      "presencePenaltyExplore": "Исследование"
    },
    "performance": {
      "title": "Производительность",
      "gpuLayers": "Слои графического процессора",
      "gpuLayersDesc": "Слои, выгруженные на GPU. 0 = только CPU.",
      "threads": "Потоки",
      "threadsDesc": "Потоки CPU для инференса.",
      "batchThreads": "Пакетные потоки",
      "batchThreadsDesc": "Потоки CPU для пакетной обработки.",
      "batchSize": "Размер пакета",
      "batchSizeDesc": "Размер блока обработки промпта.",
      "contextLength": "Длина контекста",
      "contextLengthDesc": "Переопределить размер окна контекста.",
      "flashAttention": "Flash Внимание",
      "flashAttentionDesc": "Оптимизация памяти GPU.",
      "enabled": "Включено",
      "disabled": "Отключено"
    },
    "samplingMemory": {
      "title": "Сэмплирование и память",
      "minP": "Min P",
      "minPDesc": "Минимальный порог вероятности.",
      "typicalP": "Типичный P",
      "typicalPDesc": "Порог типичного сэмплирования.",
      "seed": "Seed",
      "seedDesc": "Случайный seed. Оставьте пустым для случайного.",
      "ropeBase": "RoPE Base",
      "ropeBaseDesc": "Переопределение базы частоты.",
      "ropeScale": "Масштаб RoPE",
      "ropeScaleDesc": "Переопределение масштаба частоты.",
      "kvCacheType": "KV Тип кэша",
      "kvCacheTypeDesc": "Квантизация KV cache для экономии VRAM.",
      "offloadKqv": "Разгрузка KQV",
      "offloadKqvDesc": "KV cache и операции KQV на GPU.",
      "on": "Вкл",
      "off": "Выкл",
      "samplerProfile": "Профиль сэмплера",
      "samplerProfileDesc": "Настроенные значения по умолчанию для стабильности или рассуждений.",
      "balanced": "Сбалансированный",
      "creative": "Креативный",
      "stable": "Стабильный",
      "reasoning": "Рассуждение"
    },
    "systemInfo": {
      "title": "Системная информация",
      "maxContext": "Макс. контекст",
      "recommended": "Рекомендуемый",
      "availableRam": "Доступная RAM",
      "availableVram": "Доступная VRAM",
      "modelSize": "Размер модели"
    }
  },
  "exportMenu": {
    "title": "Формат экспорта",
    "selectFormat": "Выберите формат",
    "uscTitle": "Карта единой системы",
    "formats": {
      "uscPrompt": "Портативный экспорт USC для шаблонов промптов.",
      "uscLorebook": "Портативный экспорт USC для лорбуков.",
      "uscModel": "Портативный экспорт USC для профилей моделей.",
      "uscChatTemplate": "Портативный экспорт USC для шаблонов чатов.",
      "legacyPromptJson": "Подсказка прежней версии JSON",
      "legacyPromptJsonDesc": "Текущий формат внешнего пакета промптов.",
      "lorebookJson": "Lorebook JSON",
      "lorebookJsonDesc": "Текущий формат экспорта лорбуков.",
      "modelJson": "Модель JSON",
      "modelJsonDesc": "Безопасный JSON профиля модели без учётных данных.",
      "chatTemplateJson": "Шаблон чата JSON",
      "chatTemplateJsonDesc": "Нативный формат экспорта шаблонов чатов."
    },
    "extra": {
      "selectFormat": "Выберите формат",
      "exportFormatTitle": "Формат экспорта",
      "uscDesc": "Портативный экспорт USC",
      "legacyJsonDesc": "Устаревший формат экспорта JSON",
      "formatV3Desc": "Экспорт карты символов V3",
      "formatV2Desc": "Экспорт карты символов V2",
      "formatV1Desc": "Экспорт карты символов V1",
      "uscLorebook": "Карта единой системы (USC)",
      "portableLorebook": "Портативный экспорт USC для книг знаний",
      "lorebookJson": "Lorebook JSON",
      "currentLorebook": "Текущий формат экспорта книги знаний",
      "uscModel": "Unified System Card (USC)",
      "portableModel": "Портативный экспорт USC для профилей моделей",
      "modelJson": "Модель JSON",
      "safeModel": "Профиль безопасной модели JSON без учетных данных",
      "uscTemplate": "Карта единой системы (USC)",
      "portableTemplate": "Портативный экспорт USC для шаблонов чата",
      "templateJson": "Шаблон чата JSON",
      "nativeTemplate": "Нативный формат экспорта шаблонов чата"
    }
  },
  "designReference": {
    "title": "Референсы дизайна",
    "description": "Загрузите несколько чётких референсных изображений и одно каноническое визуальное описание.",
    "descriptionPlaceholder": "Опишите стабильный облик: лицо, волосы, телосложение, возрастная подача, элементы одежды, аксессуары и направление стиля.",
    "addReferences": "Добавить референсы",
    "visualDescription": "Визуальное описание",
    "draftWithAi": "Черновик с AI",
    "referenceImages": "Референсные изображения",
    "imageAlt": "Референс дизайна {{index}}",
    "loading": "Загрузка...",
    "removeAria": "Удалить референс дизайна",
    "noImages": "Референсных изображений пока нет",
    "imageCount": "{{count}} референсное изображение(я) прикреплено",
    "emptyReferences": "Добавьте несколько чётких референсных фото, чтобы зафиксировать лицо, пропорции, одежду и стиль.",
    "noWriterModel": "Сначала добавьте совместимую модель scene writer в настройках генерации изображений.",
    "noImagesForGeneration": "Добавьте аватар или хотя бы одно референсное изображение перед генерацией.",
    "writerModelHelp": "Использует {{model}} для создания черновика на основе вашего аватара и референсных изображений.",
    "noWriterModelHelp": "Добавьте совместимую модель scene writer в настройках генерации изображений для автоматического создания черновика.",
    "draftMenuTitle": "AI-черновик дизайна",
    "draftMenuDesc": "Черновик создан {{model}} на основе текущего аватара и референсных изображений.",
    "draftMenuNoWriter": "Добавьте совместимую модель scene writer перед использованием этого помощника.",
    "regenerate": "Перегенерировать",
    "useThis": "Использовать"
  },
  "samplerOrder": {
    "title": "Порядок сэмплера",
    "description": "Перетаскивайте этапы для изменения порядка. Выполняются сверху вниз.",
    "reset": "Сбросить",
    "resetAria": "Сбросить порядок сэмплера на значения по умолчанию",
    "stages": {
      "penalties": {
        "label": "Штрафы",
        "desc": "Применить штрафы за частоту и присутствие перед фильтрацией."
      },
      "grammar": {
        "label": "Грамматика",
        "desc": "Ограничить токены при активной грамматике нативного шаблона чата."
      },
      "topK": {
        "label": "Top K",
        "desc": "Сократить пул кандидатов до сильнейших токенов."
      },
      "topP": {
        "label": "Top P",
        "desc": "Применить ядерную фильтрацию к оставшемуся распределению."
      },
      "minP": {
        "label": "Min P",
        "desc": "Отбросить маловероятные хвостовые токены с минимальным порогом."
      },
      "typical": {
        "label": "Типичный P",
        "desc": "Предпочитать статистически типичные токены в текущем распределении."
      },
      "temp": {
        "label": "Температура",
        "desc": "Сгладить или заострить финальное распределение перед выбором."
      }
    },
    "presets": {
      "default": {
        "label": "По умолчанию",
        "hint": "Lettuce local"
      },
      "unsloth": {
        "label": "Unsloth",
        "hint": "llama.cpp style"
      },
      "focused": {
        "label": "Фокусированный",
        "hint": "Жёсткая обрезка"
      },
      "creative": {
        "label": "Креативный",
        "hint": "Поздний фильтр"
      }
    }
  },
  "lorebookGen": {
    "flow": {
      "pickerCharactersTitle": "Символы",
      "pickerSessionsTitle": "Сеансы",
      "noCharacters": "Нет символов",
      "noSessions": "Нет сеансов",
      "clearSelection": "Очистить выбор",
      "directionTitle": "Необязательное направление генерации",
      "directionLabel": "Направление",
      "toggleForceMode": "Переключить принудительный режим",
      "entryTitlePlaceholder": "Заголовок",
      "entryContentPlaceholder": "Содержание записи в книге знаний",
      "editDirectionBeforeRegenerate": "Изменить направление перед регенерацией",
      "generatorReturnedNoDraft": "Генератор не возвратил черновик",
      "pageTitle": "Создать книгу знаний ",
      "missingContext": "Отсутствует контекст книги знаний для страницы генератора.",
      "characterLocked": "Персонаж привязан к владельцу этой книги знаний",
      "chooseSession": "Выбрать сеанс",
      "pickCharacter": "Выберите персонажа",
      "searchMemories": "Поиск воспоминаний",
      "searchMessages": "Поиск сообщений",
      "selectLastN": "Выбрать последние {{n}} сообщений",
      "selectAll": "Выбрать все",
      "loadSessionPrompt": "Выбрать сеанс для загрузки",
      "messagesText": "messages",
      "memoriesText": "memories",
      "messagesAndMemories": "сообщений и воспоминаний",
      "titleAndContentRequired": "Требуется заголовок и содержимое",
      "keywordsOrAlwaysActive": "Добавить хотя бы одно ключевое слово или включить Всегда активно",
      "lorybookEntrySaved": "Запись в книге знаний сохранена",
      "saveFailed": "Сохранить не удалось",
      "generationFailed": "Ошибка генерации",
      "failedToLoadContext": "Не удалось загрузить ",
      "failedToLoadSessions": "Не удалось загрузить сеансы",
      "failedToLoadMessages": "Не удалось загрузить сообщения",
      "userRole": "USER",
      "aiRole": "AI"
    },
    "triggerPreview": {
      "resizeInspector": "Инспектор изменения размера"
    }
  },
  "companion": {
    "download": {
      "emotionClassifier": "Классификатор эмоций",
      "emotionClassifierDesc": "Считывает ходы и обновляет векторы чувств, выраженных и заблокированных эмоций компаньона.",
      "emotionSize": "~120 МБ",
      "entityExtractor": "Entity Extractor (NER)",
      "entityExtractorDesc": "Идентифицирует людей, места и объекты, чтобы воспоминания можно было канонизировать и связать.",
      "entitySize": "~140 МБ",
      "memoryRouter": "Маршрутизатор памяти",
      "memoryRouterDesc": "Решает, следует ли сохранять новые ходы в виде отношений, вех, эпизодов или других категорий памяти.",
      "routerSize": "~70 МБ",
      "unknownModel": "Неизвестная сопутствующая модель. "
    }
  },
  "voices": {
    "extra": {
      "customVoices": "Мои голоса",
      "providerVoices": "Голоса провайдера",
      "myVoices": "Мои голоса",
      "page": {
        "noAudioProvidersHint": "Добавьте провайдера в Провайдеры → Аудио",
        "noVoicesTitle": "Голоса ещё не созданы",
        "noVoicesDescription": "Создайте голоса с пользовательскими промптами для персонажей",
        "addProviderFirst": "Сначала добавьте аудиопровайдера",
        "noPrompt": "Нет промпта",
        "noProviderVoices": "Голоса не найдены. Нажмите «Обновить», чтобы загрузить.",
        "showLess": "Показать меньше",
        "showAllVoices": "Показать все {{count}} голоса",
        "voiceFallbackTitle": "Голос"
      },
      "cache": {
        "section": "Кеш аудио",
        "title": "Кеш TTS-аудио",
        "description": "Сгенерированный голос кешируется, чтобы сократить повторную генерацию",
        "clearing": "Очистка...",
        "clear": "Очистить кеш"
      },
      "menu": {
        "editDescription": "Изменить этот голос",
        "deleteDescription": "Удалить этот голос",
        "provider": "Провайдер",
        "category": "Категория",
        "createVoiceConfig": "Создать конфигурацию голоса",
        "createVoiceConfigDescription": "Использовать этот голос с пользовательскими настройками"
      },
      "editor": {
        "editTitle": "Редактировать голос",
        "createTitle": "Создать голос",
        "voiceName": "Название голоса",
        "voiceNamePlaceholder": "Голос персонажа",
        "provider": "Провайдер",
        "model": "Модель",
        "modelIdPlaceholder": "gpt-4o-mini-tts",
        "modelIdHint": "Введите точный идентификатор модели, который ожидает ваш совместимый эндпоинт",
        "elevenlabsVoice": "Голос ElevenLabs",
        "noVoicesAvailable": "Нет доступных голосов",
        "selectVoice": "Выберите голос...",
        "elevenlabsVoiceHint": "Выберите один из голосов ElevenLabs",
        "geminiVoice": "Голос Gemini",
        "geminiVoiceHint": "Выберите голос Gemini TTS",
        "voiceId": "Идентификатор голоса",
        "voiceIdPlaceholder": "alloy",
        "voiceIdHint": "Введите идентификатор голоса, поддерживаемый вашим совместимым эндпоинтом",
        "voicePrompt": "Промпт голоса",
        "voicePromptPlaceholder": "Тёплый, дружелюбный голос с жизнерадостной интонацией...",
        "voicePromptHint": "Опишите, как должен звучать голос",
        "exampleText": "Пример текста",
        "exampleTextPlaceholder": "Привет! Вот так я звучу при разговоре...",
        "exampleTextHint": "Образцовый текст для тестирования голоса",
        "voiceDesignChars": "{{current}}/{{minimum}} символов требуется для предпросмотра дизайна голоса",
        "defaultSample": "Привет! Вот так я звучу при разговоре. Я умею читать длинные тексты с теплотой, чёткостью и выражением, чтобы вы могли оценить тон и темп.",
        "playing": "Воспроизведение...",
        "previewVoice": "Предпросмотр голоса"
      },
      "kokoro": {
        "title": "Kokoro Studio",
        "newBlend": "Новая смесь",
        "editBlend": "Редактировать смесь",
        "tryText": "Привет! Это быстрый тест моего звучания.",
        "experimentDefaultText": "Привет! Вот так я звучу при разговоре. Я умею читать длинные тексты с теплотой, чёткостью и выражением.",
        "livePreview": "Предпросмотр в реальном времени",
        "savedBlend": "Сохранённая смесь",
        "defaultPreviewText": "Привет! Это быстрый предпросмотр звучания этого голоса.",
        "experiment": "Эксперимент",
        "providerNotFound": "Провайдер Kokoro не найден",
        "backToProviders": "Назад к провайдерам",
        "variantUnset": "Вариант не задан",
        "ready": "Готово",
        "modelNotInstalled": "Модель не установлена",
        "voiceCount": "{{count}} голос",
        "engineActions": "Действия с движком",
        "engineNotInstalled": "Движок не установлен",
        "installAtLeastOneVoice": "Установите хотя бы один голос",
        "continueSetup": "Продолжите настройку, чтобы установить модель Kokoro.",
        "pickVoiceOrStarter": "Выберите голос или стартовый пакет, чтобы начать.",
        "downloadsFailed": "{{count}} загрузок не удалось",
        "retryOrDismissAll": "Повторите отдельно или закройте все.",
        "dismissAll": "Закрыть все",
        "model": "Модель",
        "voice": "Голос",
        "downloads": "Загрузки",
        "cancelAll": "Отменить всё",
        "experimentPlaceholder": "Введите фразу, чтобы услышать её произношение...",
        "speed": "Скорость",
        "speak": "Произнести",
        "yourBlends": "Ваши смеси",
        "noSavedBlends": "Сохранённых смесей пока нет.",
        "installModelAndVoiceFirst": "Сначала установите модель и голос.",
        "featured": "Рекомендуемые",
        "stop": "Стоп",
        "sample": "Образец",
        "voiceLibrary": "Библиотека голосов",
        "starterPack": "Стартовый пакет",
        "select": "Выбрать",
        "all": "Все",
        "installed": "Установленные",
        "installModelToBrowse": "Установите модель, чтобы просматривать голоса.",
        "noVoicesInCatalog": "В каталоге нет голосов. Нажмите «Обновить».",
        "noVoicesMatch": "Нет голосов, соответствующих фильтрам.",
        "collapseAll": "Свернуть все",
        "expandAll": "Развернуть все",
        "selectedCount": "{{count}} выбрано",
        "engineTitle": "Движок Kokoro",
        "variant": "Вариант",
        "status": "Статус",
        "notInstalled": "Не установлен",
        "file": "Файл",
        "modelSize": "Размер модели",
        "voicesSize": "Размер голосов",
        "total": "Итого",
        "assetRoot": "Корневая папка",
        "reinstallModel": "Переустановить модель",
        "installModel": "Установить модель",
        "deleteModel": "Удалить модель",
        "deleteModelDescription": "Освобождает диск; голоса сохраняются.",
        "blend": "Смесь",
        "previewDescription": "Быстрое прослушивание с текстом по умолчанию",
        "editBlendDescription": "Настройте голоса, веса и скорость",
        "deleteBlendDescription": "Удалить этот сохранённый голос",
        "setupTitle": "Настройка Kokoro",
        "allSet": "Всё готово",
        "allSetDescription": "Просматривайте голоса, создавайте смеси или тестируйте в зоне эксперимента."
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
      "conditionalsSection": "Условия",
      "injectionPoints": "Точки внедрения"
    }
  },
  "embeddings": {
    "download": {
      "bestForQuick": "Лучший вариант для быстрого реагирования",
      "balancedPerf": "Сбалансированная производительность",
      "maxContext": "Максимальный контекст",
      "capacity1k": "1K токенов",
      "capacity2k": "2K токенов",
      "capacity4k": "4K токенов",
      "approxSize": "~138 МБ",
      "downloadVersion": "V4"
    },
    "test": {
      "health": "Проверка работоспособности",
      "retrieval": "Извлечение",
      "separation": "Разделение",
      "passed": "Пройдено",
      "failed": "Не удалось",
      "testing": "Тестирование..."
    }
  },
  "convert": {
    "extra": {
      "uec": "Карта унифицированной сущности (UEC)",
      "charaCardV3": "Карта символов V3",
      "charaCardV2": "Карта символов V2"
    }
  },
  "companionSoulWriter": {
    "extra": {
      "json": "JSON",
      "jsonDesc": "Компактный структурированный вывод, когда вызов инструмента недоступен.",
      "xml": "XML",
      "xmlDesc": "Использовать, когда модель форматирует XML более надежно, чем JSON.",
      "fallbackFormat": "Резервный формат"
    }
  },
  "usageAnalytics": {
    "page": {
      "filters": "Фильтры",
      "model": "Модель",
      "character": "Персонаж",
      "clearAll": "Очистить всё",
      "applyFilters": "Применить фильтры",
      "recentActivity": "Недавняя активность",
      "customRange": "Произвольный период",
      "startDate": "Начало периода",
      "endDate": "Конец периода",
      "applyRange": "Применить период",
      "dashboard": "ДАШБОРД",
      "appTime": "ВРЕМЯ В ПРИЛОЖЕНИИ",
      "today": "Сегодня",
      "last7Days": "7 дней",
      "last30Days": "30 дней",
      "all": "Всё",
      "custom": "Произвольно",
      "filtersCount": "{{count}} фильтра",
      "totalCost": "Итоговая стоимость",
      "tokens": "Токены",
      "avgShort": "{{value}} в среднем",
      "requests": "Запросы",
      "period": "Период",
      "last7d": "7 дней",
      "last30d": "30 дней",
      "allTime": "Всё время",
      "recordsCount": "{{count}} записей",
      "usageTrend": "Тенденция использования",
      "tokenConsumptionOverTime": "Потребление токенов за период",
      "input": "Входящие",
      "output": "Исходящие",
      "byModel": "По модели",
      "byCharacter": "По персонажу",
      "top": "Топ",
      "active": "Активные",
      "quickView": "Быстрый просмотр",
      "viewAll": "Смотреть всё",
      "startChatting": "Начните чат, чтобы увидеть данные об использовании",
      "exportedTo": "Экспортировано в: {{path}}",
      "periodTotal": "Итого за период",
      "daysActive": "{{count}} активных дней",
      "dailyAvg": "Среднее в день",
      "selectedPeriod": "выбранный период",
      "yesterdayValue": "Вчера {{value}}",
      "thirtyDayAvg": "Среднее за 30 дней",
      "appTimeTrend": "Тенденция времени в приложении",
      "usageDurationPerDay": "Продолжительность использования в день",
      "totalValue": "Итого {{value}}",
      "activeTime": "Активное время"
    },
    "activity": {
      "loading": "Загрузка активности...",
      "title": "Недавняя активность",
      "recordsCount": "{{count}} записей об использовании",
      "rangeOfTotal": "{{start}}-{{end}} из {{total}}",
      "previous": "Назад",
      "next": "Вперёд",
      "pageOf": "Страница {{page}} из {{total}}"
    },
    "shared": {
      "relativeTime": {
        "justNow": "только что",
        "minutesAgo": "{{count}}м назад",
        "hoursAgo": "{{count}}ч назад",
        "daysAgo": "{{count}}д назад"
      },
      "operations": {
        "chat": "Чат",
        "regenerate": "Повтор",
        "continue": "Продолжить",
        "summary": "Резюме",
        "memoryManager": "Память",
        "imageGeneration": "Изображение",
        "aiCreator": "AI Создатель",
        "replyHelper": "Помощник ответа",
        "groupChatMessage": "Групповой чат",
        "groupChatRegenerate": "Повтор в группе",
        "groupChatContinue": "Продолжить в группе",
        "groupChatDecisionMaker": "Выбор участника"
      },
      "outputImages": "{{count}} изображений",
      "tokens": "{{count}} токенов",
      "unknown": "Неизвестно",
      "requestDetails": "Детали запроса",
      "noStopReason": "Причина остановки не указана",
      "tokenUsage": "Использование токенов",
      "estimatedCost": "Приблизительная стоимость",
      "stats": {
        "prompt": "Промпт",
        "completion": "Ответ",
        "total": "Итого",
        "reasoning": "Рассуждение",
        "image": "Изображение",
        "memory": "Память",
        "summary": "Резюме",
        "inputImages": "Входящие изображения",
        "outputImages": "Исходящие изображения",
        "cachedPrompt": "Кешированный промпт",
        "cacheWrite": "Запись в кеш",
        "webSearches": "Веб-поиски",
        "cacheRead": "Чтение кеша",
        "requestFee": "Комиссия за запрос",
        "webSearch": "Веб-поиск",
        "providerTotal": "Итого у провайдера"
      }
    }
  }
};
