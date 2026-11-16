// SPDX-License-Identifier: GPL-3.0-or-later
//! User-interface texts. Polish only for now; every string goes through [`Msg`] so adding a
//! language later means adding one function, and a missing text is a compile error.

/// Every text the remote shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Msg {
    TabLive,
    AdminOn,
    NeedsAdmin,
    SectionDisplay,
    SectionDevice,
    SectionLibrary,
    SectionAbout,
    EndOfItemLabel,
    EndGap,
    EndContinue,
    Fullscreen,
    ExitFullscreen,
    SleepTip,
    CalibrateStart,
    CalibrateHint,
    Version,
    LicenseNote,
    ImportDone,
    NoSetHint,
    ChangeSet,
    Rename,
    SwitchSetHint,
    Chosen,
    Delete,
    SetName,
    SetEmptyHint,
    MoveUp,
    MoveDown,
    CloseSearch,
    InSet,
    AddShort,
    SongCard,
    ShowNowOnScreen,
    AddToSetLong,
    ShortText,
    ShortPicture,
    ShortGap,
    ShortBlack,
    ShortLogo,
    ShortNothing,
    TabSet,
    TabSettings,
    ShowsText,
    ShowsBlack,
    ShowsLogo,
    ShowsGap,
    ShowsPicture,
    ShowsNothing,
    ScreenSheet,
    BlackOut,
    ShowText,
    ShowLogo,
    LogoOnScreen,
    Search,
    PrevSong,
    NextSong,
    NothingLive,
    GapAfterItem,
    SlideOf,
    NextGap,
    StartSet,
    UnderCover,
    LiveBadge,
    NextBadge,
    SetEmpty,
    ChooseSet,
    AddSongs,
    ShowNowLong,
    AddToSet,
    Added,
    Numbers,
    SearchSong,
    DisplayAsleep,
    TurnOn,
    NextSlide,
    ButtonsSide,
    ButtonsRight,
    ButtonsLeft,
    Vibration,
    SlideN,
    Items,
    Next,
    Prev,
    NextItem,
    PrevItem,
    Black,
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
    Keys,
    KeysHelp,
    KeysReset,
    KeysNone,
    KeysLearn,
    KeysPressNow,
    ParishLogo,
    ScreenMissing,
    DeleteSong,
    DeletedSong,
    KindText,
    KindImage,
    KindPause,
    SetOnScreen,
    UndoShort,
    Create,
    AdminHint,
    HoldToConfirm,
    ShowPin,
    HidePin,
    NotSent,
    Remove,
    Undo,
    NewSet,
    Announcement,
    AnnouncementText,
    Cancel,
    PairingTitle,
    Pin,
    DeviceName,
    DefaultDeviceName,
    Pair,
    WrongPin,
    SetupTitle,
    SetupHint,
    RepeatPin,
    SetPin,
    PinsDiffer,
    InvalidPin,
    PinAlreadySet,
}

