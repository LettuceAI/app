import type { DeepPartialMessageTree } from "../types";
import { type LocaleMessages } from "./en";
import type { LocaleMetadata } from ".";

export const trMetadata: LocaleMetadata = {
  name: "Turkish",
  label: "Türkçe",
};

export const trMessages: DeepPartialMessageTree<LocaleMessages> = {
  "common": {
    "nav": {
      "chats": "Sohbetler",
      "settings": "Ayarlar",
      "providers": "Sağlayıcılar",
      "responseStyle": "Yanıt tarzı",
      "models": "Modeller",
      "security": "Güvenlik",
      "accessibility": "Erişilebilirlik",
      "speechRecognition": "Konuşma tanıma",
      "reset": "Sıfırla",
      "backupRestore": "Yedekleme ve geri yükleme",
      "convertFiles": "Dosya dönüştür",
      "usageAnalytics": "Kullanım analizi",
      "changelog": "Değişiklik günlüğü",
      "about": "Hakkında",
      "createSystemPrompt": "Sistem promptu oluştur",
      "editSystemPrompt": "Sistem promptunu düzenle",
      "systemPrompts": "Sistem promptları",
      "developer": "Geliştirici",
      "advanced": "Gelişmiş",
      "hostApi": "API sunucusu",
      "characters": "Karakterler",
      "lorebooks": "Lore kitapları",
      "personas": "Personalar",
      "dynamicMemory": "Dinamik hafıza",
      "creationHelper": "Oluşturma asistanı",
      "helpMeReply": "Yanıtlamamda yardım et",
      "lorebookEntryGenerator": "Lorebook girdi oluşturucu",
      "companionSoulWriter": "Yoldaş ruh yazarı",
      "editPersona": "Personayı düzenle",
      "newTemplate": "Yeni şablon",
      "editTemplate": "Şablonu düzenle",
      "chatTemplates": "Sohbet şablonları",
      "editCharacter": "Karakteri düzenle",
      "sync": "Senkronizasyon",
      "newCharacter": "Yeni karakter",
      "engineSetup": "Motor kurulumu",
      "llmProviders": "LLM Sağlayıcıları",
      "engineSettings": "Motor ayarları",
      "lettuceEngine": "Lettuce Motoru",
      "create": "Oluştur",
      "setup": "Kurulum",
      "welcome": "Hoş geldin",
      "conversation": "Konuşma",
      "library": "Kütüphane",
      "groupChats": "Grup sohbetleri",
      "groupChat": "Grup sohbeti",
      "imageGeneration": "Görsel oluşturma",
      "voices": "Sesler",
      "chatAppearance": "Sohbet görünümü",
      "colorCustomization": "Özel renkler",
      "embeddingDownload": "Embedding indirme",
      "embeddingTest": "Embedding testi",
      "editModel": "Modeli düzenle",
      "messageDebug": "Mesaj hata ayıklama"
    },
    "bottomNav": {
      "chats": "Sohbetler",
      "groups": "Gruplar",
      "create": "Oluştur",
      "discover": "Keşfet",
      "library": "Kütüphane"
    },
    "buttons": {
      "cancel": "İptal",
      "confirm": "Onayla",
      "save": "Kaydet",
      "saving": "Kaydediliyor...",
      "delete": "Sil",
      "deleting": "Siliniyor...",
      "create": "Oluştur",
      "creating": "Oluşturuluyor...",
      "edit": "Düzenle",
      "back": "Geri",
      "done": "Tamam",
      "download": "İndir",
      "later": "Daha sonra",
      "proceed": "Devam et",
      "retry": "Tekrar dene",
      "discard": "Vazgeç",
      "import": "İçe aktar",
      "importing": "İçe aktarılıyor...",
      "export": "Dışa aktar",
      "exporting": "Dışa aktarılıyor...",
      "update": "Güncelle",
      "generate": "Oluştur",
      "refresh": "Yenile",
      "continue": "Devam et",
      "goBack": "Geri dön",
      "search": "Ara",
      "clearSearch": "Aramayı temizle",
      "add": "Ekle",
      "install": "Kur",
      "remove": "Kaldır",
      "rename": "Yeniden adlandır",
      "copy": "Kopyala",
      "copied": "Kopyalandı!",
      "browseFiles": "Dosyalara gözat"
    },
    "labels": {
      "processing": "İşleniyor...",
      "loading": "Yükleniyor...",
      "noDescriptionYet": "Henüz açıklama yok",
      "untitled": "Başlıksız",
      "default": "Varsayılan",
      "enabled": "Etkin",
      "disabled": "Devre dışı",
      "on": "Açık",
      "off": "Kapalı",
      "none": "Hiçbiri",
      "auto": "Otomatik",
      "custom": "Özel",
      "name": "Ad",
      "description": "Açıklama",
      "content": "İçerik",
      "preview": "Önizleme",
      "options": "Seçenekler"
    },
    "time": {
      "justNow": "Az önce",
      "minutesAgo": "{{minutes}}dk önce",
      "hoursAgo": "{{hours}}sa önce",
      "daysAgo": "{{days}}g önce",
      "today": "Bugün",
      "yesterday": "Dün",
      "last7Days": "Son 7 gün",
      "older": "Daha eski"
    }
  },
  "settings": {
    "sections": {
      "intelligence": "Zeka",
      "experience": "Deneyim",
      "connectivity": "Bağlantı",
      "securityPrivacy": "Güvenlik ve gizlilik",
      "supportInfo": "Destek ve bilgi",
      "dangerZone": "Tehlikeli bölge",
      "developer": "Geliştirici"
    },
    "items": {
      "providers": {
        "title": "Sağlayıcılar",
        "subtitle": "Yapay zeka servislerine bağlan"
      },
      "models": {
        "title": "Modeller",
        "subtitle": "Yapay zeka modellerini yapılandır"
      },
      "imageGeneration": {
        "title": "Görsel oluşturma",
        "subtitle": "Görseller oluştur ve test et"
      },
      "voices": {
        "title": "Sesler",
        "subtitle": "Metinden konuşmaya sesler"
      },
      "accessibility": {
        "title": "Erişilebilirlik",
        "subtitle": "Sesler ve dokunsal geri bildirim"
      },
      "prompts": {
        "title": "Sistem promptları",
        "subtitle": "Yapay zekanın kişiliğini şekillendir"
      },
      "security": {
        "title": "Güvenlik",
        "subtitle": "Şifreleme ve gizlilik"
      },
      "backup": {
        "title": "Yedekleme",
        "subtitle": "Verileri dışa veya içe aktar"
      },
      "convert": {
        "title": "Dosya dönüştür",
        "subtitle": ".json dışa aktarımlarını .uec'ye dönüştür"
      },
      "sync": {
        "title": "Yerel senkronizasyon",
        "subtitle": "Cihazlar arası senkronize et"
      },
      "usage": {
        "title": "Kullanım analizi",
        "subtitle": "Maliyet ve token istatistikleri"
      },
      "advanced": {
        "title": "Gelişmiş",
        "subtitle": "Hafıza ve işlevler"
      },
      "logs": {
        "title": "Günlükler",
        "subtitle": "Hata ayıklama ve tanılama"
      },
      "guide": {
        "title": "Kurulum rehberi",
        "subtitle": "Tanıtımı tekrarla"
      },
      "docs": {
        "title": "Belgeler",
        "subtitle": "Kılavuzlar ve referans"
      },
      "github": {
        "title": "Sorun bildir",
        "subtitle": "Hatalar ve geri bildirim • v{{version}}"
      },
      "discord": {
        "title": "Discord'a katıl",
        "subtitle": "Topluluk ve yardım"
      },
      "about": {
        "title": "Hakkında",
        "subtitle": "Sürüm, güncellemeler ve bağlantılar"
      },
      "changelog": {
        "title": "Değişiklik günlüğü",
        "subtitle": "Yenilikler"
      },
      "reset": {
        "title": "Sıfırla",
        "subtitle": "Her şeyi sil"
      },
      "developer": {
        "title": "Geliştirici araçları",
        "subtitle": "Hata ayıklama ve test"
      }
    }
  },
  "accessibility": {
    "sectionTitles": {
      "language": "Dil",
      "sounds": "Sesli geri bildirim",
      "haptics": "Dokunsal geri bildirim",
      "appearance": "Görünüm"
    },
    "language": {
      "appLanguage": "Uygulama dili",
      "description": "Gezinme ve ayarlarda kullanılan dili seç."
    },
    "appearance": {
      "customColors": "Özel renkler",
      "customColorsDesc": "Uygulamanın renk şemasını özelleştir",
      "chatAppearance": "Sohbet görünümü",
      "chatAppearanceDesc": "Mesaj balonlarını, yazı tiplerini ve düzeni özelleştir"
    },
    "haptics": {
      "vibrateOnChat": "Sohbette titreşim",
      "vibrateDesc": "Asistan yazarken kısa titreşim darbeleri",
      "intensity": "Yoğunluk",
      "light": "Hafif",
      "medium": "Orta",
      "heavy": "Güçlü",
      "soft": "Yumuşak",
      "rigid": "Sert"
    },
    "sounds": {
      "send": "Gönder",
      "sendDescription": "Mesaj gönderildiğinde ses çalar",
      "success": "Başarılı",
      "successDescription": "Asistan başarıyla tamamlandığında ses çalar",
      "failure": "Hata",
      "failureDescription": "Hata veya iptal durumunda ses çalar",
      "testButton": "Test et"
    },
    "feedbackInfo": "Geri bildirim, mesajların gönderildiğini veya alındığını fark etmeni sağlar.",
    "hapticsInfo": "Dokunsal geri bildirim mobil cihazlarda kullanılabilir.",
    "extra": {
      "easterEggs": "Sürprizler",
      "beetrootRain": "Pancar yağmuru",
      "beetrootDesc": "Sohbette pancardan bahsedilince yukarıdan pancarlar düşer"
    }
  },
  "topNav": {
    "unsavedChangesTitle": "Kaydedilmemiş değişiklikler",
    "unsavedChangesMessage": "Çıkmadan önce değişikliklerini kaydet veya vazgeç.",
    "goBack": "Geri dön",
    "changeLayout": "Düzeni değiştir",
    "search": "Ara",
    "settings": "Ayarlar",
    "help": "Yardım",
    "add": "Ekle",
    "openFilters": "Filtreleri aç",
    "save": "Kaydet",
    "extra": {
      "installedModels": "Kurulu modeller",
      "refresh": "Yenile",
      "minimize": "Küçült",
      "maximize": "Büyüt",
      "close": "Kapat"
    }
  },
  "updates": {
    "available": {
      "title": "Yeni sürüm mevcut",
      "description": "v{{latestVersion}} kullanılabilir. Şu anda v{{currentVersion}} kullanıyorsun.",
      "actions": {
        "view": "Sürümü görüntüle"
      }
    }
  },
  "about": {
    "kicker": "Uygulama bilgisi",
    "appName": "LettuceAI",
    "description": "Sürüm bilgileri, güncelleme kontrolü ve faydalı bağlantılar.",
    "info": {
      "version": "Sürüm",
      "channel": "Kanal",
      "platform": "Platform"
    },
    "buildChannel": {
      "dev": "Geliştirme sürümü",
      "release": "Kararlı sürüm"
    },
    "update": {
      "sectionTitle": "Güncellemeler",
      "title": "Uygulama güncellemeleri",
      "description": "Yeni sürümleri manuel olarak kontrol et veya başlangıçta otomatik kontrolleri devre dışı bırak.",
      "autoChecks": "Otomatik güncelleme kontrolleri",
      "autoChecksDescription": "Etkinleştirildiğinde, LettuceAI uygulama açıldığında yeni sürümleri kontrol eder.",
      "checkNow": "Şimdi kontrol et",
      "checking": "Güncellemeler kontrol ediliyor...",
      "upToDateTitle": "Güncelsin",
      "upToDateDescription": "Bu cihaz için şu anda daha yeni bir sürüm mevcut değil.",
      "failedTitle": "Güncelleme kontrolü başarısız",
      "failedDescription": "Güncellemeler şu anda kontrol edilemedi. Biraz sonra tekrar dene."
    },
    "links": {
      "sectionTitle": "Bağlantılar",
      "website": "Web sitesi",
      "websiteDescription": "İndirme sayfası ve sürüm bilgileri",
      "github": "GitHub",
      "githubDescription": "Kaynak kodu, sorunlar ve geliştirme",
      "discord": "Discord",
      "discordDescription": "Topluluk sunucusu ve destek",
      "reddit": "Reddit",
      "redditDescription": "Topluluk tartışmaları, geri bildirim ve güncellemeler"
    },
    "developerMode": {
      "enable": "Geliştirici modunu etkinleştir",
      "enabled": "Geliştirici modu etkinleştirildi"
    },
    "errors": {
      "saveTitle": "Tercih kaydedilemedi",
      "saveDescription": "Güncelleme kontrolü tercihin değiştirilemedi."
    }
  },
  "components": {
    "tooltip": {
      "dismissHint": "Kapatmak için herhangi bir yere dokun"
    },
    "backgroundPosition": {
      "title": "Arka planı konumlandır",
      "instructions": "Konumlandırmak için sürükle • Yakınlaştırmak için sıkıştır veya kaydır"
    },
    "avatarSource": {
      "generateImage": "Görsel oluştur",
      "generateImageDesc": "Yapay zeka ile avatar oluşturma",
      "noImageModels": "Görsel modeli mevcut değil",
      "editCurrent": "Mevcut olanı düzenle",
      "editCurrentDesc": "Yeniden konumlandır veya kırp",
      "chooseImage": "Görsel seç",
      "chooseImageDesc": "Cihazından seç"
    },
    "avatarCurrentEdit": {
      "title": "Mevcut olanı düzenle",
      "reposition": "Yeniden konumlandır",
      "repositionDesc": "Mevcut avatarı taşı veya kırp",
      "editWithAI": "Yapay zeka ile düzenle",
      "editWithAIDesc": "Mevcut avatar için yapay zeka düzenlemesini aç",
      "noImageModels": "Görsel modeli mevcut değil"
    },
    "avatarGeneration": {
      "modelsLoadError": "Görsel oluşturma modelleri yüklenemedi",
      "title": "Avatar oluştur",
      "help": "Avatar oluşturma yardımı",
      "model": "Model",
      "selectModel": "Model seç",
      "describe": "Avatarını tanımla",
      "describePlaceholder": "Renkli saçlı, sıcak bir şekilde gülümseyen arkadaş canlısı bir anime kız...",
      "inProgress": "Avatar oluşturuluyor...",
      "editingInProgress": "Avatar düzenlemesi uygulanıyor...",
      "alt": "Oluşturulan avatar",
      "regenerate": "Yeniden oluştur",
      "useThis": "Bunu kullan",
      "previousVariant": "Önceki varyant",
      "nextVariant": "Sonraki varyant",
      "variantCounter": "{{current}} / {{total}}",
      "editRequest": "Düzenleme isteği",
      "editRequestPlaceholder": "Saçını koyulaştır, gözlük ekle, yüzü aynı tut...",
      "applyEdit": "Düzenlemeyi uygula",
      "editImageLoadError": "Oluşturulan avatar düzenleme için hazırlanamadı",
      "aiAssistant": "Yapay zeka asistanı",
      "backToResults": "Mesaja dön",
      "magicInTheWorks": "Sihir hazırlanıyor...",
      "refine": "İyileştir",
      "apply": "Uygula"
    },
    "avatarPosition": {
      "title": "Avatarı konumlandır",
      "instructions": "Konumlandırmak için sürükle • Yakınlaştırmak için sıkıştır veya kaydır",
      "alt": "Konumlandırılacak avatar"
    },
    "confirmDialog": {
      "defaultTitle": "Onayla",
      "defaultLabel": "Onayla"
    },
    "bottomMenu": {
      "defaultTitle": "Menü",
      "dragTip": "Menüyü kapatmak için sürükle",
      "closeLabel": "Menüyü kapat",
      "buttonProcessing": "İşleniyor..."
    },
    "modelSelector": {
      "placeholder": "Model seç",
      "clearLabel": "Global varsayılanı kullan",
      "loading": "Modeller yükleniyor...",
      "noModels": "Kullanılabilir model yok",
      "addProviderFirst": "Önce ayarlardan bir sağlayıcı ekle",
      "title": "Model seç",
      "searchPlaceholder": "Model ara...",
      "noResults": "Model bulunamadı",
      "noResultsHint": "Başka bir arama terimi dene"
    },
    "localeSelector": {
      "title": "Dil seç"
    },
    "promptTemplate": {
      "nameContentRequired": "Ad ve içerik zorunludur",
      "saveError": "Şablon kaydedilemedi",
      "editTitle": "Promptu düzenle",
      "createTitle": "Prompt oluştur",
      "name": "Ad",
      "namePlaceholder": "ör. Roleplay ustası",
      "content": "İçerik",
      "variablesButton": "Değişkenler",
      "contentPlaceholder": "Yardımsever bir yapay zeka asistanısın...\n\nPromptunda {{char.name}} ve {{scene}} kullan.",
      "characterCount": "{{count}} karakter",
      "preview": "Önizleme",
      "characterPlaceholder": "Karakter…",
      "personaPlaceholder": "Kişilik…",
      "rendering": "İşleniyor…",
      "noPreview": "Henüz önizleme yok",
      "saving": "Kaydediliyor...",
      "update": "Güncelle",
      "create": "Oluştur",
      "variablesTitle": "Şablon değişkenleri",
      "variablesCopyHint": "Panoya kopyalamak için dokun",
      "variablesCopied": "Kopyalandı",
      "variables": {
        "charName": "Karakter adı",
        "charNameDesc": "Karakterin adı",
        "charDesc": "Karakter açıklaması",
        "charDescDesc": "Karakterin açıklaması",
        "scene": "Sahne",
        "sceneDesc": "Başlangıç sahnesi/senaryosu",
        "userName": "Kullanıcı adı",
        "userNameDesc": "Kullanıcı personasının adı",
        "userDesc": "Kullanıcı açıklaması",
        "userDescDesc": "Kullanıcı personasının açıklaması",
        "rules": "Kurallar",
        "rulesDesc": "Karakter davranış kuralları",
        "contextSummary": "Bağlam özeti",
        "contextSummaryDesc": "Konuşmanın dinamik özeti",
        "keyMemories": "Önemli anılar",
        "keyMemoriesDesc": "İlgili anıların listesi"
      }
    },
    "characterExport": {
      "title": "Dışa aktarma formatı",
      "selectFormat": "Bir format seç",
      "loading": "Formatlar yükleniyor...",
      "formatUecDesc": "Unified Entity Card (.uec) formatı (önerilen).",
      "formatLegacyJsonDesc": "Eski JSON (yalnızca içe aktarma).",
      "formatV3Desc": "Character Card V3 JSON (en son spesifikasyon).",
      "formatV2Desc": "Character Card V2 JSON (Tavern spesifikasyonu).",
      "formatV1Desc": "Character Card V1 (yalnızca içe aktarma)."
    },
    "embeddingDownload": {
      "downloadRequired": "İndirme gerekli",
      "modelRequired": "Embedding modeli gerekli",
      "description": "Dinamik hafıza, çalışabilmek için yerel bir embedding modeli (~260 MB) indirmeyi gerektirir.",
      "localStorage": "• Model cihazında yerel olarak depolanacak",
      "downloadSize": "• İndirme boyutu: yaklaşık 260 MB",
      "summarization": "• Konuşma özetleme için gerekli"
    },
    "embeddingUpgrade": {
      "title": "Embedding modeli v3 mevcut",
      "v1Message": "512 token ile v1 kullanıyorsun. Daha iyi hafıza kalitesi ve uzun bağlam desteği için v3'e yükselt.",
      "v2Message": "Eski v2 kullanıyorsun. En son embedding modeli ile daha iyi hafıza kalitesi için v3'e yükselt.",
      "v3Message": "v4 kullanım dışı ve v3'e göre rol oyunu hafızasının hatırlanmasını önemli ölçüde artırıyor (geri çağırma@1 0,02 -> 0,92). ",
      "button": "v3'e yükselt"
    },
    "v2UpgradeToast": {
      "title": "Hafıza modeli v3",
      "badge": "Mevcut",
      "message": "v2'ye göre geliştirilmiş embedding kalitesi",
      "dismiss": "Kapat",
      "upgrade": "Yükselt"
    },
    "v1UpgradeToast": {
      "title": "Hafıza modeli v3 mevcut",
      "message": "Daha iyi hafıza kalitesi ve uzun bağlam desteği için yükselt.",
      "dismiss": "Kapat",
      "upgrade": "Yükselt"
    },
    "v3UpgradeToast": {
      "title": "Bellek modeli v4",
      "badge": "Mevcut",
      "message": "v4, v3'e göre rol oyunu hafızasının hatırlanmasını önemli ölçüde artırır (geri çağırma@1 0,02 → 0,92). ",
      "dismiss": "Daha sonra",
      "upgrade": "Yükseltme"
    },
    "createMenu": {
      "title": "Yeni oluştur",
      "smartCreator": "Akıllı oluşturucu",
      "smartCreatorDesc": "Asistanın oluşturma sürecini yönlendirsin",
      "divider": "Veya manuel oluştur",
      "character": "Karakter",
      "characterDesc": "Özel bir karakter oluştur",
      "persona": "Persona",
      "personaDesc": "Yeniden kullanılabilir bir ses oluştur",
      "groupChat": "Grup sohbeti",
      "groupChatDesc": "Birden fazla karakterle sohbet et",
      "lorebook": "Lore kitabı",
      "lorebookDesc": "Dünya referansını oluştur",
      "characterSmartDesc": "Rehberli oluşturma ile bir karakter inşa et",
      "personaSmartDesc": "Yeniden kullanılabilir bir ses veya kişilik oluştur",
      "lorebookSmartDesc": "Yapılandırılmış bir dünya referansı inşa et",
      "loadingConversations": "Konuşmalar yükleniyor...",
      "createNew": "Yeni oluştur",
      "createNewDesc": "Yeni bir rehberli oluşturma sohbeti başlat",
      "editExisting": "Mevcut olanı düzenle",
      "continueLast": "Son konuşmaya devam et",
      "seeOlder": "Eski konuşmaları gör",
      "seeOlderDesc": "Herhangi bir önceki Akıllı oluşturucu konuşmasını aç",
      "noConversations": "Bu oluşturucu için henüz konuşma yok.",
      "sessionCompleted": "Tamamlandı",
      "sessionCancelled": "İptal edildi",
      "sessionDraft": "Taslak",
      "sessionMessages": "{{count}} mesaj",
      "untitledConversation": "Başlıksız konuşma",
      "nameLorebookTitle": "Lore kitabını adlandır",
      "lorebookNamePlaceholder": "Lore kitabı adını gir...",
      "lorebookImporting": "İçe aktarılıyor...",
      "lorebookImport": "İçe aktar",
      "lorebookCreating": "Oluşturuluyor...",
      "lorebookCreate": "Oluştur",
      "editExistingDesc": "Bir {{goal}} seç ve Akıllı oluşturucu ile düzenle",
      "creatorTitle": "{{goal}} oluşturucusu",
      "editTitle": "{{goal}} düzenle",
      "conversationsTitle": "{{goal}} konuşmaları",
      "loadingItems": "{{items}} yükleniyor...",
      "noItemsFound": "{{items}} bulunamadı.",
      "unnamedCharacter": "Adsız karakter",
      "untitledPersona": "Başlıksız persona",
      "untitledLorebook": "Başlıksız lore kitabı"
    },
    "advancedModelSettings": {
      "temperature": "Sıcaklık",
      "temperatureDesc": "Yüksek = daha yaratıcı",
      "topP": "Top P",
      "topPDesc": "Düşük = daha odaklı",
      "maxTokens": "Maksimum çıktı tokenleri",
      "maxTokensDesc": "Varsayılan için boş bırak",
      "contextLength": "Bağlam uzunluğu",
      "contextLengthDesc": "Yalnızca yerel modeller",
      "contextLengthAuto": "Otomatik",
      "frequencyPenalty": "Frekans cezası",
      "frequencyPenaltyDesc": "Token tekrarını azalt",
      "presencePenalty": "Varlık cezası",
      "presencePenaltyDesc": "Yeni konuları teşvik et",
      "topK": "Top K",
      "topKDesc": "Token havuzu boyutunu sınırla",
      "reasoning": "Düşünme / Akıl yürütme",
      "reasoningAutoDesc": "Bu model her zaman akıl yürütme kullanır. Yapılandırma gerekmez.",
      "reasoningEnableDesc": "Karmaşık problem çözme ve akıl yürütme görevleri için gelişmiş düşünme yeteneklerini etkinleştir.",
      "effortMode": "Çaba modu",
      "budgetMode": "Bütçe modu",
      "reasoningEffort": "Akıl yürütme çabası",
      "reasoningEffortDesc": "Düşünme derinliğini kontrol eder",
      "reasoningEffortAuto": "Otomatik",
      "reasoningEffortLow": "Düşük",
      "reasoningEffortMed": "Orta",
      "reasoningEffortHigh": "Yüksek",
      "reasoningBudget": "Akıl yürütme bütçesi (token)",
      "reasoningBudgetDesc": "Düşünme için ayrılan maksimum token",
      "reasoningEffortLowDesc": "Minimum akıl yürütme ile hızlı yanıtlar",
      "reasoningEffortMediumDesc": "Dengeli akıl yürütme derinliği",
      "reasoningEffortHighDesc": "Karmaşık problemler için maksimum akıl yürütme derinliği",
      "reasoningBudgetExtendedDesc": "Düşünme için ayrılan maksimum token. Çıktı limitine eklenir."
    },
    "providerParameterSupport": {
      "unknownProvider": "Bilinmeyen sağlayıcı: {{providerId}}",
      "reasoningNotSupportedEffort": "Desteklenmiyor — bu sağlayıcı çaba seviyeleri kullanmaz",
      "reasoningNotSupportedBudgetOnly": "Desteklenmiyor — bu sağlayıcı yalnızca bütçe kullanır",
      "reasoningNotSupported": "Desteklenmiyor — bu sağlayıcı akıl yürütmeyi desteklemiyor",
      "unsupportedParametersIgnored": "Desteklenmeyen parametreler {{providerName}} tarafından yok sayılacak.",
      "reasoningEffortSupported": "Düşünen modeller (o1, DeepSeek-R1 vb.) için akıl yürütme çabası destekleniyor",
      "reasoningBudgetSupported": "Bu sağlayıcı bütçe tabanlı düşünme kullanır (çaba seviyeleri yok). Bunun yerine akıl yürütme bütçe tokenlerini yapılandır.",
      "reasoningNotSupportedProvider": "Bu sağlayıcı akıl yürütme parametrelerini desteklemiyor.",
      "matrixTitle": "Sağlayıcı parametre destek matrisi",
      "providerColumn": "Sağlayıcı",
      "supportedIndicator": "Sağlayıcı API'si tarafından destekleniyor",
      "notSupportedIndicator": "Desteklenmiyor (parametre yok sayılacak)"
    },
    "extra": {
      "promptCachingTitle": "İstemi Önbelleğe Alma",
      "promptCachingDescription": "Oluşturmayı hızlandırır ve uzun, tekrarlanan bağlamlar için maliyetleri azaltır (büyük sistem istemleri veya derin sohbet geçmişleri gibi).",
      "avatarAlt": "Avatar",
      "chooseFromLibrary": "Kütüphaneden seçim yapın",
      "chooseFromLibraryDesc": "Uygulamaya önceden kaydedilmiş bir resmi kullanın",
      "generateFailed": "Resim oluşturulamadı",
      "editFailed": "Avatar düzenlenemedi",
      "backgroundAlt": "Arka plan konumu",
      "formatsLoadFailed": "Dışa aktarma formatları yüklenemedi",
      "formatsShowingDefaults": "(varsayılanlar gösteriliyor)",
      "timeJustNow": "az önce",
      "timeMinutesAgo": "{{minutes}}m önce",
      "timeHoursAgo": "{{hours}}h önce",
      "timeDaysAgo": "{{days}}d önce",
      "removeReference": "Tasarım referansını kaldır",
      "thumbLoading": "Yükleniyor..._",
      "addReferences": "Referans ekle",
      "visualDescription": "Görsel açıklama",
      "draftWithAi": "AI ile taslak",
      "regenerate": "Yeniden oluştur",
      "useThis": "Bunu Kullan_",
      "referenceImagesLabel": "Referans görselleri",
      "writerHelpFallback": "uyumlu sahne yazıcısı modeli",
      "writerHelpUses": "Avatarınızdan ve referans görsellerinden taslak oluşturmak için {{model}} kullanır.",
      "writerHelpUnavailable": "Bunun otomatik olarak taslağını çıkarmak için Görüntü Oluşturma ayarlarına uyumlu bir sahne yazıcısı modeli ekleyin.",
      "writerNotAvailableError": "Önce Görüntü Oluşturma ayarlarına uyumlu bir sahne yazıcısı modeli ekleyin.",
      "writerNoSourcesError": "Oluşturmadan önce bir avatar veya en az bir referans görüntü ekleyin.",
      "noUsableReferences": "Kullanılabilir referans görseli bulunamadı.",
      "draftFailed": "Tasarım açıklaması oluşturulamadı.",
      "draftReadFailed": "Resim varlığı okunamadı ({{status}})",
      "draftConvertFailed": "Resim varlığı veriye dönüştürülemedi URL",
      "draftGenerationFailed": "Taslak oluşturma başarısız oldu.",
      "draftMenuTitle": "AI Tasarım Taslağı",
      "draftedBy": "Geçerli avatar ve referans görsellerinden {{model}} tarafından hazırlanmıştır.",
      "draftedByFallback": "sahne yazar modeliniz",
      "noWriterUseHelper": "Bu yardımcıyı kullanmadan önce uyumlu bir sahne yazıcı modeli ekleyin.",
      "emptyReferences": "Yüzü, oranları, kıyafeti ve stili sabitlemek için birkaç net referans çekimi ekleyin.",
      "designReferencesTitle": "Tasarım referansları",
      "designReferencesDescription": "Birkaç net referans görseli ve standart bir görsel açıklama yükleyin.",
      "designReferencesPlaceholder": "Sabit görünümü açıklayın: yüz, saç, vücut yapısı, yaş sunumu, kıyafet ipuçları, aksesuarlar ve sanat/stil yönü.",
      "dismissAria": "Azletmek",
      "v3MessageFallback": "marul-emb-v4 çıktı ve rol oyunu hafızasının hatırlanmasını önemli ölçüde artırıyor. ",
      "uploadButton": "Yüklemek",
      "libraryButton": "Library",
      "companionSetupTitle": "Yardımcının kurulumu gerekiyor",
      "companionSetupSubtitleSingle": "Tamamlayıcı Modun çalışabilmesi için bir modele daha ihtiyacı var. ",
      "companionSetupSubtitleMany": "Tamamlayıcı Modun çalışabilmesi için {{count}} modele daha ihtiyacı var. ",
      "companionSetupBody": "Tamamlayıcı mod, duyguyu analiz etmek, varlıkları çıkarmak, anıları yönlendirmek ve geçmiş bağlamı hatırlamak için bazı yerel modellere ihtiyaç duyar.",
      "companionUseRoleplay": "Bunun yerine Roleplay'i kullanın",
      "companionDownloadNow": "Hemen indirin",
      "searchModelsPlaceholder": "Modelleri ara...",
      "loadingModelsDefault": "Modeller yükleniyor...",
      "noModelsAvailable": "Model yok.",
      "noModelsMatching": "\"{{query}}\" ile eşleşen model bulunamadı.",
      "contentPlaceholderText": "Sen yardımsever bir yapay zeka asistanısın...\n\n",
      "previewRenderFailed": "<önizleme oluşturulamadı>",
      "charactersCount": "{{count}} karakterler"
    }
  },
  "chats": {
    "characterNotFound": "Karakter bulunamadı",
    "chatHistory": "Sohbet geçmişi",
    "previousConversationsWithCharacter": "{{name}} ile önceki konuşmalar",
    "previousConversations": "Önceki konuşmalar",
    "searchChats": "Sohbetlerde ara...",
    "searchResults": "{{count}} sonuç",
    "noConversationsYet": "Henüz konuşma yok",
    "startChattingPrompt": "Geçmişini burada görmek için sohbet etmeye başla",
    "noMatchingChats": "Eşleşen sohbet yok",
    "tryDifferentSearchTerm": "Başka bir arama terimi dene",
    "untitledChat": "Başlıksız sohbet",
    "chatTitlePlaceholder": "Sohbet başlığı...",
    "exportChatPackage": "Sohbet paketini dışa aktar",
    "characterSpecificExport": "Karaktere özel dışa aktarma",
    "characterSpecificExportDesc": "Bu paketi varsayılan olarak sohbetin karakterine bağla.",
    "nonCharacterSpecificExport": "Karaktere özel olmayan dışa aktarma",
    "nonCharacterSpecificExportDesc": "İçe aktarırken karakter seçimi gereksin.",
    "deleteChat": "Sohbeti sil?",
    "deleteConfirmDesc": "Geçmişten kalıcı olarak silinecek",
    "keepThisChat": "Bu sohbeti koru",
    "editCharacter": "Karakteri düzenle",
    "exportCharacter": "Karakteri dışa aktar",
    "chatAppearance": "Sohbet görünümü",
    "importChatPackage": "Sohbet paketi içe aktar",
    "hideThisCharacter": "Bu karakteri gizle",
    "deleteCharacter": "Karakteri sil",
    "deleteCharacterTitle": "Karakter silinsin mi?",
    "deleteCharacterConfirmation": "\"{{name}}\" silmek istediğinden emin misin? Bu, bu karakterle olan tüm sohbet oturumlarını da silecek.",
    "characterSpecificMatches": "Bu paket karaktere özeldir ve seçili karakterle eşleşiyor.",
    "characterSpecificMismatch": "Bu paket karaktere özeldir ve başka bir karaktere yöneliktir. {{name}} içine aktarılacak.",
    "nonCharacterSpecificImport": "Bu paket karaktere özel değildir. {{name}} içine aktarılacak.",
    "noCharactersYet": "Henüz karakter yok",
    "createFirstCharacter": "Sohbet etmeye başlamak için aşağıdaki + butonundan ilk karakterini oluştur",
    "scrollToBottom": "Alta kaydır",
    "selectCharacter": "Karakter seç",
    "branchToGroupChat": "Grup sohbetine dallan",
    "addContent": "İçerik ekle",
    "swapPlaces": "Yer değiştir",
    "swapPlacesOn": "Yer değiştir (Açık)",
    "uploadImage": "Görsel yükle",
    "helpMeReply": "Yanıtlamamda yardım et",
    "sceneImage": {
      "modeTitle": "Sahne görseli",
      "modeDescription": "Sahneyi kendiniz mi yazmak yoksa yapay zekanın önce taslak hazırlamasını mı istediğinizi seçin.",
      "writePrompt": "Prompt yaz",
      "writePromptDesc": "Kullanmak istediğiniz sahne görseli promptunu tam olarak yazın.",
      "askAi": "Yapay zekaya sor",
      "askAiDesc": "Mevcut sohbet modelinin seçili andan bir sahne promptu taslağı hazırlamasına izin verin.",
      "generateTitle": "Sahne görseli oluştur",
      "regenerateTitle": "Sahne görselini yeniden oluştur",
      "aiTitle": "Yapay zeka sahne promptu",
      "promptLabel": "SAHNE PROMPTU",
      "promptPlaceholder": "Sahneyi, karakterleri, atmosferi, aydınlatmayı, kamera açısını ve önemli detayları tanımlayın...",
      "suggestedPrompt": "Önerilen prompt",
      "regeneratePrompt": "Yeniden oluşturuldu",
      "editPrompt": "Promptu düzenle",
      "reviewTitle": "Sahne promptunu incele",
      "denyPrompt": "Reddet",
      "acceptPrompt": "Kabul et",
      "generateImage": "Görsel oluştur",
      "updateImage": "Görseli güncelle"
    },
    "useMyTextAsBase": "Metnimi temel olarak kullan",
    "writeNewReply": "Yeni bir şey yaz",
    "suggestedReply": "Önerilen yanıt",
    "selectTwoCharactersMinimum": "Grup sohbeti için en az 2 karakter seç.",
    "groupBranchCreated": "Grup dalı oluşturuldu! Yönlendiriliyor...",
    "memories": "Anılar",
    "tools": "Araçlar",
    "pinned": "Sabitlenmiş",
    "searchMemories": "Anılarda ara...",
    "addMemory": "Anı ekle",
    "memoryActions": "Anı işlemleri",
    "pinnedMessages": "Sabitlenmiş mesajlar",
    "pinnedMessagesDesc": "Her zaman bağlama dahil edilir",
    "contextSummary": "Bağlam özeti",
    "contextSummaryPlaceholder": "Mesajlar arası bağlam tutarlılığını korumak için kısa özet...",
    "memoryContentPlaceholder": "Ne hatırlanmalı?",
    "editMemoryPlaceholder": "Anı içeriğini gir...",
    "togglePin": {
      "pin": "Sabitle",
      "unpin": "Sabitlemeyi kaldır"
    },
    "toggleMemoryState": {
      "setHot": "Aktif yap",
      "setCold": "Soğuk yap"
    },
    "selectModelForRetry": "Tekrar denemek için model seç",
    "memoryCategories": {
      "characterTrait": "karakter özelliği",
      "relationship": "ilişki",
      "plotEvent": "olay örgüsü olayı",
      "worldDetail": "dünya detayı",
      "preference": "tercih",
      "other": "diğer"
    },
    "settings": {
      "memorySection": "Hafıza",
      "memorySectionDesc": "Özet, etiketler, araç çağrı geçmişi",
      "quickSettings": "Hızlı ayarlar",
      "quickSettingsDesc": "En yaygın ayarlar",
      "persona": "Persona",
      "model": "Model",
      "fallbackModel": "Yedek model",
      "voice": "Ses",
      "voiceDesc": "Metinden konuşmaya oynatma",
      "advanced": "Gelişmiş",
      "advancedDesc": "Bu oturum için model parametrelerini geçersiz kıl",
      "session": "Oturum",
      "sessionDesc": "Yeni sohbetler başlat ve geçmişte gezin",
      "newChat": "Yeni sohbet",
      "newChatDesc": "Yeni bir konuşma başlat",
      "chatHistoryDesc": "Önceki oturumları görüntüle",
      "importChatPackageDesc": "Bu karaktere .chatpkg içe aktar",
      "selectModel": "Model seç",
      "selectFallbackModel": "Yedek model seç",
      "personaActions": "Persona işlemleri",
      "sessionAdvancedSettings": "Gelişmiş oturum ayarları",
      "parameterSupport": "Parametre desteği",
      "backToChat": "Sohbete geri dön",
      "chatSettingsTitle": "Sohbet Ayarları",
      "chatSettingsSubtitle": "Konuşma tercihlerini yönet",
      "modelDefaults": "Model varsayılanları_",
      "appDefaults": "Uygulama varsayılanları",
      "openChatSessionFirst": "Önce bir sohbet oturumu açın",
      "sessionRequired": "Oturum gerekli",
      "noPersona": "Kişi yok",
      "customPersona": "Özel kişi",
      "noModelAvailable": "Mevcut model yok",
      "fallbackNone": "Yok",
      "unknownModel": "Bilinmeyen model",
      "authorNote": "Yazar Notu",
      "identityProfileAuthored": "Kimlik profili yazılmıştır",
      "addIdentityProfile": "Tamamlayıcı kimlik profili ekle",
      "soulLabel": "Soul",
      "sessionTitle": "Oturum: {{title}}",
      "sessionUntitled": "Başlıksız",
      "messageCount": "{{count}} mesajlar",
      "usingCharacterDefault": "Karakter varsayılanı kullanılıyor",
      "sessionOverrideActive": "Oturumu geçersiz kılma etkin",
      "autoplayVoice": "Sesi otomatik oynat_",
      "useCharacterDefault": "Varsayılan karakter kullan"
    },
    "search": {
      "placeholder": "Konuşmada ara...",
      "noMessagesFound": "Mesaj bulunamadı",
      "you": "Sen",
      "character": "Karakter",
      "failed": "Mesaj araması başarısız"
    },
    "templates": {
      "startWithTemplate": "Şablonla başla?",
      "noTemplate": "Şablon yok",
      "startWithSceneOnly": "Yalnızca sahne ile başla",
      "you": "Sen",
      "bot": "Bot",
      "messageCount": "{{count}} mesaj"
    },
    "header": {
      "back": "Geri",
      "openSettings": "Sohbet ayarlarını aç",
      "manageMemories": "Anıları yönet",
      "searchMessages": "Mesajlarda ara",
      "manageLorebooks": "Lore kitaplarını yönet",
      "conversationSettings": "Konuşma ayarları"
    },
    "footer": {
      "sendMessagePlaceholder": "Bir mesaj gönder...",
      "stopGeneration": "Oluşturmayı durdur",
      "sendMessage": "Mesaj gönder",
      "continueConversation": "Konuşmaya devam et",
      "moreOptions": "Daha fazla seçenek",
      "addImage": "Görsel ekle",
      "addImageAttachment": "Görsel eki ekle",
      "removeAttachment": "Eki kaldır",
      "recordVoice": "Ses kaydet"
    },
    "message": {
      "thinkingMessages": {
        "thinkingReallyHard": "Çok yoğun düşünüyor…",
        "lettuceCouncil": "Marul konseyine danışıyor…",
        "stealingThoughts": "Boşluktan düşünceler çalıyor…",
        "warmingBrainCells": "Beyin hücrelerini ısıtıyor…",
        "forbiddenKnowledge": "Yasak bilgi yükleniyor…",
        "overthinking": "Gereğinden fazla düşünüyor (her zamanki gibi)…",
        "pretendingToBeSmart": "Akıllıymış gibi yapıyor…",
        "crunchingNumbers": "Hayali sayılar hesaplanıyor…",
        "arguingWithMyself": "Kendimle tartışıyorum…",
        "askingUniverse": "Evrene kibarca soruyor…"
      },
      "thoughtProcess": "Düşünce süreci",
      "regenerateResponse": "Yanıtı yeniden oluştur",
      "guidedRegenerationTitle": "Yenileme kılavuzu",
      "guidedRegenerationLabel": "Nasıl değişmeli?",
      "guidedRegenerationDescription": "Ton, uzunluk, saklanacak veya kaldırılacak ayrıntılar ve bir sonraki yanıtın farklı yapması gereken her şeyi açıklayın.",
      "guidedRegenerationPlaceholder": "Daha kısa, daha sıcak ve daha doğrudan yapın...",
      "guidedRegenerationSubmit": "Yeniden oluştur",
      "cancelAudioGeneration": "Ses oluşturmayı iptal et",
      "stopAudio": "Sesi durdur",
      "playMessageAudio": "Mesaj sesini çal",
      "playAudio": "Sesi çal",
      "sceneLabel": "Sahne",
      "variantLabel": "Varyant",
      "regenerating": "Yeniden oluşturuluyor",
      "assistantIsTyping": "Asistan yazıyor",
      "attachedImage": "Ekli görsel"
    },
    "actions": {
      "assistantMessage": "Asistan mesajı",
      "userMessage": "Kullanıcı mesajı",
      "promptTokens": "Prompt tokenleri",
      "completionTokens": "Tamamlama tokenleri",
      "fallbackModelUsed": "Yedek model kullanıldı",
      "total": "toplam",
      "timeToFirstToken": "İlk token süresi",
      "completionTokenSpeed": "Tamamlama token hızı",
      "edit": "Düzenle",
      "copy": "Kopyala",
      "pin": "Sabitle",
      "unpin": "Sabitlemeyi kaldır",
      "rewindToHere": "Buraya geri sar",
      "branchFromHere": "Buradan dallan",
      "branchToGroupChat": "Grup sohbetine dallan",
      "branchToCharacter": "Karaktere dallan",
      "generateSceneImage": "Sahne görseli oluştur",
      "regenerateSceneImage": "Sahne görselini yeniden oluştur",
      "chatAppearance": "Sohbet görünümü",
      "delete": "Sil",
      "debug": "Hata ayıkla",
      "unpinToDelete": "Silmek için sabitlemeyi kaldır",
      "editPlaceholder": "Mesajını düzenle...",
      "memoriesUsed": "{{count}} anı kullanıldı",
      "lorebookUsage": "Lore kitabı kullanımı",
      "lorebookUsageDesc": "Bu yanıt aşağıdaki lore kitabı girdilerini kullandı.",
      "matchScore": "Eşleşme: {{score}}%",
      "unknownModel": "Bilinmeyen model",
      "loadingModel": "Model yükleniyor..."
    },
    "emptyState": {
      "goBack": "Geri dön"
    },
    "settingsDrawer": {
      "title": "Sohbet ayarları",
      "subtitle": "Konuşma tercihlerini yönet"
    },
    "history": {
      "archivedBadge": "Arşivlendi",
      "messagesCount": "{{count}} mesajlar",
      "previousGroupPage": "Önceki {{label}} sayfası",
      "nextGroupPage": "Sonraki {{label}} sayfası",
      "sillyTavernFormat": "SillyTavern formatı",
      "sillyTavernFormatDesc": "SillyTavern ile uyumlu .jsonl olarak dışa aktar.",
      "failedLoad": "Veriler yüklenemedi",
      "failedDelete": "Silinemedi: {{error}}",
      "failedRename": "Yeniden adlandırılamadı: {{error}}",
      "chatPackageExportedTo": "Sohbet paketi şuraya aktarıldı:\n",
      "sillyTavernExportedTo": "SillyTavern sohbeti şuraya aktarıldı:\n",
      "failedExportChatPackage": "Sohbet paketi dışa aktarılamadı",
      "failedExportSillyTavern": "SillyTavern sohbeti dışa aktarılamadı"
    },
    "authorNote": {
      "title": "Yazar Notu",
      "inlineEditor": "Satır içi düzenleyici",
      "inlineEditorDesc": "Hızlı düzenlemeler için yazar notunu sohbet girişinin üzerinde gösterin.",
      "toggleInlineEditor": "Satır içi yazar notu düzenleyicisini değiştir",
      "placeholder": "Bu sohbet için özel yönlendirme",
      "willBeRemoved": "Yazar notu kaydedildiğinde kaldırılacak",
      "noNoteSaved": "Yazar notu kaydedilmedi",
      "charactersCount": "{{count}} karakter",
      "clear": "Clear",
      "save": "Kaydet",
      "saving": "Kaydediliyor..._",
      "failedSave": "Yazar notu kaydedilemedi",
      "addPlaceholder": "Yazar notu ekleyin...",
      "savingShort": "Kaydediliyor..."
    },
    "drawer": {
      "chatSettingsTitle": "Sohbet Ayarları",
      "chatSettingsSubtitle": "_Konuşma tercihlerini yönet_"
    },
    "companionSoul": {
      "loading": "_Companion Soul Yükleniyor..._",
      "unavailable": "_Companion Soul kullanılamıyor",
      "unavailableDesc": "Soul düzenleme yalnızca tamamlayıcı mod karakterleri için kullanılabilir.",
      "pageTitle": "Companion Soul",
      "back": "Back",
      "save": "Save_"
    },
    "companionRelationship": {
      "back": "Back",
      "loading": "İlişki durumu yükleniyor...",
      "unavailableTitle": "İlişki durumu kullanılamıyor",
      "sessionLoadFailed": "Sohbet oturumu yüklenemedi.",
      "backToChat": "Sohbete geri dön",
      "notCompanionTitle": "Bu sohbet tamamlayıcı modda değil",
      "notCompanionDesc": "Arkadaş ilişkisi sayfaları yalnızca karakter modu tamamlayıcı olan sohbetler için oluşturulur.",
      "openRegularMemories": "Normal anıları aç",
      "pageTitle": "İlişki durumu",
      "memoryButton": "Bellek",
      "lastInteraction": "Son etkileşim {{time}}",
      "bond": "Bağ",
      "closeness": "Yakınlık",
      "trust": "Güven",
      "affection": "Sevgi",
      "tension": "Gerilim",
      "stability": "İstikrar",
      "interactions": "Etkileşimler",
      "vsDefaults": "ve karakter varsayılanları",
      "updatedAt": "Güncellendi {{time}}",
      "emotionalEngine": "Duygusal motor",
      "felt": "Keçe",
      "feltDesc": "Dahili etki",
      "expressed": "İfade edildi",
      "expressedDesc": "Yanıtlardaki yüzeyler",
      "blocked": "Engellendi",
      "blockedDesc": "Kişi tarafından bastırıldı",
      "momentum": "Momentum",
      "momentumDesc": "Son dönüşlerdeki eğilim",
      "activeDrivers": "Aktif sürücüler",
      "soul": "Soul_",
      "essence": "_Essence",
      "voice": "Voice",
      "relationalStyle": "İlişkisel stil",
      "vulnerabilities": "Güvenlik açıkları",
      "habits": "Alışkanlıklar",
      "boundaries": "Sınırlar",
      "eventsCount": "{{count}} olaylar",
      "recentTimeline": "Son zaman çizelgesi",
      "superseded": "süperseded",
      "promptScore": "İstem {{score}}",
      "persistenceScore": "Kalıcılık {{score}}",
      "noTimeline": "Henüz zaman çizelgesi yok",
      "noTimelineDesc": "Refakatçi konuşmalardan öğrenirken ilişki, dönüm noktası ve duygusal anlık görüntü anıları burada görünecek.",
      "notAuthoredYet": "Henüz yazılmadı.",
      "noSignal": "Sinyal yok."
    },
    "companionUi": {
      "relationship": "İlişki",
      "milestones": "Dönüm Noktaları_",
      "boundaries": "Sınırlar",
      "preferences": "Tercihler",
      "profile": "Profil",
      "routines": "Rutinler",
      "episodes": "Bölümler",
      "emotionalSnapshots": "_Duygusal anlık görüntüler_",
      "unknownTime": "_Bilinmeyen_",
      "justNow": "_Az önce",
      "minutesAgo": "{{minutes}}dk önce",
      "hoursAgo": "{{hours}}h önce",
      "daysAgo": "{{days}}d önce",
      "notSetYet": "_Henüz ayarlanmadı_",
      "missingCharacterId": "_Eksik karakterId_",
      "sessionNotFound": "_Oturum bulunamadı",
      "failedLoadCompanion": "Tamamlayıcı oturum yüklenemedi"
    },
    "chatPage": {
      "characterNotFound": "Karakter bulunamadı",
      "characterDoesntExist": "Aradığınız karakter mevcut değil."
    },
    "debugPage": {
      "copy": "Copy_"
    },
    "companionMemoryPage": {
      "backLabel": "_Back",
      "refineMemoryPlaceholder": "_Saklananı hassaslaştırın ",
      "superseded": "değiştirildi",
      "pinned": "pinned",
      "cold": "soğuk"
    }
  },
  "groupChats": {
    "list": {
      "editGroup": "Grubu düzenle",
      "deleteGroup": "Grubu sil",
      "deleteConfirmTitle": "Grup sohbeti silinsin mi?",
      "deleteConfirmMessage": "\"{{name}}\" silmek istediğinden emin misin? Bu, bu grup sohbetindeki tüm mesajları da silecek.",
      "noGroupChatsYet": "Henüz grup sohbeti yok",
      "noGroupChatsDesc": "Aynı anda birden fazla karakterle konuşmak için aşağıdaki + butonundan ilk grup sohbetini oluştur",
      "newChat": "Yeni sohbet",
      "openChat": "Sohbeti aç",
      "chatSettings": "Sohbet ayarları",
      "sessionCount": "{{count}} konuşma"
    },
    "create": {
      "invalidPackage": "Bu paket bir grup sohbet paketi değil.",
      "inspectPackageError": "Grup sohbet paketi incelenemedi",
      "importPackageError": "Grup sohbet paketi içe aktarılamadı",
      "importChatpkg": "`.chatpkg` içe aktar",
      "mapParticipantsTitle": "Katılımcıları eşle",
      "selectLocalCharacter": "Bu katılımcı için yerel karakteri seç.",
      "selectCharacterPlaceholder": "Karakter seç...",
      "importChatPackageTitle": "Sohbet paketi içe aktar",
      "importChatPackageDesc": "Bu, seçilen `.chatpkg` dosyasını yeni bir grup oturumu olarak içe aktaracak.",
      "characterSelect": {
        "title": "Grup sohbeti oluştur",
        "subtitle": "Grup konuşman için karakterleri seç",
        "selected": "seçili",
        "ready": "Hazır",
        "minRequired": "Min. 2 gerekli",
        "noCharactersYet": "Henüz karakter yok",
        "noCharactersDesc": "Grup sohbeti başlatmak için önce birkaç karakter oluştur",
        "continueToSetup": "Grup kurulumuna devam et"
      },
      "groupSetup": {
        "title": "Grup kurulumu",
        "subtitle": "Grup sohbeti ayarlarını yapılandır",
        "chatType": "Sohbet türü",
        "conversation": "Konuşma",
        "casualChat": "Sıradan sohbet",
        "roleplay": "Rol yapma",
        "withScenes": "Sahneli",
        "conversationDesc": "Başlangıç sahnesi olmayan sıradan grup konuşması",
        "roleplayDesc": "Başlangıç sahnesi ve sürükleyici promptlarla roleplay senaryosu",
        "speakerSelection": "Konuşmacı seçimi",
        "llm": "LLM",
        "aiPicks": "Yapay zeka seçer",
        "heuristic": "Sezgisel",
        "scoreBased": "Puana dayalı",
        "roundRobin": "Sırayla",
        "takeTurns": "Sıra ile",
        "llmDesc": "Kimin konuşacağını seçmek için varsayılan modelini kullanır (token harcar)",
        "heuristicDesc": "Katılım dengesi ve bağlam ipuçlarını kullanır (ücretsiz)",
        "roundRobinDesc": "Karakterler sırayla konuşur (ücretsiz)",
        "chatBackground": "Sohbet arka planı",
        "optional": "(İsteğe bağlı)",
        "uploadBackground": "Arka plan görseli yükle",
        "backgroundDesc": "Bu grup sohbeti için arka plan görseli ayarla",
        "groupName": "Grup adı",
        "removeBackground": "Arka plan görselini kaldır",
        "groupNameAutoGenerate": "Karakter adlarından otomatik oluşturmak için boş bırak",
        "continueToScene": "Başlangıç sahnesine devam et",
        "createGroupChat": "Grup sohbeti oluştur"
      },
      "startingScene": {
        "title": "Başlangıç sahnesi",
        "subtitle": "Roleplayın için açılış sahnesini belirle",
        "sceneSource": "Sahne kaynağı",
        "none": "Yok",
        "custom": "Özel",
        "fromCharacter": "Karakterden",
        "noneDesc": "Önceden tanımlanmış bir sahne olmadan başla",
        "customDesc": "Kendi açılış senaryonu yaz",
        "fromCharacterDesc": "Karakterlerinden birinin sahnesini kullan",
        "sceneContent": "Sahne içeriği",
        "sceneContentPlaceholder": "Bu roleplay için başlangıç sahnesini tanımla...",
        "sceneReferenceTip": "İpucu: Karakterlere referans vermek için {{@\" yaz",
        "selectScene": "Sahne seç",
        "sceneLabel": " — Sahne",
        "copyToCustom": "Özel olarak kopyala ve düzenle"
      }
    },
    "history": {
      "title": "Grup sohbet geçmişi",
      "subtitle": "Tüm grup konuşmaları",
      "searchPlaceholder": "Grup sohbetlerinde ara...",
      "active": "Aktif ({{count}})",
      "archived": "Arşivlenmiş ({{count}})",
      "noChatsYet": "Henüz grup sohbeti yok",
      "noChatsDesc": "Geçmişini burada görmek için bir grup sohbeti oluştur",
      "noMatchingChats": "Eşleşen sohbet yok",
      "noMatchingDesc": "Başka bir arama terimi dene",
      "deleteSessionTitle": "Grup sohbeti silinsin mi?",
      "deleteSessionDesc": "Geçmişten kalıcı olarak silinecek",
      "deleteSessionButton": "Sohbeti sil",
      "keepChat": "Bu sohbeti koru"
    },
    "session": {
      "chatTitlePlaceholder": "Sohbet başlığı...",
      "newChat": "Yeni sohbet",
      "rename": "Yeniden adlandır",
      "unarchive": "Arşivden çıkar",
      "archive": "Arşivle",
      "messageCount": "{{count}} mesaj"
    },
    "memories": {
      "tabMemories": "Anılar",
      "tabPinned": "Sabitlenmiş",
      "tabActivity": "Etkinlik",
      "processing": "İşleniyor",
      "contextSummaryTitle": "Bağlam özeti",
      "addContextSummaryPrompt": "Bağlam özeti eklemek için dokun...",
      "savedMemories": "Kayıtlı anılar",
      "resultsCount": "Sonuçlar ({{count}})",
      "searchPlaceholder": "Anılarda ara...",
      "addMemory": "Anı ekle",
      "noMemoriesYet": "Henüz anı yok",
      "noMemoriesDesc": "Bir tane oluşturmak için yukarıdaki Ekle butonuna dokun",
      "noMatchingMemories": "Eşleşen anı yok",
      "noMatchingDesc": "Başka bir arama terimi dene",
      "sessionNotFound": "Oturum bulunamadı",
      "memoryActions": "Anı işlemleri",
      "tokens": "token",
      "cycle": "Döngü",
      "accessed": "Erişildi",
      "cold": "Soğuk",
      "hot": "Aktif",
      "activityLog": "Etkinlik günlüğü",
      "events": "olay",
      "run": "Çalıştırma",
      "processingMemories": "Yapay zeka grubun anılarını düzenliyor...",
      "memoryCycleSuccess": "Hafıza döngüsü başarıyla işlendi!",
      "memoryActionFailed": "Anı işlemi başarısız",
      "newMemoryUpdates": "Yeni anı güncellemeleri mevcut",
      "noActivityYet": "Henüz etkinlik yok",
      "noActivityDesc": "Araç çağrıları, yapay zeka dinamik modda anıları yönettiğinde görünür",
      "contextSummaryPlaceholder": "Mesajlar arası bağlam tutarlılığını korumak için kısa özet...",
      "addMemoryTitle": "Anı ekle",
      "memoryPlaceholder": "Ne hatırlanmalı?",
      "saveMemory": "Anıyı kaydet",
      "editMemoryTitle": "Anıyı düzenle",
      "editMemoryPlaceholder": "Anı içeriğini gir...",
      "edit": "Düzenle",
      "pin": "Sabitle",
      "unpin": "Sabitlemeyi kaldır",
      "setHot": "Aktif yap",
      "setCold": "Soğuk yap"
    },
    "toolLog": {
      "created": "Oluşturuldu",
      "deleted": "Silindi",
      "pinned": "Sabitlendi",
      "unpinned": "Sabitleme kaldırıldı",
      "done": "Tamamlandı",
      "pinnedBadge": "sabitlenmiş",
      "softDelete": "yumuşak silme",
      "memoryCycle": "Hafıza döngüsü",
      "failedAt": "Başarısız olduğu yer:",
      "window": "Pencere",
      "justNow": "az önce",
      "minutesAgo": "{{count}}dk. önce",
      "hoursAgo": "{{count}}h önce",
      "yesterday": "dün",
      "daysAgo": "{{count}}d önce",
      "revertingTitle": "Geri döndürülüyor..._",
      "revertCycleTitle": "_Bu döngüyü geri döndür_",
      "revertedAt": "_Geri döndürüldü ",
      "failedAtStage": "Başarısız oldu: {{stage}}",
      "hideDebug": "Hata Ayıklamayı Gizle",
      "debug": "Hata Ayıklama",
      "windowRange": "Pencere {{start}}-{{end}}",
      "actionCreated": "Oluşturuldu",
      "actionDeleted": "Silindi",
      "actionPinned": "Sabitlendi_",
      "actionUnpinned": "Sabitlenmemiş",
      "actionDone": "Bitti",
      "badgePinned": "_sabitlendi_",
      "badgeSoftDelete": "_soft-delete_",
      "badgeUndone": "_geri alındı",
      "badgeReverted": "geri alındı",
      "activityEmptyTitle": "Henüz etkinlik yok",
      "activityEmptyDesc": "Yapay zeka anıları dinamik modda yönettiğinde araç çağrıları görünüyor"
    },
    "message": {
      "thinkingHard": "Çok yoğun düşünüyor…",
      "thinkingLettuce": "Marul konseyine danışıyor…",
      "thinkingVoid": "Boşluktan düşünceler çalıyor…",
      "thinkingBrainCells": "Beyin hücrelerini ısıtıyor…",
      "thinkingForbidden": "Yasak bilgi yükleniyor…",
      "thinkingOverthinking": "Gereğinden fazla düşünüyor (her zamanki gibi)…",
      "thinkingPretending": "Akıllıymış gibi yapıyor…",
      "thinkingCrunching": "Hayali sayılar hesaplanıyor…",
      "thinkingArguing": "Kendimle tartışıyorum…",
      "thinkingUniverse": "Evrene kibarca soruyor…",
      "thoughtProcess": "Düşünce süreci",
      "userAlt": "Kullanıcı",
      "assistantAlt": "Asistan",
      "regenerateResponse": "Yanıtı yeniden oluştur",
      "variantLabel": "Varyant",
      "regenerating": "Yeniden oluşturuluyor",
      "cancelAudioGeneration": "Ses oluşturmayı iptal et",
      "stopAudio": "Sesi durdur",
      "playMessageAudio": "Mesaj sesini çal",
      "playAudio": "Sesi çal",
      "attachedImage": "Ekli görsel",
      "assistantIsTyping": "Asistan yazıyor",
      "assistantTyping": "Asistan yazıyor"
    },
    "header": {
      "back": "Geri",
      "memories": "Anılar",
      "settings": "Ayarlar",
      "characters": "karakter"
    },
    "footer": {
      "mentionCharacter": "Karakter bahset",
      "noCharactersFound": "Karakter bulunamadı",
      "moreOptions": "Daha fazla seçenek",
      "addImage": "Görsel ekle",
      "messagePlaceholder": "Mesaj... (bahsetmek için @)",
      "stopGeneration": "Oluşturmayı durdur",
      "sendMessage": "Mesaj gönder",
      "continueConversation": "Konuşmaya devam et",
      "dismissError": "Hatayı kapat",
      "removeAttachment": "Eki kaldır",
      "tabToSelect": "Seçmek için Tab"
    },
    "messageActions": {
      "characterMessage": "Karakter mesajı",
      "yourMessage": "Senin mesajın",
      "whyCharacterResponded": "Bu karakter neden yanıt verdi",
      "total": "toplam",
      "edit": "Düzenle",
      "copy": "Kopyala",
      "regenerateWithDifferent": "Farklı karakterle yeniden oluştur",
      "rewindToHere": "Buraya geri sar",
      "unpinToDelete": "Silmek için sabitlemeyi kaldır",
      "delete": "Sil",
      "editPlaceholder": "Mesajını düzenle...",
      "chooseCharacterTitle": "Karakter seç",
      "selectCharacterForRegeneration": "Bunun yerine hangi karakter yanıt vermeli seç:"
    },
    "settings": {
      "appDefault": "Uygulama varsayılanı",
      "selectPersona": "Persona seç",
      "noPersonas": "Kullanılabilir persona yok",
      "noPersonasDesc": "Konuşmalarını kişiselleştirmek için ayarlarda bir persona oluştur.",
      "searchPersonas": "Persona ara...",
      "noPersona": "Persona yok",
      "noPersonaDesc": "Persona olmadan devam et",
      "noPersonasFound": "Persona bulunamadı",
      "noPersonasFoundDesc": "Başka bir arama terimi dene"
    },
    "groupSettings": {
      "title": "Grubu düzenle",
      "subtitle": "Gelecek oturumlar için varsayılan yapılandırmayı güncelle",
      "enterGroupName": "Grup adı",
      "participant": "katılımcı",
      "participants": "katılımcı",
      "uploading": "Yükleniyor...",
      "changeBackground": "Arka planı değiştir",
      "addBackgroundImage": "Arka plan görseli ekle",
      "removeBackground": "Arka planı kaldır",
      "persona": "_Persona_",
      "personaSubtitle": "Yeni oturumlar için varsayılan persona",
      "personaLabel": "Persona",
      "speakerSelection": "Konuşmacı seçimi",
      "speakerSubtitle": "Yeni oturumlar için varsayılan yöntem",
      "llm": "LLM",
      "aiPicks": "Yapay zeka seçer",
      "heuristic": "Sezgisel",
      "scoreBased": "Puana dayalı",
      "roundRobin": "Sırayla",
      "takeTurns": "Sıra ile",
      "llmDesc": "Kimin konuşacağını seçmek için varsayılan modelini kullanır (token harcar)",
      "heuristicDesc": "Katılım dengesi ve bağlam ipuçlarını kullanır (ücretsiz)",
      "roundRobinDesc": "Karakterler sırayla konuşur (ücretsiz)",
      "memoryMode": "Hafıza modu",
      "memorySubtitle": "Yeni oturumlar için varsayılan hafıza modu",
      "manual": "Manuel",
      "manualDesc": "Notları kendin yönet",
      "dynamic": "Dinamik",
      "dynamicDesc": "Otomatik geri çağırma",
      "memoryDynamicInfo": "Yapay zeka konuşmalardan otomatik olarak anıları oluşturur ve geri çağırır",
      "memoryManualInfo": "Hafıza notlarını sen ekler ve yönetirsin",
      "characters": "Karakterler",
      "participantsActive": "{{total}} katılımcı · {{active}} aktif",
      "add": "Ekle",
      "muted": "(sessize alınmış)",
      "mutedByDefault": "Varsayılan olarak sessize alınmış",
      "activeByDefault": "Varsayılan olarak aktif",
      "unmuteCharacter": "Karakterin sesini aç",
      "muteCharacter": "Karakteri sessize al",
      "minTwoRequired": "En az 2 karakter gerekli",
      "removeCharacter": "Karakteri kaldır",
      "groupMinCharacters": "Bir grup en az 2 karakter gerektirir",
      "mutedCharactersNote": "Sessize alınan karakterler otomatik konuşmacı seçiminde atlanır, ancak açık `@bahsetme` ile yine de yanıt verebilirler.",
      "addCharacterTitle": "Karakter ekle",
      "allCharactersInGroup": "Tüm karakterler zaten bu grupta.",
      "removeCharacterTitle": "Karakter kaldırılsın mı?",
      "removeCharacterConfirm": "Şunu kaldırmak istediğinden emin misin:",
      "removeCharacterFrom": "grup varsayılanlarından?",
      "removing": "Kaldırılıyor...",
      "remove": "Kaldır"
    },
    "sessionSettings": {
      "subtitle": "Grup sohbeti tercihlerini yönet",
      "enterGroupName": "Grup adı",
      "participant": "katılımcı",
      "participants": "katılımcı",
      "message": "mesaj",
      "messages": "mesaj",
      "uploading": "Yükleniyor...",
      "changeBackground": "Arka planı değiştir",
      "addBackgroundImage": "Arka plan görseli ekle",
      "removeBackground": "Arka planı kaldır",
      "persona": "Persona",
      "personaSubtitle": "Bu konuşmadaki kimliğin",
      "personaLabel": "Persona",
      "speakerSelection": "Konuşmacı seçimi",
      "speakerSubtitle": "Sonraki konuşmacının nasıl seçileceği",
      "llm": "LLM",
      "aiPicks": "Yapay zeka seçer",
      "heuristic": "Sezgisel",
      "scoreBased": "Puana dayalı",
      "roundRobin": "Sırayla",
      "takeTurns": "Sıra ile",
      "llmDesc": "Kimin konuşacağını seçmek için varsayılan modelini kullanır (token harcar)",
      "heuristicDesc": "Katılım dengesi ve bağlam ipuçlarını kullanır (ücretsiz)",
      "roundRobinDesc": "Karakterler sırayla konuşur (ücretsiz)",
      "characters": "Karakterler",
      "participantsActive": "{{total}} katılımcı · {{active}} aktif",
      "add": "Ekle",
      "muted": "(sessize alınmış)",
      "unmuteCharacter": "Karakterin sesini aç",
      "muteCharacter": "Karakteri sessize al",
      "minTwoRequired": "En az 2 karakter gerekli",
      "removeCharacter": "Karakteri kaldır",
      "groupMinCharacters": "Bir grup sohbeti en az 2 karakter gerektirir",
      "mutedCharactersNote": "Sessize alınan karakterler otomatik konuşmacı seçiminde atlanır, ancak açık `@bahsetme` ile yine de yanıt verebilirler.",
      "data": "Veri",
      "dataSubtitle": "Konuşmaları dışa veya içe aktar",
      "export": "Dışa aktar",
      "exportDesc": "Paylaşılabilir dosya olarak kaydet",
      "import": "İçe aktar",
      "importDesc": "Dosyadan konuşma yükle",
      "conversation": "Konuşma",
      "conversationSubtitle": "Çoğalt veya yeni sohbette devam et",
      "duplicate": "Çoğalt",
      "duplicateDesc": "Bu sohbeti mesajlarla veya mesajsız kopyala",
      "branchTo1on1": "1'e 1'e dallan",
      "branchTo1on1Desc": "Bir karakterle özel olarak devam et",
      "participation": "Katılım",
      "participationSubtitle": "Karakterler arası konuşma dağılımı",
      "addCharacterTitle": "Karakter ekle",
      "allCharactersInGroup": "Tüm karakterler zaten bu grupta.",
      "removeCharacterTitle": "Karakter kaldırılsın mı?",
      "removeCharacterConfirm": "Şunu kaldırmak istediğinden emin misin:",
      "removeCharacterFrom": "bu grup sohbetinden?",
      "removing": "Kaldırılıyor...",
      "remove": "Kaldır",
      "cloneGroupTitle": "Grubu klonla",
      "withMessages": "Mesajlarla",
      "withMessagesDesc": "Sohbet geçmişi dahil her şeyi klonla",
      "withoutMessages": "Mesajsız",
      "withoutMessagesDesc": "Yalnızca yapılandırmayı klonla (karakterler, başlangıç sahnesi)",
      "branchWithCharacterTitle": "Karakterle dallan",
      "branchWithCharacterDesc": "1'e 1 konuşma olarak devam etmek için bir karakter seç. Bu gruptaki tüm mesajlar dönüştürülecek.",
      "continueWith": "{{name}} ile konuşmaya devam et",
      "exportChatPackageTitle": "Sohbet paketini dışa aktar",
      "includeCharacterSnapshots": "Karakter anlık görüntülerini dahil et",
      "includeCharacterSnapshotsDesc": "Karakter verilerini paketin içinde tut",
      "sessionOnly": "Yalnızca oturum",
      "sessionOnlyDesc": "Yalnızca mesajları ve meta verileri dışa aktar",
      "mapParticipantsTitle": "Katılımcıları eşle",
      "selectLocalCharacter": "Bu katılımcı için yerel karakteri seç.",
      "selectCharacterPlaceholder": "Karakter seç...",
      "continue": "Devam et",
      "importChatPackageTitle": "Sohbet paketi içe aktar",
      "importChatPackageDesc": "Bu, seçilen `.chatpkg` dosyasını yeni bir grup oturumu olarak içe aktaracak.",
      "importing": "İçe aktarılıyor..."
    },
    "page": {
      "selectingCharacter": "Karakter seçiliyor...",
      "sessionNotFound": "Grup oturumu bulunamadı_",
      "backToGroupChats": "Grup Sohbetlerine Geri Dön_",
      "startConversation": "_{{names}} ile görüşme başlat",
      "scrollToBottom": "Aşağıya doğru kaydırın",
      "addContent": "İçerik Ekle",
      "uploadImage": "Resim Yükle",
      "helpMeReply": "_Yanıtlamama Yardım Edin_",
      "helpMeReplyDesc": "_Yapay zekanın ne söyleyeceğimi önermesine izin verin_",
      "haveDraftPrompt": "_Taslak mesajınız var. ",
      "useMyTextAsBase": "Metnimi temel olarak kullan",
      "useMyTextAsBaseDesc": "Taslağınızı genişletin ve geliştirin",
      "writeSomethingNew": "Yeni bir şeyler yazın",
      "writeSomethingNewDesc": "Yeni bir yanıt oluşturun",
      "suggestedReply": "_Önerilen Yanıt",
      "reasoningBeforeWriting": "Akıl yürütme ",
      "writingYourReply": "Yanıtınız yazılıyor...",
      "regenerate": "Yeniden oluştur",
      "useReply": "Yanıtı Kullan",
      "helpMeReplyFailedGeneric": "Yanıtlamama Yardım Et başarısız oldu.",
      "helpMeReplyFailedGenerate": "_Bana Yardım Et Yanıt oluşturulamadı.",
      "noAudioCaptured": "Hayır ",
      "noWhisperModel": "Kurulu Whisper modeli bulunamadı. ",
      "cancelRecording": "Kaydı iptal et_",
      "transcribing": "Transkripsiyon yapılıyor",
      "stopAndTranscribe": "Durdurun ve yazıya aktarın",
      "recordVoice": "Sesi kaydedin",
      "learnCorrection": "Düzeltmeyi öğrenin:",
      "learning": "Öğreniliyor...",
      "learn": "Öğren",
      "ignore": "Yoksay",
      "groupChatFailed": "Grup sohbeti başarısız oldu.",
      "failedToCopy": "Kopyalama başarısız oldu_",
      "copied": "Kopyalandı!",
      "cannotDeletePinned": "Sabitlenmiş mesaj silinemiyor. ",
      "failedToDelete": "Silinemedi",
      "messageNotFound": "Mesaj bulunamadı",
      "cannotRewindPinned": "Geri alınamıyor: Bu noktadan sonra sabitlenmiş mesajlar var. ",
      "failedToRewind": "Geri sarma başarısız oldu",
      "failedToTogglePin": "Pin değiştirilemedi",
      "messagePinned": "Mesaj sabitlendi",
      "messageUnpinned": "Mesajın sabitlemesi kaldırıldı",
      "failedToSave": "Kaydedilemedi"
    },
    "memoriesPage": {
      "summarizingConversation": "_Konuşmayı özetleme_",
      "analyzingMemories": "Anılar analiz ediliyor",
      "applyingChanges": "_Değişiklikleri uygulama",
      "organizingMemories": "Anılar düzenleniyor",
      "retryingMemoryCycle": "Bellek Döngüsü Yeniden Deniyor...",
      "processingMemories": "Anılar işleniyor...",
      "memorySystemError": "Bellek Sistemi Hatası_",
      "contextSummary": "_İçerik Özeti_",
      "contextSummaryTitle": "_İçerik Özeti",
      "savedMemories": "Kaydedilen Anılar_",
      "resultsCount": "Sonuçlar ({{count}})",
      "searchMemoriesPlaceholder": "Anılarda ara...",
      "addMemory": "_Bellek ekle_",
      "memoryActions": "_Bellek eylemleri",
      "clearSearch": "_Aramayı temizle",
      "noMatchingMemories": "Eşleşen anı yok",
      "noMemoriesYet": "Henüz anı yok",
      "tryDifferentSearch": "Farklı bir arama terimi deneyin",
      "tapAddToCreate": "Bir tane oluşturmak için yukarıdaki Ekle düğmesine dokunun",
      "pinnedMessages": "Sabitlenmiş Mesajlar",
      "refresh": "Yenile",
      "noPinnedMessages": "Sabitlenmiş mesaj yok",
      "pinImportantDesc": "Önemli grup sohbeti mesajlarını her zaman bağlamda tutmak için sabitleyin.",
      "assistant": "Assistant",
      "user": "Kullanıcı_",
      "unpin": "_Sabitlemeyi kaldır_",
      "failedToUnpinMessage": "Mesajın sabitlemesi kaldırılamadı",
      "activityLog": "Etkinlik Günlüğü",
      "run": "Çalıştır",
      "addMemoryTitle": "Bellek Ekle",
      "editMemoryTitle": "Belleği Düzenle_",
      "memoryTitle": "Bellek_",
      "memoryPlaceholder": "_Ne hatırlanmalıdır?",
      "saveMemory": "Belleği Kaydet",
      "editMemoryPlaceholder": "Bellek içeriğini girin...",
      "saving": "Kaydediliyor...",
      "save": "Kaydet_",
      "cancel": "İptal_",
      "contextSummaryPlaceholder": "Mesajlar arasında bağlamın tutarlı olmasını sağlamak için kullanılan kısa özet...",
      "contextSummaryPrompt": "Bağlam özeti eklemek için dokunun...",
      "cycle": "Döngü",
      "accessed": "Erişildi",
      "cold": "Soğuk_",
      "hot": "Sıcak_",
      "tokens": "tokens",
      "pin": "Pin",
      "setHot": "Sıcak Ayarla",
      "setCold": "Soğuk Ayarla",
      "edit": "Edit_",
      "delete": "Delete_",
      "failedToToggleMemPin": "Pin değiştirilemedi",
      "failedToRemoveMemory": "Bellek kaldırılamadı",
      "toolEventsCountAria": "events",
      "activityEmptyDesc": "Yapay zeka anıları dinamik modda yönettiğinde araç çağrıları görünüyor",
      "memoryCycleSuccess": "Bellek döngüsü başarıyla işlendi!"
    },
    "messageActionsExtra": {
      "characterMessage": "Karakter Mesajı",
      "yourMessage": "Mesajınız",
      "whyCharacterResponded": "Bu karakter neden yanıt verdi_",
      "promptTokensTitle": "İstem Belirteçleri",
      "completionTokensTitle": "Tamamlama Belirteçleri",
      "total": "toplam_",
      "regenerateWithDifferent": "Farklı karakterlerle yeniden oluştur_",
      "unpin": "_Sabitlemeyi kaldır",
      "pin": "Pin",
      "rewindToHere": "Buraya geri sar",
      "unpinToDelete": "Silmek için sabitlemeyi kaldırın",
      "editPlaceholder": "Mesajınızı düzenleyin..._",
      "chooseCharacter": "Karakter Seçin",
      "selectCharacterForRegeneration": "Bunun yerine hangi karakterin yanıt vermesi gerektiğini seçin:"
    },
    "timeAgo": {
      "justNow": "Az önce",
      "minutesAgo": "{{count}}dk önce",
      "hoursAgo": "{{count}}h önce",
      "daysAgo": "{{count}}d önce_"
    },
    "memoriesController": {
      "missingSessionId": "_Eksik oturumId",
      "sessionNotFound": "_Oturum bulunamadı",
      "failedToLoadSession": "Oturum yüklenemedi",
      "failedToUpdateTemperature": "Bellek sıcaklığı güncellenemedi",
      "failedToSaveSummary": "Özet kaydedilemedi",
      "cycleFailed": "Grup bellek döngüsü başarısız oldu_",
      "failedToAddMemory": "Bellek eklenemedi_",
      "failedToUpdateMemory": "Bellek güncellenemedi",
      "failedToRunCycle": "Bellek döngüsü çalıştırılamadı",
      "failedToRevertCycle": "Döngü geri alınamadı",
      "failedToRefresh": "Yenileme başarısız oldu",
      "failedToDismissError": "Hata giderilemedi",
      "failedToTogglePinned": "Sabitlenmiş mesaj değiştirilemedi",
      "sessionNotLoaded": "Oturum yüklenmedi",
      "revertCycleTitle": "Bu döngüyü geri almak mı istiyorsunuz?",
      "revertConfirm": "Geri döndürme"
    },
    "chatController": {
      "sessionNotFound": "Grup oturumu bulunamadı",
      "failedToLoadGroupChat": "Grup sohbeti yüklenemedi_",
      "requestFailed": "_Grup sohbet isteği başarısız oldu_",
      "failedToSendMessage": "_Mesaj gönderilemedi",
      "failedToRegenerate": "Yeniden oluşturulamadı",
      "failedToContinue": "Devam edilemedi",
      "failedToCopy": "Kopyalanamadı",
      "cannotDeletePinned": "Sabitlenmiş mesaj silinemiyor. ",
      "failedToDelete": "Silinemedi",
      "messageNotFound": "Mesaj bulunamadı",
      "cannotRewindPinned": "Geri alınamıyor: Bu noktadan sonra sabitlenmiş mesajlar var. ",
      "failedToRewind": "Geri sarma başarısız oldu",
      "failedToTogglePin": "Pin değiştirilemedi",
      "messagePinned": "Mesaj sabitlendi",
      "messageUnpinned": "Mesajın sabitlemesi kaldırıldı",
      "failedToSave": "Kaydedilemedi",
      "copied": "Kopyalandı!"
    },
    "historyController": {
      "failedToLoadData": "Veriler yüklenemedi",
      "failedToDelete": "Silinemedi: {{error}}",
      "failedToRename": "Yeniden adlandırılamadı: {{error}}",
      "failedToArchive": "Arşivlenemedi: {{error}}",
      "failedToUnarchive": "Arşivden çıkarılamadı: {{error}}",
      "failedToDuplicate": "Kopyalama başarısız oldu_"
    },
    "sessionSettingsController": {
      "failedToLoad": "Grup sohbeti ayarları yüklenemedi",
      "noPersona": "Kişi yok",
      "customPersona": "_Özel kişi_",
      "minOneActive": "En az bir katılımcı aktif kalmalıdır_"
    },
    "groupSettingsController": {
      "notFound": "_Grup ",
      "failedToLoad": "Grup ayarları yüklenemedi"
    },
    "createForm": {
      "failedToLoadCharacters": "Karakterler yüklenemedi",
      "selectAtLeastTwo": "Lütfen grup sohbeti için en az 2 karakter seçin",
      "failedToCreate": "Grup sohbeti oluşturulamadı"
    },
    "groupSetupExtra": {
      "memoryMode": "Bellek Modu",
      "manual": "Manuel",
      "manualDesc": "Notları kendiniz yönetin",
      "dynamic": "Dynamic",
      "dynamicDesc": "Otomatik özetler ve geri çağırma",
      "memoryManualInfo": "Anı notlarını kendiniz ekler ve yönetirsiniz",
      "memoryDynamicInfo": "AI, konuşmalardan anıları otomatik olarak oluşturur ve alır",
      "backgroundPreviewAlt": "Arka plan önizlemesi"
    },
    "createExtra": {
      "enterGroupNamePlaceholder": "Grup adını girin...",
      "unknown": "Bilinmeyen"
    },
    "startingSceneExtra": {
      "insertAs": "{{snippet}}"
    },
    "sessionCardExtra": {
      "unknown": "Bilinmeyen",
      "untitledChat": "Başlıksız Sohbet"
    },
    "sessionListExtra": {
      "unknown": "_Bilinmiyor"
    },
    "chatHeaderExtra": {
      "unknownError": "Bilinmeyen hata"
    },
    "chatSettingsExtra": {
      "invalidPackage": "Bu paket bir grup sohbet paketi değil.",
      "failedExport": "Grup sohbet paketi dışa aktarılamadı",
      "failedInspect": "Grup sohbet paketi incelenemedi",
      "failedImport": "Grup sohbet paketi içe aktarılamadı",
      "exportSuccess": "Grup sohbet paketi şuraya aktarıldı:\n",
      "backgroundAlt": "Arka plan",
      "removeBackgroundAria": "Arka planı kaldır",
      "backAria": "Geri",
      "backToGroupChats": "Grup Sohbetlerine Geri Dön"
    },
    "groupSettingsExtra": {
      "backToGroups": "Gruplara Geri Dön",
      "backAria": "Geri",
      "removeBackgroundAria": "Arka planı kaldır"
    },
    "historyPage": {
      "backAria": "Grup sohbetlerine geri dön",
      "clearSearchAria": "Aramayı temizle",
      "resultsLabel": "{{count}} sonuç_",
      "resultsLabelPlural": "{{count}} sonuçlar",
      "untitledChat": "_Başlıksız Sohbet",
      "noPinnedMessages": "Sabitlenmiş mesaj yok"
    },
    "personaSelectorExtra": {
      "insertAs": "Farklı ekle",
      "appDefault": "Uygulama varsayılanı",
      "defaultBadge": "_Default_",
      "selectPersonaTitle": "_Kişi Seç",
      "noPersonaTitle": "_Kişi Yok",
      "noPersonaDesc": "Kişi olmadan devam edin",
      "noPersonasAvailable": "Kullanılabilir kişi yok",
      "noPersonasDesc": "Görüşmelerinizi kişiselleştirmek için ayarlarda bir kişi oluşturun.",
      "searchPersonas": "Kişileri arayın...",
      "noPersonasFound": "Kişi bulunamadı",
      "tryDifferentSearch": "Farklı bir arama terimi deneyin"
    },
    "footerExtra": {
      "cancelRecording": "Kaydı iptal et",
      "transcribing": "Transkripsiyon",
      "stopAndTranscribe": "Durdur ve metne dönüştür",
      "recordVoice": "_Sesi kaydet_",
      "attachmentAltDefault": "_Ek_"
    },
    "pageExtra": {
      "noAudioCaptured": "_Ses yakalanmadı.",
      "noWhisperModelInstalled": "Kurulu Whisper modeli bulunamadı. ",
      "scrollToBottomAria": "Aşağıya doğru kaydırın"
    },
    "addContentMenu": {
      "title": "İçerik Ekle",
      "uploadImage": "Resim Yükle"
    },
    "helpMeReplyMenu": {
      "title": "Yanıtlamama Yardım Edin",
      "helpMeReplyDesc": "Yapay zekanın ne söyleyeceğimi önermesine izin verin",
      "draftPrompt": "Taslak mesajınız var. ",
      "useTextAsBase": "Metnimi temel olarak kullan",
      "useTextAsBaseDesc": "Taslağınızı genişletin ve geliştirin",
      "writeSomethingNew": "Yeni bir şeyler yazın",
      "writeSomethingNewDesc": "Yeni bir yanıt oluşturun"
    },
    "suggestedReplyMenu": {
      "title": "Önerilen Yanıt",
      "reasoningBeforeWriting": "Cevabınızı yazmadan önce akıl yürütme...",
      "writingYourReply": "Cevapınızı yazma...",
      "regenerate": "Yeniden Oluştur",
      "useReply": "Yanıtı Kullan_"
    },
    "memoriesPageExtra": {
      "sessionNotFound": "Oturum bulunamadı_",
      "memorySystemError": "Bellek Sistemi Hatası",
      "retryingMemoryCycle": "Bellek Döngüsü yeniden deneniyor...",
      "processingMemories": "Anılar işleniyor...",
      "memoryCycleSuccess": "Bellek döngüsü başarıyla işlendi!",
      "contextSummaryTitle": "Bağlam Özeti",
      "activityTabLabel": "Etkinlik_",
      "noMatchingMemoriesDesc": "Farklı bir arama terimi deneyin",
      "chatHistoryTitle": "Sohbet Geçmişi",
      "chatHistoryDesc": "Konuşmaları görüntüleyin ve yönetin"
    },
    "settingsPageExtra": {
      "quickActionsSection": "Hızlı İşlemler",
      "chatHistoryTitle": "Sohbet Geçmişi_",
      "chatHistoryDesc": "Konuşmaları görüntüleyin ve yönetin",
      "lorebrooksTitle": "Bilgi Kitapları",
      "lorebrooksDesc": "Oturum bilgi kitaplarını ekleyin ve isteğe bağlı olarak her karakterin kendi bilgi kitaplarını yok sayın.",
      "manageLorebooks": "Bilgi kitaplarını yönet"
    },
    "groupSettingsPageExtra": {
      "lorebrooksTitle": "Bilgi kitapları",
      "lorebrooksDesc": "Paylaşılan bilgi kitaplarını ekleyin ve karakter bilgi kitaplarının varsayılan olarak uygulanıp uygulanmayacağını kontrol edin.",
      "manageLorebooks": "Bilgi kitaplarını yönet"
    }
  },
  "characters": {
    "empty": {
      "title": "Henüz karakter yok",
      "description": "Benzersiz kişiliklere sahip özel yapay zeka karakterleri oluştur",
      "createButton": "Karakter oluştur"
    },
    "progress": {
      "identity": "Kimlik",
      "scenes": "Sahneler",
      "details": "Detaylar"
    },
    "identity": {
      "title": "Karakter oluştur",
      "subtitle": "Yapay zeka karakterine bir kimlik ver",
      "tapCameraToAdd": "Avatar eklemek veya oluşturmak için kameraya dokun",
      "importingAvatar": "Avatar içe aktarılıyor...",
      "characterName": "Karakter adı *",
      "characterNamePlaceholder": "Karakter adını gir...",
      "characterNameDesc": "Bu ad sohbet konuşmalarında görünecek",
      "avatarGradient": "Avatar gradyanı",
      "avatarGradientDesc": "Avatar renklerinden dinamik gradyanlar oluştur",
      "chatBackgroundLabel": "Sohbet Arka Planı",
      "optionalSuffix": "(İsteğe bağlı)",
      "backgroundPreviewAlt": "Arka plan önizleme",
      "backgroundPreviewBadge": "Arka Plan Önizlemesi",
      "addBackground": "Arka Plan Ekle_",
      "addBackgroundHint": "Bir tane yükleyin veya kitaplıktan seçin_",
      "uploadImage": "_Resim yükle",
      "chooseFromLibrary": "Kütüphaneden seçim yapın",
      "backgroundDesc": "Sohbet konuşmaları için isteğe bağlı arka plan resmi",
      "continueToDescription": "Açıklamaya Devam Et",
      "importCharacterFromFile": "Dosyadan Karakter İçe Aktar",
      "importCharacterDesc": "PNG kartından, .uec veya .json dışa aktarma dosyasından bir karakter yükleyin"
    },
    "details": {
      "title": "Karakter detayları",
      "subtitle": "Kişilik ve davranışı tanımla",
      "definition": "Tanım *",
      "wordCount": "{{count}} kelime",
      "definitionPlaceholder": "Kişiliği, konuşma tarzını, geçmişi, uzmanlık alanlarını tanımla...",
      "definitionDesc": "Ton, özellikler ve konuşma tarzı hakkında spesifik ol",
      "availablePlaceholders": "Kullanılabilir yer tutucular:"
    },
    "scenes": {
      "title": "Başlangıç sahneleri",
      "subtitle": "Konuşmalarınız için açılış senaryoları oluştur",
      "default": "Varsayılan",
      "hasSceneDirection": "Sahne yönlendirmesi var",
      "continueToScenes": "Başlangıç sahnelerine devam et"
    },
    "extras": {
      "title": "Ek detaylar",
      "subtitle": "Tüm alanlar isteğe bağlıdır",
      "nickname": "Takma ad",
      "nicknamePlaceholder": "Kullanıcı bu karaktere nasıl hitap etmeli?",
      "nicknameDesc": "Konuşmalarda gösterilen alternatif ad",
      "creator": "Oluşturan",
      "creatorPlaceholder": "Oluşturanın adı...",
      "tags": "Etiketler",
      "tagsPlaceholder": "fantezi, bilim kurgu, romantik...",
      "tagsDesc": "Filtreleme ve düzenleme için virgülle ayrılmış liste",
      "creatorNotes": "Oluşturanın notları",
      "creatorNotesPlaceholder": "Kullanım ipuçları, lore bağlamı veya diğer kullanıcılar için talimatlar...",
      "multilingualNotes": "Çok Dilli Notlar",
      "multilingualNotesHint": "Anahtar olarak dil kodlarına sahip JSON nesnesi",
      "creatingCharacter": "Karakter Oluşturuluyor..."
    },
    "preview": {
      "unnamedCharacter": "Adsız karakter",
      "sceneCount": "{{count}} sahne",
      "customPrompt": "Özel prompt",
      "description": "Açıklama",
      "startingScene": "Başlangıç sahnesi",
      "gradientEnabled": "Gradyan etkin",
      "customModel": "Özel model",
      "avatarAlt": "Karakter avatarı",
      "characterFallback": "Character"
    },
    "personaPreview": {
      "unnamedPersona": "Adsız persona",
      "noDescription": "Henüz açıklama yok",
      "default": "Varsayılan",
      "description": "Açıklama"
    },
    "lorebookPreview": {
      "untitledLorebook": "Başlıksız lore kitabı",
      "entryCount": "{{count}} girdi",
      "entries": "Girdiler",
      "noEntriesYet": "Henüz girdi yok",
      "untitledEntry": "Başlıksız girdi",
      "noContentYet": "Henüz içerik yok",
      "alwaysActive": "Her zaman aktif",
      "moreEntries": "+{{count}} daha fazla giriş",
      "moreEntry": "+{{count}} daha fazla giriş"
    },
    "creationHelper": {
      "moreOptions": "Daha fazla seçenek",
      "describePlaceholder": "Karakterini tanımla...",
      "stopGeneration": "Oluşturmayı durdur",
      "sendMessage": "Mesaj gönder",
      "addToMessage": "Mesaja ekle",
      "uploadImageDesc": "Avatar veya referans görseli ekle",
      "referenceCharacterDesc": "Mevcut bir karakteri ilham kaynağı olarak kullan",
      "referencePersonaDesc": "Personanı bağlam olarak kullan",
      "retry": "Yeniden dene",
      "attachmentAlt": "Ek",
      "removeAttachment": "Eki kaldır",
      "removeReference": "Referansı kaldır",
      "uploadImageTitle": "Resmi Yükle",
      "referenceCharacterTitle": "Referans Karakteri",
      "referencePersonaTitle": "Referans Kişisi"
    },
    "lorebook": {
      "keywords": "ANAHTAR KELİMELER",
      "caseSensitive": "Büyük/küçük harf duyarlı",
      "typeKeyword": "Anahtar kelime yaz...",
      "addButton": "Ekle",
      "untitledEntry": "Başlıksız girdi",
      "alwaysActive": "Her zaman aktif",
      "noKeywords": "Anahtar kelime yok",
      "dragToReorder": "Sıralamak için sürükle",
      "disabled": "Devre dışı",
      "noLorebooksYet": "Henüz lore kitabı yok",
      "createLorebookDesc": "Bu karakter için dünya loresi eklemek üzere bir lore kitabı oluştur",
      "createLorebook": "Lore kitabı oluştur",
      "searchPlaceholder": "Lore kitaplarında ara...",
      "noMatchingLorebooks": "Eşleşen lore kitabı bulunamadı",
      "activeLorebooks": "Aktif lore kitapları",
      "sectionActive": "Aktif",
      "sectionAvailable": "Kullanılabilir",
      "entryCount": "{{count}} girişler",
      "enabledForCharacter": "bu karakter için etkin",
      "enabledForGroup": "bu grup için etkin",
      "enabledForSession": "bu oturum için etkin",
      "tapToViewEntries": "Girdileri görmek için dokun",
      "newLorebookTitle": "Yeni lore kitabı",
      "nameLabel": "AD",
      "enterNamePlaceholder": "Lore kitabı adını gir...",
      "lorebookExplanation": "Lore kitapları, anahtar kelimeler eşleştiğinde promptlara enjekte edilen girdiler içerir.",
      "keywordDetectionMode": "_ANAHTAR KELİME ALGILAMA_",
      "keywordDetectionRecentWindow": "Son 10 mesaj_",
      "keywordDetectionRecentWindowDesc": "_Son mesajlarla eşleşir ",
      "keywordDetectionLatestUser": "Yalnızca en son kullanıcı mesajı",
      "keywordDetectionLatestUserDesc": "Yalnızca kullanıcı tarafından gönderilen en son mesajla eşleşir.",
      "viewEntries": "Girdileri görüntüle",
      "editEntriesDesc": "Lore kitabı girdilerini düzenle",
      "disableForCharacter": "Karakter için devre dışı bırak",
      "enableForCharacter": "Karakter için etkinleştir",
      "disableForGroup": "Grup için devre dışı bırak",
      "enableForGroup": "Grup için etkinleştir",
      "disableForSession": "Oturum için devre dışı bırak",
      "enableForSession": "Oturum için etkinleştir",
      "removeFromActive": "Bu karakterin aktif lore kitaplarından kaldır",
      "addToActive": "Bu karakterin aktif lore kitaplarına ekle",
      "removeFromActiveGroup": "Bu grubun aktif lore kitaplarından kaldır",
      "addToActiveGroup": "Bu grubun aktif lore kitaplarına ekle",
      "removeFromActiveSession": "Bu oturumun aktif lore kitaplarından kaldır",
      "addToActiveSession": "Bu oturumun aktif lore kitaplarına ekle",
      "deleteConfirmTitle": "Lore kitabı silinsin mi?",
      "deleteConfirmMessage": "Bu lore kitabı silinsin mi? Tüm girdiler kaybolacak.",
      "deleteLorebook": "Lore kitabını sil",
      "noEntriesYet": "Henüz girdi yok",
      "addEntriesToInject": "Sohbetlerine lore enjekte etmek için girdi ekle",
      "createEntry": "Girdi oluştur",
      "searchEntries": "Girdilerde ara...",
      "noMatchingEntries": "Eşleşen girdi bulunamadı",
      "entryDefaultName": "Girdi",
      "editEntry": "Girdiyi düzenle",
      "editEntryDesc": "Başlık, anahtar kelimeler ve içeriği düzenle",
      "disableEntry": "Girdiyi devre dışı bırak",
      "enableEntry": "Girdiyi etkinleştir",
      "entryDisabledDesc": "Girdi promptlara enjekte edilmeyecek",
      "entryEnabledDesc": "Girdi, anahtar kelimeler eşleştiğinde enjekte edilecek",
      "deleteEntry": "Girdiyi sil",
      "titleLabel": "BAŞLIK",
      "titlePlaceholder": "Bu girdiyi adlandır...",
      "enabled": "Etkin",
      "includeInPrompts": "Promptlara dahil et",
      "alwaysOn": "Her zaman açık",
      "noKeywordsNeeded": "Anahtar kelime gerekmez",
      "contentLabel": "İÇERİK",
      "contentPlaceholder": "Lore bağlamını buraya yaz...",
      "saveEntry": "Girdiyi kaydet",
      "noCharacterId": "Karakter kimliği sağlanmadı",
      "preview": {
        "title": "Tetikleyici Önizleme",
        "openButton": "Önizleme",
        "missingContext": "Hiçbir bilgi kitabı seçilmedi.",
        "noEntries": "Neyin tetikleyeceğini görmek için bu bilgi kitabına girişler ekleyin.",
        "modeRecentShort": "Penceredeki en son {{count}}",
        "modeLatestUserShort": "Son kullanıcı",
        "inWindow": "{{count}}",
        "tabSession": "Oturum",
        "tabCompose": "Oluşturma",
        "activeStat": "{{active}} / {{total}} etkin",
        "tokensStat": "{{count}} jetonlar",
        "sessionPickerLabel": "Oturumlar",
        "sessionMeta": "{{count}} msjlar",
        "noSessions": "Henüz sohbet oturumu yok.",
        "noSessionsHint": "Bir oturum seçin",
        "noMessages": "Bu oturumda henüz mesaj yok.",
        "scanHeaderRecent": "Son {{depth}} mesajın {{shown}} tanesi taranıyor",
        "scanHeaderLatest": "En son kullanıcı mesajı taranıyor",
        "matchCount": "{{hits}} isabet · {{entries}} giriş",
        "emptyMessage": "(boş)",
        "roleUser": "Kullanıcı",
        "roleAssistant": "Asistan",
        "roleScene": "Sahne",
        "roleSystem": "Sistem",
        "composeHeader": "Karalama defteri",
        "composeMatches": "{{count}} eşleşir_",
        "activeLabel": "{{count}} active_",
        "composePlaceholder": "Anahtar kelime eşleşmesini test etmek için metin yazın veya yapıştırın...\n\n",
        "sectionActive": "Aktif · {{count}}",
        "sectionInactive": "Etkin değil · {{count}}",
        "statusMatched": "Eşleşti",
        "statusAlways": "Her zaman",
        "statusDisabled": "Kapalı",
        "statusNoKeywords": "Anahtar yok",
        "statusNotMatched": "Eşleşme yok",
        "tokensShort": "{{count}}t",
        "injectionTitle": "Son enjeksiyon",
        "injectionEmpty": "Hiçbir giriş etkin değil — hiçbir şey enjekte edilmeyecek.",
        "copy": "Kopyalama_",
        "copyFailed": "Panoya kopyalanamadı.",
        "saveFailed": "Giriş kaydedilemedi.",
        "deleteFailed": "Giriş silinemedi.",
        "deleteConfirmTitle": "Emin misin?",
        "deleteConfirmMessage": "\"{{title}}\" silinsin mi? ",
        "contextMenuTitle": "{{count}} girişler bu"
      }
    },
    "templates": {
      "characterNotFound": "Karakter bulunamadı",
      "templateCount": "{{count}} şablon",
      "newTemplate": "Yeni şablon",
      "noTemplatesYet": "Henüz şablon yok",
      "explanation": "Sohbet şablonları, hem senden hem de {{name}}'den önceden yazılmış mesajlarla konuşma başlatmanı sağlar.",
      "createTemplate": "Şablon oluştur",
      "messageCount": "{{count}} mesaj",
      "deleteTemplate": "Şablonu sil",
      "contextMenuFallbackTitle": "Şablonunu kullanıyor",
      "importedToast": {
        "title": "İçe aktarıldı",
        "message": "\"{{name}}\" eklendi."
      },
      "importFailed": "İçe aktarma başarısız oldu",
      "exportFailed": "Dışa aktarma başarısız oldu_"
    },
    "templateEditor": {
      "noScene": "Sahne yok",
      "untitled": "Başlıksız",
      "dragMessage": "Mesajı sürükle",
      "editMessage": "Mesajı düzenle",
      "deleteMessage": "Mesajı sil",
      "writeMessagePlaceholder": "Mesaj içeriğini yaz...",
      "characterNotFound": "Karakter bulunamadı",
      "scene": "Sahne",
      "noMessagesYet": "Henüz mesaj yok",
      "addMessagesDesc": "{{name}} ile konuşma başlangıcı oluşturmak için mesaj ekle.",
      "addMessage": "Mesaj ekle",
      "name": "Ad",
      "nameExample": "ör. Rahat selamlama",
      "startingScene": "Başlangıç sahnesi",
      "systemPrompt": "Sistem promptu",
      "characterDefault": "Karakter varsayılanı",
      "nextMessageAs": "Sonraki mesaj olarak",
      "messages": "Mesajlar",
      "roles": "Roller",
      "hoverTip": "Sürüklemek, düzenlemek veya silmek için mesajların üzerine gel.",
      "footerTip": "Konuşmaya yeni mesajlar eklemek için alt çubuğu kullan.",
      "templateNamePlaceholder": "Şablon adı...",
      "selectScene": "Sahne seç",
      "startWithoutScene": "Sahne mesajı olmadan başla",
      "selectSystemPrompt": "Sistem promptu seç",
      "useCharacterSystemPrompt": "Karakterin sistem promptunu kullan"
    },
    "referenceSelector": {
      "selectCharacter": "Karakter seç",
      "selectPersona": "Persona seç",
      "searchPlaceholder": "{{type}} ara...",
      "loading": "_Yükleniyor...",
      "noMatch": "Aramanızla eşleşen {{type}} yok",
      "noAvailable": "Mevcut {{type}} yok"
    },
    "voiceLoading": {
      "failed": "Sesler yüklenemedi"
    },
    "activeLorebooks": {
      "sectionTitle": "Aktif Bilgikitapları",
      "selectedSummary": "{{count}} aktif",
      "untitledLorebook": "Başlıksız bilgi kitabı",
      "usingNone": "Karaktersiz bilgi kitapları kullanılmıyor",
      "loading": "Bilgi kitapları yükleniyor...",
      "loadFailed": "Bilgi kitapları yüklenemedi",
      "inheritHint": "Oturum bunları geçersiz kılmadığı sürece oturumlar bunları devralır.",
      "clear": "Clear",
      "chooseHint": "Bu karakterin varsayılan olarak etkinleştirmesi gereken bilgi kitaplarını seçin. ",
      "emptyState": "Henüz lorebook yok. "
    },
    "description": {
      "descriptionLabel": "Açıklama",
      "descriptionPlaceholder": "Kartlarda ve listelerde gösterilen kısa özet...",
      "descriptionHint": "Kullanıcı arayüzü için isteğe bağlı kısa açıklama; ",
      "companionPromptLabel": "Tamamlayıcı İstem (İsteğe bağlı)",
      "systemPromptLabel": "Sistem İstemi (İsteğe bağlı)",
      "loadingTemplates": "Şablonlar yükleniyor...",
      "useAppCompanionDefault": "Uygulama tamamlayıcısı varsayılanını kullan",
      "useAppDefault": "Uygulama varsayılanını kullan",
      "companionPromptHint": "Tamamlayıcı bilgi istemi olarak ayrı olarak saklanır. ",
      "systemPromptHint": "Özel bir sistem istemi seçin veya varsayılanı kullanın.",
      "groupChatConvLabel": "Grup Sohbet İstemi (Konuşma)",
      "groupChatConvHint": "Grup sohbetlerinde bu karakterin konuşma istemini geçersiz kıl",
      "groupChatRpLabel": "Grup Sohbet İstemi (Rol Oynama)",
      "groupChatRpHint": "Grup sohbetlerinde bu karakterin rol yapma istemini geçersiz kılın",
      "voiceLabel": "Ses (İsteğe bağlı)",
      "loadingVoices": "Sesler yükleniyor...",
      "customVoiceFallback": "Özel Ses_",
      "providerVoiceFallback": "Sağlayıcı Sesi",
      "selectedVoiceFallback": "Seçili Ses",
      "noVoiceAssigned": "Ses atanmadı_",
      "addVoicesHint": "Ayarlar'da sesler ekleyin → Sesler_",
      "voiceAssignHint": "_Gelecek için bir ses atayın ",
      "autoplayLabel": "Sesi otomatik oynat",
      "autoplayOn": "Bu karakterin yanıtlarını otomatik olarak oynat",
      "autoplayOff": "Önce bir ses seçin",
      "aiModelLabel": "AI Modeli *_",
      "loadingModels": "_Modeller yükleniyor..._",
      "selectedModelFallback": "Seçilen Model",
      "selectModelPlaceholder": "Bir model seçin",
      "noModelsConfigured": "Yapılandırılan model yok",
      "noModelsHint": "Devam etmek için önce ayarlara bir sağlayıcı ekleyin",
      "aiModelHint": "Bu model, karakterin yanıtlarını güçlendirecektir",
      "fallbackModelLabel": "_Yedek Model (İsteğe bağlı)",
      "selectedFallbackFallback": "Seçilen Yedek Model",
      "fallbackOff": "Kapalı (geri dönüş yok)",
      "fallbackHint": "Yalnızca birincil model başarısız olursa bu modelle yeniden dener",
      "memoryModeLabel": "Bellek Modu",
      "enableInSettingsHint": "Geçiş yapmak için Ayarlar'da etkinleştirin",
      "memoryManual": "Manuel",
      "memoryManualDescDesktop": "Anı notlarını kendiniz ekleyin ve yönetin.",
      "memoryManualDescMobile": "Mevcut sistem: Anı notlarını kendiniz ekleyin ve yönetin.",
      "memoryDynamic": "Dynamic",
      "memoryDynamicDescDesktop": "Otomatik özetler ve içerik güncellemeleri.",
      "memoryDynamicDescMobile": "Bu karakter için otomatik özetler ve içerik güncellemeleri.",
      "memoryHint": "Dinamik bellek, Gelişmiş ayarlarda etkinleştirilmesini gerektirir. ",
      "selectModelTitle": "Model Seç_",
      "selectFallbackModelTitle": "Geri Dönüş Modeli Seç",
      "searchModelsPlaceholder": "Modelleri ara...",
      "selectVoiceTitle": "_Ses Seç_",
      "searchVoicesPlaceholder": "Sesleri ara..._",
      "myVoices": "_Seslerim",
      "providerVoicesLabel": "{{provider}} Sesler",
      "providerFallback": "Provider"
    },
    "interactionMode": {
      "sectionLabel": "Etkileşim Modu",
      "sectionHint": "Bu karakterin bir RP karakteri gibi mi yoksa kalıcı bir tamamlayıcı gibi mi davranacağını seçin.",
      "activeBadge": "Aktif",
      "roleplayTitle": "Rol yapma_",
      "roleplaySubtitle": "Sahne odaklı sohbetler, anlatı çerçeveleme ve başlangıç ​​senaryoları.",
      "companionTitle": "Companion",
      "companionSubtitle": "Duygusal durum ve eşlik eden hafızayla ilişki odaklı sohbetler._"
    },
    "startingScene": {
      "openingContextTitle": "_Açılış İçeriği",
      "openingContextSubtitle": "Bu arkadaşınız için isteğe bağlı ilk sohbet bağlamı. ",
      "sceneDirectionLabel": "Sahne Yönü",
      "setAsDefault": "Varsayılan olarak ayarla",
      "noOpeningContext": "Henüz açılış içeriği yok",
      "noScenesYet": "Henüz sahne yok",
      "skipForCompanion": "Tamamlayıcı mod için bunu atlayabilirsiniz.",
      "createFirstScene": "Başlamak için ilk sahnenizi oluşturun",
      "openingPlaceholder": "Arkadaşınızın ilk mesajdan önce nerede olduğu veya ne yaptığı gibi isteğe bağlı açılış bağlamı...",
      "scenePlaceholder": "Rol oyunu için bir başlangıç ​​sahnesi veya senaryo oluşturun (örneğin, 'Alacakaranlıkta kendinizi mistik bir ormanda buluyorsunuz...')",
      "addDirection": "+ Yön Ekle",
      "directionAdded": "Yön eklendi",
      "wordsCount": "{{count}} kelimeler",
      "placeholderHelp": "Karakter için {{charTag}} ve kişi için {{userTag}} (takma ad {{personaTag}}) kullanın.",
      "sceneBackgroundLabel": "Sahne Arka Planı",
      "optionalLabel": "_İsteğe bağlı",
      "sceneBgOverrideHint": "Bu sahneyi kullanan sohbetler için karakter arka planını geçersiz kılar.",
      "sceneBgUsedHint": "Oturum bunu geçersiz kılmadığı sürece bu sahnenin sohbet arka planı olarak kullanılır.",
      "cancel": "İptal etmek",
      "directionPlaceholderNew": "örneğin, 'Rehine kurtarılacak' veya 'Gergin atmosferi sürdürün'",
      "directionPlaceholderEdit": "örneğin, 'Rehine kurtarılacak' veya 'Gerginliği kademeli olarak artırın'",
      "directionAiHint": "Bu sahnenin nasıl gelişmesi gerektiğine dair yapay zeka için gizli rehberlik",
      "addScene": "Sahne Ekle",
      "multipleScenesHint": "Birden fazla başlangıç ​​senaryosu oluşturun. ",
      "companionContextHint": "Açılış bağlamı tamamlayıcılar için isteğe bağlıdır; ",
      "skipContext": "Bağlamı Atla",
      "editSceneTitle": "Sahneyi Düzenle",
      "sceneContentPlaceholder": "Sahne içeriğini girin...",
      "addLabel": "+ Ekle",
      "save": "Kaydet_",
      "library": "_Kütüphane_",
      "upload": "_Yükle",
      "sceneBackgroundAlt": "Sahne arka planı",
      "removeBackground": "Arka planı kaldır"
    },
    "companionSoul": {
      "title": "Companion Soul",
      "subtitle": "Altında kim olduklarını şekillendirin. ",
      "retry": "Tekrar dene",
      "back": "Geri",
      "continue": "Devam_",
      "addNameFirst": "Önce bir ad ekleyin.",
      "addDefinitionFirst": "Önce bir tanım ekleyin."
    },
    "soulEditor": {
      "generateTitle": "Karakterden oluştur",
      "generateUsingModel": "{{model}} kullanır. ",
      "generateDefaultDesc": "Karakterin adından, tanımından ve sahnelerinden bir ruh taslağı çıkarır.",
      "directionLabel": "Yön",
      "directionOptional": "LLM",
      "directionPlaceholder": "e.g. için isteğe bağlı direksiyon. ",
      "directionEditTooltip": "Üretim için isteğe bağlı yön",
      "generating": "Oluşturma",
      "generate": "Oluştur",
      "presetLabel": "Kişilik ön ayarı",
      "presetMatches": "Eşleşmeler: {{label}}",
      "presetHint": "Temel etki, düzenleme ve ilişki kaydırıcılarını ayarlar. ",
      "identityLabel": "Kimlik",
      "hideExamples": "Örnekleri gizle",
      "showExamples": "Örnekleri göster",
      "insertExample": "Örnek ekleyin",
      "exampleEg": "e.g., {{example}}",
      "fineTuneLabel": "Duygulara ince ayar yapın",
      "baselineAffect": "_Temel Duygulanım_",
      "baselineAffectInfo": "_Varsayılan olarak nasıl hissettikleri; ",
      "regulationStyle": "Düzenleme Stili",
      "regulationStyleInfo": "Ne hissettiklerini nasıl ele alıyorlar ve ifade ediyorlar; ",
      "relationshipDefaults": "İlişki Varsayılanları",
      "relationshipDefaultsInfo": "Bu oturumun başladığı yer. "
    },
    "soulPresets": {
      "secureLabel": "Secure_",
      "secureBlurb": "Sıcak, sabit, hızla iyileşir. ",
      "anxiousLabel": "Kaygılı",
      "anxiousBlurb": "Güçlü bağlanma, mesafeden korkar, güvence arar.",
      "avoidantLabel": "Kaçıngan",
      "avoidantBlurb": "Kendine güvenir, yavaş açılır, duygusal mesafeyi korur.",
      "volatileLabel": "_Uçucu",
      "volatileBlurb": "Reaktiftir, yoğundur, duyguları filtresiz ifade eder.",
      "reservedLabel": "Ayrılmış",
      "reservedBlurb": "Sessiz, sakin, güvenmek ve kendini göstermek zaman alır.",
      "playfulLabel": "Şakacı",
      "playfulBlurb": "Sıcak, etkileyici ve hafif. "
    },
    "soulFields": {
      "essence": "Öz",
      "essencePlaceholder": "Kart tanımının altında kimler var.",
      "essenceExample": "Güvendikleri insanlar için kolayca bozulan pratik bir sakinlik. ",
      "voice": "İç Ses_",
      "voicePlaceholder": "Yakın konuşmalarda nasıl ses çıkarıyorlar?",
      "voiceExample": "Düşük, kasıtlı, uzun duraklamalarla. ",
      "relationalStyle": "İlişkisel Stil",
      "relationalStylePlaceholder": "Nasıl bağlanırlar, güvenirler, geri çekilirler, yeniden bağlanırlar.",
      "relationalStyleExample": "Açılması yavaş ama açıldıklarında sadıklar. ",
      "vulnerabilities": "Güvenlik açıkları",
      "vulnerabilitiesPlaceholder": "Zayıf noktalar, güvensizlikler, nadiren söyledikleri şeyler.",
      "vulnerabilitiesExample": "Yük olmaktan korkuyorum. ",
      "habits": "Alışkanlıklar",
      "habitsPlaceholder": "Tekrarlanan anlatımlar, ritüeller, konuşma kalıpları.",
      "habitsExample": "Sinirlendiğinde saçı kulağın arkasına sıkıştırır. ",
      "boundaries": "Sınırlar",
      "boundariesPlaceholder": "Aşmayacakları çizgiler. ",
      "boundariesExample": "Savunmasızlığa sürüklenmeyeceğiz. "
    },
    "soulSliders": {
      "warmth": "Sıcaklık",
      "warmthLow": "Soğuk",
      "warmthHigh": "Sevgi dolu",
      "trust": "Güven",
      "trustLow": "Korunan",
      "trustHigh": "Açık",
      "calm": "Sakin",
      "calmLow": "Endişeli",
      "calmHigh": "Steady",
      "vulnerability": "Güvenlik Açığı",
      "vulnerabilityLow": "Duvarlı",
      "vulnerabilityHigh": "Açığa Çıktı",
      "longing": "Özlem",
      "longingLow": "İçerik",
      "longingHigh": "Yıllık_",
      "hurt": "_Zarar_",
      "hurtLow": "İyileşti",
      "hurtHigh": "İhale",
      "tension": "Gerginlik",
      "tensionLow": "Gevşemiş",
      "tensionHigh": "_Yaralanma",
      "irritation": "Tahriş",
      "irritationLow": "Hasta",
      "irritationHigh": "Kolayca yola koyulur_",
      "affection": "Sevgi",
      "affectionLow": "Kısıtlanmış",
      "affectionHigh": "_Effusive_",
      "reassuranceNeed": "_Güven İhtiyacı_",
      "reassuranceNeedLow": "_Kendini sakinleştirme",
      "reassuranceNeedHigh": "Kelimelere ihtiyaç var",
      "suppression": "Bastırma",
      "suppressionLow": "İfadeler",
      "suppressionHigh": "_Hides_",
      "volatility": "_Volatility_",
      "volatilityLow": "_Eşit omurgalı",
      "volatilityHigh": "Reaktif",
      "recoverySpeed": "Kurtarma Hızı",
      "recoverySpeedLow": "Yavaş",
      "recoverySpeedHigh": "_Hızlı_",
      "conflictAvoidance": "_Çakışmalardan Kaçınma_",
      "conflictAvoidanceLow": "_Etkileşime Geçer",
      "conflictAvoidanceHigh": "Para Çekme",
      "reassuranceSeeking": "Güvence Arayan",
      "reassuranceSeekingLow": "Bağımsız",
      "reassuranceSeekingHigh": "Sık sık sorar_",
      "protestBehavior": "Protesto Davranışı",
      "protestBehaviorLow": "_Sessiz",
      "protestBehaviorHigh": "Yüksek Sesli_",
      "transparency": "Şeffaflık",
      "transparencyLow": "Opak",
      "transparencyHigh": "_Görüntüler_",
      "attachmentActivation": "Ek Aktivasyonu_",
      "attachmentActivationLow": "_Ayrılmış",
      "attachmentActivationHigh": "Kolayca tetiklenir",
      "pride": "Pride",
      "prideLow": "Bükümler",
      "prideHigh": "Hattı tutar_",
      "closeness": "_Yakınlığı Başlatma",
      "closenessLow": "_Yabancılar",
      "closenessHigh": "Samimi_",
      "relTrust": "Güveni Başlatmak",
      "relTrustLow": "Uyarı",
      "relTrustHigh": "Güvenmek",
      "relAffection": "_Sevgiyi Başlatmak",
      "relAffectionLow": "Nötr",
      "relAffectionHigh": "Sevgi dolu",
      "relTension": "Gerginliği Başlatma",
      "relTensionLow": "Kolay",
      "relTensionHigh": "_Şarjlı_"
    },
    "soulReview": {
      "reviewTitle": "_İnceleme oluşturulan ruh",
      "noDifferences": "_Şu anki durumdan hiçbir fark yok.",
      "changesHeader": "{{count}} değişiklik(ler); ",
      "close": "Close",
      "identityLabel": "Kimlik",
      "nEdited": "{{count}} düzenlendi",
      "edited": "Düzenlendi",
      "tuningLabel": "Ayarlama",
      "unchanged": "Değişmedi_",
      "nChanges": "{{count}} değişiklik(ler)",
      "direction": "Yön",
      "directionApplyHint": "Düzenlemeler bir sonraki Yeniden Oluşturma işleminde uygulanır",
      "directionPlaceholder": "örneğin ",
      "directionTooltip": "Yeniden oluşturmadan önce yönü düzenleyin",
      "regenerate": "Yeniden oluştur_",
      "discard": "Discard",
      "apply": "Uygula"
    },
    "hookErrors": {
      "creatorNotesInvalidJson": "Yaratıcı notları çok dilli geçerli bir JSON nesnesi olmalıdır",
      "creatorNotesNotObject": "creatorNotesÇok dilli bir JSON nesnesi olmalıdır",
      "saveFailed": "Karakter kaydedilemedi",
      "importFailed": "Karakter içe aktarılamadı",
      "avatarLoadFailed": "Avatar URL'si yüklenemedi",
      "avatarProcessFailed": "Avatar resmi işlenemedi",
      "avatarConvertFailed": "Avatar URL'si dönüştürülemedi",
      "avatarUrlLoadFailed": "Avatar URL'si yüklenemedi",
      "remoteAvatarDisabled": "Uzaktan avatar indirme Güvenlik ayarlarında devre dışı bırakıldı.\n",
      "importReadyTitle": "İçe aktarma hazır",
      "importReadyMessage": "Algılandı {{label}}",
      "legacyJsonTitle": "Eski JSON içe aktarma algılandı",
      "legacyJsonMessage": "JSON içe aktarmaları kullanımdan kaldırıldı ve yakında kaldırılacak. ",
      "loadFailed": "Karakter yüklenemedi",
      "exportFailed": "Karakter dışa aktarılamadı"
    }
  },
  "providers": {
    "empty": {
      "title": "Henüz sağlayıcı yok",
      "description": "Yapay zeka modelleri için API sağlayıcıları ekle ve yönet",
      "addButton": "Sağlayıcı ekle"
    },
    "actions": {
      "openDashboard": "Paneli aç",
      "openDashboardDesc": "Karakterleri, kullanımı ve ayarları görüntüle",
      "edit": "Düzenle",
      "editDesc": "Sağlayıcı ayarlarını değiştir"
    },
    "extra": {
      "apiKeyNotFound": "OpenRouter API anahtarı bulunamadı. Önce Ayarlar > Sağlayıcılar bölümünden yapılandır.",
      "audioEmpty": {
        "title": "Ses sağlayıcısı yok",
        "description": "Karakterleriniz için sesler oluşturmak amacıyla bir TTS sağlayıcısı ekleyin.",
        "addButton": "Sağlayıcı ekle"
      },
      "audioProviderLabel": {
        "elevenlabs": "ElevenLabs",
        "geminiTts": "Gemini TTS",
        "openaiTts": "OpenAI uyumlu TTS",
        "kokoro": "Kokoro (Yerel)"
      }
    },
    "audioEditor": {
      "titleEdit": "Sağlayıcıyı düzenle",
      "titleCreate": "Ses sağlayıcısı ekle",
      "fields": {
        "providerType": "Sağlayıcı türü",
        "label": "Etiket",
        "apiKey": "API anahtarı",
        "modelVariant": "Model varyantı",
        "assetRoot": "Varlık kök dizini",
        "projectId": "Google Cloud proje kimliği",
        "region": "Bölge (isteğe bağlı)",
        "baseUrl": "Temel URL",
        "requestPath": "İstek yolu"
      },
      "types": {
        "gemini": "Gemini TTS (Google)",
        "openai": "OpenAI uyumlu TTS",
        "kokoro": "Kokoro (Yerel)"
      },
      "placeholders": {
        "labelGemini": "Gemini TTS'im",
        "labelOpenai": "Uyumlu TTS'im",
        "labelKokoro": "Kokoro Yerel",
        "labelElevenlabs": "ElevenLabs'ım",
        "apiKey": "API anahtarınızı girin",
        "assetRoot": "/kokoro/yolu",
        "projectId": "proje-kimliginiz",
        "region": "us-central1",
        "baseUrl": "https://api.ornek.com"
      },
      "errors": {
        "chooseModelVariant": "Bir model varyantı seçin",
        "assetRootRequired": "Varlık kök dizini zorunludur",
        "saveFailed": "Kaydetme başarısız",
        "apiKeyRequired": "API anahtarı zorunludur",
        "projectIdRequired": "Gemini TTS için proje kimliği zorunludur",
        "baseUrlRequired": "OpenAI uyumlu TTS için temel URL zorunludur",
        "invalidCredentials": "Geçersiz API anahtarı veya kimlik bilgileri",
        "verificationFailed": "Doğrulama başarısız"
      },
      "loadingVariants": "Varyantlar yükleniyor...",
      "kokoroVariantHint": "Mobil derlemeler yalnızca int8'i destekler. Kaydetme işleminin ardından modeli Kokoro Studio'dan kurun.",
      "managed": "Yönetilen",
      "managedPath": "Yönetilen: {{path}}",
      "requestPathHint": "Sağlayıcı yolu OpenAI varsayılanından farklıysa kullanın",
      "verifying": "Doğrulanıyor..."
    }
  },
  "models": {
    "empty": {
      "title": "Henüz model yok",
      "description": "Farklı sağlayıcılardan yapay zeka modelleri ekle ve yönet",
      "addButton": "Model ekle"
    },
    "sort": {
      "title": "Modelleri sırala",
      "alphabetical": "Alfabetik",
      "alphabeticalDescription": "Model adına göre sırala",
      "byProvider": "Sağlayıcıya göre",
      "byProviderDescription": "Modelleri sağlayıcıya göre grupla"
    },
    "extra": {
      "cpuFallbackSucceeded": "Bu model son çalıştırmada CPU'ya geri düştü.",
      "cpuFallbackFailed": "Bu model son çalıştırmada başarısız oldu."
    },
    "labels": {
      "vision": "Görüntü",
      "audio": "Ses",
      "model": "Model"
    },
    "menu": {
      "editDescription": "Model parametrelerini yapılandır",
      "alreadyDefault": "Zaten varsayılan",
      "setAsDefault": "Varsayılan yap",
      "setAsDefaultDescription": "Bunu birincil modelin yap",
      "exportDescription": "Bu model profilini kaydet",
      "deleteTitle": "Model silinsin mi?",
      "deleteMessage": "{{name}} silinsin mi?",
      "deleteDescription": "Bu modeli kalıcı olarak kaldır"
    },
    "toasts": {
      "exportFailed": "Dışa aktarma başarısız",
      "importSuccessTitle": "Başarıyla içe aktarıldı",
      "importSuccessDescription": "\"{{name}}\" modeli içe aktarıldı.",
      "importFailed": "İçe aktarma başarısız"
    }
  },
  "installedModels": {
    "title": "Yerel GGUF envanteri",
    "fileCount": "{{count}} dosya",
    "searchPlaceholder": "Model adı, dosya adı, yol, kuantizasyon veya mimari ara",
    "loading": "Kurulu modeller taranıyor...",
    "loadFailed": "Kurulu modeller yüklenemedi: {{error}}",
    "empty": {
      "title": "Kurulu GGUF modeli bulunamadı",
      "description": "Önce tarayıcıdan modelleri indir veya `.gguf` dosyalarını models klasörüne yerleştir."
    },
    "columns": {
      "type": "Tür",
      "params": "Parametre",
      "arch": "Mimari",
      "context": "Bağlam",
      "size": "Boyut",
      "action": "İşlem"
    },
    "confirm": {
      "deleteTitle": "Model dosyasını sil",
      "deleteMessage": "{{filename}} silinsin mi? Bu işlem yalnızca models klasöründeki yerel GGUF dosyasını kaldırır."
    },
    "toasts": {
      "pathCopied": "Yol kopyalandı",
      "copyFailed": "Kopyalama başarısız",
      "modelDeleted": "Model silindi",
      "deleteFailed": "Silme başarısız"
    }
  },
  "editModel": {
    "setup": {
      "title": "Model kurulumu",
      "description": "Platformu seç, bu girişe okunabilir bir ad ver ve kullanmak istediğin model kimliğine veya dosyaya bağla.",
      "selectPlatform": "Platform seç"
    },
    "errors": {
      "loadFailed": "Model ayarları yüklenemedi"
    },
    "runtime": {
      "lastReport": "Son çalışma raporu",
      "gpuFallbackReason": "GPU geri düşme nedeni",
      "finalError": "Son hata",
      "workingRecoveryConfig": "Çalışan kurtarma yapılandırması",
      "context": "Bağlam",
      "batch": "Toplu iş",
      "na": "yok",
      "applyWorkingConfig": "Çalışan yapılandırmayı uygula",
      "badges": {
        "succeeded": "Çalıştırma başarılı",
        "cpuFallbackSucceeded": "CPU geri dönüşü kurtardı",
        "cpuFallbackFailed": "CPU geri dönüşü başarısız oldu",
        "failed": "Çalıştırma başarısız"
      },
      "headline": {
        "succeeded": "Son yerel çalıştırma başarıyla tamamlandı.",
        "cpuFallbackSucceeded": "GPU yüklemesi başarısız oldu, ardından model CPU'da toparlandı.",
        "cpuFallbackFailed": "GPU yüklemesi başarısız oldu ve CPU geri dönüşü de tamamlanamadı.",
        "failed": "Son yerel çalıştırma, llama.cpp tamamlanmadan önce başarısız oldu."
      },
      "detail": {
        "succeeded": "Bu rapor ayrıca akıllı yük boşaltma önbelleğini de besler, böylece gelecekteki çalıştırmalar son kararlı GPU kurulumunu yeniden kullanabilir.",
        "cpuFallbackSucceeded": "Çalışan CPU-güvenli bağlam ve toplu iş ayarlarını kaydettik; bunları yeniden kullanabilirsin.",
        "cpuFallbackFailed": "Model CPU'da yeniden denendi, ancak kurtarılan yapılandırma yine de başarısız oldu.",
        "failed": "Bu rapor, ne olduğunu inceleyebilmen için son bilinen çalışma zamanı durumunu tutar."
      }
    },
    "sections": {
      "generation": "Üretim parametreleri",
      "runtime": "Çalışma zamanı",
      "reasoning": "Akıl yürütme",
      "reasoningThinking": "Akıl yürütme (Düşünme)",
      "promptCaching": "Prompt önbellekleme",
      "capabilities": "Yetenekler"
    },
    "sectionDescriptions": {
      "generation": "Sıcaklık, örnekleme, cezalar ve çıktı sınırları.",
      "generationAutomatic1111": "Örnekleyici, CFG, seed, negatif prompt, denoise ve varsayılan boyutlar.",
      "runtime": "{{runtime}} çalıştırma, bellek ve donanım kontrolleri.",
      "reasoning": "Düşünme modu, çaba ve token bütçesi kontrolleri.",
      "promptCaching": "Bağlam yeniden kullanımı ve önek önbellekleme davranışı.",
      "capabilities": "Bu modelin desteklediği giriş ve çıkış kipleri.",
      "capabilitiesSimple": "Bu modelin hangi kipleri kabul ettiğini ve hangi çıktıları üretebildiğini işaretle."
    },
    "summaries": {
      "generationAutomatic1111": "Stable Diffusion örnekleyicisi, CFG, seed ve varsayılan boyutlar",
      "generationDefault": "Varsayılan örnekleme ve çıktı sınırları",
      "runtimeLlama": "Yürütme, bellek ve donanım varsayılanları",
      "runtimeOllama": "Ollama çalışma zamanı varsayılanları",
      "reasoningAlwaysEnabled": "Bu sağlayıcı için her zaman etkin",
      "reasoningDisabled": "Akıl yürütme devre dışı",
      "reasoningDefault": "Düşünme denetimleri sağlayıcının varsayılanlarında kalır",
      "textOnly": "Yalnızca metin",
      "capabilities": "Girdi: {{input}} • Çıktı: {{output}}"
    },
    "reasoning": {
      "enabled": "Etkin",
      "enabledDescription": "Düşünme sürecini göster",
      "effort": "Akıl yürütme çabası",
      "helpLabel": "Akıl yürütme modu yardımı",
      "budgetTokens": "Bütçe tokenleri",
      "providerDefault": "Sağlayıcı varsayılanı"
    },
    "runtimeFacts": {
      "updated": "Güncellendi",
      "modelPath": "Model yolu",
      "backendUsed": "Kullanılan arka uç",
      "failureStage": "Hata aşaması",
      "requestedContext": "İstenen bağlam",
      "recommendedContext": "Önerilen bağlam",
      "initialContext": "Başlangıç bağlamı",
      "actualContext": "Gerçek bağlam",
      "requestedGpuLayers": "İstenen GPU katmanları",
      "actualGpuLayers": "Gerçek GPU katmanları",
      "requestedBatch": "İstenen toplu iş",
      "initialBatch": "Başlangıç toplu işi",
      "actualBatch": "Gerçek toplu iş",
      "smartOffloadFallback": "Akıllı yük boşaltma geri dönüşü",
      "active": "Etkin",
      "notNeeded": "Gerekli değil",
      "kqvFallback": "KQV geri dönüşü",
      "movedToRam": "RAM'e taşındı",
      "smartOffloadEstimate": "Akıllı yük boşaltma tahmini",
      "smartOffloadCandidates": "Akıllı yük boşaltma adayları",
      "kvCache": "KV önbelleği",
      "kqvOffload": "KQV yük boşaltma",
      "flashAttention": "_Flash Attention_",
      "gpuBackends": "GPU arka uçları",
      "availableRam": "Kullanılabilir RAM",
      "availableVram": "Kullanılabilir VRAM",
      "modelSize": "Model boyutu",
      "promptTokens": "Prompt tokenleri",
      "promptPositions": "Prompt konumları",
      "targetNewTokens": "Hedef yeni tokenler",
      "completionTokens": "Tamamlama tokenleri",
      "finishReason": "Bitiş nedeni",
      "firstToken": "İlk token",
      "throughput": "Aktarım hızı",
      "promptTemplate": "Prompt şablonu"
    },
    "fields": {
      "platform": "Platform_",
      "displayName": "Görünen ad",
      "modelPath": "Model Yolu (GGUF)",
      "modelId": "Model Kimliği"
    },
    "placeholders": {
      "displayName": "örn. En Sevdiğim ChatGPT",
      "modelPath": "/path/to/model.gguf",
      "modelId": "örn. gpt-4o",
      "sdSteps": "28_",
      "sdCfgScale": "6.5",
      "sdSize": "1024x1024",
      "sdSampler": "_DPM++ 2M Karras_",
      "random": "Rastgele",
      "sdDenoise": "_0.75_",
      "sdNegativePrompt": "bulanık, düşük kalite, kötü anatomi, fazla parmak",
      "temperature": "_0.70",
      "topP": "1.00_",
      "zero": "0.00",
      "batch512": "512",
      "default": "Varsayılan",
      "mmprojPath": "/path/to/mmproj.gguf_",
      "stopSequences": "örn. \n\n###\nUser:\n"
    },
    "localLibrary": {
      "localPathHelp": "Yerel GGUF modeline giden tam dosya yolunu kullan.",
      "mmprojTitle": "İndirilen MMProj Dosyaları",
      "mmprojEmpty": "Henüz indirilmiş mmproj dosyası yok",
      "mmprojEmptyHint": "Model Tarayıcıdan çok kipli bir projektör indir veya manuel olarak yol gir."
    },
    "modelSource": {
      "useCatalog": "Kataloğu kullan",
      "enterManually": "Manuel gir",
      "refreshModelList": "Model listesini yenile",
      "onlyFreeModels": "yalnızca ücretsiz modeller",
      "customEndpointFetchDisabled": "Bu özel uç nokta için model çekme kapalı. Model listesi keşfi istiyorsan Sağlayıcı ayarlarında bunu etkinleştir ve bir Models Endpoint ayarla."
    },
    "promptCaching": {
      "automatic": {
        "title": "Otomatik sağlayıcı önbelleklemesi",
        "description": "Bu sağlayıcı, prompt önbelleklemesini üst API'de otomatik olarak yönetir. Uygulama tarafında gönderilecek bir geçiş yoktur."
      },
      "enableTitle": "Bağlam önbelleklemeyi etkinleştir",
      "enableDescription": "Etkileşimler arasında statik sistem promptlarını ve belge bağlamını koru.",
      "ttlTitle": "Önbellek TTL",
      "ttlDescription": "Önbelleğe alınmış öneklerin istekler arasında ne kadar süre geçerli kalacağı.",
      "pricingTitle": "Fiyatlandırma notu:",
      "pricingDescription": "Önbellekleme tekrar eden giriş tokenlerinin maliyetini düşürse de, önbelleğe ilk yazım seçilen sağlayıcıya bağlı olarak küçük bir ek ücret doğurabilir.",
      "oneHourNote": "Uzatılmış 1 saatlik TTL tüm sağlayıcılar tarafından desteklenmeyebilir. Desteklenmediğinde sağlayıcının varsayılan önbellek ömrüne geri düşer.",
      "openai24hNote": "OpenAI, 1 saatlik TTL yerine `in_memory` ve `24h` saklama politikalarını kullanır.",
      "groqLabel": "_Groq:_",
      "groqDescription": "önbellekleme yalnızca desteklenen modellerde otomatik olarak çalışır. Bu uygulama bunu istek bazında zorlamaz veya ayarlamaz.",
      "geminiLabel": "_İkizler:",
      "geminiDescription": "örtük önbellekleme desteklenen modellerde otomatik olarak çalışır. Açık önbellekli içerik kaynakları bu uygulama tarafından henüz oluşturulmaz.",
      "ttl": {
        "inMemory": "Bellek içi",
        "24h": "24 sa",
        "5min": "5 dk",
        "1h": "1 sa"
      }
    },
    "toasts": {
      "runtimeConfigApplied": "Çalışma zamanı yapılandırması uygulandı",
      "runtimeConfigAppliedDescription": "Gelecekteki yerel çalıştırmalar son CPU-güvenli bağlamı ve toplu işi yeniden kullanacak.",
      "modelPathRequired": "Model yolu gerekli",
      "modelPathRequiredDescription": "Gömülü şablonu okumadan önce bir GGUF model yolu seç.",
      "embeddedTemplatePasted": "Gömülü şablon düzenleyiciye yapıştırıldı"
    },
    "search": {
      "didYouMean": "Şunu mu demek istedin:"
    },
    "capabilities": {
      "helpLabel": "Yetenekler yardımı",
      "input": "Girdi",
      "output": "Çıktı",
      "automatic1111Fixed": "AUTOMATIC1111 modelleri metin + görsel girdi ve görsel çıktı olarak sabittir."
    },
    "runtimeSummary": {
      "ram": "RAM",
      "vram": "VRAM",
      "kvCacheInVram": "VRAM'de KV önbelleği",
      "kvCacheInRam": "RAM'de KV önbelleği"
    },
    "help": {
      "temperature": "Sıcaklık yardımı",
      "topP": "Top P yardımı",
      "maxOutputTokens": "Maksimum çıktı tokenleri yardımı",
      "topK": "Top K yardımı",
      "frequencyPenalty": "Frekans cezası yardımı",
      "presencePenalty": "Varlık cezası yardımı",
      "contextLength": "Bağlam uzunluğu yardımı"
    },
    "contextInfo": {
      "maxSupported": "Desteklenen maksimum",
      "recommended": "Önerilen",
      "availableRam": "Kullanılabilir RAM",
      "availableVram": "Kullanılabilir VRAM",
      "modelSize": "Model boyutu",
      "contextCache": "Bağlam önbelleği",
      "memoryFitness": "Bellek uygunluğu",
      "gpuAcceleration": "GPU hızlandırma",
      "kvHeadroom": "KV yedek alanı",
      "quantQuality": "Kuantizasyon kalitesi"
    },
    "llama": {
      "toggleStrictMode": "llama katı modunu aç/kapat"
    },
    "ollama": {
      "numCtxShort": "Sayı Ctx"
    },
    "continueSetup": {
      "continue": "Kuruluma devam et",
      "saveToContinue": "Devam etmek için bir model kaydet"
    },
    "generation": {
      "automatic1111Help": "AUTOMATIC1111 burada Stable Diffusion denetimlerini kullanır. Bu değerler avatarlar, sahne görselleri ve diğer yerel diffusion istekleri için varsayılan örnekleyici ayarları olur.",
      "formatWidthHeight": "Biçim: genişlik x yükseklik"
    },
    "generationDescriptions": {
      "sdSteps": "Diffusion örnekleme adımları",
      "sdCfgScale": "Prompt yönlendirme gücü",
      "sdSize": "İstek boyutu ezmediğinde kullanılır",
      "sdSampler": "A1111'e gönderilen örnekleyici adı",
      "sdSeed": "Rastgele üretimler için boş bırak",
      "sdDenoise": "Referans tabanlı üretimler için düzenleme gücü",
      "sdNegativePrompt": "Bu model için her AUTOMATIC1111 isteğine uygulanır",
      "temperature": "Yüksek = daha yaratıcı",
      "topP": "Düşük = daha odaklı",
      "maxOutputTokens": "Yanıt uzunluğunu sınırla",
      "topK": "İlk K token arasından örnekle",
      "frequencyPenalty": "Kelime tekrarını azalt",
      "presencePenalty": "Yeni konuları teşvik et"
    },
    "moveModel": {
      "title": "Model dosyasını taşı"
    },
    "parameterSupport": {
      "title": "Parametre desteği"
    },
    "editorMode": {
      "title": "Düzenleyici modu",
      "description": "Basit mod kapalı başlar. Gelişmiş mod mevcut tam düzenleyiciyi korur.",
      "simple": "Basit",
      "advanced": "Gelişmiş",
      "emptyState": "Ayarlarını düzenlemek için bir bölüm aç. Tam forma ihtiyaç duyduğunda gelişmiş düzenleyici kullanılabilir kalır."
    },
    "templateOverride": {
      "title": "Şablon geçersiz kılma",
      "hideEmbedded": "Gömülüyü gizle",
      "showEmbedded": "Gömülüyü göster",
      "close": "Kapat",
      "readingEmbedded": "Gömülü şablon okunuyor...",
      "readEmbeddedFailed": "Gömülü şablon okunamadı",
      "copiedToClipboard": "Panoya kopyalandı",
      "pasteIntoEditor": "Düzenleyiciye yapıştır",
      "jinjaTemplate": "Jinja Şablonu",
      "placeholder": "Bir Jinja sohbet şablonu veya dahili şablon adı gir..."
    }
  },
  "hfBrowser": {
    "title": "Model Tarayıcı",
    "searchPlaceholder": "HuggingFace'te GGUF modelleri ara...",
    "searching": "Aranıyor...",
    "trending": "Popüler GGUF Modelleri",
    "noResults": "Model bulunamadı",
    "noResultsHint": "Başka bir arama terimi dene veya popüler modellere göz at.",
    "likes": "beğeni",
    "downloads": "indirme",
    "viewFiles": "Dosyaları Görüntüle",
    "files": "Mevcut Dosyalar",
    "noFiles": "Bu depoda GGUF dosyası bulunamadı.",
    "architecture": "Mimari",
    "contextLength": "Bağlam Uzunluğu",
    "parameters": "Parametreler",
    "quantization": "Kuantizasyon",
    "fileSize": "Boyut",
    "download": "İndir",
    "downloading": "İndiriliyor...",
    "cancelDownload": "İndirmeyi İptal Et",
    "downloadComplete": "İndirme tamamlandı!",
    "downloadFailed": "İndirme başarısız",
    "downloadCancelled": "İndirme iptal edildi",
    "downloadProgress": "{{downloaded}} / {{total}}",
    "progress": "İlerleme",
    "fileOfTotal": "Dosya {{current}} / {{total}}",
    "speed": "{{speed}}/s",
    "alreadyDownloaded": "Zaten indirilmiş",
    "createModel": "Model Oluştur",
    "backToSearch": "Aramaya dön",
    "backToFiles": "Dosyalara dön",
    "sortTrending": "Popüler",
    "sortDownloads": "En Çok İndirilen",
    "sortLikes": "En Çok Beğenilen",
    "sortRecent": "Son Güncellenen",
    "browseOnHuggingFace": "HuggingFace'te Gözat",
    "selectFromLibrary": "Kütüphaneden Seç",
    "libraryEmpty": "Henüz indirilmiş model yok",
    "libraryEmptyHint": "Model Tarayıcıdan GGUF modelleri indir veya manuel olarak bir yol gir.",
    "libraryTitle": "İndirilmiş Modeller",
    "moveToLibrary": "Bu model dosyasını GGUF model klasörüne taşıyabilirim istersen. Böylece tüm modellerini tek bir yerde düzenli tutarsın.",
    "moveToLibraryYes": "Evet, taşı",
    "moveToLibraryNo": "Hayır, olduğu yerde bırak",
    "moveToLibraryMoving": "Model taşınıyor...",
    "moveToLibrarySuccess": "Model başarıyla taşındı!",
    "moveToLibraryFailed": "Model taşınırken hata oluştu",
    "runabilityExcellent": "Mükemmel!",
    "runabilityGood": "İyi",
    "runabilityMarginal": "Sınırda",
    "runabilityPoor": "Zayıf",
    "runabilityUnrunnable": "Çalıştırılamaz",
    "recommendedSettings": "Önerilen Ayarlar",
    "kvCacheType": "KV Cache Türü",
    "gpuFull": "Tam GPU yüklemesi",
    "gpuNearFull": "Neredeyse tam GPU, hafif KV taşması",
    "gpuKvSpill": "Ağırlıklar GPU'da, KV RAM'e taşıyor",
    "gpuKvHeavySpill": "Ağırlıklar GPU'da, KV büyük ölçüde RAM'de",
    "gpuMostLayers": "Katmanların çoğu GPU'da",
    "gpuHalfLayers": "Katmanların yarısı GPU'da",
    "gpuFewLayers": "Az sayıda katman GPU'da",
    "gpuCpu": "Yalnızca CPU",
    "notRecommended": "Bu modeli cihazında çalıştırmanı önermiyoruz. Sorunsuz çalışmayacaktır.",
    "moreDetails": "Daha fazla",
    "detailedReport": "Kaynak Raporu",
    "detailSystem": "Sistem Kaynakları",
    "detailRam": "Kullanılabilir RAM",
    "detailVram": "Kullanılabilir VRAM",
    "detailVramBudget": "VRAM Bütçesi (90%)",
    "detailTotalAvailable": "Toplam Kullanılabilir",
    "detailArchitecture": "Model Mimarisi",
    "detailArch": "Mimari",
    "detailLayers": "Katmanlar",
    "detailEmbedding": "Embedding Boyutu",
    "detailHeads": "Dikkat Başlıkları",
    "detailKvHeads": "KV Başlıkları",
    "detailFfn": "Feed-Forward Boyutu",
    "detailTrainCtx": "Eğitim Bağlamı",
    "detailConfig": "Mevcut Yapılandırma",
    "detailModelSize": "Model Dosya Boyutu",
    "detailMemory": "Bellek Dağılımı",
    "detailWeights": "Model Ağırlıkları",
    "detailKvCache": "KV Önbellek",
    "detailTotalNeeded": "Toplam Gerekli",
    "detailHeadroom": "Yedek Alan",
    "detailGpuFit": "GPU Yüklemesi",
    "detailScoreBreakdown": "Puan Dağılımı",
    "detailMemFitness": "Bellek Uygunluğu",
    "detailGpuAccel": "GPU Hızlandırma",
    "detailKvHeadroom": "KV Yedek Alanı",
    "detailQuantQuality": "Kuantizasyon Kalitesi",
    "detailFinalScore": "Ağırlıklı Puan",
    "detailComputeBuffer": "Hesaplama Tamponu",
    "detailMemMode": "Bellek Modu",
    "detailUnified": "Birleşik (RAM/VRAM paylaşımlı)",
    "detailSwa": "Kayan Pencere",
    "detailMlaRank": "MLA Gizli Sırası",
    "detailParseStatus": "Başlık Ayrıştırma",
    "detailIncomplete": "Tamamlanmamış (MoE meta verisi çok büyük)",
    "detailEffectiveKvCtx": "Etkin KV Bağlamı",
    "detailOffload": "GPU Yüklemesi",
    "detailCtxTip": "Bağlamı {{ctx}} tokene düşürmek %100 GPU yüklemesine olanak tanır.",
    "upgradeSuggestion": "{{quant}} ({{size}}) da sığar ve {{score}} puan alır — daha iyi kalite.",
    "layerTip": "Önerilen: {{layers}}/{{total}} katman yükle (-ngl {{layers}})",
    "detailKvDistribution": "KV Cache Dağılımı",
    "detailKvOnGpu": "GPU (VRAM)",
    "detailKvOnRam": "Sistem RAM'i",
    "kvDistributionTip": "KV cache'in %{{pct}}'i RAM'de. Prompt işleme (prefill) daha yavaş olacak — %100 GPU anlık tutar.",
    "detailLayers-ngl": "Yüklenecek Katmanlar (-ngl)",
    "detailOptimalGpuCtx": "Optimal GPU Bağlamı",
    "detailOptimalRamCtx": "Maks. RAM Bağlamı",
    "optimalGpuCtxLabel": "Tam GPU hızı: {{ctx}} token",
    "optimalRamCtxLabel": "Maks. RAM: {{ctx}} token",
    "optimalGpuCtxShort": "GPU: {{ctx}}",
    "optimalRamCtxShort": "Maks: {{ctx}}",
    "fullGpuOffloadShort": "%100 GPU: {{ctx}}",
    "ctxExceedsGpu": "Bağlam optimal GPU değerini ({{ctx}}) aşıyor. KV cache RAM'e taşacak ve hızı düşürecek.",
    "modelExceedsVram": "Model VRAM'i aşıyor. Kısmi GPU yüklemesiyle RAM'den çalıştırılıyor."
  },
  "systemPrompts": {
    "filters": {
      "all": "Tümü",
      "system": "Sistem",
      "internal": "Dahili",
      "custom": "Özel"
    },
    "empty": {
      "title": "Henüz özel prompt yok",
      "description": "Yapay zeka konuşmalarını kişiselleştirmek için özel sistem promptları oluştur",
      "createButton": "Prompt oluştur"
    },
    "preview": {
      "whatLlmSees": "LLM'nin gördükleri",
      "turns": "Turns",
      "noMessages": "Mesaj yok",
      "noMessagesHint": "_Giriş ekleyin veya dönüşleri artırın_",
      "showMore": "_Daha fazlasını göster_",
      "showLess": "Daha az göster",
      "statChat": "sohbet",
      "statInjected": "enjekte edildi",
      "statTotal": "toplam",
      "entry": "Giriş",
      "editEntry": "Girişi düzenle_",
      "reorder": "Yeniden sırala",
      "delete": "Sil",
      "deleteTitle": "Giriş silinsin mi?",
      "deleteMessage": "\"{{name}}\" istem şablonundan kaldırılsın mı? ",
      "deleteConfirm": "Sil",
      "thisEntry": "bu girişi",
      "condensedName": "Yoğunlaştırılmış Sistem İstemi",
      "imageAttachment": "[Resim eki: {{label}}]",
      "imageSlot": {
        "character": "Karakter referans resmi",
        "persona": "Kişi referans resmi",
        "chatBackground": "Sohbet arka plan resmi",
        "avatar": "Avatar resmi",
        "references": "Referans görselleri"
      },
      "injection": {
        "relative": "relative",
        "inChat": "inChat · derinlik {{depth}}",
        "conditional": "koşullu · minimum {{min}}",
        "interval": "aralık · her {{every}}"
      }
    }
  },
  "personas": {
    "empty": {
      "title": "Henüz persona yok",
      "description": "Yapay zekanın sana nasıl hitap edeceğini tanımlamak için bir persona oluştur",
      "createButton": "Persona oluştur"
    },
    "actions": {
      "editPersona": "Personayı düzenle",
      "setAsDefault": "Varsayılan olarak ayarla",
      "setAsDefaultDesc": "Tüm yeni sohbetler için bunu kullan",
      "unsetAsDefault": "Varsayılan olarak kaldır",
      "unsetAsDefaultDesc": "Varsayılan durumunu kaldır",
      "exportPersona": "Personayı dışa aktar",
      "deletePersona": "Personayı sil"
    },
    "edit": {
      "avatarHint": "Avatar eklemek veya oluşturmak için dokun",
      "nameLabel": "PERSONA ADI",
      "namePlaceholder": "ör. Profesyonel, Yaratıcı yazar, Öğrenci...",
      "nameHint": "Personana açıklayıcı bir ad ver",
      "nicknameLabel": "TAKMA AD (İSTEĞE BAĞLI)",
      "nicknamePlaceholder": "ör. İş Varyantı, RPG Modu...",
      "nicknameHint": "Bu personanın varyantlarını kütüphanede ayırt etmek için özel bir takma ad",
      "descriptionLabel": "AÇIKLAMA",
      "descriptionPlaceholder": "Yapay zekanın sana nasıl hitap etmesi gerektiğini, tercihlerini, geçmişini veya iletişim tarzını tanımla...",
      "wordCount": "kelime",
      "descriptionHint": "Nasıl muamele görmek istediğin konusunda spesifik ol",
      "setAsDefault": "Varsayılan olarak ayarla",
      "defaultDescription": "Tüm yeni konuşmalar için bu personayı kullan",
      "exportButton": "Personayı dışa aktar"
    },
    "designReferences": {
      "title": "Tasarım referansları",
      "description": "Sahne oluşturma için birkaç sabit görüntü referansı ve kısa bir tasarım notu ekleyin."
    },
    "create": {
      "namePlaceholderExample": "Profesyonel Yazar",
      "descriptionPlaceholderExample": "Profesyonel, net ve özlü bir üslupla yazın. "
    },
    "errors": {
      "exportFailed": "Kişi dışa aktarılamadı",
      "importFailed": "Kişi içe aktarılamadı",
      "loadFailed": "Kişi yüklenemedi",
      "saveFailed": "Kişi kaydedilemedi"
    },
    "importToast": {
      "legacyJsonTitle": "Eski JSON içe aktarımı algılandı",
      "legacyJsonMessage": "JSON içe aktarmaları kullanımdan kaldırıldı ve yakında kaldırılacak. ",
      "successMessage": "Persona imported successfully! "
    }
  },
  "security": {
    "pureMode": {
      "off": "Kapalı",
      "offDesc": "Tüm içeriğe izin verilir",
      "low": "Düşük",
      "lowDesc": "Cinsel içerik + küfürleri engeller",
      "standard": "Standart",
      "standardDesc": "NSFW + grafik şiddeti engeller",
      "strict": "Sıkı",
      "strictDesc": "Maksimum filtreleme + ima edici ton yok"
    }
  },
  "advanced": {
    "sectionTitles": {
      "aiFeatures": "Yapay zeka özellikleri",
      "memorySystem": "Hafıza sistemi",
      "usageAnalytics": "Kullanım analizi"
    },
    "creationHelper": {
      "title": "Oluşturma asistanı",
      "description": "Yapay zeka rehberliğinde karakter oluşturma asistanı"
    },
    "helpMeReply": {
      "title": "Yanıtlamamda yardım et",
      "description": "Yapay zeka destekli yanıt önerileri"
    },
    "dynamicMemory": {
      "title": "Dinamik hafıza",
      "contextWindow": "Bağlam penceresi",
      "contextWindowDesc": "Dahil edilecek son mesaj sayısı (1-1000)",
      "infoText": "Dinamik Hafıza, konuşma bağlamını otomatik olarak özetlemek ve yönetmek için yapay zeka kullanır, daha uzun ve tutarlı konuşmalar sağlar.",
      "disabledText": "Devre dışı bırakıldığında, uygulama Bağlam Penceresi ayarına göre basit bir kayan pencere kullanır."
    },
    "usageAnalytics": {
      "recalculateTitle": "Kullanım maliyetlerini yeniden hesapla",
      "recalculateDesc": "Tüm geçmiş kullanım kayıtlarını doğru fiyatlarla güncelle",
      "recalculating": "Yeniden hesaplanıyor...",
      "recalculateButton": "Tüm maliyetleri yeniden hesapla",
      "openRouterApiKeyRequired": "OpenRouter API anahtarı gerekli. Ayarlar → Sağlayıcılar'dan yapılandır.",
      "importantLabel": "Önemli:",
      "warningCannotUndo": "Bu işlem geri alınamaz",
      "warningMayTakeTime": "Çok sayıda kaydın varsa zaman alabilir",
      "warningOnlyOpenRouter": "Yalnızca tokenleri olan OpenRouter kayıtları güncellenecek",
      "warningExistingValues": "Mevcut maliyet değerlerinin üzerine yazılacak"
    },
    "extra": {
      "creationHelperDetail": "Kişilik özellikleri, geçmiş hikaye ve diyalog stili için akıllı öneriler alın",
      "helpMeReplyDetail": "Konuşma geçmişine dayalı bağlamsal yanıt seçenekleri oluşturun",
      "lorebookEntryGenerator": "Lorebook Giriş Oluşturucu_",
      "lorebookEntryDesc": "Seçilen sohbet mesajlarını dayanıklı lorebook girişlerine dönüştürün ve giriş yazma ve anahtar kelime oluşturma için taslak istemleri yapılandırın.",
      "companions": "Tamamlayıcılar",
      "companionModeDesc": "Yardımcı karakterler tarafından kullanılan duygu, varlık çıkarma ve bellek yönlendirmeye yönelik yerel analiz modellerini yönetin.",
      "companionSoulWriter": "Yoldaş Ruh Yazarı",
      "companionSoulDesc": "Companion Souls'un taslağını oluşturmak için kullanılan modeli, yedek modeli ve bilgi istemi şablonunu seçin. ",
      "network": "Ağ",
      "apiServer": "API Sunucusu",
      "apiServerDesc": "OpenAI uyumlu bir API sunucusu aracılığıyla modelleri açığa çıkarın",
      "apiServerRunning": "Sunucu şu anda çalışıyor"
    }
  },
  "backup": {
    "tabs": {
      "create": "Oluştur"
    },
    "create": {
      "newBackupButton": "Yeni yedekleme",
      "exportDescription": "Tüm verileri şifreleyerek dışa aktar",
      "createButton": "Yedekleme oluştur"
    },
    "restore": {
      "availableBackups": "Mevcut yedeklemeler",
      "browseFiles": "Dosyalara gözat",
      "noBackupsFound": "Yedekleme bulunamadı",
      "noBackupsDesc": "Bir yedekleme oluştur veya bulmak için \"Dosyalara gözat\"a dokun",
      "browseDesc": ".lettuce dosyası ara",
      "restoreDialogTitle": "Yedeklemeyi geri yükle",
      "deleteDialogTitle": "Yedeklemeyi sil",
      "embeddingPrompt": "Dinamik Bellek Yerleştirme",
      "downloadModel": "Modeli İndir",
      "disableAndContinue": "Devre Dışı Bırak ve Devam Et"
    },
    "extra": {
      "successMessage": "Yedekleme oluşturuldu!",
      "savedLocation": "İndirilenlere Kaydedildi"
    }
  },
  "reset": {
    "title": "Her şeyi sıfırla",
    "description": "Bu, bu cihazdaki tüm sağlayıcıları, modelleri, karakterleri, sohbet oturumlarını ve tercihleri kalıcı olarak silecek.",
    "warning": "Bu işlem geri alınamaz",
    "resetButton": "Tüm verileri sıfırla",
    "confirmTitle": "Emin misin?",
    "confirmDescription": "Tüm verilerin kalıcı olarak silinecek. Uygulama ilk kurulum durumuna dönecek.",
    "confirmButton": "Evet, her şeyi sıfırla"
  },
  "chatAppearance": {
    "typography": "Tipografi",
    "fontSize": {
      "label": "Yazı tipi boyutu",
      "small": "Küçük",
      "medium": "Orta",
      "large": "Büyük",
      "xLarge": "Çok büyük"
    },
    "lineSpacing": {
      "label": "Satır aralığı",
      "tight": "Sıkı",
      "normal": "Normal_",
      "relaxed": "Geniş"
    },
    "messageBubbles": {
      "label": "Mesaj balonları",
      "style": {
        "label": "Stil",
        "bordered": "Kenarlıklı",
        "filled": "Dolgulu",
        "minimal": "Minimal"
      },
      "cornerRadius": {
        "label": "Köşe yarıçapı",
        "sharp": "Keskin",
        "rounded": "Yuvarlak",
        "pill": "Hap"
      },
      "maxWidth": {
        "label": "Maksimum genişlik",
        "compact": "Kompakt",
        "normal": "Normal",
        "wide": "Geniş"
      },
      "padding": {
        "label": "İç boşluk",
        "compact": "Kompakt",
        "normal": "Normal",
        "spacious": "Geniş"
      }
    },
    "layout": {
      "label": "Düzen",
      "messageSpacing": "Mesaj aralığı",
      "tight": "Sıkı",
      "normal": "Normal",
      "relaxed": "Geniş"
    },
    "avatar": {
      "shape": {
        "label": "Avatar şekli",
        "circle": "Daire",
        "rounded": "Yuvarlak",
        "hidden": "Gizli"
      },
      "size": {
        "label": "Avatar boyutu",
        "small": "Küçük",
        "medium": "Orta",
        "large": "Büyük"
      }
    },
    "colors": {
      "label": "Renkler",
      "userBubble": "Kullanıcı balonu rengi",
      "assistantBubble": "Asistan balonu rengi",
      "userBubbleHex": "Kullanıcı balonu özel hex",
      "assistantBubbleHex": "Asistan balonu özel hex",
      "neutral": "Nötr",
      "accent": "Vurgu",
      "info": "Bilgi",
      "secondary": "İkincil",
      "warning": "Uyarı",
      "textColors": "Metin Renkleri",
      "messageTextHex": "Satır içi alıntı rengi",
      "plainTextHex": "Düz Metin Rengi",
      "italicTextHex": "İtalik Metin Rengi",
      "quotedTextHex": "Blok alıntı rengi",
      "inlineCodeTextHex": "Satır içi kod rengi"
    },
    "backgroundTransparency": {
      "label": "Arka plan ve şeffaflık",
      "backgroundDim": "Arka plan koyulaştırma",
      "backgroundBlur": "Arka plan bulanıklığı",
      "bubbleBlur": "Balon bulanıklığı",
      "none": "Yok",
      "light": "Hafif",
      "medium": "Orta",
      "heavy": "Güçlü",
      "bubbleOpacity": "Balon saydamlığı"
    },
    "textColorMode": {
      "label": "Metin renk modu",
      "auto": "Otomatik",
      "light": "Açık",
      "dark": "Koyu"
    },
    "preview": {
      "label": "Önizleme",
      "generic": "Genel",
      "live": "Canlı"
    },
    "extra": {
      "reset": "Sıfırla"
    }
  },
  "colorCustomization": {
    "tokens": {
      "surface": "Yüzey",
      "surfaceDesc": "Sayfa arka planları",
      "surfaceEl": "Yükseltilmiş yüzey",
      "surfaceElDesc": "Kartlar, modaller, yükseltilmiş elemanlar",
      "nav": "Gezinme",
      "navDesc": "Üst ve alt çubuklar",
      "foreground": "Ön plan",
      "foregroundDesc": "Kenarlıklar, katmanlar, gezinme ve arayüz elemanları",
      "appText": "Uygulama metni",
      "appTextDesc": "Ana metin ve arayüz etiketleri",
      "appTextMuted": "Soluk metin",
      "appTextMutedDesc": "İkincil metin ve destekleyici metin",
      "appTextSubtle": "İnce metin",
      "appTextSubtleDesc": "İpuçları, yardım metni ve yer tutucular",
      "accent": "Vurgu",
      "accentDesc": "Ana eylemler, başarı",
      "info": "Bilgi",
      "infoDesc": "Bilgi durumları, bağlantılar",
      "warning": "Uyarı",
      "warningDesc": "Dikkat durumları, uyarılar",
      "danger": "Tehlike",
      "dangerDesc": "Yıkıcı eylemler, hatalar",
      "secondary": "İkincil",
      "secondaryDesc": "Yapay zeka özellikleri, yaratıcı araçlar"
    },
    "presetsLabel": "Hazır ayarlar",
    "customPresetsLabel": "Özel hazır ayarlar",
    "previewLabel": "Önizleme",
    "settingsCardsLabel": "Ayar kartları",
    "settingsCardsOpacity": "Kart saydamlığı",
    "settingsCardsOpacityDesc": "Ayar kartlarının ve liste satırlarının ne kadar şeffaf göründüğünü kontrol eder.",
    "importButton": "İçe aktar",
    "exportButton": "Dışa aktar",
    "resetAllButton": "Tümünü sıfırla",
    "presets": {
      "defaultDark": "Varsayılan koyu",
      "midnightBlue": "Gece mavisi",
      "warmEarth": "Sıcak toprak",
      "purpleHaze": "Mor sis",
      "rosePine": "Gül Çamı",
      "tokyoNight": "Tokyo Gecesi",
      "catppuccin": "Catppuccin",
      "gruvbox": "Gruvbox",
      "nord": "Nord",
      "dracula": "Dracula",
      "solarized": "Solarized",
      "ayuDark": "Ayu Dark",
      "oneDark": "One Dark",
      "vesper": "Vesper",
      "cyberNeon": "Cyber Neon",
      "monochrome": "Monokrom"
    },
    "groups": {
      "backgrounds": "Arka planlar",
      "content": "İçerik",
      "semantic": "Anlamsal"
    },
    "extra": {
      "surface": "Yüzey",
      "surfaceDesc": "Sayfa arka planları",
      "surfaceEl": "Yükseltilmiş Yüzey",
      "surfaceElDesc": "Kartlar, modeller, yükseltilmiş öğeler",
      "nav": "Navigasyon_",
      "navDesc": "Üst ve alt çubuklar",
      "fg": "Ön plan",
      "fgDesc": "Kenarlıklar, kaplamalar, gezinme, kullanıcı arayüzü kromu",
      "appText": "Uygulama Metni",
      "appTextDesc": "_Birincil metin ve arayüz etiketleri_",
      "appTextMuted": "_Sessiz Metin",
      "appTextMutedDesc": "_İkincil ",
      "appTextSubtle": "İnce Metin",
      "appTextSubtleDesc": "İpuçları, yardımcı metin, yer tutucular",
      "accent": "Aksan",
      "accentDesc": "_Birincil eylemler, başarı_",
      "info": "_Bilgi_",
      "infoDesc": "Bilgi durumları, bağlantılar",
      "warning": "Uyarı",
      "warningDesc": "Dikkat durumları, uyarılar",
      "danger": "Tehlike",
      "dangerDesc": "Yıkıcı eylemler, hatalar_",
      "secondary": "_İkincil_"
    }
  },
  "dynamicMemory": {
    "page": {
      "info": "Dinamik Hafıza, bağlamı verimli bir şekilde korumak için konuşmaları otomatik olarak özetler. Bir hazır ayar seç veya ihtiyaçlarına göre ayarları yapılandır.",
      "disabledDirectTitle": "Doğrudan sohbetler için dinamik hafıza devre dışı",
      "disabledDirectDescription": "Etkinleştirmek için Doğrudan Sohbetler sekmesindeki anahtarı aç. Grup sohbetleri oturum başına hafıza modunu kullanır.",
      "directChats": "Doğrudan Sohbetler",
      "groupChats": "Grup Sohbetleri",
      "enableDirectChats": "Doğrudan Sohbetler için Etkinleştir",
      "groupChatsInfo": "Grup sohbetleri oturum başına hafıza modunu kullanır. Her grubun ayarlarından dinamik hafızayı etkinleştir. Bu ayarlar dinamik hafızanın nasıl davrandığını kontrol eder.",
      "memoryProfile": "Hafıza Profili",
      "customSettings": "Özel yapılandırma — değerleri aşağıdaki Gelişmiş Seçenekler'den ayarla.",
      "contextEnrichment": "Bağlam Zenginleştirme",
      "experimental": "Deneysel",
      "contextEnrichmentDescription": "Daha akıllı hafıza geri çağırma için son mesajları kullan",
      "advancedOptions": "Gelişmiş Seçenekler",
      "advancedOptionsDescription": "Hafıza davranışında ince ayar",
      "summaryInterval": "Özet Aralığı",
      "summaryIntervalDescription": "Özetler arası mesaj sayısı",
      "maxMemoryEntries": "Maksimum Hafıza Girdisi",
      "maxMemoryEntriesDescription": "Depolanan maksimum anı sayısı",
      "hotMemoryBudget": "Aktif Hafıza Bütçesi",
      "hotMemoryBudgetDescription": "Aktif anılar için token limiti",
      "relevanceThreshold": "İlgi Eşiği",
      "relevanceThresholdDescription": "Geri çağırma için minimum benzerlik",
      "retrievalMode": "Geri Çağırma Modu",
      "retrievalModeSmart": "Akıllı",
      "retrievalModeCosine": "Kosinüs",
      "retrievalModeDescription": "Akıllı, ilgiyi frekans/yakınlıkla birleştirir. Kosinüs en yüksek saf benzerliği kullanır.",
      "retrievalLimit": "Geri Çağırma Limiti",
      "retrievalLimitDescription": "Tur başına seçilen maksimum anı",
      "decayRate": "Azalma Oranı",
      "decayRateDescription": "Önemin ne kadar hızlı azaldığı",
      "coldStorageThreshold": "Soğuk Depolama Eşiği",
      "coldStorageThresholdDescription": "Anıların arşive taşınma zamanı",
      "sharedSettings": "Paylaşılan Ayarlar",
      "summarisationModel": "Özetleme Modeli",
      "selectedModel": "Seçili Model",
      "useGlobalDefaultModel": "Global varsayılan modeli kullan",
      "noModelsAvailable": "Kullanılabilir model yok",
      "summarisationModelDescription": "Konuşma özetleme için kullanılır",
      "modelManagement": "Model Yönetimi",
      "testModel": "Modeli Test Et",
      "downloadModel": "Modeli İndir",
      "delete": "Sil",
      "embeddingModel": "Embedding Modeli",
      "tokenCapacity": "Token Kapasitesi",
      "tokenCapacityDescription": "Yüksek değerler = uzun konuşmalar için daha iyi hafıza",
      "keepModelLoaded": "Modeli Yüklü Tut",
      "keepModelLoadedDescription": "Yeniden yükleme yükünü önlemek için embedding modeli + tokenizeri bellekte tutar",
      "installedModel": "Kurulu model: {{version}} (maks. {{tokens}} token)",
      "downloadEmbeddingModel": "Embedding Modeli İndir",
      "downloadEmbeddingDescription": "Hangi sürümü indireceğini seç. Kurulu sürümler devre dışıdır.",
      "downloadVersion": "{{version}} İndir",
      "downloadV2Description": "Doğruluk ve uzun bağlam geri çağırma için optimize edilmiş",
      "downloadV3Description": "En son embedding kalitesi",
      "installed": "Kurulu",
      "selectModel": "Model Seç",
      "searchModels": "Model ara...",
      "deleteEmbeddingTitle": "{{version}} modeli silinsin mi?",
      "deleteEmbeddingMessage": "{{version}} silmek istediğinden emin misin? Daha sonra tekrar indirebilirsin.",
      "msgsUnit": "msj",
      "entriesUnit": "girdi",
      "tokensUnit": "token",
      "itemsUnit": "öğe",
      "perCycleUnit": "/ döngü"
    },
    "presets": {
      "minimal": "minimal",
      "balanced": "dengeli",
      "comprehensive": "kapsamlı",
      "minimalDesc": "Hızlı ve verimli. ",
      "balancedDesc": "Bağlamın korunması ve performansın iyi bir karışımı.",
      "comprehensiveDesc": "Maksimum bağlam. "
    },
    "presetInfo": {
      "minimal": "Hızlı ve verimli. Yalnızca temel anıları korur.",
      "balanced": "İyi bir bağlam tutma ve performans dengesi.",
      "comprehensive": "Maksimum bağlam. Uzun ve detaylı konuşmalar için ideal."
    }
  },
  "helpMeReply": {
    "page": {
      "info": "Yanıtlamamda Yardım Et, konuşma geçmişine dayalı olarak bir sonraki mesajın için bağlamsal öneriler oluşturur. Modeli ve yanıt tarzını aşağıda yapılandır."
    },
    "sectionTitles": {
      "modelConfiguration": "Model yapılandırması",
      "responseStyle": "Yanıt tarzı"
    },
    "labels": {
      "replyModel": "Yanıt Modeli",
      "selectedModel": "Seçili Model",
      "useAppDefault": "Uygulama varsayılanını kullan{{model}}",
      "useAppDefaultBase": "Uygulama varsayılanını kullan",
      "noModelsAvailable": "Kullanılabilir model yok",
      "replyModelDescription": "Yanıt önerileri oluşturmak için yapay zeka modeli",
      "streamingOutput": "Akış Çıktısı",
      "streamingDescription": "Öneriler oluşturulurken göster",
      "maxTokens": "Maksimum Token",
      "maxTokensDescription": "Önerilerin maksimum uzunluğu",
      "conversationalHint": "Öneriler, sıradan sohbetlere uygun doğal diyalog olarak yazılacak.",
      "roleplayHint": "Öneriler, *eylemler* ve anlatım açıklamaları gibi roleplay öğeleri içerecek.",
      "footerInfo": "Bu yapılandırma tüm konuşmalarda genel olarak uygulanır. Daha az token daha kısa ve hızlı öneriler, daha fazla token daha detaylı yanıtlar sağlar.",
      "selectReplyModel": "Yanıt Modeli Seç",
      "searchModels": "Model ara..."
    },
    "responseStyle": {
      "conversational": "Konuşma",
      "conversationalDesc": "Doğal ve rahat ton",
      "roleplay": "Rol Oyunu",
      "roleplayDesc": "Karakter içi eylemler"
    },
    "extra": {
      "conversational": "Konuşmalı",
      "roleplay": "Rol Oyunu"
    }
  },
  "imageGeneration": {
    "promptPlaceholder": "Oluşturmak istediğin görseli tanımla...",
    "labels": {
      "model": "MODEL",
      "prompt": "PROMPT",
      "size": "BOYUT",
      "quality": "KALİTE",
      "style": "STİL",
      "searchModels": "Model ara...",
      "selectAvatarModel": "Avatar modeli seç",
      "selectSceneModel": "Sahne modeli seç",
      "selectWriterModel": "Sahne yazarı modeli seç",
      "useFirstAvailable": "İlk uygun modeli kullan",
      "useFirstCompatible": "İlk uyumlu yazar modelini kullan"
    },
    "mode": {
      "title": "Davranış",
      "description": "Model çıktısında algılanan sahne promptlarının nasıl işleneceğini seç.",
      "auto": "Otomatik",
      "autoDescription": "Model bir sahne promptu sağladığında sahne görselini hemen oluşturur.",
      "askFirst": "Önce sor",
      "askFirstDescription": "Algılanan sahne promptunu gösterir ve görsel oluşturmadan önce onayını bekler.",
      "manual": "Manuel",
      "manualDescription": "Model yanıtlarındaki sahne promptlarını yok sayar. Yalnızca kullanıcı tarafından başlatılan eylemleri kullanır."
    },
    "empty": {
      "title": "Görsel modeli yok",
      "description": "Görsel oluşturmaya başlamak için Modeller sayfasından bir görsel oluşturma modeli ekle."
    },
    "sections": {
      "avatar": {
        "title": "Avatar Oluşturma",
        "description": "Avatar seçiciden veya ilgili profil görseli akışlarından avatar oluştururken kullanılan varsayılan model."
      },
      "scene": {
        "title": "Sahne oluşturma",
        "description": "Konuşma bağlamından veya sahne ipuçlarından oluşturulan sahne görselleri için ayrılmış model."
      },
      "writer": {
        "title": "Sahne yazarı",
        "description": "Sohbet bağlamından, avatarlardan ve referans görsellerinden sahne promptları ve tasarım referansı açıklamaları yazmak için ayrılmış çok modlu metin modeli."
      }
    },
    "extra": {
      "avatarGeneration": "Avatar Generation",
      "sceneGeneration": "Sahne Oluşturma_",
      "sceneWriter": "Sahne Yazarı"
    }
  },
  "logs": {
    "sectionTitles": {
      "diagnostics": "Tanılama",
      "generate": "Oluştur",
      "copy": "Kopyala"
    },
    "extra": {
      "debug": "DEBUG",
      "info": "INFO_",
      "warn": "WARN",
      "error": "ERROR"
    }
  },
  "developer": {
    "sectionTitles": {
      "testDataGenerators": "Test veri oluşturucuları",
      "storageMaintenance": "Depolama bakımı",
      "usageTracking": "Kullanım takibi",
      "crashTesting": "Çökme testi",
      "environmentInfo": "Ortam bilgisi"
    },
    "testData": {
      "generateCharacter": "Test karakteri oluştur",
      "generateCharacterDesc": "Tek bir test karakteri oluştur",
      "generatePersona": "Test personası oluştur",
      "generatePersonaDesc": "Tek bir test personası oluştur",
      "generateSession": "Test oturumu oluştur",
      "generateSessionDesc": "Mevcut bir karakterle test sohbet oturumu oluştur",
      "generateBulk": "Toplu test verisi oluştur",
      "generateBulkDesc": "3 karakter ve 2 persona oluştur"
    },
    "storageMaintenance": {
      "optimizeDb": "Veritabanını optimize et",
      "optimizeDbDesc": "PRAGMA'ları uygula ve VACUUM çalıştır (yalnızca mobil)",
      "backupLegacy": "Eski dosyaları yedekle ve sil",
      "backupLegacyDesc": "Eski .bin depolamayı yedekleme klasörüne taşır"
    },
    "usageTracking": {
      "recalculateAll": "Tüm kullanım maliyetlerini yeniden hesapla",
      "recalculateAllDesc": "Güncel fiyatları alır ve tüm OpenRouter kullanım kayıtları için maliyetleri yeniden hesaplar"
    },
    "crashTesting": {
      "forceCrash": "Uygulamayı şimdi çökert",
      "forceCrashDesc": "Çökme algılamayı test etmek için yerel uygulama sürecini hemen sonlandırır.",
      "forceCrashConfirm": "Bu, çökme algılayıcıyı test etmek için uygulamayı hemen çökertecek. Devam edilsin mi?"
    },
    "environmentInfo": {
      "mode": "Mod",
      "devMode": "Geliştirici modu",
      "viteVersion": "Vite sürümü"
    },
    "status": {
      "testCharacterCreated": "✓ Test karakteri başarıyla oluşturuldu",
      "testPersonaCreated": "✓ Test personası başarıyla oluşturuldu",
      "testSessionCreated": "✓ Test oturumu oluşturuldu: {{id}}",
      "generatingBulkData": "Toplu test verisi oluşturuluyor...",
      "bulkDataCreated": "✓ Toplu test verisi oluşturuldu: 3 karakter, 2 persona",
      "creatingBenchmarkChat": "Başlangıç verili referans karakter ve oturumu oluşturuluyor...",
      "seededBenchmarkReady": "✓ Başlangıç verili referans hazır: {{name}} / {{id}}",
      "creatingBenchmarkGroupChat": "Başlangıç verili referans grup sohbeti oluşturuluyor...",
      "seededGroupBenchmarkReady": "✓ Başlangıç verili grup referansı hazır: {{id}}",
      "dbOptimized": "✓ Veritabanı optimize edildi",
      "recalculatingCosts": "Kullanım maliyetleri yeniden hesaplanıyor... Bu biraz zaman alabilir.",
      "toursReset": "✓ Tüm rehberli turlar sıfırlandı — bir sonraki ziyarette tekrar gösterilecek",
      "crashingApp": "Uygulama kapatılıyor..."
    },
    "errors": {
      "noCharacters": "Kullanılabilir karakter yok. Önce bir test karakteri oluştur.",
      "createCharacterFailed": "Test karakteri oluşturulamadı: {{error}}",
      "createPersonaFailed": "Test personası oluşturulamadı: {{error}}",
      "createSessionFailed": "Test oturumu oluşturulamadı: {{error}}",
      "createBulkFailed": "Toplu test verisi oluşturulamadı: {{error}}",
      "createBenchmarkFailed": "Referans oturumu oluşturulamadı: {{error}}",
      "createGroupBenchmarkFailed": "Grup referans oturumu oluşturulamadı: {{error}}",
      "dbOptimizeFailed": "Veritabanı optimizasyonu başarısız: {{error}}",
      "backupFailed": "Yedekleme hatası: {{error}}",
      "openRouterKeyMissing": "OpenRouter API anahtarı bulunamadı. Önce Ayarlar > Sağlayıcılar'dan yapılandır.",
      "recalculationFailed": "Yeniden hesaplama hatası: {{error}}",
      "resetToursFailed": "Turlar sıfırlanırken hata: {{error}}",
      "crashFailed": "Uygulama kapatılırken hata: {{error}}"
    },
    "onboarding": {
      "title": "Tanıtım",
      "resetTours": "Tüm rehberli turları sıfırla",
      "resetToursDesc": "Her turun görüntülenme durumunu temizler, böylece bir sonraki ziyarette tekrar oynatılır."
    },
    "benchmarks": {
      "createChat": "Başlangıç verili referans sohbet oluştur",
      "createChatDesc": "Dinamik hafızalı, başlangıç sahneli bir karakter ve 20 mesajlık süreklilik test oturumu oluşturur, sonra açar.",
      "createGroupChat": "Başlangıç verili referans grup sohbeti oluştur",
      "createGroupChatDesc": "Üç referans karakter ve 30 başlangıç mesajıyla dinamik hafızalı bir grup sohbeti oluşturur, sonra açar."
    },
    "extra": {
      "testCharacter": "_Test Karakteri_",
      "testCharacterDesc": "_Geliştirme amacıyla oluşturulmuş bir test karakteri._",
      "testScene": "_Geliştirme için basit bir test sahnesi",
      "testPersona": "Test Kişisi",
      "testPersonaDesc": "Geliştirme için bir test kişiliği",
      "successChar": "_✓ Test karakteri başarıyla oluşturuldu",
      "successPersona": "✓ Test kişiliği başarıyla oluşturuldu",
      "successSession": "_✓ Test oturumu oluşturuldu: {{id}}",
      "successBulk": "_✓ Toplu test verileri oluşturuldu: 3 karakter, 2 kişi",
      "errorCharAvailable": "Kullanılabilir karakter yok. ",
      "generatingBulk": "Toplu test verileri oluşturuluyor..."
    }
  },
  "embeddingDownload": {
    "capacityOptions": {
      "oneK": "1K token",
      "oneKDesc": "Hızlı yanıtlar için en iyisi",
      "twoK": "2K token",
      "twoKDesc": "Dengeli performans",
      "fourK": "4K token",
      "fourKDesc": "Maksimum bağlam"
    },
    "extra": {
      "status": "İndiriliyor..."
    }
  },
  "embeddingTest": {
    "categories": {
      "semanticSimilarity": "Anlamsal benzerlik",
      "dissimilarityCheck": "Farklılık kontrolü",
      "roleplayContext": "Roleplay bağlamı"
    },
    "extra": {
      "placeholder": "Yerleştirilecek metni girin..."
    }
  },
  "discovery": {
    "tabs": {
      "forYou": "Senin için",
      "trending": "Popüler",
      "popular": "En çok kullanılan",
      "new": "Yeni"
    },
    "searchPlaceholder": "Karakter ara...",
    "viewAll": "Tümünü gör",
    "errorTitle": "Bir şeyler yanlış gitti",
    "noCardsFound": "Kart bulunamadı",
    "sections": {
      "trendingNow": "Şu an popüler",
      "trendingSubtitle": "Bu hafta en çok aranan",
      "mostPopular": "En popüler",
      "popularSubtitle": "Topluluk favorileri",
      "freshArrivals": "Yeni gelenler",
      "freshSubtitle": "Yeni eklenenler"
    },
    "browse": {
      "newArrivals": "Yeni gelenler",
      "freshCharacters": "Yeni karakterler",
      "noCharactersFound": "Karakter bulunamadı",
      "noCharactersSubtitle": "Yeni içerik için daha sonra tekrar gel"
    },
    "sort": {
      "mostLiked": "En çok beğenilen",
      "mostDownloaded": "En çok indirilen",
      "mostViewed": "En çok görüntülenen",
      "mostMessages": "En çok mesaj",
      "newestFirst": "En yeni önce",
      "recentlyUpdated": "Son güncellenen",
      "nameAZ": "Ad (A-Z)"
    },
    "sortBy": "Sırala",
    "resultsUnit": "karakter",
    "detail": {
      "share": "Paylaş",
      "nsfwOverlay": "NSFW İçerik",
      "nsfwBadge": "NSFW",
      "originalBadge": "Orijinal",
      "lorebookBadge": "Lore kitabı",
      "alsoKnownAs": "Diğer adı:",
      "followersUnit": "takipçi",
      "sections": {
        "description": "Açıklama",
        "tokenUsage": "Token kullanımı",
        "startingScenes": "Başlangıç sahneleri",
        "scenario": "Senaryo",
        "personality": "Kişilik",
        "stats": "İstatistikler",
        "tags": "Etiketler",
        "author": "Yazar"
      },
      "tokensTotalLabel": "toplam",
      "tokens": {
        "description": "Açıklama",
        "personality": "Kişilik",
        "scenario": "Senaryo",
        "firstMessage": "İlk mesaj",
        "scenes": "Sahneler",
        "examples": "Örnekler",
        "systemPrompt": "Sistem promptu"
      },
      "sceneLabels": {
        "primary": "Ana",
        "alternate": "Alternatif"
      },
      "stats": {
        "views": "Görüntülenme",
        "downloads": "İndirme",
        "messages": "Mesaj"
      },
      "downloaded": "İndirildi",
      "startChat": "Sohbet başlat",
      "downloadCharacter": "Karakteri indir",
      "downloading": "İndiriliyor...",
      "downloadSuccess": {
        "title": "Karakter indirildi!",
        "subtitle": "Kütüphanene eklendi",
        "badge": "Kaydedildi",
        "startChat": "Sohbet başlat",
        "startChatDesc": "İlk sahneyi şimdi aç",
        "viewLibrary": "Kütüphanede görüntüle",
        "viewLibraryDesc": "Daha sonra düzenle, yönet veya dışa aktar",
        "continueBrowsing": "Keşfetmeye devam et",
        "continueBrowsingDesc": "Keşfete dön"
      },
      "errorTitle": "Hata",
      "errorSubtitle": "Yükleme hatası",
      "errorNotFound": "Karakter bulunamadı",
      "defaultChatTitle": "Yeni Sohbet"
    },
    "search": {
      "placeholder": "Karakter, etiket, yazar ara...",
      "resultsUnit": "sonuç",
      "timingUnit": "ms",
      "recentSearches": "Son aramalar",
      "clearAll": "Tümünü temizle",
      "trendingSearches": "Popüler aramalar",
      "trends": {
        "anime": "anime",
        "fantasy": "fantezi",
        "romance": "romantik",
        "villain": "kötü karakter",
        "adventure": "macera",
        "comedy": "komedi",
        "mystery": "gizem",
        "sciFi": "bilim kurgu"
      },
      "tips": {
        "title": "Arama ipuçları",
        "tip1": "Karakter adı, yazar veya açıklamaya göre ara",
        "tip2": "\"anime\", \"fantezi\" veya \"romantik\" gibi etiketler kullan",
        "tip3": "\"tsundere\" veya \"kötü karakter\" gibi spesifik özellikler dene"
      },
      "loading": "Yükleniyor...",
      "loadMore": "Daha fazla yükle",
      "noResults": "Sonuç bulunamadı",
      "noResultsFor": "Şunun için karakter bulunamadı:",
      "noResultsHint": "Farklı anahtar kelimeler dene veya kategorilere göz at"
    },
    "errors": {
      "loadContent": "İçerik yüklenemedi",
      "searchFailed": "Arama başarısız oldu_",
      "noCardPath": "Kart yolu sağlanmadı_",
      "loadCharacter": "Karakter yüklenemedi",
      "downloadCharacter": "Karakter indirilemedi"
    },
    "card": {
      "byAuthor": "by {{author}}",
      "ocBadge": "OC"
    }
  },
  "engine": {
    "gpuInsufficient": "Yetersiz GPU belleği",
    "gpuFallbackDesc": "Bu model GPU belleğine sığmıyor. CPU'ya geçilsin (daha yavaş) mi yoksa iptal mi?",
    "switchToCpu": "CPU'ya geç",
    "abort": "İptal",
    "errors": {
      "providerNotFound": "Motor sağlayıcısı bulunamadı.",
      "engineOffline": "Motor çevrimdışı veya ulaşılamıyor.",
      "deleteCharacterFailed": "Karakter silinemedi.",
      "unknownCharacter": "Bilinmiyor",
      "seedRequired": "Kaynak açıklaması gerekli.",
      "characterNameRequired": "Karakter adı gerekli.",
      "atLeastOneProvider": "En az bir sağlayıcı etkinleştirilmelidir.",
      "enableLlmProvider": "Lütfen en az bir LLM sağlayıcısını etkinleştirin.",
      "modelRequired": "{{provider}} için model gereklidir.",
      "apiKeyRequired": "API anahtarı {{provider}} için gereklidir.",
      "sendMessageFailed": "Mesaj gönderilemedi."
    },
    "status": {
      "connected": "Bağlı",
      "offline": "Çevrimdışı",
      "needsSetup": "Kurulum gerekli"
    },
    "home": {
      "characters": "Karakterler",
      "newButton": "Yeni",
      "noCharactersFound": "Karakter bulunamadı.",
      "tokenUsage": "Token kullanımı",
      "totalTokens": "toplam token",
      "backgroundActivity": "Arka plan etkinliği",
      "quickActions": "Hızlı eylemler",
      "configureProviders": "Sağlayıcıları yapılandır",
      "engineSettings": "Motor ayarları",
      "chat": "Sohbet",
      "chatDesc": "Bu karakterle bir konuşma başlat",
      "deleteCharacter": "Karakteri sil",
      "deletingCharacter": "Siliniyor...",
      "deleteDesc": "Bu karakteri kalıcı olarak sil",
      "character": "Karakter",
      "never": "Hiç",
      "justNow": "Az önce",
      "timeAgo": {
        "minutes": "{{n}}dk önce",
        "hours": "{{n}}h önce",
        "days": "{{n}}d önce"
      }
    },
    "tokens": {
      "input": "Giriş",
      "output": "Çıkış"
    },
    "activity": {
      "synthesis": "Sentez",
      "consolidation": "Birleştirme",
      "bm25Rebuild": "BM25 yeniden oluşturma",
      "dripResearch": "Damlama araştırması",
      "running": "Çalışıyor",
      "stopped": "Durduruldu"
    },
    "setup": {
      "complete": "Kurulum tamamlandı!",
      "completeMessage": "Lettuce motorun yapılandırıldı ve hazır.",
      "openDashboard": "Paneli aç"
    },
    "welcome": {
      "title": "Lettuce motoruna hoş geldin",
      "subtitle": "Yapay zeka karakter motorunu kuralım. Bu yaklaşık 2 dakika sürecek.",
      "feature1": "Motor, yapay zeka karakterlerine kalıcı hafıza, duygular, ilişkiler ve gerçek bir kimlik verir.",
      "feature2": "Önce bir LLM arka ucu, ardından motor ayarlarını yapılandıracağız.",
      "getStarted": "Başla"
    },
    "config": {
      "activeProviders": "Aktif sağlayıcılar",
      "noModelSet": "Model ayarlanmamış",
      "defaultBadge": "Varsayılan",
      "noProvidersWarning": "Yapılandırılmış sağlayıcı yok. Aşağıdan en az bir LLM arka ucu ekle.",
      "addProvider": "Sağlayıcı ekle",
      "quickImport": "Uygulama sağlayıcılarından hızlı içe aktarma",
      "importButton": "İçe aktar",
      "fields": {
        "model": "Model",
        "modelPlaceholder": "ör. claude-sonnet-4-5-20250929",
        "apiKey": "API Anahtarı",
        "apiKeyPlaceholder": "API anahtarını gir",
        "currentKey": "Mevcut anahtar:",
        "baseUrl": "Temel URL",
        "baseUrlPlaceholder": "http://localhost:11434",
        "maxTokens": "Maksimum token",
        "temperature": "Sıcaklık"
      },
      "enableProvider": "Sağlayıcıyı etkinleştir",
      "setAsDefault": "Varsayılan olarak ayarla",
      "defaultBackend": "Varsayılan arka uç",
      "remove": "Kaldır",
      "saveChanges": "Değişiklikleri kaydet",
      "saving": "Kaydediliyor...",
      "saved": "Kaydedildi"
    },
    "providers": {
      "title": "LLM Sağlayıcı",
      "subtitle": "Motor, çalışabilmek için en az bir LLM arka ucuna ihtiyaç duyar. Aşağıdan bir veya daha fazla sağlayıcı yapılandır.",
      "importFromProviders": "Sağlayıcılarından içe aktar",
      "imported": "İçe aktarıldı",
      "use": "Kullan",
      "saveContinue": "Kaydet ve devam et"
    },
    "settings": {
      "fields": {
        "dataDirectory": "Veri dizini",
        "logLevel": "Günlük seviyesi",
        "maxHistory": "Maksimum geçmiş (konuşma turları)"
      },
      "logLevels": {
        "debug": "HATA AYIKLAMA",
        "info": "BİLGİ",
        "warning": "UYARI",
        "error": "HATA"
      },
      "sections": {
        "engine": "Motor",
        "backgroundLoops": "Arka plan döngüleri",
        "memory": "Hafıza",
        "safety": "Güvenlik",
        "research": "Araştırma"
      },
      "backgroundLoops": {
        "synthesis": "Sentez (dk)",
        "consolidation": "Birleştirme (dk)",
        "bm25Rebuild": "BM25 yeniden oluşturma (dk)",
        "dripResearch": "Damlama araştırması (dk)"
      },
      "memory": {
        "embeddingModel": "Embedding modeli",
        "maxRetrieval": "Maksimum geri çağırma sonucu",
        "denseWeight": "Yoğun ağırlık",
        "bm25Weight": "BM25 ağırlığı",
        "graphWeight": "Graf ağırlığı",
        "recencyBoost": "Yakınlık artışı (saat)",
        "randomSurface": "Rastgele yüzey olasılığı"
      },
      "safety": {
        "honestySection": "Dürüstlük bölümü",
        "honestyDesc": "Sistem promptuna dürüstlük bölümü dahil et",
        "userDataDeletion": "Kullanıcı verisi silme",
        "userDataDesc": "Kullanıcıların veri silme talebinde bulunmasına izin ver"
      },
      "research": {
        "scrapeOnBoot": "Başlangıçta tarama",
        "scrapeDesc": "Motor başlatıldığında araştırma taraması çalıştır",
        "periodicInterval": "Periyodik aralık (saat)"
      },
      "saveChanges": "Değişiklikleri kaydet",
      "saving": "Kaydediliyor...",
      "saved": "Kaydedildi"
    },
    "settingsStep": {
      "title": "Motor ayarları",
      "subtitle": "Genel motor ayarlarını yapılandır. Hepsinin makul varsayılan değerleri var — atlamaktan çekinme.",
      "completingSetup": "Kurulum tamamlanıyor...",
      "completeSetup": "Kurulumu tamamla"
    },
    "chat": {
      "sendMessage": "Bir mesaj gönder...",
      "sendButton": "Mesaj gönder",
      "typeMessage": "Bir mesaj yaz",
      "back": "Geri",
      "assistantTyping": "Asistan yazıyor",
      "fallbackName": "Sohbet"
    },
    "tagInput": {
      "addMore": "Daha fazla ekle...",
      "typeAndPressEnter": "Yaz ve Enter'a bas"
    },
    "characterCreate": {
      "steps": {
        "identity": {
          "title": "Kimlik",
          "aiGenerated": "Yapay zeka ile oluşturulmuş",
          "nameLabel": "Ad *",
          "namePlaceholder": "Karakter adı",
          "eraLabel": "Dönem",
          "eraPlaceholder": "ör. modern, Viktorya dönemi",
          "roleLabel": "Rol",
          "rolePlaceholder": "ör. Dedektif, Bilim insanı",
          "settingLabel": "Ortam",
          "settingPlaceholder": "Karakterin nerede yaşadığını tanımla (birinci şahıs)...",
          "coreIdentityLabel": "Temel kimlik",
          "coreIdentityPlaceholder": "Bu karakter özünde kim? (birinci şahıs, 3-5 cümle)",
          "backstoryLabel": "Geçmiş hikayesi",
          "backstoryPlaceholder": "Hayat hikayesi ve önemli olaylar (birinci şahıs)..."
        },
        "mode": {
          "title": "Karakter oluştur",
          "subtitle": "Yapay zeka ile karakter oluştur veya sıfırdan oluştur.",
          "aiBoost": "Yapay zeka desteği",
          "aiBoostDesc": "Karakter fikrini tanımla ve yapay zeka tam bir tanım oluştursun.",
          "nameOptional": "Ad (isteğe bağlı)",
          "namePlaceholder": "ör. Marcus Cole",
          "seedDescription": "Tohum açıklaması *",
          "seedPlaceholder": "ör. 50'lerin Harlem'inde caz piyanisti, felsefi, gece sohbetlerini sever",
          "eraOptional": "Dönem (isteğe bağlı)",
          "eraPlaceholder": "ör. 50'ler, modern, Viktorya dönemi",
          "generating": "Oluşturuluyor...",
          "generateCharacter": "Karakter oluştur",
          "or": "veya",
          "startFromScratch": "Sıfırdan başla"
        },
        "personality": {
          "title": "Kişilik",
          "traits": "Kişilik özellikleri",
          "traitsPlaceholder": "ör. esprili, merhametli, inatçı",
          "speechPatterns": "Konuşma kalıpları",
          "formality": "Resmiyet",
          "formal": "Resmi",
          "casual": "Rahat",
          "texting": "Mesajlaşma",
          "verbosity": "Ayrıntılılık",
          "terse": "Kısa",
          "medium": "Orta",
          "verbose": "Detaylı",
          "textStyle": "Metin stili",
          "dialect": "Lehçe",
          "dialectPlaceholder": "ör. İstanbul Türkçesi, Karadeniz ağzı",
          "catchphrases": "Karakteristik ifadeler",
          "catchphrasesPlaceholder": "ör. Yahu, ne diyorsun...",
          "vocabPreferences": "Kelime tercihleri",
          "vocabPreferencesPlaceholder": "Tercih ettiği kelimeler",
          "vocabAvoidances": "Kaçınılan kelimeler",
          "vocabAvoidancesPlaceholder": "Kaçındığı kelimeler",
          "fillerWords": "Dolgu kelimeleri",
          "fillerWordsPlaceholder": "ör. şey, yani, hani",
          "exampleQuotes": "Örnek alıntılar",
          "exampleQuotesPlaceholder": "3-5 satır örnek diyalog"
        },
        "world": {
          "title": "Dünya ve davranış",
          "knowledgeDomains": "Bilgi alanları",
          "knowledgeDomainsPlaceholder": "ör. caz tarihi, müzik teorisi",
          "knowledgeBoundaries": "Bilgi sınırları",
          "knowledgeBoundariesPlaceholder": "Bilmediği konular",
          "researchSeeds": "Araştırma tohumları",
          "researchSeedsPlaceholder": "Arka plan araştırması için başlangıç konuları",
          "researchEnabled": "Araştırma etkin",
          "researchEnabledDesc": "Arka planda bilgi toplama izni ver",
          "physicalDescription": "Fiziksel tanım",
          "physicalDescPlaceholder": "Fiziksel görünüm ve jestler...",
          "physicalHabits": "Fiziksel alışkanlıklar",
          "physicalHabitsPlaceholder": "ör. parmaklarıyla davul çalar, gözlüğünü düzeltir",
          "idleBehaviors": "Boşta davranışları",
          "idleBehaviorsPlaceholder": "Meşgul olmadığında ne yapar",
          "timeBehaviors": "Zamana göre davranışlar",
          "timePlaceholder": "{{period}} boyunca ne yapar?",
          "earlyMorning": "Sabah erken",
          "morning": "Sabah",
          "afternoon": "Öğleden sonra",
          "evening": "Akşam",
          "night": "Gece",
          "baselineEmotions": "Temel duygular (Plutchik)",
          "emotionDesc": "Duygusal temel çizgiyi ayarla (0 = yok, 1 = maksimum)",
          "joy": "Neşe",
          "trust": "Güven",
          "fear": "Korku",
          "surprise": "Şaşkınlık",
          "sadness": "Üzüntü",
          "disgust": "Tiksinme",
          "anger": "Öfke",
          "anticipation": "Beklenti",
          "engineOverrides": "Motor geçersiz kılmaları",
          "backend": "Arka uç",
          "model": "Model_",
          "temperature": "Sıcaklık",
          "leaveEmpty": "Varsayılan için boş bırak"
        },
        "review": {
          "title": "İnceleme",
          "subtitle": "Oluşturmadan önce karakterini incele.",
          "edit": "Düzenle",
          "notSet": "Ayarlanmamış",
          "identitySection": "Kimlik",
          "personalitySection": "Kişilik",
          "worldSection": "Dünya ve davranış",
          "nameLabel": "Ad",
          "eraLabel": "Dönem",
          "roleLabel": "Rol",
          "settingLabel": "Ortam",
          "coreIdentityLabel": "Temel kimlik",
          "backstoryLabel": "Geçmiş hikayesi",
          "traitsLabel": "Özellikler",
          "formalityLabel": "Resmiyet",
          "verbosityLabel": "Ayrıntılılık",
          "dialectLabel": "Lehçe",
          "catchphrasesLabel": "Karakteristik ifadeler",
          "domainsLabel": "Alanlar",
          "boundariesLabel": "Sınırlar",
          "researchSeedsLabel": "Araştırma tohumları",
          "researchLabel": "Araştırma",
          "enabled": "Etkin",
          "disabled": "Devre dışı",
          "physicalLabel": "Fiziksel",
          "habitsLabel": "Alışkanlıklar",
          "idleLabel": "Boşta",
          "timeBehaviorsLabel": "Zamana göre davranışlar",
          "emotionsLabel": "Duygular",
          "configured": "Yapılandırılmış",
          "backendLabel": "Arka uç",
          "modelLabel": "Model",
          "temperatureLabel": "Sıcaklık",
          "creating": "Oluşturuluyor...",
          "createCharacter": "Karakter oluştur"
        }
      }
    }
  },
  "library": {
    "filterTitle": "Kütüphaneyi filtrele",
    "filters": {
      "all": "Tümü",
      "characters": "Karakterler",
      "personas": "Personalar",
      "lorebooks": "Lore kitapları",
      "images": "Görüntüler"
    },
    "emptyStates": {
      "all": {
        "title": "Kütüphanen boş",
        "description": "Burada görmek için karakterler, personalar ve lore kitapları oluştur"
      },
      "characters": {
        "title": "Henüz karakter yok",
        "description": "Sohbet etmeye başlamak için ilk karakterini oluştur"
      },
      "personas": {
        "title": "Henüz persona yok",
        "description": "Sohbetteki kimliğini kişiselleştirmek için bir persona oluştur"
      },
      "lorebooks": {
        "title": "Henüz lore kitabı yok",
        "description": "Lore kitapları bir karakterin ayarlarından oluşturulur"
      }
    },
    "actions": {
      "startChat": "Sohbet başlat",
      "editCharacter": "Karakteri düzenle",
      "editPersona": "Personayı düzenle",
      "editLorebook": "Lore kitabını düzenle",
      "renameLorebook": "Lore kitabını yeniden adlandır",
      "exportCharacter": "Karakteri dışa aktar",
      "exportPersona": "Personayı dışa aktar",
      "chatAppearance": "Sohbet görünümü",
      "deleteCharacter": "Karakteri sil",
      "deletePersona": "Personayı sil",
      "deleteLorebook": "Lore kitabını sil",
      "importLorebook": "Lore kitabı içe aktar"
    },
    "imageLibrary": {
      "filters": {
        "all": "Tümü",
        "backgrounds": "Arka planlar",
        "avatars": "Avatarlar",
        "attachments": "Ekler",
        "other": "Diğer"
      },
      "searchPlaceholder": "Dosya adı, yol, oturum kimliği veya varlık kimliği ile ara",
      "empty": {
        "title": "Bu görünüm için görsel yok",
        "description": "Başka bir filtre veya arama terimi dene. Kütüphane yalnızca uygulamanın yerel depolamasına kaydedilmiş görselleri gösterir."
      },
      "actions": {
        "sort": "Sırala",
        "useThis": "Bunu kullan",
        "using": "Kullanılıyor...",
        "copyPath": "Yolu kopyala",
        "saving": "Kaydediliyor...",
        "download": "İndir",
        "delete": "Görseli sil",
        "deleting": "Siliniyor..."
      },
      "active": "Aktif",
      "messages": {
        "loadFailed": "Görsel kütüphanesi yüklenemedi",
        "saved": "Görsel kaydedildi",
        "downloadFailed": "İndirme başarısız",
        "useFailed": "Bu görsel kullanılamadı",
        "deleted": "Görsel silindi",
        "deleteFailed": "Görsel silinemedi"
      },
      "deleteConfirm": {
        "title": "Görsel silinsin mi?",
        "message": "\"{{filename}}\" silmek istediğinden emin misin? Bu, hala kullanan avatarları, sohbet arka planlarını veya mesaj eklerini bozabilir."
      },
      "sort": {
        "newest": "En Yeni",
        "largest": "En Büyük_",
        "name": "Ad_"
      },
      "kinds": {
        "background": "Arka Plan",
        "avatar": "Avatar",
        "attachment": "Ek",
        "stored": "Stored"
      },
      "detailsTitle": "{{kind}} ayrıntılar",
      "formatsLabel": "Biçimler",
      "storagePath": "Depolama yolu",
      "contextLabel": "Bağlam",
      "contextLinkedFallback": "Bağlantılı",
      "show": "Show",
      "hide": "Hide_",
      "contextRoles": {
        "character": "karakter:",
        "session": "oturum:",
        "role": "rol:"
      },
      "downloadFormat": "{{download}} Biçim",
      "unknownDate": "Bilinmiyor",
      "clearSearch": "Aramayı temizle_",
      "copyFilename": "Dosya adını kopyala",
      "copyLabels": {
        "filename": "Dosya adı",
        "storagePath": "Depolama yolu"
      },
      "copy": {
        "copied": "{{label}} kopyalandı",
        "failed": "Kopyalanamadı {{label}}"
      }
    },
    "deleteConfirm": {
      "title": "{{itemType}} silinsin mi?",
      "message": "Silmek istediğinden emin misin:",
      "characterWarning": "Bu, bu karakterle olan tüm sohbet oturumlarını da silecek."
    },
    "rename": {
      "title": "Lore kitabını yeniden adlandır",
      "placeholder": "Yeni adı gir..."
    },
    "itemTypes": {
      "character": "Karakter",
      "persona": "Kişilik",
      "lorebook": "Lore kitabı"
    },
    "lorebookLabel": "Lore kitabı",
    "noDescriptionYet": "Henüz açıklama yok",
    "errors": {
      "importLorebook": "Lorebook içe aktarılamadı. ",
      "exportFailed": "Dışa aktarma başarısız oldu"
    },
    "card": {
      "avatarAlt": "{{name}} avatar"
    },
    "lorebookEditor": {
      "titleOverride": "Lorebook - {{name}}",
      "dragToReorder": "Yeniden sıralamak için sürükleyin",
      "aria": {
        "generateEntry": "Bilgi kitabı girişi oluştur_",
        "editLorebook": "Bilgi kitabını düzenle",
        "exportLorebook": "Bilgi kitabını dışa aktar"
      }
    }
  },
  "onboarding": {
    "loading": "Sağlayıcılar yükleniyor...",
    "stepIndicator": "Adım {{current}} / {{total}}",
    "steps": {
      "provider": "Sağlayıcı kurulumu",
      "model": "Model kurulumu",
      "memory": "Hafıza sistemi",
      "stepNofM": "{{current}} / {{total}} Adımı"
    },
    "provider": {
      "availableProviders": "Kullanılabilir sağlayıcılar",
      "chooseProvider": "Bir sağlayıcı seç",
      "titleMobile": "Yapay zeka sağlayıcınızı seçin",
      "descMobile": "Başlamak için bir yapay zeka sağlayıcısı seçin. ",
      "configureProvider": "Yapılandır {{name}}",
      "connectProvider": "Bağlan {{name}}",
      "connectProviderDesc": "Sohbetleri etkinleştirmek için API anahtarınızı aşağıya yapıştırın. ",
      "localLLMs": "Yerel LLM'ler_",
      "useLocalLLMs": "Yerel Yüksek Lisans'ları kullanmak istiyorum",
      "browseModelLibrary": "Model Kitaplığına Göz Atın",
      "browseModelLibraryDesc": "HuggingFace'ten GGUF modellerini arayın ve indirin",
      "useOwnGguf": "Kendi GGUF dosyalarımı kullanın",
      "useOwnGgufDesc": "Cihazınızdan bir GGUF modeli ve isteğe bağlı mmproj dosyası seçin",
      "fields": {
        "displayLabel": "Görüntü Etiketi",
        "displayLabelHint": "Bu sağlayıcı menülerinizde nasıl görünecek?",
        "displayLabelPlaceholder": "{{name}}",
        "defaultLabelFallback": "Provider",
        "apiKey": "API Anahtarım",
        "apiKeyOptional": "API Anahtarı (İsteğe bağlı)",
        "apiKeyHint": "Anahtarlar yerel olarak şifrelenir",
        "apiKeyPlaceholderRemote": "sk-...",
        "apiKeyPlaceholderLocal": "Genellikle gerekli değildir",
        "whereToFind": "_Nerede bulunur_",
        "baseUrl": "Temel URL",
        "baseUrlPlaceholderHost": "http://192.168.1.10:3333",
        "baseUrlPlaceholderLocal": "http://localhost:11434",
        "baseUrlPlaceholderRemote": "https://api.provider.com",
        "baseUrlPlaceholderIntenseRP": "http://127.0.0.1:7777/v1",
        "baseUrlHintLocal": "Bağlantı noktasıyla birlikte yerel sunucu adresiniz",
        "baseUrlHintHost": "Ana makineniz tarafından gösterilen masaüstü ana bilgisayar URL'sini girin",
        "baseUrlHintRemote": "Gerekirse varsayılan uç noktayı geçersiz kılın",
        "chatEndpoint": "Sohbet Bitiş Noktası",
        "systemRole": "Sistem Rolü",
        "userRole": "Kullanıcı Rolü",
        "assistantRole": "Asistan Rolü",
        "supportsStreaming": "Akışı Destekler",
        "mergeSameRole": "Aynı Rol İletilerini Birleştir",
        "toolChoiceMode": "Araç Seçimi Modu",
        "toolChoiceHint": "tool_choice'un özel uç noktaya nasıl gönderildiğini denetler."
      },
      "toolChoice": {
        "auto": "_Auto_",
        "required": "Gerekli_",
        "none": "_Yok",
        "omit": "Alanı Atla",
        "passthrough": "Geçit (Araç Yapılandırması)"
      },
      "buttons": {
        "testConnection": "Bağlantıyı Test Et",
        "testing": "Test Ediliyor..."
      },
      "descriptions": {
        "chutes": "Etkileyici RP için en iyi açık kaynaklı modeller için OpenAI uyumlu çıkarım",
        "openai": "GPT-5, GPT-4.1 ve GPT-4o modelleri",
        "lettuceHost": "OpenAI tarzı API ile LAN üzerinden kendi masaüstü Marul Sunucunuza bağlanın",
        "anthropic": "Claude 4.5 Sonnet ve Haiku derin, duygusal diyalog için",
        "aggregator": "GPT-5, Claude 4.5, Grok-3, Mixtral ve daha fazlası gibi modellere erişin",
        "openaiCompatible": "Herhangi bir OpenAI tarzı API uç noktasını kullanın",
        "mistral": "Mistral Small 3.2, Mixtral 8x22B ve diğer Mistral modellerini kullanın",
        "deepseek": "DeepSeek-V3.2-Exp, DeepSeek-R1 ve diğer yüksek verimli modeller",
        "xai": "Grok-1.5, Grok-3 ve daha yeni xAI modelleri",
        "zai": "GLM-4.5, GLM-4.6 ve Air çeşitleri",
        "moonshot": "Kimi-K2 Thinking ve Kimi-K1 modelleri",
        "gemini": "Gemini 2.5 Flash, 2.5 Pro ve daha fazlası",
        "qwen": "Qwen3-VL ve daha yeni Qwen modelleri",
        "nvidia": "Nemotron, Llama, DeepSeek ve daha fazlası NVIDIA NIM",
        "custom": "Point LettuceAI aracılığıyla herhangi bir özel model uç noktasına",
        "fallback": "Yapay zeka modeli sağlayıcı"
      },
      "descriptionsShort": {
        "chutes": "Açık kaynak model çıkarımı",
        "openai": "GPT-5, GPT-4o, GPT-4.1",
        "lettuceHost": "Kendi LAN sunucunuz",
        "anthropic": "Claude 4.5 Sonnet ve Haiku",
        "aggregator": "Çok modelli toplayıcı",
        "openaiCompatible": "Özel OpenAI uç noktası",
        "mistral": "Mistral ve Mixtral modelleri",
        "deepseek": "DeepSeek-V3, R1",
        "xai": "Grok-1.5, Grok-3",
        "zai": "GLM-4.5, GLM-4.6",
        "moonshot": "Kimi-K2 Düşünme",
        "gemini": "Gemini 2.5 Flash ve Pro",
        "qwen": "Qwen3-VL modelleri",
        "nvidia": "NVIDIA NIM çıkarımı",
        "custom": "Özel uç nokta",
        "fallback": "AI sağlayıcı"
      },
      "cardLabel": "{{step}}: {{name}}",
      "errors": {
        "hostUrlRequired": "Ana makine URL'si gerekli (ör. http://192.168.1.10:3333)",
        "baseUrlRequired": "Temel URL gerekli (ör. http://localhost:11434)",
        "apiKeyTooShort": "API anahtarı çok kısa görünüyor",
        "invalidApiKey": "Geçersiz API anahtarı",
        "connectionFailed": "Bağlantı başarısız oldu",
        "verificationFailed": "Doğrulama başarısız oldu",
        "failedToSave": "Sağlayıcı kaydedilemedi",
        "connectionSuccessful": "Bağlantı başarılı!",
        "modelNotFound": "Sağlayıcıda model bulunamadı",
        "modelVerificationFailed": "_Model doğrulama başarısız_",
        "failedToSaveModel": "_Model kaydedilemedi_"
      }
    },
    "model": {
      "noProvidersTitle": "Yapılandırılmış sağlayıcı yok",
      "noProvidersDesc": "Varsayılan model seçmeden önce bir sağlayıcı bağlaman gerekecek.",
      "goToProviderSetup": "Sağlayıcı kurulumuna git",
      "yourProviders": "_Sağlayıcılarınız",
      "yourProvidersHint": "Kullanılacak sağlayıcıyı seçin",
      "setDefaultModel": "Varsayılan modelinizi ayarlayın",
      "setDefaultModelDesc": "LettuceAI'nin varsayılan olarak hangi sağlayıcıyı ve model adını kullanması gerektiğini seçin. ",
      "setDefaultModelDescDesktop": "Modelinizi yapılandırmak için listeden bir sağlayıcı seçin.",
      "modelDetails": "Model Ayrıntıları",
      "modelDetailsDesc": "Uygulamanın içinde göreceğiniz API tanımlayıcısını ve etiketi tanımlayın.",
      "whichModel": "Hangi modeli kullanmalıyım?",
      "nextMemorySystem": "Sonraki: Bellek Sistemi",
      "fields": {
        "displayName": "Görünen Ad",
        "displayNamePlaceholder": "_Yaratıcı akıl hocası_",
        "displayNameHint": "_Bu model menülerde nasıl görünür_",
        "modelId": "_Model Kimliği",
        "modelPathGguf": "Model Yolu (GGUF)",
        "modelIdPlaceholder": "örneğin ",
        "modelPathPlaceholder": "/path/to/model.gguf",
        "modelIdHint": "API çağrıları için kullanılan tam tanımlayıcı",
        "showList": "Listeyi Göster",
        "manualInput": "Manuel Giriş",
        "refreshModelList": "Model listesini yenileyin",
        "selectModel": "Model Seç",
        "selectAModel": "Bir model seçin...",
        "searchModels": "Modelleri arayın..._",
        "noModelsFound": "\"{{query}}\" ile eşleşen model bulunamadı"
      },
      "fillBothFields": "Bitir düğmesini etkinleştirmek için yukarıdaki her iki alanı da doldurun.",
      "providerNames": {
        "chutes": "Chutes",
        "intenseRpNext": "IntenseRP Next",
        "openai": "OpenAI",
        "anthropic": "Anthropic",
        "openrouter": "OpenRouter",
        "openaiCompatible": "OpenAI uyumlu",
        "custom": "Özel uç nokta"
      }
    },
    "memory": {
      "dynamicTitle": "Dinamik hafıza",
      "recommended": "Önerilen",
      "settingUp": "Kuruluyor...",
      "finishSetup": "Kurulumu bitir",
      "promptTitle": "Dinamik hafızayı yapılandır",
      "oneLastStep": "Son bir adım",
      "downloadAndEnable": "İndir ve etkinleştir",
      "chooseStyle": "Bellek stilinizi seçin",
      "howRemember": "Yapay zeka arkadaşlarınız sizinle ve konuşmalarınızla ilgili ayrıntıları nasıl hatırlamalıdır?",
      "dynamicDescription": "Bağlamı akıllıca yönetmek için <0>yerel yerleştirme modeli</0> kullanır. ",
      "dynamicFeatures": {
        "quality": "Uzun sohbetlerde kaliteyi korur",
        "cost": "API maliyetlerini önemli ölçüde azaltır",
        "auto": "Otomatik içerik yönetimi",
        "zeroConfig": "Sıfır yapılandırma gerekli"
      },
      "manualTitle": "Manuel Bellek",
      "manualBadge": "Klasik deneyim_",
      "manualDescription": "Mesajları açıkça sabitler ve \"Dünya Bilgisi\"ni veya karakteri düzenlersiniz ",
      "manualFeatures": {
        "control": "Gerçekler üzerinde tam kontrol",
        "scenarios": "Belirli senaryolar için en iyisi"
      },
      "setupModelMessage": "Dinamik Belleği kullanmak için cihazınıza küçük bir yerleştirme modeli (~120MB) indirmemiz gerekiyor.",
      "setupBullets": {
        "offline": "Model, cihazınızda %100 çevrimdışı çalışır",
        "remembering": "Bağlamı hatırlamak için gereklidir",
        "disable": "Bunu daha sonra ayarlardan devre dışı bırakabilirsiniz"
      },
      "stepLabel": "Adım 3 / 3",
      "stepLabelMemory": "Bellek Sistemi"
    },
    "welcome": {
      "appName": "LettuceAI",
      "tagline": "Kişisel yapay zeka arkadaşın. Gizli, güvenli ve her zaman cihazında.",
      "features": {
        "onDevice": "Yalnızca cihazda",
        "characterReady": "Karakterler hazır"
      },
      "betaWarning": {
        "title": "Masaüstü beta sürümü",
        "description": "Masaüstü sürümünü kullanıyorsun. Bazı özellikler mobil sürümden farklı olabilir. Sorunları GitHub'da bildir."
      },
      "languageSelector": {
        "title": "Dil",
        "description": "Cihazından otomatik olarak algılandı. İstediğin zaman ayarlardan değiştirebilirsin."
      },
      "getStarted": "Başla",
      "skipForNow": "Şimdilik atla",
      "restoreFromBackup": "Yedekten geri yükle",
      "setupTime": "Kurulum 2 dakikadan kısa sürer",
      "skipWarning": {
        "title": "Kurulum atlansın mı?",
        "warningTitle": "Sohbet etmek için sağlayıcı gerekli",
        "warningMessage": "Sağlayıcı olmadan mesaj gönderemezsin. Daha sonra ayarlardan ekleyebilirsin.",
        "addProvider": "Sağlayıcı ekle",
        "skipAnyway": "Yine de atla"
      },
      "restoreBackup": {
        "title": "Yedeği geri yükle",
        "selectMessage": "Geri yüklemek için bir yedekleme seç.",
        "browse": "Dosyalara gözat",
        "processing": "Dosya işleniyor...",
        "processingNote": "Büyük yedeklemeler bir dakika sürebilir",
        "noBackups": "Yedekleme bulunamadı",
        "noBackupsHint": ".lettuce dosyası seçmek için gözata dokun",
        "browseLettuce": ".lettuce dosyası ara",
        "passwordLabel": "Yedekleme parolası",
        "passwordPlaceholder": "Parolayı gir",
        "restoreButton": "Yedeği geri yükle",
        "restoring": "Geri yükleniyor...",
        "infoMessage": "Bu, uygulamayı karakterler, sohbetler ve ayarlar dahil yedeklenmiş verilerinle kuracak.",
        "embeddingTitle": "Embedding modeli gerekli",
        "dynamicMemoryDetected": "Dinamik hafıza algılandı",
        "dynamicMemoryMessage": "Bu yedekleme, dinamik hafızası etkinleştirilmiş karakterler içeriyor, bu da embedding modeli (~120MB) gerektiriyor.",
        "embeddingOptions": "Dinamik hafızayı etkinleştirmek için modeli şimdi indirebilir veya onsuz devam edebilirsin (etkilenen karakterler için dinamik hafıza devre dışı bırakılacak).",
        "downloadModel": "Modeli indir",
        "continueWithoutDynamic": "Dinamik hafıza olmadan devam et",
        "embeddingNote": "Modeli indirdikten sonra karakter ayarlarından dinamik hafızayı yeniden etkinleştirebilirsin.",
        "back": "Geri",
        "cancel": "İptal_",
        "errors": {
          "passwordRequired": "Şifre gerekli",
          "incorrectPassword": "Yanlış şifre",
          "failedToOpenFile": "Dosya açılamadı",
          "failedToRestore": "Yedekleme geri yüklenemedi",
          "failedToUpdateSettings": "Ayarlar güncellenemedi"
        }
      }
    },
    "common": {
      "back": "Geri_",
      "cancel": "_İptal",
      "continue": "_Devam",
      "verifying": "Doğrulanıyor..._",
      "skipForNow": "Şimdilik atla",
      "selectAProvider": "Yapılacak sağlayıcıyı seçin",
      "clickToSelectProvider": "Bir sağlayıcı seçmek için tıklayın",
      "selectProviderFromList": "Başlamak için listeden bir sağlayıcı seçin.",
      "enterApiKey": "Yapay zeka sohbet işlevini etkinleştirmek için API anahtarınızı girin."
    },
    "modelGuide": {
      "badge": "Model Rehberi",
      "title": "Bir modeli nasıl seçerim?",
      "intro": "LettuceAI tek bir \"en iyi\" modeli zorlamaz. ",
      "askYourself": "Kendinize şunu sorun:",
      "factors": {
        "quality": {
          "title": "Kalite ve yetenekler",
          "description": "Modelin ne kadar akıllı olması gerekiyor? ",
          "q1": "Derin karakter tutarlılığına ve duygusal zekaya mı ihtiyacınız var?",
          "q2": "Sürükleyici hikaye anlatımına ve inandırıcı karakter kişiliklerine önem veriyor musunuz?",
          "q3": "Modelin karakter ayrıntılarını hatırlamasını ve uzun oturumlar boyunca karakterini korumasını mı istiyorsunuz?"
        },
        "speed": {
          "title": "Hız ve gecikme",
          "description": "Daha hızlı modeller, konuşkan ve ileri geri konuşmalar için daha iyi hissettirir. ",
          "q1": "Rol oyununun doğal bir şekilde akmasını sağlamak için neredeyse anında yanıtlar mı istiyorsunuz?",
          "q2": "Beklemenin sürükleyiciliği bozacağı hızlı diyalog sahneleri mi yapıyorsunuz?",
          "q3": "Bu, hızlı ileri geri hareketlerin mükemmel yanıtlardan daha önemli olduğu sıradan RP için mi?"
        },
        "budget": {
          "title": "Bütçe ve kullanım",
          "description": "Her sağlayıcı, jeton başına fatura keser. ",
          "q1": "Daha zengin karakter etkileşimleri için daha fazla ödemeye razı mısınız, yoksa günlük RP için ucuz bir şey mi istiyorsunuz?",
          "q2": "Sağlayıcınızdan/yönlendiricinizden önce deneyebileceğiniz ücretsiz modeller var mı?",
          "q3": "Ayrıntılı sahne açıklamaları içeren uzun rol yapma oturumları düzenleyecek misiniz?",
          "q4": "Aşmak istemediğiniz zorlu bir aylık bütçeniz mi var?"
        },
        "safety": {
          "title": "Güvenlik, gizlilik ve ekstralar",
          "description": "Sağlayıcıların güvenliği, günlük kaydını ve resimler, araçlar veya uzun bağlam pencereleri gibi ekstra özellikleri nasıl ele aldıkları konusunda farklılık gösterir.",
          "q1": "Yetişkinlere yönelik veya yaratıcı rol yapma senaryoları için daha az içerik filtresine mi ihtiyacınız var?",
          "q2": "Özel RP konuşmalarınızın günlüğe kaydedilmesini veya eğitim için kullanılmasını önemsiyor musunuz?",
          "q3": "Karmaşık hikayeler ve karakter geçmişleri için uzun bağlam pencerelerine mi ihtiyacınız var?"
        }
      },
      "where": {
        "title": "Modelleri nerede bulabilirim?",
        "intro": "Çoğu sağlayıcı ve yönlendiricinin bir <0>model listesi veya kataloğu</0> vardır. ",
        "directTitle": "Doğrudan sağlayıcılar",
        "directDesc": "OpenAI, Anthropic, Google Gemini, xAI, Mistral, vb. Her birinin resmi model adlarını, yeteneklerini ve fiyatlarını görebileceğiniz bir konsolu/kontrol paneli vardır.",
        "routersTitle": "Yönlendiriciler ve hub'lar",
        "routersDesc": "OpenRouter veya diğer toplayıcılar gibi hizmetler, farklı sağlayıcılardan birçok modeli tek bir yerde listeler ve genellikle karşılaştırmalar ve fiyat karşılaştırmalarıyla birlikte.",
        "communityTitle": "Topluluk önerileri",
        "communityDesc": "Sağlayıcınızdan/yönlendiricinizden gelen belgelere, bloglara veya topluluk gönderilerine bakın. "
      },
      "rules": {
        "title": "Basit kurallar",
        "casual": "Gündelik sohbet için: sağlayıcınızdan/yönlendiricinizden hızlı, ucuz bir sohbet modeli seçin.",
        "experiments": "Denemeler veya yüksek hacim için: yeterince iyi hissettiren en ucuz modelle başlayın, ardından gerekirse yükseltin.",
        "switch": "Bir şeyler ters gidiyorsa (çok yavaş / çok saçma / çok pahalı): LettuceAI'de istediğiniz zaman modelleri değiştirebilirsiniz."
      },
      "disclaimer": "En son model listesi, limitler ve fiyatlandırma için her zaman sağlayıcının kendi belgelerini kontrol edin. "
    },
    "whereToFind": {
      "badge": "API Anahtarı Yardımı",
      "intro": "API anahtarınızı almak için bu adımları izleyin, ardından LettuceAI'ye dönün ve bunu sağlayıcı ayarlarına yapıştırın.",
      "readyPrompt": "Anahtarı almaya hazır mısınız?",
      "openProviderSite": "Sağlayıcı sitesini açın",
      "keyWarning": "API anahtarınızı asla herkese açık olarak paylaşmayın. ",
      "stuckPrompt": "Hala çözemediniz mi?",
      "joinDiscord": "Yardım için Discord sunucumuza katılın",
      "guides": {
        "chutes": {
          "title": "Chutes API anahtarınızı nasıl bulabilirsiniz",
          "s1": "Chutes.ai/app adresine gidin ve oturum açın.",
          "s2": "Hesap/ayarlar alanınızı açın ve API Anahtarlarını bulun.",
          "s3": "Yeni bir anahtar oluşturun (veya mevcut bir anahtarı kopyalayın).",
          "s4": "Anahtarı LettuceAI'ye yapıştırın."
        },
        "openai": {
          "title": "OpenAI API anahtarınızı nasıl bulabilirsiniz_",
          "s1": "Platform.openai.com adresine gidin ve oturum açın.",
          "s2": "Sağ üstteki profil avatarınızı tıklayın, ardından API anahtarlarını seçin.",
          "s3": "Yeni gizli anahtar oluştur'u tıklayın ve gösterilen değeri kopyalayın.",
          "s4": "Anahtarı LettuceAI'ye yapıştırın ve güvenli bir yerde saklayın. "
        },
        "anthropic": {
          "title": "Antropik API anahtarınızı nasıl bulabilirsiniz?",
          "s1": "Console.anthropic.com adresine gidin ve oturum açın.",
          "s2": "Sol kenar çubuğundan Ayarlar'ı açın.",
          "s3": "API anahtarlarını seçin ve Anahtar oluştur'a tıklayın.",
          "s4": "Anahtarı kopyalayıp LettuceAI'ye yapıştırın."
        },
        "openrouter": {
          "title": "_Nasıl bulunur? ",
          "s1": "openrouter.ai adresini ziyaret edin ve giriş yapın.",
          "s2": "Profil menünüzden Anahtarlar sayfasını açın.",
          "s3": "Anahtar oluştur'a tıklayın, ona bir ad verin ve kaydedin.",
          "s4": "Anahtarı kopyalayıp LettuceAI'ye yapıştırın."
        },
        "mistral": {
          "title": "Nasıl ",
          "s1": "Console.mistral.ai adresine gidin ve oturum açın.",
          "s2": "Kenar çubuğundaki API anahtarlarına tıklayın.",
          "s3": "API anahtarı oluştur seçeneğine tıklayın.",
          "s4": "Anahtarı kopyalayıp LettuceAI'ye yapıştırın."
        },
        "deepseek": {
          "title": "_DeepSeek'inizi nasıl bulabilirsiniz? ",
          "s1": "platform.deepseek.com'u açın ve oturum açın.",
          "s2": "Üst gezinme bölümünde API Anahtarları'na tıklayın.",
          "s3": "Henüz anahtarınız yoksa yeni bir anahtar oluşturun.",
          "s4": "Anahtarı kopyalayın ve LettuceAI'ye yapıştırın."
        },
        "groq": {
          "title": "Groq API anahtarınızı nasıl bulabilirsiniz?",
          "s1": "Console.groq.com adresini ziyaret edin ve oturum açın.",
          "s2": "Kenar çubuğundan API Anahtarlarını açın.",
          "s3": "Yeni bir anahtar oluşturun ve kopyalayın.",
          "s4": "Anahtarı LettuceAI'ye yapıştırın."
        },
        "gemini": {
          "title": "_Google'ınızı nasıl bulabilirsiniz? ",
          "s1": "aistudio.google.com adresinden Google AI Studio'ya gidin ve oturum açın.",
          "s2": "API anahtarını al veya Anahtarları yönet'i tıklayın.",
          "s3": "Gerekirse yeni bir anahtar oluşturun.",
          "s4": "Anahtarı kopyalayın ve LettuceAI'ye yapıştırın."
        },
        "xai": {
          "title": "XAI API anahtarınızı nasıl bulabilirsiniz?",
          "s1": "Console.x.ai'yi açın ve oturum açın.",
          "s2": "Konsoldaki API Anahtarları bölümüne gidin.",
          "s3": "Yeni bir anahtar oluşturun.",
          "s4": "Anahtarı kopyalayıp LettuceAI'ye yapıştırın."
        },
        "zai": {
          "title": "_Nasıl bulabilirsiniz? ",
          "s1": "open.bigmodel.cn adresine gidin ve oturum açın.",
          "s2": "Kullanıcı Merkezini açın, ardından API Anahtarları'na gidin.",
          "s3": "Anahtarınız yoksa yeni bir anahtar oluşturun.",
          "s4": "Anahtarı kopyalayın ve LettuceAI'ye yapıştırın."
        },
        "moonshot": {
          "title": "Moonshot (Kimi) API anahtarınızı nasıl bulabilirsiniz?",
          "s1": "platform.moonshot.cn adresini ziyaret edin ve oturum açın.",
          "s2": "Konsoldaki API Anahtarları bölümünü açın.",
          "s3": "Yeni bir anahtar oluşturun ve kopyalayın.",
          "s4": "Anahtarı LettuceAI'ye yapıştırın."
        },
        "qwen": {
          "title": "_Şifrenizi nasıl bulabilirsiniz? ",
          "s1": "dashscope.aliyun.com adresini açın ve giriş yapın.",
          "s2": "Kenar çubuğundaki API Anahtarları bölümüne gidin.",
          "s3": "Yeni bir anahtar oluşturun.",
          "s4": "Anahtarı kopyalayıp LettuceAI'ye yapıştırın."
        },
        "nanogpt": {
          "title": "_Nasıl bulunur? ",
          "s1": "Nano-gpt.com'a gidin ve giriş yapın.",
          "s2": "Kontrol panelini açın ve API anahtarları bölümüne gidin.",
          "s3": "Gerekiyorsa yeni bir anahtar oluşturun.",
          "s4": "Anahtarı kopyalayıp LettuceAI'ye yapıştırın."
        },
        "featherless": {
          "title": "_Nasıl bulunur? ",
          "s1": "tüysüz.ai'yi ziyaret edin ve oturum açın.",
          "s2": "Kontrol panelinden hesabınızı veya API bölümünü açın.",
          "s3": "Görmüyorsanız yeni bir anahtar oluşturun.",
          "s4": "Anahtarı kopyalayıp LettuceAI'ye yapıştırın."
        },
        "anannas": {
          "title": "_Nasıl yapılır? ",
          "s1": "Dashboard.anannas.ai adresine gidin ve giriş yapın.",
          "s2": "API Anahtarları bölümüne gidin.",
          "s3": "Yeni bir anahtar oluşturun ve kopyalayın.",
          "s4": "Anahtarı LettuceAI'ye yapıştırın."
        },
        "default": {
          "title": "API anahtarınızı nasıl bulabilirsiniz",
          "s1": "AI sağlayıcı kontrol panelinizi bir tarayıcıda açın ve oturum açın.",
          "s2": "API, Geliştirici veya Entegrasyon ayarlarını arayın.",
          "s3": "Yeni bir API anahtarı oluşturun veya mevcut olanı görüntüleyin.",
          "s4": "Anahtarı kopyalayın ve LettuceAI'ye yapıştırın."
        }
      }
    },
    "welcomeFooter": {
      "setupTimeMobile": "Kurulum 2 dakikadan az sürer"
    }
  },
  "search": {
    "placeholder": "Ara...",
    "tabs": {
      "characters": "Karakterler",
      "personas": "Personalar"
    },
    "noResults": "{{type}} bulunamadı",
    "emptyState": "Henüz {{type}} yok",
    "noResultsHint": "Başka bir arama terimi dene",
    "emptyCharacters": "Sohbet etmeye başlamak için ilk karakterini oluştur",
    "emptyPersonas": "Ayarlardan bir persona oluştur",
    "a11y": {
      "goBack": "Geri dön",
      "clearSearch": "Aramayı temizle",
      "characterAvatar": "{{name}} avatar"
    },
    "session": {
      "newChatTitle": "Yeni Sohbet_"
    },
    "noDescription": "Açıklama yok",
    "defaultBadge": "_Varsayılan_"
  },
  "sync": {
    "modes": {
      "join": "Katıl",
      "joinDesc": "Ana bilgisayara bağlan",
      "host": "Ana bilgisayar",
      "hostDesc": "Verilerini paylaş"
    },
    "sections": {
      "mode": "Mod",
      "connectToHost": "Ana bilgisayara bağlan",
      "startHosting": "Barındırmaya başla",
      "status": "Durum",
      "hosting": "Barındırma servisi",
      "localAddress": "Yerel ağ adresi",
      "connectionPin": "Bağlantı PIN'i",
      "setupGuide": "Kurulum rehberi"
    },
    "fields": {
      "hostAddress": "Ana bilgisayar adresi veya JSON",
      "hostPlaceholder": "ör. 192.168.1.100:12345",
      "pinCode": "PIN kodu",
      "pinPlaceholder": "_000000_"
    },
    "buttons": {
      "scanQRCode": "QR kodu tara",
      "connect": "Bağlan",
      "connecting": "Bağlanıyor...",
      "startHosting": "Barındırmaya başla",
      "startingServer": "Sunucu başlatılıyor...",
      "stopHosting": "Barındırmayı durdur",
      "hostAgain": "Tekrar barındır",
      "done": "Tamam"
    },
    "status": {
      "connecting": "Bağlanıyor...",
      "connected": "Bağlandı",
      "waitingConfirmation": "Onay bekleniyor",
      "waitingConfirmationDesc": "Devam etmek için ana cihazda bağlantıyı onaylayın.",
      "syncing": "Senkronize ediliyor...",
      "transferringData": "Veri aktarılıyor",
      "syncInProgress": "Senkronizasyon devam ediyor",
      "live": "Canlı",
      "broadcasting": "Yayın yapılıyor",
      "clientsLabel": "Bağlı",
      "clientsUnit": "İstemciler"
    },
    "pinDescription": "Bu PIN'i bağlanan cihazla paylaş",
    "hostingDesc1": "Diğer cihazlar bu cihazdan bağlanıp veri senkronize edebilir.",
    "hostingDesc2": "Verilerin bağlı istemcilerle paylaşılacak.",
    "setupSteps": {
      "step1": "Uygulamayı başka bir cihazda aç",
      "step2": "Ayarlar → Yerel senkronizasyon'a git",
      "step3": "QR kodunu tara veya adresi gir"
    },
    "messages": {
      "completed": "Senkronizasyon tamamlandı!",
      "completedDesc": "Tüm veriler senkronize edildi",
      "error": "Bağlantı hatası",
      "outdatedClient": "Güncel olmayan istemci algılandı"
    },
    "disclaimer": "Senkronizasyon yerel ağın üzerinden çalışır. Her iki cihaz da aynı WiFi'de olmalıdır.",
    "modals": {
      "connectionRequest": "Bağlantı isteği",
      "requestMessage": "bu cihazla senkronize olmak istiyor.",
      "acceptConnection": "Bağlantıyı kabul et",
      "acceptDesc": "Bu cihazın veri senkronize etmesine izin ver",
      "decline": "Reddet",
      "declineDesc": "Bu bağlantı girişimini engelle",
      "readyToSync": "Senkronize etmeye hazır",
      "connectionEstablished": "Bağlantı kuruldu",
      "deviceReady": "hazır.",
      "startSyncMessage": "Veri senkronizasyonunu başlatmak için aşağıya dokun.",
      "startSyncing": "Senkronizasyonu başlat",
      "startSyncingDesc": "Veri aktarımını şimdi başlat"
    },
    "scanner": {
      "title": "QR kodu tara",
      "cancel": "Taramayı iptal et"
    },
    "unknownDevice": "_Bilinmeyen Cihaz",
    "aria": {
      "dismissStatus": "Senkronizasyon durumunu kapat",
      "dismissError": "Senkronizasyon hatasını kapat"
    },
    "stats": {
      "statusLabel": "Durum"
    }
  },
  "creationHelper": {
    "page": {
      "info": "Oluşturma Asistanı, yapay zeka yardımıyla karakter oluşturma sürecinde sana rehberlik eder. Karakter oluşturma sırasında kullanılan modeli ve araçları yapılandır.",
      "modelConfiguration": "Model Yapılandırması",
      "chatModel": "Sohbet Modeli",
      "selectedModel": "Seçili Model",
      "useAppDefault": "Uygulama varsayılanını kullan{{model}}",
      "useAppDefaultBase": "Uygulama varsayılanını kullan",
      "noModelsAvailable": "Kullanılabilir model yok",
      "chatModelDescription": "Karakter oluşturma konuşmaları için yapay zeka modeli",
      "streamingOutput": "Akış Çıktısı",
      "streamingDescription": "Yanıtları oluşturulurken göster",
      "imageGenerationModel": "Görsel Oluşturma Modeli",
      "noModelSelected": "Model seçilmedi",
      "noImageModelsAvailable": "Görsel modeli mevcut değil",
      "imageModelDescription": "Karakter avatarları oluşturmak için",
      "toolSelection": "Araç Seçimi",
      "smartToolSelection": "Akıllı Araç Seçimi",
      "smartToolDescription": "Yapay zeka hangi araçların kullanılacağını otomatik seçer",
      "smartToolEnabledHint": "Etkinleştirildiğinde, Oluşturma Asistanı ne oluşturmak istediğini sorar ve yalnızca ilgili araç setini yükler.",
      "smartToolDisabledHint": "Devre dışı bırakıldığında, Oluşturma Asistanı doğrudan açılır ve tüm etkin araçları kullanır; asistan ne inşa edileceğine karar verir.",
      "quickPresets": "Hızlı Hazır Ayarlar",
      "customSelection": "Özel seçim - {{count}} araç etkin",
      "footerInfo": "Akıllı Araç Seçimi etkinleştirildiğinde, yapay zeka bağlama göre hangi araçları kullanacağına karar verir. Hangi araçların kullanılabilir olduğunu manuel olarak kontrol etmek için devre dışı bırak.",
      "selectChatModel": "Sohbet Modeli Seç",
      "selectImageModel": "Görsel Modeli Seç",
      "searchModels": "Model ara..."
    },
    "categories": {
      "basic": "Temel",
      "content": "İçerik",
      "visual": "Görsel",
      "settings": "Ayarlar",
      "flow": "Akış",
      "persona": "Personalar",
      "lorebook": "Lore kitapları"
    },
    "presets": {
      "all": {
        "name": "Tüm araçlar",
        "desc": "Tüm mevcut araçları etkinleştir"
      },
      "essential": {
        "name": "Temel",
        "desc": "Yalnızca ad, tanım ve sahneler"
      },
      "minimal": {
        "name": "Minimal_",
        "desc": "Yalnızca ad ve tanım"
      }
    },
    "tools": {
      "setName": "Ad belirle",
      "setNameDesc": "Karakter adını belirle",
      "setDefinition": "Tanım belirle",
      "setDefinitionDesc": "Kişilik ve geçmişi belirle",
      "set_character_name": {
        "name": "Ad belirle",
        "desc": "Karakter adını belirle"
      },
      "set_character_definition": {
        "name": "Tanım belirle",
        "desc": "Kişilik ve geçmişi belirle"
      },
      "add_scene": {
        "name": "Sahne ekle",
        "desc": "Roleplay için başlangıç sahnesi ekle"
      },
      "update_scene": {
        "name": "Sahneyi güncelle",
        "desc": "Mevcut bir sahneyi düzenle"
      },
      "toggle_avatar_gradient": {
        "name": "Avatar gradyanı",
        "desc": "Avatarda gradyan kaplamasını aç/kapat"
      },
      "set_default_model": {
        "name": "Model belirle",
        "desc": "Konuşmalar için yapay zeka modelini belirle"
      },
      "set_system_prompt": {
        "name": "Sistem promptu",
        "desc": "Davranış yönergelerini belirle"
      },
      "get_system_prompt_list": {
        "name": "Promptları listele",
        "desc": "Mevcut promptları görüntüle"
      },
      "get_model_list": {
        "name": "Modelleri listele",
        "desc": "Mevcut modelleri görüntüle"
      },
      "use_uploaded_image_as_avatar": {
        "name": "Görsel avatar olarak",
        "desc": "Yüklenen görseli avatar olarak kullan"
      },
      "use_uploaded_image_as_chat_background": {
        "name": "Görsel arka plan olarak",
        "desc": "Yüklenen görseli arka plan olarak kullan"
      },
      "generate_image": {
        "name": "Görsel oluştur",
        "desc": "Yapay zeka modeliyle görsel oluştur"
      },
      "show_preview": {
        "name": "Önizleme göster",
        "desc": "Karakteri önizle"
      },
      "request_confirmation": {
        "name": "Onay iste",
        "desc": "Kaydedilip devam edilip edilmeyeceğini sor"
      },
      "list_personas": {
        "name": "Personaları listele",
        "desc": "Personalara göz at"
      },
      "upsert_persona": {
        "name": "Persona kaydet",
        "desc": "Persona oluştur veya güncelle"
      },
      "use_uploaded_image_as_persona_avatar": {
        "name": "Persona avatarı",
        "desc": "Yüklenen görseli persona avatarı olarak kullan"
      },
      "delete_persona": {
        "name": "Personayı sil",
        "desc": "Bir personayı sil"
      },
      "get_default_persona": {
        "name": "Varsayılan persona",
        "desc": "Varsayılan personayı al"
      },
      "list_lorebooks": {
        "name": "Lore kitaplarını listele",
        "desc": "Lore kitaplarına göz at"
      },
      "upsert_lorebook": {
        "name": "Lore kitabını kaydet",
        "desc": "Lore kitabı oluştur veya güncelle"
      },
      "delete_lorebook": {
        "name": "Lore kitabını sil",
        "desc": "Lore kitabını sil"
      },
      "list_lorebook_entries": {
        "name": "Girdileri listele",
        "desc": "Lore kitabı girdilerini görüntüle"
      },
      "get_lorebook_entry": {
        "name": "Girdi al",
        "desc": "Lore kitabı girdisini al"
      },
      "upsert_lorebook_entry": {
        "name": "Girdi kaydet",
        "desc": "Girdi oluştur veya güncelle"
      },
      "delete_lorebook_entry": {
        "name": "Girdiyi sil",
        "desc": "Lore kitabı girdisini sil"
      },
      "create_blank_lorebook_entry": {
        "name": "Boş girdi",
        "desc": "Boş bir girdi oluştur"
      },
      "reorder_lorebook_entries": {
        "name": "Girdileri yeniden sırala",
        "desc": "Girdi sırasını değiştir"
      },
      "list_character_lorebooks": {
        "name": "Karakter kitaplarını listele",
        "desc": "Karakterin lore kitaplarını görüntüle"
      },
      "set_character_lorebooks": {
        "name": "Karaktere kitap ata",
        "desc": "Karaktere lore kitapları ata"
      },
      "addScene": "Sahne Ekle",
      "addSceneDesc": "Rol oyunu için bir başlangıç ​​sahnesi ekle",
      "updateScene": "Sahneyi Güncelle",
      "updateSceneDesc": "Mevcut bir sahneyi değiştirme",
      "avatarGradient": "Avatar Gradyanı",
      "avatarGradientDesc": "_Avatardaki degrade kaplamayı aç/kapat_",
      "setModel": "_Modeli Ayarla",
      "setModelDesc": "Konuşmalar için AI modelini ayarla",
      "systemPrompt": "Sistem İstemi",
      "systemPromptDesc": "Davranış yönergelerini ayarlayın",
      "listPrompts": "İstemleri Listele",
      "listPromptsDesc": "_Mevcut istemleri görüntüleyin_",
      "listModels": "_Modelleri Listele",
      "listModelsDesc": "Mevcut modelleri görüntüleyin",
      "imageAsAvatar": "Avatar olarak resim",
      "imageAsAvatarDesc": "Yüklenen resmi avatar olarak kullan"
    }
  },
  "tour": {
    "stepCounter": "Adım {{current}} / {{total}}",
    "skipTour": "Turu atla",
    "next": "İleri",
    "gotIt": "Anladım",
    "appShell": {
      "chats": {
        "title": "Sohbetlerin burada",
        "body": "Karakterlerle olan tüm bireysel konuşmaların burada. İstediğin zaman geri dön, yerini saklarız."
      },
      "groups": {
        "title": "Grup sohbetleri",
        "body": "Birden fazla karakteri aynı odada topla ve sohbet etmelerini izle ya da istediğin zaman katıl."
      },
      "discover": {
        "title": "Yeni karakterler keşfet",
        "body": "Topluluğun paylaştıklarını keşfet ve ilgini çeken herhangi bir karakteri ekle. Yeni favoriler bir dokunuş uzağında."
      },
      "library": {
        "title": "Kişisel kütüphanen",
        "body": "Oluşturduğun veya kaydettiğin her şey burada: karakterler, personalar, promptlar, her şey. Koleksiyonun gibi düşün."
      },
      "settings": {
        "title": "Kişiselleştir",
        "body": "Sağlayıcıları değiştir, farklı modeller seç, görünümü ayarla. Hemen hemen her şey ayarlardan yapılandırılabilir."
      },
      "search": {
        "title": "Her şeyi hızlıca bul",
        "body": "Belirli bir sohbet veya karakter mi arıyorsun? Her şeyi buradan ara. Gezinmeye gerek yok."
      },
      "create": {
        "title": "Ve son olarak, oluştur!",
        "body": "İlham geldiğinde artı butonuna dokun. Yeni bir karakter, persona oluştur veya sıfırdan bir şey başlat."
      }
    },
    "chatDetail": {
      "chatTitle": {
        "title": "Sohbet bazlı ayarlar",
        "body": "Bu sohbetin ayarlarını açmak için yukarıdaki karakter adına dokun. Konuşma başına farklı personalar, düzenler ve modeller."
      },
      "chatMemory": {
        "title": "Hatırladıkları",
        "body": "Beyin simgesi, karakterin konuşmalarından ne hatırladığını gösterir. Anıları incelemek, düzenlemek veya silmek için dokun."
      },
      "chatSearch": {
        "title": "O satırı bul",
        "body": "Yalnızca bu konuşmada ara. 200 mesaj önceki o detayı sonsuza kadar kaydırmadan bulmak için ideal."
      },
      "chatLorebook": {
        "title": "Lore kitabı girdileri",
        "body": "Belirli anahtar kelimeler göründüğünde prompta enjekte edilen ekstra veriler, dünya oluşturma ve bağlam. Karakterinin kopya kağıdı."
      },
      "chatPlus": {
        "title": "Bir şeyler ekle",
        "body": "Görsel ekle veya ekstralar menüsünü aç. Eklediğin şey bir sonraki mesajınla birlikte gönderilecek."
      },
      "chatComposer": {
        "title": "Mesajın, sıran",
        "body": "Buraya yaz. Enter gönderir, Shift+Enter yeni satır ekler. İpucu: düzenlemek, dallanmak veya silmek için herhangi bir sohbet mesajını basılı tut."
      },
      "chatSend": {
        "title": "Bir buton, dört işlev",
        "body": "Gönder butonu duruma göre işlevini değiştirir:"
      }
    },
    "postFirstMessage": {
      "chatRegenerate": {
        "title": "Beğenmedin mi? Yeniden oluştur",
        "body": "Karakterden tamamen yeni bir yanıt almak için yenile simgesine dokun. Her yeniden oluşturma, inceleyebileceğin bir varyant olarak kaydedilir."
      },
      "chatVariants": {
        "title": "Varyantlar arası kaydır",
        "body": "Yeniden oluşturduktan sonra mesajın altında bir varyant sayacı göreceksin. Farklı yanıtları görmek için mesaj balonunda sola veya sağa kaydır."
      },
      "chatLongPress": {
        "title": "Burada daha fazlası var",
        "body": "Düzenlemek, kopyalamak, dallanmak, sabitlemek, silmek veya konuşmayı geri sarmak için herhangi bir mesajı basılı tut. Masaüstünde sağ tıklama da çalışır."
      }
    },
    "sendButton": {
      "continue": {
        "label": "Devam et",
        "desc": "Boş alan. Dokunmak karakteri konuşmaya devam ettirmeye teşvik eder."
      },
      "send": {
        "label": "Gönder",
        "desc": "Bir şey yazdın veya ekledin. Göndermek için dokun."
      },
      "sending": {
        "label": "Gönderiliyor",
        "desc": "Yanıt yolda. Buton kilitli."
      },
      "stop": {
        "label": "Durdur",
        "desc": "Fikrini değiştirirsen yanıtı yarıda kesmek için dokun."
      }
    },
    "extra": {
      "rerunOnboarding": "İlk katılımı yeniden çalıştır"
    }
  },
  "sessionAdvanced": {
    "title": "Oturum parametreleri",
    "subtitle": "Bu konuşma için model varsayılanlarını geçersiz kıl",
    "goBack": "Geri dön",
    "support": "Destek",
    "reset": "Sıfırla",
    "save": "Kaydet",
    "noSessionWarning": "Oturum bazlı ayarları yapılandırmak için bir sohbet oturumu aç.",
    "overrideDefaults": "Varsayılanları geçersiz kıl",
    "overrideDefaultsDesc": "Parametreleri yalnızca bu konuşma için özelleştir",
    "loadingContextInfo": "Bağlam bilgisi yükleniyor...",
    "sampling": {
      "title": "Örnekleme",
      "temperature": "Sıcaklık",
      "temperatureDesc": "Rastgeleliği kontrol eder. Düşük = daha belirleyici, yüksek = daha yaratıcı.",
      "temperaturePrecise": "Hassas",
      "temperatureCreative": "Yaratıcı",
      "topP": "Top P",
      "topPDesc": "Çekirdek örnekleme. Tokenleri kümülatif olasılıkla sınırlar.",
      "topPFocused": "Odaklı",
      "topPDiverse": "Çeşitli",
      "topK": "Top K",
      "topKDesc": "Örneklemeyi en olası K tokene sınırlar."
    },
    "outputPenalties": {
      "title": "Çıktı ve cezalar",
      "maxOutputTokens": "Maksimum çıktı tokenleri",
      "maxOutputTokensDesc": "Maksimum yanıt uzunluğu. Otomatik, modelin karar vermesine izin verir.",
      "auto": "Otomatik",
      "custom": "Özel",
      "frequencyPenalty": "Frekans cezası",
      "frequencyPenaltyDesc": "Token dizisi tekrarını azaltır.",
      "frequencyPenaltyRepeat": "Tekrar",
      "frequencyPenaltyVary": "Çeşitlendir",
      "presencePenalty": "Varlık cezası",
      "presencePenaltyDesc": "Yeni konuların keşfini teşvik eder.",
      "presencePenaltyRepeat": "Tekrar",
      "presencePenaltyExplore": "Keşfet"
    },
    "performance": {
      "title": "Performans",
      "gpuLayers": "GPU Katmanları",
      "gpuLayersDesc": "GPU'ya yüklenen katmanlar. 0 = yalnızca CPU.",
      "threads": "İş parçacıkları",
      "threadsDesc": "Çıkarım için CPU iş parçacıkları.",
      "batchThreads": "Toplu iş parçacıkları",
      "batchThreadsDesc": "Toplu işleme için CPU iş parçacıkları.",
      "batchSize": "Toplu boyut",
      "batchSizeDesc": "Prompt işleme blok boyutu.",
      "contextLength": "Bağlam uzunluğu",
      "contextLengthDesc": "Bağlam penceresi boyutunu geçersiz kıl.",
      "flashAttention": "Flash Dikkati",
      "flashAttentionDesc": "GPU bellek optimizasyonu.",
      "enabled": "Etkin",
      "disabled": "Devre dışı"
    },
    "samplingMemory": {
      "title": "Örnekleme ve hafıza",
      "minP": "Min P",
      "minPDesc": "Minimum olasılık eşiği.",
      "typicalP": "Tipik P",
      "typicalPDesc": "Tipik örnekleme eşiği.",
      "seed": "Seed",
      "seedDesc": "Rastgele tohum. Rastgele için boş bırak.",
      "ropeBase": "RoPE Base",
      "ropeBaseDesc": "Frekans tabanı geçersiz kılma.",
      "ropeScale": "RoPE Scale",
      "ropeScaleDesc": "Frekans ölçeği geçersiz kılma.",
      "kvCacheType": "KV Önbellek Türü",
      "kvCacheTypeDesc": "VRAM tasarrufu için KV cache'i kuantize et.",
      "offloadKqv": "Boşaltma KQV",
      "offloadKqvDesc": "KV cache ve KQV işlemleri GPU'da.",
      "on": "Açık",
      "off": "Kapalı",
      "samplerProfile": "Örnekleyici profili",
      "samplerProfileDesc": "Kararlılık veya akıl yürütme için ayarlanmış hazır değerler.",
      "balanced": "Dengeli",
      "creative": "Yaratıcı",
      "stable": "Kararlı",
      "reasoning": "Akıl yürütme"
    },
    "systemInfo": {
      "title": "Sistem bilgisi",
      "maxContext": "Maksimum bağlam",
      "recommended": "Önerilen",
      "availableRam": "Kullanılabilir RAM",
      "availableVram": "Kullanılabilir VRAM",
      "modelSize": "Model boyutu"
    }
  },
  "exportMenu": {
    "title": "Dışa aktarma formatı",
    "selectFormat": "Bir format seç",
    "uscTitle": "Birleşik Sistem Kartı",
    "formats": {
      "uscPrompt": "Prompt şablonları için taşınabilir USC dışa aktarma.",
      "uscLorebook": "Lore kitapları için taşınabilir USC dışa aktarma.",
      "uscModel": "Model profilleri için taşınabilir USC dışa aktarma.",
      "uscChatTemplate": "Sohbet şablonları için taşınabilir USC dışa aktarma.",
      "legacyPromptJson": "Eski Prompt JSON",
      "legacyPromptJsonDesc": "Mevcut harici prompt paketi formatı.",
      "lorebookJson": "Lorebook JSON",
      "lorebookJsonDesc": "Mevcut lorebook dışa aktarma formatı.",
      "modelJson": "Model JSON",
      "modelJsonDesc": "Kimlik bilgisi içermeyen güvenli model profili JSON'ı.",
      "chatTemplateJson": "Sohbet Şablonu JSON",
      "chatTemplateJsonDesc": "Yerel sohbet şablonu dışa aktarma formatı."
    },
    "extra": {
      "selectFormat": "Biçim seçin_",
      "exportFormatTitle": "Dışa Aktarma Formatı_",
      "uscDesc": "_Taşınabilir USC dışa aktarımı",
      "legacyJsonDesc": "Eski JSON dışa aktarma biçimi",
      "formatV3Desc": "Karakter Kartı V3'ü dışa aktarma",
      "formatV2Desc": "Karakter Kartı V2'yi dışa aktarma",
      "formatV1Desc": "Karakter Kartı V1 dışa aktarma",
      "uscLorebook": "Birleşik Sistem Kartı (USC)",
      "portableLorebook": "Bilgi kitapları için taşınabilir USC dışa aktarımı",
      "lorebookJson": "Lorebook JSON",
      "currentLorebook": "Mevcut lorebook dışa aktarma formatı",
      "uscModel": "Birleşik Sistem Kartı (USC)",
      "portableModel": "Model profilleri için taşınabilir USC dışa aktarma",
      "modelJson": "Model JSON",
      "safeModel": "Kimlik bilgileri olmadan güvenli model profili JSON",
      "uscTemplate": "Birleşik Sistem Kartı (USC)",
      "portableTemplate": "Sohbet şablonları için taşınabilir USC dışa aktarma",
      "templateJson": "Sohbet Şablonu JSON",
      "nativeTemplate": "Yerel sohbet şablonu dışa aktarma formatı"
    }
  },
  "designReference": {
    "title": "Tasarım referansları",
    "description": "Birkaç net referans görsel ve kanonik bir görsel tanım yükle.",
    "descriptionPlaceholder": "Sabit görünümü tanımla: yüz, saç, beden yapısı, tahmini yaş, kıyafet ipuçları, aksesuarlar ve sanatsal yön/stil.",
    "addReferences": "Referans ekle",
    "visualDescription": "Görsel tanım",
    "draftWithAi": "Yapay zeka ile taslak",
    "referenceImages": "Referans görseller",
    "imageAlt": "Tasarım referansı {{index}}",
    "loading": "Yükleniyor...",
    "removeAria": "Tasarım referansını kaldır",
    "noImages": "Henüz referans görsel eklenmedi",
    "imageCount": "{{count}} referans görsel eklendi",
    "emptyReferences": "Yüz, oranlar, kıyafet ve stili sabitlemek için birkaç net referans fotoğraf ekle.",
    "noWriterModel": "Önce Görsel Oluşturma ayarlarından uyumlu bir sahne yazarı modeli ekle.",
    "noImagesForGeneration": "Oluşturmadan önce bir avatar veya en az bir referans görsel ekle.",
    "writerModelHelp": "Avatarından ve referans görsellerinden taslak hazırlamak için {{model}} kullan.",
    "noWriterModelHelp": "Bunu otomatik olarak taslak hazırlamak için Görsel Oluşturma ayarlarından uyumlu bir sahne yazarı modeli ekle.",
    "draftMenuTitle": "Yapay zeka ile tasarım taslağı",
    "draftMenuDesc": "Mevcut avatar ve referans görsellerden {{model}} tarafından hazırlandı.",
    "draftMenuNoWriter": "Bu asistanı kullanmadan önce uyumlu bir sahne yazarı modeli ekle.",
    "regenerate": "Yeniden oluştur",
    "useThis": "Bunu kullan"
  },
  "samplerOrder": {
    "title": "Örnekleyici sırası",
    "description": "Yeniden sıralamak için aşamaları sürükle. Yukarıdan aşağıya çalışır.",
    "reset": "Sıfırla",
    "resetAria": "Örnekleyici sırasını varsayılana sıfırla",
    "stages": {
      "penalties": {
        "label": "Cezalar",
        "desc": "Filtrelemeden önce frekans ve varlık cezalarını uygula."
      },
      "grammar": {
        "label": "Dilbilgisi",
        "desc": "Yerel sohbet şablonu dilbilgisi aktifken tokenleri kısıtla."
      },
      "topK": {
        "label": "Top K",
        "desc": "Aday havuzunu en güçlü tokenlere kırp."
      },
      "topP": {
        "label": "Top P",
        "desc": "Kalan dağılıma çekirdek filtreleme uygula."
      },
      "minP": {
        "label": "Min P",
        "desc": "Minimum eşik kullanarak düşük olasılıklı tokenleri kaldır."
      },
      "typical": {
        "label": "Tipik P",
        "desc": "Mevcut dağılımda istatistiksel olarak tipik tokenleri tercih et."
      },
      "temp": {
        "label": "Sıcaklık",
        "desc": "Seçimden önce son dağılımı düzleştir veya keskinleştir."
      }
    },
    "presets": {
      "default": {
        "label": "Varsayılan",
        "hint": "Lettuce yerel"
      },
      "unsloth": {
        "label": "Unsloth",
        "hint": "llama.cpp stili"
      },
      "focused": {
        "label": "Odaklı",
        "hint": "Sıkı budama"
      },
      "creative": {
        "label": "Yaratıcı",
        "hint": "Geç filtreleme"
      }
    }
  },
  "lorebookGen": {
    "flow": {
      "pickerCharactersTitle": "Karakterler",
      "pickerSessionsTitle": "Oturumlar",
      "noCharacters": "Karakter yok_",
      "noSessions": "Oturum yok",
      "clearSelection": "Seçimi temizle",
      "directionTitle": "İsteğe bağlı oluşturma yönü",
      "directionLabel": "Yön",
      "toggleForceMode": "Geçiş kuvvet modu",
      "entryTitlePlaceholder": "Giriş başlığı_",
      "entryContentPlaceholder": "Bilgi kitabı girişi içeriği_",
      "editDirectionBeforeRegenerate": "_Yeniden oluşturmadan önce yönü düzenleyin",
      "generatorReturnedNoDraft": "Oluşturucu taslak döndürmedi",
      "pageTitle": "Lorebook Girişi Oluştur",
      "missingContext": "Oluşturucu sayfası için lorebook içeriği eksik.",
      "characterLocked": "Karakter bu bilgi kitabının sahibine kilitlendi",
      "chooseSession": "Oturum seç",
      "pickCharacter": "Karakter seç",
      "searchMemories": "Anılarda ara",
      "searchMessages": "Mesajlarda ara",
      "selectLastN": "Son {{n}} mesajları seç",
      "selectAll": "Tümünü seç_",
      "loadSessionPrompt": "Yüklenecek oturumu seçin_",
      "messagesText": "_mesajlar",
      "memoriesText": "anılar",
      "messagesAndMemories": "mesajlar ve anılar",
      "titleAndContentRequired": "Başlık ve içerik gerekli",
      "keywordsOrAlwaysActive": "En az bir anahtar kelime ekleyin veya Her zaman etkin'i etkinleştirin_",
      "lorybookEntrySaved": "_Bilgi kitabı girişi kaydedildi_",
      "saveFailed": "Kaydetme başarısız oldu",
      "generationFailed": "Oluşturma başarısız oldu",
      "failedToLoadContext": "Lorebook oluşturucu yüklenemedi",
      "failedToLoadSessions": "Oturumlar yüklenemedi",
      "failedToLoadMessages": "Mesajlar yüklenemedi_",
      "userRole": "_USER",
      "aiRole": "AI"
    },
    "triggerPreview": {
      "resizeInspector": "Yeniden boyutlandırma denetçisi"
    }
  },
  "companion": {
    "download": {
      "emotionClassifier": "Duygu sınıflandırıcı",
      "emotionClassifierDesc": "Arkadaşın hissettiği, ifade ettiği ve bloke ettiği duygu vektörlerini okur ve günceller.",
      "emotionSize": "~120 MB",
      "entityExtractor": "Varlık çıkarıcı (NER)",
      "entityExtractorDesc": "İnsanları, yerleri ve nesneleri tanımlar, böylece anılar kanonikleştirilebilir ve bağlanabilir.",
      "entitySize": "~140 MB",
      "memoryRouter": "Bellek yönlendirici",
      "memoryRouterDesc": "Yeni dönüşlerin ilişki, kilometre taşı, epizodik veya diğer bellek kategorileri olarak mı saklanacağına karar verir.",
      "routerSize": "~70MB",
      "unknownModel": "Bilinmeyen tamamlayıcı model. "
    }
  },
  "voices": {
    "extra": {
      "customVoices": "Seslerim",
      "providerVoices": "Sağlayıcı Sesleri",
      "myVoices": "Seslerim",
      "page": {
        "noAudioProvidersHint": "Başlamak için Sağlayıcılar > Ses bölümünden bir tane ekleyin",
        "noVoicesTitle": "Henüz ses oluşturulmadı",
        "noVoicesDescription": "Karakterleriniz için özel promptlarla sesler oluşturun",
        "addProviderFirst": "Önce bir ses sağlayıcısı ekleyin",
        "noPrompt": "Prompt yok",
        "noProviderVoices": "Ses bulunamadı. Sesleri getirmek için Yenile'ye tıklayın.",
        "showLess": "Daha az göster",
        "showAllVoices": "{{count}} sesin tamamını göster",
        "voiceFallbackTitle": "Ses"
      },
      "cache": {
        "section": "Ses önbelleği",
        "title": "TTS ses önbelleği",
        "description": "Oluşturulan ses verisi, yeniden oluşturma sayısını azaltmak için önbelleğe alınır",
        "clearing": "Temizleniyor...",
        "clear": "Önbelleği temizle"
      },
      "menu": {
        "editDescription": "Bu sesi düzenle",
        "deleteDescription": "Bu sesi kaldır",
        "provider": "Sağlayıcı",
        "category": "Kategori",
        "createVoiceConfig": "Ses yapılandırması oluştur",
        "createVoiceConfigDescription": "Bu sesi özel ayarlarla kullan"
      },
      "editor": {
        "editTitle": "Sesi düzenle",
        "createTitle": "Ses oluştur",
        "voiceName": "Ses adı",
        "voiceNamePlaceholder": "Karakter Sesim",
        "provider": "Sağlayıcı",
        "model": "Model",
        "modelIdPlaceholder": "gpt-4o-mini-tts",
        "modelIdHint": "Uyumlu uç noktanızın beklediği tam model kimliğini girin",
        "elevenlabsVoice": "ElevenLabs Sesi",
        "noVoicesAvailable": "Kullanılabilir ses yok",
        "selectVoice": "Bir ses seçin...",
        "elevenlabsVoiceHint": "ElevenLabs seslerinizden seçin",
        "geminiVoice": "Gemini Sesi",
        "geminiVoiceHint": "Bir Gemini TTS sesi seçin",
        "voiceId": "Ses kimliği",
        "voiceIdPlaceholder": "alloy",
        "voiceIdHint": "Uyumlu uç noktanızın desteklediği ses kimliğini girin",
        "voicePrompt": "Ses promptu",
        "voicePromptPlaceholder": "Neşeli bir tonla sıcak ve samimi bir ses...",
        "voicePromptHint": "Sesin nasıl çıkması gerektiğini açıklayın",
        "exampleText": "Örnek metin",
        "exampleTextPlaceholder": "Merhaba! Konuşurken böyle çıkarım...",
        "exampleTextHint": "Sesi test etmek için örnek metin",
        "voiceDesignChars": "Ses tasarımı önizlemesi için {{current}}/{{minimum}} karakter gerekli",
        "defaultSample": "Merhaba! Konuşurken böyle çıkarım. Tonumu ve hızımı değerlendirmeniz için daha uzun pasajları sıcaklık, netlik ve duyguyla okuyabilirim.",
        "playing": "Çalıyor...",
        "previewVoice": "Sesi önizle"
      },
      "kokoro": {
        "title": "Kokoro Studio",
        "newBlend": "Yeni karışım",
        "editBlend": "Karışımı düzenle",
        "tryText": "Merhaba! Bu, nasıl çıktığımın hızlı bir testidir.",
        "experimentDefaultText": "Merhaba! Konuşurken böyle çıkarım. Daha uzun pasajları sıcaklık, netlik ve duyguyla okuyabilirim.",
        "livePreview": "Canlı önizleme",
        "savedBlend": "Kaydedilmiş karışım",
        "defaultPreviewText": "Merhaba! Bu sesin nasıl çıktığının hızlı bir önizlemesidir.",
        "experiment": "Deney",
        "providerNotFound": "Kokoro sağlayıcısı bulunamadı",
        "backToProviders": "Sağlayıcılara dön",
        "variantUnset": "Varyant ayarlı değil",
        "ready": "Hazır",
        "modelNotInstalled": "Model kurulu değil",
        "voiceCount": "{{count}} ses",
        "engineActions": "Motor işlemleri",
        "engineNotInstalled": "Motor kurulu değil",
        "installAtLeastOneVoice": "En az bir ses kurun",
        "continueSetup": "Kokoro modelini kurmak için kuruluma devam edin.",
        "pickVoiceOrStarter": "Başlamak için bir ses seçin veya başlangıç paketini alın.",
        "downloadsFailed": "{{count}} indirme başarısız",
        "retryOrDismissAll": "Tek tek yeniden deneyin veya tümünü kapatın.",
        "dismissAll": "Tümünü kapat",
        "model": "Model",
        "voice": "Ses",
        "downloads": "İndirmeler",
        "cancelAll": "Tümünü iptal et",
        "experimentPlaceholder": "Duydurmak istediğiniz bir ifade yazın...",
        "speed": "Hız",
        "speak": "Konuş",
        "yourBlends": "Karışımlarınız",
        "noSavedBlends": "Henüz kaydedilmiş karışım yok.",
        "installModelAndVoiceFirst": "Önce modeli ve bir sesi kurun.",
        "featured": "Öne çıkan",
        "stop": "Durdur",
        "sample": "Örnek",
        "voiceLibrary": "Ses kütüphanesi",
        "starterPack": "Başlangıç paketi",
        "select": "Seç",
        "all": "Tümü",
        "installed": "Kurulu",
        "installModelToBrowse": "Seslere göz atmak için modeli kurun.",
        "noVoicesInCatalog": "Katalogda ses yok. Yenile'ye dokunun.",
        "noVoicesMatch": "Filtrelerle eşleşen ses yok.",
        "collapseAll": "Tümünü daralt",
        "expandAll": "Tümünü genişlet",
        "selectedCount": "{{count}} seçildi",
        "engineTitle": "Kokoro motoru",
        "variant": "Varyant",
        "status": "Durum",
        "notInstalled": "Kurulu değil",
        "file": "Dosya",
        "modelSize": "Model boyutu",
        "voicesSize": "Ses boyutu",
        "total": "Toplam",
        "assetRoot": "Varlık kök dizini",
        "reinstallModel": "Modeli yeniden kur",
        "installModel": "Modeli kur",
        "deleteModel": "Modeli sil",
        "deleteModelDescription": "Disk alanını boşaltır; sesler korunur.",
        "blend": "Karışım",
        "previewDescription": "Varsayılan metinle hızlı dinleme",
        "editBlendDescription": "Sesleri, ağırlıkları ve hızı ayarla",
        "deleteBlendDescription": "Bu kaydedilmiş sesi kaldır",
        "setupTitle": "Kokoro'yu kur",
        "allSet": "Her şey hazır",
        "allSetDescription": "Seslere göz atın, karışımlar tasarlayın veya deney alanında test edin."
      },
      "badge": {
        "gemini": "G",
        "openai": "API",
        "elevenlabs": "11_"
      }
    }
  },
  "editPrompt": {
    "extra": {
      "conditionalsSection": "_Koşullar_",
      "injectionPoints": "_Enjeksiyon Noktaları"
    }
  },
  "embeddings": {
    "download": {
      "bestForQuick": "Hızlı yanıtlar için en iyisi",
      "balancedPerf": "Dengeli performans",
      "maxContext": "Maksimum bağlam",
      "capacity1k": "1K jeton_",
      "capacity2k": "2K jeton_",
      "capacity4k": "_4K jeton",
      "approxSize": "~138 MB",
      "downloadVersion": "V4"
    },
    "test": {
      "health": "Sağlık Kontrolü",
      "retrieval": "_Alma_",
      "separation": "_Ayırma_",
      "passed": "_Geçildi",
      "failed": "Başarısız",
      "testing": "Test ediliyor..."
    }
  },
  "convert": {
    "extra": {
      "uec": "Birleşik Varlık Kartı (UEC)",
      "charaCardV3": "Karakter Kartı V3",
      "charaCardV2": "Karakter Kartı V2"
    }
  },
  "companionSoulWriter": {
    "extra": {
      "json": "JSON",
      "jsonDesc": "Araç çağrısı kullanılamadığında kompakt yapılandırılmış çıktı.",
      "xml": "XML",
      "xmlDesc": "Model XML'i JSON'dan daha güvenilir bir şekilde biçimlendirdiğinde kullanın.",
      "fallbackFormat": "Geri Dönüş Biçimi"
    }
  },
  "usageAnalytics": {
    "page": {
      "filters": "Filtreler",
      "model": "Model",
      "character": "Karakter",
      "clearAll": "Tümünü Temizle",
      "applyFilters": "Filtreleri Uygula",
      "recentActivity": "Son Aktivite",
      "customRange": "Özel Aralık",
      "startDate": "Başlangıç Tarihi",
      "endDate": "Bitiş Tarihi",
      "applyRange": "Aralığı Uygula",
      "dashboard": "GÖSTERGE PANELİ",
      "appTime": "UYGULAMA SÜRESİ",
      "today": "Bugün",
      "last7Days": "7 Gün",
      "last30Days": "30 Gün",
      "all": "Tümü",
      "custom": "Özel",
      "filtersCount": "{{count}} Filtre",
      "totalCost": "Toplam Maliyet",
      "tokens": "Token",
      "avgShort": "{{value}} ort.",
      "requests": "İstekler",
      "period": "Dönem",
      "last7d": "Son 7g",
      "last30d": "Son 30g",
      "allTime": "Tüm Zamanlar",
      "recordsCount": "{{count}} kayıt",
      "usageTrend": "Kullanım Trendi",
      "tokenConsumptionOverTime": "Zaman içinde token tüketimi",
      "input": "Girdi",
      "output": "Çıktı",
      "byModel": "Modele Göre",
      "byCharacter": "Karaktere Göre",
      "top": "En İyi",
      "active": "Aktif",
      "quickView": "Hızlı Görünüm",
      "viewAll": "Tümünü Gör",
      "startChatting": "Kullanım verilerini görmek için sohbet başlatın",
      "exportedTo": "Dışa aktarıldı: {{path}}",
      "periodTotal": "Dönem Toplamı",
      "daysActive": "{{count}} aktif gün",
      "dailyAvg": "Günlük Ort.",
      "selectedPeriod": "seçili dönem",
      "yesterdayValue": "Dün {{value}}",
      "thirtyDayAvg": "30 Günlük Ort.",
      "appTimeTrend": "Uygulama Süresi Trendi",
      "usageDurationPerDay": "Günlük kullanım süresi",
      "totalValue": "Toplam {{value}}",
      "activeTime": "Aktif Süre"
    },
    "activity": {
      "loading": "Aktiviteniz yükleniyor...",
      "title": "Son Aktivite",
      "recordsCount": "{{count}} kullanım kaydı",
      "rangeOfTotal": "{{start}}-{{end}} / {{total}}",
      "previous": "Önceki",
      "next": "Sonraki",
      "pageOf": "{{page}} / {{total}} sayfa"
    },
    "shared": {
      "relativeTime": {
        "justNow": "az önce",
        "minutesAgo": "{{count}}d önce",
        "hoursAgo": "{{count}}s önce",
        "daysAgo": "{{count}}g önce"
      },
      "operations": {
        "chat": "Sohbet",
        "regenerate": "Yeniden Üret",
        "continue": "Devam Et",
        "summary": "Özet",
        "memoryManager": "Bellek",
        "imageGeneration": "Görsel Üretimi",
        "aiCreator": "AI Oluşturucu",
        "replyHelper": "Yanıt Yardımcısı",
        "groupChatMessage": "Grup Sohbeti",
        "groupChatRegenerate": "Grup Yeniden Üret",
        "groupChatContinue": "Grup Devam Et",
        "groupChatDecisionMaker": "Karar Verici"
      },
      "outputImages": "{{count}} görsel",
      "tokens": "{{count}} token",
      "unknown": "Bilinmiyor",
      "requestDetails": "İstek Ayrıntıları",
      "noStopReason": "Durdurma nedeni yok",
      "tokenUsage": "Token Kullanımı",
      "estimatedCost": "Tahmini Maliyet",
      "stats": {
        "prompt": "Prompt",
        "completion": "Tamamlama",
        "total": "Toplam",
        "reasoning": "Akıl Yürütme",
        "image": "Görsel",
        "memory": "Bellek",
        "summary": "Özet",
        "inputImages": "Girdi Görselleri",
        "outputImages": "Çıktı Görselleri",
        "cachedPrompt": "Önbelleğe Alınan Prompt",
        "cacheWrite": "Önbellek Yazma",
        "webSearches": "Web Aramaları",
        "cacheRead": "Önbellek Okuma",
        "requestFee": "İstek Ücreti",
        "webSearch": "Web Araması",
        "providerTotal": "Sağlayıcı Toplamı"
      }
    }
  }
};
