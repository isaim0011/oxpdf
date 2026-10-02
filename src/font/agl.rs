//! Adobe Glyph List (AGL) and algorithmic glyph name resolution.
//!
//! Maps PostScript glyph names to Unicode code points, supporting:
//! - Standard Adobe Glyph List glyph names (e.g. `ampersand`, `fi`, `space`).
//! - Standard 258 Macintosh glyph names from TrueType `post` table.
//! - Algorithmic Unicode names: `uniXXXX` (4-hex digits) and `uXXXX` / `uXXXXXX` (4-6 hex digits).
//! - Glyph name variants with period suffixes (e.g. `ampersand.alt` -> `ampersand`).

pub struct AdobeGlyphList;

impl AdobeGlyphList {
    /// Resolves a PostScript glyph name to a Unicode character.
    pub fn name_to_unicode(name: &str) -> Option<char> {
        if name.is_empty() || name == ".notdef" || name == ".null" {
            return None;
        }

        // 1. Direct table lookup
        if let Some(ch) = lookup_agl(name) {
            return Some(ch);
        }

        // 2. Single ASCII char fallback
        if name.len() == 1 {
            let b = name.as_bytes()[0];
            if b.is_ascii_graphic() {
                return Some(b as char);
            }
        }

        // 3. Algorithmic uniXXXX (BMP)
        if name.starts_with("uni") && name.len() >= 7 {
            let hex_part = &name[3..7];
            if let Ok(code) = u32::from_str_radix(hex_part, 16) {
                if let Some(ch) = char::from_u32(code) {
                    return Some(ch);
                }
            }
        }

        // 4. Algorithmic uXXXX..uXXXXXX (full Unicode range)
        if name.starts_with('u') && name.len() >= 5 && name.len() <= 7 {
            let hex_part = &name[1..];
            if hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
                if let Ok(code) = u32::from_str_radix(hex_part, 16) {
                    if let Some(ch) = char::from_u32(code) {
                        return Some(ch);
                    }
                }
            }
        }

        // 5. Variant suffix: strip at first '.' and retry
        if let Some((base, _)) = name.split_once('.') {
            if !base.is_empty() {
                return Self::name_to_unicode(base);
            }
        }

        None
    }

    /// Alias for `name_to_unicode`.
    #[inline]
    pub fn glyph_name_to_unicode(name: &str) -> Option<char> {
        Self::name_to_unicode(name)
    }
}

/// Binary searches the sorted Adobe Glyph List table.
fn lookup_agl(name: &str) -> Option<char> {
    AGL_TABLE
        .binary_search_by_key(&name, |&(k, _)| k)
        .ok()
        .map(|idx| AGL_TABLE[idx].1)
}

