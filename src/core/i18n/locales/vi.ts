import type { DeepPartialMessageTree } from "../types";
import { type LocaleMessages } from "./en";
import type { LocaleMetadata } from ".";

export const viMetadata: LocaleMetadata = {
  name: "Vietnamese",
  label: "Tiếng Việt",
};

export const viMessages: DeepPartialMessageTree<LocaleMessages> = {
  "common": {
    "nav": {
      "chats": "Trò chuyện",
      "settings": "Cài đặt",
      "providers": "Nhà cung cấp",
      "responseStyle": "Phong cách phản hồi",
      "models": "Mô hình",
      "security": "Bảo mật",
      "accessibility": "Trợ năng",
      "reset": "Đặt lại",
      "backupRestore": "Sao lưu & Khôi phục",
      "convertFiles": "Chuyển đổi tệp",
      "usageAnalytics": "Phân tích sử dụng",
      "changelog": "Nhật ký thay đổi",
      "about": "Giới thiệu",
      "createSystemPrompt": "Tạo lời nhắc hệ thống",
      "editSystemPrompt": "Sửa lời nhắc hệ thống",
      "systemPrompts": "Lời nhắc hệ thống",
      "developer": "Nhà phát triển",
      "advanced": "Nâng cao",
      "characters": "Nhân vật",
      "lorebooks": "Sách lore",
      "personas": "Persona",
      "dynamicMemory": "Bộ nhớ động",
      "creationHelper": "Trợ lý tạo",
      "helpMeReply": "Giúp tôi trả lời",
      "editPersona": "Sửa Persona",
      "newTemplate": "Mẫu mới",
      "editTemplate": "Sửa mẫu",
      "chatTemplates": "Mẫu trò chuyện",
      "editCharacter": "Sửa nhân vật",
      "sync": "Đồng bộ",
      "newCharacter": "Nhân vật mới",
      "engineSetup": "Thiết lập Engine",
      "llmProviders": "Nhà cung cấp LLM",
      "engineSettings": "Cài đặt Engine",
      "lettuceEngine": "Lettuce Engine",
      "create": "Tạo",
      "setup": "Thiết lập",
      "welcome": "Chào mừng",
      "conversation": "Cuộc trò chuyện",
      "library": "Thư viện",
      "groupChats": "Nhóm trò chuyện",
      "groupChat": "Nhóm trò chuyện",
      "imageGeneration": "Tạo hình ảnh",
      "voices": "Giọng nói",
      "chatAppearance": "Giao diện trò chuyện",
      "colorCustomization": "Tùy chỉnh màu sắc",
      "embeddingDownload": "Tải Embedding",
      "embeddingTest": "Kiểm tra Embedding",
      "editModel": "Sửa mô hình",
      "messageDebug": "Gỡ lỗi tin nhắn",
      "speechRecognition": "Speech Recognition",
      "hostApi": "API Server",
      "lorebookEntryGenerator": "Lorebook Entry Generator",
      "companionSoulWriter": "Trình viết Companion Soul"
    },
    "bottomNav": {
      "chats": "Trò chuyện",
      "groups": "Nhóm",
      "create": "Tạo",
      "discover": "Khám phá",
      "library": "Thư viện"
    },
    "buttons": {
      "cancel": "Hủy",
      "confirm": "Xác nhận",
      "save": "Lưu",
      "saving": "Đang lưu...",
      "delete": "Xóa",
      "deleting": "Đang xóa...",
      "create": "Tạo",
      "creating": "Đang tạo...",
      "edit": "Sửa",
      "back": "Quay lại",
      "done": "Xong",
      "download": "Tải xuống",
      "later": "Để sau",
      "proceed": "Tiếp tục",
      "retry": "Thử lại",
      "discard": "Hủy bỏ",
      "import": "Nhập",
      "importing": "Đang nhập...",
      "export": "Xuất",
      "exporting": "Đang xuất...",
      "update": "Cập nhật",
      "generate": "Tạo",
      "refresh": "Làm mới",
      "continue": "Tiếp tục",
      "goBack": "Quay lại",
      "search": "Tìm kiếm",
      "clearSearch": "Xóa tìm kiếm",
      "add": "Thêm",
      "remove": "Xóa",
      "rename": "Đổi tên",
      "copy": "Sao chép",
      "copied": "Đã sao chép!",
      "browseFiles": "Duyệt tệp",
      "install": "Cài đặt"
    },
    "labels": {
      "processing": "Đang xử lý...",
      "loading": "Đang tải...",
      "noDescriptionYet": "Chưa có mô tả",
      "untitled": "Chưa đặt tên",
      "default": "Mặc định",
      "enabled": "Đã bật",
      "disabled": "Đã tắt",
      "on": "Bật",
      "off": "Tắt",
      "none": "Không có",
      "auto": "Tự động",
      "custom": "Tùy chỉnh",
      "name": "Tên",
      "description": "Mô tả",
      "content": "Nội dung",
      "preview": "Xem trước",
      "options": "Tùy chọn"
    },
    "time": {
      "justNow": "Vừa xong",
      "minutesAgo": "{{minutes}} phút trước",
      "hoursAgo": "{{hours}} giờ trước",
      "daysAgo": "{{days}} ngày trước",
      "today": "Hôm nay",
      "yesterday": "Hôm qua",
      "last7Days": "7 ngày qua",
      "older": "Cũ hơn"
    }
  },
  "settings": {
    "sections": {
      "intelligence": "Trí tuệ",
      "experience": "Trải nghiệm",
      "connectivity": "Kết nối",
      "securityPrivacy": "Bảo mật & Quyền riêng tư",
      "supportInfo": "Hỗ trợ & Thông tin",
      "dangerZone": "Vùng nguy hiểm",
      "developer": "Nhà phát triển"
    },
    "items": {
      "providers": {
        "title": "Nhà cung cấp",
        "subtitle": "Kết nối dịch vụ AI"
      },
      "models": {
        "title": "Mô hình",
        "subtitle": "Cấu hình mô hình AI"
      },
      "imageGeneration": {
        "title": "Tạo hình ảnh",
        "subtitle": "Tạo và kiểm tra hình ảnh"
      },
      "voices": {
        "title": "Giọng nói",
        "subtitle": "Giọng đọc văn bản"
      },
      "accessibility": {
        "title": "Trợ năng",
        "subtitle": "Âm thanh & rung"
      },
      "prompts": {
        "title": "Lời nhắc hệ thống",
        "subtitle": "Định hình tính cách AI"
      },
      "security": {
        "title": "Bảo mật",
        "subtitle": "Mã hóa & quyền riêng tư"
      },
      "backup": {
        "title": "Sao lưu & Khôi phục",
        "subtitle": "Xuất hoặc nhập dữ liệu"
      },
      "convert": {
        "title": "Chuyển đổi tệp",
        "subtitle": "Chuyển đổi .json sang .uec"
      },
      "sync": {
        "title": "Đồng bộ cục bộ",
        "subtitle": "Đồng bộ giữa các thiết bị"
      },
      "usage": {
        "title": "Phân tích sử dụng",
        "subtitle": "Chi phí & thống kê token"
      },
      "advanced": {
        "title": "Nâng cao",
        "subtitle": "Bộ nhớ & tính năng"
      },
      "logs": {
        "title": "Nhật ký",
        "subtitle": "Gỡ lỗi & chẩn đoán"
      },
      "guide": {
        "title": "Hướng dẫn thiết lập",
        "subtitle": "Chạy lại hướng dẫn"
      },
      "docs": {
        "title": "Tài liệu",
        "subtitle": "Hướng dẫn & tham khảo"
      },
      "github": {
        "title": "Báo lỗi",
        "subtitle": "Lỗi & phản hồi • v{{version}}"
      },
      "discord": {
        "title": "Tham gia Discord",
        "subtitle": "Cộng đồng & hỗ trợ"
      },
      "about": {
        "title": "Giới thiệu",
        "subtitle": "Phiên bản, cập nhật & liên kết"
      },
      "changelog": {
        "title": "Nhật ký thay đổi",
        "subtitle": "Có gì mới"
      },
      "reset": {
        "title": "Đặt lại",
        "subtitle": "Xóa tất cả"
      },
      "developer": {
        "title": "Công cụ phát triển",
        "subtitle": "Gỡ lỗi & kiểm thử"
      }
    }
  },
  "accessibility": {
    "sectionTitles": {
      "language": "Ngôn ngữ",
      "sounds": "Phản hồi âm thanh",
      "haptics": "Phản hồi rung",
      "appearance": "Giao diện"
    },
    "language": {
      "appLanguage": "Ngôn ngữ ứng dụng",
      "description": "Chọn ngôn ngữ cho điều hướng và cài đặt."
    },
    "appearance": {
      "customColors": "Tùy chỉnh màu sắc",
      "customColorsDesc": "Cá nhân hóa bảng màu ứng dụng",
      "chatAppearance": "Giao diện trò chuyện",
      "chatAppearanceDesc": "Tùy chỉnh bong bóng tin nhắn, phông chữ và bố cục"
    },
    "haptics": {
      "vibrateOnChat": "Rung khi trò chuyện",
      "vibrateDesc": "Rung nhẹ khi trợ lý đang gõ",
      "intensity": "Cường độ",
      "light": "Nhẹ",
      "medium": "Vừa",
      "heavy": "Mạnh",
      "soft": "Mềm",
      "rigid": "Cứng"
    },
    "sounds": {
      "send": "Gửi",
      "sendDescription": "Phát khi bạn gửi tin nhắn",
      "success": "Thành công",
      "successDescription": "Phát khi trợ lý hoàn thành",
      "failure": "Thất bại",
      "failureDescription": "Phát khi có lỗi hoặc bạn hủy",
      "testButton": "Kiểm tra"
    },
    "feedbackInfo": "Phản hồi giúp bạn nhận biết khi tin nhắn được gửi hoặc nhận.",
    "hapticsInfo": "Rung có sẵn trên thiết bị di động.",
    "extra": {
      "easterEggs": "Easter Eggs",
      "beetrootRain": "Beetroot Rain",
      "beetrootDesc": "Beetroots fall when chats mention them"
    }
  },
  "topNav": {
    "unsavedChangesTitle": "Thay đổi chưa lưu",
    "unsavedChangesMessage": "Lưu hoặc hủy bỏ thay đổi trước khi rời đi.",
    "goBack": "Quay lại",
    "changeLayout": "Đổi bố cục",
    "search": "Tìm kiếm",
    "settings": "Cài đặt",
    "help": "Trợ giúp",
    "add": "Thêm",
    "openFilters": "Mở bộ lọc",
    "save": "Lưu",
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
      "title": "Có phiên bản mới",
      "description": "v{{latestVersion}} đã có sẵn. Bạn đang dùng v{{currentVersion}}.",
      "actions": {
        "view": "Xem bản phát hành"
      }
    }
  },
  "about": {
    "kicker": "Thông tin ứng dụng",
    "appName": "LettuceAI",
    "description": "Chi tiết phiên bản, kiểm tra cập nhật và các liên kết hữu ích.",
    "info": {
      "version": "Phiên bản",
      "channel": "Kênh",
      "platform": "Nền tảng"
    },
    "buildChannel": {
      "dev": "Bản dựng phát triển",
      "release": "Bản phát hành ổn định"
    },
    "update": {
      "sectionTitle": "Cập nhật",
      "title": "Cập nhật ứng dụng",
      "description": "Kiểm tra bản phát hành mới thủ công hoặc tắt kiểm tra tự động khi khởi động.",
      "autoChecks": "Tự động kiểm tra cập nhật",
      "autoChecksDescription": "Khi bật, LettuceAI sẽ kiểm tra phiên bản mới khi mở ứng dụng.",
      "checkNow": "Kiểm tra cập nhật",
      "checking": "Đang kiểm tra cập nhật...",
      "upToDateTitle": "Bạn đang dùng phiên bản mới nhất",
      "upToDateDescription": "Hiện chưa có bản phát hành mới hơn cho thiết bị này.",
      "failedTitle": "Kiểm tra cập nhật thất bại",
      "failedDescription": "Không thể kiểm tra cập nhật ngay bây giờ. Hãy thử lại sau ít phút."
    },
    "links": {
      "sectionTitle": "Liên kết",
      "website": "Trang web",
      "websiteDescription": "Trang tải xuống và thông tin phát hành",
      "github": "GitHub",
      "githubDescription": "Mã nguồn, vấn đề và phát triển",
      "discord": "Discord",
      "discordDescription": "Máy chủ cộng đồng và hỗ trợ",
      "reddit": "Reddit",
      "redditDescription": "Thảo luận cộng đồng, phản hồi và cập nhật"
    },
    "developerMode": {
      "enable": "Bật Chế độ Nhà phát triển",
      "enabled": "Chế độ Nhà phát triển đã bật"
    },
    "errors": {
      "saveTitle": "Không thể lưu tùy chọn",
      "saveDescription": "Tùy chọn kiểm tra cập nhật của bạn không được thay đổi."
    }
  },
  "components": {
    "tooltip": {
      "dismissHint": "Nhấn bất kỳ đâu để đóng"
    },
    "backgroundPosition": {
      "title": "Vị trí nền",
      "instructions": "Kéo để di chuyển • Chụm hoặc cuộn để thu phóng"
    },
    "avatarSource": {
      "generateImage": "Tạo hình ảnh",
      "generateImageDesc": "Tạo avatar bằng AI",
      "noImageModels": "Không có mô hình hình ảnh",
      "editCurrent": "Sửa hiện tại",
      "editCurrentDesc": "Đặt lại vị trí hoặc cắt",
      "chooseImage": "Chọn hình ảnh",
      "chooseImageDesc": "Chọn từ thiết bị"
    },
    "avatarCurrentEdit": {
      "title": "Chỉnh sửa hiện tại",
      "reposition": "Tái định vị",
      "repositionDesc": "Di chuyển hoặc cắt hình đại diện hiện tại",
      "editWithAI": "Chỉnh sửa bằng AI",
      "editWithAIDesc": "Mở AI chỉnh sửa cho avatar hiện tại",
      "noImageModels": "Không có mẫu hình ảnh nào"
    },
    "avatarGeneration": {
      "modelsLoadError": "Không thể tải mô hình tạo hình ảnh",
      "title": "Tạo Avatar",
      "help": "Trợ giúp tạo avatar",
      "model": "Mô hình",
      "selectModel": "Chọn mô hình",
      "describe": "Mô tả avatar của bạn",
      "describePlaceholder": "Cô gái anime thân thiện với mái tóc nhiều màu, mỉm cười ấm áp...",
      "inProgress": "Đang tạo avatar...",
      "editingInProgress": "Đang áp dụng chỉnh sửa hình đại diện...",
      "previousVariant": "Biến thể trước đó",
      "nextVariant": "Biến thể tiếp theo",
      "variantCounter": "{{current}} / {{total}}",
      "editRequest": "Chỉnh sửa yêu cầu",
      "editRequestPlaceholder": "Làm tóc đen hơn, đeo kính, giữ nguyên khuôn mặt...",
      "applyEdit": "Áp dụng Chỉnh sửa",
      "editImageLoadError": "Không thể chuẩn bị hình đại diện đã tạo để chỉnh sửa",
      "aiAssistant": "Trợ lý AI",
      "backToResults": "Quay lại lời nhắc",
      "magicInTheWorks": "Phép thuật trong tác phẩm...",
      "refine": "Tinh chỉnh",
      "apply": "Áp dụng",
      "alt": "Avatar đã tạo",
      "regenerate": "Tạo lại",
      "useThis": "Sử dụng"
    },
    "avatarPosition": {
      "title": "Vị trí Avatar",
      "instructions": "Kéo để di chuyển • Chụm hoặc cuộn để thu phóng",
      "alt": "Avatar cần đặt vị trí"
    },
    "confirmDialog": {
      "defaultTitle": "Xác nhận",
      "defaultLabel": "Xác nhận"
    },
    "bottomMenu": {
      "defaultTitle": "Menu",
      "dragTip": "Kéo để đóng menu",
      "closeLabel": "Đóng menu",
      "buttonProcessing": "Đang xử lý..."
    },
    "modelSelector": {
      "placeholder": "Chọn mô hình",
      "clearLabel": "Sử dụng mặc định chung",
      "loading": "Đang tải mô hình...",
      "noModels": "Không có mô hình",
      "addProviderFirst": "Thêm nhà cung cấp trong cài đặt trước",
      "title": "Chọn mô hình",
      "searchPlaceholder": "Tìm mô hình...",
      "noResults": "Không tìm thấy mô hình",
      "noResultsHint": "Thử từ khóa khác"
    },
    "localeSelector": {
      "title": "Chọn ngôn ngữ"
    },
    "promptTemplate": {
      "nameContentRequired": "Cần nhập tên và nội dung",
      "saveError": "Không thể lưu mẫu",
      "editTitle": "Sửa lời nhắc",
      "createTitle": "Tạo lời nhắc",
      "name": "Tên",
      "namePlaceholder": "VD: Bậc thầy nhập vai",
      "content": "Nội dung",
      "variablesButton": "Biến",
      "contentPlaceholder": "Bạn là một trợ lý AI hữu ích...\n\nSử dụng {{char.name}} và {{scene}} trong lời nhắc.",
      "characterCount": "{{count}} ký tự",
      "preview": "Xem trước",
      "characterPlaceholder": "Nhân vật…",
      "personaPlaceholder": "Persona…",
      "rendering": "Đang dựng…",
      "noPreview": "Chưa có xem trước",
      "saving": "Đang lưu...",
      "update": "Cập nhật",
      "create": "Tạo",
      "variablesTitle": "Biến mẫu",
      "variablesCopyHint": "Nhấn để sao chép",
      "variablesCopied": "Đã sao chép",
      "variables": {
        "charName": "Tên nhân vật",
        "charNameDesc": "Tên của nhân vật",
        "charDesc": "Mô tả nhân vật",
        "charDescDesc": "Mô tả nhân vật",
        "scene": "Bối cảnh",
        "sceneDesc": "Bối cảnh/kịch bản mở đầu",
        "userName": "Tên người dùng",
        "userNameDesc": "Tên persona người dùng",
        "userDesc": "Mô tả người dùng",
        "userDescDesc": "Mô tả persona người dùng",
        "rules": "Quy tắc",
        "rulesDesc": "Quy tắc hành vi nhân vật",
        "contextSummary": "Tóm tắt ngữ cảnh",
        "contextSummaryDesc": "Tóm tắt cuộc trò chuyện động",
        "keyMemories": "Ký ức chính",
        "keyMemoriesDesc": "Danh sách ký ức liên quan"
      }
    },
    "characterExport": {
      "title": "Định dạng xuất",
      "selectFormat": "Chọn định dạng",
      "loading": "Đang tải định dạng...",
      "formatUecDesc": "Định dạng Unified Entity Card (.uec) (khuyến nghị).",
      "formatLegacyJsonDesc": "JSON cũ (chỉ nhập).",
      "formatV3Desc": "Character Card V3 JSON (phiên bản mới nhất).",
      "formatV2Desc": "Character Card V2 JSON (đặc tả Tavern).",
      "formatV1Desc": "Character Card V1 (chỉ nhập)."
    },
    "embeddingDownload": {
      "downloadRequired": "Cần tải xuống",
      "modelRequired": "Cần mô hình Embedding",
      "description": "Bộ nhớ động cần tải mô hình embedding cục bộ (~260 MB) để hoạt động.",
      "localStorage": "• Mô hình sẽ được lưu trữ cục bộ trên thiết bị",
      "downloadSize": "• Kích thước tải: khoảng 260 MB",
      "summarization": "• Cần thiết cho việc tóm tắt cuộc trò chuyện"
    },
    "embeddingUpgrade": {
      "title": "Mô hình Embedding v3 có sẵn",
      "v1Message": "Bạn đang dùng v1 với 512 token. Nâng cấp lên v3 để chất lượng bộ nhớ tốt hơn và hỗ trợ ngữ cảnh dài.",
      "v2Message": "Bạn đang dùng v2 cũ. Nâng cấp lên v3 để chất lượng bộ nhớ tốt hơn với mô hình embedding mới nhất.",
      "button": "Nâng cấp lên v3",
      "v3Message": "v4 is out and dramatically improves roleplay memory recall over v3 (recall@1 0.02 -> 0.92). Upgrading is recommended."
    },
    "v2UpgradeToast": {
      "title": "Mô hình bộ nhớ v3",
      "badge": "Có sẵn",
      "message": "Chất lượng embedding cải thiện so với v2",
      "dismiss": "Bỏ qua",
      "upgrade": "Nâng cấp"
    },
    "v1UpgradeToast": {
      "title": "Mô hình bộ nhớ v3 có sẵn",
      "message": "Nâng cấp để chất lượng bộ nhớ tốt hơn và hỗ trợ ngữ cảnh dài.",
      "dismiss": "Bỏ qua",
      "upgrade": "Nâng cấp"
    },
    "createMenu": {
      "title": "Tạo mới",
      "smartCreator": "Trợ lý thông minh",
      "smartCreatorDesc": "Để trợ lý hướng dẫn bạn tạo",
      "divider": "Hoặc tạo thủ công",
      "character": "Nhân vật",
      "characterDesc": "Tạo nhân vật tùy chỉnh",
      "persona": "Persona",
      "personaDesc": "Tạo giọng nói tái sử dụng",
      "groupChat": "Nhóm trò chuyện",
      "groupChatDesc": "Trò chuyện với nhiều nhân vật",
      "lorebook": "Sách lore",
      "lorebookDesc": "Xây dựng tài liệu tham khảo thế giới",
      "characterSmartDesc": "Tạo nhân vật với hướng dẫn",
      "personaSmartDesc": "Tạo giọng nói hoặc tính cách tái sử dụng",
      "lorebookSmartDesc": "Xây dựng tài liệu thế giới có cấu trúc",
      "loadingConversations": "Đang tải cuộc trò chuyện...",
      "createNew": "Tạo mới",
      "createNewDesc": "Bắt đầu phiên tạo mới",
      "editExisting": "Sửa hiện có",
      "continueLast": "Tiếp tục cuộc trò chuyện cuối",
      "seeOlder": "Xem các cuộc trò chuyện cũ hơn",
      "seeOlderDesc": "Mở cuộc trò chuyện Trợ lý thông minh trước đó",
      "noConversations": "Chưa có cuộc trò chuyện nào cho trợ lý này.",
      "sessionCompleted": "Hoàn thành",
      "sessionCancelled": "Đã hủy",
      "sessionDraft": "Nháp",
      "sessionMessages": "{{count}} tin nhắn",
      "untitledConversation": "Cuộc trò chuyện chưa đặt tên",
      "nameLorebookTitle": "Đặt tên sách lore",
      "lorebookNamePlaceholder": "Nhập tên sách lore...",
      "lorebookImporting": "Đang nhập...",
      "lorebookImport": "Nhập",
      "lorebookCreating": "Đang tạo...",
      "lorebookCreate": "Tạo",
      "editExistingDesc": "Chọn {{goal}} và sửa với Trợ lý thông minh",
      "creatorTitle": "Trợ lý tạo {{goal}}",
      "editTitle": "Sửa {{goal}}",
      "conversationsTitle": "Cuộc trò chuyện {{goal}}",
      "loadingItems": "Đang tải {{items}}...",
      "noItemsFound": "Không tìm thấy {{items}}.",
      "unnamedCharacter": "Nhân vật chưa đặt tên",
      "untitledPersona": "Persona chưa đặt tên",
      "untitledLorebook": "Sách lore chưa đặt tên"
    },
    "advancedModelSettings": {
      "temperature": "Nhiệt độ",
      "temperatureDesc": "Cao hơn = sáng tạo hơn",
      "topP": "Top P",
      "topPDesc": "Thấp hơn = tập trung hơn",
      "maxTokens": "Token đầu ra tối đa",
      "maxTokensDesc": "Để trống cho mặc định",
      "contextLength": "Độ dài ngữ cảnh",
      "contextLengthDesc": "Chỉ mô hình cục bộ",
      "contextLengthAuto": "Tự động",
      "frequencyPenalty": "Phạt tần suất",
      "frequencyPenaltyDesc": "Giảm lặp lại token",
      "presencePenalty": "Phạt hiện diện",
      "presencePenaltyDesc": "Khuyến khích chủ đề mới",
      "topK": "Top K",
      "topKDesc": "Giới hạn nhóm token",
      "reasoning": "Suy luận / Tư duy",
      "reasoningAutoDesc": "Mô hình này luôn sử dụng suy luận. Không cần cấu hình.",
      "reasoningEnableDesc": "Bật khả năng suy luận nâng cao cho các bài toán phức tạp và tác vụ cần tư duy.",
      "effortMode": "Chế độ nỗ lực",
      "budgetMode": "Chế độ ngân sách",
      "reasoningEffort": "Nỗ lực suy luận",
      "reasoningEffortDesc": "Điều khiển độ sâu tư duy",
      "reasoningEffortAuto": "Tự động",
      "reasoningEffortLow": "Thấp",
      "reasoningEffortMed": "Vừa",
      "reasoningEffortHigh": "Cao",
      "reasoningBudget": "Ngân sách suy luận (token)",
      "reasoningBudgetDesc": "Token tối đa dành cho tư duy",
      "reasoningEffortLowDesc": "Phản hồi nhanh với suy luận tối thiểu",
      "reasoningEffortMediumDesc": "Độ sâu suy luận cân bằng",
      "reasoningEffortHighDesc": "Độ sâu suy luận tối đa cho bài toán phức tạp",
      "reasoningBudgetExtendedDesc": "Token tối đa dành cho tư duy. Được cộng thêm vào giới hạn đầu ra."
    },
    "providerParameterSupport": {
      "unknownProvider": "Nhà cung cấp không xác định: {{providerId}}",
      "reasoningNotSupportedEffort": "Không hỗ trợ - nhà cung cấp này không sử dụng mức nỗ lực",
      "reasoningNotSupportedBudgetOnly": "Không hỗ trợ - nhà cung cấp này chỉ dùng phương pháp ngân sách",
      "reasoningNotSupported": "Không hỗ trợ - nhà cung cấp này không hỗ trợ suy luận",
      "unsupportedParametersIgnored": "Các tham số không được hỗ trợ sẽ bị bỏ qua bởi {{providerName}}.",
      "reasoningEffortSupported": "Nỗ lực suy luận được hỗ trợ cho mô hình tư duy (o1, DeepSeek-R1, v.v.)",
      "reasoningBudgetSupported": "Nhà cung cấp này dùng tư duy theo ngân sách (không có mức nỗ lực). Đặt token ngân sách suy luận thay thế.",
      "reasoningNotSupportedProvider": "Nhà cung cấp này không hỗ trợ tham số suy luận.",
      "matrixTitle": "Ma trận hỗ trợ tham số nhà cung cấp",
      "providerColumn": "Nhà cung cấp",
      "supportedIndicator": "Được hỗ trợ bởi API nhà cung cấp",
      "notSupportedIndicator": "Không hỗ trợ (tham số sẽ bị bỏ qua)"
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
    "characterNotFound": "Không tìm thấy nhân vật",
    "chatHistory": "Lịch sử trò chuyện",
    "previousConversationsWithCharacter": "Cuộc trò chuyện trước với {{name}}",
    "previousConversations": "Cuộc trò chuyện trước",
    "searchChats": "Tìm kiếm trò chuyện...",
    "searchResults": "{{count}} kết quả",
    "noConversationsYet": "Chưa có cuộc trò chuyện nào",
    "startChattingPrompt": "Bắt đầu trò chuyện để xem lịch sử ở đây",
    "noMatchingChats": "Không có trò chuyện phù hợp",
    "tryDifferentSearchTerm": "Thử từ khóa khác",
    "untitledChat": "Trò chuyện chưa đặt tên",
    "chatTitlePlaceholder": "Tiêu đề trò chuyện...",
    "exportChatPackage": "Xuất gói trò chuyện",
    "characterSpecificExport": "Xuất theo nhân vật",
    "characterSpecificExportDesc": "Liên kết gói này với nhân vật trò chuyện theo mặc định.",
    "nonCharacterSpecificExport": "Xuất chung",
    "nonCharacterSpecificExportDesc": "Yêu cầu chọn nhân vật khi nhập.",
    "deleteChat": "Xóa trò chuyện?",
    "deleteConfirmDesc": "Xóa vĩnh viễn khỏi lịch sử",
    "keepThisChat": "Giữ trò chuyện này",
    "editCharacter": "Sửa nhân vật",
    "exportCharacter": "Xuất nhân vật",
    "chatAppearance": "Giao diện trò chuyện",
    "importChatPackage": "Nhập gói trò chuyện",
    "hideThisCharacter": "Ẩn nhân vật này",
    "deleteCharacter": "Xóa nhân vật",
    "deleteCharacterTitle": "Xóa nhân vật?",
    "deleteCharacterConfirmation": "Bạn có chắc muốn xóa \"{{name}}\"? Điều này cũng sẽ xóa tất cả phiên trò chuyện với nhân vật này.",
    "characterSpecificMatches": "Gói này dành riêng cho nhân vật và khớp với nhân vật đã chọn.",
    "characterSpecificMismatch": "Gói này dành riêng cho nhân vật khác. Nó sẽ được nhập vào {{name}}.",
    "nonCharacterSpecificImport": "Gói này không dành riêng cho nhân vật. Nó sẽ được nhập vào {{name}}.",
    "noCharactersYet": "Chưa có nhân vật nào",
    "createFirstCharacter": "Tạo nhân vật đầu tiên từ nút + bên dưới để bắt đầu trò chuyện",
    "scrollToBottom": "Cuộn xuống dưới",
    "selectCharacter": "Chọn nhân vật",
    "branchToGroupChat": "Phân nhánh sang nhóm trò chuyện",
    "addContent": "Thêm nội dung",
    "swapPlaces": "Đổi vị trí",
    "swapPlacesOn": "Đổi vị trí (Bật)",
    "uploadImage": "Tải hình ảnh lên",
    "helpMeReply": "Giúp tôi trả lời",
    "sceneImage": {
      "modeTitle": "Hình ảnh cảnh",
      "modeDescription": "Chọn tự viết cảnh nhắc hay để AI phác thảo cảnh đó trước.",
      "writePrompt": "Viết lời nhắc",
      "writePromptDesc": "Nhập lời nhắc hình ảnh cảnh chính xác mà bạn muốn sử dụng.",
      "askAi": "Hỏi AI",
      "askAiDesc": "Để mô hình trò chuyện hiện tại soạn thảo lời nhắc cảnh từ thời điểm đã chọn.",
      "generateTitle": "Tạo hình ảnh cảnh",
      "regenerateTitle": "Tái tạo hình ảnh cảnh",
      "aiTitle": "Lời nhắc cảnh AI",
      "promptLabel": "NHẮC CẢNH CẢNH",
      "promptPlaceholder": "Miêu tả khung cảnh, nhân vật, tâm trạng, ánh sáng, khung hình và các chi tiết quan trọng...",
      "suggestedPrompt": "Lời nhắc được đề xuất",
      "regeneratePrompt": "tái sinh",
      "editPrompt": "Chỉnh sửa lời nhắc",
      "reviewTitle": "Xem lại prompt cảnh",
      "denyPrompt": "Từ chối",
      "acceptPrompt": "Chấp nhận",
      "generateImage": "Tạo hình ảnh",
      "updateImage": "Cập nhật hình ảnh"
    },
    "useMyTextAsBase": "Dùng văn bản của tôi làm cơ sở",
    "writeNewReply": "Viết nội dung mới",
    "suggestedReply": "Gợi ý trả lời",
    "selectTwoCharactersMinimum": "Chọn ít nhất 2 nhân vật cho nhóm trò chuyện.",
    "groupBranchCreated": "Đã tạo nhánh nhóm! Đang chuyển hướng...",
    "memories": "Ký ức",
    "tools": "Công cụ",
    "pinned": "Đã ghim",
    "searchMemories": "Tìm ký ức...",
    "addMemory": "Thêm ký ức",
    "memoryActions": "Hành động ký ức",
    "pinnedMessages": "Tin nhắn đã ghim",
    "pinnedMessagesDesc": "Luôn được bao gồm trong ngữ cảnh",
    "contextSummary": "Tóm tắt ngữ cảnh",
    "contextSummaryPlaceholder": "Tóm tắt ngắn để giữ ngữ cảnh nhất quán giữa các tin nhắn...",
    "memoryContentPlaceholder": "Cần ghi nhớ điều gì?",
    "editMemoryPlaceholder": "Nhập nội dung ký ức...",
    "togglePin": {
      "pin": "Ghim",
      "unpin": "Bỏ ghim"
    },
    "toggleMemoryState": {
      "setHot": "Đặt nóng",
      "setCold": "Đặt lạnh"
    },
    "selectModelForRetry": "Chọn mô hình để thử lại",
    "memoryCategories": {
      "characterTrait": "đặc điểm nhân vật",
      "relationship": "mối quan hệ",
      "plotEvent": "sự kiện cốt truyện",
      "worldDetail": "chi tiết thế giới",
      "preference": "sở thích",
      "other": "khác"
    },
    "settings": {
      "memorySection": "Bộ nhớ",
      "memorySectionDesc": "Tóm tắt, nhãn, lịch sử gọi công cụ",
      "quickSettings": "Cài đặt nhanh",
      "quickSettingsDesc": "Điều chỉnh thường dùng nhất",
      "persona": "Persona",
      "model": "Mô hình",
      "fallbackModel": "Mô hình dự phòng",
      "voice": "Giọng nói",
      "voiceDesc": "Đọc văn bản thành giọng",
      "advanced": "Nâng cao",
      "advancedDesc": "Ghi đè tham số mô hình cho phiên này",
      "session": "Phiên",
      "sessionDesc": "Bắt đầu trò chuyện mới và duyệt lịch sử",
      "newChat": "Trò chuyện mới",
      "newChatDesc": "Bắt đầu cuộc trò chuyện mới",
      "chatHistoryDesc": "Xem các phiên trước",
      "importChatPackageDesc": "Nhập .chatpkg vào nhân vật này",
      "selectModel": "Chọn mô hình",
      "selectFallbackModel": "Chọn mô hình dự phòng",
      "personaActions": "Hành động Persona",
      "sessionAdvancedSettings": "Cài đặt nâng cao phiên",
      "parameterSupport": "Hỗ trợ tham số",
      "backToChat": "Quay lại cuộc trò chuyện",
      "chatSettingsTitle": "Cài đặt trò chuyện",
      "chatSettingsSubtitle": "Quản lý tùy chọn cuộc trò chuyện",
      "modelDefaults": "Mặc định của mô hình",
      "appDefaults": "Mặc định của ứng dụng",
      "openChatSessionFirst": "Hãy mở một phiên trò chuyện trước",
      "sessionRequired": "Cần có phiên",
      "noPersona": "Không có persona",
      "customPersona": "Persona tùy chỉnh",
      "noModelAvailable": "Không có mô hình khả dụng",
      "fallbackNone": "Không",
      "unknownModel": "Mô hình không xác định",
      "authorNote": "Ghi chú của tác giả",
      "identityProfileAuthored": "Hồ sơ danh tính đã được tạo",
      "addIdentityProfile": "Thêm hồ sơ danh tính của bạn đồng hành",
      "soulLabel": "Linh hồn",
      "sessionTitle": "Phiên: {{title}}",
      "sessionUntitled": "Untitled",
      "messageCount": "{{count}} messages",
      "usingCharacterDefault": "Using character default",
      "sessionOverrideActive": "Session override active",
      "autoplayVoice": "Autoplay voice",
      "useCharacterDefault": "Use character default"
    },
    "search": {
      "placeholder": "Tìm trong cuộc trò chuyện...",
      "noMessagesFound": "Không tìm thấy tin nhắn",
      "you": "Bạn",
      "character": "Nhân vật",
      "failed": "Tìm kiếm tin nhắn thất bại"
    },
    "templates": {
      "startWithTemplate": "Bắt đầu với mẫu?",
      "noTemplate": "Không có mẫu",
      "startWithSceneOnly": "Bắt đầu chỉ với bối cảnh",
      "you": "Bạn",
      "bot": "Bot",
      "messageCount": "{{count}} tin nhắn"
    },
    "header": {
      "back": "Quay lại",
      "openSettings": "Mở cài đặt trò chuyện",
      "manageMemories": "Quản lý ký ức",
      "searchMessages": "Tìm tin nhắn",
      "manageLorebooks": "Quản lý sách lore",
      "conversationSettings": "Cài đặt cuộc trò chuyện"
    },
    "footer": {
      "sendMessagePlaceholder": "Gửi tin nhắn...",
      "stopGeneration": "Dừng tạo",
      "sendMessage": "Gửi tin nhắn",
      "continueConversation": "Tiếp tục cuộc trò chuyện",
      "moreOptions": "Thêm tùy chọn",
      "addImage": "Thêm hình ảnh",
      "addImageAttachment": "Thêm đính kèm hình ảnh",
      "removeAttachment": "Xóa đính kèm",
      "recordVoice": "Ghi âm giọng nói"
    },
    "message": {
      "thinkingMessages": {
        "thinkingReallyHard": "Đang suy nghĩ rất chăm…",
        "lettuceCouncil": "Đang hỏi ý kiến hội đồng rau diếp…",
        "stealingThoughts": "Đang đánh cắp suy nghĩ từ hư không…",
        "warmingBrainCells": "Đang khởi động tế bào não…",
        "forbiddenKnowledge": "Đang tải kiến thức cấm…",
        "overthinking": "Đang nghĩ quá nhiều (như thường lệ)…",
        "pretendingToBeSmart": "Đang giả vờ thông minh…",
        "crunchingNumbers": "Đang xử lý số tưởng tượng…",
        "arguingWithMyself": "Đang tranh luận với chính mình…",
        "askingUniverse": "Đang lịch sự hỏi vũ trụ…"
      },
      "thoughtProcess": "Quá trình tư duy",
      "regenerateResponse": "Tạo lại phản hồi",
      "cancelAudioGeneration": "Hủy tạo âm thanh",
      "stopAudio": "Dừng âm thanh",
      "playMessageAudio": "Phát âm thanh tin nhắn",
      "playAudio": "Phát",
      "sceneLabel": "Bối cảnh",
      "variantLabel": "Biến thể",
      "regenerating": "Đang tạo lại",
      "assistantIsTyping": "Trợ lý đang gõ",
      "attachedImage": "Hình ảnh đính kèm",
      "guidedRegenerationTitle": "Guide regeneration",
      "guidedRegenerationLabel": "How should it change?",
      "guidedRegenerationDescription": "Describe the tone, length, details to keep or remove, and anything the next reply should do differently.",
      "guidedRegenerationPlaceholder": "Make it shorter, warmer, more direct...",
      "guidedRegenerationSubmit": "Regenerate"
    },
    "actions": {
      "assistantMessage": "Tin nhắn trợ lý",
      "userMessage": "Tin nhắn người dùng",
      "promptTokens": "Token lời nhắc",
      "completionTokens": "Token phản hồi",
      "fallbackModelUsed": "Đã dùng mô hình dự phòng",
      "total": "tổng",
      "timeToFirstToken": "Thời gian đến token đầu tiên",
      "completionTokenSpeed": "Tốc độ token phản hồi",
      "edit": "Sửa",
      "copy": "Sao chép",
      "pin": "Ghim",
      "unpin": "Bỏ ghim",
      "rewindToHere": "Tua lại đến đây",
      "branchFromHere": "Phân nhánh từ đây",
      "branchToGroupChat": "Phân nhánh sang nhóm trò chuyện",
      "branchToCharacter": "Phân nhánh sang nhân vật",
      "generateSceneImage": "Tạo hình ảnh cảnh",
      "regenerateSceneImage": "Tái tạo hình ảnh cảnh",
      "chatAppearance": "Giao diện trò chuyện",
      "delete": "Xóa",
      "debug": "Gỡ lỗi",
      "unpinToDelete": "Bỏ ghim để xóa",
      "editPlaceholder": "Sửa tin nhắn của bạn...",
      "memoriesUsed": "{{count}} ký ức đã dùng",
      "lorebookUsage": "Sử dụng sách lore",
      "lorebookUsageDesc": "Phản hồi này đã sử dụng các mục sách lore sau.",
      "matchScore": "Khớp: {{score}}%",
      "unknownModel": "Mô hình không xác định",
      "loadingModel": "Đang tải mô hình..."
    },
    "emptyState": {
      "goBack": "Quay lại"
    },
    "settingsDrawer": {
      "title": "Cài đặt trò chuyện",
      "subtitle": "Quản lý tùy chọn cuộc trò chuyện"
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
      "sillyTavernExportedTo": "SillyTavern chat exported to:\n{{path}}",
      "failedExportChatPackage": "Failed to export chat package",
      "failedExportSillyTavern": "Failed to export SillyTavern chat"
    },
    "authorNote": {
      "title": "Ghi chú của tác giả",
      "inlineEditor": "Inline editor",
      "inlineEditorDesc": "Show the author note above the chat input for quick edits.",
      "toggleInlineEditor": "Toggle inline author note editor",
      "placeholder": "Private direction for this chat",
      "willBeRemoved": "Author note will be removed on save",
      "noNoteSaved": "No author note saved",
      "charactersCount": "{{count}} characters",
      "clear": "Clear",
      "save": "Save",
      "saving": "Saving...",
      "failedSave": "Failed to save author note",
      "addPlaceholder": "Add an author note...",
      "savingShort": "Saving..."
    },
    "drawer": {
      "chatSettingsTitle": "Cài đặt trò chuyện",
      "chatSettingsSubtitle": "Quản lý tùy chọn cuộc trò chuyện"
    },
    "companionSoul": {
      "loading": "Loading Companion Soul...",
      "unavailable": "Companion Soul is unavailable",
      "unavailableDesc": "Soul editing is only available for companion-mode characters.",
      "pageTitle": "Companion Soul",
      "back": "Back",
      "save": "Save"
    },
    "companionRelationship": {
      "back": "Back",
      "loading": "Loading relationship state...",
      "unavailableTitle": "Relationship state is unavailable",
      "sessionLoadFailed": "The chat session could not be loaded.",
      "backToChat": "Quay lại cuộc trò chuyện",
      "notCompanionTitle": "This chat is not in companion mode",
      "notCompanionDesc": "Companion relationship pages only render for chats whose character mode is companion.",
      "openRegularMemories": "Open regular memories",
      "pageTitle": "Relationship state",
      "memoryButton": "Memory",
      "lastInteraction": "Last interaction {{time}}",
      "bond": "Bond",
      "closeness": "Closeness",
      "trust": "Trust",
      "affection": "Affection",
      "tension": "Tension",
      "stability": "Stability",
      "interactions": "Interactions",
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
      "editGroup": "Sửa nhóm",
      "deleteGroup": "Xóa nhóm",
      "deleteConfirmTitle": "Xóa nhóm trò chuyện?",
      "deleteConfirmMessage": "Bạn có chắc muốn xóa \"{{name}}\"? Điều này cũng sẽ xóa tất cả tin nhắn trong nhóm trò chuyện này.",
      "noGroupChatsYet": "Chưa có nhóm trò chuyện nào",
      "noGroupChatsDesc": "Tạo nhóm trò chuyện đầu tiên từ nút + bên dưới để trò chuyện với nhiều nhân vật cùng lúc",
      "newChat": "Cuộc trò chuyện mới",
      "openChat": "Mở cuộc trò chuyện",
      "chatSettings": "Cài đặt cuộc trò chuyện",
      "sessionCount": "{{count}} cuộc trò chuyện"
    },
    "create": {
      "invalidPackage": "Gói này không phải là gói nhóm trò chuyện.",
      "inspectPackageError": "Không thể kiểm tra gói nhóm trò chuyện",
      "importPackageError": "Không thể nhập gói nhóm trò chuyện",
      "importChatpkg": "Nhập `.chatpkg`",
      "mapParticipantsTitle": "Ánh xạ người tham gia",
      "selectLocalCharacter": "Chọn nhân vật cục bộ cho người tham gia này.",
      "selectCharacterPlaceholder": "Chọn nhân vật...",
      "importChatPackageTitle": "Nhập gói trò chuyện",
      "importChatPackageDesc": "Thao tác này sẽ nhập `.chatpkg` đã chọn như một phiên nhóm mới.",
      "characterSelect": {
        "title": "Tạo nhóm trò chuyện",
        "subtitle": "Chọn nhân vật cho cuộc trò chuyện nhóm",
        "selected": "đã chọn",
        "ready": "Sẵn sàng",
        "minRequired": "Tối thiểu 2 bắt buộc",
        "noCharactersYet": "Chưa có nhân vật nào",
        "noCharactersDesc": "Tạo nhân vật trước để bắt đầu nhóm trò chuyện",
        "continueToSetup": "Tiếp tục thiết lập nhóm"
      },
      "groupSetup": {
        "title": "Thiết lập nhóm",
        "subtitle": "Cấu hình cài đặt nhóm trò chuyện",
        "chatType": "Loại trò chuyện",
        "conversation": "Trò chuyện",
        "casualChat": "Trò chuyện thường",
        "roleplay": "Nhập vai",
        "withScenes": "Có bối cảnh",
        "conversationDesc": "Trò chuyện nhóm thường không có bối cảnh mở đầu",
        "roleplayDesc": "Kịch bản nhập vai với bối cảnh mở đầu và lời nhắc nhập vai",
        "speakerSelection": "Chọn người nói",
        "llm": "LLM",
        "aiPicks": "AI chọn",
        "heuristic": "Heuristic",
        "scoreBased": "Dựa trên điểm",
        "roundRobin": "Luân phiên",
        "takeTurns": "Thay phiên",
        "llmDesc": "Dùng mô hình mặc định để chọn người nói (tốn token)",
        "heuristicDesc": "Dùng cân bằng tham gia và gợi ý ngữ cảnh (miễn phí)",
        "roundRobinDesc": "Nhân vật nói theo thứ tự (miễn phí)",
        "chatBackground": "Nền trò chuyện",
        "optional": "(Tùy chọn)",
        "uploadBackground": "Tải ảnh nền lên",
        "backgroundDesc": "Đặt ảnh nền cho nhóm trò chuyện này",
        "groupName": "Tên nhóm",
        "removeBackground": "Xóa ảnh nền",
        "groupNameAutoGenerate": "Để trống để tự động tạo từ tên nhân vật",
        "continueToScene": "Tiếp tục đến bối cảnh mở đầu",
        "createGroupChat": "Tạo nhóm trò chuyện"
      },
      "startingScene": {
        "title": "Bối cảnh mở đầu",
        "subtitle": "Đặt kịch bản mở đầu cho nhập vai",
        "sceneSource": "Nguồn bối cảnh",
        "none": "Không",
        "custom": "Tùy chỉnh",
        "fromCharacter": "Từ nhân vật",
        "noneDesc": "Bắt đầu không có bối cảnh định sẵn",
        "customDesc": "Viết kịch bản mở đầu của riêng bạn",
        "fromCharacterDesc": "Dùng bối cảnh từ một trong các nhân vật",
        "sceneContent": "Nội dung bối cảnh",
        "sceneContentPlaceholder": "Mô tả bối cảnh mở đầu cho nhập vai...",
        "sceneReferenceTip": "Mẹo: Gõ {{@\" để tham chiếu nhân vật",
        "selectScene": "Chọn bối cảnh",
        "sceneLabel": " — Bối cảnh",
        "copyToCustom": "Sao chép sang tùy chỉnh & Sửa"
      }
    },
    "history": {
      "title": "Lịch sử nhóm trò chuyện",
      "subtitle": "Tất cả cuộc trò chuyện nhóm",
      "searchPlaceholder": "Tìm nhóm trò chuyện...",
      "active": "Hoạt động ({{count}})",
      "archived": "Lưu trữ ({{count}})",
      "noChatsYet": "Chưa có nhóm trò chuyện nào",
      "noChatsDesc": "Tạo nhóm trò chuyện để xem lịch sử ở đây",
      "noMatchingChats": "Không có trò chuyện phù hợp",
      "noMatchingDesc": "Thử từ khóa khác",
      "deleteSessionTitle": "Xóa nhóm trò chuyện?",
      "deleteSessionDesc": "Xóa vĩnh viễn khỏi lịch sử",
      "deleteSessionButton": "Xóa trò chuyện",
      "keepChat": "Giữ trò chuyện này"
    },
    "session": {
      "chatTitlePlaceholder": "Tiêu đề trò chuyện...",
      "newChat": "Trò chuyện mới",
      "rename": "Đổi tên",
      "unarchive": "Bỏ lưu trữ",
      "archive": "Lưu trữ",
      "messageCount": "{{count}} tin nhắn"
    },
    "memories": {
      "tabMemories": "Ký ức",
      "tabPinned": "Đã ghim",
      "tabActivity": "Hoạt động",
      "processing": "Đang xử lý",
      "contextSummaryTitle": "Tóm tắt ngữ cảnh",
      "addContextSummaryPrompt": "Nhấn để thêm tóm tắt ngữ cảnh...",
      "savedMemories": "Ký ức đã lưu",
      "resultsCount": "Kết quả ({{count}})",
      "searchPlaceholder": "Tìm ký ức...",
      "addMemory": "Thêm ký ức",
      "noMemoriesYet": "Chưa có ký ức nào",
      "noMemoriesDesc": "Nhấn nút Thêm ở trên để tạo",
      "noMatchingMemories": "Không có ký ức phù hợp",
      "noMatchingDesc": "Thử từ khóa khác",
      "sessionNotFound": "Không tìm thấy phiên",
      "memoryActions": "Hành động ký ức",
      "tokens": "token",
      "cycle": "Chu kỳ",
      "accessed": "Đã truy cập",
      "cold": "Lạnh",
      "hot": "Nóng",
      "activityLog": "Nhật ký hoạt động",
      "events": "sự kiện",
      "run": "Chạy",
      "processingMemories": "AI đang sắp xếp ký ức nhóm...",
      "memoryCycleSuccess": "Chu kỳ bộ nhớ xử lý thành công!",
      "memoryActionFailed": "Hành động ký ức thất bại",
      "newMemoryUpdates": "Có cập nhật ký ức mới",
      "noActivityYet": "Chưa có hoạt động nào",
      "noActivityDesc": "Lệnh gọi công cụ xuất hiện khi AI quản lý ký ức ở chế độ động",
      "contextSummaryPlaceholder": "Tóm tắt ngắn để giữ ngữ cảnh nhất quán giữa các tin nhắn...",
      "addMemoryTitle": "Thêm ký ức",
      "memoryPlaceholder": "Cần ghi nhớ điều gì?",
      "saveMemory": "Lưu ký ức",
      "editMemoryTitle": "Sửa ký ức",
      "editMemoryPlaceholder": "Nhập nội dung ký ức...",
      "edit": "Sửa",
      "pin": "Ghim",
      "unpin": "Bỏ ghim",
      "setHot": "Đặt nóng",
      "setCold": "Đặt lạnh"
    },
    "toolLog": {
      "created": "Đã tạo",
      "deleted": "Đã xóa",
      "pinned": "Đã ghim",
      "unpinned": "Đã bỏ ghim",
      "done": "Xong",
      "pinnedBadge": "đã ghim",
      "softDelete": "xóa mềm",
      "memoryCycle": "Chu kỳ bộ nhớ",
      "failedAt": "Thất bại tại:",
      "window": "Cửa sổ",
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
      "thinkingHard": "Đang suy nghĩ rất chăm…",
      "thinkingLettuce": "Đang hỏi ý kiến hội đồng rau diếp…",
      "thinkingVoid": "Đang đánh cắp suy nghĩ từ hư không…",
      "thinkingBrainCells": "Đang khởi động tế bào não…",
      "thinkingForbidden": "Đang tải kiến thức cấm…",
      "thinkingOverthinking": "Đang nghĩ quá nhiều (như thường lệ)…",
      "thinkingPretending": "Đang giả vờ thông minh…",
      "thinkingCrunching": "Đang xử lý số tưởng tượng…",
      "thinkingArguing": "Đang tranh luận với chính mình…",
      "thinkingUniverse": "Đang lịch sự hỏi vũ trụ…",
      "thoughtProcess": "Quá trình tư duy",
      "userAlt": "Người dùng",
      "assistantAlt": "Trợ lý",
      "regenerateResponse": "Tạo lại phản hồi",
      "variantLabel": "Biến thể",
      "regenerating": "Đang tạo lại",
      "cancelAudioGeneration": "Hủy tạo âm thanh",
      "stopAudio": "Dừng âm thanh",
      "playMessageAudio": "Phát âm thanh tin nhắn",
      "playAudio": "Phát",
      "attachedImage": "Hình ảnh đính kèm",
      "assistantIsTyping": "Trợ lý đang gõ",
      "assistantTyping": "Assistant is typing"
    },
    "header": {
      "back": "Quay lại",
      "memories": "Ký ức",
      "settings": "Cài đặt",
      "characters": "nhân vật"
    },
    "footer": {
      "mentionCharacter": "Đề cập nhân vật",
      "noCharactersFound": "Không tìm thấy nhân vật",
      "moreOptions": "Thêm tùy chọn",
      "addImage": "Thêm hình ảnh",
      "messagePlaceholder": "Tin nhắn... (@ để đề cập)",
      "stopGeneration": "Dừng tạo",
      "sendMessage": "Gửi tin nhắn",
      "continueConversation": "Tiếp tục cuộc trò chuyện",
      "dismissError": "Bỏ qua lỗi",
      "removeAttachment": "Xóa đính kèm",
      "tabToSelect": "Tab để chọn"
    },
    "messageActions": {
      "characterMessage": "Tin nhắn nhân vật",
      "yourMessage": "Tin nhắn của bạn",
      "whyCharacterResponded": "Tại sao nhân vật này trả lời",
      "total": "tổng",
      "edit": "Sửa",
      "copy": "Sao chép",
      "regenerateWithDifferent": "Tạo lại với nhân vật khác",
      "rewindToHere": "Tua lại đến đây",
      "unpinToDelete": "Bỏ ghim để xóa",
      "delete": "Xóa",
      "editPlaceholder": "Sửa tin nhắn của bạn...",
      "chooseCharacterTitle": "Chọn nhân vật",
      "selectCharacterForRegeneration": "Chọn nhân vật nào nên trả lời thay thế:"
    },
    "settings": {
      "appDefault": "Mặc định ứng dụng",
      "selectPersona": "Chọn Persona",
      "noPersonas": "Không có persona",
      "noPersonasDesc": "Tạo persona trong cài đặt để cá nhân hóa cuộc trò chuyện.",
      "searchPersonas": "Tìm persona...",
      "noPersona": "Không có Persona",
      "noPersonaDesc": "Tiếp tục không có persona",
      "noPersonasFound": "Không tìm thấy persona",
      "noPersonasFoundDesc": "Thử từ khóa khác"
    },
    "groupSettings": {
      "title": "Chỉnh sửa nhóm",
      "subtitle": "Cập nhật thiết lập mặc định cho các phiên tương lai",
      "enterGroupName": "Nhập tên nhóm",
      "participant": "người tham gia",
      "participants": "người tham gia",
      "uploading": "Đang tải lên...",
      "changeBackground": "Đổi nền",
      "addBackgroundImage": "Thêm ảnh nền",
      "removeBackground": "Xóa nền",
      "persona": "Nhân cách",
      "personaSubtitle": "Nhân cách mặc định cho phiên mới",
      "personaLabel": "Nhân cách",
      "speakerSelection": "Chọn người nói",
      "speakerSubtitle": "Phương thức mặc định cho phiên mới",
      "llm": "LLM",
      "aiPicks": "AI chọn",
      "heuristic": "Heuristic",
      "scoreBased": "Dựa trên điểm",
      "roundRobin": "Luân phiên",
      "takeTurns": "Lần lượt",
      "llmDesc": "Sử dụng mô hình mặc định để chọn ai nói (tốn token)",
      "heuristicDesc": "Sử dụng cân bằng tham gia và gợi ý ngữ cảnh (miễn phí)",
      "roundRobinDesc": "Nhân vật nói theo lượt (miễn phí)",
      "memoryMode": "Chế độ bộ nhớ",
      "memorySubtitle": "Chế độ bộ nhớ mặc định cho phiên mới",
      "manual": "Thủ công",
      "manualDesc": "Tự quản lý ghi chú",
      "dynamic": "Động",
      "dynamicDesc": "Gọi lại tự động",
      "memoryDynamicInfo": "AI tự động tạo và truy xuất ký ức từ các cuộc trò chuyện",
      "memoryManualInfo": "Bạn tự thêm và quản lý ghi chú bộ nhớ",
      "characters": "Nhân vật",
      "participantsActive": "{{total}} người tham gia · {{active}} đang hoạt động",
      "add": "Thêm",
      "muted": "(đã tắt tiếng)",
      "mutedByDefault": "Tắt tiếng theo mặc định",
      "activeByDefault": "Hoạt động theo mặc định",
      "unmuteCharacter": "Bật tiếng nhân vật",
      "muteCharacter": "Tắt tiếng nhân vật",
      "minTwoRequired": "Cần tối thiểu 2 nhân vật",
      "removeCharacter": "Xóa nhân vật",
      "groupMinCharacters": "Một nhóm cần ít nhất 2 nhân vật",
      "mutedCharactersNote": "Nhân vật bị tắt tiếng sẽ bị bỏ qua bởi lựa chọn người nói tự động, nhưng vẫn có thể phản hồi qua `@đề cập` rõ ràng.",
      "addCharacterTitle": "Thêm nhân vật",
      "allCharactersInGroup": "Tất cả nhân vật đã ở trong nhóm này.",
      "removeCharacterTitle": "Xóa nhân vật?",
      "removeCharacterConfirm": "Bạn có chắc muốn xóa",
      "removeCharacterFrom": "khỏi mặc định nhóm?",
      "removing": "Đang xóa...",
      "remove": "Xóa"
    },
    "sessionSettings": {
      "subtitle": "Quản lý tùy chọn trò chuyện nhóm",
      "enterGroupName": "Nhập tên nhóm",
      "participant": "người tham gia",
      "participants": "người tham gia",
      "message": "tin nhắn",
      "messages": "tin nhắn",
      "uploading": "Đang tải lên...",
      "changeBackground": "Đổi nền",
      "addBackgroundImage": "Thêm ảnh nền",
      "removeBackground": "Xóa nền",
      "persona": "Nhân cách",
      "personaSubtitle": "Danh tính của bạn trong cuộc trò chuyện này",
      "personaLabel": "Nhân cách",
      "speakerSelection": "Chọn người nói",
      "speakerSubtitle": "Cách chọn người nói tiếp theo",
      "llm": "LLM",
      "aiPicks": "AI chọn",
      "heuristic": "Heuristic",
      "scoreBased": "Dựa trên điểm",
      "roundRobin": "Luân phiên",
      "takeTurns": "Lần lượt",
      "llmDesc": "Sử dụng mô hình mặc định để chọn ai nói (tốn token)",
      "heuristicDesc": "Sử dụng cân bằng tham gia và gợi ý ngữ cảnh (miễn phí)",
      "roundRobinDesc": "Nhân vật nói theo lượt (miễn phí)",
      "characters": "Nhân vật",
      "participantsActive": "{{total}} người tham gia · {{active}} đang hoạt động",
      "add": "Thêm",
      "muted": "(đã tắt tiếng)",
      "unmuteCharacter": "Bật tiếng nhân vật",
      "muteCharacter": "Tắt tiếng nhân vật",
      "minTwoRequired": "Cần tối thiểu 2 nhân vật",
      "removeCharacter": "Xóa nhân vật",
      "groupMinCharacters": "Trò chuyện nhóm cần ít nhất 2 nhân vật",
      "mutedCharactersNote": "Nhân vật bị tắt tiếng sẽ bị bỏ qua bởi lựa chọn người nói tự động, nhưng vẫn có thể phản hồi qua `@đề cập` rõ ràng.",
      "data": "Dữ liệu",
      "dataSubtitle": "Xuất hoặc nhập cuộc trò chuyện",
      "export": "Xuất",
      "exportDesc": "Lưu dưới dạng tệp chia sẻ",
      "import": "Nhập",
      "importDesc": "Tải cuộc trò chuyện từ tệp",
      "conversation": "Cuộc trò chuyện",
      "conversationSubtitle": "Nhân bản hoặc tiếp tục trong cuộc trò chuyện mới",
      "duplicate": "Nhân bản",
      "duplicateDesc": "Sao chép cuộc trò chuyện này có hoặc không có tin nhắn",
      "branchTo1on1": "Phân nhánh sang 1-1",
      "branchTo1on1Desc": "Tiếp tục riêng tư với một nhân vật",
      "participation": "Sự tham gia",
      "participationSubtitle": "Phân bố phát biểu giữa các nhân vật",
      "addCharacterTitle": "Thêm nhân vật",
      "allCharactersInGroup": "Tất cả nhân vật đã ở trong nhóm này.",
      "removeCharacterTitle": "Xóa nhân vật?",
      "removeCharacterConfirm": "Bạn có chắc muốn xóa",
      "removeCharacterFrom": "khỏi cuộc trò chuyện nhóm này?",
      "removing": "Đang xóa...",
      "remove": "Xóa",
      "cloneGroupTitle": "Nhân bản nhóm",
      "withMessages": "Có tin nhắn",
      "withMessagesDesc": "Nhân bản tất cả bao gồm lịch sử trò chuyện",
      "withoutMessages": "Không tin nhắn",
      "withoutMessagesDesc": "Chỉ nhân bản thiết lập (nhân vật, cảnh bắt đầu)",
      "branchWithCharacterTitle": "Phân nhánh với nhân vật",
      "branchWithCharacterDesc": "Chọn nhân vật để tiếp tục dưới dạng cuộc trò chuyện 1-1. Tất cả tin nhắn từ nhóm này sẽ được chuyển đổi.",
      "continueWith": "Tiếp tục trò chuyện với {{name}}",
      "exportChatPackageTitle": "Xuất gói trò chuyện",
      "includeCharacterSnapshots": "Bao gồm snapshot nhân vật",
      "includeCharacterSnapshotsDesc": "Giữ dữ liệu nhân vật trong gói",
      "sessionOnly": "Chỉ phiên",
      "sessionOnlyDesc": "Chỉ xuất tin nhắn và siêu dữ liệu",
      "mapParticipantsTitle": "Ánh xạ người tham gia",
      "selectLocalCharacter": "Chọn nhân vật cục bộ cho người tham gia này.",
      "selectCharacterPlaceholder": "Chọn nhân vật...",
      "continue": "Tiếp tục",
      "importChatPackageTitle": "Nhập gói trò chuyện",
      "importChatPackageDesc": "Điều này sẽ nhập `.chatpkg` đã chọn dưới dạng phiên nhóm mới.",
      "importing": "Đang nhập..."
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
      "cancelRecording": "Cancel recording",
      "transcribing": "Transcribing",
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
      "save": "Save",
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
      "failedToDelete": "Failed to delete: {{error}}",
      "failedToRename": "Failed to rename: {{error}}",
      "failedToArchive": "Failed to archive: {{error}}",
      "failedToUnarchive": "Failed to unarchive: {{error}}",
      "failedToDuplicate": "Failed to duplicate"
    },
    "sessionSettingsController": {
      "failedToLoad": "Không tải được cài đặt trò chuyện nhóm",
      "noPersona": "Không có persona",
      "customPersona": "Persona tùy chỉnh",
      "minOneActive": "Ít nhất một người tham gia phải luôn hoạt động"
    },
    "groupSettingsController": {
      "notFound": "Không tìm thấy nhóm",
      "failedToLoad": "Không tải được cài đặt nhóm"
    },
    "createForm": {
      "failedToLoadCharacters": "Không tải được nhân vật",
      "selectAtLeastTwo": "Vui lòng chọn ít nhất 2 nhân vật cho một cuộc trò chuyện nhóm",
      "failedToCreate": "Không tạo được cuộc trò chuyện nhóm"
    },
    "groupSetupExtra": {
      "memoryMode": "Chế độ bộ nhớ",
      "manual": "Manual",
      "manualDesc": "Tự quản lý ghi chú",
      "dynamic": "Dynamic",
      "dynamicDesc": "Tóm tắt và truy hồi tự động",
      "memoryManualInfo": "Bạn tự thêm và quản lý ghi chú bộ nhớ",
      "memoryDynamicInfo": "AI tự động tạo và truy xuất ký ức từ các cuộc trò chuyện",
      "backgroundPreviewAlt": "Xem trước nền"
    },
    "createExtra": {
      "enterGroupNamePlaceholder": "Nhập tên nhóm...",
      "unknown": "Không rõ"
    },
    "startingSceneExtra": {
      "insertAs": "Insert as {{snippet}}"
    },
    "sessionCardExtra": {
      "unknown": "Không rõ",
      "untitledChat": "Cuộc trò chuyện không có tiêu đề"
    },
    "sessionListExtra": {
      "unknown": "Không rõ"
    },
    "chatHeaderExtra": {
      "unknownError": "Lỗi không xác định"
    },
    "chatSettingsExtra": {
      "invalidPackage": "This package is not a group chat package.",
      "failedExport": "Không xuất được gói trò chuyện nhóm",
      "failedInspect": "Không kiểm tra được gói trò chuyện nhóm",
      "failedImport": "Không nhập được gói trò chuyện nhóm",
      "exportSuccess": "Gói trò chuyện nhóm đã được xuất tới:\n{{path}}",
      "backgroundAlt": "Nền",
      "removeBackgroundAria": "Xóa nền",
      "backAria": "Quay lại",
      "backToGroupChats": "Quay lại trò chuyện nhóm"
    },
    "groupSettingsExtra": {
      "backToGroups": "Quay lại nhóm",
      "backAria": "Quay lại",
      "removeBackgroundAria": "Xóa nền"
    },
    "historyPage": {
      "backAria": "Quay lại trò chuyện nhóm",
      "clearSearchAria": "Xóa tìm kiếm",
      "resultsLabel": "{{count}} result",
      "resultsLabelPlural": "{{count}} results",
      "untitledChat": "Cuộc trò chuyện không có tiêu đề",
      "noPinnedMessages": "Không có tin nhắn được ghim"
    },
    "personaSelectorExtra": {
      "insertAs": "Chèn dưới dạng",
      "appDefault": "Mặc định của ứng dụng",
      "defaultBadge": "Mặc định",
      "selectPersonaTitle": "Chọn persona",
      "noPersonaTitle": "Không có persona",
      "noPersonaDesc": "Tiếp tục mà không có persona",
      "noPersonasAvailable": "Không có persona nào khả dụng",
      "noPersonasDesc": "Tạo persona trong cài đặt để cá nhân hóa các cuộc trò chuyện của bạn.",
      "searchPersonas": "Tìm persona...",
      "noPersonasFound": "Không tìm thấy persona nào",
      "tryDifferentSearch": "Thử một từ khóa khác"
    },
    "footerExtra": {
      "cancelRecording": "Cancel recording",
      "transcribing": "Transcribing",
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
      "title": "Chưa có nhân vật nào",
      "description": "Tạo nhân vật AI tùy chỉnh với tính cách độc đáo",
      "createButton": "Tạo nhân vật"
    },
    "progress": {
      "identity": "Danh tính",
      "scenes": "Bối cảnh",
      "details": "Chi tiết"
    },
    "identity": {
      "title": "Tạo nhân vật",
      "subtitle": "Tạo danh tính cho nhân vật AI của bạn",
      "tapCameraToAdd": "Nhấn camera để thêm hoặc tạo avatar",
      "importingAvatar": "Đang nhập avatar...",
      "characterName": "Tên nhân vật *",
      "characterNamePlaceholder": "Nhập tên nhân vật...",
      "characterNameDesc": "Tên này sẽ hiển thị trong cuộc trò chuyện",
      "avatarGradient": "Gradient Avatar",
      "avatarGradientDesc": "Tạo gradient động từ màu avatar",
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
      "title": "Chi tiết nhân vật",
      "subtitle": "Định nghĩa tính cách và hành vi",
      "definition": "Định nghĩa *",
      "wordCount": "{{count}} từ",
      "definitionPlaceholder": "Mô tả tính cách, phong cách nói, lý lịch, lĩnh vực kiến thức...",
      "definitionDesc": "Hãy cụ thể về giọng điệu, đặc điểm và phong cách trò chuyện",
      "availablePlaceholders": "Biến có sẵn:"
    },
    "scenes": {
      "title": "Bối cảnh mở đầu",
      "subtitle": "Tạo kịch bản mở đầu cho cuộc trò chuyện",
      "default": "Mặc định",
      "hasSceneDirection": "Có hướng dẫn bối cảnh",
      "continueToScenes": "Tiếp tục đến bối cảnh mở đầu"
    },
    "extras": {
      "title": "Chi tiết bổ sung",
      "subtitle": "Tất cả các trường là tùy chọn",
      "nickname": "Biệt danh",
      "nicknamePlaceholder": "Người dùng nên gọi nhân vật này là gì?",
      "nicknameDesc": "Tên hiển thị thay thế trong cuộc trò chuyện",
      "creator": "Người tạo",
      "creatorPlaceholder": "Tên người tạo...",
      "tags": "Nhãn",
      "tagsPlaceholder": "fantasy, sci-fi, romance...",
      "tagsDesc": "Danh sách phân cách bằng dấu phẩy để lọc và tổ chức",
      "creatorNotes": "Ghi chú người tạo",
      "creatorNotesPlaceholder": "Mẹo sử dụng, bối cảnh lore, hoặc hướng dẫn cho người dùng khác...",
      "multilingualNotes": "Multilingual Notes",
      "multilingualNotesHint": "JSON object with language codes as keys",
      "creatingCharacter": "Creating Character..."
    },
    "preview": {
      "unnamedCharacter": "Nhân vật chưa đặt tên",
      "sceneCount": "{{count}} bối cảnh",
      "customPrompt": "Lời nhắc tùy chỉnh",
      "description": "Mô tả",
      "startingScene": "Bối cảnh mở đầu",
      "gradientEnabled": "Đã bật gradient",
      "customModel": "Mô hình tùy chỉnh",
      "avatarAlt": "Avatar nhân vật",
      "characterFallback": "Character"
    },
    "personaPreview": {
      "unnamedPersona": "Persona chưa đặt tên",
      "noDescription": "Chưa có mô tả",
      "default": "Mặc định",
      "description": "Mô tả"
    },
    "lorebookPreview": {
      "untitledLorebook": "Sách lore chưa đặt tên",
      "entryCount": "{{count}} mục",
      "entries": "Mục",
      "noEntriesYet": "Chưa có mục nào",
      "untitledEntry": "Mục chưa đặt tên",
      "noContentYet": "Chưa có nội dung",
      "alwaysActive": "Luôn hoạt động",
      "moreEntries": "+{{count}} more entries",
      "moreEntry": "+{{count}} more entry"
    },
    "creationHelper": {
      "moreOptions": "Thêm tùy chọn",
      "describePlaceholder": "Mô tả nhân vật của bạn...",
      "stopGeneration": "Dừng tạo",
      "sendMessage": "Gửi tin nhắn",
      "addToMessage": "Thêm vào tin nhắn",
      "uploadImageDesc": "Thêm avatar hoặc ảnh tham khảo",
      "referenceCharacterDesc": "Dùng nhân vật hiện có làm cảm hứng",
      "referencePersonaDesc": "Dùng persona của bạn làm ngữ cảnh",
      "retry": "Retry",
      "attachmentAlt": "Attachment",
      "removeAttachment": "Remove attachment",
      "removeReference": "Remove reference",
      "uploadImageTitle": "Upload Image",
      "referenceCharacterTitle": "Reference Character",
      "referencePersonaTitle": "Reference Persona"
    },
    "lorebook": {
      "keywords": "TỪ KHÓA",
      "caseSensitive": "Phân biệt hoa thường",
      "typeKeyword": "Nhập từ khóa...",
      "addButton": "Thêm",
      "untitledEntry": "Mục chưa đặt tên",
      "alwaysActive": "Luôn hoạt động",
      "noKeywords": "Không có từ khóa",
      "dragToReorder": "Kéo để sắp xếp lại",
      "disabled": "Đã tắt",
      "noLorebooksYet": "Chưa có sách lore nào",
      "createLorebookDesc": "Tạo sách lore để thêm kiến thức thế giới cho nhân vật này",
      "createLorebook": "Tạo sách lore",
      "searchPlaceholder": "Tìm sách lore...",
      "noMatchingLorebooks": "Không tìm thấy sách lore phù hợp",
      "activeLorebooks": "Sách lore đang hoạt động",
      "enabledForCharacter": "đã bật cho nhân vật này",
      "enabledForGroup": "đã bật cho nhóm này",
      "enabledForSession": "đã bật cho phiên này",
      "tapToViewEntries": "Nhấn để xem mục",
      "newLorebookTitle": "Sách lore mới",
      "nameLabel": "TÊN",
      "enterNamePlaceholder": "Nhập tên sách lore...",
      "lorebookExplanation": "Sách lore chứa các mục lore được chèn vào lời nhắc khi từ khóa khớp.",
      "viewEntries": "Xem mục",
      "editEntriesDesc": "Sửa các mục sách lore",
      "disableForCharacter": "Tắt cho nhân vật",
      "enableForCharacter": "Bật cho nhân vật",
      "disableForGroup": "Tắt cho nhóm",
      "enableForGroup": "Bật cho nhóm",
      "disableForSession": "Tắt cho phiên",
      "enableForSession": "Bật cho phiên",
      "removeFromActive": "Xóa khỏi sách lore đang hoạt động của nhân vật này",
      "addToActive": "Thêm vào sách lore đang hoạt động của nhân vật này",
      "removeFromActiveGroup": "Xóa khỏi sách lore đang hoạt động của nhóm này",
      "addToActiveGroup": "Thêm vào sách lore đang hoạt động của nhóm này",
      "removeFromActiveSession": "Xóa khỏi sách lore đang hoạt động của phiên này",
      "addToActiveSession": "Thêm vào sách lore đang hoạt động của phiên này",
      "deleteConfirmTitle": "Xóa sách lore?",
      "deleteConfirmMessage": "Xóa sách lore này? Tất cả mục sẽ bị mất.",
      "deleteLorebook": "Xóa sách lore",
      "noEntriesYet": "Chưa có mục nào",
      "addEntriesToInject": "Thêm mục để chèn lore vào cuộc trò chuyện",
      "createEntry": "Tạo mục",
      "searchEntries": "Tìm mục...",
      "noMatchingEntries": "Không tìm thấy mục phù hợp",
      "entryDefaultName": "Mục",
      "editEntry": "Sửa mục",
      "editEntryDesc": "Thay đổi tiêu đề, từ khóa và nội dung",
      "disableEntry": "Tắt mục",
      "enableEntry": "Bật mục",
      "entryDisabledDesc": "Mục sẽ không được chèn vào lời nhắc",
      "entryEnabledDesc": "Mục sẽ được chèn khi từ khóa khớp",
      "deleteEntry": "Xóa mục",
      "titleLabel": "TIÊU ĐỀ",
      "titlePlaceholder": "Đặt tên mục này...",
      "enabled": "Đã bật",
      "includeInPrompts": "Bao gồm trong lời nhắc",
      "alwaysOn": "Luôn bật",
      "noKeywordsNeeded": "Không cần từ khóa",
      "contentLabel": "NỘI DUNG",
      "contentPlaceholder": "Viết ngữ cảnh lore ở đây...",
      "saveEntry": "Lưu mục",
      "noCharacterId": "Chưa cung cấp ID nhân vật",
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
      "characterNotFound": "Không tìm thấy nhân vật",
      "templateCount": "{{count}} mẫu",
      "newTemplate": "Mẫu mới",
      "noTemplatesYet": "Chưa có mẫu nào",
      "explanation": "Mẫu trò chuyện cho phép bạn bắt đầu cuộc trò chuyện với tin nhắn soạn sẵn từ cả bạn và {{name}}.",
      "createTemplate": "Tạo mẫu",
      "messageCount": "{{count}} tin nhắn",
      "deleteTemplate": "Xóa mẫu",
      "contextMenuFallbackTitle": "Template",
      "importedToast": {
        "title": "Imported",
        "message": "Added \"{{name}}\"."
      },
      "importFailed": "Import failed",
      "exportFailed": "Export failed"
    },
    "templateEditor": {
      "noScene": "Không có bối cảnh",
      "untitled": "Chưa đặt tên",
      "dragMessage": "Kéo tin nhắn",
      "editMessage": "Sửa tin nhắn",
      "deleteMessage": "Xóa tin nhắn",
      "writeMessagePlaceholder": "Viết nội dung tin nhắn...",
      "characterNotFound": "Không tìm thấy nhân vật",
      "scene": "Bối cảnh",
      "noMessagesYet": "Chưa có tin nhắn nào",
      "addMessagesDesc": "Thêm tin nhắn để xây dựng mở đầu cuộc trò chuyện với {{name}}.",
      "addMessage": "Thêm tin nhắn",
      "name": "Tên",
      "nameExample": "VD: Chào hỏi thân mật",
      "startingScene": "Bối cảnh mở đầu",
      "systemPrompt": "Lời nhắc hệ thống",
      "characterDefault": "Mặc định nhân vật",
      "nextMessageAs": "Tin nhắn tiếp theo là",
      "messages": "Tin nhắn",
      "roles": "Vai trò",
      "hoverTip": "Di chuột vào tin nhắn để kéo, sửa hoặc xóa.",
      "footerTip": "Dùng thanh dưới để thêm tin nhắn mới.",
      "templateNamePlaceholder": "Tên mẫu...",
      "selectScene": "Chọn bối cảnh",
      "startWithoutScene": "Bắt đầu không có bối cảnh",
      "selectSystemPrompt": "Chọn lời nhắc hệ thống",
      "useCharacterSystemPrompt": "Dùng lời nhắc hệ thống của nhân vật"
    },
    "referenceSelector": {
      "selectCharacter": "Chọn nhân vật",
      "selectPersona": "Chọn Persona",
      "searchPlaceholder": "Tìm {{type}}...",
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
      "fallbackModelLabel": "Mô hình dự phòng (tùy chọn)",
      "selectedFallbackFallback": "Mô hình dự phòng đã chọn",
      "fallbackOff": "Tắt (không có dự phòng)",
      "fallbackHint": "Chỉ thử lại với mô hình này nếu mô hình chính thất bại",
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
      "selectFallbackModelTitle": "Chọn mô hình dự phòng",
      "searchModelsPlaceholder": "Search models...",
      "selectVoiceTitle": "Select Voice",
      "searchVoicesPlaceholder": "Search voices...",
      "myVoices": "My Voices",
      "providerVoicesLabel": "{{provider}} Voices",
      "providerFallback": "Nhà cung cấp"
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
      "save": "Save",
      "library": "Library",
      "upload": "Upload",
      "sceneBackgroundAlt": "Scene background",
      "removeBackground": "Remove background"
    },
    "companionSoul": {
      "title": "Companion Soul",
      "subtitle": "Shape who they are underneath. Generation uses the opening context you set in the previous step.",
      "retry": "Retry",
      "back": "Back",
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
      "trust": "Trust",
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
      "tension": "Tension",
      "tensionLow": "Relaxed",
      "tensionHigh": "Wound up",
      "irritation": "Irritation",
      "irritationLow": "Patient",
      "irritationHigh": "Easily set off",
      "affection": "Affection",
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
      "title": "Chưa có nhà cung cấp nào",
      "description": "Thêm và quản lý nhà cung cấp API cho mô hình AI",
      "addButton": "Thêm nhà cung cấp"
    },
    "actions": {
      "openDashboard": "Mở bảng điều khiển",
      "openDashboardDesc": "Xem nhân vật, sử dụng và cài đặt",
      "edit": "Sửa",
      "editDesc": "Thay đổi cài đặt nhà cung cấp"
    },
    "extra": {
      "apiKeyNotFound": "Không tìm thấy API key OpenRouter. Vui lòng cấu hình trong Cài đặt > Nhà cung cấp trước.",
      "audioEmpty": {
        "title": "Chưa có nhà cung cấp âm thanh",
        "description": "Thêm nhà cung cấp TTS để tạo giọng nói cho nhân vật của bạn.",
        "addButton": "Thêm nhà cung cấp"
      },
      "audioProviderLabel": {
        "elevenlabs": "ElevenLabs",
        "geminiTts": "Gemini TTS",
        "openaiTts": "OpenAI-Compatible TTS",
        "kokoro": "Kokoro (Local)"
      }
    },
    "audioEditor": {
      "titleEdit": "Sửa nhà cung cấp",
      "titleCreate": "Thêm nhà cung cấp âm thanh",
      "fields": {
        "providerType": "Loại nhà cung cấp",
        "label": "Nhãn",
        "apiKey": "API Key",
        "modelVariant": "Biến thể mô hình",
        "assetRoot": "Thư mục tài nguyên",
        "projectId": "ID dự án Google Cloud",
        "region": "Khu vực (tùy chọn)",
        "baseUrl": "URL cơ sở",
        "requestPath": "Đường dẫn yêu cầu"
      },
      "types": {
        "gemini": "Gemini TTS (Google)",
        "openai": "OpenAI-Compatible TTS",
        "kokoro": "Kokoro (Local)"
      },
      "placeholders": {
        "labelGemini": "Gemini TTS của tôi",
        "labelOpenai": "TTS tương thích của tôi",
        "labelKokoro": "Kokoro Local",
        "labelElevenlabs": "ElevenLabs của tôi",
        "apiKey": "Nhập API key của bạn",
        "assetRoot": "/đường/dẫn/tới/kokoro",
        "projectId": "id-du-an-cua-ban",
        "region": "us-central1",
        "baseUrl": "https://api.example.com"
      },
      "errors": {
        "chooseModelVariant": "Chọn một biến thể mô hình",
        "assetRootRequired": "Thư mục tài nguyên là bắt buộc",
        "saveFailed": "Lưu thất bại",
        "apiKeyRequired": "API key là bắt buộc",
        "projectIdRequired": "ID dự án là bắt buộc cho Gemini TTS",
        "baseUrlRequired": "URL cơ sở là bắt buộc cho OpenAI-compatible TTS",
        "invalidCredentials": "API key hoặc thông tin xác thực không hợp lệ",
        "verificationFailed": "Xác minh thất bại"
      },
      "loadingVariants": "Đang tải biến thể...",
      "kokoroVariantHint": "Bản dựng di động chỉ hỗ trợ int8. Cài đặt mô hình từ Kokoro Studio sau khi lưu.",
      "managed": "Được quản lý",
      "managedPath": "Được quản lý: {{path}}",
      "requestPathHint": "Dùng đường dẫn của nhà cung cấp nếu khác với mặc định OpenAI",
      "verifying": "Đang xác minh..."
    }
  },
  "models": {
    "empty": {
      "title": "Chưa có mô hình nào",
      "description": "Thêm và quản lý mô hình AI từ các nhà cung cấp khác nhau",
      "addButton": "Thêm mô hình"
    },
    "sort": {
      "alphabetical": "Theo bảng chữ cái",
      "byProvider": "Theo nhà cung cấp",
      "title": "Sắp xếp mô hình",
      "alphabeticalDescription": "Sắp xếp theo tên mô hình",
      "byProviderDescription": "Nhóm mô hình theo nhà cung cấp"
    },
    "extra": {
      "cpuFallbackSucceeded": "Mô hình này đã chuyển sang CPU trong lần chạy trước.",
      "cpuFallbackFailed": "Mô hình này đã thất bại trong lần chạy trước."
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
    "title": "Trình duyệt Mô hình",
    "searchPlaceholder": "Tìm kiếm mô hình GGUF trên HuggingFace...",
    "searching": "Đang tìm kiếm...",
    "trending": "Mô hình GGUF thịnh hành",
    "noResults": "Không tìm thấy mô hình",
    "noResultsHint": "Thử từ khóa khác hoặc duyệt các mô hình thịnh hành.",
    "likes": "lượt thích",
    "downloads": "lượt tải",
    "viewFiles": "Xem tệp",
    "files": "Tệp có sẵn",
    "noFiles": "Không tìm thấy tệp GGUF trong kho lưu trữ này.",
    "architecture": "Kiến trúc",
    "contextLength": "Độ dài ngữ cảnh",
    "parameters": "Tham số",
    "quantization": "Lượng tử hóa",
    "fileSize": "Kích thước",
    "download": "Tải xuống",
    "downloading": "Đang tải...",
    "cancelDownload": "Hủy tải xuống",
    "downloadComplete": "Tải xuống hoàn tất!",
    "downloadFailed": "Tải xuống thất bại",
    "downloadCancelled": "Đã hủy tải xuống",
    "downloadProgress": "{{downloaded}} / {{total}}",
    "progress": "Tiến trình",
    "fileOfTotal": "Tệp {{current}} / {{total}}",
    "speed": "{{speed}}/s",
    "alreadyDownloaded": "Đã tải xuống",
    "createModel": "Tạo mô hình",
    "backToSearch": "Quay lại tìm kiếm",
    "backToFiles": "Quay lại tệp",
    "sortTrending": "Thịnh hành",
    "sortDownloads": "Tải nhiều nhất",
    "sortLikes": "Thích nhiều nhất",
    "sortRecent": "Cập nhật gần đây",
    "browseOnHuggingFace": "Duyệt trên HuggingFace",
    "selectFromLibrary": "Chọn từ Thư viện",
    "libraryEmpty": "Chưa có mô hình nào được tải",
    "libraryEmptyHint": "Tải mô hình GGUF từ Trình duyệt Mô hình, hoặc nhập đường dẫn thủ công.",
    "libraryTitle": "Mô hình đã tải",
    "moveToLibrary": "Này, tôi có thể di chuyển tệp mô hình này vào thư mục mô hình GGUF nếu bạn muốn. Điều này giúp tất cả mô hình của bạn được sắp xếp ở một nơi.",
    "moveToLibraryYes": "Có, di chuyển",
    "moveToLibraryNo": "Không, để nguyên",
    "moveToLibraryMoving": "Đang di chuyển mô hình...",
    "moveToLibrarySuccess": "Di chuyển mô hình thành công!",
    "moveToLibraryFailed": "Không thể di chuyển mô hình",
    "runabilityExcellent": "Xuất sắc!",
    "runabilityGood": "Tốt",
    "runabilityMarginal": "Tạm được",
    "runabilityPoor": "Kém",
    "runabilityUnrunnable": "Không thể chạy",
    "recommendedSettings": "Cài đặt đề xuất",
    "kvCacheType": "Loại bộ nhớ đệm KV",
    "gpuFull": "Giảm tải GPU hoàn toàn",
    "gpuNearFull": "GPU gần đầy, tràn KV nhẹ",
    "gpuKvSpill": "Trọng số trên GPU, KV tràn sang RAM",
    "gpuKvHeavySpill": "Trọng số trên GPU, phần lớn KV trong RAM",
    "gpuMostLayers": "Hầu hết các lớp trên GPU",
    "gpuHalfLayers": "Một nửa số lớp trên GPU",
    "gpuFewLayers": "Ít lớp trên GPU",
    "gpuCpu": "Chỉ CPU",
    "notRecommended": "Chúng tôi không khuyến nghị chạy mô hình này trên thiết bị của bạn. Nó sẽ không chạy mượt.",
    "moreDetails": "Thêm",
    "detailedReport": "Báo cáo tài nguyên",
    "detailSystem": "Tài nguyên hệ thống",
    "detailRam": "RAM khả dụng",
    "detailVram": "VRAM khả dụng",
    "detailVramBudget": "Ngân sách VRAM (90%)",
    "detailTotalAvailable": "Tổng khả dụng",
    "detailArchitecture": "Kiến trúc mô hình",
    "detailArch": "Kiến trúc",
    "detailLayers": "Lớp",
    "detailEmbedding": "Chiều nhúng",
    "detailHeads": "Đầu chú ý",
    "detailKvHeads": "Đầu KV",
    "detailFfn": "Chiều Feed-Forward",
    "detailTrainCtx": "Ngữ cảnh huấn luyện",
    "detailConfig": "Cấu hình hiện tại",
    "detailModelSize": "Kích thước tệp mô hình",
    "detailMemory": "Phân tích bộ nhớ",
    "detailWeights": "Trọng số mô hình",
    "detailKvCache": "Bộ nhớ đệm KV",
    "detailTotalNeeded": "Tổng cần thiết",
    "detailHeadroom": "Dư địa",
    "detailGpuFit": "Giảm tải GPU",
    "detailScoreBreakdown": "Chi tiết điểm",
    "detailMemFitness": "Phù hợp bộ nhớ",
    "detailGpuAccel": "Tăng tốc GPU",
    "detailKvHeadroom": "Dư địa KV",
    "detailQuantQuality": "Chất lượng lượng tử hóa",
    "detailFinalScore": "Điểm có trọng số",
    "detailComputeBuffer": "Bộ đệm tính toán",
    "detailMemMode": "Chế độ bộ nhớ",
    "detailUnified": "Hợp nhất (RAM/VRAM chia sẻ)",
    "detailSwa": "Cửa sổ trượt",
    "detailMlaRank": "Hạng ẩn MLA",
    "detailParseStatus": "Phân tích header",
    "detailIncomplete": "Không hoàn chỉnh (siêu dữ liệu MoE quá lớn)",
    "detailEffectiveKvCtx": "Ngữ cảnh KV hiệu quả",
    "detailOffload": "Giảm tải GPU",
    "detailCtxTip": "Giảm ngữ cảnh xuống {{ctx}} token sẽ cho phép giảm tải GPU 100%.",
    "upgradeSuggestion": "{{quant}} ({{size}}) cũng vừa và đạt điểm {{score}} — chất lượng tốt hơn.",
    "layerTip": "Đề xuất: giảm tải {{layers}}/{{total}} lớp (-ngl {{layers}})",
    "detailKvDistribution": "Phân bố bộ nhớ đệm KV",
    "detailKvOnGpu": "GPU (VRAM)",
    "detailKvOnRam": "RAM hệ thống",
    "kvDistributionTip": "{{pct}}% bộ nhớ đệm KV nằm trong RAM. Xử lý prompt (prefill) sẽ chậm hơn — 100% GPU giữ cho nó tức thì.",
    "detailLayers-ngl": "Lớp cần giảm tải (-ngl)",
    "detailOptimalGpuCtx": "Ngữ cảnh GPU tối ưu",
    "detailOptimalRamCtx": "Ngữ cảnh RAM tối đa",
    "optimalGpuCtxLabel": "Tốc độ GPU đầy đủ: {{ctx}} token",
    "optimalRamCtxLabel": "RAM tối đa: {{ctx}} token",
    "optimalGpuCtxShort": "GPU: {{ctx}}",
    "optimalRamCtxShort": "Tối đa: {{ctx}}",
    "fullGpuOffloadShort": "100% GPU: {{ctx}}",
    "ctxExceedsGpu": "Ngữ cảnh vượt quá tối ưu GPU ({{ctx}}). Bộ nhớ đệm KV sẽ tràn sang RAM, giảm tốc độ.",
    "modelExceedsVram": "Mô hình vượt quá VRAM. Chạy từ RAM với giảm tải GPU một phần."
  },
  "systemPrompts": {
    "filters": {
      "all": "Tất cả",
      "system": "Hệ thống",
      "internal": "Nội bộ",
      "custom": "Tùy chỉnh"
    },
    "empty": {
      "title": "Chưa có lời nhắc tùy chỉnh nào",
      "description": "Tạo lời nhắc hệ thống tùy chỉnh để cá nhân hóa cuộc trò chuyện AI",
      "createButton": "Tạo lời nhắc"
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
      "title": "Chưa có persona nào",
      "description": "Tạo persona để xác định cách AI nên xưng hô với bạn",
      "createButton": "Tạo Persona"
    },
    "actions": {
      "editPersona": "Sửa Persona",
      "setAsDefault": "Đặt làm mặc định",
      "setAsDefaultDesc": "Sử dụng cho tất cả cuộc trò chuyện mới",
      "unsetAsDefault": "Bỏ mặc định",
      "unsetAsDefaultDesc": "Xóa trạng thái mặc định",
      "exportPersona": "Xuất Persona",
      "deletePersona": "Xóa Persona"
    },
    "edit": {
      "avatarHint": "Nhấn để thêm hoặc tạo avatar",
      "nameLabel": "TÊN PERSONA",
      "namePlaceholder": "VD: Chuyên nghiệp, Nhà văn sáng tạo, Sinh viên...",
      "nameHint": "Đặt tên mô tả cho persona của bạn",
      "nicknameLabel": "BIỆT DANH (TÙY CHỌN)",
      "nicknamePlaceholder": "VD: Biến thể công việc, Chế độ RPG...",
      "nicknameHint": "Một biệt danh riêng để phân biệt các biến thể của persona này trong thư viện của bạn",
      "descriptionLabel": "MÔ TẢ",
      "descriptionPlaceholder": "Mô tả cách AI nên xưng hô với bạn, sở thích, lý lịch, hoặc phong cách giao tiếp...",
      "wordCount": "từ",
      "descriptionHint": "Hãy cụ thể về cách bạn muốn được xưng hô",
      "setAsDefault": "Đặt làm mặc định",
      "defaultDescription": "Sử dụng persona này cho tất cả cuộc trò chuyện mới",
      "exportButton": "Xuất Persona"
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
      "off": "Tắt",
      "offDesc": "Cho phép tất cả nội dung",
      "low": "Thấp",
      "lowDesc": "Chặn nội dung tình dục rõ ràng + từ xúc phạm",
      "standard": "Tiêu chuẩn",
      "standardDesc": "Chặn NSFW + bạo lực đồ họa",
      "strict": "Nghiêm ngặt",
      "strictDesc": "Lọc tối đa + không có giọng gợi ý"
    }
  },
  "advanced": {
    "sectionTitles": {
      "aiFeatures": "Tính năng AI",
      "memorySystem": "Hệ thống bộ nhớ",
      "usageAnalytics": "Phân tích sử dụng"
    },
    "creationHelper": {
      "title": "Trợ lý tạo",
      "description": "Trình hướng dẫn tạo nhân vật bằng AI"
    },
    "helpMeReply": {
      "title": "Giúp tôi trả lời",
      "description": "Gợi ý trả lời bằng AI"
    },
    "dynamicMemory": {
      "title": "Bộ nhớ động",
      "contextWindow": "Cửa sổ ngữ cảnh",
      "contextWindowDesc": "Số tin nhắn gần đây cần bao gồm (1-1000)",
      "infoText": "Bộ nhớ Động sử dụng AI để tự động tóm tắt và quản lý ngữ cảnh cuộc trò chuyện, cho phép các cuộc trò chuyện dài hơn và mạch lạc hơn.",
      "disabledText": "Khi bị tắt, ứng dụng sử dụng cửa sổ trượt đơn giản của các tin nhắn gần đây được xác định bởi cài đặt Cửa sổ Ngữ cảnh."
    },
    "usageAnalytics": {
      "recalculateTitle": "Tính lại chi phí sử dụng",
      "recalculateDesc": "Cập nhật tất cả bản ghi sử dụng lịch sử với giá đúng",
      "recalculating": "Đang tính lại...",
      "recalculateButton": "Tính lại tất cả chi phí",
      "openRouterApiKeyRequired": "Cần có khóa API OpenRouter. Cấu hình trong Cài đặt → Nhà cung cấp.",
      "importantLabel": "Quan trọng:",
      "warningCannotUndo": "Thao tác này không thể hoàn tác",
      "warningMayTakeTime": "Có thể mất thời gian nếu bạn có nhiều bản ghi",
      "warningOnlyOpenRouter": "Chỉ cập nhật bản ghi OpenRouter có token",
      "warningExistingValues": "Giá trị chi phí hiện có sẽ bị ghi đè"
    },
    "extra": {
      "creationHelperDetail": "Get intelligent suggestions for personality traits, backstory, and dialogue style",
      "helpMeReplyDetail": "Generate contextual response options based on conversation history",
      "lorebookEntryGenerator": "Lorebook Entry Generator",
      "lorebookEntryDesc": "Turn selected chat messages into durable lorebook entries and configure the draft prompts for entry writing and keyword generation.",
      "companions": "Companions",
      "companionModeDesc": "Manage local analysis models for emotion, entity extraction, and memory routing used by companion characters.",
      "companionSoulWriter": "Trình viết Companion Soul",
      "companionSoulDesc": "Chọn mô hình, mô hình dự phòng và mẫu prompt dùng để soạn Companion Souls. Ưu tiên tool-calling, dùng fallback có cấu trúc nếu không hỗ trợ.",
      "network": "Network",
      "apiServer": "API Server",
      "apiServerDesc": "Expose models via an OpenAI-compatible API server",
      "apiServerRunning": "Server is currently running"
    }
  },
  "backup": {
    "tabs": {
      "create": "Tạo"
    },
    "create": {
      "newBackupButton": "Sao lưu mới",
      "exportDescription": "Xuất tất cả dữ liệu với mã hóa",
      "createButton": "Tạo bản sao lưu"
    },
    "restore": {
      "availableBackups": "Bản sao lưu có sẵn",
      "browseFiles": "Duyệt tệp",
      "noBackupsFound": "Không tìm thấy bản sao lưu",
      "noBackupsDesc": "Tạo bản sao lưu hoặc nhấn \"Duyệt tệp\" để tìm",
      "browseDesc": "Duyệt tìm tệp .lettuce",
      "restoreDialogTitle": "Khôi phục bản sao lưu",
      "deleteDialogTitle": "Xóa bản sao lưu",
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
    "title": "Đặt lại tất cả",
    "description": "Thao tác này sẽ xóa vĩnh viễn tất cả nhà cung cấp, mô hình, nhân vật, phiên trò chuyện và tùy chọn khỏi thiết bị này.",
    "warning": "Thao tác này không thể hoàn tác",
    "resetButton": "Đặt lại tất cả dữ liệu",
    "confirmTitle": "Bạn có chắc không?",
    "confirmDescription": "Tất cả dữ liệu của bạn sẽ bị xóa vĩnh viễn. Ứng dụng sẽ trở về thiết lập ban đầu.",
    "confirmButton": "Có, đặt lại tất cả"
  },
  "chatAppearance": {
    "typography": "Kiểu chữ",
    "fontSize": {
      "label": "Cỡ chữ",
      "small": "Nhỏ",
      "medium": "Vừa",
      "large": "Lớn",
      "xLarge": "Rất lớn"
    },
    "lineSpacing": {
      "label": "Khoảng cách dòng",
      "tight": "Chặt",
      "normal": "Bình thường",
      "relaxed": "Thoải mái"
    },
    "messageBubbles": {
      "label": "Bong bóng tin nhắn",
      "style": {
        "label": "Kiểu",
        "bordered": "Có viền",
        "filled": "Đặc",
        "minimal": "Tối giản"
      },
      "cornerRadius": {
        "label": "Bán kính góc",
        "sharp": "Sắc",
        "rounded": "Bo tròn",
        "pill": "Viên thuốc"
      },
      "maxWidth": {
        "label": "Chiều rộng tối đa",
        "compact": "Gọn",
        "normal": "Bình thường",
        "wide": "Rộng"
      },
      "padding": {
        "label": "Khoảng đệm",
        "compact": "Gọn",
        "normal": "Bình thường",
        "spacious": "Rộng rãi"
      }
    },
    "layout": {
      "label": "Bố cục",
      "messageSpacing": "Khoảng cách tin nhắn",
      "tight": "Chặt",
      "normal": "Bình thường",
      "relaxed": "Thoải mái"
    },
    "avatar": {
      "shape": {
        "label": "Hình dạng Avatar",
        "circle": "Tròn",
        "rounded": "Bo tròn",
        "hidden": "Ẩn"
      },
      "size": {
        "label": "Kích thước Avatar",
        "small": "Nhỏ",
        "medium": "Vừa",
        "large": "Lớn"
      }
    },
    "colors": {
      "label": "Màu sắc",
      "userBubble": "Màu bong bóng người dùng",
      "assistantBubble": "Màu bong bóng trợ lý",
      "userBubbleHex": "Mã hex bong bóng người dùng",
      "assistantBubbleHex": "Mã hex bong bóng trợ lý",
      "neutral": "Trung tính",
      "accent": "Nhấn",
      "info": "Thông tin",
      "secondary": "Phụ",
      "warning": "Cảnh báo",
      "textColors": "Text Colors",
      "messageTextHex": "Màu trích dẫn nội dòng",
      "plainTextHex": "Plain Text Color",
      "italicTextHex": "Italic Text Color",
      "quotedTextHex": "Màu trích dẫn khối",
      "inlineCodeTextHex": "Màu mã nội dòng"
    },
    "backgroundTransparency": {
      "label": "Nền & Trong suốt",
      "backgroundDim": "Làm tối nền",
      "backgroundBlur": "Làm mờ nền",
      "bubbleBlur": "Làm mờ bong bóng",
      "none": "Không",
      "light": "Nhẹ",
      "medium": "Vừa",
      "heavy": "Đậm",
      "bubbleOpacity": "Độ mờ bong bóng"
    },
    "textColorMode": {
      "label": "Chế độ màu chữ",
      "auto": "Tự động",
      "light": "Sáng",
      "dark": "Tối"
    },
    "preview": {
      "label": "Xem trước",
      "generic": "Chung",
      "live": "Trực tiếp"
    },
    "extra": {
      "reset": "Reset"
    }
  },
  "colorCustomization": {
    "tokens": {
      "surface": "Bề mặt",
      "surfaceDesc": "Nền trang",
      "surfaceEl": "Bề mặt nâng",
      "surfaceElDesc": "Thẻ, cửa sổ, phần tử nổi",
      "nav": "Thanh điều hướng",
      "navDesc": "Thanh trên & dưới",
      "foreground": "Tiền cảnh",
      "foregroundDesc": "Viền, lớp phủ, điều hướng và thành phần giao diện",
      "appText": "Văn bản ứng dụng",
      "appTextDesc": "Văn bản chính và nhãn giao diện",
      "appTextMuted": "Văn bản phụ",
      "appTextMutedDesc": "Văn bản phụ và nội dung hỗ trợ",
      "appTextSubtle": "Văn bản nhẹ",
      "appTextSubtleDesc": "Gợi ý, văn bản trợ giúp và chỗ giữ chỗ",
      "accent": "Nhấn",
      "accentDesc": "Hành động chính, thành công",
      "info": "Thông tin",
      "infoDesc": "Trạng thái thông tin, liên kết",
      "warning": "Cảnh báo",
      "warningDesc": "Trạng thái cẩn trọng, cảnh báo",
      "danger": "Nguy hiểm",
      "dangerDesc": "Hành động phá hủy, lỗi",
      "secondary": "Phụ",
      "secondaryDesc": "Tính năng AI, công cụ sáng tạo"
    },
    "presetsLabel": "Preset",
    "customPresetsLabel": "Preset tùy chỉnh",
    "previewLabel": "Xem trước",
    "settingsCardsLabel": "Thẻ cài đặt",
    "settingsCardsOpacity": "Độ mờ thẻ",
    "settingsCardsOpacityDesc": "Điều chỉnh mức độ trong suốt của thẻ cài đặt và các hàng danh sách.",
    "importButton": "Nhập",
    "exportButton": "Xuất",
    "resetAllButton": "Đặt lại tất cả",
    "presets": {
      "defaultDark": "Tối mặc định",
      "midnightBlue": "Midnight Blue",
      "warmEarth": "Warm Earth",
      "purpleHaze": "Purple Haze",
      "rosePine": "Rosé Pine",
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
      "monochrome": "Monochrome"
    },
    "groups": {
      "backgrounds": "Nền",
      "content": "Nội dung",
      "semantic": "Ngữ nghĩa"
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
      "info": "Bộ nhớ Động tự động tóm tắt các cuộc trò chuyện để duy trì ngữ cảnh hiệu quả. Chọn một preset hoặc tinh chỉnh cài đặt theo nhu cầu của bạn.",
      "disabledDirectTitle": "Bộ nhớ động đã bị tắt cho trò chuyện trực tiếp",
      "disabledDirectDescription": "Bật công tắc trong tab Trò chuyện Trực tiếp để kích hoạt. Trò chuyện nhóm sử dụng chế độ bộ nhớ theo phiên.",
      "directChats": "Trò chuyện Trực tiếp",
      "groupChats": "Trò chuyện Nhóm",
      "enableDirectChats": "Bật cho Trò chuyện Trực tiếp",
      "groupChatsInfo": "Trò chuyện nhóm sử dụng chế độ bộ nhớ theo phiên. Bật bộ nhớ động trong cài đặt của từng nhóm. Các cài đặt này kiểm soát cách bộ nhớ động hoạt động.",
      "memoryProfile": "Hồ sơ Bộ nhớ",
      "customSettings": "Cài đặt tùy chỉnh - điều chỉnh giá trị trong Tùy chọn Nâng cao bên dưới.",
      "contextEnrichment": "Làm giàu Ngữ cảnh",
      "experimental": "Thử nghiệm",
      "contextEnrichmentDescription": "Sử dụng tin nhắn gần đây để truy xuất bộ nhớ thông minh hơn",
      "advancedOptions": "Tùy chọn Nâng cao",
      "advancedOptionsDescription": "Tinh chỉnh hành vi bộ nhớ",
      "summaryInterval": "Khoảng cách Tóm tắt",
      "summaryIntervalDescription": "Số tin nhắn giữa các lần tóm tắt",
      "maxMemoryEntries": "Số mục Bộ nhớ Tối đa",
      "maxMemoryEntriesDescription": "Số ký ức được lưu trữ tối đa",
      "hotMemoryBudget": "Ngân sách Bộ nhớ Nóng",
      "hotMemoryBudgetDescription": "Giới hạn token cho ký ức đang hoạt động",
      "relevanceThreshold": "Ngưỡng Liên quan",
      "relevanceThresholdDescription": "Độ tương đồng tối thiểu để truy xuất",
      "retrievalMode": "Chế độ Truy xuất",
      "retrievalModeSmart": "Thông minh",
      "retrievalModeCosine": "Cosine",
      "retrievalModeDescription": "Thông minh kết hợp mức độ liên quan với tần suất/gần đây. Cosine sử dụng độ tương đồng cao nhất thuần túy.",
      "retrievalLimit": "Giới hạn Truy xuất",
      "retrievalLimitDescription": "Số ký ức tối đa được chọn mỗi lượt",
      "decayRate": "Tốc độ Suy giảm",
      "decayRateDescription": "Mức độ quan trọng giảm nhanh như thế nào",
      "coldStorageThreshold": "Ngưỡng Lưu trữ Lạnh",
      "coldStorageThresholdDescription": "Khi nào ký ức được chuyển vào lưu trữ",
      "sharedSettings": "Cài đặt Chung",
      "summarisationModel": "Mô hình Tóm tắt",
      "selectedModel": "Mô hình đã chọn",
      "useGlobalDefaultModel": "Sử dụng mô hình mặc định toàn cục",
      "noModelsAvailable": "Không có mô hình nào",
      "summarisationModelDescription": "Được sử dụng để tóm tắt cuộc trò chuyện",
      "modelManagement": "Quản lý Mô hình",
      "testModel": "Kiểm tra Mô hình",
      "downloadModel": "Tải Mô hình",
      "delete": "Xóa",
      "embeddingModel": "Mô hình Embedding",
      "tokenCapacity": "Dung lượng Token",
      "tokenCapacityDescription": "Giá trị cao hơn = bộ nhớ tốt hơn cho cuộc trò chuyện dài hơn",
      "keepModelLoaded": "Giữ Mô hình Đã tải",
      "keepModelLoadedDescription": "Giữ mô hình embedding + tokenizer trong bộ nhớ để tránh chi phí tải lại",
      "installedModel": "Mô hình đã cài: {{version}} ({{tokens}} token tối đa)",
      "downloadEmbeddingModel": "Tải Mô hình Embedding",
      "downloadEmbeddingDescription": "Chọn phiên bản để tải. Các phiên bản đã cài đặt bị vô hiệu hóa.",
      "downloadVersion": "Tải {{version}}",
      "downloadV2Description": "Tối ưu cho độ chính xác và khả năng nhớ ngữ cảnh dài",
      "downloadV3Description": "Chất lượng embedding mới nhất",
      "installed": "Đã cài đặt",
      "selectModel": "Chọn Mô hình",
      "searchModels": "Tìm mô hình...",
      "deleteEmbeddingTitle": "Xóa mô hình {{version}}?",
      "deleteEmbeddingMessage": "Bạn có chắc muốn xóa {{version}}? Bạn có thể tải lại sau.",
      "msgsUnit": "tin nhắn",
      "entriesUnit": "mục",
      "tokensUnit": "token",
      "itemsUnit": "mục",
      "perCycleUnit": "/ chu kỳ"
    },
    "presets": {
      "minimal": "tối thiểu",
      "balanced": "cân bằng",
      "comprehensive": "toàn diện",
      "minimalDesc": "Fast & efficient. Keeps only essential memories.",
      "balancedDesc": "Good mix of context retention and performance.",
      "comprehensiveDesc": "Maximum context. Best for long, detailed conversations."
    },
    "presetInfo": {
      "minimal": "Nhanh và hiệu quả. Chỉ giữ những ký ức thiết yếu.",
      "balanced": "Kết hợp tốt giữa duy trì ngữ cảnh và hiệu suất.",
      "comprehensive": "Ngữ cảnh tối đa. Tốt nhất cho các cuộc trò chuyện dài và chi tiết."
    }
  },
  "helpMeReply": {
    "page": {
      "info": "Giúp Tôi Trả lời tạo các gợi ý theo ngữ cảnh cho tin nhắn tiếp theo của bạn dựa trên lịch sử trò chuyện. Cấu hình mô hình và phong cách phản hồi bên dưới."
    },
    "labels": {
      "replyModel": "Mô hình Trả lời",
      "selectedModel": "Mô hình đã chọn",
      "useAppDefault": "Sử dụng mặc định ứng dụng{{model}}",
      "useAppDefaultBase": "Sử dụng mặc định ứng dụng",
      "noModelsAvailable": "Không có mô hình nào",
      "replyModelDescription": "Mô hình AI để tạo gợi ý trả lời",
      "streamingOutput": "Đầu ra Streaming",
      "streamingDescription": "Hiển thị gợi ý khi đang được tạo",
      "maxTokens": "Token Tối đa",
      "maxTokensDescription": "Độ dài tối đa của gợi ý",
      "conversationalHint": "Các gợi ý sẽ được viết dưới dạng đối thoại tự nhiên, phù hợp cho trò chuyện thông thường.",
      "roleplayHint": "Các gợi ý sẽ bao gồm các yếu tố nhập vai như *hành động* và mô tả tường thuật.",
      "footerInfo": "Cài đặt này áp dụng toàn cục cho tất cả cuộc trò chuyện. Số token thấp hơn tạo gợi ý ngắn hơn, nhanh hơn trong khi số cao hơn cho phép phản hồi chi tiết hơn.",
      "selectReplyModel": "Chọn Mô hình Trả lời",
      "searchModels": "Tìm mô hình..."
    },
    "sectionTitles": {
      "modelConfiguration": "Cấu hình mô hình",
      "responseStyle": "Phong cách phản hồi"
    },
    "responseStyle": {
      "conversational": "Trò chuyện",
      "conversationalDesc": "Giọng tự nhiên, thoải mái",
      "roleplay": "Nhập vai",
      "roleplayDesc": "Hành động trong nhân vật"
    },
    "extra": {
      "conversational": "Conversational",
      "roleplay": "Roleplay"
    }
  },
  "imageGeneration": {
    "promptPlaceholder": "Mô tả hình ảnh bạn muốn tạo...",
    "labels": {
      "model": "MÔ HÌNH",
      "prompt": "LỜI NHẮC",
      "size": "KÍCH THƯỚC",
      "quality": "CHẤT LƯỢNG",
      "style": "PHONG CÁCH",
      "searchModels": "Tìm kiếm mô hình...",
      "selectAvatarModel": "Chọn mẫu Avatar",
      "selectSceneModel": "Chọn mô hình cảnh",
      "selectWriterModel": "Chọn mô hình viết cảnh",
      "useFirstAvailable": "Sử dụng mô hình có sẵn đầu tiên",
      "useFirstCompatible": "Dùng mô hình viết tương thích đầu tiên"
    },
    "mode": {
      "title": "Chế độ",
      "description": "Chọn cách xử lý các prompt cảnh được phát hiện từ đầu ra của mô hình.",
      "auto": "Tự động",
      "autoDescription": "Tạo ảnh cảnh ngay khi mô hình cung cấp prompt cảnh.",
      "askFirst": "Hỏi trước",
      "askFirstDescription": "Hiển thị prompt cảnh đã phát hiện và chờ bạn chấp thuận trước khi tạo ảnh.",
      "manual": "Thủ công",
      "manualDescription": "Bỏ qua các prompt cảnh trong phản hồi của mô hình. Chỉ dùng các hành động do người dùng tự kích hoạt."
    },
    "empty": {
      "title": "Không có mô hình hình ảnh",
      "description": "Thêm mô hình tạo hình ảnh từ trang Mô hình để bắt đầu tạo hình ảnh."
    },
    "sections": {
      "avatar": {
        "title": "Thế hệ Avatar",
        "description": "Mô hình mặc định được sử dụng khi tạo hình đại diện từ bộ chọn hình đại diện hoặc luồng hình ảnh hồ sơ liên quan."
      },
      "scene": {
        "title": "Tạo cảnh",
        "description": "Mô hình dành riêng cho hình ảnh cảnh được tạo từ bối cảnh hội thoại hoặc lời nhắc cảnh."
      },
      "writer": {
        "title": "Trình viết cảnh",
        "description": "Mô hình văn bản đa phương thức dành riêng để soạn prompt cảnh và mô tả tham chiếu thiết kế từ ngữ cảnh chat, avatar và hình ảnh tham chiếu."
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
      "diagnostics": "Chẩn đoán",
      "generate": "Tạo",
      "copy": "Sao chép"
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
      "testDataGenerators": "Trình tạo dữ liệu thử",
      "storageMaintenance": "Bảo trì lưu trữ",
      "usageTracking": "Theo dõi sử dụng",
      "crashTesting": "Kiểm tra sự cố",
      "environmentInfo": "Thông tin môi trường"
    },
    "testData": {
      "generateCharacter": "Tạo nhân vật thử",
      "generateCharacterDesc": "Tạo một nhân vật thử nghiệm",
      "generatePersona": "Tạo persona thử",
      "generatePersonaDesc": "Tạo một persona thử nghiệm",
      "generateSession": "Tạo phiên thử",
      "generateSessionDesc": "Tạo phiên trò chuyện thử với nhân vật hiện có",
      "generateBulk": "Tạo dữ liệu thử hàng loạt",
      "generateBulkDesc": "Tạo 3 nhân vật và 2 persona"
    },
    "storageMaintenance": {
      "optimizeDb": "Tối ưu cơ sở dữ liệu",
      "optimizeDbDesc": "Áp dụng PRAGMA và chạy VACUUM (chỉ di động)",
      "backupLegacy": "Sao lưu & Xóa tệp cũ",
      "backupLegacyDesc": "Di chuyển tệp .bin cũ vào thư mục sao lưu"
    },
    "usageTracking": {
      "recalculateAll": "Tính lại tất cả chi phí sử dụng",
      "recalculateAllDesc": "Lấy lại giá và tính lại chi phí cho tất cả bản ghi sử dụng OpenRouter"
    },
    "crashTesting": {
      "forceCrash": "Ứng dụng gặp sự cố ngay bây giờ",
      "forceCrashDesc": "Chấm dứt ngay lập tức quá trình ứng dụng gốc để kiểm tra tính năng phát hiện sự cố",
      "forceCrashConfirm": "Điều này sẽ ngay lập tức làm hỏng ứng dụng để kiểm tra trình phát hiện sự cố. Tiếp tục?"
    },
    "environmentInfo": {
      "mode": "Chế độ",
      "devMode": "Chế độ phát triển",
      "viteVersion": "Phiên bản Vite"
    },
    "status": {
      "testCharacterCreated": "✓ Nhân vật thử đã tạo thành công",
      "testPersonaCreated": "✓ Persona thử đã tạo thành công",
      "testSessionCreated": "✓ Phiên thử đã tạo: {{id}}",
      "generatingBulkData": "Đang tạo dữ liệu thử hàng loạt...",
      "bulkDataCreated": "✓ Dữ liệu thử hàng loạt đã tạo: 3 nhân vật, 2 persona",
      "creatingBenchmarkChat": "Đang tạo nhân vật và phiên benchmark...",
      "seededBenchmarkReady": "✓ Benchmark sẵn sàng: {{name}} / {{id}}",
      "creatingBenchmarkGroupChat": "Đang tạo nhóm trò chuyện benchmark...",
      "seededGroupBenchmarkReady": "✓ Benchmark nhóm sẵn sàng: {{id}}",
      "dbOptimized": "✓ Cơ sở dữ liệu đã tối ưu",
      "recalculatingCosts": "Đang tính lại chi phí sử dụng... Có thể mất một lúc.",
      "toursReset": "✓ Tất cả hướng dẫn đã đặt lại — sẽ hiện lại ở lần truy cập tiếp theo",
      "crashingApp": "Đang gây sự cố ứng dụng..."
    },
    "errors": {
      "noCharacters": "Không có nhân vật nào. Hãy tạo nhân vật thử trước.",
      "createCharacterFailed": "Không thể tạo nhân vật thử: {{error}}",
      "createPersonaFailed": "Không thể tạo persona thử: {{error}}",
      "createSessionFailed": "Không thể tạo phiên thử: {{error}}",
      "createBulkFailed": "Không thể tạo dữ liệu thử hàng loạt: {{error}}",
      "createBenchmarkFailed": "Không thể tạo phiên benchmark: {{error}}",
      "createGroupBenchmarkFailed": "Không thể tạo phiên nhóm benchmark: {{error}}",
      "dbOptimizeFailed": "Tối ưu DB thất bại: {{error}}",
      "backupFailed": "Sao lưu thất bại: {{error}}",
      "openRouterKeyMissing": "Không tìm thấy khóa API OpenRouter. Vui lòng cấu hình trong Cài đặt > Nhà cung cấp trước.",
      "recalculationFailed": "Tính lại thất bại: {{error}}",
      "resetToursFailed": "Không thể đặt lại hướng dẫn: {{error}}",
      "crashFailed": "Không thể gây sự cố ứng dụng: {{error}}"
    },
    "onboarding": {
      "title": "Giới thiệu",
      "resetTours": "Đặt lại tất cả hướng dẫn",
      "resetToursDesc": "Xóa trạng thái đã xem của mọi hướng dẫn onboarding để chúng phát lại ở lần truy cập tiếp theo."
    },
    "benchmarks": {
      "createChat": "Tạo trò chuyện benchmark",
      "createChatDesc": "Tạo nhân vật bộ nhớ động, cảnh mở đầu và phiên thử liên tục 20 tin nhắn, rồi mở nó.",
      "createGroupChat": "Tạo nhóm trò chuyện benchmark",
      "createGroupChatDesc": "Tạo nhóm trò chuyện bộ nhớ động với ba nhân vật benchmark và 30 tin nhắn, rồi mở nó."
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
      "oneK": "1K token",
      "oneKDesc": "Tốt nhất cho phản hồi nhanh",
      "twoK": "2K token",
      "twoKDesc": "Hiệu suất cân bằng",
      "fourK": "4K token",
      "fourKDesc": "Ngữ cảnh tối đa"
    },
    "extra": {
      "status": "Downloading..."
    }
  },
  "embeddingTest": {
    "categories": {
      "semanticSimilarity": "Tương đồng ngữ nghĩa",
      "dissimilarityCheck": "Kiểm tra khác biệt",
      "roleplayContext": "Ngữ cảnh nhập vai"
    },
    "extra": {
      "placeholder": "Enter text to embed..."
    }
  },
  "discovery": {
    "tabs": {
      "forYou": "Dành cho bạn",
      "trending": "Xu hướng",
      "popular": "Phổ biến",
      "new": "Mới"
    },
    "searchPlaceholder": "Tìm nhân vật...",
    "viewAll": "Xem tất cả",
    "errorTitle": "Đã xảy ra lỗi",
    "noCardsFound": "Không tìm thấy thẻ nào",
    "sections": {
      "trendingNow": "Xu hướng hiện tại",
      "trendingSubtitle": "Hot tuần này",
      "mostPopular": "Phổ biến nhất",
      "popularSubtitle": "Yêu thích cộng đồng",
      "freshArrivals": "Mới đến",
      "freshSubtitle": "Vừa được thêm"
    },
    "browse": {
      "newArrivals": "Mới đến",
      "freshCharacters": "Nhân vật mới",
      "noCharactersFound": "Không tìm thấy nhân vật",
      "noCharactersSubtitle": "Quay lại sau để xem nội dung mới"
    },
    "sort": {
      "mostLiked": "Được thích nhất",
      "mostDownloaded": "Tải nhiều nhất",
      "mostViewed": "Xem nhiều nhất",
      "mostMessages": "Nhiều tin nhắn nhất",
      "newestFirst": "Mới nhất trước",
      "recentlyUpdated": "Cập nhật gần đây",
      "nameAZ": "Tên (A-Z)"
    },
    "sortBy": "Sắp xếp theo",
    "resultsUnit": "nhân vật",
    "detail": {
      "share": "Chia sẻ",
      "nsfwOverlay": "Nội dung NSFW",
      "nsfwBadge": "NSFW",
      "originalBadge": "Gốc",
      "lorebookBadge": "Sách lore",
      "alsoKnownAs": "Còn được gọi là:",
      "followersUnit": "người theo dõi",
      "sections": {
        "description": "Mô tả",
        "tokenUsage": "Sử dụng token",
        "startingScenes": "Bối cảnh mở đầu",
        "scenario": "Kịch bản",
        "personality": "Tính cách",
        "stats": "Thống kê",
        "tags": "Nhãn",
        "author": "Tác giả"
      },
      "tokensTotalLabel": "tổng",
      "tokens": {
        "description": "Mô tả",
        "personality": "Tính cách",
        "scenario": "Kịch bản",
        "firstMessage": "Tin nhắn đầu tiên",
        "scenes": "Bối cảnh",
        "examples": "Ví dụ",
        "systemPrompt": "Lời nhắc hệ thống"
      },
      "sceneLabels": {
        "primary": "Chính",
        "alternate": "Thay thế"
      },
      "stats": {
        "views": "Lượt xem",
        "downloads": "Lượt tải",
        "messages": "Tin nhắn"
      },
      "downloaded": "Đã tải",
      "startChat": "Bắt đầu trò chuyện",
      "downloadCharacter": "Tải nhân vật",
      "downloading": "Đang tải...",
      "downloadSuccess": {
        "title": "Đã tải nhân vật!",
        "subtitle": "Đã thêm vào thư viện",
        "badge": "Đã lưu",
        "startChat": "Bắt đầu trò chuyện",
        "startChatDesc": "Mở bối cảnh đầu tiên ngay",
        "viewLibrary": "Xem trong thư viện",
        "viewLibraryDesc": "Sửa, quản lý hoặc xuất sau",
        "continueBrowsing": "Tiếp tục duyệt",
        "continueBrowsingDesc": "Quay lại khám phá"
      },
      "errorTitle": "Lỗi",
      "errorSubtitle": "Không thể tải",
      "errorNotFound": "Không tìm thấy nhân vật",
      "defaultChatTitle": "New Chat"
    },
    "search": {
      "placeholder": "Tìm nhân vật, nhãn, tác giả...",
      "resultsUnit": "kết quả",
      "timingUnit": "ms",
      "recentSearches": "Tìm kiếm gần đây",
      "clearAll": "Xóa tất cả",
      "trendingSearches": "Tìm kiếm xu hướng",
      "trends": {
        "anime": "anime",
        "fantasy": "fantasy",
        "romance": "romance",
        "villain": "villain",
        "adventure": "adventure",
        "comedy": "comedy",
        "mystery": "mystery",
        "sciFi": "sci-fi"
      },
      "tips": {
        "title": "Mẹo tìm kiếm",
        "tip1": "Tìm theo tên nhân vật, tác giả hoặc mô tả",
        "tip2": "Dùng nhãn như \"anime\", \"fantasy\", hoặc \"romance\"",
        "tip3": "Thử đặc điểm cụ thể như \"tsundere\" hoặc \"villain\""
      },
      "loading": "Đang tải...",
      "loadMore": "Tải thêm",
      "noResults": "Không tìm thấy kết quả",
      "noResultsFor": "Không tìm thấy nhân vật cho",
      "noResultsHint": "Thử từ khóa khác hoặc duyệt danh mục"
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
    "gpuInsufficient": "Bộ nhớ GPU không đủ",
    "gpuFallbackDesc": "Mô hình này không vừa bộ nhớ GPU. Chuyển sang CPU (chậm hơn) hay hủy bỏ?",
    "switchToCpu": "Chuyển sang CPU",
    "abort": "Hủy bỏ",
    "errors": {
      "providerNotFound": "Không tìm thấy nhà cung cấp Engine.",
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
      "connected": "Đã kết nối",
      "offline": "Ngoại tuyến",
      "needsSetup": "Cần thiết lập"
    },
    "home": {
      "characters": "Nhân vật",
      "newButton": "Mới",
      "noCharactersFound": "Không tìm thấy nhân vật.",
      "tokenUsage": "Sử dụng token",
      "totalTokens": "tổng token",
      "backgroundActivity": "Hoạt động nền",
      "quickActions": "Hành động nhanh",
      "configureProviders": "Cấu hình nhà cung cấp",
      "engineSettings": "Cài đặt Engine",
      "chat": "Trò chuyện",
      "chatDesc": "Bắt đầu cuộc trò chuyện với nhân vật này",
      "deleteCharacter": "Xóa nhân vật",
      "deletingCharacter": "Đang xóa...",
      "deleteDesc": "Xóa vĩnh viễn nhân vật này",
      "character": "Nhân vật",
      "never": "Chưa bao giờ",
      "justNow": "Vừa xong",
      "timeAgo": {
        "minutes": "{{n}}m ago",
        "hours": "{{n}}h ago",
        "days": "{{n}}d ago"
      }
    },
    "tokens": {
      "input": "Đầu vào",
      "output": "Đầu ra"
    },
    "activity": {
      "synthesis": "Tổng hợp",
      "consolidation": "Hợp nhất",
      "bm25Rebuild": "Xây dựng lại BM25",
      "dripResearch": "Nghiên cứu từng bước",
      "running": "Đang chạy",
      "stopped": "Đã dừng"
    },
    "setup": {
      "complete": "Thiết lập hoàn tất!",
      "completeMessage": "Lettuce Engine của bạn đã được cấu hình và sẵn sàng.",
      "openDashboard": "Mở bảng điều khiển"
    },
    "welcome": {
      "title": "Chào mừng đến Lettuce Engine",
      "subtitle": "Hãy cấu hình engine nhân vật AI của bạn. Việc này mất khoảng 2 phút.",
      "feature1": "Engine mang lại cho nhân vật AI bộ nhớ bền vững, cảm xúc, mối quan hệ và danh tính thực sự.",
      "feature2": "Đầu tiên, chúng ta sẽ thiết lập backend LLM, sau đó cấu hình cài đặt engine.",
      "getStarted": "Bắt đầu"
    },
    "config": {
      "activeProviders": "Nhà cung cấp đang hoạt động",
      "noModelSet": "Chưa đặt mô hình",
      "defaultBadge": "Mặc định",
      "noProvidersWarning": "Chưa cấu hình nhà cung cấp. Thêm ít nhất một backend LLM bên dưới.",
      "addProvider": "Thêm nhà cung cấp",
      "quickImport": "Nhập nhanh từ nhà cung cấp ứng dụng",
      "importButton": "Nhập",
      "fields": {
        "model": "Mô hình",
        "modelPlaceholder": "VD: claude-sonnet-4-5-20250929",
        "apiKey": "Khóa API",
        "apiKeyPlaceholder": "Nhập khóa API của bạn",
        "currentKey": "Khóa hiện tại:",
        "baseUrl": "URL cơ sở",
        "baseUrlPlaceholder": "http://localhost:11434",
        "maxTokens": "Token tối đa",
        "temperature": "Nhiệt độ"
      },
      "enableProvider": "Bật nhà cung cấp",
      "setAsDefault": "Đặt làm mặc định",
      "defaultBackend": "Backend mặc định",
      "remove": "Xóa",
      "saveChanges": "Lưu thay đổi",
      "saving": "Đang lưu...",
      "saved": "Đã lưu"
    },
    "providers": {
      "title": "Nhà cung cấp LLM",
      "subtitle": "Engine cần ít nhất một backend LLM để hoạt động. Cấu hình một hoặc nhiều nhà cung cấp bên dưới.",
      "importFromProviders": "Nhập từ nhà cung cấp của bạn",
      "imported": "Đã nhập",
      "use": "Sử dụng",
      "saveContinue": "Lưu & Tiếp tục"
    },
    "settings": {
      "fields": {
        "dataDirectory": "Thư mục dữ liệu",
        "logLevel": "Mức nhật ký",
        "maxHistory": "Lịch sử tối đa (lượt trò chuyện)"
      },
      "logLevels": {
        "debug": "GỠ LỖI",
        "info": "THÔNG TIN",
        "warning": "CẢNH BÁO",
        "error": "LỖI"
      },
      "sections": {
        "engine": "Engine",
        "backgroundLoops": "Vòng lặp nền",
        "memory": "Bộ nhớ",
        "safety": "An toàn",
        "research": "Nghiên cứu"
      },
      "backgroundLoops": {
        "synthesis": "Tổng hợp (phút)",
        "consolidation": "Hợp nhất (phút)",
        "bm25Rebuild": "Xây dựng lại BM25 (phút)",
        "dripResearch": "Nghiên cứu từng bước (phút)"
      },
      "memory": {
        "embeddingModel": "Mô hình Embedding",
        "maxRetrieval": "Kết quả truy xuất tối đa",
        "denseWeight": "Trọng số Dense",
        "bm25Weight": "Trọng số BM25",
        "graphWeight": "Trọng số Graph",
        "recencyBoost": "Tăng cường gần đây (giờ)",
        "randomSurface": "Xác suất hiển thị ngẫu nhiên"
      },
      "safety": {
        "honestySection": "Phần trung thực",
        "honestyDesc": "Bao gồm phần trung thực trong lời nhắc hệ thống",
        "userDataDeletion": "Xóa dữ liệu người dùng",
        "userDataDesc": "Cho phép người dùng yêu cầu xóa dữ liệu"
      },
      "research": {
        "scrapeOnBoot": "Thu thập khi khởi động",
        "scrapeDesc": "Chạy thu thập nghiên cứu khi khởi động engine",
        "periodicInterval": "Khoảng định kỳ (giờ)"
      },
      "saveChanges": "Lưu thay đổi",
      "saving": "Đang lưu...",
      "saved": "Đã lưu"
    },
    "settingsStep": {
      "title": "Cài đặt Engine",
      "subtitle": "Cấu hình cài đặt toàn cục engine. Tất cả đều có giá trị mặc định hợp lý — hãy bỏ qua nếu muốn.",
      "completingSetup": "Đang hoàn tất thiết lập...",
      "completeSetup": "Hoàn tất thiết lập"
    },
    "chat": {
      "sendMessage": "Gửi tin nhắn...",
      "sendButton": "Gửi tin nhắn",
      "typeMessage": "Nhập tin nhắn",
      "back": "Quay lại",
      "assistantTyping": "Assistant is typing",
      "fallbackName": "Chat"
    },
    "tagInput": {
      "addMore": "Thêm...",
      "typeAndPressEnter": "Nhập và ấn Enter"
    },
    "characterCreate": {
      "steps": {
        "identity": {
          "title": "Danh tính",
          "aiGenerated": "AI tạo",
          "nameLabel": "Tên *",
          "namePlaceholder": "Tên nhân vật",
          "eraLabel": "Thời đại",
          "eraPlaceholder": "VD: hiện đại, Victoria",
          "roleLabel": "Vai trò",
          "rolePlaceholder": "VD: Thám tử, Nhà khoa học",
          "settingLabel": "Bối cảnh",
          "settingPlaceholder": "Mô tả nơi nhân vật sống (ngôi thứ nhất)...",
          "coreIdentityLabel": "Danh tính cốt lõi",
          "coreIdentityPlaceholder": "Nhân vật này là ai ở cốt lõi? (ngôi thứ nhất, 3-5 câu)",
          "backstoryLabel": "Lý lịch",
          "backstoryPlaceholder": "Câu chuyện cuộc đời và sự kiện quan trọng (ngôi thứ nhất)..."
        },
        "mode": {
          "title": "Tạo nhân vật",
          "subtitle": "Tạo nhân vật bằng AI hoặc xây dựng từ đầu.",
          "aiBoost": "Hỗ trợ AI",
          "aiBoostDesc": "Mô tả ý tưởng nhân vật và AI sẽ tạo định nghĩa nhân vật đầy đủ.",
          "nameOptional": "Tên (tùy chọn)",
          "namePlaceholder": "VD: Nguyễn Văn An",
          "seedDescription": "Mô tả ban đầu *",
          "seedPlaceholder": "VD: nghệ sĩ piano jazz ở Harlem thập niên 1950, triết lý, thích trò chuyện đêm khuya",
          "eraOptional": "Thời đại (tùy chọn)",
          "eraPlaceholder": "VD: thập niên 1950, hiện đại, Victoria",
          "generating": "Đang tạo...",
          "generateCharacter": "Tạo nhân vật",
          "or": "hoặc",
          "startFromScratch": "Bắt đầu từ đầu"
        },
        "personality": {
          "title": "Tính cách",
          "traits": "Đặc điểm tính cách",
          "traitsPlaceholder": "VD: hài hước, từ bi, bướng bỉnh",
          "speechPatterns": "Mẫu lời nói",
          "formality": "Trang trọng",
          "formal": "Trang trọng",
          "casual": "Thường ngày",
          "texting": "Nhắn tin",
          "verbosity": "Mức độ chi tiết",
          "terse": "Ngắn gọn",
          "medium": "Vừa phải",
          "verbose": "Chi tiết",
          "textStyle": "Phong cách văn bản",
          "dialect": "Phương ngữ",
          "dialectPlaceholder": "VD: Bắc, Nam, Trung",
          "catchphrases": "Câu cửa miệng",
          "catchphrasesPlaceholder": "VD: Thế đấy...",
          "vocabPreferences": "Từ vựng ưa thích",
          "vocabPreferencesPlaceholder": "Từ họ hay dùng",
          "vocabAvoidances": "Từ vựng tránh",
          "vocabAvoidancesPlaceholder": "Từ họ tránh dùng",
          "fillerWords": "Từ đệm",
          "fillerWordsPlaceholder": "VD: ừm, kiểu, bạn biết đấy",
          "exampleQuotes": "Câu nói ví dụ",
          "exampleQuotesPlaceholder": "3-5 câu thoại ví dụ"
        },
        "world": {
          "title": "Thế giới & Hành vi",
          "knowledgeDomains": "Lĩnh vực kiến thức",
          "knowledgeDomainsPlaceholder": "VD: lịch sử jazz, lý thuyết âm nhạc",
          "knowledgeBoundaries": "Giới hạn kiến thức",
          "knowledgeBoundariesPlaceholder": "Chủ đề họ không biết",
          "researchSeeds": "Chủ đề nghiên cứu ban đầu",
          "researchSeedsPlaceholder": "Chủ đề khởi đầu cho nghiên cứu nền",
          "researchEnabled": "Nghiên cứu đã bật",
          "researchEnabledDesc": "Cho phép thu thập kiến thức nền",
          "physicalDescription": "Mô tả ngoại hình",
          "physicalDescPlaceholder": "Ngoại hình và cách cư xử...",
          "physicalHabits": "Thói quen vật lý",
          "physicalHabitsPlaceholder": "VD: gõ ngón tay, chỉnh kính",
          "idleBehaviors": "Hành vi khi rảnh",
          "idleBehaviorsPlaceholder": "Họ làm gì khi không bận",
          "timeBehaviors": "Hành vi theo thời gian",
          "timePlaceholder": "Họ làm gì vào {{period}}?",
          "earlyMorning": "Sáng sớm",
          "morning": "Buổi sáng",
          "afternoon": "Buổi chiều",
          "evening": "Buổi tối",
          "night": "Đêm",
          "baselineEmotions": "Cảm xúc cơ bản (Plutchik)",
          "emotionDesc": "Đặt đường cảm xúc cơ bản (0 = không, 1 = tối đa)",
          "joy": "Vui vẻ",
          "trust": "Tin tưởng",
          "fear": "Sợ hãi",
          "surprise": "Ngạc nhiên",
          "sadness": "Buồn bã",
          "disgust": "Ghê tởm",
          "anger": "Tức giận",
          "anticipation": "Kỳ vọng",
          "engineOverrides": "Ghi đè Engine",
          "backend": "Backend",
          "model": "Mô hình",
          "temperature": "Nhiệt độ",
          "leaveEmpty": "Để trống cho mặc định"
        },
        "review": {
          "title": "Xem lại",
          "subtitle": "Xem lại nhân vật trước khi tạo.",
          "edit": "Sửa",
          "notSet": "Chưa đặt",
          "identitySection": "Danh tính",
          "personalitySection": "Tính cách",
          "worldSection": "Thế giới & Hành vi",
          "nameLabel": "Tên",
          "eraLabel": "Thời đại",
          "roleLabel": "Vai trò",
          "settingLabel": "Bối cảnh",
          "coreIdentityLabel": "Danh tính cốt lõi",
          "backstoryLabel": "Lý lịch",
          "traitsLabel": "Đặc điểm",
          "formalityLabel": "Trang trọng",
          "verbosityLabel": "Chi tiết",
          "dialectLabel": "Phương ngữ",
          "catchphrasesLabel": "Câu cửa miệng",
          "domainsLabel": "Lĩnh vực",
          "boundariesLabel": "Giới hạn",
          "researchSeedsLabel": "Chủ đề nghiên cứu",
          "researchLabel": "Nghiên cứu",
          "enabled": "Đã bật",
          "disabled": "Đã tắt",
          "physicalLabel": "Ngoại hình",
          "habitsLabel": "Thói quen",
          "idleLabel": "Khi rảnh",
          "timeBehaviorsLabel": "Hành vi theo thời gian",
          "emotionsLabel": "Cảm xúc",
          "configured": "Đã cấu hình",
          "backendLabel": "Backend",
          "modelLabel": "Mô hình",
          "temperatureLabel": "Nhiệt độ",
          "creating": "Đang tạo...",
          "createCharacter": "Tạo nhân vật"
        }
      }
    }
  },
  "library": {
    "filterTitle": "Lọc thư viện",
    "filters": {
      "all": "Tất cả",
      "characters": "Nhân vật",
      "personas": "Persona",
      "lorebooks": "Sách lore",
      "images": "Images"
    },
    "emptyStates": {
      "all": {
        "title": "Thư viện trống",
        "description": "Tạo nhân vật, persona và sách lore để xem ở đây"
      },
      "characters": {
        "title": "Chưa có nhân vật nào",
        "description": "Tạo nhân vật đầu tiên để bắt đầu trò chuyện"
      },
      "personas": {
        "title": "Chưa có persona nào",
        "description": "Tạo persona để tùy chỉnh danh tính trò chuyện"
      },
      "lorebooks": {
        "title": "Chưa có sách lore nào",
        "description": "Sách lore được tạo từ trong cài đặt nhân vật"
      }
    },
    "actions": {
      "startChat": "Bắt đầu trò chuyện",
      "editCharacter": "Sửa nhân vật",
      "editPersona": "Sửa Persona",
      "editLorebook": "Sửa sách lore",
      "renameLorebook": "Đổi tên sách lore",
      "exportCharacter": "Xuất nhân vật",
      "exportPersona": "Xuất Persona",
      "chatAppearance": "Giao diện trò chuyện",
      "deleteCharacter": "Xóa nhân vật",
      "deletePersona": "Xóa Persona",
      "deleteLorebook": "Xóa sách lore",
      "importLorebook": "Nhập sách lore"
    },
    "imageLibrary": {
      "filters": {
        "all": "Tất cả",
        "backgrounds": "Nền",
        "avatars": "Avatar",
        "attachments": "Tệp đính kèm",
        "other": "Khác"
      },
      "searchPlaceholder": "Tìm theo tên tệp, đường dẫn, id phiên hoặc id thực thể",
      "empty": {
        "title": "Không có hình ảnh nào khớp với chế độ xem này",
        "description": "Hãy thử bộ lọc hoặc từ khóa khác. Thư viện chỉ hiển thị các hình ảnh đã được lưu trong bộ nhớ cục bộ của ứng dụng."
      },
      "actions": {
        "sort": "Sắp xếp",
        "useThis": "Dùng ảnh này",
        "using": "Đang dùng...",
        "copyPath": "Sao chép đường dẫn",
        "saving": "Đang lưu...",
        "download": "Tải xuống",
        "delete": "Xóa ảnh",
        "deleting": "Đang xóa..."
      },
      "active": "Đang dùng",
      "messages": {
        "loadFailed": "Không thể tải thư viện ảnh",
        "saved": "Đã lưu ảnh",
        "downloadFailed": "Tải xuống thất bại",
        "useFailed": "Không thể dùng ảnh này",
        "deleted": "Đã xóa ảnh",
        "deleteFailed": "Không thể xóa ảnh"
      },
      "deleteConfirm": {
        "title": "Xóa ảnh?",
        "message": "Bạn có chắc muốn xóa \"{{filename}}\" không? Việc này có thể làm hỏng avatar, nền chat hoặc tệp đính kèm tin nhắn vẫn đang dùng ảnh này."
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
      "title": "Xóa {{itemType}}?",
      "message": "Bạn có chắc muốn xóa",
      "characterWarning": "Điều này cũng sẽ xóa tất cả phiên trò chuyện với nhân vật này."
    },
    "rename": {
      "title": "Đổi tên sách lore",
      "placeholder": "Nhập tên mới..."
    },
    "itemTypes": {
      "character": "Nhân vật",
      "persona": "Persona",
      "lorebook": "Sách lore"
    },
    "lorebookLabel": "Sách lore",
    "noDescriptionYet": "Chưa có mô tả",
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
    "loading": "Đang tải nhà cung cấp...",
    "stepIndicator": "Bước {{current}} / {{total}}",
    "steps": {
      "provider": "Thiết lập nhà cung cấp",
      "model": "Thiết lập mô hình",
      "memory": "Hệ thống bộ nhớ",
      "stepNofM": "Step {{current}} of {{total}}"
    },
    "provider": {
      "availableProviders": "Nhà cung cấp có sẵn",
      "chooseProvider": "Chọn nhà cung cấp",
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
      "noProvidersTitle": "Chưa cấu hình nhà cung cấp",
      "noProvidersDesc": "Bạn cần kết nối nhà cung cấp trước khi chọn mô hình mặc định.",
      "goToProviderSetup": "Đi đến thiết lập nhà cung cấp",
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
      "dynamicTitle": "Bộ nhớ động",
      "recommended": "Khuyến nghị",
      "settingUp": "Đang thiết lập...",
      "finishSetup": "Hoàn tất thiết lập",
      "promptTitle": "Thiết lập bộ nhớ động",
      "oneLastStep": "Bước cuối cùng",
      "downloadAndEnable": "Tải & Bật",
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
      "tagline": "Trợ lý AI cá nhân. Riêng tư, an toàn, và luôn trên thiết bị.",
      "features": {
        "onDevice": "Chỉ trên thiết bị",
        "characterReady": "Nhân vật sẵn sàng"
      },
      "betaWarning": {
        "title": "Phiên bản Beta Desktop",
        "description": "Bạn đang dùng phiên bản desktop. Một số tính năng có thể khác với di động. Báo lỗi trên GitHub."
      },
      "languageSelector": {
        "title": "Ngôn ngữ",
        "description": "Tự động phát hiện từ thiết bị của bạn. Bạn có thể thay đổi bất cứ lúc nào trong cài đặt."
      },
      "getStarted": "Bắt đầu",
      "skipForNow": "Bỏ qua",
      "restoreFromBackup": "Khôi phục từ bản sao lưu",
      "setupTime": "Thiết lập mất dưới 2 phút",
      "skipWarning": {
        "title": "Bỏ qua thiết lập?",
        "warningTitle": "Cần nhà cung cấp để trò chuyện",
        "warningMessage": "Không có nhà cung cấp, bạn sẽ không thể gửi tin nhắn. Bạn có thể thêm sau từ cài đặt.",
        "addProvider": "Thêm nhà cung cấp",
        "skipAnyway": "Vẫn bỏ qua"
      },
      "restoreBackup": {
        "title": "Khôi phục bản sao lưu",
        "selectMessage": "Chọn bản sao lưu để khôi phục.",
        "browse": "Duyệt tệp",
        "processing": "Đang xử lý tệp...",
        "processingNote": "Bản sao lưu lớn có thể mất một phút",
        "noBackups": "Không tìm thấy bản sao lưu",
        "noBackupsHint": "Nhấn duyệt để chọn tệp .lettuce",
        "browseLettuce": "Duyệt tìm tệp .lettuce",
        "passwordLabel": "Mật khẩu bản sao lưu",
        "passwordPlaceholder": "Nhập mật khẩu",
        "restoreButton": "Khôi phục",
        "restoring": "Đang khôi phục...",
        "infoMessage": "Thao tác này sẽ thiết lập ứng dụng với dữ liệu đã sao lưu, bao gồm nhân vật, cuộc trò chuyện và cài đặt.",
        "embeddingTitle": "Cần mô hình Embedding",
        "dynamicMemoryDetected": "Phát hiện bộ nhớ động",
        "dynamicMemoryMessage": "Bản sao lưu này chứa nhân vật có bộ nhớ động, cần mô hình embedding (~120MB).",
        "embeddingOptions": "Bạn có thể tải mô hình ngay để bật bộ nhớ động, hoặc tiếp tục không có (bộ nhớ động sẽ bị tắt cho các nhân vật bị ảnh hưởng).",
        "downloadModel": "Tải mô hình",
        "continueWithoutDynamic": "Tiếp tục không có bộ nhớ động",
        "embeddingNote": "Bạn có thể bật lại bộ nhớ động sau trong cài đặt nhân vật sau khi tải mô hình.",
        "back": "Back",
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
      "back": "Back",
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
    "placeholder": "Tìm kiếm...",
    "tabs": {
      "characters": "Nhân vật",
      "personas": "Persona"
    },
    "noResults": "Không tìm thấy {{type}}",
    "emptyState": "Chưa có {{type}} nào",
    "noResultsHint": "Thử từ khóa khác",
    "emptyCharacters": "Tạo nhân vật đầu tiên để bắt đầu trò chuyện",
    "emptyPersonas": "Tạo persona trong cài đặt",
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
      "join": "Tham gia",
      "joinDesc": "Kết nối đến máy chủ",
      "host": "Lưu trữ",
      "hostDesc": "Chia sẻ dữ liệu"
    },
    "sections": {
      "mode": "Chế độ",
      "connectToHost": "Kết nối đến máy chủ",
      "startHosting": "Bắt đầu lưu trữ",
      "status": "Trạng thái",
      "hosting": "Dịch vụ lưu trữ",
      "localAddress": "Địa chỉ mạng cục bộ",
      "connectionPin": "PIN kết nối",
      "setupGuide": "Hướng dẫn thiết lập"
    },
    "fields": {
      "hostAddress": "Địa chỉ hoặc JSON máy chủ",
      "hostPlaceholder": "VD: 192.168.1.100:12345",
      "pinCode": "Mã PIN",
      "pinPlaceholder": "000000"
    },
    "buttons": {
      "scanQRCode": "Quét mã QR",
      "connect": "Kết nối",
      "connecting": "Đang kết nối...",
      "startHosting": "Bắt đầu lưu trữ",
      "startingServer": "Đang khởi động máy chủ...",
      "stopHosting": "Dừng lưu trữ",
      "hostAgain": "Lưu trữ lại",
      "done": "Xong"
    },
    "status": {
      "connecting": "Đang kết nối...",
      "connected": "Đã kết nối",
      "waitingConfirmation": "Đang chờ xác nhận",
      "waitingConfirmationDesc": "Phê duyệt kết nối trên thiết bị chủ để tiếp tục.",
      "syncing": "Đang đồng bộ...",
      "transferringData": "Đang chuyển dữ liệu",
      "syncInProgress": "Đồng bộ đang tiến hành",
      "live": "Trực tiếp",
      "broadcasting": "Đang phát",
      "clientsLabel": "Đã kết nối",
      "clientsUnit": "Thiết bị"
    },
    "pinDescription": "Chia sẻ mã PIN này với thiết bị kết nối",
    "hostingDesc1": "Các thiết bị khác có thể kết nối và đồng bộ dữ liệu từ thiết bị này.",
    "hostingDesc2": "Dữ liệu của bạn sẽ được chia sẻ với các thiết bị đã kết nối.",
    "setupSteps": {
      "step1": "Mở ứng dụng trên thiết bị khác",
      "step2": "Đi đến Cài đặt → Đồng bộ cục bộ",
      "step3": "Quét mã QR hoặc nhập địa chỉ"
    },
    "messages": {
      "completed": "Đồng bộ hoàn tất!",
      "completedDesc": "Tất cả dữ liệu đã đồng bộ",
      "error": "Lỗi kết nối",
      "outdatedClient": "Phát hiện thiết bị lỗi thời"
    },
    "disclaimer": "Đồng bộ hoạt động qua mạng cục bộ. Cả hai thiết bị phải cùng WiFi.",
    "modals": {
      "connectionRequest": "Yêu cầu kết nối",
      "requestMessage": "muốn đồng bộ với thiết bị này.",
      "acceptConnection": "Chấp nhận kết nối",
      "acceptDesc": "Cho phép thiết bị này đồng bộ dữ liệu",
      "decline": "Từ chối",
      "declineDesc": "Chặn yêu cầu kết nối này",
      "readyToSync": "Sẵn sàng đồng bộ",
      "connectionEstablished": "Kết nối đã thiết lập",
      "deviceReady": "đã sẵn sàng.",
      "startSyncMessage": "Nhấn bên dưới để bắt đầu đồng bộ dữ liệu.",
      "startSyncing": "Bắt đầu đồng bộ",
      "startSyncingDesc": "Bắt đầu chuyển dữ liệu ngay"
    },
    "scanner": {
      "title": "Quét mã QR",
      "cancel": "Hủy quét"
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
      "info": "Trợ lý Tạo hướng dẫn bạn xây dựng nhân vật với sự hỗ trợ của AI. Cấu hình mô hình và công cụ được sử dụng trong quá trình tạo nhân vật.",
      "modelConfiguration": "Cấu hình Mô hình",
      "chatModel": "Mô hình Trò chuyện",
      "selectedModel": "Mô hình đã chọn",
      "useAppDefault": "Sử dụng mặc định ứng dụng{{model}}",
      "useAppDefaultBase": "Sử dụng mặc định ứng dụng",
      "noModelsAvailable": "Không có mô hình nào",
      "chatModelDescription": "Mô hình AI cho cuộc trò chuyện tạo nhân vật",
      "streamingOutput": "Đầu ra Streaming",
      "streamingDescription": "Hiển thị phản hồi khi đang được tạo",
      "imageGenerationModel": "Mô hình Tạo Hình ảnh",
      "noModelSelected": "Chưa chọn mô hình",
      "noImageModelsAvailable": "Không có mô hình hình ảnh nào",
      "imageModelDescription": "Để tạo avatar nhân vật",
      "toolSelection": "Chọn Công cụ",
      "smartToolSelection": "Chọn Công cụ Thông minh",
      "smartToolDescription": "AI tự động chọn công cụ nào sẽ sử dụng",
      "smartToolEnabledHint": "Khi được bật, Trợ lý Tạo AI hỏi bạn muốn tạo gì và chỉ tải bộ công cụ liên quan.",
      "smartToolDisabledHint": "Khi bị tắt, Trợ lý Tạo AI mở trực tiếp và sử dụng tất cả công cụ đã bật; trợ lý quyết định sẽ xây dựng gì.",
      "quickPresets": "Preset Nhanh",
      "customSelection": "Chọn tùy chỉnh - {{count}} công cụ đã bật",
      "footerInfo": "Khi Chọn Công cụ Thông minh được bật, AI quyết định sử dụng công cụ nào dựa trên ngữ cảnh. Tắt để kiểm soát thủ công các công cụ có sẵn.",
      "selectChatModel": "Chọn Mô hình Trò chuyện",
      "selectImageModel": "Chọn Mô hình Hình ảnh",
      "searchModels": "Tìm mô hình..."
    },
    "categories": {
      "basic": "Cơ bản",
      "content": "Nội dung",
      "visual": "Hình ảnh",
      "settings": "Cài đặt",
      "flow": "Luồng",
      "persona": "Persona",
      "lorebook": "Sách lore"
    },
    "presets": {
      "all": {
        "name": "Tất cả công cụ",
        "desc": "Bật tất cả công cụ có sẵn"
      },
      "essential": {
        "name": "Thiết yếu",
        "desc": "Chỉ tên, định nghĩa và bối cảnh"
      },
      "minimal": {
        "name": "Tối thiểu",
        "desc": "Chỉ tên và định nghĩa"
      }
    },
    "tools": {
      "setName": "Đặt tên",
      "setNameDesc": "Đặt tên nhân vật",
      "setDefinition": "Đặt định nghĩa",
      "setDefinitionDesc": "Đặt tính cách và lý lịch",
      "set_character_name": {
        "name": "Đặt tên",
        "desc": "Đặt tên nhân vật"
      },
      "set_character_definition": {
        "name": "Đặt định nghĩa",
        "desc": "Đặt tính cách và lý lịch"
      },
      "add_scene": {
        "name": "Thêm bối cảnh",
        "desc": "Thêm bối cảnh mở đầu cho nhập vai"
      },
      "update_scene": {
        "name": "Cập nhật bối cảnh",
        "desc": "Thay đổi bối cảnh hiện có"
      },
      "toggle_avatar_gradient": {
        "name": "Gradient Avatar",
        "desc": "Bật/tắt gradient trên avatar"
      },
      "set_default_model": {
        "name": "Đặt mô hình",
        "desc": "Đặt mô hình AI cho cuộc trò chuyện"
      },
      "set_system_prompt": {
        "name": "Lời nhắc hệ thống",
        "desc": "Đặt hướng dẫn hành vi"
      },
      "get_system_prompt_list": {
        "name": "Danh sách lời nhắc",
        "desc": "Xem lời nhắc có sẵn"
      },
      "get_model_list": {
        "name": "Danh sách mô hình",
        "desc": "Xem mô hình có sẵn"
      },
      "use_uploaded_image_as_avatar": {
        "name": "Ảnh làm Avatar",
        "desc": "Dùng ảnh đã tải làm avatar"
      },
      "use_uploaded_image_as_chat_background": {
        "name": "Ảnh làm nền",
        "desc": "Dùng ảnh đã tải làm nền"
      },
      "generate_image": {
        "name": "Tạo hình ảnh",
        "desc": "Tạo hình ảnh bằng mô hình AI"
      },
      "show_preview": {
        "name": "Xem trước",
        "desc": "Xem trước nhân vật"
      },
      "request_confirmation": {
        "name": "Yêu cầu xác nhận",
        "desc": "Hỏi lưu hoặc tiếp tục"
      },
      "list_personas": {
        "name": "Danh sách Persona",
        "desc": "Duyệt persona"
      },
      "upsert_persona": {
        "name": "Lưu Persona",
        "desc": "Tạo hoặc cập nhật persona"
      },
      "use_uploaded_image_as_persona_avatar": {
        "name": "Avatar Persona",
        "desc": "Dùng ảnh đã tải làm avatar persona"
      },
      "delete_persona": {
        "name": "Xóa Persona",
        "desc": "Xóa persona"
      },
      "get_default_persona": {
        "name": "Persona mặc định",
        "desc": "Lấy persona mặc định"
      },
      "list_lorebooks": {
        "name": "Danh sách sách lore",
        "desc": "Duyệt sách lore"
      },
      "upsert_lorebook": {
        "name": "Lưu sách lore",
        "desc": "Tạo hoặc cập nhật sách lore"
      },
      "delete_lorebook": {
        "name": "Xóa sách lore",
        "desc": "Xóa sách lore"
      },
      "list_lorebook_entries": {
        "name": "Danh sách mục",
        "desc": "Xem mục sách lore"
      },
      "get_lorebook_entry": {
        "name": "Lấy mục",
        "desc": "Lấy mục sách lore"
      },
      "upsert_lorebook_entry": {
        "name": "Lưu mục",
        "desc": "Tạo hoặc cập nhật mục"
      },
      "delete_lorebook_entry": {
        "name": "Xóa mục",
        "desc": "Xóa mục sách lore"
      },
      "create_blank_lorebook_entry": {
        "name": "Mục trống",
        "desc": "Tạo mục giữ chỗ"
      },
      "reorder_lorebook_entries": {
        "name": "Sắp xếp lại mục",
        "desc": "Thay đổi thứ tự mục"
      },
      "list_character_lorebooks": {
        "name": "Sách lore nhân vật",
        "desc": "Xem sách lore của nhân vật"
      },
      "set_character_lorebooks": {
        "name": "Đặt sách lore nhân vật",
        "desc": "Gán sách lore cho nhân vật"
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
    "stepCounter": "Bước {{current}} / {{total}}",
    "skipTour": "Bỏ qua hướng dẫn",
    "next": "Tiếp",
    "gotIt": "Đã hiểu",
    "appShell": {
      "chats": {
        "title": "Đây là nơi các cuộc trò chuyện sống",
        "body": "Tất cả cuộc trò chuyện riêng của bạn với nhân vật đều ở đây. Quay lại bất cứ lúc nào và chúng tôi sẽ giữ chỗ cho bạn."
      },
      "groups": {
        "title": "Trò chuyện nhóm",
        "body": "Đưa nhiều nhân vật vào cùng phòng và xem họ nói chuyện với nhau, hoặc tham gia bất cứ khi nào bạn muốn."
      },
      "discover": {
        "title": "Khám phá nhân vật mới",
        "body": "Duyệt qua những gì cộng đồng đã chia sẻ và lấy bất kỳ nhân vật nào bạn thích. Yêu thích mới chỉ cách một chạm."
      },
      "library": {
        "title": "Thư viện cá nhân",
        "body": "Mọi thứ bạn đã tạo hoặc lưu đều ở đây: nhân vật, persona, prompt, tất tần tật. Hãy coi đây là kho của bạn."
      },
      "settings": {
        "title": "Tùy chỉnh theo ý bạn",
        "body": "Đổi nhà cung cấp, chọn mô hình khác, điều chỉnh giao diện. Hầu hết mọi thứ đều có thể điều chỉnh từ cài đặt."
      },
      "search": {
        "title": "Tìm mọi thứ, nhanh chóng",
        "body": "Đang tìm một cuộc trò chuyện hay nhân vật cụ thể? Tìm kiếm mọi thứ từ đây. Không cần đào bới."
      },
      "create": {
        "title": "Và cuối cùng, hãy tạo!",
        "body": "Nhấn dấu cộng khi cảm hứng đến. Tạo nhân vật mới, persona, hoặc bắt đầu từ đầu."
      }
    },
    "chatDetail": {
      "chatTitle": {
        "title": "Cài đặt riêng từng cuộc trò chuyện",
        "body": "Nhấn tên nhân vật ở trên để mở cài đặt chỉ cho cuộc trò chuyện này. Persona, bố cục và mô hình khác nhau cho mỗi cuộc trò chuyện."
      },
      "chatMemory": {
        "title": "Họ nhớ gì",
        "body": "Biểu tượng não cho thấy nhân vật nhớ gì về cuộc trò chuyện của bạn. Nhấn để xem lại, chỉnh sửa hoặc xóa ký ức."
      },
      "chatSearch": {
        "title": "Tìm dòng đó",
        "body": "Tìm kiếm chỉ trong cuộc trò chuyện này. Tuyệt vời để tìm chi tiết từ 200 tin nhắn trước mà không cần cuộn mãi."
      },
      "chatLorebook": {
        "title": "Mục lorebook",
        "body": "Thông tin bổ sung, xây dựng thế giới và ngữ cảnh được đưa vào prompt khi xuất hiện từ khóa cụ thể. Bảng ghi chú của nhân vật."
      },
      "chatPlus": {
        "title": "Đính kèm",
        "body": "Thả hình ảnh hoặc mở menu bổ sung. Bất cứ thứ gì đính kèm sẽ được gửi cùng tin nhắn tiếp theo."
      },
      "chatComposer": {
        "title": "Tin nhắn của bạn",
        "body": "Gõ tại đây. Enter gửi, Shift+Enter xuống dòng. Mẹo: nhấn giữ bất kỳ tin nhắn nào để chỉnh sửa, rẽ nhánh hoặc xóa."
      },
      "chatSend": {
        "title": "Một nút, bốn việc",
        "body": "Nút gửi thay đổi chức năng tùy theo tình huống:"
      }
    },
    "postFirstMessage": {
      "chatRegenerate": {
        "title": "Không hài lòng? Tạo lại",
        "body": "Nhấn biểu tượng làm mới để nhận câu trả lời hoàn toàn mới từ nhân vật. Mỗi lần tạo lại được lưu như một biến thể."
      },
      "chatVariants": {
        "title": "Vuốt giữa các biến thể",
        "body": "Sau khi tạo lại, bạn sẽ thấy bộ đếm biến thể bên dưới tin nhắn. Vuốt trái hoặc phải trên bong bóng tin nhắn để xem tất cả các câu trả lời khác nhau."
      },
      "chatLongPress": {
        "title": "Còn nhiều thứ ẩn ở đây",
        "body": "Nhấn giữ bất kỳ tin nhắn nào để chỉnh sửa, sao chép, rẽ nhánh, ghim, xóa hoặc tua lại cuộc trò chuyện. Nhấp chuột phải cũng hoạt động trên desktop."
      }
    },
    "sendButton": {
      "continue": {
        "label": "Tiếp tục",
        "desc": "Ô nhập trống. Nhấn để nhân vật tiếp tục nói."
      },
      "send": {
        "label": "Gửi",
        "desc": "Bạn đã gõ hoặc đính kèm nội dung. Nhấn để gửi."
      },
      "sending": {
        "label": "Đang gửi",
        "desc": "Câu trả lời đang đến. Nút bị khóa."
      },
      "stop": {
        "label": "Dừng",
        "desc": "Nhấn để hủy giữa chừng nếu bạn đổi ý."
      }
    },
    "extra": {
      "rerunOnboarding": "Rerun onboarding"
    }
  },
  "sessionAdvanced": {
    "title": "Tham số phiên",
    "subtitle": "Ghi đè mặc định mô hình cho cuộc trò chuyện này",
    "goBack": "Quay lại",
    "support": "Hỗ trợ",
    "reset": "Đặt lại",
    "save": "Lưu",
    "noSessionWarning": "Mở phiên trò chuyện để cấu hình cài đặt riêng từng phiên.",
    "overrideDefaults": "Ghi đè mặc định",
    "overrideDefaultsDesc": "Tùy chỉnh tham số chỉ cho cuộc trò chuyện này",
    "loadingContextInfo": "Đang tải thông tin ngữ cảnh...",
    "sampling": {
      "title": "Lấy mẫu",
      "temperature": "Temperature",
      "temperatureDesc": "Kiểm soát tính ngẫu nhiên. Thấp hơn = xác định hơn, cao hơn = sáng tạo hơn.",
      "temperaturePrecise": "Chính xác",
      "temperatureCreative": "Sáng tạo",
      "topP": "Top P",
      "topPDesc": "Nucleus sampling. Giới hạn token theo xác suất tích lũy.",
      "topPFocused": "Tập trung",
      "topPDiverse": "Đa dạng",
      "topK": "Top K",
      "topKDesc": "Giới hạn lấy mẫu ở top K token có xác suất cao nhất."
    },
    "outputPenalties": {
      "title": "Đầu ra & Phạt",
      "maxOutputTokens": "Token đầu ra tối đa",
      "maxOutputTokensDesc": "Độ dài phản hồi tối đa. Auto để mô hình tự quyết.",
      "auto": "Auto",
      "custom": "Tùy chỉnh",
      "frequencyPenalty": "Phạt tần suất",
      "frequencyPenaltyDesc": "Giảm lặp lại chuỗi token.",
      "frequencyPenaltyRepeat": "Lặp lại",
      "frequencyPenaltyVary": "Đa dạng",
      "presencePenalty": "Phạt hiện diện",
      "presencePenaltyDesc": "Khuyến khích khám phá chủ đề mới.",
      "presencePenaltyRepeat": "Lặp lại",
      "presencePenaltyExplore": "Khám phá"
    },
    "performance": {
      "title": "Hiệu suất",
      "gpuLayers": "GPU Layers",
      "gpuLayersDesc": "Lớp chuyển sang GPU. 0 = chỉ CPU.",
      "threads": "Threads",
      "threadsDesc": "CPU threads cho suy luận.",
      "batchThreads": "Batch Threads",
      "batchThreadsDesc": "CPU threads cho xử lý batch.",
      "batchSize": "Batch Size",
      "batchSizeDesc": "Kích thước khối xử lý prompt.",
      "contextLength": "Độ dài ngữ cảnh",
      "contextLengthDesc": "Ghi đè kích thước cửa sổ ngữ cảnh.",
      "flashAttention": "Flash Attention",
      "flashAttentionDesc": "Tối ưu bộ nhớ GPU.",
      "enabled": "Bật",
      "disabled": "Tắt"
    },
    "samplingMemory": {
      "title": "Lấy mẫu & Bộ nhớ",
      "minP": "Min P",
      "minPDesc": "Ngưỡng xác suất tối thiểu.",
      "typicalP": "Typical P",
      "typicalPDesc": "Ngưỡng lấy mẫu điển hình.",
      "seed": "Seed",
      "seedDesc": "Seed ngẫu nhiên. Để trống cho ngẫu nhiên.",
      "ropeBase": "RoPE Base",
      "ropeBaseDesc": "Ghi đè cơ sở tần số.",
      "ropeScale": "RoPE Scale",
      "ropeScaleDesc": "Ghi đè tỷ lệ tần số.",
      "kvCacheType": "KV Cache Type",
      "kvCacheTypeDesc": "Lượng tử hóa KV cache để tiết kiệm VRAM.",
      "offloadKqv": "Offload KQV",
      "offloadKqvDesc": "KV cache & KQV ops trên GPU.",
      "on": "Bật",
      "off": "Tắt",
      "samplerProfile": "Hồ sơ bộ lấy mẫu",
      "samplerProfileDesc": "Mặc định được tinh chỉnh cho ổn định hoặc suy luận.",
      "balanced": "Cân bằng",
      "creative": "Sáng tạo",
      "stable": "Ổn định",
      "reasoning": "Suy luận"
    },
    "systemInfo": {
      "title": "Thông tin hệ thống",
      "maxContext": "Ngữ cảnh tối đa",
      "recommended": "Khuyến nghị",
      "availableRam": "RAM khả dụng",
      "availableVram": "VRAM khả dụng",
      "modelSize": "Kích thước mô hình"
    }
  },
  "exportMenu": {
    "title": "Định dạng xuất",
    "selectFormat": "Chọn định dạng",
    "uscTitle": "Unified System Card",
    "formats": {
      "uscPrompt": "Xuất USC di động cho template prompt.",
      "uscLorebook": "Xuất USC di động cho lorebook.",
      "uscModel": "Xuất USC di động cho hồ sơ mô hình.",
      "uscChatTemplate": "Xuất USC di động cho template trò chuyện.",
      "legacyPromptJson": "Legacy Prompt JSON",
      "legacyPromptJsonDesc": "Định dạng gói prompt ngoài hiện tại.",
      "lorebookJson": "Lorebook JSON",
      "lorebookJsonDesc": "Định dạng xuất lorebook hiện tại.",
      "modelJson": "Model JSON",
      "modelJsonDesc": "JSON hồ sơ mô hình an toàn không có thông tin đăng nhập.",
      "chatTemplateJson": "Chat Template JSON",
      "chatTemplateJsonDesc": "Định dạng xuất template trò chuyện gốc."
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
    "title": "Tham chiếu thiết kế",
    "description": "Tải lên vài ảnh tham chiếu rõ ràng cùng một mô tả hình ảnh chuẩn.",
    "descriptionPlaceholder": "Mô tả diện mạo ổn định: khuôn mặt, tóc, thể hình, tuổi, trang phục, phụ kiện và hướng phong cách.",
    "addReferences": "Thêm tham chiếu",
    "visualDescription": "Mô tả hình ảnh",
    "draftWithAi": "Phác thảo với AI",
    "referenceImages": "Ảnh tham chiếu",
    "imageAlt": "Tham chiếu thiết kế {{index}}",
    "loading": "Đang tải...",
    "removeAria": "Xóa tham chiếu thiết kế",
    "noImages": "Chưa có ảnh tham chiếu nào",
    "imageCount": "{{count}} ảnh tham chiếu đính kèm",
    "emptyReferences": "Thêm vài ảnh tham chiếu rõ ràng để cố định khuôn mặt, tỷ lệ, trang phục và phong cách.",
    "noWriterModel": "Thêm mô hình scene writer tương thích trong cài đặt Tạo ảnh trước.",
    "noImagesForGeneration": "Thêm avatar hoặc ít nhất một ảnh tham chiếu trước khi tạo.",
    "writerModelHelp": "Sử dụng {{model}} để phác thảo từ avatar và ảnh tham chiếu của bạn.",
    "noWriterModelHelp": "Thêm mô hình scene writer tương thích trong cài đặt Tạo ảnh để phác thảo tự động.",
    "draftMenuTitle": "Phác thảo AI",
    "draftMenuDesc": "Phác thảo bởi {{model}} từ avatar và ảnh tham chiếu hiện tại.",
    "draftMenuNoWriter": "Thêm mô hình scene writer tương thích trước khi sử dụng trợ giúp này.",
    "regenerate": "Tạo lại",
    "useThis": "Dùng cái này"
  },
  "samplerOrder": {
    "title": "Thứ tự bộ lấy mẫu",
    "description": "Kéo các giai đoạn để sắp xếp lại. Thực thi từ trên xuống dưới.",
    "reset": "Đặt lại",
    "resetAria": "Đặt lại thứ tự bộ lấy mẫu về mặc định",
    "stages": {
      "penalties": {
        "label": "Phạt",
        "desc": "Áp dụng phạt tần suất và hiện diện trước khi lọc."
      },
      "grammar": {
        "label": "Ngữ pháp",
        "desc": "Ràng buộc token khi ngữ pháp template trò chuyện gốc đang hoạt động."
      },
      "topK": {
        "label": "Top K",
        "desc": "Cắt giảm nhóm ứng viên xuống các token mạnh nhất."
      },
      "topP": {
        "label": "Top P",
        "desc": "Áp dụng lọc nucleus cho phân phối còn lại."
      },
      "minP": {
        "label": "Min P",
        "desc": "Loại bỏ token đuôi xác suất thấp bằng ngưỡng tối thiểu."
      },
      "typical": {
        "label": "Typical P",
        "desc": "Ưu tiên token thống kê điển hình trong phân phối hiện tại."
      },
      "temp": {
        "label": "Temperature",
        "desc": "Làm phẳng hoặc làm sắc phân phối cuối cùng trước khi chọn."
      }
    },
    "presets": {
      "default": {
        "label": "Mặc định",
        "hint": "Lettuce local"
      },
      "unsloth": {
        "label": "Unsloth",
        "hint": "llama.cpp style"
      },
      "focused": {
        "label": "Tập trung",
        "hint": "Cắt tỉa chặt"
      },
      "creative": {
        "label": "Sáng tạo",
        "hint": "Lọc muộn"
      }
    }
  },
  "installedModels": {
    "title": "Kho GGUF cục bộ",
    "fileCount": "{{count}} files",
    "searchPlaceholder": "Tìm tên mô hình, tên tệp, đường dẫn, lượng tử hóa hoặc kiến trúc",
    "loading": "Đang quét các mô hình đã cài đặt...",
    "loadFailed": "Không tải được các mô hình đã cài đặt: {{error}}",
    "empty": {
      "title": "Không tìm thấy mô hình GGUF nào đã cài đặt",
      "description": "Hãy tải mô hình trong trình duyệt trước hoặc đặt các tệp `.gguf` vào thư mục mô hình."
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
      "gpuFallbackReason": "Lý do dự phòng GPU",
      "finalError": "Final error",
      "workingRecoveryConfig": "Working recovery config",
      "context": "Context",
      "batch": "Batch",
      "na": "n/a",
      "applyWorkingConfig": "Apply working config",
      "badges": {
        "succeeded": "Run succeeded",
        "cpuFallbackSucceeded": "Dự phòng CPU đã phục hồi",
        "cpuFallbackFailed": "Dự phòng CPU đã thất bại",
        "failed": "Run failed"
      },
      "headline": {
        "succeeded": "The last local run completed successfully.",
        "cpuFallbackSucceeded": "Tải GPU thất bại, sau đó mô hình phục hồi trên CPU.",
        "cpuFallbackFailed": "Tải GPU thất bại và dự phòng CPU vẫn chưa hoàn tất.",
        "failed": "The last local run failed before llama.cpp could complete."
      },
      "detail": {
        "succeeded": "This report also seeds the smart offload cache so future runs can reuse the last stable GPU setup.",
        "cpuFallbackSucceeded": "Chúng tôi đã lưu ngữ cảnh và batch an toàn cho CPU đã chạy được để bạn có thể dùng lại.",
        "cpuFallbackFailed": "Mô hình đã được thử lại trên CPU, nhưng cấu hình phục hồi vẫn thất bại.",
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
      "smartOffloadFallback": "Dự phòng giảm tải thông minh",
      "active": "Active",
      "notNeeded": "Not needed",
      "kqvFallback": "Dự phòng KQV",
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
      "customVoices": "Giọng của tôi",
      "providerVoices": "Giọng nhà cung cấp",
      "myVoices": "Giọng của tôi",
      "page": {
        "noAudioProvidersHint": "Thêm một nhà cung cấp trong Nhà cung cấp → Âm thanh để bắt đầu",
        "noVoicesTitle": "Chưa tạo giọng nói nào",
        "noVoicesDescription": "Tạo giọng nói với lời nhắc tùy chỉnh cho nhân vật của bạn",
        "addProviderFirst": "Thêm nhà cung cấp âm thanh trước",
        "noPrompt": "Không có lời nhắc",
        "noProviderVoices": "Không tìm thấy giọng nào. Nhấn Làm mới để tải giọng.",
        "showLess": "Hiển thị ít hơn",
        "showAllVoices": "Hiển thị tất cả {{count}} giọng",
        "voiceFallbackTitle": "Giọng nói"
      },
      "cache": {
        "section": "Bộ nhớ đệm âm thanh",
        "title": "Bộ nhớ đệm âm thanh TTS",
        "description": "Âm thanh giọng nói đã tạo được lưu vào bộ nhớ đệm để giảm thiểu tái tạo",
        "clearing": "Đang xóa...",
        "clear": "Xóa bộ nhớ đệm"
      },
      "menu": {
        "editDescription": "Sửa đổi giọng nói này",
        "deleteDescription": "Xóa giọng nói này",
        "provider": "Nhà cung cấp",
        "category": "Danh mục",
        "createVoiceConfig": "Tạo cấu hình giọng nói",
        "createVoiceConfigDescription": "Sử dụng giọng nói này với cài đặt tùy chỉnh"
      },
      "editor": {
        "editTitle": "Sửa giọng nói",
        "createTitle": "Tạo giọng nói",
        "voiceName": "Tên giọng nói",
        "voiceNamePlaceholder": "Giọng nhân vật của tôi",
        "provider": "Nhà cung cấp",
        "model": "Mô hình",
        "modelIdPlaceholder": "gpt-4o-mini-tts",
        "modelIdHint": "Nhập ID mô hình chính xác mà điểm cuối tương thích của bạn yêu cầu",
        "elevenlabsVoice": "Giọng ElevenLabs",
        "noVoicesAvailable": "Không có giọng nào",
        "selectVoice": "Chọn một giọng...",
        "elevenlabsVoiceHint": "Chọn từ danh sách giọng ElevenLabs của bạn",
        "geminiVoice": "Giọng Gemini",
        "geminiVoiceHint": "Chọn một giọng Gemini TTS",
        "voiceId": "ID giọng nói",
        "voiceIdPlaceholder": "alloy",
        "voiceIdHint": "Nhập ID giọng nói được hỗ trợ bởi điểm cuối tương thích của bạn",
        "voicePrompt": "Lời nhắc giọng nói",
        "voicePromptPlaceholder": "Một giọng ấm áp, thân thiện với âm điệu vui tươi...",
        "voicePromptHint": "Mô tả cách giọng nói nên nghe",
        "exampleText": "Văn bản ví dụ",
        "exampleTextPlaceholder": "Xin chào! Đây là cách tôi phát âm khi nói...",
        "exampleTextHint": "Văn bản mẫu để kiểm tra giọng nói",
        "voiceDesignChars": "{{current}}/{{minimum}} ký tự cần thiết để xem trước thiết kế giọng",
        "defaultSample": "Xin chào! Đây là cách tôi phát âm khi nói. Tôi có thể đọc các đoạn dài hơn với sự ấm áp, rõ ràng và cảm xúc để bạn đánh giá giọng điệu và nhịp độ của tôi.",
        "playing": "Đang phát...",
        "previewVoice": "Xem trước giọng nói"
      },
      "kokoro": {
        "title": "Kokoro Studio",
        "newBlend": "Pha trộn mới",
        "editBlend": "Sửa pha trộn",
        "tryText": "Xin chào! Đây là bài kiểm tra nhanh về cách tôi phát âm.",
        "experimentDefaultText": "Xin chào! Đây là cách tôi phát âm khi nói. Tôi có thể đọc các đoạn dài hơn với sự ấm áp, rõ ràng và cảm xúc.",
        "livePreview": "Xem trước trực tiếp",
        "savedBlend": "Pha trộn đã lưu",
        "defaultPreviewText": "Xin chào! Đây là bản xem trước nhanh về cách giọng nói này nghe.",
        "experiment": "Thử nghiệm",
        "providerNotFound": "Không tìm thấy nhà cung cấp Kokoro",
        "backToProviders": "Quay lại nhà cung cấp",
        "variantUnset": "Chưa đặt biến thể",
        "ready": "Sẵn sàng",
        "modelNotInstalled": "Mô hình chưa được cài đặt",
        "voiceCount": "{{count}} giọng",
        "engineActions": "Thao tác engine",
        "engineNotInstalled": "Engine chưa được cài đặt",
        "installAtLeastOneVoice": "Cài đặt ít nhất một giọng nói",
        "continueSetup": "Tiếp tục thiết lập để cài đặt mô hình Kokoro.",
        "pickVoiceOrStarter": "Chọn một giọng hoặc lấy gói khởi đầu để bắt đầu.",
        "downloadsFailed": "{{count}} lượt tải thất bại",
        "retryOrDismissAll": "Thử lại từng cái hoặc bỏ qua tất cả.",
        "dismissAll": "Bỏ qua tất cả",
        "model": "Mô hình",
        "voice": "Giọng nói",
        "downloads": "Tải xuống",
        "cancelAll": "Hủy tất cả",
        "experimentPlaceholder": "Nhập một câu để nghe cách phát âm...",
        "speed": "Tốc độ",
        "speak": "Phát",
        "yourBlends": "Pha trộn của bạn",
        "noSavedBlends": "Chưa có pha trộn nào được lưu.",
        "installModelAndVoiceFirst": "Cài đặt mô hình và một giọng nói trước.",
        "featured": "Nổi bật",
        "stop": "Dừng",
        "sample": "Mẫu",
        "voiceLibrary": "Thư viện giọng nói",
        "starterPack": "Gói khởi đầu",
        "select": "Chọn",
        "all": "Tất cả",
        "installed": "Đã cài đặt",
        "installModelToBrowse": "Cài đặt mô hình để duyệt giọng nói.",
        "noVoicesInCatalog": "Không có giọng nào trong danh mục. Nhấn Làm mới.",
        "noVoicesMatch": "Không có giọng nào khớp với bộ lọc của bạn.",
        "collapseAll": "Thu gọn tất cả",
        "expandAll": "Mở rộng tất cả",
        "selectedCount": "Đã chọn {{count}}",
        "engineTitle": "Engine Kokoro",
        "variant": "Biến thể",
        "status": "Trạng thái",
        "notInstalled": "Chưa cài đặt",
        "file": "Tệp",
        "modelSize": "Kích thước mô hình",
        "voicesSize": "Kích thước giọng nói",
        "total": "Tổng cộng",
        "assetRoot": "Thư mục tài nguyên",
        "reinstallModel": "Cài đặt lại mô hình",
        "installModel": "Cài đặt mô hình",
        "deleteModel": "Xóa mô hình",
        "deleteModelDescription": "Giải phóng dung lượng đĩa; giọng nói vẫn được giữ lại.",
        "blend": "Pha trộn",
        "previewDescription": "Nghe nhanh với văn bản mặc định",
        "editBlendDescription": "Điều chỉnh giọng nói, trọng số và tốc độ",
        "deleteBlendDescription": "Xóa giọng nói đã lưu này",
        "setupTitle": "Thiết lập Kokoro",
        "allSet": "Hoàn tất",
        "allSetDescription": "Duyệt giọng nói, thiết kế pha trộn, hoặc thử nghiệm trong khu vực thử nghiệm."
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
      "fallbackFormat": "Định dạng dự phòng"
    }
  },
  "usageAnalytics": {
    "page": {
      "filters": "Bộ lọc",
      "model": "Mô hình",
      "character": "Nhân vật",
      "clearAll": "Xóa tất cả",
      "applyFilters": "Áp dụng bộ lọc",
      "recentActivity": "Hoạt động gần đây",
      "customRange": "Khoảng tùy chỉnh",
      "startDate": "Ngày bắt đầu",
      "endDate": "Ngày kết thúc",
      "applyRange": "Áp dụng khoảng",
      "dashboard": "BẢNG ĐIỀU KHIỂN",
      "appTime": "THỜI GIAN ỨNG DỤNG",
      "today": "Hôm nay",
      "last7Days": "7 ngày",
      "last30Days": "30 ngày",
      "all": "Tất cả",
      "custom": "Tùy chỉnh",
      "filtersCount": "{{count}} bộ lọc",
      "totalCost": "Tổng chi phí",
      "tokens": "Token",
      "avgShort": "TB {{value}}",
      "requests": "Yêu cầu",
      "period": "Khoảng thời gian",
      "last7d": "7 ngày qua",
      "last30d": "30 ngày qua",
      "allTime": "Tất cả thời gian",
      "recordsCount": "{{count}} bản ghi",
      "usageTrend": "Xu hướng sử dụng",
      "tokenConsumptionOverTime": "Mức tiêu thụ token theo thời gian",
      "input": "Đầu vào",
      "output": "Đầu ra",
      "byModel": "Theo mô hình",
      "byCharacter": "Theo nhân vật",
      "top": "Hàng đầu",
      "active": "Hoạt động",
      "quickView": "Xem nhanh",
      "viewAll": "Xem tất cả",
      "startChatting": "Bắt đầu trò chuyện để xem dữ liệu sử dụng",
      "exportedTo": "Đã xuất tới: {{path}}",
      "periodTotal": "Tổng kỳ",
      "daysActive": "{{count}} ngày hoạt động",
      "dailyAvg": "TB hàng ngày",
      "selectedPeriod": "kỳ đã chọn",
      "yesterdayValue": "Hôm qua {{value}}",
      "thirtyDayAvg": "TB 30 ngày",
      "appTimeTrend": "Xu hướng thời gian ứng dụng",
      "usageDurationPerDay": "Thời gian sử dụng mỗi ngày",
      "totalValue": "Tổng {{value}}",
      "activeTime": "Thời gian hoạt động"
    },
    "activity": {
      "loading": "Đang tải hoạt động của bạn...",
      "title": "Hoạt động gần đây",
      "recordsCount": "{{count}} bản ghi sử dụng",
      "rangeOfTotal": "{{start}}-{{end}} trong {{total}}",
      "previous": "Trước",
      "next": "Tiếp",
      "pageOf": "Trang {{page}} trong {{total}}"
    },
    "shared": {
      "relativeTime": {
        "justNow": "vừa xong",
        "minutesAgo": "{{count}} phút trước",
        "hoursAgo": "{{count}} giờ trước",
        "daysAgo": "{{count}} ngày trước"
      },
      "operations": {
        "chat": "Trò chuyện",
        "regenerate": "Tạo lại",
        "continue": "Tiếp tục",
        "summary": "Tóm tắt",
        "memoryManager": "Bộ nhớ",
        "imageGeneration": "Tạo ảnh",
        "aiCreator": "AI Creator",
        "replyHelper": "Hỗ trợ trả lời",
        "groupChatMessage": "Trò chuyện nhóm",
        "groupChatRegenerate": "Tạo lại nhóm",
        "groupChatContinue": "Tiếp tục nhóm",
        "groupChatDecisionMaker": "Người ra quyết định"
      },
      "outputImages": "{{count}} hình ảnh",
      "tokens": "{{count}} token",
      "unknown": "Không rõ",
      "requestDetails": "Chi tiết yêu cầu",
      "noStopReason": "Không có lý do dừng",
      "tokenUsage": "Lượng token sử dụng",
      "estimatedCost": "Chi phí ước tính",
      "stats": {
        "prompt": "Prompt",
        "completion": "Hoàn thành",
        "total": "Tổng cộng",
        "reasoning": "Suy luận",
        "image": "Hình ảnh",
        "memory": "Bộ nhớ",
        "summary": "Tóm tắt",
        "inputImages": "Ảnh đầu vào",
        "outputImages": "Ảnh đầu ra",
        "cachedPrompt": "Prompt đã lưu cache",
        "cacheWrite": "Ghi cache",
        "webSearches": "Tìm kiếm web",
        "cacheRead": "Đọc cache",
        "requestFee": "Phí yêu cầu",
        "webSearch": "Tìm kiếm web",
        "providerTotal": "Tổng nhà cung cấp"
      }
    }
  }
};