/// The Polish text for a message.
pub fn pl(msg: Msg) -> &'static str {
    match msg {
        Msg::TabLive => "Msza",
        Msg::AdminOn => "Tryb administratora jest włączony (wyłączy się sam po 15 minutach)",
        Msg::NeedsAdmin => "wymaga trybu administratora",
        Msg::SectionDisplay => "Wyświetlanie",
        Msg::SectionDevice => "To urządzenie",
        Msg::SectionLibrary => "Baza pieśni",
        Msg::SectionAbout => "O programie",
        Msg::EndOfItemLabel => "Po ostatnim slajdzie pieśni „Dalej”…",
        Msg::EndGap => "najpierw wygasza ekran (pusty slajd), potem następna pozycja",
        Msg::EndContinue => "od razu pokazuje następną pozycję",
        Msg::Fullscreen => "Pełny ekran",
        Msg::ExitFullscreen => "Zamknij pełny ekran",
        Msg::SleepTip => {
            "Tablet nie może zgasnąć w trakcie mszy: w ustawieniach Androida wydłuż czas wygaszania ekranu (albo włącz „Pozostaw ekran włączony” w opcjach programisty)."
        }
        Msg::CalibrateStart => "Rozpocznij kalibrację",
        Msg::CalibrateHint => {
            "Tylko poza mszą: na ekranie w kościele pojawi się siatka kalibracyjna zamiast slajdu."
        }
        Msg::Version => "wersja",
        Msg::LicenseNote => {
            "Licencja GPL-3.0-or-later, bez żadnej gwarancji. Kod źródłowy: github.com/dpieczynski/sursum."
        }
        Msg::ImportDone => "Zaimportowano",
        Msg::NoSetHint => "Nie ma jeszcze zestawu. Utwórz go przyciskiem „Zmień zestaw” → „Nowy zestaw”.",
        Msg::ChangeSet => "Zmień zestaw",
        Msg::Rename => "Zmień nazwę",
        Msg::SwitchSetHint => "Ekran się nie zmieni, ale „Dalej” przejdzie do pozycji nowego zestawu.",
        Msg::Chosen => "wybrany",
        Msg::Delete => "Usuń",
        Msg::SetName => "Nazwa zestawu",
        Msg::SetEmptyHint => {
            "Zestaw jest pusty. Wyszukaj pieśń powyżej albo dodaj ogłoszenie, obraz lub przerwę."
        }
        Msg::MoveUp => "Przesuń w górę",
        Msg::MoveDown => "Przesuń w dół",
        Msg::CloseSearch => "Zamknij",
        Msg::InSet => "w zestawie",
        Msg::AddShort => "+ Dodaj",
        Msg::SongCard => "Pieśń",
        Msg::ShowNowOnScreen => "Pokaż teraz na ekranie",
        Msg::AddToSetLong => "Dodaj do zestawu",
        Msg::ShortText => "Tekst",
        Msg::ShortPicture => "Obraz",
        Msg::ShortGap => "Przerwa",
        Msg::ShortBlack => "Wygaszony",
        Msg::ShortLogo => "Logo",
        Msg::ShortNothing => "Pusty",
        Msg::TabSet => "Zestaw",
        Msg::TabSettings => "Ustawienia",
        Msg::ShowsText => "Na ekranie: tekst",
        Msg::ShowsBlack => "Ekran wygaszony",
        Msg::ShowsLogo => "Na ekranie: logo",
        Msg::ShowsGap => "Przerwa – pusty ekran",
        Msg::ShowsPicture => "Na ekranie: obraz",
        Msg::ShowsNothing => "Nic nie jest wyświetlane",
        Msg::ScreenSheet => "Ekran",
        Msg::BlackOut => "Wygaś ekran",
        Msg::ShowText => "Pokaż tekst",
        Msg::ShowLogo => "Pokaż logo",
        Msg::LogoOnScreen => "Logo na ekranie",
        Msg::Search => "Szukaj",
        Msg::PrevSong => "Poprzednia pieśń",
        Msg::NextSong => "Następna pieśń",
        Msg::NothingLive => "Nic nie jest wyświetlane. Dotknij pozycji w zestawie albo naciśnij Dalej.",
        Msg::GapAfterItem => "Przerwa po pozycji – pusty ekran",
        Msg::SlideOf => "slajd",
        Msg::NextGap => "→ przerwa",
        Msg::StartSet => "→ początek zestawu",
        Msg::UnderCover => "pod zasłoną",
        Msg::LiveBadge => "na ekranie",
        Msg::NextBadge => "następna",
        Msg::SetEmpty => "Zestaw jest pusty",
        Msg::ChooseSet => "Wybierz zestaw",
        Msg::AddSongs => "Dodaj pieśni",
        Msg::ShowNowLong => "Pokaż teraz",
        Msg::AddToSet => "+ Do zestawu",
        Msg::Added => "Dodano",
        Msg::Numbers => "123",
        Msg::SearchSong => "Szukaj pieśni",
        Msg::DisplayAsleep => "Ekran jest uśpiony",
        Msg::TurnOn => "Włącz",
        Msg::NextSlide => "Następny slajd",
        Msg::ButtonsSide => "Przyciski sterujące",
        Msg::ButtonsRight => "Po prawej",
        Msg::ButtonsLeft => "Po lewej",
        Msg::Vibration => "Wibracja przy dotknięciu",
        Msg::SlideN => "Slajd",
        Msg::Items => "poz.",
        Msg::Next => "Dalej",
        Msg::Prev => "Wstecz",
        Msg::NextItem => "Następna pieśń",
        Msg::PrevItem => "Poprzednia pieśń",
        Msg::Black => "Wygaś / pokaż ekran",
        Msg::Disconnected => "Brak połączenia – ekran pokazuje ostatni slajd. Ponawiam…",
        Msg::Connecting => "Łączenie…",
        Msg::AlreadyAdvanced => "Już przełączone przez inny pilot",
        Msg::NotPossible => "Nie da się",
        Msg::NoSet => "Nie wybrano zestawu",
        Msg::Untitled => "(bez tytułu)",
        Msg::EndOfSet => "Koniec zestawu",
        Msg::About => "Licencje",
        Msg::SearchPlaceholder => "Szukaj: numer, tytuł lub fragment tekstu",
        Msg::NoResults => "Brak wyników",
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
        Msg::Remove => "Usuń",
        Msg::Undo => "Cofnij usunięcie",
        Msg::NewSet => "Nowy zestaw",
        Msg::Announcement => "Ogłoszenie",
        Msg::AnnouncementText => "Treść (pusta linia zaczyna nowy slajd)",
        Msg::Cancel => "Anuluj",
        Msg::PairingTitle => "Połącz pilota z ekranem",
        Msg::Pin => "PIN",
        Msg::DeviceName => "Nazwa tego urządzenia",
        Msg::DefaultDeviceName => "Pilot",
        Msg::Pair => "Połącz",
        Msg::WrongPin => "Błędny PIN",
        Msg::SetupTitle => "Pierwsze uruchomienie: ustaw PIN",
        Msg::SetupHint => {
            "PIN (6–12 cyfr) chroni sterowanie ekranem. Będzie potrzebny przy łączeniu \
                           kolejnych pilotów i w trybie administratora – zapisz go w bezpiecznym miejscu."
        }
        Msg::RepeatPin => "Powtórz PIN",
        Msg::SetPin => "Ustaw PIN",
        Msg::PinsDiffer => "Wpisane PIN-y się różnią",
        Msg::InvalidPin => "PIN musi mieć od 6 do 12 cyfr",
        Msg::PinAlreadySet => "PIN został już ustawiony na innym urządzeniu – połącz się, wpisując go",
        Msg::ImportFailed => "Import nie powiódł się – czy to plik ZIP z folderem OpenSong?",
        Msg::ExportLibrary => "Eksport bazy",
        Msg::MergeLibrary => "Scal bazę z pliku",
        Msg::MergeChecking => "Sprawdzanie pliku…",
        Msg::MergeWouldDo => "Sprawdzono plik. Po scaleniu:",
        Msg::MergeConfirm => "Scal",
        Msg::Merging => "Scalanie…",
        Msg::MergeDone => "Scalono.",
        Msg::MergeFailed => "Scalanie nie powiodło się – czy to plik .sursum?",
        Msg::Calibrate => "Kalibracja ekranu",
        Msg::CalibrationTitle => "Kalibracja ekranu",
        Msg::CalibrationHelp => {
            "Tylko poza mszą: na ekranie jest teraz siatka. Przeciągnij palcem po podglądzie, by \
             zaznaczyć strefę, w której nie może być tekstu (czerwona). Zielona ramka to miejsce na \
             tekst, żółta – margines bezpieczny. Zmiany zapisują się od razu."
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
        Msg::Revoke => "Odłącz",
        Msg::AdminRequired => "To wymaga trybu administratora",
        Msg::Picture => "Obraz",
        Msg::PictureFailed => "Nie udało się dodać obrazu (PNG, JPEG lub WebP, do 25 MB)",
        Msg::Pause => "Przerwa",
        Msg::DisplayOn => "Włącz ekran",
        Msg::DisplayStandby => "Uśpij ekran",
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
        Msg::Keys => "Pedał i klawisze",
        Msg::KeysHelp => {
            "Pedał lub pilot do prezentacji działa jak klawiatura. Kliknij „Naucz” i naciśnij \
             przycisk pedału, aby przypisać go do akcji. Ustawienia zapamiętuje to urządzenie."
        }
        Msg::KeysReset => "Przywróć domyślne",
        Msg::KeysNone => "brak",
        Msg::KeysLearn => "Naucz",
        Msg::KeysPressNow => "Naciśnij teraz…",
        Msg::ParishLogo => "Logo parafii",
        Msg::ScreenMissing => "Ekran nie jest połączony z sursum",
        Msg::DeleteSong => "Usuń pieśń z bazy",
        Msg::DeletedSong => "(pieśń usunięta z bazy)",
        Msg::KindText => "tekst",
        Msg::KindImage => "obraz",
        Msg::KindPause => "przerwa",
        Msg::SetOnScreen => "Tego zestawu nie można usunąć – jego pozycja jest na ekranie",
        Msg::UndoShort => "Cofnij",
        Msg::Create => "Utwórz",
        Msg::AdminHint => "Wpisz PIN jeszcze raz. Tryb administratora wyłączy się sam po 15 minutach.",
        Msg::HoldToConfirm => "Przytrzymaj przycisk, aby potwierdzić",
        Msg::ShowPin => "Pokaż",
        Msg::HidePin => "Ukryj",
        Msg::NotSent => "Nie wysłano – brak połączenia z sursum",
    }
}

