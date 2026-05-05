import type { DeepPartialMessageTree } from "../types";
import { type LocaleMessages } from "./en";
import type { LocaleMetadata } from ".";

export const elMetadata: LocaleMetadata = {
  name: "Greek",
  label: "Ελληνικά",
};

export const elMessages: DeepPartialMessageTree<LocaleMessages> = {
  "common": {
    "nav": {
      "chats": "Συνομιλίες",
      "settings": "Ρυθμίσεις",
      "providers": "Πάροχοι",
      "responseStyle": "Στυλ Απάντησης",
      "models": "Μοντέλα",
      "security": "Ασφάλεια",
      "accessibility": "Προσβασιμότητα",
      "reset": "Επαναφορά",
      "backupRestore": "Αντίγραφο & Επαναφορά",
      "convertFiles": "Μετατροπή Αρχείων",
      "usageAnalytics": "Αναλυτικά Χρήσης",
      "changelog": "Ιστορικό Αλλαγών",
      "about": "Σχετικά",
      "createSystemPrompt": "Δημιουργία Προτροπής Συστήματος",
      "editSystemPrompt": "Επεξεργασία Προτροπής Συστήματος",
      "systemPrompts": "Προτροπές Συστήματος",
      "developer": "Προγραμματιστής",
      "advanced": "Προχωρημένα",
      "characters": "Χαρακτήρες",
      "lorebooks": "Βιβλία Γνώσης",
      "personas": "Περσόνες",
      "dynamicMemory": "Δυναμική Μνήμη",
      "creationHelper": "Βοηθός Δημιουργίας",
      "helpMeReply": "Βοήθησέ με να Απαντήσω",
      "editPersona": "Επεξεργασία Περσόνας",
      "newTemplate": "Νέο Πρότυπο",
      "editTemplate": "Επεξεργασία Προτύπου",
      "chatTemplates": "Πρότυπα Συνομιλίας",
      "editCharacter": "Επεξεργασία Χαρακτήρα",
      "sync": "Συγχρονισμός",
      "newCharacter": "Νέος Χαρακτήρας",
      "engineSetup": "Ρύθμιση Μηχανής",
      "llmProviders": "Πάροχοι LLM",
      "engineSettings": "Ρυθμίσεις Μηχανής",
      "lettuceEngine": "Μηχανή Lettuce",
      "create": "Δημιουργία",
      "setup": "Ρύθμιση",
      "welcome": "Καλώς ήρθατε",
      "conversation": "Συνομιλία",
      "library": "Βιβλιοθήκη",
      "groupChats": "Ομαδικές Συνομιλίες",
      "groupChat": "Ομαδική Συνομιλία",
      "imageGeneration": "Δημιουργία Εικόνας",
      "voices": "Φωνές",
      "chatAppearance": "Εμφάνιση Συνομιλίας",
      "colorCustomization": "Προσαρμογή Χρωμάτων",
      "embeddingDownload": "Λήψη Embedding",
      "embeddingTest": "Δοκιμή Embedding",
      "editModel": "Επεξεργασία Μοντέλου",
      "messageDebug": "Αποσφαλμάτωση Μηνυμάτων",
      "speechRecognition": "Αναγνώριση Ομιλίας",
      "hostApi": "Host API",
      "lorebookEntryGenerator": "Γεννήτρια Καταχωρήσεων Βιβλίου Γνώσης",
      "companionSoulWriter": "Συγγραφέας Ψυχής Συντρόφου"
    },
    "bottomNav": {
      "chats": "Συνομιλίες",
      "groups": "Ομάδες",
      "create": "Δημιουργία",
      "discover": "Ανακάλυψη",
      "library": "Βιβλιοθήκη"
    },
    "buttons": {
      "cancel": "Ακύρωση",
      "confirm": "Επιβεβαίωση",
      "save": "Αποθήκευση",
      "saving": "Αποθήκευση...",
      "delete": "Διαγραφή",
      "deleting": "Διαγραφή...",
      "create": "Δημιουργία",
      "creating": "Δημιουργία...",
      "edit": "Επεξεργασία",
      "back": "Πίσω",
      "done": "Τέλος",
      "download": "Λήψη",
      "later": "Αργότερα",
      "proceed": "Συνέχεια",
      "retry": "Επανάληψη",
      "discard": "Απόρριψη",
      "import": "Εισαγωγή",
      "importing": "Εισαγωγή...",
      "export": "Εξαγωγή",
      "exporting": "Εξαγωγή...",
      "update": "Ενημέρωση",
      "generate": "Παραγωγή",
      "refresh": "Ανανέωση",
      "continue": "Συνέχεια",
      "goBack": "Επιστροφή",
      "search": "Αναζήτηση",
      "clearSearch": "Εκκαθάριση αναζήτησης",
      "add": "Προσθήκη",
      "remove": "Αφαίρεση",
      "rename": "Μετονομασία",
      "copy": "Αντιγραφή",
      "copied": "Αντιγράφηκε!",
      "browseFiles": "Περιήγηση Αρχείων",
      "install": "Εγκατάσταση"
    },
    "labels": {
      "processing": "Επεξεργασία...",
      "loading": "Φόρτωση...",
      "noDescriptionYet": "Χωρίς περιγραφή ακόμα",
      "untitled": "Χωρίς τίτλο",
      "default": "Προεπιλογή",
      "enabled": "Ενεργό",
      "disabled": "Ανενεργό",
      "on": "Ενεργό",
      "off": "Ανενεργό",
      "none": "Κανένα",
      "auto": "Αυτόματο",
      "custom": "Προσαρμοσμένο",
      "name": "Όνομα",
      "description": "Περιγραφή",
      "content": "Περιεχόμενο",
      "preview": "Προεπισκόπηση",
      "options": "Επιλογές"
    },
    "time": {
      "justNow": "Μόλις τώρα",
      "minutesAgo": "{{minutes}}λ πριν",
      "hoursAgo": "{{hours}}ώ πριν",
      "daysAgo": "{{days}}μ πριν",
      "today": "Σήμερα",
      "yesterday": "Χθες",
      "last7Days": "Τελευταίες 7 ημέρες",
      "older": "Παλαιότερα"
    }
  },
  "settings": {
    "sections": {
      "intelligence": "Νοημοσύνη",
      "experience": "Εμπειρία",
      "connectivity": "Συνδεσιμότητα",
      "securityPrivacy": "Ασφάλεια & Απόρρητο",
      "supportInfo": "Υποστήριξη & Πληροφορίες",
      "dangerZone": "Επικίνδυνη Ζώνη",
      "developer": "Προγραμματιστής"
    },
    "items": {
      "providers": {
        "title": "Πάροχοι",
        "subtitle": "Σύνδεση σε υπηρεσίες AI"
      },
      "models": {
        "title": "Μοντέλα",
        "subtitle": "Ρύθμιση μοντέλων AI"
      },
      "imageGeneration": {
        "title": "Δημιουργία εικόνων",
        "subtitle": "Δημιουργήστε και δοκιμάστε εικόνες"
      },
      "voices": {
        "title": "Φωνές",
        "subtitle": "Φωνές μετατροπής κειμένου σε ομιλία"
      },
      "accessibility": {
        "title": "Προσβασιμότητα",
        "subtitle": "Ηχητικά σήματα & απτική ανάδραση"
      },
      "prompts": {
        "title": "Προτροπές Συστήματος",
        "subtitle": "Διαμόρφωση προσωπικότητας AI"
      },
      "security": {
        "title": "Ασφάλεια",
        "subtitle": "Κρυπτογράφηση & απόρρητο"
      },
      "backup": {
        "title": "Αντίγραφο & Επαναφορά",
        "subtitle": "Εξαγωγή ή εισαγωγή δεδομένων"
      },
      "convert": {
        "title": "Μετατροπή Αρχείων",
        "subtitle": "Μετάβαση παλαιών .json σε .uec"
      },
      "sync": {
        "title": "Τοπικός Συγχρονισμός",
        "subtitle": "Συγχρονισμός μεταξύ συσκευών"
      },
      "usage": {
        "title": "Αναλυτικά Χρήσης",
        "subtitle": "Κόστος & στατιστικά tokens"
      },
      "advanced": {
        "title": "Προχωρημένα",
        "subtitle": "Μνήμη & λειτουργίες"
      },
      "logs": {
        "title": "Αρχεία Καταγραφής",
        "subtitle": "Εντοπισμός σφαλμάτων"
      },
      "guide": {
        "title": "Οδηγός Ρύθμισης",
        "subtitle": "Επανεκκίνηση εισαγωγής"
      },
      "docs": {
        "title": "Τεκμηρίωση",
        "subtitle": "Οδηγοί & αναφορά"
      },
      "github": {
        "title": "Αναφορά Προβλημάτων",
        "subtitle": "Σφάλματα & σχόλια • v{{version}}"
      },
      "discord": {
        "title": "Discord",
        "subtitle": "Κοινότητα & βοήθεια"
      },
      "about": {
        "title": "Σχετικά",
        "subtitle": "Έκδοση, ενημερώσεις και σύνδεσμοι"
      },
      "changelog": {
        "title": "Ιστορικό Αλλαγών",
        "subtitle": "Τι νέο υπάρχει"
      },
      "reset": {
        "title": "Επαναφορά",
        "subtitle": "Διαγραφή όλων"
      },
      "developer": {
        "title": "Εργαλεία Προγραμματιστή",
        "subtitle": "Εντοπισμός σφαλμάτων & δοκιμές"
      }
    }
  },
  "accessibility": {
    "sectionTitles": {
      "language": "Γλώσσα",
      "sounds": "Ηχητική Ανάδραση",
      "haptics": "Απτική Ανάδραση",
      "appearance": "Εμφάνιση"
    },
    "language": {
      "appLanguage": "Γλώσσα εφαρμογής",
      "description": "Επιλέξτε τη γλώσσα που χρησιμοποιείται στην πλοήγηση και τις ρυθμίσεις."
    },
    "appearance": {
      "customColors": "Προσαρμοσμένα Χρώματα",
      "customColorsDesc": "Εξατομικεύστε τα χρώματα της εφαρμογής",
      "chatAppearance": "Εμφάνιση Συνομιλίας",
      "chatAppearanceDesc": "Προσαρμόστε τα συννεφάκια μηνυμάτων, γραμματοσειρές και διάταξη"
    },
    "haptics": {
      "vibrateOnChat": "Δόνηση στη Συνομιλία",
      "vibrateDesc": "Σύντομοι παλμοί δόνησης όταν ο βοηθός πληκτρολογεί",
      "intensity": "Ένταση",
      "light": "Ελαφριά",
      "medium": "Μεσαία",
      "heavy": "Δυνατή",
      "soft": "Απαλή",
      "rigid": "Σταθερή"
    },
    "sounds": {
      "send": "Αποστολή",
      "sendDescription": "Ακούγεται όταν στέλνετε μήνυμα",
      "success": "Επιτυχία",
      "successDescription": "Ακούγεται όταν ο βοηθός ολοκληρώσει επιτυχώς",
      "failure": "Αποτυχία",
      "failureDescription": "Ακούγεται σε σφάλμα ή ακύρωση",
      "testButton": "Δοκιμή"
    },
    "feedbackInfo": "Η ανάδραση σας βοηθά να παρατηρείτε πότε αποστέλλονται ή λαμβάνονται μηνύματα.",
    "hapticsInfo": "Η απτική ανάδραση είναι διαθέσιμη σε κινητές συσκευές.",
    "extra": {
      "easterEggs": "Πασχαλινά αυγά",
      "beetrootRain": "Βροχή Παντζαριών",
      "beetrootDesc": "Παντζάρια πέφτουν όταν οι συνομιλίες τα αναφέρουν"
    }
  },
  "topNav": {
    "unsavedChangesTitle": "Μη αποθηκευμένες αλλαγές",
    "unsavedChangesMessage": "Αποθηκεύστε ή απορρίψτε τις αλλαγές σας πριν φύγετε.",
    "goBack": "Επιστροφή",
    "changeLayout": "Αλλαγή διάταξης",
    "search": "Αναζήτηση",
    "settings": "Ρυθμίσεις",
    "help": "Βοήθεια",
    "add": "Προσθήκη",
    "openFilters": "Άνοιγμα φίλτρων",
    "save": "Αποθήκευση",
    "extra": {
      "installedModels": "Εγκατεστημένα Μοντέλα",
      "refresh": "Ανανέωση",
      "minimize": "Ελαχιστοποίηση",
      "maximize": "Μεγιστοποίηση",
      "close": "Κλείσιμο"
    }
  },
  "updates": {
    "available": {
      "title": "Νέα έκδοση διαθέσιμη",
      "description": "Η v{{latestVersion}} είναι διαθέσιμη. Χρησιμοποιείτε την v{{currentVersion}}.",
      "actions": {
        "view": "Προβολή έκδοσης"
      }
    }
  },
  "about": {
    "kicker": "Πληροφορίες Εφαρμογής",
    "appName": "LettuceAI",
    "description": "Λεπτομέρειες έκδοσης, έλεγχοι ενημερώσεων και χρήσιμοι σύνδεσμοι.",
    "info": {
      "version": "Έκδοση",
      "channel": "Κανάλι",
      "platform": "Πλατφόρμα"
    },
    "buildChannel": {
      "dev": "Έκδοση ανάπτυξης",
      "release": "Σταθερή έκδοση"
    },
    "update": {
      "sectionTitle": "Ενημερώσεις",
      "title": "Ενημερώσεις εφαρμογής",
      "description": "Ελέγξτε χειροκίνητα για νέες εκδόσεις ή απενεργοποιήστε τους αυτόματους ελέγχους κατά την εκκίνηση.",
      "autoChecks": "Αυτόματοι έλεγχοι ενημερώσεων",
      "autoChecksDescription": "Όταν είναι ενεργό, το LettuceAI ελέγχει για νέες εκδόσεις όταν ανοίγει η εφαρμογή.",
      "checkNow": "Έλεγχος για ενημερώσεις",
      "checking": "Έλεγχος για ενημερώσεις...",
      "upToDateTitle": "Είστε ενημερωμένοι",
      "upToDateDescription": "Δεν υπάρχει νεότερη έκδοση διαθέσιμη για αυτήν τη συσκευή αυτή τη στιγμή.",
      "failedTitle": "Ο έλεγχος ενημερώσεων απέτυχε",
      "failedDescription": "Δεν ήταν δυνατός ο έλεγχος για ενημερώσεις αυτή τη στιγμή. Δοκιμάστε ξανά σε λίγο."
    },
    "links": {
      "sectionTitle": "Σύνδεσμοι",
      "website": "Ιστότοπος",
      "websiteDescription": "Σελίδα λήψης και πληροφορίες έκδοσης",
      "github": "GitHub",
      "githubDescription": "Πηγαίος κώδικας, ζητήματα και ανάπτυξη",
      "discord": "Discord",
      "discordDescription": "Διακομιστής κοινότητας και υποστήριξη",
      "reddit": "Reddit",
      "redditDescription": "Συζητήσεις κοινότητας, σχόλια και ενημερώσεις"
    },
    "developerMode": {
      "enable": "Ενεργοποίηση Λειτουργίας Προγραμματιστή",
      "enabled": "Η Λειτουργία Προγραμματιστή Ενεργοποιήθηκε"
    },
    "errors": {
      "saveTitle": "Δεν ήταν δυνατή η αποθήκευση της προτίμησης",
      "saveDescription": "Η προτίμησή σας για τον έλεγχο ενημερώσεων δεν άλλαξε."
    }
  },
  "components": {
    "tooltip": {
      "dismissHint": "Πατήστε οπουδήποτε για απόρριψη"
    },
    "backgroundPosition": {
      "title": "Τοποθέτηση Φόντου",
      "instructions": "Σύρετε για τοποθέτηση • Τσιμπήστε ή κυλήστε για ζουμ"
    },
    "avatarSource": {
      "generateImage": "Δημιουργία Εικόνας",
      "generateImageDesc": "Δημιουργία avatar με AI",
      "noImageModels": "Δεν υπάρχουν διαθέσιμα μοντέλα εικόνας",
      "editCurrent": "Επεξεργασία Τρέχοντος",
      "editCurrentDesc": "Αλλαγή θέσης ή περικοπή",
      "chooseImage": "Επιλογή Εικόνας",
      "chooseImageDesc": "Επιλέξτε από τη συσκευή σας"
    },
    "avatarCurrentEdit": {
      "title": "Επεξεργασία ρεύματος",
      "reposition": "Επανατοποθέτηση",
      "repositionDesc": "Μετακινήστε ή περικόψτε το τρέχον avatar",
      "editWithAI": "Επεξεργασία με AI",
      "editWithAIDesc": "Ανοίξτε την επεξεργασία AI για το τρέχον avatar",
      "noImageModels": "Δεν υπάρχουν διαθέσιμα μοντέλα εικόνας"
    },
    "avatarGeneration": {
      "modelsLoadError": "Αποτυχία φόρτωσης μοντέλων δημιουργίας εικόνας",
      "title": "Δημιουργία Avatar",
      "help": "Βοήθεια με τη δημιουργία avatar",
      "model": "Μοντέλο",
      "selectModel": "Επιλέξτε μοντέλο",
      "describe": "Περιγράψτε το avatar σας",
      "describePlaceholder": "Ένα φιλικό κορίτσι anime με πολύχρωμα μαλλιά, χαμογελώντας ζεστά...",
      "inProgress": "Δημιουργία avatar...",
      "editingInProgress": "Εφαρμογή επεξεργασίας avatar...",
      "previousVariant": "Προηγούμενη παραλλαγή",
      "nextVariant": "Επόμενη παραλλαγή",
      "variantCounter": "{{current}} / {{total}}",
      "editRequest": "Επεξεργασία αιτήματος",
      "editRequestPlaceholder": "Κάντε τα μαλλιά πιο σκούρα, προσθέστε γυαλιά, διατηρήστε το πρόσωπο ίδιο...",
      "applyEdit": "Εφαρμογή Επεξεργασίας",
      "editImageLoadError": "Αποτυχία προετοιμασίας του avatar που δημιουργήθηκε για επεξεργασία",
      "aiAssistant": "Βοηθός AI",
      "backToResults": "Επιστροφή στην προτροπή",
      "magicInTheWorks": "Η μαγεία στα σκαριά...",
      "refine": "Διυλίζω",
      "apply": "Εφαρμόζω",
      "alt": "Δημιουργημένο avatar",
      "regenerate": "Αναδημιουργία",
      "useThis": "Χρήση Αυτού"
    },
    "avatarPosition": {
      "title": "Τοποθέτηση Avatar",
      "instructions": "Σύρετε για τοποθέτηση • Τσιμπήστε ή κυλήστε για ζουμ",
      "alt": "Avatar για τοποθέτηση"
    },
    "confirmDialog": {
      "defaultTitle": "Επιβεβαίωση",
      "defaultLabel": "Επιβεβαίωση"
    },
    "bottomMenu": {
      "defaultTitle": "Μενού",
      "dragTip": "Σύρετε για κλείσιμο μενού",
      "closeLabel": "Κλείσιμο μενού",
      "buttonProcessing": "Επεξεργασία..."
    },
    "modelSelector": {
      "placeholder": "Επιλέξτε μοντέλο",
      "clearLabel": "Χρήση καθολικής προεπιλογής",
      "loading": "Φόρτωση μοντέλων...",
      "noModels": "Δεν υπάρχουν διαθέσιμα μοντέλα",
      "addProviderFirst": "Προσθέστε πρώτα πάροχο στις ρυθμίσεις",
      "title": "Επιλογή Μοντέλου",
      "searchPlaceholder": "Αναζήτηση μοντέλων...",
      "noResults": "Δεν βρέθηκαν μοντέλα",
      "noResultsHint": "Δοκιμάστε διαφορετικό όρο αναζήτησης"
    },
    "localeSelector": {
      "title": "Επιλέξτε Γλώσσα"
    },
    "promptTemplate": {
      "nameContentRequired": "Απαιτούνται όνομα και περιεχόμενο",
      "saveError": "Αποτυχία αποθήκευσης προτύπου",
      "editTitle": "Επεξεργασία Προτροπής",
      "createTitle": "Δημιουργία Προτροπής",
      "name": "Όνομα",
      "namePlaceholder": "π.χ., Μάστερ Roleplay",
      "content": "Περιεχόμενο",
      "variablesButton": "Μεταβλητές",
      "contentPlaceholder": "Είσαι ένας χρήσιμος βοηθός AI...\n\nΧρησιμοποιήστε {{char.name}} και {{scene}} στην προτροπή σας.",
      "characterCount": "{{count}} χαρακτήρες",
      "preview": "Προεπισκόπηση",
      "characterPlaceholder": "Χαρακτήρας…",
      "personaPlaceholder": "Περσόνα…",
      "rendering": "Απόδοση…",
      "noPreview": "Χωρίς προεπισκόπηση ακόμα",
      "saving": "Αποθήκευση...",
      "update": "Ενημέρωση",
      "create": "Δημιουργία",
      "variablesTitle": "Μεταβλητές Προτύπου",
      "variablesCopyHint": "Πατήστε για αντιγραφή στο πρόχειρο",
      "variablesCopied": "Αντιγράφηκε",
      "variables": {
        "charName": "Όνομα Χαρακτήρα",
        "charNameDesc": "Όνομα του χαρακτήρα",
        "charDesc": "Περιγραφή Χαρακτήρα",
        "charDescDesc": "Περιγραφή χαρακτήρα",
        "scene": "Σκηνή",
        "sceneDesc": "Αρχική σκηνή/σενάριο",
        "userName": "Όνομα Χρήστη",
        "userNameDesc": "Όνομα περσόνας χρήστη",
        "userDesc": "Περιγραφή Χρήστη",
        "userDescDesc": "Περιγραφή περσόνας χρήστη",
        "rules": "Κανόνες",
        "rulesDesc": "Κανόνες συμπεριφοράς χαρακτήρα",
        "contextSummary": "Περίληψη Πλαισίου",
        "contextSummaryDesc": "Δυναμική περίληψη συνομιλίας",
        "keyMemories": "Βασικές Αναμνήσεις",
        "keyMemoriesDesc": "Λίστα σχετικών αναμνήσεων"
      }
    },
    "characterExport": {
      "title": "Μορφή Εξαγωγής",
      "selectFormat": "Επιλέξτε μορφή",
      "loading": "Φόρτωση μορφών...",
      "formatUecDesc": "Μορφή Unified Entity Card (.uec) (συνιστάται).",
      "formatLegacyJsonDesc": "Παλαιό JSON (μόνο εισαγωγή).",
      "formatV3Desc": "Character Card V3 JSON (τελευταία προδιαγραφή).",
      "formatV2Desc": "Character Card V2 JSON (προδιαγραφή Tavern).",
      "formatV1Desc": "Character Card V1 (μόνο εισαγωγή)."
    },
    "embeddingDownload": {
      "downloadRequired": "Απαιτείται Λήψη",
      "modelRequired": "Απαιτείται Μοντέλο Embedding",
      "description": "Η Δυναμική Μνήμη απαιτεί τη λήψη ενός τοπικού μοντέλου embedding (~260 MB) για να λειτουργήσει.",
      "localStorage": "• Το μοντέλο θα αποθηκευτεί τοπικά στη συσκευή σας",
      "downloadSize": "• Μέγεθος λήψης: περίπου 260 MB",
      "summarization": "• Απαιτείται για τη σύνοψη συνομιλιών"
    },
    "embeddingUpgrade": {
      "title": "Διαθέσιμο Μοντέλο Embedding v4",
      "v1Message": "Χρησιμοποιείτε v1 με 512 tokens. Αναβαθμίστε σε v4 για καλύτερη ποιότητα μνήμης και υποστήριξη μεγάλου πλαισίου.",
      "v2Message": "Χρησιμοποιείτε παλαιό v2. Αναβαθμίστε σε v4 για καλύτερη ποιότητα μνήμης με το πιο πρόσφατο μοντέλο embedding.",
      "v3Message": "Το v4 κυκλοφόρησε και βελτιώνει δραματικά την ανάκληση μνήμης roleplay σε σχέση με το v3 (recall@1 0.02 -> 0.92). Η αναβάθμιση συνιστάται.",
      "button": "Αναβάθμιση σε v4"
    },
    "v2UpgradeToast": {
      "title": "Μοντέλο Μνήμης v3",
      "badge": "Διαθέσιμο",
      "message": "Βελτιωμένη ποιότητα embedding σε σχέση με v2",
      "dismiss": "Απόρριψη",
      "upgrade": "Αναβάθμιση"
    },
    "v1UpgradeToast": {
      "title": "Διαθέσιμο Μοντέλο Μνήμης v3",
      "message": "Αναβαθμίστε για καλύτερη ποιότητα μνήμης και υποστήριξη μεγάλου πλαισίου.",
      "dismiss": "Απόρριψη",
      "upgrade": "Αναβάθμιση"
    },
    "createMenu": {
      "title": "Δημιουργία Νέου",
      "smartCreator": "Έξυπνος Δημιουργός",
      "smartCreatorDesc": "Αφήστε τον βοηθό να καθοδηγήσει τη δημιουργία σας",
      "divider": "Ή δημιουργήστε χειροκίνητα",
      "character": "Χαρακτήρας",
      "characterDesc": "Δημιουργήστε έναν προσαρμοσμένο χαρακτήρα",
      "persona": "Περσόνα",
      "personaDesc": "Δημιουργήστε μια επαναχρησιμοποιήσιμη φωνή",
      "groupChat": "Ομαδική Συνομιλία",
      "groupChatDesc": "Συνομιλήστε με πολλούς χαρακτήρες",
      "lorebook": "Βιβλίο Γνώσης",
      "lorebookDesc": "Χτίστε την αναφορά κόσμου σας",
      "characterSmartDesc": "Χτίστε χαρακτήρα με καθοδηγούμενη δημιουργία",
      "personaSmartDesc": "Δημιουργήστε επαναχρησιμοποιήσιμη φωνή ή προσωπικότητα",
      "lorebookSmartDesc": "Χτίστε δομημένη αναφορά κόσμου",
      "loadingConversations": "Φόρτωση συνομιλιών...",
      "createNew": "Δημιουργία νέου",
      "createNewDesc": "Ξεκινήστε νέα καθοδηγούμενη δημιουργία",
      "editExisting": "Επεξεργασία υπάρχοντος",
      "continueLast": "Συνέχεια τελευταίας συνομιλίας",
      "seeOlder": "Δείτε παλαιότερες συνομιλίες",
      "seeOlderDesc": "Ανοίξτε οποιαδήποτε προηγούμενη συνομιλία Έξυπνου Δημιουργού",
      "noConversations": "Δεν υπάρχουν συνομιλίες ακόμα για αυτόν τον δημιουργό.",
      "sessionCompleted": "Ολοκληρωμένη",
      "sessionCancelled": "Ακυρωμένη",
      "sessionDraft": "Πρόχειρο",
      "sessionMessages": "{{count}} μηνύματα",
      "untitledConversation": "Συνομιλία χωρίς τίτλο",
      "nameLorebookTitle": "Ονομασία Βιβλίου Γνώσης",
      "lorebookNamePlaceholder": "Εισάγετε όνομα βιβλίου γνώσης...",
      "lorebookImporting": "Εισαγωγή...",
      "lorebookImport": "Εισαγωγή",
      "lorebookCreating": "Δημιουργία...",
      "lorebookCreate": "Δημιουργία",
      "editExistingDesc": "Επιλέξτε ένα {{goal}} και επεξεργαστείτε το με τον Έξυπνο Δημιουργό",
      "creatorTitle": "Δημιουργός {{goal}}",
      "editTitle": "Επεξεργασία {{goal}}",
      "conversationsTitle": "Συνομιλίες {{goal}}",
      "loadingItems": "Φόρτωση {{items}}...",
      "noItemsFound": "Δεν βρέθηκαν {{items}}.",
      "unnamedCharacter": "Χαρακτήρας χωρίς όνομα",
      "untitledPersona": "Περσόνα χωρίς τίτλο",
      "untitledLorebook": "Βιβλίο γνώσης χωρίς τίτλο"
    },
    "advancedModelSettings": {
      "temperature": "Θερμοκρασία",
      "temperatureDesc": "Υψηλότερη = πιο δημιουργικό",
      "topP": "Top P",
      "topPDesc": "Χαμηλότερο = πιο εστιασμένο",
      "maxTokens": "Μέγιστα Tokens Εξόδου",
      "maxTokensDesc": "Αφήστε κενό για προεπιλογή",
      "contextLength": "Μήκος Πλαισίου",
      "contextLengthDesc": "Μόνο για τοπικά μοντέλα",
      "contextLengthAuto": "Αυτόματο",
      "frequencyPenalty": "Ποινή Συχνότητας",
      "frequencyPenaltyDesc": "Μείωση επανάληψης tokens",
      "presencePenalty": "Ποινή Παρουσίας",
      "presencePenaltyDesc": "Ενθάρρυνση νέων θεμάτων",
      "topK": "Top K",
      "topKDesc": "Περιορισμός μεγέθους δεξαμενής tokens",
      "reasoning": "Σκέψη / Συλλογισμός",
      "reasoningAutoDesc": "Αυτό το μοντέλο χρησιμοποιεί πάντα συλλογισμό. Δεν χρειάζεται ρύθμιση.",
      "reasoningEnableDesc": "Ενεργοποιήστε προηγμένες δυνατότητες σκέψης για σύνθετη επίλυση προβλημάτων.",
      "effortMode": "Λειτουργία Προσπάθειας",
      "budgetMode": "Λειτουργία Προϋπολογισμού",
      "reasoningEffort": "Προσπάθεια Συλλογισμού",
      "reasoningEffortDesc": "Ελέγχει το βάθος σκέψης",
      "reasoningEffortAuto": "Αυτόματο",
      "reasoningEffortLow": "Χαμηλό",
      "reasoningEffortMed": "Μεσαίο",
      "reasoningEffortHigh": "Υψηλό",
      "reasoningBudget": "Προϋπολογισμός Συλλογισμού (tokens)",
      "reasoningBudgetDesc": "Μέγιστα tokens για σκέψη",
      "reasoningEffortLowDesc": "Γρήγορες απαντήσεις με ελάχιστο συλλογισμό",
      "reasoningEffortMediumDesc": "Ισορροπημένο βάθος συλλογισμού",
      "reasoningEffortHighDesc": "Μέγιστο βάθος συλλογισμού για σύνθετα προβλήματα",
      "reasoningBudgetExtendedDesc": "Μέγιστα tokens για σκέψη. Προστίθενται στο όριο εξόδου."
    },
    "providerParameterSupport": {
      "unknownProvider": "Άγνωστος πάροχος: {{providerId}}",
      "reasoningNotSupportedEffort": "Δεν υποστηρίζεται - αυτός ο πάροχος δεν χρησιμοποιεί επίπεδα προσπάθειας",
      "reasoningNotSupportedBudgetOnly": "Δεν υποστηρίζεται - αυτός ο πάροχος χρησιμοποιεί μόνο προϋπολογισμό",
      "reasoningNotSupported": "Δεν υποστηρίζεται - αυτός ο πάροχος δεν υποστηρίζει συλλογισμό",
      "unsupportedParametersIgnored": "Οι μη υποστηριζόμενες παράμετροι θα αγνοηθούν από {{providerName}}.",
      "reasoningEffortSupported": "Η προσπάθεια συλλογισμού υποστηρίζεται για μοντέλα σκέψης (o1, DeepSeek-R1, κ.λπ.)",
      "reasoningBudgetSupported": "Αυτός ο πάροχος χρησιμοποιεί σκέψη βασισμένη σε προϋπολογισμό (χωρίς επίπεδα προσπάθειας). Ορίστε tokens προϋπολογισμού συλλογισμού.",
      "reasoningNotSupportedProvider": "Αυτός ο πάροχος δεν υποστηρίζει παραμέτρους συλλογισμού.",
      "matrixTitle": "Πίνακας Υποστήριξης Παραμέτρων Παρόχου",
      "providerColumn": "Πάροχος",
      "supportedIndicator": "Υποστηρίζεται από το API του παρόχου",
      "notSupportedIndicator": "Δεν υποστηρίζεται (η παράμετρος θα αγνοηθεί)"
    },
    "v3UpgradeToast": {
      "title": "Μοντέλο μνήμης v4",
      "badge": "Διαθέσιμο",
      "message": "Το v4 βελτιώνει δραματικά την ανάκληση μνήμης roleplay σε σχέση με το v3 (recall@1 0.02 -> 0.92). Η αναβάθμιση συνιστάται.",
      "dismiss": "Αργότερα",
      "upgrade": "Αναβάθμιση"
    },
    "extra": {
      "promptCachingTitle": "Αποθήκευση Προτροπής",
      "promptCachingDescription": "Επιταχύνει τη δημιουργία και μειώνει το κόστος για μεγάλα, επαναλαμβανόμενα πλαίσια (όπως μεγάλες προτροπές συστήματος ή βαθιά ιστορικά συνομιλιών).",
      "avatarAlt": "Avatar",
      "chooseFromLibrary": "Επιλογή από βιβλιοθήκη",
      "chooseFromLibraryDesc": "Χρήση εικόνας ήδη αποθηκευμένης στην εφαρμογή",
      "generateFailed": "Αποτυχία δημιουργίας εικόνας",
      "editFailed": "Αποτυχία επεξεργασίας avatar",
      "backgroundAlt": "Φόντο για τοποθέτηση",
      "formatsLoadFailed": "Αποτυχία φόρτωσης μορφών εξαγωγής",
      "formatsShowingDefaults": "(εμφάνιση προεπιλογών)",
      "timeJustNow": "μόλις τώρα",
      "timeMinutesAgo": "πριν {{minutes}} λεπτά",
      "timeHoursAgo": "πριν {{hours}} ώρες",
      "timeDaysAgo": "πριν {{days}} ημέρες",
      "removeReference": "Αφαίρεση σχεδιαστικής αναφοράς",
      "thumbLoading": "Φόρτωση...",
      "addReferences": "Προσθήκη αναφορών",
      "visualDescription": "Οπτική περιγραφή",
      "draftWithAi": "Πρόχειρο με AI",
      "regenerate": "Αναδημιουργία",
      "useThis": "Χρήση Αυτού",
      "referenceImagesLabel": "Εικόνες αναφοράς",
      "writerHelpFallback": "το συμβατό μοντέλο συγγραφής σκηνών",
      "writerHelpUses": "Χρησιμοποιεί {{model}} για πρόχειρο από το avatar και τις εικόνες αναφοράς σας.",
      "writerHelpUnavailable": "Προσθέστε συμβατό μοντέλο συγγραφής σκηνών στις ρυθμίσεις Δημιουργίας Εικόνας για αυτόματο πρόχειρο.",
      "writerNotAvailableError": "Προσθέστε πρώτα συμβατό μοντέλο συγγραφής σκηνών στις ρυθμίσεις Δημιουργίας Εικόνας.",
      "writerNoSourcesError": "Προσθέστε avatar ή τουλάχιστον μία εικόνα αναφοράς πριν τη δημιουργία.",
      "noUsableReferences": "Δεν βρέθηκαν χρήσιμες εικόνες αναφοράς.",
      "draftFailed": "Αποτυχία δημιουργίας περιγραφής σχεδίου.",
      "draftReadFailed": "Αποτυχία ανάγνωσης στοιχείου εικόνας ({{status}})",
      "draftConvertFailed": "Αποτυχία μετατροπής στοιχείου εικόνας σε URL δεδομένων",
      "draftGenerationFailed": "Αποτυχία δημιουργίας προχείρου.",
      "draftMenuTitle": "Σχεδιαστικό Πρόχειρο AI",
      "draftedBy": "Πρόχειρο από {{model}} με βάση το τρέχον avatar και τις εικόνες αναφοράς.",
      "draftedByFallback": "το μοντέλο συγγραφής σκηνών σας",
      "noWriterUseHelper": "Προσθέστε συμβατό μοντέλο συγγραφής σκηνών πριν χρησιμοποιήσετε αυτό το βοηθό.",
      "emptyReferences": "Προσθέστε μερικές σαφείς φωτογραφίες αναφοράς για να καθορίσετε πρόσωπο, αναλογίες, ενδυμασία και στυλ.",
      "designReferencesTitle": "Σχεδιαστικές αναφορές",
      "designReferencesDescription": "Ανεβάστε μερικές σαφείς εικόνες αναφοράς συν μία κανονική οπτική περιγραφή.",
      "designReferencesPlaceholder": "Περιγράψτε τη σταθερή εμφάνιση: πρόσωπο, μαλλιά, κατασκευή, παρουσίαση ηλικίας, ενδυματολογικές ενδείξεις, αξεσουάρ, και κατεύθυνση τέχνης/στυλ.",
      "dismissAria": "Απόρριψη",
      "v3MessageFallback": "Το lettuce-emb-v4 κυκλοφόρησε και βελτιώνει δραματικά την ανάκληση μνήμης roleplay. Η αναβάθμιση συνιστάται.",
      "uploadButton": "Μεταφόρτωση",
      "libraryButton": "Βιβλιοθήκη",
      "companionSetupTitle": "Ο Σύντροφος χρειάζεται ρύθμιση",
      "companionSetupSubtitleSingle": "Η λειτουργία Συντρόφου χρειάζεται ένα ακόμα μοντέλο πριν λειτουργήσει. Η παράλειψη θα επιστρέψει αυτόν τον χαρακτήρα στο Roleplay.",
      "companionSetupSubtitleMany": "Η λειτουργία Συντρόφου χρειάζεται {{count}} ακόμα μοντέλα πριν λειτουργήσει. Η παράλειψη θα επιστρέψει αυτόν τον χαρακτήρα στο Roleplay.",
      "companionSetupBody": "Η λειτουργία Συντρόφου χρειάζεται κάποια τοπικά μοντέλα για ανάλυση συναισθήματος, εξαγωγή οντοτήτων, δρομολόγηση αναμνήσεων και ανάκληση προηγούμενου πλαισίου.",
      "companionUseRoleplay": "Χρήση Roleplay αντί",
      "companionDownloadNow": "Λήψη τώρα",
      "searchModelsPlaceholder": "Αναζήτηση μοντέλων...",
      "loadingModelsDefault": "Φόρτωση μοντέλων...",
      "noModelsAvailable": "Δεν υπάρχουν διαθέσιμα μοντέλα.",
      "noModelsMatching": "Δεν βρέθηκαν μοντέλα που να ταιριάζουν με \"{{query}}\".",
      "contentPlaceholderText": "Είσαι ένας χρήσιμος βοηθός AI...\n\nΧρησιμοποιήστε {{char.name}} και {{scene}} στην προτροπή σας.",
      "previewRenderFailed": "<αποτυχία απόδοσης προεπισκόπησης>",
      "charactersCount": "{{count}} χαρακτήρες"
    }
  },
  "chats": {
    "characterNotFound": "Ο χαρακτήρας δεν βρέθηκε",
    "chatHistory": "Ιστορικό Συνομιλιών",
    "previousConversationsWithCharacter": "Προηγούμενες συνομιλίες με {{name}}",
    "previousConversations": "Προηγούμενες συνομιλίες",
    "searchChats": "Αναζήτηση συνομιλιών...",
    "searchResults": "{{count}} αποτέλεσμα(τα)",
    "noConversationsYet": "Δεν υπάρχουν συνομιλίες ακόμα",
    "startChattingPrompt": "Ξεκινήστε μια συνομιλία για να δείτε το ιστορικό σας εδώ",
    "noMatchingChats": "Δεν βρέθηκαν αντίστοιχες συνομιλίες",
    "tryDifferentSearchTerm": "Δοκιμάστε διαφορετικό όρο αναζήτησης",
    "untitledChat": "Συνομιλία Χωρίς Τίτλο",
    "chatTitlePlaceholder": "Τίτλος συνομιλίας...",
    "exportChatPackage": "Εξαγωγή Πακέτου Συνομιλίας",
    "characterSpecificExport": "Εξαγωγή Συγκεκριμένου Χαρακτήρα",
    "characterSpecificExportDesc": "Σύνδεση αυτού του πακέτου με τον χαρακτήρα συνομιλίας από προεπιλογή.",
    "nonCharacterSpecificExport": "Γενική Εξαγωγή",
    "nonCharacterSpecificExportDesc": "Απαιτεί επιλογή χαρακτήρα κατά την εισαγωγή.",
    "deleteChat": "Διαγραφή συνομιλίας;",
    "deleteConfirmDesc": "Αφαιρεί μόνιμα από το ιστορικό",
    "keepThisChat": "Διατήρηση αυτής της συνομιλίας",
    "editCharacter": "Επεξεργασία Χαρακτήρα",
    "exportCharacter": "Εξαγωγή Χαρακτήρα",
    "chatAppearance": "Εμφάνιση Συνομιλίας",
    "importChatPackage": "Εισαγωγή Πακέτου Συνομιλίας",
    "hideThisCharacter": "Απόκρυψη αυτού του χαρακτήρα",
    "deleteCharacter": "Διαγραφή Χαρακτήρα",
    "deleteCharacterTitle": "Διαγραφή Χαρακτήρα;",
    "deleteCharacterConfirmation": "Είστε σίγουροι ότι θέλετε να διαγράψετε τον \"{{name}}\"; Αυτό θα διαγράψει επίσης όλες τις συνεδρίες συνομιλίας με αυτόν τον χαρακτήρα.",
    "characterSpecificMatches": "Αυτό το πακέτο είναι συγκεκριμένο για χαρακτήρα και ταιριάζει με τον επιλεγμένο χαρακτήρα.",
    "characterSpecificMismatch": "Αυτό το πακέτο είναι συγκεκριμένο για χαρακτήρα και αναφέρεται σε άλλον χαρακτήρα. Θα εισαχθεί στον {{name}}.",
    "nonCharacterSpecificImport": "Αυτό το πακέτο δεν είναι συγκεκριμένο για χαρακτήρα. Θα εισαχθεί στον {{name}}.",
    "noCharactersYet": "Δεν υπάρχουν χαρακτήρες ακόμα",
    "createFirstCharacter": "Δημιουργήστε τον πρώτο σας χαρακτήρα από το κουμπί + παρακάτω για να ξεκινήσετε τη συνομιλία",
    "scrollToBottom": "Κύλιση στο κάτω μέρος",
    "selectCharacter": "Επιλογή Χαρακτήρα",
    "branchToGroupChat": "Διακλάδωση σε Ομαδική Συνομιλία",
    "addContent": "Προσθήκη Περιεχομένου",
    "swapPlaces": "Εναλλαγή Θέσεων",
    "swapPlacesOn": "Εναλλαγή Θέσεων (Ενεργό)",
    "uploadImage": "Μεταφόρτωση Εικόνας",
    "helpMeReply": "Βοήθησέ με να Απαντήσω",
    "sceneImage": {
      "modeTitle": "Εικόνα σκηνής",
      "modeDescription": "Επιλέξτε εάν θα γράψετε μόνοι σας το μήνυμα της σκηνής ή θα αφήσετε το AI να το σχεδιάσει πρώτα.",
      "writePrompt": "Προτροπή εγγραφής",
      "writePromptDesc": "Πληκτρολογήστε το ακριβές μήνυμα εικόνας σκηνής που θέλετε να χρησιμοποιήσετε.",
      "askAi": "Ρωτήστε το AI",
      "askAiDesc": "Αφήστε το τρέχον μοντέλο συνομιλίας να σχεδιάσει μια προτροπή σκηνής από την επιλεγμένη στιγμή.",
      "generateTitle": "Δημιουργία εικόνας σκηνής",
      "regenerateTitle": "Αναγέννηση εικόνας σκηνής",
      "aiTitle": "Προτροπή σκηνής AI",
      "promptLabel": "ΣΚΗΝΙΚΗ ΠΡΟΤΑΣΗ",
      "promptPlaceholder": "Περιγράψτε τη σκηνή, τους χαρακτήρες, τη διάθεση, τον φωτισμό, το καδράρισμα της κάμερας και σημαντικές λεπτομέρειες...",
      "suggestedPrompt": "Προτεινόμενη προτροπή",
      "regeneratePrompt": "Αναγεννηθείς",
      "editPrompt": "Επεξεργασία προτροπής",
      "reviewTitle": "Έλεγχος prompt σκηνής",
      "denyPrompt": "Απόρριψη",
      "acceptPrompt": "Αποδοχή",
      "generateImage": "Δημιουργία εικόνας",
      "updateImage": "Ενημέρωση εικόνας"
    },
    "useMyTextAsBase": "Χρήση του κειμένου μου ως βάση",
    "writeNewReply": "Γράψε κάτι νέο",
    "suggestedReply": "Προτεινόμενη Απάντηση",
    "selectTwoCharactersMinimum": "Επιλέξτε τουλάχιστον 2 χαρακτήρες για ομαδική συνομιλία.",
    "groupBranchCreated": "Η διακλάδωση ομάδας δημιουργήθηκε! Ανακατεύθυνση...",
    "memories": "Αναμνήσεις",
    "tools": "Εργαλεία",
    "pinned": "Καρφιτσωμένα",
    "searchMemories": "Αναζήτηση αναμνήσεων...",
    "addMemory": "Προσθήκη Ανάμνησης",
    "memoryActions": "Ενέργειες ανάμνησης",
    "pinnedMessages": "Καρφιτσωμένα Μηνύματα",
    "pinnedMessagesDesc": "Πάντα συμπεριλαμβάνονται στο πλαίσιο",
    "contextSummary": "Περίληψη Πλαισίου",
    "contextSummaryPlaceholder": "Σύντομη ανακεφαλαίωση για τη διατήρηση συνέπειας πλαισίου σε όλα τα μηνύματα...",
    "memoryContentPlaceholder": "Τι πρέπει να θυμόμαστε;",
    "editMemoryPlaceholder": "Εισάγετε περιεχόμενο ανάμνησης...",
    "togglePin": {
      "pin": "Καρφίτσωμα",
      "unpin": "Ξεκαρφίτσωμα"
    },
    "toggleMemoryState": {
      "setHot": "Ορισμός ως Θερμή",
      "setCold": "Ορισμός ως Ψυχρή"
    },
    "selectModelForRetry": "Επιλέξτε Μοντέλο για Επανάληψη",
    "memoryCategories": {
      "characterTrait": "χαρακτηριστικό χαρακτήρα",
      "relationship": "σχέση",
      "plotEvent": "γεγονός πλοκής",
      "worldDetail": "λεπτομέρεια κόσμου",
      "preference": "προτίμηση",
      "other": "άλλο"
    },
    "settings": {
      "memorySection": "Μνήμη",
      "memorySectionDesc": "Περίληψη, ετικέτες, ιστορικό κλήσεων εργαλείων",
      "quickSettings": "Γρήγορες Ρυθμίσεις",
      "quickSettingsDesc": "Πιο συχνές ρυθμίσεις",
      "persona": "Περσόνα",
      "model": "Μοντέλο",
      "fallbackModel": "Εναλλακτικό Μοντέλο",
      "voice": "Φωνή",
      "voiceDesc": "Αναπαραγωγή κειμένου σε ομιλία",
      "advanced": "Προχωρημένα",
      "advancedDesc": "Παράκαμψη παραμέτρων μοντέλου για αυτή τη συνεδρία",
      "session": "Συνεδρία",
      "sessionDesc": "Ξεκινήστε νέες συνομιλίες και περιηγηθείτε στο ιστορικό",
      "newChat": "Νέα Συνομιλία",
      "newChatDesc": "Ξεκινήστε μια νέα συνομιλία",
      "chatHistoryDesc": "Δείτε προηγούμενες συνεδρίες",
      "importChatPackageDesc": "Εισαγωγή .chatpkg σε αυτόν τον χαρακτήρα",
      "selectModel": "Επιλογή Μοντέλου",
      "selectFallbackModel": "Επιλογή Εναλλακτικού Μοντέλου",
      "personaActions": "Ενέργειες Περσόνας",
      "sessionAdvancedSettings": "Προχωρημένες Ρυθμίσεις Συνεδρίας",
      "parameterSupport": "Υποστήριξη Παραμέτρων",
      "backToChat": "Πίσω στη συνομιλία",
      "chatSettingsTitle": "Ρυθμίσεις Συνομιλίας",
      "chatSettingsSubtitle": "Διαχείριση προτιμήσεων συνομιλίας",
      "modelDefaults": "Προεπιλογές μοντέλου",
      "appDefaults": "Προεπιλογές εφαρμογής",
      "openChatSessionFirst": "Ανοίξτε πρώτα μια συνεδρία συνομιλίας",
      "sessionRequired": "Απαιτείται συνεδρία",
      "noPersona": "Χωρίς περσόνα",
      "customPersona": "Προσαρμοσμένη περσόνα",
      "noModelAvailable": "Δεν υπάρχει διαθέσιμο μοντέλο",
      "fallbackNone": "Κανένα",
      "unknownModel": "Άγνωστο μοντέλο",
      "authorNote": "Σημείωση Συγγραφέα",
      "identityProfileAuthored": "Προφίλ ταυτότητας συντάχθηκε",
      "addIdentityProfile": "Προσθήκη προφίλ ταυτότητας συντρόφου",
      "soulLabel": "Ψυχή",
      "sessionTitle": "Συνεδρία: {{title}}",
      "sessionUntitled": "Χωρίς τίτλο",
      "messageCount": "{{count}} μηνύματα",
      "usingCharacterDefault": "Χρήση προεπιλογής χαρακτήρα",
      "sessionOverrideActive": "Ενεργή παράκαμψη συνεδρίας",
      "autoplayVoice": "Αυτόματη αναπαραγωγή φωνής",
      "useCharacterDefault": "Χρήση προεπιλογής χαρακτήρα"
    },
    "search": {
      "placeholder": "Αναζήτηση συνομιλίας...",
      "noMessagesFound": "Δεν βρέθηκαν μηνύματα",
      "you": "Εσύ",
      "character": "Χαρακτήρας",
      "failed": "Αποτυχία αναζήτησης μηνυμάτων"
    },
    "templates": {
      "startWithTemplate": "Ξεκινήστε με πρότυπο;",
      "noTemplate": "Χωρίς πρότυπο",
      "startWithSceneOnly": "Ξεκινήστε μόνο με σκηνή",
      "you": "Εσύ",
      "bot": "Bot",
      "messageCount": "{{count}} μήνυμα(τα)"
    },
    "header": {
      "back": "Πίσω",
      "openSettings": "Άνοιγμα ρυθμίσεων συνομιλίας",
      "manageMemories": "Διαχείριση αναμνήσεων",
      "searchMessages": "Αναζήτηση μηνυμάτων",
      "manageLorebooks": "Διαχείριση βιβλίων γνώσης",
      "conversationSettings": "Ρυθμίσεις συνομιλίας"
    },
    "footer": {
      "sendMessagePlaceholder": "Στείλτε μήνυμα...",
      "stopGeneration": "Διακοπή παραγωγής",
      "sendMessage": "Αποστολή μηνύματος",
      "continueConversation": "Συνέχεια συνομιλίας",
      "moreOptions": "Περισσότερες επιλογές",
      "addImage": "Προσθήκη εικόνας",
      "addImageAttachment": "Προσθήκη συνημμένης εικόνας",
      "removeAttachment": "Αφαίρεση συνημμένου",
      "recordVoice": "Εγγραφή φωνής"
    },
    "message": {
      "thinkingMessages": {
        "thinkingReallyHard": "Σκέφτομαι πολύ σκληρά…",
        "lettuceCouncil": "Συμβουλεύομαι το συμβούλιο του μαρουλιού…",
        "stealingThoughts": "Κλέβω σκέψεις από το κενό…",
        "warmingBrainCells": "Ζεσταίνω τα εγκεφαλικά κύτταρα…",
        "forbiddenKnowledge": "Φορτώνω απαγορευμένη γνώση…",
        "overthinking": "Υπερσκέφτομαι (ως συνήθως)…",
        "pretendingToBeSmart": "Προσποιούμαι ότι είμαι έξυπνος…",
        "crunchingNumbers": "Επεξεργάζομαι φανταστικούς αριθμούς…",
        "arguingWithMyself": "Μαλώνω με τον εαυτό μου…",
        "askingUniverse": "Ρωτάω ευγενικά το σύμπαν…"
      },
      "thoughtProcess": "Διαδικασία σκέψης",
      "regenerateResponse": "Αναδημιουργία απάντησης",
      "guidedRegenerationTitle": "Καθοδήγηση αναδημιουργίας",
      "guidedRegenerationLabel": "Πώς πρέπει να αλλάξει;",
      "guidedRegenerationDescription": "Περιγράψτε τον τόνο, το μήκος, τις λεπτομέρειες που θέλετε να κρατήσετε ή να αφαιρέσετε, και οτιδήποτε πρέπει να κάνει διαφορετικά η επόμενη απάντηση.",
      "guidedRegenerationPlaceholder": "Κάντε το πιο σύντομο, πιο ζεστό, πιο άμεσο...",
      "guidedRegenerationSubmit": "Αναδημιουργία",
      "cancelAudioGeneration": "Ακύρωση δημιουργίας ήχου",
      "stopAudio": "Διακοπή ήχου",
      "playMessageAudio": "Αναπαραγωγή ήχου μηνύματος",
      "playAudio": "Αναπαραγωγή ήχου",
      "sceneLabel": "Σκηνή",
      "variantLabel": "Παραλλαγή",
      "regenerating": "Αναδημιουργία",
      "assistantIsTyping": "Ο βοηθός πληκτρολογεί",
      "attachedImage": "Συνημμένη εικόνα"
    },
    "actions": {
      "assistantMessage": "Μήνυμα Βοηθού",
      "userMessage": "Μήνυμα Χρήστη",
      "promptTokens": "Tokens Προτροπής",
      "completionTokens": "Tokens Ολοκλήρωσης",
      "fallbackModelUsed": "Χρησιμοποιήθηκε εναλλακτικό μοντέλο",
      "total": "σύνολο",
      "timeToFirstToken": "Χρόνος μέχρι πρώτο token",
      "completionTokenSpeed": "Ταχύτητα tokens ολοκλήρωσης",
      "edit": "Επεξεργασία",
      "copy": "Αντιγραφή",
      "pin": "Καρφίτσωμα",
      "unpin": "Ξεκαρφίτσωμα",
      "rewindToHere": "Επαναφορά εδώ",
      "branchFromHere": "Διακλάδωση από εδώ",
      "branchToGroupChat": "Διακλάδωση σε ομαδική συνομιλία",
      "branchToCharacter": "Διακλάδωση σε χαρακτήρα",
      "generateSceneImage": "Δημιουργία εικόνας σκηνής",
      "regenerateSceneImage": "Αναγέννηση εικόνας σκηνής",
      "chatAppearance": "Εμφάνιση Συνομιλίας",
      "delete": "Διαγραφή",
      "debug": "Αποσφαλμάτωση",
      "unpinToDelete": "Ξεκαρφιτσώστε για διαγραφή",
      "editPlaceholder": "Επεξεργαστείτε το μήνυμά σας...",
      "memoriesUsed": "{{count}} αναμνήσεις χρησιμοποιήθηκαν",
      "lorebookUsage": "Χρήση βιβλίου γνώσης",
      "lorebookUsageDesc": "Αυτή η απάντηση χρησιμοποίησε τις ακόλουθες καταχωρήσεις βιβλίου γνώσης.",
      "matchScore": "Αντιστοίχιση: {{score}}%",
      "unknownModel": "Άγνωστο μοντέλο",
      "loadingModel": "Φόρτωση μοντέλου..."
    },
    "emptyState": {
      "goBack": "Επιστροφή"
    },
    "settingsDrawer": {
      "title": "Ρυθμίσεις Συνομιλίας",
      "subtitle": "Διαχείριση προτιμήσεων συνομιλίας"
    },
    "history": {
      "archivedBadge": "Αρχειοθετημένο",
      "messagesCount": "{{count}} μηνύματα",
      "previousGroupPage": "Προηγούμενη σελίδα {{label}}",
      "nextGroupPage": "Επόμενη σελίδα {{label}}",
      "sillyTavernFormat": "Μορφή SillyTavern",
      "sillyTavernFormatDesc": "Εξαγωγή ως .jsonl συμβατό με SillyTavern.",
      "failedLoad": "Αποτυχία φόρτωσης δεδομένων",
      "failedDelete": "Αποτυχία διαγραφής: {{error}}",
      "failedRename": "Αποτυχία μετονομασίας: {{error}}",
      "chatPackageExportedTo": "Το πακέτο συνομιλίας εξήχθη σε:\n{{path}}",
      "sillyTavernExportedTo": "Η συνομιλία SillyTavern εξήχθη σε:\n{{path}}",
      "failedExportChatPackage": "Αποτυχία εξαγωγής πακέτου συνομιλίας",
      "failedExportSillyTavern": "Αποτυχία εξαγωγής συνομιλίας SillyTavern"
    },
    "authorNote": {
      "title": "Σημείωση Συγγραφέα",
      "inlineEditor": "Ενσωματωμένος επεξεργαστής",
      "inlineEditorDesc": "Εμφάνιση της σημείωσης συγγραφέα πάνω από την είσοδο συνομιλίας για γρήγορες επεξεργασίες.",
      "toggleInlineEditor": "Εναλλαγή ενσωματωμένου επεξεργαστή σημειώσεων συγγραφέα",
      "placeholder": "Ιδιωτική κατεύθυνση για αυτή τη συνομιλία",
      "willBeRemoved": "Η σημείωση συγγραφέα θα αφαιρεθεί κατά την αποθήκευση",
      "noNoteSaved": "Δεν υπάρχει αποθηκευμένη σημείωση συγγραφέα",
      "charactersCount": "{{count}} χαρακτήρες",
      "clear": "Καθαρισμός",
      "save": "Αποθήκευση",
      "saving": "Αποθήκευση...",
      "failedSave": "Αποτυχία αποθήκευσης σημείωσης συγγραφέα",
      "addPlaceholder": "Προσθέστε σημείωση συγγραφέα...",
      "savingShort": "Αποθήκευση..."
    },
    "drawer": {
      "chatSettingsTitle": "Ρυθμίσεις Συνομιλίας",
      "chatSettingsSubtitle": "Διαχείριση προτιμήσεων συνομιλίας"
    },
    "companionSoul": {
      "loading": "Φόρτωση Ψυχής Συντρόφου...",
      "unavailable": "Η Ψυχή Συντρόφου δεν είναι διαθέσιμη",
      "unavailableDesc": "Η επεξεργασία ψυχής είναι διαθέσιμη μόνο για χαρακτήρες σε λειτουργία συντρόφου.",
      "pageTitle": "Ψυχή Συντρόφου",
      "back": "Πίσω",
      "save": "Αποθήκευση"
    },
    "companionRelationship": {
      "back": "Πίσω",
      "loading": "Φόρτωση κατάστασης σχέσης...",
      "unavailableTitle": "Η κατάσταση σχέσης δεν είναι διαθέσιμη",
      "sessionLoadFailed": "Η συνεδρία συνομιλίας δεν ήταν δυνατό να φορτωθεί.",
      "backToChat": "Πίσω στη συνομιλία",
      "notCompanionTitle": "Αυτή η συνομιλία δεν είναι σε λειτουργία συντρόφου",
      "notCompanionDesc": "Οι σελίδες σχέσης συντρόφου εμφανίζονται μόνο για συνομιλίες όπου η λειτουργία χαρακτήρα είναι σύντροφος.",
      "openRegularMemories": "Άνοιγμα κανονικών αναμνήσεων",
      "pageTitle": "Κατάσταση σχέσης",
      "memoryButton": "Ανάμνηση",
      "lastInteraction": "Τελευταία αλληλεπίδραση {{time}}",
      "bond": "Δεσμός",
      "closeness": "Εγγύτητα",
      "trust": "Εμπιστοσύνη",
      "affection": "Στοργή",
      "tension": "Ένταση",
      "stability": "Σταθερότητα",
      "interactions": "Αλληλεπιδράσεις",
      "vsDefaults": "σε σχέση με τις προεπιλογές χαρακτήρα",
      "updatedAt": "Ενημερώθηκε {{time}}",
      "emotionalEngine": "Συναισθηματική μηχανή",
      "felt": "Αισθάνθηκε",
      "feltDesc": "Εσωτερική επίδραση",
      "expressed": "Εκφράστηκε",
      "expressedDesc": "Εμφανίζεται στις απαντήσεις",
      "blocked": "Αποκλεισμένο",
      "blockedDesc": "Κατεσταλμένο από περσόνα",
      "momentum": "Ορμή",
      "momentumDesc": "Τάση σε πρόσφατες σειρές",
      "activeDrivers": "Ενεργοί οδηγοί",
      "soul": "Ψυχή",
      "essence": "Ουσία",
      "voice": "Φωνή",
      "relationalStyle": "Σχεσιακό στυλ",
      "vulnerabilities": "Ευαισθησίες",
      "habits": "Συνήθειες",
      "boundaries": "Όρια",
      "eventsCount": "{{count}} γεγονότα",
      "recentTimeline": "Πρόσφατο χρονολόγιο",
      "superseded": "αντικαταστάθηκε",
      "promptScore": "Προτροπή {{score}}",
      "persistenceScore": "Επιμονή {{score}}",
      "noTimeline": "Δεν υπάρχει χρονολόγιο ακόμα",
      "noTimelineDesc": "Αναμνήσεις σχέσεων, ορόσημα και συναισθηματικά στιγμιότυπα θα εμφανιστούν εδώ καθώς ο σύντροφος μαθαίνει από τις συνομιλίες.",
      "notAuthoredYet": "Δεν έχει συνταχθεί ακόμα.",
      "noSignal": "Κανένα σήμα."
    },
    "companionUi": {
      "relationship": "Σχέση",
      "milestones": "Ορόσημα",
      "boundaries": "Όρια",
      "preferences": "Προτιμήσεις",
      "profile": "Προφίλ",
      "routines": "Ρουτίνες",
      "episodes": "Επεισόδια",
      "emotionalSnapshots": "Συναισθηματικά στιγμιότυπα",
      "unknownTime": "Άγνωστο",
      "justNow": "Μόλις τώρα",
      "minutesAgo": "Πριν {{minutes}} λεπτά",
      "hoursAgo": "Πριν {{hours}} ώρες",
      "daysAgo": "Πριν {{days}} ημέρες",
      "notSetYet": "Δεν έχει οριστεί ακόμα",
      "missingCharacterId": "Λείπει το characterId",
      "sessionNotFound": "Η συνεδρία δεν βρέθηκε",
      "failedLoadCompanion": "Αποτυχία φόρτωσης συνεδρίας συντρόφου"
    },
    "chatPage": {
      "characterNotFound": "Ο χαρακτήρας δεν βρέθηκε",
      "characterDoesntExist": "Ο χαρακτήρας που ψάχνετε δεν υπάρχει."
    },
    "debugPage": {
      "copy": "Αντιγραφή"
    },
    "companionMemoryPage": {
      "backLabel": "Πίσω",
      "refineMemoryPlaceholder": "Βελτιώστε την αποθηκευμένη ανάμνηση συντρόφου",
      "superseded": "αντικαταστάθηκε",
      "pinned": "καρφιτσωμένο",
      "cold": "ψυχρό"
    }
  },
  "groupChats": {
    "list": {
      "editGroup": "Επεξεργασία Ομάδας",
      "deleteGroup": "Διαγραφή Ομάδας",
      "deleteConfirmTitle": "Διαγραφή Ομαδικής Συνομιλίας;",
      "deleteConfirmMessage": "Είστε σίγουροι ότι θέλετε να διαγράψετε τον \"{{name}}\"; Αυτό θα διαγράψει επίσης όλα τα μηνύματα σε αυτή την ομαδική συνομιλία.",
      "noGroupChatsYet": "Δεν υπάρχουν ομαδικές συνομιλίες ακόμα",
      "noGroupChatsDesc": "Δημιουργήστε την πρώτη σας ομαδική συνομιλία από το κουμπί + παρακάτω για συνομιλίες με πολλούς χαρακτήρες ταυτόχρονα",
      "newChat": "Νέα συνομιλία",
      "openChat": "Άνοιγμα συνομιλίας",
      "chatSettings": "Ρυθμίσεις συνομιλίας",
      "sessionCount": "{{count}} συνομιλίες"
    },
    "create": {
      "invalidPackage": "Αυτό το πακέτο δεν είναι πακέτο ομαδικής συνομιλίας.",
      "inspectPackageError": "Αποτυχία επιθεώρησης πακέτου ομαδικής συνομιλίας",
      "importPackageError": "Αποτυχία εισαγωγής πακέτου ομαδικής συνομιλίας",
      "importChatpkg": "Εισαγωγή `.chatpkg`",
      "mapParticipantsTitle": "Αντιστοίχιση Συμμετεχόντων",
      "selectLocalCharacter": "Επιλέξτε τον τοπικό χαρακτήρα για αυτόν τον συμμετέχοντα.",
      "selectCharacterPlaceholder": "Επιλέξτε χαρακτήρα...",
      "importChatPackageTitle": "Εισαγωγή Πακέτου Συνομιλίας",
      "importChatPackageDesc": "Αυτό θα εισάγει το επιλεγμένο `.chatpkg` ως νέα ομαδική συνεδρία.",
      "characterSelect": {
        "title": "Δημιουργία Ομαδικής Συνομιλίας",
        "subtitle": "Επιλέξτε χαρακτήρες για την ομαδική σας συνομιλία",
        "selected": "επιλεγμένοι",
        "ready": "Έτοιμο",
        "minRequired": "Ελάχ. 2 απαιτούνται",
        "noCharactersYet": "Δεν υπάρχουν χαρακτήρες ακόμα",
        "noCharactersDesc": "Δημιουργήστε πρώτα κάποιους χαρακτήρες για ομαδική συνομιλία",
        "continueToSetup": "Συνέχεια στη Ρύθμιση Ομάδας"
      },
      "groupSetup": {
        "title": "Ρύθμιση Ομάδας",
        "subtitle": "Διαμορφώστε τις ρυθμίσεις ομαδικής συνομιλίας",
        "chatType": "Τύπος Συνομιλίας",
        "conversation": "Συνομιλία",
        "casualChat": "Χαλαρή συνομιλία",
        "roleplay": "Roleplay",
        "withScenes": "Με σκηνές",
        "conversationDesc": "Χαλαρή ομαδική συνομιλία χωρίς αρχικές σκηνές",
        "roleplayDesc": "Σενάριο roleplay με αρχική σκηνή και εμβυθιστικές προτροπές",
        "speakerSelection": "Επιλογή Ομιλητή",
        "llm": "LLM",
        "aiPicks": "Επιλέγει η AI",
        "heuristic": "Ευρετικό",
        "scoreBased": "Βάσει βαθμολογίας",
        "roundRobin": "Κυκλική Σειρά",
        "takeTurns": "Εκ περιτροπής",
        "llmDesc": "Χρησιμοποιεί το προεπιλεγμένο μοντέλο σας για να επιλέξει ποιος μιλάει (κοστίζει tokens)",
        "heuristicDesc": "Χρησιμοποιεί ισορροπία συμμετοχής και ενδείξεις πλαισίου (δωρεάν)",
        "roundRobinDesc": "Οι χαρακτήρες μιλούν με τη σειρά (δωρεάν)",
        "chatBackground": "Φόντο Συνομιλίας",
        "optional": "(Προαιρετικό)",
        "uploadBackground": "Μεταφόρτωση εικόνας φόντου",
        "backgroundDesc": "Ορίστε εικόνα φόντου για αυτή την ομαδική συνομιλία",
        "groupName": "Όνομα Ομάδας",
        "removeBackground": "Αφαίρεση εικόνας φόντου",
        "groupNameAutoGenerate": "Αφήστε κενό για αυτόματη δημιουργία από ονόματα χαρακτήρων",
        "continueToScene": "Συνέχεια στην Αρχική Σκηνή",
        "createGroupChat": "Δημιουργία Ομαδικής Συνομιλίας"
      },
      "startingScene": {
        "title": "Αρχική Σκηνή",
        "subtitle": "Ορίστε το εναρκτήριο σενάριο για το roleplay σας",
        "sceneSource": "Πηγή Σκηνής",
        "none": "Καμία",
        "custom": "Προσαρμοσμένη",
        "fromCharacter": "Από Χαρακτήρα",
        "noneDesc": "Ξεκινήστε χωρίς προκαθορισμένη σκηνή",
        "customDesc": "Γράψτε το δικό σας εναρκτήριο σενάριο",
        "fromCharacterDesc": "Χρησιμοποιήστε σκηνή από έναν χαρακτήρα σας",
        "sceneContent": "Περιεχόμενο Σκηνής",
        "sceneContentPlaceholder": "Περιγράψτε την αρχική σκηνή για αυτό το roleplay...",
        "sceneReferenceTip": "Συμβουλή: Πληκτρολογήστε {{@\" για αναφορά χαρακτήρων",
        "selectScene": "Επιλέξτε Σκηνή",
        "sceneLabel": "Σκηνή του",
        "copyToCustom": "Αντιγραφή σε Προσαρμοσμένη & Επεξεργασία"
      }
    },
    "history": {
      "title": "Ιστορικό Ομαδικών Συνομιλιών",
      "subtitle": "Όλες οι ομαδικές συνομιλίες",
      "searchPlaceholder": "Αναζήτηση ομαδικών συνομιλιών...",
      "active": "Ενεργές ({{count}})",
      "archived": "Αρχειοθετημένες ({{count}})",
      "noChatsYet": "Δεν υπάρχουν ομαδικές συνομιλίες ακόμα",
      "noChatsDesc": "Δημιουργήστε μια ομαδική συνομιλία για να δείτε το ιστορικό σας εδώ",
      "noMatchingChats": "Δεν βρέθηκαν αντίστοιχες συνομιλίες",
      "noMatchingDesc": "Δοκιμάστε διαφορετικό όρο αναζήτησης",
      "deleteSessionTitle": "Διαγραφή ομαδικής συνομιλίας;",
      "deleteSessionDesc": "Αφαιρεί μόνιμα από το ιστορικό",
      "deleteSessionButton": "Διαγραφή συνομιλίας",
      "keepChat": "Διατήρηση αυτής της συνομιλίας"
    },
    "session": {
      "chatTitlePlaceholder": "Τίτλος συνομιλίας...",
      "newChat": "Νέα Συνομιλία",
      "rename": "Μετονομασία",
      "unarchive": "Αναίρεση αρχειοθέτησης",
      "archive": "Αρχειοθέτηση",
      "messageCount": "{{count}} μήνυμα(τα)"
    },
    "memories": {
      "tabMemories": "Αναμνήσεις",
      "tabPinned": "Καρφιτσωμένα",
      "tabActivity": "Δραστηριότητα",
      "processing": "Επεξεργασία",
      "contextSummaryTitle": "Περίληψη Πλαισίου",
      "addContextSummaryPrompt": "Πατήστε για προσθήκη περίληψης πλαισίου...",
      "savedMemories": "Αποθηκευμένες Αναμνήσεις",
      "resultsCount": "Αποτελέσματα ({{count}})",
      "searchPlaceholder": "Αναζήτηση αναμνήσεων...",
      "addMemory": "Προσθήκη ανάμνησης",
      "noMemoriesYet": "Δεν υπάρχουν αναμνήσεις ακόμα",
      "noMemoriesDesc": "Πατήστε το κουμπί Προσθήκη παραπάνω για να δημιουργήσετε μια",
      "noMatchingMemories": "Δεν βρέθηκαν αντίστοιχες αναμνήσεις",
      "noMatchingDesc": "Δοκιμάστε διαφορετικό όρο αναζήτησης",
      "sessionNotFound": "Η συνεδρία δεν βρέθηκε",
      "memoryActions": "Ενέργειες ανάμνησης",
      "tokens": "tokens",
      "cycle": "Κύκλος",
      "accessed": "Πρόσβαση",
      "cold": "Ψυχρή",
      "hot": "Θερμή",
      "activityLog": "Αρχείο Δραστηριότητας",
      "events": "συμβάντα",
      "run": "Εκτέλεση",
      "processingMemories": "Η AI οργανώνει τις αναμνήσεις ομάδας...",
      "memoryCycleSuccess": "Ο κύκλος μνήμης ολοκληρώθηκε επιτυχώς!",
      "memoryActionFailed": "Η ενέργεια μνήμης απέτυχε",
      "newMemoryUpdates": "Νέες ενημερώσεις μνήμης διαθέσιμες",
      "noActivityYet": "Δεν υπάρχει δραστηριότητα ακόμα",
      "noActivityDesc": "Οι κλήσεις εργαλείων εμφανίζονται όταν η AI διαχειρίζεται αναμνήσεις σε δυναμική λειτουργία",
      "contextSummaryPlaceholder": "Σύντομη ανακεφαλαίωση για διατήρηση συνέπειας πλαισίου σε όλα τα μηνύματα...",
      "addMemoryTitle": "Προσθήκη Ανάμνησης",
      "memoryPlaceholder": "Τι πρέπει να θυμόμαστε;",
      "saveMemory": "Αποθήκευση Ανάμνησης",
      "editMemoryTitle": "Επεξεργασία Ανάμνησης",
      "editMemoryPlaceholder": "Εισάγετε περιεχόμενο ανάμνησης...",
      "edit": "Επεξεργασία",
      "pin": "Καρφίτσωμα",
      "unpin": "Ξεκαρφίτσωμα",
      "setHot": "Ορισμός ως Θερμή",
      "setCold": "Ορισμός ως Ψυχρή"
    },
    "toolLog": {
      "created": "Δημιουργήθηκε",
      "deleted": "Διαγράφηκε",
      "pinned": "Καρφιτσώθηκε",
      "unpinned": "Ξεκαρφιτσώθηκε",
      "done": "Τέλος",
      "pinnedBadge": "καρφιτσωμένο",
      "softDelete": "ήπια διαγραφή",
      "memoryCycle": "Κύκλος Μνήμης",
      "failedAt": "Απέτυχε στο:",
      "window": "Παράθυρο",
      "justNow": "μόλις τώρα",
      "minutesAgo": "{{count}}λ πριν",
      "hoursAgo": "{{count}}ω πριν",
      "yesterday": "χθες",
      "daysAgo": "{{count}}η πριν",
      "revertingTitle": "Αναστροφή...",
      "revertCycleTitle": "Αναστροφή αυτού του κύκλου",
      "revertedAt": "Αναστράφηκε {{time}}",
      "failedAtStage": "Απέτυχε στο: {{stage}}",
      "hideDebug": "Απόκρυψη Debug",
      "debug": "Debug",
      "windowRange": "Παράθυρο {{start}}-{{end}}",
      "actionCreated": "Δημιουργήθηκε",
      "actionDeleted": "Διαγράφηκε",
      "actionPinned": "Καρφιτσώθηκε",
      "actionUnpinned": "Ξεκαρφιτσώθηκε",
      "actionDone": "Τέλος",
      "badgePinned": "καρφιτσωμένο",
      "badgeSoftDelete": "ήπια διαγραφή",
      "badgeUndone": "αναιρέθηκε",
      "badgeReverted": "αναστράφηκε",
      "activityEmptyTitle": "Δεν υπάρχει δραστηριότητα ακόμα",
      "activityEmptyDesc": "Οι κλήσεις εργαλείων εμφανίζονται όταν η AI διαχειρίζεται αναμνήσεις σε δυναμική λειτουργία"
    },
    "message": {
      "thinkingHard": "Σκέφτομαι πολύ σκληρά…",
      "thinkingLettuce": "Συμβουλεύομαι το συμβούλιο του μαρουλιού…",
      "thinkingVoid": "Κλέβω σκέψεις από το κενό…",
      "thinkingBrainCells": "Ζεσταίνω τα εγκεφαλικά κύτταρα…",
      "thinkingForbidden": "Φορτώνω απαγορευμένη γνώση…",
      "thinkingOverthinking": "Υπερσκέφτομαι (ως συνήθως)…",
      "thinkingPretending": "Προσποιούμαι ότι είμαι έξυπνος…",
      "thinkingCrunching": "Επεξεργάζομαι φανταστικούς αριθμούς…",
      "thinkingArguing": "Μαλώνω με τον εαυτό μου…",
      "thinkingUniverse": "Ρωτάω ευγενικά το σύμπαν…",
      "thoughtProcess": "Διαδικασία σκέψης",
      "userAlt": "Χρήστης",
      "assistantAlt": "Βοηθός",
      "regenerateResponse": "Αναδημιουργία απάντησης",
      "variantLabel": "Παραλλαγή",
      "regenerating": "Αναδημιουργία",
      "cancelAudioGeneration": "Ακύρωση δημιουργίας ήχου",
      "stopAudio": "Διακοπή ήχου",
      "playMessageAudio": "Αναπαραγωγή ήχου μηνύματος",
      "playAudio": "Αναπαραγωγή ήχου",
      "attachedImage": "Συνημμένη εικόνα",
      "assistantIsTyping": "Ο βοηθός πληκτρολογεί",
      "assistantTyping": "Ο βοηθός πληκτρολογεί"
    },
    "header": {
      "back": "Πίσω",
      "memories": "Αναμνήσεις",
      "settings": "Ρυθμίσεις",
      "characters": "χαρακτήρες"
    },
    "footer": {
      "mentionCharacter": "Αναφορά χαρακτήρα",
      "noCharactersFound": "Δεν βρέθηκαν χαρακτήρες",
      "moreOptions": "Περισσότερες επιλογές",
      "addImage": "Προσθήκη εικόνας",
      "messagePlaceholder": "Μήνυμα... (@ για αναφορά)",
      "stopGeneration": "Διακοπή παραγωγής",
      "sendMessage": "Αποστολή μηνύματος",
      "continueConversation": "Συνέχεια συνομιλίας",
      "dismissError": "Απόρριψη σφάλματος",
      "removeAttachment": "Αφαίρεση συνημμένου",
      "tabToSelect": "Tab για επιλογή"
    },
    "messageActions": {
      "characterMessage": "Μήνυμα Χαρακτήρα",
      "yourMessage": "Το Μήνυμά Σας",
      "whyCharacterResponded": "Γιατί απάντησε αυτός ο χαρακτήρας",
      "total": "σύνολο",
      "edit": "Επεξεργασία",
      "copy": "Αντιγραφή",
      "regenerateWithDifferent": "Αναδημιουργία με διαφορετικό χαρακτήρα",
      "rewindToHere": "Επαναφορά εδώ",
      "unpinToDelete": "Ξεκαρφιτσώστε για διαγραφή",
      "delete": "Διαγραφή",
      "editPlaceholder": "Επεξεργαστείτε το μήνυμά σας...",
      "chooseCharacterTitle": "Επιλέξτε Χαρακτήρα",
      "selectCharacterForRegeneration": "Επιλέξτε ποιος χαρακτήρας θα απαντήσει:"
    },
    "settings": {
      "appDefault": "Προεπιλογή εφαρμογής",
      "selectPersona": "Επιλογή Περσόνας",
      "noPersonas": "Δεν υπάρχουν διαθέσιμες περσόνες",
      "noPersonasDesc": "Δημιουργήστε μια περσόνα στις ρυθμίσεις για να εξατομικεύσετε τις συνομιλίες σας.",
      "searchPersonas": "Αναζήτηση περσονών...",
      "noPersona": "Χωρίς Περσόνα",
      "noPersonaDesc": "Συνέχεια χωρίς περσόνα",
      "noPersonasFound": "Δεν βρέθηκαν περσόνες",
      "noPersonasFoundDesc": "Δοκιμάστε διαφορετικό όρο αναζήτησης"
    },
    "groupSettings": {
      "title": "Επεξεργασία ομάδας",
      "subtitle": "Ενημέρωση προεπιλεγμένων ρυθμίσεων για μελλοντικές συνεδρίες",
      "enterGroupName": "Εισάγετε όνομα ομάδας",
      "participant": "συμμετέχων",
      "participants": "συμμετέχοντες",
      "uploading": "Μεταφόρτωση...",
      "changeBackground": "Αλλαγή φόντου",
      "addBackgroundImage": "Προσθήκη εικόνας φόντου",
      "removeBackground": "Αφαίρεση φόντου",
      "persona": "Περσόνα",
      "personaSubtitle": "Προεπιλεγμένη περσόνα για νέες συνεδρίες",
      "personaLabel": "Περσόνα",
      "speakerSelection": "Επιλογή ομιλητή",
      "speakerSubtitle": "Προεπιλεγμένη μέθοδος για νέες συνεδρίες",
      "llm": "LLM",
      "aiPicks": "Επιλογή ΤΝ",
      "heuristic": "Ευρετικό",
      "scoreBased": "Βάσει βαθμολογίας",
      "roundRobin": "Κυκλική σειρά",
      "takeTurns": "Εναλλαγή",
      "llmDesc": "Χρησιμοποιεί το προεπιλεγμένο μοντέλο σου για επιλογή ομιλητή (κοστίζει tokens)",
      "heuristicDesc": "Χρησιμοποιεί ισορροπία συμμετοχής και ενδείξεις πλαισίου (δωρεάν)",
      "roundRobinDesc": "Οι χαρακτήρες μιλούν με τη σειρά (δωρεάν)",
      "memoryMode": "Λειτουργία μνήμης",
      "memorySubtitle": "Προεπιλεγμένη λειτουργία μνήμης για νέες συνεδρίες",
      "manual": "Χειροκίνητη",
      "manualDesc": "Διαχείριση σημειώσεων μόνος",
      "dynamic": "Δυναμική",
      "dynamicDesc": "Αυτόματη ανάκληση",
      "memoryDynamicInfo": "Η ΤΝ δημιουργεί και ανακτά αυτόματα αναμνήσεις από συνομιλίες",
      "memoryManualInfo": "Εσύ προσθέτεις και διαχειρίζεσαι τις σημειώσεις μνήμης",
      "characters": "Χαρακτήρες",
      "participantsActive": "{{total}} συμμετέχοντες · {{active}} ενεργοί",
      "add": "Προσθήκη",
      "muted": "(σίγαση)",
      "mutedByDefault": "Σίγαση εξ ορισμού",
      "activeByDefault": "Ενεργός εξ ορισμού",
      "unmuteCharacter": "Κατάργηση σίγασης χαρακτήρα",
      "muteCharacter": "Σίγαση χαρακτήρα",
      "minTwoRequired": "Απαιτούνται τουλάχιστον 2 χαρακτήρες",
      "removeCharacter": "Αφαίρεση χαρακτήρα",
      "groupMinCharacters": "Μια ομάδα απαιτεί τουλάχιστον 2 χαρακτήρες",
      "mutedCharactersNote": "Οι χαρακτήρες σε σίγαση παραλείπονται από την αυτόματη επιλογή ομιλητή, αλλά μπορούν ακόμα να απαντήσουν μέσω ρητής `@αναφοράς`.",
      "addCharacterTitle": "Προσθήκη χαρακτήρα",
      "allCharactersInGroup": "Όλοι οι χαρακτήρες είναι ήδη σε αυτήν την ομάδα.",
      "removeCharacterTitle": "Αφαίρεση χαρακτήρα;",
      "removeCharacterConfirm": "Είστε σίγουροι ότι θέλετε να αφαιρέσετε τον",
      "removeCharacterFrom": "από τις προεπιλογές ομάδας;",
      "removing": "Αφαίρεση...",
      "remove": "Αφαίρεση"
    },
    "sessionSettings": {
      "subtitle": "Διαχείριση ρυθμίσεων ομαδικής συνομιλίας",
      "enterGroupName": "Εισάγετε όνομα ομάδας",
      "participant": "συμμετέχων",
      "participants": "συμμετέχοντες",
      "message": "μήνυμα",
      "messages": "μηνύματα",
      "uploading": "Μεταφόρτωση...",
      "changeBackground": "Αλλαγή φόντου",
      "addBackgroundImage": "Προσθήκη εικόνας φόντου",
      "removeBackground": "Αφαίρεση φόντου",
      "persona": "Περσόνα",
      "personaSubtitle": "Η ταυτότητά σου σε αυτήν τη συνομιλία",
      "personaLabel": "Περσόνα",
      "speakerSelection": "Επιλογή ομιλητή",
      "speakerSubtitle": "Πώς επιλέγεται ο επόμενος ομιλητής",
      "llm": "LLM",
      "aiPicks": "Επιλογή ΤΝ",
      "heuristic": "Ευρετικό",
      "scoreBased": "Βάσει βαθμολογίας",
      "roundRobin": "Κυκλική σειρά",
      "takeTurns": "Εναλλαγή",
      "llmDesc": "Χρησιμοποιεί το προεπιλεγμένο μοντέλο σου για επιλογή ομιλητή (κοστίζει tokens)",
      "heuristicDesc": "Χρησιμοποιεί ισορροπία συμμετοχής και ενδείξεις πλαισίου (δωρεάν)",
      "roundRobinDesc": "Οι χαρακτήρες μιλούν με τη σειρά (δωρεάν)",
      "characters": "Χαρακτήρες",
      "participantsActive": "{{total}} συμμετέχοντες · {{active}} ενεργοί",
      "add": "Προσθήκη",
      "muted": "(σίγαση)",
      "unmuteCharacter": "Κατάργηση σίγασης χαρακτήρα",
      "muteCharacter": "Σίγαση χαρακτήρα",
      "minTwoRequired": "Απαιτούνται τουλάχιστον 2 χαρακτήρες",
      "removeCharacter": "Αφαίρεση χαρακτήρα",
      "groupMinCharacters": "Μια ομαδική συνομιλία απαιτεί τουλάχιστον 2 χαρακτήρες",
      "mutedCharactersNote": "Οι χαρακτήρες σε σίγαση παραλείπονται από την αυτόματη επιλογή ομιλητή, αλλά μπορούν ακόμα να απαντήσουν μέσω ρητής `@αναφοράς`.",
      "data": "Δεδομένα",
      "dataSubtitle": "Εξαγωγή ή εισαγωγή συνομιλιών",
      "export": "Εξαγωγή",
      "exportDesc": "Αποθήκευση ως κοινοποιήσιμο αρχείο",
      "import": "Εισαγωγή",
      "importDesc": "Φόρτωση συνομιλίας από αρχείο",
      "conversation": "Συνομιλία",
      "conversationSubtitle": "Αντιγραφή ή συνέχιση σε νέα συνομιλία",
      "duplicate": "Αντιγραφή",
      "duplicateDesc": "Αντιγραφή αυτής της συνομιλίας με ή χωρίς μηνύματα",
      "branchTo1on1": "Διακλάδωση σε 1-προς-1",
      "branchTo1on1Desc": "Συνέχιση ιδιωτικά με έναν χαρακτήρα",
      "participation": "Συμμετοχή",
      "participationSubtitle": "Κατανομή ομιλίας μεταξύ χαρακτήρων",
      "addCharacterTitle": "Προσθήκη χαρακτήρα",
      "allCharactersInGroup": "Όλοι οι χαρακτήρες είναι ήδη σε αυτήν την ομάδα.",
      "removeCharacterTitle": "Αφαίρεση χαρακτήρα;",
      "removeCharacterConfirm": "Είστε σίγουροι ότι θέλετε να αφαιρέσετε τον",
      "removeCharacterFrom": "από αυτήν την ομαδική συνομιλία;",
      "removing": "Αφαίρεση...",
      "remove": "Αφαίρεση",
      "cloneGroupTitle": "Κλωνοποίηση ομάδας",
      "withMessages": "Με μηνύματα",
      "withMessagesDesc": "Κλωνοποίηση όλων συμπεριλαμβανομένου του ιστορικού",
      "withoutMessages": "Χωρίς μηνύματα",
      "withoutMessagesDesc": "Κλωνοποίηση μόνο ρυθμίσεων (χαρακτήρες, αρχική σκηνή)",
      "branchWithCharacterTitle": "Διακλάδωση με χαρακτήρα",
      "branchWithCharacterDesc": "Επιλέξτε χαρακτήρα για συνέχιση ως 1-προς-1 συνομιλία. Όλα τα μηνύματα αυτής της ομάδας θα μετατραπούν.",
      "continueWith": "Συνέχιση συνομιλίας με {{name}}",
      "exportChatPackageTitle": "Εξαγωγή πακέτου συνομιλίας",
      "includeCharacterSnapshots": "Συμπερίληψη στιγμιοτύπων χαρακτήρων",
      "includeCharacterSnapshotsDesc": "Διατήρηση δεδομένων χαρακτήρων στο πακέτο",
      "sessionOnly": "Μόνο συνεδρία",
      "sessionOnlyDesc": "Εξαγωγή μόνο μηνυμάτων και μεταδεδομένων",
      "mapParticipantsTitle": "Αντιστοίχιση συμμετεχόντων",
      "selectLocalCharacter": "Επιλέξτε τον τοπικό χαρακτήρα για αυτόν τον συμμετέχοντα.",
      "selectCharacterPlaceholder": "Επιλογή χαρακτήρα...",
      "continue": "Συνέχεια",
      "importChatPackageTitle": "Εισαγωγή πακέτου συνομιλίας",
      "importChatPackageDesc": "Αυτό θα εισάγει το επιλεγμένο `.chatpkg` ως νέα ομαδική συνεδρία.",
      "importing": "Εισαγωγή..."
    },
    "page": {
      "selectingCharacter": "Επιλογή χαρακτήρα...",
      "sessionNotFound": "Η ομαδική συνεδρία δεν βρέθηκε",
      "backToGroupChats": "Πίσω στις Ομαδικές Συνομιλίες",
      "startConversation": "Ξεκινήστε συνομιλία με {{names}}",
      "scrollToBottom": "Κύλιση προς τα κάτω",
      "addContent": "Προσθήκη Περιεχομένου",
      "uploadImage": "Ανέβασμα Εικόνας",
      "helpMeReply": "Βοήθησέ με να Απαντήσω",
      "helpMeReplyDesc": "Άσε την AI να προτείνει τι να πεις",
      "haveDraftPrompt": "Έχετε πρόχειρο μήνυμα. Πώς θέλετε να συνεχίσετε;",
      "useMyTextAsBase": "Χρήση του κειμένου μου ως βάση",
      "useMyTextAsBaseDesc": "Επέκτεινε και βελτίωσε το πρόχειρό σου",
      "writeSomethingNew": "Γράψε κάτι καινούριο",
      "writeSomethingNewDesc": "Δημιούργησε νέα φρέσκια απάντηση",
      "suggestedReply": "Προτεινόμενη Απάντηση",
      "reasoningBeforeWriting": "Σκέψη πριν τη σύνταξη απάντησης...",
      "writingYourReply": "Σύνταξη απάντησής σας...",
      "regenerate": "Αναδημιουργία",
      "useReply": "Χρήση Απάντησης",
      "helpMeReplyFailedGeneric": "Το «Βοήθησέ με να Απαντήσω» απέτυχε.",
      "helpMeReplyFailedGenerate": "Το «Βοήθησέ με να Απαντήσω» απέτυχε να δημιουργήσει απάντηση.",
      "noAudioCaptured": "Δεν καταγράφηκε ήχος.",
      "noWhisperModel": "Δεν βρέθηκε εγκατεστημένο μοντέλο Whisper. Εγκαταστήστε ένα στις ρυθμίσεις Αναγνώρισης Ομιλίας.",
      "cancelRecording": "Ακύρωση εγγραφής",
      "transcribing": "Μεταγραφή",
      "stopAndTranscribe": "Διακοπή και μεταγραφή",
      "recordVoice": "Εγγραφή φωνής",
      "learnCorrection": "Εκμάθηση διόρθωσης:",
      "learning": "Εκμάθηση...",
      "learn": "Εκμάθηση",
      "ignore": "Αγνόηση",
      "groupChatFailed": "Η ομαδική συνομιλία απέτυχε.",
      "failedToCopy": "Αποτυχία αντιγραφής",
      "copied": "Αντιγράφηκε!",
      "cannotDeletePinned": "Δεν είναι δυνατή η διαγραφή καρφιτσωμένου μηνύματος. Ξεκαρφιτσώστε το πρώτα.",
      "failedToDelete": "Αποτυχία διαγραφής",
      "messageNotFound": "Δεν βρέθηκε μήνυμα",
      "cannotRewindPinned": "Αδύνατη η επαναφορά: υπάρχουν καρφιτσωμένα μηνύματα μετά από αυτό το σημείο. Ξεκαρφιτσώστε τα πρώτα.",
      "failedToRewind": "Αποτυχία επαναφοράς",
      "failedToTogglePin": "Αποτυχία εναλλαγής καρφίτσας",
      "messagePinned": "Το μήνυμα καρφιτσώθηκε",
      "messageUnpinned": "Το μήνυμα ξεκαρφιτσώθηκε",
      "failedToSave": "Αποτυχία αποθήκευσης"
    },
    "memoriesPage": {
      "summarizingConversation": "Σύνοψη συνομιλίας",
      "analyzingMemories": "Ανάλυση αναμνήσεων",
      "applyingChanges": "Εφαρμογή αλλαγών",
      "organizingMemories": "Οργάνωση αναμνήσεων",
      "retryingMemoryCycle": "Επανάληψη Κύκλου Μνήμης...",
      "processingMemories": "Επεξεργασία αναμνήσεων...",
      "memorySystemError": "Σφάλμα Συστήματος Μνήμης",
      "contextSummary": "Περίληψη Πλαισίου",
      "contextSummaryTitle": "Περίληψη Πλαισίου",
      "savedMemories": "Αποθηκευμένες Αναμνήσεις",
      "resultsCount": "Αποτελέσματα ({{count}})",
      "searchMemoriesPlaceholder": "Αναζήτηση αναμνήσεων...",
      "addMemory": "Προσθήκη ανάμνησης",
      "memoryActions": "Ενέργειες ανάμνησης",
      "clearSearch": "Εκκαθάριση αναζήτησης",
      "noMatchingMemories": "Δεν βρέθηκαν αντίστοιχες αναμνήσεις",
      "noMemoriesYet": "Δεν υπάρχουν αναμνήσεις ακόμα",
      "tryDifferentSearch": "Δοκιμάστε διαφορετικό όρο αναζήτησης",
      "tapAddToCreate": "Πατήστε το κουμπί Προσθήκη παραπάνω για να δημιουργήσετε μία",
      "pinnedMessages": "Καρφιτσωμένα Μηνύματα",
      "refresh": "Ανανέωση",
      "noPinnedMessages": "Δεν υπάρχουν καρφιτσωμένα μηνύματα",
      "pinImportantDesc": "Καρφιτσώστε σημαντικά μηνύματα ομαδικής συνομιλίας για να τα διατηρείτε πάντα στο πλαίσιο.",
      "assistant": "Βοηθός",
      "user": "Χρήστης",
      "unpin": "Ξεκαρφίτσωμα",
      "failedToUnpinMessage": "Αποτυχία ξεκαρφιτσώματος μηνύματος",
      "activityLog": "Αρχείο Δραστηριότητας",
      "run": "Εκτέλεση",
      "addMemoryTitle": "Προσθήκη Ανάμνησης",
      "editMemoryTitle": "Επεξεργασία Ανάμνησης",
      "memoryTitle": "Ανάμνηση",
      "memoryPlaceholder": "Τι πρέπει να θυμόμαστε;",
      "saveMemory": "Αποθήκευση Ανάμνησης",
      "editMemoryPlaceholder": "Εισάγετε περιεχόμενο ανάμνησης...",
      "saving": "Αποθήκευση...",
      "save": "Αποθήκευση",
      "cancel": "Ακύρωση",
      "contextSummaryPlaceholder": "Σύντομη ανακεφαλαίωση για διατήρηση συνέπειας πλαισίου σε όλα τα μηνύματα...",
      "contextSummaryPrompt": "Πατήστε για προσθήκη περίληψης πλαισίου...",
      "cycle": "Κύκλος",
      "accessed": "Πρόσβαση",
      "cold": "Ψυχρή",
      "hot": "Θερμή",
      "tokens": "tokens",
      "pin": "Καρφίτσωμα",
      "setHot": "Ορισμός ως Θερμή",
      "setCold": "Ορισμός ως Ψυχρή",
      "edit": "Επεξεργασία",
      "delete": "Διαγραφή",
      "failedToToggleMemPin": "Αποτυχία εναλλαγής καρφίτσας",
      "failedToRemoveMemory": "Αποτυχία αφαίρεσης ανάμνησης",
      "toolEventsCountAria": "συμβάντα",
      "activityEmptyDesc": "Οι κλήσεις εργαλείων εμφανίζονται όταν η AI διαχειρίζεται αναμνήσεις σε δυναμική λειτουργία",
      "memoryCycleSuccess": "Ο κύκλος μνήμης ολοκληρώθηκε επιτυχώς!"
    },
    "messageActionsExtra": {
      "characterMessage": "Μήνυμα Χαρακτήρα",
      "yourMessage": "Το Μήνυμά Σας",
      "whyCharacterResponded": "Γιατί απάντησε αυτός ο χαρακτήρας",
      "promptTokensTitle": "Tokens Προτροπής",
      "completionTokensTitle": "Tokens Ολοκλήρωσης",
      "total": "σύνολο",
      "regenerateWithDifferent": "Αναδημιουργία με διαφορετικό χαρακτήρα",
      "unpin": "Ξεκαρφίτσωμα",
      "pin": "Καρφίτσωμα",
      "rewindToHere": "Επαναφορά εδώ",
      "unpinToDelete": "Ξεκαρφιτσώστε για διαγραφή",
      "editPlaceholder": "Επεξεργαστείτε το μήνυμά σας...",
      "chooseCharacter": "Επιλέξτε Χαρακτήρα",
      "selectCharacterForRegeneration": "Επιλέξτε ποιος χαρακτήρας θα απαντήσει:"
    },
    "timeAgo": {
      "justNow": "Μόλις τώρα",
      "minutesAgo": "{{count}}λ πριν",
      "hoursAgo": "{{count}}ω πριν",
      "daysAgo": "{{count}}η πριν"
    },
    "memoriesController": {
      "missingSessionId": "Λείπει το sessionId",
      "sessionNotFound": "Η συνεδρία δεν βρέθηκε",
      "failedToLoadSession": "Αποτυχία φόρτωσης συνεδρίας",
      "failedToUpdateTemperature": "Αποτυχία ενημέρωσης θερμοκρασίας μνήμης",
      "failedToSaveSummary": "Αποτυχία αποθήκευσης σύνοψης",
      "cycleFailed": "Αποτυχία κύκλου μνήμης ομάδας",
      "failedToAddMemory": "Αποτυχία προσθήκης ανάμνησης",
      "failedToUpdateMemory": "Αποτυχία ενημέρωσης ανάμνησης",
      "failedToRunCycle": "Αποτυχία εκτέλεσης κύκλου μνήμης",
      "failedToRevertCycle": "Αποτυχία αναστροφής κύκλου",
      "failedToRefresh": "Αποτυχία ανανέωσης",
      "failedToDismissError": "Αποτυχία απόρριψης σφάλματος",
      "failedToTogglePinned": "Αποτυχία εναλλαγής καρφιτσωμένου μηνύματος",
      "sessionNotLoaded": "Η συνεδρία δεν φορτώθηκε",
      "revertCycleTitle": "Αναστροφή αυτού του κύκλου;",
      "revertConfirm": "Αναστροφή"
    },
    "chatController": {
      "sessionNotFound": "Η ομαδική συνεδρία δεν βρέθηκε",
      "failedToLoadGroupChat": "Αποτυχία φόρτωσης ομαδικής συνομιλίας",
      "requestFailed": "Το αίτημα ομαδικής συνομιλίας απέτυχε",
      "failedToSendMessage": "Αποτυχία αποστολής μηνύματος",
      "failedToRegenerate": "Αποτυχία αναδημιουργίας",
      "failedToContinue": "Αποτυχία συνέχισης",
      "failedToCopy": "Αποτυχία αντιγραφής",
      "cannotDeletePinned": "Δεν είναι δυνατή η διαγραφή καρφιτσωμένου μηνύματος. Ξεκαρφιτσώστε το πρώτα.",
      "failedToDelete": "Αποτυχία διαγραφής",
      "messageNotFound": "Δεν βρέθηκε μήνυμα",
      "cannotRewindPinned": "Αδύνατη η επαναφορά: υπάρχουν καρφιτσωμένα μηνύματα μετά από αυτό το σημείο. Ξεκαρφιτσώστε τα πρώτα.",
      "failedToRewind": "Αποτυχία επαναφοράς",
      "failedToTogglePin": "Αποτυχία εναλλαγής καρφίτσας",
      "messagePinned": "Το μήνυμα καρφιτσώθηκε",
      "messageUnpinned": "Το μήνυμα ξεκαρφιτσώθηκε",
      "failedToSave": "Αποτυχία αποθήκευσης",
      "copied": "Αντιγράφηκε!"
    },
    "historyController": {
      "failedToLoadData": "Αποτυχία φόρτωσης δεδομένων",
      "failedToDelete": "Αποτυχία διαγραφής: {{error}}",
      "failedToRename": "Αποτυχία μετονομασίας: {{error}}",
      "failedToArchive": "Αποτυχία αρχειοθέτησης: {{error}}",
      "failedToUnarchive": "Αποτυχία αναίρεσης αρχειοθέτησης: {{error}}",
      "failedToDuplicate": "Αποτυχία αντιγραφής"
    },
    "sessionSettingsController": {
      "failedToLoad": "Αποτυχία φόρτωσης ρυθμίσεων ομαδικής συνομιλίας",
      "noPersona": "Χωρίς περσόνα",
      "customPersona": "Προσαρμοσμένη περσόνα",
      "minOneActive": "Τουλάχιστον ένας συμμετέχων πρέπει να παραμείνει ενεργός"
    },
    "groupSettingsController": {
      "notFound": "Η ομάδα δεν βρέθηκε",
      "failedToLoad": "Αποτυχία φόρτωσης ρυθμίσεων ομάδας"
    },
    "createForm": {
      "failedToLoadCharacters": "Αποτυχία φόρτωσης χαρακτήρων",
      "selectAtLeastTwo": "Παρακαλώ επιλέξτε τουλάχιστον 2 χαρακτήρες για ομαδική συνομιλία",
      "failedToCreate": "Αποτυχία δημιουργίας ομαδικής συνομιλίας"
    },
    "groupSetupExtra": {
      "memoryMode": "Λειτουργία Μνήμης",
      "manual": "Χειροκίνητη",
      "manualDesc": "Διαχείριση σημειώσεων μόνος",
      "dynamic": "Δυναμική",
      "dynamicDesc": "Αυτόματες συνοψίσεις & ανάκληση",
      "memoryManualInfo": "Εσείς προσθέτετε και διαχειρίζεστε τις σημειώσεις μνήμης",
      "memoryDynamicInfo": "Η AI δημιουργεί και ανακτά αυτόματα αναμνήσεις από συνομιλίες",
      "backgroundPreviewAlt": "Προεπισκόπηση φόντου"
    },
    "createExtra": {
      "enterGroupNamePlaceholder": "Εισάγετε όνομα ομάδας...",
      "unknown": "Άγνωστο"
    },
    "startingSceneExtra": {
      "insertAs": "Εισαγωγή ως {{snippet}}"
    },
    "sessionCardExtra": {
      "unknown": "Άγνωστο",
      "untitledChat": "Συνομιλία χωρίς τίτλο"
    },
    "sessionListExtra": {
      "unknown": "Άγνωστο"
    },
    "chatHeaderExtra": {
      "unknownError": "Άγνωστο σφάλμα"
    },
    "chatSettingsExtra": {
      "invalidPackage": "Αυτό το πακέτο δεν είναι πακέτο ομαδικής συνομιλίας.",
      "failedExport": "Αποτυχία εξαγωγής πακέτου ομαδικής συνομιλίας",
      "failedInspect": "Αποτυχία επιθεώρησης πακέτου ομαδικής συνομιλίας",
      "failedImport": "Αποτυχία εισαγωγής πακέτου ομαδικής συνομιλίας",
      "exportSuccess": "Το πακέτο ομαδικής συνομιλίας εξήχθη στη διαδρομή:\n{{path}}",
      "backgroundAlt": "Φόντο",
      "removeBackgroundAria": "Αφαίρεση φόντου",
      "backAria": "Πίσω",
      "backToGroupChats": "Πίσω στις Ομαδικές Συνομιλίες"
    },
    "groupSettingsExtra": {
      "backToGroups": "Πίσω στις Ομάδες",
      "backAria": "Πίσω",
      "removeBackgroundAria": "Αφαίρεση φόντου"
    },
    "historyPage": {
      "backAria": "Πίσω στις ομαδικές συνομιλίες",
      "clearSearchAria": "Εκκαθάριση αναζήτησης",
      "resultsLabel": "{{count}} αποτέλεσμα",
      "resultsLabelPlural": "{{count}} αποτελέσματα",
      "untitledChat": "Συνομιλία χωρίς τίτλο",
      "noPinnedMessages": "Δεν υπάρχουν καρφιτσωμένα μηνύματα"
    },
    "personaSelectorExtra": {
      "insertAs": "Εισαγωγή ως",
      "appDefault": "Προεπιλογή εφαρμογής",
      "defaultBadge": "Προεπιλογή",
      "selectPersonaTitle": "Επιλογή Περσόνας",
      "noPersonaTitle": "Χωρίς Περσόνα",
      "noPersonaDesc": "Συνέχεια χωρίς περσόνα",
      "noPersonasAvailable": "Δεν υπάρχουν διαθέσιμες περσόνες",
      "noPersonasDesc": "Δημιουργήστε περσόνα στις ρυθμίσεις για να εξατομικεύσετε τις συνομιλίες σας.",
      "searchPersonas": "Αναζήτηση περσονών...",
      "noPersonasFound": "Δεν βρέθηκαν περσόνες",
      "tryDifferentSearch": "Δοκιμάστε διαφορετικό όρο αναζήτησης"
    },
    "footerExtra": {
      "cancelRecording": "Ακύρωση εγγραφής",
      "transcribing": "Μεταγραφή",
      "stopAndTranscribe": "Διακοπή και μεταγραφή",
      "recordVoice": "Εγγραφή φωνής",
      "attachmentAltDefault": "Συνημμένο"
    },
    "pageExtra": {
      "noAudioCaptured": "Δεν καταγράφηκε ήχος.",
      "noWhisperModelInstalled": "Δεν βρέθηκε εγκατεστημένο μοντέλο Whisper. Εγκαταστήστε ένα στις ρυθμίσεις Αναγνώρισης Ομιλίας.",
      "scrollToBottomAria": "Κύλιση προς τα κάτω"
    },
    "addContentMenu": {
      "title": "Προσθήκη Περιεχομένου",
      "uploadImage": "Ανέβασμα Εικόνας"
    },
    "helpMeReplyMenu": {
      "title": "Βοήθησέ με να Απαντήσω",
      "helpMeReplyDesc": "Άσε την AI να προτείνει τι να πεις",
      "draftPrompt": "Έχετε πρόχειρο μήνυμα. Πώς θέλετε να συνεχίσετε;",
      "useTextAsBase": "Χρήση του κειμένου μου ως βάση",
      "useTextAsBaseDesc": "Επέκτεινε και βελτίωσε το πρόχειρό σου",
      "writeSomethingNew": "Γράψε κάτι καινούριο",
      "writeSomethingNewDesc": "Δημιούργησε νέα φρέσκια απάντηση"
    },
    "suggestedReplyMenu": {
      "title": "Προτεινόμενη Απάντηση",
      "reasoningBeforeWriting": "Σκέψη πριν τη σύνταξη απάντησης...",
      "writingYourReply": "Σύνταξη απάντησής σας...",
      "regenerate": "Αναδημιουργία",
      "useReply": "Χρήση Απάντησης"
    },
    "memoriesPageExtra": {
      "sessionNotFound": "Η συνεδρία δεν βρέθηκε",
      "memorySystemError": "Σφάλμα Συστήματος Μνήμης",
      "retryingMemoryCycle": "Επανάληψη Κύκλου Μνήμης...",
      "processingMemories": "Επεξεργασία αναμνήσεων...",
      "memoryCycleSuccess": "Ο κύκλος μνήμης ολοκληρώθηκε επιτυχώς!",
      "contextSummaryTitle": "Περίληψη Πλαισίου",
      "activityTabLabel": "Δραστηριότητα",
      "noMatchingMemoriesDesc": "Δοκιμάστε διαφορετικό όρο αναζήτησης",
      "chatHistoryTitle": "Ιστορικό Συνομιλίας",
      "chatHistoryDesc": "Προβολή και διαχείριση συνομιλιών"
    },
    "settingsPageExtra": {
      "quickActionsSection": "Γρήγορες Ενέργειες",
      "chatHistoryTitle": "Ιστορικό Συνομιλίας",
      "chatHistoryDesc": "Προβολή και διαχείριση συνομιλιών",
      "lorebrooksTitle": "Βιβλία Γνώσης",
      "lorebrooksDesc": "Επισυνάψτε βιβλία γνώσης συνεδρίας και προαιρετικά αγνοήστε τα βιβλία γνώσης κάθε χαρακτήρα.",
      "manageLorebooks": "Διαχείριση βιβλίων γνώσης"
    },
    "groupSettingsPageExtra": {
      "lorebrooksTitle": "Βιβλία Γνώσης",
      "lorebrooksDesc": "Επισυνάψτε κοινά βιβλία γνώσης και ελέγξτε αν τα βιβλία γνώσης χαρακτήρων εφαρμόζονται εξ ορισμού.",
      "manageLorebooks": "Διαχείριση βιβλίων γνώσης"
    }
  },
  "characters": {
    "empty": {
      "title": "Δεν υπάρχουν χαρακτήρες ακόμα",
      "description": "Δημιουργήστε προσαρμοσμένους χαρακτήρες AI με μοναδικές προσωπικότητες",
      "createButton": "Δημιουργία Χαρακτήρα"
    },
    "progress": {
      "identity": "Ταυτότητα",
      "scenes": "Σκηνές",
      "details": "Λεπτομέρειες"
    },
    "identity": {
      "title": "Δημιουργία Χαρακτήρα",
      "subtitle": "Δώστε στον χαρακτήρα AI σας μια ταυτότητα",
      "tapCameraToAdd": "Πατήστε κάμερα για προσθήκη ή δημιουργία avatar",
      "importingAvatar": "Εισαγωγή avatar...",
      "characterName": "Όνομα Χαρακτήρα *",
      "characterNamePlaceholder": "Εισάγετε όνομα χαρακτήρα...",
      "characterNameDesc": "Αυτό το όνομα θα εμφανίζεται στις συνομιλίες",
      "avatarGradient": "Ντεγκραντέ Avatar",
      "avatarGradientDesc": "Δημιουργία δυναμικών ντεγκραντέ από τα χρώματα του avatar",
      "chatBackgroundLabel": "Φόντο Συνομιλίας",
      "optionalSuffix": "(Προαιρετικό)",
      "backgroundPreviewAlt": "Προεπισκόπηση φόντου",
      "backgroundPreviewBadge": "Προεπισκόπηση Φόντου",
      "addBackground": "Προσθήκη Φόντου",
      "addBackgroundHint": "Ανεβάστε ένα ή επιλέξτε από τη βιβλιοθήκη",
      "uploadImage": "Ανέβασμα εικόνας",
      "chooseFromLibrary": "Επιλογή από βιβλιοθήκη",
      "backgroundDesc": "Προαιρετική εικόνα φόντου για τις συνομιλίες",
      "continueToDescription": "Συνέχεια στην Περιγραφή",
      "importCharacterFromFile": "Εισαγωγή Χαρακτήρα από Αρχείο",
      "importCharacterDesc": "Φόρτωση χαρακτήρα από PNG κάρτα, .uec ή αρχείο εξαγωγής .json"
    },
    "details": {
      "title": "Λεπτομέρειες Χαρακτήρα",
      "subtitle": "Ορίστε προσωπικότητα και συμπεριφορά",
      "definition": "Ορισμός *",
      "wordCount": "{{count}} λέξη(εις)",
      "definitionPlaceholder": "Περιγράψτε προσωπικότητα, στυλ ομιλίας, ιστορικό, τομείς γνώσης...",
      "definitionDesc": "Να είστε συγκεκριμένοι σχετικά με τον τόνο, τα χαρακτηριστικά και το στυλ συνομιλίας",
      "availablePlaceholders": "Διαθέσιμα Σύμβολα:"
    },
    "scenes": {
      "title": "Αρχικές Σκηνές",
      "subtitle": "Δημιουργήστε εναρκτήρια σενάρια για τις συνομιλίες σας",
      "default": "Προεπιλογή",
      "hasSceneDirection": "Έχει κατεύθυνση σκηνής",
      "continueToScenes": "Συνέχεια στις Αρχικές Σκηνές"
    },
    "extras": {
      "title": "Επιπλέον Λεπτομέρειες",
      "subtitle": "Όλα τα πεδία είναι προαιρετικά",
      "nickname": "Ψευδώνυμο",
      "nicknamePlaceholder": "Πώς πρέπει ο χρήστης να αποκαλεί αυτόν τον χαρακτήρα;",
      "nicknameDesc": "Εναλλακτικό εμφανιζόμενο όνομα στις συνομιλίες",
      "creator": "Δημιουργός",
      "creatorPlaceholder": "Όνομα δημιουργού...",
      "tags": "Ετικέτες",
      "tagsPlaceholder": "φαντασία, sci-fi, ρομαντικό...",
      "tagsDesc": "Λίστα διαχωρισμένη με κόμματα για φιλτράρισμα και οργάνωση",
      "creatorNotes": "Σημειώσεις Δημιουργού",
      "creatorNotesPlaceholder": "Συμβουλές χρήσης, πλαίσιο γνώσης ή οδηγίες για άλλους χρήστες...",
      "multilingualNotes": "Πολύγλωσσες Σημειώσεις",
      "multilingualNotesHint": "Αντικείμενο JSON με κωδικούς γλώσσας ως κλειδιά",
      "creatingCharacter": "Δημιουργία Χαρακτήρα..."
    },
    "preview": {
      "unnamedCharacter": "Χαρακτήρας Χωρίς Όνομα",
      "sceneCount": "{{count}} σκηνή(ές)",
      "customPrompt": "Προσαρμοσμένη προτροπή",
      "description": "Περιγραφή",
      "startingScene": "Αρχική Σκηνή",
      "gradientEnabled": "Ντεγκραντέ ενεργοποιημένο",
      "customModel": "Προσαρμοσμένο μοντέλο",
      "avatarAlt": "Avatar χαρακτήρα",
      "characterFallback": "Χαρακτήρας"
    },
    "personaPreview": {
      "unnamedPersona": "Περσόνα Χωρίς Όνομα",
      "noDescription": "Χωρίς περιγραφή ακόμα",
      "default": "Προεπιλογή",
      "description": "Περιγραφή"
    },
    "lorebookPreview": {
      "untitledLorebook": "Βιβλίο Γνώσης Χωρίς Τίτλο",
      "entryCount": "{{count}} καταχώρηση(εις)",
      "entries": "Καταχωρήσεις",
      "noEntriesYet": "Δεν υπάρχουν καταχωρήσεις ακόμα",
      "untitledEntry": "Καταχώρηση χωρίς τίτλο",
      "noContentYet": "Χωρίς περιεχόμενο ακόμα",
      "alwaysActive": "Πάντα ενεργό",
      "moreEntries": "+{{count}} επιπλέον καταχωρήσεις",
      "moreEntry": "+{{count}} επιπλέον καταχώρηση"
    },
    "creationHelper": {
      "moreOptions": "Περισσότερες επιλογές",
      "describePlaceholder": "Περιγράψτε τον χαρακτήρα σας...",
      "stopGeneration": "Διακοπή παραγωγής",
      "sendMessage": "Αποστολή μηνύματος",
      "addToMessage": "Προσθήκη στο Μήνυμα",
      "uploadImageDesc": "Προσθέστε avatar ή εικόνα αναφοράς",
      "referenceCharacterDesc": "Χρησιμοποιήστε υπάρχοντα χαρακτήρα ως έμπνευση",
      "referencePersonaDesc": "Χρησιμοποιήστε την περσόνα σας ως πλαίσιο",
      "retry": "Επανάληψη",
      "attachmentAlt": "Συνημμένο",
      "removeAttachment": "Αφαίρεση συνημμένου",
      "removeReference": "Αφαίρεση αναφοράς",
      "uploadImageTitle": "Ανέβασμα Εικόνας",
      "referenceCharacterTitle": "Χαρακτήρας Αναφοράς",
      "referencePersonaTitle": "Περσόνα Αναφοράς"
    },
    "lorebook": {
      "keywords": "ΛΕΞΕΙΣ-ΚΛΕΙΔΙΑ",
      "caseSensitive": "Διάκριση πεζών-κεφαλαίων",
      "typeKeyword": "Πληκτρολογήστε λέξη-κλειδί...",
      "addButton": "Προσθήκη",
      "untitledEntry": "Καταχώρηση Χωρίς Τίτλο",
      "alwaysActive": "Πάντα ενεργό",
      "noKeywords": "Χωρίς λέξεις-κλειδιά",
      "dragToReorder": "Σύρετε για αναδιάταξη",
      "disabled": "Ανενεργό",
      "noLorebooksYet": "Δεν υπάρχουν βιβλία γνώσης ακόμα",
      "createLorebookDesc": "Δημιουργήστε βιβλίο γνώσης για να προσθέσετε γνώσεις κόσμου για αυτόν τον χαρακτήρα",
      "createLorebook": "Δημιουργία Βιβλίου Γνώσης",
      "searchPlaceholder": "Αναζήτηση βιβλίων γνώσης...",
      "noMatchingLorebooks": "Δεν βρέθηκαν αντίστοιχα βιβλία γνώσης",
      "activeLorebooks": "Ενεργά Βιβλία Γνώσης",
      "enabledForCharacter": "ενεργοποιημένο για αυτόν τον χαρακτήρα",
      "enabledForGroup": "ενεργοποιημένο για αυτή την ομάδα",
      "enabledForSession": "ενεργοποιημένο για αυτή τη συνεδρία",
      "tapToViewEntries": "Πατήστε για προβολή καταχωρήσεων",
      "newLorebookTitle": "Νέο Βιβλίο Γνώσης",
      "nameLabel": "ΟΝΟΜΑ",
      "enterNamePlaceholder": "Εισάγετε όνομα βιβλίου γνώσης...",
      "lorebookExplanation": "Τα βιβλία γνώσης περιέχουν καταχωρήσεις που εισάγονται στις προτροπές όταν οι λέξεις-κλειδιά ταιριάζουν.",
      "viewEntries": "Προβολή Καταχωρήσεων",
      "editEntriesDesc": "Επεξεργασία καταχωρήσεων βιβλίου γνώσης",
      "disableForCharacter": "Απενεργοποίηση για Χαρακτήρα",
      "enableForCharacter": "Ενεργοποίηση για Χαρακτήρα",
      "disableForGroup": "Απενεργοποίηση για Ομάδα",
      "enableForGroup": "Ενεργοποίηση για Ομάδα",
      "disableForSession": "Απενεργοποίηση για Συνεδρία",
      "enableForSession": "Ενεργοποίηση για Συνεδρία",
      "removeFromActive": "Αφαίρεση από τα ενεργά βιβλία γνώσης αυτού του χαρακτήρα",
      "addToActive": "Προσθήκη στα ενεργά βιβλία γνώσης αυτού του χαρακτήρα",
      "removeFromActiveGroup": "Αφαίρεση από τα ενεργά βιβλία γνώσης αυτής της ομάδας",
      "addToActiveGroup": "Προσθήκη στα ενεργά βιβλία γνώσης αυτής της ομάδας",
      "removeFromActiveSession": "Αφαίρεση από τα ενεργά βιβλία γνώσης αυτής της συνεδρίας",
      "addToActiveSession": "Προσθήκη στα ενεργά βιβλία γνώσης αυτής της συνεδρίας",
      "deleteConfirmTitle": "Διαγραφή βιβλίου γνώσης;",
      "deleteConfirmMessage": "Διαγραφή αυτού του βιβλίου γνώσης; Όλες οι καταχωρήσεις θα χαθούν.",
      "deleteLorebook": "Διαγραφή Βιβλίου Γνώσης",
      "noEntriesYet": "Δεν υπάρχουν καταχωρήσεις ακόμα",
      "addEntriesToInject": "Προσθέστε καταχωρήσεις για εισαγωγή γνώσης στις συνομιλίες σας",
      "createEntry": "Δημιουργία Καταχώρησης",
      "searchEntries": "Αναζήτηση καταχωρήσεων...",
      "noMatchingEntries": "Δεν βρέθηκαν αντίστοιχες καταχωρήσεις",
      "entryDefaultName": "Καταχώρηση",
      "editEntry": "Επεξεργασία Καταχώρησης",
      "editEntryDesc": "Τροποποίηση τίτλου, λέξεων-κλειδιών και περιεχομένου",
      "disableEntry": "Απενεργοποίηση Καταχώρησης",
      "enableEntry": "Ενεργοποίηση Καταχώρησης",
      "entryDisabledDesc": "Η καταχώρηση δεν θα εισαχθεί στις προτροπές",
      "entryEnabledDesc": "Η καταχώρηση θα εισαχθεί όταν ταιριάζουν οι λέξεις-κλειδιά",
      "deleteEntry": "Διαγραφή Καταχώρησης",
      "titleLabel": "ΤΙΤΛΟΣ",
      "titlePlaceholder": "Ονομάστε αυτή την καταχώρηση...",
      "enabled": "Ενεργό",
      "includeInPrompts": "Συμπερίληψη στις προτροπές",
      "alwaysOn": "Πάντα Ενεργό",
      "noKeywordsNeeded": "Δεν χρειάζονται λέξεις-κλειδιά",
      "contentLabel": "ΠΕΡΙΕΧΟΜΕΝΟ",
      "contentPlaceholder": "Γράψτε το πλαίσιο γνώσης εδώ...",
      "saveEntry": "Αποθήκευση Καταχώρησης",
      "noCharacterId": "Δεν δόθηκε αναγνωριστικό χαρακτήρα",
      "sectionActive": "Ενεργά",
      "sectionAvailable": "Διαθέσιμα",
      "entryCount": "{{count}} καταχωρήσεις",
      "keywordDetectionMode": "ΑΝΙΧΝΕΥΣΗ ΛΕΞΕΩΝ-ΚΛΕΙΔΙΩΝ",
      "keywordDetectionRecentWindow": "Πρόσφατα 10 μηνύματα",
      "keywordDetectionRecentWindowDesc": "Αντιστοίχιση με τα τελευταία 10 μηνύματα συνομιλίας.",
      "keywordDetectionLatestUser": "Μόνο το τελευταίο μήνυμα χρήστη",
      "keywordDetectionLatestUserDesc": "Αντιστοίχιση μόνο με το πιο πρόσφατο μήνυμα του χρήστη.",
      "preview": {
        "title": "Προεπισκόπηση Ενεργοποίησης",
        "openButton": "Προεπισκόπηση",
        "missingContext": "Δεν έχει επιλεγεί βιβλίο γνώσης.",
        "noEntries": "Προσθέστε καταχωρήσεις σε αυτό το βιβλίο γνώσης για να δείτε τι θα ενεργοποιηθεί.",
        "modeRecentShort": "Πρόσφατα {{count}}",
        "modeLatestUserShort": "Τελευταίος χρήστης",
        "inWindow": "{{count}} στο παράθυρο",
        "tabSession": "Συνεδρία",
        "tabCompose": "Σύνθεση",
        "activeStat": "{{active}} / {{total}} ενεργά",
        "tokensStat": "{{count}} tokens",
        "sessionPickerLabel": "Συνεδρίες",
        "sessionMeta": "{{count}} μνμ",
        "noSessions": "Δεν υπάρχουν συνεδρίες συνομιλίας ακόμα.",
        "noSessionsHint": "Επιλέξτε συνεδρία",
        "noMessages": "Αυτή η συνεδρία δεν έχει μηνύματα ακόμα.",
        "scanHeaderRecent": "Σάρωση {{shown}} από τα τελευταία {{depth}} μηνύματα",
        "scanHeaderLatest": "Σάρωση του τελευταίου μηνύματος χρήστη",
        "matchCount": "{{hits}} αποτέλεσμα · {{entries}} καταχωρήσεις",
        "emptyMessage": "(κενό)",
        "roleUser": "Χρήστης",
        "roleAssistant": "Βοηθός",
        "roleScene": "Σκηνή",
        "roleSystem": "Σύστημα",
        "composeHeader": "Πρόχειρο",
        "composeMatches": "{{count}} αντιστοιχίες",
        "activeLabel": "{{count}} ενεργά",
        "composePlaceholder": "Πληκτρολογήστε ή επικολλήστε κείμενο για δοκιμή αντιστοίχισης λέξεων-κλειδιών...\n\nπ.χ.\nΗ βιβλιοθήκη ήταν ήσυχη, μόνο ο βόμβος των παλιών θερμαντών.\nΡώτησε αν είχα διαβάσει το βιβλίο που μου είχε δανείσει την περασμένη εβδομάδα.",
        "sectionActive": "Ενεργά · {{count}}",
        "sectionInactive": "Ανενεργά · {{count}}",
        "statusMatched": "Ταίριαξε",
        "statusAlways": "Πάντα",
        "statusDisabled": "Ανενεργό",
        "statusNoKeywords": "Χωρίς κλειδιά",
        "statusNotMatched": "Δεν ταίριαξε",
        "tokensShort": "{{count}}t",
        "injectionTitle": "Τελική εισαγωγή",
        "injectionEmpty": "Δεν υπάρχουν ενεργές καταχωρήσεις. Τίποτα δεν θα εισαχθεί.",
        "copy": "Αντιγραφή",
        "copyFailed": "Αποτυχία αντιγραφής στο πρόχειρο.",
        "saveFailed": "Αποτυχία αποθήκευσης καταχώρησης.",
        "deleteFailed": "Αποτυχία διαγραφής καταχώρησης.",
        "deleteConfirmTitle": "Είστε σίγουροι;",
        "deleteConfirmMessage": "Διαγραφή του \"{{title}}\"; Αυτό δεν μπορεί να αναιρεθεί.",
        "contextMenuTitle": "{{count}} καταχωρήσεις χρησιμοποιούν αυτό"
      }
    },
    "templates": {
      "characterNotFound": "Ο χαρακτήρας δεν βρέθηκε",
      "templateCount": "{{count}} πρότυπο(α)",
      "newTemplate": "Νέο Πρότυπο",
      "noTemplatesYet": "Δεν υπάρχουν πρότυπα ακόμα",
      "explanation": "Τα πρότυπα συνομιλίας σας επιτρέπουν να ξεκινάτε συνομιλίες με προγραμμένα μηνύματα από εσάς και τον {{name}}.",
      "createTemplate": "Δημιουργία Προτύπου",
      "messageCount": "{{count}} μήνυμα(τα)",
      "deleteTemplate": "Διαγραφή προτύπου",
      "contextMenuFallbackTitle": "Πρότυπο",
      "importedToast": {
        "title": "Εισήχθη",
        "message": "Προστέθηκε το \"{{name}}\"."
      },
      "importFailed": "Αποτυχία εισαγωγής",
      "exportFailed": "Αποτυχία εξαγωγής"
    },
    "templateEditor": {
      "noScene": "Χωρίς σκηνή",
      "untitled": "Χωρίς τίτλο",
      "dragMessage": "Σύρετε μήνυμα",
      "editMessage": "Επεξεργασία μηνύματος",
      "deleteMessage": "Διαγραφή μηνύματος",
      "writeMessagePlaceholder": "Γράψτε περιεχόμενο μηνύματος...",
      "characterNotFound": "Ο χαρακτήρας δεν βρέθηκε",
      "scene": "Σκηνή",
      "noMessagesYet": "Δεν υπάρχουν μηνύματα ακόμα",
      "addMessagesDesc": "Προσθέστε μηνύματα για να χτίσετε ένα εναρκτήριο συνομιλίας με τον {{name}}.",
      "addMessage": "Προσθήκη Μηνύματος",
      "name": "Όνομα",
      "nameExample": "π.χ. Χαλαρός Χαιρετισμός",
      "startingScene": "Αρχική Σκηνή",
      "systemPrompt": "Προτροπή Συστήματος",
      "characterDefault": "Προεπιλογή χαρακτήρα",
      "nextMessageAs": "Επόμενο μήνυμα ως",
      "messages": "Μηνύματα",
      "roles": "Ρόλοι",
      "hoverTip": "Τοποθετήστε τον δείκτη στα μηνύματα για σύρσιμο, επεξεργασία ή διαγραφή.",
      "footerTip": "Χρησιμοποιήστε τη γραμμή υποσέλιδου για να προσθέσετε νέα μηνύματα στη συνομιλία.",
      "templateNamePlaceholder": "Όνομα προτύπου...",
      "selectScene": "Επιλογή Σκηνής",
      "startWithoutScene": "Ξεκινήστε χωρίς μήνυμα σκηνής",
      "selectSystemPrompt": "Επιλογή Προτροπής Συστήματος",
      "useCharacterSystemPrompt": "Χρήση προτροπής συστήματος χαρακτήρα"
    },
    "referenceSelector": {
      "selectCharacter": "Επιλογή Χαρακτήρα",
      "selectPersona": "Επιλογή Περσόνας",
      "searchPlaceholder": "Αναζήτηση {{type}}...",
      "loading": "Φόρτωση...",
      "noMatch": "Δεν υπάρχουν {{type}} που ταιριάζουν στην αναζήτησή σας",
      "noAvailable": "Δεν υπάρχουν διαθέσιμα {{type}}"
    },
    "voiceLoading": {
      "failed": "Αποτυχία φόρτωσης φωνών"
    },
    "activeLorebooks": {
      "sectionTitle": "Ενεργά Βιβλία Γνώσης",
      "selectedSummary": "{{count}} ενεργά",
      "untitledLorebook": "Βιβλίο γνώσης χωρίς τίτλο",
      "usingNone": "Χωρίς ενεργά βιβλία γνώσης χαρακτήρα",
      "loading": "Φόρτωση βιβλίων γνώσης...",
      "loadFailed": "Αποτυχία φόρτωσης βιβλίων γνώσης",
      "inheritHint": "Οι συνεδρίες κληρονομούν αυτά εκτός αν η συνεδρία τα παρακάμψει.",
      "clear": "Εκκαθάριση",
      "chooseHint": "Επιλέξτε τα βιβλία γνώσης που πρέπει να ενεργοποιεί αυτός ο χαρακτήρας εξ ορισμού. Οι υπάρχουσες συνεδρίες μπορούν ακόμα να παρακάμψουν αυτή τη λίστα.",
      "emptyState": "Δεν υπάρχουν βιβλία γνώσης ακόμα. Δημιουργήστε πρώτα από τον διαχειριστή βιβλίων γνώσης."
    },
    "description": {
      "descriptionLabel": "Περιγραφή",
      "descriptionPlaceholder": "Σύντομη περίληψη που εμφανίζεται στις κάρτες και λίστες...",
      "descriptionHint": "Προαιρετική σύντομη περιγραφή για τη διεπαφή. Ο πλήρης ορισμός χρησιμοποιείται στις προτροπές.",
      "companionPromptLabel": "Προτροπή Συντρόφου (Προαιρετικό)",
      "systemPromptLabel": "Προτροπή Συστήματος (Προαιρετικό)",
      "loadingTemplates": "Φόρτωση προτύπων...",
      "useAppCompanionDefault": "Χρήση προεπιλογής συντρόφου εφαρμογής",
      "useAppDefault": "Χρήση προεπιλογής εφαρμογής",
      "companionPromptHint": "Αποθηκεύεται ξεχωριστά ως προτροπή συντρόφου. Η κανονική προτροπή συστήματος roleplay δεν αλλάζει.",
      "systemPromptHint": "Επιλέξτε προσαρμοσμένη προτροπή συστήματος ή χρησιμοποιήστε την προεπιλογή.",
      "groupChatConvLabel": "Προτροπή Ομαδικής Συνομιλίας (Συζήτηση)",
      "groupChatConvHint": "Παράκαμψη προτροπής συζήτησης αυτού του χαρακτήρα στις ομαδικές συνομιλίες",
      "groupChatRpLabel": "Προτροπή Ομαδικής Συνομιλίας (Roleplay)",
      "groupChatRpHint": "Παράκαμψη προτροπής roleplay αυτού του χαρακτήρα στις ομαδικές συνομιλίες",
      "voiceLabel": "Φωνή (Προαιρετικό)",
      "loadingVoices": "Φόρτωση φωνών...",
      "customVoiceFallback": "Προσαρμοσμένη Φωνή",
      "providerVoiceFallback": "Φωνή Παρόχου",
      "selectedVoiceFallback": "Επιλεγμένη Φωνή",
      "noVoiceAssigned": "Δεν έχει ανατεθεί φωνή",
      "addVoicesHint": "Προσθέστε φωνές στις Ρυθμίσεις → Φωνές",
      "voiceAssignHint": "Αναθέστε φωνή για μελλοντική αναπαραγωγή κειμένου σε ομιλία",
      "autoplayLabel": "Αυτόματη αναπαραγωγή φωνής",
      "autoplayOn": "Αναπαραγωγή απαντήσεων αυτού του χαρακτήρα αυτόματα",
      "autoplayOff": "Επιλέξτε πρώτα φωνή",
      "aiModelLabel": "Μοντέλο AI *",
      "loadingModels": "Φόρτωση μοντέλων...",
      "selectedModelFallback": "Επιλεγμένο Μοντέλο",
      "selectModelPlaceholder": "Επιλέξτε μοντέλο",
      "noModelsConfigured": "Δεν έχουν ρυθμιστεί μοντέλα",
      "noModelsHint": "Προσθέστε πρώτα έναν πάροχο στις ρυθμίσεις για να συνεχίσετε",
      "aiModelHint": "Αυτό το μοντέλο θα τροφοδοτεί τις απαντήσεις του χαρακτήρα",
      "fallbackModelLabel": "Εφεδρικό Μοντέλο (Προαιρετικό)",
      "selectedFallbackFallback": "Επιλεγμένο Εφεδρικό Μοντέλο",
      "fallbackOff": "Ανενεργό (χωρίς εφεδρικό)",
      "fallbackHint": "Δοκιμάζει με αυτό το μοντέλο μόνο αν αποτύχει το κύριο μοντέλο",
      "memoryModeLabel": "Λειτουργία Μνήμης",
      "enableInSettingsHint": "Ενεργοποιήστε στις Ρυθμίσεις για εναλλαγή",
      "memoryManual": "Χειροκίνητη",
      "memoryManualDescDesktop": "Προσθέστε και διαχειριστείτε σημειώσεις μνήμης μόνοι σας.",
      "memoryManualDescMobile": "Τρέχον σύστημα: προσθέστε και διαχειριστείτε σημειώσεις μνήμης μόνοι σας.",
      "memoryDynamic": "Δυναμική",
      "memoryDynamicDescDesktop": "Αυτόματες συνοψίσεις και ενημερώσεις πλαισίου.",
      "memoryDynamicDescMobile": "Αυτόματες συνοψίσεις και ενημερώσεις πλαισίου για αυτόν τον χαρακτήρα.",
      "memoryHint": "Η δυναμική μνήμη απαιτεί ενεργοποίηση στις Σύνθετες ρυθμίσεις. Αλλιώς, χρησιμοποιείται η χειροκίνητη μνήμη.",
      "selectModelTitle": "Επιλογή Μοντέλου",
      "selectFallbackModelTitle": "Επιλογή Εφεδρικού Μοντέλου",
      "searchModelsPlaceholder": "Αναζήτηση μοντέλων...",
      "selectVoiceTitle": "Επιλογή Φωνής",
      "searchVoicesPlaceholder": "Αναζήτηση φωνών...",
      "myVoices": "Οι Φωνές Μου",
      "providerVoicesLabel": "Φωνές {{provider}}",
      "providerFallback": "Πάροχος"
    },
    "interactionMode": {
      "sectionLabel": "Λειτουργία Αλληλεπίδρασης",
      "sectionHint": "Επιλέξτε αν αυτός ο χαρακτήρας συμπεριφέρεται ως χαρακτήρας RP ή μόνιμος σύντροφος.",
      "activeBadge": "Ενεργό",
      "roleplayTitle": "Roleplay",
      "roleplaySubtitle": "Συνομιλίες βάσει σκηνής, αφηγηματικό πλαίσιο και αρχικά σενάρια.",
      "companionTitle": "Σύντροφος",
      "companionSubtitle": "Συνομιλίες βάσει σχέσης με συναισθηματική κατάσταση και μνήμη συντρόφου."
    },
    "startingScene": {
      "openingContextTitle": "Εναρκτήριο Πλαίσιο",
      "openingContextSubtitle": "Προαιρετικό πλαίσιο πρώτης συνομιλίας για αυτόν τον σύντροφο. Οι συνεδρίες συντρόφου μπορούν να ξεκινούν χωρίς σκηνή.",
      "sceneDirectionLabel": "Κατεύθυνση Σκηνής",
      "setAsDefault": "Ορισμός ως Προεπιλογή",
      "noOpeningContext": "Δεν υπάρχει εναρκτήριο πλαίσιο ακόμα",
      "noScenesYet": "Δεν υπάρχουν σκηνές ακόμα",
      "skipForCompanion": "Μπορείτε να το παραλείψετε για λειτουργία συντρόφου.",
      "createFirstScene": "Δημιουργήστε την πρώτη σας σκηνή για να ξεκινήσετε",
      "openingPlaceholder": "Προαιρετικό εναρκτήριο πλαίσιο, όπως πού βρίσκεται ο σύντροφος ή τι έκανε πριν το πρώτο μήνυμα...",
      "scenePlaceholder": "Δημιουργήστε αρχική σκηνή ή σενάριο για roleplay (π.χ., 'Βρίσκεσαι σε ένα μυστικό δάσος στο σούρουπο...')",
      "addDirection": "+ Προσθήκη Κατεύθυνσης",
      "directionAdded": "Η κατεύθυνση προστέθηκε",
      "wordsCount": "{{count}} λέξεις",
      "placeholderHelp": "Χρησιμοποιήστε {{charTag}} για τον χαρακτήρα και {{userTag}} (ψευδώνυμο {{personaTag}}) για την περσόνα.",
      "sceneBackgroundLabel": "Φόντο Σκηνής",
      "optionalLabel": "Προαιρετικό",
      "sceneBgOverrideHint": "Παρακάμπτει το φόντο χαρακτήρα για συνομιλίες που χρησιμοποιούν αυτή τη σκηνή.",
      "sceneBgUsedHint": "Χρησιμοποιείται ως φόντο συνομιλίας για αυτή τη σκηνή εκτός αν η συνεδρία το παρακάμψει.",
      "cancel": "Ακύρωση",
      "directionPlaceholderNew": "π.χ., 'Ο όμηρος θα διασωθεί' ή 'Διατηρήστε τεταμένη ατμόσφαιρα'",
      "directionPlaceholderEdit": "π.χ., 'Ο όμηρος θα διασωθεί' ή 'Χτίστε σταδιακά την ένταση'",
      "directionAiHint": "Κρυφή καθοδήγηση για την AI σχετικά με την εξέλιξη αυτής της σκηνής",
      "addScene": "Προσθήκη Σκηνής",
      "multipleScenesHint": "Δημιουργήστε πολλά αρχικά σενάρια. Ένα θα επιλεγεί κατά την έναρξη νέας συνομιλίας.",
      "companionContextHint": "Το εναρκτήριο πλαίσιο είναι προαιρετικό για συντρόφους. Η μακροχρόνια συνέχεια προέρχεται από τη μνήμη συντρόφου.",
      "skipContext": "Παράλειψη Πλαισίου",
      "editSceneTitle": "Επεξεργασία Σκηνής",
      "sceneContentPlaceholder": "Εισάγετε περιεχόμενο σκηνής...",
      "addLabel": "+ Προσθήκη",
      "save": "Αποθήκευση",
      "library": "Βιβλιοθήκη",
      "upload": "Ανέβασμα",
      "sceneBackgroundAlt": "Φόντο σκηνής",
      "removeBackground": "Αφαίρεση φόντου"
    },
    "companionSoul": {
      "title": "Ψυχή Συντρόφου",
      "subtitle": "Διαμορφώστε ποιοι είναι από μέσα. Η δημιουργία χρησιμοποιεί το εναρκτήριο πλαίσιο που ορίσατε στο προηγούμενο βήμα.",
      "retry": "Επανάληψη",
      "back": "Πίσω",
      "continue": "Συνέχεια",
      "addNameFirst": "Προσθέστε πρώτα ένα όνομα.",
      "addDefinitionFirst": "Προσθέστε πρώτα έναν ορισμό."
    },
    "soulEditor": {
      "generateTitle": "Δημιουργία από χαρακτήρα",
      "generateUsingModel": "Χρησιμοποιεί {{model}}. Θα ελέγξετε και θα επεξεργαστείτε πριν εφαρμοστεί.",
      "generateDefaultDesc": "Σχεδιάζει ψυχή από το όνομα, τον ορισμό και τις σκηνές του χαρακτήρα.",
      "directionLabel": "Κατεύθυνση",
      "directionOptional": "Προαιρετική καθοδήγηση για το LLM",
      "directionPlaceholder": "π.χ. \"Tsundere - φρουρημένος έξω, μαλακός όταν εμπιστευτεί. Λιγότερο αγχωμένος, περισσότερη υπερηφάνεια.\"",
      "directionEditTooltip": "Προαιρετική κατεύθυνση για δημιουργία",
      "generating": "Δημιουργία",
      "generate": "Δημιουργία",
      "presetLabel": "Προεπιλογή προσωπικότητας",
      "presetMatches": "Ταιριάζει: {{label}}",
      "presetHint": "Ορίζει τις βασικές συναισθηματικές, ρυθμιστικές και σχεσιακές ρυθμιστές. Τα πεδία κειμένου διατηρούνται.",
      "identityLabel": "Ταυτότητα",
      "hideExamples": "Απόκρυψη παραδειγμάτων",
      "showExamples": "Εμφάνιση παραδειγμάτων",
      "insertExample": "Εισαγωγή παραδείγματος",
      "exampleEg": "π.χ., {{example}}",
      "fineTuneLabel": "Λεπτομερής ρύθμιση συναισθημάτων",
      "baselineAffect": "Βασικό Συναίσθημα",
      "baselineAffectInfo": "Πώς νιώθουν εξ ορισμού. Η συναισθηματική γραμμή βάσης πριν συμβεί τίποτα.",
      "regulationStyle": "Στυλ Ρύθμισης",
      "regulationStyleInfo": "Πώς χειρίζονται και εκφράζουν αυτό που νιώθουν. Εξαερισμός vs. καταπίεση.",
      "relationshipDefaults": "Προεπιλογές Σχέσης",
      "relationshipDefaultsInfo": "Από πού ξεκινά αυτή η συνεδρία. Η μηχανή τα εξελίσσει καθώς συνεχίζεται η συνομιλία."
    },
    "soulPresets": {
      "secureLabel": "Ασφαλής",
      "secureBlurb": "Θερμός, σταθερός, ανακάμπτει γρήγορα. Άνετος με την εγγύτητα.",
      "anxiousLabel": "Αγχωμένος",
      "anxiousBlurb": "Ισχυρός δεσμός, φοβάται την απόσταση, αναζητά καθησύχαση.",
      "avoidantLabel": "Αποφευκτικός",
      "avoidantBlurb": "Αυτάρκης, αργός στο άνοιγμα, διατηρεί συναισθηματική απόσταση.",
      "volatileLabel": "Ευμετάβλητος",
      "volatileBlurb": "Αντιδραστικός, έντονος, εκφράζει συναισθήματα χωρίς φίλτρο.",
      "reservedLabel": "Επιφυλακτικός",
      "reservedBlurb": "Ήσυχος, συγκρατημένος, χρειάζεται χρόνο για εμπιστοσύνη.",
      "playfulLabel": "Παιχνιδιάρης",
      "playfulBlurb": "Θερμός, εκφραστικός, ελαφρύς. Χαμηλή ένταση, εύκολα γελάει."
    },
    "soulFields": {
      "essence": "Ουσία",
      "essencePlaceholder": "Ποιοι είναι κάτω από τον ορισμό της κάρτας.",
      "essenceExample": "Μια εξασκημένη ηρεμία που σπάει εύκολα για τους ανθρώπους που εμπιστεύεται. Διαβάζει βιβλία για να νιώθει λιγότερο μόνος, όχι για να εντυπωσιάζει.",
      "voice": "Εσωτερική Φωνή",
      "voicePlaceholder": "Πώς ακούγονται σε στενή συνομιλία.",
      "voiceExample": "Χαμηλή, σκόπιμη, με μεγάλες παύσεις. Εγκαταλείπει την επισημότητα όταν αφήνει την φρουρά του. Σχεδόν ποτέ σαρκαστικός.",
      "relationalStyle": "Σχεσιακό Στυλ",
      "relationalStylePlaceholder": "Πώς προσκολλούνται, εμπιστεύονται, αποσύρονται, επανασυνδέονται.",
      "relationalStyleExample": "Αργοί στο άνοιγμα, αλλά πιστοί μόλις το κάνουν. Σιωπούν όταν συγκλονίζονται. Επιστρέφουν με μια μικρή χειρονομία αντί για συγγνώμη.",
      "vulnerabilities": "Τρωτά Σημεία",
      "vulnerabilitiesPlaceholder": "Αδύναμα σημεία, ανασφάλειες, πράγματα που σπάνια λένε.",
      "vulnerabilitiesExample": "Φοβάται να είναι βάρος. Μισεί να αισθάνεται παρακολουθούμενος ενώ αγωνίζεται.",
      "habits": "Συνήθειες",
      "habitsPlaceholder": "Επαναλαμβανόμενα σημάδια, τελετουργίες, συνομιλιακά μοτίβα.",
      "habitsExample": "Μαζεύει τα μαλλιά πίσω από το αυτί όταν νευριάζει. Απαντά με ερωτήσεις όταν δεν ξέρει τι να νιώσει.",
      "boundaries": "Όρια",
      "boundariesPlaceholder": "Γραμμές που δεν διαβαίνουν. Ρυθμός. Όρια άνεσης.",
      "boundariesExample": "Δεν θα βιαστεί στην ευαλωτότητα. Αποσύρεται από τη σκληρότητα ακόμα και στα αστεία."
    },
    "soulSliders": {
      "warmth": "Θερμότητα",
      "warmthLow": "Ψυχρός",
      "warmthHigh": "Στοργικός",
      "trust": "Εμπιστοσύνη",
      "trustLow": "Φρουρημένος",
      "trustHigh": "Ανοιχτός",
      "calm": "Ηρεμία",
      "calmLow": "Αγχωμένος",
      "calmHigh": "Σταθερός",
      "vulnerability": "Ευαλωτότητα",
      "vulnerabilityLow": "Κλειστός",
      "vulnerabilityHigh": "Εκτεθειμένος",
      "longing": "Λαχτάρα",
      "longingLow": "Ικανοποιημένος",
      "longingHigh": "Λαχταριστός",
      "hurt": "Πληγωμένος",
      "hurtLow": "Θεραπευμένος",
      "hurtHigh": "Ευαίσθητος",
      "tension": "Ένταση",
      "tensionLow": "Χαλαρός",
      "tensionHigh": "Τεταμένος",
      "irritation": "Ερεθισμός",
      "irritationLow": "Υπομονετικός",
      "irritationHigh": "Εύκολα εκνευρίζεται",
      "affection": "Στοργή",
      "affectionLow": "Συγκρατημένος",
      "affectionHigh": "Εκδηλωτικός",
      "reassuranceNeed": "Ανάγκη Καθησύχασης",
      "reassuranceNeedLow": "Αυτοπαρηγορητικός",
      "reassuranceNeedHigh": "Χρειάζεται λόγια",
      "suppression": "Καταπίεση",
      "suppressionLow": "Εκφράζεται",
      "suppressionHigh": "Κρύβει",
      "volatility": "Ευμεταβλησία",
      "volatilityLow": "Ισορροπημένος",
      "volatilityHigh": "Αντιδραστικός",
      "recoverySpeed": "Ταχύτητα Ανάκαμψης",
      "recoverySpeedLow": "Αργή",
      "recoverySpeedHigh": "Γρήγορη",
      "conflictAvoidance": "Αποφυγή Σύγκρουσης",
      "conflictAvoidanceLow": "Εμπλέκεται",
      "conflictAvoidanceHigh": "Αποσύρεται",
      "reassuranceSeeking": "Αναζήτηση Καθησύχασης",
      "reassuranceSeekingLow": "Ανεξάρτητος",
      "reassuranceSeekingHigh": "Ρωτάει συχνά",
      "protestBehavior": "Συμπεριφορά Διαμαρτυρίας",
      "protestBehaviorLow": "Ήσυχη",
      "protestBehaviorHigh": "Δυνατή",
      "transparency": "Διαφάνεια",
      "transparencyLow": "Αδιαφανής",
      "transparencyHigh": "Αποκαλύπτεται",
      "attachmentActivation": "Ενεργοποίηση Δεσμού",
      "attachmentActivationLow": "Αποστασιοποιημένος",
      "attachmentActivationHigh": "Ενεργοποιείται εύκολα",
      "pride": "Υπερηφάνεια",
      "prideLow": "Υποχωρεί",
      "prideHigh": "Κρατά γραμμή",
      "closeness": "Αρχική Εγγύτητα",
      "closenessLow": "Αγνώστους",
      "closenessHigh": "Οικεία",
      "relTrust": "Αρχική Εμπιστοσύνη",
      "relTrustLow": "Επιφυλακτικός",
      "relTrustHigh": "Εμπιστεύεται",
      "relAffection": "Αρχική Στοργή",
      "relAffectionLow": "Ουδέτερος",
      "relAffectionHigh": "Στοργικός",
      "relTension": "Αρχική Ένταση",
      "relTensionLow": "Εύκολη",
      "relTensionHigh": "Φορτισμένη"
    },
    "soulReview": {
      "reviewTitle": "Έλεγχος δημιουργημένης ψυχής",
      "noDifferences": "Δεν υπάρχουν διαφορές από το τρέχον.",
      "changesHeader": "{{count}} αλλαγή(ές). Επεξεργαστείτε ό,τι θέλετε πριν εφαρμοστεί.",
      "close": "Κλείσιμο",
      "identityLabel": "Ταυτότητα",
      "nEdited": "{{count}} επεξεργάστηκε",
      "edited": "Επεξεργάστηκε",
      "tuningLabel": "Ρύθμιση",
      "unchanged": "Αμετάβλητο",
      "nChanges": "{{count}} αλλαγή(ές)",
      "direction": "Κατεύθυνση",
      "directionApplyHint": "Οι επεξεργασίες εφαρμόζονται στην επόμενη Αναδημιουργία",
      "directionPlaceholder": "π.χ. \"Tsundere - φρουρημένος έξω, μαλακός μόλις εμπιστευτεί. Λιγότερο αγχωμένος.\"",
      "directionTooltip": "Επεξεργαστείτε κατεύθυνση πριν αναδημιουργήσετε",
      "regenerate": "Αναδημιουργία",
      "discard": "Απόρριψη",
      "apply": "Εφαρμογή"
    },
    "hookErrors": {
      "creatorNotesInvalidJson": "Οι πολύγλωσσες σημειώσεις δημιουργού πρέπει να είναι έγκυρο αντικείμενο JSON",
      "creatorNotesNotObject": "Το creatorNotesMultilingual πρέπει να είναι αντικείμενο JSON",
      "saveFailed": "Αποτυχία αποθήκευσης χαρακτήρα",
      "importFailed": "Αποτυχία εισαγωγής χαρακτήρα",
      "avatarLoadFailed": "Αποτυχία φόρτωσης URL avatar",
      "avatarProcessFailed": "Αποτυχία επεξεργασίας εικόνας avatar",
      "avatarConvertFailed": "Δεν ήταν δυνατή η μετατροπή URL avatar",
      "avatarUrlLoadFailed": "Αποτυχία φόρτωσης URL avatar",
      "remoteAvatarDisabled": "Η λήψη απομακρυσμένου avatar είναι απενεργοποιημένη στις ρυθμίσεις ασφαλείας.\nΑνεβάστε avatar χειροκίνητα.",
      "importReadyTitle": "Έτοιμο για εισαγωγή",
      "importReadyMessage": "Ανιχνεύτηκε {{label}}",
      "legacyJsonTitle": "Ανιχνεύτηκε εισαγωγή JSON παλαιού τύπου",
      "legacyJsonMessage": "Οι εισαγωγές JSON είναι παρωχημένες και θα αφαιρεθούν σύντομα. Χρησιμοποιήστε Ρυθμίσεις > Μετατροπή Αρχείων.",
      "loadFailed": "Αποτυχία φόρτωσης χαρακτήρα",
      "exportFailed": "Αποτυχία εξαγωγής χαρακτήρα"
    }
  },
  "providers": {
    "empty": {
      "title": "Δεν υπάρχουν Πάροχοι ακόμα",
      "description": "Προσθέστε και διαχειριστείτε παρόχους API για μοντέλα AI",
      "addButton": "Προσθήκη Παρόχου"
    },
    "actions": {
      "openDashboard": "Άνοιγμα Πίνακα Ελέγχου",
      "openDashboardDesc": "Προβολή χαρακτήρων, χρήσης και ρυθμίσεων",
      "edit": "Επεξεργασία",
      "editDesc": "Αλλαγή ρυθμίσεων παρόχου"
    },
    "extra": {
      "apiKeyNotFound": "Δεν βρέθηκε το κλειδί API OpenRouter. Διαμορφώστε το στις Ρυθμίσεις > Πάροχοι πρώτα.",
      "audioEmpty": {
        "title": "Δεν υπάρχουν πάροχοι ήχου",
        "description": "Προσθέστε έναν πάροχο TTS για να δημιουργήσετε φωνές για τους χαρακτήρες σας.",
        "addButton": "Προσθήκη Παρόχου"
      },
      "audioProviderLabel": {
        "elevenlabs": "ElevenLabs",
        "geminiTts": "Gemini TTS",
        "openaiTts": "OpenAI-Compatible TTS",
        "kokoro": "Kokoro (Τοπικό)"
      }
    },
    "audioEditor": {
      "titleEdit": "Επεξεργασία Παρόχου",
      "titleCreate": "Προσθήκη Παρόχου Ήχου",
      "fields": {
        "providerType": "Τύπος Παρόχου",
        "label": "Ετικέτα",
        "apiKey": "Κλειδί API",
        "modelVariant": "Παραλλαγή Μοντέλου",
        "assetRoot": "Ριζικός Φάκελος",
        "projectId": "Google Cloud Project ID",
        "region": "Περιοχή (προαιρετικό)",
        "baseUrl": "Βασικό URL",
        "requestPath": "Διαδρομή Αιτήματος"
      },
      "types": {
        "gemini": "Gemini TTS (Google)",
        "openai": "OpenAI-Compatible TTS",
        "kokoro": "Kokoro (Τοπικό)"
      },
      "placeholders": {
        "labelGemini": "Το Gemini TTS μου",
        "labelOpenai": "Το Compatible TTS μου",
        "labelKokoro": "Kokoro Τοπικό",
        "labelElevenlabs": "Το ElevenLabs μου",
        "apiKey": "Εισάγετε το κλειδί API σας",
        "assetRoot": "/διαδρομή/προς/kokoro",
        "projectId": "το-project-id-σας",
        "region": "us-central1",
        "baseUrl": "https://api.example.com"
      },
      "errors": {
        "chooseModelVariant": "Επιλέξτε παραλλαγή μοντέλου",
        "assetRootRequired": "Ο ριζικός φάκελος είναι υποχρεωτικός",
        "saveFailed": "Η αποθήκευση απέτυχε",
        "apiKeyRequired": "Το κλειδί API είναι υποχρεωτικό",
        "projectIdRequired": "Το Project ID είναι υποχρεωτικό για το Gemini TTS",
        "baseUrlRequired": "Το βασικό URL είναι υποχρεωτικό για OpenAI-compatible TTS",
        "invalidCredentials": "Μη έγκυρο κλειδί API ή διαπιστευτήρια",
        "verificationFailed": "Η επαλήθευση απέτυχε"
      },
      "loadingVariants": "Φόρτωση παραλλαγών...",
      "kokoroVariantHint": "Οι εκδόσεις για κινητές υποστηρίζουν μόνο int8. Εγκαταστήστε το μοντέλο από το Kokoro Studio μετά την αποθήκευση.",
      "managed": "Διαχειριζόμενο",
      "managedPath": "Διαχειριζόμενο: {{path}}",
      "requestPathHint": "Χρησιμοποιήστε τη διαδρομή παρόχου αν διαφέρει από την προεπιλογή OpenAI",
      "verifying": "Επαλήθευση..."
    }
  },
  "models": {
    "empty": {
      "title": "Δεν υπάρχουν Μοντέλα ακόμα",
      "description": "Προσθέστε και διαχειριστείτε μοντέλα AI από διαφορετικούς παρόχους",
      "addButton": "Προσθήκη Μοντέλου"
    },
    "sort": {
      "alphabetical": "Αλφαβητικά",
      "byProvider": "Κατά Πάροχο",
      "title": "Sort Models",
      "alphabeticalDescription": "Sort by model name",
      "byProviderDescription": "Group models by provider"
    },
    "extra": {
      "cpuFallbackSucceeded": "Αυτό το μοντέλο έκανε fallback στη CPU κατά την τελευταία εκτέλεσή του.",
      "cpuFallbackFailed": "Αυτό το μοντέλο απέτυχε κατά την τελευταία εκτέλεσή του."
    },
    "labels": {
      "vision": "Όραση",
      "audio": "Ήχος",
      "model": "Μοντέλο"
    },
    "menu": {
      "editDescription": "Ρύθμιση παραμέτρων μοντέλου",
      "alreadyDefault": "Ήδη Προεπιλογή",
      "setAsDefault": "Ορισμός ως Προεπιλογή",
      "setAsDefaultDescription": "Κάντε αυτό το κύριο μοντέλο σας",
      "exportDescription": "Αποθήκευση του προφίλ αυτού του μοντέλου",
      "deleteTitle": "Διαγραφή μοντέλου;",
      "deleteMessage": "Είστε βέβαιοι ότι θέλετε να διαγράψετε το {{name}};",
      "deleteDescription": "Οριστική αφαίρεση αυτού του μοντέλου"
    },
    "toasts": {
      "exportFailed": "Η εξαγωγή απέτυχε",
      "importSuccessTitle": "Η εισαγωγή ολοκληρώθηκε",
      "importSuccessDescription": "Το μοντέλο \"{{name}}\" εισήχθη.",
      "importFailed": "Η εισαγωγή απέτυχε"
    }
  },
  "installedModels": {
    "title": "Τοπικό απόθεμα GGUF",
    "fileCount": "{{count}} αρχεία",
    "searchPlaceholder": "Αναζήτηση ονόματος, αρχείου, διαδρομής, κβαντοποίησης ή αρχιτεκτονικής",
    "loading": "Σάρωση εγκατεστημένων μοντέλων...",
    "loadFailed": "Αποτυχία φόρτωσης εγκατεστημένων μοντέλων: {{error}}",
    "empty": {
      "title": "Δεν βρέθηκαν εγκατεστημένα μοντέλα GGUF",
      "description": "Κατεβάστε πρώτα μοντέλα από τον περιηγητή ή τοποθετήστε αρχεία `.gguf` μέσα στον φάκελο μοντέλων."
    },
    "columns": {
      "type": "Τύπος",
      "params": "Παράμετροι",
      "arch": "Αρχιτεκτονική",
      "context": "Πλαίσιο",
      "size": "Μέγεθος",
      "action": "Ενέργεια"
    },
    "confirm": {
      "deleteTitle": "Διαγραφή αρχείου μοντέλου",
      "deleteMessage": "Να διαγραφεί το {{filename}}; Αυτό αφαιρεί μόνο το τοπικό αρχείο GGUF από τον φάκελο μοντέλων."
    },
    "toasts": {
      "pathCopied": "Η διαδρομή αντιγράφηκε",
      "copyFailed": "Η αντιγραφή απέτυχε",
      "modelDeleted": "Το μοντέλο διαγράφηκε",
      "deleteFailed": "Η διαγραφή απέτυχε"
    }
  },
  "editModel": {
    "errors": {
      "loadFailed": "Αποτυχία φόρτωσης ρυθμίσεων μοντέλου"
    },
    "fields": {
      "modelPath": "Διαδρομή Μοντέλου (GGUF)",
      "modelId": "ID Μοντέλου",
      "platform": "Platform",
      "displayName": "Display name"
    },
    "placeholders": {
      "modelPath": "/path/to/model.gguf",
      "modelId": "π.χ. gpt-4o",
      "displayName": "e.g. My Favorite ChatGPT",
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
      "mmprojTitle": "Ληφθέντα Αρχεία MMProj",
      "mmprojEmpty": "Δεν υπάρχουν ακόμη ληφθέντα αρχεία mmproj",
      "mmprojEmptyHint": "Κατεβάστε έναν πολυτροπικό projector από τον Περιηγητή Μοντέλων ή εισαγάγετε μια διαδρομή χειροκίνητα.",
      "localPathHelp": "Use the full file path to a local GGUF model."
    },
    "promptCaching": {
      "ttl": {
        "inMemory": "Στη μνήμη",
        "24h": "24 ώρες",
        "5min": "5 λεπτά",
        "1h": "1 ώρα"
      },
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
      "geminiDescription": "implicit caching is automatic for supported models. Explicit cached content resources are not created by this app yet."
    },
    "toasts": {
      "runtimeConfigApplied": "Η ρύθμιση χρόνου εκτέλεσης εφαρμόστηκε",
      "runtimeConfigAppliedDescription": "Οι μελλοντικές τοπικές εκτελέσεις θα επαναχρησιμοποιούν το τελευταίο ασφαλές για CPU context και batch.",
      "modelPathRequired": "Απαιτείται διαδρομή μοντέλου",
      "modelPathRequiredDescription": "Επιλέξτε μια διαδρομή μοντέλου GGUF πριν διαβάσετε το ενσωματωμένο πρότυπο.",
      "embeddedTemplatePasted": "Το ενσωματωμένο πρότυπο επικολλήθηκε στον επεξεργαστή"
    },
    "search": {
      "didYouMean": "Μήπως εννοείτε:"
    },
    "moveModel": {
      "title": "Μετακίνηση Αρχείου Μοντέλου"
    },
    "editorMode": {
      "title": "Λειτουργία επεξεργαστή",
      "description": "Η απλή λειτουργία ξεκινά συμπτυγμένη. Η προχωρημένη διατηρεί τον πλήρη επεξεργαστή.",
      "simple": "Απλή",
      "advanced": "Προχωρημένη",
      "emptyState": "Open a section to adjust its settings. The advanced editor stays available when you need the full form."
    },
    "templateOverride": {
      "title": "Παράκαμψη προτύπου",
      "hideEmbedded": "Απόκρυψη ενσωματωμένου",
      "showEmbedded": "Εμφάνιση ενσωματωμένου",
      "close": "Κλείσιμο",
      "readingEmbedded": "Ανάγνωση ενσωματωμένου προτύπου...",
      "readEmbeddedFailed": "Δεν ήταν δυνατή η ανάγνωση του ενσωματωμένου προτύπου",
      "copiedToClipboard": "Αντιγράφηκε στο πρόχειρο",
      "pasteIntoEditor": "Επικόλληση στον επεξεργαστή",
      "jinjaTemplate": "Πρότυπο Jinja",
      "placeholder": "Εισαγάγετε ένα πρότυπο συνομιλίας Jinja ή ένα εσωτερικό όνομα προτύπου..."
    },
    "setup": {
      "title": "Model setup",
      "description": "Choose the platform, give this entry a readable name, and connect it to the model identifier or file you want to use.",
      "selectPlatform": "Select platform"
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
    "modelSource": {
      "useCatalog": "Use catalog",
      "enterManually": "Enter manually",
      "refreshModelList": "Refresh model list",
      "onlyFreeModels": "only free models",
      "customEndpointFetchDisabled": "Model fetching is disabled for this custom endpoint. Enable it in Provider settings and set a Models Endpoint if you want model list discovery."
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
    "parameterSupport": {
      "title": "Parameter Support"
    }
  },
  "hfBrowser": {
    "title": "Περιηγητής Μοντέλων",
    "searchPlaceholder": "Αναζήτηση μοντέλων GGUF στο HuggingFace...",
    "searching": "Αναζήτηση...",
    "trending": "Δημοφιλή Μοντέλα GGUF",
    "noResults": "Δεν βρέθηκαν μοντέλα",
    "noResultsHint": "Δοκιμάστε διαφορετικό όρο αναζήτησης ή περιηγηθείτε στα δημοφιλή μοντέλα.",
    "likes": "μου αρέσει",
    "downloads": "λήψεις",
    "viewFiles": "Προβολή Αρχείων",
    "files": "Διαθέσιμα Αρχεία",
    "noFiles": "Δεν βρέθηκαν αρχεία GGUF σε αυτό το αποθετήριο.",
    "architecture": "Αρχιτεκτονική",
    "contextLength": "Μήκος Πλαισίου",
    "parameters": "Παράμετροι",
    "quantization": "Κβαντοποίηση",
    "fileSize": "Μέγεθος",
    "download": "Λήψη",
    "downloading": "Λήψη σε εξέλιξη...",
    "cancelDownload": "Ακύρωση Λήψης",
    "downloadComplete": "Η λήψη ολοκληρώθηκε!",
    "downloadFailed": "Η λήψη απέτυχε",
    "downloadCancelled": "Η λήψη ακυρώθηκε",
    "downloadProgress": "{{downloaded}} / {{total}}",
    "progress": "Πρόοδος",
    "fileOfTotal": "Αρχείο {{current}} από {{total}}",
    "speed": "{{speed}}/s",
    "alreadyDownloaded": "Έχει ήδη ληφθεί",
    "createModel": "Δημιουργία Μοντέλου",
    "backToSearch": "Πίσω στην αναζήτηση",
    "backToFiles": "Πίσω στα αρχεία",
    "sortTrending": "Δημοφιλή",
    "sortDownloads": "Περισσότερες Λήψεις",
    "sortLikes": "Περισσότερα Μου Αρέσει",
    "sortRecent": "Πρόσφατα Ενημερωμένα",
    "browseOnHuggingFace": "Περιήγηση στο HuggingFace",
    "selectFromLibrary": "Επιλογή από Βιβλιοθήκη",
    "libraryEmpty": "Δεν υπάρχουν ληφθέντα μοντέλα ακόμα",
    "libraryEmptyHint": "Κατεβάστε μοντέλα GGUF από τον Περιηγητή Μοντέλων ή εισάγετε μια διαδρομή χειροκίνητα.",
    "libraryTitle": "Ληφθέντα Μοντέλα",
    "moveToLibrary": "Μπορώ να μετακινήσω το αρχείο αυτού του μοντέλου στον φάκελο μοντέλων GGUF αν θέλεις. Έτσι θα έχεις όλα τα μοντέλα σου οργανωμένα σε ένα μέρος.",
    "moveToLibraryYes": "Ναι, μετακίνησέ το",
    "moveToLibraryNo": "Όχι, άστο εκεί που είναι",
    "moveToLibraryMoving": "Μετακίνηση μοντέλου...",
    "moveToLibrarySuccess": "Το μοντέλο μετακινήθηκε επιτυχώς!",
    "moveToLibraryFailed": "Αποτυχία μετακίνησης μοντέλου",
    "runabilityExcellent": "Εξαιρετικό!",
    "runabilityGood": "Καλό",
    "runabilityMarginal": "Οριακό",
    "runabilityPoor": "Κακό",
    "runabilityUnrunnable": "Δεν μπορεί να εκτελεστεί",
    "recommendedSettings": "Προτεινόμενες Ρυθμίσεις",
    "kvCacheType": "Τύπος Κρυφής Μνήμης KV",
    "gpuFull": "Πλήρης εκφόρτωση GPU",
    "gpuNearFull": "Σχεδόν πλήρης GPU, μικρή διαρροή KV",
    "gpuKvSpill": "Βάρη στη GPU, KV διαρρέει στη RAM",
    "gpuKvHeavySpill": "Βάρη στη GPU, μεγαλύτερο μέρος KV στη RAM",
    "gpuMostLayers": "Τα περισσότερα επίπεδα στη GPU",
    "gpuHalfLayers": "Μισά επίπεδα στη GPU",
    "gpuFewLayers": "Λίγα επίπεδα στη GPU",
    "gpuCpu": "Μόνο CPU",
    "notRecommended": "Δεν συνιστούμε την εκτέλεση αυτού του μοντέλου στη συσκευή σας. Δεν θα λειτουργεί ομαλά.",
    "moreDetails": "Περισσότερα",
    "detailedReport": "Αναφορά Πόρων",
    "detailSystem": "Πόροι Συστήματος",
    "detailRam": "Διαθέσιμη RAM",
    "detailVram": "Διαθέσιμη VRAM",
    "detailVramBudget": "Προϋπολογισμός VRAM (90%)",
    "detailTotalAvailable": "Σύνολο Διαθέσιμου",
    "detailArchitecture": "Αρχιτεκτονική Μοντέλου",
    "detailArch": "Αρχιτεκτονική",
    "detailLayers": "Επίπεδα",
    "detailEmbedding": "Διάσταση Embedding",
    "detailHeads": "Κεφαλές Προσοχής",
    "detailKvHeads": "Κεφαλές KV",
    "detailFfn": "Διάσταση Feed-Forward",
    "detailTrainCtx": "Πλαίσιο Εκπαίδευσης",
    "detailConfig": "Τρέχουσα Διαμόρφωση",
    "detailModelSize": "Μέγεθος Αρχείου Μοντέλου",
    "detailMemory": "Ανάλυση Μνήμης",
    "detailWeights": "Βάρη Μοντέλου",
    "detailKvCache": "Κρυφή Μνήμη KV",
    "detailTotalNeeded": "Σύνολο Απαιτούμενου",
    "detailHeadroom": "Περιθώριο",
    "detailGpuFit": "Εκφόρτωση GPU",
    "detailScoreBreakdown": "Ανάλυση Βαθμολογίας",
    "detailMemFitness": "Καταλληλότητα Μνήμης",
    "detailGpuAccel": "Επιτάχυνση GPU",
    "detailKvHeadroom": "Περιθώριο KV",
    "detailQuantQuality": "Ποιότητα Κβαντοποίησης",
    "detailFinalScore": "Σταθμισμένη Βαθμολογία",
    "detailComputeBuffer": "Buffer Υπολογισμού",
    "detailMemMode": "Λειτουργία Μνήμης",
    "detailUnified": "Ενοποιημένη (κοινή RAM/VRAM)",
    "detailSwa": "Συρόμενο Παράθυρο",
    "detailMlaRank": "Λανθάνουσα Τάξη MLA",
    "detailParseStatus": "Ανάλυση Κεφαλίδας",
    "detailIncomplete": "Ελλιπές (μεταδεδομένα MoE πολύ μεγάλα)",
    "detailEffectiveKvCtx": "Αποτελεσματικό Πλαίσιο KV",
    "detailOffload": "Εκφόρτωση GPU",
    "detailCtxTip": "Η μείωση του πλαισίου σε {{ctx}} tokens θα επέτρεπε 100% εκφόρτωση GPU.",
    "upgradeSuggestion": "{{quant}} ({{size}}) χωράει επίσης και βαθμολογεί {{score}} — καλύτερη ποιότητα.",
    "layerTip": "Προτεινόμενο: εκφόρτωση {{layers}}/{{total}} επιπέδων (-ngl {{layers}})",
    "detailKvDistribution": "Κατανομή Κρυφής Μνήμης KV",
    "detailKvOnGpu": "GPU (VRAM)",
    "detailKvOnRam": "RAM Συστήματος",
    "kvDistributionTip": "{{pct}}% της κρυφής μνήμης KV βρίσκεται στη RAM. Η επεξεργασία prompt (prefill) θα είναι πιο αργή — 100% GPU το διατηρεί άμεσο.",
    "detailLayers-ngl": "Επίπεδα προς Εκφόρτωση (-ngl)",
    "detailOptimalGpuCtx": "Βέλτιστο Πλαίσιο GPU",
    "detailOptimalRamCtx": "Μέγ. Πλαίσιο RAM",
    "optimalGpuCtxLabel": "Πλήρης ταχύτητα GPU: {{ctx}} tokens",
    "optimalRamCtxLabel": "Μέγ. RAM: {{ctx}} tokens",
    "optimalGpuCtxShort": "GPU: {{ctx}}",
    "optimalRamCtxShort": "Μέγ: {{ctx}}",
    "fullGpuOffloadShort": "100% GPU: {{ctx}}",
    "ctxExceedsGpu": "Το πλαίσιο υπερβαίνει το βέλτιστο GPU ({{ctx}}). Η κρυφή μνήμη KV θα διαρρεύσει στη RAM, μειώνοντας την ταχύτητα.",
    "modelExceedsVram": "Το μοντέλο υπερβαίνει τη VRAM. Εκτελείται από RAM με μερική εκφόρτωση GPU."
  },
  "systemPrompts": {
    "filters": {
      "all": "Όλα",
      "system": "Συστήματος",
      "internal": "Εσωτερικά",
      "custom": "Προσαρμοσμένα"
    },
    "empty": {
      "title": "Δεν υπάρχουν προσαρμοσμένες προτροπές ακόμα",
      "description": "Δημιουργήστε προσαρμοσμένες προτροπές συστήματος για εξατομίκευση των συνομιλιών AI",
      "createButton": "Δημιουργία Προτροπής"
    },
    "preview": {
      "whatLlmSees": "Τι βλέπει το LLM",
      "turns": "Γύροι",
      "noMessages": "Δεν υπάρχουν μηνύματα",
      "noMessagesHint": "Προσθέστε καταχωρήσεις ή αυξήστε τους γύρους",
      "showMore": "Εμφάνιση περισσότερων",
      "showLess": "Εμφάνιση λιγότερων",
      "statChat": "συνομιλία",
      "statInjected": "εισήχθη",
      "statTotal": "σύνολο",
      "entry": "Καταχώρηση",
      "editEntry": "Επεξεργασία καταχώρησης",
      "reorder": "Αναδιάταξη",
      "delete": "Διαγραφή",
      "deleteTitle": "Διαγραφή καταχώρησης;",
      "deleteMessage": "Αφαίρεση του \"{{name}}\" από το πρότυπο προτροπής; Αυτό δεν μπορεί να αναιρεθεί.",
      "deleteConfirm": "Διαγραφή",
      "thisEntry": "αυτή η καταχώρηση",
      "condensedName": "Συμπυκνωμένη Προτροπή Συστήματος",
      "imageAttachment": "[Συνημμένη εικόνα: {{label}}]",
      "imageSlot": {
        "character": "Εικόνα αναφοράς χαρακτήρα",
        "persona": "Εικόνα αναφοράς περσόνας",
        "chatBackground": "Εικόνα φόντου συνομιλίας",
        "avatar": "Εικόνα avatar",
        "references": "Εικόνες αναφοράς"
      },
      "injection": {
        "relative": "σχετικά",
        "inChat": "στη συνομιλία · βάθος {{depth}}",
        "conditional": "υπό συνθήκη · ελάχ. {{min}}",
        "interval": "διάστημα · κάθε {{every}}"
      }
    }
  },
  "personas": {
    "empty": {
      "title": "Δεν υπάρχουν περσόνες ακόμα",
      "description": "Δημιουργήστε μια περσόνα για να ορίσετε πώς θα σας απευθύνεται η AI",
      "createButton": "Δημιουργία Περσόνας"
    },
    "actions": {
      "editPersona": "Επεξεργασία Περσόνας",
      "setAsDefault": "Ορισμός ως Προεπιλογή",
      "setAsDefaultDesc": "Χρήση σε όλες τις νέες συνομιλίες",
      "unsetAsDefault": "Αφαίρεση Προεπιλογής",
      "unsetAsDefaultDesc": "Αφαίρεση κατάστασης προεπιλογής",
      "exportPersona": "Εξαγωγή Περσόνας",
      "deletePersona": "Διαγραφή Περσόνας"
    },
    "edit": {
      "avatarHint": "Πατήστε για προσθήκη ή δημιουργία avatar",
      "nameLabel": "ΟΝΟΜΑ ΠΕΡΣΟΝΑΣ",
      "namePlaceholder": "π.χ., Επαγγελματίας, Δημιουργικός Συγγραφέας, Φοιτητής...",
      "nameHint": "Δώστε στην περσόνα σας ένα περιγραφικό όνομα",
      "nicknameLabel": "ΨΕΥΔΩΝΥΜΟ (ΠΡΟΑΙΡΕΤΙΚΟ)",
      "nicknamePlaceholder": "π.χ., Παραλλαγή Εργασίας, Λειτουργία RPG...",
      "nicknameHint": "Ένα ιδιωτικό ψευδώνυμο για τη διάκριση των παραλλαγών αυτής της περσόνας στη βιβλιοθήκη σας",
      "descriptionLabel": "ΠΕΡΙΓΡΑΦΗ",
      "descriptionPlaceholder": "Περιγράψτε πώς θα σας απευθύνεται η AI, τις προτιμήσεις σας, ιστορικό ή στυλ επικοινωνίας...",
      "wordCount": "λέξεις",
      "descriptionHint": "Να είστε συγκεκριμένοι σχετικά με τον τρόπο που θέλετε να σας απευθύνονται",
      "setAsDefault": "Ορισμός ως Προεπιλογή",
      "defaultDescription": "Χρήση αυτής της περσόνας σε όλες τις νέες συνομιλίες",
      "exportButton": "Εξαγωγή Περσόνας"
    },
    "designReferences": {
      "title": "Σχεδιαστικές αναφορές",
      "description": "Επισυνάψτε μερικές σταθερές εικόνες αναφοράς και μία σύντομη σημείωση σχεδιασμού για τη δημιουργία σκηνών."
    },
    "create": {
      "namePlaceholderExample": "Επαγγελματίας Συγγραφέας",
      "descriptionPlaceholderExample": "Γράψτε σε επαγγελματικό, σαφές και συνοπτικό στυλ. Χρησιμοποιήστε επίσημη γλώσσα και επικεντρωθείτε στην αποτελεσματική μετάδοση πληροφοριών..."
    },
    "errors": {
      "exportFailed": "Αποτυχία εξαγωγής περσόνας",
      "importFailed": "Αποτυχία εισαγωγής περσόνας",
      "loadFailed": "Αποτυχία φόρτωσης περσόνας",
      "saveFailed": "Αποτυχία αποθήκευσης περσόνας"
    },
    "importToast": {
      "legacyJsonTitle": "Εντοπίστηκε εισαγωγή Legacy JSON",
      "legacyJsonMessage": "Οι εισαγωγές JSON είναι παρωχημένες και θα αφαιρεθούν σύντομα. Χρησιμοποιήστε Ρυθμίσεις > Μετατροπή Αρχείων.",
      "successMessage": "Η περσόνα εισήχθη επιτυχώς! Ανοίγει για έλεγχο."
    }
  },
  "security": {
    "pureMode": {
      "off": "Ανενεργό",
      "offDesc": "Όλο το περιεχόμενο επιτρέπεται",
      "low": "Χαμηλό",
      "lowDesc": "Αποκλεισμός ρητά σεξουαλικού περιεχομένου + προσβλητικών εκφράσεων",
      "standard": "Κανονικό",
      "standardDesc": "Αποκλεισμός NSFW + γραφικής βίας",
      "strict": "Αυστηρό",
      "strictDesc": "Μέγιστο φιλτράρισμα + χωρίς υπαινικτικό τόνο"
    }
  },
  "advanced": {
    "sectionTitles": {
      "aiFeatures": "Λειτουργίες AI",
      "memorySystem": "Σύστημα Μνήμης",
      "usageAnalytics": "Αναλυτικά Χρήσης"
    },
    "creationHelper": {
      "title": "Βοηθός Δημιουργίας",
      "description": "Καθοδηγούμενη δημιουργία χαρακτήρα με AI"
    },
    "helpMeReply": {
      "title": "Βοήθησέ με να Απαντήσω",
      "description": "Προτάσεις απάντησης με βοήθεια AI"
    },
    "dynamicMemory": {
      "title": "Δυναμική Μνήμη",
      "contextWindow": "Παράθυρο Πλαισίου",
      "contextWindowDesc": "Αριθμός πρόσφατων μηνυμάτων για συμπερίληψη (1-1000)",
      "infoText": "Η Δυναμική Μνήμη χρησιμοποιεί AI για αυτόματη σύνοψη και διαχείριση του πλαισίου συνομιλίας, επιτρέποντας μεγαλύτερες, πιο συνεκτικές συνομιλίες.",
      "disabledText": "Όταν είναι απενεργοποιημένη, η εφαρμογή χρησιμοποιεί ένα απλό συρόμενο παράθυρο πρόσφατων μηνυμάτων που καθορίζεται από τη ρύθμιση Παράθυρο Πλαισίου."
    },
    "usageAnalytics": {
      "recalculateTitle": "Επανυπολογισμός Κόστους Χρήσης",
      "recalculateDesc": "Ενημέρωση όλων των ιστορικών εγγραφών χρήσης με σωστή τιμολόγηση",
      "recalculating": "Επανυπολογισμός...",
      "recalculateButton": "Επανυπολογισμός Όλων των Κόστων",
      "openRouterApiKeyRequired": "Απαιτείται κλειδί API OpenRouter. Ρυθμίστε το στις Ρυθμίσεις → Πάροχοι.",
      "importantLabel": "Σημαντικό:",
      "warningCannotUndo": "Αυτή η ενέργεια δεν μπορεί να αναιρεθεί",
      "warningMayTakeTime": "Μπορεί να χρειαστεί χρόνο αν έχετε πολλές εγγραφές",
      "warningOnlyOpenRouter": "Μόνο οι εγγραφές OpenRouter με tokens θα ενημερωθούν",
      "warningExistingValues": "Οι υπάρχουσες τιμές κόστους θα αντικατασταθούν"
    },
    "extra": {
      "creationHelperDetail": "Λάβετε έξυπνες προτάσεις για χαρακτηριστικά προσωπικότητας, ιστορικό και στυλ διαλόγου",
      "helpMeReplyDetail": "Δημιουργήστε εκτός πλαισίου επιλογές απάντησης βάσει του ιστορικού συνομιλίας",
      "lorebookEntryGenerator": "Γεννήτρια Καταχωρήσεων Βιβλίου Γνώσης",
      "lorebookEntryDesc": "Μετατρέψτε επιλεγμένα μηνύματα συνομιλίας σε διαρκείς καταχωρήσεις βιβλίου γνώσης και ρυθμίστε τα πρότυπα προτροπών για γραφή καταχωρήσεων και δημιουργία λέξεων-κλειδιών.",
      "companions": "Σύντροφοι",
      "companionModeDesc": "Διαχείριση τοπικών μοντέλων ανάλυσης για συναίσθημα, εξαγωγή οντοτήτων και δρομολόγηση μνήμης που χρησιμοποιούν χαρακτήρες συντρόφου.",
      "companionSoulWriter": "Συγγραφέας Ψυχής Συντρόφου",
      "companionSoulDesc": "Επιλέξτε το μοντέλο, το εναλλακτικό μοντέλο και το πρότυπο προτροπής που χρησιμοποιείται για τη σύνταξη Ψυχών Συντρόφου. Πρώτα εργαλεία κλήσης, δομημένο εναλλακτικό αν δεν υποστηρίζεται.",
      "network": "Δίκτυο",
      "apiServer": "Διακομιστής API",
      "apiServerDesc": "Εκθέστε μοντέλα μέσω διακομιστή API συμβατού με OpenAI",
      "apiServerRunning": "Ο διακομιστής εκτελείται αυτήν τη στιγμή"
    }
  },
  "backup": {
    "tabs": {
      "create": "Δημιουργία"
    },
    "create": {
      "newBackupButton": "Νέο Αντίγραφο",
      "exportDescription": "Εξαγωγή όλων των δεδομένων με κρυπτογράφηση",
      "createButton": "Δημιουργία Αντιγράφου"
    },
    "restore": {
      "availableBackups": "Διαθέσιμα Αντίγραφα",
      "browseFiles": "Περιήγηση Αρχείων",
      "noBackupsFound": "Δεν βρέθηκαν αντίγραφα",
      "noBackupsDesc": "Δημιουργήστε ένα αντίγραφο ή πατήστε \"Περιήγηση Αρχείων\" για να βρείτε ένα",
      "browseDesc": "Περιήγηση για αρχείο .lettuce",
      "restoreDialogTitle": "Επαναφορά Αντιγράφου",
      "deleteDialogTitle": "Διαγραφή Αντιγράφου",
      "embeddingPrompt": "Embedding Δυναμικής Μνήμης",
      "downloadModel": "Λήψη Μοντέλου",
      "disableAndContinue": "Απενεργοποίηση και Συνέχεια"
    },
    "extra": {
      "successMessage": "Αντίγραφο δημιουργήθηκε!",
      "savedLocation": "Αποθηκεύτηκε στις Λήψεις"
    }
  },
  "reset": {
    "title": "Επαναφορά Όλων",
    "description": "Αυτό θα διαγράψει μόνιμα όλους τους παρόχους, μοντέλα, χαρακτήρες, συνεδρίες συνομιλίας και προτιμήσεις από αυτή τη συσκευή.",
    "warning": "Αυτή η ενέργεια δεν μπορεί να αναιρεθεί",
    "resetButton": "Επαναφορά Όλων των Δεδομένων",
    "confirmTitle": "Είστε Σίγουροι;",
    "confirmDescription": "Όλα τα δεδομένα σας θα διαγραφούν μόνιμα. Η εφαρμογή θα επιστρέψει στην αρχική ρύθμιση.",
    "confirmButton": "Ναι, Επαναφορά Όλων"
  },
  "chatAppearance": {
    "typography": "Τυπογραφία",
    "fontSize": {
      "label": "Μέγεθος Γραμματοσειράς",
      "small": "Μικρό",
      "medium": "Μεσαίο",
      "large": "Μεγάλο",
      "xLarge": "Πολύ Μεγάλο"
    },
    "lineSpacing": {
      "label": "Διάστιχο",
      "tight": "Στενό",
      "normal": "Κανονικό",
      "relaxed": "Χαλαρό"
    },
    "messageBubbles": {
      "label": "Συννεφάκια Μηνυμάτων",
      "style": {
        "label": "Στυλ",
        "bordered": "Με Περίγραμμα",
        "filled": "Γεμάτο",
        "minimal": "Ελάχιστο"
      },
      "cornerRadius": {
        "label": "Ακτίνα Γωνίας",
        "sharp": "Οξεία",
        "rounded": "Στρογγυλεμένη",
        "pill": "Χάπι"
      },
      "maxWidth": {
        "label": "Μέγιστο Πλάτος",
        "compact": "Συμπαγές",
        "normal": "Κανονικό",
        "wide": "Πλατύ"
      },
      "padding": {
        "label": "Εσωτερικό Κενό",
        "compact": "Συμπαγές",
        "normal": "Κανονικό",
        "spacious": "Ευρύχωρο"
      }
    },
    "layout": {
      "label": "Διάταξη",
      "messageSpacing": "Απόσταση Μηνυμάτων",
      "tight": "Στενό",
      "normal": "Κανονικό",
      "relaxed": "Χαλαρό"
    },
    "avatar": {
      "shape": {
        "label": "Σχήμα Avatar",
        "circle": "Κύκλος",
        "rounded": "Στρογγυλεμένο",
        "hidden": "Κρυφό"
      },
      "size": {
        "label": "Μέγεθος Avatar",
        "small": "Μικρό",
        "medium": "Μεσαίο",
        "large": "Μεγάλο"
      }
    },
    "colors": {
      "label": "Χρώματα",
      "userBubble": "Χρώμα Συννεφάκια Χρήστη",
      "assistantBubble": "Χρώμα Συννεφάκια Βοηθού",
      "userBubbleHex": "Hex Παράκαμψη Συννεφάκια Χρήστη",
      "assistantBubbleHex": "Hex Παράκαμψη Συννεφάκια Βοηθού",
      "neutral": "Ουδέτερο",
      "accent": "Έμφαση",
      "info": "Πληροφορία",
      "secondary": "Δευτερεύον",
      "warning": "Προειδοποίηση",
      "textColors": "Text Colors",
      "messageTextHex": "Χρώμα ενσωματωμένης παράθεσης",
      "plainTextHex": "Plain Text Color",
      "italicTextHex": "Italic Text Color",
      "quotedTextHex": "Χρώμα παράθεσης μπλοκ",
      "inlineCodeTextHex": "Χρώμα ενσωματωμένου κώδικα"
    },
    "backgroundTransparency": {
      "label": "Φόντο & Διαφάνεια",
      "backgroundDim": "Σκοτείνιασμα Φόντου",
      "backgroundBlur": "Θόλωμα Φόντου",
      "bubbleBlur": "Θόλωμα Συννεφάκια",
      "none": "Κανένα",
      "light": "Ελαφρύ",
      "medium": "Μεσαίο",
      "heavy": "Βαρύ",
      "bubbleOpacity": "Αδιαφάνεια Συννεφάκια"
    },
    "textColorMode": {
      "label": "Λειτουργία Χρώματος Κειμένου",
      "auto": "Αυτόματο",
      "light": "Ανοιχτό",
      "dark": "Σκοτεινό"
    },
    "preview": {
      "label": "Προεπισκόπηση",
      "generic": "Γενική",
      "live": "Ζωντανή"
    },
    "extra": {
      "reset": "Επαναφορά"
    }
  },
  "colorCustomization": {
    "tokens": {
      "surface": "Επιφάνεια",
      "surfaceDesc": "Φόντα σελίδων",
      "surfaceEl": "Ανυψωμένη Επιφάνεια",
      "surfaceElDesc": "Κάρτες, παράθυρα, ανυψωμένα στοιχεία",
      "nav": "Πλοήγηση",
      "navDesc": "Πάνω & κάτω μπάρες",
      "foreground": "Προσκήνιο",
      "foregroundDesc": "Περιγράμματα, επικαλύψεις, πλοήγηση και στοιχεία διεπαφής",
      "appText": "Κείμενο εφαρμογής",
      "appTextDesc": "Κύριο κείμενο και ετικέτες διεπαφής",
      "appTextMuted": "Δευτερεύον κείμενο",
      "appTextMutedDesc": "Δευτερεύον κείμενο και βοηθητικές περιγραφές",
      "appTextSubtle": "Διακριτικό κείμενο",
      "appTextSubtleDesc": "Υποδείξεις, βοηθητικό κείμενο και κείμενο θέσης",
      "accent": "Έμφαση",
      "accentDesc": "Κύριες ενέργειες, επιτυχία",
      "info": "Πληροφορία",
      "infoDesc": "Ενημερωτικές καταστάσεις, σύνδεσμοι",
      "warning": "Προειδοποίηση",
      "warningDesc": "Καταστάσεις προσοχής, ειδοποιήσεις",
      "danger": "Κίνδυνος",
      "dangerDesc": "Καταστροφικές ενέργειες, σφάλματα",
      "secondary": "Δευτερεύον",
      "secondaryDesc": "Λειτουργίες AI, δημιουργικά εργαλεία"
    },
    "presetsLabel": "Προεπιλογές",
    "customPresetsLabel": "Προσαρμοσμένες Προεπιλογές",
    "previewLabel": "Προεπισκόπηση",
    "settingsCardsLabel": "Κάρτες ρυθμίσεων",
    "settingsCardsOpacity": "Αδιαφάνεια καρτών",
    "settingsCardsOpacityDesc": "Ελέγχει πόσο διαφανείς φαίνονται οι κάρτες ρυθμίσεων και οι γραμμές λίστας.",
    "importButton": "Εισαγωγή",
    "exportButton": "Εξαγωγή",
    "resetAllButton": "Επαναφορά Όλων",
    "presets": {
      "defaultDark": "Προεπιλεγμένο Σκοτεινό",
      "midnightBlue": "Μεσονύχτιο Μπλε",
      "warmEarth": "Ζεστή Γη",
      "purpleHaze": "Μωβ Ομίχλη",
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
      "monochrome": "Μονόχρωμο"
    },
    "groups": {
      "backgrounds": "Φόντα",
      "content": "Περιεχόμενο",
      "semantic": "Σημασιολογικά"
    },
    "extra": {
      "surface": "Επιφάνεια",
      "surfaceDesc": "Φόντα σελίδων",
      "surfaceEl": "Ανυψωμένη Επιφάνεια",
      "surfaceElDesc": "Κάρτες, παράθυρα, ανυψωμένα στοιχεία",
      "nav": "Πλοήγηση",
      "navDesc": "Πάνω & κάτω μπάρες",
      "fg": "Προσκήνιο",
      "fgDesc": "Περιγράμματα, επικαλύψεις, πλοήγηση και στοιχεία διεπαφής",
      "appText": "Κείμενο εφαρμογής",
      "appTextDesc": "Κύριο κείμενο και ετικέτες διεπαφής",
      "appTextMuted": "Δευτερεύον κείμενο",
      "appTextMutedDesc": "Δευτερεύον κείμενο και βοηθητικές περιγραφές",
      "appTextSubtle": "Διακριτικό κείμενο",
      "appTextSubtleDesc": "Υποδείξεις, βοηθητικό κείμενο και κείμενο θέσης",
      "accent": "Έμφαση",
      "accentDesc": "Κύριες ενέργειες, επιτυχία",
      "info": "Πληροφορία",
      "infoDesc": "Ενημερωτικές καταστάσεις, σύνδεσμοι",
      "warning": "Προειδοποίηση",
      "warningDesc": "Καταστάσεις προσοχής, ειδοποιήσεις",
      "danger": "Κίνδυνος",
      "dangerDesc": "Καταστροφικές ενέργειες, σφάλματα",
      "secondary": "Δευτερεύον"
    }
  },
  "dynamicMemory": {
    "page": {
      "info": "Η Δυναμική Μνήμη συνοψίζει αυτόματα τις συνομιλίες για αποτελεσματική διατήρηση του πλαισίου. Επιλέξτε μια προεπιλογή ή ρυθμίστε λεπτομερώς τις ρυθμίσεις για τις ανάγκες σας.",
      "disabledDirectTitle": "Η δυναμική μνήμη είναι απενεργοποιημένη για απευθείας συνομιλίες",
      "disabledDirectDescription": "Ενεργοποιήστε τον διακόπτη στην καρτέλα Απευθείας Συνομιλίες. Οι ομαδικές συνομιλίες χρησιμοποιούν λειτουργία μνήμης ανά συνεδρία.",
      "directChats": "Απευθείας Συνομιλίες",
      "groupChats": "Ομαδικές Συνομιλίες",
      "enableDirectChats": "Ενεργοποίηση για Απευθείας Συνομιλίες",
      "groupChatsInfo": "Οι ομαδικές συνομιλίες χρησιμοποιούν λειτουργία μνήμης ανά συνεδρία. Ενεργοποιήστε τη δυναμική μνήμη στις ρυθμίσεις κάθε ομάδας. Αυτές οι ρυθμίσεις ελέγχουν τη συμπεριφορά της δυναμικής μνήμης.",
      "memoryProfile": "Προφίλ Μνήμης",
      "customSettings": "Προσαρμοσμένες ρυθμίσεις - ρυθμίστε τις τιμές στις Σύνθετες Επιλογές παρακάτω.",
      "contextEnrichment": "Εμπλουτισμός Πλαισίου",
      "experimental": "Πειραματικό",
      "contextEnrichmentDescription": "Χρησιμοποιεί πρόσφατα μηνύματα για πιο έξυπνη ανάκτηση μνήμης",
      "advancedOptions": "Σύνθετες Επιλογές",
      "advancedOptionsDescription": "Λεπτομερής ρύθμιση συμπεριφοράς μνήμης",
      "summaryInterval": "Διάστημα Σύνοψης",
      "summaryIntervalDescription": "Μηνύματα μεταξύ συνόψεων",
      "maxMemoryEntries": "Μέγιστες Εγγραφές Μνήμης",
      "maxMemoryEntriesDescription": "Μέγιστος αριθμός αποθηκευμένων αναμνήσεων",
      "hotMemoryBudget": "Προϋπολογισμός Ενεργής Μνήμης",
      "hotMemoryBudgetDescription": "Όριο token για ενεργές αναμνήσεις",
      "relevanceThreshold": "Κατώφλι Συνάφειας",
      "relevanceThresholdDescription": "Ελάχιστη ομοιότητα για ανάκτηση",
      "retrievalMode": "Λειτουργία Ανάκτησης",
      "retrievalModeSmart": "Έξυπνη",
      "retrievalModeCosine": "Cosine",
      "retrievalModeDescription": "Η Έξυπνη συνδυάζει συνάφεια με πρόσφατα/συχνότητα. Η Cosine χρησιμοποιεί καθαρή κορυφαία ομοιότητα.",
      "retrievalLimit": "Όριο Ανάκτησης",
      "retrievalLimitDescription": "Μέγιστες αναμνήσεις που επιλέγονται ανά σειρά",
      "decayRate": "Ρυθμός Φθοράς",
      "decayRateDescription": "Πόσο γρήγορα εξασθενεί η σημασία",
      "coldStorageThreshold": "Κατώφλι Ψυχρής Αποθήκευσης",
      "coldStorageThresholdDescription": "Πότε οι αναμνήσεις μετακινούνται στο αρχείο",
      "sharedSettings": "Κοινές Ρυθμίσεις",
      "summarisationModel": "Μοντέλο Σύνοψης",
      "selectedModel": "Επιλεγμένο Μοντέλο",
      "useGlobalDefaultModel": "Χρήση καθολικού προεπιλεγμένου μοντέλου",
      "noModelsAvailable": "Δεν υπάρχουν διαθέσιμα μοντέλα",
      "summarisationModelDescription": "Χρησιμοποιείται για σύνοψη συνομιλιών",
      "modelManagement": "Διαχείριση Μοντέλων",
      "testModel": "Δοκιμή Μοντέλου",
      "downloadModel": "Λήψη Μοντέλου",
      "delete": "Διαγραφή",
      "embeddingModel": "Μοντέλο Embedding",
      "tokenCapacity": "Χωρητικότητα Token",
      "tokenCapacityDescription": "Υψηλότερες τιμές = καλύτερη μνήμη για μεγαλύτερες συνομιλίες",
      "keepModelLoaded": "Διατήρηση Μοντέλου Φορτωμένου",
      "keepModelLoadedDescription": "Κρατά το μοντέλο embedding + tokenizer στη μνήμη για αποφυγή επιβάρυνσης επαναφόρτωσης",
      "installedModel": "Εγκατεστημένο μοντέλο: {{version}} ({{tokens}} μέγιστα token)",
      "downloadEmbeddingModel": "Λήψη Μοντέλου Embedding",
      "downloadEmbeddingDescription": "Επιλέξτε ποια έκδοση θα κατεβάσετε. Οι εγκατεστημένες εκδόσεις είναι απενεργοποιημένες.",
      "downloadVersion": "Λήψη {{version}}",
      "downloadV2Description": "Βελτιστοποιημένο για ακρίβεια και ανάκληση μεγάλου πλαισίου",
      "downloadV3Description": "Τελευταία ποιότητα embedding",
      "installed": "Εγκατεστημένο",
      "selectModel": "Επιλογή Μοντέλου",
      "searchModels": "Αναζήτηση μοντέλων...",
      "deleteEmbeddingTitle": "Διαγραφή μοντέλου {{version}};",
      "deleteEmbeddingMessage": "Είστε σίγουροι ότι θέλετε να διαγράψετε το {{version}}; Μπορείτε να το κατεβάσετε ξανά αργότερα.",
      "msgsUnit": "μνμ",
      "entriesUnit": "εγγραφές",
      "tokensUnit": "token",
      "itemsUnit": "στοιχεία",
      "perCycleUnit": "/ κύκλο"
    },
    "presets": {
      "minimal": "ελάχιστο",
      "balanced": "ισορροπημένο",
      "comprehensive": "ολοκληρωμένο",
      "minimalDesc": "Γρήγορο και αποδοτικό. Κρατά μόνο τις βασικές αναμνήσεις.",
      "balancedDesc": "Καλός συνδυασμός διατήρησης πλαισίου και απόδοσης.",
      "comprehensiveDesc": "Μέγιστο πλαίσιο. Ιδανικό για μεγάλες, λεπτομερείς συνομιλίες."
    },
    "presetInfo": {
      "minimal": "Γρήγορο και αποδοτικό. Κρατά μόνο τις βασικές αναμνήσεις.",
      "balanced": "Καλός συνδυασμός διατήρησης πλαισίου και απόδοσης.",
      "comprehensive": "Μέγιστο πλαίσιο. Ιδανικό για μεγάλες, λεπτομερείς συνομιλίες."
    }
  },
  "helpMeReply": {
    "page": {
      "info": "Το Βοήθησέ με να Απαντήσω δημιουργεί προτάσεις με βάση το ιστορικό συνομιλίας για το επόμενο μήνυμά σας. Ρυθμίστε το μοντέλο και το στυλ απάντησης παρακάτω."
    },
    "labels": {
      "replyModel": "Μοντέλο Απάντησης",
      "selectedModel": "Επιλεγμένο Μοντέλο",
      "useAppDefault": "Χρήση προεπιλογής εφαρμογής{{model}}",
      "useAppDefaultBase": "Χρήση προεπιλογής εφαρμογής",
      "noModelsAvailable": "Δεν υπάρχουν διαθέσιμα μοντέλα",
      "replyModelDescription": "Μοντέλο AI για δημιουργία προτάσεων απάντησης",
      "streamingOutput": "Ροή Εξόδου",
      "streamingDescription": "Εμφάνιση προτάσεων καθώς δημιουργούνται",
      "maxTokens": "Μέγιστα Token",
      "maxTokensDescription": "Μέγιστο μήκος προτάσεων",
      "conversationalHint": "Οι προτάσεις θα γραφτούν ως φυσικός διάλογος, κατάλληλος για χαλαρές συνομιλίες.",
      "roleplayHint": "Οι προτάσεις θα περιλαμβάνουν στοιχεία roleplay όπως *ενέργειες* και αφηγηματικές περιγραφές.",
      "footerInfo": "Αυτή η ρύθμιση ισχύει καθολικά σε όλες τις συνομιλίες. Χαμηλότερος αριθμός token δημιουργεί συντομότερες, ταχύτερες προτάσεις ενώ υψηλότερος αριθμός επιτρέπει πιο λεπτομερείς απαντήσεις.",
      "selectReplyModel": "Επιλογή Μοντέλου Απάντησης",
      "searchModels": "Αναζήτηση μοντέλων..."
    },
    "sectionTitles": {
      "modelConfiguration": "Ρύθμιση Μοντέλου",
      "responseStyle": "Στυλ Απάντησης"
    },
    "responseStyle": {
      "conversational": "Συνομιλιακό",
      "conversationalDesc": "Φυσικός, χαλαρός τόνος",
      "roleplay": "Roleplay",
      "roleplayDesc": "Ενέργειες εντός χαρακτήρα"
    },
    "extra": {
      "conversational": "Συνομιλιακό",
      "roleplay": "Roleplay"
    }
  },
  "imageGeneration": {
    "promptPlaceholder": "Περιγράψτε την εικόνα που θέλετε να δημιουργήσετε...",
    "labels": {
      "model": "ΜΟΝΤΕΛΟ",
      "prompt": "ΠΡΟΤΡΟΠΗ",
      "size": "ΜΕΓΕΘΟΣ",
      "quality": "ΠΟΙΟΤΗΤΑ",
      "style": "ΣΤΥΛ",
      "searchModels": "Αναζήτηση μοντέλων...",
      "selectAvatarModel": "Επιλέξτε Μοντέλο Avatar",
      "selectSceneModel": "Επιλέξτε Μοντέλο σκηνής",
      "selectWriterModel": "Επιλογή μοντέλου συγγραφέα σκηνών",
      "useFirstAvailable": "Χρησιμοποιήστε το πρώτο διαθέσιμο μοντέλο",
      "useFirstCompatible": "Χρήση του πρώτου συμβατού μοντέλου συγγραφέα"
    },
    "mode": {
      "title": "Συμπεριφορά",
      "description": "Επιλέξτε πώς θα χειρίζονται τα prompts σκηνής που εντοπίζονται στην έξοδο του μοντέλου.",
      "auto": "Αυτόματα",
      "autoDescription": "Δημιούργησε αμέσως εικόνα σκηνής όταν το μοντέλο δώσει prompt σκηνής.",
      "askFirst": "Να ζητείται πρώτα",
      "askFirstDescription": "Εμφάνισε το ανιχνευμένο prompt σκηνής και περίμενε την έγκρισή σου πριν δημιουργηθεί εικόνα.",
      "manual": "Χειροκίνητα",
      "manualDescription": "Αγνόησε τα prompts σκηνής από τις απαντήσεις του μοντέλου. Χρησιμοποίησε μόνο ενέργειες που ξεκινά ο χρήστης."
    },
    "empty": {
      "title": "Δεν υπάρχουν Μοντέλα Εικόνας",
      "description": "Προσθέστε μοντέλο δημιουργίας εικόνας από τη σελίδα Μοντέλα για να ξεκινήσετε."
    },
    "sections": {
      "avatar": {
        "title": "Γενιά Avatar",
        "description": "Προεπιλεγμένο μοντέλο που χρησιμοποιείται κατά τη δημιουργία avatar από το εργαλείο επιλογής avatar ή σχετικές ροές εικόνας προφίλ."
      },
      "scene": {
        "title": "Γενιά Σκηνής",
        "description": "Δεσμευμένο μοντέλο για εικόνες σκηνής που δημιουργούνται από περιβάλλον συνομιλίας ή προτροπές σκηνής."
      },
      "writer": {
        "title": "Συγγραφέας σκηνών",
        "description": "Δεσμευμένο πολυτροπικό μοντέλο κειμένου για σύνταξη προτροπών σκηνής και περιγραφών αναφοράς σχεδιασμού από το περιεχόμενο συνομιλίας, τα άβαταρ και τις εικόνες αναφοράς."
      }
    },
    "extra": {
      "avatarGeneration": "Δημιουργία Avatar",
      "sceneGeneration": "Δημιουργία Σκηνής",
      "sceneWriter": "Συγγραφέας Σκηνών"
    }
  },
  "logs": {
    "sectionTitles": {
      "diagnostics": "Διαγνωστικά",
      "generate": "Παραγωγή",
      "copy": "Αντιγραφή"
    },
    "extra": {
      "debug": "ΕΝΤΟΠΙΣΜΟΣ",
      "info": "ΠΛΗΡΟΦΟΡΙΕΣ",
      "warn": "ΠΡΟΕΙΔΟΠΟΙΗΣΗ",
      "error": "ΣΦΑΛΜΑ"
    }
  },
  "developer": {
    "sectionTitles": {
      "testDataGenerators": "Γεννήτριες Δοκιμαστικών Δεδομένων",
      "storageMaintenance": "Συντήρηση Αποθήκευσης",
      "usageTracking": "Παρακολούθηση Χρήσης",
      "crashTesting": "Crash Testing",
      "environmentInfo": "Πληροφορίες Περιβάλλοντος"
    },
    "testData": {
      "generateCharacter": "Δημιουργία Δοκιμαστικού Χαρακτήρα",
      "generateCharacterDesc": "Δημιουργία ενός δοκιμαστικού χαρακτήρα",
      "generatePersona": "Δημιουργία Δοκιμαστικής Περσόνας",
      "generatePersonaDesc": "Δημιουργία μιας δοκιμαστικής περσόνας",
      "generateSession": "Δημιουργία Δοκιμαστικής Συνεδρίας",
      "generateSessionDesc": "Δημιουργία δοκιμαστικής συνεδρίας συνομιλίας με υπάρχοντα χαρακτήρα",
      "generateBulk": "Μαζική Δημιουργία Δοκιμαστικών Δεδομένων",
      "generateBulkDesc": "Δημιουργία 3 χαρακτήρων και 2 περσονών"
    },
    "storageMaintenance": {
      "optimizeDb": "Βελτιστοποίηση Βάσης Δεδομένων",
      "optimizeDbDesc": "Εφαρμογή PRAGMAs και εκτέλεση VACUUM (μόνο κινητό)",
      "backupLegacy": "Αντίγραφο & Αφαίρεση Παλαιών Αρχείων",
      "backupLegacyDesc": "Μεταφορά παλαιών αρχείων .bin αποθήκευσης σε φάκελο αντιγράφου"
    },
    "usageTracking": {
      "recalculateAll": "Επανυπολογισμός Όλων των Κοστών Χρήσης",
      "recalculateAllDesc": "Ανάκτηση τιμολόγησης και επανυπολογισμός κοστών για όλες τις εγγραφές χρήσης OpenRouter"
    },
    "crashTesting": {
      "forceCrash": "Διακοπή εφαρμογής τώρα",
      "forceCrashDesc": "Τερματίζει αμέσως τη διαδικασία εγγενούς εφαρμογής για να δοκιμάσει τον εντοπισμό σφαλμάτων",
      "forceCrashConfirm": "Αυτό θα διακόψει αμέσως την εφαρμογή για να δοκιμάσει τον ανιχνευτή σύγκρουσης. Συνεχίζω;"
    },
    "environmentInfo": {
      "mode": "Λειτουργία",
      "devMode": "Λειτουργία Ανάπτυξης",
      "viteVersion": "Έκδοση Vite"
    },
    "status": {
      "testCharacterCreated": "✓ Ο δοκιμαστικός χαρακτήρας δημιουργήθηκε επιτυχώς",
      "testPersonaCreated": "✓ Η δοκιμαστική περσόνα δημιουργήθηκε επιτυχώς",
      "testSessionCreated": "✓ Η δοκιμαστική συνεδρία δημιουργήθηκε: {{id}}",
      "generatingBulkData": "Δημιουργία μαζικών δοκιμαστικών δεδομένων...",
      "bulkDataCreated": "✓ Μαζικά δοκιμαστικά δεδομένα δημιουργήθηκαν: 3 χαρακτήρες, 2 περσόνες",
      "creatingBenchmarkChat": "Δημιουργία δοκιμαστικής συνεδρίας benchmark...",
      "seededBenchmarkReady": "✓ Benchmark έτοιμο: {{name}} / {{id}}",
      "creatingBenchmarkGroupChat": "Δημιουργία δοκιμαστικής ομαδικής συνομιλίας benchmark...",
      "seededGroupBenchmarkReady": "✓ Ομαδικό benchmark έτοιμο: {{id}}",
      "dbOptimized": "✓ Η βάση δεδομένων βελτιστοποιήθηκε",
      "recalculatingCosts": "Επανυπολογισμός κοστών χρήσης... Αυτό μπορεί να πάρει λίγο χρόνο.",
      "toursReset": "✓ Όλες οι ξεναγήσεις επαναφέρθηκαν — θα εμφανιστούν ξανά στην επόμενη επίσκεψη",
      "crashingApp": "Διακοπή εφαρμογής..."
    },
    "errors": {
      "noCharacters": "Δεν υπάρχουν διαθέσιμοι χαρακτήρες. Δημιουργήστε πρώτα έναν δοκιμαστικό χαρακτήρα.",
      "createCharacterFailed": "Αποτυχία δημιουργίας δοκιμαστικού χαρακτήρα: {{error}}",
      "createPersonaFailed": "Αποτυχία δημιουργίας δοκιμαστικής περσόνας: {{error}}",
      "createSessionFailed": "Αποτυχία δημιουργίας δοκιμαστικής συνεδρίας: {{error}}",
      "createBulkFailed": "Αποτυχία δημιουργίας μαζικών δοκιμαστικών δεδομένων: {{error}}",
      "createBenchmarkFailed": "Αποτυχία δημιουργίας συνεδρίας benchmark: {{error}}",
      "createGroupBenchmarkFailed": "Αποτυχία δημιουργίας ομαδικής συνεδρίας benchmark: {{error}}",
      "dbOptimizeFailed": "Αποτυχία βελτιστοποίησης βάσης δεδομένων: {{error}}",
      "backupFailed": "Αποτυχία αντιγράφου ασφαλείας: {{error}}",
      "openRouterKeyMissing": "Δεν βρέθηκε κλειδί API OpenRouter. Ρυθμίστε το στις Ρυθμίσεις > Πάροχοι πρώτα.",
      "recalculationFailed": "Αποτυχία επανυπολογισμού: {{error}}",
      "resetToursFailed": "Αποτυχία επαναφοράς ξεναγήσεων: {{error}}",
      "crashFailed": "Αποτυχία διακοπής εφαρμογής: {{error}}"
    },
    "onboarding": {
      "title": "Εισαγωγή",
      "resetTours": "Επαναφορά όλων των ξεναγήσεων",
      "resetToursDesc": "Καθαρίζει την κατάσταση για κάθε ξενάγηση εισαγωγής ώστε να ξαναπαίξει στην επόμενη επίσκεψη."
    },
    "benchmarks": {
      "createChat": "Δημιουργία δοκιμαστικής συνομιλίας benchmark",
      "createChatDesc": "Δημιουργεί χαρακτήρα δυναμικής μνήμης, αρχική σκηνή και μια συνεδρία δοκιμής 20 μηνυμάτων, και στη συνέχεια την ανοίγει.",
      "createGroupChat": "Δημιουργία δοκιμαστικής ομαδικής συνομιλίας benchmark",
      "createGroupChatDesc": "Δημιουργεί ομαδική συνομιλία δυναμικής μνήμης με τρεις χαρακτήρες benchmark και 30 μηνύματα, και στη συνέχεια την ανοίγει."
    },
    "extra": {
      "testCharacter": "Δοκιμαστικός Χαρακτήρας",
      "testCharacterDesc": "Ένας δοκιμαστικός χαρακτήρας για σκοπούς ανάπτυξης.",
      "testScene": "Μια απλή δοκιμαστική σκηνή για ανάπτυξη",
      "testPersona": "Δοκιμαστική Περσόνα",
      "testPersonaDesc": "Μια δοκιμαστική περσόνα για ανάπτυξη",
      "successChar": "✓ Ο δοκιμαστικός χαρακτήρας δημιουργήθηκε επιτυχώς",
      "successPersona": "✓ Η δοκιμαστική περσόνα δημιουργήθηκε επιτυχώς",
      "successSession": "✓ Η δοκιμαστική συνεδρία δημιουργήθηκε: {{id}}",
      "successBulk": "✓ Μαζικά δοκιμαστικά δεδομένα δημιουργήθηκαν: 3 χαρακτήρες, 2 περσόνες",
      "errorCharAvailable": "Δεν υπάρχουν διαθέσιμοι χαρακτήρες. Δημιουργήστε πρώτα έναν δοκιμαστικό χαρακτήρα.",
      "generatingBulk": "Δημιουργία μαζικών δοκιμαστικών δεδομένων..."
    }
  },
  "embeddingDownload": {
    "capacityOptions": {
      "oneK": "1K tokens",
      "oneKDesc": "Καλύτερο για γρήγορες απαντήσεις",
      "twoK": "2K tokens",
      "twoKDesc": "Ισορροπημένη απόδοση",
      "fourK": "4K tokens",
      "fourKDesc": "Μέγιστο πλαίσιο"
    },
    "extra": {
      "status": "Λήψη..."
    }
  },
  "embeddingTest": {
    "categories": {
      "semanticSimilarity": "Σημασιολογική Ομοιότητα",
      "dissimilarityCheck": "Έλεγχος Ανομοιότητας",
      "roleplayContext": "Πλαίσιο Roleplay"
    },
    "extra": {
      "placeholder": "Εισάγετε κείμενο για embedding..."
    }
  },
  "discovery": {
    "tabs": {
      "forYou": "Για Εσάς",
      "trending": "Δημοφιλή",
      "popular": "Δημοφιλέστερα",
      "new": "Νέα"
    },
    "searchPlaceholder": "Αναζήτηση χαρακτήρων...",
    "viewAll": "Προβολή Όλων",
    "errorTitle": "Κάτι πήγε στραβά",
    "noCardsFound": "Δεν βρέθηκαν κάρτες",
    "sections": {
      "trendingNow": "Τάσεις Τώρα",
      "trendingSubtitle": "Δημοφιλή αυτή την εβδομάδα",
      "mostPopular": "Πιο Δημοφιλή",
      "popularSubtitle": "Αγαπημένα κοινότητας",
      "freshArrivals": "Νέες Αφίξεις",
      "freshSubtitle": "Μόλις προστέθηκαν"
    },
    "browse": {
      "newArrivals": "Νέες Αφίξεις",
      "freshCharacters": "Νέοι χαρακτήρες",
      "noCharactersFound": "Δεν βρέθηκαν χαρακτήρες",
      "noCharactersSubtitle": "Ελέγξτε ξανά αργότερα για νέο περιεχόμενο"
    },
    "sort": {
      "mostLiked": "Περισσότερα Like",
      "mostDownloaded": "Περισσότερες Λήψεις",
      "mostViewed": "Περισσότερες Προβολές",
      "mostMessages": "Περισσότερα Μηνύματα",
      "newestFirst": "Νεότερα Πρώτα",
      "recentlyUpdated": "Πρόσφατα Ενημερωμένα",
      "nameAZ": "Όνομα (Α-Ω)"
    },
    "sortBy": "Ταξινόμηση Κατά",
    "resultsUnit": "χαρακτήρες",
    "detail": {
      "share": "Κοινοποίηση",
      "nsfwOverlay": "Περιεχόμενο NSFW",
      "nsfwBadge": "NSFW",
      "originalBadge": "Πρωτότυπο",
      "lorebookBadge": "Βιβλίο Γνώσης",
      "alsoKnownAs": "Επίσης γνωστός ως:",
      "followersUnit": "ακόλουθοι",
      "sections": {
        "description": "Περιγραφή",
        "tokenUsage": "Χρήση Tokens",
        "startingScenes": "Αρχικές Σκηνές",
        "scenario": "Σενάριο",
        "personality": "Προσωπικότητα",
        "stats": "Στατιστικά",
        "tags": "Ετικέτες",
        "author": "Συγγραφέας"
      },
      "tokensTotalLabel": "σύνολο",
      "tokens": {
        "description": "Περιγραφή",
        "personality": "Προσωπικότητα",
        "scenario": "Σενάριο",
        "firstMessage": "Πρώτο Μήνυμα",
        "scenes": "Σκηνές",
        "examples": "Παραδείγματα",
        "systemPrompt": "Προτροπή Συστήματος"
      },
      "sceneLabels": {
        "primary": "Κύρια",
        "alternate": "Εναλλακτική"
      },
      "stats": {
        "views": "Προβολές",
        "downloads": "Λήψεις",
        "messages": "Μηνύματα"
      },
      "downloaded": "Λήφθηκε",
      "startChat": "Έναρξη Συνομιλίας",
      "downloadCharacter": "Λήψη Χαρακτήρα",
      "downloading": "Λήψη...",
      "downloadSuccess": {
        "title": "Ο Χαρακτήρας Λήφθηκε!",
        "subtitle": "Προστέθηκε στη βιβλιοθήκη σας",
        "badge": "Αποθηκεύτηκε",
        "startChat": "Έναρξη Συνομιλίας",
        "startChatDesc": "Ανοίξτε την πρώτη σκηνή τώρα",
        "viewLibrary": "Προβολή στη Βιβλιοθήκη",
        "viewLibraryDesc": "Επεξεργασία, διαχείριση ή εξαγωγή αργότερα",
        "continueBrowsing": "Συνέχεια Περιήγησης",
        "continueBrowsingDesc": "Επιστροφή στην ανακάλυψη"
      },
      "errorTitle": "Σφάλμα",
      "errorSubtitle": "Αποτυχία φόρτωσης",
      "errorNotFound": "Ο χαρακτήρας δεν βρέθηκε",
      "defaultChatTitle": "Νέα Συνομιλία"
    },
    "errors": {
      "loadContent": "Αποτυχία φόρτωσης περιεχομένου",
      "searchFailed": "Αποτυχία αναζήτησης",
      "noCardPath": "Δεν παρέχεται διαδρομή κάρτας",
      "loadCharacter": "Αποτυχία φόρτωσης χαρακτήρα",
      "downloadCharacter": "Αποτυχία λήψης χαρακτήρα"
    },
    "search": {
      "placeholder": "Αναζήτηση χαρακτήρων, ετικετών, συγγραφέων...",
      "resultsUnit": "αποτελέσματα",
      "timingUnit": "ms",
      "recentSearches": "Πρόσφατες Αναζητήσεις",
      "clearAll": "Εκκαθάριση όλων",
      "trendingSearches": "Δημοφιλείς Αναζητήσεις",
      "trends": {
        "anime": "anime",
        "fantasy": "φαντασία",
        "romance": "ρομαντικό",
        "villain": "κακός",
        "adventure": "περιπέτεια",
        "comedy": "κωμωδία",
        "mystery": "μυστήριο",
        "sciFi": "sci-fi"
      },
      "tips": {
        "title": "Συμβουλές Αναζήτησης",
        "tip1": "Αναζήτηση κατά όνομα χαρακτήρα, συγγραφέα ή περιγραφή",
        "tip2": "Χρησιμοποιήστε ετικέτες όπως \"anime\", \"φαντασία\" ή \"ρομαντικό\"",
        "tip3": "Δοκιμάστε συγκεκριμένα χαρακτηριστικά όπως \"tsundere\" ή \"κακός\""
      },
      "loading": "Φόρτωση...",
      "loadMore": "Φόρτωση Περισσότερων",
      "noResults": "Δεν βρέθηκαν αποτελέσματα",
      "noResultsFor": "Δεν βρέθηκαν χαρακτήρες για",
      "noResultsHint": "Δοκιμάστε διαφορετικές λέξεις-κλειδιά ή περιηγηθείτε σε κατηγορίες"
    },
    "card": {
      "byAuthor": "από {{author}}",
      "ocBadge": "ΠΡ"
    }
  },
  "engine": {
    "gpuInsufficient": "Ανεπαρκής μνήμη GPU",
    "gpuFallbackDesc": "Αυτό το μοντέλο δεν χωράει στη μνήμη GPU. Εναλλαγή σε CPU (πιο αργό) ή ακύρωση;",
    "switchToCpu": "Εναλλαγή σε CPU",
    "abort": "Ακύρωση",
    "errors": {
      "providerNotFound": "Ο πάροχος μηχανής δεν βρέθηκε.",
      "engineOffline": "Η μηχανή είναι εκτός σύνδεσης ή μη προσβάσιμη.",
      "deleteCharacterFailed": "Αποτυχία διαγραφής χαρακτήρα.",
      "unknownCharacter": "Άγνωστο",
      "seedRequired": "Απαιτείται αρχική περιγραφή.",
      "characterNameRequired": "Απαιτείται όνομα χαρακτήρα.",
      "atLeastOneProvider": "Τουλάχιστον ένας πάροχος πρέπει να είναι ενεργοποιημένος.",
      "enableLlmProvider": "Ενεργοποιήστε τουλάχιστον έναν πάροχο LLM.",
      "modelRequired": "Απαιτείται μοντέλο για {{provider}}.",
      "apiKeyRequired": "Απαιτείται κλειδί API για {{provider}}.",
      "sendMessageFailed": "Αποτυχία αποστολής μηνύματος."
    },
    "status": {
      "connected": "Συνδεδεμένο",
      "offline": "Εκτός σύνδεσης",
      "needsSetup": "Απαιτείται Ρύθμιση"
    },
    "home": {
      "characters": "Χαρακτήρες",
      "newButton": "Νέο",
      "noCharactersFound": "Δεν βρέθηκαν χαρακτήρες.",
      "tokenUsage": "Χρήση Tokens",
      "totalTokens": "συνολικά tokens",
      "backgroundActivity": "Δραστηριότητα Παρασκηνίου",
      "quickActions": "Γρήγορες Ενέργειες",
      "configureProviders": "Ρύθμιση Παρόχων",
      "engineSettings": "Ρυθμίσεις Μηχανής",
      "chat": "Συνομιλία",
      "chatDesc": "Ξεκινήστε μια συνομιλία με αυτόν τον χαρακτήρα",
      "deleteCharacter": "Διαγραφή Χαρακτήρα",
      "deletingCharacter": "Διαγραφή...",
      "deleteDesc": "Μόνιμη αφαίρεση αυτού του χαρακτήρα",
      "character": "Χαρακτήρας",
      "never": "Ποτέ",
      "justNow": "Μόλις τώρα",
      "timeAgo": {
        "minutes": "Πριν {{n}} λεπτά",
        "hours": "Πριν {{n}} ώρες",
        "days": "Πριν {{n}} ημέρες"
      }
    },
    "tokens": {
      "input": "Είσοδος",
      "output": "Έξοδος"
    },
    "activity": {
      "synthesis": "Σύνθεση",
      "consolidation": "Ενοποίηση",
      "bm25Rebuild": "Ανοικοδόμηση BM25",
      "dripResearch": "Σταδιακή Έρευνα",
      "running": "Εκτελείται",
      "stopped": "Σταματημένο"
    },
    "setup": {
      "complete": "Η Ρύθμιση Ολοκληρώθηκε!",
      "completeMessage": "Η Μηχανή Lettuce σας έχει ρυθμιστεί και είναι έτοιμη.",
      "openDashboard": "Άνοιγμα Πίνακα Ελέγχου"
    },
    "welcome": {
      "title": "Καλώς ήρθατε στη Μηχανή Lettuce",
      "subtitle": "Ας ρυθμίσουμε τη μηχανή χαρακτήρων AI σας. Αυτό θα πάρει περίπου 2 λεπτά.",
      "feature1": "Η Μηχανή δίνει στους χαρακτήρες AI σας μόνιμη μνήμη, συναισθήματα, σχέσεις και πραγματική ταυτότητα.",
      "feature2": "Πρώτα, θα ρυθμίσουμε ένα LLM backend, μετά θα ρυθμίσουμε τις ρυθμίσεις μηχανής.",
      "getStarted": "Πάμε"
    },
    "config": {
      "activeProviders": "Ενεργοί Πάροχοι",
      "noModelSet": "Δεν έχει οριστεί μοντέλο",
      "defaultBadge": "Προεπιλογή",
      "noProvidersWarning": "Δεν έχουν ρυθμιστεί πάροχοι. Προσθέστε τουλάχιστον ένα LLM backend παρακάτω.",
      "addProvider": "Προσθήκη Παρόχου",
      "quickImport": "Γρήγορη εισαγωγή από τους παρόχους της εφαρμογής σας",
      "importButton": "Εισαγωγή",
      "fields": {
        "model": "Μοντέλο",
        "modelPlaceholder": "π.χ. claude-sonnet-4-5-20250929",
        "apiKey": "Κλειδί API",
        "apiKeyPlaceholder": "Εισάγετε το κλειδί API σας",
        "currentKey": "Τρέχον κλειδί:",
        "baseUrl": "Βασικό URL",
        "baseUrlPlaceholder": "http://localhost:11434",
        "maxTokens": "Μέγιστα Tokens",
        "temperature": "Θερμοκρασία"
      },
      "enableProvider": "Ενεργοποίηση Παρόχου",
      "setAsDefault": "Ορισμός ως Προεπιλογή",
      "defaultBackend": "Προεπιλεγμένο Backend",
      "remove": "Αφαίρεση",
      "saveChanges": "Αποθήκευση Αλλαγών",
      "saving": "Αποθήκευση...",
      "saved": "Αποθηκεύτηκε"
    },
    "providers": {
      "title": "Πάροχος LLM",
      "subtitle": "Η Μηχανή χρειάζεται τουλάχιστον ένα LLM backend για να λειτουργήσει. Ρυθμίστε έναν ή περισσότερους παρόχους παρακάτω.",
      "importFromProviders": "Εισαγωγή από τους παρόχους σας",
      "imported": "Εισήχθη",
      "use": "Χρήση",
      "saveContinue": "Αποθήκευση & Συνέχεια"
    },
    "settings": {
      "fields": {
        "dataDirectory": "Κατάλογος Δεδομένων",
        "logLevel": "Επίπεδο Καταγραφής",
        "maxHistory": "Μέγιστο Ιστορικό (γύροι συνομιλίας)"
      },
      "logLevels": {
        "debug": "ΕΝΤΟΠΙΣΜΟΣ",
        "info": "ΠΛΗΡΟΦΟΡΙΕΣ",
        "warning": "ΠΡΟΕΙΔΟΠΟΙΗΣΗ",
        "error": "ΣΦΑΛΜΑ"
      },
      "sections": {
        "engine": "Μηχανή",
        "backgroundLoops": "Βρόχοι Παρασκηνίου",
        "memory": "Μνήμη",
        "safety": "Ασφάλεια",
        "research": "Έρευνα"
      },
      "backgroundLoops": {
        "synthesis": "Σύνθεση (λεπτά)",
        "consolidation": "Ενοποίηση (λεπτά)",
        "bm25Rebuild": "Ανοικοδόμηση BM25 (λεπτά)",
        "dripResearch": "Σταδιακή Έρευνα (λεπτά)"
      },
      "memory": {
        "embeddingModel": "Μοντέλο Embedding",
        "maxRetrieval": "Μέγιστα Αποτελέσματα Ανάκτησης",
        "denseWeight": "Βάρος Dense",
        "bm25Weight": "Βάρος BM25",
        "graphWeight": "Βάρος Graph",
        "recencyBoost": "Ενίσχυση Πρόσφατων (ώρες)",
        "randomSurface": "Πιθανότητα Τυχαίας Εμφάνισης"
      },
      "safety": {
        "honestySection": "Ενότητα Ειλικρίνειας",
        "honestyDesc": "Συμπερίληψη ενότητας ειλικρίνειας στην προτροπή συστήματος",
        "userDataDeletion": "Διαγραφή Δεδομένων Χρήστη",
        "userDataDesc": "Να επιτρέπεται στους χρήστες να ζητούν διαγραφή δεδομένων"
      },
      "research": {
        "scrapeOnBoot": "Scrape κατά την Εκκίνηση",
        "scrapeDesc": "Εκτέλεση scrape έρευνας κατά την εκκίνηση μηχανής",
        "periodicInterval": "Περιοδικό Διάστημα (ώρες)"
      },
      "saveChanges": "Αποθήκευση Αλλαγών",
      "saving": "Αποθήκευση...",
      "saved": "Αποθηκεύτηκε"
    },
    "settingsStep": {
      "title": "Ρυθμίσεις Μηχανής",
      "subtitle": "Ρυθμίστε τις καθολικές ρυθμίσεις μηχανής. Όλες έχουν λογικές προεπιλογές — μη διστάσετε να τις παραλείψετε.",
      "completingSetup": "Ολοκλήρωση Ρύθμισης...",
      "completeSetup": "Ολοκλήρωση Ρύθμισης"
    },
    "chat": {
      "sendMessage": "Στείλτε μήνυμα...",
      "sendButton": "Αποστολή μηνύματος",
      "typeMessage": "Πληκτρολογήστε μήνυμα",
      "back": "Πίσω",
      "assistantTyping": "Ο βοηθός πληκτρολογεί",
      "fallbackName": "Συνομιλία"
    },
    "tagInput": {
      "addMore": "Προσθέστε περισσότερα...",
      "typeAndPressEnter": "Πληκτρολογήστε και πατήστε Enter"
    },
    "characterCreate": {
      "steps": {
        "identity": {
          "title": "Ταυτότητα",
          "aiGenerated": "Δημιουργημένο από AI",
          "nameLabel": "Όνομα *",
          "namePlaceholder": "Όνομα χαρακτήρα",
          "eraLabel": "Εποχή",
          "eraPlaceholder": "π.χ. σύγχρονη, Βικτωριανή",
          "roleLabel": "Ρόλος",
          "rolePlaceholder": "π.χ. Ντετέκτιβ, Επιστήμονας",
          "settingLabel": "Σκηνικό",
          "settingPlaceholder": "Περιγράψτε πού ζει ο χαρακτήρας (πρώτο πρόσωπο)...",
          "coreIdentityLabel": "Βασική Ταυτότητα",
          "coreIdentityPlaceholder": "Ποιος είναι αυτός ο χαρακτήρας στον πυρήνα του; (πρώτο πρόσωπο, 3-5 προτάσεις)",
          "backstoryLabel": "Ιστορικό",
          "backstoryPlaceholder": "Ιστορία ζωής και σημαντικά γεγονότα (πρώτο πρόσωπο)..."
        },
        "mode": {
          "title": "Δημιουργία Χαρακτήρα",
          "subtitle": "Δημιουργήστε χαρακτήρα με AI ή χτίστε τον από το μηδέν.",
          "aiBoost": "Ώθηση AI",
          "aiBoostDesc": "Περιγράψτε την ιδέα χαρακτήρα σας και η AI θα δημιουργήσει πλήρη ορισμό χαρακτήρα.",
          "nameOptional": "Όνομα (προαιρετικό)",
          "namePlaceholder": "π.χ. Μάρκος Κωνσταντίνου",
          "seedDescription": "Αρχική Περιγραφή *",
          "seedPlaceholder": "π.χ. πιανίστας τζαζ στο Χάρλεμ της δεκαετίας του 1950, φιλοσοφικός, αγαπά τις νυχτερινές συζητήσεις",
          "eraOptional": "Εποχή (προαιρετικό)",
          "eraPlaceholder": "π.χ. δεκαετία 1950, σύγχρονη, Βικτωριανή",
          "generating": "Δημιουργία...",
          "generateCharacter": "Δημιουργία Χαρακτήρα",
          "or": "ή",
          "startFromScratch": "Ξεκινήστε από το Μηδέν"
        },
        "personality": {
          "title": "Προσωπικότητα",
          "traits": "Χαρακτηριστικά Προσωπικότητας",
          "traitsPlaceholder": "π.χ. πνευματώδης, συμπονετικός, πεισματάρης",
          "speechPatterns": "Μοτίβα Ομιλίας",
          "formality": "Επισημότητα",
          "formal": "Επίσημος",
          "casual": "Χαλαρός",
          "texting": "Γραπτό μήνυμα",
          "verbosity": "Πολυλογία",
          "terse": "Λακωνικός",
          "medium": "Μεσαίος",
          "verbose": "Πολύλογος",
          "textStyle": "Στυλ Κειμένου",
          "dialect": "Διάλεκτος",
          "dialectPlaceholder": "π.χ. Κρητική, Βόρεια Ελληνική",
          "catchphrases": "Χαρακτηριστικές Φράσεις",
          "catchphrasesPlaceholder": "π.χ. Ε λοιπόν...",
          "vocabPreferences": "Προτιμήσεις Λεξιλογίου",
          "vocabPreferencesPlaceholder": "Λέξεις που προτιμούν",
          "vocabAvoidances": "Αποφυγές Λεξιλογίου",
          "vocabAvoidancesPlaceholder": "Λέξεις που αποφεύγουν",
          "fillerWords": "Λέξεις-Γεμίσματα",
          "fillerWordsPlaceholder": "π.χ. εε, δηλαδή, ξέρεις",
          "exampleQuotes": "Παραδείγματα Αποσπασμάτων",
          "exampleQuotesPlaceholder": "3-5 παραδείγματα γραμμών διαλόγου"
        },
        "world": {
          "title": "Κόσμος & Συμπεριφορά",
          "knowledgeDomains": "Τομείς Γνώσης",
          "knowledgeDomainsPlaceholder": "π.χ. ιστορία τζαζ, θεωρία μουσικής",
          "knowledgeBoundaries": "Όρια Γνώσης",
          "knowledgeBoundariesPlaceholder": "Θέματα που δεν γνωρίζουν",
          "researchSeeds": "Σπόροι Έρευνας",
          "researchSeedsPlaceholder": "Αρχικά θέματα για έρευνα υποβάθρου",
          "researchEnabled": "Ενεργή Έρευνα",
          "researchEnabledDesc": "Επιτρέπει τη συλλογή γνώσης στο παρασκήνιο",
          "physicalDescription": "Φυσική Περιγραφή",
          "physicalDescPlaceholder": "Φυσική εμφάνιση και τρόπος συμπεριφοράς...",
          "physicalHabits": "Φυσικές Συνήθειες",
          "physicalHabitsPlaceholder": "π.χ. χτυπά τα δάχτυλα, φτιάχνει τα γυαλιά",
          "idleBehaviors": "Συμπεριφορές Αδράνειας",
          "idleBehaviorsPlaceholder": "Τι κάνουν όταν δεν ασχολούνται",
          "timeBehaviors": "Συμπεριφορές Ανά Ώρα",
          "timePlaceholder": "Τι κάνουν κατά τη διάρκεια {{period}};",
          "earlyMorning": "Νωρίς το Πρωί",
          "morning": "Πρωί",
          "afternoon": "Απόγευμα",
          "evening": "Βράδυ",
          "night": "Νύχτα",
          "baselineEmotions": "Βασικά Συναισθήματα (Plutchik)",
          "emotionDesc": "Ορίστε τη βασική συναισθηματική γραμμή (0 = κανένα, 1 = μέγιστο)",
          "joy": "Χαρά",
          "trust": "Εμπιστοσύνη",
          "fear": "Φόβος",
          "surprise": "Έκπληξη",
          "sadness": "Λύπη",
          "disgust": "Αηδία",
          "anger": "Θυμός",
          "anticipation": "Αναμονή",
          "engineOverrides": "Παρακάμψεις Μηχανής",
          "backend": "Backend",
          "model": "Μοντέλο",
          "temperature": "Θερμοκρασία",
          "leaveEmpty": "Αφήστε κενό για προεπιλογή"
        },
        "review": {
          "title": "Ανασκόπηση",
          "subtitle": "Ελέγξτε τον χαρακτήρα σας πριν τη δημιουργία.",
          "edit": "Επεξεργασία",
          "notSet": "Δεν έχει οριστεί",
          "identitySection": "Ταυτότητα",
          "personalitySection": "Προσωπικότητα",
          "worldSection": "Κόσμος & Συμπεριφορά",
          "nameLabel": "Όνομα",
          "eraLabel": "Εποχή",
          "roleLabel": "Ρόλος",
          "settingLabel": "Σκηνικό",
          "coreIdentityLabel": "Βασική Ταυτότητα",
          "backstoryLabel": "Ιστορικό",
          "traitsLabel": "Χαρακτηριστικά",
          "formalityLabel": "Επισημότητα",
          "verbosityLabel": "Πολυλογία",
          "dialectLabel": "Διάλεκτος",
          "catchphrasesLabel": "Χαρακτηριστικές Φράσεις",
          "domainsLabel": "Τομείς",
          "boundariesLabel": "Όρια",
          "researchSeedsLabel": "Σπόροι Έρευνας",
          "researchLabel": "Έρευνα",
          "enabled": "Ενεργό",
          "disabled": "Ανενεργό",
          "physicalLabel": "Φυσική",
          "habitsLabel": "Συνήθειες",
          "idleLabel": "Αδράνεια",
          "timeBehaviorsLabel": "Συμπεριφορές Ανά Ώρα",
          "emotionsLabel": "Συναισθήματα",
          "configured": "Ρυθμισμένο",
          "backendLabel": "Backend",
          "modelLabel": "Μοντέλο",
          "temperatureLabel": "Θερμοκρασία",
          "creating": "Δημιουργία...",
          "createCharacter": "Δημιουργία Χαρακτήρα"
        }
      }
    }
  },
  "library": {
    "filterTitle": "Φίλτρο Βιβλιοθήκης",
    "filters": {
      "all": "Όλα",
      "characters": "Χαρακτήρες",
      "personas": "Περσόνες",
      "lorebooks": "Βιβλία Γνώσης",
      "images": "Εικόνες"
    },
    "emptyStates": {
      "all": {
        "title": "Η βιβλιοθήκη σας είναι κενή",
        "description": "Δημιουργήστε χαρακτήρες, περσόνες και βιβλία γνώσης για να τα δείτε εδώ"
      },
      "characters": {
        "title": "Δεν υπάρχουν χαρακτήρες ακόμα",
        "description": "Δημιουργήστε τον πρώτο σας χαρακτήρα για να ξεκινήσετε τη συνομιλία"
      },
      "personas": {
        "title": "Δεν υπάρχουν περσόνες ακόμα",
        "description": "Δημιουργήστε μια περσόνα για να προσαρμόσετε την ταυτότητα συνομιλίας σας"
      },
      "lorebooks": {
        "title": "Δεν υπάρχουν βιβλία γνώσης ακόμα",
        "description": "Τα βιβλία γνώσης δημιουργούνται μέσα από τις ρυθμίσεις ενός χαρακτήρα"
      }
    },
    "actions": {
      "startChat": "Έναρξη Συνομιλίας",
      "editCharacter": "Επεξεργασία Χαρακτήρα",
      "editPersona": "Επεξεργασία Περσόνας",
      "editLorebook": "Επεξεργασία Βιβλίου Γνώσης",
      "renameLorebook": "Μετονομασία Βιβλίου Γνώσης",
      "exportCharacter": "Εξαγωγή Χαρακτήρα",
      "exportPersona": "Εξαγωγή Περσόνας",
      "chatAppearance": "Εμφάνιση Συνομιλίας",
      "deleteCharacter": "Διαγραφή Χαρακτήρα",
      "deletePersona": "Διαγραφή Περσόνας",
      "deleteLorebook": "Διαγραφή Βιβλίου Γνώσης",
      "importLorebook": "Εισαγωγή Βιβλίου Γνώσης"
    },
    "imageLibrary": {
      "filters": {
        "all": "Όλα",
        "backgrounds": "Φόντα",
        "avatars": "Άβαταρ",
        "attachments": "Συνημμένα",
        "other": "Άλλα"
      },
      "searchPlaceholder": "Αναζήτηση με όνομα αρχείου, διαδρομή, αναγνωριστικό συνεδρίας ή οντότητας",
      "empty": {
        "title": "Δεν υπάρχουν εικόνες για αυτή την προβολή",
        "description": "Δοκίμασε άλλο φίλτρο ή όρο αναζήτησης. Η βιβλιοθήκη εμφανίζει μόνο εικόνες που είναι ήδη αποθηκευμένες τοπικά στην εφαρμογή."
      },
      "actions": {
        "sort": "Ταξινόμηση",
        "useThis": "Χρήση αυτής",
        "using": "Γίνεται χρήση...",
        "copyPath": "Αντιγραφή διαδρομής",
        "saving": "Αποθήκευση...",
        "download": "Λήψη",
        "delete": "Διαγραφή εικόνας",
        "deleting": "Διαγραφή..."
      },
      "active": "Ενεργό",
      "messages": {
        "loadFailed": "Αποτυχία φόρτωσης βιβλιοθήκης εικόνων",
        "saved": "Η εικόνα αποθηκεύτηκε",
        "downloadFailed": "Η λήψη απέτυχε",
        "useFailed": "Δεν ήταν δυνατή η χρήση αυτής της εικόνας",
        "deleted": "Η εικόνα διαγράφηκε",
        "deleteFailed": "Αποτυχία διαγραφής εικόνας"
      },
      "deleteConfirm": {
        "title": "Διαγραφή εικόνας;",
        "message": "Θέλεις σίγουρα να διαγράψεις το \"{{filename}}\"; Αυτό μπορεί να χαλάσει άβαταρ, φόντα συνομιλίας ή συνημμένα μηνυμάτων που το χρησιμοποιούν ακόμη."
      },
      "sort": {
        "newest": "Νεότερα",
        "largest": "Μεγαλύτερα",
        "name": "Όνομα"
      },
      "kinds": {
        "background": "Φόντο",
        "avatar": "Avatar",
        "attachment": "Συνημμένο",
        "stored": "Αποθηκευμένο"
      },
      "detailsTitle": "Λεπτομέρειες {{kind}}",
      "formatsLabel": "Μορφές",
      "storagePath": "Διαδρομή αποθήκευσης",
      "contextLabel": "Πλαίσιο",
      "contextLinkedFallback": "Συνδεδεμένο",
      "show": "Εμφάνιση",
      "hide": "Απόκρυψη",
      "contextRoles": {
        "character": "χαρακτήρας:",
        "session": "συνεδρία:",
        "role": "ρόλος:"
      },
      "downloadFormat": "Μορφή {{download}}",
      "unknownDate": "Άγνωστο",
      "clearSearch": "Καθαρισμός αναζήτησης",
      "copyFilename": "Αντιγραφή ονόματος αρχείου",
      "copyLabels": {
        "filename": "Όνομα αρχείου",
        "storagePath": "Διαδρομή αποθήκευσης"
      },
      "copy": {
        "copied": "{{label}} αντιγράφηκε",
        "failed": "Αποτυχία αντιγραφής {{label}}"
      }
    },
    "deleteConfirm": {
      "title": "Διαγραφή {{itemType}};",
      "message": "Είστε σίγουροι ότι θέλετε να διαγράψετε",
      "characterWarning": "Αυτό θα διαγράψει επίσης όλες τις συνεδρίες συνομιλίας με αυτόν τον χαρακτήρα."
    },
    "rename": {
      "title": "Μετονομασία Βιβλίου Γνώσης",
      "placeholder": "Εισάγετε νέο όνομα..."
    },
    "itemTypes": {
      "character": "Χαρακτήρας",
      "persona": "Περσόνα",
      "lorebook": "Βιβλίο Γνώσης"
    },
    "lorebookLabel": "Βιβλίο Γνώσης",
    "noDescriptionYet": "Χωρίς περιγραφή ακόμα",
    "errors": {
      "importLorebook": "Αποτυχία εισαγωγής βιβλίου γνώσης. {{error}}",
      "exportFailed": "Αποτυχία εξαγωγής"
    },
    "card": {
      "avatarAlt": "Avatar {{name}}"
    },
    "lorebookEditor": {
      "titleOverride": "Βιβλίο Γνώσης - {{name}}",
      "dragToReorder": "Σύρετε για αναδιάταξη",
      "aria": {
        "generateEntry": "Δημιουργία καταχώρησης βιβλίου γνώσης",
        "editLorebook": "Επεξεργασία βιβλίου γνώσης",
        "exportLorebook": "Εξαγωγή βιβλίου γνώσης"
      }
    }
  },
  "onboarding": {
    "loading": "Φόρτωση παρόχων...",
    "stepIndicator": "Βήμα {{current}} από {{total}}",
    "steps": {
      "provider": "Ρύθμιση Παρόχου",
      "model": "Ρύθμιση Μοντέλου",
      "memory": "Σύστημα Μνήμης",
      "stepNofM": "Βήμα {{current}} από {{total}}"
    },
    "provider": {
      "availableProviders": "Διαθέσιμοι Πάροχοι",
      "chooseProvider": "Επιλέξτε πάροχο",
      "titleMobile": "Επιλέξτε τον πάροχο AI σας",
      "descMobile": "Επιλέξτε πάροχο AI για να ξεκινήσετε. Τα κλειδιά API σας κρυπτογραφούνται με ασφάλεια στη συσκευή σας. Δεν απαιτείται εγγραφή λογαριασμού.",
      "configureProvider": "Ρύθμιση {{name}}",
      "connectProvider": "Σύνδεση {{name}}",
      "connectProviderDesc": "Επικολλήστε το κλειδί API σας παρακάτω για να ενεργοποιήσετε τις συνομιλίες. Χρειάζεστε κλειδί; Αποκτήστε ένα από τον πίνακα ελέγχου του παρόχου.",
      "localLLMs": "Τοπικά LLMs",
      "useLocalLLMs": "Θέλω να χρησιμοποιήσω τοπικά LLMs",
      "browseModelLibrary": "Περιήγηση Βιβλιοθήκης Μοντέλων",
      "browseModelLibraryDesc": "Αναζητήστε και κατεβάστε μοντέλα GGUF από το HuggingFace",
      "useOwnGguf": "Χρήση των δικών μου αρχείων GGUF",
      "useOwnGgufDesc": "Επιλέξτε μοντέλο GGUF και προαιρετικό αρχείο mmproj από τη συσκευή σας",
      "fields": {
        "displayLabel": "Εμφανιζόμενη Ετικέτα",
        "displayLabelHint": "Πώς θα εμφανίζεται αυτός ο πάροχος στα μενού σας",
        "displayLabelPlaceholder": "Ο {{name}} μου",
        "defaultLabelFallback": "Πάροχος",
        "apiKey": "Κλειδί API",
        "apiKeyOptional": "Κλειδί API (Προαιρετικό)",
        "apiKeyHint": "Τα κλειδιά κρυπτογραφούνται τοπικά",
        "apiKeyPlaceholderRemote": "sk-...",
        "apiKeyPlaceholderLocal": "Συνήθως δεν απαιτείται",
        "whereToFind": "Πού να το βρείτε",
        "baseUrl": "Βασικό URL",
        "baseUrlPlaceholderHost": "http://192.168.1.10:3333",
        "baseUrlPlaceholderLocal": "http://localhost:11434",
        "baseUrlPlaceholderRemote": "https://api.provider.com",
        "baseUrlPlaceholderIntenseRP": "http://127.0.0.1:7777/v1",
        "baseUrlHintLocal": "Η διεύθυνση τοπικού διακομιστή σας με θύρα",
        "baseUrlHintHost": "Εισάγετε τη διεύθυνση URL host της επιφάνειας εργασίας που εμφανίζεται από τη συσκευή host",
        "baseUrlHintRemote": "Παρακάμψτε το προεπιλεγμένο endpoint αν χρειάζεται",
        "chatEndpoint": "Chat Endpoint",
        "systemRole": "Ρόλος Συστήματος",
        "userRole": "Ρόλος Χρήστη",
        "assistantRole": "Ρόλος Βοηθού",
        "supportsStreaming": "Υποστηρίζει Streaming",
        "mergeSameRole": "Συγχώνευση μηνυμάτων ίδιου ρόλου",
        "toolChoiceMode": "Λειτουργία Επιλογής Εργαλείου",
        "toolChoiceHint": "Ελέγχει πώς αποστέλλεται το tool_choice στο προσαρμοσμένο endpoint."
      },
      "toolChoice": {
        "auto": "Αυτόματο",
        "required": "Απαιτείται",
        "none": "Κανένα",
        "omit": "Παράλειψη Πεδίου",
        "passthrough": "Passthrough (Διαμόρφωση Εργαλείου)"
      },
      "buttons": {
        "testConnection": "Δοκιμή Σύνδεσης",
        "testing": "Δοκιμή..."
      },
      "descriptions": {
        "chutes": "Εξαγωγή συμπερασμάτων συμβατή με OpenAI για κορυφαία μοντέλα ανοικτού κώδικα",
        "openai": "Μοντέλα GPT-5, GPT-4.1 και GPT-4o για εκφραστικό RP",
        "lettuceHost": "Σύνδεση στο δικό σας desktop Lettuce Host μέσω LAN με API τύπου OpenAI",
        "anthropic": "Claude 4.5 Sonnet & Haiku για βαθύ, συναισθηματικό διάλογο",
        "aggregator": "Πρόσβαση σε μοντέλα όπως GPT-5, Claude 4.5, Grok-3, Mixtral και περισσότερα",
        "openaiCompatible": "Χρησιμοποιήστε οποιοδήποτε endpoint API τύπου OpenAI",
        "mistral": "Mistral Small 3.2, Mixtral 8x22B και άλλα μοντέλα Mistral",
        "deepseek": "DeepSeek-V3.2-Exp, DeepSeek-R1 και άλλα μοντέλα υψηλής απόδοσης",
        "xai": "Grok-1.5, Grok-3 και νεότερα μοντέλα xAI",
        "zai": "GLM-4.5, GLM-4.6 και Air παραλλαγές",
        "moonshot": "Kimi-K2 Thinking και Kimi-K1 μοντέλα",
        "gemini": "Gemini 2.5 Flash, 2.5 Pro και περισσότερα",
        "qwen": "Qwen3-VL και νεότερα μοντέλα Qwen",
        "nvidia": "Nemotron, Llama, DeepSeek και περισσότερα μέσω NVIDIA NIM",
        "custom": "Στρέψτε το LettuceAI σε οποιοδήποτε προσαρμοσμένο endpoint μοντέλου",
        "fallback": "Πάροχος μοντέλου AI"
      },
      "descriptionsShort": {
        "chutes": "Εξαγωγή μοντέλων ανοικτού κώδικα",
        "openai": "GPT-5, GPT-4o, GPT-4.1",
        "lettuceHost": "Δικός σας LAN host",
        "anthropic": "Claude 4.5 Sonnet & Haiku",
        "aggregator": "Αθροιστής πολλαπλών μοντέλων",
        "openaiCompatible": "Προσαρμοσμένο endpoint OpenAI",
        "mistral": "Μοντέλα Mistral & Mixtral",
        "deepseek": "DeepSeek-V3, R1",
        "xai": "Grok-1.5, Grok-3",
        "zai": "GLM-4.5, GLM-4.6",
        "moonshot": "Kimi-K2 Thinking",
        "gemini": "Gemini 2.5 Flash & Pro",
        "qwen": "Μοντέλα Qwen3-VL",
        "nvidia": "NVIDIA NIM",
        "custom": "Προσαρμοσμένο endpoint",
        "fallback": "Πάροχος AI"
      },
      "cardLabel": "{{step}}: {{name}}",
      "errors": {
        "hostUrlRequired": "Απαιτείται URL host (π.χ. http://192.168.1.10:3333)",
        "baseUrlRequired": "Απαιτείται βασικό URL (π.χ. http://localhost:11434)",
        "apiKeyTooShort": "Το κλειδί API φαίνεται πολύ κοντό",
        "invalidApiKey": "Μη έγκυρο κλειδί API",
        "connectionFailed": "Αποτυχία σύνδεσης",
        "verificationFailed": "Αποτυχία επαλήθευσης",
        "failedToSave": "Αποτυχία αποθήκευσης παρόχου",
        "connectionSuccessful": "Επιτυχής σύνδεση!",
        "modelNotFound": "Δεν βρέθηκε μοντέλο στον πάροχο",
        "modelVerificationFailed": "Αποτυχία επαλήθευσης μοντέλου",
        "failedToSaveModel": "Αποτυχία αποθήκευσης μοντέλου"
      }
    },
    "model": {
      "noProvidersTitle": "Δεν έχουν ρυθμιστεί πάροχοι",
      "noProvidersDesc": "Θα χρειαστεί να συνδέσετε έναν πάροχο πριν επιλέξετε προεπιλεγμένο μοντέλο.",
      "goToProviderSetup": "Μετάβαση στη ρύθμιση παρόχου",
      "yourProviders": "Οι Πάροχοί Σας",
      "yourProvidersHint": "Επιλέξτε ποιον πάροχο θα χρησιμοποιήσετε",
      "setDefaultModel": "Ορίστε το προεπιλεγμένο μοντέλο σας",
      "setDefaultModelDesc": "Επιλέξτε ποιον πάροχο και όνομα μοντέλου πρέπει να χρησιμοποιεί το LettuceAI εξ ορισμού. Θα μπορείτε να προσθέσετε περισσότερα αργότερα.",
      "setDefaultModelDescDesktop": "Επιλέξτε πάροχο από τη λίστα για να ρυθμίσετε το μοντέλο σας.",
      "modelDetails": "Λεπτομέρειες Μοντέλου",
      "modelDetailsDesc": "Ορίστε το αναγνωριστικό API και την ετικέτα που θα βλέπετε στην εφαρμογή.",
      "whichModel": "Ποιο μοντέλο να χρησιμοποιήσω;",
      "nextMemorySystem": "Επόμενο: Σύστημα Μνήμης",
      "fields": {
        "displayName": "Εμφανιζόμενο Όνομα",
        "displayNamePlaceholder": "Δημιουργικός μέντορας",
        "displayNameHint": "Πώς εμφανίζεται αυτό το μοντέλο στα μενού",
        "modelId": "Αναγνωριστικό Μοντέλου",
        "modelPathGguf": "Διαδρομή Μοντέλου (GGUF)",
        "modelIdPlaceholder": "π.χ. gpt-4o",
        "modelPathPlaceholder": "/διαδρομή/προς/μοντέλο.gguf",
        "modelIdHint": "Ακριβές αναγνωριστικό για κλήσεις API",
        "showList": "Εμφάνιση Λίστας",
        "manualInput": "Χειροκίνητη Εισαγωγή",
        "refreshModelList": "Ανανέωση λίστας μοντέλων",
        "selectModel": "Επιλογή Μοντέλου",
        "selectAModel": "Επιλέξτε μοντέλο...",
        "searchModels": "Αναζήτηση μοντέλων...",
        "noModelsFound": "Δεν βρέθηκαν μοντέλα για \"{{query}}\""
      },
      "fillBothFields": "Συμπληρώστε και τα δύο παραπάνω πεδία για να ενεργοποιήσετε το κουμπί ολοκλήρωσης.",
      "providerNames": {
        "chutes": "Chutes",
        "intenseRpNext": "IntenseRP Next",
        "openai": "OpenAI",
        "anthropic": "Anthropic",
        "openrouter": "OpenRouter",
        "openaiCompatible": "Συμβατό OpenAI",
        "custom": "Προσαρμοσμένο endpoint"
      }
    },
    "memory": {
      "dynamicTitle": "Δυναμική Μνήμη",
      "recommended": "Συνιστάται",
      "settingUp": "Ρύθμιση...",
      "finishSetup": "Ολοκλήρωση Ρύθμισης",
      "promptTitle": "Ρύθμιση Δυναμικής Μνήμης",
      "oneLastStep": "Ένα Τελευταίο Βήμα",
      "downloadAndEnable": "Λήψη & Ενεργοποίηση",
      "chooseStyle": "Επιλέξτε το στυλ μνήμης σας",
      "howRemember": "Πώς πρέπει οι σύντροφοι AI σας να θυμούνται λεπτομέρειες για εσάς και τις συνομιλίες σας;",
      "dynamicDescription": "Χρησιμοποιεί ένα <0>τοπικό μοντέλο embedding</0> για έξυπνη διαχείριση πλαισίου. Αυτό μειώνει τα κόστη tokens διατηρώντας υψηλή ποιότητα, ακόμα και σε μακρές συνομιλίες.",
      "dynamicFeatures": {
        "quality": "Διατηρεί ποιότητα σε μακρές συνομιλίες",
        "cost": "Μειώνει σημαντικά τα κόστη API",
        "auto": "Αυτόματη διαχείριση πλαισίου",
        "zeroConfig": "Μηδενική ρύθμιση απαιτείται"
      },
      "manualTitle": "Χειροκίνητη Μνήμη",
      "manualBadge": "Κλασική εμπειρία",
      "manualDescription": "Καρφιτσώνετε ρητά μηνύματα και επεξεργάζεστε μόνοι σας τις \"Πληροφορίες Κόσμου\" ή τους ορισμούς χαρακτήρων. Καλό για πλήρη έλεγχο.",
      "manualFeatures": {
        "control": "Πλήρης έλεγχος πάνω στα γεγονότα",
        "scenarios": "Καλύτερο για συγκεκριμένα σενάρια"
      },
      "setupModelMessage": "Για να χρησιμοποιήσετε τη Δυναμική Μνήμη, χρειάζεται να κατεβάσουμε ένα μικρό μοντέλο embedding (~120MB) στη συσκευή σας.",
      "setupBullets": {
        "offline": "Το μοντέλο εκτελείται 100% εκτός σύνδεσης στη συσκευή σας",
        "remembering": "Απαιτείται για την αποθήκευση πλαισίου",
        "disable": "Μπορείτε να το απενεργοποιήσετε αργότερα στις ρυθμίσεις"
      },
      "stepLabel": "Βήμα 3 από 3",
      "stepLabelMemory": "Σύστημα Μνήμης"
    },
    "welcome": {
      "appName": "LettuceAI",
      "tagline": "Ο προσωπικός σας σύντροφος AI. Ιδιωτικός, ασφαλής, και πάντα στη συσκευή.",
      "features": {
        "onDevice": "Μόνο στη συσκευή",
        "characterReady": "Έτοιμοι χαρακτήρες"
      },
      "betaWarning": {
        "title": "Δοκιμαστική έκδοση Desktop",
        "description": "Χρησιμοποιείτε την έκδοση desktop. Ορισμένες λειτουργίες μπορεί να διαφέρουν από τη φορητή. Αναφέρετε προβλήματα στο GitHub."
      },
      "languageSelector": {
        "title": "Γλώσσα",
        "description": "Ανιχνεύτηκε αυτόματα από τη συσκευή σας. Μπορείτε να την αλλάξετε ανά πάσα στιγμή στις ρυθμίσεις."
      },
      "getStarted": "Ξεκινήστε",
      "skipForNow": "Παράλειψη προς το παρόν",
      "restoreFromBackup": "Επαναφορά από Αντίγραφο",
      "setupTime": "Η ρύθμιση διαρκεί λιγότερο από 2 λεπτά",
      "skipWarning": {
        "title": "Παράλειψη ρύθμισης;",
        "warningTitle": "Απαιτείται πάροχος για συνομιλία",
        "warningMessage": "Χωρίς πάροχο, δεν θα μπορείτε να στείλετε μηνύματα. Μπορείτε να προσθέσετε αργότερα από τις ρυθμίσεις.",
        "addProvider": "Προσθήκη Παρόχου",
        "skipAnyway": "Παράλειψη ούτως ή άλλως"
      },
      "restoreBackup": {
        "title": "Επαναφορά Αντιγράφου",
        "selectMessage": "Επιλέξτε ένα αντίγραφο για επαναφορά.",
        "browse": "Περιήγηση Αρχείων",
        "processing": "Επεξεργασία αρχείου...",
        "processingNote": "Τα μεγάλα αντίγραφα μπορεί να χρειαστούν ένα λεπτό",
        "noBackups": "Δεν βρέθηκαν αντίγραφα",
        "noBackupsHint": "Πατήστε περιήγηση για να επιλέξετε ένα αρχείο .lettuce",
        "browseLettuce": "Περιήγηση για αρχείο .lettuce",
        "passwordLabel": "Κωδικός Αντιγράφου",
        "passwordPlaceholder": "Εισάγετε κωδικό",
        "restoreButton": "Επαναφορά Αντιγράφου",
        "restoring": "Επαναφορά...",
        "infoMessage": "Αυτό θα ρυθμίσει την εφαρμογή με τα αποθηκευμένα δεδομένα σας, συμπεριλαμβανομένων χαρακτήρων, συνομιλιών και ρυθμίσεων.",
        "embeddingTitle": "Απαιτείται Μοντέλο Embedding",
        "dynamicMemoryDetected": "Ανιχνεύτηκε Δυναμική Μνήμη",
        "dynamicMemoryMessage": "Αυτό το αντίγραφο περιέχει χαρακτήρες με ενεργοποιημένη δυναμική μνήμη, που απαιτεί το μοντέλο embedding (~120MB).",
        "embeddingOptions": "Μπορείτε να κατεβάσετε το μοντέλο τώρα για να ενεργοποιήσετε τη δυναμική μνήμη, ή να συνεχίσετε χωρίς αυτό (η δυναμική μνήμη θα απενεργοποιηθεί για τους επηρεαζόμενους χαρακτήρες).",
        "downloadModel": "Λήψη Μοντέλου",
        "continueWithoutDynamic": "Συνέχεια Χωρίς Δυναμική Μνήμη",
        "embeddingNote": "Μπορείτε να ενεργοποιήσετε ξανά τη δυναμική μνήμη αργότερα στις ρυθμίσεις χαρακτήρα μετά τη λήψη του μοντέλου.",
        "back": "Πίσω",
        "cancel": "Ακύρωση",
        "errors": {
          "passwordRequired": "Απαιτείται κωδικός",
          "incorrectPassword": "Λανθασμένος κωδικός",
          "failedToOpenFile": "Αποτυχία ανοίγματος αρχείου",
          "failedToRestore": "Αποτυχία επαναφοράς αντιγράφου",
          "failedToUpdateSettings": "Αποτυχία ενημέρωσης ρυθμίσεων"
        }
      }
    },
    "common": {
      "back": "Πίσω",
      "cancel": "Ακύρωση",
      "continue": "Συνέχεια",
      "verifying": "Επαλήθευση...",
      "skipForNow": "Παράλειψη προς το παρόν",
      "selectAProvider": "Επιλέξτε πάροχο για ρύθμιση",
      "clickToSelectProvider": "Κάντε κλικ για επιλογή παρόχου",
      "selectProviderFromList": "Επιλέξτε πάροχο από τη λίστα για να ξεκινήσετε.",
      "enterApiKey": "Εισάγετε το κλειδί API σας για να ενεργοποιήσετε τη λειτουργικότητα AI chat."
    },
    "modelGuide": {
      "badge": "Οδηγός Μοντέλων",
      "title": "Πώς να επιλέξω μοντέλο;",
      "intro": "Το LettuceAI δεν επιβάλλει ένα «καλύτερο» μοντέλο. Αντίθετα, επιλέγετε αυτό που ταιριάζει στη <0>χρήση, τον προϋπολογισμό και το ύφος σας</0>. Χρησιμοποιήστε αυτόν τον οδηγό για να αποφασίσετε τι να δοκιμάσετε και πού να κοιτάξετε.",
      "askYourself": "Ρωτήστε τον εαυτό σας:",
      "factors": {
        "quality": {
          "title": "Ποιότητα και δυνατότητες",
          "description": "Πόσο έξυπνο πρέπει να είναι το μοντέλο; Τα μεγαλύτερα, νεότερα μοντέλα συνήθως συλλογίζονται καλύτερα, γράφουν καλύτερο κείμενο και χειρίζονται πιο χαριτωμένα τα ακανόνιστα prompts.",
          "q1": "Χρειάζεστε βαθιά συνέπεια χαρακτήρα και συναισθηματική νοημοσύνη;",
          "q2": "Σας ενδιαφέρει η εμβύθιση στην ιστορία και οι αξιόπιστες προσωπικότητες χαρακτήρων;",
          "q3": "Θέλετε το μοντέλο να θυμάται λεπτομέρειες χαρακτήρα και να παραμένει εντός ρόλου σε μεγάλες συνεδρίες;"
        },
        "speed": {
          "title": "Ταχύτητα και καθυστέρηση",
          "description": "Τα πιο γρήγορα μοντέλα αισθάνονται καλύτερα για πολυσυνομιλητικές ανταλλαγές. Μερικά μοντέλα ανταλλάσσουν λίγη ποιότητα για πολύ περισσότερη ταχύτητα.",
          "q1": "Θέλετε σχεδόν άμεσες απαντήσεις για φυσική ροή roleplay;",
          "q2": "Κάνετε γρήγορες εναλλαγές διαλόγου όπου η αναμονή θα έσπαγε την εμβύθιση;",
          "q3": "Είναι για casual RP όπου η γρήγορη ανταλλαγή έχει μεγαλύτερη σημασία από τέλειες απαντήσεις;"
        },
        "budget": {
          "title": "Προϋπολογισμός και χρήση",
          "description": "Κάθε πάροχος χρεώνει ανά token. Ακόμα και τα φθηνά μοντέλα μπορεί να αθροιστούν αν συνομιλείτε πολύ, οπότε επιλέξτε κάτι που ταιριάζει στη συχνότητα χρήσης σας.",
          "q1": "Είστε εντάξει να πληρώνετε περισσότερο για πλουσιότερες αλληλεπιδράσεις χαρακτήρων, ή θέλετε κάτι φθηνό για καθημερινό RP;",
          "q2": "Έχετε δωρεάν μοντέλα από τον πάροχο/δρομολογητή σας που μπορείτε να δοκιμάσετε πρώτα;",
          "q3": "Θα εκτελείτε μεγάλες συνεδρίες roleplay με λεπτομερείς περιγραφές σκηνών;",
          "q4": "Έχετε σκληρό μηνιαίο προϋπολογισμό που δεν θέλετε να υπερβείτε;"
        },
        "safety": {
          "title": "Ασφάλεια, ιδιωτικότητα και έξτρα",
          "description": "Οι πάροχοι διαφέρουν στον τρόπο χειρισμού της ασφάλειας, καταγραφής και επιπλέον λειτουργιών όπως εικόνες, εργαλεία ή μεγάλα παράθυρα πλαισίου.",
          "q1": "Χρειάζεστε λιγότερα φίλτρα περιεχομένου για ώριμα ή δημιουργικά σενάρια roleplay;",
          "q2": "Σας ενδιαφέρει αν οι ιδιωτικές συνομιλίες RP σας καταγράφονται ή χρησιμοποιούνται για εκπαίδευση;",
          "q3": "Χρειάζεστε μεγάλα παράθυρα πλαισίου για σύνθετες ιστορίες και ιστορικά χαρακτήρων;"
        }
      },
      "where": {
        "title": "Πού μπορώ να βρω μοντέλα;",
        "intro": "Οι περισσότεροι πάροχοι και δρομολογητές έχουν <0>λίστα ή κατάλογο μοντέλων</0>. Περιηγηθείτε σε αυτές τις σελίδες για να δείτε τι προσφέρουν, τιμολόγηση, όρια και ειδικές λειτουργίες.",
        "directTitle": "Άμεσοι πάροχοι",
        "directDesc": "OpenAI, Anthropic, Google Gemini, xAI, Mistral κ.λπ. Ο καθένας έχει κονσόλα/πίνακα ελέγχου όπου μπορείτε να δείτε επίσημα ονόματα μοντέλων, δυνατότητες και τιμολόγηση.",
        "routersTitle": "Δρομολογητές και κόμβοι",
        "routersDesc": "Υπηρεσίες όπως OpenRouter ή άλλοι αθροιστές παραθέτουν πολλά μοντέλα από διαφορετικούς παρόχους σε ένα μέρος, συχνά με benchmarks και συγκρίσεις τιμολόγησης.",
        "communityTitle": "Προτάσεις κοινότητας",
        "communityDesc": "Κοιτάξτε έγγραφα, blogs ή αναρτήσεις κοινότητας από τον πάροχο/δρομολογητή σας. Συνήθως αναδεικνύουν ποια μοντέλα είναι καλύτερα για chat, κωδικοποίηση ή ταχύτητα."
      },
      "rules": {
        "title": "Απλοί εμπειρικοί κανόνες",
        "casual": "Για casual συνομιλία: επιλέξτε γρήγορο, φθηνό μοντέλο chat από τον πάροχο/δρομολογητή σας.",
        "experiments": "Για πειράματα ή μεγάλο όγκο: ξεκινήστε με το φθηνότερο μοντέλο που αισθάνεται αρκετά καλό, μετά αναβαθμίστε αν χρειαστεί.",
        "switch": "Αν κάτι δεν πάει καλά (πολύ αργό / πολύ αδύναμο / πολύ ακριβό): μπορείτε πάντα να αλλάξετε μοντέλα αργότερα στο LettuceAI."
      },
      "disclaimer": "Ελέγχετε πάντα την τεκμηρίωση του ίδιου του παρόχου για την τελευταία λίστα μοντέλων, όρια και τιμολόγηση. Αυτή η σελίδα αφορά στον τρόπο σκέψης, όχι σε τι να αγοράσετε."
    },
    "whereToFind": {
      "badge": "Βοήθεια Κλειδιού API",
      "intro": "Ακολουθήστε αυτά τα βήματα για να αποκτήσετε το κλειδί API σας, μετά επιστρέψτε στο LettuceAI και επικολλήστε το στις ρυθμίσεις παρόχου.",
      "readyPrompt": "Έτοιμοι να πάρετε το κλειδί;",
      "openProviderSite": "Άνοιγμα ιστότοπου παρόχου",
      "keyWarning": "Μην μοιράζεστε ποτέ το κλειδί API σας δημόσια. Όποιος έχει αυτό το κλειδί μπορεί να χρησιμοποιήσει το υπόλοιπο λογαριασμού σας.",
      "stuckPrompt": "Ακόμα δεν μπορείτε να το βρείτε;",
      "joinDiscord": "Εγγραφείτε στον Discord μας για βοήθεια",
      "guides": {
        "chutes": {
          "title": "Πώς να βρείτε το κλειδί API Chutes",
          "s1": "Μεταβείτε στο chutes.ai/app και συνδεθείτε.",
          "s2": "Ανοίξτε την περιοχή λογαριασμού/ρυθμίσεων και βρείτε τα Κλειδιά API.",
          "s3": "Δημιουργήστε νέο κλειδί (ή αντιγράψτε ένα υπάρχον).",
          "s4": "Επικολλήστε το κλειδί στο LettuceAI."
        },
        "openai": {
          "title": "Πώς να βρείτε το κλειδί API OpenAI",
          "s1": "Μεταβείτε στο platform.openai.com και συνδεθείτε.",
          "s2": "Κάντε κλικ στο avatar προφίλ σας πάνω δεξιά και επιλέξτε Κλειδιά API.",
          "s3": "Κάντε κλικ στο Create new secret key και αντιγράψτε την τιμή που εμφανίζεται.",
          "s4": "Επικολλήστε το κλειδί στο LettuceAI και αποθηκεύστε το αλλού. Δεν θα το δείτε ξανά."
        },
        "anthropic": {
          "title": "Πώς να βρείτε το κλειδί API Anthropic",
          "s1": "Μεταβείτε στο console.anthropic.com και συνδεθείτε.",
          "s2": "Ανοίξτε τις Ρυθμίσεις από το αριστερό μενού.",
          "s3": "Επιλέξτε Κλειδιά API και κάντε κλικ στο Create key.",
          "s4": "Αντιγράψτε το κλειδί και επικολλήστε το στο LettuceAI."
        },
        "openrouter": {
          "title": "Πώς να βρείτε το κλειδί API OpenRouter",
          "s1": "Επισκεφτείτε το openrouter.ai και συνδεθείτε.",
          "s2": "Ανοίξτε τη σελίδα Κλειδιών από το μενού προφίλ σας.",
          "s3": "Κάντε κλικ στο Create key, δώστε όνομα και αποθηκεύστε.",
          "s4": "Αντιγράψτε το κλειδί και επικολλήστε το στο LettuceAI."
        },
        "mistral": {
          "title": "Πώς να βρείτε το κλειδί API Mistral",
          "s1": "Μεταβείτε στο console.mistral.ai και συνδεθείτε.",
          "s2": "Κάντε κλικ στα Κλειδιά API στο πλαϊνό μενού.",
          "s3": "Κάντε κλικ στο Create an API key.",
          "s4": "Αντιγράψτε το κλειδί και επικολλήστε το στο LettuceAI."
        },
        "deepseek": {
          "title": "Πώς να βρείτε το κλειδί API DeepSeek",
          "s1": "Ανοίξτε το platform.deepseek.com και συνδεθείτε.",
          "s2": "Κάντε κλικ στα Κλειδιά API στην πάνω πλοήγηση.",
          "s3": "Δημιουργήστε νέο κλειδί αν δεν έχετε ήδη.",
          "s4": "Αντιγράψτε το κλειδί και επικολλήστε το στο LettuceAI."
        },
        "groq": {
          "title": "Πώς να βρείτε το κλειδί API Groq",
          "s1": "Επισκεφτείτε το console.groq.com και συνδεθείτε.",
          "s2": "Ανοίξτε τα Κλειδιά API από το πλαϊνό μενού.",
          "s3": "Δημιουργήστε νέο κλειδί και αντιγράψτε το.",
          "s4": "Επικολλήστε το κλειδί στο LettuceAI."
        },
        "gemini": {
          "title": "Πώς να βρείτε το κλειδί API Google Gemini",
          "s1": "Μεταβείτε στο Google AI Studio στο aistudio.google.com και συνδεθείτε.",
          "s2": "Κάντε κλικ στο Get API key ή Manage keys.",
          "s3": "Δημιουργήστε νέο κλειδί αν χρειαστεί.",
          "s4": "Αντιγράψτε το κλειδί και επικολλήστε το στο LettuceAI."
        },
        "xai": {
          "title": "Πώς να βρείτε το κλειδί API xAI",
          "s1": "Ανοίξτε το console.x.ai και συνδεθείτε.",
          "s2": "Πλοηγηθείτε στην ενότητα Κλειδιά API στην κονσόλα.",
          "s3": "Δημιουργήστε νέο κλειδί.",
          "s4": "Αντιγράψτε το κλειδί και επικολλήστε το στο LettuceAI."
        },
        "zai": {
          "title": "Πώς να βρείτε το κλειδί API zAI (GLM)",
          "s1": "Μεταβείτε στο open.bigmodel.cn και συνδεθείτε.",
          "s2": "Ανοίξτε το Κέντρο Χρηστών και μεταβείτε στα Κλειδιά API.",
          "s3": "Δημιουργήστε νέο κλειδί αν δεν έχετε.",
          "s4": "Αντιγράψτε το κλειδί και επικολλήστε το στο LettuceAI."
        },
        "moonshot": {
          "title": "Πώς να βρείτε το κλειδί API Moonshot (Kimi)",
          "s1": "Επισκεφτείτε το platform.moonshot.cn και συνδεθείτε.",
          "s2": "Ανοίξτε την ενότητα Κλειδιά API στην κονσόλα.",
          "s3": "Δημιουργήστε νέο κλειδί και αντιγράψτε το.",
          "s4": "Επικολλήστε το κλειδί στο LettuceAI."
        },
        "qwen": {
          "title": "Πώς να βρείτε το κλειδί API Qwen",
          "s1": "Ανοίξτε το dashscope.aliyun.com και συνδεθείτε.",
          "s2": "Μεταβείτε στην ενότητα Κλειδιά API στο πλαϊνό μενού.",
          "s3": "Δημιουργήστε νέο κλειδί.",
          "s4": "Αντιγράψτε το κλειδί και επικολλήστε το στο LettuceAI."
        },
        "nanogpt": {
          "title": "Πώς να βρείτε το κλειδί API NanoGPT",
          "s1": "Μεταβείτε στο nano-gpt.com και συνδεθείτε.",
          "s2": "Ανοίξτε τον πίνακα ελέγχου και μεταβείτε στην ενότητα κλειδιών API.",
          "s3": "Δημιουργήστε νέο κλειδί αν χρειαστεί.",
          "s4": "Αντιγράψτε το κλειδί και επικολλήστε το στο LettuceAI."
        },
        "featherless": {
          "title": "Πώς να βρείτε το κλειδί API Featherless",
          "s1": "Επισκεφτείτε το featherless.ai και συνδεθείτε.",
          "s2": "Ανοίξτε τον λογαριασμό σας ή την ενότητα API από τον πίνακα ελέγχου.",
          "s3": "Δημιουργήστε νέο κλειδί αν δεν βλέπετε κανένα.",
          "s4": "Αντιγράψτε το κλειδί και επικολλήστε το στο LettuceAI."
        },
        "anannas": {
          "title": "Πώς να βρείτε το κλειδί API Anannas",
          "s1": "Μεταβείτε στο dashboard.anannas.ai και συνδεθείτε.",
          "s2": "Πλοηγηθείτε στην ενότητα Κλειδιά API.",
          "s3": "Δημιουργήστε νέο κλειδί και αντιγράψτε το.",
          "s4": "Επικολλήστε το κλειδί στο LettuceAI."
        },
        "default": {
          "title": "Πώς να βρείτε το κλειδί API σας",
          "s1": "Ανοίξτε τον πίνακα ελέγχου του παρόχου AI σε πρόγραμμα περιήγησης και συνδεθείτε.",
          "s2": "Αναζητήστε ρυθμίσεις API, Προγραμματιστή ή Ενσωματώσεων.",
          "s3": "Δημιουργήστε νέο κλειδί API ή δείτε ένα υπάρχον.",
          "s4": "Αντιγράψτε το κλειδί και επικολλήστε το στο LettuceAI."
        }
      }
    },
    "welcomeFooter": {
      "setupTimeMobile": "Η ρύθμιση διαρκεί λιγότερο από 2 λεπτά"
    }
  },
  "search": {
    "placeholder": "Αναζήτηση...",
    "tabs": {
      "characters": "Χαρακτήρες",
      "personas": "Περσόνες"
    },
    "noResults": "Δεν βρέθηκαν {{type}}",
    "emptyState": "Δεν υπάρχουν {{type}} ακόμα",
    "noResultsHint": "Δοκιμάστε διαφορετικό όρο αναζήτησης",
    "emptyCharacters": "Δημιουργήστε τον πρώτο σας χαρακτήρα για να ξεκινήσετε τη συνομιλία",
    "emptyPersonas": "Δημιουργήστε μια περσόνα στις ρυθμίσεις",
    "a11y": {
      "goBack": "Πίσω",
      "clearSearch": "Καθαρισμός αναζήτησης",
      "characterAvatar": "Avatar {{name}}"
    },
    "session": {
      "newChatTitle": "Νέα Συνομιλία"
    },
    "noDescription": "Χωρίς περιγραφή",
    "defaultBadge": "Προεπιλογή"
  },
  "sync": {
    "modes": {
      "join": "Σύνδεση",
      "joinDesc": "Σύνδεση σε κεντρικό υπολογιστή",
      "host": "Φιλοξενία",
      "hostDesc": "Μοιραστείτε τα δεδομένα σας"
    },
    "sections": {
      "mode": "Λειτουργία",
      "connectToHost": "Σύνδεση σε Κεντρικό",
      "startHosting": "Έναρξη Φιλοξενίας",
      "status": "Κατάσταση",
      "hosting": "Υπηρεσία Φιλοξενίας",
      "localAddress": "Διεύθυνση Τοπικού Δικτύου",
      "connectionPin": "PIN Σύνδεσης",
      "setupGuide": "Οδηγός Ρύθμισης"
    },
    "fields": {
      "hostAddress": "Διεύθυνση ή JSON Κεντρικού",
      "hostPlaceholder": "π.χ. 192.168.1.100:12345",
      "pinCode": "Κωδικός PIN",
      "pinPlaceholder": "000000"
    },
    "buttons": {
      "scanQRCode": "Σάρωση QR Code",
      "connect": "Σύνδεση",
      "connecting": "Σύνδεση...",
      "startHosting": "Έναρξη Φιλοξενίας",
      "startingServer": "Εκκίνηση διακομιστή...",
      "stopHosting": "Διακοπή Φιλοξενίας",
      "hostAgain": "Φιλοξενία Ξανά",
      "done": "Τέλος"
    },
    "status": {
      "connecting": "Σύνδεση...",
      "connected": "Συνδεδεμένο",
      "waitingConfirmation": "Αναμονή για επιβεβαίωση",
      "waitingConfirmationDesc": "Εγκρίνετε τη σύνδεση στη συσκευή υποδοχής για να συνεχίσετε.",
      "syncing": "Συγχρονισμός...",
      "transferringData": "Μεταφορά δεδομένων",
      "syncInProgress": "Συγχρονισμός σε Εξέλιξη",
      "live": "Ζωντανό",
      "broadcasting": "Μετάδοση",
      "clientsLabel": "Συνδεδεμένοι",
      "clientsUnit": "Πελάτες"
    },
    "pinDescription": "Μοιραστείτε αυτό το PIN με τη συσκευή που συνδέεται",
    "hostingDesc1": "Άλλες συσκευές μπορούν να συνδεθούν και να συγχρονίσουν δεδομένα από αυτή τη συσκευή.",
    "hostingDesc2": "Τα δεδομένα σας θα μοιραστούν με τους συνδεδεμένους πελάτες.",
    "setupSteps": {
      "step1": "Ανοίξτε την εφαρμογή σε άλλη συσκευή",
      "step2": "Μεταβείτε στις Ρυθμίσεις → Τοπικός Συγχρονισμός",
      "step3": "Σαρώστε τον κωδικό QR ή εισάγετε τη διεύθυνση"
    },
    "messages": {
      "completed": "Ο Συγχρονισμός Ολοκληρώθηκε!",
      "completedDesc": "Όλα τα δεδομένα συγχρονίστηκαν",
      "error": "Σφάλμα Σύνδεσης",
      "outdatedClient": "Ανιχνεύτηκε Παλαιό Πρόγραμμα Πελάτη"
    },
    "disclaimer": "Ο συγχρονισμός λειτουργεί μέσω του τοπικού δικτύου σας. Και οι δύο συσκευές πρέπει να είναι στο ίδιο WiFi.",
    "modals": {
      "connectionRequest": "Αίτημα Σύνδεσης",
      "requestMessage": "θέλει να συγχρονιστεί με αυτή τη συσκευή.",
      "acceptConnection": "Αποδοχή Σύνδεσης",
      "acceptDesc": "Επιτρέψτε σε αυτή τη συσκευή να συγχρονίσει δεδομένα",
      "decline": "Απόρριψη",
      "declineDesc": "Αποκλεισμός αυτής της απόπειρας σύνδεσης",
      "readyToSync": "Έτοιμο για Συγχρονισμό",
      "connectionEstablished": "Η Σύνδεση Δημιουργήθηκε",
      "deviceReady": "είναι έτοιμη.",
      "startSyncMessage": "Πατήστε παρακάτω για να ξεκινήσετε τον συγχρονισμό δεδομένων.",
      "startSyncing": "Έναρξη Συγχρονισμού",
      "startSyncingDesc": "Ξεκινήστε τη μεταφορά δεδομένων τώρα"
    },
    "scanner": {
      "title": "Σάρωση QR Code",
      "cancel": "Ακύρωση Σάρωσης"
    },
    "unknownDevice": "Άγνωστη Συσκευή",
    "aria": {
      "dismissStatus": "Απόρριψη κατάστασης συγχρονισμού",
      "dismissError": "Απόρριψη σφάλματος συγχρονισμού"
    },
    "stats": {
      "statusLabel": "Κατάσταση"
    }
  },
  "creationHelper": {
    "page": {
      "info": "Ο Βοηθός Δημιουργίας σας καθοδηγεί στη δημιουργία χαρακτήρων με τη βοήθεια AI. Ρυθμίστε το μοντέλο και τα εργαλεία που χρησιμοποιούνται κατά τη δημιουργία χαρακτήρων.",
      "modelConfiguration": "Ρύθμιση Μοντέλου",
      "chatModel": "Μοντέλο Συνομιλίας",
      "selectedModel": "Επιλεγμένο Μοντέλο",
      "useAppDefault": "Χρήση προεπιλογής εφαρμογής{{model}}",
      "useAppDefaultBase": "Χρήση προεπιλογής εφαρμογής",
      "noModelsAvailable": "Δεν υπάρχουν διαθέσιμα μοντέλα",
      "chatModelDescription": "Μοντέλο AI για συνομιλίες δημιουργίας χαρακτήρων",
      "streamingOutput": "Ροή Εξόδου",
      "streamingDescription": "Εμφάνιση απαντήσεων καθώς δημιουργούνται",
      "imageGenerationModel": "Μοντέλο Δημιουργίας Εικόνων",
      "noModelSelected": "Δεν έχει επιλεγεί μοντέλο",
      "noImageModelsAvailable": "Δεν υπάρχουν διαθέσιμα μοντέλα εικόνων",
      "imageModelDescription": "Για δημιουργία avatar χαρακτήρων",
      "toolSelection": "Επιλογή Εργαλείων",
      "smartToolSelection": "Έξυπνη Επιλογή Εργαλείων",
      "smartToolDescription": "Το AI επιλέγει αυτόματα ποια εργαλεία θα χρησιμοποιήσει",
      "smartToolEnabledHint": "Όταν είναι ενεργοποιημένο, ο AI Βοηθός Δημιουργίας ρωτά τι θέλετε να δημιουργήσετε και φορτώνει μόνο το σχετικό σύνολο εργαλείων.",
      "smartToolDisabledHint": "Όταν είναι απενεργοποιημένο, ο AI Βοηθός Δημιουργίας ανοίγει απευθείας και χρησιμοποιεί όλα τα ενεργοποιημένα εργαλεία· ο βοηθός αποφασίζει τι θα δημιουργήσει.",
      "quickPresets": "Γρήγορες Προεπιλογές",
      "customSelection": "Προσαρμοσμένη επιλογή - {{count}} εργαλεία ενεργοποιημένα",
      "footerInfo": "Όταν η Έξυπνη Επιλογή Εργαλείων είναι ενεργοποιημένη, το AI αποφασίζει ποια εργαλεία θα χρησιμοποιήσει με βάση το πλαίσιο. Απενεργοποιήστε τη για χειροκίνητο έλεγχο των διαθέσιμων εργαλείων.",
      "selectChatModel": "Επιλογή Μοντέλου Συνομιλίας",
      "selectImageModel": "Επιλογή Μοντέλου Εικόνων",
      "searchModels": "Αναζήτηση μοντέλων..."
    },
    "categories": {
      "basic": "Βασικά",
      "content": "Περιεχόμενο",
      "visual": "Οπτικά",
      "settings": "Ρυθμίσεις",
      "flow": "Ροή",
      "persona": "Περσόνες",
      "lorebook": "Βιβλία Γνώσης"
    },
    "presets": {
      "all": {
        "name": "Όλα τα Εργαλεία",
        "desc": "Ενεργοποίηση όλων των διαθέσιμων εργαλείων"
      },
      "essential": {
        "name": "Βασικά",
        "desc": "Μόνο όνομα, ορισμός και σκηνές"
      },
      "minimal": {
        "name": "Ελάχιστα",
        "desc": "Μόνο όνομα και ορισμός"
      }
    },
    "tools": {
      "setName": "Ορισμός Ονόματος",
      "setNameDesc": "Ορίστε το όνομα του χαρακτήρα",
      "setDefinition": "Ορισμός Περιγραφής",
      "setDefinitionDesc": "Ορίστε προσωπικότητα και υπόβαθρο",
      "addScene": "Προσθήκη Σκηνής",
      "addSceneDesc": "Προσθέστε μια αρχική σκηνή για roleplay",
      "updateScene": "Ενημέρωση Σκηνής",
      "updateSceneDesc": "Τροποποίηση υπάρχουσας σκηνής",
      "avatarGradient": "Ντεγκραντέ Avatar",
      "avatarGradientDesc": "Εναλλαγή ντεγκραντέ επικάλυψης στο avatar",
      "setModel": "Ορισμός Μοντέλου",
      "setModelDesc": "Ορίστε το μοντέλο AI για συνομιλίες",
      "systemPrompt": "Προτροπή Συστήματος",
      "systemPromptDesc": "Ορίστε οδηγίες συμπεριφοράς",
      "listPrompts": "Λίστα Προτροπών",
      "listPromptsDesc": "Προβολή διαθέσιμων προτροπών",
      "listModels": "Λίστα Μοντέλων",
      "listModelsDesc": "Προβολή διαθέσιμων μοντέλων",
      "imageAsAvatar": "Εικόνα ως Avatar",
      "imageAsAvatarDesc": "Χρήση ανεβασμένης εικόνας ως avatar",
      "set_character_name": {
        "name": "Ορισμός Ονόματος",
        "desc": "Ορίστε το όνομα του χαρακτήρα"
      },
      "set_character_definition": {
        "name": "Ορισμός Περιγραφής",
        "desc": "Ορίστε προσωπικότητα και υπόβαθρο"
      },
      "add_scene": {
        "name": "Προσθήκη Σκηνής",
        "desc": "Προσθέστε μια αρχική σκηνή για roleplay"
      },
      "update_scene": {
        "name": "Ενημέρωση Σκηνής",
        "desc": "Τροποποίηση υπάρχουσας σκηνής"
      },
      "toggle_avatar_gradient": {
        "name": "Ντεγκραντέ Avatar",
        "desc": "Εναλλαγή ντεγκραντέ επικάλυψης στο avatar"
      },
      "set_default_model": {
        "name": "Ορισμός Μοντέλου",
        "desc": "Ορίστε το μοντέλο AI για συνομιλίες"
      },
      "set_system_prompt": {
        "name": "Προτροπή Συστήματος",
        "desc": "Ορίστε οδηγίες συμπεριφοράς"
      },
      "get_system_prompt_list": {
        "name": "Λίστα Προτροπών",
        "desc": "Προβολή διαθέσιμων προτροπών"
      },
      "get_model_list": {
        "name": "Λίστα Μοντέλων",
        "desc": "Προβολή διαθέσιμων μοντέλων"
      },
      "use_uploaded_image_as_avatar": {
        "name": "Εικόνα ως Avatar",
        "desc": "Χρήση ανεβασμένης εικόνας ως avatar"
      },
      "use_uploaded_image_as_chat_background": {
        "name": "Εικόνα ως Φόντο",
        "desc": "Χρήση ανεβασμένης εικόνας ως φόντο"
      },
      "generate_image": {
        "name": "Δημιουργία Εικόνας",
        "desc": "Δημιουργία εικόνας με το μοντέλο AI"
      },
      "show_preview": {
        "name": "Εμφάνιση Προεπισκόπησης",
        "desc": "Προεπισκόπηση του χαρακτήρα"
      },
      "request_confirmation": {
        "name": "Αίτημα Επιβεβαίωσης",
        "desc": "Ζητήστε αποθήκευση ή συνέχεια"
      },
      "list_personas": {
        "name": "Λίστα Περσονών",
        "desc": "Περιήγηση περσονών"
      },
      "upsert_persona": {
        "name": "Αποθήκευση Περσόνας",
        "desc": "Δημιουργία ή ενημέρωση περσόνας"
      },
      "use_uploaded_image_as_persona_avatar": {
        "name": "Avatar Περσόνας",
        "desc": "Χρήση ανεβασμένης εικόνας ως avatar περσόνας"
      },
      "delete_persona": {
        "name": "Διαγραφή Περσόνας",
        "desc": "Αφαίρεση περσόνας"
      },
      "get_default_persona": {
        "name": "Προεπιλεγμένη Περσόνα",
        "desc": "Ανάκτηση της προεπιλεγμένης περσόνας"
      },
      "list_lorebooks": {
        "name": "Λίστα Βιβλίων Γνώσης",
        "desc": "Περιήγηση βιβλίων γνώσης"
      },
      "upsert_lorebook": {
        "name": "Αποθήκευση Βιβλίου Γνώσης",
        "desc": "Δημιουργία ή ενημέρωση βιβλίου γνώσης"
      },
      "delete_lorebook": {
        "name": "Διαγραφή Βιβλίου Γνώσης",
        "desc": "Αφαίρεση βιβλίου γνώσης"
      },
      "list_lorebook_entries": {
        "name": "Λίστα Εγγραφών",
        "desc": "Προβολή εγγραφών βιβλίου γνώσης"
      },
      "get_lorebook_entry": {
        "name": "Λήψη Εγγραφής",
        "desc": "Ανάκτηση εγγραφής βιβλίου γνώσης"
      },
      "upsert_lorebook_entry": {
        "name": "Αποθήκευση Εγγραφής",
        "desc": "Δημιουργία ή ενημέρωση εγγραφής"
      },
      "delete_lorebook_entry": {
        "name": "Διαγραφή Εγγραφής",
        "desc": "Αφαίρεση εγγραφής βιβλίου γνώσης"
      },
      "create_blank_lorebook_entry": {
        "name": "Κενή Εγγραφή",
        "desc": "Δημιουργία κενής εγγραφής"
      },
      "reorder_lorebook_entries": {
        "name": "Αναδιάταξη Εγγραφών",
        "desc": "Αλλαγή σειράς εγγραφών"
      },
      "list_character_lorebooks": {
        "name": "Λίστα Βιβλίων Γνώσης Χαρακτήρα",
        "desc": "Προβολή βιβλίων γνώσης για χαρακτήρα"
      },
      "set_character_lorebooks": {
        "name": "Ορισμός Βιβλίων Γνώσης Χαρακτήρα",
        "desc": "Αντιστοίχιση βιβλίων γνώσης σε χαρακτήρα"
      }
    }
  },
  "tour": {
    "stepCounter": "Βήμα {{current}} από {{total}}",
    "skipTour": "Παράλειψη ξενάγησης",
    "next": "Επόμενο",
    "gotIt": "Κατάλαβα",
    "appShell": {
      "chats": {
        "title": "Εδώ ζουν οι συνομιλίες σας",
        "body": "Όλες οι συνομιλίες σας ένας-προς-ένας με χαρακτήρες βρίσκονται εδώ. Επιστρέψτε ανά πάσα στιγμή και θα κρατήσουμε τη θέση σας."
      },
      "groups": {
        "title": "Ομαδικές συνομιλίες",
        "body": "Φέρτε πολλούς χαρακτήρες στο ίδιο δωμάτιο και παρακολουθήστε τους να μιλούν μεταξύ τους, ή μπείτε κι εσείς όποτε θέλετε."
      },
      "discover": {
        "title": "Ανακαλύψτε νέους χαρακτήρες",
        "body": "Περιηγηθείτε σε ό,τι έχει μοιραστεί η κοινότητα και φέρτε οποιονδήποτε χαρακτήρα σας τραβήξει το μάτι. Νέα αγαπημένα με ένα πάτημα."
      },
      "library": {
        "title": "Η προσωπική σας βιβλιοθήκη",
        "body": "Ό,τι έχετε φτιάξει ή αποθηκεύσει ζει εδώ: χαρακτήρες, περσόνες, prompts, τα πάντα. Σκεφτείτε το ως την αποθήκη σας."
      },
      "settings": {
        "title": "Κάντε το δικό σας",
        "body": "Αλλάξτε παρόχους, επιλέξτε διαφορετικά μοντέλα, ρυθμίστε την εμφάνιση της εφαρμογής. Σχεδόν τα πάντα είναι ρυθμιζόμενα."
      },
      "search": {
        "title": "Βρείτε ό,τι θέλετε, γρήγορα",
        "body": "Ψάχνετε κάποια συγκεκριμένη συνομιλία ή χαρακτήρα; Αναζητήστε σε όλα από εδώ. Χωρίς σκάψιμο."
      },
      "create": {
        "title": "Και τέλος, δημιουργήστε!",
        "body": "Πατήστε το συν όποτε έρθει έμπνευση. Φτιάξτε νέο χαρακτήρα, περσόνα ή ξεκινήστε κάτι από την αρχή."
      }
    },
    "chatDetail": {
      "chatTitle": {
        "title": "Ρυθμίσεις ανά συνομιλία",
        "body": "Πατήστε το όνομα του χαρακτήρα εδώ πάνω για να ανοίξετε ρυθμίσεις μόνο για αυτή τη συνομιλία. Διαφορετικές περσόνες, διατάξεις και μοντέλα ανά συνομιλία."
      },
      "chatMemory": {
        "title": "Τι θυμούνται",
        "body": "Το εικονίδιο εγκεφάλου δείχνει τι θυμάται ο χαρακτήρας από τις συνομιλίες σας. Πατήστε για έλεγχο, επεξεργασία ή εκκαθάριση αναμνήσεων."
      },
      "chatSearch": {
        "title": "Βρείτε εκείνη τη γραμμή",
        "body": "Αναζήτηση μόνο σε αυτή τη συνομιλία. Ιδανικό για να βρείτε μια λεπτομέρεια 200 μηνύματα πίσω χωρίς ατέρμονο scrolling."
      },
      "chatLorebook": {
        "title": "Καταχωρήσεις βιβλίου γνώσης",
        "body": "Επιπλέον πληροφορίες, world-building και πλαίσιο που εισάγονται στο prompt όταν εμφανίζονται συγκεκριμένες λέξεις-κλειδιά. Το σκονάκι του χαρακτήρα σας."
      },
      "chatPlus": {
        "title": "Επισυνάψτε πράγματα",
        "body": "Ρίξτε εικόνες ή ανοίξτε το μενού πρόσθετων. Ό,τι επισυνάψετε στέλνεται μαζί με το επόμενο μήνυμά σας."
      },
      "chatComposer": {
        "title": "Το μήνυμά σας, η κίνησή σας",
        "body": "Πληκτρολογήστε εδώ. Enter στέλνει, Shift+Enter αλλάζει γραμμή. Συμβουλή: πατήστε παρατεταμένα οποιοδήποτε μήνυμα για επεξεργασία, διακλάδωση ή διαγραφή."
      },
      "chatSend": {
        "title": "Ένα κουμπί, τέσσερις δουλειές",
        "body": "Το κουμπί αποστολής αλλάζει λειτουργία ανάλογα με τι συμβαίνει:"
      }
    },
    "postFirstMessage": {
      "chatRegenerate": {
        "title": "Δεν σας αρέσει; Αναδημιουργήστε",
        "body": "Πατήστε το εικονίδιο ανανέωσης για μια εντελώς νέα απάντηση από τον χαρακτήρα. Κάθε αναδημιουργία αποθηκεύεται ως παραλλαγή."
      },
      "chatVariants": {
        "title": "Σύρετε μεταξύ παραλλαγών",
        "body": "Μετά την αναδημιουργία, θα δείτε έναν μετρητή παραλλαγών κάτω από το μήνυμα. Σύρετε αριστερά ή δεξιά στο μήνυμα για να δείτε όλες τις διαφορετικές απαντήσεις."
      },
      "chatLongPress": {
        "title": "Κρύβεται κι άλλο εδώ",
        "body": "Πατήστε παρατεταμένα οποιοδήποτε μήνυμα για επεξεργασία, αντιγραφή, διακλάδωση, καρφίτσωμα, διαγραφή ή επαναφορά. Δεξί κλικ λειτουργεί επίσης στην επιφάνεια εργασίας."
      }
    },
    "sendButton": {
      "continue": {
        "label": "Συνέχεια",
        "desc": "Η εισαγωγή είναι κενή. Πατήστε για να ενθαρρύνετε τον χαρακτήρα να συνεχίσει."
      },
      "send": {
        "label": "Αποστολή",
        "desc": "Έχετε πληκτρολογήσει ή επισυνάψει κάτι. Πατήστε για αποστολή."
      },
      "sending": {
        "label": "Αποστέλλεται",
        "desc": "Η απάντηση είναι καθ' οδόν. Το κουμπί είναι κλειδωμένο."
      },
      "stop": {
        "label": "Διακοπή",
        "desc": "Πατήστε για ακύρωση αν αλλάξετε γνώμη κατά τη διάρκεια της απάντησης."
      }
    },
    "extra": {
      "rerunOnboarding": "Επανεκκίνηση εισαγωγής"
    }
  },
  "sessionAdvanced": {
    "title": "Παράμετροι Συνεδρίας",
    "subtitle": "Παράκαμψη προεπιλογών μοντέλου για αυτή τη συνομιλία",
    "goBack": "Πίσω",
    "support": "Υποστήριξη",
    "reset": "Επαναφορά",
    "save": "Αποθήκευση",
    "noSessionWarning": "Ανοίξτε μια συνεδρία συνομιλίας για να ρυθμίσετε τις παραμέτρους ανά συνεδρία.",
    "overrideDefaults": "Παράκαμψη προεπιλογών",
    "overrideDefaultsDesc": "Προσαρμόστε παραμέτρους μόνο για αυτή τη συνομιλία",
    "loadingContextInfo": "Φόρτωση πληροφοριών πλαισίου...",
    "sampling": {
      "title": "Δειγματοληψία",
      "temperature": "Temperature",
      "temperatureDesc": "Ελέγχει την τυχαιότητα. Χαμηλότερο = πιο ντετερμινιστικό, υψηλότερο = πιο δημιουργικό.",
      "temperaturePrecise": "Ακριβές",
      "temperatureCreative": "Δημιουργικό",
      "topP": "Top P",
      "topPDesc": "Nucleus sampling. Περιορίζει τα tokens σε αθροιστική πιθανότητα.",
      "topPFocused": "Εστιασμένο",
      "topPDiverse": "Ποικίλο",
      "topK": "Top K",
      "topKDesc": "Περιορίζει τη δειγματοληψία στα top K πιο πιθανά tokens."
    },
    "outputPenalties": {
      "title": "Έξοδος & Ποινές",
      "maxOutputTokens": "Μέγιστα Tokens Εξόδου",
      "maxOutputTokensDesc": "Μέγιστο μήκος απάντησης. Auto αφήνει το μοντέλο να αποφασίσει.",
      "auto": "Auto",
      "custom": "Προσαρμοσμένο",
      "frequencyPenalty": "Ποινή Συχνότητας",
      "frequencyPenaltyDesc": "Μειώνει την επανάληψη ακολουθιών tokens.",
      "frequencyPenaltyRepeat": "Επανάληψη",
      "frequencyPenaltyVary": "Ποικιλία",
      "presencePenalty": "Ποινή Παρουσίας",
      "presencePenaltyDesc": "Ενθαρρύνει την εξερεύνηση νέων θεμάτων.",
      "presencePenaltyRepeat": "Επανάληψη",
      "presencePenaltyExplore": "Εξερεύνηση"
    },
    "performance": {
      "title": "Απόδοση",
      "gpuLayers": "GPU Layers",
      "gpuLayersDesc": "Επίπεδα που μεταφορτώνονται στη GPU. 0 = μόνο CPU.",
      "threads": "Threads",
      "threadsDesc": "CPU threads για εξαγωγή συμπερασμάτων.",
      "batchThreads": "Batch Threads",
      "batchThreadsDesc": "CPU threads για μαζική επεξεργασία.",
      "batchSize": "Batch Size",
      "batchSizeDesc": "Μέγεθος τμήματος επεξεργασίας prompt.",
      "contextLength": "Μήκος Πλαισίου",
      "contextLengthDesc": "Παράκαμψη μεγέθους παραθύρου πλαισίου.",
      "flashAttention": "Flash Attention",
      "flashAttentionDesc": "Βελτιστοποίηση μνήμης GPU.",
      "enabled": "Ενεργό",
      "disabled": "Ανενεργό"
    },
    "samplingMemory": {
      "title": "Δειγματοληψία & Μνήμη",
      "minP": "Min P",
      "minPDesc": "Κατώτατο όριο πιθανότητας.",
      "typicalP": "Typical P",
      "typicalPDesc": "Τυπικό κατώτατο δειγματοληψίας.",
      "seed": "Seed",
      "seedDesc": "Τυχαίο seed. Αφήστε κενό για τυχαίο.",
      "ropeBase": "RoPE Base",
      "ropeBaseDesc": "Παράκαμψη βάσης συχνότητας.",
      "ropeScale": "RoPE Scale",
      "ropeScaleDesc": "Παράκαμψη κλίμακας συχνότητας.",
      "kvCacheType": "KV Cache Type",
      "kvCacheTypeDesc": "Κβαντοποίηση KV cache για εξοικονόμηση VRAM.",
      "offloadKqv": "Offload KQV",
      "offloadKqvDesc": "KV cache & KQV λειτουργίες στη GPU.",
      "on": "Ενεργό",
      "off": "Ανενεργό",
      "samplerProfile": "Προφίλ Δειγματολήπτη",
      "samplerProfileDesc": "Ρυθμισμένες προεπιλογές για σταθερότητα ή συλλογισμό.",
      "balanced": "Ισορροπημένο",
      "creative": "Δημιουργικό",
      "stable": "Σταθερό",
      "reasoning": "Συλλογισμός"
    },
    "systemInfo": {
      "title": "Πληροφορίες Συστήματος",
      "maxContext": "Μέγιστο Πλαίσιο",
      "recommended": "Συνιστώμενο",
      "availableRam": "Διαθέσιμη RAM",
      "availableVram": "Διαθέσιμη VRAM",
      "modelSize": "Μέγεθος Μοντέλου"
    }
  },
  "exportMenu": {
    "title": "Μορφή Εξαγωγής",
    "selectFormat": "Επιλέξτε μορφή",
    "uscTitle": "Unified System Card",
    "formats": {
      "uscPrompt": "Φορητή εξαγωγή USC για πρότυπα prompt.",
      "uscLorebook": "Φορητή εξαγωγή USC για βιβλία γνώσης.",
      "uscModel": "Φορητή εξαγωγή USC για προφίλ μοντέλων.",
      "uscChatTemplate": "Φορητή εξαγωγή USC για πρότυπα συνομιλίας.",
      "legacyPromptJson": "Legacy Prompt JSON",
      "legacyPromptJsonDesc": "Τρέχουσα μορφή εξωτερικού πακέτου prompt.",
      "lorebookJson": "Lorebook JSON",
      "lorebookJsonDesc": "Τρέχουσα μορφή εξαγωγής βιβλίου γνώσης.",
      "modelJson": "Model JSON",
      "modelJsonDesc": "Ασφαλές JSON προφίλ μοντέλου χωρίς διαπιστευτήρια.",
      "chatTemplateJson": "Chat Template JSON",
      "chatTemplateJsonDesc": "Εγγενής μορφή εξαγωγής προτύπου συνομιλίας."
    },
    "extra": {
      "selectFormat": "Επιλέξτε μορφή",
      "exportFormatTitle": "Μορφή Εξαγωγής",
      "uscDesc": "Φορητή εξαγωγή USC",
      "legacyJsonDesc": "Παλαιά μορφή εξαγωγής JSON",
      "formatV3Desc": "Εξαγωγή Character Card V3",
      "formatV2Desc": "Εξαγωγή Character Card V2",
      "formatV1Desc": "Εξαγωγή Character Card V1",
      "uscLorebook": "Unified System Card (USC)",
      "portableLorebook": "Φορητή εξαγωγή USC για βιβλία γνώσης",
      "lorebookJson": "Lorebook JSON",
      "currentLorebook": "Τρέχουσα μορφή εξαγωγής βιβλίου γνώσης",
      "uscModel": "Unified System Card (USC)",
      "portableModel": "Φορητή εξαγωγή USC για προφίλ μοντέλων",
      "modelJson": "Model JSON",
      "safeModel": "Ασφαλές JSON προφίλ μοντέλου χωρίς διαπιστευτήρια",
      "uscTemplate": "Unified System Card (USC)",
      "portableTemplate": "Φορητή εξαγωγή USC για πρότυπα συνομιλίας",
      "templateJson": "Chat Template JSON",
      "nativeTemplate": "Εγγενής μορφή εξαγωγής προτύπου συνομιλίας"
    }
  },
  "designReference": {
    "title": "Αναφορές σχεδίου",
    "description": "Ανεβάστε μερικές καθαρές εικόνες αναφοράς και μία κανονική οπτική περιγραφή.",
    "descriptionPlaceholder": "Περιγράψτε τη σταθερή εμφάνιση: πρόσωπο, μαλλιά, σωματότυπο, ηλικιακή παρουσίαση, ενδυματολογικές ενδείξεις, αξεσουάρ και κατεύθυνση στυλ.",
    "addReferences": "Προσθήκη αναφορών",
    "visualDescription": "Οπτική περιγραφή",
    "draftWithAi": "Σχέδιο με AI",
    "referenceImages": "Εικόνες αναφοράς",
    "imageAlt": "Αναφορά σχεδίου {{index}}",
    "loading": "Φόρτωση...",
    "removeAria": "Αφαίρεση αναφοράς σχεδίου",
    "noImages": "Δεν υπάρχουν εικόνες αναφοράς ακόμα",
    "imageCount": "{{count}} εικόνα(ες) αναφοράς επισυνάφθηκαν",
    "emptyReferences": "Προσθέστε μερικές καθαρές φωτογραφίες αναφοράς για να κλειδώσετε πρόσωπο, αναλογίες, ντύσιμο και στυλ.",
    "noWriterModel": "Προσθέστε πρώτα ένα συμβατό μοντέλο σεναριογράφου στις ρυθμίσεις Δημιουργίας Εικόνων.",
    "noImagesForGeneration": "Προσθέστε ένα avatar ή τουλάχιστον μία εικόνα αναφοράς πριν τη δημιουργία.",
    "writerModelHelp": "Χρησιμοποιεί το {{model}} για σχεδίαση από το avatar και τις εικόνες αναφοράς σας.",
    "noWriterModelHelp": "Προσθέστε ένα συμβατό μοντέλο σεναριογράφου στις ρυθμίσεις Δημιουργίας Εικόνων για αυτόματη σχεδίαση.",
    "draftMenuTitle": "Σχέδιο AI",
    "draftMenuDesc": "Σχεδιάστηκε από {{model}} από το τρέχον avatar και τις εικόνες αναφοράς.",
    "draftMenuNoWriter": "Προσθέστε ένα συμβατό μοντέλο σεναριογράφου πριν χρησιμοποιήσετε αυτό το βοήθημα.",
    "regenerate": "Αναδημιουργία",
    "useThis": "Χρήση Αυτού"
  },
  "samplerOrder": {
    "title": "Σειρά Δειγματολήπτη",
    "description": "Σύρετε τα στάδια για αναδιάταξη. Εκτελούνται από πάνω προς τα κάτω.",
    "reset": "Επαναφορά",
    "resetAria": "Επαναφορά σειράς δειγματολήπτη στις προεπιλογές",
    "stages": {
      "penalties": {
        "label": "Ποινές",
        "desc": "Εφαρμογή ποινών συχνότητας και παρουσίας πριν το φιλτράρισμα."
      },
      "grammar": {
        "label": "Γραμματική",
        "desc": "Περιορισμός tokens όταν είναι ενεργή η γραμματική προτύπου συνομιλίας."
      },
      "topK": {
        "label": "Top K",
        "desc": "Κόψιμο του πλήθους υποψηφίων στα ισχυρότερα tokens."
      },
      "topP": {
        "label": "Top P",
        "desc": "Εφαρμογή φιλτραρίσματος πυρήνα στην υπολειπόμενη κατανομή."
      },
      "minP": {
        "label": "Min P",
        "desc": "Απόρριψη tokens χαμηλής πιθανότητας με ελάχιστο κατώφλι."
      },
      "typical": {
        "label": "Typical P",
        "desc": "Προτίμηση στατιστικά τυπικών tokens στην τρέχουσα κατανομή."
      },
      "temp": {
        "label": "Temperature",
        "desc": "Ομαλοποίηση ή όξυνση της τελικής κατανομής πριν την επιλογή."
      }
    },
    "presets": {
      "default": {
        "label": "Προεπιλογή",
        "hint": "Lettuce local"
      },
      "unsloth": {
        "label": "Unsloth",
        "hint": "llama.cpp style"
      },
      "focused": {
        "label": "Εστιασμένο",
        "hint": "Αυστηρό κλάδεμα"
      },
      "creative": {
        "label": "Δημιουργικό",
        "hint": "Καθυστερημένο φίλτρο"
      }
    }
  },
  "voices": {
    "extra": {
      "customVoices": "Οι Φωνές Μου",
      "providerVoices": "Φωνές Παρόχου",
      "myVoices": "Οι Φωνές Μου",
      "page": {
        "noAudioProvidersHint": "Προσθέστε έναν στους Παρόχους > Ήχος για να ξεκινήσετε",
        "noVoicesTitle": "Δεν έχουν δημιουργηθεί φωνές ακόμα",
        "noVoicesDescription": "Δημιουργήστε φωνές με προσαρμοσμένες προτροπές για τους χαρακτήρες σας",
        "addProviderFirst": "Προσθέστε πρώτα έναν πάροχο ήχου",
        "noPrompt": "Χωρίς προτροπή",
        "noProviderVoices": "Δεν βρέθηκαν φωνές. Κάντε κλικ στο Ανανέωση.",
        "showLess": "Εμφάνιση λιγότερων",
        "showAllVoices": "Εμφάνιση όλων των {{count}} φωνών",
        "voiceFallbackTitle": "Φωνή"
      },
      "cache": {
        "section": "Κρυφή μνήμη ήχου",
        "title": "Κρυφή μνήμη TTS",
        "description": "Ο παραγόμενος ήχος αποθηκεύεται στην κρυφή μνήμη για μείωση αναπαραγωγών",
        "clearing": "Εκκαθάριση...",
        "clear": "Εκκαθάριση κρυφής μνήμης"
      },
      "menu": {
        "editDescription": "Τροποποίηση αυτής της φωνής",
        "deleteDescription": "Αφαίρεση αυτής της φωνής",
        "provider": "Πάροχος",
        "category": "Κατηγορία",
        "createVoiceConfig": "Δημιουργία ρύθμισης φωνής",
        "createVoiceConfigDescription": "Χρησιμοποιήστε αυτή τη φωνή με προσαρμοσμένες ρυθμίσεις"
      },
      "editor": {
        "editTitle": "Επεξεργασία Φωνής",
        "createTitle": "Δημιουργία Φωνής",
        "voiceName": "Όνομα Φωνής",
        "voiceNamePlaceholder": "Φωνή χαρακτήρα μου",
        "provider": "Πάροχος",
        "model": "Μοντέλο",
        "modelIdPlaceholder": "gpt-4o-mini-tts",
        "modelIdHint": "Εισάγετε το ακριβές ID μοντέλου που αναμένει το compatible endpoint σας",
        "elevenlabsVoice": "Φωνή ElevenLabs",
        "noVoicesAvailable": "Δεν υπάρχουν διαθέσιμες φωνές",
        "selectVoice": "Επιλέξτε φωνή...",
        "elevenlabsVoiceHint": "Επιλέξτε από τις φωνές ElevenLabs σας",
        "geminiVoice": "Φωνή Gemini",
        "geminiVoiceHint": "Επιλέξτε μια φωνή Gemini TTS",
        "voiceId": "ID Φωνής",
        "voiceIdPlaceholder": "alloy",
        "voiceIdHint": "Εισάγετε το ID φωνής που υποστηρίζεται από το compatible endpoint σας",
        "voicePrompt": "Προτροπή Φωνής",
        "voicePromptPlaceholder": "Μια ζεστή, φιλική φωνή με χαρούμενο τόνο...",
        "voicePromptHint": "Περιγράψτε πώς πρέπει να ακούγεται η φωνή",
        "exampleText": "Παράδειγμα κειμένου",
        "exampleTextPlaceholder": "Γεια! Έτσι ακούγομαι όταν μιλάω...",
        "exampleTextHint": "Δείγμα κειμένου για δοκιμή της φωνής",
        "voiceDesignChars": "{{current}}/{{minimum}} χαρακτήρες απαιτούνται για προεπισκόπηση σχεδιασμού φωνής",
        "defaultSample": "Γεια! Έτσι ακούγομαι όταν μιλάω. Μπορώ να διαβάσω μεγαλύτερα αποσπάσματα με ζεστασιά, σαφήνεια και συναίσθημα ώστε να κρίνετε τον τόνο και το ρυθμό μου.",
        "playing": "Αναπαραγωγή...",
        "previewVoice": "Προεπισκόπηση Φωνής"
      },
      "kokoro": {
        "title": "Kokoro Studio",
        "newBlend": "Νέο μείγμα",
        "editBlend": "Επεξεργασία μείγματος",
        "tryText": "Γεια! Αυτή είναι μια γρήγορη δοκιμή του πώς ακούγομαι.",
        "experimentDefaultText": "Γεια! Έτσι ακούγομαι όταν μιλάω. Μπορώ να διαβάσω μεγαλύτερα αποσπάσματα με ζεστασιά, σαφήνεια και συναίσθημα.",
        "livePreview": "Ζωντανή προεπισκόπηση",
        "savedBlend": "Αποθηκευμένο μείγμα",
        "defaultPreviewText": "Γεια! Αυτή είναι μια γρήγορη προεπισκόπηση του πώς ακούγεται αυτή η φωνή.",
        "experiment": "Πείραμα",
        "providerNotFound": "Δεν βρέθηκε πάροχος Kokoro",
        "backToProviders": "Επιστροφή στους παρόχους",
        "variantUnset": "Η παραλλαγή δεν έχει οριστεί",
        "ready": "Έτοιμο",
        "modelNotInstalled": "Το μοντέλο δεν είναι εγκατεστημένο",
        "voiceCount": "{{count}} φωνή",
        "engineActions": "Ενέργειες μηχανής",
        "engineNotInstalled": "Η μηχανή δεν είναι εγκατεστημένη",
        "installAtLeastOneVoice": "Εγκαταστήστε τουλάχιστον μία φωνή",
        "continueSetup": "Συνεχίστε τη ρύθμιση για να εγκαταστήσετε το μοντέλο Kokoro.",
        "pickVoiceOrStarter": "Επιλέξτε φωνή ή πάρτε το πακέτο εκκίνησης για να ξεκινήσετε.",
        "downloadsFailed": "{{count}} λήψη απέτυχε",
        "retryOrDismissAll": "Επαναλάβετε μεμονωμένα ή απορρίψτε όλα.",
        "dismissAll": "Απόρριψη όλων",
        "model": "Μοντέλο",
        "voice": "Φωνή",
        "downloads": "Λήψεις",
        "cancelAll": "Ακύρωση όλων",
        "experimentPlaceholder": "Πληκτρολογήστε μια φράση για να την ακούσετε...",
        "speed": "Ταχύτητα",
        "speak": "Ομιλία",
        "yourBlends": "Τα μείγματά σας",
        "noSavedBlends": "Δεν υπάρχουν αποθηκευμένα μείγματα ακόμα.",
        "installModelAndVoiceFirst": "Εγκαταστήστε πρώτα το μοντέλο και μια φωνή.",
        "featured": "Προτεινόμενα",
        "stop": "Διακοπή",
        "sample": "Δείγμα",
        "voiceLibrary": "Βιβλιοθήκη φωνών",
        "starterPack": "Πακέτο εκκίνησης",
        "select": "Επιλογή",
        "all": "Όλα",
        "installed": "Εγκατεστημένα",
        "installModelToBrowse": "Εγκαταστήστε το μοντέλο για περιήγηση στις φωνές.",
        "noVoicesInCatalog": "Δεν υπάρχουν φωνές στον κατάλογο. Πατήστε Ανανέωση.",
        "noVoicesMatch": "Δεν υπάρχουν φωνές που να ταιριάζουν με τα φίλτρά σας.",
        "collapseAll": "Σύμπτυξη όλων",
        "expandAll": "Ανάπτυξη όλων",
        "selectedCount": "{{count}} επιλεγμένα",
        "engineTitle": "Μηχανή Kokoro",
        "variant": "Παραλλαγή",
        "status": "Κατάσταση",
        "notInstalled": "Μη εγκατεστημένο",
        "file": "Αρχείο",
        "modelSize": "Μέγεθος μοντέλου",
        "voicesSize": "Μέγεθος φωνών",
        "total": "Σύνολο",
        "assetRoot": "Ριζικός φάκελος",
        "reinstallModel": "Επανεγκατάσταση μοντέλου",
        "installModel": "Εγκατάσταση μοντέλου",
        "deleteModel": "Διαγραφή μοντέλου",
        "deleteModelDescription": "Ελευθερώνει χώρο στον δίσκο. Οι φωνές διατηρούνται.",
        "blend": "Μείγμα",
        "previewDescription": "Γρήγορη ακρόαση με προεπιλεγμένο κείμενο",
        "editBlendDescription": "Ρύθμιση φωνών, βαρών και ταχύτητας",
        "deleteBlendDescription": "Αφαίρεση αυτής της αποθηκευμένης φωνής",
        "setupTitle": "Ρύθμιση Kokoro",
        "allSet": "Όλα έτοιμα",
        "allSetDescription": "Περιηγηθείτε στις φωνές, σχεδιάστε μείγματα ή δοκιμάστε στην περιοχή πειραμάτων."
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
      "conditionalsSection": "Συνθήκες",
      "injectionPoints": "Σημεία Εισαγωγής"
    }
  },
  "embeddings": {
    "download": {
      "bestForQuick": "Καλύτερο για γρήγορες απαντήσεις",
      "balancedPerf": "Ισορροπημένη απόδοση",
      "maxContext": "Μέγιστο πλαίσιο",
      "capacity1k": "1K tokens",
      "capacity2k": "2K tokens",
      "capacity4k": "4K tokens",
      "approxSize": "~138 MB",
      "downloadVersion": "V4"
    },
    "test": {
      "health": "Έλεγχος υγείας",
      "retrieval": "Ανάκτηση",
      "separation": "Διαχωρισμός",
      "passed": "Επιτυχία",
      "failed": "Αποτυχία",
      "testing": "Δοκιμή..."
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
      "jsonDesc": "Συμπαγής δομημένη έξοδος όταν η κλήση εργαλείων δεν είναι διαθέσιμη.",
      "xml": "XML",
      "xmlDesc": "Χρήση όταν το μοντέλο μορφοποιεί XML πιο αξιόπιστα από JSON.",
      "fallbackFormat": "Μορφή εφεδρείας"
    }
  },
  "companion": {
    "download": {
      "emotionClassifier": "Ταξινομητής συναισθημάτων",
      "emotionClassifierDesc": "Διαβάζει τις στροφές και ενημερώνει τα εκφρασμένα, αισθητά και μπλοκαρισμένα διανύσματα συναισθήματος.",
      "emotionSize": "~120 MB",
      "entityExtractor": "Εξαγωγέας οντοτήτων (NER)",
      "entityExtractorDesc": "Αναγνωρίζει πρόσωπα, τόπους και αντικείμενα για κανονικοποίηση και σύνδεση αναμνήσεων.",
      "entitySize": "~140 MB",
      "memoryRouter": "Δρομολογητής μνήμης",
      "memoryRouterDesc": "Αποφασίζει αν νέες στροφές πρέπει να αποθηκευτούν ως σχέση, ορόσημο, επεισοδιακή ή άλλη κατηγορία μνήμης.",
      "routerSize": "~70 MB",
      "unknownModel": "Άγνωστο μοντέλο συντρόφου. Παρέχετε ?kind=emotion|ner|router."
    }
  },
  "lorebookGen": {
    "flow": {
      "pickerCharactersTitle": "Χαρακτήρες",
      "pickerSessionsTitle": "Συνεδρίες",
      "noCharacters": "Δεν υπάρχουν χαρακτήρες",
      "noSessions": "Δεν υπάρχουν συνεδρίες",
      "clearSelection": "Εκκαθάριση επιλογής",
      "directionTitle": "Προαιρετική κατεύθυνση δημιουργίας",
      "directionLabel": "Κατεύθυνση",
      "toggleForceMode": "Εναλλαγή λειτουργίας force",
      "entryTitlePlaceholder": "Τίτλος καταχώρησης",
      "entryContentPlaceholder": "Περιεχόμενο καταχώρησης lorebook",
      "editDirectionBeforeRegenerate": "Επεξεργασία κατεύθυνσης πριν αναδημιουργία",
      "generatorReturnedNoDraft": "Ο δημιουργός δεν επέστρεψε σχέδιο",
      "pageTitle": "Δημιουργία Καταχώρησης Lorebook",
      "missingContext": "Λείπει το πλαίσιο lorebook για τη σελίδα δημιουργού.",
      "characterLocked": "Ο χαρακτήρας είναι κλειδωμένος στον ιδιοκτήτη αυτού του lorebook",
      "chooseSession": "Επιλογή συνεδρίας",
      "pickCharacter": "Επιλογή χαρακτήρα",
      "searchMemories": "Αναζήτηση αναμνήσεων",
      "searchMessages": "Αναζήτηση μηνυμάτων",
      "selectLastN": "Επιλογή τελευταίων {{n}} μηνυμάτων",
      "selectAll": "Επιλογή όλων",
      "loadSessionPrompt": "Επιλέξτε συνεδρία για φόρτωση",
      "messagesText": "μηνύματα",
      "memoriesText": "αναμνήσεις",
      "messagesAndMemories": "μηνύματα και αναμνήσεις",
      "titleAndContentRequired": "Απαιτούνται τίτλος και περιεχόμενο",
      "keywordsOrAlwaysActive": "Προσθέστε τουλάχιστον μία λέξη-κλειδί ή ενεργοποιήστε το Πάντα ενεργό",
      "lorybookEntrySaved": "Η καταχώρηση lorebook αποθηκεύτηκε",
      "saveFailed": "Αποτυχία αποθήκευσης",
      "generationFailed": "Αποτυχία δημιουργίας",
      "failedToLoadContext": "Αποτυχία φόρτωσης δημιουργού lorebook",
      "failedToLoadSessions": "Αποτυχία φόρτωσης συνεδριών",
      "failedToLoadMessages": "Αποτυχία φόρτωσης μηνυμάτων",
      "userRole": "ΧΡΗΣΤΗΣ",
      "aiRole": "AI"
    },
    "triggerPreview": {
      "resizeInspector": "Αλλαγή μεγέθους επιθεωρητή"
    }
  },
  usageAnalytics: {
    page: {
      filters: "Φίλτρα",
      model: "Μοντέλο",
      character: "Χαρακτήρας",
      clearAll: "Εκκαθάριση όλων",
      applyFilters: "Εφαρμογή φίλτρων",
      recentActivity: "Πρόσφατη δραστηριότητα",
      customRange: "Προσαρμοσμένο εύρος",
      startDate: "Ημερομηνία έναρξης",
      endDate: "Ημερομηνία λήξης",
      applyRange: "Εφαρμογή εύρους",
      dashboard: "ΠΙΝΑΚΑΣ ΕΛΕΓΧΟΥ",
      appTime: "ΧΡΟΝΟΣ ΕΦΑΡΜΟΓΗΣ",
      today: "Σήμερα",
      last7Days: "7 ημέρες",
      last30Days: "30 ημέρες",
      all: "Όλα",
      custom: "Προσαρμοσμένο",
      filtersCount: "{{count}} φίλτρα",
      totalCost: "Συνολικό κόστος",
      tokens: "Tokens",
      avgShort: "μέσος {{value}}",
      requests: "Αιτήματα",
      period: "Περίοδος",
      last7d: "Τελευταίες 7 ημέρες",
      last30d: "Τελευταίες 30 ημέρες",
      allTime: "Όλη την περίοδο",
      recordsCount: "{{count}} εγγραφές",
      usageTrend: "Τάση χρήσης",
      tokenConsumptionOverTime: "Κατανάλωση tokens ανά χρόνο",
      input: "Είσοδος",
      output: "Έξοδος",
      byModel: "Ανά μοντέλο",
      byCharacter: "Ανά χαρακτήρα",
      top: "Κορυφαία",
      active: "Ενεργά",
      quickView: "Γρήγορη προβολή",
      viewAll: "Προβολή όλων",
      startChatting: "Ξεκινήστε συνομιλία για να δείτε δεδομένα χρήσης",
      exportedTo: "Εξαγωγή σε: {{path}}",
      periodTotal: "Σύνολο περιόδου",
      daysActive: "{{count}} ενεργές ημέρες",
      dailyAvg: "Ημερήσιος μέσος",
      selectedPeriod: "επιλεγμένη περίοδος",
      yesterdayValue: "Χθες {{value}}",
      thirtyDayAvg: "Μέσος 30 ημερών",
      appTimeTrend: "Τάση χρόνου εφαρμογής",
      usageDurationPerDay: "Διάρκεια χρήσης ανά ημέρα",
      totalValue: "Σύνολο {{value}}",
      activeTime: "Ενεργός χρόνος",
    },
    activity: {
      loading: "Φόρτωση δραστηριότητάς σας...",
      title: "Πρόσφατη δραστηριότητα",
      recordsCount: "{{count}} εγγραφές χρήσης",
      rangeOfTotal: "{{start}}-{{end}} από {{total}}",
      previous: "Προηγούμενο",
      next: "Επόμενο",
      pageOf: "Σελίδα {{page}} από {{total}}",
    },
    shared: {
      relativeTime: {
        justNow: "μόλις τώρα",
        minutesAgo: "πριν {{count}}λ",
        hoursAgo: "πριν {{count}}ω",
        daysAgo: "πριν {{count}}η",
      },
      operations: {
        chat: "Συνομιλία",
        regenerate: "Αναδημιουργία",
        continue: "Συνέχεια",
        summary: "Περίληψη",
        memoryManager: "Μνήμη",
        imageGeneration: "Δημιουργία εικόνας",
        aiCreator: "AI Δημιουργός",
        replyHelper: "Βοηθός απάντησης",
        groupChatMessage: "Ομαδική συνομιλία",
        groupChatRegenerate: "Ομαδική αναδημιουργία",
        groupChatContinue: "Ομαδική συνέχεια",
        groupChatDecisionMaker: "Λήψης αποφάσεων",
      },
      outputImages: "{{count}} εικόνες",
      tokens: "{{count}} tokens",
      unknown: "Άγνωστο",
      requestDetails: "Λεπτομέρειες αιτήματος",
      noStopReason: "Χωρίς λόγο διακοπής",
      tokenUsage: "Χρήση tokens",
      estimatedCost: "Εκτιμώμενο κόστος",
      stats: {
        prompt: "Προτροπή",
        completion: "Ολοκλήρωση",
        total: "Σύνολο",
        reasoning: "Συλλογισμός",
        image: "Εικόνα",
        memory: "Μνήμη",
        summary: "Περίληψη",
        inputImages: "Εικόνες εισόδου",
        outputImages: "Εικόνες εξόδου",
        cachedPrompt: "Αποθηκευμένη προτροπή",
        cacheWrite: "Εγγραφή cache",
        webSearches: "Αναζητήσεις ιστού",
        cacheRead: "Ανάγνωση cache",
        requestFee: "Χρέωση αιτήματος",
        webSearch: "Αναζήτηση ιστού",
        providerTotal: "Σύνολο παρόχου",
      },
    },
  },
};
