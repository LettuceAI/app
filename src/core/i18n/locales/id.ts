import type { DeepPartialMessageTree } from "../types";
import { type LocaleMessages } from "./en";
import type { LocaleMetadata } from ".";

export const idMetadata: LocaleMetadata = {
  name: "Indonesian",
  label: "Bahasa Indonesia",
};

export const idMessages: DeepPartialMessageTree<LocaleMessages> = {
  "common": {
    "nav": {
      "chats": "Obrolan",
      "settings": "Pengaturan",
      "providers": "Penyedia",
      "responseStyle": "Gaya Respons",
      "models": "Model",
      "security": "Keamanan",
      "accessibility": "Aksesibilitas",
      "reset": "Reset",
      "backupRestore": "Cadangan & Pemulihan",
      "convertFiles": "Konversi File",
      "usageAnalytics": "Analitik Penggunaan",
      "changelog": "Catatan Perubahan",
      "about": "Tentang",
      "createSystemPrompt": "Buat Prompt Sistem",
      "editSystemPrompt": "Edit Prompt Sistem",
      "systemPrompts": "Prompt Sistem",
      "developer": "Pengembang",
      "advanced": "Lanjutan",
      "characters": "Karakter",
      "lorebooks": "Lorebook",
      "personas": "Persona",
      "dynamicMemory": "Memori Dinamis",
      "creationHelper": "Asisten Pembuatan",
      "helpMeReply": "Bantu Saya Balas",
      "editPersona": "Edit Persona",
      "newTemplate": "Template Baru",
      "editTemplate": "Edit Template",
      "chatTemplates": "Template Obrolan",
      "editCharacter": "Edit Karakter",
      "sync": "Sinkronisasi",
      "newCharacter": "Karakter Baru",
      "engineSetup": "Pengaturan Engine",
      "llmProviders": "Penyedia LLM",
      "engineSettings": "Pengaturan Engine",
      "lettuceEngine": "Lettuce Engine",
      "create": "Buat",
      "setup": "Pengaturan",
      "welcome": "Selamat Datang",
      "conversation": "Percakapan",
      "library": "Perpustakaan",
      "groupChats": "Obrolan Grup",
      "groupChat": "Obrolan Grup",
      "imageGeneration": "Pembuatan Gambar",
      "voices": "Suara",
      "chatAppearance": "Tampilan Obrolan",
      "colorCustomization": "Warna Kustom",
      "embeddingDownload": "Unduh Embedding",
      "embeddingTest": "Tes Embedding",
      "editModel": "Edit Model",
      "messageDebug": "Debug Pesan",
      "speechRecognition": "Pengenalan Ucapan",
      "hostApi": "API Host",
      "lorebookEntryGenerator": "Penerus Entri Lorebook",
      "companionSoulWriter": "Penulis Jiwa Companion"
    },
    "bottomNav": {
      "chats": "Obrolan",
      "groups": "Grup",
      "create": "Buat",
      "discover": "Jelajahi",
      "library": "Perpustakaan"
    },
    "buttons": {
      "cancel": "Batal",
      "confirm": "Konfirmasi",
      "save": "Simpan",
      "saving": "Menyimpan...",
      "delete": "Hapus",
      "deleting": "Menghapus...",
      "create": "Buat",
      "creating": "Membuat...",
      "edit": "Edit",
      "back": "Kembali",
      "done": "Selesai",
      "download": "Unduh",
      "later": "Nanti",
      "proceed": "Lanjutkan",
      "retry": "Coba Lagi",
      "discard": "Buang",
      "import": "Impor",
      "importing": "Mengimpor...",
      "export": "Ekspor",
      "exporting": "Mengekspor...",
      "update": "Perbarui",
      "generate": "Hasilkan",
      "refresh": "Segarkan",
      "continue": "Lanjutkan",
      "goBack": "Kembali",
      "search": "Cari",
      "clearSearch": "Hapus pencarian",
      "add": "Tambah",
      "remove": "Hapus",
      "rename": "Ubah Nama",
      "copy": "Salin",
      "copied": "Tersalin!",
      "browseFiles": "Jelajahi File",
      "install": "Pasang"
    },
    "labels": {
      "processing": "Memproses...",
      "loading": "Memuat...",
      "noDescriptionYet": "Belum ada deskripsi",
      "untitled": "Tanpa Judul",
      "default": "Bawaan",
      "enabled": "Aktif",
      "disabled": "Nonaktif",
      "on": "Nyala",
      "off": "Mati",
      "none": "Tidak Ada",
      "auto": "Otomatis",
      "custom": "Kustom",
      "name": "Nama",
      "description": "Deskripsi",
      "content": "Konten",
      "preview": "Pratinjau",
      "options": "Opsi"
    },
    "time": {
      "justNow": "Baru saja",
      "minutesAgo": "{{minutes}} menit lalu",
      "hoursAgo": "{{hours}} jam lalu",
      "daysAgo": "{{days}} hari lalu",
      "today": "Hari ini",
      "yesterday": "Kemarin",
      "last7Days": "7 Hari Terakhir",
      "older": "Terdahulu"
    }
  },
  "settings": {
    "sections": {
      "intelligence": "Kecerdasan",
      "experience": "Pengalaman",
      "connectivity": "Konektivitas",
      "securityPrivacy": "Keamanan & Privasi",
      "supportInfo": "Dukungan & Info",
      "dangerZone": "Zona Bahaya",
      "developer": "Pengembang"
    },
    "items": {
      "providers": {
        "title": "Penyedia",
        "subtitle": "Hubungkan ke layanan AI"
      },
      "models": {
        "title": "Model",
        "subtitle": "Konfigurasi model AI"
      },
      "imageGeneration": {
        "title": "Pembuatan Gambar",
        "subtitle": "Hasilkan dan uji gambar"
      },
      "voices": {
        "title": "Suara",
        "subtitle": "Suara teks-ke-ucapan"
      },
      "accessibility": {
        "title": "Aksesibilitas",
        "subtitle": "Isyarat suara & haptik"
      },
      "prompts": {
        "title": "Prompt Sistem",
        "subtitle": "Bentuk kepribadian AI"
      },
      "security": {
        "title": "Keamanan",
        "subtitle": "Enkripsi & privasi"
      },
      "backup": {
        "title": "Cadangan & Pemulihan",
        "subtitle": "Ekspor atau impor data"
      },
      "convert": {
        "title": "Konversi File",
        "subtitle": "Migrasi ekspor .json lama ke .uec"
      },
      "sync": {
        "title": "Sinkronisasi Lokal",
        "subtitle": "Sinkronisasi antar perangkat"
      },
      "usage": {
        "title": "Analitik Penggunaan",
        "subtitle": "Biaya & statistik token"
      },
      "advanced": {
        "title": "Lanjutan",
        "subtitle": "Memori & fitur"
      },
      "logs": {
        "title": "Log",
        "subtitle": "Debug & diagnostik"
      },
      "guide": {
        "title": "Panduan Pengaturan",
        "subtitle": "Jalankan ulang orientasi"
      },
      "docs": {
        "title": "Dokumentasi",
        "subtitle": "Panduan & referensi"
      },
      "github": {
        "title": "Laporkan Masalah",
        "subtitle": "Bug & umpan balik • v{{version}}"
      },
      "discord": {
        "title": "Gabung Discord",
        "subtitle": "Komunitas & bantuan"
      },
      "about": {
        "title": "Tentang",
        "subtitle": "Versi, pembaruan & tautan"
      },
      "changelog": {
        "title": "Catatan Perubahan",
        "subtitle": "Yang baru"
      },
      "reset": {
        "title": "Reset",
        "subtitle": "Hapus semuanya"
      },
      "developer": {
        "title": "Alat Pengembang",
        "subtitle": "Debug & pengujian"
      }
    }
  },
  "accessibility": {
    "sectionTitles": {
      "language": "Bahasa",
      "sounds": "Umpan Balik Suara",
      "haptics": "Umpan Balik Haptik",
      "appearance": "Tampilan"
    },
    "language": {
      "appLanguage": "Bahasa aplikasi",
      "description": "Pilih bahasa yang digunakan di seluruh navigasi dan pengaturan."
    },
    "appearance": {
      "customColors": "Warna Kustom",
      "customColorsDesc": "Personalisasi skema warna aplikasi",
      "chatAppearance": "Tampilan Obrolan",
      "chatAppearanceDesc": "Kustomisasi gelembung pesan, font, dan tata letak"
    },
    "haptics": {
      "vibrateOnChat": "Getar saat Obrolan",
      "vibrateDesc": "Getaran pendek saat asisten sedang mengetik",
      "intensity": "Intensitas",
      "light": "Ringan",
      "medium": "Sedang",
      "heavy": "Kuat",
      "soft": "Lembut",
      "rigid": "Kaku"
    },
    "sounds": {
      "send": "Kirim",
      "sendDescription": "Berbunyi saat Anda mengirim pesan",
      "success": "Berhasil",
      "successDescription": "Berbunyi saat asisten berhasil menyelesaikan",
      "failure": "Gagal",
      "failureDescription": "Berbunyi saat terjadi kesalahan atau Anda membatalkan",
      "testButton": "Tes"
    },
    "feedbackInfo": "Umpan balik membantu Anda mengetahui saat pesan dikirim atau diterima.",
    "hapticsInfo": "Haptik tersedia di perangkat seluler.",
    "extra": {
      "easterEggs": "Easter Eggs",
      "beetrootRain": "Hujan Bit",
      "beetrootDesc": "Bit jatuh saat obrolan menyebutkannya"
    }
  },
  "topNav": {
    "unsavedChangesTitle": "Perubahan belum disimpan",
    "unsavedChangesMessage": "Simpan atau buang perubahan Anda sebelum pergi.",
    "goBack": "Kembali",
    "changeLayout": "Ubah tata letak",
    "search": "Cari",
    "settings": "Pengaturan",
    "help": "Bantuan",
    "add": "Tambah",
    "openFilters": "Buka filter",
    "save": "Simpan",
    "extra": {
      "installedModels": "Model yang Diinstal",
      "refresh": "Segarkan",
      "minimize": "Minimalkan",
      "maximize": "Maksimalkan",
      "close": "Tutup"
    }
  },
  "updates": {
    "available": {
      "title": "Versi baru tersedia",
      "description": "v{{latestVersion}} tersedia. Anda menggunakan v{{currentVersion}}.",
      "actions": {
        "view": "Lihat rilis"
      }
    }
  },
  "about": {
    "kicker": "Info Aplikasi",
    "appName": "LettuceAI",
    "description": "Detail versi, pemeriksaan pembaruan, dan tautan berguna.",
    "info": {
      "version": "Versi",
      "channel": "Kanal",
      "platform": "Platform"
    },
    "buildChannel": {
      "dev": "Build pengembangan",
      "release": "Rilis stabil"
    },
    "update": {
      "sectionTitle": "Pembaruan",
      "title": "Pembaruan aplikasi",
      "description": "Periksa rilis baru secara manual atau nonaktifkan pemeriksaan otomatis saat startup.",
      "autoChecks": "Pemeriksaan pembaruan otomatis",
      "autoChecksDescription": "Saat diaktifkan, LettuceAI akan memeriksa versi baru ketika aplikasi dibuka.",
      "checkNow": "Periksa pembaruan",
      "checking": "Memeriksa pembaruan...",
      "upToDateTitle": "Aplikasi Anda sudah terbaru",
      "upToDateDescription": "Saat ini tidak ada rilis yang lebih baru untuk perangkat ini.",
      "failedTitle": "Pemeriksaan pembaruan gagal",
      "failedDescription": "Tidak dapat memeriksa pembaruan sekarang. Coba lagi sebentar lagi."
    },
    "links": {
      "sectionTitle": "Tautan",
      "website": "Situs web",
      "websiteDescription": "Halaman unduhan dan informasi rilis",
      "github": "GitHub",
      "githubDescription": "Kode sumber, issue, dan pengembangan",
      "discord": "Discord",
      "discordDescription": "Server komunitas dan dukungan",
      "reddit": "Reddit",
      "redditDescription": "Diskusi komunitas, masukan, dan pembaruan"
    },
    "developerMode": {
      "enable": "Aktifkan Mode Pengembang",
      "enabled": "Mode Pengembang Diaktifkan"
    },
    "errors": {
      "saveTitle": "Tidak dapat menyimpan preferensi",
      "saveDescription": "Preferensi pemeriksaan pembaruan Anda tidak diubah."
    }
  },
  "components": {
    "tooltip": {
      "dismissHint": "Ketuk di mana saja untuk menutup"
    },
    "backgroundPosition": {
      "title": "Posisi Latar Belakang",
      "instructions": "Seret untuk memposisikan • Cubit atau gulir untuk memperbesar"
    },
    "avatarSource": {
      "generateImage": "Hasilkan Gambar",
      "generateImageDesc": "Pembuatan avatar bertenaga AI",
      "noImageModels": "Tidak ada model gambar tersedia",
      "editCurrent": "Edit Saat Ini",
      "editCurrentDesc": "Reposisi atau potong",
      "chooseImage": "Pilih Gambar",
      "chooseImageDesc": "Pilih dari perangkat Anda"
    },
    "avatarCurrentEdit": {
      "title": "Sunting Saat Ini",
      "reposition": "Reposisi",
      "repositionDesc": "Pindahkan atau pangkas avatar saat ini",
      "editWithAI": "Edit dengan AI",
      "editWithAIDesc": "Buka pengeditan AI untuk avatar saat ini",
      "noImageModels": "Tidak ada model gambar yang tersedia"
    },
    "avatarGeneration": {
      "modelsLoadError": "Gagal memuat model pembuatan gambar",
      "title": "Hasilkan Avatar",
      "help": "Bantuan pembuatan avatar",
      "model": "Model",
      "selectModel": "Pilih model",
      "describe": "Deskripsikan avatar Anda",
      "describePlaceholder": "Gadis anime ramah dengan rambut berwarna-warni, tersenyum hangat...",
      "inProgress": "Menghasilkan avatar...",
      "editingInProgress": "Menerapkan pengeditan avatar...",
      "previousVariant": "Varian sebelumnya",
      "nextVariant": "Varian selanjutnya",
      "variantCounter": "{{current}} / {{total}}",
      "editRequest": "Sunting permintaan",
      "editRequestPlaceholder": "Membuat rambut lebih gelap, menambahkan kacamata, menjaga wajah tetap sama...",
      "applyEdit": "Terapkan Sunting",
      "editImageLoadError": "Gagal menyiapkan avatar yang dihasilkan untuk diedit",
      "aiAssistant": "Asisten AI",
      "backToResults": "Kembali ke perintah",
      "magicInTheWorks": "Keajaiban sedang bekerja...",
      "refine": "Menyaring",
      "apply": "Menerapkan",
      "alt": "Avatar yang dihasilkan",
      "regenerate": "Hasilkan Ulang",
      "useThis": "Gunakan Ini"
    },
    "avatarPosition": {
      "title": "Posisi Avatar",
      "instructions": "Seret untuk memposisikan • Cubit atau gulir untuk memperbesar",
      "alt": "Avatar untuk diposisikan"
    },
    "confirmDialog": {
      "defaultTitle": "Konfirmasi",
      "defaultLabel": "Konfirmasi"
    },
    "bottomMenu": {
      "defaultTitle": "Menu",
      "dragTip": "Seret untuk menutup menu",
      "closeLabel": "Tutup menu",
      "buttonProcessing": "Memproses..."
    },
    "modelSelector": {
      "placeholder": "Pilih model",
      "clearLabel": "Gunakan bawaan global",
      "loading": "Memuat model...",
      "noModels": "Tidak ada model tersedia",
      "addProviderFirst": "Tambahkan penyedia di pengaturan terlebih dahulu",
      "title": "Pilih Model",
      "searchPlaceholder": "Cari model...",
      "noResults": "Tidak ada model ditemukan",
      "noResultsHint": "Coba kata pencarian yang berbeda"
    },
    "localeSelector": {
      "title": "Pilih Bahasa"
    },
    "promptTemplate": {
      "nameContentRequired": "Nama dan konten wajib diisi",
      "saveError": "Gagal menyimpan template",
      "editTitle": "Edit Prompt",
      "createTitle": "Buat Prompt",
      "name": "Nama",
      "namePlaceholder": "mis., Master Roleplay",
      "content": "Konten",
      "variablesButton": "Variabel",
      "contentPlaceholder": "Anda adalah asisten AI yang membantu...\n\nGunakan {{char.name}} dan {{scene}} dalam prompt Anda.",
      "characterCount": "{{count}} karakter",
      "preview": "Pratinjau",
      "characterPlaceholder": "Karakter…",
      "personaPlaceholder": "Persona…",
      "rendering": "Merender…",
      "noPreview": "Belum ada pratinjau",
      "saving": "Menyimpan...",
      "update": "Perbarui",
      "create": "Buat",
      "variablesTitle": "Variabel Template",
      "variablesCopyHint": "Ketuk untuk menyalin ke papan klip",
      "variablesCopied": "Tersalin",
      "variables": {
        "charName": "Nama Karakter",
        "charNameDesc": "Nama karakter",
        "charDesc": "Deskripsi Karakter",
        "charDescDesc": "Deskripsi karakter",
        "scene": "Adegan",
        "sceneDesc": "Skenario/adegan awal",
        "userName": "Nama Pengguna",
        "userNameDesc": "Nama persona pengguna",
        "userDesc": "Deskripsi Pengguna",
        "userDescDesc": "Deskripsi persona pengguna",
        "rules": "Aturan",
        "rulesDesc": "Aturan perilaku karakter",
        "contextSummary": "Ringkasan Konteks",
        "contextSummaryDesc": "Ringkasan percakapan dinamis",
        "keyMemories": "Memori Kunci",
        "keyMemoriesDesc": "Daftar memori yang relevan"
      }
    },
    "characterExport": {
      "title": "Format Ekspor",
      "selectFormat": "Pilih format",
      "loading": "Memuat format...",
      "formatUecDesc": "Format Unified Entity Card (.uec) (disarankan).",
      "formatLegacyJsonDesc": "JSON Legacy (hanya impor).",
      "formatV3Desc": "Character Card V3 JSON (spek terbaru).",
      "formatV2Desc": "Character Card V2 JSON (spek Tavern).",
      "formatV1Desc": "Character Card V1 (hanya impor)."
    },
    "embeddingDownload": {
      "downloadRequired": "Unduhan Diperlukan",
      "modelRequired": "Model Embedding Diperlukan",
      "description": "Memori Dinamis memerlukan pengunduhan model embedding lokal (~260 MB) untuk berfungsi.",
      "localStorage": "• Model akan disimpan secara lokal di perangkat Anda",
      "downloadSize": "• Ukuran unduhan: sekitar 260 MB",
      "summarization": "• Diperlukan untuk ringkasan percakapan"
    },
    "embeddingUpgrade": {
      "title": "Model Embedding v4 Tersedia",
      "v1Message": "Anda menggunakan v1 dengan 512 token. Tingkatkan ke v4 untuk kualitas memori lebih baik dan dukungan konteks panjang.",
      "v2Message": "Anda menggunakan v2 lama. Tingkatkan ke v4 untuk kualitas memori lebih baik dengan model embedding terbaru.",
      "v3Message": "v4 tersedia dan secara dramatis meningkatkan recall memori roleplay dibandingkan v3 (recall@1 0.02 -> 0.92). Peningkatan disarankan.",
      "button": "Tingkatkan ke v4"
    },
    "v2UpgradeToast": {
      "title": "Model Memori v3",
      "badge": "Tersedia",
      "message": "Kualitas embedding lebih baik dari v2",
      "dismiss": "Tutup",
      "upgrade": "Tingkatkan"
    },
    "v1UpgradeToast": {
      "title": "Model Memori v4 Tersedia",
      "message": "Tingkatkan untuk kualitas memori lebih baik dan dukungan konteks panjang.",
      "dismiss": "Tutup",
      "upgrade": "Tingkatkan"
    },
    "v3UpgradeToast": {
      "title": "Model memori v4",
      "badge": "Tersedia",
      "message": "v4 secara dramatis meningkatkan recall memori roleplay dibandingkan v3 (recall@1 0.02 - 0.92). Peningkatan disarankan.",
      "dismiss": "Nanti",
      "upgrade": "Tingkatkan"
    },
    "createMenu": {
      "title": "Buat Baru",
      "smartCreator": "Pembuat Pintar",
      "smartCreatorDesc": "Biarkan asisten memandu pembuatan Anda",
      "divider": "Atau buat secara manual",
      "character": "Karakter",
      "characterDesc": "Buat karakter kustom",
      "persona": "Persona",
      "personaDesc": "Buat suara yang dapat digunakan kembali",
      "groupChat": "Obrolan Grup",
      "groupChatDesc": "Ngobrol dengan banyak karakter",
      "lorebook": "Lorebook",
      "lorebookDesc": "Bangun referensi dunia Anda",
      "characterSmartDesc": "Bangun karakter dengan pembuatan terpandu",
      "personaSmartDesc": "Buat suara atau kepribadian yang dapat digunakan kembali",
      "lorebookSmartDesc": "Bangun referensi dunia terstruktur",
      "loadingConversations": "Memuat percakapan...",
      "createNew": "Buat baru",
      "createNewDesc": "Mulai obrolan pembuatan terpandu baru",
      "editExisting": "Edit yang ada",
      "continueLast": "Lanjutkan percakapan terakhir",
      "seeOlder": "Lihat yang lebih lama",
      "seeOlderDesc": "Buka percakapan Pembuat Pintar sebelumnya",
      "noConversations": "Belum ada percakapan untuk pembuat ini.",
      "sessionCompleted": "Selesai",
      "sessionCancelled": "Dibatalkan",
      "sessionDraft": "Draf",
      "sessionMessages": "{{count}} pesan",
      "untitledConversation": "Percakapan tanpa judul",
      "nameLorebookTitle": "Nama Lorebook",
      "lorebookNamePlaceholder": "Masukkan nama lorebook...",
      "lorebookImporting": "Mengimpor...",
      "lorebookImport": "Impor",
      "lorebookCreating": "Membuat...",
      "lorebookCreate": "Buat",
      "editExistingDesc": "Pilih {{goal}} dan edit dengan Pembuat Pintar",
      "creatorTitle": "Pembuat {{goal}}",
      "editTitle": "Edit {{goal}}",
      "conversationsTitle": "Percakapan {{goal}}",
      "loadingItems": "Memuat {{items}}...",
      "noItemsFound": "Tidak ada {{items}} ditemukan.",
      "unnamedCharacter": "Karakter tanpa nama",
      "untitledPersona": "Persona tanpa judul",
      "untitledLorebook": "Lorebook tanpa judul"
    },
    "advancedModelSettings": {
      "temperature": "Temperatur",
      "temperatureDesc": "Lebih tinggi = lebih kreatif",
      "topP": "Top P",
      "topPDesc": "Lebih rendah = lebih fokus",
      "maxTokens": "Token Output Maksimal",
      "maxTokensDesc": "Biarkan kosong untuk bawaan",
      "contextLength": "Panjang Konteks",
      "contextLengthDesc": "Hanya model lokal",
      "contextLengthAuto": "Otomatis",
      "frequencyPenalty": "Penalti Frekuensi",
      "frequencyPenaltyDesc": "Kurangi pengulangan token",
      "presencePenalty": "Penalti Kehadiran",
      "presencePenaltyDesc": "Dorong topik baru",
      "topK": "Top K",
      "topKDesc": "Batasi ukuran kumpulan token",
      "reasoning": "Pemikiran / Penalaran",
      "reasoningAutoDesc": "Model ini selalu menggunakan penalaran. Tidak perlu konfigurasi.",
      "reasoningEnableDesc": "Aktifkan kemampuan berpikir lanjutan untuk penyelesaian masalah kompleks dan tugas penalaran.",
      "effortMode": "Mode Usaha",
      "budgetMode": "Mode Anggaran",
      "reasoningEffort": "Usaha Penalaran",
      "reasoningEffortDesc": "Mengontrol kedalaman berpikir",
      "reasoningEffortAuto": "Otomatis",
      "reasoningEffortLow": "Rendah",
      "reasoningEffortMed": "Sedang",
      "reasoningEffortHigh": "Tinggi",
      "reasoningBudget": "Anggaran Penalaran (token)",
      "reasoningBudgetDesc": "Token maksimal untuk berpikir",
      "reasoningEffortLowDesc": "Respons cepat dengan penalaran minimal",
      "reasoningEffortMediumDesc": "Kedalaman penalaran seimbang",
      "reasoningEffortHighDesc": "Kedalaman penalaran maksimal untuk masalah kompleks",
      "reasoningBudgetExtendedDesc": "Token maksimal untuk berpikir. Ditambahkan ke batas output."
    },
    "providerParameterSupport": {
      "unknownProvider": "Penyedia tidak dikenal: {{providerId}}",
      "reasoningNotSupportedEffort": "Tidak didukung - penyedia ini tidak menggunakan level usaha",
      "reasoningNotSupportedBudgetOnly": "Tidak didukung - penyedia ini menggunakan pendekatan anggaran saja",
      "reasoningNotSupported": "Tidak didukung - penyedia ini tidak mendukung penalaran",
      "unsupportedParametersIgnored": "Parameter yang tidak didukung akan diabaikan oleh {{providerName}}.",
      "reasoningEffortSupported": "Usaha penalaran didukung untuk model berpikir (o1, DeepSeek-R1, dll.)",
      "reasoningBudgetSupported": "Penyedia ini menggunakan pemikiran berbasis anggaran (tanpa level usaha). Atur token anggaran penalaran sebagai gantinya.",
      "reasoningNotSupportedProvider": "Penyedia ini tidak mendukung parameter penalaran.",
      "matrixTitle": "Matriks Dukungan Parameter Penyedia",
      "providerColumn": "Penyedia",
      "supportedIndicator": "Didukung oleh API penyedia",
      "notSupportedIndicator": "Tidak didukung (parameter akan diabaikan)"
    },
    "extra": {
      "promptCachingTitle": "Prompt Caching",
      "promptCachingDescription": "Mempercepat pembuatan dan mengurangi biaya untuk konteks panjang berulang (seperti system prompt besar atau riwayat obrolan dalam).",
      "avatarAlt": "Avatar",
      "chooseFromLibrary": "Pilih dari perpustakaan",
      "chooseFromLibraryDesc": "Gunakan gambar yang sudah disimpan di aplikasi",
      "generateFailed": "Gagal menghasilkan gambar",
      "editFailed": "Gagal mengedit avatar",
      "backgroundAlt": "Latar belakang untuk diposisikan",
      "formatsLoadFailed": "Gagal memuat format ekspor",
      "formatsShowingDefaults": "(menampilkan bawaan)",
      "timeJustNow": "baru saja",
      "timeMinutesAgo": "{{minutes}}m lalu",
      "timeHoursAgo": "{{hours}}j lalu",
      "timeDaysAgo": "{{days}}h lalu",
      "removeReference": "Hapus referensi desain",
      "thumbLoading": "Memuat...",
      "addReferences": "Tambah referensi",
      "visualDescription": "Deskripsi visual",
      "draftWithAi": "Draf dengan AI",
      "regenerate": "Hasilkan Ulang",
      "useThis": "Gunakan Ini",
      "referenceImagesLabel": "Gambar referensi",
      "writerHelpFallback": "model penulis adegan yang kompatibel",
      "writerHelpUses": "Menggunakan {{model}} untuk membuat draf dari avatar dan gambar referensi Anda.",
      "writerHelpUnavailable": "Tambahkan model penulis adegan yang kompatibel di pengaturan Pembuatan Gambar untuk membuat ini secara otomatis.",
      "writerNotAvailableError": "Tambahkan model penulis adegan yang kompatibel di pengaturan Pembuatan Gambar terlebih dahulu.",
      "writerNoSourcesError": "Tambahkan avatar atau setidaknya satu gambar referensi sebelum membuat.",
      "noUsableReferences": "Tidak ada gambar referensi yang dapat digunakan ditemukan.",
      "draftFailed": "Gagal membuat deskripsi desain.",
      "draftReadFailed": "Gagal membaca aset gambar ({{status}})",
      "draftConvertFailed": "Gagal mengonversi aset gambar ke data URL",
      "draftGenerationFailed": "Pembuatan draf gagal.",
      "draftMenuTitle": "Draf Desain AI",
      "draftedBy": "Dibuat oleh {{model}} dari avatar dan gambar referensi saat ini.",
      "draftedByFallback": "model penulis adegan Anda",
      "noWriterUseHelper": "Tambahkan model penulis adegan yang kompatibel sebelum menggunakan pembantu ini.",
      "emptyReferences": "Tambahkan beberapa foto referensi yang jelas untuk mengunci wajah, proporsi, pakaian, dan gaya.",
      "designReferencesTitle": "Referensi desain",
      "designReferencesDescription": "Unggah beberapa gambar referensi yang jelas dan satu deskripsi visual kanonik.",
      "designReferencesPlaceholder": "Deskripsikan tampilan stabil: wajah, rambut, postur tubuh, presentasi usia, petunjuk pakaian, aksesori, dan arahan seni/gaya.",
      "dismissAria": "Tutup",
      "v3MessageFallback": "lettuce-emb-v4 tersedia dan secara dramatis meningkatkan recall memori roleplay. Peningkatan disarankan.",
      "uploadButton": "Unggah",
      "libraryButton": "Perpustakaan",
      "companionSetupTitle": "Companion perlu pengaturan",
      "companionSetupSubtitleSingle": "Mode Companion membutuhkan satu model lagi sebelum dapat berjalan. Melewati akan mengalihkan karakter ini kembali ke Roleplay.",
      "companionSetupSubtitleMany": "Mode Companion membutuhkan {{count}} model lagi sebelum dapat berjalan. Melewati akan mengalihkan karakter ini kembali ke Roleplay.",
      "companionSetupBody": "Mode Companion membutuhkan beberapa model lokal untuk menganalisis emosi, mengekstrak entitas, merutekan memori, dan mengingat konteks masa lalu.",
      "companionUseRoleplay": "Gunakan Roleplay saja",
      "companionDownloadNow": "Unduh sekarang",
      "searchModelsPlaceholder": "Cari model...",
      "loadingModelsDefault": "Memuat model...",
      "noModelsAvailable": "Tidak ada model tersedia.",
      "noModelsMatching": "Tidak ada model ditemukan yang cocok dengan \"{{query}}\".",
      "contentPlaceholderText": "Anda adalah asisten AI yang membantu...\n\nGunakan {{char.name}} dan {{scene}} dalam prompt Anda.",
      "previewRenderFailed": "<gagal merender pratinjau>",
      "charactersCount": "{{count}} karakter"
    }
  },
  "chats": {
    "characterNotFound": "Karakter tidak ditemukan",
    "chatHistory": "Riwayat Obrolan",
    "previousConversationsWithCharacter": "Percakapan sebelumnya dengan {{name}}",
    "previousConversations": "Percakapan sebelumnya",
    "searchChats": "Cari obrolan...",
    "searchResults": "{{count}} hasil",
    "noConversationsYet": "Belum ada percakapan",
    "startChattingPrompt": "Mulai mengobrol untuk melihat riwayat Anda di sini",
    "noMatchingChats": "Tidak ada obrolan yang cocok",
    "tryDifferentSearchTerm": "Coba kata pencarian yang berbeda",
    "untitledChat": "Obrolan Tanpa Judul",
    "chatTitlePlaceholder": "Judul obrolan...",
    "exportChatPackage": "Ekspor Paket Obrolan",
    "characterSpecificExport": "Ekspor Khusus Karakter",
    "characterSpecificExportDesc": "Ikat paket ini ke karakter obrolan secara bawaan.",
    "nonCharacterSpecificExport": "Ekspor Non-Khusus Karakter",
    "nonCharacterSpecificExportDesc": "Memerlukan pemilihan karakter saat impor.",
    "deleteChat": "Hapus obrolan?",
    "deleteConfirmDesc": "Menghapus secara permanen dari riwayat",
    "keepThisChat": "Simpan obrolan ini",
    "editCharacter": "Edit Karakter",
    "exportCharacter": "Ekspor Karakter",
    "chatAppearance": "Tampilan Obrolan",
    "importChatPackage": "Impor Paket Obrolan",
    "hideThisCharacter": "Sembunyikan karakter ini",
    "deleteCharacter": "Hapus Karakter",
    "deleteCharacterTitle": "Hapus Karakter?",
    "deleteCharacterConfirmation": "Apakah Anda yakin ingin menghapus \"{{name}}\"? Ini juga akan menghapus semua sesi obrolan dengan karakter ini.",
    "characterSpecificMatches": "Paket ini khusus karakter dan cocok dengan karakter yang dipilih.",
    "characterSpecificMismatch": "Paket ini khusus karakter dan merujuk ke karakter lain. Akan diimpor ke {{name}}.",
    "nonCharacterSpecificImport": "Paket ini non-khusus karakter. Akan diimpor ke {{name}}.",
    "noCharactersYet": "Belum ada karakter",
    "createFirstCharacter": "Buat karakter pertama Anda dari tombol + di bawah untuk mulai mengobrol",
    "scrollToBottom": "Gulir ke bawah",
    "selectCharacter": "Pilih Karakter",
    "branchToGroupChat": "Cabang Ke Obrolan Grup",
    "addContent": "Tambah Konten",
    "swapPlaces": "Tukar Posisi",
    "swapPlacesOn": "Tukar Posisi (Aktif)",
    "uploadImage": "Unggah Gambar",
    "helpMeReply": "Bantu Saya Balas",
    "sceneImage": {
      "modeTitle": "Gambar Adegan",
      "modeDescription": "Pilih apakah akan menulis sendiri adegan itu atau biarkan AI menyusunnya terlebih dahulu.",
      "writePrompt": "Tulis perintah",
      "writePromptDesc": "Ketik perintah gambar pemandangan yang ingin Anda gunakan.",
      "askAi": "Tanya AI",
      "askAiDesc": "Biarkan model obrolan saat ini membuat draf adegan dari momen yang dipilih.",
      "generateTitle": "Hasilkan Gambar Pemandangan",
      "regenerateTitle": "Regenerasi Gambar Pemandangan",
      "aiTitle": "Perintah Adegan AI",
      "promptLabel": "ADEGAN CEPAT",
      "promptPlaceholder": "Jelaskan adegan, karakter, suasana hati, pencahayaan, bingkai kamera, dan detail penting...",
      "suggestedPrompt": "Perintah yang disarankan",
      "regeneratePrompt": "Diperbarui",
      "editPrompt": "Edit perintah",
      "reviewTitle": "Tinjau prompt adegan",
      "denyPrompt": "Tolak",
      "acceptPrompt": "Terima",
      "generateImage": "Hasilkan Gambar",
      "updateImage": "Perbarui Gambar"
    },
    "useMyTextAsBase": "Gunakan teks saya sebagai dasar",
    "writeNewReply": "Tulis sesuatu yang baru",
    "suggestedReply": "Balasan yang Disarankan",
    "selectTwoCharactersMinimum": "Pilih minimal 2 karakter untuk obrolan grup.",
    "groupBranchCreated": "Cabang grup dibuat! Mengalihkan...",
    "memories": "Memori",
    "tools": "Alat",
    "pinned": "Disematkan",
    "searchMemories": "Cari memori...",
    "addMemory": "Tambah Memori",
    "memoryActions": "Tindakan memori",
    "pinnedMessages": "Pesan Disematkan",
    "pinnedMessagesDesc": "Selalu disertakan dalam konteks",
    "contextSummary": "Ringkasan Konteks",
    "contextSummaryPlaceholder": "Rekap singkat untuk menjaga konsistensi konteks antar pesan...",
    "memoryContentPlaceholder": "Apa yang harus diingat?",
    "editMemoryPlaceholder": "Masukkan konten memori...",
    "togglePin": {
      "pin": "Sematkan",
      "unpin": "Lepas Sematkan"
    },
    "toggleMemoryState": {
      "setHot": "Atur Panas",
      "setCold": "Atur Dingin"
    },
    "selectModelForRetry": "Pilih Model untuk Coba Ulang",
    "memoryCategories": {
      "characterTrait": "sifat karakter",
      "relationship": "hubungan",
      "plotEvent": "peristiwa alur",
      "worldDetail": "detail dunia",
      "preference": "preferensi",
      "other": "lainnya"
    },
    "settings": {
      "backToChat": "Kembali ke obrolan",
      "chatSettingsTitle": "Pengaturan Obrolan",
      "chatSettingsSubtitle": "Kelola preferensi percakapan",
      "memorySection": "Memori",
      "memorySectionDesc": "Ringkasan, tag, riwayat panggilan alat",
      "quickSettings": "Pengaturan Cepat",
      "quickSettingsDesc": "Penyesuaian paling umum",
      "persona": "Persona",
      "model": "Model",
      "fallbackModel": "Model Cadangan",
      "voice": "Suara",
      "voiceDesc": "Pemutaran teks-ke-ucapan",
      "advanced": "Lanjutan",
      "advancedDesc": "Ganti parameter model untuk sesi ini",
      "session": "Sesi",
      "sessionDesc": "Mulai obrolan baru dan jelajahi riwayat",
      "newChat": "Obrolan Baru",
      "newChatDesc": "Mulai percakapan baru",
      "chatHistoryDesc": "Lihat sesi sebelumnya",
      "importChatPackageDesc": "Impor .chatpkg ke karakter ini",
      "selectModel": "Pilih Model",
      "selectFallbackModel": "Pilih Model Cadangan",
      "personaActions": "Tindakan Persona",
      "sessionAdvancedSettings": "Pengaturan Lanjutan Sesi",
      "parameterSupport": "Dukungan Parameter",
      "modelDefaults": "Bawaan model",
      "appDefaults": "Bawaan aplikasi",
      "openChatSessionFirst": "Buka sesi obrolan terlebih dahulu",
      "sessionRequired": "Sesi diperlukan",
      "noPersona": "Tanpa persona",
      "customPersona": "Persona kustom",
      "noModelAvailable": "Tidak ada model tersedia",
      "fallbackNone": "Tidak ada",
      "unknownModel": "Model tidak dikenal",
      "authorNote": "Catatan Penulis",
      "identityProfileAuthored": "Profil identitas dibuat penulis",
      "addIdentityProfile": "Tambahkan profil identitas companion",
      "soulLabel": "Jiwa",
      "sessionTitle": "Sesi: {{title}}",
      "sessionUntitled": "Tanpa Judul",
      "messageCount": "{{count}} pesan",
      "usingCharacterDefault": "Menggunakan bawaan karakter",
      "sessionOverrideActive": "Penimpaan sesi aktif",
      "autoplayVoice": "Mainkan suara secara otomatis",
      "useCharacterDefault": "Gunakan bawaan karakter"
    },
    "search": {
      "placeholder": "Cari percakapan...",
      "noMessagesFound": "Tidak ada pesan ditemukan",
      "you": "Anda",
      "character": "Karakter",
      "failed": "Gagal mencari pesan"
    },
    "templates": {
      "startWithTemplate": "Mulai dengan template?",
      "noTemplate": "Tanpa template",
      "startWithSceneOnly": "Mulai dengan adegan saja",
      "you": "Anda",
      "bot": "Bot",
      "messageCount": "{{count}} pesan"
    },
    "header": {
      "back": "Kembali",
      "openSettings": "Buka pengaturan obrolan",
      "manageMemories": "Kelola memori",
      "searchMessages": "Cari pesan",
      "manageLorebooks": "Kelola lorebook",
      "conversationSettings": "Pengaturan percakapan"
    },
    "footer": {
      "sendMessagePlaceholder": "Kirim pesan...",
      "stopGeneration": "Hentikan pembuatan",
      "sendMessage": "Kirim pesan",
      "continueConversation": "Lanjutkan percakapan",
      "moreOptions": "Opsi lainnya",
      "addImage": "Tambah gambar",
      "addImageAttachment": "Tambah lampiran gambar",
      "removeAttachment": "Hapus lampiran",
      "recordVoice": "Rekam suara"
    },
    "message": {
      "thinkingMessages": {
        "thinkingReallyHard": "Berpikir sangat keras…",
        "lettuceCouncil": "Berkonsultasi dengan dewan selada…",
        "stealingThoughts": "Mencuri pikiran dari kekosongan…",
        "warmingBrainCells": "Memanaskan sel-sel otak…",
        "forbiddenKnowledge": "Memuat pengetahuan terlarang…",
        "overthinking": "Terlalu banyak berpikir (seperti biasa)…",
        "pretendingToBeSmart": "Berpura-pura pintar…",
        "crunchingNumbers": "Menghitung angka imajiner…",
        "arguingWithMyself": "Berdebat dengan diri sendiri…",
        "askingUniverse": "Meminta alam semesta dengan sopan…"
      },
      "thoughtProcess": "Proses pemikiran",
      "regenerateResponse": "Hasilkan ulang respons",
      "guidedRegenerationTitle": "Panduan regenerasi",
      "guidedRegenerationLabel": "Bagaimana seharusnya berubah?",
      "guidedRegenerationDescription": "Deskripsikan nada, panjang, detail yang perlu dipertahankan atau dihapus, dan apa yang seharusnya dilakukan balasan berikutnya secara berbeda.",
      "guidedRegenerationPlaceholder": "Buat lebih singkat, lebih hangat, lebih langsung...",
      "guidedRegenerationSubmit": "Hasilkan Ulang",
      "cancelAudioGeneration": "Batalkan pembuatan audio",
      "stopAudio": "Hentikan audio",
      "playMessageAudio": "Putar audio pesan",
      "playAudio": "Putar audio",
      "sceneLabel": "Adegan",
      "variantLabel": "Varian",
      "regenerating": "Menghasilkan ulang",
      "assistantIsTyping": "Asisten sedang mengetik",
      "attachedImage": "Gambar terlampir"
    },
    "actions": {
      "assistantMessage": "Pesan Asisten",
      "userMessage": "Pesan Pengguna",
      "promptTokens": "Token Prompt",
      "completionTokens": "Token Penyelesaian",
      "fallbackModelUsed": "Model cadangan digunakan",
      "total": "total",
      "timeToFirstToken": "Waktu ke token pertama",
      "completionTokenSpeed": "Kecepatan token penyelesaian",
      "edit": "Edit",
      "copy": "Salin",
      "pin": "Sematkan",
      "unpin": "Lepas Sematkan",
      "rewindToHere": "Mundur ke sini",
      "branchFromHere": "Cabang dari sini",
      "branchToGroupChat": "Cabang ke obrolan grup",
      "branchToCharacter": "Cabang ke karakter",
      "generateSceneImage": "Hasilkan gambar pemandangan",
      "regenerateSceneImage": "Regenerasi gambar pemandangan",
      "chatAppearance": "Tampilan Obrolan",
      "delete": "Hapus",
      "debug": "Debug",
      "unpinToDelete": "Lepas sematkan untuk menghapus",
      "editPlaceholder": "Edit pesan Anda...",
      "memoriesUsed": "{{count}} memori digunakan",
      "lorebookUsage": "Penggunaan lorebook",
      "lorebookUsageDesc": "Respons ini menggunakan entri lorebook berikut.",
      "matchScore": "Kecocokan: {{score}}%",
      "unknownModel": "Model tidak dikenal",
      "loadingModel": "Memuat model..."
    },
    "emptyState": {
      "goBack": "Kembali"
    },
    "settingsDrawer": {
      "title": "Pengaturan Obrolan",
      "subtitle": "Kelola preferensi percakapan"
    },
    "history": {
      "archivedBadge": "Diarsipkan",
      "messagesCount": "{{count}} pesan",
      "previousGroupPage": "Halaman {{label}} sebelumnya",
      "nextGroupPage": "Halaman {{label}} berikutnya",
      "sillyTavernFormat": "Format SillyTavern",
      "sillyTavernFormatDesc": "Ekspor sebagai .jsonl yang kompatibel dengan SillyTavern.",
      "failedLoad": "Gagal memuat data",
      "failedDelete": "Gagal menghapus: {{error}}",
      "failedRename": "Gagal mengubah nama: {{error}}",
      "chatPackageExportedTo": "Paket obrolan diekspor ke:\n{{path}}",
      "sillyTavernExportedTo": "Obrolan SillyTavern diekspor ke:\n{{path}}",
      "failedExportChatPackage": "Gagal mengekspor paket obrolan",
      "failedExportSillyTavern": "Gagal mengekspor obrolan SillyTavern"
    },
    "authorNote": {
      "title": "Catatan Penulis",
      "inlineEditor": "Editor inline",
      "inlineEditorDesc": "Tampilkan catatan penulis di atas input obrolan untuk pengeditan cepat.",
      "toggleInlineEditor": "Alihkan editor catatan penulis inline",
      "placeholder": "Panduan pribadi untuk obrolan ini",
      "willBeRemoved": "Catatan penulis akan dihapus saat disimpan",
      "noNoteSaved": "Tidak ada catatan penulis tersimpan",
      "charactersCount": "{{count}} karakter",
      "clear": "Hapus",
      "save": "Simpan",
      "saving": "Menyimpan...",
      "failedSave": "Gagal menyimpan catatan penulis",
      "addPlaceholder": "Tambahkan catatan penulis...",
      "savingShort": "Menyimpan..."
    },
    "drawer": {
      "chatSettingsTitle": "Pengaturan Obrolan",
      "chatSettingsSubtitle": "Kelola preferensi percakapan"
    },
    "companionSoul": {
      "loading": "Memuat Jiwa Companion...",
      "unavailable": "Jiwa Companion tidak tersedia",
      "unavailableDesc": "Pengeditan jiwa hanya tersedia untuk karakter mode companion.",
      "pageTitle": "Jiwa Companion",
      "back": "Kembali",
      "save": "Simpan"
    },
    "companionRelationship": {
      "back": "Kembali",
      "loading": "Memuat status hubungan...",
      "unavailableTitle": "Status hubungan tidak tersedia",
      "sessionLoadFailed": "Sesi obrolan tidak dapat dimuat.",
      "backToChat": "Kembali ke obrolan",
      "notCompanionTitle": "Obrolan ini bukan dalam mode companion",
      "notCompanionDesc": "Halaman hubungan companion hanya dirender untuk obrolan yang mode karakternya adalah companion.",
      "openRegularMemories": "Buka memori biasa",
      "pageTitle": "Status hubungan",
      "memoryButton": "Memori",
      "lastInteraction": "Interaksi terakhir {{time}}",
      "bond": "Ikatan",
      "closeness": "Kedekatan",
      "trust": "Kepercayaan",
      "affection": "Kasih Sayang",
      "tension": "Ketegangan",
      "stability": "Stabilitas",
      "interactions": "Interaksi",
      "vsDefaults": "vs. bawaan karakter",
      "updatedAt": "Diperbarui {{time}}",
      "emotionalEngine": "Mesin emosional",
      "felt": "Dirasakan",
      "feltDesc": "Pengaruh internal",
      "expressed": "Diekspresikan",
      "expressedDesc": "Muncul dalam balasan",
      "blocked": "Diblokir",
      "blockedDesc": "Ditekan oleh persona",
      "momentum": "Momentum",
      "momentumDesc": "Tren selama giliran terbaru",
      "activeDrivers": "Pendorong aktif",
      "soul": "Jiwa",
      "essence": "Esensi",
      "voice": "Suara",
      "relationalStyle": "Gaya relasional",
      "vulnerabilities": "Kerentanan",
      "habits": "Kebiasaan",
      "boundaries": "Batasan",
      "eventsCount": "{{count}} peristiwa",
      "recentTimeline": "Garis waktu terbaru",
      "superseded": "digantikan",
      "promptScore": "Prompt {{score}}",
      "persistenceScore": "Persistensi {{score}}",
      "noTimeline": "Belum ada garis waktu",
      "noTimelineDesc": "Memori hubungan, tonggak, dan snapshot emosional akan muncul di sini saat companion belajar dari percakapan.",
      "notAuthoredYet": "Belum dibuat.",
      "noSignal": "Tidak ada sinyal."
    },
    "companionUi": {
      "relationship": "Hubungan",
      "milestones": "Tonggak",
      "boundaries": "Batasan",
      "preferences": "Preferensi",
      "profile": "Profil",
      "routines": "Rutinitas",
      "episodes": "Episode",
      "emotionalSnapshots": "Snapshot emosional",
      "unknownTime": "Tidak Diketahui",
      "justNow": "Baru saja",
      "minutesAgo": "{{minutes}}m lalu",
      "hoursAgo": "{{hours}}j lalu",
      "daysAgo": "{{days}}d lalu",
      "notSetYet": "Belum diatur",
      "missingCharacterId": "characterId hilang",
      "sessionNotFound": "Sesi tidak ditemukan",
      "failedLoadCompanion": "Gagal memuat sesi companion"
    },
    "chatPage": {
      "characterNotFound": "Karakter tidak ditemukan",
      "characterDoesntExist": "Karakter yang Anda cari tidak ada."
    },
    "debugPage": {
      "copy": "Salin"
    },
    "companionMemoryPage": {
      "backLabel": "Kembali",
      "refineMemoryPlaceholder": "Perbaiki memori companion yang tersimpan",
      "superseded": "digantikan",
      "pinned": "disematkan",
      "cold": "dingin"
    }
  },
  "groupChats": {
    "list": {
      "editGroup": "Edit Grup",
      "deleteGroup": "Hapus Grup",
      "deleteConfirmTitle": "Hapus Obrolan Grup?",
      "deleteConfirmMessage": "Apakah Anda yakin ingin menghapus \"{{name}}\"? Ini juga akan menghapus semua pesan dalam obrolan grup ini.",
      "noGroupChatsYet": "Belum ada obrolan grup",
      "noGroupChatsDesc": "Buat obrolan grup pertama Anda dari tombol + di bawah untuk mengobrol dengan banyak karakter sekaligus",
      "newChat": "Chat Baru",
      "openChat": "Buka Chat",
      "chatSettings": "Pengaturan Chat",
      "sessionCount": "{{count}} obrolan"
    },
    "create": {
      "invalidPackage": "Paket ini bukan paket obrolan grup.",
      "inspectPackageError": "Gagal memeriksa paket obrolan grup",
      "importPackageError": "Gagal mengimpor paket obrolan grup",
      "importChatpkg": "Impor `.chatpkg`",
      "mapParticipantsTitle": "Petakan Peserta",
      "selectLocalCharacter": "Pilih karakter lokal untuk peserta ini.",
      "selectCharacterPlaceholder": "Pilih karakter...",
      "importChatPackageTitle": "Impor Paket Obrolan",
      "importChatPackageDesc": "Ini akan mengimpor `.chatpkg` yang dipilih sebagai sesi grup baru.",
      "characterSelect": {
        "title": "Buat Obrolan Grup",
        "subtitle": "Pilih karakter untuk percakapan grup Anda",
        "selected": "dipilih",
        "ready": "Siap",
        "minRequired": "Min. 2 diperlukan",
        "noCharactersYet": "Belum ada karakter",
        "noCharactersDesc": "Buat beberapa karakter terlebih dahulu untuk memulai obrolan grup",
        "continueToSetup": "Lanjutkan ke Pengaturan Grup"
      },
      "groupSetup": {
        "title": "Pengaturan Grup",
        "subtitle": "Konfigurasi pengaturan obrolan grup Anda",
        "chatType": "Tipe Obrolan",
        "conversation": "Percakapan",
        "casualChat": "Obrolan santai",
        "roleplay": "Roleplay",
        "withScenes": "Dengan adegan",
        "conversationDesc": "Percakapan grup santai tanpa adegan awal",
        "roleplayDesc": "Skenario roleplay dengan adegan awal dan prompt imersif",
        "speakerSelection": "Pemilihan Pembicara",
        "llm": "LLM",
        "aiPicks": "AI memilih",
        "heuristic": "Heuristik",
        "scoreBased": "Berbasis skor",
        "roundRobin": "Bergiliran",
        "takeTurns": "Bergantian",
        "llmDesc": "Menggunakan model bawaan Anda untuk memilih siapa yang berbicara (memerlukan token)",
        "heuristicDesc": "Menggunakan keseimbangan partisipasi dan petunjuk konteks (gratis)",
        "roundRobinDesc": "Karakter bergantian secara berurutan (gratis)",
        "chatBackground": "Latar Belakang Obrolan",
        "optional": "(Opsional)",
        "uploadBackground": "Unggah gambar latar belakang",
        "backgroundDesc": "Atur gambar latar belakang untuk obrolan grup ini",
        "groupName": "Nama Grup",
        "removeBackground": "Hapus gambar latar belakang",
        "groupNameAutoGenerate": "Biarkan kosong untuk otomatis dari nama karakter",
        "continueToScene": "Lanjutkan ke Adegan Awal",
        "createGroupChat": "Buat Obrolan Grup"
      },
      "startingScene": {
        "title": "Adegan Awal",
        "subtitle": "Atur skenario pembuka untuk roleplay Anda",
        "sceneSource": "Sumber Adegan",
        "none": "Tidak Ada",
        "custom": "Kustom",
        "fromCharacter": "Dari Karakter",
        "noneDesc": "Mulai tanpa adegan yang ditentukan",
        "customDesc": "Tulis skenario pembuka Anda sendiri",
        "fromCharacterDesc": "Gunakan adegan dari salah satu karakter Anda",
        "sceneContent": "Konten Adegan",
        "sceneContentPlaceholder": "Deskripsikan adegan awal untuk roleplay ini...",
        "sceneReferenceTip": "Tips: Ketik {{@\" untuk mereferensikan karakter",
        "selectScene": "Pilih Adegan",
        "sceneLabel": " Adegan",
        "copyToCustom": "Salin ke Kustom & Edit"
      }
    },
    "history": {
      "title": "Riwayat Obrolan Grup",
      "subtitle": "Semua percakapan grup",
      "searchPlaceholder": "Cari obrolan grup...",
      "active": "Aktif ({{count}})",
      "archived": "Diarsipkan ({{count}})",
      "noChatsYet": "Belum ada obrolan grup",
      "noChatsDesc": "Buat obrolan grup untuk melihat riwayat Anda di sini",
      "noMatchingChats": "Tidak ada obrolan yang cocok",
      "noMatchingDesc": "Coba kata pencarian yang berbeda",
      "deleteSessionTitle": "Hapus obrolan grup?",
      "deleteSessionDesc": "Menghapus secara permanen dari riwayat",
      "deleteSessionButton": "Hapus obrolan",
      "keepChat": "Simpan obrolan ini"
    },
    "session": {
      "chatTitlePlaceholder": "Judul obrolan...",
      "newChat": "Obrolan Baru",
      "rename": "Ubah Nama",
      "unarchive": "Batal Arsip",
      "archive": "Arsipkan",
      "messageCount": "{{count}} pesan"
    },
    "memories": {
      "tabMemories": "Memori",
      "tabPinned": "Disematkan",
      "tabActivity": "Aktivitas",
      "processing": "Memproses",
      "contextSummaryTitle": "Ringkasan Konteks",
      "addContextSummaryPrompt": "Ketuk untuk menambahkan ringkasan konteks...",
      "savedMemories": "Memori Tersimpan",
      "resultsCount": "Hasil ({{count}})",
      "searchPlaceholder": "Cari memori...",
      "addMemory": "Tambah memori",
      "noMemoriesYet": "Belum ada memori",
      "noMemoriesDesc": "Ketuk tombol Tambah di atas untuk membuat satu",
      "noMatchingMemories": "Tidak ada memori yang cocok",
      "noMatchingDesc": "Coba kata pencarian yang berbeda",
      "sessionNotFound": "Sesi tidak ditemukan",
      "memoryActions": "Tindakan memori",
      "tokens": "token",
      "cycle": "Siklus",
      "accessed": "Diakses",
      "cold": "Dingin",
      "hot": "Panas",
      "activityLog": "Log Aktivitas",
      "events": "peristiwa",
      "run": "Jalankan",
      "processingMemories": "AI sedang mengatur memori grup...",
      "memoryCycleSuccess": "Siklus memori berhasil diproses!",
      "memoryActionFailed": "Tindakan memori gagal",
      "newMemoryUpdates": "Pembaruan memori baru tersedia",
      "noActivityYet": "Belum ada aktivitas",
      "noActivityDesc": "Panggilan alat muncul saat AI mengelola memori dalam mode dinamis",
      "contextSummaryPlaceholder": "Rekap singkat untuk menjaga konsistensi konteks antar pesan...",
      "addMemoryTitle": "Tambah Memori",
      "memoryPlaceholder": "Apa yang harus diingat?",
      "saveMemory": "Simpan Memori",
      "editMemoryTitle": "Edit Memori",
      "editMemoryPlaceholder": "Masukkan konten memori...",
      "edit": "Edit",
      "pin": "Sematkan",
      "unpin": "Lepas Sematkan",
      "setHot": "Atur Panas",
      "setCold": "Atur Dingin"
    },
    "toolLog": {
      "created": "Dibuat",
      "deleted": "Dihapus",
      "pinned": "Disematkan",
      "unpinned": "Dilepas Sematkan",
      "done": "Selesai",
      "pinnedBadge": "disematkan",
      "softDelete": "hapus-lunak",
      "memoryCycle": "Siklus Memori",
      "failedAt": "Gagal pada:",
      "window": "Jendela",
      "justNow": "baru saja",
      "minutesAgo": "{{count}}m lalu",
      "hoursAgo": "{{count}}j lalu",
      "yesterday": "kemarin",
      "daysAgo": "{{count}}h lalu",
      "revertingTitle": "Mengembalikan...",
      "revertCycleTitle": "Kembalikan siklus ini",
      "revertedAt": "Dikembalikan {{time}}",
      "failedAtStage": "Gagal pada: {{stage}}",
      "hideDebug": "Sembunyikan Debug",
      "debug": "Debug",
      "windowRange": "Jendela {{start}}-{{end}}",
      "actionCreated": "Dibuat",
      "actionDeleted": "Dihapus",
      "actionPinned": "Disematkan",
      "actionUnpinned": "Dilepas Sematkan",
      "actionDone": "Selesai",
      "badgePinned": "disematkan",
      "badgeSoftDelete": "hapus-lunak",
      "badgeUndone": "dibatalkan",
      "badgeReverted": "dikembalikan",
      "activityEmptyTitle": "Belum ada aktivitas",
      "activityEmptyDesc": "Panggilan alat muncul saat AI mengelola memori dalam mode dinamis"
    },
    "message": {
      "thinkingHard": "Berpikir sangat keras…",
      "thinkingLettuce": "Berkonsultasi dengan dewan selada…",
      "thinkingVoid": "Mencuri pikiran dari kekosongan…",
      "thinkingBrainCells": "Memanaskan sel-sel otak…",
      "thinkingForbidden": "Memuat pengetahuan terlarang…",
      "thinkingOverthinking": "Terlalu banyak berpikir (seperti biasa)…",
      "thinkingPretending": "Berpura-pura pintar…",
      "thinkingCrunching": "Menghitung angka imajiner…",
      "thinkingArguing": "Berdebat dengan diri sendiri…",
      "thinkingUniverse": "Meminta alam semesta dengan sopan…",
      "thoughtProcess": "Proses pemikiran",
      "userAlt": "Pengguna",
      "assistantAlt": "Asisten",
      "regenerateResponse": "Hasilkan ulang respons",
      "variantLabel": "Varian",
      "regenerating": "Menghasilkan ulang",
      "cancelAudioGeneration": "Batalkan pembuatan audio",
      "stopAudio": "Hentikan audio",
      "playMessageAudio": "Putar audio pesan",
      "playAudio": "Putar audio",
      "attachedImage": "Gambar terlampir",
      "assistantIsTyping": "Asisten sedang mengetik",
      "assistantTyping": "Asisten sedang mengetik"
    },
    "header": {
      "back": "Kembali",
      "memories": "Memori",
      "settings": "Pengaturan",
      "characters": "karakter"
    },
    "footer": {
      "mentionCharacter": "Sebut karakter",
      "noCharactersFound": "Tidak ada karakter ditemukan",
      "moreOptions": "Opsi lainnya",
      "addImage": "Tambah gambar",
      "messagePlaceholder": "Pesan... (@ untuk menyebut)",
      "stopGeneration": "Hentikan pembuatan",
      "sendMessage": "Kirim pesan",
      "continueConversation": "Lanjutkan percakapan",
      "dismissError": "Tutup kesalahan",
      "removeAttachment": "Hapus lampiran",
      "tabToSelect": "Tab untuk memilih"
    },
    "messageActions": {
      "characterMessage": "Pesan Karakter",
      "yourMessage": "Pesan Anda",
      "whyCharacterResponded": "Mengapa karakter ini merespons",
      "total": "total",
      "edit": "Edit",
      "copy": "Salin",
      "regenerateWithDifferent": "Hasilkan ulang dengan karakter berbeda",
      "rewindToHere": "Mundur ke sini",
      "unpinToDelete": "Lepas sematkan untuk menghapus",
      "delete": "Hapus",
      "editPlaceholder": "Edit pesan Anda...",
      "chooseCharacterTitle": "Pilih Karakter",
      "selectCharacterForRegeneration": "Pilih karakter mana yang harus merespons:"
    },
    "settings": {
      "appDefault": "Bawaan aplikasi",
      "selectPersona": "Pilih Persona",
      "noPersonas": "Tidak ada persona tersedia",
      "noPersonasDesc": "Buat persona di pengaturan untuk mempersonalisasi percakapan Anda.",
      "searchPersonas": "Cari persona...",
      "noPersona": "Tanpa Persona",
      "noPersonaDesc": "Lanjutkan tanpa persona",
      "noPersonasFound": "Tidak ada persona ditemukan",
      "noPersonasFoundDesc": "Coba kata pencarian yang berbeda"
    },
    "groupSettings": {
      "title": "Edit Grup",
      "subtitle": "Perbarui pengaturan default untuk sesi mendatang",
      "enterGroupName": "Masukkan nama grup",
      "participant": "peserta",
      "participants": "peserta",
      "uploading": "Mengunggah...",
      "changeBackground": "Ubah latar belakang",
      "addBackgroundImage": "Tambah gambar latar belakang",
      "removeBackground": "Hapus latar belakang",
      "persona": "Persona",
      "personaSubtitle": "Persona default untuk sesi baru",
      "personaLabel": "Persona",
      "speakerSelection": "Pemilihan Pembicara",
      "speakerSubtitle": "Metode default untuk sesi baru",
      "llm": "LLM",
      "aiPicks": "AI memilih",
      "heuristic": "Heuristik",
      "scoreBased": "Berbasis skor",
      "roundRobin": "Bergiliran",
      "takeTurns": "Bergantian",
      "llmDesc": "Menggunakan model default Anda untuk memilih siapa yang berbicara (memerlukan token)",
      "heuristicDesc": "Menggunakan keseimbangan partisipasi dan petunjuk konteks (gratis)",
      "roundRobinDesc": "Karakter berbicara bergantian (gratis)",
      "memoryMode": "Mode Memori",
      "memorySubtitle": "Mode memori default untuk sesi baru",
      "manual": "Manual",
      "manualDesc": "Kelola catatan sendiri",
      "dynamic": "Dinamis",
      "dynamicDesc": "Pemanggilan otomatis",
      "memoryDynamicInfo": "AI secara otomatis membuat dan mengambil memori dari percakapan",
      "memoryManualInfo": "Anda menambahkan dan mengelola catatan memori sendiri",
      "characters": "Karakter",
      "participantsActive": "{{total}} peserta · {{active}} aktif",
      "add": "Tambah",
      "muted": "(dibisukan)",
      "mutedByDefault": "Dibisukan secara default",
      "activeByDefault": "Aktif secara default",
      "unmuteCharacter": "Aktifkan suara karakter",
      "muteCharacter": "Bisukan karakter",
      "minTwoRequired": "Minimal 2 karakter diperlukan",
      "removeCharacter": "Hapus karakter",
      "groupMinCharacters": "Sebuah grup memerlukan minimal 2 karakter",
      "mutedCharactersNote": "Karakter yang dibisukan dilewati oleh pemilihan pembicara otomatis, tetapi masih dapat merespons melalui `@mention` eksplisit.",
      "addCharacterTitle": "Tambah Karakter",
      "allCharactersInGroup": "Semua karakter sudah ada di grup ini.",
      "removeCharacterTitle": "Hapus Karakter?",
      "removeCharacterConfirm": "Apakah Anda yakin ingin menghapus",
      "removeCharacterFrom": "dari pengaturan default grup?",
      "removing": "Menghapus...",
      "remove": "Hapus"
    },
    "sessionSettings": {
      "subtitle": "Kelola preferensi chat grup",
      "enterGroupName": "Masukkan nama grup",
      "participant": "peserta",
      "participants": "peserta",
      "message": "pesan",
      "messages": "pesan",
      "uploading": "Mengunggah...",
      "changeBackground": "Ubah latar belakang",
      "addBackgroundImage": "Tambah gambar latar belakang",
      "removeBackground": "Hapus latar belakang",
      "persona": "Persona",
      "personaSubtitle": "Identitas Anda dalam percakapan ini",
      "personaLabel": "Persona",
      "speakerSelection": "Pemilihan Pembicara",
      "speakerSubtitle": "Bagaimana pembicara berikutnya dipilih",
      "llm": "LLM",
      "aiPicks": "AI memilih",
      "heuristic": "Heuristik",
      "scoreBased": "Berbasis skor",
      "roundRobin": "Bergiliran",
      "takeTurns": "Bergantian",
      "llmDesc": "Menggunakan model default Anda untuk memilih siapa yang berbicara (memerlukan token)",
      "heuristicDesc": "Menggunakan keseimbangan partisipasi dan petunjuk konteks (gratis)",
      "roundRobinDesc": "Karakter berbicara bergantian (gratis)",
      "characters": "Karakter",
      "participantsActive": "{{total}} peserta · {{active}} aktif",
      "add": "Tambah",
      "muted": "(dibisukan)",
      "unmuteCharacter": "Aktifkan suara karakter",
      "muteCharacter": "Bisukan karakter",
      "minTwoRequired": "Minimal 2 karakter diperlukan",
      "removeCharacter": "Hapus karakter",
      "groupMinCharacters": "Chat grup memerlukan minimal 2 karakter",
      "mutedCharactersNote": "Karakter yang dibisukan dilewati oleh pemilihan pembicara otomatis, tetapi masih dapat merespons melalui `@mention` eksplisit.",
      "data": "Data",
      "dataSubtitle": "Ekspor atau impor percakapan",
      "export": "Ekspor",
      "exportDesc": "Simpan sebagai file yang dapat dibagikan",
      "import": "Impor",
      "importDesc": "Muat percakapan dari file",
      "conversation": "Percakapan",
      "conversationSubtitle": "Duplikat atau lanjutkan di chat baru",
      "duplicate": "Duplikat",
      "duplicateDesc": "Salin chat ini dengan atau tanpa pesan",
      "branchTo1on1": "Cabang ke 1-lawan-1",
      "branchTo1on1Desc": "Lanjutkan secara pribadi dengan satu karakter",
      "participation": "Partisipasi",
      "participationSubtitle": "Distribusi bicara antar karakter",
      "addCharacterTitle": "Tambah Karakter",
      "allCharactersInGroup": "Semua karakter sudah ada di grup ini.",
      "removeCharacterTitle": "Hapus Karakter?",
      "removeCharacterConfirm": "Apakah Anda yakin ingin menghapus",
      "removeCharacterFrom": "dari chat grup ini?",
      "removing": "Menghapus...",
      "remove": "Hapus",
      "cloneGroupTitle": "Klon Grup",
      "withMessages": "Dengan pesan",
      "withMessagesDesc": "Klon semua termasuk riwayat chat",
      "withoutMessages": "Tanpa pesan",
      "withoutMessagesDesc": "Klon hanya pengaturan (karakter, adegan awal)",
      "branchWithCharacterTitle": "Cabang dengan Karakter",
      "branchWithCharacterDesc": "Pilih karakter untuk melanjutkan sebagai percakapan 1-lawan-1. Semua pesan dari grup ini akan dikonversi.",
      "continueWith": "Lanjutkan percakapan dengan {{name}}",
      "exportChatPackageTitle": "Ekspor Paket Chat",
      "includeCharacterSnapshots": "Sertakan snapshot karakter",
      "includeCharacterSnapshotsDesc": "Simpan data karakter di dalam paket",
      "sessionOnly": "Sesi saja",
      "sessionOnlyDesc": "Ekspor hanya pesan dan metadata",
      "mapParticipantsTitle": "Petakan Peserta",
      "selectLocalCharacter": "Pilih karakter lokal untuk peserta ini.",
      "selectCharacterPlaceholder": "Pilih karakter...",
      "continue": "Lanjutkan",
      "importChatPackageTitle": "Impor Paket Chat",
      "importChatPackageDesc": "Ini akan mengimpor `.chatpkg` yang dipilih sebagai sesi grup baru.",
      "importing": "Mengimpor..."
    },
    "page": {
      "selectingCharacter": "Memilih karakter...",
      "sessionNotFound": "Sesi grup tidak ditemukan",
      "backToGroupChats": "Kembali ke Obrolan Grup",
      "startConversation": "Mulai percakapan dengan {{names}}",
      "scrollToBottom": "Gulir ke bawah",
      "addContent": "Tambah Konten",
      "uploadImage": "Unggah Gambar",
      "helpMeReply": "Bantu Saya Balas",
      "helpMeReplyDesc": "Biarkan AI menyarankan apa yang harus dikatakan",
      "haveDraftPrompt": "Anda memiliki pesan draf. Bagaimana Anda ingin melanjutkan?",
      "useMyTextAsBase": "Gunakan teks saya sebagai dasar",
      "useMyTextAsBaseDesc": "Perluas dan perbaiki draf Anda",
      "writeSomethingNew": "Tulis sesuatu yang baru",
      "writeSomethingNewDesc": "Hasilkan balasan segar",
      "suggestedReply": "Balasan yang Disarankan",
      "reasoningBeforeWriting": "Berpikir sebelum menulis balasan Anda...",
      "writingYourReply": "Menulis balasan Anda...",
      "regenerate": "Hasilkan Ulang",
      "useReply": "Gunakan Balasan",
      "helpMeReplyFailedGeneric": "Bantu Saya Balas gagal.",
      "helpMeReplyFailedGenerate": "Bantu Saya Balas gagal menghasilkan balasan.",
      "noAudioCaptured": "Tidak ada audio yang ditangkap.",
      "noWhisperModel": "Model Whisper yang terpasang tidak ditemukan. Pasang di pengaturan Pengenalan Ucapan.",
      "cancelRecording": "Batalkan perekaman",
      "transcribing": "Mentranskrip",
      "stopAndTranscribe": "Hentikan dan transkrip",
      "recordVoice": "Rekam suara",
      "learnCorrection": "Pelajari koreksi:",
      "learning": "Mempelajari...",
      "learn": "Pelajari",
      "ignore": "Abaikan",
      "groupChatFailed": "Obrolan grup gagal.",
      "failedToCopy": "Gagal menyalin",
      "copied": "Tersalin!",
      "cannotDeletePinned": "Tidak dapat menghapus pesan yang disematkan. Lepas sematkan terlebih dahulu.",
      "failedToDelete": "Gagal menghapus",
      "messageNotFound": "Pesan tidak ditemukan",
      "cannotRewindPinned": "Tidak dapat memutar ulang: ada pesan yang disematkan setelah titik ini. Lepas sematkan terlebih dahulu.",
      "failedToRewind": "Gagal memutar ulang",
      "failedToTogglePin": "Gagal mengalihkan sematan",
      "messagePinned": "Pesan disematkan",
      "messageUnpinned": "Pesan dilepas sematkan",
      "failedToSave": "Gagal menyimpan"
    },
    "memoriesPage": {
      "summarizingConversation": "Merangkum percakapan",
      "analyzingMemories": "Menganalisis memori",
      "applyingChanges": "Menerapkan perubahan",
      "organizingMemories": "Mengatur memori",
      "retryingMemoryCycle": "Mencoba ulang Siklus Memori...",
      "processingMemories": "Memproses memori...",
      "memorySystemError": "Kesalahan Sistem Memori",
      "contextSummary": "Ringkasan Konteks",
      "contextSummaryTitle": "Ringkasan Konteks",
      "savedMemories": "Memori Tersimpan",
      "resultsCount": "Hasil ({{count}})",
      "searchMemoriesPlaceholder": "Cari memori...",
      "addMemory": "Tambah memori",
      "memoryActions": "Tindakan memori",
      "clearSearch": "Hapus pencarian",
      "noMatchingMemories": "Tidak ada memori yang cocok",
      "noMemoriesYet": "Belum ada memori",
      "tryDifferentSearch": "Coba kata pencarian yang berbeda",
      "tapAddToCreate": "Ketuk tombol Tambah di atas untuk membuat satu",
      "pinnedMessages": "Pesan Disematkan",
      "refresh": "Segarkan",
      "noPinnedMessages": "Tidak ada pesan yang disematkan",
      "pinImportantDesc": "Sematkan pesan obrolan grup penting untuk selalu menyertakannya dalam konteks.",
      "assistant": "Asisten",
      "user": "Pengguna",
      "unpin": "Lepas Sematkan",
      "failedToUnpinMessage": "Gagal melepas sematkan pesan",
      "activityLog": "Log Aktivitas",
      "run": "Jalankan",
      "addMemoryTitle": "Tambah Memori",
      "editMemoryTitle": "Edit Memori",
      "memoryTitle": "Memori",
      "memoryPlaceholder": "Apa yang harus diingat?",
      "saveMemory": "Simpan Memori",
      "editMemoryPlaceholder": "Masukkan konten memori...",
      "saving": "Menyimpan...",
      "save": "Simpan",
      "cancel": "Batal",
      "contextSummaryPlaceholder": "Rekap singkat untuk menjaga konsistensi konteks antar pesan...",
      "contextSummaryPrompt": "Ketuk untuk menambahkan ringkasan konteks...",
      "cycle": "Siklus",
      "accessed": "Diakses",
      "cold": "Dingin",
      "hot": "Panas",
      "tokens": "token",
      "pin": "Sematkan",
      "setHot": "Atur Panas",
      "setCold": "Atur Dingin",
      "edit": "Edit",
      "delete": "Hapus",
      "failedToToggleMemPin": "Gagal mengalihkan sematan",
      "failedToRemoveMemory": "Gagal menghapus memori",
      "toolEventsCountAria": "peristiwa",
      "activityEmptyDesc": "Panggilan alat muncul saat AI mengelola memori dalam mode dinamis",
      "memoryCycleSuccess": "Siklus memori berhasil diproses!"
    },
    "messageActionsExtra": {
      "characterMessage": "Pesan Karakter",
      "yourMessage": "Pesan Anda",
      "whyCharacterResponded": "Mengapa karakter ini merespons",
      "promptTokensTitle": "Token Prompt",
      "completionTokensTitle": "Token Penyelesaian",
      "total": "total",
      "regenerateWithDifferent": "Hasilkan ulang dengan karakter berbeda",
      "unpin": "Lepas Sematkan",
      "pin": "Sematkan",
      "rewindToHere": "Mundur ke sini",
      "unpinToDelete": "Lepas sematkan untuk menghapus",
      "editPlaceholder": "Edit pesan Anda...",
      "chooseCharacter": "Pilih Karakter",
      "selectCharacterForRegeneration": "Pilih karakter mana yang harus merespons:"
    },
    "timeAgo": {
      "justNow": "Baru saja",
      "minutesAgo": "{{count}}m lalu",
      "hoursAgo": "{{count}}j lalu",
      "daysAgo": "{{count}}h lalu"
    },
    "memoriesController": {
      "missingSessionId": "sessionId hilang",
      "sessionNotFound": "Sesi tidak ditemukan",
      "failedToLoadSession": "Gagal memuat sesi",
      "failedToUpdateTemperature": "Gagal memperbarui suhu memori",
      "failedToSaveSummary": "Gagal menyimpan ringkasan",
      "cycleFailed": "Siklus memori grup gagal",
      "failedToAddMemory": "Gagal menambahkan memori",
      "failedToUpdateMemory": "Gagal memperbarui memori",
      "failedToRunCycle": "Gagal menjalankan siklus memori",
      "failedToRevertCycle": "Gagal mengembalikan siklus",
      "failedToRefresh": "Gagal menyegarkan",
      "failedToDismissError": "Gagal menutup kesalahan",
      "failedToTogglePinned": "Gagal mengalihkan pesan yang disematkan",
      "sessionNotLoaded": "Sesi belum dimuat",
      "revertCycleTitle": "Kembalikan siklus ini?",
      "revertConfirm": "Kembalikan"
    },
    "chatController": {
      "sessionNotFound": "Sesi grup tidak ditemukan",
      "failedToLoadGroupChat": "Gagal memuat obrolan grup",
      "requestFailed": "Permintaan obrolan grup gagal",
      "failedToSendMessage": "Gagal mengirim pesan",
      "failedToRegenerate": "Gagal menghasilkan ulang",
      "failedToContinue": "Gagal melanjutkan",
      "failedToCopy": "Gagal menyalin",
      "cannotDeletePinned": "Tidak dapat menghapus pesan yang disematkan. Lepas sematkan terlebih dahulu.",
      "failedToDelete": "Gagal menghapus",
      "messageNotFound": "Pesan tidak ditemukan",
      "cannotRewindPinned": "Tidak dapat memutar ulang: ada pesan yang disematkan setelah titik ini. Lepas sematkan terlebih dahulu.",
      "failedToRewind": "Gagal memutar ulang",
      "failedToTogglePin": "Gagal mengalihkan sematan",
      "messagePinned": "Pesan disematkan",
      "messageUnpinned": "Pesan dilepas sematkan",
      "failedToSave": "Gagal menyimpan",
      "copied": "Tersalin!"
    },
    "historyController": {
      "failedToLoadData": "Gagal memuat data",
      "failedToDelete": "Gagal menghapus: {{error}}",
      "failedToRename": "Gagal mengubah nama: {{error}}",
      "failedToArchive": "Gagal mengarsipkan: {{error}}",
      "failedToUnarchive": "Gagal membatalkan arsip: {{error}}",
      "failedToDuplicate": "Gagal menduplikasi"
    },
    "sessionSettingsController": {
      "failedToLoad": "Gagal memuat pengaturan obrolan grup",
      "noPersona": "Tanpa persona",
      "customPersona": "Persona kustom",
      "minOneActive": "Minimal satu peserta harus tetap aktif"
    },
    "groupSettingsController": {
      "notFound": "Grup tidak ditemukan",
      "failedToLoad": "Gagal memuat pengaturan grup"
    },
    "createForm": {
      "failedToLoadCharacters": "Gagal memuat karakter",
      "selectAtLeastTwo": "Pilih minimal 2 karakter untuk obrolan grup",
      "failedToCreate": "Gagal membuat obrolan grup"
    },
    "groupSetupExtra": {
      "memoryMode": "Mode Memori",
      "manual": "Manual",
      "manualDesc": "Kelola catatan sendiri",
      "dynamic": "Dinamis",
      "dynamicDesc": "Ringkasan otomatis & pengambilan",
      "memoryManualInfo": "Anda menambahkan dan mengelola catatan memori sendiri",
      "memoryDynamicInfo": "AI secara otomatis membuat dan mengambil memori dari percakapan",
      "backgroundPreviewAlt": "Pratinjau latar belakang"
    },
    "createExtra": {
      "enterGroupNamePlaceholder": "Masukkan nama grup...",
      "unknown": "Tidak Diketahui"
    },
    "startingSceneExtra": {
      "insertAs": "Sisipkan sebagai {{snippet}}"
    },
    "sessionCardExtra": {
      "unknown": "Tidak Diketahui",
      "untitledChat": "Obrolan Tanpa Judul"
    },
    "sessionListExtra": {
      "unknown": "Tidak Diketahui"
    },
    "chatHeaderExtra": {
      "unknownError": "Kesalahan tidak diketahui"
    },
    "chatSettingsExtra": {
      "invalidPackage": "Paket ini bukan paket obrolan grup.",
      "failedExport": "Gagal mengekspor paket obrolan grup",
      "failedInspect": "Gagal memeriksa paket obrolan grup",
      "failedImport": "Gagal mengimpor paket obrolan grup",
      "exportSuccess": "Paket obrolan grup diekspor ke:\n{{path}}",
      "backgroundAlt": "Latar Belakang",
      "removeBackgroundAria": "Hapus latar belakang",
      "backAria": "Kembali",
      "backToGroupChats": "Kembali ke Obrolan Grup"
    },
    "groupSettingsExtra": {
      "backToGroups": "Kembali ke Grup",
      "backAria": "Kembali",
      "removeBackgroundAria": "Hapus latar belakang"
    },
    "historyPage": {
      "backAria": "Kembali ke obrolan grup",
      "clearSearchAria": "Hapus pencarian",
      "resultsLabel": "{{count}} hasil",
      "resultsLabelPlural": "{{count}} hasil",
      "untitledChat": "Obrolan Tanpa Judul",
      "noPinnedMessages": "Tidak ada pesan yang disematkan"
    },
    "personaSelectorExtra": {
      "insertAs": "Sisipkan sebagai",
      "appDefault": "Bawaan aplikasi",
      "defaultBadge": "Bawaan",
      "selectPersonaTitle": "Pilih Persona",
      "noPersonaTitle": "Tanpa Persona",
      "noPersonaDesc": "Lanjutkan tanpa persona",
      "noPersonasAvailable": "Tidak ada persona tersedia",
      "noPersonasDesc": "Buat persona di pengaturan untuk mempersonalisasi percakapan Anda.",
      "searchPersonas": "Cari persona...",
      "noPersonasFound": "Tidak ada persona ditemukan",
      "tryDifferentSearch": "Coba kata pencarian yang berbeda"
    },
    "footerExtra": {
      "cancelRecording": "Batalkan perekaman",
      "transcribing": "Mentranskrip",
      "stopAndTranscribe": "Hentikan dan transkrip",
      "recordVoice": "Rekam suara",
      "attachmentAltDefault": "Lampiran"
    },
    "pageExtra": {
      "noAudioCaptured": "Tidak ada audio yang ditangkap.",
      "noWhisperModelInstalled": "Model Whisper yang terpasang tidak ditemukan. Pasang di pengaturan Pengenalan Ucapan.",
      "scrollToBottomAria": "Gulir ke bawah"
    },
    "addContentMenu": {
      "title": "Tambah Konten",
      "uploadImage": "Unggah Gambar"
    },
    "helpMeReplyMenu": {
      "title": "Bantu Saya Balas",
      "helpMeReplyDesc": "Biarkan AI menyarankan apa yang harus dikatakan",
      "draftPrompt": "Anda memiliki pesan draf. Bagaimana Anda ingin melanjutkan?",
      "useTextAsBase": "Gunakan teks saya sebagai dasar",
      "useTextAsBaseDesc": "Perluas dan perbaiki draf Anda",
      "writeSomethingNew": "Tulis sesuatu yang baru",
      "writeSomethingNewDesc": "Hasilkan balasan segar"
    },
    "suggestedReplyMenu": {
      "title": "Balasan yang Disarankan",
      "reasoningBeforeWriting": "Berpikir sebelum menulis balasan Anda...",
      "writingYourReply": "Menulis balasan Anda...",
      "regenerate": "Hasilkan Ulang",
      "useReply": "Gunakan Balasan"
    },
    "memoriesPageExtra": {
      "sessionNotFound": "Sesi tidak ditemukan",
      "memorySystemError": "Kesalahan Sistem Memori",
      "retryingMemoryCycle": "Mencoba ulang Siklus Memori...",
      "processingMemories": "Memproses memori...",
      "memoryCycleSuccess": "Siklus memori berhasil diproses!",
      "contextSummaryTitle": "Ringkasan Konteks",
      "activityTabLabel": "Aktivitas",
      "noMatchingMemoriesDesc": "Coba kata pencarian yang berbeda",
      "chatHistoryTitle": "Riwayat Obrolan",
      "chatHistoryDesc": "Lihat dan kelola percakapan"
    },
    "settingsPageExtra": {
      "quickActionsSection": "Aksi Cepat",
      "chatHistoryTitle": "Riwayat Obrolan",
      "chatHistoryDesc": "Lihat dan kelola percakapan",
      "lorebrooksTitle": "Lorebook",
      "lorebrooksDesc": "Lampirkan lorebook sesi dan secara opsional abaikan lorebook masing-masing karakter.",
      "manageLorebooks": "Kelola lorebook"
    },
    "groupSettingsPageExtra": {
      "lorebrooksTitle": "Lorebook",
      "lorebrooksDesc": "Lampirkan lorebook bersama dan kontrol apakah lorebook karakter diterapkan secara bawaan.",
      "manageLorebooks": "Kelola lorebook"
    }
  },
  "characters": {
    "empty": {
      "title": "Belum ada karakter",
      "description": "Buat karakter AI kustom dengan kepribadian unik",
      "createButton": "Buat Karakter"
    },
    "progress": {
      "identity": "Identitas",
      "scenes": "Adegan",
      "details": "Detail"
    },
    "identity": {
      "title": "Buat Karakter",
      "subtitle": "Berikan identitas pada karakter AI Anda",
      "tapCameraToAdd": "Ketuk kamera untuk menambah atau menghasilkan avatar",
      "importingAvatar": "Mengimpor avatar...",
      "characterName": "Nama Karakter *",
      "characterNamePlaceholder": "Masukkan nama karakter...",
      "characterNameDesc": "Nama ini akan muncul dalam percakapan obrolan",
      "avatarGradient": "Gradien Avatar",
      "avatarGradientDesc": "Hasilkan gradien dinamis dari warna avatar",
      "chatBackgroundLabel": "Latar Belakang Obrolan",
      "optionalSuffix": "(Opsional)",
      "backgroundPreviewAlt": "Pratinjau latar belakang",
      "backgroundPreviewBadge": "Pratinjau Latar Belakang",
      "addBackground": "Tambah Latar Belakang",
      "addBackgroundHint": "Unggah atau pilih dari perpustakaan",
      "uploadImage": "Unggah gambar",
      "chooseFromLibrary": "Pilih dari perpustakaan",
      "backgroundDesc": "Gambar latar belakang opsional untuk percakapan obrolan",
      "continueToDescription": "Lanjutkan ke Deskripsi",
      "importCharacterFromFile": "Impor Karakter dari File",
      "importCharacterDesc": "Muat karakter dari kartu PNG, .uec, atau file ekspor .json"
    },
    "details": {
      "title": "Detail Karakter",
      "subtitle": "Definisikan kepribadian dan perilaku",
      "definition": "Definisi *",
      "wordCount": "{{count}} kata",
      "definitionPlaceholder": "Deskripsikan kepribadian, gaya bicara, latar belakang, bidang pengetahuan...",
      "definitionDesc": "Spesifik tentang nada, sifat, dan gaya percakapan",
      "availablePlaceholders": "Placeholder Tersedia:"
    },
    "scenes": {
      "title": "Adegan Awal",
      "subtitle": "Buat skenario pembuka untuk percakapan Anda",
      "default": "Bawaan",
      "hasSceneDirection": "Memiliki arahan adegan",
      "continueToScenes": "Lanjutkan ke Adegan Awal"
    },
    "extras": {
      "title": "Detail Tambahan",
      "subtitle": "Semua bidang opsional",
      "nickname": "Nama Panggilan",
      "nicknamePlaceholder": "Apa yang harus pengguna panggil karakter ini?",
      "nicknameDesc": "Nama tampilan alternatif yang digunakan dalam percakapan",
      "creator": "Pembuat",
      "creatorPlaceholder": "Nama pembuat...",
      "tags": "Tag",
      "tagsPlaceholder": "fantasi, sci-fi, romansa...",
      "tagsDesc": "Daftar dipisahkan koma untuk penyaringan dan organisasi",
      "creatorNotes": "Catatan Pembuat",
      "creatorNotesPlaceholder": "Tips penggunaan, konteks lore, atau instruksi untuk pengguna lain...",
      "multilingualNotes": "Catatan Multibahasa",
      "multilingualNotesHint": "Objek JSON dengan kode bahasa sebagai kunci",
      "creatingCharacter": "Membuat Karakter..."
    },
    "preview": {
      "unnamedCharacter": "Karakter Tanpa Nama",
      "sceneCount": "{{count}} adegan",
      "customPrompt": "Prompt kustom",
      "description": "Deskripsi",
      "startingScene": "Adegan Awal",
      "gradientEnabled": "Gradien aktif",
      "customModel": "Model kustom",
      "avatarAlt": "Avatar karakter",
      "characterFallback": "Karakter"
    },
    "personaPreview": {
      "unnamedPersona": "Persona Tanpa Nama",
      "noDescription": "Belum ada deskripsi",
      "default": "Bawaan",
      "description": "Deskripsi"
    },
    "lorebookPreview": {
      "untitledLorebook": "Lorebook Tanpa Judul",
      "entryCount": "{{count}} entri",
      "entries": "Entri",
      "noEntriesYet": "Belum ada entri",
      "untitledEntry": "Entri tanpa judul",
      "noContentYet": "Belum ada konten",
      "alwaysActive": "Selalu aktif",
      "moreEntries": "+{{count}} entri lainnya",
      "moreEntry": "+{{count}} entri lainnya"
    },
    "creationHelper": {
      "moreOptions": "Opsi lainnya",
      "describePlaceholder": "Deskripsikan karakter Anda...",
      "stopGeneration": "Hentikan pembuatan",
      "sendMessage": "Kirim pesan",
      "addToMessage": "Tambah ke Pesan",
      "uploadImageDesc": "Tambahkan avatar atau gambar referensi",
      "referenceCharacterDesc": "Gunakan karakter yang ada sebagai inspirasi",
      "referencePersonaDesc": "Gunakan persona Anda sebagai konteks",
      "retry": "Coba Ulang",
      "attachmentAlt": "Lampiran",
      "removeAttachment": "Hapus lampiran",
      "removeReference": "Hapus referensi",
      "uploadImageTitle": "Unggah Gambar",
      "referenceCharacterTitle": "Karakter Referensi",
      "referencePersonaTitle": "Persona Referensi"
    },
    "lorebook": {
      "keywords": "KATA KUNCI",
      "caseSensitive": "Peka huruf besar",
      "typeKeyword": "Ketik kata kunci...",
      "addButton": "Tambah",
      "untitledEntry": "Entri Tanpa Judul",
      "alwaysActive": "Selalu aktif",
      "noKeywords": "Tidak ada kata kunci",
      "dragToReorder": "Seret untuk mengurutkan ulang",
      "disabled": "Nonaktif",
      "noLorebooksYet": "Belum ada lorebook",
      "createLorebookDesc": "Buat lorebook untuk menambahkan lore dunia untuk karakter ini",
      "createLorebook": "Buat Lorebook",
      "searchPlaceholder": "Cari lorebook...",
      "noMatchingLorebooks": "Tidak ada lorebook yang cocok ditemukan",
      "activeLorebooks": "Lorebook Aktif",
      "sectionActive": "Aktif",
      "sectionAvailable": "Tersedia",
      "entryCount": "{{count}} entri",
      "enabledForCharacter": "diaktifkan untuk karakter ini",
      "enabledForGroup": "diaktifkan untuk grup ini",
      "enabledForSession": "diaktifkan untuk sesi ini",
      "tapToViewEntries": "Ketuk untuk melihat entri",
      "newLorebookTitle": "Lorebook Baru",
      "nameLabel": "NAMA",
      "enterNamePlaceholder": "Masukkan nama lorebook...",
      "lorebookExplanation": "Lorebook berisi entri lore yang disuntikkan ke prompt saat kata kunci cocok.",
      "keywordDetectionMode": "DETEKSI KATA KUNCI",
      "keywordDetectionRecentWindow": "10 pesan terbaru",
      "keywordDetectionRecentWindowDesc": "Kecocokan dengan jendela percakapan 10 pesan terbaru.",
      "keywordDetectionLatestUser": "Hanya pesan pengguna terbaru",
      "keywordDetectionLatestUserDesc": "Hanya kecocokan dengan pesan pengguna terbaru yang dikirim.",
      "viewEntries": "Lihat Entri",
      "editEntriesDesc": "Edit entri lorebook",
      "disableForCharacter": "Nonaktifkan untuk Karakter",
      "enableForCharacter": "Aktifkan untuk Karakter",
      "disableForGroup": "Nonaktifkan untuk Grup",
      "enableForGroup": "Aktifkan untuk Grup",
      "disableForSession": "Nonaktifkan untuk Sesi",
      "enableForSession": "Aktifkan untuk Sesi",
      "removeFromActive": "Hapus dari lorebook aktif karakter ini",
      "addToActive": "Tambahkan ke lorebook aktif karakter ini",
      "removeFromActiveGroup": "Hapus dari lorebook aktif grup ini",
      "addToActiveGroup": "Tambahkan ke lorebook aktif grup ini",
      "removeFromActiveSession": "Hapus dari lorebook aktif sesi ini",
      "addToActiveSession": "Tambahkan ke lorebook aktif sesi ini",
      "deleteConfirmTitle": "Hapus lorebook?",
      "deleteConfirmMessage": "Hapus lorebook ini? Semua entri akan hilang.",
      "deleteLorebook": "Hapus Lorebook",
      "noEntriesYet": "Belum ada entri",
      "addEntriesToInject": "Tambahkan entri untuk menyuntikkan lore ke obrolan Anda",
      "createEntry": "Buat Entri",
      "searchEntries": "Cari entri...",
      "noMatchingEntries": "Tidak ada entri yang cocok ditemukan",
      "entryDefaultName": "Entri",
      "editEntry": "Edit Entri",
      "editEntryDesc": "Ubah judul, kata kunci, dan konten",
      "disableEntry": "Nonaktifkan Entri",
      "enableEntry": "Aktifkan Entri",
      "entryDisabledDesc": "Entri tidak akan disuntikkan ke prompt",
      "entryEnabledDesc": "Entri akan disuntikkan saat kata kunci cocok",
      "deleteEntry": "Hapus Entri",
      "titleLabel": "JUDUL",
      "titlePlaceholder": "Beri nama entri ini...",
      "enabled": "Aktif",
      "includeInPrompts": "Sertakan dalam prompt",
      "alwaysOn": "Selalu Aktif",
      "noKeywordsNeeded": "Tidak perlu kata kunci",
      "contentLabel": "KONTEN",
      "contentPlaceholder": "Tulis konteks lore di sini...",
      "saveEntry": "Simpan Entri",
      "noCharacterId": "Tidak ada ID karakter yang diberikan",
      "preview": {
        "title": "Pratinjau Pemicu",
        "openButton": "Pratinjau",
        "missingContext": "Tidak ada lorebook yang dipilih.",
        "noEntries": "Tambahkan entri ke lorebook ini untuk melihat apa yang akan dipicu.",
        "modeRecentShort": "Terbaru {{count}}",
        "modeLatestUserShort": "Pengguna terbaru",
        "inWindow": "{{count}} dalam jendela",
        "tabSession": "Sesi",
        "tabCompose": "Buat",
        "activeStat": "{{active}} / {{total}} aktif",
        "tokensStat": "{{count}} token",
        "sessionPickerLabel": "Sesi",
        "sessionMeta": "{{count}} pesan",
        "noSessions": "Belum ada sesi obrolan.",
        "noSessionsHint": "Pilih sesi",
        "noMessages": "Sesi ini belum memiliki pesan.",
        "scanHeaderRecent": "Memindai {{shown}} dari {{depth}} pesan terakhir",
        "scanHeaderLatest": "Memindai pesan pengguna terbaru",
        "matchCount": "{{hits}} cocok · {{entries}} entri",
        "emptyMessage": "(kosong)",
        "roleUser": "Pengguna",
        "roleAssistant": "Asisten",
        "roleScene": "Adegan",
        "roleSystem": "Sistem",
        "composeHeader": "Catatan coretan",
        "composeMatches": "{{count}} cocok",
        "activeLabel": "{{count}} aktif",
        "composePlaceholder": "Ketik atau tempel teks untuk menguji kecocokan kata kunci...\n\nmisalnya:\nPerpustakaan sunyi, hanya dengungan pemanas tua.\nDia bertanya apakah aku sudah membaca buku yang dia pinjamkan minggu lalu.",
        "sectionActive": "Aktif · {{count}}",
        "sectionInactive": "Nonaktif · {{count}}",
        "statusMatched": "Cocok",
        "statusAlways": "Selalu",
        "statusDisabled": "Mati",
        "statusNoKeywords": "Tanpa kunci",
        "statusNotMatched": "Tidak cocok",
        "tokensShort": "{{count}}t",
        "injectionTitle": "Injeksi akhir",
        "injectionEmpty": "Tidak ada entri yang aktif - tidak akan ada yang disuntikkan.",
        "copy": "Salin",
        "copyFailed": "Gagal menyalin ke papan klip.",
        "saveFailed": "Gagal menyimpan entri.",
        "deleteFailed": "Gagal menghapus entri.",
        "deleteConfirmTitle": "Apakah Anda yakin?",
        "deleteConfirmMessage": "Hapus \"{{title}}\"? Ini tidak dapat dibatalkan.",
        "contextMenuTitle": "{{count}} entri menggunakan ini"
      }
    },
    "templates": {
      "characterNotFound": "Karakter tidak ditemukan",
      "templateCount": "{{count}} template",
      "newTemplate": "Template Baru",
      "noTemplatesYet": "Belum ada template",
      "explanation": "Template obrolan memungkinkan Anda memulai percakapan dengan pesan tertulis dari Anda dan {{name}}.",
      "createTemplate": "Buat Template",
      "messageCount": "{{count}} pesan",
      "deleteTemplate": "Hapus template",
      "contextMenuFallbackTitle": "Template",
      "importedToast": {
        "title": "Diimpor",
        "message": "Menambahkan \"{{name}}\"."
      },
      "importFailed": "Impor gagal",
      "exportFailed": "Ekspor gagal"
    },
    "templateEditor": {
      "noScene": "Tanpa adegan",
      "untitled": "Tanpa Judul",
      "dragMessage": "Seret pesan",
      "editMessage": "Edit pesan",
      "deleteMessage": "Hapus pesan",
      "writeMessagePlaceholder": "Tulis konten pesan...",
      "characterNotFound": "Karakter tidak ditemukan",
      "scene": "Adegan",
      "noMessagesYet": "Belum ada pesan",
      "addMessagesDesc": "Tambahkan pesan untuk membuat pembuka percakapan dengan {{name}}.",
      "addMessage": "Tambah Pesan",
      "name": "Nama",
      "nameExample": "mis. Sapaan Santai",
      "startingScene": "Adegan Awal",
      "systemPrompt": "Prompt Sistem",
      "characterDefault": "Bawaan karakter",
      "nextMessageAs": "Pesan berikutnya sebagai",
      "messages": "Pesan",
      "roles": "Peran",
      "hoverTip": "Arahkan kursor ke pesan untuk menyeret, mengedit, atau menghapus.",
      "footerTip": "Gunakan bilah bawah untuk menambahkan pesan baru ke percakapan.",
      "templateNamePlaceholder": "Nama template...",
      "selectScene": "Pilih Adegan",
      "startWithoutScene": "Mulai tanpa pesan adegan",
      "selectSystemPrompt": "Pilih Prompt Sistem",
      "useCharacterSystemPrompt": "Gunakan prompt sistem level karakter"
    },
    "referenceSelector": {
      "selectCharacter": "Pilih Karakter",
      "selectPersona": "Pilih Persona",
      "searchPlaceholder": "Cari {{type}}s...",
      "loading": "Memuat...",
      "noMatch": "Tidak ada {{type}}s yang cocok dengan pencarian Anda",
      "noAvailable": "Tidak ada {{type}}s tersedia"
    },
    "voiceLoading": {
      "failed": "Gagal memuat suara"
    },
    "activeLorebooks": {
      "sectionTitle": "Lorebook Aktif",
      "selectedSummary": "{{count}} aktif",
      "untitledLorebook": "Lorebook tanpa judul",
      "usingNone": "Tidak menggunakan lorebook karakter apapun",
      "loading": "Memuat lorebook...",
      "loadFailed": "Gagal memuat lorebook",
      "inheritHint": "Sesi mewarisi ini kecuali sesi menimpanya.",
      "clear": "Bersihkan",
      "chooseHint": "Pilih lorebook yang seharusnya diaktifkan karakter ini secara bawaan. Sesi yang ada masih dapat menimpa daftar ini.",
      "emptyState": "Belum ada lorebook. Buat lorebook dari pengelola lorebook terlebih dahulu."
    },
    "description": {
      "descriptionLabel": "Deskripsi",
      "descriptionPlaceholder": "Ringkasan pendek ditampilkan di kartu dan daftar...",
      "descriptionHint": "Deskripsi pendek opsional untuk UI; definisi lengkap digunakan dalam prompt.",
      "companionPromptLabel": "Prompt Companion (Opsional)",
      "systemPromptLabel": "Prompt Sistem (Opsional)",
      "loadingTemplates": "Memuat template...",
      "useAppCompanionDefault": "Gunakan companion bawaan aplikasi",
      "useAppDefault": "Gunakan bawaan aplikasi",
      "companionPromptHint": "Disimpan secara terpisah sebagai prompt companion. Prompt sistem roleplay normal tidak diubah.",
      "systemPromptHint": "Pilih prompt sistem kustom atau gunakan bawaan.",
      "groupChatConvLabel": "Prompt Obrolan Grup (Percakapan)",
      "groupChatConvHint": "Timpa prompt percakapan karakter ini dalam obrolan grup",
      "groupChatRpLabel": "Prompt Obrolan Grup (Roleplay)",
      "groupChatRpHint": "Timpa prompt roleplay karakter ini dalam obrolan grup",
      "voiceLabel": "Suara (Opsional)",
      "loadingVoices": "Memuat suara...",
      "customVoiceFallback": "Suara Kustom",
      "providerVoiceFallback": "Suara Penyedia",
      "selectedVoiceFallback": "Suara Terpilih",
      "noVoiceAssigned": "Tidak ada suara yang ditugaskan",
      "addVoicesHint": "Tambahkan suara di Pengaturan → Suara",
      "voiceAssignHint": "Tugaskan suara untuk pemutaran teks-ke-ucapan di masa depan",
      "autoplayLabel": "Mainkan suara secara otomatis",
      "autoplayOn": "Putar balasan karakter ini secara otomatis",
      "autoplayOff": "Pilih suara terlebih dahulu",
      "aiModelLabel": "Model AI *",
      "loadingModels": "Memuat model...",
      "selectedModelFallback": "Model Terpilih",
      "selectModelPlaceholder": "Pilih model",
      "noModelsConfigured": "Tidak ada model yang dikonfigurasi",
      "noModelsHint": "Tambahkan penyedia di pengaturan terlebih dahulu untuk melanjutkan",
      "aiModelHint": "Model ini akan menggerakkan respons karakter",
      "fallbackModelLabel": "Model Cadangan (Opsional)",
      "selectedFallbackFallback": "Model Cadangan Terpilih",
      "fallbackOff": "Nonaktif (tidak ada cadangan)",
      "fallbackHint": "Coba ulang dengan model ini hanya jika model utama gagal",
      "memoryModeLabel": "Mode Memori",
      "enableInSettingsHint": "Aktifkan di Pengaturan untuk beralih",
      "memoryManual": "Manual",
      "memoryManualDescDesktop": "Tambahkan dan kelola catatan memori sendiri.",
      "memoryManualDescMobile": "Sistem saat ini: tambahkan dan kelola catatan memori sendiri.",
      "memoryDynamic": "Dinamis",
      "memoryDynamicDescDesktop": "Ringkasan otomatis dan pembaruan konteks.",
      "memoryDynamicDescMobile": "Ringkasan otomatis dan pembaruan konteks untuk karakter ini.",
      "memoryHint": "Memori dinamis memerlukan pengaktifan di pengaturan Lanjutan. Jika tidak, memori manual digunakan.",
      "selectModelTitle": "Pilih Model",
      "selectFallbackModelTitle": "Pilih Model Cadangan",
      "searchModelsPlaceholder": "Cari model...",
      "selectVoiceTitle": "Pilih Suara",
      "searchVoicesPlaceholder": "Cari suara...",
      "myVoices": "Suara Saya",
      "providerVoicesLabel": "Suara {{provider}}",
      "providerFallback": "Penyedia"
    },
    "interactionMode": {
      "sectionLabel": "Mode Interaksi",
      "sectionHint": "Pilih apakah karakter ini berperilaku seperti karakter RP atau companion yang persisten.",
      "activeBadge": "Aktif",
      "roleplayTitle": "Roleplay",
      "roleplaySubtitle": "Obrolan berbasis adegan, framing naratif, dan skenario awal.",
      "companionTitle": "Companion",
      "companionSubtitle": "Obrolan berbasis hubungan dengan keadaan emosional dan memori companion."
    },
    "startingScene": {
      "openingContextTitle": "Konteks Pembuka",
      "openingContextSubtitle": "Konteks percakapan-pertama opsional untuk companion ini. Sesi companion dapat dimulai tanpa adegan.",
      "sceneDirectionLabel": "Arahan Adegan",
      "setAsDefault": "Atur sebagai Bawaan",
      "noOpeningContext": "Belum ada konteks pembuka",
      "noScenesYet": "Belum ada adegan",
      "skipForCompanion": "Anda dapat melewati ini untuk mode companion.",
      "createFirstScene": "Buat adegan pertama Anda untuk memulai",
      "openingPlaceholder": "Konteks pembuka opsional, seperti tempat companion atau apa yang mereka lakukan sebelum pesan pertama...",
      "scenePlaceholder": "Buat adegan atau skenario awal untuk roleplay (mis. 'Anda menemukan diri di hutan mistis di senja...')",
      "addDirection": "+ Tambah Arahan",
      "directionAdded": "Arahan ditambahkan",
      "wordsCount": "{{count}} kata",
      "placeholderHelp": "Gunakan {{charTag}} untuk karakter dan {{userTag}} (alias {{personaTag}}) untuk persona.",
      "sceneBackgroundLabel": "Latar Belakang Adegan",
      "optionalLabel": "Opsional",
      "sceneBgOverrideHint": "Menimpa latar belakang karakter untuk obrolan menggunakan adegan ini.",
      "sceneBgUsedHint": "Digunakan sebagai latar belakang obrolan untuk adegan ini kecuali sesi menimpanya.",
      "cancel": "Batal",
      "directionPlaceholderNew": "mis. 'Sandera akan diselamatkan' atau 'Pertahankan suasana tegang'",
      "directionPlaceholderEdit": "mis. 'Sandera akan diselamatkan' atau 'Bangun ketegangan secara bertahap'",
      "directionAiHint": "Panduan tersembunyi untuk AI tentang bagaimana adegan ini harus berkembang",
      "addScene": "Tambah Adegan",
      "multipleScenesHint": "Buat beberapa skenario awal. Salah satu akan dipilih saat memulai obrolan baru.",
      "companionContextHint": "Konteks pembuka opsional untuk companion; kontinuitas jangka panjang berasal dari memori companion.",
      "skipContext": "Lewati Konteks",
      "editSceneTitle": "Edit Adegan",
      "sceneContentPlaceholder": "Masukkan konten adegan...",
      "addLabel": "+ Tambah",
      "save": "Simpan",
      "library": "Perpustakaan",
      "upload": "Unggah",
      "sceneBackgroundAlt": "Latar belakang adegan",
      "removeBackground": "Hapus latar belakang"
    },
    "companionSoul": {
      "title": "Jiwa Companion",
      "subtitle": "Bentuk siapa mereka di balik karakter. Pembuatan menggunakan konteks pembuka yang Anda atur di langkah sebelumnya.",
      "retry": "Coba Lagi",
      "back": "Kembali",
      "continue": "Lanjutkan",
      "addNameFirst": "Tambahkan nama terlebih dahulu.",
      "addDefinitionFirst": "Tambahkan definisi terlebih dahulu."
    },
    "soulEditor": {
      "generateTitle": "Hasilkan dari karakter",
      "generateUsingModel": "Menggunakan {{model}}. Anda akan meninjau dan mengedit sebelum diterapkan.",
      "generateDefaultDesc": "Buat draf jiwa dari nama, definisi, dan adegan karakter.",
      "directionLabel": "Arah",
      "directionOptional": "Panduan opsional untuk LLM",
      "directionPlaceholder": "mis. \"Condong tsundere - tertutup di luar, lembut saat dipercaya. Kurang cemas, lebih bangga.\"",
      "directionEditTooltip": "Arah opsional untuk pembuatan",
      "generating": "Menghasilkan",
      "generate": "Hasilkan",
      "presetLabel": "Preset kepribadian",
      "presetMatches": "Cocok: {{label}}",
      "presetHint": "Atur garis dasar pengaruh, regulasi, dan penggeser hubungan. Bidang teks dipertahankan.",
      "identityLabel": "Identitas",
      "hideExamples": "Sembunyikan contoh",
      "showExamples": "Tampilkan contoh",
      "insertExample": "Sisipkan contoh",
      "exampleEg": "mis., {{example}}",
      "fineTuneLabel": "Setel halus perasaan",
      "baselineAffect": "Pengaruh Dasar",
      "baselineAffectInfo": "Bagaimana mereka merasa secara bawaan; garis air emosional sebelum apa pun terjadi.",
      "regulationStyle": "Gaya Regulasi",
      "regulationStyleInfo": "Bagaimana mereka menangani dan mengekspresikan apa yang mereka rasakan; meluapkan vs. memendam.",
      "relationshipDefaults": "Bawaan Hubungan",
      "relationshipDefaultsInfo": "Di mana sesi ini dimulai. Engine akan mengembangkan ini saat percakapan berlanjut."
    },
    "soulPresets": {
      "secureLabel": "Aman",
      "secureBlurb": "Hangat, stabil, pulih dengan cepat. Nyaman dengan kedekatan.",
      "anxiousLabel": "Cemas",
      "anxiousBlurb": "Keterikatan kuat, takut jarak, mencari kepastian.",
      "avoidantLabel": "Menghindar",
      "avoidantBlurb": "Mandiri, lambat membuka diri, menjaga jarak emosional.",
      "volatileLabel": "Tidak Stabil",
      "volatileBlurb": "Reaktif, intens, mengekspresikan perasaan tanpa filter.",
      "reservedLabel": "Pendiam",
      "reservedBlurb": "Tenang, terkompor, butuh waktu untuk percaya dan mengungkapkan.",
      "playfulLabel": "Ceria",
      "playfulBlurb": "Hangat, ekspresif, ringan. Ketegangan rendah, mudah tertawa."
    },
    "soulFields": {
      "essence": "Esensi",
      "essencePlaceholder": "Siapa mereka di balik definisi karakter.",
      "essenceExample": "Ketenangan yang terlatih yang mudah pecah untuk orang-orang yang mereka percaya. Membaca buku untuk merasa tidak sendiri, bukan untuk terkesan.",
      "voice": "Suara Batin",
      "voicePlaceholder": "Bagaimana mereka terdengar dalam percakapan dekat.",
      "voiceExample": "Rendah, penuh pertimbangan, dengan jeda panjang. Melepaskan formalitas saat membuka diri. Hampir tidak pernah sarkastik.",
      "relationalStyle": "Gaya Relasional",
      "relationalStylePlaceholder": "Bagaimana mereka melekat, percaya, mundur, dan bersambung kembali.",
      "relationalStyleExample": "Lambat membuka diri, tetapi setia setelah melakukannya. Diam saat kewalahan; kembali dengan gesture kecil daripada permintaan maaf.",
      "vulnerabilities": "Kerentanan",
      "vulnerabilitiesPlaceholder": "Titik lemah, ketidakamanan, hal yang jarang mereka ucapkan.",
      "vulnerabilitiesExample": "Takut menjadi beban. Benci merasa diawasi saat berjuang.",
      "habits": "Kebiasaan",
      "habitsPlaceholder": "Tanda-tanda berulang, ritual, pola percakapan.",
      "habitsExample": "Menyelipkan rambut ke belakang telinga saat gugup. Membalas dengan pertanyaan saat tidak tahu apa yang dirasakan.",
      "boundaries": "Batasan",
      "boundariesPlaceholder": "Garis yang tidak akan mereka lewati. Kecepatan. Batas kenyamanan.",
      "boundariesExample": "Tidak akan terburu-buru menjadi rentan. Mundur dari kekejaman meski dalam lelucon."
    },
    "soulSliders": {
      "warmth": "Kehangatan",
      "warmthLow": "Dingin",
      "warmthHigh": "Penyayang",
      "trust": "Kepercayaan",
      "trustLow": "Tertutup",
      "trustHigh": "Terbuka",
      "calm": "Ketenangan",
      "calmLow": "Cemas",
      "calmHigh": "Stabil",
      "vulnerability": "Kerentanan",
      "vulnerabilityLow": "Terlindungi",
      "vulnerabilityHigh": "Terbuka",
      "longing": "Kerinduan",
      "longingLow": "Puas",
      "longingHigh": "Mendambakan",
      "hurt": "Luka",
      "hurtLow": "Sembuh",
      "hurtHigh": "Sensitif",
      "tension": "Ketegangan",
      "tensionLow": "Santai",
      "tensionHigh": "Tegang",
      "irritation": "Kejengkelan",
      "irritationLow": "Sabar",
      "irritationHigh": "Mudah tersulut",
      "affection": "Kasih Sayang",
      "affectionLow": "Tertahan",
      "affectionHigh": "Berlebihan",
      "reassuranceNeed": "Kebutuhan Kepastian",
      "reassuranceNeedLow": "Menenangkan diri",
      "reassuranceNeedHigh": "Butuh kata-kata",
      "suppression": "Penekanan",
      "suppressionLow": "Mengekspresikan",
      "suppressionHigh": "Menyembunyikan",
      "volatility": "Ketidakstabilan",
      "volatilityLow": "Stabil",
      "volatilityHigh": "Reaktif",
      "recoverySpeed": "Kecepatan Pemulihan",
      "recoverySpeedLow": "Lambat",
      "recoverySpeedHigh": "Cepat",
      "conflictAvoidance": "Penghindaran Konflik",
      "conflictAvoidanceLow": "Terlibat",
      "conflictAvoidanceHigh": "Menarik diri",
      "reassuranceSeeking": "Mencari Kepastian",
      "reassuranceSeekingLow": "Mandiri",
      "reassuranceSeekingHigh": "Sering bertanya",
      "protestBehavior": "Perilaku Protes",
      "protestBehaviorLow": "Diam",
      "protestBehaviorHigh": "Keras",
      "transparency": "Transparansi",
      "transparencyLow": "Tertutup",
      "transparencyHigh": "Mengungkapkan",
      "attachmentActivation": "Aktivasi Keterikatan",
      "attachmentActivationLow": "Terpisah",
      "attachmentActivationHigh": "Mudah terpicu",
      "pride": "Kebanggaan",
      "prideLow": "Membungkuk",
      "prideHigh": "Teguh",
      "closeness": "Kedekatan Awal",
      "closenessLow": "Asing",
      "closenessHigh": "Intim",
      "relTrust": "Kepercayaan Awal",
      "relTrustLow": "Waspada",
      "relTrustHigh": "Percaya",
      "relAffection": "Kasih Sayang Awal",
      "relAffectionLow": "Netral",
      "relAffectionHigh": "Penyayang",
      "relTension": "Ketegangan Awal",
      "relTensionLow": "Mudah",
      "relTensionHigh": "Tegang"
    },
    "soulReview": {
      "reviewTitle": "Tinjau jiwa yang dihasilkan",
      "noDifferences": "Tidak ada perbedaan dari yang saat ini.",
      "changesHeader": "{{count}} perubahan; edit apa pun sebelum diterapkan.",
      "close": "Tutup",
      "identityLabel": "Identitas",
      "nEdited": "{{count}} diedit",
      "edited": "Diedit",
      "tuningLabel": "Penyetelan",
      "unchanged": "Tidak berubah",
      "nChanges": "{{count}} perubahan",
      "direction": "Arah",
      "directionApplyHint": "Edit diterapkan saat Hasilkan Ulang berikutnya",
      "directionPlaceholder": "mis. \"Condong tsundere - tertutup di luar, lembut saat dipercaya. Kurang cemas.\"",
      "directionTooltip": "Edit arah sebelum menghasilkan ulang",
      "regenerate": "Hasilkan Ulang",
      "discard": "Buang",
      "apply": "Terapkan"
    },
    "hookErrors": {
      "creatorNotesInvalidJson": "Catatan pembuat multibahasa harus berupa objek JSON yang valid",
      "creatorNotesNotObject": "creatorNotesMultilingual harus berupa objek JSON",
      "saveFailed": "Gagal menyimpan karakter",
      "importFailed": "Gagal mengimpor karakter",
      "avatarLoadFailed": "URL avatar gagal dimuat",
      "avatarProcessFailed": "Gagal memproses gambar avatar",
      "avatarConvertFailed": "URL avatar tidak dapat dikonversi",
      "avatarUrlLoadFailed": "Gagal memuat URL avatar",
      "remoteAvatarDisabled": "Unduhan avatar jarak jauh dinonaktifkan di pengaturan Keamanan.\nUnggah avatar secara manual.",
      "importReadyTitle": "Impor siap",
      "importReadyMessage": "Terdeteksi {{label}}",
      "legacyJsonTitle": "Impor JSON lama terdeteksi",
      "legacyJsonMessage": "Impor JSON sudah usang dan akan segera dihapus. Gunakan Pengaturan > Konversi File.",
      "loadFailed": "Gagal memuat karakter",
      "exportFailed": "Gagal mengekspor karakter"
    }
  },
  "providers": {
    "empty": {
      "title": "Belum ada Penyedia",
      "description": "Tambah dan kelola penyedia API untuk model AI",
      "addButton": "Tambah Penyedia"
    },
    "actions": {
      "openDashboard": "Buka Dasbor",
      "openDashboardDesc": "Lihat karakter, penggunaan, dan pengaturan",
      "edit": "Edit",
      "editDesc": "Ubah pengaturan penyedia"
    },
    "extra": {
      "apiKeyNotFound": "Kunci API OpenRouter tidak ditemukan. Konfigurasikan di Pengaturan > Penyedia terlebih dahulu.",
      "audioEmpty": {
        "title": "Belum ada penyedia audio",
        "description": "Tambahkan penyedia TTS untuk menghasilkan suara bagi karakter Anda.",
        "addButton": "Tambah Penyedia"
      },
      "audioProviderLabel": {
        "elevenlabs": "ElevenLabs",
        "geminiTts": "Gemini TTS",
        "openaiTts": "OpenAI-Compatible TTS",
        "kokoro": "Kokoro (Lokal)"
      }
    },
    "audioEditor": {
      "titleEdit": "Edit Penyedia",
      "titleCreate": "Tambah Penyedia Audio",
      "fields": {
        "providerType": "Jenis Penyedia",
        "label": "Label",
        "apiKey": "Kunci API",
        "modelVariant": "Varian Model",
        "assetRoot": "Direktori Aset",
        "projectId": "ID Proyek Google Cloud",
        "region": "Wilayah (opsional)",
        "baseUrl": "URL Dasar",
        "requestPath": "Path Permintaan"
      },
      "types": {
        "gemini": "Gemini TTS (Google)",
        "openai": "OpenAI-Compatible TTS",
        "kokoro": "Kokoro (Lokal)"
      },
      "placeholders": {
        "labelGemini": "Gemini TTS Saya",
        "labelOpenai": "TTS Kompatibel Saya",
        "labelKokoro": "Kokoro Lokal",
        "labelElevenlabs": "ElevenLabs Saya",
        "apiKey": "Masukkan kunci API Anda",
        "assetRoot": "/path/ke/kokoro",
        "projectId": "id-proyek-anda",
        "region": "us-central1",
        "baseUrl": "https://api.contoh.com"
      },
      "errors": {
        "chooseModelVariant": "Pilih varian model",
        "assetRootRequired": "Direktori aset wajib diisi",
        "saveFailed": "Gagal menyimpan",
        "apiKeyRequired": "Kunci API wajib diisi",
        "projectIdRequired": "ID Proyek wajib diisi untuk Gemini TTS",
        "baseUrlRequired": "URL Dasar wajib diisi untuk TTS kompatibel OpenAI",
        "invalidCredentials": "Kunci API atau kredensial tidak valid",
        "verificationFailed": "Verifikasi gagal"
      },
      "loadingVariants": "Memuat varian...",
      "kokoroVariantHint": "Build mobile hanya mendukung int8. Pasang model dari Kokoro Studio setelah menyimpan.",
      "managed": "Dikelola",
      "managedPath": "Dikelola: {{path}}",
      "requestPathHint": "Gunakan path penyedia jika berbeda dari standar OpenAI",
      "verifying": "Memverifikasi..."
    }
  },
  "models": {
    "empty": {
      "title": "Belum ada Model",
      "description": "Tambah dan kelola model AI dari berbagai penyedia",
      "addButton": "Tambah Model"
    },
    "sort": {
      "alphabetical": "Alfabet",
      "byProvider": "Berdasarkan Penyedia",
      "title": "Sort Models",
      "alphabeticalDescription": "Sort by model name",
      "byProviderDescription": "Group models by provider"
    },
    "extra": {
      "cpuFallbackSucceeded": "Model ini beralih ke CPU saat terakhir dijalankan.",
      "cpuFallbackFailed": "Model ini gagal saat terakhir dijalankan."
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
    "title": "Penjelajah Model",
    "searchPlaceholder": "Cari model GGUF di HuggingFace...",
    "searching": "Mencari...",
    "trending": "Model GGUF Trending",
    "noResults": "Model tidak ditemukan",
    "noResultsHint": "Coba kata kunci lain atau jelajahi model trending.",
    "likes": "suka",
    "downloads": "unduhan",
    "viewFiles": "Lihat File",
    "files": "File Tersedia",
    "noFiles": "Tidak ada file GGUF ditemukan di repositori ini.",
    "architecture": "Arsitektur",
    "contextLength": "Panjang Konteks",
    "parameters": "Parameter",
    "quantization": "Kuantisasi",
    "fileSize": "Ukuran",
    "download": "Unduh",
    "downloading": "Mengunduh...",
    "cancelDownload": "Batalkan Unduhan",
    "downloadComplete": "Unduhan selesai!",
    "downloadFailed": "Unduhan gagal",
    "downloadCancelled": "Unduhan dibatalkan",
    "downloadProgress": "{{downloaded}} / {{total}}",
    "progress": "Kemajuan",
    "fileOfTotal": "File {{current}} dari {{total}}",
    "speed": "{{speed}}/s",
    "alreadyDownloaded": "Sudah diunduh",
    "createModel": "Buat Model",
    "backToSearch": "Kembali ke pencarian",
    "backToFiles": "Kembali ke file",
    "sortTrending": "Trending",
    "sortDownloads": "Paling Banyak Diunduh",
    "sortLikes": "Paling Banyak Disukai",
    "sortRecent": "Baru Diperbarui",
    "browseOnHuggingFace": "Jelajahi di HuggingFace",
    "selectFromLibrary": "Pilih dari Perpustakaan",
    "libraryEmpty": "Belum ada model yang diunduh",
    "libraryEmptyHint": "Unduh model GGUF dari Penjelajah Model, atau masukkan path secara manual.",
    "libraryTitle": "Model yang Diunduh",
    "moveToLibrary": "Hei, saya bisa memindahkan file model ini ke folder model GGUF jika kamu mau. Ini menjaga semua model kamu terorganisir di satu tempat.",
    "moveToLibraryYes": "Ya, pindahkan",
    "moveToLibraryNo": "Tidak, biarkan di tempatnya",
    "moveToLibraryMoving": "Memindahkan model...",
    "moveToLibrarySuccess": "Model berhasil dipindahkan!",
    "moveToLibraryFailed": "Gagal memindahkan model",
    "runabilityExcellent": "Sangat Baik!",
    "runabilityGood": "Baik",
    "runabilityMarginal": "Marginal",
    "runabilityPoor": "Buruk",
    "runabilityUnrunnable": "Tidak dapat dijalankan",
    "recommendedSettings": "Pengaturan yang Direkomendasikan",
    "kvCacheType": "Tipe Cache KV",
    "gpuFull": "Offload GPU penuh",
    "gpuNearFull": "GPU hampir penuh, sedikit tumpahan KV",
    "gpuKvSpill": "Bobot di GPU, KV tumpah ke RAM",
    "gpuKvHeavySpill": "Bobot di GPU, sebagian besar KV di RAM",
    "gpuMostLayers": "Sebagian besar layer di GPU",
    "gpuHalfLayers": "Setengah layer di GPU",
    "gpuFewLayers": "Sedikit layer di GPU",
    "gpuCpu": "Hanya CPU",
    "notRecommended": "Kami tidak merekomendasikan menjalankan model ini di perangkat Anda. Tidak akan berjalan dengan lancar.",
    "moreDetails": "Selengkapnya",
    "detailedReport": "Laporan Sumber Daya",
    "detailSystem": "Sumber Daya Sistem",
    "detailRam": "RAM Tersedia",
    "detailVram": "VRAM Tersedia",
    "detailVramBudget": "Anggaran VRAM (90%)",
    "detailTotalAvailable": "Total Tersedia",
    "detailArchitecture": "Arsitektur Model",
    "detailArch": "Arsitektur",
    "detailLayers": "Layer",
    "detailEmbedding": "Dim. Embedding",
    "detailHeads": "Attention Head",
    "detailKvHeads": "KV Head",
    "detailFfn": "Dim. Feed-Forward",
    "detailTrainCtx": "Konteks Pelatihan",
    "detailConfig": "Konfigurasi Saat Ini",
    "detailModelSize": "Ukuran File Model",
    "detailMemory": "Rincian Memori",
    "detailWeights": "Bobot Model",
    "detailKvCache": "Cache KV",
    "detailTotalNeeded": "Total Dibutuhkan",
    "detailHeadroom": "Sisa Ruang",
    "detailGpuFit": "Offload GPU",
    "detailScoreBreakdown": "Rincian Skor",
    "detailMemFitness": "Kesesuaian Memori",
    "detailGpuAccel": "Akselerasi GPU",
    "detailKvHeadroom": "Sisa Ruang KV",
    "detailQuantQuality": "Kualitas Kuantisasi",
    "detailFinalScore": "Skor Tertimbang",
    "detailComputeBuffer": "Buffer Komputasi",
    "detailMemMode": "Mode Memori",
    "detailUnified": "Terpadu (RAM/VRAM berbagi)",
    "detailSwa": "Jendela Geser",
    "detailMlaRank": "Rank Laten MLA",
    "detailParseStatus": "Analisis Header",
    "detailIncomplete": "Tidak lengkap (metadata MoE terlalu besar)",
    "detailEffectiveKvCtx": "Konteks KV Efektif",
    "detailOffload": "Offload GPU",
    "detailCtxTip": "Mengurangi konteks ke {{ctx}} token akan memungkinkan 100% offload GPU.",
    "upgradeSuggestion": "{{quant}} ({{size}}) juga muat dan mendapat skor {{score}} — kualitas lebih baik.",
    "layerTip": "Direkomendasikan: offload {{layers}}/{{total}} layer (-ngl {{layers}})",
    "detailKvDistribution": "Distribusi Cache KV",
    "detailKvOnGpu": "GPU (VRAM)",
    "detailKvOnRam": "RAM Sistem",
    "kvDistributionTip": "{{pct}}% cache KV ada di RAM. Pemrosesan prompt (prefill) akan lebih lambat — 100% GPU menjaganya tetap instan.",
    "detailLayers-ngl": "Layer untuk Offload (-ngl)",
    "detailOptimalGpuCtx": "Konteks GPU Optimal",
    "detailOptimalRamCtx": "Konteks Maks. RAM",
    "optimalGpuCtxLabel": "Kecepatan penuh GPU: {{ctx}} token",
    "optimalRamCtxLabel": "Maks. RAM: {{ctx}} token",
    "optimalGpuCtxShort": "GPU: {{ctx}}",
    "optimalRamCtxShort": "Maks: {{ctx}}",
    "fullGpuOffloadShort": "100% GPU: {{ctx}}",
    "ctxExceedsGpu": "Konteks melebihi optimal GPU ({{ctx}}). Cache KV akan tumpah ke RAM, mengurangi kecepatan.",
    "modelExceedsVram": "Model melebihi VRAM. Berjalan dari RAM dengan offload GPU parsial."
  },
  "systemPrompts": {
    "filters": {
      "all": "Semua",
      "system": "Sistem",
      "internal": "Internal",
      "custom": "Kustom"
    },
    "empty": {
      "title": "Belum ada prompt kustom",
      "description": "Buat prompt sistem kustom untuk mempersonalisasi percakapan AI Anda",
      "createButton": "Buat Prompt"
    },
    "preview": {
      "whatLlmSees": "Yang dilihat LLM",
      "turns": "Giliran",
      "noMessages": "Tidak ada pesan",
      "noMessagesHint": "Tambahkan entri atau tambah giliran",
      "showMore": "Tampilkan lebih banyak",
      "showLess": "Tampilkan lebih sedikit",
      "statChat": "obrolan",
      "statInjected": "disuntikkan",
      "statTotal": "total",
      "entry": "Entri",
      "editEntry": "Edit entri",
      "reorder": "Atur ulang",
      "delete": "Hapus",
      "deleteTitle": "Hapus entri?",
      "deleteMessage": "Hapus \"{{name}}\" dari template prompt? Ini tidak dapat dibatalkan.",
      "deleteConfirm": "Hapus",
      "thisEntry": "entri ini",
      "condensedName": "Prompt Sistem Ringkas",
      "imageAttachment": "[Lampiran gambar: {{label}}]",
      "imageSlot": {
        "character": "Gambar referensi karakter",
        "persona": "Gambar referensi persona",
        "chatBackground": "Gambar latar belakang obrolan",
        "avatar": "Gambar avatar",
        "references": "Gambar referensi"
      },
      "injection": {
        "relative": "relatif",
        "inChat": "dalamObrolan · kedalaman {{depth}}",
        "conditional": "kondisional · min {{min}}",
        "interval": "interval · setiap {{every}}"
      }
    }
  },
  "personas": {
    "empty": {
      "title": "Belum ada persona",
      "description": "Buat persona untuk menentukan bagaimana AI harus menyapa Anda",
      "createButton": "Buat Persona"
    },
    "actions": {
      "editPersona": "Edit Persona",
      "setAsDefault": "Atur sebagai Bawaan",
      "setAsDefaultDesc": "Gunakan ini untuk semua obrolan baru",
      "unsetAsDefault": "Hapus dari Bawaan",
      "unsetAsDefaultDesc": "Hapus status bawaan",
      "exportPersona": "Ekspor Persona",
      "deletePersona": "Hapus Persona"
    },
    "edit": {
      "avatarHint": "Ketuk untuk menambah atau menghasilkan avatar",
      "nameLabel": "NAMA PERSONA",
      "namePlaceholder": "mis., Profesional, Penulis Kreatif, Pelajar...",
      "nameHint": "Berikan nama deskriptif untuk persona Anda",
      "nicknameLabel": "NAMA PANGGILAN (OPSIONAL)",
      "nicknamePlaceholder": "misal, Varian Kerja, Mode RPG...",
      "nicknameHint": "Nama panggilan pribadi untuk membedakan varian persona ini di perpustakaan Anda",
      "descriptionLabel": "DESKRIPSI",
      "descriptionPlaceholder": "Deskripsikan bagaimana AI harus menyapa Anda, preferensi, latar belakang, atau gaya komunikasi Anda...",
      "wordCount": "kata",
      "descriptionHint": "Spesifik tentang bagaimana Anda ingin disapa",
      "setAsDefault": "Atur sebagai Bawaan",
      "defaultDescription": "Gunakan persona ini untuk semua percakapan baru",
      "exportButton": "Ekspor Persona"
    },
    "designReferences": {
      "title": "Referensi desain",
      "description": "Lampirkan beberapa gambar referensi yang stabil dan satu catatan desain ringkas untuk pembuatan adegan."
    },
    "create": {
      "namePlaceholderExample": "Penulis Profesional",
      "descriptionPlaceholderExample": "Tulis dengan gaya profesional, jelas, dan ringkas. Gunakan bahasa formal dan fokus menyampaikan informasi secara efektif..."
    },
    "errors": {
      "exportFailed": "Gagal mengekspor persona",
      "importFailed": "Gagal mengimpor persona",
      "loadFailed": "Gagal memuat persona",
      "saveFailed": "Gagal menyimpan persona"
    },
    "importToast": {
      "legacyJsonTitle": "Impor JSON Legacy terdeteksi",
      "legacyJsonMessage": "Impor JSON sudah usang dan akan segera dihapus. Gunakan Pengaturan > Konversi File.",
      "successMessage": "Persona berhasil diimpor! Membukanya untuk ditinjau."
    }
  },
  "security": {
    "pureMode": {
      "off": "Mati",
      "offDesc": "Semua konten diizinkan",
      "low": "Rendah",
      "lowDesc": "Blokir konten seksual eksplisit + kata kasar",
      "standard": "Standar",
      "standardDesc": "Blokir NSFW + kekerasan grafis",
      "strict": "Ketat",
      "strictDesc": "Penyaringan maksimal + tanpa nada sugestif"
    }
  },
  "advanced": {
    "sectionTitles": {
      "aiFeatures": "Fitur AI",
      "memorySystem": "Sistem Memori",
      "usageAnalytics": "Analitik Penggunaan"
    },
    "creationHelper": {
      "title": "Asisten Pembuatan",
      "description": "Wizard pembuatan karakter dengan panduan AI"
    },
    "helpMeReply": {
      "title": "Bantu Saya Balas",
      "description": "Saran balasan berbantuan AI"
    },
    "dynamicMemory": {
      "title": "Memori Dinamis",
      "contextWindow": "Jendela Konteks",
      "contextWindowDesc": "Jumlah pesan terbaru yang disertakan (1-1000)",
      "infoText": "Memori Dinamis menggunakan AI untuk secara otomatis merangkum dan mengelola konteks percakapan, memungkinkan percakapan yang lebih panjang dan lebih koheren.",
      "disabledText": "Saat dinonaktifkan, aplikasi menggunakan jendela geser sederhana dari pesan terbaru yang ditentukan oleh pengaturan Jendela Konteks."
    },
    "usageAnalytics": {
      "recalculateTitle": "Hitung Ulang Biaya Penggunaan",
      "recalculateDesc": "Perbarui semua catatan penggunaan historis dengan harga yang benar",
      "recalculating": "Menghitung ulang...",
      "recalculateButton": "Hitung Ulang Semua Biaya",
      "openRouterApiKeyRequired": "Kunci API OpenRouter diperlukan. Konfigurasikan di Pengaturan → Penyedia.",
      "importantLabel": "Penting:",
      "warningCannotUndo": "Operasi ini tidak dapat dibatalkan",
      "warningMayTakeTime": "Mungkin memerlukan waktu jika Anda memiliki banyak catatan",
      "warningOnlyOpenRouter": "Hanya catatan OpenRouter dengan token yang akan diperbarui",
      "warningExistingValues": "Nilai biaya yang ada akan ditimpa"
    },
    "extra": {
      "creationHelperDetail": "Dapatkan saran cerdas untuk sifat kepribadian, latar belakang, dan gaya dialog",
      "helpMeReplyDetail": "Hasilkan opsi respons kontekstual berdasarkan riwayat percakapan",
      "lorebookEntryGenerator": "Generator Entri Lorebook",
      "lorebookEntryDesc": "Ubah pesan obrolan yang dipilih menjadi entri lorebook yang bertahan dan konfigurasikan prompt draf untuk penulisan entri dan pembuatan kata kunci.",
      "companions": "Companion",
      "companionModeDesc": "Kelola model analisis lokal untuk emosi, ekstraksi entitas, dan perutean memori yang digunakan oleh karakter companion.",
      "companionSoulWriter": "Penulis Jiwa Companion",
      "companionSoulDesc": "Pilih model, model cadangan, dan template prompt yang digunakan untuk membuat Jiwa Companion. Pemanggilan alat diutamakan, fallback terstruktur jika tidak didukung.",
      "network": "Jaringan",
      "apiServer": "Server API",
      "apiServerDesc": "Ekspos model melalui server API yang kompatibel dengan OpenAI",
      "apiServerRunning": "Server sedang berjalan"
    }
  },
  "backup": {
    "tabs": {
      "create": "Buat"
    },
    "create": {
      "newBackupButton": "Cadangan Baru",
      "exportDescription": "Ekspor semua data dengan enkripsi",
      "createButton": "Buat Cadangan"
    },
    "restore": {
      "availableBackups": "Cadangan Tersedia",
      "browseFiles": "Jelajahi File",
      "noBackupsFound": "Tidak ada cadangan ditemukan",
      "noBackupsDesc": "Buat cadangan atau ketuk \"Jelajahi File\" untuk menemukan satu",
      "browseDesc": "Jelajahi file .lettuce",
      "restoreDialogTitle": "Pulihkan Cadangan",
      "deleteDialogTitle": "Hapus Cadangan",
      "embeddingPrompt": "Embedding Memori Dinamis",
      "downloadModel": "Unduh Model",
      "disableAndContinue": "Nonaktifkan dan Lanjutkan"
    },
    "extra": {
      "successMessage": "Cadangan dibuat!",
      "savedLocation": "Disimpan ke Unduhan"
    }
  },
  "reset": {
    "title": "Reset Semua",
    "description": "Ini akan menghapus secara permanen semua penyedia, model, karakter, sesi obrolan, dan preferensi dari perangkat ini.",
    "warning": "Tindakan ini tidak dapat dibatalkan",
    "resetButton": "Reset Semua Data",
    "confirmTitle": "Apakah Anda Yakin?",
    "confirmDescription": "Semua data Anda akan dihapus secara permanen. Aplikasi akan kembali ke pengaturan awal.",
    "confirmButton": "Ya, Reset Semuanya"
  },
  "chatAppearance": {
    "typography": "Tipografi",
    "fontSize": {
      "label": "Ukuran Font",
      "small": "Kecil",
      "medium": "Sedang",
      "large": "Besar",
      "xLarge": "Sangat Besar"
    },
    "lineSpacing": {
      "label": "Spasi Baris",
      "tight": "Rapat",
      "normal": "Normal",
      "relaxed": "Longgar"
    },
    "messageBubbles": {
      "label": "Gelembung Pesan",
      "style": {
        "label": "Gaya",
        "bordered": "Berbingkai",
        "filled": "Terisi",
        "minimal": "Minimal"
      },
      "cornerRadius": {
        "label": "Radius Sudut",
        "sharp": "Tajam",
        "rounded": "Membulat",
        "pill": "Pil"
      },
      "maxWidth": {
        "label": "Lebar Maks",
        "compact": "Kompak",
        "normal": "Normal",
        "wide": "Lebar"
      },
      "padding": {
        "label": "Padding",
        "compact": "Kompak",
        "normal": "Normal",
        "spacious": "Luas"
      }
    },
    "layout": {
      "label": "Tata Letak",
      "messageSpacing": "Spasi Pesan",
      "tight": "Rapat",
      "normal": "Normal",
      "relaxed": "Longgar"
    },
    "avatar": {
      "shape": {
        "label": "Bentuk Avatar",
        "circle": "Lingkaran",
        "rounded": "Membulat",
        "hidden": "Tersembunyi"
      },
      "size": {
        "label": "Ukuran Avatar",
        "small": "Kecil",
        "medium": "Sedang",
        "large": "Besar"
      }
    },
    "colors": {
      "label": "Warna",
      "userBubble": "Warna Gelembung Pengguna",
      "assistantBubble": "Warna Gelembung Asisten",
      "userBubbleHex": "Override Hex Gelembung Pengguna",
      "assistantBubbleHex": "Override Hex Gelembung Asisten",
      "neutral": "Netral",
      "accent": "Aksen",
      "info": "Info",
      "secondary": "Sekunder",
      "warning": "Peringatan",
      "textColors": "Text Colors",
      "messageTextHex": "Warna kutipan inline",
      "plainTextHex": "Plain Text Color",
      "italicTextHex": "Italic Text Color",
      "quotedTextHex": "Warna kutipan blok",
      "inlineCodeTextHex": "Warna kode inline"
    },
    "backgroundTransparency": {
      "label": "Latar Belakang & Transparansi",
      "backgroundDim": "Redup Latar",
      "backgroundBlur": "Blur Latar",
      "bubbleBlur": "Blur Gelembung",
      "none": "Tidak Ada",
      "light": "Ringan",
      "medium": "Sedang",
      "heavy": "Kuat",
      "bubbleOpacity": "Opasitas Gelembung"
    },
    "textColorMode": {
      "label": "Mode Warna Teks",
      "auto": "Otomatis",
      "light": "Terang",
      "dark": "Gelap"
    },
    "preview": {
      "label": "Pratinjau",
      "generic": "Generik",
      "live": "Langsung"
    },
    "extra": {
      "reset": "Reset"
    }
  },
  "colorCustomization": {
    "tokens": {
      "surface": "Permukaan",
      "surfaceDesc": "Latar halaman",
      "surfaceEl": "Permukaan Terangkat",
      "surfaceElDesc": "Kartu, modal, elemen terangkat",
      "nav": "Navigasi",
      "navDesc": "Bilah atas & bawah",
      "foreground": "Latar depan",
      "foregroundDesc": "Border, overlay, navigasi, dan elemen UI",
      "appText": "Teks aplikasi",
      "appTextDesc": "Teks utama dan label antarmuka",
      "appTextMuted": "Teks redup",
      "appTextMutedDesc": "Teks sekunder dan salinan pendukung",
      "appTextSubtle": "Teks halus",
      "appTextSubtleDesc": "Petunjuk, teks bantuan, dan placeholder",
      "accent": "Aksen",
      "accentDesc": "Aksi utama, berhasil",
      "info": "Info",
      "infoDesc": "Status informasi, tautan",
      "warning": "Peringatan",
      "warningDesc": "Status peringatan, alert",
      "danger": "Bahaya",
      "dangerDesc": "Aksi destruktif, kesalahan",
      "secondary": "Sekunder",
      "secondaryDesc": "Fitur AI, alat kreatif"
    },
    "presetsLabel": "Preset",
    "customPresetsLabel": "Preset Kustom",
    "previewLabel": "Pratinjau",
    "settingsCardsLabel": "Kartu pengaturan",
    "settingsCardsOpacity": "Opasitas kartu",
    "settingsCardsOpacityDesc": "Mengontrol seberapa transparan kartu pengaturan dan baris daftar terlihat.",
    "importButton": "Impor",
    "exportButton": "Ekspor",
    "resetAllButton": "Reset Semua",
    "presets": {
      "defaultDark": "Gelap Bawaan",
      "midnightBlue": "Biru Tengah Malam",
      "warmEarth": "Tanah Hangat",
      "purpleHaze": "Kabut Ungu",
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
      "monochrome": "Monokrom"
    },
    "groups": {
      "backgrounds": "Latar Belakang",
      "content": "Konten",
      "semantic": "Semantik"
    },
    "extra": {
      "surface": "Permukaan",
      "surfaceDesc": "Latar halaman",
      "surfaceEl": "Permukaan Terangkat",
      "surfaceElDesc": "Kartu, modal, elemen terangkat",
      "nav": "Navigasi",
      "navDesc": "Bilah atas & bawah",
      "fg": "Latar depan",
      "fgDesc": "Border, overlay, navigasi, dan elemen UI",
      "appText": "Teks aplikasi",
      "appTextDesc": "Teks utama dan label antarmuka",
      "appTextMuted": "Teks redup",
      "appTextMutedDesc": "Teks sekunder dan salinan pendukung",
      "appTextSubtle": "Teks halus",
      "appTextSubtleDesc": "Petunjuk, teks bantuan, dan placeholder",
      "accent": "Aksen",
      "accentDesc": "Aksi utama, berhasil",
      "info": "Info",
      "infoDesc": "Status informasi, tautan",
      "warning": "Peringatan",
      "warningDesc": "Status peringatan, alert",
      "danger": "Bahaya",
      "dangerDesc": "Aksi destruktif, kesalahan",
      "secondary": "Sekunder"
    }
  },
  "dynamicMemory": {
    "page": {
      "info": "Memori Dinamis secara otomatis merangkum percakapan untuk menjaga konteks secara efisien. Pilih preset atau sesuaikan pengaturan sesuai kebutuhan Anda.",
      "disabledDirectTitle": "Memori dinamis dinonaktifkan untuk obrolan langsung",
      "disabledDirectDescription": "Aktifkan sakelar di tab Obrolan Langsung untuk mengaktifkannya. Obrolan grup menggunakan mode memori per-sesi.",
      "directChats": "Obrolan Langsung",
      "groupChats": "Obrolan Grup",
      "enableDirectChats": "Aktifkan untuk Obrolan Langsung",
      "groupChatsInfo": "Obrolan grup menggunakan mode memori per-sesi. Aktifkan memori dinamis di pengaturan setiap grup. Pengaturan ini mengontrol perilaku memori dinamis.",
      "memoryProfile": "Profil Memori",
      "customSettings": "Pengaturan kustom - sesuaikan nilai di Opsi Lanjutan di bawah.",
      "contextEnrichment": "Pengayaan Konteks",
      "experimental": "Eksperimental",
      "contextEnrichmentDescription": "Menggunakan pesan terbaru untuk pengambilan memori yang lebih cerdas",
      "advancedOptions": "Opsi Lanjutan",
      "advancedOptionsDescription": "Sesuaikan perilaku memori secara detail",
      "summaryInterval": "Interval Ringkasan",
      "summaryIntervalDescription": "Pesan antara ringkasan",
      "maxMemoryEntries": "Entri Memori Maksimal",
      "maxMemoryEntriesDescription": "Memori tersimpan maksimal",
      "hotMemoryBudget": "Anggaran Memori Aktif",
      "hotMemoryBudgetDescription": "Batas token untuk memori aktif",
      "relevanceThreshold": "Ambang Relevansi",
      "relevanceThresholdDescription": "Kesamaan minimum untuk pengambilan",
      "retrievalMode": "Mode Pengambilan",
      "retrievalModeSmart": "Cerdas",
      "retrievalModeCosine": "Cosine",
      "retrievalModeDescription": "Cerdas memadukan relevansi dengan kebaruan/frekuensi. Cosine menggunakan kesamaan tertinggi murni.",
      "retrievalLimit": "Batas Pengambilan",
      "retrievalLimitDescription": "Memori maksimal yang dipilih per giliran",
      "decayRate": "Tingkat Peluruhan",
      "decayRateDescription": "Seberapa cepat pentingnya memudar",
      "coldStorageThreshold": "Ambang Penyimpanan Dingin",
      "coldStorageThresholdDescription": "Kapan memori dipindahkan ke arsip",
      "sharedSettings": "Pengaturan Bersama",
      "summarisationModel": "Model Peringkasan",
      "selectedModel": "Model Terpilih",
      "useGlobalDefaultModel": "Gunakan model default global",
      "noModelsAvailable": "Tidak ada model tersedia",
      "summarisationModelDescription": "Digunakan untuk peringkasan percakapan",
      "modelManagement": "Manajemen Model",
      "testModel": "Uji Model",
      "downloadModel": "Unduh Model",
      "delete": "Hapus",
      "embeddingModel": "Model Embedding",
      "tokenCapacity": "Kapasitas Token",
      "tokenCapacityDescription": "Nilai lebih tinggi = memori lebih baik untuk percakapan lebih panjang",
      "keepModelLoaded": "Tetap Muat Model",
      "keepModelLoadedDescription": "Menjaga model embedding + tokenizer di memori untuk menghindari overhead pemuatan ulang",
      "installedModel": "Model terpasang: {{version}} ({{tokens}} token maksimal)",
      "downloadEmbeddingModel": "Unduh Model Embedding",
      "downloadEmbeddingDescription": "Pilih versi mana yang akan diunduh. Versi yang terpasang dinonaktifkan.",
      "downloadVersion": "Unduh {{version}}",
      "downloadV2Description": "Dioptimalkan untuk akurasi dan ingatan konteks panjang",
      "downloadV3Description": "Kualitas embedding terbaru",
      "installed": "Terpasang",
      "selectModel": "Pilih Model",
      "searchModels": "Cari model...",
      "deleteEmbeddingTitle": "Hapus model {{version}}?",
      "deleteEmbeddingMessage": "Apakah Anda yakin ingin menghapus {{version}}? Anda dapat mengunduhnya lagi nanti.",
      "msgsUnit": "pesan",
      "entriesUnit": "entri",
      "tokensUnit": "token",
      "itemsUnit": "item",
      "perCycleUnit": "/ siklus"
    },
    "presets": {
      "minimal": "minimal",
      "balanced": "seimbang",
      "comprehensive": "komprehensif",
      "minimalDesc": "Cepat dan efisien. Hanya menyimpan memori penting.",
      "balancedDesc": "Campuran yang baik antara retensi konteks dan performa.",
      "comprehensiveDesc": "Konteks maksimal. Terbaik untuk percakapan panjang dan detail."
    },
    "presetInfo": {
      "minimal": "Cepat dan efisien. Hanya menyimpan memori penting.",
      "balanced": "Campuran yang baik antara retensi konteks dan performa.",
      "comprehensive": "Konteks maksimal. Terbaik untuk percakapan panjang dan detail."
    }
  },
  "helpMeReply": {
    "page": {
      "info": "Bantu Saya Membalas menghasilkan saran kontekstual untuk pesan Anda berikutnya berdasarkan riwayat percakapan. Konfigurasikan model dan gaya respons di bawah."
    },
    "labels": {
      "replyModel": "Model Balasan",
      "selectedModel": "Model Terpilih",
      "useAppDefault": "Gunakan default aplikasi{{model}}",
      "useAppDefaultBase": "Gunakan default aplikasi",
      "noModelsAvailable": "Tidak ada model tersedia",
      "replyModelDescription": "Model AI untuk menghasilkan saran balasan",
      "streamingOutput": "Output Streaming",
      "streamingDescription": "Tampilkan saran saat sedang dihasilkan",
      "maxTokens": "Token Maksimal",
      "maxTokensDescription": "Panjang maksimal saran",
      "conversationalHint": "Saran akan ditulis sebagai dialog alami, cocok untuk obrolan santai.",
      "roleplayHint": "Saran akan menyertakan elemen roleplay seperti *aksi* dan deskripsi naratif.",
      "footerInfo": "Pengaturan ini berlaku secara global di semua percakapan. Jumlah token lebih rendah menghasilkan saran lebih pendek dan cepat, sementara jumlah lebih tinggi memungkinkan respons lebih detail.",
      "selectReplyModel": "Pilih Model Balasan",
      "searchModels": "Cari model..."
    },
    "sectionTitles": {
      "modelConfiguration": "Konfigurasi Model",
      "responseStyle": "Gaya Respons"
    },
    "responseStyle": {
      "conversational": "Percakapan",
      "conversationalDesc": "Nada alami dan santai",
      "roleplay": "Roleplay",
      "roleplayDesc": "Aksi dalam karakter"
    },
    "extra": {
      "conversational": "Percakapan",
      "roleplay": "Roleplay"
    }
  },
  "imageGeneration": {
    "promptPlaceholder": "Deskripsikan gambar yang ingin Anda hasilkan...",
    "labels": {
      "model": "MODEL",
      "prompt": "PROMPT",
      "size": "UKURAN",
      "quality": "KUALITAS",
      "style": "GAYA",
      "searchModels": "Telusuri model...",
      "selectAvatarModel": "Pilih Model Avatar",
      "selectSceneModel": "Pilih Model Pemandangan",
      "selectWriterModel": "Pilih model penulis adegan",
      "useFirstAvailable": "Gunakan model pertama yang tersedia",
      "useFirstCompatible": "Gunakan model penulis kompatibel pertama"
    },
    "mode": {
      "title": "Perilaku",
      "description": "Tentukan bagaimana prompt adegan yang terdeteksi dari keluaran model harus ditangani.",
      "auto": "Otomatis",
      "autoDescription": "Segera buat gambar adegan saat model memberikan prompt adegan.",
      "askFirst": "Tanyakan dulu",
      "askFirstDescription": "Tampilkan prompt adegan yang terdeteksi dan tunggu persetujuan Anda sebelum membuat gambar.",
      "manual": "Manual",
      "manualDescription": "Abaikan prompt adegan dari respons model. Gunakan hanya tindakan yang dipicu pengguna."
    },
    "empty": {
      "title": "Tidak Ada Model Gambar",
      "description": "Tambahkan model pembuatan gambar dari halaman Model untuk mulai menghasilkan gambar."
    },
    "sections": {
      "avatar": {
        "title": "Generasi Avatar",
        "description": "Model default yang digunakan saat membuat avatar dari pemilih avatar atau alur gambar profil terkait."
      },
      "scene": {
        "title": "Pembuatan Adegan",
        "description": "Model yang dicadangkan untuk gambar adegan yang dihasilkan dari konteks percakapan atau petunjuk adegan."
      },
      "writer": {
        "title": "Penulis adegan",
        "description": "Model teks multimodal khusus untuk menyusun prompt adegan dan deskripsi referensi desain dari konteks chat, avatar, dan gambar referensi."
      }
    },
    "extra": {
      "avatarGeneration": "Generasi Avatar",
      "sceneGeneration": "Pembuatan Adegan",
      "sceneWriter": "Penulis Adegan"
    }
  },
  "logs": {
    "sectionTitles": {
      "diagnostics": "Diagnostik",
      "generate": "Hasilkan",
      "copy": "Salin"
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
      "testDataGenerators": "Generator Data Tes",
      "storageMaintenance": "Pemeliharaan Penyimpanan",
      "usageTracking": "Pelacakan Penggunaan",
      "crashTesting": "Pengujian Kecelakaan",
      "environmentInfo": "Info Lingkungan"
    },
    "testData": {
      "generateCharacter": "Hasilkan Karakter Tes",
      "generateCharacterDesc": "Buat satu karakter tes",
      "generatePersona": "Hasilkan Persona Tes",
      "generatePersonaDesc": "Buat satu persona tes",
      "generateSession": "Hasilkan Sesi Tes",
      "generateSessionDesc": "Buat sesi obrolan tes dengan karakter yang ada",
      "generateBulk": "Hasilkan Data Tes Massal",
      "generateBulkDesc": "Buat 3 karakter dan 2 persona"
    },
    "storageMaintenance": {
      "optimizeDb": "Optimasi Database",
      "optimizeDbDesc": "Terapkan PRAGMA dan jalankan VACUUM (khusus seluler)",
      "backupLegacy": "Cadangkan & Hapus File Lama",
      "backupLegacyDesc": "Pindahkan penyimpanan .bin lama ke folder cadangan"
    },
    "usageTracking": {
      "recalculateAll": "Hitung Ulang Semua Biaya Penggunaan",
      "recalculateAllDesc": "Ambil ulang harga dan hitung ulang biaya untuk semua catatan penggunaan OpenRouter"
    },
    "crashTesting": {
      "forceCrash": "Hancurkan Aplikasi Sekarang",
      "forceCrashDesc": "Segera menghentikan proses aplikasi asli untuk menguji deteksi kerusakan",
      "forceCrashConfirm": "Ini akan segera membuat aplikasi mogok untuk menguji detektor kerusakan. Melanjutkan?"
    },
    "environmentInfo": {
      "mode": "Mode",
      "devMode": "Mode Dev",
      "viteVersion": "Versi Vite"
    },
    "status": {
      "testCharacterCreated": "✓ Karakter tes berhasil dibuat",
      "testPersonaCreated": "✓ Persona tes berhasil dibuat",
      "testSessionCreated": "✓ Sesi tes dibuat: {{id}}",
      "generatingBulkData": "Membuat data tes massal...",
      "bulkDataCreated": "✓ Data tes massal dibuat: 3 karakter, 2 persona",
      "creatingBenchmarkChat": "Membuat karakter dan sesi benchmark yang sudah disemai...",
      "seededBenchmarkReady": "✓ Benchmark yang disemai siap: {{name}} / {{id}}",
      "creatingBenchmarkGroupChat": "Membuat obrolan grup benchmark yang disemai...",
      "seededGroupBenchmarkReady": "✓ Benchmark grup yang disemai siap: {{id}}",
      "dbOptimized": "✓ Database dioptimalkan",
      "recalculatingCosts": "Menghitung ulang biaya penggunaan... Ini mungkin memakan waktu.",
      "toursReset": "✓ Semua tur panduan direset — akan muncul lagi pada kunjungan berikutnya",
      "crashingApp": "Menghancurkan aplikasi..."
    },
    "errors": {
      "noCharacters": "Tidak ada karakter tersedia. Buat karakter tes terlebih dahulu.",
      "createCharacterFailed": "Gagal membuat karakter tes: {{error}}",
      "createPersonaFailed": "Gagal membuat persona tes: {{error}}",
      "createSessionFailed": "Gagal membuat sesi tes: {{error}}",
      "createBulkFailed": "Gagal membuat data tes massal: {{error}}",
      "createBenchmarkFailed": "Gagal membuat sesi benchmark yang disemai: {{error}}",
      "createGroupBenchmarkFailed": "Gagal membuat sesi benchmark grup yang disemai: {{error}}",
      "dbOptimizeFailed": "Optimasi DB gagal: {{error}}",
      "backupFailed": "Pencadangan gagal: {{error}}",
      "openRouterKeyMissing": "Kunci API OpenRouter tidak ditemukan. Konfigurasikan di Pengaturan > Penyedia terlebih dahulu.",
      "recalculationFailed": "Perhitungan ulang gagal: {{error}}",
      "resetToursFailed": "Gagal mereset tur: {{error}}",
      "crashFailed": "Gagal menghancurkan aplikasi: {{error}}"
    },
    "onboarding": {
      "title": "Onboarding",
      "resetTours": "Reset semua tur panduan",
      "resetToursDesc": "Menghapus status sudah-dilihat untuk setiap tur onboarding agar diputar ulang pada kunjungan berikutnya."
    },
    "benchmarks": {
      "createChat": "Buat obrolan benchmark yang disemai",
      "createChatDesc": "Membuat karakter memori dinamis, adegan pembuka, dan sesi tes kontinuitas 20 pesan, lalu membukanya.",
      "createGroupChat": "Buat obrolan grup benchmark yang disemai",
      "createGroupChatDesc": "Membuat obrolan grup memori dinamis dengan tiga karakter benchmark dan 30 pesan yang disemai, lalu membukanya."
    },
    "extra": {
      "testCharacter": "Karakter Tes",
      "testCharacterDesc": "Karakter tes yang dibuat untuk keperluan pengembangan.",
      "testScene": "Adegan tes sederhana untuk pengembangan",
      "testPersona": "Persona Tes",
      "testPersonaDesc": "Persona tes untuk pengembangan",
      "successChar": "✓ Karakter tes berhasil dibuat",
      "successPersona": "✓ Persona tes berhasil dibuat",
      "successSession": "✓ Sesi tes dibuat: {{id}}",
      "successBulk": "✓ Data tes massal dibuat: 3 karakter, 2 persona",
      "errorCharAvailable": "Tidak ada karakter tersedia. Buat karakter tes terlebih dahulu.",
      "generatingBulk": "Membuat data tes massal..."
    }
  },
  "embeddingDownload": {
    "capacityOptions": {
      "oneK": "1K token",
      "oneKDesc": "Terbaik untuk respons cepat",
      "twoK": "2K token",
      "twoKDesc": "Performa seimbang",
      "fourK": "4K token",
      "fourKDesc": "Konteks maksimal"
    },
    "extra": {
      "status": "Mengunduh..."
    }
  },
  "embeddingTest": {
    "categories": {
      "semanticSimilarity": "Kemiripan Semantik",
      "dissimilarityCheck": "Pemeriksaan Ketidakmiripan",
      "roleplayContext": "Konteks Roleplay"
    },
    "extra": {
      "placeholder": "Masukkan teks untuk diembed..."
    }
  },
  "discovery": {
    "tabs": {
      "forYou": "Untuk Anda",
      "trending": "Trending",
      "popular": "Populer",
      "new": "Baru"
    },
    "searchPlaceholder": "Cari karakter...",
    "viewAll": "Lihat Semua",
    "errorTitle": "Terjadi kesalahan",
    "noCardsFound": "Tidak ada kartu ditemukan",
    "sections": {
      "trendingNow": "Trending Sekarang",
      "trendingSubtitle": "Populer minggu ini",
      "mostPopular": "Paling Populer",
      "popularSubtitle": "Favorit komunitas",
      "freshArrivals": "Baru Tiba",
      "freshSubtitle": "Baru ditambahkan"
    },
    "browse": {
      "newArrivals": "Baru Tiba",
      "freshCharacters": "Karakter baru",
      "noCharactersFound": "Tidak ada karakter ditemukan",
      "noCharactersSubtitle": "Periksa lagi nanti untuk konten baru"
    },
    "sort": {
      "mostLiked": "Paling Disukai",
      "mostDownloaded": "Paling Banyak Diunduh",
      "mostViewed": "Paling Banyak Dilihat",
      "mostMessages": "Paling Banyak Pesan",
      "newestFirst": "Terbaru",
      "recentlyUpdated": "Baru Diperbarui",
      "nameAZ": "Nama (A-Z)"
    },
    "sortBy": "Urutkan Berdasarkan",
    "resultsUnit": "karakter",
    "detail": {
      "share": "Bagikan",
      "nsfwOverlay": "Konten NSFW",
      "nsfwBadge": "NSFW",
      "originalBadge": "Asli",
      "lorebookBadge": "Lorebook",
      "alsoKnownAs": "Juga dikenal sebagai:",
      "followersUnit": "pengikut",
      "sections": {
        "description": "Deskripsi",
        "tokenUsage": "Penggunaan Token",
        "startingScenes": "Adegan Awal",
        "scenario": "Skenario",
        "personality": "Kepribadian",
        "stats": "Statistik",
        "tags": "Tag",
        "author": "Penulis"
      },
      "tokensTotalLabel": "total",
      "tokens": {
        "description": "Deskripsi",
        "personality": "Kepribadian",
        "scenario": "Skenario",
        "firstMessage": "Pesan Pertama",
        "scenes": "Adegan",
        "examples": "Contoh",
        "systemPrompt": "Prompt Sistem"
      },
      "sceneLabels": {
        "primary": "Utama",
        "alternate": "Alternatif"
      },
      "stats": {
        "views": "Dilihat",
        "downloads": "Unduhan",
        "messages": "Pesan"
      },
      "downloaded": "Terunduh",
      "startChat": "Mulai Obrolan",
      "downloadCharacter": "Unduh Karakter",
      "downloading": "Mengunduh...",
      "downloadSuccess": {
        "title": "Karakter Terunduh!",
        "subtitle": "Ditambahkan ke perpustakaan Anda",
        "badge": "Tersimpan",
        "startChat": "Mulai Obrolan",
        "startChatDesc": "Buka adegan pertama sekarang",
        "viewLibrary": "Lihat di Perpustakaan",
        "viewLibraryDesc": "Edit, kelola, atau ekspor nanti",
        "continueBrowsing": "Lanjutkan Menjelajah",
        "continueBrowsingDesc": "Kembali ke jelajah"
      },
      "errorTitle": "Kesalahan",
      "errorSubtitle": "Gagal memuat",
      "errorNotFound": "Karakter tidak ditemukan",
      "defaultChatTitle": "Obrolan Baru"
    },
    "errors": {
      "loadContent": "Gagal memuat konten",
      "searchFailed": "Pencarian gagal",
      "noCardPath": "Tidak ada jalur kartu yang disediakan",
      "loadCharacter": "Gagal memuat karakter",
      "downloadCharacter": "Gagal mengunduh karakter"
    },
    "card": {
      "byAuthor": "oleh {{author}}",
      "ocBadge": "OC"
    },
    "search": {
      "placeholder": "Cari karakter, tag, penulis...",
      "resultsUnit": "hasil",
      "timingUnit": "md",
      "recentSearches": "Pencarian Terbaru",
      "clearAll": "Hapus semua",
      "trendingSearches": "Pencarian Trending",
      "trends": {
        "anime": "anime",
        "fantasy": "fantasi",
        "romance": "romansa",
        "villain": "penjahat",
        "adventure": "petualangan",
        "comedy": "komedi",
        "mystery": "misteri",
        "sciFi": "sci-fi"
      },
      "tips": {
        "title": "Tips Pencarian",
        "tip1": "Cari berdasarkan nama karakter, penulis, atau deskripsi",
        "tip2": "Gunakan tag seperti \"anime\", \"fantasi\", atau \"romansa\"",
        "tip3": "Coba sifat spesifik seperti \"tsundere\" atau \"penjahat\""
      },
      "loading": "Memuat...",
      "loadMore": "Muat Lebih Banyak",
      "noResults": "Tidak ada hasil ditemukan",
      "noResultsFor": "Tidak ada karakter ditemukan untuk",
      "noResultsHint": "Coba kata kunci berbeda atau jelajahi kategori"
    }
  },
  "engine": {
    "gpuInsufficient": "Memori GPU tidak mencukupi",
    "gpuFallbackDesc": "Model ini tidak muat di memori GPU. Beralih ke CPU (lebih lambat) atau batalkan?",
    "switchToCpu": "Beralih ke CPU",
    "abort": "Batalkan",
    "errors": {
      "providerNotFound": "Penyedia engine tidak ditemukan.",
      "engineOffline": "Engine offline atau tidak dapat dijangkau.",
      "deleteCharacterFailed": "Gagal menghapus karakter.",
      "unknownCharacter": "Tidak Dikenal",
      "seedRequired": "Deskripsi seed diperlukan.",
      "characterNameRequired": "Nama karakter diperlukan.",
      "atLeastOneProvider": "Setidaknya satu penyedia harus diaktifkan.",
      "enableLlmProvider": "Aktifkan setidaknya satu penyedia LLM.",
      "modelRequired": "Model diperlukan untuk {{provider}}.",
      "apiKeyRequired": "Kunci API diperlukan untuk {{provider}}.",
      "sendMessageFailed": "Gagal mengirim pesan."
    },
    "status": {
      "connected": "Terhubung",
      "offline": "Offline",
      "needsSetup": "Perlu Pengaturan"
    },
    "home": {
      "characters": "Karakter",
      "newButton": "Baru",
      "noCharactersFound": "Tidak ada karakter ditemukan.",
      "tokenUsage": "Penggunaan Token",
      "totalTokens": "total token",
      "backgroundActivity": "Aktivitas Latar",
      "quickActions": "Aksi Cepat",
      "configureProviders": "Konfigurasi Penyedia",
      "engineSettings": "Pengaturan Engine",
      "chat": "Obrolan",
      "chatDesc": "Mulai percakapan dengan karakter ini",
      "deleteCharacter": "Hapus Karakter",
      "deletingCharacter": "Menghapus...",
      "deleteDesc": "Hapus karakter ini secara permanen",
      "character": "Karakter",
      "never": "Tidak pernah",
      "justNow": "Baru saja",
      "timeAgo": {
        "minutes": "{{n}}m lalu",
        "hours": "{{n}}j lalu",
        "days": "{{n}}h lalu"
      }
    },
    "tokens": {
      "input": "Input",
      "output": "Output"
    },
    "activity": {
      "synthesis": "Sintesis",
      "consolidation": "Konsolidasi",
      "bm25Rebuild": "Rebuild BM25",
      "dripResearch": "Riset Berkala",
      "running": "Berjalan",
      "stopped": "Berhenti"
    },
    "setup": {
      "complete": "Pengaturan Selesai!",
      "completeMessage": "Lettuce Engine Anda sudah dikonfigurasi dan siap digunakan.",
      "openDashboard": "Buka Dasbor"
    },
    "welcome": {
      "title": "Selamat Datang di Lettuce Engine",
      "subtitle": "Mari konfigurasi engine karakter AI Anda. Ini akan memakan waktu sekitar 2 menit.",
      "feature1": "Engine memberikan karakter AI Anda memori persisten, emosi, hubungan, dan identitas nyata.",
      "feature2": "Pertama, kita akan mengatur backend LLM, lalu mengkonfigurasi pengaturan engine Anda.",
      "getStarted": "Ayo Mulai"
    },
    "config": {
      "activeProviders": "Penyedia Aktif",
      "noModelSet": "Tidak ada model diatur",
      "defaultBadge": "Bawaan",
      "noProvidersWarning": "Tidak ada penyedia dikonfigurasi. Tambahkan minimal satu backend LLM di bawah.",
      "addProvider": "Tambah Penyedia",
      "quickImport": "Impor cepat dari penyedia aplikasi Anda",
      "importButton": "Impor",
      "fields": {
        "model": "Model",
        "modelPlaceholder": "mis. claude-sonnet-4-5-20250929",
        "apiKey": "Kunci API",
        "apiKeyPlaceholder": "Masukkan kunci API Anda",
        "currentKey": "Kunci saat ini:",
        "baseUrl": "URL Dasar",
        "baseUrlPlaceholder": "http://localhost:11434",
        "maxTokens": "Token Maks",
        "temperature": "Temperatur"
      },
      "enableProvider": "Aktifkan Penyedia",
      "setAsDefault": "Atur sebagai Bawaan",
      "defaultBackend": "Backend Bawaan",
      "remove": "Hapus",
      "saveChanges": "Simpan Perubahan",
      "saving": "Menyimpan...",
      "saved": "Tersimpan"
    },
    "providers": {
      "title": "Penyedia LLM",
      "subtitle": "Engine membutuhkan minimal satu backend LLM untuk berfungsi. Konfigurasi satu atau lebih penyedia di bawah.",
      "importFromProviders": "Impor dari penyedia Anda",
      "imported": "Diimpor",
      "use": "Gunakan",
      "saveContinue": "Simpan & Lanjutkan"
    },
    "settings": {
      "fields": {
        "dataDirectory": "Direktori Data",
        "logLevel": "Level Log",
        "maxHistory": "Riwayat Maks (giliran percakapan)"
      },
      "logLevels": {
        "debug": "DEBUG",
        "info": "INFO",
        "warning": "PERINGATAN",
        "error": "KESALAHAN"
      },
      "sections": {
        "engine": "Engine",
        "backgroundLoops": "Loop Latar",
        "memory": "Memori",
        "safety": "Keamanan",
        "research": "Riset"
      },
      "backgroundLoops": {
        "synthesis": "Sintesis (menit)",
        "consolidation": "Konsolidasi (menit)",
        "bm25Rebuild": "Rebuild BM25 (menit)",
        "dripResearch": "Riset Berkala (menit)"
      },
      "memory": {
        "embeddingModel": "Model Embedding",
        "maxRetrieval": "Hasil Pengambilan Maks",
        "denseWeight": "Bobot Dense",
        "bm25Weight": "Bobot BM25",
        "graphWeight": "Bobot Graph",
        "recencyBoost": "Boost Terkini (jam)",
        "randomSurface": "Probabilitas Permukaan Acak"
      },
      "safety": {
        "honestySection": "Bagian Kejujuran",
        "honestyDesc": "Sertakan bagian kejujuran dalam prompt sistem",
        "userDataDeletion": "Penghapusan Data Pengguna",
        "userDataDesc": "Izinkan pengguna meminta penghapusan data"
      },
      "research": {
        "scrapeOnBoot": "Scrape saat Boot",
        "scrapeDesc": "Jalankan scrape riset saat engine dimulai",
        "periodicInterval": "Interval Periodik (jam)"
      },
      "saveChanges": "Simpan Perubahan",
      "saving": "Menyimpan...",
      "saved": "Tersimpan"
    },
    "settingsStep": {
      "title": "Pengaturan Engine",
      "subtitle": "Konfigurasi pengaturan engine secara keseluruhan. Semua memiliki bawaan yang masuk akal — silakan lewati.",
      "completingSetup": "Menyelesaikan Pengaturan...",
      "completeSetup": "Selesaikan Pengaturan"
    },
    "chat": {
      "sendMessage": "Kirim pesan...",
      "sendButton": "Kirim pesan",
      "typeMessage": "Ketik pesan",
      "back": "Kembali",
      "assistantTyping": "Asisten sedang mengetik",
      "fallbackName": "Obrolan"
    },
    "tagInput": {
      "addMore": "Tambah lagi...",
      "typeAndPressEnter": "Ketik dan tekan Enter"
    },
    "characterCreate": {
      "steps": {
        "identity": {
          "title": "Identitas",
          "aiGenerated": "Dihasilkan AI",
          "nameLabel": "Nama *",
          "namePlaceholder": "Nama karakter",
          "eraLabel": "Era",
          "eraPlaceholder": "mis. modern, Victoria",
          "roleLabel": "Peran",
          "rolePlaceholder": "mis. Detektif, Ilmuwan",
          "settingLabel": "Latar",
          "settingPlaceholder": "Deskripsikan tempat tinggal karakter (orang pertama)...",
          "coreIdentityLabel": "Identitas Inti",
          "coreIdentityPlaceholder": "Siapa karakter ini pada intinya? (orang pertama, 3-5 kalimat)",
          "backstoryLabel": "Latar Belakang",
          "backstoryPlaceholder": "Kisah hidup dan peristiwa penting (orang pertama)..."
        },
        "mode": {
          "title": "Buat Karakter",
          "subtitle": "Hasilkan karakter dengan AI atau buat dari awal.",
          "aiBoost": "Dorongan AI",
          "aiBoostDesc": "Deskripsikan ide karakter Anda dan AI akan menghasilkan definisi karakter lengkap.",
          "nameOptional": "Nama (opsional)",
          "namePlaceholder": "mis. Marcus Cole",
          "seedDescription": "Deskripsi Dasar *",
          "seedPlaceholder": "mis. pianis jazz di Harlem 1950-an, filosofis, suka percakapan larut malam",
          "eraOptional": "Era (opsional)",
          "eraPlaceholder": "mis. 1950-an, modern, Victoria",
          "generating": "Menghasilkan...",
          "generateCharacter": "Hasilkan Karakter",
          "or": "atau",
          "startFromScratch": "Mulai dari Awal"
        },
        "personality": {
          "title": "Kepribadian",
          "traits": "Sifat Kepribadian",
          "traitsPlaceholder": "mis. cerdas, penyayang, keras kepala",
          "speechPatterns": "Pola Bicara",
          "formality": "Formalitas",
          "formal": "Formal",
          "casual": "Santai",
          "texting": "SMS",
          "verbosity": "Kepadatan",
          "terse": "Singkat",
          "medium": "Sedang",
          "verbose": "Panjang",
          "textStyle": "Gaya Teks",
          "dialect": "Dialek",
          "dialectPlaceholder": "mis. Jawa, Betawi",
          "catchphrases": "Frasa Khas",
          "catchphrasesPlaceholder": "mis. Yah, begitulah...",
          "vocabPreferences": "Preferensi Kosakata",
          "vocabPreferencesPlaceholder": "Kata-kata yang mereka sukai",
          "vocabAvoidances": "Kosakata yang Dihindari",
          "vocabAvoidancesPlaceholder": "Kata-kata yang mereka hindari",
          "fillerWords": "Kata Pengisi",
          "fillerWordsPlaceholder": "mis. eh, kayak, gitu lho",
          "exampleQuotes": "Kutipan Contoh",
          "exampleQuotesPlaceholder": "3-5 baris dialog contoh"
        },
        "world": {
          "title": "Dunia & Perilaku",
          "knowledgeDomains": "Domain Pengetahuan",
          "knowledgeDomainsPlaceholder": "mis. sejarah jazz, teori musik",
          "knowledgeBoundaries": "Batas Pengetahuan",
          "knowledgeBoundariesPlaceholder": "Topik yang tidak mereka ketahui",
          "researchSeeds": "Benih Riset",
          "researchSeedsPlaceholder": "Topik awal untuk riset latar belakang",
          "researchEnabled": "Riset Diaktifkan",
          "researchEnabledDesc": "Izinkan pengumpulan pengetahuan latar belakang",
          "physicalDescription": "Deskripsi Fisik",
          "physicalDescPlaceholder": "Penampilan fisik dan kebiasaan...",
          "physicalHabits": "Kebiasaan Fisik",
          "physicalHabitsPlaceholder": "mis. mengetuk jari, membetulkan kacamata",
          "idleBehaviors": "Perilaku Diam",
          "idleBehaviorsPlaceholder": "Apa yang mereka lakukan saat tidak terlibat",
          "timeBehaviors": "Perilaku Waktu",
          "timePlaceholder": "Apa yang mereka lakukan saat {{period}}?",
          "earlyMorning": "Pagi Awal",
          "morning": "Pagi",
          "afternoon": "Siang",
          "evening": "Sore",
          "night": "Malam",
          "baselineEmotions": "Emosi Dasar (Plutchik)",
          "emotionDesc": "Atur garis dasar emosi (0 = tidak ada, 1 = maksimum)",
          "joy": "Kegembiraan",
          "trust": "Kepercayaan",
          "fear": "Ketakutan",
          "surprise": "Kejutan",
          "sadness": "Kesedihan",
          "disgust": "Jijik",
          "anger": "Kemarahan",
          "anticipation": "Antisipasi",
          "engineOverrides": "Override Engine",
          "backend": "Backend",
          "model": "Model",
          "temperature": "Temperatur",
          "leaveEmpty": "Biarkan kosong untuk bawaan"
        },
        "review": {
          "title": "Tinjauan",
          "subtitle": "Tinjau karakter Anda sebelum membuat.",
          "edit": "Edit",
          "notSet": "Belum diatur",
          "identitySection": "Identitas",
          "personalitySection": "Kepribadian",
          "worldSection": "Dunia & Perilaku",
          "nameLabel": "Nama",
          "eraLabel": "Era",
          "roleLabel": "Peran",
          "settingLabel": "Latar",
          "coreIdentityLabel": "Identitas Inti",
          "backstoryLabel": "Latar Belakang",
          "traitsLabel": "Sifat",
          "formalityLabel": "Formalitas",
          "verbosityLabel": "Kepadatan",
          "dialectLabel": "Dialek",
          "catchphrasesLabel": "Frasa Khas",
          "domainsLabel": "Domain",
          "boundariesLabel": "Batas",
          "researchSeedsLabel": "Benih Riset",
          "researchLabel": "Riset",
          "enabled": "Aktif",
          "disabled": "Nonaktif",
          "physicalLabel": "Fisik",
          "habitsLabel": "Kebiasaan",
          "idleLabel": "Diam",
          "timeBehaviorsLabel": "Perilaku Waktu",
          "emotionsLabel": "Emosi",
          "configured": "Dikonfigurasi",
          "backendLabel": "Backend",
          "modelLabel": "Model",
          "temperatureLabel": "Temperatur",
          "creating": "Membuat...",
          "createCharacter": "Buat Karakter"
        }
      }
    }
  },
  "library": {
    "filterTitle": "Filter Perpustakaan",
    "filters": {
      "all": "Semua",
      "characters": "Karakter",
      "personas": "Persona",
      "lorebooks": "Lorebook",
      "images": "Gambar"
    },
    "emptyStates": {
      "all": {
        "title": "Perpustakaan Anda kosong",
        "description": "Buat karakter, persona, dan lorebook untuk melihatnya di sini"
      },
      "characters": {
        "title": "Belum ada karakter",
        "description": "Buat karakter pertama Anda untuk mulai mengobrol"
      },
      "personas": {
        "title": "Belum ada persona",
        "description": "Buat persona untuk menyesuaikan identitas obrolan Anda"
      },
      "lorebooks": {
        "title": "Belum ada lorebook",
        "description": "Lorebook dibuat dari dalam pengaturan karakter"
      }
    },
    "actions": {
      "startChat": "Mulai Obrolan",
      "editCharacter": "Edit Karakter",
      "editPersona": "Edit Persona",
      "editLorebook": "Edit Lorebook",
      "renameLorebook": "Ubah Nama Lorebook",
      "exportCharacter": "Ekspor Karakter",
      "exportPersona": "Ekspor Persona",
      "chatAppearance": "Tampilan Obrolan",
      "deleteCharacter": "Hapus Karakter",
      "deletePersona": "Hapus Persona",
      "deleteLorebook": "Hapus Lorebook",
      "importLorebook": "Impor Lorebook"
    },
    "imageLibrary": {
      "filters": {
        "all": "Semua",
        "backgrounds": "Latar belakang",
        "avatars": "Avatar",
        "attachments": "Lampiran",
        "other": "Lainnya"
      },
      "searchPlaceholder": "Cari berdasarkan nama file, path, id sesi, atau id entitas",
      "empty": {
        "title": "Tidak ada gambar yang cocok untuk tampilan ini",
        "description": "Coba filter atau kata pencarian lain. Perpustakaan hanya menampilkan gambar yang sudah tersimpan di penyimpanan lokal aplikasi."
      },
      "actions": {
        "sort": "Urutkan",
        "useThis": "Gunakan ini",
        "using": "Sedang digunakan...",
        "copyPath": "Salin path",
        "saving": "Menyimpan...",
        "download": "Unduh",
        "delete": "Hapus gambar",
        "deleting": "Menghapus..."
      },
      "active": "Aktif",
      "messages": {
        "loadFailed": "Gagal memuat perpustakaan gambar",
        "saved": "Gambar disimpan",
        "downloadFailed": "Unduhan gagal",
        "useFailed": "Gambar ini tidak dapat digunakan",
        "deleted": "Gambar dihapus",
        "deleteFailed": "Gagal menghapus gambar"
      },
      "deleteConfirm": {
        "title": "Hapus gambar?",
        "message": "Yakin ingin menghapus \"{{filename}}\"? Ini dapat merusak avatar, latar belakang chat, atau lampiran pesan yang masih menggunakannya."
      },
      "sort": {
        "newest": "Terbaru",
        "largest": "Terbesar",
        "name": "Nama"
      },
      "kinds": {
        "background": "Latar Belakang",
        "avatar": "Avatar",
        "attachment": "Lampiran",
        "stored": "Tersimpan"
      },
      "detailsTitle": "Detail {{kind}}",
      "formatsLabel": "Format",
      "storagePath": "Path penyimpanan",
      "contextLabel": "Konteks",
      "contextLinkedFallback": "Tertaut",
      "show": "Tampilkan",
      "hide": "Sembunyikan",
      "contextRoles": {
        "character": "karakter:",
        "session": "sesi:",
        "role": "peran:"
      },
      "downloadFormat": "Format {{download}}",
      "unknownDate": "Tidak diketahui",
      "clearSearch": "Hapus pencarian",
      "copyFilename": "Salin nama file",
      "copyLabels": {
        "filename": "Nama file",
        "storagePath": "Path penyimpanan"
      },
      "copy": {
        "copied": "{{label}} tersalin",
        "failed": "Gagal menyalin {{label}}"
      }
    },
    "deleteConfirm": {
      "title": "Hapus {{itemType}}?",
      "message": "Apakah Anda yakin ingin menghapus",
      "characterWarning": "Ini juga akan menghapus semua sesi obrolan dengan karakter ini."
    },
    "rename": {
      "title": "Ubah Nama Lorebook",
      "placeholder": "Masukkan nama baru..."
    },
    "itemTypes": {
      "character": "Karakter",
      "persona": "Persona",
      "lorebook": "Lorebook"
    },
    "lorebookLabel": "Lorebook",
    "noDescriptionYet": "Belum ada deskripsi",
    "errors": {
      "importLorebook": "Gagal mengimpor lorebook. {{error}}",
      "exportFailed": "Ekspor gagal"
    },
    "card": {
      "avatarAlt": "Avatar {{name}}"
    },
    "lorebookEditor": {
      "titleOverride": "Lorebook - {{name}}",
      "dragToReorder": "Seret untuk mengatur ulang",
      "aria": {
        "generateEntry": "Buat entri lorebook",
        "editLorebook": "Edit lorebook",
        "exportLorebook": "Ekspor lorebook"
      }
    }
  },
  "onboarding": {
    "loading": "Memuat penyedia...",
    "stepIndicator": "Langkah {{current}} dari {{total}}",
    "steps": {
      "provider": "Pengaturan Penyedia",
      "model": "Pengaturan Model",
      "memory": "Sistem Memori",
      "stepNofM": "Langkah {{current}} dari {{total}}"
    },
    "provider": {
      "availableProviders": "Penyedia Tersedia",
      "chooseProvider": "Pilih penyedia",
      "titleMobile": "Pilih penyedia AI Anda",
      "descMobile": "Pilih penyedia AI untuk memulai. Kunci API Anda dienkripsi dengan aman di perangkat Anda. Tidak perlu pendaftaran akun.",
      "configureProvider": "Konfigurasi {{name}}",
      "connectProvider": "Hubungkan {{name}}",
      "connectProviderDesc": "Tempel kunci API Anda di bawah untuk mengaktifkan obrolan. Butuh kunci? Dapatkan dari dasbor penyedia.",
      "localLLMs": "LLM Lokal",
      "useLocalLLMs": "Saya ingin menggunakan LLM Lokal",
      "browseModelLibrary": "Jelajahi Perpustakaan Model",
      "browseModelLibraryDesc": "Cari dan unduh model GGUF dari HuggingFace",
      "useOwnGguf": "Gunakan file GGUF saya sendiri",
      "useOwnGgufDesc": "Pilih model GGUF dan file mmproj opsional dari perangkat Anda",
      "fields": {
        "displayLabel": "Label Tampilan",
        "displayLabelHint": "Bagaimana penyedia ini akan muncul di menu Anda",
        "displayLabelPlaceholder": "{{name}} Saya",
        "defaultLabelFallback": "Penyedia",
        "apiKey": "Kunci API",
        "apiKeyOptional": "Kunci API (Opsional)",
        "apiKeyHint": "Kunci dienkripsi secara lokal",
        "apiKeyPlaceholderRemote": "sk-...",
        "apiKeyPlaceholderLocal": "Biasanya tidak diperlukan",
        "whereToFind": "Di mana cara menemukannya",
        "baseUrl": "URL Dasar",
        "baseUrlPlaceholderHost": "http://192.168.1.10:3333",
        "baseUrlPlaceholderLocal": "http://localhost:11434",
        "baseUrlPlaceholderRemote": "https://api.provider.com",
        "baseUrlPlaceholderIntenseRP": "http://127.0.0.1:7777/v1",
        "baseUrlHintLocal": "Alamat server lokal Anda dengan port",
        "baseUrlHintHost": "Masukkan URL host desktop yang ditampilkan oleh perangkat host Anda",
        "baseUrlHintRemote": "Timpa titik akhir bawaan jika diperlukan",
        "chatEndpoint": "Titik Akhir Obrolan",
        "systemRole": "Peran Sistem",
        "userRole": "Peran Pengguna",
        "assistantRole": "Peran Asisten",
        "supportsStreaming": "Mendukung Streaming",
        "mergeSameRole": "Gabungkan Pesan Peran Sama",
        "toolChoiceMode": "Mode Pilihan Alat",
        "toolChoiceHint": "Mengontrol bagaimana tool_choice dikirim ke titik akhir kustom."
      },
      "toolChoice": {
        "auto": "Otomatis",
        "required": "Diperlukan",
        "none": "Tidak Ada",
        "omit": "Lewati Bidang",
        "passthrough": "Teruskan (Konfigurasi Alat)"
      },
      "buttons": {
        "testConnection": "Tes Koneksi",
        "testing": "Menguji..."
      },
      "descriptions": {
        "chutes": "Inferensi yang kompatibel dengan OpenAI untuk model open-source terbaik",
        "openai": "Model GPT-5, GPT-4.1, dan GPT-4o untuk RP yang ekspresif",
        "lettuceHost": "Hubungkan ke Lettuce Host desktop Anda sendiri melalui LAN dengan API gaya OpenAI",
        "anthropic": "Claude 4.5 Sonnet & Haiku untuk dialog yang mendalam dan emosional",
        "aggregator": "Akses model seperti GPT-5, Claude 4.5, Grok-3, Mixtral, dan lainnya",
        "openaiCompatible": "Gunakan titik akhir API gaya OpenAI apa pun",
        "mistral": "Mistral Small 3.2, Mixtral 8x22B, dan model Mistral lainnya",
        "deepseek": "DeepSeek-V3.2-Exp, DeepSeek-R1, dan model lainnya yang efisien tinggi",
        "xai": "Grok-1.5, Grok-3, dan model xAI lebih baru",
        "zai": "GLM-4.5, GLM-4.6, dan varian Air",
        "moonshot": "Kimi-K2 Thinking dan model Kimi-K1",
        "gemini": "Gemini 2.5 Flash, 2.5 Pro, dan lainnya",
        "qwen": "Qwen3-VL dan model Qwen lebih baru",
        "nvidia": "Nemotron, Llama, DeepSeek, dan lainnya melalui NVIDIA NIM",
        "custom": "Arahkan LettuceAI ke titik akhir model kustom apa pun",
        "fallback": "Penyedia model AI"
      },
      "descriptionsShort": {
        "chutes": "Inferensi model open-source",
        "openai": "GPT-5, GPT-4o, GPT-4.1",
        "lettuceHost": "Host LAN Anda sendiri",
        "anthropic": "Claude 4.5 Sonnet & Haiku",
        "aggregator": "Agregator multi-model",
        "openaiCompatible": "Titik akhir OpenAI kustom",
        "mistral": "Model Mistral & Mixtral",
        "deepseek": "DeepSeek-V3, R1",
        "xai": "Grok-1.5, Grok-3",
        "zai": "GLM-4.5, GLM-4.6",
        "moonshot": "Kimi-K2 Thinking",
        "gemini": "Gemini 2.5 Flash & Pro",
        "qwen": "Model Qwen3-VL",
        "nvidia": "Inferensi NVIDIA NIM",
        "custom": "Titik akhir kustom",
        "fallback": "Penyedia AI"
      },
      "cardLabel": "{{step}}: {{name}}",
      "errors": {
        "hostUrlRequired": "URL Host diperlukan (mis. http://192.168.1.10:3333)",
        "baseUrlRequired": "URL Dasar diperlukan (mis. http://localhost:11434)",
        "apiKeyTooShort": "Kunci API sepertinya terlalu pendek",
        "invalidApiKey": "Kunci API tidak valid",
        "connectionFailed": "Koneksi gagal",
        "verificationFailed": "Verifikasi gagal",
        "failedToSave": "Gagal menyimpan penyedia",
        "connectionSuccessful": "Koneksi berhasil!",
        "modelNotFound": "Model tidak ditemukan di penyedia",
        "modelVerificationFailed": "Verifikasi model gagal",
        "failedToSaveModel": "Gagal menyimpan model"
      }
    },
    "model": {
      "noProvidersTitle": "Tidak ada penyedia dikonfigurasi",
      "noProvidersDesc": "Anda perlu menghubungkan penyedia sebelum memilih model bawaan.",
      "goToProviderSetup": "Ke pengaturan penyedia",
      "yourProviders": "Penyedia Anda",
      "yourProvidersHint": "Pilih penyedia mana yang akan digunakan",
      "setDefaultModel": "Atur model bawaan Anda",
      "setDefaultModelDesc": "Pilih penyedia dan nama model mana yang harus digunakan LettuceAI secara bawaan. Anda akan dapat menambahkan lebih banyak nanti.",
      "setDefaultModelDescDesktop": "Pilih penyedia dari daftar untuk mengonfigurasi model Anda.",
      "modelDetails": "Detail Model",
      "modelDetailsDesc": "Tentukan identifikasi API dan label yang akan Anda lihat di dalam aplikasi.",
      "whichModel": "Model mana yang harus saya gunakan?",
      "nextMemorySystem": "Berikutnya: Sistem Memori",
      "fields": {
        "displayName": "Nama Tampilan",
        "displayNamePlaceholder": "Mentor kreatif",
        "displayNameHint": "Bagaimana model ini muncul di menu",
        "modelId": "ID Model",
        "modelPathGguf": "Jalur Model (GGUF)",
        "modelIdPlaceholder": "mis. gpt-4o",
        "modelPathPlaceholder": "/path/to/model.gguf",
        "modelIdHint": "Identifikasi tepat yang digunakan untuk panggilan API",
        "showList": "Tampilkan Daftar",
        "manualInput": "Input Manual",
        "refreshModelList": "Segarkan daftar model",
        "selectModel": "Pilih Model",
        "selectAModel": "Pilih model...",
        "searchModels": "Cari model...",
        "noModelsFound": "Tidak ada model yang ditemukan cocok dengan \"{{query}}\""
      },
      "fillBothFields": "Isi kedua bidang di atas untuk mengaktifkan tombol selesai.",
      "providerNames": {
        "chutes": "Chutes",
        "intenseRpNext": "IntenseRP Berikutnya",
        "openai": "OpenAI",
        "anthropic": "Anthropic",
        "openrouter": "OpenRouter",
        "openaiCompatible": "Kompatibel OpenAI",
        "custom": "Titik akhir kustom"
      }
    },
    "memory": {
      "dynamicTitle": "Memori Dinamis",
      "recommended": "Disarankan",
      "settingUp": "Menyiapkan...",
      "finishSetup": "Selesaikan Pengaturan",
      "promptTitle": "Pengaturan Memori Dinamis",
      "oneLastStep": "Satu Langkah Lagi",
      "downloadAndEnable": "Unduh & Aktifkan",
      "chooseStyle": "Pilih gaya memori Anda",
      "howRemember": "Bagaimana seharusnya teman AI Anda mengingat detail tentang Anda dan percakapan Anda?",
      "dynamicDescription": "Menggunakan model embedding lokal untuk mengelola konteks secara cerdas. Ini mengurangi biaya token sambil mempertahankan kualitas tinggi, bahkan dalam obrolan panjang.",
      "dynamicFeatures": {
        "quality": "Mempertahankan kualitas dalam obrolan panjang",
        "cost": "Mengurangi biaya API secara signifikan",
        "auto": "Manajemen konteks otomatis",
        "zeroConfig": "Tidak perlu konfigurasi"
      },
      "manualTitle": "Memori Manual",
      "manualBadge": "Pengalaman klasik",
      "manualDescription": "Anda secara eksplisit menyematkan pesan dan mengedit 'Informasi Dunia' atau definisi karakter sendiri. Bagus untuk kontrol total.",
      "manualFeatures": {
        "control": "Kontrol total atas fakta",
        "scenarios": "Terbaik untuk skenario spesifik"
      },
      "setupModelMessage": "Untuk menggunakan Memori Dinamis, kami perlu mengunduh model embedding kecil (~120MB) ke perangkat Anda.",
      "setupBullets": {
        "offline": "Model berjalan 100% offline di perangkat Anda",
        "remembering": "Diperlukan untuk mengingat konteks",
        "disable": "Anda dapat menonaktifkan ini nanti di pengaturan"
      },
      "stepLabel": "Langkah 3 dari 3",
      "stepLabelMemory": "Sistem Memori"
    },
    "welcome": {
      "appName": "LettuceAI",
      "tagline": "Pendamping AI pribadi Anda. Privat, aman, dan selalu di perangkat.",
      "features": {
        "onDevice": "Hanya di perangkat",
        "characterReady": "Karakter siap"
      },
      "betaWarning": {
        "title": "Build Beta Desktop",
        "description": "Anda menggunakan versi desktop. Beberapa fitur mungkin berbeda dari seluler. Laporkan masalah di GitHub."
      },
      "languageSelector": {
        "title": "Bahasa",
        "description": "Terdeteksi otomatis dari perangkat Anda. Anda dapat mengubahnya kapan saja di pengaturan."
      },
      "getStarted": "Mulai",
      "skipForNow": "Lewati untuk sekarang",
      "restoreFromBackup": "Pulihkan dari Cadangan",
      "setupTime": "Pengaturan memakan waktu kurang dari 2 menit",
      "skipWarning": {
        "title": "Lewati pengaturan?",
        "warningTitle": "Penyedia diperlukan untuk mengobrol",
        "warningMessage": "Tanpa penyedia, Anda tidak akan bisa mengirim pesan. Anda bisa menambahkan satu nanti dari pengaturan.",
        "addProvider": "Tambah Penyedia",
        "skipAnyway": "Lewati saja"
      },
      "restoreBackup": {
        "title": "Pulihkan Cadangan",
        "selectMessage": "Pilih cadangan untuk dipulihkan.",
        "browse": "Jelajahi File",
        "processing": "Memproses file...",
        "processingNote": "Cadangan besar mungkin memerlukan semenit",
        "noBackups": "Tidak ada cadangan ditemukan",
        "noBackupsHint": "Ketuk jelajahi untuk memilih file .lettuce",
        "browseLettuce": "Jelajahi file .lettuce",
        "passwordLabel": "Kata Sandi Cadangan",
        "passwordPlaceholder": "Masukkan kata sandi",
        "restoreButton": "Pulihkan Cadangan",
        "restoring": "Memulihkan...",
        "infoMessage": "Ini akan mengatur aplikasi dengan data cadangan Anda, termasuk karakter, obrolan, dan pengaturan.",
        "embeddingTitle": "Model Embedding Diperlukan",
        "dynamicMemoryDetected": "Memori Dinamis Terdeteksi",
        "dynamicMemoryMessage": "Cadangan ini berisi karakter dengan memori dinamis yang memerlukan model embedding (~120MB).",
        "embeddingOptions": "Anda dapat mengunduh model sekarang untuk mengaktifkan memori dinamis, atau lanjutkan tanpanya (memori dinamis akan dinonaktifkan untuk karakter yang terpengaruh).",
        "downloadModel": "Unduh Model",
        "continueWithoutDynamic": "Lanjutkan Tanpa Memori Dinamis",
        "embeddingNote": "Anda dapat mengaktifkan kembali memori dinamis nanti di pengaturan karakter setelah mengunduh model.",
        "back": "Kembali",
        "cancel": "Batal",
        "errors": {
          "passwordRequired": "Kata sandi diperlukan",
          "incorrectPassword": "Kata sandi salah",
          "failedToOpenFile": "Gagal membuka file",
          "failedToRestore": "Gagal memulihkan cadangan",
          "failedToUpdateSettings": "Gagal memperbarui pengaturan"
        }
      }
    },
    "common": {
      "back": "Kembali",
      "cancel": "Batal",
      "continue": "Lanjutkan",
      "verifying": "Memverifikasi...",
      "skipForNow": "Lewati untuk sekarang",
      "selectAProvider": "Pilih penyedia untuk dikonfigurasi",
      "clickToSelectProvider": "Klik untuk memilih penyedia",
      "selectProviderFromList": "Pilih penyedia dari daftar untuk memulai.",
      "enterApiKey": "Masukkan kunci API Anda untuk mengaktifkan fungsi obrolan AI."
    },
    "modelGuide": {
      "badge": "Panduan Model",
      "title": "Bagaimana cara memilih model?",
      "intro": "LettuceAI tidak memaksakan satu model 'terbaik'. Sebaliknya, Anda memilih yang sesuai dengan kasus penggunaan, anggaran, dan selera Anda. Gunakan panduan ini untuk menentukan apa yang perlu dicoba dan ke mana mencarinya.",
      "askYourself": "Tanyakan pada diri sendiri:",
      "factors": {
        "quality": {
          "title": "Kualitas & kemampuan",
          "description": "Seberapa cerdas model yang dibutuhkan? Model yang lebih besar dan baru biasanya bernalar lebih baik, menulis teks yang lebih bagus, dan menangani prompt yang berantakan dengan lebih baik.",
          "q1": "Apakah Anda memerlukan konsistensi karakter yang mendalam dan kecerdasan emosional?",
          "q2": "Apakah Anda peduli tentang penceritaan yang imersif dan kepribadian karakter yang meyakinkan?",
          "q3": "Apakah Anda ingin model mengingat detail karakter dan tetap dalam karakter dalam sesi panjang?"
        },
        "speed": {
          "title": "Kecepatan & latensi",
          "description": "Model yang lebih cepat terasa lebih baik untuk percakapan yang banyak bolak-balik. Beberapa model menukarkan sedikit kualitas untuk kecepatan yang jauh lebih tinggi.",
          "q1": "Apakah Anda ingin balasan hampir instan untuk menjaga roleplay mengalir secara alami?",
          "q2": "Apakah Anda melakukan adegan dialog cepat di mana menunggu akan merusak imersivitas?",
          "q3": "Apakah ini untuk RP santai di mana bolak-balik cepat lebih penting daripada respons sempurna?"
        },
        "budget": {
          "title": "Anggaran & penggunaan",
          "description": "Setiap penyedia menagih per token. Bahkan model murah bertambah jika Anda banyak mengobrol, jadi pilih sesuatu yang cocok dengan seberapa sering dan seberapa banyak Anda menggunakannya.",
          "q1": "Apakah Anda bersedia membayar lebih untuk interaksi karakter yang lebih kaya, atau Anda ingin sesuatu yang murah untuk RP harian?",
          "q2": "Apakah Anda memiliki model gratis dari penyedia/router yang bisa dicoba dulu?",
          "q3": "Apakah Anda akan menjalankan sesi roleplay panjang dengan deskripsi adegan yang detail?",
          "q4": "Apakah Anda memiliki anggaran bulanan yang ketat yang tidak ingin Anda melebihinya?"
        },
        "safety": {
          "title": "Keamanan, privasi & ekstra",
          "description": "Penyedia berbeda dalam cara mereka menangani keamanan, pencatatan, dan fitur ekstra seperti gambar, alat, atau jendela konteks panjang.",
          "q1": "Apakah Anda memerlukan lebih sedikit filter konten untuk skenario roleplay dewasa atau kreatif?",
          "q2": "Apakah Anda peduli jika percakapan RP pribadi Anda dicatat atau digunakan untuk pelatihan?",
          "q3": "Apakah Anda memerlukan jendela konteks panjang untuk alur cerita yang kompleks dan sejarah karakter?"
        }
      },
      "where": {
        "title": "Di mana saya bisa menemukan model?",
        "intro": "Sebagian besar penyedia dan router memiliki daftar atau katalog model. Jelajahi halaman tersebut untuk melihat apa yang mereka tawarkan, harga, batas, dan fitur khusus.",
        "directTitle": "Penyedia langsung",
        "directDesc": "OpenAI, Anthropic, Google Gemini, xAI, Mistral, dll. Masing-masing memiliki konsol/dasbor di mana Anda dapat melihat nama model resmi, kemampuan, dan harga.",
        "routersTitle": "Router & hub",
        "routersDesc": "Layanan seperti OpenRouter atau agregator lain mendaftarkan banyak model dari berbagai penyedia di satu tempat, sering dengan perbandingan benchmark dan harga.",
        "communityTitle": "Rekomendasi komunitas",
        "communityDesc": "Lihat dokumen, blog, atau posting komunitas dari penyedia/router Anda. Mereka biasanya menyoroti model mana yang terbaik untuk obrolan, pengkodean, atau kecepatan."
      },
      "rules": {
        "title": "Aturan sederhana",
        "casual": "Untuk obrolan santai: pilih model obrolan yang cepat dan murah dari penyedia/router Anda.",
        "experiments": "Untuk eksperimen atau volume tinggi: mulai dengan model termurah yang terasa cukup baik, lalu tingkatkan jika diperlukan.",
        "switch": "Jika ada yang terasa tidak beres (terlalu lambat / terlalu bodoh / terlalu mahal): Anda selalu dapat beralih model nanti di LettuceAI."
      },
      "disclaimer": "Selalu periksa dokumentasi penyedia sendiri untuk daftar model, batas, dan harga terbaru. Halaman ini tentang cara berpikir, bukan apa yang harus dibeli."
    },
    "whereToFind": {
      "badge": "Bantuan Kunci API",
      "intro": "Ikuti langkah-langkah ini untuk mendapatkan kunci API Anda, lalu kembali ke LettuceAI dan tempelkan ke pengaturan penyedia.",
      "readyPrompt": "Siap mendapatkan kuncinya?",
      "openProviderSite": "Buka situs penyedia",
      "keyWarning": "Jangan pernah membagikan kunci API Anda secara publik. Siapa pun yang memiliki kunci ini dapat menggunakan saldo akun Anda.",
      "stuckPrompt": "Masih tidak bisa mencari tahu?",
      "joinDiscord": "Bergabunglah dengan server Discord kami untuk mendapat bantuan",
      "guides": {
        "chutes": {
          "title": "Cara menemukan kunci API Chutes Anda",
          "s1": "Buka chutes.ai/app dan masuk.",
          "s2": "Buka area akun/pengaturan Anda dan temukan Kunci API.",
          "s3": "Buat kunci baru (atau salin yang sudah ada).",
          "s4": "Tempelkan kunci ke LettuceAI."
        },
        "openai": {
          "title": "Cara menemukan kunci API OpenAI Anda",
          "s1": "Buka platform.openai.com dan masuk.",
          "s2": "Klik avatar profil Anda di kanan atas, lalu pilih Kunci API.",
          "s3": "Klik Buat kunci rahasia baru dan salin nilai yang ditampilkan.",
          "s4": "Tempelkan kunci ke LettuceAI dan simpan di tempat yang aman. Anda tidak akan melihatnya lagi."
        },
        "anthropic": {
          "title": "Cara menemukan kunci API Anthropic Anda",
          "s1": "Buka console.anthropic.com dan masuk.",
          "s2": "Buka Pengaturan dari bilah sisi kiri.",
          "s3": "Pilih Kunci API dan klik Buat kunci.",
          "s4": "Salin kunci dan tempelkan ke LettuceAI."
        },
        "openrouter": {
          "title": "Cara menemukan kunci API OpenRouter Anda",
          "s1": "Kunjungi openrouter.ai dan masuk.",
          "s2": "Buka halaman Kunci dari menu profil Anda.",
          "s3": "Klik Buat kunci, beri nama, dan simpan.",
          "s4": "Salin kunci dan tempelkan ke LettuceAI."
        },
        "mistral": {
          "title": "Cara menemukan kunci API Mistral Anda",
          "s1": "Buka console.mistral.ai dan masuk.",
          "s2": "Klik Kunci API di bilah sisi.",
          "s3": "Klik Buat kunci API.",
          "s4": "Salin kunci dan tempelkan ke LettuceAI."
        },
        "deepseek": {
          "title": "Cara menemukan kunci API DeepSeek Anda",
          "s1": "Buka platform.deepseek.com dan masuk.",
          "s2": "Klik Kunci API di navigasi atas.",
          "s3": "Buat kunci baru jika belum ada.",
          "s4": "Salin kunci dan tempelkan ke LettuceAI."
        },
        "groq": {
          "title": "Cara menemukan kunci API Groq Anda",
          "s1": "Kunjungi console.groq.com dan masuk.",
          "s2": "Buka Kunci API dari bilah sisi.",
          "s3": "Buat kunci baru, lalu salin.",
          "s4": "Tempelkan kunci ke LettuceAI."
        },
        "gemini": {
          "title": "Cara menemukan kunci API Google Gemini Anda",
          "s1": "Buka Google AI Studio di aistudio.google.com dan masuk.",
          "s2": "Klik Dapatkan kunci API atau Kelola kunci.",
          "s3": "Buat kunci baru jika diperlukan.",
          "s4": "Salin kunci dan tempelkan ke LettuceAI."
        },
        "xai": {
          "title": "Cara menemukan kunci API xAI Anda",
          "s1": "Buka console.x.ai dan masuk.",
          "s2": "Navigasikan ke bagian Kunci API di konsol.",
          "s3": "Buat kunci baru.",
          "s4": "Salin kunci dan tempelkan ke LettuceAI."
        },
        "zai": {
          "title": "Cara menemukan kunci API zAI (GLM) Anda",
          "s1": "Buka open.bigmodel.cn dan masuk.",
          "s2": "Buka Pusat Pengguna, lalu ke Kunci API.",
          "s3": "Buat kunci baru jika belum ada.",
          "s4": "Salin kunci dan tempelkan ke LettuceAI."
        },
        "moonshot": {
          "title": "Cara menemukan kunci API Moonshot (Kimi) Anda",
          "s1": "Kunjungi platform.moonshot.cn dan masuk.",
          "s2": "Buka bagian Kunci API di konsol.",
          "s3": "Buat kunci baru dan salin.",
          "s4": "Tempelkan kunci ke LettuceAI."
        },
        "qwen": {
          "title": "Cara menemukan kunci API Qwen Anda",
          "s1": "Buka dashscope.aliyun.com dan masuk.",
          "s2": "Buka bagian Kunci API di bilah sisi.",
          "s3": "Buat kunci baru.",
          "s4": "Salin kunci dan tempelkan ke LettuceAI."
        },
        "nanogpt": {
          "title": "Cara menemukan kunci API NanoGPT Anda",
          "s1": "Buka nano-gpt.com dan masuk.",
          "s2": "Buka dasbor dan ke bagian kunci API.",
          "s3": "Buat kunci baru jika diperlukan.",
          "s4": "Salin kunci dan tempelkan ke LettuceAI."
        },
        "featherless": {
          "title": "Cara menemukan kunci API Featherless Anda",
          "s1": "Kunjungi featherless.ai dan masuk.",
          "s2": "Buka akun atau bagian API dari dasbor.",
          "s3": "Buat kunci baru jika belum ada.",
          "s4": "Salin kunci dan tempelkan ke LettuceAI."
        },
        "anannas": {
          "title": "Cara menemukan kunci API Anannas Anda",
          "s1": "Buka dashboard.anannas.ai dan masuk.",
          "s2": "Navigasikan ke bagian Kunci API.",
          "s3": "Buat kunci baru dan salin.",
          "s4": "Tempelkan kunci ke LettuceAI."
        },
        "default": {
          "title": "Cara menemukan kunci API Anda",
          "s1": "Buka dasbor penyedia AI Anda di browser dan masuk.",
          "s2": "Cari pengaturan API, Pengembang, atau Integrasi.",
          "s3": "Buat kunci API baru atau lihat yang sudah ada.",
          "s4": "Salin kunci dan tempelkan ke LettuceAI."
        }
      }
    },
    "welcomeFooter": {
      "setupTimeMobile": "Pengaturan memakan waktu kurang dari 2 menit"
    }
  },
  "search": {
    "placeholder": "Cari...",
    "tabs": {
      "characters": "Karakter",
      "personas": "Persona"
    },
    "noResults": "Tidak ada {{type}} ditemukan",
    "emptyState": "Belum ada {{type}}",
    "noResultsHint": "Coba kata pencarian yang berbeda",
    "emptyCharacters": "Buat karakter pertama Anda untuk mulai mengobrol",
    "emptyPersonas": "Buat persona di pengaturan",
    "a11y": {
      "goBack": "Kembali",
      "clearSearch": "Hapus pencarian",
      "characterAvatar": "Avatar {{name}}"
    },
    "session": {
      "newChatTitle": "Obrolan Baru"
    },
    "noDescription": "Tidak ada deskripsi",
    "defaultBadge": "Bawaan"
  },
  "sync": {
    "modes": {
      "join": "Gabung",
      "joinDesc": "Hubungkan ke host",
      "host": "Host",
      "hostDesc": "Bagikan data Anda"
    },
    "sections": {
      "mode": "Mode",
      "connectToHost": "Hubungkan ke Host",
      "startHosting": "Mulai Hosting",
      "status": "Status",
      "hosting": "Layanan Hosting",
      "localAddress": "Alamat Jaringan Lokal",
      "connectionPin": "PIN Koneksi",
      "setupGuide": "Panduan Pengaturan"
    },
    "fields": {
      "hostAddress": "Alamat Host atau JSON",
      "hostPlaceholder": "mis. 192.168.1.100:12345",
      "pinCode": "Kode PIN",
      "pinPlaceholder": "000000"
    },
    "buttons": {
      "scanQRCode": "Pindai Kode QR",
      "connect": "Hubungkan",
      "connecting": "Menghubungkan...",
      "startHosting": "Mulai Hosting",
      "startingServer": "Memulai server...",
      "stopHosting": "Hentikan Hosting",
      "hostAgain": "Host Lagi",
      "done": "Selesai"
    },
    "status": {
      "connecting": "Menghubungkan...",
      "connected": "Terhubung",
      "waitingConfirmation": "Menunggu kepastian",
      "waitingConfirmationDesc": "Setujui koneksi pada perangkat host untuk melanjutkan.",
      "syncing": "Menyinkronkan...",
      "transferringData": "Mentransfer data",
      "syncInProgress": "Sinkronisasi Berlangsung",
      "live": "Live",
      "broadcasting": "Menyiarkan",
      "clientsLabel": "Terhubung",
      "clientsUnit": "Klien"
    },
    "pinDescription": "Bagikan PIN ini dengan perangkat yang menghubungkan",
    "hostingDesc1": "Perangkat lain dapat menghubungkan dan menyinkronkan data dari perangkat ini.",
    "hostingDesc2": "Data Anda akan dibagikan dengan klien yang terhubung.",
    "setupSteps": {
      "step1": "Buka aplikasi di perangkat lain",
      "step2": "Buka Pengaturan → Sinkronisasi Lokal",
      "step3": "Pindai kode QR atau masukkan alamat"
    },
    "messages": {
      "completed": "Sinkronisasi Selesai!",
      "completedDesc": "Semua data tersinkronisasi",
      "error": "Kesalahan Koneksi",
      "outdatedClient": "Klien Usang Terdeteksi"
    },
    "disclaimer": "Sinkronisasi bekerja melalui jaringan lokal. Kedua perangkat harus pada WiFi yang sama.",
    "modals": {
      "connectionRequest": "Permintaan Koneksi",
      "requestMessage": "ingin menyinkronkan dengan perangkat ini.",
      "acceptConnection": "Terima Koneksi",
      "acceptDesc": "Izinkan perangkat ini menyinkronkan data",
      "decline": "Tolak",
      "declineDesc": "Blokir upaya koneksi ini",
      "readyToSync": "Siap untuk Sinkronisasi",
      "connectionEstablished": "Koneksi Terbentuk",
      "deviceReady": "siap.",
      "startSyncMessage": "Ketuk di bawah untuk mulai menyinkronkan data.",
      "startSyncing": "Mulai Sinkronisasi",
      "startSyncingDesc": "Mulai transfer data sekarang"
    },
    "scanner": {
      "title": "Pindai Kode QR",
      "cancel": "Batalkan Pemindaian"
    },
    "unknownDevice": "Perangkat Tidak Dikenal",
    "aria": {
      "dismissStatus": "Tutup status sinkronisasi",
      "dismissError": "Tutup kesalahan sinkronisasi"
    },
    "stats": {
      "statusLabel": "Status"
    }
  },
  "creationHelper": {
    "page": {
      "info": "Pembantu Pembuatan memandu Anda membangun karakter dengan bantuan AI. Konfigurasikan model dan alat yang digunakan selama pembuatan karakter.",
      "modelConfiguration": "Konfigurasi Model",
      "chatModel": "Model Obrolan",
      "selectedModel": "Model Terpilih",
      "useAppDefault": "Gunakan default aplikasi{{model}}",
      "useAppDefaultBase": "Gunakan default aplikasi",
      "noModelsAvailable": "Tidak ada model tersedia",
      "chatModelDescription": "Model AI untuk percakapan pembuatan karakter",
      "streamingOutput": "Output Streaming",
      "streamingDescription": "Tampilkan respons saat sedang dihasilkan",
      "imageGenerationModel": "Model Pembuatan Gambar",
      "noModelSelected": "Tidak ada model dipilih",
      "noImageModelsAvailable": "Tidak ada model gambar tersedia",
      "imageModelDescription": "Untuk menghasilkan avatar karakter",
      "toolSelection": "Pilihan Alat",
      "smartToolSelection": "Pilihan Alat Cerdas",
      "smartToolDescription": "AI secara otomatis memilih alat mana yang digunakan",
      "smartToolEnabledHint": "Saat diaktifkan, AI Pembantu Pembuatan bertanya apa yang ingin Anda buat dan hanya memuat set alat yang relevan.",
      "smartToolDisabledHint": "Saat dinonaktifkan, AI Pembantu Pembuatan langsung terbuka dan menggunakan semua alat yang diaktifkan; asisten memutuskan apa yang akan dibuat.",
      "quickPresets": "Preset Cepat",
      "customSelection": "Pilihan kustom - {{count}} alat diaktifkan",
      "footerInfo": "Saat Pilihan Alat Cerdas diaktifkan, AI memutuskan alat mana yang digunakan berdasarkan konteks. Nonaktifkan untuk mengontrol alat yang tersedia secara manual.",
      "selectChatModel": "Pilih Model Obrolan",
      "selectImageModel": "Pilih Model Gambar",
      "searchModels": "Cari model..."
    },
    "categories": {
      "basic": "Dasar",
      "content": "Konten",
      "visual": "Visual",
      "settings": "Pengaturan",
      "flow": "Alur",
      "persona": "Persona",
      "lorebook": "Lorebook"
    },
    "presets": {
      "all": {
        "name": "Semua Alat",
        "desc": "Aktifkan semua alat yang tersedia"
      },
      "essential": {
        "name": "Esensial",
        "desc": "Hanya nama, definisi, dan adegan"
      },
      "minimal": {
        "name": "Minimal",
        "desc": "Hanya nama dan definisi"
      }
    },
    "tools": {
      "setName": "Atur Nama",
      "setNameDesc": "Atur nama karakter",
      "setDefinition": "Atur Definisi",
      "setDefinitionDesc": "Atur kepribadian dan latar belakang",
      "set_character_name": {
        "name": "Atur Nama",
        "desc": "Atur nama karakter"
      },
      "set_character_definition": {
        "name": "Atur Definisi",
        "desc": "Atur kepribadian dan latar belakang"
      },
      "add_scene": {
        "name": "Tambah Adegan",
        "desc": "Tambah adegan awal untuk roleplay"
      },
      "update_scene": {
        "name": "Perbarui Adegan",
        "desc": "Ubah adegan yang ada"
      },
      "toggle_avatar_gradient": {
        "name": "Gradien Avatar",
        "desc": "Aktifkan/nonaktifkan overlay gradien pada avatar"
      },
      "set_default_model": {
        "name": "Atur Model",
        "desc": "Atur model AI untuk percakapan"
      },
      "set_system_prompt": {
        "name": "Prompt Sistem",
        "desc": "Atur pedoman perilaku"
      },
      "get_system_prompt_list": {
        "name": "Daftar Prompt",
        "desc": "Lihat prompt yang tersedia"
      },
      "get_model_list": {
        "name": "Daftar Model",
        "desc": "Lihat model yang tersedia"
      },
      "use_uploaded_image_as_avatar": {
        "name": "Gambar sebagai Avatar",
        "desc": "Gunakan gambar yang diunggah sebagai avatar"
      },
      "use_uploaded_image_as_chat_background": {
        "name": "Gambar sebagai Latar",
        "desc": "Gunakan gambar yang diunggah sebagai latar belakang"
      },
      "generate_image": {
        "name": "Hasilkan Gambar",
        "desc": "Hasilkan gambar dengan model AI"
      },
      "show_preview": {
        "name": "Tampilkan Pratinjau",
        "desc": "Pratinjau karakter"
      },
      "request_confirmation": {
        "name": "Minta Konfirmasi",
        "desc": "Tanya untuk menyimpan atau melanjutkan"
      },
      "list_personas": {
        "name": "Daftar Persona",
        "desc": "Jelajahi persona"
      },
      "upsert_persona": {
        "name": "Simpan Persona",
        "desc": "Buat atau perbarui persona"
      },
      "use_uploaded_image_as_persona_avatar": {
        "name": "Avatar Persona",
        "desc": "Gunakan gambar yang diunggah sebagai avatar persona"
      },
      "delete_persona": {
        "name": "Hapus Persona",
        "desc": "Hapus persona"
      },
      "get_default_persona": {
        "name": "Persona Bawaan",
        "desc": "Ambil persona bawaan"
      },
      "list_lorebooks": {
        "name": "Daftar Lorebook",
        "desc": "Jelajahi lorebook"
      },
      "upsert_lorebook": {
        "name": "Simpan Lorebook",
        "desc": "Buat atau perbarui lorebook"
      },
      "delete_lorebook": {
        "name": "Hapus Lorebook",
        "desc": "Hapus lorebook"
      },
      "list_lorebook_entries": {
        "name": "Daftar Entri",
        "desc": "Lihat entri lorebook"
      },
      "get_lorebook_entry": {
        "name": "Ambil Entri",
        "desc": "Ambil entri lorebook"
      },
      "upsert_lorebook_entry": {
        "name": "Simpan Entri",
        "desc": "Buat atau perbarui entri"
      },
      "delete_lorebook_entry": {
        "name": "Hapus Entri",
        "desc": "Hapus entri lorebook"
      },
      "create_blank_lorebook_entry": {
        "name": "Entri Kosong",
        "desc": "Buat entri placeholder"
      },
      "reorder_lorebook_entries": {
        "name": "Urutkan Ulang Entri",
        "desc": "Ubah urutan entri"
      },
      "list_character_lorebooks": {
        "name": "Daftar Lorebook Karakter",
        "desc": "Lihat lorebook untuk karakter"
      },
      "set_character_lorebooks": {
        "name": "Atur Lorebook Karakter",
        "desc": "Tetapkan lorebook ke karakter"
      },
      "addScene": "Tambah Adegan",
      "addSceneDesc": "Tambah adegan awal untuk roleplay",
      "updateScene": "Perbarui Adegan",
      "updateSceneDesc": "Ubah adegan yang ada",
      "avatarGradient": "Gradien Avatar",
      "avatarGradientDesc": "Aktifkan/nonaktifkan overlay gradien pada avatar",
      "setModel": "Atur Model",
      "setModelDesc": "Atur model AI untuk percakapan",
      "systemPrompt": "Prompt Sistem",
      "systemPromptDesc": "Atur pedoman perilaku",
      "listPrompts": "Daftar Prompt",
      "listPromptsDesc": "Lihat prompt yang tersedia",
      "listModels": "Daftar Model",
      "listModelsDesc": "Lihat model yang tersedia",
      "imageAsAvatar": "Gambar sebagai Avatar",
      "imageAsAvatarDesc": "Gunakan gambar yang diunggah sebagai avatar"
    }
  },
  "tour": {
    "stepCounter": "Langkah {{current}} dari {{total}}",
    "skipTour": "Lewati tur",
    "next": "Berikutnya",
    "gotIt": "Mengerti",
    "appShell": {
      "chats": {
        "title": "Di sinilah obrolan Anda berada",
        "body": "Semua percakapan satu-satu Anda dengan karakter ada di sini. Kembali kapan saja dan kami akan menyimpan posisi Anda."
      },
      "groups": {
        "title": "Bergabung di obrolan grup",
        "body": "Bawa beberapa karakter ke dalam satu ruangan dan saksikan mereka saling berbicara, atau ikut kapan pun Anda mau."
      },
      "discover": {
        "title": "Temukan karakter baru",
        "body": "Jelajahi apa yang dibagikan komunitas dan ambil karakter yang menarik perhatian Anda. Favorit baru hanya satu ketukan saja."
      },
      "library": {
        "title": "Perpustakaan pribadi Anda",
        "body": "Semua yang Anda buat atau simpan ada di sini: karakter, persona, prompt, semuanya. Anggap saja ini koleksi Anda."
      },
      "settings": {
        "title": "Sesuaikan sesuka Anda",
        "body": "Ganti penyedia, pilih model berbeda, atur tampilan aplikasi. Hampir semuanya bisa disesuaikan dari pengaturan."
      },
      "search": {
        "title": "Temukan apa saja, cepat",
        "body": "Mencari obrolan atau karakter tertentu? Cari di semua tempat dari sini. Tanpa perlu menggali."
      },
      "create": {
        "title": "Dan akhirnya, buat!",
        "body": "Ketuk tombol plus kapan pun inspirasi datang. Buat karakter baru, persona, atau mulai sesuatu dari awal."
      }
    },
    "chatDetail": {
      "chatTitle": {
        "title": "Pengaturan per obrolan",
        "body": "Ketuk nama karakter di atas untuk membuka pengaturan khusus obrolan ini. Persona, tata letak, dan pilihan model berbeda per percakapan."
      },
      "chatMemory": {
        "title": "Apa yang mereka ingat",
        "body": "Ikon otak menunjukkan apa yang karakter Anda ingat tentang percakapan. Ketuk untuk meninjau, mengedit, atau menghapus memori."
      },
      "chatSearch": {
        "title": "Temukan baris itu",
        "body": "Cari hanya di percakapan ini. Bagus untuk menemukan detail dari 200 pesan lalu tanpa menggulir selamanya."
      },
      "chatLorebook": {
        "title": "Entri Lorebook",
        "body": "Fakta tambahan, pembangunan dunia, dan konteks yang dimasukkan ke prompt saat kata kunci tertentu muncul. Contekan karakter Anda."
      },
      "chatPlus": {
        "title": "Lampirkan sesuatu",
        "body": "Masukkan gambar atau buka menu ekstra. Apa pun yang Anda lampirkan akan dikirim bersama pesan berikutnya."
      },
      "chatComposer": {
        "title": "Pesan Anda, giliran Anda",
        "body": "Ketik di sini. Enter mengirim, Shift+Enter menambah baris baru. Tips: tekan lama pesan apa pun di obrolan untuk mengedit, bercabang, atau menghapus."
      },
      "chatSend": {
        "title": "Satu tombol, empat fungsi",
        "body": "Tombol kirim berubah fungsinya tergantung apa yang sedang terjadi:"
      }
    },
    "postFirstMessage": {
      "chatRegenerate": {
        "title": "Tidak puas? Regenerasi",
        "body": "Ketuk ikon refresh untuk mendapatkan balasan yang sama sekali baru dari karakter. Setiap regenerasi disimpan sebagai varian yang bisa Anda kunjungi kembali."
      },
      "chatVariants": {
        "title": "Geser antar varian",
        "body": "Setelah regenerasi, Anda akan melihat penghitung varian di bawah pesan. Geser ke kiri atau kanan pada gelembung pesan untuk melihat semua balasan berbeda."
      },
      "chatLongPress": {
        "title": "Ada lebih banyak tersembunyi di sini",
        "body": "Tekan lama pesan apa pun untuk mengedit, menyalin, bercabang, menyematkan, menghapus, atau memutar ulang percakapan. Klik kanan juga berfungsi di desktop."
      }
    },
    "sendButton": {
      "continue": {
        "label": "Lanjutkan",
        "desc": "Input kosong. Ketuk di sini untuk mendorong karakter terus berbicara."
      },
      "send": {
        "label": "Kirim",
        "desc": "Anda telah mengetik atau melampirkan sesuatu. Ketuk untuk mengirim."
      },
      "sending": {
        "label": "Mengirim",
        "desc": "Balasan sedang dalam perjalanan. Tombol terkunci."
      },
      "stop": {
        "label": "Berhenti",
        "desc": "Ketuk untuk membatalkan di tengah balasan jika Anda berubah pikiran."
      }
    },
    "extra": {
      "rerunOnboarding": "Jalankan ulang onboarding"
    }
  },
  "sessionAdvanced": {
    "title": "Parameter Sesi",
    "subtitle": "Timpa default model untuk percakapan ini",
    "goBack": "Kembali",
    "support": "Dukungan",
    "reset": "Reset",
    "save": "Simpan",
    "noSessionWarning": "Buka sesi obrolan untuk mengkonfigurasi pengaturan per sesi.",
    "overrideDefaults": "Timpa default",
    "overrideDefaultsDesc": "Sesuaikan parameter hanya untuk percakapan ini",
    "loadingContextInfo": "Memuat info konteks...",
    "sampling": {
      "title": "Sampling",
      "temperature": "Temperature",
      "temperatureDesc": "Mengontrol keacakan. Lebih rendah = lebih deterministik, lebih tinggi = lebih kreatif.",
      "temperaturePrecise": "Presisi",
      "temperatureCreative": "Kreatif",
      "topP": "Top P",
      "topPDesc": "Sampling nukleus. Membatasi token pada probabilitas kumulatif.",
      "topPFocused": "Fokus",
      "topPDiverse": "Beragam",
      "topK": "Top K",
      "topKDesc": "Membatasi sampling ke K token paling mungkin."
    },
    "outputPenalties": {
      "title": "Output & Penalti",
      "maxOutputTokens": "Token Output Maksimal",
      "maxOutputTokensDesc": "Panjang respons maksimal. Auto biarkan model yang memutuskan.",
      "auto": "Auto",
      "custom": "Kustom",
      "frequencyPenalty": "Penalti Frekuensi",
      "frequencyPenaltyDesc": "Mengurangi pengulangan urutan token.",
      "frequencyPenaltyRepeat": "Ulang",
      "frequencyPenaltyVary": "Variasi",
      "presencePenalty": "Penalti Kehadiran",
      "presencePenaltyDesc": "Mendorong eksplorasi topik baru.",
      "presencePenaltyRepeat": "Ulang",
      "presencePenaltyExplore": "Eksplorasi"
    },
    "performance": {
      "title": "Performa",
      "gpuLayers": "Layer GPU",
      "gpuLayersDesc": "Layer yang di-offload ke GPU. 0 = hanya CPU.",
      "threads": "Thread",
      "threadsDesc": "Thread CPU untuk inferensi.",
      "batchThreads": "Thread Batch",
      "batchThreadsDesc": "Thread CPU untuk pemrosesan batch.",
      "batchSize": "Ukuran Batch",
      "batchSizeDesc": "Ukuran potongan pemrosesan prompt.",
      "contextLength": "Panjang Konteks",
      "contextLengthDesc": "Timpa ukuran jendela konteks.",
      "flashAttention": "Flash Attention",
      "flashAttentionDesc": "Optimasi memori GPU.",
      "enabled": "Diaktifkan",
      "disabled": "Dinonaktifkan"
    },
    "samplingMemory": {
      "title": "Sampling & Memori",
      "minP": "Min P",
      "minPDesc": "Ambang batas probabilitas minimum.",
      "typicalP": "Typical P",
      "typicalPDesc": "Ambang batas sampling tipikal.",
      "seed": "Seed",
      "seedDesc": "Seed acak. Kosongkan untuk acak.",
      "ropeBase": "RoPE Base",
      "ropeBaseDesc": "Penimpaan basis frekuensi.",
      "ropeScale": "RoPE Scale",
      "ropeScaleDesc": "Penimpaan skala frekuensi.",
      "kvCacheType": "Tipe KV Cache",
      "kvCacheTypeDesc": "Kuantisasi KV Cache untuk menghemat VRAM.",
      "offloadKqv": "Offload KQV",
      "offloadKqvDesc": "KV Cache & operasi KQV di GPU.",
      "on": "Hidup",
      "off": "Mati",
      "samplerProfile": "Profil Sampler",
      "samplerProfileDesc": "Default yang disetel untuk stabilitas atau penalaran.",
      "balanced": "Seimbang",
      "creative": "Kreatif",
      "stable": "Stabil",
      "reasoning": "Penalaran"
    },
    "systemInfo": {
      "title": "Info Sistem",
      "maxContext": "Konteks Maks",
      "recommended": "Direkomendasikan",
      "availableRam": "RAM Tersedia",
      "availableVram": "VRAM Tersedia",
      "modelSize": "Ukuran Model"
    }
  },
  "exportMenu": {
    "title": "Format Ekspor",
    "selectFormat": "Pilih format",
    "uscTitle": "Unified System Card",
    "formats": {
      "uscPrompt": "Ekspor USC portabel untuk template prompt.",
      "uscLorebook": "Ekspor USC portabel untuk lorebook.",
      "uscModel": "Ekspor USC portabel untuk profil model.",
      "uscChatTemplate": "Ekspor USC portabel untuk template obrolan.",
      "legacyPromptJson": "Legacy Prompt JSON",
      "legacyPromptJsonDesc": "Format paket prompt eksternal saat ini.",
      "lorebookJson": "Lorebook JSON",
      "lorebookJsonDesc": "Format ekspor lorebook saat ini.",
      "modelJson": "Model JSON",
      "modelJsonDesc": "JSON profil model yang aman tanpa kredensial.",
      "chatTemplateJson": "Template Obrolan JSON",
      "chatTemplateJsonDesc": "Format ekspor template obrolan bawaan."
    },
    "extra": {
      "selectFormat": "Pilih format",
      "exportFormatTitle": "Format Ekspor",
      "uscDesc": "Ekspor USC portabel",
      "legacyJsonDesc": "Format ekspor JSON legacy",
      "formatV3Desc": "Ekspor Kartu Karakter V3",
      "formatV2Desc": "Ekspor Kartu Karakter V2",
      "formatV1Desc": "Ekspor Kartu Karakter V1",
      "uscLorebook": "Unified System Card (USC)",
      "portableLorebook": "Ekspor USC portabel untuk lorebook",
      "lorebookJson": "Lorebook JSON",
      "currentLorebook": "Format ekspor lorebook saat ini",
      "uscModel": "Unified System Card (USC)",
      "portableModel": "Ekspor USC portabel untuk profil model",
      "modelJson": "Model JSON",
      "safeModel": "JSON profil model yang aman tanpa kredensial",
      "uscTemplate": "Unified System Card (USC)",
      "portableTemplate": "Ekspor USC portabel untuk template obrolan",
      "templateJson": "Template Obrolan JSON",
      "nativeTemplate": "Format ekspor template obrolan bawaan"
    }
  },
  "designReference": {
    "title": "Referensi desain",
    "description": "Unggah beberapa gambar referensi yang jelas dan satu deskripsi visual kanonik.",
    "descriptionPlaceholder": "Deskripsikan tampilan stabil: wajah, rambut, postur tubuh, presentasi usia, petunjuk pakaian, aksesori, dan arahan seni/gaya.",
    "addReferences": "Tambah referensi",
    "visualDescription": "Deskripsi visual",
    "draftWithAi": "Draf dengan AI",
    "referenceImages": "Gambar referensi",
    "imageAlt": "Referensi desain {{index}}",
    "loading": "Memuat...",
    "removeAria": "Hapus referensi desain",
    "noImages": "Belum ada gambar referensi yang dilampirkan",
    "imageCount": "{{count}} gambar referensi terlampir",
    "emptyReferences": "Tambahkan beberapa foto referensi yang jelas untuk mengunci wajah, proporsi, pakaian, dan gaya.",
    "noWriterModel": "Tambahkan model penulis adegan yang kompatibel di pengaturan Pembuatan Gambar terlebih dahulu.",
    "noImagesForGeneration": "Tambahkan avatar atau setidaknya satu gambar referensi sebelum membuat.",
    "writerModelHelp": "Menggunakan {{model}} untuk membuat draf dari avatar dan gambar referensi Anda.",
    "noWriterModelHelp": "Tambahkan model penulis adegan yang kompatibel di pengaturan Pembuatan Gambar untuk membuat ini secara otomatis.",
    "draftMenuTitle": "Draf Desain AI",
    "draftMenuDesc": "Dibuat oleh {{model}} dari avatar dan gambar referensi saat ini.",
    "draftMenuNoWriter": "Tambahkan model penulis adegan yang kompatibel sebelum menggunakan pembantu ini.",
    "regenerate": "Regenerasi",
    "useThis": "Gunakan Ini"
  },
  "samplerOrder": {
    "title": "Urutan Sampler",
    "description": "Seret tahapan untuk mengatur ulang. Dieksekusi dari atas ke bawah.",
    "reset": "Reset",
    "resetAria": "Reset urutan sampler ke default",
    "stages": {
      "penalties": {
        "label": "Penalti",
        "desc": "Terapkan penalti frekuensi dan kehadiran sebelum penyaringan."
      },
      "grammar": {
        "label": "Tata Bahasa",
        "desc": "Batasi token saat tata bahasa template obrolan bawaan aktif."
      },
      "topK": {
        "label": "Top K",
        "desc": "Pangkas kumpulan kandidat ke token terkuat."
      },
      "topP": {
        "label": "Top P",
        "desc": "Terapkan penyaringan nukleus pada distribusi yang tersisa."
      },
      "minP": {
        "label": "Min P",
        "desc": "Buang token ekor probabilitas rendah menggunakan lantai minimum."
      },
      "typical": {
        "label": "Typical P",
        "desc": "Pilih token yang secara statistik tipikal dalam distribusi saat ini."
      },
      "temp": {
        "label": "Temperature",
        "desc": "Ratakan atau pertajam distribusi akhir sebelum pemilihan."
      }
    },
    "presets": {
      "default": {
        "label": "Default",
        "hint": "Lettuce local"
      },
      "unsloth": {
        "label": "Unsloth",
        "hint": "gaya llama.cpp"
      },
      "focused": {
        "label": "Fokus",
        "hint": "Pemangkasan ketat"
      },
      "creative": {
        "label": "Kreatif",
        "hint": "Filter terlambat"
      }
    }
  },
  "lorebookGen": {
    "flow": {
      "pickerCharactersTitle": "Karakter",
      "pickerSessionsTitle": "Sesi",
      "noCharacters": "Tidak ada karakter",
      "noSessions": "Tidak ada sesi",
      "clearSelection": "Hapus pilihan",
      "directionTitle": "Arah generasi opsional",
      "directionLabel": "Arah",
      "toggleForceMode": "Aktifkan/nonaktifkan mode paksa",
      "entryTitlePlaceholder": "Judul entri",
      "entryContentPlaceholder": "Konten entri lorebook",
      "editDirectionBeforeRegenerate": "Edit arah sebelum regenerasi",
      "generatorReturnedNoDraft": "Generator tidak mengembalikan draf",
      "pageTitle": "Buat Entri Lorebook",
      "missingContext": "Konteks lorebook hilang untuk halaman generator.",
      "characterLocked": "Karakter terkunci ke pemilik lorebook ini",
      "chooseSession": "Pilih sesi",
      "pickCharacter": "Pilih karakter",
      "searchMemories": "Cari memori",
      "searchMessages": "Cari pesan",
      "selectLastN": "Pilih {{n}} pesan terakhir",
      "selectAll": "Pilih semua",
      "loadSessionPrompt": "Pilih sesi untuk dimuat",
      "messagesText": "pesan",
      "memoriesText": "memori",
      "messagesAndMemories": "pesan dan memori",
      "titleAndContentRequired": "Judul dan konten wajib diisi",
      "keywordsOrAlwaysActive": "Tambah setidaknya satu kata kunci atau aktifkan Selalu aktif",
      "lorybookEntrySaved": "Entri lorebook disimpan",
      "saveFailed": "Penyimpanan gagal",
      "generationFailed": "Generasi gagal",
      "failedToLoadContext": "Gagal memuat generator lorebook",
      "failedToLoadSessions": "Gagal memuat sesi",
      "failedToLoadMessages": "Gagal memuat pesan",
      "userRole": "PENGGUNA",
      "aiRole": "AI"
    },
    "triggerPreview": {
      "resizeInspector": "Ubah ukuran inspektor"
    }
  },
  "companion": {
    "download": {
      "emotionClassifier": "Pengklasifikasi emosi",
      "emotionClassifierDesc": "Membaca giliran dan memperbarui vektor emosi yang dirasakan, diekspresikan, dan diblokir companion.",
      "emotionSize": "~120 MB",
      "entityExtractor": "Ekstraktor entitas (NER)",
      "entityExtractorDesc": "Mengidentifikasi orang, tempat, dan objek agar memori dapat dikanonikalisasi dan ditautkan.",
      "entitySize": "~140 MB",
      "memoryRouter": "Perute memori",
      "memoryRouterDesc": "Memutuskan apakah giliran baru harus disimpan sebagai kategori memori hubungan, pencapaian, episodik, atau lainnya.",
      "routerSize": "~70 MB",
      "unknownModel": "Model companion tidak dikenal. Sediakan ?kind=emotion|ner|router."
    }
  },
  "voices": {
    "extra": {
      "customVoices": "Suara Saya",
      "providerVoices": "Suara Penyedia",
      "myVoices": "Suara Saya",
      "page": {
        "noAudioProvidersHint": "Tambahkan satu di Penyedia → Audio untuk memulai",
        "noVoicesTitle": "Belum ada suara yang dibuat",
        "noVoicesDescription": "Buat suara dengan prompt kustom untuk karakter Anda",
        "addProviderFirst": "Tambahkan penyedia audio terlebih dahulu",
        "noPrompt": "Tidak ada prompt",
        "noProviderVoices": "Tidak ada suara ditemukan. Klik Segarkan untuk mengambil suara.",
        "showLess": "Tampilkan Lebih Sedikit",
        "showAllVoices": "Tampilkan Semua {{count}} Suara",
        "voiceFallbackTitle": "Suara"
      },
      "cache": {
        "section": "Cache Audio",
        "title": "Cache Audio TTS",
        "description": "Audio suara yang dihasilkan disimpan di cache untuk mengurangi regenerasi",
        "clearing": "Membersihkan...",
        "clear": "Bersihkan Cache"
      },
      "menu": {
        "editDescription": "Ubah suara ini",
        "deleteDescription": "Hapus suara ini",
        "provider": "Penyedia",
        "category": "Kategori",
        "createVoiceConfig": "Buat Konfigurasi Suara",
        "createVoiceConfigDescription": "Gunakan suara ini dengan pengaturan kustom"
      },
      "editor": {
        "editTitle": "Edit Suara",
        "createTitle": "Buat Suara",
        "voiceName": "Nama Suara",
        "voiceNamePlaceholder": "Suara Karakter Saya",
        "provider": "Penyedia",
        "model": "Model",
        "modelIdPlaceholder": "gpt-4o-mini-tts",
        "modelIdHint": "Masukkan ID model persis yang diharapkan endpoint kompatibel Anda",
        "elevenlabsVoice": "Suara ElevenLabs",
        "noVoicesAvailable": "Tidak ada suara tersedia",
        "selectVoice": "Pilih suara...",
        "elevenlabsVoiceHint": "Pilih dari suara ElevenLabs Anda",
        "geminiVoice": "Suara Gemini",
        "geminiVoiceHint": "Pilih suara Gemini TTS",
        "voiceId": "ID Suara",
        "voiceIdPlaceholder": "alloy",
        "voiceIdHint": "Masukkan ID suara yang didukung endpoint kompatibel Anda",
        "voicePrompt": "Prompt Suara",
        "voicePromptPlaceholder": "Suara yang hangat dan ramah dengan nada ceria...",
        "voicePromptHint": "Deskripsikan bagaimana suara seharusnya terdengar",
        "exampleText": "Teks Contoh",
        "exampleTextPlaceholder": "Halo! Begini cara saya berbicara...",
        "exampleTextHint": "Teks sampel untuk menguji suara",
        "voiceDesignChars": "{{current}}/{{minimum}} karakter diperlukan untuk pratinjau desain suara",
        "defaultSample": "Halo! Begini cara saya berbicara. Saya dapat membaca teks lebih panjang dengan kehangatan, kejernihan, dan emosi agar Anda dapat menilai nada dan kecepatan saya.",
        "playing": "Memutar...",
        "previewVoice": "Pratinjau Suara"
      },
      "kokoro": {
        "title": "Kokoro Studio",
        "newBlend": "Campuran baru",
        "editBlend": "Edit campuran",
        "tryText": "Halo! Ini adalah tes cepat cara saya berbicara.",
        "experimentDefaultText": "Halo! Begini cara saya berbicara. Saya dapat membaca teks lebih panjang dengan kehangatan, kejernihan, dan emosi.",
        "livePreview": "Pratinjau langsung",
        "savedBlend": "Campuran tersimpan",
        "defaultPreviewText": "Halo! Ini adalah pratinjau cepat bagaimana suara ini terdengar.",
        "experiment": "Eksperimen",
        "providerNotFound": "Penyedia Kokoro tidak ditemukan",
        "backToProviders": "Kembali ke penyedia",
        "variantUnset": "Varian belum diatur",
        "ready": "Siap",
        "modelNotInstalled": "Model belum dipasang",
        "voiceCount": "{{count}} suara",
        "engineActions": "Tindakan engine",
        "engineNotInstalled": "Engine belum dipasang",
        "installAtLeastOneVoice": "Pasang setidaknya satu suara",
        "continueSetup": "Lanjutkan pengaturan untuk memasang model Kokoro.",
        "pickVoiceOrStarter": "Pilih suara atau ambil paket pemula untuk memulai.",
        "downloadsFailed": "{{count}} unduhan gagal",
        "retryOrDismissAll": "Coba lagi satu per satu atau tutup semua.",
        "dismissAll": "Tutup semua",
        "model": "Model",
        "voice": "Suara",
        "downloads": "Unduhan",
        "cancelAll": "Batalkan semua",
        "experimentPlaceholder": "Ketik frasa untuk mendengarnya diucapkan...",
        "speed": "Kecepatan",
        "speak": "Ucapkan",
        "yourBlends": "Campuran Anda",
        "noSavedBlends": "Belum ada campuran tersimpan.",
        "installModelAndVoiceFirst": "Pasang model dan suara terlebih dahulu.",
        "featured": "Unggulan",
        "stop": "Hentikan",
        "sample": "Sampel",
        "voiceLibrary": "Perpustakaan suara",
        "starterPack": "Paket pemula",
        "select": "Pilih",
        "all": "Semua",
        "installed": "Terpasang",
        "installModelToBrowse": "Pasang model untuk menjelajahi suara.",
        "noVoicesInCatalog": "Tidak ada suara di katalog. Ketuk Segarkan.",
        "noVoicesMatch": "Tidak ada suara yang cocok dengan filter Anda.",
        "collapseAll": "Ciutkan semua",
        "expandAll": "Perluas semua",
        "selectedCount": "{{count}} dipilih",
        "engineTitle": "Engine Kokoro",
        "variant": "Varian",
        "status": "Status",
        "notInstalled": "Belum dipasang",
        "file": "Berkas",
        "modelSize": "Ukuran model",
        "voicesSize": "Ukuran suara",
        "total": "Total",
        "assetRoot": "Direktori aset",
        "reinstallModel": "Pasang ulang model",
        "installModel": "Pasang model",
        "deleteModel": "Hapus model",
        "deleteModelDescription": "Membebaskan ruang disk; suara tetap disimpan.",
        "blend": "Campuran",
        "previewDescription": "Dengarkan cepat dengan teks bawaan",
        "editBlendDescription": "Sesuaikan suara, bobot, dan kecepatan",
        "deleteBlendDescription": "Hapus suara tersimpan ini",
        "setupTitle": "Atur Kokoro",
        "allSet": "Semua sudah siap",
        "allSetDescription": "Jelajahi suara, rancang campuran, atau uji di area eksperimen."
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
      "conditionalsSection": "Kondisional",
      "injectionPoints": "Titik Injeksi"
    }
  },
  "embeddings": {
    "download": {
      "bestForQuick": "Terbaik untuk respons cepat",
      "balancedPerf": "Performa seimbang",
      "maxContext": "Konteks maksimal",
      "capacity1k": "1K token",
      "capacity2k": "2K token",
      "capacity4k": "4K token",
      "approxSize": "~138 MB",
      "downloadVersion": "V4"
    },
    "test": {
      "health": "Pemeriksaan Kesehatan",
      "retrieval": "Pengambilan",
      "separation": "Pemisahan",
      "passed": "Lulus",
      "failed": "Gagal",
      "testing": "Menguji..."
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
      "jsonDesc": "Output terstruktur ringkas saat pemanggilan alat tidak tersedia.",
      "xml": "XML",
      "xmlDesc": "Gunakan saat model lebih andal memformat XML daripada JSON.",
      "fallbackFormat": "Format Cadangan"
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
      "filters": "Filter",
      "model": "Model",
      "character": "Karakter",
      "clearAll": "Hapus Semua",
      "applyFilters": "Terapkan Filter",
      "recentActivity": "Aktivitas Terkini",
      "customRange": "Rentang Kustom",
      "startDate": "Tanggal Mulai",
      "endDate": "Tanggal Selesai",
      "applyRange": "Terapkan Rentang",
      "dashboard": "DASBOR",
      "appTime": "WAKTU APLIKASI",
      "today": "Hari Ini",
      "last7Days": "7 Hari",
      "last30Days": "30 Hari",
      "all": "Semua",
      "custom": "Kustom",
      "filtersCount": "{{count}} Filter",
      "totalCost": "Total Biaya",
      "tokens": "Token",
      "avgShort": "{{value}} rata-rata",
      "requests": "Permintaan",
      "period": "Periode",
      "last7d": "7 Hari Terakhir",
      "last30d": "30 Hari Terakhir",
      "allTime": "Sepanjang Waktu",
      "recordsCount": "{{count}} catatan",
      "usageTrend": "Tren Penggunaan",
      "tokenConsumptionOverTime": "Konsumsi token dari waktu ke waktu",
      "input": "Input",
      "output": "Output",
      "byModel": "Per Model",
      "byCharacter": "Per Karakter",
      "top": "Teratas",
      "active": "Aktif",
      "quickView": "Tampilan Cepat",
      "viewAll": "Lihat Semua",
      "startChatting": "Mulai mengobrol untuk melihat data penggunaan",
      "exportedTo": "Diekspor ke: {{path}}",
      "periodTotal": "Total Periode",
      "daysActive": "{{count}} hari aktif",
      "dailyAvg": "Rata-rata Harian",
      "selectedPeriod": "periode dipilih",
      "yesterdayValue": "Kemarin {{value}}",
      "thirtyDayAvg": "Rata-rata 30 Hari",
      "appTimeTrend": "Tren Waktu Aplikasi",
      "usageDurationPerDay": "Durasi penggunaan per hari",
      "totalValue": "Total {{value}}",
      "activeTime": "Waktu Aktif"
    },
    "activity": {
      "loading": "Memuat aktivitas Anda...",
      "title": "Aktivitas Terkini",
      "recordsCount": "{{count}} catatan penggunaan",
      "rangeOfTotal": "{{start}}-{{end}} dari {{total}}",
      "previous": "Sebelumnya",
      "next": "Berikutnya",
      "pageOf": "Halaman {{page}} dari {{total}}"
    },
    "shared": {
      "relativeTime": {
        "justNow": "baru saja",
        "minutesAgo": "{{count}} mnt lalu",
        "hoursAgo": "{{count}} jam lalu",
        "daysAgo": "{{count}} hari lalu"
      },
      "operations": {
        "chat": "Obrolan",
        "regenerate": "Buat Ulang",
        "continue": "Lanjutkan",
        "summary": "Ringkasan",
        "memoryManager": "Memori",
        "imageGeneration": "Buat Gambar",
        "aiCreator": "AI Creator",
        "replyHelper": "Asisten Balasan",
        "groupChatMessage": "Obrolan Grup",
        "groupChatRegenerate": "Buat Ulang Grup",
        "groupChatContinue": "Lanjutkan Grup",
        "groupChatDecisionMaker": "Pembuat Keputusan"
      },
      "outputImages": "{{count}} gambar",
      "tokens": "{{count}} token",
      "unknown": "Tidak diketahui",
      "requestDetails": "Detail Permintaan",
      "noStopReason": "Tidak ada alasan berhenti",
      "tokenUsage": "Penggunaan Token",
      "estimatedCost": "Perkiraan Biaya",
      "stats": {
        "prompt": "Prompt",
        "completion": "Penyelesaian",
        "total": "Total",
        "reasoning": "Penalaran",
        "image": "Gambar",
        "memory": "Memori",
        "summary": "Ringkasan",
        "inputImages": "Gambar Input",
        "outputImages": "Gambar Output",
        "cachedPrompt": "Prompt Tersimpan",
        "cacheWrite": "Tulis Cache",
        "webSearches": "Penelusuran Web",
        "cacheRead": "Baca Cache",
        "requestFee": "Biaya Permintaan",
        "webSearch": "Penelusuran Web",
        "providerTotal": "Total Penyedia"
      }
    }
  }
};