/// Sorted table of standard Adobe Glyph List pairs (name, char).
/// Must remain sorted lexicographically by name for binary search!
static AGL_TABLE: &[(&str, char)] = &[
    ("A", 'A'),
    ("AE", 'Æ'),
    ("Aacute", 'Á'),
    ("Acircumflex", 'Â'),
    ("Adieresis", 'Ä'),
    ("Agrave", 'À'),
    ("Aring", 'Å'),
    ("Atilde", 'Ã'),
    ("B", 'B'),
    ("C", 'C'),
    ("Cacute", 'Ć'),
    ("Ccaron", 'Č'),
    ("Ccedilla", 'Ç'),
    ("D", 'D'),
    ("Delta", 'Δ'),
    ("E", 'E'),
    ("Eacute", 'É'),
    ("Ecircumflex", 'Ê'),
    ("Edieresis", 'Ë'),
    ("Egrave", 'È'),
    ("Eth", 'Ð'),
    ("Euro", '€'),
    ("F", 'F'),
    ("G", 'G'),
    ("Gbreve", 'Ğ'),
    ("H", 'H'),
    ("I", 'I'),
    ("Iacute", 'Í'),
    ("Icircumflex", 'Î'),
    ("Idieresis", 'Ï'),
    ("Idotaccent", 'İ'),
    ("Igrave", 'Ì'),
    ("J", 'J'),
    ("K", 'K'),
    ("L", 'L'),
    ("Lslash", 'Ł'),
    ("M", 'M'),
    ("N", 'N'),
    ("Ntilde", 'Ñ'),
    ("O", 'O'),
    ("OE", 'Œ'),
    ("Oacute", 'Ó'),
    ("Ocircumflex", 'Ô'),
    ("Odieresis", 'Ö'),
    ("Ograve", 'Ò'),
    ("Omega", 'Ω'),
    ("Oslash", 'Ø'),
    ("Otilde", 'Õ'),
    ("P", 'P'),
    ("Q", 'Q'),
    ("R", 'R'),
    ("S", 'S'),
    ("Scaron", 'Š'),
    ("Scedilla", 'Ş'),
    ("T", 'T'),
    ("Thorn", 'Þ'),
    ("U", 'U'),
    ("Uacute", 'Ú'),
    ("Ucircumflex", 'Û'),
    ("Udieresis", 'Ü'),
    ("Ugrave", 'Ù'),
    ("V", 'V'),
    ("W", 'W'),
    ("X", 'X'),
    ("Y", 'Y'),
    ("Yacute", 'Ý'),
    ("Ydieresis", 'Ÿ'),
    ("Z", 'Z'),
    ("Zcaron", 'Ž'),
    ("a", 'a'),
    ("aacute", 'á'),
    ("acircumflex", 'â'),
    ("acute", '´'),
    ("adieresis", 'ä'),
    ("ae", 'æ'),
    ("agrave", 'à'),
    ("ampersand", '&'),
    ("approxequal", '≈'),
    ("aring", 'å'),
    ("asciicircum", '^'),
    ("asciitilde", '~'),
    ("asterisk", '*'),
    ("at", '@'),
    ("atilde", 'ã'),
    ("b", 'b'),
    ("backslash", '\\'),
    ("bar", '|'),
    ("braceleft", '{'),
    ("braceright", '}'),
    ("bracketleft", '['),
    ("bracketright", ']'),
    ("breve", '˘'),
    ("brokenbar", '¦'),
    ("bullet", '•'),
    ("c", 'c'),
    ("cacute", 'ć'),
    ("caron", 'ˇ'),
    ("ccaron", 'č'),
    ("ccedilla", 'ç'),
    ("cedilla", '¸'),
    ("cent", '¢'),
    ("circumflex", '^'),
    ("colon", ':'),
    ("comma", ','),
    ("copyright", '©'),
    ("currency", '¤'),
    ("d", 'd'),
    ("dagger", '†'),
    ("daggerdbl", '‡'),
    ("dcroat", 'đ'),
    ("degree", '°'),
    ("dieresis", '¨'),
    ("divide", '÷'),
    ("dollar", '$'),
    ("dotaccent", '˙'),
    ("dotlessi", 'ı'),
    ("e", 'e'),
    ("eacute", 'é'),
    ("ecircumflex", 'ê'),
    ("edieresis", 'ë'),
    ("egrave", 'è'),
    ("eight", '8'),
    ("ellipsis", '…'),
    ("emdash", '—'),
    ("endash", '–'),
    ("equal", '='),
    ("eth", 'ð'),
    ("euro", '€'),
    ("exclam", '!'),
    ("exclamdown", '¡'),
    ("f", 'f'),
    ("ff", 'ﬀ'),
    ("ffi", 'ﬃ'),
    ("ffl", 'ﬄ'),
    ("fi", 'ﬁ'),
    ("five", '5'),
    ("fl", 'ﬂ'),
    ("florin", 'ƒ'),
    ("four", '4'),
    ("fraction", '⁄'),
    ("franc", '₣'),
    ("g", 'g'),
    ("gbreve", 'ğ'),
    ("germandbls", 'ß'),
    ("grave", '`'),
    ("greater", '>'),
    ("greaterequal", '≥'),
    ("guillemotleft", '«'),
    ("guillemotright", '»'),
    ("guilsinglleft", '‹'),
    ("guilsinglright", '›'),
    ("h", 'h'),
    ("hungarumlaut", '˝'),
    ("hyphen", '-'),
    ("i", 'i'),
    ("iacute", 'í'),
    ("icircumflex", 'î'),
    ("idieresis", 'ï'),
    ("idotaccent", 'i'),
    ("igrave", 'ì'),
    ("infinity", '∞'),
    ("integral", '∫'),
    ("j", 'j'),
    ("k", 'k'),
    ("l", 'l'),
    ("less", '<'),
    ("lessequal", '≤'),
    ("logicalnot", '¬'),
    ("lozenge", '◊'),
    ("lslash", 'ł'),
    ("m", 'm'),
    ("macron", '¯'),
    ("minus", '-'),
    ("mu", 'µ'),
    ("multiply", '×'),
    ("n", 'n'),
    ("nine", '9'),
    ("nonbreakingspace", '\u{00A0}'),
    ("nonmarkingreturn", '\n'),
    ("notequal", '≠'),
    ("ntilde", 'ñ'),
    ("numbersign", '#'),
    ("o", 'o'),
    ("oacute", 'ó'),
    ("ocircumflex", 'ô'),
    ("odieresis", 'ö'),
    ("oe", 'œ'),
    ("ogonek", '˛'),
    ("ograve", 'ò'),
    ("one", '1'),
    ("onehalf", '½'),
    ("onequarter", '¼'),
    ("onesuperior", '¹'),
    ("ordfeminine", 'ª'),
    ("ordmasculine", 'º'),
    ("oslash", 'ø'),
    ("otilde", 'õ'),
    ("p", 'p'),
    ("paragraph", '¶'),
    ("parenleft", '('),
    ("parenright", ')'),
    ("partialdiff", '∂'),
    ("percent", '%'),
    ("period", '.'),
    ("periodcentered", '·'),
    ("perthousand", '‰'),
    ("pi", 'π'),
    ("plus", '+'),
    ("plusminus", '±'),
    ("product", '∏'),
    ("q", 'q'),
    ("question", '?'),
    ("questiondown", '¿'),
    ("quotedbl", '"'),
    ("quotedblbase", '„'),
    ("quotedblleft", '“'),
    ("quotedblright", '”'),
    ("quoteleft", '‘'),
    ("quoteright", '’'),
    ("quotesinglbase", '‚'),
    ("quotesingle", '\''),
    ("r", 'r'),
    ("radical", '√'),
    ("registered", '®'),
    ("ring", '˚'),
    ("s", 's'),
    ("scaron", 'š'),
    ("scedilla", 'ş'),
    ("section", '§'),
    ("semicolon", ';'),
    ("seven", '7'),
    ("six", '6'),
    ("slash", '/'),
    ("space", ' '),
    ("sterling", '£'),
    ("summation", '∑'),
    ("t", 't'),
    ("thorn", 'þ'),
    ("three", '3'),
    ("threequarters", '¾'),
    ("threesuperior", '³'),
    ("tilde", '~'),
    ("trademark", '™'),
    ("two", '2'),
    ("twosuperior", '²'),
    ("u", 'u'),
    ("uacute", 'ú'),
    ("ucircumflex", 'û'),
    ("udieresis", 'ü'),
    ("ugrave", 'ù'),
    ("underscore", '_'),
    ("v", 'v'),
    ("w", 'w'),
    ("x", 'x'),
    ("y", 'y'),
    ("yacute", 'ý'),
    ("ydieresis", 'ÿ'),
    ("yen", '¥'),
    ("z", 'z'),
    ("zcaron", 'ž'),
    ("zero", '0'),
];

