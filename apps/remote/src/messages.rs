// SPDX-License-Identifier: GPL-3.0-or-later
//! User-interface texts. Polish only for now; every string goes through [`Msg`] so adding a
//! language later means adding one function, and a missing text is a compile error.

/// Every text the remote shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Msg {
    TabLive,
    TabSongs,
    LibraryHint,
    DiscardTitle,
    DiscardHint,
    KeepEditing,
    Discard,
    InsertVerse,
    InsertRefrain,
    InsertBridge,
    InsertSlideBreak,
    LyricsPlaceholder,
    HowToWrite,
    HelpVerses,
    HelpRefrain,
    HelpSlide,
    HelpOpenSong,
    HelpKeys,
    Advanced,
    WarnLinesBefore,
    WarnSection,
    WarnTwice,
    WarnUnknownInOrder,
    Line,
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
    NoLogo,
    NoLogoYet,
    UploadLogo,
    RemoveLogo,
    LogoRemoved,
    AddAgain,
    DevicePhone,
    DeviceTablet,
    DeviceComputer,
    Px,
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
    PartVerse,
    PartChorus,
    PartBridge,
    PartPreChorus,
    PartEnding,
    PartIntro,
    PartOther,
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
    Searching,
    SearchFailed,
    Add,
    NewSong,
    Edit,
    EditorTitle,
    EditorNumber,
    EditorOrder,
    EditorLyrics,
    Save,
    SaveAndShow,
    Close,
    Saved,
    Conflict,
    ReviewTitle,
    CopySet,
    CopySetShort,
    SetDate,
    InvalidDate,
    StorageFailed,
    ChurchName,
    ChurchNameExample,
    ChurchMissing,
    ChurchSection,
    ChurchSave,
    ChurchNoneYet,
    ChurchOther,
    ChurchOtherHint,
    ReviewHint,
    NothingToReview,
    CompareTitle,
    VersionShown,
    VersionOther,
    VersionOrder,
    KeepThis,
    VersionKept,
    ToReview,
    SetReviewLine,
    SetsOfChurch,
    ChooseChurch,
    NoChurchesHere,
    SetReviewHint,
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
    Devices,
    ThisDevice,
    Revoke,
    NotPaired,
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
    UpdateInstall,
    UpdateConfirm,
    UpdateInstalling,
    UpdateSendFailed,
    UpdateNoKey,
    UpdateNotSupervised,
    UpdateBadBundle,
    UpdateAlreadyCurrent,
    UpdateFailed,
    UpdateBusy,
    SystemUpdateTitle,
    SystemUpdateWarning,
    SystemUpdateInstalled,
    SystemUpdateOnTrial,
    SystemUpdateWriting,
    SystemUpdateRestarting,
    SystemUpdateChecking,
    SystemUpdateDontSwitchOff,
    SystemUpdateCommitted,
    SystemUpdateRolledBack,
    SectionBackup,
    BackupHint,
    BackupDownload,
    BackupRestore,
    BackupRestoreTitle,
    BackupWarning,
    BackupConfirm,
    BackupRestoring,
    BackupRestored,
    BackupNotSupervised,
    BackupNotABackup,
    BackupNewerVersion,
    BackupOlderVersion,
    BackupTooLarge,
    BackupFailed,
    PictureCleanup,
    PictureCleanupTitle,
    PictureCleanupChecking,
    PictureCleanupNone,
    PictureCleanupKept,
    PictureCleanupConfirm,
    PictureCleanupFailed,
}

