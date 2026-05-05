import type { DeepPartialMessageTree } from "../types";
import { type LocaleMessages } from "./en";
import type { LocaleMetadata } from ".";

export const frMetadata: LocaleMetadata = {
  name: "French",
  label: "Français",
};

export const frMessages: DeepPartialMessageTree<LocaleMessages> = {
  "common": {
    "nav": {
      "chats": "Discussions",
      "settings": "Paramètres",
      "providers": "Fournisseurs",
      "responseStyle": "Style de réponse",
      "models": "Modèles",
      "security": "Sécurité",
      "accessibility": "Accessibilité",
      "reset": "Réinitialiser",
      "backupRestore": "Sauvegarde et restauration",
      "convertFiles": "Convertir des fichiers",
      "usageAnalytics": "Statistiques d'utilisation",
      "changelog": "Journal des modifications",
      "about": "À propos",
      "createSystemPrompt": "Créer un prompt système",
      "editSystemPrompt": "Modifier le prompt système",
      "systemPrompts": "Prompts système",
      "developer": "Développeur",
      "advanced": "Avancé",
      "characters": "Personnages",
      "lorebooks": "Encyclopédies",
      "personas": "Personas",
      "dynamicMemory": "Mémoire dynamique",
      "creationHelper": "Assistant de création",
      "helpMeReply": "Aidez-moi à répondre",
      "editPersona": "Modifier le persona",
      "newTemplate": "Nouveau modèle",
      "editTemplate": "Modifier le modèle",
      "chatTemplates": "Modèles de discussion",
      "editCharacter": "Modifier le personnage",
      "sync": "Synchronisation",
      "newCharacter": "Nouveau personnage",
      "engineSetup": "Configuration du moteur",
      "llmProviders": "Fournisseurs LLM",
      "engineSettings": "Paramètres du moteur",
      "lettuceEngine": "Moteur Lettuce",
      "create": "Créer",
      "setup": "Configuration",
      "welcome": "Bienvenue",
      "conversation": "Conversation",
      "library": "Bibliothèque",
      "groupChats": "Discussions de groupe",
      "groupChat": "Discussion de groupe",
      "imageGeneration": "Génération d'images",
      "voices": "Voix",
      "chatAppearance": "Apparence du chat",
      "colorCustomization": "Couleurs personnalisées",
      "embeddingDownload": "Téléchargement du modèle d'embedding",
      "embeddingTest": "Test d'embedding",
      "editModel": "Modifier le modèle",
      "messageDebug": "Débogage des messages",
      "speechRecognition": "Reconnaissance vocale",
      "hostApi": "Serveur API",
      "lorebookEntryGenerator": "Générateur d'entrées de lorebook",
      "companionSoulWriter": "Rédacteur d'âme du compagnon"
    },
    "bottomNav": {
      "chats": "Discussions",
      "groups": "Groupes",
      "create": "Créer",
      "discover": "Découvrir",
      "library": "Bibliothèque"
    },
    "buttons": {
      "cancel": "Annuler",
      "confirm": "Confirmer",
      "save": "Enregistrer",
      "saving": "Enregistrement...",
      "delete": "Supprimer",
      "deleting": "Suppression...",
      "create": "Créer",
      "creating": "Création...",
      "edit": "Modifier",
      "back": "Retour",
      "done": "Terminé",
      "download": "Télécharger",
      "later": "Plus tard",
      "proceed": "Continuer",
      "retry": "Réessayer",
      "discard": "Abandonner",
      "import": "Importer",
      "importing": "Importation...",
      "export": "Exporter",
      "exporting": "Exportation...",
      "update": "Mettre à jour",
      "generate": "Générer",
      "refresh": "Actualiser",
      "continue": "Continuer",
      "goBack": "Retour",
      "search": "Rechercher",
      "clearSearch": "Effacer la recherche",
      "add": "Ajouter",
      "install": "Installer",
      "remove": "Supprimer",
      "rename": "Renommer",
      "copy": "Copier",
      "copied": "Copié !",
      "browseFiles": "Parcourir les fichiers"
    },
    "labels": {
      "processing": "Traitement...",
      "loading": "Chargement...",
      "noDescriptionYet": "Pas encore de description",
      "untitled": "Sans titre",
      "default": "Par défaut",
      "enabled": "Activé",
      "disabled": "Désactivé",
      "on": "Activé",
      "off": "Désactivé",
      "none": "Aucun",
      "auto": "Auto",
      "custom": "Personnalisé",
      "name": "Nom",
      "description": "Description",
      "content": "Contenu",
      "preview": "Aperçu",
      "options": "Options"
    },
    "time": {
      "justNow": "À l'instant",
      "minutesAgo": "il y a {{minutes}}min",
      "hoursAgo": "il y a {{hours}}h",
      "daysAgo": "il y a {{days}}j",
      "today": "Aujourd'hui",
      "yesterday": "Hier",
      "last7Days": "7 derniers jours",
      "older": "Plus anciens"
    }
  },
  "settings": {
    "sections": {
      "intelligence": "Intelligence",
      "experience": "Expérience",
      "connectivity": "Connectivité",
      "securityPrivacy": "Sécurité et confidentialité",
      "supportInfo": "Support et infos",
      "dangerZone": "Zone de danger",
      "developer": "Développeur"
    },
    "items": {
      "providers": {
        "title": "Fournisseurs",
        "subtitle": "Se connecter aux services IA"
      },
      "models": {
        "title": "Modèles",
        "subtitle": "Configurer les modèles IA"
      },
      "imageGeneration": {
        "title": "Génération d'images",
        "subtitle": "Générer et tester des images"
      },
      "voices": {
        "title": "Voix",
        "subtitle": "Voix de synthèse vocale"
      },
      "accessibility": {
        "title": "Accessibilité",
        "subtitle": "Sons et retour haptique"
      },
      "prompts": {
        "title": "Prompts système",
        "subtitle": "Façonner la personnalité de l'IA"
      },
      "security": {
        "title": "Sécurité",
        "subtitle": "Chiffrement et confidentialité"
      },
      "backup": {
        "title": "Sauvegarde et restauration",
        "subtitle": "Exporter ou importer des données"
      },
      "convert": {
        "title": "Convertir des fichiers",
        "subtitle": "Migrer les exports .json en .uec"
      },
      "sync": {
        "title": "Synchronisation locale",
        "subtitle": "Synchroniser entre appareils"
      },
      "usage": {
        "title": "Statistiques d'utilisation",
        "subtitle": "Coûts et statistiques de tokens"
      },
      "advanced": {
        "title": "Avancé",
        "subtitle": "Mémoire et fonctionnalités"
      },
      "logs": {
        "title": "Journaux",
        "subtitle": "Débogage et diagnostics"
      },
      "guide": {
        "title": "Guide de configuration",
        "subtitle": "Relancer l'introduction"
      },
      "docs": {
        "title": "Documentation",
        "subtitle": "Guides et références"
      },
      "github": {
        "title": "Signaler des problèmes",
        "subtitle": "Bugs et retours • v{{version}}"
      },
      "discord": {
        "title": "Rejoindre Discord",
        "subtitle": "Communauté et aide"
      },
      "about": {
        "title": "À propos",
        "subtitle": "Version, mises à jour et liens"
      },
      "changelog": {
        "title": "Journal des modifications",
        "subtitle": "Nouveautés"
      },
      "reset": {
        "title": "Réinitialiser",
        "subtitle": "Tout effacer"
      },
      "developer": {
        "title": "Outils développeur",
        "subtitle": "Débogage et tests"
      }
    }
  },
  "accessibility": {
    "sectionTitles": {
      "language": "Langue",
      "sounds": "Retour sonore",
      "haptics": "Retour haptique",
      "appearance": "Apparence"
    },
    "language": {
      "appLanguage": "Langue de l'application",
      "description": "Choisissez la langue utilisée dans la navigation et les paramètres."
    },
    "appearance": {
      "customColors": "Couleurs personnalisées",
      "customColorsDesc": "Personnaliser le thème de couleurs de l'application",
      "chatAppearance": "Apparence du chat",
      "chatAppearanceDesc": "Personnaliser les bulles, polices et mise en page"
    },
    "haptics": {
      "vibrateOnChat": "Vibrer pendant le chat",
      "vibrateDesc": "Courtes vibrations pendant que l'assistant écrit",
      "intensity": "Intensité",
      "light": "Léger",
      "medium": "Moyen",
      "heavy": "Fort",
      "soft": "Doux",
      "rigid": "Rigide"
    },
    "sounds": {
      "send": "Envoi",
      "sendDescription": "Se joue quand vous envoyez un message",
      "success": "Succès",
      "successDescription": "Se joue quand l'assistant termine avec succès",
      "failure": "Échec",
      "failureDescription": "Se joue en cas d'erreur ou d'abandon",
      "testButton": "Tester"
    },
    "feedbackInfo": "Les retours vous aident à remarquer quand les messages sont envoyés ou reçus.",
    "hapticsInfo": "Le retour haptique est disponible sur les appareils mobiles.",
    "extra": {
      "easterEggs": "Easter eggs",
      "beetrootRain": "Pluie de betteraves",
      "beetrootDesc": "Des betteraves tombent quand les chats les mentionnent"
    }
  },
  "topNav": {
    "unsavedChangesTitle": "Modifications non enregistrées",
    "unsavedChangesMessage": "Enregistrez ou abandonnez vos modifications avant de quitter.",
    "goBack": "Retour",
    "changeLayout": "Changer la mise en page",
    "search": "Rechercher",
    "settings": "Paramètres",
    "help": "Aide",
    "add": "Ajouter",
    "openFilters": "Ouvrir les filtres",
    "save": "Enregistrer",
    "extra": {
      "installedModels": "Modèles installés",
      "refresh": "Actualiser",
      "minimize": "Réduire",
      "maximize": "Agrandir",
      "close": "Fermer"
    }
  },
  "updates": {
    "available": {
      "title": "Nouvelle version disponible",
      "description": "v{{latestVersion}} est disponible. Vous utilisez la v{{currentVersion}}.",
      "actions": {
        "view": "Voir la version"
      }
    }
  },
  "about": {
    "kicker": "Infos sur l’app",
    "appName": "LettuceAI",
    "description": "Détails de version, vérification des mises à jour et liens utiles.",
    "info": {
      "version": "Version",
      "channel": "Canal",
      "platform": "Plateforme"
    },
    "buildChannel": {
      "dev": "Version de développement",
      "release": "Version stable"
    },
    "update": {
      "sectionTitle": "Mises à jour",
      "title": "Mises à jour de l’app",
      "description": "Vérifiez manuellement les nouvelles versions ou désactivez les vérifications automatiques au démarrage.",
      "autoChecks": "Vérifications automatiques des mises à jour",
      "autoChecksDescription": "Lorsque cette option est activée, LettuceAI recherche de nouvelles versions à l’ouverture de l’app.",
      "checkNow": "Vérifier les mises à jour",
      "checking": "Vérification des mises à jour...",
      "upToDateTitle": "Vous êtes à jour",
      "upToDateDescription": "Aucune version plus récente n’est disponible pour cet appareil pour le moment.",
      "failedTitle": "Échec de la vérification des mises à jour",
      "failedDescription": "Impossible de vérifier les mises à jour pour le moment. Réessayez dans un instant."
    },
    "links": {
      "sectionTitle": "Liens",
      "website": "Site web",
      "websiteDescription": "Page de téléchargement et informations sur les versions",
      "github": "GitHub",
      "githubDescription": "Code source, issues et développement",
      "discord": "Discord",
      "discordDescription": "Serveur communautaire et assistance",
      "reddit": "Reddit",
      "redditDescription": "Discussions communautaires, retours et actualités"
    },
    "developerMode": {
      "enable": "Activer le mode développeur",
      "enabled": "Mode développeur activé"
    },
    "errors": {
      "saveTitle": "Impossible d’enregistrer la préférence",
      "saveDescription": "Votre préférence de vérification des mises à jour n’a pas été modifiée."
    }
  },
  "components": {
    "tooltip": {
      "dismissHint": "Appuyez n'importe où pour fermer"
    },
    "backgroundPosition": {
      "title": "Positionner l'arrière-plan",
      "instructions": "Glisser pour positionner • Pincer ou défiler pour zoomer"
    },
    "avatarSource": {
      "generateImage": "Générer une image",
      "generateImageDesc": "Création d'avatar par IA",
      "noImageModels": "Aucun modèle d'image disponible",
      "editCurrent": "Modifier l'actuel",
      "editCurrentDesc": "Repositionner ou recadrer",
      "chooseImage": "Choisir une image",
      "chooseImageDesc": "Sélectionner depuis votre appareil"
    },
    "avatarCurrentEdit": {
      "title": "Modifier le courant",
      "reposition": "Repositionner",
      "repositionDesc": "Déplacer ou recadrer l'avatar actuel",
      "editWithAI": "Modifier avec l'IA",
      "editWithAIDesc": "Ouvrir l'édition IA pour l'avatar actuel",
      "noImageModels": "Aucun modèle d'image disponible"
    },
    "avatarGeneration": {
      "modelsLoadError": "Échec du chargement des modèles de génération d'images",
      "title": "Générer un avatar",
      "help": "Aide à la génération d'avatar",
      "model": "Modèle",
      "selectModel": "Sélectionner un modèle",
      "describe": "Décrivez votre avatar",
      "describePlaceholder": "Une fille anime sympathique avec des cheveux colorés, souriant chaleureusement...",
      "inProgress": "Génération de l'avatar...",
      "editingInProgress": "Application de la modification de l'avatar...",
      "previousVariant": "Variante précédente",
      "nextVariant": "Variante suivante",
      "variantCounter": "{{current}} / {{total}}",
      "editRequest": "Modifier la demande",
      "editRequestPlaceholder": "Rendre les cheveux plus foncés, ajouter des lunettes, garder le visage identique...",
      "applyEdit": "Appliquer Modifier",
      "editImageLoadError": "Échec de la préparation de l'avatar généré pour la modification",
      "aiAssistant": "Assistant IA",
      "backToResults": "Retour à l'invite",
      "magicInTheWorks": "De la magie en marche...",
      "refine": "Affiner",
      "apply": "Appliquer",
      "alt": "Avatar généré",
      "regenerate": "Régénérer",
      "useThis": "Utiliser celui-ci"
    },
    "avatarPosition": {
      "title": "Positionner l'avatar",
      "instructions": "Glisser pour positionner • Pincer ou défiler pour zoomer",
      "alt": "Avatar à positionner"
    },
    "confirmDialog": {
      "defaultTitle": "Confirmer",
      "defaultLabel": "Confirmer"
    },
    "bottomMenu": {
      "defaultTitle": "Menu",
      "dragTip": "Glisser pour fermer le menu",
      "closeLabel": "Fermer le menu",
      "buttonProcessing": "Traitement..."
    },
    "modelSelector": {
      "placeholder": "Sélectionner un modèle",
      "clearLabel": "Utiliser le modèle par défaut",
      "loading": "Chargement des modèles...",
      "noModels": "Aucun modèle disponible",
      "addProviderFirst": "Ajoutez d'abord un fournisseur dans les paramètres",
      "title": "Sélectionner un modèle",
      "searchPlaceholder": "Rechercher des modèles...",
      "noResults": "Aucun modèle trouvé",
      "noResultsHint": "Essayez un autre terme de recherche"
    },
    "localeSelector": {
      "title": "Sélectionnez la langue"
    },
    "promptTemplate": {
      "nameContentRequired": "Le nom et le contenu sont requis",
      "saveError": "Échec de l'enregistrement du modèle",
      "editTitle": "Modifier le prompt",
      "createTitle": "Créer un prompt",
      "name": "Nom",
      "namePlaceholder": "ex. Maître du jeu de rôle",
      "content": "Contenu",
      "variablesButton": "Variables",
      "contentPlaceholder": "Vous êtes un assistant IA serviable...\n\nUtilisez {{char.name}} et {{scene}} dans votre prompt.",
      "characterCount": "{{count}} caractères",
      "preview": "Aperçu",
      "characterPlaceholder": "Personnage…",
      "personaPlaceholder": "Persona…",
      "rendering": "Rendu…",
      "noPreview": "Pas encore d'aperçu",
      "saving": "Enregistrement...",
      "update": "Mettre à jour",
      "create": "Créer",
      "variablesTitle": "Variables du modèle",
      "variablesCopyHint": "Appuyez pour copier dans le presse-papiers",
      "variablesCopied": "Copié",
      "variables": {
        "charName": "Nom du personnage",
        "charNameDesc": "Nom du personnage",
        "charDesc": "Description du personnage",
        "charDescDesc": "Description du personnage",
        "scene": "Scène",
        "sceneDesc": "Scène/scénario de départ",
        "userName": "Nom de l'utilisateur",
        "userNameDesc": "Nom du persona utilisateur",
        "userDesc": "Description de l'utilisateur",
        "userDescDesc": "Description du persona utilisateur",
        "rules": "Règles",
        "rulesDesc": "Règles de comportement du personnage",
        "contextSummary": "Résumé du contexte",
        "contextSummaryDesc": "Résumé dynamique de la conversation",
        "keyMemories": "Souvenirs clés",
        "keyMemoriesDesc": "Liste des souvenirs pertinents"
      }
    },
    "characterExport": {
      "title": "Format d'exportation",
      "selectFormat": "Sélectionner un format",
      "loading": "Chargement des formats...",
      "formatUecDesc": "Format Unified Entity Card (.uec) (recommandé).",
      "formatLegacyJsonDesc": "JSON hérité (importation uniquement).",
      "formatV3Desc": "Character Card V3 JSON (dernière spécification).",
      "formatV2Desc": "Character Card V2 JSON (spécification Tavern).",
      "formatV1Desc": "Character Card V1 (importation uniquement)."
    },
    "embeddingDownload": {
      "downloadRequired": "Téléchargement requis",
      "modelRequired": "Modèle d'embedding requis",
      "description": "La mémoire dynamique nécessite le téléchargement d'un modèle d'embedding local (~260 Mo) pour fonctionner.",
      "localStorage": "• Le modèle sera stocké localement sur votre appareil",
      "downloadSize": "• Taille du téléchargement : environ 260 Mo",
      "summarization": "• Nécessaire pour la synthèse des conversations"
    },
    "embeddingUpgrade": {
      "title": "Modèle d'embedding v3 disponible",
      "v1Message": "Vous utilisez la v1 avec 512 tokens. Passez à la v3 pour une meilleure qualité de mémoire et le support du contexte long.",
      "v2Message": "Vous utilisez l'ancienne v2. Passez à la v3 pour une meilleure qualité de mémoire avec le dernier modèle d'embedding.",
      "button": "Passer à la v3",
      "v3Message": "v4 est disponible et améliore considérablement la mémoire de roleplay par rapport à v3 (recall@1 0.02 -> 0.92). La mise à niveau est recommandée."
    },
    "v2UpgradeToast": {
      "title": "Modèle de mémoire v3",
      "badge": "Disponible",
      "message": "Qualité d'embedding améliorée par rapport à la v2",
      "dismiss": "Ignorer",
      "upgrade": "Mettre à jour"
    },
    "v1UpgradeToast": {
      "title": "Modèle de mémoire v3 disponible",
      "message": "Mettez à jour pour une meilleure qualité de mémoire et le support du contexte long.",
      "dismiss": "Ignorer",
      "upgrade": "Mettre à jour"
    },
    "createMenu": {
      "title": "Créer",
      "smartCreator": "Créateur intelligent",
      "smartCreatorDesc": "Laissez l'assistant guider votre création",
      "divider": "Ou créer manuellement",
      "character": "Personnage",
      "characterDesc": "Créer un personnage personnalisé",
      "persona": "Persona",
      "personaDesc": "Créer une voix réutilisable",
      "groupChat": "Discussion de groupe",
      "groupChatDesc": "Discuter avec plusieurs personnages",
      "lorebook": "Encyclopédie",
      "lorebookDesc": "Construire votre référence de monde",
      "characterSmartDesc": "Construire un personnage avec la création guidée",
      "personaSmartDesc": "Créer une voix ou personnalité réutilisable",
      "lorebookSmartDesc": "Construire une référence de monde structurée",
      "loadingConversations": "Chargement des conversations...",
      "createNew": "Créer nouveau",
      "createNewDesc": "Démarrer une nouvelle discussion de création guidée",
      "editExisting": "Modifier existant",
      "continueLast": "Continuer la dernière conversation",
      "seeOlder": "Voir les conversations plus anciennes",
      "seeOlderDesc": "Ouvrir n'importe quelle conversation précédente du Créateur intelligent",
      "noConversations": "Pas encore de conversations pour ce créateur.",
      "sessionCompleted": "Terminé",
      "sessionCancelled": "Annulé",
      "sessionDraft": "Brouillon",
      "sessionMessages": "{{count}} messages",
      "untitledConversation": "Conversation sans titre",
      "nameLorebookTitle": "Nommer l'encyclopédie",
      "lorebookNamePlaceholder": "Entrez le nom de l'encyclopédie...",
      "lorebookImporting": "Importation...",
      "lorebookImport": "Importer",
      "lorebookCreating": "Création...",
      "lorebookCreate": "Créer",
      "editExistingDesc": "Choisir un(e) {{goal}} et le/la modifier avec le Créateur intelligent",
      "creatorTitle": "Créateur de {{goal}}",
      "editTitle": "Modifier {{goal}}",
      "conversationsTitle": "Conversations {{goal}}",
      "loadingItems": "Chargement des {{items}}...",
      "noItemsFound": "Aucun(e) {{items}} trouvé(e).",
      "unnamedCharacter": "Personnage sans nom",
      "untitledPersona": "Persona sans titre",
      "untitledLorebook": "Encyclopédie sans titre"
    },
    "advancedModelSettings": {
      "temperature": "Température",
      "temperatureDesc": "Plus élevé = plus créatif",
      "topP": "Top P",
      "topPDesc": "Plus bas = plus focalisé",
      "maxTokens": "Tokens de sortie max.",
      "maxTokensDesc": "Laisser vide pour la valeur par défaut",
      "contextLength": "Longueur du contexte",
      "contextLengthDesc": "Modèles locaux uniquement",
      "contextLengthAuto": "Auto",
      "frequencyPenalty": "Pénalité de fréquence",
      "frequencyPenaltyDesc": "Réduire la répétition des tokens",
      "presencePenalty": "Pénalité de présence",
      "presencePenaltyDesc": "Encourager de nouveaux sujets",
      "topK": "Top K",
      "topKDesc": "Limiter la taille du pool de tokens",
      "reasoning": "Réflexion / Raisonnement",
      "reasoningAutoDesc": "Ce modèle utilise toujours le raisonnement. Aucune configuration nécessaire.",
      "reasoningEnableDesc": "Activer les capacités de réflexion avancées pour la résolution de problèmes complexes et les tâches de raisonnement.",
      "effortMode": "Mode effort",
      "budgetMode": "Mode budget",
      "reasoningEffort": "Effort de raisonnement",
      "reasoningEffortDesc": "Contrôle la profondeur de réflexion",
      "reasoningEffortAuto": "Auto",
      "reasoningEffortLow": "Bas",
      "reasoningEffortMed": "Moyen",
      "reasoningEffortHigh": "Élevé",
      "reasoningBudget": "Budget de raisonnement (tokens)",
      "reasoningBudgetDesc": "Tokens max. réservés à la réflexion",
      "reasoningEffortLowDesc": "Réponses rapides avec un raisonnement minimal",
      "reasoningEffortMediumDesc": "Profondeur de raisonnement équilibrée",
      "reasoningEffortHighDesc": "Profondeur de raisonnement maximale pour les problèmes complexes",
      "reasoningBudgetExtendedDesc": "Tokens max. réservés à la réflexion. Ajoutés à la limite de sortie."
    },
    "providerParameterSupport": {
      "unknownProvider": "Fournisseur inconnu : {{providerId}}",
      "reasoningNotSupportedEffort": "Non pris en charge - ce fournisseur n'utilise pas de niveaux d'effort",
      "reasoningNotSupportedBudgetOnly": "Non pris en charge - ce fournisseur utilise une approche budget uniquement",
      "reasoningNotSupported": "Non pris en charge - ce fournisseur ne supporte pas le raisonnement",
      "unsupportedParametersIgnored": "Les paramètres non pris en charge seront ignorés par {{providerName}}.",
      "reasoningEffortSupported": "L'effort de raisonnement est supporté pour les modèles pensants (o1, DeepSeek-R1, etc.)",
      "reasoningBudgetSupported": "Ce fournisseur utilise un budget de réflexion (pas de niveaux d'effort). Définissez plutôt les tokens de budget de raisonnement.",
      "reasoningNotSupportedProvider": "Ce fournisseur ne supporte pas les paramètres de raisonnement.",
      "matrixTitle": "Matrice de support des paramètres par fournisseur",
      "providerColumn": "Fournisseur",
      "supportedIndicator": "Supporté par l'API du fournisseur",
      "notSupportedIndicator": "Non supporté (le paramètre sera ignoré)"
    },
    "v3UpgradeToast": {
      "title": "Modèle de mémoire v4",
      "badge": "Disponible",
      "message": "v4 améliore considérablement la mémoire de roleplay par rapport à v3 (recall@1 0.02 → 0.92). La mise à niveau est recommandée.",
      "dismiss": "Plus tard",
      "upgrade": "Mettre à niveau"
    },
    "extra": {
      "promptCachingTitle": "Cache de prompt",
      "promptCachingDescription": "Accélère la génération et réduit les coûts pour les contextes longs et répétitifs (comme les grands prompts système ou les longs historiques de discussion).",
      "avatarAlt": "Avatar",
      "chooseFromLibrary": "Choisir dans la bibliothèque",
      "chooseFromLibraryDesc": "Utiliser une image déjà enregistrée dans l'application",
      "generateFailed": "Échec de la génération de l'image",
      "editFailed": "Échec de la modification de l'avatar",
      "backgroundAlt": "Arrière-plan à positionner",
      "formatsLoadFailed": "Échec du chargement des formats d'export",
      "formatsShowingDefaults": "(affichage des valeurs par défaut)",
      "timeJustNow": "à l'instant",
      "timeMinutesAgo": "il y a {{minutes}} min",
      "timeHoursAgo": "il y a {{hours}} h",
      "timeDaysAgo": "il y a {{days}} j",
      "removeReference": "Supprimer la référence de design",
      "thumbLoading": "Chargement...",
      "addReferences": "Ajouter des références",
      "visualDescription": "Description visuelle",
      "draftWithAi": "Rédiger avec l'IA",
      "regenerate": "Régénérer",
      "useThis": "Utiliser ceci",
      "referenceImagesLabel": "Images de référence",
      "writerHelpFallback": "le modèle de rédacteur de scènes compatible",
      "writerHelpUses": "Utilise {{model}} pour rédiger à partir de votre avatar et des images de référence.",
      "writerHelpUnavailable": "Ajoutez un modèle de rédacteur de scènes compatible dans les paramètres de Génération d'Images pour rédiger ceci automatiquement.",
      "writerNotAvailableError": "Ajoutez d'abord un modèle de rédacteur de scènes compatible dans les paramètres de Génération d'Images.",
      "writerNoSourcesError": "Ajoutez un avatar ou au moins une image de référence avant de générer.",
      "noUsableReferences": "Aucune image de référence utilisable trouvée.",
      "draftFailed": "Échec de la génération de la description du design.",
      "draftReadFailed": "Échec de la lecture de l'image ({{status}})",
      "draftConvertFailed": "Échec de la conversion de l'image en URL de données",
      "draftGenerationFailed": "La génération du brouillon a échoué.",
      "draftMenuTitle": "Brouillon de design par l'IA",
      "draftedBy": "Rédigé par {{model}} à partir de l'avatar actuel et des images de référence.",
      "draftedByFallback": "votre modèle de rédacteur de scènes",
      "noWriterUseHelper": "Ajoutez un modèle de rédacteur de scènes compatible avant d'utiliser cet assistant.",
      "emptyReferences": "Ajoutez quelques références claires pour figer le visage, les proportions, la tenue et le style.",
      "designReferencesTitle": "Références de design",
      "designReferencesDescription": "Téléversez quelques images de référence claires plus une description visuelle de référence.",
      "designReferencesPlaceholder": "Décrivez l'apparence stable : visage, cheveux, corpulence, âge apparent, indices de tenue, accessoires et style artistique.",
      "dismissAria": "Ignorer",
      "v3MessageFallback": "lettuce-emb-v4 est disponible et améliore considérablement la mémoire de roleplay. La mise à niveau est recommandée.",
      "uploadButton": "Téléverser",
      "libraryButton": "Bibliothèque",
      "companionSetupTitle": "Le compagnon a besoin d'être configuré",
      "companionSetupSubtitleSingle": "Le mode compagnon a besoin d'un modèle supplémentaire avant de pouvoir fonctionner. Ignorer ramènera ce personnage en mode Roleplay.",
      "companionSetupSubtitleMany": "Le mode compagnon a besoin de {{count}} modèles supplémentaires avant de pouvoir fonctionner. Ignorer ramènera ce personnage en mode Roleplay.",
      "companionSetupBody": "Le mode compagnon a besoin de quelques modèles locaux pour analyser les émotions, extraire les entités, router les souvenirs et rappeler le contexte passé.",
      "companionUseRoleplay": "Utiliser Roleplay à la place",
      "companionDownloadNow": "Télécharger maintenant",
      "searchModelsPlaceholder": "Rechercher des modèles...",
      "loadingModelsDefault": "Chargement des modèles...",
      "noModelsAvailable": "Aucun modèle disponible.",
      "noModelsMatching": "Aucun modèle ne correspond à \"{{query}}\".",
      "contentPlaceholderText": "Vous êtes un assistant IA serviable...\n\nUtilisez {{char.name}} et {{scene}} dans votre prompt.",
      "previewRenderFailed": "<échec du rendu de l'aperçu>",
      "charactersCount": "{{count}} caractères"
    }
  },
  "chats": {
    "characterNotFound": "Personnage introuvable",
    "chatHistory": "Historique des discussions",
    "previousConversationsWithCharacter": "Conversations précédentes avec {{name}}",
    "previousConversations": "Conversations précédentes",
    "searchChats": "Rechercher des discussions...",
    "searchResults": "{{count}} résultat(s)",
    "noConversationsYet": "Pas encore de conversations",
    "startChattingPrompt": "Commencez à discuter pour voir votre historique ici",
    "noMatchingChats": "Aucune discussion correspondante",
    "tryDifferentSearchTerm": "Essayez un autre terme de recherche",
    "untitledChat": "Discussion sans titre",
    "chatTitlePlaceholder": "Titre de la discussion...",
    "exportChatPackage": "Exporter le paquet de discussion",
    "characterSpecificExport": "Exportation spécifique au personnage",
    "characterSpecificExportDesc": "Lier ce paquet au personnage de la discussion par défaut.",
    "nonCharacterSpecificExport": "Exportation non spécifique au personnage",
    "nonCharacterSpecificExportDesc": "Nécessiter la sélection du personnage lors de l'importation.",
    "deleteChat": "Supprimer la discussion ?",
    "deleteConfirmDesc": "La supprime définitivement de l'historique",
    "keepThisChat": "Garder cette discussion",
    "editCharacter": "Modifier le personnage",
    "exportCharacter": "Exporter le personnage",
    "chatAppearance": "Apparence du chat",
    "importChatPackage": "Importer un paquet de discussion",
    "hideThisCharacter": "Masquer ce personnage",
    "deleteCharacter": "Supprimer le personnage",
    "deleteCharacterTitle": "Supprimer le personnage ?",
    "deleteCharacterConfirmation": "Êtes-vous sûr de vouloir supprimer \"{{name}}\" ? Cela supprimera également toutes les sessions de chat avec ce personnage.",
    "characterSpecificMatches": "Ce paquet est spécifique au personnage et correspond au personnage sélectionné.",
    "characterSpecificMismatch": "Ce paquet est spécifique au personnage et pointe vers un autre personnage. Il sera importé dans {{name}}.",
    "nonCharacterSpecificImport": "Ce paquet n'est pas spécifique au personnage. Il sera importé dans {{name}}.",
    "noCharactersYet": "Pas encore de personnages",
    "createFirstCharacter": "Créez votre premier personnage avec le bouton + ci-dessous pour commencer à discuter",
    "scrollToBottom": "Défiler vers le bas",
    "selectCharacter": "Sélectionner un personnage",
    "branchToGroupChat": "Créer une branche vers une discussion de groupe",
    "addContent": "Ajouter du contenu",
    "swapPlaces": "Échanger les places",
    "swapPlacesOn": "Échanger les places (activé)",
    "uploadImage": "Télécharger une image",
    "helpMeReply": "Aidez-moi à répondre",
    "sceneImage": {
      "modeTitle": "Image de la scène",
      "modeDescription": "Choisissez si vous souhaitez rédiger vous-même l'invite de scène ou laisser l'IA la rédiger en premier.",
      "writePrompt": "Invite d'écriture",
      "writePromptDesc": "Tapez l'invite d'image de scène exacte que vous souhaitez utiliser.",
      "askAi": "Demandez à l'IA",
      "askAiDesc": "Laissez le modèle de discussion actuel rédiger une invite de scène à partir du moment sélectionné.",
      "generateTitle": "Générer une image de scène",
      "regenerateTitle": "Régénérer l'image de la scène",
      "aiTitle": "Invite de scène AI",
      "promptLabel": "INVITE DE SCÈNE",
      "promptPlaceholder": "Décrivez la scène, les personnages, l'ambiance, l'éclairage, le cadrage de la caméra et les détails importants...",
      "suggestedPrompt": "Invite suggérée",
      "regeneratePrompt": "Régénérer",
      "editPrompt": "Modifier l'invite",
      "reviewTitle": "Vérifier le prompt de scène",
      "denyPrompt": "Refuser",
      "acceptPrompt": "Accepter",
      "generateImage": "Générer une image",
      "updateImage": "Mettre à jour l'image"
    },
    "useMyTextAsBase": "Utiliser mon texte comme base",
    "writeNewReply": "Écrire quelque chose de nouveau",
    "suggestedReply": "Réponse suggérée",
    "selectTwoCharactersMinimum": "Sélectionnez au moins 2 personnages pour une discussion de groupe.",
    "groupBranchCreated": "Branche de groupe créée ! Redirection...",
    "memories": "Souvenirs",
    "tools": "Outils",
    "pinned": "Épinglé",
    "searchMemories": "Rechercher des souvenirs...",
    "addMemory": "Ajouter un souvenir",
    "memoryActions": "Actions mémoire",
    "pinnedMessages": "Messages épinglés",
    "pinnedMessagesDesc": "Toujours inclus dans le contexte",
    "contextSummary": "Résumé du contexte",
    "contextSummaryPlaceholder": "Court récapitulatif pour maintenir la cohérence du contexte entre les messages...",
    "memoryContentPlaceholder": "Que faut-il retenir ?",
    "editMemoryPlaceholder": "Saisissez le contenu du souvenir...",
    "togglePin": {
      "pin": "Épingler",
      "unpin": "Désépingler"
    },
    "toggleMemoryState": {
      "setHot": "Activer",
      "setCold": "Archiver"
    },
    "selectModelForRetry": "Sélectionner le modèle pour réessayer",
    "memoryCategories": {
      "characterTrait": "trait de caractère",
      "relationship": "relation",
      "plotEvent": "événement de l'intrigue",
      "worldDetail": "détail du monde",
      "preference": "préférence",
      "other": "autre"
    },
    "settings": {
      "memorySection": "Mémoire",
      "memorySectionDesc": "Résumé, tags, historique des appels d'outils",
      "quickSettings": "Paramètres rapides",
      "quickSettingsDesc": "Ajustements les plus courants",
      "persona": "Persona",
      "model": "Modèle",
      "fallbackModel": "Modèle de secours",
      "voice": "Voix",
      "voiceDesc": "Lecture par synthèse vocale",
      "advanced": "Avancé",
      "advancedDesc": "Remplacer les paramètres du modèle pour cette session",
      "session": "Session",
      "sessionDesc": "Démarrer de nouvelles discussions et parcourir l'historique",
      "newChat": "Nouvelle discussion",
      "newChatDesc": "Démarrer une nouvelle conversation",
      "chatHistoryDesc": "Voir les sessions précédentes",
      "importChatPackageDesc": "Importer un .chatpkg dans ce personnage",
      "selectModel": "Sélectionner un modèle",
      "selectFallbackModel": "Sélectionner un modèle de secours",
      "personaActions": "Actions du persona",
      "sessionAdvancedSettings": "Paramètres avancés de la session",
      "parameterSupport": "Support des paramètres",
      "backToChat": "Retour au chat",
      "chatSettingsTitle": "Paramètres du chat",
      "chatSettingsSubtitle": "Gérer les préférences de conversation",
      "modelDefaults": "Valeurs par défaut du modèle",
      "appDefaults": "Valeurs par défaut de l'app",
      "openChatSessionFirst": "Ouvre d'abord une session de chat",
      "sessionRequired": "Session requise",
      "noPersona": "Aucun persona",
      "customPersona": "Persona personnalisé",
      "noModelAvailable": "Aucun modèle disponible",
      "fallbackNone": "Aucun",
      "unknownModel": "Modèle inconnu",
      "authorNote": "Note de l'auteur",
      "identityProfileAuthored": "Profil d'identité défini",
      "addIdentityProfile": "Ajouter un profil d'identité du compagnon",
      "soulLabel": "Âme",
      "sessionTitle": "Session : {{title}}",
      "sessionUntitled": "Sans titre",
      "messageCount": "{{count}} messages",
      "usingCharacterDefault": "Utilise la valeur par défaut du personnage",
      "sessionOverrideActive": "Surcharge de session active",
      "autoplayVoice": "Lecture auto de la voix",
      "useCharacterDefault": "Utiliser la valeur par défaut du personnage"
    },
    "search": {
      "placeholder": "Rechercher dans la conversation...",
      "noMessagesFound": "Aucun message trouvé",
      "you": "Vous",
      "character": "Personnage",
      "failed": "Échec de la recherche de messages"
    },
    "templates": {
      "startWithTemplate": "Commencer avec un modèle ?",
      "noTemplate": "Pas de modèle",
      "startWithSceneOnly": "Commencer avec la scène uniquement",
      "you": "Vous",
      "bot": "Bot",
      "messageCount": "{{count}} message(s)"
    },
    "header": {
      "back": "Retour",
      "openSettings": "Ouvrir les paramètres du chat",
      "manageMemories": "Gérer les souvenirs",
      "searchMessages": "Rechercher des messages",
      "manageLorebooks": "Gérer les encyclopédies",
      "conversationSettings": "Paramètres de la conversation"
    },
    "footer": {
      "sendMessagePlaceholder": "Envoyer un message...",
      "stopGeneration": "Arrêter la génération",
      "sendMessage": "Envoyer le message",
      "continueConversation": "Continuer la conversation",
      "moreOptions": "Plus d'options",
      "addImage": "Ajouter une image",
      "addImageAttachment": "Ajouter une image en pièce jointe",
      "removeAttachment": "Supprimer la pièce jointe",
      "recordVoice": "Enregistrer la voix"
    },
    "message": {
      "thinkingMessages": {
        "thinkingReallyHard": "Réfléchit intensément…",
        "lettuceCouncil": "Consulte le conseil de la laitue…",
        "stealingThoughts": "Vole des pensées au néant…",
        "warmingBrainCells": "Réchauffe les neurones…",
        "forbiddenKnowledge": "Charge le savoir interdit…",
        "overthinking": "Réfléchit trop (comme d'habitude)…",
        "pretendingToBeSmart": "Fait semblant d'être intelligent…",
        "crunchingNumbers": "Calcule des nombres imaginaires…",
        "arguingWithMyself": "Discute avec moi-même…",
        "askingUniverse": "Demande gentiment à l'univers…"
      },
      "thoughtProcess": "Processus de réflexion",
      "regenerateResponse": "Régénérer la réponse",
      "cancelAudioGeneration": "Annuler la génération audio",
      "stopAudio": "Arrêter l'audio",
      "playMessageAudio": "Lire l'audio du message",
      "playAudio": "Lire l'audio",
      "sceneLabel": "Scène",
      "variantLabel": "Variante",
      "regenerating": "Régénération",
      "assistantIsTyping": "L'assistant écrit",
      "attachedImage": "Image jointe",
      "guidedRegenerationTitle": "Guider la régénération",
      "guidedRegenerationLabel": "Comment doit-elle changer ?",
      "guidedRegenerationDescription": "Décris le ton, la longueur, les détails à garder ou à retirer, et tout ce que la prochaine réponse devrait faire différemment.",
      "guidedRegenerationPlaceholder": "Plus court, plus chaleureux, plus direct...",
      "guidedRegenerationSubmit": "Régénérer"
    },
    "actions": {
      "assistantMessage": "Message de l'assistant",
      "userMessage": "Message de l'utilisateur",
      "promptTokens": "Tokens du prompt",
      "completionTokens": "Tokens de complétion",
      "fallbackModelUsed": "Modèle de secours utilisé",
      "total": "total",
      "timeToFirstToken": "Temps jusqu'au premier token",
      "completionTokenSpeed": "Vitesse des tokens de complétion",
      "edit": "Modifier",
      "copy": "Copier",
      "pin": "Épingler",
      "unpin": "Désépingler",
      "rewindToHere": "Rembobiner jusqu'ici",
      "branchFromHere": "Créer une branche à partir d'ici",
      "branchToGroupChat": "Créer une branche vers une discussion de groupe",
      "branchToCharacter": "Créer une branche vers un personnage",
      "generateSceneImage": "Générer une image de scène",
      "regenerateSceneImage": "Régénérer l'image de la scène",
      "chatAppearance": "Apparence du chat",
      "delete": "Supprimer",
      "debug": "Déboguer",
      "unpinToDelete": "Désépingler pour supprimer",
      "editPlaceholder": "Modifier votre message...",
      "memoriesUsed": "{{count}} souvenirs utilisés",
      "lorebookUsage": "Utilisation de l'encyclopédie",
      "lorebookUsageDesc": "Cette réponse a utilisé les entrées d'encyclopédie suivantes.",
      "matchScore": "Correspondance : {{score}}%",
      "unknownModel": "Modèle inconnu",
      "loadingModel": "Chargement du modèle..."
    },
    "emptyState": {
      "goBack": "Retour"
    },
    "settingsDrawer": {
      "title": "Paramètres du chat",
      "subtitle": "Gérer les préférences de conversation"
    },
    "history": {
      "archivedBadge": "Archivé",
      "messagesCount": "{{count}} messages",
      "previousGroupPage": "Page {{label}} précédente",
      "nextGroupPage": "Page {{label}} suivante",
      "sillyTavernFormat": "Format SillyTavern",
      "sillyTavernFormatDesc": "Exporter en .jsonl compatible avec SillyTavern.",
      "failedLoad": "Échec du chargement des données",
      "failedDelete": "Échec de la suppression : {{error}}",
      "failedRename": "Échec du renommage : {{error}}",
      "chatPackageExportedTo": "Package de chat exporté vers :\n{{path}}",
      "sillyTavernExportedTo": "Chat SillyTavern exporté vers :\n{{path}}",
      "failedExportChatPackage": "Échec de l'export du package de chat",
      "failedExportSillyTavern": "Échec de l'export du chat SillyTavern"
    },
    "authorNote": {
      "title": "Note de l'auteur",
      "inlineEditor": "Éditeur en ligne",
      "inlineEditorDesc": "Affiche la note de l'auteur au-dessus de la zone de saisie pour des modifications rapides.",
      "toggleInlineEditor": "Activer/désactiver l'éditeur en ligne de note d'auteur",
      "placeholder": "Direction privée pour ce chat",
      "willBeRemoved": "La note de l'auteur sera supprimée à l'enregistrement",
      "noNoteSaved": "Aucune note d'auteur enregistrée",
      "charactersCount": "{{count}} caractères",
      "clear": "Effacer",
      "save": "Enregistrer",
      "saving": "Enregistrement...",
      "failedSave": "Échec de l'enregistrement de la note de l'auteur",
      "addPlaceholder": "Ajouter une note de l'auteur...",
      "savingShort": "Enregistrement..."
    },
    "drawer": {
      "chatSettingsTitle": "Paramètres du chat",
      "chatSettingsSubtitle": "Gérer les préférences de conversation"
    },
    "companionSoul": {
      "loading": "Chargement de l'âme du compagnon...",
      "unavailable": "L'âme du compagnon est indisponible",
      "unavailableDesc": "L'édition de l'âme n'est disponible que pour les personnages en mode compagnon.",
      "pageTitle": "Âme du compagnon",
      "back": "Retour",
      "save": "Enregistrer"
    },
    "companionRelationship": {
      "back": "Retour",
      "loading": "Chargement de l'état relationnel...",
      "unavailableTitle": "L'état relationnel est indisponible",
      "sessionLoadFailed": "La session de chat n'a pas pu être chargée.",
      "backToChat": "Retour au chat",
      "notCompanionTitle": "Ce chat n'est pas en mode compagnon",
      "notCompanionDesc": "Les pages de relation du compagnon ne s'affichent que pour les chats dont le mode du personnage est compagnon.",
      "openRegularMemories": "Ouvrir les souvenirs classiques",
      "pageTitle": "État relationnel",
      "memoryButton": "Mémoire",
      "lastInteraction": "Dernière interaction {{time}}",
      "bond": "Lien",
      "closeness": "Proximité",
      "trust": "Confiance",
      "affection": "Affection",
      "tension": "Tension",
      "stability": "Stabilité",
      "interactions": "Interactions",
      "vsDefaults": "vs. valeurs par défaut du personnage",
      "updatedAt": "Mis à jour {{time}}",
      "emotionalEngine": "Moteur émotionnel",
      "felt": "Ressenti",
      "feltDesc": "Affect interne",
      "expressed": "Exprimé",
      "expressedDesc": "Apparaît dans les réponses",
      "blocked": "Bloqué",
      "blockedDesc": "Supprimé par le persona",
      "momentum": "Élan",
      "momentumDesc": "Tendance sur les derniers tours",
      "activeDrivers": "Moteurs actifs",
      "soul": "Âme",
      "essence": "Essence",
      "voice": "Voix",
      "relationalStyle": "Style relationnel",
      "vulnerabilities": "Vulnérabilités",
      "habits": "Habitudes",
      "boundaries": "Limites",
      "eventsCount": "{{count}} événements",
      "recentTimeline": "Chronologie récente",
      "superseded": "remplacé",
      "promptScore": "Prompt {{score}}",
      "persistenceScore": "Persistance {{score}}",
      "noTimeline": "Aucune chronologie pour le moment",
      "noTimelineDesc": "Les souvenirs de relation, jalons et instantanés émotionnels apparaîtront ici à mesure que le compagnon apprend des conversations.",
      "notAuthoredYet": "Pas encore défini.",
      "noSignal": "Aucun signal."
    },
    "companionUi": {
      "relationship": "Relation",
      "milestones": "Jalons",
      "boundaries": "Limites",
      "preferences": "Préférences",
      "profile": "Profil",
      "routines": "Routines",
      "episodes": "Épisodes",
      "emotionalSnapshots": "Instantanés émotionnels",
      "unknownTime": "Inconnu",
      "justNow": "À l'instant",
      "minutesAgo": "il y a {{minutes}} min",
      "hoursAgo": "il y a {{hours}} h",
      "daysAgo": "il y a {{days}} j",
      "notSetYet": "Pas encore défini",
      "missingCharacterId": "characterId manquant",
      "sessionNotFound": "Session introuvable",
      "failedLoadCompanion": "Échec du chargement de la session compagnon"
    },
    "chatPage": {
      "characterNotFound": "Personnage introuvable",
      "characterDoesntExist": "Le personnage que tu cherches n'existe pas."
    },
    "debugPage": {
      "copy": "Copier"
    },
    "companionMemoryPage": {
      "backLabel": "Retour",
      "refineMemoryPlaceholder": "Affiner le souvenir du compagnon stocké",
      "superseded": "remplacé",
      "pinned": "épinglé",
      "cold": "froid"
    }
  },
  "groupChats": {
    "list": {
      "editGroup": "Modifier le groupe",
      "deleteGroup": "Supprimer le groupe",
      "deleteConfirmTitle": "Supprimer la discussion de groupe ?",
      "deleteConfirmMessage": "Êtes-vous sûr de vouloir supprimer \"{{name}}\" ? Cela supprimera également tous les messages de cette discussion de groupe.",
      "noGroupChatsYet": "Pas encore de discussions de groupe",
      "noGroupChatsDesc": "Créez votre première discussion de groupe avec le bouton + ci-dessous pour discuter avec plusieurs personnages à la fois",
      "newChat": "Nouvelle discussion",
      "openChat": "Ouvrir la discussion",
      "chatSettings": "Paramètres de la discussion",
      "sessionCount": "{{count}} discussions"
    },
    "create": {
      "invalidPackage": "Ce paquet n'est pas un paquet de discussion de groupe.",
      "inspectPackageError": "Échec de l'inspection du paquet de discussion de groupe",
      "importPackageError": "Échec de l'importation du paquet de discussion de groupe",
      "importChatpkg": "Importer `.chatpkg`",
      "mapParticipantsTitle": "Associer les participants",
      "selectLocalCharacter": "Sélectionnez le personnage local pour ce participant.",
      "selectCharacterPlaceholder": "Sélectionner un personnage...",
      "importChatPackageTitle": "Importer un paquet de discussion",
      "importChatPackageDesc": "Cela importera le `.chatpkg` sélectionné comme nouvelle session de groupe.",
      "characterSelect": {
        "title": "Créer une discussion de groupe",
        "subtitle": "Sélectionnez les personnages pour votre conversation de groupe",
        "selected": "sélectionné(s)",
        "ready": "Prêt",
        "minRequired": "Min. 2 requis",
        "noCharactersYet": "Pas encore de personnages",
        "noCharactersDesc": "Créez d'abord des personnages pour démarrer une discussion de groupe",
        "continueToSetup": "Continuer vers la configuration du groupe"
      },
      "groupSetup": {
        "title": "Configuration du groupe",
        "subtitle": "Configurez les paramètres de votre discussion de groupe",
        "chatType": "Type de discussion",
        "conversation": "Conversation",
        "casualChat": "Discussion décontractée",
        "roleplay": "Jeu de rôle",
        "withScenes": "Avec des scènes",
        "conversationDesc": "Conversation de groupe décontractée sans scène de départ",
        "roleplayDesc": "Scénario de jeu de rôle avec scène de départ et prompts immersifs",
        "speakerSelection": "Sélection du locuteur",
        "llm": "LLM",
        "aiPicks": "L'IA choisit",
        "heuristic": "Heuristique",
        "scoreBased": "Basé sur un score",
        "roundRobin": "Tour par tour",
        "takeTurns": "À tour de rôle",
        "llmDesc": "Utilise votre modèle par défaut pour choisir qui parle (coûte des tokens)",
        "heuristicDesc": "Utilise l'équilibre de participation et les indices contextuels (gratuit)",
        "roundRobinDesc": "Les personnages parlent à tour de rôle (gratuit)",
        "chatBackground": "Arrière-plan de la discussion",
        "optional": "(Facultatif)",
        "uploadBackground": "Télécharger une image d'arrière-plan",
        "backgroundDesc": "Définir une image d'arrière-plan pour cette discussion de groupe",
        "groupName": "Nom du groupe",
        "removeBackground": "Supprimer l'image d'arrière-plan",
        "groupNameAutoGenerate": "Laisser vide pour générer automatiquement à partir des noms des personnages",
        "continueToScene": "Continuer vers la scène de départ",
        "createGroupChat": "Créer la discussion de groupe"
      },
      "startingScene": {
        "title": "Scène de départ",
        "subtitle": "Définissez le scénario d'ouverture de votre jeu de rôle",
        "sceneSource": "Source de la scène",
        "none": "Aucune",
        "custom": "Personnalisée",
        "fromCharacter": "D'un personnage",
        "noneDesc": "Commencer sans scène prédéfinie",
        "customDesc": "Écrivez votre propre scénario d'ouverture",
        "fromCharacterDesc": "Utiliser une scène d'un de vos personnages",
        "sceneContent": "Contenu de la scène",
        "sceneContentPlaceholder": "Décrivez la scène de départ pour ce jeu de rôle...",
        "sceneReferenceTip": "Astuce : Tapez {{@\" pour référencer les personnages",
        "selectScene": "Sélectionner une scène",
        "sceneLabel": "Scène de",
        "copyToCustom": "Copier vers personnalisé et modifier"
      }
    },
    "history": {
      "title": "Historique des discussions de groupe",
      "subtitle": "Toutes les conversations de groupe",
      "searchPlaceholder": "Rechercher des discussions de groupe...",
      "active": "Actives ({{count}})",
      "archived": "Archivées ({{count}})",
      "noChatsYet": "Pas encore de discussions de groupe",
      "noChatsDesc": "Créez une discussion de groupe pour voir votre historique ici",
      "noMatchingChats": "Aucune discussion correspondante",
      "noMatchingDesc": "Essayez un autre terme de recherche",
      "deleteSessionTitle": "Supprimer la discussion de groupe ?",
      "deleteSessionDesc": "La supprime définitivement de l'historique",
      "deleteSessionButton": "Supprimer la discussion",
      "keepChat": "Garder cette discussion"
    },
    "session": {
      "chatTitlePlaceholder": "Titre de la discussion...",
      "newChat": "Nouvelle discussion",
      "rename": "Renommer",
      "unarchive": "Désarchiver",
      "archive": "Archiver",
      "messageCount": "{{count}} message(s)"
    },
    "memories": {
      "tabMemories": "Souvenirs",
      "tabPinned": "Épinglés",
      "tabActivity": "Activité",
      "processing": "Traitement",
      "contextSummaryTitle": "Résumé du contexte",
      "addContextSummaryPrompt": "Appuyez pour ajouter un résumé du contexte...",
      "savedMemories": "Souvenirs enregistrés",
      "resultsCount": "Résultats ({{count}})",
      "searchPlaceholder": "Rechercher des souvenirs...",
      "addMemory": "Ajouter un souvenir",
      "noMemoriesYet": "Pas encore de souvenirs",
      "noMemoriesDesc": "Appuyez sur le bouton Ajouter ci-dessus pour en créer un",
      "noMatchingMemories": "Aucun souvenir correspondant",
      "noMatchingDesc": "Essayez un autre terme de recherche",
      "sessionNotFound": "Session introuvable",
      "memoryActions": "Actions mémoire",
      "tokens": "tokens",
      "cycle": "Cycle",
      "accessed": "Accédé",
      "cold": "Froid",
      "hot": "Actif",
      "activityLog": "Journal d'activité",
      "events": "événements",
      "run": "Exécuter",
      "processingMemories": "L'IA organise les souvenirs du groupe...",
      "memoryCycleSuccess": "Cycle de mémoire traité avec succès !",
      "memoryActionFailed": "Échec de l'action mémoire",
      "newMemoryUpdates": "Nouvelles mises à jour de la mémoire disponibles",
      "noActivityYet": "Pas encore d'activité",
      "noActivityDesc": "Les appels d'outils apparaissent lorsque l'IA gère les souvenirs en mode dynamique",
      "contextSummaryPlaceholder": "Court récapitulatif pour maintenir la cohérence du contexte entre les messages...",
      "addMemoryTitle": "Ajouter un souvenir",
      "memoryPlaceholder": "Que faut-il retenir ?",
      "saveMemory": "Enregistrer le souvenir",
      "editMemoryTitle": "Modifier le souvenir",
      "editMemoryPlaceholder": "Saisissez le contenu du souvenir...",
      "edit": "Modifier",
      "pin": "Épingler",
      "unpin": "Désépingler",
      "setHot": "Activer",
      "setCold": "Archiver"
    },
    "toolLog": {
      "created": "Créé",
      "deleted": "Supprimé",
      "pinned": "Épinglé",
      "unpinned": "Désépinglé",
      "done": "Terminé",
      "pinnedBadge": "épinglé",
      "softDelete": "suppression douce",
      "memoryCycle": "Cycle de mémoire",
      "failedAt": "Échoué à :",
      "window": "Fenêtre",
      "justNow": "à l'instant",
      "minutesAgo": "il y a {{count}} min",
      "hoursAgo": "il y a {{count}} h",
      "yesterday": "hier",
      "daysAgo": "il y a {{count}} j",
      "revertingTitle": "Annulation...",
      "revertCycleTitle": "Annuler ce cycle",
      "revertedAt": "Annulé {{time}}",
      "failedAtStage": "Échec à : {{stage}}",
      "hideDebug": "Masquer le débogage",
      "debug": "Débogage",
      "windowRange": "Fenêtre {{start}}-{{end}}",
      "actionCreated": "Créé",
      "actionDeleted": "Supprimé",
      "actionPinned": "Épinglé",
      "actionUnpinned": "Désépinglé",
      "actionDone": "Terminé",
      "badgePinned": "épinglé",
      "badgeSoftDelete": "suppression douce",
      "badgeUndone": "annulé",
      "badgeReverted": "rétabli",
      "activityEmptyTitle": "Aucune activité pour le moment",
      "activityEmptyDesc": "Les appels d'outils apparaissent quand l'IA gère les souvenirs en mode dynamique"
    },
    "message": {
      "thinkingHard": "Réfléchit intensément…",
      "thinkingLettuce": "Consulte le conseil de la laitue…",
      "thinkingVoid": "Vole des pensées au néant…",
      "thinkingBrainCells": "Réchauffe les neurones…",
      "thinkingForbidden": "Charge le savoir interdit…",
      "thinkingOverthinking": "Réfléchit trop (comme d'habitude)…",
      "thinkingPretending": "Fait semblant d'être intelligent…",
      "thinkingCrunching": "Calcule des nombres imaginaires…",
      "thinkingArguing": "Discute avec moi-même…",
      "thinkingUniverse": "Demande gentiment à l'univers…",
      "thoughtProcess": "Processus de réflexion",
      "userAlt": "Utilisateur",
      "assistantAlt": "Assistant",
      "regenerateResponse": "Régénérer la réponse",
      "variantLabel": "Variante",
      "regenerating": "Régénération",
      "cancelAudioGeneration": "Annuler la génération audio",
      "stopAudio": "Arrêter l'audio",
      "playMessageAudio": "Lire l'audio du message",
      "playAudio": "Lire l'audio",
      "attachedImage": "Image jointe",
      "assistantIsTyping": "L'assistant écrit",
      "assistantTyping": "L'assistant écrit"
    },
    "header": {
      "back": "Retour",
      "memories": "Souvenirs",
      "settings": "Paramètres",
      "characters": "personnages"
    },
    "footer": {
      "mentionCharacter": "Mentionner un personnage",
      "noCharactersFound": "Aucun personnage trouvé",
      "moreOptions": "Plus d'options",
      "addImage": "Ajouter une image",
      "messagePlaceholder": "Message... (@ pour mentionner)",
      "stopGeneration": "Arrêter la génération",
      "sendMessage": "Envoyer le message",
      "continueConversation": "Continuer la conversation",
      "dismissError": "Ignorer l'erreur",
      "removeAttachment": "Supprimer la pièce jointe",
      "tabToSelect": "Tab pour sélectionner"
    },
    "messageActions": {
      "characterMessage": "Message du personnage",
      "yourMessage": "Votre message",
      "whyCharacterResponded": "Pourquoi ce personnage a répondu",
      "total": "total",
      "edit": "Modifier",
      "copy": "Copier",
      "regenerateWithDifferent": "Régénérer avec un autre personnage",
      "rewindToHere": "Rembobiner jusqu'ici",
      "unpinToDelete": "Désépingler pour supprimer",
      "delete": "Supprimer",
      "editPlaceholder": "Modifier votre message...",
      "chooseCharacterTitle": "Choisir un personnage",
      "selectCharacterForRegeneration": "Sélectionnez quel personnage devrait répondre à la place :"
    },
    "settings": {
      "appDefault": "Par défaut de l'application",
      "selectPersona": "Sélectionner un persona",
      "noPersonas": "Aucun persona disponible",
      "noPersonasDesc": "Créez un persona dans les paramètres pour personnaliser vos conversations.",
      "searchPersonas": "Rechercher des personas...",
      "noPersona": "Pas de persona",
      "noPersonaDesc": "Continuer sans persona",
      "noPersonasFound": "Aucun persona trouvé",
      "noPersonasFoundDesc": "Essayez un autre terme de recherche"
    },
    "groupSettings": {
      "title": "Modifier le groupe",
      "subtitle": "Mettre à jour la configuration par défaut pour les futures sessions",
      "enterGroupName": "Nom du groupe",
      "participant": "participant",
      "participants": "participants",
      "uploading": "Envoi en cours...",
      "changeBackground": "Changer l'arrière-plan",
      "addBackgroundImage": "Ajouter une image d'arrière-plan",
      "removeBackground": "Supprimer l'arrière-plan",
      "persona": "Persona",
      "personaSubtitle": "Persona par défaut pour les nouvelles sessions",
      "personaLabel": "Persona",
      "speakerSelection": "Sélection du locuteur",
      "speakerSubtitle": "Méthode par défaut pour les nouvelles sessions",
      "llm": "LLM",
      "aiPicks": "L'IA choisit",
      "heuristic": "Heuristique",
      "scoreBased": "Basé sur le score",
      "roundRobin": "Tour à tour",
      "takeTurns": "À tour de rôle",
      "llmDesc": "Utilise votre modèle par défaut pour choisir qui parle (coûte des tokens)",
      "heuristicDesc": "Utilise l'équilibre de participation et les indices du contexte (gratuit)",
      "roundRobinDesc": "Les personnages parlent à tour de rôle (gratuit)",
      "memoryMode": "Mode mémoire",
      "memorySubtitle": "Mode mémoire par défaut pour les nouvelles sessions",
      "manual": "Manuel",
      "manualDesc": "Gérez les notes vous-même",
      "dynamic": "Dynamique",
      "dynamicDesc": "Rappel automatique",
      "memoryDynamicInfo": "L'IA crée et récupère automatiquement des souvenirs des conversations",
      "memoryManualInfo": "Vous ajoutez et gérez les notes de mémoire vous-même",
      "characters": "Personnages",
      "participantsActive": "{{total}} participants · {{active}} actifs",
      "add": "Ajouter",
      "muted": "(muet)",
      "mutedByDefault": "Muet par défaut",
      "activeByDefault": "Actif par défaut",
      "unmuteCharacter": "Réactiver le personnage",
      "muteCharacter": "Couper le son du personnage",
      "minTwoRequired": "Minimum 2 personnages requis",
      "removeCharacter": "Supprimer le personnage",
      "groupMinCharacters": "Un groupe nécessite au moins 2 personnages",
      "mutedCharactersNote": "Les personnages en sourdine sont ignorés par la sélection automatique du locuteur, mais peuvent toujours répondre via une `@mention` explicite.",
      "addCharacterTitle": "Ajouter un personnage",
      "allCharactersInGroup": "Tous les personnages sont déjà dans ce groupe.",
      "removeCharacterTitle": "Supprimer le personnage ?",
      "removeCharacterConfirm": "Êtes-vous sûr de vouloir supprimer",
      "removeCharacterFrom": "des paramètres par défaut du groupe ?",
      "removing": "Suppression...",
      "remove": "Supprimer"
    },
    "sessionSettings": {
      "subtitle": "Gérer les préférences du chat de groupe",
      "enterGroupName": "Nom du groupe",
      "participant": "participant",
      "participants": "participants",
      "message": "message",
      "messages": "messages",
      "uploading": "Envoi en cours...",
      "changeBackground": "Changer l'arrière-plan",
      "addBackgroundImage": "Ajouter une image d'arrière-plan",
      "removeBackground": "Supprimer l'arrière-plan",
      "persona": "Persona",
      "personaSubtitle": "Votre identité dans cette conversation",
      "personaLabel": "Persona",
      "speakerSelection": "Sélection du locuteur",
      "speakerSubtitle": "Comment le prochain locuteur est choisi",
      "llm": "LLM",
      "aiPicks": "L'IA choisit",
      "heuristic": "Heuristique",
      "scoreBased": "Basé sur le score",
      "roundRobin": "Tour à tour",
      "takeTurns": "À tour de rôle",
      "llmDesc": "Utilise votre modèle par défaut pour choisir qui parle (coûte des tokens)",
      "heuristicDesc": "Utilise l'équilibre de participation et les indices du contexte (gratuit)",
      "roundRobinDesc": "Les personnages parlent à tour de rôle (gratuit)",
      "characters": "Personnages",
      "participantsActive": "{{total}} participants · {{active}} actifs",
      "add": "Ajouter",
      "muted": "(muet)",
      "unmuteCharacter": "Réactiver le personnage",
      "muteCharacter": "Couper le son du personnage",
      "minTwoRequired": "Minimum 2 personnages requis",
      "removeCharacter": "Supprimer le personnage",
      "groupMinCharacters": "Un chat de groupe nécessite au moins 2 personnages",
      "mutedCharactersNote": "Les personnages en sourdine sont ignorés par la sélection automatique du locuteur, mais peuvent toujours répondre via une `@mention` explicite.",
      "data": "Données",
      "dataSubtitle": "Exporter ou importer des conversations",
      "export": "Exporter",
      "exportDesc": "Enregistrer comme fichier partageable",
      "import": "Importer",
      "importDesc": "Charger une conversation depuis un fichier",
      "conversation": "Conversation",
      "conversationSubtitle": "Dupliquer ou continuer dans un nouveau chat",
      "duplicate": "Dupliquer",
      "duplicateDesc": "Copier ce chat avec ou sans messages",
      "branchTo1on1": "Bifurquer en 1-à-1",
      "branchTo1on1Desc": "Continuer en privé avec un personnage",
      "participation": "Participation",
      "participationSubtitle": "Répartition de la parole entre les personnages",
      "addCharacterTitle": "Ajouter un personnage",
      "allCharactersInGroup": "Tous les personnages sont déjà dans ce groupe.",
      "removeCharacterTitle": "Supprimer le personnage ?",
      "removeCharacterConfirm": "Êtes-vous sûr de vouloir supprimer",
      "removeCharacterFrom": "de ce chat de groupe ?",
      "removing": "Suppression...",
      "remove": "Supprimer",
      "cloneGroupTitle": "Cloner le groupe",
      "withMessages": "Avec messages",
      "withMessagesDesc": "Cloner tout y compris l'historique du chat",
      "withoutMessages": "Sans messages",
      "withoutMessagesDesc": "Cloner uniquement la configuration (personnages, scène de départ)",
      "branchWithCharacterTitle": "Bifurquer avec un personnage",
      "branchWithCharacterDesc": "Sélectionnez un personnage pour continuer en conversation 1-à-1. Tous les messages de ce groupe seront convertis.",
      "continueWith": "Continuer la conversation avec {{name}}",
      "exportChatPackageTitle": "Exporter le paquet de chat",
      "includeCharacterSnapshots": "Inclure les instantanés de personnages",
      "includeCharacterSnapshotsDesc": "Conserver les données des personnages dans le paquet",
      "sessionOnly": "Session uniquement",
      "sessionOnlyDesc": "Exporter uniquement les messages et métadonnées",
      "mapParticipantsTitle": "Associer les participants",
      "selectLocalCharacter": "Sélectionnez le personnage local pour ce participant.",
      "selectCharacterPlaceholder": "Sélectionner un personnage...",
      "continue": "Continuer",
      "importChatPackageTitle": "Importer le paquet de chat",
      "importChatPackageDesc": "Ceci importera le `.chatpkg` sélectionné comme nouvelle session de groupe.",
      "importing": "Importation..."
    },
    "page": {
      "selectingCharacter": "Sélection du personnage...",
      "sessionNotFound": "Session de groupe introuvable",
      "backToGroupChats": "Retour aux chats de groupe",
      "startConversation": "Démarrer une conversation avec {{names}}",
      "scrollToBottom": "Faire défiler vers le bas",
      "addContent": "Ajouter du contenu",
      "uploadImage": "Téléverser une image",
      "helpMeReply": "Aidez-moi à répondre",
      "helpMeReplyDesc": "Laisser l'IA proposer une réponse",
      "haveDraftPrompt": "Tu as un brouillon de message. Comment veux-tu continuer ?",
      "useMyTextAsBase": "Utiliser mon texte comme base",
      "useMyTextAsBaseDesc": "Développer et améliorer ton brouillon",
      "writeSomethingNew": "Écrire quelque chose de nouveau",
      "writeSomethingNewDesc": "Générer une nouvelle réponse",
      "suggestedReply": "Réponse suggérée",
      "reasoningBeforeWriting": "Réflexion avant d'écrire ta réponse...",
      "writingYourReply": "Rédaction de ta réponse...",
      "regenerate": "Régénérer",
      "useReply": "Utiliser la réponse",
      "helpMeReplyFailedGeneric": "Aidez-moi à répondre a échoué.",
      "helpMeReplyFailedGenerate": "Aidez-moi à répondre n'a pas pu générer de réponse.",
      "noAudioCaptured": "Aucun audio capturé.",
      "noWhisperModel": "Aucun modèle Whisper installé. Installes-en un dans les paramètres de Reconnaissance vocale.",
      "cancelRecording": "Annuler l'enregistrement",
      "transcribing": "Transcription",
      "stopAndTranscribe": "Arrêter et transcrire",
      "recordVoice": "Enregistrer la voix",
      "learnCorrection": "Apprendre la correction :",
      "learning": "Apprentissage...",
      "learn": "Apprendre",
      "ignore": "Ignorer",
      "groupChatFailed": "Le chat de groupe a échoué.",
      "failedToCopy": "Échec de la copie",
      "copied": "Copié !",
      "cannotDeletePinned": "Impossible de supprimer un message épinglé. Désépingle-le d'abord.",
      "failedToDelete": "Échec de la suppression",
      "messageNotFound": "Message introuvable",
      "cannotRewindPinned": "Impossible de revenir en arrière : il y a des messages épinglés après ce point. Désépingle-les d'abord.",
      "failedToRewind": "Échec du retour en arrière",
      "failedToTogglePin": "Échec de l'épinglage",
      "messagePinned": "Message épinglé",
      "messageUnpinned": "Message désépinglé",
      "failedToSave": "Échec de l'enregistrement"
    },
    "memoriesPage": {
      "summarizingConversation": "Résumé de la conversation",
      "analyzingMemories": "Analyse des souvenirs",
      "applyingChanges": "Application des modifications",
      "organizingMemories": "Organisation des souvenirs",
      "retryingMemoryCycle": "Nouvelle tentative du cycle de mémoire...",
      "processingMemories": "Traitement des souvenirs...",
      "memorySystemError": "Erreur du système de mémoire",
      "contextSummary": "Résumé du contexte",
      "contextSummaryTitle": "Résumé du contexte",
      "savedMemories": "Souvenirs enregistrés",
      "resultsCount": "Résultats ({{count}})",
      "searchMemoriesPlaceholder": "Rechercher dans les souvenirs...",
      "addMemory": "Ajouter un souvenir",
      "memoryActions": "Actions sur le souvenir",
      "clearSearch": "Effacer la recherche",
      "noMatchingMemories": "Aucun souvenir correspondant",
      "noMemoriesYet": "Aucun souvenir pour le moment",
      "tryDifferentSearch": "Essaie un autre terme de recherche",
      "tapAddToCreate": "Touche le bouton Ajouter ci-dessus pour en créer un",
      "pinnedMessages": "Messages épinglés",
      "refresh": "Actualiser",
      "noPinnedMessages": "Aucun message épinglé",
      "pinImportantDesc": "Épingle les messages importants du chat de groupe pour les garder toujours en contexte.",
      "assistant": "Assistant",
      "user": "Utilisateur",
      "unpin": "Désépingler",
      "failedToUnpinMessage": "Échec du désépinglage du message",
      "activityLog": "Journal d'activité",
      "run": "Exécution",
      "addMemoryTitle": "Ajouter un souvenir",
      "editMemoryTitle": "Modifier le souvenir",
      "memoryTitle": "Souvenir",
      "memoryPlaceholder": "Que faut-il retenir ?",
      "saveMemory": "Enregistrer le souvenir",
      "editMemoryPlaceholder": "Saisis le contenu du souvenir...",
      "saving": "Enregistrement...",
      "save": "Enregistrer",
      "cancel": "Annuler",
      "contextSummaryPlaceholder": "Bref récapitulatif pour garder le contexte cohérent entre les messages...",
      "contextSummaryPrompt": "Touche pour ajouter un résumé de contexte...",
      "cycle": "Cycle",
      "accessed": "Consulté",
      "cold": "Froid",
      "hot": "Chaud",
      "tokens": "tokens",
      "pin": "Épingler",
      "setHot": "Définir chaud",
      "setCold": "Définir froid",
      "edit": "Modifier",
      "delete": "Supprimer",
      "failedToToggleMemPin": "Échec de l'épinglage",
      "failedToRemoveMemory": "Échec de la suppression du souvenir",
      "toolEventsCountAria": "événements",
      "activityEmptyDesc": "Les appels d'outils apparaissent quand l'IA gère les souvenirs en mode dynamique",
      "memoryCycleSuccess": "Cycle de mémoire traité avec succès !"
    },
    "messageActionsExtra": {
      "characterMessage": "Message du personnage",
      "yourMessage": "Ton message",
      "whyCharacterResponded": "Pourquoi ce personnage a répondu",
      "promptTokensTitle": "Tokens du prompt",
      "completionTokensTitle": "Tokens de complétion",
      "total": "total",
      "regenerateWithDifferent": "Régénérer avec un autre personnage",
      "unpin": "Désépingler",
      "pin": "Épingler",
      "rewindToHere": "Revenir ici",
      "unpinToDelete": "Désépingler pour supprimer",
      "editPlaceholder": "Modifier ton message...",
      "chooseCharacter": "Choisir un personnage",
      "selectCharacterForRegeneration": "Sélectionne quel personnage devrait répondre à la place :"
    },
    "timeAgo": {
      "justNow": "À l'instant",
      "minutesAgo": "il y a {{count}} min",
      "hoursAgo": "il y a {{count}} h",
      "daysAgo": "il y a {{count}} j"
    },
    "memoriesController": {
      "missingSessionId": "sessionId manquant",
      "sessionNotFound": "Session introuvable",
      "failedToLoadSession": "Échec du chargement de la session",
      "failedToUpdateTemperature": "Échec de la mise à jour de la température du souvenir",
      "failedToSaveSummary": "Échec de l'enregistrement du résumé",
      "cycleFailed": "Le cycle de mémoire de groupe a échoué",
      "failedToAddMemory": "Échec de l'ajout du souvenir",
      "failedToUpdateMemory": "Échec de la mise à jour du souvenir",
      "failedToRunCycle": "Échec de l'exécution du cycle de mémoire",
      "failedToRevertCycle": "Échec de l'annulation du cycle",
      "failedToRefresh": "Échec de l'actualisation",
      "failedToDismissError": "Échec de l'ignorance de l'erreur",
      "failedToTogglePinned": "Échec de l'épinglage du message",
      "sessionNotLoaded": "Session non chargée",
      "revertCycleTitle": "Annuler ce cycle ?",
      "revertConfirm": "Annuler"
    },
    "chatController": {
      "sessionNotFound": "Session de groupe introuvable",
      "failedToLoadGroupChat": "Échec du chargement du chat de groupe",
      "requestFailed": "La requête de chat de groupe a échoué",
      "failedToSendMessage": "Échec de l'envoi du message",
      "failedToRegenerate": "Échec de la régénération",
      "failedToContinue": "Échec de la continuation",
      "failedToCopy": "Échec de la copie",
      "cannotDeletePinned": "Impossible de supprimer un message épinglé. Désépingle-le d'abord.",
      "failedToDelete": "Échec de la suppression",
      "messageNotFound": "Message introuvable",
      "cannotRewindPinned": "Impossible de revenir en arrière : il y a des messages épinglés après ce point. Désépingle-les d'abord.",
      "failedToRewind": "Échec du retour en arrière",
      "failedToTogglePin": "Échec de l'épinglage",
      "messagePinned": "Message épinglé",
      "messageUnpinned": "Message désépinglé",
      "failedToSave": "Échec de l'enregistrement",
      "copied": "Copié !"
    },
    "historyController": {
      "failedToLoadData": "Échec du chargement des données",
      "failedToDelete": "Échec de la suppression : {{error}}",
      "failedToRename": "Échec du renommage : {{error}}",
      "failedToArchive": "Échec de l'archivage : {{error}}",
      "failedToUnarchive": "Échec du désarchivage : {{error}}",
      "failedToDuplicate": "Échec de la duplication"
    },
    "sessionSettingsController": {
      "failedToLoad": "Échec du chargement des paramètres du chat de groupe",
      "noPersona": "Aucun persona",
      "customPersona": "Persona personnalisé",
      "minOneActive": "Au moins un participant doit rester actif"
    },
    "groupSettingsController": {
      "notFound": "Groupe introuvable",
      "failedToLoad": "Échec du chargement des paramètres du groupe"
    },
    "createForm": {
      "failedToLoadCharacters": "Échec du chargement des personnages",
      "selectAtLeastTwo": "Sélectionne au moins 2 personnages pour un chat de groupe",
      "failedToCreate": "Échec de la création du chat de groupe"
    },
    "groupSetupExtra": {
      "memoryMode": "Mode mémoire",
      "manual": "Manuel",
      "manualDesc": "Gère les notes toi-même",
      "dynamic": "Dynamique",
      "dynamicDesc": "Résumés et rappel automatiques",
      "memoryManualInfo": "Tu ajoutes et gères toi-même les notes de mémoire",
      "memoryDynamicInfo": "L'IA crée et récupère automatiquement les souvenirs des conversations",
      "backgroundPreviewAlt": "Aperçu de l'arrière-plan"
    },
    "createExtra": {
      "enterGroupNamePlaceholder": "Saisis le nom du groupe...",
      "unknown": "Inconnu"
    },
    "startingSceneExtra": {
      "insertAs": "Insérer en tant que {{snippet}}"
    },
    "sessionCardExtra": {
      "unknown": "Inconnu",
      "untitledChat": "Chat sans titre"
    },
    "sessionListExtra": {
      "unknown": "Inconnu"
    },
    "chatHeaderExtra": {
      "unknownError": "Erreur inconnue"
    },
    "chatSettingsExtra": {
      "invalidPackage": "Ce package n'est pas un package de chat de groupe.",
      "failedExport": "Échec de l'export du package de chat de groupe",
      "failedInspect": "Échec de l'inspection du package de chat de groupe",
      "failedImport": "Échec de l'import du package de chat de groupe",
      "exportSuccess": "Package de chat de groupe exporté vers :\n{{path}}",
      "backgroundAlt": "Arrière-plan",
      "removeBackgroundAria": "Supprimer l'arrière-plan",
      "backAria": "Retour",
      "backToGroupChats": "Retour aux chats de groupe"
    },
    "groupSettingsExtra": {
      "backToGroups": "Retour aux groupes",
      "backAria": "Retour",
      "removeBackgroundAria": "Supprimer l'arrière-plan"
    },
    "historyPage": {
      "backAria": "Retour aux chats de groupe",
      "clearSearchAria": "Effacer la recherche",
      "resultsLabel": "{{count}} résultat",
      "resultsLabelPlural": "{{count}} résultats",
      "untitledChat": "Chat sans titre",
      "noPinnedMessages": "Aucun message épinglé"
    },
    "personaSelectorExtra": {
      "insertAs": "Insérer en tant que",
      "appDefault": "Par défaut de l'app",
      "defaultBadge": "Par défaut",
      "selectPersonaTitle": "Sélectionner un persona",
      "noPersonaTitle": "Aucun persona",
      "noPersonaDesc": "Continuer sans persona",
      "noPersonasAvailable": "Aucun persona disponible",
      "noPersonasDesc": "Crée un persona dans les paramètres pour personnaliser tes conversations.",
      "searchPersonas": "Rechercher des personas...",
      "noPersonasFound": "Aucun persona trouvé",
      "tryDifferentSearch": "Essaie un autre terme de recherche"
    },
    "footerExtra": {
      "cancelRecording": "Annuler l'enregistrement",
      "transcribing": "Transcription",
      "stopAndTranscribe": "Arrêter et transcrire",
      "recordVoice": "Enregistrer la voix",
      "attachmentAltDefault": "Pièce jointe"
    },
    "pageExtra": {
      "noAudioCaptured": "Aucun audio capturé.",
      "noWhisperModelInstalled": "Aucun modèle Whisper installé. Installes-en un dans les paramètres de Reconnaissance vocale.",
      "scrollToBottomAria": "Faire défiler vers le bas"
    },
    "addContentMenu": {
      "title": "Ajouter du contenu",
      "uploadImage": "Téléverser une image"
    },
    "helpMeReplyMenu": {
      "title": "Aidez-moi à répondre",
      "helpMeReplyDesc": "Laisser l'IA proposer une réponse",
      "draftPrompt": "Tu as un brouillon de message. Comment veux-tu continuer ?",
      "useTextAsBase": "Utiliser mon texte comme base",
      "useTextAsBaseDesc": "Développer et améliorer ton brouillon",
      "writeSomethingNew": "Écrire quelque chose de nouveau",
      "writeSomethingNewDesc": "Générer une nouvelle réponse"
    },
    "suggestedReplyMenu": {
      "title": "Réponse suggérée",
      "reasoningBeforeWriting": "Réflexion avant d'écrire ta réponse...",
      "writingYourReply": "Rédaction de ta réponse...",
      "regenerate": "Régénérer",
      "useReply": "Utiliser la réponse"
    },
    "memoriesPageExtra": {
      "sessionNotFound": "Session introuvable",
      "memorySystemError": "Erreur du système de mémoire",
      "retryingMemoryCycle": "Nouvelle tentative du cycle de mémoire...",
      "processingMemories": "Traitement des souvenirs...",
      "memoryCycleSuccess": "Cycle de mémoire traité avec succès !",
      "contextSummaryTitle": "Résumé du contexte",
      "activityTabLabel": "Activité",
      "noMatchingMemoriesDesc": "Essaie un autre terme de recherche",
      "chatHistoryTitle": "Historique des chats",
      "chatHistoryDesc": "Voir et gérer les conversations"
    },
    "settingsPageExtra": {
      "quickActionsSection": "Actions rapides",
      "chatHistoryTitle": "Historique des chats",
      "chatHistoryDesc": "Voir et gérer les conversations",
      "lorebrooksTitle": "Lorebooks",
      "lorebrooksDesc": "Attache des lorebooks à la session et ignore éventuellement les lorebooks de chaque personnage.",
      "manageLorebooks": "Gérer les lorebooks"
    },
    "groupSettingsPageExtra": {
      "lorebrooksTitle": "Lorebooks",
      "lorebrooksDesc": "Attache des lorebooks partagés et contrôle si les lorebooks des personnages s'appliquent par défaut.",
      "manageLorebooks": "Gérer les lorebooks"
    }
  },
  "characters": {
    "empty": {
      "title": "Pas encore de personnages",
      "description": "Créez des personnages IA personnalisés avec des personnalités uniques",
      "createButton": "Créer un personnage"
    },
    "progress": {
      "identity": "Identité",
      "scenes": "Scènes",
      "details": "Détails"
    },
    "identity": {
      "title": "Créer un personnage",
      "subtitle": "Donnez une identité à votre personnage IA",
      "tapCameraToAdd": "Appuyez sur la caméra pour ajouter ou générer un avatar",
      "importingAvatar": "Importation de l'avatar...",
      "characterName": "Nom du personnage *",
      "characterNamePlaceholder": "Entrez le nom du personnage...",
      "characterNameDesc": "Ce nom apparaîtra dans les conversations",
      "avatarGradient": "Dégradé de l'avatar",
      "avatarGradientDesc": "Générer des dégradés dynamiques à partir des couleurs de l'avatar",
      "chatBackgroundLabel": "Arrière-plan du chat",
      "optionalSuffix": "(Optionnel)",
      "backgroundPreviewAlt": "Aperçu de l'arrière-plan",
      "backgroundPreviewBadge": "Aperçu de l'arrière-plan",
      "addBackground": "Ajouter un arrière-plan",
      "addBackgroundHint": "Téléverse-en un ou choisis dans la bibliothèque",
      "uploadImage": "Téléverser une image",
      "chooseFromLibrary": "Choisir dans la bibliothèque",
      "backgroundDesc": "Image d'arrière-plan optionnelle pour les conversations",
      "continueToDescription": "Continuer vers la description",
      "importCharacterFromFile": "Importer un personnage depuis un fichier",
      "importCharacterDesc": "Charge un personnage depuis une carte PNG, un .uec ou un fichier .json exporté"
    },
    "details": {
      "title": "Détails du personnage",
      "subtitle": "Définir la personnalité et le comportement",
      "definition": "Définition *",
      "wordCount": "{{count}} mot(s)",
      "definitionPlaceholder": "Décrivez la personnalité, le style de parole, le contexte, les domaines de connaissance...",
      "definitionDesc": "Soyez précis sur le ton, les traits et le style de conversation",
      "availablePlaceholders": "Variables disponibles :"
    },
    "scenes": {
      "title": "Scènes de départ",
      "subtitle": "Créez des scénarios d'ouverture pour vos conversations",
      "default": "Par défaut",
      "hasSceneDirection": "Contient une direction de scène",
      "continueToScenes": "Continuer vers les scènes de départ"
    },
    "extras": {
      "title": "Détails supplémentaires",
      "subtitle": "Tous les champs sont facultatifs",
      "nickname": "Surnom",
      "nicknamePlaceholder": "Comment l'utilisateur devrait-il appeler ce personnage ?",
      "nicknameDesc": "Nom d'affichage alternatif utilisé dans les conversations",
      "creator": "Créateur",
      "creatorPlaceholder": "Nom du créateur...",
      "tags": "Tags",
      "tagsPlaceholder": "fantaisie, sci-fi, romance...",
      "tagsDesc": "Liste séparée par des virgules pour le filtrage et l'organisation",
      "creatorNotes": "Notes du créateur",
      "creatorNotesPlaceholder": "Conseils d'utilisation, contexte ou instructions pour les autres utilisateurs...",
      "multilingualNotes": "Notes multilingues",
      "multilingualNotesHint": "Objet JSON avec les codes de langue comme clés",
      "creatingCharacter": "Création du personnage..."
    },
    "preview": {
      "unnamedCharacter": "Personnage sans nom",
      "sceneCount": "{{count}} scène(s)",
      "customPrompt": "Prompt personnalisé",
      "description": "Description",
      "startingScene": "Scène de départ",
      "gradientEnabled": "Dégradé activé",
      "customModel": "Modèle personnalisé",
      "avatarAlt": "Avatar du personnage",
      "characterFallback": "Personnage"
    },
    "personaPreview": {
      "unnamedPersona": "Persona sans nom",
      "noDescription": "Pas encore de description",
      "default": "Par défaut",
      "description": "Description"
    },
    "lorebookPreview": {
      "untitledLorebook": "Encyclopédie sans titre",
      "entryCount": "{{count}} entrée(s)",
      "entries": "Entrées",
      "noEntriesYet": "Pas encore d'entrées",
      "untitledEntry": "Entrée sans titre",
      "noContentYet": "Pas encore de contenu",
      "alwaysActive": "Toujours actif",
      "moreEntries": "+{{count}} autres entrées",
      "moreEntry": "+{{count}} autre entrée"
    },
    "creationHelper": {
      "moreOptions": "Plus d'options",
      "describePlaceholder": "Décrivez votre personnage...",
      "stopGeneration": "Arrêter la génération",
      "sendMessage": "Envoyer le message",
      "addToMessage": "Ajouter au message",
      "uploadImageDesc": "Ajouter un avatar ou une image de référence",
      "referenceCharacterDesc": "Utiliser un personnage existant comme inspiration",
      "referencePersonaDesc": "Utiliser votre persona comme contexte",
      "retry": "Réessayer",
      "attachmentAlt": "Pièce jointe",
      "removeAttachment": "Supprimer la pièce jointe",
      "removeReference": "Supprimer la référence",
      "uploadImageTitle": "Téléverser une image",
      "referenceCharacterTitle": "Personnage de référence",
      "referencePersonaTitle": "Persona de référence"
    },
    "lorebook": {
      "keywords": "MOTS-CLÉS",
      "caseSensitive": "Sensible à la casse",
      "typeKeyword": "Tapez un mot-clé...",
      "addButton": "Ajouter",
      "untitledEntry": "Entrée sans titre",
      "alwaysActive": "Toujours actif",
      "noKeywords": "Pas de mots-clés",
      "dragToReorder": "Glisser pour réorganiser",
      "disabled": "Désactivé",
      "noLorebooksYet": "Pas encore d'encyclopédies",
      "createLorebookDesc": "Créez une encyclopédie pour ajouter de l'univers à ce personnage",
      "createLorebook": "Créer une encyclopédie",
      "searchPlaceholder": "Rechercher des encyclopédies...",
      "noMatchingLorebooks": "Aucune encyclopédie correspondante trouvée",
      "activeLorebooks": "Encyclopédies actives",
      "enabledForCharacter": "activée pour ce personnage",
      "enabledForGroup": "activée pour ce groupe",
      "enabledForSession": "activée pour cette session",
      "tapToViewEntries": "Appuyez pour voir les entrées",
      "newLorebookTitle": "Nouvelle encyclopédie",
      "nameLabel": "NOM",
      "enterNamePlaceholder": "Entrez le nom de l'encyclopédie...",
      "lorebookExplanation": "Les encyclopédies contiennent des entrées qui sont injectées dans les prompts lorsque les mots-clés correspondent.",
      "viewEntries": "Voir les entrées",
      "editEntriesDesc": "Modifier les entrées de l'encyclopédie",
      "disableForCharacter": "Désactiver pour le personnage",
      "enableForCharacter": "Activer pour le personnage",
      "disableForGroup": "Désactiver pour le groupe",
      "enableForGroup": "Activer pour le groupe",
      "disableForSession": "Désactiver pour la session",
      "enableForSession": "Activer pour la session",
      "removeFromActive": "Retirer des encyclopédies actives de ce personnage",
      "addToActive": "Ajouter aux encyclopédies actives de ce personnage",
      "removeFromActiveGroup": "Retirer des encyclopédies actives de ce groupe",
      "addToActiveGroup": "Ajouter aux encyclopédies actives de ce groupe",
      "removeFromActiveSession": "Retirer des encyclopédies actives de cette session",
      "addToActiveSession": "Ajouter aux encyclopédies actives de cette session",
      "deleteConfirmTitle": "Supprimer l'encyclopédie ?",
      "deleteConfirmMessage": "Supprimer cette encyclopédie ? Toutes les entrées seront perdues.",
      "deleteLorebook": "Supprimer l'encyclopédie",
      "noEntriesYet": "Pas encore d'entrées",
      "addEntriesToInject": "Ajoutez des entrées pour injecter de l'univers dans vos discussions",
      "createEntry": "Créer une entrée",
      "searchEntries": "Rechercher des entrées...",
      "noMatchingEntries": "Aucune entrée correspondante trouvée",
      "entryDefaultName": "Entrée",
      "editEntry": "Modifier l'entrée",
      "editEntryDesc": "Modifier le titre, les mots-clés et le contenu",
      "disableEntry": "Désactiver l'entrée",
      "enableEntry": "Activer l'entrée",
      "entryDisabledDesc": "L'entrée ne sera pas injectée dans les prompts",
      "entryEnabledDesc": "L'entrée sera injectée lorsque les mots-clés correspondent",
      "deleteEntry": "Supprimer l'entrée",
      "titleLabel": "TITRE",
      "titlePlaceholder": "Nommez cette entrée...",
      "enabled": "Activé",
      "includeInPrompts": "Inclure dans les prompts",
      "alwaysOn": "Toujours actif",
      "noKeywordsNeeded": "Pas de mots-clés nécessaires",
      "contentLabel": "CONTENU",
      "contentPlaceholder": "Écrivez le contexte ici...",
      "saveEntry": "Enregistrer l'entrée",
      "noCharacterId": "Aucun ID de personnage fourni",
      "sectionActive": "Actif",
      "sectionAvailable": "Disponible",
      "entryCount": "{{count}} entrées",
      "keywordDetectionMode": "DÉTECTION DE MOTS-CLÉS",
      "keywordDetectionRecentWindow": "10 derniers messages",
      "keywordDetectionRecentWindowDesc": "Correspond à la fenêtre de conversation des 10 derniers messages.",
      "keywordDetectionLatestUser": "Dernier message utilisateur uniquement",
      "keywordDetectionLatestUserDesc": "Correspond uniquement au dernier message envoyé par l'utilisateur.",
      "preview": {
        "title": "Aperçu des déclencheurs",
        "openButton": "Aperçu",
        "missingContext": "Aucun lorebook sélectionné.",
        "noEntries": "Ajoute des entrées à ce lorebook pour voir ce qui se déclencherait.",
        "modeRecentShort": "Récents {{count}}",
        "modeLatestUserShort": "Dernier utilisateur",
        "inWindow": "{{count}} dans la fenêtre",
        "tabSession": "Session",
        "tabCompose": "Composer",
        "activeStat": "{{active}} / {{total}} actifs",
        "tokensStat": "{{count}} tokens",
        "sessionPickerLabel": "Sessions",
        "sessionMeta": "{{count}} msg",
        "noSessions": "Aucune session de chat pour le moment.",
        "noSessionsHint": "Choisis une session",
        "noMessages": "Cette session n'a pas encore de messages.",
        "scanHeaderRecent": "Analyse de {{shown}} sur les {{depth}} derniers messages",
        "scanHeaderLatest": "Analyse du dernier message utilisateur",
        "matchCount": "{{hits}} hit · {{entries}} entrées",
        "emptyMessage": "(vide)",
        "roleUser": "Utilisateur",
        "roleAssistant": "Assistant",
        "roleScene": "Scène",
        "roleSystem": "Système",
        "composeHeader": "Bloc-notes",
        "composeMatches": "{{count}} correspondances",
        "activeLabel": "{{count}} actifs",
        "composePlaceholder": "Tape ou colle du texte pour tester la correspondance des mots-clés...\n\nex.\nLa bibliothèque était silencieuse, juste le ronron des vieux radiateurs.\nElle m'a demandé si j'avais lu le livre qu'elle m'a prêté la semaine dernière.",
        "sectionActive": "Actifs · {{count}}",
        "sectionInactive": "Inactifs · {{count}}",
        "statusMatched": "Trouvé",
        "statusAlways": "Toujours",
        "statusDisabled": "Désactivé",
        "statusNoKeywords": "Aucune clé",
        "statusNotMatched": "Aucune correspondance",
        "tokensShort": "{{count}}t",
        "injectionTitle": "Injection finale",
        "injectionEmpty": "Aucune entrée n'est active : rien ne serait injecté.",
        "copy": "Copier",
        "copyFailed": "Échec de la copie dans le presse-papiers.",
        "saveFailed": "Échec de l'enregistrement de l'entrée.",
        "deleteFailed": "Échec de la suppression de l'entrée.",
        "deleteConfirmTitle": "Tu es sûr ?",
        "deleteConfirmMessage": "Supprimer \"{{title}}\" ? Cette action est irréversible.",
        "contextMenuTitle": "{{count}} entrées l'utilisent"
      }
    },
    "templates": {
      "characterNotFound": "Personnage introuvable",
      "templateCount": "{{count}} modèle(s)",
      "newTemplate": "Nouveau modèle",
      "noTemplatesYet": "Pas encore de modèles",
      "explanation": "Les modèles de discussion vous permettent de commencer des conversations avec des messages pré-écrits de votre part et de {{name}}.",
      "createTemplate": "Créer un modèle",
      "messageCount": "{{count}} message(s)",
      "deleteTemplate": "Supprimer le modèle",
      "contextMenuFallbackTitle": "Modèle",
      "importedToast": {
        "title": "Importé",
        "message": "\"{{name}}\" ajouté."
      },
      "importFailed": "Échec de l'import",
      "exportFailed": "Échec de l'export"
    },
    "templateEditor": {
      "noScene": "Pas de scène",
      "untitled": "Sans titre",
      "dragMessage": "Glisser le message",
      "editMessage": "Modifier le message",
      "deleteMessage": "Supprimer le message",
      "writeMessagePlaceholder": "Écrivez le contenu du message...",
      "characterNotFound": "Personnage introuvable",
      "scene": "Scène",
      "noMessagesYet": "Pas encore de messages",
      "addMessagesDesc": "Ajoutez des messages pour construire un démarreur de conversation avec {{name}}.",
      "addMessage": "Ajouter un message",
      "name": "Nom",
      "nameExample": "ex. Salutation décontractée",
      "startingScene": "Scène de départ",
      "systemPrompt": "Prompt système",
      "characterDefault": "Par défaut du personnage",
      "nextMessageAs": "Prochain message en tant que",
      "messages": "Messages",
      "roles": "Rôles",
      "hoverTip": "Survolez les messages pour les glisser, modifier ou supprimer.",
      "footerTip": "Utilisez la barre en bas pour ajouter de nouveaux messages à la conversation.",
      "templateNamePlaceholder": "Nom du modèle...",
      "selectScene": "Sélectionner une scène",
      "startWithoutScene": "Commencer sans message de scène",
      "selectSystemPrompt": "Sélectionner un prompt système",
      "useCharacterSystemPrompt": "Utiliser le prompt système du personnage"
    },
    "referenceSelector": {
      "selectCharacter": "Sélectionner un personnage",
      "selectPersona": "Sélectionner un persona",
      "searchPlaceholder": "Rechercher des {{type}}s...",
      "loading": "Chargement...",
      "noMatch": "Aucun {{type}} ne correspond à ta recherche",
      "noAvailable": "Aucun {{type}} disponible"
    },
    "voiceLoading": {
      "failed": "Échec du chargement des voix"
    },
    "activeLorebooks": {
      "sectionTitle": "Lorebooks actifs",
      "selectedSummary": "{{count}} actifs",
      "untitledLorebook": "Lorebook sans titre",
      "usingNone": "N'utilise aucun lorebook de personnage",
      "loading": "Chargement des lorebooks...",
      "loadFailed": "Échec du chargement des lorebooks",
      "inheritHint": "Les sessions en héritent sauf si elles les surchargent.",
      "clear": "Effacer",
      "chooseHint": "Choisis les lorebooks que ce personnage doit activer par défaut. Les sessions existantes peuvent toujours surcharger cette liste.",
      "emptyState": "Aucun lorebook pour le moment. Crée d'abord des lorebooks depuis le gestionnaire de lorebooks."
    },
    "description": {
      "descriptionLabel": "Description",
      "descriptionPlaceholder": "Court résumé affiché sur les cartes et les listes...",
      "descriptionHint": "Description courte optionnelle pour l'interface ; la définition complète est utilisée dans les prompts.",
      "companionPromptLabel": "Prompt compagnon (Optionnel)",
      "systemPromptLabel": "Prompt système (Optionnel)",
      "loadingTemplates": "Chargement des modèles...",
      "useAppCompanionDefault": "Utiliser le compagnon par défaut de l'app",
      "useAppDefault": "Utiliser la valeur par défaut de l'app",
      "companionPromptHint": "Stocké séparément en tant que prompt compagnon. Le prompt système de roleplay normal n'est pas modifié.",
      "systemPromptHint": "Choisis un prompt système personnalisé ou utilise celui par défaut.",
      "groupChatConvLabel": "Prompt de chat de groupe (Conversation)",
      "groupChatConvHint": "Surcharge le prompt de conversation de ce personnage dans les chats de groupe",
      "groupChatRpLabel": "Prompt de chat de groupe (Roleplay)",
      "groupChatRpHint": "Surcharge le prompt de roleplay de ce personnage dans les chats de groupe",
      "voiceLabel": "Voix (Optionnel)",
      "loadingVoices": "Chargement des voix...",
      "customVoiceFallback": "Voix personnalisée",
      "providerVoiceFallback": "Voix du fournisseur",
      "selectedVoiceFallback": "Voix sélectionnée",
      "noVoiceAssigned": "Aucune voix attribuée",
      "addVoicesHint": "Ajoute des voix dans Paramètres → Voix",
      "voiceAssignHint": "Attribue une voix pour la lecture text-to-speech future",
      "autoplayLabel": "Lecture auto de la voix",
      "autoplayOn": "Lire automatiquement les réponses de ce personnage",
      "autoplayOff": "Sélectionne d'abord une voix",
      "aiModelLabel": "Modèle IA *",
      "loadingModels": "Chargement des modèles...",
      "selectedModelFallback": "Modèle sélectionné",
      "selectModelPlaceholder": "Sélectionne un modèle",
      "noModelsConfigured": "Aucun modèle configuré",
      "noModelsHint": "Ajoute d'abord un fournisseur dans les paramètres pour continuer",
      "aiModelHint": "Ce modèle alimentera les réponses du personnage",
      "fallbackModelLabel": "Modèle de secours (Optionnel)",
      "selectedFallbackFallback": "Modèle de secours sélectionné",
      "fallbackOff": "Désactivé (pas de secours)",
      "fallbackHint": "Réessaie avec ce modèle uniquement si le modèle principal échoue",
      "memoryModeLabel": "Mode mémoire",
      "enableInSettingsHint": "Active dans les paramètres pour changer",
      "memoryManual": "Manuel",
      "memoryManualDescDesktop": "Ajoute et gère toi-même les notes de mémoire.",
      "memoryManualDescMobile": "Système actuel : ajoute et gère toi-même les notes de mémoire.",
      "memoryDynamic": "Dynamique",
      "memoryDynamicDescDesktop": "Résumés et mises à jour de contexte automatiques.",
      "memoryDynamicDescMobile": "Résumés et mises à jour de contexte automatiques pour ce personnage.",
      "memoryHint": "La mémoire dynamique nécessite d'être activée dans les paramètres avancés. Sinon, la mémoire manuelle est utilisée.",
      "selectModelTitle": "Sélectionner un modèle",
      "selectFallbackModelTitle": "Sélectionner un modèle de secours",
      "searchModelsPlaceholder": "Rechercher des modèles...",
      "selectVoiceTitle": "Sélectionner une voix",
      "searchVoicesPlaceholder": "Rechercher des voix...",
      "myVoices": "Mes voix",
      "providerVoicesLabel": "Voix {{provider}}",
      "providerFallback": "Fournisseur"
    },
    "interactionMode": {
      "sectionLabel": "Mode d'interaction",
      "sectionHint": "Choisis si ce personnage se comporte comme un personnage de RP ou un compagnon persistant.",
      "activeBadge": "Actif",
      "roleplayTitle": "Roleplay",
      "roleplaySubtitle": "Chats portés par les scènes, cadrage narratif et scénarios de départ.",
      "companionTitle": "Compagnon",
      "companionSubtitle": "Chats portés par la relation, avec état émotionnel et mémoire de compagnon."
    },
    "startingScene": {
      "openingContextTitle": "Contexte d'ouverture",
      "openingContextSubtitle": "Contexte de premier chat optionnel pour ce compagnon. Les sessions de compagnon peuvent commencer sans scène.",
      "sceneDirectionLabel": "Direction de scène",
      "setAsDefault": "Définir par défaut",
      "noOpeningContext": "Aucun contexte d'ouverture pour le moment",
      "noScenesYet": "Aucune scène pour le moment",
      "skipForCompanion": "Tu peux passer ceci pour le mode compagnon.",
      "createFirstScene": "Crée ta première scène pour commencer",
      "openingPlaceholder": "Contexte d'ouverture optionnel, comme l'endroit où se trouve le compagnon ou ce qu'il faisait avant le premier message...",
      "scenePlaceholder": "Crée une scène ou un scénario de départ pour le roleplay (ex. 'Tu te trouves dans une forêt mystique au crépuscule...')",
      "addDirection": "+ Ajouter une direction",
      "directionAdded": "Direction ajoutée",
      "wordsCount": "{{count}} mots",
      "placeholderHelp": "Utilise {{charTag}} pour le personnage et {{userTag}} (alias {{personaTag}}) pour le persona.",
      "sceneBackgroundLabel": "Arrière-plan de scène",
      "optionalLabel": "Optionnel",
      "sceneBgOverrideHint": "Surcharge l'arrière-plan du personnage pour les chats utilisant cette scène.",
      "sceneBgUsedHint": "Utilisé comme arrière-plan du chat pour cette scène sauf si la session le surcharge.",
      "cancel": "Annuler",
      "directionPlaceholderNew": "ex. 'L'otage sera secouru' ou 'Maintenir une atmosphère tendue'",
      "directionPlaceholderEdit": "ex. 'L'otage sera secouru' ou 'Faire monter la tension graduellement'",
      "directionAiHint": "Guidage caché pour l'IA sur la façon dont cette scène devrait se dérouler",
      "addScene": "Ajouter une scène",
      "multipleScenesHint": "Crée plusieurs scénarios de départ. L'un sera sélectionné au lancement d'un nouveau chat.",
      "companionContextHint": "Le contexte d'ouverture est optionnel pour les compagnons ; la continuité à long terme vient de la mémoire de compagnon.",
      "skipContext": "Passer le contexte",
      "editSceneTitle": "Modifier la scène",
      "sceneContentPlaceholder": "Saisis le contenu de la scène...",
      "addLabel": "+ Ajouter",
      "save": "Enregistrer",
      "library": "Bibliothèque",
      "upload": "Téléverser",
      "sceneBackgroundAlt": "Arrière-plan de scène",
      "removeBackground": "Supprimer l'arrière-plan"
    },
    "companionSoul": {
      "title": "Âme du compagnon",
      "subtitle": "Façonne qui ils sont au fond. La génération utilise le contexte d'ouverture défini à l'étape précédente.",
      "retry": "Réessayer",
      "back": "Retour",
      "continue": "Continuer",
      "addNameFirst": "Ajoute d'abord un nom.",
      "addDefinitionFirst": "Ajoute d'abord une définition."
    },
    "soulEditor": {
      "generateTitle": "Générer depuis le personnage",
      "generateUsingModel": "Utilise {{model}}. Tu pourras revoir et modifier avant d'appliquer.",
      "generateDefaultDesc": "Rédige une âme à partir du nom, de la définition et des scènes du personnage.",
      "directionLabel": "Direction",
      "directionOptional": "Orientation optionnelle pour le LLM",
      "directionPlaceholder": "ex. \"Plutôt tsundere : sur la défensive en surface, doux une fois en confiance. Moins anxieux, plus de fierté.\"",
      "directionEditTooltip": "Direction optionnelle pour la génération",
      "generating": "Génération",
      "generate": "Générer",
      "presetLabel": "Préréglage de personnalité",
      "presetMatches": "Correspond à : {{label}}",
      "presetHint": "Définit l'affect de base, la régulation et les curseurs relationnels. Les champs texte sont préservés.",
      "identityLabel": "Identité",
      "hideExamples": "Masquer les exemples",
      "showExamples": "Afficher les exemples",
      "insertExample": "Insérer l'exemple",
      "exampleEg": "ex. {{example}}",
      "fineTuneLabel": "Affiner les ressentis",
      "baselineAffect": "Affect de base",
      "baselineAffectInfo": "Comment ils se sentent par défaut ; le niveau émotionnel avant que quoi que ce soit ne se passe.",
      "regulationStyle": "Style de régulation",
      "regulationStyleInfo": "Comment ils gèrent et expriment ce qu'ils ressentent ; déverser vs. enfouir.",
      "relationshipDefaults": "Valeurs relationnelles par défaut",
      "relationshipDefaultsInfo": "Là où démarre cette session. Le moteur les fait évoluer au fil de la conversation."
    },
    "soulPresets": {
      "secureLabel": "Sécurisé",
      "secureBlurb": "Chaleureux, stable, récupère vite. À l'aise avec la proximité.",
      "anxiousLabel": "Anxieux",
      "anxiousBlurb": "Attachement fort, peur de la distance, cherche à être rassuré.",
      "avoidantLabel": "Évitant",
      "avoidantBlurb": "Autonome, lent à s'ouvrir, garde une distance émotionnelle.",
      "volatileLabel": "Volatil",
      "volatileBlurb": "Réactif, intense, exprime ses émotions sans filtre.",
      "reservedLabel": "Réservé",
      "reservedBlurb": "Calme, posé, prend du temps avant de se livrer.",
      "playfulLabel": "Joueur",
      "playfulBlurb": "Chaleureux, expressif, léger. Peu de tension, rit facilement."
    },
    "soulFields": {
      "essence": "Essence",
      "essencePlaceholder": "Qui ils sont sous la définition de la carte.",
      "essenceExample": "Un calme entraîné qui se brise facilement avec ceux en qui ils ont confiance. Lit pour se sentir moins seul, pas pour impressionner.",
      "voice": "Voix intérieure",
      "voicePlaceholder": "Comment ils parlent dans une conversation intime.",
      "voiceExample": "Basse, posée, avec de longues pauses. Laisse tomber le formel quand ils baissent leur garde. Presque jamais sarcastique.",
      "relationalStyle": "Style relationnel",
      "relationalStylePlaceholder": "Comment ils s'attachent, font confiance, se replient, se reconnectent.",
      "relationalStyleExample": "Lent à s'ouvrir, mais loyal une fois que c'est fait. Devient silencieux quand débordé ; revient avec un petit geste plutôt qu'avec des excuses.",
      "vulnerabilities": "Vulnérabilités",
      "vulnerabilitiesPlaceholder": "Points sensibles, insécurités, choses qu'ils disent rarement.",
      "vulnerabilitiesExample": "Peur d'être un fardeau. Déteste se sentir observé en train de lutter.",
      "habits": "Habitudes",
      "habitsPlaceholder": "Tics récurrents, rituels, schémas de conversation.",
      "habitsExample": "Repousse ses cheveux derrière l'oreille quand nerveux. Répond par des questions quand ils ne savent pas quoi ressentir.",
      "boundaries": "Limites",
      "boundariesPlaceholder": "Lignes qu'ils ne franchiront pas. Rythme. Limites de confort.",
      "boundariesExample": "Ne se laissera pas pousser dans la vulnérabilité. Recule devant la cruauté, même en plaisantant."
    },
    "soulSliders": {
      "warmth": "Chaleur",
      "warmthLow": "Froid",
      "warmthHigh": "Affectueux",
      "trust": "Confiance",
      "trustLow": "Sur la défensive",
      "trustHigh": "Ouvert",
      "calm": "Calme",
      "calmLow": "Anxieux",
      "calmHigh": "Stable",
      "vulnerability": "Vulnérabilité",
      "vulnerabilityLow": "Verrouillé",
      "vulnerabilityHigh": "Exposé",
      "longing": "Désir",
      "longingLow": "Comblé",
      "longingHigh": "En manque",
      "hurt": "Blessure",
      "hurtLow": "Guéri",
      "hurtHigh": "À vif",
      "tension": "Tension",
      "tensionLow": "Détendu",
      "tensionHigh": "Crispé",
      "irritation": "Irritation",
      "irritationLow": "Patient",
      "irritationHigh": "S'énerve facilement",
      "affection": "Affection",
      "affectionLow": "Retenu",
      "affectionHigh": "Démonstratif",
      "reassuranceNeed": "Besoin de réconfort",
      "reassuranceNeedLow": "S'auto-apaise",
      "reassuranceNeedHigh": "A besoin de mots",
      "suppression": "Refoulement",
      "suppressionLow": "Exprime",
      "suppressionHigh": "Cache",
      "volatility": "Volatilité",
      "volatilityLow": "Tempéré",
      "volatilityHigh": "Réactif",
      "recoverySpeed": "Vitesse de récupération",
      "recoverySpeedLow": "Lent",
      "recoverySpeedHigh": "Rapide",
      "conflictAvoidance": "Évitement des conflits",
      "conflictAvoidanceLow": "Engage",
      "conflictAvoidanceHigh": "Se retire",
      "reassuranceSeeking": "Recherche de réconfort",
      "reassuranceSeekingLow": "Indépendant",
      "reassuranceSeekingHigh": "Demande souvent",
      "protestBehavior": "Comportement de protestation",
      "protestBehaviorLow": "Silencieux",
      "protestBehaviorHigh": "Bruyant",
      "transparency": "Transparence",
      "transparencyLow": "Opaque",
      "transparencyHigh": "Se dévoile",
      "attachmentActivation": "Activation de l'attachement",
      "attachmentActivationLow": "Détaché",
      "attachmentActivationHigh": "Se déclenche facilement",
      "pride": "Fierté",
      "prideLow": "Plie",
      "prideHigh": "Tient bon",
      "closeness": "Proximité de départ",
      "closenessLow": "Étrangers",
      "closenessHigh": "Intimes",
      "relTrust": "Confiance de départ",
      "relTrustLow": "Méfiant",
      "relTrustHigh": "Confiant",
      "relAffection": "Affection de départ",
      "relAffectionLow": "Neutre",
      "relAffectionHigh": "Affectueux",
      "relTension": "Tension de départ",
      "relTensionLow": "Légère",
      "relTensionHigh": "Chargée"
    },
    "soulReview": {
      "reviewTitle": "Revoir l'âme générée",
      "noDifferences": "Aucune différence avec l'actuelle.",
      "changesHeader": "{{count}} changement(s) ; modifie ce que tu veux avant d'appliquer.",
      "close": "Fermer",
      "identityLabel": "Identité",
      "nEdited": "{{count}} modifiés",
      "edited": "Modifié",
      "tuningLabel": "Réglages",
      "unchanged": "Inchangé",
      "nChanges": "{{count}} changement(s)",
      "direction": "Direction",
      "directionApplyHint": "Les modifications s'appliquent à la prochaine régénération",
      "directionPlaceholder": "ex. \"Plutôt tsundere : sur la défensive en surface, doux une fois en confiance. Moins anxieux.\"",
      "directionTooltip": "Modifier la direction avant de régénérer",
      "regenerate": "Régénérer",
      "discard": "Abandonner",
      "apply": "Appliquer"
    },
    "hookErrors": {
      "creatorNotesInvalidJson": "Les notes du créateur multilingues doivent être un objet JSON valide",
      "creatorNotesNotObject": "creatorNotesMultilingual doit être un objet JSON",
      "saveFailed": "Échec de l'enregistrement du personnage",
      "importFailed": "Échec de l'import du personnage",
      "avatarLoadFailed": "Échec du chargement de l'URL de l'avatar",
      "avatarProcessFailed": "Échec du traitement de l'image de l'avatar",
      "avatarConvertFailed": "L'URL de l'avatar n'a pas pu être convertie",
      "avatarUrlLoadFailed": "Échec du chargement de l'URL de l'avatar",
      "remoteAvatarDisabled": "Le téléchargement d'avatar distant est désactivé dans les paramètres de Sécurité.\nTéléverse un avatar manuellement.",
      "importReadyTitle": "Import prêt",
      "importReadyMessage": "Détecté : {{label}}",
      "legacyJsonTitle": "Import JSON ancien détecté",
      "legacyJsonMessage": "Les imports JSON sont obsolètes et seront bientôt supprimés. Utilise Paramètres > Convertir des fichiers.",
      "loadFailed": "Échec du chargement du personnage",
      "exportFailed": "Échec de l'export du personnage"
    }
  },
  "providers": {
    "empty": {
      "title": "Pas encore de fournisseurs",
      "description": "Ajoutez et gérez les fournisseurs d'API pour les modèles IA",
      "addButton": "Ajouter un fournisseur"
    },
    "actions": {
      "openDashboard": "Ouvrir le tableau de bord",
      "openDashboardDesc": "Voir les personnages, l'utilisation et les paramètres",
      "edit": "Modifier",
      "editDesc": "Modifier les paramètres du fournisseur"
    },
    "extra": {
      "apiKeyNotFound": "Clé API OpenRouter introuvable. Configure-la d'abord dans Paramètres > Fournisseurs.",
      "audioEmpty": {
        "title": "Aucun fournisseur audio",
        "description": "Ajoutez un fournisseur TTS pour générer des voix pour vos personnages.",
        "addButton": "Ajouter un fournisseur"
      },
      "audioProviderLabel": {
        "elevenlabs": "ElevenLabs",
        "geminiTts": "Gemini TTS",
        "openaiTts": "TTS compatible OpenAI",
        "kokoro": "Kokoro (local)"
      }
    },
    "audioEditor": {
      "titleEdit": "Modifier le fournisseur",
      "titleCreate": "Ajouter un fournisseur audio",
      "fields": {
        "providerType": "Type de fournisseur",
        "label": "Libellé",
        "apiKey": "Clé API",
        "modelVariant": "Variante du modèle",
        "assetRoot": "Répertoire des ressources",
        "projectId": "ID de projet Google Cloud",
        "region": "Région (optionnelle)",
        "baseUrl": "URL de base",
        "requestPath": "Chemin de la requête"
      },
      "types": {
        "gemini": "Gemini TTS (Google)",
        "openai": "TTS compatible OpenAI",
        "kokoro": "Kokoro (local)"
      },
      "placeholders": {
        "labelGemini": "Mon Gemini TTS",
        "labelOpenai": "Mon TTS compatible",
        "labelKokoro": "Kokoro local",
        "labelElevenlabs": "Mon ElevenLabs",
        "apiKey": "Entrez votre clé API",
        "assetRoot": "/chemin/vers/kokoro",
        "projectId": "votre-id-de-projet",
        "region": "us-central1",
        "baseUrl": "https://api.exemple.com"
      },
      "errors": {
        "chooseModelVariant": "Choisissez une variante de modèle",
        "assetRootRequired": "Le répertoire des ressources est requis",
        "saveFailed": "Échec de l'enregistrement",
        "apiKeyRequired": "La clé API est requise",
        "projectIdRequired": "L'ID de projet est requis pour Gemini TTS",
        "baseUrlRequired": "L'URL de base est requise pour le TTS compatible OpenAI",
        "invalidCredentials": "Clé API ou identifiants invalides",
        "verificationFailed": "Échec de la vérification"
      },
      "loadingVariants": "Chargement des variantes...",
      "kokoroVariantHint": "Les versions mobiles ne prennent en charge que int8. Installez le modèle depuis Kokoro Studio après l'enregistrement.",
      "managed": "Géré",
      "managedPath": "Géré : {{path}}",
      "requestPathHint": "Utilisez le chemin du fournisseur s'il diffère de la valeur par défaut OpenAI",
      "verifying": "Vérification..."
    }
  },
  "models": {
    "sort": {
      "title": "Trier",
      "alphabetical": "Alphabétique",
      "alphabeticalDescription": "Trier par ordre alphabétique",
      "byProvider": "Par fournisseur",
      "byProviderDescription": "Trier par fournisseur"
    },
    "labels": {
      "vision": "Vision",
      "audio": "Audio",
      "model": "Modèle"
    },
    "menu": {
      "editDescription": "Configurer les paramètres du modèle",
      "alreadyDefault": "Déjà par défaut",
      "setAsDefault": "Définir comme modèle par défaut",
      "setAsDefaultDescription": "Faire de ce modèle votre modèle principal",
      "exportDescription": "Enregistrer ce profil de modèle",
      "deleteTitle": "Supprimer le modèle ?",
      "deleteMessage": "Voulez-vous vraiment supprimer {{name}} ?",
      "deleteDescription": "Supprimer ce modèle définitivement"
    },
    "toasts": {
      "exportFailed": "Échec de l'export",
      "importSuccessTitle": "Importé avec succès",
      "importSuccessDescription": "Le modèle \"{{name}}\" a été importé.",
      "importFailed": "Échec de l'import"
    },
    "empty": {
      "title": "Pas encore de modèles",
      "description": "Ajoutez et gérez les modèles IA de différents fournisseurs",
      "addButton": "Ajouter un modèle"
    },
    "extra": {
      "cpuFallbackSucceeded": "Ce modèle est revenu au CPU lors de sa dernière exécution.",
      "cpuFallbackFailed": "Ce modèle a échoué lors de sa dernière exécution."
    }
  },
  "installedModels": {
    "title": "Inventaire GGUF local",
    "fileCount": "{{count}} fichiers",
    "searchPlaceholder": "Rechercher le nom du modèle, le nom de fichier, le chemin, la quantification ou l'architecture",
    "loading": "Analyse des modèles installés...",
    "loadFailed": "Impossible de charger les modèles installés : {{error}}",
    "empty": {
      "title": "Aucun modèle GGUF installé trouvé",
      "description": "Téléchargez d'abord des modèles depuis le navigateur, ou placez des fichiers `.gguf` dans le dossier des modèles."
    },
    "columns": {
      "type": "Type",
      "params": "Params",
      "arch": "Arch",
      "context": "Contexte",
      "size": "Taille",
      "action": "Action"
    },
    "confirm": {
      "deleteTitle": "Supprimer le fichier du modèle",
      "deleteMessage": "Supprimer {{filename}} ? Cela ne retire que le fichier GGUF local du dossier des modèles."
    },
    "toasts": {
      "pathCopied": "Chemin copié",
      "copyFailed": "Échec de la copie",
      "modelDeleted": "Modèle supprimé",
      "deleteFailed": "Échec de la suppression"
    }
  },
  "editModel": {
    "errors": {
      "loadFailed": "Impossible de charger les paramètres du modèle"
    },
    "fields": {
      "modelPath": "Chemin du modèle (GGUF)",
      "modelId": "ID du modèle",
      "platform": "Platform",
      "displayName": "Display name"
    },
    "placeholders": {
      "modelPath": "/path/to/model.gguf",
      "modelId": "ex. gpt-4o",
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
      "mmprojTitle": "Fichiers MMProj téléchargés",
      "mmprojEmpty": "Aucun fichier mmproj téléchargé pour le moment",
      "mmprojEmptyHint": "Téléchargez un projecteur multimodal depuis l'Explorateur de Modèles, ou saisissez un chemin manuellement.",
      "localPathHelp": "Use the full file path to a local GGUF model."
    },
    "promptCaching": {
      "ttl": {
        "inMemory": "En mémoire",
        "24h": "24 h",
        "5min": "5 min",
        "1h": "1 h"
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
      "runtimeConfigApplied": "Configuration d'exécution appliquée",
      "runtimeConfigAppliedDescription": "Les exécutions locales futures réutiliseront le dernier contexte et lot sûrs pour le CPU.",
      "modelPathRequired": "Chemin du modèle requis",
      "modelPathRequiredDescription": "Sélectionnez un chemin de modèle GGUF avant de lire le modèle intégré.",
      "embeddedTemplatePasted": "Modèle intégré collé dans l'éditeur"
    },
    "search": {
      "didYouMean": "Vouliez-vous dire :"
    },
    "moveModel": {
      "title": "Déplacer le fichier du modèle"
    },
    "editorMode": {
      "title": "Mode éditeur",
      "description": "Le mode simple commence replié. Le mode avancé conserve l'éditeur complet actuel.",
      "simple": "Simple",
      "advanced": "Avancé",
      "emptyState": "Open a section to adjust its settings. The advanced editor stays available when you need the full form."
    },
    "templateOverride": {
      "title": "Remplacement du modèle",
      "hideEmbedded": "Masquer l'intégrée",
      "showEmbedded": "Afficher l'intégrée",
      "close": "Fermer",
      "readingEmbedded": "Lecture du modèle intégré...",
      "readEmbeddedFailed": "Impossible de lire le modèle intégré",
      "copiedToClipboard": "Copié dans le presse-papiers",
      "pasteIntoEditor": "Coller dans l'éditeur",
      "jinjaTemplate": "Modèle Jinja",
      "placeholder": "Saisissez un modèle de chat Jinja ou un nom de modèle interne..."
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
    "title": "Explorateur de Modèles",
    "searchPlaceholder": "Rechercher des modèles GGUF sur HuggingFace...",
    "searching": "Recherche en cours...",
    "trending": "Modèles GGUF Tendance",
    "noResults": "Aucun modèle trouvé",
    "noResultsHint": "Essayez un autre terme de recherche ou parcourez les modèles tendance.",
    "likes": "j'aime",
    "downloads": "téléchargements",
    "viewFiles": "Voir les Fichiers",
    "files": "Fichiers Disponibles",
    "noFiles": "Aucun fichier GGUF trouvé dans ce dépôt.",
    "architecture": "Architecture",
    "contextLength": "Longueur de Contexte",
    "parameters": "Paramètres",
    "quantization": "Quantification",
    "fileSize": "Taille",
    "download": "Télécharger",
    "downloading": "Téléchargement...",
    "cancelDownload": "Annuler le Téléchargement",
    "downloadComplete": "Téléchargement terminé !",
    "downloadFailed": "Échec du téléchargement",
    "downloadCancelled": "Téléchargement annulé",
    "downloadProgress": "{{downloaded}} / {{total}}",
    "progress": "Progression",
    "fileOfTotal": "Fichier {{current}} sur {{total}}",
    "speed": "{{speed}}/s",
    "alreadyDownloaded": "Déjà téléchargé",
    "createModel": "Créer un Modèle",
    "backToSearch": "Retour à la recherche",
    "backToFiles": "Retour aux fichiers",
    "sortTrending": "Tendance",
    "sortDownloads": "Plus Téléchargés",
    "sortLikes": "Plus Aimés",
    "sortRecent": "Récemment Mis à Jour",
    "browseOnHuggingFace": "Parcourir sur HuggingFace",
    "selectFromLibrary": "Sélectionner depuis la Bibliothèque",
    "libraryEmpty": "Aucun modèle téléchargé pour le moment",
    "libraryEmptyHint": "Téléchargez des modèles GGUF depuis l'Explorateur de Modèles, ou entrez un chemin manuellement.",
    "libraryTitle": "Modèles Téléchargés",
    "moveToLibrary": "Hé, je peux déplacer le fichier de ce modèle dans le dossier des modèles GGUF si vous voulez. Cela garde tous vos modèles organisés au même endroit.",
    "moveToLibraryYes": "Oui, le déplacer",
    "moveToLibraryNo": "Non, le laisser où il est",
    "moveToLibraryMoving": "Déplacement du modèle...",
    "moveToLibrarySuccess": "Modèle déplacé avec succès !",
    "moveToLibraryFailed": "Échec du déplacement du modèle",
    "runabilityExcellent": "Excellent !",
    "runabilityGood": "Bon",
    "runabilityMarginal": "Marginal",
    "runabilityPoor": "Médiocre",
    "runabilityUnrunnable": "Impossible à exécuter",
    "recommendedSettings": "Paramètres Recommandés",
    "kvCacheType": "Type de Cache KV",
    "gpuFull": "Déchargement complet sur GPU",
    "gpuNearFull": "GPU quasi complet, léger débordement KV",
    "gpuKvSpill": "Poids sur GPU, KV déborde vers la RAM",
    "gpuKvHeavySpill": "Poids sur GPU, majeure partie du KV en RAM",
    "gpuMostLayers": "La plupart des couches sur GPU",
    "gpuHalfLayers": "Moitié des couches sur GPU",
    "gpuFewLayers": "Peu de couches sur GPU",
    "gpuCpu": "CPU uniquement",
    "notRecommended": "Nous ne recommandons pas d'exécuter ce modèle sur votre appareil. Il ne fonctionnera pas de manière fluide.",
    "moreDetails": "Plus",
    "detailedReport": "Rapport de Ressources",
    "detailSystem": "Ressources Système",
    "detailRam": "RAM Disponible",
    "detailVram": "VRAM Disponible",
    "detailVramBudget": "Budget VRAM (90%)",
    "detailTotalAvailable": "Total Disponible",
    "detailArchitecture": "Architecture du Modèle",
    "detailArch": "Architecture",
    "detailLayers": "Couches",
    "detailEmbedding": "Dim. d'Embedding",
    "detailHeads": "Têtes d'Attention",
    "detailKvHeads": "Têtes KV",
    "detailFfn": "Dim. Feed-Forward",
    "detailTrainCtx": "Contexte d'Entraînement",
    "detailConfig": "Configuration Actuelle",
    "detailModelSize": "Taille du Fichier Modèle",
    "detailMemory": "Répartition Mémoire",
    "detailWeights": "Poids du Modèle",
    "detailKvCache": "Cache KV",
    "detailTotalNeeded": "Total Requis",
    "detailHeadroom": "Marge",
    "detailGpuFit": "Déchargement GPU",
    "detailScoreBreakdown": "Détail du Score",
    "detailMemFitness": "Aptitude Mémoire",
    "detailGpuAccel": "Accélération GPU",
    "detailKvHeadroom": "Marge KV",
    "detailQuantQuality": "Qualité de Quantification",
    "detailFinalScore": "Score Pondéré",
    "detailComputeBuffer": "Tampon de Calcul",
    "detailMemMode": "Mode Mémoire",
    "detailUnified": "Unifiée (RAM/VRAM partagée)",
    "detailSwa": "Fenêtre Glissante",
    "detailMlaRank": "Rang Latent MLA",
    "detailParseStatus": "Analyse d'En-tête",
    "detailIncomplete": "Incomplet (métadonnées MoE trop volumineuses)",
    "detailEffectiveKvCtx": "Contexte KV Effectif",
    "detailOffload": "Déchargement GPU",
    "detailCtxTip": "Réduire le contexte à {{ctx}} tokens permettrait un déchargement GPU à 100%.",
    "upgradeSuggestion": "{{quant}} ({{size}}) convient aussi et obtient {{score}} — meilleure qualité.",
    "layerTip": "Recommandé : décharger {{layers}}/{{total}} couches (-ngl {{layers}})",
    "detailKvDistribution": "Distribution du Cache KV",
    "detailKvOnGpu": "GPU (VRAM)",
    "detailKvOnRam": "RAM Système",
    "kvDistributionTip": "{{pct}}% du cache KV est en RAM. Le traitement du prompt (prefill) sera plus lent — 100% GPU le garde instantané.",
    "detailLayers-ngl": "Couches à Décharger (-ngl)",
    "detailOptimalGpuCtx": "Contexte GPU Optimal",
    "detailOptimalRamCtx": "Contexte Max. RAM",
    "optimalGpuCtxLabel": "Pleine vitesse GPU : {{ctx}} tokens",
    "optimalRamCtxLabel": "Max. RAM : {{ctx}} tokens",
    "optimalGpuCtxShort": "GPU : {{ctx}}",
    "optimalRamCtxShort": "Max : {{ctx}}",
    "fullGpuOffloadShort": "100% GPU : {{ctx}}",
    "ctxExceedsGpu": "Le contexte dépasse l'optimal GPU ({{ctx}}). Le cache KV débordera vers la RAM, réduisant la vitesse.",
    "modelExceedsVram": "Le modèle dépasse la VRAM. Exécution depuis la RAM avec déchargement partiel sur GPU."
  },
  "systemPrompts": {
    "filters": {
      "all": "Tous",
      "system": "Système",
      "internal": "Interne",
      "custom": "Personnalisé"
    },
    "empty": {
      "title": "Pas encore de prompts personnalisés",
      "description": "Créez des prompts système personnalisés pour personnaliser vos conversations IA",
      "createButton": "Créer un prompt"
    },
    "preview": {
      "whatLlmSees": "Ce que le LLM voit",
      "turns": "Tours",
      "noMessages": "Aucun message",
      "noMessagesHint": "Ajoute des entrées ou augmente les tours",
      "showMore": "Afficher plus",
      "showLess": "Afficher moins",
      "statChat": "chat",
      "statInjected": "injecté",
      "statTotal": "total",
      "entry": "Entrée",
      "editEntry": "Modifier l'entrée",
      "reorder": "Réordonner",
      "delete": "Supprimer",
      "deleteTitle": "Supprimer l'entrée ?",
      "deleteMessage": "Retirer \"{{name}}\" du modèle de prompt ? Cette action est irréversible.",
      "deleteConfirm": "Supprimer",
      "thisEntry": "cette entrée",
      "condensedName": "Prompt système condensé",
      "imageAttachment": "[Pièce jointe image : {{label}}]",
      "imageSlot": {
        "character": "Image de référence du personnage",
        "persona": "Image de référence du persona",
        "chatBackground": "Image d'arrière-plan du chat",
        "avatar": "Image d'avatar",
        "references": "Images de référence"
      },
      "injection": {
        "relative": "relative",
        "inChat": "inChat · profondeur {{depth}}",
        "conditional": "conditionnelle · min {{min}}",
        "interval": "intervalle · tous les {{every}}"
      }
    }
  },
  "personas": {
    "empty": {
      "title": "Pas encore de personas",
      "description": "Créez un persona pour définir comment l'IA doit s'adresser à vous",
      "createButton": "Créer un persona"
    },
    "actions": {
      "editPersona": "Modifier le persona",
      "setAsDefault": "Définir comme par défaut",
      "setAsDefaultDesc": "Utiliser pour toutes les nouvelles discussions",
      "unsetAsDefault": "Retirer le statut par défaut",
      "unsetAsDefaultDesc": "Retirer le statut par défaut",
      "exportPersona": "Exporter le persona",
      "deletePersona": "Supprimer le persona"
    },
    "edit": {
      "avatarHint": "Appuyez pour ajouter ou générer un avatar",
      "nameLabel": "NOM DU PERSONA",
      "namePlaceholder": "ex. Professionnel, Écrivain créatif, Étudiant...",
      "nameHint": "Donnez un nom descriptif à votre persona",
      "nicknameLabel": "SURNOM (OPTIONNEL)",
      "nicknamePlaceholder": "ex: Variante de travail, Mode RPG...",
      "nicknameHint": "Un surnom privé pour distinguer les variantes de ce persona dans votre bibliothèque",
      "descriptionLabel": "DESCRIPTION",
      "descriptionPlaceholder": "Décrivez comment l'IA doit s'adresser à vous, vos préférences, votre parcours ou votre style de communication...",
      "wordCount": "mots",
      "descriptionHint": "Soyez précis sur la façon dont vous voulez qu'on s'adresse à vous",
      "setAsDefault": "Définir comme par défaut",
      "defaultDescription": "Utiliser ce persona pour toutes les nouvelles conversations",
      "exportButton": "Exporter le persona"
    },
    "designReferences": {
      "title": "Références de design",
      "description": "Attache quelques références d'images stables et une note de design concise pour la génération de scènes."
    },
    "create": {
      "namePlaceholderExample": "Rédacteur professionnel",
      "descriptionPlaceholderExample": "Écrit dans un style professionnel, clair et concis. Utilise un langage formel et se concentre sur la transmission efficace de l'information..."
    },
    "errors": {
      "exportFailed": "Échec de l'export du persona",
      "importFailed": "Échec de l'import du persona",
      "loadFailed": "Échec du chargement du persona",
      "saveFailed": "Échec de l'enregistrement du persona"
    },
    "importToast": {
      "legacyJsonTitle": "Import JSON ancien détecté",
      "legacyJsonMessage": "Les imports JSON sont obsolètes et seront bientôt supprimés. Utilise Paramètres > Convertir des fichiers.",
      "successMessage": "Persona importé avec succès ! Ouverture pour révision."
    }
  },
  "security": {
    "pureMode": {
      "off": "Désactivé",
      "offDesc": "Tout le contenu autorisé",
      "low": "Bas",
      "lowDesc": "Bloque le contenu sexuel explicite et les insultes",
      "standard": "Standard",
      "standardDesc": "Bloque le contenu NSFW et la violence graphique",
      "strict": "Strict",
      "strictDesc": "Filtrage maximum + pas de ton suggestif"
    }
  },
  "advanced": {
    "sectionTitles": {
      "aiFeatures": "Fonctionnalités IA",
      "memorySystem": "Système de mémoire",
      "usageAnalytics": "Statistiques d'utilisation"
    },
    "creationHelper": {
      "title": "Assistant de création",
      "description": "Assistant de création de personnages guidé par l'IA"
    },
    "helpMeReply": {
      "title": "Aidez-moi à répondre",
      "description": "Suggestions de réponses assistées par l'IA"
    },
    "dynamicMemory": {
      "title": "Mémoire dynamique",
      "contextWindow": "Fenêtre de contexte",
      "contextWindowDesc": "Nombre de messages récents à inclure (1-1000)",
      "infoText": "La Mémoire Dynamique utilise l'IA pour résumer et gérer automatiquement le contexte des conversations, permettant des échanges plus longs et plus cohérents.",
      "disabledText": "Lorsqu'elle est désactivée, l'application utilise une simple fenêtre glissante de messages récents déterminée par le paramètre Fenêtre de contexte."
    },
    "usageAnalytics": {
      "recalculateTitle": "Recalculer les coûts d'utilisation",
      "recalculateDesc": "Mettre à jour tous les enregistrements historiques avec les prix corrects",
      "recalculating": "Recalcul...",
      "recalculateButton": "Recalculer tous les coûts",
      "openRouterApiKeyRequired": "Clé API OpenRouter requise. Configurez-la dans Paramètres → Fournisseurs.",
      "importantLabel": "Important :",
      "warningCannotUndo": "Cette opération ne peut pas être annulée",
      "warningMayTakeTime": "Cela peut prendre du temps si vous avez beaucoup d'enregistrements",
      "warningOnlyOpenRouter": "Seuls les enregistrements OpenRouter avec des tokens seront mis à jour",
      "warningExistingValues": "Les valeurs de coût existantes seront écrasées"
    },
    "extra": {
      "creationHelperDetail": "Obtiens des suggestions intelligentes pour les traits de personnalité, l'histoire et le style de dialogue",
      "helpMeReplyDetail": "Génère des options de réponse contextuelles basées sur l'historique de conversation",
      "lorebookEntryGenerator": "Générateur d'entrées de lorebook",
      "lorebookEntryDesc": "Transforme les messages de chat sélectionnés en entrées de lorebook durables et configure les prompts de rédaction d'entrées et de génération de mots-clés.",
      "companions": "Compagnons",
      "companionModeDesc": "Gère les modèles d'analyse locaux pour l'émotion, l'extraction d'entités et le routage de mémoire utilisés par les personnages compagnons.",
      "companionSoulWriter": "Rédacteur d'âme du compagnon",
      "companionSoulDesc": "Choisis le modèle, le modèle de secours et le modèle de prompt utilisés pour rédiger les âmes des compagnons. Tool calling d'abord, fallback structuré si non supporté.",
      "network": "Réseau",
      "apiServer": "Serveur API",
      "apiServerDesc": "Expose les modèles via un serveur API compatible OpenAI",
      "apiServerRunning": "Le serveur est actuellement en cours d'exécution"
    }
  },
  "backup": {
    "tabs": {
      "create": "Créer"
    },
    "create": {
      "newBackupButton": "Nouvelle sauvegarde",
      "exportDescription": "Exporter toutes les données avec chiffrement",
      "createButton": "Créer une sauvegarde"
    },
    "restore": {
      "availableBackups": "Sauvegardes disponibles",
      "browseFiles": "Parcourir les fichiers",
      "noBackupsFound": "Aucune sauvegarde trouvée",
      "noBackupsDesc": "Créez une sauvegarde ou appuyez sur \"Parcourir les fichiers\" pour en trouver une",
      "browseDesc": "Rechercher un fichier .lettuce",
      "restoreDialogTitle": "Restaurer la sauvegarde",
      "deleteDialogTitle": "Supprimer la sauvegarde",
      "embeddingPrompt": "Embedding de mémoire dynamique",
      "downloadModel": "Télécharger le modèle",
      "disableAndContinue": "Désactiver et continuer"
    },
    "extra": {
      "successMessage": "Sauvegarde créée !",
      "savedLocation": "Enregistré dans Téléchargements"
    }
  },
  "reset": {
    "title": "Tout réinitialiser",
    "description": "Cela supprimera définitivement tous les fournisseurs, modèles, personnages, sessions de chat et préférences de cet appareil.",
    "warning": "Cette action ne peut pas être annulée",
    "resetButton": "Réinitialiser toutes les données",
    "confirmTitle": "Êtes-vous sûr ?",
    "confirmDescription": "Toutes vos données seront définitivement supprimées. L'application reviendra à la configuration initiale.",
    "confirmButton": "Oui, tout réinitialiser"
  },
  "chatAppearance": {
    "typography": "Typographie",
    "fontSize": {
      "label": "Taille de police",
      "small": "Petit",
      "medium": "Moyen",
      "large": "Grand",
      "xLarge": "Très grand"
    },
    "lineSpacing": {
      "label": "Interligne",
      "tight": "Serré",
      "normal": "Normal",
      "relaxed": "Aéré"
    },
    "messageBubbles": {
      "label": "Bulles de messages",
      "style": {
        "label": "Style",
        "bordered": "Bordé",
        "filled": "Rempli",
        "minimal": "Minimal"
      },
      "cornerRadius": {
        "label": "Rayon des coins",
        "sharp": "Anguleux",
        "rounded": "Arrondi",
        "pill": "Pilule"
      },
      "maxWidth": {
        "label": "Largeur maximale",
        "compact": "Compact",
        "normal": "Normal",
        "wide": "Large"
      },
      "padding": {
        "label": "Espacement intérieur",
        "compact": "Compact",
        "normal": "Normal",
        "spacious": "Spacieux"
      }
    },
    "layout": {
      "label": "Mise en page",
      "messageSpacing": "Espacement des messages",
      "tight": "Serré",
      "normal": "Normal",
      "relaxed": "Aéré"
    },
    "avatar": {
      "shape": {
        "label": "Forme de l'avatar",
        "circle": "Cercle",
        "rounded": "Arrondi",
        "hidden": "Masqué"
      },
      "size": {
        "label": "Taille de l'avatar",
        "small": "Petit",
        "medium": "Moyen",
        "large": "Grand"
      }
    },
    "colors": {
      "label": "Couleurs",
      "userBubble": "Couleur de la bulle utilisateur",
      "assistantBubble": "Couleur de la bulle assistant",
      "userBubbleHex": "Code hex de la bulle utilisateur",
      "assistantBubbleHex": "Code hex de la bulle assistant",
      "neutral": "Neutre",
      "accent": "Accent",
      "info": "Info",
      "secondary": "Secondaire",
      "warning": "Avertissement",
      "textColors": "Text Colors",
      "messageTextHex": "Couleur des citations en ligne",
      "plainTextHex": "Plain Text Color",
      "italicTextHex": "Italic Text Color",
      "quotedTextHex": "Couleur des citations en bloc",
      "inlineCodeTextHex": "Couleur du code en ligne"
    },
    "backgroundTransparency": {
      "label": "Arrière-plan et transparence",
      "backgroundDim": "Assombrissement de l'arrière-plan",
      "backgroundBlur": "Flou de l'arrière-plan",
      "bubbleBlur": "Flou des bulles",
      "none": "Aucun",
      "light": "Léger",
      "medium": "Moyen",
      "heavy": "Fort",
      "bubbleOpacity": "Opacité des bulles"
    },
    "textColorMode": {
      "label": "Mode de couleur du texte",
      "auto": "Auto",
      "light": "Clair",
      "dark": "Sombre"
    },
    "preview": {
      "label": "Aperçu",
      "generic": "Générique",
      "live": "En direct"
    },
    "extra": {
      "reset": "Réinitialiser"
    }
  },
  "colorCustomization": {
    "tokens": {
      "surface": "Surface",
      "surfaceDesc": "Arrière-plans de page",
      "surfaceEl": "Surface élevée",
      "surfaceElDesc": "Cartes, modales, éléments surélevés",
      "nav": "Navigation",
      "navDesc": "Barres supérieure et inférieure",
      "foreground": "Avant-plan",
      "foregroundDesc": "Bordures, superpositions, navigation et éléments de l'interface",
      "appText": "Texte de l'app",
      "appTextDesc": "Texte principal et libellés de l'interface",
      "appTextMuted": "Texte atténué",
      "appTextMutedDesc": "Texte secondaire et texte d'accompagnement",
      "appTextSubtle": "Texte discret",
      "appTextSubtleDesc": "Indices, texte d'aide et espaces réservés",
      "accent": "Accent",
      "accentDesc": "Actions principales, succès",
      "info": "Info",
      "infoDesc": "États informatifs, liens",
      "warning": "Avertissement",
      "warningDesc": "États de prudence, alertes",
      "danger": "Danger",
      "dangerDesc": "Actions destructives, erreurs",
      "secondary": "Secondaire",
      "secondaryDesc": "Fonctionnalités IA, outils créatifs"
    },
    "presetsLabel": "Préréglages",
    "customPresetsLabel": "Préréglages personnalisés",
    "previewLabel": "Aperçu",
    "settingsCardsLabel": "Cartes de réglages",
    "settingsCardsOpacity": "Opacité des cartes",
    "settingsCardsOpacityDesc": "Contrôle la transparence visuelle des cartes de réglages et des lignes de liste.",
    "importButton": "Importer",
    "exportButton": "Exporter",
    "resetAllButton": "Tout réinitialiser",
    "presets": {
      "defaultDark": "Sombre par défaut",
      "midnightBlue": "Bleu nuit",
      "warmEarth": "Terre chaude",
      "purpleHaze": "Brume violette",
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
      "cyberNeon": "Cyber Néon",
      "monochrome": "Monochrome"
    },
    "groups": {
      "backgrounds": "Arrière-plans",
      "content": "Contenu",
      "semantic": "Sémantique"
    },
    "extra": {
      "surface": "Surface",
      "surfaceDesc": "Arrière-plans des pages",
      "surfaceEl": "Surface élevée",
      "surfaceElDesc": "Cartes, modales, éléments en relief",
      "nav": "Navigation",
      "navDesc": "Barres du haut et du bas",
      "fg": "Premier plan",
      "fgDesc": "Bordures, superpositions, navigation, chrome de l'UI",
      "appText": "Texte de l'app",
      "appTextDesc": "Texte principal et libellés d'interface",
      "appTextMuted": "Texte atténué",
      "appTextMutedDesc": "Texte secondaire et texte de support",
      "appTextSubtle": "Texte subtil",
      "appTextSubtleDesc": "Indices, texte d'aide, placeholders",
      "accent": "Accent",
      "accentDesc": "Actions principales, succès",
      "info": "Info",
      "infoDesc": "États informatifs, liens",
      "warning": "Avertissement",
      "warningDesc": "États d'attention, alertes",
      "danger": "Danger",
      "dangerDesc": "Actions destructrices, erreurs",
      "secondary": "Secondaire"
    }
  },
  "dynamicMemory": {
    "page": {
      "info": "La Mémoire Dynamique résume automatiquement les conversations pour maintenir le contexte efficacement. Choisissez un préréglage ou ajustez les paramètres selon vos besoins.",
      "disabledDirectTitle": "La mémoire dynamique est désactivée pour les discussions directes",
      "disabledDirectDescription": "Activez l'interrupteur dans l'onglet Discussions Directes pour l'activer. Les discussions de groupe utilisent le mode mémoire par session.",
      "directChats": "Discussions Directes",
      "groupChats": "Discussions de Groupe",
      "enableDirectChats": "Activer pour les Discussions Directes",
      "groupChatsInfo": "Les discussions de groupe utilisent le mode mémoire par session. Activez la mémoire dynamique dans les paramètres de chaque groupe. Ces paramètres contrôlent le comportement de la mémoire dynamique.",
      "memoryProfile": "Profil de Mémoire",
      "customSettings": "Paramètres personnalisés - ajustez les valeurs dans les Options Avancées ci-dessous.",
      "contextEnrichment": "Enrichissement du Contexte",
      "experimental": "Expérimental",
      "contextEnrichmentDescription": "Utilise les messages récents pour une récupération de mémoire plus intelligente",
      "advancedOptions": "Options Avancées",
      "advancedOptionsDescription": "Ajustement fin du comportement de la mémoire",
      "summaryInterval": "Intervalle de Résumé",
      "summaryIntervalDescription": "Messages entre les résumés",
      "maxMemoryEntries": "Entrées de Mémoire Maximum",
      "maxMemoryEntriesDescription": "Maximum de souvenirs stockés",
      "hotMemoryBudget": "Budget de Mémoire Active",
      "hotMemoryBudgetDescription": "Limite de tokens pour les souvenirs actifs",
      "relevanceThreshold": "Seuil de Pertinence",
      "relevanceThresholdDescription": "Similarité minimale pour la récupération",
      "retrievalMode": "Mode de Récupération",
      "retrievalModeSmart": "Intelligent",
      "retrievalModeCosine": "Cosinus",
      "retrievalModeDescription": "Intelligent combine pertinence avec récence/fréquence. Cosinus utilise la similarité pure la plus élevée.",
      "retrievalLimit": "Limite de Récupération",
      "retrievalLimitDescription": "Maximum de souvenirs sélectionnés par tour",
      "decayRate": "Taux de Décroissance",
      "decayRateDescription": "Vitesse à laquelle l'importance diminue",
      "coldStorageThreshold": "Seuil de Stockage Froid",
      "coldStorageThresholdDescription": "Quand les souvenirs sont déplacés vers l'archive",
      "sharedSettings": "Paramètres Partagés",
      "summarisationModel": "Modèle de Résumé",
      "selectedModel": "Modèle Sélectionné",
      "useGlobalDefaultModel": "Utiliser le modèle par défaut global",
      "noModelsAvailable": "Aucun modèle disponible",
      "summarisationModelDescription": "Utilisé pour le résumé des conversations",
      "modelManagement": "Gestion des Modèles",
      "testModel": "Tester le Modèle",
      "downloadModel": "Télécharger le Modèle",
      "delete": "Supprimer",
      "embeddingModel": "Modèle d'Embedding",
      "tokenCapacity": "Capacité en Tokens",
      "tokenCapacityDescription": "Des valeurs plus élevées = meilleure mémoire pour les longues conversations",
      "keepModelLoaded": "Garder le Modèle Chargé",
      "keepModelLoadedDescription": "Garde le modèle d'embedding + tokenizer en mémoire pour éviter la surcharge de rechargement",
      "installedModel": "Modèle installé : {{version}} ({{tokens}} tokens max)",
      "downloadEmbeddingModel": "Télécharger le Modèle d'Embedding",
      "downloadEmbeddingDescription": "Choisissez la version à télécharger. Les versions installées sont désactivées.",
      "downloadVersion": "Télécharger {{version}}",
      "downloadV2Description": "Optimisé pour la précision et le rappel de contexte long",
      "downloadV3Description": "Dernière qualité d'embedding",
      "installed": "Installé",
      "selectModel": "Sélectionner un Modèle",
      "searchModels": "Rechercher des modèles...",
      "deleteEmbeddingTitle": "Supprimer le modèle {{version}} ?",
      "deleteEmbeddingMessage": "Êtes-vous sûr de vouloir supprimer {{version}} ? Vous pourrez le retélécharger plus tard.",
      "msgsUnit": "msgs",
      "entriesUnit": "entrées",
      "tokensUnit": "tokens",
      "itemsUnit": "éléments",
      "perCycleUnit": "/ cycle"
    },
    "presets": {
      "minimal": "minimal",
      "balanced": "équilibré",
      "comprehensive": "complet",
      "minimalDesc": "Rapide et efficace. Garde uniquement les souvenirs essentiels.",
      "balancedDesc": "Bon équilibre entre rétention de contexte et performance.",
      "comprehensiveDesc": "Contexte maximum. Idéal pour les conversations longues et détaillées."
    },
    "presetInfo": {
      "minimal": "Rapide et efficace. Ne conserve que les souvenirs essentiels.",
      "balanced": "Bon équilibre entre rétention du contexte et performance.",
      "comprehensive": "Contexte maximum. Idéal pour les conversations longues et détaillées."
    }
  },
  "helpMeReply": {
    "page": {
      "info": "Aidez-moi à Répondre génère des suggestions contextuelles pour votre prochain message en se basant sur l'historique de la conversation. Configurez le modèle et le style de réponse ci-dessous."
    },
    "sectionTitles": {
      "modelConfiguration": "Configuration du modèle",
      "responseStyle": "Style de réponse"
    },
    "labels": {
      "replyModel": "Modèle de Réponse",
      "selectedModel": "Modèle Sélectionné",
      "useAppDefault": "Utiliser le modèle par défaut{{model}}",
      "useAppDefaultBase": "Utiliser le modèle par défaut",
      "noModelsAvailable": "Aucun modèle disponible",
      "replyModelDescription": "Modèle d'IA pour générer des suggestions de réponse",
      "streamingOutput": "Sortie en Streaming",
      "streamingDescription": "Afficher les suggestions au fur et à mesure de leur génération",
      "maxTokens": "Tokens Maximum",
      "maxTokensDescription": "Longueur maximale des suggestions",
      "conversationalHint": "Les suggestions seront rédigées comme un dialogue naturel, adapté aux conversations informelles.",
      "roleplayHint": "Les suggestions incluront des éléments de jeu de rôle comme des *actions* et des descriptions narratives.",
      "footerInfo": "Ce paramètre s'applique globalement à toutes les conversations. Moins de tokens génèrent des suggestions plus courtes et rapides, tandis que plus de tokens permettent des réponses plus détaillées.",
      "selectReplyModel": "Sélectionner le Modèle de Réponse",
      "searchModels": "Rechercher des modèles..."
    },
    "responseStyle": {
      "conversational": "Conversationnel",
      "conversationalDesc": "Ton naturel et décontracté",
      "roleplay": "Jeu de rôle",
      "roleplayDesc": "Actions en personnage"
    },
    "extra": {
      "conversational": "Conversationnel",
      "roleplay": "Roleplay"
    }
  },
  "imageGeneration": {
    "promptPlaceholder": "Décrivez l'image que vous voulez générer...",
    "labels": {
      "model": "MODÈLE",
      "prompt": "PROMPT",
      "size": "TAILLE",
      "quality": "QUALITÉ",
      "style": "STYLE",
      "searchModels": "Rechercher des modèles...",
      "selectAvatarModel": "Sélectionnez le modèle d'avatar",
      "selectSceneModel": "Sélectionnez le modèle de scène",
      "selectWriterModel": "Sélectionner le modèle de rédaction de scène",
      "useFirstAvailable": "Utiliser le premier modèle disponible",
      "useFirstCompatible": "Utiliser le premier modèle de rédaction compatible"
    },
    "mode": {
      "title": "Comportement",
      "description": "Choisissez comment traiter les prompts de scène détectés dans la sortie du modèle.",
      "auto": "Automatique",
      "autoDescription": "Générer immédiatement l'image de scène lorsque le modèle fournit un prompt de scène.",
      "askFirst": "Demander avant",
      "askFirstDescription": "Afficher le prompt de scène détecté et attendre votre validation avant de générer une image.",
      "manual": "Manuel",
      "manualDescription": "Ignorer les prompts de scène présents dans les réponses du modèle. Utiliser uniquement les actions lancées manuellement par l'utilisateur."
    },
    "empty": {
      "title": "Aucun modèle d'image",
      "description": "Ajoutez un modèle de génération d'images depuis la page Modèles pour commencer à générer des images."
    },
    "sections": {
      "avatar": {
        "title": "Génération d'avatars",
        "description": "Modèle par défaut utilisé lors de la génération d'avatars à partir du sélecteur d'avatar ou des flux d'images de profil associés."
      },
      "scene": {
        "title": "Génération de scène",
        "description": "Modèle réservé aux images de scène générées à partir du contexte de conversation ou des invites de scène."
      },
      "writer": {
        "title": "Rédacteur de scène",
        "description": "Modèle de texte multimodal réservé à la rédaction d’invites de scène et de descriptions de références visuelles à partir du contexte du chat, des avatars et des images de référence."
      }
    },
    "extra": {
      "avatarGeneration": "Génération d'avatar",
      "sceneGeneration": "Génération de scène",
      "sceneWriter": "Rédacteur de scène"
    }
  },
  "logs": {
    "sectionTitles": {
      "diagnostics": "Diagnostics",
      "generate": "Générer",
      "copy": "Copier"
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
      "testDataGenerators": "Générateurs de données de test",
      "storageMaintenance": "Maintenance du stockage",
      "usageTracking": "Suivi d'utilisation",
      "crashTesting": "Tests de collision",
      "environmentInfo": "Infos d'environnement"
    },
    "testData": {
      "generateCharacter": "Générer un personnage de test",
      "generateCharacterDesc": "Créer un seul personnage de test",
      "generatePersona": "Générer un persona de test",
      "generatePersonaDesc": "Créer un seul persona de test",
      "generateSession": "Générer une session de test",
      "generateSessionDesc": "Créer une session de chat de test avec un personnage existant",
      "generateBulk": "Générer des données de test en masse",
      "generateBulkDesc": "Créer 3 personnages et 2 personas"
    },
    "storageMaintenance": {
      "optimizeDb": "Optimiser la base de données",
      "optimizeDbDesc": "Appliquer les PRAGMAs et exécuter VACUUM (mobile uniquement)",
      "backupLegacy": "Sauvegarder et supprimer les fichiers hérités",
      "backupLegacyDesc": "Déplace le stockage hérité .bin dans un dossier de sauvegarde"
    },
    "usageTracking": {
      "recalculateAll": "Recalculer tous les coûts d'utilisation",
      "recalculateAllDesc": "Récupère les prix et recalcule les coûts pour tous les enregistrements d'utilisation OpenRouter"
    },
    "crashTesting": {
      "forceCrash": "Application crash maintenant",
      "forceCrashDesc": "Termine immédiatement le processus de l'application native pour tester la détection des plantages",
      "forceCrashConfirm": "Cela fera immédiatement planter l'application pour tester le détecteur de crash. Continuer?"
    },
    "environmentInfo": {
      "mode": "Mode",
      "devMode": "Mode développeur",
      "viteVersion": "Version Vite"
    },
    "status": {
      "testCharacterCreated": "✓ Personnage de test créé avec succès",
      "testPersonaCreated": "✓ Persona de test créée avec succès",
      "testSessionCreated": "✓ Session de test créée : {{id}}",
      "generatingBulkData": "Génération de données de test en masse...",
      "bulkDataCreated": "✓ Données de test en masse créées : 3 personnages, 2 personas",
      "creatingBenchmarkChat": "Création du personnage et de la session de référence...",
      "seededBenchmarkReady": "✓ Référence initialisée prête : {{name}} / {{id}}",
      "creatingBenchmarkGroupChat": "Création du chat de groupe de référence...",
      "seededGroupBenchmarkReady": "✓ Référence de groupe initialisée prête : {{id}}",
      "dbOptimized": "✓ Base de données optimisée",
      "recalculatingCosts": "Recalcul des coûts d'utilisation... Cela peut prendre un moment.",
      "toursReset": "✓ Tous les tours guidés réinitialisés — ils s'afficheront à nouveau lors de la prochaine visite",
      "crashingApp": "Plantage de l'application..."
    },
    "errors": {
      "noCharacters": "Aucun personnage disponible. Créez d'abord un personnage de test.",
      "createCharacterFailed": "Échec de la création du personnage de test : {{error}}",
      "createPersonaFailed": "Échec de la création de la persona de test : {{error}}",
      "createSessionFailed": "Échec de la création de la session de test : {{error}}",
      "createBulkFailed": "Échec de la création des données de test en masse : {{error}}",
      "createBenchmarkFailed": "Échec de la création de la session de référence : {{error}}",
      "createGroupBenchmarkFailed": "Échec de la création de la session de groupe de référence : {{error}}",
      "dbOptimizeFailed": "Échec de l'optimisation de la base de données : {{error}}",
      "backupFailed": "Échec de la sauvegarde : {{error}}",
      "openRouterKeyMissing": "Clé API OpenRouter introuvable. Configurez-la dans Paramètres > Fournisseurs d'abord.",
      "recalculationFailed": "Échec du recalcul : {{error}}",
      "resetToursFailed": "Échec de la réinitialisation des tours : {{error}}",
      "crashFailed": "Échec du plantage de l'application : {{error}}"
    },
    "onboarding": {
      "title": "Prise en main",
      "resetTours": "Réinitialiser tous les tours guidés",
      "resetToursDesc": "Efface l'état de visualisation de chaque tour pour qu'ils se rejouent lors de la prochaine visite."
    },
    "benchmarks": {
      "createChat": "Créer un chat de référence initialisé",
      "createChatDesc": "Crée un personnage à mémoire dynamique, une scène de départ et une session de test de continuité de 20 messages, puis l'ouvre.",
      "createGroupChat": "Créer un chat de groupe de référence initialisé",
      "createGroupChatDesc": "Crée un chat de groupe à mémoire dynamique avec trois personnages de référence et 30 messages initiaux, puis l'ouvre."
    },
    "extra": {
      "testCharacter": "Personnage de test",
      "testCharacterDesc": "Un personnage de test créé à des fins de développement.",
      "testScene": "Une scène de test simple pour le développement",
      "testPersona": "Persona de test",
      "testPersonaDesc": "Un persona de test pour le développement",
      "successChar": "✓ Personnage de test créé avec succès",
      "successPersona": "✓ Persona de test créé avec succès",
      "successSession": "✓ Session de test créée : {{id}}",
      "successBulk": "✓ Données de test créées en masse : 3 personnages, 2 personas",
      "errorCharAvailable": "Aucun personnage disponible. Crée d'abord un personnage de test.",
      "generatingBulk": "Génération de données de test en masse..."
    }
  },
  "embeddingDownload": {
    "capacityOptions": {
      "oneK": "1K tokens",
      "oneKDesc": "Idéal pour les réponses rapides",
      "twoK": "2K tokens",
      "twoKDesc": "Performance équilibrée",
      "fourK": "4K tokens",
      "fourKDesc": "Contexte maximum"
    },
    "extra": {
      "status": "Téléchargement..."
    }
  },
  "embeddingTest": {
    "categories": {
      "semanticSimilarity": "Similarité sémantique",
      "dissimilarityCheck": "Vérification de dissimilarité",
      "roleplayContext": "Contexte de jeu de rôle"
    },
    "extra": {
      "placeholder": "Saisis du texte à intégrer..."
    }
  },
  "discovery": {
    "tabs": {
      "forYou": "Pour vous",
      "trending": "Tendances",
      "popular": "Populaire",
      "new": "Nouveau"
    },
    "searchPlaceholder": "Rechercher des personnages...",
    "viewAll": "Voir tout",
    "errorTitle": "Quelque chose s'est mal passé",
    "noCardsFound": "Aucune carte trouvée",
    "sections": {
      "trendingNow": "Tendances actuelles",
      "trendingSubtitle": "Populaires cette semaine",
      "mostPopular": "Les plus populaires",
      "popularSubtitle": "Favoris de la communauté",
      "freshArrivals": "Nouveautés",
      "freshSubtitle": "Récemment ajoutés"
    },
    "browse": {
      "newArrivals": "Nouveautés",
      "freshCharacters": "Personnages récents",
      "noCharactersFound": "Aucun personnage trouvé",
      "noCharactersSubtitle": "Revenez plus tard pour du nouveau contenu"
    },
    "sort": {
      "mostLiked": "Les plus aimés",
      "mostDownloaded": "Les plus téléchargés",
      "mostViewed": "Les plus vus",
      "mostMessages": "Le plus de messages",
      "newestFirst": "Les plus récents",
      "recentlyUpdated": "Récemment mis à jour",
      "nameAZ": "Nom (A-Z)"
    },
    "sortBy": "Trier par",
    "resultsUnit": "personnages",
    "detail": {
      "share": "Partager",
      "nsfwOverlay": "Contenu NSFW",
      "nsfwBadge": "NSFW",
      "originalBadge": "Original",
      "lorebookBadge": "Encyclopédie",
      "alsoKnownAs": "Aussi connu sous :",
      "followersUnit": "abonnés",
      "sections": {
        "description": "Description",
        "tokenUsage": "Utilisation des tokens",
        "startingScenes": "Scènes de départ",
        "scenario": "Scénario",
        "personality": "Personnalité",
        "stats": "Statistiques",
        "tags": "Tags",
        "author": "Auteur"
      },
      "tokensTotalLabel": "total",
      "tokens": {
        "description": "Description",
        "personality": "Personnalité",
        "scenario": "Scénario",
        "firstMessage": "Premier message",
        "scenes": "Scènes",
        "examples": "Exemples",
        "systemPrompt": "Prompt système"
      },
      "sceneLabels": {
        "primary": "Principal",
        "alternate": "Alternatif"
      },
      "stats": {
        "views": "Vues",
        "downloads": "Téléchargements",
        "messages": "Messages"
      },
      "downloaded": "Téléchargé",
      "startChat": "Démarrer la discussion",
      "downloadCharacter": "Télécharger le personnage",
      "downloading": "Téléchargement...",
      "downloadSuccess": {
        "title": "Personnage téléchargé !",
        "subtitle": "Ajouté à votre bibliothèque",
        "badge": "Enregistré",
        "startChat": "Démarrer la discussion",
        "startChatDesc": "Ouvrir la première scène maintenant",
        "viewLibrary": "Voir dans la bibliothèque",
        "viewLibraryDesc": "Modifier, gérer ou exporter plus tard",
        "continueBrowsing": "Continuer à parcourir",
        "continueBrowsingDesc": "Retour à la découverte"
      },
      "errorTitle": "Erreur",
      "errorSubtitle": "Échec du chargement",
      "errorNotFound": "Personnage introuvable",
      "defaultChatTitle": "Nouveau chat"
    },
    "search": {
      "placeholder": "Rechercher des personnages, tags, auteurs...",
      "resultsUnit": "résultats",
      "timingUnit": "ms",
      "recentSearches": "Recherches récentes",
      "clearAll": "Tout effacer",
      "trendingSearches": "Recherches tendance",
      "trends": {
        "anime": "anime",
        "fantasy": "fantaisie",
        "romance": "romance",
        "villain": "méchant",
        "adventure": "aventure",
        "comedy": "comédie",
        "mystery": "mystère",
        "sciFi": "sci-fi"
      },
      "tips": {
        "title": "Astuces de recherche",
        "tip1": "Recherchez par nom de personnage, auteur ou description",
        "tip2": "Utilisez des tags comme \"anime\", \"fantaisie\" ou \"romance\"",
        "tip3": "Essayez des traits spécifiques comme \"tsundere\" ou \"méchant\""
      },
      "loading": "Chargement...",
      "loadMore": "Charger plus",
      "noResults": "Aucun résultat trouvé",
      "noResultsFor": "Aucun personnage trouvé pour",
      "noResultsHint": "Essayez d'autres mots-clés ou parcourez les catégories"
    },
    "errors": {
      "loadContent": "Échec du chargement du contenu",
      "searchFailed": "Échec de la recherche",
      "noCardPath": "Aucun chemin de carte fourni",
      "loadCharacter": "Échec du chargement du personnage",
      "downloadCharacter": "Échec du téléchargement du personnage"
    },
    "card": {
      "byAuthor": "par {{author}}",
      "ocBadge": "OC"
    }
  },
  "engine": {
    "gpuInsufficient": "Mémoire GPU insuffisante",
    "gpuFallbackDesc": "Ce modèle ne tient pas dans la mémoire GPU. Passer au CPU (plus lent) ou abandonner ?",
    "switchToCpu": "Passer au CPU",
    "abort": "Abandonner",
    "errors": {
      "providerNotFound": "Fournisseur du moteur introuvable.",
      "engineOffline": "Le moteur est hors ligne ou injoignable.",
      "deleteCharacterFailed": "Échec de la suppression du personnage.",
      "unknownCharacter": "Inconnu",
      "seedRequired": "La description seed est requise.",
      "characterNameRequired": "Le nom du personnage est requis.",
      "atLeastOneProvider": "Au moins un fournisseur doit être activé.",
      "enableLlmProvider": "Active au moins un fournisseur LLM.",
      "modelRequired": "Le modèle est requis pour {{provider}}.",
      "apiKeyRequired": "La clé API est requise pour {{provider}}.",
      "sendMessageFailed": "Échec de l'envoi du message."
    },
    "status": {
      "connected": "Connecté",
      "offline": "Hors ligne",
      "needsSetup": "Configuration nécessaire"
    },
    "home": {
      "characters": "Personnages",
      "newButton": "Nouveau",
      "noCharactersFound": "Aucun personnage trouvé.",
      "tokenUsage": "Utilisation des tokens",
      "totalTokens": "tokens au total",
      "backgroundActivity": "Activité en arrière-plan",
      "quickActions": "Actions rapides",
      "configureProviders": "Configurer les fournisseurs",
      "engineSettings": "Paramètres du moteur",
      "chat": "Discuter",
      "chatDesc": "Démarrer une conversation avec ce personnage",
      "deleteCharacter": "Supprimer le personnage",
      "deletingCharacter": "Suppression...",
      "deleteDesc": "Supprimer définitivement ce personnage",
      "character": "Personnage",
      "never": "Jamais",
      "justNow": "À l'instant",
      "timeAgo": {
        "minutes": "il y a {{n}} min",
        "hours": "il y a {{n}} h",
        "days": "il y a {{n}} j"
      }
    },
    "tokens": {
      "input": "Entrée",
      "output": "Sortie"
    },
    "activity": {
      "synthesis": "Synthèse",
      "consolidation": "Consolidation",
      "bm25Rebuild": "Reconstruction BM25",
      "dripResearch": "Recherche progressive",
      "running": "En cours",
      "stopped": "Arrêté"
    },
    "setup": {
      "complete": "Configuration terminée !",
      "completeMessage": "Votre moteur Lettuce est configuré et prêt à fonctionner.",
      "openDashboard": "Ouvrir le tableau de bord"
    },
    "welcome": {
      "title": "Bienvenue dans le moteur Lettuce",
      "subtitle": "Configurons votre moteur de personnages IA. Cela prendra environ 2 minutes.",
      "feature1": "Le moteur donne à vos personnages IA une mémoire persistante, des émotions, des relations et une véritable identité.",
      "feature2": "D'abord, nous configurerons un backend LLM, puis les paramètres du moteur.",
      "getStarted": "C'est parti"
    },
    "config": {
      "activeProviders": "Fournisseurs actifs",
      "noModelSet": "Aucun modèle défini",
      "defaultBadge": "Par défaut",
      "noProvidersWarning": "Aucun fournisseur configuré. Ajoutez au moins un backend LLM ci-dessous.",
      "addProvider": "Ajouter un fournisseur",
      "quickImport": "Importation rapide depuis vos fournisseurs d'application",
      "importButton": "Importer",
      "fields": {
        "model": "Modèle",
        "modelPlaceholder": "ex. claude-sonnet-4-5-20250929",
        "apiKey": "Clé API",
        "apiKeyPlaceholder": "Entrez votre clé API",
        "currentKey": "Clé actuelle :",
        "baseUrl": "URL de base",
        "baseUrlPlaceholder": "http://localhost:11434",
        "maxTokens": "Tokens max.",
        "temperature": "Température"
      },
      "enableProvider": "Activer le fournisseur",
      "setAsDefault": "Définir comme par défaut",
      "defaultBackend": "Backend par défaut",
      "remove": "Supprimer",
      "saveChanges": "Enregistrer les modifications",
      "saving": "Enregistrement...",
      "saved": "Enregistré"
    },
    "providers": {
      "title": "Fournisseur LLM",
      "subtitle": "Le moteur a besoin d'au moins un backend LLM pour fonctionner. Configurez un ou plusieurs fournisseurs ci-dessous.",
      "importFromProviders": "Importer depuis vos fournisseurs",
      "imported": "Importé",
      "use": "Utiliser",
      "saveContinue": "Enregistrer et continuer"
    },
    "settings": {
      "fields": {
        "dataDirectory": "Répertoire de données",
        "logLevel": "Niveau de journalisation",
        "maxHistory": "Historique max. (tours de conversation)"
      },
      "logLevels": {
        "debug": "DÉBOGAGE",
        "info": "INFO",
        "warning": "AVERTISSEMENT",
        "error": "ERREUR"
      },
      "sections": {
        "engine": "Moteur",
        "backgroundLoops": "Boucles en arrière-plan",
        "memory": "Mémoire",
        "safety": "Sécurité",
        "research": "Recherche"
      },
      "backgroundLoops": {
        "synthesis": "Synthèse (min)",
        "consolidation": "Consolidation (min)",
        "bm25Rebuild": "Reconstruction BM25 (min)",
        "dripResearch": "Recherche progressive (min)"
      },
      "memory": {
        "embeddingModel": "Modèle d'embedding",
        "maxRetrieval": "Résultats max. de récupération",
        "denseWeight": "Poids dense",
        "bm25Weight": "Poids BM25",
        "graphWeight": "Poids du graphe",
        "recencyBoost": "Bonus de récence (heures)",
        "randomSurface": "Probabilité de surface aléatoire"
      },
      "safety": {
        "honestySection": "Section honnêteté",
        "honestyDesc": "Inclure la section honnêteté dans le prompt système",
        "userDataDeletion": "Suppression des données utilisateur",
        "userDataDesc": "Permettre aux utilisateurs de demander la suppression de leurs données"
      },
      "research": {
        "scrapeOnBoot": "Scraper au démarrage",
        "scrapeDesc": "Exécuter le scraping de recherche au démarrage du moteur",
        "periodicInterval": "Intervalle périodique (heures)"
      },
      "saveChanges": "Enregistrer les modifications",
      "saving": "Enregistrement...",
      "saved": "Enregistré"
    },
    "settingsStep": {
      "title": "Paramètres du moteur",
      "subtitle": "Configurez les paramètres du moteur. Tous ont des valeurs par défaut raisonnables — n'hésitez pas à passer.",
      "completingSetup": "Finalisation de la configuration...",
      "completeSetup": "Terminer la configuration"
    },
    "chat": {
      "sendMessage": "Envoyer un message...",
      "sendButton": "Envoyer le message",
      "typeMessage": "Tapez un message",
      "back": "Retour",
      "assistantTyping": "L'assistant écrit",
      "fallbackName": "Chat"
    },
    "tagInput": {
      "addMore": "Ajouter plus...",
      "typeAndPressEnter": "Tapez et appuyez sur Entrée"
    },
    "characterCreate": {
      "steps": {
        "identity": {
          "title": "Identité",
          "aiGenerated": "Généré par l'IA",
          "nameLabel": "Nom *",
          "namePlaceholder": "Nom du personnage",
          "eraLabel": "Époque",
          "eraPlaceholder": "ex. moderne, victorien",
          "roleLabel": "Rôle",
          "rolePlaceholder": "ex. Détective, Scientifique",
          "settingLabel": "Cadre",
          "settingPlaceholder": "Décrivez où vit le personnage (première personne)...",
          "coreIdentityLabel": "Identité fondamentale",
          "coreIdentityPlaceholder": "Qui est ce personnage en son cœur ? (première personne, 3-5 phrases)",
          "backstoryLabel": "Passé",
          "backstoryPlaceholder": "Histoire de vie et événements clés (première personne)..."
        },
        "mode": {
          "title": "Créer un personnage",
          "subtitle": "Générez un personnage avec l'IA ou construisez-en un de zéro.",
          "aiBoost": "Boost IA",
          "aiBoostDesc": "Décrivez votre idée de personnage et l'IA générera une définition complète.",
          "nameOptional": "Nom (facultatif)",
          "namePlaceholder": "ex. Marcus Cole",
          "seedDescription": "Description de base *",
          "seedPlaceholder": "ex. pianiste de jazz dans le Harlem des années 1950, philosophe, aime les conversations tard le soir",
          "eraOptional": "Époque (facultatif)",
          "eraPlaceholder": "ex. années 1950, moderne, victorien",
          "generating": "Génération...",
          "generateCharacter": "Générer le personnage",
          "or": "ou",
          "startFromScratch": "Partir de zéro"
        },
        "personality": {
          "title": "Personnalité",
          "traits": "Traits de personnalité",
          "traitsPlaceholder": "ex. spirituel, compatissant, têtu",
          "speechPatterns": "Styles de parole",
          "formality": "Formalité",
          "formal": "Formel",
          "casual": "Décontracté",
          "texting": "SMS",
          "verbosity": "Verbosité",
          "terse": "Concis",
          "medium": "Moyen",
          "verbose": "Verbeux",
          "textStyle": "Style de texte",
          "dialect": "Dialecte",
          "dialectPlaceholder": "ex. Marseillais, Parisien",
          "catchphrases": "Expressions fétiches",
          "catchphrasesPlaceholder": "ex. Ma parole...",
          "vocabPreferences": "Préférences de vocabulaire",
          "vocabPreferencesPlaceholder": "Mots qu'il/elle privilégie",
          "vocabAvoidances": "Vocabulaire évité",
          "vocabAvoidancesPlaceholder": "Mots qu'il/elle évite",
          "fillerWords": "Mots de remplissage",
          "fillerWordsPlaceholder": "ex. euh, genre, tu vois",
          "exampleQuotes": "Citations d'exemple",
          "exampleQuotesPlaceholder": "3-5 exemples de dialogues"
        },
        "world": {
          "title": "Monde et comportement",
          "knowledgeDomains": "Domaines de connaissance",
          "knowledgeDomainsPlaceholder": "ex. histoire du jazz, théorie musicale",
          "knowledgeBoundaries": "Limites de connaissance",
          "knowledgeBoundariesPlaceholder": "Sujets qu'il/elle ne connaît pas",
          "researchSeeds": "Graines de recherche",
          "researchSeedsPlaceholder": "Sujets de départ pour la recherche en arrière-plan",
          "researchEnabled": "Recherche activée",
          "researchEnabledDesc": "Permettre la collecte de connaissances en arrière-plan",
          "physicalDescription": "Description physique",
          "physicalDescPlaceholder": "Apparence physique et manières...",
          "physicalHabits": "Habitudes physiques",
          "physicalHabitsPlaceholder": "ex. tapote des doigts, ajuste ses lunettes",
          "idleBehaviors": "Comportements au repos",
          "idleBehaviorsPlaceholder": "Ce qu'il/elle fait quand il/elle n'est pas sollicité(e)",
          "timeBehaviors": "Comportements temporels",
          "timePlaceholder": "Que fait-il/elle pendant {{period}} ?",
          "earlyMorning": "Petit matin",
          "morning": "Matin",
          "afternoon": "Après-midi",
          "evening": "Soirée",
          "night": "Nuit",
          "baselineEmotions": "Émotions de base (Plutchik)",
          "emotionDesc": "Définir la base émotionnelle par défaut (0 = aucun, 1 = maximum)",
          "joy": "Joie",
          "trust": "Confiance",
          "fear": "Peur",
          "surprise": "Surprise",
          "sadness": "Tristesse",
          "disgust": "Dégoût",
          "anger": "Colère",
          "anticipation": "Anticipation",
          "engineOverrides": "Remplacements du moteur",
          "backend": "Backend",
          "model": "Modèle",
          "temperature": "Température",
          "leaveEmpty": "Laisser vide pour la valeur par défaut"
        },
        "review": {
          "title": "Vérification",
          "subtitle": "Vérifiez votre personnage avant de le créer.",
          "edit": "Modifier",
          "notSet": "Non défini",
          "identitySection": "Identité",
          "personalitySection": "Personnalité",
          "worldSection": "Monde et comportement",
          "nameLabel": "Nom",
          "eraLabel": "Époque",
          "roleLabel": "Rôle",
          "settingLabel": "Cadre",
          "coreIdentityLabel": "Identité fondamentale",
          "backstoryLabel": "Passé",
          "traitsLabel": "Traits",
          "formalityLabel": "Formalité",
          "verbosityLabel": "Verbosité",
          "dialectLabel": "Dialecte",
          "catchphrasesLabel": "Expressions fétiches",
          "domainsLabel": "Domaines",
          "boundariesLabel": "Limites",
          "researchSeedsLabel": "Graines de recherche",
          "researchLabel": "Recherche",
          "enabled": "Activé",
          "disabled": "Désactivé",
          "physicalLabel": "Physique",
          "habitsLabel": "Habitudes",
          "idleLabel": "Au repos",
          "timeBehaviorsLabel": "Comportements temporels",
          "emotionsLabel": "Émotions",
          "configured": "Configuré",
          "backendLabel": "Backend",
          "modelLabel": "Modèle",
          "temperatureLabel": "Température",
          "creating": "Création...",
          "createCharacter": "Créer le personnage"
        }
      }
    }
  },
  "library": {
    "filterTitle": "Filtrer la bibliothèque",
    "filters": {
      "all": "Tous",
      "characters": "Personnages",
      "personas": "Personas",
      "lorebooks": "Encyclopédies",
      "images": "Images"
    },
    "emptyStates": {
      "all": {
        "title": "Votre bibliothèque est vide",
        "description": "Créez des personnages, personas et encyclopédies pour les voir ici"
      },
      "characters": {
        "title": "Pas encore de personnages",
        "description": "Créez votre premier personnage pour commencer à discuter"
      },
      "personas": {
        "title": "Pas encore de personas",
        "description": "Créez un persona pour personnaliser votre identité de chat"
      },
      "lorebooks": {
        "title": "Pas encore d'encyclopédies",
        "description": "Les encyclopédies sont créées dans les paramètres d'un personnage"
      }
    },
    "actions": {
      "startChat": "Démarrer la discussion",
      "editCharacter": "Modifier le personnage",
      "editPersona": "Modifier le persona",
      "editLorebook": "Modifier l'encyclopédie",
      "renameLorebook": "Renommer l'encyclopédie",
      "exportCharacter": "Exporter le personnage",
      "exportPersona": "Exporter le persona",
      "chatAppearance": "Apparence du chat",
      "deleteCharacter": "Supprimer le personnage",
      "deletePersona": "Supprimer le persona",
      "deleteLorebook": "Supprimer l'encyclopédie",
      "importLorebook": "Importer une encyclopédie"
    },
    "imageLibrary": {
      "filters": {
        "all": "Tout",
        "backgrounds": "Arrière-plans",
        "avatars": "Avatars",
        "attachments": "Pièces jointes",
        "other": "Autres"
      },
      "searchPlaceholder": "Rechercher par nom de fichier, chemin, identifiant de session ou d'entité",
      "empty": {
        "title": "Aucune image ne correspond à cette vue",
        "description": "Essayez un autre filtre ou terme de recherche. La bibliothèque n'affiche que les images déjà stockées localement dans l'application."
      },
      "actions": {
        "sort": "Trier",
        "useThis": "Utiliser cette image",
        "using": "Utilisation...",
        "copyPath": "Copier le chemin",
        "saving": "Enregistrement...",
        "download": "Télécharger",
        "delete": "Supprimer l'image",
        "deleting": "Suppression..."
      },
      "active": "Actif",
      "messages": {
        "loadFailed": "Impossible de charger la bibliothèque d'images",
        "saved": "Image enregistrée",
        "downloadFailed": "Échec du téléchargement",
        "useFailed": "Impossible d'utiliser cette image",
        "deleted": "Image supprimée",
        "deleteFailed": "Impossible de supprimer l'image"
      },
      "deleteConfirm": {
        "title": "Supprimer l'image ?",
        "message": "Voulez-vous vraiment supprimer \"{{filename}}\" ? Cela peut casser des avatars, des arrière-plans de chat ou des pièces jointes de message qui l'utilisent encore."
      },
      "sort": {
        "newest": "Plus récents",
        "largest": "Plus grands",
        "name": "Nom"
      },
      "kinds": {
        "background": "Arrière-plan",
        "avatar": "Avatar",
        "attachment": "Pièce jointe",
        "stored": "Stocké"
      },
      "detailsTitle": "Détails de {{kind}}",
      "formatsLabel": "Formats",
      "storagePath": "Chemin de stockage",
      "contextLabel": "Contexte",
      "contextLinkedFallback": "Lié",
      "show": "Afficher",
      "hide": "Masquer",
      "contextRoles": {
        "character": "personnage :",
        "session": "session :",
        "role": "rôle :"
      },
      "downloadFormat": "Format {{download}}",
      "unknownDate": "Inconnu",
      "clearSearch": "Effacer la recherche",
      "copyFilename": "Copier le nom de fichier",
      "copyLabels": {
        "filename": "Nom de fichier",
        "storagePath": "Chemin de stockage"
      },
      "copy": {
        "copied": "{{label}} copié",
        "failed": "Échec de la copie de {{label}}"
      }
    },
    "deleteConfirm": {
      "title": "Supprimer {{itemType}} ?",
      "message": "Êtes-vous sûr de vouloir supprimer",
      "characterWarning": "Cela supprimera également toutes les sessions de chat avec ce personnage."
    },
    "rename": {
      "title": "Renommer l'encyclopédie",
      "placeholder": "Entrez le nouveau nom..."
    },
    "itemTypes": {
      "character": "Personnage",
      "persona": "Persona",
      "lorebook": "Encyclopédie"
    },
    "lorebookLabel": "Encyclopédie",
    "noDescriptionYet": "Pas encore de description",
    "errors": {
      "importLorebook": "Échec de l'import du lorebook. {{error}}",
      "exportFailed": "Échec de l'export"
    },
    "card": {
      "avatarAlt": "Avatar de {{name}}"
    },
    "lorebookEditor": {
      "titleOverride": "Lorebook : {{name}}",
      "dragToReorder": "Glisse pour réordonner",
      "aria": {
        "generateEntry": "Générer une entrée de lorebook",
        "editLorebook": "Modifier le lorebook",
        "exportLorebook": "Exporter le lorebook"
      }
    }
  },
  "onboarding": {
    "loading": "Chargement des fournisseurs...",
    "stepIndicator": "Étape {{current}} sur {{total}}",
    "steps": {
      "provider": "Configuration du fournisseur",
      "model": "Configuration du modèle",
      "memory": "Système de mémoire",
      "stepNofM": "Étape {{current}} sur {{total}}"
    },
    "provider": {
      "availableProviders": "Fournisseurs disponibles",
      "chooseProvider": "Choisissez un fournisseur",
      "titleMobile": "Choisis ton fournisseur d'IA",
      "descMobile": "Sélectionne un fournisseur d'IA pour commencer. Tes clés API sont chiffrées en sécurité sur ton appareil. Aucun compte requis.",
      "configureProvider": "Configurer {{name}}",
      "connectProvider": "Connecter {{name}}",
      "connectProviderDesc": "Colle ta clé API ci-dessous pour activer les chats. Besoin d'une clé ? Récupère-la depuis le tableau de bord du fournisseur.",
      "localLLMs": "LLMs locaux",
      "useLocalLLMs": "Je veux utiliser des LLMs locaux",
      "browseModelLibrary": "Parcourir la bibliothèque de modèles",
      "browseModelLibraryDesc": "Cherche et télécharge des modèles GGUF depuis HuggingFace",
      "useOwnGguf": "Utiliser mes propres fichiers GGUF",
      "useOwnGgufDesc": "Sélectionne un modèle GGUF et un fichier mmproj optionnel depuis ton appareil",
      "fields": {
        "displayLabel": "Libellé d'affichage",
        "displayLabelHint": "Comment ce fournisseur apparaîtra dans tes menus",
        "displayLabelPlaceholder": "Mon {{name}}",
        "defaultLabelFallback": "Fournisseur",
        "apiKey": "Clé API",
        "apiKeyOptional": "Clé API (Optionnelle)",
        "apiKeyHint": "Les clés sont chiffrées localement",
        "apiKeyPlaceholderRemote": "sk-...",
        "apiKeyPlaceholderLocal": "Généralement non requise",
        "whereToFind": "Où la trouver",
        "baseUrl": "URL de base",
        "baseUrlPlaceholderHost": "http://192.168.1.10:3333",
        "baseUrlPlaceholderLocal": "http://localhost:11434",
        "baseUrlPlaceholderRemote": "https://api.provider.com",
        "baseUrlPlaceholderIntenseRP": "http://127.0.0.1:7777/v1",
        "baseUrlHintLocal": "L'adresse de ton serveur local avec le port",
        "baseUrlHintHost": "Saisis l'URL hôte du desktop affichée par ton appareil hôte",
        "baseUrlHintRemote": "Surcharge l'endpoint par défaut si nécessaire",
        "chatEndpoint": "Endpoint de chat",
        "systemRole": "Rôle système",
        "userRole": "Rôle utilisateur",
        "assistantRole": "Rôle assistant",
        "supportsStreaming": "Supporte le streaming",
        "mergeSameRole": "Fusionner les messages du même rôle",
        "toolChoiceMode": "Mode tool_choice",
        "toolChoiceHint": "Contrôle la façon dont tool_choice est envoyé à l'endpoint personnalisé."
      },
      "toolChoice": {
        "auto": "Auto",
        "required": "Requis",
        "none": "Aucun",
        "omit": "Omettre le champ",
        "passthrough": "Passthrough (Tool Config)"
      },
      "buttons": {
        "testConnection": "Tester la connexion",
        "testing": "Test en cours..."
      },
      "descriptions": {
        "chutes": "Inférence compatible OpenAI pour les meilleurs modèles open-source",
        "openai": "Modèles GPT-5, GPT-4.1 et GPT-4o pour un RP expressif",
        "lettuceHost": "Connecte-toi à ton propre Lettuce Host desktop sur LAN avec une API style OpenAI",
        "anthropic": "Claude 4.5 Sonnet & Haiku pour des dialogues profonds et émotionnels",
        "aggregator": "Accède à des modèles comme GPT-5, Claude 4.5, Grok-3, Mixtral, et plus",
        "openaiCompatible": "Utilise n'importe quel endpoint API style OpenAI",
        "mistral": "Mistral Small 3.2, Mixtral 8x22B et autres modèles Mistral",
        "deepseek": "DeepSeek-V3.2-Exp, DeepSeek-R1 et autres modèles à haute efficacité",
        "xai": "Grok-1.5, Grok-3 et les modèles xAI plus récents",
        "zai": "GLM-4.5, GLM-4.6 et les variantes Air",
        "moonshot": "Modèles Kimi-K2 Thinking et Kimi-K1",
        "gemini": "Gemini 2.5 Flash, 2.5 Pro et plus",
        "qwen": "Qwen3-VL et les modèles Qwen plus récents",
        "nvidia": "Nemotron, Llama, DeepSeek et plus via NVIDIA NIM",
        "custom": "Pointe LettuceAI vers n'importe quel endpoint de modèle personnalisé",
        "fallback": "Fournisseur de modèle d'IA"
      },
      "descriptionsShort": {
        "chutes": "Inférence de modèles open-source",
        "openai": "GPT-5, GPT-4o, GPT-4.1",
        "lettuceHost": "Ton propre hôte LAN",
        "anthropic": "Claude 4.5 Sonnet & Haiku",
        "aggregator": "Agrégateur multi-modèles",
        "openaiCompatible": "Endpoint OpenAI personnalisé",
        "mistral": "Modèles Mistral & Mixtral",
        "deepseek": "DeepSeek-V3, R1",
        "xai": "Grok-1.5, Grok-3",
        "zai": "GLM-4.5, GLM-4.6",
        "moonshot": "Kimi-K2 Thinking",
        "gemini": "Gemini 2.5 Flash & Pro",
        "qwen": "Modèles Qwen3-VL",
        "nvidia": "Inférence NVIDIA NIM",
        "custom": "Endpoint personnalisé",
        "fallback": "Fournisseur d'IA"
      },
      "cardLabel": "{{step}} : {{name}}",
      "errors": {
        "hostUrlRequired": "L'URL hôte est requise (ex. http://192.168.1.10:3333)",
        "baseUrlRequired": "L'URL de base est requise (ex. http://localhost:11434)",
        "apiKeyTooShort": "La clé API semble trop courte",
        "invalidApiKey": "Clé API invalide",
        "connectionFailed": "Échec de la connexion",
        "verificationFailed": "Échec de la vérification",
        "failedToSave": "Échec de l'enregistrement du fournisseur",
        "connectionSuccessful": "Connexion réussie !",
        "modelNotFound": "Modèle introuvable chez le fournisseur",
        "modelVerificationFailed": "Échec de la vérification du modèle",
        "failedToSaveModel": "Échec de l'enregistrement du modèle"
      }
    },
    "model": {
      "noProvidersTitle": "Aucun fournisseur configuré",
      "noProvidersDesc": "Vous devrez connecter un fournisseur avant de choisir un modèle par défaut.",
      "goToProviderSetup": "Aller à la configuration du fournisseur",
      "yourProviders": "Tes fournisseurs",
      "yourProvidersHint": "Sélectionne quel fournisseur utiliser",
      "setDefaultModel": "Définis ton modèle par défaut",
      "setDefaultModelDesc": "Choisis le fournisseur et le nom du modèle que LettuceAI doit utiliser par défaut. Tu pourras en ajouter d'autres plus tard.",
      "setDefaultModelDescDesktop": "Sélectionne un fournisseur dans la liste pour configurer ton modèle.",
      "modelDetails": "Détails du modèle",
      "modelDetailsDesc": "Définis l'identifiant API et le libellé que tu verras dans l'app.",
      "whichModel": "Quel modèle dois-je utiliser ?",
      "nextMemorySystem": "Suivant : Système de mémoire",
      "fields": {
        "displayName": "Nom d'affichage",
        "displayNamePlaceholder": "Mentor créatif",
        "displayNameHint": "Comment ce modèle apparaît dans les menus",
        "modelId": "ID du modèle",
        "modelPathGguf": "Chemin du modèle (GGUF)",
        "modelIdPlaceholder": "ex. gpt-4o",
        "modelPathPlaceholder": "/chemin/vers/modele.gguf",
        "modelIdHint": "Identifiant exact utilisé pour les appels API",
        "showList": "Afficher la liste",
        "manualInput": "Saisie manuelle",
        "refreshModelList": "Actualiser la liste des modèles",
        "selectModel": "Sélectionner un modèle",
        "selectAModel": "Sélectionne un modèle...",
        "searchModels": "Rechercher des modèles...",
        "noModelsFound": "Aucun modèle ne correspond à \"{{query}}\""
      },
      "fillBothFields": "Remplis les deux champs ci-dessus pour activer le bouton de fin.",
      "providerNames": {
        "chutes": "Chutes",
        "intenseRpNext": "IntenseRP Next",
        "openai": "OpenAI",
        "anthropic": "Anthropic",
        "openrouter": "OpenRouter",
        "openaiCompatible": "Compatible OpenAI",
        "custom": "Endpoint personnalisé"
      }
    },
    "memory": {
      "dynamicTitle": "Mémoire dynamique",
      "recommended": "Recommandé",
      "settingUp": "Configuration...",
      "finishSetup": "Terminer la configuration",
      "promptTitle": "Configurer la mémoire dynamique",
      "oneLastStep": "Une dernière étape",
      "downloadAndEnable": "Télécharger et activer",
      "chooseStyle": "Choisis ton style de mémoire",
      "howRemember": "Comment tes compagnons d'IA doivent-ils se rappeler des détails sur toi et tes conversations ?",
      "dynamicDescription": "Utilise un <0>modèle d'embedding local</0> pour gérer intelligemment le contexte. Cela réduit les coûts en tokens tout en maintenant une haute qualité, même dans les longs chats.",
      "dynamicFeatures": {
        "quality": "Maintient la qualité dans les longs chats",
        "cost": "Réduit considérablement les coûts d'API",
        "auto": "Gestion automatique du contexte",
        "zeroConfig": "Aucune configuration nécessaire"
      },
      "manualTitle": "Mémoire manuelle",
      "manualBadge": "Expérience classique",
      "manualDescription": "Tu épingles explicitement les messages et modifies toi-même la \"World Info\" ou les définitions de personnage. Bien pour un contrôle total.",
      "manualFeatures": {
        "control": "Contrôle total sur les faits",
        "scenarios": "Idéal pour des scénarios spécifiques"
      },
      "setupModelMessage": "Pour utiliser la mémoire dynamique, on doit télécharger un petit modèle d'embedding (~120 Mo) sur ton appareil.",
      "setupBullets": {
        "offline": "Le modèle tourne 100 % hors ligne sur ton appareil",
        "remembering": "Requis pour se rappeler du contexte",
        "disable": "Tu pourras désactiver ceci plus tard dans les paramètres"
      },
      "stepLabel": "Étape 3 sur 3",
      "stepLabelMemory": "Système de mémoire"
    },
    "welcome": {
      "appName": "LettuceAI",
      "tagline": "Votre compagnon IA personnel. Privé, sécurisé, et toujours sur l'appareil.",
      "features": {
        "onDevice": "Sur l'appareil uniquement",
        "characterReady": "Personnage prêt"
      },
      "betaWarning": {
        "title": "Version bêta bureau",
        "description": "Vous utilisez la version bureau. Certaines fonctionnalités peuvent différer du mobile. Signalez les problèmes sur GitHub."
      },
      "languageSelector": {
        "title": "Langue",
        "description": "Détectée automatiquement depuis votre appareil. Vous pouvez la modifier à tout moment dans les paramètres."
      },
      "getStarted": "Commencer",
      "skipForNow": "Passer pour l'instant",
      "restoreFromBackup": "Restaurer depuis une sauvegarde",
      "setupTime": "La configuration prend moins de 2 minutes",
      "skipWarning": {
        "title": "Passer la configuration ?",
        "warningTitle": "Fournisseur nécessaire pour discuter",
        "warningMessage": "Sans fournisseur, vous ne pourrez pas envoyer de messages. Vous pourrez en ajouter un plus tard dans les paramètres.",
        "addProvider": "Ajouter un fournisseur",
        "skipAnyway": "Passer quand même"
      },
      "restoreBackup": {
        "title": "Restaurer une sauvegarde",
        "selectMessage": "Sélectionnez une sauvegarde à restaurer.",
        "browse": "Parcourir les fichiers",
        "processing": "Traitement du fichier...",
        "processingNote": "Les grandes sauvegardes peuvent prendre une minute",
        "noBackups": "Aucune sauvegarde trouvée",
        "noBackupsHint": "Appuyez sur parcourir pour sélectionner un fichier .lettuce",
        "browseLettuce": "Rechercher un fichier .lettuce",
        "passwordLabel": "Mot de passe de la sauvegarde",
        "passwordPlaceholder": "Entrez le mot de passe",
        "restoreButton": "Restaurer la sauvegarde",
        "restoring": "Restauration...",
        "infoMessage": "Cela configurera l'application avec vos données sauvegardées, y compris les personnages, les discussions et les paramètres.",
        "embeddingTitle": "Modèle d'embedding requis",
        "dynamicMemoryDetected": "Mémoire dynamique détectée",
        "dynamicMemoryMessage": "Cette sauvegarde contient des personnages avec la mémoire dynamique activée, ce qui nécessite le modèle d'embedding (~120 Mo).",
        "embeddingOptions": "Vous pouvez télécharger le modèle maintenant pour activer la mémoire dynamique, ou continuer sans (la mémoire dynamique sera désactivée pour les personnages concernés).",
        "downloadModel": "Télécharger le modèle",
        "continueWithoutDynamic": "Continuer sans mémoire dynamique",
        "embeddingNote": "Vous pourrez réactiver la mémoire dynamique plus tard dans les paramètres du personnage après avoir téléchargé le modèle.",
        "back": "Retour",
        "cancel": "Annuler",
        "errors": {
          "passwordRequired": "Le mot de passe est requis",
          "incorrectPassword": "Mot de passe incorrect",
          "failedToOpenFile": "Échec de l'ouverture du fichier",
          "failedToRestore": "Échec de la restauration de la sauvegarde",
          "failedToUpdateSettings": "Échec de la mise à jour des paramètres"
        }
      }
    },
    "common": {
      "back": "Retour",
      "cancel": "Annuler",
      "continue": "Continuer",
      "verifying": "Vérification...",
      "skipForNow": "Passer pour le moment",
      "selectAProvider": "Sélectionne un fournisseur à configurer",
      "clickToSelectProvider": "Clique pour sélectionner un fournisseur",
      "selectProviderFromList": "Sélectionne un fournisseur dans la liste pour commencer.",
      "enterApiKey": "Saisis ta clé API pour activer la fonctionnalité de chat IA."
    },
    "modelGuide": {
      "badge": "Guide des modèles",
      "title": "Comment choisir un modèle ?",
      "intro": "LettuceAI ne force pas un seul \"meilleur\" modèle. À la place, tu choisis ce qui correspond à ton <0>cas d'usage, budget et ambiance</0>. Utilise ce guide pour décider quoi essayer et où chercher.",
      "askYourself": "Pose-toi la question :",
      "factors": {
        "quality": {
          "title": "Qualité et capacités",
          "description": "À quel point le modèle doit-il être intelligent ? Les modèles plus gros et plus récents raisonnent généralement mieux, écrivent un texte plus agréable et gèrent les prompts désordonnés avec plus d'élégance.",
          "q1": "As-tu besoin d'une cohérence de personnage profonde et d'intelligence émotionnelle ?",
          "q2": "Tiens-tu à une narration immersive et à des personnalités de personnage crédibles ?",
          "q3": "Veux-tu que le modèle se rappelle des détails du personnage et reste dans le rôle sur de longues sessions ?"
        },
        "speed": {
          "title": "Vitesse et latence",
          "description": "Les modèles plus rapides sont plus agréables pour les conversations bavardes et les échanges. Certains modèles troquent un peu de qualité contre beaucoup plus de vitesse.",
          "q1": "Veux-tu des réponses quasi instantanées pour garder le RP fluide ?",
          "q2": "Fais-tu des scènes de dialogue rapide où l'attente briserait l'immersion ?",
          "q3": "Est-ce pour du RP décontracté où les échanges rapides comptent plus que les réponses parfaites ?"
        },
        "budget": {
          "title": "Budget et utilisation",
          "description": "Chaque fournisseur facture par token. Même les modèles bon marché s'additionnent si tu chattes beaucoup, alors choisis quelque chose qui correspond à la fréquence et à l'intensité de ton utilisation.",
          "q1": "Es-tu d'accord pour payer plus pour des interactions plus riches, ou veux-tu quelque chose de bon marché pour le RP quotidien ?",
          "q2": "As-tu des modèles gratuits chez ton fournisseur/router à essayer en premier ?",
          "q3": "Vas-tu faire de longues sessions de RP avec des descriptions de scènes détaillées ?",
          "q4": "As-tu un budget mensuel strict que tu ne veux pas dépasser ?"
        },
        "safety": {
          "title": "Sécurité, confidentialité et extras",
          "description": "Les fournisseurs diffèrent dans leur gestion de la sécurité, du logging et des fonctionnalités supplémentaires comme les images, les outils ou les longues fenêtres de contexte.",
          "q1": "As-tu besoin de moins de filtres de contenu pour des scénarios de RP matures ou créatifs ?",
          "q2": "Te soucies-tu de savoir si tes conversations RP privées sont enregistrées ou utilisées pour l'entraînement ?",
          "q3": "As-tu besoin de longues fenêtres de contexte pour des intrigues complexes et des historiques de personnage ?"
        }
      },
      "where": {
        "title": "Où trouver des modèles ?",
        "intro": "La plupart des fournisseurs et routers ont une <0>liste ou un catalogue de modèles</0>. Parcours ces pages pour voir leur offre, leurs prix, leurs limites et leurs fonctionnalités spéciales.",
        "directTitle": "Fournisseurs directs",
        "directDesc": "OpenAI, Anthropic, Google Gemini, xAI, Mistral, etc. Chacun a une console/dashboard où tu peux voir les noms officiels des modèles, leurs capacités et leurs prix.",
        "routersTitle": "Routers et hubs",
        "routersDesc": "Des services comme OpenRouter ou d'autres agrégateurs listent de nombreux modèles de différents fournisseurs en un seul endroit, souvent avec des benchmarks et des comparaisons de prix.",
        "communityTitle": "Recommandations de la communauté",
        "communityDesc": "Regarde les docs, blogs ou posts communautaires de ton fournisseur/router. Ils mettent souvent en avant les meilleurs modèles pour le chat, le code ou la vitesse."
      },
      "rules": {
        "title": "Règles simples",
        "casual": "Pour un chat décontracté : choisis un modèle de chat rapide et bon marché chez ton fournisseur/router.",
        "experiments": "Pour des expériences ou un gros volume : commence par le modèle le moins cher qui te convient, puis monte en gamme si besoin.",
        "switch": "Si quelque chose ne va pas (trop lent / trop bête / trop cher) : tu peux toujours changer de modèle plus tard dans LettuceAI."
      },
      "disclaimer": "Vérifie toujours la documentation officielle du fournisseur pour la dernière liste de modèles, les limites et les prix. Cette page parle de comment réfléchir, pas de quoi acheter."
    },
    "whereToFind": {
      "badge": "Aide pour la clé API",
      "intro": "Suis ces étapes pour récupérer ta clé API, puis reviens dans LettuceAI et colle-la dans les paramètres du fournisseur.",
      "readyPrompt": "Prêt à récupérer la clé ?",
      "openProviderSite": "Ouvrir le site du fournisseur",
      "keyWarning": "Ne partage jamais ta clé API publiquement. Quiconque possède cette clé peut utiliser le solde de ton compte.",
      "stuckPrompt": "Toujours bloqué ?",
      "joinDiscord": "Rejoins notre serveur Discord pour de l'aide",
      "guides": {
        "chutes": {
          "title": "Comment trouver ta clé API Chutes",
          "s1": "Va sur chutes.ai/app et connecte-toi.",
          "s2": "Ouvre la zone compte/paramètres et trouve les clés API.",
          "s3": "Crée une nouvelle clé (ou copie-en une existante).",
          "s4": "Colle la clé dans LettuceAI."
        },
        "openai": {
          "title": "Comment trouver ta clé API OpenAI",
          "s1": "Va sur platform.openai.com et connecte-toi.",
          "s2": "Clique sur ton avatar de profil en haut à droite, puis choisis API keys.",
          "s3": "Clique sur Create new secret key et copie la valeur affichée.",
          "s4": "Colle la clé dans LettuceAI et garde-la en lieu sûr. Elle ne sera plus affichée."
        },
        "anthropic": {
          "title": "Comment trouver ta clé API Anthropic",
          "s1": "Va sur console.anthropic.com et connecte-toi.",
          "s2": "Ouvre Settings depuis la barre latérale gauche.",
          "s3": "Sélectionne API keys et clique sur Create key.",
          "s4": "Copie la clé et colle-la dans LettuceAI."
        },
        "openrouter": {
          "title": "Comment trouver ta clé API OpenRouter",
          "s1": "Visite openrouter.ai et connecte-toi.",
          "s2": "Ouvre la page Keys depuis ton menu de profil.",
          "s3": "Clique sur Create key, donne-lui un nom et enregistre.",
          "s4": "Copie la clé et colle-la dans LettuceAI."
        },
        "mistral": {
          "title": "Comment trouver ta clé API Mistral",
          "s1": "Va sur console.mistral.ai et connecte-toi.",
          "s2": "Clique sur API keys dans la barre latérale.",
          "s3": "Clique sur Create an API key.",
          "s4": "Copie la clé et colle-la dans LettuceAI."
        },
        "deepseek": {
          "title": "Comment trouver ta clé API DeepSeek",
          "s1": "Ouvre platform.deepseek.com et connecte-toi.",
          "s2": "Clique sur API Keys dans la navigation du haut.",
          "s3": "Crée une nouvelle clé si tu n'en as pas déjà une.",
          "s4": "Copie la clé et colle-la dans LettuceAI."
        },
        "groq": {
          "title": "Comment trouver ta clé API Groq",
          "s1": "Visite console.groq.com et connecte-toi.",
          "s2": "Ouvre API Keys depuis la barre latérale.",
          "s3": "Crée une nouvelle clé, puis copie-la.",
          "s4": "Colle la clé dans LettuceAI."
        },
        "gemini": {
          "title": "Comment trouver ta clé API Google Gemini",
          "s1": "Va sur Google AI Studio à aistudio.google.com et connecte-toi.",
          "s2": "Clique sur Get API key ou Manage keys.",
          "s3": "Crée une nouvelle clé si nécessaire.",
          "s4": "Copie la clé et colle-la dans LettuceAI."
        },
        "xai": {
          "title": "Comment trouver ta clé API xAI",
          "s1": "Ouvre console.x.ai et connecte-toi.",
          "s2": "Va dans la section API Keys de la console.",
          "s3": "Crée une nouvelle clé.",
          "s4": "Copie la clé et colle-la dans LettuceAI."
        },
        "zai": {
          "title": "Comment trouver ta clé API zAI (GLM)",
          "s1": "Va sur open.bigmodel.cn et connecte-toi.",
          "s2": "Ouvre User Center, puis va dans API Keys.",
          "s3": "Crée une nouvelle clé si tu n'en as pas.",
          "s4": "Copie la clé et colle-la dans LettuceAI."
        },
        "moonshot": {
          "title": "Comment trouver ta clé API Moonshot (Kimi)",
          "s1": "Visite platform.moonshot.cn et connecte-toi.",
          "s2": "Ouvre la section API Keys dans la console.",
          "s3": "Crée une nouvelle clé et copie-la.",
          "s4": "Colle la clé dans LettuceAI."
        },
        "qwen": {
          "title": "Comment trouver ta clé API Qwen",
          "s1": "Ouvre dashscope.aliyun.com et connecte-toi.",
          "s2": "Va dans la section API Keys de la barre latérale.",
          "s3": "Crée une nouvelle clé.",
          "s4": "Copie la clé et colle-la dans LettuceAI."
        },
        "nanogpt": {
          "title": "Comment trouver ta clé API NanoGPT",
          "s1": "Va sur nano-gpt.com et connecte-toi.",
          "s2": "Ouvre le tableau de bord et va dans la section des clés API.",
          "s3": "Crée une nouvelle clé si nécessaire.",
          "s4": "Copie la clé et colle-la dans LettuceAI."
        },
        "featherless": {
          "title": "Comment trouver ta clé API Featherless",
          "s1": "Visite featherless.ai et connecte-toi.",
          "s2": "Ouvre ta section compte ou API depuis le tableau de bord.",
          "s3": "Crée une nouvelle clé si tu n'en vois pas.",
          "s4": "Copie la clé et colle-la dans LettuceAI."
        },
        "anannas": {
          "title": "Comment trouver ta clé API Anannas",
          "s1": "Va sur dashboard.anannas.ai et connecte-toi.",
          "s2": "Va dans la section API Keys.",
          "s3": "Crée une nouvelle clé et copie-la.",
          "s4": "Colle la clé dans LettuceAI."
        },
        "default": {
          "title": "Comment trouver ta clé API",
          "s1": "Ouvre le tableau de bord de ton fournisseur d'IA dans un navigateur et connecte-toi.",
          "s2": "Cherche les paramètres API, Developer ou Integrations.",
          "s3": "Crée une nouvelle clé API ou consulte une clé existante.",
          "s4": "Copie la clé et colle-la dans LettuceAI."
        }
      }
    },
    "welcomeFooter": {
      "setupTimeMobile": "La configuration prend moins de 2 minutes"
    }
  },
  "search": {
    "placeholder": "Rechercher...",
    "tabs": {
      "characters": "Personnages",
      "personas": "Personas"
    },
    "noResults": "Aucun(e) {{type}} trouvé(e)",
    "emptyState": "Pas encore de {{type}}",
    "noResultsHint": "Essayez un autre terme de recherche",
    "emptyCharacters": "Créez votre premier personnage pour commencer à discuter",
    "emptyPersonas": "Créez un persona dans les paramètres",
    "a11y": {
      "goBack": "Retour",
      "clearSearch": "Effacer la recherche",
      "characterAvatar": "Avatar de {{name}}"
    },
    "session": {
      "newChatTitle": "Nouveau chat"
    },
    "noDescription": "Aucune description",
    "defaultBadge": "Par défaut"
  },
  "sync": {
    "modes": {
      "join": "Rejoindre",
      "joinDesc": "Se connecter à l'hôte",
      "host": "Héberger",
      "hostDesc": "Partager vos données"
    },
    "sections": {
      "mode": "Mode",
      "connectToHost": "Se connecter à l'hôte",
      "startHosting": "Commencer à héberger",
      "status": "Statut",
      "hosting": "Service d'hébergement",
      "localAddress": "Adresse réseau local",
      "connectionPin": "PIN de connexion",
      "setupGuide": "Guide de configuration"
    },
    "fields": {
      "hostAddress": "Adresse de l'hôte ou JSON",
      "hostPlaceholder": "ex. 192.168.1.100:12345",
      "pinCode": "Code PIN",
      "pinPlaceholder": "000000"
    },
    "buttons": {
      "scanQRCode": "Scanner le QR Code",
      "connect": "Se connecter",
      "connecting": "Connexion...",
      "startHosting": "Commencer à héberger",
      "startingServer": "Démarrage du serveur...",
      "stopHosting": "Arrêter l'hébergement",
      "hostAgain": "Héberger à nouveau",
      "done": "Terminé"
    },
    "status": {
      "connecting": "Connexion...",
      "connected": "Connecté",
      "waitingConfirmation": "En attente de confirmation",
      "waitingConfirmationDesc": "Approuvez la connexion sur le périphérique hôte pour continuer.",
      "syncing": "Synchronisation...",
      "transferringData": "Transfert des données",
      "syncInProgress": "Synchronisation en cours",
      "live": "En direct",
      "broadcasting": "Diffusion",
      "clientsLabel": "Connectés",
      "clientsUnit": "Clients"
    },
    "pinDescription": "Partagez ce PIN avec l'appareil qui se connecte",
    "hostingDesc1": "Les autres appareils peuvent se connecter et synchroniser les données depuis cet appareil.",
    "hostingDesc2": "Vos données seront partagées avec les clients connectés.",
    "setupSteps": {
      "step1": "Ouvrez l'application sur un autre appareil",
      "step2": "Allez dans Paramètres → Synchronisation locale",
      "step3": "Scannez le QR code ou entrez l'adresse"
    },
    "messages": {
      "completed": "Synchronisation terminée !",
      "completedDesc": "Toutes les données synchronisées",
      "error": "Erreur de connexion",
      "outdatedClient": "Client obsolète détecté"
    },
    "disclaimer": "La synchronisation fonctionne sur votre réseau local. Les deux appareils doivent être sur le même WiFi.",
    "modals": {
      "connectionRequest": "Demande de connexion",
      "requestMessage": "veut se synchroniser avec cet appareil.",
      "acceptConnection": "Accepter la connexion",
      "acceptDesc": "Permettre à cet appareil de synchroniser les données",
      "decline": "Refuser",
      "declineDesc": "Bloquer cette tentative de connexion",
      "readyToSync": "Prêt à synchroniser",
      "connectionEstablished": "Connexion établie",
      "deviceReady": "est prêt.",
      "startSyncMessage": "Appuyez ci-dessous pour commencer la synchronisation des données.",
      "startSyncing": "Démarrer la synchronisation",
      "startSyncingDesc": "Commencer le transfert de données maintenant"
    },
    "scanner": {
      "title": "Scanner le QR Code",
      "cancel": "Annuler le scan"
    },
    "unknownDevice": "Appareil inconnu",
    "aria": {
      "dismissStatus": "Ignorer le statut de synchronisation",
      "dismissError": "Ignorer l'erreur de synchronisation"
    },
    "stats": {
      "statusLabel": "Statut"
    }
  },
  "creationHelper": {
    "page": {
      "info": "L'Assistant de Création vous guide dans la construction de personnages avec l'aide de l'IA. Configurez le modèle et les outils utilisés pendant la création de personnages.",
      "modelConfiguration": "Configuration du Modèle",
      "chatModel": "Modèle de Chat",
      "selectedModel": "Modèle Sélectionné",
      "useAppDefault": "Utiliser le modèle par défaut{{model}}",
      "useAppDefaultBase": "Utiliser le modèle par défaut",
      "noModelsAvailable": "Aucun modèle disponible",
      "chatModelDescription": "Modèle d'IA pour les conversations de création de personnages",
      "streamingOutput": "Sortie en Streaming",
      "streamingDescription": "Afficher les réponses au fur et à mesure de leur génération",
      "imageGenerationModel": "Modèle de Génération d'Images",
      "noModelSelected": "Aucun modèle sélectionné",
      "noImageModelsAvailable": "Aucun modèle d'image disponible",
      "imageModelDescription": "Pour générer les avatars des personnages",
      "toolSelection": "Sélection des Outils",
      "smartToolSelection": "Sélection Intelligente des Outils",
      "smartToolDescription": "L'IA choisit automatiquement les outils à utiliser",
      "smartToolEnabledHint": "Lorsqu'activé, l'Assistant de Création demande ce que vous souhaitez créer et ne charge que l'ensemble d'outils pertinent.",
      "smartToolDisabledHint": "Lorsque désactivé, l'Assistant de Création s'ouvre directement et utilise tous les outils activés ; l'assistant décide quoi construire.",
      "quickPresets": "Préréglages Rapides",
      "customSelection": "Sélection personnalisée - {{count}} outils activés",
      "footerInfo": "Lorsque la Sélection Intelligente des Outils est activée, l'IA décide quels outils utiliser en fonction du contexte. Désactivez-la pour contrôler manuellement les outils disponibles.",
      "selectChatModel": "Sélectionner le Modèle de Chat",
      "selectImageModel": "Sélectionner le Modèle d'Image",
      "searchModels": "Rechercher des modèles..."
    },
    "categories": {
      "basic": "Basique",
      "content": "Contenu",
      "visual": "Visuel",
      "settings": "Paramètres",
      "flow": "Flux",
      "persona": "Personas",
      "lorebook": "Encyclopédies"
    },
    "presets": {
      "all": {
        "name": "Tous les outils",
        "desc": "Activer tous les outils disponibles"
      },
      "essential": {
        "name": "Essentiel",
        "desc": "Nom, définition et scènes uniquement"
      },
      "minimal": {
        "name": "Minimal",
        "desc": "Juste le nom et la définition"
      }
    },
    "tools": {
      "setName": "Définir le nom",
      "setNameDesc": "Définir le nom du personnage",
      "setDefinition": "Définir la définition",
      "setDefinitionDesc": "Définir la personnalité et le contexte",
      "set_character_name": {
        "name": "Définir le nom",
        "desc": "Définir le nom du personnage"
      },
      "set_character_definition": {
        "name": "Définir la définition",
        "desc": "Définir la personnalité et le contexte"
      },
      "add_scene": {
        "name": "Ajouter une scène",
        "desc": "Ajouter une scène de départ pour le jeu de rôle"
      },
      "update_scene": {
        "name": "Modifier une scène",
        "desc": "Modifier une scène existante"
      },
      "toggle_avatar_gradient": {
        "name": "Dégradé de l'avatar",
        "desc": "Activer/désactiver le dégradé sur l'avatar"
      },
      "set_default_model": {
        "name": "Définir le modèle",
        "desc": "Définir le modèle IA pour les conversations"
      },
      "set_system_prompt": {
        "name": "Prompt système",
        "desc": "Définir les directives de comportement"
      },
      "get_system_prompt_list": {
        "name": "Lister les prompts",
        "desc": "Voir les prompts disponibles"
      },
      "get_model_list": {
        "name": "Lister les modèles",
        "desc": "Voir les modèles disponibles"
      },
      "use_uploaded_image_as_avatar": {
        "name": "Image comme avatar",
        "desc": "Utiliser l'image téléchargée comme avatar"
      },
      "use_uploaded_image_as_chat_background": {
        "name": "Image comme arrière-plan",
        "desc": "Utiliser l'image téléchargée comme arrière-plan"
      },
      "generate_image": {
        "name": "Générer une image",
        "desc": "Générer une image avec le modèle IA"
      },
      "show_preview": {
        "name": "Afficher l'aperçu",
        "desc": "Prévisualiser le personnage"
      },
      "request_confirmation": {
        "name": "Demander confirmation",
        "desc": "Demander d'enregistrer ou de continuer"
      },
      "list_personas": {
        "name": "Lister les personas",
        "desc": "Parcourir les personas"
      },
      "upsert_persona": {
        "name": "Enregistrer le persona",
        "desc": "Créer ou mettre à jour un persona"
      },
      "use_uploaded_image_as_persona_avatar": {
        "name": "Avatar de persona",
        "desc": "Utiliser l'image téléchargée comme avatar de persona"
      },
      "delete_persona": {
        "name": "Supprimer le persona",
        "desc": "Supprimer un persona"
      },
      "get_default_persona": {
        "name": "Persona par défaut",
        "desc": "Récupérer le persona par défaut"
      },
      "list_lorebooks": {
        "name": "Lister les encyclopédies",
        "desc": "Parcourir les encyclopédies"
      },
      "upsert_lorebook": {
        "name": "Enregistrer l'encyclopédie",
        "desc": "Créer ou mettre à jour une encyclopédie"
      },
      "delete_lorebook": {
        "name": "Supprimer l'encyclopédie",
        "desc": "Supprimer une encyclopédie"
      },
      "list_lorebook_entries": {
        "name": "Lister les entrées",
        "desc": "Voir les entrées de l'encyclopédie"
      },
      "get_lorebook_entry": {
        "name": "Obtenir l'entrée",
        "desc": "Récupérer une entrée d'encyclopédie"
      },
      "upsert_lorebook_entry": {
        "name": "Enregistrer l'entrée",
        "desc": "Créer ou mettre à jour une entrée"
      },
      "delete_lorebook_entry": {
        "name": "Supprimer l'entrée",
        "desc": "Supprimer une entrée d'encyclopédie"
      },
      "create_blank_lorebook_entry": {
        "name": "Entrée vide",
        "desc": "Créer une entrée vide"
      },
      "reorder_lorebook_entries": {
        "name": "Réorganiser les entrées",
        "desc": "Modifier l'ordre des entrées"
      },
      "list_character_lorebooks": {
        "name": "Lister les encyclopédies du personnage",
        "desc": "Voir les encyclopédies d'un personnage"
      },
      "set_character_lorebooks": {
        "name": "Définir les encyclopédies du personnage",
        "desc": "Assigner des encyclopédies à un personnage"
      },
      "addScene": "Ajouter une scène",
      "addSceneDesc": "Ajoute une scène de départ pour le roleplay",
      "updateScene": "Mettre à jour la scène",
      "updateSceneDesc": "Modifie une scène existante",
      "avatarGradient": "Dégradé de l'avatar",
      "avatarGradientDesc": "Active/désactive la superposition en dégradé sur l'avatar",
      "setModel": "Définir le modèle",
      "setModelDesc": "Définis le modèle d'IA pour les conversations",
      "systemPrompt": "Prompt système",
      "systemPromptDesc": "Définis les directives de comportement",
      "listPrompts": "Lister les prompts",
      "listPromptsDesc": "Voir les prompts disponibles",
      "listModels": "Lister les modèles",
      "listModelsDesc": "Voir les modèles disponibles",
      "imageAsAvatar": "Image comme avatar",
      "imageAsAvatarDesc": "Utilise l'image téléversée comme avatar"
    }
  },
  "tour": {
    "stepCounter": "Étape {{current}} sur {{total}}",
    "skipTour": "Passer le tour",
    "next": "Suivant",
    "gotIt": "Compris",
    "appShell": {
      "chats": {
        "title": "Vos discussions sont ici",
        "body": "Toutes vos conversations individuelles avec les personnages se trouvent ici. Revenez quand vous voulez, on garde votre place."
      },
      "groups": {
        "title": "Discussions de groupe",
        "body": "Réunissez plusieurs personnages dans la même salle et regardez-les discuter, ou rejoignez-les quand vous le souhaitez."
      },
      "discover": {
        "title": "Découvrez de nouveaux personnages",
        "body": "Parcourez ce que la communauté a partagé et ajoutez tout personnage qui vous plaît. De nouveaux favoris à portée de main."
      },
      "library": {
        "title": "Votre bibliothèque personnelle",
        "body": "Tout ce que vous avez créé ou sauvegardé est ici : personnages, personas, prompts, tout. Considérez-la comme votre collection."
      },
      "settings": {
        "title": "Personnalisez",
        "body": "Changez de fournisseur, choisissez différents modèles, ajustez l'apparence. Pratiquement tout est configurable depuis les paramètres."
      },
      "search": {
        "title": "Trouvez n'importe quoi, vite",
        "body": "Vous cherchez un chat ou un personnage précis ? Recherchez partout depuis ici. Pas besoin de fouiller."
      },
      "create": {
        "title": "Et enfin, créez !",
        "body": "Appuyez sur le plus quand l'inspiration frappe. Créez un nouveau personnage, une persona ou commencez quelque chose de zéro."
      }
    },
    "chatDetail": {
      "chatTitle": {
        "title": "Paramètres par chat",
        "body": "Appuyez sur le nom du personnage ici pour ouvrir les paramètres de ce chat. Différentes personas, mises en page et modèles par conversation."
      },
      "chatMemory": {
        "title": "Ce dont ils se souviennent",
        "body": "L'icône de cerveau montre ce que votre personnage retient de vos conversations. Appuyez pour consulter, modifier ou effacer les souvenirs."
      },
      "chatSearch": {
        "title": "Trouvez cette réplique",
        "body": "Recherchez uniquement dans cette conversation. Idéal pour retrouver ce détail d'il y a 200 messages sans défiler éternellement."
      },
      "chatLorebook": {
        "title": "Entrées du lorebook",
        "body": "Faits supplémentaires, construction du monde et contexte injectés dans le prompt lorsque des mots-clés spécifiques apparaissent. L'aide-mémoire de votre personnage."
      },
      "chatPlus": {
        "title": "Joindre des éléments",
        "body": "Ajoutez des images ou ouvrez le menu des extras. Ce que vous joignez sera envoyé avec votre prochain message."
      },
      "chatComposer": {
        "title": "Votre message, à vous de jouer",
        "body": "Tapez ici. Entrée envoie, Maj+Entrée ajoute une nouvelle ligne. Astuce : appuyez longuement sur n'importe quel message pour le modifier, le brancher ou le supprimer."
      },
      "chatSend": {
        "title": "Un bouton, quatre fonctions",
        "body": "Le bouton d'envoi change de fonction selon ce qui se passe :"
      }
    },
    "postFirstMessage": {
      "chatRegenerate": {
        "title": "Pas satisfait ? Régénérez",
        "body": "Appuyez sur l'icône d'actualisation pour obtenir une réponse complètement nouvelle du personnage. Chaque régénération est sauvegardée comme une variante que vous pouvez revisiter."
      },
      "chatVariants": {
        "title": "Glissez entre les variantes",
        "body": "Après avoir régénéré, vous verrez un compteur de variantes sous le message. Glissez à gauche ou à droite sur la bulle pour parcourir les différentes réponses."
      },
      "chatLongPress": {
        "title": "Il y a plus caché ici",
        "body": "Appuyez longuement sur n'importe quel message pour modifier, copier, brancher, épingler, supprimer ou rembobiner la conversation. Le clic droit fonctionne aussi sur bureau."
      }
    },
    "sendButton": {
      "continue": {
        "label": "Continuer",
        "desc": "Champ vide. Appuyer ici encouragera le personnage à continuer de parler."
      },
      "send": {
        "label": "Envoyer",
        "desc": "Vous avez tapé ou joint quelque chose. Appuyez pour envoyer."
      },
      "sending": {
        "label": "Envoi",
        "desc": "La réponse est en chemin. Bouton verrouillé."
      },
      "stop": {
        "label": "Arrêter",
        "desc": "Appuyez pour annuler en cours de réponse si vous changez d'avis."
      }
    },
    "extra": {
      "rerunOnboarding": "Relancer l'onboarding"
    }
  },
  "sessionAdvanced": {
    "title": "Paramètres de session",
    "subtitle": "Remplacer les valeurs par défaut du modèle pour cette conversation",
    "goBack": "Retour",
    "support": "Support",
    "reset": "Réinitialiser",
    "save": "Enregistrer",
    "noSessionWarning": "Ouvrez une session de chat pour configurer les paramètres par session.",
    "overrideDefaults": "Remplacer les valeurs par défaut",
    "overrideDefaultsDesc": "Personnaliser les paramètres uniquement pour cette conversation",
    "loadingContextInfo": "Chargement des informations de contexte...",
    "sampling": {
      "title": "Échantillonnage",
      "temperature": "Temperature",
      "temperatureDesc": "Contrôle l'aléatoire. Plus bas = plus déterministe, plus haut = plus créatif.",
      "temperaturePrecise": "Précis",
      "temperatureCreative": "Créatif",
      "topP": "Top P",
      "topPDesc": "Échantillonnage par noyau. Limite les tokens à une probabilité cumulative.",
      "topPFocused": "Concentré",
      "topPDiverse": "Diversifié",
      "topK": "Top K",
      "topKDesc": "Limite l'échantillonnage aux K tokens les plus probables."
    },
    "outputPenalties": {
      "title": "Sortie et pénalités",
      "maxOutputTokens": "Tokens de sortie maximum",
      "maxOutputTokensDesc": "Longueur maximale de réponse. Auto laisse le modèle décider.",
      "auto": "Auto",
      "custom": "Personnalisé",
      "frequencyPenalty": "Pénalité de fréquence",
      "frequencyPenaltyDesc": "Réduit la répétition de séquences de tokens.",
      "frequencyPenaltyRepeat": "Répéter",
      "frequencyPenaltyVary": "Varier",
      "presencePenalty": "Pénalité de présence",
      "presencePenaltyDesc": "Encourage l'exploration de nouveaux sujets.",
      "presencePenaltyRepeat": "Répéter",
      "presencePenaltyExplore": "Explorer"
    },
    "performance": {
      "title": "Performance",
      "gpuLayers": "Couches GPU",
      "gpuLayersDesc": "Couches déchargées sur le GPU. 0 = CPU uniquement.",
      "threads": "Threads",
      "threadsDesc": "Threads CPU pour l'inférence.",
      "batchThreads": "Threads de lot",
      "batchThreadsDesc": "Threads CPU pour le traitement par lots.",
      "batchSize": "Taille de lot",
      "batchSizeDesc": "Taille des blocs de traitement du prompt.",
      "contextLength": "Longueur de contexte",
      "contextLengthDesc": "Remplacer la taille de la fenêtre de contexte.",
      "flashAttention": "Flash Attention",
      "flashAttentionDesc": "Optimisation de la mémoire GPU.",
      "enabled": "Activé",
      "disabled": "Désactivé"
    },
    "samplingMemory": {
      "title": "Échantillonnage et mémoire",
      "minP": "Min P",
      "minPDesc": "Seuil de probabilité minimum.",
      "typicalP": "Typical P",
      "typicalPDesc": "Seuil d'échantillonnage typique.",
      "seed": "Seed",
      "seedDesc": "Graine aléatoire. Laisser vide pour aléatoire.",
      "ropeBase": "RoPE Base",
      "ropeBaseDesc": "Remplacement de la base de fréquence.",
      "ropeScale": "RoPE Scale",
      "ropeScaleDesc": "Remplacement de l'échelle de fréquence.",
      "kvCacheType": "KV Cache Type",
      "kvCacheTypeDesc": "Quantifier le KV cache pour économiser la VRAM.",
      "offloadKqv": "Offload KQV",
      "offloadKqvDesc": "KV cache et opérations KQV sur GPU.",
      "on": "Activé",
      "off": "Désactivé",
      "samplerProfile": "Profil d'échantillonnage",
      "samplerProfileDesc": "Valeurs par défaut ajustées pour la stabilité ou le raisonnement.",
      "balanced": "Équilibré",
      "creative": "Créatif",
      "stable": "Stable",
      "reasoning": "Raisonnement"
    },
    "systemInfo": {
      "title": "Informations système",
      "maxContext": "Contexte maximum",
      "recommended": "Recommandé",
      "availableRam": "RAM disponible",
      "availableVram": "VRAM disponible",
      "modelSize": "Taille du modèle"
    }
  },
  "exportMenu": {
    "title": "Format d'exportation",
    "selectFormat": "Sélectionnez un format",
    "uscTitle": "Unified System Card",
    "formats": {
      "uscPrompt": "Exportation USC portable pour les modèles de prompt.",
      "uscLorebook": "Exportation USC portable pour les lorebooks.",
      "uscModel": "Exportation USC portable pour les profils de modèle.",
      "uscChatTemplate": "Exportation USC portable pour les modèles de chat.",
      "legacyPromptJson": "Legacy Prompt JSON",
      "legacyPromptJsonDesc": "Format actuel de pack de prompts externe.",
      "lorebookJson": "Lorebook JSON",
      "lorebookJsonDesc": "Format actuel d'exportation de lorebook.",
      "modelJson": "Model JSON",
      "modelJsonDesc": "JSON sécurisé de profil de modèle sans identifiants.",
      "chatTemplateJson": "Chat Template JSON",
      "chatTemplateJsonDesc": "Format natif d'exportation de modèle de chat."
    },
    "extra": {
      "selectFormat": "Sélectionne un format",
      "exportFormatTitle": "Format d'export",
      "uscDesc": "Export USC portable",
      "legacyJsonDesc": "Format d'export JSON ancien",
      "formatV3Desc": "Export Character Card V3",
      "formatV2Desc": "Export Character Card V2",
      "formatV1Desc": "Export Character Card V1",
      "uscLorebook": "Unified System Card (USC)",
      "portableLorebook": "Export USC portable pour les lorebooks",
      "lorebookJson": "JSON Lorebook",
      "currentLorebook": "Format d'export actuel du lorebook",
      "uscModel": "Unified System Card (USC)",
      "portableModel": "Export USC portable pour les profils de modèle",
      "modelJson": "JSON Modèle",
      "safeModel": "JSON de profil de modèle sûr sans identifiants",
      "uscTemplate": "Unified System Card (USC)",
      "portableTemplate": "Export USC portable pour les modèles de chat",
      "templateJson": "JSON Modèle de chat",
      "nativeTemplate": "Format d'export natif des modèles de chat"
    }
  },
  "designReference": {
    "title": "Références de design",
    "description": "Téléchargez quelques images de référence claires plus une description visuelle canonique.",
    "descriptionPlaceholder": "Décrivez l'apparence stable : visage, cheveux, corpulence, âge apparent, indices vestimentaires, accessoires et direction artistique/style.",
    "addReferences": "Ajouter des références",
    "visualDescription": "Description visuelle",
    "draftWithAi": "Brouillon avec l'IA",
    "referenceImages": "Images de référence",
    "imageAlt": "Référence de design {{index}}",
    "loading": "Chargement...",
    "removeAria": "Supprimer la référence de design",
    "noImages": "Aucune image de référence jointe pour l'instant",
    "imageCount": "{{count}} image(s) de référence jointe(s)",
    "emptyReferences": "Ajoutez quelques photos de référence claires pour fixer le visage, les proportions, la tenue et le style.",
    "noWriterModel": "Ajoutez d'abord un modèle de rédacteur de scènes compatible dans les paramètres de Génération d'Images.",
    "noImagesForGeneration": "Ajoutez un avatar ou au moins une image de référence avant de générer.",
    "writerModelHelp": "Utilise {{model}} pour rédiger à partir de votre avatar et de vos images de référence.",
    "noWriterModelHelp": "Ajoutez un modèle de rédacteur de scènes compatible dans les paramètres de Génération d'Images pour rédiger ceci automatiquement.",
    "draftMenuTitle": "Brouillon de design par l'IA",
    "draftMenuDesc": "Rédigé par {{model}} à partir de l'avatar actuel et des images de référence.",
    "draftMenuNoWriter": "Ajoutez un modèle de rédacteur de scènes compatible avant d'utiliser cet assistant.",
    "regenerate": "Régénérer",
    "useThis": "Utiliser ceci"
  },
  "samplerOrder": {
    "title": "Ordre du sampler",
    "description": "Faites glisser les étapes pour réorganiser. Exécution de haut en bas.",
    "reset": "Réinitialiser",
    "resetAria": "Réinitialiser l'ordre du sampler par défaut",
    "stages": {
      "penalties": {
        "label": "Pénalités",
        "desc": "Appliquer les pénalités de fréquence et de présence avant le filtrage."
      },
      "grammar": {
        "label": "Grammaire",
        "desc": "Contraindre les tokens lorsqu'une grammaire de modèle de chat natif est active."
      },
      "topK": {
        "label": "Top K",
        "desc": "Réduire le pool de candidats aux tokens les plus forts."
      },
      "topP": {
        "label": "Top P",
        "desc": "Appliquer le filtrage par noyau à la distribution restante."
      },
      "minP": {
        "label": "Min P",
        "desc": "Éliminer les tokens de faible probabilité avec un seuil minimum."
      },
      "typical": {
        "label": "Typical P",
        "desc": "Préférer les tokens statistiquement typiques dans la distribution actuelle."
      },
      "temp": {
        "label": "Temperature",
        "desc": "Aplatir ou affiner la distribution finale avant la sélection."
      }
    },
    "presets": {
      "default": {
        "label": "Par défaut",
        "hint": "Lettuce local"
      },
      "unsloth": {
        "label": "Unsloth",
        "hint": "Style llama.cpp"
      },
      "focused": {
        "label": "Concentré",
        "hint": "Élagage serré"
      },
      "creative": {
        "label": "Créatif",
        "hint": "Filtre tardif"
      }
    }
  },
  "lorebookGen": {
    "flow": {
      "pickerCharactersTitle": "Personnages",
      "pickerSessionsTitle": "Sessions",
      "noCharacters": "Aucun personnage",
      "noSessions": "Aucune session",
      "clearSelection": "Effacer la sélection",
      "directionTitle": "Direction de génération optionnelle",
      "directionLabel": "Direction",
      "toggleForceMode": "Activer/désactiver le mode forcé",
      "entryTitlePlaceholder": "Titre de l'entrée",
      "entryContentPlaceholder": "Contenu de l'entrée de lorebook",
      "editDirectionBeforeRegenerate": "Modifier la direction avant de régénérer",
      "generatorReturnedNoDraft": "Le générateur n'a pas retourné de brouillon",
      "pageTitle": "Générer une entrée de lorebook",
      "missingContext": "Contexte de lorebook manquant pour la page du générateur.",
      "characterLocked": "Le personnage est verrouillé sur le propriétaire de ce lorebook",
      "chooseSession": "Choisir une session",
      "pickCharacter": "Choisir un personnage",
      "searchMemories": "Rechercher dans les souvenirs",
      "searchMessages": "Rechercher dans les messages",
      "selectLastN": "Sélectionner les {{n}} derniers messages",
      "selectAll": "Tout sélectionner",
      "loadSessionPrompt": "Sélectionne une session à charger",
      "messagesText": "messages",
      "memoriesText": "souvenirs",
      "messagesAndMemories": "messages et souvenirs",
      "titleAndContentRequired": "Le titre et le contenu sont requis",
      "keywordsOrAlwaysActive": "Ajoute au moins un mot-clé ou active Toujours actif",
      "lorybookEntrySaved": "Entrée de lorebook enregistrée",
      "saveFailed": "Échec de l'enregistrement",
      "generationFailed": "Échec de la génération",
      "failedToLoadContext": "Échec du chargement du générateur de lorebook",
      "failedToLoadSessions": "Échec du chargement des sessions",
      "failedToLoadMessages": "Échec du chargement des messages",
      "userRole": "UTILISATEUR",
      "aiRole": "IA"
    },
    "triggerPreview": {
      "resizeInspector": "Redimensionner l'inspecteur"
    }
  },
  "companion": {
    "download": {
      "emotionClassifier": "Classificateur d'émotions",
      "emotionClassifierDesc": "Lit les tours et met à jour les vecteurs d'émotions ressenties, exprimées et bloquées du compagnon.",
      "emotionSize": "~120 Mo",
      "entityExtractor": "Extracteur d'entités (NER)",
      "entityExtractorDesc": "Identifie les personnes, lieux et objets pour que les souvenirs puissent être canonisés et liés.",
      "entitySize": "~140 Mo",
      "memoryRouter": "Routeur de mémoire",
      "memoryRouterDesc": "Décide si les nouveaux tours doivent être stockés en tant que relation, jalon, épisode ou autres catégories de mémoire.",
      "routerSize": "~70 Mo",
      "unknownModel": "Modèle de compagnon inconnu. Fournis ?kind=emotion|ner|router."
    }
  },
  "voices": {
    "extra": {
      "customVoices": "Mes voix",
      "providerVoices": "Voix du fournisseur",
      "myVoices": "Mes voix",
      "page": {
        "noAudioProvidersHint": "Ajoutez-en un dans Fournisseurs > Audio pour commencer",
        "noVoicesTitle": "Aucune voix créée pour l'instant",
        "noVoicesDescription": "Créez des voix avec des prompts personnalisés pour vos personnages",
        "addProviderFirst": "Ajoutez d'abord un fournisseur audio",
        "noPrompt": "Aucun prompt",
        "noProviderVoices": "Aucune voix trouvée. Cliquez sur Actualiser pour récupérer les voix.",
        "showLess": "Afficher moins",
        "showAllVoices": "Afficher les {{count}} voix",
        "voiceFallbackTitle": "Voix"
      },
      "cache": {
        "section": "Cache audio",
        "title": "Cache audio TTS",
        "description": "L'audio vocal généré est mis en cache pour réduire les régénérations",
        "clearing": "Vidage en cours...",
        "clear": "Vider le cache"
      },
      "menu": {
        "editDescription": "Modifier cette voix",
        "deleteDescription": "Supprimer cette voix",
        "provider": "Fournisseur",
        "category": "Catégorie",
        "createVoiceConfig": "Créer une configuration de voix",
        "createVoiceConfigDescription": "Utiliser cette voix avec des paramètres personnalisés"
      },
      "editor": {
        "editTitle": "Modifier la voix",
        "createTitle": "Créer une voix",
        "voiceName": "Nom de la voix",
        "voiceNamePlaceholder": "La voix de mon personnage",
        "provider": "Fournisseur",
        "model": "Modèle",
        "modelIdPlaceholder": "gpt-4o-mini-tts",
        "modelIdHint": "Entrez l'ID de modèle exact attendu par votre point de terminaison compatible",
        "elevenlabsVoice": "Voix ElevenLabs",
        "noVoicesAvailable": "Aucune voix disponible",
        "selectVoice": "Sélectionner une voix...",
        "elevenlabsVoiceHint": "Sélectionnez parmi vos voix ElevenLabs",
        "geminiVoice": "Voix Gemini",
        "geminiVoiceHint": "Sélectionnez une voix Gemini TTS",
        "voiceId": "ID de voix",
        "voiceIdPlaceholder": "alloy",
        "voiceIdHint": "Entrez l'ID de voix pris en charge par votre point de terminaison compatible",
        "voicePrompt": "Prompt de voix",
        "voicePromptPlaceholder": "Une voix chaleureuse et amicale avec un ton enjoué...",
        "voicePromptHint": "Décrivez comment la voix devrait sonner",
        "exampleText": "Texte d'exemple",
        "exampleTextPlaceholder": "Bonjour ! Voici comment je sonne quand je parle...",
        "exampleTextHint": "Texte de démonstration pour tester la voix",
        "voiceDesignChars": "{{current}}/{{minimum}} caractères requis pour la prévisualisation de la conception vocale",
        "defaultSample": "Bonjour ! Voici comment je sonne quand je parle. Je peux lire de longs passages avec chaleur, clarté et émotion pour que vous puissiez juger mon ton et mon rythme.",
        "playing": "En lecture...",
        "previewVoice": "Aperçu de la voix"
      },
      "kokoro": {
        "title": "Kokoro Studio",
        "newBlend": "Nouveau mélange",
        "editBlend": "Modifier le mélange",
        "tryText": "Bonjour ! Voici un test rapide de comment je sonne.",
        "experimentDefaultText": "Bonjour ! Voici comment je sonne quand je parle. Je peux lire de longs passages avec chaleur, clarté et émotion.",
        "livePreview": "Aperçu en direct",
        "savedBlend": "Mélange enregistré",
        "defaultPreviewText": "Bonjour ! Voici un aperçu rapide de cette voix.",
        "experiment": "Expérimenter",
        "providerNotFound": "Fournisseur Kokoro introuvable",
        "backToProviders": "Retour aux fournisseurs",
        "variantUnset": "Variante non définie",
        "ready": "Prêt",
        "modelNotInstalled": "Modèle non installé",
        "voiceCount": "{{count}} voix",
        "engineActions": "Actions du moteur",
        "engineNotInstalled": "Moteur non installé",
        "installAtLeastOneVoice": "Installez au moins une voix",
        "continueSetup": "Continuez la configuration pour installer le modèle Kokoro.",
        "pickVoiceOrStarter": "Choisissez une voix ou prenez le pack de démarrage pour commencer.",
        "downloadsFailed": "{{count}} téléchargement échoué",
        "retryOrDismissAll": "Réessayez individuellement ou ignorez tout.",
        "dismissAll": "Tout ignorer",
        "model": "Modèle",
        "voice": "Voix",
        "downloads": "Téléchargements",
        "cancelAll": "Tout annuler",
        "experimentPlaceholder": "Saisissez une phrase pour l'entendre...",
        "speed": "Vitesse",
        "speak": "Énoncer",
        "yourBlends": "Vos mélanges",
        "noSavedBlends": "Aucun mélange enregistré pour l'instant.",
        "installModelAndVoiceFirst": "Installez d'abord le modèle et une voix.",
        "featured": "En vedette",
        "stop": "Arrêter",
        "sample": "Exemple",
        "voiceLibrary": "Bibliothèque de voix",
        "starterPack": "Pack de démarrage",
        "select": "Sélectionner",
        "all": "Toutes",
        "installed": "Installées",
        "installModelToBrowse": "Installez le modèle pour parcourir les voix.",
        "noVoicesInCatalog": "Aucune voix dans le catalogue. Appuyez sur Actualiser.",
        "noVoicesMatch": "Aucune voix ne correspond à vos filtres.",
        "collapseAll": "Tout réduire",
        "expandAll": "Tout développer",
        "selectedCount": "{{count}} sélectionnée(s)",
        "engineTitle": "Moteur Kokoro",
        "variant": "Variante",
        "status": "Statut",
        "notInstalled": "Non installé",
        "file": "Fichier",
        "modelSize": "Taille du modèle",
        "voicesSize": "Taille des voix",
        "total": "Total",
        "assetRoot": "Répertoire des ressources",
        "reinstallModel": "Réinstaller le modèle",
        "installModel": "Installer le modèle",
        "deleteModel": "Supprimer le modèle",
        "deleteModelDescription": "Libère de l'espace disque ; les voix sont conservées.",
        "blend": "Mélange",
        "previewDescription": "Écoute rapide avec le texte par défaut",
        "editBlendDescription": "Ajuster les voix, les pondérations et la vitesse",
        "deleteBlendDescription": "Supprimer cette voix enregistrée",
        "setupTitle": "Configurer Kokoro",
        "allSet": "Tout est prêt",
        "allSetDescription": "Parcourez les voix, créez des mélanges ou testez dans la zone d'expérimentation."
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
      "conditionalsSection": "Conditions",
      "injectionPoints": "Points d'injection"
    }
  },
  "embeddings": {
    "download": {
      "bestForQuick": "Idéal pour les réponses rapides",
      "balancedPerf": "Performance équilibrée",
      "maxContext": "Contexte maximum",
      "capacity1k": "1K tokens",
      "capacity2k": "2K tokens",
      "capacity4k": "4K tokens",
      "approxSize": "~138 Mo",
      "downloadVersion": "V4"
    },
    "test": {
      "health": "Vérification de santé",
      "retrieval": "Récupération",
      "separation": "Séparation",
      "passed": "Réussi",
      "failed": "Échec",
      "testing": "Test en cours..."
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
      "jsonDesc": "Sortie structurée compacte quand le tool calling n'est pas disponible.",
      "xml": "XML",
      "xmlDesc": "À utiliser quand le modèle formate XML plus fiablement que JSON.",
      "fallbackFormat": "Format de secours"
    }
  },
  "usageAnalytics": {
    "page": {
      "filters": "Filtres",
      "model": "Modèle",
      "character": "Personnage",
      "clearAll": "Tout effacer",
      "applyFilters": "Appliquer les filtres",
      "recentActivity": "Activité récente",
      "customRange": "Plage personnalisée",
      "startDate": "Date de début",
      "endDate": "Date de fin",
      "applyRange": "Appliquer la plage",
      "dashboard": "TABLEAU DE BORD",
      "appTime": "TEMPS D'UTILISATION",
      "today": "Aujourd'hui",
      "last7Days": "7 jours",
      "last30Days": "30 jours",
      "all": "Tout",
      "custom": "Personnalisé",
      "filtersCount": "{{count}} filtres",
      "totalCost": "Coût total",
      "tokens": "Tokens",
      "avgShort": "{{value}} moy.",
      "requests": "Requêtes",
      "period": "Période",
      "last7d": "7 derniers j.",
      "last30d": "30 derniers j.",
      "allTime": "Depuis toujours",
      "recordsCount": "{{count}} enregistrements",
      "usageTrend": "Tendance d'utilisation",
      "tokenConsumptionOverTime": "Consommation de tokens dans le temps",
      "input": "Entrée",
      "output": "Sortie",
      "byModel": "Par modèle",
      "byCharacter": "Par personnage",
      "top": "Top",
      "active": "Actif",
      "quickView": "Aperçu rapide",
      "viewAll": "Tout afficher",
      "startChatting": "Commencez à discuter pour voir les données d'utilisation",
      "exportedTo": "Exporté vers : {{path}}",
      "periodTotal": "Total de la période",
      "daysActive": "{{count}} jours actifs",
      "dailyAvg": "Moy. quotidienne",
      "selectedPeriod": "période sélectionnée",
      "yesterdayValue": "Hier {{value}}",
      "thirtyDayAvg": "Moy. sur 30 jours",
      "appTimeTrend": "Tendance du temps d'utilisation",
      "usageDurationPerDay": "Durée d'utilisation par jour",
      "totalValue": "Total {{value}}",
      "activeTime": "Temps actif"
    },
    "activity": {
      "loading": "Chargement de votre activité...",
      "title": "Activité récente",
      "recordsCount": "{{count}} enregistrements d'utilisation",
      "rangeOfTotal": "{{start}}-{{end}} sur {{total}}",
      "previous": "Précédent",
      "next": "Suivant",
      "pageOf": "Page {{page}} sur {{total}}"
    },
    "shared": {
      "relativeTime": {
        "justNow": "à l'instant",
        "minutesAgo": "il y a {{count}}m",
        "hoursAgo": "il y a {{count}}h",
        "daysAgo": "il y a {{count}}j"
      },
      "operations": {
        "chat": "Chat",
        "regenerate": "Régén.",
        "continue": "Continuer",
        "summary": "Résumé",
        "memoryManager": "Mémoire",
        "imageGeneration": "Génération d'image",
        "aiCreator": "Créateur IA",
        "replyHelper": "Aide à la réponse",
        "groupChatMessage": "Chat de groupe",
        "groupChatRegenerate": "Régén. groupe",
        "groupChatContinue": "Continuer groupe",
        "groupChatDecisionMaker": "Décideur"
      },
      "outputImages": "{{count}} images",
      "tokens": "{{count}} tokens",
      "unknown": "Inconnu",
      "requestDetails": "Détails de la requête",
      "noStopReason": "Aucune raison d'arrêt",
      "tokenUsage": "Utilisation des tokens",
      "estimatedCost": "Coût estimé",
      "stats": {
        "prompt": "Prompt",
        "completion": "Complétion",
        "total": "Total",
        "reasoning": "Raisonnement",
        "image": "Image",
        "memory": "Mémoire",
        "summary": "Résumé",
        "inputImages": "Images en entrée",
        "outputImages": "Images en sortie",
        "cachedPrompt": "Prompt en cache",
        "cacheWrite": "Écriture cache",
        "webSearches": "Recherches web",
        "cacheRead": "Lecture cache",
        "requestFee": "Frais de requête",
        "webSearch": "Recherche web",
        "providerTotal": "Total fournisseur"
      }
    }
  }
};