/// The 258 standard Macintosh glyph names in the TrueType `post` table.
pub static MAC_POST_NAMES: [&str; 258] = [
    ".notdef",
    ".null",
    "nonmarkingreturn",
    "space",
    "exclam",
    "quotedbl",
    "numbersign",
    "dollar",
    "percent",
    "ampersand",
    "quotesingle",
    "parenleft",
    "parenright",
    "asterisk",
    "plus",
    "comma",
    "hyphen",
    "period",
    "slash",
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "colon",
    "semicolon",
    "less",
    "equal",
    "greater",
    "question",
    "at",
    "A",
    "B",
    "C",
    "D",
    "E",
    "F",
    "G",
    "H",
    "I",
    "J",
    "K",
    "L",
    "M",
    "N",
    "O",
    "P",
    "Q",
    "R",
    "S",
    "T",
    "U",
    "V",
    "W",
    "X",
    "Y",
    "Z",
    "bracketleft",
    "backslash",
    "bracketright",
    "asciicircum",
    "underscore",
    "grave",
    "a",
    "b",
    "c",
    "d",
    "e",
    "f",
    "g",
    "h",
    "i",
    "j",
    "k",
    "l",
    "m",
    "n",
    "o",
    "p",
    "q",
    "r",
    "s",
    "t",
    "u",
    "v",
    "w",
    "x",
    "y",
    "z",
    "braceleft",
    "bar",
    "braceright",
    "asciitilde",
    "Adieresis",
    "Aring",
    "Ccedilla",
    "Eacute",
    "Ntilde",
    "Odieresis",
    "Udieresis",
    "aacute",
    "agrave",
    "acircumflex",
    "adieresis",
    "atilde",
    "aring",
    "ccedilla",
    "eacute",
    "egrave",
    "ecircumflex",
    "edieresis",
    "iacute",
    "igrave",
    "icircumflex",
    "idieresis",
    "ntilde",
    "oacute",
    "ograve",
    "ocircumflex",
    "odieresis",
    "otilde",
    "uacute",
    "ugrave",
    "ucircumflex",
    "udieresis",
    "dagger",
    "degree",
    "cent",
    "sterling",
    "section",
    "bullet",
    "paragraph",
    "germandbls",
    "registered",
    "copyright",
    "trademark",
    "acute",
    "dieresis",
    "notequal",
    "AE",
    "Oslash",
    "infinity",
    "plusminus",
    "lessequal",
    "greaterequal",
    "yen",
    "mu",
    "partialdiff",
    "summation",
    "product",
    "pi",
    "integral",
    "ordfeminine",
    "ordmasculine",
    "Omega",
    "ae",
    "oslash",
    "questiondown",
    "exclamdown",
    "logicalnot",
    "radical",
    "florin",
    "approxequal",
    "Delta",
    "guillemotleft",
    "guillemotright",
    "ellipsis",
    "nonbreakingspace",
    "Agrave",
    "Atilde",
    "Otilde",
    "OE",
    "oe",
    "endash",
    "emdash",
    "quotedblleft",
    "quotedblright",
    "quoteleft",
    "quoteright",
    "divide",
    "lozenge",
    "ydieresis",
    "Ydieresis",
    "fraction",
    "currency",
    "guilsinglleft",
    "guilsinglright",
    "fi",
    "fl",
    "daggerdbl",
    "periodcentered",
    "quotesinglbase",
    "quotedblbase",
    "perthousand",
    "Acircumflex",
    "Ecircumflex",
    "Aacute",
    "Edieresis",
    "Egrave",
    "Iacute",
    "Icircumflex",
    "Idieresis",
    "Igrave",
    "Oacute",
    "Ocircumflex",
    "apple",
    "Ograve",
    "Uacute",
    "Ucircumflex",
    "Ugrave",
    "dotlessi",
    "circumflex",
    "tilde",
    "macron",
    "breve",
    "dotaccent",
    "ring",
    "cedilla",
    "hungarumlaut",
    "ogonek",
    "caron",
    "Lslash",
    "lslash",
    "Scaron",
    "scaron",
    "Zcaron",
    "zcaron",
    "brokenbar",
    "Eth",
    "eth",
    "Yacute",
    "yacute",
    "Thorn",
    "thorn",
    "minus",
    "multiply",
    "onesuperior",
    "twosuperior",
    "threesuperior",
    "onehalf",
    "onequarter",
    "threequarters",
    "franc",
    "Gbreve",
    "gbreve",
    "Idotaccent",
    "Scedilla",
    "scedilla",
    "Cacute",
    "cacute",
    "Ccaron",
    "ccaron",
    "dcroat",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agl_standard_lookups() {
        assert_eq!(AdobeGlyphList::name_to_unicode("ampersand"), Some('&'));
        assert_eq!(AdobeGlyphList::name_to_unicode("fi"), Some('ﬁ'));
        assert_eq!(AdobeGlyphList::name_to_unicode("fl"), Some('ﬂ'));
        assert_eq!(AdobeGlyphList::name_to_unicode("space"), Some(' '));
        assert_eq!(AdobeGlyphList::name_to_unicode("Euro"), Some('€'));
        assert_eq!(AdobeGlyphList::name_to_unicode("germandbls"), Some('ß'));
        assert_eq!(AdobeGlyphList::name_to_unicode("copyright"), Some('©'));
    }

    #[test]
    fn test_agl_algorithmic_uni_and_u() {
        assert_eq!(AdobeGlyphList::name_to_unicode("uni0041"), Some('A'));
        assert_eq!(AdobeGlyphList::name_to_unicode("uni0020"), Some(' '));
        assert_eq!(AdobeGlyphList::name_to_unicode("uni03C0"), Some('π'));
        assert_eq!(AdobeGlyphList::name_to_unicode("u0041"), Some('A'));
        assert_eq!(AdobeGlyphList::name_to_unicode("u0024"), Some('$'));
        assert_eq!(AdobeGlyphList::name_to_unicode("u1F600"), Some('😀'));
    }

    #[test]
    fn test_agl_variant_suffixes() {
        assert_eq!(AdobeGlyphList::name_to_unicode("ampersand.alt"), Some('&'));
        assert_eq!(AdobeGlyphList::name_to_unicode("fi.swash"), Some('ﬁ'));
        assert_eq!(AdobeGlyphList::name_to_unicode("A.sc"), Some('A'));
        assert_eq!(AdobeGlyphList::name_to_unicode("uni0041.sc"), Some('A'));
    }

    #[test]
    fn test_agl_mac_names() {
        for &name in &MAC_POST_NAMES {
            if name != ".notdef" && name != ".null" && name != "apple" {
                assert!(
                    AdobeGlyphList::name_to_unicode(name).is_some(),
                    "Missing AGL entry for Mac post name: {}",
                    name
                );
            }
        }
    }
}