/// The Polish text for a message.
pub fn pl(msg: Msg) -> &'static str {
    match msg {
        Msg::TabLive => "Msza",
        Msg::TabSongs => "Pieśni",
        Msg::LibraryHint => {
            "Wyszukaj pieśń, żeby ją poprawić, albo dopisz nową. Gotowe pieśni i zestawy zapisz w \
             pliku (Ustawienia → Synchronizacja → Zapisz plik) i wczytaj go na urządzeniu w kościele \
             (Wczytaj plik)."
        }
        Msg::DiscardTitle => "Odrzucić zmiany?",
        Msg::DiscardHint => "Pieśń ma niezapisane zmiany. Po zamknięciu przepadną.",
        Msg::KeepEditing => "Wróć do edycji",
        Msg::Discard => "Odrzuć zmiany",
        Msg::InsertVerse => "+ Zwrotka",
        Msg::InsertRefrain => "+ Refren",
        Msg::InsertBridge => "+ Most",
        Msg::InsertSlideBreak => "Nowy slajd",
        Msg::LyricsPlaceholder => "1. Pierwsza zwrotka…\n\nRef. Refren…\n\n2. Druga zwrotka…\n\nRef.",
        Msg::HowToWrite => "Jak pisać?",
        Msg::HelpVerses => "Zwrotki oddzielaj pustą linią. Numer na początku („1.”) można pominąć.",
        Msg::HelpRefrain => {
            "Refren zacznij od „Ref.”. Samo „Ref.” w osobnym akapicie powtarza refren w tym miejscu."
        }
        Msg::HelpSlide => "„||” na końcu linii zaczyna nowy slajd; za długa zwrotka i tak podzieli się sama.",
        Msg::HelpOpenSong => "Można też pisać jak w OpenSong: [V1], [C], [B] na początku sekcji.",
        Msg::HelpKeys => "Ctrl+S zapisuje, Ctrl+Enter zapisuje i pokazuje, Esc zamyka.",
        Msg::Advanced => "Zaawansowane",
        Msg::WarnLinesBefore => "tekst przed pierwszym nagłówkiem [..] trafił do zwrotki 1",
        Msg::WarnSection => "sekcja",
        Msg::WarnTwice => "występuje dwa razy – połączono",
        Msg::WarnUnknownInOrder => "kolejność zawiera nieznaną sekcję",
        Msg::Line => "Linia",
        Msg::SectionDisplay => "Wyświetlanie",
        Msg::SectionDevice => "To urządzenie",
        Msg::SectionLibrary => "Synchronizacja",
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
        Msg::NoLogo => "Brak logo",
        Msg::NoLogoYet => "nie wgrano",
        Msg::UploadLogo => "Wgraj logo",
        Msg::RemoveLogo => "Usuń logo",
        Msg::LogoRemoved => "Usunięto logo",
        Msg::AddAgain => "Dodaj jeszcze raz",
        Msg::DevicePhone => "Telefon",
        Msg::DeviceTablet => "Tablet",
        Msg::DeviceComputer => "Komputer",
        Msg::Px => "px",
        Msg::Search => "Szukaj",
        Msg::PrevSong => "Poprzednia pozycja",
        Msg::NextSong => "Następna pozycja",
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
        Msg::PartVerse => "Zwrotka",
        Msg::PartChorus => "Refren",
        Msg::PartBridge => "Most",
        Msg::PartPreChorus => "Przedrefren",
        Msg::PartEnding => "Zakończenie",
        Msg::PartIntro => "Wstęp",
        Msg::PartOther => "Część",
        Msg::Items => "poz.",
        Msg::Next => "Dalej",
        Msg::Prev => "Wstecz",
        Msg::NextItem => "Następna pozycja",
        Msg::PrevItem => "Poprzednia pozycja",
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
        Msg::Searching => "Szukam…",
        Msg::SearchFailed => "Nie udało się zapytać urządzenia – sprawdź połączenie",
        Msg::Add => "Dodaj",
        Msg::NewSong => "Nowa pieśń",
        Msg::Edit => "Edytuj",
        Msg::EditorTitle => "Tytuł",
        Msg::EditorNumber => "Numer",
        Msg::EditorOrder => "Kolejność sekcji (np. V1 C V2 C; puste = jak w tekście)",
        Msg::EditorLyrics => "Tekst",
        Msg::Save => "Zapisz",
        Msg::SaveAndShow => "Zapisz i pokaż na ekranie",
        Msg::Close => "Zamknij",
        Msg::Saved => "Zapisano",
        Msg::Conflict => "Ktoś zapisał tę pieśń w międzyczasie – otwórz ją ponownie",
        Msg::ReviewTitle => "Pieśni do przejrzenia",
        Msg::CopySet => "Nowy zestaw z tego",
        Msg::CopySetShort => "Nowy z tego",
        Msg::SetDate => "Data (niekoniecznie)",
        Msg::InvalidDate => "Niepoprawna data zestawu",
        Msg::StorageFailed => {
            "Urządzenie nie mogło zapisać zmiany (karta pełna lub uszkodzona) – nic się nie zmieniło"
        }
        Msg::ChurchName => "Nazwa kościoła",
        Msg::ChurchNameExample => "np. Tulce",
        Msg::ChurchMissing => "Wpisz nazwę kościoła, w którym stoi to urządzenie",
        Msg::ChurchSection => "Kościół",
        Msg::ChurchSave => "Zapisz nazwę",
        Msg::ChurchNoneYet => {
            "To urządzenie nie ma jeszcze nazwy kościoła; zestawy będą przypisane do niego."
        }
        Msg::ChurchOther => "To urządzenie należy do innego kościoła:",
        Msg::ChurchOtherHint => {
            "Wybierz, jeśli nazwę tego kościoła wpisano tu inaczej niż na innym urządzeniu (np. po \
             wymianie karty)."
        }
        Msg::ReviewHint => {
            "Te pieśni zmieniono w dwóch miejscach, np. w domu i w kościele. Wszędzie pokazywana jest \
             wersja zmieniana więcej razy, a druga czeka tutaj. Wybierz, która ma zostać; potem możesz \
             ją jeszcze poprawić w edytorze."
        }
        Msg::NothingToReview => "Nie ma pieśni do przejrzenia.",
        Msg::CompareTitle => "Porównaj wersje",
        Msg::VersionShown => "Pokazywana teraz",
        Msg::VersionOther => "Druga wersja",
        Msg::VersionOrder => "kolejność:",
        Msg::KeepThis => "Zostaw tę",
        Msg::VersionKept => "Zostawiono wybraną wersję",
        Msg::ToReview => "do przejrzenia",
        Msg::SetsOfChurch => "Zestawy kościoła",
        Msg::ChooseChurch => "Wybierz kościół",
        Msg::NoChurchesHere => {
            "Wczytaj plik z kościoła (Ustawienia → Synchronizacja → Wczytaj plik), żeby przygotowywać \
             jego zestawy. Zestaw zrobiony przed wyborem kościoła pokaże się w każdym kościele."
        }
        Msg::SetReviewLine => "Ten zestaw zmieniono w dwóch miejscach. Porównaj wersje",
        Msg::SetReviewHint => {
            "Wszędzie pokazywana jest wersja zmieniana więcej razy. Zostaw jedną z nich; „Dodaj” przy \
             pozycji, której brakuje w drugiej wersji, przeniesie ją do tej, którą zostawisz."
        }
        Msg::TitleMissing => "Brak tytułu",
        Msg::LyricsMissing => "Brak tekstu pieśni",
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
                           kolejnych pilotów – zapisz go w bezpiecznym miejscu."
        }
        Msg::RepeatPin => "Powtórz PIN",
        Msg::SetPin => "Ustaw PIN",
        Msg::PinsDiffer => "Wpisane PIN-y się różnią",
        Msg::InvalidPin => "PIN musi mieć od 6 do 12 cyfr",
        Msg::PinAlreadySet => "PIN został już ustawiony na innym urządzeniu – połącz się, wpisując go",
        Msg::ImportFailed => "Import nie powiódł się – czy to plik ZIP z folderem OpenSong?",
        Msg::ExportLibrary => "Zapisz plik",
        Msg::MergeLibrary => "Wczytaj plik",
        Msg::MergeChecking => "Sprawdzanie pliku…",
        Msg::MergeWouldDo => "Sprawdzono plik. Po wczytaniu:",
        Msg::MergeConfirm => "Wczytaj",
        Msg::Merging => "Wczytywanie…",
        Msg::MergeDone => "Wczytano.",
        Msg::MergeFailed => "Wczytanie nie powiodło się – czy to plik .sursum z tej wersji programu?",
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
        Msg::InvalidProfile => {
            "Strefy zasłoniłyby cały ekran albo jest ich za dużo (najwyżej 16) – zmiana nie została zapisana"
        }
        Msg::Devices => "Urządzenia",
        Msg::ThisDevice => "to urządzenie",
        Msg::Revoke => "Odłącz",
        Msg::NotPaired => "To urządzenie nie jest już połączone – wpisz PIN ponownie",
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
        Msg::HoldToConfirm => "Przytrzymaj przycisk, aby potwierdzić",
        Msg::ShowPin => "Pokaż",
        Msg::HidePin => "Ukryj",
        Msg::NotSent => "Nie wysłano – brak połączenia z sursum",
        Msg::UpdateInstall => "Zainstaluj aktualizację",
        Msg::UpdateConfirm => "Zainstaluj",
        Msg::UpdateInstalling => "Instalowanie…",
        Msg::UpdateSendFailed => "Nie udało się wysłać pliku.",
        Msg::UpdateNoKey => "Ta wersja programu nie przyjmuje aktualizacji.",
        Msg::UpdateNotSupervised => "To urządzenie nie przyjmuje aktualizacji z pilota.",
        Msg::UpdateBadBundle => {
            "To nie jest aktualizacja dla tego urządzenia: plik jest uszkodzony albo nie ma \
             właściwego podpisu."
        }
        Msg::UpdateAlreadyCurrent => "Ta wersja już działa.",
        Msg::UpdateFailed => "Instalacja nie powiodła się (np. brak miejsca). Działa dotychczasowa wersja.",
        Msg::UpdateBusy => "Trwa już instalowanie aktualizacji. Spróbuj ponownie, gdy się zakończy.",
        Msg::SystemUpdateTitle => "Aktualizacja",
        Msg::SystemUpdateWarning => {
            "Nie aktualizuj w czasie mszy. Urządzenie zapisze nową wersję obok obecnej i uruchomi się \
             ponownie; ekran zgaśnie na kilka minut. Jeśli nowa wersja nie zadziała, urządzenie samo \
             wróci do obecnej. Pieśni, zestawy i ustawienia zostają."
        }
        Msg::SystemUpdateInstalled => {
            "Urządzenie zapisuje nową wersję i za chwilę uruchomi się ponownie. Pilot połączy się sam."
        }
        Msg::SystemUpdateOnTrial => {
            "Nowa wersja jest sprawdzana. Jeśli nie zadziała, urządzenie samo wróci do poprzedniej."
        }
        Msg::SystemUpdateWriting => "Zapisywanie nowej wersji…",
        Msg::SystemUpdateRestarting => "Urządzenie uruchamia się ponownie…",
        Msg::SystemUpdateChecking => "Nowa wersja jest sprawdzana (do 3 minut)…",
        Msg::SystemUpdateDontSwitchOff => {
            "Nie wyłączaj urządzenia. To okno zamknie się, gdy aktualizacja się skończy."
        }
        Msg::SystemUpdateCommitted => "Nowa wersja działa.",
        Msg::SystemUpdateRolledBack => "Nowa wersja nie zadziałała – urządzenie wróciło do poprzedniej.",
        Msg::SectionBackup => "Dane urządzenia",
        Msg::BackupHint => {
            "Pełna kopia: pieśni z historią zmian, zestawy, logo, obrazy, kalibracja i ustawienia \
             (bez PIN-u i sparowanych pilotów). Trzymaj ją poza urządzeniem, np. na komputerze."
        }
        Msg::BackupDownload => "Pobierz kopię",
        Msg::BackupRestore => "Przywróć z kopii",
        Msg::BackupRestoreTitle => "Przywracanie kopii",
        Msg::BackupWarning => {
            "Nie przywracaj w czasie mszy. Pieśni, zestawy, logo, obrazy i ustawienia zostaną \
             zastąpione zawartością kopii; obecne dane zostaną zachowane na urządzeniu. PIN \
             i sparowane piloty się nie zmienią. Program uruchomi się ponownie, a ekran na kilka \
             sekund zgaśnie."
        }
        Msg::BackupConfirm => "Przywróć",
        Msg::BackupRestoring => "Przywracanie…",
        Msg::BackupRestored => "Program uruchamia się ponownie z danymi z kopii. Pilot połączy się sam.",
        Msg::BackupNotSupervised => "Na tym komputerze nie można przywrócić kopii z pilota.",
        Msg::BackupNotABackup => "To nie jest kopia zapasowa sursum albo plik jest uszkodzony.",
        Msg::BackupNewerVersion => "Kopię zrobiła nowsza wersja programu. Najpierw zaktualizuj program.",
        Msg::BackupOlderVersion => {
            "Kopię zrobiła wcześniejsza wersja programu, której danych ta wersja nie przenosi."
        }
        Msg::BackupTooLarge => "Plik jest za duży, by był kopią tego urządzenia.",
        Msg::BackupFailed => {
            "Przywracanie nie powiodło się (np. brak miejsca). Dane urządzenia się nie zmieniły."
        }
        Msg::PictureCleanup => "Usuń nieużywane obrazy",
        Msg::PictureCleanupTitle => "Nieużywane obrazy",
        Msg::PictureCleanupChecking => "Sprawdzanie…",
        Msg::PictureCleanupNone => "Nie ma nieużywanych obrazów.",
        Msg::PictureCleanupKept => {
            "Obrazy z zestawów, logo, to, co jest na ekranie, i obrazy przesłane w ostatnich \
             10 minutach zostają. Usuniętych obrazów nie da się przywrócić (chyba że z kopii)."
        }
        Msg::PictureCleanupConfirm => "Usuń obrazy",
        Msg::PictureCleanupFailed => "Nie udało się sprawdzić ani usunąć obrazów.",
    }
}

