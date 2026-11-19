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
    UpdateTitle,
    UpdateWarning,
    UpdateConfirm,
    UpdateInstalling,
    UpdateInstalled,
    UpdateSendFailed,
    UpdateNoKey,
    UpdateNotSupervised,
    UpdateBadBundle,
    UpdateAlreadyCurrent,
    UpdateFailed,
    UpdateOnTrial,
    UpdateBusy,
    SystemUpdateInstall,
    SystemUpdateTitle,
    SystemUpdateWarning,
    SystemUpdateInstalled,
    SystemUpdateOnTrial,
    SystemUpdateWriting,
    ProgramUpdateRestarting,
    ProgramUpdateChecking,
    ProgramUpdateCommitted,
    ProgramUpdateRolledBack,
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
            "Wyszukaj pieśń, żeby ją poprawić, albo dopisz nową. Gotową bazę zapisz \
             w Ustawieniach (Eksport bazy) i scal na urządzeniu w kościele (Scal bazę z pliku)."
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
        Msg::NoLogo => "Brak logo",
        Msg::NoLogoYet => "nie wgrano",
        Msg::UploadLogo => "Wgraj logo",
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
        Msg::UpdateTitle => "Aktualizacja programu",
        Msg::UpdateWarning => {
            "Nie aktualizuj w czasie mszy. Program uruchomi się ponownie, a ekran na kilka sekund \
             zgaśnie. Jeśli nowa wersja nie zadziała, urządzenie samo wróci do obecnej."
        }
        Msg::UpdateConfirm => "Zainstaluj",
        Msg::UpdateInstalling => "Instalowanie…",
        Msg::UpdateInstalled => "Program uruchamia się ponownie. Pilot odświeży się sam.",
        Msg::UpdateSendFailed => "Nie udało się wysłać pliku.",
        Msg::UpdateNoKey => "Ta wersja programu nie przyjmuje aktualizacji.",
        Msg::UpdateNotSupervised => "Na tym komputerze aktualizację instaluje się ręcznie.",
        Msg::UpdateBadBundle => {
            "To nie jest aktualizacja dla tego urządzenia: plik jest uszkodzony albo nie ma \
             właściwego podpisu."
        }
        Msg::UpdateAlreadyCurrent => "Ta wersja już działa.",
        Msg::UpdateFailed => "Instalacja nie powiodła się (np. brak miejsca). Działa dotychczasowa wersja.",
        Msg::UpdateOnTrial => {
            "Nowa wersja jest sprawdzana. Jeśli nie zadziała, urządzenie samo wróci do poprzedniej."
        }
        Msg::UpdateBusy => "Trwa już instalowanie aktualizacji. Spróbuj ponownie, gdy się zakończy.",
        Msg::SystemUpdateInstall => "Zainstaluj aktualizację systemu",
        Msg::SystemUpdateTitle => "Aktualizacja systemu",
        Msg::SystemUpdateWarning => {
            "Nie aktualizuj w czasie mszy. Urządzenie zapisze nowy system obok obecnego i uruchomi się \
             ponownie; ekran zgaśnie na kilka minut. Jeśli nowy system nie zadziała, urządzenie samo \
             wróci do obecnego. Pieśni, zestawy i ustawienia zostają."
        }
        Msg::SystemUpdateInstalled => {
            "Urządzenie zapisuje nowy system i za chwilę uruchomi się ponownie. Pilot połączy się sam."
        }
        Msg::SystemUpdateOnTrial => {
            "Nowy system jest sprawdzany. Jeśli nie zadziała, urządzenie samo wróci do poprzedniego."
        }
        Msg::SystemUpdateWriting => "Zapisywanie nowego systemu…",
        Msg::ProgramUpdateRestarting => "Program uruchamia się ponownie…",
        Msg::ProgramUpdateChecking => "Nowa wersja programu jest sprawdzana (około pół minuty)…",
        Msg::ProgramUpdateCommitted => "Nowa wersja programu działa.",
        Msg::ProgramUpdateRolledBack => "Nowa wersja programu nie zadziałała – działa poprzednia.",
        Msg::SystemUpdateRestarting => "Urządzenie uruchamia się ponownie…",
        Msg::SystemUpdateChecking => "Nowy system jest sprawdzany (do 3 minut)…",
        Msg::SystemUpdateDontSwitchOff => {
            "Nie wyłączaj urządzenia. To okno zamknie się, gdy aktualizacja się skończy."
        }
        Msg::SystemUpdateCommitted => "Nowy system działa.",
        Msg::SystemUpdateRolledBack => "Nowy system nie zadziałał – urządzenie wróciło do poprzedniego.",
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

/// How the most recent update ended, for "O programie".
pub fn last_update(last: &sursum_protocol::LastUpdate) -> String {
    use sursum_protocol::LastUpdate;
    match last {
        LastUpdate::Committed { version } => {
            format!("Ostatnia aktualizacja: wersja {version} działa poprawnie.")
        }
        LastUpdate::RolledBack { failed, restored, database_restored } => {
            let database =
                if *database_restored { " Baza pieśni wróciła do stanu sprzed aktualizacji." } else { "" };
            format!(
                "Aktualizacja do wersji {failed} nie zadziałała – przywrócono wersję {restored}.{database}"
            )
        }
    }
}

/// The system's lines in "O programie": its version, a trial in progress, the last update.
pub fn system_lines(system: &sursum_protocol::SystemStatus) -> Vec<String> {
    use sursum_protocol::LastSystemUpdate;
    let mut lines = vec![format!("System: {}", system.version)];
    if system.installing {
        lines.push(
            "Trwa instalowanie aktualizacji systemu – urządzenie za chwilę uruchomi się ponownie.".to_owned(),
        );
    }
    if system.on_trial {
        lines.push(pl(Msg::SystemUpdateOnTrial).to_owned());
    }
    match &system.last {
        Some(LastSystemUpdate::Committed { version }) => {
            lines.push(format!("Ostatnia aktualizacja systemu: wersja {version} działa poprawnie."));
        }
        Some(LastSystemUpdate::RolledBack { failed }) => {
            lines.push(format!("Aktualizacja systemu ({failed}) nie zadziałała – działa poprzedni system."));
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
}