/// What to tell the operator when the server refuses a command.
pub fn rejection(code: sursum_protocol::ErrorCode) -> Msg {
    use sursum_protocol::ErrorCode;
    match code {
        ErrorCode::InvalidProfile => Msg::InvalidProfile,
        ErrorCode::SetOnScreen => Msg::SetOnScreen,
        ErrorCode::Forbidden => Msg::AdminRequired,
        ErrorCode::DisplayBusy => Msg::DisplayBusy,
        ErrorCode::DisplayAuthFailed => Msg::DisplayAuthFailed,
        ErrorCode::DisplayUnreachable => Msg::DisplayNoAnswer,
        ErrorCode::NotFound | ErrorCode::NoActiveSet | ErrorCode::Unsupported => Msg::NotPossible,
    }
}

/// The import report, one line per number, then the problems found (file and line).
pub fn import_lines(report: &sursum_protocol::ImportReport) -> Vec<String> {
    let mut lines = vec![
        format!("Nowe pieśni: {}", report.inserted),
        format!("Zmienione: {}", report.updated),
        format!("Bez zmian: {}", report.unchanged),
        format!("Zestawy: {}", report.sets),
    ];
    if report.failed > 0 {
        lines.push(format!("Nie udało się wczytać: {}", report.failed));
    }
    lines.extend(report.problems.iter().map(|problem| format!("Uwaga: {problem}")));
    lines
}