/// Why the server did not restore a backup.
pub fn restore_refusal(reason: sursum_protocol::RestoreRefusal) -> Msg {
    use sursum_protocol::RestoreRefusal;
    match reason {
        RestoreRefusal::NotSupervised => Msg::BackupNotSupervised,
        RestoreRefusal::NotABackup => Msg::BackupNotABackup,
        RestoreRefusal::NewerVersion => Msg::BackupNewerVersion,
        RestoreRefusal::OlderVersion => Msg::BackupOlderVersion,
        RestoreRefusal::TooLarge => Msg::BackupTooLarge,
        RestoreRefusal::Failed => Msg::BackupFailed,
    }
}

/// "Nieużywane obrazy: 12 (35,0 MB)" before removing, "Usunięto obrazy: 12 (35,0 MB)" after.
pub fn picture_cleanup_line(cleanup: &sursum_protocol::PictureCleanup, removed: bool) -> String {
    let what = if removed { "Usunięto obrazy" } else { "Nieużywane obrazy" };
    #[allow(clippy::cast_precision_loss, reason = "one decimal of megabytes is enough")]
    let megabytes = cleanup.bytes as f64 / (1024.0 * 1024.0);
    format!("{what}: {} ({} MB)", cleanup.pictures, format!("{megabytes:.1}").replace('.', ","))
}

