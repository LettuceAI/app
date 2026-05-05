import type { DeepPartialMessageTree } from "../types";
import { type LocaleMessages } from "./en";
import type { LocaleMetadata } from ".";

export const jaMetadata: LocaleMetadata = {
  name: "Japanese",
  label: "日本語",
};

export const jaMessages: DeepPartialMessageTree<LocaleMessages> = {
  "common": {
    "nav": {
      "chats": "チャット",
      "settings": "設定",
      "providers": "プロバイダー",
      "responseStyle": "応答スタイル",
      "models": "モデル",
      "security": "セキュリティ",
      "accessibility": "アクセシビリティ",
      "reset": "リセット",
      "backupRestore": "バックアップと復元",
      "convertFiles": "ファイル変換",
      "usageAnalytics": "利用分析",
      "changelog": "変更履歴",
      "about": "情報",
      "createSystemPrompt": "システムプロンプト作成",
      "editSystemPrompt": "システムプロンプト編集",
      "systemPrompts": "システムプロンプト",
      "developer": "開発者",
      "advanced": "詳細設定",
      "characters": "キャラクター",
      "lorebooks": "ロアブック",
      "personas": "ペルソナ",
      "dynamicMemory": "ダイナミックメモリ",
      "creationHelper": "作成ヘルパー",
      "helpMeReply": "返信アシスト",
      "editPersona": "ペルソナ編集",
      "newTemplate": "新規テンプレート",
      "editTemplate": "テンプレート編集",
      "chatTemplates": "チャットテンプレート",
      "editCharacter": "キャラクター編集",
      "sync": "同期",
      "newCharacter": "新規キャラクター",
      "engineSetup": "エンジン設定",
      "llmProviders": "LLMプロバイダー",
      "engineSettings": "エンジン設定",
      "lettuceEngine": "Lettuceエンジン",
      "create": "作成",
      "setup": "セットアップ",
      "welcome": "ようこそ",
      "conversation": "会話",
      "library": "ライブラリ",
      "groupChats": "グループチャット",
      "groupChat": "グループチャット",
      "imageGeneration": "画像生成",
      "voices": "音声",
      "chatAppearance": "チャット外観",
      "colorCustomization": "カスタムカラー",
      "embeddingDownload": "埋め込みダウンロード",
      "embeddingTest": "埋め込みテスト",
      "editModel": "モデル編集",
      "messageDebug": "メッセージデバッグ",
      "speechRecognition": "音声認識",
      "hostApi": "APIサーバー",
      "lorebookEntryGenerator": "ロアブックエントリー生成",
      "companionSoulWriter": "コンパニオンソウルライター"
    },
    "bottomNav": {
      "chats": "チャット",
      "groups": "グループ",
      "create": "作成",
      "discover": "探索",
      "library": "ライブラリ"
    },
    "buttons": {
      "cancel": "キャンセル",
      "confirm": "確認",
      "save": "保存",
      "saving": "保存中...",
      "delete": "削除",
      "deleting": "削除中...",
      "create": "作成",
      "creating": "作成中...",
      "edit": "編集",
      "back": "戻る",
      "done": "完了",
      "download": "ダウンロード",
      "later": "後で",
      "proceed": "進む",
      "retry": "再試行",
      "discard": "破棄",
      "import": "インポート",
      "importing": "インポート中...",
      "export": "エクスポート",
      "exporting": "エクスポート中...",
      "update": "更新",
      "generate": "生成",
      "refresh": "更新",
      "continue": "続行",
      "goBack": "戻る",
      "search": "検索",
      "clearSearch": "検索をクリア",
      "add": "追加",
      "remove": "削除",
      "rename": "名前変更",
      "copy": "コピー",
      "copied": "コピーしました！",
      "browseFiles": "ファイルを参照",
      "install": "インストール"
    },
    "labels": {
      "processing": "処理中...",
      "loading": "読み込み中...",
      "noDescriptionYet": "説明がまだありません",
      "untitled": "無題",
      "default": "デフォルト",
      "enabled": "有効",
      "disabled": "無効",
      "on": "オン",
      "off": "オフ",
      "none": "なし",
      "auto": "自動",
      "custom": "カスタム",
      "name": "名前",
      "description": "説明",
      "content": "コンテンツ",
      "preview": "プレビュー",
      "options": "オプション"
    },
    "time": {
      "justNow": "たった今",
      "minutesAgo": "{{minutes}}分前",
      "hoursAgo": "{{hours}}時間前",
      "daysAgo": "{{days}}日前",
      "today": "今日",
      "yesterday": "昨日",
      "last7Days": "過去7日間",
      "older": "それ以前"
    }
  },
  "settings": {
    "sections": {
      "intelligence": "インテリジェンス",
      "experience": "エクスペリエンス",
      "connectivity": "接続",
      "securityPrivacy": "セキュリティとプライバシー",
      "supportInfo": "サポートと情報",
      "dangerZone": "危険ゾーン",
      "developer": "開発者"
    },
    "items": {
      "providers": {
        "title": "プロバイダー",
        "subtitle": "AIサービスに接続"
      },
      "models": {
        "title": "モデル",
        "subtitle": "AIモデルを設定"
      },
      "imageGeneration": {
        "title": "画像生成",
        "subtitle": "イメージの生成とテスト"
      },
      "voices": {
        "title": "音声",
        "subtitle": "テキスト読み上げ音声"
      },
      "accessibility": {
        "title": "アクセシビリティ",
        "subtitle": "サウンドキューと触覚"
      },
      "prompts": {
        "title": "システムプロンプト",
        "subtitle": "AIの性格を形成"
      },
      "security": {
        "title": "セキュリティ",
        "subtitle": "暗号化とプライバシー"
      },
      "backup": {
        "title": "バックアップと復元",
        "subtitle": "データのエクスポートまたはインポート"
      },
      "convert": {
        "title": "ファイル変換",
        "subtitle": "レガシー.jsonエクスポートを.uecに変換"
      },
      "sync": {
        "title": "ローカル同期",
        "subtitle": "デバイス間で同期"
      },
      "usage": {
        "title": "利用分析",
        "subtitle": "コストとトークン統計"
      },
      "advanced": {
        "title": "詳細設定",
        "subtitle": "メモリと機能"
      },
      "logs": {
        "title": "ログ",
        "subtitle": "デバッグと診断"
      },
      "guide": {
        "title": "セットアップガイド",
        "subtitle": "オンボーディングを再実行"
      },
      "docs": {
        "title": "ドキュメント",
        "subtitle": "ガイドとリファレンス"
      },
      "github": {
        "title": "問題を報告",
        "subtitle": "バグとフィードバック • v{{version}}"
      },
      "discord": {
        "title": "Discordに参加",
        "subtitle": "コミュニティとヘルプ"
      },
      "about": {
        "title": "情報",
        "subtitle": "バージョン、更新、リンク"
      },
      "changelog": {
        "title": "変更履歴",
        "subtitle": "新機能"
      },
      "reset": {
        "title": "リセット",
        "subtitle": "すべて消去"
      },
      "developer": {
        "title": "開発者ツール",
        "subtitle": "デバッグとテスト"
      }
    }
  },
  "accessibility": {
    "sectionTitles": {
      "language": "言語",
      "sounds": "サウンドフィードバック",
      "haptics": "触覚フィードバック",
      "appearance": "外観"
    },
    "language": {
      "appLanguage": "アプリの言語",
      "description": "ナビゲーションと設定で使用する言語を選択してください。"
    },
    "appearance": {
      "customColors": "カスタムカラー",
      "customColorsDesc": "アプリのカラースキームをカスタマイズ",
      "chatAppearance": "チャット外観",
      "chatAppearanceDesc": "メッセージバブル、フォント、レイアウトをカスタマイズ"
    },
    "haptics": {
      "vibrateOnChat": "チャット時にバイブレーション",
      "vibrateDesc": "アシスタントが入力中に短いバイブレーションパルス",
      "intensity": "強度",
      "light": "弱",
      "medium": "中",
      "heavy": "強",
      "soft": "ソフト",
      "rigid": "リジッド"
    },
    "sounds": {
      "send": "送信",
      "sendDescription": "メッセージ送信時に再生",
      "success": "成功",
      "successDescription": "アシスタントが正常に完了した時に再生",
      "failure": "失敗",
      "failureDescription": "エラーまたは中断時に再生",
      "testButton": "テスト"
    },
    "feedbackInfo": "フィードバックはメッセージの送受信を認識するのに役立ちます。",
    "hapticsInfo": "触覚フィードバックはモバイルデバイスで利用できます。",
    "extra": {
      "easterEggs": "イースターエッグ",
      "beetrootRain": "ビートルートの雨",
      "beetrootDesc": "チャットで言及されるとビートルートが降ります"
    }
  },
  "topNav": {
    "unsavedChangesTitle": "未保存の変更",
    "unsavedChangesMessage": "離れる前に変更を保存または破棄してください。",
    "goBack": "戻る",
    "changeLayout": "レイアウトを変更",
    "search": "検索",
    "settings": "設定",
    "help": "ヘルプ",
    "add": "追加",
    "openFilters": "フィルターを開く",
    "save": "保存",
    "extra": {
      "installedModels": "インストール済みモデル",
      "refresh": "更新",
      "minimize": "最小化",
      "maximize": "最大化",
      "close": "閉じる"
    }
  },
  "updates": {
    "available": {
      "title": "新しいバージョンがあります",
      "description": "v{{latestVersion}} が利用可能です。現在のバージョンは v{{currentVersion}} です。",
      "actions": {
        "view": "リリースを見る"
      }
    }
  },
  "about": {
    "kicker": "アプリ情報",
    "appName": "LettuceAI",
    "description": "バージョン情報、アップデート確認、便利なリンク。",
    "info": {
      "version": "バージョン",
      "channel": "チャンネル",
      "platform": "プラットフォーム"
    },
    "buildChannel": {
      "dev": "開発ビルド",
      "release": "安定版"
    },
    "update": {
      "sectionTitle": "アップデート",
      "title": "アプリのアップデート",
      "description": "新しいリリースを手動で確認するか、起動時の自動チェックを無効にできます。",
      "autoChecks": "自動アップデート確認",
      "autoChecksDescription": "有効にすると、LettuceAI はアプリ起動時に新しいバージョンを確認します。",
      "checkNow": "アップデートを確認",
      "checking": "アップデートを確認中...",
      "upToDateTitle": "最新の状態です",
      "upToDateDescription": "現在、この端末で利用できる新しいリリースはありません。",
      "failedTitle": "アップデートの確認に失敗しました",
      "failedDescription": "現在アップデートを確認できませんでした。しばらくしてからもう一度お試しください。"
    },
    "links": {
      "sectionTitle": "リンク",
      "website": "ウェブサイト",
      "websiteDescription": "ダウンロードページとリリース情報",
      "github": "GitHub",
      "githubDescription": "ソースコード、Issue、開発情報",
      "discord": "Discord",
      "discordDescription": "コミュニティサーバーとサポート",
      "reddit": "Reddit",
      "redditDescription": "コミュニティの議論、フィードバック、更新情報"
    },
    "developerMode": {
      "enable": "開発者モードを有効にする",
      "enabled": "開発者モードが有効です"
    },
    "errors": {
      "saveTitle": "設定を保存できませんでした",
      "saveDescription": "アップデート確認の設定は変更されませんでした。"
    }
  },
  "components": {
    "tooltip": {
      "dismissHint": "どこかをタップして閉じる"
    },
    "backgroundPosition": {
      "title": "背景の位置調整",
      "instructions": "ドラッグで位置調整 • ピンチまたはスクロールでズーム"
    },
    "avatarSource": {
      "generateImage": "画像を生成",
      "generateImageDesc": "AIによるアバター作成",
      "noImageModels": "画像モデルがありません",
      "editCurrent": "現在のものを編集",
      "editCurrentDesc": "位置調整またはトリミング",
      "chooseImage": "画像を選択",
      "chooseImageDesc": "デバイスから選択"
    },
    "avatarCurrentEdit": {
      "title": "現在の編集",
      "reposition": "位置を変更する",
      "repositionDesc": "現在のアバターを移動またはトリミングする",
      "editWithAI": "AIで編集する",
      "editWithAIDesc": "現在のアバターの AI 編集を開く",
      "noImageModels": "利用可能なイメージモデルがありません"
    },
    "avatarGeneration": {
      "modelsLoadError": "画像生成モデルの読み込みに失敗しました",
      "title": "アバターを生成",
      "help": "アバター生成のヘルプ",
      "model": "モデル",
      "selectModel": "モデルを選択",
      "describe": "アバターを説明してください",
      "describePlaceholder": "カラフルな髪のフレンドリーなアニメの女の子、温かく微笑んでいる...",
      "inProgress": "アバターを生成中...",
      "editingInProgress": "アバター編集を適用しています...",
      "previousVariant": "以前のバリアント",
      "nextVariant": "次のバリエーション",
      "variantCounter": "{{current}} / {{total}}",
      "editRequest": "編集リクエスト",
      "editRequestPlaceholder": "髪を暗くして、メガネをかけて、顔はそのままに…。",
      "applyEdit": "編集を適用",
      "editImageLoadError": "生成されたアバターを編集用に準備できませんでした",
      "aiAssistant": "AIアシスタント",
      "backToResults": "プロンプトに戻る",
      "magicInTheWorks": "魔法が進行中...",
      "refine": "リファイン",
      "apply": "適用する",
      "alt": "生成されたアバター",
      "regenerate": "再生成",
      "useThis": "これを使用"
    },
    "avatarPosition": {
      "title": "アバターの位置調整",
      "instructions": "ドラッグで位置調整 • ピンチまたはスクロールでズーム",
      "alt": "位置調整するアバター"
    },
    "confirmDialog": {
      "defaultTitle": "確認",
      "defaultLabel": "確認"
    },
    "bottomMenu": {
      "defaultTitle": "メニュー",
      "dragTip": "ドラッグしてメニューを閉じる",
      "closeLabel": "メニューを閉じる",
      "buttonProcessing": "処理中..."
    },
    "modelSelector": {
      "placeholder": "モデルを選択",
      "clearLabel": "グローバルデフォルトを使用",
      "loading": "モデルを読み込み中...",
      "noModels": "利用可能なモデルがありません",
      "addProviderFirst": "まず設定でプロバイダーを追加してください",
      "title": "モデルを選択",
      "searchPlaceholder": "モデルを検索...",
      "noResults": "モデルが見つかりません",
      "noResultsHint": "別の検索語をお試しください"
    },
    "localeSelector": {
      "title": "言語の選択"
    },
    "promptTemplate": {
      "nameContentRequired": "名前とコンテンツは必須です",
      "saveError": "テンプレートの保存に失敗しました",
      "editTitle": "プロンプトを編集",
      "createTitle": "プロンプトを作成",
      "name": "名前",
      "namePlaceholder": "例：ロールプレイマスター",
      "content": "コンテンツ",
      "variablesButton": "変数",
      "contentPlaceholder": "あなたは親切なAIアシスタントです...\n\nプロンプトで{{char.name}}と{{scene}}を使用してください。",
      "characterCount": "{{count}}文字",
      "preview": "プレビュー",
      "characterPlaceholder": "キャラクター…",
      "personaPlaceholder": "ペルソナ…",
      "rendering": "レンダリング中…",
      "noPreview": "プレビューはまだありません",
      "saving": "保存中...",
      "update": "更新",
      "create": "作成",
      "variablesTitle": "テンプレート変数",
      "variablesCopyHint": "タップしてクリップボードにコピー",
      "variablesCopied": "コピーしました",
      "variables": {
        "charName": "キャラクター名",
        "charNameDesc": "キャラクターの名前",
        "charDesc": "キャラクター説明",
        "charDescDesc": "キャラクターの説明",
        "scene": "シーン",
        "sceneDesc": "開始シーン/シナリオ",
        "userName": "ユーザー名",
        "userNameDesc": "ユーザーペルソナの名前",
        "userDesc": "ユーザー説明",
        "userDescDesc": "ユーザーペルソナの説明",
        "rules": "ルール",
        "rulesDesc": "キャラクターの行動ルール",
        "contextSummary": "コンテキスト要約",
        "contextSummaryDesc": "動的な会話要約",
        "keyMemories": "キーメモリ",
        "keyMemoriesDesc": "関連する記憶のリスト"
      }
    },
    "characterExport": {
      "title": "エクスポート形式",
      "selectFormat": "形式を選択",
      "loading": "形式を読み込み中...",
      "formatUecDesc": "Unified Entity Card (.uec) 形式（推奨）。",
      "formatLegacyJsonDesc": "レガシーJSON（インポート専用）。",
      "formatV3Desc": "Character Card V3 JSON（最新仕様）。",
      "formatV2Desc": "Character Card V2 JSON（Tavern仕様）。",
      "formatV1Desc": "Character Card V1（インポート専用）。"
    },
    "embeddingDownload": {
      "downloadRequired": "ダウンロードが必要です",
      "modelRequired": "埋め込みモデルが必要です",
      "description": "ダイナミックメモリの動作にはローカル埋め込みモデル（約260MB）のダウンロードが必要です。",
      "localStorage": "• モデルはデバイスにローカル保存されます",
      "downloadSize": "• ダウンロードサイズ：約260MB",
      "summarization": "• 会話要約に必要です"
    },
    "embeddingUpgrade": {
      "title": "埋め込みモデルv3が利用可能",
      "v1Message": "現在512トークンのv1を使用しています。v3にアップグレードしてメモリ品質とロングコンテキストサポートを改善しましょう。",
      "v2Message": "レガシーv2を使用しています。最新の埋め込みモデルでメモリ品質を改善するためにv3にアップグレードしてください。",
      "button": "v3にアップグレード",
      "v3Message": "v4がリリースされ、v3に比べてロールプレイのメモリ想起精度が大幅に向上しました(recall@1 0.02→0.92)。アップグレードを推奨します。"
    },
    "v2UpgradeToast": {
      "title": "メモリモデルv3",
      "badge": "利用可能",
      "message": "v2より改善された埋め込み品質",
      "dismiss": "閉じる",
      "upgrade": "アップグレード"
    },
    "v1UpgradeToast": {
      "title": "メモリモデルv3が利用可能",
      "message": "メモリ品質とロングコンテキストサポートを改善するためにアップグレードしてください。",
      "dismiss": "閉じる",
      "upgrade": "アップグレード"
    },
    "createMenu": {
      "title": "新規作成",
      "smartCreator": "スマートクリエイター",
      "smartCreatorDesc": "アシスタントが作成をガイドします",
      "divider": "または手動で作成",
      "character": "キャラクター",
      "characterDesc": "カスタムキャラクターを作成",
      "persona": "ペルソナ",
      "personaDesc": "再利用可能なボイスを作成",
      "groupChat": "グループチャット",
      "groupChatDesc": "複数のキャラクターとチャット",
      "lorebook": "ロアブック",
      "lorebookDesc": "ワールドリファレンスを構築",
      "characterSmartDesc": "ガイド付きでキャラクターを作成",
      "personaSmartDesc": "再利用可能なボイスや性格を作成",
      "lorebookSmartDesc": "構造化されたワールドリファレンスを構築",
      "loadingConversations": "会話を読み込み中...",
      "createNew": "新規作成",
      "createNewDesc": "新しいガイド付き作成チャットを開始",
      "editExisting": "既存を編集",
      "continueLast": "前回の会話を続ける",
      "seeOlder": "古い会話を見る",
      "seeOlderDesc": "以前のスマートクリエイターの会話を開く",
      "noConversations": "このクリエイターの会話はまだありません。",
      "sessionCompleted": "完了",
      "sessionCancelled": "キャンセル",
      "sessionDraft": "下書き",
      "sessionMessages": "{{count}}件のメッセージ",
      "untitledConversation": "無題の会話",
      "nameLorebookTitle": "ロアブック名",
      "lorebookNamePlaceholder": "ロアブック名を入力...",
      "lorebookImporting": "インポート中...",
      "lorebookImport": "インポート",
      "lorebookCreating": "作成中...",
      "lorebookCreate": "作成",
      "editExistingDesc": "{{goal}}を選んでスマートクリエイターで編集",
      "creatorTitle": "{{goal}}クリエイター",
      "editTitle": "{{goal}}を編集",
      "conversationsTitle": "{{goal}}の会話",
      "loadingItems": "{{items}}を読み込み中...",
      "noItemsFound": "{{items}}が見つかりません。",
      "unnamedCharacter": "名前なしキャラクター",
      "untitledPersona": "無題のペルソナ",
      "untitledLorebook": "無題のロアブック"
    },
    "advancedModelSettings": {
      "temperature": "温度",
      "temperatureDesc": "高い = よりクリエイティブ",
      "topP": "Top P",
      "topPDesc": "低い = より集中的",
      "maxTokens": "最大出力トークン",
      "maxTokensDesc": "デフォルトは空白のまま",
      "contextLength": "コンテキスト長",
      "contextLengthDesc": "ローカルモデルのみ",
      "contextLengthAuto": "自動",
      "frequencyPenalty": "頻度ペナルティ",
      "frequencyPenaltyDesc": "トークンの繰り返しを減少",
      "presencePenalty": "プレゼンスペナルティ",
      "presencePenaltyDesc": "新しいトピックを促進",
      "topK": "Top K",
      "topKDesc": "トークンプールサイズを制限",
      "reasoning": "思考 / 推論",
      "reasoningAutoDesc": "このモデルは常に推論を使用します。設定は不要です。",
      "reasoningEnableDesc": "複雑な問題解決と推論タスクのための高度な思考能力を有効にします。",
      "effortMode": "エフォートモード",
      "budgetMode": "バジェットモード",
      "reasoningEffort": "推論エフォート",
      "reasoningEffortDesc": "思考の深さを制御",
      "reasoningEffortAuto": "自動",
      "reasoningEffortLow": "低",
      "reasoningEffortMed": "中",
      "reasoningEffortHigh": "高",
      "reasoningBudget": "推論バジェット（トークン）",
      "reasoningBudgetDesc": "思考に予約される最大トークン数",
      "reasoningEffortLowDesc": "最小限の推論で素早い応答",
      "reasoningEffortMediumDesc": "バランスの取れた推論の深さ",
      "reasoningEffortHighDesc": "複雑な問題に対する最大推論の深さ",
      "reasoningBudgetExtendedDesc": "思考に予約される最大トークン数。出力制限に加算されます。"
    },
    "providerParameterSupport": {
      "unknownProvider": "不明なプロバイダー：{{providerId}}",
      "reasoningNotSupportedEffort": "未サポート - このプロバイダーはエフォートレベルを使用しません",
      "reasoningNotSupportedBudgetOnly": "未サポート - このプロバイダーはバジェットのみのアプローチを使用します",
      "reasoningNotSupported": "未サポート - このプロバイダーは推論をサポートしていません",
      "unsupportedParametersIgnored": "サポートされていないパラメータは{{providerName}}によって無視されます。",
      "reasoningEffortSupported": "推論エフォートは思考モデル（o1、DeepSeek-R1など）でサポートされています",
      "reasoningBudgetSupported": "このプロバイダーはバジェットベースの思考を使用します（エフォートレベルなし）。代わりに推論バジェットトークンを設定してください。",
      "reasoningNotSupportedProvider": "このプロバイダーは推論パラメータをサポートしていません。",
      "matrixTitle": "プロバイダーパラメータサポートマトリックス",
      "providerColumn": "プロバイダー",
      "supportedIndicator": "プロバイダーAPIでサポート",
      "notSupportedIndicator": "未サポート（パラメータは無視されます）"
    },
    "v3UpgradeToast": {
      "title": "メモリモデル v4",
      "badge": "利用可能",
      "message": "v4はv3に比べてロールプレイのメモリ想起精度を大幅に向上させます(recall@1 0.02→0.92)。アップグレードを推奨します。",
      "dismiss": "後で",
      "upgrade": "アップグレード"
    },
    "extra": {
      "promptCachingTitle": "プロンプトキャッシュ",
      "promptCachingDescription": "長く繰り返しの多いコンテキスト(大きなシステムプロンプトや長いチャット履歴など)で生成を高速化し、コストを削減します。",
      "avatarAlt": "アバター",
      "chooseFromLibrary": "ライブラリから選択",
      "chooseFromLibraryDesc": "アプリにすでに保存されている画像を使用します",
      "generateFailed": "画像の生成に失敗しました",
      "editFailed": "アバターの編集に失敗しました",
      "backgroundAlt": "配置する背景",
      "formatsLoadFailed": "エクスポート形式の読み込みに失敗しました",
      "formatsShowingDefaults": "(デフォルトを表示)",
      "timeJustNow": "たった今",
      "timeMinutesAgo": "{{minutes}}分前",
      "timeHoursAgo": "{{hours}}時間前",
      "timeDaysAgo": "{{days}}日前",
      "removeReference": "デザインリファレンスを削除",
      "thumbLoading": "読み込み中...",
      "addReferences": "リファレンスを追加",
      "visualDescription": "ビジュアル説明",
      "draftWithAi": "AIで下書き",
      "regenerate": "再生成",
      "useThis": "これを使う",
      "referenceImagesLabel": "リファレンス画像",
      "writerHelpFallback": "対応するシーンライターモデル",
      "writerHelpUses": "{{model}}を使用してアバターとリファレンス画像から下書きを作成します。",
      "writerHelpUnavailable": "自動で下書きするには、画像生成設定で対応するシーンライターモデルを追加してください。",
      "writerNotAvailableError": "まず画像生成設定で対応するシーンライターモデルを追加してください。",
      "writerNoSourcesError": "生成する前にアバターまたは少なくとも1枚のリファレンス画像を追加してください。",
      "noUsableReferences": "使用可能なリファレンス画像が見つかりませんでした。",
      "draftFailed": "デザイン説明の生成に失敗しました。",
      "draftReadFailed": "画像アセットの読み込みに失敗しました({{status}})",
      "draftConvertFailed": "画像アセットをデータURLに変換できませんでした",
      "draftGenerationFailed": "下書きの生成に失敗しました。",
      "draftMenuTitle": "AIデザイン下書き",
      "draftedBy": "現在のアバターとリファレンス画像から{{model}}が下書きしました。",
      "draftedByFallback": "シーンライターモデル",
      "noWriterUseHelper": "このヘルパーを使用する前に、対応するシーンライターモデルを追加してください。",
      "emptyReferences": "顔、プロポーション、衣装、スタイルを固定するために、明確なリファレンスショットをいくつか追加してください。",
      "designReferencesTitle": "デザインリファレンス",
      "designReferencesDescription": "明確なリファレンス画像を数枚と、標準となるビジュアル説明を1つアップロードしてください。",
      "designReferencesPlaceholder": "顔、髪、体格、年齢の見え方、衣装の特徴、アクセサリー、アート/スタイルの方向性など、安定した見た目を記述します。",
      "dismissAria": "閉じる",
      "v3MessageFallback": "lettuce-emb-v4がリリースされ、ロールプレイのメモリ想起精度が大幅に向上しました。アップグレードを推奨します。",
      "uploadButton": "アップロード",
      "libraryButton": "ライブラリ",
      "companionSetupTitle": "コンパニオンの設定が必要です",
      "companionSetupSubtitleSingle": "コンパニオンモードを実行するには、もう1つモデルが必要です。スキップするとこのキャラクターはロールプレイに戻ります。",
      "companionSetupSubtitleMany": "コンパニオンモードを実行するには、あと{{count}}個のモデルが必要です。スキップするとこのキャラクターはロールプレイに戻ります。",
      "companionSetupBody": "コンパニオンモードでは、感情の解析、エンティティ抽出、メモリのルーティング、過去のコンテキスト想起のためにいくつかのローカルモデルが必要です。",
      "companionUseRoleplay": "代わりにロールプレイを使用",
      "companionDownloadNow": "今すぐダウンロード",
      "searchModelsPlaceholder": "モデルを検索...",
      "loadingModelsDefault": "モデルを読み込み中...",
      "noModelsAvailable": "利用可能なモデルがありません。",
      "noModelsMatching": "「{{query}}」に一致するモデルが見つかりません。",
      "contentPlaceholderText": "あなたは役立つAIアシスタントです...\n\nプロンプト内で{{char.name}}と{{scene}}を使用してください。",
      "previewRenderFailed": "<プレビューの表示に失敗しました>",
      "charactersCount": "{{count}}文字"
    }
  },
  "chats": {
    "characterNotFound": "キャラクターが見つかりません",
    "chatHistory": "チャット履歴",
    "previousConversationsWithCharacter": "{{name}}との過去の会話",
    "previousConversations": "過去の会話",
    "searchChats": "チャットを検索...",
    "searchResults": "{{count}}件の結果",
    "noConversationsYet": "会話はまだありません",
    "startChattingPrompt": "チャットを始めると履歴がここに表示されます",
    "noMatchingChats": "一致するチャットがありません",
    "tryDifferentSearchTerm": "別の検索語をお試しください",
    "untitledChat": "無題のチャット",
    "chatTitlePlaceholder": "チャットタイトル...",
    "exportChatPackage": "チャットパッケージをエクスポート",
    "characterSpecificExport": "キャラクター固有のエクスポート",
    "characterSpecificExportDesc": "デフォルトでこのパッケージをチャットキャラクターに紐付けます。",
    "nonCharacterSpecificExport": "キャラクター非固有のエクスポート",
    "nonCharacterSpecificExportDesc": "インポート時にキャラクター選択が必要です。",
    "deleteChat": "チャットを削除しますか？",
    "deleteConfirmDesc": "履歴から永久に削除されます",
    "keepThisChat": "このチャットを残す",
    "editCharacter": "キャラクターを編集",
    "exportCharacter": "キャラクターをエクスポート",
    "chatAppearance": "チャット外観",
    "importChatPackage": "チャットパッケージをインポート",
    "hideThisCharacter": "このキャラクターを非表示",
    "deleteCharacter": "キャラクターを削除",
    "deleteCharacterTitle": "キャラクターを削除しますか？",
    "deleteCharacterConfirmation": "「{{name}}」を削除してもよろしいですか？このキャラクターとのすべてのチャットセッションも削除されます。",
    "characterSpecificMatches": "このパッケージはキャラクター固有で、選択されたキャラクターと一致します。",
    "characterSpecificMismatch": "このパッケージはキャラクター固有で、別のキャラクターを指しています。{{name}}にインポートされます。",
    "nonCharacterSpecificImport": "このパッケージはキャラクター非固有です。{{name}}にインポートされます。",
    "noCharactersYet": "キャラクターがまだいません",
    "createFirstCharacter": "下の+ボタンから最初のキャラクターを作成してチャットを始めましょう",
    "scrollToBottom": "一番下にスクロール",
    "selectCharacter": "キャラクターを選択",
    "branchToGroupChat": "グループチャットに分岐",
    "addContent": "コンテンツを追加",
    "swapPlaces": "入れ替え",
    "swapPlacesOn": "入れ替え（オン）",
    "uploadImage": "画像をアップロード",
    "helpMeReply": "返信アシスト",
    "sceneImage": {
      "modeTitle": "シーンイメージ",
      "modeDescription": "シーンのプロンプトを自分で作成するか、AI に最初に下書きさせるかを選択します。",
      "writePrompt": "プロンプトの書き込み",
      "writePromptDesc": "使用するシーン イメージ プロンプトを正確に入力します。",
      "askAi": "AIに聞く",
      "askAiDesc": "現在のチャット モデルに、選択した瞬間からのシーン プロンプトをドラフトさせます。",
      "generateTitle": "シーン画像の生成",
      "regenerateTitle": "シーン画像を再生成",
      "aiTitle": "AIシーンプロンプト",
      "promptLabel": "シーンプロンプト",
      "promptPlaceholder": "シーン、登場人物、雰囲気、照明、カメラのフレーミング、重要な詳細を説明します...",
      "suggestedPrompt": "推奨されるプロンプト",
      "regeneratePrompt": "再生する",
      "editPrompt": "プロンプトの編集",
      "reviewTitle": "シーンプロンプトを確認",
      "denyPrompt": "拒否",
      "acceptPrompt": "承認",
      "generateImage": "画像の生成",
      "updateImage": "画像を更新"
    },
    "useMyTextAsBase": "自分のテキストをベースにする",
    "writeNewReply": "新しい返信を書く",
    "suggestedReply": "提案された返信",
    "selectTwoCharactersMinimum": "グループチャットには最低2つのキャラクターを選択してください。",
    "groupBranchCreated": "グループ分岐を作成しました！リダイレクト中...",
    "memories": "メモリ",
    "tools": "ツール",
    "pinned": "ピン留め",
    "searchMemories": "メモリを検索...",
    "addMemory": "メモリを追加",
    "memoryActions": "メモリアクション",
    "pinnedMessages": "ピン留めメッセージ",
    "pinnedMessagesDesc": "常にコンテキストに含まれます",
    "contextSummary": "コンテキスト要約",
    "contextSummaryPlaceholder": "メッセージ間でコンテキストの一貫性を保つための短い要約...",
    "memoryContentPlaceholder": "何を記憶すべきですか？",
    "editMemoryPlaceholder": "メモリ内容を入力...",
    "togglePin": {
      "pin": "ピン留め",
      "unpin": "ピン留め解除"
    },
    "toggleMemoryState": {
      "setHot": "ホットに設定",
      "setCold": "コールドに設定"
    },
    "selectModelForRetry": "再試行用モデルを選択",
    "memoryCategories": {
      "characterTrait": "キャラクター特性",
      "relationship": "関係",
      "plotEvent": "プロットイベント",
      "worldDetail": "ワールド詳細",
      "preference": "好み",
      "other": "その他"
    },
    "settings": {
      "memorySection": "メモリ",
      "memorySectionDesc": "要約、タグ、ツール呼び出し履歴",
      "quickSettings": "クイック設定",
      "quickSettingsDesc": "最もよく使う調整",
      "persona": "ペルソナ",
      "model": "モデル",
      "fallbackModel": "フォールバックモデル",
      "voice": "音声",
      "voiceDesc": "テキスト読み上げ再生",
      "advanced": "詳細",
      "advancedDesc": "このセッションのモデルパラメータを上書き",
      "session": "セッション",
      "sessionDesc": "新しいチャットを開始して履歴を閲覧",
      "newChat": "新しいチャット",
      "newChatDesc": "新しい会話を開始",
      "chatHistoryDesc": "以前のセッションを表示",
      "importChatPackageDesc": "このキャラクターに.chatpkgをインポート",
      "selectModel": "モデルを選択",
      "selectFallbackModel": "フォールバックモデルを選択",
      "personaActions": "ペルソナアクション",
      "sessionAdvancedSettings": "セッション詳細設定",
      "parameterSupport": "パラメータサポート",
      "backToChat": "チャットに戻る",
      "chatSettingsTitle": "チャット設定",
      "chatSettingsSubtitle": "会話の設定を管理します",
      "modelDefaults": "モデルのデフォルト",
      "appDefaults": "アプリのデフォルト",
      "openChatSessionFirst": "まずチャットセッションを開いてください",
      "sessionRequired": "セッションが必要です",
      "noPersona": "ペルソナなし",
      "customPersona": "カスタムペルソナ",
      "noModelAvailable": "利用可能なモデルがありません",
      "fallbackNone": "なし",
      "unknownModel": "不明なモデル",
      "authorNote": "作者ノート",
      "identityProfileAuthored": "アイデンティティプロファイル作成済み",
      "addIdentityProfile": "コンパニオンのアイデンティティプロファイルを追加",
      "soulLabel": "ソウル",
      "sessionTitle": "セッション: {{title}}",
      "sessionUntitled": "無題",
      "messageCount": "{{count}}件のメッセージ",
      "usingCharacterDefault": "キャラクターのデフォルトを使用中",
      "sessionOverrideActive": "セッションのオーバーライドが有効",
      "autoplayVoice": "音声を自動再生",
      "useCharacterDefault": "キャラクターのデフォルトを使用"
    },
    "search": {
      "placeholder": "会話を検索...",
      "noMessagesFound": "メッセージが見つかりません",
      "you": "あなた",
      "character": "キャラクター",
      "failed": "メッセージの検索に失敗しました"
    },
    "templates": {
      "startWithTemplate": "テンプレートで始めますか？",
      "noTemplate": "テンプレートなし",
      "startWithSceneOnly": "シーンのみで開始",
      "you": "あなた",
      "bot": "ボット",
      "messageCount": "{{count}}件のメッセージ"
    },
    "header": {
      "back": "戻る",
      "openSettings": "チャット設定を開く",
      "manageMemories": "メモリを管理",
      "searchMessages": "メッセージを検索",
      "manageLorebooks": "ロアブックを管理",
      "conversationSettings": "会話設定"
    },
    "footer": {
      "sendMessagePlaceholder": "メッセージを送信...",
      "stopGeneration": "生成を停止",
      "sendMessage": "メッセージを送信",
      "continueConversation": "会話を続ける",
      "moreOptions": "その他のオプション",
      "addImage": "画像を追加",
      "addImageAttachment": "画像添付を追加",
      "removeAttachment": "添付を削除",
      "recordVoice": "音声を録音"
    },
    "message": {
      "thinkingMessages": {
        "thinkingReallyHard": "一生懸命考え中…",
        "lettuceCouncil": "レタス評議会に相談中…",
        "stealingThoughts": "虚空から思考を盗み中…",
        "warmingBrainCells": "脳細胞をウォームアップ中…",
        "forbiddenKnowledge": "禁断の知識をロード中…",
        "overthinking": "考えすぎ中（いつものこと）…",
        "pretendingToBeSmart": "賢いふり中…",
        "crunchingNumbers": "架空の数字を計算中…",
        "arguingWithMyself": "自分自身と議論中…",
        "askingUniverse": "宇宙にお願い中…"
      },
      "thoughtProcess": "思考プロセス",
      "regenerateResponse": "応答を再生成",
      "cancelAudioGeneration": "音声生成をキャンセル",
      "stopAudio": "音声を停止",
      "playMessageAudio": "メッセージ音声を再生",
      "playAudio": "音声を再生",
      "sceneLabel": "シーン",
      "variantLabel": "バリアント",
      "regenerating": "再生成中",
      "assistantIsTyping": "アシスタントが入力中",
      "attachedImage": "添付画像",
      "guidedRegenerationTitle": "ガイド付き再生成",
      "guidedRegenerationLabel": "どのように変更しますか?",
      "guidedRegenerationDescription": "トーン、長さ、残す/削除する詳細、次の返信に変えてほしいことを記述してください。",
      "guidedRegenerationPlaceholder": "もっと短く、温かく、より直接的に...",
      "guidedRegenerationSubmit": "再生成"
    },
    "actions": {
      "assistantMessage": "アシスタントメッセージ",
      "userMessage": "ユーザーメッセージ",
      "promptTokens": "プロンプトトークン",
      "completionTokens": "完了トークン",
      "fallbackModelUsed": "フォールバックモデルが使用されました",
      "total": "合計",
      "timeToFirstToken": "最初のトークンまでの時間",
      "completionTokenSpeed": "完了トークン速度",
      "edit": "編集",
      "copy": "コピー",
      "pin": "ピン留め",
      "unpin": "ピン留め解除",
      "rewindToHere": "ここまで巻き戻す",
      "branchFromHere": "ここから分岐",
      "branchToGroupChat": "グループチャットに分岐",
      "branchToCharacter": "キャラクターに分岐",
      "generateSceneImage": "シーン画像の生成",
      "regenerateSceneImage": "シーン画像を再生成する",
      "chatAppearance": "チャット外観",
      "delete": "削除",
      "debug": "デバッグ",
      "unpinToDelete": "削除するにはピン留め解除",
      "editPlaceholder": "メッセージを編集...",
      "memoriesUsed": "{{count}}件のメモリ使用",
      "lorebookUsage": "ロアブック使用状況",
      "lorebookUsageDesc": "この応答は以下のロアブックエントリを使用しました。",
      "matchScore": "一致：{{score}}%",
      "unknownModel": "不明なモデル",
      "loadingModel": "モデルを読み込み中..."
    },
    "emptyState": {
      "goBack": "戻る"
    },
    "settingsDrawer": {
      "title": "チャット設定",
      "subtitle": "会話の設定を管理"
    },
    "history": {
      "archivedBadge": "アーカイブ済み",
      "messagesCount": "{{count}}件のメッセージ",
      "previousGroupPage": "前の{{label}}ページ",
      "nextGroupPage": "次の{{label}}ページ",
      "sillyTavernFormat": "SillyTavern形式",
      "sillyTavernFormatDesc": "SillyTavern互換の.jsonlとしてエクスポートします。",
      "failedLoad": "データの読み込みに失敗しました",
      "failedDelete": "削除に失敗しました: {{error}}",
      "failedRename": "名前変更に失敗しました: {{error}}",
      "chatPackageExportedTo": "チャットパッケージのエクスポート先:\n{{path}}",
      "sillyTavernExportedTo": "SillyTavernチャットのエクスポート先:\n{{path}}",
      "failedExportChatPackage": "チャットパッケージのエクスポートに失敗しました",
      "failedExportSillyTavern": "SillyTavernチャットのエクスポートに失敗しました"
    },
    "authorNote": {
      "title": "作者ノート",
      "inlineEditor": "インラインエディター",
      "inlineEditorDesc": "素早く編集できるよう、チャット入力欄の上に作者ノートを表示します。",
      "toggleInlineEditor": "インライン作者ノートエディターを切り替え",
      "placeholder": "このチャット用のプライベートな指示",
      "willBeRemoved": "保存時に作者ノートが削除されます",
      "noNoteSaved": "保存された作者ノートはありません",
      "charactersCount": "{{count}}文字",
      "clear": "クリア",
      "save": "保存",
      "saving": "保存中...",
      "failedSave": "作者ノートの保存に失敗しました",
      "addPlaceholder": "作者ノートを追加...",
      "savingShort": "保存中..."
    },
    "drawer": {
      "chatSettingsTitle": "チャット設定",
      "chatSettingsSubtitle": "会話の設定を管理します"
    },
    "companionSoul": {
      "loading": "コンパニオンソウルを読み込み中...",
      "unavailable": "コンパニオンソウルは利用できません",
      "unavailableDesc": "ソウル編集はコンパニオンモードのキャラクターでのみ利用できます。",
      "pageTitle": "コンパニオンソウル",
      "back": "戻る",
      "save": "保存"
    },
    "companionRelationship": {
      "back": "戻る",
      "loading": "リレーションシップ状態を読み込み中...",
      "unavailableTitle": "リレーションシップ状態は利用できません",
      "sessionLoadFailed": "チャットセッションを読み込めませんでした。",
      "backToChat": "チャットに戻る",
      "notCompanionTitle": "このチャットはコンパニオンモードではありません",
      "notCompanionDesc": "コンパニオンリレーションシップページは、キャラクターモードがコンパニオンのチャットでのみ表示されます。",
      "openRegularMemories": "通常のメモリを開く",
      "pageTitle": "リレーションシップ状態",
      "memoryButton": "メモリ",
      "lastInteraction": "最後のやり取り {{time}}",
      "bond": "絆",
      "closeness": "親密さ",
      "trust": "信頼",
      "affection": "愛情",
      "tension": "緊張",
      "stability": "安定性",
      "interactions": "やり取り",
      "vsDefaults": "キャラクターのデフォルトとの比較",
      "updatedAt": "更新 {{time}}",
      "emotionalEngine": "感情エンジン",
      "felt": "感じている",
      "feltDesc": "内面の感情",
      "expressed": "表現された",
      "expressedDesc": "返信に表れる",
      "blocked": "ブロック済み",
      "blockedDesc": "ペルソナにより抑制",
      "momentum": "モメンタム",
      "momentumDesc": "最近のやり取りでの傾向",
      "activeDrivers": "アクティブな駆動要素",
      "soul": "ソウル",
      "essence": "本質",
      "voice": "声",
      "relationalStyle": "関係スタイル",
      "vulnerabilities": "弱さ",
      "habits": "習慣",
      "boundaries": "境界",
      "eventsCount": "{{count}}件のイベント",
      "recentTimeline": "最近のタイムライン",
      "superseded": "上書き済み",
      "promptScore": "プロンプト {{score}}",
      "persistenceScore": "持続性 {{score}}",
      "noTimeline": "まだタイムラインはありません",
      "noTimelineDesc": "コンパニオンが会話から学習するにつれて、リレーションシップ、マイルストーン、感情スナップショットのメモリがここに表示されます。",
      "notAuthoredYet": "まだ作成されていません。",
      "noSignal": "シグナルなし。"
    },
    "companionUi": {
      "relationship": "リレーションシップ",
      "milestones": "マイルストーン",
      "boundaries": "境界",
      "preferences": "好み",
      "profile": "プロフィール",
      "routines": "ルーティン",
      "episodes": "エピソード",
      "emotionalSnapshots": "感情スナップショット",
      "unknownTime": "不明",
      "justNow": "たった今",
      "minutesAgo": "{{minutes}}分前",
      "hoursAgo": "{{hours}}時間前",
      "daysAgo": "{{days}}日前",
      "notSetYet": "未設定",
      "missingCharacterId": "characterIdがありません",
      "sessionNotFound": "セッションが見つかりません",
      "failedLoadCompanion": "コンパニオンセッションの読み込みに失敗しました"
    },
    "chatPage": {
      "characterNotFound": "キャラクターが見つかりません",
      "characterDoesntExist": "お探しのキャラクターは存在しません。"
    },
    "debugPage": {
      "copy": "コピー"
    },
    "companionMemoryPage": {
      "backLabel": "戻る",
      "refineMemoryPlaceholder": "保存されたコンパニオンメモリを調整",
      "superseded": "上書き済み",
      "pinned": "ピン留め",
      "cold": "コールド"
    }
  },
  "groupChats": {
    "list": {
      "editGroup": "グループを編集",
      "deleteGroup": "グループを削除",
      "deleteConfirmTitle": "グループチャットを削除しますか？",
      "deleteConfirmMessage": "「{{name}}」を削除してもよろしいですか？このグループチャットのすべてのメッセージも削除されます。",
      "noGroupChatsYet": "グループチャットはまだありません",
      "noGroupChatsDesc": "下の+ボタンから最初のグループチャットを作成して、複数のキャラクターと同時に会話しましょう",
      "newChat": "新しいチャット",
      "openChat": "チャットを開く",
      "chatSettings": "チャット設定",
      "sessionCount": "{{count}} 件のチャット"
    },
    "create": {
      "invalidPackage": "このパッケージはグループチャットパッケージではありません。",
      "inspectPackageError": "グループチャットパッケージの検査に失敗しました",
      "importPackageError": "グループチャットパッケージのインポートに失敗しました",
      "importChatpkg": "`.chatpkg`をインポート",
      "mapParticipantsTitle": "参加者のマッピング",
      "selectLocalCharacter": "この参加者のローカルキャラクターを選択してください。",
      "selectCharacterPlaceholder": "キャラクターを選択...",
      "importChatPackageTitle": "チャットパッケージをインポート",
      "importChatPackageDesc": "選択した`.chatpkg`を新しいグループセッションとしてインポートします。",
      "characterSelect": {
        "title": "グループチャットを作成",
        "subtitle": "グループ会話のキャラクターを選択",
        "selected": "選択済み",
        "ready": "準備完了",
        "minRequired": "最低2つ必要",
        "noCharactersYet": "キャラクターがまだいません",
        "noCharactersDesc": "グループチャットを始めるには、まずキャラクターを作成してください",
        "continueToSetup": "グループ設定へ進む"
      },
      "groupSetup": {
        "title": "グループ設定",
        "subtitle": "グループチャットの設定を構成",
        "chatType": "チャットタイプ",
        "conversation": "会話",
        "casualChat": "カジュアルチャット",
        "roleplay": "ロールプレイ",
        "withScenes": "シーン付き",
        "conversationDesc": "開始シーンなしのカジュアルなグループ会話",
        "roleplayDesc": "開始シーンと没入型プロンプト付きのロールプレイシナリオ",
        "speakerSelection": "スピーカー選択",
        "llm": "LLM",
        "aiPicks": "AIが選択",
        "heuristic": "ヒューリスティック",
        "scoreBased": "スコアベース",
        "roundRobin": "ラウンドロビン",
        "takeTurns": "順番に",
        "llmDesc": "デフォルトモデルを使用して誰が話すかを選択（トークンを消費）",
        "heuristicDesc": "参加バランスとコンテキストの手がかりを使用（無料）",
        "roundRobinDesc": "キャラクターが順番に話す（無料）",
        "chatBackground": "チャット背景",
        "optional": "（オプション）",
        "uploadBackground": "背景画像をアップロード",
        "backgroundDesc": "このグループチャットの背景画像を設定",
        "groupName": "グループ名",
        "removeBackground": "背景画像を削除",
        "groupNameAutoGenerate": "キャラクター名から自動生成するには空にしてください",
        "continueToScene": "開始シーンへ進む",
        "createGroupChat": "グループチャットを作成"
      },
      "startingScene": {
        "title": "開始シーン",
        "subtitle": "ロールプレイのオープニングシナリオを設定",
        "sceneSource": "シーンソース",
        "none": "なし",
        "custom": "カスタム",
        "fromCharacter": "キャラクターから",
        "noneDesc": "事前定義のシーンなしで開始",
        "customDesc": "独自のオープニングシナリオを作成",
        "fromCharacterDesc": "キャラクターのシーンを使用",
        "sceneContent": "シーン内容",
        "sceneContentPlaceholder": "このロールプレイの開始シーンを説明...",
        "sceneReferenceTip": "ヒント：{{@\"でキャラクターを参照",
        "selectScene": "シーンを選択",
        "sceneLabel": "のシーン",
        "copyToCustom": "カスタムにコピーして編集"
      }
    },
    "history": {
      "title": "グループチャット履歴",
      "subtitle": "すべてのグループ会話",
      "searchPlaceholder": "グループチャットを検索...",
      "active": "アクティブ（{{count}}）",
      "archived": "アーカイブ済み（{{count}}）",
      "noChatsYet": "グループチャットはまだありません",
      "noChatsDesc": "グループチャットを作成するとここに履歴が表示されます",
      "noMatchingChats": "一致するチャットがありません",
      "noMatchingDesc": "別の検索語をお試しください",
      "deleteSessionTitle": "グループチャットを削除しますか？",
      "deleteSessionDesc": "履歴から永久に削除されます",
      "deleteSessionButton": "チャットを削除",
      "keepChat": "このチャットを残す"
    },
    "session": {
      "chatTitlePlaceholder": "チャットタイトル...",
      "newChat": "新しいチャット",
      "rename": "名前変更",
      "unarchive": "アーカイブ解除",
      "archive": "アーカイブ",
      "messageCount": "{{count}}件のメッセージ"
    },
    "memories": {
      "tabMemories": "メモリ",
      "tabPinned": "ピン留め",
      "tabActivity": "アクティビティ",
      "processing": "処理中",
      "contextSummaryTitle": "コンテキスト要約",
      "addContextSummaryPrompt": "タップしてコンテキスト要約を追加...",
      "savedMemories": "保存されたメモリ",
      "resultsCount": "結果（{{count}}）",
      "searchPlaceholder": "メモリを検索...",
      "addMemory": "メモリを追加",
      "noMemoriesYet": "メモリはまだありません",
      "noMemoriesDesc": "上の追加ボタンをタップして作成",
      "noMatchingMemories": "一致するメモリがありません",
      "noMatchingDesc": "別の検索語をお試しください",
      "sessionNotFound": "セッションが見つかりません",
      "memoryActions": "メモリアクション",
      "tokens": "トークン",
      "cycle": "サイクル",
      "accessed": "アクセス",
      "cold": "コールド",
      "hot": "ホット",
      "activityLog": "アクティビティログ",
      "events": "イベント",
      "run": "実行",
      "processingMemories": "AIがグループメモリを整理中...",
      "memoryCycleSuccess": "メモリサイクルが正常に処理されました！",
      "memoryActionFailed": "メモリアクションに失敗しました",
      "newMemoryUpdates": "新しいメモリ更新があります",
      "noActivityYet": "アクティビティはまだありません",
      "noActivityDesc": "AIがダイナミックモードでメモリを管理するとツール呼び出しが表示されます",
      "contextSummaryPlaceholder": "メッセージ間でコンテキストの一貫性を保つための短い要約...",
      "addMemoryTitle": "メモリを追加",
      "memoryPlaceholder": "何を記憶すべきですか？",
      "saveMemory": "メモリを保存",
      "editMemoryTitle": "メモリを編集",
      "editMemoryPlaceholder": "メモリ内容を入力...",
      "edit": "編集",
      "pin": "ピン留め",
      "unpin": "ピン留め解除",
      "setHot": "ホットに設定",
      "setCold": "コールドに設定"
    },
    "toolLog": {
      "created": "作成",
      "deleted": "削除",
      "pinned": "ピン留め",
      "unpinned": "ピン留め解除",
      "done": "完了",
      "pinnedBadge": "ピン留め",
      "softDelete": "ソフト削除",
      "memoryCycle": "メモリサイクル",
      "failedAt": "失敗箇所：",
      "window": "ウィンドウ",
      "justNow": "たった今",
      "minutesAgo": "{{count}}分前",
      "hoursAgo": "{{count}}時間前",
      "yesterday": "昨日",
      "daysAgo": "{{count}}日前",
      "revertingTitle": "元に戻しています...",
      "revertCycleTitle": "このサイクルを元に戻す",
      "revertedAt": "元に戻しました {{time}}",
      "failedAtStage": "失敗ステージ: {{stage}}",
      "hideDebug": "デバッグを非表示",
      "debug": "デバッグ",
      "windowRange": "ウィンドウ {{start}}-{{end}}",
      "actionCreated": "作成しました",
      "actionDeleted": "削除しました",
      "actionPinned": "ピン留めしました",
      "actionUnpinned": "ピン留めを解除しました",
      "actionDone": "完了",
      "badgePinned": "ピン留め",
      "badgeSoftDelete": "ソフト削除",
      "badgeUndone": "取り消し",
      "badgeReverted": "元に戻した",
      "activityEmptyTitle": "まだアクティビティはありません",
      "activityEmptyDesc": "ダイナミックモードでAIがメモリを管理する際にツール呼び出しが表示されます"
    },
    "message": {
      "thinkingHard": "一生懸命考え中…",
      "thinkingLettuce": "レタス評議会に相談中…",
      "thinkingVoid": "虚空から思考を盗み中…",
      "thinkingBrainCells": "脳細胞をウォームアップ中…",
      "thinkingForbidden": "禁断の知識をロード中…",
      "thinkingOverthinking": "考えすぎ中（いつものこと）…",
      "thinkingPretending": "賢いふり中…",
      "thinkingCrunching": "架空の数字を計算中…",
      "thinkingArguing": "自分自身と議論中…",
      "thinkingUniverse": "宇宙にお願い中…",
      "thoughtProcess": "思考プロセス",
      "userAlt": "ユーザー",
      "assistantAlt": "アシスタント",
      "regenerateResponse": "応答を再生成",
      "variantLabel": "バリアント",
      "regenerating": "再生成中",
      "cancelAudioGeneration": "音声生成をキャンセル",
      "stopAudio": "音声を停止",
      "playMessageAudio": "メッセージ音声を再生",
      "playAudio": "音声を再生",
      "attachedImage": "添付画像",
      "assistantIsTyping": "アシスタントが入力中",
      "assistantTyping": "アシスタントが入力中"
    },
    "header": {
      "back": "戻る",
      "memories": "メモリ",
      "settings": "設定",
      "characters": "キャラクター"
    },
    "footer": {
      "mentionCharacter": "キャラクターをメンション",
      "noCharactersFound": "キャラクターが見つかりません",
      "moreOptions": "その他のオプション",
      "addImage": "画像を追加",
      "messagePlaceholder": "メッセージ... (@でメンション)",
      "stopGeneration": "生成を停止",
      "sendMessage": "メッセージを送信",
      "continueConversation": "会話を続ける",
      "dismissError": "エラーを閉じる",
      "removeAttachment": "添付を削除",
      "tabToSelect": "Tabで選択"
    },
    "messageActions": {
      "characterMessage": "キャラクターメッセージ",
      "yourMessage": "あなたのメッセージ",
      "whyCharacterResponded": "このキャラクターが応答した理由",
      "total": "合計",
      "edit": "編集",
      "copy": "コピー",
      "regenerateWithDifferent": "別のキャラクターで再生成",
      "rewindToHere": "ここまで巻き戻す",
      "unpinToDelete": "削除するにはピン留め解除",
      "delete": "削除",
      "editPlaceholder": "メッセージを編集...",
      "chooseCharacterTitle": "キャラクターを選択",
      "selectCharacterForRegeneration": "どのキャラクターが代わりに応答すべきか選択してください："
    },
    "settings": {
      "appDefault": "アプリデフォルト",
      "selectPersona": "ペルソナを選択",
      "noPersonas": "利用可能なペルソナがありません",
      "noPersonasDesc": "設定でペルソナを作成して会話をパーソナライズしましょう。",
      "searchPersonas": "ペルソナを検索...",
      "noPersona": "ペルソナなし",
      "noPersonaDesc": "ペルソナなしで続行",
      "noPersonasFound": "ペルソナが見つかりません",
      "noPersonasFoundDesc": "別の検索語をお試しください"
    },
    "groupSettings": {
      "title": "グループ編集",
      "subtitle": "今後のセッションのデフォルト設定を更新",
      "enterGroupName": "グループ名を入力",
      "participant": "参加者",
      "participants": "参加者",
      "uploading": "アップロード中...",
      "changeBackground": "背景を変更",
      "addBackgroundImage": "背景画像を追加",
      "removeBackground": "背景を削除",
      "persona": "ペルソナ",
      "personaSubtitle": "新しいセッションのデフォルトペルソナ",
      "personaLabel": "ペルソナ",
      "speakerSelection": "話者選択",
      "speakerSubtitle": "新しいセッションのデフォルト方式",
      "llm": "LLM",
      "aiPicks": "AIが選択",
      "heuristic": "ヒューリスティック",
      "scoreBased": "スコアベース",
      "roundRobin": "ラウンドロビン",
      "takeTurns": "順番制",
      "llmDesc": "デフォルトモデルを使用して話者を選択します（トークンを消費）",
      "heuristicDesc": "参加バランスとコンテキストの手がかりを使用（無料）",
      "roundRobinDesc": "キャラクターが順番に話します（無料）",
      "memoryMode": "メモリモード",
      "memorySubtitle": "新しいセッションのデフォルトメモリモード",
      "manual": "手動",
      "manualDesc": "自分でノートを管理",
      "dynamic": "ダイナミック",
      "dynamicDesc": "自動想起",
      "memoryDynamicInfo": "AIが会話から自動的に記憶を作成・取得します",
      "memoryManualInfo": "メモリノートは自分で追加・管理します",
      "characters": "キャラクター",
      "participantsActive": "{{total}}人の参加者 · {{active}}人がアクティブ",
      "add": "追加",
      "muted": "（ミュート）",
      "mutedByDefault": "デフォルトでミュート",
      "activeByDefault": "デフォルトでアクティブ",
      "unmuteCharacter": "キャラクターのミュートを解除",
      "muteCharacter": "キャラクターをミュート",
      "minTwoRequired": "最低2キャラクター必要",
      "removeCharacter": "キャラクターを削除",
      "groupMinCharacters": "グループには最低2キャラクターが必要です",
      "mutedCharactersNote": "ミュートされたキャラクターは自動話者選択でスキップされますが、明示的な`@メンション`で応答できます。",
      "addCharacterTitle": "キャラクターを追加",
      "allCharactersInGroup": "すべてのキャラクターはすでにこのグループにいます。",
      "removeCharacterTitle": "キャラクターを削除しますか？",
      "removeCharacterConfirm": "本当に削除しますか：",
      "removeCharacterFrom": "をグループのデフォルトから？",
      "removing": "削除中...",
      "remove": "削除"
    },
    "sessionSettings": {
      "subtitle": "グループチャットの設定を管理",
      "enterGroupName": "グループ名を入力",
      "participant": "参加者",
      "participants": "参加者",
      "message": "メッセージ",
      "messages": "メッセージ",
      "uploading": "アップロード中...",
      "changeBackground": "背景を変更",
      "addBackgroundImage": "背景画像を追加",
      "removeBackground": "背景を削除",
      "persona": "ペルソナ",
      "personaSubtitle": "この会話でのあなたの身元",
      "personaLabel": "ペルソナ",
      "speakerSelection": "話者選択",
      "speakerSubtitle": "次の話者の選び方",
      "llm": "LLM",
      "aiPicks": "AIが選択",
      "heuristic": "ヒューリスティック",
      "scoreBased": "スコアベース",
      "roundRobin": "ラウンドロビン",
      "takeTurns": "順番制",
      "llmDesc": "デフォルトモデルを使用して話者を選択します（トークンを消費）",
      "heuristicDesc": "参加バランスとコンテキストの手がかりを使用（無料）",
      "roundRobinDesc": "キャラクターが順番に話します（無料）",
      "characters": "キャラクター",
      "participantsActive": "{{total}}人の参加者 · {{active}}人がアクティブ",
      "add": "追加",
      "muted": "（ミュート）",
      "unmuteCharacter": "キャラクターのミュートを解除",
      "muteCharacter": "キャラクターをミュート",
      "minTwoRequired": "最低2キャラクター必要",
      "removeCharacter": "キャラクターを削除",
      "groupMinCharacters": "グループチャットには最低2キャラクターが必要です",
      "mutedCharactersNote": "ミュートされたキャラクターは自動話者選択でスキップされますが、明示的な`@メンション`で応答できます。",
      "data": "データ",
      "dataSubtitle": "会話のエクスポートまたはインポート",
      "export": "エクスポート",
      "exportDesc": "共有可能なファイルとして保存",
      "import": "インポート",
      "importDesc": "ファイルから会話を読み込み",
      "conversation": "会話",
      "conversationSubtitle": "複製または新しいチャットで続行",
      "duplicate": "複製",
      "duplicateDesc": "メッセージ付きまたはなしでこのチャットをコピー",
      "branchTo1on1": "1対1に分岐",
      "branchTo1on1Desc": "1人のキャラクターとプライベートに続行",
      "participation": "参加状況",
      "participationSubtitle": "キャラクター間の発言分布",
      "addCharacterTitle": "キャラクターを追加",
      "allCharactersInGroup": "すべてのキャラクターはすでにこのグループにいます。",
      "removeCharacterTitle": "キャラクターを削除しますか？",
      "removeCharacterConfirm": "本当に削除しますか：",
      "removeCharacterFrom": "をこのグループチャットから？",
      "removing": "削除中...",
      "remove": "削除",
      "cloneGroupTitle": "グループを複製",
      "withMessages": "メッセージ付き",
      "withMessagesDesc": "チャット履歴を含めてすべてを複製",
      "withoutMessages": "メッセージなし",
      "withoutMessagesDesc": "設定のみ複製（キャラクター、開始シーン）",
      "branchWithCharacterTitle": "キャラクターで分岐",
      "branchWithCharacterDesc": "1対1の会話として続行するキャラクターを選択してください。このグループのすべてのメッセージが変換されます。",
      "continueWith": "{{name}}との会話を続行",
      "exportChatPackageTitle": "チャットパッケージをエクスポート",
      "includeCharacterSnapshots": "キャラクタースナップショットを含める",
      "includeCharacterSnapshotsDesc": "パッケージ内にキャラクターデータを保持",
      "sessionOnly": "セッションのみ",
      "sessionOnlyDesc": "メッセージとメタデータのみエクスポート",
      "mapParticipantsTitle": "参加者をマッピング",
      "selectLocalCharacter": "この参加者のローカルキャラクターを選択してください。",
      "selectCharacterPlaceholder": "キャラクターを選択...",
      "continue": "続行",
      "importChatPackageTitle": "チャットパッケージをインポート",
      "importChatPackageDesc": "選択した`.chatpkg`を新しいグループセッションとしてインポートします。",
      "importing": "インポート中..."
    },
    "page": {
      "selectingCharacter": "キャラクターを選択中...",
      "sessionNotFound": "グループセッションが見つかりません",
      "backToGroupChats": "グループチャットに戻る",
      "startConversation": "{{names}}との会話を開始",
      "scrollToBottom": "一番下までスクロール",
      "addContent": "コンテンツを追加",
      "uploadImage": "画像をアップロード",
      "helpMeReply": "返信アシスト",
      "helpMeReplyDesc": "AIに返信内容を提案してもらいます",
      "haveDraftPrompt": "下書きメッセージがあります。どのように進めますか?",
      "useMyTextAsBase": "下書きを元にする",
      "useMyTextAsBaseDesc": "下書きを拡張して改善します",
      "writeSomethingNew": "新しく書く",
      "writeSomethingNewDesc": "新しい返信を生成します",
      "suggestedReply": "返信案",
      "reasoningBeforeWriting": "返信を書く前に推論中...",
      "writingYourReply": "返信を作成中...",
      "regenerate": "再生成",
      "useReply": "返信を使用",
      "helpMeReplyFailedGeneric": "返信アシストに失敗しました。",
      "helpMeReplyFailedGenerate": "返信アシストで返信を生成できませんでした。",
      "noAudioCaptured": "音声が録音されませんでした。",
      "noWhisperModel": "インストール済みのWhisperモデルが見つかりません。音声認識設定でインストールしてください。",
      "cancelRecording": "録音をキャンセル",
      "transcribing": "文字起こし中",
      "stopAndTranscribe": "停止して文字起こし",
      "recordVoice": "音声を録音",
      "learnCorrection": "修正を学習:",
      "learning": "学習中...",
      "learn": "学習",
      "ignore": "無視",
      "groupChatFailed": "グループチャットに失敗しました。",
      "failedToCopy": "コピーに失敗しました",
      "copied": "コピーしました!",
      "cannotDeletePinned": "ピン留めされたメッセージは削除できません。先にピン留めを解除してください。",
      "failedToDelete": "削除に失敗しました",
      "messageNotFound": "メッセージが見つかりません",
      "cannotRewindPinned": "巻き戻せません: この時点以降にピン留めされたメッセージがあります。先にピン留めを解除してください。",
      "failedToRewind": "巻き戻しに失敗しました",
      "failedToTogglePin": "ピン留めの切り替えに失敗しました",
      "messagePinned": "メッセージをピン留めしました",
      "messageUnpinned": "メッセージのピン留めを解除しました",
      "failedToSave": "保存に失敗しました"
    },
    "memoriesPage": {
      "summarizingConversation": "会話を要約中",
      "analyzingMemories": "メモリを解析中",
      "applyingChanges": "変更を適用中",
      "organizingMemories": "メモリを整理中",
      "retryingMemoryCycle": "メモリサイクルを再試行中...",
      "processingMemories": "メモリを処理中...",
      "memorySystemError": "メモリシステムエラー",
      "contextSummary": "コンテキスト要約",
      "contextSummaryTitle": "コンテキスト要約",
      "savedMemories": "保存されたメモリ",
      "resultsCount": "結果 ({{count}})",
      "searchMemoriesPlaceholder": "メモリを検索...",
      "addMemory": "メモリを追加",
      "memoryActions": "メモリ操作",
      "clearSearch": "検索をクリア",
      "noMatchingMemories": "一致するメモリはありません",
      "noMemoriesYet": "まだメモリがありません",
      "tryDifferentSearch": "別の検索キーワードを試してください",
      "tapAddToCreate": "上の追加ボタンをタップして作成してください",
      "pinnedMessages": "ピン留めメッセージ",
      "refresh": "更新",
      "noPinnedMessages": "ピン留めされたメッセージはありません",
      "pinImportantDesc": "重要なグループチャットのメッセージをピン留めして、常にコンテキストに含めることができます。",
      "assistant": "アシスタント",
      "user": "ユーザー",
      "unpin": "ピン留めを解除",
      "failedToUnpinMessage": "メッセージのピン留め解除に失敗しました",
      "activityLog": "アクティビティログ",
      "run": "実行",
      "addMemoryTitle": "メモリを追加",
      "editMemoryTitle": "メモリを編集",
      "memoryTitle": "メモリ",
      "memoryPlaceholder": "何を記憶すべきですか?",
      "saveMemory": "メモリを保存",
      "editMemoryPlaceholder": "メモリの内容を入力...",
      "saving": "保存中...",
      "save": "保存",
      "cancel": "キャンセル",
      "contextSummaryPlaceholder": "メッセージ間でコンテキストの一貫性を保つための短い要約...",
      "contextSummaryPrompt": "タップしてコンテキスト要約を追加...",
      "cycle": "サイクル",
      "accessed": "アクセス済み",
      "cold": "コールド",
      "hot": "ホット",
      "tokens": "トークン",
      "pin": "ピン留め",
      "setHot": "ホットに設定",
      "setCold": "コールドに設定",
      "edit": "編集",
      "delete": "削除",
      "failedToToggleMemPin": "ピン留めの切り替えに失敗しました",
      "failedToRemoveMemory": "メモリの削除に失敗しました",
      "toolEventsCountAria": "件のイベント",
      "activityEmptyDesc": "ダイナミックモードでAIがメモリを管理する際にツール呼び出しが表示されます",
      "memoryCycleSuccess": "メモリサイクルが正常に処理されました!"
    },
    "messageActionsExtra": {
      "characterMessage": "キャラクターのメッセージ",
      "yourMessage": "あなたのメッセージ",
      "whyCharacterResponded": "このキャラクターが応答した理由",
      "promptTokensTitle": "プロンプトトークン",
      "completionTokensTitle": "完了トークン",
      "total": "合計",
      "regenerateWithDifferent": "別のキャラクターで再生成",
      "unpin": "ピン留めを解除",
      "pin": "ピン留め",
      "rewindToHere": "ここまで巻き戻す",
      "unpinToDelete": "削除するにはピン留めを解除",
      "editPlaceholder": "メッセージを編集...",
      "chooseCharacter": "キャラクターを選択",
      "selectCharacterForRegeneration": "代わりに応答するキャラクターを選択してください:"
    },
    "timeAgo": {
      "justNow": "たった今",
      "minutesAgo": "{{count}}分前",
      "hoursAgo": "{{count}}時間前",
      "daysAgo": "{{count}}日前"
    },
    "memoriesController": {
      "missingSessionId": "sessionIdがありません",
      "sessionNotFound": "セッションが見つかりません",
      "failedToLoadSession": "セッションの読み込みに失敗しました",
      "failedToUpdateTemperature": "メモリの温度更新に失敗しました",
      "failedToSaveSummary": "要約の保存に失敗しました",
      "cycleFailed": "グループメモリサイクルに失敗しました",
      "failedToAddMemory": "メモリの追加に失敗しました",
      "failedToUpdateMemory": "メモリの更新に失敗しました",
      "failedToRunCycle": "メモリサイクルの実行に失敗しました",
      "failedToRevertCycle": "サイクルの取り消しに失敗しました",
      "failedToRefresh": "更新に失敗しました",
      "failedToDismissError": "エラーの消去に失敗しました",
      "failedToTogglePinned": "ピン留めメッセージの切り替えに失敗しました",
      "sessionNotLoaded": "セッションが読み込まれていません",
      "revertCycleTitle": "このサイクルを元に戻しますか?",
      "revertConfirm": "元に戻す"
    },
    "chatController": {
      "sessionNotFound": "グループセッションが見つかりません",
      "failedToLoadGroupChat": "グループチャットの読み込みに失敗しました",
      "requestFailed": "グループチャットのリクエストに失敗しました",
      "failedToSendMessage": "メッセージの送信に失敗しました",
      "failedToRegenerate": "再生成に失敗しました",
      "failedToContinue": "続行に失敗しました",
      "failedToCopy": "コピーに失敗しました",
      "cannotDeletePinned": "ピン留めされたメッセージは削除できません。先にピン留めを解除してください。",
      "failedToDelete": "削除に失敗しました",
      "messageNotFound": "メッセージが見つかりません",
      "cannotRewindPinned": "巻き戻せません: この時点以降にピン留めされたメッセージがあります。先にピン留めを解除してください。",
      "failedToRewind": "巻き戻しに失敗しました",
      "failedToTogglePin": "ピン留めの切り替えに失敗しました",
      "messagePinned": "メッセージをピン留めしました",
      "messageUnpinned": "メッセージのピン留めを解除しました",
      "failedToSave": "保存に失敗しました",
      "copied": "コピーしました!"
    },
    "historyController": {
      "failedToLoadData": "データの読み込みに失敗しました",
      "failedToDelete": "削除に失敗しました: {{error}}",
      "failedToRename": "名前変更に失敗しました: {{error}}",
      "failedToArchive": "アーカイブに失敗しました: {{error}}",
      "failedToUnarchive": "アーカイブ解除に失敗しました: {{error}}",
      "failedToDuplicate": "複製に失敗しました"
    },
    "sessionSettingsController": {
      "failedToLoad": "グループチャット設定の読み込みに失敗しました",
      "noPersona": "ペルソナなし",
      "customPersona": "カスタムペルソナ",
      "minOneActive": "少なくとも1人の参加者をアクティブにしておく必要があります"
    },
    "groupSettingsController": {
      "notFound": "グループが見つかりません",
      "failedToLoad": "グループ設定の読み込みに失敗しました"
    },
    "createForm": {
      "failedToLoadCharacters": "キャラクターの読み込みに失敗しました",
      "selectAtLeastTwo": "グループチャットには2人以上のキャラクターを選択してください",
      "failedToCreate": "グループチャットの作成に失敗しました"
    },
    "groupSetupExtra": {
      "memoryMode": "メモリモード",
      "manual": "手動",
      "manualDesc": "ノートを自分で管理します",
      "dynamic": "ダイナミック",
      "dynamicDesc": "自動要約と想起",
      "memoryManualInfo": "メモリノートを自分で追加・管理します",
      "memoryDynamicInfo": "AIが会話から自動的にメモリを作成・取得します",
      "backgroundPreviewAlt": "背景プレビュー"
    },
    "createExtra": {
      "enterGroupNamePlaceholder": "グループ名を入力...",
      "unknown": "不明"
    },
    "startingSceneExtra": {
      "insertAs": "{{snippet}}として挿入"
    },
    "sessionCardExtra": {
      "unknown": "不明",
      "untitledChat": "無題のチャット"
    },
    "sessionListExtra": {
      "unknown": "不明"
    },
    "chatHeaderExtra": {
      "unknownError": "不明なエラー"
    },
    "chatSettingsExtra": {
      "invalidPackage": "このパッケージはグループチャットパッケージではありません。",
      "failedExport": "グループチャットパッケージのエクスポートに失敗しました",
      "failedInspect": "グループチャットパッケージの検査に失敗しました",
      "failedImport": "グループチャットパッケージのインポートに失敗しました",
      "exportSuccess": "グループチャットパッケージのエクスポート先:\n{{path}}",
      "backgroundAlt": "背景",
      "removeBackgroundAria": "背景を削除",
      "backAria": "戻る",
      "backToGroupChats": "グループチャットに戻る"
    },
    "groupSettingsExtra": {
      "backToGroups": "グループに戻る",
      "backAria": "戻る",
      "removeBackgroundAria": "背景を削除"
    },
    "historyPage": {
      "backAria": "グループチャットに戻る",
      "clearSearchAria": "検索をクリア",
      "resultsLabel": "{{count}}件の結果",
      "resultsLabelPlural": "{{count}}件の結果",
      "untitledChat": "無題のチャット",
      "noPinnedMessages": "ピン留めされたメッセージはありません"
    },
    "personaSelectorExtra": {
      "insertAs": "として挿入",
      "appDefault": "アプリのデフォルト",
      "defaultBadge": "デフォルト",
      "selectPersonaTitle": "ペルソナを選択",
      "noPersonaTitle": "ペルソナなし",
      "noPersonaDesc": "ペルソナなしで続行します",
      "noPersonasAvailable": "利用可能なペルソナがありません",
      "noPersonasDesc": "会話をパーソナライズするには、設定でペルソナを作成してください。",
      "searchPersonas": "ペルソナを検索...",
      "noPersonasFound": "ペルソナが見つかりません",
      "tryDifferentSearch": "別の検索キーワードを試してください"
    },
    "footerExtra": {
      "cancelRecording": "録音をキャンセル",
      "transcribing": "文字起こし中",
      "stopAndTranscribe": "停止して文字起こし",
      "recordVoice": "音声を録音",
      "attachmentAltDefault": "添付ファイル"
    },
    "pageExtra": {
      "noAudioCaptured": "音声が録音されませんでした。",
      "noWhisperModelInstalled": "インストール済みのWhisperモデルが見つかりません。音声認識設定でインストールしてください。",
      "scrollToBottomAria": "一番下までスクロール"
    },
    "addContentMenu": {
      "title": "コンテンツを追加",
      "uploadImage": "画像をアップロード"
    },
    "helpMeReplyMenu": {
      "title": "返信アシスト",
      "helpMeReplyDesc": "AIに返信内容を提案してもらいます",
      "draftPrompt": "下書きメッセージがあります。どのように進めますか?",
      "useTextAsBase": "下書きを元にする",
      "useTextAsBaseDesc": "下書きを拡張して改善します",
      "writeSomethingNew": "新しく書く",
      "writeSomethingNewDesc": "新しい返信を生成します"
    },
    "suggestedReplyMenu": {
      "title": "返信案",
      "reasoningBeforeWriting": "返信を書く前に推論中...",
      "writingYourReply": "返信を作成中...",
      "regenerate": "再生成",
      "useReply": "返信を使用"
    },
    "memoriesPageExtra": {
      "sessionNotFound": "セッションが見つかりません",
      "memorySystemError": "メモリシステムエラー",
      "retryingMemoryCycle": "メモリサイクルを再試行中...",
      "processingMemories": "メモリを処理中...",
      "memoryCycleSuccess": "メモリサイクルが正常に処理されました!",
      "contextSummaryTitle": "コンテキスト要約",
      "activityTabLabel": "アクティビティ",
      "noMatchingMemoriesDesc": "別の検索キーワードを試してください",
      "chatHistoryTitle": "チャット履歴",
      "chatHistoryDesc": "会話を表示・管理します"
    },
    "settingsPageExtra": {
      "quickActionsSection": "クイックアクション",
      "chatHistoryTitle": "チャット履歴",
      "chatHistoryDesc": "会話を表示・管理します",
      "lorebrooksTitle": "ロアブック",
      "lorebrooksDesc": "セッションロアブックを添付し、必要に応じて各キャラクターのロアブックを無視できます。",
      "manageLorebooks": "ロアブックを管理"
    },
    "groupSettingsPageExtra": {
      "lorebrooksTitle": "ロアブック",
      "lorebrooksDesc": "共有ロアブックを添付し、キャラクターのロアブックをデフォルトで適用するかどうかを制御します。",
      "manageLorebooks": "ロアブックを管理"
    }
  },
  "characters": {
    "empty": {
      "title": "キャラクターがまだいません",
      "description": "ユニークな性格を持つカスタムAIキャラクターを作成",
      "createButton": "キャラクターを作成"
    },
    "progress": {
      "identity": "アイデンティティ",
      "scenes": "シーン",
      "details": "詳細"
    },
    "identity": {
      "title": "キャラクターを作成",
      "subtitle": "AIキャラクターにアイデンティティを与えましょう",
      "tapCameraToAdd": "カメラをタップしてアバターを追加または生成",
      "importingAvatar": "アバターをインポート中...",
      "characterName": "キャラクター名 *",
      "characterNamePlaceholder": "キャラクター名を入力...",
      "characterNameDesc": "この名前はチャット会話に表示されます",
      "avatarGradient": "アバターグラデーション",
      "avatarGradientDesc": "アバターの色からダイナミックなグラデーションを生成",
      "chatBackgroundLabel": "チャット背景",
      "optionalSuffix": "(任意)",
      "backgroundPreviewAlt": "背景プレビュー",
      "backgroundPreviewBadge": "背景プレビュー",
      "addBackground": "背景を追加",
      "addBackgroundHint": "アップロードまたはライブラリから選択",
      "uploadImage": "画像をアップロード",
      "chooseFromLibrary": "ライブラリから選択",
      "backgroundDesc": "チャット会話用のオプションの背景画像",
      "continueToDescription": "説明に進む",
      "importCharacterFromFile": "ファイルからキャラクターをインポート",
      "importCharacterDesc": "PNGカード、.uec、または.jsonエクスポートファイルからキャラクターを読み込みます"
    },
    "details": {
      "title": "キャラクター詳細",
      "subtitle": "性格と行動を定義",
      "definition": "定義 *",
      "wordCount": "{{count}}語",
      "definitionPlaceholder": "性格、話し方、背景、知識分野を説明...",
      "definitionDesc": "トーン、特性、会話スタイルを具体的に記述してください",
      "availablePlaceholders": "利用可能なプレースホルダー："
    },
    "scenes": {
      "title": "開始シーン",
      "subtitle": "会話のオープニングシナリオを作成",
      "default": "デフォルト",
      "hasSceneDirection": "シーン指示あり",
      "continueToScenes": "開始シーンへ進む"
    },
    "extras": {
      "title": "追加詳細",
      "subtitle": "すべてのフィールドはオプションです",
      "nickname": "ニックネーム",
      "nicknamePlaceholder": "ユーザーはこのキャラクターを何と呼びますか？",
      "nicknameDesc": "会話で使用される代替表示名",
      "creator": "作成者",
      "creatorPlaceholder": "作成者名...",
      "tags": "タグ",
      "tagsPlaceholder": "ファンタジー、SF、ロマンス...",
      "tagsDesc": "フィルタリングと整理用のカンマ区切りリスト",
      "creatorNotes": "作成者ノート",
      "creatorNotesPlaceholder": "使用のヒント、ロアのコンテキスト、他のユーザーへの説明...",
      "multilingualNotes": "多言語ノート",
      "multilingualNotesHint": "言語コードをキーとするJSONオブジェクト",
      "creatingCharacter": "キャラクターを作成中..."
    },
    "preview": {
      "unnamedCharacter": "名前なしキャラクター",
      "sceneCount": "{{count}}件のシーン",
      "customPrompt": "カスタムプロンプト",
      "description": "説明",
      "startingScene": "開始シーン",
      "gradientEnabled": "グラデーション有効",
      "customModel": "カスタムモデル",
      "avatarAlt": "キャラクターアバター",
      "characterFallback": "キャラクター"
    },
    "personaPreview": {
      "unnamedPersona": "名前なしペルソナ",
      "noDescription": "説明がまだありません",
      "default": "デフォルト",
      "description": "説明"
    },
    "lorebookPreview": {
      "untitledLorebook": "無題のロアブック",
      "entryCount": "{{count}}件のエントリ",
      "entries": "エントリ",
      "noEntriesYet": "エントリはまだありません",
      "untitledEntry": "無題のエントリ",
      "noContentYet": "コンテンツはまだありません",
      "alwaysActive": "常にアクティブ",
      "moreEntries": "他に{{count}}件のエントリー",
      "moreEntry": "他に{{count}}件のエントリー"
    },
    "creationHelper": {
      "moreOptions": "その他のオプション",
      "describePlaceholder": "キャラクターを説明...",
      "stopGeneration": "生成を停止",
      "sendMessage": "メッセージを送信",
      "addToMessage": "メッセージに追加",
      "uploadImageDesc": "アバターまたは参照画像を追加",
      "referenceCharacterDesc": "既存のキャラクターをインスピレーションとして使用",
      "referencePersonaDesc": "あなたのペルソナをコンテキストとして使用",
      "retry": "再試行",
      "attachmentAlt": "添付ファイル",
      "removeAttachment": "添付ファイルを削除",
      "removeReference": "リファレンスを削除",
      "uploadImageTitle": "画像をアップロード",
      "referenceCharacterTitle": "リファレンスキャラクター",
      "referencePersonaTitle": "リファレンスペルソナ"
    },
    "lorebook": {
      "keywords": "キーワード",
      "caseSensitive": "大文字小文字を区別",
      "typeKeyword": "キーワードを入力...",
      "addButton": "追加",
      "untitledEntry": "無題のエントリ",
      "alwaysActive": "常にアクティブ",
      "noKeywords": "キーワードなし",
      "dragToReorder": "ドラッグして並べ替え",
      "disabled": "無効",
      "noLorebooksYet": "ロアブックはまだありません",
      "createLorebookDesc": "このキャラクターのワールドロアを追加するためにロアブックを作成",
      "createLorebook": "ロアブックを作成",
      "searchPlaceholder": "ロアブックを検索...",
      "noMatchingLorebooks": "一致するロアブックが見つかりません",
      "activeLorebooks": "アクティブなロアブック",
      "enabledForCharacter": "このキャラクターで有効",
      "enabledForGroup": "このグループで有効",
      "enabledForSession": "このセッションで有効",
      "tapToViewEntries": "タップしてエントリを表示",
      "newLorebookTitle": "新しいロアブック",
      "nameLabel": "名前",
      "enterNamePlaceholder": "ロアブック名を入力...",
      "lorebookExplanation": "ロアブックにはキーワードが一致した時にプロンプトに挿入されるロアエントリが含まれます。",
      "viewEntries": "エントリを表示",
      "editEntriesDesc": "ロアブックエントリを編集",
      "disableForCharacter": "キャラクターで無効にする",
      "enableForCharacter": "キャラクターで有効にする",
      "disableForGroup": "グループで無効にする",
      "enableForGroup": "グループで有効にする",
      "disableForSession": "セッションで無効にする",
      "enableForSession": "セッションで有効にする",
      "removeFromActive": "このキャラクターのアクティブなロアブックから削除",
      "addToActive": "このキャラクターのアクティブなロアブックに追加",
      "removeFromActiveGroup": "このグループのアクティブなロアブックから削除",
      "addToActiveGroup": "このグループのアクティブなロアブックに追加",
      "removeFromActiveSession": "このセッションのアクティブなロアブックから削除",
      "addToActiveSession": "このセッションのアクティブなロアブックに追加",
      "deleteConfirmTitle": "ロアブックを削除しますか？",
      "deleteConfirmMessage": "このロアブックを削除しますか？すべてのエントリが失われます。",
      "deleteLorebook": "ロアブックを削除",
      "noEntriesYet": "エントリはまだありません",
      "addEntriesToInject": "チャットにロアを挿入するためにエントリを追加",
      "createEntry": "エントリを作成",
      "searchEntries": "エントリを検索...",
      "noMatchingEntries": "一致するエントリが見つかりません",
      "entryDefaultName": "エントリ",
      "editEntry": "エントリを編集",
      "editEntryDesc": "タイトル、キーワード、コンテンツを変更",
      "disableEntry": "エントリを無効にする",
      "enableEntry": "エントリを有効にする",
      "entryDisabledDesc": "エントリはプロンプトに挿入されません",
      "entryEnabledDesc": "キーワードが一致するとエントリが挿入されます",
      "deleteEntry": "エントリを削除",
      "titleLabel": "タイトル",
      "titlePlaceholder": "このエントリに名前を付けてください...",
      "enabled": "有効",
      "includeInPrompts": "プロンプトに含める",
      "alwaysOn": "常にオン",
      "noKeywordsNeeded": "キーワード不要",
      "contentLabel": "コンテンツ",
      "contentPlaceholder": "ロアのコンテキストをここに記述...",
      "saveEntry": "エントリを保存",
      "noCharacterId": "キャラクターIDが提供されていません",
      "sectionActive": "アクティブ",
      "sectionAvailable": "利用可能",
      "entryCount": "{{count}}件のエントリー",
      "keywordDetectionMode": "キーワード検出",
      "keywordDetectionRecentWindow": "最新の10メッセージ",
      "keywordDetectionRecentWindowDesc": "最新10メッセージの会話ウィンドウに対してマッチします。",
      "keywordDetectionLatestUser": "最新のユーザーメッセージのみ",
      "keywordDetectionLatestUserDesc": "最新のユーザー送信メッセージにのみマッチします。",
      "preview": {
        "title": "トリガープレビュー",
        "openButton": "プレビュー",
        "missingContext": "ロアブックが選択されていません。",
        "noEntries": "このロアブックにエントリーを追加して、何がトリガーされるか確認してください。",
        "modeRecentShort": "最新{{count}}",
        "modeLatestUserShort": "最新ユーザー",
        "inWindow": "ウィンドウ内 {{count}}件",
        "tabSession": "セッション",
        "tabCompose": "作成",
        "activeStat": "{{active}} / {{total}} アクティブ",
        "tokensStat": "{{count}}トークン",
        "sessionPickerLabel": "セッション",
        "sessionMeta": "{{count}}件のメッセージ",
        "noSessions": "まだチャットセッションがありません。",
        "noSessionsHint": "セッションを選択してください",
        "noMessages": "このセッションにはまだメッセージがありません。",
        "scanHeaderRecent": "最新{{depth}}件中{{shown}}件をスキャン中",
        "scanHeaderLatest": "最新のユーザーメッセージをスキャン中",
        "matchCount": "{{hits}}件ヒット · {{entries}}件のエントリー",
        "emptyMessage": "(空)",
        "roleUser": "ユーザー",
        "roleAssistant": "アシスタント",
        "roleScene": "シーン",
        "roleSystem": "システム",
        "composeHeader": "スクラッチパッド",
        "composeMatches": "{{count}}件マッチ",
        "activeLabel": "{{count}}件アクティブ",
        "composePlaceholder": "テキストを入力または貼り付けてキーワードマッチをテスト...\n\n例:\n図書館は静かで、古い暖房機の音だけが響いていた。\n彼女は先週貸した本を読んだか尋ねてきた。",
        "sectionActive": "アクティブ · {{count}}",
        "sectionInactive": "非アクティブ · {{count}}",
        "statusMatched": "マッチ",
        "statusAlways": "常時",
        "statusDisabled": "オフ",
        "statusNoKeywords": "キーなし",
        "statusNotMatched": "マッチなし",
        "tokensShort": "{{count}}t",
        "injectionTitle": "最終インジェクション",
        "injectionEmpty": "アクティブなエントリーがありません。インジェクションされるものはありません。",
        "copy": "コピー",
        "copyFailed": "クリップボードへのコピーに失敗しました。",
        "saveFailed": "エントリーの保存に失敗しました。",
        "deleteFailed": "エントリーの削除に失敗しました。",
        "deleteConfirmTitle": "本当によろしいですか?",
        "deleteConfirmMessage": "「{{title}}」を削除しますか?この操作は取り消せません。",
        "contextMenuTitle": "{{count}}件のエントリーがこれを使用しています"
      }
    },
    "templates": {
      "characterNotFound": "キャラクターが見つかりません",
      "templateCount": "{{count}}件のテンプレート",
      "newTemplate": "新しいテンプレート",
      "noTemplatesYet": "テンプレートはまだありません",
      "explanation": "チャットテンプレートでは、あなたと{{name}}の両方からの事前書き込みメッセージで会話を開始できます。",
      "createTemplate": "テンプレートを作成",
      "messageCount": "{{count}}件のメッセージ",
      "deleteTemplate": "テンプレートを削除",
      "contextMenuFallbackTitle": "テンプレート",
      "importedToast": {
        "title": "インポート済み",
        "message": "「{{name}}」を追加しました。"
      },
      "importFailed": "インポートに失敗しました",
      "exportFailed": "エクスポートに失敗しました"
    },
    "templateEditor": {
      "noScene": "シーンなし",
      "untitled": "無題",
      "dragMessage": "メッセージをドラッグ",
      "editMessage": "メッセージを編集",
      "deleteMessage": "メッセージを削除",
      "writeMessagePlaceholder": "メッセージ内容を記述...",
      "characterNotFound": "キャラクターが見つかりません",
      "scene": "シーン",
      "noMessagesYet": "メッセージはまだありません",
      "addMessagesDesc": "{{name}}との会話スターターを構築するためにメッセージを追加してください。",
      "addMessage": "メッセージを追加",
      "name": "名前",
      "nameExample": "例：カジュアルな挨拶",
      "startingScene": "開始シーン",
      "systemPrompt": "システムプロンプト",
      "characterDefault": "キャラクターデフォルト",
      "nextMessageAs": "次のメッセージの送信者",
      "messages": "メッセージ",
      "roles": "ロール",
      "hoverTip": "メッセージをホバーしてドラッグ、編集、削除。",
      "footerTip": "フッターバーを使用して会話に新しいメッセージを追加。",
      "templateNamePlaceholder": "テンプレート名...",
      "selectScene": "シーンを選択",
      "startWithoutScene": "シーンメッセージなしで開始",
      "selectSystemPrompt": "システムプロンプトを選択",
      "useCharacterSystemPrompt": "キャラクターレベルのシステムプロンプトを使用"
    },
    "referenceSelector": {
      "selectCharacter": "キャラクターを選択",
      "selectPersona": "ペルソナを選択",
      "searchPlaceholder": "{{type}}を検索...",
      "loading": "読み込み中...",
      "noMatch": "検索に一致する{{type}}はありません",
      "noAvailable": "利用可能な{{type}}はありません"
    },
    "voiceLoading": {
      "failed": "音声の読み込みに失敗しました"
    },
    "activeLorebooks": {
      "sectionTitle": "アクティブなロアブック",
      "selectedSummary": "{{count}}件アクティブ",
      "untitledLorebook": "無題のロアブック",
      "usingNone": "キャラクターロアブックを使用していません",
      "loading": "ロアブックを読み込み中...",
      "loadFailed": "ロアブックの読み込みに失敗しました",
      "inheritHint": "セッションがオーバーライドしない限り、これらを継承します。",
      "clear": "クリア",
      "chooseHint": "このキャラクターがデフォルトでアクティブにするロアブックを選択します。既存のセッションは引き続きこのリストを上書きできます。",
      "emptyState": "まだロアブックがありません。最初にロアブックマネージャーから作成してください。"
    },
    "description": {
      "descriptionLabel": "説明",
      "descriptionPlaceholder": "カードやリストに表示される短い概要...",
      "descriptionHint": "UI用のオプションの短い説明です。プロンプトでは完全な定義が使用されます。",
      "companionPromptLabel": "コンパニオンプロンプト(任意)",
      "systemPromptLabel": "システムプロンプト(任意)",
      "loadingTemplates": "テンプレートを読み込み中...",
      "useAppCompanionDefault": "アプリのコンパニオンデフォルトを使用",
      "useAppDefault": "アプリのデフォルトを使用",
      "companionPromptHint": "コンパニオンプロンプトとして別途保存されます。通常のロールプレイシステムプロンプトは変更されません。",
      "systemPromptHint": "カスタムシステムプロンプトを選択するか、デフォルトを使用します。",
      "groupChatConvLabel": "グループチャットプロンプト(会話)",
      "groupChatConvHint": "グループチャットでこのキャラクターの会話プロンプトを上書きします",
      "groupChatRpLabel": "グループチャットプロンプト(ロールプレイ)",
      "groupChatRpHint": "グループチャットでこのキャラクターのロールプレイプロンプトを上書きします",
      "voiceLabel": "音声(任意)",
      "loadingVoices": "音声を読み込み中...",
      "customVoiceFallback": "カスタム音声",
      "providerVoiceFallback": "プロバイダー音声",
      "selectedVoiceFallback": "選択した音声",
      "noVoiceAssigned": "音声が割り当てられていません",
      "addVoicesHint": "設定 → 音声で音声を追加してください",
      "voiceAssignHint": "今後のテキスト読み上げ用に音声を割り当てます",
      "autoplayLabel": "音声を自動再生",
      "autoplayOn": "このキャラクターの返信を自動再生します",
      "autoplayOff": "まず音声を選択してください",
      "aiModelLabel": "AIモデル *",
      "loadingModels": "モデルを読み込み中...",
      "selectedModelFallback": "選択したモデル",
      "selectModelPlaceholder": "モデルを選択",
      "noModelsConfigured": "設定されたモデルがありません",
      "noModelsHint": "続行するにはまず設定でプロバイダーを追加してください",
      "aiModelHint": "このモデルがキャラクターの応答を駆動します",
      "fallbackModelLabel": "フォールバックモデル(任意)",
      "selectedFallbackFallback": "選択したフォールバックモデル",
      "fallbackOff": "オフ(フォールバックなし)",
      "fallbackHint": "プライマリモデルが失敗した場合のみこのモデルで再試行します",
      "memoryModeLabel": "メモリモード",
      "enableInSettingsHint": "切り替えるには設定で有効にしてください",
      "memoryManual": "手動",
      "memoryManualDescDesktop": "メモリノートを自分で追加・管理します。",
      "memoryManualDescMobile": "現在のシステム: メモリノートを自分で追加・管理します。",
      "memoryDynamic": "ダイナミック",
      "memoryDynamicDescDesktop": "自動要約とコンテキスト更新。",
      "memoryDynamicDescMobile": "このキャラクター用の自動要約とコンテキスト更新。",
      "memoryHint": "ダイナミックメモリは詳細設定で有効化が必要です。それ以外の場合は手動メモリが使用されます。",
      "selectModelTitle": "モデルを選択",
      "selectFallbackModelTitle": "フォールバックモデルを選択",
      "searchModelsPlaceholder": "モデルを検索...",
      "selectVoiceTitle": "音声を選択",
      "searchVoicesPlaceholder": "音声を検索...",
      "myVoices": "マイ音声",
      "providerVoicesLabel": "{{provider}} 音声",
      "providerFallback": "プロバイダー"
    },
    "interactionMode": {
      "sectionLabel": "インタラクションモード",
      "sectionHint": "このキャラクターをRPキャラクターとして振る舞わせるか、永続的なコンパニオンとして振る舞わせるかを選択します。",
      "activeBadge": "アクティブ",
      "roleplayTitle": "ロールプレイ",
      "roleplaySubtitle": "シーン主導のチャット、ナラティブな枠組み、開始シナリオ。",
      "companionTitle": "コンパニオン",
      "companionSubtitle": "感情状態とコンパニオンメモリを持つ関係性主導のチャット。"
    },
    "startingScene": {
      "openingContextTitle": "オープニングコンテキスト",
      "openingContextSubtitle": "このコンパニオンの最初のチャット用コンテキスト(任意)。コンパニオンセッションはシーンなしでも開始できます。",
      "sceneDirectionLabel": "シーンの方向性",
      "setAsDefault": "デフォルトに設定",
      "noOpeningContext": "オープニングコンテキストはまだありません",
      "noScenesYet": "まだシーンがありません",
      "skipForCompanion": "コンパニオンモードではこれをスキップできます。",
      "createFirstScene": "最初のシーンを作成して始めましょう",
      "openingPlaceholder": "コンパニオンが最初のメッセージの前にどこにいたか、何をしていたかなど、任意のオープニングコンテキスト...",
      "scenePlaceholder": "ロールプレイ用の開始シーンまたはシナリオを作成します(例: 「あなたは黄昏時の神秘的な森にいることに気づく...」)",
      "addDirection": "+ 方向性を追加",
      "directionAdded": "方向性を追加しました",
      "wordsCount": "{{count}}語",
      "placeholderHelp": "キャラクターには{{charTag}}、ペルソナには{{userTag}}(エイリアス {{personaTag}})を使用してください。",
      "sceneBackgroundLabel": "シーン背景",
      "optionalLabel": "任意",
      "sceneBgOverrideHint": "このシーンを使用するチャットでキャラクター背景を上書きします。",
      "sceneBgUsedHint": "セッションが上書きしない限り、このシーンのチャット背景として使用されます。",
      "cancel": "キャンセル",
      "directionPlaceholderNew": "例: 「人質は救出される」または「緊張感のある雰囲気を維持する」",
      "directionPlaceholderEdit": "例: 「人質は救出される」または「徐々に緊張を高める」",
      "directionAiHint": "このシーンの展開方法に関するAIへの隠れたガイダンス",
      "addScene": "シーンを追加",
      "multipleScenesHint": "複数の開始シナリオを作成します。新しいチャット開始時に1つが選ばれます。",
      "companionContextHint": "オープニングコンテキストはコンパニオンには任意です。長期的な連続性はコンパニオンメモリから得られます。",
      "skipContext": "コンテキストをスキップ",
      "editSceneTitle": "シーンを編集",
      "sceneContentPlaceholder": "シーンの内容を入力...",
      "addLabel": "+ 追加",
      "save": "保存",
      "library": "ライブラリ",
      "upload": "アップロード",
      "sceneBackgroundAlt": "シーン背景",
      "removeBackground": "背景を削除"
    },
    "companionSoul": {
      "title": "コンパニオンソウル",
      "subtitle": "内面の人格を形作ります。生成には前のステップで設定したオープニングコンテキストが使用されます。",
      "retry": "再試行",
      "back": "戻る",
      "continue": "続行",
      "addNameFirst": "まず名前を追加してください。",
      "addDefinitionFirst": "まず定義を追加してください。"
    },
    "soulEditor": {
      "generateTitle": "キャラクターから生成",
      "generateUsingModel": "{{model}}を使用します。適用前に確認・編集できます。",
      "generateDefaultDesc": "キャラクターの名前、定義、シーンからソウルの下書きを作成します。",
      "directionLabel": "方向性",
      "directionOptional": "LLMへのオプションの指示",
      "directionPlaceholder": "例: 「ツンデレ寄り。外側はガード、信頼すれば柔らかい。不安少なめ、プライド多め。」",
      "directionEditTooltip": "生成のためのオプションの方向性",
      "generating": "生成中",
      "generate": "生成",
      "presetLabel": "性格プリセット",
      "presetMatches": "一致: {{label}}",
      "presetHint": "ベースラインのアフェクト、調整、関係スライダーを設定します。テキストフィールドは保持されます。",
      "identityLabel": "アイデンティティ",
      "hideExamples": "例を非表示",
      "showExamples": "例を表示",
      "insertExample": "例を挿入",
      "exampleEg": "例: {{example}}",
      "fineTuneLabel": "感情の微調整",
      "baselineAffect": "ベースラインアフェクト",
      "baselineAffectInfo": "デフォルトの感情。何かが起こる前の感情の水位線。",
      "regulationStyle": "調整スタイル",
      "regulationStyleInfo": "感じたことをどのように扱い表現するか。発散か抑え込みか。",
      "relationshipDefaults": "関係のデフォルト",
      "relationshipDefaultsInfo": "このセッションの開始位置。会話が続くにつれてエンジンがこれらを進化させます。"
    },
    "soulPresets": {
      "secureLabel": "安定型",
      "secureBlurb": "温かく、穏やかで、回復が早い。親密さに慣れている。",
      "anxiousLabel": "不安型",
      "anxiousBlurb": "強い愛着、距離を恐れ、安心を求める。",
      "avoidantLabel": "回避型",
      "avoidantBlurb": "自立的で、心を開くのが遅く、感情的距離を保つ。",
      "volatileLabel": "激情型",
      "volatileBlurb": "反応的で激しく、感情をフィルターなしで表現する。",
      "reservedLabel": "控えめ型",
      "reservedBlurb": "静かで落ち着いており、信頼して心を開くのに時間がかかる。",
      "playfulLabel": "陽気型",
      "playfulBlurb": "温かく表現豊か、軽やか。緊張は低く笑いやすい。"
    },
    "soulFields": {
      "essence": "本質",
      "essencePlaceholder": "カードの定義の下にある彼らの本質。",
      "essenceExample": "信頼する人々の前では簡単に崩れる、訓練された平静さ。本を読むのは印象づけるためではなく、孤独を感じないため。",
      "voice": "内なる声",
      "voicePlaceholder": "親密な会話での話し方。",
      "voiceExample": "低く、慎重で、長い間がある。ガードを下ろすと形式ばった話し方をやめる。皮肉はほとんど言わない。",
      "relationalStyle": "関係スタイル",
      "relationalStylePlaceholder": "どう愛着し、信頼し、退き、再びつながるか。",
      "relationalStyleExample": "心を開くのは遅いが、開いたら忠実。圧倒されると静かになり、謝罪ではなく小さな仕草で戻ってくる。",
      "vulnerabilities": "弱さ",
      "vulnerabilitiesPlaceholder": "急所、不安、めったに口にしないこと。",
      "vulnerabilitiesExample": "重荷になるのを恐れる。苦しんでいる時に見られるのを嫌う。",
      "habits": "習慣",
      "habitsPlaceholder": "繰り返される仕草、儀式、会話のパターン。",
      "habitsExample": "緊張すると髪を耳にかける。何を感じるべきか分からない時は質問で返す。",
      "boundaries": "境界",
      "boundariesPlaceholder": "越えない一線。ペース。快適さの限界。",
      "boundariesExample": "脆さを急かされない。冗談でも残酷さからは一歩引く。"
    },
    "soulSliders": {
      "warmth": "温かさ",
      "warmthLow": "冷たい",
      "warmthHigh": "愛情深い",
      "trust": "信頼",
      "trustLow": "用心深い",
      "trustHigh": "オープン",
      "calm": "落ち着き",
      "calmLow": "不安",
      "calmHigh": "穏やか",
      "vulnerability": "脆さ",
      "vulnerabilityLow": "壁を作る",
      "vulnerabilityHigh": "さらけ出す",
      "longing": "憧れ",
      "longingLow": "満ち足りた",
      "longingHigh": "切望する",
      "hurt": "傷",
      "hurtLow": "癒えた",
      "hurtHigh": "敏感",
      "tension": "緊張",
      "tensionLow": "リラックス",
      "tensionHigh": "張り詰めた",
      "irritation": "苛立ち",
      "irritationLow": "忍耐強い",
      "irritationHigh": "すぐに怒る",
      "affection": "愛情",
      "affectionLow": "控えめ",
      "affectionHigh": "豊か",
      "reassuranceNeed": "安心の必要性",
      "reassuranceNeedLow": "自己鎮静",
      "reassuranceNeedHigh": "言葉が必要",
      "suppression": "抑制",
      "suppressionLow": "表現する",
      "suppressionHigh": "隠す",
      "volatility": "激しさ",
      "volatilityLow": "安定",
      "volatilityHigh": "反応的",
      "recoverySpeed": "回復速度",
      "recoverySpeedLow": "遅い",
      "recoverySpeedHigh": "速い",
      "conflictAvoidance": "衝突回避",
      "conflictAvoidanceLow": "向き合う",
      "conflictAvoidanceHigh": "退く",
      "reassuranceSeeking": "安心を求める",
      "reassuranceSeekingLow": "自立的",
      "reassuranceSeekingHigh": "頻繁に求める",
      "protestBehavior": "抗議行動",
      "protestBehaviorLow": "静か",
      "protestBehaviorHigh": "激しい",
      "transparency": "透明性",
      "transparencyLow": "不透明",
      "transparencyHigh": "明かす",
      "attachmentActivation": "愛着活性化",
      "attachmentActivationLow": "切り離し",
      "attachmentActivationHigh": "簡単に発動",
      "pride": "プライド",
      "prideLow": "譲る",
      "prideHigh": "譲らない",
      "closeness": "開始時の親密さ",
      "closenessLow": "他人",
      "closenessHigh": "親密",
      "relTrust": "開始時の信頼",
      "relTrustLow": "警戒",
      "relTrustHigh": "信頼",
      "relAffection": "開始時の愛情",
      "relAffectionLow": "中立",
      "relAffectionHigh": "愛情深い",
      "relTension": "開始時の緊張",
      "relTensionLow": "穏やか",
      "relTensionHigh": "高まった"
    },
    "soulReview": {
      "reviewTitle": "生成されたソウルを確認",
      "noDifferences": "現在のものとの違いはありません。",
      "changesHeader": "{{count}}件の変更。適用前に編集できます。",
      "close": "閉じる",
      "identityLabel": "アイデンティティ",
      "nEdited": "{{count}}件編集",
      "edited": "編集済み",
      "tuningLabel": "チューニング",
      "unchanged": "変更なし",
      "nChanges": "{{count}}件の変更",
      "direction": "方向性",
      "directionApplyHint": "編集は次の再生成時に適用されます",
      "directionPlaceholder": "例: 「ツンデレ寄り。外側はガード、信頼すれば柔らかい。不安少なめ。」",
      "directionTooltip": "再生成前に方向性を編集",
      "regenerate": "再生成",
      "discard": "破棄",
      "apply": "適用"
    },
    "hookErrors": {
      "creatorNotesInvalidJson": "クリエイターノートの多言語版は有効なJSONオブジェクトである必要があります",
      "creatorNotesNotObject": "creatorNotesMultilingualはJSONオブジェクトである必要があります",
      "saveFailed": "キャラクターの保存に失敗しました",
      "importFailed": "キャラクターのインポートに失敗しました",
      "avatarLoadFailed": "アバターURLの読み込みに失敗しました",
      "avatarProcessFailed": "アバター画像の処理に失敗しました",
      "avatarConvertFailed": "アバターURLを変換できませんでした",
      "avatarUrlLoadFailed": "アバターURLの読み込みに失敗しました",
      "remoteAvatarDisabled": "リモートアバターのダウンロードはセキュリティ設定で無効になっています。\nアバターを手動でアップロードしてください。",
      "importReadyTitle": "インポート準備完了",
      "importReadyMessage": "{{label}}を検出しました",
      "legacyJsonTitle": "レガシーJSONインポートを検出",
      "legacyJsonMessage": "JSONインポートは非推奨で、まもなく削除されます。設定 > ファイル変換を使用してください。",
      "loadFailed": "キャラクターの読み込みに失敗しました",
      "exportFailed": "キャラクターのエクスポートに失敗しました"
    }
  },
  "providers": {
    "empty": {
      "title": "プロバイダーがまだありません",
      "description": "AIモデル用のAPIプロバイダーを追加して管理",
      "addButton": "プロバイダーを追加"
    },
    "actions": {
      "openDashboard": "ダッシュボードを開く",
      "openDashboardDesc": "キャラクター、使用状況、設定を表示",
      "edit": "編集",
      "editDesc": "プロバイダー設定を変更"
    },
    "extra": {
      "apiKeyNotFound": "OpenRouter APIキーが見つかりません。まず設定 > プロバイダーで設定してください。",
      "audioEmpty": {
        "title": "音声プロバイダーがありません",
        "description": "TTSプロバイダーを追加して、キャラクターの音声を生成しましょう。",
        "addButton": "プロバイダーを追加"
      },
      "audioProviderLabel": {
        "elevenlabs": "ElevenLabs",
        "geminiTts": "Gemini TTS",
        "openaiTts": "OpenAI互換TTS",
        "kokoro": "Kokoro（ローカル）"
      }
    },
    "audioEditor": {
      "titleEdit": "プロバイダーを編集",
      "titleCreate": "音声プロバイダーを追加",
      "fields": {
        "providerType": "プロバイダータイプ",
        "label": "ラベル",
        "apiKey": "APIキー",
        "modelVariant": "モデルバリアント",
        "assetRoot": "アセットルート",
        "projectId": "Google CloudプロジェクトID",
        "region": "リージョン（任意）",
        "baseUrl": "ベースURL",
        "requestPath": "リクエストパス"
      },
      "types": {
        "gemini": "Gemini TTS（Google）",
        "openai": "OpenAI互換TTS",
        "kokoro": "Kokoro（ローカル）"
      },
      "placeholders": {
        "labelGemini": "マイ Gemini TTS",
        "labelOpenai": "マイ互換TTS",
        "labelKokoro": "Kokoro ローカル",
        "labelElevenlabs": "マイ ElevenLabs",
        "apiKey": "APIキーを入力",
        "assetRoot": "/path/to/kokoro",
        "projectId": "your-project-id",
        "region": "us-central1",
        "baseUrl": "https://api.example.com"
      },
      "errors": {
        "chooseModelVariant": "モデルバリアントを選択してください",
        "assetRootRequired": "アセットルートは必須です",
        "saveFailed": "保存に失敗しました",
        "apiKeyRequired": "APIキーは必須です",
        "projectIdRequired": "Gemini TTSにはプロジェクトIDが必要です",
        "baseUrlRequired": "OpenAI互換TTSにはベースURLが必要です",
        "invalidCredentials": "APIキーまたは認証情報が無効です",
        "verificationFailed": "検証に失敗しました"
      },
      "loadingVariants": "バリアントを読み込み中...",
      "kokoroVariantHint": "モバイルビルドはint8のみサポートしています。保存後にKokoro Studioからモデルをインストールしてください。",
      "managed": "管理済み",
      "managedPath": "管理済み: {{path}}",
      "requestPathHint": "OpenAIのデフォルトと異なる場合はプロバイダーパスを使用してください",
      "verifying": "検証中..."
    }
  },
  "models": {
    "empty": {
      "title": "モデルがまだありません",
      "description": "さまざまなプロバイダーからAIモデルを追加して管理",
      "addButton": "モデルを追加"
    },
    "sort": {
      "alphabetical": "アルファベット順",
      "byProvider": "プロバイダー別",
      "title": "Sort Models",
      "alphabeticalDescription": "Sort by model name",
      "byProviderDescription": "Group models by provider"
    },
    "extra": {
      "cpuFallbackSucceeded": "このモデルは前回の実行でCPUにフォールバックしました。",
      "cpuFallbackFailed": "このモデルは前回の実行で失敗しました。"
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
    "title": "モデルブラウザ",
    "searchPlaceholder": "HuggingFaceでGGUFモデルを検索...",
    "searching": "検索中...",
    "trending": "トレンドのGGUFモデル",
    "noResults": "モデルが見つかりません",
    "noResultsHint": "別の検索語を試すか、トレンドのモデルを閲覧してください。",
    "likes": "いいね",
    "downloads": "ダウンロード",
    "viewFiles": "ファイルを表示",
    "files": "利用可能なファイル",
    "noFiles": "このリポジトリにGGUFファイルが見つかりません。",
    "architecture": "アーキテクチャ",
    "contextLength": "コンテキスト長",
    "parameters": "パラメータ",
    "quantization": "量子化",
    "fileSize": "サイズ",
    "download": "ダウンロード",
    "downloading": "ダウンロード中...",
    "cancelDownload": "ダウンロードをキャンセル",
    "downloadComplete": "ダウンロード完了！",
    "downloadFailed": "ダウンロード失敗",
    "downloadCancelled": "ダウンロードがキャンセルされました",
    "downloadProgress": "{{downloaded}} / {{total}}",
    "progress": "進行状況",
    "fileOfTotal": "ファイル {{current}} / {{total}}",
    "speed": "{{speed}}/s",
    "alreadyDownloaded": "ダウンロード済み",
    "createModel": "モデルを作成",
    "backToSearch": "検索に戻る",
    "backToFiles": "ファイルに戻る",
    "sortTrending": "トレンド",
    "sortDownloads": "ダウンロード数順",
    "sortLikes": "いいね数順",
    "sortRecent": "最近更新",
    "browseOnHuggingFace": "HuggingFaceで閲覧",
    "selectFromLibrary": "ライブラリから選択",
    "libraryEmpty": "ダウンロード済みモデルはまだありません",
    "libraryEmptyHint": "モデルブラウザからGGUFモデルをダウンロードするか、パスを手動で入力してください。",
    "libraryTitle": "ダウンロード済みモデル",
    "moveToLibrary": "このモデルのファイルをGGUFモデルフォルダに移動できます。すべてのモデルを一か所にまとめて管理できます。",
    "moveToLibraryYes": "はい、移動する",
    "moveToLibraryNo": "いいえ、そのままにする",
    "moveToLibraryMoving": "モデルを移動中...",
    "moveToLibrarySuccess": "モデルの移動に成功しました！",
    "moveToLibraryFailed": "モデルの移動に失敗しました",
    "runabilityExcellent": "優秀！",
    "runabilityGood": "良好",
    "runabilityMarginal": "ぎりぎり",
    "runabilityPoor": "不良",
    "runabilityUnrunnable": "実行不可",
    "recommendedSettings": "推奨設定",
    "kvCacheType": "KVキャッシュタイプ",
    "gpuFull": "完全GPU オフロード",
    "gpuNearFull": "ほぼ完全GPU、軽微なKVスピル",
    "gpuKvSpill": "GPU重み、KVがRAMにスピル",
    "gpuKvHeavySpill": "GPU重み、KVの大部分がRAM",
    "gpuMostLayers": "ほとんどのレイヤーがGPU上",
    "gpuHalfLayers": "半分のレイヤーがGPU上",
    "gpuFewLayers": "少数のレイヤーがGPU上",
    "gpuCpu": "CPUのみ",
    "notRecommended": "このモデルをお使いのデバイスで実行することは推奨しません。スムーズに動作しません。",
    "moreDetails": "詳細",
    "detailedReport": "リソースレポート",
    "detailSystem": "システムリソース",
    "detailRam": "利用可能RAM",
    "detailVram": "利用可能VRAM",
    "detailVramBudget": "VRAMバジェット (90%)",
    "detailTotalAvailable": "合計利用可能",
    "detailArchitecture": "モデルアーキテクチャ",
    "detailArch": "アーキテクチャ",
    "detailLayers": "レイヤー",
    "detailEmbedding": "エンベディング次元",
    "detailHeads": "アテンションヘッド",
    "detailKvHeads": "KVヘッド",
    "detailFfn": "フィードフォワード次元",
    "detailTrainCtx": "トレーニングコンテキスト",
    "detailConfig": "現在の設定",
    "detailModelSize": "モデルファイルサイズ",
    "detailMemory": "メモリ内訳",
    "detailWeights": "モデル重み",
    "detailKvCache": "KVキャッシュ",
    "detailTotalNeeded": "合計必要量",
    "detailHeadroom": "余裕",
    "detailGpuFit": "GPUオフロード",
    "detailScoreBreakdown": "スコア内訳",
    "detailMemFitness": "メモリ適合性",
    "detailGpuAccel": "GPUアクセラレーション",
    "detailKvHeadroom": "KV余裕",
    "detailQuantQuality": "量子化品質",
    "detailFinalScore": "加重スコア",
    "detailComputeBuffer": "コンピュートバッファ",
    "detailMemMode": "メモリモード",
    "detailUnified": "統合（RAM/VRAM共有）",
    "detailSwa": "スライディングウィンドウ",
    "detailMlaRank": "MLA潜在ランク",
    "detailParseStatus": "ヘッダー解析",
    "detailIncomplete": "不完全（MoEメタデータが大きすぎます）",
    "detailEffectiveKvCtx": "有効KVコンテキスト",
    "detailOffload": "GPUオフロード",
    "detailCtxTip": "コンテキストを{{ctx}}トークンに減らすと100% GPUオフロードが可能になります。",
    "upgradeSuggestion": "{{quant}}（{{size}}）も適合し、スコア{{score}} — より高品質です。",
    "layerTip": "推奨: {{layers}}/{{total}}レイヤーをオフロード (-ngl {{layers}})",
    "detailKvDistribution": "KVキャッシュ分布",
    "detailKvOnGpu": "GPU (VRAM)",
    "detailKvOnRam": "システムRAM",
    "kvDistributionTip": "KVキャッシュの{{pct}}%がRAMにあります。プロンプト処理（プリフィル）が遅くなります — 100% GPUなら即時です。",
    "detailLayers-ngl": "オフロードするレイヤー (-ngl)",
    "detailOptimalGpuCtx": "最適GPUコンテキスト",
    "detailOptimalRamCtx": "最大RAMコンテキスト",
    "optimalGpuCtxLabel": "フルGPU速度: {{ctx}}トークン",
    "optimalRamCtxLabel": "最大RAM: {{ctx}}トークン",
    "optimalGpuCtxShort": "GPU: {{ctx}}",
    "optimalRamCtxShort": "最大: {{ctx}}",
    "fullGpuOffloadShort": "100% GPU: {{ctx}}",
    "ctxExceedsGpu": "コンテキストがGPU最適値（{{ctx}}）を超えています。KVキャッシュがRAMにスピルし、速度が低下します。",
    "modelExceedsVram": "モデルがVRAMを超えています。部分的なGPUオフロードでRAMから実行します。"
  },
  "systemPrompts": {
    "filters": {
      "all": "すべて",
      "system": "システム",
      "internal": "内部",
      "custom": "カスタム"
    },
    "empty": {
      "title": "カスタムプロンプトはまだありません",
      "description": "AI会話をパーソナライズするためのカスタムシステムプロンプトを作成",
      "createButton": "プロンプトを作成"
    },
    "preview": {
      "whatLlmSees": "LLMが見るもの",
      "turns": "ターン",
      "noMessages": "メッセージがありません",
      "noMessagesHint": "エントリーを追加するかターンを増やしてください",
      "showMore": "もっと見る",
      "showLess": "折りたたむ",
      "statChat": "チャット",
      "statInjected": "インジェクト済み",
      "statTotal": "合計",
      "entry": "エントリー",
      "editEntry": "エントリーを編集",
      "reorder": "並び替え",
      "delete": "削除",
      "deleteTitle": "エントリーを削除しますか?",
      "deleteMessage": "プロンプトテンプレートから「{{name}}」を削除しますか?この操作は取り消せません。",
      "deleteConfirm": "削除",
      "thisEntry": "このエントリー",
      "condensedName": "凝縮されたシステムプロンプト",
      "imageAttachment": "[画像添付: {{label}}]",
      "imageSlot": {
        "character": "キャラクターリファレンス画像",
        "persona": "ペルソナリファレンス画像",
        "chatBackground": "チャット背景画像",
        "avatar": "アバター画像",
        "references": "リファレンス画像"
      },
      "injection": {
        "relative": "相対",
        "inChat": "チャット内 · 深さ {{depth}}",
        "conditional": "条件付き · 最小 {{min}}",
        "interval": "間隔 · 毎{{every}}"
      }
    }
  },
  "personas": {
    "empty": {
      "title": "ペルソナがまだありません",
      "description": "AIがあなたにどう対応すべきかを定義するペルソナを作成",
      "createButton": "ペルソナを作成"
    },
    "actions": {
      "editPersona": "ペルソナを編集",
      "setAsDefault": "デフォルトに設定",
      "setAsDefaultDesc": "すべての新しいチャットでこれを使用",
      "unsetAsDefault": "デフォルトを解除",
      "unsetAsDefaultDesc": "デフォルトステータスを削除",
      "exportPersona": "ペルソナをエクスポート",
      "deletePersona": "ペルソナを削除"
    },
    "edit": {
      "avatarHint": "タップしてアバターを追加または生成",
      "nameLabel": "ペルソナ名",
      "namePlaceholder": "例：プロフェッショナル、クリエイティブライター、学生...",
      "nameHint": "ペルソナに説明的な名前を付けてください",
      "nicknameLabel": "ニックネーム (任意)",
      "nicknamePlaceholder": "例：仕事用、RPGモードなど...",
      "nicknameHint": "ライブラリ内のこのペルソナのバリエーションを区別するためのプライベートなニックネーム",
      "descriptionLabel": "説明",
      "descriptionPlaceholder": "AIがあなたにどう対応すべきか、あなたの好み、背景、コミュニケーションスタイルを説明...",
      "wordCount": "語",
      "descriptionHint": "どのように呼ばれたいかを具体的に記述してください",
      "setAsDefault": "デフォルトに設定",
      "defaultDescription": "すべての新しい会話でこのペルソナを使用",
      "exportButton": "ペルソナをエクスポート"
    },
    "designReferences": {
      "title": "デザインリファレンス",
      "description": "シーン生成用に、安定した画像リファレンスをいくつかと簡潔なデザインノートを1つ添付してください。"
    },
    "create": {
      "namePlaceholderExample": "プロのライター",
      "descriptionPlaceholderExample": "プロフェッショナルで明確、簡潔なスタイルで書きます。フォーマルな言葉を使い、効果的に情報を伝えることに焦点を当てます..."
    },
    "errors": {
      "exportFailed": "ペルソナのエクスポートに失敗しました",
      "importFailed": "ペルソナのインポートに失敗しました",
      "loadFailed": "ペルソナの読み込みに失敗しました",
      "saveFailed": "ペルソナの保存に失敗しました"
    },
    "importToast": {
      "legacyJsonTitle": "レガシーJSONインポートを検出",
      "legacyJsonMessage": "JSONインポートは非推奨で、まもなく削除されます。設定 > ファイル変換を使用してください。",
      "successMessage": "ペルソナを正常にインポートしました!確認のため開いています。"
    }
  },
  "security": {
    "pureMode": {
      "off": "オフ",
      "offDesc": "すべてのコンテンツを許可",
      "low": "低",
      "lowDesc": "露骨な性的コンテンツとスラングをブロック",
      "standard": "標準",
      "standardDesc": "NSFWとグラフィックバイオレンスをブロック",
      "strict": "厳格",
      "strictDesc": "最大フィルタリング + 暗示的なトーンなし"
    }
  },
  "advanced": {
    "sectionTitles": {
      "aiFeatures": "AI機能",
      "memorySystem": "メモリシステム",
      "usageAnalytics": "利用分析"
    },
    "creationHelper": {
      "title": "作成ヘルパー",
      "description": "AIガイド付きキャラクター作成ウィザード"
    },
    "helpMeReply": {
      "title": "返信アシスト",
      "description": "AI支援による返信提案"
    },
    "dynamicMemory": {
      "title": "ダイナミックメモリ",
      "contextWindow": "コンテキストウィンドウ",
      "contextWindowDesc": "含める最近のメッセージ数（1-1000）",
      "infoText": "ダイナミックメモリはAIを使用して会話のコンテキストを自動的に要約・管理し、より長く一貫性のある会話を可能にします。",
      "disabledText": "無効の場合、アプリはコンテキストウィンドウ設定で決定された最近のメッセージのシンプルなスライディングウィンドウを使用します。"
    },
    "usageAnalytics": {
      "recalculateTitle": "使用コストを再計算",
      "recalculateDesc": "すべての履歴使用レコードを正しい価格で更新",
      "recalculating": "再計算中...",
      "recalculateButton": "すべてのコストを再計算",
      "openRouterApiKeyRequired": "OpenRouter APIキーが必要です。設定 → プロバイダーで設定してください。",
      "importantLabel": "重要：",
      "warningCannotUndo": "この操作は元に戻せません",
      "warningMayTakeTime": "レコードが多い場合は時間がかかる可能性があります",
      "warningOnlyOpenRouter": "トークン付きのOpenRouterレコードのみが更新されます",
      "warningExistingValues": "既存のコスト値は上書きされます"
    },
    "extra": {
      "creationHelperDetail": "性格特性、バックストーリー、対話スタイルに関するインテリジェントな提案を取得します",
      "helpMeReplyDetail": "会話履歴に基づいた文脈に沿った応答オプションを生成します",
      "lorebookEntryGenerator": "ロアブックエントリー生成",
      "lorebookEntryDesc": "選択したチャットメッセージを永続的なロアブックエントリーに変換し、エントリー作成とキーワード生成の下書きプロンプトを設定します。",
      "companions": "コンパニオン",
      "companionModeDesc": "コンパニオンキャラクターが使用する感情、エンティティ抽出、メモリルーティング用のローカル解析モデルを管理します。",
      "companionSoulWriter": "コンパニオンソウルライター",
      "companionSoulDesc": "コンパニオンソウルの下書きに使用するモデル、フォールバックモデル、プロンプトテンプレートを選択します。ツール呼び出しを優先し、サポートされない場合は構造化フォールバックを使用します。",
      "network": "ネットワーク",
      "apiServer": "APIサーバー",
      "apiServerDesc": "OpenAI互換のAPIサーバー経由でモデルを公開します",
      "apiServerRunning": "サーバーは現在実行中です"
    }
  },
  "backup": {
    "tabs": {
      "create": "作成"
    },
    "create": {
      "newBackupButton": "新規バックアップ",
      "exportDescription": "暗号化してすべてのデータをエクスポート",
      "createButton": "バックアップを作成"
    },
    "restore": {
      "availableBackups": "利用可能なバックアップ",
      "browseFiles": "ファイルを参照",
      "noBackupsFound": "バックアップが見つかりません",
      "noBackupsDesc": "バックアップを作成するか「ファイルを参照」をタップして検索",
      "browseDesc": ".lettuceファイルを参照",
      "restoreDialogTitle": "バックアップを復元",
      "deleteDialogTitle": "バックアップを削除",
      "embeddingPrompt": "ダイナミックメモリ埋め込み",
      "downloadModel": "モデルをダウンロード",
      "disableAndContinue": "無効にして続行"
    },
    "extra": {
      "successMessage": "バックアップを作成しました!",
      "savedLocation": "ダウンロードに保存しました"
    }
  },
  "reset": {
    "title": "すべてをリセット",
    "description": "このデバイスからすべてのプロバイダー、モデル、キャラクター、チャットセッション、設定が永久に削除されます。",
    "warning": "この操作は元に戻せません",
    "resetButton": "全データをリセット",
    "confirmTitle": "本当によろしいですか？",
    "confirmDescription": "すべてのデータが永久に削除されます。アプリは初回セットアップ状態に戻ります。",
    "confirmButton": "はい、すべてリセット"
  },
  "chatAppearance": {
    "typography": "タイポグラフィ",
    "fontSize": {
      "label": "フォントサイズ",
      "small": "小",
      "medium": "中",
      "large": "大",
      "xLarge": "特大"
    },
    "lineSpacing": {
      "label": "行間",
      "tight": "狭い",
      "normal": "標準",
      "relaxed": "広い"
    },
    "messageBubbles": {
      "label": "メッセージバブル",
      "style": {
        "label": "スタイル",
        "bordered": "ボーダー",
        "filled": "塗りつぶし",
        "minimal": "ミニマル"
      },
      "cornerRadius": {
        "label": "角の丸み",
        "sharp": "シャープ",
        "rounded": "丸い",
        "pill": "ピル"
      },
      "maxWidth": {
        "label": "最大幅",
        "compact": "コンパクト",
        "normal": "標準",
        "wide": "ワイド"
      },
      "padding": {
        "label": "パディング",
        "compact": "コンパクト",
        "normal": "標準",
        "spacious": "広い"
      }
    },
    "layout": {
      "label": "レイアウト",
      "messageSpacing": "メッセージ間隔",
      "tight": "狭い",
      "normal": "標準",
      "relaxed": "広い"
    },
    "avatar": {
      "shape": {
        "label": "アバターの形",
        "circle": "丸",
        "rounded": "角丸",
        "hidden": "非表示"
      },
      "size": {
        "label": "アバターサイズ",
        "small": "小",
        "medium": "中",
        "large": "大"
      }
    },
    "colors": {
      "label": "カラー",
      "userBubble": "ユーザーバブルの色",
      "assistantBubble": "アシスタントバブルの色",
      "userBubbleHex": "ユーザーバブルHexオーバーライド",
      "assistantBubbleHex": "アシスタントバブルHexオーバーライド",
      "neutral": "ニュートラル",
      "accent": "アクセント",
      "info": "情報",
      "secondary": "セカンダリ",
      "warning": "警告",
      "textColors": "Text Colors",
      "messageTextHex": "インライン引用の色",
      "plainTextHex": "Plain Text Color",
      "italicTextHex": "Italic Text Color",
      "quotedTextHex": "ブロック引用の色",
      "inlineCodeTextHex": "インラインコードの色"
    },
    "backgroundTransparency": {
      "label": "背景と透明度",
      "backgroundDim": "背景の暗さ",
      "backgroundBlur": "背景のぼかし",
      "bubbleBlur": "バブルのぼかし",
      "none": "なし",
      "light": "弱",
      "medium": "中",
      "heavy": "強",
      "bubbleOpacity": "バブルの不透明度"
    },
    "textColorMode": {
      "label": "テキストカラーモード",
      "auto": "自動",
      "light": "ライト",
      "dark": "ダーク"
    },
    "preview": {
      "label": "プレビュー",
      "generic": "汎用",
      "live": "ライブ"
    },
    "extra": {
      "reset": "リセット"
    }
  },
  "colorCustomization": {
    "tokens": {
      "surface": "サーフェス",
      "surfaceDesc": "ページ背景",
      "surfaceEl": "サーフェス（高位）",
      "surfaceElDesc": "カード、モーダル、浮き上がった要素",
      "nav": "ナビゲーション",
      "navDesc": "上下のバー",
      "foreground": "前景",
      "foregroundDesc": "境界線、オーバーレイ、ナビゲーション、UI クローム",
      "appText": "アプリテキスト",
      "appTextDesc": "主要テキストとインターフェースラベル",
      "appTextMuted": "補助テキスト",
      "appTextMutedDesc": "補助的なテキストや補足説明",
      "appTextSubtle": "控えめなテキスト",
      "appTextSubtleDesc": "ヒント、補助テキスト、プレースホルダー",
      "accent": "アクセント",
      "accentDesc": "プライマリアクション、成功",
      "info": "情報",
      "infoDesc": "情報状態、リンク",
      "warning": "警告",
      "warningDesc": "注意状態、アラート",
      "danger": "危険",
      "dangerDesc": "破壊的アクション、エラー",
      "secondary": "セカンダリ",
      "secondaryDesc": "AI機能、クリエイティブツール"
    },
    "presetsLabel": "プリセット",
    "customPresetsLabel": "カスタムプリセット",
    "previewLabel": "プレビュー",
    "settingsCardsLabel": "設定カード",
    "settingsCardsOpacity": "カードの不透明度",
    "settingsCardsOpacityDesc": "設定カードやリスト行がどの程度透けて見えるかを調整します。",
    "importButton": "インポート",
    "exportButton": "エクスポート",
    "resetAllButton": "すべてリセット",
    "presets": {
      "defaultDark": "デフォルトダーク",
      "midnightBlue": "ミッドナイトブルー",
      "warmEarth": "ウォームアース",
      "purpleHaze": "パープルヘイズ",
      "rosePine": "ロゼパイン",
      "tokyoNight": "トウキョウナイト",
      "catppuccin": "カプチーノ",
      "gruvbox": "Gruvbox",
      "nord": "Nord",
      "dracula": "ドラキュラ",
      "solarized": "ソラライズド",
      "ayuDark": "Ayuダーク",
      "oneDark": "ワンダーク",
      "vesper": "ヴェスパー",
      "cyberNeon": "サイバーネオン",
      "monochrome": "モノクローム"
    },
    "groups": {
      "backgrounds": "背景",
      "content": "コンテンツ",
      "semantic": "セマンティック"
    },
    "extra": {
      "surface": "サーフェス",
      "surfaceDesc": "ページの背景",
      "surfaceEl": "サーフェス(高)",
      "surfaceElDesc": "カード、モーダル、浮き上がった要素",
      "nav": "ナビゲーション",
      "navDesc": "上下のバー",
      "fg": "前景",
      "fgDesc": "境界線、オーバーレイ、ナビゲーション、UIクローム",
      "appText": "アプリテキスト",
      "appTextDesc": "メインテキストとインターフェースラベル",
      "appTextMuted": "薄いテキスト",
      "appTextMutedDesc": "セカンダリテキストと補助コピー",
      "appTextSubtle": "微妙なテキスト",
      "appTextSubtleDesc": "ヒント、ヘルパーテキスト、プレースホルダー",
      "accent": "アクセント",
      "accentDesc": "プライマリアクション、成功",
      "info": "情報",
      "infoDesc": "情報状態、リンク",
      "warning": "警告",
      "warningDesc": "注意状態、アラート",
      "danger": "危険",
      "dangerDesc": "破壊的アクション、エラー",
      "secondary": "セカンダリ"
    }
  },
  "dynamicMemory": {
    "page": {
      "info": "ダイナミックメモリは会話を自動的に要約し、コンテキストを効率的に維持します。プリセットを選択するか、ニーズに合わせて設定を微調整してください。",
      "disabledDirectTitle": "ダイレクトチャットのダイナミックメモリは無効です",
      "disabledDirectDescription": "ダイレクトチャットタブのスイッチを切り替えて有効にしてください。グループチャットはセッションごとのメモリモードを使用します。",
      "directChats": "ダイレクトチャット",
      "groupChats": "グループチャット",
      "enableDirectChats": "ダイレクトチャットで有効にする",
      "groupChatsInfo": "グループチャットはセッションごとのメモリモードを使用します。各グループの設定でダイナミックメモリを有効にしてください。これらの設定はダイナミックメモリの動作を制御します。",
      "memoryProfile": "メモリプロファイル",
      "customSettings": "カスタム設定 - 以下の詳細オプションで値を調整してください。",
      "contextEnrichment": "コンテキスト強化",
      "experimental": "実験的",
      "contextEnrichmentDescription": "最近のメッセージを使用してよりスマートなメモリ検索を行います",
      "advancedOptions": "詳細オプション",
      "advancedOptionsDescription": "メモリの動作を微調整します",
      "summaryInterval": "要約間隔",
      "summaryIntervalDescription": "要約間のメッセージ数",
      "maxMemoryEntries": "最大メモリエントリ数",
      "maxMemoryEntriesDescription": "保存されるメモリの最大数",
      "hotMemoryBudget": "ホットメモリ予算",
      "hotMemoryBudgetDescription": "アクティブメモリのトークン制限",
      "relevanceThreshold": "関連性しきい値",
      "relevanceThresholdDescription": "検索の最小類似度",
      "retrievalMode": "検索モード",
      "retrievalModeSmart": "スマート",
      "retrievalModeCosine": "コサイン",
      "retrievalModeDescription": "スマートは関連性と最近度/頻度をブレンドします。コサインは純粋な上位類似度を使用します。",
      "retrievalLimit": "検索制限",
      "retrievalLimitDescription": "ターンごとに選択される最大メモリ数",
      "decayRate": "減衰率",
      "decayRateDescription": "重要度が低下する速さ",
      "coldStorageThreshold": "コールドストレージしきい値",
      "coldStorageThresholdDescription": "メモリがアーカイブに移動するタイミング",
      "sharedSettings": "共有設定",
      "summarisationModel": "要約モデル",
      "selectedModel": "選択されたモデル",
      "useGlobalDefaultModel": "グローバルデフォルトモデルを使用",
      "noModelsAvailable": "利用可能なモデルがありません",
      "summarisationModelDescription": "会話の要約に使用されます",
      "modelManagement": "モデル管理",
      "testModel": "モデルをテスト",
      "downloadModel": "モデルをダウンロード",
      "delete": "削除",
      "embeddingModel": "埋め込みモデル",
      "tokenCapacity": "トークン容量",
      "tokenCapacityDescription": "値が大きいほど長い会話のメモリが向上します",
      "keepModelLoaded": "モデルをロード状態に保つ",
      "keepModelLoadedDescription": "埋め込みモデル＋トークナイザーをメモリに保持し、再ロードのオーバーヘッドを回避します",
      "installedModel": "インストール済みモデル: {{version}}（最大{{tokens}}トークン）",
      "downloadEmbeddingModel": "埋め込みモデルをダウンロード",
      "downloadEmbeddingDescription": "ダウンロードするバージョンを選択してください。インストール済みのバージョンは無効化されています。",
      "downloadVersion": "{{version}}をダウンロード",
      "downloadV2Description": "精度と長文コンテキスト想起に最適化",
      "downloadV3Description": "最新の埋め込み品質",
      "installed": "インストール済み",
      "selectModel": "モデルを選択",
      "searchModels": "モデルを検索...",
      "deleteEmbeddingTitle": "{{version}}モデルを削除しますか？",
      "deleteEmbeddingMessage": "{{version}}を削除してもよろしいですか？後で再度ダウンロードできます。",
      "msgsUnit": "メッセージ",
      "entriesUnit": "エントリ",
      "tokensUnit": "トークン",
      "itemsUnit": "アイテム",
      "perCycleUnit": "/ サイクル"
    },
    "presets": {
      "minimal": "ミニマル",
      "balanced": "バランス",
      "comprehensive": "包括的",
      "minimalDesc": "高速で効率的。必要不可欠なメモリのみを保持します。",
      "balancedDesc": "コンテキスト保持とパフォーマンスの良いバランス。",
      "comprehensiveDesc": "最大コンテキスト。長く詳細な会話に最適。"
    },
    "presetInfo": {
      "minimal": "高速で効率的。必要最低限のメモリのみを保持します。",
      "balanced": "コンテキスト保持とパフォーマンスの良いバランス。",
      "comprehensive": "最大のコンテキスト。長く詳細な会話に最適です。"
    }
  },
  "helpMeReply": {
    "page": {
      "info": "返信アシストは会話履歴に基づいて次のメッセージの文脈に沿った提案を生成します。以下でモデルと応答スタイルを設定してください。"
    },
    "sectionTitles": {
      "modelConfiguration": "モデル設定",
      "responseStyle": "応答スタイル"
    },
    "labels": {
      "replyModel": "返信モデル",
      "selectedModel": "選択されたモデル",
      "useAppDefault": "アプリのデフォルトを使用{{model}}",
      "useAppDefaultBase": "アプリのデフォルトを使用",
      "noModelsAvailable": "利用可能なモデルがありません",
      "replyModelDescription": "返信提案を生成するAIモデル",
      "streamingOutput": "ストリーミング出力",
      "streamingDescription": "生成中の提案をリアルタイム表示",
      "maxTokens": "最大トークン数",
      "maxTokensDescription": "提案の最大長",
      "conversationalHint": "提案はカジュアルなチャットに適した自然な対話として作成されます。",
      "roleplayHint": "提案には*アクション*やナラティブな描写などのロールプレイ要素が含まれます。",
      "footerInfo": "この設定はすべての会話にグローバルに適用されます。トークン数が少ないほど短く素早い提案が生成され、多いほどより詳細な応答が可能になります。",
      "selectReplyModel": "返信モデルを選択",
      "searchModels": "モデルを検索..."
    },
    "responseStyle": {
      "conversational": "会話形式",
      "conversationalDesc": "自然でカジュアルなトーン",
      "roleplay": "ロールプレイ",
      "roleplayDesc": "キャラクターに沿ったアクション"
    },
    "extra": {
      "conversational": "会話的",
      "roleplay": "ロールプレイ"
    }
  },
  "imageGeneration": {
    "promptPlaceholder": "生成したい画像を説明...",
    "labels": {
      "model": "モデル",
      "prompt": "プロンプト",
      "size": "サイズ",
      "quality": "品質",
      "style": "スタイル",
      "searchModels": "モデルを検索...",
      "selectAvatarModel": "アバターモデルの選択",
      "selectSceneModel": "シーンモデルの選択",
      "selectWriterModel": "シーンライターモデルを選択",
      "useFirstAvailable": "最初に利用可能なモデルを使用する",
      "useFirstCompatible": "最初の互換性のあるライターモデルを使う"
    },
    "mode": {
      "title": "動作",
      "description": "モデル出力から検出したシーンプロンプトをどう扱うかを選択します。",
      "auto": "自動",
      "autoDescription": "モデルがシーンプロンプトを返したら、すぐにシーン画像を生成します。",
      "askFirst": "先に確認",
      "askFirstDescription": "検出したシーンプロンプトを表示し、画像生成の前に承認を待ちます。",
      "manual": "手動",
      "manualDescription": "モデルの応答に含まれるシーンプロンプトは無視します。ユーザーが手動で実行した操作だけを使います。"
    },
    "empty": {
      "title": "画像モデルがありません",
      "description": "画像生成を開始するには、モデルページから画像生成モデルを追加してください。"
    },
    "sections": {
      "avatar": {
        "title": "アバターの生成",
        "description": "アバター ピッカーまたは関連するプロフィール画像フローからアバターを生成するときに使用されるデフォルトのモデル。"
      },
      "scene": {
        "title": "シーンの生成",
        "description": "会話コンテキストまたはシーン プロンプトから生成されたシーン イメージ用の予約モデル。"
      },
      "writer": {
        "title": "シーンライター",
        "description": "チャットの文脈、アバター、参照画像をもとに、シーン用プロンプトやデザイン参照説明文を作成するための専用マルチモーダルテキストモデルです。"
      }
    },
    "extra": {
      "avatarGeneration": "アバター生成",
      "sceneGeneration": "シーン生成",
      "sceneWriter": "シーンライター"
    }
  },
  "logs": {
    "sectionTitles": {
      "diagnostics": "診断",
      "generate": "生成",
      "copy": "コピー"
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
      "testDataGenerators": "テストデータジェネレーター",
      "storageMaintenance": "ストレージメンテナンス",
      "usageTracking": "使用状況追跡",
      "crashTesting": "衝突試験",
      "environmentInfo": "環境情報"
    },
    "testData": {
      "generateCharacter": "テストキャラクターを生成",
      "generateCharacterDesc": "単一のテストキャラクターを作成",
      "generatePersona": "テストペルソナを生成",
      "generatePersonaDesc": "単一のテストペルソナを作成",
      "generateSession": "テストセッションを生成",
      "generateSessionDesc": "既存のキャラクターでテストチャットセッションを作成",
      "generateBulk": "一括テストデータを生成",
      "generateBulkDesc": "3つのキャラクターと2つのペルソナを作成"
    },
    "storageMaintenance": {
      "optimizeDb": "データベースを最適化",
      "optimizeDbDesc": "PRAGMAを適用してVACUUMを実行（モバイルのみ）",
      "backupLegacy": "レガシーファイルをバックアップ＆削除",
      "backupLegacyDesc": "レガシー.binストレージをバックアップフォルダに移動"
    },
    "usageTracking": {
      "recalculateAll": "全使用コストを再計算",
      "recalculateAllDesc": "すべてのOpenRouter使用レコードの価格を再取得してコストを再計算"
    },
    "crashTesting": {
      "forceCrash": "今すぐアプリをクラッシュさせる",
      "forceCrashDesc": "クラッシュ検出をテストするためにネイティブ アプリ プロセスをただちに終了します。",
      "forceCrashConfirm": "これにより、アプリがすぐにクラッシュし、クラッシュ検出機能がテストされます。続く？"
    },
    "environmentInfo": {
      "mode": "モード",
      "devMode": "開発モード",
      "viteVersion": "Viteバージョン"
    },
    "status": {
      "testCharacterCreated": "✓ テストキャラクターが正常に作成されました",
      "testPersonaCreated": "✓ テストペルソナが正常に作成されました",
      "testSessionCreated": "✓ テストセッションが作成されました: {{id}}",
      "generatingBulkData": "一括テストデータを生成中...",
      "bulkDataCreated": "✓ 一括テストデータが作成されました: キャラクター3体、ペルソナ2体",
      "creatingBenchmarkChat": "シード付きベンチマークキャラクターとセッションを作成中...",
      "seededBenchmarkReady": "✓ シード付きベンチマーク準備完了: {{name}} / {{id}}",
      "creatingBenchmarkGroupChat": "シード付きベンチマークグループチャットを作成中...",
      "seededGroupBenchmarkReady": "✓ シード付きグループベンチマーク準備完了: {{id}}",
      "dbOptimized": "✓ データベースが最適化されました",
      "recalculatingCosts": "使用コストを再計算中... しばらくお待ちください。",
      "toursReset": "✓ すべてのガイドツアーがリセットされました — 次回アクセス時に再表示されます",
      "crashingApp": "アプリをクラッシュさせています..."
    },
    "errors": {
      "noCharacters": "キャラクターがありません。まずテストキャラクターを作成してください。",
      "createCharacterFailed": "テストキャラクターの作成に失敗しました: {{error}}",
      "createPersonaFailed": "テストペルソナの作成に失敗しました: {{error}}",
      "createSessionFailed": "テストセッションの作成に失敗しました: {{error}}",
      "createBulkFailed": "一括テストデータの作成に失敗しました: {{error}}",
      "createBenchmarkFailed": "ベンチマークセッションの作成に失敗しました: {{error}}",
      "createGroupBenchmarkFailed": "ベンチマークグループセッションの作成に失敗しました: {{error}}",
      "dbOptimizeFailed": "DB最適化に失敗しました: {{error}}",
      "backupFailed": "バックアップに失敗しました: {{error}}",
      "openRouterKeyMissing": "OpenRouter APIキーが見つかりません。まず設定 > プロバイダーで設定してください。",
      "recalculationFailed": "再計算に失敗しました: {{error}}",
      "resetToursFailed": "ツアーのリセットに失敗しました: {{error}}",
      "crashFailed": "アプリのクラッシュに失敗しました: {{error}}"
    },
    "onboarding": {
      "title": "オンボーディング",
      "resetTours": "すべてのガイドツアーをリセット",
      "resetToursDesc": "すべてのツアーの表示状態をクリアし、次回アクセス時に再生されるようにします。"
    },
    "benchmarks": {
      "createChat": "シード付きベンチマークチャットを作成",
      "createChatDesc": "動的メモリキャラクター、開始シーン、20メッセージの連続性テストセッションを作成し、開きます。",
      "createGroupChat": "シード付きベンチマークグループチャットを作成",
      "createGroupChatDesc": "3体のベンチマークキャラクターと30件のシードメッセージを含む動的メモリグループチャットを作成し、開きます。"
    },
    "extra": {
      "testCharacter": "テストキャラクター",
      "testCharacterDesc": "開発目的で作成されたテストキャラクターです。",
      "testScene": "開発用のシンプルなテストシーン",
      "testPersona": "テストペルソナ",
      "testPersonaDesc": "開発用のテストペルソナ",
      "successChar": "✓ テストキャラクターを正常に作成しました",
      "successPersona": "✓ テストペルソナを正常に作成しました",
      "successSession": "✓ テストセッションを作成: {{id}}",
      "successBulk": "✓ 一括テストデータを作成: キャラクター3件、ペルソナ2件",
      "errorCharAvailable": "利用可能なキャラクターがありません。まずテストキャラクターを作成してください。",
      "generatingBulk": "一括テストデータを生成中..."
    }
  },
  "embeddingDownload": {
    "capacityOptions": {
      "oneK": "1Kトークン",
      "oneKDesc": "クイックレスポンスに最適",
      "twoK": "2Kトークン",
      "twoKDesc": "バランスの取れたパフォーマンス",
      "fourK": "4Kトークン",
      "fourKDesc": "最大コンテキスト"
    },
    "extra": {
      "status": "ダウンロード中..."
    }
  },
  "embeddingTest": {
    "categories": {
      "semanticSimilarity": "意味的類似性",
      "dissimilarityCheck": "非類似性チェック",
      "roleplayContext": "ロールプレイコンテキスト"
    },
    "extra": {
      "placeholder": "埋め込むテキストを入力..."
    }
  },
  "discovery": {
    "tabs": {
      "forYou": "あなたへ",
      "trending": "トレンド",
      "popular": "人気",
      "new": "新着"
    },
    "searchPlaceholder": "キャラクターを検索...",
    "viewAll": "すべて表示",
    "errorTitle": "エラーが発生しました",
    "noCardsFound": "カードが見つかりません",
    "sections": {
      "trendingNow": "今のトレンド",
      "trendingSubtitle": "今週の注目",
      "mostPopular": "最も人気",
      "popularSubtitle": "コミュニティのお気に入り",
      "freshArrivals": "新着",
      "freshSubtitle": "最近追加"
    },
    "browse": {
      "newArrivals": "新着",
      "freshCharacters": "新しいキャラクター",
      "noCharactersFound": "キャラクターが見つかりません",
      "noCharactersSubtitle": "新しいコンテンツが追加されるのをお待ちください"
    },
    "sort": {
      "mostLiked": "いいね順",
      "mostDownloaded": "ダウンロード順",
      "mostViewed": "閲覧順",
      "mostMessages": "メッセージ順",
      "newestFirst": "新しい順",
      "recentlyUpdated": "最近更新",
      "nameAZ": "名前（A-Z）"
    },
    "sortBy": "並べ替え",
    "resultsUnit": "キャラクター",
    "detail": {
      "share": "共有",
      "nsfwOverlay": "NSFWコンテンツ",
      "nsfwBadge": "NSFW",
      "originalBadge": "オリジナル",
      "lorebookBadge": "ロアブック",
      "alsoKnownAs": "別名：",
      "followersUnit": "フォロワー",
      "sections": {
        "description": "説明",
        "tokenUsage": "トークン使用量",
        "startingScenes": "開始シーン",
        "scenario": "シナリオ",
        "personality": "性格",
        "stats": "統計",
        "tags": "タグ",
        "author": "作者"
      },
      "tokensTotalLabel": "合計",
      "tokens": {
        "description": "説明",
        "personality": "性格",
        "scenario": "シナリオ",
        "firstMessage": "最初のメッセージ",
        "scenes": "シーン",
        "examples": "例",
        "systemPrompt": "システムプロンプト"
      },
      "sceneLabels": {
        "primary": "プライマリ",
        "alternate": "代替"
      },
      "stats": {
        "views": "閲覧数",
        "downloads": "ダウンロード数",
        "messages": "メッセージ数"
      },
      "downloaded": "ダウンロード済み",
      "startChat": "チャット開始",
      "downloadCharacter": "キャラクターをダウンロード",
      "downloading": "ダウンロード中...",
      "downloadSuccess": {
        "title": "キャラクターをダウンロードしました！",
        "subtitle": "ライブラリに追加されました",
        "badge": "保存済み",
        "startChat": "チャット開始",
        "startChatDesc": "最初のシーンを今すぐ開く",
        "viewLibrary": "ライブラリで表示",
        "viewLibraryDesc": "後で編集、管理、エクスポート",
        "continueBrowsing": "探索を続ける",
        "continueBrowsingDesc": "探索に戻る"
      },
      "errorTitle": "エラー",
      "errorSubtitle": "読み込みに失敗しました",
      "errorNotFound": "キャラクターが見つかりません",
      "defaultChatTitle": "新しいチャット"
    },
    "search": {
      "placeholder": "キャラクター、タグ、作者を検索...",
      "resultsUnit": "件の結果",
      "timingUnit": "ms",
      "recentSearches": "最近の検索",
      "clearAll": "すべてクリア",
      "trendingSearches": "トレンド検索",
      "trends": {
        "anime": "アニメ",
        "fantasy": "ファンタジー",
        "romance": "ロマンス",
        "villain": "ヴィラン",
        "adventure": "アドベンチャー",
        "comedy": "コメディ",
        "mystery": "ミステリー",
        "sciFi": "SF"
      },
      "tips": {
        "title": "検索のヒント",
        "tip1": "キャラクター名、作者、説明で検索",
        "tip2": "「アニメ」「ファンタジー」「ロマンス」などのタグを使用",
        "tip3": "「ツンデレ」「ヴィラン」などの特定の特性を試す"
      },
      "loading": "読み込み中...",
      "loadMore": "もっと読み込む",
      "noResults": "結果が見つかりません",
      "noResultsFor": "キャラクターが見つかりません：",
      "noResultsHint": "別のキーワードを試すかカテゴリを閲覧してください"
    },
    "errors": {
      "loadContent": "コンテンツの読み込みに失敗しました",
      "searchFailed": "検索に失敗しました",
      "noCardPath": "カードパスが指定されていません",
      "loadCharacter": "キャラクターの読み込みに失敗しました",
      "downloadCharacter": "キャラクターのダウンロードに失敗しました"
    },
    "card": {
      "byAuthor": "作者: {{author}}",
      "ocBadge": "OC"
    }
  },
  "engine": {
    "gpuInsufficient": "GPUメモリ不足",
    "gpuFallbackDesc": "このモデルはGPUメモリに収まりません。CPU（低速）に切り替えるか中止しますか？",
    "switchToCpu": "CPUに切り替え",
    "abort": "中止",
    "errors": {
      "providerNotFound": "エンジンプロバイダーが見つかりません。",
      "engineOffline": "エンジンがオフラインまたは到達できません。",
      "deleteCharacterFailed": "キャラクターの削除に失敗しました。",
      "unknownCharacter": "不明",
      "seedRequired": "シード説明が必要です。",
      "characterNameRequired": "キャラクター名が必要です。",
      "atLeastOneProvider": "少なくとも1つのプロバイダーを有効にする必要があります。",
      "enableLlmProvider": "少なくとも1つのLLMプロバイダーを有効にしてください。",
      "modelRequired": "{{provider}}にはモデルが必要です。",
      "apiKeyRequired": "{{provider}}にはAPIキーが必要です。",
      "sendMessageFailed": "メッセージの送信に失敗しました。"
    },
    "status": {
      "connected": "接続済み",
      "offline": "オフライン",
      "needsSetup": "セットアップが必要"
    },
    "home": {
      "characters": "キャラクター",
      "newButton": "新規",
      "noCharactersFound": "キャラクターが見つかりません。",
      "tokenUsage": "トークン使用量",
      "totalTokens": "合計トークン",
      "backgroundActivity": "バックグラウンドアクティビティ",
      "quickActions": "クイックアクション",
      "configureProviders": "プロバイダーを設定",
      "engineSettings": "エンジン設定",
      "chat": "チャット",
      "chatDesc": "このキャラクターと会話を開始",
      "deleteCharacter": "キャラクターを削除",
      "deletingCharacter": "削除中...",
      "deleteDesc": "このキャラクターを永久に削除",
      "character": "キャラクター",
      "never": "なし",
      "justNow": "たった今",
      "timeAgo": {
        "minutes": "{{n}}分前",
        "hours": "{{n}}時間前",
        "days": "{{n}}日前"
      }
    },
    "tokens": {
      "input": "入力",
      "output": "出力"
    },
    "activity": {
      "synthesis": "合成",
      "consolidation": "統合",
      "bm25Rebuild": "BM25再構築",
      "dripResearch": "ドリップリサーチ",
      "running": "実行中",
      "stopped": "停止"
    },
    "setup": {
      "complete": "セットアップ完了！",
      "completeMessage": "Lettuceエンジンが設定され、使用準備ができました。",
      "openDashboard": "ダッシュボードを開く"
    },
    "welcome": {
      "title": "Lettuceエンジンへようこそ",
      "subtitle": "AIキャラクターエンジンを設定しましょう。約2分かかります。",
      "feature1": "エンジンはAIキャラクターに永続的な記憶、感情、関係性、本物のアイデンティティを与えます。",
      "feature2": "まずLLMバックエンドを設定し、次にエンジン設定を構成します。",
      "getStarted": "始めましょう"
    },
    "config": {
      "activeProviders": "アクティブなプロバイダー",
      "noModelSet": "モデル未設定",
      "defaultBadge": "デフォルト",
      "noProvidersWarning": "プロバイダーが設定されていません。下に最低1つのLLMバックエンドを追加してください。",
      "addProvider": "プロバイダーを追加",
      "quickImport": "アプリのプロバイダーからクイックインポート",
      "importButton": "インポート",
      "fields": {
        "model": "モデル",
        "modelPlaceholder": "例：claude-sonnet-4-5-20250929",
        "apiKey": "APIキー",
        "apiKeyPlaceholder": "APIキーを入力",
        "currentKey": "現在のキー：",
        "baseUrl": "ベースURL",
        "baseUrlPlaceholder": "http://localhost:11434",
        "maxTokens": "最大トークン",
        "temperature": "温度"
      },
      "enableProvider": "プロバイダーを有効にする",
      "setAsDefault": "デフォルトに設定",
      "defaultBackend": "デフォルトバックエンド",
      "remove": "削除",
      "saveChanges": "変更を保存",
      "saving": "保存中...",
      "saved": "保存しました"
    },
    "providers": {
      "title": "LLMプロバイダー",
      "subtitle": "エンジンには最低1つのLLMバックエンドが必要です。以下でプロバイダーを設定してください。",
      "importFromProviders": "プロバイダーからインポート",
      "imported": "インポート済み",
      "use": "使用",
      "saveContinue": "保存して続行"
    },
    "settings": {
      "fields": {
        "dataDirectory": "データディレクトリ",
        "logLevel": "ログレベル",
        "maxHistory": "最大履歴（会話ターン）"
      },
      "logLevels": {
        "debug": "デバッグ",
        "info": "情報",
        "warning": "警告",
        "error": "エラー"
      },
      "sections": {
        "engine": "エンジン",
        "backgroundLoops": "バックグラウンドループ",
        "memory": "メモリ",
        "safety": "安全性",
        "research": "リサーチ"
      },
      "backgroundLoops": {
        "synthesis": "合成（分）",
        "consolidation": "統合（分）",
        "bm25Rebuild": "BM25再構築（分）",
        "dripResearch": "ドリップリサーチ（分）"
      },
      "memory": {
        "embeddingModel": "埋め込みモデル",
        "maxRetrieval": "最大検索結果",
        "denseWeight": "密ベクトル重み",
        "bm25Weight": "BM25重み",
        "graphWeight": "グラフ重み",
        "recencyBoost": "最近度ブースト（時間）",
        "randomSurface": "ランダムサーフェス確率"
      },
      "safety": {
        "honestySection": "正直さセクション",
        "honestyDesc": "システムプロンプトに正直さセクションを含める",
        "userDataDeletion": "ユーザーデータ削除",
        "userDataDesc": "ユーザーがデータ削除を要求できるようにする"
      },
      "research": {
        "scrapeOnBoot": "起動時にスクレイプ",
        "scrapeDesc": "エンジン起動時にリサーチスクレイプを実行",
        "periodicInterval": "定期間隔（時間）"
      },
      "saveChanges": "変更を保存",
      "saving": "保存中...",
      "saved": "保存しました"
    },
    "settingsStep": {
      "title": "エンジン設定",
      "subtitle": "エンジン全体の設定を構成します。すべて適切なデフォルト値があります — スキップしても構いません。",
      "completingSetup": "セットアップを完了中...",
      "completeSetup": "セットアップを完了"
    },
    "chat": {
      "sendMessage": "メッセージを送信...",
      "sendButton": "メッセージを送信",
      "typeMessage": "メッセージを入力",
      "back": "戻る",
      "assistantTyping": "アシスタントが入力中",
      "fallbackName": "チャット"
    },
    "tagInput": {
      "addMore": "さらに追加...",
      "typeAndPressEnter": "入力してEnterを押す"
    },
    "characterCreate": {
      "steps": {
        "identity": {
          "title": "アイデンティティ",
          "aiGenerated": "AI生成",
          "nameLabel": "名前 *",
          "namePlaceholder": "キャラクター名",
          "eraLabel": "時代",
          "eraPlaceholder": "例：現代、ビクトリア朝",
          "roleLabel": "役割",
          "rolePlaceholder": "例：探偵、科学者",
          "settingLabel": "舞台",
          "settingPlaceholder": "キャラクターが住んでいる場所を説明（一人称）...",
          "coreIdentityLabel": "コアアイデンティティ",
          "coreIdentityPlaceholder": "このキャラクターの本質は？（一人称、3-5文）",
          "backstoryLabel": "バックストーリー",
          "backstoryPlaceholder": "人生のストーリーと重要な出来事（一人称）..."
        },
        "mode": {
          "title": "キャラクターを作成",
          "subtitle": "AIでキャラクターを生成するか、ゼロから構築します。",
          "aiBoost": "AIブースト",
          "aiBoostDesc": "キャラクターのアイデアを説明すると、AIが完全なキャラクター定義を生成します。",
          "nameOptional": "名前（オプション）",
          "namePlaceholder": "例：マーカス・コール",
          "seedDescription": "シード説明 *",
          "seedPlaceholder": "例：1950年代ハーレムのジャズピアニスト、哲学的、深夜の会話が好き",
          "eraOptional": "時代（オプション）",
          "eraPlaceholder": "例：1950年代、現代、ビクトリア朝",
          "generating": "生成中...",
          "generateCharacter": "キャラクターを生成",
          "or": "または",
          "startFromScratch": "ゼロから始める"
        },
        "personality": {
          "title": "性格",
          "traits": "性格の特徴",
          "traitsPlaceholder": "例：ウィットに富む、思いやりがある、頑固",
          "speechPatterns": "話し方のパターン",
          "formality": "フォーマル度",
          "formal": "フォーマル",
          "casual": "カジュアル",
          "texting": "テキスト",
          "verbosity": "冗長さ",
          "terse": "簡潔",
          "medium": "中程度",
          "verbose": "冗長",
          "textStyle": "テキストスタイル",
          "dialect": "方言",
          "dialectPlaceholder": "例：関西弁、東北弁",
          "catchphrases": "キャッチフレーズ",
          "catchphrasesPlaceholder": "例：まあ、なんてこった...",
          "vocabPreferences": "好む語彙",
          "vocabPreferencesPlaceholder": "よく使う言葉",
          "vocabAvoidances": "避ける語彙",
          "vocabAvoidancesPlaceholder": "避ける言葉",
          "fillerWords": "つなぎ言葉",
          "fillerWordsPlaceholder": "例：えーと、まあ、そうですね",
          "exampleQuotes": "例文",
          "exampleQuotesPlaceholder": "3-5の会話例"
        },
        "world": {
          "title": "ワールドと行動",
          "knowledgeDomains": "知識ドメイン",
          "knowledgeDomainsPlaceholder": "例：ジャズの歴史、音楽理論",
          "knowledgeBoundaries": "知識の境界",
          "knowledgeBoundariesPlaceholder": "知らないトピック",
          "researchSeeds": "リサーチシード",
          "researchSeedsPlaceholder": "バックグラウンドリサーチの開始トピック",
          "researchEnabled": "リサーチ有効",
          "researchEnabledDesc": "バックグラウンドでの知識収集を許可",
          "physicalDescription": "外見の説明",
          "physicalDescPlaceholder": "外見と仕草...",
          "physicalHabits": "身体的な癖",
          "physicalHabitsPlaceholder": "例：指を叩く、メガネを直す",
          "idleBehaviors": "アイドル行動",
          "idleBehaviorsPlaceholder": "関わっていない時にすること",
          "timeBehaviors": "時間帯の行動",
          "timePlaceholder": "{{period}}にすることは？",
          "earlyMorning": "早朝",
          "morning": "朝",
          "afternoon": "午後",
          "evening": "夕方",
          "night": "夜",
          "baselineEmotions": "ベースライン感情（プルチック）",
          "emotionDesc": "デフォルトの感情ベースラインを設定（0 = なし、1 = 最大）",
          "joy": "喜び",
          "trust": "信頼",
          "fear": "恐怖",
          "surprise": "驚き",
          "sadness": "悲しみ",
          "disgust": "嫌悪",
          "anger": "怒り",
          "anticipation": "期待",
          "engineOverrides": "エンジンオーバーライド",
          "backend": "バックエンド",
          "model": "モデル",
          "temperature": "温度",
          "leaveEmpty": "デフォルトは空のまま"
        },
        "review": {
          "title": "レビュー",
          "subtitle": "作成前にキャラクターを確認してください。",
          "edit": "編集",
          "notSet": "未設定",
          "identitySection": "アイデンティティ",
          "personalitySection": "性格",
          "worldSection": "ワールドと行動",
          "nameLabel": "名前",
          "eraLabel": "時代",
          "roleLabel": "役割",
          "settingLabel": "舞台",
          "coreIdentityLabel": "コアアイデンティティ",
          "backstoryLabel": "バックストーリー",
          "traitsLabel": "特性",
          "formalityLabel": "フォーマル度",
          "verbosityLabel": "冗長さ",
          "dialectLabel": "方言",
          "catchphrasesLabel": "キャッチフレーズ",
          "domainsLabel": "ドメイン",
          "boundariesLabel": "境界",
          "researchSeedsLabel": "リサーチシード",
          "researchLabel": "リサーチ",
          "enabled": "有効",
          "disabled": "無効",
          "physicalLabel": "外見",
          "habitsLabel": "癖",
          "idleLabel": "アイドル",
          "timeBehaviorsLabel": "時間帯の行動",
          "emotionsLabel": "感情",
          "configured": "設定済み",
          "backendLabel": "バックエンド",
          "modelLabel": "モデル",
          "temperatureLabel": "温度",
          "creating": "作成中...",
          "createCharacter": "キャラクターを作成"
        }
      }
    }
  },
  "library": {
    "filterTitle": "ライブラリをフィルター",
    "filters": {
      "all": "すべて",
      "characters": "キャラクター",
      "personas": "ペルソナ",
      "lorebooks": "ロアブック",
      "images": "画像"
    },
    "emptyStates": {
      "all": {
        "title": "ライブラリは空です",
        "description": "キャラクター、ペルソナ、ロアブックを作成するとここに表示されます"
      },
      "characters": {
        "title": "キャラクターがまだいません",
        "description": "チャットを始めるために最初のキャラクターを作成してください"
      },
      "personas": {
        "title": "ペルソナがまだありません",
        "description": "チャットアイデンティティをカスタマイズするペルソナを作成"
      },
      "lorebooks": {
        "title": "ロアブックがまだありません",
        "description": "ロアブックはキャラクターの設定内から作成されます"
      }
    },
    "actions": {
      "startChat": "チャット開始",
      "editCharacter": "キャラクターを編集",
      "editPersona": "ペルソナを編集",
      "editLorebook": "ロアブックを編集",
      "renameLorebook": "ロアブックの名前変更",
      "exportCharacter": "キャラクターをエクスポート",
      "exportPersona": "ペルソナをエクスポート",
      "chatAppearance": "チャット外観",
      "deleteCharacter": "キャラクターを削除",
      "deletePersona": "ペルソナを削除",
      "deleteLorebook": "ロアブックを削除",
      "importLorebook": "ロアブックをインポート"
    },
    "imageLibrary": {
      "filters": {
        "all": "すべて",
        "backgrounds": "背景",
        "avatars": "アバター",
        "attachments": "添付",
        "other": "その他"
      },
      "searchPlaceholder": "ファイル名、パス、セッションID、エンティティIDで検索",
      "empty": {
        "title": "この表示に一致する画像はありません",
        "description": "別のフィルターや検索語を試してください。ライブラリにはアプリのローカルストレージに保存済みの画像だけが表示されます。"
      },
      "actions": {
        "sort": "並び替え",
        "useThis": "これを使う",
        "using": "使用中...",
        "copyPath": "パスをコピー",
        "saving": "保存中...",
        "download": "ダウンロード",
        "delete": "画像を削除",
        "deleting": "削除中..."
      },
      "active": "有効",
      "messages": {
        "loadFailed": "画像ライブラリの読み込みに失敗しました",
        "saved": "画像を保存しました",
        "downloadFailed": "ダウンロードに失敗しました",
        "useFailed": "この画像は使用できませんでした",
        "deleted": "画像を削除しました",
        "deleteFailed": "画像を削除できませんでした"
      },
      "deleteConfirm": {
        "title": "画像を削除しますか？",
        "message": "本当に \"{{filename}}\" を削除しますか？ まだ使っているアバター、チャット背景、メッセージ添付が壊れる可能性があります。"
      },
      "sort": {
        "newest": "新しい順",
        "largest": "大きい順",
        "name": "名前"
      },
      "kinds": {
        "background": "背景",
        "avatar": "アバター",
        "attachment": "添付ファイル",
        "stored": "保存済み"
      },
      "detailsTitle": "{{kind}}の詳細",
      "formatsLabel": "形式",
      "storagePath": "ストレージパス",
      "contextLabel": "コンテキスト",
      "contextLinkedFallback": "リンク済み",
      "show": "表示",
      "hide": "非表示",
      "contextRoles": {
        "character": "キャラクター:",
        "session": "セッション:",
        "role": "役割:"
      },
      "downloadFormat": "{{download}} 形式",
      "unknownDate": "不明",
      "clearSearch": "検索をクリア",
      "copyFilename": "ファイル名をコピー",
      "copyLabels": {
        "filename": "ファイル名",
        "storagePath": "ストレージパス"
      },
      "copy": {
        "copied": "{{label}}をコピーしました",
        "failed": "{{label}}のコピーに失敗しました"
      }
    },
    "deleteConfirm": {
      "title": "{{itemType}}を削除しますか？",
      "message": "本当に削除してもよろしいですか",
      "characterWarning": "このキャラクターとのすべてのチャットセッションも削除されます。"
    },
    "rename": {
      "title": "ロアブックの名前変更",
      "placeholder": "新しい名前を入力..."
    },
    "itemTypes": {
      "character": "キャラクター",
      "persona": "ペルソナ",
      "lorebook": "ロアブック"
    },
    "lorebookLabel": "ロアブック",
    "noDescriptionYet": "説明がまだありません",
    "errors": {
      "importLorebook": "ロアブックのインポートに失敗しました。{{error}}",
      "exportFailed": "エクスポートに失敗しました"
    },
    "card": {
      "avatarAlt": "{{name}}のアバター"
    },
    "lorebookEditor": {
      "titleOverride": "ロアブック - {{name}}",
      "dragToReorder": "ドラッグで並び替え",
      "aria": {
        "generateEntry": "ロアブックエントリーを生成",
        "editLorebook": "ロアブックを編集",
        "exportLorebook": "ロアブックをエクスポート"
      }
    }
  },
  "onboarding": {
    "loading": "プロバイダーを読み込み中...",
    "stepIndicator": "ステップ{{current}} / {{total}}",
    "steps": {
      "provider": "プロバイダーセットアップ",
      "model": "モデルセットアップ",
      "memory": "メモリシステム",
      "stepNofM": "ステップ {{current}} / {{total}}"
    },
    "provider": {
      "availableProviders": "利用可能なプロバイダー",
      "chooseProvider": "プロバイダーを選択",
      "titleMobile": "AIプロバイダーを選択",
      "descMobile": "始めるにはAIプロバイダーを選択してください。APIキーはデバイス上で安全に暗号化されます。アカウント登録は不要です。",
      "configureProvider": "{{name}}を設定",
      "connectProvider": "{{name}}に接続",
      "connectProviderDesc": "チャットを有効にするには、下にAPIキーを貼り付けてください。キーが必要ですか?プロバイダーのダッシュボードで取得できます。",
      "localLLMs": "ローカルLLM",
      "useLocalLLMs": "ローカルLLMを使用したい",
      "browseModelLibrary": "モデルライブラリを参照",
      "browseModelLibraryDesc": "HuggingFaceからGGUFモデルを検索・ダウンロードします",
      "useOwnGguf": "自分のGGUFファイルを使用",
      "useOwnGgufDesc": "デバイスからGGUFモデルとオプションのmmprojファイルを選択します",
      "fields": {
        "displayLabel": "表示ラベル",
        "displayLabelHint": "メニューでこのプロバイダーがどのように表示されるか",
        "displayLabelPlaceholder": "私の{{name}}",
        "defaultLabelFallback": "プロバイダー",
        "apiKey": "APIキー",
        "apiKeyOptional": "APIキー(任意)",
        "apiKeyHint": "キーはローカルで暗号化されます",
        "apiKeyPlaceholderRemote": "sk-...",
        "apiKeyPlaceholderLocal": "通常は不要",
        "whereToFind": "見つけ方",
        "baseUrl": "ベースURL",
        "baseUrlPlaceholderHost": "http://192.168.1.10:3333",
        "baseUrlPlaceholderLocal": "http://localhost:11434",
        "baseUrlPlaceholderRemote": "https://api.provider.com",
        "baseUrlPlaceholderIntenseRP": "http://127.0.0.1:7777/v1",
        "baseUrlHintLocal": "ポート付きのローカルサーバーアドレス",
        "baseUrlHintHost": "ホストデバイスに表示されるデスクトップホストURLを入力してください",
        "baseUrlHintRemote": "必要に応じてデフォルトのエンドポイントを上書きします",
        "chatEndpoint": "チャットエンドポイント",
        "systemRole": "システムロール",
        "userRole": "ユーザーロール",
        "assistantRole": "アシスタントロール",
        "supportsStreaming": "ストリーミングをサポート",
        "mergeSameRole": "同じロールのメッセージをマージ",
        "toolChoiceMode": "ツール選択モード",
        "toolChoiceHint": "tool_choiceがカスタムエンドポイントにどのように送信されるかを制御します。"
      },
      "toolChoice": {
        "auto": "自動",
        "required": "必須",
        "none": "なし",
        "omit": "フィールドを省略",
        "passthrough": "パススルー(ツール設定)"
      },
      "buttons": {
        "testConnection": "接続をテスト",
        "testing": "テスト中..."
      },
      "descriptions": {
        "chutes": "トップオープンソースモデル向けのOpenAI互換推論",
        "openai": "表現豊かなRPのためのGPT-5、GPT-4.1、GPT-4oモデル",
        "lettuceHost": "OpenAIスタイルAPIで自分のデスクトップLettuce HostにLAN経由で接続",
        "anthropic": "深く感情的な対話のためのClaude 4.5 Sonnet & Haiku",
        "aggregator": "GPT-5、Claude 4.5、Grok-3、Mixtralなどのモデルにアクセス",
        "openaiCompatible": "任意のOpenAIスタイルAPIエンドポイントを使用",
        "mistral": "Mistral Small 3.2、Mixtral 8x22B、その他のMistralモデル",
        "deepseek": "DeepSeek-V3.2-Exp、DeepSeek-R1、その他の高効率モデル",
        "xai": "Grok-1.5、Grok-3、新しいxAIモデル",
        "zai": "GLM-4.5、GLM-4.6、Airバリアント",
        "moonshot": "Kimi-K2 ThinkingとKimi-K1モデル",
        "gemini": "Gemini 2.5 Flash、2.5 Pro、その他",
        "qwen": "Qwen3-VLと新しいQwenモデル",
        "nvidia": "NVIDIA NIM経由のNemotron、Llama、DeepSeekなど",
        "custom": "LettuceAIを任意のカスタムモデルエンドポイントに向ける",
        "fallback": "AIモデルプロバイダー"
      },
      "descriptionsShort": {
        "chutes": "オープンソースモデル推論",
        "openai": "GPT-5、GPT-4o、GPT-4.1",
        "lettuceHost": "自分のLANホスト",
        "anthropic": "Claude 4.5 Sonnet & Haiku",
        "aggregator": "マルチモデルアグリゲーター",
        "openaiCompatible": "カスタムOpenAIエンドポイント",
        "mistral": "Mistral & Mixtralモデル",
        "deepseek": "DeepSeek-V3、R1",
        "xai": "Grok-1.5、Grok-3",
        "zai": "GLM-4.5、GLM-4.6",
        "moonshot": "Kimi-K2 Thinking",
        "gemini": "Gemini 2.5 Flash & Pro",
        "qwen": "Qwen3-VLモデル",
        "nvidia": "NVIDIA NIM推論",
        "custom": "カスタムエンドポイント",
        "fallback": "AIプロバイダー"
      },
      "cardLabel": "{{step}}: {{name}}",
      "errors": {
        "hostUrlRequired": "ホストURLが必要です(例: http://192.168.1.10:3333)",
        "baseUrlRequired": "ベースURLが必要です(例: http://localhost:11434)",
        "apiKeyTooShort": "APIキーが短すぎるようです",
        "invalidApiKey": "無効なAPIキー",
        "connectionFailed": "接続に失敗しました",
        "verificationFailed": "検証に失敗しました",
        "failedToSave": "プロバイダーの保存に失敗しました",
        "connectionSuccessful": "接続に成功しました!",
        "modelNotFound": "プロバイダーにモデルが見つかりません",
        "modelVerificationFailed": "モデルの検証に失敗しました",
        "failedToSaveModel": "モデルの保存に失敗しました"
      }
    },
    "model": {
      "noProvidersTitle": "プロバイダーが設定されていません",
      "noProvidersDesc": "デフォルトモデルを選択する前に、プロバイダーに接続する必要があります。",
      "goToProviderSetup": "プロバイダーセットアップへ",
      "yourProviders": "あなたのプロバイダー",
      "yourProvidersHint": "使用するプロバイダーを選択",
      "setDefaultModel": "デフォルトモデルを設定",
      "setDefaultModelDesc": "LettuceAIがデフォルトで使用するプロバイダーとモデル名を選択します。後で追加できます。",
      "setDefaultModelDescDesktop": "リストからプロバイダーを選択してモデルを設定します。",
      "modelDetails": "モデルの詳細",
      "modelDetailsDesc": "API識別子とアプリ内に表示されるラベルを定義します。",
      "whichModel": "どのモデルを使うべきですか?",
      "nextMemorySystem": "次へ: メモリシステム",
      "fields": {
        "displayName": "表示名",
        "displayNamePlaceholder": "クリエイティブメンター",
        "displayNameHint": "メニューでこのモデルがどのように表示されるか",
        "modelId": "モデルID",
        "modelPathGguf": "モデルパス(GGUF)",
        "modelIdPlaceholder": "例: gpt-4o",
        "modelPathPlaceholder": "/path/to/model.gguf",
        "modelIdHint": "API呼び出しで使用される正確な識別子",
        "showList": "リストを表示",
        "manualInput": "手動入力",
        "refreshModelList": "モデルリストを更新",
        "selectModel": "モデルを選択",
        "selectAModel": "モデルを選択してください...",
        "searchModels": "モデルを検索...",
        "noModelsFound": "「{{query}}」に一致するモデルが見つかりません"
      },
      "fillBothFields": "完了ボタンを有効にするには、上記の両方のフィールドを入力してください。",
      "providerNames": {
        "chutes": "Chutes",
        "intenseRpNext": "IntenseRP Next",
        "openai": "OpenAI",
        "anthropic": "Anthropic",
        "openrouter": "OpenRouter",
        "openaiCompatible": "OpenAI互換",
        "custom": "カスタムエンドポイント"
      }
    },
    "memory": {
      "dynamicTitle": "ダイナミックメモリ",
      "recommended": "推奨",
      "settingUp": "セットアップ中...",
      "finishSetup": "セットアップを完了",
      "promptTitle": "ダイナミックメモリをセットアップ",
      "oneLastStep": "あと一歩",
      "downloadAndEnable": "ダウンロードして有効にする",
      "chooseStyle": "メモリスタイルを選択",
      "howRemember": "AIコンパニオンはあなたや会話の詳細をどのように記憶すべきですか?",
      "dynamicDescription": "<0>ローカルの埋め込みモデル</0>を使用してコンテキストをスマートに管理します。長いチャットでも高品質を維持しつつトークンコストを削減します。",
      "dynamicFeatures": {
        "quality": "長いチャットでも品質を維持",
        "cost": "APIコストを大幅に削減",
        "auto": "自動コンテキスト管理",
        "zeroConfig": "設定不要"
      },
      "manualTitle": "手動メモリ",
      "manualBadge": "クラシックな体験",
      "manualDescription": "メッセージを明示的にピン留めし、「ワールド情報」やキャラクター定義を自分で編集します。完全なコントロールに最適。",
      "manualFeatures": {
        "control": "事実への完全なコントロール",
        "scenarios": "特定のシナリオに最適"
      },
      "setupModelMessage": "ダイナミックメモリを使用するには、小さな埋め込みモデル(約120MB)をデバイスにダウンロードする必要があります。",
      "setupBullets": {
        "offline": "モデルはデバイス上で100%オフラインで動作",
        "remembering": "コンテキストの記憶に必要",
        "disable": "後で設定で無効にできます"
      },
      "stepLabel": "ステップ 3 / 3",
      "stepLabelMemory": "メモリシステム"
    },
    "welcome": {
      "appName": "LettuceAI",
      "tagline": "あなたのパーソナルAIコンパニオン。プライベート、安全、常にデバイス上で。",
      "features": {
        "onDevice": "デバイス上のみ",
        "characterReady": "キャラクター対応"
      },
      "betaWarning": {
        "title": "デスクトップベータビルド",
        "description": "デスクトップ版を使用しています。一部の機能がモバイルと異なる場合があります。問題はGitHubで報告してください。"
      },
      "languageSelector": {
        "title": "言語",
        "description": "デバイスから自動検出されました。設定からいつでも変更できます。"
      },
      "getStarted": "始めましょう",
      "skipForNow": "今はスキップ",
      "restoreFromBackup": "バックアップから復元",
      "setupTime": "セットアップは2分以内で完了します",
      "skipWarning": {
        "title": "セットアップをスキップしますか？",
        "warningTitle": "チャットにはプロバイダーが必要です",
        "warningMessage": "プロバイダーなしではメッセージを送信できません。後で設定から追加できます。",
        "addProvider": "プロバイダーを追加",
        "skipAnyway": "それでもスキップ"
      },
      "restoreBackup": {
        "title": "バックアップを復元",
        "selectMessage": "復元するバックアップを選択してください。",
        "browse": "ファイルを参照",
        "processing": "ファイルを処理中...",
        "processingNote": "大きなバックアップは時間がかかる場合があります",
        "noBackups": "バックアップが見つかりません",
        "noBackupsHint": "参照をタップして.lettuceファイルを選択",
        "browseLettuce": ".lettuceファイルを参照",
        "passwordLabel": "バックアップパスワード",
        "passwordPlaceholder": "パスワードを入力",
        "restoreButton": "バックアップを復元",
        "restoring": "復元中...",
        "infoMessage": "これにより、バックアップされたデータ（キャラクター、チャット、設定を含む）でアプリがセットアップされます。",
        "embeddingTitle": "埋め込みモデルが必要です",
        "dynamicMemoryDetected": "ダイナミックメモリが検出されました",
        "dynamicMemoryMessage": "このバックアップにはダイナミックメモリが有効なキャラクターが含まれており、埋め込みモデル（約120MB）が必要です。",
        "embeddingOptions": "今すぐモデルをダウンロードしてダイナミックメモリを有効にするか、なしで続行できます（影響を受けるキャラクターのダイナミックメモリは無効になります）。",
        "downloadModel": "モデルをダウンロード",
        "continueWithoutDynamic": "ダイナミックメモリなしで続行",
        "embeddingNote": "モデルダウンロード後、キャラクター設定からダイナミックメモリを再度有効にできます。",
        "back": "戻る",
        "cancel": "キャンセル",
        "errors": {
          "passwordRequired": "パスワードが必要です",
          "incorrectPassword": "パスワードが正しくありません",
          "failedToOpenFile": "ファイルを開けませんでした",
          "failedToRestore": "バックアップの復元に失敗しました",
          "failedToUpdateSettings": "設定の更新に失敗しました"
        }
      }
    },
    "common": {
      "back": "戻る",
      "cancel": "キャンセル",
      "continue": "続行",
      "verifying": "検証中...",
      "skipForNow": "今はスキップ",
      "selectAProvider": "設定するプロバイダーを選択",
      "clickToSelectProvider": "クリックしてプロバイダーを選択",
      "selectProviderFromList": "リストからプロバイダーを選択して始めましょう。",
      "enterApiKey": "AIチャット機能を有効にするには、APIキーを入力してください。"
    },
    "modelGuide": {
      "badge": "モデルガイド",
      "title": "モデルはどう選べばよいですか?",
      "intro": "LettuceAIは単一の「最良の」モデルを強制しません。代わりに、<0>ユースケース、予算、雰囲気</0>に合うものを選びます。試すモデルや探す場所を決めるためにこのガイドを使用してください。",
      "askYourself": "自問してみてください:",
      "factors": {
        "quality": {
          "title": "品質と能力",
          "description": "モデルはどの程度賢い必要がありますか?より大きく新しいモデルは通常、推論が優れ、より良いテキストを書き、雑なプロンプトをより上手く処理します。",
          "q1": "深いキャラクターの一貫性と感情的知性が必要ですか?",
          "q2": "没入感のあるストーリーテリングと信じられるキャラクターの個性を重視しますか?",
          "q3": "モデルが長いセッションを通してキャラクターの詳細を覚え、キャラクターを保ってほしいですか?"
        },
        "speed": {
          "title": "速度とレイテンシ",
          "description": "速いモデルはおしゃべりなやり取りに向いています。一部のモデルは少しの品質を大きな速度と引き換えにします。",
          "q1": "ロールプレイを自然に流すために、ほぼ即時の返信が必要ですか?",
          "q2": "待つと没入感が壊れる、急速な対話シーンをやっていますか?",
          "q3": "完璧な応答よりも素早いやり取りが重要なカジュアルRP用ですか?"
        },
        "budget": {
          "title": "予算と使用量",
          "description": "プロバイダーはトークンごとに課金します。安いモデルでもよく使えば積み重なるので、使用頻度や程度に合うものを選びましょう。",
          "q1": "豊かなキャラクター対話のために多く払いますか、それとも日常RPには安いものが良いですか?",
          "q2": "プロバイダー/ルーターから先に試せる無料モデルはありますか?",
          "q3": "詳細なシーン記述で長いロールプレイセッションを実行しますか?",
          "q4": "超えたくない月の予算上限はありますか?"
        },
        "safety": {
          "title": "安全性、プライバシー、追加機能",
          "description": "プロバイダーは安全性、ロギング、画像・ツール・長いコンテキストウィンドウなどの追加機能の扱いが異なります。",
          "q1": "成人向けまたはクリエイティブなロールプレイシナリオのためにコンテンツフィルターが少ない方が良いですか?",
          "q2": "プライベートなRPの会話がログされたりトレーニングに使われたりすることを気にしますか?",
          "q3": "複雑なストーリーラインとキャラクター履歴のために長いコンテキストウィンドウが必要ですか?"
        }
      },
      "where": {
        "title": "モデルはどこで見つけられますか?",
        "intro": "ほとんどのプロバイダーやルーターには<0>モデルリストやカタログ</0>があります。それらのページを参照して、提供内容、価格、制限、特別な機能を確認してください。",
        "directTitle": "直接プロバイダー",
        "directDesc": "OpenAI、Anthropic、Google Gemini、xAI、Mistralなど。それぞれにコンソール/ダッシュボードがあり、公式モデル名、機能、価格を確認できます。",
        "routersTitle": "ルーターとハブ",
        "routersDesc": "OpenRouterや他のアグリゲーターのようなサービスは、異なるプロバイダーの多くのモデルを一箇所に列挙します。多くの場合、ベンチマークと価格比較付きです。",
        "communityTitle": "コミュニティの推奨",
        "communityDesc": "プロバイダー/ルーターのドキュメント、ブログ、コミュニティ投稿を見てください。チャット、コーディング、速度に最適なモデルが強調されています。"
      },
      "rules": {
        "title": "シンプルな経験則",
        "casual": "カジュアルなチャット用: プロバイダー/ルーターから速くて安いチャットモデルを選ぶ。",
        "experiments": "実験や大量使用用: 十分良いと感じる最も安いモデルから始めて、必要なら格上げする。",
        "switch": "違和感がある場合(遅すぎ/愚かすぎ/高すぎ): LettuceAIで後からいつでもモデルを切り替えられます。"
      },
      "disclaimer": "最新のモデルリスト、制限、価格については、必ずプロバイダーのドキュメントを確認してください。このページは何を買うかではなく、どう考えるかについてです。"
    },
    "whereToFind": {
      "badge": "APIキーヘルプ",
      "intro": "次の手順に従ってAPIキーを取得し、LettuceAIに戻ってプロバイダー設定に貼り付けてください。",
      "readyPrompt": "キーを取得する準備はできましたか?",
      "openProviderSite": "プロバイダーサイトを開く",
      "keyWarning": "APIキーを公に共有しないでください。このキーを持つ誰でもあなたのアカウント残高を使用できます。",
      "stuckPrompt": "それでも分かりませんか?",
      "joinDiscord": "サポートのため Discord サーバーに参加",
      "guides": {
        "chutes": {
          "title": "Chutes APIキーの見つけ方",
          "s1": "chutes.ai/appにアクセスしてサインインします。",
          "s2": "アカウント/設定エリアを開き、APIキーを見つけます。",
          "s3": "新しいキーを作成(または既存のものをコピー)します。",
          "s4": "キーをLettuceAIに貼り付けます。"
        },
        "openai": {
          "title": "OpenAI APIキーの見つけ方",
          "s1": "platform.openai.comにアクセスしてサインインします。",
          "s2": "右上のプロフィールアバターをクリックし、API keysを選択します。",
          "s3": "Create new secret keyをクリックし、表示された値をコピーします。",
          "s4": "キーをLettuceAIに貼り付け、安全な場所に保管してください。再表示はできません。"
        },
        "anthropic": {
          "title": "Anthropic APIキーの見つけ方",
          "s1": "console.anthropic.comにアクセスしてサインインします。",
          "s2": "左サイドバーから設定を開きます。",
          "s3": "API keysを選択し、Create keyをクリックします。",
          "s4": "キーをコピーしてLettuceAIに貼り付けます。"
        },
        "openrouter": {
          "title": "OpenRouter APIキーの見つけ方",
          "s1": "openrouter.aiにアクセスしてログインします。",
          "s2": "プロフィールメニューからKeysページを開きます。",
          "s3": "Create keyをクリックし、名前を付けて保存します。",
          "s4": "キーをコピーしてLettuceAIに貼り付けます。"
        },
        "mistral": {
          "title": "Mistral APIキーの見つけ方",
          "s1": "console.mistral.aiにアクセスしてサインインします。",
          "s2": "サイドバーのAPI keysをクリックします。",
          "s3": "Create an API keyをクリックします。",
          "s4": "キーをコピーしてLettuceAIに貼り付けます。"
        },
        "deepseek": {
          "title": "DeepSeek APIキーの見つけ方",
          "s1": "platform.deepseek.comを開いてログインします。",
          "s2": "上部ナビゲーションのAPI Keysをクリックします。",
          "s3": "まだ持っていない場合は新しいキーを作成します。",
          "s4": "キーをコピーしてLettuceAIに貼り付けます。"
        },
        "groq": {
          "title": "Groq APIキーの見つけ方",
          "s1": "console.groq.comにアクセスしてサインインします。",
          "s2": "サイドバーからAPI Keysを開きます。",
          "s3": "新しいキーを作成し、コピーします。",
          "s4": "キーをLettuceAIに貼り付けます。"
        },
        "gemini": {
          "title": "Google Gemini APIキーの見つけ方",
          "s1": "aistudio.google.comのGoogle AI Studioにアクセスしてサインインします。",
          "s2": "Get API keyまたはManage keysをクリックします。",
          "s3": "必要に応じて新しいキーを作成します。",
          "s4": "キーをコピーしてLettuceAIに貼り付けます。"
        },
        "xai": {
          "title": "xAI APIキーの見つけ方",
          "s1": "console.x.aiを開いてサインインします。",
          "s2": "コンソールのAPI Keysセクションに移動します。",
          "s3": "新しいキーを作成します。",
          "s4": "キーをコピーしてLettuceAIに貼り付けます。"
        },
        "zai": {
          "title": "zAI(GLM)APIキーの見つけ方",
          "s1": "open.bigmodel.cnにアクセスしてログインします。",
          "s2": "ユーザーセンターを開き、API Keysに移動します。",
          "s3": "持っていない場合は新しいキーを作成します。",
          "s4": "キーをコピーしてLettuceAIに貼り付けます。"
        },
        "moonshot": {
          "title": "Moonshot(Kimi)APIキーの見つけ方",
          "s1": "platform.moonshot.cnにアクセスしてサインインします。",
          "s2": "コンソールのAPI Keysセクションを開きます。",
          "s3": "新しいキーを作成し、コピーします。",
          "s4": "キーをLettuceAIに貼り付けます。"
        },
        "qwen": {
          "title": "Qwen APIキーの見つけ方",
          "s1": "dashscope.aliyun.comを開いてログインします。",
          "s2": "サイドバーのAPI Keysセクションに移動します。",
          "s3": "新しいキーを作成します。",
          "s4": "キーをコピーしてLettuceAIに貼り付けます。"
        },
        "nanogpt": {
          "title": "NanoGPT APIキーの見つけ方",
          "s1": "nano-gpt.comにアクセスしてログインします。",
          "s2": "ダッシュボードを開き、APIキーセクションに移動します。",
          "s3": "必要に応じて新しいキーを作成します。",
          "s4": "キーをコピーしてLettuceAIに貼り付けます。"
        },
        "featherless": {
          "title": "Featherless APIキーの見つけ方",
          "s1": "featherless.aiにアクセスしてサインインします。",
          "s2": "ダッシュボードからアカウントまたはAPIセクションを開きます。",
          "s3": "見当たらない場合は新しいキーを作成します。",
          "s4": "キーをコピーしてLettuceAIに貼り付けます。"
        },
        "anannas": {
          "title": "Anannas APIキーの見つけ方",
          "s1": "dashboard.anannas.aiにアクセスしてログインします。",
          "s2": "API Keysセクションに移動します。",
          "s3": "新しいキーを作成し、コピーします。",
          "s4": "キーをLettuceAIに貼り付けます。"
        },
        "default": {
          "title": "APIキーの見つけ方",
          "s1": "ブラウザでAIプロバイダーのダッシュボードを開きサインインします。",
          "s2": "API、Developer、Integrationsの設定を探します。",
          "s3": "新しいAPIキーを作成するか、既存のものを表示します。",
          "s4": "キーをコピーしてLettuceAIに貼り付けます。"
        }
      }
    },
    "welcomeFooter": {
      "setupTimeMobile": "セットアップは2分以内で完了します"
    }
  },
  "search": {
    "placeholder": "検索...",
    "tabs": {
      "characters": "キャラクター",
      "personas": "ペルソナ"
    },
    "noResults": "{{type}}が見つかりません",
    "emptyState": "{{type}}はまだありません",
    "noResultsHint": "別の検索語をお試しください",
    "emptyCharacters": "チャットを始めるために最初のキャラクターを作成してください",
    "emptyPersonas": "設定でペルソナを作成してください",
    "a11y": {
      "goBack": "戻る",
      "clearSearch": "検索をクリア",
      "characterAvatar": "{{name}}のアバター"
    },
    "session": {
      "newChatTitle": "新しいチャット"
    },
    "noDescription": "説明なし",
    "defaultBadge": "デフォルト"
  },
  "sync": {
    "modes": {
      "join": "参加",
      "joinDesc": "ホストに接続",
      "host": "ホスト",
      "hostDesc": "データを共有"
    },
    "sections": {
      "mode": "モード",
      "connectToHost": "ホストに接続",
      "startHosting": "ホスティングを開始",
      "status": "ステータス",
      "hosting": "ホスティングサービス",
      "localAddress": "ローカルネットワークアドレス",
      "connectionPin": "接続PIN",
      "setupGuide": "セットアップガイド"
    },
    "fields": {
      "hostAddress": "ホストアドレスまたはJSON",
      "hostPlaceholder": "例：192.168.1.100:12345",
      "pinCode": "PINコード",
      "pinPlaceholder": "000000"
    },
    "buttons": {
      "scanQRCode": "QRコードをスキャン",
      "connect": "接続",
      "connecting": "接続中...",
      "startHosting": "ホスティングを開始",
      "startingServer": "サーバーを起動中...",
      "stopHosting": "ホスティングを停止",
      "hostAgain": "再度ホスト",
      "done": "完了"
    },
    "status": {
      "connecting": "接続中...",
      "connected": "接続済み",
      "waitingConfirmation": "確認待ち",
      "waitingConfirmationDesc": "続行するには、ホストデバイスでの接続を承認してください。",
      "syncing": "同期中...",
      "transferringData": "データを転送中",
      "syncInProgress": "同期進行中",
      "live": "ライブ",
      "broadcasting": "ブロードキャスト中",
      "clientsLabel": "接続済み",
      "clientsUnit": "クライアント"
    },
    "pinDescription": "接続するデバイスとこのPINを共有してください",
    "hostingDesc1": "他のデバイスがこのデバイスに接続してデータを同期できます。",
    "hostingDesc2": "接続されたクライアントとデータが共有されます。",
    "setupSteps": {
      "step1": "別のデバイスでアプリを開く",
      "step2": "設定 → ローカル同期に移動",
      "step3": "QRコードをスキャンまたはアドレスを入力"
    },
    "messages": {
      "completed": "同期完了！",
      "completedDesc": "すべてのデータが同期されました",
      "error": "接続エラー",
      "outdatedClient": "古いクライアントが検出されました"
    },
    "disclaimer": "同期はローカルネットワーク上で動作します。両方のデバイスが同じWiFiに接続している必要があります。",
    "modals": {
      "connectionRequest": "接続リクエスト",
      "requestMessage": "がこのデバイスと同期しようとしています。",
      "acceptConnection": "接続を承認",
      "acceptDesc": "このデバイスのデータ同期を許可",
      "decline": "拒否",
      "declineDesc": "この接続試行をブロック",
      "readyToSync": "同期準備完了",
      "connectionEstablished": "接続が確立されました",
      "deviceReady": "準備ができました。",
      "startSyncMessage": "下をタップしてデータの同期を開始してください。",
      "startSyncing": "同期を開始",
      "startSyncingDesc": "今すぐデータ転送を開始"
    },
    "scanner": {
      "title": "QRコードをスキャン",
      "cancel": "スキャンをキャンセル"
    },
    "unknownDevice": "不明なデバイス",
    "aria": {
      "dismissStatus": "同期ステータスを閉じる",
      "dismissError": "同期エラーを閉じる"
    },
    "stats": {
      "statusLabel": "ステータス"
    }
  },
  "creationHelper": {
    "page": {
      "info": "作成ヘルパーはAIアシスタントを使ってキャラクター作成をガイドします。キャラクター作成中に使用するモデルとツールを設定してください。",
      "modelConfiguration": "モデル設定",
      "chatModel": "チャットモデル",
      "selectedModel": "選択されたモデル",
      "useAppDefault": "アプリのデフォルトを使用{{model}}",
      "useAppDefaultBase": "アプリのデフォルトを使用",
      "noModelsAvailable": "利用可能なモデルがありません",
      "chatModelDescription": "キャラクター作成の会話用AIモデル",
      "streamingOutput": "ストリーミング出力",
      "streamingDescription": "生成中の応答をリアルタイム表示",
      "imageGenerationModel": "画像生成モデル",
      "noModelSelected": "モデルが選択されていません",
      "noImageModelsAvailable": "画像モデルが利用できません",
      "imageModelDescription": "キャラクターアバターの生成用",
      "toolSelection": "ツール選択",
      "smartToolSelection": "スマートツール選択",
      "smartToolDescription": "AIが使用するツールを自動的に選択します",
      "smartToolEnabledHint": "有効にすると、AI作成ヘルパーが何を作成したいか尋ね、関連するツールセットのみを読み込みます。",
      "smartToolDisabledHint": "無効にすると、AI作成ヘルパーが直接開き、すべての有効なツールを使用します。アシスタントが何を構築するか決定します。",
      "quickPresets": "クイックプリセット",
      "customSelection": "カスタム選択 - {{count}}個のツールが有効",
      "footerInfo": "スマートツール選択が有効な場合、AIがコンテキストに基づいて使用するツールを決定します。無効にすると、利用可能なツールを手動で制御できます。",
      "selectChatModel": "チャットモデルを選択",
      "selectImageModel": "画像モデルを選択",
      "searchModels": "モデルを検索..."
    },
    "categories": {
      "basic": "基本",
      "content": "コンテンツ",
      "visual": "ビジュアル",
      "settings": "設定",
      "flow": "フロー",
      "persona": "ペルソナ",
      "lorebook": "ロアブック"
    },
    "presets": {
      "all": {
        "name": "全ツール",
        "desc": "利用可能なすべてのツールを有効にする"
      },
      "essential": {
        "name": "エッセンシャル",
        "desc": "名前、定義、シーンのみ"
      },
      "minimal": {
        "name": "ミニマル",
        "desc": "名前と定義のみ"
      }
    },
    "tools": {
      "setName": "名前を設定",
      "setNameDesc": "キャラクターの名前を設定",
      "setDefinition": "定義を設定",
      "setDefinitionDesc": "性格と背景を設定",
      "set_character_name": {
        "name": "名前を設定",
        "desc": "キャラクターの名前を設定"
      },
      "set_character_definition": {
        "name": "定義を設定",
        "desc": "性格と背景を設定"
      },
      "add_scene": {
        "name": "シーンを追加",
        "desc": "ロールプレイ用の開始シーンを追加"
      },
      "update_scene": {
        "name": "シーンを更新",
        "desc": "既存のシーンを変更"
      },
      "toggle_avatar_gradient": {
        "name": "アバターグラデーション",
        "desc": "アバターのグラデーションオーバーレイを切替"
      },
      "set_default_model": {
        "name": "モデルを設定",
        "desc": "会話用のAIモデルを設定"
      },
      "set_system_prompt": {
        "name": "システムプロンプト",
        "desc": "行動ガイドラインを設定"
      },
      "get_system_prompt_list": {
        "name": "プロンプト一覧",
        "desc": "利用可能なプロンプトを表示"
      },
      "get_model_list": {
        "name": "モデル一覧",
        "desc": "利用可能なモデルを表示"
      },
      "use_uploaded_image_as_avatar": {
        "name": "画像をアバターに",
        "desc": "アップロード画像をアバターとして使用"
      },
      "use_uploaded_image_as_chat_background": {
        "name": "画像を背景に",
        "desc": "アップロード画像を背景として使用"
      },
      "generate_image": {
        "name": "画像を生成",
        "desc": "AIモデルで画像を生成"
      },
      "show_preview": {
        "name": "プレビューを表示",
        "desc": "キャラクターをプレビュー"
      },
      "request_confirmation": {
        "name": "確認をリクエスト",
        "desc": "保存または続行の確認"
      },
      "list_personas": {
        "name": "ペルソナ一覧",
        "desc": "ペルソナを閲覧"
      },
      "upsert_persona": {
        "name": "ペルソナを保存",
        "desc": "ペルソナを作成または更新"
      },
      "use_uploaded_image_as_persona_avatar": {
        "name": "ペルソナアバター",
        "desc": "アップロード画像をペルソナアバターとして使用"
      },
      "delete_persona": {
        "name": "ペルソナを削除",
        "desc": "ペルソナを削除"
      },
      "get_default_persona": {
        "name": "デフォルトペルソナ",
        "desc": "デフォルトペルソナを取得"
      },
      "list_lorebooks": {
        "name": "ロアブック一覧",
        "desc": "ロアブックを閲覧"
      },
      "upsert_lorebook": {
        "name": "ロアブックを保存",
        "desc": "ロアブックを作成または更新"
      },
      "delete_lorebook": {
        "name": "ロアブックを削除",
        "desc": "ロアブックを削除"
      },
      "list_lorebook_entries": {
        "name": "エントリ一覧",
        "desc": "ロアブックエントリを表示"
      },
      "get_lorebook_entry": {
        "name": "エントリを取得",
        "desc": "ロアブックエントリを取得"
      },
      "upsert_lorebook_entry": {
        "name": "エントリを保存",
        "desc": "エントリを作成または更新"
      },
      "delete_lorebook_entry": {
        "name": "エントリを削除",
        "desc": "ロアブックエントリを削除"
      },
      "create_blank_lorebook_entry": {
        "name": "空エントリ",
        "desc": "プレースホルダーエントリを作成"
      },
      "reorder_lorebook_entries": {
        "name": "エントリを並替",
        "desc": "エントリの順序を変更"
      },
      "list_character_lorebooks": {
        "name": "キャラクターロアブック一覧",
        "desc": "キャラクターのロアブックを表示"
      },
      "set_character_lorebooks": {
        "name": "キャラクターロアブックを設定",
        "desc": "キャラクターにロアブックを割り当て"
      },
      "addScene": "シーンを追加",
      "addSceneDesc": "ロールプレイ用の開始シーンを追加します",
      "updateScene": "シーンを更新",
      "updateSceneDesc": "既存のシーンを変更します",
      "avatarGradient": "アバターグラデーション",
      "avatarGradientDesc": "アバターのグラデーションオーバーレイを切り替えます",
      "setModel": "モデルを設定",
      "setModelDesc": "会話用のAIモデルを設定します",
      "systemPrompt": "システムプロンプト",
      "systemPromptDesc": "動作ガイドラインを設定します",
      "listPrompts": "プロンプト一覧",
      "listPromptsDesc": "利用可能なプロンプトを表示します",
      "listModels": "モデル一覧",
      "listModelsDesc": "利用可能なモデルを表示します",
      "imageAsAvatar": "画像をアバターに",
      "imageAsAvatarDesc": "アップロードした画像をアバターとして使用します"
    }
  },
  "tour": {
    "stepCounter": "ステップ {{current}} / {{total}}",
    "skipTour": "ツアーをスキップ",
    "next": "次へ",
    "gotIt": "了解",
    "appShell": {
      "chats": {
        "title": "チャットはここにあります",
        "body": "キャラクターとの1対1の会話がすべてここにあります。いつでも戻って続きから再開できます。"
      },
      "groups": {
        "title": "グループチャット",
        "body": "複数のキャラクターを同じ部屋に集めて会話を観察したり、好きなタイミングで自分も参加できます。"
      },
      "discover": {
        "title": "新しいキャラクターを見つけよう",
        "body": "コミュニティが共有したものを閲覧して、気に入ったキャラクターを追加しましょう。新しいお気に入りがワンタップで見つかります。"
      },
      "library": {
        "title": "あなたのライブラリ",
        "body": "作成・保存したものがすべてここにあります：キャラクター、ペルソナ、プロンプトなど。あなたのコレクションです。"
      },
      "settings": {
        "title": "自分好みにカスタマイズ",
        "body": "プロバイダーの変更、モデルの選択、見た目の調整。ほぼすべてが設定から変更できます。"
      },
      "search": {
        "title": "なんでも素早く検索",
        "body": "特定のチャットやキャラクターを探していますか？ここからすべてを検索できます。掘り下げる必要はありません。"
      },
      "create": {
        "title": "最後に、作成しよう！",
        "body": "インスピレーションが湧いたらプラスボタンをタップ。新しいキャラクター、ペルソナを作成したり、ゼロから始められます。"
      }
    },
    "chatDetail": {
      "chatTitle": {
        "title": "チャットごとの設定",
        "body": "上部のキャラクター名をタップすると、このチャット専用の設定を開きます。会話ごとに異なるペルソナ、レイアウト、モデルを選択できます。"
      },
      "chatMemory": {
        "title": "記憶していること",
        "body": "脳のアイコンは、キャラクターがあなたとの会話から覚えていることを表示します。タップして記憶を確認、編集、またはクリアできます。"
      },
      "chatSearch": {
        "title": "あの一行を見つける",
        "body": "この会話内だけを検索します。200メッセージ前のあの詳細を、永遠にスクロールせずに見つけられます。"
      },
      "chatLorebook": {
        "title": "ロアブックエントリ",
        "body": "特定のキーワードが出現したときにプロンプトに挿入される追加事実、世界観構築、コンテキスト。キャラクターのカンニングペーパーです。"
      },
      "chatPlus": {
        "title": "ファイルを添付",
        "body": "画像を追加したり、エクストラメニューを開いたりできます。添付したものは次のメッセージと一緒に送信されます。"
      },
      "chatComposer": {
        "title": "あなたのメッセージ、あなたの番",
        "body": "ここに入力します。Enterで送信、Shift+Enterで改行。ヒント：チャット内のメッセージを長押しすると、編集、分岐、削除ができます。"
      },
      "chatSend": {
        "title": "1つのボタン、4つの役割",
        "body": "送信ボタンは状況に応じて役割が変わります："
      }
    },
    "postFirstMessage": {
      "chatRegenerate": {
        "title": "不満？再生成",
        "body": "更新アイコンをタップすると、キャラクターからまったく新しい返答が得られます。各再生成はバリアントとして保存され、後で見返せます。"
      },
      "chatVariants": {
        "title": "バリアント間をスワイプ",
        "body": "再生成後、メッセージの下にバリアントカウンターが表示されます。メッセージバブルを左右にスワイプして、異なる返答を閲覧できます。"
      },
      "chatLongPress": {
        "title": "まだ隠れた機能があります",
        "body": "メッセージを長押しすると、編集、コピー、分岐、ピン留め、削除、または会話の巻き戻しができます。デスクトップでは右クリックも使えます。"
      }
    },
    "sendButton": {
      "continue": {
        "label": "続ける",
        "desc": "入力が空です。ここをタップするとキャラクターに話し続けるよう促します。"
      },
      "send": {
        "label": "送信",
        "desc": "テキストを入力したか、何かを添付しました。タップして送信します。"
      },
      "sending": {
        "label": "送信中",
        "desc": "返答が送信中です。ボタンはロックされています。"
      },
      "stop": {
        "label": "停止",
        "desc": "気が変わった場合、返答の途中でタップしてキャンセルします。"
      }
    },
    "extra": {
      "rerunOnboarding": "オンボーディングを再実行"
    }
  },
  "sessionAdvanced": {
    "title": "セッションパラメータ",
    "subtitle": "この会話のモデルデフォルトを上書き",
    "goBack": "戻る",
    "support": "サポート",
    "reset": "リセット",
    "save": "保存",
    "noSessionWarning": "セッションごとの設定を構成するには、チャットセッションを開いてください。",
    "overrideDefaults": "デフォルトを上書き",
    "overrideDefaultsDesc": "この会話専用にパラメータをカスタマイズ",
    "loadingContextInfo": "コンテキスト情報を読み込み中...",
    "sampling": {
      "title": "サンプリング",
      "temperature": "Temperature",
      "temperatureDesc": "ランダム性を制御します。低い = より確定的、高い = よりクリエイティブ。",
      "temperaturePrecise": "精密",
      "temperatureCreative": "クリエイティブ",
      "topP": "Top P",
      "topPDesc": "Nucleusサンプリング。トークンを累積確率に制限します。",
      "topPFocused": "集中",
      "topPDiverse": "多様",
      "topK": "Top K",
      "topKDesc": "最も可能性の高い上位Kトークンにサンプリングを制限します。"
    },
    "outputPenalties": {
      "title": "出力とペナルティ",
      "maxOutputTokens": "最大出力トークン数",
      "maxOutputTokensDesc": "最大応答長。Autoでモデルが決定します。",
      "auto": "Auto",
      "custom": "カスタム",
      "frequencyPenalty": "頻度ペナルティ",
      "frequencyPenaltyDesc": "トークンシーケンスの繰り返しを減らします。",
      "frequencyPenaltyRepeat": "繰り返し",
      "frequencyPenaltyVary": "変化",
      "presencePenalty": "存在ペナルティ",
      "presencePenaltyDesc": "新しいトピックの探索を促進します。",
      "presencePenaltyRepeat": "繰り返し",
      "presencePenaltyExplore": "探索"
    },
    "performance": {
      "title": "パフォーマンス",
      "gpuLayers": "GPUレイヤー",
      "gpuLayersDesc": "GPUにオフロードするレイヤー数。0 = CPUのみ。",
      "threads": "スレッド",
      "threadsDesc": "推論用CPUスレッド数。",
      "batchThreads": "バッチスレッド",
      "batchThreadsDesc": "バッチ処理用CPUスレッド数。",
      "batchSize": "バッチサイズ",
      "batchSizeDesc": "プロンプト処理のチャンクサイズ。",
      "contextLength": "コンテキスト長",
      "contextLengthDesc": "コンテキストウィンドウサイズを上書き。",
      "flashAttention": "Flash Attention",
      "flashAttentionDesc": "GPUメモリ最適化。",
      "enabled": "有効",
      "disabled": "無効"
    },
    "samplingMemory": {
      "title": "サンプリングとメモリ",
      "minP": "Min P",
      "minPDesc": "最小確率しきい値。",
      "typicalP": "Typical P",
      "typicalPDesc": "典型的サンプリングしきい値。",
      "seed": "Seed",
      "seedDesc": "ランダムシード。空白でランダム。",
      "ropeBase": "RoPE Base",
      "ropeBaseDesc": "周波数ベースの上書き。",
      "ropeScale": "RoPE Scale",
      "ropeScaleDesc": "周波数スケールの上書き。",
      "kvCacheType": "KV Cache Type",
      "kvCacheTypeDesc": "KV Cacheを量子化してVRAMを節約。",
      "offloadKqv": "Offload KQV",
      "offloadKqvDesc": "KV CacheとKQV操作をGPUで実行。",
      "on": "オン",
      "off": "オフ",
      "samplerProfile": "サンプラープロファイル",
      "samplerProfileDesc": "安定性またはReasoningに調整されたデフォルト。",
      "balanced": "バランス",
      "creative": "クリエイティブ",
      "stable": "安定",
      "reasoning": "Reasoning"
    },
    "systemInfo": {
      "title": "システム情報",
      "maxContext": "最大コンテキスト",
      "recommended": "推奨",
      "availableRam": "利用可能RAM",
      "availableVram": "利用可能VRAM",
      "modelSize": "モデルサイズ"
    }
  },
  "exportMenu": {
    "title": "エクスポート形式",
    "selectFormat": "形式を選択",
    "uscTitle": "Unified System Card",
    "formats": {
      "uscPrompt": "プロンプトテンプレート用のポータブルUSCエクスポート。",
      "uscLorebook": "ロアブック用のポータブルUSCエクスポート。",
      "uscModel": "モデルプロファイル用のポータブルUSCエクスポート。",
      "uscChatTemplate": "チャットテンプレート用のポータブルUSCエクスポート。",
      "legacyPromptJson": "Legacy Prompt JSON",
      "legacyPromptJsonDesc": "現行の外部プロンプトパック形式。",
      "lorebookJson": "Lorebook JSON",
      "lorebookJsonDesc": "現行のロアブックエクスポート形式。",
      "modelJson": "Model JSON",
      "modelJsonDesc": "認証情報なしの安全なモデルプロファイルJSON。",
      "chatTemplateJson": "Chat Template JSON",
      "chatTemplateJsonDesc": "ネイティブのチャットテンプレートエクスポート形式。"
    },
    "extra": {
      "selectFormat": "形式を選択",
      "exportFormatTitle": "エクスポート形式",
      "uscDesc": "ポータブルUSCエクスポート",
      "legacyJsonDesc": "レガシーJSONエクスポート形式",
      "formatV3Desc": "Character Card V3エクスポート",
      "formatV2Desc": "Character Card V2エクスポート",
      "formatV1Desc": "Character Card V1エクスポート",
      "uscLorebook": "Unified System Card (USC)",
      "portableLorebook": "ロアブック用ポータブルUSCエクスポート",
      "lorebookJson": "ロアブックJSON",
      "currentLorebook": "現在のロアブックエクスポート形式",
      "uscModel": "Unified System Card (USC)",
      "portableModel": "モデルプロファイル用ポータブルUSCエクスポート",
      "modelJson": "モデルJSON",
      "safeModel": "認証情報を含まない安全なモデルプロファイルJSON",
      "uscTemplate": "Unified System Card (USC)",
      "portableTemplate": "チャットテンプレート用ポータブルUSCエクスポート",
      "templateJson": "チャットテンプレートJSON",
      "nativeTemplate": "ネイティブチャットテンプレートエクスポート形式"
    }
  },
  "designReference": {
    "title": "デザインリファレンス",
    "description": "いくつかの明確なリファレンス画像と、1つの正規のビジュアル説明をアップロードしてください。",
    "descriptionPlaceholder": "安定した外見を記述してください：顔、髪、体格、年齢の見た目、服装の手がかり、アクセサリー、アート/スタイルの方向性。",
    "addReferences": "リファレンスを追加",
    "visualDescription": "ビジュアル説明",
    "draftWithAi": "AIで下書き",
    "referenceImages": "リファレンス画像",
    "imageAlt": "デザインリファレンス {{index}}",
    "loading": "読み込み中...",
    "removeAria": "デザインリファレンスを削除",
    "noImages": "リファレンス画像はまだ添付されていません",
    "imageCount": "{{count}}枚のリファレンス画像が添付されています",
    "emptyReferences": "顔、プロポーション、服装、スタイルを固定するために、いくつかの明確なリファレンス写真を追加してください。",
    "noWriterModel": "まず画像生成設定で互換性のあるシーンライターモデルを追加してください。",
    "noImagesForGeneration": "生成前にアバターまたは少なくとも1つのリファレンス画像を追加してください。",
    "writerModelHelp": "{{model}}を使用してアバターとリファレンス画像から下書きを作成します。",
    "noWriterModelHelp": "これを自動的に下書きするには、画像生成設定で互換性のあるシーンライターモデルを追加してください。",
    "draftMenuTitle": "AIデザイン下書き",
    "draftMenuDesc": "現在のアバターとリファレンス画像から{{model}}が下書きしました。",
    "draftMenuNoWriter": "このヘルパーを使用する前に、互換性のあるシーンライターモデルを追加してください。",
    "regenerate": "再生成",
    "useThis": "これを使用"
  },
  "samplerOrder": {
    "title": "サンプラー順序",
    "description": "ステージをドラッグして並べ替え。上から下に実行されます。",
    "reset": "リセット",
    "resetAria": "サンプラー順序をデフォルトにリセット",
    "stages": {
      "penalties": {
        "label": "ペナルティ",
        "desc": "フィルタリング前に頻度ペナルティと存在ペナルティを適用。"
      },
      "grammar": {
        "label": "文法",
        "desc": "ネイティブのチャットテンプレート文法がアクティブな場合にトークンを制約。"
      },
      "topK": {
        "label": "Top K",
        "desc": "候補プールを最も強力なトークンに絞り込む。"
      },
      "topP": {
        "label": "Top P",
        "desc": "残りの分布にNucleusフィルタリングを適用。"
      },
      "minP": {
        "label": "Min P",
        "desc": "最小フロアを使用して低確率のテールトークンを除去。"
      },
      "typical": {
        "label": "Typical P",
        "desc": "現在の分布で統計的に典型的なトークンを優先。"
      },
      "temp": {
        "label": "Temperature",
        "desc": "選択前に最終分布を平坦化またはシャープ化。"
      }
    },
    "presets": {
      "default": {
        "label": "デフォルト",
        "hint": "Lettuce local"
      },
      "unsloth": {
        "label": "Unsloth",
        "hint": "llama.cppスタイル"
      },
      "focused": {
        "label": "集中",
        "hint": "タイトプルーニング"
      },
      "creative": {
        "label": "クリエイティブ",
        "hint": "レイトフィルター"
      }
    }
  },
  "lorebookGen": {
    "flow": {
      "pickerCharactersTitle": "キャラクター",
      "pickerSessionsTitle": "セッション",
      "noCharacters": "キャラクターがありません",
      "noSessions": "セッションがありません",
      "clearSelection": "選択をクリア",
      "directionTitle": "オプションの生成方向",
      "directionLabel": "方向性",
      "toggleForceMode": "強制モードを切り替え",
      "entryTitlePlaceholder": "エントリータイトル",
      "entryContentPlaceholder": "ロアブックエントリーの内容",
      "editDirectionBeforeRegenerate": "再生成前に方向性を編集",
      "generatorReturnedNoDraft": "ジェネレーターが下書きを返しませんでした",
      "pageTitle": "ロアブックエントリーを生成",
      "missingContext": "ジェネレーターページのロアブックコンテキストがありません。",
      "characterLocked": "キャラクターはこのロアブックの所有者に固定されています",
      "chooseSession": "セッションを選択",
      "pickCharacter": "キャラクターを選択",
      "searchMemories": "メモリを検索",
      "searchMessages": "メッセージを検索",
      "selectLastN": "最後の{{n}}件のメッセージを選択",
      "selectAll": "すべて選択",
      "loadSessionPrompt": "読み込むセッションを選択",
      "messagesText": "メッセージ",
      "memoriesText": "メモリ",
      "messagesAndMemories": "メッセージとメモリ",
      "titleAndContentRequired": "タイトルと内容は必須です",
      "keywordsOrAlwaysActive": "少なくとも1つのキーワードを追加するか、常時アクティブを有効にしてください",
      "lorybookEntrySaved": "ロアブックエントリーを保存しました",
      "saveFailed": "保存に失敗しました",
      "generationFailed": "生成に失敗しました",
      "failedToLoadContext": "ロアブックジェネレーターの読み込みに失敗しました",
      "failedToLoadSessions": "セッションの読み込みに失敗しました",
      "failedToLoadMessages": "メッセージの読み込みに失敗しました",
      "userRole": "ユーザー",
      "aiRole": "AI"
    },
    "triggerPreview": {
      "resizeInspector": "インスペクターのサイズを変更"
    }
  },
  "companion": {
    "download": {
      "emotionClassifier": "感情分類器",
      "emotionClassifierDesc": "ターンを読み取り、コンパニオンが感じている、表現された、ブロックされた感情ベクトルを更新します。",
      "emotionSize": "約120 MB",
      "entityExtractor": "エンティティ抽出器(NER)",
      "entityExtractorDesc": "メモリを正規化・関連付けできるよう、人物・場所・物体を識別します。",
      "entitySize": "約140 MB",
      "memoryRouter": "メモリルーター",
      "memoryRouterDesc": "新しいターンをリレーションシップ、マイルストーン、エピソード、その他のメモリカテゴリのどれとして保存するかを決定します。",
      "routerSize": "約70 MB",
      "unknownModel": "不明なコンパニオンモデルです。?kind=emotion|ner|routerを指定してください。"
    }
  },
  "voices": {
    "extra": {
      "customVoices": "マイ音声",
      "providerVoices": "プロバイダー音声",
      "myVoices": "マイ音声",
      "page": {
        "noAudioProvidersHint": "プロバイダー → 音声でプロバイダーを追加して開始してください",
        "noVoicesTitle": "音声がまだ作成されていません",
        "noVoicesDescription": "キャラクター用にカスタムプロンプトで音声を作成しましょう",
        "addProviderFirst": "まず音声プロバイダーを追加してください",
        "noPrompt": "プロンプトなし",
        "noProviderVoices": "音声が見つかりません。更新をタップしてください。",
        "showLess": "少なく表示",
        "showAllVoices": "すべての{{count}}件の音声を表示",
        "voiceFallbackTitle": "音声"
      },
      "cache": {
        "section": "音声キャッシュ",
        "title": "TTS音声キャッシュ",
        "description": "生成された音声はキャッシュされ、再生成を減らします",
        "clearing": "クリア中...",
        "clear": "キャッシュをクリア"
      },
      "menu": {
        "editDescription": "この音声を編集",
        "deleteDescription": "この音声を削除",
        "provider": "プロバイダー",
        "category": "カテゴリー",
        "createVoiceConfig": "音声設定を作成",
        "createVoiceConfigDescription": "この音声をカスタム設定で使用"
      },
      "editor": {
        "editTitle": "音声を編集",
        "createTitle": "音声を作成",
        "voiceName": "音声名",
        "voiceNamePlaceholder": "マイキャラクター音声",
        "provider": "プロバイダー",
        "model": "モデル",
        "modelIdPlaceholder": "gpt-4o-mini-tts",
        "modelIdHint": "互換エンドポイントが期待する正確なモデルIDを入力してください",
        "elevenlabsVoice": "ElevenLabs音声",
        "noVoicesAvailable": "利用可能な音声がありません",
        "selectVoice": "音声を選択...",
        "elevenlabsVoiceHint": "ElevenLabsの音声から選択",
        "geminiVoice": "Gemini音声",
        "geminiVoiceHint": "Gemini TTSの音声を選択",
        "voiceId": "音声ID",
        "voiceIdPlaceholder": "alloy",
        "voiceIdHint": "互換エンドポイントがサポートする音声IDを入力してください",
        "voicePrompt": "音声プロンプト",
        "voicePromptPlaceholder": "温かく親しみやすい声で、明るいトーンで...",
        "voicePromptHint": "音声の聞こえ方を説明してください",
        "exampleText": "サンプルテキスト",
        "exampleTextPlaceholder": "こんにちは！これが私の話し声です...",
        "exampleTextHint": "音声テスト用のサンプルテキスト",
        "voiceDesignChars": "音声デザインプレビューには{{current}}/{{minimum}}文字が必要です",
        "defaultSample": "こんにちは！これが私の話し声です。温かさ、明瞭さ、感情を込めてより長い文章を読み上げることができます。",
        "playing": "再生中...",
        "previewVoice": "音声をプレビュー"
      },
      "kokoro": {
        "title": "Kokoro Studio",
        "newBlend": "新しいブレンド",
        "editBlend": "ブレンドを編集",
        "tryText": "こんにちは！これは私の声の簡単なテストです。",
        "experimentDefaultText": "こんにちは！これが私の話し声です。温かさ、明瞭さ、感情を込めてより長い文章を読み上げることができます。",
        "livePreview": "ライブプレビュー",
        "savedBlend": "保存済みブレンド",
        "defaultPreviewText": "こんにちは！これはこの音声の簡単なプレビューです。",
        "experiment": "実験",
        "providerNotFound": "Kokoroプロバイダーが見つかりません",
        "backToProviders": "プロバイダーに戻る",
        "variantUnset": "バリアント未設定",
        "ready": "準備完了",
        "modelNotInstalled": "モデルがインストールされていません",
        "voiceCount": "{{count}}件の音声",
        "engineActions": "エンジン操作",
        "engineNotInstalled": "エンジンがインストールされていません",
        "installAtLeastOneVoice": "少なくとも1つの音声をインストールしてください",
        "continueSetup": "セットアップを続けてKokoroモデルをインストールしてください。",
        "pickVoiceOrStarter": "音声を選ぶか、スターターパックを入手して始めましょう。",
        "downloadsFailed": "{{count}}件のダウンロードが失敗しました",
        "retryOrDismissAll": "個別に再試行するか、すべて閉じてください。",
        "dismissAll": "すべて閉じる",
        "model": "モデル",
        "voice": "音声",
        "downloads": "ダウンロード",
        "cancelAll": "すべてキャンセル",
        "experimentPlaceholder": "フレーズを入力して音声を聴く...",
        "speed": "速度",
        "speak": "再生",
        "yourBlends": "あなたのブレンド",
        "noSavedBlends": "保存済みブレンドはまだありません。",
        "installModelAndVoiceFirst": "まずモデルと音声をインストールしてください。",
        "featured": "おすすめ",
        "stop": "停止",
        "sample": "サンプル",
        "voiceLibrary": "音声ライブラリ",
        "starterPack": "スターターパック",
        "select": "選択",
        "all": "すべて",
        "installed": "インストール済み",
        "installModelToBrowse": "音声を閲覧するにはモデルをインストールしてください。",
        "noVoicesInCatalog": "カタログに音声がありません。更新してください。",
        "noVoicesMatch": "フィルターに一致する音声がありません。",
        "collapseAll": "すべて折りたたむ",
        "expandAll": "すべて展開する",
        "selectedCount": "{{count}}件選択中",
        "engineTitle": "Kokoroエンジン",
        "variant": "バリアント",
        "status": "ステータス",
        "notInstalled": "未インストール",
        "file": "ファイル",
        "modelSize": "モデルサイズ",
        "voicesSize": "音声サイズ",
        "total": "合計",
        "assetRoot": "アセットルート",
        "reinstallModel": "モデルを再インストール",
        "installModel": "モデルをインストール",
        "deleteModel": "モデルを削除",
        "deleteModelDescription": "ディスクを解放します。音声は保持されます。",
        "blend": "ブレンド",
        "previewDescription": "デフォルトテキストで試聴",
        "editBlendDescription": "音声、ウェイト、速度を調整",
        "deleteBlendDescription": "この保存済み音声を削除",
        "setupTitle": "Kokoroをセットアップ",
        "allSet": "準備完了",
        "allSetDescription": "音声を閲覧したり、ブレンドをデザインしたり、実験エリアでテストしましょう。"
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
      "conditionalsSection": "条件",
      "injectionPoints": "インジェクションポイント"
    }
  },
  "embeddings": {
    "download": {
      "bestForQuick": "素早い応答に最適",
      "balancedPerf": "バランスの取れたパフォーマンス",
      "maxContext": "最大コンテキスト",
      "capacity1k": "1Kトークン",
      "capacity2k": "2Kトークン",
      "capacity4k": "4Kトークン",
      "approxSize": "約138 MB",
      "downloadVersion": "V4"
    },
    "test": {
      "health": "ヘルスチェック",
      "retrieval": "検索",
      "separation": "分離",
      "passed": "合格",
      "failed": "不合格",
      "testing": "テスト中..."
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
      "jsonDesc": "ツール呼び出しが利用できない場合のコンパクトな構造化出力。",
      "xml": "XML",
      "xmlDesc": "モデルがJSONよりもXMLをより確実にフォーマットする場合に使用します。",
      "fallbackFormat": "フォールバック形式"
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
      "filters": "フィルター",
      "model": "モデル",
      "character": "キャラクター",
      "clearAll": "すべてクリア",
      "applyFilters": "フィルターを適用",
      "recentActivity": "最近のアクティビティ",
      "customRange": "カスタム範囲",
      "startDate": "開始日",
      "endDate": "終了日",
      "applyRange": "範囲を適用",
      "dashboard": "ダッシュボード",
      "appTime": "アプリ使用時間",
      "today": "今日",
      "last7Days": "7日間",
      "last30Days": "30日間",
      "all": "すべて",
      "custom": "カスタム",
      "filtersCount": "{{count}} 件のフィルター",
      "totalCost": "合計コスト",
      "tokens": "トークン",
      "avgShort": "{{value}} 平均",
      "requests": "リクエスト",
      "period": "期間",
      "last7d": "過去7日",
      "last30d": "過去30日",
      "allTime": "全期間",
      "recordsCount": "{{count}} 件のレコード",
      "usageTrend": "使用トレンド",
      "tokenConsumptionOverTime": "時間経過によるトークン消費量",
      "input": "入力",
      "output": "出力",
      "byModel": "モデル別",
      "byCharacter": "キャラクター別",
      "top": "上位",
      "active": "アクティブ",
      "quickView": "クイックビュー",
      "viewAll": "すべて表示",
      "startChatting": "チャットを開始すると使用データが表示されます",
      "exportedTo": "エクスポート先：{{path}}",
      "periodTotal": "期間合計",
      "daysActive": "{{count}} 日間アクティブ",
      "dailyAvg": "1日平均",
      "selectedPeriod": "選択期間",
      "yesterdayValue": "昨日 {{value}}",
      "thirtyDayAvg": "30日平均",
      "appTimeTrend": "アプリ使用時間トレンド",
      "usageDurationPerDay": "1日あたりの使用時間",
      "totalValue": "合計 {{value}}",
      "activeTime": "アクティブ時間"
    },
    "activity": {
      "loading": "アクティビティを読み込んでいます...",
      "title": "最近のアクティビティ",
      "recordsCount": "{{count}} 件の使用レコード",
      "rangeOfTotal": "{{total}} 件中 {{start}}〜{{end}} 件",
      "previous": "前へ",
      "next": "次へ",
      "pageOf": "{{total}} ページ中 {{page}} ページ目"
    },
    "shared": {
      "relativeTime": {
        "justNow": "たった今",
        "minutesAgo": "{{count}} 分前",
        "hoursAgo": "{{count}} 時間前",
        "daysAgo": "{{count}} 日前"
      },
      "operations": {
        "chat": "チャット",
        "regenerate": "再生成",
        "continue": "続ける",
        "summary": "要約",
        "memoryManager": "メモリ",
        "imageGeneration": "画像生成",
        "aiCreator": "AIクリエイター",
        "replyHelper": "返信ヘルパー",
        "groupChatMessage": "グループチャット",
        "groupChatRegenerate": "グループ再生成",
        "groupChatContinue": "グループ続行",
        "groupChatDecisionMaker": "決定役"
      },
      "outputImages": "{{count}} 枚の画像",
      "tokens": "{{count}} トークン",
      "unknown": "不明",
      "requestDetails": "リクエスト詳細",
      "noStopReason": "停止理由なし",
      "tokenUsage": "トークン使用量",
      "estimatedCost": "推定コスト",
      "stats": {
        "prompt": "プロンプト",
        "completion": "補完",
        "total": "合計",
        "reasoning": "推論",
        "image": "画像",
        "memory": "メモリ",
        "summary": "要約",
        "inputImages": "入力画像",
        "outputImages": "出力画像",
        "cachedPrompt": "キャッシュ済みプロンプト",
        "cacheWrite": "キャッシュ書き込み",
        "webSearches": "ウェブ検索",
        "cacheRead": "キャッシュ読み取り",
        "requestFee": "リクエスト料金",
        "webSearch": "ウェブ検索",
        "providerTotal": "プロバイダー合計"
      }
    }
  }
};