/// The merge report, one line per number; conflicting titles are listed because they need a
/// human decision.
pub fn merge_lines(report: &sursum_protocol::MergeReport) -> Vec<String> {
    let mut lines = vec![
        format!("Nowe pieśni: {}", report.added),
        format!("Zaktualizowane: {}", report.updated),
        format!("Usunięte: {}", report.deleted),
        format!("Bez zmian: {}", report.unchanged),
        format!("Nowsze tutaj (zostają): {}", report.kept_local),
    ];
    if !report.conflicts.is_empty() {
        lines.push(format!(
            "Zmienione w obu kościołach (zapisane jako kopie): {}",
            report.conflicts.join(", ")
        ));
    }
    lines
}

/// "Nowa pieśń: „Barka”" (from a search without results).
pub fn new_song_named(title: &str) -> String {
    if title.is_empty() {
        pl(Msg::NewSong).to_owned()
    } else {
        format!("{}: „{title}”", pl(Msg::NewSong))
    }
}

/// "Usunięto: Barka" (with an undo button in the toast).
pub fn removed(title: &str) -> String {
    format!("Usunięto: {title}")
}

/// "Za dużo prób – spróbuj ponownie za 30 s".
pub fn locked_for(seconds: u32) -> String {
    format!("Za dużo prób – spróbuj ponownie za {seconds} s")
}

/// Full section name for headings: `v1` → "Zwrotka 1", `c` → "Refren", `c2` → "Refren 2".
pub fn section_title(code: &str) -> String {
    let split = code.find(|c: char| c.is_ascii_digit()).unwrap_or(code.len());
    let (kind, number) = code.split_at(split);
    let word = match kind {
        "v" => "Zwrotka",
        "c" => "Refren",
        "b" => "Most",
        "p" => "Przedrefren",
        "e" => "Zakończenie",
        _ => return code.to_owned(),
    };
    if number.is_empty() { word.to_owned() } else { format!("{word} {number}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_codes_become_polish_titles() {
        assert_eq!(section_title("v1"), "Zwrotka 1");
        assert_eq!(section_title("c"), "Refren");
        assert_eq!(section_title("c2"), "Refren 2");
        assert_eq!(section_title("Alleluja"), "Alleluja");
    }
}