/// "Kopia z 17.11.2026, 9:05" and "Pieśni: 254, zestawy: 12, obrazy: 3". The date parts come
/// from the browser (local time) as day, month, year, hour, minute.
pub fn backup_summary_lines(summary: &sursum_protocol::BackupSummary, date: [u32; 5]) -> [String; 2] {
    let [day, month, year, hour, minute] = date;
    [
        format!("Kopia z {day}.{month:02}.{year}, {hour}:{minute:02}"),
        format!("Pieśni: {}, zestawy: {}, obrazy: {}", summary.songs, summary.sets, summary.pictures),
    ]
}

/// "sparowano 18.11.2026, 1:15" from day, month, year, hour and minute in local time.
pub fn paired_on(date: [u32; 5]) -> String {
    let [day, month, year, hour, minute] = date;
    format!("sparowano {day}.{month:02}.{year}, {hour}:{minute:02}")
}

/// "nr 12 · kolejność: V1 C V2 C" under a song version's title in the review: the fields that
/// are not in the lyrics, so versions that differ only there do not look the same. Empty when
/// the version has neither.
pub fn song_version_details(number: Option<&str>, presentation: &str) -> String {
    let order = presentation.trim();
    let order = (!order.is_empty()).then(|| format!("{} {order}", pl(Msg::VersionOrder)));
    [Some(song_number(number)).filter(|number| !number.is_empty()), order]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ")
}

