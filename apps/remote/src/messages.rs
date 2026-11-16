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
    ImportOpenSong,
    Importing,
    ImportFailed,
    ExportLibrary,
    MergeLibrary,
    MergeChecking,
    MergeWouldDo,
    MergeConfirm,
    Merging,
    MergeDone,
    MergeFailed,
    Calibrate,
    CalibrationTitle,
    CalibrationHelp,
    Zone,
    AddZone,
    DrawThisZone,
    Width,
    Height,
    MaskZone,
    RemoveZone,
    SafeArea,
    MarginTop,
    MarginBottom,
    MarginLeft,
    MarginRight,
    InvalidProfile,
    AdminMode,
    AdminPin,
    Unlock,
    EndAdmin,
    Devices,
    ThisDevice,
    Revoke,
    AdminRequired,
    Picture,
    PictureFailed,
    Pause,
    DisplayOn,
    DisplayStandby,
    SlideOnScreen,
    ProjectorCooling,
    DisplayUnknown,
    DisplayIsOn,
    DisplayIsStandby,
    DisplayWarming,
    DisplayCooling,
    DisplayNoAnswer,
    DisplayBusy,
    DisplayAuthFailed,
    EditSet,
    DoneEditing,
    Remove,
    Undo,
    NewSet,
    NewSetPrompt,
    Announcement,
    AnnouncementText,
    Cancel,
    PairingTitle,
    Pin,
    DeviceName,
    DefaultDeviceName,
    Pair,
    WrongPin,
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
        Msg::ImportOpenSong => "Import z OpenSong (ZIP)",
        Msg::Importing => "Importowanie…",
        Msg::EditSet => "Edytuj zestaw",
        Msg::DoneEditing => "Gotowe",
        Msg::Remove => "Usuń (przytrzymaj)",
        Msg::Undo => "Cofnij usunięcie",
        Msg::NewSet => "Nowy zestaw",
        Msg::NewSetPrompt => "Nazwa nowego zestawu",
        Msg::Announcement => "Ogłoszenie",
        Msg::AnnouncementText => "Treść (pusta linia zaczyna nowy slajd)",
        Msg::Cancel => "Anuluj",
        Msg::PairingTitle => "Połącz pilota z ekranem",
        Msg::Pin => "PIN",
        Msg::DeviceName => "Nazwa tego urządzenia",
        Msg::DefaultDeviceName => "Pilot",
        Msg::Pair => "Połącz",
        Msg::WrongPin => "Błędny PIN",
        Msg::ImportFailed => "Import nie powiódł się – czy to plik ZIP z folderem OpenSong?",
        Msg::ExportLibrary => "Eksport bazy",
        Msg::MergeLibrary => "Scal bazę z pliku",
        Msg::MergeChecking => "Sprawdzanie pliku…",
        Msg::MergeWouldDo => "Po scaleniu:",
        Msg::MergeConfirm => "Scal",
        Msg::Merging => "Scalanie…",
        Msg::MergeDone => "Scalono:",
        Msg::MergeFailed => "Scalanie nie powiodło się – czy to plik .sursum?",
        Msg::Calibrate => "Kalibracja ekranu (przytrzymaj)",
        Msg::CalibrationTitle => "Kalibracja ekranu",
        Msg::CalibrationHelp => {
            "Tylko poza mszą: na ekranie jest teraz siatka. Przeciągnij palcem po podglądzie, by              zaznaczyć słup (czerwony). Zielona ramka to miejsce na tekst, żółta – margines              bezpieczny. Zmiany zapisują się od razu."
        }
        Msg::Zone => "Strefa",
        Msg::AddZone => "Dodaj strefę",
        Msg::DrawThisZone => "Rysuj tę strefę",
        Msg::Width => "szer.",
        Msg::Height => "wys.",
        Msg::MaskZone => "zaciemniaj",
        Msg::RemoveZone => "Usuń",
        Msg::SafeArea => "Margines bezpieczny",
        Msg::MarginTop => "góra",
        Msg::MarginBottom => "dół",
        Msg::MarginLeft => "lewo",
        Msg::MarginRight => "prawo",
        Msg::InvalidProfile => "Ta strefa zasłoniłaby cały ekran – zmiana nie została zapisana",
        Msg::AdminMode => "Tryb administratora",
        Msg::AdminPin => "PIN administratora",
        Msg::Unlock => "Odblokuj",
        Msg::EndAdmin => "Zakończ tryb administratora",
        Msg::Devices => "Urządzenia",
        Msg::ThisDevice => "to urządzenie",
        Msg::Revoke => "Odłącz (przytrzymaj)",
        Msg::AdminRequired => "To wymaga trybu administratora",
        Msg::Picture => "Obraz",
        Msg::PictureFailed => "Nie udało się dodać obrazu (PNG, JPEG lub WebP, do 25 MB)",
        Msg::Pause => "Przerwa",
        Msg::DisplayOn => "Włącz ekran",
        Msg::DisplayStandby => "Uśpij ekran (przytrzymaj)",
        Msg::SlideOnScreen => "Uwaga: na ekranie jest slajd",
        Msg::ProjectorCooling => "Projektor stygnie, spróbuj za chwilę",
        Msg::DisplayUnknown => "Ekran: …",
        Msg::DisplayIsOn => "Ekran: włączony",
        Msg::DisplayIsStandby => "Ekran: uśpiony",
        Msg::DisplayWarming => "Ekran: rozgrzewa się",
        Msg::DisplayCooling => "Ekran: stygnie",
        Msg::DisplayNoAnswer => "Ekran: brak odpowiedzi",
        Msg::DisplayBusy => "Ekran jest zajęty – spróbuj za chwilę",
        Msg::DisplayAuthFailed => "Ekran odrzucił hasło – sprawdź config.toml",
    }
}

