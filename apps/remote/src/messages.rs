// SPDX-License-Identifier: GPL-3.0-or-later
//! User-interface texts. Polish only for now; every string goes through [`Msg`] so adding a
//! language later means adding one function, and a missing text is a compile error.

/// Every text the remote shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Msg {
    Live,
    Next,
    Prev,
    NextItem,
    PrevItem,
    Black,
    Logo,
    Connected,
    Disconnected,
    Connecting,
    AlreadyAdvanced,
    NotPossible,
    NoSet,
    Untitled,
    EndOfSet,
    About,
    SearchPlaceholder,
    NoResults,
    ShowNow,
    Add,
    NewSong,
    Edit,
    EditorTitle,
    EditorNumber,
    EditorOrder,
    EditorLyrics,
    EditorHelp,
    Save,
    SaveAndShow,
    Close,
    Saved,
    Conflict,
    TitleMissing,
    LyricsMissing,
    SaveFailed,
    LoadFailed,
}

/// The Polish text for a message.
pub fn pl(msg: Msg) -> &'static str {
    match msg {
        Msg::Live => "Na ekranie",
        Msg::Next => "Dalej ▶",
        Msg::Prev => "◀ Wstecz",
        Msg::NextItem => "Pieśń »",
        Msg::PrevItem => "« Pieśń",
        Msg::Black => "CZERŃ",
        Msg::Logo => "LOGO",
        Msg::Connected => "Połączono",
        Msg::Disconnected => "Brak połączenia – ponawiam…",
        Msg::Connecting => "Łączenie…",
        Msg::AlreadyAdvanced => "Już przełączone przez inny pilot",
        Msg::NotPossible => "Nie da się",
        Msg::NoSet => "Brak zestawu",
        Msg::Untitled => "(bez tytułu)",
        Msg::EndOfSet => "Koniec zestawu",
        Msg::About => "Licencje",
        Msg::SearchPlaceholder => "Szukaj: numer, tytuł lub fragment tekstu",
        Msg::NoResults => "Brak wyników",
        Msg::ShowNow => "Pokaż",
        Msg::Add => "Dodaj",
        Msg::NewSong => "Nowa pieśń",
        Msg::Edit => "Edytuj",
        Msg::EditorTitle => "Tytuł",
        Msg::EditorNumber => "Numer",
        Msg::EditorOrder => "Kolejność (np. V1 C V2 C)",
        Msg::EditorLyrics => "Tekst",
        Msg::EditorHelp => {
            "[V1] zwrotka · [C] refren · [B] most · linia tekstu zaczyna się spacją · || nowy slajd · \
             --- podział, jeśli się nie zmieści · Ctrl+S zapisz · Ctrl+Enter zapisz i pokaż · Esc zamknij"
        }
        Msg::Save => "Zapisz",
        Msg::SaveAndShow => "Zapisz i pokaż",
        Msg::Close => "Zamknij",
        Msg::Saved => "Zapisano",
        Msg::Conflict => "Ktoś zapisał tę pieśń w międzyczasie – otwórz ją ponownie",
        Msg::TitleMissing => "Brak tytułu",
        Msg::LyricsMissing => "Brak tekstu (zacznij od [V1] i linii ze spacją na początku)",
        Msg::SaveFailed => "Nie udało się zapisać",
        Msg::LoadFailed => "Nie udało się wczytać pieśni",
    }
}

/// Human label for a section code sent by the server (`v1`, `c`, `c2`, `b`, …).
pub fn section_label(code: &str) -> String {
    let split = code.find(|c: char| c.is_ascii_digit()).unwrap_or(code.len());
    let (kind, number) = code.split_at(split);
    let with_number =
        |word: &str| if number.is_empty() { word.to_owned() } else { format!("{word} {number}") };
    match kind {
        "v" if !number.is_empty() => number.to_owned(),
        "v" => "Zwr.".to_owned(),
        "c" => with_number("Ref."),
        "b" => with_number("Most"),
        "p" => with_number("Przedref."),
        "e" => with_number("Zak."),
        _ => code.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_codes_become_polish_labels() {
        assert_eq!(section_label("v2"), "2");
        assert_eq!(section_label("c"), "Ref.");
        assert_eq!(section_label("c2"), "Ref. 2");
        assert_eq!(section_label("b"), "Most");
        assert_eq!(section_label("Alleluja"), "Alleluja");
    }
}
