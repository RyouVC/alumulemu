// utility functions for formatting data


// Convert a date of `yyyymmdd` to a date object
export function dateFromYYYYMMDD(yyyymmdd: string): Date {
    return new Date(
        yyyymmdd.replace(/(\d{4})(\d{2})(\d{2})/, "$1-$2-$3")
    );
}

// Format file size in bytes to human-readable form (GB or MB)
export function formatFileSize(sizeInBytes: number): string {
    return sizeInBytes > 1024 * 1024 * 1024
        ? (sizeInBytes / (1024 * 1024 * 1024)).toFixed(2) + " GB"
        : (sizeInBytes / (1024 * 1024)).toFixed(2) + " MB";
}

// Language code to language name mapping
const languageMap: Record<string, string> = {
    "ja": "Japanese",
    "en": "English",
    "fr": "French",
    "de": "German",
    "it": "Italian",
    "es": "Spanish",
    "zh": "Chinese",
    "ko": "Korean",
    "nl": "Dutch",
    "pt": "Portuguese",
    "ru": "Russian",
    "zh-CN": "Chinese (Simplified)",
    "zh-TW": "Chinese (Traditional)",
    "en-GB": "English (UK)",
    "en-US": "English (US)",
    "es-419": "Spanish (Latin America)",
    "es-ES": "Spanish (Spain)",
    "pt-BR": "Portuguese (Brazil)",
    "pt-PT": "Portuguese (Portugal)",
    "fr-CA": "French (Canada)",
    "fr-FR": "French (France)",
    "da": "Danish",
    "fi": "Finnish",
    "nb": "Norwegian",
    "sv": "Swedish",
    "cs": "Czech",
    "hu": "Hungarian",
    "pl": "Polish",
    "ro": "Romanian",
    "tr": "Turkish",
    "ar": "Arabic",
    "he": "Hebrew",
    "th": "Thai",
    "id": "Indonesian",
    "vi": "Vietnamese",
    "el": "Greek",
    "uk": "Ukrainian",
};

// Get language name from language code
export function getLanguageName(code: string): string {
    return languageMap[code] || code;
}

