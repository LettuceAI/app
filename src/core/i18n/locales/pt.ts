import type { DeepPartialMessageTree } from "../types";
import { type LocaleMessages } from "./en";
import type { LocaleMetadata } from ".";

export const ptMetadata: LocaleMetadata = {
  name: "Portuguese",
  label: "Português",
};

export const ptMessages: DeepPartialMessageTree<LocaleMessages> = {
  "common": {
    "nav": {
      "chats": "Conversas",
      "settings": "Configurações",
      "providers": "Provedores",
      "responseStyle": "Estilo de Resposta",
      "models": "Modelos",
      "security": "Segurança",
      "accessibility": "Acessibilidade",
      "reset": "Reiniciar",
      "backupRestore": "Backup e Restauração",
      "convertFiles": "Converter Arquivos",
      "usageAnalytics": "Análise de Uso",
      "changelog": "Registro de Alterações",
      "about": "Sobre",
      "createSystemPrompt": "Criar Prompt de Sistema",
      "editSystemPrompt": "Editar Prompt de Sistema",
      "systemPrompts": "Prompts de Sistema",
      "developer": "Desenvolvedor",
      "advanced": "Avançado",
      "characters": "Personagens",
      "lorebooks": "Livros de Lore",
      "personas": "Personas",
      "dynamicMemory": "Memória Dinâmica",
      "creationHelper": "Assistente de Criação",
      "helpMeReply": "Me Ajude a Responder",
      "editPersona": "Editar Persona",
      "newTemplate": "Novo Modelo",
      "editTemplate": "Editar Modelo",
      "chatTemplates": "Modelos de Conversa",
      "editCharacter": "Editar Personagem",
      "sync": "Sincronizar",
      "newCharacter": "Novo Personagem",
      "engineSetup": "Configuração do Motor",
      "llmProviders": "Provedores LLM",
      "engineSettings": "Configurações do Motor",
      "lettuceEngine": "Lettuce Engine",
      "create": "Criar",
      "setup": "Configuração",
      "welcome": "Bem-vindo",
      "conversation": "Conversa",
      "library": "Biblioteca",
      "groupChats": "Conversas em Grupo",
      "groupChat": "Conversa em Grupo",
      "imageGeneration": "Geração de Imagem",
      "voices": "Vozes",
      "chatAppearance": "Aparência do Chat",
      "colorCustomization": "Cores Personalizadas",
      "embeddingDownload": "Download de Embedding",
      "embeddingTest": "Teste de Embedding",
      "editModel": "Editar Modelo",
      "messageDebug": "Depuração de mensagem",
      "speechRecognition": "Speech Recognition",
      "hostApi": "API Server",
      "lorebookEntryGenerator": "Lorebook Entry Generator",
      "companionSoulWriter": "Escritor de Companion Soul"
    },
    "bottomNav": {
      "chats": "Conversas",
      "groups": "Grupos",
      "create": "Criar",
      "discover": "Descobrir",
      "library": "Biblioteca"
    },
    "buttons": {
      "cancel": "Cancelar",
      "confirm": "Confirmar",
      "save": "Salvar",
      "saving": "Salvando...",
      "delete": "Excluir",
      "deleting": "Excluindo...",
      "create": "Criar",
      "creating": "Criando...",
      "edit": "Editar",
      "back": "Voltar",
      "done": "Concluído",
      "download": "Baixar",
      "later": "Depois",
      "proceed": "Prosseguir",
      "retry": "Tentar novamente",
      "discard": "Descartar",
      "import": "Importar",
      "importing": "Importando...",
      "export": "Exportar",
      "exporting": "Exportando...",
      "update": "Atualizar",
      "generate": "Gerar",
      "refresh": "Atualizar",
      "continue": "Continuar",
      "goBack": "Voltar",
      "search": "Pesquisar",
      "clearSearch": "Limpar pesquisa",
      "add": "Adicionar",
      "remove": "Remover",
      "rename": "Renomear",
      "copy": "Copiar",
      "copied": "Copiado!",
      "browseFiles": "Procurar Arquivos",
      "install": "Instalar"
    },
    "labels": {
      "processing": "Processando...",
      "loading": "Carregando...",
      "noDescriptionYet": "Sem descrição ainda",
      "untitled": "Sem título",
      "default": "Padrão",
      "enabled": "Ativado",
      "disabled": "Desativado",
      "on": "Ligado",
      "off": "Desligado",
      "none": "Nenhum",
      "auto": "Automático",
      "custom": "Personalizado",
      "name": "Nome",
      "description": "Descrição",
      "content": "Conteúdo",
      "preview": "Pré-visualização",
      "options": "Opções"
    },
    "time": {
      "justNow": "Agora mesmo",
      "minutesAgo": "{{minutes}}min atrás",
      "hoursAgo": "{{hours}}h atrás",
      "daysAgo": "{{days}}d atrás",
      "today": "Hoje",
      "yesterday": "Ontem",
      "last7Days": "Últimos 7 dias",
      "older": "Mais antigos"
    }
  },
  "settings": {
    "sections": {
      "intelligence": "Inteligência",
      "experience": "Experiência",
      "connectivity": "Conectividade",
      "securityPrivacy": "Segurança e Privacidade",
      "supportInfo": "Suporte e Informações",
      "dangerZone": "Zona de Perigo",
      "developer": "Desenvolvedor"
    },
    "items": {
      "providers": {
        "title": "Provedores",
        "subtitle": "Conectar a serviços de IA"
      },
      "models": {
        "title": "Modelos",
        "subtitle": "Configurar modelos de IA"
      },
      "imageGeneration": {
        "title": "Geração de imagem",
        "subtitle": "Gerar e testar imagens"
      },
      "voices": {
        "title": "Vozes",
        "subtitle": "Vozes de texto para fala"
      },
      "accessibility": {
        "title": "Acessibilidade",
        "subtitle": "Sons e vibrações"
      },
      "prompts": {
        "title": "Prompts de Sistema",
        "subtitle": "Moldar a personalidade da IA"
      },
      "security": {
        "title": "Segurança",
        "subtitle": "Criptografia e privacidade"
      },
      "backup": {
        "title": "Backup e Restauração",
        "subtitle": "Exportar ou importar dados"
      },
      "convert": {
        "title": "Converter Arquivos",
        "subtitle": "Migrar exports .json legados para .uec"
      },
      "sync": {
        "title": "Sincronização Local",
        "subtitle": "Sincronizar entre dispositivos"
      },
      "usage": {
        "title": "Análise de Uso",
        "subtitle": "Custos e estatísticas de tokens"
      },
      "advanced": {
        "title": "Avançado",
        "subtitle": "Memória e recursos"
      },
      "logs": {
        "title": "Logs",
        "subtitle": "Debug e diagnósticos"
      },
      "guide": {
        "title": "Guia de Configuração",
        "subtitle": "Reiniciar integração"
      },
      "docs": {
        "title": "Documentação",
        "subtitle": "Guias e referência"
      },
      "github": {
        "title": "Reportar Problemas",
        "subtitle": "Bugs e feedback • v{{version}}"
      },
      "discord": {
        "title": "Entrar no Discord",
        "subtitle": "Comunidade e ajuda"
      },
      "about": {
        "title": "Sobre",
        "subtitle": "Versão, atualizações e links"
      },
      "changelog": {
        "title": "Registro de Alterações",
        "subtitle": "O que há de novo"
      },
      "reset": {
        "title": "Reiniciar",
        "subtitle": "Apagar tudo"
      },
      "developer": {
        "title": "Ferramentas de Desenvolvedor",
        "subtitle": "Debug e testes"
      }
    }
  },
  "accessibility": {
    "sectionTitles": {
      "language": "Idioma",
      "sounds": "Feedback Sonoro",
      "haptics": "Feedback Tátil",
      "appearance": "Aparência"
    },
    "language": {
      "appLanguage": "Idioma do aplicativo",
      "description": "Escolha o idioma usado na navegação e configurações."
    },
    "appearance": {
      "customColors": "Cores Personalizadas",
      "customColorsDesc": "Personalize o esquema de cores do aplicativo",
      "chatAppearance": "Aparência do Chat",
      "chatAppearanceDesc": "Personalize balões de mensagem, fontes e layout"
    },
    "haptics": {
      "vibrateOnChat": "Vibrar no Chat",
      "vibrateDesc": "Pulsos curtos de vibração enquanto o assistente digita",
      "intensity": "Intensidade",
      "light": "Leve",
      "medium": "Médio",
      "heavy": "Forte",
      "soft": "Suave",
      "rigid": "Rígido"
    },
    "sounds": {
      "send": "Enviar",
      "sendDescription": "Toca quando você envia uma mensagem",
      "success": "Sucesso",
      "successDescription": "Toca quando o assistente termina com sucesso",
      "failure": "Falha",
      "failureDescription": "Toca em caso de erro ou quando você cancela",
      "testButton": "Testar"
    },
    "feedbackInfo": "O feedback ajuda você a perceber quando mensagens são enviadas ou recebidas.",
    "hapticsInfo": "Vibrações estão disponíveis em dispositivos móveis.",
    "extra": {
      "easterEggs": "Easter Eggs",
      "beetrootRain": "Beetroot Rain",
      "beetrootDesc": "Beetroots fall when chats mention them"
    }
  },
  "topNav": {
    "unsavedChangesTitle": "Alterações não salvas",
    "unsavedChangesMessage": "Salve ou descarte suas alterações antes de sair.",
    "goBack": "Voltar",
    "changeLayout": "Alterar layout",
    "search": "Pesquisar",
    "settings": "Configurações",
    "help": "Ajuda",
    "add": "Adicionar",
    "openFilters": "Abrir filtros",
    "save": "Salvar",
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
      "title": "Nova versão disponível",
      "description": "A v{{latestVersion}} está disponível. Você está na v{{currentVersion}}.",
      "actions": {
        "view": "Ver versão"
      }
    }
  },
  "about": {
    "kicker": "Informações do app",
    "appName": "LettuceAI",
    "description": "Detalhes da versão, verificação de atualizações e links úteis.",
    "info": {
      "version": "Versão",
      "channel": "Canal",
      "platform": "Plataforma"
    },
    "buildChannel": {
      "dev": "Build de desenvolvimento",
      "release": "Versão estável"
    },
    "update": {
      "sectionTitle": "Atualizações",
      "title": "Atualizações do app",
      "description": "Verifique novas versões manualmente ou desative as verificações automáticas na inicialização.",
      "autoChecks": "Verificações automáticas de atualização",
      "autoChecksDescription": "Quando ativado, o LettuceAI verifica novas versões ao abrir o app.",
      "checkNow": "Verificar atualizações",
      "checking": "Verificando atualizações...",
      "upToDateTitle": "Você está em dia",
      "upToDateDescription": "Não há uma versão mais recente disponível para este dispositivo no momento.",
      "failedTitle": "Falha ao verificar atualizações",
      "failedDescription": "Não foi possível verificar atualizações agora. Tente novamente em instantes."
    },
    "links": {
      "sectionTitle": "Links",
      "website": "Site",
      "websiteDescription": "Página de download e informações de lançamento",
      "github": "GitHub",
      "githubDescription": "Código-fonte, issues e desenvolvimento",
      "discord": "Discord",
      "discordDescription": "Servidor da comunidade e suporte",
      "reddit": "Reddit",
      "redditDescription": "Discussões da comunidade, feedback e atualizações"
    },
    "developerMode": {
      "enable": "Ativar Modo Desenvolvedor",
      "enabled": "Modo Desenvolvedor Ativado"
    },
    "errors": {
      "saveTitle": "Não foi possível salvar a preferência",
      "saveDescription": "Sua preferência de verificação de atualizações não foi alterada."
    }
  },
  "components": {
    "tooltip": {
      "dismissHint": "Toque em qualquer lugar para fechar"
    },
    "backgroundPosition": {
      "title": "Posicionar Fundo",
      "instructions": "Arraste para posicionar • Aperte ou role para ampliar"
    },
    "avatarSource": {
      "generateImage": "Gerar Imagem",
      "generateImageDesc": "Criação de avatar com IA",
      "noImageModels": "Nenhum modelo de imagem disponível",
      "editCurrent": "Editar Atual",
      "editCurrentDesc": "Reposicionar ou cortar",
      "chooseImage": "Escolher Imagem",
      "chooseImageDesc": "Selecionar do seu dispositivo"
    },
    "avatarCurrentEdit": {
      "title": "Editar atual",
      "reposition": "Reposicionar",
      "repositionDesc": "Mover ou cortar o avatar atual",
      "editWithAI": "Edite com IA",
      "editWithAIDesc": "Abra a edição de IA para o avatar atual",
      "noImageModels": "Nenhum modelo de imagem disponível"
    },
    "avatarGeneration": {
      "modelsLoadError": "Falha ao carregar modelos de geração de imagem",
      "title": "Gerar Avatar",
      "help": "Ajuda com geração de avatar",
      "model": "Modelo",
      "selectModel": "Selecione um modelo",
      "describe": "Descreva seu avatar",
      "describePlaceholder": "Uma garota anime amigável com cabelo colorido, sorrindo calorosamente...",
      "inProgress": "Gerando avatar...",
      "editingInProgress": "Aplicando edição de avatar...",
      "previousVariant": "Variante anterior",
      "nextVariant": "Próxima variante",
      "variantCounter": "{{current}} / {{total}}",
      "editRequest": "Editar solicitação",
      "editRequestPlaceholder": "Escurecer o cabelo, colocar óculos, manter o rosto igual...",
      "applyEdit": "Aplicar Editar",
      "editImageLoadError": "Falha ao preparar o avatar gerado para edição",
      "aiAssistant": "Assistente de IA",
      "backToResults": "Voltar ao prompt",
      "magicInTheWorks": "Magia em ação...",
      "refine": "Refinar",
      "apply": "Aplicar",
      "alt": "Avatar gerado",
      "regenerate": "Regenerar",
      "useThis": "Usar Este"
    },
    "avatarPosition": {
      "title": "Posicionar Avatar",
      "instructions": "Arraste para posicionar • Aperte ou role para ampliar",
      "alt": "Avatar para posicionar"
    },
    "confirmDialog": {
      "defaultTitle": "Confirmar",
      "defaultLabel": "Confirmar"
    },
    "bottomMenu": {
      "defaultTitle": "Menu",
      "dragTip": "Arraste para fechar o menu",
      "closeLabel": "Fechar menu",
      "buttonProcessing": "Processando..."
    },
    "modelSelector": {
      "placeholder": "Selecione um modelo",
      "clearLabel": "Usar padrão global",
      "loading": "Carregando modelos...",
      "noModels": "Nenhum modelo disponível",
      "addProviderFirst": "Adicione um provedor nas configurações primeiro",
      "title": "Selecionar Modelo",
      "searchPlaceholder": "Pesquisar modelos...",
      "noResults": "Nenhum modelo encontrado",
      "noResultsHint": "Tente um termo de pesquisa diferente"
    },
    "localeSelector": {
      "title": "Selecione o idioma"
    },
    "promptTemplate": {
      "nameContentRequired": "Nome e conteúdo são obrigatórios",
      "saveError": "Falha ao salvar modelo",
      "editTitle": "Editar Prompt",
      "createTitle": "Criar Prompt",
      "name": "Nome",
      "namePlaceholder": "ex., Mestre de Roleplay",
      "content": "Conteúdo",
      "variablesButton": "Variáveis",
      "contentPlaceholder": "Você é um assistente de IA útil...\n\nUse {{char.name}} e {{scene}} no seu prompt.",
      "characterCount": "{{count}} caracteres",
      "preview": "Pré-visualização",
      "characterPlaceholder": "Personagem…",
      "personaPlaceholder": "Persona…",
      "rendering": "Renderizando…",
      "noPreview": "Sem pré-visualização ainda",
      "saving": "Salvando...",
      "update": "Atualizar",
      "create": "Criar",
      "variablesTitle": "Variáveis do Modelo",
      "variablesCopyHint": "Toque para copiar para a área de transferência",
      "variablesCopied": "Copiado",
      "variables": {
        "charName": "Nome do Personagem",
        "charNameDesc": "Nome do personagem",
        "charDesc": "Desc. do Personagem",
        "charDescDesc": "Descrição do personagem",
        "scene": "Cena",
        "sceneDesc": "Cena/cenário inicial",
        "userName": "Nome do Usuário",
        "userNameDesc": "Nome da persona do usuário",
        "userDesc": "Desc. do Usuário",
        "userDescDesc": "Descrição da persona do usuário",
        "rules": "Regras",
        "rulesDesc": "Regras comportamentais do personagem",
        "contextSummary": "Resumo do Contexto",
        "contextSummaryDesc": "Resumo dinâmico da conversa",
        "keyMemories": "Memórias Principais",
        "keyMemoriesDesc": "Lista de memórias relevantes"
      }
    },
    "characterExport": {
      "title": "Formato de Exportação",
      "selectFormat": "Selecione um formato",
      "loading": "Carregando formatos...",
      "formatUecDesc": "Formato Unified Entity Card (.uec) (recomendado).",
      "formatLegacyJsonDesc": "JSON Legado (somente importação).",
      "formatV3Desc": "Character Card V3 JSON (especificação mais recente).",
      "formatV2Desc": "Character Card V2 JSON (especificação Tavern).",
      "formatV1Desc": "Character Card V1 (somente importação)."
    },
    "embeddingDownload": {
      "downloadRequired": "Download Necessário",
      "modelRequired": "Modelo de Embedding Necessário",
      "description": "A Memória Dinâmica requer o download de um modelo de embedding local (~260 MB) para funcionar.",
      "localStorage": "• O modelo será armazenado localmente no seu dispositivo",
      "downloadSize": "• Tamanho do download: aproximadamente 260 MB",
      "summarization": "• Necessário para resumo de conversas"
    },
    "embeddingUpgrade": {
      "title": "Modelo de Embedding v3 Disponível",
      "v1Message": "Você está usando v1 com 512 tokens. Atualize para v3 para melhor qualidade de memória e suporte a contexto longo.",
      "v2Message": "Você está usando o v2 legado. Atualize para v3 para melhor qualidade de memória com o modelo de embedding mais recente.",
      "button": "Atualizar para v3",
      "v3Message": "v4 is out and dramatically improves roleplay memory recall over v3 (recall@1 0.02 -> 0.92). Upgrading is recommended."
    },
    "v2UpgradeToast": {
      "title": "Modelo de Memória v3",
      "badge": "Disponível",
      "message": "Qualidade de embedding melhorada em relação ao v2",
      "dismiss": "Dispensar",
      "upgrade": "Atualizar"
    },
    "v1UpgradeToast": {
      "title": "Modelo de Memória v3 Disponível",
      "message": "Atualize para melhor qualidade de memória e suporte a contexto longo.",
      "dismiss": "Dispensar",
      "upgrade": "Atualizar"
    },
    "createMenu": {
      "title": "Criar Novo",
      "smartCreator": "Criador Inteligente",
      "smartCreatorDesc": "Deixe o assistente guiar sua criação",
      "divider": "Ou crie manualmente",
      "character": "Personagem",
      "characterDesc": "Criar um personagem personalizado",
      "persona": "Persona",
      "personaDesc": "Criar uma voz reutilizável",
      "groupChat": "Conversa em Grupo",
      "groupChatDesc": "Conversar com vários personagens",
      "lorebook": "Livro de Lore",
      "lorebookDesc": "Construir sua referência de mundo",
      "characterSmartDesc": "Construir um personagem com criação guiada",
      "personaSmartDesc": "Criar uma voz ou personalidade reutilizável",
      "lorebookSmartDesc": "Construir uma referência de mundo estruturada",
      "loadingConversations": "Carregando conversas...",
      "createNew": "Criar novo",
      "createNewDesc": "Iniciar uma nova conversa de criação guiada",
      "editExisting": "Editar existente",
      "continueLast": "Continuar última conversa",
      "seeOlder": "Ver conversas anteriores",
      "seeOlderDesc": "Abrir qualquer conversa anterior do Criador Inteligente",
      "noConversations": "Nenhuma conversa ainda para este criador.",
      "sessionCompleted": "Concluído",
      "sessionCancelled": "Cancelado",
      "sessionDraft": "Rascunho",
      "sessionMessages": "{{count}} mensagens",
      "untitledConversation": "Conversa sem título",
      "nameLorebookTitle": "Nomear Livro de Lore",
      "lorebookNamePlaceholder": "Digite o nome do livro de lore...",
      "lorebookImporting": "Importando...",
      "lorebookImport": "Importar",
      "lorebookCreating": "Criando...",
      "lorebookCreate": "Criar",
      "editExistingDesc": "Escolha um(a) {{goal}} e edite com o Criador Inteligente",
      "creatorTitle": "Criador de {{goal}}",
      "editTitle": "Editar {{goal}}",
      "conversationsTitle": "Conversas de {{goal}}",
      "loadingItems": "Carregando {{items}}...",
      "noItemsFound": "Nenhum(a) {{items}} encontrado(a).",
      "unnamedCharacter": "Personagem sem nome",
      "untitledPersona": "Persona sem título",
      "untitledLorebook": "Livro de lore sem título"
    },
    "advancedModelSettings": {
      "temperature": "Temperatura",
      "temperatureDesc": "Mais alto = mais criativo",
      "topP": "Top P",
      "topPDesc": "Mais baixo = mais focado",
      "maxTokens": "Máx. Tokens de Saída",
      "maxTokensDesc": "Deixe em branco para o padrão",
      "contextLength": "Comprimento do Contexto",
      "contextLengthDesc": "Somente modelos locais",
      "contextLengthAuto": "Automático",
      "frequencyPenalty": "Penalidade de Frequência",
      "frequencyPenaltyDesc": "Reduzir repetição de tokens",
      "presencePenalty": "Penalidade de Presença",
      "presencePenaltyDesc": "Incentivar novos tópicos",
      "topK": "Top K",
      "topKDesc": "Limitar tamanho do pool de tokens",
      "reasoning": "Pensamento / Raciocínio",
      "reasoningAutoDesc": "Este modelo sempre usa raciocínio. Nenhuma configuração necessária.",
      "reasoningEnableDesc": "Habilitar capacidades avançadas de pensamento para resolução de problemas complexos e tarefas de raciocínio.",
      "effortMode": "Modo de Esforço",
      "budgetMode": "Modo de Orçamento",
      "reasoningEffort": "Esforço de Raciocínio",
      "reasoningEffortDesc": "Controla a profundidade do pensamento",
      "reasoningEffortAuto": "Automático",
      "reasoningEffortLow": "Baixo",
      "reasoningEffortMed": "Médio",
      "reasoningEffortHigh": "Alto",
      "reasoningBudget": "Orçamento de Raciocínio (tokens)",
      "reasoningBudgetDesc": "Máx. tokens reservados para pensamento",
      "reasoningEffortLowDesc": "Respostas rápidas com raciocínio mínimo",
      "reasoningEffortMediumDesc": "Profundidade de raciocínio equilibrada",
      "reasoningEffortHighDesc": "Profundidade máxima de raciocínio para problemas complexos",
      "reasoningBudgetExtendedDesc": "Máx. tokens reservados para pensamento. Adicionado ao limite de saída."
    },
    "providerParameterSupport": {
      "unknownProvider": "Provedor desconhecido: {{providerId}}",
      "reasoningNotSupportedEffort": "Não suportado - este provedor não usa níveis de esforço",
      "reasoningNotSupportedBudgetOnly": "Não suportado - este provedor usa abordagem somente de orçamento",
      "reasoningNotSupported": "Não suportado - este provedor não suporta raciocínio",
      "unsupportedParametersIgnored": "Parâmetros não suportados serão ignorados por {{providerName}}.",
      "reasoningEffortSupported": "Esforço de raciocínio é suportado para modelos pensantes (o1, DeepSeek-R1, etc.)",
      "reasoningBudgetSupported": "Este provedor usa pensamento baseado em orçamento (sem níveis de esforço). Defina tokens de orçamento de raciocínio em vez disso.",
      "reasoningNotSupportedProvider": "Este provedor não suporta parâmetros de raciocínio.",
      "matrixTitle": "Matriz de Suporte de Parâmetros do Provedor",
      "providerColumn": "Provedor",
      "supportedIndicator": "Suportado pela API do provedor",
      "notSupportedIndicator": "Não suportado (parâmetro será ignorado)"
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
    "characterNotFound": "Personagem não encontrado",
    "chatHistory": "Histórico de Conversas",
    "previousConversationsWithCharacter": "Conversas anteriores com {{name}}",
    "previousConversations": "Conversas anteriores",
    "searchChats": "Pesquisar conversas...",
    "searchResults": "{{count}} resultado(s)",
    "noConversationsYet": "Nenhuma conversa ainda",
    "startChattingPrompt": "Comece a conversar para ver seu histórico aqui",
    "noMatchingChats": "Nenhuma conversa correspondente",
    "tryDifferentSearchTerm": "Tente um termo de pesquisa diferente",
    "untitledChat": "Conversa sem título",
    "chatTitlePlaceholder": "Título da conversa...",
    "exportChatPackage": "Exportar Pacote de Conversa",
    "characterSpecificExport": "Exportação Específica do Personagem",
    "characterSpecificExportDesc": "Vincular este pacote ao personagem da conversa por padrão.",
    "nonCharacterSpecificExport": "Exportação Não Específica",
    "nonCharacterSpecificExportDesc": "Exigir seleção de personagem durante a importação.",
    "deleteChat": "Excluir conversa?",
    "deleteConfirmDesc": "Remove permanentemente do histórico",
    "keepThisChat": "Manter esta conversa",
    "editCharacter": "Editar Personagem",
    "exportCharacter": "Exportar Personagem",
    "chatAppearance": "Aparência do Chat",
    "importChatPackage": "Importar Pacote de Conversa",
    "hideThisCharacter": "Ocultar este personagem",
    "deleteCharacter": "Excluir Personagem",
    "deleteCharacterTitle": "Excluir Personagem?",
    "deleteCharacterConfirmation": "Tem certeza de que deseja excluir \"{{name}}\"? Isso também excluirá todas as sessões de conversa com este personagem.",
    "characterSpecificMatches": "Este pacote é específico do personagem e corresponde ao personagem selecionado.",
    "characterSpecificMismatch": "Este pacote é específico do personagem e aponta para outro personagem. Será importado para {{name}}.",
    "nonCharacterSpecificImport": "Este pacote não é específico de personagem. Será importado para {{name}}.",
    "noCharactersYet": "Nenhum personagem ainda",
    "createFirstCharacter": "Crie seu primeiro personagem pelo botão + abaixo para começar a conversar",
    "scrollToBottom": "Rolar para o final",
    "selectCharacter": "Selecionar Personagem",
    "branchToGroupChat": "Ramificar para Conversa em Grupo",
    "addContent": "Adicionar Conteúdo",
    "swapPlaces": "Trocar Posições",
    "swapPlacesOn": "Trocar Posições (Ativo)",
    "uploadImage": "Enviar Imagem",
    "helpMeReply": "Me Ajude a Responder",
    "sceneImage": {
      "modeTitle": "Imagem da cena",
      "modeDescription": "Escolha se você mesmo deseja escrever a cena ou deixe a IA esboçá-la primeiro.",
      "writePrompt": "Solicitação de gravação",
      "writePromptDesc": "Digite o prompt exato da imagem da cena que deseja usar.",
      "askAi": "Pergunte à IA",
      "askAiDesc": "Deixe o modelo de bate-papo atual esboçar um prompt de cena a partir do momento selecionado.",
      "generateTitle": "Gerar imagem de cena",
      "regenerateTitle": "Regenerar imagem da cena",
      "aiTitle": "Prompt de cena AI",
      "promptLabel": "AVISO DE CENA",
      "promptPlaceholder": "Descreva a cena, personagens, clima, iluminação, enquadramento da câmera e detalhes importantes...",
      "suggestedPrompt": "Solicitação sugerida",
      "regeneratePrompt": "Regenerado",
      "editPrompt": "Editar solicitação",
      "reviewTitle": "Revisar prompt de cena",
      "denyPrompt": "Negar",
      "acceptPrompt": "Aceitar",
      "generateImage": "Gerar imagem",
      "updateImage": "Atualizar imagem"
    },
    "useMyTextAsBase": "Usar meu texto como base",
    "writeNewReply": "Escrever algo novo",
    "suggestedReply": "Resposta Sugerida",
    "selectTwoCharactersMinimum": "Selecione pelo menos 2 personagens para uma conversa em grupo.",
    "groupBranchCreated": "Ramificação de grupo criada! Redirecionando...",
    "memories": "Memórias",
    "tools": "Ferramentas",
    "pinned": "Fixadas",
    "searchMemories": "Pesquisar memórias...",
    "addMemory": "Adicionar Memória",
    "memoryActions": "Ações de memória",
    "pinnedMessages": "Mensagens Fixadas",
    "pinnedMessagesDesc": "Sempre incluídas no contexto",
    "contextSummary": "Resumo do Contexto",
    "contextSummaryPlaceholder": "Resumo curto usado para manter o contexto consistente entre mensagens...",
    "memoryContentPlaceholder": "O que deve ser lembrado?",
    "editMemoryPlaceholder": "Digite o conteúdo da memória...",
    "togglePin": {
      "pin": "Fixar",
      "unpin": "Desfixar"
    },
    "toggleMemoryState": {
      "setHot": "Definir Quente",
      "setCold": "Definir Fria"
    },
    "selectModelForRetry": "Selecionar Modelo para Nova Tentativa",
    "memoryCategories": {
      "characterTrait": "traço do personagem",
      "relationship": "relacionamento",
      "plotEvent": "evento da trama",
      "worldDetail": "detalhe do mundo",
      "preference": "preferência",
      "other": "outro"
    },
    "settings": {
      "memorySection": "Memória",
      "memorySectionDesc": "Resumo, tags, histórico de chamadas de ferramentas",
      "quickSettings": "Configurações Rápidas",
      "quickSettingsDesc": "Ajustes mais comuns",
      "persona": "Persona",
      "model": "Modelo",
      "fallbackModel": "Modelo Alternativo",
      "voice": "Voz",
      "voiceDesc": "Reprodução de texto para fala",
      "advanced": "Avançado",
      "advancedDesc": "Substituir parâmetros do modelo para esta sessão",
      "session": "Sessão",
      "sessionDesc": "Iniciar novas conversas e navegar no histórico",
      "newChat": "Nova Conversa",
      "newChatDesc": "Iniciar uma conversa nova",
      "chatHistoryDesc": "Ver sessões anteriores",
      "importChatPackageDesc": "Importar um .chatpkg para este personagem",
      "selectModel": "Selecionar Modelo",
      "selectFallbackModel": "Selecionar Modelo Alternativo",
      "personaActions": "Ações de Persona",
      "sessionAdvancedSettings": "Configurações Avançadas da Sessão",
      "parameterSupport": "Suporte a Parâmetros",
      "backToChat": "Voltar ao chat",
      "chatSettingsTitle": "Configurações do chat",
      "chatSettingsSubtitle": "Gerencie as preferências da conversa",
      "modelDefaults": "Padrões do modelo",
      "appDefaults": "Padrões do app",
      "openChatSessionFirst": "Abra uma sessão de chat primeiro",
      "sessionRequired": "Sessão obrigatória",
      "noPersona": "Sem persona",
      "customPersona": "Persona personalizada",
      "noModelAvailable": "Nenhum modelo disponível",
      "fallbackNone": "Nenhum",
      "unknownModel": "Modelo desconhecido",
      "authorNote": "Nota do autor",
      "identityProfileAuthored": "Perfil de identidade criado",
      "addIdentityProfile": "Adicionar perfil de identidade do companheiro",
      "soulLabel": "Alma",
      "sessionTitle": "Sessão: {{title}}",
      "sessionUntitled": "Untitled",
      "messageCount": "{{count}} messages",
      "usingCharacterDefault": "Using character default",
      "sessionOverrideActive": "Session override active",
      "autoplayVoice": "Autoplay voice",
      "useCharacterDefault": "Use character default"
    },
    "search": {
      "placeholder": "Pesquisar conversa...",
      "noMessagesFound": "Nenhuma mensagem encontrada",
      "you": "Você",
      "character": "Personagem",
      "failed": "Falha ao pesquisar mensagens"
    },
    "templates": {
      "startWithTemplate": "Começar com um modelo?",
      "noTemplate": "Sem modelo",
      "startWithSceneOnly": "Começar somente com cena",
      "you": "Você",
      "bot": "Bot",
      "messageCount": "{{count}} mensagem(ns)"
    },
    "header": {
      "back": "Voltar",
      "openSettings": "Abrir configurações do chat",
      "manageMemories": "Gerenciar memórias",
      "searchMessages": "Pesquisar mensagens",
      "manageLorebooks": "Gerenciar livros de lore",
      "conversationSettings": "Configurações da conversa"
    },
    "footer": {
      "sendMessagePlaceholder": "Enviar uma mensagem...",
      "stopGeneration": "Parar geração",
      "sendMessage": "Enviar mensagem",
      "continueConversation": "Continuar conversa",
      "moreOptions": "Mais opções",
      "addImage": "Adicionar imagem",
      "addImageAttachment": "Adicionar anexo de imagem",
      "removeAttachment": "Remover anexo",
      "recordVoice": "Gravar voz"
    },
    "message": {
      "thinkingMessages": {
        "thinkingReallyHard": "Pensando muito…",
        "lettuceCouncil": "Consultando o conselho de alfaces…",
        "stealingThoughts": "Roubando pensamentos do vazio…",
        "warmingBrainCells": "Aquecendo os neurônios…",
        "forbiddenKnowledge": "Carregando conhecimento proibido…",
        "overthinking": "Pensando demais (como sempre)…",
        "pretendingToBeSmart": "Fingindo ser inteligente…",
        "crunchingNumbers": "Calculando números imaginários…",
        "arguingWithMyself": "Discutindo comigo mesmo…",
        "askingUniverse": "Pedindo gentilmente ao universo…"
      },
      "thoughtProcess": "Processo de pensamento",
      "regenerateResponse": "Regenerar resposta",
      "cancelAudioGeneration": "Cancelar geração de áudio",
      "stopAudio": "Parar áudio",
      "playMessageAudio": "Reproduzir áudio da mensagem",
      "playAudio": "Reproduzir áudio",
      "sceneLabel": "Cena",
      "variantLabel": "Variante",
      "regenerating": "Regenerando",
      "assistantIsTyping": "O assistente está digitando",
      "attachedImage": "Imagem anexada",
      "guidedRegenerationTitle": "Guide regeneration",
      "guidedRegenerationLabel": "How should it change?",
      "guidedRegenerationDescription": "Describe the tone, length, details to keep or remove, and anything the next reply should do differently.",
      "guidedRegenerationPlaceholder": "Make it shorter, warmer, more direct...",
      "guidedRegenerationSubmit": "Regenerate"
    },
    "actions": {
      "assistantMessage": "Mensagem do Assistente",
      "userMessage": "Mensagem do Usuário",
      "promptTokens": "Tokens de Prompt",
      "completionTokens": "Tokens de Conclusão",
      "fallbackModelUsed": "Modelo alternativo usado",
      "total": "total",
      "timeToFirstToken": "Tempo até o primeiro token",
      "completionTokenSpeed": "Velocidade de tokens de conclusão",
      "edit": "Editar",
      "copy": "Copiar",
      "pin": "Fixar",
      "unpin": "Desfixar",
      "rewindToHere": "Retroceder até aqui",
      "branchFromHere": "Ramificar daqui",
      "branchToGroupChat": "Ramificar para conversa em grupo",
      "branchToCharacter": "Ramificar para personagem",
      "generateSceneImage": "Gerar imagem de cena",
      "regenerateSceneImage": "Regenerar imagem da cena",
      "chatAppearance": "Aparência do Chat",
      "delete": "Excluir",
      "debug": "Depurar",
      "unpinToDelete": "Desfixe para excluir",
      "editPlaceholder": "Edite sua mensagem...",
      "memoriesUsed": "{{count}} memórias usadas",
      "lorebookUsage": "Uso do livro de lore",
      "lorebookUsageDesc": "Esta resposta usou as seguintes entradas do livro de lore.",
      "matchScore": "Correspondência: {{score}}%",
      "unknownModel": "Modelo desconhecido",
      "loadingModel": "Carregando modelo..."
    },
    "emptyState": {
      "goBack": "Voltar"
    },
    "settingsDrawer": {
      "title": "Configurações do Chat",
      "subtitle": "Gerenciar preferências da conversa"
    },
    "history": {
      "archivedBadge": "Archived",
      "messagesCount": "{{count}} messages",
      "previousGroupPage": "Previous {{label}} page",
      "nextGroupPage": "Next {{label}} page",
      "sillyTavernFormat": "SillyTavern format",
      "sillyTavernFormatDesc": "Export as .jsonl compatible with SillyTavern.",
      "failedLoad": "Failed to load data",
      "failedDelete": "Failed to delete: {{error}}",
      "failedRename": "Failed to rename: {{error}}",
      "chatPackageExportedTo": "Chat package exported to:\n{{path}}",
      "sillyTavernExportedTo": "Chat do SillyTavern exportado para:\n{{path}}",
      "failedExportChatPackage": "Falha ao exportar o pacote de chat",
      "failedExportSillyTavern": "Falha ao exportar o chat do SillyTavern"
    },
    "authorNote": {
      "title": "Nota do autor",
      "inlineEditor": "Editor inline",
      "inlineEditorDesc": "Mostra a nota do autor acima da entrada do chat para edições rápidas.",
      "toggleInlineEditor": "Alternar editor inline da nota do autor",
      "placeholder": "Direção privada para este chat",
      "willBeRemoved": "A nota do autor será removida ao salvar",
      "noNoteSaved": "Nenhuma nota do autor salva",
      "charactersCount": "{{count}} characters",
      "clear": "Clear",
      "save": "Salvar",
      "saving": "Saving...",
      "failedSave": "Falha ao salvar a nota do autor",
      "addPlaceholder": "Adicionar uma nota do autor...",
      "savingShort": "Salvando..."
    },
    "drawer": {
      "chatSettingsTitle": "Configurações do chat",
      "chatSettingsSubtitle": "Gerencie as preferências da conversa"
    },
    "companionSoul": {
      "loading": "Carregando Companion Soul...",
      "unavailable": "Companion Soul não está disponível",
      "unavailableDesc": "A edição da alma só está disponível para personagens no modo companion.",
      "pageTitle": "Companion Soul",
      "back": "Voltar",
      "save": "Salvar"
    },
    "companionRelationship": {
      "back": "Voltar",
      "loading": "Carregando estado da relação...",
      "unavailableTitle": "O estado da relação não está disponível",
      "sessionLoadFailed": "A sessão de chat não pôde ser carregada.",
      "backToChat": "Voltar ao chat",
      "notCompanionTitle": "Este chat não está no modo companion",
      "notCompanionDesc": "As páginas de relação companion só são renderizadas para chats cujo modo de personagem é companion.",
      "openRegularMemories": "Abrir memórias normais",
      "pageTitle": "Estado da relação",
      "memoryButton": "Memória",
      "lastInteraction": "Última interação {{time}}",
      "bond": "Vínculo",
      "closeness": "Proximidade",
      "trust": "Confiança",
      "affection": "Afeto",
      "tension": "Tensão",
      "stability": "Estabilidade",
      "interactions": "Interações",
      "vsDefaults": "vs. character defaults",
      "updatedAt": "Updated {{time}}",
      "emotionalEngine": "Emotional engine",
      "felt": "Felt",
      "feltDesc": "Internal affect",
      "expressed": "Expressed",
      "expressedDesc": "Surfaces in replies",
      "blocked": "Blocked",
      "blockedDesc": "Suppressed by persona",
      "momentum": "Momentum",
      "momentumDesc": "Trend over recent turns",
      "activeDrivers": "Active drivers",
      "soul": "Soul",
      "essence": "Essence",
      "voice": "Voice",
      "relationalStyle": "Relational style",
      "vulnerabilities": "Vulnerabilities",
      "habits": "Habits",
      "boundaries": "Boundaries",
      "eventsCount": "{{count}} events",
      "recentTimeline": "Recent timeline",
      "superseded": "superseded",
      "promptScore": "Prompt {{score}}",
      "persistenceScore": "Persistence {{score}}",
      "noTimeline": "No timeline yet",
      "noTimelineDesc": "Relationship, milestone, and emotional snapshot memories will appear here as the companion learns from conversations.",
      "notAuthoredYet": "Not authored yet.",
      "noSignal": "No signal."
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
      "unknownTime": "Unknown",
      "justNow": "Just now",
      "minutesAgo": "{{minutes}}m ago",
      "hoursAgo": "{{hours}}h ago",
      "daysAgo": "{{days}}d ago",
      "notSetYet": "Not set yet",
      "missingCharacterId": "Missing characterId",
      "sessionNotFound": "Session not found",
      "failedLoadCompanion": "Failed to load companion session"
    },
    "chatPage": {
      "characterNotFound": "Character not found",
      "characterDoesntExist": "The character you're looking for doesn't exist."
    },
    "debugPage": {
      "copy": "Copy"
    },
    "companionMemoryPage": {
      "backLabel": "Back",
      "refineMemoryPlaceholder": "Refine the stored companion memory",
      "superseded": "superseded",
      "pinned": "pinned",
      "cold": "cold"
    }
  },
  "groupChats": {
    "list": {
      "editGroup": "Editar Grupo",
      "deleteGroup": "Excluir Grupo",
      "deleteConfirmTitle": "Excluir Conversa em Grupo?",
      "deleteConfirmMessage": "Tem certeza de que deseja excluir \"{{name}}\"? Isso também excluirá todas as mensagens nesta conversa em grupo.",
      "noGroupChatsYet": "Nenhuma conversa em grupo ainda",
      "noGroupChatsDesc": "Crie sua primeira conversa em grupo pelo botão + abaixo para conversar com vários personagens ao mesmo tempo",
      "newChat": "Nova conversa",
      "openChat": "Abrir conversa",
      "chatSettings": "Configurações da conversa",
      "sessionCount": "{{count}} conversas"
    },
    "create": {
      "invalidPackage": "Este pacote não é um pacote de conversa em grupo.",
      "inspectPackageError": "Falha ao inspecionar pacote de conversa em grupo",
      "importPackageError": "Falha ao importar pacote de conversa em grupo",
      "importChatpkg": "Importar `.chatpkg`",
      "mapParticipantsTitle": "Mapear Participantes",
      "selectLocalCharacter": "Selecione o personagem local para este participante.",
      "selectCharacterPlaceholder": "Selecionar personagem...",
      "importChatPackageTitle": "Importar Pacote de Conversa",
      "importChatPackageDesc": "Isso importará o `.chatpkg` selecionado como uma nova sessão de grupo.",
      "characterSelect": {
        "title": "Criar Conversa em Grupo",
        "subtitle": "Selecione personagens para sua conversa em grupo",
        "selected": "selecionados",
        "ready": "Pronto",
        "minRequired": "Mín. 2 necessários",
        "noCharactersYet": "Nenhum personagem ainda",
        "noCharactersDesc": "Crie alguns personagens primeiro para iniciar uma conversa em grupo",
        "continueToSetup": "Continuar para Configuração do Grupo"
      },
      "groupSetup": {
        "title": "Configuração do Grupo",
        "subtitle": "Configure as opções da sua conversa em grupo",
        "chatType": "Tipo de Conversa",
        "conversation": "Conversa",
        "casualChat": "Conversa casual",
        "roleplay": "Roleplay",
        "withScenes": "Com cenas",
        "conversationDesc": "Conversa casual em grupo sem cenas iniciais",
        "roleplayDesc": "Cenário de roleplay com cena inicial e prompts imersivos",
        "speakerSelection": "Seleção de Falante",
        "llm": "LLM",
        "aiPicks": "IA escolhe",
        "heuristic": "Heurística",
        "scoreBased": "Baseado em pontuação",
        "roundRobin": "Round Robin",
        "takeTurns": "Revezamento",
        "llmDesc": "Usa seu modelo padrão para escolher quem fala (custa tokens)",
        "heuristicDesc": "Usa equilíbrio de participação e pistas de contexto (gratuito)",
        "roundRobinDesc": "Personagens se revezam em ordem (gratuito)",
        "chatBackground": "Fundo do Chat",
        "optional": "(Opcional)",
        "uploadBackground": "Enviar imagem de fundo",
        "backgroundDesc": "Definir uma imagem de fundo para esta conversa em grupo",
        "groupName": "Nome do Grupo",
        "removeBackground": "Remover imagem de fundo",
        "groupNameAutoGenerate": "Deixe vazio para gerar automaticamente a partir dos nomes dos personagens",
        "continueToScene": "Continuar para Cena Inicial",
        "createGroupChat": "Criar Conversa em Grupo"
      },
      "startingScene": {
        "title": "Cena Inicial",
        "subtitle": "Defina o cenário de abertura para seu roleplay",
        "sceneSource": "Origem da Cena",
        "none": "Nenhuma",
        "custom": "Personalizada",
        "fromCharacter": "De Personagem",
        "noneDesc": "Começar sem uma cena predefinida",
        "customDesc": "Escrever seu próprio cenário de abertura",
        "fromCharacterDesc": "Usar uma cena de um dos seus personagens",
        "sceneContent": "Conteúdo da Cena",
        "sceneContentPlaceholder": "Descreva a cena inicial para este roleplay...",
        "sceneReferenceTip": "Dica: Digite {{@\" para referenciar personagens",
        "selectScene": "Selecionar uma Cena",
        "sceneLabel": " - Cena",
        "copyToCustom": "Copiar para Personalizada e Editar"
      }
    },
    "history": {
      "title": "Histórico de Conversas em Grupo",
      "subtitle": "Todas as conversas em grupo",
      "searchPlaceholder": "Pesquisar conversas em grupo...",
      "active": "Ativas ({{count}})",
      "archived": "Arquivadas ({{count}})",
      "noChatsYet": "Nenhuma conversa em grupo ainda",
      "noChatsDesc": "Crie uma conversa em grupo para ver seu histórico aqui",
      "noMatchingChats": "Nenhuma conversa correspondente",
      "noMatchingDesc": "Tente um termo de pesquisa diferente",
      "deleteSessionTitle": "Excluir conversa em grupo?",
      "deleteSessionDesc": "Remove permanentemente do histórico",
      "deleteSessionButton": "Excluir conversa",
      "keepChat": "Manter esta conversa"
    },
    "session": {
      "chatTitlePlaceholder": "Título da conversa...",
      "newChat": "Nova Conversa",
      "rename": "Renomear",
      "unarchive": "Desarquivar",
      "archive": "Arquivar",
      "messageCount": "{{count}} mensagem(ns)"
    },
    "memories": {
      "tabMemories": "Memórias",
      "tabPinned": "Fixadas",
      "tabActivity": "Atividade",
      "processing": "Processando",
      "contextSummaryTitle": "Resumo do Contexto",
      "addContextSummaryPrompt": "Toque para adicionar um resumo do contexto...",
      "savedMemories": "Memórias Salvas",
      "resultsCount": "Resultados ({{count}})",
      "searchPlaceholder": "Pesquisar memórias...",
      "addMemory": "Adicionar memória",
      "noMemoriesYet": "Nenhuma memória ainda",
      "noMemoriesDesc": "Toque no botão Adicionar acima para criar uma",
      "noMatchingMemories": "Nenhuma memória correspondente",
      "noMatchingDesc": "Tente um termo de pesquisa diferente",
      "sessionNotFound": "Sessão não encontrada",
      "memoryActions": "Ações de memória",
      "tokens": "tokens",
      "cycle": "Ciclo",
      "accessed": "Acessada",
      "cold": "Fria",
      "hot": "Quente",
      "activityLog": "Registro de Atividade",
      "events": "eventos",
      "run": "Executar",
      "processingMemories": "A IA está organizando memórias do grupo...",
      "memoryCycleSuccess": "Ciclo de memória processado com sucesso!",
      "memoryActionFailed": "Ação de memória falhou",
      "newMemoryUpdates": "Novas atualizações de memória disponíveis",
      "noActivityYet": "Nenhuma atividade ainda",
      "noActivityDesc": "Chamadas de ferramentas aparecem quando a IA gerencia memórias no modo dinâmico",
      "contextSummaryPlaceholder": "Resumo curto usado para manter o contexto consistente entre mensagens...",
      "addMemoryTitle": "Adicionar Memória",
      "memoryPlaceholder": "O que deve ser lembrado?",
      "saveMemory": "Salvar Memória",
      "editMemoryTitle": "Editar Memória",
      "editMemoryPlaceholder": "Digite o conteúdo da memória...",
      "edit": "Editar",
      "pin": "Fixar",
      "unpin": "Desfixar",
      "setHot": "Definir Quente",
      "setCold": "Definir Fria"
    },
    "toolLog": {
      "created": "Criado",
      "deleted": "Excluído",
      "pinned": "Fixado",
      "unpinned": "Desfixado",
      "done": "Concluído",
      "pinnedBadge": "fixado",
      "softDelete": "exclusão suave",
      "memoryCycle": "Ciclo de Memória",
      "failedAt": "Falhou em:",
      "window": "Janela",
      "justNow": "just now",
      "minutesAgo": "{{count}}m ago",
      "hoursAgo": "{{count}}h ago",
      "yesterday": "yesterday",
      "daysAgo": "{{count}}d ago",
      "revertingTitle": "Reverting...",
      "revertCycleTitle": "Revert this cycle",
      "revertedAt": "Reverted {{time}}",
      "failedAtStage": "Failed at: {{stage}}",
      "hideDebug": "Hide Debug",
      "debug": "Debug",
      "windowRange": "Window {{start}}-{{end}}",
      "actionCreated": "Created",
      "actionDeleted": "Deleted",
      "actionPinned": "Pinned",
      "actionUnpinned": "Unpinned",
      "actionDone": "Done",
      "badgePinned": "pinned",
      "badgeSoftDelete": "soft-delete",
      "badgeUndone": "undone",
      "badgeReverted": "reverted",
      "activityEmptyTitle": "No activity yet",
      "activityEmptyDesc": "Tool calls appear when AI manages memories in dynamic mode"
    },
    "message": {
      "thinkingHard": "Pensando muito…",
      "thinkingLettuce": "Consultando o conselho de alfaces…",
      "thinkingVoid": "Roubando pensamentos do vazio…",
      "thinkingBrainCells": "Aquecendo os neurônios…",
      "thinkingForbidden": "Carregando conhecimento proibido…",
      "thinkingOverthinking": "Pensando demais (como sempre)…",
      "thinkingPretending": "Fingindo ser inteligente…",
      "thinkingCrunching": "Calculando números imaginários…",
      "thinkingArguing": "Discutindo comigo mesmo…",
      "thinkingUniverse": "Pedindo gentilmente ao universo…",
      "thoughtProcess": "Processo de pensamento",
      "userAlt": "Usuário",
      "assistantAlt": "Assistente",
      "regenerateResponse": "Regenerar resposta",
      "variantLabel": "Variante",
      "regenerating": "Regenerando",
      "cancelAudioGeneration": "Cancelar geração de áudio",
      "stopAudio": "Parar áudio",
      "playMessageAudio": "Reproduzir áudio da mensagem",
      "playAudio": "Reproduzir áudio",
      "attachedImage": "Imagem anexada",
      "assistantIsTyping": "O assistente está digitando",
      "assistantTyping": "Assistant is typing"
    },
    "header": {
      "back": "Voltar",
      "memories": "Memórias",
      "settings": "Configurações",
      "characters": "personagens"
    },
    "footer": {
      "mentionCharacter": "Mencionar um personagem",
      "noCharactersFound": "Nenhum personagem encontrado",
      "moreOptions": "Mais opções",
      "addImage": "Adicionar imagem",
      "messagePlaceholder": "Mensagem... (@ para mencionar)",
      "stopGeneration": "Parar geração",
      "sendMessage": "Enviar mensagem",
      "continueConversation": "Continuar conversa",
      "dismissError": "Dispensar erro",
      "removeAttachment": "Remover anexo",
      "tabToSelect": "Tab para selecionar"
    },
    "messageActions": {
      "characterMessage": "Mensagem do Personagem",
      "yourMessage": "Sua Mensagem",
      "whyCharacterResponded": "Por que este personagem respondeu",
      "total": "total",
      "edit": "Editar",
      "copy": "Copiar",
      "regenerateWithDifferent": "Regenerar com personagem diferente",
      "rewindToHere": "Retroceder até aqui",
      "unpinToDelete": "Desfixe para excluir",
      "delete": "Excluir",
      "editPlaceholder": "Edite sua mensagem...",
      "chooseCharacterTitle": "Escolher Personagem",
      "selectCharacterForRegeneration": "Selecione qual personagem deve responder em vez disso:"
    },
    "settings": {
      "appDefault": "Padrão do app",
      "selectPersona": "Selecionar Persona",
      "noPersonas": "Nenhuma persona disponível",
      "noPersonasDesc": "Crie uma persona nas configurações para personalizar suas conversas.",
      "searchPersonas": "Pesquisar personas...",
      "noPersona": "Sem Persona",
      "noPersonaDesc": "Continuar sem uma persona",
      "noPersonasFound": "Nenhuma persona encontrada",
      "noPersonasFoundDesc": "Tente um termo de pesquisa diferente"
    },
    "groupSettings": {
      "title": "Editar grupo",
      "subtitle": "Atualizar a configuração padrão para sessões futuras",
      "enterGroupName": "Nome do grupo",
      "participant": "participante",
      "participants": "participantes",
      "uploading": "Enviando...",
      "changeBackground": "Alterar fundo",
      "addBackgroundImage": "Adicionar imagem de fundo",
      "removeBackground": "Remover fundo",
      "persona": "Persona",
      "personaSubtitle": "Persona padrão para novas sessões",
      "personaLabel": "Persona",
      "speakerSelection": "Seleção de orador",
      "speakerSubtitle": "Método padrão para novas sessões",
      "llm": "LLM",
      "aiPicks": "IA escolhe",
      "heuristic": "Heurístico",
      "scoreBased": "Baseado em pontuação",
      "roundRobin": "Rodízio",
      "takeTurns": "Revezamento",
      "llmDesc": "Usa seu modelo padrão para escolher quem fala (custa tokens)",
      "heuristicDesc": "Usa equilíbrio de participação e pistas de contexto (gratuito)",
      "roundRobinDesc": "Personagens falam em ordem (gratuito)",
      "memoryMode": "Modo de memória",
      "memorySubtitle": "Modo de memória padrão para novas sessões",
      "manual": "Manual",
      "manualDesc": "Gerencie notas você mesmo",
      "dynamic": "Dinâmico",
      "dynamicDesc": "Recuperação automática",
      "memoryDynamicInfo": "A IA cria e recupera automaticamente memórias das conversas",
      "memoryManualInfo": "Você adiciona e gerencia as notas de memória",
      "characters": "Personagens",
      "participantsActive": "{{total}} participantes · {{active}} ativos",
      "add": "Adicionar",
      "muted": "(silenciado)",
      "mutedByDefault": "Silenciado por padrão",
      "activeByDefault": "Ativo por padrão",
      "unmuteCharacter": "Ativar áudio do personagem",
      "muteCharacter": "Silenciar personagem",
      "minTwoRequired": "Mínimo de 2 personagens necessários",
      "removeCharacter": "Remover personagem",
      "groupMinCharacters": "Um grupo requer pelo menos 2 personagens",
      "mutedCharactersNote": "Personagens silenciados são ignorados pela seleção automática de orador, mas ainda podem responder via `@menção` explícita.",
      "addCharacterTitle": "Adicionar personagem",
      "allCharactersInGroup": "Todos os personagens já estão neste grupo.",
      "removeCharacterTitle": "Remover personagem?",
      "removeCharacterConfirm": "Tem certeza de que deseja remover",
      "removeCharacterFrom": "das configurações padrão do grupo?",
      "removing": "Removendo...",
      "remove": "Remover"
    },
    "sessionSettings": {
      "subtitle": "Gerenciar preferências do chat em grupo",
      "enterGroupName": "Nome do grupo",
      "participant": "participante",
      "participants": "participantes",
      "message": "mensagem",
      "messages": "mensagens",
      "uploading": "Enviando...",
      "changeBackground": "Alterar fundo",
      "addBackgroundImage": "Adicionar imagem de fundo",
      "removeBackground": "Remover fundo",
      "persona": "Persona",
      "personaSubtitle": "Sua identidade nesta conversa",
      "personaLabel": "Persona",
      "speakerSelection": "Seleção de orador",
      "speakerSubtitle": "Como o próximo orador é escolhido",
      "llm": "LLM",
      "aiPicks": "IA escolhe",
      "heuristic": "Heurístico",
      "scoreBased": "Baseado em pontuação",
      "roundRobin": "Rodízio",
      "takeTurns": "Revezamento",
      "llmDesc": "Usa seu modelo padrão para escolher quem fala (custa tokens)",
      "heuristicDesc": "Usa equilíbrio de participação e pistas de contexto (gratuito)",
      "roundRobinDesc": "Personagens falam em ordem (gratuito)",
      "characters": "Personagens",
      "participantsActive": "{{total}} participantes · {{active}} ativos",
      "add": "Adicionar",
      "muted": "(silenciado)",
      "unmuteCharacter": "Ativar áudio do personagem",
      "muteCharacter": "Silenciar personagem",
      "minTwoRequired": "Mínimo de 2 personagens necessários",
      "removeCharacter": "Remover personagem",
      "groupMinCharacters": "Um chat em grupo requer pelo menos 2 personagens",
      "mutedCharactersNote": "Personagens silenciados são ignorados pela seleção automática de orador, mas ainda podem responder via `@menção` explícita.",
      "data": "Dados",
      "dataSubtitle": "Exportar ou importar conversas",
      "export": "Exportar",
      "exportDesc": "Salvar como arquivo compartilhável",
      "import": "Importar",
      "importDesc": "Carregar uma conversa de um arquivo",
      "conversation": "Conversa",
      "conversationSubtitle": "Duplicar ou continuar em um novo chat",
      "duplicate": "Duplicar",
      "duplicateDesc": "Copiar este chat com ou sem mensagens",
      "branchTo1on1": "Ramificar para 1 a 1",
      "branchTo1on1Desc": "Continuar privativamente com um personagem",
      "participation": "Participação",
      "participationSubtitle": "Distribuição de fala entre personagens",
      "addCharacterTitle": "Adicionar personagem",
      "allCharactersInGroup": "Todos os personagens já estão neste grupo.",
      "removeCharacterTitle": "Remover personagem?",
      "removeCharacterConfirm": "Tem certeza de que deseja remover",
      "removeCharacterFrom": "deste chat em grupo?",
      "removing": "Removendo...",
      "remove": "Remover",
      "cloneGroupTitle": "Clonar grupo",
      "withMessages": "Com mensagens",
      "withMessagesDesc": "Clonar tudo incluindo histórico de chat",
      "withoutMessages": "Sem mensagens",
      "withoutMessagesDesc": "Clonar apenas a configuração (personagens, cena inicial)",
      "branchWithCharacterTitle": "Ramificar com personagem",
      "branchWithCharacterDesc": "Selecione um personagem para continuar como conversa 1 a 1. Todas as mensagens deste grupo serão convertidas.",
      "continueWith": "Continuar conversa com {{name}}",
      "exportChatPackageTitle": "Exportar pacote de chat",
      "includeCharacterSnapshots": "Incluir snapshots de personagens",
      "includeCharacterSnapshotsDesc": "Manter dados de personagens dentro do pacote",
      "sessionOnly": "Apenas sessão",
      "sessionOnlyDesc": "Exportar apenas mensagens e metadados",
      "mapParticipantsTitle": "Mapear participantes",
      "selectLocalCharacter": "Selecione o personagem local para este participante.",
      "selectCharacterPlaceholder": "Selecionar personagem...",
      "continue": "Continuar",
      "importChatPackageTitle": "Importar pacote de chat",
      "importChatPackageDesc": "Isso importará o `.chatpkg` selecionado como uma nova sessão em grupo.",
      "importing": "Importando..."
    },
    "page": {
      "selectingCharacter": "Selecting character...",
      "sessionNotFound": "Group session not found",
      "backToGroupChats": "Back to Group Chats",
      "startConversation": "Start a conversation with {{names}}",
      "scrollToBottom": "Scroll to bottom",
      "addContent": "Add Content",
      "uploadImage": "Upload Image",
      "helpMeReply": "Help Me Reply",
      "helpMeReplyDesc": "Let AI suggest what to say",
      "haveDraftPrompt": "You have a draft message. How would you like to proceed?",
      "useMyTextAsBase": "Use my text as base",
      "useMyTextAsBaseDesc": "Expand and improve your draft",
      "writeSomethingNew": "Write something new",
      "writeSomethingNewDesc": "Generate a fresh reply",
      "suggestedReply": "Suggested Reply",
      "reasoningBeforeWriting": "Reasoning before writing your reply...",
      "writingYourReply": "Writing your reply...",
      "regenerate": "Regenerate",
      "useReply": "Use Reply",
      "helpMeReplyFailedGeneric": "Help Me Reply failed.",
      "helpMeReplyFailedGenerate": "Help Me Reply failed to generate a reply.",
      "noAudioCaptured": "No audio captured.",
      "noWhisperModel": "No installed Whisper model found. Install one in Speech Recognition settings.",
      "cancelRecording": "Cancelar gravação",
      "transcribing": "Transcrevendo",
      "stopAndTranscribe": "Stop and transcribe",
      "recordVoice": "Record voice",
      "learnCorrection": "Learn correction:",
      "learning": "Learning...",
      "learn": "Learn",
      "ignore": "Ignore",
      "groupChatFailed": "Group chat failed.",
      "failedToCopy": "Failed to copy",
      "copied": "Copied!",
      "cannotDeletePinned": "Cannot delete pinned message. Unpin it first.",
      "failedToDelete": "Failed to delete",
      "messageNotFound": "Message not found",
      "cannotRewindPinned": "Cannot rewind: there are pinned messages after this point. Unpin them first.",
      "failedToRewind": "Failed to rewind",
      "failedToTogglePin": "Failed to toggle pin",
      "messagePinned": "Message pinned",
      "messageUnpinned": "Message unpinned",
      "failedToSave": "Failed to save"
    },
    "memoriesPage": {
      "summarizingConversation": "Summarizing conversation",
      "analyzingMemories": "Analyzing memories",
      "applyingChanges": "Applying changes",
      "organizingMemories": "Organizing memories",
      "retryingMemoryCycle": "Retrying Memory Cycle...",
      "processingMemories": "Processing memories...",
      "memorySystemError": "Memory System Error",
      "contextSummary": "Context Summary",
      "contextSummaryTitle": "Context Summary",
      "savedMemories": "Saved Memories",
      "resultsCount": "Results ({{count}})",
      "searchMemoriesPlaceholder": "Search memories...",
      "addMemory": "Add memory",
      "memoryActions": "Memory actions",
      "clearSearch": "Clear search",
      "noMatchingMemories": "No matching memories",
      "noMemoriesYet": "No memories yet",
      "tryDifferentSearch": "Try a different search term",
      "tapAddToCreate": "Tap the Add button above to create one",
      "pinnedMessages": "Pinned Messages",
      "refresh": "Refresh",
      "noPinnedMessages": "No pinned messages",
      "pinImportantDesc": "Pin important group chat messages to always keep them in context.",
      "assistant": "Assistant",
      "user": "User",
      "unpin": "Unpin",
      "failedToUnpinMessage": "Failed to unpin message",
      "activityLog": "Activity Log",
      "run": "Run",
      "addMemoryTitle": "Add Memory",
      "editMemoryTitle": "Edit Memory",
      "memoryTitle": "Memory",
      "memoryPlaceholder": "What should be remembered?",
      "saveMemory": "Save Memory",
      "editMemoryPlaceholder": "Enter memory content...",
      "saving": "Saving...",
      "save": "Salvar",
      "cancel": "Cancel",
      "contextSummaryPlaceholder": "Short recap used to keep context consistent across messages...",
      "contextSummaryPrompt": "Tap to add a context summary...",
      "cycle": "Cycle",
      "accessed": "Accessed",
      "cold": "Cold",
      "hot": "Hot",
      "tokens": "tokens",
      "pin": "Pin",
      "setHot": "Set Hot",
      "setCold": "Set Cold",
      "edit": "Edit",
      "delete": "Delete",
      "failedToToggleMemPin": "Failed to toggle pin",
      "failedToRemoveMemory": "Failed to remove memory",
      "toolEventsCountAria": "events",
      "activityEmptyDesc": "Tool calls appear when AI manages memories in dynamic mode",
      "memoryCycleSuccess": "Memory cycle processed successfully!"
    },
    "messageActionsExtra": {
      "characterMessage": "Character Message",
      "yourMessage": "Your Message",
      "whyCharacterResponded": "Why this character responded",
      "promptTokensTitle": "Prompt Tokens",
      "completionTokensTitle": "Completion Tokens",
      "total": "total",
      "regenerateWithDifferent": "Regenerate with different character",
      "unpin": "Unpin",
      "pin": "Pin",
      "rewindToHere": "Rewind to here",
      "unpinToDelete": "Unpin to delete",
      "editPlaceholder": "Edit your message...",
      "chooseCharacter": "Choose Character",
      "selectCharacterForRegeneration": "Select which character should respond instead:"
    },
    "timeAgo": {
      "justNow": "Just now",
      "minutesAgo": "{{count}}m ago",
      "hoursAgo": "{{count}}h ago",
      "daysAgo": "{{count}}d ago"
    },
    "memoriesController": {
      "missingSessionId": "Missing sessionId",
      "sessionNotFound": "Session not found",
      "failedToLoadSession": "Failed to load session",
      "failedToUpdateTemperature": "Failed to update memory temperature",
      "failedToSaveSummary": "Failed to save summary",
      "cycleFailed": "Group memory cycle failed",
      "failedToAddMemory": "Failed to add memory",
      "failedToUpdateMemory": "Failed to update memory",
      "failedToRunCycle": "Failed to run memory cycle",
      "failedToRevertCycle": "Failed to revert cycle",
      "failedToRefresh": "Failed to refresh",
      "failedToDismissError": "Failed to dismiss error",
      "failedToTogglePinned": "Failed to toggle pinned message",
      "sessionNotLoaded": "Session not loaded",
      "revertCycleTitle": "Revert this cycle?",
      "revertConfirm": "Revert"
    },
    "chatController": {
      "sessionNotFound": "Group session not found",
      "failedToLoadGroupChat": "Failed to load group chat",
      "requestFailed": "Group chat request failed",
      "failedToSendMessage": "Failed to send message",
      "failedToRegenerate": "Failed to regenerate",
      "failedToContinue": "Failed to continue",
      "failedToCopy": "Failed to copy",
      "cannotDeletePinned": "Cannot delete pinned message. Unpin it first.",
      "failedToDelete": "Failed to delete",
      "messageNotFound": "Message not found",
      "cannotRewindPinned": "Cannot rewind: there are pinned messages after this point. Unpin them first.",
      "failedToRewind": "Failed to rewind",
      "failedToTogglePin": "Failed to toggle pin",
      "messagePinned": "Message pinned",
      "messageUnpinned": "Message unpinned",
      "failedToSave": "Failed to save",
      "copied": "Copied!"
    },
    "historyController": {
      "failedToLoadData": "Failed to load data",
      "failedToDelete": "Falha ao excluir: {{error}}",
      "failedToRename": "Falha ao renomear: {{error}}",
      "failedToArchive": "Falha ao arquivar: {{error}}",
      "failedToUnarchive": "Falha ao desarquivar: {{error}}",
      "failedToDuplicate": "Falha ao duplicar"
    },
    "sessionSettingsController": {
      "failedToLoad": "Falha ao carregar as configurações do chat em grupo",
      "noPersona": "Sem persona",
      "customPersona": "Persona personalizada",
      "minOneActive": "Pelo menos um participante precisa permanecer ativo"
    },
    "groupSettingsController": {
      "notFound": "Grupo não encontrado",
      "failedToLoad": "Falha ao carregar as configurações do grupo"
    },
    "createForm": {
      "failedToLoadCharacters": "Falha ao carregar personagens",
      "selectAtLeastTwo": "Selecione pelo menos 2 personagens para um chat em grupo",
      "failedToCreate": "Falha ao criar o chat em grupo"
    },
    "groupSetupExtra": {
      "memoryMode": "Modo de memória",
      "manual": "Manual",
      "manualDesc": "Gerencie as notas você mesmo",
      "dynamic": "Dinâmico",
      "dynamicDesc": "Resumos e recuperação automáticos",
      "memoryManualInfo": "Você adiciona e gerencia as notas de memória manualmente",
      "memoryDynamicInfo": "A IA cria e recupera memórias automaticamente das conversas",
      "backgroundPreviewAlt": "Pré-visualização do plano de fundo"
    },
    "createExtra": {
      "enterGroupNamePlaceholder": "Digite o nome do grupo...",
      "unknown": "Desconhecido"
    },
    "startingSceneExtra": {
      "insertAs": "Inserir como {{snippet}}"
    },
    "sessionCardExtra": {
      "unknown": "Desconhecido",
      "untitledChat": "Chat sem título"
    },
    "sessionListExtra": {
      "unknown": "Desconhecido"
    },
    "chatHeaderExtra": {
      "unknownError": "Erro desconhecido"
    },
    "chatSettingsExtra": {
      "invalidPackage": "This package is not a group chat package.",
      "failedExport": "Falha ao exportar o pacote do chat em grupo",
      "failedInspect": "Falha ao inspecionar o pacote do chat em grupo",
      "failedImport": "Falha ao importar o pacote do chat em grupo",
      "exportSuccess": "Pacote do chat em grupo exportado para:\n{{path}}",
      "backgroundAlt": "Plano de fundo",
      "removeBackgroundAria": "Remover plano de fundo",
      "backAria": "Voltar",
      "backToGroupChats": "Voltar para chats em grupo"
    },
    "groupSettingsExtra": {
      "backToGroups": "Voltar para grupos",
      "backAria": "Voltar",
      "removeBackgroundAria": "Remover plano de fundo"
    },
    "historyPage": {
      "backAria": "Voltar para chats em grupo",
      "clearSearchAria": "Limpar busca",
      "resultsLabel": "{{count}} resultado",
      "resultsLabelPlural": "{{count}} resultados",
      "untitledChat": "Chat sem título",
      "noPinnedMessages": "Nenhuma mensagem fixada"
    },
    "personaSelectorExtra": {
      "insertAs": "Inserir como",
      "appDefault": "Padrão do app",
      "defaultBadge": "Padrão",
      "selectPersonaTitle": "Selecionar persona",
      "noPersonaTitle": "Sem persona",
      "noPersonaDesc": "Continuar sem uma persona",
      "noPersonasAvailable": "Nenhuma persona disponível",
      "noPersonasDesc": "Crie uma persona nas configurações para personalizar suas conversas.",
      "searchPersonas": "Pesquisar personas...",
      "noPersonasFound": "Nenhuma persona encontrada",
      "tryDifferentSearch": "Tente um termo de busca diferente"
    },
    "footerExtra": {
      "cancelRecording": "Cancelar gravação",
      "transcribing": "Transcrevendo",
      "stopAndTranscribe": "Stop and transcribe",
      "recordVoice": "Record voice",
      "attachmentAltDefault": "Attachment"
    },
    "pageExtra": {
      "noAudioCaptured": "No audio captured.",
      "noWhisperModelInstalled": "No installed Whisper model found. Install one in Speech Recognition settings.",
      "scrollToBottomAria": "Scroll to bottom"
    },
    "addContentMenu": {
      "title": "Add Content",
      "uploadImage": "Upload Image"
    },
    "helpMeReplyMenu": {
      "title": "Help Me Reply",
      "helpMeReplyDesc": "Let AI suggest what to say",
      "draftPrompt": "You have a draft message. How would you like to proceed?",
      "useTextAsBase": "Use my text as base",
      "useTextAsBaseDesc": "Expand and improve your draft",
      "writeSomethingNew": "Write something new",
      "writeSomethingNewDesc": "Generate a fresh reply"
    },
    "suggestedReplyMenu": {
      "title": "Suggested Reply",
      "reasoningBeforeWriting": "Reasoning before writing your reply...",
      "writingYourReply": "Writing your reply...",
      "regenerate": "Regenerate",
      "useReply": "Use Reply"
    },
    "memoriesPageExtra": {
      "sessionNotFound": "Session not found",
      "memorySystemError": "Memory System Error",
      "retryingMemoryCycle": "Retrying Memory Cycle...",
      "processingMemories": "Processing memories...",
      "memoryCycleSuccess": "Memory cycle processed successfully!",
      "contextSummaryTitle": "Context Summary",
      "activityTabLabel": "Activity",
      "noMatchingMemoriesDesc": "Try a different search term",
      "chatHistoryTitle": "Chat History",
      "chatHistoryDesc": "View and manage conversations"
    },
    "settingsPageExtra": {
      "quickActionsSection": "Quick Actions",
      "chatHistoryTitle": "Chat History",
      "chatHistoryDesc": "View and manage conversations",
      "lorebrooksTitle": "Lorebooks",
      "lorebrooksDesc": "Attach session lorebooks and optionally ignore each character's own lorebooks.",
      "manageLorebooks": "Manage lorebooks"
    },
    "groupSettingsPageExtra": {
      "lorebrooksTitle": "Lorebooks",
      "lorebrooksDesc": "Attach shared lorebooks and control whether character lorebooks apply by default.",
      "manageLorebooks": "Manage lorebooks"
    }
  },
  "characters": {
    "empty": {
      "title": "Nenhum personagem ainda",
      "description": "Crie personagens de IA personalizados com personalidades únicas",
      "createButton": "Criar Personagem"
    },
    "progress": {
      "identity": "Identidade",
      "scenes": "Cenas",
      "details": "Detalhes"
    },
    "identity": {
      "title": "Criar Personagem",
      "subtitle": "Dê uma identidade ao seu personagem de IA",
      "tapCameraToAdd": "Toque na câmera para adicionar ou gerar avatar",
      "importingAvatar": "Importando avatar...",
      "characterName": "Nome do Personagem *",
      "characterNamePlaceholder": "Digite o nome do personagem...",
      "characterNameDesc": "Este nome aparecerá nas conversas",
      "avatarGradient": "Gradiente do Avatar",
      "avatarGradientDesc": "Gerar gradientes dinâmicos a partir das cores do avatar",
      "chatBackgroundLabel": "Chat Background",
      "optionalSuffix": "(Optional)",
      "backgroundPreviewAlt": "Background preview",
      "backgroundPreviewBadge": "Background Preview",
      "addBackground": "Add Background",
      "addBackgroundHint": "Upload one or pick from library",
      "uploadImage": "Upload image",
      "chooseFromLibrary": "Choose from library",
      "backgroundDesc": "Optional background image for chat conversations",
      "continueToDescription": "Continue to Description",
      "importCharacterFromFile": "Import Character from File",
      "importCharacterDesc": "Load a character from a PNG card, .uec, or .json export file"
    },
    "details": {
      "title": "Detalhes do Personagem",
      "subtitle": "Definir personalidade e comportamento",
      "definition": "Definição *",
      "wordCount": "{{count}} palavra(s)",
      "definitionPlaceholder": "Descreva personalidade, estilo de fala, história, áreas de conhecimento...",
      "definitionDesc": "Seja específico sobre tom, traços e estilo de conversa",
      "availablePlaceholders": "Marcadores Disponíveis:"
    },
    "scenes": {
      "title": "Cenas Iniciais",
      "subtitle": "Crie cenários de abertura para suas conversas",
      "default": "Padrão",
      "hasSceneDirection": "Tem direção de cena",
      "continueToScenes": "Continuar para Cenas Iniciais"
    },
    "extras": {
      "title": "Detalhes Extras",
      "subtitle": "Todos os campos são opcionais",
      "nickname": "Apelido",
      "nicknamePlaceholder": "Como o usuário deve chamar este personagem?",
      "nicknameDesc": "Nome alternativo usado nas conversas",
      "creator": "Criador",
      "creatorPlaceholder": "Nome do criador...",
      "tags": "Tags",
      "tagsPlaceholder": "fantasia, sci-fi, romance...",
      "tagsDesc": "Lista separada por vírgulas para filtragem e organização",
      "creatorNotes": "Notas do Criador",
      "creatorNotesPlaceholder": "Dicas de uso, contexto de lore ou instruções para outros usuários...",
      "multilingualNotes": "Multilingual Notes",
      "multilingualNotesHint": "JSON object with language codes as keys",
      "creatingCharacter": "Creating Character..."
    },
    "preview": {
      "unnamedCharacter": "Personagem sem nome",
      "sceneCount": "{{count}} cena(s)",
      "customPrompt": "Prompt personalizado",
      "description": "Descrição",
      "startingScene": "Cena Inicial",
      "gradientEnabled": "Gradiente ativado",
      "customModel": "Modelo personalizado",
      "avatarAlt": "Avatar do personagem",
      "characterFallback": "Character"
    },
    "personaPreview": {
      "unnamedPersona": "Persona sem nome",
      "noDescription": "Sem descrição ainda",
      "default": "Padrão",
      "description": "Descrição"
    },
    "lorebookPreview": {
      "untitledLorebook": "Livro de Lore sem título",
      "entryCount": "{{count}} entrada(s)",
      "entries": "Entradas",
      "noEntriesYet": "Nenhuma entrada ainda",
      "untitledEntry": "Entrada sem título",
      "noContentYet": "Sem conteúdo ainda",
      "alwaysActive": "Sempre ativo",
      "moreEntries": "+{{count}} more entries",
      "moreEntry": "+{{count}} more entry"
    },
    "creationHelper": {
      "moreOptions": "Mais opções",
      "describePlaceholder": "Descreva seu personagem...",
      "stopGeneration": "Parar geração",
      "sendMessage": "Enviar mensagem",
      "addToMessage": "Adicionar à Mensagem",
      "uploadImageDesc": "Adicionar um avatar ou imagem de referência",
      "referenceCharacterDesc": "Usar um personagem existente como inspiração",
      "referencePersonaDesc": "Usar sua persona como contexto",
      "retry": "Retry",
      "attachmentAlt": "Attachment",
      "removeAttachment": "Remove attachment",
      "removeReference": "Remove reference",
      "uploadImageTitle": "Upload Image",
      "referenceCharacterTitle": "Reference Character",
      "referencePersonaTitle": "Reference Persona"
    },
    "lorebook": {
      "keywords": "PALAVRAS-CHAVE",
      "caseSensitive": "Sensível a maiúsculas",
      "typeKeyword": "Digite uma palavra-chave...",
      "addButton": "Adicionar",
      "untitledEntry": "Entrada sem título",
      "alwaysActive": "Sempre ativo",
      "noKeywords": "Sem palavras-chave",
      "dragToReorder": "Arraste para reordenar",
      "disabled": "Desativado",
      "noLorebooksYet": "Nenhum livro de lore ainda",
      "createLorebookDesc": "Crie um livro de lore para adicionar lore do mundo para este personagem",
      "createLorebook": "Criar Livro de Lore",
      "searchPlaceholder": "Pesquisar livros de lore...",
      "noMatchingLorebooks": "Nenhum livro de lore correspondente encontrado",
      "activeLorebooks": "Livros de Lore Ativos",
      "enabledForCharacter": "ativado para este personagem",
      "enabledForGroup": "ativado para este grupo",
      "enabledForSession": "ativado para esta sessão",
      "tapToViewEntries": "Toque para ver entradas",
      "newLorebookTitle": "Novo Livro de Lore",
      "nameLabel": "NOME",
      "enterNamePlaceholder": "Digite o nome do livro de lore...",
      "lorebookExplanation": "Livros de lore contêm entradas que são injetadas nos prompts quando palavras-chave correspondem.",
      "viewEntries": "Ver Entradas",
      "editEntriesDesc": "Editar entradas do livro de lore",
      "disableForCharacter": "Desativar para Personagem",
      "enableForCharacter": "Ativar para Personagem",
      "disableForGroup": "Desativar para Grupo",
      "enableForGroup": "Ativar para Grupo",
      "disableForSession": "Desativar para Sessão",
      "enableForSession": "Ativar para Sessão",
      "removeFromActive": "Remover dos livros de lore ativos deste personagem",
      "addToActive": "Adicionar aos livros de lore ativos deste personagem",
      "removeFromActiveGroup": "Remover dos livros de lore ativos deste grupo",
      "addToActiveGroup": "Adicionar aos livros de lore ativos deste grupo",
      "removeFromActiveSession": "Remover dos livros de lore ativos desta sessão",
      "addToActiveSession": "Adicionar aos livros de lore ativos desta sessão",
      "deleteConfirmTitle": "Excluir livro de lore?",
      "deleteConfirmMessage": "Excluir este livro de lore? Todas as entradas serão perdidas.",
      "deleteLorebook": "Excluir Livro de Lore",
      "noEntriesYet": "Nenhuma entrada ainda",
      "addEntriesToInject": "Adicione entradas para injetar lore nas suas conversas",
      "createEntry": "Criar Entrada",
      "searchEntries": "Pesquisar entradas...",
      "noMatchingEntries": "Nenhuma entrada correspondente encontrada",
      "entryDefaultName": "Entrada",
      "editEntry": "Editar Entrada",
      "editEntryDesc": "Modificar título, palavras-chave e conteúdo",
      "disableEntry": "Desativar Entrada",
      "enableEntry": "Ativar Entrada",
      "entryDisabledDesc": "A entrada não será injetada nos prompts",
      "entryEnabledDesc": "A entrada será injetada quando palavras-chave corresponderem",
      "deleteEntry": "Excluir Entrada",
      "titleLabel": "TÍTULO",
      "titlePlaceholder": "Nomeie esta entrada...",
      "enabled": "Ativado",
      "includeInPrompts": "Incluir nos prompts",
      "alwaysOn": "Sempre Ativo",
      "noKeywordsNeeded": "Sem necessidade de palavras-chave",
      "contentLabel": "CONTEÚDO",
      "contentPlaceholder": "Escreva o contexto de lore aqui...",
      "saveEntry": "Salvar Entrada",
      "noCharacterId": "Nenhum ID de personagem fornecido",
      "sectionActive": "Active",
      "sectionAvailable": "Available",
      "entryCount": "{{count}} entries",
      "keywordDetectionMode": "KEYWORD DETECTION",
      "keywordDetectionRecentWindow": "Recent 10 messages",
      "keywordDetectionRecentWindowDesc": "Matches against the recent 10-message conversation window.",
      "keywordDetectionLatestUser": "Latest user message only",
      "keywordDetectionLatestUserDesc": "Matches only against the most recent user-sent message.",
      "preview": {
        "title": "Trigger Preview",
        "openButton": "Preview",
        "missingContext": "No lorebook selected.",
        "noEntries": "Add entries to this lorebook to see what would trigger.",
        "modeRecentShort": "Recent {{count}}",
        "modeLatestUserShort": "Latest user",
        "inWindow": "{{count}} in window",
        "tabSession": "Session",
        "tabCompose": "Compose",
        "activeStat": "{{active}} / {{total}} active",
        "tokensStat": "{{count}} tokens",
        "sessionPickerLabel": "Sessions",
        "sessionMeta": "{{count}} msgs",
        "noSessions": "No chat sessions yet.",
        "noSessionsHint": "Pick a session",
        "noMessages": "This session has no messages yet.",
        "scanHeaderRecent": "Scanning {{shown}} of the last {{depth}} messages",
        "scanHeaderLatest": "Scanning the latest user message",
        "matchCount": "{{hits}} hit · {{entries}} entries",
        "emptyMessage": "(empty)",
        "roleUser": "User",
        "roleAssistant": "Assistant",
        "roleScene": "Scene",
        "roleSystem": "System",
        "composeHeader": "Scratch pad",
        "composeMatches": "{{count}} matches",
        "activeLabel": "{{count}} active",
        "composePlaceholder": "Type or paste text to test keyword matching...\n\ne.g.\nThe library was quiet, just the hum of the old heaters.\nShe asked if I'd read the book she lent me last week.",
        "sectionActive": "Active · {{count}}",
        "sectionInactive": "Inactive · {{count}}",
        "statusMatched": "Matched",
        "statusAlways": "Always",
        "statusDisabled": "Off",
        "statusNoKeywords": "No keys",
        "statusNotMatched": "No match",
        "tokensShort": "{{count}}t",
        "injectionTitle": "Final injection",
        "injectionEmpty": "No entries are active — nothing would be injected.",
        "copy": "Copy",
        "copyFailed": "Failed to copy to clipboard.",
        "saveFailed": "Failed to save entry.",
        "deleteFailed": "Failed to delete entry.",
        "deleteConfirmTitle": "Are you sure?",
        "deleteConfirmMessage": "Delete \"{{title}}\"? This can't be undone.",
        "contextMenuTitle": "{{count}} entries use this"
      }
    },
    "templates": {
      "characterNotFound": "Personagem não encontrado",
      "templateCount": "{{count}} modelo(s)",
      "newTemplate": "Novo Modelo",
      "noTemplatesYet": "Nenhum modelo ainda",
      "explanation": "Modelos de conversa permitem iniciar conversas com mensagens pré-escritas tanto suas quanto de {{name}}.",
      "createTemplate": "Criar Modelo",
      "messageCount": "{{count}} mensagem(ns)",
      "deleteTemplate": "Excluir modelo",
      "contextMenuFallbackTitle": "Template",
      "importedToast": {
        "title": "Imported",
        "message": "Added \"{{name}}\"."
      },
      "importFailed": "Import failed",
      "exportFailed": "Export failed"
    },
    "templateEditor": {
      "noScene": "Sem cena",
      "untitled": "Sem título",
      "dragMessage": "Arrastar mensagem",
      "editMessage": "Editar mensagem",
      "deleteMessage": "Excluir mensagem",
      "writeMessagePlaceholder": "Escreva o conteúdo da mensagem...",
      "characterNotFound": "Personagem não encontrado",
      "scene": "Cena",
      "noMessagesYet": "Nenhuma mensagem ainda",
      "addMessagesDesc": "Adicione mensagens para construir um início de conversa com {{name}}.",
      "addMessage": "Adicionar Mensagem",
      "name": "Nome",
      "nameExample": "ex. Saudação Casual",
      "startingScene": "Cena Inicial",
      "systemPrompt": "Prompt de Sistema",
      "characterDefault": "Padrão do personagem",
      "nextMessageAs": "Próxima mensagem como",
      "messages": "Mensagens",
      "roles": "Funções",
      "hoverTip": "Passe o mouse nas mensagens para arrastar, editar ou excluir.",
      "footerTip": "Use a barra inferior para adicionar novas mensagens à conversa.",
      "templateNamePlaceholder": "Nome do modelo...",
      "selectScene": "Selecionar Cena",
      "startWithoutScene": "Começar sem mensagem de cena",
      "selectSystemPrompt": "Selecionar Prompt de Sistema",
      "useCharacterSystemPrompt": "Usar prompt de sistema do personagem"
    },
    "referenceSelector": {
      "selectCharacter": "Selecionar Personagem",
      "selectPersona": "Selecionar Persona",
      "searchPlaceholder": "Pesquisar {{type}}s...",
      "loading": "Loading...",
      "noMatch": "No {{type}}s match your search",
      "noAvailable": "No {{type}}s available"
    },
    "voiceLoading": {
      "failed": "Failed to load voices"
    },
    "activeLorebooks": {
      "sectionTitle": "Active Lorebooks",
      "selectedSummary": "{{count}} active",
      "untitledLorebook": "Untitled lorebook",
      "usingNone": "Using no character lorebooks",
      "loading": "Loading lorebooks...",
      "loadFailed": "Failed to load lorebooks",
      "inheritHint": "Sessions inherit these unless the session overrides them.",
      "clear": "Clear",
      "chooseHint": "Choose the lorebooks this character should activate by default. Existing sessions can still override this list.",
      "emptyState": "No lorebooks yet. Create lorebooks from the lorebook manager first."
    },
    "description": {
      "descriptionLabel": "Description",
      "descriptionPlaceholder": "Short summary shown on cards and lists...",
      "descriptionHint": "Optional short description for the UI; the full definition is used in prompts.",
      "companionPromptLabel": "Companion Prompt (Optional)",
      "systemPromptLabel": "System Prompt (Optional)",
      "loadingTemplates": "Loading templates...",
      "useAppCompanionDefault": "Use app companion default",
      "useAppDefault": "Use app default",
      "companionPromptHint": "Stored separately as the companion prompt. The normal roleplay system prompt is not changed.",
      "systemPromptHint": "Choose a custom system prompt or use the default.",
      "groupChatConvLabel": "Group Chat Prompt (Conversation)",
      "groupChatConvHint": "Override this character's conversation prompt in group chats",
      "groupChatRpLabel": "Group Chat Prompt (Roleplay)",
      "groupChatRpHint": "Override this character's roleplay prompt in group chats",
      "voiceLabel": "Voice (Optional)",
      "loadingVoices": "Loading voices...",
      "customVoiceFallback": "Custom Voice",
      "providerVoiceFallback": "Provider Voice",
      "selectedVoiceFallback": "Selected Voice",
      "noVoiceAssigned": "No voice assigned",
      "addVoicesHint": "Add voices in Settings → Voices",
      "voiceAssignHint": "Assign a voice for future text-to-speech playback",
      "autoplayLabel": "Autoplay voice",
      "autoplayOn": "Play this character's replies automatically",
      "autoplayOff": "Select a voice first",
      "aiModelLabel": "AI Model *",
      "loadingModels": "Loading models...",
      "selectedModelFallback": "Selected Model",
      "selectModelPlaceholder": "Select a model",
      "noModelsConfigured": "No models configured",
      "noModelsHint": "Add a provider in settings first to continue",
      "aiModelHint": "This model will power the character's responses",
      "fallbackModelLabel": "Modelo de fallback (opcional)",
      "selectedFallbackFallback": "Modelo de fallback selecionado",
      "fallbackOff": "Desativado (sem fallback)",
      "fallbackHint": "Tenta novamente com este modelo somente se o modelo principal falhar",
      "memoryModeLabel": "Memory Mode",
      "enableInSettingsHint": "Enable in Settings to switch",
      "memoryManual": "Manual",
      "memoryManualDescDesktop": "Add and manage memory notes yourself.",
      "memoryManualDescMobile": "Current system: add and manage memory notes yourself.",
      "memoryDynamic": "Dynamic",
      "memoryDynamicDescDesktop": "Automatic summaries and context updates.",
      "memoryDynamicDescMobile": "Automatic summaries and context updates for this character.",
      "memoryHint": "Dynamic memory requires it to be enabled in Advanced settings. Otherwise, manual memory is used.",
      "selectModelTitle": "Select Model",
      "selectFallbackModelTitle": "Selecionar modelo de fallback",
      "searchModelsPlaceholder": "Search models...",
      "selectVoiceTitle": "Select Voice",
      "searchVoicesPlaceholder": "Search voices...",
      "myVoices": "My Voices",
      "providerVoicesLabel": "{{provider}} Voices",
      "providerFallback": "Provedor"
    },
    "interactionMode": {
      "sectionLabel": "Interaction Mode",
      "sectionHint": "Choose whether this character behaves like an RP character or a persistent companion.",
      "activeBadge": "Active",
      "roleplayTitle": "Roleplay",
      "roleplaySubtitle": "Scene-driven chats, narrative framing, and starting scenarios.",
      "companionTitle": "Companion",
      "companionSubtitle": "Relationship-driven chats with emotional state and companion memory."
    },
    "startingScene": {
      "openingContextTitle": "Opening Context",
      "openingContextSubtitle": "Optional first-chat context for this companion. Companion sessions can start without a scene.",
      "sceneDirectionLabel": "Scene Direction",
      "setAsDefault": "Set as Default",
      "noOpeningContext": "No opening context yet",
      "noScenesYet": "No scenes yet",
      "skipForCompanion": "You can skip this for companion mode.",
      "createFirstScene": "Create your first scene to get started",
      "openingPlaceholder": "Optional opening context, like where the companion is or what they were doing before the first message...",
      "scenePlaceholder": "Create a starting scene or scenario for roleplay (e.g., 'You find yourself in a mystical forest at twilight...')",
      "addDirection": "+ Add Direction",
      "directionAdded": "Direction added",
      "wordsCount": "{{count}} words",
      "placeholderHelp": "Use {{charTag}} for the character and {{userTag}} (alias {{personaTag}}) for the persona.",
      "sceneBackgroundLabel": "Scene Background",
      "optionalLabel": "Optional",
      "sceneBgOverrideHint": "Overrides the character background for chats using this scene.",
      "sceneBgUsedHint": "Used as the chat background for this scene unless the session overrides it.",
      "cancel": "Cancel",
      "directionPlaceholderNew": "e.g., 'The hostage will be rescued' or 'Maintain tense atmosphere'",
      "directionPlaceholderEdit": "e.g., 'The hostage will be rescued' or 'Build tension gradually'",
      "directionAiHint": "Hidden guidance for the AI on how this scene should unfold",
      "addScene": "Add Scene",
      "multipleScenesHint": "Create multiple starting scenarios. One will be selected when starting a new chat.",
      "companionContextHint": "Opening context is optional for companions; long-term continuity comes from companion memory.",
      "skipContext": "Skip Context",
      "editSceneTitle": "Edit Scene",
      "sceneContentPlaceholder": "Enter scene content...",
      "addLabel": "+ Add",
      "save": "Salvar",
      "library": "Library",
      "upload": "Upload",
      "sceneBackgroundAlt": "Scene background",
      "removeBackground": "Remove background"
    },
    "companionSoul": {
      "title": "Companion Soul",
      "subtitle": "Shape who they are underneath. Generation uses the opening context you set in the previous step.",
      "retry": "Retry",
      "back": "Voltar",
      "continue": "Continue",
      "addNameFirst": "Add a name first.",
      "addDefinitionFirst": "Add a definition first."
    },
    "soulEditor": {
      "generateTitle": "Generate from character",
      "generateUsingModel": "Uses {{model}}. You'll review and edit before applying.",
      "generateDefaultDesc": "Drafts a soul from the character's name, definition, and scenes.",
      "directionLabel": "Direction",
      "directionOptional": "Optional steering for the LLM",
      "directionPlaceholder": "e.g. \"Lean tsundere - guarded outside, soft once trusted. Less anxious, more pride.\"",
      "directionEditTooltip": "Optional direction for generation",
      "generating": "Generating",
      "generate": "Generate",
      "presetLabel": "Personality preset",
      "presetMatches": "Matches: {{label}}",
      "presetHint": "Sets baseline affect, regulation, and relationship sliders. Text fields are preserved.",
      "identityLabel": "Identity",
      "hideExamples": "Hide examples",
      "showExamples": "Show examples",
      "insertExample": "Insert example",
      "exampleEg": "e.g., {{example}}",
      "fineTuneLabel": "Fine-tune feelings",
      "baselineAffect": "Baseline Affect",
      "baselineAffectInfo": "How they feel by default; the emotional waterline before anything happens.",
      "regulationStyle": "Regulation Style",
      "regulationStyleInfo": "How they handle and express what they feel; venting vs. burying.",
      "relationshipDefaults": "Relationship Defaults",
      "relationshipDefaultsInfo": "Where this session starts. The engine evolves these as the conversation continues."
    },
    "soulPresets": {
      "secureLabel": "Secure",
      "secureBlurb": "Warm, steady, recovers quickly. Comfortable with closeness.",
      "anxiousLabel": "Anxious",
      "anxiousBlurb": "Strong attachment, fears distance, seeks reassurance.",
      "avoidantLabel": "Avoidant",
      "avoidantBlurb": "Self-reliant, slow to open up, keeps emotional distance.",
      "volatileLabel": "Volatile",
      "volatileBlurb": "Reactive, intense, expresses feelings without filter.",
      "reservedLabel": "Reserved",
      "reservedBlurb": "Quiet, composed, takes time to trust and reveal.",
      "playfulLabel": "Playful",
      "playfulBlurb": "Warm, expressive, light. Low tension, easy to laugh."
    },
    "soulFields": {
      "essence": "Essence",
      "essencePlaceholder": "Who they are underneath the card definition.",
      "essenceExample": "A practiced calm that breaks easily for the people they trust. Reads books to feel less alone, not to be impressive.",
      "voice": "Inner Voice",
      "voicePlaceholder": "How they sound in close conversation.",
      "voiceExample": "Low, deliberate, with long pauses. Drops formality when they let down their guard. Almost never sarcastic.",
      "relationalStyle": "Relational Style",
      "relationalStylePlaceholder": "How they attach, trust, retreat, reconnect.",
      "relationalStyleExample": "Slow to open up, but loyal once they do. Goes quiet when overwhelmed; comes back with a small gesture rather than an apology.",
      "vulnerabilities": "Vulnerabilities",
      "vulnerabilitiesPlaceholder": "Soft spots, insecurities, things they rarely say.",
      "vulnerabilitiesExample": "Afraid of being a burden. Hates feeling watched while struggling.",
      "habits": "Habits",
      "habitsPlaceholder": "Recurring tells, rituals, conversational patterns.",
      "habitsExample": "Tucks hair behind ear when nervous. Replies with questions when they don't know what to feel.",
      "boundaries": "Boundaries",
      "boundariesPlaceholder": "Lines they won't cross. Pace. Comfort limits.",
      "boundariesExample": "Won't be rushed into vulnerability. Steps back from cruelty even in jokes."
    },
    "soulSliders": {
      "warmth": "Warmth",
      "warmthLow": "Cold",
      "warmthHigh": "Affectionate",
      "trust": "Confiança",
      "trustLow": "Guarded",
      "trustHigh": "Open",
      "calm": "Calm",
      "calmLow": "Anxious",
      "calmHigh": "Steady",
      "vulnerability": "Vulnerability",
      "vulnerabilityLow": "Walled",
      "vulnerabilityHigh": "Exposed",
      "longing": "Longing",
      "longingLow": "Content",
      "longingHigh": "Yearning",
      "hurt": "Hurt",
      "hurtLow": "Healed",
      "hurtHigh": "Tender",
      "tension": "Tensão",
      "tensionLow": "Relaxed",
      "tensionHigh": "Wound up",
      "irritation": "Irritation",
      "irritationLow": "Patient",
      "irritationHigh": "Easily set off",
      "affection": "Afeto",
      "affectionLow": "Restrained",
      "affectionHigh": "Effusive",
      "reassuranceNeed": "Reassurance Need",
      "reassuranceNeedLow": "Self-soothing",
      "reassuranceNeedHigh": "Needs words",
      "suppression": "Suppression",
      "suppressionLow": "Expresses",
      "suppressionHigh": "Hides",
      "volatility": "Volatility",
      "volatilityLow": "Even-keeled",
      "volatilityHigh": "Reactive",
      "recoverySpeed": "Recovery Speed",
      "recoverySpeedLow": "Slow",
      "recoverySpeedHigh": "Fast",
      "conflictAvoidance": "Conflict Avoidance",
      "conflictAvoidanceLow": "Engages",
      "conflictAvoidanceHigh": "Withdraws",
      "reassuranceSeeking": "Reassurance Seeking",
      "reassuranceSeekingLow": "Independent",
      "reassuranceSeekingHigh": "Asks often",
      "protestBehavior": "Protest Behavior",
      "protestBehaviorLow": "Quiet",
      "protestBehaviorHigh": "Loud",
      "transparency": "Transparency",
      "transparencyLow": "Opaque",
      "transparencyHigh": "Reveals",
      "attachmentActivation": "Attachment Activation",
      "attachmentActivationLow": "Detached",
      "attachmentActivationHigh": "Triggers easily",
      "pride": "Pride",
      "prideLow": "Bends",
      "prideHigh": "Holds line",
      "closeness": "Starting Closeness",
      "closenessLow": "Strangers",
      "closenessHigh": "Intimate",
      "relTrust": "Starting Trust",
      "relTrustLow": "Wary",
      "relTrustHigh": "Trusting",
      "relAffection": "Starting Affection",
      "relAffectionLow": "Neutral",
      "relAffectionHigh": "Affectionate",
      "relTension": "Starting Tension",
      "relTensionLow": "Easy",
      "relTensionHigh": "Charged"
    },
    "soulReview": {
      "reviewTitle": "Review generated soul",
      "noDifferences": "No differences from current.",
      "changesHeader": "{{count}} change(s); edit anything before applying.",
      "close": "Close",
      "identityLabel": "Identity",
      "nEdited": "{{count}} edited",
      "edited": "Edited",
      "tuningLabel": "Tuning",
      "unchanged": "Unchanged",
      "nChanges": "{{count}} change(s)",
      "direction": "Direction",
      "directionApplyHint": "Edits apply on next Regenerate",
      "directionPlaceholder": "e.g. \"Lean tsundere - guarded outside, soft once trusted. Less anxious.\"",
      "directionTooltip": "Edit direction before regenerating",
      "regenerate": "Regenerate",
      "discard": "Discard",
      "apply": "Apply"
    },
    "hookErrors": {
      "creatorNotesInvalidJson": "Creator notes multilingual must be valid JSON object",
      "creatorNotesNotObject": "creatorNotesMultilingual must be a JSON object",
      "saveFailed": "Failed to save character",
      "importFailed": "Failed to import character",
      "avatarLoadFailed": "Avatar URL failed to load",
      "avatarProcessFailed": "Failed to process avatar image",
      "avatarConvertFailed": "Avatar URL could not be converted",
      "avatarUrlLoadFailed": "Failed to load avatar URL",
      "remoteAvatarDisabled": "Remote avatar download is disabled in Security settings.\nUpload an avatar manually.",
      "importReadyTitle": "Import ready",
      "importReadyMessage": "Detected {{label}}",
      "legacyJsonTitle": "Legacy JSON import detected",
      "legacyJsonMessage": "JSON imports are deprecated and will be removed soon. Use Settings > Convert Files.",
      "loadFailed": "Failed to load character",
      "exportFailed": "Failed to export character"
    }
  },
  "providers": {
    "empty": {
      "title": "Nenhum Provedor ainda",
      "description": "Adicione e gerencie provedores de API para modelos de IA",
      "addButton": "Adicionar Provedor"
    },
    "actions": {
      "openDashboard": "Abrir Painel",
      "openDashboardDesc": "Ver personagens, uso e configurações",
      "edit": "Editar",
      "editDesc": "Alterar configurações do provedor"
    },
    "extra": {
      "apiKeyNotFound": "OpenRouter API key not found. Please configure it in Settings > Providers first.",
      "audioEmpty": {
        "title": "Nenhum provedor de áudio",
        "description": "Adicione um provedor TTS para gerar vozes para seus personagens.",
        "addButton": "Adicionar Provedor"
      },
      "audioProviderLabel": {
        "elevenlabs": "ElevenLabs",
        "geminiTts": "Gemini TTS",
        "openaiTts": "OpenAI-Compatible TTS",
        "kokoro": "Kokoro (Local)"
      }
    },
    "audioEditor": {
      "titleEdit": "Editar Provedor",
      "titleCreate": "Adicionar Provedor de Áudio",
      "fields": {
        "providerType": "Tipo de Provedor",
        "label": "Rótulo",
        "apiKey": "Chave de API",
        "modelVariant": "Variante do Modelo",
        "assetRoot": "Diretório de Assets",
        "projectId": "ID do Projeto no Google Cloud",
        "region": "Região (opcional)",
        "baseUrl": "URL Base",
        "requestPath": "Caminho da Requisição"
      },
      "types": {
        "gemini": "Gemini TTS (Google)",
        "openai": "TTS Compatível com OpenAI",
        "kokoro": "Kokoro (Local)"
      },
      "placeholders": {
        "labelGemini": "Meu Gemini TTS",
        "labelOpenai": "Meu TTS Compatível",
        "labelKokoro": "Kokoro Local",
        "labelElevenlabs": "Meu ElevenLabs",
        "apiKey": "Insira sua chave de API",
        "assetRoot": "/caminho/para/kokoro",
        "projectId": "seu-id-de-projeto",
        "region": "us-central1",
        "baseUrl": "https://api.exemplo.com"
      },
      "errors": {
        "chooseModelVariant": "Escolha uma variante do modelo",
        "assetRootRequired": "O diretório de assets é obrigatório",
        "saveFailed": "Falha ao salvar",
        "apiKeyRequired": "A chave de API é obrigatória",
        "projectIdRequired": "O ID do projeto é obrigatório para o Gemini TTS",
        "baseUrlRequired": "A URL base é obrigatória para TTS compatível com OpenAI",
        "invalidCredentials": "Chave de API ou credenciais inválidas",
        "verificationFailed": "Falha na verificação"
      },
      "loadingVariants": "Carregando variantes...",
      "kokoroVariantHint": "Builds móveis suportam apenas int8. Instale o modelo pelo Kokoro Studio após salvar.",
      "managed": "Gerenciado",
      "managedPath": "Gerenciado: {{path}}",
      "requestPathHint": "Use o caminho do provedor se for diferente do padrão OpenAI",
      "verifying": "Verificando..."
    }
  },
  "models": {
    "empty": {
      "title": "Nenhum Modelo ainda",
      "description": "Adicione e gerencie modelos de IA de diferentes provedores",
      "addButton": "Adicionar Modelo"
    },
    "sort": {
      "alphabetical": "Alfabético",
      "byProvider": "Por provedor",
      "title": "Ordenar modelos",
      "alphabeticalDescription": "Ordenar por nome do modelo",
      "byProviderDescription": "Agrupar modelos por provedor"
    },
    "extra": {
      "cpuFallbackSucceeded": "Este modelo recorreu à CPU na última execução.",
      "cpuFallbackFailed": "Este modelo falhou na última execução."
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
    "title": "Explorador de Modelos",
    "searchPlaceholder": "Pesquisar modelos GGUF no HuggingFace...",
    "searching": "Pesquisando...",
    "trending": "Modelos GGUF em Tendência",
    "noResults": "Nenhum modelo encontrado",
    "noResultsHint": "Tente outro termo de pesquisa ou explore os modelos em tendência.",
    "likes": "curtidas",
    "downloads": "downloads",
    "viewFiles": "Ver Arquivos",
    "files": "Arquivos Disponíveis",
    "noFiles": "Nenhum arquivo GGUF encontrado neste repositório.",
    "architecture": "Arquitetura",
    "contextLength": "Comprimento do Contexto",
    "parameters": "Parâmetros",
    "quantization": "Quantização",
    "fileSize": "Tamanho",
    "download": "Baixar",
    "downloading": "Baixando...",
    "cancelDownload": "Cancelar Download",
    "downloadComplete": "Download concluído!",
    "downloadFailed": "Download falhou",
    "downloadCancelled": "Download cancelado",
    "downloadProgress": "{{downloaded}} / {{total}}",
    "progress": "Progresso",
    "fileOfTotal": "Arquivo {{current}} de {{total}}",
    "speed": "{{speed}}/s",
    "alreadyDownloaded": "Já baixado",
    "createModel": "Criar Modelo",
    "backToSearch": "Voltar à pesquisa",
    "backToFiles": "Voltar aos arquivos",
    "sortTrending": "Tendência",
    "sortDownloads": "Mais Baixados",
    "sortLikes": "Mais Curtidos",
    "sortRecent": "Atualizados Recentemente",
    "browseOnHuggingFace": "Explorar no HuggingFace",
    "selectFromLibrary": "Selecionar da Biblioteca",
    "libraryEmpty": "Nenhum modelo baixado ainda",
    "libraryEmptyHint": "Baixe modelos GGUF pelo Explorador de Modelos, ou insira um caminho manualmente.",
    "libraryTitle": "Modelos Baixados",
    "moveToLibrary": "Ei, posso mover o arquivo deste modelo para a pasta de modelos GGUF se quiser. Assim todos os seus modelos ficam organizados em um só lugar.",
    "moveToLibraryYes": "Sim, mover",
    "moveToLibraryNo": "Não, deixar onde está",
    "moveToLibraryMoving": "Movendo modelo...",
    "moveToLibrarySuccess": "Modelo movido com sucesso!",
    "moveToLibraryFailed": "Falha ao mover o modelo",
    "runabilityExcellent": "Excelente!",
    "runabilityGood": "Bom",
    "runabilityMarginal": "Marginal",
    "runabilityPoor": "Fraco",
    "runabilityUnrunnable": "Não pode executar",
    "recommendedSettings": "Configurações Recomendadas",
    "kvCacheType": "Tipo de Cache KV",
    "gpuFull": "Offload completo na GPU",
    "gpuNearFull": "GPU quase cheia, pequeno vazamento KV",
    "gpuKvSpill": "Pesos na GPU, KV vaza para RAM",
    "gpuKvHeavySpill": "Pesos na GPU, maior parte do KV na RAM",
    "gpuMostLayers": "Maioria das camadas na GPU",
    "gpuHalfLayers": "Metade das camadas na GPU",
    "gpuFewLayers": "Poucas camadas na GPU",
    "gpuCpu": "Apenas CPU",
    "notRecommended": "Não recomendamos executar este modelo no seu dispositivo. Não funcionará de forma fluida.",
    "moreDetails": "Mais",
    "detailedReport": "Relatório de Recursos",
    "detailSystem": "Recursos do Sistema",
    "detailRam": "RAM Disponível",
    "detailVram": "VRAM Disponível",
    "detailVramBudget": "Orçamento VRAM (90%)",
    "detailTotalAvailable": "Total Disponível",
    "detailArchitecture": "Arquitetura do Modelo",
    "detailArch": "Arquitetura",
    "detailLayers": "Camadas",
    "detailEmbedding": "Dim. de Embedding",
    "detailHeads": "Cabeças de Atenção",
    "detailKvHeads": "Cabeças KV",
    "detailFfn": "Dim. Feed-Forward",
    "detailTrainCtx": "Contexto de Treinamento",
    "detailConfig": "Configuração Atual",
    "detailModelSize": "Tamanho do Arquivo do Modelo",
    "detailMemory": "Detalhamento de Memória",
    "detailWeights": "Pesos do Modelo",
    "detailKvCache": "Cache KV",
    "detailTotalNeeded": "Total Necessário",
    "detailHeadroom": "Margem",
    "detailGpuFit": "Offload GPU",
    "detailScoreBreakdown": "Detalhamento da Pontuação",
    "detailMemFitness": "Aptidão de Memória",
    "detailGpuAccel": "Aceleração GPU",
    "detailKvHeadroom": "Margem KV",
    "detailQuantQuality": "Qualidade de Quantização",
    "detailFinalScore": "Pontuação Ponderada",
    "detailComputeBuffer": "Buffer de Computação",
    "detailMemMode": "Modo de Memória",
    "detailUnified": "Unificada (RAM/VRAM compartilhada)",
    "detailSwa": "Janela Deslizante",
    "detailMlaRank": "Rank Latente MLA",
    "detailParseStatus": "Análise de Cabeçalho",
    "detailIncomplete": "Incompleto (metadados MoE muito grandes)",
    "detailEffectiveKvCtx": "Contexto KV Efetivo",
    "detailOffload": "Offload GPU",
    "detailCtxTip": "Reduzir o contexto para {{ctx}} tokens permitiria 100% de offload na GPU.",
    "upgradeSuggestion": "{{quant}} ({{size}}) também cabe e pontua {{score}} — melhor qualidade.",
    "layerTip": "Recomendado: offload de {{layers}}/{{total}} camadas (-ngl {{layers}})",
    "detailKvDistribution": "Distribuição do Cache KV",
    "detailKvOnGpu": "GPU (VRAM)",
    "detailKvOnRam": "RAM do Sistema",
    "kvDistributionTip": "{{pct}}% do cache KV está na RAM. O processamento do prompt (prefill) será mais lento — 100% GPU mantém instantâneo.",
    "detailLayers-ngl": "Camadas para Offload (-ngl)",
    "detailOptimalGpuCtx": "Contexto GPU Ótimo",
    "detailOptimalRamCtx": "Contexto Máx. RAM",
    "optimalGpuCtxLabel": "Velocidade total GPU: {{ctx}} tokens",
    "optimalRamCtxLabel": "Máx. RAM: {{ctx}} tokens",
    "optimalGpuCtxShort": "GPU: {{ctx}}",
    "optimalRamCtxShort": "Máx: {{ctx}}",
    "fullGpuOffloadShort": "100% GPU: {{ctx}}",
    "ctxExceedsGpu": "O contexto excede o ótimo para GPU ({{ctx}}). O cache KV vazará para a RAM, reduzindo a velocidade.",
    "modelExceedsVram": "O modelo excede a VRAM. Executando a partir da RAM com offload parcial na GPU."
  },
  "systemPrompts": {
    "filters": {
      "all": "Todos",
      "system": "Sistema",
      "internal": "Interno",
      "custom": "Personalizado"
    },
    "empty": {
      "title": "Nenhum prompt personalizado ainda",
      "description": "Crie prompts de sistema personalizados para personalizar suas conversas com IA",
      "createButton": "Criar Prompt"
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
      "title": "Nenhuma persona ainda",
      "description": "Crie uma persona para definir como a IA deve se dirigir a você",
      "createButton": "Criar Persona"
    },
    "actions": {
      "editPersona": "Editar Persona",
      "setAsDefault": "Definir como Padrão",
      "setAsDefaultDesc": "Usar para todas as novas conversas",
      "unsetAsDefault": "Remover Padrão",
      "unsetAsDefaultDesc": "Remover status de padrão",
      "exportPersona": "Exportar Persona",
      "deletePersona": "Excluir Persona"
    },
    "edit": {
      "avatarHint": "Toque para adicionar ou gerar avatar",
      "nameLabel": "NOME DA PERSONA",
      "namePlaceholder": "ex., Profissional, Escritor Criativo, Estudante...",
      "nameHint": "Dê um nome descritivo à sua persona",
      "nicknameLabel": "APELIDO (OPCIONAL)",
      "nicknamePlaceholder": "ex., Variante de trabalho, Modo RPG...",
      "nicknameHint": "Um apelido privado para distinguir variantes desta persona na sua biblioteca",
      "descriptionLabel": "DESCRIÇÃO",
      "descriptionPlaceholder": "Descreva como a IA deve se dirigir a você, suas preferências, histórico ou estilo de comunicação...",
      "wordCount": "palavras",
      "descriptionHint": "Seja específico sobre como você quer ser tratado",
      "setAsDefault": "Definir como Padrão",
      "defaultDescription": "Usar esta persona para todas as novas conversas",
      "exportButton": "Exportar Persona"
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
      "off": "Desligado",
      "offDesc": "Todo conteúdo permitido",
      "low": "Baixo",
      "lowDesc": "Bloqueia conteúdo sexual explícito + palavrões",
      "standard": "Padrão",
      "standardDesc": "Bloqueia NSFW + violência gráfica",
      "strict": "Rigoroso",
      "strictDesc": "Filtragem máxima + sem tom sugestivo"
    }
  },
  "advanced": {
    "sectionTitles": {
      "aiFeatures": "Recursos de IA",
      "memorySystem": "Sistema de Memória",
      "usageAnalytics": "Análise de Uso"
    },
    "creationHelper": {
      "title": "Assistente de Criação",
      "description": "Assistente guiado por IA para criação de personagens"
    },
    "helpMeReply": {
      "title": "Me Ajude a Responder",
      "description": "Sugestões de resposta assistidas por IA"
    },
    "dynamicMemory": {
      "title": "Memória Dinâmica",
      "contextWindow": "Janela de Contexto",
      "contextWindowDesc": "Número de mensagens recentes a incluir (1-1000)",
      "infoText": "A Memória Dinâmica usa IA para resumir e gerenciar automaticamente o contexto da conversa, permitindo conversas mais longas e coerentes.",
      "disabledText": "Quando desativada, o app usa uma janela deslizante simples de mensagens recentes determinada pela configuração da Janela de Contexto."
    },
    "usageAnalytics": {
      "recalculateTitle": "Recalcular Custos de Uso",
      "recalculateDesc": "Atualizar todos os registros de uso históricos com preços corretos",
      "recalculating": "Recalculando...",
      "recalculateButton": "Recalcular Todos os Custos",
      "openRouterApiKeyRequired": "Chave de API do OpenRouter necessária. Configure-a em Configurações → Provedores.",
      "importantLabel": "Importante:",
      "warningCannotUndo": "Esta operação não pode ser desfeita",
      "warningMayTakeTime": "Pode levar tempo se você tiver muitos registros",
      "warningOnlyOpenRouter": "Somente registros OpenRouter com tokens serão atualizados",
      "warningExistingValues": "Valores de custo existentes serão sobrescritos"
    },
    "extra": {
      "creationHelperDetail": "Get intelligent suggestions for personality traits, backstory, and dialogue style",
      "helpMeReplyDetail": "Generate contextual response options based on conversation history",
      "lorebookEntryGenerator": "Lorebook Entry Generator",
      "lorebookEntryDesc": "Turn selected chat messages into durable lorebook entries and configure the draft prompts for entry writing and keyword generation.",
      "companions": "Companions",
      "companionModeDesc": "Manage local analysis models for emotion, entity extraction, and memory routing used by companion characters.",
      "companionSoulWriter": "Escritor de Companion Soul",
      "companionSoulDesc": "Escolha o modelo, o modelo de fallback e o template de prompt usados para redigir Companion Souls. Primeiro chamada de ferramenta, fallback estruturado se não houver suporte.",
      "network": "Network",
      "apiServer": "API Server",
      "apiServerDesc": "Expose models via an OpenAI-compatible API server",
      "apiServerRunning": "Server is currently running"
    }
  },
  "backup": {
    "tabs": {
      "create": "Criar"
    },
    "create": {
      "newBackupButton": "Novo Backup",
      "exportDescription": "Exportar todos os dados com criptografia",
      "createButton": "Criar Backup"
    },
    "restore": {
      "availableBackups": "Backups Disponíveis",
      "browseFiles": "Procurar Arquivos",
      "noBackupsFound": "Nenhum backup encontrado",
      "noBackupsDesc": "Crie um backup ou toque em \"Procurar Arquivos\" para encontrar um",
      "browseDesc": "Procurar arquivo .lettuce",
      "restoreDialogTitle": "Restaurar Backup",
      "deleteDialogTitle": "Excluir Backup",
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
    "title": "Reiniciar Tudo",
    "description": "Isso excluirá permanentemente todos os provedores, modelos, personagens, sessões de conversa e preferências deste dispositivo.",
    "warning": "Esta ação não pode ser desfeita",
    "resetButton": "Reiniciar Todos os Dados",
    "confirmTitle": "Tem Certeza?",
    "confirmDescription": "Todos os seus dados serão excluídos permanentemente. O aplicativo retornará à configuração inicial.",
    "confirmButton": "Sim, Reiniciar Tudo"
  },
  "chatAppearance": {
    "typography": "Tipografia",
    "fontSize": {
      "label": "Tamanho da Fonte",
      "small": "Pequeno",
      "medium": "Médio",
      "large": "Grande",
      "xLarge": "Extra Grande"
    },
    "lineSpacing": {
      "label": "Espaçamento de Linha",
      "tight": "Apertado",
      "normal": "Normal",
      "relaxed": "Relaxado"
    },
    "messageBubbles": {
      "label": "Balões de Mensagem",
      "style": {
        "label": "Estilo",
        "bordered": "Com Borda",
        "filled": "Preenchido",
        "minimal": "Minimalista"
      },
      "cornerRadius": {
        "label": "Raio do Canto",
        "sharp": "Afiado",
        "rounded": "Arredondado",
        "pill": "Pílula"
      },
      "maxWidth": {
        "label": "Largura Máxima",
        "compact": "Compacto",
        "normal": "Normal",
        "wide": "Largo"
      },
      "padding": {
        "label": "Preenchimento",
        "compact": "Compacto",
        "normal": "Normal",
        "spacious": "Espaçoso"
      }
    },
    "layout": {
      "label": "Layout",
      "messageSpacing": "Espaçamento de Mensagens",
      "tight": "Apertado",
      "normal": "Normal",
      "relaxed": "Relaxado"
    },
    "avatar": {
      "shape": {
        "label": "Forma do Avatar",
        "circle": "Círculo",
        "rounded": "Arredondado",
        "hidden": "Oculto"
      },
      "size": {
        "label": "Tamanho do Avatar",
        "small": "Pequeno",
        "medium": "Médio",
        "large": "Grande"
      }
    },
    "colors": {
      "label": "Cores",
      "userBubble": "Cor do Balão do Usuário",
      "assistantBubble": "Cor do Balão do Assistente",
      "userBubbleHex": "Hex do Balão do Usuário",
      "assistantBubbleHex": "Hex do Balão do Assistente",
      "neutral": "Neutro",
      "accent": "Destaque",
      "info": "Info",
      "secondary": "Secundário",
      "warning": "Aviso",
      "textColors": "Text Colors",
      "messageTextHex": "Cor de citação em linha",
      "plainTextHex": "Plain Text Color",
      "italicTextHex": "Italic Text Color",
      "quotedTextHex": "Cor de citação em bloco",
      "inlineCodeTextHex": "Cor do código inline"
    },
    "backgroundTransparency": {
      "label": "Fundo e Transparência",
      "backgroundDim": "Escurecimento do Fundo",
      "backgroundBlur": "Desfoque do Fundo",
      "bubbleBlur": "Desfoque do Balão",
      "none": "Nenhum",
      "light": "Leve",
      "medium": "Médio",
      "heavy": "Forte",
      "bubbleOpacity": "Opacidade do Balão"
    },
    "textColorMode": {
      "label": "Modo de Cor do Texto",
      "auto": "Automático",
      "light": "Claro",
      "dark": "Escuro"
    },
    "preview": {
      "label": "Pré-visualização",
      "generic": "Genérico",
      "live": "Ao Vivo"
    },
    "extra": {
      "reset": "Reset"
    }
  },
  "colorCustomization": {
    "tokens": {
      "surface": "Superfície",
      "surfaceDesc": "Fundos de página",
      "surfaceEl": "Superfície Elevada",
      "surfaceElDesc": "Cards, modais, elementos elevados",
      "nav": "Navegação",
      "navDesc": "Barras superior e inferior",
      "foreground": "Primeiro plano",
      "foregroundDesc": "Bordas, sobreposições, navegação e elementos da interface",
      "appText": "Texto do app",
      "appTextDesc": "Texto principal e rótulos da interface",
      "appTextMuted": "Texto secundário",
      "appTextMutedDesc": "Texto secundário e texto de apoio",
      "appTextSubtle": "Texto sutil",
      "appTextSubtleDesc": "Dicas, texto auxiliar e marcadores de posição",
      "accent": "Destaque",
      "accentDesc": "Ações primárias, sucesso",
      "info": "Info",
      "infoDesc": "Estados informativos, links",
      "warning": "Aviso",
      "warningDesc": "Estados de cautela, alertas",
      "danger": "Perigo",
      "dangerDesc": "Ações destrutivas, erros",
      "secondary": "Secundário",
      "secondaryDesc": "Recursos de IA, ferramentas criativas"
    },
    "presetsLabel": "Predefinições",
    "customPresetsLabel": "Predefinições Personalizadas",
    "previewLabel": "Pré-visualização",
    "settingsCardsLabel": "Cartões de configurações",
    "settingsCardsOpacity": "Opacidade dos cartões",
    "settingsCardsOpacityDesc": "Controla o quão transparentes parecem os cartões de configurações e as linhas de lista.",
    "importButton": "Importar",
    "exportButton": "Exportar",
    "resetAllButton": "Reiniciar Tudo",
    "presets": {
      "defaultDark": "Escuro Padrão",
      "midnightBlue": "Azul Meia-Noite",
      "warmEarth": "Terra Quente",
      "purpleHaze": "Névoa Roxa",
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
      "monochrome": "Monocromático"
    },
    "groups": {
      "backgrounds": "Fundos",
      "content": "Conteúdo",
      "semantic": "Semântico"
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
      "info": "A Memória Dinâmica resume automaticamente as conversas para manter o contexto de forma eficiente. Escolha um preset ou ajuste as configurações conforme suas necessidades.",
      "disabledDirectTitle": "A memória dinâmica está desativada para chats diretos",
      "disabledDirectDescription": "Ative o interruptor na aba de Chats Diretos para habilitá-la. Os chats em grupo usam o modo de memória por sessão.",
      "directChats": "Chats Diretos",
      "groupChats": "Chats em Grupo",
      "enableDirectChats": "Ativar para Chats Diretos",
      "groupChatsInfo": "Os chats em grupo usam o modo de memória por sessão. Ative a memória dinâmica nas configurações de cada grupo. Estas configurações controlam como a memória dinâmica se comporta.",
      "memoryProfile": "Perfil de Memória",
      "customSettings": "Configurações personalizadas - ajuste os valores nas Opções Avançadas abaixo.",
      "contextEnrichment": "Enriquecimento de Contexto",
      "experimental": "Experimental",
      "contextEnrichmentDescription": "Usa mensagens recentes para uma recuperação de memória mais inteligente",
      "advancedOptions": "Opções Avançadas",
      "advancedOptionsDescription": "Ajuste fino do comportamento da memória",
      "summaryInterval": "Intervalo de Resumo",
      "summaryIntervalDescription": "Mensagens entre resumos",
      "maxMemoryEntries": "Máximo de Entradas de Memória",
      "maxMemoryEntriesDescription": "Máximo de memórias armazenadas",
      "hotMemoryBudget": "Orçamento de Memória Ativa",
      "hotMemoryBudgetDescription": "Limite de tokens para memórias ativas",
      "relevanceThreshold": "Limiar de Relevância",
      "relevanceThresholdDescription": "Similaridade mínima para recuperação",
      "retrievalMode": "Modo de Recuperação",
      "retrievalModeSmart": "Inteligente",
      "retrievalModeCosine": "Cosseno",
      "retrievalModeDescription": "Inteligente combina relevância com recência/frequência. Cosseno usa a similaridade pura mais alta.",
      "retrievalLimit": "Limite de Recuperação",
      "retrievalLimitDescription": "Máximo de memórias selecionadas por turno",
      "decayRate": "Taxa de Decaimento",
      "decayRateDescription": "Quão rápido a importância diminui",
      "coldStorageThreshold": "Limiar de Armazenamento Frio",
      "coldStorageThresholdDescription": "Quando as memórias são movidas para o arquivo",
      "sharedSettings": "Configurações Compartilhadas",
      "summarisationModel": "Modelo de Resumo",
      "selectedModel": "Modelo Selecionado",
      "useGlobalDefaultModel": "Usar modelo padrão global",
      "noModelsAvailable": "Nenhum modelo disponível",
      "summarisationModelDescription": "Usado para resumo de conversas",
      "modelManagement": "Gerenciamento de Modelos",
      "testModel": "Testar Modelo",
      "downloadModel": "Baixar Modelo",
      "delete": "Excluir",
      "embeddingModel": "Modelo de Embedding",
      "tokenCapacity": "Capacidade de Tokens",
      "tokenCapacityDescription": "Valores maiores = melhor memória para conversas longas",
      "keepModelLoaded": "Manter Modelo Carregado",
      "keepModelLoadedDescription": "Mantém o modelo de embedding + tokenizador na memória para evitar sobrecarga de recarregamento",
      "installedModel": "Modelo instalado: {{version}} ({{tokens}} tokens máx.)",
      "downloadEmbeddingModel": "Baixar Modelo de Embedding",
      "downloadEmbeddingDescription": "Escolha qual versão baixar. Versões instaladas estão desabilitadas.",
      "downloadVersion": "Baixar {{version}}",
      "downloadV2Description": "Otimizado para precisão e recuperação de contexto longo",
      "downloadV3Description": "Qualidade de embedding mais recente",
      "installed": "Instalado",
      "selectModel": "Selecionar Modelo",
      "searchModels": "Pesquisar modelos...",
      "deleteEmbeddingTitle": "Excluir modelo {{version}}?",
      "deleteEmbeddingMessage": "Tem certeza de que deseja excluir {{version}}? Você pode baixá-lo novamente depois.",
      "msgsUnit": "msgs",
      "entriesUnit": "entradas",
      "tokensUnit": "tokens",
      "itemsUnit": "itens",
      "perCycleUnit": "/ ciclo"
    },
    "presets": {
      "minimal": "mínimo",
      "balanced": "equilibrado",
      "comprehensive": "abrangente",
      "minimalDesc": "Fast & efficient. Keeps only essential memories.",
      "balancedDesc": "Good mix of context retention and performance.",
      "comprehensiveDesc": "Maximum context. Best for long, detailed conversations."
    },
    "presetInfo": {
      "minimal": "Rápido e eficiente. Mantém apenas as memórias essenciais.",
      "balanced": "Boa combinação de retenção de contexto e desempenho.",
      "comprehensive": "Contexto máximo. Ideal para conversas longas e detalhadas."
    }
  },
  "helpMeReply": {
    "page": {
      "info": "Ajude-me a Responder gera sugestões contextuais para a sua próxima mensagem com base no histórico da conversa. Configure o modelo e o estilo de resposta abaixo."
    },
    "sectionTitles": {
      "modelConfiguration": "Configuração do Modelo",
      "responseStyle": "Estilo de Resposta"
    },
    "labels": {
      "replyModel": "Modelo de Resposta",
      "selectedModel": "Modelo Selecionado",
      "useAppDefault": "Usar padrão do app{{model}}",
      "useAppDefaultBase": "Usar padrão do app",
      "noModelsAvailable": "Nenhum modelo disponível",
      "replyModelDescription": "Modelo de IA para gerar sugestões de resposta",
      "streamingOutput": "Saída em Streaming",
      "streamingDescription": "Mostrar sugestões enquanto são geradas",
      "maxTokens": "Máximo de Tokens",
      "maxTokensDescription": "Comprimento máximo das sugestões",
      "conversationalHint": "As sugestões serão escritas como diálogo natural, adequado para conversas casuais.",
      "roleplayHint": "As sugestões incluirão elementos de roleplay como *ações* e descrições narrativas.",
      "footerInfo": "Esta configuração se aplica globalmente em todas as conversas. Menos tokens geram sugestões mais curtas e rápidas, enquanto mais tokens permitem respostas mais detalhadas.",
      "selectReplyModel": "Selecionar Modelo de Resposta",
      "searchModels": "Pesquisar modelos..."
    },
    "responseStyle": {
      "conversational": "Conversacional",
      "conversationalDesc": "Tom natural e casual",
      "roleplay": "Roleplay",
      "roleplayDesc": "Ações em personagem"
    },
    "extra": {
      "conversational": "Conversational",
      "roleplay": "Roleplay"
    }
  },
  "imageGeneration": {
    "promptPlaceholder": "Descreva a imagem que você quer gerar...",
    "labels": {
      "model": "MODELO",
      "prompt": "PROMPT",
      "size": "TAMANHO",
      "quality": "QUALIDADE",
      "style": "ESTILO",
      "searchModels": "Pesquisar modelos...",
      "selectAvatarModel": "Selecione o modelo do avatar",
      "selectSceneModel": "Selecione o modelo de cena",
      "selectWriterModel": "Selecionar modelo de redator de cena",
      "useFirstAvailable": "Use o primeiro modelo disponível",
      "useFirstCompatible": "Usar o primeiro modelo de redator compatível"
    },
    "mode": {
      "title": "Comportamento",
      "description": "Defina como os prompts de cena detectados na saída do modelo devem ser tratados.",
      "auto": "Automático",
      "autoDescription": "Gere a imagem da cena assim que o modelo fornecer um prompt de cena.",
      "askFirst": "Perguntar antes",
      "askFirstDescription": "Mostre o prompt de cena detectado e espere sua aprovação antes de gerar uma imagem.",
      "manual": "Manual",
      "manualDescription": "Ignore prompts de cena nas respostas do modelo. Use apenas ações iniciadas manualmente pelo usuário."
    },
    "empty": {
      "title": "Sem Modelos de Imagem",
      "description": "Adicione um modelo de geração de imagem na página de Modelos para começar a gerar imagens."
    },
    "sections": {
      "avatar": {
        "title": "Geração de Avatares",
        "description": "Modelo padrão usado ao gerar avatares a partir do seletor de avatar ou de fluxos de imagens de perfil relacionados."
      },
      "scene": {
        "title": "Geração de cena",
        "description": "Modelo reservado para imagens de cena geradas a partir de contexto de conversa ou prompts de cena."
      },
      "writer": {
        "title": "Redator de cena",
        "description": "Modelo de texto multimodal reservado para redigir prompts de cena e descrições de referência de design a partir do contexto do chat, avatares e imagens de referência."
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
      "diagnostics": "Diagnósticos",
      "generate": "Gerar",
      "copy": "Copiar"
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
      "testDataGenerators": "Geradores de Dados de Teste",
      "storageMaintenance": "Manutenção de Armazenamento",
      "usageTracking": "Rastreamento de Uso",
      "crashTesting": "Teste de colisão",
      "environmentInfo": "Informações do Ambiente"
    },
    "testData": {
      "generateCharacter": "Gerar Personagem de Teste",
      "generateCharacterDesc": "Criar um único personagem de teste",
      "generatePersona": "Gerar Persona de Teste",
      "generatePersonaDesc": "Criar uma única persona de teste",
      "generateSession": "Gerar Sessão de Teste",
      "generateSessionDesc": "Criar uma sessão de conversa de teste com personagem existente",
      "generateBulk": "Gerar Dados de Teste em Massa",
      "generateBulkDesc": "Criar 3 personagens e 2 personas"
    },
    "storageMaintenance": {
      "optimizeDb": "Otimizar Banco de Dados",
      "optimizeDbDesc": "Aplicar PRAGMAs e executar VACUUM (somente mobile)",
      "backupLegacy": "Backup e Remoção de Arquivos Legados",
      "backupLegacyDesc": "Move arquivos .bin legados para uma pasta de backup"
    },
    "usageTracking": {
      "recalculateAll": "Recalcular Todos os Custos de Uso",
      "recalculateAllDesc": "Re-busca preços e recalcula custos para todos os registros de uso do OpenRouter"
    },
    "crashTesting": {
      "forceCrash": "Crash App agora",
      "forceCrashDesc": "Encerra imediatamente o processo do aplicativo nativo para testar a detecção de falhas",
      "forceCrashConfirm": "Isso travará imediatamente o aplicativo para testar o detector de falhas. Continuar?"
    },
    "environmentInfo": {
      "mode": "Modo",
      "devMode": "Modo Dev",
      "viteVersion": "Versão do Vite"
    },
    "status": {
      "testCharacterCreated": "✓ Personagem de teste criado com sucesso",
      "testPersonaCreated": "✓ Persona de teste criada com sucesso",
      "testSessionCreated": "✓ Sessão de teste criada: {{id}}",
      "generatingBulkData": "Gerando dados de teste em massa...",
      "bulkDataCreated": "✓ Dados de teste em massa criados: 3 personagens, 2 personas",
      "creatingBenchmarkChat": "Criando personagem e sessão de benchmark semeados...",
      "seededBenchmarkReady": "✓ Benchmark semeado pronto: {{name}} / {{id}}",
      "creatingBenchmarkGroupChat": "Criando chat em grupo de benchmark semeado...",
      "seededGroupBenchmarkReady": "✓ Benchmark de grupo semeado pronto: {{id}}",
      "dbOptimized": "✓ Banco de dados otimizado",
      "recalculatingCosts": "Recalculando custos de uso... Isso pode demorar um pouco.",
      "toursReset": "✓ Todos os tours guiados redefinidos — eles aparecerão novamente na próxima visita",
      "crashingApp": "Travando o aplicativo..."
    },
    "errors": {
      "noCharacters": "Nenhum personagem disponível. Crie um personagem de teste primeiro.",
      "createCharacterFailed": "Falha ao criar personagem de teste: {{error}}",
      "createPersonaFailed": "Falha ao criar persona de teste: {{error}}",
      "createSessionFailed": "Falha ao criar sessão de teste: {{error}}",
      "createBulkFailed": "Falha ao criar dados de teste em massa: {{error}}",
      "createBenchmarkFailed": "Falha ao criar sessão de benchmark semeada: {{error}}",
      "createGroupBenchmarkFailed": "Falha ao criar sessão de benchmark de grupo semeada: {{error}}",
      "dbOptimizeFailed": "Falha na otimização do BD: {{error}}",
      "backupFailed": "Falha no backup: {{error}}",
      "openRouterKeyMissing": "Chave de API do OpenRouter não encontrada. Configure-a em Configurações > Provedores primeiro.",
      "recalculationFailed": "Falha no recálculo: {{error}}",
      "resetToursFailed": "Falha ao redefinir tours: {{error}}",
      "crashFailed": "Falha ao travar o aplicativo: {{error}}"
    },
    "onboarding": {
      "title": "Integração",
      "resetTours": "Redefinir todos os tours guiados",
      "resetToursDesc": "Limpa o estado de visualização de todos os tours de integração para que sejam reproduzidos na próxima visita."
    },
    "benchmarks": {
      "createChat": "Criar chat de benchmark semeado",
      "createChatDesc": "Cria um personagem com memória dinâmica, cena inicial e uma sessão de teste de continuidade com 20 mensagens, e então a abre.",
      "createGroupChat": "Criar chat em grupo de benchmark semeado",
      "createGroupChatDesc": "Cria um chat em grupo com memória dinâmica com três personagens de benchmark e 30 mensagens semeadas, e então o abre."
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
      "oneK": "1K tokens",
      "oneKDesc": "Melhor para respostas rápidas",
      "twoK": "2K tokens",
      "twoKDesc": "Desempenho equilibrado",
      "fourK": "4K tokens",
      "fourKDesc": "Contexto máximo"
    },
    "extra": {
      "status": "Downloading..."
    }
  },
  "embeddingTest": {
    "categories": {
      "semanticSimilarity": "Similaridade Semântica",
      "dissimilarityCheck": "Verificação de Dissimilaridade",
      "roleplayContext": "Contexto de Roleplay"
    },
    "extra": {
      "placeholder": "Enter text to embed..."
    }
  },
  "discovery": {
    "tabs": {
      "forYou": "Para Você",
      "trending": "Em Alta",
      "popular": "Popular",
      "new": "Novo"
    },
    "searchPlaceholder": "Pesquisar personagens...",
    "viewAll": "Ver Todos",
    "errorTitle": "Algo deu errado",
    "noCardsFound": "Nenhum card encontrado",
    "sections": {
      "trendingNow": "Em Alta Agora",
      "trendingSubtitle": "Populares esta semana",
      "mostPopular": "Mais Populares",
      "popularSubtitle": "Favoritos da comunidade",
      "freshArrivals": "Recém-Chegados",
      "freshSubtitle": "Adicionados recentemente"
    },
    "browse": {
      "newArrivals": "Recém-Chegados",
      "freshCharacters": "Personagens novos",
      "noCharactersFound": "Nenhum personagem encontrado",
      "noCharactersSubtitle": "Volte mais tarde para novo conteúdo"
    },
    "sort": {
      "mostLiked": "Mais Curtidos",
      "mostDownloaded": "Mais Baixados",
      "mostViewed": "Mais Vistos",
      "mostMessages": "Mais Mensagens",
      "newestFirst": "Mais Recentes",
      "recentlyUpdated": "Atualizados Recentemente",
      "nameAZ": "Nome (A-Z)"
    },
    "sortBy": "Ordenar Por",
    "resultsUnit": "personagens",
    "detail": {
      "share": "Compartilhar",
      "nsfwOverlay": "Conteúdo NSFW",
      "nsfwBadge": "NSFW",
      "originalBadge": "Original",
      "lorebookBadge": "Livro de Lore",
      "alsoKnownAs": "Também conhecido como:",
      "followersUnit": "seguidores",
      "sections": {
        "description": "Descrição",
        "tokenUsage": "Uso de Tokens",
        "startingScenes": "Cenas Iniciais",
        "scenario": "Cenário",
        "personality": "Personalidade",
        "stats": "Estatísticas",
        "tags": "Tags",
        "author": "Autor"
      },
      "tokensTotalLabel": "total",
      "tokens": {
        "description": "Descrição",
        "personality": "Personalidade",
        "scenario": "Cenário",
        "firstMessage": "Primeira Mensagem",
        "scenes": "Cenas",
        "examples": "Exemplos",
        "systemPrompt": "Prompt de Sistema"
      },
      "sceneLabels": {
        "primary": "Principal",
        "alternate": "Alternativa"
      },
      "stats": {
        "views": "Visualizações",
        "downloads": "Downloads",
        "messages": "Mensagens"
      },
      "downloaded": "Baixado",
      "startChat": "Iniciar Conversa",
      "downloadCharacter": "Baixar Personagem",
      "downloading": "Baixando...",
      "downloadSuccess": {
        "title": "Personagem Baixado!",
        "subtitle": "Adicionado à sua biblioteca",
        "badge": "Salvo",
        "startChat": "Iniciar Conversa",
        "startChatDesc": "Abrir a primeira cena agora",
        "viewLibrary": "Ver na Biblioteca",
        "viewLibraryDesc": "Editar, gerenciar ou exportar depois",
        "continueBrowsing": "Continuar Navegando",
        "continueBrowsingDesc": "Voltar para descoberta"
      },
      "errorTitle": "Erro",
      "errorSubtitle": "Falha ao carregar",
      "errorNotFound": "Personagem não encontrado",
      "defaultChatTitle": "New Chat"
    },
    "search": {
      "placeholder": "Pesquisar personagens, tags, autores...",
      "resultsUnit": "resultados",
      "timingUnit": "ms",
      "recentSearches": "Pesquisas Recentes",
      "clearAll": "Limpar tudo",
      "trendingSearches": "Pesquisas em Alta",
      "trends": {
        "anime": "anime",
        "fantasy": "fantasia",
        "romance": "romance",
        "villain": "vilão",
        "adventure": "aventura",
        "comedy": "comédia",
        "mystery": "mistério",
        "sciFi": "sci-fi"
      },
      "tips": {
        "title": "Dicas de Pesquisa",
        "tip1": "Pesquise por nome do personagem, autor ou descrição",
        "tip2": "Use tags como \"anime\", \"fantasia\" ou \"romance\"",
        "tip3": "Tente traços específicos como \"tsundere\" ou \"vilão\""
      },
      "loading": "Carregando...",
      "loadMore": "Carregar Mais",
      "noResults": "Nenhum resultado encontrado",
      "noResultsFor": "Nenhum personagem encontrado para",
      "noResultsHint": "Tente palavras-chave diferentes ou navegue pelas categorias"
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
    "gpuInsufficient": "Memória GPU insuficiente",
    "gpuFallbackDesc": "Este modelo não cabe na memória GPU. Mudar para CPU (mais lento) ou cancelar?",
    "switchToCpu": "Mudar para CPU",
    "abort": "Cancelar",
    "errors": {
      "providerNotFound": "Provedor do motor não encontrado.",
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
      "connected": "Conectado",
      "offline": "Offline",
      "needsSetup": "Precisa de Configuração"
    },
    "home": {
      "characters": "Personagens",
      "newButton": "Novo",
      "noCharactersFound": "Nenhum personagem encontrado.",
      "tokenUsage": "Uso de Tokens",
      "totalTokens": "tokens totais",
      "backgroundActivity": "Atividade em Segundo Plano",
      "quickActions": "Ações Rápidas",
      "configureProviders": "Configurar Provedores",
      "engineSettings": "Configurações do Motor",
      "chat": "Conversa",
      "chatDesc": "Iniciar uma conversa com este personagem",
      "deleteCharacter": "Excluir Personagem",
      "deletingCharacter": "Excluindo...",
      "deleteDesc": "Remover permanentemente este personagem",
      "character": "Personagem",
      "never": "Nunca",
      "justNow": "Agora mesmo",
      "timeAgo": {
        "minutes": "{{n}}m ago",
        "hours": "{{n}}h ago",
        "days": "{{n}}d ago"
      }
    },
    "tokens": {
      "input": "Entrada",
      "output": "Saída"
    },
    "activity": {
      "synthesis": "Síntese",
      "consolidation": "Consolidação",
      "bm25Rebuild": "Reconstrução BM25",
      "dripResearch": "Pesquisa Gradual",
      "running": "Executando",
      "stopped": "Parado"
    },
    "setup": {
      "complete": "Configuração Concluída!",
      "completeMessage": "Seu Lettuce Engine está configurado e pronto para usar.",
      "openDashboard": "Abrir Painel"
    },
    "welcome": {
      "title": "Bem-vindo ao Lettuce Engine",
      "subtitle": "Vamos configurar seu motor de personagens IA. Isso levará cerca de 2 minutos.",
      "feature1": "O Motor dá aos seus personagens IA memória persistente, emoções, relacionamentos e uma identidade real.",
      "feature2": "Primeiro, vamos configurar um backend LLM, depois configurar as opções do motor.",
      "getStarted": "Vamos Lá"
    },
    "config": {
      "activeProviders": "Provedores Ativos",
      "noModelSet": "Nenhum modelo definido",
      "defaultBadge": "Padrão",
      "noProvidersWarning": "Nenhum provedor configurado. Adicione pelo menos um backend LLM abaixo.",
      "addProvider": "Adicionar Provedor",
      "quickImport": "Importação rápida dos seus provedores do app",
      "importButton": "Importar",
      "fields": {
        "model": "Modelo",
        "modelPlaceholder": "ex. claude-sonnet-4-5-20250929",
        "apiKey": "Chave de API",
        "apiKeyPlaceholder": "Digite sua chave de API",
        "currentKey": "Chave atual:",
        "baseUrl": "URL Base",
        "baseUrlPlaceholder": "http://localhost:11434",
        "maxTokens": "Máx. Tokens",
        "temperature": "Temperatura"
      },
      "enableProvider": "Ativar Provedor",
      "setAsDefault": "Definir como Padrão",
      "defaultBackend": "Backend Padrão",
      "remove": "Remover",
      "saveChanges": "Salvar Alterações",
      "saving": "Salvando...",
      "saved": "Salvo"
    },
    "providers": {
      "title": "Provedor LLM",
      "subtitle": "O Motor precisa de pelo menos um backend LLM para funcionar. Configure um ou mais provedores abaixo.",
      "importFromProviders": "Importar dos seus provedores",
      "imported": "Importado",
      "use": "Usar",
      "saveContinue": "Salvar e Continuar"
    },
    "settings": {
      "fields": {
        "dataDirectory": "Diretório de Dados",
        "logLevel": "Nível de Log",
        "maxHistory": "Máx. Histórico (turnos de conversa)"
      },
      "logLevels": {
        "debug": "DEBUG",
        "info": "INFO",
        "warning": "AVISO",
        "error": "ERRO"
      },
      "sections": {
        "engine": "Motor",
        "backgroundLoops": "Loops em Segundo Plano",
        "memory": "Memória",
        "safety": "Segurança",
        "research": "Pesquisa"
      },
      "backgroundLoops": {
        "synthesis": "Síntese (min)",
        "consolidation": "Consolidação (min)",
        "bm25Rebuild": "Reconstrução BM25 (min)",
        "dripResearch": "Pesquisa Gradual (min)"
      },
      "memory": {
        "embeddingModel": "Modelo de Embedding",
        "maxRetrieval": "Máx. Resultados de Recuperação",
        "denseWeight": "Peso Denso",
        "bm25Weight": "Peso BM25",
        "graphWeight": "Peso do Grafo",
        "recencyBoost": "Impulso de Recência (horas)",
        "randomSurface": "Probabilidade de Superfície Aleatória"
      },
      "safety": {
        "honestySection": "Seção de Honestidade",
        "honestyDesc": "Incluir seção de honestidade no prompt de sistema",
        "userDataDeletion": "Exclusão de Dados do Usuário",
        "userDataDesc": "Permitir que usuários solicitem exclusão de dados"
      },
      "research": {
        "scrapeOnBoot": "Coletar ao Iniciar",
        "scrapeDesc": "Executar coleta de pesquisa ao iniciar o motor",
        "periodicInterval": "Intervalo Periódico (horas)"
      },
      "saveChanges": "Salvar Alterações",
      "saving": "Salvando...",
      "saved": "Salvo"
    },
    "settingsStep": {
      "title": "Configurações do Motor",
      "subtitle": "Configure as configurações gerais do motor. Todos têm valores padrão sensatos — fique à vontade para pular.",
      "completingSetup": "Concluindo Configuração...",
      "completeSetup": "Concluir Configuração"
    },
    "chat": {
      "sendMessage": "Enviar uma mensagem...",
      "sendButton": "Enviar mensagem",
      "typeMessage": "Digite uma mensagem",
      "back": "Voltar",
      "assistantTyping": "Assistant is typing",
      "fallbackName": "Chat"
    },
    "tagInput": {
      "addMore": "Adicionar mais...",
      "typeAndPressEnter": "Digite e pressione Enter"
    },
    "characterCreate": {
      "steps": {
        "identity": {
          "title": "Identidade",
          "aiGenerated": "Gerado por IA",
          "nameLabel": "Nome *",
          "namePlaceholder": "Nome do personagem",
          "eraLabel": "Era",
          "eraPlaceholder": "ex. moderno, Vitoriano",
          "roleLabel": "Função",
          "rolePlaceholder": "ex. Detetive, Cientista",
          "settingLabel": "Cenário",
          "settingPlaceholder": "Descreva onde o personagem vive (primeira pessoa)...",
          "coreIdentityLabel": "Identidade Central",
          "coreIdentityPlaceholder": "Quem é este personagem em sua essência? (primeira pessoa, 3-5 frases)",
          "backstoryLabel": "História",
          "backstoryPlaceholder": "História de vida e eventos-chave (primeira pessoa)..."
        },
        "mode": {
          "title": "Criar Personagem",
          "subtitle": "Gere um personagem com IA ou construa do zero.",
          "aiBoost": "Impulso de IA",
          "aiBoostDesc": "Descreva sua ideia de personagem e a IA gerará uma definição completa.",
          "nameOptional": "Nome (opcional)",
          "namePlaceholder": "ex. Marcus Cole",
          "seedDescription": "Descrição Semente *",
          "seedPlaceholder": "ex. pianista de jazz no Harlem dos anos 1950, filosófico, ama conversas noturnas",
          "eraOptional": "Era (opcional)",
          "eraPlaceholder": "ex. anos 1950, moderno, Vitoriano",
          "generating": "Gerando...",
          "generateCharacter": "Gerar Personagem",
          "or": "ou",
          "startFromScratch": "Começar do Zero"
        },
        "personality": {
          "title": "Personalidade",
          "traits": "Traços de Personalidade",
          "traitsPlaceholder": "ex. espirituoso, compassivo, teimoso",
          "speechPatterns": "Padrões de Fala",
          "formality": "Formalidade",
          "formal": "Formal",
          "casual": "Casual",
          "texting": "Mensagem de texto",
          "verbosity": "Verbosidade",
          "terse": "Conciso",
          "medium": "Médio",
          "verbose": "Verboso",
          "textStyle": "Estilo de Texto",
          "dialect": "Dialeto",
          "dialectPlaceholder": "ex. Sulista Americano, RP Britânico",
          "catchphrases": "Bordões",
          "catchphrasesPlaceholder": "ex. Ora vejam só...",
          "vocabPreferences": "Preferências de Vocabulário",
          "vocabPreferencesPlaceholder": "Palavras que preferem",
          "vocabAvoidances": "Evitações de Vocabulário",
          "vocabAvoidancesPlaceholder": "Palavras que evitam",
          "fillerWords": "Palavras de Preenchimento",
          "fillerWordsPlaceholder": "ex. tipo, sabe, né",
          "exampleQuotes": "Citações de Exemplo",
          "exampleQuotesPlaceholder": "3-5 linhas de diálogo de exemplo"
        },
        "world": {
          "title": "Mundo e Comportamento",
          "knowledgeDomains": "Domínios de Conhecimento",
          "knowledgeDomainsPlaceholder": "ex. história do jazz, teoria musical",
          "knowledgeBoundaries": "Limites de Conhecimento",
          "knowledgeBoundariesPlaceholder": "Assuntos que não conhecem",
          "researchSeeds": "Sementes de Pesquisa",
          "researchSeedsPlaceholder": "Tópicos iniciais para pesquisa de fundo",
          "researchEnabled": "Pesquisa Ativada",
          "researchEnabledDesc": "Permitir coleta de conhecimento em segundo plano",
          "physicalDescription": "Descrição Física",
          "physicalDescPlaceholder": "Aparência física e maneirismos...",
          "physicalHabits": "Hábitos Físicos",
          "physicalHabitsPlaceholder": "ex. tamborila os dedos, ajusta os óculos",
          "idleBehaviors": "Comportamentos Ociosos",
          "idleBehaviorsPlaceholder": "O que fazem quando não estão engajados",
          "timeBehaviors": "Comportamentos por Horário",
          "timePlaceholder": "O que fazem durante {{period}}?",
          "earlyMorning": "Madrugada",
          "morning": "Manhã",
          "afternoon": "Tarde",
          "evening": "Noite",
          "night": "Noite",
          "baselineEmotions": "Emoções Base (Plutchik)",
          "emotionDesc": "Defina a base emocional padrão (0 = nenhum, 1 = máximo)",
          "joy": "Alegria",
          "trust": "Confiança",
          "fear": "Medo",
          "surprise": "Surpresa",
          "sadness": "Tristeza",
          "disgust": "Nojo",
          "anger": "Raiva",
          "anticipation": "Antecipação",
          "engineOverrides": "Substituições do Motor",
          "backend": "Backend",
          "model": "Modelo",
          "temperature": "Temperatura",
          "leaveEmpty": "Deixe vazio para o padrão"
        },
        "review": {
          "title": "Revisão",
          "subtitle": "Revise seu personagem antes de criar.",
          "edit": "Editar",
          "notSet": "Não definido",
          "identitySection": "Identidade",
          "personalitySection": "Personalidade",
          "worldSection": "Mundo e Comportamento",
          "nameLabel": "Nome",
          "eraLabel": "Era",
          "roleLabel": "Função",
          "settingLabel": "Cenário",
          "coreIdentityLabel": "Identidade Central",
          "backstoryLabel": "História",
          "traitsLabel": "Traços",
          "formalityLabel": "Formalidade",
          "verbosityLabel": "Verbosidade",
          "dialectLabel": "Dialeto",
          "catchphrasesLabel": "Bordões",
          "domainsLabel": "Domínios",
          "boundariesLabel": "Limites",
          "researchSeedsLabel": "Sementes de Pesquisa",
          "researchLabel": "Pesquisa",
          "enabled": "Ativado",
          "disabled": "Desativado",
          "physicalLabel": "Físico",
          "habitsLabel": "Hábitos",
          "idleLabel": "Ocioso",
          "timeBehaviorsLabel": "Comportamentos por Horário",
          "emotionsLabel": "Emoções",
          "configured": "Configurado",
          "backendLabel": "Backend",
          "modelLabel": "Modelo",
          "temperatureLabel": "Temperatura",
          "creating": "Criando...",
          "createCharacter": "Criar Personagem"
        }
      }
    }
  },
  "library": {
    "filterTitle": "Filtrar Biblioteca",
    "filters": {
      "all": "Todos",
      "characters": "Personagens",
      "personas": "Personas",
      "lorebooks": "Livros de Lore",
      "images": "Images"
    },
    "emptyStates": {
      "all": {
        "title": "Sua biblioteca está vazia",
        "description": "Crie personagens, personas e livros de lore para vê-los aqui"
      },
      "characters": {
        "title": "Nenhum personagem ainda",
        "description": "Crie seu primeiro personagem para começar a conversar"
      },
      "personas": {
        "title": "Nenhuma persona ainda",
        "description": "Crie uma persona para personalizar sua identidade no chat"
      },
      "lorebooks": {
        "title": "Nenhum livro de lore ainda",
        "description": "Livros de lore são criados nas configurações de um personagem"
      }
    },
    "actions": {
      "startChat": "Iniciar Conversa",
      "editCharacter": "Editar Personagem",
      "editPersona": "Editar Persona",
      "editLorebook": "Editar Livro de Lore",
      "renameLorebook": "Renomear Livro de Lore",
      "exportCharacter": "Exportar Personagem",
      "exportPersona": "Exportar Persona",
      "chatAppearance": "Aparência do Chat",
      "deleteCharacter": "Excluir Personagem",
      "deletePersona": "Excluir Persona",
      "deleteLorebook": "Excluir Livro de Lore",
      "importLorebook": "Importar Livro de Lore"
    },
    "imageLibrary": {
      "filters": {
        "all": "Tudo",
        "backgrounds": "Fundos",
        "avatars": "Avatares",
        "attachments": "Anexos",
        "other": "Outros"
      },
      "searchPlaceholder": "Pesquisar por nome do arquivo, caminho, id da sessão ou id da entidade",
      "empty": {
        "title": "Nenhuma imagem corresponde a esta visualização",
        "description": "Tente outro filtro ou termo de pesquisa. A biblioteca mostra apenas imagens já armazenadas localmente no app."
      },
      "actions": {
        "sort": "Ordenar",
        "useThis": "Usar esta",
        "using": "Usando...",
        "copyPath": "Copiar caminho",
        "saving": "Salvando...",
        "download": "Baixar",
        "delete": "Excluir imagem",
        "deleting": "Excluindo..."
      },
      "active": "Ativo",
      "messages": {
        "loadFailed": "Falha ao carregar a biblioteca de imagens",
        "saved": "Imagem salva",
        "downloadFailed": "Falha no download",
        "useFailed": "Não foi possível usar esta imagem",
        "deleted": "Imagem excluída",
        "deleteFailed": "Falha ao excluir a imagem"
      },
      "deleteConfirm": {
        "title": "Excluir imagem?",
        "message": "Tem certeza de que deseja excluir \"{{filename}}\"? Isso pode quebrar avatares, fundos de chat ou anexos de mensagem que ainda a utilizam."
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
      "title": "Excluir {{itemType}}?",
      "message": "Tem certeza de que deseja excluir",
      "characterWarning": "Isso também excluirá todas as sessões de conversa com este personagem."
    },
    "rename": {
      "title": "Renomear Livro de Lore",
      "placeholder": "Digite o novo nome..."
    },
    "itemTypes": {
      "character": "Personagem",
      "persona": "Persona",
      "lorebook": "Livro de Lore"
    },
    "lorebookLabel": "Livro de Lore",
    "noDescriptionYet": "Sem descrição ainda",
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
    "loading": "Carregando provedores...",
    "stepIndicator": "Passo {{current}} de {{total}}",
    "steps": {
      "provider": "Configuração do Provedor",
      "model": "Configuração do Modelo",
      "memory": "Sistema de Memória",
      "stepNofM": "Step {{current}} of {{total}}"
    },
    "provider": {
      "availableProviders": "Provedores Disponíveis",
      "chooseProvider": "Escolha um provedor",
      "titleMobile": "Choose your AI provider",
      "descMobile": "Select an AI provider to get started. Your API keys are securely encrypted on your device. No account signup needed.",
      "configureProvider": "Configure {{name}}",
      "connectProvider": "Connect {{name}}",
      "connectProviderDesc": "Paste your API key below to enable chats. Need a key? Get one from the provider dashboard.",
      "localLLMs": "Local LLMs",
      "useLocalLLMs": "I want to use Local LLMs",
      "browseModelLibrary": "Browse Model Library",
      "browseModelLibraryDesc": "Search and download GGUF models from HuggingFace",
      "useOwnGguf": "Use my own GGUF files",
      "useOwnGgufDesc": "Select a GGUF model and optional mmproj file from your device",
      "fields": {
        "displayLabel": "Display Label",
        "displayLabelHint": "How this provider will appear in your menus",
        "displayLabelPlaceholder": "My {{name}}",
        "defaultLabelFallback": "Provider",
        "apiKey": "API Key",
        "apiKeyOptional": "API Key (Optional)",
        "apiKeyHint": "Keys are encrypted locally",
        "apiKeyPlaceholderRemote": "sk-...",
        "apiKeyPlaceholderLocal": "Usually not required",
        "whereToFind": "Where to find it",
        "baseUrl": "Base URL",
        "baseUrlPlaceholderHost": "http://192.168.1.10:3333",
        "baseUrlPlaceholderLocal": "http://localhost:11434",
        "baseUrlPlaceholderRemote": "https://api.provider.com",
        "baseUrlPlaceholderIntenseRP": "http://127.0.0.1:7777/v1",
        "baseUrlHintLocal": "Your local server address with port",
        "baseUrlHintHost": "Enter the desktop host URL shown by your host device",
        "baseUrlHintRemote": "Override the default endpoint if needed",
        "chatEndpoint": "Chat Endpoint",
        "systemRole": "System Role",
        "userRole": "User Role",
        "assistantRole": "Assistant Role",
        "supportsStreaming": "Supports Streaming",
        "mergeSameRole": "Merge Same-role Messages",
        "toolChoiceMode": "Tool Choice Mode",
        "toolChoiceHint": "Controls how tool_choice is sent to the custom endpoint."
      },
      "toolChoice": {
        "auto": "Auto",
        "required": "Required",
        "none": "None",
        "omit": "Omit Field",
        "passthrough": "Passthrough (Tool Config)"
      },
      "buttons": {
        "testConnection": "Test Connection",
        "testing": "Testing..."
      },
      "descriptions": {
        "chutes": "OpenAI-compatible inference for top open-source models",
        "openai": "GPT-5, GPT-4.1, and GPT-4o models for expressive RP",
        "lettuceHost": "Connect to your own desktop Lettuce Host over LAN with OpenAI-style API",
        "anthropic": "Claude 4.5 Sonnet & Haiku for deep, emotional dialogue",
        "aggregator": "Access models like GPT-5, Claude 4.5, Grok-3, Mixtral, and more",
        "openaiCompatible": "Use any OpenAI-style API endpoint",
        "mistral": "Mistral Small 3.2, Mixtral 8x22B, and other Mistral models",
        "deepseek": "DeepSeek-V3.2-Exp, DeepSeek-R1, and other high-efficiency models",
        "xai": "Grok-1.5, Grok-3, and newer xAI models",
        "zai": "GLM-4.5, GLM-4.6, and Air variants",
        "moonshot": "Kimi-K2 Thinking and Kimi-K1 models",
        "gemini": "Gemini 2.5 Flash, 2.5 Pro, and more",
        "qwen": "Qwen3-VL and newer Qwen models",
        "nvidia": "Nemotron, Llama, DeepSeek, and more via NVIDIA NIM",
        "custom": "Point LettuceAI to any custom model endpoint",
        "fallback": "AI model provider"
      },
      "descriptionsShort": {
        "chutes": "Open-source model inference",
        "openai": "GPT-5, GPT-4o, GPT-4.1",
        "lettuceHost": "Your own LAN host",
        "anthropic": "Claude 4.5 Sonnet & Haiku",
        "aggregator": "Multi-model aggregator",
        "openaiCompatible": "Custom OpenAI endpoint",
        "mistral": "Mistral & Mixtral models",
        "deepseek": "DeepSeek-V3, R1",
        "xai": "Grok-1.5, Grok-3",
        "zai": "GLM-4.5, GLM-4.6",
        "moonshot": "Kimi-K2 Thinking",
        "gemini": "Gemini 2.5 Flash & Pro",
        "qwen": "Qwen3-VL models",
        "nvidia": "NVIDIA NIM inference",
        "custom": "Custom endpoint",
        "fallback": "AI provider"
      },
      "cardLabel": "{{step}}: {{name}}",
      "errors": {
        "hostUrlRequired": "Host URL is required (e.g., http://192.168.1.10:3333)",
        "baseUrlRequired": "Base URL is required (e.g., http://localhost:11434)",
        "apiKeyTooShort": "API key seems too short",
        "invalidApiKey": "Invalid API key",
        "connectionFailed": "Connection failed",
        "verificationFailed": "Verification failed",
        "failedToSave": "Failed to save provider",
        "connectionSuccessful": "Connection successful!",
        "modelNotFound": "Model not found on provider",
        "modelVerificationFailed": "Model verification failed",
        "failedToSaveModel": "Failed to save model"
      }
    },
    "model": {
      "noProvidersTitle": "Nenhum provedor configurado",
      "noProvidersDesc": "Você precisará conectar um provedor antes de escolher um modelo padrão.",
      "goToProviderSetup": "Ir para configuração do provedor",
      "yourProviders": "Your Providers",
      "yourProvidersHint": "Select which provider to use",
      "setDefaultModel": "Set your default model",
      "setDefaultModelDesc": "Choose which provider and model name LettuceAI should use by default. You'll be able to add more later.",
      "setDefaultModelDescDesktop": "Select a provider from the list to configure your model.",
      "modelDetails": "Model Details",
      "modelDetailsDesc": "Define the API identifier and the label you'll see inside the app.",
      "whichModel": "Which model should I use?",
      "nextMemorySystem": "Next: Memory System",
      "fields": {
        "displayName": "Display Name",
        "displayNamePlaceholder": "Creative mentor",
        "displayNameHint": "How this model appears in menus",
        "modelId": "Model ID",
        "modelPathGguf": "Model Path (GGUF)",
        "modelIdPlaceholder": "e.g. gpt-4o",
        "modelPathPlaceholder": "/path/to/model.gguf",
        "modelIdHint": "Exact identifier used for API calls",
        "showList": "Show List",
        "manualInput": "Manual Input",
        "refreshModelList": "Refresh model list",
        "selectModel": "Select Model",
        "selectAModel": "Select a model...",
        "searchModels": "Search models...",
        "noModelsFound": "No models found matching \"{{query}}\""
      },
      "fillBothFields": "Fill out both fields above to enable the finish button.",
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
      "dynamicTitle": "Memória Dinâmica",
      "recommended": "Recomendado",
      "settingUp": "Configurando...",
      "finishSetup": "Concluir Configuração",
      "promptTitle": "Configurar Memória Dinâmica",
      "oneLastStep": "Mais Um Passo",
      "downloadAndEnable": "Baixar e Ativar",
      "chooseStyle": "Choose your memory style",
      "howRemember": "How should your AI companions remember details about you and your conversations?",
      "dynamicDescription": "Uses a <0>local embedding model</0> to smartly manage context. This cuts token costs while maintaining high quality, even in long chats.",
      "dynamicFeatures": {
        "quality": "Maintains quality in long chats",
        "cost": "Reduces API costs significantly",
        "auto": "Automatic context management",
        "zeroConfig": "Zero configuration needed"
      },
      "manualTitle": "Manual Memory",
      "manualBadge": "Classic experience",
      "manualDescription": "You explicitly pin messages and edit the \"World Info\" or character definitions yourself. Good for total control.",
      "manualFeatures": {
        "control": "Total control over facts",
        "scenarios": "Best for specific scenarios"
      },
      "setupModelMessage": "To use Dynamic Memory, we need to download a small embedding model (~120MB) to your device.",
      "setupBullets": {
        "offline": "Model runs 100% offline on your device",
        "remembering": "Required for remembering context",
        "disable": "You can disable this later in settings"
      },
      "stepLabel": "Step 3 of 3",
      "stepLabelMemory": "Memory System"
    },
    "welcome": {
      "appName": "LettuceAI",
      "tagline": "Seu companheiro pessoal de IA. Privado, seguro e sempre no dispositivo.",
      "features": {
        "onDevice": "Somente no dispositivo",
        "characterReady": "Personagem pronto"
      },
      "betaWarning": {
        "title": "Versão Beta para Desktop",
        "description": "Você está usando a versão desktop. Alguns recursos podem diferir do mobile. Reporte problemas no GitHub."
      },
      "languageSelector": {
        "title": "Idioma",
        "description": "Detectado automaticamente do seu dispositivo. Você pode alterar a qualquer momento nas configurações."
      },
      "getStarted": "Começar",
      "skipForNow": "Pular por enquanto",
      "restoreFromBackup": "Restaurar do Backup",
      "setupTime": "A configuração leva menos de 2 minutos",
      "skipWarning": {
        "title": "Pular configuração?",
        "warningTitle": "Provedor necessário para conversar",
        "warningMessage": "Sem um provedor, você não poderá enviar mensagens. Você pode adicionar um depois nas configurações.",
        "addProvider": "Adicionar Provedor",
        "skipAnyway": "Pular mesmo assim"
      },
      "restoreBackup": {
        "title": "Restaurar Backup",
        "selectMessage": "Selecione um backup para restaurar.",
        "browse": "Procurar Arquivos",
        "processing": "Processando arquivo...",
        "processingNote": "Backups grandes podem levar um minuto",
        "noBackups": "Nenhum backup encontrado",
        "noBackupsHint": "Toque em procurar para selecionar um arquivo .lettuce",
        "browseLettuce": "Procurar arquivo .lettuce",
        "passwordLabel": "Senha do Backup",
        "passwordPlaceholder": "Digite a senha",
        "restoreButton": "Restaurar Backup",
        "restoring": "Restaurando...",
        "infoMessage": "Isso configurará o app com seus dados de backup, incluindo personagens, conversas e configurações.",
        "embeddingTitle": "Modelo de Embedding Necessário",
        "dynamicMemoryDetected": "Memória Dinâmica Detectada",
        "dynamicMemoryMessage": "Este backup contém personagens com memória dinâmica ativada, que requer o modelo de embedding (~120MB).",
        "embeddingOptions": "Você pode baixar o modelo agora para ativar a memória dinâmica, ou continuar sem ele (a memória dinâmica será desativada para os personagens afetados).",
        "downloadModel": "Baixar Modelo",
        "continueWithoutDynamic": "Continuar Sem Memória Dinâmica",
        "embeddingNote": "Você pode reativar a memória dinâmica depois nas configurações do personagem após baixar o modelo.",
        "back": "Voltar",
        "cancel": "Cancel",
        "errors": {
          "passwordRequired": "Password is required",
          "incorrectPassword": "Incorrect password",
          "failedToOpenFile": "Failed to open file",
          "failedToRestore": "Failed to restore backup",
          "failedToUpdateSettings": "Failed to update settings"
        }
      }
    },
    "common": {
      "back": "Voltar",
      "cancel": "Cancel",
      "continue": "Continue",
      "verifying": "Verifying...",
      "skipForNow": "Skip for now",
      "selectAProvider": "Select a provider to configure",
      "clickToSelectProvider": "Click to select a provider",
      "selectProviderFromList": "Select a provider from the list to get started.",
      "enterApiKey": "Enter your API key to enable AI chat functionality."
    },
    "modelGuide": {
      "badge": "Model Guide",
      "title": "How do I choose a model?",
      "intro": "LettuceAI doesn't force a single \"best\" model. Instead, you pick what fits your <0>use case, budget, and vibe</0>. Use this guide to decide what to try and where to look.",
      "askYourself": "Ask yourself:",
      "factors": {
        "quality": {
          "title": "Quality & capabilities",
          "description": "How smart does the model need to be? Bigger, newer models usually reason better, write nicer text, and handle messy prompts more gracefully.",
          "q1": "Do you need deep character consistency and emotional intelligence?",
          "q2": "Do you care about immersive storytelling and believable character personalities?",
          "q3": "Do you want the model to remember character details and stay in-character across long sessions?"
        },
        "speed": {
          "title": "Speed & latency",
          "description": "Faster models feel better for chatty and back-and-forth conversations. Some models trade a bit of quality for a lot more speed.",
          "q1": "Do you want near-instant replies to keep roleplay flowing naturally?",
          "q2": "Are you doing rapid-fire dialogue scenes where waiting would break immersion?",
          "q3": "Is this for casual RP where quick back-and-forth matters more than perfect responses?"
        },
        "budget": {
          "title": "Budget & usage",
          "description": "Every provider bills per token. Even cheap models add up if you chat a lot, so pick something that matches how often and how heavily you use it.",
          "q1": "Are you okay paying more for richer character interactions, or do you want something cheap for daily RP?",
          "q2": "Do you have free models from your provider/router you can try first?",
          "q3": "Will you run long roleplay sessions with detailed scene descriptions?",
          "q4": "Do you have a hard monthly budget you don't want to exceed?"
        },
        "safety": {
          "title": "Safety, privacy & extras",
          "description": "Providers differ in how they handle safety, logging, and extra features like images, tools, or long context windows.",
          "q1": "Do you need fewer content filters for mature or creative roleplay scenarios?",
          "q2": "Do you care if your private RP conversations are logged or used for training?",
          "q3": "Do you need long context windows for complex storylines and character histories?"
        }
      },
      "where": {
        "title": "Where can I find models?",
        "intro": "Most providers and routers have a <0>model list or catalog</0>. Browse those pages to see what they offer, pricing, limits, and special features.",
        "directTitle": "Direct providers",
        "directDesc": "OpenAI, Anthropic, Google Gemini, xAI, Mistral, etc. Each has a console/dashboard where you can see official model names, capabilities, and pricing.",
        "routersTitle": "Routers & hubs",
        "routersDesc": "Services like OpenRouter or other aggregators list many models from different providers in one place, often with benchmarks and pricing comparisons.",
        "communityTitle": "Community recommendations",
        "communityDesc": "Look at docs, blogs, or community posts from your provider/router. They usually highlight which models are best for chat, coding, or speed."
      },
      "rules": {
        "title": "Simple rules of thumb",
        "casual": "For casual chatting: pick a fast, cheap chat model from your provider/router.",
        "experiments": "For experiments or high volume: start with the cheapest model that feels good enough, then upgrade if needed.",
        "switch": "If something feels off (too slow / too dumb / too expensive): you can always switch models later in LettuceAI."
      },
      "disclaimer": "Always check the provider's own documentation for the latest model list, limits, and pricing. This page is about how to think, not what to buy."
    },
    "whereToFind": {
      "badge": "API Key Help",
      "intro": "Follow these steps to get your API key, then return to LettuceAI and paste it into the provider settings.",
      "readyPrompt": "Ready to get the key?",
      "openProviderSite": "Open provider site",
      "keyWarning": "Never share your API key publicly. Anyone with this key can use your account balance.",
      "stuckPrompt": "Still can't figure it out?",
      "joinDiscord": "Join our Discord server for help",
      "guides": {
        "chutes": {
          "title": "How to find your Chutes API key",
          "s1": "Go to chutes.ai/app and sign in.",
          "s2": "Open your account/settings area and find API Keys.",
          "s3": "Create a new key (or copy an existing one).",
          "s4": "Paste the key into LettuceAI."
        },
        "openai": {
          "title": "How to find your OpenAI API key",
          "s1": "Go to platform.openai.com and sign in.",
          "s2": "Click your profile avatar in the top-right, then choose API keys.",
          "s3": "Click Create new secret key and copy the value shown.",
          "s4": "Paste the key into LettuceAI and store it somewhere safe. You will not see it again."
        },
        "anthropic": {
          "title": "How to find your Anthropic API key",
          "s1": "Go to console.anthropic.com and sign in.",
          "s2": "Open Settings from the left sidebar.",
          "s3": "Select API keys and click Create key.",
          "s4": "Copy the key and paste it into LettuceAI."
        },
        "openrouter": {
          "title": "How to find your OpenRouter API key",
          "s1": "Visit openrouter.ai and log in.",
          "s2": "Open the Keys page from your profile menu.",
          "s3": "Click Create key, give it a name, and save.",
          "s4": "Copy the key and paste it into LettuceAI."
        },
        "mistral": {
          "title": "How to find your Mistral API key",
          "s1": "Go to console.mistral.ai and sign in.",
          "s2": "Click API keys in the sidebar.",
          "s3": "Click Create an API key.",
          "s4": "Copy the key and paste it into LettuceAI."
        },
        "deepseek": {
          "title": "How to find your DeepSeek API key",
          "s1": "Open platform.deepseek.com and log in.",
          "s2": "Click API Keys in the top navigation.",
          "s3": "Create a new key if you do not already have one.",
          "s4": "Copy the key and paste it into LettuceAI."
        },
        "groq": {
          "title": "How to find your Groq API key",
          "s1": "Visit console.groq.com and sign in.",
          "s2": "Open API Keys from the sidebar.",
          "s3": "Create a new key, then copy it.",
          "s4": "Paste the key into LettuceAI."
        },
        "gemini": {
          "title": "How to find your Google Gemini API key",
          "s1": "Go to Google AI Studio at aistudio.google.com and sign in.",
          "s2": "Click Get API key or Manage keys.",
          "s3": "Create a new key if needed.",
          "s4": "Copy the key and paste it into LettuceAI."
        },
        "xai": {
          "title": "How to find your xAI API key",
          "s1": "Open console.x.ai and sign in.",
          "s2": "Navigate to the API Keys section in the console.",
          "s3": "Create a new key.",
          "s4": "Copy the key and paste it into LettuceAI."
        },
        "zai": {
          "title": "How to find your zAI (GLM) API key",
          "s1": "Go to open.bigmodel.cn and log in.",
          "s2": "Open User Center, then go to API Keys.",
          "s3": "Create a new key if you do not have one.",
          "s4": "Copy the key and paste it into LettuceAI."
        },
        "moonshot": {
          "title": "How to find your Moonshot (Kimi) API key",
          "s1": "Visit platform.moonshot.cn and sign in.",
          "s2": "Open the API Keys section in the console.",
          "s3": "Create a new key and copy it.",
          "s4": "Paste the key into LettuceAI."
        },
        "qwen": {
          "title": "How to find your Qwen API key",
          "s1": "Open dashscope.aliyun.com and log in.",
          "s2": "Go to the API Keys section in the sidebar.",
          "s3": "Create a new key.",
          "s4": "Copy the key and paste it into LettuceAI."
        },
        "nanogpt": {
          "title": "How to find your NanoGPT API key",
          "s1": "Go to nano-gpt.com and log in.",
          "s2": "Open the dashboard and go to the API keys section.",
          "s3": "Create a new key if needed.",
          "s4": "Copy the key and paste it into LettuceAI."
        },
        "featherless": {
          "title": "How to find your Featherless API key",
          "s1": "Visit featherless.ai and sign in.",
          "s2": "Open your account or API section from the dashboard.",
          "s3": "Create a new key if you do not see one.",
          "s4": "Copy the key and paste it into LettuceAI."
        },
        "anannas": {
          "title": "How to find your Anannas API key",
          "s1": "Go to dashboard.anannas.ai and log in.",
          "s2": "Navigate to the API Keys section.",
          "s3": "Create a new key and copy it.",
          "s4": "Paste the key into LettuceAI."
        },
        "default": {
          "title": "How to find your API key",
          "s1": "Open your AI provider dashboard in a browser and sign in.",
          "s2": "Look for API, Developer, or Integrations settings.",
          "s3": "Create a new API key or view an existing one.",
          "s4": "Copy the key and paste it into LettuceAI."
        }
      }
    },
    "welcomeFooter": {
      "setupTimeMobile": "Setup takes less than 2 minutes"
    }
  },
  "search": {
    "placeholder": "Pesquisar...",
    "tabs": {
      "characters": "Personagens",
      "personas": "Personas"
    },
    "noResults": "Nenhum(a) {{type}} encontrado(a)",
    "emptyState": "Nenhum(a) {{type}} ainda",
    "noResultsHint": "Tente um termo de pesquisa diferente",
    "emptyCharacters": "Crie seu primeiro personagem para começar a conversar",
    "emptyPersonas": "Crie uma persona nas configurações",
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
      "join": "Entrar",
      "joinDesc": "Conectar ao host",
      "host": "Hospedar",
      "hostDesc": "Compartilhar seus dados"
    },
    "sections": {
      "mode": "Modo",
      "connectToHost": "Conectar ao Host",
      "startHosting": "Iniciar Hospedagem",
      "status": "Status",
      "hosting": "Serviço de Hospedagem",
      "localAddress": "Endereço da Rede Local",
      "connectionPin": "PIN de Conexão",
      "setupGuide": "Guia de Configuração"
    },
    "fields": {
      "hostAddress": "Endereço do Host ou JSON",
      "hostPlaceholder": "ex. 192.168.1.100:12345",
      "pinCode": "Código PIN",
      "pinPlaceholder": "000000"
    },
    "buttons": {
      "scanQRCode": "Escanear Código QR",
      "connect": "Conectar",
      "connecting": "Conectando...",
      "startHosting": "Iniciar Hospedagem",
      "startingServer": "Iniciando servidor...",
      "stopHosting": "Parar Hospedagem",
      "hostAgain": "Hospedar Novamente",
      "done": "Concluído"
    },
    "status": {
      "connecting": "Conectando...",
      "connected": "Conectado",
      "waitingConfirmation": "Aguardando confirmação",
      "waitingConfirmationDesc": "Aprove a conexão no dispositivo host para continuar.",
      "syncing": "Sincronizando...",
      "transferringData": "Transferindo dados",
      "syncInProgress": "Sincronização em Andamento",
      "live": "Ao Vivo",
      "broadcasting": "Transmitindo",
      "clientsLabel": "Conectados",
      "clientsUnit": "Clientes"
    },
    "pinDescription": "Compartilhe este PIN com o dispositivo que está conectando",
    "hostingDesc1": "Outros dispositivos podem conectar e sincronizar dados deste dispositivo.",
    "hostingDesc2": "Seus dados serão compartilhados com clientes conectados.",
    "setupSteps": {
      "step1": "Abra o app em outro dispositivo",
      "step2": "Vá para Configurações → Sincronização Local",
      "step3": "Escaneie o código QR ou digite o endereço"
    },
    "messages": {
      "completed": "Sincronização Concluída!",
      "completedDesc": "Todos os dados sincronizados",
      "error": "Erro de Conexão",
      "outdatedClient": "Cliente Desatualizado Detectado"
    },
    "disclaimer": "A sincronização funciona pela sua rede local. Ambos os dispositivos devem estar no mesmo WiFi.",
    "modals": {
      "connectionRequest": "Solicitação de Conexão",
      "requestMessage": "quer sincronizar com este dispositivo.",
      "acceptConnection": "Aceitar Conexão",
      "acceptDesc": "Permitir que este dispositivo sincronize dados",
      "decline": "Recusar",
      "declineDesc": "Bloquear esta tentativa de conexão",
      "readyToSync": "Pronto para Sincronizar",
      "connectionEstablished": "Conexão Estabelecida",
      "deviceReady": "está pronto.",
      "startSyncMessage": "Toque abaixo para começar a sincronizar dados.",
      "startSyncing": "Iniciar Sincronização",
      "startSyncingDesc": "Começar transferência de dados agora"
    },
    "scanner": {
      "title": "Escanear Código QR",
      "cancel": "Cancelar Escaneamento"
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
      "info": "O Assistente de Criação guia você na construção de personagens com assistência de IA. Configure o modelo e as ferramentas usadas durante a criação de personagens.",
      "modelConfiguration": "Configuração do Modelo",
      "chatModel": "Modelo de Chat",
      "selectedModel": "Modelo Selecionado",
      "useAppDefault": "Usar padrão do app{{model}}",
      "useAppDefaultBase": "Usar padrão do app",
      "noModelsAvailable": "Nenhum modelo disponível",
      "chatModelDescription": "Modelo de IA para conversas de criação de personagens",
      "streamingOutput": "Saída em Streaming",
      "streamingDescription": "Mostrar respostas enquanto são geradas",
      "imageGenerationModel": "Modelo de Geração de Imagens",
      "noModelSelected": "Nenhum modelo selecionado",
      "noImageModelsAvailable": "Nenhum modelo de imagem disponível",
      "imageModelDescription": "Para gerar avatares de personagens",
      "toolSelection": "Seleção de Ferramentas",
      "smartToolSelection": "Seleção Inteligente de Ferramentas",
      "smartToolDescription": "A IA escolhe automaticamente quais ferramentas usar",
      "smartToolEnabledHint": "Quando ativado, o Assistente de Criação pergunta o que você quer criar e carrega apenas o conjunto de ferramentas relevante.",
      "smartToolDisabledHint": "Quando desativado, o Assistente de Criação abre diretamente e usa todas as ferramentas habilitadas; o assistente decide o que construir.",
      "quickPresets": "Presets Rápidos",
      "customSelection": "Seleção personalizada - {{count}} ferramentas habilitadas",
      "footerInfo": "Quando a Seleção Inteligente de Ferramentas está ativada, a IA decide quais ferramentas usar com base no contexto. Desative para controlar manualmente quais ferramentas estão disponíveis.",
      "selectChatModel": "Selecionar Modelo de Chat",
      "selectImageModel": "Selecionar Modelo de Imagem",
      "searchModels": "Pesquisar modelos..."
    },
    "categories": {
      "basic": "Básico",
      "content": "Conteúdo",
      "visual": "Visual",
      "settings": "Configurações",
      "flow": "Fluxo",
      "persona": "Personas",
      "lorebook": "Livros de Lore"
    },
    "presets": {
      "all": {
        "name": "Todas as Ferramentas",
        "desc": "Ativar todas as ferramentas disponíveis"
      },
      "essential": {
        "name": "Essencial",
        "desc": "Somente nome, definição e cenas"
      },
      "minimal": {
        "name": "Mínimo",
        "desc": "Apenas nome e definição"
      }
    },
    "tools": {
      "setName": "Definir Nome",
      "setNameDesc": "Definir o nome do personagem",
      "setDefinition": "Definir Definição",
      "setDefinitionDesc": "Definir personalidade e histórico",
      "set_character_name": {
        "name": "Definir Nome",
        "desc": "Definir o nome do personagem"
      },
      "set_character_definition": {
        "name": "Definir Definição",
        "desc": "Definir personalidade e histórico"
      },
      "add_scene": {
        "name": "Adicionar Cena",
        "desc": "Adicionar uma cena inicial para roleplay"
      },
      "update_scene": {
        "name": "Atualizar Cena",
        "desc": "Modificar uma cena existente"
      },
      "toggle_avatar_gradient": {
        "name": "Gradiente do Avatar",
        "desc": "Alternar sobreposição de gradiente no avatar"
      },
      "set_default_model": {
        "name": "Definir Modelo",
        "desc": "Definir o modelo de IA para conversas"
      },
      "set_system_prompt": {
        "name": "Prompt de Sistema",
        "desc": "Definir diretrizes comportamentais"
      },
      "get_system_prompt_list": {
        "name": "Listar Prompts",
        "desc": "Ver prompts disponíveis"
      },
      "get_model_list": {
        "name": "Listar Modelos",
        "desc": "Ver modelos disponíveis"
      },
      "use_uploaded_image_as_avatar": {
        "name": "Imagem como Avatar",
        "desc": "Usar imagem enviada como avatar"
      },
      "use_uploaded_image_as_chat_background": {
        "name": "Imagem como Fundo",
        "desc": "Usar imagem enviada como fundo"
      },
      "generate_image": {
        "name": "Gerar Imagem",
        "desc": "Gerar uma imagem com o modelo de IA"
      },
      "show_preview": {
        "name": "Mostrar Pré-visualização",
        "desc": "Pré-visualizar o personagem"
      },
      "request_confirmation": {
        "name": "Solicitar Confirmação",
        "desc": "Perguntar para salvar ou continuar"
      },
      "list_personas": {
        "name": "Listar Personas",
        "desc": "Navegar pelas personas"
      },
      "upsert_persona": {
        "name": "Salvar Persona",
        "desc": "Criar ou atualizar uma persona"
      },
      "use_uploaded_image_as_persona_avatar": {
        "name": "Avatar da Persona",
        "desc": "Usar imagem enviada como avatar da persona"
      },
      "delete_persona": {
        "name": "Excluir Persona",
        "desc": "Remover uma persona"
      },
      "get_default_persona": {
        "name": "Persona Padrão",
        "desc": "Buscar a persona padrão"
      },
      "list_lorebooks": {
        "name": "Listar Livros de Lore",
        "desc": "Navegar pelos livros de lore"
      },
      "upsert_lorebook": {
        "name": "Salvar Livro de Lore",
        "desc": "Criar ou atualizar um livro de lore"
      },
      "delete_lorebook": {
        "name": "Excluir Livro de Lore",
        "desc": "Remover um livro de lore"
      },
      "list_lorebook_entries": {
        "name": "Listar Entradas",
        "desc": "Ver entradas do livro de lore"
      },
      "get_lorebook_entry": {
        "name": "Obter Entrada",
        "desc": "Buscar uma entrada do livro de lore"
      },
      "upsert_lorebook_entry": {
        "name": "Salvar Entrada",
        "desc": "Criar ou atualizar uma entrada"
      },
      "delete_lorebook_entry": {
        "name": "Excluir Entrada",
        "desc": "Remover uma entrada do livro de lore"
      },
      "create_blank_lorebook_entry": {
        "name": "Entrada em Branco",
        "desc": "Criar uma entrada vazia"
      },
      "reorder_lorebook_entries": {
        "name": "Reordenar Entradas",
        "desc": "Alterar ordenação das entradas"
      },
      "list_character_lorebooks": {
        "name": "Listar Livros de Lore do Personagem",
        "desc": "Ver livros de lore de um personagem"
      },
      "set_character_lorebooks": {
        "name": "Definir Livros de Lore do Personagem",
        "desc": "Atribuir livros de lore a um personagem"
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
    "stepCounter": "Passo {{current}} de {{total}}",
    "skipTour": "Pular tour",
    "next": "Próximo",
    "gotIt": "Entendi",
    "appShell": {
      "chats": {
        "title": "Aqui ficam seus chats",
        "body": "Todas as suas conversas individuais com personagens ficam aqui. Volte a qualquer momento e nós guardaremos seu lugar."
      },
      "groups": {
        "title": "Participe de chats em grupo",
        "body": "Traga vários personagens para a mesma sala e veja-os conversando entre si, ou entre na conversa quando quiser."
      },
      "discover": {
        "title": "Descubra novos personagens",
        "body": "Navegue pelo que a comunidade compartilhou e adicione qualquer personagem que chamar sua atenção. Novos favoritos estão a um toque de distância."
      },
      "library": {
        "title": "Sua biblioteca pessoal",
        "body": "Tudo o que você criou ou salvou fica aqui: personagens, personas, prompts, tudo. Pense nisso como seu acervo."
      },
      "settings": {
        "title": "Personalize do seu jeito",
        "body": "Troque provedores, escolha modelos diferentes, ajuste a aparência do app. Praticamente tudo é configurável nas configurações."
      },
      "search": {
        "title": "Encontre qualquer coisa, rápido",
        "body": "Procurando um chat ou personagem específico? Pesquise em tudo daqui mesmo. Sem precisar cavar."
      },
      "create": {
        "title": "E por fim, crie!",
        "body": "Toque no botão de mais sempre que a inspiração surgir. Crie um novo personagem, persona ou comece algo do zero."
      }
    },
    "chatDetail": {
      "chatTitle": {
        "title": "Configurações por chat",
        "body": "Toque no nome do personagem aqui em cima para abrir configurações só para este chat. Personas, layouts e modelos diferentes por conversa."
      },
      "chatMemory": {
        "title": "O que eles lembram",
        "body": "O ícone de cérebro mostra o que seu personagem lembra das suas conversas. Toque para revisar, editar ou limpar memórias."
      },
      "chatSearch": {
        "title": "Encontre aquela frase",
        "body": "Pesquise apenas nesta conversa. Ótimo para encontrar aquele detalhe de 200 mensagens atrás sem rolar infinitamente."
      },
      "chatLorebook": {
        "title": "Entradas do Lorebook",
        "body": "Fatos extras, construção de mundo e contexto que são injetados no prompt quando palavras-chave específicas aparecem. A cola do seu personagem."
      },
      "chatPlus": {
        "title": "Anexe coisas",
        "body": "Adicione imagens ou abra o menu de extras. O que você anexar será enviado junto com sua próxima mensagem."
      },
      "chatComposer": {
        "title": "Sua mensagem, sua vez",
        "body": "Digite aqui. Enter envia, Shift+Enter adiciona uma nova linha. Dica: pressione e segure qualquer mensagem no chat para editar, ramificar ou excluir."
      },
      "chatSend": {
        "title": "Um botão, quatro funções",
        "body": "O botão de envio muda sua função dependendo do que está acontecendo:"
      }
    },
    "postFirstMessage": {
      "chatRegenerate": {
        "title": "Não gostou? Regenere",
        "body": "Toque no ícone de atualização para obter uma resposta completamente nova do personagem. Cada regeneração é salva como uma variante que você pode revisitar."
      },
      "chatVariants": {
        "title": "Deslize entre variantes",
        "body": "Após regenerar, você verá um contador de variantes abaixo da mensagem. Deslize para a esquerda ou direita na bolha da mensagem para alternar entre todas as respostas diferentes."
      },
      "chatLongPress": {
        "title": "Tem mais escondido aqui",
        "body": "Pressione e segure qualquer mensagem para editar, copiar, ramificar, fixar, excluir ou rebobinar a conversa. Clique com o botão direito também funciona no desktop."
      }
    },
    "sendButton": {
      "continue": {
        "label": "Continuar",
        "desc": "Campo vazio. Tocar aqui incentiva o personagem a continuar falando."
      },
      "send": {
        "label": "Enviar",
        "desc": "Você digitou ou anexou algo. Toque para enviar."
      },
      "sending": {
        "label": "Enviando",
        "desc": "A resposta está a caminho. Botão bloqueado."
      },
      "stop": {
        "label": "Parar",
        "desc": "Toque para cancelar a resposta no meio se mudar de ideia."
      }
    },
    "extra": {
      "rerunOnboarding": "Rerun onboarding"
    }
  },
  "sessionAdvanced": {
    "title": "Parâmetros da Sessão",
    "subtitle": "Sobrescrever padrões do modelo para esta conversa",
    "goBack": "Voltar",
    "support": "Suporte",
    "reset": "Redefinir",
    "save": "Salvar",
    "noSessionWarning": "Abra uma sessão de chat para configurar as configurações por sessão.",
    "overrideDefaults": "Sobrescrever padrões",
    "overrideDefaultsDesc": "Personalizar parâmetros apenas para esta conversa",
    "loadingContextInfo": "Carregando informações de contexto...",
    "sampling": {
      "title": "Amostragem",
      "temperature": "Temperature",
      "temperatureDesc": "Controla a aleatoriedade. Menor = mais determinístico, maior = mais criativo.",
      "temperaturePrecise": "Preciso",
      "temperatureCreative": "Criativo",
      "topP": "Top P",
      "topPDesc": "Amostragem de núcleo. Limita tokens a uma probabilidade cumulativa.",
      "topPFocused": "Focado",
      "topPDiverse": "Diverso",
      "topK": "Top K",
      "topKDesc": "Limita a amostragem aos top K tokens mais prováveis."
    },
    "outputPenalties": {
      "title": "Saída e Penalidades",
      "maxOutputTokens": "Tokens Máximos de Saída",
      "maxOutputTokensDesc": "Comprimento máximo da resposta. Auto deixa o modelo decidir.",
      "auto": "Auto",
      "custom": "Personalizado",
      "frequencyPenalty": "Penalidade de Frequência",
      "frequencyPenaltyDesc": "Reduz repetição de sequências de tokens.",
      "frequencyPenaltyRepeat": "Repetir",
      "frequencyPenaltyVary": "Variar",
      "presencePenalty": "Penalidade de Presença",
      "presencePenaltyDesc": "Incentiva explorar novos tópicos.",
      "presencePenaltyRepeat": "Repetir",
      "presencePenaltyExplore": "Explorar"
    },
    "performance": {
      "title": "Desempenho",
      "gpuLayers": "Camadas GPU",
      "gpuLayersDesc": "Camadas descarregadas na GPU. 0 = apenas CPU.",
      "threads": "Threads",
      "threadsDesc": "Threads de CPU para inferência.",
      "batchThreads": "Threads de Lote",
      "batchThreadsDesc": "Threads de CPU para processamento em lote.",
      "batchSize": "Tamanho do Lote",
      "batchSizeDesc": "Tamanho do bloco de processamento do prompt.",
      "contextLength": "Comprimento do Contexto",
      "contextLengthDesc": "Sobrescrever tamanho da janela de contexto.",
      "flashAttention": "Flash Attention",
      "flashAttentionDesc": "Otimização de memória GPU.",
      "enabled": "Ativado",
      "disabled": "Desativado"
    },
    "samplingMemory": {
      "title": "Amostragem e Memória",
      "minP": "Min P",
      "minPDesc": "Limiar mínimo de probabilidade.",
      "typicalP": "Typical P",
      "typicalPDesc": "Limiar de amostragem típica.",
      "seed": "Seed",
      "seedDesc": "Semente aleatória. Deixe em branco para aleatório.",
      "ropeBase": "RoPE Base",
      "ropeBaseDesc": "Sobrescrita da base de frequência.",
      "ropeScale": "RoPE Scale",
      "ropeScaleDesc": "Sobrescrita da escala de frequência.",
      "kvCacheType": "Tipo de KV Cache",
      "kvCacheTypeDesc": "Quantizar KV Cache para economizar VRAM.",
      "offloadKqv": "Offload KQV",
      "offloadKqvDesc": "KV Cache e operações KQV na GPU.",
      "on": "Ligado",
      "off": "Desligado",
      "samplerProfile": "Perfil do Sampler",
      "samplerProfileDesc": "Padrões ajustados para estabilidade ou raciocínio.",
      "balanced": "Equilibrado",
      "creative": "Criativo",
      "stable": "Estável",
      "reasoning": "Raciocínio"
    },
    "systemInfo": {
      "title": "Informações do Sistema",
      "maxContext": "Contexto Máximo",
      "recommended": "Recomendado",
      "availableRam": "RAM Disponível",
      "availableVram": "VRAM Disponível",
      "modelSize": "Tamanho do Modelo"
    }
  },
  "exportMenu": {
    "title": "Formato de Exportação",
    "selectFormat": "Selecione um formato",
    "uscTitle": "Unified System Card",
    "formats": {
      "uscPrompt": "Exportação USC portátil para templates de prompt.",
      "uscLorebook": "Exportação USC portátil para lorebooks.",
      "uscModel": "Exportação USC portátil para perfis de modelo.",
      "uscChatTemplate": "Exportação USC portátil para templates de chat.",
      "legacyPromptJson": "Prompt JSON Legado",
      "legacyPromptJsonDesc": "Formato atual de pacote de prompt externo.",
      "lorebookJson": "Lorebook JSON",
      "lorebookJsonDesc": "Formato atual de exportação de lorebook.",
      "modelJson": "Modelo JSON",
      "modelJsonDesc": "JSON seguro de perfil de modelo sem credenciais.",
      "chatTemplateJson": "Template de Chat JSON",
      "chatTemplateJsonDesc": "Formato nativo de exportação de template de chat."
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
    "title": "Referências de design",
    "description": "Envie algumas imagens de referência claras e uma descrição visual canônica.",
    "descriptionPlaceholder": "Descreva a aparência estável: rosto, cabelo, porte físico, aparência etária, pistas de roupa, acessórios e direção artística/estilo.",
    "addReferences": "Adicionar referências",
    "visualDescription": "Descrição visual",
    "draftWithAi": "Rascunhar com IA",
    "referenceImages": "Imagens de referência",
    "imageAlt": "Referência de design {{index}}",
    "loading": "Carregando...",
    "removeAria": "Remover referência de design",
    "noImages": "Nenhuma imagem de referência anexada ainda",
    "imageCount": "{{count}} imagem(ns) de referência anexada(s)",
    "emptyReferences": "Adicione algumas fotos de referência claras para fixar rosto, proporções, roupa e estilo.",
    "noWriterModel": "Adicione um modelo de escritor de cenas compatível nas configurações de Geração de Imagem primeiro.",
    "noImagesForGeneration": "Adicione um avatar ou pelo menos uma imagem de referência antes de gerar.",
    "writerModelHelp": "Usa {{model}} para rascunhar a partir do seu avatar e imagens de referência.",
    "noWriterModelHelp": "Adicione um modelo de escritor de cenas compatível nas configurações de Geração de Imagem para rascunhar isso automaticamente.",
    "draftMenuTitle": "Rascunho de Design com IA",
    "draftMenuDesc": "Rascunhado por {{model}} a partir do avatar e imagens de referência atuais.",
    "draftMenuNoWriter": "Adicione um modelo de escritor de cenas compatível antes de usar este assistente.",
    "regenerate": "Regenerar",
    "useThis": "Usar Este"
  },
  "samplerOrder": {
    "title": "Ordem do Sampler",
    "description": "Arraste as etapas para reordenar. Executado de cima para baixo.",
    "reset": "Redefinir",
    "resetAria": "Redefinir ordem do sampler para o padrão",
    "stages": {
      "penalties": {
        "label": "Penalidades",
        "desc": "Aplicar penalidades de frequência e presença antes da filtragem."
      },
      "grammar": {
        "label": "Gramática",
        "desc": "Restringir tokens quando uma gramática nativa de template de chat está ativa."
      },
      "topK": {
        "label": "Top K",
        "desc": "Reduzir o pool de candidatos aos tokens mais fortes."
      },
      "topP": {
        "label": "Top P",
        "desc": "Aplicar filtragem de núcleo à distribuição restante."
      },
      "minP": {
        "label": "Min P",
        "desc": "Descartar tokens de cauda de baixa probabilidade usando um piso mínimo."
      },
      "typical": {
        "label": "Typical P",
        "desc": "Preferir tokens estatisticamente típicos na distribuição atual."
      },
      "temp": {
        "label": "Temperature",
        "desc": "Achatar ou aguçar a distribuição final antes da seleção."
      }
    },
    "presets": {
      "default": {
        "label": "Padrão",
        "hint": "Lettuce local"
      },
      "unsloth": {
        "label": "Unsloth",
        "hint": "estilo llama.cpp"
      },
      "focused": {
        "label": "Focado",
        "hint": "Poda restrita"
      },
      "creative": {
        "label": "Criativo",
        "hint": "Filtro tardio"
      }
    }
  },
  "installedModels": {
    "title": "Inventário local de GGUF",
    "fileCount": "{{count}} arquivos",
    "searchPlaceholder": "Pesquisar nome do modelo, nome do arquivo, caminho, quantização ou arquitetura",
    "loading": "Verificando modelos instalados...",
    "loadFailed": "Falha ao carregar os modelos instalados: {{error}}",
    "empty": {
      "title": "Nenhum modelo GGUF instalado encontrado",
      "description": "Baixe os modelos no navegador primeiro ou coloque os arquivos `.gguf` dentro da pasta de modelos."
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
      "gpuFallbackReason": "Motivo do fallback da GPU",
      "finalError": "Final error",
      "workingRecoveryConfig": "Working recovery config",
      "context": "Context",
      "batch": "Batch",
      "na": "n/a",
      "applyWorkingConfig": "Apply working config",
      "badges": {
        "succeeded": "Run succeeded",
        "cpuFallbackSucceeded": "Fallback para CPU recuperado",
        "cpuFallbackFailed": "Fallback para CPU falhou",
        "failed": "Run failed"
      },
      "headline": {
        "succeeded": "The last local run completed successfully.",
        "cpuFallbackSucceeded": "A carga na GPU falhou e, em seguida, o modelo se recuperou na CPU.",
        "cpuFallbackFailed": "A carga na GPU falhou e o fallback para CPU ainda não concluiu.",
        "failed": "The last local run failed before llama.cpp could complete."
      },
      "detail": {
        "succeeded": "This report also seeds the smart offload cache so future runs can reuse the last stable GPU setup.",
        "cpuFallbackSucceeded": "Armazenamos o contexto e o batch seguros para CPU que realmente funcionaram para você reutilizá-los.",
        "cpuFallbackFailed": "O modelo foi tentado na CPU, mas a configuração recuperada ainda falhou.",
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
      "smartOffloadFallback": "Fallback de descarregamento inteligente",
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
      "customVoices": "Minhas Vozes",
      "providerVoices": "Vozes do Provedor",
      "myVoices": "Minhas Vozes",
      "page": {
        "noAudioProvidersHint": "Adicione um em Provedores > Áudio para começar",
        "noVoicesTitle": "Nenhuma voz criada ainda",
        "noVoicesDescription": "Crie vozes com prompts personalizados para seus personagens",
        "addProviderFirst": "Adicione um provedor de áudio primeiro",
        "noPrompt": "Sem prompt",
        "noProviderVoices": "Nenhuma voz encontrada. Clique em Atualizar para buscar vozes.",
        "showLess": "Mostrar Menos",
        "showAllVoices": "Mostrar Todas as {{count}} Vozes",
        "voiceFallbackTitle": "Voz"
      },
      "cache": {
        "section": "Cache de Áudio",
        "title": "Cache de Áudio TTS",
        "description": "O áudio de voz gerado é armazenado em cache para reduzir regenerações",
        "clearing": "Limpando...",
        "clear": "Limpar Cache"
      },
      "menu": {
        "editDescription": "Modificar esta voz",
        "deleteDescription": "Remover esta voz",
        "provider": "Provedor",
        "category": "Categoria",
        "createVoiceConfig": "Criar Config. de Voz",
        "createVoiceConfigDescription": "Usar esta voz com configurações personalizadas"
      },
      "editor": {
        "editTitle": "Editar Voz",
        "createTitle": "Criar Voz",
        "voiceName": "Nome da Voz",
        "voiceNamePlaceholder": "Minha Voz do Personagem",
        "provider": "Provedor",
        "model": "Modelo",
        "modelIdPlaceholder": "gpt-4o-mini-tts",
        "modelIdHint": "Insira o ID exato do modelo esperado pelo seu endpoint compatível",
        "elevenlabsVoice": "Voz ElevenLabs",
        "noVoicesAvailable": "Nenhuma voz disponível",
        "selectVoice": "Selecione uma voz...",
        "elevenlabsVoiceHint": "Selecione entre as suas vozes do ElevenLabs",
        "geminiVoice": "Voz Gemini",
        "geminiVoiceHint": "Selecione uma voz do Gemini TTS",
        "voiceId": "ID da Voz",
        "voiceIdPlaceholder": "alloy",
        "voiceIdHint": "Insira o ID da voz suportado pelo seu endpoint compatível",
        "voicePrompt": "Prompt de Voz",
        "voicePromptPlaceholder": "Uma voz calorosa e amigável com tom animado...",
        "voicePromptHint": "Descreva como a voz deve soar",
        "exampleText": "Texto de Exemplo",
        "exampleTextPlaceholder": "Olá! É assim que soo quando falo...",
        "exampleTextHint": "Texto de amostra para testar a voz",
        "voiceDesignChars": "{{current}}/{{minimum}} caracteres necessários para a prévia de design de voz",
        "defaultSample": "Olá! É assim que soo quando falo. Consigo ler trechos mais longos com calor, clareza e emoção para que você possa avaliar meu tom e ritmo.",
        "playing": "Reproduzindo...",
        "previewVoice": "Pré-visualizar Voz"
      },
      "kokoro": {
        "title": "Kokoro Studio",
        "newBlend": "Nova mistura",
        "editBlend": "Editar mistura",
        "tryText": "Olá! Este é um teste rápido de como soo.",
        "experimentDefaultText": "Olá! É assim que soo quando falo. Consigo ler trechos mais longos com calor, clareza e emoção.",
        "livePreview": "Pré-visualização ao vivo",
        "savedBlend": "Mistura salva",
        "defaultPreviewText": "Olá! Esta é uma pré-visualização rápida de como esta voz soa.",
        "experiment": "Experimento",
        "providerNotFound": "Provedor Kokoro não encontrado",
        "backToProviders": "Voltar aos provedores",
        "variantUnset": "Variante não definida",
        "ready": "Pronto",
        "modelNotInstalled": "Modelo não instalado",
        "voiceCount": "{{count}} voz",
        "engineActions": "Ações do motor",
        "engineNotInstalled": "Motor não instalado",
        "installAtLeastOneVoice": "Instale pelo menos uma voz",
        "continueSetup": "Continue a configuração para instalar o modelo Kokoro.",
        "pickVoiceOrStarter": "Escolha uma voz ou baixe o pacote inicial para começar.",
        "downloadsFailed": "{{count}} download falhou",
        "retryOrDismissAll": "Tente individualmente ou descarte todos.",
        "dismissAll": "Descartar todos",
        "model": "Modelo",
        "voice": "Voz",
        "downloads": "Downloads",
        "cancelAll": "Cancelar todos",
        "experimentPlaceholder": "Digite uma frase para ouvi-la...",
        "speed": "Velocidade",
        "speak": "Falar",
        "yourBlends": "Suas misturas",
        "noSavedBlends": "Nenhuma mistura salva ainda.",
        "installModelAndVoiceFirst": "Instale o modelo e uma voz primeiro.",
        "featured": "Em destaque",
        "stop": "Parar",
        "sample": "Amostra",
        "voiceLibrary": "Biblioteca de vozes",
        "starterPack": "Pacote inicial",
        "select": "Selecionar",
        "all": "Todos",
        "installed": "Instalado",
        "installModelToBrowse": "Instale o modelo para explorar vozes.",
        "noVoicesInCatalog": "Nenhuma voz no catálogo. Toque em Atualizar.",
        "noVoicesMatch": "Nenhuma voz corresponde aos seus filtros.",
        "collapseAll": "Recolher todos",
        "expandAll": "Expandir todos",
        "selectedCount": "{{count}} selecionado",
        "engineTitle": "Motor Kokoro",
        "variant": "Variante",
        "status": "Status",
        "notInstalled": "Não instalado",
        "file": "Arquivo",
        "modelSize": "Tamanho do modelo",
        "voicesSize": "Tamanho das vozes",
        "total": "Total",
        "assetRoot": "Diretório de assets",
        "reinstallModel": "Reinstalar modelo",
        "installModel": "Instalar modelo",
        "deleteModel": "Excluir modelo",
        "deleteModelDescription": "Libera espaço em disco; as vozes são mantidas.",
        "blend": "Mistura",
        "previewDescription": "Ouvir rapidamente com texto padrão",
        "editBlendDescription": "Ajustar vozes, pesos e velocidade",
        "deleteBlendDescription": "Remover esta voz salva",
        "setupTitle": "Configurar Kokoro",
        "allSet": "Tudo pronto",
        "allSetDescription": "Explore vozes, crie misturas ou teste na área de experimento."
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
      "fallbackFormat": "Formato de fallback"
    }
  },
  "usageAnalytics": {
    "page": {
      "filters": "Filtros",
      "model": "Modelo",
      "character": "Personagem",
      "clearAll": "Limpar Tudo",
      "applyFilters": "Aplicar Filtros",
      "recentActivity": "Atividade Recente",
      "customRange": "Intervalo Personalizado",
      "startDate": "Data de Inicio",
      "endDate": "Data de Fim",
      "applyRange": "Aplicar Intervalo",
      "dashboard": "PAINEL",
      "appTime": "TEMPO NO APP",
      "today": "Hoje",
      "last7Days": "7 Dias",
      "last30Days": "30 Dias",
      "all": "Todos",
      "custom": "Personalizado",
      "filtersCount": "{{count}} Filtros",
      "totalCost": "Custo Total",
      "tokens": "Tokens",
      "avgShort": "{{value}} med",
      "requests": "Solicitacoes",
      "period": "Periodo",
      "last7d": "Ultimos 7d",
      "last30d": "Ultimos 30d",
      "allTime": "Todo o Periodo",
      "recordsCount": "{{count}} registros",
      "usageTrend": "Tendencia de Uso",
      "tokenConsumptionOverTime": "Consumo de tokens ao longo do tempo",
      "input": "Entrada",
      "output": "Saida",
      "byModel": "Por Modelo",
      "byCharacter": "Por Personagem",
      "top": "Top",
      "active": "Ativo",
      "quickView": "Visao Rapida",
      "viewAll": "Ver Tudo",
      "startChatting": "Comece a conversar para ver os dados de uso",
      "exportedTo": "Exportado para: {{path}}",
      "periodTotal": "Total do Periodo",
      "daysActive": "{{count}} dias ativos",
      "dailyAvg": "Media Diaria",
      "selectedPeriod": "periodo selecionado",
      "yesterdayValue": "Ontem {{value}}",
      "thirtyDayAvg": "Media de 30 Dias",
      "appTimeTrend": "Tendencia de Tempo no App",
      "usageDurationPerDay": "Duracao de uso por dia",
      "totalValue": "Total {{value}}",
      "activeTime": "Tempo Ativo"
    },
    "activity": {
      "loading": "Carregando sua atividade...",
      "title": "Atividade Recente",
      "recordsCount": "{{count}} registros de uso",
      "rangeOfTotal": "{{start}}-{{end}} de {{total}}",
      "previous": "Anterior",
      "next": "Proximo",
      "pageOf": "Pagina {{page}} de {{total}}"
    },
    "shared": {
      "relativeTime": {
        "justNow": "agora mesmo",
        "minutesAgo": "ha {{count}}min",
        "hoursAgo": "ha {{count}}h",
        "daysAgo": "ha {{count}}d"
      },
      "operations": {
        "chat": "Chat",
        "regenerate": "Regen",
        "continue": "Continuar",
        "summary": "Resumo",
        "memoryManager": "Memoria",
        "imageGeneration": "Gerar Imagem",
        "aiCreator": "Criador IA",
        "replyHelper": "Assistente de Resposta",
        "groupChatMessage": "Chat em Grupo",
        "groupChatRegenerate": "Regen em Grupo",
        "groupChatContinue": "Continuar em Grupo",
        "groupChatDecisionMaker": "Tomador de Decisao"
      },
      "outputImages": "{{count}} imagens",
      "tokens": "{{count}} tokens",
      "unknown": "Desconhecido",
      "requestDetails": "Detalhes da Solicitacao",
      "noStopReason": "Sem motivo de parada",
      "tokenUsage": "Uso de Tokens",
      "estimatedCost": "Custo Estimado",
      "stats": {
        "prompt": "Prompt",
        "completion": "Conclusao",
        "total": "Total",
        "reasoning": "Raciocinio",
        "image": "Imagem",
        "memory": "Memoria",
        "summary": "Resumo",
        "inputImages": "Imagens de Entrada",
        "outputImages": "Imagens de Saida",
        "cachedPrompt": "Prompt em Cache",
        "cacheWrite": "Gravacao em Cache",
        "webSearches": "Buscas na Web",
        "cacheRead": "Leitura de Cache",
        "requestFee": "Taxa de Solicitacao",
        "webSearch": "Busca na Web",
        "providerTotal": "Total do Provedor"
      }
    }
  }
};