/// What to tell the operator when the server refuses a command.
pub fn rejection(code: sursum_protocol::ErrorCode) -> Msg {
    use sursum_protocol::ErrorCode;
    match code {
        ErrorCode::InvalidProfile => Msg::InvalidProfile,
        ErrorCode::Forbidden => Msg::AdminRequired,
        ErrorCode::DisplayBusy => Msg::DisplayBusy,
        ErrorCode::DisplayAuthFailed => Msg::DisplayAuthFailed,
        ErrorCode::DisplayUnreachable => Msg::DisplayNoAnswer,
        ErrorCode::NotFound | ErrorCode::NoActiveSet | ErrorCode::Unsupported => Msg::NotPossible,
    }
}

/// One-line import report, e.g. "Zaimportowano: 12 nowych, 3 zmienionych, …".
pub fn import_summary(
    inserted: u32,
    updated: u32,
    unchanged: u32,
    failed: u32,
    sets: u32,
    problems: usize,
) -> String {
    format!(
        "Zaimportowano: {inserted} nowych, {updated} zmienionych, {unchanged} bez zmian, {failed} błędów, {sets} zestawów, {problems} uwag"
    )
}

/// One-line merge report; conflicting titles are listed because they need a human decision.
pub fn merge_summary(
    added: u32,
    updated: u32,
    unchanged: u32,
    kept_local: u32,
    conflicts: &[String],
) -> String {
    let mut text = format!(
        "{added} nowych, {updated} zaktualizowanych, {unchanged} bez zmian, {kept_local} nowszych tutaj"
    );
    if !conflicts.is_empty() {
        text.push_str(&format!(
            ", {} konfliktów (zapisane jako kopie): {}",
            conflicts.len(),
            conflicts.join(", ")
        ));
    }
    text
}

/// "Za dużo prób – spróbuj ponownie za 30 s".
pub fn locked_for(seconds: u32) -> String {
    format!("Za dużo prób – spróbuj ponownie za {seconds} s")
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