/// "nr 12" for a song's number in the hymnal (so it is not mistaken for a count); empty without.
pub fn song_number(number: Option<&str>) -> String {
    number
        .map(str::trim)
        .filter(|number| !number.is_empty())
        .map(|number| format!("nr {number}"))
        .unwrap_or_default()
}

/// Why the server did not install an update.
pub fn install_refusal(reason: sursum_protocol::InstallRefusal) -> Msg {
    use sursum_protocol::InstallRefusal;
    match reason {
        InstallRefusal::NoKey => Msg::UpdateNoKey,
        InstallRefusal::NotSupervised => Msg::UpdateNotSupervised,
        InstallRefusal::BadBundle => Msg::UpdateBadBundle,
        InstallRefusal::AlreadyCurrent => Msg::UpdateAlreadyCurrent,
        InstallRefusal::Failed => Msg::UpdateFailed,
        InstallRefusal::Busy => Msg::UpdateBusy,
    }
}

/// "Zainstalowano wersję 0.3.0." (the server restarts into it next).
pub fn installed_version(version: &str) -> String {
    format!("Zainstalowano wersję {version}.")
}

/// The update lines in "O programie": a developer's build running for a while, an update in
/// progress or on trial, and how the last one ended.
pub fn update_lines(status: &sursum_protocol::UpdateStatus) -> Vec<String> {
    use sursum_protocol::LastSystemUpdate;
    let Some(system) = &status.system else { return Vec::new() };
    let mut lines = Vec::new();
    if status.build != system.version {
        lines.push(format!(
            "Działa wersja testowa {} – po ponownym uruchomieniu urządzenia wróci wersja {}.",
            status.build, system.version
        ));
    }
    if system.installing {
        lines.push("Trwa instalowanie aktualizacji – urządzenie za chwilę uruchomi się ponownie.".to_owned());
    }
    if system.on_trial {
        lines.push(pl(Msg::SystemUpdateOnTrial).to_owned());
    }
    match &system.last {
        Some(LastSystemUpdate::Committed { version }) => {
            lines.push(format!("Ostatnia aktualizacja: wersja {version} działa poprawnie."));
        }
        Some(LastSystemUpdate::RolledBack { failed, database_restored }) => {
            let database = if *database_restored {
                " Pieśni i zestawy wróciły do stanu sprzed aktualizacji."
            } else {
                ""
            };
            lines.push(format!(
                "Aktualizacja do wersji {failed} nie zadziałała – działa poprzednia wersja.{database}"
            ));
        }
        None => {}
    }
    lines
}

