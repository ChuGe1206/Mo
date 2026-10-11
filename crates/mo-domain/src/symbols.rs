/// Categories in the manually opened symbol panel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolCategory {
    /// Facial expression.
    Smile,
    /// Gesture or everyday symbol.
    Gesture,
    /// Text emoticon.
    Kaomoji,
}
/// An owned catalog entry, never supplied by an IPC peer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Symbol {
    /// Unicode sequence committed without truncation or normalization.
    pub text: &'static str,
    /// Accessible description.
    pub label: &'static str,
    /// Filter category.
    pub category: SymbolCategory,
}
/// Version 1 catalog. Wire indices are permanent: never reorder entries.
pub const SYMBOLS_V1: &[Symbol] = &[
    Symbol {
        text: "😀",
        label: "开心",
        category: SymbolCategory::Smile,
    },
    Symbol {
        text: "😃",
        label: "大笑",
        category: SymbolCategory::Smile,
    },
    Symbol {
        text: "😄",
        label: "喜悦",
        category: SymbolCategory::Smile,
    },
    Symbol {
        text: "😁",
        label: "灿烂",
        category: SymbolCategory::Smile,
    },
    Symbol {
        text: "😆",
        label: "笑眯眼",
        category: SymbolCategory::Smile,
    },
    Symbol {
        text: "😅",
        label: "微汗",
        category: SymbolCategory::Smile,
    },
    Symbol {
        text: "😂",
        label: "笑哭",
        category: SymbolCategory::Smile,
    },
    Symbol {
        text: "🤣",
        label: "捧腹",
        category: SymbolCategory::Smile,
    },
    Symbol {
        text: "😊",
        label: "微笑",
        category: SymbolCategory::Smile,
    },
    Symbol {
        text: "🥰",
        label: "喜爱",
        category: SymbolCategory::Smile,
    },
    Symbol {
        text: "😍",
        label: "心动",
        category: SymbolCategory::Smile,
    },
    Symbol {
        text: "🤩",
        label: "惊喜",
        category: SymbolCategory::Smile,
    },
    Symbol {
        text: "😘",
        label: "飞吻",
        category: SymbolCategory::Smile,
    },
    Symbol {
        text: "😎",
        label: "酷",
        category: SymbolCategory::Smile,
    },
    Symbol {
        text: "🤔",
        label: "思考",
        category: SymbolCategory::Smile,
    },
    Symbol {
        text: "😭",
        label: "流泪",
        category: SymbolCategory::Smile,
    },
    Symbol {
        text: "👍",
        label: "赞",
        category: SymbolCategory::Gesture,
    },
    Symbol {
        text: "👏",
        label: "鼓掌",
        category: SymbolCategory::Gesture,
    },
    Symbol {
        text: "🙏",
        label: "感谢",
        category: SymbolCategory::Gesture,
    },
    Symbol {
        text: "👋",
        label: "挥手",
        category: SymbolCategory::Gesture,
    },
    Symbol {
        text: "❤️",
        label: "爱心",
        category: SymbolCategory::Gesture,
    },
    Symbol {
        text: "🎉",
        label: "庆祝",
        category: SymbolCategory::Gesture,
    },
    Symbol {
        text: "🌟",
        label: "星星",
        category: SymbolCategory::Gesture,
    },
    Symbol {
        text: "🌸",
        label: "花朵",
        category: SymbolCategory::Gesture,
    },
    Symbol {
        text: "( •̀ ω •́ )✧",
        label: "加油",
        category: SymbolCategory::Kaomoji,
    },
    Symbol {
        text: "(≧▽≦)",
        label: "开心",
        category: SymbolCategory::Kaomoji,
    },
    Symbol {
        text: "(づ｡◕‿‿◕｡)づ",
        label: "抱抱",
        category: SymbolCategory::Kaomoji,
    },
    Symbol {
        text: "(╥﹏╥)",
        label: "委屈",
        category: SymbolCategory::Kaomoji,
    },
];
