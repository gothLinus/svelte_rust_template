/// Whether `character` is a bidirectional embedding, override or isolate (U+202A to U+202E,
/// U+2066 to U+2069).
///
/// `char::is_control` misses these, yet they reorder what follows: `invoice\u{202E}fdp.exe`
/// displays as `invoiceexe.pdf`. The marks (U+200E, U+200F, U+061C) stay allowed: right-to-left
/// text needs them and they cannot reverse a run.
pub fn is_bidi_override(character: char) -> bool {
    matches!(character, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}