/// What to tell the operator when the server refuses a command.
pub fn rejection(code: sursum_protocol::ErrorCode) -> Msg {
    use sursum_protocol::ErrorCode;
    match code {
        ErrorCode::InvalidProfile => Msg::InvalidProfile,
        ErrorCode::SetOnScreen => Msg::SetOnScreen,
        ErrorCode::InvalidDate => Msg::InvalidDate,
        ErrorCode::StorageFailed => Msg::StorageFailed,
        ErrorCode::Forbidden => Msg::NotPaired,
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
    if !report.to_review.is_empty() {
        lines.push(format!(
            "Zmienione i w OpenSong, i w sursum (do przejrzenia): {}",
            report.to_review.join(", ")
        ));
    }
    lines.extend(report.problems.iter().map(|problem| format!("Uwaga: {}", import_problem(problem))));
    lines
}

/// One import problem in Polish, after the file (and line) it is in, e.g.
/// "Barka, linia 3: sekcja [V1] występuje dwa razy – połączono".
pub fn import_problem(problem: &sursum_protocol::ImportProblem) -> String {
    use sursum_protocol::ImportProblemCode as Code;
    let what = match &problem.code {
        Code::LinesBeforeFirstSection => pl(Msg::WarnLinesBefore).to_owned(),
        Code::UnexpectedLineStart { character } => {
            format!("linia zaczyna się od „{character}” zamiast spacji – zostaje jako tekst")
        }
        Code::DuplicateSection { section } => {
            format!("{} [{section}] {}", pl(Msg::WarnSection), pl(Msg::WarnTwice))
        }
        Code::UnknownSectionInOrder { section } => {
            format!("{} „{section}” – pominięto", pl(Msg::WarnUnknownInOrder))
        }
        Code::AssumedWindows1250 => "plik nie jest w UTF-8 – odczytano go jako Windows-1250".to_owned(),
        Code::InvalidBytes => "nieczytelne znaki w pliku zastąpiono znakiem „\u{fffd}”".to_owned(),
        Code::RepeatedElement { element } => format!("<{element}> występuje kilka razy – użyto pierwszego"),
        Code::TitleFromFileName => "brak tytułu – użyto nazwy pliku".to_owned(),
        Code::NoLyrics => "pieśń nie ma tekstu".to_owned(),
        Code::NotOpenSong => "to nie jest pieśń ani zestaw OpenSong – pominięto".to_owned(),
        Code::InvalidXml { detail } => format!("uszkodzony plik (błąd XML: {detail}) – pominięto"),
        Code::Unreadable { detail } => format!("nie udało się odczytać pliku ({detail}) – pominięto"),
        Code::EmptyGroup { group } => format!("pozycja „{group}” nie ma slajdów – pominięto"),
        Code::UnsupportedGroup { group, group_type } => {
            format!("pozycji „{group}” (typ {group_type}) nie da się wczytać – pominięto")
        }
        Code::SongNotFound { song } => format!("pieśni „{song}” nie ma w bibliotece – pominięto"),
        Code::AmbiguousSong { song, count, used } => {
            format!("pieśni o nazwie „{song}”: {count} – użyto {used}")
        }
        Code::UnknownSectionInSetOrder { song, section } => {
            format!("kolejność pieśni „{song}” zawiera nieznaną sekcję „{section}” – pominięto")
        }
        Code::StorageFailed => "nie udało się zapisać – import niczego nie zmienił".to_owned(),
    };
    match (problem.file.as_str(), problem.line) {
        ("", _) => what,
        (file, Some(line)) => format!("{file}, {} {line}: {what}", pl(Msg::Line).to_lowercase()),
        (file, None) => format!("{file}: {what}"),
    }
}

/// The merge report, one line per number; songs changed in two places are listed by title,
/// because someone should look at the other version.
pub fn merge_lines(report: &sursum_protocol::MergeReport) -> Vec<String> {
    let mut lines = vec![
        format!("Nowe pieśni: {}", report.added),
        format!("Zaktualizowane: {}", report.updated),
        format!("Usunięte: {}", report.deleted),
        format!("Bez zmian: {}", report.unchanged),
        format!("Tutaj już nowsze (zostają): {}", report.kept_local),
    ];
    if !report.conflicts.is_empty() {
        lines.push(format!(
            "Zmienione w dwóch miejscach (pokazana wersja z większą liczbą zmian, druga czeka na \
             przejrzenie): {}",
            report.conflicts.join(", ")
        ));
    }
    // Sets and pictures only when the file brought some: most files carry songs only.
    let counts = [
        ("Nowe zestawy", report.sets_added),
        ("Zmienione zestawy", report.sets_updated),
        ("Zestawy innych kościołów (przechowane, niewidoczne tutaj)", report.other_church_sets),
        ("Nowe obrazy", report.pictures_added),
    ];
    lines.extend(
        counts.iter().filter(|(_, count)| *count > 0).map(|(label, count)| format!("{label}: {count}")),
    );
    if !report.sets_to_review.is_empty() {
        lines.push(format!(
            "Zestawy zmienione w dwóch miejscach (do przejrzenia): {}",
            report.sets_to_review.join(", ")
        ));
    }
    lines
}

/// A set's date as people read it: "22.11.2026", with ", 10:00" when it has a time. Anything
/// unexpected is shown as it came.
pub fn set_date_label(date: &str) -> String {
    let (day, time) = date.split_once('T').map_or((date, None), |(day, time)| (day, Some(time)));
    let parts: Vec<&str> = day.split('-').collect();
    let day = match parts.as_slice() {
        [year, month, day] => format!("{day}.{month}.{year}"),
        _ => return date.to_owned(),
    };
    time.map_or(day.clone(), |time| format!("{day}, {time}"))
}

/// "Pieśni do przejrzenia: 2" (songs changed in two places, ADR 0025).
pub fn songs_to_review(count: u32) -> String {
    format!("Pieśni do przejrzenia: {count}")
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

/// Full section name for headings: verse 1 → "Zwrotka 1", chorus → "Refren", verse 1a →
/// "Zwrotka 1a"; a section with a name of its own keeps it ("Alleluja").
pub fn section_title(section: &sursum_protocol::SectionName) -> String {
    use sursum_protocol::SectionKind;
    if let Some(label) = &section.label {
        return label.clone();
    }
    let word = pl(match section.kind {
        SectionKind::Verse => Msg::PartVerse,
        SectionKind::Chorus => Msg::PartChorus,
        SectionKind::Bridge => Msg::PartBridge,
        SectionKind::PreChorus => Msg::PartPreChorus,
        SectionKind::Ending => Msg::PartEnding,
        SectionKind::Intro => Msg::PartIntro,
        SectionKind::Other => Msg::PartOther,
    });
    let number = section.number.map(|number| number.to_string()).unwrap_or_default();
    let suffix = section.suffix.map(String::from).unwrap_or_default();
    if number.is_empty() && suffix.is_empty() { word.to_owned() } else { format!("{word} {number}{suffix}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_become_polish_titles() {
        use sursum_protocol::{SectionKind, SectionName};
        let title = |kind, number, suffix, label: Option<&str>| {
            section_title(&SectionName { kind, number, suffix, label: label.map(str::to_owned) })
        };
        assert_eq!(title(SectionKind::Verse, Some(1), None, None), "Zwrotka 1");
        assert_eq!(title(SectionKind::Chorus, None, None, None), "Refren");
        assert_eq!(title(SectionKind::Chorus, Some(2), None, None), "Refren 2");
        assert_eq!(title(SectionKind::Verse, Some(1), Some('a'), None), "Zwrotka 1a");
        assert_eq!(title(SectionKind::Other, Some(1), None, Some("Alleluja")), "Alleluja");
    }

    #[test]
    fn import_problems_name_the_file_and_line_in_polish() {
        use sursum_protocol::{ImportProblem, ImportProblemCode};
        let duplicate = ImportProblem {
            file: "Barka".into(),
            line: Some(3),
            code: ImportProblemCode::DuplicateSection { section: "V1".into() },
        };
        assert_eq!(import_problem(&duplicate), "Barka, linia 3: sekcja [V1] występuje dwa razy – połączono");
        let missing = ImportProblem {
            file: "Sets/Niedziela".into(),
            line: None,
            code: ImportProblemCode::SongNotFound { song: "Pieśń".into() },
        };
        assert_eq!(
            import_problem(&missing),
            "Sets/Niedziela: pieśni „Pieśń” nie ma w bibliotece – pominięto"
        );
        let failed =
            ImportProblem { file: String::new(), line: None, code: ImportProblemCode::StorageFailed };
        assert_eq!(import_problem(&failed), "nie udało się zapisać – import niczego nie zmienił");
    }

    #[test]
    fn a_song_version_names_its_number_and_order() {
        assert_eq!(song_version_details(Some("12"), "V1 C V2 C"), "nr 12 · kolejność: V1 C V2 C");
        assert_eq!(song_version_details(None, "V1 C"), "kolejność: V1 C");
        assert_eq!(song_version_details(Some(" "), ""), "");
    }

    #[test]
    fn a_backup_summary_reads_as_a_date_and_counts() {
        let summary = sursum_protocol::BackupSummary {
            created_unix: 0,
            build: "0.1.0".into(),
            songs: 254,
            sets: 12,
            pictures: 3,
        };
        assert_eq!(
            backup_summary_lines(&summary, [7, 3, 2026, 9, 5]),
            ["Kopia z 7.03.2026, 9:05", "Pieśni: 254, zestawy: 12, obrazy: 3"]
        );
    }

    #[test]
    fn a_picture_cleanup_reads_in_megabytes_with_a_decimal_comma() {
        let cleanup = sursum_protocol::PictureCleanup { pictures: 12, bytes: 36_700_160 };
        assert_eq!(picture_cleanup_line(&cleanup, false), "Nieużywane obrazy: 12 (35,0 MB)");
        assert_eq!(picture_cleanup_line(&cleanup, true), "Usunięto obrazy: 12 (35,0 MB)");
    }

    #[test]
    fn about_names_a_test_build_and_a_rollback_that_restored_the_songs() {
        use sursum_protocol::{LastSystemUpdate, SystemStatus, UpdateStatus};
        let system = SystemStatus {
            version: "v0.2.0".into(),
            on_trial: false,
            installing: false,
            target: None,
            last: Some(LastSystemUpdate::RolledBack { failed: "v0.3.0".into(), database_restored: true }),
            can_install: true,
        };
        let status = UpdateStatus { build: "v0.2.0".into(), system: Some(system) };
        assert_eq!(
            update_lines(&status),
            ["Aktualizacja do wersji v0.3.0 nie zadziałała – działa poprzednia wersja. Pieśni i zestawy \
              wróciły do stanu sprzed aktualizacji."]
        );
        let testing = UpdateStatus { build: "v0.2.0-1-gabc-dirty".into(), ..status };
        assert!(update_lines(&testing)[0].starts_with("Działa wersja testowa v0.2.0-1-gabc-dirty"));
        assert!(update_lines(&UpdateStatus { build: "dev".into(), system: None }).is_empty());
    }
}
