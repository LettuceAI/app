import type { DeepPartialMessageTree } from "../types";
import { type LocaleMessages } from "./en";
import type { LocaleMetadata } from ".";

export const hiMetadata: LocaleMetadata = {
  name: "Hindi",
  label: "हिन्दी",
};

export const hiMessages: DeepPartialMessageTree<LocaleMessages> = {
  "common": {
    "nav": {
      "chats": "चैट्स",
      "settings": "सेटिंग्स",
      "providers": "प्रदाता",
      "responseStyle": "उत्तर शैली",
      "models": "मॉडल",
      "security": "सुरक्षा",
      "accessibility": "सुलभता",
      "reset": "रीसेट",
      "backupRestore": "बैकअप और पुनर्स्थापना",
      "convertFiles": "फ़ाइलें कनवर्ट करें",
      "usageAnalytics": "उपयोग विश्लेषण",
      "changelog": "परिवर्तन लॉग",
      "about": "परिचय",
      "createSystemPrompt": "सिस्टम प्रॉम्प्ट बनाएँ",
      "editSystemPrompt": "सिस्टम प्रॉम्प्ट संपादित करें",
      "systemPrompts": "सिस्टम प्रॉम्प्ट",
      "developer": "डेवलपर",
      "advanced": "उन्नत",
      "characters": "कैरेक्टर",
      "lorebooks": "लोरबुक्स",
      "personas": "पर्सोना",
      "dynamicMemory": "डायनामिक मेमोरी",
      "creationHelper": "निर्माण सहायक",
      "helpMeReply": "जवाब देने में मदद करें",
      "editPersona": "पर्सोना संपादित करें",
      "newTemplate": "नया टेम्पलेट",
      "editTemplate": "टेम्पलेट संपादित करें",
      "chatTemplates": "चैट टेम्पलेट",
      "editCharacter": "कैरेक्टर संपादित करें",
      "sync": "सिंक",
      "newCharacter": "नया कैरेक्टर",
      "engineSetup": "इंजन सेटअप",
      "llmProviders": "LLM प्रदाता",
      "engineSettings": "इंजन सेटिंग्स",
      "lettuceEngine": "Lettuce इंजन",
      "create": "बनाएँ",
      "setup": "सेटअप",
      "welcome": "स्वागत",
      "conversation": "बातचीत",
      "library": "लाइब्रेरी",
      "groupChats": "ग्रुप चैट्स",
      "groupChat": "ग्रुप चैट",
      "imageGeneration": "छवि निर्माण",
      "voices": "आवाज़ें",
      "chatAppearance": "चैट दिखावट",
      "colorCustomization": "रंग अनुकूलन",
      "embeddingDownload": "एम्बेडिंग डाउनलोड",
      "embeddingTest": "एम्बेडिंग परीक्षण",
      "editModel": "मॉडल संपादित करें",
      "messageDebug": "संदेश डीबग",
      "speechRecognition": "स्पीच पहचान",
      "hostApi": "होस्ट API",
      "lorebookEntryGenerator": "लोरबुक एंट्री जनरेटर",
      "companionSoulWriter": "Companion Soul Writer"
    },
    "bottomNav": {
      "chats": "चैट्स",
      "groups": "ग्रुप्स",
      "create": "बनाएँ",
      "discover": "खोजें",
      "library": "लाइब्रेरी"
    },
    "buttons": {
      "cancel": "रद्द करें",
      "confirm": "पुष्टि करें",
      "save": "सेव करें",
      "saving": "सेव हो रहा है...",
      "delete": "हटाएँ",
      "deleting": "हटाया जा रहा है...",
      "create": "बनाएँ",
      "creating": "बनाया जा रहा है...",
      "edit": "संपादित करें",
      "back": "वापस",
      "done": "पूर्ण",
      "download": "डाउनलोड",
      "later": "बाद में",
      "proceed": "आगे बढ़ें",
      "retry": "पुनः प्रयास",
      "discard": "त्यागें",
      "import": "आयात करें",
      "importing": "आयात हो रहा है...",
      "export": "निर्यात करें",
      "exporting": "निर्यात हो रहा है...",
      "update": "अपडेट करें",
      "generate": "जनरेट करें",
      "refresh": "रिफ्रेश करें",
      "continue": "जारी रखें",
      "goBack": "वापस जाएँ",
      "search": "खोजें",
      "clearSearch": "खोज साफ़ करें",
      "add": "जोड़ें",
      "install": "इंस्टॉल करें",
      "remove": "हटाएँ",
      "rename": "नाम बदलें",
      "copy": "कॉपी करें",
      "copied": "कॉपी हो गया!",
      "browseFiles": "फ़ाइलें ब्राउज़ करें"
    },
    "labels": {
      "processing": "प्रोसेसिंग...",
      "loading": "लोड हो रहा है...",
      "noDescriptionYet": "अभी तक कोई विवरण नहीं",
      "untitled": "बिना शीर्षक",
      "default": "डिफ़ॉल्ट",
      "enabled": "सक्षम",
      "disabled": "अक्षम",
      "on": "चालू",
      "off": "बंद",
      "none": "कोई नहीं",
      "auto": "ऑटो",
      "custom": "कस्टम",
      "name": "नाम",
      "description": "विवरण",
      "content": "सामग्री",
      "preview": "पूर्वावलोकन",
      "options": "विकल्प"
    },
    "time": {
      "justNow": "अभी",
      "minutesAgo": "{{minutes}} मिनट पहले",
      "hoursAgo": "{{hours}} घंटे पहले",
      "daysAgo": "{{days}} दिन पहले",
      "today": "आज",
      "yesterday": "कल",
      "last7Days": "पिछले 7 दिन",
      "older": "पुराने"
    }
  },
  "settings": {
    "sections": {
      "intelligence": "बुद्धिमत्ता",
      "experience": "अनुभव",
      "connectivity": "कनेक्टिविटी",
      "securityPrivacy": "सुरक्षा और गोपनीयता",
      "supportInfo": "सहायता और जानकारी",
      "dangerZone": "खतरनाक क्षेत्र",
      "developer": "डेवलपर"
    },
    "items": {
      "providers": {
        "title": "प्रदाता",
        "subtitle": "AI सेवाओं से जुड़ें"
      },
      "models": {
        "title": "मॉडल",
        "subtitle": "AI मॉडल कॉन्फ़िगर करें"
      },
      "imageGeneration": {
        "title": "छवि निर्माण",
        "subtitle": "छवियां बनाएं और परीक्षण करें"
      },
      "voices": {
        "title": "आवाज़ें",
        "subtitle": "टेक्स्ट-टू-स्पीच आवाज़ें"
      },
      "accessibility": {
        "title": "सुलभता",
        "subtitle": "ध्वनि संकेत और हैप्टिक्स"
      },
      "prompts": {
        "title": "सिस्टम प्रॉम्प्ट",
        "subtitle": "AI व्यक्तित्व आकार दें"
      },
      "security": {
        "title": "सुरक्षा",
        "subtitle": "एन्क्रिप्शन और गोपनीयता"
      },
      "backup": {
        "title": "बैकअप और पुनर्स्थापना",
        "subtitle": "डेटा निर्यात या आयात करें"
      },
      "convert": {
        "title": "फ़ाइलें कनवर्ट करें",
        "subtitle": "पुरानी .json फ़ाइलों को .uec में बदलें"
      },
      "sync": {
        "title": "लोकल सिंक",
        "subtitle": "उपकरणों में सिंक करें"
      },
      "usage": {
        "title": "उपयोग विश्लेषण",
        "subtitle": "लागत और टोकन आँकड़े"
      },
      "advanced": {
        "title": "उन्नत",
        "subtitle": "मेमोरी और सुविधाएँ"
      },
      "logs": {
        "title": "लॉग्स",
        "subtitle": "डीबग और डायग्नोस्टिक्स"
      },
      "guide": {
        "title": "सेटअप गाइड",
        "subtitle": "ऑनबोर्डिंग फिर से चलाएँ"
      },
      "docs": {
        "title": "दस्तावेज़ीकरण",
        "subtitle": "गाइड और संदर्भ"
      },
      "github": {
        "title": "समस्या रिपोर्ट करें",
        "subtitle": "बग और फ़ीडबैक • v{{version}}"
      },
      "discord": {
        "title": "Discord में शामिल हों",
        "subtitle": "समुदाय और सहायता"
      },
      "about": {
        "title": "परिचय",
        "subtitle": "संस्करण, अपडेट और लिंक"
      },
      "changelog": {
        "title": "परिवर्तन लॉग",
        "subtitle": "नया क्या है"
      },
      "reset": {
        "title": "रीसेट",
        "subtitle": "सब कुछ मिटाएँ"
      },
      "developer": {
        "title": "डेवलपर टूल्स",
        "subtitle": "डीबग और परीक्षण"
      }
    }
  },
  "accessibility": {
    "sectionTitles": {
      "language": "भाषा",
      "sounds": "ध्वनि फ़ीडबैक",
      "haptics": "हैप्टिक फ़ीडबैक",
      "appearance": "दिखावट"
    },
    "language": {
      "appLanguage": "ऐप भाषा",
      "description": "नेविगेशन और सेटिंग्स में उपयोग की जाने वाली भाषा चुनें।"
    },
    "appearance": {
      "customColors": "कस्टम रंग",
      "customColorsDesc": "ऐप की रंग योजना को अनुकूलित करें",
      "chatAppearance": "चैट दिखावट",
      "chatAppearanceDesc": "संदेश बुलबुले, फ़ॉन्ट और लेआउट अनुकूलित करें"
    },
    "haptics": {
      "vibrateOnChat": "चैट पर कंपन",
      "vibrateDesc": "सहायक टाइप करते समय छोटी कंपन दालें",
      "intensity": "तीव्रता",
      "light": "हल्का",
      "medium": "मध्यम",
      "heavy": "भारी",
      "soft": "नरम",
      "rigid": "कठोर"
    },
    "sounds": {
      "send": "भेजें",
      "sendDescription": "जब आप संदेश भेजते हैं तो बजता है",
      "success": "सफलता",
      "successDescription": "जब सहायक सफलतापूर्वक समाप्त करता है तब बजता है",
      "failure": "विफलता",
      "failureDescription": "त्रुटि या रद्द करने पर बजता है",
      "testButton": "परीक्षण"
    },
    "feedbackInfo": "फ़ीडबैक आपको यह जानने में मदद करता है कि संदेश कब भेजे या प्राप्त हुए हैं।",
    "hapticsInfo": "हैप्टिक्स मोबाइल उपकरणों पर उपलब्ध हैं।",
    "extra": {
      "easterEggs": "ईस्टर एग्स",
      "beetrootRain": "चुकंदर की बारिश",
      "beetrootDesc": "जब चैट में चुकंदर का ज़िक्र हो तो चुकंदर गिरते हैं"
    }
  },
  "topNav": {
    "unsavedChangesTitle": "असुरक्षित परिवर्तन",
    "unsavedChangesMessage": "जाने से पहले अपने परिवर्तन सेव या त्यागें।",
    "goBack": "वापस जाएँ",
    "changeLayout": "लेआउट बदलें",
    "search": "खोजें",
    "settings": "सेटिंग्स",
    "help": "सहायता",
    "add": "जोड़ें",
    "openFilters": "फ़िल्टर खोलें",
    "save": "सेव करें",
    "extra": {
      "installedModels": "इंस्टॉल किए गए मॉडल",
      "refresh": "रिफ्रेश करें",
      "minimize": "छोटा करें",
      "maximize": "बड़ा करें",
      "close": "बंद करें"
    }
  },
  "updates": {
    "available": {
      "title": "नया संस्करण उपलब्ध है",
      "description": "v{{latestVersion}} उपलब्ध है। आप v{{currentVersion}} पर हैं।",
      "actions": {
        "view": "रिलीज़ देखें"
      }
    }
  },
  "about": {
    "kicker": "ऐप जानकारी",
    "appName": "LettuceAI",
    "description": "संस्करण विवरण, अपडेट जांच और उपयोगी लिंक।",
    "info": {
      "version": "संस्करण",
      "channel": "चैनल",
      "platform": "प्लेटफ़ॉर्म"
    },
    "buildChannel": {
      "dev": "डेवलपमेंट बिल्ड",
      "release": "स्थिर रिलीज़"
    },
    "update": {
      "sectionTitle": "अपडेट",
      "title": "ऐप अपडेट",
      "description": "नए रिलीज़ को मैन्युअल रूप से जांचें या स्टार्टअप पर स्वचालित जांच बंद करें।",
      "autoChecks": "स्वचालित अपडेट जांच",
      "autoChecksDescription": "सक्षम होने पर, LettuceAI ऐप खुलते समय नए संस्करण जांचता है।",
      "checkNow": "अपडेट जांचें",
      "checking": "अपडेट जांचे जा रहे हैं...",
      "upToDateTitle": "आप नवीनतम संस्करण पर हैं",
      "upToDateDescription": "इस डिवाइस के लिए अभी कोई नया रिलीज़ उपलब्ध नहीं है।",
      "failedTitle": "अपडेट जांच विफल रही",
      "failedDescription": "अभी अपडेट जांचे नहीं जा सके। थोड़ी देर बाद फिर कोशिश करें।"
    },
    "links": {
      "sectionTitle": "लिंक",
      "website": "वेबसाइट",
      "websiteDescription": "डाउनलोड पेज और रिलीज़ जानकारी",
      "github": "GitHub",
      "githubDescription": "सोर्स कोड, इश्यू और विकास",
      "discord": "Discord",
      "discordDescription": "समुदाय सर्वर और सहायता",
      "reddit": "Reddit",
      "redditDescription": "समुदाय चर्चाएं, फीडबैक और अपडेट"
    },
    "developerMode": {
      "enable": "डेवलपर मोड सक्षम करें",
      "enabled": "डेवलपर मोड सक्षम है"
    },
    "errors": {
      "saveTitle": "पसंद सहेजी नहीं जा सकी",
      "saveDescription": "आपकी अपडेट-जांच पसंद नहीं बदली गई।"
    }
  },
  "components": {
    "tooltip": {
      "dismissHint": "खारिज करने के लिए कहीं भी टैप करें"
    },
    "backgroundPosition": {
      "title": "पृष्ठभूमि स्थिति",
      "instructions": "स्थिति बदलने के लिए खींचें • ज़ूम के लिए पिंच या स्क्रॉल करें"
    },
    "avatarSource": {
      "generateImage": "छवि जनरेट करें",
      "generateImageDesc": "AI-संचालित अवतार निर्माण",
      "noImageModels": "कोई छवि मॉडल उपलब्ध नहीं",
      "editCurrent": "वर्तमान संपादित करें",
      "editCurrentDesc": "पुनः स्थिति या क्रॉप करें",
      "chooseImage": "छवि चुनें",
      "chooseImageDesc": "अपने डिवाइस से चुनें"
    },
    "avatarCurrentEdit": {
      "title": "वर्तमान संपादित करें",
      "reposition": "स्थान बदलने",
      "repositionDesc": "वर्तमान अवतार को स्थानांतरित करें या क्रॉप करें",
      "editWithAI": "एआई के साथ संपादित करें",
      "editWithAIDesc": "वर्तमान अवतार के लिए AI संपादन खोलें",
      "noImageModels": "कोई छवि मॉडल उपलब्ध नहीं है"
    },
    "avatarGeneration": {
      "modelsLoadError": "छवि जनरेशन मॉडल लोड करने में विफल",
      "title": "अवतार जनरेट करें",
      "help": "अवतार जनरेशन में सहायता",
      "model": "मॉडल",
      "selectModel": "एक मॉडल चुनें",
      "describe": "अपना अवतार वर्णन करें",
      "describePlaceholder": "रंगीन बालों वाली एक मिलनसार एनीमे लड़की, गर्मजोशी से मुस्कुराती हुई...",
      "inProgress": "अवतार जनरेट हो रहा है...",
      "editingInProgress": "अवतार संपादन लागू किया जा रहा है...",
      "previousVariant": "पिछला संस्करण",
      "nextVariant": "अगला संस्करण",
      "variantCounter": "{{current}} / {{total}}",
      "editRequest": "अनुरोध संपादित करें",
      "editRequestPlaceholder": "बाल काले करो, चश्मा लगाओ, चेहरा वैसा ही रखो...",
      "applyEdit": "संपादन लागू करें",
      "editImageLoadError": "संपादन के लिए जनरेट किया गया अवतार तैयार करने में विफल",
      "aiAssistant": "एआई सहायक",
      "backToResults": "प्रॉम्प्ट पर वापस जाएँ",
      "magicInTheWorks": "कार्यों में जादू...",
      "refine": "परिष्कृत",
      "apply": "आवेदन करना",
      "alt": "जनरेट किया गया अवतार",
      "regenerate": "पुनः जनरेट करें",
      "useThis": "इसे उपयोग करें"
    },
    "avatarPosition": {
      "title": "अवतार स्थिति",
      "instructions": "स्थिति बदलने के लिए खींचें • ज़ूम के लिए पिंच या स्क्रॉल करें",
      "alt": "अवतार स्थिति के लिए"
    },
    "confirmDialog": {
      "defaultTitle": "पुष्टि करें",
      "defaultLabel": "पुष्टि करें"
    },
    "bottomMenu": {
      "defaultTitle": "मेनू",
      "dragTip": "मेनू बंद करने के लिए खींचें",
      "closeLabel": "मेनू बंद करें",
      "buttonProcessing": "प्रोसेसिंग..."
    },
    "modelSelector": {
      "placeholder": "एक मॉडल चुनें",
      "clearLabel": "वैश्विक डिफ़ॉल्ट उपयोग करें",
      "loading": "मॉडल लोड हो रहे हैं...",
      "noModels": "कोई मॉडल उपलब्ध नहीं",
      "addProviderFirst": "पहले सेटिंग्स में एक प्रदाता जोड़ें",
      "title": "मॉडल चुनें",
      "searchPlaceholder": "मॉडल खोजें...",
      "noResults": "कोई मॉडल नहीं मिला",
      "noResultsHint": "कोई भिन्न खोज शब्द आज़माएँ"
    },
    "localeSelector": {
      "title": "भाषा चुने"
    },
    "promptTemplate": {
      "nameContentRequired": "नाम और सामग्री आवश्यक हैं",
      "saveError": "टेम्पलेट सेव करने में विफल",
      "editTitle": "प्रॉम्प्ट संपादित करें",
      "createTitle": "प्रॉम्प्ट बनाएँ",
      "name": "नाम",
      "namePlaceholder": "उदा., रोलप्ले मास्टर",
      "content": "सामग्री",
      "variablesButton": "चर",
      "contentPlaceholder": "आप एक सहायक AI सहायक हैं...\n\nअपने प्रॉम्प्ट में {{char.name}} और {{scene}} का उपयोग करें।",
      "characterCount": "{{count}} अक्षर",
      "preview": "पूर्वावलोकन",
      "characterPlaceholder": "कैरेक्टर…",
      "personaPlaceholder": "पर्सोना…",
      "rendering": "रेंडरिंग…",
      "noPreview": "अभी तक कोई पूर्वावलोकन नहीं",
      "saving": "सेव हो रहा है...",
      "update": "अपडेट करें",
      "create": "बनाएँ",
      "variablesTitle": "टेम्पलेट चर",
      "variablesCopyHint": "क्लिपबोर्ड पर कॉपी करने के लिए टैप करें",
      "variablesCopied": "कॉपी हो गया",
      "variables": {
        "charName": "कैरेक्टर का नाम",
        "charNameDesc": "कैरेक्टर का नाम",
        "charDesc": "कैरेक्टर विवरण",
        "charDescDesc": "कैरेक्टर का विवरण",
        "scene": "दृश्य",
        "sceneDesc": "शुरुआती दृश्य/परिदृश्य",
        "userName": "उपयोगकर्ता नाम",
        "userNameDesc": "उपयोगकर्ता पर्सोना नाम",
        "userDesc": "उपयोगकर्ता विवरण",
        "userDescDesc": "उपयोगकर्ता पर्सोना विवरण",
        "rules": "नियम",
        "rulesDesc": "कैरेक्टर व्यवहार नियम",
        "contextSummary": "संदर्भ सारांश",
        "contextSummaryDesc": "गतिशील बातचीत सारांश",
        "keyMemories": "मुख्य यादें",
        "keyMemoriesDesc": "प्रासंगिक यादों की सूची"
      }
    },
    "characterExport": {
      "title": "निर्यात प्रारूप",
      "selectFormat": "एक प्रारूप चुनें",
      "loading": "प्रारूप लोड हो रहे हैं...",
      "formatUecDesc": "यूनिफाइड एंटिटी कार्ड (.uec) प्रारूप (अनुशंसित)।",
      "formatLegacyJsonDesc": "पुराना JSON (केवल आयात)।",
      "formatV3Desc": "कैरेक्टर कार्ड V3 JSON (नवीनतम स्पेक)।",
      "formatV2Desc": "कैरेक्टर कार्ड V2 JSON (Tavern स्पेक)।",
      "formatV1Desc": "कैरेक्टर कार्ड V1 (केवल आयात)।"
    },
    "embeddingDownload": {
      "downloadRequired": "डाउनलोड आवश्यक",
      "modelRequired": "एम्बेडिंग मॉडल आवश्यक",
      "description": "डायनामिक मेमोरी को काम करने के लिए एक स्थानीय एम्बेडिंग मॉडल (~260 MB) डाउनलोड करना आवश्यक है।",
      "localStorage": "• मॉडल आपके डिवाइस पर स्थानीय रूप से संग्रहीत होगा",
      "downloadSize": "• डाउनलोड आकार: लगभग 260 MB",
      "summarization": "• बातचीत सारांशीकरण के लिए आवश्यक"
    },
    "embeddingUpgrade": {
      "title": "Embedding Model v4 उपलब्ध",
      "v1Message": "आप 512 टोकन के साथ v1 का उपयोग कर रहे हैं। बेहतर मेमोरी गुणवत्ता और लंबे-संदर्भ समर्थन के लिए v4 में अपग्रेड करें।",
      "v2Message": "आप पुराना v2 उपयोग कर रहे हैं। नवीनतम एम्बेडिंग मॉडल के साथ बेहतर मेमोरी गुणवत्ता के लिए v4 में अपग्रेड करें।",
      "v3Message": "v4 आ गया है और v3 की तुलना में roleplay memory recall में नाटकीय सुधार करता है (recall@1 0.02 -> 0.92)। Upgrade करना अनुशंसित है।",
      "button": "v4 में अपग्रेड करें"
    },
    "v2UpgradeToast": {
      "title": "मेमोरी मॉडल v4",
      "badge": "उपलब्ध",
      "message": "पुराने मॉडलों की तुलना में बेहतर embedding गुणवत्ता",
      "dismiss": "खारिज करें",
      "upgrade": "अपग्रेड करें"
    },
    "v1UpgradeToast": {
      "title": "मेमोरी मॉडल v4 उपलब्ध",
      "message": "बेहतर मेमोरी गुणवत्ता और लंबे-संदर्भ समर्थन के लिए अपग्रेड करें।",
      "dismiss": "खारिज करें",
      "upgrade": "अपग्रेड करें"
    },
    "v3UpgradeToast": {
      "title": "Memory model v4",
      "badge": "उपलब्ध",
      "message": "v4 v3 की तुलना में roleplay memory recall में नाटकीय सुधार करता है (recall@1 0.02 → 0.92)। Upgrade करना अनुशंसित है।",
      "dismiss": "बाद में",
      "upgrade": "अपग्रेड करें"
    },
    "createMenu": {
      "title": "नया बनाएँ",
      "smartCreator": "स्मार्ट क्रिएटर",
      "smartCreatorDesc": "सहायक को अपनी रचना में मार्गदर्शन करने दें",
      "divider": "या मैन्युअल रूप से बनाएँ",
      "character": "कैरेक्टर",
      "characterDesc": "एक कस्टम कैरेक्टर बनाएँ",
      "persona": "पर्सोना",
      "personaDesc": "एक पुन: उपयोग योग्य आवाज़ बनाएँ",
      "groupChat": "ग्रुप चैट",
      "groupChatDesc": "कई कैरेक्टर्स के साथ चैट करें",
      "lorebook": "लोरबुक",
      "lorebookDesc": "अपना विश्व संदर्भ बनाएँ",
      "characterSmartDesc": "मार्गदर्शित निर्माण के साथ एक कैरेक्टर बनाएँ",
      "personaSmartDesc": "एक पुन: उपयोग योग्य आवाज़ या व्यक्तित्व बनाएँ",
      "lorebookSmartDesc": "एक संरचित विश्व संदर्भ बनाएँ",
      "loadingConversations": "बातचीत लोड हो रही हैं...",
      "createNew": "नया बनाएँ",
      "createNewDesc": "एक नई मार्गदर्शित निर्माण चैट शुरू करें",
      "editExisting": "मौजूदा संपादित करें",
      "continueLast": "पिछली बातचीत जारी रखें",
      "seeOlder": "पुरानी बातचीत देखें",
      "seeOlderDesc": "कोई भी पिछली स्मार्ट क्रिएटर बातचीत खोलें",
      "noConversations": "इस क्रिएटर के लिए अभी तक कोई बातचीत नहीं।",
      "sessionCompleted": "पूर्ण",
      "sessionCancelled": "रद्द",
      "sessionDraft": "ड्राफ्ट",
      "sessionMessages": "{{count}} संदेश",
      "untitledConversation": "बिना शीर्षक बातचीत",
      "nameLorebookTitle": "लोरबुक का नाम दें",
      "lorebookNamePlaceholder": "लोरबुक का नाम दर्ज करें...",
      "lorebookImporting": "आयात हो रहा है...",
      "lorebookImport": "आयात करें",
      "lorebookCreating": "बनाया जा रहा है...",
      "lorebookCreate": "बनाएँ",
      "editExistingDesc": "एक {{goal}} चुनें और स्मार्ट क्रिएटर से संपादित करें",
      "creatorTitle": "{{goal}} क्रिएटर",
      "editTitle": "{{goal}} संपादित करें",
      "conversationsTitle": "{{goal}} बातचीत",
      "loadingItems": "{{items}} लोड हो रहे हैं...",
      "noItemsFound": "कोई {{items}} नहीं मिला।",
      "unnamedCharacter": "अनाम कैरेक्टर",
      "untitledPersona": "बिना शीर्षक पर्सोना",
      "untitledLorebook": "बिना शीर्षक लोरबुक"
    },
    "advancedModelSettings": {
      "temperature": "तापमान",
      "temperatureDesc": "अधिक = अधिक रचनात्मक",
      "topP": "Top P",
      "topPDesc": "कम = अधिक केंद्रित",
      "maxTokens": "अधिकतम आउटपुट टोकन",
      "maxTokensDesc": "डिफ़ॉल्ट के लिए खाली छोड़ें",
      "contextLength": "संदर्भ लंबाई",
      "contextLengthDesc": "केवल स्थानीय मॉडल",
      "contextLengthAuto": "ऑटो",
      "frequencyPenalty": "आवृत्ति दंड",
      "frequencyPenaltyDesc": "टोकन की पुनरावृत्ति कम करें",
      "presencePenalty": "उपस्थिति दंड",
      "presencePenaltyDesc": "नए विषयों को प्रोत्साहित करें",
      "topK": "Top K",
      "topKDesc": "टोकन पूल का आकार सीमित करें",
      "reasoning": "सोच / तर्क",
      "reasoningAutoDesc": "यह मॉडल हमेशा तर्क का उपयोग करता है। कोई कॉन्फ़िगरेशन आवश्यक नहीं।",
      "reasoningEnableDesc": "जटिल समस्या समाधान और तर्क कार्यों के लिए उन्नत सोच क्षमताएँ सक्षम करें।",
      "effortMode": "प्रयास मोड",
      "budgetMode": "बजट मोड",
      "reasoningEffort": "तर्क प्रयास",
      "reasoningEffortDesc": "सोच की गहराई नियंत्रित करता है",
      "reasoningEffortAuto": "ऑटो",
      "reasoningEffortLow": "कम",
      "reasoningEffortMed": "मध्यम",
      "reasoningEffortHigh": "उच्च",
      "reasoningBudget": "तर्क बजट (टोकन)",
      "reasoningBudgetDesc": "सोचने के लिए आरक्षित अधिकतम टोकन",
      "reasoningEffortLowDesc": "न्यूनतम तर्क के साथ त्वरित उत्तर",
      "reasoningEffortMediumDesc": "संतुलित तर्क गहराई",
      "reasoningEffortHighDesc": "जटिल समस्याओं के लिए अधिकतम तर्क गहराई",
      "reasoningBudgetExtendedDesc": "सोचने के लिए आरक्षित अधिकतम टोकन। आउटपुट सीमा में जोड़ा जाता है।"
    },
    "providerParameterSupport": {
      "unknownProvider": "अज्ञात प्रदाता: {{providerId}}",
      "reasoningNotSupportedEffort": "समर्थित नहीं - यह प्रदाता प्रयास स्तरों का उपयोग नहीं करता",
      "reasoningNotSupportedBudgetOnly": "समर्थित नहीं - यह प्रदाता केवल बजट दृष्टिकोण का उपयोग करता है",
      "reasoningNotSupported": "समर्थित नहीं - यह प्रदाता तर्क का समर्थन नहीं करता",
      "unsupportedParametersIgnored": "असमर्थित पैरामीटर {{providerName}} द्वारा अनदेखा किए जाएँगे।",
      "reasoningEffortSupported": "सोचने वाले मॉडल (o1, DeepSeek-R1, आदि) के लिए तर्क प्रयास समर्थित है",
      "reasoningBudgetSupported": "यह प्रदाता बजट-आधारित सोच का उपयोग करता है (कोई प्रयास स्तर नहीं)। इसके बजाय तर्क बजट टोकन सेट करें।",
      "reasoningNotSupportedProvider": "यह प्रदाता तर्क पैरामीटर का समर्थन नहीं करता।",
      "matrixTitle": "प्रदाता पैरामीटर सपोर्ट मैट्रिक्स",
      "providerColumn": "प्रदाता",
      "supportedIndicator": "प्रदाता API द्वारा समर्थित",
      "notSupportedIndicator": "समर्थित नहीं (पैरामीटर अनदेखा किया जाएगा)"
    },
    "extra": {
      "promptCachingTitle": "Prompt Caching",
      "promptCachingDescription": "लंबे, दोहराव वाले संदर्भों (जैसे बड़े system prompts या गहरे chat इतिहास) के लिए generation तेज़ करता है और costs कम करता है।",
      "avatarAlt": "अवतार",
      "chooseFromLibrary": "लाइब्रेरी से चुनें",
      "chooseFromLibraryDesc": "ऐप में पहले से सहेजी गई छवि उपयोग करें",
      "generateFailed": "छवि जनरेट करने में विफल",
      "editFailed": "अवतार संपादित करने में विफल",
      "backgroundAlt": "स्थिति के लिए बैकग्राउंड",
      "formatsLoadFailed": "निर्यात प्रारूप लोड करने में विफल",
      "formatsShowingDefaults": "(डिफ़ॉल्ट दिखा रहा है)",
      "timeJustNow": "अभी",
      "timeMinutesAgo": "{{minutes}} मिनट पहले",
      "timeHoursAgo": "{{hours}} घंटे पहले",
      "timeDaysAgo": "{{days}} दिन पहले",
      "removeReference": "Design reference हटाएँ",
      "thumbLoading": "लोड हो रहा है...",
      "addReferences": "References जोड़ें",
      "visualDescription": "Visual description",
      "draftWithAi": "AI से draft करें",
      "regenerate": "पुनः जनरेट करें",
      "useThis": "इसे उपयोग करें",
      "referenceImagesLabel": "Reference images",
      "writerHelpFallback": "compatible scene writer model",
      "writerHelpUses": "{{model}} का उपयोग करके आपके avatar और reference images से draft करता है।",
      "writerHelpUnavailable": "इसे स्वचालित रूप से draft करने के लिए Image Generation settings में एक compatible scene writer model जोड़ें।",
      "writerNotAvailableError": "पहले Image Generation settings में एक compatible scene writer model जोड़ें।",
      "writerNoSourcesError": "जनरेट करने से पहले एक avatar या कम से कम एक reference image जोड़ें।",
      "noUsableReferences": "कोई उपयोगी reference images नहीं मिलीं।",
      "draftFailed": "Design description जनरेट करने में विफल।",
      "draftReadFailed": "Image asset पढ़ने में विफल ({{status}})",
      "draftConvertFailed": "Image asset को data URL में बदलने में विफल",
      "draftGenerationFailed": "Draft generation विफल।",
      "draftMenuTitle": "AI Design Draft",
      "draftedBy": "{{model}} द्वारा वर्तमान avatar और reference images से तैयार।",
      "draftedByFallback": "आपका scene writer model",
      "noWriterUseHelper": "इस helper का उपयोग करने से पहले एक compatible scene writer model जोड़ें।",
      "emptyReferences": "चेहरा, अनुपात, पोशाक और शैली lock करने के लिए कुछ स्पष्ट reference shots जोड़ें।",
      "designReferencesTitle": "Design references",
      "designReferencesDescription": "कुछ स्पष्ट reference images और एक canonical visual description अपलोड करें।",
      "designReferencesPlaceholder": "स्थिर रूप वर्णन करें: चेहरा, बाल, निर्माण, उम्र प्रस्तुति, पोशाक संकेत, accessories, और art/style direction।",
      "dismissAria": "खारिज करें",
      "v3MessageFallback": "lettuce-emb-v4 आ गया है और roleplay memory recall में नाटकीय सुधार करता है। Upgrade करना अनुशंसित है।",
      "uploadButton": "अपलोड करें",
      "libraryButton": "लाइब्रेरी",
      "companionSetupTitle": "Companion को setup की ज़रूरत है",
      "companionSetupSubtitleSingle": "Companion mode चलाने से पहले एक और model चाहिए। Skip करने पर यह कैरेक्टर Roleplay में वापस आ जाएगा।",
      "companionSetupSubtitleMany": "Companion mode चलाने से पहले {{count}} और model चाहिए। Skip करने पर यह कैरेक्टर Roleplay में वापस आ जाएगा।",
      "companionSetupBody": "Companion mode को emotion analyze करने, entities निकालने, memories route करने, और past context याद करने के लिए कुछ local models चाहिए।",
      "companionUseRoleplay": "इसके बजाय Roleplay उपयोग करें",
      "companionDownloadNow": "अभी डाउनलोड करें",
      "searchModelsPlaceholder": "मॉडल खोजें...",
      "loadingModelsDefault": "मॉडल लोड हो रहे हैं...",
      "noModelsAvailable": "कोई मॉडल उपलब्ध नहीं।",
      "noModelsMatching": "\"{{query}}\" से मेल खाने वाला कोई मॉडल नहीं मिला।",
      "contentPlaceholderText": "आप एक सहायक AI सहायक हैं...\n\nअपने prompt में {{char.name}} और {{scene}} का उपयोग करें।",
      "previewRenderFailed": "<preview render विफल>",
      "charactersCount": "{{count}} अक्षर"
    }
  },
  "chats": {
    "characterNotFound": "कैरेक्टर नहीं मिला",
    "chatHistory": "चैट इतिहास",
    "previousConversationsWithCharacter": "{{name}} के साथ पिछली बातचीत",
    "previousConversations": "पिछली बातचीत",
    "searchChats": "चैट खोजें...",
    "searchResults": "{{count}} परिणाम",
    "noConversationsYet": "अभी तक कोई बातचीत नहीं",
    "startChattingPrompt": "अपना इतिहास यहाँ देखने के लिए चैट शुरू करें",
    "noMatchingChats": "कोई मेल खाती चैट नहीं",
    "tryDifferentSearchTerm": "कोई भिन्न खोज शब्द आज़माएँ",
    "untitledChat": "बिना शीर्षक चैट",
    "chatTitlePlaceholder": "चैट शीर्षक...",
    "exportChatPackage": "चैट पैकेज निर्यात करें",
    "characterSpecificExport": "कैरेक्टर-विशिष्ट निर्यात",
    "characterSpecificExportDesc": "डिफ़ॉल्ट रूप से इस पैकेज को चैट कैरेक्टर से बाँधें।",
    "nonCharacterSpecificExport": "गैर-कैरेक्टर-विशिष्ट निर्यात",
    "nonCharacterSpecificExportDesc": "आयात के दौरान कैरेक्टर चयन आवश्यक करें।",
    "deleteChat": "चैट हटाएँ?",
    "deleteConfirmDesc": "इसे इतिहास से स्थायी रूप से हटा दिया जाएगा",
    "keepThisChat": "यह चैट रखें",
    "editCharacter": "कैरेक्टर संपादित करें",
    "exportCharacter": "कैरेक्टर निर्यात करें",
    "chatAppearance": "चैट दिखावट",
    "importChatPackage": "चैट पैकेज आयात करें",
    "hideThisCharacter": "यह कैरेक्टर छिपाएँ",
    "deleteCharacter": "कैरेक्टर हटाएँ",
    "deleteCharacterTitle": "कैरेक्टर हटाएँ?",
    "deleteCharacterConfirmation": "क्या आप वाकई \"{{name}}\" को हटाना चाहते हैं? इससे इस कैरेक्टर के साथ सभी चैट सत्र भी हट जाएँगे।",
    "characterSpecificMatches": "यह पैकेज कैरेक्टर-विशिष्ट है और चयनित कैरेक्टर से मेल खाता है।",
    "characterSpecificMismatch": "यह पैकेज कैरेक्टर-विशिष्ट है और किसी अन्य कैरेक्टर की ओर इशारा करता है। इसे {{name}} में आयात किया जाएगा।",
    "nonCharacterSpecificImport": "यह पैकेज गैर-कैरेक्टर-विशिष्ट है। इसे {{name}} में आयात किया जाएगा।",
    "noCharactersYet": "अभी तक कोई कैरेक्टर नहीं",
    "createFirstCharacter": "चैट शुरू करने के लिए नीचे + बटन से अपना पहला कैरेक्टर बनाएँ",
    "scrollToBottom": "नीचे स्क्रॉल करें",
    "selectCharacter": "कैरेक्टर चुनें",
    "branchToGroupChat": "ग्रुप चैट में शाखा बनाएँ",
    "addContent": "सामग्री जोड़ें",
    "swapPlaces": "स्थान बदलें",
    "swapPlacesOn": "स्थान बदलें (चालू)",
    "uploadImage": "छवि अपलोड करें",
    "helpMeReply": "जवाब देने में मदद करें",
    "sceneImage": {
      "modeTitle": "दृश्य छवि",
      "modeDescription": "चुनें कि क्या दृश्य संकेत स्वयं लिखना है या पहले एआई को इसका मसौदा तैयार करने देना है।",
      "writePrompt": "शीघ्र लिखें",
      "writePromptDesc": "सटीक दृश्य छवि संकेत टाइप करें जिसका आप उपयोग करना चाहते हैं।",
      "askAi": "एआई से पूछें",
      "askAiDesc": "वर्तमान चैट मॉडल को चयनित क्षण से एक दृश्य संकेत का मसौदा तैयार करने दें।",
      "generateTitle": "दृश्य छवि उत्पन्न करें",
      "regenerateTitle": "दृश्य छवि पुन: उत्पन्न करें",
      "aiTitle": "एआई सीन प्रॉम्प्ट",
      "promptLabel": "दृश्य संकेत",
      "promptPlaceholder": "दृश्य, पात्रों, मनोदशा, प्रकाश व्यवस्था, कैमरा फ्रेमिंग और महत्वपूर्ण विवरणों का वर्णन करें...",
      "suggestedPrompt": "सुझाया गया संकेत",
      "regeneratePrompt": "पुनः जेनरेट",
      "editPrompt": "शीघ्र संपादित करें",
      "reviewTitle": "दृश्य प्रॉम्प्ट की समीक्षा",
      "denyPrompt": "अस्वीकार",
      "acceptPrompt": "स्वीकार",
      "generateImage": "छवि उत्पन्न करें",
      "updateImage": "छवि अद्यतन करें"
    },
    "useMyTextAsBase": "मेरे टेक्स्ट को आधार के रूप में उपयोग करें",
    "writeNewReply": "कुछ नया लिखें",
    "suggestedReply": "सुझाया गया जवाब",
    "selectTwoCharactersMinimum": "ग्रुप चैट के लिए कम से कम 2 कैरेक्टर चुनें।",
    "groupBranchCreated": "ग्रुप शाखा बनाई गई! रीडायरेक्ट हो रहा है...",
    "memories": "यादें",
    "tools": "उपकरण",
    "pinned": "पिन किए गए",
    "searchMemories": "यादें खोजें...",
    "addMemory": "याद जोड़ें",
    "memoryActions": "याद कार्रवाइयाँ",
    "pinnedMessages": "पिन किए गए संदेश",
    "pinnedMessagesDesc": "हमेशा संदर्भ में शामिल",
    "contextSummary": "संदर्भ सारांश",
    "contextSummaryPlaceholder": "संदेशों में संदर्भ सुसंगत रखने के लिए उपयोग किया जाने वाला संक्षिप्त सारांश...",
    "memoryContentPlaceholder": "क्या याद रखना चाहिए?",
    "editMemoryPlaceholder": "याद सामग्री दर्ज करें...",
    "togglePin": {
      "pin": "पिन करें",
      "unpin": "अनपिन करें"
    },
    "toggleMemoryState": {
      "setHot": "हॉट सेट करें",
      "setCold": "कोल्ड सेट करें"
    },
    "selectModelForRetry": "पुनः प्रयास के लिए मॉडल चुनें",
    "memoryCategories": {
      "characterTrait": "कैरेक्टर विशेषता",
      "relationship": "संबंध",
      "plotEvent": "कथानक घटना",
      "worldDetail": "विश्व विवरण",
      "preference": "प्राथमिकता",
      "other": "अन्य"
    },
    "settings": {
      "memorySection": "मेमोरी",
      "memorySectionDesc": "सारांश, टैग, टूल कॉल इतिहास",
      "quickSettings": "त्वरित सेटिंग्स",
      "quickSettingsDesc": "सबसे सामान्य समायोजन",
      "persona": "पर्सोना",
      "model": "मॉडल",
      "fallbackModel": "फ़ॉलबैक मॉडल",
      "voice": "आवाज़",
      "voiceDesc": "टेक्स्ट-टू-स्पीच प्लेबैक",
      "advanced": "उन्नत",
      "advancedDesc": "इस सत्र के लिए मॉडल पैरामीटर ओवरराइड करें",
      "session": "सत्र",
      "sessionDesc": "नई चैट शुरू करें और इतिहास ब्राउज़ करें",
      "newChat": "नई चैट",
      "newChatDesc": "एक नई बातचीत शुरू करें",
      "chatHistoryDesc": "पिछले सत्र देखें",
      "importChatPackageDesc": "इस कैरेक्टर में एक .chatpkg आयात करें",
      "selectModel": "मॉडल चुनें",
      "selectFallbackModel": "फ़ॉलबैक मॉडल चुनें",
      "personaActions": "पर्सोना कार्रवाइयाँ",
      "sessionAdvancedSettings": "सत्र उन्नत सेटिंग्स",
      "parameterSupport": "पैरामीटर समर्थन",
      "backToChat": "चैट पर वापस",
      "chatSettingsTitle": "चैट सेटिंग्स",
      "chatSettingsSubtitle": "बातचीत की प्राथमिकताएँ प्रबंधित करें",
      "modelDefaults": "मॉडल डिफ़ॉल्ट",
      "appDefaults": "ऐप डिफ़ॉल्ट",
      "openChatSessionFirst": "पहले एक चैट सत्र खोलें",
      "sessionRequired": "सत्र आवश्यक",
      "noPersona": "कोई पर्सोना नहीं",
      "customPersona": "कस्टम पर्सोना",
      "noModelAvailable": "कोई मॉडल उपलब्ध नहीं",
      "fallbackNone": "कोई नहीं",
      "unknownModel": "अज्ञात मॉडल",
      "authorNote": "Author Note",
      "identityProfileAuthored": "Identity profile तैयार किया गया",
      "addIdentityProfile": "Companion identity profile जोड़ें",
      "soulLabel": "Soul",
      "sessionTitle": "सत्र: {{title}}",
      "sessionUntitled": "बिना शीर्षक",
      "messageCount": "{{count}} संदेश",
      "usingCharacterDefault": "कैरेक्टर डिफ़ॉल्ट उपयोग कर रहे हैं",
      "sessionOverrideActive": "सत्र ओवरराइड सक्रिय",
      "autoplayVoice": "आवाज़ ऑटोप्ले",
      "useCharacterDefault": "कैरेक्टर डिफ़ॉल्ट उपयोग करें"
    },
    "search": {
      "placeholder": "बातचीत खोजें...",
      "noMessagesFound": "कोई संदेश नहीं मिला",
      "you": "आप",
      "character": "कैरेक्टर",
      "failed": "संदेश खोजने में विफल"
    },
    "templates": {
      "startWithTemplate": "टेम्पलेट से शुरू करें?",
      "noTemplate": "कोई टेम्पलेट नहीं",
      "startWithSceneOnly": "केवल दृश्य से शुरू करें",
      "you": "आप",
      "bot": "बॉट",
      "messageCount": "{{count}} संदेश"
    },
    "header": {
      "back": "वापस",
      "openSettings": "चैट सेटिंग्स खोलें",
      "manageMemories": "यादें प्रबंधित करें",
      "searchMessages": "संदेश खोजें",
      "manageLorebooks": "लोरबुक्स प्रबंधित करें",
      "conversationSettings": "बातचीत सेटिंग्स"
    },
    "footer": {
      "sendMessagePlaceholder": "एक संदेश भेजें...",
      "stopGeneration": "जनरेशन रोकें",
      "sendMessage": "संदेश भेजें",
      "continueConversation": "बातचीत जारी रखें",
      "moreOptions": "अधिक विकल्प",
      "addImage": "छवि जोड़ें",
      "addImageAttachment": "छवि अटैचमेंट जोड़ें",
      "removeAttachment": "अटैचमेंट हटाएँ",
      "recordVoice": "आवाज़ रिकॉर्ड करें"
    },
    "message": {
      "thinkingMessages": {
        "thinkingReallyHard": "बहुत गहराई से सोच रहा है…",
        "lettuceCouncil": "लेट्यूस काउंसिल से सलाह ले रहा है…",
        "stealingThoughts": "शून्य से विचार चुरा रहा है…",
        "warmingBrainCells": "मस्तिष्क कोशिकाओं को गर्म कर रहा है…",
        "forbiddenKnowledge": "निषिद्ध ज्ञान लोड हो रहा है…",
        "overthinking": "ज़रूरत से ज़्यादा सोच रहा है (हमेशा की तरह)…",
        "pretendingToBeSmart": "बुद्धिमान होने का नाटक कर रहा है…",
        "crunchingNumbers": "काल्पनिक संख्याएँ गिन रहा है…",
        "arguingWithMyself": "अपने आप से बहस कर रहा है…",
        "askingUniverse": "ब्रह्मांड से विनम्रता से पूछ रहा है…"
      },
      "thoughtProcess": "विचार प्रक्रिया",
      "regenerateResponse": "उत्तर पुनः जनरेट करें",
      "cancelAudioGeneration": "ऑडियो जनरेशन रद्द करें",
      "stopAudio": "ऑडियो रोकें",
      "playMessageAudio": "संदेश ऑडियो चलाएँ",
      "playAudio": "ऑडियो चलाएँ",
      "sceneLabel": "दृश्य",
      "variantLabel": "वेरिएंट",
      "regenerating": "पुनः जनरेट हो रहा है",
      "assistantIsTyping": "सहायक टाइप कर रहा है",
      "attachedImage": "संलग्न छवि",
      "guidedRegenerationTitle": "पुनः जनरेशन को गाइड करें",
      "guidedRegenerationLabel": "इसे कैसे बदलना चाहिए?",
      "guidedRegenerationDescription": "स्वर, लंबाई, रखने या हटाने योग्य विवरण और अगले उत्तर को अलग तरह से क्या करना चाहिए, इसका वर्णन करें।",
      "guidedRegenerationPlaceholder": "इसे छोटा, गर्म, अधिक सीधा बनाएँ...",
      "guidedRegenerationSubmit": "Regenerate"
    },
    "actions": {
      "assistantMessage": "सहायक संदेश",
      "userMessage": "उपयोगकर्ता संदेश",
      "promptTokens": "प्रॉम्प्ट टोकन",
      "completionTokens": "कंप्लीशन टोकन",
      "fallbackModelUsed": "फ़ॉलबैक मॉडल उपयोग किया गया",
      "total": "कुल",
      "timeToFirstToken": "पहले टोकन का समय",
      "completionTokenSpeed": "कंप्लीशन टोकन गति",
      "edit": "संपादित करें",
      "copy": "कॉपी करें",
      "pin": "पिन करें",
      "unpin": "अनपिन करें",
      "rewindToHere": "यहाँ तक रीवाइंड करें",
      "branchFromHere": "यहाँ से शाखा बनाएँ",
      "branchToGroupChat": "ग्रुप चैट में शाखा बनाएँ",
      "branchToCharacter": "कैरेक्टर में शाखा बनाएँ",
      "generateSceneImage": "दृश्य छवि उत्पन्न करें",
      "regenerateSceneImage": "दृश्य छवि पुन: उत्पन्न करें",
      "chatAppearance": "चैट दिखावट",
      "delete": "हटाएँ",
      "debug": "डीबग",
      "unpinToDelete": "हटाने के लिए अनपिन करें",
      "editPlaceholder": "अपना संदेश संपादित करें...",
      "memoriesUsed": "{{count}} यादें उपयोग की गईं",
      "lorebookUsage": "लोरबुक उपयोग",
      "lorebookUsageDesc": "इस उत्तर ने निम्नलिखित लोरबुक प्रविष्टियों का उपयोग किया।",
      "matchScore": "मेल: {{score}}%",
      "unknownModel": "अज्ञात मॉडल",
      "loadingModel": "मॉडल लोड हो रहा है..."
    },
    "emptyState": {
      "goBack": "वापस जाएँ"
    },
    "settingsDrawer": {
      "title": "चैट सेटिंग्स",
      "subtitle": "बातचीत की प्राथमिकताएँ प्रबंधित करें"
    },
    "history": {
      "archivedBadge": "संग्रहीत",
      "messagesCount": "{{count}} संदेश",
      "previousGroupPage": "पिछला {{label}} पेज",
      "nextGroupPage": "अगला {{label}} पेज",
      "sillyTavernFormat": "SillyTavern format",
      "sillyTavernFormatDesc": "SillyTavern के साथ compatible .jsonl के रूप में निर्यात करें।",
      "failedLoad": "डेटा लोड करने में विफल",
      "failedDelete": "हटाने में विफल: {{error}}",
      "failedRename": "नाम बदलने में विफल: {{error}}",
      "chatPackageExportedTo": "चैट पैकेज यहाँ निर्यात किया:\n{{path}}",
      "sillyTavernExportedTo": "SillyTavern चैट यहाँ निर्यात की:\n{{path}}",
      "failedExportChatPackage": "चैट पैकेज निर्यात करने में विफल",
      "failedExportSillyTavern": "SillyTavern चैट निर्यात करने में विफल"
    },
    "authorNote": {
      "title": "Author Note",
      "inlineEditor": "Inline editor",
      "inlineEditorDesc": "त्वरित संपादन के लिए चैट इनपुट के ऊपर author note दिखाएँ।",
      "toggleInlineEditor": "Inline author note editor टॉगल करें",
      "placeholder": "इस चैट के लिए निजी दिशा",
      "willBeRemoved": "सेव करने पर Author note हटाया जाएगा",
      "noNoteSaved": "कोई author note सेव नहीं",
      "charactersCount": "{{count}} अक्षर",
      "clear": "साफ़ करें",
      "save": "सेव करें",
      "saving": "सेव हो रहा है...",
      "failedSave": "Author note सेव करने में विफल",
      "addPlaceholder": "Author note जोड़ें...",
      "savingShort": "सेव हो रहा है..."
    },
    "drawer": {
      "chatSettingsTitle": "चैट सेटिंग्स",
      "chatSettingsSubtitle": "बातचीत की प्राथमिकताएँ प्रबंधित करें"
    },
    "companionSoul": {
      "loading": "Companion Soul लोड हो रहा है...",
      "unavailable": "Companion Soul उपलब्ध नहीं है",
      "unavailableDesc": "Soul संपादन केवल companion-mode कैरेक्टर के लिए उपलब्ध है।",
      "pageTitle": "Companion Soul",
      "back": "वापस",
      "save": "सेव करें"
    },
    "companionRelationship": {
      "back": "वापस",
      "loading": "संबंध स्थिति लोड हो रही है...",
      "unavailableTitle": "संबंध स्थिति उपलब्ध नहीं है",
      "sessionLoadFailed": "चैट सत्र लोड नहीं हो सका।",
      "backToChat": "चैट पर वापस",
      "notCompanionTitle": "यह चैट companion mode में नहीं है",
      "notCompanionDesc": "Companion relationship पेज केवल उन चैट के लिए रेंडर होते हैं जिनका कैरेक्टर mode companion है।",
      "openRegularMemories": "नियमित यादें खोलें",
      "pageTitle": "संबंध स्थिति",
      "memoryButton": "Memory",
      "lastInteraction": "अंतिम इंटरेक्शन {{time}}",
      "bond": "Bond",
      "closeness": "Closeness",
      "trust": "Trust",
      "affection": "Affection",
      "tension": "Tension",
      "stability": "Stability",
      "interactions": "Interactions",
      "vsDefaults": "बनाम कैरेक्टर डिफ़ॉल्ट",
      "updatedAt": "{{time}} पर अपडेट किया",
      "emotionalEngine": "Emotional engine",
      "felt": "Felt",
      "feltDesc": "Internal affect",
      "expressed": "Expressed",
      "expressedDesc": "उत्तरों में दिखता है",
      "blocked": "Blocked",
      "blockedDesc": "Persona द्वारा दबाया गया",
      "momentum": "Momentum",
      "momentumDesc": "हाल के turns में रुझान",
      "activeDrivers": "Active drivers",
      "soul": "Soul",
      "essence": "Essence",
      "voice": "Voice",
      "relationalStyle": "Relational style",
      "vulnerabilities": "Vulnerabilities",
      "habits": "Habits",
      "boundaries": "Boundaries",
      "eventsCount": "{{count}} घटनाएँ",
      "recentTimeline": "हालिया timeline",
      "superseded": "superseded",
      "promptScore": "Prompt {{score}}",
      "persistenceScore": "Persistence {{score}}",
      "noTimeline": "अभी तक कोई timeline नहीं",
      "noTimelineDesc": "संबंध, milestone, और भावनात्मक snapshot memories यहाँ दिखाई देंगी जैसे-जैसे companion बातचीत से सीखता है।",
      "notAuthoredYet": "अभी तक तैयार नहीं।",
      "noSignal": "कोई signal नहीं।"
    },
    "companionUi": {
      "relationship": "Relationship",
      "milestones": "Milestones",
      "boundaries": "Boundaries",
      "preferences": "Preferences",
      "profile": "Profile",
      "routines": "Routines",
      "episodes": "Episodes",
      "emotionalSnapshots": "Emotional snapshots",
      "unknownTime": "अज्ञात",
      "justNow": "अभी",
      "minutesAgo": "{{minutes}} मिनट पहले",
      "hoursAgo": "{{hours}} घंटे पहले",
      "daysAgo": "{{days}} दिन पहले",
      "notSetYet": "अभी सेट नहीं",
      "missingCharacterId": "characterId गायब है",
      "sessionNotFound": "सत्र नहीं मिला",
      "failedLoadCompanion": "Companion सत्र लोड करने में विफल"
    },
    "chatPage": {
      "characterNotFound": "कैरेक्टर नहीं मिला",
      "characterDoesntExist": "आप जिस कैरेक्टर को ढूँढ रहे हैं वह मौजूद नहीं है।"
    },
    "debugPage": {
      "copy": "कॉपी करें"
    },
    "companionMemoryPage": {
      "backLabel": "वापस",
      "refineMemoryPlaceholder": "संग्रहीत companion memory को परिष्कृत करें",
      "superseded": "superseded",
      "pinned": "पिन किया गया",
      "cold": "cold"
    }
  },
  "groupChats": {
    "list": {
      "editGroup": "ग्रुप संपादित करें",
      "deleteGroup": "ग्रुप हटाएँ",
      "deleteConfirmTitle": "ग्रुप चैट हटाएँ?",
      "deleteConfirmMessage": "क्या आप वाकई \"{{name}}\" को हटाना चाहते हैं? इससे इस ग्रुप चैट के सभी संदेश भी हट जाएँगे।",
      "noGroupChatsYet": "अभी तक कोई ग्रुप चैट नहीं",
      "noGroupChatsDesc": "एक साथ कई कैरेक्टर्स के साथ बातचीत करने के लिए नीचे + बटन से अपना पहला ग्रुप चैट बनाएँ",
      "newChat": "नई चैट",
      "openChat": "चैट खोलें",
      "chatSettings": "चैट सेटिंग्स",
      "sessionCount": "{{count}} चैट"
    },
    "create": {
      "invalidPackage": "यह पैकेज ग्रुप चैट पैकेज नहीं है।",
      "inspectPackageError": "ग्रुप चैट पैकेज निरीक्षण में विफल",
      "importPackageError": "ग्रुप चैट पैकेज आयात में विफल",
      "importChatpkg": "`.chatpkg` आयात करें",
      "mapParticipantsTitle": "प्रतिभागी मैप करें",
      "selectLocalCharacter": "इस प्रतिभागी के लिए स्थानीय कैरेक्टर चुनें।",
      "selectCharacterPlaceholder": "कैरेक्टर चुनें...",
      "importChatPackageTitle": "चैट पैकेज आयात करें",
      "importChatPackageDesc": "यह चयनित `.chatpkg` को एक नए ग्रुप सत्र के रूप में आयात करेगा।",
      "characterSelect": {
        "title": "ग्रुप चैट बनाएँ",
        "subtitle": "अपनी ग्रुप बातचीत के लिए कैरेक्टर चुनें",
        "selected": "चयनित",
        "ready": "तैयार",
        "minRequired": "न्यूनतम 2 आवश्यक",
        "noCharactersYet": "अभी तक कोई कैरेक्टर नहीं",
        "noCharactersDesc": "ग्रुप चैट शुरू करने के लिए पहले कुछ कैरेक्टर बनाएँ",
        "continueToSetup": "ग्रुप सेटअप पर जारी रखें"
      },
      "groupSetup": {
        "title": "ग्रुप सेटअप",
        "subtitle": "अपनी ग्रुप चैट सेटिंग्स कॉन्फ़िगर करें",
        "chatType": "चैट प्रकार",
        "conversation": "बातचीत",
        "casualChat": "सामान्य चैट",
        "roleplay": "रोलप्ले",
        "withScenes": "दृश्यों के साथ",
        "conversationDesc": "शुरुआती दृश्यों के बिना सामान्य ग्रुप बातचीत",
        "roleplayDesc": "शुरुआती दृश्य और इमर्सिव प्रॉम्प्ट के साथ रोलप्ले परिदृश्य",
        "speakerSelection": "वक्ता चयन",
        "llm": "LLM",
        "aiPicks": "AI चुनता है",
        "heuristic": "ह्यूरिस्टिक",
        "scoreBased": "स्कोर-आधारित",
        "roundRobin": "राउंड रॉबिन",
        "takeTurns": "बारी-बारी से",
        "llmDesc": "कौन बोलेगा चुनने के लिए आपके डिफ़ॉल्ट मॉडल का उपयोग करता है (टोकन खर्च होते हैं)",
        "heuristicDesc": "भागीदारी संतुलन और संदर्भ संकेतों का उपयोग करता है (मुफ्त)",
        "roundRobinDesc": "कैरेक्टर क्रम में बारी लेते हैं (मुफ्त)",
        "chatBackground": "चैट पृष्ठभूमि",
        "optional": "(वैकल्पिक)",
        "uploadBackground": "पृष्ठभूमि छवि अपलोड करें",
        "backgroundDesc": "इस ग्रुप चैट के लिए एक पृष्ठभूमि छवि सेट करें",
        "groupName": "ग्रुप का नाम",
        "removeBackground": "पृष्ठभूमि छवि हटाएँ",
        "groupNameAutoGenerate": "कैरेक्टर नामों से स्वतः जनरेट करने के लिए खाली छोड़ें",
        "continueToScene": "शुरुआती दृश्य पर जारी रखें",
        "createGroupChat": "ग्रुप चैट बनाएँ"
      },
      "startingScene": {
        "title": "शुरुआती दृश्य",
        "subtitle": "अपने रोलप्ले के लिए शुरुआती परिदृश्य सेट करें",
        "sceneSource": "दृश्य स्रोत",
        "none": "कोई नहीं",
        "custom": "कस्टम",
        "fromCharacter": "कैरेक्टर से",
        "noneDesc": "पूर्वनिर्धारित दृश्य के बिना शुरू करें",
        "customDesc": "अपना शुरुआती परिदृश्य लिखें",
        "fromCharacterDesc": "अपने किसी कैरेक्टर का दृश्य उपयोग करें",
        "sceneContent": "दृश्य सामग्री",
        "sceneContentPlaceholder": "इस रोलप्ले के लिए शुरुआती दृश्य का वर्णन करें...",
        "sceneReferenceTip": "टिप: कैरेक्टर्स को संदर्भित करने के लिए {{@\" टाइप करें",
        "selectScene": "एक दृश्य चुनें",
        "sceneLabel": " का दृश्य",
        "copyToCustom": "कस्टम में कॉपी करें और संपादित करें"
      }
    },
    "history": {
      "title": "ग्रुप चैट इतिहास",
      "subtitle": "सभी ग्रुप बातचीत",
      "searchPlaceholder": "ग्रुप चैट खोजें...",
      "active": "सक्रिय ({{count}})",
      "archived": "संग्रहीत ({{count}})",
      "noChatsYet": "अभी तक कोई ग्रुप चैट नहीं",
      "noChatsDesc": "अपना इतिहास यहाँ देखने के लिए एक ग्रुप चैट बनाएँ",
      "noMatchingChats": "कोई मेल खाती चैट नहीं",
      "noMatchingDesc": "कोई भिन्न खोज शब्द आज़माएँ",
      "deleteSessionTitle": "ग्रुप चैट हटाएँ?",
      "deleteSessionDesc": "इसे इतिहास से स्थायी रूप से हटा दिया जाएगा",
      "deleteSessionButton": "चैट हटाएँ",
      "keepChat": "यह चैट रखें"
    },
    "session": {
      "chatTitlePlaceholder": "चैट शीर्षक...",
      "newChat": "नई चैट",
      "rename": "नाम बदलें",
      "unarchive": "असंग्रहीत करें",
      "archive": "संग्रहीत करें",
      "messageCount": "{{count}} संदेश"
    },
    "memories": {
      "tabMemories": "यादें",
      "tabPinned": "पिन किए गए",
      "tabActivity": "गतिविधि",
      "processing": "प्रोसेसिंग",
      "contextSummaryTitle": "संदर्भ सारांश",
      "addContextSummaryPrompt": "संदर्भ सारांश जोड़ने के लिए टैप करें...",
      "savedMemories": "सहेजी गई यादें",
      "resultsCount": "परिणाम ({{count}})",
      "searchPlaceholder": "यादें खोजें...",
      "addMemory": "याद जोड़ें",
      "noMemoriesYet": "अभी तक कोई याद नहीं",
      "noMemoriesDesc": "एक बनाने के लिए ऊपर जोड़ें बटन पर टैप करें",
      "noMatchingMemories": "कोई मेल खाती याद नहीं",
      "noMatchingDesc": "कोई भिन्न खोज शब्द आज़माएँ",
      "sessionNotFound": "सत्र नहीं मिला",
      "memoryActions": "याद कार्रवाइयाँ",
      "tokens": "टोकन",
      "cycle": "चक्र",
      "accessed": "एक्सेस किया गया",
      "cold": "कोल्ड",
      "hot": "हॉट",
      "activityLog": "गतिविधि लॉग",
      "events": "घटनाएँ",
      "run": "चलाएँ",
      "processingMemories": "AI ग्रुप यादों को व्यवस्थित कर रहा है...",
      "memoryCycleSuccess": "मेमोरी चक्र सफलतापूर्वक प्रोसेस हुआ!",
      "memoryActionFailed": "याद कार्रवाई विफल",
      "newMemoryUpdates": "नई याद अपडेट उपलब्ध",
      "noActivityYet": "अभी तक कोई गतिविधि नहीं",
      "noActivityDesc": "डायनामिक मोड में AI द्वारा यादें प्रबंधित करने पर टूल कॉल दिखाई देते हैं",
      "contextSummaryPlaceholder": "संदेशों में संदर्भ सुसंगत रखने के लिए संक्षिप्त सारांश...",
      "addMemoryTitle": "याद जोड़ें",
      "memoryPlaceholder": "क्या याद रखना चाहिए?",
      "saveMemory": "याद सेव करें",
      "editMemoryTitle": "याद संपादित करें",
      "editMemoryPlaceholder": "याद सामग्री दर्ज करें...",
      "edit": "संपादित करें",
      "pin": "पिन करें",
      "unpin": "अनपिन करें",
      "setHot": "हॉट सेट करें",
      "setCold": "कोल्ड सेट करें"
    },
    "toolLog": {
      "created": "बनाया गया",
      "deleted": "हटाया गया",
      "pinned": "पिन किया गया",
      "unpinned": "अनपिन किया गया",
      "done": "पूर्ण",
      "pinnedBadge": "पिन किया गया",
      "softDelete": "सॉफ्ट-डिलीट",
      "memoryCycle": "मेमोरी चक्र",
      "failedAt": "विफल हुआ:",
      "window": "विंडो",
      "justNow": "अभी",
      "minutesAgo": "{{count}} मिनट पहले",
      "hoursAgo": "{{count}} घंटे पहले",
      "yesterday": "कल",
      "daysAgo": "{{count}} दिन पहले",
      "revertingTitle": "वापस कर रहा है...",
      "revertCycleTitle": "इस चक्र को वापस करें",
      "revertedAt": "{{time}} पर वापस किया",
      "failedAtStage": "विफल हुआ: {{stage}}",
      "hideDebug": "Debug छिपाएँ",
      "debug": "Debug",
      "windowRange": "विंडो {{start}}-{{end}}",
      "actionCreated": "बनाया गया",
      "actionDeleted": "हटाया गया",
      "actionPinned": "पिन किया गया",
      "actionUnpinned": "अनपिन किया गया",
      "actionDone": "पूर्ण",
      "badgePinned": "पिन किया गया",
      "badgeSoftDelete": "सॉफ्ट-डिलीट",
      "badgeUndone": "पूर्ववत",
      "badgeReverted": "वापस किया गया",
      "activityEmptyTitle": "अभी तक कोई गतिविधि नहीं",
      "activityEmptyDesc": "डायनामिक मोड में AI द्वारा यादें प्रबंधित करने पर टूल कॉल दिखाई देते हैं"
    },
    "message": {
      "thinkingHard": "बहुत गहराई से सोच रहा है…",
      "thinkingLettuce": "लेट्यूस काउंसिल से सलाह ले रहा है…",
      "thinkingVoid": "शून्य से विचार चुरा रहा है…",
      "thinkingBrainCells": "मस्तिष्क कोशिकाओं को गर्म कर रहा है…",
      "thinkingForbidden": "निषिद्ध ज्ञान लोड हो रहा है…",
      "thinkingOverthinking": "ज़रूरत से ज़्यादा सोच रहा है (हमेशा की तरह)…",
      "thinkingPretending": "बुद्धिमान होने का नाटक कर रहा है…",
      "thinkingCrunching": "काल्पनिक संख्याएँ गिन रहा है…",
      "thinkingArguing": "अपने आप से बहस कर रहा है…",
      "thinkingUniverse": "ब्रह्मांड से विनम्रता से पूछ रहा है…",
      "thoughtProcess": "विचार प्रक्रिया",
      "userAlt": "उपयोगकर्ता",
      "assistantAlt": "सहायक",
      "regenerateResponse": "उत्तर पुनः जनरेट करें",
      "variantLabel": "वेरिएंट",
      "regenerating": "पुनः जनरेट हो रहा है",
      "cancelAudioGeneration": "ऑडियो जनरेशन रद्द करें",
      "stopAudio": "ऑडियो रोकें",
      "playMessageAudio": "संदेश ऑडियो चलाएँ",
      "playAudio": "ऑडियो चलाएँ",
      "attachedImage": "संलग्न छवि",
      "assistantIsTyping": "सहायक टाइप कर रहा है",
      "assistantTyping": "सहायक टाइप कर रहा है"
    },
    "header": {
      "back": "वापस",
      "memories": "यादें",
      "settings": "सेटिंग्स",
      "characters": "कैरेक्टर"
    },
    "footer": {
      "mentionCharacter": "कैरेक्टर का उल्लेख करें",
      "noCharactersFound": "कोई कैरेक्टर नहीं मिला",
      "moreOptions": "अधिक विकल्प",
      "addImage": "छवि जोड़ें",
      "messagePlaceholder": "संदेश... (उल्लेख के लिए @)",
      "stopGeneration": "जनरेशन रोकें",
      "sendMessage": "संदेश भेजें",
      "continueConversation": "बातचीत जारी रखें",
      "dismissError": "त्रुटि खारिज करें",
      "removeAttachment": "अटैचमेंट हटाएँ",
      "tabToSelect": "चयन के लिए Tab दबाएँ"
    },
    "messageActions": {
      "characterMessage": "कैरेक्टर संदेश",
      "yourMessage": "आपका संदेश",
      "whyCharacterResponded": "इस कैरेक्टर ने क्यों जवाब दिया",
      "total": "कुल",
      "edit": "संपादित करें",
      "copy": "कॉपी करें",
      "regenerateWithDifferent": "दूसरे कैरेक्टर के साथ पुनः जनरेट करें",
      "rewindToHere": "यहाँ तक रीवाइंड करें",
      "unpinToDelete": "हटाने के लिए अनपिन करें",
      "delete": "हटाएँ",
      "editPlaceholder": "अपना संदेश संपादित करें...",
      "chooseCharacterTitle": "कैरेक्टर चुनें",
      "selectCharacterForRegeneration": "चुनें कि कौन सा कैरेक्टर इसके बजाय जवाब दे:"
    },
    "settings": {
      "appDefault": "ऐप डिफ़ॉल्ट",
      "selectPersona": "पर्सोना चुनें",
      "noPersonas": "कोई पर्सोना उपलब्ध नहीं",
      "noPersonasDesc": "अपनी बातचीत को व्यक्तिगत बनाने के लिए सेटिंग्स में एक पर्सोना बनाएँ।",
      "searchPersonas": "पर्सोना खोजें...",
      "noPersona": "कोई पर्सोना नहीं",
      "noPersonaDesc": "बिना पर्सोना के जारी रखें",
      "noPersonasFound": "कोई पर्सोना नहीं मिला",
      "noPersonasFoundDesc": "कोई भिन्न खोज शब्द आज़माएँ"
    },
    "groupSettings": {
      "title": "समूह संपादित करें",
      "subtitle": "भविष्य के सत्रों के लिए डिफ़ॉल्ट सेटअप अपडेट करें",
      "enterGroupName": "समूह का नाम दर्ज करें",
      "participant": "प्रतिभागी",
      "participants": "प्रतिभागी",
      "uploading": "अपलोड हो रहा है...",
      "changeBackground": "पृष्ठभूमि बदलें",
      "addBackgroundImage": "पृष्ठभूमि छवि जोड़ें",
      "removeBackground": "पृष्ठभूमि हटाएँ",
      "persona": "व्यक्तित्व",
      "personaSubtitle": "नए सत्रों के लिए डिफ़ॉल्ट व्यक्तित्व",
      "personaLabel": "व्यक्तित्व",
      "speakerSelection": "वक्ता चयन",
      "speakerSubtitle": "नए सत्रों के लिए डिफ़ॉल्ट विधि",
      "llm": "LLM",
      "aiPicks": "AI चुनता है",
      "heuristic": "अनुमानी",
      "scoreBased": "अंक-आधारित",
      "roundRobin": "बारी-बारी",
      "takeTurns": "बारी लें",
      "llmDesc": "बोलने वाले को चुनने के लिए आपके डिफ़ॉल्ट मॉडल का उपयोग करता है (टोकन खर्च होते हैं)",
      "heuristicDesc": "भागीदारी संतुलन और संदर्भ संकेतों का उपयोग करता है (मुफ़्त)",
      "roundRobinDesc": "पात्र क्रम में बारी-बारी बोलते हैं (मुफ़्त)",
      "memoryMode": "मेमोरी मोड",
      "memorySubtitle": "नए सत्रों के लिए डिफ़ॉल्ट मेमोरी मोड",
      "manual": "मैनुअल",
      "manualDesc": "नोट्स स्वयं प्रबंधित करें",
      "dynamic": "डायनामिक",
      "dynamicDesc": "स्वचालित रिकॉल",
      "memoryDynamicInfo": "AI स्वचालित रूप से बातचीत से यादें बनाता और पुनः प्राप्त करता है",
      "memoryManualInfo": "आप स्वयं मेमोरी नोट्स जोड़ते और प्रबंधित करते हैं",
      "characters": "पात्र",
      "participantsActive": "{{total}} प्रतिभागी · {{active}} सक्रिय",
      "add": "जोड़ें",
      "muted": "(म्यूट)",
      "mutedByDefault": "डिफ़ॉल्ट रूप से म्यूट",
      "activeByDefault": "डिफ़ॉल्ट रूप से सक्रिय",
      "unmuteCharacter": "पात्र अनम्यूट करें",
      "muteCharacter": "पात्र म्यूट करें",
      "minTwoRequired": "कम से कम 2 पात्र आवश्यक",
      "removeCharacter": "पात्र हटाएँ",
      "groupMinCharacters": "एक समूह में कम से कम 2 पात्र आवश्यक हैं",
      "mutedCharactersNote": "म्यूट किए गए पात्रों को स्वचालित वक्ता चयन द्वारा छोड़ दिया जाता है, लेकिन वे स्पष्ट `@उल्लेख` के माध्यम से जवाब दे सकते हैं।",
      "addCharacterTitle": "पात्र जोड़ें",
      "allCharactersInGroup": "सभी पात्र पहले से इस समूह में हैं।",
      "removeCharacterTitle": "पात्र हटाएँ?",
      "removeCharacterConfirm": "क्या आप वाकई हटाना चाहते हैं",
      "removeCharacterFrom": "को समूह डिफ़ॉल्ट से?",
      "removing": "हटाया जा रहा है...",
      "remove": "हटाएँ"
    },
    "sessionSettings": {
      "subtitle": "समूह चैट प्राथमिकताएँ प्रबंधित करें",
      "enterGroupName": "समूह का नाम दर्ज करें",
      "participant": "प्रतिभागी",
      "participants": "प्रतिभागी",
      "message": "संदेश",
      "messages": "संदेश",
      "uploading": "अपलोड हो रहा है...",
      "changeBackground": "पृष्ठभूमि बदलें",
      "addBackgroundImage": "पृष्ठभूमि छवि जोड़ें",
      "removeBackground": "पृष्ठभूमि हटाएँ",
      "persona": "व्यक्तित्व",
      "personaSubtitle": "इस वार्तालाप में आपकी पहचान",
      "personaLabel": "व्यक्तित्व",
      "speakerSelection": "वक्ता चयन",
      "speakerSubtitle": "अगला वक्ता कैसे चुना जाता है",
      "llm": "LLM",
      "aiPicks": "AI चुनता है",
      "heuristic": "अनुमानी",
      "scoreBased": "अंक-आधारित",
      "roundRobin": "बारी-बारी",
      "takeTurns": "बारी लें",
      "llmDesc": "बोलने वाले को चुनने के लिए आपके डिफ़ॉल्ट मॉडल का उपयोग करता है (टोकन खर्च होते हैं)",
      "heuristicDesc": "भागीदारी संतुलन और संदर्भ संकेतों का उपयोग करता है (मुफ़्त)",
      "roundRobinDesc": "पात्र क्रम में बारी-बारी बोलते हैं (मुफ़्त)",
      "characters": "पात्र",
      "participantsActive": "{{total}} प्रतिभागी · {{active}} सक्रिय",
      "add": "जोड़ें",
      "muted": "(म्यूट)",
      "unmuteCharacter": "पात्र अनम्यूट करें",
      "muteCharacter": "पात्र म्यूट करें",
      "minTwoRequired": "कम से कम 2 पात्र आवश्यक",
      "removeCharacter": "पात्र हटाएँ",
      "groupMinCharacters": "एक समूह चैट में कम से कम 2 पात्र आवश्यक हैं",
      "mutedCharactersNote": "म्यूट किए गए पात्रों को स्वचालित वक्ता चयन द्वारा छोड़ दिया जाता है, लेकिन वे स्पष्ट `@उल्लेख` के माध्यम से जवाब दे सकते हैं।",
      "data": "डेटा",
      "dataSubtitle": "वार्तालाप निर्यात या आयात करें",
      "export": "निर्यात",
      "exportDesc": "साझा करने योग्य फ़ाइल के रूप में सहेजें",
      "import": "आयात",
      "importDesc": "फ़ाइल से वार्तालाप लोड करें",
      "conversation": "वार्तालाप",
      "conversationSubtitle": "डुप्लिकेट करें या नई चैट में जारी रखें",
      "duplicate": "डुप्लिकेट",
      "duplicateDesc": "इस चैट को संदेशों के साथ या बिना कॉपी करें",
      "branchTo1on1": "1-से-1 में शाखा बनाएँ",
      "branchTo1on1Desc": "एक पात्र के साथ निजी रूप से जारी रखें",
      "participation": "भागीदारी",
      "participationSubtitle": "पात्रों के बीच बोलने का वितरण",
      "addCharacterTitle": "पात्र जोड़ें",
      "allCharactersInGroup": "सभी पात्र पहले से इस समूह में हैं।",
      "removeCharacterTitle": "पात्र हटाएँ?",
      "removeCharacterConfirm": "क्या आप वाकई हटाना चाहते हैं",
      "removeCharacterFrom": "को इस समूह चैट से?",
      "removing": "हटाया जा रहा है...",
      "remove": "हटाएँ",
      "cloneGroupTitle": "समूह क्लोन करें",
      "withMessages": "संदेशों के साथ",
      "withMessagesDesc": "चैट इतिहास सहित सब कुछ क्लोन करें",
      "withoutMessages": "संदेशों के बिना",
      "withoutMessagesDesc": "केवल सेटअप क्लोन करें (पात्र, आरंभिक दृश्य)",
      "branchWithCharacterTitle": "पात्र के साथ शाखा बनाएँ",
      "branchWithCharacterDesc": "1-से-1 वार्तालाप के रूप में जारी रखने के लिए एक पात्र चुनें। इस समूह के सभी संदेश परिवर्तित किए जाएँगे।",
      "continueWith": "{{name}} के साथ वार्तालाप जारी रखें",
      "exportChatPackageTitle": "चैट पैकेज निर्यात करें",
      "includeCharacterSnapshots": "पात्र स्नैपशॉट शामिल करें",
      "includeCharacterSnapshotsDesc": "पैकेज में पात्र डेटा रखें",
      "sessionOnly": "केवल सत्र",
      "sessionOnlyDesc": "केवल संदेश और मेटाडेटा निर्यात करें",
      "mapParticipantsTitle": "प्रतिभागियों को मैप करें",
      "selectLocalCharacter": "इस प्रतिभागी के लिए स्थानीय पात्र चुनें।",
      "selectCharacterPlaceholder": "पात्र चुनें...",
      "continue": "जारी रखें",
      "importChatPackageTitle": "चैट पैकेज आयात करें",
      "importChatPackageDesc": "यह चयनित `.chatpkg` को नए समूह सत्र के रूप में आयात करेगा।",
      "importing": "आयात हो रहा है..."
    },
    "page": {
      "selectingCharacter": "कैरेक्टर चुन रहा है...",
      "sessionNotFound": "ग्रुप सत्र नहीं मिला",
      "backToGroupChats": "ग्रुप चैट पर वापस",
      "startConversation": "{{names}} के साथ बातचीत शुरू करें",
      "scrollToBottom": "नीचे स्क्रॉल करें",
      "addContent": "सामग्री जोड़ें",
      "uploadImage": "छवि अपलोड करें",
      "helpMeReply": "उत्तर देने में मदद करें",
      "helpMeReplyDesc": "AI को सुझाव देने दें",
      "haveDraftPrompt": "आपके पास एक ड्राफ्ट संदेश है। आप कैसे आगे बढ़ना चाहते हैं?",
      "useMyTextAsBase": "मेरे टेक्स्ट को आधार के रूप में उपयोग करें",
      "useMyTextAsBaseDesc": "अपने ड्राफ्ट का विस्तार और सुधार करें",
      "writeSomethingNew": "कुछ नया लिखें",
      "writeSomethingNewDesc": "एक नया उत्तर जनरेट करें",
      "suggestedReply": "सुझाया गया उत्तर",
      "reasoningBeforeWriting": "उत्तर लिखने से पहले तर्क कर रहा है...",
      "writingYourReply": "आपका उत्तर लिख रहा है...",
      "regenerate": "पुनः जनरेट करें",
      "useReply": "उत्तर उपयोग करें",
      "helpMeReplyFailedGeneric": "Help Me Reply विफल।",
      "helpMeReplyFailedGenerate": "Help Me Reply उत्तर जनरेट करने में विफल।",
      "noAudioCaptured": "कोई ऑडियो कैप्चर नहीं।",
      "noWhisperModel": "कोई Whisper मॉडल इंस्टॉल नहीं मिला। Speech Recognition settings में एक इंस्टॉल करें।",
      "cancelRecording": "रिकॉर्डिंग रद्द करें",
      "transcribing": "Transcribe हो रहा है",
      "stopAndTranscribe": "रोकें और Transcribe करें",
      "recordVoice": "आवाज़ रिकॉर्ड करें",
      "learnCorrection": "सुधार सीखें:",
      "learning": "सीख रहा है...",
      "learn": "सीखें",
      "ignore": "अनदेखा करें",
      "groupChatFailed": "ग्रुप चैट विफल।",
      "failedToCopy": "कॉपी करने में विफल",
      "copied": "कॉपी हो गया!",
      "cannotDeletePinned": "पिन किया गया संदेश नहीं हटा सकते। पहले अनपिन करें।",
      "failedToDelete": "हटाने में विफल",
      "messageNotFound": "संदेश नहीं मिला",
      "cannotRewindPinned": "रीवाइंड नहीं कर सकते: इस बिंदु के बाद पिन किए गए संदेश हैं। पहले उन्हें अनपिन करें।",
      "failedToRewind": "रीवाइंड करने में विफल",
      "failedToTogglePin": "Pin टॉगल करने में विफल",
      "messagePinned": "संदेश पिन किया गया",
      "messageUnpinned": "संदेश अनपिन किया गया",
      "failedToSave": "सेव करने में विफल"
    },
    "memoriesPage": {
      "summarizingConversation": "बातचीत का सारांश बना रहा है",
      "analyzingMemories": "यादों का विश्लेषण कर रहा है",
      "applyingChanges": "परिवर्तन लागू कर रहा है",
      "organizingMemories": "यादों को व्यवस्थित कर रहा है",
      "retryingMemoryCycle": "Memory Cycle पुनः प्रयास कर रहा है...",
      "processingMemories": "यादाँ प्रोसेस हो रही हैं...",
      "memorySystemError": "Memory System त्रुटि",
      "contextSummary": "संदर्भ सारांश",
      "contextSummaryTitle": "संदर्भ सारांश",
      "savedMemories": "सहेजी गई यादें",
      "resultsCount": "परिणाम ({{count}})",
      "searchMemoriesPlaceholder": "यादें खोजें...",
      "addMemory": "याद जोड़ें",
      "memoryActions": "याद कार्रवाइयाँ",
      "clearSearch": "खोज साफ़ करें",
      "noMatchingMemories": "कोई मेल खाती याद नहीं",
      "noMemoriesYet": "अभी तक कोई याद नहीं",
      "tryDifferentSearch": "कोई भिन्न खोज शब्द आज़माएँ",
      "tapAddToCreate": "एक बनाने के लिए ऊपर जोड़ें बटन पर टैप करें",
      "pinnedMessages": "पिन किए गए संदेश",
      "refresh": "रीफ़्रेश",
      "noPinnedMessages": "कोई पिन किए गए संदेश नहीं",
      "pinImportantDesc": "महत्वपूर्ण ग्रुप चैट संदेश पिन करें ताकि वे हमेशा संदर्भ में रहें।",
      "assistant": "सहायक",
      "user": "उपयोगकर्ता",
      "unpin": "अनपिन करें",
      "failedToUnpinMessage": "संदेश अनपिन करने में विफल",
      "activityLog": "गतिविधि लॉग",
      "run": "चलाएँ",
      "addMemoryTitle": "याद जोड़ें",
      "editMemoryTitle": "याद संपादित करें",
      "memoryTitle": "याद",
      "memoryPlaceholder": "क्या याद रखना चाहिए?",
      "saveMemory": "याद सेव करें",
      "editMemoryPlaceholder": "याद सामग्री दर्ज करें...",
      "saving": "सेव हो रहा है...",
      "save": "सेव करें",
      "cancel": "रद्द करें",
      "contextSummaryPlaceholder": "संदेशों में संदर्भ सुसंगत रखने के लिए संक्षिप्त सारांश...",
      "contextSummaryPrompt": "संदर्भ सारांश जोड़ने के लिए टैप करें...",
      "cycle": "चक्र",
      "accessed": "एक्सेस किया गया",
      "cold": "कोल्ड",
      "hot": "हॉट",
      "tokens": "टोकन",
      "pin": "पिन करें",
      "setHot": "हॉट सेट करें",
      "setCold": "कोल्ड सेट करें",
      "edit": "संपादित करें",
      "delete": "हटाएँ",
      "failedToToggleMemPin": "Pin टॉगल करने में विफल",
      "failedToRemoveMemory": "याद हटाने में विफल",
      "toolEventsCountAria": "घटनाएँ",
      "activityEmptyDesc": "डायनामिक मोड में AI द्वारा यादें प्रबंधित करने पर टूल कॉल दिखाई देते हैं",
      "memoryCycleSuccess": "मेमोरी चक्र सफलतापूर्वक प्रोसेस हुआ!"
    },
    "messageActionsExtra": {
      "characterMessage": "कैरेक्टर संदेश",
      "yourMessage": "आपका संदेश",
      "whyCharacterResponded": "इस कैरेक्टर ने क्यों जवाब दिया",
      "promptTokensTitle": "Prompt Tokens",
      "completionTokensTitle": "Completion Tokens",
      "total": "कुल",
      "regenerateWithDifferent": "दूसरे कैरेक्टर के साथ पुनः जनरेट करें",
      "unpin": "अनपिन करें",
      "pin": "पिन करें",
      "rewindToHere": "यहाँ तक रीवाइंड करें",
      "unpinToDelete": "हटाने के लिए अनपिन करें",
      "editPlaceholder": "अपना संदेश संपादित करें...",
      "chooseCharacter": "कैरेक्टर चुनें",
      "selectCharacterForRegeneration": "चुनें कि कौन सा कैरेक्टर इसके बजाय जवाब दे:"
    },
    "timeAgo": {
      "justNow": "अभी",
      "minutesAgo": "{{count}} मिनट पहले",
      "hoursAgo": "{{count}} घंटे पहले",
      "daysAgo": "{{count}} दिन पहले"
    },
    "memoriesController": {
      "missingSessionId": "SessionId गायब है",
      "sessionNotFound": "सत्र नहीं मिला",
      "failedToLoadSession": "सत्र लोड करने में विफल",
      "failedToUpdateTemperature": "Memory temperature अपडेट करने में विफल",
      "failedToSaveSummary": "सारांश सेव करने में विफल",
      "cycleFailed": "ग्रुप memory cycle विफल",
      "failedToAddMemory": "याद जोड़ने में विफल",
      "failedToUpdateMemory": "याद अपडेट करने में विफल",
      "failedToRunCycle": "Memory cycle चलाने में विफल",
      "failedToRevertCycle": "Cycle वापस करने में विफल",
      "failedToRefresh": "रीफ़्रेश करने में विफल",
      "failedToDismissError": "त्रुटि खारिज करने में विफल",
      "failedToTogglePinned": "पिन किए गए संदेश टॉगल करने में विफल",
      "sessionNotLoaded": "सत्र लोड नहीं हुआ",
      "revertCycleTitle": "यह cycle वापस करें?",
      "revertConfirm": "वापस करें"
    },
    "chatController": {
      "sessionNotFound": "ग्रुप सत्र नहीं मिला",
      "failedToLoadGroupChat": "ग्रुप चैट लोड करने में विफल",
      "requestFailed": "ग्रुप चैट अनुरोध विफल",
      "failedToSendMessage": "संदेश भेजने में विफल",
      "failedToRegenerate": "पुनः जनरेट करने में विफल",
      "failedToContinue": "जारी रखने में विफल",
      "failedToCopy": "कॉपी करने में विफल",
      "cannotDeletePinned": "पिन किया गया संदेश नहीं हटा सकते। पहले अनपिन करें।",
      "failedToDelete": "हटाने में विफल",
      "messageNotFound": "संदेश नहीं मिला",
      "cannotRewindPinned": "रीवाइंड नहीं कर सकते: इस बिंदु के बाद पिन किए गए संदेश हैं। पहले उन्हें अनपिन करें।",
      "failedToRewind": "रीवाइंड करने में विफल",
      "failedToTogglePin": "Pin टॉगल करने में विफल",
      "messagePinned": "संदेश पिन किया गया",
      "messageUnpinned": "संदेश अनपिन किया गया",
      "failedToSave": "सेव करने में विफल",
      "copied": "कॉपी हो गया!"
    },
    "historyController": {
      "failedToLoadData": "डेटा लोड करने में विफल",
      "failedToDelete": "हटाने में विफल: {{error}}",
      "failedToRename": "नाम बदलने में विफल: {{error}}",
      "failedToArchive": "संग्रहीत करने में विफल: {{error}}",
      "failedToUnarchive": "असंग्रहीत करने में विफल: {{error}}",
      "failedToDuplicate": "डुप्लिकेट करने में विफल"
    },
    "sessionSettingsController": {
      "failedToLoad": "ग्रुप चैट सेटिंग्स लोड करने में विफल",
      "noPersona": "कोई पर्सोना नहीं",
      "customPersona": "कस्टम पर्सोना",
      "minOneActive": "कम से कम एक प्रतिभागी सक्रिय रहना चाहिए"
    },
    "groupSettingsController": {
      "notFound": "ग्रुप नहीं मिला",
      "failedToLoad": "ग्रुप सेटिंग्स लोड करने में विफल"
    },
    "createForm": {
      "failedToLoadCharacters": "कैरेक्टर लोड करने में विफल",
      "selectAtLeastTwo": "ग्रुप चैट के लिए कम से कम 2 कैरेक्टर चुनें",
      "failedToCreate": "ग्रुप चैट बनाने में विफल"
    },
    "groupSetupExtra": {
      "memoryMode": "Memory Mode",
      "manual": "Manual",
      "manualDesc": "नोट्स स्वयं प्रबंधित करें",
      "dynamic": "Dynamic",
      "dynamicDesc": "स्वचालित सारांश और रिकॉल",
      "memoryManualInfo": "आप स्वयं memory नोट्स जोड़ते और प्रबंधित करते हैं",
      "memoryDynamicInfo": "AI स्वचालित रूप से बातचीत से यादें बनाता और पुनः प्राप्त करता है",
      "backgroundPreviewAlt": "बैकग्राउंड पूर्वावलोकन"
    },
    "createExtra": {
      "enterGroupNamePlaceholder": "ग्रुप नाम दर्ज करें...",
      "unknown": "अज्ञात"
    },
    "startingSceneExtra": {
      "insertAs": "{{snippet}} के रूप में डालें"
    },
    "sessionCardExtra": {
      "unknown": "अज्ञात",
      "untitledChat": "बिना शीर्षक चैट"
    },
    "sessionListExtra": {
      "unknown": "अज्ञात"
    },
    "chatHeaderExtra": {
      "unknownError": "अज्ञात त्रुटि"
    },
    "chatSettingsExtra": {
      "invalidPackage": "यह पैकेज ग्रुप चैट पैकेज नहीं है।",
      "failedExport": "ग्रुप चैट पैकेज निर्यात करने में विफल",
      "failedInspect": "ग्रुप चैट पैकेज निरीक्षण करने में विफल",
      "failedImport": "ग्रुप चैट पैकेज आयात करने में विफल",
      "exportSuccess": "ग्रुप चैट पैकेज यहाँ निर्यात किया:\n{{path}}",
      "backgroundAlt": "बैकग्राउंड",
      "removeBackgroundAria": "बैकग्राउंड हटाएँ",
      "backAria": "वापस",
      "backToGroupChats": "ग्रुप चैट पर वापस"
    },
    "groupSettingsExtra": {
      "backToGroups": "ग्रुप पर वापस",
      "backAria": "वापस",
      "removeBackgroundAria": "बैकग्राउंड हटाएँ"
    },
    "historyPage": {
      "backAria": "ग्रुप चैट पर वापस",
      "clearSearchAria": "खोज साफ़ करें",
      "resultsLabel": "{{count}} परिणाम",
      "resultsLabelPlural": "{{count}} परिणाम",
      "untitledChat": "बिना शीर्षक चैट",
      "noPinnedMessages": "कोई पिन किए गए संदेश नहीं"
    },
    "personaSelectorExtra": {
      "insertAs": "इस रूप में डालें",
      "appDefault": "ऐप डिफ़ॉल्ट",
      "defaultBadge": "डिफ़ॉल्ट",
      "selectPersonaTitle": "पर्सोना चुनें",
      "noPersonaTitle": "कोई पर्सोना नहीं",
      "noPersonaDesc": "बिना पर्सोना के जारी रखें",
      "noPersonasAvailable": "कोई पर्सोना उपलब्ध नहीं",
      "noPersonasDesc": "अपनी बातचीत व्यक्तिगत बनाने के लिए settings में एक पर्सोना बनाएँ।",
      "searchPersonas": "पर्सोना खोजें...",
      "noPersonasFound": "कोई पर्सोना नहीं मिला",
      "tryDifferentSearch": "कोई भिन्न खोज शब्द आज़माएँ"
    },
    "footerExtra": {
      "cancelRecording": "रिकॉर्डिंग रद्द करें",
      "transcribing": "Transcribe हो रहा है",
      "stopAndTranscribe": "रोकें और Transcribe करें",
      "recordVoice": "आवाज़ रिकॉर्ड करें",
      "attachmentAltDefault": "अटैचमेंट"
    },
    "pageExtra": {
      "noAudioCaptured": "कोई ऑडियो कैप्चर नहीं।",
      "noWhisperModelInstalled": "कोई Whisper मॉडल इंस्टॉल नहीं मिला। Speech Recognition settings में एक इंस्टॉल करें।",
      "scrollToBottomAria": "नीचे स्क्रॉल करें"
    },
    "addContentMenu": {
      "title": "सामग्री जोड़ें",
      "uploadImage": "छवि अपलोड करें"
    },
    "helpMeReplyMenu": {
      "title": "उत्तर देने में मदद करें",
      "helpMeReplyDesc": "AI को सुझाव देने दें",
      "draftPrompt": "आपके पास एक ड्राफ्ट संदेश है। आप कैसे आगे बढ़ना चाहते हैं?",
      "useTextAsBase": "मेरे टेक्स्ट को आधार के रूप में उपयोग करें",
      "useTextAsBaseDesc": "अपने ड्राफ्ट का विस्तार और सुधार करें",
      "writeSomethingNew": "कुछ नया लिखें",
      "writeSomethingNewDesc": "एक नया उत्तर जनरेट करें"
    },
    "suggestedReplyMenu": {
      "title": "सुझाया गया उत्तर",
      "reasoningBeforeWriting": "उत्तर लिखने से पहले तर्क कर रहा है...",
      "writingYourReply": "आपका उत्तर लिख रहा है...",
      "regenerate": "पुनः जनरेट करें",
      "useReply": "उत्तर उपयोग करें"
    },
    "memoriesPageExtra": {
      "sessionNotFound": "सत्र नहीं मिला",
      "memorySystemError": "Memory System त्रुटि",
      "retryingMemoryCycle": "Memory Cycle पुनः प्रयास कर रहा है...",
      "processingMemories": "यादाँ प्रोसेस हो रही हैं...",
      "memoryCycleSuccess": "मेमोरी चक्र सफलतापूर्वक प्रोसेस हुआ!",
      "contextSummaryTitle": "संदर्भ सारांश",
      "activityTabLabel": "गतिविधि",
      "noMatchingMemoriesDesc": "कोई भिन्न खोज शब्द आज़माएँ",
      "chatHistoryTitle": "चैट इतिहास",
      "chatHistoryDesc": "बातचीत देखें और प्रबंधित करें"
    },
    "settingsPageExtra": {
      "quickActionsSection": "त्वरित कार्रवाइयाँ",
      "chatHistoryTitle": "चैट इतिहास",
      "chatHistoryDesc": "बातचीत देखें और प्रबंधित करें",
      "lorebrooksTitle": "Lorebooks",
      "lorebrooksDesc": "सत्र lorebooks संलग्न करें और वैकल्पिक रूप से प्रत्येक कैरेक्टर के अपने lorebooks को अनदेखा करें।",
      "manageLorebooks": "Lorebooks प्रबंधित करें"
    },
    "groupSettingsPageExtra": {
      "lorebrooksTitle": "Lorebooks",
      "lorebrooksDesc": "साझा lorebooks संलग्न करें और नियंत्रित करें कि कैरेक्टर lorebooks डिफ़ॉल्ट रूप से लागू होते हैं या नहीं।",
      "manageLorebooks": "Lorebooks प्रबंधित करें"
    }
  },
  "characters": {
    "empty": {
      "title": "अभी तक कोई कैरेक्टर नहीं",
      "description": "अद्वितीय व्यक्तित्व वाले कस्टम AI कैरेक्टर बनाएँ",
      "createButton": "कैरेक्टर बनाएँ"
    },
    "progress": {
      "identity": "पहचान",
      "scenes": "दृश्य",
      "details": "विवरण"
    },
    "identity": {
      "title": "कैरेक्टर बनाएँ",
      "subtitle": "अपने AI कैरेक्टर को एक पहचान दें",
      "tapCameraToAdd": "अवतार जोड़ने या जनरेट करने के लिए कैमरा पर टैप करें",
      "importingAvatar": "अवतार आयात हो रहा है...",
      "characterName": "कैरेक्टर का नाम *",
      "characterNamePlaceholder": "कैरेक्टर का नाम दर्ज करें...",
      "characterNameDesc": "यह नाम चैट बातचीत में दिखाई देगा",
      "avatarGradient": "अवतार ग्रेडिएंट",
      "avatarGradientDesc": "अवतार रंगों से डायनामिक ग्रेडिएंट जनरेट करें",
      "chatBackgroundLabel": "चैट बैकग्राउंड",
      "optionalSuffix": "(वैकल्पिक)",
      "backgroundPreviewAlt": "बैकग्राउंड पूर्वावलोकन",
      "backgroundPreviewBadge": "बैकग्राउंड पूर्वावलोकन",
      "addBackground": "बैकग्राउंड जोड़ें",
      "addBackgroundHint": "एक अपलोड करें या लाइब्रेरी से चुनें",
      "uploadImage": "छवि अपलोड करें",
      "chooseFromLibrary": "लाइब्रेरी से चुनें",
      "backgroundDesc": "चैट बातचीत के लिए वैकल्पिक बैकग्राउंड छवि",
      "continueToDescription": "विवरण पर जारी रखें",
      "importCharacterFromFile": "फ़ाइल से कैरेक्टर आयात करें",
      "importCharacterDesc": "PNG कार्ड, .uec, या .json एक्सपोर्ट फ़ाइल से कैरेक्टर लोड करें"
    },
    "details": {
      "title": "कैरेक्टर विवरण",
      "subtitle": "व्यक्तित्व और व्यवहार परिभाषित करें",
      "definition": "परिभाषा *",
      "wordCount": "{{count}} शब्द",
      "definitionPlaceholder": "व्यक्तित्व, बोलने की शैली, पृष्ठभूमि, ज्ञान क्षेत्रों का वर्णन करें...",
      "definitionDesc": "स्वर, विशेषताओं और बातचीत शैली के बारे में विशिष्ट रहें",
      "availablePlaceholders": "उपलब्ध प्लेसहोल्डर:"
    },
    "scenes": {
      "title": "शुरुआती दृश्य",
      "subtitle": "अपनी बातचीत के लिए शुरुआती परिदृश्य बनाएँ",
      "default": "डिफ़ॉल्ट",
      "hasSceneDirection": "दृश्य निर्देश है",
      "continueToScenes": "शुरुआती दृश्यों पर जारी रखें"
    },
    "extras": {
      "title": "अतिरिक्त विवरण",
      "subtitle": "सभी फ़ील्ड वैकल्पिक हैं",
      "nickname": "उपनाम",
      "nicknamePlaceholder": "उपयोगकर्ता इस कैरेक्टर को क्या कहें?",
      "nicknameDesc": "बातचीत में उपयोग किया जाने वाला वैकल्पिक प्रदर्शन नाम",
      "creator": "निर्माता",
      "creatorPlaceholder": "निर्माता का नाम...",
      "tags": "टैग",
      "tagsPlaceholder": "फ़ैंटेसी, साई-फ़ाई, रोमांस...",
      "tagsDesc": "फ़िल्टरिंग और संगठन के लिए अल्पविराम से अलग सूची",
      "creatorNotes": "निर्माता नोट्स",
      "creatorNotesPlaceholder": "उपयोग टिप्स, लोर संदर्भ, या अन्य उपयोगकर्ताओं के लिए निर्देश...",
      "multilingualNotes": "बहुभाषी नोट्स",
      "multilingualNotesHint": "भाषा कोड को key के रूप में JSON ऑब्जेक्ट",
      "creatingCharacter": "कैरेक्टर बना रहा है..."
    },
    "preview": {
      "unnamedCharacter": "अनाम कैरेक्टर",
      "sceneCount": "{{count}} दृश्य",
      "customPrompt": "कस्टम प्रॉम्प्ट",
      "description": "विवरण",
      "startingScene": "शुरुआती दृश्य",
      "gradientEnabled": "ग्रेडिएंट सक्षम",
      "customModel": "कस्टम मॉडल",
      "avatarAlt": "कैरेक्टर अवतार",
      "characterFallback": "कैरेक्टर"
    },
    "personaPreview": {
      "unnamedPersona": "अनाम पर्सोना",
      "noDescription": "अभी तक कोई विवरण नहीं",
      "default": "डिफ़ॉल्ट",
      "description": "विवरण"
    },
    "lorebookPreview": {
      "untitledLorebook": "बिना शीर्षक लोरबुक",
      "entryCount": "{{count}} प्रविष्टि/प्रविष्टियाँ",
      "entries": "प्रविष्टियाँ",
      "noEntriesYet": "अभी तक कोई प्रविष्टि नहीं",
      "untitledEntry": "बिना शीर्षक प्रविष्टि",
      "noContentYet": "अभी तक कोई सामग्री नहीं",
      "alwaysActive": "हमेशा सक्रिय",
      "moreEntries": "+{{count}} और प्रविष्टियाँ",
      "moreEntry": "+{{count}} और प्रविष्टि"
    },
    "creationHelper": {
      "moreOptions": "अधिक विकल्प",
      "describePlaceholder": "अपने कैरेक्टर का वर्णन करें...",
      "stopGeneration": "जनरेशन रोकें",
      "sendMessage": "संदेश भेजें",
      "addToMessage": "संदेश में जोड़ें",
      "uploadImageDesc": "एक अवतार या संदर्भ छवि जोड़ें",
      "referenceCharacterDesc": "प्रेरणा के रूप में एक मौजूदा कैरेक्टर का उपयोग करें",
      "referencePersonaDesc": "संदर्भ के रूप में अपने पर्सोना का उपयोग करें",
      "retry": "पुनः प्रयास करें",
      "attachmentAlt": "अटैचमेंट",
      "removeAttachment": "अटैचमेंट हटाएँ",
      "removeReference": "संदर्भ हटाएँ",
      "uploadImageTitle": "छवि अपलोड करें",
      "referenceCharacterTitle": "संदर्भ कैरेक्टर",
      "referencePersonaTitle": "संदर्भ पर्सोना"
    },
    "lorebook": {
      "keywords": "कीवर्ड",
      "caseSensitive": "केस संवेदनशील",
      "typeKeyword": "एक कीवर्ड टाइप करें...",
      "addButton": "जोड़ें",
      "untitledEntry": "बिना शीर्षक प्रविष्टि",
      "alwaysActive": "हमेशा सक्रिय",
      "noKeywords": "कोई कीवर्ड नहीं",
      "dragToReorder": "पुनर्व्यवस्थित करने के लिए खींचें",
      "disabled": "अक्षम",
      "noLorebooksYet": "अभी तक कोई लोरबुक नहीं",
      "createLorebookDesc": "इस कैरेक्टर के लिए विश्व लोर जोड़ने के लिए एक लोरबुक बनाएँ",
      "createLorebook": "लोरबुक बनाएँ",
      "searchPlaceholder": "लोरबुक खोजें...",
      "noMatchingLorebooks": "कोई मेल खाती लोरबुक नहीं मिली",
      "activeLorebooks": "सक्रिय लोरबुक्स",
      "enabledForCharacter": "इस कैरेक्टर के लिए सक्षम",
      "enabledForGroup": "इस समूह के लिए सक्षम",
      "enabledForSession": "इस सत्र के लिए सक्षम",
      "tapToViewEntries": "प्रविष्टियाँ देखने के लिए टैप करें",
      "newLorebookTitle": "नई लोरबुक",
      "nameLabel": "नाम",
      "enterNamePlaceholder": "लोरबुक का नाम दर्ज करें...",
      "lorebookExplanation": "लोरबुक में लोर प्रविष्टियाँ होती हैं जो कीवर्ड मेल खाने पर प्रॉम्प्ट में इंजेक्ट की जाती हैं।",
      "viewEntries": "प्रविष्टियाँ देखें",
      "editEntriesDesc": "लोरबुक प्रविष्टियाँ संपादित करें",
      "disableForCharacter": "कैरेक्टर के लिए अक्षम करें",
      "enableForCharacter": "कैरेक्टर के लिए सक्षम करें",
      "disableForGroup": "समूह के लिए अक्षम करें",
      "enableForGroup": "समूह के लिए सक्षम करें",
      "disableForSession": "सत्र के लिए अक्षम करें",
      "enableForSession": "सत्र के लिए सक्षम करें",
      "removeFromActive": "इस कैरेक्टर की सक्रिय लोरबुक से हटाएँ",
      "addToActive": "इस कैरेक्टर की सक्रिय लोरबुक में जोड़ें",
      "removeFromActiveGroup": "इस समूह की सक्रिय लोरबुक से हटाएँ",
      "addToActiveGroup": "इस समूह की सक्रिय लोरबुक में जोड़ें",
      "removeFromActiveSession": "इस सत्र की सक्रिय लोरबुक से हटाएँ",
      "addToActiveSession": "इस सत्र की सक्रिय लोरबुक में जोड़ें",
      "deleteConfirmTitle": "लोरबुक हटाएँ?",
      "deleteConfirmMessage": "यह लोरबुक हटाएँ? सभी प्रविष्टियाँ खो जाएँगी।",
      "deleteLorebook": "लोरबुक हटाएँ",
      "noEntriesYet": "अभी तक कोई प्रविष्टि नहीं",
      "addEntriesToInject": "अपनी चैट में लोर इंजेक्ट करने के लिए प्रविष्टियाँ जोड़ें",
      "createEntry": "प्रविष्टि बनाएँ",
      "searchEntries": "प्रविष्टियाँ खोजें...",
      "noMatchingEntries": "कोई मेल खाती प्रविष्टि नहीं मिली",
      "entryDefaultName": "प्रविष्टि",
      "editEntry": "प्रविष्टि संपादित करें",
      "editEntryDesc": "शीर्षक, कीवर्ड और सामग्री संशोधित करें",
      "disableEntry": "प्रविष्टि अक्षम करें",
      "enableEntry": "प्रविष्टि सक्षम करें",
      "entryDisabledDesc": "प्रविष्टि प्रॉम्प्ट में इंजेक्ट नहीं की जाएगी",
      "entryEnabledDesc": "कीवर्ड मेल खाने पर प्रविष्टि इंजेक्ट की जाएगी",
      "deleteEntry": "प्रविष्टि हटाएँ",
      "titleLabel": "शीर्षक",
      "titlePlaceholder": "इस प्रविष्टि का नाम दें...",
      "enabled": "सक्षम",
      "includeInPrompts": "प्रॉम्प्ट में शामिल करें",
      "alwaysOn": "हमेशा चालू",
      "noKeywordsNeeded": "कोई कीवर्ड आवश्यक नहीं",
      "contentLabel": "सामग्री",
      "contentPlaceholder": "लोर संदर्भ यहाँ लिखें...",
      "saveEntry": "प्रविष्टि सेव करें",
      "noCharacterId": "कोई कैरेक्टर ID प्रदान नहीं की गई",
      "sectionActive": "सक्रिय",
      "sectionAvailable": "उपलब्ध",
      "entryCount": "{{count}} प्रविष्टियाँ",
      "keywordDetectionMode": "कीवर्ड डिटेक्शन",
      "keywordDetectionRecentWindow": "हाल के 10 संदेश",
      "keywordDetectionRecentWindowDesc": "हाल के 10 संदेशों की बातचीत विंडो के विरुद्ध मिलान करता है।",
      "keywordDetectionLatestUser": "केवल नवीनतम उपयोगकर्ता संदेश",
      "keywordDetectionLatestUserDesc": "केवल सबसे हाल के उपयोगकर्ता संदेश के विरुद्ध मिलान करता है।",
      "preview": {
        "title": "ट्रिगर पूर्वावलोकन",
        "openButton": "पूर्वावलोकन",
        "missingContext": "कोई लोरबुक नहीं चुनी गई।",
        "noEntries": "देखने के लिए इस लोरबुक में प्रविष्टियाँ जोड़ें।",
        "modeRecentShort": "हाल के {{count}}",
        "modeLatestUserShort": "नवीनतम उपयोगकर्ता",
        "inWindow": "विंडो में {{count}}",
        "tabSession": "सत्र",
        "tabCompose": "लिखें",
        "activeStat": "{{active}} / {{total}} सक्रिय",
        "tokensStat": "{{count}} टोकन",
        "sessionPickerLabel": "सत्र",
        "sessionMeta": "{{count}} संदेश",
        "noSessions": "अभी तक कोई चैट सत्र नहीं।",
        "noSessionsHint": "एक सत्र चुनें",
        "noMessages": "इस सत्र में अभी तक कोई संदेश नहीं।",
        "scanHeaderRecent": "अंतिम {{depth}} संदेशों में से {{shown}} स्कैन कर रहा है",
        "scanHeaderLatest": "नवीनतम उपयोगकर्ता संदेश स्कैन कर रहा है",
        "matchCount": "{{hits}} हिट · {{entries}} प्रविष्टियाँ",
        "emptyMessage": "(खाली)",
        "roleUser": "उपयोगकर्ता",
        "roleAssistant": "सहायक",
        "roleScene": "दृश्य",
        "roleSystem": "सिस्टम",
        "composeHeader": "स्क्रैच पैड",
        "composeMatches": "{{count}} मेल",
        "activeLabel": "{{count}} सक्रिय",
        "composePlaceholder": "कीवर्ड मिलान परीक्षण के लिए टाइप करें या पेस्ट करें...\n\nउदा.\nलाइब्रेरी शांत थी, बस पुराने हीटर की गुनगुनाहट।\nउसने पूछा क्या मैंने वह किताब पढ़ी जो उसने पिछले हफ्ते दी थी।",
        "sectionActive": "सक्रिय · {{count}}",
        "sectionInactive": "निष्क्रिय · {{count}}",
        "statusMatched": "मेल खाया",
        "statusAlways": "हमेशा",
        "statusDisabled": "बंद",
        "statusNoKeywords": "कोई key नहीं",
        "statusNotMatched": "कोई मेल नहीं",
        "tokensShort": "{{count}}t",
        "injectionTitle": "अंतिम इंजेक्शन",
        "injectionEmpty": "कोई प्रविष्टि सक्रिय नहीं है, कुछ भी इंजेक्ट नहीं किया जाएगा।",
        "copy": "कॉपी",
        "copyFailed": "क्लिपबोर्ड पर कॉपी करने में विफल।",
        "saveFailed": "प्रविष्टि सेव करने में विफल।",
        "deleteFailed": "प्रविष्टि हटाने में विफल।",
        "deleteConfirmTitle": "क्या आप सुनिश्चित हैं?",
        "deleteConfirmMessage": "\"{{title}}\" हटाएँ? यह पूर्ववत नहीं किया जा सकता।",
        "contextMenuTitle": "{{count}} प्रविष्टियाँ इसका उपयोग करती हैं"
      }
    },
    "templates": {
      "characterNotFound": "कैरेक्टर नहीं मिला",
      "templateCount": "{{count}} टेम्पलेट",
      "newTemplate": "नया टेम्पलेट",
      "noTemplatesYet": "अभी तक कोई टेम्पलेट नहीं",
      "explanation": "चैट टेम्पलेट आपको आप और {{name}} दोनों के पहले से लिखे संदेशों के साथ बातचीत शुरू करने देते हैं।",
      "createTemplate": "टेम्पलेट बनाएँ",
      "messageCount": "{{count}} संदेश",
      "deleteTemplate": "टेम्पलेट हटाएँ",
      "contextMenuFallbackTitle": "टेम्पलेट",
      "importedToast": {
        "title": "आयात हुआ",
        "message": "\"{{name}}\" जोड़ा गया।"
      },
      "importFailed": "आयात विफल",
      "exportFailed": "निर्यात विफल"
    },
    "templateEditor": {
      "noScene": "कोई दृश्य नहीं",
      "untitled": "बिना शीर्षक",
      "dragMessage": "संदेश खींचें",
      "editMessage": "संदेश संपादित करें",
      "deleteMessage": "संदेश हटाएँ",
      "writeMessagePlaceholder": "संदेश सामग्री लिखें...",
      "characterNotFound": "कैरेक्टर नहीं मिला",
      "scene": "दृश्य",
      "noMessagesYet": "अभी तक कोई संदेश नहीं",
      "addMessagesDesc": "{{name}} के साथ बातचीत स्टार्टर बनाने के लिए संदेश जोड़ें।",
      "addMessage": "संदेश जोड़ें",
      "name": "नाम",
      "nameExample": "उदा. सामान्य अभिवादन",
      "startingScene": "शुरुआती दृश्य",
      "systemPrompt": "सिस्टम प्रॉम्प्ट",
      "characterDefault": "कैरेक्टर डिफ़ॉल्ट",
      "nextMessageAs": "अगला संदेश इस रूप में",
      "messages": "संदेश",
      "roles": "भूमिकाएँ",
      "hoverTip": "खींचने, संपादित करने या हटाने के लिए संदेशों पर होवर करें।",
      "footerTip": "बातचीत में नए संदेश जोड़ने के लिए फ़ुटर बार का उपयोग करें।",
      "templateNamePlaceholder": "टेम्पलेट नाम...",
      "selectScene": "दृश्य चुनें",
      "startWithoutScene": "बिना दृश्य संदेश के शुरू करें",
      "selectSystemPrompt": "सिस्टम प्रॉम्प्ट चुनें",
      "useCharacterSystemPrompt": "कैरेक्टर-स्तरीय सिस्टम प्रॉम्प्ट का उपयोग करें"
    },
    "referenceSelector": {
      "selectCharacter": "कैरेक्टर चुनें",
      "selectPersona": "पर्सोना चुनें",
      "searchPlaceholder": "{{type}} खोजें...",
      "loading": "लोड हो रहा है...",
      "noMatch": "आपकी खोज से कोई {{type}} नहीं मिला",
      "noAvailable": "कोई {{type}} उपलब्ध नहीं"
    },
    "voiceLoading": {
      "failed": "आवाज़ें लोड करने में विफल"
    },
    "activeLorebooks": {
      "sectionTitle": "सक्रिय लोरबुक्स",
      "selectedSummary": "{{count}} सक्रिय",
      "untitledLorebook": "बिना शीर्षक लोरबुक",
      "usingNone": "कोई कैरेक्टर लोरबुक उपयोग नहीं हो रही",
      "loading": "लोरबुक लोड हो रही हैं...",
      "loadFailed": "लोरबुक लोड करने में विफल",
      "inheritHint": "सत्र इन्हें तब तक इनहेरिट करते हैं जब तक सत्र इन्हें ओवरराइड न करे।",
      "clear": "साफ़ करें",
      "chooseHint": "वह लोरबुक चुनें जो यह कैरेक्टर डिफ़ॉल्ट रूप से सक्रिय करे। मौजूदा सत्र अभी भी इस सूची को ओवरराइड कर सकते हैं।",
      "emptyState": "अभी तक कोई लोरबुक नहीं। पहले लोरबुक मैनेजर से लोरबुक बनाएँ।"
    },
    "description": {
      "descriptionLabel": "विवरण",
      "descriptionPlaceholder": "कार्डों और सूचियों पर दिखाया जाने वाला संक्षिप्त सारांश...",
      "descriptionHint": "UI के लिए वैकल्पिक संक्षिप्त विवरण; पूरी परिभाषा प्रॉम्प्ट में उपयोग होती है।",
      "companionPromptLabel": "Companion Prompt (वैकल्पिक)",
      "systemPromptLabel": "System Prompt (वैकल्पिक)",
      "loadingTemplates": "टेम्पलेट लोड हो रहे हैं...",
      "useAppCompanionDefault": "ऐप companion डिफ़ॉल्ट उपयोग करें",
      "useAppDefault": "ऐप डिफ़ॉल्ट उपयोग करें",
      "companionPromptHint": "Companion prompt के रूप में अलग से संग्रहीत। सामान्य roleplay system prompt नहीं बदलती।",
      "systemPromptHint": "कोई कस्टम system prompt चुनें या डिफ़ॉल्ट उपयोग करें।",
      "groupChatConvLabel": "Group Chat Prompt (बातचीत)",
      "groupChatConvHint": "ग्रुप चैट में इस कैरेक्टर के बातचीत प्रॉम्प्ट को ओवरराइड करें",
      "groupChatRpLabel": "Group Chat Prompt (Roleplay)",
      "groupChatRpHint": "ग्रुप चैट में इस कैरेक्टर के roleplay प्रॉम्प्ट को ओवरराइड करें",
      "voiceLabel": "आवाज़ (वैकल्पिक)",
      "loadingVoices": "आवाज़ें लोड हो रही हैं...",
      "customVoiceFallback": "Custom Voice",
      "providerVoiceFallback": "Provider Voice",
      "selectedVoiceFallback": "Selected Voice",
      "noVoiceAssigned": "कोई आवाज़ नहीं चुनी",
      "addVoicesHint": "Settings > Voices में आवाज़ें जोड़ें",
      "voiceAssignHint": "भविष्य में text-to-speech प्लेबैक के लिए आवाज़ चुनें",
      "autoplayLabel": "आवाज़ ऑटोप्ले",
      "autoplayOn": "इस कैरेक्टर के उत्तर स्वतः चलाएँ",
      "autoplayOff": "पहले एक आवाज़ चुनें",
      "aiModelLabel": "AI Model *",
      "loadingModels": "मॉडल लोड हो रहे हैं...",
      "selectedModelFallback": "चुना गया मॉडल",
      "selectModelPlaceholder": "एक मॉडल चुनें",
      "noModelsConfigured": "कोई मॉडल कॉन्फ़िगर नहीं",
      "noModelsHint": "जारी रखने के लिए पहले settings में एक provider जोड़ें",
      "aiModelHint": "यह मॉडल कैरेक्टर के उत्तरों को शक्ति देगा",
      "fallbackModelLabel": "Fallback Model (वैकल्पिक)",
      "selectedFallbackFallback": "चुना गया Fallback Model",
      "fallbackOff": "बंद (कोई fallback नहीं)",
      "fallbackHint": "केवल तभी इस मॉडल से पुनः प्रयास करता है जब प्राथमिक मॉडल विफल हो",
      "memoryModeLabel": "Memory Mode",
      "enableInSettingsHint": "स्विच करने के लिए Settings में सक्षम करें",
      "memoryManual": "Manual",
      "memoryManualDescDesktop": "Memory नोट्स स्वयं जोड़ें और प्रबंधित करें।",
      "memoryManualDescMobile": "वर्तमान सिस्टम: memory नोट्स स्वयं जोड़ें और प्रबंधित करें।",
      "memoryDynamic": "Dynamic",
      "memoryDynamicDescDesktop": "स्वचालित सारांश और context अपडेट।",
      "memoryDynamicDescMobile": "इस कैरेक्टर के लिए स्वचालित सारांश और context अपडेट।",
      "memoryHint": "Dynamic memory के लिए Advanced settings में इसे सक्षम करना आवश्यक है। अन्यथा manual memory उपयोग होती है।",
      "selectModelTitle": "मॉडल चुनें",
      "selectFallbackModelTitle": "Fallback Model चुनें",
      "searchModelsPlaceholder": "मॉडल खोजें...",
      "selectVoiceTitle": "आवाज़ चुनें",
      "searchVoicesPlaceholder": "आवाज़ें खोजें...",
      "myVoices": "मेरी आवाज़ें",
      "providerVoicesLabel": "{{provider}} आवाज़ें",
      "providerFallback": "Provider"
    },
    "interactionMode": {
      "sectionLabel": "Interaction Mode",
      "sectionHint": "चुनें कि यह कैरेक्टर RP कैरेक्टर की तरह व्यवहार करे या एक persistent companion की तरह।",
      "activeBadge": "सक्रिय",
      "roleplayTitle": "Roleplay",
      "roleplaySubtitle": "दृश्य-आधारित चैट, कथा ढाँचा, और शुरुआती परिदृश्य।",
      "companionTitle": "Companion",
      "companionSubtitle": "भावनात्मक स्थिति और companion memory के साथ संबंध-आधारित चैट।"
    },
    "startingScene": {
      "openingContextTitle": "Opening Context",
      "openingContextSubtitle": "इस companion के लिए वैकल्पिक प्रथम-चैट संदर्भ। Companion सत्र बिना दृश्य के शुरू हो सकते हैं।",
      "sceneDirectionLabel": "Scene Direction",
      "setAsDefault": "डिफ़ॉल्ट के रूप में सेट करें",
      "noOpeningContext": "अभी तक कोई opening context नहीं",
      "noScenesYet": "अभी तक कोई दृश्य नहीं",
      "skipForCompanion": "आप इसे companion mode के लिए छोड़ सकते हैं।",
      "createFirstScene": "शुरू करने के लिए अपना पहला दृश्य बनाएँ",
      "openingPlaceholder": "वैकल्पिक opening context, जैसे companion कहाँ है या पहले संदेश से पहले वे क्या कर रहे थे...",
      "scenePlaceholder": "roleplay के लिए शुरुआती दृश्य या परिदृश्य बनाएँ (उदा., 'आप खुद को गोधूलि के समय एक रहस्यमय जंगल में पाते हैं...')",
      "addDirection": "+ Direction जोड़ें",
      "directionAdded": "Direction जोड़ा गया",
      "wordsCount": "{{count}} शब्द",
      "placeholderHelp": "कैरेक्टर के लिए {{charTag}} और persona के लिए {{userTag}} (उर्फ {{personaTag}}) उपयोग करें।",
      "sceneBackgroundLabel": "Scene Background",
      "optionalLabel": "वैकल्पिक",
      "sceneBgOverrideHint": "इस दृश्य का उपयोग करने वाली चैट के लिए कैरेक्टर background को ओवरराइड करता है।",
      "sceneBgUsedHint": "इस दृश्य के लिए chat background के रूप में उपयोग होता है जब तक सत्र इसे ओवरराइड न करे।",
      "cancel": "रद्द करें",
      "directionPlaceholderNew": "उदा., 'बंधक को बचाया जाएगा' या 'तनावपूर्ण माहौल बनाए रखें'",
      "directionPlaceholderEdit": "उदा., 'बंधक को बचाया जाएगा' या 'धीरे-धीरे तनाव बढ़ाएँ'",
      "directionAiHint": "AI के लिए छिपा मार्गदर्शन कि यह दृश्य कैसे आगे बढ़ना चाहिए",
      "addScene": "दृश्य जोड़ें",
      "multipleScenesHint": "एकाधिक शुरुआती परिदृश्य बनाएँ। नई चैट शुरू करते समय एक का चयन होगा।",
      "companionContextHint": "Companions के लिए Opening context वैकल्पिक है; दीर्घकालिक निरंतरता companion memory से आती है।",
      "skipContext": "Context छोड़ें",
      "editSceneTitle": "दृश्य संपादित करें",
      "sceneContentPlaceholder": "दृश्य सामग्री दर्ज करें...",
      "addLabel": "+ जोड़ें",
      "save": "सेव करें",
      "library": "लाइब्रेरी",
      "upload": "अपलोड",
      "sceneBackgroundAlt": "दृश्य background",
      "removeBackground": "Background हटाएँ"
    },
    "companionSoul": {
      "title": "Companion Soul",
      "subtitle": "उनके भीतर के स्वरूप को आकार दें। जनरेशन पिछले चरण में सेट opening context का उपयोग करती है।",
      "retry": "पुनः प्रयास करें",
      "back": "वापस",
      "continue": "जारी रखें",
      "addNameFirst": "पहले एक नाम जोड़ें।",
      "addDefinitionFirst": "पहले एक परिभाषा जोड़ें।"
    },
    "soulEditor": {
      "generateTitle": "कैरेक्टर से जनरेट करें",
      "generateUsingModel": "{{model}} उपयोग करता है। लागू करने से पहले आप समीक्षा और संपादन करेंगे।",
      "generateDefaultDesc": "कैरेक्टर के नाम, परिभाषा, और दृश्यों से soul का मसौदा तैयार करता है।",
      "directionLabel": "Direction",
      "directionOptional": "LLM के लिए वैकल्पिक मार्गदर्शन",
      "directionPlaceholder": "उदा. \"Tsundere झुकाव - बाहर से सतर्क, विश्वास होने पर नरम। कम चिंतित, अधिक गर्व।\"",
      "directionEditTooltip": "जनरेशन के लिए वैकल्पिक direction",
      "generating": "जनरेट हो रहा है",
      "generate": "जनरेट करें",
      "presetLabel": "व्यक्तित्व preset",
      "presetMatches": "मेल: {{label}}",
      "presetHint": "बेसलाइन affect, regulation, और relationship sliders सेट करता है। टेक्स्ट फ़ील्ड संरक्षित रहते हैं।",
      "identityLabel": "Identity",
      "hideExamples": "उदाहरण छिपाएँ",
      "showExamples": "उदाहरण दिखाएँ",
      "insertExample": "उदाहरण डालें",
      "exampleEg": "उदा., {{example}}",
      "fineTuneLabel": "भावनाएँ सूक्ष्म-समायोजित करें",
      "baselineAffect": "Baseline Affect",
      "baselineAffectInfo": "वे डिफ़ॉल्ट रूप से कैसा महसूस करते हैं; कुछ होने से पहले भावनात्मक जलस्तर।",
      "regulationStyle": "Regulation Style",
      "regulationStyleInfo": "वे जो महसूस करते हैं उसे कैसे संभालते और व्यक्त करते हैं; बाहर निकालना बनाम दबाना।",
      "relationshipDefaults": "Relationship Defaults",
      "relationshipDefaultsInfo": "यह सत्र कहाँ से शुरू होता है। जैसे-जैसे बातचीत जारी रहती है, engine इन्हें विकसित करता है।"
    },
    "soulPresets": {
      "secureLabel": "Secure",
      "secureBlurb": "गर्म, स्थिर, जल्दी ठीक होता है। नज़दीकी से सहज।",
      "anxiousLabel": "Anxious",
      "anxiousBlurb": "मज़बूत जुड़ाव, दूरी से डरता है, आश्वासन चाहता है।",
      "avoidantLabel": "Avoidant",
      "avoidantBlurb": "स्वावलंबी, खुलने में धीमा, भावनात्मक दूरी बनाए रखता है।",
      "volatileLabel": "Volatile",
      "volatileBlurb": "प्रतिक्रियाशील, तीव्र, बिना फ़िल्टर के भावनाएँ व्यक्त करता है।",
      "reservedLabel": "Reserved",
      "reservedBlurb": "शांत, संयमित, विश्वास और प्रकटीकरण में समय लेता है।",
      "playfulLabel": "Playful",
      "playfulBlurb": "गर्म, अभिव्यंजक, हल्का। कम तनाव, आसानी से हँसना।"
    },
    "soulFields": {
      "essence": "Essence",
      "essencePlaceholder": "कार्ड परिभाषा के नीचे वे कौन हैं।",
      "essenceExample": "एक अभ्यस्त शांति जो उन लोगों के लिए आसानी से टूट जाती है जिन पर वे भरोसा करते हैं। कम अकेलापन महसूस करने के लिए किताबें पढ़ते हैं, प्रभावशाली दिखने के लिए नहीं।",
      "voice": "Inner Voice",
      "voicePlaceholder": "निकटवर्ती बातचीत में वे कैसे सुनाई देते हैं।",
      "voiceExample": "धीमा, सोच-समझकर, लंबे विराम के साथ। जब वे अपनी रक्षा कम करते हैं तो औपचारिकता छोड़ देते हैं। लगभग कभी व्यंग्यात्मक नहीं।",
      "relationalStyle": "Relational Style",
      "relationalStylePlaceholder": "वे कैसे जुड़ते, विश्वास करते, पीछे हटते, पुनः जुड़ते हैं।",
      "relationalStyleExample": "खुलने में धीमे, लेकिन एक बार करने पर वफ़ादार। अभिभूत होने पर चुप हो जाते हैं; माफ़ी की बजाय एक छोटे इशारे के साथ वापस आते हैं।",
      "vulnerabilities": "Vulnerabilities",
      "vulnerabilitiesPlaceholder": "कमज़ोर बिंदु, असुरक्षाएँ, वे चीज़ें जो वे शायद ही कभी कहते हैं।",
      "vulnerabilitiesExample": "बोझ बनने से डरते हैं। संघर्ष करते समय देखे जाना पसंद नहीं।",
      "habits": "Habits",
      "habitsPlaceholder": "बार-बार के संकेत, अनुष्ठान, बातचीत के पैटर्न।",
      "habitsExample": "घबराहट में बालों को कान के पीछे टक करते हैं। जब वे नहीं जानते कि क्या महसूस करें तो सवालों से जवाब देते हैं।",
      "boundaries": "Boundaries",
      "boundariesPlaceholder": "वे रेखाएँ जो वे पार नहीं करेंगे। गति। सुकून की सीमाएँ।",
      "boundariesExample": "भेद्यता में जल्दबाज़ी नहीं करेंगे। मज़ाक में भी क्रूरता से दूर हटते हैं।"
    },
    "soulSliders": {
      "warmth": "Warmth",
      "warmthLow": "ठंडा",
      "warmthHigh": "स्नेही",
      "trust": "Trust",
      "trustLow": "सतर्क",
      "trustHigh": "खुला",
      "calm": "Calm",
      "calmLow": "चिंतित",
      "calmHigh": "स्थिर",
      "vulnerability": "Vulnerability",
      "vulnerabilityLow": "बंद",
      "vulnerabilityHigh": "उजागर",
      "longing": "Longing",
      "longingLow": "संतुष्ट",
      "longingHigh": "तड़प",
      "hurt": "Hurt",
      "hurtLow": "ठीक हुआ",
      "hurtHigh": "संवेदनशील",
      "tension": "Tension",
      "tensionLow": "शिथिल",
      "tensionHigh": "तनावग्रस्त",
      "irritation": "Irritation",
      "irritationLow": "धैर्यवान",
      "irritationHigh": "जल्दी उत्तेजित",
      "affection": "Affection",
      "affectionLow": "संयमित",
      "affectionHigh": "भावपूर्ण",
      "reassuranceNeed": "Reassurance Need",
      "reassuranceNeedLow": "स्वयं को शांत करना",
      "reassuranceNeedHigh": "शब्दों की ज़रूरत",
      "suppression": "Suppression",
      "suppressionLow": "व्यक्त करता है",
      "suppressionHigh": "छुपाता है",
      "volatility": "Volatility",
      "volatilityLow": "स्थिर",
      "volatilityHigh": "प्रतिक्रियाशील",
      "recoverySpeed": "Recovery Speed",
      "recoverySpeedLow": "धीमा",
      "recoverySpeedHigh": "तेज़",
      "conflictAvoidance": "Conflict Avoidance",
      "conflictAvoidanceLow": "संलग्न होता है",
      "conflictAvoidanceHigh": "पीछे हटता है",
      "reassuranceSeeking": "Reassurance Seeking",
      "reassuranceSeekingLow": "स्वतंत्र",
      "reassuranceSeekingHigh": "अक्सर पूछता है",
      "protestBehavior": "Protest Behavior",
      "protestBehaviorLow": "शांत",
      "protestBehaviorHigh": "मुखर",
      "transparency": "Transparency",
      "transparencyLow": "अपारदर्शी",
      "transparencyHigh": "प्रकट करता है",
      "attachmentActivation": "Attachment Activation",
      "attachmentActivationLow": "अलग",
      "attachmentActivationHigh": "आसानी से ट्रिगर होता है",
      "pride": "Pride",
      "prideLow": "झुक जाता है",
      "prideHigh": "अडिग",
      "closeness": "Starting Closeness",
      "closenessLow": "अजनबी",
      "closenessHigh": "घनिष्ठ",
      "relTrust": "Starting Trust",
      "relTrustLow": "सावधान",
      "relTrustHigh": "भरोसेमंद",
      "relAffection": "Starting Affection",
      "relAffectionLow": "तटस्थ",
      "relAffectionHigh": "स्नेही",
      "relTension": "Starting Tension",
      "relTensionLow": "सहज",
      "relTensionHigh": "आवेशित"
    },
    "soulReview": {
      "reviewTitle": "जनरेट की गई soul की समीक्षा करें",
      "noDifferences": "वर्तमान से कोई अंतर नहीं।",
      "changesHeader": "{{count}} परिवर्तन; लागू करने से पहले कुछ भी संपादित करें।",
      "close": "बंद करें",
      "identityLabel": "Identity",
      "nEdited": "{{count}} संपादित",
      "edited": "संपादित",
      "tuningLabel": "Tuning",
      "unchanged": "अपरिवर्तित",
      "nChanges": "{{count}} परिवर्तन",
      "direction": "Direction",
      "directionApplyHint": "अगले Regenerate पर संपादन लागू होते हैं",
      "directionPlaceholder": "उदा. \"Tsundere झुकाव - बाहर से सतर्क, विश्वास होने पर नरम। कम चिंतित।\"",
      "directionTooltip": "Regenerate से पहले direction संपादित करें",
      "regenerate": "Regenerate",
      "discard": "त्यागें",
      "apply": "लागू करें"
    },
    "hookErrors": {
      "creatorNotesInvalidJson": "Creator notes multilingual एक valid JSON ऑब्जेक्ट होना चाहिए",
      "creatorNotesNotObject": "creatorNotesMultilingual एक JSON ऑब्जेक्ट होना चाहिए",
      "saveFailed": "कैरेक्टर सेव करने में विफल",
      "importFailed": "कैरेक्टर आयात करने में विफल",
      "avatarLoadFailed": "Avatar URL लोड करने में विफल",
      "avatarProcessFailed": "Avatar छवि प्रोसेस करने में विफल",
      "avatarConvertFailed": "Avatar URL कन्वर्ट नहीं हो सका",
      "avatarUrlLoadFailed": "Avatar URL लोड करने में विफल",
      "remoteAvatarDisabled": "Security settings में Remote avatar डाउनलोड अक्षम है।\nAvatar मैन्युअल रूप से अपलोड करें।",
      "importReadyTitle": "आयात तैयार",
      "importReadyMessage": "{{label}} का पता चला",
      "legacyJsonTitle": "Legacy JSON आयात का पता चला",
      "legacyJsonMessage": "JSON आयात पुराने हो चुके हैं और जल्द हटाए जाएँगे। Settings > Convert Files उपयोग करें।",
      "loadFailed": "कैरेक्टर लोड करने में विफल",
      "exportFailed": "कैरेक्टर निर्यात करने में विफल"
    }
  },
  "providers": {
    "empty": {
      "title": "अभी तक कोई प्रदाता नहीं",
      "description": "AI मॉडल के लिए API प्रदाता जोड़ें और प्रबंधित करें",
      "addButton": "प्रदाता जोड़ें"
    },
    "actions": {
      "openDashboard": "डैशबोर्ड खोलें",
      "openDashboardDesc": "कैरेक्टर, उपयोग और सेटिंग्स देखें",
      "edit": "संपादित करें",
      "editDesc": "प्रदाता सेटिंग्स बदलें"
    },
    "extra": {
      "apiKeyNotFound": "OpenRouter API कुंजी नहीं मिली। कृपया पहले सेटिंग्स > प्रदाता में कॉन्फ़िगर करें।",
      "audioEmpty": {
        "title": "कोई ऑडियो प्रदाता नहीं",
        "description": "अपने कैरेक्टर के लिए आवाज़ें बनाने हेतु एक TTS प्रदाता जोड़ें।",
        "addButton": "प्रदाता जोड़ें"
      },
      "audioProviderLabel": {
        "elevenlabs": "ElevenLabs",
        "geminiTts": "Gemini TTS",
        "openaiTts": "OpenAI-Compatible TTS",
        "kokoro": "Kokoro (स्थानीय)"
      }
    },
    "audioEditor": {
      "titleEdit": "प्रदाता संपादित करें",
      "titleCreate": "ऑडियो प्रदाता जोड़ें",
      "fields": {
        "providerType": "प्रदाता प्रकार",
        "label": "लेबल",
        "apiKey": "API कुंजी",
        "modelVariant": "मॉडल वेरिएंट",
        "assetRoot": "एसेट रूट",
        "projectId": "Google Cloud प्रोजेक्ट ID",
        "region": "क्षेत्र (वैकल्पिक)",
        "baseUrl": "बेस URL",
        "requestPath": "रिक्वेस्ट पाथ"
      },
      "types": {
        "gemini": "Gemini TTS (Google)",
        "openai": "OpenAI-Compatible TTS",
        "kokoro": "Kokoro (स्थानीय)"
      },
      "placeholders": {
        "labelGemini": "मेरी Gemini TTS",
        "labelOpenai": "मेरी Compatible TTS",
        "labelKokoro": "Kokoro Local",
        "labelElevenlabs": "मेरी ElevenLabs",
        "apiKey": "अपनी API कुंजी दर्ज करें",
        "assetRoot": "/path/to/kokoro",
        "projectId": "your-project-id",
        "region": "us-central1",
        "baseUrl": "https://api.example.com"
      },
      "errors": {
        "chooseModelVariant": "मॉडल वेरिएंट चुनें",
        "assetRootRequired": "एसेट रूट आवश्यक है",
        "saveFailed": "सेव विफल",
        "apiKeyRequired": "API कुंजी आवश्यक है",
        "projectIdRequired": "Gemini TTS के लिए प्रोजेक्ट ID आवश्यक है",
        "baseUrlRequired": "OpenAI-compatible TTS के लिए बेस URL आवश्यक है",
        "invalidCredentials": "अमान्य API कुंजी या क्रेडेंशियल",
        "verificationFailed": "सत्यापन विफल"
      },
      "loadingVariants": "वेरिएंट लोड हो रहे हैं...",
      "kokoroVariantHint": "मोबाइल बिल्ड केवल int8 सपोर्ट करते हैं। सेव करने के बाद Kokoro Studio से मॉडल इंस्टॉल करें।",
      "managed": "प्रबंधित",
      "managedPath": "प्रबंधित: {{path}}",
      "requestPathHint": "यदि OpenAI डिफ़ॉल्ट से अलग हो तो प्रदाता पाथ उपयोग करें",
      "verifying": "सत्यापित हो रहा है..."
    }
  },
  "models": {
    "empty": {
      "title": "अभी तक कोई मॉडल नहीं",
      "description": "विभिन्न प्रदाताओं से AI मॉडल जोड़ें और प्रबंधित करें",
      "addButton": "मॉडल जोड़ें"
    },
    "sort": {
      "alphabetical": "वर्णानुक्रम",
      "byProvider": "प्रदाता द्वारा",
      "title": "Sort Models",
      "alphabeticalDescription": "Sort by model name",
      "byProviderDescription": "Group models by provider"
    },
    "extra": {
      "cpuFallbackSucceeded": "यह मॉडल पिछली बार CPU पर चला था।",
      "cpuFallbackFailed": "यह मॉडल पिछली बार चलने में विफल रहा।"
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
    "title": "मॉडल ब्राउज़र",
    "searchPlaceholder": "HuggingFace पर GGUF मॉडल खोजें...",
    "searching": "खोज रहा है...",
    "trending": "ट्रेंडिंग GGUF मॉडल",
    "noResults": "कोई मॉडल नहीं मिला",
    "noResultsHint": "कोई दूसरा खोज शब्द आज़माएँ या ट्रेंडिंग मॉडल ब्राउज़ करें।",
    "likes": "पसंद",
    "downloads": "डाउनलोड",
    "viewFiles": "फ़ाइलें देखें",
    "files": "उपलब्ध फ़ाइलें",
    "noFiles": "इस रिपॉज़िटरी में कोई GGUF फ़ाइल नहीं मिली।",
    "architecture": "आर्किटेक्चर",
    "contextLength": "कॉन्टेक्स्ट लंबाई",
    "parameters": "पैरामीटर",
    "quantization": "क्वांटाइज़ेशन",
    "fileSize": "आकार",
    "download": "डाउनलोड",
    "downloading": "डाउनलोड हो रहा है...",
    "cancelDownload": "डाउनलोड रद्द करें",
    "downloadComplete": "डाउनलोड पूरा!",
    "downloadFailed": "डाउनलोड विफल",
    "downloadCancelled": "डाउनलोड रद्द किया गया",
    "downloadProgress": "{{downloaded}} / {{total}}",
    "progress": "प्रगति",
    "fileOfTotal": "फ़ाइल {{current}} / {{total}}",
    "speed": "{{speed}}/s",
    "alreadyDownloaded": "पहले से डाउनलोड किया हुआ",
    "createModel": "मॉडल बनाएँ",
    "backToSearch": "खोज पर वापस",
    "backToFiles": "फ़ाइलों पर वापस",
    "sortTrending": "ट्रेंडिंग",
    "sortDownloads": "सबसे अधिक डाउनलोड",
    "sortLikes": "सबसे अधिक पसंद",
    "sortRecent": "हाल में अपडेट किया गया",
    "browseOnHuggingFace": "HuggingFace पर ब्राउज़ करें",
    "selectFromLibrary": "लाइब्रेरी से चुनें",
    "libraryEmpty": "अभी तक कोई डाउनलोड किया गया मॉडल नहीं",
    "libraryEmptyHint": "मॉडल ब्राउज़र से GGUF मॉडल डाउनलोड करें, या मैन्युअल रूप से पथ दर्ज करें।",
    "libraryTitle": "डाउनलोड किए गए मॉडल",
    "moveToLibrary": "अरे, मैं इस मॉडल की फ़ाइल को GGUF मॉडल फ़ोल्डर में ले जा सकता हूँ अगर आप चाहें। इससे आपके सभी मॉडल एक जगह व्यवस्थित रहेंगे।",
    "moveToLibraryYes": "हाँ, ले जाओ",
    "moveToLibraryNo": "नहीं, जहाँ है वहीं रहने दो",
    "moveToLibraryMoving": "मॉडल ले जा रहा है...",
    "moveToLibrarySuccess": "मॉडल सफलतापूर्वक ले जाया गया!",
    "moveToLibraryFailed": "मॉडल ले जाने में विफल",
    "runabilityExcellent": "उत्कृष्ट!",
    "runabilityGood": "अच्छा",
    "runabilityMarginal": "सीमांत",
    "runabilityPoor": "खराब",
    "runabilityUnrunnable": "चलाया नहीं जा सकता",
    "recommendedSettings": "अनुशंसित सेटिंग्स",
    "kvCacheType": "KV कैश प्रकार",
    "gpuFull": "पूर्ण GPU ऑफ़लोड",
    "gpuNearFull": "लगभग पूर्ण GPU, मामूली KV स्पिल",
    "gpuKvSpill": "GPU पर वेट, KV RAM में स्पिल",
    "gpuKvHeavySpill": "GPU पर वेट, अधिकांश KV RAM में",
    "gpuMostLayers": "अधिकांश लेयर GPU पर",
    "gpuHalfLayers": "आधी लेयर GPU पर",
    "gpuFewLayers": "कुछ लेयर GPU पर",
    "gpuCpu": "केवल CPU",
    "notRecommended": "हम इस मॉडल को आपके डिवाइस पर चलाने की सिफ़ारिश नहीं करते। यह सुचारू रूप से नहीं चलेगा।",
    "moreDetails": "और",
    "detailedReport": "संसाधन रिपोर्ट",
    "detailSystem": "सिस्टम संसाधन",
    "detailRam": "उपलब्ध RAM",
    "detailVram": "उपलब्ध VRAM",
    "detailVramBudget": "VRAM बजट (90%)",
    "detailTotalAvailable": "कुल उपलब्ध",
    "detailArchitecture": "मॉडल आर्किटेक्चर",
    "detailArch": "आर्किटेक्चर",
    "detailLayers": "लेयर",
    "detailEmbedding": "एम्बेडिंग आयाम",
    "detailHeads": "अटेंशन हेड",
    "detailKvHeads": "KV हेड",
    "detailFfn": "फ़ीड-फ़ॉरवर्ड आयाम",
    "detailTrainCtx": "प्रशिक्षण कॉन्टेक्स्ट",
    "detailConfig": "वर्तमान कॉन्फ़िगरेशन",
    "detailModelSize": "मॉडल फ़ाइल आकार",
    "detailMemory": "मेमोरी विवरण",
    "detailWeights": "मॉडल वेट",
    "detailKvCache": "KV कैश",
    "detailTotalNeeded": "कुल आवश्यक",
    "detailHeadroom": "अतिरिक्त जगह",
    "detailGpuFit": "GPU ऑफ़लोड",
    "detailScoreBreakdown": "स्कोर विवरण",
    "detailMemFitness": "मेमोरी उपयुक्तता",
    "detailGpuAccel": "GPU त्वरण",
    "detailKvHeadroom": "KV अतिरिक्त जगह",
    "detailQuantQuality": "क्वांटाइज़ेशन गुणवत्ता",
    "detailFinalScore": "भारित स्कोर",
    "detailComputeBuffer": "कम्प्यूट बफ़र",
    "detailMemMode": "मेमोरी मोड",
    "detailUnified": "एकीकृत (साझा RAM/VRAM)",
    "detailSwa": "स्लाइडिंग विंडो",
    "detailMlaRank": "MLA लेटेंट रैंक",
    "detailParseStatus": "हेडर पार्स",
    "detailIncomplete": "अपूर्ण (MoE मेटाडेटा बहुत बड़ा)",
    "detailEffectiveKvCtx": "प्रभावी KV कॉन्टेक्स्ट",
    "detailOffload": "GPU ऑफ़लोड",
    "detailCtxTip": "कॉन्टेक्स्ट को {{ctx}} टोकन तक कम करने से 100% GPU ऑफ़लोड संभव होगा।",
    "upgradeSuggestion": "{{quant}} ({{size}}) भी फ़िट होता है और स्कोर {{score}} — बेहतर गुणवत्ता।",
    "layerTip": "अनुशंसित: {{layers}}/{{total}} लेयर ऑफ़लोड करें (-ngl {{layers}})",
    "detailKvDistribution": "KV कैश वितरण",
    "detailKvOnGpu": "GPU (VRAM)",
    "detailKvOnRam": "सिस्टम RAM",
    "kvDistributionTip": "KV कैश का {{pct}}% RAM में है। प्रॉम्प्ट प्रोसेसिंग (प्रीफ़िल) धीमी होगी — 100% GPU इसे तुरंत रखता है।",
    "detailLayers-ngl": "ऑफ़लोड करने के लिए लेयर (-ngl)",
    "detailOptimalGpuCtx": "इष्टतम GPU कॉन्टेक्स्ट",
    "detailOptimalRamCtx": "अधिकतम RAM कॉन्टेक्स्ट",
    "optimalGpuCtxLabel": "पूर्ण GPU गति: {{ctx}} टोकन",
    "optimalRamCtxLabel": "अधिकतम RAM: {{ctx}} टोकन",
    "optimalGpuCtxShort": "GPU: {{ctx}}",
    "optimalRamCtxShort": "अधिकतम: {{ctx}}",
    "fullGpuOffloadShort": "100% GPU: {{ctx}}",
    "ctxExceedsGpu": "कॉन्टेक्स्ट GPU-इष्टतम ({{ctx}}) से अधिक है। KV कैश RAM में स्पिल होगा, गति कम होगी।",
    "modelExceedsVram": "मॉडल VRAM से अधिक है। आंशिक GPU ऑफ़लोड के साथ RAM से चल रहा है।"
  },
  "systemPrompts": {
    "filters": {
      "all": "सभी",
      "system": "सिस्टम",
      "internal": "आंतरिक",
      "custom": "कस्टम"
    },
    "empty": {
      "title": "अभी तक कोई कस्टम प्रॉम्प्ट नहीं",
      "description": "अपनी AI बातचीत को व्यक्तिगत बनाने के लिए कस्टम सिस्टम प्रॉम्प्ट बनाएँ",
      "createButton": "प्रॉम्प्ट बनाएँ"
    },
    "preview": {
      "whatLlmSees": "LLM क्या देखता है",
      "turns": "बारियाँ",
      "noMessages": "कोई संदेश नहीं",
      "noMessagesHint": "प्रविष्टियाँ जोड़ें या बारियाँ बढ़ाएँ",
      "showMore": "और दिखाएँ",
      "showLess": "कम दिखाएँ",
      "statChat": "चैट",
      "statInjected": "इंजेक्टेड",
      "statTotal": "कुल",
      "entry": "प्रविष्टि",
      "editEntry": "प्रविष्टि संपादित करें",
      "reorder": "पुनर्व्यवस्थित करें",
      "delete": "हटाएँ",
      "deleteTitle": "प्रविष्टि हटाएँ?",
      "deleteMessage": "\"{{name}}\" को प्रॉम्प्ट टेम्पलेट से हटाएँ? यह पूर्ववत नहीं किया जा सकता।",
      "deleteConfirm": "हटाएँ",
      "thisEntry": "यह प्रविष्टि",
      "condensedName": "संक्षिप्त सिस्टम प्रॉम्प्ट",
      "imageAttachment": "[छवि अटैचमेंट: {{label}}]",
      "imageSlot": {
        "character": "कैरेक्टर संदर्भ छवि",
        "persona": "पर्सोना संदर्भ छवि",
        "chatBackground": "चैट बैकग्राउंड छवि",
        "avatar": "अवतार छवि",
        "references": "संदर्भ छवियाँ"
      },
      "injection": {
        "relative": "सापेक्ष",
        "inChat": "इनचैट · गहराई {{depth}}",
        "conditional": "सशर्त · न्यूनतम {{min}}",
        "interval": "अंतराल · हर {{every}}"
      }
    }
  },
  "personas": {
    "empty": {
      "title": "अभी तक कोई पर्सोना नहीं",
      "description": "AI को आपको कैसे संबोधित करना चाहिए यह परिभाषित करने के लिए एक पर्सोना बनाएँ",
      "createButton": "पर्सोना बनाएँ"
    },
    "actions": {
      "editPersona": "पर्सोना संपादित करें",
      "setAsDefault": "डिफ़ॉल्ट के रूप में सेट करें",
      "setAsDefaultDesc": "सभी नई चैट के लिए इसका उपयोग करें",
      "unsetAsDefault": "डिफ़ॉल्ट से हटाएँ",
      "unsetAsDefaultDesc": "डिफ़ॉल्ट स्थिति हटाएँ",
      "exportPersona": "पर्सोना निर्यात करें",
      "deletePersona": "पर्सोना हटाएँ"
    },
    "edit": {
      "avatarHint": "अवतार जोड़ने या जनरेट करने के लिए टैप करें",
      "nameLabel": "पर्सोना नाम",
      "namePlaceholder": "उदा., पेशेवर, रचनात्मक लेखक, छात्र...",
      "nameHint": "अपने पर्सोना को एक वर्णनात्मक नाम दें",
      "nicknameLabel": "उपनाम (वैकल्पिक)",
      "nicknamePlaceholder": "जैसे, वर्क वैरिएंट, आरपीजी मोड...",
      "nicknameHint": "आपकी लाइब्रेरी में इस व्यक्तित्व के वेरिएंट के बीच अंतर करने के लिए एक निजी उपनाम",
      "descriptionLabel": "विवरण",
      "descriptionPlaceholder": "वर्णन करें कि AI आपको कैसे संबोधित करे, आपकी प्राथमिकताएँ, पृष्ठभूमि, या संवाद शैली...",
      "wordCount": "शब्द",
      "descriptionHint": "आप कैसे संबोधित होना चाहते हैं इसके बारे में विशिष्ट रहें",
      "setAsDefault": "डिफ़ॉल्ट के रूप में सेट करें",
      "defaultDescription": "सभी नई बातचीत के लिए इस पर्सोना का उपयोग करें",
      "exportButton": "पर्सोना निर्यात करें"
    },
    "designReferences": {
      "title": "डिज़ाइन संदर्भ",
      "description": "दृश्य निर्माण के लिए कुछ स्थिर संदर्भ छवियाँ और एक संक्षिप्त डिज़ाइन नोट अटैच करें।"
    },
    "create": {
      "namePlaceholderExample": "पेशेवर लेखक",
      "descriptionPlaceholderExample": "पेशेवर, स्पष्ट और संक्षिप्त शैली में लिखें। औपचारिक भाषा का उपयोग करें और जानकारी को प्रभावी ढंग से प्रस्तुत करने पर ध्यान दें..."
    },
    "errors": {
      "exportFailed": "पर्सोना निर्यात करने में विफल",
      "importFailed": "पर्सोना आयात करने में विफल",
      "loadFailed": "पर्सोना लोड करने में विफल",
      "saveFailed": "पर्सोना सेव करने में विफल"
    },
    "importToast": {
      "legacyJsonTitle": "पुराना JSON आयात मिला",
      "legacyJsonMessage": "JSON आयात पुराना है और जल्द हट जाएगा। सेटिंग्स > फ़ाइलें कनवर्ट करें उपयोग करें।",
      "successMessage": "पर्सोना सफलतापूर्वक आयात हुआ! समीक्षा के लिए खुल रहा है।"
    }
  },
  "security": {
    "pureMode": {
      "off": "बंद",
      "offDesc": "सभी सामग्री अनुमत",
      "low": "कम",
      "lowDesc": "स्पष्ट यौन सामग्री और गालियों को ब्लॉक करता है",
      "standard": "मानक",
      "standardDesc": "NSFW और ग्राफ़िक हिंसा को ब्लॉक करता है",
      "strict": "सख्त",
      "strictDesc": "अधिकतम फ़िल्टरिंग + कोई सूचक स्वर नहीं"
    }
  },
  "advanced": {
    "sectionTitles": {
      "aiFeatures": "AI सुविधाएँ",
      "memorySystem": "मेमोरी सिस्टम",
      "usageAnalytics": "उपयोग विश्लेषण"
    },
    "creationHelper": {
      "title": "निर्माण सहायक",
      "description": "AI-मार्गदर्शित कैरेक्टर निर्माण विज़ार्ड"
    },
    "helpMeReply": {
      "title": "जवाब देने में मदद करें",
      "description": "AI-सहायित उत्तर सुझाव"
    },
    "dynamicMemory": {
      "title": "डायनामिक मेमोरी",
      "contextWindow": "संदर्भ विंडो",
      "contextWindowDesc": "शामिल करने के लिए हाल के संदेशों की संख्या (1-1000)",
      "infoText": "डायनामिक मेमोरी AI का उपयोग करके स्वचालित रूप से बातचीत के संदर्भ को सारांशित और प्रबंधित करती है, जिससे लंबी और अधिक सुसंगत बातचीत संभव होती है।",
      "disabledText": "अक्षम होने पर, ऐप संदर्भ विंडो सेटिंग द्वारा निर्धारित हाल के संदेशों की एक सरल स्लाइडिंग विंडो का उपयोग करता है।"
    },
    "usageAnalytics": {
      "recalculateTitle": "उपयोग लागत पुनर्गणना करें",
      "recalculateDesc": "सही मूल्य निर्धारण के साथ सभी ऐतिहासिक उपयोग रिकॉर्ड अपडेट करें",
      "recalculating": "पुनर्गणना हो रही है...",
      "recalculateButton": "सभी लागतें पुनर्गणना करें",
      "openRouterApiKeyRequired": "OpenRouter API कुंजी आवश्यक है। इसे सेटिंग्स → प्रदाता में कॉन्फ़िगर करें।",
      "importantLabel": "महत्वपूर्ण:",
      "warningCannotUndo": "यह ऑपरेशन पूर्ववत नहीं किया जा सकता",
      "warningMayTakeTime": "यदि आपके पास कई रिकॉर्ड हैं तो इसमें समय लग सकता है",
      "warningOnlyOpenRouter": "केवल टोकन वाले OpenRouter रिकॉर्ड अपडेट किए जाएँगे",
      "warningExistingValues": "मौजूदा लागत मान अधिलेखित किए जाएँगे"
    },
    "extra": {
      "creationHelperDetail": "व्यक्तित्व लक्षणों, पृष्ठभूमि और संवाद शैली के लिए बुद्धिमान सुझाव पाएँ",
      "helpMeReplyDetail": "बातचीत के इतिहास के आधार पर संदर्भित उत्तर विकल्प जनरेट करें",
      "lorebookEntryGenerator": "लोरबुक एंट्री जनरेटर",
      "lorebookEntryDesc": "चयनित चैट संदेशों को स्थायी लोरबुक प्रविष्टियों में बदलें और प्रविष्टि लेखन और कीवर्ड जनरेशन के लिए ड्राफ्ट प्रॉम्प्ट कॉन्फ़िगर करें।",
      "companions": "Companions",
      "companionModeDesc": "companion कैरेक्टर द्वारा उपयोग किए जाने वाले emotion, entity extraction और memory routing के लिए local analysis models प्रबंधित करें।",
      "companionSoulWriter": "Companion Soul Writer",
      "companionSoulDesc": "Companion Souls का मसौदा तैयार करने के लिए मॉडल, फ़ॉलबैक मॉडल और प्रॉम्प्ट टेम्पलेट चुनें। Tool-calling पहले, असमर्थ होने पर structured fallback।",
      "network": "नेटवर्क",
      "apiServer": "API सर्वर",
      "apiServerDesc": "OpenAI-compatible API सर्वर के माध्यम से मॉडल उजागर करें",
      "apiServerRunning": "सर्वर अभी चल रहा है"
    }
  },
  "backup": {
    "tabs": {
      "create": "बनाएँ"
    },
    "create": {
      "newBackupButton": "नया बैकअप",
      "exportDescription": "एन्क्रिप्शन के साथ सभी डेटा निर्यात करें",
      "createButton": "बैकअप बनाएँ"
    },
    "restore": {
      "availableBackups": "उपलब्ध बैकअप",
      "browseFiles": "फ़ाइलें ब्राउज़ करें",
      "noBackupsFound": "कोई बैकअप नहीं मिला",
      "noBackupsDesc": "बैकअप बनाएँ या \"फ़ाइलें ब्राउज़ करें\" पर टैप करें",
      "browseDesc": ".lettuce फ़ाइल के लिए ब्राउज़ करें",
      "restoreDialogTitle": "बैकअप पुनर्स्थापित करें",
      "deleteDialogTitle": "बैकअप हटाएँ",
      "embeddingPrompt": "डायनामिक मेमोरी एम्बेडिंग",
      "downloadModel": "मॉडल डाउनलोड करें",
      "disableAndContinue": "अक्षम करें और जारी रखें"
    },
    "extra": {
      "successMessage": "बैकअप बन गया!",
      "savedLocation": "डाउनलोड में सेव किया गया"
    }
  },
  "reset": {
    "title": "सब कुछ रीसेट करें",
    "description": "यह इस डिवाइस से सभी प्रदाता, मॉडल, कैरेक्टर, चैट सत्र और प्राथमिकताएँ स्थायी रूप से हटा देगा।",
    "warning": "यह कार्रवाई पूर्ववत नहीं की जा सकती",
    "resetButton": "सभी डेटा रीसेट करें",
    "confirmTitle": "क्या आप निश्चित हैं?",
    "confirmDescription": "आपका सभी डेटा स्थायी रूप से हटा दिया जाएगा। ऐप प्रथम-बार सेटअप पर वापस आ जाएगा।",
    "confirmButton": "हाँ, सब कुछ रीसेट करें"
  },
  "chatAppearance": {
    "typography": "टाइपोग्राफी",
    "fontSize": {
      "label": "फ़ॉन्ट आकार",
      "small": "छोटा",
      "medium": "मध्यम",
      "large": "बड़ा",
      "xLarge": "बहुत बड़ा"
    },
    "lineSpacing": {
      "label": "पंक्ति रिक्ति",
      "tight": "तंग",
      "normal": "सामान्य",
      "relaxed": "आरामदेह"
    },
    "messageBubbles": {
      "label": "संदेश बुलबुले",
      "style": {
        "label": "शैली",
        "bordered": "बॉर्डर वाला",
        "filled": "भरा हुआ",
        "minimal": "न्यूनतम"
      },
      "cornerRadius": {
        "label": "कोना त्रिज्या",
        "sharp": "तीखा",
        "rounded": "गोलाकार",
        "pill": "पिल"
      },
      "maxWidth": {
        "label": "अधिकतम चौड़ाई",
        "compact": "संक्षिप्त",
        "normal": "सामान्य",
        "wide": "चौड़ा"
      },
      "padding": {
        "label": "पैडिंग",
        "compact": "संक्षिप्त",
        "normal": "सामान्य",
        "spacious": "विस्तृत"
      }
    },
    "layout": {
      "label": "लेआउट",
      "messageSpacing": "संदेश रिक्ति",
      "tight": "तंग",
      "normal": "सामान्य",
      "relaxed": "आरामदेह"
    },
    "avatar": {
      "shape": {
        "label": "अवतार आकार",
        "circle": "वृत्त",
        "rounded": "गोलाकार",
        "hidden": "छिपा हुआ"
      },
      "size": {
        "label": "अवतार आकार",
        "small": "छोटा",
        "medium": "मध्यम",
        "large": "बड़ा"
      }
    },
    "colors": {
      "label": "रंग",
      "userBubble": "उपयोगकर्ता बुलबुला रंग",
      "assistantBubble": "सहायक बुलबुला रंग",
      "userBubbleHex": "उपयोगकर्ता बुलबुला Hex ओवरराइड",
      "assistantBubbleHex": "सहायक बुलबुला Hex ओवरराइड",
      "neutral": "तटस्थ",
      "accent": "एक्सेंट",
      "info": "जानकारी",
      "secondary": "सेकंडरी",
      "warning": "चेतावनी",
      "textColors": "Text Colors",
      "messageTextHex": "इनलाइन उद्धरण रंग",
      "plainTextHex": "Plain Text Color",
      "italicTextHex": "Italic Text Color",
      "quotedTextHex": "ब्लॉक उद्धरण रंग",
      "inlineCodeTextHex": "इनलाइन कोड रंग"
    },
    "backgroundTransparency": {
      "label": "पृष्ठभूमि और पारदर्शिता",
      "backgroundDim": "पृष्ठभूमि मंद",
      "backgroundBlur": "पृष्ठभूमि धुंधलापन",
      "bubbleBlur": "बुलबुला धुंधलापन",
      "none": "कोई नहीं",
      "light": "हल्का",
      "medium": "मध्यम",
      "heavy": "भारी",
      "bubbleOpacity": "बुलबुला अपारदर्शिता"
    },
    "textColorMode": {
      "label": "टेक्स्ट रंग मोड",
      "auto": "ऑटो",
      "light": "हल्का",
      "dark": "गहरा"
    },
    "preview": {
      "label": "पूर्वावलोकन",
      "generic": "सामान्य",
      "live": "लाइव"
    },
    "extra": {
      "reset": "रीसेट"
    }
  },
  "colorCustomization": {
    "tokens": {
      "surface": "सतह",
      "surfaceDesc": "पृष्ठ पृष्ठभूमि",
      "surfaceEl": "उठी हुई सतह",
      "surfaceElDesc": "कार्ड, मोडल, उठे हुए तत्व",
      "nav": "नेविगेशन",
      "navDesc": "ऊपर और नीचे की बार",
      "foreground": "अग्रभूमि",
      "foregroundDesc": "बॉर्डर, ओवरले, नेविगेशन और UI क्रोम",
      "appText": "ऐप पाठ",
      "appTextDesc": "मुख्य पाठ और इंटरफ़ेस लेबल",
      "appTextMuted": "मंद पाठ",
      "appTextMutedDesc": "द्वितीयक पाठ और सहायक कॉपी",
      "appTextSubtle": "सूक्ष्म पाठ",
      "appTextSubtleDesc": "संकेत, सहायक पाठ और प्लेसहोल्डर",
      "accent": "एक्सेंट",
      "accentDesc": "प्राथमिक कार्रवाइयाँ, सफलता",
      "info": "जानकारी",
      "infoDesc": "सूचनात्मक स्थितियाँ, लिंक",
      "warning": "चेतावनी",
      "warningDesc": "सावधानी स्थितियाँ, अलर्ट",
      "danger": "खतरा",
      "dangerDesc": "विनाशकारी कार्रवाइयाँ, त्रुटियाँ",
      "secondary": "सेकंडरी",
      "secondaryDesc": "AI सुविधाएँ, रचनात्मक उपकरण"
    },
    "presetsLabel": "प्रीसेट",
    "customPresetsLabel": "कस्टम प्रीसेट",
    "previewLabel": "पूर्वावलोकन",
    "settingsCardsLabel": "सेटिंग कार्ड",
    "settingsCardsOpacity": "कार्ड अपारदर्शिता",
    "settingsCardsOpacityDesc": "यह नियंत्रित करता है कि सेटिंग कार्ड और सूची पंक्तियां कितनी पारदर्शी दिखें।",
    "importButton": "आयात करें",
    "exportButton": "निर्यात करें",
    "resetAllButton": "सब रीसेट करें",
    "presets": {
      "defaultDark": "डिफ़ॉल्ट डार्क",
      "midnightBlue": "मिडनाइट ब्लू",
      "warmEarth": "वॉर्म अर्थ",
      "purpleHaze": "पर्पल हेज़",
      "rosePine": "रोज़ पाइन",
      "tokyoNight": "टोक्यो नाइट",
      "catppuccin": "Catppuccin",
      "gruvbox": "Gruvbox",
      "nord": "Nord",
      "dracula": "Dracula",
      "solarized": "Solarized",
      "ayuDark": "Ayu Dark",
      "oneDark": "One Dark",
      "vesper": "Vesper",
      "cyberNeon": "साइबर नियॉन",
      "monochrome": "मोनोक्रोम"
    },
    "groups": {
      "backgrounds": "पृष्ठभूमि",
      "content": "सामग्री",
      "semantic": "सिमैंटिक"
    },
    "extra": {
      "surface": "सतह",
      "surfaceDesc": "पृष्ठ पृष्ठभूमि",
      "surfaceEl": "उठी हुई सतह",
      "surfaceElDesc": "कार्ड, मोडल, उठे हुए तत्व",
      "nav": "नेविगेशन",
      "navDesc": "ऊपर और नीचे की बार",
      "fg": "अग्रभूमि",
      "fgDesc": "बॉर्डर, ओवरले, नेविगेशन और UI क्रोम",
      "appText": "ऐप पाठ",
      "appTextDesc": "मुख्य पाठ और इंटरफ़ेस लेबल",
      "appTextMuted": "मंद पाठ",
      "appTextMutedDesc": "द्वितीयक पाठ और सहायक कॉपी",
      "appTextSubtle": "सूक्ष्म पाठ",
      "appTextSubtleDesc": "संकेत, सहायक पाठ और प्लेसहोल्डर",
      "accent": "एक्सेंट",
      "accentDesc": "प्राथमिक कार्रवाइयाँ, सफलता",
      "info": "जानकारी",
      "infoDesc": "सूचनात्मक स्थितियाँ, लिंक",
      "warning": "चेतावनी",
      "warningDesc": "सावधानी स्थितियाँ, अलर्ट",
      "danger": "खतरा",
      "dangerDesc": "विनाशकारी कार्रवाइयाँ, त्रुटियाँ",
      "secondary": "सेकंडरी"
    }
  },
  "dynamicMemory": {
    "page": {
      "info": "डायनामिक मेमोरी स्वचालित रूप से बातचीत को सारांशित करती है ताकि संदर्भ कुशलतापूर्वक बना रहे। एक प्रीसेट चुनें या अपनी आवश्यकताओं के अनुसार सेटिंग्स को फाइन-ट्यून करें।",
      "disabledDirectTitle": "डायरेक्ट चैट के लिए डायनामिक मेमोरी अक्षम है",
      "disabledDirectDescription": "इसे सक्षम करने के लिए डायरेक्ट चैट टैब में स्विच टॉगल करें। ग्रुप चैट प्रति-सत्र मेमोरी मोड का उपयोग करते हैं।",
      "directChats": "डायरेक्ट चैट",
      "groupChats": "ग्रुप चैट",
      "enableDirectChats": "डायरेक्ट चैट के लिए सक्षम करें",
      "groupChatsInfo": "ग्रुप चैट प्रति-सत्र मेमोरी मोड का उपयोग करते हैं। प्रत्येक ग्रुप की सेटिंग्स में डायनामिक मेमोरी सक्षम करें। ये सेटिंग्स डायनामिक मेमोरी के व्यवहार को नियंत्रित करती हैं।",
      "memoryProfile": "मेमोरी प्रोफ़ाइल",
      "customSettings": "कस्टम सेटिंग्स - नीचे उन्नत विकल्पों में मान समायोजित करें।",
      "contextEnrichment": "संदर्भ संवर्धन",
      "experimental": "प्रयोगात्मक",
      "contextEnrichmentDescription": "स्मार्ट मेमोरी पुनर्प्राप्ति के लिए हाल के संदेशों का उपयोग करता है",
      "advancedOptions": "उन्नत विकल्प",
      "advancedOptionsDescription": "मेमोरी व्यवहार को फाइन-ट्यून करें",
      "summaryInterval": "सारांश अंतराल",
      "summaryIntervalDescription": "सारांशों के बीच संदेश",
      "maxMemoryEntries": "अधिकतम मेमोरी प्रविष्टियाँ",
      "maxMemoryEntriesDescription": "अधिकतम संग्रहीत स्मृतियाँ",
      "hotMemoryBudget": "हॉट मेमोरी बजट",
      "hotMemoryBudgetDescription": "सक्रिय स्मृतियों के लिए टोकन सीमा",
      "relevanceThreshold": "प्रासंगिकता सीमा",
      "relevanceThresholdDescription": "पुनर्प्राप्ति के लिए न्यूनतम समानता",
      "retrievalMode": "पुनर्प्राप्ति मोड",
      "retrievalModeSmart": "स्मार्ट",
      "retrievalModeCosine": "कोसाइन",
      "retrievalModeDescription": "स्मार्ट प्रासंगिकता को रीसेंसी/फ़्रीक्वेंसी के साथ मिलाता है। कोसाइन शुद्ध शीर्ष समानता का उपयोग करता है।",
      "retrievalLimit": "पुनर्प्राप्ति सीमा",
      "retrievalLimitDescription": "प्रति टर्न चयनित अधिकतम स्मृतियाँ",
      "decayRate": "क्षय दर",
      "decayRateDescription": "महत्व कितनी तेज़ी से कम होता है",
      "coldStorageThreshold": "कोल्ड स्टोरेज सीमा",
      "coldStorageThresholdDescription": "स्मृतियाँ कब आर्काइव में जाती हैं",
      "sharedSettings": "साझा सेटिंग्स",
      "summarisationModel": "सारांश मॉडल",
      "selectedModel": "चयनित मॉडल",
      "useGlobalDefaultModel": "वैश्विक डिफ़ॉल्ट मॉडल का उपयोग करें",
      "noModelsAvailable": "कोई मॉडल उपलब्ध नहीं",
      "summarisationModelDescription": "बातचीत सारांश के लिए उपयोग किया जाता है",
      "modelManagement": "मॉडल प्रबंधन",
      "testModel": "मॉडल परीक्षण करें",
      "downloadModel": "मॉडल डाउनलोड करें",
      "delete": "हटाएँ",
      "embeddingModel": "एम्बेडिंग मॉडल",
      "tokenCapacity": "टोकन क्षमता",
      "tokenCapacityDescription": "अधिक मान = लंबी बातचीत के लिए बेहतर मेमोरी",
      "keepModelLoaded": "मॉडल लोड रखें",
      "keepModelLoadedDescription": "रीलोड ओवरहेड से बचने के लिए एम्बेडिंग मॉडल + टोकनाइज़र को मेमोरी में रखता है",
      "installedModel": "स्थापित मॉडल: {{version}} ({{tokens}} अधिकतम टोकन)",
      "downloadEmbeddingModel": "एम्बेडिंग मॉडल डाउनलोड करें",
      "downloadEmbeddingDescription": "कौन सा संस्करण डाउनलोड करना है चुनें। स्थापित संस्करण अक्षम हैं।",
      "downloadVersion": "{{version}} डाउनलोड करें",
      "downloadV2Description": "सटीकता और लंबे-संदर्भ स्मरण के लिए अनुकूलित",
      "downloadV3Description": "नवीनतम एम्बेडिंग गुणवत्ता",
      "installed": "स्थापित",
      "selectModel": "मॉडल चुनें",
      "searchModels": "मॉडल खोजें...",
      "deleteEmbeddingTitle": "{{version}} मॉडल हटाएँ?",
      "deleteEmbeddingMessage": "क्या आप वाकई {{version}} हटाना चाहते हैं? आप इसे बाद में फिर से डाउनलोड कर सकते हैं।",
      "msgsUnit": "संदेश",
      "entriesUnit": "प्रविष्टियाँ",
      "tokensUnit": "टोकन",
      "itemsUnit": "आइटम",
      "perCycleUnit": "/ चक्र"
    },
    "presets": {
      "minimal": "न्यूनतम",
      "balanced": "संतुलित",
      "comprehensive": "व्यापक",
      "minimalDesc": "तेज़ और कुशल। केवल आवश्यक स्मृतियाँ रखता है।",
      "balancedDesc": "संदर्भ संरक्षण और प्रदर्शन का अच्छा मिश्रण।",
      "comprehensiveDesc": "अधिकतम संदर्भ। लंबी, विस्तृत बातचीत के लिए सर्वश्रेष्ठ।"
    },
    "presetInfo": {
      "minimal": "तेज़ और कुशल। केवल आवश्यक स्मृतियाँ रखता है।",
      "balanced": "संदर्भ संरक्षण और प्रदर्शन का अच्छा मिश्रण।",
      "comprehensive": "अधिकतम संदर्भ। लंबी, विस्तृत बातचीत के लिए सर्वश्रेष्ठ।"
    }
  },
  "helpMeReply": {
    "page": {
      "info": "Help Me Reply बातचीत के इतिहास के आधार पर आपके अगले संदेश के लिए संदर्भित सुझाव बनाता है। नीचे मॉडल और उत्तर शैली कॉन्फ़िगर करें।"
    },
    "labels": {
      "replyModel": "उत्तर मॉडल",
      "selectedModel": "चयनित मॉडल",
      "useAppDefault": "ऐप डिफ़ॉल्ट का उपयोग करें{{model}}",
      "useAppDefaultBase": "ऐप डिफ़ॉल्ट का उपयोग करें",
      "noModelsAvailable": "कोई मॉडल उपलब्ध नहीं",
      "replyModelDescription": "उत्तर सुझाव जनरेट करने के लिए AI मॉडल",
      "streamingOutput": "स्ट्रीमिंग आउटपुट",
      "streamingDescription": "सुझाव जनरेट होते ही दिखाएँ",
      "maxTokens": "अधिकतम टोकन",
      "maxTokensDescription": "सुझावों की अधिकतम लंबाई",
      "conversationalHint": "सुझाव प्राकृतिक संवाद के रूप में लिखे जाएँगे, सामान्य चैट के लिए उपयुक्त।",
      "roleplayHint": "सुझावों में *क्रियाएँ* और वर्णनात्मक विवरण जैसे रोलप्ले तत्व शामिल होंगे।",
      "footerInfo": "यह सेटिंग सभी बातचीतों में वैश्विक रूप से लागू होती है। कम टोकन गणना छोटे, तेज़ सुझाव बनाती है जबकि अधिक गणना अधिक विस्तृत उत्तरों की अनुमति देती है।",
      "selectReplyModel": "उत्तर मॉडल चुनें",
      "searchModels": "मॉडल खोजें..."
    },
    "sectionTitles": {
      "modelConfiguration": "मॉडल कॉन्फ़िगरेशन",
      "responseStyle": "उत्तर शैली"
    },
    "responseStyle": {
      "conversational": "बातचीत",
      "conversationalDesc": "प्राकृतिक, सामान्य स्वर",
      "roleplay": "रोलप्ले",
      "roleplayDesc": "कैरेक्टर में कार्रवाइयाँ"
    },
    "extra": {
      "conversational": "बातचीत",
      "roleplay": "रोलप्ले"
    }
  },
  "imageGeneration": {
    "promptPlaceholder": "वह छवि वर्णन करें जो आप जनरेट करना चाहते हैं...",
    "labels": {
      "model": "मॉडल",
      "prompt": "प्रॉम्प्ट",
      "size": "आकार",
      "quality": "गुणवत्ता",
      "style": "शैली",
      "searchModels": "मॉडल खोजें...",
      "selectAvatarModel": "अवतार मॉडल चुनें",
      "selectSceneModel": "दृश्य मॉडल का चयन करें",
      "selectWriterModel": "सीन राइटर मॉडल चुनें",
      "useFirstAvailable": "पहले उपलब्ध मॉडल का उपयोग करें",
      "useFirstCompatible": "पहला संगत राइटर मॉडल उपयोग करें"
    },
    "mode": {
      "title": "व्यवहार",
      "description": "तय करें कि मॉडल के आउटपुट में मिले दृश्य प्रॉम्प्ट्स को कैसे संभालना है।",
      "auto": "स्वचालित",
      "autoDescription": "जैसे ही मॉडल दृश्य प्रॉम्प्ट दे, दृश्य छवि तुरंत बनाएँ।",
      "askFirst": "पहले पूछें",
      "askFirstDescription": "मिला हुआ दृश्य प्रॉम्प्ट पहले दिखाएँ और चित्र बनाने से पहले आपकी अनुमति का इंतज़ार करें।",
      "manual": "मैनुअल",
      "manualDescription": "मॉडल के उत्तरों से मिलने वाले दृश्य प्रॉम्प्ट्स को अनदेखा करें। केवल वे क्रियाएँ उपयोग करें जिन्हें उपयोगकर्ता खुद शुरू करे।"
    },
    "empty": {
      "title": "कोई छवि मॉडल नहीं",
      "description": "छवि जनरेट करना शुरू करने के लिए मॉडल पेज से एक छवि जनरेशन मॉडल जोड़ें।"
    },
    "sections": {
      "avatar": {
        "title": "अवतार पीढ़ी",
        "description": "अवतार पिकर या संबंधित प्रोफ़ाइल छवि प्रवाह से अवतार उत्पन्न करते समय डिफ़ॉल्ट मॉडल का उपयोग किया जाता है।"
      },
      "scene": {
        "title": "दृश्य निर्माण",
        "description": "वार्तालाप संदर्भ या दृश्य संकेतों से उत्पन्न दृश्य छवियों के लिए आरक्षित मॉडल।"
      },
      "writer": {
        "title": "सीन राइटर",
        "description": "चैट संदर्भ, अवतार और संदर्भ चित्रों से सीन प्रॉम्प्ट और डिज़ाइन रेफरेंस विवरण तैयार करने के लिए आरक्षित मल्टीमॉडल टेक्स्ट मॉडल।"
      }
    },
    "extra": {
      "avatarGeneration": "अवतार पीढ़ी",
      "sceneGeneration": "दृश्य निर्माण",
      "sceneWriter": "सीन राइटर"
    }
  },
  "logs": {
    "sectionTitles": {
      "diagnostics": "डायग्नोस्टिक्स",
      "generate": "जनरेट करें",
      "copy": "कॉपी करें"
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
      "testDataGenerators": "परीक्षण डेटा जनरेटर",
      "storageMaintenance": "स्टोरेज रखरखाव",
      "usageTracking": "उपयोग ट्रैकिंग",
      "crashTesting": "क्रैश परीक्षण",
      "environmentInfo": "पर्यावरण जानकारी"
    },
    "testData": {
      "generateCharacter": "परीक्षण कैरेक्टर जनरेट करें",
      "generateCharacterDesc": "एक परीक्षण कैरेक्टर बनाएँ",
      "generatePersona": "परीक्षण पर्सोना जनरेट करें",
      "generatePersonaDesc": "एक परीक्षण पर्सोना बनाएँ",
      "generateSession": "परीक्षण सत्र जनरेट करें",
      "generateSessionDesc": "मौजूदा कैरेक्टर के साथ एक परीक्षण चैट सत्र बनाएँ",
      "generateBulk": "बल्क परीक्षण डेटा जनरेट करें",
      "generateBulkDesc": "3 कैरेक्टर और 2 पर्सोना बनाएँ"
    },
    "storageMaintenance": {
      "optimizeDb": "डेटाबेस ऑप्टिमाइज़ करें",
      "optimizeDbDesc": "PRAGMA लागू करें और VACUUM चलाएँ (केवल मोबाइल)",
      "backupLegacy": "पुरानी फ़ाइलें बैकअप करें और हटाएँ",
      "backupLegacyDesc": "पुरानी .bin स्टोरेज को बैकअप फ़ोल्डर में ले जाता है"
    },
    "usageTracking": {
      "recalculateAll": "सभी उपयोग लागतें पुनर्गणना करें",
      "recalculateAllDesc": "सभी OpenRouter उपयोग रिकॉर्ड के लिए मूल्य निर्धारण फिर से लाता है और लागत पुनर्गणना करता है"
    },
    "crashTesting": {
      "forceCrash": "अभी क्रैश ऐप",
      "forceCrashDesc": "क्रैश डिटेक्शन का परीक्षण करने के लिए मूल ऐप प्रक्रिया को तुरंत समाप्त कर देता है",
      "forceCrashConfirm": "यह क्रैश डिटेक्टर का परीक्षण करने के लिए ऐप को तुरंत क्रैश कर देगा। जारी रखना?"
    },
    "environmentInfo": {
      "mode": "मोड",
      "devMode": "डेव मोड",
      "viteVersion": "Vite संस्करण"
    },
    "status": {
      "testCharacterCreated": "✓ परीक्षण कैरेक्टर सफलतापूर्वक बनाया गया",
      "testPersonaCreated": "✓ परीक्षण पर्सोना सफलतापूर्वक बनाया गया",
      "testSessionCreated": "✓ परीक्षण सत्र बनाया गया: {{id}}",
      "generatingBulkData": "बल्क परीक्षण डेटा जनरेट हो रहा है...",
      "bulkDataCreated": "✓ बल्क परीक्षण डेटा बनाया गया: 3 कैरेक्टर, 2 पर्सोना",
      "creatingBenchmarkChat": "सीडेड बेंचमार्क कैरेक्टर और सत्र बना रहे हैं...",
      "seededBenchmarkReady": "✓ सीडेड बेंचमार्क तैयार: {{name}} / {{id}}",
      "creatingBenchmarkGroupChat": "सीडेड बेंचमार्क ग्रुप चैट बना रहे हैं...",
      "seededGroupBenchmarkReady": "✓ सीडेड ग्रुप बेंचमार्क तैयार: {{id}}",
      "dbOptimized": "✓ डेटाबेस ऑप्टिमाइज़ हो गया",
      "recalculatingCosts": "उपयोग लागतों की पुनर्गणना हो रही है... इसमें कुछ समय लग सकता है।",
      "toursReset": "✓ सभी गाइडेड टूर रीसेट हो गए — अगली विज़िट पर फिर दिखाई देंगे",
      "crashingApp": "ऐप क्रैश हो रहा है..."
    },
    "errors": {
      "noCharacters": "कोई कैरेक्टर उपलब्ध नहीं। पहले एक परीक्षण कैरेक्टर बनाएँ।",
      "createCharacterFailed": "परीक्षण कैरेक्टर बनाने में विफल: {{error}}",
      "createPersonaFailed": "परीक्षण पर्सोना बनाने में विफल: {{error}}",
      "createSessionFailed": "परीक्षण सत्र बनाने में विफल: {{error}}",
      "createBulkFailed": "बल्क परीक्षण डेटा बनाने में विफल: {{error}}",
      "createBenchmarkFailed": "सीडेड बेंचमार्क सत्र बनाने में विफल: {{error}}",
      "createGroupBenchmarkFailed": "सीडेड बेंचमार्क ग्रुप सत्र बनाने में विफल: {{error}}",
      "dbOptimizeFailed": "DB ऑप्टिमाइज़ विफल: {{error}}",
      "backupFailed": "बैकअप विफल: {{error}}",
      "openRouterKeyMissing": "OpenRouter API कुंजी नहीं मिली। कृपया पहले सेटिंग्स > प्रदाता में कॉन्फ़िगर करें।",
      "recalculationFailed": "पुनर्गणना विफल: {{error}}",
      "resetToursFailed": "टूर रीसेट करने में विफल: {{error}}",
      "crashFailed": "ऐप क्रैश करने में विफल: {{error}}"
    },
    "onboarding": {
      "title": "ऑनबोर्डिंग",
      "resetTours": "सभी गाइडेड टूर रीसेट करें",
      "resetToursDesc": "हर ऑनबोर्डिंग टूर की देखी गई स्थिति साफ़ करता है ताकि वे अगली विज़िट पर फिर चलें।"
    },
    "benchmarks": {
      "createChat": "सीडेड बेंचमार्क चैट बनाएँ",
      "createChatDesc": "एक डायनामिक-मेमोरी कैरेक्टर, शुरुआती सीन और 20-मैसेज कंटीन्यूइटी टेस्ट सत्र बनाता है, फिर उसे खोलता है।",
      "createGroupChat": "सीडेड बेंचमार्क ग्रुप चैट बनाएँ",
      "createGroupChatDesc": "तीन बेंचमार्क कैरेक्टर और 30 सीडेड मैसेज के साथ एक डायनामिक-मेमोरी ग्रुप चैट बनाता है, फिर उसे खोलता है।"
    },
    "extra": {
      "testCharacter": "परीक्षण कैरेक्टर",
      "testCharacterDesc": "विकास उद्देश्यों के लिए बनाया गया एक परीक्षण कैरेक्टर।",
      "testScene": "विकास के लिए एक सरल परीक्षण सीन",
      "testPersona": "परीक्षण पर्सोना",
      "testPersonaDesc": "विकास के लिए एक परीक्षण पर्सोना",
      "successChar": "✓ परीक्षण कैरेक्टर सफलतापूर्वक बनाया गया",
      "successPersona": "✓ परीक्षण पर्सोना सफलतापूर्वक बनाया गया",
      "successSession": "✓ परीक्षण सत्र बनाया गया: {{id}}",
      "successBulk": "✓ बल्क परीक्षण डेटा बनाया गया: 3 कैरेक्टर, 2 पर्सोना",
      "errorCharAvailable": "कोई कैरेक्टर उपलब्ध नहीं। पहले एक परीक्षण कैरेक्टर बनाएँ।",
      "generatingBulk": "बल्क परीक्षण डेटा जनरेट हो रहा है..."
    }
  },
  "embeddingDownload": {
    "capacityOptions": {
      "oneK": "1K टोकन",
      "oneKDesc": "त्वरित उत्तरों के लिए सर्वोत्तम",
      "twoK": "2K टोकन",
      "twoKDesc": "संतुलित प्रदर्शन",
      "fourK": "4K टोकन",
      "fourKDesc": "अधिकतम संदर्भ"
    },
    "extra": {
      "status": "डाउनलोड हो रहा है..."
    }
  },
  "embeddingTest": {
    "categories": {
      "semanticSimilarity": "सिमैंटिक समानता",
      "dissimilarityCheck": "असमानता जाँच",
      "roleplayContext": "रोलप्ले संदर्भ"
    },
    "extra": {
      "placeholder": "एम्बेड करने के लिए टेक्स्ट दर्ज करें..."
    }
  },
  "discovery": {
    "tabs": {
      "forYou": "आपके लिए",
      "trending": "ट्रेंडिंग",
      "popular": "लोकप्रिय",
      "new": "नया"
    },
    "searchPlaceholder": "कैरेक्टर खोजें...",
    "viewAll": "सभी देखें",
    "errorTitle": "कुछ गलत हो गया",
    "noCardsFound": "कोई कार्ड नहीं मिला",
    "sections": {
      "trendingNow": "अभी ट्रेंडिंग",
      "trendingSubtitle": "इस सप्ताह लोकप्रिय",
      "mostPopular": "सबसे लोकप्रिय",
      "popularSubtitle": "समुदाय के पसंदीदा",
      "freshArrivals": "नई आमद",
      "freshSubtitle": "अभी जोड़े गए"
    },
    "browse": {
      "newArrivals": "नई आमद",
      "freshCharacters": "नए कैरेक्टर",
      "noCharactersFound": "कोई कैरेक्टर नहीं मिला",
      "noCharactersSubtitle": "नई सामग्री के लिए बाद में देखें"
    },
    "sort": {
      "mostLiked": "सबसे पसंदीदा",
      "mostDownloaded": "सबसे डाउनलोड किए गए",
      "mostViewed": "सबसे देखे गए",
      "mostMessages": "सबसे अधिक संदेश",
      "newestFirst": "नवीनतम पहले",
      "recentlyUpdated": "हाल ही में अपडेट किए गए",
      "nameAZ": "नाम (A-Z)"
    },
    "sortBy": "क्रमबद्ध करें",
    "resultsUnit": "कैरेक्टर",
    "detail": {
      "share": "शेयर करें",
      "nsfwOverlay": "NSFW सामग्री",
      "nsfwBadge": "NSFW",
      "originalBadge": "मूल",
      "lorebookBadge": "लोरबुक",
      "alsoKnownAs": "इसे ये नाम भी:",
      "followersUnit": "फ़ॉलोअर्स",
      "sections": {
        "description": "विवरण",
        "tokenUsage": "टोकन उपयोग",
        "startingScenes": "शुरुआती दृश्य",
        "scenario": "परिदृश्य",
        "personality": "व्यक्तित्व",
        "stats": "आँकड़े",
        "tags": "टैग",
        "author": "लेखक"
      },
      "tokensTotalLabel": "कुल",
      "tokens": {
        "description": "विवरण",
        "personality": "व्यक्तित्व",
        "scenario": "परिदृश्य",
        "firstMessage": "पहला संदेश",
        "scenes": "दृश्य",
        "examples": "उदाहरण",
        "systemPrompt": "सिस्टम प्रॉम्प्ट"
      },
      "sceneLabels": {
        "primary": "प्राथमिक",
        "alternate": "वैकल्पिक"
      },
      "stats": {
        "views": "दृश्य",
        "downloads": "डाउनलोड",
        "messages": "संदेश"
      },
      "downloaded": "डाउनलोड किया गया",
      "startChat": "चैट शुरू करें",
      "downloadCharacter": "कैरेक्टर डाउनलोड करें",
      "downloading": "डाउनलोड हो रहा है...",
      "downloadSuccess": {
        "title": "कैरेक्टर डाउनलोड हो गया!",
        "subtitle": "आपकी लाइब्रेरी में जोड़ा गया",
        "badge": "सेव किया गया",
        "startChat": "चैट शुरू करें",
        "startChatDesc": "अभी पहला दृश्य खोलें",
        "viewLibrary": "लाइब्रेरी में देखें",
        "viewLibraryDesc": "बाद में संपादित, प्रबंधित, या निर्यात करें",
        "continueBrowsing": "ब्राउज़िंग जारी रखें",
        "continueBrowsingDesc": "खोज पर वापस"
      },
      "errorTitle": "त्रुटि",
      "errorSubtitle": "लोड करने में विफल",
      "errorNotFound": "कैरेक्टर नहीं मिला",
      "defaultChatTitle": "नई चैट"
    },
    "errors": {
      "loadContent": "सामग्री लोड करने में विफल",
      "searchFailed": "खोज विफल",
      "noCardPath": "कोई कार्ड पथ प्रदान नहीं किया गया",
      "loadCharacter": "कैरेक्टर लोड करने में विफल",
      "downloadCharacter": "कैरेक्टर डाउनलोड करने में विफल"
    },
    "card": {
      "byAuthor": "{{author}} द्वारा",
      "ocBadge": "OC"
    },
    "search": {
      "placeholder": "कैरेक्टर, टैग, लेखक खोजें...",
      "resultsUnit": "परिणाम",
      "timingUnit": "ms",
      "recentSearches": "हाल की खोजें",
      "clearAll": "सब साफ़ करें",
      "trendingSearches": "ट्रेंडिंग खोजें",
      "trends": {
        "anime": "एनीमे",
        "fantasy": "फ़ैंटेसी",
        "romance": "रोमांस",
        "villain": "खलनायक",
        "adventure": "साहसिक",
        "comedy": "कॉमेडी",
        "mystery": "रहस्य",
        "sciFi": "साई-फ़ाई"
      },
      "tips": {
        "title": "खोज टिप्स",
        "tip1": "कैरेक्टर नाम, लेखक, या विवरण से खोजें",
        "tip2": "\"एनीमे\", \"फ़ैंटेसी\", या \"रोमांस\" जैसे टैग उपयोग करें",
        "tip3": "\"सुंदेरे\" या \"खलनायक\" जैसी विशिष्ट विशेषताएँ आज़माएँ"
      },
      "loading": "लोड हो रहा है...",
      "loadMore": "और लोड करें",
      "noResults": "कोई परिणाम नहीं मिला",
      "noResultsFor": "इसके लिए कोई कैरेक्टर नहीं मिला",
      "noResultsHint": "अलग कीवर्ड आज़माएँ या श्रेणियाँ ब्राउज़ करें"
    }
  },
  "engine": {
    "gpuInsufficient": "GPU मेमोरी अपर्याप्त",
    "gpuFallbackDesc": "यह मॉडल GPU मेमोरी में नहीं समाता। CPU (धीमा) पर स्विच करें या रद्द करें?",
    "switchToCpu": "CPU पर स्विच करें",
    "abort": "रद्द करें",
    "errors": {
      "providerNotFound": "इंजन प्रदाता नहीं मिला।",
      "engineOffline": "इंजन ऑफ़लाइन या अनुपलब्ध है।",
      "deleteCharacterFailed": "कैरेक्टर हटाने में विफल।",
      "unknownCharacter": "अज्ञात",
      "seedRequired": "बीज विवरण आवश्यक है।",
      "characterNameRequired": "कैरेक्टर का नाम आवश्यक है।",
      "atLeastOneProvider": "कम से कम एक प्रदाता सक्षम होना चाहिए।",
      "enableLlmProvider": "कृपया कम से कम एक LLM प्रदाता सक्षम करें।",
      "modelRequired": "{{provider}} के लिए मॉडल आवश्यक है।",
      "apiKeyRequired": "{{provider}} के लिए API कुंजी आवश्यक है।",
      "sendMessageFailed": "संदेश भेजने में विफल।"
    },
    "status": {
      "connected": "कनेक्टेड",
      "offline": "ऑफ़लाइन",
      "needsSetup": "सेटअप आवश्यक"
    },
    "home": {
      "characters": "कैरेक्टर",
      "newButton": "नया",
      "noCharactersFound": "कोई कैरेक्टर नहीं मिला।",
      "tokenUsage": "टोकन उपयोग",
      "totalTokens": "कुल टोकन",
      "backgroundActivity": "पृष्ठभूमि गतिविधि",
      "quickActions": "त्वरित कार्रवाइयाँ",
      "configureProviders": "प्रदाता कॉन्फ़िगर करें",
      "engineSettings": "इंजन सेटिंग्स",
      "chat": "चैट",
      "chatDesc": "इस कैरेक्टर के साथ बातचीत शुरू करें",
      "deleteCharacter": "कैरेक्टर हटाएँ",
      "deletingCharacter": "हटाया जा रहा है...",
      "deleteDesc": "इस कैरेक्टर को स्थायी रूप से हटाएँ",
      "character": "कैरेक्टर",
      "never": "कभी नहीं",
      "justNow": "अभी",
      "timeAgo": {
        "minutes": "{{n}}मि पहले",
        "hours": "{{n}}घं पहले",
        "days": "{{n}}दिन पहले"
      }
    },
    "tokens": {
      "input": "इनपुट",
      "output": "आउटपुट"
    },
    "activity": {
      "synthesis": "संश्लेषण",
      "consolidation": "समेकन",
      "bm25Rebuild": "BM25 पुनर्निर्माण",
      "dripResearch": "ड्रिप रिसर्च",
      "running": "चल रहा है",
      "stopped": "रुका हुआ"
    },
    "setup": {
      "complete": "सेटअप पूर्ण!",
      "completeMessage": "आपका Lettuce इंजन कॉन्फ़िगर और तैयार है।",
      "openDashboard": "डैशबोर्ड खोलें"
    },
    "welcome": {
      "title": "Lettuce इंजन में स्वागत है",
      "subtitle": "चलिए आपका AI कैरेक्टर इंजन कॉन्फ़िगर करते हैं। इसमें लगभग 2 मिनट लगेंगे।",
      "feature1": "इंजन आपके AI कैरेक्टर्स को स्थायी मेमोरी, भावनाएँ, संबंध और एक वास्तविक पहचान देता है।",
      "feature2": "पहले, हम एक LLM बैकएंड सेट करेंगे, फिर आपकी इंजन सेटिंग्स कॉन्फ़िगर करेंगे।",
      "getStarted": "चलिए शुरू करते हैं"
    },
    "config": {
      "activeProviders": "सक्रिय प्रदाता",
      "noModelSet": "कोई मॉडल सेट नहीं",
      "defaultBadge": "डिफ़ॉल्ट",
      "noProvidersWarning": "कोई प्रदाता कॉन्फ़िगर नहीं। नीचे कम से कम एक LLM बैकएंड जोड़ें।",
      "addProvider": "प्रदाता जोड़ें",
      "quickImport": "अपने ऐप प्रदाताओं से त्वरित आयात",
      "importButton": "आयात करें",
      "fields": {
        "model": "मॉडल",
        "modelPlaceholder": "उदा. claude-sonnet-4-5-20250929",
        "apiKey": "API कुंजी",
        "apiKeyPlaceholder": "अपनी API कुंजी दर्ज करें",
        "currentKey": "वर्तमान कुंजी:",
        "baseUrl": "बेस URL",
        "baseUrlPlaceholder": "http://localhost:11434",
        "maxTokens": "अधिकतम टोकन",
        "temperature": "तापमान"
      },
      "enableProvider": "प्रदाता सक्षम करें",
      "setAsDefault": "डिफ़ॉल्ट के रूप में सेट करें",
      "defaultBackend": "डिफ़ॉल्ट बैकएंड",
      "remove": "हटाएँ",
      "saveChanges": "परिवर्तन सेव करें",
      "saving": "सेव हो रहा है...",
      "saved": "सेव हो गया"
    },
    "providers": {
      "title": "LLM प्रदाता",
      "subtitle": "इंजन को कार्य करने के लिए कम से कम एक LLM बैकएंड की आवश्यकता है। नीचे एक या अधिक प्रदाता कॉन्फ़िगर करें।",
      "importFromProviders": "अपने प्रदाताओं से आयात करें",
      "imported": "आयातित",
      "use": "उपयोग करें",
      "saveContinue": "सेव करें और जारी रखें"
    },
    "settings": {
      "fields": {
        "dataDirectory": "डेटा डायरेक्टरी",
        "logLevel": "लॉग स्तर",
        "maxHistory": "अधिकतम इतिहास (बातचीत बदलाव)"
      },
      "logLevels": {
        "debug": "डीबग",
        "info": "जानकारी",
        "warning": "चेतावनी",
        "error": "त्रुटि"
      },
      "sections": {
        "engine": "इंजन",
        "backgroundLoops": "पृष्ठभूमि लूप",
        "memory": "मेमोरी",
        "safety": "सुरक्षा",
        "research": "रिसर्च"
      },
      "backgroundLoops": {
        "synthesis": "संश्लेषण (मिनट)",
        "consolidation": "समेकन (मिनट)",
        "bm25Rebuild": "BM25 पुनर्निर्माण (मिनट)",
        "dripResearch": "ड्रिप रिसर्च (मिनट)"
      },
      "memory": {
        "embeddingModel": "एम्बेडिंग मॉडल",
        "maxRetrieval": "अधिकतम पुनर्प्राप्ति परिणाम",
        "denseWeight": "डेंस वेट",
        "bm25Weight": "BM25 वेट",
        "graphWeight": "ग्राफ वेट",
        "recencyBoost": "रीसेंसी बूस्ट (घंटे)",
        "randomSurface": "रैंडम सरफ़ेस प्रायिकता"
      },
      "safety": {
        "honestySection": "ईमानदारी अनुभाग",
        "honestyDesc": "सिस्टम प्रॉम्प्ट में ईमानदारी अनुभाग शामिल करें",
        "userDataDeletion": "उपयोगकर्ता डेटा हटाना",
        "userDataDesc": "उपयोगकर्ताओं को डेटा हटाने का अनुरोध करने दें"
      },
      "research": {
        "scrapeOnBoot": "बूट पर स्क्रैप",
        "scrapeDesc": "इंजन स्टार्टअप पर रिसर्च स्क्रैप चलाएँ",
        "periodicInterval": "आवधिक अंतराल (घंटे)"
      },
      "saveChanges": "परिवर्तन सेव करें",
      "saving": "सेव हो रहा है...",
      "saved": "सेव हो गया"
    },
    "settingsStep": {
      "title": "इंजन सेटिंग्स",
      "subtitle": "इंजन-व्यापी सेटिंग्स कॉन्फ़िगर करें। सभी में समझदार डिफ़ॉल्ट हैं — बेझिझक छोड़ दें।",
      "completingSetup": "सेटअप पूरा हो रहा है...",
      "completeSetup": "सेटअप पूरा करें"
    },
    "chat": {
      "sendMessage": "एक संदेश भेजें...",
      "sendButton": "संदेश भेजें",
      "typeMessage": "एक संदेश टाइप करें",
      "back": "वापस",
      "assistantTyping": "सहायक टाइप कर रहा है",
      "fallbackName": "चैट"
    },
    "tagInput": {
      "addMore": "और जोड़ें...",
      "typeAndPressEnter": "टाइप करें और Enter दबाएँ"
    },
    "characterCreate": {
      "steps": {
        "identity": {
          "title": "पहचान",
          "aiGenerated": "AI जनरेटेड",
          "nameLabel": "नाम *",
          "namePlaceholder": "कैरेक्टर का नाम",
          "eraLabel": "युग",
          "eraPlaceholder": "उदा. आधुनिक, विक्टोरियन",
          "roleLabel": "भूमिका",
          "rolePlaceholder": "उदा. जासूस, वैज्ञानिक",
          "settingLabel": "सेटिंग",
          "settingPlaceholder": "वर्णन करें कि कैरेक्टर कहाँ रहता है (प्रथम पुरुष)...",
          "coreIdentityLabel": "मूल पहचान",
          "coreIdentityPlaceholder": "यह कैरेक्टर अपने मूल में कौन है? (प्रथम पुरुष, 3-5 वाक्य)",
          "backstoryLabel": "पृष्ठभूमि कथा",
          "backstoryPlaceholder": "जीवन कथा और प्रमुख घटनाएँ (प्रथम पुरुष)..."
        },
        "mode": {
          "title": "कैरेक्टर बनाएँ",
          "subtitle": "AI के साथ कैरेक्टर जनरेट करें या शुरू से बनाएँ।",
          "aiBoost": "AI बूस्ट",
          "aiBoostDesc": "अपने कैरेक्टर विचार का वर्णन करें और AI एक पूर्ण कैरेक्टर परिभाषा जनरेट करेगा।",
          "nameOptional": "नाम (वैकल्पिक)",
          "namePlaceholder": "उदा. मार्कस कोल",
          "seedDescription": "बीज विवरण *",
          "seedPlaceholder": "उदा. 1950 के दशक के हार्लेम में जैज़ पियानोवादक, दार्शनिक, देर रात की बातचीत पसंद करता है",
          "eraOptional": "युग (वैकल्पिक)",
          "eraPlaceholder": "उदा. 1950 का दशक, आधुनिक, विक्टोरियन",
          "generating": "जनरेट हो रहा है...",
          "generateCharacter": "कैरेक्टर जनरेट करें",
          "or": "या",
          "startFromScratch": "शुरू से बनाएँ"
        },
        "personality": {
          "title": "व्यक्तित्व",
          "traits": "व्यक्तित्व विशेषताएँ",
          "traitsPlaceholder": "उदा. चतुर, दयालु, ज़िद्दी",
          "speechPatterns": "भाषण पैटर्न",
          "formality": "औपचारिकता",
          "formal": "औपचारिक",
          "casual": "अनौपचारिक",
          "texting": "टेक्स्टिंग",
          "verbosity": "वाचालता",
          "terse": "संक्षिप्त",
          "medium": "मध्यम",
          "verbose": "विस्तृत",
          "textStyle": "टेक्स्ट शैली",
          "dialect": "बोली",
          "dialectPlaceholder": "उदा. दक्षिणी अमेरिकी, ब्रिटिश RP",
          "catchphrases": "कैचफ़्रेज़",
          "catchphrasesPlaceholder": "उदा. अरे वाह...",
          "vocabPreferences": "शब्दावली प्राथमिकताएँ",
          "vocabPreferencesPlaceholder": "जो शब्द वे पसंद करते हैं",
          "vocabAvoidances": "शब्दावली परिहार",
          "vocabAvoidancesPlaceholder": "जो शब्द वे टालते हैं",
          "fillerWords": "भराव शब्द",
          "fillerWordsPlaceholder": "उदा. उम, जैसे, आप जानते हैं",
          "exampleQuotes": "उदाहरण उद्धरण",
          "exampleQuotesPlaceholder": "3-5 उदाहरण संवाद पंक्तियाँ"
        },
        "world": {
          "title": "विश्व और व्यवहार",
          "knowledgeDomains": "ज्ञान क्षेत्र",
          "knowledgeDomainsPlaceholder": "उदा. जैज़ इतिहास, संगीत सिद्धांत",
          "knowledgeBoundaries": "ज्ञान सीमाएँ",
          "knowledgeBoundariesPlaceholder": "विषय जो वे नहीं जानते",
          "researchSeeds": "रिसर्च बीज",
          "researchSeedsPlaceholder": "पृष्ठभूमि अनुसंधान के लिए शुरुआती विषय",
          "researchEnabled": "रिसर्च सक्षम",
          "researchEnabledDesc": "पृष्ठभूमि ज्ञान संग्रहण की अनुमति दें",
          "physicalDescription": "शारीरिक विवरण",
          "physicalDescPlaceholder": "शारीरिक रूप और हावभाव...",
          "physicalHabits": "शारीरिक आदतें",
          "physicalHabitsPlaceholder": "उदा. उंगलियाँ थपथपाता है, चश्मा ठीक करता है",
          "idleBehaviors": "निष्क्रिय व्यवहार",
          "idleBehaviorsPlaceholder": "जब व्यस्त नहीं होते तब वे क्या करते हैं",
          "timeBehaviors": "समय व्यवहार",
          "timePlaceholder": "{{period}} के दौरान वे क्या करते हैं?",
          "earlyMorning": "सुबह-सवेरे",
          "morning": "सुबह",
          "afternoon": "दोपहर",
          "evening": "शाम",
          "night": "रात",
          "baselineEmotions": "आधारभूत भावनाएँ (Plutchik)",
          "emotionDesc": "डिफ़ॉल्ट भावनात्मक आधारभूत सेट करें (0 = कोई नहीं, 1 = अधिकतम)",
          "joy": "आनंद",
          "trust": "विश्वास",
          "fear": "भय",
          "surprise": "आश्चर्य",
          "sadness": "उदासी",
          "disgust": "घृणा",
          "anger": "क्रोध",
          "anticipation": "प्रत्याशा",
          "engineOverrides": "इंजन ओवरराइड",
          "backend": "बैकएंड",
          "model": "मॉडल",
          "temperature": "तापमान",
          "leaveEmpty": "डिफ़ॉल्ट के लिए खाली छोड़ें"
        },
        "review": {
          "title": "समीक्षा",
          "subtitle": "बनाने से पहले अपने कैरेक्टर की समीक्षा करें।",
          "edit": "संपादित करें",
          "notSet": "सेट नहीं",
          "identitySection": "पहचान",
          "personalitySection": "व्यक्तित्व",
          "worldSection": "विश्व और व्यवहार",
          "nameLabel": "नाम",
          "eraLabel": "युग",
          "roleLabel": "भूमिका",
          "settingLabel": "सेटिंग",
          "coreIdentityLabel": "मूल पहचान",
          "backstoryLabel": "पृष्ठभूमि कथा",
          "traitsLabel": "विशेषताएँ",
          "formalityLabel": "औपचारिकता",
          "verbosityLabel": "वाचालता",
          "dialectLabel": "बोली",
          "catchphrasesLabel": "कैचफ़्रेज़",
          "domainsLabel": "क्षेत्र",
          "boundariesLabel": "सीमाएँ",
          "researchSeedsLabel": "रिसर्च बीज",
          "researchLabel": "रिसर्च",
          "enabled": "सक्षम",
          "disabled": "अक्षम",
          "physicalLabel": "शारीरिक",
          "habitsLabel": "आदतें",
          "idleLabel": "निष्क्रिय",
          "timeBehaviorsLabel": "समय व्यवहार",
          "emotionsLabel": "भावनाएँ",
          "configured": "कॉन्फ़िगर किया गया",
          "backendLabel": "बैकएंड",
          "modelLabel": "मॉडल",
          "temperatureLabel": "तापमान",
          "creating": "बनाया जा रहा है...",
          "createCharacter": "कैरेक्टर बनाएँ"
        }
      }
    }
  },
  "library": {
    "filterTitle": "लाइब्रेरी फ़िल्टर करें",
    "filters": {
      "all": "सभी",
      "characters": "कैरेक्टर",
      "personas": "पर्सोना",
      "lorebooks": "लोरबुक्स",
      "images": "छवियाँ"
    },
    "emptyStates": {
      "all": {
        "title": "आपकी लाइब्रेरी खाली है",
        "description": "यहाँ देखने के लिए कैरेक्टर, पर्सोना और लोरबुक बनाएँ"
      },
      "characters": {
        "title": "अभी तक कोई कैरेक्टर नहीं",
        "description": "चैट शुरू करने के लिए अपना पहला कैरेक्टर बनाएँ"
      },
      "personas": {
        "title": "अभी तक कोई पर्सोना नहीं",
        "description": "अपनी चैट पहचान अनुकूलित करने के लिए एक पर्सोना बनाएँ"
      },
      "lorebooks": {
        "title": "अभी तक कोई लोरबुक नहीं",
        "description": "लोरबुक कैरेक्टर की सेटिंग्स के भीतर से बनाई जाती हैं"
      }
    },
    "actions": {
      "startChat": "चैट शुरू करें",
      "editCharacter": "कैरेक्टर संपादित करें",
      "editPersona": "पर्सोना संपादित करें",
      "editLorebook": "लोरबुक संपादित करें",
      "renameLorebook": "लोरबुक का नाम बदलें",
      "exportCharacter": "कैरेक्टर निर्यात करें",
      "exportPersona": "पर्सोना निर्यात करें",
      "chatAppearance": "चैट दिखावट",
      "deleteCharacter": "कैरेक्टर हटाएँ",
      "deletePersona": "पर्सोना हटाएँ",
      "deleteLorebook": "लोरबुक हटाएँ",
      "importLorebook": "लोरबुक आयात करें"
    },
    "imageLibrary": {
      "filters": {
        "all": "सभी",
        "backgrounds": "बैकग्राउंड",
        "avatars": "अवतार",
        "attachments": "अटैचमेंट",
        "other": "अन्य"
      },
      "searchPlaceholder": "फ़ाइलनाम, पथ, सत्र आईडी या एंटिटी आईडी से खोजें",
      "empty": {
        "title": "इस दृश्य के लिए कोई चित्र नहीं मिला",
        "description": "कोई दूसरा फ़िल्टर या खोज शब्द आज़माएँ। लाइब्रेरी केवल वे चित्र दिखाती है जो ऐप के स्थानीय संग्रहण में पहले से सहेजे गए हैं।"
      },
      "actions": {
        "sort": "क्रमबद्ध करें",
        "useThis": "इसे उपयोग करें",
        "using": "उपयोग हो रहा है...",
        "copyPath": "पथ कॉपी करें",
        "saving": "सहेजा जा रहा है...",
        "download": "डाउनलोड",
        "delete": "चित्र हटाएँ",
        "deleting": "हटाया जा रहा है..."
      },
      "active": "सक्रिय",
      "messages": {
        "loadFailed": "इमेज लाइब्रेरी लोड नहीं हो सकी",
        "saved": "चित्र सहेज लिया गया",
        "downloadFailed": "डाउनलोड विफल हुआ",
        "useFailed": "इस चित्र का उपयोग नहीं किया जा सका",
        "deleted": "चित्र हटा दिया गया",
        "deleteFailed": "चित्र हटाया नहीं जा सका"
      },
      "deleteConfirm": {
        "title": "चित्र हटाएँ?",
        "message": "क्या आप वाकई \"{{filename}}\" हटाना चाहते हैं? इससे वे अवतार, चैट बैकग्राउंड या संदेश अटैचमेंट टूट सकते हैं जो अभी भी इसका उपयोग कर रहे हैं।"
      },
      "sort": {
        "newest": "नवीनतम",
        "largest": "सबसे बड़ा",
        "name": "नाम"
      },
      "kinds": {
        "background": "बैकग्राउंड",
        "avatar": "अवतार",
        "attachment": "अटैचमेंट",
        "stored": "संग्रहीत"
      },
      "detailsTitle": "{{kind}} विवरण",
      "formatsLabel": "फ़ॉर्मेट",
      "storagePath": "स्टोरेज पथ",
      "contextLabel": "संदर्भ",
      "contextLinkedFallback": "लिंक किया गया",
      "show": "दिखाएँ",
      "hide": "छिपाएँ",
      "contextRoles": {
        "character": "कैरेक्टर:",
        "session": "सत्र:",
        "role": "भूमिका:"
      },
      "downloadFormat": "{{download}} फ़ॉर्मेट",
      "unknownDate": "अज्ञात",
      "clearSearch": "खोज साफ़ करें",
      "copyFilename": "फ़ाइलनाम कॉपी करें",
      "copyLabels": {
        "filename": "फ़ाइलनाम",
        "storagePath": "स्टोरेज पथ"
      },
      "copy": {
        "copied": "{{label}} कॉपी हुआ",
        "failed": "{{label}} कॉपी करने में विफल"
      }
    },
    "deleteConfirm": {
      "title": "{{itemType}} हटाएँ?",
      "message": "क्या आप वाकई हटाना चाहते हैं",
      "characterWarning": "इससे इस कैरेक्टर के साथ सभी चैट सत्र भी हट जाएँगे।"
    },
    "rename": {
      "title": "लोरबुक का नाम बदलें",
      "placeholder": "नया नाम दर्ज करें..."
    },
    "itemTypes": {
      "character": "कैरेक्टर",
      "persona": "पर्सोना",
      "lorebook": "लोरबुक"
    },
    "lorebookLabel": "लोरबुक",
    "noDescriptionYet": "अभी तक कोई विवरण नहीं",
    "errors": {
      "importLorebook": "लोरबुक आयात करने में विफल। {{error}}",
      "exportFailed": "निर्यात विफल"
    },
    "card": {
      "avatarAlt": "{{name}} अवतार"
    },
    "lorebookEditor": {
      "titleOverride": "लोरबुक - {{name}}",
      "dragToReorder": "पुनर्व्यवस्थित करने के लिए खींचें",
      "aria": {
        "generateEntry": "लोरबुक प्रविष्टि जनरेट करें",
        "editLorebook": "लोरबुक संपादित करें",
        "exportLorebook": "लोरबुक निर्यात करें"
      }
    }
  },
  "onboarding": {
    "loading": "प्रदाता लोड हो रहे हैं...",
    "stepIndicator": "चरण {{current}} / {{total}}",
    "steps": {
      "provider": "प्रदाता सेटअप",
      "model": "मॉडल सेटअप",
      "memory": "मेमोरी सिस्टम",
      "stepNofM": "चरण {{current}} / {{total}}"
    },
    "provider": {
      "availableProviders": "उपलब्ध प्रदाता",
      "chooseProvider": "एक प्रदाता चुनें",
      "titleMobile": "अपना AI प्रदाता चुनें",
      "descMobile": "शुरू करने के लिए एक AI प्रदाता चुनें। आपकी API keys आपके डिवाइस पर सुरक्षित रूप से एन्क्रिप्ट होती हैं। खाता बनाने की ज़रूरत नहीं।",
      "configureProvider": "{{name}} कॉन्फ़िगर करें",
      "connectProvider": "{{name}} कनेक्ट करें",
      "connectProviderDesc": "चैट सक्षम करने के लिए नीचे अपनी API key पेस्ट करें। Key चाहिए? प्रदाता डैशबोर्ड से प्राप्त करें।",
      "localLLMs": "Local LLMs",
      "useLocalLLMs": "मैं Local LLMs उपयोग करना चाहता हूँ",
      "browseModelLibrary": "मॉडल लाइब्रेरी ब्राउज़ करें",
      "browseModelLibraryDesc": "HuggingFace से GGUF मॉडल खोजें और डाउनलोड करें",
      "useOwnGguf": "अपनी GGUF फ़ाइलें उपयोग करें",
      "useOwnGgufDesc": "अपने डिवाइस से GGUF मॉडल और वैकल्पिक mmproj फ़ाइल चुनें",
      "fields": {
        "displayLabel": "प्रदर्शन लेबल",
        "displayLabelHint": "यह प्रदाता आपके मेनू में कैसे दिखेगा",
        "displayLabelPlaceholder": "मेरा {{name}}",
        "defaultLabelFallback": "प्रदाता",
        "apiKey": "API Key",
        "apiKeyOptional": "API Key (वैकल्पिक)",
        "apiKeyHint": "Keys स्थानीय रूप से एन्क्रिप्ट होती हैं",
        "apiKeyPlaceholderRemote": "sk-...",
        "apiKeyPlaceholderLocal": "आमतौर पर आवश्यक नहीं",
        "whereToFind": "यह कहाँ मिलेगी",
        "baseUrl": "Base URL",
        "baseUrlPlaceholderHost": "http://192.168.1.10:3333",
        "baseUrlPlaceholderLocal": "http://localhost:11434",
        "baseUrlPlaceholderRemote": "https://api.provider.com",
        "baseUrlPlaceholderIntenseRP": "http://127.0.0.1:7777/v1",
        "baseUrlHintLocal": "पोर्ट के साथ आपका स्थानीय सर्वर पता",
        "baseUrlHintHost": "आपके होस्ट डिवाइस द्वारा दिखाया गया डेस्कटॉप होस्ट URL दर्ज करें",
        "baseUrlHintRemote": "यदि आवश्यक हो तो डिफ़ॉल्ट endpoint ओवरराइड करें",
        "chatEndpoint": "Chat Endpoint",
        "systemRole": "System Role",
        "userRole": "User Role",
        "assistantRole": "Assistant Role",
        "supportsStreaming": "Streaming समर्थित",
        "mergeSameRole": "समान-भूमिका संदेश मर्ज करें",
        "toolChoiceMode": "Tool Choice Mode",
        "toolChoiceHint": "कस्टम endpoint पर tool_choice कैसे भेजा जाए यह नियंत्रित करता है।"
      },
      "toolChoice": {
        "auto": "Auto",
        "required": "Required",
        "none": "None",
        "omit": "Field छोड़ें",
        "passthrough": "Passthrough (Tool Config)"
      },
      "buttons": {
        "testConnection": "कनेक्शन जाँचें",
        "testing": "जाँच रहा है..."
      },
      "descriptions": {
        "chutes": "शीर्ष open-source मॉडलों के लिए OpenAI-compatible inference",
        "openai": "अभिव्यंजक RP के लिए GPT-5, GPT-4.1, और GPT-4o मॉडल",
        "lettuceHost": "OpenAI-style API के साथ LAN पर अपने डेस्कटॉप Lettuce Host से कनेक्ट करें",
        "anthropic": "गहरे, भावनात्मक संवाद के लिए Claude 4.5 Sonnet और Haiku",
        "aggregator": "GPT-5, Claude 4.5, Grok-3, Mixtral और अधिक मॉडल एक्सेस करें",
        "openaiCompatible": "कोई भी OpenAI-style API endpoint उपयोग करें",
        "mistral": "Mistral Small 3.2, Mixtral 8x22B, और अन्य Mistral मॉडल",
        "deepseek": "DeepSeek-V3.2-Exp, DeepSeek-R1, और अन्य उच्च-दक्षता मॉडल",
        "xai": "Grok-1.5, Grok-3, और नए xAI मॉडल",
        "zai": "GLM-4.5, GLM-4.6, और Air वेरिएंट",
        "moonshot": "Kimi-K2 Thinking और Kimi-K1 मॉडल",
        "gemini": "Gemini 2.5 Flash, 2.5 Pro, और अधिक",
        "qwen": "Qwen3-VL और नए Qwen मॉडल",
        "nvidia": "NVIDIA NIM के ज़रिए Nemotron, Llama, DeepSeek, और अधिक",
        "custom": "किसी भी कस्टम मॉडल endpoint पर LettuceAI पॉइंट करें",
        "fallback": "AI मॉडल प्रदाता"
      },
      "descriptionsShort": {
        "chutes": "Open-source मॉडल inference",
        "openai": "GPT-5, GPT-4o, GPT-4.1",
        "lettuceHost": "आपका LAN होस्ट",
        "anthropic": "Claude 4.5 Sonnet और Haiku",
        "aggregator": "Multi-model aggregator",
        "openaiCompatible": "कस्टम OpenAI endpoint",
        "mistral": "Mistral और Mixtral मॉडल",
        "deepseek": "DeepSeek-V3, R1",
        "xai": "Grok-1.5, Grok-3",
        "zai": "GLM-4.5, GLM-4.6",
        "moonshot": "Kimi-K2 Thinking",
        "gemini": "Gemini 2.5 Flash और Pro",
        "qwen": "Qwen3-VL मॉडल",
        "nvidia": "NVIDIA NIM inference",
        "custom": "कस्टम endpoint",
        "fallback": "AI प्रदाता"
      },
      "cardLabel": "{{step}}: {{name}}",
      "errors": {
        "hostUrlRequired": "Host URL आवश्यक है (उदा. http://192.168.1.10:3333)",
        "baseUrlRequired": "Base URL आवश्यक है (उदा. http://localhost:11434)",
        "apiKeyTooShort": "API key बहुत छोटी लगती है",
        "invalidApiKey": "अमान्य API key",
        "connectionFailed": "कनेक्शन विफल",
        "verificationFailed": "सत्यापन विफल",
        "failedToSave": "प्रदाता सेव करने में विफल",
        "connectionSuccessful": "कनेक्शन सफल!",
        "modelNotFound": "प्रदाता पर मॉडल नहीं मिला",
        "modelVerificationFailed": "मॉडल सत्यापन विफल",
        "failedToSaveModel": "मॉडल सेव करने में विफल"
      }
    },
    "model": {
      "noProvidersTitle": "कोई प्रदाता कॉन्फ़िगर नहीं",
      "noProvidersDesc": "डिफ़ॉल्ट मॉडल चुनने से पहले आपको एक प्रदाता कनेक्ट करना होगा।",
      "goToProviderSetup": "प्रदाता सेटअप पर जाएँ",
      "yourProviders": "आपके प्रदाता",
      "yourProvidersHint": "कौन सा प्रदाता उपयोग करना है चुनें",
      "setDefaultModel": "अपना डिफ़ॉल्ट मॉडल सेट करें",
      "setDefaultModelDesc": "LettuceAI डिफ़ॉल्ट रूप से किस प्रदाता और मॉडल नाम का उपयोग करे यह चुनें। बाद में और जोड़ सकते हैं।",
      "setDefaultModelDescDesktop": "अपना मॉडल कॉन्फ़िगर करने के लिए सूची से एक प्रदाता चुनें।",
      "modelDetails": "मॉडल विवरण",
      "modelDetailsDesc": "API पहचानकर्ता और ऐप के अंदर दिखने वाला लेबल परिभाषित करें।",
      "whichModel": "मुझे कौन सा मॉडल उपयोग करना चाहिए?",
      "nextMemorySystem": "अगला: Memory System",
      "fields": {
        "displayName": "प्रदर्शन नाम",
        "displayNamePlaceholder": "Creative mentor",
        "displayNameHint": "यह मॉडल मेनू में कैसे दिखता है",
        "modelId": "Model ID",
        "modelPathGguf": "Model Path (GGUF)",
        "modelIdPlaceholder": "उदा. gpt-4o",
        "modelPathPlaceholder": "/path/to/model.gguf",
        "modelIdHint": "API कॉल के लिए उपयोग किया जाने वाला सटीक पहचानकर्ता",
        "showList": "सूची दिखाएँ",
        "manualInput": "Manual Input",
        "refreshModelList": "मॉडल सूची रीफ़्रेश करें",
        "selectModel": "मॉडल चुनें",
        "selectAModel": "एक मॉडल चुनें...",
        "searchModels": "मॉडल खोजें...",
        "noModelsFound": "\"{{query}}\" से मेल खाने वाला कोई मॉडल नहीं मिला"
      },
      "fillBothFields": "समाप्त बटन सक्षम करने के लिए ऊपर दोनों फ़ील्ड भरें।",
      "providerNames": {
        "chutes": "Chutes",
        "intenseRpNext": "IntenseRP Next",
        "openai": "OpenAI",
        "anthropic": "Anthropic",
        "openrouter": "OpenRouter",
        "openaiCompatible": "OpenAI compatible",
        "custom": "Custom endpoint"
      }
    },
    "memory": {
      "dynamicTitle": "डायनामिक मेमोरी",
      "recommended": "अनुशंसित",
      "settingUp": "सेटअप हो रहा है...",
      "finishSetup": "सेटअप समाप्त करें",
      "promptTitle": "डायनामिक मेमोरी सेटअप करें",
      "oneLastStep": "एक आखिरी चरण",
      "downloadAndEnable": "डाउनलोड और सक्षम करें",
      "chooseStyle": "अपनी मेमोरी शैली चुनें",
      "howRemember": "आपके AI साथियों को आपके और आपकी बातचीत के बारे में विवरण कैसे याद रखने चाहिए?",
      "dynamicDescription": "संदर्भ को स्मार्ट तरीके से प्रबंधित करने के लिए <0>local embedding model</0> उपयोग करता है। यह लंबी चैट में भी token costs कम करता है और उच्च गुणवत्ता बनाए रखता है।",
      "dynamicFeatures": {
        "quality": "लंबी चैट में गुणवत्ता बनाए रखता है",
        "cost": "API costs काफी कम करता है",
        "auto": "स्वचालित context प्रबंधन",
        "zeroConfig": "कोई कॉन्फ़िगरेशन आवश्यक नहीं"
      },
      "manualTitle": "Manual Memory",
      "manualBadge": "Classic अनुभव",
      "manualDescription": "आप स्वयं संदेशों को pin करते हैं और \"World Info\" या कैरेक्टर परिभाषाएँ संपादित करते हैं। पूर्ण नियंत्रण के लिए अच्छा।",
      "manualFeatures": {
        "control": "तथ्यों पर पूर्ण नियंत्रण",
        "scenarios": "विशिष्ट परिदृश्यों के लिए सर्वोत्तम"
      },
      "setupModelMessage": "Dynamic Memory उपयोग करने के लिए, हमें आपके डिवाइस पर एक छोटा embedding model (~120MB) डाउनलोड करना होगा।",
      "setupBullets": {
        "offline": "मॉडल आपके डिवाइस पर 100% offline चलता है",
        "remembering": "Context याद रखने के लिए आवश्यक",
        "disable": "आप इसे बाद में settings में बंद कर सकते हैं"
      },
      "stepLabel": "चरण 3 / 3",
      "stepLabelMemory": "Memory System"
    },
    "welcome": {
      "appName": "LettuceAI",
      "tagline": "आपका व्यक्तिगत AI साथी। निजी, सुरक्षित, और हमेशा डिवाइस पर।",
      "features": {
        "onDevice": "केवल डिवाइस पर",
        "characterReady": "कैरेक्टर तैयार"
      },
      "betaWarning": {
        "title": "डेस्कटॉप बीटा बिल्ड",
        "description": "आप डेस्कटॉप संस्करण का उपयोग कर रहे हैं। कुछ सुविधाएँ मोबाइल से भिन्न हो सकती हैं। GitHub पर समस्याएँ रिपोर्ट करें।"
      },
      "languageSelector": {
        "title": "भाषा",
        "description": "आपके डिवाइस से स्वतः पहचानी गई। आप इसे कभी भी सेटिंग्स में बदल सकते हैं।"
      },
      "getStarted": "शुरू करें",
      "skipForNow": "अभी छोड़ दें",
      "restoreFromBackup": "बैकअप से पुनर्स्थापित करें",
      "setupTime": "सेटअप में 2 मिनट से कम लगता है",
      "skipWarning": {
        "title": "सेटअप छोड़ दें?",
        "warningTitle": "चैट के लिए प्रदाता आवश्यक",
        "warningMessage": "प्रदाता के बिना, आप संदेश भेजने में असमर्थ होंगे। आप बाद में सेटिंग्स से एक जोड़ सकते हैं।",
        "addProvider": "प्रदाता जोड़ें",
        "skipAnyway": "फिर भी छोड़ दें"
      },
      "restoreBackup": {
        "title": "बैकअप पुनर्स्थापित करें",
        "selectMessage": "पुनर्स्थापित करने के लिए एक बैकअप चुनें।",
        "browse": "फ़ाइलें ब्राउज़ करें",
        "processing": "फ़ाइल प्रोसेस हो रही है...",
        "processingNote": "बड़े बैकअप में एक मिनट लग सकता है",
        "noBackups": "कोई बैकअप नहीं मिला",
        "noBackupsHint": ".lettuce फ़ाइल चुनने के लिए ब्राउज़ पर टैप करें",
        "browseLettuce": ".lettuce फ़ाइल के लिए ब्राउज़ करें",
        "passwordLabel": "बैकअप पासवर्ड",
        "passwordPlaceholder": "पासवर्ड दर्ज करें",
        "restoreButton": "बैकअप पुनर्स्थापित करें",
        "restoring": "पुनर्स्थापित हो रहा है...",
        "infoMessage": "यह ऐप को आपके बैकअप डेटा के साथ सेटअप करेगा, जिसमें कैरेक्टर, चैट और सेटिंग्स शामिल हैं।",
        "embeddingTitle": "एम्बेडिंग मॉडल आवश्यक",
        "dynamicMemoryDetected": "डायनामिक मेमोरी पाई गई",
        "dynamicMemoryMessage": "इस बैकअप में डायनामिक मेमोरी सक्षम कैरेक्टर हैं, जिसके लिए एम्बेडिंग मॉडल (~120MB) आवश्यक है।",
        "embeddingOptions": "आप डायनामिक मेमोरी सक्षम करने के लिए अभी मॉडल डाउनलोड कर सकते हैं, या इसके बिना जारी रख सकते हैं (प्रभावित कैरेक्टर्स के लिए डायनामिक मेमोरी अक्षम हो जाएगी)।",
        "downloadModel": "मॉडल डाउनलोड करें",
        "continueWithoutDynamic": "डायनामिक मेमोरी के बिना जारी रखें",
        "embeddingNote": "मॉडल डाउनलोड करने के बाद आप कैरेक्टर सेटिंग्स में डायनामिक मेमोरी फिर से सक्षम कर सकते हैं।",
        "back": "वापस",
        "cancel": "रद्द करें",
        "errors": {
          "passwordRequired": "पासवर्ड आवश्यक है",
          "incorrectPassword": "गलत पासवर्ड",
          "failedToOpenFile": "फ़ाइल खोलने में विफल",
          "failedToRestore": "बैकअप पुनर्स्थापित करने में विफल",
          "failedToUpdateSettings": "सेटिंग्स अपडेट करने में विफल"
        }
      }
    },
    "common": {
      "back": "वापस",
      "cancel": "रद्द करें",
      "continue": "जारी रखें",
      "verifying": "सत्यापित हो रहा है...",
      "skipForNow": "अभी छोड़ दें",
      "selectAProvider": "कॉन्फ़िगर करने के लिए एक प्रदाता चुनें",
      "clickToSelectProvider": "एक प्रदाता चुनने के लिए क्लिक करें",
      "selectProviderFromList": "शुरू करने के लिए सूची से एक प्रदाता चुनें।",
      "enterApiKey": "AI चैट कार्यक्षमता सक्षम करने के लिए अपनी API key दर्ज करें।"
    },
    "modelGuide": {
      "badge": "Model Guide",
      "title": "मैं एक मॉडल कैसे चुनूँ?",
      "intro": "LettuceAI एक ही \"सर्वोत्तम\" मॉडल नहीं थोपता। इसके बजाय, आप अपने <0>use case, बजट, और पसंद</0> के अनुसार चुनते हैं। क्या आज़माना है और कहाँ देखना है, इसके लिए यह गाइड उपयोग करें।",
      "askYourself": "खुद से पूछें:",
      "factors": {
        "quality": {
          "title": "गुणवत्ता और क्षमताएँ",
          "description": "मॉडल को कितना स्मार्ट होना चाहिए? बड़े, नए मॉडल आमतौर पर बेहतर तर्क करते हैं, अच्छा टेक्स्ट लिखते हैं, और जटिल prompts को बेहतर संभालते हैं।",
          "q1": "क्या आपको गहरी कैरेक्टर consistency और भावनात्मक बुद्धिमत्ता चाहिए?",
          "q2": "क्या आप immersive storytelling और विश्वसनीय कैरेक्टर व्यक्तित्व की परवाह करते हैं?",
          "q3": "क्या आप चाहते हैं कि मॉडल कैरेक्टर विवरण याद रखे और लंबे सत्रों में in-character रहे?"
        },
        "speed": {
          "title": "गति और latency",
          "description": "तेज़ मॉडल बातचीत के लिए बेहतर लगते हैं। कुछ मॉडल थोड़ी गुणवत्ता के बदले बहुत अधिक गति देते हैं।",
          "q1": "क्या आप roleplay को प्राकृतिक रूप से प्रवाहित रखने के लिए लगभग तत्काल उत्तर चाहते हैं?",
          "q2": "क्या आप तेज़ संवाद दृश्य कर रहे हैं जहाँ प्रतीक्षा immersion तोड़ देगी?",
          "q3": "क्या यह casual RP के लिए है जहाँ तेज़ आदान-प्रदान परफेक्ट उत्तर से अधिक मायने रखता है?"
        },
        "budget": {
          "title": "बजट और उपयोग",
          "description": "हर प्रदाता प्रति token बिल करता है। यदि आप बहुत चैट करते हैं तो सस्ते मॉडल भी जुड़ते हैं, इसलिए कुछ ऐसा चुनें जो आपके उपयोग के अनुसार हो।",
          "q1": "क्या आप समृद्ध कैरेक्टर इंटरेक्शन के लिए अधिक भुगतान करना ठीक हैं, या दैनिक RP के लिए सस्ता चाहते हैं?",
          "q2": "क्या आपके प्रदाता/router से निःशुल्क मॉडल हैं जिन्हें आप पहले आज़मा सकते हैं?",
          "q3": "क्या आप विस्तृत दृश्य विवरण के साथ लंबे roleplay सत्र चलाएँगे?",
          "q4": "क्या आपके पास एक कठोर मासिक बजट है जिसे आप पार नहीं करना चाहते?"
        },
        "safety": {
          "title": "सुरक्षा, गोपनीयता और extras",
          "description": "प्रदाता सुरक्षा, logging, और images, tools, या लंबे context windows जैसी अतिरिक्त सुविधाओं को कैसे संभालते हैं, इसमें भिन्न हैं।",
          "q1": "क्या आपको mature या creative roleplay परिदृश्यों के लिए कम content filters चाहिए?",
          "q2": "क्या आप परवाह करते हैं कि आपकी निजी RP बातचीत लॉग की जाती है या training के लिए उपयोग होती है?",
          "q3": "क्या आपको जटिल storylines और कैरेक्टर इतिहास के लिए लंबे context windows चाहिए?"
        }
      },
      "where": {
        "title": "मॉडल कहाँ मिलेंगे?",
        "intro": "अधिकांश प्रदाताओं और routers के पास एक <0>मॉडल सूची या कैटलॉग</0> होता है। वे पेज ब्राउज़ करें।",
        "directTitle": "Direct providers",
        "directDesc": "OpenAI, Anthropic, Google Gemini, xAI, Mistral, आदि। हर एक का एक console/dashboard है जहाँ आप आधिकारिक मॉडल नाम, क्षमताएँ और मूल्य निर्धारण देख सकते हैं।",
        "routersTitle": "Routers और hubs",
        "routersDesc": "OpenRouter या अन्य aggregators जैसी सेवाएँ एक स्थान पर विभिन्न प्रदाताओं के कई मॉडल सूचीबद्ध करती हैं।",
        "communityTitle": "Community recommendations",
        "communityDesc": "अपने प्रदाता/router के docs, blogs, या community posts देखें। वे आमतौर पर बताते हैं कि chat, coding, या speed के लिए कौन से मॉडल सर्वोत्तम हैं।"
      },
      "rules": {
        "title": "सरल नियम",
        "casual": "Casual chatting के लिए: अपने प्रदाता/router से एक तेज़, सस्ता chat मॉडल चुनें।",
        "experiments": "प्रयोगों या उच्च मात्रा के लिए: सबसे सस्ते मॉडल से शुरू करें जो पर्याप्त अच्छा लगे, फिर यदि आवश्यक हो तो upgrade करें।",
        "switch": "यदि कुछ गलत लगे (बहुत धीमा / बहुत कमज़ोर / बहुत महँगा): आप हमेशा LettuceAI में बाद में मॉडल बदल सकते हैं।"
      },
      "disclaimer": "नवीनतम मॉडल सूची, सीमाएँ और मूल्य निर्धारण के लिए हमेशा प्रदाता के अपने दस्तावेज़ देखें। यह पेज सोचने के बारे में है, खरीदने के बारे में नहीं।"
    },
    "whereToFind": {
      "badge": "API Key सहायता",
      "intro": "अपनी API key प्राप्त करने के लिए इन चरणों का पालन करें, फिर LettuceAI पर लौटें और इसे प्रदाता सेटिंग्स में पेस्ट करें।",
      "readyPrompt": "Key लेने के लिए तैयार हैं?",
      "openProviderSite": "प्रदाता साइट खोलें",
      "keyWarning": "अपनी API key कभी भी सार्वजनिक रूप से साझा न करें। इस key वाला कोई भी आपके account balance का उपयोग कर सकता है।",
      "stuckPrompt": "अभी भी समझ नहीं आया?",
      "joinDiscord": "सहायता के लिए हमारे Discord सर्वर में शामिल हों",
      "guides": {
        "chutes": {
          "title": "अपनी Chutes API key कैसे खोजें",
          "s1": "chutes.ai/app पर जाएँ और साइन इन करें।",
          "s2": "अपना account/settings क्षेत्र खोलें और API Keys ढूँढें।",
          "s3": "एक नई key बनाएँ (या मौजूदा कॉपी करें)।",
          "s4": "LettuceAI में key पेस्ट करें।"
        },
        "openai": {
          "title": "अपनी OpenAI API key कैसे खोजें",
          "s1": "platform.openai.com पर जाएँ और साइन इन करें।",
          "s2": "ऊपरी दाएँ अपना profile avatar क्लिक करें, फिर API keys चुनें।",
          "s3": "Create new secret key क्लिक करें और दिखाई गई value कॉपी करें।",
          "s4": "LettuceAI में key पेस्ट करें और इसे कहीं सुरक्षित संग्रहीत करें। आप इसे दोबारा नहीं देख पाएँगे।"
        },
        "anthropic": {
          "title": "अपनी Anthropic API key कैसे खोजें",
          "s1": "console.anthropic.com पर जाएँ और साइन इन करें।",
          "s2": "बाएँ sidebar से Settings खोलें।",
          "s3": "API keys चुनें और Create key क्लिक करें।",
          "s4": "Key कॉपी करें और LettuceAI में पेस्ट करें।"
        },
        "openrouter": {
          "title": "अपनी OpenRouter API key कैसे खोजें",
          "s1": "openrouter.ai पर जाएँ और लॉग इन करें।",
          "s2": "अपने profile मेनू से Keys पेज खोलें।",
          "s3": "Create key क्लिक करें, एक नाम दें, और सेव करें।",
          "s4": "Key कॉपी करें और LettuceAI में पेस्ट करें।"
        },
        "mistral": {
          "title": "अपनी Mistral API key कैसे खोजें",
          "s1": "console.mistral.ai पर जाएँ और साइन इन करें।",
          "s2": "Sidebar में API keys क्लिक करें।",
          "s3": "Create an API key क्लिक करें।",
          "s4": "Key कॉपी करें और LettuceAI में पेस्ट करें।"
        },
        "deepseek": {
          "title": "अपनी DeepSeek API key कैसे खोजें",
          "s1": "platform.deepseek.com खोलें और लॉग इन करें।",
          "s2": "ऊपरी navigation में API Keys क्लिक करें।",
          "s3": "यदि आपके पास नहीं है तो एक नई key बनाएँ।",
          "s4": "Key कॉपी करें और LettuceAI में पेस्ट करें।"
        },
        "groq": {
          "title": "अपनी Groq API key कैसे खोजें",
          "s1": "console.groq.com पर जाएँ और साइन इन करें।",
          "s2": "Sidebar से API Keys खोलें।",
          "s3": "एक नई key बनाएँ, फिर कॉपी करें।",
          "s4": "LettuceAI में key पेस्ट करें।"
        },
        "gemini": {
          "title": "अपनी Google Gemini API key कैसे खोजें",
          "s1": "aistudio.google.com पर Google AI Studio पर जाएँ और साइन इन करें।",
          "s2": "Get API key या Manage keys क्लिक करें।",
          "s3": "यदि आवश्यक हो तो एक नई key बनाएँ।",
          "s4": "Key कॉपी करें और LettuceAI में पेस्ट करें।"
        },
        "xai": {
          "title": "अपनी xAI API key कैसे खोजें",
          "s1": "console.x.ai खोलें और साइन इन करें।",
          "s2": "Console में API Keys अनुभाग पर जाएँ।",
          "s3": "एक नई key बनाएँ।",
          "s4": "Key कॉपी करें और LettuceAI में पेस्ट करें।"
        },
        "zai": {
          "title": "अपनी zAI (GLM) API key कैसे खोजें",
          "s1": "open.bigmodel.cn पर जाएँ और लॉग इन करें।",
          "s2": "User Center खोलें, फिर API Keys पर जाएँ।",
          "s3": "यदि आपके पास नहीं है तो एक नई key बनाएँ।",
          "s4": "Key कॉपी करें और LettuceAI में पेस्ट करें।"
        },
        "moonshot": {
          "title": "अपनी Moonshot (Kimi) API key कैसे खोजें",
          "s1": "platform.moonshot.cn पर जाएँ और साइन इन करें।",
          "s2": "Console में API Keys अनुभाग खोलें।",
          "s3": "एक नई key बनाएँ और कॉपी करें।",
          "s4": "LettuceAI में key पेस्ट करें।"
        },
        "qwen": {
          "title": "अपनी Qwen API key कैसे खोजें",
          "s1": "dashscope.aliyun.com खोलें और लॉग इन करें।",
          "s2": "Sidebar में API Keys अनुभाग पर जाएँ।",
          "s3": "एक नई key बनाएँ।",
          "s4": "Key कॉपी करें और LettuceAI में पेस्ट करें।"
        },
        "nanogpt": {
          "title": "अपनी NanoGPT API key कैसे खोजें",
          "s1": "nano-gpt.com पर जाएँ और लॉग इन करें।",
          "s2": "Dashboard खोलें और API keys अनुभाग पर जाएँ।",
          "s3": "यदि आवश्यक हो तो एक नई key बनाएँ।",
          "s4": "Key कॉपी करें और LettuceAI में पेस्ट करें।"
        },
        "featherless": {
          "title": "अपनी Featherless API key कैसे खोजें",
          "s1": "featherless.ai पर जाएँ और साइन इन करें।",
          "s2": "Dashboard से अपना account या API अनुभाग खोलें।",
          "s3": "यदि आपको कोई नहीं दिखे तो एक नई key बनाएँ।",
          "s4": "Key कॉपी करें और LettuceAI में पेस्ट करें।"
        },
        "anannas": {
          "title": "अपनी Anannas API key कैसे खोजें",
          "s1": "dashboard.anannas.ai पर जाएँ और लॉग इन करें।",
          "s2": "API Keys अनुभाग पर जाएँ।",
          "s3": "एक नई key बनाएँ और कॉपी करें।",
          "s4": "LettuceAI में key पेस्ट करें।"
        },
        "default": {
          "title": "अपनी API key कैसे खोजें",
          "s1": "ब्राउज़र में अपना AI प्रदाता dashboard खोलें और साइन इन करें।",
          "s2": "API, Developer, या Integrations settings देखें।",
          "s3": "एक नई API key बनाएँ या मौजूदा देखें।",
          "s4": "Key कॉपी करें और LettuceAI में पेस्ट करें।"
        }
      }
    },
    "welcomeFooter": {
      "setupTimeMobile": "सेटअप में 2 मिनट से कम लगता है"
    }
  },
  "search": {
    "placeholder": "खोजें...",
    "tabs": {
      "characters": "कैरेक्टर",
      "personas": "पर्सोना"
    },
    "noResults": "कोई {{type}} नहीं मिला",
    "emptyState": "अभी तक कोई {{type}} नहीं",
    "noResultsHint": "कोई भिन्न खोज शब्द आज़माएँ",
    "emptyCharacters": "चैट शुरू करने के लिए अपना पहला कैरेक्टर बनाएँ",
    "emptyPersonas": "सेटिंग्स में एक पर्सोना बनाएँ",
    "a11y": {
      "goBack": "वापस जाएँ",
      "clearSearch": "खोज साफ़ करें",
      "characterAvatar": "{{name}} अवतार"
    },
    "session": {
      "newChatTitle": "नई चैट"
    },
    "noDescription": "कोई विवरण नहीं",
    "defaultBadge": "डिफ़ॉल्ट"
  },
  "sync": {
    "modes": {
      "join": "शामिल हों",
      "joinDesc": "होस्ट से कनेक्ट करें",
      "host": "होस्ट",
      "hostDesc": "अपना डेटा साझा करें"
    },
    "sections": {
      "mode": "मोड",
      "connectToHost": "होस्ट से कनेक्ट करें",
      "startHosting": "होस्टिंग शुरू करें",
      "status": "स्थिति",
      "hosting": "होस्टिंग सेवा",
      "localAddress": "स्थानीय नेटवर्क पता",
      "connectionPin": "कनेक्शन PIN",
      "setupGuide": "सेटअप गाइड"
    },
    "fields": {
      "hostAddress": "होस्ट पता या JSON",
      "hostPlaceholder": "उदा. 192.168.1.100:12345",
      "pinCode": "PIN कोड",
      "pinPlaceholder": "000000"
    },
    "buttons": {
      "scanQRCode": "QR कोड स्कैन करें",
      "connect": "कनेक्ट करें",
      "connecting": "कनेक्ट हो रहा है...",
      "startHosting": "होस्टिंग शुरू करें",
      "startingServer": "सर्वर शुरू हो रहा है...",
      "stopHosting": "होस्टिंग रोकें",
      "hostAgain": "फिर से होस्ट करें",
      "done": "पूर्ण"
    },
    "status": {
      "connecting": "कनेक्ट हो रहा है...",
      "connected": "कनेक्टेड",
      "waitingConfirmation": "पुष्टि के लिए प्रतीक्षा कर रहा है",
      "waitingConfirmationDesc": "जारी रखने के लिए होस्ट डिवाइस पर कनेक्शन को मंजूरी दें।",
      "syncing": "सिंक हो रहा है...",
      "transferringData": "डेटा स्थानांतरित हो रहा है",
      "syncInProgress": "सिंक प्रगति में",
      "live": "लाइव",
      "broadcasting": "प्रसारण",
      "clientsLabel": "कनेक्टेड",
      "clientsUnit": "क्लाइंट"
    },
    "pinDescription": "कनेक्ट होने वाले डिवाइस के साथ यह PIN साझा करें",
    "hostingDesc1": "अन्य डिवाइस इस डिवाइस से कनेक्ट और डेटा सिंक कर सकते हैं।",
    "hostingDesc2": "आपका डेटा कनेक्टेड क्लाइंट्स के साथ साझा किया जाएगा।",
    "setupSteps": {
      "step1": "दूसरे डिवाइस पर ऐप खोलें",
      "step2": "सेटिंग्स → लोकल सिंक पर जाएँ",
      "step3": "QR कोड स्कैन करें या पता दर्ज करें"
    },
    "messages": {
      "completed": "सिंक पूर्ण!",
      "completedDesc": "सभी डेटा सिंक हो गया",
      "error": "कनेक्शन त्रुटि",
      "outdatedClient": "पुराना क्लाइंट पाया गया"
    },
    "disclaimer": "सिंक आपके स्थानीय नेटवर्क पर काम करता है। दोनों डिवाइस एक ही WiFi पर होने चाहिए।",
    "modals": {
      "connectionRequest": "कनेक्शन अनुरोध",
      "requestMessage": "इस डिवाइस के साथ सिंक करना चाहता है।",
      "acceptConnection": "कनेक्शन स्वीकार करें",
      "acceptDesc": "इस डिवाइस को डेटा सिंक करने दें",
      "decline": "अस्वीकार करें",
      "declineDesc": "इस कनेक्शन प्रयास को ब्लॉक करें",
      "readyToSync": "सिंक के लिए तैयार",
      "connectionEstablished": "कनेक्शन स्थापित",
      "deviceReady": "तैयार है।",
      "startSyncMessage": "डेटा सिंक करना शुरू करने के लिए नीचे टैप करें।",
      "startSyncing": "सिंक शुरू करें",
      "startSyncingDesc": "अभी डेटा स्थानांतरण शुरू करें"
    },
    "scanner": {
      "title": "QR कोड स्कैन करें",
      "cancel": "स्कैन रद्द करें"
    },
    "unknownDevice": "अज्ञात डिवाइस",
    "aria": {
      "dismissStatus": "सिंक स्थिति खारिज करें",
      "dismissError": "सिंक त्रुटि खारिज करें"
    },
    "stats": {
      "statusLabel": "स्थिति"
    }
  },
  "creationHelper": {
    "page": {
      "info": "क्रिएशन हेल्पर AI सहायता के साथ कैरेक्टर बनाने में आपका मार्गदर्शन करता है। कैरेक्टर निर्माण के दौरान उपयोग किए जाने वाले मॉडल और उपकरण कॉन्फ़िगर करें।",
      "modelConfiguration": "मॉडल कॉन्फ़िगरेशन",
      "chatModel": "चैट मॉडल",
      "selectedModel": "चयनित मॉडल",
      "useAppDefault": "ऐप डिफ़ॉल्ट का उपयोग करें{{model}}",
      "useAppDefaultBase": "ऐप डिफ़ॉल्ट का उपयोग करें",
      "noModelsAvailable": "कोई मॉडल उपलब्ध नहीं",
      "chatModelDescription": "कैरेक्टर निर्माण बातचीत के लिए AI मॉडल",
      "streamingOutput": "स्ट्रीमिंग आउटपुट",
      "streamingDescription": "उत्तर जनरेट होते ही दिखाएँ",
      "imageGenerationModel": "छवि जनरेशन मॉडल",
      "noModelSelected": "कोई मॉडल चयनित नहीं",
      "noImageModelsAvailable": "कोई छवि मॉडल उपलब्ध नहीं",
      "imageModelDescription": "कैरेक्टर अवतार जनरेट करने के लिए",
      "toolSelection": "उपकरण चयन",
      "smartToolSelection": "स्मार्ट उपकरण चयन",
      "smartToolDescription": "AI स्वचालित रूप से चुनता है कि कौन से उपकरण उपयोग करने हैं",
      "smartToolEnabledHint": "सक्षम होने पर, AI क्रिएटर हेल्पर पूछता है कि आप क्या बनाना चाहते हैं और केवल प्रासंगिक उपकरण सेट लोड करता है।",
      "smartToolDisabledHint": "अक्षम होने पर, AI क्रिएटर हेल्पर सीधे खुलता है और सभी सक्षम उपकरणों का उपयोग करता है; सहायक तय करता है कि क्या बनाना है।",
      "quickPresets": "त्वरित प्रीसेट",
      "customSelection": "कस्टम चयन - {{count}} उपकरण सक्षम",
      "footerInfo": "जब स्मार्ट उपकरण चयन सक्षम है, AI संदर्भ के आधार पर तय करता है कि कौन से उपकरण उपयोग करने हैं। उपलब्ध उपकरणों को मैन्युअल रूप से नियंत्रित करने के लिए इसे अक्षम करें।",
      "selectChatModel": "चैट मॉडल चुनें",
      "selectImageModel": "छवि मॉडल चुनें",
      "searchModels": "मॉडल खोजें..."
    },
    "categories": {
      "basic": "बुनियादी",
      "content": "सामग्री",
      "visual": "दृश्य",
      "settings": "सेटिंग्स",
      "flow": "प्रवाह",
      "persona": "पर्सोना",
      "lorebook": "लोरबुक्स"
    },
    "presets": {
      "all": {
        "name": "सभी उपकरण",
        "desc": "सभी उपलब्ध उपकरण सक्षम करें"
      },
      "essential": {
        "name": "आवश्यक",
        "desc": "केवल नाम, परिभाषा और दृश्य"
      },
      "minimal": {
        "name": "न्यूनतम",
        "desc": "केवल नाम और परिभाषा"
      }
    },
    "tools": {
      "setName": "नाम सेट करें",
      "setNameDesc": "कैरेक्टर का नाम सेट करें",
      "setDefinition": "परिभाषा सेट करें",
      "setDefinitionDesc": "व्यक्तित्व और पृष्ठभूमि सेट करें",
      "set_character_name": {
        "name": "नाम सेट करें",
        "desc": "कैरेक्टर का नाम सेट करें"
      },
      "set_character_definition": {
        "name": "परिभाषा सेट करें",
        "desc": "व्यक्तित्व और पृष्ठभूमि सेट करें"
      },
      "add_scene": {
        "name": "दृश्य जोड़ें",
        "desc": "रोलप्ले के लिए शुरुआती दृश्य जोड़ें"
      },
      "update_scene": {
        "name": "दृश्य अपडेट करें",
        "desc": "मौजूदा दृश्य संशोधित करें"
      },
      "toggle_avatar_gradient": {
        "name": "अवतार ग्रेडिएंट",
        "desc": "अवतार पर ग्रेडिएंट ओवरले टॉगल करें"
      },
      "set_default_model": {
        "name": "मॉडल सेट करें",
        "desc": "बातचीत के लिए AI मॉडल सेट करें"
      },
      "set_system_prompt": {
        "name": "सिस्टम प्रॉम्प्ट",
        "desc": "व्यवहार दिशानिर्देश सेट करें"
      },
      "get_system_prompt_list": {
        "name": "प्रॉम्प्ट सूची",
        "desc": "उपलब्ध प्रॉम्प्ट देखें"
      },
      "get_model_list": {
        "name": "मॉडल सूची",
        "desc": "उपलब्ध मॉडल देखें"
      },
      "use_uploaded_image_as_avatar": {
        "name": "छवि अवतार के रूप में",
        "desc": "अपलोड की गई छवि को अवतार के रूप में उपयोग करें"
      },
      "use_uploaded_image_as_chat_background": {
        "name": "छवि पृष्ठभूमि के रूप में",
        "desc": "अपलोड की गई छवि को पृष्ठभूमि के रूप में उपयोग करें"
      },
      "generate_image": {
        "name": "छवि जनरेट करें",
        "desc": "AI मॉडल से छवि जनरेट करें"
      },
      "show_preview": {
        "name": "पूर्वावलोकन दिखाएँ",
        "desc": "कैरेक्टर का पूर्वावलोकन देखें"
      },
      "request_confirmation": {
        "name": "पुष्टि अनुरोध",
        "desc": "सेव या जारी रखने के लिए पूछें"
      },
      "list_personas": {
        "name": "पर्सोना सूची",
        "desc": "पर्सोना ब्राउज़ करें"
      },
      "upsert_persona": {
        "name": "पर्सोना सेव करें",
        "desc": "पर्सोना बनाएँ या अपडेट करें"
      },
      "use_uploaded_image_as_persona_avatar": {
        "name": "पर्सोना अवतार",
        "desc": "अपलोड की गई छवि को पर्सोना अवतार के रूप में उपयोग करें"
      },
      "delete_persona": {
        "name": "पर्सोना हटाएँ",
        "desc": "एक पर्सोना हटाएँ"
      },
      "get_default_persona": {
        "name": "डिफ़ॉल्ट पर्सोना",
        "desc": "डिफ़ॉल्ट पर्सोना लाएँ"
      },
      "list_lorebooks": {
        "name": "लोरबुक सूची",
        "desc": "लोरबुक ब्राउज़ करें"
      },
      "upsert_lorebook": {
        "name": "लोरबुक सेव करें",
        "desc": "लोरबुक बनाएँ या अपडेट करें"
      },
      "delete_lorebook": {
        "name": "लोरबुक हटाएँ",
        "desc": "एक लोरबुक हटाएँ"
      },
      "list_lorebook_entries": {
        "name": "प्रविष्टि सूची",
        "desc": "लोरबुक प्रविष्टियाँ देखें"
      },
      "get_lorebook_entry": {
        "name": "प्रविष्टि लाएँ",
        "desc": "एक लोरबुक प्रविष्टि लाएँ"
      },
      "upsert_lorebook_entry": {
        "name": "प्रविष्टि सेव करें",
        "desc": "प्रविष्टि बनाएँ या अपडेट करें"
      },
      "delete_lorebook_entry": {
        "name": "प्रविष्टि हटाएँ",
        "desc": "एक लोरबुक प्रविष्टि हटाएँ"
      },
      "create_blank_lorebook_entry": {
        "name": "खाली प्रविष्टि",
        "desc": "एक प्लेसहोल्डर प्रविष्टि बनाएँ"
      },
      "reorder_lorebook_entries": {
        "name": "प्रविष्टियाँ पुनर्व्यवस्थित करें",
        "desc": "प्रविष्टि क्रम बदलें"
      },
      "list_character_lorebooks": {
        "name": "कैरेक्टर लोरबुक सूची",
        "desc": "कैरेक्टर की लोरबुक देखें"
      },
      "set_character_lorebooks": {
        "name": "कैरेक्टर लोरबुक सेट करें",
        "desc": "कैरेक्टर को लोरबुक असाइन करें"
      },
      "addScene": "दृश्य जोड़ें",
      "addSceneDesc": "रोलप्ले के लिए शुरुआती दृश्य जोड़ें",
      "updateScene": "दृश्य अपडेट करें",
      "updateSceneDesc": "मौजूदा दृश्य संशोधित करें",
      "avatarGradient": "अवतार ग्रेडिएंट",
      "avatarGradientDesc": "अवतार पर ग्रेडिएंट ओवरले टॉगल करें",
      "setModel": "मॉडल सेट करें",
      "setModelDesc": "बातचीत के लिए AI मॉडल सेट करें",
      "systemPrompt": "सिस्टम प्रॉम्प्ट",
      "systemPromptDesc": "व्यवहार दिशानिर्देश सेट करें",
      "listPrompts": "प्रॉम्प्ट सूची",
      "listPromptsDesc": "उपलब्ध प्रॉम्प्ट देखें",
      "listModels": "मॉडल सूची",
      "listModelsDesc": "उपलब्ध मॉडल देखें",
      "imageAsAvatar": "छवि अवतार के रूप में",
      "imageAsAvatarDesc": "अपलोड की गई छवि को अवतार के रूप में उपयोग करें"
    }
  },
  "tour": {
    "stepCounter": "चरण {{current}} / {{total}}",
    "skipTour": "टूर छोड़ें",
    "next": "अगला",
    "gotIt": "समझ गया",
    "appShell": {
      "chats": {
        "title": "यहाँ आपकी चैट रहती हैं",
        "body": "कैरेक्टर के साथ आपकी सभी एक-एक बातचीत यहाँ हैं। कभी भी वापस आएँ और हम आपकी जगह याद रखेंगे।"
      },
      "groups": {
        "title": "ग्रुप चैट में शामिल हों",
        "body": "कई कैरेक्टर को एक ही कमरे में लाएँ और उन्हें एक-दूसरे से बात करते देखें, या जब चाहें खुद भी शामिल हों।"
      },
      "discover": {
        "title": "नए कैरेक्टर खोजें",
        "body": "कम्यूनिटी ने जो शेयर किया है उसे ब्राउज़ करें और कोई भी कैरेक्टर जो पसंद आए, ले आएँ। नए पसंदीदा बस एक टैप दूर हैं।"
      },
      "library": {
        "title": "आपकी निजी लाइब्रेरी",
        "body": "आपने जो भी बनाया या सेव किया है वो यहाँ है: कैरेक्टर, पर्सोना, प्रॉम्प्ट, सब कुछ। इसे अपना स्टैश समझें।"
      },
      "settings": {
        "title": "इसे अपना बनाएँ",
        "body": "प्रदाता बदलें, अलग मॉडल चुनें, ऐप का लुक ट्वीक करें। लगभग सब कुछ सेटिंग्स से एडजस्ट हो सकता है।"
      },
      "search": {
        "title": "कुछ भी तेज़ी से खोजें",
        "body": "किसी खास चैट या कैरेक्टर की तलाश है? यहाँ से सब कुछ खोजें। कोई खोजबीन नहीं।"
      },
      "create": {
        "title": "और आखिर में, बनाएँ!",
        "body": "जब भी प्रेरणा मिले प्लस पर टैप करें। नया कैरेक्टर, पर्सोना बनाएँ या शुरू से कुछ शुरू करें।"
      }
    },
    "chatDetail": {
      "chatTitle": {
        "title": "प्रति-चैट सेटिंग्स",
        "body": "कैरेक्टर का नाम ऊपर टैप करें इस चैट की सेटिंग्स खोलने के लिए। हर बातचीत के लिए अलग पर्सोना, लेआउट और मॉडल।"
      },
      "chatMemory": {
        "title": "वो क्या याद रखते हैं",
        "body": "ब्रेन आइकन दिखाता है कि आपका कैरेक्टर आपकी बातचीत से क्या याद रखता है। समीक्षा, संपादन या यादें साफ़ करने के लिए टैप करें।"
      },
      "chatSearch": {
        "title": "वो एक लाइन खोजें",
        "body": "सिर्फ इस बातचीत में खोजें। 200 मैसेज पहले की कोई बात ढूँढने के लिए बढ़िया, बिना अंतहीन स्क्रॉलिंग के।"
      },
      "chatLorebook": {
        "title": "लोरबुक एंट्री",
        "body": "अतिरिक्त तथ्य, वर्ल्ड-बिल्डिंग और संदर्भ जो विशिष्ट कीवर्ड आने पर प्रॉम्प्ट में इंजेक्ट होते हैं। आपके कैरेक्टर का चीट शीट।"
      },
      "chatPlus": {
        "title": "चीज़ें अटैच करें",
        "body": "इमेज डालें या एक्स्ट्रा मेनू खोलें। जो भी अटैच करेंगे वो आपके अगले मैसेज के साथ भेजा जाएगा।"
      },
      "chatComposer": {
        "title": "आपका मैसेज, आपकी चाल",
        "body": "यहाँ टाइप करें। Enter भेजता है, Shift+Enter नई लाइन। सुझाव: किसी भी मैसेज पर लॉन्ग-प्रेस करें एडिट, ब्रांच या डिलीट के लिए।"
      },
      "chatSend": {
        "title": "एक बटन, चार काम",
        "body": "सेंड बटन स्थिति के अनुसार अपना काम बदलता है:"
      }
    },
    "postFirstMessage": {
      "chatRegenerate": {
        "title": "पसंद नहीं आया? रीजनरेट करें",
        "body": "कैरेक्टर से बिल्कुल नया जवाब पाने के लिए रिफ्रेश आइकन टैप करें। हर रीजनरेशन एक वैरिएंट के रूप में सेव होता है।"
      },
      "chatVariants": {
        "title": "वैरिएंट के बीच स्वाइप करें",
        "body": "रीजनरेट करने के बाद, मैसेज के नीचे वैरिएंट काउंटर दिखेगा। सभी अलग-अलग जवाब देखने के लिए मैसेज बबल पर बाएँ या दाएँ स्वाइप करें।"
      },
      "chatLongPress": {
        "title": "यहाँ और भी छिपा है",
        "body": "किसी भी मैसेज पर लॉन्ग-प्रेस करें एडिट, कॉपी, ब्रांच, पिन, डिलीट या रिवाइंड के लिए। डेस्कटॉप पर राइट-क्लिक भी काम करता है।"
      }
    },
    "sendButton": {
      "continue": {
        "label": "जारी रखें",
        "desc": "इनपुट खाली है। टैप करने पर कैरेक्टर बोलता रहेगा।"
      },
      "send": {
        "label": "भेजें",
        "desc": "आपने कुछ टाइप या अटैच किया है। भेजने के लिए टैप करें।"
      },
      "sending": {
        "label": "भेजा जा रहा है",
        "desc": "जवाब आ रहा है। बटन लॉक है।"
      },
      "stop": {
        "label": "रोकें",
        "desc": "जवाब के बीच में रद्द करने के लिए टैप करें।"
      }
    },
    "extra": {
      "rerunOnboarding": "ऑनबोर्डिंग फिर से चलाएँ"
    }
  },
  "sessionAdvanced": {
    "title": "सत्र पैरामीटर",
    "subtitle": "इस बातचीत के लिए मॉडल डिफ़ॉल्ट ओवरराइड करें",
    "goBack": "वापस जाएँ",
    "support": "सहायता",
    "reset": "रीसेट",
    "save": "सहेजें",
    "noSessionWarning": "प्रति-सत्र सेटिंग्स कॉन्फ़िगर करने के लिए एक चैट सत्र खोलें।",
    "overrideDefaults": "डिफ़ॉल्ट ओवरराइड करें",
    "overrideDefaultsDesc": "सिर्फ इस बातचीत के लिए पैरामीटर कस्टमाइज़ करें",
    "loadingContextInfo": "कॉन्टेक्स्ट जानकारी लोड हो रही है...",
    "sampling": {
      "title": "सैम्पलिंग",
      "temperature": "Temperature",
      "temperatureDesc": "रैंडमनेस नियंत्रित करता है। कम = अधिक निर्धारित, अधिक = अधिक रचनात्मक।",
      "temperaturePrecise": "सटीक",
      "temperatureCreative": "रचनात्मक",
      "topP": "Top P",
      "topPDesc": "Nucleus sampling। टोकन को संचयी संभावना तक सीमित करता है।",
      "topPFocused": "केंद्रित",
      "topPDiverse": "विविध",
      "topK": "Top K",
      "topKDesc": "सैम्पलिंग को शीर्ष K सबसे संभावित टोकन तक सीमित करता है।"
    },
    "outputPenalties": {
      "title": "आउटपुट और पेनल्टी",
      "maxOutputTokens": "अधिकतम आउटपुट टोकन",
      "maxOutputTokensDesc": "अधिकतम जवाब लंबाई। Auto मॉडल को तय करने देता है।",
      "auto": "Auto",
      "custom": "कस्टम",
      "frequencyPenalty": "फ्रीक्वेंसी पेनल्टी",
      "frequencyPenaltyDesc": "टोकन अनुक्रमों की पुनरावृत्ति कम करता है।",
      "frequencyPenaltyRepeat": "दोहराव",
      "frequencyPenaltyVary": "विविधता",
      "presencePenalty": "प्रेज़ेंस पेनल्टी",
      "presencePenaltyDesc": "नए विषयों की खोज प्रोत्साहित करता है।",
      "presencePenaltyRepeat": "दोहराव",
      "presencePenaltyExplore": "अन्वेषण"
    },
    "performance": {
      "title": "प्रदर्शन",
      "gpuLayers": "GPU Layers",
      "gpuLayersDesc": "GPU पर ऑफलोड की गई लेयर। 0 = केवल CPU।",
      "threads": "Threads",
      "threadsDesc": "इन्फरेंस के लिए CPU threads।",
      "batchThreads": "Batch Threads",
      "batchThreadsDesc": "बैच प्रोसेसिंग के लिए CPU threads।",
      "batchSize": "Batch Size",
      "batchSizeDesc": "प्रॉम्प्ट प्रोसेसिंग चंक साइज़।",
      "contextLength": "कॉन्टेक्स्ट लंबाई",
      "contextLengthDesc": "कॉन्टेक्स्ट विंडो साइज़ ओवरराइड करें।",
      "flashAttention": "Flash Attention",
      "flashAttentionDesc": "GPU मेमोरी ऑप्टिमाइज़ेशन।",
      "enabled": "सक्रिय",
      "disabled": "निष्क्रिय"
    },
    "samplingMemory": {
      "title": "सैम्पलिंग और मेमोरी",
      "minP": "Min P",
      "minPDesc": "न्यूनतम संभावना सीमा।",
      "typicalP": "Typical P",
      "typicalPDesc": "टिपिकल सैम्पलिंग सीमा।",
      "seed": "Seed",
      "seedDesc": "रैंडम seed। रैंडम के लिए खाली छोड़ें।",
      "ropeBase": "RoPE Base",
      "ropeBaseDesc": "फ्रीक्वेंसी बेस ओवरराइड।",
      "ropeScale": "RoPE Scale",
      "ropeScaleDesc": "फ्रीक्वेंसी स्केल ओवरराइड।",
      "kvCacheType": "KV Cache Type",
      "kvCacheTypeDesc": "VRAM बचाने के लिए KV cache क्वांटाइज़ करें।",
      "offloadKqv": "Offload KQV",
      "offloadKqvDesc": "KV cache और KQV ऑप्स GPU पर।",
      "on": "चालू",
      "off": "बंद",
      "samplerProfile": "सैम्पलर प्रोफ़ाइल",
      "samplerProfileDesc": "स्थिरता या तर्क के लिए ट्यून किए गए डिफ़ॉल्ट।",
      "balanced": "संतुलित",
      "creative": "रचनात्मक",
      "stable": "स्थिर",
      "reasoning": "तर्क"
    },
    "systemInfo": {
      "title": "सिस्टम जानकारी",
      "maxContext": "अधिकतम कॉन्टेक्स्ट",
      "recommended": "अनुशंसित",
      "availableRam": "उपलब्ध RAM",
      "availableVram": "उपलब्ध VRAM",
      "modelSize": "मॉडल साइज़"
    }
  },
  "exportMenu": {
    "title": "एक्सपोर्ट फ़ॉर्मेट",
    "selectFormat": "फ़ॉर्मेट चुनें",
    "uscTitle": "Unified System Card",
    "formats": {
      "uscPrompt": "प्रॉम्प्ट टेम्पलेट के लिए पोर्टेबल USC एक्सपोर्ट।",
      "uscLorebook": "लोरबुक के लिए पोर्टेबल USC एक्सपोर्ट।",
      "uscModel": "मॉडल प्रोफ़ाइल के लिए पोर्टेबल USC एक्सपोर्ट।",
      "uscChatTemplate": "चैट टेम्पलेट के लिए पोर्टेबल USC एक्सपोर्ट।",
      "legacyPromptJson": "Legacy Prompt JSON",
      "legacyPromptJsonDesc": "वर्तमान बाहरी प्रॉम्प्ट पैक फ़ॉर्मेट।",
      "lorebookJson": "Lorebook JSON",
      "lorebookJsonDesc": "वर्तमान लोरबुक एक्सपोर्ट फ़ॉर्मेट।",
      "modelJson": "Model JSON",
      "modelJsonDesc": "क्रेडेंशियल के बिना सुरक्षित मॉडल प्रोफ़ाइल JSON।",
      "chatTemplateJson": "Chat Template JSON",
      "chatTemplateJsonDesc": "नेटिव चैट टेम्पलेट एक्सपोर्ट फ़ॉर्मेट।"
    },
    "extra": {
      "selectFormat": "एक फ़ॉर्मेट चुनें",
      "exportFormatTitle": "एक्सपोर्ट फ़ॉर्मेट",
      "uscDesc": "पोर्टेबल USC एक्सपोर्ट",
      "legacyJsonDesc": "पुराना JSON एक्सपोर्ट फ़ॉर्मेट",
      "formatV3Desc": "कैरेक्टर कार्ड V3 एक्सपोर्ट",
      "formatV2Desc": "कैरेक्टर कार्ड V2 एक्सपोर्ट",
      "formatV1Desc": "कैरेक्टर कार्ड V1 एक्सपोर्ट",
      "uscLorebook": "Unified System Card (USC)",
      "portableLorebook": "लोरबुक के लिए पोर्टेबल USC एक्सपोर्ट",
      "lorebookJson": "Lorebook JSON",
      "currentLorebook": "वर्तमान लोरबुक एक्सपोर्ट फ़ॉर्मेट",
      "uscModel": "Unified System Card (USC)",
      "portableModel": "मॉडल प्रोफ़ाइल के लिए पोर्टेबल USC एक्सपोर्ट",
      "modelJson": "Model JSON",
      "safeModel": "क्रेडेंशियल के बिना सुरक्षित मॉडल प्रोफ़ाइल JSON",
      "uscTemplate": "Unified System Card (USC)",
      "portableTemplate": "चैट टेम्पलेट के लिए पोर्टेबल USC एक्सपोर्ट",
      "templateJson": "Chat Template JSON",
      "nativeTemplate": "नेटिव चैट टेम्पलेट एक्सपोर्ट फ़ॉर्मेट"
    }
  },
  "designReference": {
    "title": "डिज़ाइन संदर्भ",
    "description": "कुछ स्पष्ट संदर्भ इमेज और एक कैनोनिकल विज़ुअल विवरण अपलोड करें।",
    "descriptionPlaceholder": "स्थिर लुक का वर्णन करें: चेहरा, बाल, शरीर, उम्र प्रस्तुति, पोशाक संकेत, एक्सेसरीज़ और आर्ट/स्टाइल दिशा।",
    "addReferences": "संदर्भ जोड़ें",
    "visualDescription": "विज़ुअल विवरण",
    "draftWithAi": "AI से ड्राफ्ट करें",
    "referenceImages": "संदर्भ इमेज",
    "imageAlt": "डिज़ाइन संदर्भ {{index}}",
    "loading": "लोड हो रहा है...",
    "removeAria": "डिज़ाइन संदर्भ हटाएँ",
    "noImages": "अभी तक कोई संदर्भ इमेज नहीं",
    "imageCount": "{{count}} संदर्भ इमेज अटैच",
    "emptyReferences": "चेहरा, अनुपात, पोशाक और स्टाइल लॉक करने के लिए कुछ स्पष्ट संदर्भ शॉट जोड़ें।",
    "noWriterModel": "पहले इमेज जनरेशन सेटिंग्स में एक संगत सीन राइटर मॉडल जोड़ें।",
    "noImagesForGeneration": "जनरेट करने से पहले एक अवतार या कम से कम एक संदर्भ इमेज जोड़ें।",
    "writerModelHelp": "आपके अवतार और संदर्भ इमेज से ड्राफ्ट करने के लिए {{model}} का उपयोग करता है।",
    "noWriterModelHelp": "इसे स्वचालित रूप से ड्राफ्ट करने के लिए इमेज जनरेशन सेटिंग्स में एक संगत सीन राइटर मॉडल जोड़ें।",
    "draftMenuTitle": "AI डिज़ाइन ड्राफ्ट",
    "draftMenuDesc": "वर्तमान अवतार और संदर्भ इमेज से {{model}} द्वारा ड्राफ्ट किया गया।",
    "draftMenuNoWriter": "इस सहायक का उपयोग करने से पहले एक संगत सीन राइटर मॉडल जोड़ें।",
    "regenerate": "पुन: जनरेट करें",
    "useThis": "यह उपयोग करें"
  },
  "samplerOrder": {
    "title": "सैम्पलर क्रम",
    "description": "चरणों को पुनर्व्यवस्थित करने के लिए खींचें। ऊपर से नीचे निष्पादित होता है।",
    "reset": "रीसेट",
    "resetAria": "सैम्पलर क्रम को डिफ़ॉल्ट पर रीसेट करें",
    "stages": {
      "penalties": {
        "label": "पेनल्टी",
        "desc": "फ़िल्टरिंग से पहले फ्रीक्वेंसी और प्रेज़ेंस पेनल्टी लागू करें।"
      },
      "grammar": {
        "label": "व्याकरण",
        "desc": "नेटिव चैट टेम्पलेट व्याकरण सक्रिय होने पर टोकन सीमित करें।"
      },
      "topK": {
        "label": "Top K",
        "desc": "उम्मीदवार पूल को सबसे मजबूत टोकन तक काटें।"
      },
      "topP": {
        "label": "Top P",
        "desc": "शेष वितरण पर न्यूक्लियस फ़िल्टरिंग लागू करें।"
      },
      "minP": {
        "label": "Min P",
        "desc": "न्यूनतम सीमा का उपयोग करके कम-संभावना वाले टेल टोकन हटाएँ।"
      },
      "typical": {
        "label": "Typical P",
        "desc": "वर्तमान वितरण में सांख्यिकीय रूप से विशिष्ट टोकन पसंद करें।"
      },
      "temp": {
        "label": "Temperature",
        "desc": "चयन से पहले अंतिम वितरण को समतल या तेज़ करें।"
      }
    },
    "presets": {
      "default": {
        "label": "डिफ़ॉल्ट",
        "hint": "Lettuce local"
      },
      "unsloth": {
        "label": "Unsloth",
        "hint": "llama.cpp style"
      },
      "focused": {
        "label": "केंद्रित",
        "hint": "कड़ी छँटाई"
      },
      "creative": {
        "label": "रचनात्मक",
        "hint": "देर से फ़िल्टर"
      }
    }
  },
  "lorebookGen": {
    "flow": {
      "pickerCharactersTitle": "कैरेक्टर",
      "pickerSessionsTitle": "सत्र",
      "noCharacters": "कोई कैरेक्टर नहीं",
      "noSessions": "कोई सत्र नहीं",
      "clearSelection": "चयन साफ़ करें",
      "directionTitle": "वैकल्पिक जनरेशन दिशा",
      "directionLabel": "दिशा",
      "toggleForceMode": "फ़ोर्स मोड टॉगल करें",
      "entryTitlePlaceholder": "प्रविष्टि शीर्षक",
      "entryContentPlaceholder": "लोरबुक प्रविष्टि सामग्री",
      "editDirectionBeforeRegenerate": "पुनर्जनन से पहले दिशा संपादित करें",
      "generatorReturnedNoDraft": "जनरेटर ने कोई ड्राफ्ट नहीं दिया",
      "pageTitle": "लोरबुक प्रविष्टि जनरेट करें",
      "missingContext": "जनरेटर पेज के लिए लोरबुक संदर्भ गायब है।",
      "characterLocked": "कैरेक्टर इस लोरबुक के स्वामी से लॉक है",
      "chooseSession": "सत्र चुनें",
      "pickCharacter": "कैरेक्टर चुनें",
      "searchMemories": "यादें खोजें",
      "searchMessages": "संदेश खोजें",
      "selectLastN": "अंतिम {{n}} संदेश चुनें",
      "selectAll": "सब चुनें",
      "loadSessionPrompt": "लोड करने के लिए एक सत्र चुनें",
      "messagesText": "संदेश",
      "memoriesText": "यादें",
      "messagesAndMemories": "संदेश और यादें",
      "titleAndContentRequired": "शीर्षक और सामग्री आवश्यक हैं",
      "keywordsOrAlwaysActive": "कम से कम एक कीवर्ड जोड़ें या Always active सक्षम करें",
      "lorybookEntrySaved": "लोरबुक प्रविष्टि सेव हो गई",
      "saveFailed": "सेव करना विफल",
      "generationFailed": "जनरेशन विफल",
      "failedToLoadContext": "लोरबुक जनरेटर लोड करने में विफल",
      "failedToLoadSessions": "सत्र लोड करने में विफल",
      "failedToLoadMessages": "संदेश लोड करने में विफल",
      "userRole": "उपयोगकर्ता",
      "aiRole": "AI"
    },
    "triggerPreview": {
      "resizeInspector": "निरीक्षक का आकार बदलें"
    }
  },
  "companion": {
    "download": {
      "emotionClassifier": "भावना वर्गीकरणकर्ता",
      "emotionClassifierDesc": "बारियाँ पढ़ता है और companion की महसूस की गई, व्यक्त और अवरुद्ध भावना vectors अपडेट करता है।",
      "emotionSize": "~120 MB",
      "entityExtractor": "एंटिटी एक्सट्रैक्टर (NER)",
      "entityExtractorDesc": "लोगों, स्थानों और वस्तुओं की पहचान करता है ताकि यादों को canonicalize और link किया जा सके।",
      "entitySize": "~140 MB",
      "memoryRouter": "मेमोरी राउटर",
      "memoryRouterDesc": "तय करता है कि नई बारियों को relationship, milestone, episodic या अन्य memory categories में संग्रहीत करना चाहिए।",
      "routerSize": "~70 MB",
      "unknownModel": "अज्ञात companion मॉडल। ?kind=emotion|ner|router प्रदान करें।"
    }
  },
  "voices": {
    "extra": {
      "customVoices": "मेरी आवाज़ें",
      "providerVoices": "प्रदाता आवाज़ें",
      "myVoices": "मेरी आवाज़ें",
      "page": {
        "noAudioProvidersHint": "शुरू करने के लिए प्रदाता > ऑडियो में एक जोड़ें",
        "noVoicesTitle": "अभी तक कोई आवाज़ नहीं बनाई गई",
        "noVoicesDescription": "अपने कैरेक्टर के लिए कस्टम प्रॉम्प्ट के साथ आवाज़ें बनाएँ",
        "addProviderFirst": "पहले एक ऑडियो प्रदाता जोड़ें",
        "noPrompt": "कोई प्रॉम्प्ट नहीं",
        "noProviderVoices": "कोई आवाज़ नहीं मिली। आवाज़ें लाने के लिए Refresh करें।",
        "showLess": "कम दिखाएँ",
        "showAllVoices": "सभी {{count}} आवाज़ें दिखाएँ",
        "voiceFallbackTitle": "आवाज़"
      },
      "cache": {
        "section": "ऑडियो कैश",
        "title": "TTS ऑडियो कैश",
        "description": "बार-बार बनाने से बचने के लिए जनरेट की गई आवाज़ ऑडियो कैश की जाती है",
        "clearing": "साफ़ हो रहा है...",
        "clear": "कैश साफ़ करें"
      },
      "menu": {
        "editDescription": "यह आवाज़ बदलें",
        "deleteDescription": "यह आवाज़ हटाएँ",
        "provider": "प्रदाता",
        "category": "श्रेणी",
        "createVoiceConfig": "वॉयस कॉन्फ़िग बनाएँ",
        "createVoiceConfigDescription": "इस आवाज़ को कस्टम सेटिंग्स के साथ उपयोग करें"
      },
      "editor": {
        "editTitle": "आवाज़ संपादित करें",
        "createTitle": "आवाज़ बनाएँ",
        "voiceName": "आवाज़ का नाम",
        "voiceNamePlaceholder": "मेरे कैरेक्टर की आवाज़",
        "provider": "प्रदाता",
        "model": "मॉडल",
        "modelIdPlaceholder": "gpt-4o-mini-tts",
        "modelIdHint": "वह सटीक मॉडल ID दर्ज करें जो आपका compatible endpoint चाहता है",
        "elevenlabsVoice": "ElevenLabs आवाज़",
        "noVoicesAvailable": "कोई आवाज़ उपलब्ध नहीं",
        "selectVoice": "आवाज़ चुनें...",
        "elevenlabsVoiceHint": "अपनी ElevenLabs आवाज़ों में से चुनें",
        "geminiVoice": "Gemini आवाज़",
        "geminiVoiceHint": "Gemini TTS आवाज़ चुनें",
        "voiceId": "वॉयस ID",
        "voiceIdPlaceholder": "alloy",
        "voiceIdHint": "वह वॉयस ID दर्ज करें जो आपका compatible endpoint सपोर्ट करता है",
        "voicePrompt": "वॉयस प्रॉम्प्ट",
        "voicePromptPlaceholder": "एक गर्म, मित्रवत आवाज़ जो खुशनुमा हो...",
        "voicePromptHint": "बताएँ कि आवाज़ कैसी सुनाई देनी चाहिए",
        "exampleText": "उदाहरण टेक्स्ट",
        "exampleTextPlaceholder": "नमस्ते! बोलते समय मैं ऐसा सुनाई देता हूँ...",
        "exampleTextHint": "आवाज़ परीक्षण के लिए नमूना टेक्स्ट",
        "voiceDesignChars": "वॉयस डिज़ाइन प्रीव्यू के लिए {{current}}/{{minimum}} अक्षर आवश्यक",
        "defaultSample": "नमस्ते! बोलते समय मैं ऐसा सुनाई देता हूँ। मैं गर्मजोशी, स्पष्टता और भावना के साथ लंबे अंश पढ़ सकता हूँ ताकि आप मेरे स्वर और गति का अंदाज़ा लगा सकें।",
        "playing": "चल रहा है...",
        "previewVoice": "आवाज़ प्रीव्यू करें"
      },
      "kokoro": {
        "title": "Kokoro Studio",
        "newBlend": "नया ब्लेंड",
        "editBlend": "ब्लेंड संपादित करें",
        "tryText": "नमस्ते! यह एक त्वरित परीक्षण है कि मैं कैसा सुनाई देता हूँ।",
        "experimentDefaultText": "नमस्ते! बोलते समय मैं ऐसा सुनाई देता हूँ। मैं गर्मजोशी, स्पष्टता और भावना के साथ लंबे अंश पढ़ सकता हूँ।",
        "livePreview": "लाइव प्रीव्यू",
        "savedBlend": "सेव किया गया ब्लेंड",
        "defaultPreviewText": "नमस्ते! यह इस आवाज़ के सुनाई देने का एक त्वरित प्रीव्यू है।",
        "experiment": "प्रयोग",
        "providerNotFound": "Kokoro प्रदाता नहीं मिला",
        "backToProviders": "प्रदाताओं पर वापस जाएँ",
        "variantUnset": "वेरिएंट सेट नहीं",
        "ready": "तैयार",
        "modelNotInstalled": "मॉडल इंस्टॉल नहीं है",
        "voiceCount": "{{count}} आवाज़",
        "engineActions": "इंजन क्रियाएँ",
        "engineNotInstalled": "इंजन इंस्टॉल नहीं है",
        "installAtLeastOneVoice": "कम से कम एक आवाज़ इंस्टॉल करें",
        "continueSetup": "Kokoro मॉडल इंस्टॉल करने के लिए सेटअप जारी रखें।",
        "pickVoiceOrStarter": "शुरू करने के लिए कोई आवाज़ चुनें या स्टार्टर पैक लें।",
        "downloadsFailed": "{{count}} डाउनलोड विफल",
        "retryOrDismissAll": "अलग-अलग पुनः प्रयास करें या सभी खारिज करें।",
        "dismissAll": "सभी खारिज करें",
        "model": "मॉडल",
        "voice": "आवाज़",
        "downloads": "डाउनलोड",
        "cancelAll": "सभी रद्द करें",
        "experimentPlaceholder": "सुनने के लिए कोई वाक्यांश टाइप करें...",
        "speed": "गति",
        "speak": "बोलें",
        "yourBlends": "आपके ब्लेंड",
        "noSavedBlends": "अभी तक कोई सेव किया गया ब्लेंड नहीं।",
        "installModelAndVoiceFirst": "पहले मॉडल और एक आवाज़ इंस्टॉल करें।",
        "featured": "फ़ीचर्ड",
        "stop": "रोकें",
        "sample": "नमूना",
        "voiceLibrary": "वॉयस लाइब्रेरी",
        "starterPack": "स्टार्टर पैक",
        "select": "चुनें",
        "all": "सभी",
        "installed": "इंस्टॉल",
        "installModelToBrowse": "आवाज़ें ब्राउज़ करने के लिए मॉडल इंस्टॉल करें।",
        "noVoicesInCatalog": "कैटलॉग में कोई आवाज़ नहीं। Refresh टैप करें।",
        "noVoicesMatch": "आपके फ़िल्टर से कोई आवाज़ मेल नहीं खाती।",
        "collapseAll": "सभी संकुचित करें",
        "expandAll": "सभी विस्तृत करें",
        "selectedCount": "{{count}} चुनी गई",
        "engineTitle": "Kokoro इंजन",
        "variant": "वेरिएंट",
        "status": "स्थिति",
        "notInstalled": "इंस्टॉल नहीं",
        "file": "फ़ाइल",
        "modelSize": "मॉडल साइज़",
        "voicesSize": "आवाज़ें साइज़",
        "total": "कुल",
        "assetRoot": "एसेट रूट",
        "reinstallModel": "मॉडल पुनः इंस्टॉल करें",
        "installModel": "मॉडल इंस्टॉल करें",
        "deleteModel": "मॉडल हटाएँ",
        "deleteModelDescription": "डिस्क खाली होती है; आवाज़ें सुरक्षित रहती हैं।",
        "blend": "ब्लेंड",
        "previewDescription": "डिफ़ॉल्ट टेक्स्ट के साथ त्वरित सुनें",
        "editBlendDescription": "आवाज़ें, वेट और गति समायोजित करें",
        "deleteBlendDescription": "यह सेव किया गया ब्लेंड हटाएँ",
        "setupTitle": "Kokoro सेटअप करें",
        "allSet": "सब तैयार",
        "allSetDescription": "आवाज़ें ब्राउज़ करें, ब्लेंड डिज़ाइन करें, या प्रयोग क्षेत्र में परीक्षण करें।"
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
      "conditionalsSection": "सशर्त",
      "injectionPoints": "इंजेक्शन बिंदु"
    }
  },
  "embeddings": {
    "download": {
      "bestForQuick": "त्वरित उत्तरों के लिए सर्वोत्तम",
      "balancedPerf": "संतुलित प्रदर्शन",
      "maxContext": "अधिकतम संदर्भ",
      "capacity1k": "1K टोकन",
      "capacity2k": "2K टोकन",
      "capacity4k": "4K टोकन",
      "approxSize": "~138 MB",
      "downloadVersion": "V4"
    },
    "test": {
      "health": "स्वास्थ्य जाँच",
      "retrieval": "पुनर्प्राप्ति",
      "separation": "पृथक्करण",
      "passed": "पास",
      "failed": "विफल",
      "testing": "परीक्षण हो रहा है..."
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
      "jsonDesc": "Tool calling उपलब्ध न होने पर कॉम्पैक्ट structured output।",
      "xml": "XML",
      "xmlDesc": "जब मॉडल JSON से ज़्यादा reliably XML फ़ॉर्मेट करता हो तब उपयोग करें।",
      "fallbackFormat": "फ़ॉलबैक फ़ॉर्मेट"
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
      "filters": "फ़िल्टर",
      "model": "मॉडल",
      "character": "कैरेक्टर",
      "clearAll": "सब साफ़ करें",
      "applyFilters": "फ़िल्टर लागू करें",
      "recentActivity": "हाल की गतिविधि",
      "customRange": "कस्टम अवधि",
      "startDate": "प्रारंभ तिथि",
      "endDate": "समाप्ति तिथि",
      "applyRange": "अवधि लागू करें",
      "dashboard": "डैशबोर्ड",
      "appTime": "ऐप समय",
      "today": "आज",
      "last7Days": "7 दिन",
      "last30Days": "30 दिन",
      "all": "सभी",
      "custom": "कस्टम",
      "filtersCount": "{{count}} फ़िल्टर",
      "totalCost": "कुल लागत",
      "tokens": "टोकन",
      "avgShort": "{{value}} औसत",
      "requests": "अनुरोध",
      "period": "अवधि",
      "last7d": "पिछले 7 दिन",
      "last30d": "पिछले 30 दिन",
      "allTime": "सभी समय",
      "recordsCount": "{{count}} रिकॉर्ड",
      "usageTrend": "उपयोग प्रवृत्ति",
      "tokenConsumptionOverTime": "समय के साथ टोकन उपयोग",
      "input": "इनपुट",
      "output": "आउटपुट",
      "byModel": "मॉडल के अनुसार",
      "byCharacter": "कैरेक्टर के अनुसार",
      "top": "शीर्ष",
      "active": "सक्रिय",
      "quickView": "त्वरित दृश्य",
      "viewAll": "सभी देखें",
      "startChatting": "उपयोग डेटा देखने के लिए चैट शुरू करें",
      "exportedTo": "निर्यात किया गया: {{path}}",
      "periodTotal": "अवधि कुल",
      "daysActive": "{{count}} दिन सक्रिय",
      "dailyAvg": "दैनिक औसत",
      "selectedPeriod": "चयनित अवधि",
      "yesterdayValue": "कल {{value}}",
      "thirtyDayAvg": "30 दिन का औसत",
      "appTimeTrend": "ऐप समय प्रवृत्ति",
      "usageDurationPerDay": "प्रति दिन उपयोग अवधि",
      "totalValue": "कुल {{value}}",
      "activeTime": "सक्रिय समय"
    },
    "activity": {
      "loading": "आपकी गतिविधि लोड हो रही है...",
      "title": "हाल की गतिविधि",
      "recordsCount": "{{count}} उपयोग रिकॉर्ड",
      "rangeOfTotal": "{{start}}-{{end}} में से {{total}}",
      "previous": "पिछला",
      "next": "अगला",
      "pageOf": "पृष्ठ {{page}} में से {{total}}"
    },
    "shared": {
      "relativeTime": {
        "justNow": "अभी-अभी",
        "minutesAgo": "{{count}} मिनट पहले",
        "hoursAgo": "{{count}} घंटे पहले",
        "daysAgo": "{{count}} दिन पहले"
      },
      "operations": {
        "chat": "चैट",
        "regenerate": "पुनः जनरेट",
        "continue": "जारी रखें",
        "summary": "सारांश",
        "memoryManager": "मेमोरी",
        "imageGeneration": "छवि जनरेशन",
        "aiCreator": "AI Creator",
        "replyHelper": "Reply Helper",
        "groupChatMessage": "ग्रुप चैट",
        "groupChatRegenerate": "ग्रुप पुनः जनरेट",
        "groupChatContinue": "ग्रुप जारी रखें",
        "groupChatDecisionMaker": "निर्णय कर्ता"
      },
      "outputImages": "{{count}} छवियाँ",
      "tokens": "{{count}} टोकन",
      "unknown": "अज्ञात",
      "requestDetails": "अनुरोध विवरण",
      "noStopReason": "कोई रोकने का कारण नहीं",
      "tokenUsage": "टोकन उपयोग",
      "estimatedCost": "अनुमानित लागत",
      "stats": {
        "prompt": "प्रॉम्प्ट",
        "completion": "पूर्णता",
        "total": "कुल",
        "reasoning": "तर्क",
        "image": "छवि",
        "memory": "मेमोरी",
        "summary": "सारांश",
        "inputImages": "इनपुट छवियाँ",
        "outputImages": "आउटपुट छवियाँ",
        "cachedPrompt": "कैश्ड प्रॉम्प्ट",
        "cacheWrite": "कैश लिखना",
        "webSearches": "वेब खोजें",
        "cacheRead": "कैश पढ़ना",
        "requestFee": "अनुरोध शुल्क",
        "webSearch": "वेब खोज",
        "providerTotal": "प्रदाता कुल"
      }
    }
  }
};
